// Reference-informed adaptation of NIST SP800-90B_EntropyAssessment.
// Modified in Rust, 2026-10-19–2026-10-20; see docs/provenance.md for sources
// and changes. Preserve the upstream notice in LICENSE-NIST (also in LICENSE).
use crate::{ConfidenceConstant, EstimateError};
use rug::{Float, float};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct MultiMmcOptions {
    pub precision_bits: u32,
    pub confidence_constant: ConfidenceConstant,
    /// Standard depth is 16; supported range is 1..=16.
    pub max_depth: usize,
    /// Maximum distinct (context, next-symbol) pairs per depth; standard is 100000.
    pub max_entries: usize,
}
impl Default for MultiMmcOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
            confidence_constant: ConfidenceConstant::NistReference,
            max_depth: 16,
            max_entries: 100_000,
        }
    }
}
#[derive(Clone, Debug)]
pub struct MultiMmcEstimate {
    pub sample_count: usize,
    pub alphabet_size: usize,
    pub prediction_count: usize,
    pub correct_count: usize,
    /// NIST-compatible run counter: an unavailable winning prediction leaves it unchanged.
    pub longest_correct_run: usize,
    /// Scores for depths 1..=min(max_depth, sample_count-2).
    pub scoreboard: Vec<usize>,
    /// Distinct context/successor pairs stored per depth, including the final update.
    pub entries: Vec<usize>,
    pub p_global: Float,
    pub p_global_upper: Float,
    /// None when the local bound cannot affect the result.
    pub p_local: Option<Float>,
    pub p_upper: Float,
    pub min_entropy: Float,
}
#[derive(Default)]
struct Successors {
    counts: Vec<(u8, usize)>,
    best: (usize, u8),
}
impl Successors {
    fn increment(&mut self, symbol: u8, allow_new: bool) -> bool {
        let (count, added) =
            if let Some((_, count)) = self.counts.iter_mut().find(|(s, _)| *s == symbol) {
                *count += 1;
                (*count, false)
            } else if allow_new {
                self.counts.push((symbol, 1));
                (1, true)
            } else {
                return false;
            };
        self.best = self.best.max((count, symbol));
        added
    }
}
fn key(context: &[u8]) -> u128 {
    context
        .iter()
        .fold(0, |value, &symbol| (value << 8) | u128::from(symbol))
}

/// MultiMMC (§6.3.9), with NIST reference prediction and run-counting behavior.
/// Requires four byte-valued samples and infers the alphabet from the input.
///
/// ```
/// use sp_800_90b::multi_mmc_estimate;
/// let result = multi_mmc_estimate(&[7; 100]).unwrap();
/// assert_eq!(result.correct_count, 98);
/// assert_eq!(result.min_entropy, 0);
/// ```
pub fn multi_mmc_estimate(samples: &[u8]) -> Result<MultiMmcEstimate, EstimateError> {
    multi_mmc_estimate_with_options(samples, MultiMmcOptions::default())
}
pub fn multi_mmc_estimate_with_options(
    samples: &[u8],
    options: MultiMmcOptions,
) -> Result<MultiMmcEstimate, EstimateError> {
    let prec = options.precision_bits;
    if !(64..=float::prec_max()).contains(&prec) {
        return Err(EstimateError::InvalidPrecision { bits: prec });
    }
    if !(1..=16).contains(&options.max_depth) {
        return Err(EstimateError::InvalidMmcDepth);
    }
    if options.max_entries == 0 {
        return Err(EstimateError::InvalidMmcEntryLimit);
    }
    if samples.len() < 4 {
        return Err(EstimateError::TooFewPredictions {
            count: samples.len().saturating_sub(2),
        });
    }
    let depth = options.max_depth.min(samples.len() - 2);
    let (correct, longest, scoreboard, entries) = if samples.iter().all(|&s| s <= 1) {
        binary_counts(samples, depth, options.max_entries)
    } else {
        general_counts(samples, depth, options.max_entries)
    };
    let mut seen = [false; 256];
    for &s in samples {
        seen[s as usize] = true;
    }
    let k = seen.iter().filter(|&&x| x).count();
    let n = samples.len() - 2;
    let (p_global, p_global_upper, p_local, p_upper, min_entropy) =
        crate::prediction::estimate(correct, n, longest, k, prec, options.confidence_constant);
    Ok(MultiMmcEstimate {
        sample_count: samples.len(),
        alphabet_size: k,
        prediction_count: n,
        correct_count: correct,
        longest_correct_run: longest,
        scoreboard,
        entries,
        p_global,
        p_global_upper,
        p_local,
        p_upper,
        min_entropy,
    })
}

