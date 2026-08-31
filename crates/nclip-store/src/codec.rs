//! 레코드 바이너리 코덱 — **serde를 링크하지 않는다**(DR-8). 필드는 전부 길이 명시형이라
//! 앞에서부터 읽다 모자라면 그 지점에서 None(잘린 꼬리 = 크래시 관용).
//!
//! 문법: u8 · u32/u64(LE) · bytes(u32 len ‖ 원문) · str(bytes의 UTF-8) · opt(u8 태그 ‖ 값).

pub(crate) struct W(pub Vec<u8>);

impl W {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }
    pub(crate) fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub(crate) fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn bytes(&mut self, v: &[u8]) {
        self.u32(u32::try_from(v.len()).unwrap_or(u32::MAX));
        self.0.extend_from_slice(v);
    }
    pub(crate) fn str(&mut self, v: &str) {
        self.bytes(v.as_bytes());
    }
    pub(crate) fn opt_str(&mut self, v: Option<&str>) {
        match v {
            Some(s) => {
                self.u8(1);
                self.str(s);
            }
            None => self.u8(0),
        }
    }
}

pub(crate) struct R<'a>(pub &'a [u8]);

impl<'a> R<'a> {
    pub(crate) fn u8(&mut self) -> Option<u8> {
        let (&v, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(v)
    }
    pub(crate) fn u32(&mut self) -> Option<u32> {
        let (v, rest) = self.0.split_at_checked(4)?;
        self.0 = rest;
        Some(u32::from_le_bytes(v.try_into().ok()?))
    }
    pub(crate) fn u64(&mut self) -> Option<u64> {
        let (v, rest) = self.0.split_at_checked(8)?;
        self.0 = rest;
        Some(u64::from_le_bytes(v.try_into().ok()?))
    }
    pub(crate) fn bytes(&mut self) -> Option<&'a [u8]> {
        let n = self.u32()? as usize;
        let (v, rest) = self.0.split_at_checked(n)?;
        self.0 = rest;
        Some(v)
    }
    pub(crate) fn str(&mut self) -> Option<String> {
        String::from_utf8(self.bytes()?.to_vec()).ok()
    }
    pub(crate) fn opt_str(&mut self) -> Option<Option<String>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(self.str()?)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_shapes() {
        let mut w = W::new();
        w.u8(3);
        w.u32(70_000);
        w.u64(u64::MAX - 1);
        w.bytes(b"\x00raw\xff");
        w.str("한글");
        w.opt_str(None);
        w.opt_str(Some("x"));
        let mut r = R(&w.0);
        assert_eq!(r.u8(), Some(3));
        assert_eq!(r.u32(), Some(70_000));
        assert_eq!(r.u64(), Some(u64::MAX - 1));
        assert_eq!(r.bytes(), Some(&b"\x00raw\xff"[..]));
        assert_eq!(r.str().as_deref(), Some("한글"));
        assert_eq!(r.opt_str(), Some(None));
        assert_eq!(r.opt_str(), Some(Some("x".into())));
        assert!(r.0.is_empty());
    }

    /// 잘린 입력은 그 지점에서 None — 패닉 금지(디스크는 남의 데이터일 수 있다).
    #[test]
    fn truncated_input_is_none_not_panic() {
        let mut w = W::new();
        w.str("hello");
        for cut in 0..w.0.len() {
            let mut r = R(&w.0[..cut]);
            assert!(r.str().is_none());
        }
    }
}
