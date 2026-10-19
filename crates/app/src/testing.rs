//! Test doubles shared by the app crate's unit tests.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use picturesque_core::hash::ContentHash;
use picturesque_core::media::MediaFormat;
use picturesque_core::metadata::{CaptureMetadata, GeoPoint, Place};

use crate::ports::fs::{DirEntry, EntryKind, FileStamp, FileSystem, RenameError};
use crate::ports::geocoder::ReverseGeocoder;
use crate::ports::metadata::MetadataReader;

/// Hash used by the fake: BLAKE3 is an infrastructure concern, so the
/// fake uses a simple, deterministic stand-in.
pub fn fake_hash(bytes: &[u8]) -> ContentHash {
    let mut out = [0u8; 32];
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (i, slot) in out.iter_mut().enumerate() {
        for b in bytes.iter().chain([i as u8].iter()) {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        *slot = (h >> 24) as u8;
    }
    ContentHash::from_bytes(out)
}

#[derive(Default)]
struct State {
    files: BTreeMap<PathBuf, (Vec<u8>, SystemTime)>,
    dirs: BTreeSet<PathBuf>,
    symlinked_dirs: BTreeSet<PathBuf>,
    unreadable: BTreeSet<PathBuf>,
    read_only: BTreeSet<PathBuf>,
    /// The next N copies write damaged bytes.
    corrupt_next_copies: usize,
    /// Copies of these sources fail.
    failing_sources: BTreeSet<PathBuf>,
    /// Sources whose contents change right after the next copy of them.
    change_after_copy: BTreeSet<PathBuf>,
    /// Free space; `None` is unlimited. Copies use it up.
    space: Option<u64>,
    /// The next copy fills the disk (someone else wrote a big file).
    fill_during_next_copy: bool,
    clock: u64,
}

/// In-memory file system with failure injection. Paths are absolute.
#[derive(Default)]
pub struct MemFs {
    state: Mutex<State>,
}

impl MemFs {
    pub fn new() -> Self {
        Self::default()
    }

    fn add_dir(state: &mut State, path: &Path) {
        let mut p = Some(path);
        while let Some(dir) = p {
            if dir.as_os_str().is_empty() {
                break;
            }
            state.dirs.insert(dir.to_path_buf());
            p = dir.parent();
        }
    }

    pub fn file(self, path: &str, bytes: &[u8]) -> Self {
        self.write(Path::new(path), bytes);
        self
    }

    pub fn dir(self, path: &str) -> Self {
        Self::add_dir(&mut self.state.lock().unwrap(), Path::new(path));
        self
    }

    pub fn symlinked_dir(self, path: &str) -> Self {
        let mut s = self.state.lock().unwrap();
        Self::add_dir(&mut s, Path::new(path).parent().unwrap());
        s.symlinked_dirs.insert(PathBuf::from(path));
        drop(s);
        self
    }

    pub fn unreadable(self, path: &str) -> Self {
        self.state
            .lock()
            .unwrap()
            .unreadable
            .insert(PathBuf::from(path));
        self
    }

    pub fn read_only(self, path: &str) -> Self {
        self.state
            .lock()
            .unwrap()
            .read_only
            .insert(PathBuf::from(path));
        self
    }

    /// Deletes a file (as if removed outside the app).
    pub fn delete(&self, path: &Path) {
        let _ = self.remove_file(path);
    }

    /// Removes a folder and everything in it (a card taken out).
    pub fn delete_dir(&self, path: &Path) {
        let mut s = self.state.lock().unwrap();
        s.dirs.retain(|d| !d.starts_with(path));
        s.files.retain(|f, _| !f.starts_with(path));
    }

    pub fn write(&self, path: &Path, bytes: &[u8]) {
        let mut s = self.state.lock().unwrap();
        s.clock += 1;
        let time = SystemTime::UNIX_EPOCH + Duration::from_secs(s.clock);
        Self::add_dir(&mut s, path.parent().unwrap());
        s.files.insert(path.to_path_buf(), (bytes.to_vec(), time));
    }

    pub fn read(&self, path: &Path) -> Option<Vec<u8>> {
        self.state
            .lock()
            .unwrap()
            .files
            .get(path)
            .map(|(b, _)| b.clone())
    }

    /// All file paths under `dir`, sorted.
    pub fn files_under(&self, dir: &Path) -> Vec<PathBuf> {
        self.state
            .lock()
            .unwrap()
            .files
            .keys()
            .filter(|p| p.starts_with(dir))
            .cloned()
            .collect()
    }

    pub fn corrupt_next_copies(&self, n: usize) {
        self.state.lock().unwrap().corrupt_next_copies = n;
    }

    pub fn fail_copies_of(&self, path: &str) {
        self.state
            .lock()
            .unwrap()
            .failing_sources
            .insert(PathBuf::from(path));
    }

    /// Undoes `fail_copies_of`.
    pub fn allow_copies_of(&self, path: &str) {
        self.state
            .lock()
            .unwrap()
            .failing_sources
            .remove(Path::new(path));
    }

    /// Limits free space (`None`: unlimited); copies that do not fit fail.
    pub fn set_space(&self, bytes: Option<u64>) {
        self.state.lock().unwrap().space = bytes;
    }

    /// The next copy fails because the disk filled up meanwhile.
    pub fn fill_during_next_copy(&self) {
        self.state.lock().unwrap().fill_during_next_copy = true;
    }

    pub fn change_after_copy(&self, path: &str) {
        self.state
            .lock()
            .unwrap()
            .change_after_copy
            .insert(PathBuf::from(path));
    }
}

impl FileSystem for MemFs {
    fn probe_writable_dir(&self, path: &Path) -> Result<(), String> {
        let s = self.state.lock().unwrap();
        if s.files.contains_key(path) {
            return Err("it is not a folder".into());
        }
        if !s.dirs.contains(path) {
            return Err("it does not exist".into());
        }
        if s.read_only.contains(path) {
            return Err("it is not writable".into());
        }
        Ok(())
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, String> {
        let s = self.state.lock().unwrap();
        if s.unreadable.contains(path) {
            return Err("permission denied".into());
        }
        if !s.dirs.contains(path) {
            return Err("it does not exist".into());
        }
        let name = |p: &Path| p.file_name().unwrap().to_string_lossy().into_owned();
        let mut entries: Vec<DirEntry> = s
            .dirs
            .iter()
            .filter(|d| d.parent() == Some(path))
            .map(|d| DirEntry {
                name: name(d),
                kind: EntryKind::Dir,
                size: 0,
            })
            .collect();
        entries.extend(
            s.symlinked_dirs
                .iter()
                .filter(|d| d.parent() == Some(path))
                .map(|d| DirEntry {
                    name: name(d),
                    kind: EntryKind::Other,
                    size: 0,
                }),
        );
        entries.extend(
            s.files
                .iter()
                .filter(|(f, _)| f.parent() == Some(path))
                .map(|(f, (b, _))| DirEntry {
                    name: name(f),
                    kind: EntryKind::File,
                    size: b.len() as u64,
                }),
        );
        Ok(entries)
    }

    fn exists(&self, path: &Path) -> bool {
        let s = self.state.lock().unwrap();
        s.files.contains_key(path) || s.dirs.contains(path)
    }

    fn stamp(&self, path: &Path) -> Result<FileStamp, String> {
        let s = self.state.lock().unwrap();
        let (bytes, time) = s.files.get(path).ok_or("it does not exist")?;
        Ok(FileStamp {
            size: bytes.len() as u64,
            modified: Some(*time),
        })
    }

    fn hash_file(&self, path: &Path) -> Result<(ContentHash, u64), String> {
        let bytes = self.read(path).ok_or("it does not exist")?;
        Ok((fake_hash(&bytes), bytes.len() as u64))
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), String> {
        Self::add_dir(&mut self.state.lock().unwrap(), path);
        Ok(())
    }

    fn copy_hashing(&self, src: &Path, dst: &Path) -> Result<(ContentHash, u64), String> {
        let (bytes, corrupt, change) = {
            let mut s = self.state.lock().unwrap();
            if s.failing_sources.contains(src) {
                return Err("input/output error".into());
            }
            let bytes = s
                .files
                .get(src)
                .map(|(b, _)| b.clone())
                .ok_or("it does not exist")?;
            if std::mem::take(&mut s.fill_during_next_copy) {
                s.space = Some(0);
            }
            if let Some(space) = s.space.as_mut() {
                if (bytes.len() as u64) > *space {
                    return Err("No space left on device (os error 28)".into());
                }
                *space -= bytes.len() as u64;
            }
            let corrupt = s.corrupt_next_copies > 0;
            if corrupt {
                s.corrupt_next_copies -= 1;
            }
            (bytes, corrupt, s.change_after_copy.remove(src))
        };
        let mut written = bytes.clone();
        if corrupt {
            written.push(0xFF);
        }
        self.write(dst, &written);
        if change {
            let mut changed = bytes.clone();
            changed.extend_from_slice(b"edited");
            self.write(src, &changed);
        }
        Ok((fake_hash(&bytes), bytes.len() as u64))
    }

    fn rename_no_replace(&self, from: &Path, to: &Path) -> Result<(), RenameError> {
        let mut s = self.state.lock().unwrap();
        if s.files.contains_key(to) {
            return Err(RenameError::Exists);
        }
        let file = s
            .files
            .remove(from)
            .ok_or_else(|| RenameError::Other("it does not exist".into()))?;
        Self::add_dir(&mut s, to.parent().unwrap());
        s.files.insert(to.to_path_buf(), file);
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<(), String> {
        let mut s = self.state.lock().unwrap();
        if let Some((bytes, _)) = s.files.remove(path)
            && let Some(space) = s.space.as_mut()
        {
            *space += bytes.len() as u64;
        }
        Ok(())
    }

    fn available_space(&self, _path: &Path) -> Result<u64, String> {
        Ok(self.state.lock().unwrap().space.unwrap_or(u64::MAX))
    }
}

/// Metadata reader returning canned metadata per path; files not listed
/// get empty metadata, and listed failures return an error.
#[derive(Default)]
pub struct FakeMetadata {
    pub by_path: HashMap<PathBuf, CaptureMetadata>,
    pub failing: Vec<PathBuf>,
}

impl MetadataReader for FakeMetadata {
    fn read(&self, path: &Path, _format: MediaFormat) -> Result<CaptureMetadata, String> {
        if self.failing.iter().any(|p| p == path) {
            return Err("corrupt file".into());
        }
        Ok(self.by_path.get(path).cloned().unwrap_or_default())
    }
}

/// Geocoder that names every point "Lisbon".
pub struct FakeGeocoder;

impl ReverseGeocoder for FakeGeocoder {
    fn place(&self, _point: GeoPoint) -> Option<Place> {
        Some(Place {
            city: "Lisbon".into(),
            region: Some("Lisbon".into()),
            country: "Portugal".into(),
            country_code: "PT".into(),
        })
    }
}

// --- Catalog, events -------------------------------------------------

use std::collections::HashSet as StdHashSet;
use std::sync::Arc;

use picturesque_core::photo::{NewPhoto, Photo, PhotoId};

use crate::api::AppContext;
use crate::ports::catalog::{AnalyzerId, CatalogError, PhotoCatalog};
use crate::ports::events::{AppEvent, EventSink};
use crate::settings::tests::MemorySettings;

/// Photos kept in memory, with the catalog's one-copy rule for media.
#[derive(Default)]
pub struct MemoryCatalog {
    pub photos: Mutex<Vec<Photo>>,
    pub decisions: Mutex<HashMap<PhotoId, picturesque_core::decision::Decision>>,
    /// (revision, revision the sidecar reflects) per decision.
    pub revisions: Mutex<HashMap<PhotoId, (u64, Option<u64>)>>,
    /// Photos marked to carry the auto-pick label.
    pub auto_labeled: Mutex<StdHashSet<PhotoId>>,
    /// (photo, analyzer) → (version, result).
    pub analysis: Mutex<HashMap<(PhotoId, String), (u32, serde_json::Value)>>,
    /// Results marked for analysis again.
    pub stale: Mutex<StdHashSet<(PhotoId, String)>>,
    pub series_edits: Mutex<HashMap<PhotoId, picturesque_core::series::SeriesEdit>>,
    pub series: Mutex<Vec<crate::ports::catalog::StoredSeries>>,
    pub best_choices: Mutex<Vec<PhotoId>>,
    /// Combined scores and AI signals.
    pub scores: Mutex<HashMap<PhotoId, crate::ports::catalog::PhotoScore>>,
    /// Photos moved to the rejected folder → batch.
    pub reject_moves: Mutex<HashMap<PhotoId, i64>>,
    /// Writes of photos and analysis fail as if the disk were full.
    pub full: std::sync::atomic::AtomicBool,
}

impl MemoryCatalog {
    fn check_space(&self) -> Result<(), CatalogError> {
        if self.full.load(std::sync::atomic::Ordering::Relaxed) {
            Err(CatalogError::Full)
        } else {
            Ok(())
        }
    }
}

impl MemoryCatalog {
    fn is_analyzed(&self, id: PhotoId, analyzer: AnalyzerId) -> bool {
        self.analysis
            .lock()
            .unwrap()
            .get(&(id, analyzer.name.to_owned()))
            .is_some_and(|(v, _)| *v == analyzer.version)
            && !self
                .stale
                .lock()
                .unwrap()
                .contains(&(id, analyzer.name.to_owned()))
    }
}

impl PhotoCatalog for MemoryCatalog {
    fn insert_photo(&self, photo: &NewPhoto) -> Result<PhotoId, CatalogError> {
        self.check_space()?;
        let mut photos = self.photos.lock().unwrap();
        for asset in photo
            .assets
            .iter()
            .filter(|a| a.role() != picturesque_core::media::AssetRole::Sidecar)
        {
            let taken = photos.iter().flat_map(|p| p.assets.iter()).any(|a| {
                a.hash == asset.hash && a.role() != picturesque_core::media::AssetRole::Sidecar
            });
            if taken {
                return Err(CatalogError::Duplicate(asset.hash.to_string()));
            }
        }
        let id = PhotoId(photos.len() as i64 + 1);
        photos.push(Photo {
            id,
            metadata: photo.metadata.clone(),
            assets: photo.assets.clone(),
        });
        Ok(id)
    }

    fn photo(&self, id: PhotoId) -> Result<Option<Photo>, CatalogError> {
        Ok(self
            .photos
            .lock()
            .unwrap()
            .iter()
            .find(|p| p.id == id)
            .cloned())
    }

    fn photo_with_media_hash(&self, hash: &ContentHash) -> Result<Option<PhotoId>, CatalogError> {
        Ok(self
            .photos
            .lock()
            .unwrap()
            .iter()
            .find(|p| {
                p.assets
                    .raw()
                    .into_iter()
                    .chain(p.assets.image())
                    .any(|a| a.hash == *hash)
            })
            .map(|p| p.id))
    }

    fn photo_count(&self) -> Result<u64, CatalogError> {
        Ok(self.photos.lock().unwrap().len() as u64)
    }

    fn all_photos(&self) -> Result<Vec<Photo>, CatalogError> {
        Ok(self.photos.lock().unwrap().clone())
    }

    fn set_dimensions(&self, id: PhotoId, width: u32, height: u32) -> Result<(), CatalogError> {
        let mut photos = self.photos.lock().unwrap();
        let photo = photos
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| CatalogError::Storage("no photo".into()))?;
        photo.metadata.dimensions = Some((width, height));
        Ok(())
    }

    /// Only the flag and photo-id filters, newest (or oldest) first, in one
    /// page;
    /// queries are tested in full against SQLite.
    fn query(
        &self,
        query: &crate::library::PhotoQuery,
    ) -> Result<crate::library::PhotoPage, CatalogError> {
        let decisions = self.decisions.lock().unwrap().clone();
        let scores = self.scores.lock().unwrap().clone();
        let keeper = |id: &PhotoId| scores.get(id).is_some_and(|s| s.keeper);
        let mut photos: Vec<Photo> = self
            .photos
            .lock()
            .unwrap()
            .iter()
            .filter(|p| {
                let flag = decisions.get(&p.id).map(|d| d.flag).unwrap_or_default();
                let auto = keeper(&p.id) && flag == picturesque_core::decision::Flag::Unflagged;
                // Flags and auto-picked photos match either.
                let decided = match (&query.flags, query.auto_picked == Some(true)) {
                    (None, false) => true,
                    (None, true) => auto,
                    (Some(f), false) => f.contains(&flag),
                    (Some(f), true) => f.contains(&flag) || auto,
                };
                decided
                    && query
                        .photo_ids
                        .as_ref()
                        .is_none_or(|ids| ids.contains(&p.id.0))
            })
            .cloned()
            .collect();
        photos.sort_by(|a, b| {
            let t = |p: &Photo| p.metadata.captured_at.map(|t| t.local);
            t(b).cmp(&t(a)).then(b.id.cmp(&a.id))
        });
        if query.sort == Some(crate::library::PhotoSort::CapturedAsc) {
            photos.reverse();
        }
        let summaries: Vec<crate::library::PhotoSummary> = photos
            .iter()
            .map(|p| {
                let d = decisions.get(&p.id).copied().unwrap_or_default();
                crate::library::PhotoSummary {
                    id: p.id.0,
                    captured_at: None,
                    day: None,
                    camera: None,
                    lens: None,
                    city: None,
                    country: None,
                    flag: d.flag,
                    rating: d.rating.stars(),
                    width: None,
                    height: None,
                    has_raw: p.assets.raw().is_some(),
                    file_name: String::new(),
                    series_id: None,
                    best_in_series: false,
                    series_size: 0,
                    quality: None,
                    aesthetic: None,
                    eyes_closed: None,
                    faces: 0,
                    keeper: keeper(&p.id),
                }
            })
            .collect();
        let total = summaries.len();
        Ok(crate::library::PhotoPage {
            photos: summaries,
            next_cursor: None,
            total,
        })
    }

    fn facets(
        &self,
        _filter: Option<&crate::library::PhotoQuery>,
    ) -> Result<crate::library::PhotoFacets, CatalogError> {
        Err(CatalogError::Storage(
            "facets are tested against SQLite".into(),
        ))
    }

    fn gear_usage(&self) -> Result<crate::stats::GearUsage, CatalogError> {
        Err(CatalogError::Storage(
            "statistics are tested against SQLite".into(),
        ))
    }

    fn decision(&self, id: PhotoId) -> Result<picturesque_core::decision::Decision, CatalogError> {
        Ok(self
            .decisions
            .lock()
            .unwrap()
            .get(&id)
            .copied()
            .unwrap_or_default())
    }

    fn set_decision(
        &self,
        id: PhotoId,
        decision: picturesque_core::decision::Decision,
    ) -> Result<(), CatalogError> {
        if !self.photos.lock().unwrap().iter().any(|p| p.id == id) {
            return Err(CatalogError::Storage(format!("no photo {}", id.0)));
        }
        self.decisions.lock().unwrap().insert(id, decision);
        self.revisions
            .lock()
            .unwrap()
            .entry(id)
            .or_insert((0, None))
            .0 += 1;
        Ok(())
    }

    fn set_decision_saved(
        &self,
        id: PhotoId,
        decision: picturesque_core::decision::Decision,
    ) -> Result<(), CatalogError> {
        self.set_decision(id, decision)?;
        if let Some((rev, written)) = self.revisions.lock().unwrap().get_mut(&id) {
            *written = Some(*rev);
        }
        Ok(())
    }

    fn dirty_decisions(&self) -> Result<Vec<crate::ports::catalog::DirtyDecision>, CatalogError> {
        let decisions = self.decisions.lock().unwrap();
        let auto = self.auto_labeled.lock().unwrap();
        let mut dirty: Vec<_> = self
            .revisions
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, (rev, written))| *written != Some(*rev))
            .map(|(id, (rev, _))| crate::ports::catalog::DirtyDecision {
                id: *id,
                decision: decisions.get(id).copied().unwrap_or_default(),
                token: rev.to_string(),
                auto_labeled: auto.contains(id),
            })
            .collect();
        dirty.sort_by_key(|d| d.id);
        Ok(dirty)
    }

    fn auto_labeled(&self) -> Result<StdHashSet<PhotoId>, CatalogError> {
        Ok(self.auto_labeled.lock().unwrap().clone())
    }

    fn set_auto_labeled(&self, ids: &[PhotoId]) -> Result<usize, CatalogError> {
        let wanted: StdHashSet<PhotoId> = ids.iter().copied().collect();
        let mut auto = self.auto_labeled.lock().unwrap();
        let changed: Vec<PhotoId> = auto.symmetric_difference(&wanted).copied().collect();
        let mut revisions = self.revisions.lock().unwrap();
        for id in &changed {
            revisions.entry(*id).or_insert((0, None)).0 += 1;
        }
        *auto = wanted;
        Ok(changed.len())
    }

    fn mark_sidecar_written(&self, id: PhotoId, token: &str) -> Result<(), CatalogError> {
        if let Some((rev, written)) = self.revisions.lock().unwrap().get_mut(&id)
            && rev.to_string() == token
        {
            *written = Some(*rev);
        }
        Ok(())
    }

    fn upsert_sidecar(
        &self,
        id: PhotoId,
        path: &Path,
        size: u64,
        hash: &ContentHash,
    ) -> Result<(), CatalogError> {
        let mut photos = self.photos.lock().unwrap();
        let photo = photos
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| CatalogError::Storage("no photo".into()))?;
        let mut assets: Vec<picturesque_core::photo::Asset> = photo.assets.clone().into();
        match assets.iter_mut().find(|a| a.path == path) {
            Some(a) => {
                a.size = size;
                a.hash = *hash;
            }
            None => assets.push(picturesque_core::photo::Asset {
                format: MediaFormat::Xmp,
                path: path.to_path_buf(),
                size,
                hash: *hash,
            }),
        }
        photo.assets = picturesque_core::photo::PhotoAssets::new(assets)
            .map_err(|e| CatalogError::Storage(e.to_string()))?;
        Ok(())
    }

    fn pending_analysis(
        &self,
        analyzers: &[AnalyzerId],
        limit: usize,
    ) -> Result<Vec<PhotoId>, CatalogError> {
        let mut photos: Vec<(bool, Option<chrono::NaiveDateTime>, PhotoId)> = self
            .photos
            .lock()
            .unwrap()
            .iter()
            .map(|p| {
                let time = p.metadata.captured_at.map(|t| t.local);
                (time.is_none(), time, p.id)
            })
            .collect();
        // Oldest first, undated last.
        photos.sort();
        Ok(photos
            .into_iter()
            .map(|(_, _, id)| id)
            .filter(|id| analyzers.iter().any(|a| !self.is_analyzed(*id, *a)))
            .take(limit)
            .collect())
    }

    fn analysis_progress(&self, analyzers: &[AnalyzerId]) -> Result<(usize, usize), CatalogError> {
        let total = self.photos.lock().unwrap().len();
        let pending = self.pending_analysis(analyzers, usize::MAX)?.len();
        Ok((pending, total))
    }

    fn analyzed(
        &self,
        id: PhotoId,
        analyzers: &[AnalyzerId],
    ) -> Result<Vec<AnalyzerId>, CatalogError> {
        Ok(analyzers
            .iter()
            .copied()
            .filter(|a| self.is_analyzed(id, *a))
            .collect())
    }

    fn store_analysis(
        &self,
        id: PhotoId,
        analyzer: AnalyzerId,
        result: &serde_json::Value,
    ) -> Result<(), CatalogError> {
        self.check_space()?;
        if self.photo(id)?.is_none() {
            return Err(CatalogError::Storage(format!("no photo {}", id.0)));
        }
        self.analysis.lock().unwrap().insert(
            (id, analyzer.name.to_owned()),
            (analyzer.version, result.clone()),
        );
        self.stale
            .lock()
            .unwrap()
            .remove(&(id, analyzer.name.to_owned()));
        Ok(())
    }

    fn mark_analysis_stale(&self, ids: Option<&[PhotoId]>) -> Result<usize, CatalogError> {
        let photos: Vec<PhotoId> = self.photos.lock().unwrap().iter().map(|p| p.id).collect();
        let marked: Vec<PhotoId> = match ids {
            None => photos,
            Some(ids) => ids
                .iter()
                .copied()
                .filter(|id| photos.contains(id))
                .collect(),
        };
        let analysis = self.analysis.lock().unwrap();
        let mut stale = self.stale.lock().unwrap();
        for (id, name) in analysis.keys() {
            if marked.contains(id) {
                stale.insert((*id, name.clone()));
            }
        }
        Ok(marked.len())
    }

    fn analysis(&self, id: PhotoId) -> Result<Vec<(String, u32, serde_json::Value)>, CatalogError> {
        let mut results: Vec<_> = self
            .analysis
            .lock()
            .unwrap()
            .iter()
            .filter(|((p, _), _)| *p == id)
            .map(|((_, name), (version, value))| (name.clone(), *version, value.clone()))
            .collect();
        results.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(results)
    }

    fn capture_times(&self) -> Result<Vec<(PhotoId, Option<chrono::NaiveDateTime>)>, CatalogError> {
        Ok(self
            .photos
            .lock()
            .unwrap()
            .iter()
            .map(|p| (p.id, p.metadata.captured_at.map(|t| t.local)))
            .collect())
    }

    fn analysis_results(
        &self,
        analyzer: AnalyzerId,
    ) -> Result<Vec<(PhotoId, serde_json::Value)>, CatalogError> {
        let mut results: Vec<_> = self
            .analysis
            .lock()
            .unwrap()
            .iter()
            .filter(|((_, name), (version, _))| {
                name == analyzer.name && *version == analyzer.version
            })
            .map(|((id, _), (_, value))| (*id, value.clone()))
            .collect();
        results.sort_by_key(|(id, _)| *id);
        Ok(results)
    }

    fn series_edits(
        &self,
    ) -> Result<Vec<(PhotoId, picturesque_core::series::SeriesEdit)>, CatalogError> {
        let mut edits: Vec<_> = self
            .series_edits
            .lock()
            .unwrap()
            .iter()
            .map(|(id, e)| (*id, *e))
            .collect();
        edits.sort_by_key(|(id, _)| *id);
        Ok(edits)
    }

    fn set_series_edit(
        &self,
        id: PhotoId,
        edit: Option<picturesque_core::series::SeriesEdit>,
    ) -> Result<(), CatalogError> {
        let mut edits = self.series_edits.lock().unwrap();
        match edit {
            Some(e) => {
                edits.insert(id, e);
            }
            None => {
                edits.remove(&id);
            }
        }
        Ok(())
    }

    fn replace_series(
        &self,
        series: &[crate::ports::catalog::StoredSeries],
    ) -> Result<(), CatalogError> {
        *self.series.lock().unwrap() = series.to_vec();
        Ok(())
    }

    fn series_of(
        &self,
        id: PhotoId,
    ) -> Result<Option<(PhotoId, crate::ports::catalog::StoredSeries)>, CatalogError> {
        Ok(self
            .series
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.members.contains(&id))
            .map(|s| (s.members[0], s.clone())))
    }

    fn best_choices(&self) -> Result<Vec<PhotoId>, CatalogError> {
        Ok(self.best_choices.lock().unwrap().clone())
    }

    fn set_best_choice(&self, id: PhotoId, chosen: bool) -> Result<(), CatalogError> {
        let mut choices = self.best_choices.lock().unwrap();
        choices.retain(|c| *c != id);
        if chosen {
            choices.push(id);
        }
        Ok(())
    }

    fn rejected_photos(&self) -> Result<Vec<PhotoId>, CatalogError> {
        let mut ids: Vec<PhotoId> = self
            .decisions
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, d)| d.flag == picturesque_core::decision::Flag::Reject)
            .map(|(id, _)| *id)
            .collect();
        ids.sort();
        Ok(ids)
    }

    fn move_assets(&self, id: PhotoId, moves: &[(PathBuf, PathBuf)]) -> Result<(), CatalogError> {
        let mut photos = self.photos.lock().unwrap();
        let photo = photos
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| CatalogError::Storage("no photo".into()))?;
        let mut assets: Vec<picturesque_core::photo::Asset> = photo.assets.clone().into();
        for (old, new) in moves {
            let asset = assets
                .iter_mut()
                .find(|a| &a.path == old)
                .ok_or_else(|| CatalogError::Storage(format!("no asset {}", old.display())))?;
            asset.path = new.clone();
        }
        photo.assets = picturesque_core::photo::PhotoAssets::new(assets)
            .map_err(|e| CatalogError::Storage(e.to_string()))?;
        Ok(())
    }

    fn record_reject_move(&self, batch: i64, ids: &[PhotoId]) -> Result<(), CatalogError> {
        let mut moves = self.reject_moves.lock().unwrap();
        for id in ids {
            moves.insert(*id, batch);
        }
        Ok(())
    }

    fn last_reject_move(&self) -> Result<Option<(i64, Vec<PhotoId>)>, CatalogError> {
        let moves = self.reject_moves.lock().unwrap();
        let Some(batch) = moves.values().max().copied() else {
            return Ok(None);
        };
        let mut ids: Vec<PhotoId> = moves
            .iter()
            .filter(|(_, b)| **b == batch)
            .map(|(id, _)| *id)
            .collect();
        ids.sort();
        Ok(Some((batch, ids)))
    }

    fn forget_reject_move(&self, ids: &[PhotoId]) -> Result<(), CatalogError> {
        let mut moves = self.reject_moves.lock().unwrap();
        for id in ids {
            moves.remove(id);
        }
        Ok(())
    }

    fn replace_scores(
        &self,
        scores: &[crate::ports::catalog::PhotoScore],
    ) -> Result<(), CatalogError> {
        *self.scores.lock().unwrap() = scores.iter().map(|s| (s.id, *s)).collect();
        Ok(())
    }

    fn media_sizes(&self) -> Result<StdHashSet<u64>, CatalogError> {
        Ok(self
            .photos
            .lock()
            .unwrap()
            .iter()
            .flat_map(|p| {
                p.assets
                    .raw()
                    .into_iter()
                    .chain(p.assets.image())
                    .map(|a| a.size)
                    .collect::<Vec<_>>()
            })
            .collect())
    }
}

