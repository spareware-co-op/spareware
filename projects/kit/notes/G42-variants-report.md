# G42: exact-path variants on every target

Piece G42 of the geometry kernel's board (`projects/engines/geometry/architecture/30-build-order.md`), for chapter 23.3 of the kernel architecture (`projects/engines/geometry/architecture/23-performance.md`).

## Question

Chapter 23.3 asks which exact arithmetic each target should ship. WebAssembly's 64-bit multiply returns only the low 64 bits, so Rust's 128-bit products become calls to a library routine. Each predicate's exact path is compared in three variants, natively and in WebAssembly engines, with and without the wide-arithmetic proposal; each target then ships its fastest variant.

## The variants

| Variant | Where | What it does |
| --- | --- | --- |
| Shipped, width-aware | `geom_num::generated::<predicate>::value` | Each step in the narrowest of `i64`, `i128`, `I256`, `I320`, `I384` and `I512` its certificate allows; a product of operands that fit `i128` uses `product_i128` or `checked_mul_i128` (piece G29). 64-bit limbs, 128-bit intermediates. |
| Plain `i128` | `geom_num::generated::plain` | Each step in `i128`, or in its certificate's wide type with both operands widened to it and the general `checked_mul`. |
| Width-aware limbs | `geom_num::generated::limbs`, on `geom_num::Wide32` | Each step in `i64`, or in a `Wide32` of as many 32-bit limbs as its operands' certified bits need. Every limb product is one 32 × 32 → 64-bit multiply, and operands of different widths are multiplied without being widened first. The result comes back in the shipped type. |
| Wide-arithmetic build | `-C target-feature=+wide-arithmetic` | Not a fourth source: the same three variants built for `wasm32-wasip1` with the proposal's instructions. |

All three give the same value on every input:

