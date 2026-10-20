#![cfg(feature = "cli")]

use rug::Float;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sp_800_90b::*;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Input(PathBuf);
impl Input {
    fn new(bytes: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "sp90b-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_sp-800-90b"))
            .arg("non-iid")
            .args(args)
            .arg(&self.0)
            .output()
            .unwrap()
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn report(output: &Output, code: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn subset_constant_and_hash() {
    let file = Input::new(&[255, 7, 7, 7, 255]);
    let r = report(
        &file.run(&[
            "--json",
            "--offset",
            "1",
            "--samples",
            "3",
            "--bits-per-symbol",
            "3",
        ]),
        0,
    );
    assert_eq!(r["status"], "complete");
    assert_eq!(r["input"]["sample_count"], 3);
    assert_eq!(
        r["input"]["selected_samples_sha256"],
        format!("{:x}", Sha256::digest([7, 7, 7]))
    );
    assert_eq!(r["alphabet"], serde_json::json!([7]));
    assert_eq!(
        Float::parse(r["min_entropy"].as_str().unwrap())
            .map(|x| Float::with_val(256, x))
            .unwrap(),
        0
    );
    assert!(r["literal"].is_null());
    assert!(
        r["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["code"] == "constant_input")
    );
    let r = report(
        &file.run(&["--json", "--offset", "3", "--samples", "100"]),
        3,
    );
    assert_eq!(r["input"]["sample_count"], 2); // at most N, through EOF
}
#[test]
fn incomplete_never_presents_partial_as_final() {
    let file = Input::new(&[0, 1]);
    let r = report(&file.run(&["--json"]), 3);
    assert_eq!(r["status"], "incomplete");
    assert!(r["min_entropy"].is_null());
    assert!(r["minimum_available_entropy"].is_string());
    assert!(
        r["literal"]["estimators"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["status"] == "unavailable" && e["error"]["message"].is_string())
    );
    let human = file.run(&[]);
    assert_eq!(human.status.code(), Some(3));
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains("incomplete")
    );
}
#[test]
fn modes_and_precision_match_library() {
    let mut state = 19u64;
    let bytes: Vec<u8> = (0..6000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % 4) as u8
        })
        .collect();
    let file = Input::new(&bytes);
    for (label, mode) in [
        ("initial", NonIidMode::Initial),
        ("conditioned", NonIidMode::Conditioned),
    ] {
        let expected = assess_non_iid(
            &bytes,
            NonIidOptions {
                mode,
                bits_per_symbol: Some(2),
                precision_bits: 384,
                confidence_constant: ConfidenceConstant::Publication,
                bitstring_limit: Some(10000),
            },
        )
        .unwrap();
        assert!(
            expected.is_complete(),
            "fixture must exercise a complete nonconstant assessment"
        );
        let r = report(
            &file.run(&[
                "--json",
                "--mode",
                label,
                "--bits-per-symbol",
                "2",
                "--precision",
                "384",
                "--confidence",
                "publication",
                "--bitstring-limit",
                "10000",
            ]),
            if expected.is_complete() { 0 } else { 3 },
        );
        let parsed = Float::with_val(
            384,
            Float::parse(r["minimum_available_entropy"].as_str().unwrap()).unwrap(),
        );
        assert_eq!(parsed, expected.min_entropy); // decimal strings round-trip MPFR
        assert_eq!(r["bitstring"]["sample_count"], 10000);
        assert_eq!(
            r["literal"].is_null(),
            matches!(mode, NonIidMode::Conditioned)
        );
        assert_eq!(r["entropy_units"], "bits_per_input_symbol");
        if matches!(mode, NonIidMode::Initial) {
            assert!(
                r["literal"]["estimators"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["status"] == "not_applicable")
            );
        }
    }
}
#[test]
fn errors_are_structured_and_nonzero() {
    let file = Input::new(&[0, 4]);
    for args in [
        vec!["--bits-per-symbol", "2"],
        vec!["--offset", "99"],
        vec!["--samples", "0"],
        vec!["--precision", "63"],
        vec!["--unknown"],
        vec!["--mode", "wrong"],
        vec!["--offset", "1", "--offset", "2"],
        vec!["--offset", "18446744073709551616"],
    ] {
        let mut flags = vec!["--json"];
        flags.extend(args);
        let r = report(&file.run(&flags), 2);
        assert_eq!(r["status"], "error");
        assert!(r["min_entropy"].is_null());
    }
    for bytes in [vec![], vec![0]] {
        report(&Input::new(&bytes).run(&["--json"]), 2);
    }
    fs::remove_file(&file.0).unwrap();
    assert_eq!(
        report(&file.run(&["--json"]), 2)["error"]["code"],
        "input_io"
    );
}
#[test]
fn help_version_and_option_terminator() {
    for arg in ["--help", "--version"] {
        assert!(
            Command::new(env!("CARGO_BIN_EXE_sp-800-90b"))
                .arg(arg)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let file = Input::new(&[0, 0]);
    report(&file.run(&["--json", "--"]), 0);
}

impl Input {
    fn iid(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_sp-800-90b"))
            .arg("iid")
            .args(args)
            .arg(&self.0)
            .output()
            .unwrap()
    }
}
fn iid_data() -> Vec<u8> {
    let mut state = 123456789u64;
    (0..300)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % 7) as u8
        })
        .collect()
}
#[test]
fn iid_modes_match_library_checks_and_precision() {
    let bytes = iid_data();
    let mut padded = vec![255];
    padded.extend(&bytes);
    padded.push(255);
    let input = Input::new(&padded);
    for (label, mode) in [
        ("initial", IidMode::Initial),
        ("conditioned", IidMode::Conditioned),
    ] {
        let expected = assess_iid(
            &bytes,
            IidOptions {
                mode,
                bits_per_symbol: Some(3),
                precision_bits: 384,
                bitstring_limit: Some(400),
                permutation: PermutationOptions {
                    seed: Some([1, 2, 3, 4]),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(expected.decision, IidDecision::NotRejected);
        let r = report(
            &input.iid(&[
                "--json",
                "--mode",
                label,
                "--bits-per-symbol",
                "3",
                "--precision",
                "384",
                "--bitstring-limit",
                "400",
                "--seed",
                "1,2,3,4",
                "--offset",
                "1",
                "--samples",
                "300",
            ]),
            0,
        );
        assert_eq!(r["command"], "iid");
        assert_eq!(r["status"], "not_rejected");
        assert_eq!(r["checks_complete"], true);
        assert_eq!(
            Float::with_val(
                384,
                Float::parse(r["min_entropy"].as_str().unwrap()).unwrap()
            ),
            expected.min_entropy.unwrap()
        );
        assert_eq!(r["bitstring"]["sample_count"], 400);
        assert_eq!(r["literal"].is_null(), matches!(mode, IidMode::Conditioned));
        let checks = expected.checks.unwrap();
        assert_eq!(
            r["checks"]["independence"]["p_value"],
            checks.independence.unwrap().p_value.to_string()
        );
        assert_eq!(
            r["checks"]["goodness_of_fit"]["p_value"],
            checks.goodness_of_fit.unwrap().p_value.to_string()
        );
        assert_eq!(
            r["checks"]["longest_repeated_substring"]["p_value"],
            checks
                .longest_repeated_substring
                .unwrap()
                .p_value
                .to_string()
        );
        let perm = checks.permutation.unwrap();
        assert_eq!(
            r["checks"]["permutation"]["permutations_performed"],
            perm.permutations_performed
        );
        for (actual, expected) in r["checks"]["permutation"]["statistics"]
            .as_array()
            .unwrap()
            .iter()
            .zip(perm.tests)
        {
            assert_eq!(actual["greater"], expected.greater);
            assert_eq!(actual["equal"], expected.equal);
            assert_eq!(actual["less"], expected.less);
        }
    }
}
#[test]
fn iid_rejected_incomplete_and_invalid_are_distinct() {
    let constant = Input::new(&[7; 300]);
    let r = report(&constant.iid(&["--json"]), 1);
    assert_eq!(r["status"], "rejected");
    assert!(r["checks"].is_null());
    assert!(r["min_entropy"].is_null());
    assert!(r["candidate_min_entropy"].is_string());
    let short = Input::new(&[0, 1]);
    let r = report(&short.iid(&["--json", "--seed", "1,2,3,4"]), 3);
    assert_eq!(r["status"], "incomplete");
    assert!(r["min_entropy"].is_null());
    assert_eq!(r["checks"]["permutation"]["status"], "unavailable");
    // A detected rejection takes precedence over another unavailable check.
    let patterned = Input::new(&[0, 1].repeat(40));
    let r = report(&patterned.iid(&["--json", "--seed", "1,2,3,4"]), 1);
    assert_eq!(r["status"], "rejected");
    assert!(r["min_entropy"].is_null());
    for seed in [
        "0,0,0,0",
        "1,2,3",
        "1,2,3,4,5",
        "1,-2,3,4",
        "1,2,3,18446744073709551616",
    ] {
        let r = report(&short.iid(&["--json", "--seed", seed]), 2);
        assert_eq!(r["command"], "iid");
        assert_eq!(r["status"], "error");
    }
    report(&short.run(&["--json", "--seed", "1,2,3,4"]), 2);
    let output = constant.iid(&[]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("No eligible entropy result"));
}
#[test]
fn iid_os_seed_can_be_replayed() {
    let file = Input::new(&iid_data());
    let first = file.iid(&["--json"]);
    let first_code = first.status.code().unwrap();
    assert!([0, 1].contains(&first_code));
    let r = report(&first, first_code);
    let seed = r["checks"]["permutation"]["seed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>()
        .join(",");
    let replay = report(&file.iid(&["--json", "--seed", &seed]), first_code);
    assert_eq!(r["checks"], replay["checks"]);
    assert_eq!(r["min_entropy"], replay["min_entropy"]);
}

impl Input {
    fn restart(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_sp-800-90b"))
            .arg("restart")
            .args(args)
            .arg(&self.0)
            .output()
            .unwrap()
    }
}
#[test]
fn restart_invalid_input_and_options() {
    let file = Input::new(&[0, 1]);
    for flags in [
        vec![],
        vec!["--initial-entropy", "0"],
        vec!["--initial-entropy", "NaN"],
        vec!["--initial-entropy", "-1"],
        vec!["--initial-entropy", "wrong"],
        vec!["--initial-entropy", "0", "--simulations", "4999999"],
        vec!["--initial-entropy", "0", "--bitstring-limit", "100"],
        vec!["--initial-entropy", "0", "--seed", "1,2,3,4"],
        vec!["--initial-entropy", "0", "--simulation-seed", "0,0,0,0"],
        vec!["--initial-entropy", "0", "--mode", "initial"],
    ] {
        let mut args = vec!["--json"];
        args.extend(flags);
        let r = report(&file.restart(&args), 2);
        assert_eq!(r["command"], "restart");
        assert!(r["min_entropy"].is_null());
    }
    report(&file.iid(&["--json", "--initial-entropy", "0"]), 2);
    let constant = Input::new(&vec![0; RESTART_SAMPLES]);
    report(&constant.restart(&["--json", "--initial-entropy", "0"]), 2);
}
#[test]
#[ignore = "million-sample restart CLI assessments in both modes"]
fn restart_success_in_both_modes() {
    let mut state = 42u64;
    let data: Vec<u8> = (0..RESTART_SAMPLES)
        .map(|_| {
            state = state.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            ((z ^ (z >> 31)) >> 63) as u8
        })
        .collect();
    let mut padded = vec![255];
    padded.extend(&data);
    padded.push(255);
    let file = Input::new(&padded);
    for mode in ["non-iid", "iid"] {
        let mut args = vec![
            "--json",
            "--initial-entropy",
            "0",
            "--mode",
            mode,
            "--bits-per-symbol",
            "1",
            "--offset",
            "1",
            "--samples",
            "1000000",
            "--simulation-seed",
            "1,2,3,4",
        ];
        if mode == "iid" {
            args.extend(["--seed", "1,2,3,4"]);
        }
        let r = report(&file.restart(&args), 0);
        assert_eq!(r["status"], "validated");
        assert_eq!(r["min_entropy"], "0");
        assert_eq!(r["sanity"]["cutoff"], 1000);
        assert_eq!(r["sanity"]["row_maxima"].as_array().unwrap().len(), 1000);
        assert_eq!(r["sanity"]["column_maxima"].as_array().unwrap().len(), 1000);
        assert_eq!(
            r["input"]["selected_samples_sha256"],
            format!("{:x}", Sha256::digest(&data))
        );
        for branch in ["rows", "columns"] {
            assert_eq!(r[branch]["complete"], true);
            assert_eq!(r[branch]["mode"], mode);
            if mode == "iid" {
                assert_eq!(r[branch]["checks"]["permutation"]["status"], "not_rejected");
            } else {
                assert_eq!(r[branch]["estimators"].as_array().unwrap().len(), 10);
            }
        }
    }
}
#[test]
#[ignore = "five-million-round restart sanity simulation"]
fn restart_sanity_rejection_has_no_entropy_or_datasets() {
    let file = Input::new(&[0, 1].repeat(RESTART_SAMPLES / 2));
    let r = report(
        &file.restart(&[
            "--json",
            "--initial-entropy",
            "1",
            "--simulation-seed",
            "1,2,3,4",
        ]),
        1,
    );
    assert_eq!(r["status"], "sanity_rejected");
    assert!(r["min_entropy"].is_null());
    assert!(r["rows"].is_null());
    assert!(r["columns"].is_null());
    assert_eq!(r["sanity"]["maximum_row_count"], 500);
    assert_eq!(r["sanity"]["maximum_column_count"], 1000);
}

fn conditioning(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sp-800-90b"))
        .arg("conditioning")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn conditioning_modes_match_library() {
    for vetted in [true, false] {
        let mut args = vec![
            "--json",
            "--mode",
            if vetted { "vetted" } else { "non-vetted" },
            "--input-bits",
            "512",
            "--input-entropy",
            "400.1234567890123456789",
            "--output-bits",
            "256",
            "--internal-width",
            "256",
            "--precision",
            "384",
        ];
        if vetted {
            args.extend([
                "--additional-input-bits",
                "128",
                "--retained-output-bits",
                "128",
            ]);
        } else {
            args.extend(["--assessed-entropy-per-bit", "0.75"]);
        }
        let r = report(&conditioning(&args), 0);
        let h = Float::with_val_round(
            384,
            Float::parse("400.1234567890123456789").unwrap(),
            rug::float::Round::Down,
        )
        .0;
        let expected = conditioning_entropy(
            512,
            256,
            256,
            &h,
            if vetted {
                ConditioningKind::Vetted
            } else {
                ConditioningKind::NonVetted {
                    assessed_entropy_per_bit: Float::with_val(384, 0.75),
                }
            },
            ConditioningOptions {
                precision_bits: 384,
                additional_input_bits: if vetted { 128 } else { 0 },
                retained_output_bits: if vetted { Some(128) } else { None },
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r["status"], "complete");
        assert_eq!(r["min_entropy"], expected.min_entropy.to_string());
        assert_eq!(r["psi_upper"], expected.psi_upper.to_string());
        assert_eq!(r["omega_upper"], expected.omega_upper.to_string());
        assert_eq!(
            r["input_accounting"]["total_bits"],
            if vetted { 640 } else { 512 }
        );
        assert_eq!(r["retained_output_bits"], if vetted { 128 } else { 256 });
        assert_eq!(
            r["non_vetted_cap"],
            serde_json::to_value(expected.non_vetted_cap.map(|h| h.to_string())).unwrap()
        );
        assert_eq!(
            r["assessed_output_cap"],
            serde_json::to_value(expected.assessed_output_cap.map(|h| h.to_string())).unwrap()
        );
    }
}

#[test]
fn conditioning_errors_and_zero_entropy() {
    let base = [
        "--mode",
        "vetted",
        "--input-bits",
        "512",
        "--input-entropy",
        "0",
        "--output-bits",
        "256",
        "--internal-width",
        "256",
    ];
    let mut good = base.to_vec();
    good.push("--json");
    assert_eq!(report(&conditioning(&good), 0)["min_entropy"], "0");
    for extra in [
        vec!["--precision", "0"],
        vec!["--precision", "4294967296"],
        vec!["--max-working-precision", "64"],
        vec!["--max-working-precision", "256"],
        vec!["--retained-output-bits", "257"],
        vec!["--additional-input-bits", "18446744073709551615"],
        vec!["--assessed-entropy-per-bit", "0.5"],
        vec!["samples.bin"],
        vec!["--seed", "1,2,3,4"],
        vec!["--json"],
    ] {
        let mut args = good.clone();
        args.extend(extra);
        let r = report(&conditioning(&args), 2);
        assert_eq!(r["command"], "conditioning");
        assert!(r["min_entropy"].is_null());
    }
    for h in ["NaN", "inf", "-1", "513", "oops"] {
        let mut args = good.clone();
        args[5] = h;
        report(&conditioning(&args), 2);
    }
    for extra in [
        vec![],
        vec!["--assessed-entropy-per-bit", "1.1"],
        vec![
            "--assessed-entropy-per-bit",
            "0.5",
            "--additional-input-bits",
            "1",
        ],
        vec![
            "--assessed-entropy-per-bit",
            "0.5",
            "--retained-output-bits",
            "256",
        ],
    ] {
        let mut args = good.clone();
        args[1] = "non-vetted";
        args.extend(extra);
        report(&conditioning(&args), 2);
    }
    report(&conditioning(&["--json"]), 2);
    let human = conditioning(&base);
    assert!(human.status.success());
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains("Delivered entropy bound: 0")
    );
}

fn validate(seq: &Input, restart: &Input, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sp-800-90b"))
        .args(["validate", "--sequential"])
        .arg(&seq.0)
        .arg("--restart")
        .arg(&restart.0)
        .args(args)
        .output()
        .unwrap()
}
fn validation_random() -> Vec<u8> {
    let mut state = 42u64;
    (0..RESTART_SAMPLES)
        .map(|_| {
            state = state.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            ((z ^ (z >> 31)) >> 63) as u8
        })
        .collect()
}
#[test]
fn validate_invalid_inputs_and_options() {
    let small = Input::new(&[0, 1]);
    let base = [
        "--json",
        "--bits-per-symbol",
        "1",
        "--submitter-entropy",
        "0",
    ];
    report(&validate(&small, &small, &base), 2);
    for extra in [
        vec!["--iid-claim", "--iid-claim"],
        vec!["--precision", "0"],
        vec!["--seed", "1,2,3,4"],
        vec!["--simulation-seed", "0,0,0,0"],
        vec!["--simulations", "1"],
        vec!["--samples-per-block", "8"],
        vec!["--conditioning", "bad"],
        vec!["--conditioning", "vetted"],
        vec!["--conditioned-output", "missing"],
        vec!["--mode", "iid"],
        vec!["--offset", "1"],
    ] {
        let mut args = base.to_vec();
        args.extend(extra);
        let r = report(&validate(&small, &small, &args), 2);
        assert_eq!(r["command"], "validate");
        assert!(r["min_entropy"].is_null());
    }
    for h in ["NaN", "inf", "-1", "2", "invalid"] {
        let mut args = base;
        args[4] = h;
        report(&validate(&small, &small, &args), 2);
    }
    let missing = Input(small.0.with_extension("missing"));
    assert_eq!(
        report(&validate(&missing, &small, &base), 2)["error"]["code"],
        "input_io"
    );
    let large = Input::new(&vec![0; RESTART_SAMPLES]);
    report(&validate(&large, &small, &base), 2);
    let bad_symbol = Input::new(&vec![2; RESTART_SAMPLES]);
    report(&validate(&bad_symbol, &large, &base), 2);
    let mut args = base.to_vec();
    args.extend([
        "--conditioning",
        "non-vetted",
        "--samples-per-block",
        "8",
        "--output-bits",
        "2",
        "--internal-width",
        "8",
        "--conditioned-output",
    ]);
    args.push(large.0.to_str().unwrap());
    // One million bits cannot represent one million two-bit output blocks.
    report(&validate(&large, &large, &args), 2);
}

#[test]
#[ignore = "full-size validation CLI workflows, including conditioned output"]
fn validate_conditioning_and_fallback() {
    let zero = Input::new(&vec![0; RESTART_SAMPLES]);
    let restart = Input::new(&validation_random());
    for mode in ["none", "vetted", "non-vetted"] {
        let mut args = vec![
            "--json",
            "--bits-per-symbol",
            "1",
            "--submitter-entropy",
            "0.25",
            "--simulation-seed",
            "1,2,3,4",
        ];
        if mode != "none" {
            args.extend([
                "--conditioning",
                mode,
                "--samples-per-block",
                "8",
                "--output-bits",
                if mode == "vetted" { "2" } else { "1" },
                "--internal-width",
                "8",
            ]);
        }
        if mode == "non-vetted" {
            args.extend([
                "--iid-claim",
                "--seed",
                "1,2,3,4",
                "--conditioned-output",
                zero.0.to_str().unwrap(),
            ]);
        }
        if mode == "vetted" {
            args.extend([
                "--additional-input-bits",
                "32",
                "--retained-output-bits",
                "1",
            ]);
        }
        let sequential = if mode == "vetted" { &restart } else { &zero };
        let r = report(&validate(sequential, &restart, &args), 0);
        assert_eq!(r["status"], "complete");
        assert_eq!(r["track"], "non-iid");
        assert_eq!(r["iid_fallback"], mode == "non-vetted");
        let expected = if mode == "vetted" {
            conditioning_entropy(
                8,
                2,
                8,
                &Float::with_val(256, 2),
                ConditioningKind::Vetted,
                ConditioningOptions {
                    additional_input_bits: 32,
                    retained_output_bits: Some(1),
                    ..Default::default()
                },
            )
            .unwrap()
            .min_entropy
        } else {
            Float::with_val(256, 0)
        };
        assert_eq!(
            Float::parse(r["min_entropy"].as_str().unwrap())
                .map(|v| Float::with_val(256, v))
                .unwrap(),
            expected
        );
        assert_eq!(r["restart"]["status"], "validated");
        assert_eq!(
            r["inputs"]["restart"]["sha256"],
            format!("{:x}", Sha256::digest(validation_random()))
        );
        assert_eq!(
            r["entropy_units"],
            if mode == "none" {
                "bits_per_raw_sample"
            } else {
                "bits_per_delivered_output_block"
            }
        );
        if mode == "vetted" {
            assert_eq!(r["conditioning"]["input_accounting"]["total_bits"], 40);
            assert_eq!(r["raw_entropy"], Float::with_val(256, 0.25).to_string());
        }
        if mode == "non-vetted" {
            assert_eq!(r["iid_evidence"][0]["stage"], "sequential");
            assert_eq!(r["iid_evidence"][0]["assessment"]["status"], "rejected");
            assert!(r["conditioning"]["assessed_output_cap"].is_string());
            assert_eq!(r["conditioned"]["mode"], "non-iid");
        }
    }
}
#[test]
#[ignore = "full-size IID validation CLI workflow"]
fn validate_iid_success() {
    let data = Input::new(&validation_random());
    let r = report(
        &validate(
            &data,
            &data,
            &[
                "--json",
                "--iid-claim",
                "--bits-per-symbol",
                "1",
                "--submitter-entropy",
                "0",
                "--seed",
                "1,2,3,4",
                "--simulation-seed",
                "1,2,3,4",
            ],
        ),
        0,
    );
    assert_eq!(r["track"], "iid");
    assert_eq!(r["iid_fallback"], false);
    assert_eq!(r["iid_evidence"].as_array().unwrap().len(), 3);
    assert_eq!(r["min_entropy"], "0");
    assert_eq!(r["restart"]["rows"]["mode"], "iid");
    assert_eq!(
        r["options"]["effective_permutation_seed"],
        serde_json::json!(["1", "2", "3", "4"])
    );
}
#[test]
#[ignore = "full-size validation rejection and sanity simulation"]
fn validate_restart_rejection() {
    let data = Input::new(&validation_random());
    let restart = Input::new(
        &(0..RESTART_SAMPLES)
            .map(|i| ((i / 1000) % 2) as u8)
            .collect::<Vec<_>>(),
    );
    let r = report(
        &validate(
            &data,
            &restart,
            &[
                "--json",
                "--bits-per-symbol",
                "1",
                "--submitter-entropy",
                "0.25",
                "--simulation-seed",
                "1,2,3,4",
            ],
        ),
        1,
    );
    assert_eq!(r["status"], "restart_rejected");
    assert_eq!(r["restart"]["status"], "sanity_rejected");
    assert!(r["min_entropy"].is_null());
    assert!(r["raw_entropy"].is_null());
}

fn transpose(input: &Input, output: &Input, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sp-800-90b"))
        .args(["transpose", "--input"])
        .arg(&input.0)
        .arg("--output")
        .arg(&output.0)
        .args(args)
        .output()
        .unwrap()
}
fn new_output(input: &Input, suffix: &str) -> Input {
    Input(input.0.with_extension(suffix))
}
#[test]
fn transpose_order_subset_hash_and_round_trip() {
    let matrix: Vec<u8> = (0..RESTART_SAMPLES)
        .map(|i| ((i / 1000 * 17 + i % 1000 * 31) % 256) as u8)
        .collect();
    let mut padded = vec![255, 254];
    padded.extend(&matrix);
    padded.push(253);
    let input = Input::new(&padded);
    let output = new_output(&input, "transposed");
    let r = report(
        &transpose(
            &input,
            &output,
            &["--json", "--offset", "2", "--samples", "1000000"],
        ),
        0,
    );
    let bytes = fs::read(&output.0).unwrap();
    assert_eq!(bytes.len(), RESTART_SAMPLES);
    for row in 0..1000 {
        for col in 0..1000 {
            assert_eq!(bytes[col * 1000 + row], matrix[row * 1000 + col]);
        }
    }
    assert_eq!(
        r["input"]["selected_samples_sha256"],
        format!("{:x}", Sha256::digest(&matrix))
    );
    assert_eq!(
        r["output"]["sha256"],
        format!("{:x}", Sha256::digest(&bytes))
    );
    assert_eq!(r["status"], "complete");
    assert_eq!(fs::read(&input.0).unwrap(), padded);
    let restored = new_output(&input, "restored");
    let human = transpose(&output, &restored, &[]);
    assert!(human.status.success());
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains("column-major")
    );
    assert_eq!(fs::read(&restored.0).unwrap(), matrix);
}
#[test]
fn transpose_rejects_bad_inputs_and_preserves_existing_files() {
    let input = Input::new(&vec![0; RESTART_SAMPLES]);
    let output = Input::new(b"keep me");
    assert_eq!(
        report(&transpose(&input, &output, &["--json"]), 2)["error"]["code"],
        "output_io"
    );
    assert_eq!(fs::read(&output.0).unwrap(), b"keep me");
    report(&transpose(&input, &input, &["--json"]), 2);
    assert_eq!(fs::read(&input.0).unwrap().len(), RESTART_SAMPLES);
    let new = new_output(&input, "new");
    for extra in [
        vec!["--samples", "999999"],
        vec!["--offset", "1"],
        vec!["--offset", "-1"],
        vec!["--offset", "18446744073709551616"],
        vec!["--samples"],
        vec!["--bits-per-symbol", "8"],
        vec!["--json"],
    ] {
        let mut args = vec!["--json"];
        args.extend(extra);
        report(&transpose(&input, &new, &args), 2);
        assert!(!new.0.exists());
    }
    for len in [0, 999999, 1000001] {
        let bad = Input::new(&vec![0; len]);
        assert_eq!(
            report(&transpose(&bad, &new, &["--json"]), 2)["error"]["code"],
            "invalid_input"
        );
        assert!(!new.0.exists());
    }
    let missing = new_output(&input, "missing");
    assert_eq!(
        report(&transpose(&missing, &new, &["--json"]), 2)["error"]["code"],
        "input_io"
    );
    #[cfg(unix)]
    {
        let alias = new_output(&input, "alias");
        std::os::unix::fs::symlink(&input.0, &alias.0).unwrap();
        report(&transpose(&input, &alias, &["--json"]), 2);
        assert_eq!(fs::read(&input.0).unwrap(), vec![0; RESTART_SAMPLES]);
    }
}
