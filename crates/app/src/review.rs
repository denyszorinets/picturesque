//! Explicit review of a frozen, filtered set. No image or sidecar writes.
use crate::library::{MAX_PAGE, PhotoQuery};
use crate::ports::catalog::{CatalogError, PhotoCatalog};
use picturesque_core::photo::PhotoId;
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use ts_rs::TS;

static NEXT_PLAN: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ReviewPlan {
    pub token: String,
    pub count: usize,
    pub first_day: Option<String>,
    pub last_day: Option<String>,
    pub undated: usize,
}

#[derive(Debug, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ApplyReviewArgs {
    pub token: String,
    pub reviewed: bool,
}

#[derive(Debug)]
struct Pending {
    plan: ReviewPlan,
    ids: Vec<PhotoId>,
}

pub struct ReviewService {
    catalog: Arc<dyn PhotoCatalog>,
    pending: Mutex<Option<Pending>>,
}

impl std::fmt::Debug for ReviewService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReviewService").finish_non_exhaustive()
    }
}

impl ReviewService {
    pub fn new(catalog: Arc<dyn PhotoCatalog>) -> Self {
        Self {
            catalog,
            pending: Mutex::new(None),
        }
    }

    pub fn plan(&self, mut query: PhotoQuery) -> Result<ReviewPlan, CatalogError> {
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        *pending = None;
        query.cursor = None;
        query.limit = Some(MAX_PAGE);
        let mut ids = std::collections::BTreeSet::new();
        let mut days = std::collections::BTreeSet::new();
        let mut undated = 0;
        loop {
            let page = self.catalog.query(&query)?;
            for photo in page.photos {
                if !ids.insert(PhotoId(photo.id)) {
                    continue;
                }
                match photo.day {
                    Some(day) => {
                        days.insert(day);
                    }
                    None => undated += 1,
                }
            }
            query.cursor = page.next_cursor;
            if query.cursor.is_none() {
                break;
            }
        }
        let plan = ReviewPlan {
            token: NEXT_PLAN.fetch_add(1, Ordering::Relaxed).to_string(),
            count: ids.len(),
            first_day: days.first().cloned(),
            last_day: days.last().cloned(),
            undated,
        };
        *pending = Some(Pending {
            plan: plan.clone(),
            ids: ids.into_iter().collect(),
        });
        Ok(plan)
    }

    pub fn apply(&self, args: ApplyReviewArgs) -> Result<usize, CatalogError> {
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        let Some(p) = pending.as_ref().filter(|p| p.plan.token == args.token) else {
            return Err(CatalogError::Storage(
                "Review preview expired; open it again".into(),
            ));
        };
        let changed = self.catalog.set_reviewed(&p.ids, args.reviewed)?;
        *pending = None;
        Ok(changed)
    }
}
