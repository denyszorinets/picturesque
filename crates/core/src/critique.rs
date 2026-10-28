//! Turning critique ratings into a score (`docs/features/ai.rst`).
//!
//! Vision-language models are not calibrated: one rated an ordinary photo
//! 8 and a featured one 7, and most photos land on 7 or 8
//! (`spikes/llama/results.md`). Ratings are therefore compared within the
//! library: a photo's critique score is the share of critiqued photos
//! rated below it (ties count half), so 0.9 means "better than 90% of
//! your critiqued photos". With few critiques a percentile means little,
//! and the rating is mapped linearly instead.

use std::collections::HashMap;
use std::hash::Hash;

/// Fewest critiques for percentiles; below, ratings map linearly.
pub const MIN_FOR_PERCENTILES: usize = 20;

/// Each item's critique score, 0–1, from its rating (1–10).
pub fn scores<K: Copy + Eq + Hash>(ratings: &[(K, f32)]) -> HashMap<K, f32> {
    if ratings.len() < MIN_FOR_PERCENTILES {
        return ratings
            .iter()
            .map(|(k, r)| (*k, ((r - 1.0) / 9.0).clamp(0.0, 1.0)))
            .collect();
    }
    let mut sorted: Vec<f32> = ratings.iter().map(|(_, r)| *r).collect();
    sorted.sort_by(f32::total_cmp);
    let n = sorted.len() as f32;
    ratings
        .iter()
        .map(|(k, r)| {
            let below = sorted.partition_point(|x| x < r);
            let equal = sorted.partition_point(|x| x <= r) - below;
            (*k, (below as f32 + equal as f32 / 2.0) / n)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn few_ratings_map_linearly() {
        let s = scores(&[(1, 1.0), (2, 5.5), (3, 10.0), (4, 12.0)]);
        assert_eq!(s[&1], 0.0);
        assert_eq!(s[&2], 0.5);
        assert_eq!(s[&3], 1.0);
        assert_eq!(s[&4], 1.0);
    }

    #[test]
    fn many_ratings_become_percentiles() {
        // A model that rates almost everything 7 or 8.
        let ratings: Vec<(usize, f32)> = (0..40)
            .map(|i| {
                (
                    i,
                    if i < 10 {
                        7.0
                    } else if i < 39 {
                        8.0
                    } else {
                        9.0
                    },
                )
            })
            .collect();
        let s = scores(&ratings);
        // Ten 7s of forty: below none, ties count half.
        assert!((s[&0] - 0.125).abs() < 1e-6);
        // Twenty-nine 8s: above the 7s, ties count half.
        assert!((s[&10] - (10.0 + 14.5) / 40.0).abs() < 1e-6);
        // The single 9 is above everything else.
        assert!((s[&39] - (39.0 + 0.5) / 40.0).abs() < 1e-6);
    }
}
