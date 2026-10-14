//! [`FolderWatcher`] with the `notify` crate: inotify on Linux, FSEvents on
//! macOS, ReadDirectoryChangesW on Windows.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use notify::event::EventKind;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use picturesque_app::ports::watcher::{ChangeHandler, FolderWatcher};

#[derive(Default)]
pub struct NotifyWatcher {
    current: Mutex<Option<RecommendedWatcher>>,
}

impl std::fmt::Debug for NotifyWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotifyWatcher").finish_non_exhaustive()
    }
}

impl NotifyWatcher {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A watched folder as the user gave it, and with symbolic links resolved
/// (FSEvents reports `/private/var/...` for a folder under `/var`).
struct Root {
    given: PathBuf,
    resolved: Option<PathBuf>,
}

impl Root {
    fn new(given: &Path) -> Self {
        let resolved = std::fs::canonicalize(given)
            .ok()
            .filter(|r| r.as_path() != given);
        Self {
            given: given.to_path_buf(),
            resolved,
        }
    }

    /// How deep `path` is inside this folder, if it is.
    fn depth_of(&self, path: &Path) -> Option<usize> {
        [Some(&self.given), self.resolved.as_ref()]
            .into_iter()
            .flatten()
            .filter(|f| path.starts_with(f))
            .map(|f| f.as_os_str().len())
            .max()
    }
}

/// The watched folder `path` is in (the deepest, if they nest).
fn folder_of<'a>(folders: &'a [Root], path: &Path) -> Option<&'a PathBuf> {
    folders
        .iter()
        .filter_map(|f| f.depth_of(path).map(|depth| (depth, &f.given)))
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, given)| given)
}

impl FolderWatcher for NotifyWatcher {
    fn watch(&self, folders: &[PathBuf], changed: ChangeHandler) -> Vec<(PathBuf, String)> {
        let mut current = self.current.lock().expect("watcher lock");
        // Dropping the old watcher stops it.
        *current = None;
        if folders.is_empty() {
            return Vec::new();
        }
        let roots: Arc<[Root]> = folders.iter().map(|f| Root::new(f)).collect();
        let handler_roots = Arc::clone(&roots);
        let handler = move |result: notify::Result<Event>| {
            let Ok(event) = result else {
                // Events were lost (for example an overflow): rescan all.
                for root in handler_roots.iter() {
                    changed(&root.given);
                }
                return;
            };
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            let mut seen: Vec<&PathBuf> = Vec::new();
            for path in &event.paths {
                if let Some(root) = folder_of(&handler_roots, path)
                    && !seen.contains(&root)
                {
                    seen.push(root);
                    changed(root);
                }
            }
        };
        let mut watcher = match notify::recommended_watcher(handler) {
            Ok(w) => w,
            Err(e) => {
                return folders.iter().map(|f| (f.clone(), e.to_string())).collect();
            }
        };
        let failed = roots
            .iter()
            .filter_map(|root| {
                watcher
                    .watch(&root.given, RecursiveMode::Recursive)
                    .err()
                    .map(|e| (root.given.clone(), e.to_string()))
            })
            .collect();
        *current = Some(watcher);
        failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_map_to_the_deepest_watched_folder() {
        let folders = ["/a", "/a/b", "/c"].map(|f| Root::new(Path::new(f)));
        assert_eq!(
            folder_of(&folders, Path::new("/a/b/x.jpg")),
            Some(&PathBuf::from("/a/b"))
        );
        assert_eq!(
            folder_of(&folders, Path::new("/a/x.jpg")),
            Some(&PathBuf::from("/a"))
        );
        assert_eq!(folder_of(&folders, Path::new("/ab/x.jpg")), None);
    }

    #[cfg(unix)]
    #[test]
    fn events_under_the_resolved_path_map_to_the_folder_as_given() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().canonicalize().unwrap().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let folders = [Root::new(&link)];
        assert_eq!(folder_of(&folders, &real.join("x.jpg")), Some(&link));
        assert_eq!(folder_of(&folders, &link.join("x.jpg")), Some(&link));
    }
}
