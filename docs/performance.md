# Non-IID performance

These measurements cover the post-v0.1.0 optimizations in this source tree.
The published v0.1.0 package does not contain them. They retain the existing
public API, 256-bit MPFR defaults, confidence constants and stopping rules.

## Local comparison — 2026-10-20

Each fixture contains 1,000,000 samples from the pinned NIST repository.
Both tools use initial assessment mode and all bits: four-bit and eight-bit
inputs expand to 4,000,000 and 8,000,000 bit samples respectively. Binary input
is assessed once. These are bundled software fixtures, not newly collected
physical noise-source measurements.

Median end-to-end wall time over three measured runs after one warm-up:

| Fixture | Rust before | Rust optimized | NIST | Rust speedup |
| --- | ---: | ---: | ---: | ---: |
| `truerand_1bit.bin` | 4.05 s | 1.70 s | 0.56 s | 2.38× |
| `truerand_4bit.bin` | 13.64 s | 5.56 s | 8.76 s | 2.45× |
| `truerand_8bit.bin` | 26.24 s | 7.86 s | 12.27 s | 3.34× |

Median per-process peak resident memory (MiB):

| Fixture | Rust before | Rust optimized | NIST |
| --- | ---: | ---: | ---: |
| `truerand_1bit.bin` | 93.1 | 61.2 | 21.2 |
| `truerand_4bit.bin` | 392.7 | 327.4 | 268.2 |
| `truerand_8bit.bin` | 678.8 | 562.3 | 348.2 |

The optimized Rust tool is faster than this NIST build on the four-bit and
eight-bit fixtures; NIST remains faster on binary input and uses less memory
on all three. This is one host and three fixtures, not a general ranking.

The environment was an Apple M1 with 16 GiB RAM and macOS 27.0.1. Rust used
1.97.1 and a default release build (`cargo build --release --locked --offline`).
The baseline executable was built from `91ff3e459d0b25b498c288fda69bb58f2a1ad162`,
whose implementation matches v0.1.0. The optimized executable used the changes
described below. NIST revision `87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289`
used GCC 16.2.0 with `-O2 -ffloat-store`, OpenMP limited to one thread, and
53-bit `long double`; Rust retained 256-bit MPFR. This compares the tools at
those precisions, not the languages with identical arithmetic.

All runs were sequential, with tool order rotated across measured rounds and
JSON output enabled. Builds were excluded. Wall time was measured around the
CLI subprocess; `/usr/bin/time -l` supplied peak RSS. No builds or tests ran
alongside the measurements, but other desktop activity and thermal state were
not controlled. The eight-bit wall-time ranges were 25.38–26.33 s before,
7.79–8.84 s optimized, and 11.43–12.34 s for NIST. Earlier one-shot pilot timings
(23.24 s Rust and 9.71 s NIST) were separate observations.

Every Rust JSON report matched the baseline exactly, including all estimator
entropy strings, statuses and warnings. Every NIST overall estimate agreed
within `1e-9` bits per input symbol; the separate reference tests retain their
existing, estimator-specific tolerances.

All 10 individual-estimator and 3 combined non-IID reference tests subsequently
passed using the existing pinned M1 snapshot. Regular checks passed 182 tests
and 16 doctests, including new regressions for predictor equivalence, cached
MPFR moments and shared tuple results. See the [coverage audit](coverage-audit.md)
for the verification scope and distinction from release CI.

The local raw report, commands, hashes, outputs and runner are in
`target/benchmarks/optimization/` (ignored build artifacts). Input SHA-256:

| Fixture | SHA-256 |
| --- | --- |
| 1-bit | `f9ea8832af4c4205f518845b264465800921688fc2c4d566fbc087664aeb2313` |
| 4-bit | `489bc841bb364ba86da70b1617138aef76b25dd9196ad669eef40c1441b6cb88` |
| 8-bit | `c7e56911d2657fa9b6e86c03d4477474d6ec698691c5f32d3918ec513713e3c3` |

## Changes

- Combined assessments compute suffix counts once for t-Tuple and LRS. The
  individual estimator APIs remain available and compute their own counts.
- Compression caches logarithms of recurring distances, with at most 4096
  cache slots. It traverses the distances twice instead of storing a separate
  MPFR value for every observation. Addition order and precision are unchanged;
  distances outside the cache still use MPFR.
- MultiMMC and LZ78Y use directly indexed context tables for samples encoded as
  `0` and `1`. Other inputs retain the general implementation. Both paths use
  the same high-precision probability calculation. The binary paths preserve
  dictionary caps, score ties, null predictions and final model updates.

## Diagnostic timings

Build the diagnostic example from a checkout and give it an existing sample
file and its symbol width:

```sh
cargo build --release --locked --example profile_non_iid
target/release/examples/profile_non_iid samples.bin 8
```

It runs the individual estimators on the rank-mapped literal samples and, for
alphabets larger than two, the full MSB-first bitstring. It then times the
combined initial assessment separately. Output is tab-separated and includes
the sample count, elapsed seconds and full-precision entropy. The combined
assessment can share work, so its time is not the sum of the individual times.
File reading and result formatting are outside these diagnostic timings.
Constant inputs are rejected by this example because the combined API returns
zero without running the individual estimators.

For an end-to-end CLI comparison, build both tools first, use the same file,
width and bitstring policy, and run sequentially:

```sh
env OMP_NUM_THREADS=1 OMP_DYNAMIC=FALSE /usr/bin/time -l \
  target/release/sp-800-90b non-iid samples.bin \
  --bits-per-symbol 8 --mode initial --precision 256 --json > rust.json

env OMP_NUM_THREADS=1 OMP_DYNAMIC=FALSE /usr/bin/time -l \
  /path/to/nist/cpp/ea_non_iid -i -a -q -o nist.json samples.bin 8
```

`time -l` is the macOS command; GNU time uses `-v` and reports maximum RSS in
KiB rather than bytes. Run a warm-up for each tool, exclude it from the results,
then rotate tool order over repeated measurements. Avoid simultaneous builds
or tests. Record input and binary hashes, compiler flags, arithmetic precision,
raw observations and peak memory alongside the results.
