use rug::Float;
use sp_800_90b::{
    ConfidenceConstant, EstimateError, Lz78yOptions, lz78y_estimate, lz78y_estimate_with_options,
};
use std::collections::BTreeMap;
// Independent chronological algorithm: update all prior contexts, then predict.
fn oracle(s: &[u8], b: usize, cap: usize) -> (usize, usize, Vec<usize>) {
    let mut dict: BTreeMap<Vec<u8>, BTreeMap<u8, usize>> = BTreeMap::new();
    let (mut correct, mut run, mut longest) = (0, 0, 0);
    for i in b + 1..=s.len() {
        for length in (1..=b).rev() {
            let context = s[i - length - 1..i - 1].to_vec();
            if dict.contains_key(&context) || dict.len() < cap {
                *dict
                    .entry(context)
                    .or_default()
                    .entry(s[i - 1])
                    .or_default() += 1;
            }
        }
        // Include the final model update, matching the public diagnostics.
        if i == s.len() {
            break;
        }
        let mut candidates = Vec::new();
        for length in 1..=b {
            if let Some(counts) = dict.get(&s[i - length..i]) {
                let (&symbol, &count) = counts
                    .iter()
                    .max_by_key(|&(symbol, count)| (count, symbol))
                    .unwrap();
                candidates.push((count, length, symbol));
            }
        }
        let prediction = candidates.into_iter().max().map(|(_, _, symbol)| symbol);
        if prediction == Some(s[i]) {
            correct += 1;
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let sizes = (1..=b)
        .map(|length| dict.keys().filter(|key| key.len() == length).count())
        .collect();
    (correct, longest, sizes)
}
#[test]
fn publication_example() {
    let r = lz78y_estimate_with_options(
        &[2, 1, 3, 2, 1, 3, 1, 3, 1, 2, 1, 3, 2],
        Lz78yOptions {
            context_length: 4,
            confidence_constant: ConfidenceConstant::Publication,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.prediction_count, 8);
    assert_eq!(r.correct_count, 4);
    assert_eq!(r.longest_correct_run, 2);
    let expected = -(0.5 + 2.576_f64 * (0.25_f64 / 7.0).sqrt()).log2();
    assert!((r.min_entropy.to_f64() - expected).abs() < 1e-14);
}
#[test]
fn packed_models_match_independent_chronological_models() {
    let mut state = 103u64;
    for alphabet in [2, 3, 17, 256] {
        for b in [1, 4, 16] {
            for cap in [b, b + 1, 64, 65536] {
                let data: Vec<u8> = (0..120)
                    .map(|_| {
                        state ^= state << 13;
                        state ^= state >> 7;
                        state ^= state << 17;
                        (state % alphabet) as u8
                    })
                    .collect();
                let r = lz78y_estimate_with_options(
                    &data,
                    Lz78yOptions {
                        context_length: b,
                        max_dictionary_size: cap,
                        ..Default::default()
                    },
                )
                .unwrap();
                assert_eq!(
                    (
                        r.correct_count,
                        r.longest_correct_run,
                        r.contexts_per_length.clone()
                    ),
                    oracle(&data, b, cap),
                    "alphabet={alphabet}, B={b}, cap={cap}"
                );
                assert!(r.dictionary_size <= cap);
                assert_eq!(
                    r.dictionary_size,
                    r.contexts_per_length.iter().sum::<usize>()
                );
            }
        }
    }
}
#[test]
fn saturated_contexts_learn_new_successors_and_null_resets_run() {
    let options = Lz78yOptions {
        context_length: 1,
        max_dictionary_size: 1,
        ..Default::default()
    };
    let r = lz78y_estimate_with_options(&[0, 0, 1, 0, 1, 0, 1, 0, 1], options).unwrap();
    assert_eq!(r.dictionary_size, 1);
    assert_eq!(r.correct_count, 3);
    assert_eq!(r.longest_correct_run, 1);
    let r = lz78y_estimate_with_options(&[0, 1, 0, 1, 0, 1], options).unwrap();
    assert_eq!(r.correct_count, 2);
    assert_eq!(r.longest_correct_run, 1);
}
#[test]
fn minimum_constant_and_unique_inputs() {
    let r = lz78y_estimate(&[255; 19]).unwrap();
    assert_eq!(r.correct_count, 2);
    assert_eq!(r.longest_correct_run, 2);
    assert_eq!(r.contexts_per_length, vec![1; 16]);
    assert_eq!(r.min_entropy, 0);
    let r = lz78y_estimate(&(0..=255).collect::<Vec<_>>()).unwrap();
    assert_eq!(r.correct_count, 0);
    assert_eq!(r.longest_correct_run, 0);
    assert!((r.p_global_upper.to_f64() - (1.0 - 0.01_f64.powf(1.0 / 239.0))).abs() < 1e-15);
    assert!(r.min_entropy.is_finite());
}
#[test]
fn validation() {
    for n in [0usize, 1, 16, 17, 18] {
        assert_eq!(
            lz78y_estimate(&vec![0; n]).unwrap_err(),
            EstimateError::TooFewPredictions {
                count: n.saturating_sub(17)
            }
        );
    }
    for context_length in [0, 17, usize::MAX] {
        assert_eq!(
            lz78y_estimate_with_options(
                &[0; 19],
                Lz78yOptions {
                    context_length,
                    ..Default::default()
                }
            )
            .unwrap_err(),
            EstimateError::InvalidLz78yContextLength
        );
    }
    for max_dictionary_size in [0, 15] {
        assert_eq!(
            lz78y_estimate_with_options(
                &[0; 19],
                Lz78yOptions {
                    max_dictionary_size,
                    ..Default::default()
                }
            )
            .unwrap_err(),
            EstimateError::InvalidLz78yDictionarySize
        );
    }
    assert_eq!(
        lz78y_estimate_with_options(
            &[0; 19],
            Lz78yOptions {
                precision_bits: 0,
                ..Default::default()
            }
        )
        .unwrap_err(),
        EstimateError::InvalidPrecision { bits: 0 }
    );
}
#[test]
fn precision_and_order_preserving_relabeling() {
    let data = [0, 0, 1, 2, 1, 2, 0, 1, 1, 2].repeat(30);
    let a = lz78y_estimate(&data).unwrap();
    let b = lz78y_estimate_with_options(
        &data
            .iter()
            .map(|&x| [7, 100, 255][x as usize])
            .collect::<Vec<_>>(),
        Lz78yOptions {
            precision_bits: 512,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(a.correct_count, b.correct_count);
    assert_eq!(a.contexts_per_length, b.contexts_per_length);
    assert!(
        (Float::with_val(512, a.min_entropy) - b.min_entropy).abs()
            < Float::with_val(512, Float::parse("1e-65").unwrap())
    );
}

#[test]
fn longer_context_and_greater_symbol_win_ties() {
    let options = Lz78yOptions {
        context_length: 2,
        ..Default::default()
    };
    // Choosing the shorter tied context would produce only one success.
    let r = lz78y_estimate_with_options(&[0, 0, 0, 1, 1, 2, 0, 1, 1], options).unwrap();
    assert_eq!(r.correct_count, 2);
    // Choosing the smaller tied symbol would produce three successes.
    let r = lz78y_estimate_with_options(&[0, 0, 0, 0, 1, 0, 1, 0, 0], options).unwrap();
    assert_eq!(r.correct_count, 2);
}
