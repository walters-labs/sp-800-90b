use super::*;
use std::{collections::HashMap, fs::OpenOptions};

pub(super) fn run(args: &[OsString]) -> Result<(Value, u8), CliError> {
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
        if !["--input", "--output", "--offset", "--samples", "--json"].contains(&flag) {
            return Err(error("usage", format!("unknown transpose option {flag}")));
        }
        let value = if flag == "--json" {
            &args[i - 1]
        } else {
            let value = args
                .get(i)
                .ok_or_else(|| error("usage", format!("{flag} requires a value")))?;
            i += 1;
            value
        };
        flags.insert(flag, value);
    }
    let required = |flag| {
        flags
            .get(flag)
            .copied()
            .ok_or_else(|| error("usage", format!("transpose requires {flag}")))
    };
    let number = |flag| -> Result<Option<u64>, CliError> {
        flags
            .get(flag)
            .map(|v| {
                v.to_str()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| error("usage", format!("invalid integer for {flag}")))
            })
            .transpose()
    };
    let input = required("--input")?;
    let output = required("--output")?;
    let offset = number("--offset")?.unwrap_or(0);
    let samples = number("--samples")?;
    if samples.is_some_and(|n| n != RESTART_SAMPLES as u64) {
        return Err(error("usage", "--samples must be 1000000"));
    }
    let mut file = File::open(input).map_err(|e| error("input_io", e))?;
    if !file.metadata().map_err(|e| error("input_io", e))?.is_file() {
        return Err(error("input_io", "input must be a regular file"));
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| error("input_io", e))?;
    // One extra byte detects oversize selections without loading an unbounded file.
    let mut data = Vec::new();
    file.take(samples.unwrap_or(RESTART_SAMPLES as u64 + 1))
        .read_to_end(&mut data)
        .map_err(|e| error("input_io", e))?;
    let transposed = restart_column_sequence(&data).map_err(|_| {
        error(
            "invalid_input",
            "selected input must contain exactly 1000000 bytes",
        )
    })?;
    // Exclusive creation also rejects symlinks and aliases of the input.
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|e| error("output_io", format!("cannot create new output file: {e}")))?;
    destination
        .write_all(&transposed)
        .and_then(|_| destination.sync_all())
        .map_err(|e| {
            error(
                "output_io",
                format!("writing output failed; the new file may be partial: {e}"),
            )
        })?;
    Ok((
        json!({
            "schema_version":1,"tool":{"name":"sp-800-90b","version":env!("CARGO_PKG_VERSION")},
            "command":"transpose","status":"complete","error":null,
            "rows":1000,"columns":1000,"sample_count":RESTART_SAMPLES,
            "encoding":"one_symbol_per_byte",
            "input":{"path":input.to_string_lossy(),"layout":"row_major","offset_samples":offset,
                "requested_samples":samples,"selected_samples_sha256":format!("{:x}",Sha256::digest(&data))},
            "output":{"path":output.to_string_lossy(),"layout":"column_major",
                "sha256":format!("{:x}",Sha256::digest(&transposed))}
        }),
        0,
    ))
}

pub(super) fn human(r: &Value) -> String {
    format!(
        "Transposed 1000 × 1000 samples into column-major order.\nOutput: {}\nSHA-256: {}\n",
        r["output"]["path"].as_str().unwrap(),
        r["output"]["sha256"].as_str().unwrap()
    )
}