- geom-gen's independent checker re-derives both variants line by line.
- `geom-num/tests/attack_g42.rs` compares them with the shipped path at every sign combination of the grid's extremes and on random inputs.
- geom-fuzz's `variants` target compares all three with each description evaluated in `num-bigint`.
- Kani proves each kind of limb step (43 kinds) never overflows on any operand its certified bits allow (`kernel-fuzz.yml`'s `kani-limbs` job).

The fourth comparison chapter 23.3 names, Avnaim, Boissonnat, Devillers, Preparata and Yvinec's sign of small determinants in b-bit arithmetic, is a different kind of method: it gives only the sign, of orient2d and orient3d, so it is compared with those two in its own section below.

## Why the variants differ on WebAssembly

The instruction each Rust operation becomes on `wasm32`, compiled by Rust 1.99 at `opt-level=3` (`--emit asm`):

| Rust | `wasm32`, no wide arithmetic | `wasm32` with `+wide-arithmetic` |
| --- | --- | --- |
| `i128::from(a).checked_mul(i128::from(b))`, `a`, `b`: `i64` | one call to `__multi3` | one `i64.mul_wide_s` |
| `u128::from(a) * u128::from(b)`, `a`, `b`: `u64` (`Wide`'s limb product) | one call to `__multi3` | one `i64.mul_wide_u` |
| `a.checked_mul(b)`, `a`, `b`: `i64` (the overflow check needs the high half) | one call to `__multi3` | one `i64.mul_wide_s` |
| `u64::from(a) * u64::from(b)`, `a`, `b`: `u32` (`Wide32`'s limb product) | one `i64.mul` | one `i64.mul` |

Without wide arithmetic, every 64-bit-limb product and every checked `i64` product is a library call whose result travels back through memory. A 32-bit-limb product never is. With wide arithmetic, every 64-bit-limb product is one instruction. Rust 1.99 accepts `+wide-arithmetic` on `wasm32-wasip1` but warns that the target feature is unstable.

## Method

geom-bench benchmarks every variant of every predicate on four input classes: typical, degenerate, nearly degenerate and extreme. Inputs are drawn from seeded generators, the same classes as the shipped `value` benchmarks, and their digests are pinned in `golden.txt`. The instruction-count and fuel gates (`kernel-bench.yml`) count them on every pull request. Wall-clock time comes from `geom-bench-guest time <benchmark> <ms>`: six rounds of passes over the batch of 64 inputs, each as many passes as fit 150 ms, with the first round a warm-up and the median of the other five reported as nanoseconds per call. `kernel-variants.yml` runs it:

- natively on GitHub's x86-64 and aarch64 Linux runners;
- for `wasm32-wasip1`, with and without wide arithmetic, in Wasmtime 50 and Node 24;
- in Chromium, Firefox and WebKit through Playwright 1.52, and in Safari through safaridriver, with `@bjorn3/browser_wasi_shim` 0.3.0 as the browsers' WASI.

An engine that does not validate a build records it as unsupported. `geom-bench/variants/report.py` writes the tables.

## Results

Nanoseconds per call on typical inputs, from `kernel-variants.yml`'s run on pull request #260 (GitHub's runners: x86-64 Intel Xeon Platinum 8573C, aarch64 Neoverse-N2). Each cell is shipped / plain `i128` / limbs, with the fastest in bold. The full tables, extreme inputs and the 32- against 64-bit-limb integers included, are in the run's report artifact.

| Predicate | x86-64 native | aarch64 native | Node 24 | Wasmtime 50 | Wasmtime 50, wide arithmetic |
| --- | --- | --- | --- | --- | --- |
| orient2d | **2.4** / 7.7 / 12.8 | **2.7** / 4.2 / 11.3 | **11.5** / 28.5 / 50.5 | **18.8** / 37.6 / 51.2 | **7.3** / 35.0 / 51.7 |
| orient3d | **12.2** / 30.5 / 75.3 | **7.1** / 11.5 / 51.8 | **70.9** / 112 / 137 | **93.1** / 149 / 155 | **56.0** / 153 / 157 |
| incircle | **39.0** / 66.8 / 142 | **35.4** / 41.7 / 78.6 | **159** / 259 / 185 | 275 / 462 / **235** | **96.8** / 389 / 240 |
| insphere | **89.9** / 172 / 585 | **62.5** / 78.1 / 300 | **416** / 641 / 680 | **696** / 977 / 835 | **349** / 993 / 887 |
| plane_side | **11.9** / 11.9 / 50.4 | 5.3 / **5.3** / 43.3 | **41.8** / 41.9 / 96.6 | **53.8** / 53.8 / 106 | 53.5 / **53.3** / 107 |
| tpi_weight | 140 / **140** / 208 | **96.5** / 105 / 142 | 328 / 398 / **232** | 598 / 768 / **277** | 324 / 525 / **278** |
| tpi_side | **691** / 714 / 849 | **460** / 529 / 580 | 1697 / 1850 / **1026** | 2835 / 3676 / **1305** | 2088 / 2647 / **1303** |
| lpi_weight | **10.4** / 14.3 / 39.2 | **5.2** / 6.2 / 34.3 | **41.7** / 42.1 / 86.1 | **52.8** / 54.1 / 96.1 | 53.6 / **52.5** / 93.9 |
| lpi_side | **70.2** / 77.0 / 288 | **33.5** / 43.6 / 208 | **228** / 317 / 320 | **349** / 478 / 363 | **299** / 382 / 362 |
| ssi_weight | **2.2** / 8.0 / 13.0 | **2.9** / 4.4 / 11.5 | **12.1** / 28.2 / 52.0 | **19.0** / 37.7 / 50.9 | **7.6** / 34.9 / 53.6 |
| ssi_side | **29.6** / 49.0 / 87.5 | **24.7** / 33.2 / 55.1 | **112** / 178 / 120 | 187 / 302 / **146** | **67.4** / 237 / 144 |

Node 24 does not validate the wide-arithmetic build.

The browsers ran through Playwright 1.52 (Chromium 136, Firefox 137, WebKit 18.4) and Safari 26.6.2 through safaridriver, on the same runners. None of the four validates the wide-arithmetic build with default settings.

| Predicate | Chromium 136 | Firefox 137 | WebKit 18.4 | Safari 26.6.2 |
| --- | --- | --- | --- | --- |
| orient2d | **16.6** / 33.1 / 55.6 | **90.6** / 174 / 286 | **9.0** / 20.2 / 51.3 | **8.9** / 16.1 / 37.2 |
| orient3d | **78.3** / 120 / 142 | **439** / 687 / 692 | **38.5** / 63.3 / 88.4 | **38.8** / 58.6 / 78.0 |
| incircle | **174** / 265 / 192 | 1002 / 1510 / **859** | **89.7** / 162 / 107 | **99.4** / 174 / 101 |
| insphere | **437** / 677 / 692 | **2630** / 3874 / 2974 | **228** / 412 / 347 | **253** / 390 / 322 |
| plane_side | **47.0** / 47.4 / 104 | 251 / **251** / 481 | 24.1 / **23.7** / 58.4 | 26.1 / **23.6** / 55.5 |
| tpi_weight | 348 / 418 / **237** | 2003 / 2040 / **1235** | 196 / 224 / **158** | 198 / 253 / **123** |
| tpi_side | 1749 / 1938 / **1045** | 10102 / 10653 / **5703** | 910 / 1302 / **745** | 1062 / 1230 / **633** |
| lpi_weight | **47.0** / 47.1 / 90.4 | 254 / **253** / 447 | **24.4** / 25.1 / 56.7 | **26.3** / 27.4 / 44.1 |
| lpi_side | **235** / 286 / 327 | **1356** / 1615 / 1466 | **128** / 192 / 175 | **130** / 177 / 156 |
| ssi_weight | **16.6** / 32.7 / 56.6 | **92.3** / 177 / 289 | **8.6** / 19.6 / 36.9 | **9.3** / 16.7 / 33.2 |
| ssi_side | **122** / 187 / 125 | 694 / 1050 / **617** | **68.1** / 147 / 71.2 | 71.5 / 115 / **68.6** |

Firefox, in Playwright's build on a Linux runner, ran every variant five to six times slower than the other engines, with the same ranking; the cause is not investigated here, and the choice below rests on each engine's own ranking.

## Choice per target

| Target | Ships | Why |
| --- | --- | --- |
| x86-64 and aarch64, native | The shipped width-aware paths, every predicate | Fastest on every predicate on both; plain `i128` ties on `plane_side` and `tpi_weight` |
| wasm32, every engine | The limb variant for `tpi_side` and `tpi_weight`; the shipped paths for the other nine | Limbs is fastest for these two on all seven WebAssembly engines and builds measured: 1.2 times (WebKit) to 2.2 times (Wasmtime). For the other nine the shipped paths are fastest or within a few percent everywhere: limbs wins `incircle` and `ssi_side` only in Firefox, in Wasmtime without wide arithmetic and, for `ssi_side`, by 4% in Safari, and loses them in Chromium, Node and WebKit |
| wasm32 with wide arithmetic | The same choice, in the build with `+wide-arithmetic` | In Wasmtime it halves the shipped paths' time on the small predicates (orient2d 18.8 → 7.3 ns, incircle 275 → 97 ns), and limbs still wins `tpi_side` and `tpi_weight`. No browser measured validates it by default, so the browser package loads it only where `WebAssembly.validate` accepts it (chapter 23.3's rule) |

The choice is per predicate and per target, and every variant gives the same value on every input. So the kernel can switch a predicate's exact path for `target_arch = "wasm32"` without changing any result. Wiring that switch into `geom-num`'s entry points is the next step.

**The I384 cost on wasm32.** The 384-bit exact path (`tpi_side`) costs 1.5 to 1.9 times native x86-64 with the limb variant (Node, Wasmtime, Chromium; WebKit and Safari 0.9 to 1.1 times, Firefox 8.3 times), against 2.5 to 4.1 times as shipped. Section 31 records it under R4.

## Avnaim et al.'s method, the reference

`geom_num::avnaim` implements both cases of the paper, as orient2d's and orient3d's reference: the sign of a 2 × 2 determinant from comparisons, subtractions and Euclidean divisions on the entries, never a product (section 2), and the sign of a 3 × 3 determinant by a walk in the lattice its first two rows span, every test of which is a 2 × 2 sign (section 3). Checking the algebra corrected two printed steps: in the 2 × 2 case, replacing the second row by the first minus the second negates the determinant, which the paper's equation omits; in the 3 × 3 case, equation 5 puts a cell's reference point two cells off along each negated axis. The module header lists every departure, each decided by the algebra. Both agree with exact arithmetic on random, near-degenerate and extreme cases (128-bit products for the 2 × 2 case; big integers for the 3 × 3 case, including inputs whose lattice walk runs its full depth of 119 doublings), with the shipped `orient2d` and `orient3d` at every sign combination of the grid's extremes (`tests/attack_g42.rs`), and with big integers in geom-fuzz's `variants` target. Measured locally (x86-64 Windows, and Node 24), in nanoseconds per call:

| Predicate | Inputs | Shipped value, native | Avnaim, native | Shipped value, Node | Avnaim, Node |
| --- | --- | ---: | ---: | ---: | ---: |
| orient2d | typical | 4.5 | 13.8 | 22.3 | 29.9 |
| orient2d | extreme | 4.5 | 15.6 | 20.9 | 27.2 |
| orient2d | degenerate | 4.6 | 79.5 | 20.9 | 224 |
| orient2d | nearly degenerate | 4.5 | 204 | 20.8 | 438 |
| orient3d | typical | 24.1 | 151 | 109 | 271 |
| orient3d | extreme | 24.4 | 401 | 119 | 623 |
| orient3d | degenerate | 24.2 | 14,059 | 109 | 20,780 |
| orient3d | nearly degenerate | 23.8 | 1,529 | 121 | 2,775 |

The method's cost grows with how close the input is to degenerate: the 2 × 2 case halves one row per pass, and the 3 × 3 case halves the largest height at least every three passes, each pass a lattice search of up to a few hundred 2 × 2 signs. The shipped paths' cost is constant. Buildings are full of exactly those inputs: walls sharing a face, aligned openings (risk R4). The method needs a fraction of the bits, but it is slower on every class and every engine measured, by 1.3 to 6.3 times on typical inputs and by up to 190 times on degenerate ones in Node (580 natively), so it is not adopted for any target. `kernel-variants.yml` times both cases beside the shipped paths, on all four input classes, on every engine.

## Open

- **Newer browsers.** Playwright 1.52's builds are older than current releases. Rerun with a current Playwright, and with each browser's wide-arithmetic flag where it has one, to learn when the wide build reaches a browser by default.
- **Budgets above 384 bits** (576 and 768 bits, 13 and 18) are unmeasured; the same variants apply to them once they are generated.
- **Firefox's factor** of five to six over the other engines is not explained.

## Appendix: the full tables

Generated by `projects/engines/geometry/crates/geom-bench/variants/report.py` from the same run: extreme inputs as well as typical, and the 64- against 32-bit-limb integers.

Nanoseconds per call; median of five rounds after a warm-up (geom-bench-guest's `time`). The fastest variant of each row is in bold.

#### aarch64-linux, native build (rustc 1.99.0 (b940084d7 2026-09-28), Neoverse-N2)

| Predicate | Inputs | Shipped | Plain `i128` | Limbs | Fastest |
| --- | --- | ---: | ---: | ---: | --- |
| incircle | typical | **35.4** | 41.7 | 78.6 | shipped |
| incircle | extreme | **35.0** | 46.5 | 82.5 | shipped |
| insphere | typical | **62.5** | 78.1 | 300 | shipped |
| insphere | extreme | **75.4** | 99.2 | 315 | shipped |
| lpi_side | typical | **33.5** | 43.6 | 208 | shipped |
| lpi_side | extreme | **48.2** | 56.1 | 206 | shipped |
| lpi_weight | typical | **5.2** | 6.2 | 34.3 | shipped |
| lpi_weight | extreme | **8.9** | 9.8 | 35.7 | shipped |
| orient2d | typical | **2.7** | 4.2 | 11.3 | shipped |
| orient2d | extreme | **2.7** | 4.2 | 12.1 | shipped |
| orient3d | typical | **7.1** | 11.5 | 51.8 | shipped |
| orient3d | extreme | **9.5** | 14.3 | 54.3 | shipped |
| plane_side | typical | 5.3 | **5.3** | 43.3 | plain |
| plane_side | extreme | 9.5 | **8.9** | 51.5 | plain |
| ssi_side | typical | **24.7** | 33.2 | 55.1 | shipped |
| ssi_side | extreme | **25.1** | 33.2 | 57.7 | shipped |
| ssi_weight | typical | **2.9** | 4.4 | 11.5 | shipped |
| ssi_weight | extreme | **2.9** | 4.4 | 12.1 | shipped |
| tpi_side | typical | **460** | 529 | 580 | shipped |
| tpi_side | extreme | **477** | 573 | 608 | shipped |
| tpi_weight | typical | **96.5** | 105 | 142 | shipped |
| tpi_weight | extreme | **101** | 114 | 146 | shipped |

| 64-bit limbs | ns | 32-bit limbs | ns |
| --- | ---: | --- | ---: |
| `wide/i256_mul/typical` | 11.9 | `wide32/w8_product/typical` | 16.9 |
| `wide/i384_mul/typical` | 19.2 | `wide32/w12_product/typical` | 32.1 |
| `wide/i256_add/typical` | 3.6 | `wide32/w8_add/typical` | 7.3 |
| `wide/i384_add/typical` | 5.1 | `wide32/w12_add/typical` | 10.4 |

#### chromium, plain build (136.0.7103.25)

| Predicate | Inputs | Shipped | Plain `i128` | Limbs | Fastest |
| --- | --- | ---: | ---: | ---: | --- |
| incircle | typical | **174** | 265 | 192 | shipped |
| incircle | extreme | **174** | 308 | 197 | shipped |
| insphere | typical | **437** | 677 | 692 | shipped |
| insphere | extreme | **464** | 766 | 709 | shipped |
| lpi_side | typical | **235** | 286 | 327 | shipped |
| lpi_side | extreme | **272** | 329 | 335 | shipped |
| lpi_weight | typical | **47.0** | 47.1 | 90.4 | shipped |
| lpi_weight | extreme | **57.8** | 57.9 | 90.9 | shipped |
| orient2d | typical | **16.6** | 33.1 | 55.6 | shipped |
| orient2d | extreme | **16.7** | 33.1 | 56.5 | shipped |
| orient3d | typical | **78.3** | 120 | 142 | shipped |
| orient3d | extreme | **83.1** | 125 | 143 | shipped |
| plane_side | typical | **47.0** | 47.4 | 104 | shipped |
| plane_side | extreme | 58.3 | **57.6** | 105 | plain |
| ssi_side | typical | **122** | 187 | 125 | shipped |
| ssi_side | extreme | **122** | 207 | 128 | shipped |
| ssi_weight | typical | **16.6** | 32.7 | 56.6 | shipped |
| ssi_weight | extreme | **16.7** | 32.7 | 57.2 | shipped |
| tpi_side | typical | 1749 | 1938 | **1045** | limbs |
| tpi_side | extreme | 1858 | 2232 | **1087** | limbs |
| tpi_weight | typical | 348 | 418 | **237** | limbs |
| tpi_weight | extreme | 387 | 516 | **245** | limbs |

| 64-bit limbs | ns | 32-bit limbs | ns |
| --- | ---: | --- | ---: |
| `wide/i256_mul/typical` | 55.6 | `wide32/w8_product/typical` | 39.6 |
| `wide/i384_mul/typical` | 78.5 | `wide32/w12_product/typical` | 70.6 |
| `wide/i256_add/typical` | 11.1 | `wide32/w8_add/typical` | 22.0 |
| `wide/i384_add/typical` | 14.8 | `wide32/w12_add/typical` | 29.4 |

#### chromium, wide build (136.0.7103.25)

The engine does not validate this build: unsupported.

#### firefox, plain build (137.0)

| Predicate | Inputs | Shipped | Plain `i128` | Limbs | Fastest |
| --- | --- | ---: | ---: | ---: | --- |
| incircle | typical | 1002 | 1510 | **859** | limbs |
| incircle | extreme | 1000 | 1770 | **934** | limbs |
| insphere | typical | **2630** | 3874 | 2974 | shipped |
| insphere | extreme | **2748** | 4389 | 3219 | shipped |
| lpi_side | typical | **1356** | 1615 | 1466 | shipped |
| lpi_side | extreme | **1542** | 1806 | 1547 | shipped |
| lpi_weight | typical | 254 | **253** | 447 | plain |
| lpi_weight | extreme | 296 | **296** | 461 | plain |
| orient2d | typical | **90.6** | 174 | 286 | shipped |
| orient2d | extreme | **90.4** | 175 | 298 | shipped |
| orient3d | typical | **439** | 687 | 692 | shipped |
| orient3d | extreme | **461** | 717 | 717 | shipped |
| plane_side | typical | 251 | **251** | 481 | plain |
| plane_side | extreme | 297 | **291** | 495 | plain |
| ssi_side | typical | 694 | 1050 | **617** | limbs |
| ssi_side | extreme | 691 | 1142 | **647** | limbs |
| ssi_weight | typical | **92.3** | 177 | 289 | shipped |
| ssi_weight | extreme | **92.0** | 178 | 300 | shipped |
| tpi_side | typical | 10102 | 10653 | **5703** | limbs |
| tpi_side | extreme | 10702 | 12336 | **6250** | limbs |
| tpi_weight | typical | 2003 | 2040 | **1235** | limbs |
| tpi_weight | extreme | 2192 | 2633 | **1320** | limbs |

| 64-bit limbs | ns | 32-bit limbs | ns |
| --- | ---: | --- | ---: |
| `wide/i256_mul/typical` | 305 | `wide32/w8_product/typical` | 203 |
| `wide/i384_mul/typical` | 496 | `wide32/w12_product/typical` | 367 |
| `wide/i256_add/typical` | 52.4 | `wide32/w8_add/typical` | 75.7 |
| `wide/i384_add/typical` | 68.0 | `wide32/w12_add/typical` | 104 |

#### firefox, wide build (137.0)

The engine does not validate this build: unsupported.

#### node, plain build (Node v24.21.0)

| Predicate | Inputs | Shipped | Plain `i128` | Limbs | Fastest |
| --- | --- | ---: | ---: | ---: | --- |
| incircle | typical | **159** | 259 | 185 | shipped |
| incircle | extreme | **159** | 307 | 190 | shipped |
| insphere | typical | **416** | 641 | 680 | shipped |
| insphere | extreme | **441** | 750 | 683 | shipped |
| lpi_side | typical | **228** | 317 | 320 | shipped |
| lpi_side | extreme | **252** | 311 | 373 | shipped |
| lpi_weight | typical | **41.7** | 42.1 | 86.1 | shipped |
| lpi_weight | extreme | **49.5** | 49.7 | 85.9 | shipped |
| orient2d | typical | **11.5** | 28.5 | 50.5 | shipped |
| orient2d | extreme | **11.7** | 28.3 | 50.5 | shipped |
| orient3d | typical | **70.9** | 112 | 137 | shipped |
| orient3d | extreme | **74.2** | 119 | 139 | shipped |
| plane_side | typical | **41.8** | 41.9 | 96.6 | shipped |
| plane_side | extreme | 50.4 | **49.1** | 98.5 | plain |
| ssi_side | typical | **112** | 178 | 120 | shipped |
| ssi_side | extreme | **119** | 192 | 122 | shipped |
| ssi_weight | typical | **12.1** | 28.2 | 52.0 | shipped |
| ssi_weight | extreme | **12.1** | 28.3 | 51.8 | shipped |
| tpi_side | typical | 1697 | 1850 | **1026** | limbs |
| tpi_side | extreme | 1780 | 2218 | **1067** | limbs |
| tpi_weight | typical | 328 | 398 | **232** | limbs |
| tpi_weight | extreme | 352 | 495 | **238** | limbs |

| 64-bit limbs | ns | 32-bit limbs | ns |
| --- | ---: | --- | ---: |
| `wide/i256_mul/typical` | 51.8 | `wide32/w8_product/typical` | 34.8 |
| `wide/i384_mul/typical` | 72.9 | `wide32/w12_product/typical` | 62.7 |
| `wide/i256_add/typical` | 6.3 | `wide32/w8_add/typical` | 16.9 |
| `wide/i384_add/typical` | 10.2 | `wide32/w12_add/typical` | 23.8 |

#### node, wide build

The engine does not validate this build: unsupported.

#### safari, plain build (26.6.2)

| Predicate | Inputs | Shipped | Plain `i128` | Limbs | Fastest |
| --- | --- | ---: | ---: | ---: | --- |
| incircle | typical | **99.4** | 174 | 101 | shipped |
| incircle | extreme | **99.6** | 201 | 108 | shipped |
| insphere | typical | **253** | 390 | 322 | shipped |
| insphere | extreme | **296** | 452 | 337 | shipped |
| lpi_side | typical | **130** | 177 | 156 | shipped |
| lpi_side | extreme | 168 | 200 | **161** | limbs |
| lpi_weight | typical | **26.3** | 27.4 | 44.1 | shipped |
| lpi_weight | extreme | **30.7** | 32.1 | 44.6 | shipped |
| orient2d | typical | **8.9** | 16.1 | 37.2 | shipped |
| orient2d | extreme | **9.1** | 17.6 | 34.8 | shipped |
| orient3d | typical | **38.8** | 58.6 | 78.0 | shipped |
| orient3d | extreme | **44.8** | 71.1 | 79.6 | shipped |
| plane_side | typical | 26.1 | **23.6** | 55.5 | plain |
| plane_side | extreme | **30.1** | 33.1 | 53.5 | shipped |
| ssi_side | typical | 71.5 | 115 | **68.6** | limbs |
| ssi_side | extreme | **71.1** | 129 | 73.1 | shipped |
| ssi_weight | typical | **9.3** | 16.7 | 33.2 | shipped |
| ssi_weight | extreme | **9.0** | 16.2 | 33.7 | shipped |
| tpi_side | typical | 1062 | 1230 | **633** | limbs |
| tpi_side | extreme | 1126 | 1433 | **634** | limbs |
| tpi_weight | typical | 198 | 253 | **123** | limbs |
| tpi_weight | extreme | 221 | 296 | **128** | limbs |

| 64-bit limbs | ns | 32-bit limbs | ns |
| --- | ---: | --- | ---: |
| `wide/i256_mul/typical` | 29.0 | `wide32/w8_product/typical` | 29.1 |
| `wide/i384_mul/typical` | 45.6 | `wide32/w12_product/typical` | 49.4 |
| `wide/i256_add/typical` | 5.3 | `wide32/w8_add/typical` | 9.8 |
| `wide/i384_add/typical` | 7.1 | `wide32/w12_add/typical` | 14.9 |

#### safari, wide build (26.6.2)

The engine does not validate this build: unsupported.

#### wasmtime, plain build (wasmtime 50.0.0 (ebe5e8df1 2026-10-20))

| Predicate | Inputs | Shipped | Plain `i128` | Limbs | Fastest |
| --- | --- | ---: | ---: | ---: | --- |
| incircle | typical | 275 | 462 | **235** | limbs |
| incircle | extreme | 273 | 517 | **237** | limbs |
| insphere | typical | **696** | 977 | 835 | shipped |
| insphere | extreme | **740** | 1204 | 849 | shipped |
| lpi_side | typical | **349** | 478 | 363 | shipped |
| lpi_side | extreme | 398 | 489 | **378** | limbs |
| lpi_weight | typical | **52.8** | 54.1 | 96.1 | shipped |
| lpi_weight | extreme | **62.5** | 63.1 | 97.9 | shipped |
| orient2d | typical | **18.8** | 37.6 | 51.2 | shipped |
| orient2d | extreme | **18.9** | 37.7 | 51.5 | shipped |
| orient3d | typical | **93.1** | 149 | 155 | shipped |
| orient3d | extreme | **104** | 165 | 159 | shipped |
| plane_side | typical | **53.8** | 53.8 | 106 | shipped |
| plane_side | extreme | 63.7 | **62.5** | 109 | plain |
| ssi_side | typical | 187 | 302 | **146** | limbs |
| ssi_side | extreme | 193 | 318 | **143** | limbs |
| ssi_weight | typical | **19.0** | 37.7 | 50.9 | shipped |
| ssi_weight | extreme | **19.1** | 37.5 | 52.2 | shipped |
| tpi_side | typical | 2835 | 3676 | **1305** | limbs |
| tpi_side | extreme | 3144 | 4234 | **1382** | limbs |
| tpi_weight | typical | 598 | 768 | **277** | limbs |
| tpi_weight | extreme | 661 | 921 | **291** | limbs |

| 64-bit limbs | ns | 32-bit limbs | ns |
| --- | ---: | --- | ---: |
| `wide/i256_mul/typical` | 74.0 | `wide32/w8_product/typical` | 40.0 |
| `wide/i384_mul/typical` | 114 | `wide32/w12_product/typical` | 75.9 |
| `wide/i256_add/typical` | 10.3 | `wide32/w8_add/typical` | 19.8 |
| `wide/i384_add/typical` | 15.2 | `wide32/w12_add/typical` | 37.6 |

#### wasmtime, wide build (wasmtime 50.0.0 (ebe5e8df1 2026-10-20))

| Predicate | Inputs | Shipped | Plain `i128` | Limbs | Fastest |
| --- | --- | ---: | ---: | ---: | --- |
| incircle | typical | **96.8** | 389 | 240 | shipped |
| incircle | extreme | **96.8** | 423 | 244 | shipped |
| insphere | typical | **349** | 993 | 887 | shipped |
| insphere | extreme | **386** | 1123 | 901 | shipped |
| lpi_side | typical | **299** | 382 | 362 | shipped |
| lpi_side | extreme | **364** | 431 | 377 | shipped |
| lpi_weight | typical | 53.6 | **52.5** | 93.9 | plain |
| lpi_weight | extreme | 63.6 | **62.0** | 95.9 | plain |
| orient2d | typical | **7.3** | 35.0 | 51.7 | shipped |
| orient2d | extreme | **7.3** | 35.1 | 51.9 | shipped |
| orient3d | typical | **56.0** | 153 | 157 | shipped |
| orient3d | extreme | **62.3** | 166 | 163 | shipped |
| plane_side | typical | 53.5 | **53.3** | 107 | plain |
| plane_side | extreme | 63.5 | **63.3** | 110 | plain |
| ssi_side | typical | **67.4** | 237 | 144 | shipped |
| ssi_side | extreme | **67.3** | 296 | 149 | shipped |
| ssi_weight | typical | **7.6** | 34.9 | 53.6 | shipped |
| ssi_weight | extreme | **7.3** | 35.0 | 51.8 | shipped |
| tpi_side | typical | 2088 | 2647 | **1303** | limbs |
| tpi_side | extreme | 2148 | 2857 | **1384** | limbs |
| tpi_weight | typical | 324 | 525 | **278** | limbs |
| tpi_weight | extreme | 363 | 639 | **290** | limbs |

| 64-bit limbs | ns | 32-bit limbs | ns |
| --- | ---: | --- | ---: |
| `wide/i256_mul/typical` | 28.8 | `wide32/w8_product/typical` | 40.6 |
| `wide/i384_mul/typical` | 46.4 | `wide32/w12_product/typical` | 75.4 |
| `wide/i256_add/typical` | 8.7 | `wide32/w8_add/typical` | 19.8 |
| `wide/i384_add/typical` | 10.8 | `wide32/w12_add/typical` | 37.4 |

#### webkit, plain build (18.4)

| Predicate | Inputs | Shipped | Plain `i128` | Limbs | Fastest |
| --- | --- | ---: | ---: | ---: | --- |
| incircle | typical | **89.7** | 162 | 107 | shipped |
| incircle | extreme | **80.0** | 179 | 115 | shipped |
| insphere | typical | **228** | 412 | 347 | shipped |
| insphere | extreme | **238** | 422 | 463 | shipped |
| lpi_side | typical | **128** | 192 | 175 | shipped |
| lpi_side | extreme | **146** | 200 | 179 | shipped |
| lpi_weight | typical | **24.4** | 25.1 | 56.7 | shipped |
| lpi_weight | extreme | **27.6** | 29.8 | 54.9 | shipped |
| orient2d | typical | **9.0** | 20.2 | 51.3 | shipped |
| orient2d | extreme | **8.0** | 18.8 | 51.5 | shipped |
| orient3d | typical | **38.5** | 63.3 | 88.4 | shipped |
| orient3d | extreme | **52.3** | 72.0 | 82.0 | shipped |
| plane_side | typical | 24.1 | **23.7** | 58.4 | plain |
| plane_side | extreme | **27.3** | 29.2 | 58.1 | shipped |
| ssi_side | typical | **68.1** | 147 | 71.2 | shipped |
| ssi_side | extreme | **75.7** | 134 | 82.2 | shipped |
| ssi_weight | typical | **8.6** | 19.6 | 36.9 | shipped |
| ssi_weight | extreme | **8.8** | 21.0 | 36.2 | shipped |
| tpi_side | typical | 910 | 1302 | **745** | limbs |
| tpi_side | extreme | 943 | 1392 | **770** | limbs |
| tpi_weight | typical | 196 | 224 | **158** | limbs |
| tpi_weight | extreme | 197 | 277 | **146** | limbs |

| 64-bit limbs | ns | 32-bit limbs | ns |
| --- | ---: | --- | ---: |
| `wide/i256_mul/typical` | 28.5 | `wide32/w8_product/typical` | 21.6 |
| `wide/i384_mul/typical` | 35.9 | `wide32/w12_product/typical` | 57.1 |
| `wide/i256_add/typical` | 5.3 | `wide32/w8_add/typical` | 10.2 |
| `wide/i384_add/typical` | 9.7 | `wide32/w12_add/typical` | 14.2 |

#### webkit, wide build (18.4)

The engine does not validate this build: unsupported.

#### x86_64-linux, native build (rustc 1.99.0 (b940084d7 2026-09-28), INTEL(R) XEON(R) PLATINUM 8573C)

| Predicate | Inputs | Shipped | Plain `i128` | Limbs | Fastest |
| --- | --- | ---: | ---: | ---: | --- |
| incircle | typical | **39.0** | 66.8 | 142 | shipped |
| incircle | extreme | **39.2** | 74.9 | 146 | shipped |
| insphere | typical | **89.9** | 172 | 585 | shipped |
| insphere | extreme | **89.9** | 184 | 568 | shipped |
| lpi_side | typical | **70.2** | 77.0 | 288 | shipped |
| lpi_side | extreme | **70.0** | 77.3 | 297 | shipped |
| lpi_weight | typical | **10.4** | 14.3 | 39.2 | shipped |
| lpi_weight | extreme | **10.4** | 14.3 | 40.5 | shipped |
| orient2d | typical | **2.4** | 7.7 | 12.8 | shipped |
| orient2d | extreme | **2.4** | 7.7 | 14.2 | shipped |
| orient3d | typical | **12.2** | 30.5 | 75.3 | shipped |
| orient3d | extreme | **12.3** | 30.3 | 77.3 | shipped |
| plane_side | typical | **11.9** | 11.9 | 50.4 | shipped |
| plane_side | extreme | **11.6** | 11.6 | 51.4 | shipped |
| ssi_side | typical | **29.6** | 49.0 | 87.5 | shipped |
| ssi_side | extreme | **29.9** | 51.9 | 90.6 | shipped |
| ssi_weight | typical | **2.2** | 8.0 | 13.0 | shipped |
| ssi_weight | extreme | **2.2** | 8.0 | 14.3 | shipped |
| tpi_side | typical | **691** | 714 | 849 | shipped |
| tpi_side | extreme | **700** | 763 | 875 | shipped |
| tpi_weight | typical | 140 | **140** | 208 | plain |
| tpi_weight | extreme | **140** | 156 | 217 | shipped |

| 64-bit limbs | ns | 32-bit limbs | ns |
| --- | ---: | --- | ---: |
| `wide/i256_mul/typical` | 14.6 | `wide32/w8_product/typical` | 20.4 |
| `wide/i384_mul/typical` | 20.1 | `wide32/w12_product/typical` | 31.5 |
| `wide/i256_add/typical` | 8.1 | `wide32/w8_add/typical` | 10.9 |
| `wide/i384_add/typical` | 9.1 | `wide32/w12_add/typical` | 13.0 |

#### Fastest variant on typical inputs

| Predicate | aarch64-linux (native) | chromium (plain) | firefox (plain) | node (plain) | safari (plain) | wasmtime (plain) | wasmtime (wide) | webkit (plain) | x86_64-linux (native) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| incircle | shipped | shipped | limbs | shipped | shipped | limbs | shipped | shipped | shipped |
| insphere | shipped | shipped | shipped | shipped | shipped | shipped | shipped | shipped | shipped |
| lpi_side | shipped | shipped | shipped | shipped | shipped | shipped | shipped | shipped | shipped |
| lpi_weight | shipped | shipped | plain | shipped | shipped | shipped | plain | shipped | shipped |
| orient2d | shipped | shipped | shipped | shipped | shipped | shipped | shipped | shipped | shipped |
| orient3d | shipped | shipped | shipped | shipped | shipped | shipped | shipped | shipped | shipped |
| plane_side | plain | shipped | plain | shipped | plain | shipped | plain | plain | shipped |
| ssi_side | shipped | shipped | limbs | shipped | limbs | limbs | shipped | shipped | shipped |
| ssi_weight | shipped | shipped | shipped | shipped | shipped | shipped | shipped | shipped | shipped |
| tpi_side | shipped | limbs | limbs | limbs | limbs | limbs | limbs | limbs | shipped |
| tpi_weight | shipped | limbs | limbs | limbs | limbs | limbs | limbs | limbs | plain |

