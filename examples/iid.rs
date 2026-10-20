//! File-based IID assessment. One symbol per byte; optional declared bit width.
use sp_800_90b::{IidOptions, assess_iid};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: iid <sample-file> [bits-per-symbol]")?;
    let bits_per_symbol = args.next().map(|s| s.parse::<u8>()).transpose()?;
    if args.next().is_some() {
        return Err("usage: iid <sample-file> [bits-per-symbol]".into());
    }
    let report = assess_iid(
        &std::fs::read(path)?,
        IidOptions {
            bits_per_symbol,
            ..Default::default()
        },
    )?;
    println!(
        "Decision: {:?}; checks complete: {}",
        report.decision,
        report.is_complete()
    );
    println!(
        "Candidate entropy: {} bits/sample",
        report.candidate_min_entropy
    );
    match &report.min_entropy {
        Some(h) => println!("IID estimate: {h} bits/sample"),
        None => println!("IID estimate unavailable: inspect rejected or incomplete checks"),
    }
    for warning in &report.warnings {
        eprintln!("Warning: {warning:?}");
    }
    if let Some(checks) = report.checks {
        println!(
            "Independence: {:?}",
            checks.independence.as_ref().map(|r| r.reject_independence)
        );
        println!(
            "Goodness of fit: {:?}",
            checks
                .goodness_of_fit
                .as_ref()
                .map(|r| r.reject_goodness_of_fit)
        );
        println!(
            "LRS: {:?}",
            checks.longest_repeated_substring.as_ref().map(|r| r.reject)
        );
        match checks.permutation {
            Ok(r) => {
                println!(
                    "Permutation seed: {:?}; shuffles: {}",
                    r.seed, r.permutations_performed
                );
                for t in r.tests {
                    println!(
                        "{}: reject={}, greater={}, equal={}, less={}",
                        t.name, t.reject, t.greater, t.equal, t.less
                    );
                }
            }
            Err(e) => println!("Permutation tests unavailable: {e}"),
        }
    }
    Ok(())
}
