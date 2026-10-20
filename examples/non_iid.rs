//! Minimal file-based example; the library API also supports conditioned mode
//! and bitstring truncation. This is not an IID/restart/source-validation tool.
use sp_800_90b::{EstimatorOutcome, NonIidOptions, assess_non_iid};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("usage: non_iid <sample-file> [bits-per-symbol]")?;
    let bits_per_symbol = args
        .next()
        .map(|s| {
            s.to_str()
                .ok_or("bits-per-symbol must be UTF-8")?
                .parse::<u8>()
                .map_err(|_| "bits-per-symbol must be an integer in 1..=8")
        })
        .transpose()?;
    if args.next().is_some() {
        return Err("usage: non_iid <sample-file> [bits-per-symbol]".into());
    }
    let samples = std::fs::read(path)?;
    let report = assess_non_iid(
        &samples,
        NonIidOptions {
            bits_per_symbol,
            ..Default::default()
        },
    )?;
    println!(
        "{} samples; {} distinct symbols; {} bits per symbol",
        report.sample_count,
        report.alphabet.len(),
        report.bits_per_symbol
    );
    for warning in &report.warnings {
        eprintln!("Warning: {warning:?}");
    }
    for (name, dataset) in [
        ("Literal", report.literal.as_ref()),
        ("Bitstring", report.bitstring.as_ref()),
    ] {
        if let Some(dataset) = dataset {
            for result in &dataset.estimators {
                match &result.outcome {
                    EstimatorOutcome::Available(details) => {
                        println!("{name} {:?}: {}", result.estimator, details.min_entropy())
                    }
                    EstimatorOutcome::NotApplicable => {
                        println!("{name} {:?}: not applicable", result.estimator)
                    }
                    EstimatorOutcome::Unavailable(error) => {
                        println!("{name} {:?}: unavailable ({error})", result.estimator)
                    }
                }
            }
            println!("{name} minimum: {}", dataset.min_entropy);
        }
    }
    println!(
        "All applicable estimators completed: {}",
        report.is_complete()
    );
    println!(
        "{} entropy: {} bits per input symbol",
        if report.is_complete() {
            "Assessed"
        } else {
            "Partial"
        },
        report.min_entropy
    );
    Ok(())
}
