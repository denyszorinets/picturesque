//! Building an import plan: what to copy where, and what to skip.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::UNIX_EPOCH;

use picturesque_core::grouping::base_name;
use picturesque_core::hash::ContentHash;
use picturesque_core::media::{AssetRole, MediaFormat};
use picturesque_core::metadata::CaptureMetadata;
use picturesque_core::template::{PathTemplate, TemplateValues, unique_stem};

use super::model::{FileStatus, ImportProblem, PhotoStatus, PlannedFile, PlannedPhoto};
use super::{Deps, Progress};
use crate::ports::catalog::CatalogError;
use crate::scan::{SourcePhoto, scan_sources};

/// Planning stopped before finishing.
#[derive(Debug)]
pub enum PlanError {
    Cancelled,
    Catalog(CatalogError),
}

/// Scans `sources` and builds the plan. Nothing is copied.
pub fn build_plan(
    deps: &Deps,
    sources: &[PathBuf],
    library_root: &Path,
    template: &PathTemplate,
    cancel: &AtomicBool,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<(Vec<PlannedPhoto>, Vec<ImportProblem>), PlanError> {
    progress(Progress::Scanning);
    let report = scan_sources(deps.fs.as_ref(), sources, &[library_root.to_path_buf()]);
    let mut problems: Vec<ImportProblem> = report
        .unreadable
        .iter()
        .map(|(path, reason)| ImportProblem {
            path: path.clone(),
            reason: format!("Cannot read folder: {reason}"),
        })
        .chain(report.orphan_sidecars.iter().map(|path| ImportProblem {
            path: path.clone(),
            reason: "Sidecar without a matching photo; not imported".into(),
        }))
        .collect();

    let mut photos = read_all(deps, &report.photos, cancel, progress)?;
    // Duplicates keep the copy from the first source the user listed.
    photos.sort_by_key(|p| source_rank(sources, &primary_file(&p.files).source));
    mark_duplicates(deps, &mut photos)?;
    assign_targets(deps, &mut photos, library_root, template);

    for photo in &photos {
        if photo.status == PhotoStatus::Failed {
            problems.push(ImportProblem {
                path: photo.files[0].source.clone(),
                reason: photo.reason.clone().unwrap_or_default(),
            });
        }
    }
    Ok((photos, problems))
}

/// Reads metadata and file stamps for every photo, in parallel.
fn read_all(
    deps: &Deps,
    sources: &[SourcePhoto],
    cancel: &AtomicBool,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<Vec<PlannedPhoto>, PlanError> {
    let total = sources.len();
    progress(Progress::Reading { done: 0, total });
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<PlannedPhoto>>> = Mutex::new(vec![None; total]);
    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= total || cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let photo = read_one(deps, &sources[i]);
                    results.lock().expect("results lock")[i] = Some(photo);
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    progress(Progress::Reading { done: d, total });
                }
            });
        }
    });

    if cancel.load(Ordering::Relaxed) {
        return Err(PlanError::Cancelled);
    }
    Ok(results
        .into_inner()
        .expect("results lock")
        .into_iter()
        .map(|p| p.expect("every photo read"))
        .collect())
}

fn read_one(deps: &Deps, source: &SourcePhoto) -> PlannedPhoto {
    let primary = source.primary();
    let mut reason = None;
    let mut metadata = match deps.metadata.read(&primary.path, primary.format) {
        Ok(m) => m,
        Err(e) => {
            reason = Some(format!("Cannot read {}: {e}", file_name(&primary.path)));
            CaptureMetadata::default()
        }
    };
    if let (Some(raw), Some(image)) = (&source.raw, &source.image)
        && raw.path == primary.path
        && let Ok(extra) = deps.metadata.read(&image.path, image.format)
    {
        if reason.is_some() {
            // The RAW is unreadable but its JPEG is fine: use the JPEG.
            metadata = extra;
            reason = None;
        } else {
            metadata.fill_missing_from(&extra);
        }
    }
    if metadata.place.is_none()
        && let Some(gps) = metadata.gps
    {
        metadata.place = deps.geocoder.place(gps);
    }

    let mut files = Vec::new();
    for file in source.files() {
        let stamp = deps.fs.stamp(&file.path);
        if let Err(e) = &stamp {
            reason.get_or_insert_with(|| format!("Cannot read {}: {e}", file_name(&file.path)));
        }
        let stamp = stamp.ok();
        files.push(PlannedFile {
            id: None,
            source: file.path.clone(),
            format: file.format,
            size: stamp.map_or(file.size, |s| s.size),
            modified_ns: stamp
                .and_then(|s| s.modified)
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .and_then(|d| i64::try_from(d.as_nanos()).ok()),
            hash: None,
            target: PathBuf::new(),
            status: FileStatus::Planned,
            error: None,
        });
    }

    PlannedPhoto {
        id: None,
        metadata,
        status: if reason.is_some() {
            PhotoStatus::Failed
        } else {
            PhotoStatus::Planned
        },
        reason,
        photo_id: None,
        files,
    }
}

