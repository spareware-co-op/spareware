# G86: the written audit of `sha2` 0.11.0 and the crates it brings

*Piece G86, Spare Kit geometry kernel. The audit chapter 29's rule 3 requires before a published crate ships inside a kernel crate (owner action G79, decision 32.20). Written 2026-10-31 by Claude for the owners: Sonnet 5.5 read the crates, and Opus 5.5 checked its claims against the source and the kernel's build. `deny-no-std.toml`'s records for these nine crates name this file.*

## The answer in plain words

`geom-op` hashes with `sha2`'s SHA-256 and calls four things: `Sha256::new()`, `update`, `finalize()` converted into a `[u8; 32]`, and `Clone`, `Debug` and `Default` of the hasher. Those calls reach `sha2` and eight other published crates: `digest`, `block-buffer`, `crypto-common`, `hybrid-array`, `typenum`, `cfg-if`, and, only on x86-64 and AArch64, `cpufeatures`, which brings `libc` on AArch64 Linux, Android and Apple only.

Read at the exact releases `Cargo.lock` pins, the code those calls reach meets the kernel's bar, with four things to know:

- **`unsafe` is wider than decision 32.20 says.** 32.20 puts it in `sha2`'s processor backends and one table read. Those are real (section 4.1). But the same calls also run `unsafe` code in `block-buffer` (the byte buffer: raw copies into a buffer that starts uninitialised, with the position kept in its last byte), in `hybrid-array` (slice and array casts, and the array constructor) and in `cpufeatures` (the `cpuid` instruction, and on AArch64 Linux one `getauxval` call). `digest`, `crypto-common` and `typenum` forbid `unsafe` outright, and `cfg-if` has none. Every site I read is sound as read.
- **"Behind a runtime check" is true on some targets only.** x86-64 and AArch64 Linux check at run time. AArch64 macOS takes the hardware path at compile time, with no check. On AArch64 Windows the check always says no, so the software path runs. A build with `-C target-cpu=native`, which the kernel's CI runs, can turn the check into a constant. Section 7 has the table. The digest is the same on every path.
- **Notices.** All nine crates are MIT OR Apache-2.0 and carry copyright lines that the generated third-party notices file (piece G481) must keep (section 3). One question is open: `sha2`'s two AArch64 backends say they are "adapted from mbedtls" and carry no separate notice.
- **No allocation, no reachable panics, one cache.** Nothing allocates. With overflow checks on, the code can panic by arithmetic overflow after about 2 exbibytes of input; every other panic site is on a branch that cannot be taken for SHA-256. The only mutable global state is one `static AtomicU8` per processor-feature check in `cpufeatures`, a cache of a machine fact that cannot change a digest.

The record's other claims hold: the bits are the same on every target, every crate is `#![no_std]`, none has a procedural macro, only `libc` has a build script, and the versions are pinned by `Cargo.lock`, by checksum, and by the allow-list entries. Section 12 lists each claim in 32.20 and in the `deny-no-std.toml` records against what the source says, and section 13 lists what I could not verify, the most important being that I did not re-derive the SHA-NI and AArch64 instruction sequences (the NIST vectors in `geom-op`'s tests stand behind them).

## 1. What was audited

Nine published crates, each read in the unpacked `.crate` file, not in a newer upstream.

