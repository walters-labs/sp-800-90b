# sp-800-90b

[![NIST reference comparison](https://github.com/walters-labs/sp-800-90b/actions/workflows/reference-comparison.yml/badge.svg)](https://github.com/walters-labs/sp-800-90b/actions/workflows/reference-comparison.yml)

Estimate the min-entropy of a noise source with a Rust library and six command-line
tools implementing NIST SP 800-90B numerical assessments. Includes all ten non-IID
estimators, IID tests, restart testing, conditioning bounds, and streaming RCT/APT
health tests. Results retain high-precision values and diagnostic evidence.

**Requires Rust 1.85 or later and a native C build toolchain.** The assessment engine uses
MPFR through `rug`; this is currently a `std` library, not an embedded/`no_std`
package. It provides numerical evidence, not entropy-source certification.

[Install](#install) · [Library example](#use-the-library) · [CLI examples](#use-the-cli) · [Results and units](#interpret-results) · [Verification](#verification-and-limits)

## Install

Until the first crates.io release, add the library from Git:

```sh
cargo add sp-800-90b --git https://github.com/walters-labs/sp-800-90b
```

To build and install the CLI from source:

```sh
git clone https://github.com/walters-labs/sp-800-90b.git
cd sp-800-90b
cargo install --path . --locked
sp-800-90b --help
```

The first build compiles GMP/MPFR and can take several minutes. On macOS, install
the Xcode Command Line Tools. On GNU/Linux, install a C compiler, `make`, `m4`,
and `diffutils` (for example, `sudo apt-get install build-essential m4 diffutils`
on Ubuntu). Use a checkout path without spaces, a native dependency limitation.
**The NIST repository, jsoncpp, and OpenSSL are not required to use this crate.**

The minimum supported Rust version (MSRV) is 1.85.0, tested with the checked-in
`Cargo.lock`. Development checks cover Apple Silicon macOS, and reference CI
targets Ubuntu 24.04 x86-64. Windows and other targets are unverified.

## Choose a workflow

| You want to… | Library API | CLI command |
| --- | --- | --- |
| Estimate entropy without an IID assumption | `assess_non_iid` | `non-iid` |
| Test an IID claim and estimate entropy | `assess_iid` | `iid` |
| Check a restart dataset against initial entropy | `assess_restart` | `restart` |
| Calculate a conditioning output bound | `conditioning_entropy` / `VettedConditioner::assess` | `conditioning` |
| Combine sequential data, restart data, submitter bound and optional conditioning | `assess_validation` | `validate` |
| Transpose a restart matrix | `restart_column_sequence` | `transpose` |
| Monitor incoming symbols with RCT/APT | `HealthTests` | Library only |
| Reduce ranked source symbols wider than a byte | `SymbolReduction` | Library only |

Start with the combined assessment APIs unless you need an individual estimator.
Use `validate` for the strict numerical workflow: it enforces dataset sizes,
coordinates IID-to-non-IID fallback, applies the submitter bound, and withholds
final entropy if any required stage is incomplete or rejected. Standalone `iid`
reports its decision without automatically running non-IID estimation.

## Use the library

Read your own sample file and retain the report, including incomplete results:

```rust
use sp_800_90b::{assess_non_iid, NonIidOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let samples = std::fs::read("samples.bin")?;
    let report = assess_non_iid(&samples, NonIidOptions {
        bits_per_symbol: Some(8),
        ..Default::default()
    })?;
    match report.eligible_entropy() {
        Some(h) => println!("{h} bits per input symbol"),
        None => eprintln!("Incomplete assessment: {report:#?}"),
    }
    for warning in &report.warnings {
        eprintln!("{warning:?}");
    }
    Ok(())
}
```

All public entry points are available at the crate root. Options implement
`Default`. `sp_800_90b::Float` exposes the MPFR type for constructing entropy
inputs without adding a direct `rug` dependency. Keep it as `Float` or a decimal
string to preserve precision; convert to `f64` only when that loss is acceptable.

Use `NonIidDatasetAssessment::estimator(kind)` for individual diagnostics.
IID, permutation and validation APIs provide `_with_progress` variants for
synchronous progress callbacks. Generate API docs with `cargo doc --open --no-deps`;
see the [library guide](docs/library-guide.md) for examples of conditioning,
health monitoring, wider symbols, and each estimator's numerical conventions.

## Use the CLI

`samples.bin`, `sequential.bin`, and `restarts.bin` below are **files you supply**,
not files generated or bundled by the crate. Commands use the installed binary;
from a checkout, substitute `cargo run --release --` for `sp-800-90b`.
Use optimized builds for million-sample assessments.

```sh
# Non-IID estimate, with a versioned JSON report.
sp-800-90b non-iid samples.bin --bits-per-symbol 8 --json > report.json

# IID assessment with a reproducible permutation seed.
sp-800-90b iid samples.bin --bits-per-symbol 8 --seed 1,2,3,4 --json

# Full numerical workflow; replace 2 with your justified submitter bound.
sp-800-90b validate --sequential sequential.bin --restart restarts.bin \
  --bits-per-symbol 8 --submitter-entropy 2 \
  --simulation-seed 1,2,3,4 --progress --json
```

`validate` defaults to the non-IID track. Add `--iid-claim --seed 1,2,3,4` only
when you have a rationale for that claim. Non-vetted conditioning requires a
separate conditioned-output dataset; the workflow assesses it to derive the
output cap. `--progress` writes stage timing to stderr, leaving stdout valid JSON.

A conditioning calculation needs no sample file. For example, a caller-asserted
vetted component with 400 bits of entropy in a 512-bit primary input block:

```sh
sp-800-90b conditioning --mode vetted --input-bits 512 --input-entropy 400 \
  --output-bits 256 --internal-width 256 --json
```

This calculates a bound; it does not execute a cryptographic conditioner.
See the [CLI reference](docs/cli.md) for every command, selection options, seeds,
conditioning flags, JSON fields, and exact statuses. Flags differ from NIST's
CLI; this is not a drop-in command-line replacement.

## Prepare the input correctly

- **One symbol per byte.** A binary sequence is bytes containing `0` or `1`,
  not packed bits or ASCII characters. Declared widths are 1–8 bits; values
  outside the declared width are rejected. A four-bit symbol still occupies a byte.
- **Sequential data:** strict validation needs at least 1,000,000 samples.
  Standalone assessments allow smaller datasets with warnings, and some
  estimators may be unavailable on them.
- **Restart data:** exactly 1,000,000 symbols in a 1000×1000 row-major matrix,
  collected from repeated source restarts. Reshaping an ordinary file is only a
  software test; it does not create restart evidence.
- **Non-vetted conditioned output:** unpacked bits, one `0` or `1` byte per bit,
  representing at least 1,000,000 complete output blocks for `validate`.

The CLI accepts regular files, not stdin or device streams. Non-IID/IID/restart
commands support byte offsets and sample selection; `validate` reads complete
files. For wider source symbols, use `SymbolReduction` with a caller-supplied
ranking rather than silently treating each byte as an independent sample.

## Interpret results

**`Ok(report)` does not mean the assessment passed.** Inspect the decision,
warnings and `eligible_entropy()`. `None` withholds a partial or rejected result;
raw fields may still contain useful diagnostic minima or IID candidate entropy.
A complete small-sample estimate does not satisfy the strict validation rules.

| Result | Units |
| --- | --- |
| Combined IID/non-IID and validated restart entropy | Bits per original input symbol/raw sample |
| Bitstring branch estimate | Bits per bit |
| Conditioning result | Bits per delivered output block, after optional truncation |
| Final `validate` result | Bits per raw sample without conditioning; bits per delivered output block with it |

All CLI commands support `--json`. Entropy values are decimal strings, not JSON
floating-point numbers. Reports retain relevant options, file hashes and actual
random seeds. Final assessment entropy is null when required evidence is
incomplete or rejected; intermediate fields are not final entropy awards.

| Exit code | Meaning |
| --- | --- |
| `0` | Complete calculation, non-rejected assessment, or successful file operation |
| `1` | Statistical/restart rejection, where applicable |
| `2` | Usage, input, calculation or I/O error |
| `3` | Incomplete assessment |

Non-IID estimation has no hypothesis-test rejection verdict. An IID test that
does not reject is not proof of independence. RCT/APT health monitoring detects
specified failure patterns; it does not replace entropy assessment.

## Verification and limits

The reference harness builds pinned NIST revision
`87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289`, generates fresh results, and compares
Rust against that build across seven suites. It separately compares fresh NIST
results with bundled `refdata`. Comparisons use explicit per-test tolerances,
not a universal claim of bit-for-bit agreement or direct Rust-to-`refdata` parity.

Most calculations use 256-bit MPFR by default. Reference grouping/permutation
decisions and restart simulation retain documented binary64 behavior. On M1,
NIST's `long double` has 53-bit precision; the harness offers an opt-in allowance
for eight named predictor fields across `biased-random-bytes` and `ringOsc-nist`,
bounded by `1e-9`. These are fresh-NIST-versus-`refdata` baseline exceptions,
not permission for arbitrary Rust mismatches. Linux CI requires extended
precision and enables no such baseline allowance.

See [reference testing](docs/reference-testing.md) to reproduce the checks,
[CI runs](https://github.com/walters-labs/sp-800-90b/actions/workflows/reference-comparison.yml)
for revision-specific results, and the [coverage audit](docs/coverage-audit.md)
for dated evidence, deliberate differences and remaining coverage gaps.
Full end-to-end workflow reference parity, broad fuzzing and performance
benchmarks remain open. Physical noise models, collection provenance, IID
rationale, vetted-component assurance and device failure handling remain
responsibilities of the source developer and validation process.

## License and attribution

Original contributions are MIT licensed; reference-informed adaptations retain
NIST's exact notice. [LICENSE](LICENSE) contains the combined terms and
modification notice, with separate [MIT](LICENSE-MIT) and [NIST](LICENSE-NIST)
texts. See [source provenance](docs/provenance.md) for the adaptation map and
[THIRD_PARTY.md](THIRD_PARTY.md) for dependency licenses, including the LGPL
arithmetic dependencies. The inventory is not a linked-binary redistribution
bundle. This project is not NIST-endorsed or certified.
