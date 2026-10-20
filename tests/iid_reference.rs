#![cfg(feature = "assessment")]

use sp_800_90b::*;
use std::{fs, path::PathBuf, process::Command};
fn number(text: &str, label: &str) -> f64 {
    text.lines()
        .find_map(|line| line.strip_prefix(label))
        .unwrap_or_else(|| panic!("missing {label}: {text}"))
        .trim()
        .parse()
        .unwrap()
}
#[test]
#[ignore = "requires NIST_REFERENCE_DIR and NIST_IID_LRS_REFERENCE"]
fn iid_lrs_matches_reference_datasets() {
    let root = PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").unwrap());
    let adapter = std::env::var_os("NIST_IID_LRS_REFERENCE").unwrap();
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|x| x != "bin") {
            continue;
        }
        // Full ring oscillator data underflows and asserts in NIST on binary64 long double.
        if path.file_name().unwrap() == "ringOsc-nist.bin" {
            continue;
        }
        let out = Command::new(&adapter).arg(&path).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8(out.stdout).unwrap();
        let r = iid_lrs_test(&fs::read(path).unwrap(), 256).unwrap();
        assert_eq!(
            r.longest_repeat,
            number(&text, "Literal Longest Repeated Substring results: W = ") as usize
        );
        assert!(
            (r.collision_probability.to_f64()
                - number(
                    &text,
                    "Literal Longest Repeated Substring results: P_col = "
                ))
            .abs()
                < 1e-12
        );
        assert!(
            (r.p_value.to_f64()
                - number(
                    &text,
                    "Literal Longest Repeated Substring results: Pr(X >= 1) = "
                ))
            .abs()
                < 1e-9
        );
        assert_eq!(!r.reject, number(&text, "pass = ") == 1.0);
        checked += 1;
    }
    assert_eq!(checked, 10);
}
#[test]
#[ignore = "requires NIST_REFERENCE_DIR and built cpp/ea_iid"]
fn combined_report_matches_reference_initial_and_conditioned() {
    let root = PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").unwrap());
    let data = fs::read(root.join("bin/truerand_4bit.bin")).unwrap()[..4096].to_vec();
    let path = std::env::temp_dir().join(format!("iid-api-{}.bin", std::process::id()));
    fs::write(&path, &data).unwrap();
    for mode in [IidMode::Initial, IidMode::Conditioned] {
        let out = Command::new(root.join("cpp/ea_iid"))
            .env("OMP_NUM_THREADS", "1")
            .args([
                "-v",
                "-v",
                if mode == IidMode::Initial { "-i" } else { "-c" },
            ])
            .arg(&path)
            .arg("4")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8(out.stdout).unwrap();
        let r = assess_iid(
            &data,
            IidOptions {
                mode,
                bits_per_symbol: Some(4),
                permutation: PermutationOptions {
                    seed: Some([1, 2, 3, 4]),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            (r.candidate_min_entropy.to_f64() - number(&text, "Assessed min entropy: ")).abs()
                < 1e-12
        );
        let checks = r.checks.unwrap();
        assert_eq!(
            checks.independence.unwrap().reject_independence,
            number(&text, "Chi square independence: P-value = ") < 0.001
        );
        assert_eq!(
            checks.goodness_of_fit.unwrap().reject_goodness_of_fit,
            number(&text, "Chi square goodness of fit: P-value = ") < 0.001
        );
        assert_eq!(
            checks.longest_repeated_substring.unwrap().longest_repeat,
            number(&text, "Literal Longest Repeated Substring results: W = ") as usize
        );
        // Native RNG is OS-seeded; deterministic per-shuffle parity is tested separately.
        assert!(checks.permutation.is_ok());
    }
    fs::remove_file(path).unwrap();
}
