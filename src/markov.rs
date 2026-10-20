use crate::EstimateError;
use rug::{Float, float};

/// Arithmetic precision for the binary Markov estimator (no confidence multiplier).
#[derive(Clone, Copy, Debug)]
pub struct MarkovOptions {
    pub precision_bits: u32,
}
impl Default for MarkovOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
        }
    }
}

/// Section 6.3.3 statistics, retained at the requested precision.
#[derive(Clone, Debug)]
pub struct MarkovEstimate {
    pub sample_count: usize,
    pub symbol_counts: [usize; 2],
    /// Overlapping transition counts, indexed by `[previous][next]`.
    pub transition_counts: [[usize; 2]; 2],
    pub initial_probabilities: [Float; 2],
    /// Unobserved outgoing rows are both zero, matching NIST.
    pub transition_probabilities: [[Float; 2]; 2],
    /// Negative log2 probabilities for the six sequences in the paper's order:
    /// all zeros, alternating from zero, zero then ones, one then zeros,
    /// alternating from one, all ones. None means a zero-probability sequence.
    pub sequence_entropies: [Option<Float>; 6],
    /// Minimum sequence entropy divided by 128, capped at one bit/sample.
    /// If all candidate sequences are impossible, NIST's fallback is one.
    pub min_entropy: Float,
}

/// Estimate binary Markov min-entropy with 256-bit MPFR arithmetic.
///
/// Each byte must be 0 or 1; at least two samples are required. This models
/// 128-bit sequences regardless of input length and is not source validation.
/// ```
/// let result = sp_800_90b::markov_estimate(&[0, 0, 0, 0])?;
/// assert_eq!(result.min_entropy, 0);
/// # Ok::<(), sp_800_90b::EstimateError>(())
/// ```
pub fn markov_estimate(samples: &[u8]) -> Result<MarkovEstimate, EstimateError> {
    markov_estimate_with_options(samples, MarkovOptions::default())
}

/// Configurable section 6.3.3 estimator, using log-probabilities to avoid
/// underflow from multiplying 127 small transition probabilities.
pub fn markov_estimate_with_options(
    samples: &[u8],
    options: MarkovOptions,
) -> Result<MarkovEstimate, EstimateError> {
    let precision = options.precision_bits;
    if !(64..=float::prec_max()).contains(&precision) {
        return Err(EstimateError::InvalidPrecision { bits: precision });
    }
    if samples.len() < 2 {
        return Err(EstimateError::TooFewSamples {
            count: samples.len(),
        });
    }
    let mut symbol_counts = [0; 2];
    let mut transition_counts = [[0; 2]; 2];
    for (index, &value) in samples.iter().enumerate() {
        if value > 1 {
            return Err(EstimateError::NonBinarySample { index, value });
        }
        symbol_counts[usize::from(value)] += 1;
        if index > 0 {
            transition_counts[usize::from(samples[index - 1])][usize::from(value)] += 1;
        }
    }
    let initial_probabilities = std::array::from_fn(|i| {
        let mut p = Float::with_val(precision, symbol_counts[i]);
        p /= samples.len();
        p
    });
    let transition_probabilities = std::array::from_fn(|i| {
        let denominator = transition_counts[i][0] + transition_counts[i][1];
        std::array::from_fn(|j| {
            let mut p = Float::with_val(precision, transition_counts[i][j]);
            if denominator > 0 {
                p /= denominator;
            }
            p
        })
    });
    // Each row lists a starting symbol and counts of 00,01,10,11 transitions.
    let candidates = [
        (0, [127, 0, 0, 0]),
        (0, [0, 64, 63, 0]),
        (0, [0, 1, 0, 126]),
        (1, [126, 0, 1, 0]),
        (1, [0, 63, 64, 0]),
        (1, [0, 0, 0, 127]),
    ];
    let sequence_entropies = candidates.map(|(start, powers)| {
        if initial_probabilities[start] == 0 {
            return None;
        }
        let mut entropy = -initial_probabilities[start].clone().log2();
        for (index, power) in powers.into_iter().enumerate() {
            if power == 0 {
                continue;
            }
            let p = &transition_probabilities[index / 2][index % 2];
            if p == &0 {
                return None;
            }
            let mut term = -p.clone().log2();
            term *= power;
            entropy += term;
        }
        Some(entropy)
    });
    let mut min_entropy = Float::with_val(precision, 128);
    for entropy in sequence_entropies.iter().flatten() {
        if entropy < &min_entropy {
            min_entropy = entropy.clone();
        }
    }
    min_entropy /= 128;
    if min_entropy == 0 {
        min_entropy = Float::with_val(precision, 0);
    }
    Ok(MarkovEstimate {
        sample_count: samples.len(),
        symbol_counts,
        transition_counts,
        initial_probabilities,
        transition_probabilities,
        sequence_entropies,
        min_entropy,
    })
}
