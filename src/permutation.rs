//! NIST §5.1 permutation tests, including binary conversions I and II.
use bzip2::{Compression, write::BzEncoder};
use rug::Float;
use std::{cmp::Ordering, error::Error, fmt, io::Write};

pub const PERMUTATION_NAMES: [&str; 19] = [
    "excursion",
    "numDirectionalRuns",
    "lenDirectionalRuns",
    "numIncreasesDecreases",
    "numRunsMedian",
    "lenRunsMedian",
    "avgCollision",
    "maxCollision",
    "periodicity(1)",
    "periodicity(2)",
    "periodicity(8)",
    "periodicity(16)",
    "periodicity(32)",
    "covariance(1)",
    "covariance(2)",
    "covariance(8)",
    "covariance(16)",
    "covariance(32)",
    "compression",
];
const LAGS: [usize; 5] = [1, 2, 8, 16, 32];
const ROUNDS: usize = 10_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PermutationArithmetic {
    /// Binary64 excursion and average collision, as in the NIST reference.
    #[default]
    NistReference,
    /// Exact rational comparisons, including ties, for all statistics.
    ExactCounts,
}
#[derive(Clone, Copy, Debug)]
pub struct PermutationOptions {
    /// Initial xoshiro256** state. None obtains a fresh seed from the OS.
    pub seed: Option<[u64; 4]>,
    /// Stop evaluating a statistic once both inclusive tail counts exceed five.
    pub early_stop: bool,
    pub arithmetic: PermutationArithmetic,
}
impl Default for PermutationOptions {
    fn default() -> Self {
        Self {
            seed: None,
            early_stop: true,
            arithmetic: PermutationArithmetic::NistReference,
        }
    }
}
#[derive(Debug)]
pub enum PermutationError {
    ConstantInput,
    TooFewSamples { count: usize, minimum: usize },
    TooManySamples,
    ZeroSeed,
    Randomness(String),
    Compression(std::io::Error),
}
impl fmt::Display for PermutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConstantInput => write!(f, "permutation tests require at least two symbols"),
            Self::TooFewSamples { count, minimum } => write!(
                f,
                "need at least {minimum} samples to guarantee collision statistics in every shuffle; got {count}"
            ),
            Self::TooManySamples => write!(f, "input exceeds the supported bzip2 buffer size"),
            Self::ZeroSeed => write!(f, "xoshiro256** state must not be all zero"),
            Self::Randomness(e) => write!(f, "OS randomness failed: {e}"),
            Self::Compression(e) => write!(f, "bzip2 failed: {e}"),
        }
    }
}
impl Error for PermutationError {}
#[derive(Clone, Debug)]
pub struct PermutationTestResult {
    pub name: &'static str,
    /// Original statistic, represented at 256-bit precision.
    pub original: Float,
    pub greater: usize,
    pub equal: usize,
    pub less: usize,
    pub reject: bool,
}
impl PermutationTestResult {
    pub fn evaluated_permutations(&self) -> usize {
        self.greater + self.equal + self.less
    }
    fn passed(&self) -> bool {
        self.greater + self.equal > 5 && self.less + self.equal > 5
    }
}
#[derive(Clone, Debug)]
pub struct PermutationAssessment {
    pub sample_count: usize,
    pub alphabet: Vec<u8>,
    pub seed: [u64; 4],
    pub arithmetic: PermutationArithmetic,
    pub early_stop: bool,
    pub permutations_performed: usize,
    pub tests: Vec<PermutationTestResult>,
    /// Any rejected statistic rejects the permutation suite; non-rejection is not full IID validation.
    pub reject: bool,
}

