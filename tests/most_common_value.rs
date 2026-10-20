use rug::Float;
use sp_800_90b::{
    ConfidenceConstant, EstimateError, McvOptions, most_common_value,
    most_common_value_with_options,
};

const PAPER_DATA: [u8; 20] = [0, 1, 1, 2, 0, 1, 2, 2, 0, 1, 0, 1, 1, 0, 2, 2, 1, 0, 2, 1];

#[test]
fn potential_errata_2025_section_6_3_1_example_preserves_l_minus_one() {
    // NIST's May 29, 2025 potential errata correct the worked example:
    // https://csrc.nist.gov/files/pubs/sp/800/90/b/final/docs/sp800-90b_errata_potential_updates.pdf
    // The proposed 0.6822 / 0.5518 match its printed variance 0.012, but that
    // variance is p_hat * (1 - p_hat) / L. The specified algorithm uses L - 1
    // and still gives 0.6895 / 0.5363 for these 20 samples and z=2.576.
    let decimal = |s: &str| Float::with_val(256, Float::parse(s).unwrap());
    let result = most_common_value_with_options(
        &PAPER_DATA,
        McvOptions {
            confidence_constant: ConfidenceConstant::Publication,
            ..McvOptions::default()
        },
    )
    .unwrap();
    assert_eq!(result.sample_count, 20);
    assert_eq!(result.mode_count, 8);
    assert_eq!(result.p_hat, decimal("0.4"));

    // Independent Python decimal evaluation at 85 digits of the L - 1 formula.
    let expected_probability = decimal(
        "0.6895174060761332387268472182671005323505597188855837302485010649092531278093266005464",
    );
    let expected_entropy = decimal(
        "0.5363411235066874276409024703312270483989643600248889096933013321973256073213488095637",
    );
    assert!((result.p_upper - expected_probability).abs() < decimal("1e-74"));
    assert!((result.min_entropy - expected_entropy).abs() < decimal("1e-74"));

    // Separately verify the proposal's arithmetic to its printed four places.
    // These are not the expected outputs of the estimator's L - 1 formula.
    let printed_probability = decimal("0.4") + decimal("2.576") * decimal("0.012").sqrt();
    let printed_entropy = -printed_probability.clone().log2();
    assert!((printed_probability - decimal("0.6822")).abs() < decimal("0.00005"));
    assert!((printed_entropy - decimal("0.5518")).abs() < decimal("0.00005"));
}

#[test]
fn matches_independent_100_digit_decimal_calculation() {
    // Independently evaluated using Python decimal (100 digits), not MPFR.
    for (constant, expected) in [
        (
            ConfidenceConstant::NistReference,
            "0.5363812645502801660283848648085645521357439676315503911210845126435556275781657872671955321972046879",
        ),
        (
            ConfidenceConstant::Publication,
            "0.5363411235066874276409024703312270483989643600248889096933013321973256073213488095639771043014422995",
        ),
    ] {
        let result = most_common_value_with_options(
            &PAPER_DATA,
            McvOptions {
                confidence_constant: constant,
                ..McvOptions::default()
            },
        )
        .unwrap();
        let expected = Float::with_val(256, Float::parse(expected).unwrap());
        let error = (result.min_entropy - expected).abs();
        assert!(error < Float::with_val(256, Float::parse("1e-74").unwrap()));
        assert_eq!(result.mode_count, 8);
        assert_eq!(result.sample_count, 20);
    }
}

#[test]
fn constant_samples_have_exact_zero_entropy() {
    for symbol in [0, 7, 255] {
        let result = most_common_value(&[symbol; 100]).unwrap();
        assert_eq!(result.min_entropy, 0);
        assert_eq!(result.p_upper, 1);
        assert_eq!(result.mode_count, 100);
    }
}

#[test]
fn confidence_bound_is_clipped() {
    let result = most_common_value(&[0, 1]).unwrap();
    assert_eq!(result.p_hat, 0.5);
    assert_eq!(result.p_upper, 1);
    assert_eq!(result.min_entropy, 0);
}

#[test]
fn rejects_insufficient_samples_and_invalid_precision() {
    for data in [&[][..], &[1][..]] {
        assert_eq!(
            most_common_value(data).unwrap_err(),
            EstimateError::TooFewSamples { count: data.len() }
        );
    }
    for bits in [0, 53, 63] {
        let result = most_common_value_with_options(
            &PAPER_DATA,
            McvOptions {
                precision_bits: bits,
                ..McvOptions::default()
            },
        );
        assert_eq!(
            result.unwrap_err(),
            EstimateError::InvalidPrecision { bits }
        );
    }
}

#[test]
fn supports_full_byte_alphabet_and_symbol_relabeling() {
    let samples: Vec<u8> = (0..=255).cycle().take(25600).collect();
    let result = most_common_value(&samples).unwrap();
    assert_eq!(result.mode_count, 100);
    assert!(result.min_entropy > 7);
    assert!(result.min_entropy < 8);
    let a = most_common_value(&PAPER_DATA).unwrap();
    let b = most_common_value(&PAPER_DATA.map(|x| [255, 77, 9][x as usize])).unwrap();
    assert_eq!(a.min_entropy, b.min_entropy);
}

#[test]
fn higher_precision_preserves_default_result() {
    let default = most_common_value(&PAPER_DATA).unwrap();
    let higher = most_common_value_with_options(
        &PAPER_DATA,
        McvOptions {
            precision_bits: 512,
            ..McvOptions::default()
        },
    )
    .unwrap();
    assert_eq!(default.min_entropy.prec(), 256);
    assert_eq!(higher.min_entropy.prec(), 512);
    let difference = (Float::with_val(512, default.min_entropy) - higher.min_entropy).abs();
    assert!(difference < Float::with_val(512, Float::parse("1e-74").unwrap()));
}
