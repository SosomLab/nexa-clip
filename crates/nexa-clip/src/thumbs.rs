//! ★ 섬네일 상주 캐시(09-04 · [docs/30 §2 T·§4](../../../docs/30-memory-residency.md) · DR-42 ⑤).
//!
//! 섬네일은 저장소에 **PNG blob**으로 있고(인덱스엔 참조만), RAM에는 **화면 근처 것만** 디코드본으로 든다.
//! - 그리는 쪽(메인·팝업)은 [`ThumbCache::get`]으로 꺼내고, 없으면 [`ThumbCache::want`]으로 **요청만** 남긴다
//!   (그리기 루프는 보이는 행만 돌기 때문에 요청도 뷰포트만 — DR-41 요청/수행 분리).
//! - 셸이 박동마다 [`ThumbCache::take_wanted`]로 최대 4개를 꺼내 워커 스레드에 디코드를 맡기고, 결과는
//!   `ShellEvent::ThumbReady`로 돌아와 [`ThumbCache::insert`]된다.
//! - LRU 상한(`cap` 장) — 384² RGBA ≈ 590KB/장 · 32장 ≈ 19MB. 고정 항목은 축출 제외(DR-42 ①).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use nclip_ctl::theme::IconImage;

/// 메인·팝업·셸이 나눠 드는 손잡이.
pub(crate) type Thumbs = Rc<RefCell<ThumbCache>>;

pub(crate) struct ThumbCache {
    imgs: HashMap<u64, Rc<IconImage>>,
    /// 최근 사용 순(뒤가 최신).
    lru: VecDeque<u64>,
    /// 축출 제외(고정 항목 — DR-42 ①).
    keep: HashSet<u64>,
    /// 워커에 나가 있는 것.
    pending: HashSet<u64>,
    /// 그리기 루프가 남긴 요청(중복 없음).
    wanted: Vec<u64>,
    cap: usize,
}

impl ThumbCache {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            imgs: HashMap::new(),
            lru: VecDeque::new(),
            keep: HashSet::new(),
            pending: HashSet::new(),
            wanted: Vec::new(),
            cap: cap.max(1),
        }
    }

    /// 디코드본 — 있으면 LRU 갱신.
    pub(crate) fn get(&mut self, id: u64) -> Option<Rc<IconImage>> {
        let img = self.imgs.get(&id).cloned()?;
        if let Some(pos) = self.lru.iter().position(|&k| k == id) {
            self.lru.remove(pos);
        }
        self.lru.push_back(id);
        Some(img)
    }

    /// 요청만 남긴다(이미 있거나 나가 있으면 무시).
    pub(crate) fn want(&mut self, id: u64) {
        if self.imgs.contains_key(&id) || self.pending.contains(&id) || self.wanted.contains(&id) {
            return;
        }
        self.wanted.push(id);
    }

    /// 요청을 최대 `max`개 꺼내 진행 중으로 옮긴다(동시 디코드 상한은 호출자가 정한다).
    pub(crate) fn take_wanted(&mut self, max: usize) -> Vec<u64> {
        let room = max.saturating_sub(self.pending.len());
        let n = room.min(self.wanted.len());
        // 최근 요청(= 지금 보이는 행)부터.
        let out: Vec<u64> = self.wanted.drain(self.wanted.len() - n..).rev().collect();
        for id in &out {
            self.pending.insert(*id);
        }
        out
    }

    /// 디코드 결과 — LRU 상한을 넘으면 오래된(고정 아닌) 것부터 내린다.
    pub(crate) fn insert(&mut self, id: u64, img: IconImage, keep: bool) {
        self.pending.remove(&id);
        self.imgs.insert(id, Rc::new(img));
        if let Some(pos) = self.lru.iter().position(|&k| k == id) {
            self.lru.remove(pos);
        }
        self.lru.push_back(id);
        if keep {
            self.keep.insert(id);
        }
        self.trim(self.cap);
    }

    /// 실패 — 진행 중 표시만 지운다(다음 그리기가 다시 요청할 수 있다 · 없는 blob은 호출자가 걸러 준다).
    pub(crate) fn fail(&mut self, id: u64) {
        self.pending.remove(&id);
    }

    pub(crate) fn remove(&mut self, id: u64) {
        self.imgs.remove(&id);
        self.keep.remove(&id);
        self.pending.remove(&id);
        self.wanted.retain(|&k| k != id);
        self.lru.retain(|&k| k != id);
    }

    /// 고정 토글 반영.
    pub(crate) fn set_keep(&mut self, id: u64, keep: bool) {
        if keep {
            self.keep.insert(id);
        } else {
            self.keep.remove(&id);
        }
    }

    /// `cap`장까지 내린다(고정 제외 · 오래된 것부터).
    pub(crate) fn trim(&mut self, cap: usize) {
        let mut over = self.imgs.len().saturating_sub(cap);
        let mut k = 0usize;
        while over > 0 && k < self.lru.len() {
            let id = self.lru[k];
            if self.keep.contains(&id) {
                k += 1;
                continue;
            }
            self.lru.remove(k);
            self.imgs.remove(&id);
            over -= 1;
        }
    }

    /// 상주 바이트(회계용).
    pub(crate) fn bytes(&self) -> u64 {
        self.imgs.values().map(|i| i.rgba.len() as u64).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img() -> IconImage {
        IconImage::from_rgba(1, 1, vec![0, 0, 0, 255])
    }

    /// 요청은 중복 없이 · 최근 것부터 · 상한만큼 · 진행 중이면 다시 요청되지 않는다 · LRU 축출은 고정 제외.
    #[test]
    fn want_take_insert_trim() {
        let mut c = ThumbCache::new(2);
        c.want(1);
        c.want(2);
        c.want(2);
        c.want(3);
        assert_eq!(c.take_wanted(2), vec![3, 2]);
        c.want(3); // 진행 중 — 재요청 안 됨
        assert_eq!(
            c.take_wanted(4),
            vec![1],
            "상한 4 − 진행 2 = 2 자리 · 남은 요청 1"
        );
        assert!(c.take_wanted(4).is_empty());
        c.insert(3, img(), true);
        c.insert(2, img(), false);
        c.insert(1, img(), false); // cap 2 → 가장 오래된 비고정(2) 축출 · 3은 고정이라 남는다
        assert!(c.get(3).is_some() && c.get(1).is_some() && c.get(2).is_none());
        assert_eq!(c.imgs.len(), 2);
        c.set_keep(3, false);
        c.trim(1);
        assert_eq!(c.imgs.len(), 1);
        c.remove(1);
        assert_eq!(c.imgs.len(), 0);
    }
}
