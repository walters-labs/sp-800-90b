#![cfg(feature = "assessment")]

use rug::Float;
use sp_800_90b::*;
fn f(x: &str) -> Float {
    Float::with_val(256, Float::parse(x).unwrap())
}
#[test]
fn publication_table_two_and_rct_example() {
    for (alphabet, h, c) in [
        (2, "0.2", 941),
        (2, "0.4", 840),
        (2, "0.6", 748),
        (2, "0.8", 664),
        (2, "1", 589),
        (4, "0.5", 410),
        (4, "1", 311),
        (4, "2", 177),
        (16, "4", 62),
        (256, "8", 13),
    ] {
        let r = health_test_cutoffs(
            &f(h),
            HealthTestOptions {
                alphabet_size: alphabet,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.adaptive_proportion, c, "H={h}, k={alphabet}");
        assert!(r.adaptive_tail_probability <= r.alpha);
        let config = r.monitor_config().unwrap();
        assert_eq!(config.alphabet_size(), alphabet);
        assert_eq!(config.repetition_count(), r.repetition_count);
        assert_eq!(config.adaptive_proportion() as usize, c);
        assert_eq!(config.window_size(), r.window_size);
    }
    assert_eq!(
        health_test_cutoffs(
            &f("2"),
            HealthTestOptions {
                alphabet_size: 4,
                ..Default::default()
            }
        )
        .unwrap()
        .repetition_count,
        11
    );
}

#[test]
fn exporting_monitor_config_validates_public_report_fields() {
    let cutoffs = health_test_cutoffs(&f("1"), HealthTestOptions::default()).unwrap();
    let mut invalid = cutoffs.clone();
    invalid.window_size = 512;
    assert_eq!(
        invalid.monitor_config(),
        Err(HealthTestError::InvalidWindowSize)
    );
    invalid = cutoffs.clone();
    invalid.options.alphabet_size = 257;
    assert_eq!(
        invalid.monitor_config(),
        Err(HealthTestError::InvalidAlphabet)
    );
    invalid = cutoffs.clone();
    invalid.repetition_count = 1;
    assert_eq!(
        invalid.monitor_config(),
        Err(HealthTestError::CutoffOutOfRange)
    );
    invalid = cutoffs;
    invalid.adaptive_proportion = usize::MAX;
    assert_eq!(
        invalid.monitor_config(),
        Err(HealthTestError::CutoffOutOfRange)
    );
}

#[test]
fn tiny_entropy_exports_apt_cutoff_above_window() {
    let cutoffs = health_test_cutoffs(&f("0.01"), HealthTestOptions::default()).unwrap();
    assert_eq!(cutoffs.adaptive_proportion, 1025);
    let mut monitor = HealthTestMonitor::new(cutoffs.monitor_config().unwrap());
    monitor.observe_slice(&[0; 1024]).unwrap();
    assert!(!monitor.status().failed());
}
#[test]
fn repetition_boundary_latch_and_reset() {
    let mut test = HealthTests::new(&f("1"), HealthTestOptions::default()).unwrap();
    assert_eq!(test.cutoffs().repetition_count, 21);
    test.observe_slice(&[7; 20]).unwrap();
    assert!(!test.status().failed());
    test.observe(7).unwrap();
    assert_eq!(
        test.status()
            .first_repetition_failure
            .as_ref()
            .unwrap()
            .sample_index,
        20
    );
    test.observe_slice(&[9, 7].repeat(600)).unwrap();
    assert!(test.status().failed());
    assert!(!test.status().startup_ready());
    test.reset();
    test.observe_slice(&[7, 9].repeat(512)).unwrap();
    assert!(test.status().startup_ready());
}
#[test]
fn apt_includes_first_sample_and_keeps_nonoverlapping_windows() {
    let mut test = HealthTests::new(&f("1"), HealthTestOptions::default()).unwrap();
    let c = test.cutoffs().adaptive_proportion;
    test.observe_slice(&vec![0; c - 1]).unwrap();
    assert!(test.status().first_adaptive_failure.is_none());
    test.observe(0).unwrap();
    assert_eq!(
        test.status()
            .first_adaptive_failure
            .as_ref()
            .unwrap()
            .sample_index,
        (c - 1) as u64
    );
    test.reset();
    test.observe_slice(&[0, 1].repeat(512)).unwrap();
    assert_eq!(test.status().completed_windows, 1);
    assert_eq!(test.status().samples_in_window, 0);
    test.observe_slice(&vec![1; c - 1]).unwrap();
    assert!(test.status().first_adaptive_failure.is_none());
    test.observe(1).unwrap();
    assert_eq!(
        test.status()
            .first_adaptive_failure
            .as_ref()
            .unwrap()
            .sample_index,
        (1024 + c - 1) as u64
    );
}
#[test]
fn chunks_do_not_change_results_or_startup_count() {
    let samples = [0, 1, 2, 3].repeat(600);
    let opts = HealthTestOptions {
        alphabet_size: 4,
        ..Default::default()
    };
    let mut whole = HealthTests::new(&f("2"), opts).unwrap();
    let mut chunked = whole.clone();
    whole.observe_slice(&samples).unwrap();
    for chunk in samples.chunks(137) {
        chunked.observe_slice(chunk).unwrap();
    }
    assert_eq!(whole.status(), chunked.status());
    assert!(whole.status().startup_ready());
    assert_eq!(whole.status().completed_windows, 4);
    assert_eq!(whole.status().samples_in_window, 352);
}
#[test]
fn invalid_configuration_and_alphabet() {
    for h in ["0", "-1", "2", "NaN", "inf"] {
        assert!(HealthTests::new(&f(h), HealthTestOptions::default()).is_err());
    }
    assert!(
        HealthTests::new(
            &f("1"),
            HealthTestOptions {
                alphabet_size: 1,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        HealthTests::new(
            &f("1"),
            HealthTestOptions {
                false_positive_exponent: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
    let mut t = HealthTests::new(&f("1"), HealthTestOptions::default()).unwrap();
    t.observe_slice(&[4, 7]).unwrap();
    let before = t.status().clone();
    assert!(matches!(
        t.observe(8),
        Err(HealthTestError::AlphabetExceeded)
    ));
    assert_eq!(t.status(), &before);
}

#[test]
fn apt_detects_bias_without_a_long_identical_run() {
    let mut t = HealthTests::new(&f("1"), HealthTestOptions::default()).unwrap();
    t.observe_slice(&[0, 0, 0, 1].repeat(256)).unwrap();
    assert!(t.status().first_repetition_failure.is_none());
    assert_eq!(
        t.status()
            .first_adaptive_failure
            .as_ref()
            .unwrap()
            .sample_index,
        784
    );
}
#[test]
fn rct_crosses_apt_boundaries_and_startup_requires_1024() {
    let mut t = HealthTests::new(&f("1"), HealthTestOptions::default()).unwrap();
    t.observe_slice(&[0, 1].repeat(511)).unwrap();
    t.observe(0).unwrap();
    assert!(!t.status().startup_ready());
    t.observe(1).unwrap();
    assert!(t.status().startup_ready());
    t.observe_slice(&[1; 20]).unwrap();
    assert_eq!(
        t.status()
            .first_repetition_failure
            .as_ref()
            .unwrap()
            .sample_index,
        1043
    );
}