/// Records emitted events.
#[derive(Default)]
pub struct RecordedEvents(pub Mutex<Vec<AppEvent>>);

impl EventSink for RecordedEvents {
    fn emit(&self, event: &AppEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

/// Decoder that writes a small marker file and records each call.
#[derive(Default)]
pub struct FakeDecoder {
    pub calls: Mutex<Vec<(PathBuf, u32)>>,
    /// Source size it reports, upright.
    pub source_size: Option<(u32, u32)>,
    pub fail: bool,
    pub fs: std::sync::OnceLock<Arc<MemFs>>,
}

impl crate::ports::images::ImageDecoder for FakeDecoder {
    fn render(
        &self,
        source: &Path,
        _format: MediaFormat,
        _orientation: picturesque_core::metadata::Orientation,
        max_edge: u32,
        dest: &Path,
    ) -> Result<crate::ports::images::Rendered, String> {
        self.calls
            .lock()
            .unwrap()
            .push((source.to_path_buf(), max_edge));
        if self.fail {
            return Err("unsupported image".into());
        }
        let fs = self.fs.get().expect("fake decoder needs the test fs");
        if !fs.exists(source) {
            return Err("it does not exist".into());
        }
        fs.write(dest, format!("jpeg {max_edge}").as_bytes());
        Ok(crate::ports::images::Rendered {
            width: max_edge,
            height: max_edge * 2 / 3,
            source_size: self.source_size,
        })
    }
}

/// Pixel loader returning a solid grey image of the requested size.
#[derive(Default)]
pub struct FakePixels {
    pub calls: Mutex<Vec<PathBuf>>,
    pub fail: bool,
}

impl crate::ports::pixels::PixelLoader for FakePixels {
    fn load(
        &self,
        path: &Path,
        max_edge: u32,
    ) -> Result<picturesque_core::pixels::RgbImage, String> {
        self.calls.lock().unwrap().push(path.to_path_buf());
        if self.fail {
            return Err("cannot decode".into());
        }
        Ok(picturesque_core::pixels::RgbImage::filled(
            max_edge,
            max_edge * 2 / 3,
            [128, 128, 128],
        ))
    }
}

/// An inference engine whose GPU providers always work and whose models
/// echo their inputs (integers as floats); other providers fall back to
/// CPU.
#[derive(Debug, Default)]
pub struct FakeInference {
    gpus: Vec<crate::ports::inference::Provider>,
    broken: bool,
    probes: Mutex<usize>,
}

impl FakeInference {
    pub fn new(gpus: &[crate::ports::inference::Provider]) -> Self {
        Self {
            gpus: gpus.to_vec(),
            ..Default::default()
        }
    }

    /// Nothing loads, not even on CPU.
    pub fn broken() -> Self {
        Self {
            broken: true,
            ..Default::default()
        }
    }

    pub fn probes(&self) -> usize {
        *self.probes.lock().unwrap()
    }
}

#[derive(Debug)]
struct EchoModel {
    provider: crate::ports::inference::Provider,
    fallback: Option<String>,
}

impl crate::ports::inference::Model for EchoModel {
    fn provider(&self) -> crate::ports::inference::Provider {
        self.provider
    }

    fn fallback(&self) -> Option<&str> {
        self.fallback.as_deref()
    }

    fn run(
        &self,
        inputs: Vec<crate::ports::inference::Tensor>,
    ) -> Result<Vec<crate::ports::inference::Tensor>, crate::ports::inference::InferenceError> {
        use crate::ports::inference::TensorData;
        // Integer inputs (token ids) come back as floats, like embeddings.
        Ok(inputs
            .into_iter()
            .map(|mut t| {
                t.data = match t.data {
                    TensorData::I32(v) => {
                        TensorData::F32(v.into_iter().map(|x| x as f32).collect())
                    }
                    TensorData::I64(v) => {
                        TensorData::F32(v.into_iter().map(|x| x as f32).collect())
                    }
                    f => f,
                };
                t
            })
            .collect())
    }
}

impl crate::ports::inference::InferenceEngine for FakeInference {
    fn available(&self) -> Vec<crate::ports::inference::Provider> {
        let mut all = self.gpus.clone();
        all.push(crate::ports::inference::Provider::Cpu);
        all
    }

    fn load(
        &self,
        _model: crate::ports::inference::ModelSource<'_>,
        choice: crate::ports::inference::ProviderChoice,
    ) -> Result<Box<dyn crate::ports::inference::Model>, crate::ports::inference::InferenceError>
    {
        use crate::ports::inference::{InferenceError, Provider};
        if self.broken {
            return Err(InferenceError("CPU: runtime missing".into()));
        }
        let mut failures = Vec::new();
        for provider in choice.candidates(&self.gpus) {
            if provider == Provider::Cpu || self.gpus.contains(&provider) {
                return Ok(Box::new(EchoModel {
                    provider,
                    fallback: (!failures.is_empty()).then(|| failures.join("; ")),
                }));
            }
            failures.push(format!("{} is not available", provider.label()));
        }
        unreachable!("candidates end with CPU")
    }

    fn probe(
        &self,
        choice: crate::ports::inference::ProviderChoice,
    ) -> Result<crate::ports::inference::Probe, crate::ports::inference::InferenceError> {
        *self.probes.lock().unwrap() += 1;
        let model = self.load(crate::ports::inference::ModelSource::Bytes(&[]), choice)?;
        Ok(crate::ports::inference::Probe {
            provider: model.provider(),
            fallback: model.fallback().map(str::to_owned),
        })
    }
}

/// Model files kept in memory. Fetches deliver `served` bytes for a URL
/// (in chunks, so progress and cancelling can be tested), checked against
/// the expected size and checksum like the real store.
#[derive(Default)]
pub struct MemoryModelStore {
    /// URL → bytes the "server" returns; missing URLs are network errors.
    pub served: Mutex<HashMap<String, Vec<u8>>>,
    /// Stored files: (model id, version, file name) → bytes.
    pub files: Mutex<HashMap<(String, u32, String), Vec<u8>>>,
    /// Files that count as stored whatever their bytes (see `install`).
    pub installed: Mutex<std::collections::HashSet<(String, u32, String)>>,
    /// Blocks fetches until set to `false` (to observe downloads).
    pub hold: std::sync::atomic::AtomicBool,
    pub fetches: Mutex<Vec<String>>,
}

impl MemoryModelStore {
    /// Marks every file of `model` as downloaded, with `contents` for the
    /// files that need them (others read as empty).
    pub fn install(&self, model: &crate::ports::models::ModelSpec, contents: &[(&str, &[u8])]) {
        for f in &model.files {
            let key = (model.id.clone(), model.version, f.name.clone());
            let bytes = contents
                .iter()
                .find(|(name, _)| *name == f.name)
                .map_or_else(Vec::new, |(_, b)| b.to_vec());
            self.files.lock().unwrap().insert(key.clone(), bytes);
            self.installed.lock().unwrap().insert(key);
        }
    }
}

impl crate::ports::models::ModelStore for MemoryModelStore {
    fn path(
        &self,
        model: &crate::ports::models::ModelSpec,
        file: &crate::ports::models::ModelFile,
    ) -> PathBuf {
        PathBuf::from(format!(
            "/models/{}/v{}/{}",
            model.id, model.version, file.name
        ))
    }

    fn has(
        &self,
        model: &crate::ports::models::ModelSpec,
        file: &crate::ports::models::ModelFile,
    ) -> bool {
        let key = (model.id.clone(), model.version, file.name.clone());
        self.installed.lock().unwrap().contains(&key)
            || self
                .files
                .lock()
                .unwrap()
                .get(&key)
                .is_some_and(|b| b.len() as u64 == file.size)
    }

    fn fetch(
        &self,
        model: &crate::ports::models::ModelSpec,
        file: &crate::ports::models::ModelFile,
        progress: &mut dyn FnMut(u64) -> bool,
    ) -> Result<(), crate::ports::models::FetchError> {
        use crate::ports::models::FetchError;
        self.fetches.lock().unwrap().push(file.url.clone());
        while self.hold.load(std::sync::atomic::Ordering::SeqCst) {
            if !progress(0) {
                return Err(FetchError::Cancelled);
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let bytes = self
            .served
            .lock()
            .unwrap()
            .get(&file.url)
            .cloned()
            .ok_or_else(|| FetchError::Network("404".into()))?;
        for (i, _) in bytes.chunks(4).enumerate() {
            if !progress(((i + 1) * 4).min(bytes.len()) as u64) {
                return Err(FetchError::Cancelled);
            }
        }
        if bytes.len() as u64 != file.size || fake_sha256(&bytes) != file.sha256 {
            return Err(FetchError::Corrupt("checksum mismatch".into()));
        }
        self.files
            .lock()
            .unwrap()
            .insert((model.id.clone(), model.version, file.name.clone()), bytes);
        Ok(())
    }

    fn read(
        &self,
        model: &crate::ports::models::ModelSpec,
        file: &crate::ports::models::ModelFile,
    ) -> Result<Vec<u8>, String> {
        self.files
            .lock()
            .unwrap()
            .get(&(model.id.clone(), model.version, file.name.clone()))
            .cloned()
            .ok_or_else(|| "not downloaded".into())
    }

    fn prune(&self, model: &crate::ports::models::ModelSpec) -> Result<(), String> {
        self.files
            .lock()
            .unwrap()
            .retain(|(id, version, _), _| id != &model.id || *version == model.version);
        self.installed
            .lock()
            .unwrap()
            .retain(|(id, version, _)| id != &model.id || *version == model.version);
        Ok(())
    }

    fn remove(&self, model: &crate::ports::models::ModelSpec) -> Result<(), String> {
        self.files
            .lock()
            .unwrap()
            .retain(|(id, _, _), _| id != &model.id);
        self.installed
            .lock()
            .unwrap()
            .retain(|(id, _, _)| id != &model.id);
        Ok(())
    }
}

/// Stand-in checksum for `MemoryModelStore` (the app layer has no SHA-256).
pub fn fake_sha256(bytes: &[u8]) -> String {
    format!("{:064x}", bytes.iter().map(|b| u64::from(*b)).sum::<u64>())
}

/// Sidecars kept in memory: path → decision.
#[derive(Default)]
pub struct MemorySidecars(
    pub Mutex<HashMap<PathBuf, picturesque_core::decision::Decision>>,
    /// The colour label last written to each sidecar.
    pub Mutex<HashMap<PathBuf, Option<crate::ports::sidecars::ColorLabel>>>,
);

impl crate::ports::sidecars::SidecarStore for MemorySidecars {
    fn read(&self, path: &Path) -> Result<Option<picturesque_core::decision::Decision>, String> {
        Ok(self.0.lock().unwrap().get(path).copied())
    }

    fn write(
        &self,
        path: &Path,
        values: crate::ports::sidecars::SidecarValues,
    ) -> Result<(ContentHash, u64), String> {
        self.0
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), values.decision);
        self.1
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), values.label);
        Ok((fake_hash(path.to_string_lossy().as_bytes()), 1))
    }
}

