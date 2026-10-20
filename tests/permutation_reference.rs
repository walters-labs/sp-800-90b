#![cfg(feature = "assessment")]

use sp_800_90b::*;
use std::{fs, path::PathBuf, process::Command};
#[test]
#[ignore = "requires NIST_REFERENCE_DIR and NIST_PERMUTATION_REFERENCE"]
fn original_statistics_match_all_nist_datasets() {
    let root = PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").unwrap());
    let binary = std::env::var_os("NIST_PERMUTATION_REFERENCE").unwrap();
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|x| x != "bin") {
            continue;
        }
        let out = Command::new(&binary).arg(&path).output().unwrap();
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        let actual = permutation_statistics(
            &fs::read(&path).unwrap(),
            PermutationArithmetic::NistReference,
        )
        .unwrap();
        for (i, line) in text.lines().take(19).enumerate() {
            let expected: f64 = line.split_whitespace().next().unwrap().parse().unwrap();
            if i == 0 {
                // C++ compilers may contract the excursion subtraction into an FMA.
                assert!((actual[i].to_f64() - expected).abs() < 1e-9 * expected.abs().max(1.0));
            } else {
                assert_eq!(
                    actual[i].to_f64(),
                    expected,
                    "{} {}",
                    path.display(),
                    PERMUTATION_NAMES[i]
                );
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 11);
}
#[test]
#[ignore = "requires NIST_REFERENCE_DIR and NIST_PERMUTATION_REFERENCE"]
fn seeded_counts_match_reference() {
    let root = PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").unwrap());
    let binary = std::env::var_os("NIST_PERMUTATION_REFERENCE").unwrap();
    for name in ["truerand_1bit.bin", "truerand_8bit.bin", "ringOsc-nist.bin"] {
        let data = fs::read(root.join("bin").join(name)).unwrap()[..4096].to_vec();
        let path =
            std::env::temp_dir().join(format!("nist-permutation-{}-{name}", std::process::id()));
        fs::write(&path, &data).unwrap();
        let out = Command::new(&binary)
            .arg(&path)
            .arg("run")
            .output()
            .unwrap();
        fs::remove_file(path).unwrap();
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        let actual = permutation_tests_with_options(
            &data,
            PermutationOptions {
                seed: Some([1, 2, 3, 4]),
                ..Default::default()
            },
        )
        .unwrap();
        for (i, line) in text.lines().take(19).enumerate() {
            let values: Vec<_> = line.split_whitespace().collect();
            let counts: Vec<usize> = values[1..].iter().map(|x| x.parse().unwrap()).collect();
            let r = &actual.tests[i];
            assert_eq!(
                [r.greater, r.equal, r.less],
                counts[..],
                "{name} {}",
                r.name
            );
        }
        assert_eq!(
            actual.permutations_performed,
            text.lines().nth(19).unwrap().parse::<usize>().unwrap()
        );
    }
}
