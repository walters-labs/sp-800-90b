# SP 800-90B coverage audit

Updated: 2026-10-20 (original audit: 2026-10-19). Implementation scope: published
release `v0.1.0`, commit
[`0b4831660435445814c07211724ea7321a980959`](https://github.com/walters-labs/sp-800-90b/commit/0b4831660435445814c07211724ea7321a980959).
Reference implementation: pinned NIST revision
`87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289`. This post-release documentation update
records the evidence for that implementation; it changes no numerical behavior.

The core numerical engine is broadly implemented. The crate is **not yet a complete implementation of the entire publication or a command-compatible replacement for NIST's package**. The strict numerical validation workflow now applies the submitter bound and coordinates track selection, restart testing, and conditioning. Additional-source accounting is implemented for concatenated conditioning inputs. Ranked symbol reduction is available for unsigned inputs up to 128 bits. Release evidence also needs expansion, especially complete validation-workflow comparisons.

This is a coverage and integration audit, not a proof of algorithm correctness or a certification. “Implemented” below means an API and targeted tests exist; the evidence column defines the strength of that claim. The current-status tables incorporate the implementation follow-ups below. Earlier verification counts are historical snapshots; the latest evidence is summarized in “Evidence and its limits.” This documentation reconciliation changes no estimator or decision behavior.

## Baselines

- [January 2018 publication](https://nvlpubs.nist.gov/nistpubs/SpecialPublications/nist.sp.800-90b.pdf).
- [NIST publication status](https://csrc.nist.gov/pubs/sp/800/90/b/final) and [May 2025 potential errata](https://csrc.nist.gov/files/pubs/sp/800/90/b/final/docs/sp800-90b_errata_potential_updates.pdf).
- [Pinned reference source](https://github.com/usnistgov/SP800-90B_EntropyAssessment/tree/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289). This audit does not claim parity with an unpinned future upstream revision.

The May 2025 potential errata propose corrections; they are not official changes
to the publication. Both entries now have explicitly named regression tests:

- **§5.2.4:** `potential_errata_2025_section_5_2_4_binary_critical_value` in
  [chi_square_goodness_of_fit.rs](../tests/chi_square_goodness_of_fit.rs) checks
  binary samples with statistics 27.8768 (pass) and 27.8776 (reject). These bracket
  the df=9, alpha=0.001 critical value, rounded to the proposed 27.877. The rejected
  case would pass under the original printed 27.887 cutoff. The implementation
  evaluates the tail probability rather than using a rounded cutoff.
- **§6.3.1:** `potential_errata_2025_section_6_3_1_example_preserves_l_minus_one` in
  [most_common_value.rs](../tests/most_common_value.rs) distinguishes the proposed
  example arithmetic from the specified estimator. The printed intermediate
  variance 0.012 gives approximately 0.6822 for the upper probability and 0.5518
  bits for entropy, as proposed. However, the example's 20 samples have
  `p_hat = 8/20`, and the specified `p_hat * (1 - p_hat) / (L - 1)` is
  `0.24/19`, not 0.012. With the publication's confidence constant 2.576, the
  estimator therefore returns approximately 0.6895174061 and 0.5363411235 bits.
  The test checks these outputs against independent high-precision decimal
  calculations and separately checks the proposed printed arithmetic. No
  estimator formula is changed to match the inconsistent example.

## Computational coverage

| Area | Status | Implementation and evidence | Remaining scope |
| --- | --- | --- | --- |
| §6.3.1 Most Common Value | Implemented | `most_common_value.rs`; unit tests and optional NIST dataset comparisons | Default reference constant differs intentionally from the rounded publication constant |
| §6.3.2 Collision | Implemented | `collision.rs`; numerical/boundary tests and reference comparisons | Binary input only, as intended |
| §6.3.3 Markov | Implemented | `markov.rs`; tests and reference comparisons | Binary input only |
| §6.3.4 Compression estimator | Implemented | `compression.rs`; tests and reference comparisons | Not the bzip2 permutation statistic |
| §6.3.5 t-Tuple | Implemented | `t_tuple.rs`, shared `tuples.rs`; tests and reference comparisons | Wider input requires preprocessing |
| §6.3.6 LRS estimator | Implemented | `lrs.rs`, shared `tuples.rs`; tests and reference comparisons | Distinct from the IID LRS hypothesis test |
| §6.3.7 MultiMCW | Implemented | `multi_mcw.rs`; tests and reference comparisons | — |
| §6.3.8 Lag | Implemented | `lag.rs`; tests and reference comparisons | — |
| §6.3.9 MultiMMC | Implemented | `multi_mmc.rs`; tests and reference comparisons | — |
| §6.3.10 LZ78Y | Implemented | `lz78y.rs`; tests and reference comparisons | — |
| §5.1 permutation statistics | Implemented | `permutation.rs`; all 19 statistics, seeded counters, binary conversion tests | Serial RNG stream, not OpenMP scheduling equivalence |
| §5.2.1–5.2.4 chi-square | Implemented | `chi_square.rs`; binary/nonbinary independence and goodness of fit; all eleven dataset comparisons available | Small-input errors intentionally differ from reference degenerate passes |
| §5.2.5 IID LRS | Implemented | `iid.rs`; brute-force checks, MPFR tail, ten dataset reference comparison | Reference underflows on full ring-oscillator input; Rust has a synthetic extreme-repetition regression |
| §6.1 and §6.2 assessment | Implemented for byte-valued data | `assess_iid`, `assess_non_iid`; initial/conditioned modes, width, raw bit expansion, truncation | Strict numerical coordinator now available through `assess_validation` |
| §3.1.2 track selection | Implemented numerical workflow | `assess_validation` and `validate` coordinate sequential/restart/conditioned evidence, reuse IID checks, and fall back to non-IID after rejection | Physical rationale for the IID claim remains external; complete workflow reference parity is not established |
| §3.1.3 initial entropy | Implemented | `assess_validation` and `validate` apply the smaller of sequential entropy and H_submitter before restart testing | Submitter bound must be justified externally |
| §3.1.4 restart | Implemented with validation gaps | `restart.rs`; strict matrix size, simulated cutoff, both estimator tracks, transpose | Positive-entropy IID/non-IID success and non-IID entropy rejection are covered; complete validation-workflow comparison remains separate |
| §3.1.5 conditioning bounds | Formula implemented | `conditioning.rs`; directed rounding, eight raw-dimension reference cases and nine typed-component cases | Caller supplies widths, h_in, h'; typed Table 1 dimensions, proportional truncation and numerical orchestration are implemented; additional-source accounting is implemented |
| §3.1.6 additional sources | Implemented numerical accounting | Separate primary/additional/total lengths; only primary entropy and width credited; vetted conditioning enforced | Correct physical source identification and actual concatenation remain external |
| §4.4 RCT/APT | Implemented, including `no_std` monitoring | `health.rs` calculates MPFR cutoffs; `health_core.rs` provides allocation-free monitoring from validated integer parameters; all ten Table 2 cutoffs, boundaries, chunking, latching, reset | Only byte-valued alphabets; optional opposite-symbol binary APT extension absent; cutoff derivation requires `assessment` |
| §4.2–4.3 startup and on-demand operations | Partial | Startup sample counter, readiness, explicit reset | Device output gating, failure recovery, on-demand scheduling, and source-specific tests are not implemented |
| §6.4 symbol-space reduction | Implemented (up to 128 bits) | `SymbolReduction` validates a complete caller-supplied ranking and selects 1–8 bits per sample | Source-model ranking and file byte order remain caller responsibilities; arbitrary-width and alternative reductions are not supported |
| Appendix E internal-width accounting | Partial | Conditioning clamps nw to n_in | Caller must establish component width |
| Appendix F CBC-MAC conditioner | Outside numerical-tool scope; absent | No cryptographic conditioning implementation | Needed only if scope expands to operating a conditioner, not merely assessing entropy |
| Appendices A–D and G | Explanatory/model material | Algorithms use the corresponding estimator concepts | No separate “appendix implementation” or proof claim |

Source links for the corresponding code are collected in [the public module exports](../src/lib.rs). Individual estimator reference coverage is in [nist_reference.rs](../tests/nist_reference.rs); orchestration has separate reference suites.

## NIST command parity

| Reference command | Rust equivalent | Missing command behavior |
| --- | --- | --- |
| `ea_non_iid` | `assess_non_iid`, installed `sp-800-90b non-iid` | Explicit Rust flags/status contract; full intermediate diagnostic JSON and NIST flag aliases are not implemented |
| `ea_iid` | `assess_iid`, installed `sp-800-90b iid` | Rust CLI contract differs from NIST flags; full intermediate grouping diagnostics remain API-only |
| `ea_restart` | `assess_restart`, installed `sp-800-90b restart` | Explicit Rust flags/report contract; refuses validated IID entropy after rejected/unavailable checks |
| `ea_conditioning` | `conditioning_entropy`, `examples/conditioning.rs`, installed `conditioning` command with JSON | Standalone file-to-h' orchestration (available through `validate`), reference epsilon diagnostics and full-entropy eligibility messages |
| `ea_transpose` | `restart_column_sequence`, installed `transpose` utility | Fixed 1000×1000 matrices; explicit byte offset/selection flags rather than NIST flag aliases |
| Reference self-test runner | `scripts/reference_harness.py`, pinned isolated builds and seven Rust reference targets; Linux CI passed for `v0.1.0` | Full validation-workflow parity is a separate coverage goal |

`src/main.rs` provides `non-iid`, `iid`, `restart`, `conditioning`, `validate`, and `transpose` commands with versioned JSON reports and explicit completion/rejection/error exit codes. Transposition writes a new file and refuses overwrite. Example programs may return successfully after printing rejected assessments; their process exit status must not be used as an assessment pass indicator. The installed CLI intentionally uses its own flags and result contract.

The reference package does not supply the RCT/APT streaming device integration that our health module would need to replace. Its absence is a publication/application integration gap, not a missing `ea_health` port.

## Intentional compatibility differences

These require preservation in documentation and regression tests rather than automatic removal.

- **Arithmetic:** most estimates use 256-bit MPFR by default; conditioning uses directed rounding and additional working precision. General estimators do not provide certified interval enclosures. Higher precision does not itself prove correctness.
- **Discrete reference decisions:** default chi-square grouping uses binary64 thresholds/order; permutation reference mode uses binary64 excursion/average collision; restart simulation uses binary64 probabilities. Exact-count alternatives exist for grouping and permutation comparisons, not restart simulation.
- **Randomness:** seeds are retained and reproducible in serial execution. Neither permutation nor restart results promise the same streams as the reference's OS-seeded OpenMP execution.
- **Input handling:** out-of-width values are rejected rather than masked. Initial binary selection uses observed alphabet size. Strict byte input and caller-declared source alphabet have different roles in assessment and health monitoring.
- **Small and incomplete inputs:** assessment APIs allow exploratory smaller datasets with warnings. Some individual checks return explicit errors. Non-IID reports expose partial minima and `is_complete`; IID exposes a candidate separately from an eligible result. A successful Rust function call alone is not validation success.
- **Restart:** exact million-sample shape; sanity failure short-circuits; IID rejection and incomplete estimation prevent a validated output. The reference can reach its final validation-success message after printing IID failures.
- **Conditioning:** zero input entropy/h' is supported; no automatic full-entropy label; finite working-precision budget. The caller asserts vetted status.
- **Permutation minimums:** conservative lengths guarantee collision statistics for every shuffle. Some shorter sequences that happen to have collisions are rejected by the API.

## Evidence and its limits

### Published v0.1.0 verification

The [crates.io package](https://crates.io/crates/sp-800-90b/0.1.0) and
[GitHub release](https://github.com/walters-labs/sp-800-90b/releases/tag/v0.1.0)
were published on 2026-10-20. The release tag resolves to commit
`0b4831660435445814c07211724ea7321a980959`, the revision verified by both hosted
workflows below.

- **Regular CI:** [run 39484659047](https://github.com/walters-labs/sp-800-90b/actions/runs/39484659047)
  passed all four jobs on Ubuntu 24.04 x86-64 using Rust 1.85.0 and stable
  (Rust 1.99.1 in this run). Each compiler passed **178 default-feature tests and
  16 doctests**, with 36 opt-in tests ignored, plus **10 minimal-feature tests and
  1 doctest**. Assessment-only compilation, the minimal dependency guard,
  `thumbv7em-none-eabi` library compilation, and full/minimal documentation builds
  with warnings denied passed. Stable also passed formatting and full/minimal
  Clippy with warnings denied.
- **Strict NIST reference comparison:** [run 39484659051](https://github.com/walters-labs/sp-800-90b/actions/runs/39484659051)
  passed on Ubuntu 24.04 x86-64 with Rust 1.97.1 and the pinned NIST revision.
  The report records `long_double_mantissa_bits: 64`, status `passed`, and no
  failures. All **28 Rust reference tests across seven targets** passed, as did
  all **11 fresh-NIST-versus-bundled-`refdata` baseline comparisons**. The binary64
  baseline allowance was disabled. The workflow's `nist-reference-results`
  artifact contains `20261020T193559Z-2777/report.json` and the command logs.
- **Release package:** `cargo package --locked` and
  `cargo publish --dry-run --locked` passed on Apple Silicon macOS. The package
  contains 87 files, including the README, guides, provenance record, dependency
  inventory, and all three license files. The unpacked package passed default
  and assessment-only all-target compilation on Rust 1.85.0; minimal-feature
  tests, its doctest, and bare-metal compilation also passed, with no enabled
  normal/build dependencies in the minimal configuration. The published registry
  checksum matches the verified local `.crate` archive:
  `e692f21f34d0cafdf867ce5ca0a37b390c8eb1909223f954dff09cdbb828b674`.

The Rust comparisons use freshly built NIST outputs. The separate baseline
check compares fresh NIST outputs with bundled `refdata`; this is not a direct
Rust-to-`refdata` comparison. Passing these fixtures does not establish complete
validation-workflow parity or source certification. The bare-metal check verifies
compilation, not execution on a device.

### Local supporting checks

Local verification also passed on 2026-10-20, with Rust 1.85.0 and the locally
installed stable toolchain (Rust 1.97.1) on Apple Silicon macOS after separating
`no_std` health monitoring and adding regular CI:

- `cargo test --locked --offline --all-targets` on each compiler: **178 passed, 0 failed, 36 opt-in tests ignored**, including both named potential-errata regressions.
- `cargo test --locked --offline --doc` on each compiler: **16 passed**.
- With `--no-default-features` on each compiler: **10 tests and 1 doctest passed**; all test targets compile, with assessment/CLI tests disabled.
- `cargo check --locked --offline --no-default-features --features assessment --all-targets` on each compiler: passed, covering the full library without the CLI.
- Minimal library compilation for `thumbv7em-none-eabi` on each compiler: passed. This is a compile check, not hardware execution.
- The minimal dependency guard passed on both compilers. It also rejected simulated normal/build dependencies on another target while permitting dev-only dependencies.
- Clippy with warnings denied for full/minimal features and formatting on Rust 1.97.1, and whitespace checks: passed. Full/minimal rustdoc builds pass with warnings denied on both compilers.
- Actionlint, including shell checks, passed for the CI workflows. This is local workflow validation, not a hosted Linux run.
- Before the API usability additions, the pinned reference harness separately ran **28 reference tests across seven targets; all passed**. Its qualified status was `passed_with_binary64_baseline_differences`: nine clean baseline datasets and the two documented M1 exceptions.
- The new full-size restart and validation CLI tests were run separately and passed, including IID success, fallback, conditioning, and rejection. The regular-suite ignored count is not a claim that all opt-in tests were skipped throughout the session or that all 36 were rerun together.

The minimum supported Rust version (MSRV) is **1.85.0**. The 2024 edition and
locked dependencies `rug` 1.30.0, `getrandom` 0.4.3 and `az` 1.3.0 already require
Rust 1.85, so this is the earliest compatible compiler without changing those
choices. Six `if let` chains were rewritten as nested conditions and one integer
`is_multiple_of(2)` call became `% 2 == 0`; the conditions, calculations and
dependency versions are unchanged. All targets, including the CLI, examples and
opt-in test binaries, compile on the MSRV; the ignored tests were not executed
in the regular suite; the reference workflow runs its seven opt-in targets
separately. The reference CI workflow selects Rust 1.97.1. The separate
[regular CI workflow](https://github.com/walters-labs/sp-800-90b/blob/main/.github/workflows/ci.yml) checks default, assessment-only
and minimal features on Rust 1.85.0 and stable, including minimal dependency
isolation and bare-metal compilation. Stable jobs also enforce formatting and
Clippy; both compilers build full/minimal documentation with warnings denied.
Both workflows passed for the published release, as recorded above.

The default features remain `assessment` and `cli`, preserving the existing API
and commands. `HealthTests` delegates monitoring to the same allocation-free
`HealthTestMonitor` available with no default features. `HealthTestConfig` accepts
precomputed integer cutoffs and derives the standard window from the alphabet.
It validates structural ranges, not statistical suitability; host-side
`HealthTestCutoffs::monitor_config()` exports parameters from the MPFR result.
The minimal core uses no `std`, `alloc`, floating-point operations or external
dependencies. Tests cover rejection at cutoff equality, startup counting,
chunk/window boundaries, sparse alphabets, retained prefixes after input errors,
counter overflow/saturation, reset, and the valid APT cutoff W+1. Hardware
integration and automatic output gating remain application responsibilities.

The [earlier M1 reference verification](#reference-verification--2026-10-20)
records local evidence paths; the [published release verification](#published-v010-verification)
records the strict Linux result. The following limits come from the test assertions:

| Suite | What it checks | What it does not establish |
| --- | --- | --- |
| `nist_reference.rs` | Each estimator against bundled dataset outputs | Inputs beyond those fixtures; independent proof of formulas |
| `non_iid_reference.rs` | Eleven complete reports and selected mode/width/truncation cases | Universal preprocessing parity |
| `chi_square_reference.rs` | Degrees of freedom, scores, probabilities and decisions on eleven datasets | Near-threshold coverage for every grouping branch |
| `permutation_reference.rs` | Original statistics on eleven datasets; seeded counters on three 4096-sample prefixes | Full-million-sample seeded suite parity or all seed distributions |
| `iid_reference.rs` | LRS on ten datasets; entropy/chi-square/LRS-length comparison in both modes on one 4096-sample prefix | Combined test does not compare the native permutation verdict; native seed is uncontrolled |
| `restart_reference.rs` | Binary and four-bit non-IID restart cases with positive H_I; sanity-pass/entropy-rejection case; exact seeded sanity cutoffs; H_I=0 baseline, seeded successful IID restart and IID incomplete-check propagation | Broader source fixtures |
| Restart simulator tests | Seeded 10,000-trial quantile parity at five H values; full five-million-trial sanity rejection | Distribution of empirical cutoff variation (positive-H_I workflow comparisons are covered separately) |
| `conditioning_reference.rs` | Eight vetted/non-vetted outputs and nine typed-component dimension cases within tolerance | Full parameter domain, all near-full-entropy regimes or a formal rounding proof |
| `health.rs` / `health_core.rs` tests | Published cutoff examples, exported parameters, streaming semantics in full/minimal builds, integer parameter validation and counter boundaries | Device fault injection, operational failure response or arbitrary-parameter interval certification |

Individual reference tests can be run against caller-supplied environment paths. The harness provides stronger provenance: it exports the pinned source, rebuilds tools/adapters, generates fresh fixture results, and records hashes, compiler flags, precision and timings. The successful Linux release CI and local Apple Silicon checks do not constitute a comprehensive cross-platform test matrix. Dedicated benchmark and fuzz suites remain open, as does complete validation-workflow reference parity.

## Prioritized completion work

| Priority | Gap | Concrete completion criterion |
| --- | --- | --- |
| Done | **G1 Validation workflow and submitter bound** | `assess_validation` accepts H_submitter, checks dataset eligibility, coordinates one IID/non-IID track, restart and conditioning, and withholds final entropy on incomplete or rejected evidence. Exploratory APIs remain available; integration tests cover bounds, units and decisions. |
| Verified for v0.1.0 | **G2 Reproducible reference harness and CI** | Pinned builds, adapters, provenance and fresh results are implemented. Strict Linux reference CI passed on the published commit: 28 reference tests and all 11 NIST baseline comparisons, with 64-bit `long double` precision and no binary64 allowance. See the release verification above. |
| Implemented | **G3 Restart validation evidence** | Positive-H_I binary/nonbinary non-IID success, sanity-pass/entropy-rejection, and successful IID restart comparisons with deterministic permutation control are implemented. |
| Done (up to 128 bits) | **G4 Wider symbol support** | `SymbolReduction` preserves sample boundaries and records input/output widths, ranking, and entropy ceiling. Tests cover bit order, width boundaries, invalid rankings and samples, exhaustive 16-bit projection, and downstream NIST reference agreement. |
| Implemented | **G5 Conditioning integration** | Typed Table 1 dimensions, vetted truncation, and strict assessment integration are implemented. Explicit additional-source accounting is implemented. |
| Implemented (schema version 1) | **G6 CLI and reports** | All six commands expose documented flags and applicable status/exit, decimal-string entropy, hash and seed fields. Non-IID/IID/restart/transpose support selection; validate reads full files and conditioning accepts numerical inputs. Full intermediate estimator diagnostics remain API-only; NIST flag aliases are intentionally absent. |
| P2 | **G7 Numerical and robustness coverage** | Named regressions cover both potential errata as described above. Add independent golden vectors, rounding-bound tests for conditioning, tail minimality/boundary checks for health cutoffs, and property/fuzz tests for preprocessing and state machines. Benchmark time and peak memory on representative million-sample inputs. |
| Published v0.1.0 | **G8 Release preparation** | The API, NIST attribution, README, potential-errata and MSRV reviews are complete. Rust 1.85.0 support, feature configurations, package contents and both hosted workflows were verified on the published revision. Version 0.1.0 is published on crates.io and GitHub; the registry archive checksum matches the verified package. Platform limits and dependency licenses remain documented. Prebuilt CLI binaries are not provided; the license inventory is not a linked-binary redistribution bundle. |
| Partial; further scope decision | **G9 Operational source integration** | Allocation-free `no_std` RCT/APT monitoring is available. GetNoise/GetEntropy interfaces, output gating, on-demand operation and failure recovery remain application responsibilities; adding those device operations would require a separate scope decision. |

Version 0.1.0 publishes the numerical workflows through the library and six-command
CLI, with successful package verification and both hosted CI workflows on the
release commit. Remaining work includes broader numerical and robustness coverage,
performance benchmarks, and complete validation-workflow reference comparisons.
Operational source integration remains a separate scope decision.

## Requirements that remain external

Physical noise models, restart procedures, environmental characterization, collection provenance, evidence for IID assumptions, vetted-component assurance, and source-specific failure analysis remain responsibilities of a source developer and validation process. A software API can collect and enforce the presence of that evidence, but cannot establish it from a byte file. Accordingly, completing the code roadmap would produce a more complete assessment implementation, not a claim of NIST certification.

## Implementation and verification history

The sections below record incremental implementation evidence. Test counts and
statements that a suite was not rerun refer to that stage, not the latest full
verification summarized above.

### Validation workflow follow-up

The G1 implementation adds deterministic integration tests for submitter bounds, conditioning units, eligibility failures, restart rejection, incomplete IID evidence and IID fallback. The opt-in full-size IID success regression passed on 2026-10-20 in 46.34 seconds after internal restart evidence reuse and test-profile optimization of libbz2-rs-sys. It verifies three permutation assessments and reuse of both restart checks. This is a deterministic synthetic fixture, not reference parity. These tests support the current coverage summary; they do not establish reference parity for the complete workflow. G5 now includes typed vetted algorithm metadata and explicit additional-source input accounting; proportional vetted-output truncation is now implemented.

The workflow now exposes stage timings and live permutation counts/active statistics through `assess_validation_with_progress`. Low-level IID and permutation APIs have corresponding observer variants. Regression tests verify unchanged permutation tail counts/results and preservation of literal MCV entropy in reused restart evidence.

## Reference harness follow-up (2026-10-20)

`scripts/reference_harness.py` exports the pinned commit from a local checkout
or explicitly fetches it with `--fetch`, builds isolated NIST tools and adapters,
and runs seven reference integration-test targets. It preserves NIST baseline
comparisons separately from Rust comparisons, records compiler precision,
commands, hashes, logs and timings, and fails on unexpected differences. The
binary64 exception is an opt-in allowlist of eight predictor fields across two
datasets, bounded by 1e-9 absolute error. Linux CI requires extended precision
and enables no baseline exceptions. Adding the workflow does not constitute a
completed hosted CI run.

The harness adds exact fixed-seed comparison of the positive-H_I=1 restart sanity
cutoff over five million rounds, including the resulting sanity rejection on a
structured restart matrix. Successful IID restart parity is covered by the seeded CLI adapter described below. Non-IID coverage
is expanded further by the cases below.

An earlier local M1 harness run on 2026-10-20 passed the then-current 22 reference-suite
tests. NIST's own baseline comparisons had nine clean passes and two datasets
with the eight explicitly allowed binary64 differences. The resulting status
was `passed_with_binary64_baseline_differences`; this is not a clean upstream
self-test pass or evidence of a hosted Linux CI run.

## Positive-entropy restart coverage

Three non-IID end-to-end reference cases now exercise the pinned NIST CLI and
Rust against identical million-sample matrices: binary truerand at H_I=0.5,
four-bit truerand at H_I=2, and a generated checkerboard at H_I=0.5. The first
two require successful validation with positive awarded entropy. The last
requires successful sanity checking followed specifically by entropy-threshold
rejection, with no awarded entropy. All cases compare complete row/column
estimates (1e-6 for six-decimal CLI summaries), maximum symbol frequency, final
decisions and the awarded minimum or its absence.

NIST's CLI retains its OS-seeded sanity simulation. These cases use comfortable
frequency margins and compare the actual sanity decisions rather than requiring
identical random cutoffs. An additional call to the unmodified simulator adapter
uses seed [1, 2, 3, 4] and five million rounds, allowing exact cutoff comparison
with Rust. The fixtures test numerical behavior, not physical restart provenance.
The existing harness discovers these tests within `restart_reference`; no new
environment variables or adapter modifications are required.

Local verification on 2026-10-20 passed all three new cases plus the existing
positive-entropy cutoff regression (four tests, 189.23 seconds). The new cases'
maximum frequencies and seeded cutoffs were 556/769 (binary success), 96/316
(four-bit success), and 500/769 (entropy rejection after sanity passed). In this
run the independently seeded CLI cutoffs happened to match too; that coincidence
is not an assertion or reproducibility guarantee.


## Successful IID restart comparison

The seeded restart adapter now also invokes NIST's original CLI workflow via
`--cli`. Only the OS seed boundary is replaced with [1, 2, 3, 4], and OpenMP is
restricted to one thread. The simulator, IID tests, and CLI decision logic stay
unchanged. The original cutoff-only adapter interface remains available.

The new `positive_iid_restart_matches_reference` case uses the bundled million-
sample four-bit fixture and H_I=2. It requires both row and column chi-square,
LRS, and permutation checks to complete and pass, in addition to sanity and final
validation. NIST's success banner alone is insufficient: the test separately
requires its three combined IID pass reports. Cutoffs and maximum frequencies
are compared exactly; row/column and awarded entropy use the CLI's six-decimal
precision (1e-6 tolerance). A single seeded reference invocation supplies both
cutoff and full-workflow evidence, avoiding a duplicate reference simulation.

The user reported that the previously pushed Linux CI suite passed in 15 minutes
9 seconds. That run predates this new IID case; its hosted execution is pending.

Local verification on 2026-10-20 passed the successful IID comparison in 74.96
seconds (while another reference regression ran concurrently). Both row and
column entropy estimates were 3.97119433672960964 bits/sample in Rust, matching
NIST's printed 3.971194; both implementations awarded 2 bits/sample. Maximum
frequency was 96 and the exact seeded sanity cutoff was 316. The existing
cutoff-only adapter regression also passed at five entropy levels.


## Ranked symbol reduction (§6.4)

`SymbolReduction` supports 1–128-bit unsigned inputs and 1–8-bit outputs,
with a validated full ranking supplied by the caller. Rank 1 becomes the most
significant output bit; numeric positions are indexed from the input's least
significant bit. One output symbol represents one original sample, so the
reduced width is the entropy ceiling per original sample, without rescaling.

Verification on 2026-10-20 passed the mapping tests, regular all-target suite,
nine documentation examples, and Clippy. The opt-in million-sample downstream
reference comparison passed in 20.11 seconds: scattered 32-bit inputs reduced
exactly to NIST's four-bit fixture, and all non-IID estimates and the combined
result matched its recorded self-test output. This checks assessment integration;
NIST does not expose a ranked wide-symbol reducer for direct conversion parity.


## Vetted conditioning truncation

`ConditioningOptions::retained_output_bits` applies §3.1.5.1.2 proportional
entropy scaling after the full-output bound, with downward MPFR rounding.
Reports retain the full length and bound and expose the delivered length and
final entropy. `vetted_truncation_entropy` also supports explicit accounting
from a caller-supplied vetted bound. Non-vetted requests are rejected under
§3.1.5.2. The validation coordinator checks configurations before assessment
and uses the final delivered-block bound. Tests cover the publication's
6-to-4.5-bit example, invalid inputs, non-vetted rejection, conservative rounding,
report semantics, and validation integration. The NIST CLI has no corresponding
truncation flag; existing untruncated reference comparisons remain applicable.

Local verification on 2026-10-20 passed all 138 regular tests, ten documentation
examples, the eight existing conditioning reference cases, Clippy, formatting,
and whitespace checks. The new truncation rule is checked against the publication
example and directed-rounding invariants, not a nonexistent NIST CLI option.


## Typed vetted component descriptions

`VettedConditioner`, `ConditioningHash`, and `ConditioningAesKey` implement the
January 2018 Table 1 dimension rules for all six component families. The typed
assessment derives dimensions and retains component identity; the strict
workflow supports `ValidationConditioning::VettedComponent`. Manual dimensions
remain available for existing callers. AES numeric key sizes and explicit
component/dimension mismatches are rejected. Hash variants constrain digest
sizes by construction, including SHA-512/224 and SHA-512/256.

The metadata does not implement cryptography, verify CAVP assurance, validate
HMAC key handling/CBC-MAC padding, or track changing approval policy. SHA-1 is
present for historical modeling; XOFs and arbitrary digest lengths are excluded.
Table tests cover every supported hash and AES key size. Nine new NIST CLI
comparisons exercise every distinct table width and all six families; these
compare derived numerical inputs, since the CLI itself accepts raw dimensions.
Explicit additional-source accounting is implemented in the follow-up below.

Local verification on 2026-10-20 passed 141 regular tests, eleven documentation
examples, both conditioning reference tests (seventeen numerical cases total),
Clippy, formatting, and whitespace checks.


## Additional-source accounting

`additional_input_bits` defaults to zero in conditioning options. Reports retain
primary/additional/total lengths and primary input entropy. The formula uses only
primary-source n_in and h_in. Additional-source inputs require vetted
conditioning, a nonempty primary input, and a representable total length. The
validation coordinator rejects invalid combinations before statistical work,
and derives primary entropy solely from the primary-source restart result.
Tests cover invariant entropy/probability/precision results as additional lengths
increase, zero primary entropy, invalid primary bounds, non-vetted rejection,
length overflow, truncation, and strict-workflow accounting. Physical source
identification, collection and actual concatenation remain caller responsibilities.

Verification passed all 144 regular tests, twelve documentation examples,
Clippy, formatting and whitespace checks. The external NIST comparison suite
was not rerun for this change; additional-input invariance tests exercise the
existing conditioning calculation without altering its credited inputs.


## Non-IID CLI and JSON summaries

The installed `sp-800-90b non-iid` command supports initial/conditioned modes,
width, precision/confidence options, absolute sample offset, at-most sample
count, and expanded-bit limits. Version 1 JSON preserves entropy as decimal
strings and records selected-input SHA-256, options, warnings, estimator
availability and explicit completeness. Incomplete reports withhold final
entropy while retaining a labeled available minimum. Exit codes are 0 complete,
2 error, and 3 incomplete; no non-IID hypothesis rejection status is invented.
Full intermediate estimator diagnostic serialization remains API-only. All
six planned CLI commands are now implemented. Tests compare CLI output to the library in both modes, check 384-bit
round trips, subsets/hashes, constant and incomplete inputs, and error handling.

CLI verification passed 149 regular tests (including five subprocess CLI tests),
twelve documentation examples, Clippy, formatting, and whitespace checks.
Both non-IID modes were compared directly with the library at 384-bit precision.
The external NIST reference suite was not rerun for this interface change.


## IID CLI

The IID command shares non-IID file selection and entropy options and adds an
optional reproducible four-word permutation seed. JSON retains all four check
outcomes, p-values/statistics, permutation tails and actual seed, candidate
entropy, eligible entropy, and completeness. Decisions map to exit 0 (not
rejected), 1 (rejected), 3 (incomplete), and 2 (error). Rejection/incompleteness
withhold final entropy; constant input explicitly rejects without running checks.
Tests compare both modes with the library at 384-bit precision, verify subset
and bit-limit semantics, exercise rejection/incomplete/error branches, and replay
an OS-seeded run exactly. The other implemented commands are described below.

IID CLI verification passed 152 regular tests, twelve documentation examples,
Clippy, formatting, and whitespace checks. External NIST comparisons were not
rerun for this command-interface addition.


## Restart CLI

The restart subcommand accepts initial entropy, IID/non-IID mode, selection,
precision, confidence, and separate sanity/permutation seeds. It enforces the
million-sample shape and minimum five-million simulation rounds through the API.
Versioned reports retain actual seeds, maxima, cutoff, row/column assessments,
and distinct sanity/entropy/IID rejection or incomplete outcomes. Only validated
results award final entropy. Tests cover malformed options/data, status mapping,
incomplete IID row serialization, full-size success in both modes, and seeded
sanity rejection. The full-size subprocess tests are opt-in due to runtime.

Restart CLI verification passed 155 regular tests, twelve documentation examples,
and both opt-in restart subprocess tests (success in both modes and sanity
rejection), plus Clippy, formatting, and whitespace checks. External NIST
comparisons were not rerun for this command-interface addition.

## Conditioning CLI

The `conditioning` command exposes raw block dimensions and primary input
entropy, explicit vetted/non-vetted mode, assessed output entropy per bit,
additional-source accounting, retained output length, and precision controls.
It accepts no file and does not assess output data or establish vetted status.
Reports retain supplied/effective decimal inputs, probability bounds, caps,
working precision, and pre/post-truncation entropy. Success exits 0; errors
exit 2 and withhold entropy. Subprocess tests compare both modes to the library
at 384-bit precision and cover zero entropy, human output, malformed options,
invalid entropy, dimension/precision limits, additional-input overflow, and
unsupported non-vetted combinations.

Verification passed 157 regular tests, twelve doctests, Clippy, formatting,
and whitespace checks. The 33 opt-in tests and external NIST comparisons were
not rerun for this CLI addition.

## End-to-end validation CLI

`validate` exposes the strict workflow with separate full sequential/restart
files, explicit raw width and submitter bound, optional IID claim, and optional
vetted/non-vetted conditioning. Non-vetted output is assessed from an unpacked
bit file with at least one million complete blocks. The API controls fallback,
evidence reuse, entropy bounds, minimum sample sizes, and withholding results.
Reports include file hashes, actual seeds, attempted IID evidence, intermediate
estimates, restart rejection reasons, conditioning accounting/caps, and units.
Incomplete IID execution leaves the selected track null. Optional stage progress
goes to stderr. No file selection or typed component aliases are exposed.

Subprocess tests cover input/flag errors and full-size unconditioned, vetted,
non-vetted fallback, successful IID, and restart rejection paths. The vetted
case checks positive submitter-limited entropy against the library's conditioning
calculation, including proportional truncation and additional-source accounting.
Unit tests cover incomplete evidence serialization and decision/exit mapping.

Verification passed 159 regular tests, twelve doctests, all three new opt-in
full-size validation CLI tests, Clippy, formatting, and whitespace checks.
External NIST comparisons were not rerun for this interface addition.

## Transpose CLI

The installed `transpose` utility reads a fixed 1000×1000 byte matrix, optionally
selected by absolute offset and sample count, and writes its column-major
sequence. Exclusive file creation prevents overwriting existing paths or aliases
of the input. It validates the selected size before creating output, reports
write failures, and returns versioned metadata with both hashes. It accepts all
byte values and makes no entropy claim. Subprocess tests check every matrix
position, round-trip recovery, subset/hash semantics, malformed input/options,
and preservation of existing files and symlink targets.

Verification passed 161 regular tests, twelve doctests, Clippy, formatting,
and whitespace checks. Opt-in and external NIST comparisons were not rerun.

## Reference verification — 2026-10-20

The full pinned harness completed on the M1 using GCC 16, with status
`passed_with_binary64_baseline_differences`. All 28 Rust reference tests passed
across seven targets: estimator (10), combined non-IID (3), chi-square (2),
permutation (2), IID (2), restart (7), and conditioning (2). Nine of the eleven
fresh NIST baseline comparisons passed directly; `biased-random-bytes` and
`ringOsc-nist` used only the documented binary64 allowance. There were no
unexpected failures. Build/test commands took approximately eleven minutes.

Local evidence: `target/reference-runs/20261020T175029Z-8835/report.json` and its
adjacent command logs. The reference source was pinned to
`87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289`. Rust comparisons used the freshly
built M1 reference; the baseline comparator separately checked fresh NIST results
against bundled `refdata`. This run does not replace strict extended-precision
Linux CI, nor add full end-to-end validation parity beyond the harness's existing
coverage. CLI subprocess coverage remains recorded separately above.

## Public API review — 2026-10-20

The crate-level guide now maps common tasks to public entry points, demonstrates
report handling and typed conditioning, and explains input representation,
units, precision and replay limits. All numerical workflows were already public;
no reference RNG, prepared-data cache, or estimator internals were exposed.

Additive conveniences include a re-export of `rug::Float`, borrowed
`eligible_entropy()` accessors on assessment/dataset reports, estimator lookup
by kind, and the public IID rejection predicate. Accessors withhold partial or
rejected results but do not waive sample-size warnings or certify a source.
Validation/restart/compression errors now preserve their underlying causes.
An existing broken rustdoc link was corrected. Existing diagnostic fields and
entry points remain available. Report mutation is caller-controlled; accessor
predicates are conveniences for returned reports, not invariant enforcement on
arbitrarily modified public fields.

CLI JSON remains a separate versioned presentation contract; library consumers
retain typed high-precision reports. Core algorithms, numerical tolerances and
seed behavior are unchanged. Final-revision CI and package verification must
be refreshed after these API additions before publishing.

Verification passed 164 regular tests, fourteen doctests, the opt-in successful
million-sample IID validation workflow, Clippy, formatting, and whitespace
checks. Rustdoc builds with warnings denied. Three new public-API integration
tests exercise partial/constant results, incomplete/rejected IID evidence,
estimator lookup, the Float re-export, and typed error chains; existing
validation tests also check eligible entropy on complete and rejected workflows.
The external reference harness was not rerun for these additive API changes.

## NIST provenance and attribution review — 2026-10-20

The source review identified reference-specific adaptations in prediction,
permutation, restart simulation and estimator/test conventions. The project now
explicitly acknowledges NIST and conservatively attributes the reference-informed
implementation, without claiming clean-room development. `docs/provenance.md`
records the pinned source map, modification dates from Git history, changes,
and the distinction between Rust implementations and external C++ adapters.

`LICENSE-NIST` preserves the complete pinned upstream README notice unchanged;
`LICENSE-MIT` preserves the project's original MIT text. `LICENSE` contains both
and a modification notice. Cargo now uses `license-file = "LICENSE"`, rather
than an MIT-only SPDX declaration or an approximate NIST identifier. Source
headers, README and the dependency inventory point to these notices. Blackman/
Vigna generator attribution and the external Cephes-header distinction are
recorded separately. This is a provenance record, not exhaustive authorship
reconstruction or a blanket legal-compliance certification.

Exact-text checks verified both original notices; all license/provenance files
are included in the 82-file package. Offline locked package verification built
successfully. Formatting and whitespace checks passed. Changes are licensing,
metadata, documentation and source comments only; algorithms were not changed.

## User documentation restructuring — 2026-10-20

The README is now a getting-started guide: build requirements, installation,
workflow selection, a compilable library example, practical CLI commands, input
formats, result units/statuses, verification limits and licensing. Detailed
algorithm/API notes moved to `docs/library-guide.md`, complete command/report
contracts to `docs/cli.md`, and harness instructions to
`docs/reference-testing.md`. Session-specific verification history stays in
this audit; the README links to revision-specific CI rather than narrating runs.

All local links and section anchors in the four user guides were checked. The
README Rust example compiled as a separate consumer with only a direct
`sp-800-90b` dependency. All three new guides are included in the Cargo package.
No numerical implementation or CLI behavior changed in this documentation step.