/// Everything a test needs, with handles to inspect afterwards.
pub struct TestWorld {
    pub fs: Arc<MemFs>,
    pub settings: Arc<MemorySettings>,
    pub catalog: Arc<MemoryCatalog>,
    pub metadata: Arc<FakeMetadata>,
    pub events: Arc<RecordedEvents>,
    pub decoder: Arc<FakeDecoder>,
    pub sidecars: Arc<MemorySidecars>,
    pub pixels: Arc<FakePixels>,
    pub pipeline: Arc<MemoryPipelineStore>,
    pub watcher: Arc<FakeWatcher>,
}

/// Records what is watched; tests report changes with [`FakeWatcher::touch`].
#[derive(Default)]
pub struct FakeWatcher {
    state: Mutex<(Vec<PathBuf>, Option<crate::ports::watcher::ChangeHandler>)>,
    /// Folders that cannot be watched.
    pub failing: Mutex<Vec<PathBuf>>,
}

impl std::fmt::Debug for FakeWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeWatcher").finish_non_exhaustive()
    }
}

impl FakeWatcher {
    pub fn folders(&self) -> Vec<PathBuf> {
        self.state.lock().unwrap().0.clone()
    }

    /// Reports a change in a watched folder.
    pub fn touch(&self, folder: &str) {
        let handler = self.state.lock().unwrap().1.clone();
        if let Some(handler) = handler {
            handler(Path::new(folder));
        }
    }
}

