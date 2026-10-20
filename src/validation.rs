//! Strict numerical workflow for §3.1. Collection provenance, physical models,
//! and certification remain outside this API.
use crate::*;
use rug::{Float, float::Round};
use std::{error::Error, fmt, sync::Arc};

/// `samples_per_block` counts primary-source samples only. Additional input
/// lengths are supplied through `ValidationOptions::conditioning`.
/// Non-vetted outputs are unpacked bits, concatenated in generation order.
/// Their length must represent at least one million complete output blocks.
#[derive(Clone, Copy, Debug)]
pub enum ValidationConditioning<'a> {
    /// Derives full output and internal widths from SP 800-90B Table 1.
    VettedComponent {
        samples_per_block: u32,
        component: VettedConditioner,
    },
    Vetted {
        samples_per_block: u32,
        output_bits: u32,
        internal_width: u32,
    },
    NonVetted {
        samples_per_block: u32,
        output_bits: u32,
        internal_width: u32,
        output_data: &'a [u8],
    },
}
#[derive(Clone, Copy, Debug)]
pub struct ValidationOptions {
    /// Explicit raw sample width, shared by sequential and restart data.
    pub bits_per_symbol: u8,
    /// Caller asserts a documented physical/design rationale for the IID claim.
    /// Statistical rejection selects non-IID for the entire workflow.
    pub iid_claim: bool,
    /// Precision, confidence constant, simulation and IID test settings.
    /// `mode` and `bits_per_symbol` are overridden by the workflow.
    pub restart: RestartOptions,
    pub conditioning: ConditioningOptions,
}
impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            bits_per_symbol: 1,
            iid_claim: false,
            restart: RestartOptions::default(),
            conditioning: ConditioningOptions::default(),
        }
    }
}
#[derive(Debug)]
pub enum ValidationError {
    InsufficientSequentialData { count: usize },
    InvalidSubmitterEntropy,
    InvalidConditioningData,
    Input(AssessmentError),
    Restart(RestartError),
    Conditioning(ConditioningError),
}
impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "validation workflow error: {self:?}")
    }
}
impl Error for ValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(e) => Some(e),
            Self::Restart(e) => Some(e),
            Self::Conditioning(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationDecision {
    Complete,
    Incomplete,
    RestartRejected,
}
#[derive(Debug)]
pub enum ValidationDataset {
    Iid(Arc<IidAssessment>),
    NonIid(Box<NonIidAssessment>),
}
impl ValidationDataset {
    /// Completed dataset entropy in bits per input symbol. For the conditioned
    /// output dataset, input symbols are unpacked bits, so this is bits per bit.
    pub fn eligible_entropy(&self) -> Option<&Float> {
        match self {
            Self::Iid(r) => r.eligible_entropy(),
            Self::NonIid(r) => r.eligible_entropy(),
        }
    }
}
#[derive(Debug)]
pub struct ValidationAssessment {
    pub options: ValidationOptions,
    pub submitter_entropy: Float,
    pub track: RestartMode,
    /// Attempted checks: sequential, restart rows, restart columns, then non-vetted output.
    /// Stops on rejection or failed execution; later IID checks are then unnecessary.
    /// Present only for a claimed IID source; retains evidence even on fallback.
    pub iid_evidence: Vec<Arc<IidAssessment>>,
    pub sequential: Option<ValidationDataset>,
    pub conditioned: Option<ValidationDataset>,
    /// Bits per raw sample, after the submitter bound and before restart testing.
    pub initial_entropy: Option<Float>,
    pub restart: Option<RestartAssessment>,
    pub conditioning: Option<ConditioningEstimate>,
    pub decision: ValidationDecision,
    /// Bits per raw sample. Available only when the entire requested workflow completes.
    pub raw_entropy: Option<Float>,
    /// Bits per delivered output block (after truncation) if conditioned; otherwise bits per raw sample.
    pub output_entropy: Option<Float>,
}
impl ValidationAssessment {
    /// Final workflow entropy, available only on complete numerical validation.
    ///
    /// Units are bits per raw sample without conditioning, or bits per delivered
    /// output block with conditioning (including any requested truncation).
    /// The `raw_entropy` field separately reports bits per raw sample.
    pub fn eligible_entropy(&self) -> Option<&Float> {
        (self.decision == ValidationDecision::Complete)
            .then_some(self.output_entropy.as_ref())
            .flatten()
    }
}

/// Live, synchronous workflow diagnostics. Finished events are emitted even
/// when a stage returns an error; inspect the function result for success.
#[derive(Clone, Debug)]
pub enum ValidationProgress {
    StageStarted {
        stage: &'static str,
    },
    StageFinished {
        stage: &'static str,
        elapsed: std::time::Duration,
    },
    Permutation {
        stage: &'static str,
        progress: PermutationProgress,
    },
    RestartIidChecksReused,
}
fn measured<T>(
    observer: &mut dyn FnMut(ValidationProgress),
    stage: &'static str,
    work: impl FnOnce(&mut dyn FnMut(ValidationProgress)) -> T,
) -> T {
    observer(ValidationProgress::StageStarted { stage });
    let start = std::time::Instant::now();
    let result = work(observer);
    observer(ValidationProgress::StageFinished {
        stage,
        elapsed: start.elapsed(),
    });
    result
}
// Copy only successful check results. No error is converted to a pass and the
// literal MCV estimate is used (never the combined literal/bitstring bound).
fn restart_evidence(report: &IidAssessment) -> RestartDatasetAssessment {
    assert_eq!(report.decision, IidDecision::NotRejected);
    let checks = report.checks.as_ref().expect("accepted IID evidence");
    RestartDatasetAssessment::Iid {
        estimate: Box::new(
            report
                .literal
                .as_ref()
                .expect("initial literal estimate")
                .clone(),
        ),
        checks: Box::new(IidChecks {
            independence: Ok(checks
                .independence
                .as_ref()
                .expect("complete check")
                .clone()),
            goodness_of_fit: Ok(checks
                .goodness_of_fit
                .as_ref()
                .expect("complete check")
                .clone()),
            longest_repeated_substring: Ok(checks
                .longest_repeated_substring
                .as_ref()
                .expect("complete check")
                .clone()),
            permutation: Ok(checks.permutation.as_ref().expect("complete check").clone()),
        }),
    }
}

fn check_symbols(data: &[u8], width: u8) -> Result<(), ValidationError> {
    if !(1..=8).contains(&width) {
        return Err(ValidationError::Input(
            AssessmentError::InvalidBitsPerSymbol { bits: width },
        ));
    }
    if let Some((index, &value)) = data
        .iter()
        .enumerate()
        .find(|(_, v)| **v as u16 >= (1u16 << width))
    {
        return Err(ValidationError::Input(AssessmentError::SymbolOutOfRange {
            index,
            value,
            bits: width,
        }));
    }
    Ok(())
}
fn iid_options(o: ValidationOptions, width: u8) -> IidOptions {
    IidOptions {
        precision_bits: o.restart.precision_bits,
        confidence_constant: o.restart.confidence_constant,
        bits_per_symbol: Some(width),
        bitstring_limit: Some(RESTART_SAMPLES),
        chi_square_grouping: o.restart.chi_square_grouping,
        permutation: o.restart.permutation,
        ..Default::default()
    }
}
fn dataset(
    data: &[u8],
    width: u8,
    track: RestartMode,
    o: ValidationOptions,
) -> Result<ValidationDataset, ValidationError> {
    if track == RestartMode::Iid {
        assess_iid(data, iid_options(o, width))
            .map(|r| ValidationDataset::Iid(Arc::new(r)))
            .map_err(ValidationError::Input)
    } else {
        assess_non_iid(
            data,
            NonIidOptions {
                precision_bits: o.restart.precision_bits,
                confidence_constant: o.restart.confidence_constant,
                bits_per_symbol: Some(width),
                bitstring_limit: Some(RESTART_SAMPLES),
                ..Default::default()
            },
        )
        .map(|r| ValidationDataset::NonIid(Box::new(r)))
        .map_err(ValidationError::Input)
    }
}
/// Assess eligible datasets, choose one track, apply H_submitter, test restarts,
/// and optionally bound conditioning output. A complete report is numerical
/// evidence, not certification. No entropy is awarded for incomplete stages.
///
/// Non-vetted data uses one byte per bit (0 or 1), not packed bytes. The caller
/// supplies exactly the raw samples consumed per conditioning block; additional
/// inputs are not credited. Vetted status is a caller assertion.
pub fn assess_validation(
    sequential: &[u8],
    restart_data: &[u8],
    submitter_entropy: &Float,
    conditioning: Option<ValidationConditioning<'_>>,
    options: ValidationOptions,
) -> Result<ValidationAssessment, ValidationError> {
    assess_validation_with_progress(
        sequential,
        restart_data,
        submitter_entropy,
        conditioning,
        options,
        &mut |_| {},
    )
}
/// Strict workflow with stage timings and live permutation progress. Observers
/// cannot change computation; keep callbacks brief to avoid slowing the run.
pub fn assess_validation_with_progress(
    sequential: &[u8],
    restart_data: &[u8],
    submitter_entropy: &Float,
    conditioning: Option<ValidationConditioning<'_>>,
    options: ValidationOptions,
    observer: &mut dyn FnMut(ValidationProgress),
) -> Result<ValidationAssessment, ValidationError> {
    let mut o = options;
    if sequential.len() < RESTART_SAMPLES {
        return Err(ValidationError::InsufficientSequentialData {
            count: sequential.len(),
        });
    }
    if restart_data.len() != RESTART_SAMPLES {
        return Err(ValidationError::Restart(RestartError::InvalidSampleCount {
            count: restart_data.len(),
        }));
    }
    check_symbols(sequential, o.bits_per_symbol)?;
    check_symbols(restart_data, o.bits_per_symbol)?;
    if !submitter_entropy.is_finite()
        || submitter_entropy < &0
        || submitter_entropy > &o.bits_per_symbol
    {
        return Err(ValidationError::InvalidSubmitterEntropy);
    }
    let dimensions = conditioning.map(|c| match c {
        ValidationConditioning::Vetted {
            samples_per_block,
            output_bits,
            internal_width,
        }
        | ValidationConditioning::NonVetted {
            samples_per_block,
            output_bits,
            internal_width,
            ..
        } => (samples_per_block, output_bits, internal_width),
        ValidationConditioning::VettedComponent {
            samples_per_block,
            component,
        } => (
            samples_per_block,
            component.output_bits(),
            component.internal_width(),
        ),
    });
    if let Some((samples, out, width)) = dimensions {
        if samples == 0
            || out == 0
            || width == 0
            || samples.checked_mul(o.bits_per_symbol as u32).is_none()
        {
            return Err(ValidationError::InvalidConditioningData);
        }
    }
    if let Some((samples, _, _)) = dimensions {
        crate::conditioning::account_conditioning_inputs(
            samples * u32::from(o.bits_per_symbol),
            o.conditioning.additional_input_bits,
            matches!(conditioning, Some(ValidationConditioning::NonVetted { .. })),
        )
        .map_err(ValidationError::Conditioning)?;
    } else if o.conditioning.additional_input_bits != 0 {
        return Err(ValidationError::InvalidConditioningData);
    }
    if dimensions.is_some()
        && (!(64..=rug::float::prec_max()).contains(&o.conditioning.precision_bits)
            || o.conditioning.max_working_precision < o.conditioning.precision_bits
            || o.conditioning.max_working_precision > rug::float::prec_max())
    {
        return Err(ValidationError::Conditioning(
            ConditioningError::InvalidPrecision,
        ));
    }
    if let Some((_, out, _)) = dimensions {
        crate::conditioning::validate_truncation(
            out,
            matches!(conditioning, Some(ValidationConditioning::NonVetted { .. })),
            o.conditioning.retained_output_bits,
        )
        .map_err(ValidationError::Conditioning)?;
    } else if o.conditioning.retained_output_bits.is_some() {
        return Err(ValidationError::InvalidConditioningData);
    }
    let output_data = match conditioning {
        Some(ValidationConditioning::NonVetted {
            output_data,
            output_bits,
            ..
        }) => {
            let minimum = (output_bits as usize)
                .checked_mul(RESTART_SAMPLES)
                .ok_or(ValidationError::InvalidConditioningData)?;
            if output_data.len() < minimum || output_data.len() % output_bits as usize != 0 {
                return Err(ValidationError::InvalidConditioningData);
            }
            check_symbols(output_data, 1)?;
            Some(output_data)
        }
        _ => None,
    };
    if o.iid_claim && o.restart.permutation.seed.is_none() {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes)
            .map_err(|e| ValidationError::Restart(RestartError::Randomness(e.to_string())))?;
        o.restart.permutation.seed = Some(std::array::from_fn(|i| {
            u64::from_le_bytes(bytes[8 * i..8 * i + 8].try_into().unwrap())
        }));
    }
    let mut report = ValidationAssessment {
        options: o,
        submitter_entropy: submitter_entropy.clone(),
        track: RestartMode::NonIid,
        iid_evidence: vec![],
        sequential: None,
        conditioned: None,
        initial_entropy: None,
        restart: None,
        conditioning: None,
        decision: ValidationDecision::Incomplete,
        raw_entropy: None,
        output_entropy: None,
    };
    if o.iid_claim {
        let columns = restart_column_sequence(restart_data).map_err(ValidationError::Restart)?;
        for (stage, data, width) in [
            ("sequential IID", sequential, o.bits_per_symbol),
            ("restart rows IID", restart_data, o.bits_per_symbol),
            ("restart columns IID", &columns, o.bits_per_symbol),
        ]
        .into_iter()
        .chain(output_data.map(|d| ("conditioned IID", d, 1)))
        {
            report.iid_evidence.push(Arc::new(
                measured(observer, stage, |observer| {
                    assess_iid_with_progress(data, iid_options(o, width), &mut |progress| {
                        observer(ValidationProgress::Permutation { stage, progress })
                    })
                })
                .map_err(ValidationError::Input)?,
            ));
            let last = report.iid_evidence.last().unwrap();
            if last.decision != IidDecision::NotRejected || !last.is_complete() {
                break;
            }
        }
        // Failed execution is not evidence for or against IID. Retain diagnostics.
        if report.iid_evidence.iter().any(|r| {
            r.decision == IidDecision::Incomplete || (r.checks.is_some() && !r.is_complete())
        }) {
            return Ok(report);
        }
        if report
            .iid_evidence
            .iter()
            .all(|r| r.decision == IidDecision::NotRejected)
        {
            report.track = RestartMode::Iid;
        }
    }
    report.sequential = Some(if report.track == RestartMode::Iid {
        ValidationDataset::Iid(Arc::clone(&report.iid_evidence[0]))
    } else {
        measured(observer, "sequential non-IID", |_| {
            dataset(sequential, o.bits_per_symbol, report.track, o)
        })?
    });
    let Some(h) = report
        .sequential
        .as_ref()
        .and_then(ValidationDataset::eligible_entropy)
    else {
        return Ok(report);
    };
    let initial = Float::with_val_round(
        o.restart.precision_bits,
        h.min_ref(submitter_entropy),
        Round::Down,
    )
    .0;
    report.initial_entropy = Some(initial.clone());
    let evidence = (report.track == RestartMode::Iid).then(|| {
        observer(ValidationProgress::RestartIidChecksReused);
        [
            restart_evidence(&report.iid_evidence[1]),
            restart_evidence(&report.iid_evidence[2]),
        ]
    });
    let restart = measured(observer, "restart assessment", |_| {
        crate::restart::assess_restart_reusing_iid(
            restart_data,
            &initial,
            RestartOptions {
                mode: report.track,
                bits_per_symbol: Some(o.bits_per_symbol),
                ..o.restart
            },
            evidence,
        )
    })
    .map_err(ValidationError::Restart)?;
    let raw = restart.min_entropy.clone();
    report.decision = match restart.decision {
        RestartDecision::Validated => ValidationDecision::Incomplete,
        RestartDecision::Incomplete => ValidationDecision::Incomplete,
        _ => ValidationDecision::RestartRejected,
    };
    report.restart = Some(restart);
    let Some(raw) = raw else {
        return Ok(report);
    };
    let mut output = raw.clone();
    if let Some((samples, out, width)) = dimensions {
        let kind = if let Some(data) = output_data {
            report.conditioned = Some(if report.track == RestartMode::Iid {
                ValidationDataset::Iid(Arc::clone(&report.iid_evidence[3]))
            } else {
                measured(observer, "conditioned non-IID", |_| {
                    dataset(data, 1, report.track, o)
                })?
            });
            let Some(h) = report
                .conditioned
                .as_ref()
                .and_then(ValidationDataset::eligible_entropy)
            else {
                return Ok(report);
            };
            ConditioningKind::NonVetted {
                assessed_entropy_per_bit: h.clone(),
            }
        } else {
            ConditioningKind::Vetted
        };
        let input_entropy =
            Float::with_val_round(o.conditioning.precision_bits, &raw * samples, Round::Down).0;
        let result = measured(observer, "conditioning bound", |_| {
            if let Some(ValidationConditioning::VettedComponent { component, .. }) = conditioning {
                return component.assess(
                    samples * o.bits_per_symbol as u32,
                    &input_entropy,
                    o.conditioning,
                );
            }
            conditioning_entropy(
                samples * o.bits_per_symbol as u32,
                out,
                width,
                &input_entropy,
                kind,
                o.conditioning,
            )
        })
        .map_err(ValidationError::Conditioning)?;
        output = result.min_entropy.clone();
        report.conditioning = Some(result);
    }
    report.raw_entropy = Some(raw);
    report.output_entropy = Some(output);
    report.decision = ValidationDecision::Complete;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reused_restart_evidence_keeps_literal_entropy_and_all_checks() {
        let mut state = 123456789u64;
        let data: Vec<u8> = (0..300)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state % 7) as u8
            })
            .collect();
        let report = assess_iid(
            &data,
            IidOptions {
                bits_per_symbol: Some(8),
                permutation: PermutationOptions {
                    seed: Some([1, 2, 3, 4]),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(report.decision, IidDecision::NotRejected);
        let cached = restart_evidence(&report);
        assert_eq!(
            cached.min_entropy(),
            &most_common_value(&data).unwrap().min_entropy
        );
        assert!(cached.min_entropy() > report.min_entropy.as_ref().unwrap());
        let RestartDatasetAssessment::Iid { checks, .. } = cached else {
            panic!("IID evidence");
        };
        let original = report.checks.unwrap();
        assert_eq!(format!("{checks:?}"), format!("{original:?}"));
    }
}
