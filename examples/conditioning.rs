use rug::Float;
use sp_800_90b::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(4..=5).contains(&args.len()) {
        return Err("usage: conditioning <input-bits> <output-bits> <internal-width> <input-entropy> [non-vetted-h-prime]".into());
    }
    let f = |s: &str| Float::parse(s).map(|x| Float::with_val(256, x));
    let kind = if args.len() == 5 {
        ConditioningKind::NonVetted {
            assessed_entropy_per_bit: f(&args[4])?,
        }
    } else {
        ConditioningKind::Vetted
    };
    let r = conditioning_entropy(
        args[0].parse()?,
        args[1].parse()?,
        args[2].parse()?,
        &f(&args[3])?,
        kind,
        ConditioningOptions::default(),
    )?;
    println!("Output_Entropy: {} bits/block", r.output_entropy);
    println!(
        "Final bound: {} bits/block (working precision: {})",
        r.min_entropy, r.working_precision
    );
    Ok(())
}
