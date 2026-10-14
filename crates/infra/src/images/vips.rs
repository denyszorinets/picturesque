//! Minimal FFI to libvips: the few operations Picturesque uses, called
//! through libvips' C API (name/value option lists ending in NULL), the same
//! on Linux, macOS and Windows (ADR-0005).

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;
use std::ptr::{NonNull, null, null_mut};
use std::sync::{Arc, OnceLock};

#[repr(C)]
struct VipsImage {
    _private: [u8; 0],
}

// libvips enums, as C `int`s.
const SIZE_DOWN: c_int = 2;
const SIZE_FORCE: c_int = 3;
#[cfg(test)]
const INTERPRETATION_B_W: c_int = 1;
const INTERPRETATION_SRGB: c_int = 22;
const FORMAT_UCHAR: c_int = 0;
const DIRECTION_HORIZONTAL: c_int = 0;
const KEEP_NONE: c_int = 0;
const KEEP_ALL: c_int = 31;

#[link(name = "vips")]
unsafe extern "C" {
    fn vips_init(argv0: *const c_char) -> c_int;
    fn vips_cache_set_max(max: c_int);
    fn vips_cache_set_max_mem(max_mem: usize);
    fn vips_error_buffer() -> *const c_char;
    fn vips_error_clear();

    fn vips_image_new_from_file(name: *const c_char, ...) -> *mut VipsImage;
    fn vips_image_new_from_buffer(
        buf: *const c_void,
        len: usize,
        option_string: *const c_char,
        ...
    ) -> *mut VipsImage;
    fn vips_image_new_from_memory_copy(
        data: *const c_void,
        size: usize,
        width: c_int,
        height: c_int,
        bands: c_int,
        format: c_int,
    ) -> *mut VipsImage;
    fn vips_image_write_to_memory(image: *mut VipsImage, size: *mut usize) -> *mut c_void;
    fn vips_image_get_width(image: *const VipsImage) -> c_int;
    fn vips_image_get_height(image: *const VipsImage) -> c_int;
    fn vips_image_get_bands(image: *const VipsImage) -> c_int;

    fn vips_thumbnail(
        filename: *const c_char,
        out: *mut *mut VipsImage,
        width: c_int,
        ...
    ) -> c_int;
    fn vips_thumbnail_image(
        image: *mut VipsImage,
        out: *mut *mut VipsImage,
        width: c_int,
        ...
    ) -> c_int;
    fn vips_jpegsave(image: *mut VipsImage, filename: *const c_char, ...) -> c_int;
    fn vips_flatten(image: *mut VipsImage, out: *mut *mut VipsImage, ...) -> c_int;
    fn vips_colourspace(
        image: *mut VipsImage,
        out: *mut *mut VipsImage,
        space: c_int,
        ...
    ) -> c_int;
    fn vips_cast(image: *mut VipsImage, out: *mut *mut VipsImage, format: c_int, ...) -> c_int;
    fn vips_rot(image: *mut VipsImage, out: *mut *mut VipsImage, angle: c_int, ...) -> c_int;
    fn vips_flip(image: *mut VipsImage, out: *mut *mut VipsImage, direction: c_int, ...) -> c_int;
    fn vips_copy(image: *mut VipsImage, out: *mut *mut VipsImage, ...) -> c_int;
}

#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn g_object_unref(object: *mut c_void);
}

#[link(name = "glib-2.0")]
unsafe extern "C" {
    fn g_free(memory: *mut c_void);
}

/// Ends a name/value option list.
const END: *const c_char = null();

/// Initializes libvips once per process.
pub fn init() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let name = c"picturesque";
        // SAFETY: a valid C string; libvips may be initialized from any thread.
        let failed = unsafe { vips_init(name.as_ptr()) } != 0;
        assert!(!failed, "libvips initializes");
        // SAFETY: plain setters, after initialization. Each image is rendered
        // once; the operation cache would only use memory.
        unsafe {
            vips_cache_set_max(0);
            vips_cache_set_max_mem(0);
        }
    });
}

