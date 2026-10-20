#![cfg(feature = "assessment")]

use sp_800_90b::*;
#[test]
fn hand_computed_periodic_statistics() {
    let data = [0, 1, 2].repeat(100);
    let s = permutation_statistics(&data, PermutationArithmetic::ExactCounts).unwrap();
    assert_eq!(s.len(), 19);
    assert_eq!(s[0], 1); // Prefix sums deviate from mean 1 by at most one.
    assert_eq!(s[1], 199);
    assert_eq!(s[2], 2);
    assert_eq!(s[3], 200);
    assert_eq!(s[4], 200);
    assert_eq!(s[5], 2);
    assert_eq!(s[6], 4);
    assert_eq!(s[7], 4);
    for x in &s[8..13] {
        assert_eq!(x, &0);
    }
    assert_eq!(s[13], 200);
}
#[test]
fn raw_values_are_preserved_where_required() {
    let a = [0, 1, 2].repeat(100);
    let b = [10, 20, 30].repeat(100);
    let a = permutation_statistics(&a, PermutationArithmetic::ExactCounts).unwrap();
    let b = permutation_statistics(&b, PermutationArithmetic::ExactCounts).unwrap();
    assert_eq!(b[0], 10);
    assert_eq!(a[1..13], b[1..13]);
    assert_ne!(a[13], b[13]);
}
#[test]
fn invalid_inputs() {
    assert!(matches!(
        permutation_tests(&[0; 300]),
        Err(PermutationError::ConstantInput)
    ));
    assert!(matches!(
        permutation_tests(&[0, 1].repeat(100)),
        Err(PermutationError::TooFewSamples { .. })
    ));
    assert!(matches!(
        permutation_tests_with_options(
            &[0, 1, 2].repeat(100),
            PermutationOptions {
                seed: Some([0; 4]),
                ..Default::default()
            }
        ),
        Err(PermutationError::ZeroSeed)
    ));
}
#[test]
fn structured_sequence_rejects_and_reproduces() {
    let data = [0, 1, 2].repeat(100);
    let options = PermutationOptions {
        seed: Some([1, 2, 3, 4]),
        ..Default::default()
    };
    let a = permutation_tests_with_options(&data, options).unwrap();
    let b = permutation_tests_with_options(&data, options).unwrap();
    assert!(a.reject);
    assert_eq!(a.permutations_performed, 10_000);
    for (a, b) in a.tests.iter().zip(b.tests) {
        assert_eq!((a.greater, a.equal, a.less), (b.greater, b.equal, b.less));
        assert_eq!(a.reject, a.greater + a.equal <= 5 || a.less + a.equal <= 5);
        if a.reject {
            assert_eq!(a.evaluated_permutations(), 10_000);
        }
    }
}

#[test]
fn early_stopping_preserves_full_run_decisions() {
    let mut state = 123456789u64;
    let data: Vec<u8> = (0..300)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % 7) as u8
        })
        .collect();
    let options = PermutationOptions {
        seed: Some([1, 2, 3, 4]),
        arithmetic: PermutationArithmetic::ExactCounts,
        ..Default::default()
    };
    let early = permutation_tests_with_options(&data, options).unwrap();
    let full = permutation_tests_with_options(
        &data,
        PermutationOptions {
            early_stop: false,
            ..options
        },
    )
    .unwrap();
    assert_eq!(early.reject, full.reject);
    assert!(early.permutations_performed < 10_000);
    assert_eq!(full.permutations_performed, 10_000);
    for (a, b) in early.tests.iter().zip(full.tests) {
        assert_eq!(a.reject, b.reject);
        assert_eq!(b.evaluated_permutations(), 10_000);
    }
}

#[test]
fn progress_preserves_rng_stream_and_statistics() {
    let data = [0, 1, 2].repeat(100);
    let options = PermutationOptions {
        seed: Some([1, 2, 3, 4]),
        ..Default::default()
    };
    let expected = permutation_tests_with_options(&data, options).unwrap();
    let mut events = Vec::new();
    let actual = permutation_tests_with_progress(&data, options, &mut |p| events.push(p)).unwrap();
    assert_eq!(actual.reject, expected.reject);
    assert_eq!(
        actual.permutations_performed,
        expected.permutations_performed
    );
    for (a, b) in actual.tests.iter().zip(&expected.tests) {
        assert_eq!(
            (a.greater, a.equal, a.less, a.reject),
            (b.greater, b.equal, b.less, b.reject)
        );
        assert_eq!(a.original, b.original);
    }
    assert_eq!(events.first().unwrap().permutations_completed, 0);
    assert_eq!(
        events.last().unwrap().permutations_completed,
        actual.permutations_performed
    );
    for pair in events.windows(2) {
        assert_eq!(
            pair[1].permutations_completed,
            pair[0].permutations_completed + 1
        );
        assert!(pair[1].elapsed >= pair[0].elapsed);
        assert!(
            pair[1]
                .active_statistics
                .iter()
                .all(|s| pair[0].active_statistics.contains(s))
        );
    }
}
