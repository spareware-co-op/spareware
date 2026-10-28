# G24: the written audit of `libm` 0.2.16

*Piece G24, Spare Kit geometry kernel. The audit chapter 29.20 (rule 3) requires before a crate ships inside a kernel crate. Written 2026-10-26 by Claude Opus 5.5 for the owners. `deny-no-std.toml`'s record for `libm` names this file.*

## The answer in plain words

`libm` is the one published crate inside the kernel. `geom-num` calls 21 of its functions: the square root, fused multiply-add, rounding, and the maths functions with no proven bound, which stay apart in `geom_num::math::unproven`.

Read at the exact release `Cargo.lock` pins, the code those 21 functions reach meets the kernel's bar, with four things to know:

- **Notices.** 17 of the files reached carry Sun Microsystems' 1993 or 2004 notice, which must be kept, and one carries Alexei Sibidanov's (CORE-MATH, MIT). The generated third-party notices file (piece G481) must carry them.
- **`unsafe`.** Three kinds, all sound as read: processor instructions for the square root, fused multiply-add and rounding; one function-pointer cache for the fused multiply-add on x86-64; and unchecked table indexing in release builds.
- **Panics.** None in release builds. In debug builds the table indexing and some internal checks panic instead of misbehaving. That is the safer way round, because the kernel's tests run in debug on every target.
- **No allocation.** The only global state is the x86-64 fused multiply-add choice, and every choice gives the same bits.

The record's other claims hold: the bits are the same on every target, the crate is `no_std`, and the version is pinned. One part of the board's sentence is not done: "vendored". The merge gate refuses a `libm` patched in from a path, so vendoring needs an owner's choice (section 9).

## 1. What was audited

