use crate::{ConfidenceConstant, EstimateError};
use rug::{Float, float};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub struct LagOptions {
    pub precision_bits: u32,
    pub confidence_constant: ConfidenceConstant,
    /// Standard value is 128. Overrides support educational examples.
    pub max_lag: usize,
}
impl Default for LagOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
            confidence_constant: ConfidenceConstant::NistReference,
            max_lag: 128,
        }
    }
}
#[derive(Clone, Debug)]
pub struct LagEstimate {
    pub sample_count: usize,
    pub alphabet_size: usize,
    pub prediction_count: usize,
    pub correct_count: usize,
    /// NIST's r is one greater than this length.
    pub longest_correct_run: usize,
    /// Scores for lags 1..=min(max_lag, sample_count-1); longer lags never activate.
    pub scoreboard: Vec<usize>,
    pub p_global: Float,
    pub p_global_upper: Float,
    /// None when the local bound cannot affect the result.
    pub p_local: Option<Float>,
    pub p_upper: Float,
    /// Bits per input symbol.
    pub min_entropy: Float,
}

/// Section 6.3.8 with 128 lag predictors and 256-bit MPFR arithmetic.
/// Requires three samples so the global confidence variance is defined.
/// The alphabet is inferred from distinct byte-valued symbols.
///
/// ```
/// use sp_800_90b::lag_estimate;
/// let result = lag_estimate(&[7; 100]).unwrap();
/// assert_eq!(result.correct_count, 99);
/// assert_eq!(result.min_entropy, 0);
/// ```
pub fn lag_estimate(samples: &[u8]) -> Result<LagEstimate, EstimateError> {
    lag_estimate_with_options(samples, LagOptions::default())
}

pub fn lag_estimate_with_options(
    samples: &[u8],
    options: LagOptions,
) -> Result<LagEstimate, EstimateError> {
    let prec = options.precision_bits;
    if !(64..=float::prec_max()).contains(&prec) {
        return Err(EstimateError::InvalidPrecision { bits: prec });
    }
    if options.max_lag == 0 {
        return Err(EstimateError::InvalidMaxLag);
    }
    if samples.len() < 3 {
        return Err(EstimateError::TooFewPredictions {
            count: samples.len().saturating_sub(1),
        });
    }
    let d = options.max_lag.min(samples.len() - 1);
    let mut positions: [VecDeque<usize>; 256] = std::array::from_fn(|_| VecDeque::new());
    let mut seen = [false; 256];
    seen[samples[0] as usize] = true;
    positions[samples[0] as usize].push_back(0);
    let mut scoreboard = vec![0usize; d];
    let (mut winner, mut correct, mut run, mut longest) = (1usize, 0, 0, 0);
    for i in 1..samples.len() {
        let symbol = samples[i] as usize;
        seen[symbol] = true;
        // Evaluate before updating scores: the winner was selected on prior data.
        if samples[i - winner] == samples[i] {
            correct += 1;
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
        let history = &mut positions[symbol];
        let cutoff = i.saturating_sub(d);
        while history.front().is_some_and(|&position| position < cutoff) {
            history.pop_front();
        }
        // Newest to oldest means increasing lag order, as in the specification.
        // Only successful predictors change scores; a tied success takes over.
        for &position in history.iter().rev() {
            let lag = i - position;
            scoreboard[lag - 1] += 1;
            if scoreboard[lag - 1] >= scoreboard[winner - 1] {
                winner = lag;
            }
        }
        if history.len() == d {
            history.pop_front();
        }
        history.push_back(i);
    }
    let n = samples.len() - 1;
    let k = seen.iter().filter(|&&present| present).count();
    let (p_global, p_global_upper, p_local, p_upper, min_entropy) =
        crate::prediction::estimate(correct, n, longest, k, prec, options.confidence_constant);
    Ok(LagEstimate {
        sample_count: samples.len(),
        alphabet_size: k,
        prediction_count: n,
        correct_count: correct,
        longest_correct_run: longest,
        scoreboard,
        p_global,
        p_global_upper,
        p_local,
        p_upper,
        min_entropy,
    })
}
