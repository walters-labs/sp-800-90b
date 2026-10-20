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
