//! Component metadata for the January 2018 SP 800-90B §3.1.5.1.1 table.
//! These types describe algorithms; they neither execute cryptography nor
//! establish CAVP validation, correct key handling, or current approval status.
use crate::{
    ConditioningError, ConditioningEstimate, ConditioningKind, ConditioningOptions,
    conditioning_entropy,
};
use rug::Float;

/// Fixed-output FIPS 180-4 and FIPS 202 hash functions. SHAKE/XOFs are not
/// fixed-output hashes and are intentionally not represented. SHA-1 is retained
/// for publication-era modeling, not as a recommendation for new designs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditioningHash {
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
    Sha512_224,
    Sha512_256,
    Sha3_224,
    Sha3_256,
    Sha3_384,
    Sha3_512,
}
impl ConditioningHash {
    pub const fn output_bits(self) -> u32 {
        match self {
            Self::Sha1 => 160,
            Self::Sha224 | Self::Sha512_224 | Self::Sha3_224 => 224,
            Self::Sha256 | Self::Sha512_256 | Self::Sha3_256 => 256,
            Self::Sha384 | Self::Sha3_384 => 384,
            Self::Sha512 | Self::Sha3_512 => 512,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditioningAesKey {
    Bits128,
    Bits192,
    Bits256,
}
impl ConditioningAesKey {
    pub const fn bits(self) -> u32 {
        match self {
            Self::Bits128 => 128,
            Self::Bits192 => 192,
            Self::Bits256 => 256,
        }
    }
}
impl TryFrom<u32> for ConditioningAesKey {
    type Error = ConditioningError;
    fn try_from(bits: u32) -> Result<Self, Self::Error> {
        match bits {
            128 => Ok(Self::Bits128),
            192 => Ok(Self::Bits192),
            256 => Ok(Self::Bits256),
            _ => Err(ConditioningError::InvalidAesKeySize { bits }),
        }
    }
}

/// Typed dimensions from SP 800-90B Table 1. Selection asserts the described
/// implementation satisfies the cited algorithm specification and required
/// validation assurances. HMAC keys and CBC-MAC padding are not checked here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VettedConditioner {
    Hmac(ConditioningHash),
    Cmac(ConditioningAesKey),
    CbcMac(ConditioningAesKey),
    Hash(ConditioningHash),
    HashDf(ConditioningHash),
    BlockCipherDf(ConditioningAesKey),
}
impl VettedConditioner {
    /// Full output length, before optional truncation. For derivation functions
    /// this is also the required no_of_bits_to_return argument.
    pub const fn output_bits(self) -> u32 {
        match self {
            Self::Hmac(h) | Self::Hash(h) | Self::HashDf(h) => h.output_bits(),
            Self::Cmac(_) | Self::CbcMac(_) => 128,
            Self::BlockCipherDf(k) => k.bits(),
        }
    }
    /// Nominal narrowest internal width from Table 1; the numerical formula
    /// still clamps this to the input width when necessary.
    pub const fn internal_width(self) -> u32 {
        self.output_bits()
    }

    /// Check externally supplied full-output dimensions against the table.
    /// Truncated lengths belong in ConditioningOptions::retained_output_bits.
    pub fn validate_dimensions(
        self,
        output_bits: u32,
        internal_width: u32,
    ) -> Result<(), ConditioningError> {
        if output_bits != self.output_bits() || internal_width != self.internal_width() {
            return Err(ConditioningError::ComponentDimensionsMismatch);
        }
        Ok(())
    }

    /// Derive dimensions and calculate the entropy bound, retaining component
    /// identity in the report. Input lengths count primary-source bits only.
    ///
    /// ```
    /// use rug::Float;
    /// use sp_800_90b::{VettedConditioner, ConditioningHash, ConditioningOptions};
    /// let component = VettedConditioner::Hash(ConditioningHash::Sha256);
    /// let report = component.assess(512, &Float::with_val(256, 400), ConditioningOptions::default())?;
    /// assert_eq!(report.output_bits, 256);
    /// assert_eq!(report.vetted_component, Some(component));
    /// # Ok::<(), sp_800_90b::ConditioningError>(())
    /// ```
    pub fn assess(
        self,
        input_bits: u32,
        input_entropy: &Float,
        options: ConditioningOptions,
    ) -> Result<ConditioningEstimate, ConditioningError> {
        let mut report = conditioning_entropy(
            input_bits,
            self.output_bits(),
            self.internal_width(),
            input_entropy,
            ConditioningKind::Vetted,
            options,
        )?;
        report.vetted_component = Some(self);
        Ok(report)
    }
}
