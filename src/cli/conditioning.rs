use super::{CliError, error};
use rug::{Float, float::Round};
use serde_json::{Value, json};
use sp_800_90b::{ConditioningKind, ConditioningOptions, conditioning_entropy};
use std::{collections::HashMap, ffi::OsString};

pub(super) fn assess(args: &[OsString]) -> Result<(Value, u8), CliError> {
    let mut values = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let flag = args[i]
            .to_str()
            .ok_or_else(|| error("usage", "options must be UTF-8"))?;
        i += 1;
        if values.contains_key(flag) {
            return Err(error("usage", format!("duplicate option {flag}")));
        }
        if flag == "--json" {
            values.insert(flag, "");
            continue;
        }
        if ![
            "--mode",
            "--input-bits",
            "--input-entropy",
            "--output-bits",
            "--internal-width",
            "--assessed-entropy-per-bit",
            "--additional-input-bits",
            "--retained-output-bits",
            "--precision",
            "--max-working-precision",
        ]
        .contains(&flag)
        {
            return Err(error(
                "usage",
                format!("unknown conditioning option {flag}; no input file is accepted"),
            ));
        }
        let value = args
            .get(i)
            .and_then(|a| a.to_str())
            .ok_or_else(|| error("usage", format!("{flag} requires a UTF-8 value")))?;
        i += 1;
        values.insert(flag, value);
    }
    let required = |flag: &str| {
        values
            .get(flag)
            .copied()
            .ok_or_else(|| error("usage", format!("conditioning requires {flag}")))
    };
    let number = |flag: &str, default: Option<u64>| -> Result<u64, CliError> {
        match values.get(flag) {
            Some(s) => s
                .parse()
                .map_err(|_| error("usage", format!("invalid integer for {flag}"))),
            None => default.ok_or_else(|| error("usage", format!("conditioning requires {flag}"))),
        }
    };
    let number32 = |flag, default| {
        u32::try_from(number(flag, default)?)
            .map_err(|_| error("usage", format!("{flag} exceeds u32")))
    };
    let precision = number32("--precision", Some(256))?;
    let max_precision = number32("--max-working-precision", Some(1_048_576))?;
    // Validate before constructing any MPFR value (rug rejects invalid precision).
    if !(64..=rug::float::prec_max()).contains(&precision)
        || !(precision..=rug::float::prec_max()).contains(&max_precision)
    {
        return Err(error(
            "invalid_input",
            "invalid precision or working-precision ceiling",
        ));
    }
    let decimal = |flag: &str| -> Result<Float, CliError> {
        let parsed = Float::parse(required(flag)?)
            .map_err(|e| error("usage", format!("invalid {flag}: {e}")))?;
        let h = Float::with_val_round(precision, parsed, Round::Down).0;
        if !h.is_finite() || h < 0 {
            return Err(error(
                "invalid_input",
                format!("{flag} must be finite and nonnegative"),
            ));
        }
        Ok(h)
    };
    let mode = required("--mode")?;
    let kind = match mode {
        "vetted" => {
            if values.contains_key("--assessed-entropy-per-bit") {
                return Err(error(
                    "usage",
                    "--assessed-entropy-per-bit requires non-vetted mode",
                ));
            }
            ConditioningKind::Vetted
        }
        "non-vetted" => ConditioningKind::NonVetted {
            assessed_entropy_per_bit: decimal("--assessed-entropy-per-bit")?,
        },
        _ => {
            return Err(error(
                "usage",
                "conditioning mode must be vetted or non-vetted",
            ));
        }
    };
    let h = decimal("--input-entropy")?;
    let options = ConditioningOptions {
        precision_bits: precision,
        max_working_precision: max_precision,
        additional_input_bits: number("--additional-input-bits", Some(0))?,
        retained_output_bits: if values.contains_key("--retained-output-bits") {
            Some(number32("--retained-output-bits", None)?)
        } else {
            None
        },
    };
    let r = conditioning_entropy(
        number32("--input-bits", None)?,
        number32("--output-bits", None)?,
        number32("--internal-width", None)?,
        &h,
        kind,
        options,
    )
    .map_err(|e| error("invalid_input", e))?;
    Ok((
        json!({
            "schema_version": 1, "tool": {"name":"sp-800-90b","version":env!("CARGO_PKG_VERSION")},
            "command":"conditioning", "status":"complete", "error":null,
            "entropy_units":"bits_per_delivered_output_block",
            "options": {
                "mode":mode, "precision_bits":precision, "max_working_precision":max_precision,
                "requested_input_entropy":required("--input-entropy")?,
                "requested_assessed_entropy_per_bit":values.get("--assessed-entropy-per-bit"),
                "retained_output_bits":options.retained_output_bits
            },
            "input_accounting":{"primary_bits":r.input_accounting.primary_bits,
                "additional_bits":r.input_accounting.additional_bits,"total_bits":r.input_accounting.total_bits},
            "primary_input_entropy":r.primary_input_entropy.to_string(),
            "assessed_entropy_per_bit":if mode=="non-vetted" {Some(decimal("--assessed-entropy-per-bit")?.to_string())}else{None},
            "output_bits":r.output_bits, "internal_width":number32("--internal-width",None)?,
            "effective_internal_width":r.effective_internal_width,
            "retained_output_bits":r.retained_output_bits, "working_precision":r.working_precision,
            "psi_upper":r.psi_upper.to_string(), "omega_upper":r.omega_upper.to_string(),
            "output_entropy":r.output_entropy.to_string(),
            "non_vetted_cap":r.non_vetted_cap.as_ref().map(ToString::to_string),
            "assessed_output_cap":r.assessed_output_cap.as_ref().map(ToString::to_string),
            "untruncated_min_entropy":r.untruncated_min_entropy.to_string(),
            "min_entropy":if r.min_entropy.is_zero() { "0".to_owned() } else { r.min_entropy.to_string() }
        }),
        0,
    ))
}

pub(super) fn human(r: &Value) -> String {
    let mut text = format!(
        "Conditioning calculation: complete ({})\nInput: {} primary bits + {} additional bits = {} total bits\nPrimary input entropy: {} bits per block\nOutput: {} full bits; {} retained bits\n",
        r["options"]["mode"].as_str().unwrap(),
        r["input_accounting"]["primary_bits"],
        r["input_accounting"]["additional_bits"],
        r["input_accounting"]["total_bits"],
        r["primary_input_entropy"].as_str().unwrap(),
        r["output_bits"],
        r["retained_output_bits"]
    );
    for (key, label) in [
        ("output_entropy", "Output entropy bound"),
        ("non_vetted_cap", "Non-vetted cap"),
        ("assessed_output_cap", "Assessed output cap"),
        ("untruncated_min_entropy", "Untruncated bound"),
        ("min_entropy", "Delivered entropy bound"),
    ] {
        if let Some(value) = r[key].as_str() {
            text.push_str(&format!("{label}: {value} bits per block\n"));
        }
    }
    text
}
