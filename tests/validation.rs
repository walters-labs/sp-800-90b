#![cfg(feature = "assessment")]

use rug::Float;
use sp_800_90b::*;
fn options() -> ValidationOptions {
    ValidationOptions {
        restart: RestartOptions {
            simulation_seed: Some([1, 2, 3, 4]),
            permutation: PermutationOptions {
                seed: Some([1, 2, 3, 4]),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    }
}
fn data() -> Vec<u8> {
    let mut x = 123456789u64;
    (0..RESTART_SAMPLES)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x & 1) as u8
        })
        .collect()
}
#[test]
fn eligibility_and_units() {
    let d = vec![0; RESTART_SAMPLES];
    let h = Float::with_val(256, 0.5);
    assert!(matches!(
        assess_validation(&d[..100], &d, &h, None, options()),
        Err(ValidationError::InsufficientSequentialData { .. })
    ));
    assert!(matches!(
        assess_validation(&d, &d[..100], &h, None, options()),
        Err(ValidationError::Restart(
            RestartError::InvalidSampleCount { .. }
        ))
    ));
    for value in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            assess_validation(&d, &d, &Float::with_val(256, value), None, options()),
            Err(ValidationError::InvalidSubmitterEntropy)
        ));
    }
    // A million bits is not a million two-bit conditioning outputs.
    let c = ValidationConditioning::NonVetted {
        samples_per_block: 8,
        output_bits: 2,
        internal_width: 8,
        output_data: &d,
    };
    assert!(matches!(
        assess_validation(&d, &d, &h, Some(c), options()),
        Err(ValidationError::InvalidConditioningData)
    ));
    let c = ValidationConditioning::Vetted {
        samples_per_block: u32::MAX,
        output_bits: 2,
        internal_width: 8,
    };
    let o = ValidationOptions {
        bits_per_symbol: 8,
        ..options()
    };
    assert!(matches!(
        assess_validation(&d, &d, &h, Some(c), o),
        Err(ValidationError::InvalidConditioningData)
    ));
    let c = ValidationConditioning::Vetted {
        samples_per_block: 8,
        output_bits: 2,
        internal_width: 8,
    };
    let mut o = options();
    o.conditioning.precision_bits = 0;
    assert!(matches!(
        assess_validation(&d, &d, &h, Some(c), o),
        Err(ValidationError::Conditioning(
            ConditioningError::InvalidPrecision
        ))
    ));
    let mut invalid = d.clone();
    invalid[999_999] = 2;
    assert!(matches!(
        assess_validation(&d, &invalid, &h, None, options()),
        Err(ValidationError::Input(
            AssessmentError::SymbolOutOfRange { .. }
        ))
    ));
}
#[test]
fn submitter_bound_restart_and_conditioning() {
    let d = data();
    let h = Float::with_val(256, 0.25);
    let c = ValidationConditioning::VettedComponent {
        samples_per_block: 8,
        component: VettedConditioner::Hash(ConditioningHash::Sha256),
    };
    let mut o = options();
    o.conditioning.retained_output_bits = Some(1);
    o.conditioning.additional_input_bits = 512;
    let report = assess_validation(&d, &d, &h, Some(c), o).unwrap();
    assert_eq!(report.decision, ValidationDecision::Complete);
    assert_eq!(report.eligible_entropy(), report.output_entropy.as_ref());
    let restart = report.restart.as_ref().unwrap();
    assert_eq!(restart.eligible_entropy(), report.raw_entropy.as_ref());
    assert!(restart.rows.as_ref().unwrap().eligible_entropy().is_some());
    assert_eq!(report.initial_entropy.as_ref().unwrap(), &h);
    assert_eq!(report.restart.as_ref().unwrap().initial_entropy, h);
    assert_eq!(report.raw_entropy.as_ref().unwrap(), &h);
    let expected = conditioning_entropy(
        8,
        256,
        256,
        &Float::with_val(256, 2),
        ConditioningKind::Vetted,
        o.conditioning,
    )
    .unwrap();
    assert_eq!(
        report.conditioning.as_ref().unwrap().retained_output_bits,
        1
    );
    assert_eq!(
        report.conditioning.as_ref().unwrap().vetted_component,
        Some(VettedConditioner::Hash(ConditioningHash::Sha256))
    );
    let conditioning = report.conditioning.as_ref().unwrap();
    assert_eq!(
        conditioning.input_accounting,
        ConditioningInputAccounting {
            primary_bits: 8,
            additional_bits: 512,
            total_bits: 520,
        }
    );
    assert_eq!(conditioning.primary_input_entropy, 2);
    assert_eq!(report.output_entropy.unwrap(), expected.min_entropy);
}
#[test]
fn zero_submitter_bound_is_preserved() {
    let d = data();
    let report = assess_validation(&d, &d, &Float::with_val(256, 0), None, options()).unwrap();
    assert_eq!(report.decision, ValidationDecision::Complete);
    assert_eq!(report.eligible_entropy(), report.output_entropy.as_ref());
    let restart = report.restart.as_ref().unwrap();
    assert_eq!(restart.eligible_entropy(), report.raw_entropy.as_ref());
    assert!(restart.rows.as_ref().unwrap().eligible_entropy().is_some());
    assert_eq!(report.raw_entropy.unwrap(), 0);
    assert_eq!(report.output_entropy.unwrap(), 0);
}
#[test]
fn restart_failure_never_awards_output() {
    let d = data();
    let restart: Vec<u8> = (0..RESTART_SAMPLES)
        .map(|i| ((i / 1000) % 2) as u8)
        .collect();
    let report =
        assess_validation(&d, &restart, &Float::with_val(256, 0.25), None, options()).unwrap();
    assert_eq!(report.decision, ValidationDecision::RestartRejected);
    assert!(report.eligible_entropy().is_none());
    assert!(
        report
            .restart
            .as_ref()
            .unwrap()
            .eligible_entropy()
            .is_none()
    );
    assert_eq!(
        report.restart.unwrap().decision,
        RestartDecision::SanityRejected
    );
    assert!(report.raw_entropy.is_none());
    assert!(report.output_entropy.is_none());
}
#[test]
fn incomplete_iid_evidence_blocks_award() {
    let d = data();
    let mut o = options();
    o.iid_claim = true;
    o.restart.permutation.seed = Some([0; 4]);
    let report = assess_validation(&d, &d, &Float::with_val(256, 0), None, o).unwrap();
    assert_eq!(report.decision, ValidationDecision::Incomplete);
    assert!(report.eligible_entropy().is_none());
    assert_eq!(report.iid_evidence.len(), 1);
    assert!(report.restart.is_none());
    assert!(report.output_entropy.is_none());
}
#[test]
fn non_vetted_output_cap_and_iid_fallback() {
    // Constant input rejects IID immediately, yet has a complete zero bound on
    // the non-IID track. Nonconstant restart input still must be assessed.
    let d = vec![0; RESTART_SAMPLES];
    let restart = data();
    let c = ValidationConditioning::NonVetted {
        samples_per_block: 8,
        output_bits: 1,
        internal_width: 8,
        output_data: &d,
    };
    let report = assess_validation(
        &d,
        &restart,
        &Float::with_val(256, 0.5),
        Some(c),
        ValidationOptions {
            iid_claim: true,
            ..options()
        },
    )
    .unwrap();
    assert_eq!(report.track, RestartMode::NonIid);
    assert_eq!(report.iid_evidence.len(), 1);
    assert_eq!(report.decision, ValidationDecision::Complete);
    assert_eq!(report.eligible_entropy(), report.output_entropy.as_ref());
    let restart = report.restart.as_ref().unwrap();
    assert_eq!(restart.eligible_entropy(), report.raw_entropy.as_ref());
    assert!(restart.rows.as_ref().unwrap().eligible_entropy().is_some());
    assert_eq!(report.initial_entropy.unwrap(), 0);
    assert_eq!(report.conditioning.unwrap().assessed_output_cap.unwrap(), 0);
    assert_eq!(report.output_entropy.unwrap(), 0);
}
#[test]
#[ignore = "full million-sample IID permutation workflow; takes several minutes"]
fn successful_iid_workflow_uses_one_track() {
    let mut state = 42u64;
    let d: Vec<u8> = (0..RESTART_SAMPLES)
        .map(|_| {
            state = state.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            ((z ^ (z >> 31)) >> 63) as u8
        })
        .collect();
    let o = ValidationOptions {
        iid_claim: true,
        ..options()
    };
    let mut permutation_stages = 0;
    let mut reused = false;
    let report =
        assess_validation_with_progress(&d, &d, &Float::with_val(256, 0), None, o, &mut |event| {
            match &event {
                ValidationProgress::Permutation { progress, .. } => {
                    if progress.permutations_completed == 0 {
                        permutation_stages += 1;
                    }
                    if progress.permutations_completed > 10
                        && progress.permutations_completed % 100 != 0
                        && !progress.active_statistics.is_empty()
                    {
                        return;
                    }
                }
                ValidationProgress::RestartIidChecksReused => reused = true,
                _ => {}
            }
            eprintln!("{event:?}");
        })
        .unwrap();
    assert_eq!(permutation_stages, 3);
    assert!(reused);
    assert_eq!(
        report.track,
        RestartMode::Iid,
        "IID evidence: {:#?}",
        report.iid_evidence
    );
    assert_eq!(report.decision, ValidationDecision::Complete);
    assert_eq!(report.eligible_entropy(), report.output_entropy.as_ref());
    let restart = report.restart.as_ref().unwrap();
    assert_eq!(restart.eligible_entropy(), report.raw_entropy.as_ref());
    assert!(restart.rows.as_ref().unwrap().eligible_entropy().is_some());
    assert!(
        report
            .iid_evidence
            .iter()
            .all(|r| r.decision == IidDecision::NotRejected)
    );
    assert!(matches!(report.sequential, Some(ValidationDataset::Iid(_))));
    assert_eq!(report.restart.unwrap().options.mode, RestartMode::Iid);
    assert_eq!(report.output_entropy.unwrap(), 0);
}

