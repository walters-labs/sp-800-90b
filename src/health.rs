//! MPFR cutoff calculation and the default health-test convenience API (§4.4).
use crate::{HealthTestConfig, HealthTestError, HealthTestMonitor, HealthTestStatus};
use rug::{
    Float,
    float::{self, Round},
    ops::Pow,
};
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
impl HealthTestCutoffs {
    /// Export validated integer parameters for a dependency-free health monitor.
    ///
    /// Checks structural ranges and the alphabet's required window size, since
    /// these report fields are public. Does not re-derive cutoffs from entropy
    /// or verify that caller-modified values retain the intended significance.
    ///
    /// ```
    /// use sp_800_90b::{Float, HealthTestOptions, HealthTestMonitor, health_test_cutoffs};
    /// let cutoffs = health_test_cutoffs(&Float::with_val(256, 1), HealthTestOptions::default())?;
    /// let config = cutoffs.monitor_config()?;
    /// assert_eq!(config.repetition_count(), 21);
    /// assert_eq!(config.adaptive_proportion(), 589);
    /// let monitor = HealthTestMonitor::new(config);
    /// assert!(!monitor.status().startup_ready());
    /// # Ok::<(), sp_800_90b::HealthTestError>(())
    /// ```
    pub fn monitor_config(&self) -> Result<HealthTestConfig, HealthTestError> {
        let adaptive_proportion = u32::try_from(self.adaptive_proportion)
            .map_err(|_| HealthTestError::CutoffOutOfRange)?;
        let config = HealthTestConfig::new(
            self.options.alphabet_size,
            self.repetition_count,
            adaptive_proportion,
        )?;
        if self.window_size != config.window_size() {
            return Err(HealthTestError::InvalidWindowSize);
        }
        Ok(config)
    }
}

/// Streaming health tests with MPFR cutoff calculation and retained diagnostics.
///
/// Available with the `assessment` feature (enabled by default). Monitoring
/// delegates to [`HealthTestMonitor`], which is also available in `no_std` builds.
#[derive(Clone, Debug)]
pub struct HealthTests {
    cutoffs: HealthTestCutoffs,
    monitor: HealthTestMonitor,
}
impl HealthTests {
    pub fn new(h: &Float, options: HealthTestOptions) -> Result<Self, HealthTestError> {
        let cutoffs = health_test_cutoffs(h, options)?;
        let monitor = HealthTestMonitor::new(cutoffs.monitor_config()?);
        Ok(Self { cutoffs, monitor })
    }
    pub fn cutoffs(&self) -> &HealthTestCutoffs {
        &self.cutoffs
    }
    pub fn status(&self) -> &HealthTestStatus {
        self.monitor.status()
    }
    /// Explicitly restart both tests and startup counting, retaining the cutoffs.
    pub fn reset(&mut self) {
        self.monitor.reset();
    }
    /// Invalid alphabet expansion or counter overflow leaves this sample unconsumed.
    pub fn observe(&mut self, symbol: u8) -> Result<(), HealthTestError> {
        self.monitor.observe(symbol)
    }
    /// Stops on an input error, retaining the successfully processed prefix.
    pub fn observe_slice(&mut self, samples: &[u8]) -> Result<(), HealthTestError> {
        self.monitor.observe_slice(samples)
    }
}
