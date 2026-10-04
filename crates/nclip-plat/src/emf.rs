//! EMF(확장 메타파일) → RGBA 래스터화 — ★ **파서를 링크하지 않는다: OS(GDI)가 그린다**(DR-8).
//!
//! PPT 글상자·도형 복사는 래스터 표현(PNG/DIB) 없이 `CF_ENHMETAFILE`·SVG만 준다
//! (09-02 실기). SVG 렌더러는 없지만 EMF는 `PlayEnhMetaFile`로 Windows가 직접
//! 그려 주므로, 서식(색·굵기) 그대로의 미리보기를 공짜로 얻는다.
//!
//! ## 크기 정책
//! EMF는 벡터라 어느 해상도로든 무손실 확대가 된다 — 긴 변은 `max_side`로 죄되,
//! **짧은 변이 96px는 되게** 끌어올린다(와가로 글상자가 행 존·썸네일에서 뭉개지지
//! 않게). 총화소 4M 상한(RGBA 16MiB)으로 폭주를 막는다.

#![cfg(target_os = "windows")]

type Handle = isize;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Sizel {
    cx: i32,
    cy: i32,
}

/// `ENHMETAHEADER` v1(88바이트) — 프레임(0.01mm)만 읽으면 된다.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct EnhMetaHeader {
    i_type: u32,
    n_size: u32,
    rcl_bounds: Rect,
    rcl_frame: Rect,
    d_signature: u32,
    n_version: u32,
    n_bytes: u32,
    n_records: u32,
    n_handles: u16,
    s_reserved: u16,
    n_description: u32,
    off_description: u32,
    n_pal_entries: u32,
    szl_device: Sizel,
    szl_millimeters: Sizel,
}

#[repr(C)]
struct BitmapInfoHeader {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_ppm: i32,
    bi_y_ppm: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[link(name = "gdi32")]
extern "system" {
    fn SetEnhMetaFileBits(cb: u32, bytes: *const u8) -> Handle;
    fn GetEnhMetaFileHeader(hemf: Handle, cb: u32, out: *mut EnhMetaHeader) -> u32;
    fn DeleteEnhMetaFile(hemf: Handle) -> i32;
    fn PlayEnhMetaFile(hdc: Handle, hemf: Handle, rect: *const Rect) -> i32;
    fn CreateCompatibleDC(hdc: Handle) -> Handle;
    fn CreateDIBSection(
        hdc: Handle,
        bmi: *const BitmapInfoHeader,
        usage: u32,
        bits: *mut *mut core::ffi::c_void,
        section: Handle,
        offset: u32,
    ) -> Handle;
    fn SelectObject(hdc: Handle, obj: Handle) -> Handle;
    fn DeleteObject(obj: Handle) -> i32;
    fn DeleteDC(hdc: Handle) -> i32;
    fn GdiFlush() -> i32;
}

/// 총화소 상한 — RGBA 16MiB.
const PIXELS_MAX: u64 = 4_000_000;

/// EMF 바이트 → RGBA. 실패는 전부 `None`(래스터 폴백 없음 = 글리프 폴백).
#[must_use]
pub fn emf_to_rgba(bytes: &[u8], max_side: u32) -> Option<(u32, u32, Vec<u8>)> {
    if bytes.len() < 88 || max_side == 0 {
        return None;
    }
    // SAFETY: 실패는 전부 널/0으로 돌아오고, 만든 핸들은 아래에서 짝 맞춰 지운다.
    unsafe {
        let hemf = SetEnhMetaFileBits(bytes.len() as u32, bytes.as_ptr());
        if hemf == 0 {
            return None;
        }
        let out = raster(hemf, max_side);
        DeleteEnhMetaFile(hemf);
        out
    }
}

/// EMF 바이트 → **클립보드에 올릴 `HENHMETAFILE` 핸들**(10-04). 실패는 `None`.
///
/// `CF_ENHMETAFILE`은 핸들 포맷이라 바이트를 `HGLOBAL`에 담아 올리면 안 된다 —
/// 받는 앱이 그 메모리 핸들을 메타파일로 쓰려다 실패한다. `SetClipboardData`가 성공하면
/// 핸들은 **시스템 소유**이고, 실패했을 때만 [`delete_handle`]로 지운다.
#[must_use]
pub fn handle_from_bytes(bytes: &[u8]) -> Option<isize> {
    if bytes.len() < 88 {
        return None;
    }
    // SAFETY: 길이·포인터가 같은 슬라이스에서 나온다. 실패는 0.
    let h = unsafe { SetEnhMetaFileBits(bytes.len() as u32, bytes.as_ptr()) };
    (h != 0).then_some(h)
}

/// [`handle_from_bytes`]가 만든 핸들을 지운다(클립보드에 넘기지 못했을 때만).
pub fn delete_handle(hemf: isize) {
    // SAFETY: 우리가 만든 핸들을 한 번 지운다.
    unsafe {
        DeleteEnhMetaFile(hemf);
    }
}

/// EMF 바이트 → **`CF_DIB` 바이트**(BITMAPINFOHEADER · 32bpp · 바텀업 BGRA · 흰 바탕).
///
/// 그림판처럼 **비트맵만 받는 앱**에 붙이기 위한 것(10-04 사용자 — "mspaint 기준으로 이미지
/// 붙여넣기가 유지되게"). 감시는 Excel 범위의 비트맵을 받지 않으므로(수백 MB 위험)
/// 붙여넣는 순간에 벡터 그림에서 만든다 — 보관하지 않는다.
#[must_use]
pub fn dib_from_bytes(bytes: &[u8], max_side: u32) -> Option<Vec<u8>> {
    let (w, h, rgba) = emf_to_rgba(bytes, max_side)?;
    let (wu, hu) = (w as usize, h as usize);
    let mut out = Vec::with_capacity(40 + rgba.len());
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes()); // 양수 = 바텀업
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bpp
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&((wu * hu * 4) as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]); // ppm×2 · clrUsed · clrImportant
    for row in (0..hu).rev() {
        for px in rgba[row * wu * 4..(row + 1) * wu * 4].chunks_exact(4) {
            out.extend_from_slice(&[px[2], px[1], px[0], 0xFF]);
        }
    }
    Some(out)
}

