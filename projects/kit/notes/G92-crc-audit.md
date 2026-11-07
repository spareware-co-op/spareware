# G92: the written audit of `crc` 3.4.0 and `crc-catalog` 2.5.0

*Piece G92, Spare Kit geometry kernel. The audit chapter 29's rule 3 requires before a published crate ships inside a kernel crate. Written 2026-10-31 by Claude Opus 5.5 for the owners. `deny-no-std.toml`'s records for these two crates name this file.*

## The answer in plain words

`geom-format` computes every record's check, CRC-32C (24.2), with the `crc` crate and its catalogue's `CRC_32_ISCSI` entry. Read at the exact releases `Cargo.lock` pins, both crates meet the kernel's bar:

- **No `unsafe`.** Both crates open with `#![forbid(unsafe_code)]`, so the compiler refuses any `unsafe` block in them.
- **No panic.** The CRC-32 path reads its tables with indices bounded by its loop guards (`i < len`, `i + 16 <= len`) and masked to eight bits (`& 0xFF`), into tables of 256 entries.
- **No allocation, no global state.** The table is built by a `const fn`; `geom-format` holds the engine in a `const`, so it is computed at compile time.
- **The same bits everywhere.** Integer table arithmetic only, no processor-specific path.
- **Notices.** Both are MIT OR Apache-2.0. Their licence files must be carried by the generated third-party notices file (piece G481), as for every crate compiled into a shipped binary.

## 1. What was audited

| Crate | Licence | Source (the commit in the crate's `.cargo_vcs_info.json`) | Features in our build |
| --- | --- | --- | --- |
| `crc` 3.4.0 | MIT OR Apache-2.0 (`Cargo.toml`) | [mrhooray/crc-rs at `2c8fd961`](https://github.com/mrhooray/crc-rs/tree/2c8fd9615d620b5a5f8c9556b79a4ca173d6d401) | the default (none that adds code) |
| `crc-catalog` 2.5.0 | MIT OR Apache-2.0 (`Cargo.toml`) | [akhilles/crc-catalog at `ed4ad631`](https://github.com/akhilles/crc-catalog/tree/ed4ad631f22b05055c21a3a4127eb7cf6d75bb62) | none |

`crc` asks for `crc-catalog` `^2.4.0`; `Cargo.lock` holds 2.5.0. Neither has a build script or a procedural macro. `cargo tree -p geom-format -e normal --target all` lists exactly these two beside the crates already vetted (`libm`, `sha2` and its chain).

## 2. What the kernel calls

`geom-format`'s `checksum.rs` holds `const CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);` and calls `CASTAGNOLI.checksum(bytes)`. With the default implementation `Table<1>`, that is `crc32.rs`'s `Crc::new` (the table, through `table.rs`'s `crc32_table`), `checksum`, `init`, `update_table` and `finalize`.

`CRC_32_ISCSI` (`crc-catalog`'s `src/algorithm.rs`, line 2270) is width 32, polynomial `0x1edc6f41`, initial value `0xffffffff`, reflected input and output, final XOR `0xffffffff`, check value `0xe3069283`: the Castagnoli CRC of RFC 3720 and RFC 7143. `geom-format`'s tests check the check value and RFC 3720 appendix B.4's four examples.

## 3. Notices

Both crates are published under MIT OR Apache-2.0 with the licence files in the package. No file reached carries a separate notice. The generated third-party notices file (G481) carries their licences; nothing ships yet.

## 4. `unsafe`

None: `crc` `src/lib.rs` line 29 and `crc-catalog` `src/lib.rs` line 2 are `#![forbid(unsafe_code)]`, which no inner attribute can lift.

## 5. Panics

In `update_table` (`crc32.rs`), the 16-byte loop runs while `i + 16 <= len` and reads `bytes[i]` to `bytes[i + 15]`; the byte loop runs while `i < len` and reads `bytes[i]`. Every table index is a byte or a value masked with `& 0xFF`, into `[u32; 256]` tables. The shifts are by `32 - width` with width 32, so by 0. No arithmetic in the path can overflow: `i` is below `len`, and the CRC is shifted and XORed only. So no input reaches a panic, in debug or release.

## 6. Allocation and global state

None. The table is a `[[u32; 256]; 1]` value built by a `const fn`; held in a `const`, it is built at compile time. No `static`, no thread-local.

## 7. Determinism

Table lookups, shifts and XORs on `u32`: the same bits on every target. No processor-specific path exists in 3.4.0.

## 8. `no_std`

`crc` `src/lib.rs` line 28 and `crc-catalog` `src/lib.rs` line 1 are `#![no_std]`, and neither uses `alloc`.

## 9. Pinning

`geom-format` asks for `crc` `^3.4.0`; `Cargo.lock` and the exact allow-list entries (`crc@3.4.0`, `crc-catalog@2.5.0`) hold the versions, so a lock that moves fails the gate until a new record is written.

## 10. Open

- Vendoring: as G24's section 9 says for `libm`.
- The published crate was read; it was not diffed against the repository at the recorded commit.
