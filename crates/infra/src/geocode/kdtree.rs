//! A static 3-D k-d tree over points on the unit sphere.
//!
//! Coordinates are converted to unit vectors, so nearest-by-chord is
//! nearest-by-great-circle, with no special cases at the poles or the
//! date line.

/// A point on the unit sphere.
pub type Vec3 = [f64; 3];

/// Unit vector for a latitude/longitude in degrees.
pub fn to_unit(latitude: f64, longitude: f64) -> Vec3 {
    let (lat, lon) = (latitude.to_radians(), longitude.to_radians());
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn dist2(a: &Vec3, b: &Vec3) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Mean Earth radius in kilometres.
pub const EARTH_RADIUS_KM: f64 = 6371.0;

/// Great-circle distance in km for a squared chord length on the unit sphere.
pub fn chord2_to_km(chord2: f64) -> f64 {
    2.0 * (chord2.sqrt() / 2.0).min(1.0).asin() * EARTH_RADIUS_KM
}

/// Balanced k-d tree stored implicitly: the median of each range is its
/// node, split on axis `depth % 3`.
#[derive(Debug)]
pub struct KdTree {
    /// `(point, payload index)`, arranged so each range's median is a node.
    nodes: Vec<(Vec3, u32)>,
}

impl KdTree {
    pub fn build(points: impl IntoIterator<Item = Vec3>) -> Self {
        let mut nodes: Vec<(Vec3, u32)> = points
            .into_iter()
            .enumerate()
            .map(|(i, p)| (p, i as u32))
            .collect();
        arrange(&mut nodes, 0);
        Self { nodes }
    }

    /// All payload indices within squared chord distance `max_chord2` of
    /// `target`, with their squared chord distances (unordered).
    pub fn within(&self, target: &Vec3, max_chord2: f64) -> Vec<(u32, f64)> {
        let mut found = Vec::new();
        collect_within(&self.nodes, target, max_chord2, 0, &mut found);
        found
    }

    /// The payload index nearest to `target` and the squared chord distance.
    pub fn nearest(&self, target: &Vec3) -> Option<(u32, f64)> {
        let mut best = None;
        search(&self.nodes, target, 0, &mut best);
        best
    }
}

fn arrange(nodes: &mut [(Vec3, u32)], depth: usize) {
    if nodes.len() <= 1 {
        return;
    }
    let axis = depth % 3;
    let mid = nodes.len() / 2;
    nodes.select_nth_unstable_by(mid, |a, b| a.0[axis].total_cmp(&b.0[axis]));
    let (left, right) = nodes.split_at_mut(mid);
    arrange(left, depth + 1);
    arrange(&mut right[1..], depth + 1);
}

fn search(nodes: &[(Vec3, u32)], target: &Vec3, depth: usize, best: &mut Option<(u32, f64)>) {
    if nodes.is_empty() {
        return;
    }
    let mid = nodes.len() / 2;
    let (point, index) = &nodes[mid];
    let d = dist2(point, target);
    if best.is_none_or(|(_, b)| d < b) {
        *best = Some((*index, d));
    }
    let axis = depth % 3;
    let delta = target[axis] - point[axis];
    let (near, far) = if delta < 0.0 {
        (&nodes[..mid], &nodes[mid + 1..])
    } else {
        (&nodes[mid + 1..], &nodes[..mid])
    };
    search(near, target, depth + 1, best);
    if best.is_none_or(|(_, b)| delta * delta < b) {
        search(far, target, depth + 1, best);
    }
}

fn collect_within(
    nodes: &[(Vec3, u32)],
    target: &Vec3,
    max_chord2: f64,
    depth: usize,
    found: &mut Vec<(u32, f64)>,
) {
    if nodes.is_empty() {
        return;
    }
    let mid = nodes.len() / 2;
    let (point, index) = &nodes[mid];
    let d = dist2(point, target);
    if d <= max_chord2 {
        found.push((*index, d));
    }
    let delta = target[depth % 3] - point[depth % 3];
    if delta <= 0.0 || delta * delta <= max_chord2 {
        collect_within(&nodes[..mid], target, max_chord2, depth + 1, found);
    }
    if delta >= 0.0 || delta * delta <= max_chord2 {
        collect_within(&nodes[mid + 1..], target, max_chord2, depth + 1, found);
    }
}

/// Squared chord length on the unit sphere for a great-circle distance in km.
pub fn km_to_chord2(km: f64) -> f64 {
    let half_angle = (km / EARTH_RADIUS_KM / 2.0).min(std::f64::consts::FRAC_PI_2);
    (2.0 * half_angle.sin()).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_same_point_as_brute_force() {
        // Deterministic pseudo-random points over the whole sphere.
        let mut seed = 0x2545_f491_u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 1_000_000) as f64 / 1_000_000.0
        };
        let points: Vec<Vec3> = (0..2000)
            .map(|_| to_unit(next() * 180.0 - 90.0, next() * 360.0 - 180.0))
            .collect();
        let tree = KdTree::build(points.clone());

        for _ in 0..500 {
            let q = to_unit(next() * 180.0 - 90.0, next() * 360.0 - 180.0);
            let brute = points
                .iter()
                .enumerate()
                .map(|(i, p)| (i as u32, dist2(p, &q)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            assert_eq!(tree.nearest(&q).unwrap().0, brute.0);
        }
    }

    #[test]
    fn within_matches_brute_force() {
        let points: Vec<Vec3> = (0..400)
            .map(|i| to_unit(38.0 + (i % 20) as f64 * 0.05, -9.5 + (i / 20) as f64 * 0.05))
            .collect();
        let tree = KdTree::build(points.clone());
        let q = to_unit(38.5, -9.0);
        let r = km_to_chord2(10.0);

        let mut got: Vec<u32> = tree.within(&q, r).into_iter().map(|(i, _)| i).collect();
        got.sort_unstable();
        let want: Vec<u32> = (0..points.len() as u32)
            .filter(|&i| dist2(&points[i as usize], &q) <= r)
            .collect();

        assert!(!want.is_empty());
        assert_eq!(got, want);
    }

    #[test]
    fn km_and_chord_conversions_are_inverse() {
        for km in [0.5, 10.0, 50.0, 1000.0] {
            assert!((chord2_to_km(km_to_chord2(km)) - km).abs() < 1e-6);
        }
    }

    #[test]
    fn nearest_across_the_date_line() {
        let tree = KdTree::build([to_unit(-18.14, 178.44), to_unit(-18.0, 170.0)]);

        // Just east of the date line is closest to the point just west of it.
        assert_eq!(tree.nearest(&to_unit(-18.1, -179.9)).unwrap().0, 0);
    }

    #[test]
    fn converts_chords_to_kilometres() {
        let lisbon = to_unit(38.7223, -9.1393);
        let porto = to_unit(41.1579, -8.6291);
        let km = chord2_to_km(dist2(&lisbon, &porto));

        assert!((km - 274.0).abs() < 3.0, "{km}");
    }

    #[test]
    fn empty_tree_has_no_nearest() {
        let tree = KdTree::build(std::iter::empty());

        assert_eq!(tree.nearest(&to_unit(0.0, 0.0)), None);
    }
}