/// The first line of libvips' error buffer (cleared), prefixed by `what`.
pub fn error(what: &str) -> String {
    // SAFETY: libvips returns a valid, NUL-terminated buffer it owns.
    let message = unsafe {
        let buffer = vips_error_buffer();
        let text = if buffer.is_null() {
            String::new()
        } else {
            CStr::from_ptr(buffer).to_string_lossy().into_owned()
        };
        vips_error_clear();
        text
    };
    match message.lines().next().filter(|l| !l.is_empty()) {
        Some(line) => format!("{what}: {line}"),
        None => what.to_owned(),
    }
}

fn c_path(path: &Path) -> Result<CString, String> {
    CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())
}

/// Rotation in steps of 90 degrees, clockwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Angle {
    D0 = 0,
    D90 = 1,
    D180 = 2,
    D270 = 3,
}

/// A libvips image. Images read from a buffer keep the buffer alive (libvips
/// reads it lazily, without copying), and so do images derived from them.
pub struct Image {
    ptr: NonNull<VipsImage>,
    _source: Option<Arc<Vec<u8>>>,
}

impl Drop for Image {
    fn drop(&mut self) {
        // SAFETY: we own one reference to this GObject.
        unsafe { g_object_unref(self.ptr.as_ptr().cast()) }
    }
}

impl Image {
    fn wrap(ptr: *mut VipsImage, source: Option<Arc<Vec<u8>>>, what: &str) -> Result<Self, String> {
        NonNull::new(ptr)
            .map(|ptr| Self {
                ptr,
                _source: source,
            })
            .ok_or_else(|| error(what))
    }

    /// Runs an operation writing one output image, which keeps this image's
    /// buffer alive.
    fn derive(
        &self,
        what: &str,
        op: impl FnOnce(*mut VipsImage, *mut *mut VipsImage) -> c_int,
    ) -> Result<Self, String> {
        let mut out = null_mut();
        if op(self.ptr.as_ptr(), &mut out) != 0 {
            return Err(error(what));
        }
        Self::wrap(out, self._source.clone(), what)
    }

    /// The image's header (pixels are decoded on use).
    pub fn from_file(path: &Path) -> Result<Self, String> {
        init();
        let name = c_path(path)?;
        // SAFETY: valid C string; the option list is empty.
        let ptr = unsafe { vips_image_new_from_file(name.as_ptr(), END) };
        Self::wrap(ptr, None, "open")
    }

    /// An encoded image (JPEG, …) in memory.
    pub fn from_buffer(bytes: Vec<u8>) -> Result<Self, String> {
        init();
        let bytes = Arc::new(bytes);
        // SAFETY: the buffer lives as long as this image and its derivatives
        // (they hold `bytes`); the option string and list are empty.
        let ptr = unsafe {
            vips_image_new_from_buffer(bytes.as_ptr().cast(), bytes.len(), c"".as_ptr(), END)
        };
        Self::wrap(ptr, Some(bytes), "read buffer")
    }

    /// 8-bit interleaved pixels (copied).
    pub fn from_pixels(pixels: &[u8], width: u32, height: u32, bands: u32) -> Result<Self, String> {
        init();
        let expected = width as usize * height as usize * bands as usize;
        if pixels.len() < expected {
            return Err("pixel buffer too small".into());
        }
        let size = |v: u32| c_int::try_from(v).map_err(|e| e.to_string());
        // SAFETY: `pixels` holds at least `expected` bytes; libvips copies them.
        let ptr = unsafe {
            vips_image_new_from_memory_copy(
                pixels.as_ptr().cast(),
                expected,
                size(width)?,
                size(height)?,
                size(bands)?,
                FORMAT_UCHAR,
            )
        };
        Self::wrap(ptr, None, "read pixels")
    }

    /// Decodes a file shrunk to fit `edge` × `edge` (shrink-on-load),
    /// upright from its EXIF orientation; never enlarged.
    pub fn thumbnail(path: &Path, edge: i32) -> Result<Self, String> {
        init();
        let name = c_path(path)?;
        let mut out = null_mut();
        // SAFETY: valid strings and ints; the list ends with END.
        let failed = unsafe {
            vips_thumbnail(
                name.as_ptr(),
                &mut out,
                edge,
                c"height".as_ptr(),
                edge,
                c"size".as_ptr(),
                SIZE_DOWN,
                END,
            )
        } != 0;
        if failed {
            return Err(error("decode"));
        }
        Self::wrap(out, None, "decode")
    }

