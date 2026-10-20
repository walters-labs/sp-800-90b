# SP 800-90B coverage audit

Audit date: 2026-10-19. Scope: the current working tree, including uncommitted files, based on commit `3e1cd16de077bde2f804dc1b5a75cabea7e39b5b`. Reference implementation: local NIST checkout at `87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289`.

The core numerical engine is broadly implemented. The crate is **not yet a complete implementation of the entire publication or a command-compatible replacement for NIST's package**. The strict numerical validation workflow now applies the submitter bound and coordinates track selection, restart testing, and conditioning. Outstanding computational pieces include wider-symbol preprocessing and the remaining conditioning metadata/truncation support. Release evidence also needs expansion, especially successful IID restart runs and automated reference testing.

This is a coverage and integration audit, not a proof of algorithm correctness or a certification. “Implemented” below means an API and targeted tests exist; the evidence column defines the strength of that claim. No estimator or decision behavior was changed during this audit.

## Baselines

- [January 2018 publication](https://nvlpubs.nist.gov/nistpubs/SpecialPublications/nist.sp.800-90b.pdf).
- [NIST publication status](https://csrc.nist.gov/pubs/sp/800/90/b/final) and [May 2025 potential errata](https://csrc.nist.gov/files/pubs/sp/800/90/b/final/docs/sp800-90b_errata_potential_updates.pdf).
- [Pinned reference source](https://github.com/usnistgov/SP800-90B_EntropyAssessment/tree/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289). This audit does not claim parity with an unpinned future upstream revision.

The potential errata correct the binary goodness-of-fit critical value to 27.877 and the MCV example to approximately 0.6822 for the upper probability and 0.5518 bits for entropy. Our chi-square code evaluates the tail probability, and MCV calculates the formula rather than hardcoding those example numbers. Dedicated regressions identifying these errata would improve traceability; they are not currently named tests.

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
| §3.1.2 track selection | Partial | IID decisions and both assessment tracks exist | Caller chooses track; no coordinated sequential/restart/conditioned evidence or automatic fallback |
| §3.1.3 initial entropy | Partial | Literal/bitstring combination exists | No `H_submitter` input or final bound application |
| §3.1.4 restart | Implemented with validation gaps | `restart.rs`; strict matrix size, simulated cutoff, both estimator tracks, transpose | Full reference test is one binary non-IID dataset at H_I=0; successful full IID restart reference parity not tested |
| §3.1.5 conditioning bounds | Formula implemented | `conditioning.rs`; directed rounding and eight reference cases | Caller supplies widths, h_in, h'; no vetted-component registry, output-truncation helper, or complete orchestration |
| §3.1.6 additional sources | Caller responsibility | No composition API | No enforcement that credited entropy/input accounting comes from the primary source |
| §4.4 RCT/APT | Implemented | `health.rs`; all ten Table 2 cutoffs, boundaries, chunking, latching, reset | Only byte-valued alphabets; optional opposite-symbol binary APT extension absent |
| §4.2–4.3 startup and on-demand operations | Partial | Startup sample counter, readiness, explicit reset | Device output gating, failure recovery, on-demand scheduling, and source-specific tests are not implemented |
| §6.4 symbol-space reduction | Missing | All assessment inputs are `&[u8]` | No wider-symbol extraction/ranking adapter; rank-remapping byte labels is not this procedure |
| Appendix E internal-width accounting | Partial | Conditioning clamps nw to n_in | Caller must establish component width |
| Appendix F CBC-MAC conditioner | Outside numerical-tool scope; absent | No cryptographic conditioning implementation | Needed only if scope expands to operating a conditioner, not merely assessing entropy |
| Appendices A–D and G | Explanatory/model material | Algorithms use the corresponding estimator concepts | No separate “appendix implementation” or proof claim |

Source links for the corresponding code are collected in [the public module exports](../src/lib.rs). Individual estimator reference coverage is in [nist_reference.rs](../tests/nist_reference.rs); orchestration has separate reference suites.

## NIST command parity

| Reference command | Rust equivalent | Missing command behavior |
| --- | --- | --- |
| `ea_non_iid` | `assess_non_iid`, `examples/non_iid.rs` | Installed binary, complete flag surface, JSON, subset selection, provenance and standardized exit statuses |
| `ea_iid` | `assess_iid`, `examples/iid.rs` | Same CLI/reporting gaps; examples are not automation-compatible replacements |
| `ea_restart` | `assess_restart`, `examples/restart.rs` | Same CLI/reporting gaps; Rust deliberately refuses validated IID entropy after rejected/unavailable checks |
| `ea_conditioning` | `conditioning_entropy`, `examples/conditioning.rs` | File-to-h' orchestration, reference epsilon diagnostics and full-entropy eligibility messages, JSON |
| `ea_transpose` | `restart_column_sequence` | File-writing command and block selection |
| Reference self-test runner | Optional Rust integration tests and C++ adapters | One reproducible command that builds the pinned reference/adapters and runs the entire compatibility suite |

There is no `src/main.rs`, `src/bin/`, explicit `[[bin]]`, or serialization dependency. Example programs return successfully after printing some rejected assessments, so their process exit status must not be used as an assessment pass indicator. Reference CLI behavior and a safer CLI contract should be explicitly distinguished in a future design.

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

Fresh checks during this audit:

- `cargo test -q`: **130 passed, 0 failed, 23 ignored**, including the doctest summary.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.

The ignored tests are opt-in reference or expensive tests, not unexplained test failures. They were not all rerun during this audit. Earlier session runs demonstrated reference agreement, but the following limits come directly from the checked-in test assertions:

| Suite | What it checks | What it does not establish |
| --- | --- | --- |
| `nist_reference.rs` | Each estimator against bundled dataset outputs | Inputs beyond those fixtures; independent proof of formulas |
| `non_iid_reference.rs` | Eleven complete reports and selected mode/width/truncation cases | Universal preprocessing parity |
| `chi_square_reference.rs` | Degrees of freedom, scores, probabilities and decisions on eleven datasets | Near-threshold coverage for every grouping branch |
| `permutation_reference.rs` | Original statistics on eleven datasets; seeded counters on three 4096-sample prefixes | Full-million-sample seeded suite parity or all seed distributions |
| `iid_reference.rs` | LRS on ten datasets; entropy/chi-square/LRS-length comparison in both modes on one 4096-sample prefix | Combined test does not compare the native permutation verdict; native seed is uncontrolled |
| `restart_reference.rs` | Million-sample non-IID estimates on one binary fixture at H_I=0; IID incomplete-check propagation with an intentionally invalid permutation seed | Successful full IID restart, nonbinary non-IID restart parity, positive-H_I end-to-end validation |
| Restart simulator tests | Seeded 10,000-trial quantile parity at five H values; full five-million-trial sanity rejection | Distribution of empirical cutoff variation or positive-H_I successful reference assessment |
| `conditioning_reference.rs` | Eight vetted/non-vetted final outputs within tolerance | Full parameter domain, all near-full-entropy regimes or a formal rounding proof |
| `health.rs` tests | Published cutoff examples and streaming semantics | Device fault injection, operational failure response or arbitrary-parameter interval certification |

Reference tests depend on environment paths and some existing self-test outputs. They do not currently verify the checkout hash, compiler flags, or freshness of every fixture. No CI configuration, benchmark suite, fuzz target, or cross-platform test matrix is present. These are evidence gaps, not evidence that those platforms or inputs fail.

## Prioritized completion work

| Priority | Gap | Concrete completion criterion |
| --- | --- | --- |
| Done | **G1 Validation workflow and submitter bound** | `assess_validation` accepts H_submitter, checks dataset eligibility, coordinates one IID/non-IID track, restart and conditioning, and withholds final entropy on incomplete or rejected evidence. Exploratory APIs remain available; integration tests cover bounds, units and decisions. |
| Implemented; CI run pending | **G2 Reproducible reference harness and CI** | Pinned isolated builds, automatic adapters, compiler/precision metadata, fresh results and full reference CI are configured. Hosted Linux execution and named regressions for both potential errata remain to be verified/completed. |
| P1 | **G3 Restart validation evidence** | Add positive-H_I successful and failing end-to-end cases, nonbinary restart fixtures, and a successful IID restart with deterministic permutation control. Preserve strict rejection semantics. |
| P2 | **G4 Wider symbol support** | Add a §6.4 preprocessing API for caller-specified ranked bit positions, preserving sample boundaries and entropy accounting. Test >8-bit samples, invalid rankings, bit order, and effective entropy ceilings. Do not infer a physical ranking from frequency alone. |
| P2 | **G5 Conditioning integration** | Add typed component metadata or validated width presets, a completed-assessment-to-h' bridge, explicit input-block entropy accounting, and the vetted output-truncation adjustment. Test that per-bit and per-sample quantities cannot be confused. |
| P2 | **G6 CLI and stable reports** | Implement supported subcommands, file subsets, explicit pass/reject/incomplete statuses and exit codes, precision-preserving JSON, dataset hashes, seed/provenance records, and transpose output. Publish which flags match NIST. |
| P2 | **G7 Numerical and robustness coverage** | Add independent golden vectors, rounding-bound tests for conditioning, tail minimality/boundary checks for health cutoffs, and property/fuzz tests for preprocessing and state machines. Benchmark time and peak memory on representative million-sample inputs. |
| P2 | **G8 Release preparation** | Establish minimum Rust/platform support, review public API stability, add package metadata and reproducible examples, and inventory redistribution notices. Cargo metadata identifies rug and gmp-mpfr-sys as LGPL-3.0+; the crate's MIT declaration is not a complete dependency notice inventory. |
| Scope decision | **G9 Operational source integration** | If this becomes a device-facing source library, design GetNoise/GetEntropy/HealthTest interfaces, startup withholding, on-demand operation and recovery. Otherwise document those responsibilities as outside the assessment crate. |

G1 is implemented by `assess_validation` for byte-valued raw samples and unpacked conditioned output bits. It enforces dataset sizes, consistent track selection, H_submitter, complete evidence, restart acceptance, and conditioning block units. G2 now has a pinned build/comparison harness and Linux CI configuration (hosted CI execution is still pending); G3 should precede a claim that the full restart workflow is reference-validated. A unified CLI is useful after the report semantics are stable.

## Requirements that remain external

Physical noise models, restart procedures, environmental characterization, collection provenance, evidence for IID assumptions, vetted-component assurance, and source-specific failure analysis remain responsibilities of a source developer and validation process. A software API can collect and enforce the presence of that evidence, but cannot establish it from a byte file. Accordingly, completing the code roadmap would produce a more complete assessment implementation, not a claim of NIST certification.

## Validation workflow follow-up

The G1 implementation adds deterministic integration tests for submitter bounds, conditioning units, eligibility failures, restart rejection, incomplete IID evidence and IID fallback. The opt-in full-size IID success regression passed on 2026-10-20 in 46.34 seconds after internal restart evidence reuse and test-profile optimization of libbz2-rs-sys. It verifies three permutation assessments and reuse of both restart checks. This is a deterministic synthetic fixture, not reference parity. These extend the audit snapshot above; they do not establish reference parity for the complete workflow. G5 remains open for vetted algorithm metadata, truncation, and richer source-input accounting.

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
structured restart matrix. This expands G3 evidence but does not replace the
remaining nonbinary and successful positive-entropy full-workflow parity cases.

The complete local M1 harness run on 2026-10-20 passed all 22 reference-suite
tests. NIST's own baseline comparisons had nine clean passes and two datasets
with the eight explicitly allowed binary64 differences. The resulting status
was `passed_with_binary64_baseline_differences`; this is not a clean upstream
self-test pass or evidence of a hosted Linux CI run.
