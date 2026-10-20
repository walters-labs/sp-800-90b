#![cfg(feature = "assessment")]

use rug::Float;
use sp_800_90b::{
    ChiSquareError, ChiSquareGrouping, ChiSquareOptions, chi_square_goodness_of_fit as test,
    chi_square_goodness_of_fit_with_options as with_options,
};

#[test]
fn identical_binary_blocks() {
    let r = test(&[7, 255].repeat(50)).unwrap();
    assert_eq!(r.alphabet, [7, 255]);
    assert_eq!(r.observed, vec![vec![5, 5]; 10]);
    assert_eq!(r.expected, vec![Float::with_val(256, 5); 2]);
    assert_eq!(r.degrees_of_freedom, 9);
    assert_eq!(r.statistic, 0);
    assert_eq!(r.p_value, 1);
    assert!(!r.reject_goodness_of_fit);
}
#[test]
fn binary_distribution_changes() {
    let mut data = vec![0; 50];
    data.extend([1; 50]);
    let r = test(&data).unwrap();
    assert_eq!(r.statistic, 100);
    assert!(r.reject_goodness_of_fit);
    // Independent MPFR upper incomplete gamma evaluation, df=9.
    let a = Float::with_val(256, 4.5);
    let p = a.clone().gamma_inc(&Float::with_val(256, 50)) / a.gamma();
    assert!(Float::with_val(256, r.p_value / p - 1).abs() < 1e-73);
}

#[test]
fn potential_errata_2025_section_5_2_4_binary_critical_value() {
    // NIST's May 29, 2025 potential errata replace 27.887 with 27.877:
    // https://csrc.nist.gov/files/pubs/sp/800/90/b/final/docs/sp800-90b_errata_potential_updates.pdf
    // The latter is rounded; the API uses the df=9 upper tail at alpha=0.001.
    // Bracket that boundary with actual samples. The rejected case is also
    // below 27.887, so the original printed cutoff would incorrectly pass it.
    for (deviations, expected_statistic, reject) in [
        ([186usize, 15, 5, 0, 0], "27.8768", false),
        ([186usize, 15, 5, 1, 0], "27.8776", true),
    ] {
        let mut data = Vec::with_capacity(100_000);
        for deviation in deviations {
            for ones in [5_000 + deviation, 5_000 - deviation] {
                data.extend(std::iter::repeat_n(0, 10_000 - ones));
                data.extend(std::iter::repeat_n(1, ones));
            }
        }
        let result = test(&data).unwrap();
        assert_eq!(result.degrees_of_freedom, 9);
        assert_eq!(result.expected, vec![Float::with_val(256, 5_000); 2]);
        // Each balanced pair of blocks contributes 8 * deviation^2 / 10_000.
        let expected = Float::with_val(256, Float::parse(expected_statistic).unwrap());
        assert!(Float::with_val(256, &result.statistic - expected).abs() < 1e-73);
        let alpha = Float::with_val(256, Float::parse("0.001").unwrap());
        assert_eq!(result.p_value < alpha, reject);
        assert_eq!(
            result.reject_goodness_of_fit, reject,
            "incorrect decision for statistic {expected_statistic}"
        );
    }
}

#[test]
fn nonbinary_uniform_and_changed_blocks() {
    let r = test(&[0, 1, 2].repeat(100)).unwrap();
    assert_eq!(r.symbol_bins, [0, 1, 2]);
    assert_eq!(r.degrees_of_freedom, 18);
    assert_eq!(r.statistic, 0);
    assert_eq!(r.p_value, 1);
    let mut data = vec![0; 100];
    data.extend([1; 100]);
    data.extend([2; 100]);
    let r = test(&data).unwrap();
    assert_eq!(r.statistic, 520);
    assert!(r.reject_goodness_of_fit);
}
#[test]
fn pooling_and_last_bin_merge() {
    // Counts 20,40,50,60 give expectations 2,4,5,6: first two merge.
    let mut data = Vec::new();
    for (s, n) in [20, 40, 50, 60].into_iter().enumerate() {
        data.extend(vec![s as u8; n]);
    }
    for grouping in [
        ChiSquareGrouping::ExactCounts,
        ChiSquareGrouping::NistReference,
    ] {
        let r = with_options(
            &data,
            ChiSquareOptions {
                grouping,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.symbol_bins, [0, 0, 1, 2]);
        assert_eq!(r.degrees_of_freedom, 18);
    }
    // Five equal expectations of three: two bins, final three merges backward.
    let r = test(&[0, 1, 2, 3, 4].repeat(30)).unwrap();
    assert_eq!(r.symbol_bins, [0, 0, 1, 1, 1]);
    assert_eq!(
        r.expected,
        vec![Float::with_val(256, 6), Float::with_val(256, 9)]
    );
}
#[test]
fn remainder_affects_expectations_only() {
    let mut data = [0, 1].repeat(50);
    data.push(1);
    let r = test(&data).unwrap();
    assert_eq!(r.discarded_samples, 1);
    assert_eq!(r.observed, vec![vec![5, 5]; 10]);
    assert_eq!(
        r.expected[0],
        Float::with_val(256, Float::with_val(256, 500) / 101)
    );
    // Ten blocks each contribute 1/1020, giving T=1/102.
    assert!(
        Float::with_val(
            256,
            r.statistic - Float::with_val(256, Float::with_val(256, 1) / 102)
        )
        .abs()
            < 1e-74
    );
}
#[test]
fn errors_and_binary_small_sample_reference_behavior() {
    assert!(matches!(
        test(&[0; 9]),
        Err(ChiSquareError::TooFewSubsequenceSamples { count: 9 })
    ));
    assert!(matches!(
        test(&[0; 100]),
        Err(ChiSquareError::ConstantInput)
    ));
    assert!(matches!(
        test(&[0, 1, 2].repeat(10)),
        Err(ChiSquareError::InsufficientGoodnessOfFitBins { bins: 1 })
    ));
    assert!(matches!(
        with_options(
            &[0; 100],
            ChiSquareOptions {
                precision_bits: 32,
                ..Default::default()
            }
        ),
        Err(ChiSquareError::InvalidPrecision { .. })
    ));
    // The binary reference does not pool or require expected counts >=5.
    assert_eq!(test(&[0, 1].repeat(5)).unwrap().degrees_of_freedom, 9);
}
#[test]
fn selected_precision() {
    let data = [0, 0, 1].repeat(100);
    let a = test(&data).unwrap();
    let b = with_options(
        &data,
        ChiSquareOptions {
            precision_bits: 512,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(b.p_value.prec(), 512);
    assert!(Float::with_val(512, a.p_value - b.p_value).abs() < 1e-73);
}
