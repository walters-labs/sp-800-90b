//! Entropy assessment and streaming noise-source health tests (NIST SP 800-90B).
//!
//! Default features provide the full assessment API and CLI. Disable default
//! features for an allocation-free `no_std` library with no external dependencies:
//! [`HealthTestConfig`], [`HealthTestMonitor`], and their status/error types.
//! Supply precomputed integer cutoffs; this minimal build does not estimate
//! entropy or calculate cutoffs. The `assessment` feature adds those calculations
//! using `std` and MPFR; `cli` adds the command-line tool and enables `assessment`.
#![cfg_attr(not(feature = "assessment"), no_std)]
#![cfg_attr(
    feature = "assessment",
    doc = r#"
NIST SP 800-90B entropy assessment for byte-valued samples.

# Choose a workflow

| Task | Entry point |
| --- | --- |
| Estimate entropy without an IID claim | [`assess_non_iid`] |
| Check an IID claim and estimate entropy | [`assess_iid`] |
| Assess a 1000×1000 restart matrix | [`assess_restart`] |
| Coordinate sequential data, restart data, submitter bound and conditioning | [`assess_validation`] |
| Calculate conditioning bounds from known entropy | [`conditioning_entropy`] or [`VettedConditioner::assess`] |
| Monitor a stream using RCT/APT | [`HealthTests`] |
| Reduce ranked symbols wider than a byte | [`SymbolReduction`] |

All ten individual estimators (§6.3.1–6.3.10) are also exported at the crate
root. Options implement `Default`; override only the settings you need.
IID, permutation and validation APIs have synchronous `_with_progress`
variants. Reading files and storing reports belong to the caller; the CLI
supplies those conveniences separately.

# Handling results

An `Ok(report)` can contain rejected or incomplete evidence. Use
`eligible_entropy()` to borrow a completed, non-rejected estimate, and keep
the report for warnings and diagnostic results. Small exploratory datasets
can produce complete estimates without meeting validation sample requirements.

```
use sp_800_90b::{assess_non_iid, NonIidOptions};
let report = assess_non_iid(&[0, 1, 1, 0, 1, 0], NonIidOptions::default())?;
match report.eligible_entropy() {
    Some(h) => println!("{h} bits per input symbol"),
    None => println!("Incomplete estimation: inspect the estimator outcomes"),
}
for warning in &report.warnings {
    eprintln!("{warning:?}");
}
# Ok::<(), sp_800_90b::AssessmentError>(())
```

# Representation, units and precision

Samples are `u8` symbols, one symbol per element, not packed bits. Declared
widths are 1–8 bits. Non-vetted validation output uses one `0` or `1` byte per
bit. Wider source symbols can first be processed with [`SymbolReduction`].

Assessment entropy is in bits per input symbol; bitstring branch entropy is
in bits per bit. Conditioning entropy is in bits per delivered output block.
[`ValidationAssessment::eligible_entropy`] uses the latter units when
conditioning is requested, and bits per raw sample otherwise.

[`Float`] re-exports the MPFR-backed `rug::Float` used throughout the API,
so callers can construct inputs without a separate direct dependency.
Results normally retain 256-bit precision. Explicitly use `to_f64()` only
when losing precision is acceptable.

```
use sp_800_90b::{Float, ConditioningHash, ConditioningOptions, VettedConditioner};
let entropy = Float::with_val(256, 400);
let result = VettedConditioner::Hash(ConditioningHash::Sha256)
    .assess(512, &entropy, ConditioningOptions::default())?;
println!("{} bits per output block", result.min_entropy);
# Ok::<(), sp_800_90b::ConditioningError>(())
```

Seeds are optional four-word `u64` arrays; retain actual seeds from reports
to replay stochastic checks. A fixed seed controls serial calculations,
not arbitrary NIST OpenMP scheduling. Most arithmetic is MPFR; reference
grouping/permutation decisions and restart simulation retain documented
binary64 behavior.

These APIs provide numerical evidence, not physical source validation or
certification. Vetted conditioning is a caller assertion. Raw diagnostic
fields remain public; entropy accessors are convenience checks for returned
reports, not protection against callers modifying those fields.
"#
)]

mod health_core;
pub use health_core::*;

/// MPFR-backed floating-point type used by the public API.
#[cfg(feature = "assessment")]
pub use rug::Float;

#[cfg(feature = "assessment")]
mod most_common_value;
#[cfg(feature = "assessment")]
pub use most_common_value::*;

#[cfg(feature = "assessment")]
mod collision;
#[cfg(feature = "assessment")]
pub use collision::*;

#[cfg(feature = "assessment")]
mod markov;
#[cfg(feature = "assessment")]
pub use markov::*;

#[cfg(feature = "assessment")]
mod compression;
#[cfg(feature = "assessment")]
pub use compression::*;

#[cfg(feature = "assessment")]
mod t_tuple;
#[cfg(feature = "assessment")]
pub use t_tuple::*;

#[cfg(feature = "assessment")]
mod lrs;
#[cfg(feature = "assessment")]
mod tuples;
#[cfg(feature = "assessment")]
pub use lrs::*;

#[cfg(feature = "assessment")]
mod multi_mcw;
#[cfg(feature = "assessment")]
mod prediction;
#[cfg(feature = "assessment")]
pub use multi_mcw::*;

#[cfg(feature = "assessment")]
mod lag;
#[cfg(feature = "assessment")]
pub use lag::*;

#[cfg(feature = "assessment")]
mod multi_mmc;
#[cfg(feature = "assessment")]
pub use multi_mmc::*;

#[cfg(feature = "assessment")]
mod lz78y;
#[cfg(feature = "assessment")]
pub use lz78y::*;

#[cfg(feature = "assessment")]
mod non_iid;
#[cfg(feature = "assessment")]
pub use non_iid::*;

#[cfg(feature = "assessment")]
mod chi_square;
#[cfg(feature = "assessment")]
pub use chi_square::*;

#[cfg(feature = "assessment")]
mod permutation;
#[cfg(feature = "assessment")]
pub use permutation::*;

#[cfg(feature = "assessment")]
mod iid;
#[cfg(feature = "assessment")]
pub use iid::*;

#[cfg(feature = "assessment")]
mod restart;
#[cfg(feature = "assessment")]
pub use restart::*;

#[cfg(feature = "assessment")]
mod conditioning;
#[cfg(feature = "assessment")]
pub use conditioning::*;
#[cfg(feature = "assessment")]
mod health;
#[cfg(feature = "assessment")]
pub use health::*;

#[cfg(feature = "assessment")]
mod validation;
#[cfg(feature = "assessment")]
pub use validation::*;

#[cfg(feature = "assessment")]
mod symbol_reduction;
#[cfg(feature = "assessment")]
pub use symbol_reduction::*;

#[cfg(feature = "assessment")]
mod vetted_conditioning;
#[cfg(feature = "assessment")]
pub use vetted_conditioning::*;