| Crate | Checksum: the SHA-256 of the published `.crate` file, equal to `Cargo.lock`'s | Source (the commit in the crate's `.cargo_vcs_info.json`) |
| --- | --- | --- |
| `sha2` 0.11.0 | `446ba717509524cb3f22f17ecc096f10f4822d76ab5c0b9822c5f9c284e825f4` | [RustCrypto/hashes at `ffe09398`](https://github.com/RustCrypto/hashes/tree/ffe093984c004769747e998f77da8ff7c0e7a765/sha2), folder `sha2` |
| `digest` 0.11.3 | `f1dd6dbb5841937940781866fa1281a1ff7bd3bf827091440879f9994983d5c2` | [RustCrypto/traits at `2fb9ed89`](https://github.com/RustCrypto/traits/tree/2fb9ed8922e244117040bb037a7d141a6a2b8228/digest), folder `digest` |
| `block-buffer` 0.12.1 | `d2f6c7dbe95a6ed67ad9f18e57daf93a2f034c524b99fd2b76d18fdfeb6660aa` | [RustCrypto/utils at `cbd0963c`](https://github.com/RustCrypto/utils/tree/cbd0963c685df025c42bed31f78c50d1bada3805/block-buffer), folder `block-buffer` |
| `crypto-common` 0.2.2 | `ce6e4c961d6cd6c9a86db418387425e8bdeaf05b3c8bc1411e6dca4c252f1453` | [RustCrypto/traits at `93dee26c`](https://github.com/RustCrypto/traits/tree/93dee26c6bde3741a197f1c5f6b7baac277705f3/crypto-common), folder `crypto-common` |
| `hybrid-array` 0.4.15 | `27f864f10dfb56725ce5ce5472bc52252c8f93a4ab86327122cebf62c5f59a17` | [RustCrypto/hybrid-array at `09310c55`](https://github.com/RustCrypto/hybrid-array/tree/09310c55b75e2cd408951c7e2e86dba16c63fc68), repository root |
| `typenum` 1.20.1 | `b6f5e870be6c3b371b77fe0ee0bafb859fa4964b4404c27de1d380043c4dda20` | [paholg/typenum at `0db9a0f7`](https://github.com/paholg/typenum/tree/0db9a0f731981f29266b63586c29fa07e4477b1a), repository root |
| `cfg-if` 1.0.5 | `4e7648175b45a9a48536d676f68d918270699102aa8dab5496df06904c914600` | [rust-lang/cfg-if at `8b0a2b0f`](https://github.com/rust-lang/cfg-if/tree/8b0a2b0fc1fa3b3741b9e72895f390cf499fe36f), repository root |
| `cpufeatures` 0.3.1 | `5ca28b0ae3115b884660db4118d803791fd6756b6e88f39c0f3f7859060d7566` | [RustCrypto/utils at `e3ac92bf`](https://github.com/RustCrypto/utils/tree/e3ac92bfd33051e146025baa2d0ce79891b7fc73/cpufeatures), folder `cpufeatures` |
| `libc` 0.2.191 | `65cd87c9a6d60536580b316ad985726104730a5151c99845f2490428ef91240a` | [rust-lang/libc at `ccc2ff4a`](https://github.com/rust-lang/libc/tree/ccc2ff4aaab962f7fde7b8edda7379c045b1ca12), repository root |

I computed each checksum from the `.crate` file and compared it with `Cargo.lock`: nine of nine equal. `Cargo.lock` holds one version of each crate, with exactly these edges: `sha2` to `cfg-if`, `cpufeatures`, `digest`; `digest` to `block-buffer`, `crypto-common`; `block-buffer` and `crypto-common` to `hybrid-array`; `hybrid-array` to `typenum`; `cpufeatures` to `libc`. The upstream commits were checked through GitHub's API on 2026-10-31: all nine exist, and each is the commit of the crate's release tag, except `cpufeatures`, whose tag is the commit before it; the one change between them is `.github/dependabot.yml`. The line numbers in this note are the published crates'. The links assume the published files equal the repository's at that commit, which I did not diff.

| Crate | Licence | Size | Features on in our build | Build script, macro | Compiled for |
| --- | --- | --- | --- | --- | --- |
| `sha2` | MIT OR Apache-2.0 (`Cargo.toml`, `LICENSE-MIT`, `LICENSE-APACHE`) | 27 source files, 3,151 lines; the `.crate` is 33,969 bytes | none. `default-features = false` turns off `alloc` and `oid`; `zeroize` is off | none (`build = false`) | every target |
| `digest` | MIT OR Apache-2.0 | 16 files, 2,857 lines | `block-api` only (its default, which `sha2` asks for). Off: `alloc`, `dev`, `getrandom`, `mac`, `oid`, `rand_core`, `zeroize` | none | every target |
| `block-buffer` | MIT OR Apache-2.0 | 3 files, 756 lines | none (`zeroize` off) | none | every target |
| `crypto-common` | MIT OR Apache-2.0 | 3 files, 853 lines | none (`getrandom`, `rand_core`, `zeroize` off) | none | every target |
| `hybrid-array` | MIT OR Apache-2.0 | 7 files, 3,229 lines | none (`alloc`, `extra-sizes`, `serde`, `bytemuck`, `zeroize` and the other optional dependencies off) | none | every target |
| `typenum` | MIT OR Apache-2.0 | 14 files, 19,354 lines of type-level definitions and constants | `const-generics`, which `hybrid-array` asks for. Off: `i128`, `scale_info`, `strict` | none (`build = false`, and no `build.rs` in the package) | every target |
| `cfg-if` | MIT OR Apache-2.0 | 1 file, 218 lines | none | none | every target |
| `cpufeatures` | MIT OR Apache-2.0 | 5 files, 589 lines | none | none | x86, x86-64 and AArch64 only: `sha2`'s dependency on it is target-specific (`sha2`'s `Cargo.toml`) |
| `libc` | MIT OR Apache-2.0 | 442 files, 133,106 lines; the calls reach one function declaration and a few constants | default features off (`std`, `extra_traits` and `align` all off), as `cpufeatures` asks | `build.rs`, 630 lines (section 4.4) | AArch64 Linux, Android and Apple only (and LoongArch Linux): `cpufeatures`' dependency on it is target-specific. Not x86-64, not AArch64 Windows, not wasm32 |

Features, checked against the lock: `digest`, `block-buffer`, `crypto-common`, `hybrid-array` and `typenum` have no dependent in `Cargo.lock` outside this chain, and the three crates that name `sha2` (`geom-op`, `geom-test`, `geom-bench`) all turn its default features off. So the features above are exactly the ones in the build. `cfg-if`, `cpufeatures` and `libc` also have dependents elsewhere in the lock (`chacha20` for `cpufeatures`; `divan`, `getrandom`, `rustix` and `errno` for `libc`). In a workspace-wide build Cargo unifies features across them, but `cpufeatures` has no features, and `libc`'s optional features add declarations, derives or `#[link]` directives for system libraries (`src/unix/mod.rs:487-640`), none of which changes what `sha2` computes. I did not run `cargo tree` or build anything; the feature sets come from reading each `Cargo.toml`.

## 2. What the kernel calls

`geom-op`'s `hash.rs` wraps `sha2::Sha256` in a newtype and calls these. Paths are in the published crates.

| `geom-op` code | What it reaches |
| --- | --- |
| `sha2::Sha256::new()` | `Digest::new` (`digest` `src/digest.rs:60-63`) calls `Default::default` (`src/buffer_macros/fixed.rs:238-246`), which builds `CtOutWrapper::default` (`src/block_api/ct_variable.rs:125-130`, calling `Sha256VarCore::new(32)`, `sha2` `src/block_api.rs:53-61`) and `BlockBuffer::default` (`block-buffer` `src/lib.rs:99-107`) |
| `self.0.update(bytes)` (`Digest::update`) | `digest.rs:75-78`, then `fixed.rs:200-208`, then `BlockBuffer::digest_blocks` (`lib.rs:160-213`), then `Sha256VarCore::update_blocks` (`block_api.rs:36-43`), `Array::cast_slice_to_core` (`hybrid-array` `src/lib.rs:521-524`), `compress256` (`sha256.rs:86-88`), and the backend |
| `finalize().into()` into `[u8; 32]` | `digest.rs:86-89`, `digest` `src/lib.rs:101-106`, `fixed.rs:220-226`, `ct_variable.rs:103-116`, `Sha256VarCore::finalize_variable_core` (`block_api.rs:63-72`), `BlockBuffer::len64_padding_be` and `digest_pad` (`lib.rs:366-407`), the backend; then `From<Array<u8, U32>> for [u8; 32]` (`hybrid-array` `src/lib.rs:757-765`), which moves the inner array |
| `Clone` | `fixed.rs:278-286`: the core's derived `Clone` (`block_api.rs:20`) and `BlockBuffer::clone` (`lib.rs:109-116`) |
| `Debug` | `fixed.rs:117-122`: prints `Sha256 { ... }` and never the state |
| `Default` | `fixed.rs:238-246`, as for `new()` |

Not called, so not read for what they do: SHA-224, SHA-384 and the SHA-512 types, serialisation, `reset`, `finalize_reset`, extendable output, MAC and OID support, and `compress256` as a free function.

Of `sha2`'s 27 source files, 7 are reached on x86-64 or AArch64 (`lib.rs`, `block_api.rs`, `consts.rs`, `sha256.rs`, `sha256/soft.rs`, `sha256/soft/unroll.rs` and the one processor backend) and 6 on wasm32. SHA-512's files compile on x86-64 and AArch64 and are not reached.

## 3. Notices

Each crate ships `LICENSE-MIT` and `LICENSE-APACHE`; none ships a `NOTICE` file, so the Apache-2.0 clause that carries a `NOTICE` has nothing to carry. The copyright lines in each `LICENSE-MIT`, which the MIT licence requires to be kept with the text:

| Crate | Copyright lines |
| --- | --- |
| `sha2` | 2016-2026 The RustCrypto Project Developers; 2016 Artyom Pavlov; 2009-2013 Mozilla Foundation; 2006-2009 Graydon Hoare |
| `digest` | 2017-2025 RustCrypto Developers; 2017 Artyom Pavlov |
| `block-buffer` | 2018-2025 The RustCrypto Project Developers |
| `crypto-common` | 2021-2026 RustCrypto Developers |
| `hybrid-array` | 2022-2026 The RustCrypto Project Developers |
| `typenum` | 2014 Paho Lurie-Gregg (its `LICENSE` file is the single line "MIT OR Apache-2.0") |
| `cfg-if` | 2014 Alex Crichton |
| `cpufeatures` | 2020-2026 The RustCrypto Project Developers |
| `libc` | The Rust Project Developers (no year) |

The source files carry no copyright headers. The crate does not say why the Mozilla Foundation and Graydon Hoare lines are in `sha2`'s licence file; they are to be kept as they are.

One question is not settled by anything I read. `sha2`'s AArch64 SHA-256 backend begins "Implementation adapted from mbedtls" (`sha256/aarch64_sha2.rs:3`), and its AArch64 SHA-512 backend says the same (`sha512/aarch64_sha3.rs:1`, not called by the kernel). Neither file has a copyright header, and `sha2`'s licence statement (`Cargo.toml`, `README.md` lines 106 to 113) names only MIT OR Apache-2.0. mbedtls's own licence is not in the published crate, and I did not research it. The AArch64 SHA-256 file is compiled on every AArch64 target. Whether the notices file needs a line for it is an owner's question (section 13).

What follows: `projects/engines/geometry/NOTICE` keeps notices for ported code only, and says the notices of crates compiled into a shipped binary go in the generated third-party notices file (29.20, rule 4; piece G481), as G24 records for `libm`. That file must carry the MIT text and the copyright lines above for each crate that is compiled into the binary for the target. Nothing ships yet, so nothing is missing today. G481 owns it.

## 4. `unsafe`

A raw count for scale, of lines containing the word outside comments: `sha2` 59 (most are in backends the kernel does not reach), `block-buffer` 22, `hybrid-array` 37, `cpufeatures` 11. `digest`, `crypto-common` and `typenum` have one each, which is their `#![forbid(unsafe_code)]` line (`digest` `src/lib.rs:4`, `crypto-common` `src/lib.rs:8`, `typenum` `src/lib.rs:43`). `cfg-if` has none. What follows is only what the four calls reach, traced by reading.

### 4.1 `sha2`

`sha2` allows `unsafe_op_in_unsafe_fn` in its two processor backends (`sha256/x86_sha.rs:3`, `sha256/aarch64_sha2.rs:4`), so each backend function is one unsafe context with no inner `unsafe` blocks; the table lists the unsafe operations in them.

| Site | Where, what it does | Sound as read | Compiled for |
| --- | --- | --- | --- |
| [S1] | `sha256/x86_sha.rs:13-19, 44-102`. SHA-NI compression. The unsafe operations are the SHA and SSE intrinsics, 2 unaligned 16-byte loads from `state` (:54-55), 4 per block from the 64-byte block (:67-70), and 2 unaligned stores to `state` (:100-101) | Yes. `state` is a `[u32; 8]` (32 bytes) and `block` a `[u8; 64]`, exactly the bytes touched; the unaligned load and store forms are used; the round constants are `K32X4[$i]` with constant `$i` below 16, bounds-checked | x86-64, every OS. Runs only where the processor, or the build, has `sha`, `sse2`, `ssse3` and `sse4.1` |
| [S2] | `sha256/aarch64_sha2.rs:12-94`. AArch64 SHA2 compression, same shape: `vld1q_u32(state[0..4].as_ptr())` and the like | Yes. Every pointer comes from a bounds-checked slice of 16 bytes (`state[0..4]`, `state[4..8]`, `block[0..16]` and so on) or from `&K32[t]` with `t` at most 60, which reads `K32[60..64]` | AArch64, every OS. Runs only where `sha2` is detected or on at build time |
| [S3] | `sha256/soft/unroll.rs:39-42`. `rk(i)` reads the round constant `K32[i]` through a raw pointer, with `ptr::read` on x86 and `read_volatile` elsewhere (:31-37) | Yes. `i` is a literal from 0 to 63 written out by the `repeat64!` macro (:4-23, used at :50) and `K32` has 64 entries (`consts.rs:37`). There is no SAFETY comment | Every target. It is the only `sha2` `unsafe` on wasm32 |
| [S4] | `sha256.rs:67` and `:72`. The calls to S1 and S2 | Yes. The check before each covers every feature the callee enables: `sha256.rs:55` lists `sha`, `sse2`, `ssse3`, `sse4.1`, the four in S1's `#[target_feature]` (`x86_sha.rs:13, 45`); `:58` lists `sha2`, as S2's does (`aarch64_sha2.rs:12`) | x86-64 and AArch64 |

Notes on this table:

- **Selected by build flags only.** Three more call sites (`sha256.rs:16, 31, 41`) belong to backends chosen by a `--cfg` flag. They are not compiled in our build.
- **Compiled, not reached.** On x86-64 and AArch64, the SHA-512 backends (`sha512/x86_avx2.rs`, `sha512/aarch64_sha3.rs`, `sha512/soft/unroll.rs:33`) are compiled because the SHA-512 types exist, and the kernel never calls them. The LoongArch, RISC-V and `wasm32-simd128` backends are cfg-gated off on every kernel target.
- **The compact software path.** `sha256/soft/compact.rs` has no `unsafe` and indexes `K32[i]` with a bounds check (:26), as 32.20 says. It is compiled only with `--cfg sha2_backend_soft="compact"`, which the kernel does not set, so the unrolled path with S3 is the one that runs.
- **Software helper.** `sha256/soft.rs:13` has `try_into().unwrap()` on a 4-byte slice of a 64-byte block at an index below 16; it cannot fail.

### 4.2 `block-buffer`

This crate has no target-specific code and no feature on in our build, so the same code runs on every target. For SHA-256 the buffer is `BlockBuffer<U64, Eager>`: 64 bytes in a `MaybeUninit<Array<u8, U64>>`, with the position (0 to 63) stored in the last byte (`sealed.rs:34-58`). Position and data share the buffer, which is why there is `unsafe`.

| Site | Where, what it does | Sound as read |
| --- | --- | --- |
| [B1] | `lib.rs:109-116`. `Clone` by `ptr::read(self)` | Yes: a bitwise copy of a `MaybeUninit` buffer and a zero-sized position. The SAFETY comment (:112-113) says the type has no `Drop` impl, but it has one (`lib.rs:433-439`). That impl does nothing unless the `zeroize` feature is on, and it is off, so the copy is still sound. The comment is stale |
| [B2] | `sealed.rs:39-58`. `Eager::get_pos` reads, and `set_pos` writes, byte 63 of the buffer | Yes. Byte 63 is always initialised: `BlockBuffer::default` (`lib.rs:99-107`) writes 0 into it before anything reads it |
| [B3] | `lib.rs:173-183`. `digest_blocks`, short input: copies `n` bytes to `pos..pos+n`, then sets the position | Yes. This branch is taken only when `n < 64 - pos`, so the last byte written is at most index 62 and the position byte is not overwritten |
| [B4] | `lib.rs:184-201`. `digest_blocks`, the buffer fills: copies the next `64 - pos` input bytes to `pos..64` (on purpose over the position byte), `assume_init_ref`, then compresses that block | Yes. Bytes 0 to `pos-1` hold data by the invariant and the copy fills the rest, so all 64 bytes are initialised. `ResetGuard` (`lib.rs:444-451`) puts the position back to 0 when the block ends, also if `compress` panics |
| [B5] | `lib.rs:208-212` and `:304-310`. `set_data_unchecked(leftover)`: sets the position, then copies the `leftover` bytes to the start | Yes. `leftover` is the tail of `Array::slice_as_chunks`, so under 64 bytes; at most 63 bytes are written, and not byte 63 |
| [B6] | `lib.rs:236-246`. `get_pos` calls `unreachable_unchecked` if the position were 64 or more | Sound while the invariant holds, and no safe call breaks it as read: `set` asserts (:262), `try_new` and `deserialize` check (:146, :342), and the other writers write values below 64. In a debug build a `debug_assert!(false)` fires first |
| [B7] | `lib.rs:366-400`. `digest_pad`, used by `finalize`: writes the `0x80` byte at `pos`, zeros the rest (`63 - pos` bytes), `assume_init_mut`, compresses once or twice, writes the 8 length bytes at 56 | Yes. The writes cover exactly `pos..64`, so the block is fully initialised; `pad_len < 8` (that is `pos > 55`) takes the two-block branch. `ResetGuard` as above |
| [B8] | `lib.rs:217-222` and `:292-295`. `reset` and `set_pos_unchecked` write the position byte | Yes: 0 is a valid position |

Not reached: `try_new` (`lib.rs:145-155`), `get_data` (:250-254), `set` (:261-269), `deserialize` (:338-355) and all of `read.rs`, whose `ReadBuffer` serves extendable-output hashes and has five more `unsafe` sites (`read.rs:26, 47, 72, 88, 125`).

Two things about the upstream's own hygiene: `lib.rs:41` carries `#![allow(clippy::undocumented_unsafe_blocks)] // TODO(tarcieri): document all unsafe blocks`, so the crate says some of its blocks are undocumented; and the stale comment in B1. Neither is a fault in the code as read. The weekly Miri run (section 11) is what checks this code for undefined behaviour in our use.

### 4.3 `hybrid-array`

The same code runs on every target. `Array<T, U>` is a `repr(transparent)` wrapper over `[T; N]` (`src/lib.rs:164-165`); most `unsafe` converts between it and slices or arrays. Its soundness rests on one fact: `ArraySize::USIZE` equals the array length. That is the contract of an `unsafe trait` (`src/traits.rs:14-22`) implemented by one macro that writes each pair side by side (`src/sizes.rs:21-43`; `32 => U32` at :89 and `64 => U64` at :121, the only two sizes the calls use for data). A `#[test]` in that file checks every pair; the test is not part of our build.

| Site | Where, what it does | Sound as read |
| --- | --- | --- |
| [H1] | `src/lib.rs:177-189`. `as_slice` and `as_mut_slice`: `from_raw_parts(self.as_ptr(), U::USIZE)`. Reached through `Index` and `IndexMut` (:864-886) | Yes, given the contract above |
| [H2] | `src/lib.rs:332-346`. `slice_as_chunks`: splits a slice into `len / 64` arrays of 64 bytes and a tail of under 64. Called by `block-buffer` for the bulk of the input | Yes. The lengths are computed without overflow (`tail_pos = 64 * (len / 64)` never exceeds `len`); `Array<u8, U64>` has the size of `[u8; 64]` and alignment 1; the borrows keep the input's lifetime |
| [H3] | `src/lib.rs:521-524`. `cast_slice_to_core`: `&[Array<u8, U64>]` to `&[[u8; 64]]`, called by `sha2` | Yes: same layout |
| [H4] | `src/lib.rs:540-551` and `:560-575`. `Array::uninit` and `assume_init` (a `transmute_copy` of a fully written array). Used by `Array::default` through `from_fn` (`src/from_fn.rs:16-33`), which builds the 32-byte output and `finalize`'s scratch | Yes. `uninit` makes an array of `MaybeUninit`, which needs no initialisation; `assume_init` runs only after every element was written |
| [H5] | `src/from_fn.rs:40-58, 80-90, 93-106`. A guard that writes each element with `get_unchecked_mut(initialized)` and drops the written prefix if the generator panics | Yes. The loop condition `initialized < len` keeps every write in bounds; for `u8` the drop does nothing |
| [H6] | `src/traits.rs:22` and `src/sizes.rs:26` (`unsafe trait ArraySize` and its impls), and `unsafe impl Send` and `Sync` for `Array` (`src/lib.rs:945, 949`) | Yes: marker contracts, discussed above. `Send` and `Sync` follow the element type |

Not reached: `split`, `split_ref`, `split_ref_mut` and `concat` (`lib.rs:222-290`), `slice_as_array` and its mutable form (:297-323), `slice_as_flattened` and its mutable form (:376-401), the `cast_from_core` and `cast_slice_from_core` families, `TryFrom<&[T]>` (:963, :979), all of `flatten.rs`, and the `bytemuck`, `zerocopy` and `serde` impls, which are off.

### 4.4 `cpufeatures` and `libc`

`cpufeatures` is a macro, `cpufeatures::new!`, that `sha2` expands once per target for SHA-256 (`shani_cpuid` on x86 at `sha256.rs:55`, `sha2_hwcap` on AArch64 at `:58`) and once for SHA-512. Each expansion makes a module with the cache static of section 6. The detection code is chosen by target.

| Site | Where, what it does | Sound as read | Compiled for |
| --- | --- | --- | --- |
| [C1] | `src/x86.rs:41-51`. `unsafe fn cpuid` and `cpuid_count` call `__cpuid` and `__cpuid_count`, run for leaf 1 and for leaf 7 sub-leaves 0 and 1. The bit positions it then reads are in the table at `x86.rs:107-149`: `sha` is leaf 7 `ebx` bit 29 (:135), `sse2` leaf 1 `edx` bit 26 (:121), `ssse3` leaf 1 `ecx` bit 9 (:110), `sse4.1` leaf 1 `ecx` bit 19 (:112) | Yes. `cpuid` exists on x86-64 and touches no memory. See section 13, item 6, for one place where leaf 7 is read without a check | x86-64, every OS, unless all four features are on at build time, when the call is compiled out |
| [C2] | `src/x86.rs:71`. `_xgetbv`, which asks the operating system which registers it saves | It sits in the match arms for `xmm`, `ymm` and `zmm` features. The four features SHA-256 needs have an empty register class (`x86.rs:110-135`), so these arms are never taken. SHA-512's `avx2` check would take one; the kernel does not call it | x86-64 (the arms compile, and are dead for SHA-256) |
| [C3] | `src/aarch64.rs:34-38`. `getauxval_hwcap` calls `libc::getauxval(AT_HWCAP)`; `sha2` is `HWCAP_SHA2` (`aarch64.rs:71, 89`) | Yes: an FFI call that passes a number and returns a number, no pointers | AArch64 Linux and Android only |
| [C4] | `src/aarch64.rs:133-173`. `sysctlbyname`, which passes pointers to a local | Yes as read, with `assert!`s on its result. It is not called for SHA-256, because `check!("sha2")` is the constant `true` on Apple (:118-120); and in a default AArch64 macOS build not even SHA-512 calls it, because rustc turns `sha2` and `sha3` on for that target (section 7) | AArch64 Apple. Compiled, not called |

`libc` itself contributes the `extern` declaration of `getauxval` (`src/unix/linux_like/linux/gnu/mod.rs:973`, and the musl and Android copies) and the `HWCAP_*` constants (`HWCAP_SHA2` is `1 << 6`, `.../gnu/b64/aarch64/mod.rs:486`). It has `build.rs`, 630 lines, which only runs when `libc` is in the graph, so on AArch64 Linux, Android and Apple builds. As read, it:

- reads Cargo's `CARGO_CFG_*` variables and a few of its own (`LIBC_BUILD_VERBOSE`, `LIBC_CI`, `RUST_LIBC_UNSTABLE_*`, `WIND_RELEASE_ID`);
- runs `rustc --version`, through `RUSTC` and `RUSTC_WRAPPER` (:391-419);
- runs `emcc` and `freebsd-version` only for Emscripten, and for FreeBSD with `LIBC_CI` set, which are not our targets;
- prints `cargo:rustc-cfg` and `cargo:rustc-check-cfg` lines (:617-620), and sets `linux_time_bits64` on 64-bit Linux (:359-361).

It writes no files and uses no network. Like any build script, it runs on the machine that builds.

## 5. Panics

In a release build with overflow checks off, nothing in the code the calls reach can panic on valid input at any size the machine can hold. The places that look as if they could:

- **Overflow, only with overflow checks on (debug, and the CI release legs with `-C overflow-checks=on`, `kernel.yml:135, 149, 158, 167`).** `sha2` `block_api.rs:39` adds the block count to a `u64`, and `:66` computes the bit length `8 * (pos + 64 * block_len)` in a `u64`. The first overflows after 2^64 blocks and the second after 2^61 bytes (2 exbibytes) of input. Without overflow checks they wrap, and SHA-256 is not defined past 2^64 bits anyway. At 200 to 400 MB/s (32.20's figure) 2^61 bytes is about 180 to 360 years of hashing.
- **`unwrap` and `expect`, each on a case that cannot fail for SHA-256.** `digest` `ct_variable.rs:127` unwraps `Sha256VarCore::new(32)`, which returns `Ok` for 28 and 32 (`block_api.rs:54-58`). `sha256/soft.rs:13` is in section 4.1. `block-buffer` `lib.rs:378-380` is `checked_sub(8).expect(...)` on `64 - 8`.
- **Other `split_at`, `copy_from_slice`, slice and `assert!` sites.** `block-buffer` `lib.rs:185` `split_at(rem)` runs only after `n >= rem` was established; its `lib.rs:398` copies 8 bytes into 8; `digest` `ct_variable.rs:111` computes `32 - 32`; `hybrid-array` `lib.rs:333` asserts the constant `64 != 0`.
- **Backends.** S1 indexes `K32X4` with constants below 16; S2 slices `state` and `block` at constant ranges and indexes `K32` at most at 60 plus 3; the software path indexes `block[(i - 15) % 16]` for `i` from 16 up, so nothing underflows; none panics.
- **Debug assertions only.** `block-buffer`'s `debug_assert!` in `get_pos` (`lib.rs:239`), `set_pos_unchecked` (:293) and `sealed.rs:51`. They fire only if the invariant were already broken.

On the three wasm32 targets rustc's default panic strategy is abort (read from `rustc --print cfg`), so a panic there would stop the program rather than unwind.

## 6. Allocation and other effects

- **Allocation: none.** No `alloc` feature is on in `sha2`, `digest` or `hybrid-array`. Their `extern crate alloc` is behind that feature (`digest` `src/lib.rs:36-38`; `hybrid-array` `src/lib.rs:98-99`), and `sha2` has none. The state of one `Sha256` is a `[u32; 8]`, a `u64` and a 64-byte buffer, 104 bytes by the field types (not measured). There is no recursion.
- **Mutable global state: one cache per feature check, in `cpufeatures`.** `static STORAGE: AtomicU8` (`src/lib.rs:41`), inside each `cpufeatures::new!` module, starts at 255 (not yet asked) and holds 0 or 1 after the first call (:83-91). Loads and stores are `Relaxed`. Two threads that race both ask the processor the same question and store the same answer. `sha2` creates two such statics on a target, one for SHA-256 (`shani_cpuid` on x86-64, `sha2_hwcap` on AArch64) and one for SHA-512; SHA-256 touches only its own. The static is not read at all where the feature is on at build time (AArch64 macOS, and `-C target-cpu=native` on a processor with the feature), because `get()` is then a constant (`x86.rs:13-28`, `aarch64.rs:9-21`).
- **It cannot change a digest.** Both answers lead to code that computes FIPS 180-4's function (section 8). Principle 7 bans this kind of state from the kernel's own crates. Here it is a cache of a fact about the machine inside a vetted crate, the same call G24 made for `libm`'s fused multiply-add pointer, and the audit accepts it on the same ground.
- **Read-only statics:** `K32` and `K32X4` (`consts.rs:37, 73`).
- **None of these:** thread-locals, `static mut`, lazy initialisation, I/O, clock, randomness, environment variables at run time. I searched every source file of the eight crates other than `libc` for `static`, `thread_local`, `Atomic`, `OnceCell`, `OnceLock` and `std::`. The only mutable statics are the `cpufeatures` caches. The other hits are the read-only tables above, `extern crate alloc` lines behind features that are off, doc-test lines, and a `std::arch::asm!` in `cpufeatures`' LoongArch file, which is not compiled for any kernel target.

## 7. Which path runs where, and what can force one

| Target | SHA-256 path | Chosen by |
| --- | --- | --- |
| x86-64: Linux, macOS (Intel), Windows | SHA-NI (S1) if the processor has `sha`, `sse2`, `ssse3` and `sse4.1`, otherwise unrolled software (S3) | Run time, by `cpuid` (C1). At build time instead if the build enables all four features, which `-C target-cpu=native` does on a processor that has them |
| AArch64 Linux | AArch64 SHA2 (S2) if the kernel reports `HWCAP_SHA2`, otherwise software | Run time, by `getauxval` (C3) |
| AArch64 macOS | AArch64 SHA2 (S2), always | Compile time. rustc's default features for `aarch64-apple-darwin` include `sha2` and `sha3`, so `get()` is a constant `true`. No libc call is made |
| AArch64 Windows | Unrolled software (S3) | The run-time check is a constant `false` on Windows, because `cpufeatures` has no Windows detection (`aarch64.rs:176-183`). S2 is compiled but never called, unless the build enables `sha2` (for instance `-C target-cpu=native`) |
| wasm32: `wasm32v1-none`, `wasm32-unknown-unknown`, `wasm32-wasip1` | Unrolled software (S3) | Compile time. rustc's default features for all three have no `simd128`, and the kernel sets none. `cpufeatures` is not compiled |

The default features are from `rustc --print cfg --target <target>` run with the repository's pinned toolchain, rustc 1.99.0, on 2026-10-31. For example `x86_64-unknown-linux-gnu` lists `sse2` only; `aarch64-unknown-linux-gnu` and `aarch64-pc-windows-msvc` list `neon` only; `aarch64-apple-darwin` lists `aes`, `sha2`, `sha3` and more than twenty others; `wasm32v1-none` lists `mutable-globals`, and `wasm32-unknown-unknown` and `wasm32-wasip1` add `bulk-memory`, `multivalue`, `nontrapping-fptoint`, `reference-types` and `sign-ext`.

What can force a different path, and what the kernel's build sets (nothing of these):

- **A `--cfg` flag, in `RUSTFLAGS` or `.cargo/config.toml`, never a Cargo feature** (`sha256.rs:1-80`; `README.md`'s Backends section):
  - `sha2_backend="soft"` or `sha2_256_backend="soft"` forces the software path.
  - `sha2_backend_soft="compact"` picks the compact software path (`soft.rs:2`).
  - `sha2_256_backend="x86-sha"` forces S1 and refuses to compile without the four target features (:21-27).
  - `sha2_*="riscv-zknh"` forces a RISC-V backend (needs a nightly compiler).
  - `sha2_256_backend="aarch64-sha2"` would not compile as published: its wrapper at `sha256.rs:39` takes `&mut [u64; 8]` and `&[[u8; 128]]`, and the function it calls (`aarch64_sha2.rs:13`) takes `&mut [u32; 8]` and `&[[u8; 64]]`. SHA-512's `x86-avx2` wrapper has the same swap the other way (`sha512.rs:24`). Both fail to build, so they fail closed, and are upstream faults on paths we do not use.
- **A target feature.** `simd128` on a wasm32 build selects `sha256/wasm32_simd128.rs` (`sha256.rs:46-48`). Enabling all four of `sha`, `sse2`, `ssse3` and `sse4.1` (x86-64), or `sha2` (AArch64), at build time turns the run-time check into a constant true.
- **The kernel's own build:** a search of the repository for `sha2_backend`, `sha2_256_backend`, `sha2_backend_soft` and `simd128` finds nothing. The repository has no `.cargo/config.toml`. The CI sets `-C overflow-checks=on`, `-C target-cpu=native` (native legs only, `kernel.yml:146-160`), `-D warnings`, `-C llvm-args=-align-all-functions=6` (`kernel-bench.yml:299`), `-C target-feature=+wide-arithmetic` for a wasm32 build (`kernel-variants.yml:106`), and, in the coverage job, `-Z cross-crate-inline-threshold=never -C link-dead-code` (`kernel-quality.yml:461`). None selects a SHA-256 path, except that `target-cpu=native` can make the hardware path a compile-time choice on a processor that has it. I found no check that would fail a future workflow that sets one of these flags (section 13).

## 8. Determinism

Every path computes SHA-256 as FIPS 180-4 defines it, for these reasons:

- **One function, written four times in the build.** The two software paths (S3 and the compact one), SHA-NI (S1) and AArch64 SHA2 (S2) all do the same 64 rounds of 32-bit wrapping integer arithmetic on the same state and message schedule. There is no floating point and no data-dependent table. I read the software paths round by round against the standard's functions (`Ch`, `Maj`, the two `Sigma` and two `sigma` functions, the schedule recurrence), and the memory behaviour of S1 and S2. I did not re-derive the two hardware instruction sequences; the NIST vectors in `geom-op`'s tests do that on every leg where the path runs (section 11).
- **The constants are the standard's.** I computed the first 32 bits of the fractional parts of the cube roots of the first 64 primes and of the square roots of the first 8 primes, in exact integer arithmetic, and compared them with `K32` (`consts.rs:37`) and `H256_256` (`consts.rs:11-14`): all 64 and all 8 match.
- **Padding and length are one code.** `block-buffer`'s `digest_pad` and `sha2`'s `finalize_variable_core` run on every target, so message padding, the 64-bit big-endian bit length and the big-endian output (`block_api.rs:63-72`) do not differ by target. The software path reads and writes words with explicit `from_be_bytes` (`soft.rs:14`) and `to_be_bytes`. The two hardware paths rely on little-endian loads and a byte swap (`aarch64_sha2.rs:26` says so); x86-64, AArch64 and wasm32 are all little-endian.
- **Pointer width does not matter.** The counts are `u64`; wasm32's 32-bit `usize` only bounds one `update` call.
- **The cache and the build-time choice cannot change the answer.** Which path runs depends on the machine (section 7), and each path gives the same digest.

What this does not cover: the speed differs by a large factor between paths, which is why determinism rests on tests and on this reading, not on timing.

## 9. `no_std`

Every crate is `#![no_std]` in our build:

| Crate | Where |
| --- | --- |
| `sha2` | `src/lib.rs:1` |
| `digest` | `src/lib.rs:1` |
| `block-buffer` | `src/lib.rs:36` |
| `crypto-common` | `src/lib.rs:1` |
| `hybrid-array` | `src/lib.rs:1` |
| `typenum` | `src/lib.rs:42` |
| `cfg-if` | `src/lib.rs:32` |
| `cpufeatures` | `src/lib.rs:1` |
| `libc` | `src/lib.rs:219`: `no_std` unless its internal `rustc-dep-of-std` feature is on |

None of them uses `std` or `alloc` in our build (section 6), so each builds against `core` alone. `geom-op`, with the whole chain, builds for `wasm32v1-none`, a target with no `std` at all, as the gate's build does (checked when this note was committed).

## 10. Pinning

`geom-op` asks for `sha2` with `version = "0.11.0"` (`geom-op/Cargo.toml:18`), `default-features = false`. That is a caret requirement, so what holds the version is `Cargo.lock`, the checksums of section 1, and the gate's exact-version allow-list entries (`sha2@0.11.0` and the eight others in `deny-no-std.toml`). `geom-test` asks for `0.11.0` (`Cargo.toml:36`) and `geom-bench` for `=0.11.0` (`Cargo.toml:37`), both with default features off. The requirements inside the chain, from the published `Cargo.toml` files, are `digest ^0.11` and `cfg-if ^1` and `cpufeatures ^0.3` (from `sha2`); `block-buffer ^0.12`, `crypto-common ^0.2` (from `digest`); `hybrid-array ^0.4` and `^0.4.7`; `typenum ^1.20`; and `libc ^0.2.155` (from `cpufeatures`). Each is a range, so a `cargo update` could move a crate inside it. The gate's entries are exact (`digest@0.11.3` and so on), so a moved lock fails the gate until a new record is written, as for `libm`.

## 11. How the kernel's own checks stand behind this

- **NIST vectors.** `geom-op`'s `tests/hash.rs` checks SHA-256 and its 128-bit cut against the Cryptographic Algorithm Validation Program's byte-oriented short and long message files and the Monte Carlo chain, kept as published and checked against their own SHA-256. I read the head of that file, not each test.
- **The CI matrix.** `kernel.yml` (lines 94-112) runs the goldens and tests on Linux, Windows and macOS for x86-64 and for arm64, in debug and release, with the release legs also run natively with `-C target-cpu=native` (:146-160), and on `wasm32-wasip1` under wasmtime and under Node (:198-). From section 7, that is the software path on wasm32 and on Windows arm64 in the default builds, the AArch64 path on macOS arm64 and, if the runner reports it, on Linux arm64, and the SHA-NI path on the x86-64 runners if their processors have it. Which path each leg ran is not recorded by the code, so that is inferred from the source, not observed. Line 145 prints `rustc --print cfg -C target-cpu=native`, which shows in a leg's log whether the runner's processor has `sha`.
- **Miri.** `kernel-quality.yml:41-47` runs every kernel crate's tests under Miri weekly, "to catch undefined behaviour inside dependencies". Under Miri, `cpufeatures` reports no features (`src/miri.rs:6-20`), so the run covers the software path, `block-buffer` and `hybrid-array`, which is where most of the `unsafe` of sections 4.2 and 4.3 is, and not the SHA-NI and AArch64 backends, which Miri cannot run. I read that the workflow does this, not a Miri report, and did not check that `geom-op`'s hash tests are among the tests that finish inside Miri's per-test budget.

## 12. The claims in 32.20 and in the `deny-no-std.toml` records, checked

| Claim | Verdict |
| --- | --- |
| 32.20: "pure Rust, written to FIPS 180-4" | Holds for the kernel's targets. The x86 and AArch64 backends use `core::arch` intrinsics; only the LoongArch and RISC-V backends use `asm!`, and they are not compiled. The constants match the standard |
| 32.20: "`default-features = false`, `no_std`, and its `digest` dependency chain" | Holds. The chain is eight more crates, two of them target-specific |
| 32.20: "Its `unsafe` code is confined to the processor-specific backends ... and to its unrolled software path's table read" | True of `sha2`'s own code (S1 to S4). Not true of the chain: `block-buffer` (B1 to B8), `hybrid-array` (H1 to H6) and `cpufeatures` (C1 to C4) have `unsafe` that every call reaches. The same sentence is in `hash.rs`'s module comment (lines 22-23). Suggested wording: "Its `unsafe` code is in its processor backends, its software path's table read, and, in the crates it brings, the block buffer, the array casts and the processor-feature probe; the audit reads each" |
| 32.20: "each behind a runtime check of the processor's features" | Holds on x86-64 and AArch64 Linux. Not on AArch64 macOS (compile time), AArch64 Windows (the check is always false), or a `-C target-cpu=native` build (a constant) |
| 32.20: "its compact software path has none" | Holds (`soft/compact.rs`, `soft.rs`). The kernel does not select it |
| 32.20: "Every backend computes the same function, so the bits never depend on the machine" | Holds as read for the software paths; the hardware paths rest on the NIST vectors (sections 8 and 11) |
| 32.20: "the tests check the NIST test vectors on every target, natively and in WebAssembly, where it runs its software path" | Holds. The three wasm32 targets run the unrolled software path with no `simd128`, as the kernel builds them |
| `cpufeatures` record: "reads at run time whether the processor has SHA instructions", "compiled only for x86, x86-64 and AArch64, never for wasm32" | Holds, with two qualifications: on AArch64 Windows it reads nothing and answers no, and where the feature is on at build time it reads nothing and answers yes |
| `libc` record: "declarations only ... AArch64 Linux, Android and Apple targets ... not compiled for x86-64 or wasm32" | Holds, with two additions: `libc` has a build script, and on Apple targets SHA-256 never calls into it |
| `block-buffer` record: "copies and counts bytes, no arithmetic on their values" | Holds. It also keeps the position in the buffer's last byte, which is where its `unsafe` is |
| `hybrid-array` record: "it copies and indexes, and computes nothing of its own" | Holds. It also has the pointer casts of section 4.3 |
| `digest` record: "the crate is `#![no_std]`, with its alloc, oid, zeroize and dev features off" | Holds. Its `block-api` feature is on (its default), and `mac`, `getrandom` and `rand_core` are off too |
| `typenum` record: "no code of it runs" | Holds: type-level numbers, folded to constants by the compiler |
| `crypto-common` and `cfg-if` records | Hold |
| `sha2` record: "it builds for wasm32v1-none in the gate's build of geom-op" | Holds: `cargo build -p geom-op --target wasm32v1-none` builds it |

## 13. Open

1. **The wording in 32.20** (section 12, third row). The decision's sentence understates where the `unsafe` is. The code is not the problem; this note is the true record, and `hash.rs`'s module comment and the `deny-no-std.toml` records were corrected to it when this note was committed. 32.20 is left as the owner recorded the decision.
2. **The mbedtls notice** (section 3). The two AArch64 backends say they are adapted from mbedtls and carry no separate notice. Settling it needs mbedtls's licence and the backends' history; I read neither. If an owner decides the notices file needs a line, G481 adds it.
3. **Which path each CI leg runs** (section 11). Every leg checks the vectors, but nothing records whether the SHA-NI path ran on an x86-64 leg or the software path ran on a Linux arm64 one. If an owner wants every path exercised on purpose, a leg that forces the software path with `--cfg sha2_backend="soft"` and a leg that fails when the hardware path is not taken would do it. I did not read a CI log, so I do not know which paths ran.
4. **The hardware paths' arithmetic is not re-derived.** I read S1 and S2 for memory safety and checked the constants, not the instruction sequences. The NIST vectors are the evidence.
5. **Backends that are not in the build.** I did not read the LoongArch and RISC-V backends, or the SHA-512 backends beyond listing their `unsafe` and their call sites. I read `sha256/wasm32_simd128.rs` in full for structure and bounds: its loads are 16 bytes at offsets `0 to 3` of the block and at `K32[0..4]` to `K32[60..64]`, in bounds. That backend is selected only if a build turns on `simd128`, and nothing in the repository does. A future build that does would change the code this audit covers, and I found no check that fails it (section 7).
6. **`cpuid` leaf 7 without a check** (C1). `cpufeatures` asks for leaf 7 sub-leaves 0 and 1 (`x86.rs:49-51`) without first reading the highest supported leaf. What a processor returns for a leaf it does not support is not in the source, and I did not research it. If a very old processor answered "SHA present" wrongly, the result would be an illegal-instruction fault, not a wrong digest. I do not expect any kernel target to be affected. I am flagging it because I cannot rule it out from the files.
7. **Upstream faults on unused paths** (section 7): the forced `aarch64-sha2` SHA-256 backend and the forced `x86-avx2` SHA-512 backend do not compile as published. They fail closed. I have not reported them upstream.
8. **Upstream hygiene in `block-buffer`** (section 4.2): a stale SAFETY comment (B1) and a TODO that some `unsafe` blocks are undocumented. The audit's evidence for this code is my reading plus the weekly Miri run.
9. **Published crate against repository, not diffed.** This audit reads the published crates, as G24 did for `libm`. The links in this note assume the published files equal the repository's at the recorded commits.
10. **Who may name `sha2`.** Settled: the crate table's registry (`REGISTRY` in the scaffold test's `table.rs`) governs only the `no_std` kernel crates, and there it lets `geom-op` and `geom-format` name `sha2`. `geom-test` and `geom-bench` are development crates, which any permissive crate may serve; `deny-no-std.toml`'s comment says "of the kernel crates".
11. **Vendoring.** The question G24's section 9 raises for `libm` applies to these nine crates in the same way. The checksums of section 1 fix the exact bytes audited, so that note's option C applies unchanged.

## Method

I read the published crates, unpacked by Cargo from the `.crate` files whose SHA-256 are in section 1, and traced from each of the four calls the code it reaches: through `digest`'s `Digest` and `buffer_fixed!` plumbing, `sha2`'s `block_api.rs` and its backend selection in `sha256.rs`, `block-buffer`'s `digest_blocks` and `digest_pad`, and `hybrid-array`'s `Array`. For each crate I recorded its licence and copyright lines; its features and build script, from `Cargo.toml`; every `unsafe` site and its bounds; every way to panic outside test code; any allocation, static or thread-local; and the `cfg` that decides whether the code compiles on each target the kernel builds. I did not build anything, run `cargo tree` or run a test; when the note was committed, `cargo tree -p geom-op -e normal --target all` listed exactly these nine crates beside the kernel's own, and `geom-op` built for `wasm32v1-none`. The commands I did run were reads: checksums of the cached `.crate` files, `rustc --print cfg` for the nine targets, searches of the repository's workflows and manifests, a small script that recomputes the SHA-256 constants from the primes, and GitHub API queries for the upstream commits and tags.

The rules audited against are chapter 29's rule 3 and the kernel's principles 5, 7 and 13 as G24 applies them, and decision 32.20's description of `sha2`.

[S1]: https://github.com/RustCrypto/hashes/blob/ffe093984c004769747e998f77da8ff7c0e7a765/sha2/src/sha256/x86_sha.rs#L44-L102
[S2]: https://github.com/RustCrypto/hashes/blob/ffe093984c004769747e998f77da8ff7c0e7a765/sha2/src/sha256/aarch64_sha2.rs#L12-L94
[S3]: https://github.com/RustCrypto/hashes/blob/ffe093984c004769747e998f77da8ff7c0e7a765/sha2/src/sha256/soft/unroll.rs#L39-L42
[S4]: https://github.com/RustCrypto/hashes/blob/ffe093984c004769747e998f77da8ff7c0e7a765/sha2/src/sha256.rs#L49-L79
[B1]: https://github.com/RustCrypto/utils/blob/cbd0963c685df025c42bed31f78c50d1bada3805/block-buffer/src/lib.rs#L109-L116
[B2]: https://github.com/RustCrypto/utils/blob/cbd0963c685df025c42bed31f78c50d1bada3805/block-buffer/src/sealed.rs#L34-L58
[B3]: https://github.com/RustCrypto/utils/blob/cbd0963c685df025c42bed31f78c50d1bada3805/block-buffer/src/lib.rs#L173-L183
[B4]: https://github.com/RustCrypto/utils/blob/cbd0963c685df025c42bed31f78c50d1bada3805/block-buffer/src/lib.rs#L184-L201
[B5]: https://github.com/RustCrypto/utils/blob/cbd0963c685df025c42bed31f78c50d1bada3805/block-buffer/src/lib.rs#L208-L212
[B6]: https://github.com/RustCrypto/utils/blob/cbd0963c685df025c42bed31f78c50d1bada3805/block-buffer/src/lib.rs#L236-L246
[B7]: https://github.com/RustCrypto/utils/blob/cbd0963c685df025c42bed31f78c50d1bada3805/block-buffer/src/lib.rs#L366-L400
[B8]: https://github.com/RustCrypto/utils/blob/cbd0963c685df025c42bed31f78c50d1bada3805/block-buffer/src/lib.rs#L217-L222
[H1]: https://github.com/RustCrypto/hybrid-array/blob/09310c55b75e2cd408951c7e2e86dba16c63fc68/src/lib.rs#L177-L189
[H2]: https://github.com/RustCrypto/hybrid-array/blob/09310c55b75e2cd408951c7e2e86dba16c63fc68/src/lib.rs#L332-L346
[H3]: https://github.com/RustCrypto/hybrid-array/blob/09310c55b75e2cd408951c7e2e86dba16c63fc68/src/lib.rs#L521-L524
[H4]: https://github.com/RustCrypto/hybrid-array/blob/09310c55b75e2cd408951c7e2e86dba16c63fc68/src/lib.rs#L540-L575
[H5]: https://github.com/RustCrypto/hybrid-array/blob/09310c55b75e2cd408951c7e2e86dba16c63fc68/src/from_fn.rs#L40-L106
[H6]: https://github.com/RustCrypto/hybrid-array/blob/09310c55b75e2cd408951c7e2e86dba16c63fc68/src/traits.rs#L14-L22
[C1]: https://github.com/RustCrypto/utils/blob/e3ac92bfd33051e146025baa2d0ce79891b7fc73/cpufeatures/src/x86.rs#L34-L55
[C2]: https://github.com/RustCrypto/utils/blob/e3ac92bfd33051e146025baa2d0ce79891b7fc73/cpufeatures/src/x86.rs#L57-L77
[C3]: https://github.com/RustCrypto/utils/blob/e3ac92bfd33051e146025baa2d0ce79891b7fc73/cpufeatures/src/aarch64.rs#L34-L38
[C4]: https://github.com/RustCrypto/utils/blob/e3ac92bfd33051e146025baa2d0ce79891b7fc73/cpufeatures/src/aarch64.rs#L133-L173
