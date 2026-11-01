//! Learning the user's taste from their picks and rejects
//! (`docs/features/ai.rst`).
//!
//! A logistic regression on CLIP embeddings: picks are positive examples,
//! rejects negative. Both classes count equally however many of each there
//! are, and strong L2 regularization keeps a few dozen examples from being
//! memorized. Features are standardized for training and the scaling is
//! folded back into the weights, so scoring is one dot product.
//!
//! Raw outputs of a heavily regularized model bunch up around 0.5. They are
//! calibrated (Platt scaling) on out-of-fold predictions of a k-fold
//! cross-validation, which also measures how well the model agrees with the
//! user (AUC). Folds are contiguous blocks of the examples in the order
//! given; callers pass them in capture order so that near-duplicates of a
//! series mostly stay in one fold and do not inflate the measurement.

use crate::clip::Embedding;

/// Fewest picks and fewest rejects needed to learn anything.
pub const MIN_EACH: usize = 10;
/// Most examples of each class used; more only slow training down.
pub const MAX_EACH: usize = 1_000;

const FOLDS: usize = 5;
const ITERATIONS: usize = 200;
const LEARNING_RATE: f64 = 0.5;
/// L2 penalty on standardized weights.
const L2: f64 = 0.05;

/// A trained personal model.
#[derive(Debug, Clone, PartialEq)]
pub struct TasteModel {
    weights: Vec<f32>,
    bias: f32,
    /// Platt scaling of the raw logit: `sigmoid(scale * z + shift)`.
    scale: f32,
    shift: f32,
    /// Cross-validated chance that a pick outscores a reject.
    pub agreement: f32,
    pub picks: usize,
    pub rejects: usize,
}

impl TasteModel {
    /// How much the photo matches the user's taste, 0–1 (about the
    /// probability that they would pick rather than reject it). `None` for
    /// an embedding of another model.
    pub fn score(&self, embedding: &Embedding) -> Option<f32> {
        let z = self.logit(embedding.values())?;
        Some(sigmoid(f64::from(self.scale * z + self.shift)) as f32)
    }

    fn logit(&self, values: &[f32]) -> Option<f32> {
        (values.len() == self.weights.len()).then(|| {
            self.weights
                .iter()
                .zip(values)
                .map(|(w, v)| w * v)
                .sum::<f32>()
                + self.bias
        })
    }
}

/// Learns a model from `(embedding, picked)` examples, in capture order.
/// `None` with fewer than [`MIN_EACH`] picks or rejects, or embeddings of
/// different sizes.
pub fn train(examples: &[(&Embedding, bool)]) -> Option<TasteModel> {
    let examples = capped(examples);
    let picks = examples.iter().filter(|(_, p)| *p).count();
    let rejects = examples.len() - picks;
    let dims = examples.first()?.0.values().len();
    if picks < MIN_EACH
        || rejects < MIN_EACH
        || examples.iter().any(|(e, _)| e.values().len() != dims)
    {
        return None;
    }

    // Out-of-fold logits, for calibration and agreement.
    let mut held_out: Vec<(f64, bool)> = Vec::with_capacity(examples.len());
    for fold in 0..FOLDS {
        let range = fold_range(examples.len(), fold);
        let train: Vec<(&Embedding, bool)> = examples
            .iter()
            .enumerate()
            .filter(|(i, _)| !range.contains(i))
            .map(|(_, e)| *e)
            .collect();
        let Some(linear) = fit(&train) else {
            continue;
        };
        for (e, picked) in &examples[range] {
            held_out.push((f64::from(linear.logit(e.values())?), *picked));
        }
    }
    let (scale, shift) = calibrate(&held_out);
    let logits: Vec<(f32, bool)> = held_out.iter().map(|(z, p)| (*z as f32, *p)).collect();
    let mut model = fit(&examples)?;
    model.scale = scale as f32;
    model.shift = shift as f32;
    model.agreement = auc(&logits).unwrap_or(0.5);
    model.picks = picks;
    model.rejects = rejects;
    Some(model)
}

/// At most [`MAX_EACH`] of each class, spread evenly over the examples.
fn capped<'a>(examples: &[(&'a Embedding, bool)]) -> Vec<(&'a Embedding, bool)> {
    let keep = |class: bool| -> Vec<usize> {
        let idx: Vec<usize> = (0..examples.len())
            .filter(|i| examples[*i].1 == class)
            .collect();
        if idx.len() <= MAX_EACH {
            return idx;
        }
        (0..MAX_EACH)
            .map(|k| idx[k * idx.len() / MAX_EACH])
            .collect()
    };
    let mut kept = keep(true);
    kept.extend(keep(false));
    kept.sort_unstable();
    kept.into_iter().map(|i| examples[i]).collect()
}

fn fold_range(len: usize, fold: usize) -> std::ops::Range<usize> {
    len * fold / FOLDS..len * (fold + 1) / FOLDS
}

