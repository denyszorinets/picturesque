//! Grouping the files of one directory into photos.
//!
//! Cameras write a RAW and a JPEG with the same base name (`IMG_1.CR3`,
//! `IMG_1.JPG`); editors write sidecars either next to the base name
//! (`IMG_1.xmp`, Lightroom) or after the full file name (`IMG_1.CR3.xmp`,
//! darktable). Names are compared case-insensitively.

use std::collections::BTreeMap;

use crate::media::{AssetRole, MediaFormat};

/// File names (within one directory) that form one photo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileGroup {
    pub raw: Option<String>,
    pub image: Option<String>,
    pub sidecars: Vec<String>,
}

/// Result of grouping a directory listing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Grouping {
    /// One entry per photo, ordered by base name.
    pub groups: Vec<FileGroup>,
    /// Sidecars that match no RAW or image file.
    pub orphan_sidecars: Vec<String>,
}

/// Groups the file names of one directory into photos. Unsupported files
/// are ignored.
///
/// A base name with at most one RAW and one image becomes one photo. When a
/// base name has more (for example `a.CR2` and `a.DNG`), each file becomes
/// its own photo; a `name.ext.xmp` sidecar then follows its exact file,
/// and a `name.xmp` sidecar follows the RAW only if there is exactly one.
pub fn group_files<S: AsRef<str>>(names: &[S]) -> Grouping {
    #[derive(Default)]
    struct Bucket<'a> {
        raws: Vec<&'a str>,
        images: Vec<&'a str>,
        sidecars: Vec<&'a str>,
    }

    let mut buckets: BTreeMap<String, Bucket> = BTreeMap::new();
    for name in names {
        let name = name.as_ref();
        let Some(format) = MediaFormat::from_file_name(name) else {
            continue;
        };
        let bucket = buckets.entry(base_key(name, format)).or_default();
        match format.role() {
            AssetRole::Raw => bucket.raws.push(name),
            AssetRole::Image => bucket.images.push(name),
            AssetRole::Sidecar => bucket.sidecars.push(name),
        }
    }

    let mut grouping = Grouping::default();
    for (_, mut bucket) in buckets {
        bucket.raws.sort_unstable();
        bucket.images.sort_unstable();
        bucket.sidecars.sort_unstable();

        if bucket.raws.is_empty() && bucket.images.is_empty() {
            grouping
                .orphan_sidecars
                .extend(bucket.sidecars.iter().map(|s| s.to_string()));
            continue;
        }

        if bucket.raws.len() <= 1 && bucket.images.len() <= 1 {
            grouping.groups.push(FileGroup {
                raw: bucket.raws.first().map(|s| s.to_string()),
                image: bucket.images.first().map(|s| s.to_string()),
                sidecars: bucket.sidecars.iter().map(|s| s.to_string()).collect(),
            });
            continue;
        }

        // Ambiguous: split into one photo per file.
        let mut split: Vec<FileGroup> = bucket
            .raws
            .iter()
            .map(|r| FileGroup {
                raw: Some(r.to_string()),
                ..Default::default()
            })
            .chain(bucket.images.iter().map(|i| FileGroup {
                image: Some(i.to_string()),
                ..Default::default()
            }))
            .collect();
        for sidecar in bucket.sidecars {
            let target = sidecar_target(sidecar);
            let exact = split.iter_mut().find(|g| {
                let file = g.raw.as_deref().or(g.image.as_deref()).unwrap_or_default();
                target.is_some_and(|t| t.eq_ignore_ascii_case(file))
            });
            match exact {
                Some(group) => group.sidecars.push(sidecar.to_string()),
                None if target.is_none() && bucket.raws.len() == 1 => {
                    split[0].sidecars.push(sidecar.to_string())
                }
                None => grouping.orphan_sidecars.push(sidecar.to_string()),
            }
        }
        grouping.groups.extend(split);
    }
    grouping
}

/// The base name a supported file is grouped by, with case preserved:
/// the name without its extension, and for sidecars also without a media
/// extension before `.xmp` (`IMG_1.CR3.xmp` → `IMG_1`). `None` for
/// unsupported files.
pub fn base_name(name: &str) -> Option<&str> {
    let format = MediaFormat::from_file_name(name)?;
    let stem = strip_ext(name);
    Some(
        if format == MediaFormat::Xmp && MediaFormat::from_file_name(stem).is_some() {
            strip_ext(stem)
        } else {
            stem
        },
    )
}