/// 본체 — `hemf` 정리는 호출자 몫.
unsafe fn raster(hemf: Handle, max_side: u32) -> Option<(u32, u32, Vec<u8>)> {
    unsafe {
        let mut hdr = EnhMetaHeader::default();
        if GetEnhMetaFileHeader(hemf, core::mem::size_of::<EnhMetaHeader>() as u32, &mut hdr) == 0 {
            return None;
        }
        // 프레임(0.01mm) → 96dpi 픽셀. 퇴화 프레임은 rclBounds(장치 픽셀)로 폴백.
        let (fw, fh) = (
            i64::from(hdr.rcl_frame.right - hdr.rcl_frame.left),
            i64::from(hdr.rcl_frame.bottom - hdr.rcl_frame.top),
        );
        let (mut nw, mut nh) = (fw * 96 / 2540, fh * 96 / 2540);
        if nw <= 0 || nh <= 0 {
            nw = i64::from(hdr.rcl_bounds.right - hdr.rcl_bounds.left);
            nh = i64::from(hdr.rcl_bounds.bottom - hdr.rcl_bounds.top);
        }
        if nw <= 0 || nh <= 0 {
            return None;
        }
        // 배율 — 긴 변 ≤ max_side · 총화소 ≤ 4M. ★ 확대 보정 없음(09-02 실기 —
        //   표시가 문서 논리 크기 기준이 된 뒤로는 키웠다 줄이는 이중 왜곡만 남는다.
        //   논리 크기로 "작게 다시 그리는" 쪽이 GDI 글자 힌팅도 살아 선명하다).
        let long = nw.max(nh) as f64;
        let scale = if long > f64::from(max_side) {
            f64::from(max_side) / long
        } else {
            1.0
        };
        let mut scale = scale;
        let cap = (PIXELS_MAX as f64 / (nw as f64 * nh as f64)).sqrt();
        if scale > cap {
            scale = cap;
        }
        let w = ((nw as f64 * scale).round() as i64).clamp(1, 8192) as i32;
        let h = ((nh as f64 * scale).round() as i64).clamp(1, 8192) as i32;

        let bmi = BitmapInfoHeader {
            bi_size: core::mem::size_of::<BitmapInfoHeader>() as u32,
            bi_width: w,
            bi_height: -h, // 톱다운.
            bi_planes: 1,
            bi_bit_count: 32,
            bi_compression: 0, // BI_RGB.
            bi_size_image: 0,
            bi_x_ppm: 0,
            bi_y_ppm: 0,
            bi_clr_used: 0,
            bi_clr_important: 0,
        };
        let dc = CreateCompatibleDC(0);
        if dc == 0 {
            return None;
        }
        let mut bits: *mut core::ffi::c_void = core::ptr::null_mut();
        let bmp = CreateDIBSection(dc, &bmi, 0 /* DIB_RGB_COLORS */, &mut bits, 0, 0);
        if bmp == 0 || bits.is_null() {
            if bmp != 0 {
                DeleteObject(bmp);
            }
            DeleteDC(dc);
            return None;
        }
        let old = SelectObject(dc, bmp);
        let n = (w as usize) * (h as usize);
        // 흰 바탕 — PPT 글상자는 투명 배경을 흰 종이에 그린 모양이 원본과 같다.
        core::ptr::write_bytes(bits.cast::<u8>(), 0xFF, n * 4);
        let rect = Rect {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        let played = PlayEnhMetaFile(dc, hemf, &rect);
        GdiFlush();
        let out = if played != 0 {
            // BGRA(알파는 GDI가 안 쓴다) → RGBA(불투명).
            let src = core::slice::from_raw_parts(bits.cast::<u8>(), n * 4);
            let mut rgba = Vec::with_capacity(n * 4);
            for px in src.chunks_exact(4) {
                rgba.extend_from_slice(&[px[2], px[1], px[0], 0xFF]);
            }
            Some((w as u32, h as u32, rgba))
        } else {
            None
        };
        SelectObject(dc, old);
        DeleteObject(bmp);
        DeleteDC(dc);
        out
    }
}
