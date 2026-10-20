#![cfg(feature = "assessment")]

use rug::Float;
use sp_800_90b::{
    ChiSquareError, ChiSquareGrouping, ChiSquareOptions, chi_square_independence,
    chi_square_independence_with_options,
};

fn example() -> Vec<u8> {
    vec![
        2, 2, 3, 1, 3, 2, 3, 2, 1, 3, 1, 1, 2, 3, 1, 1, 2, 2, 2, 3, 3, 2, 3, 2, 3, 1, 2, 2, 3, 3,
        2, 2, 2, 1, 3, 3, 3, 2, 3, 2, 1, 3, 2, 3, 1, 2, 2, 3, 1, 1, 3, 2, 3, 2, 3, 1, 2, 2, 3, 3,
        2, 2, 2, 1, 3, 3, 3, 2, 3, 2, 1, 2, 2, 3, 3, 3, 2, 3, 2, 1, 2, 2, 2, 1, 3, 3, 3, 2, 3, 2,
        1, 3, 2, 3, 1, 2, 2, 3, 1, 1,
    ]
}
fn close(actual: &Float, expected: &str) {
    let difference = Float::with_val(
        256,
        actual - Float::with_val(256, Float::parse(expected).unwrap()),
    )
    .abs();
    assert!(difference < Float::with_val(256, 1e-73), "{actual}");
}
#[test]
fn publication_example_11() {
    for grouping in [
        ChiSquareGrouping::NistReference,
        ChiSquareGrouping::ExactCounts,
    ] {
        let r = chi_square_independence_with_options(
            &example(),
            ChiSquareOptions {
                grouping,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.sample_count, 100);
        assert_eq!(r.observed, [7, 6, 10, 8, 12, 7]);
        assert_eq!(r.pair_bins.unwrap(), [0, 1, 0, 2, 5, 3, 1, 4, 2]);
        assert_eq!(r.degrees_of_freedom, 3);
        for (e, value) in r
            .expected
            .iter()
            .zip(["6.195", "8.295", "11.525", "7.79", "7.79", "8.405"])
        {
            close(e, value);
        }
        close(
            &r.statistic,
            "3.45711970587502063391859130596874388970659704910606234398751663540702591367163524845128986354863858458723",
        );
        // Independently evaluated using Q(3/2,x) = erfc(sqrt(x)) + 2*sqrt(x)*exp(-x)/sqrt(pi).
        close(
            &r.p_value,
            "0.326366283505233675014159193240471594986861183136990411081888856496408035065518806377018887300076008965155",
        );
        assert!(!r.reject_independence);
    }
}
#[test]
fn binary_threshold_and_sparse_symbols() {
    let data = [0, 0, 0, 1, 1, 0, 1, 1].repeat(5);
    for grouping in [
        ChiSquareGrouping::NistReference,
        ChiSquareGrouping::ExactCounts,
    ] {
        let opts = ChiSquareOptions {
            grouping,
            ..Default::default()
        };
        let r = chi_square_independence_with_options(&data, opts).unwrap();
        assert_eq!(r.tuple_length, 2);
        assert_eq!(r.observed, [5; 4]);
        assert_eq!(r.statistic, 0);
        assert_eq!(r.p_value, 1);
        assert_eq!(r.degrees_of_freedom, 2);
        assert!(matches!(
            chi_square_independence_with_options(&data[..39], opts),
            Err(ChiSquareError::InsufficientBinaryCounts)
        ));
    }
    let mapped: Vec<_> = data.iter().map(|&x| if x == 0 { 7 } else { 255 }).collect();
    let r = chi_square_independence(&mapped).unwrap();
    assert_eq!(r.alphabet, [7, 255]);
    assert_eq!(r.observed, [5; 4]);
}
#[test]
fn trailing_sample_changes_marginals_but_not_pairs() {
    let mut data = Vec::new();
    for _ in 0..10 {
        for a in 0..3 {
            for b in 0..3 {
                data.extend([a, b]);
            }
        }
    }
    data.push(0);
    let r = chi_square_independence(&data).unwrap();
    assert_eq!(r.discarded_samples, 1);
    assert_eq!(r.block_count, 90);
    assert_eq!(r.observed, [10; 9]);
    let bins = r.pair_bins.unwrap();
    let expected = Float::with_val(256, Float::with_val(256, 61 * 61 * 90) / (181 * 181));
    assert_eq!(r.expected[bins[0]], expected);
    assert!(r.statistic > 0);
}
#[test]
fn merge_last_underfilled_bin() {
    let r = chi_square_independence_with_options(
        &[0, 1, 2].repeat(20),
        ChiSquareOptions {
            grouping: ChiSquareGrouping::ExactCounts,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.pair_bins.unwrap(), [0, 0, 1, 1, 2, 2, 3, 3, 3]);
    assert_eq!(r.degrees_of_freedom, 1);
    assert_eq!(r.expected[3], 10);
    assert_eq!(r.observed.iter().sum::<usize>(), 30);
}
#[test]
fn invalid_inputs_are_not_passes() {
    assert!(matches!(
        chi_square_independence(&[]),
        Err(ChiSquareError::TooFewSamples { count: 0 })
    ));
    assert!(matches!(
        chi_square_independence(&[1]),
        Err(ChiSquareError::TooFewSamples { count: 1 })
    ));
    assert!(matches!(
        chi_square_independence(&[1; 100]),
        Err(ChiSquareError::ConstantInput)
    ));
    assert!(matches!(
        chi_square_independence(&[0, 1, 2]),
        Err(ChiSquareError::InsufficientDegreesOfFreedom { .. })
    ));
    assert!(matches!(
        chi_square_independence_with_options(
            &example(),
            ChiSquareOptions {
                precision_bits: 63,
                ..Default::default()
            }
        ),
        Err(ChiSquareError::InvalidPrecision { bits: 63 })
    ));
}
#[test]
fn higher_precision_agrees() {
    let a = chi_square_independence(&example()).unwrap();
    let b = chi_square_independence_with_options(
        &example(),
        ChiSquareOptions {
            precision_bits: 512,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(b.p_value.prec(), 512);
    assert!(Float::with_val(512, a.p_value - b.p_value).abs() < 1e-73);
}

#[test]
fn binary_block_length_is_capped_at_eleven() {
    let r = chi_square_independence(&[0, 1].repeat(100_000)).unwrap();
    assert_eq!(r.tuple_length, 11);
    assert_eq!(r.degrees_of_freedom, 2046);
    assert_eq!(r.block_count, 200_000 / 11);
    assert_eq!(r.discarded_samples, 200_000 % 11);
    assert!(r.reject_independence);
    assert!(r.p_value > 0); // Retain a tail far below binary64's exponent range.
}
