#![cfg(feature = "assessment")]

use rug::Float;
use sp_800_90b::{
    CollisionOptions, ConfidenceConstant, EstimateError, collision_estimate,
    collision_estimate_with_options,
};
const DATA: [u8; 40] = [
    1, 0, 0, 0, 1, 1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 1, 0,
    1, 0, 1, 0, 1, 1, 1, 0,
];

#[test]
fn paper_example_and_independent_decimal_values() {
    for (constant, expected) in [
        (
            ConfidenceConstant::NistReference,
            "0.448384785674801363708160657802313900226325845261186748924700180930041074238725",
        ),
        (
            ConfidenceConstant::Publication,
            "0.448339587791089936504749614593032152483466339309650359921441016758265180593401",
        ),
    ] {
        let r = collision_estimate_with_options(
            &DATA,
            CollisionOptions {
                confidence_constant: constant,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.collision_count, 14);
        assert_eq!(r.consumed_samples, 38);
        let error = (r.min_entropy - Float::with_val(256, Float::parse(expected).unwrap())).abs();
        assert!(error < Float::with_val(256, Float::parse("1e-73").unwrap()));
    }
}

#[test]
fn boundary_handling_matches_reference() {
    for symbol in [0, 1] {
        assert_eq!(collision_estimate(&[symbol; 100]).unwrap().min_entropy, 0);
    }
    let r = collision_estimate(&[0, 0, 0, 1, 0]).unwrap(); // lengths 2,3
    assert!(r.raw_lower_bound < 2);
    assert_eq!(r.lower_bound, 2);
    assert_eq!(r.min_entropy, 0);
    let r = collision_estimate(&[0, 1, 0].repeat(10)).unwrap();
    assert_eq!(r.mean, 3);
    assert_eq!(r.p, 0.5);
    assert_eq!(r.min_entropy, 1);
}

#[test]
fn discards_incomplete_tail_and_preserves_relabeling() {
    let base = &DATA[..38];
    let expected = collision_estimate(base).unwrap();
    for tail in [&[1][..], &[1, 0][..]] {
        let r = collision_estimate(&[base, tail].concat()).unwrap();
        assert_eq!(r.consumed_samples, 38);
        assert_eq!(r.min_entropy, expected.min_entropy);
    }
    assert_eq!(
        collision_estimate(&DATA.map(|x| 1 - x))
            .unwrap()
            .min_entropy,
        expected.min_entropy
    );
}

#[test]
fn validates_entire_input_and_precision() {
    for data in [
        &[][..],
        &[0][..],
        &[0, 1][..],
        &[0, 0][..],
        &[0, 1, 0, 1][..],
    ] {
        assert!(matches!(
            collision_estimate(data),
            Err(EstimateError::TooFewCollisions { .. })
        ));
    }
    assert_eq!(
        collision_estimate(&[0, 0, 0, 0, 2]).unwrap_err(),
        EstimateError::NonBinarySample { index: 4, value: 2 }
    );
    assert_eq!(
        collision_estimate_with_options(
            &DATA,
            CollisionOptions {
                precision_bits: 0,
                ..Default::default()
            }
        )
        .unwrap_err(),
        EstimateError::InvalidPrecision { bits: 0 }
    );
}

#[test]
fn precision_converges() {
    let a = collision_estimate(&DATA).unwrap();
    let b = collision_estimate_with_options(
        &DATA,
        CollisionOptions {
            precision_bits: 512,
            ..Default::default()
        },
    )
    .unwrap();
    let difference = (Float::with_val(512, a.min_entropy) - b.min_entropy).abs();
    assert!(difference < Float::with_val(512, Float::parse("1e-73").unwrap()));
}
