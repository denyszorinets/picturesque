//! Minimal FFI to LibRaw (thread-safe `libraw_r`): embedded preview
//! extraction only (ADR-0005). This is the only module in the workspace
//! that uses `unsafe`.

use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::path::Path;

#[repr(C)]
struct ProcessedImage {
    kind: c_int,
    height: u16,
    width: u16,
    colors: u16,
    bits: u16,
    data_size: c_uint,
    data: [u8; 1],
}

const LIBRAW_IMAGE_JPEG: c_int = 1;
const LIBRAW_IMAGE_BITMAP: c_int = 2;

#[link(name = "raw_r")]
unsafe extern "C" {
    fn libraw_init(flags: c_uint) -> *mut c_void;
    fn libraw_open_file(lr: *mut c_void, file: *const c_char) -> c_int;
    fn libraw_unpack_thumb(lr: *mut c_void) -> c_int;
    fn libraw_dcraw_make_mem_thumb(lr: *mut c_void, errc: *mut c_int) -> *mut ProcessedImage;
    fn libraw_dcraw_clear_mem(img: *mut ProcessedImage);
    fn libraw_close(lr: *mut c_void);
    fn libraw_strerror(err: c_int) -> *const c_char;
}

/// An embedded preview.
#[derive(Debug)]
pub enum Preview {
    Jpeg(Vec<u8>),
    /// 8-bit interleaved pixels.
    Bitmap {
        width: u32,
        height: u32,
        bands: u32,
        pixels: Vec<u8>,
    },
}

struct Handle(*mut c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the pointer came from libraw_init and is closed once.
        unsafe { libraw_close(self.0) }
    }
}

fn check(code: c_int, what: &str) -> Result<(), String> {
    if code == 0 {
        return Ok(());
    }
    // SAFETY: libraw_strerror returns a static NUL-terminated string.
    let msg = unsafe { CStr::from_ptr(libraw_strerror(code)) }.to_string_lossy();
    Err(format!("{what}: {msg}"))
}

/// Extracts the largest embedded preview. `Ok(None)` when the file has none.
pub fn preview(path: &Path) -> Result<Option<Preview>, String> {
    // SAFETY: libraw_init has no preconditions; a null result is handled.
    let lr = unsafe { libraw_init(0) };
    if lr.is_null() {
        return Err("LibRaw failed to initialize".into());
    }
    let handle = Handle(lr);
    let c_path = CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    // SAFETY: handle.0 is a live LibRaw handle; c_path outlives the call.
    check(
        unsafe { libraw_open_file(handle.0, c_path.as_ptr()) },
        "open",
    )?;
    // SAFETY: handle.0 is a live, opened LibRaw handle.
    let unpacked = unsafe { libraw_unpack_thumb(handle.0) };
    if unpacked != 0 {
        // LIBRAW_NO_THUMBNAIL and similar: no usable preview.
        return Ok(None);
    }
    let mut err = 0;
    // SAFETY: handle.0 is live and its thumbnail was unpacked.
    let img = unsafe { libraw_dcraw_make_mem_thumb(handle.0, &mut err) };
    if img.is_null() {
        return if err == 0 {
            Ok(None)
        } else {
            check(err, "preview").map(|_| None)
        };
    }
    // SAFETY: img is a valid ProcessedImage whose `data` holds `data_size`
    // bytes; it is copied before being freed exactly once below.
    let preview = unsafe {
        let p = &*img;
        let bytes = std::slice::from_raw_parts(p.data.as_ptr(), p.data_size as usize).to_vec();
        match p.kind {
            LIBRAW_IMAGE_JPEG => Some(Preview::Jpeg(bytes)),
            LIBRAW_IMAGE_BITMAP if p.bits == 8 && p.colors > 0 => Some(Preview::Bitmap {
                width: u32::from(p.width),
                height: u32::from(p.height),
                bands: u32::from(p.colors),
                pixels: bytes,
            }),
            _ => None,
        }
    };
    // SAFETY: img came from libraw_dcraw_make_mem_thumb and is freed once.
    unsafe { libraw_dcraw_clear_mem(img) };
    Ok(preview)
}
