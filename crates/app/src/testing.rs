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
        self.state.lock().unwrap().files.remove(path);
        Ok(())
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

// --- Catalog, import store, events -------------------------------------

use std::collections::HashSet as StdHashSet;
use std::sync::Arc;

use picturesque_core::photo::{NewPhoto, Photo, PhotoId};

use crate::api::AppContext;
use crate::import::model::{
    FileStatus, ImportProblem, ImportRun, ImportState, PhotoStatus, PlannedPhoto,
};
use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::ports::events::{AppEvent, EventSink};
use crate::ports::imports::ImportStore;
use crate::settings::tests::MemorySettings;

/// Photos kept in memory, with the catalog's one-copy rule for media.
#[derive(Default)]
pub struct MemoryCatalog {
    pub photos: Mutex<Vec<Photo>>,
}

impl PhotoCatalog for MemoryCatalog {
    fn insert_photo(&self, photo: &NewPhoto) -> Result<PhotoId, CatalogError> {
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

/// Import runs kept in memory.
#[derive(Default)]
pub struct MemoryImports {
    runs: Mutex<BTreeMap<i64, ImportRun>>,
    next_id: Mutex<i64>,
}

impl MemoryImports {
    fn id(&self) -> i64 {
        let mut n = self.next_id.lock().unwrap();
        *n += 1;
        *n
    }
}

impl ImportStore for MemoryImports {
    fn create_run(
        &self,
        library_root: &Path,
        template: &str,
        sources: &[PathBuf],
    ) -> Result<i64, CatalogError> {
        let id = self.id();
        self.runs.lock().unwrap().insert(
            id,
            ImportRun {
                id,
                state: ImportState::Planning,
                library_root: library_root.to_path_buf(),
                template: template.to_owned(),
                sources: sources.to_vec(),
                photos: Vec::new(),
                problems: Vec::new(),
                error: None,
            },
        );
        Ok(id)
    }

    fn save_plan(
        &self,
        run_id: i64,
        photos: &mut [PlannedPhoto],
        problems: &[ImportProblem],
    ) -> Result<(), CatalogError> {
        for photo in photos.iter_mut() {
            photo.id = Some(self.id());
            for file in &mut photo.files {
                file.id = Some(self.id());
            }
        }
        let mut runs = self.runs.lock().unwrap();
        let run = runs
            .get_mut(&run_id)
            .ok_or_else(|| CatalogError::Storage("no run".into()))?;
        run.photos = photos.to_vec();
        run.problems = problems.to_vec();
        run.state = ImportState::Planned;
        Ok(())
    }

    fn load_run(&self, run_id: i64) -> Result<Option<ImportRun>, CatalogError> {
        Ok(self.runs.lock().unwrap().get(&run_id).cloned())
    }

    fn set_state(
        &self,
        run_id: i64,
        state: ImportState,
        error: Option<&str>,
    ) -> Result<(), CatalogError> {
        let mut runs = self.runs.lock().unwrap();
        let run = runs
            .get_mut(&run_id)
            .ok_or_else(|| CatalogError::Storage("no run".into()))?;
        run.state = state;
        run.error = error.map(str::to_owned);
        Ok(())
    }

    fn update_file(
        &self,
        file_id: i64,
        status: FileStatus,
        hash: Option<&ContentHash>,
        error: Option<&str>,
    ) -> Result<(), CatalogError> {
        let mut runs = self.runs.lock().unwrap();
        let file = runs
            .values_mut()
            .flat_map(|r| r.photos.iter_mut())
            .flat_map(|p| p.files.iter_mut())
            .find(|f| f.id == Some(file_id))
            .ok_or_else(|| CatalogError::Storage("no file".into()))?;
        file.status = status;
        if let Some(h) = hash {
            file.hash = Some(*h);
        }
        file.error = error.map(str::to_owned);
        Ok(())
    }

    fn update_photo(
        &self,
        import_photo_id: i64,
        status: PhotoStatus,
        photo_id: Option<PhotoId>,
        reason: Option<&str>,
    ) -> Result<(), CatalogError> {
        let mut runs = self.runs.lock().unwrap();
        let photo = runs
            .values_mut()
            .flat_map(|r| r.photos.iter_mut())
            .find(|p| p.id == Some(import_photo_id))
            .ok_or_else(|| CatalogError::Storage("no photo".into()))?;
        photo.status = status;
        photo.photo_id = photo_id;
        photo.reason = reason.map(str::to_owned);
        Ok(())
    }

    fn mark_unfinished_interrupted(&self) -> Result<(), CatalogError> {
        for run in self.runs.lock().unwrap().values_mut() {
            if matches!(run.state, ImportState::Planning | ImportState::Copying) {
                run.state = ImportState::Interrupted;
            }
        }
        Ok(())
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

/// Everything a test needs, with handles to inspect afterwards.
pub struct TestWorld {
    pub fs: Arc<MemFs>,
    pub settings: Arc<MemorySettings>,
    pub catalog: Arc<MemoryCatalog>,
    pub imports: Arc<MemoryImports>,
    pub metadata: Arc<FakeMetadata>,
    pub events: Arc<RecordedEvents>,
    pub decoder: Arc<FakeDecoder>,
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
            fs,
            settings: Arc::default(),
            catalog: Arc::default(),
            imports: Arc::default(),
            metadata: Arc::new(metadata),
            events: Arc::default(),
        }
    }

    pub fn context(&self) -> AppContext {
        AppContext {
            settings_store: self.settings.clone(),
            catalog: self.catalog.clone(),
            imports: self.imports.clone(),
            fs: self.fs.clone(),
            metadata: self.metadata.clone(),
            geocoder: Arc::new(FakeGeocoder),
            events: self.events.clone(),
            decoder: self.decoder.clone(),
            cache_dir: PathBuf::from("/cache"),
        }
    }
}