impl crate::ports::watcher::FolderWatcher for FakeWatcher {
    fn watch(
        &self,
        folders: &[PathBuf],
        changed: crate::ports::watcher::ChangeHandler,
    ) -> Vec<(PathBuf, String)> {
        *self.state.lock().unwrap() = (folders.to_vec(), Some(changed));
        let failing = self.failing.lock().unwrap();
        folders
            .iter()
            .filter(|f| failing.contains(f))
            .map(|f| (f.clone(), "too many watches".into()))
            .collect()
    }
}

impl TestWorld {
    pub fn new(fs: MemFs, metadata: FakeMetadata) -> Self {
        Self::with_decoder(fs, metadata, FakeDecoder::default())
    }

    pub fn with_decoder(fs: MemFs, metadata: FakeMetadata, decoder: FakeDecoder) -> Self {
        let fs = Arc::new(fs);
        let _ = decoder.fs.set(Arc::clone(&fs));
        Self {
            decoder: Arc::new(decoder),
            sidecars: Arc::default(),
            pixels: Arc::default(),
            fs,
            settings: Arc::default(),
            catalog: Arc::default(),
            metadata: Arc::new(metadata),
            events: Arc::default(),
            pipeline: Arc::default(),
            watcher: Arc::default(),
        }
    }

