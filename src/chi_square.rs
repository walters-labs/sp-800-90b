//! Chi-square independence and goodness-of-fit tests (§5.2.1–§5.2.4).
use rug::{Float, float, ops::Pow};
use std::{error::Error, fmt};

/// Arithmetic used only for discrete tuple-length and binning decisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChiSquareGrouping {
    /// Preserve NIST's binary64 sorting, accumulation, and eligibility decisions.
    #[default]
    NistReference,
    /// Use exact count products and rational threshold comparisons.
    ExactCounts,
}

#[derive(Clone, Copy, Debug)]
pub struct ChiSquareOptions {
    pub precision_bits: u32,
    pub grouping: ChiSquareGrouping,
}
impl Default for ChiSquareOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
            grouping: ChiSquareGrouping::NistReference,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChiSquareError {
    InvalidPrecision {
        bits: u32,
    },
    TooFewSamples {
        count: usize,
    },
    ConstantInput,
    TooFewSubsequenceSamples {
        count: usize,
    },
    InsufficientGoodnessOfFitBins {
        bins: usize,
    },
    /// No m in 2..=11 has expected count >=5 for the least likely binary tuple.
    InsufficientBinaryCounts,
    InsufficientDegreesOfFreedom {
        bins: usize,
        alphabet_size: usize,
    },
    NumericalFailure,
}
impl fmt::Display for ChiSquareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewSubsequenceSamples { count } => write!(
                f,
                "ten subsequences require at least ten samples; got {count}"
            ),
            Self::InsufficientGoodnessOfFitBins { bins } => {
                write!(f, "goodness of fit requires at least two bins; got {bins}")
            }
            Self::InvalidPrecision { bits } => {
                write!(f, "precision {bits} is outside 64..={}", float::prec_max())
            }
            Self::TooFewSamples { count } => {
                write!(f, "at least two samples are required; got {count}")
            }
            Self::ConstantInput => {
                write!(f, "chi-square test requires at least two distinct symbols")
            }
            Self::InsufficientBinaryCounts => write!(
                f,
                "no binary tuple length 2..=11 has minimum expected count at least five"
            ),
            Self::InsufficientDegreesOfFreedom {
                bins,
                alphabet_size,
            } => write!(
                f,
                "{bins} bins and {alphabet_size} symbols leave no positive degrees of freedom"
            ),
            Self::NumericalFailure => write!(
                f,
                "chi-square probability calculation did not produce a finite value in [0,1]"
            ),
        }
    }
}
impl Error for ChiSquareError {}

#[derive(Clone, Debug)]
pub struct ChiSquareIndependence {
    pub sample_count: usize,
    pub grouping: ChiSquareGrouping,
    /// Original symbols in increasing order; indices are the remapped symbols.
    pub alphabet: Vec<u8>,
    /// Selected m for binary data, or 2 for nonbinary pairs.
    pub tuple_length: usize,
    pub block_count: usize,
    /// Tail excluded from observations, but still included in symbol proportions.
    pub discarded_samples: usize,
    /// Nonbinary pair (a,b) maps through pair_bins[a*k+b]; absent for binary data.
    pub pair_bins: Option<Vec<usize>>,
    /// Binary tuples are indexed by their MSB-first integer encoding.
    pub observed: Vec<usize>,
    pub expected: Vec<Float>,
    pub degrees_of_freedom: usize,
    pub statistic: Float,
    /// Upper-tail probability Q(df/2, statistic/2), evaluated with MPFR.
    pub p_value: Float,
    /// True precisely when p_value < 0.001. False does not establish IID.
    pub reject_independence: bool,
}

// For integer df, the gamma recurrence is a finite positive sum. This
// avoids cancellation and the very slow general gamma_inc path at large x.
fn chi_square_tail(statistic: &Float, df: usize) -> Float {
    let output_prec = statistic.prec();
    let prec = output_prec.saturating_add(32).min(float::prec_max());
    if statistic == &0 {
        return Float::with_val(output_prec, 1);
    }
    let x = Float::with_val(prec, statistic / 2);
    let exponential = (-x.clone()).exp();
    let tail = if df.is_multiple_of(2) {
        let mut term = exponential;
        let mut sum = term.clone();
        for j in 1..df / 2 {
            term *= &x;
            term /= j;
            sum += &term;
        }
        sum
    } else {
        let root = x.clone().sqrt();
        let mut sum = root.clone().erfc();
        let mut term = exponential * root;
        term /= Float::with_val(prec, rug::float::Constant::Pi).sqrt();
        term *= 2;
        for j in 0..df / 2 {
            sum += &term;
            term *= &x;
            term /= Float::with_val(prec, j) + 1.5;
        }
        sum
    };
    // Positive summation may round slightly above one when the lower tail
    // is below working precision. Respect the probability bound.
    Float::with_val(output_prec, tail.min(&Float::with_val(prec, 1)))
}

