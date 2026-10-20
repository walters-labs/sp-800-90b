use rug::Float;
use sp_800_90b::*;
use std::{fs, path::PathBuf, process::Command};
#[test]
#[ignore = "requires NIST_REFERENCE_DIR; full million-sample non-IID restart assessment"]
fn full_non_iid_restart_matches_reference() {
    let root = PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").unwrap());
    let path = root.join("bin/truerand_1bit.bin");
    let data = fs::read(&path).unwrap();
    let out = Command::new(root.join("cpp/ea_restart"))
        .env("OMP_NUM_THREADS", "1")
        .args(["-n", "-v"])
        .arg(path)
        .args(["1", "0"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    let r = assess_restart(
        &data,
        &Float::with_val(256, 0),
        RestartOptions {
            simulation_seed: Some([1, 2, 3, 4]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.decision, RestartDecision::Validated);
    assert_eq!(r.min_entropy.unwrap(), 0);
    for (label, dataset) in [("H_r: ", r.rows.unwrap()), ("H_c: ", r.columns.unwrap())] {
        let value: f64 = text
            .lines()
            .find_map(|line| line.strip_prefix(label))
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            (dataset.min_entropy().to_f64() - value).abs() < 1e-6,
            "{label}"
        );
        assert!(dataset.is_complete());
    }
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR; full million-sample IID checks"]
fn iid_restart_retains_incomplete_checks_and_literal_entropy() {
    let root = PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").unwrap());
    let data = fs::read(root.join("bin/truerand_4bit.bin")).unwrap();
    let r = assess_restart(
        &data,
        &Float::with_val(256, 0),
        RestartOptions {
            mode: RestartMode::Iid,
            simulation_seed: Some([1, 2, 3, 4]),
            permutation: PermutationOptions {
                seed: Some([0; 4]),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.decision, RestartDecision::Incomplete);
    assert!(r.min_entropy.is_none());
    let expected = most_common_value(&data).unwrap().min_entropy;
    for dataset in [r.rows.unwrap(), r.columns.unwrap()] {
        assert_eq!(dataset.min_entropy(), &expected);
        let RestartDatasetAssessment::Iid { checks, .. } = dataset else {
            panic!("wrong mode")
        };
        assert!(matches!(
            checks.permutation,
            Err(PermutationError::ZeroSeed)
        ));
        assert!(
            checks.independence.is_ok()
                && checks.goodness_of_fit.is_ok()
                && checks.longest_repeated_substring.is_ok()
        );
    }
}

#[test]
#[ignore = "requires NIST_RESTART_REFERENCE; five-million-round simulation parity"]
fn positive_entropy_sanity_cutoff_matches_reference() {
    let binary = std::env::var_os("NIST_RESTART_REFERENCE").unwrap();
    let output = Command::new(binary)
        .args(["1", &RESTART_SIMULATIONS.to_string()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: usize = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let data = [0, 1].repeat(RESTART_SAMPLES / 2);
    let result = restart_sanity_check(
        &data,
        &Float::with_val(256, 1),
        RestartOptions {
            simulation_seed: Some([1, 2, 3, 4]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.cutoff, expected);
    assert!(result.reject);
    assert_eq!(result.maximum_row_count, 500);
    assert_eq!(result.maximum_column_count, 1000);
}
