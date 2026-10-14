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

/// The watched folder `path` is in (the deepest, if they nest).
fn folder_of<'a>(folders: &'a [PathBuf], path: &Path) -> Option<&'a PathBuf> {
    folders
        .iter()
        .filter(|f| path.starts_with(f))
        .max_by_key(|f| f.as_os_str().len())
}

impl FolderWatcher for NotifyWatcher {
    fn watch(&self, folders: &[PathBuf], changed: ChangeHandler) -> Vec<(PathBuf, String)> {
        let mut current = self.current.lock().expect("watcher lock");
        // Dropping the old watcher stops it.
        *current = None;
        if folders.is_empty() {
            return Vec::new();
        }
        let roots: Arc<[PathBuf]> = folders.into();
        let handler_roots = Arc::clone(&roots);
        let handler = move |result: notify::Result<Event>| {
            let Ok(event) = result else {
                // Events were lost (for example an overflow): rescan all.
                for root in handler_roots.iter() {
                    changed(root);
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
            .filter_map(|folder| {
                watcher
                    .watch(folder, RecursiveMode::Recursive)
                    .err()
                    .map(|e| (folder.clone(), e.to_string()))
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
        let folders = [
            PathBuf::from("/a"),
            PathBuf::from("/a/b"),
            PathBuf::from("/c"),
        ];
        assert_eq!(
            folder_of(&folders, Path::new("/a/b/x.jpg")),
            Some(&folders[1])
        );
        assert_eq!(
            folder_of(&folders, Path::new("/a/x.jpg")),
            Some(&folders[0])
        );
        assert_eq!(folder_of(&folders, Path::new("/ab/x.jpg")), None);
    }
}
