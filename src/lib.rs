//! Entropy estimators from NIST SP 800-90B.
//!
//! Implements sections 6.3.1–6.3.10 and the combined non-IID assessment.
//! Combined IID assessment includes chi-square, LRS, and permutation tests.
//! Restart testing, conditioning bounds, and continuous RCT/APT health tests
//! are available. This is not complete entropy-source validation.

mod most_common_value;
pub use most_common_value::*;

mod collision;
pub use collision::*;

mod markov;
pub use markov::*;

mod compression;
pub use compression::*;

mod t_tuple;
pub use t_tuple::*;

mod lrs;
mod tuples;
pub use lrs::*;

mod multi_mcw;
mod prediction;
pub use multi_mcw::*;

mod lag;
pub use lag::*;

mod multi_mmc;
pub use multi_mmc::*;

mod lz78y;
pub use lz78y::*;

mod non_iid;
pub use non_iid::*;

mod chi_square;
pub use chi_square::*;

mod permutation;
pub use permutation::*;

mod iid;
pub use iid::*;

mod restart;
pub use restart::*;

mod conditioning;
pub use conditioning::*;
mod health;
pub use health::*;

mod validation;
pub use validation::*;