/// Class-balanced, L2-regularized logistic regression by gradient descent
/// on standardized features. `None` without both classes.
fn fit(examples: &[(&Embedding, bool)]) -> Option<TasteModel> {
    let picks = examples.iter().filter(|(_, p)| *p).count();
    let rejects = examples.len() - picks;
    if picks == 0 || rejects == 0 {
        return None;
    }
    let dims = examples[0].0.values().len();
    let n = examples.len() as f64;
    let mut mean = vec![0.0f64; dims];
    for (e, _) in examples {
        for (m, v) in mean.iter_mut().zip(e.values()) {
            *m += f64::from(*v) / n;
        }
    }
    let mut std = vec![0.0f64; dims];
    for (e, _) in examples {
        for ((s, m), v) in std.iter_mut().zip(&mean).zip(e.values()) {
            *s += (f64::from(*v) - m).powi(2) / n;
        }
    }
    for s in &mut std {
        *s = s.sqrt().max(1e-6);
    }
    let x: Vec<Vec<f64>> = examples
        .iter()
        .map(|(e, _)| {
            e.values()
                .iter()
                .zip(&mean)
                .zip(&std)
                .map(|((v, m), s)| (f64::from(*v) - m) / s)
                .collect()
        })
        .collect();
    // Each class carries half the total weight.
    let weight = |picked: bool| {
        if picked {
            0.5 / picks as f64
        } else {
            0.5 / rejects as f64
        }
    };

    let mut w = vec![0.0f64; dims];
    let mut b = 0.0f64;
    let mut grad = vec![0.0f64; dims];
    for _ in 0..ITERATIONS {
        grad.iter_mut().zip(&w).for_each(|(g, wi)| *g = L2 * wi);
        let mut grad_b = 0.0;
        for (row, (_, picked)) in x.iter().zip(examples) {
            let z: f64 = row.iter().zip(&w).map(|(a, c)| a * c).sum::<f64>() + b;
            let target = if *picked { 1.0 } else { 0.0 };
            let err = weight(*picked) * (sigmoid(z) - target);
            for (g, a) in grad.iter_mut().zip(row) {
                *g += err * a;
            }
            grad_b += err;
        }
        for (wi, g) in w.iter_mut().zip(&grad) {
            *wi -= LEARNING_RATE * g;
        }
        b -= LEARNING_RATE * grad_b;
    }

    // Fold the standardization into the weights.
    let weights: Vec<f32> = w.iter().zip(&std).map(|(wi, s)| (wi / s) as f32).collect();
    let bias = b - w
        .iter()
        .zip(&mean)
        .zip(&std)
        .map(|((wi, m), s)| wi * m / s)
        .sum::<f64>();
    Some(TasteModel {
        weights,
        bias: bias as f32,
        scale: 1.0,
        shift: 0.0,
        agreement: 0.5,
        picks,
        rejects,
    })
}

/// Platt scaling: the `(scale, shift)` that make `sigmoid(scale * z +
/// shift)` fit the labels, classes weighted equally. Falls back to the
/// identity without both classes.
fn calibrate(logits: &[(f64, bool)]) -> (f64, f64) {
    let picks = logits.iter().filter(|(_, p)| *p).count();
    let rejects = logits.len() - picks;
    if picks == 0 || rejects == 0 {
        return (1.0, 0.0);
    }
    let (mut a, mut c) = (1.0f64, 0.0f64);
    for _ in 0..500 {
        let (mut ga, mut gc) = (0.0, 0.0);
        for (z, picked) in logits {
            let weight = if *picked {
                0.5 / picks as f64
            } else {
                0.5 / rejects as f64
            };
            let err = weight * (sigmoid(a * z + c) - if *picked { 1.0 } else { 0.0 });
            ga += err * z;
            gc += err;
        }
        // A light penalty keeps the scale finite when classes separate
        // perfectly.
        ga += 1e-3 * (a - 1.0);
        a -= 1.0 * ga;
        c -= 1.0 * gc;
    }
    (a, c)
}

fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + (-z).exp())
}

/// The chance that a random positive outscores a random negative (ties
/// count half); `None` without both.
pub fn auc(scored: &[(f32, bool)]) -> Option<f32> {
    let mut sorted: Vec<(f32, bool)> = scored.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let positives = sorted.iter().filter(|(_, p)| *p).count();
    let negatives = sorted.len() - positives;
    if positives == 0 || negatives == 0 {
        return None;
    }
    // Sum of positive ranks (1-based, ties averaged).
    let mut rank_sum = 0.0f64;
    let mut i = 0;
    while i < sorted.len() {
        let mut j = i;
        while j < sorted.len() && sorted[j].0 == sorted[i].0 {
            j += 1;
        }
        let average = (i + 1 + j) as f64 / 2.0;
        rank_sum += average * sorted[i..j].iter().filter(|(_, p)| *p).count() as f64;
        i = j;
    }
    let p = positives as f64;
    Some(((rank_sum - p * (p + 1.0) / 2.0) / (p * negatives as f64)) as f32)
}

