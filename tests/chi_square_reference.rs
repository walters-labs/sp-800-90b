#![cfg(feature = "assessment")]

use sp_800_90b::chi_square_independence;
use std::{fs, path::PathBuf, process::Command};
#[test]
#[ignore = "requires NIST_REFERENCE_DIR and NIST_CHI_SQUARE_REFERENCE adapter binary"]
fn independence_matches_nist_on_all_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let binary =
        std::env::var_os("NIST_CHI_SQUARE_REFERENCE").expect("set NIST_CHI_SQUARE_REFERENCE");
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|x| x != "bin") {
            continue;
        }
        let output = Command::new(&binary).arg(&path).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let values: Vec<f64> = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .last()
            .unwrap()
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        let r = chi_square_independence(&fs::read(&path).unwrap()).unwrap();
        eprintln!(
            "{}: df {} / {}, T {} / {}, p {} / {}",
            path.display(),
            r.degrees_of_freedom,
            values[1],
            r.statistic.to_f64(),
            values[0],
            r.p_value.to_f64(),
            values[2]
        );
        assert_eq!(r.degrees_of_freedom, values[1] as usize);
        assert!(
            (r.statistic.to_f64() - values[0]).abs() < 1e-9 * values[0].max(1.0),
            "{} statistic",
            path.display()
        );
        assert!(
            (r.p_value.to_f64() - values[2]).abs() < 1e-9,
            "{} p-value",
            path.display()
        );
        assert_eq!(r.reject_independence, values[2] < 0.001);
        checked += 1;
    }
    assert_eq!(checked, 11);
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR and NIST_CHI_SQUARE_REFERENCE adapter binary"]
fn goodness_of_fit_matches_nist_on_all_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let binary =
        std::env::var_os("NIST_CHI_SQUARE_REFERENCE").expect("set NIST_CHI_SQUARE_REFERENCE");
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|x| x != "bin") {
            continue;
        }
        let output = Command::new(&binary)
            .arg(&path)
            .arg("goodness-of-fit")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let values: Vec<f64> = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .last()
            .unwrap()
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        let r = sp_800_90b::chi_square_goodness_of_fit(&fs::read(&path).unwrap()).unwrap();
        eprintln!(
            "{}: df {} / {}, T {} / {}, p {} / {}",
            path.display(),
            r.degrees_of_freedom,
            values[1],
            r.statistic.to_f64(),
            values[0],
            r.p_value.to_f64(),
            values[2]
        );
        assert_eq!(r.degrees_of_freedom, values[1] as usize);
        assert!(
            (r.statistic.to_f64() - values[0]).abs() < 1e-9 * values[0].max(1.0),
            "{} statistic",
            path.display()
        );
        assert!(
            (r.p_value.to_f64() - values[2]).abs() < 1e-9,
            "{} p-value",
            path.display()
        );
        assert_eq!(r.reject_goodness_of_fit, values[2] < 0.001);
        checked += 1;
    }
    assert_eq!(checked, 11);
}