    /// An import pipeline over this world (not started).
    pub fn pipeline(&self) -> Arc<crate::pipeline::ImportPipeline> {
        Arc::new(crate::pipeline::ImportPipeline::new(
            crate::pipeline::PipelineDeps {
                store: self.pipeline.clone(),
                catalog: self.catalog.clone(),
                settings: self.settings.clone(),
                fs: self.fs.clone(),
                metadata: self.metadata.clone(),
                geocoder: Arc::new(FakeGeocoder),
                sidecars: self.sidecars.clone(),
                events: self.events.clone(),
                watcher: self.watcher.clone(),
            },
        ))
    }

    pub fn context(&self) -> AppContext {
        AppContext {
            settings_store: self.settings.clone(),
            catalog: self.catalog.clone(),
            fs: self.fs.clone(),
            metadata: self.metadata.clone(),
            geocoder: Arc::new(FakeGeocoder),
            events: self.events.clone(),
            decoder: self.decoder.clone(),
            cache_dir: PathBuf::from("/cache"),
            sidecars: self.sidecars.clone(),
            pixels: self.pixels.clone(),
            analyzers: Vec::new(),
            inference: Arc::new(FakeInference::new(&[])),
            models: Vec::new(),
            model_store: Arc::new(MemoryModelStore::default()),
            pipeline_store: self.pipeline.clone(),
            watcher: self.watcher.clone(),
        }
    }
}

