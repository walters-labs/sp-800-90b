use crate::most_common_value::confidence_value;
use crate::{EstimateError, McvOptions};
use rug::{Float, float};

/// Precision and confidence settings, identical to those used by MCV.
pub type CollisionOptions = McvOptions;

/// Collision statistics and estimate, using NIST reference boundary handling.
#[derive(Clone, Debug)]
pub struct CollisionEstimate {
    pub collision_count: usize,
    pub consumed_samples: usize,
    pub mean: Float,
    pub standard_deviation: Float,
    /// Confidence-adjusted mean before NIST's clamp to 2.
    pub raw_lower_bound: Float,
    pub lower_bound: Float,
    /// Most likely symbol probability, or 1/2 for the full-entropy fallback.
    pub p: Float,
    pub min_entropy: Float,
}

/// Estimate section 6.3.2 entropy from one binary sample per byte.
///
/// Incomplete trailing groups are discarded. Nonbinary input (including a
/// nonbinary tail) and fewer than two completed collisions are rejected.
/// The default uses 256-bit MPFR arithmetic and NIST's confidence constant.
pub fn collision_estimate(samples: &[u8]) -> Result<CollisionEstimate, EstimateError> {
    collision_estimate_with_options(samples, CollisionOptions::default())
}

/// Configurable collision estimator. The constant option changes only the
/// confidence multiplier; NIST reference clamping applies in both modes.
pub fn collision_estimate_with_options(
    samples: &[u8],
    options: CollisionOptions,
) -> Result<CollisionEstimate, EstimateError> {
    let precision = options.precision_bits;
    if !(64..=float::prec_max()).contains(&precision) {
        return Err(EstimateError::InvalidPrecision { bits: precision });
    }
    for (index, &value) in samples.iter().enumerate() {
        if value > 1 {
            return Err(EstimateError::NonBinarySample { index, value });
        }
    }
    let (mut index, mut twos, mut threes) = (0, 0usize, 0usize);
    while samples.len() - index >= 2 {
        if samples[index] == samples[index + 1] {
            twos += 1;
            index += 2;
        } else if samples.len() - index >= 3 {
            threes += 1;
            index += 3;
        } else {
            break;
        }
    }
    let v = twos + threes;
    if v < 2 {
        return Err(EstimateError::TooFewCollisions { count: v });
    }
    let mut mean = Float::with_val(precision, index);
    mean /= v;
    // For lengths in {2,3}, sample variance = n2*n3 / (v*(v-1)).
    // Convert before multiplying to avoid integer overflow and cancellation.
    let mut variance = Float::with_val(precision, twos);
    variance *= threes;
    variance /= v;
    variance /= v - 1;
    let standard_deviation = variance.sqrt();
    let mut adjustment = standard_deviation.clone();
    adjustment /= Float::with_val(precision, v).sqrt();
    adjustment *= confidence_value(options.confidence_constant, precision);
    let raw_lower_bound = Float::with_val(precision, &mean - adjustment);
    let lower_bound = if raw_lower_bound < 2 {
        Float::with_val(precision, 2)
    } else {
        raw_lower_bound.clone()
    };
    // E[T] = 2 + 2p(1-p); choose the root p >= 1/2.
    let p = if lower_bound >= 2.5 {
        Float::with_val(precision, 0.5)
    } else {
        let mut discriminant = Float::with_val(precision, 5);
        discriminant -= Float::with_val(precision, &lower_bound * 2);
        discriminant.sqrt_mut();
        discriminant += 1;
        discriminant /= 2;
        discriminant
    };
    let min_entropy = if p == 1 {
        Float::with_val(precision, 0)
    } else {
        -p.clone().log2()
    };
    Ok(CollisionEstimate {
        collision_count: v,
        consumed_samples: index,
        mean,
        standard_deviation,
        raw_lower_bound,
        lower_bound,
        p,
        min_entropy,
    })
}
