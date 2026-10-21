# 21. Determinism

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) section 7 and [03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md) section 5.

**What Rust already guarantees.** Addition, subtraction, multiplication, division, remainder, square root, fused multiply-add (only when written as `mul_add`), casts and comparisons follow IEEE 754 exactly on every supported target, and Rust never fuses a multiply and an add on its own (RFC 3514).

**What the kernel must police, and how:**

| Hazard | Rule | Enforced by |
| --- | --- | --- |
| Maths functions (`sin`, `cos`, `atan2`, `exp`, `powi` and so on) differ by platform | Only from the kernel's own deterministic maths module: the pure-Rust `libm` crate now, moving to correctly rounded CORE-MATH ports, which are identical by specification | Kernel crates are `no_std` (the standard ones are not even available); `clippy` disallowed-methods |
| NaN sign and payload are not deterministic, even in WebAssembly | The kernel never produces NaN. Scalars are checked at construction | A NaN-free scalar type; fuzzing; tripwires in debug builds |
| `max` and `min` may return either zero sign | Use the kernel's own total-order helpers | `clippy` disallowed-methods |
| Relaxed and "algebraic" operations, relaxed SIMD | Forbidden | `clippy` disallowed-methods; no relaxed SIMD feature |
| Parallel float reductions change order | Fixed chunk sizes independent of thread count, results collected in order, no float reduction across chunks | One shared parallel helper; a thread-count test (1, 2, 3 and 8 threads) |
| Hash map iteration order is random | No iteration over hash maps | `clippy` disallowed-types for `HashMap` and `HashSet` in kernel crates |
| Tie-breaks by memory address | Never. Ties break by persistent name | Code review rule; tests that shuffle allocation |
| Targets that break IEEE (i586 x87, 32-bit ARM NEON flushing denormals) | Excluded from the guarantee | Documented platform list |

**Checked on every pull request:** an operation-log replay corpus runs on x86-64 Linux, Windows and macOS, aarch64 Linux and macOS, and wasm32 under wasmtime and Node, in debug and release, with `-C target-cpu=native`, and with 1, 2, 3 and 8 threads. The SHA-256 of each canonical output must match the golden value. This is the method Rapier and Box2D use to keep their physics deterministic ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) section 7).
