//! Finishing a folder (`docs/features/folders.rst`): auto-picked photos
//! get the auto-pick colour label in their sidecars, and every pending
//! sidecar is written, so other tools see the result of the cull.

use std::collections::HashSet;
use std::sync::Arc;

use picturesque_core::decision::Flag;
use picturesque_core::photo::PhotoId;
use serde::Serialize;
use ts_rs::TS;

use crate::library::{MAX_PAGE, PhotoQuery};
use crate::ports::catalog::{CatalogError, PhotoCatalog};
use crate::sidecars::SidecarService;

/// What finishing would do (`finish_status`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct FinishStatus {
    /// Photos auto-pick suggests keeping, without a flag of their own.
    pub auto_picked: usize,
    /// Photos whose auto-pick label is not in their sidecars yet (or
    /// should be removed).
    pub labels_to_write: usize,
    pub picks: usize,
    pub rejects: usize,
}

/// What finishing did (`finish_folder`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct FinishResult {
    /// Photos now carrying the auto-pick label.
    pub auto_labeled: usize,
    /// Sidecars written now.
    pub written: usize,
    /// Photos whose sidecar could not be written, and why the first failed.
    pub failed: usize,
    pub error: Option<String>,
}

pub struct FinishService {
    catalog: Arc<dyn PhotoCatalog>,
    sidecars: Arc<SidecarService>,
}

impl std::fmt::Debug for FinishService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FinishService").finish_non_exhaustive()
    }
}

impl FinishService {
    pub fn new(catalog: Arc<dyn PhotoCatalog>, sidecars: Arc<SidecarService>) -> Self {
        Self { catalog, sidecars }
    }

    /// Every photo auto-pick suggests keeping now.
    fn auto_picked(&self) -> Result<HashSet<PhotoId>, CatalogError> {
        let mut ids = HashSet::new();
        let mut cursor = None;
        loop {
            let page = self.catalog.query(&PhotoQuery {
                auto_picked: Some(true),
                limit: Some(MAX_PAGE),
                cursor,
                ..Default::default()
            })?;
            ids.extend(
                page.photos
                    .iter()
                    .filter(|p| p.keeper && p.flag == Flag::Unflagged)
                    .map(|p| PhotoId(p.id)),
            );
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => return Ok(ids),
            }
        }
    }

    fn count(&self, flag: Flag) -> Result<usize, CatalogError> {
        Ok(self
            .catalog
            .query(&PhotoQuery {
                flags: Some(vec![flag]),
                limit: Some(1),
                ..Default::default()
            })?
            .total)
    }

    pub fn status(&self) -> Result<FinishStatus, CatalogError> {
        let auto = self.auto_picked()?;
        let labeled = self.catalog.auto_labeled()?;
        Ok(FinishStatus {
            auto_picked: auto.len(),
            labels_to_write: auto.symmetric_difference(&labeled).count(),
            picks: self.count(Flag::Pick)?,
            rejects: self.count(Flag::Reject)?,
        })
    }

    /// Marks the auto-picked photos (and only them) for the auto-pick
    /// label, and writes every pending sidecar now.
    pub fn finish(&self) -> Result<FinishResult, CatalogError> {
        let mut auto: Vec<PhotoId> = self.auto_picked()?.into_iter().collect();
        auto.sort();
        self.catalog.set_auto_labeled(&auto)?;
        let report = self.sidecars.flush();
        Ok(FinishResult {
            auto_labeled: auto.len(),
            written: report.written,
            failed: report.failed.len(),
            error: report.failed.first().map(|(_, reason)| reason.clone()),
        })
    }
}
