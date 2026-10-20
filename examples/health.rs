//! Offline demonstration of streaming raw byte-valued source samples.
use rug::Float;
use sp_800_90b::*;
use std::io::Read;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: health <raw-sample-file> <entropy-per-sample> <alphabet-size>".into());
    }
    let h = Float::with_val(256, Float::parse(&args[1])?);
    let mut tests = HealthTests::new(
        &h,
        HealthTestOptions {
            alphabet_size: args[2].parse()?,
            ..Default::default()
        },
    )?;
    let mut file = std::fs::File::open(&args[0])?;
    let mut buffer = [0u8; 4096];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        tests.observe_slice(&buffer[..count])?;
    }
    println!(
        "RCT cutoff: {}; APT cutoff: {}; window: {}",
        tests.cutoffs().repetition_count,
        tests.cutoffs().adaptive_proportion,
        tests.cutoffs().window_size
    );
    println!("{:?}", tests.status());
    println!("Startup ready: {}", tests.status().startup_ready());
    Ok(())
}