/// Marks photos whose RAW or image content is already catalogued, or
/// appears earlier in this plan. Only files whose size collides with
/// another file are hashed.
fn mark_duplicates(deps: &Deps, photos: &mut [PlannedPhoto]) -> Result<(), PlanError> {
    let catalog_sizes = deps.catalog.media_sizes().map_err(PlanError::Catalog)?;
    let mut size_counts: HashMap<u64, usize> = HashMap::new();
    for file in photos
        .iter()
        .flat_map(|p| p.files.iter())
        .filter(|f| is_media(f))
    {
        *size_counts.entry(file.size).or_default() += 1;
    }

    let mut seen: HashMap<ContentHash, PathBuf> = HashMap::new();
    for photo in photos
        .iter_mut()
        .filter(|p| p.status == PhotoStatus::Planned)
    {
        for file in photo.files.iter_mut().filter(|f| is_media(f)) {
            if catalog_sizes.contains(&file.size) || size_counts[&file.size] > 1 {
                match deps.fs.hash_file(&file.source) {
                    Ok((hash, _)) => file.hash = Some(hash),
                    Err(e) => {
                        photo.status = PhotoStatus::Failed;
                        photo.reason =
                            Some(format!("Cannot read {}: {e}", file_name(&file.source)));
                    }
                }
            }
        }
        if photo.status != PhotoStatus::Planned {
            continue;
        }
        let hashes: Vec<(ContentHash, PathBuf)> = photo
            .files
            .iter()
            .filter(|f| is_media(f))
            .filter_map(|f| f.hash.map(|h| (h, f.source.clone())))
            .collect();
        for (hash, source) in &hashes {
            if deps
                .catalog
                .photo_with_media_hash(hash)
                .map_err(PlanError::Catalog)?
                .is_some()
            {
                photo.status = PhotoStatus::Duplicate;
                photo.reason = Some(format!("{} is already in the library", file_name(source)));
                break;
            }
            if let Some(first) = seen.get(hash) {
                photo.status = PhotoStatus::Duplicate;
                photo.reason = Some(format!(
                    "{} is identical to {}",
                    file_name(source),
                    first.display()
                ));
                break;
            }
        }
        if photo.status == PhotoStatus::Planned {
            seen.extend(hashes);
        }
    }
    Ok(())
}

/// Assigns sequence numbers (capture order) and collision-free targets.
fn assign_targets(
    deps: &Deps,
    photos: &mut [PlannedPhoto],
    library_root: &Path,
    template: &PathTemplate,
) {
    sort_by_capture(photos);
    let mut planned_stems: HashSet<(PathBuf, String)> = HashSet::new();
    let mut disk_stems: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    let mut seq = 0;

    for photo in photos
        .iter_mut()
        .filter(|p| p.status == PhotoStatus::Planned)
    {
        seq += 1;
        let primary = primary_file(&photo.files);
        let original_stem = stem_of(&primary.source);
        let m = &photo.metadata;
        let rendered = template.render(&TemplateValues {
            captured_at: m.captured_at.map(|t| t.local),
            place: m.place.as_ref(),
            camera: m.camera.as_ref(),
            lens: m.lens.as_deref(),
            original_name: &original_stem,
            seq: Some(seq),
        });

        let dir_key = PathBuf::from(rendered.dir.to_string_lossy().to_lowercase());
        let on_disk = disk_stems.entry(dir_key.clone()).or_insert_with(|| {
            deps.fs
                .list_dir(&library_root.join(&rendered.dir))
                .map(|entries| entries.iter().map(|e| stem_key(&e.name)).collect())
                .unwrap_or_default()
        });
        let stem = unique_stem(&rendered.stem, |candidate| {
            let key = candidate.to_lowercase();
            on_disk.contains(&key) || planned_stems.contains(&(dir_key.clone(), key))
        });
        planned_stems.insert((dir_key, stem.to_lowercase()));

        for file in &mut photo.files {
            let name = file_name(&file.source);
            let suffix = match file.format.role() {
                // `IMG_1.CR3.xmp` keeps `.CR3.xmp`; `IMG_1.xmp` keeps `.xmp`.
                AssetRole::Sidecar => name.get(original_stem.len()..).unwrap_or(".xmp").to_owned(),
                _ => format!(".{}", extension(&name)),
            };
            file.target = rendered.dir.join(format!("{stem}{suffix}"));
        }
    }
}

/// Index of the first source folder containing `path`.
fn source_rank(sources: &[PathBuf], path: &Path) -> usize {
    sources
        .iter()
        .position(|s| path.starts_with(s))
        .unwrap_or(usize::MAX)
}

fn sort_by_capture(photos: &mut [PlannedPhoto]) {
    photos.sort_by(|a, b| {
        let time = |p: &PlannedPhoto| p.metadata.captured_at.map(|t| t.local);
        time(a).cmp(&time(b)).then_with(|| {
            primary_file(&a.files)
                .source
                .cmp(&primary_file(&b.files).source)
        })
    });
}

fn is_media(file: &PlannedFile) -> bool {
    file.format.role() != AssetRole::Sidecar
}

/// RAW if present, else the image.
pub(super) fn primary_file(files: &[PlannedFile]) -> &PlannedFile {
    files
        .iter()
        .find(|f| f.format == MediaFormat::Raw)
        .or_else(|| files.iter().find(|f| is_media(f)))
        .unwrap_or(&files[0])
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn stem_of(path: &Path) -> String {
    let name = file_name(path);
    base_name(&name).unwrap_or(&name).to_owned()
}

/// Lowercased base name of any directory entry, for collision checks.
fn stem_key(name: &str) -> String {
    base_name(name)
        .unwrap_or_else(|| name.rsplit_once('.').map_or(name, |(s, _)| s))
        .to_lowercase()
}

fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_owned())
        .unwrap_or_default()
}
