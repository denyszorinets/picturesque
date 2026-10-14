//! Minimal hand-written FFI to LibRaw's C API: just what the spike needs.

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

#[link(name = "raw_r")]
unsafe extern "C" {
    fn libraw_init(flags: c_uint) -> *mut c_void;
    fn libraw_open_file(lr: *mut c_void, file: *const c_char) -> c_int;
    fn libraw_unpack_thumb(lr: *mut c_void) -> c_int;
    fn libraw_unpack(lr: *mut c_void) -> c_int;
    fn libraw_dcraw_make_mem_thumb(lr: *mut c_void, errc: *mut c_int) -> *mut ProcessedImage;
    fn libraw_dcraw_clear_mem(img: *mut ProcessedImage);
    fn libraw_close(lr: *mut c_void);
    fn libraw_strerror(err: c_int) -> *const c_char;
    fn libraw_version() -> *const c_char;
}

pub fn version() -> String {
    unsafe { CStr::from_ptr(libraw_version()) }
        .to_string_lossy()
        .into_owned()
}

/// An embedded preview: JPEG bytes, or an uncompressed bitmap.
#[derive(Debug)]
pub struct Thumb {
    pub is_jpeg: bool,
    /// Dimensions reported by LibRaw for bitmaps (0 for JPEG).
    pub width: u16,
    pub height: u16,
    pub bytes: Vec<u8>,
}

struct Handle(*mut c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { libraw_close(self.0) }
    }
}

fn check(code: c_int, what: &str) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        let msg = unsafe { CStr::from_ptr(libraw_strerror(code)) }.to_string_lossy();
        Err(format!("{what}: {msg} ({code})"))
    }
}

fn open(path: &Path) -> Result<Handle, String> {
    let lr = unsafe { libraw_init(0) };
    if lr.is_null() {
        return Err("libraw_init failed".into());
    }
    let handle = Handle(lr);
    let c_path = CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    check(
        unsafe { libraw_open_file(handle.0, c_path.as_ptr()) },
        "open",
    )?;
    Ok(handle)
}

/// Extracts the largest embedded preview.
pub fn thumb(path: &Path) -> Result<Thumb, String> {
    let handle = open(path)?;
    check(unsafe { libraw_unpack_thumb(handle.0) }, "unpack_thumb")?;
    let mut err = 0;
    let img = unsafe { libraw_dcraw_make_mem_thumb(handle.0, &mut err) };
    if img.is_null() {
        check(err, "make_mem_thumb")?;
        return Err("make_mem_thumb returned null".into());
    }
    let thumb = unsafe {
        let p = &*img;
        let data = std::slice::from_raw_parts(p.data.as_ptr(), p.data_size as usize).to_vec();
        Thumb {
            is_jpeg: p.kind == LIBRAW_IMAGE_JPEG,
            width: p.width,
            height: p.height,
            bytes: data,
        }
    };
    unsafe { libraw_dcraw_clear_mem(img) };
    Ok(thumb)
}

/// Unpacks the raw sensor data (decoder coverage check, no demosaic).
pub fn unpack(path: &Path) -> Result<(), String> {
    let handle = open(path)?;
    check(unsafe { libraw_unpack(handle.0) }, "unpack")
}
