# CLI reference

[Getting started](../README.md) · [Library guide](library-guide.md) · [Reference testing](reference-testing.md)

Install from a checkout with `cargo install --path . --locked`, or use the Cargo
commands below from the repository. `samples.bin`, `sequential.bin` and other
unqualified filenames represent files you supply. Paths into the NIST checkout
require that separate checkout; fixtures are not bundled in this crate.
All commands support `--help` and `--json`. `--version` is a top-level option.

- [non-iid](#non-iid-command-line-interface)
- [iid](#iid-command-line-interface)
- [restart](#restart-command-line-interface)
- [conditioning](#conditioning-command-line-interface)
- [validate](#end-to-end-validation-command)
- [transpose](#transpose-utility)

## Non-IID command-line interface

```sh
cargo run --release -- non-iid samples.bin --bits-per-symbol 8 --mode initial
cargo run --release -- non-iid samples.bin --mode conditioned --bitstring-limit 1000000 --json
cargo run --release -- non-iid samples.bin --offset 1000 --samples 1000000 --precision 384
cargo run --release -- --help
```

Input is one symbol per byte; it is not a packed binary bitstream. Width is
inferred unless `--bits-per-symbol 1..8` is specified. Out-of-width values are
rejected rather than masked. `--mode` defaults to `initial`; `--confidence`
accepts `reference` (default) or `publication`. Precision defaults to 256 bits.

`--offset` is an absolute sample/byte offset. `--samples` reads at most that many
samples, stopping at EOF; the report gives the actual count. Selection happens
before width inference, hashing, and bit expansion. `--bitstring-limit` limits
only the expanded bit branch; the default uses all bits. Counts and bit limits
must be at least two. Use `--` before filenames beginning with a dash. Input
must be a regular file; stdin and device streams are not supported yet.

| Exit | Meaning |
| --- | --- |
| 0 | All applicable estimators completed (including constant, zero-entropy input) |
| 2 | Invalid command/input, file I/O failure, or output I/O failure |
| 3 | At least one applicable estimator was unavailable; only partial estimates exist |

Non-IID estimation has no hypothesis-test rejection verdict. Completion does
not mean source validation or adequate sample size; fewer than one million
samples produce a warning. This is an explicit CLI contract, not a clone of
NIST's flags or exit codes. In particular, `--offset` is not NIST's `-l` block
index. Each command has its own supported options, documented below.

### JSON report version 1

`--json` emits one JSON object to stdout. Successful/incomplete reports include:

- `schema_version`, `tool` (name/version), `command`, and `status`.
- `input`: display path, encoding, absolute offset, requested/actual counts,
  and `selected_samples_sha256` over exactly the selected bytes.
- `options`: mode, declared width, precision, confidence convention, bit limit.
- Effective `bits_per_symbol`, original sorted `alphabet`, and `entropy_units`.
- `min_entropy`: decimal string for complete assessments, otherwise null.
- `minimum_available_entropy`: decimal string retaining the partial minimum.
- `literal` and `bitstring`: null when not assessed, otherwise sample count,
  units, completeness, both minima, and per-estimator outcome summaries.
- `warnings`: structured codes and associated counts or widths.

Estimator statuses are `available`, `not_applicable`, and `unavailable`.
Unavailable outcomes carry an error code and message. Entropy values are strings
so JSON consumers do not silently round MPFR results to binary64; parse them with
an arbitrary-precision decimal library if preserving precision matters. Counts
are JSON integers (consumers must likewise handle large integers appropriately).
Top-level entropy always uses bits per input symbol; bitstring branch entropy
uses bits per bit. Full intermediate estimator diagnostics remain in the Rust
API and are not serialized by schema version 1. There is no random
seed field: the non-IID estimators here are deterministic.

Usage/input errors with `--json` emit an error object containing schema/tool/
command, `status: "error"`, `min_entropy: null`, and `error.code`/`error.message`.
Error messages are diagnostic text, not a stable parsing interface. Output-write
failures cannot guarantee a complete JSON document. Help/version print plain
text. Paths are displayed with lossy UTF-8 conversion if necessary; the selected
byte hash identifies the assessed content. The CLI does not hash unselected file
contents or certify their provenance.


## IID command-line interface

```sh
cargo run --release -- iid ../SP800-90B_EntropyAssessment/bin/truerand_4bit.bin --bits-per-symbol 4 --seed 1,2,3,4 --json
```

`iid` accepts the same file, subset, width, precision, mode, confidence and
bitstring-limit options as `non-iid`. Both modes test the full selected literal
sample sequence: `--mode conditioned` and `--bitstring-limit` affect entropy
estimation only, never shorten the IID hypothesis tests. Small datasets remain
exploratory and carry warnings.

`--seed A,B,C,D` takes four decimal unsigned 64-bit words; all zero is invalid.
Omitting it obtains OS randomness. A completed permutation check records the
actual seed as four decimal **strings** for exact replay, even in JSON clients
whose numeric type cannot represent all u64 values. The requested seed appears
in `options.requested_seed` (null for OS seeding). Serial reference permutation
arithmetic, early stopping, and reference chi-square grouping are used. No
non-IID fallback runs automatically.

| IID status | Exit | Final `min_entropy` |
| --- | --- | --- |
| `not_rejected` | 0 | Decimal string; all checks completed without rejection |
| `rejected` | 1 | null; at least one rejection or constant input |
| `incomplete` | 3 | null; no rejection observed, but a check could not run |
| `error` | 2 | null; invalid input/options or I/O failure |

`candidate_min_entropy` retains the MCV-derived estimate in bits per input
symbol in every assessment report. It is diagnostic, not eligible entropy after
rejection or incompleteness. `checks_complete` reports whether all four checks
executed; a rejection takes precedence even if another check is unavailable.
Constant input rejects with zero candidate entropy, `checks: null`, and
`checks_complete: false`. Non-rejection does not establish physical IID behavior
or complete source validation.

Schema version 1 IID reports share the input hash, options, warnings, alphabet,
and width fields with non-IID reports. `literal`/`bitstring` contain MCV summaries
rather than lists of non-IID estimators. `checks` contains independence,
goodness-of-fit, longest-repeated-substring, and permutation results. Each has a
`not_rejected`, `rejected`, or `unavailable` status; unavailable checks carry
errors. Statistics, probabilities and entropy use decimal strings. LRS pair
counts are also strings to preserve u128 values. Permutation results include
the seed, rounds performed, all 19 statistic values, tail counts and decisions.
Human-readable output includes check decisions, p-values, permutation seed,
candidate/eligible entropy, warnings and unavailable-check errors.


## Restart command-line interface

```sh
cargo run --release -- restart ../SP800-90B_EntropyAssessment/bin/truerand_4bit.bin --bits-per-symbol 4 --initial-entropy 2 --simulation-seed 1,2,3,4 --json
# To run IID restart checks, add: --mode iid --seed 1,2,3,4
```

The selected input must contain exactly 1,000,000 symbols arranged as a 1000×1000
row-major restart matrix, one symbol per byte. `--offset` and `--samples` select
a matrix from a larger file; omission selects the whole file. A differently
sized selection or constant input is an error. Reshaping an ordinary sample file
is useful for software testing but does not supply physical restart evidence.

`--initial-entropy H` is required, in bits per sample, finite and nonnegative.
It is parsed directly into MPFR with downward rounding at the requested
precision, avoiding binary64 conversion. JSON records both the supplied string
and effective value. Library checks enforce width/alphabet constraints.
`--mode` accepts `non-iid` (default) or `iid`; initial/conditioned modes and
`--bitstring-limit` do not apply. Width, precision, and confidence options work
as in the other commands.

`--simulation-seed` controls sanity simulation; `--seed` controls permutation
tests and is accepted only in IID mode. Both take four decimal u64 words and
reject all-zero states. Omitted seeds use OS randomness. `--simulations` defaults
to 5,000,000 and cannot be smaller. Full restart tests are expensive; optimized
builds are recommended. JSON is emitted after completion.

Statuses are `validated` (exit 0), `sanity_rejected`, `entropy_rejected`, or
`iid_rejected` (exit 1), `incomplete` (exit 3), and `error` (exit 2).
Only validated assessments have a final `min_entropy`. This validates the restart
numerical checks, not the complete physical source. Row/column diagnostics remain
available after entropy or IID rejection; sanity rejection skips both datasets.

Version 1 JSON includes the selected-input hash and matrix layout, options,
initial entropy, actual sanity seed, simulation rounds, cutoff, all row/column
maximum frequencies, and final result. Sanity probability and alpha are decimal
strings representing the reference-compatible binary64 simulation arithmetic.
Row/column objects contain non-IID estimator summaries or IID MCV estimates and
all four checks. IID candidate entropy stays separate from eligible entropy.
Actual permutation seeds are recorded separately for rows and columns; a
supplied `--seed` initializes each assessment with that same seed. Seed words
are decimal strings. A fixed sanity seed plus the recorded row/column seeds
makes the individual computations reproducible.

## Conditioning command-line interface

Calculate a bound for a caller-asserted vetted component with a 512-bit primary
input block containing 400 bits of entropy, a 256-bit full output, and a
256-bit narrowest internal width:

```sh
cargo run --release -- conditioning --mode vetted \
  --input-bits 512 --input-entropy 400 \
  --output-bits 256 --internal-width 256 --json
```

This command performs the entropy calculation; it does not execute a conditioner
or establish that an implementation is vetted. Dimensions are explicit, as in
the reference calculation interface. There is no input file. For non-vetted
conditioning, use `--mode non-vetted --assessed-entropy-per-bit 0.75`, replacing
0.75 with the independently assessed output entropy per bit. The result applies
both the non-vetted cap and the assessed output cap.

Vetted mode also accepts `--additional-input-bits N` for concatenated additional
sources (no entropy credit) and `--retained-output-bits N` for proportional
truncation of the full output bound. `--input-bits` and `--input-entropy` always
refer only to the primary source. Non-vetted mode disallows additional sources
and truncation through the same validation rules as the library.

`--precision` defaults to 256 bits; `--max-working-precision` defaults to 1048576
bits and bounds the library's automatic working precision. Entropy inputs are
parsed directly into MPFR with downward rounding, without binary64 conversion.
JSON records the supplied decimals and effective values, physical input
accounting, dimensions, working precision, probability bounds, caps, and entropy
before and after truncation. Entropy values are decimal strings; final
`min_entropy` is in **bits per delivered output block**.

A completed calculation exits 0 with status `complete`. Invalid arguments or
inputs exit 2 with status `error` and null `min_entropy`. Completion does not
assert full-entropy eligibility or complete entropy-source validation.

## End-to-end validation command

`validate` runs the strict numerical workflow over a sequential dataset and a
separate restart matrix. For a software smoke test using the reference fixture:

```sh
cargo run --release -- validate \
  --sequential ../SP800-90B_EntropyAssessment/bin/truerand_4bit.bin \
  --restart ../SP800-90B_EntropyAssessment/bin/truerand_4bit.bin \
  --bits-per-symbol 4 --submitter-entropy 2 \
  --simulation-seed 1,2,3,4 --progress --json
```

Reusing that fixture exercises the software only. A real assessment requires
separately collected sequential samples and restart data, together with the
source's physical/design evidence. The sequential file must contain at least
one million samples; the restart file must contain exactly one million samples
in a 1000×1000 row-major matrix. Both use one symbol per byte and the explicitly
required `--bits-per-symbol` width. Files are read in full; this command has no
subset options. `--submitter-entropy` is required, in bits per raw sample.

The default track is non-IID. Add `--iid-claim --seed 1,2,3,4` to assert an IID
rationale and attempt the IID checks. Statistical rejection selects non-IID for
the whole workflow; failed/incomplete checks withhold a result. Attempted IID
evidence is retained and labeled by stage. Successful restart IID evidence is
reused. The workflow applies the smaller of the sequential estimate and the
submitter bound before restart testing. Expanded bitstrings use the API's fixed
one-million-bit limit; reports retain truncation warnings.

To include a caller-asserted vetted conditioner, append, for example:

```sh
--conditioning vetted --samples-per-block 128 \
--output-bits 256 --internal-width 256
```

`--samples-per-block` counts primary raw samples, not bits or additional-source
input. The workflow derives the primary input width and entropy per block from
the raw sample width and validated raw entropy. Vetted conditioning supports
`--additional-input-bits` (no entropy credit) and `--retained-output-bits`.
Dimensions are explicit; the command does not run or vet a cryptographic algorithm.

For non-vetted conditioning, use `--conditioning non-vetted` with the same block
dimensions and `--conditioned-output FILE`. The output file must contain unpacked
bits (one byte containing 0 or 1 per bit), in generation order, with at least
one million complete output blocks. The workflow assesses this file to obtain
the output cap; it does not accept a manually supplied per-bit assessment.

`--precision`, `--confidence`, `--simulation-seed`, and `--simulations` behave as
in `restart`. `--seed` requires `--iid-claim`. An omitted permutation seed is
generated once and reused across the workflow; actual seeds are retained for
replay. `--max-working-precision` controls the optional conditioning calculation.
Submitter entropy is parsed directly into MPFR with downward rounding.

JSON includes file hashes, effective options, selected track, fallback evidence,
sequential/conditioned estimates, restart diagnostics, conditioning bounds, and
final units. `track` is null if incomplete IID evidence prevented selection.
`raw_entropy` is in bits per raw sample. Final `min_entropy` is in bits per
raw sample without conditioning, or bits per delivered output block with it.
Both final values are withheld unless the entire requested workflow completes.
Intermediate estimates remain diagnostic and are not final entropy awards.

Exit codes are 0 (`complete`), 1 (`restart_rejected`), 3 (`incomplete`), and
2 (`error`). A successful IID-to-non-IID fallback can still exit 0. `--progress`
prints stage starts and elapsed times to stderr, keeping stdout valid JSON.
Completion describes numerical evidence, not source certification.

## Transpose utility

Write the column-major sequence of a 1000×1000 row-major restart matrix:

```sh
cargo run --release -- transpose \
  --input ../SP800-90B_EntropyAssessment/bin/truerand_4bit.bin \
  --output truerand_4bit.columns.bin --json
```

Input uses one symbol per byte; all byte values are preserved. The selected
input must contain exactly 1,000,000 bytes. By default, the whole file is
selected. For a matrix embedded in a larger file, use `--offset N --samples
1000000`, where N is an absolute byte/sample offset, not a block index. With
only `--offset`, the remainder of the file must have the exact matrix length.
The output contains precisely one million bytes, with
`output[column * 1000 + row] = input[row * 1000 + column]`. Transposing twice
recovers the original matrix.

`--output` is required and must name a new file. Existing files, including
symlinks and the input itself, are never overwritten. Input dimensions are
checked before creating the output. A write failure exits with an error and may
leave a partial newly created file; it is never reported as complete.

Without `--json`, the command prints the output path and SHA-256 hash. Version 1
JSON includes dimensions, selection details, input/output hashes, and layout.
Exit 0 means the file was written successfully; exit 2 means usage, input, or
output failure. This utility rearranges bytes and does not assess entropy.
`restart` and `validate` already perform their own internal transposition, so
this utility is not a prerequisite for those commands.
