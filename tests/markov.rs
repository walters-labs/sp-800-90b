use rug::Float;
use sp_800_90b::{EstimateError, MarkovOptions, markov_estimate, markov_estimate_with_options};
const DATA: [u8; 40] = [
    1, 0, 0, 0, 1, 1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 1, 0,
    1, 0, 1, 0, 1, 1, 1, 0,
];

#[test]
fn paper_example_matches_independent_decimal_result() {
    let r = markov_estimate(&DATA).unwrap();
    assert_eq!(r.symbol_counts, [19, 21]);
    assert_eq!(r.transition_counts, [[7, 11], [12, 9]]);
    // Python Decimal, 100 digits: -(log2((21/40)*(12/21)^64*(11/18)^63))/128.
    let expected=Float::with_val(256,Float::parse("0.7606360062539938801396240817566159243509587786914077659998961964775359194285972457797069546935074148").unwrap());
    assert!(
        (r.min_entropy - expected).abs() < Float::with_val(256, Float::parse("1e-74").unwrap())
    );
}

#[test]
fn constant_and_alternating_inputs() {
    for bit in [0, 1] {
        assert_eq!(markov_estimate(&[bit; 100]).unwrap().min_entropy, 0);
    }
    assert_eq!(
        markov_estimate(&[0, 1].repeat(100)).unwrap().min_entropy,
        1.0 / 128.0
    );
}

#[test]
fn missing_rows_and_impossible_chains() {
    for data in [[0, 1], [1, 0]] {
        let r = markov_estimate(&data).unwrap();
        assert!(r.sequence_entropies.iter().all(Option::is_none));
        assert_eq!(r.min_entropy, 1);
    }
    let r = markov_estimate(&[0, 0, 1]).unwrap();
    assert_eq!(r.transition_counts, [[1, 1], [0, 0]]);
    assert_eq!(r.transition_probabilities[1][0], 0);
    assert_eq!(r.transition_probabilities[1][1], 0);
    assert!(r.min_entropy < 1);
}

#[test]
fn rejects_invalid_input() {
    for data in [&[][..], &[0][..]] {
        assert_eq!(
            markov_estimate(data).unwrap_err(),
            EstimateError::TooFewSamples { count: data.len() }
        );
    }
    assert_eq!(
        markov_estimate(&[0, 1, 255]).unwrap_err(),
        EstimateError::NonBinarySample {
            index: 2,
            value: 255
        }
    );
    assert_eq!(
        markov_estimate_with_options(&DATA, MarkovOptions { precision_bits: 53 }).unwrap_err(),
        EstimateError::InvalidPrecision { bits: 53 }
    );
}

#[test]
fn precision_and_symbol_symmetry() {
    let a = markov_estimate(&DATA).unwrap();
    let b = markov_estimate_with_options(
        &DATA,
        MarkovOptions {
            precision_bits: 512,
        },
    )
    .unwrap();
    assert_eq!(b.min_entropy.prec(), 512);
    assert!(
        (Float::with_val(512, a.min_entropy.clone()) - b.min_entropy).abs()
            < Float::with_val(512, Float::parse("1e-74").unwrap())
    );
    assert_eq!(
        markov_estimate(&DATA.map(|x| 1 - x)).unwrap().min_entropy,
        a.min_entropy
    );
}

#[test]
fn agrees_with_explicit_candidate_sequences_for_every_eight_bit_input() {
    // Multiply transitions along explicit sequences, independently of the log formulas.
    for pattern in 0..256u32 {
        let data: Vec<u8> = (0..8).map(|i| ((pattern >> i) & 1) as u8).collect();
        let mut counts = [0usize; 2];
        let mut pairs = [[0usize; 2]; 2];
        for &bit in &data {
            counts[bit as usize] += 1;
        }
        for pair in data.windows(2) {
            pairs[pair[0] as usize][pair[1] as usize] += 1;
        }
        let transition: [[f64; 2]; 2] = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                let total = pairs[i][0] + pairs[i][1];
                if total == 0 {
                    0.0
                } else {
                    pairs[i][j] as f64 / total as f64
                }
            })
        });
        let mut zero_then_ones = vec![1u8; 128];
        zero_then_ones[0] = 0;
        let mut one_then_zeros = vec![0u8; 128];
        one_then_zeros[0] = 1;
        let candidates = [
            vec![0; 128],
            (0..128).map(|i| (i % 2) as u8).collect(),
            zero_then_ones,
            one_then_zeros,
            (0..128).map(|i| (1 - i % 2) as u8).collect(),
            vec![1; 128],
        ];
        let p = candidates
            .iter()
            .map(|sequence| {
                let mut probability = counts[sequence[0] as usize] as f64 / 8.0;
                for pair in sequence.windows(2) {
                    probability *= transition[pair[0] as usize][pair[1] as usize];
                }
                probability
            })
            .fold(0.0_f64, f64::max);
        let expected = if p == 0.0 {
            1.0
        } else {
            (-p.log2() / 128.0).min(1.0)
        };
        let actual = markov_estimate(&data).unwrap().min_entropy.to_f64();
        assert!(
            (actual - expected).abs() < 1e-12,
            "pattern={pattern}: {actual} vs {expected}"
        );
    }
}
