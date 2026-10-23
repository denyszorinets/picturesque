//! Photos and the files (assets) that make them up.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::hash::ContentHash;
use crate::media::{AssetRole, MediaFormat};
use crate::metadata::CaptureMetadata;

/// Catalog identifier of a photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PhotoId(pub i64);

/// One file belonging to a photo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub format: MediaFormat,
    /// Path relative to the Clean library root.
    pub path: PathBuf,
    pub size: u64,
    pub hash: ContentHash,
}

impl Asset {
    pub fn role(&self) -> AssetRole {
        self.format.role()
    }
}

/// Which file of a RAW+JPEG pair is shown. Photos with only one of the two
/// show that one whatever is chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairView {
    /// The rendered image the camera wrote (usually a JPEG).
    #[default]
    Image,
    /// The RAW, shown through its embedded preview.
    Raw,
}

impl PairView {
    pub fn name(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Raw => "raw",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Image, Self::Raw]
            .into_iter()
            .find(|v| v.name() == name)
    }
}

/// Why a set of assets cannot form a photo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetsError {
    /// Only sidecars, no RAW or image file.
    NoMediaFile,
    /// More than one file of the same role (two RAWs or two images).
    DuplicateRole(AssetRole),
}

impl fmt::Display for AssetsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoMediaFile => f.write_str("a photo needs a RAW or image file"),
            Self::DuplicateRole(role) => write!(f, "a photo can have only one {role:?} file"),
        }
    }
}

impl std::error::Error for AssetsError {}

/// The files of one photo: at most one RAW, at most one rendered image
/// (together a RAW+JPEG pair), and any number of sidecars. At least one
/// RAW or image is always present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Asset>", into = "Vec<Asset>")]
pub struct PhotoAssets {
    raw: Option<Asset>,
    image: Option<Asset>,
    sidecars: Vec<Asset>,
}

impl PhotoAssets {
    pub fn new(assets: Vec<Asset>) -> Result<Self, AssetsError> {
        let mut raw = None;
        let mut image = None;
        let mut sidecars = Vec::new();
        for asset in assets {
            let slot = match asset.role() {
                AssetRole::Raw => &mut raw,
                AssetRole::Image => &mut image,
                AssetRole::Sidecar => {
                    sidecars.push(asset);
                    continue;
                }
            };
            if slot.is_some() {
                return Err(AssetsError::DuplicateRole(asset.role()));
            }
            *slot = Some(asset);
        }
        if raw.is_none() && image.is_none() {
            return Err(AssetsError::NoMediaFile);
        }
        Ok(Self {
            raw,
            image,
            sidecars,
        })
    }

    pub fn raw(&self) -> Option<&Asset> {
        self.raw.as_ref()
    }

    pub fn image(&self) -> Option<&Asset> {
        self.image.as_ref()
    }

    pub fn sidecars(&self) -> &[Asset] {
        &self.sidecars
    }

    /// The file metadata is read from: the RAW when present, since cameras
    /// record the most complete EXIF there.
    pub fn primary(&self) -> &Asset {
        self.raw
            .as_ref()
            .or(self.image.as_ref())
            .expect("invariant: RAW or image present")
    }

    /// The file to display: the rendered image when present, otherwise the
    /// RAW (shown through its embedded preview).
    pub fn display(&self) -> &Asset {
        self.shown(PairView::Image)
    }

    /// The file to show for `view`, falling back to the other file when the
    /// photo is not a RAW+JPEG pair.
    pub fn shown(&self, view: PairView) -> &Asset {
        let (first, second) = match view {
            PairView::Image => (&self.image, &self.raw),
            PairView::Raw => (&self.raw, &self.image),
        };
        first
            .as_ref()
            .or(second.as_ref())
            .expect("invariant: RAW or image present")
    }

    pub fn iter(&self) -> impl Iterator<Item = &Asset> {
        self.raw
            .iter()
            .chain(self.image.iter())
            .chain(self.sidecars.iter())
    }
}

impl TryFrom<Vec<Asset>> for PhotoAssets {
    type Error = AssetsError;

    fn try_from(assets: Vec<Asset>) -> Result<Self, Self::Error> {
        Self::new(assets)
    }
}

impl From<PhotoAssets> for Vec<Asset> {
    fn from(assets: PhotoAssets) -> Self {
        assets
            .raw
            .into_iter()
            .chain(assets.image)
            .chain(assets.sidecars)
            .collect()
    }
}

/// A photo not yet stored in the catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewPhoto {
    pub metadata: CaptureMetadata,
    pub assets: PhotoAssets,
}