#[derive(Clone, Copy, Debug)]
struct Value {
    numerator: u128,
    denominator: u128,
    reference: f64,
}
impl Value {
    fn integer(n: u128) -> Self {
        Self {
            numerator: n,
            denominator: 1,
            reference: n as f64,
        }
    }
    fn float(self, mode: PermutationArithmetic) -> Float {
        match mode {
            PermutationArithmetic::NistReference => Float::with_val(256, self.reference),
            PermutationArithmetic::ExactCounts => Float::with_val(
                256,
                Float::with_val(256, self.numerator) / Float::with_val(256, self.denominator),
            ),
        }
    }
    fn compare(self, other: Self, mode: PermutationArithmetic) -> Ordering {
        match mode {
            PermutationArithmetic::NistReference => self.reference.total_cmp(&other.reference),
            PermutationArithmetic::ExactCounts => {
                (self.numerator * other.denominator).cmp(&(other.numerator * self.denominator))
            }
        }
    }
}
struct Context {
    alphabet: Vec<u8>,
    ranks: [u8; 256],
    sum: u128,
    median_twice: usize,
}
impl Context {
    fn new(data: &[u8]) -> Result<Self, PermutationError> {
        if data.len() > u32::MAX as usize / 4 {
            return Err(PermutationError::TooManySamples);
        }
        let mut counts = [0usize; 256];
        for &s in data {
            counts[s as usize] += 1;
        }
        let alphabet: Vec<u8> = (0..=255).filter(|&s| counts[s as usize] > 0).collect();
        if alphabet.len() < 2 {
            return Err(PermutationError::ConstantInput);
        }
        // A full 257-byte conversion II sequence guarantees a collision in every permutation.
        let minimum = if alphabet.len() == 2 { 2056 } else { 257 };
        if data.len() < minimum {
            return Err(PermutationError::TooFewSamples {
                count: data.len(),
                minimum,
            });
        }
        let mut ranks = [0; 256];
        let mut cumulative = 0;
        let mut median_twice = 0;
        for (i, &s) in alphabet.iter().enumerate() {
            ranks[s as usize] = i as u8;
            for target in [(data.len() - 1) / 2, data.len() / 2] {
                if cumulative <= target && target < cumulative + counts[s as usize] {
                    median_twice += i;
                }
            }
            cumulative += counts[s as usize];
        }
        Ok(Self {
            alphabet,
            ranks,
            sum: data.iter().map(|&x| x as u128).sum(),
            median_twice,
        })
    }
}
fn runs(signs: impl Iterator<Item = bool>) -> (usize, usize, usize) {
    let (mut count, mut longest, mut length, mut positive) = (0, 0, 0, 0);
    let mut previous = None;
    for sign in signs {
        positive += usize::from(sign);
        if previous == Some(sign) {
            length += 1;
        } else {
            count += 1;
            length = 1;
        }
        longest = longest.max(length);
        previous = Some(sign);
    }
    (count, longest, positive)
}
fn conversions(data: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut ones = Vec::with_capacity(data.len().div_ceil(8));
    let mut bytes = Vec::with_capacity(data.len().div_ceil(8));
    for block in data.chunks(8) {
        ones.push(block.iter().sum());
        bytes.push(block.iter().enumerate().map(|(i, &x)| x << (7 - i)).sum());
    }
    (ones, bytes)
}
fn statistics(
    raw: &[u8],
    ctx: &Context,
    active: &[bool; 19],
) -> Result<[Value; 19], PermutationError> {
    let mut values = [Value::integer(0); 19];
    let mapped: Vec<u8> = raw.iter().map(|&x| ctx.ranks[x as usize]).collect();
    let binary = ctx.alphabet.len() == 2;
    let (ones, bytes) = if binary {
        conversions(&mapped)
    } else {
        (Vec::new(), Vec::new())
    };
    let directional = if binary { &ones[..] } else { &mapped[..] };
    if active[0] {
        let (mut running, mut maximum) = (0u128, 0u128);
        let mut reference = 0.0f64;
        let mean = ctx.sum as f64 / raw.len() as f64;
        for (i, &x) in raw.iter().enumerate() {
            running += x as u128;
            maximum =
                maximum.max((running * raw.len() as u128).abs_diff((i + 1) as u128 * ctx.sum));
            reference = reference.max((running as f64 - (i + 1) as f64 * mean).abs());
        }
        values[0] = Value {
            numerator: maximum,
            denominator: raw.len() as u128,
            reference,
        };
    }
    if active[1..4].iter().any(|&x| x) {
        let (count, longest, positive) = runs(directional.windows(2).map(|w| w[0] <= w[1]));
        values[1] = Value::integer(count as u128);
        values[2] = Value::integer(longest as u128);
        values[3] = Value::integer(positive.max(directional.len() - 1 - positive) as u128);
    }
    if active[4] || active[5] {
        let (count, longest, _) = runs(mapped.iter().map(|&x| {
            if binary {
                x == 1
            } else {
                2 * x as usize >= ctx.median_twice
            }
        }));
        values[4] = Value::integer(count as u128);
        values[5] = Value::integer(longest as u128);
    }
    if active[6] || active[7] {
        let data = if binary { &bytes[..] } else { &mapped[..] };
        let mut seen = [false; 256];
        let (mut length, mut count, mut total, mut longest) = (0usize, 0usize, 0usize, 0usize);
        for &x in data {
            length += 1;
            if seen[x as usize] {
                count += 1;
                total += length;
                longest = longest.max(length);
                seen.fill(false);
                length = 0;
            } else {
                seen[x as usize] = true;
            }
        }
        debug_assert!(count > 0);
        values[6] = Value {
            numerator: total as u128,
            denominator: count as u128,
            reference: total as f64 / count as f64,
        };
        values[7] = Value::integer(longest as u128);
    }
    for (i, lag) in LAGS.into_iter().enumerate() {
        if active[8 + i] {
            values[8 + i] = Value::integer(
                directional
                    .iter()
                    .zip(&directional[lag..])
                    .filter(|(a, b)| a == b)
                    .count() as u128,
            );
        }
        if active[13 + i] {
            let data = if binary { &ones[..] } else { raw };
            values[13 + i] = Value::integer(
                data.iter()
                    .zip(&data[lag..])
                    .map(|(&a, &b)| a as u128 * b as u128)
                    .sum(),
            );
        }
    }
    if active[18] {
        let mut text = Vec::with_capacity(raw.len() * 4);
        for (i, x) in raw.iter().enumerate() {
            if i > 0 {
                text.push(b' ');
            }
            write!(&mut text, "{x}").map_err(PermutationError::Compression)?;
        }
        let mut encoder = BzEncoder::new(Vec::new(), Compression::new(5));
        encoder
            .write_all(&text)
            .map_err(PermutationError::Compression)?;
        values[18] = Value::integer(
            encoder
                .finish()
                .map_err(PermutationError::Compression)?
                .len() as u128,
        );
    }
    Ok(values)
}
/// Compute the 19 unpermuted statistics in PERMUTATION_NAMES order.
pub fn permutation_statistics(
    samples: &[u8],
    arithmetic: PermutationArithmetic,
) -> Result<Vec<Float>, PermutationError> {
    let ctx = Context::new(samples)?;
    Ok(statistics(samples, &ctx, &[true; 19])?
        .into_iter()
        .map(|v| v.float(arithmetic))
        .collect())
}
// xoshiro256** (Blackman/Vigna, public domain), with the same unbiased
// multiply-high rejection sampler and descending Fisher–Yates shuffle as NIST.
pub(crate) struct Random(pub(crate) [u64; 4]);
impl Random {
    pub(crate) fn next(&mut self) -> u64 {
        let s = &mut self.0;
        let result = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }
    fn below(&mut self, bound: u64) -> usize {
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let product = self.next() as u128 * bound as u128;
            if product as u64 >= threshold {
                return (product >> 64) as usize;
            }
        }
    }
    fn shuffle(&mut self, data: &mut [u8]) {
        for i in (1..data.len()).rev() {
            let j = self.below((i + 1) as u64);
            data.swap(i, j);
        }
    }
}
/// Snapshot emitted before work and after each permutation. Active statistics
/// have not yet accumulated enough observations in both inclusive tails.
#[derive(Clone, Debug)]
pub struct PermutationProgress {
    pub permutations_completed: usize,
    pub elapsed: std::time::Duration,
    pub active_statistics: Vec<&'static str>,
}
pub fn permutation_tests(samples: &[u8]) -> Result<PermutationAssessment, PermutationError> {
    permutation_tests_with_options(samples, PermutationOptions::default())
}
/// Run up to 10,000 permutations. Each statistic rejects when either inclusive
/// tail has at most five observations. Early stopping preserves that decision.
pub fn permutation_tests_with_options(
    samples: &[u8],
    options: PermutationOptions,
) -> Result<PermutationAssessment, PermutationError> {
    permutation_tests_with_progress(samples, options, &mut |_| {})
}
/// Same computation and RNG stream as `permutation_tests_with_options`.
/// The synchronous observer must return promptly; its time is included in elapsed time.
pub fn permutation_tests_with_progress(
    samples: &[u8],
    options: PermutationOptions,
    observer: &mut dyn FnMut(PermutationProgress),
) -> Result<PermutationAssessment, PermutationError> {
    let started = std::time::Instant::now();
    let ctx = Context::new(samples)?;
    let seed = if let Some(seed) = options.seed {
        seed
    } else {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| PermutationError::Randomness(e.to_string()))?;
        std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap()))
    };
    if seed == [0; 4] {
        return Err(PermutationError::ZeroSeed);
    }
    let mut rng = Random(seed);
    let mut data = samples.to_vec();
    let mut active = [true; 19];
    observer(PermutationProgress {
        permutations_completed: 0,
        elapsed: started.elapsed(),
        active_statistics: PERMUTATION_NAMES.to_vec(),
    });
    let original = statistics(samples, &ctx, &active)?;
    let mut tests: Vec<_> = original
        .iter()
        .enumerate()
        .map(|(i, &v)| PermutationTestResult {
            name: PERMUTATION_NAMES[i],
            original: v.float(options.arithmetic),
            greater: 0,
            equal: 0,
            less: 0,
            reject: false,
        })
        .collect();
    let mut permutations_performed = 0;
    for _ in 0..ROUNDS {
        rng.shuffle(&mut data);
        let shuffled = statistics(&data, &ctx, &active)?;
        permutations_performed += 1;
        for i in 0..19 {
            if active[i] {
                match shuffled[i].compare(original[i], options.arithmetic) {
                    Ordering::Greater => tests[i].greater += 1,
                    Ordering::Equal => tests[i].equal += 1,
                    Ordering::Less => tests[i].less += 1,
                }
                if options.early_stop && tests[i].passed() {
                    active[i] = false;
                }
            }
        }
        observer(PermutationProgress {
            permutations_completed: permutations_performed,
            elapsed: started.elapsed(),
            active_statistics: PERMUTATION_NAMES
                .iter()
                .zip(active)
                .filter_map(|(&name, enabled)| enabled.then_some(name))
                .collect(),
        });
        if active.iter().all(|&x| !x) {
            break;
        }
    }
    for test in &mut tests {
        test.reject = !test.passed();
    }
    let reject = tests.iter().any(|test| test.reject);
    Ok(PermutationAssessment {
        sample_count: samples.len(),
        alphabet: ctx.alphabet,
        seed,
        arithmetic: options.arithmetic,
        early_stop: options.early_stop,
        permutations_performed,
        tests,
        reject,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversions_keep_partial_block_with_zero_padding() {
        let (a, b) = conversions(&[1, 0, 1, 0, 1, 0, 1, 0, 1, 1]);
        assert_eq!(a, [4, 2]);
        assert_eq!(b, [170, 192]);
    }
    #[test]
    fn inclusive_tail_boundary_and_ties() {
        let mut r = PermutationTestResult {
            name: "test",
            original: Float::with_val(256, 0),
            greater: 5,
            equal: 0,
            less: 9995,
            reject: false,
        };
        assert!(!r.passed());
        r.equal = 1;
        assert!(r.passed());
        r.greater = 0;
        r.less = 0;
        r.equal = 6;
        assert!(r.passed());
        r.equal = 5;
        assert!(!r.passed());
    }
    #[test]
    fn shuffle_preserves_multiset_and_seed_is_reproducible() {
        let mut a: Vec<u8> = (0..=255).collect();
        let mut b = a.clone();
        Random([1, 2, 3, 4]).shuffle(&mut a);
        Random([1, 2, 3, 4]).shuffle(&mut b);
        assert_eq!(a, b);
        assert_ne!(a, (0..=255).collect::<Vec<u8>>());
        a.sort_unstable();
        assert_eq!(a, (0..=255).collect::<Vec<u8>>());
    }
    #[test]
    fn exact_ratio_ties() {
        let a = Value {
            numerator: 2,
            denominator: 3,
            reference: 0.0,
        };
        let b = Value {
            numerator: 4,
            denominator: 6,
            reference: 1.0,
        };
        assert_eq!(
            a.compare(b, PermutationArithmetic::ExactCounts),
            Ordering::Equal
        );
    }
}
