# Reference testing

[Getting started](../README.md) · [Coverage audit](coverage-audit.md)

From the repository root, regular checks require no NIST checkout:

```sh
cargo test --all-targets --locked
cargo test --doc --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --check
```

Expensive and external-reference tests are opt-in. The pinned harness below
runs the reference targets; it does not run every ignored test or replace the
regular checks above. A normal crate build needs no NIST checkout or C++ tools.

## Continuous integration

The [regular CI workflow](../.github/workflows/ci.yml) runs four jobs on Ubuntu
24.04 x86-64: assessments and minimal health monitoring, each on Rust 1.85.0
(the MSRV) and stable. It uses the checked-in lockfile.

| Configuration | Checks |
| --- | --- |
| Default features | All-target tests, doctests, documentation with warnings denied |
| `assessment` without defaults | All-target compilation, including library examples and assessment test binaries |
| No default features | All-target tests, doctests, documentation with warnings denied, no enabled normal/build dependencies on any target, and library compilation for `thumbv7em-none-eabi` |
| Stable only | Formatting and Clippy with warnings denied for full and minimal features |

The assessment jobs cache Cargo and GMP/MPFR builds. Minimal jobs require no
native-library installation. Their host tests exercise the monitor's behavior;
the bare-metal check verifies compilation without `std`, not execution on a
device. Expensive ignored tests remain opt-in or covered by the separate
reference workflow.

To reproduce the feature checks locally, use `cargo +1.85.0` or `cargo +stable`:

```sh
cargo +1.85.0 check --locked --no-default-features --features assessment --all-targets
cargo +1.85.0 test --locked --no-default-features --all-targets
cargo +1.85.0 test --locked --no-default-features --doc
cargo +1.85.0 tree --locked --no-default-features --edges normal,build --target all
rustup target add --toolchain 1.85.0 thumbv7em-none-eabi
cargo +1.85.0 check --locked --no-default-features --lib --target thumbv7em-none-eabi
RUSTDOCFLAGS="-D warnings" cargo +1.85.0 doc --locked --all-features --no-deps
RUSTDOCFLAGS="-D warnings" cargo +1.85.0 doc --locked --no-default-features --no-deps
```

