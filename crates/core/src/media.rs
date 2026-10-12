//! File formats Picturesque understands, recognized by extension.

use serde::{Deserialize, Serialize};

/// A supported file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaFormat {
    /// Camera RAW (any vendor format, including DNG).
    Raw,
    Jpeg,
    /// HEIC, HEIF and AVIF (all read through libheif).
    Heif,
    Png,
    Tiff,
    Webp,
    /// XMP metadata sidecar.
    Xmp,
}

/// What a file contributes to a photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetRole {
    /// Camera RAW data.
    Raw,
    /// A rendered image (JPEG, HEIC, PNG, TIFF, WebP).
    Image,
    /// Metadata only.
    Sidecar,
}

const RAW_EXTENSIONS: &[&str] = &[
    "3fr", "ari", "arw", "bay", "cap", "cr2", "cr3", "crw", "dcr", "dcs", "dng", "erf", "fff",
    "gpr", "iiq", "k25", "kdc", "mdc", "mef", "mos", "mrw", "nef", "nrw", "orf", "ori", "pef",
    "raf", "raw", "rw2", "rwl", "sr2", "srf", "srw", "x3f",
];

impl MediaFormat {
    /// Recognizes a file extension (without the dot, any case).
    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext = ext.to_ascii_lowercase();
        let format = match ext.as_str() {
            "jpg" | "jpeg" | "jpe" => Self::Jpeg,
            "heic" | "heif" | "hif" | "avif" => Self::Heif,
            "png" => Self::Png,
            "tif" | "tiff" => Self::Tiff,
            "webp" => Self::Webp,
            "xmp" => Self::Xmp,
            e if RAW_EXTENSIONS.contains(&e) => Self::Raw,
            _ => return None,
        };
        Some(format)
    }

    /// Recognizes a file name by its last extension.
    pub fn from_file_name(name: &str) -> Option<Self> {
        let (_, ext) = name.rsplit_once('.')?;
        Self::from_extension(ext)
    }

    pub fn role(self) -> AssetRole {
        match self {
            Self::Raw => AssetRole::Raw,
            Self::Xmp => AssetRole::Sidecar,
            Self::Jpeg | Self::Heif | Self::Png | Self::Tiff | Self::Webp => AssetRole::Image,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_extensions_case_insensitively() {
        assert_eq!(MediaFormat::from_extension("CR3"), Some(MediaFormat::Raw));
        assert_eq!(MediaFormat::from_extension("dng"), Some(MediaFormat::Raw));
        assert_eq!(MediaFormat::from_extension("JPEG"), Some(MediaFormat::Jpeg));
        assert_eq!(MediaFormat::from_extension("HEIC"), Some(MediaFormat::Heif));
        assert_eq!(MediaFormat::from_extension("tif"), Some(MediaFormat::Tiff));
        assert_eq!(MediaFormat::from_extension("xmp"), Some(MediaFormat::Xmp));
    }

    #[test]
    fn ignores_unsupported_extensions() {
        assert_eq!(MediaFormat::from_extension("mov"), None);
        assert_eq!(MediaFormat::from_extension("txt"), None);
        assert_eq!(MediaFormat::from_file_name("README"), None);
    }

    #[test]
    fn uses_the_last_extension_of_a_file_name() {
        assert_eq!(
            MediaFormat::from_file_name("IMG_1.CR3.xmp"),
            Some(MediaFormat::Xmp)
        );
        assert_eq!(
            MediaFormat::from_file_name("holiday.final.JPG"),
            Some(MediaFormat::Jpeg)
        );
    }

    #[test]
    fn maps_formats_to_roles() {
        assert_eq!(MediaFormat::Raw.role(), AssetRole::Raw);
        assert_eq!(MediaFormat::Webp.role(), AssetRole::Image);
        assert_eq!(MediaFormat::Xmp.role(), AssetRole::Sidecar);
    }
}
