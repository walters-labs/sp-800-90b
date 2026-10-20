//! Output_Entropy (§3.1.5), with conservative directed MPFR rounding.
use rug::{
    Float,
    float::{self, Round},
};
use std::{error::Error, fmt};
#[derive(Clone, Copy, Debug)]
pub struct ConditioningOptions {
    pub precision_bits: u32,
    /// Resource bound for automatic working precision (default one mebibit).
    pub max_working_precision: u32,
}
impl Default for ConditioningOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
            max_working_precision: 1_048_576,
        }
    }
}
#[derive(Clone, Debug)]
pub enum ConditioningKind {
    /// Caller asserts the component is vetted; this API does not vet algorithms.
    Vetted,
    NonVetted {
        assessed_entropy_per_bit: Float,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditioningError {
    InvalidDimensions,
    InvalidEntropy,
    InvalidPerBitEntropy,
    InvalidPrecision,
    PrecisionBudgetExceeded,
    NumericalFailure,
}
impl fmt::Display for ConditioningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conditioning error: {self:?}")
    }
}
impl Error for ConditioningError {}
#[derive(Clone, Debug)]
pub struct ConditioningEstimate {
    pub input_bits: u32,
    pub output_bits: u32,
    pub effective_internal_width: u32,
    pub working_precision: u32,
    /// Conservative upper bounds for the two probability expressions.
    pub psi_upper: Float,
    pub omega_upper: Float,
    /// Lower bound before non-vetted caps, in bits per output block.
    pub output_entropy: Float,
    pub non_vetted_cap: Option<Float>,
    pub assessed_output_cap: Option<Float>,
    pub min_entropy: Float,
}
/// Calculate a vetted or non-vetted conditioning entropy bound. Inputs are bits
/// per conditioning block, except h' which is bits per output bit. Zero entropy
/// is supported explicitly. No automatic claim of full entropy is made.
pub fn conditioning_entropy(
    input_bits: u32,
    output_bits: u32,
    internal_width: u32,
    input_entropy: &Float,
    kind: ConditioningKind,
    options: ConditioningOptions,
) -> Result<ConditioningEstimate, ConditioningError> {
    if input_bits == 0 || output_bits == 0 || internal_width == 0 {
        return Err(ConditioningError::InvalidDimensions);
    }
    if !input_entropy.is_finite() || input_entropy < &0 || input_entropy > &input_bits {
        return Err(ConditioningError::InvalidEntropy);
    }
    if !(64..=float::prec_max()).contains(&options.precision_bits)
        || options.max_working_precision < options.precision_bits
        || options.max_working_precision > float::prec_max()
    {
        return Err(ConditioningError::InvalidPrecision);
    }
    if let ConditioningKind::NonVetted {
        assessed_entropy_per_bit: h,
    } = &kind
        && (!h.is_finite() || !(0..=1).contains(h))
    {
        return Err(ConditioningError::InvalidPerBitEntropy);
    }
    let precision = input_bits
        .checked_mul(2)
        .and_then(|x| x.checked_add(64))
        .ok_or(ConditioningError::PrecisionBudgetExceeded)?
        .max(options.precision_bits.saturating_add(32))
        .max(input_entropy.prec().saturating_add(32));
    if precision > options.max_working_precision {
        return Err(ConditioningError::PrecisionBudgetExceeded);
    }
    let width = internal_width.min(input_bits);
    let n = output_bits.min(width);
    let up = |x| Float::with_val_round(precision, x, Round::Up).0;
    let mut p_high = Float::with_val(precision, -input_entropy);
    let mut p_high_lower = p_high.clone();
    p_high.exp2_round(Round::Up);
    p_high_lower.exp2_round(Round::Down);
    let mut denominator = Float::with_val(precision, 1);
    denominator <<= input_bits;
    denominator -= 1;
    let numerator = Float::with_val_round(
        precision,
        &Float::with_val(precision, 1) - &p_high_lower,
        Round::Up,
    )
    .0;
    let p_low = Float::with_val_round(precision, &numerator / &denominator, Round::Up).0;
    let mut power = Float::with_val(precision, 1);
    power <<= input_bits - n;
    let mut psi = Float::with_val_round(precision, &power * &p_low, Round::Up).0;
    psi = Float::with_val_round(precision, &psi + &p_high, Round::Up).0;
    let mut omega = up(2);
    omega.ln_round(Round::Up);
    omega = Float::with_val_round(precision, &omega * &power, Round::Up).0;
    omega = Float::with_val_round(precision, &omega * (2u64 * n as u64), Round::Up).0;
    omega.sqrt_round(Round::Up);
    omega = Float::with_val_round(precision, &omega + &power, Round::Up).0;
    omega = Float::with_val_round(precision, &omega * &p_low, Round::Up).0;
    psi = psi.min(&Float::with_val(precision, 1));
    omega = omega.min(&Float::with_val(precision, 1));
    let mut bound = psi.clone().max(&omega);
    bound.log2_round(Round::Up);
    bound = -bound;
    if !bound.is_finite() || bound < 0 {
        return Err(ConditioningError::NumericalFailure);
    }
    let output_entropy = Float::with_val_round(options.precision_bits, bound, Round::Down).0;
    let (non_vetted_cap, assessed_output_cap) = match kind {
        ConditioningKind::Vetted => (None, None),
        ConditioningKind::NonVetted {
            assessed_entropy_per_bit: h,
        } => {
            let cap = Float::with_val_round(
                precision,
                Float::with_val(precision, 999) * output_bits,
                Round::Down,
            )
            .0;
            let cap = Float::with_val_round(options.precision_bits, &cap / 1000, Round::Down).0;
            let assessed =
                Float::with_val_round(options.precision_bits, &h * output_bits, Round::Down).0;
            (Some(cap), Some(assessed))
        }
    };
    let mut min_entropy = output_entropy.clone();
    if let Some(cap) = &non_vetted_cap {
        min_entropy = min_entropy.min(cap);
    }
    if let Some(cap) = &assessed_output_cap {
        min_entropy = min_entropy.min(cap);
    }
    Ok(ConditioningEstimate {
        input_bits,
        output_bits,
        effective_internal_width: width,
        working_precision: precision,
        psi_upper: psi,
        omega_upper: omega,
        output_entropy,
        non_vetted_cap,
        assessed_output_cap,
        min_entropy,
    })
}
