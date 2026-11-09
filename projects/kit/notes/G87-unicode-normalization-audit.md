# G87: the written audit of unicode-normalization 0.1.26 and tinyvec 1.13.3

*Piece G87, Spare Kit geometry kernel. Written 2026-10-31 by Claude (Sonnet 5.5) so the kernel may depend on these two crates for decision 32.19's NFC check. Read-only: I read the unpacked packages and ran small scripts over their tables. I built nothing and ran no `cargo` command.*

## The answer in plain words

The kernel calls one function, `unicode_normalization::is_nfc(&str) -> bool` ([Q1]), and builds with `default-features = false`. That reaches `unicode-normalization` and one other published crate, `tinyvec`, which `unicode-normalization` always asks for, with its `alloc` feature (`Cargo.toml:65-67`). With those features `tinyvec` brings no further crate: all seven of its dependencies are optional and off (`Cargo.toml:134-165`).

Read at the exact releases, I found nothing in the code `is_nfc` reaches that should stop the kernel depending on these two crates. What to know:

- **`unsafe`.** Four `unsafe` blocks and one `unsafe fn`, all in `unicode-normalization`'s `normalize.rs`: five `char::from_u32_unchecked` calls on Hangul arithmetic, three of them inside the `unsafe fn`, plus the one call of that function. Each is sound by its arithmetic; I enumerated every Hangul syllable and every jamo pair and every result is a valid character (section 4). All are reached. `tinyvec` has `#![forbid(unsafe_code)]` and no `unsafe`.
- **Panics.** None reachable for any `&str`, with overflow checks on or off. The perfect-hash lookups index in range because each table's two arrays have the same length (checked by script). Every slice taken from the tables is in range (checked for every entry). The buffer indexes follow an invariant I traced (section 5).
- **Allocation.** It can allocate, on the fallback path only. ASCII strings and strings the quick check settles never do. A run of four or more combining marks after a letter can fill `tinyvec`'s four inline slots and move to the heap (section 6). With `std` off the `alloc` crate is still required.
- **Determinism.** The answer is a pure function of the string, the same on x86-64, aarch64 and wasm32. No float, no seeded hash, no `cfg(target_*)`. The perfect hash uses `u32` and `u64` arithmetic, so a 32-bit `usize` changes nothing (section 7).
- **Unicode version.** `UNICODE_VERSION` is `(18, 0, 0)` ([V1]). The tables come from `scripts/unicode.py`. I did not compare them with unicode.org (section 8).
- **Notices.** The licence is `(MIT OR Apache-2.0) AND Unicode-3.0`, so the Unicode notice ships whichever of MIT or Apache is chosen. `tinyvec`'s MIT file has no copyright holder line (section 3).

Two things differ from what "built without `std`" suggests. The crate is not dependency-free, and it is not allocation-free. Section 9 sets the decision's wording against the source.

## 1. What was audited

