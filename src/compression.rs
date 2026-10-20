// Reference-informed adaptation of NIST SP800-90B_EntropyAssessment.
// Modified in Rust, 2026-10-19–2026-10-20; see docs/provenance.md for sources
// and changes. Preserve the upstream notice in LICENSE-NIST (also in LICENSE).
use crate::most_common_value::confidence_value;
use crate::{ConfidenceConstant, EstimateError};
use rug::{Float, float};

#[derive(Clone, Copy, Debug)]
pub struct CompressionOptions {
    pub precision_bits: u32,
    pub confidence_constant: ConfidenceConstant,
    /// Standard value is 1000; override only for educational examples.
    pub dictionary_blocks: usize,
}
impl Default for CompressionOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
            confidence_constant: ConfidenceConstant::NistReference,
            dictionary_blocks: 1000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompressionEstimate {
    pub block_count: usize,
    pub test_blocks: usize,
    pub discarded_bits: usize,
    pub mean: Float,
    pub standard_deviation: Float,
    pub lower_bound: Float,
    pub p: Float,
    /// Bits per input bit, after dividing block entropy by six.
    pub min_entropy: Float,
}

/// Section 6.3.4: one binary sample per byte, six-bit blocks, 1000 training blocks.
/// At least two test blocks are required (6012 input bits). Trailing partial
/// blocks are ignored after validating the entire input.
pub fn compression_estimate(samples: &[u8]) -> Result<CompressionEstimate, EstimateError> {
    compression_estimate_with_options(samples, CompressionOptions::default())
}

fn distances(samples: &[u8], d: usize) -> Vec<usize> {
    let mut last_seen = [0usize; 64];
    let mut result = Vec::with_capacity(samples.len() / 6 - d);
    for (index, bits) in samples.chunks_exact(6).enumerate() {
        let symbol = bits
            .iter()
            .fold(0usize, |value, &bit| value * 2 + usize::from(bit));
        let position = index + 1;
        if index >= d {
            result.push(position - last_seen[symbol]);
        }
        last_seen[symbol] = position;
    }
    result
}

// Shared log cache avoids recomputing MPFR logarithms at every bisection step.
struct Model {
    precision: u32,
    d: usize,
    n: usize,
    logs: Vec<Float>,
    log_n: Float,
    tolerance: Float,
}
impl Model {
    fn new(precision: u32, d: usize, n: usize) -> Self {
        let mut tolerance = Float::with_val(precision, 1);
        tolerance >>= precision;
        tolerance >>= 16;
        Self {
            precision,
            d,
            n,
            logs: vec![Float::with_val(precision, 0)],
            log_n: Float::with_val(precision, n).log2(),
            tolerance,
        }
    }
    fn log(&mut self, u: usize) -> Float {
        while self.logs.len() < u {
            self.logs
                .push(Float::with_val(self.precision, self.logs.len() + 1).log2());
        }
        self.logs[u - 1].clone()
    }
    fn g(&mut self, z: &Float) -> Float {
        let prec = self.precision;
        if z == &0 || z == &1 {
            return Float::with_val(prec, 0);
        }
        let v = self.n - self.d;
        let mut r = Float::with_val(prec, 1);
        r -= z;
        let mut power = r.clone();
        let mut sum = Float::with_val(prec, 0);
        let mut tail_factor = Float::with_val(prec, 1);
        tail_factor /= v;
        tail_factor += z;
        tail_factor *= &self.log_n;
        // Interchange the two finite sums. Each u<t contribution occurs
        // min(v,n-u) times; the u=t contribution occurs once when u>d.
        for u in 2..=self.n {
            let mut weight = Float::with_val(prec, z * z);
            weight *= v.min(self.n - u);
            if u > self.d {
                weight += z;
            }
            let mut term = self.log(u);
            term *= &power;
            term *= weight;
            sum += term;
            power *= &r;
            // Remaining G terms <= log2(n)*r^u*(z+1/v).
            // Test periodically; this bounds truncation, not all rounding error.
            if u % 32 == 0 && Float::with_val(prec, &power * &tail_factor) < self.tolerance {
                break;
            }
        }
        sum /= v;
        sum
    }
    fn expected(&mut self, p: &Float) -> Float {
        let mut q = Float::with_val(self.precision, 1);
        q -= p;
        q /= 63;
        let mut result = self.g(&q);
        result *= 63;
        result += self.g(p);
        result
    }
}