/// Import sources, jobs and log kept in memory.
#[derive(Default)]
pub struct MemoryPipelineStore {
    state: Mutex<PipelineState>,
}

#[derive(Default)]
struct PipelineState {
    sources: Vec<crate::ports::pipeline::ImportSource>,
    jobs: Vec<(String, crate::ports::pipeline::Job)>,
    log: Vec<crate::ports::pipeline::LogEntry>,
    next_id: i64,
}

impl PipelineState {
    fn id(&mut self) -> i64 {
        self.next_id += 1;
        self.next_id
    }
}

impl MemoryPipelineStore {
    /// Every log message, oldest first.
    pub fn messages(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .log
            .iter()
            .map(|e| e.message.clone())
            .collect()
    }
}

impl crate::ports::pipeline::PipelineStore for MemoryPipelineStore {
    fn add_source(
        &self,
        path: &Path,
        watch: bool,
    ) -> Result<crate::ports::pipeline::ImportSource, CatalogError> {
        let mut s = self.state.lock().unwrap();
        if let Some(existing) = s.sources.iter().find(|x| x.path == path) {
            return Ok(existing.clone());
        }
        let id = s.id();
        let source = crate::ports::pipeline::ImportSource {
            id,
            path: path.to_path_buf(),
            watch,
            scanned_at: None,
            scan_error: None,
        };
        s.sources.push(source.clone());
        Ok(source)
    }