| | `unicode-normalization` 0.1.26 | `tinyvec` 1.13.3 |
| --- | --- | --- |
| SHA-256 of the `.crate` | `75cd3fb03d5288b3ba9a1ee103ac7e9cbb59f0b03d64f8ddfcc04dd42758a85d` | `fd3ca314f692efd6c868f8408f53fe444634a845f96c028b97d35f6a1f79f0ee` |
| Source commit (`.cargo_vcs_info.json`, repository root) | [`16c9597b73ec5b6b8ed55f4249a5ac854892544c`](https://github.com/unicode-rs/unicode-normalization/tree/16c9597b73ec5b6b8ed55f4249a5ac854892544c) | [`e343bbbb5a6594c9d57de718e6f232e6c0115dcb`](https://github.com/Lokathor/tinyvec/tree/e343bbbb5a6594c9d57de718e6f232e6c0115dcb) (repository from `Cargo.toml:41`) |
| Licence (declared) | `(MIT OR Apache-2.0) AND Unicode-3.0` (`Cargo.toml:50`) | `Zlib OR Apache-2.0 OR MIT` (`Cargo.toml:40`) |
| Size | `.crate` 130,122 bytes. 12 files in `src`, 24,571 lines, of which 22,993 are the generated `tables.rs` | `.crate` 60,306 bytes. 8 files in `src`, 5,709 lines |
| Features on in our build | none. `default = ["std"]`, `std = []` (`Cargo.toml:53-55`); `std` only switches `no_std` off (`lib.rs:45`) and enables no other crate | `alloc` only, asked for by `unicode-normalization` (`Cargo.toml:67`). `default = []` (`:74`) |
| Dependencies | `tinyvec ^1`, not optional | none in this build. Seven optional, all off; the old `tinyvec_macros` is now an empty feature (`:90`) |
| Build script, proc macro | none (`build = false`, `:21`); no macros | none (`build = false`, `:17`); only `macro_rules!` macros (`tiny_vec!` `src/tinyvec.rs:57`, `array_vec!` `src/arrayvec.rs:30`) |
| Compiled for | every target. No `cfg(target_*)` anywhere in `src` | every target. Same |
| Edition, declared minimum Rust | 2018, 1.36 (`:13-14`) | 2018, none declared (its README badge says 1.47) |

- I computed both SHA-256 values from the `.crate` files and they equal the values I was given for `Cargo.lock`. I did not read that lock file. So I cannot say whether other kernel crates ask `tinyvec` for more features; the feature sets above come from the manifests.
- I compared every file of the unpacked folders with the bytes in the `.crate`: 26 of 26 and 38 of 38 equal, no extra files.
- The links assume the published files equal the repository's at those commits. I did not diff them or check the commits on GitHub. An earlier draft read the `unicode-normalization` commit page: message "Release 0.1.26", only the version line of `Cargo.toml` changed, no tag seen.
- Older copies of `tinyvec` 1.10.0 and `tinyvec_macros` 0.1.1 were unpacked beside these. They are not audited and not in the build.

## 2. What the kernel calls

| Step | Code reached |
| --- | --- |
| `is_nfc` | `quick_check.rs:109-117`. Calls `is_nfc_quick(s.chars())`. `Yes` gives `true`, `No` gives `false`, `Maybe` gives `s.chars().eq(s.chars().nfc())` (`:115`). Re-exported at `lib.rs:56-60` |
| Quick check | `is_nfc_quick` (`:74-77`) calls `quick_check(s, tables::qc_nfc, false)` ([Q2]). ASCII is skipped (`:34-38`). Other characters get `canonical_combining_class` (`lookups.rs:19-28`, then `mph_lookup` and `my_hash`, `perfect_hash.rs:16-50`, and the key and value helpers `lookups.rs:104-114`). A mark with a lower class than the one before it returns `No` (`:42-44`). `tables::qc_nfc` (`tables.rs:21335-21462`) is one `match` on ranges returning `Yes`, `No` or `Maybe`. The stream-safe branch (`:52-65`) is never taken, because `false` is passed |
| Building the iterator | `.nfc()` (`lib.rs:202-216`, the impl for any `Iterator<Item = char>`) calls `Recompositions::new_canonical` (`recompose.rs:41-49`), which builds `Decompositions::new_canonical` (`decompose.rs:45-52`). Each makes a `TinyVec::new()` |
| Recomposition | `Recompositions::next` (`recompose.rs:67-158`). Uses `canonical_combining_class` and `compose` (`normalize.rs:110-112`), which tries `compose_hangul` (`:182-212`) then `composition_table` (`lookups.rs:30-43`). That is `mph_lookup` on `COMPOSITION_TABLE_*` for pairs below U+10000, or the `match` `composition_table_astral` (`tables.rs:1233-1270`). Then `TinyVec` `push`, `get` and `clear` on a `TinyVec<[char; 4]>` |
| Decomposition | `Decompositions::next` (`decompose.rs:113-156`) calls `decompose_canonical` (`normalize.rs:22-28`), then `decompose` (`:74-105`): ASCII, then Hangul (`is_hangul_syllable` `:135-139`, `decompose_hangul` `:141-169`), then `canonical_fully_decomposed` (`lookups.rs:45-55`: `mph_lookup` on `CANONICAL_DECOMPOSED_*`, then a slice of `CANONICAL_DECOMPOSED_CHARS`). Also `push_back`, `sort_pending`, `reset_buffer` and `increment_next_ready` (`decompose.rs:71-110`) on a `TinyVec<[(u8, char); 4]>` |
| `tinyvec` | `TinyVec::new` and `default` (`tinyvec.rs:151-156, 1122-1124`), `push` ([T2]), `Deref` and `Index` (`:158-191`), `len` and `truncate` (`impl_mirrored!`, `:808-873`), `clear` (`:950-952`). Inside: `ArrayVec` `try_push` ([T4]), `Deref` (`arrayvec.rs:155-168`), `truncate` (`:1198-1213`), `drain_to_vec_and_reserve` ([T3]), `set_len` (`:1039-1056`) |
| Standard library | `Iterator::eq`, `Chars`, `Fuse`, `[T]::sort_by_key`, `Vec`. Not audited here |

Compiled in our build but not reached: in `unicode-normalization`, `replace.rs`, `stream_safe.rs`, `__test_api.rs`, the other `is_*` functions, `decompose_compatible` and `decompose_cjk_compat_variants`, and the compatibility, CJK, combining-mark and trailing-nonstarter tables. In `tinyvec`, all of `slicevec.rs` and `arrayvec_drain.rs`, and every `ArrayVec` and `TinyVec` method not listed above. `array/generic_array_impl.rs` is behind the `generic-array` feature, off.

## 3. Notices

| Package, file | What it carries |
| --- | --- |
| `unicode-normalization`, `LICENSE-MIT` | "Copyright (c) 2015 The Rust Project Developers" and the MIT text |
| `LICENSE-APACHE` | Apache-2.0 text, 201 lines. The appendix is the unfilled template (`Copyright [yyyy] [name of copyright owner]`, line 189) |
| `COPYRIGHT` | 7 lines: licensed under Apache-2.0 or MIT, at your option |
| `LICENSE-UNICODE` | Unicode License V3, "Copyright © 1991-2023 Unicode, Inc." It requires this copyright and permission notice to appear with all copies of the Data Files or Software, or in associated documentation |
| Source headers | "Copyright 2012-2015 The Rust Project Developers" at line 1 of `lib.rs`, `decompose.rs`, `normalize.rs`, `recompose.rs`, `replace.rs`, `test.rs`. "Copyright 2012-2018" in `tables.rs`, and "2011-2018" in `scripts/unicode.py`. "Copyright 2019" in `lookups.rs` and `perfect_hash.rs`. None in `quick_check.rs`, `stream_safe.rs`, `__test_api.rs`. `Cargo.toml` authors: kwantam, Manish Goregaokar |
| `tinyvec`, `LICENSE-ZLIB.md` | `Copyright (c) 2019 Daniel "Lokathor" Gee.` and the Zlib terms |
| `LICENSE-MIT.md` | The MIT text only. It refers to "the above copyright notice" but has no copyright line |
| `LICENSE-APACHE.md` | Apache-2.0 text, 202 lines, unfilled template at line 190 |
| Source files | no copyright header in any file under `src` |

Neither `.crate` has a `NOTICE` file, so the Apache-2.0 clause that carries one has nothing to carry.

What a third-party notices file must keep:

- **`unicode-normalization`:** the MIT text with "Copyright (c) 2015 The Rust Project Developers", or, if Apache-2.0 is chosen, the licence text and the same copyright lines. The source headers name The Rust Project Developers for 2012 to 2019, so keep those years too.
- **Unicode, either way.** The licence is `AND`, so the Unicode License V3 text, with its copyright line, ships whichever of MIT or Apache-2.0 is chosen. The crate does not say which files it covers. `tables.rs` is the Unicode-derived data, so treat the notice as covering the compiled tables. Keep the text as shipped. Its years end at 2023 while the data is Unicode 18.0.0; I would not edit that.
- **`tinyvec`:** the election is the owners'. For Zlib: keep the Zlib text with the holder line; its third condition says the notice may not be removed from a source distribution, and an acknowledgment in product documentation is welcome but not required. For MIT: the MIT file has no holder line, so carry the holder line from the Zlib file with it, since that is the only copyright line in the package. For Apache-2.0: the licence text.
- These belong in the generated third-party notices file.

## 4. `unsafe`

`lib.rs:40` is `#![deny(missing_docs, unsafe_code)]`. I searched `src`, `scripts` and `benches` for the word `unsafe`. It appears only in `normalize.rs`, under three `#[allow(unsafe_code)]` attributes (`:75`, `:143`, `:183`).

| Site | Where, what it does | Sound as read |
| --- | --- | --- |
| [U1] | `normalize.rs:88-93`. `decompose` calls `unsafe fn decompose_hangul` after `is_hangul_syllable(c)` | Yes. The contract is that `s` is a Hangul syllable. `is_hangul_syllable` tests U+AC00 to U+D7A3 (`:135-139`). It is the only caller and the function is private |
| [U2] | `:141-169`. `decompose_hangul`: three `from_u32_unchecked` calls (`:155, :160, :166`) | Yes. `s_index = s - 0xAC00` is 0 to 11171. L is `0x1100 + s_index / 588`, at most U+1112. V is `0x1161 + (s_index % 588) / 28`, at most U+1175. T is `0x11A7 + s_index % 28`, only if nonzero, so U+11A8 to U+11C2. All below U+D800. I enumerated all 11,172 syllables: every output is a valid character |
| [U3] | `:190-202`. `compose_hangul`, L plus V: `from_u32_unchecked(0xAC00 + l * 588 + v * 28)` | Yes. The match arm limits `a` to U+1100 to U+1112 and `b` to U+1161 to U+1175. The largest result is U+D788. I enumerated all 19 x 21 pairs: all valid and inside the Hangul block |
| [U4] | `:204-209`. `compose_hangul`, LV plus T: `from_u32_unchecked(a + (b - 0x11A7))` | Yes. `a` is an LV syllable (guard at `:204`), `b` is U+11A8 to U+11C2, so the sum adds 1 to 27 and is at most U+D7A3. Without the guard the largest would be U+D7BE, still below U+D800. I enumerated all 10,773 pairs: all valid |

- **Stale comments.** Some SAFETY comments carry wrong numbers. Lines `:195-198` give the LV bound as 8400 and U+CCD0; the real figures are 11144 and U+D788. Lines `:205-207` give the added amount as 0 to 19; it is 1 to 27. Each comment's conclusion still holds. These are comment faults, not code faults.
- **Reached.** U1 and U2 run for any precomposed Hangul syllable in a string that gets the full check. U3 and U4 run in `compose` for any adjacent pair in the right ranges. All of it is on the fallback path only.
- **Compiled but not reached:** none in `unicode-normalization`. Every `unsafe` operation is on a path `is_nfc` takes. Other callers of the same code (`decompose_compatible`, the stream-safe code) are not called.
- **`tinyvec`:** `#![forbid(unsafe_code)]` at `lib.rs:2` ([T1]). `forbid` cannot be lifted by an inner `allow`. The word `unsafe` appears only in comments (`array.rs:14`, `lib.rs:38-40`, `arrayvec.rs:495`).
- The standard library's `Vec`, `Chars` and slice sort contain `unsafe` that I did not read. I ran no Miri.

## 5. Panics

Nothing in the reached code can panic for any `&str`, with overflow checks on or off.

- **Perfect-hash indexing** (`perfect_hash.rs:43-44`). `salt[my_hash(x, 0, n)]` and `kv[my_hash(x, s, n)]` with `n = salt.len()`. `my_hash` returns `(y as u64 * n as u64) >> 32` with `y < 2^32`, so the index is below `n` for any salt value. The second index is in range only if `kv.len() >= salt.len()`. I counted the entries with a script. The two lengths are equal for every table: combining class 1002 and 1002, composition 928 and 928, canonical decomposition 2081 and 2081, and equal for the unreached tables too. All are above zero. I also checked that every key of every table sits in its own slot under the Rust formula.
- **Slices from the decomposition tables** (`lookups.rs:54`). `CHARS[start..][..len]` panics if `start + len` exceeds the array. For every entry of the canonical table the largest `start + len` is 3450, the length of `CANONICAL_DECOMPOSED_CHARS`. The same holds for the compatibility and CJK tables.
- **Buffer indexing** (`decompose.rs:87, 94-96, 153`). Whenever `ready.end` is not zero, `ready.start < ready.end <= buffer.len()`. `ready` starts as `0..0`. `push_back` sets `ready.end = buffer.len()` only just after a push. `next` sets it only to `buffer.len()`, and only when that is above zero. `increment_next_ready` moves `start` only below `end` and resets both to zero at equality. `reset_buffer` copies from `i + ready.end < len`. I traced each path, including a decomposition that emits several characters. `ArrayVec` keeps `len <= 4` (`try_push` fails at the end of the slice, `arrayvec.rs:818-819`), so its `[..len]` slice (`:159`) holds.
- **Arithmetic with overflow checks on.** `my_hash` uses `wrapping_add` and `wrapping_mul`, and the `u64` product is under 2^44. `(c1 as u32) << 16` shifts a value below 0x10000. The Hangul `u32` arithmetic is bounded (section 4). Each subtraction follows a range test (`s - S_BASE` after `is_hangul_syllable`, `a - S_BASE` in a guard after the pattern matched). Every divisor is a nonzero constant. `next + 1` (`recompose.rs:141, 151`) and `ready.start + 1` are bounded by buffer length. `tinyvec`'s `self.len += 1` is a `u16` that is at most 4, and `n + self.len()` (`arrayvec.rs:2043`) is 4 plus 4.
- **`unwrap`, `expect`, `panic!`, `assert!`.** None in the reached code of either crate. `ArrayVec::push` asserts (`arrayvec.rs:798`) but `TinyVec::push` calls `try_push` ([T2]). `set_len` panics above capacity (`:1045-1054`) and is called with 0 (`:2047`). The `debug_assert!` at `:816` holds by the invariant above. `replace.rs` pushes into a two-slot `ArrayVec`, which would panic on a third; it is not reached.
- **Out of memory.** `Vec` growth failure is handled by the standard library, which I did not read.
- **Panic strategy.** `rustc --print cfg` (1.99.0) gives `panic="abort"` for `wasm32v1-none` and `wasm32-unknown-unknown`, and `unwind` for x86-64 and aarch64 Linux. A panic on wasm32 would stop the program. None is expected.

## 6. Allocation and other effects

- **No allocation** for any ASCII string (never `Maybe`, `quick_check.rs:34-38`), and for any string the quick check settles as `Yes` or `No`. `quick_check` (`:24-69`) holds only locals. Text of precomposed letters with no `Maybe` character takes this route.
- **Fallback.** The quick check says `Maybe` if some character has NFC_QC `Maybe` (combining marks such as U+0300 to U+0304, Hangul vowel and final jamo, and more; `tables.rs:21411-21460`) and none has `No` or breaks mark order. Then `.nfc()` runs with two inline buffers of four slots: `TinyVec<[(u8, char); 4]>` (`decompose.rs:35`) and `TinyVec<[char; 4]>` (`recompose.rs:30`).
- **When `TinyVec` spills.** `push` on a full inline array moves the four items to a `Vec` with capacity 8 and pushes ([T2], [T3]): 64 bytes requested for the decomposition buffer, 32 for the recomposition buffer. The decomposition buffer holds the marks after a starter until the next starter or the end of the string. So a fifth entry arrives for five marks at the end of a string, or four marks then another starter. Marks from decomposing a precomposed letter count. The longest canonical decomposition is 4 characters, at most 3 of them marks, so one character alone never overflows; I computed this from the table. Example, derived by reading and not run: `"a"` followed by five U+0301 allocates. U+0301 has class 230 and quick check `Maybe`, so the full check runs. The recomposition buffer fills the same way, with marks that did not combine.
- **How much.** Heap use follows the longest run of combining marks, not the string length. A run above 512 pairs also needs a scratch buffer in the standard library's stable sort: `sort_by_key` (`decompose.rs:87`) uses insertion sort up to 20 elements, a 4 KiB stack scratch up to 512 eight-byte pairs, and the heap above that. I read that in the library source of the nightly-2026-10-01 toolchain, not 1.99.0's, so it may differ.
- **Freed** when the iterator drops. `clear` keeps the `Vec`'s capacity until then. No leak as read. Time is linear plus a sort of each run; I did not measure it.
- **`alloc` is required with `std` off.** `lib.rs:47-48` has `extern crate alloc`. `tinyvec`'s `alloc` feature is not optional (`Cargo.toml:65-67`; `tinyvec` `lib.rs:91-93`), and `sort_by_key` lives in `alloc`. The 1.99.0 sysroot for `wasm32v1-none` has `liballoc` and `libcore` and no `libstd`. A final binary on a target without `std` needs a global allocator; that is rustc behaviour I did not verify here.
- **Table data** is about 50 KB by field sizes for the tables `is_nfc` uses. I did not measure it; a linker may keep more.
- **Global or mutable state:** none. I searched all non-table source of both crates for `static`, `thread_local`, `Atomic`, `OnceCell` and `lazy_static`. The only `static` hits are `'static` lifetimes. The tables are `const` items.
- **I/O, clock, randomness, threads, environment:** none. The only `std::` uses in `tinyvec` are behind `feature = "std"` (`arrayvec.rs:1555-1556`, `tinyvec.rs:193-205`), which is off. `unicode-normalization` has none. No recursion.

## 7. Determinism

The answer depends only on the characters of the input. The values that decide it are `char` code points, class bytes (`u8`) and table entries (`u32`, `u16`, `char`). There is no float, no `HashMap`, no seeded hash, no pointer value and no `cfg(target_*)`. Byte order does not enter: the tables are typed constants, not byte blobs. The sort is stable, so any correct stable sort gives the same order. `Iterator::eq` stops at the first difference and does not depend on timing.

- **32-bit `usize` (wasm32).** `my_hash` (`perfect_hash.rs:16-20`) works in `u32` with wrapping operations and widens to `u64` for the product. `n` is at most 3917 for the tables `is_nfc` uses, so the product is under 2^44. After the shift by 32 the result is below `n`, so the cast to a 32-bit `usize` loses nothing. The other `usize` values (buffer lengths, `ready`, `Purging(next)`, and `start as usize` from a `u16`) only index buffers and decide nothing. The index is the same on 32 and 64 bits.
- **Tables and generator agree.** The generator's `my_hash` (`scripts/unicode.py:528-533`) is the same formula. My own port of the Rust function placed every key of all five lookup tables in its own slot, so each lookup finds exactly its key.
- **Not covered.** Correctness against the Unicode standard rests on upstream's tests. The `tests/` folder is excluded from the package (`Cargo.toml:22`), so I did not read the `NormalizationTest.txt` tests. The in-package tests (`src/test.rs`) check the iterators, not `is_nfc`, and `benches/bench.rs` calls `is_nfc` without checking it. I ran no test.

## 8. Unicode version

- `pub const UNICODE_VERSION: (u8, u8, u8) = (18, 0, 0);` is at `tables.rs:18` ([V1]) and re-exported at `lib.rs:64`. Confirmed.
- **How the tables were made.** `tables.rs:11` says it was generated by `scripts/unicode.py`. The script sets `UNICODE_VERSION = "18.0.0"` and `UCD_URL` to the 18.0.0 folder on unicode.org ([V2]). It fetches `UnicodeData.txt`, `DerivedNormalizationProps.txt`, `StandardizedVariants.txt` and `NormalizationTest.txt` (header `:13-17`; fetches `:108, :140, :177, :199`). It computes the combining classes, compositions, full decompositions and quick-check ranges, builds the perfect-hash tables (`:358`, `:528-581`), and writes `tables.rs` with the version constant (`:583-613`). It needs network access and Python 3.12 (`itertools.batched`, `:23`).
- **Kept in step by CI.** The upstream workflow re-runs the generator and diffs the result against the committed `src/tables.rs` ([V3], job `regen`). I read the workflow and saw no run.
- **Not done.** I did not run the generator (it needs the network), compare the tables with the Unicode files, or check what Unicode 18.0.0 contains. So 18.0.0 is confirmed as what the crate states, not independently.

## 9. The claims in decision 32.19

The wording is as recorded in the earlier draft. I did not read the decision itself.

| Claim | Verdict |
| --- | --- |
| Checked by `unicode-normalization` at 0.1.26 | Holds. `is_nfc` is authoritative: a full comparison runs whenever the quick check says `Maybe` ([Q1]) |
| Tables are Unicode 18.0.0 | Holds as the crate states it (section 8). Not compared with the Unicode files |
| `(MIT OR Apache-2.0)` with Unicode-3.0 data | Differs. The expression is `(MIT OR Apache-2.0) AND Unicode-3.0` for the whole package, not for data only. `tinyvec` adds `Zlib OR Apache-2.0 OR MIT`. The allow-list should use both as published |
| Built without `std` | Holds: `no_std` (`lib.rs:45`). Not enough alone: the `alloc` crate is needed (section 6) and `tinyvec` comes with it |
| (not said) no dependencies | Would be wrong. One dependency, `tinyvec`, which has none |
| (not said) allocation-free | Would be wrong for runs of combining marks (section 6) |

## Open questions

1. **Allocation.** May a kernel crate need the `alloc` crate and a global allocator? I do not know the kernel's rule. `is_nfc_quick` (`quick_check.rs:74-77`) never allocates but may answer `Maybe`; the kernel calls `is_nfc`.
2. **Input length.** `is_nfc` has no cap. Heap use grows with the longest run of combining marks. Is the length bounded before the call?
3. **Licence election** for `tinyvec`, and whether the Unicode notice covers the whole crate (section 3). The owners decide.
4. **Unicode 18.0.0 content** is not compared with the Unicode files (section 8).
5. **Conformance tests.** Upstream's `NormalizationTest` tests are not in the package and I did not read them. Does the kernel run such vectors against `is_nfc`?
6. **Sort allocation threshold** (section 6) was read from a nightly library source, not 1.99.0's, which has no `rust-src` installed here.
7. **Commits and links** are not checked on GitHub. The `v0.1.26` tag is not confirmed. The `tinyvec` commit comes from `.cargo_vcs_info.json` only. Published files are not diffed against the repository.
8. **Other `tinyvec` features.** I did not read the kernel's lock file or run `cargo tree`. Another crate in the kernel graph that names `tinyvec` could switch on its optional features.
9. **Stale SAFETY comments** in `normalize.rs` (section 4) are not reported upstream.
10. **Not run:** no build, no tests, no Miri. The `unsafe` and panic conclusions rest on reading plus the table and arithmetic scripts.

## Method

I read the unpacked packages. For `unicode-normalization`: `lib.rs`, `quick_check.rs`, `normalize.rs`, `decompose.rs`, `recompose.rs`, `lookups.rs`, `perfect_hash.rs`, `replace.rs` and `__test_api.rs` in full; `stream_safe.rs` to line 120 (the rest is tests); the first 40 lines of `test.rs`; `tables.rs` for its header, `UNICODE_VERSION`, the table declarations, `qc_nfc` and `composition_table_astral`; `scripts/unicode.py` lines 1 to 230 and 520 to 617; the manifests, licence files and CI workflow. For `tinyvec`: `lib.rs`, `array.rs`, `array/const_generic_impl.rs`, and the parts of `tinyvec.rs` and `arrayvec.rs` that `is_nfc` reaches. I did not read `slicevec.rs`, `arrayvec_drain.rs` or the unreached methods line by line; I searched all of `tinyvec` for `unsafe`, `panic!`, `assert!`, `unwrap`, `expect`, `static`, `thread_local`, `Atomic`, `std::` and `cfg(`.

Scripts I wrote (not part of either crate, read-only): one counts the entries of each table and checks, in a port of `my_hash`, that every key lands in its own slot and that every `start + len` is in range; one enumerates the Hangul arithmetic; one measures decomposition lengths and combining classes; one compares each unpacked file with the `.crate` bytes. I also ran `rustc --print cfg` for four targets, listed the 1.99.0 sysroots, and read the stable-sort source in the nightly-2026-10-01 library.

[Q1]: https://github.com/unicode-rs/unicode-normalization/blob/16c9597b73ec5b6b8ed55f4249a5ac854892544c/src/quick_check.rs#L109-L117
[Q2]: https://github.com/unicode-rs/unicode-normalization/blob/16c9597b73ec5b6b8ed55f4249a5ac854892544c/src/quick_check.rs#L22-L69
[U1]: https://github.com/unicode-rs/unicode-normalization/blob/16c9597b73ec5b6b8ed55f4249a5ac854892544c/src/normalize.rs#L88-L93
[U2]: https://github.com/unicode-rs/unicode-normalization/blob/16c9597b73ec5b6b8ed55f4249a5ac854892544c/src/normalize.rs#L141-L169
[U3]: https://github.com/unicode-rs/unicode-normalization/blob/16c9597b73ec5b6b8ed55f4249a5ac854892544c/src/normalize.rs#L190-L202
[U4]: https://github.com/unicode-rs/unicode-normalization/blob/16c9597b73ec5b6b8ed55f4249a5ac854892544c/src/normalize.rs#L204-L209
[V1]: https://github.com/unicode-rs/unicode-normalization/blob/16c9597b73ec5b6b8ed55f4249a5ac854892544c/src/tables.rs#L18
[V2]: https://github.com/unicode-rs/unicode-normalization/blob/16c9597b73ec5b6b8ed55f4249a5ac854892544c/scripts/unicode.py#L25-L26
[V3]: https://github.com/unicode-rs/unicode-normalization/blob/16c9597b73ec5b6b8ed55f4249a5ac854892544c/.github/workflows/rust.yml#L120-L132
[T1]: https://github.com/Lokathor/tinyvec/blob/e343bbbb5a6594c9d57de718e6f232e6c0115dcb/src/lib.rs#L1-L2
[T2]: https://github.com/Lokathor/tinyvec/blob/e343bbbb5a6594c9d57de718e6f232e6c0115dcb/src/tinyvec.rs#L1128-L1156
[T3]: https://github.com/Lokathor/tinyvec/blob/e343bbbb5a6594c9d57de718e6f232e6c0115dcb/src/arrayvec.rs#L2042-L2049
[T4]: https://github.com/Lokathor/tinyvec/blob/e343bbbb5a6594c9d57de718e6f232e6c0115dcb/src/arrayvec.rs#L815-L826
