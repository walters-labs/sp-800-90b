use serde_json::{Value, json};
mod conditioning;
mod transpose;
mod validate;
use sha2::{Digest, Sha256};
use sp_800_90b::*;
use std::{
    collections::HashSet,
    ffi::OsString,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

const HELP: &str = "Usage: sp-800-90b <non-iid|iid|restart> [OPTIONS] <FILE>
       sp-800-90b conditioning [OPTIONS] (no file)
       sp-800-90b validate --sequential FILE --restart FILE
         --bits-per-symbol N --submitter-entropy H [OPTIONS]
       sp-800-90b transpose --input FILE --output NEW_FILE [--json]
         [--offset N] [--samples 1000000]

Transpose:
  Select exactly 1000000 bytes (1000 by 1000); write column-major order.
  Output must not already exist. Offsets count bytes; default is the whole file.

Validation:
  --iid-claim                    Attempt IID; rejection selects non-IID
  --conditioning vetted|non-vetted  Optional conditioning calculation
  --samples-per-block N          Primary raw samples per conditioning block
  --output-bits N --internal-width N  Conditioning dimensions
  --conditioned-output FILE      Non-vetted only: one byte per output bit
  --additional-input-bits N --retained-output-bits N  Vetted only
  --precision N --max-working-precision N --confidence reference|publication
  --simulation-seed A,B,C,D --simulations N --seed A,B,C,D
  --progress                    Write stage timing to stderr
  --json                        Emit the combined report

Conditioning:
  --mode vetted|non-vetted       Required; vetted is a caller assertion
  --input-bits N                 Primary input bits per block
  --input-entropy H              Primary entropy in bits per input block
  --output-bits N                Full output bits per block
  --internal-width N             Narrowest internal width in bits
  --assessed-entropy-per-bit H   Required only for non-vetted conditioning
  --additional-input-bits N      Vetted only; no additional entropy credit
  --retained-output-bits N       Vetted only; retained bits after truncation
  --max-working-precision N      MPFR resource ceiling; default: 1048576
  --precision N, --json          As below

Input: one unsigned symbol per byte; values must fit the declared width.
  --bits-per-symbol N     Width 1..8; otherwise inferred
  --mode initial|conditioned  Assessment mode (default: initial)
  --mode non-iid|iid       Restart mode (default: non-iid)
  --initial-entropy H     Restart only: required bits per sample
  --simulation-seed A,B,C,D  Restart sanity seed
  --simulations N         Restart rounds, minimum/default 5000000
  --offset N              Skip N samples (bytes); default: 0
  --samples N             Read at most N samples; default: through EOF
  --bitstring-limit N     Assess at most N expanded bits; default: all
  --precision N           MPFR precision in bits, minimum 64; default: 256
  --confidence reference|publication  Default: reference
  --seed A,B,C,D          IID/permutation: four decimal u64 words, not all zero
  --json                  Emit schema-versioned JSON
  --                      End options (for filenames starting with '-')
  -h, --help              Show help
  --version               Show crate version

Exit codes: 0 complete/non-rejected, 1 rejected, 2 error, 3 incomplete.
Completion is not entropy-source validation; small datasets carry warnings.
Subset selection precedes width inference and bit expansion. Offsets are absolute
sample offsets, not NIST's -l block indices. This CLI does not emulate NIST flags.";

struct Config {
    iid: bool,
    restart: bool,
    restart_mode: RestartMode,
    initial_entropy: Option<String>,
    simulation_seed: Option<[u64; 4]>,
    simulations: usize,
    seed: Option<[u64; 4]>,
    path: PathBuf,
    options: NonIidOptions,
    offset: u64,
    samples: Option<u64>,
}
struct CliError {
    code: &'static str,
    message: String,
}
fn error(code: &'static str, message: impl ToString) -> CliError {
    CliError {
        code,
        message: message.to_string(),
    }
}
fn parse(args: &[OsString]) -> Result<Config, CliError> {
    if args
        .first()
        .is_none_or(|s| s != "non-iid" && s != "iid" && s != "restart")
    {
        return Err(error(
            "usage",
            "expected subcommand non-iid, iid, restart, conditioning, validate, or transpose; use --help",
        ));
    }
    let mut config = Config {
        iid: args[0] == "iid",
        restart: args[0] == "restart",
        restart_mode: RestartMode::NonIid,
        initial_entropy: None,
        simulation_seed: None,
        simulations: RESTART_SIMULATIONS,
        seed: None,
        path: PathBuf::new(),
        options: NonIidOptions::default(),
        offset: 0,
        samples: None,
    };
    let mut path = None;
    let mut seen = HashSet::new();
    let mut positional = false;
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        i += 1;
        if !positional && arg == "--" {
            positional = true;
            continue;
        }
        if !positional && arg.to_string_lossy().starts_with('-') {
            let flag = arg
                .to_str()
                .ok_or_else(|| error("usage", "options must be UTF-8"))?;
            if !seen.insert(flag) {
                return Err(error("usage", format!("duplicate option {flag}")));
            }
            if flag == "--json" {
                continue;
            }
            if ![
                "--bits-per-symbol",
                "--mode",
                "--offset",
                "--samples",
                "--bitstring-limit",
                "--precision",
                "--confidence",
                "--seed",
                "--initial-entropy",
                "--simulation-seed",
                "--simulations",
            ]
            .contains(&flag)
            {
                return Err(error("usage", format!("unknown option {flag}")));
            }
            let value = args
                .get(i)
                .and_then(|s| s.to_str())
                .ok_or_else(|| error("usage", format!("missing value for {flag}")))?;
            i += 1;
            let number = || {
                value
                    .parse::<u64>()
                    .map_err(|_| error("usage", format!("{flag} requires an unsigned integer")))
            };
            match flag {
                "--bits-per-symbol" => {
                    let n = number()?;
                    if !(1..=8).contains(&n) {
                        return Err(error("usage", "bits per symbol must be 1..8"));
                    }
                    config.options.bits_per_symbol = Some(n as u8);
                }
                "--mode" if config.restart => {
                    config.restart_mode = match value {
                        "non-iid" => RestartMode::NonIid,
                        "iid" => RestartMode::Iid,
                        _ => return Err(error("usage", "restart mode must be non-iid or iid")),
                    };
                }
                "--initial-entropy" | "--simulations" | "--simulation-seed" => {
                    if !config.restart {
                        return Err(error("usage", format!("{flag} requires restart")));
                    }
                    match flag {
                        "--initial-entropy" => config.initial_entropy = Some(value.to_owned()),
                        "--simulation-seed" => config.simulation_seed = Some(parse_seed(value)?),
                        _ => {
                            config.simulations = usize::try_from(number()?)
                                .map_err(|_| error("usage", "simulation count too large"))?;
                            if config.simulations < RESTART_SIMULATIONS {
                                return Err(error(
                                    "usage",
                                    "at least 5000000 simulations required",
                                ));
                            }
                        }
                    }
                }
                "--mode" => {
                    config.options.mode = match value {
                        "initial" => NonIidMode::Initial,
                        "conditioned" => NonIidMode::Conditioned,
                        _ => return Err(error("usage", "mode must be initial or conditioned")),
                    }
                }
                "--offset" => config.offset = number()?,
                "--samples" => {
                    let n = number()?;
                    if n < 2 {
                        return Err(error("usage", "samples must be at least 2"));
                    }
                    config.samples = Some(n);
                }
                "--bitstring-limit" => {
                    let n = usize::try_from(number()?)
                        .map_err(|_| error("usage", "bitstring limit is too large"))?;
                    if n < 2 {
                        return Err(error("usage", "bitstring limit must be at least 2"));
                    }
                    config.options.bitstring_limit = Some(n);
                }
                "--precision" => {
                    let n = number()?;
                    if !(64..=u64::from(rug::float::prec_max())).contains(&n) {
                        return Err(error(
                            "usage",
                            "precision is outside the supported MPFR range",
                        ));
                    }
                    config.options.precision_bits = n as u32;
                }
                "--seed" => {
                    if !config.iid && !config.restart {
                        return Err(error("usage", "--seed requires iid or restart IID mode"));
                    }
                    config.seed = Some(parse_seed(value)?);
                }
                "--confidence" => {
                    config.options.confidence_constant = match value {
                        "reference" => ConfidenceConstant::NistReference,
                        "publication" => ConfidenceConstant::Publication,
                        _ => {
                            return Err(error(
                                "usage",
                                "confidence must be reference or publication",
                            ));
                        }
                    }
                }
                _ => unreachable!(),
            }
        } else if path.replace(PathBuf::from(arg)).is_some() {
            return Err(error("usage", "expected exactly one input file"));
        }
    }
    if config.restart {
        if config.initial_entropy.is_none() {
            return Err(error("usage", "restart requires --initial-entropy"));
        }
        if config.options.bitstring_limit.is_some() {
            return Err(error("usage", "restart does not use --bitstring-limit"));
        }
        if config.seed.is_some() && config.restart_mode != RestartMode::Iid {
            return Err(error("usage", "--seed requires restart --mode iid"));
        }
    }
    config.path = path.ok_or_else(|| error("usage", "missing input file"))?;
    Ok(config)
}
fn name(kind: EstimatorKind) -> &'static str {
    match kind {
        EstimatorKind::MostCommonValue => "most_common_value",
        EstimatorKind::Collision => "collision",
        EstimatorKind::Markov => "markov",
        EstimatorKind::Compression => "compression",
        EstimatorKind::TTuple => "t_tuple",
        EstimatorKind::Lrs => "lrs",
        EstimatorKind::MultiMcw => "multi_mcw",
        EstimatorKind::Lag => "lag",
        EstimatorKind::MultiMmc => "multi_mmc",
        EstimatorKind::Lz78y => "lz78y",
    }
}
fn dataset(data: Option<&NonIidDatasetAssessment>, units: &str) -> Value {
    let Some(data) = data else {
        return Value::Null;
    };
    let estimators: Vec<Value> = data.estimators.iter().map(|r| match &r.outcome {
        EstimatorOutcome::Available(d) => json!({"estimator":name(r.estimator),"status":"available","min_entropy":d.min_entropy().to_string(),"error":null}),
        EstimatorOutcome::NotApplicable => json!({"estimator":name(r.estimator),"status":"not_applicable","min_entropy":null,"error":null}),
        EstimatorOutcome::Unavailable(e) => json!({"estimator":name(r.estimator),"status":"unavailable","min_entropy":null,"error":{"code":"estimator_unavailable","message":e.to_string()}}),
    }).collect();
    json!({"sample_count":data.sample_count,"units":units,"complete":data.is_complete(),
        "min_entropy":data.is_complete().then(||data.min_entropy.to_string()),
        "minimum_available_entropy":data.min_entropy.to_string(),"estimators":estimators})
}
fn warning(w: &AssessmentWarning) -> Value {
    match w {
        AssessmentWarning::FewerThanRecommendedSamples { count } => {
            json!({"code":"fewer_than_recommended_samples","count":count,"recommended":1_000_000})
        }
        AssessmentWarning::DeclaredWidthExceedsObserved { declared, observed } => {
            json!({"code":"declared_width_exceeds_observed","declared":declared,"observed":observed})
        }
        AssessmentWarning::BitstringTruncated { available, used } => {
            json!({"code":"bitstring_truncated","available":available,"used":used})
        }
        AssessmentWarning::ConstantInput => json!({"code":"constant_input"}),
    }
}
fn assess(config: &Config) -> Result<(Value, u8), CliError> {
    let mut file = File::open(&config.path).map_err(|e| error("input_io", e))?;
    if !file.metadata().map_err(|e| error("input_io", e))?.is_file() {
        return Err(error("input_io", "input must be a regular file"));
    }
    file.seek(SeekFrom::Start(config.offset))
        .map_err(|e| error("input_io", e))?;
    let mut samples = Vec::new();
    file.take(config.samples.unwrap_or(u64::MAX))
        .read_to_end(&mut samples)
        .map_err(|e| error("input_io", e))?;
    if config.restart {
        return assess_restart_report(config, &samples);
    }
    if config.iid {
        return assess_iid_report(config, &samples);
    }
    let report = assess_non_iid(&samples, config.options).map_err(|e| error("invalid_input", e))?;
    let complete = report.is_complete();
    let code = if complete { 0 } else { 3 };
    Ok((
        json!({
            "schema_version":1,"tool":{"name":"sp-800-90b","version":env!("CARGO_PKG_VERSION")},
            "command":"non-iid","status":if complete {"complete"} else {"incomplete"},
            "input":{"path":config.path.to_string_lossy(),"encoding":"one_symbol_per_byte",
                "offset_samples":config.offset,"requested_samples":config.samples,"sample_count":report.sample_count,
                "selected_samples_sha256":format!("{:x}",Sha256::digest(&samples))},
            "options":{"mode":if matches!(config.options.mode,NonIidMode::Initial){"initial"}else{"conditioned"},
                "declared_bits_per_symbol":config.options.bits_per_symbol,"precision_bits":config.options.precision_bits,
                "confidence_constant":if matches!(config.options.confidence_constant,ConfidenceConstant::NistReference){"reference"}else{"publication"},
                "bitstring_limit":config.options.bitstring_limit},
            "bits_per_symbol":report.bits_per_symbol,"alphabet":report.alphabet,"entropy_units":"bits_per_input_symbol",
            "min_entropy":complete.then(||report.min_entropy.to_string()),
            "minimum_available_entropy":report.min_entropy.to_string(),
            "warnings":report.warnings.iter().map(warning).collect::<Vec<_>>(),
            "literal":dataset(report.literal.as_ref(),"bits_per_literal_symbol"),
            "bitstring":dataset(report.bitstring.as_ref(),"bits_per_bit"),"error":null
        }),
        code,
    ))
}
fn human(report: &Value) -> String {
    if report["command"] == "transpose" {
        return transpose::human(report);
    }
    if report["command"] == "conditioning" {
        return conditioning::human(report);
    }
    if report["command"] == "validate" {
        return validate::human(report);
    }
    if report["command"] == "restart" {
        return human_restart(report);
    }
    if report["command"] == "iid" {
        return human_iid(report);
    }
    let mut text = format!(
        "Non-IID assessment: {}\n{} samples; {} bits per symbol\n",
        report["status"].as_str().unwrap(),
        report["input"]["sample_count"],
        report["bits_per_symbol"]
    );
    for branch in ["literal", "bitstring"] {
        if let Some(rows) = report[branch]["estimators"].as_array() {
            for row in rows {
                text.push_str(&format!(
                    "{branch} {}: {}\n",
                    row["estimator"].as_str().unwrap(),
                    row["min_entropy"]
                        .as_str()
                        .unwrap_or(row["status"].as_str().unwrap())
                ));
                if let Some(message) = row["error"]["message"].as_str() {
                    text.push_str(&format!("  {message}\n"));
                }
            }
        }
    }
    let label = if report["status"] == "complete" {
        "Assessed entropy"
    } else {
        "Minimum available estimate (incomplete)"
    };
    text.push_str(&format!(
        "{label}: {} bits per input symbol\n",
        report["minimum_available_entropy"].as_str().unwrap()
    ));
    for w in report["warnings"].as_array().unwrap() {
        text.push_str(&format!("Warning: {w}\n"));
    }
    text
}
fn emit(text: &str) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    out.write_all(text.as_bytes())?;
    out.flush()
}
pub fn run(args: Vec<OsString>) -> u8 {
    let flags: Vec<_> = args.iter().take_while(|a| *a != "--").collect();
    if flags.iter().any(|a| *a == "--help" || *a == "-h") || args.is_empty() {
        return if emit(&format!("{HELP}\n")).is_ok() {
            0
        } else {
            2
        };
    }
    if args.len() == 1 && args[0] == "--version" {
        return if emit(concat!("sp-800-90b ", env!("CARGO_PKG_VERSION"), "\n")).is_ok() {
            0
        } else {
            2
        };
    }
    let json_output = flags.iter().any(|a| *a == "--json");
    let result = if args.first().is_some_and(|a| a == "conditioning") {
        conditioning::assess(&args[1..])
    } else if args.first().is_some_and(|a| a == "validate") {
        validate::assess(&args[1..])
    } else if args.first().is_some_and(|a| a == "transpose") {
        transpose::run(&args[1..])
    } else {
        parse(&args).and_then(|c| assess(&c))
    };
    match result {
        Ok((report, code)) => {
            let text = if json_output {
                format!("{}\n", serde_json::to_string_pretty(&report).unwrap())
            } else {
                human(&report)
            };
            if let Err(e) = emit(&text) {
                eprintln!("output error: {e}");
                2
            } else {
                code
            }
        }
        Err(e) => {
            if json_output {
                let report = json!({"schema_version":1,"tool":{"name":"sp-800-90b","version":env!("CARGO_PKG_VERSION")},"command":args.first().and_then(|s|s.to_str()),"status":"error","min_entropy":null,"error":{"code":e.code,"message":e.message}});
                if emit(&format!("{report}\n")).is_err() {
                    eprintln!("unable to write error report");
                }
            } else {
                eprintln!("{}: {}", e.code, e.message);
            }
            2
        }
    }
}

