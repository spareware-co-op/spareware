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

The fourth comparison chapter 23.3 names, Avnaim, Boissonnat, Devillers, Preparata and Yvinec's sign of small determinants in b-bit arithmetic, is not measured yet; see "Open".

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
