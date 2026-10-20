#![cfg(feature = "assessment")]

use sp_800_90b::{
    ConfidenceConstant, EstimateError, MultiMcwOptions, multi_mcw_estimate,
    multi_mcw_estimate_with_options,
};
fn small() -> MultiMcwOptions {
    MultiMcwOptions {
        windows: [3, 5, 7, 9],
        confidence_constant: ConfidenceConstant::Publication,
        ..Default::default()
    }
}
// Independent implementation: reconstruct each active window from scratch.
fn oracle(s: &[u8], w: [usize; 4]) -> (usize, usize, [usize; 4]) {
    let (mut winner, mut c, mut run, mut longest) = (0, 0, 0, 0);
    let mut score = [0; 4];
    for i in w[0]..s.len() {
        let mut predictions = [None; 4];
        for j in 0..4 {
            if i < w[j] {
                continue;
            }
            let window = &s[i - w[j]..i];
            let mut counts = [0; 256];
            for &x in window {
                counts[x as usize] += 1;
            }
            let max = *counts.iter().max().unwrap();
            predictions[j] = window
                .iter()
                .rev()
                .copied()
                .find(|&x| counts[x as usize] == max);
        }
        if predictions[winner] == Some(s[i]) {
            c += 1;
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
        for j in 0..4 {
            if predictions[j] == Some(s[i]) {
                score[j] += 1;
                if score[j] >= score[winner] {
                    winner = j;
                }
            }
        }
    }
    (c, longest, score)
}
#[test]
fn publication_example() {
    let r =
        multi_mcw_estimate_with_options(&[1, 2, 1, 0, 2, 1, 1, 2, 2, 0, 0, 0], small()).unwrap();
    assert_eq!(r.prediction_count, 9);
    assert_eq!(r.correct_count, 3);
    assert_eq!(r.longest_correct_run, 1);
    assert_eq!(r.scoreboard, [2, 3, 1, 1]);
    let expected = -(1.0 / 3.0 + 2.576_f64 * ((1.0 / 3.0) * (2.0 / 3.0) / 8.0_f64).sqrt()).log2();
    assert!((r.min_entropy.to_f64() - expected).abs() < 1e-14);
}
#[test]
fn optimized_predictions_match_rebuilt_windows() {
    let mut state = 103u64;
    for alphabet in [2, 3, 8, 256] {
        for _ in 0..12 {
            let data: Vec<u8> = (0..120)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    (state % alphabet) as u8
                })
                .collect();
            let r = multi_mcw_estimate_with_options(&data, small()).unwrap();
            assert_eq!(
                (r.correct_count, r.longest_correct_run, r.scoreboard),
                oracle(&data, small().windows)
            );
        }
    }
    // Long constant runs, changes of mode, and alternating ties.
    let data = [
        vec![0; 1500],
        vec![255; 1500],
        vec![17; 1500],
        [0, 255].repeat(1000),
    ]
    .concat();
    let r = multi_mcw_estimate(&data).unwrap();
    assert_eq!(
        (r.correct_count, r.longest_correct_run, r.scoreboard),
        oracle(&data, MultiMcwOptions::default().windows)
    );
}
#[test]
fn constant_and_zero_success() {
    let r = multi_mcw_estimate(&[255; 4096]).unwrap();
    assert_eq!(r.prediction_count, 4033);
    assert_eq!(r.correct_count, 4033);
    assert_eq!(r.longest_correct_run, 4033);
    assert_eq!(r.scoreboard, [4033, 3841, 3073, 1]);
    assert_eq!(r.min_entropy, 0);
    let r = multi_mcw_estimate_with_options(&(0..20).collect::<Vec<_>>(), small()).unwrap();
    assert_eq!(r.correct_count, 0);
    assert_eq!(r.longest_correct_run, 0);
    assert!((r.p_global_upper.to_f64() - (1.0 - 0.01_f64.powf(1.0 / 17.0))).abs() < 1e-15);
    assert!(r.min_entropy.is_finite());
}
#[test]
fn validation() {
    for n in [0, 1, 4095] {
        assert!(matches!(
            multi_mcw_estimate(&vec![0; n]),
            Err(EstimateError::TooFewPredictionSamples { .. })
        ));
    }
    for windows in [[0, 3, 5, 7], [3, 3, 5, 7], [9, 7, 5, 3]] {
        assert_eq!(
            multi_mcw_estimate_with_options(&[0; 20], MultiMcwOptions { windows, ..small() })
                .unwrap_err(),
            EstimateError::InvalidPredictionWindows
        );
    }
    assert_eq!(
        multi_mcw_estimate_with_options(
            &[0; 20],
            MultiMcwOptions {
                precision_bits: 0,
                ..small()
            }
        )
        .unwrap_err(),
        EstimateError::InvalidPrecision { bits: 0 }
    );
}
#[test]
fn symbol_relabeling() {
    let data = [0, 0, 1, 2, 1, 2, 1, 0, 2, 2, 1, 1].repeat(20);
    let a = multi_mcw_estimate_with_options(&data, small()).unwrap();
    let b = multi_mcw_estimate_with_options(
        &data
            .iter()
            .map(|&x| [255, 0, 17][x as usize])
            .collect::<Vec<_>>(),
        small(),
    )
    .unwrap();
    assert_eq!(a.correct_count, b.correct_count);
    assert_eq!(a.scoreboard, b.scoreboard);
    assert_eq!(a.min_entropy, b.min_entropy);
}
