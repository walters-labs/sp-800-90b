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

#[test]
#[ignore = "requires NIST_REFERENCE_DIR with built cpp/ea_conditioning"]
fn typed_components_match_reference_dimensions() {
    use ConditioningAesKey::*;
    use ConditioningHash::*;
    let binary =
        PathBuf::from(std::env::var_os("NIST_REFERENCE_DIR").unwrap()).join("cpp/ea_conditioning");
    // Every distinct Table 1 width, all six component families, and the
    // AES-256 MAC/block-cipher-DF distinction.
    for (component, width) in [
        (VettedConditioner::Hmac(Sha1), 160),
        (VettedConditioner::Hash(Sha224), 224),
        (VettedConditioner::HashDf(Sha256), 256),
        (VettedConditioner::Hash(Sha3_384), 384),
        (VettedConditioner::Hmac(Sha512), 512),
        (VettedConditioner::Cmac(Bits256), 128),
        (VettedConditioner::CbcMac(Bits192), 128),
        (VettedConditioner::BlockCipherDf(Bits192), 192),
        (VettedConditioner::BlockCipherDf(Bits256), 256),
    ] {
        let output = Command::new(&binary)
            .args(["-v", "1024", &width.to_string(), &width.to_string(), "768"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        let expected: f64 = text
            .lines()
            .find_map(|line| line.strip_prefix("(Vetted) h_out = "))
            .unwrap()
            .parse()
            .unwrap();
        let report = component
            .assess(1024, &Float::with_val(256, 768), Default::default())
            .unwrap();
        assert_eq!(report.output_bits, width);
        assert_eq!(report.vetted_component, Some(component));
        assert!(
            (report.min_entropy.to_f64() - expected).abs() < 1e-10,
            "{component:?}"
        );
    }
}
