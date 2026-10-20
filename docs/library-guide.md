# Library guide and algorithm details

[Getting started](../README.md) · [CLI reference](cli.md) · [Reference testing](reference-testing.md)

This guide collects API examples, numerical conventions, algorithm details and
test coverage. For a map of public entry points, run `cargo doc --open --no-deps`.
Examples using `?` assume a surrounding function returning a compatible `Result`;
rustdoc-style lines beginning with `#` supply example setup or return values.

- [Non-IID assessment](#complete-non-iid-assessment-api)
- [IID assessment](#combined-iid-assessment)
- [Restart testing](#restart-testing)
- [Conditioning](#conditioning-entropy), [typed components](#typed-vetted-conditioning-components), [truncation](#truncating-vetted-conditioning-output), [additional sources](#additional-noise-source-input-accounting-316)
- [Streaming health tests](#continuous-health-tests)
- [Embedded health monitoring](#embedded-health-monitoring)
- [Strict validation workflow](#strict-numerical-validation-workflow)
- [Wider symbols](#reducing-wider-samples-64)

## Precision and reference compatibility

Assessment arithmetic uses `rug::Float` backed by MPFR, with **256-bit significand precision**
by default on all architectures, including Apple Silicon. Counts are integers;
probability, square-root, and logarithm calculations remain high precision.
The result is not narrowed to `f64` unless the caller explicitly requests it.
`most_common_value_with_options` accepts a precision of at least 64 bits and up
to MPFR's supported maximum. More precision costs time and memory; there is no
single "best possible" finite precision. Operations round to nearest; this is
not a certified interval enclosure or an exact-arithmetic result.

For MCV, the default `ConfidenceConstant::NistReference` preserves the exact binary64
constant used by NIST's MCV implementation (`2.5758293035489008`). Extra precision
improves subsequent arithmetic, not the accuracy of that fixed constant.
`ConfidenceConstant::Publication` instead uses decimal `2.576` from the paper.
The two intentionally produce different results. High-precision arithmetic also
means the default need not be bit-for-bit identical to NIST's binary64 operations.

Baseline: NIST reference commit `87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289`.

- [Publication](https://nvlpubs.nist.gov/nistpubs/SpecialPublications/nist.sp.800-90b.pdf)
- [NIST MCV implementation](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/shared/most_common.h)
- [NIST confidence constants](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/shared/utils.h)

## Collision estimator

`collision_estimate(&[u8])` accepts only 0 and 1, one sample per byte.
`collision_estimate_with_options` takes `CollisionOptions` (the same settings
as `McvOptions`). At least two completed collisions are required; an incomplete
trailing group is ignored. The result retains the collision count, consumed
sample count, mean, sample standard deviation, raw and clamped confidence
bounds, estimated probability, and entropy in bits per sample.

Like the pinned NIST reference, the implementation clamps the adjusted mean
to at least 2 and solves `E[T] = 2 + 2p(1-p)` directly. A bound of 2 gives
zero entropy; a bound at or above 2.5 gives one bit per sample. This differs
from applying the publication's no-solution fallback to a bound below 2.
Selecting the publication confidence constant changes only the multiplier,
not this reference-compatible boundary policy.

The collision variance uses `n2*n3 / (v*(v-1))` for counts of length-2 and
length-3 collisions. This avoids subtracting nearly equal second moments.
All arithmetic after counting uses the selected MPFR precision.

Reference: [NIST collision implementation](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/non_iid/collision_test.h).

## Markov estimator

`markov_estimate(&[u8])` implements §6.3.3 for binary samples, with at least
two samples required. `markov_estimate_with_options` accepts `MarkovOptions`
with a configurable `precision_bits` (default 256); no confidence constant is
used in this estimator.

The result exposes symbol counts, overlapping transition counts, initial
probabilities, transition probabilities, and the negative log2 probabilities
of the six candidate sequences in the specification's order. Unobserved
transition rows are zero; impossible candidates are represented by `None`.
Arithmetic uses logarithms rather than products to avoid probability underflow.
The minimum candidate entropy is divided by 128 and capped at one bit/sample;
if every candidate is impossible, the result is one, matching NIST.

This follows NIST's six-pattern table, not a general optimization over all
128-bit Markov paths. The two calculations can differ on some inputs.
Tests independently multiply transitions along those six explicit sequences
for all 256 eight-bit inputs, and check the publication example at high precision.

Reference: [NIST Markov implementation](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/non_iid/markov_test.h).

## Compression estimator

`compression_estimate(&[u8])` implements §6.3.4 with six-bit blocks and a
1000-block training dictionary. At least two subsequent test blocks are
required (6012 bits total). Partial trailing blocks are ignored, but nonbinary
values anywhere in the input are rejected.

`CompressionOptions` controls precision (default 256 bits), confidence constant,
and dictionary length. The dictionary override supports educational examples;
the standard specifies 1000. Results include block counts, mean log-recurrence
distance, corrected standard deviation, confidence bound, probability, and
entropy in bits per input bit.

The standard deviation follows the specific expression in step 5, including
its 0.5907 factor, rather than ordinary sample variance. Reference mode preserves
the C++ binary64 values of both numerical constants; Publication mode uses
exact decimal 0.5907 and 2.576. All subsequent arithmetic uses MPFR.

The finite double sum for G is rearranged into a linear sum and logarithms are
cached across bisection iterations. Summation may stop when a geometric bound
on the remaining terms is below `2^(-precision_bits-16)`; this bounds truncation,
not total floating-point error. Bisection stops when the endpoints are adjacent
at the requested precision, selecting the upper probability endpoint. Thus the
search does not silently stop at binary64 accuracy.

A nonpositive adjusted mean returns zero entropy (the endpoint approached by
the reference implementation). A mean at or above the uniform model's value
returns one bit per input bit. These reference boundary conventions apply in
both constant modes. Inputs with fewer than two test blocks return an error
instead of attempting an undefined variance calculation.

Reference: [NIST Compression implementation](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/non_iid/compression_test.h).

## t-Tuple estimator

`t_tuple_estimate(&[u8])` implements §6.3.5 for byte-valued symbols, with
**overlapping** tuples and the standard occurrence cutoff of 35. It returns
an error when no tuple qualifies. `TTupleOptions` configures precision,
confidence constant, and cutoff (at least 2); the cutoff override supports
the paper's example, which uses 3. Results include the selected maximum
length, maximum counts for every qualifying length, probability bounds,
and entropy in bits per sample.

Tuple counts are exact integers. A prefix-doubling suffix array, Kasai LCP
array, and monotone stack compute all maximum frequencies in O(n log n)
time and O(n) memory. There is no fixed maximum tuple length, no hashing
collision risk, and no allocation proportional to the number of possible
tuples. Long nearly constant inputs still require linear-size arrays.

NIST uses its longer `ZALPHA_L` constant here. Reference mode parses
`2.575829303548900384158` directly at the selected MPFR precision rather
than first rounding it to this machine's `long double` or `f64`. Publication
mode uses decimal `2.576`. This preserves the reference's stated decimal
constant, not the infinite-precision normal quantile. As with the other
estimators, MPFR results need not match native floating-point bits exactly.

Tests cover the paper's ternary example, overlapping runs beyond length 8,
the inclusive cutoff, invalid inputs, symbol relabeling, and precision stability.
The counting algorithm is checked against brute-force tuple counts on all
binary sequences of lengths 2 through 9 and on a full-byte-alphabet sequence.
The optional NIST integration test checks both literal and bitstring outputs
and selected tuple lengths on all 11 datasets.

Reference: [NIST t-Tuple implementation](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/shared/lrs_test.h).

## Longest Repeated Substring estimator

`lrs_estimate(&[u8])` implements §6.3.6 for byte-valued symbols. It considers
lengths from `u`, the first whose maximum occurrence count is below 35, through
`v`, the longest repeated substring. Overlapping occurrences count. For each
length, it sums `choose(count, 2)` over distinct tuples and divides by the
number of possible pairs of starting positions. The maximum length-normalized
probability receives the standard confidence correction.

`LrsOptions` configures MPFR precision (default 256 bits), confidence constant,
and cutoff (default 35). The numerical constants follow t-Tuple's conventions.
Results expose `min_length` (u), `max_length` (v), exact `u128` pair counts for
lengths u through v, probability bounds, and entropy in bits per sample.
An empty range returns `NoRepeatedSubstringRange` instead of NIST's negative
sentinel; fewer than two samples return an error.

LRS shares the suffix-array machinery with t-Tuple. A monotone stack accumulates
pair counts across substring-length intervals in O(n log n) total time and
O(n) memory, without enumerating all substrings. Tests compare counts with
brute-force enumeration, check the paper's cutoff-3 example against an independent
100-digit Decimal calculation, and cover constant inputs, empty ranges, symbol
relabeling, and precision stability. The optional reference test checks u, v,
and entropy for literal and bitstring inputs on all 11 NIST datasets.

Reference: [NIST LRS implementation](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/shared/lrs_test.h).

## Multi Most Common in Window predictor

`multi_mcw_estimate(&[u8])` implements §6.3.7 using windows of 63, 255, 1023,
and 4095 symbols. It requires at least 4096 samples, matching NIST's input
boundary. Predictions start after the first 63 symbols; larger windows become
eligible as enough history becomes available. Frequency ties favor the most
recent symbol. Scoreboard updates follow NIST's order, including ties.
The alphabet size is the number of distinct input bytes.

`MultiMcwOptions` configures precision (default 256 bits), confidence constant,
and four positive, strictly increasing window sizes. Window overrides support
the publication's small example. Results include prediction and success counts,
the longest successful run, final window scores, global probability and its
upper confidence bound, the optional local bound, and entropy in bits per symbol.
`p_local` is `None` when it cannot affect the result. The final probability is
at least `1 / alphabet_size`. Zero successful predictions use the special
`1 - 0.01^(1/N)` confidence bound.

The shared predictor arithmetic evaluates the local run equation in logarithms
and bisects until the probability endpoints are adjacent at the selected MPFR
precision. Like the pinned C++ implementation, its internal fixed-point
recurrence uses at most 66 iterations (stopping early at equality); the
publication uses ten. This is a retained numerical approximation, not a claim
of an exact run distribution. `Publication` changes the confidence multiplier
only; it does not switch the recurrence algorithm. Counts are exact and MPFR
avoids native `long double` precision differences, but C++ predictor results
can differ slightly due to cancellation and its search stopping criteria.

Tests check the publication example, independent window reconstruction,
activation boundaries, changing modes, ties, zero successes, constant inputs,
symbol relabeling, invalid settings, and high-precision Decimal local-bound
oracles. The optional NIST comparison checks exact C, N, and r for literal and
bitstring data on all 11 datasets, with entropy tolerance `1e-12` for global-bound results and `1e-9` when the
local bound controls the result. On this Apple Silicon reference build, the
largest observed discrepancy was `3.21e-10`; independent 100-digit Decimal
calculations confirm the MPFR results in both discrepant local-bound cases.

Reference: [NIST MultiMCW](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/non_iid/multi_mcw_test.h)
and [shared predictor arithmetic](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/shared/utils.h).

## Lag predictor

`lag_estimate(&[u8])` implements §6.3.8 with lags 1 through 128. Each predictor
uses the symbol at its lag as its next guess. Predictions start at the second
sample; unavailable lags cannot score. The chosen predictor is evaluated before
scores are updated. Successful predictors update in increasing lag order and
can replace the winner on a tied score, matching NIST.

`LagOptions` configures precision (default 256 bits), confidence constant, and
`max_lag` (default 128, positive). At least three samples are required to leave
two predictions for the confidence calculation, consistent with the reference's
length requirement. Constant input is accepted and returns zero entropy.
The alphabet is inferred from distinct input symbols. Results expose N, C,
the longest correct run, final scores, probability bounds, and entropy. Scores
cover lags 1 through `min(max_lag, sample_count - 1)`; longer lags never activate.

Per-symbol position queues update only successful predictors, with worst-case
O(n D) time and O(k D) storage for effective maximum lag D and alphabet size k.
Stale positions are discarded before scoring. The global/local probability
calculation is shared with MultiMCW, including its 66-iteration recurrence,
MPFR bisection, zero-success branch, and alphabet floor.

Tests cover the publication's three-lag example, direct-prediction comparisons,
queue expiration, constant and periodic input, all 256 symbols, invalid settings,
relabeling, and precision stability. The optional NIST test compares exact N, C,
and r on all 11 datasets, including bitstrings. Entropy tolerances are the same
as MultiMCW: `1e-12` for global-bound results and `1e-9` for local-bound results.

Reference: [NIST Lag implementation](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/non_iid/lag_test.h).

## MultiMMC predictor

`multi_mmc_estimate(&[u8])` implements §6.3.9 with context depths 1 through 16
and a limit of 100,000 distinct (context, successor) pairs per depth. Existing
pairs continue to accumulate counts after the limit is reached; new pairs do
not enter the model. Each model predicts the most frequent successor, breaking
ties toward the greatest symbol. Scoreboard ties follow NIST's update order.
Consequently, arbitrary symbol relabeling can change predictions; order-preserving
relabeling does not.

`MultiMmcOptions` controls precision (default 256 bits), confidence constant,
maximum depth (1 through 16), and positive per-depth entry limit. Four samples
are required to leave at least two predictions. Constant input returns zero
entropy. The alphabet is inferred from distinct input bytes. Results expose
N, C, the reference run counter, scores, stored pair counts, probability bounds,
and entropy. Scores and entry counts cover depths through
`min(max_depth, sample_count - 2)`. Entry counts include the final training update.

This implementation deliberately preserves a reference-code quirk: an unavailable
winning prediction leaves the run counter unchanged, while an incorrect available
prediction resets it. Thus `longest_correct_run` may differ from the longest
strictly consecutive run of correct predictions described in the publication,
particularly when dictionaries saturate. The `Publication` confidence constant
does not change this policy. A targeted test demonstrates the difference.

Contexts use exact packed 128-bit keys in separate maps for each depth. Sparse
successor counts avoid allocating 256 counters for every context. Hash collisions
are resolved by full-key equality. Model initialization is queried only after
its initial transition is in the past. The global/local entropy calculation is
shared with MultiMCW and Lag, with the same precision and approximation policies.

Tests cover the publication example, independent chronological model updates,
entry-limit saturation, successor ties, unavailable predictions, constant and
unique input, validation, order-preserving relabeling, and precision stability.
The optional NIST comparison checks exact N, C, and r for all 11 datasets, with
entropy tolerances of `1e-12` for global-bound and `1e-9` for local-bound results.

Reference: [NIST MultiMMC implementation](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/non_iid/multi_mmc_test.h).

## LZ78Y predictor

`lz78y_estimate(&[u8])` implements §6.3.10 with maximum context length B=16
and a global limit of 65,536 contexts. Each context tracks successor counts.
Within a context, ties favor the greater symbol; across contexts, the greatest
successor count wins, with longer contexts taking precedence on ties. An
incorrect or unavailable prediction resets the consecutive-success run.

Unlike MultiMMC, the capacity limit counts contexts rather than successor pairs.
Existing contexts always update and can acquire new successors after capacity
is reached. New contexts are inserted in descending length order. Prediction
uses counts from prior samples only; training with the current sample follows
its context's prediction. Packed context keys and masks avoid repeatedly copying
substrings. Hash collisions are resolved by full-key equality.

`Lz78yOptions` controls precision (default 256 bits), confidence constant,
`context_length` (1 through 16), and `max_dictionary_size` (at least B, so all
initial contexts fit). At least B+3 samples are required to leave two predictions;
that is 19 samples with defaults. Constant data is accepted and returns zero
entropy. The alphabet is inferred from distinct input bytes. Results expose N,
C, the longest correct run, final dictionary size, context counts by length,
probability bounds, and entropy in bits per symbol. Dictionary diagnostics
include the final training update, matching the reference's processing order.
The global/local probability calculation is shared with the other predictors.

Tests cover the publication example, an independent train-before-predict model,
both tie rules, saturated dictionaries learning new successors, unavailable
predictions resetting runs, all-byte input, validation, order-preserving
relabeling, and precision stability. The optional NIST comparison checks exact
N, C, and r for literal and bitstring inputs on all 11 datasets, using the same
`1e-12` global-bound and `1e-9` local-bound entropy tolerances as other predictors.

Reference: [NIST LZ78Y implementation](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/non_iid/lz78y_test.h).

All ten non-IID estimators in §6.3 are available individually and through the
combined API below. IID assessment, restart testing, and the strict numerical
validation workflow are also implemented; their APIs and CLI commands are
described in the sections that follow.

## Complete non-IID assessment API

```rust
use sp_800_90b::{assess_non_iid, NonIidOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let samples = std::fs::read("samples.bin")?; // one symbol per byte
    let report = assess_non_iid(&samples, NonIidOptions::default())?;
    println!("{} bits per input symbol", report.min_entropy);
    println!("All applicable estimators completed: {}", report.is_complete());
    for warning in &report.warnings {
        eprintln!("{warning:?}");
    }
    Ok(())
}
```

`NonIidOptions` supplies shared MPFR precision and confidence constant, with all
other estimator parameters fixed to their standard defaults. It also supports:

- `bits_per_symbol: None`: infer the width from the highest bit in use, with a
  minimum of one. An explicit `Some(bits)` with `bits` in `1..=8` includes declared leading zero
  bits; values that do not fit are rejected instead of silently masked.
- `mode: NonIidMode::Initial`: assess rank-mapped literal symbols. For alphabets
  larger than two, also assess the original bytes expanded MSB-first. A
  two-symbol source is mapped to 0/1 and assessed once, regardless of byte width.
- `mode: NonIidMode::Conditioned`: assess only the original raw bitstring.
- `bitstring_limit: None`: use all bits. `Some(1_000_000)` matches NIST's `-t`;
  other limits of at least two are supported. Literal input is never truncated.
  The limit has no effect when the initial two-symbol path skips bitstring testing.

For initial multivalued data, the final estimate is
`min(H_original, bits_per_symbol * H_bitstring)`, capped by the symbol width.
The two-symbol initial path uses `H_original` alone. Conditioned mode exposes
`h'` in `report.bitstring.as_ref().unwrap().min_entropy` (bits per bit), while
`report.min_entropy` remains in **bits per original input symbol**, matching
NIST's assessed-entropy output.

`literal` and `bitstring` contain per-dataset minima and all ten results in
specification order. `EstimatorOutcome::Available` retains boxed, typed
`EstimatorDetails`, including counts and high-precision intermediate values.
`NotApplicable` marks the three binary-only estimators on a nonbinary literal
stream. `Unavailable(EstimateError)` retains failures such as insufficient data
or no qualifying tuples. Those failures are excluded from the minimum;
**`is_complete()` is false whenever an applicable estimator is unavailable**.
A partial minimum must not be mistaken for a completed run. This safely handles
short inputs where some C++ paths would use a sentinel or undefined arithmetic.

Warnings identify fewer than one million input samples, declared widths wider
than observed values, and bitstring truncation. A constant input returns an
explicit zero-entropy result with `ConstantInput` and no estimator branches;
this implements NIST's no-entropy outcome without exiting the process.
Empty and single-sample inputs are rejected. `is_complete()` concerns estimator
availability only: it does not remove data-size warnings or establish source
validation. The selected options, original sorted alphabet, and data sizes are
included for interpreting the report.

A minimal file-based example is available:

```sh
cargo run --example non_iid -- /path/to/samples.bin 8
```

API tests cover raw-bit preparation before remapping, sparse two-symbol sources,
width validation and padding, truncation, partial results, constant data,
precision propagation, and aggregation. Full-report integration tests compare
all component estimates and final minima against the 11 NIST self-test outputs.
Additional live-executable comparisons cover conditioned mode, explicit widths,
and million-bit truncation:

```sh
NIST_REFERENCE_DIR=/path/to/SP800-90B_EntropyAssessment \
  cargo test --test non_iid_reference -- --ignored --test-threads=1
```

Component tolerances retain `1e-12` for non-predictors and `1e-9` for predictors;
final per-symbol comparisons allow `bits_per_symbol * 1e-9`. The test profile
optimizes this crate to keep full-data comparisons practical without changing
native dependency builds. No external NIST files are modified by these tests.

Reference: [NIST non-IID assessment driver](https://github.com/usnistgov/SP800-90B_EntropyAssessment/blob/87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289/cpp/non_iid_main.cpp).

## Chi-square independence

`chi_square_independence(&samples)` implements the binary block test (§5.2.3)
and nonbinary pair test (§5.2.1), choosing by the observed alphabet size.
It returns the statistic, degrees of freedom, upper-tail p-value, rejection
at significance level 0.001, and observed/expected counts with bin mappings.
Symbols are rank-mapped automatically. Incomplete trailing blocks are excluded
from observations but included in the symbol proportions.

```rust
use sp_800_90b::chi_square_independence;
let result = chi_square_independence(&[0, 1].repeat(500)).unwrap();
assert!(result.reject_independence);
```

`chi_square_independence_with_options` accepts `ChiSquareOptions` with
`precision_bits` (default 256) and `grouping`:

- `ChiSquareGrouping::NistReference` (default) preserves binary64 arithmetic
  for NIST's discrete block-length and binning decisions only.
- `ChiSquareGrouping::ExactCounts` uses exact count products and rational
  comparisons for those decisions. Mathematically equal expectations can sort
  differently from NIST's rounded values, changing bins and the statistic.

Both modes compute expected counts, the statistic, and the upper-tail probability
with MPFR. The probability uses the finite positive-term gamma recurrence for integer
degrees of freedom, so it avoids subtracting a CDF close to one. Results are not bit-for-bit copies
of NIST's binary64 calculations.

Constant input, inadequate binary counts, and nonpositive degrees of freedom
return explicit errors. In particular, the reference's zero-degree-of-freedom
p-value of one is not reported as a pass. A non-rejection does not establish IID;
the other IID checks are still needed.

Tests include the publication's Example 11 (using unrounded expectations),
boundary counts, tail handling, bin merging, and independent closed-form
probability checks. An optional comparison against all eleven NIST datasets uses
`tests/reference/chi_square_independence.cpp`, a thin adapter to the unmodified
NIST header. Compile it with the reference's include paths and dependency/linker
flags, then run:

```sh
NIST_REFERENCE_DIR=/path/to/SP800-90B_EntropyAssessment \
NIST_CHI_SQUARE_REFERENCE=/path/to/compiled-adapter \
cargo test --test chi_square_reference -- --ignored --nocapture
```

## Chi-square goodness of fit

`chi_square_goodness_of_fit(&samples)` compares symbol distributions across ten
consecutive blocks of `samples.len() / 10` samples (§5.2.2 and §5.2.4). Trailing
samples affect global proportions but are excluded from block observations,
matching NIST. The result exposes per-block counts, expected counts, symbol-to-bin
mapping, statistic, degrees of freedom, p-value, and `reject_goodness_of_fit`
(at p < 0.001).

```rust
use sp_800_90b::chi_square_goodness_of_fit;
let result = chi_square_goodness_of_fit(&[0, 1, 2].repeat(100)).unwrap();
assert!(!result.reject_goodness_of_fit);
assert_eq!(result.degrees_of_freedom, 18);
```

Binary data uses two unmerged bins and nine degrees of freedom. As in the
reference, this path does not enforce a minimum expected count of five.
Nonbinary data pools symbols in increasing frequency order until each bin has
expected count at least five, merging an underfilled final bin backward.
Its degrees of freedom are `9 * (number_of_bins - 1)`.

`chi_square_goodness_of_fit_with_options` uses the same `ChiSquareOptions` as
independence: default 256-bit MPFR calculations and NIST binary64 grouping
thresholds, or optional exact-count grouping. Fewer than ten samples, constant
input, or fewer than two pooled bins return errors, never a statistical pass.
Non-rejection alone does not establish IID.

The optional reference suite above now checks both chi-square tests on all
eleven NIST datasets. Rebuild the adapter to enable goodness-of-fit mode
(the adapter's optional second argument selects it).

## Permutation tests

`permutation_tests(&samples)` runs all 19 statistics from §5.1: excursion,
directional runs (count and longest), increases/decreases, median runs (count
and longest), collision (average and maximum), periodicity and covariance at
lags 1, 2, 8, 16, 32, and bzip2 compressed length.

```rust,no_run
use sp_800_90b::{permutation_tests_with_options, PermutationOptions};
let samples = [0, 1, 2].repeat(100);
let report = permutation_tests_with_options(&samples, PermutationOptions {
    seed: Some([1, 2, 3, 4]),
    ..Default::default()
}).unwrap();
println!("reject: {}, shuffles: {}", report.reject, report.permutations_performed);
```

The default seed comes from the operating system and is returned in the report.
An explicit nonzero four-word state reproduces the run. The implementation uses
xoshiro256** and NIST's unbiased Fisher–Yates shuffle, sequentially. A given seed
matches a single reference shuffle stream; the normal multithreaded NIST command
uses separate jumped streams and need not produce the same counters.

Each statistic reports its original value and counts of shuffled values greater
than, equal to, and less than it. At most 10,000 shuffles are performed. A statistic
rejects if either inclusive tail (`greater + equal` or `less + equal`) is at most
five. By default, evaluation of a statistic stops once both counts exceed five;
its counters may therefore sum to fewer than 10,000. Set `early_stop: false` to
retain all counts. These are permutation counts, not chi-square p-values.

`PermutationArithmetic::NistReference` is the default: excursion and average
collision use binary64 as in NIST, with integer-valued statistics accumulated
exactly. Compiler floating-point contraction can still cause tiny excursion
differences. `PermutationArithmetic::ExactCounts` instead compares rational
statistics by exact integer cross-products, preserving exact ties without
floating-point rounding. Reported values use `rug::Float` at 256 bits.
`permutation_statistics` computes just the unpermuted statistics in
`PERMUTATION_NAMES` order.

For binary inputs, observed symbols are rank-mapped to 0 and 1. Conversion I
counts ones per eight samples; conversion II packs them MSB first, retaining a
partial final block with zero padding. Nonbinary covariance, excursion, and
compression retain raw symbol values. Compression uses level-5 bzip2 on decimal
symbols separated by single spaces, with no trailing space.

The API rejects constant inputs and requires at least 2,056 binary samples or
257 nonbinary samples. These conservative limits ensure a completed collision
in every shuffle, avoiding the reference's undefined average when no collision
exists. The input must also fit the supported bzip2 buffer bound (`u32::MAX / 4`
samples). These API minimums do not replace NIST's assessment data requirements.
A non-rejection is not full IID validation.

Validation includes hand-calculated statistics, partial binary conversions,
ties and rejection boundaries, repeatable seeds, early/full-run agreement, and
an optional adapter calling unmodified NIST statistics and shuffle functions:

```sh
NIST_REFERENCE_DIR=/path/to/SP800-90B_EntropyAssessment \
NIST_PERMUTATION_REFERENCE=/path/to/compiled-permutation-adapter \
cargo test --test permutation_reference -- --ignored
```

Build `tests/reference/permutation.cpp` with the same reference include and link
flags as the chi-square adapter, adding `-lbz2`. The suite compares original
statistics on all eleven datasets, and exact seeded counters on 4,096-sample
prefixes of binary, byte, and ring-oscillator data. Full million-sample runs can
be expensive, particularly when a statistic requires all 10,000 shuffles.

## Combined IID assessment

`assess_iid(&samples, IidOptions)` follows the reference `ea_iid` workflow:
MCV entropy estimates, chi-square independence and goodness of fit, the IID LRS
hypothesis test (§5.2.5), and all 19 permutation statistics.

```rust,no_run
use sp_800_90b::{assess_iid, IidOptions, IidDecision};
let samples = std::fs::read("samples.bin").unwrap();
let report = assess_iid(&samples, IidOptions::default()).unwrap();
if report.decision == IidDecision::NotRejected {
    println!("{} bits/sample", report.min_entropy.as_ref().unwrap());
}
```

The report distinguishes `NotRejected`, `Rejected`, and `Incomplete`.
`checks` retains each test's full result or error; `is_complete()` reports whether
all checks ran successfully, regardless of statistical rejection. Any observed
rejection takes precedence over an unavailable check. Constant data short-circuits
with rejection, zero candidate entropy, and no hypothesis checks.

`candidate_min_entropy` is the MCV-derived estimate even if IID rejects.
`min_entropy` is present only when every check completed without rejecting.
Neither result incorporates a submitter entropy bound or establishes complete
entropy-source validation. Rejection does not automatically run the non-IID API.

Initial mode estimates literal MCV entropy and, for observed alphabets larger
than two, raw bitstring MCV entropy. The candidate is the minimum of the literal
estimate, bit width times the bit estimate, and the bit width. Conditioned mode
estimates only bitstring MCV entropy; its per-bit result is
`report.bitstring.as_ref().unwrap().min_entropy`, while the top-level estimate
remains in bits per original sample. **All hypothesis tests use the full literal
input in both modes**, as in NIST.

`IidOptions` exposes precision, confidence constant, declared width, mode,
bitstring truncation, chi-square grouping, and permutation options (including
seed and exact-count comparisons). It reuses non-IID validation, raw MSB-first
bit expansion, and warnings for small datasets, excess declared width, and
truncation. Out-of-range symbols are rejected rather than silently masked.
A non-rejected small dataset still carries the sample-size warning.

`iid_lrs_test(&samples, precision_bits)` is also available separately. It finds
the longest repeated substring including overlaps, computes the per-symbol
collision probability, and evaluates the NIST tail probability with MPFR
`log1p`/`expm1`. It is distinct from `lrs_estimate`, the non-IID entropy estimator.
This avoids the native long-double underflow assertion on highly repetitive
inputs within MPFR's much larger exponent range.

Run the file example with:

```sh
cargo run --release --example iid -- samples.bin 8
```

Optional integration tests use `NIST_REFERENCE_DIR` and `NIST_IID_LRS_REFERENCE`:

```sh
NIST_REFERENCE_DIR=/path/to/SP800-90B_EntropyAssessment \
NIST_IID_LRS_REFERENCE=/path/to/compiled-iid-lrs-adapter \
cargo test --test iid_reference -- --ignored
```

Build `tests/reference/iid_lrs.cpp` with the reference include/link flags plus
`-ldivsufsort -ldivsufsort64`. The reference LRS comparison excludes full
ring-oscillator data because NIST asserts on long-double underflow there;
Rust's extreme-repetition behavior has a separate regression test. The combined
API comparison invokes `cpp/ea_iid` in both initial and conditioned modes.

## Restart testing

`assess_restart(&samples, &initial_entropy, RestartOptions)` implements §3.1.4
for **exactly 1,000,000 samples**: 1,000 successive samples from each of 1,000
restarts, stored row-major. `initial_entropy` is a finite, nonnegative
`rug::Float` in bits per original sample. Its implied distribution must fit the
observed alphabet, and it cannot exceed the declared/inferred bit width.
Constant input and malformed data return explicit errors.

```rust,no_run
use sp_800_90b::Float;
use sp_800_90b::{assess_restart, RestartOptions};
let samples = std::fs::read("restart-samples.bin").unwrap();
let h_i = Float::with_val(256, Float::parse("0.85").unwrap());
let report = assess_restart(&samples, &h_i, RestartOptions {
    simulation_seed: Some([1, 2, 3, 4]),
    ..Default::default()
}).unwrap();
println!("{:?}: {:?}", report.decision, report.min_entropy);
```

The sanity check simulates the reference's inverted near-uniform distribution,
using five million 1,000-sample trials by default. `simulation_rounds` may be
increased but not reduced below that default. Its family-wise adjustment is
`alpha = 1 - exp(log(0.99)/2000)`. The cutoff is the sorted observation at
`floor((1-alpha)*rounds)-1`; a maximum count **equal to the cutoff passes**.
The implementation uses a histogram instead of storing/sorting every trial.
H_I=0 has deterministic cutoff 1000 and bypasses the simulation loop.

For reproducibility, the report retains the xoshiro seed, simulation count,
probability, cutoff, and all 1,000 row and 1,000 column maxima. A missing seed
uses OS randomness. The simulation deliberately uses binary64 probability and
uniform draws to match the reference; the supplied H_I and entropy estimates
retain the selected MPFR precision. A seed specifies one sequential stream,
not NIST's multiple OpenMP streams. Finite simulations can produce different
cutoffs on different seeds, particularly near a quantile boundary.

After sanity passes, both row-major and column-major sequences are assessed as
**literal symbols without bit expansion**. `RestartMode::NonIid` (default)
runs all applicable non-IID estimators. `RestartMode::Iid` runs MCV plus both
chi-square tests, the IID LRS test, and permutation tests on both sequences.
Raw symbols remain available for permutation statistics; binary estimators use
rank-mapped symbols. If a permutation seed is supplied, each orientation starts
from that state; otherwise each draws its own OS seed.

The validation condition is `min(H_r,H_c) >= H_I/2`. A successful report exposes
`min(H_I,H_r,H_c)` in `min_entropy`. Otherwise its decision distinguishes sanity
rejection, entropy rejection, IID rejection, and incomplete estimation, with no
validated entropy result. Row/column diagnostics remain available except when
sanity rejection short-circuits the assessment. Unlike the reference's final
success message, this API does not award an IID result after an IID test rejects;
unavailable estimators are also never silently treated as a complete run.

`restart_column_sequence` and `restart_sanity_check` expose the transpose and
sanity stages independently. The file example is:

```sh
cargo run --release --example restart -- restart-samples.bin 0.85 non-iid 1
```

The simulation and million-sample assessments are substantial workloads; use an
optimized build. Tests cover layout, maxima, cutoff equality, H_I/2 equality,
invalid inputs, seeded simulation parity, and full non-IID reference agreement.
The expensive checks are opt-in:

```sh
NIST_RESTART_REFERENCE=/path/to/compiled-restart-adapter \
cargo test --lib simulation_matches_reference_stream -- --ignored
cargo test --test restart sanity_rejection -- --ignored
NIST_REFERENCE_DIR=/path/to/SP800-90B_EntropyAssessment \
cargo test --test restart_reference -- --ignored
```

Build `tests/reference/restart.cpp` with the reference include/link flags,
including bzip2 and both divsufsort libraries. Its seeded comparison calls NIST's
unmodified `simulateCount` for binary, noninteger, and full-byte entropy levels.
The end-to-end non-IID comparison uses H_I=0 to remove Monte Carlo variability;
a separate test exercises the full five-million-round nondegenerate simulation.

## Conditioning entropy

`conditioning_entropy(n_in, n_out, nw, &h_in, kind, options)` calculates
`Output_Entropy` (§3.1.5.1.2), clamping the internal width to the input width as
specified in Appendix E. All entropy quantities are bits **per block**, except
non-vetted h', which is bits **per output bit**.

```rust
use sp_800_90b::Float;
use sp_800_90b::{conditioning_entropy, ConditioningKind, ConditioningOptions};
let h_in = Float::with_val(256, 400);
let result = conditioning_entropy(512, 128, 256, &h_in,
    ConditioningKind::NonVetted {
        assessed_entropy_per_bit: Float::with_val(256, 0.5),
    }, ConditioningOptions::default()).unwrap();
assert_eq!(result.min_entropy, 64);
```

For non-vetted conditioning, the final result is the minimum of Output_Entropy,
`0.999*n_out`, and `h'*n_out`. Obtain h' from the **bitstring** result of an
appropriate completed conditioned-mode assessment, not its top-level per-symbol
entropy. For IID assessments, first check that the IID decision is NotRejected.
The caller supplies the appropriate assessed input entropy and component widths;
selecting `Vetted` asserts that status rather than verifying the component.

Probability expressions are rounded upward and entropy downward. Working
precision automatically increases to at least `2*n_in+64` bits and preserves the
supplied input entropy's precision; the report gives the actual working precision
and probability bounds. The default output precision is 256 bits, and the default
working-precision budget is 1,048,576 bits. Exceeding that budget returns an error.
Zero input entropy and zero h' produce zero entropy without the reference CLI's
strict-positive restriction. The API does not label a near-full result as full
entropy or apply separate FIPS eligibility rules.

Eight vetted/non-vetted configurations are compared with `ea_conditioning`:

```sh
NIST_REFERENCE_DIR=/path/to/SP800-90B_EntropyAssessment \
cargo test --test conditioning_reference -- --ignored
cargo run --example conditioning -- 512 128 256 400 0.5
```

## Continuous health tests

`HealthTests` implements Repetition Count (RCT) and Adaptive Proportion (APT)
from §4.4 on **raw noise-source samples**, with the `assessment` feature enabled
(the default). Supply the assessed entropy per sample
and the source's alphabet size, rather than inferring an alphabet from a small
chunk. Byte-valued symbols may be sparse; encountering more distinct symbols than
the configured alphabet produces an error without consuming that sample.

```rust
use sp_800_90b::Float;
use sp_800_90b::{HealthTests, HealthTestOptions};
let mut tests = HealthTests::new(&Float::with_val(256, 1),
    HealthTestOptions::default()).unwrap();
tests.observe_slice(&[0, 1].repeat(512)).unwrap();
assert!(tests.status().startup_ready());
assert!(!tests.status().failed());
```

The default false-positive parameter is alpha=2^-20; integer exponents 1..=128
are supported. `health_test_cutoffs` exposes cutoff derivation separately. RCT
uses `1+ceil(-log2(alpha)/H)` and rejects at equality. APT uses the publication's
`1+CRITBINOM(W,2^-H,1-alpha)` convention, verified against all ten Table 2 entries.
Its non-overlapping windows have W=1024 for binary sources and W=512 otherwise.
The first sample is the reference and counts as the first occurrence. APT
rejects as soon as its reference count reaches C. The optional binary test of
the opposite symbol's count is not enabled. At very small entropy the calculated
APT cutoff may exceed W, making APT unable to reject; RCT still operates.

State persists across chunks and APT windows; a window boundary does not reset
RCT. The status retains the first failure of each test with its zero-based sample
index and latches failure until explicit `reset()`. Reset also restarts the
startup count. `startup_ready()` requires at least 1,024 tested samples with no
latched failure. The application must withhold startup output, handle failures,
and arrange any required on-demand testing; these APIs do not gate a device or
release entropy. Partial final windows remain in progress, not classified as
complete passes. `observe_slice` stops on an input error and preserves the
successfully consumed prefix.

These are the two approved general-purpose tests, not source-specific diagnostic
tests or a certification of the physical source. The NIST entropy-assessment
repository has no matching health-test executable; validation uses the published
cutoff table plus exact boundary, biased-stream, chunking, and reset tests.

```sh
cargo run --example health -- raw-samples.bin 1 2
```

### Embedded health monitoring

With default features disabled, the library is `no_std`, allocation-free and has
no external dependencies. `HealthTestMonitor` implements the same streaming
state machine used by `HealthTests`; only cutoff derivation and the retained
MPFR diagnostics require the `assessment` feature. The CLI requires `cli`,
which enables `assessment`. With only `assessment`, CLI dependencies are omitted.

Calculate cutoffs on a host with the default features, using the assessed
entropy of the source and your chosen false-positive parameter:

```rust
use sp_800_90b::{Float, HealthTestOptions, health_test_cutoffs};
let cutoffs = health_test_cutoffs(
    &Float::with_val(256, 1),
    HealthTestOptions::default(),
).unwrap();
let config = cutoffs.monitor_config().unwrap();
assert_eq!(config.alphabet_size(), 2);
assert_eq!(config.repetition_count(), 21);
assert_eq!(config.adaptive_proportion(), 589);
assert_eq!(config.window_size(), 1024);
```

Carry the three integer parameters into firmware. The following dependency uses
the Git source until the first crates.io release:

```toml
[dependencies]
sp-800-90b = { git = "https://github.com/walters-labs/sp-800-90b", default-features = false }
```

Firmware can construct both the configuration and initial monitor in a const
context. This example configuration is specific to H=1 bit/sample, a binary
alphabet and alpha=2^-20; it is not a default for unknown sources:

```rust
use sp_800_90b::{HealthTestConfig, HealthTestMonitor};
const CONFIG: HealthTestConfig = match HealthTestConfig::new(2, 21, 589) {
    Ok(config) => config,
    Err(_) => panic!("invalid health-test parameters"),
};
let mut monitor = HealthTestMonitor::new(CONFIG);
for _ in 0..512 {
    monitor.observe_slice(&[0, 1]).unwrap();
}
assert!(monitor.status().startup_ready());
```

`HealthTestConfig` validates alphabet size (2..=256), an RCT cutoff of at least 2,
and an APT cutoff in 2..=W+1. It derives W=1024 for binary alphabets and W=512
otherwise. Private fields preserve these structural checks. The constructor
does not validate a cutoff's statistical suitability; that requires the host
calculation and a justified entropy bound. `monitor_config()` also checks the
window length when exporting a report whose public fields may have been edited.

The core retains all streaming behavior described above: sparse symbol values,
failure latching, reset, startup counting, chunk independence and input errors
that leave the offending sample unconsumed. It uses fixed-size state and integer
operations, and does not estimate entropy, generate randomness or gate output.

Reproduce the minimal checks with Rust 1.85.0:

```sh
cargo +1.85.0 test --locked --no-default-features --all-targets
cargo +1.85.0 test --locked --no-default-features --doc
rustup target add thumbv7em-none-eabi --toolchain 1.85.0
cargo +1.85.0 check --locked --no-default-features --lib --target thumbv7em-none-eabi
cargo tree --no-default-features --edges normal
```

The last command lists only this crate. The target check confirms compilation
for a Cortex-M bare-metal target; behavior tests run on the host, not hardware.
Cargo features are additive: another dependency enabling `assessment` for this
crate also enables its `std`/MPFR dependencies.

## Strict numerical validation workflow

`assess_validation` coordinates §3.1 track selection, the submitter bound,
restart testing, and optional conditioning. It requires at least 1,000,000 raw
sequential samples and exactly 1,000,000 restart samples in row-major order.
An explicit sample width applies to both datasets. Existing assessment APIs
remain available for exploratory data and partial estimates.

```rust,no_run
use sp_800_90b::Float;
use sp_800_90b::{assess_validation, ValidationOptions, ValidationDecision};
# let sequential = vec![0u8; 1_000_000];
# let restart = vec![0u8; 1_000_000];
let h_submitter = Float::with_val(256, 0.5); // bits per raw sample
let report = assess_validation(
    &sequential, &restart, &h_submitter, None, ValidationOptions::default(),
)?;
if report.decision == ValidationDecision::Complete {
    println!("Entropy per sample: {}", report.output_entropy.unwrap());
}
# Ok::<(), sp_800_90b::ValidationError>(())
```

Set `iid_claim` only when the source has a documented design rationale for IID.
The workflow tests sequential data, restart rows and columns, and any non-vetted
conditioned bitstring. Statistical rejection selects the non-IID track for all
stages; a failed check execution leaves an incomplete report. IID evidence is
retained; checks stop as soon as rejection or failed execution determines the
next step. One permutation seed is recorded and reused within the workflow so
repeated checks cannot change track because of fresh randomness.

The workflow applies `H_I = min(assessed entropy, H_submitter)` before restart
testing. Rejected restarts and incomplete stages never populate `raw_entropy`
or `output_entropy`. Diagnostic stage results remain available.

Optional `ValidationConditioning` records the number of raw samples consumed
per block, output block width, and internal width. Input entropy is calculated
from the post-restart bound with downward rounding. For non-vetted conditioning,
provide one byte per output bit (0 or 1), concatenated in generation order,
representing at least a million **complete output blocks**. The workflow derives
the per-bit bound from that data; it does not accept an unchecked h′ value.
`raw_entropy` always means bits per raw sample. With conditioning,
`output_entropy` means bits per conditioning output block.

`Complete` describes the numerical workflow. Source models, collection and
restart provenance, vetted-component status, environmental testing, and health
test deployment still require external evidence. This API does not certify a
source, establish a full-entropy claim, or model output truncation.

The full million-sample IID workflow regression is opt-in. It passed in about
46 seconds on the development M1 after reusing IID evidence and optimizing the
Rust compression backend for tests:

```sh
cargo test --test validation successful_iid_workflow_uses_one_track -- --ignored --nocapture
```

For live diagnostics, use `assess_validation_with_progress` with a callback:

```rust,no_run
# use sp_800_90b::*;
# use sp_800_90b::Float;
# let sequential = vec![0u8; 1_000_000];
# let restart = sequential.clone();
let report = assess_validation_with_progress(
    &sequential, &restart, &Float::with_val(256, 0.5), None,
    ValidationOptions::default(),
    &mut |event| match event {
        ValidationProgress::Permutation { stage, progress }
            if progress.permutations_completed % 100 == 0 => {
            eprintln!("{stage}: {} permutations, {:?}, active: {:?}",
                progress.permutations_completed, progress.elapsed,
                progress.active_statistics);
        }
        ValidationProgress::Permutation { .. } => {}
        other => eprintln!("{other:?}"),
    },
)?;
# Ok::<(), ValidationError>(())
```

Callbacks run synchronously and should be brief. Stage-finished events carry
elapsed wall time and also occur when a stage returns an error; the returned
report or error determines success. Permutation snapshots arrive before the
initial statistics and after each shuffle, including the final shuffle.
`assess_iid_with_progress` and `permutation_tests_with_progress` expose the same
permutation diagnostics independently. Existing APIs remain silent.

The strict workflow reuses completed restart-row and restart-column IID checks
from track selection, including their literal MCV estimates. Reuse is internal
to one invocation; callers cannot supply unrelated cached evidence. An
unconditioned successful IID run now performs three permutation assessments
instead of five. The test profile also optimizes the Rust bzip2 backend.

## Reducing wider samples (§6.4)

`SymbolReduction` selects ranked bits from unsigned samples of 1–128 bits,
producing one 1–8-bit symbol per original sample. Supply a complete ranking
based on the noise-source model, most entropy-contributing position first.
Positions are zero-based from the least significant bit; the first ranked bit
becomes the most significant output bit. Rankings are not estimated from data.

```rust
use sp_800_90b::{SymbolReduction, NonIidOptions, assess_non_iid};
// Keep the low four bits of a 32-bit clock sample in their original order.
let mut ranking = vec![3, 2, 1, 0];
ranking.extend(4..32);
let mapping = SymbolReduction::new(32, 4, &ranking)?;
let samples: Vec<u128> = vec![0x1234_abcd, 0x1234_abce, 0x1234_abcf];
let reduced = mapping.reduce(&samples)?;
assert_eq!(reduced, [13, 14, 15]);
let report = assess_non_iid(&reduced, NonIidOptions {
    bits_per_symbol: Some(mapping.output_bits()),
    ..Default::default()
})?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Use the same mapping for sequential and restart datasets. Pass the reduced width
explicitly to IID, non-IID, restart, or validation options. Entropy is measured
per original sample because mapping preserves sample boundaries; its ceiling is
`mapping.entropy_ceiling()` bits. Never multiply it by the original/reduced width
ratio. Submitter and restart entropy bounds must respect this reduced ceiling.
The preprocessing operation alone neither estimates entropy nor establishes IID.

Decode file bytes using the source's documented byte order before conversion to
`u128`. Values exceeding the declared input width, duplicate/out-of-range ranks,
and incomplete rankings are rejected. Wider than 128-bit representations and
alternative source-specific reduction methods are not supported by this API.

NIST's C++ tools accept byte symbols and do not implement this ranked wide-input
mapping (`ea_transpose` transposes restart data). Mapping tests therefore use
specification examples and independent expected bit selections. An opt-in test
in `non_iid_reference` embeds NIST's four-bit fixture in scattered 32-bit
positions, reduces it, and compares the complete assessment with NIST's recorded
results. It runs automatically in the reference harness.


## Truncating vetted conditioning output

Set `ConditioningOptions::retained_output_bits` to the delivered length after
conditioning. The `output_bits` argument still describes the full conditioning
output. The API first calculates the full bound, then scales it by
`retained_output_bits / output_bits`, rounding downward. It does not rerun
`Output_Entropy` with a shorter output length.

```rust
use sp_800_90b::Float;
use sp_800_90b::{conditioning_entropy, ConditioningKind, ConditioningOptions};
let estimate = conditioning_entropy(
    512, 256, 256, &Float::with_val(256, 400), ConditioningKind::Vetted,
    ConditioningOptions { retained_output_bits: Some(128), ..Default::default() },
)?;
assert_eq!(estimate.output_bits, 256);
assert_eq!(estimate.retained_output_bits, 128);
// estimate.untruncated_min_entropy retains the full-output bound.
// estimate.min_entropy is in bits per delivered 128-bit block.
# Ok::<(), sp_800_90b::ConditioningError>(())
```

The standalone `vetted_truncation_entropy` helper implements the same accounting
rule: 6 bits of entropy in an 8-bit output becomes 4.5 bits when 6 output bits
are retained (SP 800-90B §3.1.5.1.2). The caller asserts vetted status. This is
not a general entropy rule for truncating arbitrary data, nor does it transform
output bytes. Zero lengths and expansion are rejected. `None` leaves output
unchanged; equal lengths are allowed for vetted components.

Non-vetted conditioning rejects any explicit truncation request, as required
by §3.1.5.2; use `None` for unmodified output. The strict validation workflow
accepts the same option through `ValidationOptions::conditioning` and reports
entropy per delivered block. Invalid truncation configurations are rejected
before statistical testing. Typed component dimensions are described below;
additional-source input accounting is described below.

## Typed vetted conditioning components

`VettedConditioner` derives full output length and nominal internal width from
SP 800-90B (January 2018), §3.1.5.1.1, Table 1. It covers HMAC, AES-CMAC,
AES-CBC-MAC, hash functions, Hash_df, and AES Block_Cipher_df.
`ConditioningHash` identifies fixed-output SHA-1, SHA-2, and SHA-3 variants;
`ConditioningAesKey` permits only 128-, 192-, and 256-bit keys. Numeric key
lengths can be checked with `ConditioningAesKey::try_from(bits)`.

```rust
use sp_800_90b::Float;
use sp_800_90b::{ConditioningHash, ConditioningOptions, VettedConditioner};
let component = VettedConditioner::Hash(ConditioningHash::Sha256);
let report = component.assess(
    512, &Float::with_val(256, 400),
    ConditioningOptions { retained_output_bits: Some(128), ..Default::default() },
)?;
assert_eq!(report.output_bits, 256);
assert_eq!(report.retained_output_bits, 128);
assert_eq!(report.vetted_component, Some(component));
# Ok::<(), sp_800_90b::ConditioningError>(())
```

For the strict workflow use `ValidationConditioning::VettedComponent {
 samples_per_block, component }`. The report retains the selected component.
The existing manually dimensioned `Vetted` variant and `conditioning_entropy`
remain available, with `vetted_component: None` in their reports.
`validate_dimensions` checks externally supplied full-output dimensions;
truncation belongs in `retained_output_bits` instead.

Hash-based dimensions use digest size, including SHA-512/224 and SHA-512/256.
AES MAC dimensions remain 128 bits regardless of key length; Block_Cipher_df
uses key length. For Hash_df and Block_Cipher_df the derived output length is
also the required `no_of_bits_to_return`. Formula-level input-width clamping
remains visible separately as `effective_internal_width`.

These are publication-era numerical descriptions, not cryptographic
implementations or a current approval registry. Selecting one does not establish
CAVP assurance or validate keys, padding, or implementation behavior. SHA-1 is
included for historical modeling. SHAKE/XOFs and custom digest sizes are not
represented. Primary-source input accounting still applies; explicit additional-source accounting is supported as described below.


## Additional noise-source input accounting (§3.1.6)

Set `ConditioningOptions::additional_input_bits` to the aggregate number of bits
from additional noise sources concatenated into each conditioning block. It
defaults to zero. `input_bits` and `input_entropy` continue to describe only the
primary source. Additional sources receive no entropy credit and do not enlarge
the formula's input width, effective internal width, or working precision.

```rust
use sp_800_90b::Float;
use sp_800_90b::{ConditioningHash, ConditioningOptions, VettedConditioner};
let report = VettedConditioner::Hash(ConditioningHash::Sha256).assess(
    512, &Float::with_val(256, 400),
    ConditioningOptions { additional_input_bits: 128, ..Default::default() },
)?;
assert_eq!(report.input_bits, 512); // n_in, never 640
assert_eq!(report.input_accounting.total_bits, 640);
assert_eq!(report.primary_input_entropy, 400);
# Ok::<(), sp_800_90b::ConditioningError>(())
```

Reports include `input_accounting` (primary, additional, and total lengths) and
`primary_input_entropy`. This works with typed components and the manual
`ConditioningKind::Vetted` assertion. Non-vetted conditioning rejects nonzero
additional lengths. Zero primary input and overflowing total lengths are errors.
This metadata describes concatenation; it does not perform it or determine
whether sources are physically distinct or independent.

For `assess_validation`, set the same option under
`ValidationOptions::conditioning`. `samples_per_block` counts primary samples
only; their assessed entropy after restart testing supplies the entire credited
input entropy. Additional inputs are invalid without conditioning. Truncation
continues to apply after the full conditioning bound. The source developer must
identify the primary source correctly; copies of one physical noise source are
not automatically separate additional sources.
