#![cfg(feature = "assessment")]

use rug::Float;
use sp_800_90b::{
    ConfidenceConstant, EstimateError, TTupleOptions, t_tuple_estimate,
    t_tuple_estimate_with_options,
};
const DATA: [u8; 21] = [
    2, 2, 0, 1, 0, 2, 0, 1, 2, 1, 2, 0, 1, 2, 1, 0, 0, 1, 0, 0, 0,
];
fn options() -> TTupleOptions {
    TTupleOptions {
        cutoff: 3,
        confidence_constant: ConfidenceConstant::Publication,
        ..Default::default()
    }
}
#[test]
fn paper_example_and_decimal_oracle() {
    let r = t_tuple_estimate_with_options(&DATA, options()).unwrap();
    assert_eq!(r.max_tuple_length, 3);
    assert_eq!(r.maximum_counts, [9, 4, 3]);
    let expected=Float::with_val(256,Float::parse("0.2730789797113258400202073186826425904853318919202048564376807201333456155949690782339247122952348465").unwrap());
    assert!(
        (r.min_entropy - expected).abs() < Float::with_val(256, Float::parse("1e-74").unwrap())
    );
}
#[test]
fn long_overlapping_tuples_and_inclusive_cutoff() {
    for n in [35, 36, 100, 10000] {
        let r = t_tuple_estimate(&vec![255; n]).unwrap();
        assert_eq!(r.max_tuple_length, n - 34);
        assert_eq!(*r.maximum_counts.last().unwrap(), 35);
        assert_eq!(r.p_upper, 1);
        assert_eq!(r.min_entropy, 0);
    }
    let r = t_tuple_estimate(&[0, 1].repeat(50)).unwrap();
    assert_eq!(r.max_tuple_length, 32);
    assert_eq!(r.maximum_counts[31], 35);
    assert_eq!(r.min_entropy, 0);
}
#[test]
fn missing_tuple_is_a_clear_error() {
    for data in [vec![0; 34], (0..=255).collect(), DATA.to_vec()] {
        assert_eq!(
            t_tuple_estimate(&data).unwrap_err(),
            EstimateError::NoQualifyingTuple { cutoff: 35 }
        );
    }
    for data in [vec![], vec![0]] {
        assert_eq!(
            t_tuple_estimate(&data).unwrap_err(),
            EstimateError::TooFewSamples { count: data.len() }
        );
    }
    for cutoff in [0, 1] {
        assert_eq!(
            t_tuple_estimate_with_options(
                &DATA,
                TTupleOptions {
                    cutoff,
                    ..Default::default()
                }
            )
            .unwrap_err(),
            EstimateError::InvalidTupleCutoff
        );
    }
    assert_eq!(
        t_tuple_estimate_with_options(
            &DATA,
            TTupleOptions {
                precision_bits: 0,
                ..options()
            }
        )
        .unwrap_err(),
        EstimateError::InvalidPrecision { bits: 0 }
    );
}
#[test]
fn counts_and_entropy_are_invariant_under_symbol_relabeling() {
    let a = t_tuple_estimate_with_options(&DATA, options()).unwrap();
    let b =
        t_tuple_estimate_with_options(&DATA.map(|x| [255, 17, 0][x as usize]), options()).unwrap();
    assert_eq!(a.maximum_counts, b.maximum_counts);
    assert_eq!(a.min_entropy, b.min_entropy);
}
#[test]
fn precision_stability() {
    let a = t_tuple_estimate_with_options(&DATA, options()).unwrap();
    let b = t_tuple_estimate_with_options(
        &DATA,
        TTupleOptions {
            precision_bits: 512,
            ..options()
        },
    )
    .unwrap();
    assert!(
        (Float::with_val(512, a.min_entropy) - b.min_entropy).abs()
            < Float::with_val(512, Float::parse("1e-74").unwrap())
    );
}
#[test]
fn confidence_denominator_is_inside_square_root() {
    let data = [0, 0, 1, 1].repeat(20);
    let r = t_tuple_estimate(&data).unwrap();
    assert_eq!(r.max_tuple_length, 1);
    assert_eq!(r.p_hat, 0.5);
    let expected = -(0.5 + 2.5758293035489004_f64 * (0.25_f64 / 79.0).sqrt()).log2();
    assert!((r.min_entropy.to_f64() - expected).abs() < 1e-14);
}