/// A photo in the catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Photo {
    pub id: PhotoId,
    pub metadata: CaptureMetadata,
    pub assets: PhotoAssets,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> Asset {
        Asset {
            format: MediaFormat::from_file_name(name).unwrap(),
            path: PathBuf::from(name),
            size: 1,
            hash: ContentHash::from_bytes([name.len() as u8; 32]),
        }
    }

    #[test]
    fn raw_and_jpeg_pair_forms_one_photo() {
        let assets =
            PhotoAssets::new(vec![asset("a.JPG"), asset("a.CR3"), asset("a.CR3.xmp")]).unwrap();

        assert_eq!(assets.raw().unwrap().path, PathBuf::from("a.CR3"));
        assert_eq!(assets.image().unwrap().path, PathBuf::from("a.JPG"));
        assert_eq!(assets.sidecars().len(), 1);
    }

    #[test]
    fn primary_prefers_raw_and_display_prefers_image() {
        let pair = PhotoAssets::new(vec![asset("a.CR3"), asset("a.JPG")]).unwrap();
        assert_eq!(pair.primary().path, PathBuf::from("a.CR3"));
        assert_eq!(pair.display().path, PathBuf::from("a.JPG"));

        let raw_only = PhotoAssets::new(vec![asset("b.NEF")]).unwrap();
        assert_eq!(raw_only.primary().path, PathBuf::from("b.NEF"));
        assert_eq!(raw_only.display().path, PathBuf::from("b.NEF"));
    }

    #[test]
    fn shown_picks_the_chosen_file_of_a_pair_and_falls_back_otherwise() {
        let pair = PhotoAssets::new(vec![asset("a.CR3"), asset("a.JPG")]).unwrap();
        assert_eq!(pair.shown(PairView::Image).path, PathBuf::from("a.JPG"));
        assert_eq!(pair.shown(PairView::Raw).path, PathBuf::from("a.CR3"));

        let jpeg_only = PhotoAssets::new(vec![asset("b.JPG")]).unwrap();
        assert_eq!(jpeg_only.shown(PairView::Raw).path, PathBuf::from("b.JPG"));
        let raw_only = PhotoAssets::new(vec![asset("c.NEF")]).unwrap();
        assert_eq!(raw_only.shown(PairView::Image).path, PathBuf::from("c.NEF"));
    }

    #[test]
    fn pair_view_names_round_trip() {
        for view in [PairView::Image, PairView::Raw] {
            assert_eq!(PairView::from_name(view.name()), Some(view));
            assert_eq!(serde_json::to_value(view).unwrap(), view.name());
        }
        assert_eq!(PairView::from_name("jpeg"), None);
    }

    #[test]
    fn rejects_sidecar_only_photo() {
        assert_eq!(
            PhotoAssets::new(vec![asset("a.xmp")]),
            Err(AssetsError::NoMediaFile)
        );
        assert_eq!(PhotoAssets::new(vec![]), Err(AssetsError::NoMediaFile));
    }

    #[test]
    fn rejects_two_files_with_the_same_role() {
        assert_eq!(
            PhotoAssets::new(vec![asset("a.CR3"), asset("a.DNG")]),
            Err(AssetsError::DuplicateRole(AssetRole::Raw))
        );
        assert_eq!(
            PhotoAssets::new(vec![asset("a.JPG"), asset("a.HEIC")]),
            Err(AssetsError::DuplicateRole(AssetRole::Image))
        );
    }

    #[test]
    fn iterates_raw_image_then_sidecars() {
        let assets =
            PhotoAssets::new(vec![asset("a.xmp"), asset("a.JPG"), asset("a.CR3")]).unwrap();
        let names: Vec<_> = assets
            .iter()
            .map(|a| a.path.to_string_lossy().into_owned())
            .collect();

        assert_eq!(names, ["a.CR3", "a.JPG", "a.xmp"]);
    }

    #[test]
    fn serializes_as_a_list_and_validates_on_deserialize() {
        let assets = PhotoAssets::new(vec![asset("a.CR3"), asset("a.JPG")]).unwrap();
        let json = serde_json::to_value(&assets).unwrap();

        assert!(json.is_array());
        assert_eq!(serde_json::from_value::<PhotoAssets>(json).unwrap(), assets);

        let sidecar_only = serde_json::to_value(vec![asset("a.xmp")]).unwrap();
        assert!(serde_json::from_value::<PhotoAssets>(sidecar_only).is_err());
    }
}
