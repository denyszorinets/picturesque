//! Turning GPS coordinates into place names (ADR-0007).

use picturesque_core::metadata::{GeoPoint, Place};

/// Finds the place a point is in or near. `None` when nothing is close
/// enough to name.
pub trait ReverseGeocoder: Send + Sync {
    fn place(&self, point: GeoPoint) -> Option<Place>;
}
