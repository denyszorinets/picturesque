//! Persistence of import runs, so imports can be reviewed and resumed.

use std::path::{Path, PathBuf};

use picturesque_core::hash::ContentHash;
use picturesque_core::photo::PhotoId;

use crate::import::model::{
    FileStatus, ImportProblem, ImportRun, ImportState, PhotoStatus, PlannedPhoto,
};
use crate::ports::catalog::CatalogError;

pub trait ImportStore: Send + Sync {
    /// Creates a run in the `planning` state and returns its id.
    fn create_run(
        &self,
        library_root: &Path,
        template: &str,
        sources: &[PathBuf],
    ) -> Result<i64, CatalogError>;

    /// Stores the plan, assigning row ids to `photos` and their files, and
    /// moves the run to `planned`.
    fn save_plan(
        &self,
        run_id: i64,
        photos: &mut [PlannedPhoto],
        problems: &[ImportProblem],
    ) -> Result<(), CatalogError>;

    fn load_run(&self, run_id: i64) -> Result<Option<ImportRun>, CatalogError>;

    fn set_state(
        &self,
        run_id: i64,
        state: ImportState,
        error: Option<&str>,
    ) -> Result<(), CatalogError>;

    fn update_file(
        &self,
        file_id: i64,
        status: FileStatus,
        hash: Option<&ContentHash>,
        error: Option<&str>,
    ) -> Result<(), CatalogError>;

    fn update_photo(
        &self,
        import_photo_id: i64,
        status: PhotoStatus,
        photo_id: Option<PhotoId>,
        reason: Option<&str>,
    ) -> Result<(), CatalogError>;

    /// Marks runs left `planning` or `copying` (the app stopped) as
    /// `interrupted`. Called at startup.
    fn mark_unfinished_interrupted(&self) -> Result<(), CatalogError>;
}