| | |
| --- | --- |
| Crate | `libm` 0.2.16, from crates.io |
| Checksum | `b6d2cec3eae94f9f509c767b45932f1ada8350c4bdb85af2fcab4a3c14807981`: the SHA-256 of the published `.crate` file, equal to `Cargo.lock`'s |
| Source | [rust-lang/compiler-builtins at `dfd2203a`](https://github.com/rust-lang/compiler-builtins/tree/dfd2203a4d6110820ad7bb65cafe1bf331a03a3d/libm), folder `libm` (the crate's `.cargo_vcs_info.json`) |
| Licence | MIT (`Cargo.toml`, `LICENSE.txt`); contributions are MIT or Apache-2.0, and the crate is published as MIT |
| Size | 146 Rust files, 19,676 lines; this audit reads the 58 files the kernel's 21 functions reach |
| Features in our build | the default, `arch`, only. `unstable-intrinsics`, `unstable-float` and `force-soft-floats` are off |

Everything below was read in the published crate, not in a newer upstream.

## 2. What the kernel calls

Only `geom-num` may name `libm` (the crate table's rule). Its calls:

| `geom-num` function | `libm` function |
| --- | --- |
| `math::sqrt` | `sqrt` |
| `math::fma`, and the CORE-MATH ports' internal `fma` (`math/support.rs`) | `fma` |
| `math::abs` | `fabs` |
| `math::floor`, `math::ceil` | `floor`, `ceil` |
| `math::round_ties_even`; the exact reductions in `math/exp.rs`, `math/enclose.rs` and `math/unproven.rs` | `rint` |
| `math::unproven::{sin, cos, tan, asin, acos, atan, atan2, exp, ln, log2, log10, pow, hypot, cbrt}`, and the unproven `sinpi` and `cospi` | `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`, `log`, `log2`, `log10`, `pow`, `hypot`, `cbrt` |
| `math::unproven::pow` (its domain check) | `trunc` |

Those 21 functions reach 58 files: each function's own file; the kernels `k_sin`, `k_cos`, `k_tan`; the argument reductions `rem_pio2` and `rem_pio2_large`; the generic `sqrt`, `fma`, `fma_wide`, `floor`, `ceil`, `rint`, `trunc`, `fabs`, `scalbn` and `copysign`; the architecture routines (`arch/`); and the support modules for float and integer traits, feature detection and macros. The support modules for modular arithmetic, narrowing division and big integers (`support/modular.rs`, `support/int_traits/narrowing_div.rs`, `support/big.rs`) are reached only from `fmod`, which the kernel does not call.

`tests/attack_g24.rs` holds this table to the code: it fails if `geom-num` calls a `libm` function this note does not list.

## 3. Notices

The crate's `LICENSE.txt` is the MIT licence. Files derived from FreeBSD's msun keep Sun's notice, which grants permission "provided that this notice is preserved":

- **Sun Microsystems, 1993:** `acos`, `asin`, `atan`, `atan2`, `cos`, `k_cos`, `k_sin`, `log`, `log10`, `log2`, `rem_pio2`, `rem_pio2_large`, `sin`, `tan`.
- **Sun Microsystems, 2004:** `exp`, `pow`, `k_tan`.
- **Alexei Sibidanov, 2021–2022:** `cbrt` is a port of CORE-MATH's `cbrt.c` under MIT, "Ported to Rust in 2025 by Trevor Gross".

The other files reached carry no notice beyond the crate's MIT licence.

What follows: `projects/engines/geometry/NOTICE` keeps notices for ported code only, and says the notices of crates compiled into a shipped binary go in the generated third-party notices file (29.20, rule 4; piece G481). That file must carry `libm`'s MIT licence and both Sun notices in full, and Sibidanov's line for `cbrt`. Nothing ships yet, so nothing is missing today. G481 owns it.

## 4. `unsafe`

The crate forbids `unsafe_op_in_unsafe_fn`, so every unsafe operation sits in a marked block. In the code the kernel reaches there are three kinds:

1. **Processor instructions** (`arch/`), compiled only with the `arch` feature, on:
   - **x86-64:** the square root by `sqrtsd` in inline assembly (`arch/x86.rs`).
   - **aarch64 with NEON:** `fsqrt`, `fmadd` and `frintn` in inline assembly (`arch/aarch64.rs`).

   Each block computes on registers only, with no memory access. Each instruction is the IEEE 754 operation, correctly rounded; `frintn` rounds to nearest, ties to even, the same as `rint`. wasm32 uses the generic Rust code, because its intrinsics need the nightly `unstable-intrinsics` feature, which is off.

   The architecture row in 29.4 names the square root and fused multiply-add on aarch64. `rint` is a processor instruction there too, and gives the same bits, because rounding to an integer is exact.
2. **The fused multiply-add on x86-64, chosen once at run time** (`arch/x86/fma.rs`, `arch/x86/detect.rs`, `support/feature_detect.rs`). The first call reads the processor's features with `cpuid` and `xgetbv`. It then stores, in a `static AtomicPtr`, one of three functions: inline-assembly `vfmadd213sd` (FMA3), `vfmaddsd` (FMA4), or the generic software fused multiply-add. Later calls jump through that pointer. The features are cached in a `static AtomicU32`.

   All three functions compute the same correctly rounded result, so the choice changes speed, not bits. The pointer only ever holds one of those functions or the initialiser. The code is sound as read.

   This is global state inside a vetted crate. Principle 7 bans such state from the kernel's own crates. Here it is a cache of a fact about the machine that cannot change a result, so the audit accepts it.
3. **Table indexing without bounds checks in release builds** (the `i!` macro, `math/mod.rs`): 63 uses in the files reached, all reading or writing fixed tables by computed index. `rem_pio2_large`, the reduction for |x| ≥ 2^20·π/2 in `sin`, `cos` and `tan`, has 62 of them. In debug builds the same macro uses `get(...).unwrap()`, so an index out of range panics, which would show a fault in our debug test runs instead of misbehaving silently.

   The indices come from FreeBSD's algorithms, which bound them by construction. This audit did not prove each bound; section 8 says how the kernel's tests stand behind it.

`force_eval!` (`math/mod.rs`) is a volatile read of a local value, used to raise floating-point exceptions on purpose. It is sound and changes no result. `support/float_traits.rs` turns floats into bits by `transmute`, the same as `to_bits`, and is sound. Its `core::intrinsics` fused multiply-add is compiled only with `unstable-intrinsics`, which is off.

## 5. Panics

- **Release builds:** no `panic!`, `unwrap`, `expect`, `assert!` or unreachable code in the files reached. The `assert!`s and `unwrap` in `support/modular.rs` are in code only `fmod` reaches.
- **Debug builds:** the code can panic in two ways, each only if `libm` had a fault:
  - the checked `i!` indexing above;
  - `debug_assert!`s: in `generic/scalbn.rs`, on constant expressions; in `support/int_traits.rs`, that a cast between integer widths is exact; in `arch/x86/fma.rs`, that the chosen instruction set is present; and in `support/feature_detect.rs`, on bit indices.

  Integer arithmetic in the dependency is overflow-checked in debug builds as well, the dev profile's default, so an overflow would panic there too.

The kernel's tests run in debug and in release on every target of the determinism matrix, so a fault on a path the tests reach would show as a panic in a debug leg.

## 6. Allocation and other effects

- **Allocation:** none. The crate is `#![no_std]` and names no `alloc`. Its `Vec` and `String` uses are in test modules only.
- **Global state:** only the x86-64 fused multiply-add cache of section 4.
- **No I/O, clock, randomness or thread identity.**
- **Floating-point environment:** the code raises exceptions on purpose (`force_eval!`), but Rust gives no access to the flags and the kernel never reads them, so this changes no result.

## 7. Determinism

Every function the kernel calls gives the same bits on every target in the matrix, for three reasons:

- The generic code is the same Rust everywhere, and Rust never contracts `a * b + c` into a fused multiply-add on its own (RFC 3514).
- The processor instructions it uses instead, the square root, fused multiply-add and round to integer, are correctly rounded by IEEE 754, so they agree with the generic code bit for bit.
- The x86-64 run-time choice picks among implementations that all give the correctly rounded result.

What this does not cover is the accuracy of the unproven functions. Their results are deterministic, but have no proven error bound, which is why they stay in `math::unproven` (6.4, 21).

The build-time configuration does not change results:

- `build.rs` and `configure.rs` read only Cargo's environment and emit `cfg` flags. `optimizations_enabled` is used by the crate's tests only. `x86_no_sse` applies only to 32-bit x86 without SSE, which the kernel's platform list excludes.
- `f16_enabled` and `f128_enabled` affect only the 16- and 128-bit functions, which the kernel does not call.
- An `ENSURE_NO_PANIC` variable in the build environment would turn on the `no_panic` attribute, whose crate is not a dependency, so such a build would fail rather than change.

## 8. How the kernel's own checks stand behind this

- **Every function is replayed in the determinism matrix:**
  - `geom-num`'s `tests/determinism.rs` hashes the results of every public maths function, the 11 of `geom_num::math` and the 17 of `geom_num::math::unproven`;
  - every golden of the store runs on six native legs and on wasm32, in debug and release (chapter 21, "What the kernel's CI runs today"; G19);
  - so a target whose bits differ fails CI.
- **Every function is benchmarked:** `geom-bench`'s `math` and `unproven` groups measure every public maths function, `libm`'s twins of the CORE-MATH functions included. Their counts are recorded in `golden.txt`, the baseline for the later CORE-MATH comparison.
- **The audit is held to the code:** `tests/attack_g24.rs` fails if:
  - `geom-num` calls a `libm` function not in section 2's table;
  - the record in `deny-no-std.toml` stops naming this file;
  - `Cargo.lock` moves `libm` off the checksum audited here.

  It also runs each audited function on seeded inputs over every binade, and on the arguments that reach `rem_pio2_large`, the code with the most unchecked indexing. In a debug build its indexing is checked, so an out-of-range index would fail the test.

## 9. Open: what "vendored" should mean

The board's done-when for G24 says "the maths module wrapping vendored libm". Chapter 29.4's row says "not yet vendored (28.13)", and 29 defines "vendored" as "a copy lives in our repository". Chapter 28's toolchain guard runs every golden when "vendored numeric code" changes.

The merge gate (G12a) refuses a vendored copy in the usual form: it allows `libm` only from crates.io, at the pinned version. `deny-no-std.toml` says so in as many words: "not code patched in". So vendoring needs an owner's choice among:

- **A. Vendor the whole crates.io graph** (`cargo vendor` and source replacement). `Cargo.lock` still names crates.io and its checksums are verified against the copy, so the gate's rule holds. But every dependency of the workspace, development crates included, moves into the repository.
- **B. Port the 58 files into `geom-num`** as the kernel's own code under its MIT and Sun notices. `libm` then becomes a PORT instead of the one DEPEND. That changes 29.1's decision and the vetted allow-list, and every later `libm` fix becomes ours to carry.
- **C. Keep the DEPEND as it is, and define "vendored" as this audit pinned to the release's checksum.** The bytes cannot change without the checksum changing, and `tests/attack_g24.rs` fails on that. The only thing not covered is crates.io becoming unavailable.

Recommendation: **C**. The checksum already fixes the exact bytes audited. A and B buy only independence from crates.io, at the cost of many files moved or carried, and B reverses an owner's decision. The question is added to chapter 31.7.

## Method

The audit read the published crate (`~/.cargo/registry/src/.../libm-0.2.16`, unpacked by Cargo from the `.crate` file whose SHA-256 is above) and traced, from each of the 21 functions, the files it reaches. For each file it recorded:

- its notice lines;
- every `unsafe` block, and what it touches;
- every way to panic outside test modules: `panic!`, `unwrap`, `expect`, `assert!` and its forms, unreachable code, and the `i!` macro in both builds;
- any allocation;
- any global state.

The architecture routines were read for each target the kernel builds: x86-64, aarch64 and wasm32. The rules audited against are chapter 29.20's rule 3 and the kernel's principles 5, 7 and 13.
