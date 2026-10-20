#![cfg(feature = "assessment")]

use sp_800_90b::{
    ConfidenceConstant, EstimateError, MultiMmcOptions, multi_mmc_estimate,
    multi_mmc_estimate_with_options,
};
use std::collections::BTreeMap;

// Train before prediction using explicit (Vec<context>, successor) counters.
// This is independent of the implementation's preloaded, predict-then-train models.
fn oracle(s: &[u8], depth: usize, cap: usize) -> (usize, usize, Vec<usize>) {
    let depth = depth.min(s.len() - 2);
    let mut models: Vec<BTreeMap<(Vec<u8>, u8), usize>> = vec![BTreeMap::new(); depth];
    let mut scores = vec![0; depth];
    let (mut winner, mut correct, mut run, mut longest) = (0, 0, 0, 0);
    for i in 2..s.len() {
        let mut predictions = vec![None; depth];
        for d in 1..=depth.min(i - 1) {
            let model = &mut models[d - 1];
            let pair = (s[i - d - 1..i - 1].to_vec(), s[i - 1]);
            if let Some(count) = model.get_mut(&pair) {
                *count += 1;
            } else if model.len() < cap {
                model.insert(pair, 1);
            }
            predictions[d - 1] = model
                .iter()
                .filter(|((context, _), _)| context.as_slice() == &s[i - d..i])
                .map(|((_, symbol), &count)| (count, *symbol))
                .max()
                .map(|(_, symbol)| symbol);
        }
        if let Some(predicted) = predictions[winner] {
            if predicted == s[i] {
                correct += 1;
                run += 1;
                longest = longest.max(run);
            } else {
                run = 0;
            }
        }
        for d in 0..depth {
            if predictions[d] == Some(s[i]) {
                scores[d] += 1;
                if scores[d] >= scores[winner] {
                    winner = d;
                }
            }
        }
    }
    (correct, longest, scores)
}
#[test]
fn publication_example() {
    let r = multi_mmc_estimate_with_options(
        &[2, 1, 3, 2, 1, 3, 1, 3, 1],
        MultiMmcOptions {
            max_depth: 3,
            confidence_constant: ConfidenceConstant::Publication,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.prediction_count, 7);
    assert_eq!(r.correct_count, 3);
    assert_eq!(r.longest_correct_run, 2);
    assert_eq!(r.scoreboard, [3, 1, 0]);
    let p = 3.0_f64 / 7.0;
    assert!(
        (r.min_entropy.to_f64() + (p + 2.576 * (p * (1.0 - p) / 6.0).sqrt()).log2()).abs() < 1e-14
    );
}
#[test]
fn models_match_chronological_oracle_with_saturation() {
    let mut state = 103u64;
    for alphabet in [2, 3, 17, 256] {
        for depth in [1, 3, 16] {
            for cap in [1, 2, 7, 100000] {
                let data: Vec<u8> = (0..110)
                    .map(|_| {
                        state ^= state << 13;
                        state ^= state >> 7;
                        state ^= state << 17;
                        (state % alphabet) as u8
                    })
                    .collect();
                let r = multi_mmc_estimate_with_options(
                    &data,
                    MultiMmcOptions {
                        max_depth: depth,
                        max_entries: cap,
                        ..Default::default()
                    },
                )
                .unwrap();
                assert_eq!(
                    (r.correct_count, r.longest_correct_run, r.scoreboard),
                    oracle(&data, depth, cap),
                    "alphabet={alphabet} depth={depth} cap={cap}"
                );
                assert!(r.entries.iter().all(|&n| n <= cap));
            }
        }
    }
}
#[test]
fn constant_minimum_and_zero_success() {
    let r = multi_mmc_estimate(&[255; 4]).unwrap();
    assert_eq!(r.correct_count, 2);
    assert_eq!(r.entries, [1, 1]);
    assert_eq!(r.scoreboard, [2, 1]);
    assert_eq!(r.min_entropy, 0);
    let r = multi_mmc_estimate(&(0..=255).collect::<Vec<_>>()).unwrap();
    assert_eq!(r.correct_count, 0);
    assert_eq!(r.longest_correct_run, 0);
    assert_eq!(r.p_global, 0);
    assert!(r.min_entropy.is_finite());
}
#[test]
fn validation() {
    for n in 0usize..4 {
        assert_eq!(
            multi_mmc_estimate(&vec![0; n]).unwrap_err(),
            EstimateError::TooFewPredictions {
                count: n.saturating_sub(2)
            }
        );
    }
    for max_depth in [0, 17, usize::MAX] {
        assert_eq!(
            multi_mmc_estimate_with_options(
                &[0; 4],
                MultiMmcOptions {
                    max_depth,
                    ..Default::default()
                }
            )
            .unwrap_err(),
            EstimateError::InvalidMmcDepth
        );
    }
    assert_eq!(
        multi_mmc_estimate_with_options(
            &[0; 4],
            MultiMmcOptions {
                max_entries: 0,
                ..Default::default()
            }
        )
        .unwrap_err(),
        EstimateError::InvalidMmcEntryLimit
    );
    assert_eq!(
        multi_mmc_estimate_with_options(
            &[0; 4],
            MultiMmcOptions {
                precision_bits: 0,
                ..Default::default()
            }
        )
        .unwrap_err(),
        EstimateError::InvalidPrecision { bits: 0 }
    );
}

#[test]
fn unavailable_prediction_preserves_reference_run_counter() {
    let data = [2, 1, 0, 1, 0, 2, 2, 1, 0, 2, 1, 0, 1, 0, 1, 2, 0, 2];
    let r = multi_mmc_estimate_with_options(
        &data,
        MultiMmcOptions {
            max_depth: 3,
            max_entries: 2,
            ..Default::default()
        },
    )
    .unwrap();
    // True consecutive successes have maximum length 2; NIST retains the run
    // through unavailable predictions once the small dictionaries saturate.
    assert_eq!(r.correct_count, 6);
    assert_eq!(r.longest_correct_run, 5);
}

#[test]
fn precision_and_order_preserving_relabeling() {
    let data = [0, 0, 1, 2, 1, 2, 0, 1, 1, 2].repeat(15);
    let a = multi_mmc_estimate(&data).unwrap();
    // Symbol order matters: equal counts predict the greatest symbol.
    let b = multi_mmc_estimate_with_options(
        &data
            .iter()
            .map(|&x| [7, 100, 255][x as usize])
            .collect::<Vec<_>>(),
        MultiMmcOptions {
            precision_bits: 512,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(a.scoreboard, b.scoreboard);
    assert_eq!(a.entries, b.entries);
    assert_eq!(a.correct_count, b.correct_count);
    assert!(
        (rug::Float::with_val(512, a.min_entropy) - b.min_entropy).abs()
            < rug::Float::with_val(512, rug::Float::parse("1e-65").unwrap())
    );
}
