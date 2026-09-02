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
        // 배율 — 긴 변 ≤ max_side · ★ 짧은 변 ≥ 96(벡터 확대 무손실) · 총화소 ≤ 4M.
        let (long, short) = (nw.max(nh) as f64, nw.min(nh) as f64);
        let mut scale = if long > f64::from(max_side) {
            f64::from(max_side) / long
        } else {
            1.0
        };
        let min_scale = (96.0 / short).min(4.0);
        if scale < min_scale {
            scale = min_scale;
        }
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
