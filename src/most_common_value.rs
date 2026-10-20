use rug::{Float, float};
use std::{error::Error, fmt};

// Keep the spelling of ZALPHA in the pinned NIST C++ source for traceability.
#[allow(clippy::excessive_precision)]
const NIST_Z_ALPHA: f64 = 2.575_829_303_548_900_8;

/// Confidence constant used in the upper probability bound.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConfidenceConstant {
    /// NIST reference constant: ZALPHA for MCV/Collision/Compression/predictors;
    /// the longer decimal ZALPHA_L for t-Tuple/LRS.
    #[default]
    NistReference,
    /// Decimal 2.576 as printed in section 6.3.1.
    Publication,
}

/// Arithmetic settings. Precision is measured in binary significand bits.
#[derive(Clone, Copy, Debug)]
pub struct McvOptions {
    pub precision_bits: u32,
    pub confidence_constant: ConfidenceConstant,
}

impl Default for McvOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
            confidence_constant: ConfidenceConstant::NistReference,
        }
    }
}

/// Intermediate probabilities and entropy, retained at the requested precision.
#[derive(Clone, Debug)]
pub struct McvEstimate {
    pub sample_count: usize,
    pub mode_count: usize,
    pub p_hat: Float,
    pub p_upper: Float,
    /// Estimated min-entropy in bits per sample, not per byte or entire dataset.
    pub min_entropy: Float,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EstimateError {
    TooFewSamples {
        count: usize,
    },
    InvalidPrecision {
        bits: u32,
    },
    NonBinarySample {
        index: usize,
        value: u8,
    },
    TooFewCollisions {
        count: usize,
    },
    InvalidDictionarySize,
    InvalidMaxLag,
    InvalidMmcDepth,
    InvalidLz78yContextLength,
    InvalidLz78yDictionarySize,
    InvalidMmcEntryLimit,
    TooFewPredictions {
        count: usize,
    },
    InvalidPredictionWindows,
    TooFewPredictionSamples {
        count: usize,
        window: usize,
    },
    InvalidTupleCutoff,
    NoRepeatedSubstringRange {
        min_length: usize,
        max_length: usize,
    },
    NoQualifyingTuple {
        cutoff: usize,
    },
    TooFewTestBlocks {
        blocks: usize,
        dictionary_blocks: usize,
    },
}

impl fmt::Display for EstimateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLz78yContextLength => write!(f, "LZ78Y context length must be in 1..=16"),
            Self::InvalidLz78yDictionarySize => write!(
                f,
                "LZ78Y dictionary limit must be at least the context length"
            ),
            Self::InvalidMmcEntryLimit => write!(f, "MMC entry limit must be positive"),
            Self::InvalidMmcDepth => write!(f, "MMC depth must be in 1..=16"),
            Self::InvalidMaxLag => write!(f, "maximum lag must be positive"),
            Self::TooFewPredictions { count } => {
                write!(f, "at least two predictions are required; got {count}")
            }
            Self::InvalidPredictionWindows => write!(
                f,
                "prediction windows must be positive and strictly increasing"
            ),
            Self::TooFewPredictionSamples { count, window } => write!(
                f,
                "{count} samples must exceed the largest prediction window ({window})"
            ),
            Self::NoRepeatedSubstringRange {
                min_length,
                max_length,
            } => write!(f, "LRS range is empty: u={min_length}, v={max_length}"),
            Self::InvalidTupleCutoff => write!(f, "tuple cutoff must be at least two"),
            Self::NoQualifyingTuple { cutoff } => {
                write!(f, "no tuple occurs at least {cutoff} times")
            }
            Self::InvalidDictionarySize => write!(f, "dictionary must contain at least one block"),
            Self::TooFewTestBlocks {
                blocks,
                dictionary_blocks,
            } => write!(
                f,
                "{blocks} blocks do not leave two test blocks after a {dictionary_blocks}-block dictionary"
            ),
            Self::NonBinarySample { index, value } => {
                write!(f, "sample {index} is {value}, expected 0 or 1")
            }
            Self::TooFewCollisions { count } => write!(
                f,
                "at least two completed collisions are required; got {count}"
            ),
            Self::TooFewSamples { count } => {
                write!(f, "at least two samples are required; got {count}")
            }
            Self::InvalidPrecision { bits } => {
                write!(f, "precision {bits} is outside 64..={}", float::prec_max())
            }
        }
    }
}
impl Error for EstimateError {}

/// Estimate MCV entropy for byte-valued symbols with 256-bit arithmetic.
///
/// Values 0..=255 are symbols; bytes are not automatically expanded to bits.
/// This estimator alone does not establish an IID assumption or validate a source.
///
/// ```
/// use sp_800_90b::most_common_value;
/// let estimate = most_common_value(&[7; 100])?;
/// assert_eq!(estimate.min_entropy, 0);
/// # Ok::<(), sp_800_90b::EstimateError>(())
/// ```
pub fn most_common_value(samples: &[u8]) -> Result<McvEstimate, EstimateError> {
    most_common_value_with_options(samples, McvOptions::default())
}

/// Section 6.3.1 with caller-selected precision and confidence constant.
///
/// Counts remain integers. All arithmetic after counting uses MPFR rounding
/// to nearest at the selected precision. This is not interval arithmetic or
/// a guarantee that the final rounded estimate is a mathematical lower bound.
pub fn most_common_value_with_options(
    samples: &[u8],
    options: McvOptions,
) -> Result<McvEstimate, EstimateError> {
    let precision = options.precision_bits;
    if !(64..=float::prec_max()).contains(&precision) {
        return Err(EstimateError::InvalidPrecision { bits: precision });
    }
    let n = samples.len();
    if n < 2 {
        return Err(EstimateError::TooFewSamples { count: n });
    }
    let mut counts = [0usize; 256];
    for &sample in samples {
        counts[usize::from(sample)] += 1;
    }
    let mode_count = counts.into_iter().max().unwrap();
    let mut p_hat = Float::with_val(precision, mode_count);
    p_hat /= n;

    let z = confidence_value(options.confidence_constant, precision);
    let mut adjustment = Float::with_val(precision, 1);
    adjustment -= &p_hat;
    adjustment *= &p_hat;
    adjustment /= n - 1;
    adjustment.sqrt_mut();
    adjustment *= z;
    let mut p_upper = p_hat.clone();
    p_upper += adjustment;
    if p_upper > 1 {
        p_upper = Float::with_val(precision, 1);
    }
    let min_entropy = if p_upper == 1 {
        Float::with_val(precision, 0)
    } else {
        -p_upper.clone().log2()
    };
    Ok(McvEstimate {
        sample_count: n,
        mode_count,
        p_hat,
        p_upper,
        min_entropy,
    })
}

pub(crate) fn confidence_value(constant: ConfidenceConstant, precision: u32) -> Float {
    match constant {
        // Deliberately preserve the reference's f64 constant, not a newly
        // computed normal quantile. Subsequent arithmetic is high precision.
        ConfidenceConstant::NistReference => Float::with_val(precision, NIST_Z_ALPHA),
        ConfidenceConstant::Publication => {
            let mut value = Float::with_val(precision, 2576);
            value /= 1000;
            value
        }
    }
}
