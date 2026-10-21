// Reference-informed adaptation of NIST SP800-90B_EntropyAssessment.
// Modified in Rust, 2026-10-19–2026-10-20; see docs/provenance.md for sources
// and changes. Preserve the upstream notice in LICENSE-NIST (also in LICENSE).
use crate::{ConfidenceConstant, EstimateError};
use rug::{Float, float};

#[derive(Clone, Copy, Debug)]
pub struct LrsOptions {
    pub precision_bits: u32,
    pub confidence_constant: ConfidenceConstant,
    /// Standard cutoff is 35; override for the publication's cutoff-3 example.
    pub cutoff: usize,
}
impl Default for LrsOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
            confidence_constant: ConfidenceConstant::NistReference,
            cutoff: 35,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LrsEstimate {
    pub sample_count: usize,
    /// First length whose most common tuple occurs fewer than cutoff times.
    pub min_length: usize,
    /// Longest tuple that occurs at least twice.
    pub max_length: usize,
    /// Sum of choose(count,2), for lengths u..=v, including overlaps.
    pub pair_counts: Vec<u128>,
    pub p_hat: Float,
    pub p_upper: Float,
    /// Bits per sample.
    pub min_entropy: Float,
}

/// Section 6.3.6 for byte-valued symbols, with 256-bit arithmetic and cutoff 35.
/// Returns an error if there are no repeated substrings in the required range.
///
/// ```
/// use sp_800_90b::lrs_estimate;
/// let estimate = lrs_estimate(&[0; 100]).unwrap();
/// assert_eq!((estimate.min_length, estimate.max_length), (67, 99));
/// assert_eq!(estimate.min_entropy, 0);
/// ```
pub fn lrs_estimate(samples: &[u8]) -> Result<LrsEstimate, EstimateError> {
    lrs_estimate_with_options(samples, LrsOptions::default())
}

/// Computes exact overlapping substring pair counts using suffix-array intervals.
/// Uses the decimal ZALPHA_L constant in reference mode, as for t-Tuple.
pub fn lrs_estimate_with_options(
    samples: &[u8],
    options: LrsOptions,
) -> Result<LrsEstimate, EstimateError> {
    let prec = options.precision_bits;
    if !(64..=float::prec_max()).contains(&prec) {
        return Err(EstimateError::InvalidPrecision { bits: prec });
    }
    let n = samples.len();
    if n < 2 {
        return Err(EstimateError::TooFewSamples { count: n });
    }
    if options.cutoff < 2 {
        return Err(EstimateError::InvalidTupleCutoff);
    }
    let (q, pairs) = crate::tuples::tuple_counts(samples, true);
    estimate_from_counts(n, &q, &pairs, options)
}

pub(crate) fn estimate_from_counts(
    n: usize,
    q: &[usize],
    pairs: &[u128],
    options: LrsOptions,
) -> Result<LrsEstimate, EstimateError> {
    let prec = options.precision_bits;
    let u = (1..=n).find(|&i| q[i] < options.cutoff).unwrap();
    let v = (1..=n).rfind(|&i| q[i] >= 2).unwrap_or(0);
    if v < u {
        return Err(EstimateError::NoRepeatedSubstringRange {
            min_length: u,
            max_length: v,
        });
    }
    let mut best_log = Float::with_val(prec, f64::NEG_INFINITY);
    for (width, &count) in pairs.iter().enumerate().take(v + 1).skip(u) {
        let positions = (n - width + 1) as u128;
        let denominator = positions * (positions - 1) / 2;
        let mut probability = Float::with_val(prec, count);
        probability /= Float::with_val(prec, denominator);
        probability.log2_mut();
        probability /= width;
        if probability > best_log {
            best_log = probability;
        }
        if best_log == 0 {
            break;
        }
    }
    let p_hat = best_log.exp2();
    let z = Float::with_val(
        prec,
        Float::parse(match options.confidence_constant {
            ConfidenceConstant::NistReference => "2.575829303548900384158",
            ConfidenceConstant::Publication => "2.576",
        })
        .unwrap(),
    );
    let mut adjustment = Float::with_val(prec, 1);
    adjustment -= &p_hat;
    adjustment *= &p_hat;
    adjustment /= n - 1;
    adjustment.sqrt_mut();
    adjustment *= z;
    let mut p_upper = p_hat.clone();
    p_upper += adjustment;
    if p_upper > 1 {
        p_upper = Float::with_val(prec, 1);
    }
    let min_entropy = if p_upper == 1 {
        Float::with_val(prec, 0)
    } else {
        -p_upper.clone().log2()
    };
    Ok(LrsEstimate {
        sample_count: n,
        min_length: u,
        max_length: v,
        pair_counts: pairs[u..=v].to_vec(),
        p_hat,
        p_upper,
        min_entropy,
    })
}
