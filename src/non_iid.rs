// Reference-informed adaptation of NIST SP800-90B_EntropyAssessment.
// Modified in Rust, 2026-10-19–2026-10-20; see docs/provenance.md for sources
// and changes. Preserve the upstream notice in LICENSE-NIST (also in LICENSE).
//! Complete non-IID estimator orchestration, following NIST's ea_non_iid.
use crate::*;
use rug::{Float, float};
use std::{error::Error, fmt};

/// Selects the equivalent of ea_non_iid's -i or -c option.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NonIidMode {
    /// Assess literal symbols and, for alphabets larger than two, their raw bits.
    #[default]
    Initial,
    /// Assess only the raw bitstring. The report also gives bits per input symbol.
    Conditioned,
}

#[derive(Clone, Copy, Debug)]
pub struct NonIidOptions {
    pub precision_bits: u32,
    pub confidence_constant: ConfidenceConstant,
    /// 1..=8, or None to infer the width from the highest bit in use (minimum 1).
    pub bits_per_symbol: Option<u8>,
    pub mode: NonIidMode,
    /// None uses all bits; Some(1_000_000) matches NIST's -t flag.
    /// Other limits >=2 are supported. Literal samples are never truncated.
    pub bitstring_limit: Option<usize>,
}
impl Default for NonIidOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
            confidence_constant: ConfidenceConstant::NistReference,
            bits_per_symbol: None,
            mode: NonIidMode::Initial,
            bitstring_limit: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssessmentError {
    TooFewSamples { count: usize },
    InvalidPrecision { bits: u32 },
    InvalidBitsPerSymbol { bits: u8 },
    SymbolOutOfRange { index: usize, value: u8, bits: u8 },
    InvalidBitstringLimit { limit: usize },
    InputTooLarge,
}
impl fmt::Display for AssessmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewSamples { count } => {
                write!(f, "at least two input symbols are required; got {count}")
            }
            Self::InvalidPrecision { bits } => {
                write!(f, "precision {bits} is outside 64..={}", float::prec_max())
            }
            Self::InvalidBitsPerSymbol { bits } => {
                write!(f, "bits per symbol must be in 1..=8; got {bits}")
            }
            Self::SymbolOutOfRange { index, value, bits } => write!(
                f,
                "sample {index} has value {value}, which does not fit in {bits} bits"
            ),
            Self::InvalidBitstringLimit { limit } => {
                write!(f, "bitstring limit must be at least two; got {limit}")
            }
            Self::InputTooLarge => write!(f, "expanded bit count exceeds the supported input size"),
        }
    }
}
impl Error for AssessmentError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssessmentWarning {
    /// NIST recommends at least one million input samples for this assessment.
    FewerThanRecommendedSamples {
        count: usize,
    },
    DeclaredWidthExceedsObserved {
        declared: u8,
        observed: u8,
    },
    BitstringTruncated {
        available: usize,
        used: usize,
    },
    /// Like NIST, award no entropy to a single-symbol source. Estimators are bypassed.
    ConstantInput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstimatorKind {
    MostCommonValue,
    Collision,
    Markov,
    Compression,
    TTuple,
    Lrs,
    MultiMcw,
    Lag,
    MultiMmc,
    Lz78y,
}
/// Full estimator diagnostics; no conversion to binary64 is performed.
#[derive(Clone, Debug)]
pub enum EstimatorDetails {
    MostCommonValue(Box<McvEstimate>),
    Collision(Box<CollisionEstimate>),
    Markov(Box<MarkovEstimate>),
    Compression(Box<CompressionEstimate>),
    TTuple(Box<TTupleEstimate>),
    Lrs(Box<LrsEstimate>),
    MultiMcw(Box<MultiMcwEstimate>),
    Lag(Box<LagEstimate>),
    MultiMmc(Box<MultiMmcEstimate>),
    Lz78y(Box<Lz78yEstimate>),
}
impl EstimatorDetails {
    pub fn min_entropy(&self) -> &Float {
        match self {
            Self::MostCommonValue(x) => &x.min_entropy,
            Self::Collision(x) => &x.min_entropy,
            Self::Markov(x) => &x.min_entropy,
            Self::Compression(x) => &x.min_entropy,
            Self::TTuple(x) => &x.min_entropy,
            Self::Lrs(x) => &x.min_entropy,
            Self::MultiMcw(x) => &x.min_entropy,
            Self::Lag(x) => &x.min_entropy,
            Self::MultiMmc(x) => &x.min_entropy,
            Self::Lz78y(x) => &x.min_entropy,
        }
    }
}
#[derive(Clone, Debug)]
pub enum EstimatorOutcome {
    Available(EstimatorDetails),
    /// Collision, Markov, and Compression are not applied to nonbinary symbols.
    NotApplicable,
    /// Not included in the minimum; retained so an incomplete run is visible.
    Unavailable(EstimateError),
}
#[derive(Clone, Debug)]
pub struct EstimatorResult {
    pub estimator: EstimatorKind,
    pub outcome: EstimatorOutcome,
}
#[derive(Clone, Debug)]
pub struct NonIidDatasetAssessment {
    /// Number of symbols (literal branch) or bits (bitstring branch) assessed.
    pub sample_count: usize,
    /// Results in specification order, including unavailable and inapplicable tests.
    pub estimators: Vec<EstimatorResult>,
    /// Minimum available estimate, in bits per symbol of this dataset.
    /// Check is_complete() before treating this as a completed estimator run.
    pub min_entropy: Float,
}
impl NonIidDatasetAssessment {
    /// Look up an estimator by kind without relying on its position in the report.
    pub fn estimator(&self, kind: EstimatorKind) -> Option<&EstimatorResult> {
        self.estimators.iter().find(|r| r.estimator == kind)
    }
    /// Completed estimate in bits per dataset symbol (or bit for a bitstring).
    /// Returns `None` if any applicable estimator was unavailable.
    pub fn eligible_entropy(&self) -> Option<&Float> {
        self.is_complete().then_some(&self.min_entropy)
    }
    pub fn is_complete(&self) -> bool {
        !self
            .estimators
            .iter()
            .any(|r| matches!(r.outcome, EstimatorOutcome::Unavailable(_)))
    }
}
#[derive(Clone, Debug)]
pub struct NonIidAssessment {
    pub options: NonIidOptions,
    pub sample_count: usize,
    pub bits_per_symbol: u8,
    /// Original symbols in increasing order; index is the remapped literal value.
    pub alphabet: Vec<u8>,
    /// Absent in conditioned mode, or when constant input is short-circuited.
    pub literal: Option<NonIidDatasetAssessment>,
    /// Absent for initial two-symbol input, or when constant input is short-circuited.
    pub bitstring: Option<NonIidDatasetAssessment>,
    /// Final entropy in bits per original input symbol, also in conditioned mode.
    /// The conditioned per-bit estimate h' is bitstring.min_entropy.
    pub min_entropy: Float,
    pub warnings: Vec<AssessmentWarning>,
}
impl NonIidAssessment {
    /// Completed estimate in bits per original input symbol.
    ///
    /// Unlike the diagnostic `min_entropy` field, this withholds a partial
    /// minimum when an applicable estimator failed. Small-sample warnings still
    /// apply; eligibility here does not imply source validation.
    pub fn eligible_entropy(&self) -> Option<&Float> {
        self.is_complete().then_some(&self.min_entropy)
    }
    /// True if no applicable estimator was unavailable. Constant input is a
    /// complete zero-entropy outcome. This does not certify an entropy source
    /// or remove the minimum-sample warning.
    pub fn is_complete(&self) -> bool {
        self.literal
            .as_ref()
            .is_none_or(NonIidDatasetAssessment::is_complete)
            && self
                .bitstring
                .as_ref()
                .is_none_or(NonIidDatasetAssessment::is_complete)
    }
}

pub(crate) struct Prepared {
    pub(crate) width: u8,
    pub(crate) alphabet: Vec<u8>,
    pub(crate) literal: Option<Vec<u8>>,
    pub(crate) bits: Option<Vec<u8>>,
    pub(crate) warnings: Vec<AssessmentWarning>,
}
pub(crate) fn prepare(samples: &[u8], options: NonIidOptions) -> Result<Prepared, AssessmentError> {
    if !(64..=float::prec_max()).contains(&options.precision_bits) {
        return Err(AssessmentError::InvalidPrecision {
            bits: options.precision_bits,
        });
    }
    if let Some(bits) = options.bits_per_symbol {
        if !(1..=8).contains(&bits) {
            return Err(AssessmentError::InvalidBitsPerSymbol { bits });
        }
    }
    if let Some(limit) = options.bitstring_limit {
        if limit < 2 {
            return Err(AssessmentError::InvalidBitstringLimit { limit });
        }
    }
    if samples.len() < 2 {
        return Err(AssessmentError::TooFewSamples {
            count: samples.len(),
        });
    }
    let observed = (8 - samples.iter().copied().max().unwrap().leading_zeros()) as u8;
    let width = options.bits_per_symbol.unwrap_or(observed.max(1));
    let mut seen = [false; 256];
    for (index, &value) in samples.iter().enumerate() {
        if u16::from(value) >= (1u16 << width) {
            return Err(AssessmentError::SymbolOutOfRange {
                index,
                value,
                bits: width,
            });
        }
        seen[value as usize] = true;
    }
    let alphabet: Vec<u8> = (0..=255u8).filter(|&s| seen[s as usize]).collect();
    let mut warnings = Vec::new();
    if samples.len() < 1_000_000 {
        warnings.push(AssessmentWarning::FewerThanRecommendedSamples {
            count: samples.len(),
        });
    }
    if options.bits_per_symbol.is_some() && width > observed {
        warnings.push(AssessmentWarning::DeclaredWidthExceedsObserved {
            declared: width,
            observed,
        });
    }
    if alphabet.len() == 1 {
        warnings.push(AssessmentWarning::ConstantInput);
        return Ok(Prepared {
            width,
            alphabet,
            literal: None,
            bits: None,
            warnings,
        });
    }
    // Expand ORIGINAL symbols, MSB first, before remapping literal symbols.
    let bits = if alphabet.len() > 2 || options.mode == NonIidMode::Conditioned {
        let available = samples
            .len()
            .checked_mul(width as usize)
            .ok_or(AssessmentError::InputTooLarge)?;
        let used = options
            .bitstring_limit
            .map_or(available, |limit| limit.min(available));
        if used < available {
            warnings.push(AssessmentWarning::BitstringTruncated { available, used });
        }
        Some(
            samples
                .iter()
                .flat_map(|&s| (0..width).rev().map(move |shift| (s >> shift) & 1))
                .take(used)
                .collect(),
        )
    } else {
        None
    };
    let literal = if options.mode == NonIidMode::Initial {
        let mut map = [0u8; 256];
        for (index, &symbol) in alphabet.iter().enumerate() {
            map[symbol as usize] = index as u8;
        }
        Some(samples.iter().map(|&s| map[s as usize]).collect())
    } else {
        None
    };
    Ok(Prepared {
        width,
        alphabet,
        literal,
        bits,
        warnings,
    })
}

pub(crate) fn assess_dataset(
    samples: &[u8],
    binary: bool,
    ceiling: u8,
    options: NonIidOptions,
) -> NonIidDatasetAssessment {
    let precision_bits = options.precision_bits;
    let confidence_constant = options.confidence_constant;
    let common = McvOptions {
        precision_bits,
        confidence_constant,
    };
    let mut estimators = Vec::with_capacity(10);
    macro_rules! add {
        ($kind:ident,$result:expr) => {
            estimators.push(EstimatorResult {
                estimator: EstimatorKind::$kind,
                outcome: match $result {
                    Ok(value) => {
                        EstimatorOutcome::Available(EstimatorDetails::$kind(Box::new(value)))
                    }
                    Err(error) => EstimatorOutcome::Unavailable(error),
                },
            });
        };
    }
    add!(
        MostCommonValue,
        most_common_value_with_options(samples, common)
    );
    if binary {
        add!(Collision, collision_estimate_with_options(samples, common));
        add!(
            Markov,
            markov_estimate_with_options(samples, MarkovOptions { precision_bits })
        );
        add!(
            Compression,
            compression_estimate_with_options(
                samples,
                CompressionOptions {
                    precision_bits,
                    confidence_constant,
                    ..Default::default()
                }
            )
        );
    } else {
        for estimator in [
            EstimatorKind::Collision,
            EstimatorKind::Markov,
            EstimatorKind::Compression,
        ] {
            estimators.push(EstimatorResult {
                estimator,
                outcome: EstimatorOutcome::NotApplicable,
            });
        }
    }
    add!(
        TTuple,
        t_tuple_estimate_with_options(
            samples,
            TTupleOptions {
                precision_bits,
                confidence_constant,
                ..Default::default()
            }
        )
    );
    add!(
        Lrs,
        lrs_estimate_with_options(
            samples,
            LrsOptions {
                precision_bits,
                confidence_constant,
                ..Default::default()
            }
        )
    );
    add!(
        MultiMcw,
        multi_mcw_estimate_with_options(
            samples,
            MultiMcwOptions {
                precision_bits,
                confidence_constant,
                ..Default::default()
            }
        )
    );
    add!(
        Lag,
        lag_estimate_with_options(
            samples,
            LagOptions {
                precision_bits,
                confidence_constant,
                ..Default::default()
            }
        )
    );
    add!(
        MultiMmc,
        multi_mmc_estimate_with_options(
            samples,
            MultiMmcOptions {
                precision_bits,
                confidence_constant,
                ..Default::default()
            }
        )
    );
    add!(
        Lz78y,
        lz78y_estimate_with_options(
            samples,
            Lz78yOptions {
                precision_bits,
                confidence_constant,
                ..Default::default()
            }
        )
    );
    let mut min_entropy = Float::with_val(precision_bits, ceiling);
    for result in &estimators {
        if let EstimatorOutcome::Available(details) = &result.outcome {
            if details.min_entropy() < &min_entropy {
                min_entropy = details.min_entropy().clone();
            }
        }
    }
    NonIidDatasetAssessment {
        sample_count: samples.len(),
        estimators,
        min_entropy,
    }
}

/// Run all applicable non-IID estimators with standard parameters.
///
/// Initial mode combines H_original with bits_per_symbol * H_bitstring.
/// Conditioned mode assesses only raw bits; its per-bit result h' is available
/// separately. Two-symbol initial data is mapped to 0/1 and assessed once.
/// Constant input returns zero without running estimators, matching NIST's
/// no-entropy policy (as a structured result rather than a process exit).
///
/// Unavailable estimators are excluded from the minimum and retained as errors;
/// inspect is_complete() and warnings. No IID/restart/health tests are performed.
///
/// ```
/// use sp_800_90b::{assess_non_iid, NonIidOptions};
/// let report = assess_non_iid(&[7; 100], NonIidOptions::default()).unwrap();
/// assert_eq!(report.min_entropy, 0);
/// assert!(report.is_complete());
/// ```
pub fn assess_non_iid(
    samples: &[u8],
    options: NonIidOptions,
) -> Result<NonIidAssessment, AssessmentError> {
    let prepared = prepare(samples, options)?;
    let literal = prepared
        .literal
        .as_ref()
        .map(|data| assess_dataset(data, prepared.alphabet.len() == 2, prepared.width, options));
    let bitstring = prepared
        .bits
        .as_ref()
        .map(|data| assess_dataset(data, true, 1, options));
    let mut min_entropy = Float::with_val(options.precision_bits, prepared.width);
    if prepared.alphabet.len() == 1 {
        min_entropy = Float::with_val(options.precision_bits, 0);
    }
    if let Some(result) = &literal {
        if result.min_entropy < min_entropy {
            min_entropy = result.min_entropy.clone();
        }
    }
    if let Some(result) = &bitstring {
        let per_symbol =
            Float::with_val(options.precision_bits, &result.min_entropy * prepared.width);
        if per_symbol < min_entropy {
            min_entropy = per_symbol;
        }
    }
    Ok(NonIidAssessment {
        options,
        sample_count: samples.len(),
        bits_per_symbol: prepared.width,
        alphabet: prepared.alphabet,
        literal,
        bitstring,
        min_entropy,
        warnings: prepared.warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_bits_precede_rank_mapping_and_padding() {
        let data = prepare(&[1, 4, 7], NonIidOptions::default()).unwrap();
        assert_eq!(data.width, 3);
        assert_eq!(data.alphabet, [1, 4, 7]);
        assert_eq!(data.literal.unwrap(), [0, 1, 2]);
        assert_eq!(data.bits.unwrap(), [0, 0, 1, 1, 0, 0, 1, 1, 1]);
        let data = prepare(
            &[1, 4, 7],
            NonIidOptions {
                bits_per_symbol: Some(4),
                bitstring_limit: Some(6),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(data.literal.unwrap(), [0, 1, 2]);
        assert_eq!(data.bits.unwrap(), [0, 0, 0, 1, 0, 1]);
        assert!(
            data.warnings
                .contains(&AssessmentWarning::BitstringTruncated {
                    available: 12,
                    used: 6
                })
        );
    }
    #[test]
    fn binary_initial_is_mapped_and_not_truncated() {
        let data = prepare(
            &[255, 7, 255, 7],
            NonIidOptions {
                bitstring_limit: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(data.width, 8);
        assert_eq!(data.literal.unwrap(), [1, 0, 1, 0]);
        assert!(data.bits.is_none());
        assert!(
            !data
                .warnings
                .iter()
                .any(|w| matches!(w, AssessmentWarning::BitstringTruncated { .. }))
        );
        let data = prepare(
            &[7, 255],
            NonIidOptions {
                mode: NonIidMode::Conditioned,
                bitstring_limit: Some(5),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(data.literal.is_none());
        assert_eq!(data.bits.unwrap(), [0, 0, 0, 0, 0]);
    }
    #[test]
    fn million_bit_limit_and_full_byte_alphabet() {
        let samples: Vec<u8> = (0..=255).cycle().take(125_001).collect();
        let data = prepare(
            &samples,
            NonIidOptions {
                bitstring_limit: Some(1_000_000),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(data.bits.unwrap().len(), 1_000_000);
        assert_eq!(data.literal.unwrap(), samples);
        assert_eq!(data.alphabet, (0..=255).collect::<Vec<_>>());
    }
}
