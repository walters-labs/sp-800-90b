# Source provenance and modification record

Reviewed 2026-10-20 against this project's Git history and the pinned reference
source. This is a source attribution record, not a claim that matching a
mathematical formula alone establishes copying, or a line-by-line authorship
certificate. The first implementation commit (`3e1cd16`) contains the estimator implementation together,
so Git history does not reconstruct every development step. We conservatively
acknowledge the reference-informed numerical implementation as adapted work,
rather than claiming an independent clean-room implementation.

## Upstream source and terms

We explicitly acknowledge the **National Institute of Standards and Technology
(NIST)** as the source of the reference software:

- Repository: [SP800-90B_EntropyAssessment](https://github.com/usnistgov/SP800-90B_EntropyAssessment).
- Reviewed revision: `87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289`.
- [Exact upstream notice](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/README.md#license), preserved without textual changes in [LICENSE-NIST](../LICENSE-NIST).
- Original project contributions: [LICENSE-MIT](../LICENSE-MIT).
- Combined packaged terms and modification notice: [LICENSE](../LICENSE).

Cargo uses `license-file = "LICENSE"` because the precise upstream notice is
preserved alongside the MIT grant. It is not an assertion that recipients can
choose MIT instead of retaining the NIST notice. Dependency licenses remain
separate; see [THIRD_PARTY.md](../THIRD_PARTY.md).

## Implementation map

Upstream paths below are relative to the pinned repository's `cpp/` directory.
The mapping records reference sources and observable implementation conventions;
it does not imply that whole files were copied verbatim.

| Rust files | Reference source | Adaptation and differences |
| --- | --- | --- |
| `most_common_value.rs`, `collision.rs`, `markov.rs` | `shared/most_common.h`, `non_iid/collision_test.h`, `non_iid/markov_test.h` | Estimator formulas, confidence/boundary conventions; Rust validation, reports and MPFR calculations |
| `compression.rs` | `non_iid/compression_test.h` | Six-bit block estimator and reference boundary/correction behavior; MPFR evaluation and Rust search implementation |
| `multi_mcw.rs`, `lag.rs`, `multi_mmc.rs`, `lz78y.rs` | Corresponding `non_iid/*_test.h` files | Predictor scoring, tie/run/dictionary behavior; Rust containers and parameter validation |
| `prediction.rs` | `shared/utils.h`, especially `prediction_estimate_function` and prediction bounds | Reference's up-to-66-step recurrence; MPFR calculations and convergence handling rather than native `long double` |
| `t_tuple.rs`, `lrs.rs`, `iid.rs`, `tuples.rs` | `shared/lrs_test.h`, `iid_main.cpp` | Tuple/LRS and IID probability/decision concepts; Rust prefix-doubling suffix array and histogram aggregation instead of linking divsufsort, MPFR IID tail calculation |
| `chi_square.rs` | `iid/chi_square_tests.h` | Reference grouping/binning and test rules; MPFR integer/half-integer tail evaluation instead of the reference's Cephes routines |
| `permutation.rs` | `iid/permutation_tests.h`, `shared/utils.h` | Statistics, serial shuffle stream, early-stop tails and reference arithmetic; Rust compression backend and optional exact-count arithmetic |
| `restart.rs` | `restart_main.cpp`, `transpose_main.cpp`, `shared/utils.h` | Matrix layout, simulated count distribution, cutoff and entropy rules; serial simulation with histogram quantiles, explicit rejection/incomplete reports |
| `non_iid.rs`, `iid.rs` | `non_iid_main.cpp`, `iid_main.cpp` | Alphabet mapping, bit expansion and assessment selection; checked Rust inputs and structured reports rather than C++ CLI behavior |
| `conditioning.rs` | `conditioning_main.cpp` and publication §3.1.5 | Output-entropy formulas; directed MPFR rounding, zero-entropy support, explicit caps, truncation and additional-source accounting |

The publication also directly informs these algorithms. `health.rs`,
`symbol_reduction.rs`, `vetted_conditioning.rs`, and the strict coordinator in
`validation.rs` implement publication concepts and project-specific APIs beyond
the reference tool's individual command interfaces. The CLI, report helpers and
Rust tests provide project-specific interfaces and checks. This distinction does
not remove upstream attribution from the numerical components they call.

## Nested upstream sources

The xoshiro256** transition in `permutation.rs` follows the implementation
credited in NIST's `shared/utils.h` to **David Blackman and Sebastiano Vigna**,
which that header identifies as public domain. The Rust source retains that
credit; restart simulation reuses the same generator. NIST's multiply-high
range sampler and descending Fisher–Yates shuffle determine the replay stream.

NIST's `iid/chi_square_tests.h` includes a separate Stephen L. Moshier/Cephes
copyright and BSD-style notice. The inspected Rust tail implementation uses
MPFR `erfc` and positive recurrences, not those Cephes code paths or coefficient
tables. The external C++ adapters do compile against that header: its notices
remain in the exported reference source and must accompany any distribution of
those reference tools as applicable. The Rust crate does not package the
reference headers, Cephes implementation, or compiled adapters.

## Test adapters and harness

`tests/reference/chi_square_independence.cpp`, `iid_lrs.cpp`, `permutation.cpp`
and `restart.cpp` are project wrappers around external reference headers/source,
not standalone copies of the NIST implementations. They add machine-readable
output, input preparation and deterministic serial controls. In particular,
`restart.cpp` renames the reference entry point and substitutes the seed at the
boundary for reproducible full-workflow tests; the original simulator and test
functions remain external.

`scripts/reference_harness.py` exports the pinned commit with its README and
notices intact into an isolated build directory. It compiles external tools and
these adapters, then produces fresh comparison results. Exported source,
datasets, binaries and logs are excluded from the published crate. This record
does not authorize dropping notices when distributing those external artifacts.

## Modification dates from project Git history

| Date | Project revisions | Nature of changes |
| --- | --- | --- |
| 2026-10-19 | `3e1cd16` | Rust estimator and IID implementations, high-precision arithmetic, predictor/reference conventions, reports and tests |
| 2026-10-20 | `1b29a5d`, `94b21af` | Restart, conditioning, health and strict numerical workflows; isolated reference harness and deterministic adapter coverage |
| 2026-10-20 | `050409b`, `1cc5011`, `857299b` | Ranked symbol reduction, proportional vetted-output truncation and typed conditioner dimensions |
| 2026-10-20 | `123815e`, `b14f502`, `fd601e1` | Additional-source accounting, six CLI commands, packaging documentation and public API conveniences |
| 2026-10-20 | This provenance review | Preserved upstream notice, combined license file, source attribution pointers and this record; no numerical behavior changes |

Dates above are recorded commit dates, not an assertion about the exact time
each line was originally written. Subsequent algorithm adaptations should update
this record. The MIT copyright notice identifies original contributions; it
does not assign ownership of NIST or other upstream material to this project.
