use rug::Float;
use sp_800_90b::{
    ConfidenceConstant, EstimateError, LagOptions, lag_estimate, lag_estimate_with_options,
};
fn oracle(s: &[u8], d: usize) -> (usize, usize, Vec<usize>) {
    let mut scores = vec![0; d.min(s.len() - 1)];
    let (mut winner, mut correct, mut run, mut longest) = (1, 0, 0, 0);
    for i in 1..s.len() {
        if s[i] == s[i - winner] {
            correct += 1;
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
        for lag in 1..=d.min(i) {
            if s[i] == s[i - lag] {
                scores[lag - 1] += 1;
                if scores[lag - 1] >= scores[winner - 1] {
                    winner = lag;
                }
            }
        }
    }
    (correct, longest, scores)
}
#[test]
fn publication_example() {
    let r = lag_estimate_with_options(
        &[2, 1, 3, 2, 1, 3, 1, 3, 1, 2],
        LagOptions {
            max_lag: 3,
            confidence_constant: ConfidenceConstant::Publication,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.prediction_count, 9);
    assert_eq!(r.correct_count, 2);
    assert_eq!(r.longest_correct_run, 2);
    assert_eq!(r.scoreboard, [0, 3, 3]);
    let p = 2.0_f64 / 9.0;
    let expected = -(p + 2.576 * (p * (1.0 - p) / 8.0).sqrt()).log2();
    assert!((r.min_entropy.to_f64() - expected).abs() < 1e-14);
}
#[test]
fn position_queues_match_direct_prediction() {
    let mut state = 103u64;
    for alphabet in [2, 3, 17, 256] {
        for d in [1, 3, 128, 1024] {
            let data: Vec<u8> = (0..650)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    (state % alphabet) as u8
                })
                .collect();
            let r = lag_estimate_with_options(
                &data,
                LagOptions {
                    max_lag: d,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(
                (r.correct_count, r.longest_correct_run, r.scoreboard),
                oracle(&data, d)
            );
        }
    }
    for data in [
        vec![255; 1000],
        [0, 1].repeat(500),
        [vec![1; 500], vec![2; 500], vec![1; 500]].concat(),
        (0..=255).cycle().take(1500).collect(),
    ] {
        let r = lag_estimate(&data).unwrap();
        assert_eq!(
            (r.correct_count, r.longest_correct_run, r.scoreboard),
            oracle(&data, 128)
        );
    }
}
#[test]
fn minimum_input_constant_and_zero_success() {
    let r = lag_estimate(&[7; 3]).unwrap();
    assert_eq!(r.scoreboard, [2, 1]);
    assert_eq!(r.correct_count, 2);
    assert_eq!(r.min_entropy, 0);
    let r = lag_estimate(&(0..=255).collect::<Vec<_>>()).unwrap();
    assert_eq!(r.correct_count, 0);
    assert_eq!(r.longest_correct_run, 0);
    assert!(r.scoreboard.iter().all(|&x| x == 0));
    assert!((r.p_global_upper.to_f64() - (1.0 - 0.01_f64.powf(1.0 / 255.0))).abs() < 1e-15);
    assert!(r.min_entropy.is_finite());
}
#[test]
fn validation_and_inactive_lags() {
    for n in 0usize..3 {
        assert_eq!(
            lag_estimate(&vec![0; n]).unwrap_err(),
            EstimateError::TooFewPredictions {
                count: n.saturating_sub(1)
            }
        );
    }
    assert_eq!(
        lag_estimate_with_options(
            &[0; 3],
            LagOptions {
                max_lag: 0,
                ..Default::default()
            }
        )
        .unwrap_err(),
        EstimateError::InvalidMaxLag
    );
    assert_eq!(
        lag_estimate_with_options(
            &[0; 3],
            LagOptions {
                precision_bits: 0,
                ..Default::default()
            }
        )
        .unwrap_err(),
        EstimateError::InvalidPrecision { bits: 0 }
    );
    let r = lag_estimate_with_options(
        &[0, 1, 0],
        LagOptions {
            max_lag: usize::MAX,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.scoreboard, [0, 1]);
}
#[test]
fn relabeling_and_precision_stability() {
    let data = [0, 0, 1, 2, 1, 1, 2, 0, 2, 0].repeat(30);
    let a = lag_estimate(&data).unwrap();
    let b = lag_estimate_with_options(
        &data
            .iter()
            .map(|&x| [255, 17, 0][x as usize])
            .collect::<Vec<_>>(),
        LagOptions {
            precision_bits: 512,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(a.correct_count, b.correct_count);
    assert_eq!(a.scoreboard, b.scoreboard);
    assert!(
        (Float::with_val(512, a.min_entropy) - b.min_entropy).abs()
            < Float::with_val(512, Float::parse("1e-65").unwrap())
    );
}