    /// Shrinks to fit `edge` × `edge` without applying EXIF orientation;
    /// `force` stretches to exactly that size, otherwise it never enlarges.
    pub fn shrink(&self, edge: i32, force: bool) -> Result<Self, String> {
        self.derive("shrink", |image, out| {
            let size = if force { SIZE_FORCE } else { SIZE_DOWN };
            // SAFETY: valid image and ints; the list ends with END.
            unsafe {
                vips_thumbnail_image(
                    image,
                    out,
                    edge,
                    c"height".as_ptr(),
                    edge,
                    c"size".as_ptr(),
                    size,
                    c"no-rotate".as_ptr(),
                    1 as c_int,
                    END,
                )
            }
        })
    }

    /// Saves as JPEG; `keep_metadata` keeps EXIF, XMP, IPTC and the ICC
    /// profile, otherwise all are dropped.
    pub fn save_jpeg(&self, path: &Path, quality: i32, keep_metadata: bool) -> Result<(), String> {
        let name = c_path(path)?;
        let keep = if keep_metadata { KEEP_ALL } else { KEEP_NONE };
        // SAFETY: valid image, string and ints; the list ends with END.
        let failed = unsafe {
            vips_jpegsave(
                self.ptr.as_ptr(),
                name.as_ptr(),
                c"Q".as_ptr(),
                quality as c_int,
                c"keep".as_ptr(),
                keep,
                END,
            )
        } != 0;
        if failed { Err(error("save")) } else { Ok(()) }
    }

    /// Removes an alpha channel (against black).
    pub fn flatten(&self) -> Result<Self, String> {
        // SAFETY: valid image; empty option list.
        self.derive("flatten", |image, out| unsafe {
            vips_flatten(image, out, END)
        })
    }

    pub fn to_srgb(&self) -> Result<Self, String> {
        // SAFETY: valid image; empty option list.
        self.derive("colourspace", |image, out| unsafe {
            vips_colourspace(image, out, INTERPRETATION_SRGB, END)
        })
    }

    #[cfg(test)]
    pub fn to_grey(&self) -> Result<Self, String> {
        // SAFETY: valid image; empty option list.
        self.derive("colourspace", |image, out| unsafe {
            vips_colourspace(image, out, INTERPRETATION_B_W, END)
        })
    }

    /// 8 bits per band.
    pub fn to_uchar(&self) -> Result<Self, String> {
        // SAFETY: valid image; empty option list.
        self.derive("cast", |image, out| unsafe {
            vips_cast(image, out, FORMAT_UCHAR, END)
        })
    }

    pub fn rotate(&self, angle: Angle) -> Result<Self, String> {
        // SAFETY: valid image; empty option list.
        self.derive("rotate", |image, out| unsafe {
            vips_rot(image, out, angle as c_int, END)
        })
    }

    /// Mirrors left to right.
    pub fn flip(&self) -> Result<Self, String> {
        // SAFETY: valid image; empty option list.
        self.derive("flip", |image, out| unsafe {
            vips_flip(image, out, DIRECTION_HORIZONTAL, END)
        })
    }

    pub fn copy(&self) -> Result<Self, String> {
        // SAFETY: valid image; empty option list.
        self.derive("copy", |image, out| unsafe { vips_copy(image, out, END) })
    }

    pub fn width(&self) -> u32 {
        // SAFETY: valid image.
        unsafe { vips_image_get_width(self.ptr.as_ptr()) }.max(0) as u32
    }

    pub fn height(&self) -> u32 {
        // SAFETY: valid image.
        unsafe { vips_image_get_height(self.ptr.as_ptr()) }.max(0) as u32
    }

    pub fn bands(&self) -> u32 {
        // SAFETY: valid image.
        unsafe { vips_image_get_bands(self.ptr.as_ptr()) }.max(0) as u32
    }

    /// The decoded pixels, interleaved.
    pub fn pixels(&self) -> Result<Vec<u8>, String> {
        let mut size = 0usize;
        // SAFETY: valid image; libvips returns a g_malloc'ed buffer of `size`
        // bytes, copied and freed here.
        unsafe {
            let data = vips_image_write_to_memory(self.ptr.as_ptr(), &mut size);
            if data.is_null() {
                return Err(error("decode pixels"));
            }
            let bytes = std::slice::from_raw_parts(data.cast::<u8>(), size).to_vec();
            g_free(data);
            Ok(bytes)
        }
    }
}
