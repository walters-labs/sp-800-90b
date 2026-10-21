//! Diagnostic stage timings, not a statistically repeated benchmark.
//! cargo run --release --example profile_non_iid -- samples.bin 8
use sp_800_90b::*;
use std::{error::Error, time::Instant};

fn dataset(label: &str, samples: &[u8], binary: bool) {
    macro_rules! time {
        ($name:literal, $call:expr) => {{
            let start = Instant::now();
            let result = $call;
            let seconds = start.elapsed().as_secs_f64();
            let outcome = match result {
                Ok(value) => value.min_entropy.to_string(),
                Err(error) => format!("unavailable: {error}"),
            };
            println!(
                "{label}\t{}\t{}\t{seconds:.6}\t{outcome}",
                $name,
                samples.len()
            );
        }};
    }
    time!("mcv", most_common_value(samples));
    if binary {
        time!("collision", collision_estimate(samples));
        time!("markov", markov_estimate(samples));
        time!("compression", compression_estimate(samples));
    }
    time!("t_tuple", t_tuple_estimate(samples));
    time!("lrs", lrs_estimate(samples));
    time!("multi_mcw", multi_mcw_estimate(samples));
    time!("lag", lag_estimate(samples));
    time!("multi_mmc", multi_mmc_estimate(samples));
    time!("lz78y", lz78y_estimate(samples));
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: profile_non_iid <samples.bin> <bits-per-symbol>".into());
    }
    let width: u8 = args[1].to_str().ok_or("invalid width")?.parse()?;
    let samples = std::fs::read(&args[0])?;
    if !(1..=8).contains(&width)
        || samples.len() < 2
        || samples.iter().any(|&s| u16::from(s) >= 1u16 << width)
    {
        return Err("require at least two samples fitting a width of 1..=8".into());
    }
    let mut seen = [false; 256];
    for &s in &samples {
        seen[usize::from(s)] = true;
    }
    let mut mapping = [0u8; 256];
    let mut alphabet = 0;
    for (s, present) in seen.into_iter().enumerate() {
        if present {
            mapping[s] = alphabet as u8;
            alphabet += 1;
        }
    }
    if alphabet < 2 {
        return Err("constant input is short-circuited by the assessment API".into());
    }
    println!("branch\testimator\tsamples\tseconds\tmin_entropy_256_bits");
    let literal: Vec<_> = samples.iter().map(|&s| mapping[usize::from(s)]).collect();
    dataset("literal", &literal, alphabet == 2);
    if alphabet > 2 {
        let bits: Vec<_> = samples
            .iter()
            .flat_map(|&s| (0..width).rev().map(move |bit| (s >> bit) & 1))
            .collect();
        dataset("bitstring", &bits, true);
    }
    // Time the actual coordinator as well: it can share work between estimators.
    // This excludes file I/O and JSON serialization, and includes preprocessing.
    let start = Instant::now();
    let report = assess_non_iid(
        &samples,
        NonIidOptions {
            bits_per_symbol: Some(width),
            ..Default::default()
        },
    )?;
    let seconds = start.elapsed().as_secs_f64();
    println!(
        "combined\tassessment\t{}\t{seconds:.6}\t{}",
        samples.len(),
        report.min_entropy
    );
    if !report.is_complete() {
        return Err("combined assessment is incomplete".into());
    }
    Ok(())
}
