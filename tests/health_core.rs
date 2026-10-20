use sp_800_90b::{
    HealthTestConfig, HealthTestError, HealthTestFailure, HealthTestKind, HealthTestMonitor,
};

const BINARY: HealthTestConfig = match HealthTestConfig::new(2, 21, 589) {
    Ok(config) => config,
    Err(_) => panic!("invalid test configuration"),
};
const INITIAL: HealthTestMonitor = HealthTestMonitor::new(BINARY);

#[test]
fn validates_integer_parameters_and_derives_window_size() {
    assert_eq!(BINARY.alphabet_size(), 2);
    assert_eq!(BINARY.repetition_count(), 21);
    assert_eq!(BINARY.adaptive_proportion(), 589);
    assert_eq!(BINARY.window_size(), 1024);
    for alphabet in [3, 4, 16, 256] {
        assert_eq!(
            HealthTestConfig::new(alphabet, 21, 311)
                .unwrap()
                .window_size(),
            512
        );
    }
    for alphabet in [0, 1, 257, u16::MAX] {
        assert_eq!(
            HealthTestConfig::new(alphabet, 21, 589),
            Err(HealthTestError::InvalidAlphabet)
        );
    }
    for (alphabet, rct, apt) in [
        (2, 0, 589),
        (2, 1, 589),
        (2, 21, 0),
        (2, 21, 1),
        (2, 21, 1026),
        (4, 21, 514),
        (256, 21, u32::MAX),
    ] {
        assert_eq!(
            HealthTestConfig::new(alphabet, rct, apt),
            Err(HealthTestError::CutoffOutOfRange)
        );
    }
}

#[test]
fn repetition_failure_is_exact_and_latches_until_reset() {
    let mut monitor = INITIAL;
    monitor.observe_slice(&[7; 20]).unwrap();
    assert!(!monitor.status().failed());
    monitor.observe(7).unwrap();
    let failure = HealthTestFailure {
        test: HealthTestKind::RepetitionCount,
        sample_index: 20,
        count: 21,
        cutoff: 21,
    };
    assert_eq!(
        monitor.status().first_repetition_failure,
        Some(failure.clone())
    );
    for _ in 0..1024 {
        monitor.observe_slice(&[7, 9]).unwrap();
    }
    assert_eq!(monitor.status().first_repetition_failure, Some(failure));
    assert!(!monitor.status().startup_ready());
    monitor.reset();
    assert_eq!(monitor.config(), &BINARY);
    assert_eq!(monitor.status(), INITIAL.status());
    // Reset also clears the previously observed alphabet.
    monitor.observe_slice(&[0, 1]).unwrap();
}

#[test]
fn apt_counts_the_reference_and_restarts_at_window_boundaries() {
    let mut monitor = INITIAL;
    monitor.observe_slice(&[0; 588]).unwrap();
    assert!(monitor.status().first_adaptive_failure.is_none());
    monitor.observe(0).unwrap();
    assert_eq!(
        monitor.status().first_adaptive_failure,
        Some(HealthTestFailure {
            test: HealthTestKind::AdaptiveProportion,
            sample_index: 588,
            count: 589,
            cutoff: 589,
        })
    );
    monitor.reset();
    for _ in 0..512 {
        monitor.observe_slice(&[0, 1]).unwrap();
    }
    assert_eq!(monitor.status().completed_windows, 1);
    assert_eq!(monitor.status().samples_in_window, 0);
    monitor.observe_slice(&[1; 588]).unwrap();
    assert!(monitor.status().first_adaptive_failure.is_none());
    monitor.observe(1).unwrap();
    assert_eq!(
        monitor
            .status()
            .first_adaptive_failure
            .as_ref()
            .unwrap()
            .sample_index,
        1024 + 588
    );
}

#[test]
fn repetition_spans_apt_windows_and_startup_requires_1024_samples() {
    let mut monitor = INITIAL;
    for _ in 0..511 {
        monitor.observe_slice(&[0, 1]).unwrap();
    }
    monitor.observe(0).unwrap();
    assert!(!monitor.status().startup_ready());
    monitor.observe(1).unwrap();
    assert!(monitor.status().startup_ready());
    monitor.observe_slice(&[1; 19]).unwrap();
    assert!(!monitor.status().failed());
    monitor.observe(1).unwrap();
    assert_eq!(
        monitor
            .status()
            .first_repetition_failure
            .as_ref()
            .unwrap()
            .sample_index,
        1043
    );
    assert!(!monitor.status().startup_ready());

    let mut nonbinary = HealthTestMonitor::new(HealthTestConfig::new(4, 11, 177).unwrap());
    for i in 0..1023 {
        nonbinary.observe((i % 4) as u8).unwrap();
    }
    assert!(!nonbinary.status().startup_ready());
    nonbinary.observe(3).unwrap();
    assert!(nonbinary.status().startup_ready());
    assert_eq!(nonbinary.status().completed_windows, 2);
}

#[test]
fn chunks_preserve_state_and_partial_windows() {
    let samples: [u8; 2400] = core::array::from_fn(|i| (i % 4) as u8);
    let config = HealthTestConfig::new(4, 11, 177).unwrap();
    let mut whole = HealthTestMonitor::new(config);
    whole.observe_slice(&samples).unwrap();
    for chunk_size in [1, 137, 512, 1024] {
        let mut chunked = HealthTestMonitor::new(config);
        for chunk in samples.chunks(chunk_size) {
            chunked.observe_slice(chunk).unwrap();
        }
        assert_eq!(whole.status(), chunked.status());
    }
    assert!(whole.status().startup_ready());
    assert_eq!(whole.status().completed_windows, 4);
    assert_eq!(whole.status().samples_in_window, 352);
}

#[test]
fn invalid_symbol_leaves_sample_unconsumed_and_slice_preserves_prefix() {
    let mut monitor = INITIAL;
    assert_eq!(
        monitor.observe_slice(&[4, 7, 8, 4]),
        Err(HealthTestError::AlphabetExceeded)
    );
    assert_eq!(monitor.status().samples_seen, 2);
    let before = monitor.status().clone();
    assert_eq!(monitor.observe(8), Err(HealthTestError::AlphabetExceeded));
    assert_eq!(monitor.status(), &before);
    monitor.observe(7).unwrap();
    let mut expected = INITIAL;
    expected.observe_slice(&[4, 7, 7]).unwrap();
    assert_eq!(monitor.status(), expected.status());
}

#[test]
fn apt_detects_bias_without_repetition_failure() {
    let mut monitor = INITIAL;
    for _ in 0..256 {
        monitor.observe_slice(&[0, 0, 0, 1]).unwrap();
    }
    assert!(monitor.status().first_repetition_failure.is_none());
    assert_eq!(
        monitor.status().first_adaptive_failure,
        Some(HealthTestFailure {
            test: HealthTestKind::AdaptiveProportion,
            sample_index: 784,
            count: 589,
            cutoff: 589,
        })
    );
}

#[test]
fn apt_cutoff_above_window_is_supported() {
    for (alphabet, window) in [(2, 1024), (256, 512)] {
        let config = HealthTestConfig::new(alphabet, u32::MAX, window + 1).unwrap();
        let mut monitor = HealthTestMonitor::new(config);
        for _ in 0..2 * window {
            monitor.observe(0).unwrap();
        }
        assert!(!monitor.status().failed());
        assert_eq!(monitor.status().completed_windows, 2);
        assert_eq!(monitor.status().samples_in_window, 0);
    }
}
