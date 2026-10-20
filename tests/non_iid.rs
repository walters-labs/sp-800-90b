#![cfg(feature = "assessment")]

use rug::Float;
use sp_800_90b::*;

fn available(result: &EstimatorResult) -> &EstimatorDetails {
    match &result.outcome {
        EstimatorOutcome::Available(details) => details,
        other => panic!("{other:?}"),
    }
}
#[test]
fn malformed_input_is_rejected_before_estimators() {
    for n in [0, 1] {
        assert_eq!(
            assess_non_iid(&vec![0; n], NonIidOptions::default()).unwrap_err(),
            AssessmentError::TooFewSamples { count: n }
        );
    }
    for bits in [0, 9, 255] {
        assert_eq!(
            assess_non_iid(
                &[0, 1],
                NonIidOptions {
                    bits_per_symbol: Some(bits),
                    ..Default::default()
                }
            )
            .unwrap_err(),
            AssessmentError::InvalidBitsPerSymbol { bits }
        );
    }
    assert_eq!(
        assess_non_iid(
            &[0, 4],
            NonIidOptions {
                bits_per_symbol: Some(2),
                ..Default::default()
            }
        )
        .unwrap_err(),
        AssessmentError::SymbolOutOfRange {
            index: 1,
            value: 4,
            bits: 2
        }
    );
    for limit in [0, 1] {
        assert_eq!(
            assess_non_iid(
                &[0, 1],
                NonIidOptions {
                    bitstring_limit: Some(limit),
                    ..Default::default()
                }
            )
            .unwrap_err(),
            AssessmentError::InvalidBitstringLimit { limit }
        );
    }
    assert_eq!(
        assess_non_iid(
            &[0, 1],
            NonIidOptions {
                precision_bits: 0,
                ..Default::default()
            }
        )
        .unwrap_err(),
        AssessmentError::InvalidPrecision { bits: 0 }
    );
}
#[test]
fn constants_are_explicit_zero_entropy_outcomes() {
    for symbol in [0, 7, 255] {
        for mode in [NonIidMode::Initial, NonIidMode::Conditioned] {
            let report = assess_non_iid(
                &[symbol; 20],
                NonIidOptions {
                    mode,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(report.min_entropy, 0);
            assert!(report.is_complete());
            assert_eq!(report.alphabet, [symbol]);
            assert!(report.literal.is_none() && report.bitstring.is_none());
            assert!(report.warnings.contains(&AssessmentWarning::ConstantInput));
        }
    }
}
#[test]
fn unavailable_is_distinct_from_inapplicable_and_minimum_is_partial() {
    let report = assess_non_iid(&[1, 4, 7], NonIidOptions::default()).unwrap();
    assert!(!report.is_complete());
    let literal = report.literal.as_ref().unwrap();
    assert_eq!(literal.estimators.len(), 10);
    for index in 1..=3 {
        assert!(matches!(
            literal.estimators[index].outcome,
            EstimatorOutcome::NotApplicable
        ));
    }
    assert!(matches!(
        literal.estimators[4].outcome,
        EstimatorOutcome::Unavailable(EstimateError::NoQualifyingTuple { .. })
    ));
    assert!(matches!(
        literal.estimators[6].outcome,
        EstimatorOutcome::Unavailable(EstimateError::TooFewPredictionSamples { .. })
    ));
    let bits = report.bitstring.as_ref().unwrap();
    assert!(matches!(
        bits.estimators[3].outcome,
        EstimatorOutcome::Unavailable(EstimateError::TooFewTestBlocks { .. })
    ));
    assert!(report.min_entropy.is_finite());
    assert!(report.min_entropy <= literal.min_entropy);
    assert!(
        report
            .warnings
            .contains(&AssessmentWarning::FewerThanRecommendedSamples { count: 3 })
    );
}
#[test]
fn two_symbol_initial_runs_all_binary_estimators_on_rank_mapped_data() {
    let report = assess_non_iid(
        &[7, 255].repeat(3100),
        NonIidOptions {
            bitstring_limit: Some(2),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(report.bits_per_symbol, 8);
    assert!(report.bitstring.is_none());
    assert!(report.is_complete());
    let literal = report.literal.unwrap();
    assert_eq!(literal.sample_count, 6200);
    assert_eq!(report.min_entropy, literal.min_entropy);
    match available(&literal.estimators[2]) {
        EstimatorDetails::Markov(x) => assert_eq!(x.transition_counts, [[0, 3100], [3099, 0]]),
        _ => panic!("wrong estimator"),
    }
}
#[test]
fn initial_combination_and_precision_propagation() {
    let mut state = 103u64;
    let data: Vec<u8> = (0..6200)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            [1, 4, 7][(state % 3) as usize]
        })
        .collect();
    let options = NonIidOptions {
        precision_bits: 128,
        confidence_constant: ConfidenceConstant::Publication,
        ..Default::default()
    };
    let report = assess_non_iid(&data, options).unwrap();
    assert!(report.is_complete());
    let literal = report.literal.as_ref().unwrap();
    let bits = report.bitstring.as_ref().unwrap();
    assert_eq!(bits.sample_count, 18600);
    let scaled = Float::with_val(128, &bits.min_entropy * 3);
    let expected = if literal.min_entropy < scaled {
        literal.min_entropy.clone()
    } else {
        scaled
    };
    assert_eq!(report.min_entropy, expected);
    for dataset in [literal, bits] {
        for result in &dataset.estimators {
            if let EstimatorOutcome::Available(details) = &result.outcome {
                assert_eq!(details.min_entropy().prec(), 128);
            }
        }
    }
    let direct = most_common_value_with_options(
        &data,
        McvOptions {
            precision_bits: 128,
            confidence_constant: ConfidenceConstant::Publication,
        },
    )
    .unwrap();
    assert_eq!(
        available(&literal.estimators[0]).min_entropy(),
        &direct.min_entropy
    );
}
#[test]
fn conditioned_mode_returns_per_bit_and_per_symbol_and_truncates_only_bits() {
    let options = NonIidOptions {
        mode: NonIidMode::Conditioned,
        bits_per_symbol: Some(4),
        bitstring_limit: Some(5),
        ..Default::default()
    };
    let report = assess_non_iid(&[1, 4, 7], options).unwrap();
    assert!(report.literal.is_none());
    let bits = report.bitstring.unwrap();
    assert_eq!(bits.sample_count, 5);
    assert_eq!(
        report.min_entropy,
        Float::with_val(256, &bits.min_entropy * 4)
    );
    assert!(
        report
            .warnings
            .contains(&AssessmentWarning::DeclaredWidthExceedsObserved {
                declared: 4,
                observed: 3
            })
    );
    assert!(
        report
            .warnings
            .contains(&AssessmentWarning::BitstringTruncated {
                available: 12,
                used: 5
            })
    );
    match available(&bits.estimators[0]) {
        EstimatorDetails::MostCommonValue(x) => assert_eq!(x.mode_count, 4),
        _ => panic!("wrong estimator"),
    }
}
