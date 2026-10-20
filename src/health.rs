//! Stateful approved health tests (§4.4), on raw noise-source symbols.
use rug::{
    Float,
    float::{self, Round},
    ops::Pow,
};
use std::{error::Error, fmt};
#[derive(Clone, Copy, Debug)]
pub struct HealthTestOptions {
    /// Noise source alphabet size, not the number observed in one chunk.
    pub alphabet_size: u16,
    /// alpha = 2^(-false_positive_exponent), default 2^-20.
    pub false_positive_exponent: u32,
    pub precision_bits: u32,
}
impl Default for HealthTestOptions {
    fn default() -> Self {
        Self {
            alphabet_size: 2,
            false_positive_exponent: 20,
            precision_bits: 256,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HealthTestError {
    InvalidAlphabet,
    InvalidEntropy,
    InvalidPrecision,
    InvalidFalsePositiveExponent,
    CutoffOutOfRange,
    AlphabetExceeded,
    SampleCountOverflow,
    NumericalFailure,
}
impl fmt::Display for HealthTestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "health test error: {self:?}")
    }
}
impl Error for HealthTestError {}
#[derive(Clone, Debug)]
pub struct HealthTestCutoffs {
    pub options: HealthTestOptions,
    pub entropy_per_sample: Float,
    pub alpha: Float,
    pub repetition_count: u32,
    pub adaptive_proportion: usize,
    pub window_size: usize,
    /// Binomial(W,2^-H) upper tail at C, per the specification's cutoff formula.
    pub adaptive_tail_probability: Float,
}
/// Derive RCT C=1+ceil(-log2(alpha)/H) and the APT cutoff from the
/// publication's 1+CRITBINOM(W,2^-H,1-alpha) convention (Table 2).
pub fn health_test_cutoffs(
    h: &Float,
    options: HealthTestOptions,
) -> Result<HealthTestCutoffs, HealthTestError> {
    if !(2..=256).contains(&options.alphabet_size) {
        return Err(HealthTestError::InvalidAlphabet);
    }
    if !(64..=float::prec_max().saturating_sub(64)).contains(&options.precision_bits) {
        return Err(HealthTestError::InvalidPrecision);
    }
    if !(1..=128).contains(&options.false_positive_exponent) {
        return Err(HealthTestError::InvalidFalsePositiveExponent);
    }
    let precision = options
        .precision_bits
        .max(h.prec())
        .saturating_add(64)
        .min(float::prec_max());
    if !h.is_finite() || h <= &0 || h > &Float::with_val(precision, options.alphabet_size).log2() {
        return Err(HealthTestError::InvalidEntropy);
    }
    let ratio = Float::with_val_round(
        precision,
        &Float::with_val(precision, options.false_positive_exponent) / h,
        Round::Up,
    )
    .0
    .ceil();
    if ratio >= u32::MAX {
        return Err(HealthTestError::CutoffOutOfRange);
    }
    let repetition_count = ratio
        .to_u32_saturating()
        .ok_or(HealthTestError::CutoffOutOfRange)?
        + 1;
    let window_size = if options.alphabet_size == 2 {
        1024
    } else {
        512
    };
    let mut alpha = Float::with_val(precision, 1);
    alpha >>= options.false_positive_exponent;
    let p = Float::with_val(precision, -h).exp2();
    // Avoid cancellation for very small positive H.
    let q = -Float::with_val(
        precision,
        -Float::with_val(precision, h * Float::with_val(precision, 2).ln()),
    )
    .exp_m1();
    let mut mass = p.clone().pow(window_size as u32);
    let mut tails = vec![Float::with_val(precision, 0); window_size + 2];
    for k in (0..=window_size).rev() {
        tails[k] = Float::with_val(precision, &tails[k + 1] + &mass);
        if k > 0 {
            mass *= k;
            mass /= window_size - k + 1;
            mass *= &q;
            mass /= &p;
        }
    }
    if !tails[0].is_finite()
        || Float::with_val(precision, &tails[0] - 1).abs() > Float::with_val(precision, 1e-12)
    {
        return Err(HealthTestError::NumericalFailure);
    }
    let adaptive_proportion = (1..=window_size + 1).find(|&c| tails[c] <= alpha).unwrap();
    Ok(HealthTestCutoffs {
        options,
        entropy_per_sample: Float::with_val(options.precision_bits, h),
        alpha: Float::with_val(options.precision_bits, alpha),
        repetition_count,
        adaptive_proportion,
        window_size,
        adaptive_tail_probability: Float::with_val(
            options.precision_bits,
            &tails[adaptive_proportion],
        ),
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthTestKind {
    RepetitionCount,
    AdaptiveProportion,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HealthTestFailure {
    pub test: HealthTestKind,
    /// Zero-based index since construction or explicit reset.
    pub sample_index: u64,
    pub count: u32,
    pub cutoff: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HealthTestStatus {
    pub samples_seen: u64,
    pub completed_windows: u64,
    pub samples_in_window: usize,
    pub first_repetition_failure: Option<HealthTestFailure>,
    pub first_adaptive_failure: Option<HealthTestFailure>,
}
impl HealthTestStatus {
    pub fn failed(&self) -> bool {
        self.first_repetition_failure.is_some() || self.first_adaptive_failure.is_some()
    }
    /// At least 1024 startup samples have been tested, with no latched failure.
    /// The caller must withhold startup output and enforce the failure response.
    pub fn startup_ready(&self) -> bool {
        self.samples_seen >= 1024 && !self.failed()
    }
}
/// Streaming RCT and non-overlapping APT windows. Chunk boundaries never reset
/// test state. Failures latch; subsequent samples remain tested for diagnostics.
#[derive(Clone, Debug)]
pub struct HealthTests {
    cutoffs: HealthTestCutoffs,
    status: HealthTestStatus,
    previous: Option<u8>,
    run: u32,
    reference: u8,
    matches: u32,
    seen: [bool; 256],
    distinct: u16,
}
impl HealthTests {
    pub fn new(h: &Float, options: HealthTestOptions) -> Result<Self, HealthTestError> {
        Ok(Self {
            cutoffs: health_test_cutoffs(h, options)?,
            status: HealthTestStatus {
                samples_seen: 0,
                completed_windows: 0,
                samples_in_window: 0,
                first_repetition_failure: None,
                first_adaptive_failure: None,
            },
            previous: None,
            run: 0,
            reference: 0,
            matches: 0,
            seen: [false; 256],
            distinct: 0,
        })
    }
    pub fn cutoffs(&self) -> &HealthTestCutoffs {
        &self.cutoffs
    }
    pub fn status(&self) -> &HealthTestStatus {
        &self.status
    }
    /// Explicitly restart both tests and startup counting, retaining the cutoffs.
    pub fn reset(&mut self) {
        self.status = HealthTestStatus {
            samples_seen: 0,
            completed_windows: 0,
            samples_in_window: 0,
            first_repetition_failure: None,
            first_adaptive_failure: None,
        };
        self.previous = None;
        self.run = 0;
        self.matches = 0;
        self.reference = 0;
        self.seen.fill(false);
        self.distinct = 0;
    }
    /// Invalid alphabet expansion or counter overflow leaves this sample unconsumed.
    pub fn observe(&mut self, symbol: u8) -> Result<(), HealthTestError> {
        if self.status.samples_seen == u64::MAX {
            return Err(HealthTestError::SampleCountOverflow);
        }
        if !self.seen[symbol as usize] {
            if self.distinct == self.cutoffs.options.alphabet_size {
                return Err(HealthTestError::AlphabetExceeded);
            }
            self.seen[symbol as usize] = true;
            self.distinct += 1;
        }
        let index = self.status.samples_seen;
        self.run = if self.previous == Some(symbol) {
            self.run.saturating_add(1)
        } else {
            1
        };
        self.previous = Some(symbol);
        if self.run >= self.cutoffs.repetition_count
            && self.status.first_repetition_failure.is_none()
        {
            self.status.first_repetition_failure = Some(HealthTestFailure {
                test: HealthTestKind::RepetitionCount,
                sample_index: index,
                count: self.run,
                cutoff: self.cutoffs.repetition_count,
            });
        }
        if self.status.samples_in_window == 0 {
            self.reference = symbol;
            self.matches = 1;
        } else if symbol == self.reference {
            self.matches += 1;
        }
        if self.matches as usize >= self.cutoffs.adaptive_proportion
            && self.status.first_adaptive_failure.is_none()
        {
            self.status.first_adaptive_failure = Some(HealthTestFailure {
                test: HealthTestKind::AdaptiveProportion,
                sample_index: index,
                count: self.matches,
                cutoff: self.cutoffs.adaptive_proportion as u32,
            });
        }
        self.status.samples_seen += 1;
        self.status.samples_in_window += 1;
        if self.status.samples_in_window == self.cutoffs.window_size {
            self.status.completed_windows += 1;
            self.status.samples_in_window = 0;
        }
        Ok(())
    }
    /// Stops on an input error, retaining the successfully processed prefix.
    pub fn observe_slice(&mut self, samples: &[u8]) -> Result<(), HealthTestError> {
        for &s in samples {
            self.observe(s)?;
        }
        Ok(())
    }
}
