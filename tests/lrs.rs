#![cfg(feature = "assessment")]

use rug::Float;
use sp_800_90b::{
    ConfidenceConstant, EstimateError, LrsOptions, lrs_estimate, lrs_estimate_with_options,
};
const DATA: [u8; 21] = [
    2, 2, 0, 1, 0, 2, 0, 1, 2, 1, 2, 0, 1, 2, 1, 0, 0, 1, 0, 0, 0,
];
fn options() -> LrsOptions {
    LrsOptions {
        cutoff: 3,
        confidence_constant: ConfidenceConstant::Publication,
        ..Default::default()
    }
}
#[test]
fn paper_example_and_independent_decimal_oracle() {
    let r = lrs_estimate_with_options(&DATA, options()).unwrap();
    assert_eq!((r.min_length, r.max_length), (4, 5));
    assert_eq!(r.pair_counts, [2, 1]);
    // Decimal(100): max((2/153)^(1/4),(1/136)^(1/5)), then confidence correction.
    let expected = Float::with_val(256, Float::parse("0.6145634622719141234049791038080910495071147264321768204019378536238582115545655291259893218767025498").unwrap());
    assert!(
        (r.min_entropy - expected).abs() < Float::with_val(256, Float::parse("1e-74").unwrap())
    );
}
#[test]
fn constant_overlaps_and_cutoff_boundary() {
    let r = lrs_estimate(&[255; 100]).unwrap();
    assert_eq!((r.min_length, r.max_length), (67, 99));
    for (i, &pairs) in r.pair_counts.iter().enumerate() {
        let count = (34 - i) as u128;
        assert_eq!(pairs, count * (count - 1) / 2);
    }
    assert_eq!(r.p_hat, 1);
    assert_eq!(r.min_entropy, 0);
    let small = lrs_estimate(&[0, 0]).unwrap();
    assert_eq!((small.min_length, small.max_length), (1, 1));
    assert_eq!(small.min_entropy, 0);
}
#[test]
fn empty_range_and_invalid_inputs() {
    assert_eq!(
        lrs_estimate(&[0, 1, 2]).unwrap_err(),
        EstimateError::NoRepeatedSubstringRange {
            min_length: 1,
            max_length: 0
        }
    );
    assert!(matches!(
        lrs_estimate_with_options(
            &DATA,
            LrsOptions {
                cutoff: 2,
                ..options()
            }
        ),
        Err(EstimateError::NoRepeatedSubstringRange { .. })
    ));
    for data in [vec![], vec![0]] {
        assert_eq!(
            lrs_estimate(&data).unwrap_err(),
            EstimateError::TooFewSamples { count: data.len() }
        );
    }
    for cutoff in [0, 1] {
        assert_eq!(
            lrs_estimate_with_options(
                &DATA,
                LrsOptions {
                    cutoff,
                    ..options()
                }
            )
            .unwrap_err(),
            EstimateError::InvalidTupleCutoff
        );
    }
    assert_eq!(
        lrs_estimate_with_options(
            &DATA,
            LrsOptions {
                precision_bits: 0,
                ..options()
            }
        )
        .unwrap_err(),
        EstimateError::InvalidPrecision { bits: 0 }
    );
}
#[test]
fn relabeling_and_precision_stability() {
    let a = lrs_estimate_with_options(&DATA, options()).unwrap();
    let b = lrs_estimate_with_options(&DATA.map(|x| [255, 17, 0][x as usize]), options()).unwrap();
    assert_eq!(a.pair_counts, b.pair_counts);
    assert_eq!(a.min_entropy, b.min_entropy);
    let higher = lrs_estimate_with_options(
        &DATA,
        LrsOptions {
            precision_bits: 512,
            ..options()
        },
    )
    .unwrap();
    assert!(
        (Float::with_val(512, a.min_entropy) - higher.min_entropy).abs()
            < Float::with_val(512, Float::parse("1e-74").unwrap())
    );
}
