use rug::Float;
use sp_800_90b::*;
fn options() -> IidOptions {
    IidOptions {
        permutation: PermutationOptions {
            seed: Some([1, 2, 3, 4]),
            ..Default::default()
        },
        ..Default::default()
    }
}
fn random_data() -> Vec<u8> {
    let mut s = 123456789u64;
    (0..300)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s % 7) as u8
        })
        .collect()
}
#[test]
fn iid_lrs_matches_brute_force() {
    for data in [
        vec![0, 1, 2, 0, 1, 2, 0],
        vec![0, 0, 0, 0],
        vec![0, 1, 2, 3],
        random_data(),
    ] {
        let mut longest = 0;
        for i in 0..data.len() {
            for j in i + 1..data.len() {
                let len = data[i..]
                    .iter()
                    .zip(&data[j..])
                    .take_while(|(a, b)| a == b)
                    .count();
                longest = longest.max(len);
            }
        }
        let r = iid_lrs_test(&data, 256).unwrap();
        assert_eq!(r.longest_repeat, longest);
        assert!(r.p_value >= 0 && r.p_value <= 1);
    }
    let r = iid_lrs_test(&[0, 1, 0, 1], 256).unwrap();
    assert_eq!(r.longest_repeat, 2);
    assert_eq!(r.substring_pairs, 3);
    assert_eq!(r.collision_probability, 0.5);
    assert!((r.p_value.to_f64() - 0.578125).abs() < 1e-15);
}
#[test]
fn high_precision_lrs_handles_extreme_repetition() {
    let r = iid_lrs_test(&[0, 1].repeat(2056), 256).unwrap();
    assert_eq!(r.longest_repeat, 4110);
    assert!(r.reject);
    assert!(r.p_value > 0);
    assert_eq!(r.p_value.to_f64(), 0.0);
    assert!(matches!(
        iid_lrs_test(&[], 256),
        Err(EstimateError::TooFewSamples { .. })
    ));
    assert!(matches!(
        iid_lrs_test(&[0, 1], 32),
        Err(EstimateError::InvalidPrecision { .. })
    ));
}
#[test]
fn initial_report_combines_entropy_and_checks() {
    let data = random_data();
    let r = assess_iid(&data, options()).unwrap();
    assert_eq!(r.decision, IidDecision::NotRejected);
    assert!(r.is_complete());
    assert_eq!(r.bits_per_symbol, 3);
    let literal = most_common_value(&data).unwrap();
    let bits: Vec<_> = data
        .iter()
        .flat_map(|&s| (0..3).rev().map(move |shift| (s >> shift) & 1))
        .collect();
    let bit = most_common_value(&bits).unwrap();
    assert_eq!(r.literal.as_ref().unwrap().min_entropy, literal.min_entropy);
    assert_eq!(r.bitstring.as_ref().unwrap().min_entropy, bit.min_entropy);
    let expected = literal
        .min_entropy
        .min(&Float::with_val(256, bit.min_entropy * 3));
    assert_eq!(r.min_entropy.as_ref().unwrap(), &expected);
    assert_eq!(r.candidate_min_entropy, expected);
    assert_eq!(r.checks.unwrap().permutation.unwrap().seed, [1, 2, 3, 4]);
}
#[test]
fn conditioned_mode_and_truncation_do_not_change_checks_input() {
    let data = random_data();
    let r = assess_iid(
        &data,
        IidOptions {
            mode: IidMode::Conditioned,
            bits_per_symbol: Some(4),
            bitstring_limit: Some(101),
            ..options()
        },
    )
    .unwrap();
    assert!(r.literal.is_none());
    assert_eq!(r.bitstring.as_ref().unwrap().sample_count, 101);
    assert_eq!(
        r.candidate_min_entropy,
        Float::with_val(256, &r.bitstring.as_ref().unwrap().min_entropy * 4)
    );
    assert_eq!(
        r.checks
            .as_ref()
            .unwrap()
            .independence
            .as_ref()
            .unwrap()
            .sample_count,
        data.len()
    );
    assert_eq!(
        r.checks
            .as_ref()
            .unwrap()
            .permutation
            .as_ref()
            .unwrap()
            .sample_count,
        data.len()
    );
    assert!(r.warnings.iter().any(|w| matches!(
        w,
        AssessmentWarning::BitstringTruncated {
            available: 1200,
            used: 101
        }
    )));
}
#[test]
fn rejects_without_exposing_iid_entropy() {
    let r = assess_iid(&[0, 1, 2].repeat(100), options()).unwrap();
    assert_eq!(r.decision, IidDecision::Rejected);
    assert!(r.min_entropy.is_none());
    assert!(r.candidate_min_entropy > 0);
    assert!(r.checks.unwrap().longest_repeated_substring.unwrap().reject);
}
#[test]
fn incomplete_and_constant_are_not_passes() {
    let r = assess_iid(
        &random_data(),
        IidOptions {
            permutation: PermutationOptions {
                seed: Some([0; 4]),
                ..Default::default()
            },
            ..options()
        },
    )
    .unwrap();
    assert_eq!(r.decision, IidDecision::Incomplete);
    assert!(!r.is_complete());
    assert!(r.min_entropy.is_none());
    assert!(matches!(
        r.checks.unwrap().permutation,
        Err(PermutationError::ZeroSeed)
    ));
    let r = assess_iid(&[7; 300], options()).unwrap();
    assert_eq!(r.decision, IidDecision::Rejected);
    assert_eq!(r.candidate_min_entropy, 0);
    assert!(r.checks.is_none());
    assert!(r.min_entropy.is_none());
    assert!(matches!(
        assess_iid(
            &[0, 8],
            IidOptions {
                bits_per_symbol: Some(3),
                ..options()
            }
        ),
        Err(AssessmentError::SymbolOutOfRange { .. })
    ));
}

#[test]
fn sparse_binary_initial_uses_only_literal_entropy() {
    let data: Vec<u8> = random_data()
        .into_iter()
        .map(|x| if x % 2 == 0 { 2 } else { 5 })
        .collect();
    let r = assess_iid(&data, options()).unwrap();
    assert_eq!(r.alphabet, [2, 5]);
    assert_eq!(r.bits_per_symbol, 3);
    assert!(r.bitstring.is_none());
    assert!(r.literal.is_some());
    assert_eq!(
        r.candidate_min_entropy,
        most_common_value(&data).unwrap().min_entropy
    );
    assert!(!r.is_complete());
    assert!(r.min_entropy.is_none());
    assert!(matches!(
        r.checks.unwrap().permutation,
        Err(PermutationError::TooFewSamples { .. })
    ));
    let r = assess_iid(&[0, 1].repeat(150), options()).unwrap();
    assert_eq!(r.decision, IidDecision::Rejected);
    assert!(!r.is_complete());
    assert!(r.checks.unwrap().longest_repeated_substring.unwrap().reject);
}
