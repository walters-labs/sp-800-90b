//! Row-major restart file: exactly 1000 restarts, each containing 1000 samples.
use rug::Float;
use sp_800_90b::{RestartMode, RestartOptions, assess_restart};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: restart <sample-file> <initial-entropy> [iid|non-iid] [bits-per-symbol]";
    let path = args.next().ok_or(usage)?;
    let entropy = Float::with_val(256, Float::parse(args.next().ok_or(usage)?)?);
    let mode = match args.next().as_deref() {
        None | Some("non-iid") => RestartMode::NonIid,
        Some("iid") => RestartMode::Iid,
        _ => return Err(usage.into()),
    };
    let bits_per_symbol = args.next().map(|s| s.parse::<u8>()).transpose()?;
    if args.next().is_some() {
        return Err(usage.into());
    }
    let r = assess_restart(
        &std::fs::read(path)?,
        &entropy,
        RestartOptions {
            mode,
            bits_per_symbol,
            ..Default::default()
        },
    )?;
    println!("Decision: {:?}", r.decision);
    println!(
        "Sanity cutoff: {}; row max: {}; column max: {}",
        r.sanity.cutoff, r.sanity.maximum_row_count, r.sanity.maximum_column_count
    );
    println!("Simulation seed: {:?}", r.sanity.seed);
    for (label, dataset) in [("Rows", &r.rows), ("Columns", &r.columns)] {
        if let Some(dataset) = dataset {
            println!(
                "{label}: {} bits/sample; complete: {}",
                dataset.min_entropy(),
                dataset.is_complete()
            );
        }
    }
    match r.min_entropy {
        Some(h) => println!("Validated entropy: {h} bits/sample"),
        None => println!("No validated entropy result"),
    }
    Ok(())
}
