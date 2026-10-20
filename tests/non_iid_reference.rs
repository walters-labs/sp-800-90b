#![cfg(feature = "assessment")]

//! Full non-IID reports against the pinned external NIST self-test outputs.
use sp_800_90b::*;
use std::{fs, path::PathBuf};
fn value(text: &str, label: &str) -> f64 {
    text.lines()
        .find_map(|line| line.strip_prefix(label))
        .unwrap_or_else(|| panic!("missing {label}"))
        .parse()
        .unwrap()
}
fn compare_dataset(result: &NonIidDatasetAssessment, text: &str, prefix: &str) {
    assert!(result.is_complete(), "{prefix}: {result:?}");
    for result in &result.estimators {
        let label = match result.estimator {
            EstimatorKind::MostCommonValue => "Most Common Value Estimate",
            EstimatorKind::Collision => "Collision Estimate",
            EstimatorKind::Markov => "Markov Estimate",
            EstimatorKind::Compression => "Compression Estimate",
            EstimatorKind::TTuple => "t-Tuple Estimate",
            EstimatorKind::Lrs => "LRS Estimate",
            EstimatorKind::MultiMcw => "MultiMCW Prediction Estimate",
            EstimatorKind::Lag => "Lag Prediction Estimate",
            EstimatorKind::MultiMmc => "MultiMMC Prediction Estimate",
            EstimatorKind::Lz78y => "LZ78Y Prediction Estimate",
        };
        match &result.outcome {
            EstimatorOutcome::Available(details) => {
                let expected = value(text, &format!("{prefix} {label}: min entropy = "));
                let tolerance = if matches!(
                    result.estimator,
                    EstimatorKind::MultiMcw
                        | EstimatorKind::Lag
                        | EstimatorKind::MultiMmc
                        | EstimatorKind::Lz78y
                ) {
                    1e-9
                } else {
                    1e-12
                };
                assert!(
                    (details.min_entropy().to_f64() - expected).abs() < tolerance,
                    "{prefix} {label}"
                );
            }
            EstimatorOutcome::NotApplicable => {
                assert!(!text.contains(&format!("{prefix} {label}: min entropy = ")))
            }
            EstimatorOutcome::Unavailable(error) => panic!("{prefix} {label}: {error}"),
        }
    }
}
fn compare(report: &NonIidAssessment, text: &str) {
    assert!(report.is_complete());
    if let Some(literal) = &report.literal {
        compare_dataset(literal, text, "Literal");
        assert!((literal.min_entropy.to_f64() - value(text, "H_original = ")).abs() < 1e-9);
    } else {
        assert!(!text.contains("H_original = "));
    }
    if let Some(bits) = &report.bitstring {
        compare_dataset(bits, text, "Bitstring");
        assert!((bits.min_entropy.to_f64() - value(text, "H_bitstring = ")).abs() < 1e-9);
    } else {
        assert!(!text.contains("H_bitstring = "));
    }
    assert!(
        (report.min_entropy.to_f64() - value(text, "Assessed min entropy: ")).abs()
            < 1e-9 * f64::from(report.bits_per_symbol)
    );
}
#[test]
#[ignore = "requires NIST_REFERENCE_DIR and completed NIST self-tests"]
fn complete_non_iid_matches_all_eleven_nist_datasets() {
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    let mut checked = 0;
    for entry in fs::read_dir(root.join("bin")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "bin") {
            continue;
        }
        let samples = fs::read(&path).unwrap();
        let text = fs::read_to_string(
            root.join("cpp/selftest")
                .join(path.file_name().unwrap())
                .with_extension("res"),
        )
        .unwrap();
        let report = assess_non_iid(&samples, NonIidOptions::default()).unwrap();
        compare(&report, &text);
        checked += 1;
        eprintln!("{}: complete report matches ({checked}/11)", path.display());
    }
    assert_eq!(checked, 11);
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR with a built ea_non_iid binary"]
fn modes_widths_and_truncation_match_nist_executable() {
    use std::process::Command;
    let root =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").expect("set NIST_REFERENCE_DIR"));
    // Sparse two- and three-symbol alphabets, with forced eight-bit encoding.
    // A million-bit limit truncates the latter fixture partway through the data.
    let path = std::env::temp_dir().join(format!("sp80090b-non-iid-{}.bin", std::process::id()));
    let mut state = 103u64;
    let samples: Vec<u8> = (0..130_001)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            [1, 4, 7][(state % 3) as usize]
        })
        .collect();
    for (mode, limit, two_symbols) in [
        (NonIidMode::Initial, None, true),
        (NonIidMode::Conditioned, None, true),
        (NonIidMode::Initial, Some(1_000_000), false),
    ] {
        let data: Vec<u8> = samples
            .iter()
            .map(|&s| if two_symbols && s == 4 { 7 } else { s })
            .collect();
        fs::write(&path, &data).unwrap();
        let output = Command::new(root.join("cpp/ea_non_iid"))
            .args([
                if mode == NonIidMode::Initial {
                    "-i"
                } else {
                    "-c"
                },
                if limit.is_some() { "-t" } else { "-a" },
                "-v",
                "-v",
            ])
            .arg(&path)
            .arg("8")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = assess_non_iid(
            &data,
            NonIidOptions {
                mode,
                bitstring_limit: limit,
                bits_per_symbol: Some(8),
                ..Default::default()
            },
        )
        .unwrap();
        compare(&report, &String::from_utf8(output.stdout).unwrap());
        eprintln!("mode={mode:?} limit={limit:?} two_symbols={two_symbols}: matches NIST");
    }
    fs::remove_file(path).unwrap();
}

#[test]
#[ignore = "requires NIST_REFERENCE_DIR and completed NIST self-tests"]
fn reduced_wide_samples_match_nist_four_bit_assessment() {
    let root = PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").unwrap());
    let expected = fs::read(root.join("bin/truerand_4bit.bin")).unwrap();
    // Independent fixture construction: embed each reference nibble into
    // scattered positions of a 32-bit sample, with nonzero discarded bits.
    let wide: Vec<u128> = expected
        .iter()
        .map(|&v| {
            0x5555_5555u128
                | (u128::from((v >> 3) & 1) << 31)
                | (u128::from((v >> 2) & 1) << 17)
                | (u128::from((v >> 1) & 1) << 9)
                | (u128::from(v & 1) << 1)
        })
        .collect();
    let mut ranking = vec![31, 17, 9, 1];
    ranking.extend((0..32).filter(|p| ![31, 17, 9, 1].contains(p)));
    let mapping = SymbolReduction::new(32, 4, &ranking).unwrap();
    let reduced = mapping.reduce(&wide).unwrap();
    assert_eq!(reduced, expected);
    let report = assess_non_iid(
        &reduced,
        NonIidOptions {
            bits_per_symbol: Some(mapping.output_bits()),
            ..Default::default()
        },
    )
    .unwrap();
    let text = fs::read_to_string(root.join("cpp/selftest/truerand_4bit.res")).unwrap();
    compare(&report, &text);
    assert_eq!(report.sample_count, wide.len());
    assert!(report.min_entropy <= mapping.entropy_ceiling());
}
