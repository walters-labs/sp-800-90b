use sp_800_90b::*;
use std::{error::Error, sync::Arc};

#[test]
fn completed_entropy_and_estimator_lookup() {
    let mut report = assess_non_iid(&[0, 1, 0, 1], NonIidOptions::default()).unwrap();
    let branch = report.literal.as_mut().unwrap();
    assert!(branch.estimator(EstimatorKind::MostCommonValue).is_some());
    assert!(
        branch
            .estimator(EstimatorKind::MostCommonValue)
            .is_some_and(|r| matches!(r.outcome, EstimatorOutcome::Available(_)))
    );
    // Short datasets retain partial diagnostics but must not award that minimum.
    assert!(!branch.is_complete());
    assert!(branch.eligible_entropy().is_none());
    assert!(report.eligible_entropy().is_none());
    let constant = assess_non_iid(&[7; 100], NonIidOptions::default()).unwrap();
    assert_eq!(
        constant.eligible_entropy().unwrap(),
        &Float::with_val(256, 0)
    );
    let dataset = ValidationDataset::NonIid(Box::new(constant));
    assert_eq!(
        dataset.eligible_entropy().unwrap(),
        &Float::with_val(256, 0)
    );
}

#[test]
fn iid_incompleteness_and_rejection_are_distinct() {
    let report = assess_iid(&[0, 1], IidOptions::default()).unwrap();
    assert!(!report.checks.as_ref().unwrap().is_complete());
    assert!(!report.checks.as_ref().unwrap().rejected());
    assert!(report.eligible_entropy().is_none());
    let dataset = ValidationDataset::Iid(Arc::new(report));
    assert!(dataset.eligible_entropy().is_none());
    let rejected = assess_iid(&[0; 1000], IidOptions::default()).unwrap();
    assert_eq!(rejected.decision, IidDecision::Rejected);
    assert!(rejected.eligible_entropy().is_none());
}

#[test]
fn nested_errors_preserve_typed_causes() {
    let compression = PermutationError::Compression(std::io::Error::other("compression failed"));
    assert!(compression.source().unwrap().is::<std::io::Error>());
    let e = ValidationError::Restart(RestartError::Input(AssessmentError::SymbolOutOfRange {
        index: 2,
        value: 9,
        bits: 3,
    }));
    let restart = e.source().unwrap();
    assert!(restart.downcast_ref::<RestartError>().is_some());
    assert!(matches!(
        restart.source().unwrap().downcast_ref::<AssessmentError>(),
        Some(AssessmentError::SymbolOutOfRange {
            index: 2,
            value: 9,
            bits: 3
        })
    ));
    let e = ValidationError::Conditioning(ConditioningError::InvalidDimensions);
    assert!(e.source().unwrap().is::<ConditioningError>());
    assert!(ValidationError::InvalidSubmitterEntropy.source().is_none());
}