// Binary contexts have at most 2^16 values. Direct tables avoid hashing while
// preserving the per-depth cap on distinct (context, successor) pairs.
fn binary_counts(
    samples: &[u8],
    depth: usize,
    max_entries: usize,
) -> (usize, usize, Vec<usize>, Vec<usize>) {
    let mut models: Vec<Vec<[usize; 2]>> = (1..=depth).map(|d| vec![[0; 2]; 1 << d]).collect();
    let mut entries = vec![1; depth];
    let mut scoreboard = vec![0; depth];
    let mut context = 0;
    for (d, model) in models.iter_mut().enumerate() {
        context = (context << 1) | usize::from(samples[d]);
        model[context][usize::from(samples[d + 1])] = 1;
    }
    let (mut winner, mut correct, mut run, mut longest) = (0, 0, 0, 0);
    for i in 2..samples.len() {
        let previous_winner = winner;
        let symbol = usize::from(samples[i]);
        let mut context = 0;
        for d in 0..depth.min(i - 1) {
            context |= usize::from(samples[i - d - 1]) << d;
            let counts = &mut models[d][context];
            let predicted = usize::from(counts[1] >= counts[0]);
            if counts[predicted] != 0 {
                if predicted == symbol {
                    scoreboard[d] += 1;
                    if scoreboard[d] >= scoreboard[winner] {
                        winner = d;
                    }
                    if d == previous_winner {
                        correct += 1;
                        run += 1;
                        longest = longest.max(run);
                    }
                } else if d == previous_winner {
                    run = 0;
                }
            }
            // An unavailable winning prediction leaves the run unchanged.
            if counts[symbol] != 0 {
                counts[symbol] += 1;
            } else if entries[d] < max_entries {
                counts[symbol] = 1;
                entries[d] += 1;
            }
        }
    }
    (correct, longest, scoreboard, entries)
}

fn general_counts(
    samples: &[u8],
    depth: usize,
    max_entries: usize,
) -> (usize, usize, Vec<usize>, Vec<usize>) {
    let mut models: Vec<HashMap<u128, Successors>> = (0..depth).map(|_| HashMap::new()).collect();
    let mut entries = vec![1usize; depth];
    let mut scoreboard = vec![0usize; depth];
    // First transition at every depth. Each model is queried only after this
    // transition is wholly in the past; initialization cannot leak future data.
    for (d, model) in models.iter_mut().enumerate() {
        let mut successors = Successors::default();
        successors.increment(samples[d + 1], true);
        model.insert(key(&samples[..d + 1]), successors);
    }
    let (mut winner, mut correct, mut run, mut longest) = (0, 0, 0, 0);
    for i in 2..samples.len() {
        let previous_winner = winner;
        for d in 0..depth.min(i - 1) {
            let context = key(&samples[i - d - 1..i]);
            let prediction = models[d].get(&context).map(|s| s.best.1);
            if let Some(predicted) = prediction {
                if predicted == samples[i] {
                    scoreboard[d] += 1;
                    if scoreboard[d] >= scoreboard[winner] {
                        winner = d;
                    }
                    if d == previous_winner {
                        correct += 1;
                        run += 1;
                        longest = longest.max(run);
                    }
                } else if d == previous_winner {
                    run = 0;
                }
            }
            // Intentionally mirror NIST: a null winning prediction does not
            // reset the run. This differs from a run of consecutive correct bits.
            let allow_new = entries[d] < max_entries;
            if let Some(successors) = models[d].get_mut(&context) {
                if successors.increment(samples[i], allow_new) {
                    entries[d] += 1;
                }
            } else if allow_new {
                let mut successors = Successors::default();
                successors.increment(samples[i], true);
                models[d].insert(context, successors);
                entries[d] += 1;
            }
        }
    }
    (correct, longest, scoreboard, entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binary_tables_match_general_models_exhaustively() {
        for n in 4..=10 {
            for pattern in 0..1usize << n {
                let samples: Vec<_> = (0..n).map(|i| ((pattern >> i) & 1) as u8).collect();
                for depth in 1..=4.min(n - 2) {
                    for cap in [1, 2, 7, 100_000] {
                        assert_eq!(
                            binary_counts(&samples, depth, cap),
                            general_counts(&samples, depth, cap),
                            "samples={samples:?}, depth={depth}, cap={cap}"
                        );
                    }
                }
            }
        }
        let mut state = 103u64;
        let samples: Vec<_> = (0..8192)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state & 1) as u8
            })
            .collect();
        for cap in [1, 7, 100_000] {
            assert_eq!(
                binary_counts(&samples, 16, cap),
                general_counts(&samples, 16, cap)
            );
        }
    }

    #[test]
    fn ties_and_existing_counts_after_capacity() {
        let mut s = Successors::default();
        assert!(s.increment(7, true));
        assert!(s.increment(255, true));
        assert_eq!(s.best, (1, 255));
        assert!(!s.increment(100, false));
        assert!(!s.increment(7, false));
        assert_eq!(s.best, (2, 7));
        assert!(!s.increment(255, false));
        assert_eq!(s.best, (2, 255));
        assert_eq!(s.counts.len(), 2);
    }
}
