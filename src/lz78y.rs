// Reference-informed adaptation of NIST SP800-90B_EntropyAssessment.
// Modified in Rust, 2026-10-19–2026-10-20; see docs/provenance.md for sources
// and changes. Preserve the upstream notice in LICENSE-NIST (also in LICENSE).
use crate::{ConfidenceConstant, EstimateError};
use rug::{Float, float};
use std::collections::{HashMap, hash_map::Entry};

#[derive(Clone, Copy, Debug)]
pub struct Lz78yOptions {
    pub precision_bits: u32,
    pub confidence_constant: ConfidenceConstant,
    /// Standard B is 16; supported range is 1..=16.
    pub context_length: usize,
    /// Global context limit, standard 65536; must be at least context_length.
    pub max_dictionary_size: usize,
}
impl Default for Lz78yOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
            confidence_constant: ConfidenceConstant::NistReference,
            context_length: 16,
            max_dictionary_size: 65536,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Lz78yEstimate {
    pub sample_count: usize,
    pub alphabet_size: usize,
    pub prediction_count: usize,
    pub correct_count: usize,
    pub longest_correct_run: usize,
    /// Number of stored contexts, across all lengths, including the final update.
    pub dictionary_size: usize,
    /// Stored contexts for lengths 1..=B, including the final update.
    pub contexts_per_length: Vec<usize>,
    pub p_global: Float,
    pub p_global_upper: Float,
    /// None when the local bound cannot affect the result.
    pub p_local: Option<Float>,
    pub p_upper: Float,
    /// Bits per input symbol.
    pub min_entropy: Float,
}
#[derive(Default)]
struct Successors {
    counts: Vec<(u8, usize)>,
    best: (usize, u8),
}
impl Successors {
    fn increment(&mut self, symbol: u8) {
        let count = if let Some((_, count)) = self.counts.iter_mut().find(|(s, _)| *s == symbol) {
            *count += 1;
            *count
        } else {
            self.counts.push((symbol, 1));
            1
        };
        self.best = self.best.max((count, symbol));
    }
}

/// LZ78Y (§6.3.10) with B=16 and 256-bit MPFR arithmetic.
/// Requires 19 byte-valued samples, leaving at least two predictions.
///
/// ```
/// use sp_800_90b::lz78y_estimate;
/// let result = lz78y_estimate(&[7; 100]).unwrap();
/// assert_eq!(result.correct_count, 83);
/// assert_eq!(result.min_entropy, 0);
/// ```
pub fn lz78y_estimate(samples: &[u8]) -> Result<Lz78yEstimate, EstimateError> {
    lz78y_estimate_with_options(samples, Lz78yOptions::default())
}
pub fn lz78y_estimate_with_options(
    samples: &[u8],
    options: Lz78yOptions,
) -> Result<Lz78yEstimate, EstimateError> {
    let prec = options.precision_bits;
    if !(64..=float::prec_max()).contains(&prec) {
        return Err(EstimateError::InvalidPrecision { bits: prec });
    }
    let b = options.context_length;
    if !(1..=16).contains(&b) {
        return Err(EstimateError::InvalidLz78yContextLength);
    }
    if options.max_dictionary_size < b {
        return Err(EstimateError::InvalidLz78yDictionarySize);
    }
    if samples.len() < b + 3 {
        return Err(EstimateError::TooFewPredictions {
            count: samples.len().saturating_sub(b + 1),
        });
    }
    let masks: Vec<u128> = (1..=b)
        .map(|length| {
            if length == 16 {
                u128::MAX
            } else {
                (1u128 << (length * 8)) - 1
            }
        })
        .collect();
    let mut dictionary: Vec<HashMap<u128, Successors>> = (0..b).map(|_| HashMap::new()).collect();
    let mut pattern = samples[..b]
        .iter()
        .fold(0u128, |value, &s| (value << 8) | u128::from(s));
    // One initial context of each length; all are in the past before predicting.
    for j in 0..b {
        let mut successors = Successors::default();
        successors.increment(samples[b]);
        dictionary[j].insert(pattern & masks[j], successors);
    }
    let mut dictionary_size = b;
    let (mut correct, mut run, mut longest) = (0, 0, 0);
    for i in b + 1..samples.len() {
        pattern = (pattern << 8) | u128::from(samples[i - 1]);
        let mut prediction = None;
        let mut max_count = 0;
        // Descending length and strict count comparison preserve the longer
        // context on ties. Within a context, the greater symbol wins a tie.
        for j in (0..b).rev() {
            match dictionary[j].entry(pattern & masks[j]) {
                Entry::Occupied(mut entry) => {
                    let successors = entry.get_mut();
                    if successors.best.0 > max_count {
                        max_count = successors.best.0;
                        prediction = Some(successors.best.1);
                    }
                    // The cap counts contexts, not successor pairs. Existing
                    // contexts can always learn new successors after saturation.
                    successors.increment(samples[i]);
                }
                Entry::Vacant(entry) if dictionary_size < options.max_dictionary_size => {
                    let mut successors = Successors::default();
                    successors.increment(samples[i]);
                    entry.insert(successors);
                    dictionary_size += 1;
                }
                Entry::Vacant(_) => {}
            }
        }
        if prediction == Some(samples[i]) {
            correct += 1;
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let mut seen = [false; 256];
    for &symbol in samples {
        seen[symbol as usize] = true;
    }
    let k = seen.iter().filter(|&&x| x).count();
    let n = samples.len() - b - 1;
    let (p_global, p_global_upper, p_local, p_upper, min_entropy) =
        crate::prediction::estimate(correct, n, longest, k, prec, options.confidence_constant);
    Ok(Lz78yEstimate {
        sample_count: samples.len(),
        alphabet_size: k,
        prediction_count: n,
        correct_count: correct,
        longest_correct_run: longest,
        dictionary_size,
        contexts_per_length: dictionary.iter().map(HashMap::len).collect(),
        p_global,
        p_global_upper,
        p_local,
        p_upper,
        min_entropy,
    })
}
