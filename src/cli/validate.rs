use super::*;
use rug::{Float, float::Round};
use std::collections::HashMap;

pub(super) fn assess(args: &[OsString]) -> Result<(Value, u8), CliError> {
    let mut flags = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let flag = args[i]
            .to_str()
            .ok_or_else(|| error("usage", "options must be UTF-8"))?;
        i += 1;
        if flags.contains_key(flag) {
            return Err(error("usage", format!("duplicate option {flag}")));
        }
        let boolean = ["--json", "--iid-claim", "--progress"].contains(&flag);
        if !boolean
            && ![
                "--sequential",
                "--restart",
                "--bits-per-symbol",
                "--submitter-entropy",
                "--conditioning",
                "--samples-per-block",
                "--output-bits",
                "--internal-width",
                "--conditioned-output",
                "--additional-input-bits",
                "--retained-output-bits",
                "--precision",
                "--max-working-precision",
                "--confidence",
                "--simulation-seed",
                "--simulations",
                "--seed",
            ]
            .contains(&flag)
        {
            return Err(error("usage", format!("unknown validate option {flag}")));
        }
        let value = if boolean {
            &args[i - 1]
        } else {
            let v = args
                .get(i)
                .ok_or_else(|| error("usage", format!("{flag} requires a value")))?;
            i += 1;
            v
        };
        flags.insert(flag, value);
    }
    let text = |flag: &str| -> Result<&str, CliError> {
        flags.get(flag).and_then(|v| v.to_str()).ok_or_else(|| {
            error(
                "usage",
                format!("validate requires a UTF-8 value for {flag}"),
            )
        })
    };
    let number = |flag: &str, default: Option<u64>| -> Result<u64, CliError> {
        if flags.contains_key(flag) {
            text(flag)?
                .parse()
                .map_err(|_| error("usage", format!("invalid integer for {flag}")))
        } else {
            default.ok_or_else(|| error("usage", format!("validate requires {flag}")))
        }
    };
    let n32 = |flag, default| {
        u32::try_from(number(flag, default)?)
            .map_err(|_| error("usage", format!("{flag} exceeds u32")))
    };
    let precision = n32("--precision", Some(256))?;
    let max_precision = n32("--max-working-precision", Some(1_048_576))?;
    if !(64..=rug::float::prec_max()).contains(&precision)
        || !(precision..=rug::float::prec_max()).contains(&max_precision)
    {
        return Err(error(
            "invalid_input",
            "invalid precision or working-precision ceiling",
        ));
    }
    let width = number("--bits-per-symbol", None)?;
    if !(1..=8).contains(&width) {
        return Err(error("usage", "bits per symbol must be 1..8"));
    }
    let requested_h = text("--submitter-entropy")?;
    let h = Float::with_val_round(
        precision,
        Float::parse(requested_h).map_err(|e| error("usage", e))?,
        Round::Down,
    )
    .0;
    if !h.is_finite() || h < 0 || h > width {
        return Err(error(
            "invalid_input",
            "submitter entropy must be finite and between zero and the sample width",
        ));
    }
    let seed = |flag| -> Result<Option<[u64; 4]>, CliError> {
        flags
            .contains_key(flag)
            .then(|| text(flag).and_then(parse_seed))
            .transpose()
    };
    let iid_claim = flags.contains_key("--iid-claim");
    if flags.contains_key("--seed") && !iid_claim {
        return Err(error("usage", "--seed requires --iid-claim"));
    }
    let rounds = usize::try_from(number("--simulations", Some(RESTART_SIMULATIONS as u64))?)
        .map_err(|_| error("usage", "simulation count too large"))?;
    if rounds < RESTART_SIMULATIONS {
        return Err(error("usage", "at least 5000000 simulations required"));
    }
    let confidence = match flags.get("--confidence") {
        None => ConfidenceConstant::NistReference,
        Some(_) => match text("--confidence")? {
            "reference" => ConfidenceConstant::NistReference,
            "publication" => ConfidenceConstant::Publication,
            _ => {
                return Err(error(
                    "usage",
                    "confidence must be reference or publication",
                ));
            }
        },
    };
    let mode = if flags.contains_key("--conditioning") {
        Some(text("--conditioning")?)
    } else {
        None
    };
    if mode.is_some_and(|m| m != "vetted" && m != "non-vetted") {
        return Err(error("usage", "conditioning must be vetted or non-vetted"));
    }
    if mode.is_none()
        && [
            "--samples-per-block",
            "--output-bits",
            "--internal-width",
            "--conditioned-output",
            "--additional-input-bits",
            "--retained-output-bits",
            "--max-working-precision",
        ]
        .iter()
        .any(|f| flags.contains_key(f))
    {
        return Err(error(
            "usage",
            "conditioning options require --conditioning",
        ));
    }
    if mode != Some("non-vetted") && flags.contains_key("--conditioned-output") {
        return Err(error(
            "usage",
            "--conditioned-output requires non-vetted conditioning",
        ));
    }
    let dims = mode
        .map(|_| -> Result<_, CliError> {
            let dims = (
                n32("--samples-per-block", None)?,
                n32("--output-bits", None)?,
                n32("--internal-width", None)?,
            );
            if dims.0 == 0
                || dims.1 == 0
                || dims.2 == 0
                || dims.0.checked_mul(width as u32).is_none()
            {
                return Err(error("invalid_input", "invalid conditioning dimensions"));
            }
            Ok(dims)
        })
        .transpose()?;
    let options = ValidationOptions {
        bits_per_symbol: width as u8,
        iid_claim,
        restart: RestartOptions {
            precision_bits: precision,
            confidence_constant: confidence,
            simulation_rounds: rounds,
            simulation_seed: seed("--simulation-seed")?,
            permutation: PermutationOptions {
                seed: seed("--seed")?,
                ..Default::default()
            },
            ..Default::default()
        },
        conditioning: ConditioningOptions {
            precision_bits: precision,
            max_working_precision: max_precision,
            additional_input_bits: number("--additional-input-bits", Some(0))?,
            retained_output_bits: flags
                .contains_key("--retained-output-bits")
                .then(|| n32("--retained-output-bits", None))
                .transpose()?,
        },
    };
    let read = |flag: &str| -> Result<(Vec<u8>, Value), CliError> {
        let path = flags
            .get(flag)
            .ok_or_else(|| error("usage", format!("validate requires {flag}")))?;
        let mut file = File::open(path).map_err(|e| error("input_io", format!("{flag}: {e}")))?;
        if !file.metadata().map_err(|e| error("input_io", e))?.is_file() {
            return Err(error("input_io", format!("{flag} must be a regular file")));
        }
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| error("input_io", e))?;
        let info = json!({"path":path.to_string_lossy(),"sample_count":data.len(),
            "encoding":if flag=="--conditioned-output" {"one_bit_per_byte"}else{"one_symbol_per_byte"},
            "sha256":format!("{:x}",Sha256::digest(&data))});
        Ok((data, info))
    };
    let (sequential, sequential_info) = read("--sequential")?;
    let (restart, mut restart_info) = read("--restart")?;
    restart_info["layout"] = json!("1000_by_1000_row_major");
    let conditioned = if mode == Some("non-vetted") {
        Some(read("--conditioned-output")?)
    } else {
        None
    };
    let conditioning = dims.map(|(samples_per_block, output_bits, internal_width)| {
        if let Some((data, _)) = &conditioned {
            ValidationConditioning::NonVetted {
                samples_per_block,
                output_bits,
                internal_width,
                output_data: data,
            }
        } else {
            ValidationConditioning::Vetted {
                samples_per_block,
                output_bits,
                internal_width,
            }
        }
    });
    let mut stages = Vec::new();
    let r = assess_validation_with_progress(
        &sequential,
        &restart,
        &h,
        conditioning,
        options,
        &mut |event| match event {
            ValidationProgress::StageStarted { stage } => {
                if flags.contains_key("--progress") {
                    eprintln!("Starting {stage}");
                }
            }
            ValidationProgress::StageFinished { stage, elapsed } => {
                if flags.contains_key("--progress") {
                    eprintln!("Finished {stage} ({:.2}s)", elapsed.as_secs_f64());
                }
                stages.push(stage);
            }
            _ => {}
        },
    )
    .map_err(|e| error("invalid_input", e))?;
    let (status, code) = status(r.decision);
    let stage_names = [
        "sequential",
        "restart_rows",
        "restart_columns",
        "conditioned_output",
    ];
    Ok((
        json!({
            "schema_version":1,"tool":{"name":"sp-800-90b","version":env!("CARGO_PKG_VERSION")},
            "command":"validate","status":status,"error":null,
            "inputs":{"sequential":sequential_info,"restart":restart_info,
                "conditioned_output":conditioned.as_ref().map(|(_,info)|info)},
            "options":{"bits_per_symbol":width,"iid_claim":iid_claim,"precision_bits":precision,
                "confidence_constant":if confidence==ConfidenceConstant::NistReference {"reference"}else{"publication"},
                "bitstring_limit":RESTART_SAMPLES,"simulation_rounds":rounds,
                "requested_simulation_seed":options.restart.simulation_seed.map(|s|s.map(|w|w.to_string())),
                "requested_permutation_seed":options.restart.permutation.seed.map(|s|s.map(|w|w.to_string())),
                "effective_permutation_seed":r.options.restart.permutation.seed.map(|s|s.map(|w|w.to_string())),
                "chi_square_grouping":"reference","permutation_arithmetic":"reference","permutation_early_stop":true,
                "conditioning":mode,"samples_per_block":dims.map(|d|d.0),"output_bits":dims.map(|d|d.1),
                "internal_width":dims.map(|d|d.2),"additional_input_bits":options.conditioning.additional_input_bits,
                "retained_output_bits":options.conditioning.retained_output_bits,"max_working_precision":max_precision,
                "requested_submitter_entropy":requested_h},
            "track":r.sequential.as_ref().map(|_|if r.track==RestartMode::Iid {"iid"}else{"non-iid"}),
            "iid_fallback":iid_claim && r.iid_evidence.iter().any(|e|e.decision==IidDecision::Rejected),
            "iid_evidence":r.iid_evidence.iter().enumerate().map(|(i,e)|json!({"stage":stage_names[i],"assessment":iid(e)})).collect::<Vec<_>>(),
            "completed_stages":stages,"submitter_entropy":r.submitter_entropy.to_string(),
            "initial_entropy":r.initial_entropy.as_ref().map(ToString::to_string),
            "sequential":r.sequential.as_ref().map(summary),
            "restart":r.restart.as_ref().map(restart_summary),
            "conditioned":r.conditioned.as_ref().map(summary),
            "conditioning":r.conditioning.as_ref().map(conditioning_summary),
            "raw_entropy":r.raw_entropy.as_ref().map(ToString::to_string),
            "raw_entropy_units":"bits_per_raw_sample",
            "min_entropy":r.output_entropy.as_ref().map(ToString::to_string),
            "entropy_units":if mode.is_some() {"bits_per_delivered_output_block"}else{"bits_per_raw_sample"}
        }),
        code,
    ))
}

