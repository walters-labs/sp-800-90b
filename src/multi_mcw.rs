// Reference-informed adaptation of NIST SP800-90B_EntropyAssessment.
// Modified in Rust, 2026-10-19–2026-10-20; see docs/provenance.md for sources
// and changes. Preserve the upstream notice in LICENSE-NIST (also in LICENSE).
use crate::{ConfidenceConstant, EstimateError};
use rug::{Float, float};

#[derive(Clone, Copy, Debug)]
pub struct MultiMcwOptions {
    pub precision_bits: u32,
    pub confidence_constant: ConfidenceConstant,
    /// Strictly increasing positive windows; overrides support teaching examples.
    pub windows: [usize; 4],
}
impl Default for MultiMcwOptions {
    fn default() -> Self {
        Self {
            precision_bits: 256,
            confidence_constant: ConfidenceConstant::NistReference,
            windows: [63, 255, 1023, 4095],
        }
    }
}
#[derive(Clone, Debug)]
pub struct MultiMcwEstimate {
    pub sample_count: usize,
    pub alphabet_size: usize,
    pub prediction_count: usize,
    pub correct_count: usize,
    /// Longest run of correct predictions; NIST's r is this value plus one.
    pub longest_correct_run: usize,
    pub scoreboard: [usize; 4],
    pub p_global: Float,
    pub p_global_upper: Float,
    /// None when the local bound cannot increase the final probability.
    pub p_local: Option<Float>,
    pub p_upper: Float,
    /// Bits per input symbol.
    pub min_entropy: Float,
}

/// Multi Most Common in Window prediction estimate (section 6.3.7).
/// Requires at least 4096 byte-valued samples; the alphabet is inferred.
pub fn multi_mcw_estimate(samples: &[u8]) -> Result<MultiMcwEstimate, EstimateError> {
    multi_mcw_estimate_with_options(samples, MultiMcwOptions::default())
}
pub fn multi_mcw_estimate_with_options(
    samples: &[u8],
    options: MultiMcwOptions,
) -> Result<MultiMcwEstimate, EstimateError> {
    let prec = options.precision_bits;
    if !(64..=float::prec_max()).contains(&prec) {
        return Err(EstimateError::InvalidPrecision { bits: prec });
    }
    let w = options.windows;
    if w[0] == 0 || !w.windows(2).all(|x| x[0] < x[1]) {
        return Err(EstimateError::InvalidPredictionWindows);
    }
    if samples.len() <= w[3] {
        return Err(EstimateError::TooFewPredictionSamples {
            count: samples.len(),
            window: w[3],
        });
    }
    let mut seen = [false; 256];
    for &s in samples {
        seen[s as usize] = true;
    }
    let alphabet: Vec<usize> = (0..256).filter(|&s| seen[s]).collect();
    let mut counts = [[0usize; 256]; 4];
    let mut positions = [[0usize; 256]; 4];
    let mut frequent = [0usize; 4];
    for j in 0..4 {
        for (i, &s) in samples[..w[j]].iter().enumerate() {
            let s = s as usize;
            counts[j][s] += 1;
            positions[j][s] = i;
            if counts[j][s] >= counts[j][frequent[j]] {
                frequent[j] = s;
            }
        }
    }
    let mut scoreboard = [0usize; 4];
    let (mut winner, mut correct, mut run, mut longest) = (0, 0, 0, 0);
    for i in w[0]..samples.len() {
        let incoming = samples[i] as usize;
        if frequent[winner] == incoming {
            correct += 1;
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
        for j in 0..4 {
            if i >= w[j] && frequent[j] == incoming {
                scoreboard[j] += 1;
                if scoreboard[j] >= scoreboard[winner] {
                    winner = j;
                }
            }
        }
        for j in 0..4 {
            if i < w[j] {
                continue;
            }
            let outgoing = samples[i - w[j]] as usize;
            counts[j][outgoing] -= 1;
            counts[j][incoming] += 1;
            positions[j][incoming] = i;
            if outgoing == frequent[j] {
                frequent[j] = *alphabet
                    .iter()
                    .max_by_key(|&&s| (counts[j][s], positions[j][s]))
                    .unwrap();
            } else if counts[j][incoming] >= counts[j][frequent[j]] {
                frequent[j] = incoming;
            }
        }
    }
    let n = samples.len() - w[0];
    let (p_global, p_global_upper, p_local, p_upper, min_entropy) = crate::prediction::estimate(
        correct,
        n,
        longest,
        alphabet.len(),
        prec,
        options.confidence_constant,
    );
    Ok(MultiMcwEstimate {
        sample_count: samples.len(),
        alphabet_size: alphabet.len(),
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