/// Test independence using 256-bit arithmetic and significance level 0.001.
/// Automatically rank-maps byte symbols and selects binary or nonbinary rules.
/// An error means the test could not be evaluated, never a statistical pass.
///
/// ```
/// use sp_800_90b::chi_square_independence;
/// let result = chi_square_independence(&[0, 1].repeat(500)).unwrap();
/// assert!(result.reject_independence);
/// ```
pub fn chi_square_independence(samples: &[u8]) -> Result<ChiSquareIndependence, ChiSquareError> {
    chi_square_independence_with_options(samples, ChiSquareOptions::default())
}
pub fn chi_square_independence_with_options(
    samples: &[u8],
    options: ChiSquareOptions,
) -> Result<ChiSquareIndependence, ChiSquareError> {
    let prec = options.precision_bits;
    if !(64..=float::prec_max()).contains(&prec) {
        return Err(ChiSquareError::InvalidPrecision { bits: prec });
    }
    let n = samples.len();
    if n < 2 {
        return Err(ChiSquareError::TooFewSamples { count: n });
    }
    let mut frequencies = [0usize; 256];
    for &s in samples {
        frequencies[s as usize] += 1;
    }
    let alphabet: Vec<u8> = (0..=255).filter(|&s| frequencies[s as usize] > 0).collect();
    let k = alphabet.len();
    if k < 2 {
        return Err(ChiSquareError::ConstantInput);
    }
    let mut ranks = [0usize; 256];
    for (rank, &symbol) in alphabet.iter().enumerate() {
        ranks[symbol as usize] = rank;
    }
    let (length, pair_bins, observed, expected, df) = if k == 2 {
        let least = frequencies[alphabet[0] as usize].min(frequencies[alphabet[1] as usize]);
        // Exact integer comparison using enough MPFR bits for n^11 * n.
        // This avoids rounding near the eligibility boundary of five.
        let exact_prec = usize::BITS * 12 + 3;
        let m = match options.grouping {
            ChiSquareGrouping::NistReference => {
                let p1 = frequencies[alphabet[1] as usize] as f64 / n as f64;
                let least = p1.min(1.0 - p1);
                (2..=11usize)
                    .rev()
                    .find(|&m| least.powi(m as i32) * (n / m) as f64 >= 5.0)
            }
            ChiSquareGrouping::ExactCounts => (2..=11usize).rev().find(|&m| {
                let left = Float::with_val(exact_prec, least).pow(m as u32) * (n / m);
                let right = Float::with_val(exact_prec, n).pow(m as u32) * 5;
                left >= right
            }),
        }
        .ok_or(ChiSquareError::InsufficientBinaryCounts)?;
        let blocks = n / m;
        let p0 = Float::with_val(
            prec,
            Float::with_val(prec, frequencies[alphabet[0] as usize]) / n,
        );
        let p1 = Float::with_val(
            prec,
            Float::with_val(prec, frequencies[alphabet[1] as usize]) / n,
        );
        let mut observed = vec![0usize; 1 << m];
        for tuple in samples.chunks_exact(m) {
            let index = tuple
                .iter()
                .fold(0, |value, &s| (value << 1) | ranks[s as usize]);
            observed[index] += 1;
        }
        let expected = (0..1usize << m)
            .map(|i| {
                let ones = i.count_ones();
                let mut e = p1.clone().pow(ones);
                e *= p0.clone().pow(m as u32 - ones);
                e *= blocks;
                e
            })
            .collect();
        (m, None, observed, expected, (1 << m) - 2)
    } else {
        let blocks = n / 2;
        // Common denominator n^2; sorting integer products preserves exact ties.
        let denominator = (n as u128) * (n as u128);
        let weights: Vec<u128> = (0..k * k)
            .map(|pair| {
                (frequencies[alphabet[pair / k] as usize] as u128)
                    * (frequencies[alphabet[pair % k] as usize] as u128)
            })
            .collect();
        let mut order: Vec<usize> = (0..k * k).collect();
        let reference_expectations: Vec<f64> = (0..k * k)
            .map(|pair| {
                let p = frequencies[alphabet[pair / k] as usize] as f64 / n as f64;
                let q = frequencies[alphabet[pair % k] as usize] as f64 / n as f64;
                p * q * blocks as f64
            })
            .collect();
        match options.grouping {
            ChiSquareGrouping::ExactCounts => {
                order.sort_unstable_by_key(|&pair| (weights[pair], pair))
            }
            ChiSquareGrouping::NistReference => order.sort_unstable_by(|&a, &b| {
                reference_expectations[a]
                    .total_cmp(&reference_expectations[b])
                    .then(a.cmp(&b))
            }),
        }
        // ceil(5*n^2/blocks), avoiding a potentially overflowing 5*n^2.
        let blocks128 = blocks as u128;
        let threshold =
            5 * (denominator / blocks128) + (5 * (denominator % blocks128)).div_ceil(blocks128);
        let mut bins = vec![0usize; k * k];
        let mut bin_weights = Vec::new();
        let mut current = 0u128;
        let mut current_reference = 0.0;
        let full = |weight: u128, reference: f64| match options.grouping {
            ChiSquareGrouping::ExactCounts => weight >= threshold,
            ChiSquareGrouping::NistReference => reference >= 5.0,
        };
        for &pair in &order {
            if full(current, current_reference) {
                bin_weights.push(current);
                current = 0;
                current_reference = 0.0;
            }
            bins[pair] = bin_weights.len();
            current += weights[pair];
            current_reference += reference_expectations[pair];
        }
        if !bin_weights.is_empty() && !full(current, current_reference) {
            let last = bin_weights.len();
            for bin in &mut bins {
                if *bin == last {
                    *bin -= 1;
                }
            }
            *bin_weights.last_mut().unwrap() += current;
        } else {
            bin_weights.push(current);
        }
        if bin_weights.len() <= k {
            return Err(ChiSquareError::InsufficientDegreesOfFreedom {
                bins: bin_weights.len(),
                alphabet_size: k,
            });
        }
        let mut observed = vec![0usize; bin_weights.len()];
        for pair in samples.chunks_exact(2) {
            observed[bins[ranks[pair[0] as usize] * k + ranks[pair[1] as usize]]] += 1;
        }
        let expected = bin_weights
            .iter()
            .map(|&weight| {
                let mut e = Float::with_val(prec, weight);
                e *= blocks;
                e /= Float::with_val(prec, denominator);
                e
            })
            .collect();
        let df = bin_weights.len() - k;
        (2, Some(bins), observed, expected, df)
    };
    let mut statistic = Float::with_val(prec, 0);
    for (&o, e) in observed.iter().zip(&expected) {
        let mut term = Float::with_val(prec, o);
        term -= e;
        term.square_mut();
        term /= e;
        statistic += term;
    }
    let p_value = chi_square_tail(&statistic, df);
    if !p_value.is_finite() || !(0..=1).contains(&p_value) {
        return Err(ChiSquareError::NumericalFailure);
    }
    let reject_independence = p_value < Float::with_val(prec, Float::parse("0.001").unwrap());
    Ok(ChiSquareIndependence {
        sample_count: n,
        grouping: options.grouping,
        alphabet,
        tuple_length: length,
        block_count: n / length,
        discarded_samples: n % length,
        pair_bins,
        observed,
        expected,
        degrees_of_freedom: df,
        statistic,
        p_value,
        reject_independence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upper_tail_closed_forms_and_tiny_probability() {
        for t in [1, 20, 2000] {
            let statistic = Float::with_val(256, t);
            let x = Float::with_val(256, &statistic / 2);
            let expected = (-x.clone()).exp();
            let actual = chi_square_tail(&statistic, 2);
            assert!(actual > 0);
            let relative: Float = Float::with_val(256, actual / &expected) - 1;
            assert!(relative.abs() < 1e-74);
            let expected4 = expected * (x + 1);
            let relative: Float =
                Float::with_val(256, chi_square_tail(&statistic, 4) / expected4) - 1;
            assert!(relative.abs() < 1e-74);
        }
    }
}

/// Goodness of fit across ten consecutive, equally sized subsequences.
#[derive(Clone, Debug)]
pub struct ChiSquareGoodnessOfFit {
    pub sample_count: usize,
    pub grouping: ChiSquareGrouping,
    pub alphabet: Vec<u8>,
    pub block_size: usize,
    /// Trailing samples excluded from blocks but included in global proportions.
    pub discarded_samples: usize,
    /// Alphabet rank to bin index. Binary inputs use two unmerged bins.
    pub symbol_bins: Vec<usize>,
    /// Counts indexed by subsequence, then bin.
    pub observed: Vec<Vec<usize>>,
    /// Expected counts for each bin in any one subsequence.
    pub expected: Vec<Float>,
    pub degrees_of_freedom: usize,
    pub statistic: Float,
    pub p_value: Float,
    /// True when p_value < 0.001; false alone does not establish IID.
    pub reject_goodness_of_fit: bool,
}

/// Compare symbol distributions across ten consecutive subsequences (§5.2.2,
/// §5.2.4), with 256-bit arithmetic and NIST-compatible grouping.
/// Binary inputs retain both symbols without a minimum expected-count rule,
/// matching the reference. Nonbinary bins are pooled to expectations >=5.
pub fn chi_square_goodness_of_fit(
    samples: &[u8],
) -> Result<ChiSquareGoodnessOfFit, ChiSquareError> {
    chi_square_goodness_of_fit_with_options(samples, ChiSquareOptions::default())
}

pub fn chi_square_goodness_of_fit_with_options(
    samples: &[u8],
    options: ChiSquareOptions,
) -> Result<ChiSquareGoodnessOfFit, ChiSquareError> {
    let prec = options.precision_bits;
    if !(64..=float::prec_max()).contains(&prec) {
        return Err(ChiSquareError::InvalidPrecision { bits: prec });
    }
    let n = samples.len();
    if n < 10 {
        return Err(ChiSquareError::TooFewSubsequenceSamples { count: n });
    }
    let block_size = n / 10;
    let mut counts = [0usize; 256];
    for &s in samples {
        counts[s as usize] += 1;
    }
    let alphabet: Vec<u8> = (0..=255).filter(|&s| counts[s as usize] > 0).collect();
    let k = alphabet.len();
    if k < 2 {
        return Err(ChiSquareError::ConstantInput);
    }
    let mut ranks = [0usize; 256];
    for (i, &s) in alphabet.iter().enumerate() {
        ranks[s as usize] = i;
    }
    let mut symbol_bins = vec![0usize; k];
    let bin_counts = if k == 2 {
        symbol_bins[1] = 1;
        alphabet
            .iter()
            .map(|&s| counts[s as usize])
            .collect::<Vec<_>>()
    } else {
        let mut order: Vec<usize> = (0..k).collect();
        // With a common denominator, count ordering is also expectation ordering.
        order.sort_unstable_by_key(|&i| (counts[alphabet[i] as usize], i));
        let mut bins = Vec::new();
        let mut current = 0usize;
        let mut reference = 0.0f64;
        let full = |count: usize, reference: f64| match options.grouping {
            ChiSquareGrouping::NistReference => reference >= 5.0,
            ChiSquareGrouping::ExactCounts => {
                (count as u128) * (block_size as u128) >= 5 * (n as u128)
            }
        };
        for i in order {
            if full(current, reference) {
                bins.push(current);
                current = 0;
                reference = 0.0;
            }
            symbol_bins[i] = bins.len();
            let count = counts[alphabet[i] as usize];
            current += count;
            reference += (count as f64 / n as f64) * block_size as f64;
        }
        if !bins.is_empty() && !full(current, reference) {
            let last = bins.len();
            for bin in &mut symbol_bins {
                if *bin == last {
                    *bin -= 1;
                }
            }
            *bins.last_mut().unwrap() += current;
        } else {
            bins.push(current);
        }
        bins
    };
    if bin_counts.len() < 2 {
        return Err(ChiSquareError::InsufficientGoodnessOfFitBins {
            bins: bin_counts.len(),
        });
    }
    let expected: Vec<Float> = bin_counts
        .iter()
        .map(|&count| {
            let mut e = Float::with_val(prec, count);
            e *= block_size;
            e /= n;
            e
        })
        .collect();
    let mut observed = vec![vec![0usize; expected.len()]; 10];
    let mut statistic = Float::with_val(prec, 0);
    for (block, row) in samples[..block_size * 10]
        .chunks_exact(block_size)
        .zip(&mut observed)
    {
        for &s in block {
            row[symbol_bins[ranks[s as usize]]] += 1;
        }
        for (&o, e) in row.iter().zip(&expected) {
            let mut term = Float::with_val(prec, o);
            term -= e;
            term.square_mut();
            term /= e;
            statistic += term;
        }
    }
    let degrees_of_freedom = 9 * (expected.len() - 1);
    let p_value = chi_square_tail(&statistic, degrees_of_freedom);
    if !p_value.is_finite() || !(0..=1).contains(&p_value) {
        return Err(ChiSquareError::NumericalFailure);
    }
    let reject_goodness_of_fit = p_value < Float::with_val(prec, Float::parse("0.001").unwrap());
    Ok(ChiSquareGoodnessOfFit {
        sample_count: n,
        grouping: options.grouping,
        alphabet,
        block_size,
        discarded_samples: n % 10,
        symbol_bins,
        observed,
        expected,
        degrees_of_freedom,
        statistic,
        p_value,
        reject_goodness_of_fit,
    })
}