fn status(d: ValidationDecision) -> (&'static str, u8) {
    match d {
        ValidationDecision::Complete => ("complete", 0),
        ValidationDecision::RestartRejected => ("restart_rejected", 1),
        ValidationDecision::Incomplete => ("incomplete", 3),
    }
}
fn iid(r: &IidAssessment) -> Value {
    json!({"mode":"iid","status":match r.decision {IidDecision::NotRejected=>"not_rejected",IidDecision::Rejected=>"rejected",IidDecision::Incomplete=>"incomplete"},
        "complete":r.is_complete(),"sample_count":r.sample_count,"bits_per_symbol":r.bits_per_symbol,"alphabet":r.alphabet,
        "candidate_min_entropy":r.candidate_min_entropy.to_string(),"min_entropy":r.min_entropy.as_ref().map(ToString::to_string),
        "checks":r.checks.as_ref().map(iid_checks_json),"literal":mcv_summary(r.literal.as_ref(),"bits_per_literal_symbol"),
        "bitstring":mcv_summary(r.bitstring.as_ref(),"bits_per_bit"),
        "warnings":r.warnings.iter().map(warning).collect::<Vec<_>>()})
}
fn summary(r: &ValidationDataset) -> Value {
    match r {
        ValidationDataset::Iid(r) => iid(r),
        ValidationDataset::NonIid(r) => json!({"mode":"non-iid","complete":r.is_complete(),
            "sample_count":r.sample_count,"bits_per_symbol":r.bits_per_symbol,"alphabet":r.alphabet,
            "min_entropy":r.is_complete().then(||r.min_entropy.to_string()),
            "minimum_available_entropy":r.min_entropy.to_string(),
            "literal":dataset(r.literal.as_ref(),"bits_per_literal_symbol"),
            "bitstring":dataset(r.bitstring.as_ref(),"bits_per_bit"),
            "warnings":r.warnings.iter().map(warning).collect::<Vec<_>>()}),
    }
}
fn restart_summary(r: &RestartAssessment) -> Value {
    let s = &r.sanity;
    json!({"status":restart_status(r.decision).0,"initial_entropy":r.initial_entropy.to_string(),
        "min_entropy":r.min_entropy.as_ref().map(ToString::to_string),
        "entropy_units":"bits_per_raw_sample",
        "sanity":{"status":check_status(s.reject),"seed":s.seed.map(|w|w.to_string()),
            "simulation_rounds":s.simulation_rounds,"arithmetic":"binary64",
            "max_symbol_probability":s.max_symbol_probability.to_string(),"alpha":s.alpha.to_string(),
            "effective_alphabet":s.effective_alphabet,"cutoff":s.cutoff,
            "maximum_row_count":s.maximum_row_count,"maximum_column_count":s.maximum_column_count,
            "row_maxima":s.row_maxima,"column_maxima":s.column_maxima},
        "rows":restart_dataset(r.rows.as_ref()),"columns":restart_dataset(r.columns.as_ref())})
}
fn conditioning_summary(r: &ConditioningEstimate) -> Value {
    json!({"input_accounting":{"primary_bits":r.input_accounting.primary_bits,
        "additional_bits":r.input_accounting.additional_bits,"total_bits":r.input_accounting.total_bits},
        "primary_input_entropy":r.primary_input_entropy.to_string(),"output_bits":r.output_bits,
        "effective_internal_width":r.effective_internal_width,"working_precision":r.working_precision,
        "psi_upper":r.psi_upper.to_string(),"omega_upper":r.omega_upper.to_string(),
        "output_entropy":r.output_entropy.to_string(),
        "non_vetted_cap":r.non_vetted_cap.as_ref().map(ToString::to_string),
        "assessed_output_cap":r.assessed_output_cap.as_ref().map(ToString::to_string),
        "untruncated_min_entropy":r.untruncated_min_entropy.to_string(),
        "retained_output_bits":r.retained_output_bits,"min_entropy":r.min_entropy.to_string(),
        "entropy_units":"bits_per_delivered_output_block"})
}
pub(super) fn human(r: &Value) -> String {
    let mut text = format!(
        "Numerical validation: {}\nTrack: {}; IID fallback: {}\nSubmitter bound: {} bits per raw sample\n",
        r["status"].as_str().unwrap(),
        r["track"].as_str().unwrap_or("undetermined"),
        r["iid_fallback"],
        r["submitter_entropy"].as_str().unwrap()
    );
    for e in r["iid_evidence"].as_array().unwrap() {
        text.push_str(&format!(
            "IID {}: {}\n",
            e["stage"].as_str().unwrap(),
            e["assessment"]["status"].as_str().unwrap()
        ));
    }
    if let Some(s) = r["restart"]["status"].as_str() {
        text.push_str(&format!("Restart: {s}\n"));
    }
    if let Some(h) = r["min_entropy"].as_str() {
        text.push_str(&format!(
            "Final entropy: {h} {}\n",
            r["entropy_units"].as_str().unwrap()
        ));
    } else {
        text.push_str("No final entropy awarded; inspect --json for stage diagnostics.\n");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decisions_and_incomplete_evidence() {
        assert_eq!(status(ValidationDecision::Complete), ("complete", 0));
        assert_eq!(
            status(ValidationDecision::RestartRejected),
            ("restart_rejected", 1)
        );
        assert_eq!(status(ValidationDecision::Incomplete), ("incomplete", 3));
        let r = assess_iid(&[0, 1], IidOptions::default()).unwrap();
        let summary = iid(&r);
        assert_eq!(summary["complete"], false);
        assert!(summary["min_entropy"].is_null());
        assert_eq!(summary["checks"]["permutation"]["status"], "unavailable");
        let text = human(
            &json!({"status":"incomplete","track":null,"iid_fallback":false,
            "submitter_entropy":"1","iid_evidence":[],"min_entropy":null}),
        );
        assert!(text.contains("undetermined"));
        assert!(text.contains("No final entropy"));
    }
}
