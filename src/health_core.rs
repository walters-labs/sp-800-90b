//! Allocation-free health-test configuration and streaming state (§4.4).
use core::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HealthTestError {
    InvalidAlphabet,
    InvalidEntropy,
    InvalidPrecision,
    InvalidFalsePositiveExponent,
    CutoffOutOfRange,
    InvalidWindowSize,
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

/// Validated integer parameters for streaming health tests.
///
/// Construct on a host or embed precomputed values in firmware. Validation checks
/// the alphabet and cutoff ranges only: the caller must establish that these
/// cutoffs match the source's assessed entropy and chosen false-positive rate.
/// With `assessment` enabled, `HealthTestCutoffs::monitor_config` exports these
/// parameters from the MPFR calculation.
///
/// Fields are private so an accepted configuration cannot be made invalid.
/// Window size follows §4.4: 1024 for binary sources, 512 otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HealthTestConfig {
    alphabet_size: u16,
    repetition_count: u32,
    adaptive_proportion: u32,
}
impl HealthTestConfig {
    /// Accept an alphabet of 2..=256 symbols, RCT cutoff >=2 and APT cutoff
    /// in 2..=W+1. A cutoff of W+1 is valid for very low entropy; in that case
    /// APT cannot reject within a window, while RCT still operates.
    /// Both tests reject when their counts reach the supplied cutoff.
    pub const fn new(
        alphabet_size: u16,
        repetition_count: u32,
        adaptive_proportion: u32,
    ) -> Result<Self, HealthTestError> {
        if alphabet_size < 2 || alphabet_size > 256 {
            return Err(HealthTestError::InvalidAlphabet);
        }
        let window_size = if alphabet_size == 2 { 1024 } else { 512 };
        if repetition_count < 2 || adaptive_proportion < 2 || adaptive_proportion > window_size + 1
        {
            return Err(HealthTestError::CutoffOutOfRange);
        }
        Ok(Self {
            alphabet_size,
            repetition_count,
            adaptive_proportion,
        })
    }
    /// Size of the source alphabet, not the alphabet observed in one chunk.
    pub const fn alphabet_size(&self) -> u16 {
        self.alphabet_size
    }
    /// Repetition Count Test rejection threshold.
    pub const fn repetition_count(&self) -> u32 {
        self.repetition_count
    }
    /// Adaptive Proportion Test rejection threshold.
    pub const fn adaptive_proportion(&self) -> u32 {
        self.adaptive_proportion
    }
    /// Fixed, non-overlapping APT window length in raw samples.
    pub const fn window_size(&self) -> usize {
        if self.alphabet_size == 2 { 1024 } else { 512 }
    }
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
/// Allocation-free streaming RCT and non-overlapping APT windows (§4.4).
///
/// Available without default features, using only `core`: no heap, floating-point
/// arithmetic, operating system or external dependencies. Samples are raw `u8`
/// noise-source symbols; sparse symbol values are accepted up to the configured
/// alphabet size. Chunk boundaries never reset state. Failures latch; subsequent
/// samples remain tested for diagnostics. The caller must gate startup output
/// and handle failures; this monitor does not generate or release entropy.
///
/// ```
/// use sp_800_90b::{HealthTestConfig, HealthTestMonitor};
/// // Precomputed for binary symbols, H=1 bit/sample and alpha=2^-20.
/// let config = HealthTestConfig::new(2, 21, 589)?;
/// let mut monitor = HealthTestMonitor::new(config);
/// for _ in 0..512 {
///     monitor.observe_slice(&[0, 1])?;
/// }
/// assert!(monitor.status().startup_ready());
/// # Ok::<(), sp_800_90b::HealthTestError>(())
/// ```
#[derive(Clone, Debug)]
pub struct HealthTestMonitor {
    config: HealthTestConfig,
    status: HealthTestStatus,
    previous: Option<u8>,
    run: u32,
    reference: u8,
    matches: u32,
    seen: [bool; 256],
    distinct: u16,
}
impl HealthTestMonitor {
    /// Start a monitor with structurally validated, precomputed cutoffs.
    pub const fn new(config: HealthTestConfig) -> Self {
        Self {
            config,
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
        }
    }
    /// The immutable parameters retained across resets.
    pub const fn config(&self) -> &HealthTestConfig {
        &self.config
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
            if self.distinct == self.config.alphabet_size {
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
        if self.run >= self.config.repetition_count
            && self.status.first_repetition_failure.is_none()
        {
            self.status.first_repetition_failure = Some(HealthTestFailure {
                test: HealthTestKind::RepetitionCount,
                sample_index: index,
                count: self.run,
                cutoff: self.config.repetition_count,
            });
        }
        if self.status.samples_in_window == 0 {
            self.reference = symbol;
            self.matches = 1;
        } else if symbol == self.reference {
            self.matches += 1;
        }
        if self.matches >= self.config.adaptive_proportion
            && self.status.first_adaptive_failure.is_none()
        {
            self.status.first_adaptive_failure = Some(HealthTestFailure {
                test: HealthTestKind::AdaptiveProportion,
                sample_index: index,
                count: self.matches,
                cutoff: self.config.adaptive_proportion,
            });
        }
        self.status.samples_seen += 1;
        self.status.samples_in_window += 1;
        if self.status.samples_in_window == self.config.window_size() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_counter_overflow_does_not_change_stream_state() {
        let config = HealthTestConfig::new(2, 21, 589).unwrap();
        let mut monitor = HealthTestMonitor::new(config);
        monitor.observe(7).unwrap();
        monitor.status.samples_seen = u64::MAX;
        let before = monitor.clone();
        assert_eq!(
            monitor.observe(9),
            Err(HealthTestError::SampleCountOverflow)
        );
        assert_eq!(monitor.status, before.status);
        assert_eq!(monitor.seen, before.seen);
        assert_eq!(monitor.distinct, before.distinct);
        assert_eq!(monitor.previous, before.previous);
        assert_eq!(monitor.run, before.run);
        assert_eq!(monitor.reference, before.reference);
        assert_eq!(monitor.matches, before.matches);
    }

    #[test]
    fn maximum_repetition_cutoff_triggers_and_count_saturates() {
        let config = HealthTestConfig::new(2, u32::MAX, 1025).unwrap();
        let mut monitor = HealthTestMonitor::new(config);
        monitor.previous = Some(0);
        monitor.run = u32::MAX - 1;
        monitor.status.samples_seen = u64::from(u32::MAX) - 1;
        monitor.observe(0).unwrap();
        let failure = monitor.status.first_repetition_failure.clone().unwrap();
        assert_eq!(failure.count, u32::MAX);
        assert_eq!(failure.cutoff, u32::MAX);
        assert_eq!(failure.sample_index, u64::from(u32::MAX) - 1);
        monitor.observe(0).unwrap();
        assert_eq!(monitor.run, u32::MAX);
        assert_eq!(monitor.status.first_repetition_failure, Some(failure));
    }
}