    fn sources(&self) -> Result<Vec<crate::ports::pipeline::ImportSource>, CatalogError> {
        Ok(self.state.lock().unwrap().sources.clone())
    }

    fn set_source_watch(&self, id: i64, watch: bool) -> Result<(), CatalogError> {
        let mut s = self.state.lock().unwrap();
        if let Some(source) = s.sources.iter_mut().find(|x| x.id == id) {
            source.watch = watch;
        }
        Ok(())
    }

    fn remove_source(&self, id: i64) -> Result<(), CatalogError> {
        let mut s = self.state.lock().unwrap();
        s.sources.retain(|x| x.id != id);
        s.jobs
            .retain(|(_, j)| j.source_id != Some(id) || j.stage.is_final());
        for (_, j) in &mut s.jobs {
            if j.source_id == Some(id) {
                j.source_id = None;
            }
        }
        Ok(())
    }

    fn record_scan(&self, id: i64, error: Option<&str>) -> Result<(), CatalogError> {
        let mut s = self.state.lock().unwrap();
        if let Some(source) = s.sources.iter_mut().find(|x| x.id == id) {
            source.scanned_at = Some("2026-10-14T10:00:00Z".into());
            source.scan_error = error.map(str::to_owned);
        }
        Ok(())
    }

    fn offer_job(
        &self,
        source_id: i64,
        key: &Path,
        files: &[crate::ports::pipeline::JobFile],
    ) -> Result<Option<i64>, CatalogError> {
        use crate::ports::pipeline::{Job, JobStage};
        let mut s = self.state.lock().unwrap();
        let key = key.display().to_string();
        if let Some((_, job)) = s.jobs.iter_mut().find(|(k, _)| *k == key) {
            if job.files == files || matches!(job.stage, JobStage::Reading | JobStage::Copying) {
                return Ok(None);
            }
            job.files = files.to_vec();
            if job.stage == JobStage::Queued {
                return Ok(None);
            }
            job.stage = JobStage::Queued;
            job.error = None;
            job.source_id = Some(source_id);
            return Ok(Some(job.id));
        }
        let id = s.id();
        s.jobs.push((
            key,
            Job {
                id,
                source_id: Some(source_id),
                files: files.to_vec(),
                stage: JobStage::Queued,
                photo_id: None,
                error: None,
                updated_at: "2026-10-14T10:00:00Z".into(),
                failed_stage: None,
            },
        ));
        Ok(Some(id))
    }

