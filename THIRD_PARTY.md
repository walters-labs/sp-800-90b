# Third-party dependency inventory

Snapshot: 2026-10-20, version 0.1.0, resolved `Cargo.lock`.

Original project contributions use [LICENSE-MIT](LICENSE-MIT). Reference-informed
adaptations retain the exact [NIST notice](LICENSE-NIST). The combined terms
and modification notice are in [LICENSE](LICENSE); the source map is in
[docs/provenance.md](docs/provenance.md). Dependencies retain their own licenses.
The table records Cargo package metadata for the
resolved graph, including target-specific and build/procedural-macro packages;
not every package is linked into every build. This is an inventory, not a
replacement for the upstream license texts or a binary redistribution bundle.
Regenerate/review it when updating the lockfile.

The default `assessment` and `cli` features activate the dependency graph below.
With default features disabled and neither feature enabled, the health-monitor
library uses only `core` and has no external dependencies; GMP/MPFR are not built
or linked. Enabling `assessment` restores the arithmetic dependencies, and `cli`
also adds JSON and hashing dependencies.

| Package | Version | Declared license |
| --- | --- | --- |
| [az](https://docs.rs/crate/az/1.3.0/source/) | 1.3.0 | `MIT/Apache-2.0` |
| [block-buffer](https://docs.rs/crate/block-buffer/0.10.4/source/) | 0.10.4 | `MIT OR Apache-2.0` |
| [bzip2](https://docs.rs/crate/bzip2/0.6.1/source/) | 0.6.1 | `MIT OR Apache-2.0` |
| [cfg-if](https://docs.rs/crate/cfg-if/1.0.5/source/) | 1.0.5 | `MIT OR Apache-2.0` |
| [cpufeatures](https://docs.rs/crate/cpufeatures/0.2.17/source/) | 0.2.17 | `MIT OR Apache-2.0` |
| [crypto-common](https://docs.rs/crate/crypto-common/0.1.7/source/) | 0.1.7 | `MIT OR Apache-2.0` |
| [digest](https://docs.rs/crate/digest/0.10.7/source/) | 0.10.7 | `MIT OR Apache-2.0` |
| [generic-array](https://docs.rs/crate/generic-array/0.14.7/source/) | 0.14.7 | `MIT` |
| [getrandom](https://docs.rs/crate/getrandom/0.4.3/source/) | 0.4.3 | `MIT OR Apache-2.0` |
| [gmp-mpfr-sys](https://docs.rs/crate/gmp-mpfr-sys/1.7.1/source/) | 1.7.1 | `LGPL-3.0+` |
| [itoa](https://docs.rs/crate/itoa/1.0.18/source/) | 1.0.18 | `MIT OR Apache-2.0` |
| [libbz2-rs-sys](https://docs.rs/crate/libbz2-rs-sys/0.2.5/source/) | 0.2.5 | `bzip2-1.0.6` |
| [libc](https://docs.rs/crate/libc/0.2.191/source/) | 0.2.191 | `MIT OR Apache-2.0` |
| [libm](https://docs.rs/crate/libm/0.2.16/source/) | 0.2.16 | `MIT` |
| [memchr](https://docs.rs/crate/memchr/2.8.3/source/) | 2.8.3 | `Unlicense OR MIT` |
| [proc-macro2](https://docs.rs/crate/proc-macro2/1.0.107/source/) | 1.0.107 | `MIT OR Apache-2.0` |
| [quote](https://docs.rs/crate/quote/1.0.47/source/) | 1.0.47 | `MIT OR Apache-2.0` |
| [r-efi](https://docs.rs/crate/r-efi/6.0.0/source/) | 6.0.0 | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` |
| [rug](https://docs.rs/crate/rug/1.30.0/source/) | 1.30.0 | `LGPL-3.0+` |
| [serde](https://docs.rs/crate/serde/1.0.229/source/) | 1.0.229 | `MIT OR Apache-2.0` |
| [serde_core](https://docs.rs/crate/serde_core/1.0.229/source/) | 1.0.229 | `MIT OR Apache-2.0` |
| [serde_derive](https://docs.rs/crate/serde_derive/1.0.229/source/) | 1.0.229 | `MIT OR Apache-2.0` |
| [serde_json](https://docs.rs/crate/serde_json/1.0.154/source/) | 1.0.154 | `MIT OR Apache-2.0` |
| [sha2](https://docs.rs/crate/sha2/0.10.9/source/) | 0.10.9 | `MIT OR Apache-2.0` |
| [syn](https://docs.rs/crate/syn/3.0.9/source/) | 3.0.9 | `MIT OR Apache-2.0` |
| [typenum](https://docs.rs/crate/typenum/1.20.1/source/) | 1.20.1 | `MIT OR Apache-2.0` |
| [unicode-ident](https://docs.rs/crate/unicode-ident/1.0.27/source/) | 1.0.27 | `(MIT OR Apache-2.0) AND Unicode-3.0` |
| [version_check](https://docs.rs/crate/version_check/0.9.5/source/) | 0.9.5 | `MIT/Apache-2.0` |
| [windows-link](https://docs.rs/crate/windows-link/0.2.1/source/) | 0.2.1 | `MIT OR Apache-2.0` |
| [windows-sys](https://docs.rs/crate/windows-sys/0.61.2/source/) | 0.61.2 | `MIT OR Apache-2.0` |
| [zmij](https://docs.rs/crate/zmij/1.0.23/source/) | 1.0.23 | `MIT` |

## Native libraries

The selected `rug` float feature uses `gmp-mpfr-sys`, whose 1.7.1 source package
contains GMP 6.3.0, MPFR 4.2.2, and MPC 1.4.1. This project enables floating-point
support, not MPC complex-number support. The binding package includes
`LICENSE-LGPL.md` and `LICENSE-GPL.md`; its bundled native source directories
contain their own `COPYING*` files. Review those source notices when distributing
linked binaries. The crate's license file does not relicense these dependencies.

`bzip2` 0.6.1 uses the Rust `libbz2-rs-sys` backend in this lockfile; the backend
has its own `bzip2-1.0.6` license declaration.

## Reference software attribution

We acknowledge the National Institute of Standards and Technology (NIST) as the
source of SP800-90B_EntropyAssessment, revision
`87c104d0ed4cbc96103e7b8b38d6f2c7e0a6b289`. This project adapts reference
algorithms and implementation conventions into Rust; it is not a clean-room
claim. Modifications dated 2026-10-19 through 2026-10-20 include MPFR arithmetic,
Rust data structures, structured diagnostics and stricter failure handling.
The complete upstream notice is preserved in LICENSE-NIST and LICENSE.

The xoshiro256** generator is credited to David Blackman and Sebastiano Vigna,
identified as public domain in the pinned reference. The external reference
chi-square header also carries a separate Cephes notice; the Rust implementation
uses different MPFR tail routines. See the provenance record for the distinction
between Rust adaptations and externally compiled reference components.

## Reference tooling

The published crate includes our small C++ comparison adapters and harness, but
not the NIST source checkout, datasets, generated results, or compiled tools.
The harness exports or fetches the pinned external source separately. Reference
build dependencies (including OpenSSL, jsoncpp, divsufsort and OpenMP) are not
runtime dependencies of this Rust crate. Their notices belong to the separately
built reference tools and are outside the Cargo dependency table above.