#[test]
fn invalid_truncation_is_rejected_before_assessment() {
    let data = vec![0; RESTART_SAMPLES];
    let mut o = options();
    o.conditioning.retained_output_bits = Some(1);
    let h = Float::with_val(256, 0);
    assert!(matches!(
        assess_validation(&data, &data, &h, None, o),
        Err(ValidationError::InvalidConditioningData)
    ));
    let c = ValidationConditioning::NonVetted {
        samples_per_block: 8,
        output_bits: 2,
        internal_width: 8,
        output_data: &[],
    };
    assert!(matches!(
        assess_validation(&data, &data, &h, Some(c), o),
        Err(ValidationError::Conditioning(
            ConditioningError::NonVettedTruncation
        ))
    ));
    o.conditioning.retained_output_bits = Some(3);
    let c = ValidationConditioning::Vetted {
        samples_per_block: 8,
        output_bits: 2,
        internal_width: 8,
    };
    assert!(matches!(
        assess_validation(&data, &data, &h, Some(c), o),
        Err(ValidationError::Conditioning(
            ConditioningError::InvalidTruncation
        ))
    ));
}

#[test]
fn additional_sources_fail_early_for_invalid_workflows() {
    let data = vec![0; RESTART_SAMPLES];
    let h = Float::with_val(256, 0);
    let mut o = options();
    o.conditioning.additional_input_bits = 1;
    assert!(matches!(
        assess_validation(&data, &data, &h, None, o),
        Err(ValidationError::InvalidConditioningData)
    ));
    let nonvetted = ValidationConditioning::NonVetted {
        samples_per_block: 8,
        output_bits: 2,
        internal_width: 8,
        output_data: &[],
    };
    assert!(matches!(
        assess_validation(&data, &data, &h, Some(nonvetted), o),
        Err(ValidationError::Conditioning(
            ConditioningError::AdditionalSourcesRequireVetted
        ))
    ));
    o.conditioning.additional_input_bits = u64::MAX;
    let vetted = ValidationConditioning::VettedComponent {
        samples_per_block: 8,
        component: VettedConditioner::Hash(ConditioningHash::Sha256),
    };
    assert!(matches!(
        assess_validation(&data, &data, &h, Some(vetted), o),
        Err(ValidationError::Conditioning(
            ConditioningError::InputLengthOverflow
        ))
    ));
}
