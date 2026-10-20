#![cfg(feature = "assessment")]

use rug::Float;
use sp_800_90b::*;
fn options() -> RestartOptions {
    RestartOptions {
        simulation_seed: Some([1, 2, 3, 4]),
        ..Default::default()
    }
}
#[test]
fn transpose_and_maxima() {
    let data: Vec<u8> = (0..RESTART_SAMPLES)
        .map(|i| ((i / 1000) % 2) as u8)
        .collect();
    let columns = restart_column_sequence(&data).unwrap();
    assert_eq!(&columns[..6], &[0, 1, 0, 1, 0, 1]);
    assert_eq!(restart_column_sequence(&columns).unwrap(), data);
    let r = restart_sanity_check(&data, &Float::with_val(256, 0), options()).unwrap();
    assert_eq!(r.maximum_row_count, 1000);
    assert_eq!(r.maximum_column_count, 500);
    assert_eq!(r.row_maxima, vec![1000; 1000]);
    assert_eq!(r.column_maxima, vec![500; 1000]);
    assert_eq!(r.cutoff, 1000);
    assert!(!r.reject); // Equality at cutoff passes.
}
#[test]
fn invalid_input_and_entropy() {
    let h = Float::with_val(256, 0);
    assert!(matches!(
        assess_restart(&[], &h, options()),
        Err(RestartError::InvalidSampleCount { count: 0 })
    ));
    assert!(matches!(
        restart_column_sequence(&[0; 10]),
        Err(RestartError::InvalidSampleCount { .. })
    ));
    assert!(matches!(
        assess_restart(&vec![0; RESTART_SAMPLES], &h, options()),
        Err(RestartError::ConstantInput)
    ));
    let data = [2, 5].repeat(RESTART_SAMPLES / 2);
    for h in [
        Float::with_val(256, -1),
        Float::with_val(256, f64::NAN),
        Float::with_val(256, f64::INFINITY),
        Float::with_val(256, 4),
    ] {
        assert!(matches!(
            assess_restart(&data, &h, options()),
            Err(RestartError::InvalidInitialEntropy)
        ));
    }
    assert!(matches!(
        assess_restart(&data, &Float::with_val(256, 2), options()),
        Err(RestartError::EntropyExceedsAlphabet)
    ));
    assert!(matches!(
        assess_restart(
            &data,
            &h,
            RestartOptions {
                bits_per_symbol: Some(2),
                ..options()
            }
        ),
        Err(RestartError::Input(
            AssessmentError::SymbolOutOfRange { .. }
        ))
    ));
    assert!(matches!(
        assess_restart(
            &data,
            &h,
            RestartOptions {
                simulation_rounds: 10,
                ..options()
            }
        ),
        Err(RestartError::TooFewSimulations { .. })
    ));
    assert!(matches!(
        assess_restart(
            &data,
            &h,
            RestartOptions {
                simulation_seed: Some([0; 4]),
                ..options()
            }
        ),
        Err(RestartError::ZeroSeed)
    ));
}
#[test]
#[ignore = "five-million-round Monte Carlo regression"]
fn sanity_rejection_stops_assessment() {
    let data: Vec<u8> = (0..RESTART_SAMPLES)
        .map(|i| ((i / 1000) % 2) as u8)
        .collect();
    let r = assess_restart(&data, &Float::with_val(256, 1), options()).unwrap();
    assert_eq!(r.decision, RestartDecision::SanityRejected);
    assert!(r.rows.is_none() && r.columns.is_none() && r.min_entropy.is_none());
    assert!(r.sanity.cutoff < 1000);
}
