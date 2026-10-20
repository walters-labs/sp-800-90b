#![cfg(feature = "assessment")]

//! Optional comparison against an external, already-tested NIST checkout.
use sp_800_90b::most_common_value;
use std::{fs, path::PathBuf};

#[test]
#[ignore = "requires NIST_REFERENCE_DIR and completed NIST self-tests"]
fn literal_mcv_matches_nist_on_all_bundled_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "bin") {
            continue;
        }
        let samples = fs::read(&path).unwrap();
        let reference = fs::read_to_string(
            root.join("cpp/selftest")
                .join(path.file_name().unwrap())
                .with_extension("res"),
        )
        .unwrap();
        let label = "Literal Most Common Value Estimate: min entropy = ";
        let expected: f64 = reference
            .lines()
            .find_map(|line| line.strip_prefix(label))
            .expect("missing literal MCV result")
            .parse()
            .unwrap();
        let actual = most_common_value(&samples).unwrap().min_entropy.to_f64();
        assert!(
            (actual - expected).abs() < 1e-12,
            "{}: actual={actual}, expected={expected}",
            path.display()
        );
        checked += 1;
    }
    assert_eq!(
        checked, 11,
        "expected the pinned reference's eleven datasets"
    );
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR and completed NIST self-tests"]
fn collision_matches_nist_on_all_bundled_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "bin") {
            continue;
        }
        let raw = fs::read(&path).unwrap();
        let width = 8 - raw.iter().max().unwrap().leading_zeros();
        let data: Vec<u8> = raw
            .iter()
            .flat_map(|&x| (0..width).rev().map(move |shift| (x >> shift) & 1))
            .collect();
        let reference = fs::read_to_string(
            root.join("cpp/selftest")
                .join(path.file_name().unwrap())
                .with_extension("res"),
        )
        .unwrap();
        let prefix = if width == 1 { "Literal" } else { "Bitstring" };
        let label = format!("{prefix} Collision Estimate: min entropy = ");
        let expected: f64 = reference
            .lines()
            .find_map(|line| line.strip_prefix(&label))
            .expect("missing collision result")
            .parse()
            .unwrap();
        let actual = sp_800_90b::collision_estimate(&data)
            .unwrap()
            .min_entropy
            .to_f64();
        assert!(
            (actual - expected).abs() < 1e-12,
            "{}: {actual} vs {expected}",
            path.display()
        );
        checked += 1;
    }
    assert_eq!(checked, 11);
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR and completed NIST self-tests"]
fn markov_matches_nist_on_all_bundled_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "bin") {
            continue;
        }
        let raw = fs::read(&path).unwrap();
        let width = 8 - raw.iter().max().unwrap().leading_zeros();
        let data: Vec<u8> = raw
            .iter()
            .flat_map(|&x| (0..width).rev().map(move |shift| (x >> shift) & 1))
            .collect();
        let reference = fs::read_to_string(
            root.join("cpp/selftest")
                .join(path.file_name().unwrap())
                .with_extension("res"),
        )
        .unwrap();
        let prefix = if width == 1 { "Literal" } else { "Bitstring" };
        let label = format!("{prefix} Markov Estimate: min entropy = ");
        let expected: f64 = reference
            .lines()
            .find_map(|line| line.strip_prefix(&label))
            .expect("missing Markov result")
            .parse()
            .unwrap();
        let actual = sp_800_90b::markov_estimate(&data)
            .unwrap()
            .min_entropy
            .to_f64();
        assert!(
            (actual - expected).abs() < 1e-12,
            "{}: {actual} vs {expected}",
            path.display()
        );
        checked += 1;
    }
    assert_eq!(checked, 11);
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR and completed NIST self-tests"]
fn compression_matches_nist_on_all_bundled_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "bin") {
            continue;
        }
        let raw = fs::read(&path).unwrap();
        let width = 8 - raw.iter().max().unwrap().leading_zeros();
        let data: Vec<u8> = raw
            .iter()
            .flat_map(|&x| (0..width).rev().map(move |shift| (x >> shift) & 1))
            .collect();
        let reference = fs::read_to_string(
            root.join("cpp/selftest")
                .join(path.file_name().unwrap())
                .with_extension("res"),
        )
        .unwrap();
        let prefix = if width == 1 { "Literal" } else { "Bitstring" };
        let label = format!("{prefix} Compression Estimate: min entropy = ");
        let expected: f64 = reference
            .lines()
            .find_map(|line| line.strip_prefix(&label))
            .expect("missing compression result")
            .parse()
            .unwrap();
        let actual = sp_800_90b::compression_estimate(&data)
            .unwrap()
            .min_entropy
            .to_f64();
        assert!(
            (actual - expected).abs() < 1e-12,
            "{}: {actual} vs {expected}",
            path.display()
        );
        checked += 1;
    }
    assert_eq!(checked, 11);
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR and completed NIST self-tests"]
fn t_tuple_matches_nist_on_all_bundled_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "bin") {
            continue;
        }
        let raw = fs::read(&path).unwrap();
        let reference = fs::read_to_string(
            root.join("cpp/selftest")
                .join(path.file_name().unwrap())
                .with_extension("res"),
        )
        .unwrap();
        let width = 8 - raw.iter().max().unwrap().leading_zeros();
        let bits: Vec<u8> = raw
            .iter()
            .flat_map(|&x| (0..width).rev().map(move |shift| (x >> shift) & 1))
            .collect();
        for (prefix, data) in [("Literal", raw.as_slice()), ("Bitstring", bits.as_slice())] {
            if prefix == "Bitstring" && width == 1 {
                continue;
            }
            let label = format!("{prefix} t-Tuple Estimate: min entropy = ");
            let expected: f64 = reference
                .lines()
                .find_map(|line| line.strip_prefix(&label))
                .expect("missing t-Tuple result")
                .parse()
                .unwrap();
            let r = sp_800_90b::t_tuple_estimate(data).unwrap();
            let actual = r.min_entropy.to_f64();
            assert!(
                (actual - expected).abs() < 1e-12,
                "{} {prefix}: {actual} vs {expected}",
                path.display()
            );
            let label = format!("{prefix} t-Tuple Estimate: t = ");
            let expected_t: usize = reference
                .lines()
                .find_map(|line| line.strip_prefix(&label))
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(r.max_tuple_length, expected_t);
        }
        checked += 1;
    }
    assert_eq!(checked, 11);
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR and completed NIST self-tests"]
fn lrs_matches_nist_on_all_bundled_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "bin") {
            continue;
        }
        let raw = fs::read(&path).unwrap();
        let reference = fs::read_to_string(
            root.join("cpp/selftest")
                .join(path.file_name().unwrap())
                .with_extension("res"),
        )
        .unwrap();
        let width = 8 - raw.iter().max().unwrap().leading_zeros();
        let bits: Vec<u8> = raw
            .iter()
            .flat_map(|&x| (0..width).rev().map(move |shift| (x >> shift) & 1))
            .collect();
        for (prefix, data) in [("Literal", raw.as_slice()), ("Bitstring", bits.as_slice())] {
            if prefix == "Bitstring" && width == 1 {
                continue;
            }
            let label = format!("{prefix} LRS Estimate: min entropy = ");
            let expected: f64 = reference
                .lines()
                .find_map(|line| line.strip_prefix(&label))
                .expect("missing LRS result")
                .parse()
                .unwrap();
            let r = sp_800_90b::lrs_estimate(data).unwrap();
            let actual = r.min_entropy.to_f64();
            assert!(
                (actual - expected).abs() < 1e-12,
                "{} {prefix}: {actual} vs {expected}",
                path.display()
            );
            for (field, actual) in [("u", r.min_length), ("v", r.max_length)] {
                let label = format!("{prefix} LRS Estimate: {field} = ");
                let expected: usize = reference
                    .lines()
                    .find_map(|line| line.strip_prefix(&label))
                    .unwrap()
                    .parse()
                    .unwrap();
                assert_eq!(actual, expected, "{} {prefix} {field}", path.display());
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 11);
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR and completed NIST self-tests"]
fn multi_mcw_matches_nist_on_all_bundled_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "bin") {
            continue;
        }
        let raw = fs::read(&path).unwrap();
        let reference = fs::read_to_string(
            root.join("cpp/selftest")
                .join(path.file_name().unwrap())
                .with_extension("res"),
        )
        .unwrap();
        let width = 8 - raw.iter().max().unwrap().leading_zeros();
        let bits: Vec<u8> = raw
            .iter()
            .flat_map(|&x| (0..width).rev().map(move |shift| (x >> shift) & 1))
            .collect();
        for (prefix, data) in [("Literal", raw.as_slice()), ("Bitstring", bits.as_slice())] {
            if prefix == "Bitstring" && width == 1 {
                continue;
            }
            let label = format!("{prefix} MultiMCW Prediction Estimate: min entropy = ");
            let expected: f64 = reference
                .lines()
                .find_map(|line| line.strip_prefix(&label))
                .expect("missing MultiMCW result")
                .parse()
                .unwrap();
            let r = sp_800_90b::multi_mcw_estimate(data).unwrap();
            let actual = r.min_entropy.to_f64();
            eprintln!(
                "{} {prefix}: entropy difference {:.3e}",
                path.display(),
                (actual - expected).abs()
            );
            assert!(
                (actual - expected).abs() < if r.p_local.is_some() { 1e-9 } else { 1e-12 },
                "{} {prefix}: {actual} vs {expected}",
                path.display()
            );
            for (field, actual) in [
                ("C", r.correct_count),
                ("r", r.longest_correct_run + 1),
                ("N", r.prediction_count),
            ] {
                let label = format!("{prefix} MultiMCW Prediction Estimate: {field} = ");
                let expected: usize = reference
                    .lines()
                    .find_map(|line| line.strip_prefix(&label))
                    .unwrap()
                    .parse()
                    .unwrap();
                assert_eq!(actual, expected, "{} {prefix} {field}", path.display());
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 11);
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR and completed NIST self-tests"]
fn lag_matches_nist_on_all_bundled_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "bin") {
            continue;
        }
        let raw = fs::read(&path).unwrap();
        let reference = fs::read_to_string(
            root.join("cpp/selftest")
                .join(path.file_name().unwrap())
                .with_extension("res"),
        )
        .unwrap();
        let width = 8 - raw.iter().max().unwrap().leading_zeros();
        let bits: Vec<u8> = raw
            .iter()
            .flat_map(|&x| (0..width).rev().map(move |shift| (x >> shift) & 1))
            .collect();
        for (prefix, data) in [("Literal", raw.as_slice()), ("Bitstring", bits.as_slice())] {
            if prefix == "Bitstring" && width == 1 {
                continue;
            }
            let label = format!("{prefix} Lag Prediction Estimate: min entropy = ");
            let expected: f64 = reference
                .lines()
                .find_map(|line| line.strip_prefix(&label))
                .expect("missing Lag result")
                .parse()
                .unwrap();
            let r = sp_800_90b::lag_estimate(data).unwrap();
            let actual = r.min_entropy.to_f64();
            eprintln!(
                "{} {prefix}: entropy difference {:.3e}",
                path.display(),
                (actual - expected).abs()
            );
            assert!(
                (actual - expected).abs() < if r.p_local.is_some() { 1e-9 } else { 1e-12 },
                "{} {prefix}: {actual} vs {expected}",
                path.display()
            );
            for (field, actual) in [
                ("C", r.correct_count),
                ("r", r.longest_correct_run + 1),
                ("N", r.prediction_count),
            ] {
                let label = format!("{prefix} Lag Prediction Estimate: {field} = ");
                let expected: usize = reference
                    .lines()
                    .find_map(|line| line.strip_prefix(&label))
                    .unwrap()
                    .parse()
                    .unwrap();
                assert_eq!(actual, expected, "{} {prefix} {field}", path.display());
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 11);
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR and completed NIST self-tests"]
fn multi_mmc_matches_nist_on_all_bundled_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "bin") {
            continue;
        }
        let raw = fs::read(&path).unwrap();
        let reference = fs::read_to_string(
            root.join("cpp/selftest")
                .join(path.file_name().unwrap())
                .with_extension("res"),
        )
        .unwrap();
        let width = 8 - raw.iter().max().unwrap().leading_zeros();
        let bits: Vec<u8> = raw
            .iter()
            .flat_map(|&x| (0..width).rev().map(move |shift| (x >> shift) & 1))
            .collect();
        for (prefix, data) in [("Literal", raw.as_slice()), ("Bitstring", bits.as_slice())] {
            if prefix == "Bitstring" && width == 1 {
                continue;
            }
            let label = format!("{prefix} MultiMMC Prediction Estimate: min entropy = ");
            let expected: f64 = reference
                .lines()
                .find_map(|line| line.strip_prefix(&label))
                .expect("missing MultiMMC result")
                .parse()
                .unwrap();
            let r = sp_800_90b::multi_mmc_estimate(data).unwrap();
            let actual = r.min_entropy.to_f64();
            eprintln!(
                "{} {prefix}: entropy difference {:.3e}",
                path.display(),
                (actual - expected).abs()
            );
            assert!(
                (actual - expected).abs() < if r.p_local.is_some() { 1e-9 } else { 1e-12 },
                "{} {prefix}: {actual} vs {expected}",
                path.display()
            );
            for (field, actual) in [
                ("C", r.correct_count),
                ("r", r.longest_correct_run + 1),
                ("N", r.prediction_count),
            ] {
                let label = format!("{prefix} MultiMMC Prediction Estimate: {field} = ");
                let expected: usize = reference
                    .lines()
                    .find_map(|line| line.strip_prefix(&label))
                    .unwrap()
                    .parse()
                    .unwrap();
                assert_eq!(actual, expected, "{} {prefix} {field}", path.display());
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 11);
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR and completed NIST self-tests"]
fn lz78y_matches_nist_on_all_bundled_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "bin") {
            continue;
        }
        let raw = fs::read(&path).unwrap();
        let reference = fs::read_to_string(
            root.join("cpp/selftest")
                .join(path.file_name().unwrap())
                .with_extension("res"),
        )
        .unwrap();
        let width = 8 - raw.iter().max().unwrap().leading_zeros();
        let bits: Vec<u8> = raw
            .iter()
            .flat_map(|&x| (0..width).rev().map(move |shift| (x >> shift) & 1))
            .collect();
        for (prefix, data) in [("Literal", raw.as_slice()), ("Bitstring", bits.as_slice())] {
            if prefix == "Bitstring" && width == 1 {
                continue;
            }
            let label = format!("{prefix} LZ78Y Prediction Estimate: min entropy = ");
            let expected: f64 = reference
                .lines()
                .find_map(|line| line.strip_prefix(&label))
                .expect("missing LZ78Y result")
                .parse()
                .unwrap();
            let r = sp_800_90b::lz78y_estimate(data).unwrap();
            let actual = r.min_entropy.to_f64();
            eprintln!(
                "{} {prefix}: entropy difference {:.3e}",
                path.display(),
                (actual - expected).abs()
            );
            assert!(
                (actual - expected).abs() < if r.p_local.is_some() { 1e-9 } else { 1e-12 },
                "{} {prefix}: {actual} vs {expected}",
                path.display()
            );
            for (field, actual) in [
                ("C", r.correct_count),
                ("r", r.longest_correct_run + 1),
                ("N", r.prediction_count),
            ] {
                let label = format!("{prefix} LZ78Y Prediction Estimate: {field} = ");
                let expected: usize = reference
                    .lines()
                    .find_map(|line| line.strip_prefix(&label))
                    .unwrap()
                    .parse()
                    .unwrap();
                assert_eq!(actual, expected, "{} {prefix} {field}", path.display());
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 11);
}