fn base_key(name: &str, _format: MediaFormat) -> String {
    base_name(name).unwrap_or(name).to_lowercase()
}

/// For `name.ext.xmp`, the file it belongs to (`name.ext`).
fn sidecar_target(sidecar: &str) -> Option<&str> {
    let stem = strip_ext(sidecar);
    MediaFormat::from_file_name(stem).map(|_| stem)
}

fn strip_ext(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(raw: Option<&str>, image: Option<&str>, sidecars: &[&str]) -> FileGroup {
        FileGroup {
            raw: raw.map(str::to_owned),
            image: image.map(str::to_owned),
            sidecars: sidecars.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn pairs_raw_and_jpeg_with_the_same_base_name() {
        let g = group_files(&["IMG_1.CR3", "IMG_1.JPG", "IMG_2.JPG"]);

        assert_eq!(
            g.groups,
            [
                group(Some("IMG_1.CR3"), Some("IMG_1.JPG"), &[]),
                group(None, Some("IMG_2.JPG"), &[])
            ]
        );
        assert!(g.orphan_sidecars.is_empty());
    }

    #[test]
    fn matches_base_names_case_insensitively() {
        let g = group_files(&["img_1.cr3", "IMG_1.JPG"]);

        assert_eq!(g.groups, [group(Some("img_1.cr3"), Some("IMG_1.JPG"), &[])]);
    }

    #[test]
    fn attaches_both_sidecar_naming_styles() {
        let g = group_files(&["a.NEF", "a.xmp", "b.ARW", "b.ARW.xmp"]);

        assert_eq!(
            g.groups,
            [
                group(Some("a.NEF"), None, &["a.xmp"]),
                group(Some("b.ARW"), None, &["b.ARW.xmp"])
            ]
        );
    }

    #[test]
    fn reports_sidecars_without_a_photo() {
        let g = group_files(&["a.JPG", "lonely.xmp", "gone.CR2.xmp"]);

        assert_eq!(g.groups, [group(None, Some("a.JPG"), &[])]);
        assert_eq!(g.orphan_sidecars, ["gone.CR2.xmp", "lonely.xmp"]);
    }

    #[test]
    fn ignores_unsupported_files() {
        let g = group_files(&["clip.MOV", "notes.txt", ".DS_Store", "a.png"]);

        assert_eq!(g.groups, [group(None, Some("a.png"), &[])]);
    }

    #[test]
    fn splits_ambiguous_base_names_into_separate_photos() {
        let g = group_files(&["a.CR2", "a.DNG", "a.JPG", "a.DNG.xmp", "a.xmp"]);

        assert_eq!(
            g.groups,
            [
                group(Some("a.CR2"), None, &[]),
                group(Some("a.DNG"), None, &["a.DNG.xmp"]),
                group(None, Some("a.JPG"), &[]),
            ]
        );
        assert_eq!(g.orphan_sidecars, ["a.xmp"]);
    }

    #[test]
    fn plain_sidecar_follows_the_single_raw_when_images_are_ambiguous() {
        let g = group_files(&["a.CR3", "a.JPG", "a.HEIC", "a.xmp"]);

        assert_eq!(
            g.groups,
            [
                group(Some("a.CR3"), None, &["a.xmp"]),
                group(None, Some("a.HEIC"), &[]),
                group(None, Some("a.JPG"), &[]),
            ]
        );
    }

    #[test]
    fn base_name_strips_media_and_sidecar_extensions() {
        assert_eq!(base_name("IMG_1.CR3"), Some("IMG_1"));
        assert_eq!(base_name("IMG_1.CR3.xmp"), Some("IMG_1"));
        assert_eq!(base_name("IMG_1.xmp"), Some("IMG_1"));
        assert_eq!(base_name("IMG_1 (2).JPG"), Some("IMG_1 (2)"));
        assert_eq!(base_name("trip.day1.JPG"), Some("trip.day1"));
        assert_eq!(base_name("notes.txt"), None);
    }

    #[test]
    fn keeps_dots_inside_base_names() {
        let g = group_files(&["trip.day1.CR3", "trip.day1.JPG", "trip.day1.CR3.xmp"]);

        assert_eq!(
            g.groups,
            [group(
                Some("trip.day1.CR3"),
                Some("trip.day1.JPG"),
                &["trip.day1.CR3.xmp"]
            )]
        );
    }
}
