// Reference-informed adaptation of NIST SP800-90B_EntropyAssessment.
// Modified in Rust, 2026-10-19–2026-10-20; see docs/provenance.md for sources
// and changes. Preserve the upstream notice in LICENSE-NIST (also in LICENSE).
use crate::{ConfidenceConstant, EstimateError};
use rug::{Float, float};

#[derive(Clone, Copy, Debug)]
pub struct TTupleOptions {
    pub precision_bits: u32,
    pub confidence_constant: ConfidenceConstant,
    /// Standard value is 35. Overrides support the publication's cutoff-3 example.
    pub cutoff: usize,
}
impl Default for TTupleOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
            confidence_constant: ConfidenceConstant::NistReference,
            cutoff: 35,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TTupleEstimate {
    pub sample_count: usize,
    pub max_tuple_length: usize,
    /// Q[i-1] is the maximum count of an overlapping i-tuple, for i=1..=t.
    pub maximum_counts: Vec<usize>,
    pub p_hat: Float,
    pub p_upper: Float,
    pub min_entropy: Float,
}

fn maximum_counts(samples: &[u8]) -> Vec<usize> {
    crate::tuples::tuple_counts(samples, false).0
}

/// Section 6.3.5 on byte-valued symbols, counting overlapping tuples.
/// Uses 256-bit MPFR arithmetic and cutoff 35 by default.
pub fn t_tuple_estimate(samples: &[u8]) -> Result<TTupleEstimate, EstimateError> {
    t_tuple_estimate_with_options(samples, TTupleOptions::default())
}

/// Reference mode parses NIST's ZALPHA_L decimal (2.575829303548900384158)
/// directly at the requested precision. Publication mode uses decimal 2.576.
/// No qualifying tuple returns an error rather than NIST's numeric -1 sentinel.
pub fn t_tuple_estimate_with_options(
    samples: &[u8],
    options: TTupleOptions,
) -> Result<TTupleEstimate, EstimateError> {
    let prec = options.precision_bits;
    if !(64..=float::prec_max()).contains(&prec) {
        return Err(EstimateError::InvalidPrecision { bits: prec });
    }
    if samples.len() < 2 {
        return Err(EstimateError::TooFewSamples {
            count: samples.len(),
        });
    }
    if options.cutoff < 2 {
        return Err(EstimateError::InvalidTupleCutoff);
    }
    let n = samples.len();
    let mut frequencies = [0usize; 256];
    for &value in samples {
        frequencies[usize::from(value)] += 1;
    }
    if frequencies.into_iter().max().unwrap() < options.cutoff {
        return Err(EstimateError::NoQualifyingTuple {
            cutoff: options.cutoff,
        });
    }
    let q = maximum_counts(samples);
    estimate_from_counts(n, &q, options)
}

// The coordinator shares suffix counts with LRS. Public entry points validate
// options first; this helper also handles the no-qualifying-tuple outcome.
pub(crate) fn estimate_from_counts(
    n: usize,
    q: &[usize],
    options: TTupleOptions,
) -> Result<TTupleEstimate, EstimateError> {
    let prec = options.precision_bits;
    let t = (1..=n)
        .take_while(|&i| q[i] >= options.cutoff)
        .last()
        .ok_or(EstimateError::NoQualifyingTuple {
            cutoff: options.cutoff,
        })?;
    let mut best_log = Float::with_val(prec, f64::NEG_INFINITY);
    for (i, &count) in q.iter().enumerate().take(t + 1).skip(1) {
        let mut value = Float::with_val(prec, count);
        value /= n - i + 1;
        value.log2_mut();
        value /= i;
        if value > best_log {
            best_log = value;
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
    Ok(TTupleEstimate {
        sample_count: n,
        max_tuple_length: t,
        maximum_counts: q[1..=t].to_vec(),
        p_hat,
        p_upper,
        min_entropy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    #[test]
    fn suffix_counts_match_exhaustive_overlapping_counts() {
        for n in 2..=9 {
            for pattern in 0..(1usize << n) {
                let s: Vec<_> = (0..n).map(|i| ((pattern >> i) & 1) as u8).collect();
                let q = maximum_counts(&s);
                for (width, &actual_count) in q.iter().enumerate().skip(1) {
                    let mut counts = HashMap::new();
                    for tuple in s.windows(width) {
                        *counts.entry(tuple).or_insert(0) += 1;
                    }
                    assert_eq!(
                        actual_count,
                        *counts.values().max().unwrap(),
                        "{s:?}, width={width}"
                    );
                }
            }
        }
        let s: Vec<_> = (0..=255).cycle().take(600).collect();
        let q = maximum_counts(&s);
        for (width, &actual_count) in q.iter().enumerate().skip(1) {
            let mut counts = HashMap::new();
            for tuple in s.windows(width) {
                *counts.entry(tuple).or_insert(0) += 1;
            }
            assert_eq!(actual_count, *counts.values().max().unwrap());
        }
    }
}