/// The aesthetic score used for ranking: the general score blended with
/// the personal one by `personal_weight` (0–1), or whichever exists.
pub fn blend(general: Option<f32>, personal: Option<f32>, personal_weight: f32) -> Option<f32> {
    match (general, personal) {
        (Some(g), Some(p)) => Some(g + (p - g) * personal_weight.clamp(0.0, 1.0)),
        (Some(g), None) => Some(g),
        (None, Some(p)) if personal_weight > 0.0 => Some(p),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic pseudo-random embedding leaning towards `+axis`
    /// when picked and `-axis` when rejected, by `signal`.
    fn example(i: usize, picked: bool, signal: f32) -> Embedding {
        let mut state = (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut raw: Vec<f32> = (0..64)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state % 2_000) as f32 / 1_000.0 - 1.0
            })
            .collect();
        raw[0] += if picked { signal } else { -signal };
        Embedding::new(&raw).unwrap()
    }

    /// Every `nth` example picked, the rest rejected, interleaved as
    /// decisions are over a shoot.
    fn set(len: usize, nth: usize, signal: f32) -> Vec<(Embedding, bool)> {
        (0..len)
            .map(|i| {
                (
                    example(i, i.is_multiple_of(nth), signal),
                    i.is_multiple_of(nth),
                )
            })
            .collect()
    }

    fn refs(examples: &[(Embedding, bool)]) -> Vec<(&Embedding, bool)> {
        examples.iter().map(|(e, p)| (e, *p)).collect()
    }

    #[test]
    fn needs_enough_picks_and_rejects() {
        let few: Vec<(Embedding, bool)> =
            (0..30).map(|i| (example(i, i < 9, 1.0), i < 9)).collect();
        assert_eq!(train(&refs(&few)), None);
        assert_eq!(train(&[]), None);
    }

    #[test]
    fn learns_a_preference_and_measures_it() {
        let examples = set(120, 3, 1.0);
        let model = train(&refs(&examples)).expect("enough examples");
        assert_eq!((model.picks, model.rejects), (40, 80));
        assert!(model.agreement > 0.9, "agreement {}", model.agreement);
        let liked = model.score(&example(1_000, true, 1.0)).unwrap();
        let disliked = model.score(&example(1_001, false, 1.0)).unwrap();
        assert!(liked > 0.65 && disliked < 0.35, "{liked} {disliked}");
    }

    #[test]
    fn noise_is_not_mistaken_for_taste() {
        let examples = set(120, 2, 0.0);
        let model = train(&refs(&examples)).expect("enough examples");
        assert!(
            (model.agreement - 0.5).abs() < 0.15,
            "agreement {}",
            model.agreement
        );
        // Calibration pulls scores of a model that learned nothing to the
        // middle.
        for i in 0..20 {
            let s = model.score(&example(2_000 + i, true, 0.0)).unwrap();
            assert!((s - 0.5).abs() < 0.2, "{s}");
        }
    }

    #[test]
    fn scores_only_embeddings_of_the_same_model() {
        let examples = set(40, 2, 1.0);
        let model = train(&refs(&examples)).unwrap();
        assert_eq!(model.score(&Embedding::new(&[1.0; 8]).unwrap()), None);
    }

    #[test]
    fn caps_each_class() {
        let many: Vec<(Embedding, bool)> = (0..MAX_EACH + 50)
            .map(|i| (example(i, false, 1.0), false))
            .chain((0..MIN_EACH).map(|i| (example(5_000 + i, true, 1.0), true)))
            .collect();
        let kept = capped(&refs(&many));
        assert_eq!(kept.iter().filter(|(_, p)| !p).count(), MAX_EACH);
        assert_eq!(kept.iter().filter(|(_, p)| *p).count(), MIN_EACH);
    }

    #[test]
    fn auc_counts_pairs() {
        assert_eq!(auc(&[(1.0, true), (0.0, false)]), Some(1.0));
        assert_eq!(auc(&[(0.0, true), (1.0, false)]), Some(0.0));
        assert_eq!(auc(&[(0.5, true), (0.5, false)]), Some(0.5));
        assert_eq!(
            auc(&[(0.9, true), (0.2, true), (0.5, false), (0.1, false)]),
            Some(0.75)
        );
        assert_eq!(auc(&[(1.0, true)]), None);
    }

    #[test]
    fn blends_general_and_personal_scores() {
        let close = |a: Option<f32>, b: f32| (a.unwrap() - b).abs() < 1e-6;
        assert!(close(blend(Some(0.2), Some(0.6), 0.5), 0.4));
        assert_eq!(blend(Some(0.2), Some(0.6), 0.0), Some(0.2));
        assert!(close(blend(Some(0.2), Some(0.6), 1.0), 0.6));
        assert_eq!(blend(Some(0.2), None, 1.0), Some(0.2));
        assert_eq!(blend(None, Some(0.6), 0.5), Some(0.6));
        assert_eq!(blend(None, Some(0.6), 0.0), None);
        assert_eq!(blend(None, None, 0.5), None);
    }
}