Both workflows run on pull requests, pushes to main/master, and manual dispatch.
See [regular CI runs](https://github.com/walters-labs/sp-800-90b/actions/workflows/ci.yml)
and [reference comparison runs](https://github.com/walters-labs/sp-800-90b/actions/workflows/reference-comparison.yml)
for results on a specific revision. A configured workflow is not evidence that
an unpushed revision has passed hosted checks.

## Run the pinned comparison harness

Run the harness using the existing NIST checkout:

```sh
python3 scripts/reference_harness.py --reference ../SP800-90B_EntropyAssessment
```

It exports the exact commit in `tests/reference/manifest.json` to a fresh
`target/reference-runs/<timestamp>-<pid>/nist` directory. The local checkout's
branch, working files, and existing binaries are not modified or used as build
inputs. The pinned commit must be present locally. To fetch it instead, use
`--fetch`; only that option downloads NIST sources.

The harness builds all five NIST tools and four thin adapters, generates fresh
results for all eleven bundled datasets, runs NIST's baseline comparator, and
runs all seven Rust reference-test targets with `--ignored --test-threads=1`.
The restart adapter also checks a five-million-round positive-entropy sanity
cutoff using a fixed seed. Full workflow/reference parity remains a separate
coverage goal; passing these comparisons does not establish source validation.

On Linux, install GCC with OpenMP, Python 3, Perl, Git, pkg-config, and the
jsoncpp, OpenSSL, bzip2, divsufsort, MPFR, and GMP development packages. The
`.github/workflows/reference-comparison.yml` workflow installs those dependencies,
selects Rust 1.97.1, fetches the pinned reference, and runs the strict harness on
Ubuntu x86-64. It requires `long double` to have more than 53 mantissa bits and
uploads logs and reports even on failure. CI runs on pull requests, pushes to
main/master, and manual dispatch. See [workflow runs](https://github.com/walters-labs/sp-800-90b/actions/workflows/reference-comparison.yml) for revision-specific results.

For Apple Silicon with GCC, use a jsoncpp build compatible with that compiler.
The following example assumes GCC 16 and a separately prepared static jsoncpp
library at the shown path; adapt those paths to your installation. Homebrew
Clang-built C++ libraries are not interchangeable with GCC-built libraries:

```sh
reference="$HOME/GitHub/SP800-90B_EntropyAssessment"
python3 scripts/reference_harness.py \
  --reference "$reference" \
  --cxx /opt/homebrew/bin/g++-16 \
  --cppflags="-I$reference/.reference-build/jsoncpp-gcc/include -I/opt/homebrew/include -I/opt/homebrew/opt/openssl@3/include" \
  --ldflags="$reference/.reference-build/jsoncpp-gcc/lib/libjsoncpp.a -L/opt/homebrew/lib -L/opt/homebrew/opt/openssl@3/lib -lcrypto" \
  --allow-binary64-baseline-differences
```

`--cppflags` and `--ldflags` (or `CPPFLAGS`/`LDFLAGS`) accept shell-style quoted
argument lists. Without overrides, jsoncpp/OpenSSL flags come from pkg-config.
Paths containing spaces need quoting *inside* those argument lists too.

Each run records commands, toolchain versions, source/dataset hashes,
`LDBL_MANT_DIG`, elapsed times, exit codes, baseline decisions, and the test
environment in `report.json`, with separate command logs. Commands have a
configurable timeout (`--timeout`, default 1800 seconds); timed-out process
groups are terminated. No prior run's results are reused.

The binary64 allowance is opt-in and applies only to eight named predictor
fields on two datasets, with absolute differences no greater than `1e-9`.
Missing/extra fields, unknown mismatches, larger errors, and Rust comparison
failures still fail the run. Qualified success is explicitly reported as
`passed_with_binary64_baseline_differences`, not as a clean NIST self-test pass.
Linux CI does not enable this allowance.

Dated results and local evidence paths are recorded in the
[coverage audit](coverage-audit.md#reference-verification--2026-10-20). Local M1
results and hosted Linux results are separate evidence.

Comparison tolerances remain explicit in the Rust test assertions: typically
`1e-12` for basic estimates, `1e-9` for predictor roots and combined summaries,
`1e-10` for conditioning, and `1e-6` for the restart CLI's printed entropy.
Chi-square scores use a scaled `1e-9` tolerance; deterministic permutation tail
counts, decisions, and restart simulation cutoffs are compared exactly.
Existing documented differences (including the skipped reference IID-LRS
underflow fixture and Rust's stricter failure handling) remain in the
[coverage audit](coverage-audit.md).

The restart reference suite includes three additional non-IID end-to-end cases:

- Binary `truerand_1bit.bin` with `H_I = 0.5` bits per sample: successful validation.
- Four-bit `truerand_4bit.bin` with `H_I = 2` bits per sample: successful validation.
- A generated 1000×1000 checkerboard with `H_I = 0.5`: balanced row/column
  frequencies pass sanity, but predictability causes entropy-threshold rejection.

Each compares row and column entropy, sanity outcomes, final decisions, and
awarded entropy (or its absence). CLI summaries use a `1e-6` tolerance because
NIST prints six decimal places. Since the CLI seeds its Monte Carlo simulation
from the OS, its numerical cutoff is not assumed to match Rust's fixed-seed
cutoff. Exact cutoff parity is checked separately against the simulator adapter
with seed `[1, 2, 3, 4]` and five million rounds. These fixtures exercise the
software; reshaping bundled bytes is not evidence of physical source restarts.

The harness includes these tests automatically. To run just the positive-entropy
restart cases using a previously built harness snapshot:

```sh
NIST_REFERENCE_DIR="<run-directory>/nist" \
NIST_RESTART_REFERENCE="<run-directory>/restart" \
cargo test --test restart_reference positive_ -- --ignored --nocapture --test-threads=1
```


The restart suite also includes `positive_iid_restart_matches_reference`, a
successful million-sample four-bit IID restart at H_I=2. It requires all row and
column IID checks to pass and compares the exact sanity cutoff, maximum count,
and the row, column, and awarded entropy. The rebuilt restart adapter's `--cli`
mode invokes NIST's original workflow with seed [1, 2, 3, 4] and one OpenMP
thread, making both sanity and permutation testing reproducible. This case uses
one reference simulation rather than separately rerunning the cutoff adapter.
Use the same environment variables as above and select
`positive_iid_restart_matches_reference` to run it alone.
