#![cfg(feature = "assessment")]

use rug::Float;
use sp_800_90b::*;
fn f(x: &str) -> Float {
    Float::with_val(256, Float::parse(x).unwrap())
}
fn calc(ni: u32, no: u32, nw: u32, h: &str, kind: ConditioningKind) -> ConditioningEstimate {
    conditioning_entropy(ni, no, nw, &f(h), kind, ConditioningOptions::default()).unwrap()
}
#[test]
fn zero_and_simple_exact_case() {
    let zero = calc(16, 8, 8, "0", ConditioningKind::Vetted);
    assert_eq!(zero.min_entropy, 0);
    // n_in=2,n=1,h=1: p_high=1/2,p_low=1/6, psi=5/6.
    let r = calc(2, 1, 1, "1", ConditioningKind::Vetted);
    let expected = Float::with_val(1024, Float::with_val(1024, 6) / 5).log2();
    assert!(r.output_entropy <= expected);
    assert!(Float::with_val(1024, expected - r.output_entropy).abs() < 1e-75);
}
#[test]
fn nonvetted_caps_and_width_clamping() {
    let r = calc(
        512,
        128,
        512,
        "400",
        ConditioningKind::NonVetted {
            assessed_entropy_per_bit: f("0.5"),
        },
    );
    assert_eq!(r.min_entropy, 64);
    let r = calc(
        512,
        128,
        512,
        "400",
        ConditioningKind::NonVetted {
            assessed_entropy_per_bit: f("1"),
        },
    );
    assert_eq!(r.min_entropy, r.non_vetted_cap.unwrap());
    let a = calc(16, 32, 100, "12", ConditioningKind::Vetted);
    let b = calc(16, 32, 16, "12", ConditioningKind::Vetted);
    assert_eq!(a.effective_internal_width, 16);
    assert_eq!(a.min_entropy, b.min_entropy);
}
#[test]
fn conservative_precision_and_near_full_entropy() {
    let low = calc(1024, 256, 512, "768", ConditioningKind::Vetted);
    let high = conditioning_entropy(
        1024,
        256,
        512,
        &f("768"),
        ConditioningKind::Vetted,
        ConditioningOptions {
            precision_bits: 1024,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(low.output_entropy <= high.output_entropy);
    assert!(high.output_entropy < 256);
    assert!(low.working_precision >= 2048);
}
#[test]
fn invalid_parameters() {
    assert!(
        conditioning_entropy(
            0,
            1,
            1,
            &f("1"),
            ConditioningKind::Vetted,
            Default::default()
        )
        .is_err()
    );
    for h in ["-1", "17", "NaN", "inf"] {
        assert!(
            conditioning_entropy(
                16,
                8,
                8,
                &f(h),
                ConditioningKind::Vetted,
                Default::default()
            )
            .is_err()
        );
    }
    assert!(
        conditioning_entropy(
            16,
            8,
            8,
            &f("8"),
            ConditioningKind::NonVetted {
                assessed_entropy_per_bit: f("1.1")
            },
            Default::default()
        )
        .is_err()
    );
    assert!(matches!(
        conditioning_entropy(
            1024,
            256,
            256,
            &f("512"),
            ConditioningKind::Vetted,
            ConditioningOptions {
                max_working_precision: 512,
                ..Default::default()
            }
        ),
        Err(ConditioningError::PrecisionBudgetExceeded)
    ));
}

#[test]
fn truncation_publication_example_and_boundaries() {
    assert_eq!(vetted_truncation_entropy(8, 6, &f("6"), 256).unwrap(), 4.5);
    assert_eq!(vetted_truncation_entropy(8, 8, &f("6"), 256).unwrap(), 6);
    assert_eq!(vetted_truncation_entropy(8, 1, &f("0"), 256).unwrap(), 0);
    assert_eq!(vetted_truncation_entropy(8, 6, &f("8"), 256).unwrap(), 6);
    for (original, retained) in [(0, 0), (8, 0), (8, 9)] {
        assert!(matches!(
            vetted_truncation_entropy(original, retained, &f("0"), 256),
            Err(ConditioningError::InvalidTruncation)
        ));
    }
    for h in ["-1", "9", "NaN", "inf"] {
        assert!(matches!(
            vetted_truncation_entropy(8, 6, &f(h), 256),
            Err(ConditioningError::InvalidEntropy)
        ));
    }
    assert!(matches!(
        vetted_truncation_entropy(8, 6, &f("6"), 63),
        Err(ConditioningError::InvalidPrecision)
    ));
}

#[test]
fn truncation_rounds_down_at_low_precision() {
    // A binary-inexact fraction: check the result by exact multiplication at
    // higher precision, without rounding the expected quotient first.
    let h = f("1");
    for retained in [1, 2, 5, 6] {
        let result = vetted_truncation_entropy(7, retained, &h, 64).unwrap();
        let scaled_back = Float::with_val(256, &result * 7);
        assert!(scaled_back <= retained);
        assert!(Float::with_val(256, Float::with_val(256, retained) - scaled_back) < 1e-17);
    }
}

#[test]
fn conditioning_retains_full_bound_and_scales_only_vetted_output() {
    let full = calc(128, 64, 128, "64", ConditioningKind::Vetted);
    let truncated = conditioning_entropy(
        128,
        64,
        128,
        &f("64"),
        ConditioningKind::Vetted,
        ConditioningOptions {
            retained_output_bits: Some(24),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(truncated.output_bits, 64);
    assert_eq!(truncated.retained_output_bits, 24);
    assert_eq!(truncated.output_entropy, full.output_entropy);
    assert_eq!(truncated.untruncated_min_entropy, full.min_entropy);
    assert_eq!(
        truncated.min_entropy,
        vetted_truncation_entropy(64, 24, &full.min_entropy, 256).unwrap()
    );
    // Recomputing Output_Entropy with a smaller n_out is a different operation.
    assert_ne!(
        truncated.min_entropy,
        calc(128, 24, 128, "64", ConditioningKind::Vetted).min_entropy
    );
    for retained in [24, 64] {
        assert!(matches!(
            conditioning_entropy(
                128,
                64,
                128,
                &f("64"),
                ConditioningKind::NonVetted {
                    assessed_entropy_per_bit: f("0.5")
                },
                ConditioningOptions {
                    retained_output_bits: Some(retained),
                    ..Default::default()
                }
            ),
            Err(ConditioningError::NonVettedTruncation)
        ));
    }
}

#[test]
fn additional_sources_are_recorded_but_never_credited() {
    let component = VettedConditioner::Hash(ConditioningHash::Sha256);
    for entropy in [0, 16, 32] {
        let h = Float::with_val(256, entropy);
        let base = component.assess(32, &h, Default::default()).unwrap();
        for extra in [1, 512, 1_000_000] {
            let report = component
                .assess(
                    32,
                    &h,
                    ConditioningOptions {
                        additional_input_bits: extra,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(
                report.input_accounting,
                ConditioningInputAccounting {
                    primary_bits: 32,
                    additional_bits: extra,
                    total_bits: 32 + extra,
                }
            );
            assert_eq!(report.primary_input_entropy, h);
            assert_eq!(report.input_bits, 32);
            assert_eq!(report.effective_internal_width, 32);
            assert_eq!(report.working_precision, base.working_precision);
            assert_eq!(report.psi_upper, base.psi_upper);
            assert_eq!(report.omega_upper, base.omega_upper);
            assert_eq!(report.min_entropy, base.min_entropy);
        }
    }
    // A larger physical input cannot make an invalid primary entropy legal.
    assert!(matches!(
        component.assess(
            32,
            &f("33"),
            ConditioningOptions {
                additional_input_bits: 512,
                ..Default::default()
            }
        ),
        Err(ConditioningError::InvalidEntropy)
    ));
}

#[test]
fn additional_source_constraints_and_overflow() {
    let options = ConditioningOptions {
        additional_input_bits: 1,
        ..Default::default()
    };
    assert!(matches!(
        conditioning_entropy(
            32,
            16,
            32,
            &f("16"),
            ConditioningKind::NonVetted {
                assessed_entropy_per_bit: f("0.5")
            },
            options
        ),
        Err(ConditioningError::AdditionalSourcesRequireVetted)
    ));
    assert!(matches!(
        conditioning_entropy(0, 16, 32, &f("0"), ConditioningKind::Vetted, options),
        Err(ConditioningError::InvalidDimensions)
    ));
    assert!(matches!(
        conditioning_entropy(
            32,
            16,
            32,
            &f("16"),
            ConditioningKind::Vetted,
            ConditioningOptions {
                additional_input_bits: u64::MAX,
                ..Default::default()
            }
        ),
        Err(ConditioningError::InputLengthOverflow)
    ));
    let report = conditioning_entropy(
        32,
        16,
        32,
        &f("16"),
        ConditioningKind::Vetted,
        ConditioningOptions {
            additional_input_bits: u64::MAX - 32,
            retained_output_bits: Some(8),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(report.input_accounting.total_bits, u64::MAX);
    assert_eq!(
        report.min_entropy,
        vetted_truncation_entropy(16, 8, &report.untruncated_min_entropy, 256).unwrap()
    );
}