    fn next_job(&self) -> Result<Option<crate::ports::pipeline::Job>, CatalogError> {
        let s = self.state.lock().unwrap();
        Ok(s.jobs
            .iter()
            .map(|(_, j)| j)
            .filter(|j| j.stage == crate::ports::pipeline::JobStage::Queued)
            .min_by_key(|j| j.id)
            .cloned())
    }

    fn set_job_stage(
        &self,
        id: i64,
        stage: crate::ports::pipeline::JobStage,
        photo_id: Option<PhotoId>,
        error: Option<&str>,
    ) -> Result<(), CatalogError> {
        use crate::ports::pipeline::JobStage;
        let mut s = self.state.lock().unwrap();
        if let Some((_, job)) = s.jobs.iter_mut().find(|(_, j)| j.id == id) {
            job.failed_stage = (stage == JobStage::Failed).then_some(job.stage);
            job.stage = stage;
            job.photo_id = photo_id.or(job.photo_id);
            job.error = error.map(str::to_owned);
        }
        Ok(())
    }

    fn requeue_unfinished(&self) -> Result<usize, CatalogError> {
        use crate::ports::pipeline::JobStage;
        let mut s = self.state.lock().unwrap();
        let mut n = 0;
        for (_, job) in &mut s.jobs {
            if matches!(job.stage, JobStage::Reading | JobStage::Copying) {
                job.stage = JobStage::Queued;
                n += 1;
            }
        }
        Ok(n)
    }

    fn retry_failed(&self, ids: Option<&[i64]>) -> Result<usize, CatalogError> {
        use crate::ports::pipeline::JobStage;
        let mut s = self.state.lock().unwrap();
        let mut n = 0;
        for (_, job) in &mut s.jobs {
            if job.stage == JobStage::Failed && ids.is_none_or(|ids| ids.contains(&job.id)) {
                job.stage = JobStage::Queued;
                job.error = None;
                n += 1;
            }
        }
        Ok(n)
    }

    fn job(&self, id: i64) -> Result<Option<crate::ports::pipeline::Job>, CatalogError> {
        let s = self.state.lock().unwrap();
        Ok(s.jobs.iter().map(|(_, j)| j).find(|j| j.id == id).cloned())
    }

    fn jobs(
        &self,
        stages: Option<&[crate::ports::pipeline::JobStage]>,
        limit: usize,
        before_id: Option<i64>,
    ) -> Result<Vec<crate::ports::pipeline::Job>, CatalogError> {
        let s = self.state.lock().unwrap();
        let mut jobs: Vec<_> = s
            .jobs
            .iter()
            .map(|(_, j)| j)
            .filter(|j| stages.is_none_or(|st| st.contains(&j.stage)))
            .filter(|j| before_id.is_none_or(|b| j.id < b))
            .cloned()
            .collect();
        jobs.sort_by_key(|j| std::cmp::Reverse(j.id));
        jobs.truncate(limit);
        Ok(jobs)
    }

    fn job_counts(&self) -> Result<Vec<(crate::ports::pipeline::JobStage, usize)>, CatalogError> {
        let s = self.state.lock().unwrap();
        let mut counts: HashMap<crate::ports::pipeline::JobStage, usize> = HashMap::new();
        for (_, job) in &s.jobs {
            *counts.entry(job.stage).or_default() += 1;
        }
        Ok(counts.into_iter().collect())
    }

    fn append_log(
        &self,
        level: crate::ports::pipeline::LogLevel,
        job_id: Option<i64>,
        photo_id: Option<PhotoId>,
        message: &str,
    ) -> Result<(), CatalogError> {
        let mut s = self.state.lock().unwrap();
        let id = s.id();
        s.log.push(crate::ports::pipeline::LogEntry {
            id,
            at: "2026-10-14T10:00:00Z".into(),
            level,
            job_id,
            photo_id: photo_id.map(|p| p.0),
            message: message.to_owned(),
        });
        Ok(())
    }

    fn log(
        &self,
        min_level: crate::ports::pipeline::LogLevel,
        limit: usize,
        before_id: Option<i64>,
    ) -> Result<Vec<crate::ports::pipeline::LogEntry>, CatalogError> {
        let rank = |l: crate::ports::pipeline::LogLevel| l as u8;
        let s = self.state.lock().unwrap();
        Ok(s.log
            .iter()
            .rev()
            .filter(|e| rank(e.level) >= rank(min_level))
            .filter(|e| before_id.is_none_or(|b| e.id < b))
            .take(limit)
            .cloned()
            .collect())
    }

    fn prune_log(&self, keep: usize) -> Result<(), CatalogError> {
        let mut s = self.state.lock().unwrap();
        let len = s.log.len();
        if len > keep {
            s.log.drain(..len - keep);
        }
        Ok(())
    }
}