/// Configurable compression estimate using MPFR and the pinned NIST reference
/// boundary behavior. The reference multiplier 0.5907 is preserved as binary64;
/// Publication uses the exact decimal coefficient as well as decimal 2.576.
pub fn compression_estimate_with_options(
    samples: &[u8],
    options: CompressionOptions,
) -> Result<CompressionEstimate, EstimateError> {
    let prec = options.precision_bits;
    if !(64..=float::prec_max()).contains(&prec) {
        return Err(EstimateError::InvalidPrecision { bits: prec });
    }
    let d = options.dictionary_blocks;
    if d == 0 {
        return Err(EstimateError::InvalidDictionarySize);
    }
    for (index, &value) in samples.iter().enumerate() {
        if value > 1 {
            return Err(EstimateError::NonBinarySample { index, value });
        }
    }
    let n = samples.len() / 6;
    if n.saturating_sub(d) < 2 {
        return Err(EstimateError::TooFewTestBlocks {
            blocks: n,
            dictionary_blocks: d,
        });
    }
    let v = n - d;
    let mut model = Model::new(prec, d, n);
    let logs: Vec<Float> = distances(samples, d)
        .into_iter()
        .map(|x| Float::with_val(prec, x).log2())
        .collect();
    let mut mean = Float::with_val(prec, 0);
    for x in &logs {
        mean += x;
    }
    mean /= v;
    // Algebraically sum(log(D)^2)/(v-1) - mean^2, using centered terms
    // to avoid cancellation. This is deliberately NOT ordinary sample variance.
    let mut variance = Float::with_val(prec, &mean * &mean);
    for x in &logs {
        let delta = Float::with_val(prec, x - &mean);
        variance += Float::with_val(prec, &delta * &delta);
    }
    variance /= v - 1;
    let mut standard_deviation = variance.sqrt();
    let c = match options.confidence_constant {
        ConfidenceConstant::NistReference => Float::with_val(prec, 0.5907_f64),
        ConfidenceConstant::Publication => {
            let mut c = Float::with_val(prec, 5907);
            c /= 10000;
            c
        }
    };
    standard_deviation *= c;
    let mut adjustment = standard_deviation.clone();
    adjustment *= confidence_value(options.confidence_constant, prec);
    adjustment /= Float::with_val(prec, v).sqrt();
    let lower_bound = Float::with_val(prec, &mean - adjustment);
    let mut low = Float::with_val(prec, 1);
    low /= 64;
    let mut high = Float::with_val(prec, 1);
    let p = if lower_bound <= 0 {
        // Reference search converges to p=1 for a nonpositive target.
        high
    } else if lower_bound >= model.expected(&low) {
        low
    } else {
        // Stop only when rounding prevents further bisection at this precision.
        loop {
            let mut mid = Float::with_val(prec, &low + &high);
            mid /= 2;
            if mid == low || mid == high {
                break;
            }
            if model.expected(&mid) > lower_bound {
                low = mid;
            } else {
                high = mid;
            }
        }
        high
    };
    let min_entropy = if p == 1 {
        Float::with_val(prec, 0)
    } else {
        let mut h = -p.clone().log2();
        h /= 6;
        h
    };
    Ok(CompressionEstimate {
        block_count: n,
        test_blocks: v,
        discarded_bits: samples.len() % 6,
        mean,
        standard_deviation,
        lower_bound,
        p,
        min_entropy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn g_matches_literal_double_sum() {
        for (d, n) in [(1, 3), (4, 8), (7, 12), (4, 300)] {
            for value in [0.0, 1e-10, 1.0 / 64.0, 0.3, 0.9, 1.0] {
                let z = Float::with_val(256, value);
                let mut r = Float::with_val(256, 1);
                r -= &z;
                let mut expected = Float::with_val(256, 0);
                for t in d + 1..=n {
                    let mut power = Float::with_val(256, 1);
                    for u in 1..=t {
                        let mut term = Float::with_val(256, u).log2();
                        term *= &power;
                        term *= &z;
                        if u < t {
                            term *= &z;
                        }
                        expected += term;
                        power *= &r;
                    }
                }
                expected /= n - d;
                let actual = Model::new(256, d, n).g(&z);
                assert!(
                    (actual - expected).abs()
                        < Float::with_val(256, Float::parse("1e-73").unwrap())
                );
            }
        }
    }
    #[test]
    fn recurrence_updates_and_first_occurrence() {
        let blocks = [5u8, 50, 5, 50, 50, 7, 7];
        let bits: Vec<u8> = blocks
            .iter()
            .flat_map(|x| (0..6).rev().map(move |i| (x >> i) & 1))
            .collect();
        assert_eq!(distances(&bits, 3), [2, 1, 6, 1]);
    }
}
