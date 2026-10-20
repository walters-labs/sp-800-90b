//! One byte per symbol; this example does not perform IID or source validation.
use sp_800_90b::most_common_value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: mcv <sample-file>")?;
    let samples = std::fs::read(path)?;
    let result = most_common_value(&samples)?;
    println!("{}", result.min_entropy);
    Ok(())
}
