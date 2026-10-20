// Reference-informed adaptation of NIST SP800-90B_EntropyAssessment.
// Modified in Rust, 2026-10-19–2026-10-20; see docs/provenance.md for sources
// and changes. Preserve the upstream notice in LICENSE-NIST (also in LICENSE).
//! Combined IID assessment and the §5.2.5 LRS hypothesis test.
use crate::*;
use rug::{Float, float, ops::Pow};

/// Initial versus conditioned entropy estimation, with the same preprocessing
/// rules as the non-IID API. Hypothesis tests always use the literal samples.
pub type IidMode = NonIidMode;

#[derive(Clone, Copy, Debug)]
pub struct IidOptions {
    pub precision_bits: u32,
    pub confidence_constant: ConfidenceConstant,
    pub bits_per_symbol: Option<u8>,
    pub mode: IidMode,
    /// Limits only the bitstring MCV input; never the IID hypothesis tests.
    pub bitstring_limit: Option<usize>,
    pub chi_square_grouping: ChiSquareGrouping,
    pub permutation: PermutationOptions,
}
impl Default for IidOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
            confidence_constant: ConfidenceConstant::NistReference,
            bits_per_symbol: None,
            mode: IidMode::Initial,
            bitstring_limit: None,
            chi_square_grouping: ChiSquareGrouping::NistReference,
            permutation: PermutationOptions::default(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct IidLrsTest {
    pub sample_count: usize,
    pub longest_repeat: usize,
    pub substring_pairs: u128,
    pub collision_probability: Float,
    /// NIST's probability of at least one match of length longest_repeat.
    pub p_value: Float,
    pub reject: bool,
}
/// The IID LRS hypothesis test, distinct from the §6.3.6 entropy estimator.
/// Uses high-precision log1p/expm1 to avoid cancellation in small probabilities.
pub fn iid_lrs_test(samples: &[u8], precision_bits: u32) -> Result<IidLrsTest, EstimateError> {
    if !(64..=float::prec_max()).contains(&precision_bits) {
        return Err(EstimateError::InvalidPrecision {
            bits: precision_bits,
        });
    }
    let n = samples.len();
    if n < 2 {
        return Err(EstimateError::TooFewSamples { count: n });
    }
    let mut counts = [0usize; 256];
    for &s in samples {
        counts[s as usize] += 1;
    }
    let sum: u128 = counts.iter().map(|&c| c as u128 * c as u128).sum();
    let collision_probability = Float::with_val(
        precision_bits,
        Float::with_val(precision_bits, sum)
            / Float::with_val(precision_bits, n as u128 * n as u128),
    );
    let (counts, _) = crate::tuples::tuple_counts(samples, false);
    let longest_repeat = counts.iter().rposition(|&count| count >= 2).unwrap_or(0);
    let substrings = (n - longest_repeat + 1) as u128;
    let substring_pairs = substrings * (substrings - 1) / 2;
    let power = collision_probability.clone().pow(longest_repeat as u64);
    let mut log_none = (-power).ln_1p();
    log_none *= Float::with_val(precision_bits, substring_pairs);
    let p_value = -log_none.exp_m1();
    let reject = p_value < Float::with_val(precision_bits, Float::parse("0.001").unwrap());
    Ok(IidLrsTest {
        sample_count: n,
        longest_repeat,
        substring_pairs,
        collision_probability,
        p_value,
        reject,
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IidDecision {
    /// All checks completed without rejecting. Does not establish source validation.
    NotRejected,
    /// At least one check rejected, or input was constant.
    Rejected,
    /// No observed rejection, but at least one check could not run.
    Incomplete,
}
#[derive(Debug)]
pub struct IidChecks {
    pub independence: Result<ChiSquareIndependence, ChiSquareError>,
    pub goodness_of_fit: Result<ChiSquareGoodnessOfFit, ChiSquareError>,
    pub longest_repeated_substring: Result<IidLrsTest, EstimateError>,
    pub permutation: Result<PermutationAssessment, PermutationError>,
}
impl IidChecks {
    pub fn is_complete(&self) -> bool {
        self.independence.is_ok()
            && self.goodness_of_fit.is_ok()
            && self.longest_repeated_substring.is_ok()
            && self.permutation.is_ok()
    }
    /// Whether any successfully executed check rejected IID.
    ///
    /// `false` does not imply that all checks ran: inspect `is_complete()` too.
    pub fn rejected(&self) -> bool {
        self.independence
            .as_ref()
            .is_ok_and(|r| r.reject_independence)
            || self
                .goodness_of_fit
                .as_ref()
                .is_ok_and(|r| r.reject_goodness_of_fit)
            || self
                .longest_repeated_substring
                .as_ref()
                .is_ok_and(|r| r.reject)
            || self.permutation.as_ref().is_ok_and(|r| r.reject)
    }
}
#[derive(Debug)]
pub struct IidAssessment {
    pub options: IidOptions,
    pub sample_count: usize,
    pub bits_per_symbol: u8,
    pub alphabet: Vec<u8>,
    /// Absent in conditioned mode and for constant input.
    pub literal: Option<McvEstimate>,
    /// Absent for initial binary input and for constant input.
    pub bitstring: Option<McvEstimate>,
    /// MCV-derived candidate, in bits per original sample, even if IID rejects.
    pub candidate_min_entropy: Float,
    /// Available only when all IID checks completed without rejection.
    pub min_entropy: Option<Float>,
    /// Constant input short-circuits with zero candidate entropy and no checks.
    pub checks: Option<IidChecks>,
    pub decision: IidDecision,
    pub warnings: Vec<AssessmentWarning>,
}
impl IidAssessment {
    /// Entropy in bits per original input symbol, only for complete,
    /// non-rejected IID evidence. Does not establish source validation.
    pub fn eligible_entropy(&self) -> Option<&Float> {
        (self.decision == IidDecision::NotRejected && self.is_complete())
            .then_some(self.min_entropy.as_ref())
            .flatten()
    }
    pub fn is_complete(&self) -> bool {
        self.checks.as_ref().is_some_and(IidChecks::is_complete)
    }
}
/// Assess MCV entropy and all four IID checks. Individual check failures remain
/// in the report; they are never treated as statistical passes. This does not
/// automatically run the non-IID estimators when an IID check rejects.
pub fn assess_iid(samples: &[u8], options: IidOptions) -> Result<IidAssessment, AssessmentError> {
    assess_iid_with_progress(samples, options, &mut |_| {})
}
/// IID assessment with live permutation progress; results are unchanged.
pub fn assess_iid_with_progress(
    samples: &[u8],
    options: IidOptions,
    observer: &mut dyn FnMut(PermutationProgress),
) -> Result<IidAssessment, AssessmentError> {
    let prepared = crate::non_iid::prepare(
        samples,
        NonIidOptions {
            precision_bits: options.precision_bits,
            confidence_constant: options.confidence_constant,
            bits_per_symbol: options.bits_per_symbol,
            mode: options.mode,
            bitstring_limit: options.bitstring_limit,
        },
    )?;
    let common = McvOptions {
        precision_bits: options.precision_bits,
        confidence_constant: options.confidence_constant,
    };
    // Preprocessing guarantees valid precision and >=2 samples in each branch.
    let literal = prepared
        .literal
        .as_ref()
        .map(|s| most_common_value_with_options(s, common).expect("validated MCV input"));
    let bitstring = prepared
        .bits
        .as_ref()
        .map(|s| most_common_value_with_options(s, common).expect("validated MCV input"));
    let constant = prepared.alphabet.len() == 1;
    let mut candidate_min_entropy = Float::with_val(
        options.precision_bits,
        if constant { 0 } else { prepared.width },
    );
    if let Some(ref r) = literal {
        candidate_min_entropy = candidate_min_entropy.min(&r.min_entropy);
    }
    if let Some(ref r) = bitstring {
        candidate_min_entropy = candidate_min_entropy.min(&Float::with_val(
            options.precision_bits,
            &r.min_entropy * prepared.width,
        ));
    }
    let checks = if constant {
        None
    } else {
        let chi = ChiSquareOptions {
            precision_bits: options.precision_bits,
            grouping: options.chi_square_grouping,
        };
        Some(IidChecks {
            independence: chi_square_independence_with_options(samples, chi),
            goodness_of_fit: chi_square_goodness_of_fit_with_options(samples, chi),
            longest_repeated_substring: iid_lrs_test(samples, options.precision_bits),
            permutation: permutation_tests_with_progress(samples, options.permutation, observer),
        })
    };
    let decision = if constant || checks.as_ref().is_some_and(IidChecks::rejected) {
        IidDecision::Rejected
    } else if checks.as_ref().is_some_and(IidChecks::is_complete) {
        IidDecision::NotRejected
    } else {
        IidDecision::Incomplete
    };
    let min_entropy = (decision == IidDecision::NotRejected).then(|| candidate_min_entropy.clone());
    Ok(IidAssessment {
        options,
        sample_count: samples.len(),
        bits_per_symbol: prepared.width,
        alphabet: prepared.alphabet,
        literal,
        bitstring,
        candidate_min_entropy,
        min_entropy,
        checks,
        decision,
        warnings: prepared.warnings,
    })
}
