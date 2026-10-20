// Reference-informed adaptation of NIST SP800-90B_EntropyAssessment.
// Modified in Rust, 2026-10-19–2026-10-20; see docs/provenance.md for sources
// and changes. Preserve the upstream notice in LICENSE-NIST (also in LICENSE).
//! Restart testing (§3.1.4): fixed 1000-by-1000 data, simulated sanity cutoff,
//! literal row/column estimates, and H_I/2 validation.
use crate::*;
use rug::{Float, float};
use std::{error::Error, fmt};
pub const RESTART_SAMPLES: usize = 1_000_000;
pub const RESTART_SIMULATIONS: usize = 5_000_000;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RestartMode {
    #[default]
    NonIid,
    Iid,
}
#[derive(Clone, Copy, Debug)]
pub struct RestartOptions {
    pub mode: RestartMode,
    pub bits_per_symbol: Option<u8>,
    pub precision_bits: u32,
    pub confidence_constant: ConfidenceConstant,
    /// At least 5,000,000, matching the reference CLI's minimum.
    pub simulation_rounds: usize,
    pub simulation_seed: Option<[u64; 4]>,
    pub chi_square_grouping: ChiSquareGrouping,
    pub permutation: PermutationOptions,
}
impl Default for RestartOptions {
    fn default() -> Self {
        Self {
            mode: RestartMode::NonIid,
            bits_per_symbol: None,
            precision_bits: 256,
            confidence_constant: ConfidenceConstant::NistReference,
            simulation_rounds: RESTART_SIMULATIONS,
            simulation_seed: None,
            chi_square_grouping: ChiSquareGrouping::NistReference,
            permutation: PermutationOptions::default(),
        }
    }
}
#[derive(Debug)]
pub enum RestartError {
    InvalidSampleCount { count: usize },
    Input(AssessmentError),
    ConstantInput,
    InvalidInitialEntropy,
    EntropyExceedsAlphabet,
    TooFewSimulations { count: usize },
    ZeroSeed,
    Randomness(String),
}
impl fmt::Display for RestartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSampleCount { count } => write!(
                f,
                "restart data must contain exactly 1,000,000 samples; got {count}"
            ),
            Self::Input(e) => write!(f, "{e}"),
            Self::ConstantInput => write!(f, "constant restart input cannot be validated"),
            Self::InvalidInitialEntropy => write!(
                f,
                "initial entropy must be finite, nonnegative, and no greater than the symbol width"
            ),
            Self::EntropyExceedsAlphabet => write!(
                f,
                "initial entropy requires more symbols than the observed alphabet"
            ),
            Self::TooFewSimulations { count } => write!(
                f,
                "at least 5,000,000 sanity simulations required; got {count}"
            ),
            Self::ZeroSeed => write!(f, "simulation seed must not be all zero"),
            Self::Randomness(e) => write!(f, "OS randomness failed: {e}"),
        }
    }
}
impl Error for RestartError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Clone, Debug)]
pub struct RestartSanity {
    pub seed: [u64; 4],
    pub simulation_rounds: usize,
    /// Binary64 probability and alpha used by the reference simulation.
    pub max_symbol_probability: f64,
    pub alpha: f64,
    pub effective_alphabet: usize,
    /// Empirical quantile; equality passes.
    pub cutoff: usize,
    pub row_maxima: Vec<usize>,
    pub column_maxima: Vec<usize>,
    pub maximum_row_count: usize,
    pub maximum_column_count: usize,
    pub reject: bool,
}
#[derive(Debug)]
pub enum RestartDatasetAssessment {
    NonIid(Box<NonIidDatasetAssessment>),
    Iid {
        estimate: Box<McvEstimate>,
        checks: Box<IidChecks>,
    },
}
impl RestartDatasetAssessment {
    /// Entropy in bits per raw sample, only if all applicable estimates/checks
    /// completed and IID was not rejected. `min_entropy()` remains diagnostic.
    pub fn eligible_entropy(&self) -> Option<&Float> {
        (self.is_complete() && !self.iid_rejected()).then(|| self.min_entropy())
    }
    pub fn min_entropy(&self) -> &Float {
        match self {
            Self::NonIid(r) => &r.min_entropy,
            Self::Iid { estimate, .. } => &estimate.min_entropy,
        }
    }
    pub fn is_complete(&self) -> bool {
        match self {
            Self::NonIid(r) => r.is_complete(),
            Self::Iid { checks, .. } => checks.is_complete(),
        }
    }
    fn iid_rejected(&self) -> bool {
        matches!(self,Self::Iid{checks,..} if checks.rejected())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestartDecision {
    Validated,
    SanityRejected,
    EntropyRejected,
    IidRejected,
    Incomplete,
}
#[derive(Debug)]
pub struct RestartAssessment {
    pub options: RestartOptions,
    pub bits_per_symbol: u8,
    pub alphabet: Vec<u8>,
    pub initial_entropy: Float,
    pub sanity: RestartSanity,
    /// Absent when sanity rejects (the remaining tests are not run).
    pub rows: Option<RestartDatasetAssessment>,
    pub columns: Option<RestartDatasetAssessment>,
    pub decision: RestartDecision,
    /// min(H_I,H_r,H_c), available only after complete successful validation.
    pub min_entropy: Option<Float>,
}
impl RestartAssessment {
    /// Final restart entropy in bits per raw sample, only after validation.
    pub fn eligible_entropy(&self) -> Option<&Float> {
        (self.decision == RestartDecision::Validated)
            .then_some(self.min_entropy.as_ref())
            .flatten()
    }
}
fn prepare_restart(
    samples: &[u8],
    h: &Float,
    o: RestartOptions,
) -> Result<(u8, Vec<u8>), RestartError> {
    if samples.len() != RESTART_SAMPLES {
        return Err(RestartError::InvalidSampleCount {
            count: samples.len(),
        });
    }
    if !(64..=float::prec_max()).contains(&o.precision_bits) {
        return Err(RestartError::Input(AssessmentError::InvalidPrecision {
            bits: o.precision_bits,
        }));
    }
    let observed = (8 - samples.iter().copied().max().unwrap().leading_zeros()) as u8;
    let width = o.bits_per_symbol.unwrap_or(observed.max(1));
    if !(1..=8).contains(&width) {
        return Err(RestartError::Input(AssessmentError::InvalidBitsPerSymbol {
            bits: width,
        }));
    }
    let mut seen = [false; 256];
    for (index, &value) in samples.iter().enumerate() {
        if u16::from(value) >= (1u16 << width) {
            return Err(RestartError::Input(AssessmentError::SymbolOutOfRange {
                index,
                value,
                bits: width,
            }));
        }
        seen[value as usize] = true;
    }
    let alphabet: Vec<u8> = (0..=255).filter(|&s| seen[s as usize]).collect();
    if alphabet.len() < 2 {
        return Err(RestartError::ConstantInput);
    }
    if !h.is_finite() || h < &0 || h > &width {
        return Err(RestartError::InvalidInitialEntropy);
    }
    let p = 2f64.powf(-h.to_f64());
    if (1.0 / p).ceil() as usize > alphabet.len() {
        return Err(RestartError::EntropyExceedsAlphabet);
    }
    if o.simulation_rounds < RESTART_SIMULATIONS {
        return Err(RestartError::TooFewSimulations {
            count: o.simulation_rounds,
        });
    }
    if o.simulation_seed == Some([0; 4]) {
        return Err(RestartError::ZeroSeed);
    }
    Ok((width, alphabet))
}
fn seed(requested: Option<[u64; 4]>) -> Result<[u64; 4], RestartError> {
    let seed = if let Some(s) = requested {
        s
    } else {
        let mut b = [0u8; 32];
        getrandom::fill(&mut b).map_err(|e| RestartError::Randomness(e.to_string()))?;
        std::array::from_fn(|i| u64::from_le_bytes(b[8 * i..8 * i + 8].try_into().unwrap()))
    };
    if seed == [0; 4] {
        Err(RestartError::ZeroSeed)
    } else {
        Ok(seed)
    }
}
fn simulate(p: f64, rounds: usize, seed: [u64; 4]) -> usize {
    if p == 1.0 {
        return 1000;
    }
    let mut rng = crate::permutation::Random(seed);
    let mut histogram = [0usize; 1001];
    let k = (1.0 / p).ceil() as usize;
    for _ in 0..rounds {
        let mut counts = [0u16; 256];
        for _ in 0..1000 {
            let unit = (rng.next() >> 11) as f64 * (1.0 / 9007199254740992.0);
            counts[(unit / p).floor() as usize] += 1;
        }
        histogram[*counts[..k].iter().max().unwrap() as usize] += 1;
    }
    let alpha = 1.0 - (0.99f64.ln() / 2000.0).exp();
    let index = ((1.0 - alpha) * rounds as f64).floor() as usize - 1;
    let mut cumulative = 0;
    for (count, n) in histogram.into_iter().enumerate() {
        cumulative += n;
        if cumulative > index {
            return count;
        }
    }
    unreachable!("quantile is within histogram")
}
/// Transpose the 1000-by-1000 row-major restart matrix to column-major order.
pub fn restart_column_sequence(samples: &[u8]) -> Result<Vec<u8>, RestartError> {
    if samples.len() != RESTART_SAMPLES {
        return Err(RestartError::InvalidSampleCount {
            count: samples.len(),
        });
    }
    let mut out = vec![0; RESTART_SAMPLES];
    for row in 0..1000 {
        for col in 0..1000 {
            out[col * 1000 + row] = samples[row * 1000 + col];
        }
    }
    Ok(out)
}
fn maxima(samples: &[u8]) -> Vec<usize> {
    samples
        .chunks_exact(1000)
        .map(|row| {
            let mut counts = [0usize; 256];
            for &s in row {
                counts[s as usize] += 1;
            }
            *counts.iter().max().unwrap()
        })
        .collect()
}
fn sanity(
    samples: &[u8],
    columns: &[u8],
    h: &Float,
    o: RestartOptions,
) -> Result<RestartSanity, RestartError> {
    let seed = seed(o.simulation_seed)?;
    let p = 2f64.powf(-h.to_f64());
    let cutoff = simulate(p, o.simulation_rounds, seed);
    let row_maxima = maxima(samples);
    let column_maxima = maxima(columns);
    let maximum_row_count = *row_maxima.iter().max().unwrap();
    let maximum_column_count = *column_maxima.iter().max().unwrap();
    Ok(RestartSanity {
        seed,
        simulation_rounds: o.simulation_rounds,
        max_symbol_probability: p,
        alpha: 1.0 - (0.99f64.ln() / 2000.0).exp(),
        effective_alphabet: (1.0 / p).ceil() as usize,
        cutoff,
        row_maxima,
        column_maxima,
        maximum_row_count,
        maximum_column_count,
        reject: maximum_row_count.max(maximum_column_count) > cutoff,
    })
}
/// Run just the simulated sanity check, retaining the cutoff and all row/column maxima.
pub fn restart_sanity_check(
    samples: &[u8],
    initial_entropy: &Float,
    options: RestartOptions,
) -> Result<RestartSanity, RestartError> {
    prepare_restart(samples, initial_entropy, options)?;
    sanity(
        samples,
        &restart_column_sequence(samples)?,
        initial_entropy,
        options,
    )
}
fn dataset(raw: &[u8], alphabet: &[u8], width: u8, o: RestartOptions) -> RestartDatasetAssessment {
    let mut map = [0u8; 256];
    for (i, &s) in alphabet.iter().enumerate() {
        map[s as usize] = i as u8;
    }
    let mapped: Vec<u8> = raw.iter().map(|&s| map[s as usize]).collect();
    if o.mode == RestartMode::NonIid {
        RestartDatasetAssessment::NonIid(Box::new(crate::non_iid::assess_dataset(
            &mapped,
            alphabet.len() == 2,
            width,
            NonIidOptions {
                precision_bits: o.precision_bits,
                confidence_constant: o.confidence_constant,
                ..Default::default()
            },
        )))
    } else {
        let estimate = most_common_value_with_options(
            &mapped,
            McvOptions {
                precision_bits: o.precision_bits,
                confidence_constant: o.confidence_constant,
            },
        )
        .expect("validated restart input");
        let chi = ChiSquareOptions {
            precision_bits: o.precision_bits,
            grouping: o.chi_square_grouping,
        };
        let checks = IidChecks {
            independence: chi_square_independence_with_options(raw, chi),
            goodness_of_fit: chi_square_goodness_of_fit_with_options(raw, chi),
            longest_repeated_substring: iid_lrs_test(raw, o.precision_bits),
            permutation: permutation_tests_with_options(raw, o.permutation),
        };
        RestartDatasetAssessment::Iid {
            estimate: Box::new(estimate),
            checks: Box::new(checks),
        }
    }
}
fn validation(
    rows: &RestartDatasetAssessment,
    columns: &RestartDatasetAssessment,
    h: &Float,
    precision: u32,
) -> (RestartDecision, Option<Float>) {
    let threshold = Float::with_val(precision, h / 2);
    let decision = if rows.iid_rejected() || columns.iid_rejected() {
        RestartDecision::IidRejected
    } else if rows.min_entropy() < &threshold || columns.min_entropy() < &threshold {
        RestartDecision::EntropyRejected
    } else if !rows.is_complete() || !columns.is_complete() {
        RestartDecision::Incomplete
    } else {
        RestartDecision::Validated
    };
    let entropy = (decision == RestartDecision::Validated)
        .then(|| h.clone().min(rows.min_entropy()).min(columns.min_entropy()));
    (decision, entropy)
}

/// Validate exactly 1000 restarts of 1000 samples each, stored row-major.
/// H_I is in bits per literal sample. No bit expansion is performed.
pub fn assess_restart(
    samples: &[u8],
    initial_entropy: &Float,
    options: RestartOptions,
) -> Result<RestartAssessment, RestartError> {
    assess_restart_reusing_iid(samples, initial_entropy, options, None)
}
// Only the validation coordinator may supply evidence: it was computed on these
// exact row/column sequences with the same settings during this invocation.
pub(crate) fn assess_restart_reusing_iid(
    samples: &[u8],
    initial_entropy: &Float,
    options: RestartOptions,
    evidence: Option<[RestartDatasetAssessment; 2]>,
) -> Result<RestartAssessment, RestartError> {
    let (width, alphabet) = prepare_restart(samples, initial_entropy, options)?;
    let columns_data = restart_column_sequence(samples)?;
    let sanity = sanity(samples, &columns_data, initial_entropy, options)?;
    let mut result = RestartAssessment {
        options,
        bits_per_symbol: width,
        alphabet,
        initial_entropy: Float::with_val(options.precision_bits, initial_entropy),
        sanity,
        rows: None,
        columns: None,
        decision: RestartDecision::SanityRejected,
        min_entropy: None,
    };
    if result.sanity.reject {
        return Ok(result);
    }
    let [rows, columns] = evidence.unwrap_or_else(|| {
        [
            dataset(samples, &result.alphabet, width, options),
            dataset(&columns_data, &result.alphabet, width, options),
        ]
    });
    (result.decision, result.min_entropy) = validation(
        &rows,
        &columns,
        &result.initial_entropy,
        options.precision_bits,
    );
    result.rows = Some(rows);
    result.columns = Some(columns);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_estimator_prevents_validation() {
        let incomplete = RestartDatasetAssessment::NonIid(Box::new(NonIidDatasetAssessment {
            sample_count: RESTART_SAMPLES,
            estimators: vec![EstimatorResult {
                estimator: EstimatorKind::Lrs,
                outcome: EstimatorOutcome::Unavailable(EstimateError::TooFewSamples { count: 0 }),
            }],
            min_entropy: Float::with_val(256, 0.8),
        }));
        let (decision, entropy) =
            validation(&incomplete, &incomplete, &Float::with_val(256, 1), 256);
        assert_eq!(decision, RestartDecision::Incomplete);
        assert!(entropy.is_none());
    }
    #[test]
    fn iid_rejection_prevents_validation_even_with_high_entropy() {
        let data = [0, 1].repeat(150);
        let checks = IidChecks {
            independence: chi_square_independence(&data),
            goodness_of_fit: chi_square_goodness_of_fit(&data),
            longest_repeated_substring: iid_lrs_test(&data, 256),
            permutation: Err(PermutationError::ZeroSeed),
        };
        let dataset = RestartDatasetAssessment::Iid {
            estimate: Box::new(most_common_value(&data).unwrap()),
            checks: Box::new(checks),
        };
        let (decision, entropy) = validation(&dataset, &dataset, &Float::with_val(256, 0), 256);
        assert_eq!(decision, RestartDecision::IidRejected);
        assert!(entropy.is_none());
    }
    #[test]
    fn validation_boundary_and_minimum() {
        let make = |h| {
            RestartDatasetAssessment::NonIid(Box::new(NonIidDatasetAssessment {
                sample_count: RESTART_SAMPLES,
                estimators: vec![],
                min_entropy: Float::with_val(256, h),
            }))
        };
        let h = Float::with_val(256, 1);
        let (decision, entropy) = validation(&make(0.5), &make(0.75), &h, 256);
        assert_eq!(decision, RestartDecision::Validated);
        assert_eq!(entropy.unwrap(), 0.5);
        let (decision, entropy) = validation(&make(0.49), &make(0.75), &h, 256);
        assert_eq!(decision, RestartDecision::EntropyRejected);
        assert!(entropy.is_none());
    }
    #[test]
    fn degenerate_distribution_and_seeded_simulation() {
        assert_eq!(simulate(1.0, RESTART_SIMULATIONS, [1, 2, 3, 4]), 1000);
        let a = simulate(0.5, 10_000, [1, 2, 3, 4]);
        assert_eq!(a, simulate(0.5, 10_000, [1, 2, 3, 4]));
        assert!((500..650).contains(&a));
    }
    #[test]
    #[ignore = "requires NIST_RESTART_REFERENCE adapter"]
    fn simulation_matches_reference_stream() {
        let adapter = std::env::var_os("NIST_RESTART_REFERENCE").unwrap();
        for h in [0.0, 0.5, 1.0, 2.3, 8.0] {
            let out = std::process::Command::new(&adapter)
                .arg(h.to_string())
                .arg("10000")
                .output()
                .unwrap();
            assert!(out.status.success());
            let expected: usize = String::from_utf8(out.stdout)
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            assert_eq!(
                simulate(2f64.powf(-h), 10_000, [1, 2, 3, 4]),
                expected,
                "H={h}"
            );
        }
    }
}
