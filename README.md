# sp-800-90b

Incremental Rust implementation of NIST SP 800-90B. Currently implements
**Most Common Value (§6.3.1)**, **Collision (§6.3.2)**, **Markov (§6.3.3)**, **Compression (§6.3.4)**, **t-Tuple (§6.3.5)**, **LRS (§6.3.6)**, **MultiMCW (§6.3.7)**, **Lag (§6.3.8)**, **MultiMMC (§6.3.9)**, and **LZ78Y (§6.3.10)**; this is not yet a full entropy-source
assessment or a replacement for the NIST tools.

```rust
use sp_800_90b::most_common_value;

let estimate = most_common_value(&[0, 1, 1, 2, 1, 0]).unwrap();
println!("{} bits/sample", estimate.min_entropy);
```

Inputs are slices of byte-valued symbols (0–255), one symbol per byte.
At least two samples are required. The result includes the sample and mode
counts, empirical probability, upper confidence bound, and entropy.

## Precision and reference compatibility

Arithmetic uses `rug::Float` backed by MPFR, with **256-bit significand precision**
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

## Build and test

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo run --example mcv -- /path/to/samples.bin
```

The first build compiles bundled GMP/MPFR through `gmp-mpfr-sys` and can take
several minutes. It requires a native C toolchain and make; on macOS install
Xcode Command Line Tools. Homebrew's jsoncpp and the NIST build are not required.
Dependency versions are recorded in `Cargo.lock`.

Tests cover independently calculated high-precision values, clipping, constant
data, all 256 symbols, symbol relabeling, invalid inputs, and precision stability.
An optional integration test reads all 11 NIST binary datasets and compares the
literal-symbol MCV, binary collision/Markov/compression, and literal/bitstring t-Tuple/LRS estimates against locally generated C++ self-test results
with an absolute tolerance of `1e-12`. MultiMCW, Lag, MultiMMC, and LZ78Y also compare exact prediction
counts and entropy, allowing `1e-9` for the local-bound cases explained below:

```sh
NIST_REFERENCE_DIR=/path/to/SP800-90B_EntropyAssessment \
  cargo test --test nist_reference -- --ignored
```

Run NIST's self-tests in that checkout first. This comparison covers the ten implemented estimators only, not the full assessment workflow.
The test harness expands multibit samples most-significant bit first for Collision.

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

All ten non-IID estimators in §6.3 are now available individually. Combined
assessment, IID tests, restart tests, and the remaining source-validation
workflow are not yet implemented.