fn check_status(reject: bool) -> &'static str {
    if reject { "rejected" } else { "not_rejected" }
}
fn check_result<T, E: std::fmt::Display>(
    result: &Result<T, E>,
    render: impl FnOnce(&T) -> Value,
) -> Value {
    match result {
        Ok(value) => render(value),
        Err(e) => {
            json!({"status":"unavailable","error":{"code":"check_unavailable","message":e.to_string()}})
        }
    }
}
fn mcv_summary(value: Option<&McvEstimate>, units: &str) -> Value {
    value.map_or(Value::Null, |r|json!({"sample_count":r.sample_count,"units":units,
        "mode_count":r.mode_count,"p_hat":r.p_hat.to_string(),"p_upper":r.p_upper.to_string(),"min_entropy":r.min_entropy.to_string()}))
}
fn assess_iid_report(config: &Config, samples: &[u8]) -> Result<(Value, u8), CliError> {
    let options = IidOptions {
        precision_bits: config.options.precision_bits,
        confidence_constant: config.options.confidence_constant,
        bits_per_symbol: config.options.bits_per_symbol,
        mode: config.options.mode,
        bitstring_limit: config.options.bitstring_limit,
        permutation: PermutationOptions {
            seed: config.seed,
            ..Default::default()
        },
        ..Default::default()
    };
    let r = assess_iid(samples, options).map_err(|e| error("invalid_input", e))?;
    let (status, exit) = match r.decision {
        IidDecision::NotRejected => ("not_rejected", 0),
        IidDecision::Rejected => ("rejected", 1),
        IidDecision::Incomplete => ("incomplete", 3),
    };
    let checks = r.checks.as_ref().map_or(Value::Null, iid_checks_json);
    Ok((
        json!({"schema_version":1,"tool":{"name":"sp-800-90b","version":env!("CARGO_PKG_VERSION")},"command":"iid","status":status,
        "checks_complete":r.is_complete(),"checks":checks,
        "input":{"path":config.path.to_string_lossy(),"encoding":"one_symbol_per_byte","offset_samples":config.offset,"requested_samples":config.samples,"sample_count":r.sample_count,"selected_samples_sha256":format!("{:x}",Sha256::digest(samples))},
        "options":{"mode":if matches!(options.mode,IidMode::Initial){"initial"}else{"conditioned"},"declared_bits_per_symbol":options.bits_per_symbol,"precision_bits":options.precision_bits,"confidence_constant":if matches!(options.confidence_constant,ConfidenceConstant::NistReference){"reference"}else{"publication"},"bitstring_limit":options.bitstring_limit,
            "requested_seed":config.seed.map(|s|s.map(|w|w.to_string())),"chi_square_grouping":"reference","permutation_arithmetic":"reference","permutation_early_stop":true},
        "bits_per_symbol":r.bits_per_symbol,"alphabet":r.alphabet,"entropy_units":"bits_per_input_symbol",
        "candidate_min_entropy":r.candidate_min_entropy.to_string(),"min_entropy":r.min_entropy.as_ref().map(ToString::to_string),
        "literal":mcv_summary(r.literal.as_ref(),"bits_per_literal_symbol"),"bitstring":mcv_summary(r.bitstring.as_ref(),"bits_per_bit"),
        "warnings":r.warnings.iter().map(warning).collect::<Vec<_>>(),"error":null}),
        exit,
    ))
}
fn human_iid(r: &Value) -> String {
    let mut text = format!(
        "IID assessment: {}\n{} samples; {} bits per symbol\n",
        r["status"].as_str().unwrap(),
        r["input"]["sample_count"],
        r["bits_per_symbol"]
    );
    if let Some(checks) = r["checks"].as_object() {
        for (name, check) in checks {
            text.push_str(&format!("{name}: {}\n", check["status"].as_str().unwrap()));
            if let Some(p) = check["p_value"].as_str() {
                text.push_str(&format!("  p-value: {p}\n"));
            }
            if let Some(e) = check["error"]["message"].as_str() {
                text.push_str(&format!("  {e}\n"));
            }
        }
        if let Some(seed) = r["checks"]["permutation"]["seed"].as_array() {
            text.push_str(&format!(
                "Permutation seed: {}\n",
                seed.iter()
                    .map(|w| w.as_str().unwrap())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
    } else {
        text.push_str("IID checks skipped for constant input.\n");
    }
    text.push_str(&format!(
        "Candidate entropy: {} bits per input symbol\n",
        r["candidate_min_entropy"].as_str().unwrap()
    ));
    if let Some(h) = r["min_entropy"].as_str() {
        text.push_str(&format!("Assessed entropy: {h} bits per input symbol\n"));
    } else {
        text.push_str("No eligible entropy result.\n");
    }
    for w in r["warnings"].as_array().unwrap() {
        text.push_str(&format!("Warning: {w}\n"));
    }
    text
}

fn iid_checks_json(c: &IidChecks) -> Value {
    json!({
        "independence":check_result(&c.independence,|v|json!({"status":check_status(v.reject_independence),"statistic":v.statistic.to_string(),"p_value":v.p_value.to_string(),"degrees_of_freedom":v.degrees_of_freedom,"error":null})),
        "goodness_of_fit":check_result(&c.goodness_of_fit,|v|json!({"status":check_status(v.reject_goodness_of_fit),"statistic":v.statistic.to_string(),"p_value":v.p_value.to_string(),"degrees_of_freedom":v.degrees_of_freedom,"error":null})),
        "longest_repeated_substring":check_result(&c.longest_repeated_substring,|v|json!({"status":check_status(v.reject),"longest_repeat":v.longest_repeat,"substring_pairs":v.substring_pairs.to_string(),"collision_probability":v.collision_probability.to_string(),"p_value":v.p_value.to_string(),"error":null})),
        "permutation":check_result(&c.permutation,|v|json!({"status":check_status(v.reject),"seed":v.seed.map(|word|word.to_string()),"permutations_performed":v.permutations_performed,
            "statistics":v.tests.iter().map(|t|json!({"name":t.name,"original":t.original.to_string(),"greater":t.greater,"equal":t.equal,"less":t.less,"status":check_status(t.reject)})).collect::<Vec<_>>(),"error":null}))
    })
}

fn parse_seed(value: &str) -> Result<[u64; 4], CliError> {
    let words: Vec<u64> = value
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| error("usage", "seed requires four decimal u64 words"))?;
    let seed: [u64; 4] = words
        .try_into()
        .map_err(|_| error("usage", "seed requires exactly four words"))?;
    if seed == [0; 4] {
        return Err(error("usage", "seed must not be all zero"));
    }
    Ok(seed)
}
fn restart_dataset(r: Option<&RestartDatasetAssessment>) -> Value {
    match r {
        None => Value::Null,
        Some(RestartDatasetAssessment::NonIid(d)) => {
            let mut v = dataset(Some(d), "bits_per_input_symbol");
            v["mode"] = json!("non-iid");
            v
        }
        Some(RestartDatasetAssessment::Iid { estimate, checks }) => {
            let c = iid_checks_json(checks);
            let accepted = checks.is_complete()
                && c.as_object()
                    .unwrap()
                    .values()
                    .all(|v| v["status"] == "not_rejected");
            json!({"mode":"iid","complete":checks.is_complete(),"candidate_min_entropy":estimate.min_entropy.to_string(),
                "min_entropy":accepted.then(||estimate.min_entropy.to_string()),"estimate":mcv_summary(Some(estimate),"bits_per_input_symbol"),"checks":c})
        }
    }
}
fn restart_status(decision: RestartDecision) -> (&'static str, u8) {
    match decision {
        RestartDecision::Validated => ("validated", 0),
        RestartDecision::SanityRejected => ("sanity_rejected", 1),
        RestartDecision::EntropyRejected => ("entropy_rejected", 1),
        RestartDecision::IidRejected => ("iid_rejected", 1),
        RestartDecision::Incomplete => ("incomplete", 3),
    }
}
fn assess_restart_report(config: &Config, samples: &[u8]) -> Result<(Value, u8), CliError> {
    let text = config.initial_entropy.as_ref().unwrap();
    let parsed = rug::Float::parse(text)
        .map_err(|e| error("usage", format!("invalid initial entropy: {e}")))?;
    let h = rug::Float::with_val_round(
        config.options.precision_bits,
        parsed,
        rug::float::Round::Down,
    )
    .0;
    if !h.is_finite() || h < 0 {
        return Err(error(
            "invalid_input",
            "initial entropy must be finite and nonnegative",
        ));
    }
    let options = RestartOptions {
        mode: config.restart_mode,
        bits_per_symbol: config.options.bits_per_symbol,
        precision_bits: config.options.precision_bits,
        confidence_constant: config.options.confidence_constant,
        simulation_rounds: config.simulations,
        simulation_seed: config.simulation_seed,
        permutation: PermutationOptions {
            seed: config.seed,
            ..Default::default()
        },
        ..Default::default()
    };
    let r = assess_restart(samples, &h, options).map_err(|e| error("invalid_input", e))?;
    let (status, exit) = restart_status(r.decision);
    let sanity = &r.sanity;
    Ok((
        json!({"schema_version":1,"tool":{"name":"sp-800-90b","version":env!("CARGO_PKG_VERSION")},"command":"restart","status":status,
        "input":{"path":config.path.to_string_lossy(),"encoding":"one_symbol_per_byte","offset_samples":config.offset,"requested_samples":config.samples,"sample_count":samples.len(),"selected_samples_sha256":format!("{:x}",Sha256::digest(samples)),"layout":"1000_by_1000_row_major"},
        "options":{"mode":if options.mode==RestartMode::Iid {"iid"}else{"non-iid"},"declared_bits_per_symbol":options.bits_per_symbol,"precision_bits":options.precision_bits,
            "confidence_constant":if options.confidence_constant==ConfidenceConstant::NistReference {"reference"}else{"publication"},
            "simulation_rounds":options.simulation_rounds,"requested_simulation_seed":config.simulation_seed.map(|s|s.map(|w|w.to_string())),"requested_permutation_seed":config.seed.map(|s|s.map(|w|w.to_string())),"chi_square_grouping":"reference","permutation_arithmetic":"reference","permutation_early_stop":true,
            "requested_initial_entropy":text},
        "bits_per_symbol":r.bits_per_symbol,"alphabet":r.alphabet,"entropy_units":"bits_per_input_symbol","initial_entropy":r.initial_entropy.to_string(),
        "min_entropy":r.min_entropy.as_ref().map(ToString::to_string),
        "sanity":{"status":if sanity.reject {"rejected"}else{"not_rejected"},"seed":sanity.seed.map(|w|w.to_string()),"simulation_rounds":sanity.simulation_rounds,
            "arithmetic":"binary64","max_symbol_probability":sanity.max_symbol_probability.to_string(),"alpha":sanity.alpha.to_string(),"effective_alphabet":sanity.effective_alphabet,
            "cutoff":sanity.cutoff,"maximum_row_count":sanity.maximum_row_count,"maximum_column_count":sanity.maximum_column_count,"row_maxima":sanity.row_maxima,"column_maxima":sanity.column_maxima},
        "rows":restart_dataset(r.rows.as_ref()),"columns":restart_dataset(r.columns.as_ref()),"error":null}),
        exit,
    ))
}
fn human_restart(r: &Value) -> String {
    let mut text = format!(
        "Restart assessment: {}\nInitial entropy: {} bits per input symbol\nSanity: {}; cutoff {}; row maximum {}; column maximum {}\n",
        r["status"].as_str().unwrap(),
        r["initial_entropy"].as_str().unwrap(),
        r["sanity"]["status"].as_str().unwrap(),
        r["sanity"]["cutoff"],
        r["sanity"]["maximum_row_count"],
        r["sanity"]["maximum_column_count"]
    );
    text.push_str(&format!(
        "Sanity seed: {}\n",
        r["sanity"]["seed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w.as_str().unwrap())
            .collect::<Vec<_>>()
            .join(",")
    ));
    for branch in ["rows", "columns"] {
        let d = &r[branch];
        if d.is_null() {
            text.push_str(&format!("{branch}: not assessed\n"));
            continue;
        }
        text.push_str(&format!(
            "{branch}: complete={}, eligible entropy={}\n",
            d["complete"],
            d["min_entropy"].as_str().unwrap_or("unavailable")
        ));
        if let Some(candidate) = d["candidate_min_entropy"].as_str() {
            text.push_str(&format!(
                "  Candidate entropy: {candidate} bits per input symbol\n"
            ));
        }
        if let Some(checks) = d["checks"].as_object() {
            for (name, c) in checks {
                text.push_str(&format!("  {name}: {}\n", c["status"].as_str().unwrap()));
                if let Some(message) = c["error"]["message"].as_str() {
                    text.push_str(&format!("    {message}\n"));
                }
                if let Some(seed) = c["seed"].as_array() {
                    text.push_str(&format!(
                        "    Seed: {}\n",
                        seed.iter()
                            .map(|w| w.as_str().unwrap())
                            .collect::<Vec<_>>()
                            .join(",")
                    ));
                }
            }
        }
        if let Some(estimators) = d["estimators"].as_array() {
            for e in estimators {
                text.push_str(&format!(
                    "  {}: {}\n",
                    e["estimator"].as_str().unwrap(),
                    e["min_entropy"]
                        .as_str()
                        .unwrap_or(e["status"].as_str().unwrap())
                ));
            }
        }
    }
    if let Some(h) = r["min_entropy"].as_str() {
        text.push_str(&format!(
            "Validated restart entropy: {h} bits per input symbol\n"
        ));
    } else {
        text.push_str("No validated restart entropy.\n");
    }
    text
}

#[cfg(test)]
mod restart_report_tests {
    use super::*;
    #[test]
    fn restart_decisions_have_distinct_statuses() {
        for (decision, status, exit) in [
            (RestartDecision::Validated, "validated", 0),
            (RestartDecision::SanityRejected, "sanity_rejected", 1),
            (RestartDecision::EntropyRejected, "entropy_rejected", 1),
            (RestartDecision::IidRejected, "iid_rejected", 1),
            (RestartDecision::Incomplete, "incomplete", 3),
        ] {
            assert_eq!(restart_status(decision), (status, exit));
        }
    }
    #[test]
    fn unavailable_row_checks_withhold_eligible_entropy() {
        let report = assess_iid(&[0, 1], IidOptions::default()).unwrap();
        let row = RestartDatasetAssessment::Iid {
            estimate: Box::new(report.literal.unwrap()),
            checks: Box::new(report.checks.unwrap()),
        };
        let value = restart_dataset(Some(&row));
        assert_eq!(value["complete"], false);
        assert!(value["min_entropy"].is_null());
        assert!(value["candidate_min_entropy"].is_string());
        assert_eq!(value["checks"]["permutation"]["status"], "unavailable");
    }
}
