//! Ranked bit selection from SP 800-90B §6.4.
use std::{error::Error, fmt};

/// A fixed, caller-supplied ranking for reducing 1..=128-bit unsigned samples
/// to 1..=8-bit symbols. The ranking must come from the source model; it is not
/// inferred from the observed data. Reuse the same mapping across datasets.
///
/// Positions are zero-based from the least significant bit. `ranking[0]` has
/// rank 1 (greatest assumed entropy contribution) and becomes the most
/// significant bit of the reduced symbol. The ranking includes every input
/// position exactly once, even positions not retained.
///
/// ```
/// use sp_800_90b::SymbolReduction;
/// let mut ranking = vec![3, 2, 1, 0];
/// ranking.extend(4..32);
/// let mapping = SymbolReduction::new(32, 4, &ranking)?;
/// assert_eq!(mapping.reduce(&[0x1234_abcd, 0x1234_abce])?, [13, 14]);
/// assert_eq!(mapping.entropy_ceiling(), 4);
/// # Ok::<(), sp_800_90b::SymbolReductionError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolReduction {
    input_bits: u8,
    output_bits: u8,
    ranking: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolReductionError {
    InvalidInputWidth { bits: u8 },
    InvalidOutputWidth { bits: u8, input_bits: u8 },
    RankingLength { expected: usize, actual: usize },
    PositionOutOfRange { rank: usize, position: u8 },
    DuplicatePosition { position: u8 },
    SampleOutOfRange { index: usize, value: u128, bits: u8 },
}
impl fmt::Display for SymbolReductionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInputWidth { bits } => {
                write!(f, "input width must be in 1..=128; got {bits}")
            }
            Self::InvalidOutputWidth { bits, input_bits } => write!(
                f,
                "output width must be in 1..=min(8, {input_bits}); got {bits}"
            ),
            Self::RankingLength { expected, actual } => {
                write!(f, "ranking must contain {expected} positions; got {actual}")
            }
            Self::PositionOutOfRange { rank, position } => write!(
                f,
                "rank {rank} selects out-of-range bit position {position}"
            ),
            Self::DuplicatePosition { position } => {
                write!(f, "bit position {position} occurs more than once")
            }
            Self::SampleOutOfRange { index, value, bits } => write!(
                f,
                "sample {index} has value {value}, which does not fit in {bits} bits"
            ),
        }
    }
}
impl Error for SymbolReductionError {}

impl SymbolReduction {
    pub fn new(
        input_bits: u8,
        output_bits: u8,
        ranking: &[u8],
    ) -> Result<Self, SymbolReductionError> {
        use SymbolReductionError::*;
        if !(1..=128).contains(&input_bits) {
            return Err(InvalidInputWidth { bits: input_bits });
        }
        if output_bits == 0 || output_bits > 8 || output_bits > input_bits {
            return Err(InvalidOutputWidth {
                bits: output_bits,
                input_bits,
            });
        }
        if ranking.len() != usize::from(input_bits) {
            return Err(RankingLength {
                expected: usize::from(input_bits),
                actual: ranking.len(),
            });
        }
        let mut seen = [false; 128];
        for (index, &position) in ranking.iter().enumerate() {
            if position >= input_bits {
                return Err(PositionOutOfRange {
                    rank: index + 1,
                    position,
                });
            }
            if std::mem::replace(&mut seen[usize::from(position)], true) {
                return Err(DuplicatePosition { position });
            }
        }
        Ok(Self {
            input_bits,
            output_bits,
            ranking: ranking.to_vec(),
        })
    }

    pub fn input_bits(&self) -> u8 {
        self.input_bits
    }
    /// Pass this width explicitly to the downstream assessment options, even
    /// if the observed values happen to use fewer bits.
    pub fn output_bits(&self) -> u8 {
        self.output_bits
    }
    pub fn ranking(&self) -> &[u8] {
        &self.ranking
    }
    /// Representation ceiling in bits per ORIGINAL sample, not an entropy
    /// estimate. Do not scale assessed entropy by input_bits / output_bits.
    pub fn entropy_ceiling(&self) -> u8 {
        self.output_bits
    }

    /// Produces one byte per input sample, preserving order and boundaries.
    /// Empty input produces empty output; assessment APIs enforce their own
    /// minimum lengths. Values outside the declared width are rejected even
    /// when the excess bits would have been discarded.
    ///
    /// Decode multi-byte files into unsigned integers with the source's byte
    /// order before calling this method; this API does not infer file encoding.
    pub fn reduce(&self, samples: &[u128]) -> Result<Vec<u8>, SymbolReductionError> {
        samples
            .iter()
            .enumerate()
            .map(|(index, &value)| {
                if self.input_bits < 128 && value >> self.input_bits != 0 {
                    return Err(SymbolReductionError::SampleOutOfRange {
                        index,
                        value,
                        bits: self.input_bits,
                    });
                }
                Ok(self.ranking[..usize::from(self.output_bits)]
                    .iter()
                    .fold(0u8, |symbol, &position| {
                        (symbol << 1) | ((value >> position) & 1) as u8
                    }))
            })
            .collect()
    }
}
