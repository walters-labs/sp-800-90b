use rug::Float;
use sp_800_90b::*;
use std::{path::PathBuf, process::Command};
#[test]
#[ignore = "requires NIST_REFERENCE_DIR with built cpp/ea_conditioning"]
fn output_entropy_matches_reference() {
    let binary =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").unwrap()).join("cpp/ea_conditioning");
    for (ni, no, nw, h, hp) in [
        (2, 1, 1, "1", None),
        (128, 64, 128, "64", None),
        (512, 256, 256, "400", None),
        (1024, 256, 512, "768", None),
        (64, 128, 256, "32", None),
        (512, 128, 256, "400", Some("0.5")),
        (512, 128, 256, "400", Some("1")),
        (256, 128, 256, "32.125", Some("0.75")),
    ] {
        let mut command = Command::new(&binary);
        command.args([
            if hp.is_some() { "-n" } else { "-v" },
            &ni.to_string(),
            &no.to_string(),
            &nw.to_string(),
            h,
        ]);
        if let Some(hp) = hp {
            command.arg(hp);
        }
        let out = command.output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8(out.stdout).unwrap();
        let label = if hp.is_some() {
            "(Non-vetted) h_out = "
        } else {
            "(Vetted) h_out = "
        };
        let expected: f64 = text
            .lines()
            .find_map(|l| l.strip_prefix(label))
            .unwrap()
            .parse()
            .unwrap();
        let f = |x: &str| Float::with_val(256, Float::parse(x).unwrap());
        let kind = hp.map_or(ConditioningKind::Vetted, |x| ConditioningKind::NonVetted {
            assessed_entropy_per_bit: f(x),
        });
        let r =
            conditioning_entropy(ni, no, nw, &f(h), kind, ConditioningOptions::default()).unwrap();
        assert!(
            (r.min_entropy.to_f64() - expected).abs() < 1e-10,
            "{ni}/{no}/{nw}/{h}: {} vs {expected}",
            r.min_entropy
        );
    }
}
