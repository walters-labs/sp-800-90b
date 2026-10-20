use rug::Float;
use sp_800_90b::{
    CompressionOptions, ConfidenceConstant, EstimateError, compression_estimate,
    compression_estimate_with_options,
};
fn bits(blocks: &[u8]) -> Vec<u8> {
    blocks
        .iter()
        .flat_map(|x| (0..6).rev().map(move |i| (x >> i) & 1))
        .collect()
}
fn example() -> Vec<u8> {
    bits(&[35, 37, 23, 12, 28, 42, 59, 35])
}
fn options() -> CompressionOptions {
    CompressionOptions {
        dictionary_blocks: 4,
        confidence_constant: ConfidenceConstant::Publication,
        ..Default::default()
    }
}

#[test]
fn published_example_matches_independent_decimal_oracle() {
    let r = compression_estimate_with_options(&example(), options()).unwrap();
    assert_eq!(r.block_count, 8);
    assert_eq!(r.test_blocks, 4);
    // Independent 100-digit Python Decimal evaluation of the literal double sum.
    let expected=Float::with_val(256,Float::parse("0.1345355440089077901734476930412262340815626402010248144102161798746341580067964364952142577451366292").unwrap());
    assert!(
        (r.min_entropy - expected).abs() < Float::with_val(256, Float::parse("1e-72").unwrap())
    );
}
#[test]
fn constant_blocks_and_nonpositive_bound() {
    for block in [0, 21, 63] {
        assert_eq!(
            compression_estimate(&bits(&[block; 1002]))
                .unwrap()
                .min_entropy,
            0
        );
    }
    let r = compression_estimate_with_options(
        &bits(&[0, 0, 1]),
        CompressionOptions {
            dictionary_blocks: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.lower_bound < 0);
    assert_eq!(r.min_entropy, 0);
}
#[test]
fn no_solution_above_uniform_model() {
    let data: Vec<u8> = (0..64).cycle().take(2048).collect();
    assert_eq!(compression_estimate(&bits(&data)).unwrap().min_entropy, 1);
}
#[test]
fn trailing_bits_and_complement_symmetry() {
    let data = example();
    let expected = compression_estimate_with_options(&data, options()).unwrap();
    for tail in 1..6 {
        let mut extended = data.clone();
        extended.extend(vec![1; tail]);
        let r = compression_estimate_with_options(&extended, options()).unwrap();
        assert_eq!(r.discarded_bits, tail);
        assert_eq!(r.min_entropy, expected.min_entropy);
    }
    let flipped: Vec<_> = data.iter().map(|x| 1 - x).collect();
    assert_eq!(
        compression_estimate_with_options(&flipped, options())
            .unwrap()
            .min_entropy,
        expected.min_entropy
    );
}
#[test]
fn validates_inputs() {
    for size in [0, 1, 5999, 6000, 6006, 6011] {
        assert!(matches!(
            compression_estimate(&vec![0; size]),
            Err(EstimateError::TooFewTestBlocks { .. })
        ));
    }
    let mut data = vec![0; 6013];
    data[6012] = 2;
    assert_eq!(
        compression_estimate(&data).unwrap_err(),
        EstimateError::NonBinarySample {
            index: 6012,
            value: 2
        }
    );
    assert_eq!(
        compression_estimate_with_options(
            &[],
            CompressionOptions {
                dictionary_blocks: 0,
                ..Default::default()
            }
        )
        .unwrap_err(),
        EstimateError::InvalidDictionarySize
    );
    assert!(matches!(
        compression_estimate_with_options(
            &[],
            CompressionOptions {
                dictionary_blocks: usize::MAX,
                ..Default::default()
            }
        ),
        Err(EstimateError::TooFewTestBlocks { .. })
    ));
    assert_eq!(
        compression_estimate_with_options(
            &[],
            CompressionOptions {
                precision_bits: 0,
                ..Default::default()
            }
        )
        .unwrap_err(),
        EstimateError::InvalidPrecision { bits: 0 }
    );
}
#[test]
fn precision_stability() {
    let a = compression_estimate_with_options(&example(), options()).unwrap();
    let b = compression_estimate_with_options(
        &example(),
        CompressionOptions {
            precision_bits: 512,
            ..options()
        },
    )
    .unwrap();
    assert_eq!(b.min_entropy.prec(), 512);
    assert!(
        (Float::with_val(512, a.min_entropy) - b.min_entropy).abs()
            < Float::with_val(512, Float::parse("1e-72").unwrap())
    );
}
