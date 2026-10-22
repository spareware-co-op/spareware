# A06: Project file: SQLite container or a custom chunk file

## The question

On the desktop, one portable project file holds the stored model, the operation log and the index ([ARCHITECTURE.md](../ARCHITECTURE.md), Storage and scale). Should that file be a SQLite database with our own published schema, or a container format we write ourselves? The piece asks for a recommendation covering crash safety, size and streaming. Spare Studio's project file follows the same decision ([STU-A2-F5-P1](../../spare-studio/FEATURES.md)).

Outside sources were read on 20 October 2026, at the commits linked. "Unverified" marks anything not confirmed in source code or on an official page.

On 22 October 2026 the note was revised after review, to fit the Spare Kit geometry kernel. The kernel is mapped in G01 ([issue #169](https://github.com/spareware-co-op/spareware/issues/169), [PR #176](https://github.com/spareware-co-op/spareware/pull/176)), which is not merged yet. Numbers such as 24.5 are that map's chapters, linked at the paths they will have once G01 merges. Until then, read them on PR #176, or at [the commit this note was checked against](https://github.com/spareware-co-op/spareware/tree/b6ca7e2513f5069ae8650bb23b24d97d6eb1c24b/projects/engines/geometry/architecture).

## Words used here

"Chunk" means two different things in our maps, so this note avoids it on its own:

- **Record:** one SKG record, the kernel's unit of storage ([24.2][k24.2]): tag, flags, schema, the content IDs it depends on, a payload in deterministic CBOR, and a CRC-32C. Its **content ID** is `[alg, digest]`, where algorithm 1 is SHA-256 over the uncompressed payload.
- **Kernel chunk:** a record holding a fixed run of 4,096 rows of one table of a body ([24.2][k24.2]).
- **Cell:** Spare Construct's spatial group of elements, by building, level and spatial cell (Storage and scale, which calls it a chunk). In the container a cell is an index row that lists its members' content IDs, not a blob (recommendation 3).

## What the file has to do

From Spare Construct's map:

- **Save in under 100 ms, never blocking.** Operations append to the log at once; compaction runs in the background (Storage and scale; Performance targets).
- **Open a 230,000-element building (1.33 GB as .rvt) to first view in under 3 s**, loading only what is in view. A city never has to fit in RAM.
- **One core, native and WebAssembly.** The browser core runs in a worker and works offline on small models (Runtime shape).
- **A version is a pointer into the operation log plus a compacted snapshot** (Versions and revisions).
- **Open format:** documented and readable without the app (ground rule 7). [EXTENDING.md](../../../EXTENDING.md) section 3: "Prefer an existing standard to a new format. A new format needs a decision note that says why none will do."
- **From scratch where it decides speed or ownership**, and the file format is on that list (ground rule 1).

From the kernel's map (G01):

- **The encoding is SKG, and SQLite is only a container.** G01 weighed SQLite as an encoding and turned it down because its "page layout depends on history"; it is "a container, not an encoding" ([24.1][k24.1]). So this note chooses the container that holds SKG records, nothing more.
- **Containers** ([24.5][k24.5]): a blob holds whole records with their flags and CRCs, keyed by content ID; header and frames are one blob per document; damage stays local, so "a bad chunk never stops the file"; salvage writes intact records out as a standalone `.skg`; container to `.skg` and back gives the same records, byte for byte.
- **Compression sits outside the canonical form** ([24.2][k24.2]): content IDs and golden hashes are always over uncompressed bytes.
- **Open without replaying anything** ([26.2][k26.2], [24.13][k24.13]). A reader without the plugin that made a shape still opens its stored geometry, and refuses only to re-run the recipe, with `RecipeOwnerMissing` ([24.4][k24.4]).
- **Saving never prunes.** History goes only through a recorded `PruneHistory` operation ([24.4][k24.4]).
- **Trust is stated by the program, and checks on unknown files run lazily**, chunk by chunk, within budget ([24.7][k24.7], [23.11][k23.11]). The kernel's share of the 3 s first view is 1 s ([23.1][k23.1]).
- **Determinism:** nothing that feeds the model may depend on an arbitrary iteration order ([21][k21]).
- **Reading forever** ([24.6][k24.6]): we may stop writing a version; we never stop reading it.

## Options

| | A. SQLite container | B. Custom container, from scratch | C. Custom container on a Rust embedded store |
| --- | --- | --- | --- |
| What it is | One SQLite database: SKG records in a table keyed by content ID, plus the operation log, cell index and versions | Our own byte layout: framed records appended to a file, with an index we maintain | Our own tables on [redb](https://github.com/cberner/redb/blob/dfa0739c9fb31c31e43a4d93e3fdae6976af3d02/README.md), a pure-Rust B-tree store |
| Crash safety | Built in: atomic commits, checksummed write-ahead log, 590 times as much test code as library code | Ours to build and prove | Built in: copy-on-write pages, checksums, two commit slots |
| Space after history is pruned | Freed pages reused, or returned in place by `incremental_vacuum` | Rewrite every live record into a new file, unless we also build a page allocator | Freed pages reused (buddy allocator) |
| Read one record | B-tree lookup by content ID | Seek to an offset from our index | B-tree lookup by key |
| Browser | rusqlite uses sqlite-wasm-rs with an OPFS backend | We write an OPFS backend | We write a `StorageBackend` for OPFS (*unverified* that anyone has) |
| Readable with other tools | Yes: any SQLite tool or language lists the tables and reads the records | Only with our reader | Only through redb |
| Licence | Public domain | Ours | MIT or Apache-2.0 |

A fourth family, formats that rewrite the whole file on every save (a zip of files, or Blender's write-then-rename), is ruled out by measurement: rewriting 192 MB took 549 ms, more than five times the save budget.

## Evidence

### Crash safety

**SQLite.** "Writes to an SQLite database are atomic. They either happen completely or not at all, even during system crashes or power failures" ([application file format](https://www.sqlite.org/appfileformat.html)). In write-ahead log (WAL) mode, every frame of the log carries two salts and a checksum. Recovery accepts a frame only if its salts match the log header and its checksum is valid, and the salts change after each checkpoint so stale frames can never be replayed (header comment of [src/wal.c](https://github.com/sqlite/sqlite/blob/dd986a5c51cd38167a99c5a5eec90ade6e84c3c0/src/wal.c)). Testing: the library is about 155.8 KSLOC with 590 times as much test code, 100% branch and MC/DC coverage of the core, and crash tests in which a special file system "randomly reorders and corrupts the unsynchronized write operations" before checking that each transaction either fully happened or fully rolled back ([how SQLite is tested](https://www.sqlite.org/testing.html)).

SQLite lists the ways a database still gets corrupted ([how to corrupt](https://www.sqlite.org/howtocorrupt.html)). The ones that matter to us:

- Network file systems with broken locking, and drives that "lie about syncing" (USB sticks especially).
- Copying the file while a transaction is open, or separating it from its `-wal` file. A safe copy uses the backup API or `VACUUM INTO`.
- `PRAGMA synchronous=OFF`.

WAL needs shared memory, so "WAL does not work over a network filesystem" ([WAL](https://www.sqlite.org/wal.html)). The exception: in `locking_mode=EXCLUSIVE`, set before the first access, "SQLite never attempts to call any of the shared-memory methods" (same page; the source turns the lock routines into no-ops in that mode). For network shares in general, SQLite advises a client/server database and treats exclusive one-at-a-time access as a mitigation, not a guarantee ([SQLite over a network](https://www.sqlite.org/useovernet.html)). Many firms keep project files on office file servers (*unverified as a general claim*), so the app has to handle this case on purpose.

**A store we write ourselves** has to solve the same problem. redb shows what that takes ([design.md](https://github.com/cberner/redb/blob/6b2d24eb9a027a16a46fc8384a24c43e087feb5a/docs/design.md)). Every page is copy-on-write. Two commit slots are chosen by one "god byte", and flipping that byte is the commit. Pages carry XXH3-128 checksums in a Merkle tree, and the default commit (1PC+C) uses one fsync. If a commit is torn, "the checksum verification will fail, and it will be rolled back." It also depends on assumptions: single-byte writes are atomic, and writes never touch bytes outside their range. And because XXH3 is not collision-resistant, files from someone you don't trust need the slower two-fsync commit. Getting a design like this right and proven is years of work. SQLite and redb have already spent those years.

**Whole-file save.** Blender writes the new file to `<name>@`, renames it over the original, and shifts older saves to `.blend1`, `.blend2` and so on (`BLO_write_file_impl` and `do_history` in [writefile.cc](https://github.com/blender/blender/blob/d374e6dfb009f87838b21414fb561f2512990943/source/blender/blenloader/intern/writefile.cc). It is GPL-2.0-or-later, so this is the method only, and no code is used). It is safe, but each save rewrites everything, which fails the save target for large models (measured below).

### Damage, recovery and trust

- **SQLite finds structural damage when it reads a page** (a malformed page, a broken tree link) and returns `SQLITE_CORRUPT` for that query, so nothing has to scan the file up front. It does not checksum page contents unless the optional [checksum shim](https://www.sqlite.org/cksumvfs.html) is compiled in, so a flipped bit inside a record is caught only by the record's own CRC and content ID.
- **The recovery extension** (SQLite 3.40.0, [16 November 2022](https://www.sqlite.org/releaselog/3_40_0.html)) salvages what it can from a damaged database into a new one ([recovery](https://www.sqlite.org/recovery.html); source in [ext/recover](https://github.com/sqlite/sqlite/blob/57e46429fe7385f868e8684b91166803143aa0c4/ext/recover/sqlite3recover.c)). SQLite is frank that "the results will always be suspect": content can be lost, altered, reappear after deletion, or move between tables. It is built from extra files (`sqlite3recover.c`, `dbdata.c`) with `SQLITE_ENABLE_DBPAGE_VTAB`, and neither rusqlite's bundled SQLite nor sqlite-wasm-rs includes it (searched both repositories on 22 October 2026).
- **Content IDs make recovered records self-checking.** A record that survives recovery is used only if its CRC-32C and its content ID both verify ([24.2][k24.2]), so altered content is caught record by record. What recovery can't make trustworthy is the index tables (which records a cell or a version lists). So everything that defines the model must live in content-addressed records, and every index table must be something we can rebuild from them (recommendation 3).
- **A full-file check on open doesn't fit.** `PRAGMA quick_check` is O(N) in the number of rows and reads the whole file ([pragmas](https://www.sqlite.org/pragma.html)); on a multi-gigabyte file that alone could take the kernel's whole 1 s open share ([23.1][k23.1]). G01 has the program state the trust of what it opens, and runs checks on unknown files lazily ([24.7][k24.7]).

### Size

The first measurement replaced chunks in place with `UPDATE`, which doesn't model a content-addressed store, so it was re-run (method under Local measurement). This time records are never updated: each edit inserts new records, named versions keep reaching the old ones, and only `PruneHistory` plus collection removes anything. Record sizes follow the kernel's mix ([24.2][k24.2]): 6,000 small records of 2 to 8 KB, 700 table chunks of 32 to 256 KB, 20 display meshes of 0.5 to 4 MB and two 16 MB terrain meshes, 207.6 MB per version. Each of 10 versions replaces 5% of the records.

| After | File | Bytes still reachable | Overhead | Custom append file |
| --- | --- | --- | --- | --- |
| Version 0 | 216.7 MB | 207.6 MB | 4.4% | 207.6 MB |
| Version 5 | 285.2 MB | 273.3 MB | 4.4% | 273.3 MB |
| Version 10 | 347.6 MB | 333.2 MB | 4.3% | 333.2 MB |
| `PruneHistory` keeping versions 0 and 10, collection, `incremental_vacuum` | 333.3 MB | 318.6 MB | 4.6% | 318.6 MB, after rewriting every live record (0.78 s) |

- **With history kept, the file grows with the history.** That is the point of keeping it. SQLite adds a steady 4.3 to 4.6% over the reachable bytes, at 16 KB pages, for this mix.
- **Collection is cheap, and space comes back in place.** Collecting 674 unreachable records took 0.12 s, and `incremental_vacuum` returned the 14.3 MB they held in 0.09 s. The custom file reclaims space only by rewriting every live record (0.78 s here, and it grows with the file), unless we build a page allocator.
- **Page size.** In the first run, 64 KB pages wasted 14% with 64 KB blobs, because a blob plus its row header spills onto an overflow page. 16 KB pages did well in both runs. SQLite's own guidance agrees: "a database page size of 8192 or 16384 gives the best performance for large BLOB I/O" ([internal versus external BLOBs](https://www.sqlite.org/intern-v-extern-blob.html), measured on a 2011 machine).
- **Large records are slow to write and read in one piece.** Inserting one 16 MB terrain mesh took 57 to 72 ms; reading it took 18 ms, plus 12 ms to check its SHA-256. SKG allows payloads up to 64 MiB ([24.2][k24.2]), and SQLite allows blobs up to 1,000,000,000 bytes by default (hard limit 2,147,483,645; [limits](https://www.sqlite.org/limits.html)). Neither limit is the problem; the cost of moving one big blob is.
- **Record table layout.** A rowid table with a unique index on the content ID, not a `WITHOUT ROWID` table keyed by it. `WITHOUT ROWID` rows should be under about 1/20th of a page, and incremental blob I/O "does not work for WITHOUT ROWID tables" ([WITHOUT ROWID](https://www.sqlite.org/withoutrowid.html)). Speckle's local cache uses `objects(hash TEXT PRIMARY KEY, content TEXT) WITHOUT ROWID` with `INSERT OR IGNORE` ([sqlite.py](https://github.com/specklesystems/specklepy/blob/b8a575c9f25f5bddb9ce09ff8366d0feb18020b3/src/specklepy/transports/sqlite.py), Apache-2.0). That suits its small JSON objects, but not our larger records.

### Streaming

- **Inside the desktop core**, streaming means paging records from disk by content ID. Reading one record took 0.02 ms (2 to 8 KB), 0.13 ms (table chunks) and about 1.0 to 1.3 ms per MB for anything larger, plus about 0.7 ms per MB to verify its SHA-256 (warm cache). As an estimate, 200 MB in view would take about 0.25 s to read and 0.14 s to verify on one core. Both fit inside the kernel's 1 s open share, which also has to decode bodies and run the cheap check ([23.1][k23.1]).
- **Between machines**, the map already sends content-addressed data, not files (Speckle's method, Sharing with consultants), and G01 keys records by content ID. If the container's record table uses the same content IDs, a sync is "send the records the other side doesn't have". The file itself never has to stream over HTTP.
- **If we ever want to stream the file itself**, both approaches have been proven:
  - SQLite: [Cloud Backed SQLite](https://sqlite.org/cloudsqlite/doc/trunk/www/index.wiki) splits a database into fixed-size, never-modified blocks in Azure or Google Cloud Storage, with a local block cache. Bentley's iModel engine ([imodel-native](https://github.com/iTwin/imodel-native/blob/648971909bd1f302a2febd97620a272ba1365bc5/README.md), Apache-2.0) uses it: [CloudSqlite.cpp](https://github.com/iTwin/imodel-native/blob/e10942d6b5ea9a3feacfd0fe89f710203e5c9db5/iModelCore/BeSQLite/CloudSqlite.cpp) calls `sqlite3_bcvfs_attach`, `sqlite3_bcvfs_upload`, `sqlite3_bcvfs_poll` and `sqlite3_bcvfs_prefetch_new`. [sql.js-httpvfs](https://github.com/phiresky/sql.js-httpvfs/blob/b43c1a49af5ee67aaeed02aac1b387d9c60ae934/README.md) (Apache-2.0) reads a SQLite file over HTTP range requests, read-only.
  - Custom: [PMTiles](https://github.com/protomaps/PMTiles/blob/8b8ddea4dbff1b0104cf2bebf2f7ff35c91b41d5/spec/v3/spec.md) (spec public domain or CC0, code BSD-3-Clause, per its [README](https://github.com/protomaps/PMTiles/blob/a3ad743d8c5f90b1486602986f903c581f65b43e/README.md)) is a single file with a 127-byte header and a root directory inside the first 16 KiB, made for static hosting. Its spec says nothing about updating a file in place, so it fits write-once archives, not a file being edited.
- **Precedents in our own field.** iModels are SQLite files ("SQLite · .bim iModel files" in the imodel-native README), and their changesets come from SQLite's session extension ([ChangeSet.cpp](https://github.com/iTwin/imodel-native/blob/176d8acb03429ea77b9ca84ac651682cad0a6314/iModelCore/BeSQLite/ChangeSet.cpp) calls `sqlite3session_create` and `sqlite3changeset_apply_v3_strm`). [GeoPackage](https://www.geopackage.org/spec/) (OGC 12-128r19) is "a platform-independent SQLite database file" for map tiles and features, with R-tree spatial indexes kept up to date by triggers.

### Determinism

- **Row order is undefined unless a query says `ORDER BY`.** "SQLite makes no guarantees about the order of results if a SELECT omits the ORDER BY clause", and it adds that the order stays the same from run to run, "and so many applications mistakenly come to depend on" it ([pragmas](https://www.sqlite.org/pragma.html), `reverse_unordered_selects`). A plain `SELECT`, and the candidates an R*Tree query returns, come back in whatever order the plan chooses, which can change with the file's history or SQLite's version. Section [21][k21] forbids anything that feeds the model from depending on such an order.
- **R*Tree bounds are coarse.** SQLite's R*Tree stores 32-bit floats, rounding lower bounds down and upper bounds up, so boxes "might be slightly larger than specified, but will never be any smaller"; `rtree_i32` stores 32-bit integers ([R*Tree](https://www.sqlite.org/rtree.html)). The kernel's boxes are integer grid steps up to ±(2^40 − 1) ([24.7][k24.7]), which neither holds exactly. An R*Tree can only be a conservative filter in front of the exact test.

### Browser

- rusqlite 0.40.1 uses [sqlite-wasm-rs](https://github.com/Spxg/sqlite-wasm-rs/blob/c484247d594c04e6e97a91d00c30268afe333742/README.md) (MIT) on `wasm32-unknown-unknown` by default (`ffi-sqlite-wasm-rs` in its [Cargo.toml](https://github.com/rusqlite/rusqlite/blob/92a7d4965b12d0bab61d0c0447d91931f1d48461/Cargo.toml)). sqlite-wasm-rs offers an in-memory store and an OPFS "sahpool" store, and builds SQLite with `-DSQLITE_THREADSAFE=0`. That fits a core that owns its storage from one worker. On native targets rusqlite bundles SQLite 3.54.0 ([sqlite3.h](https://github.com/rusqlite/rusqlite/blob/1d5d0ecae8b8372c2779caf5ab98847155dba153/libsqlite3-sys/sqlite3/sqlite3.h)).
- SQLite's own notes on the OPFS sahpool store say it needs no COOP or COEP headers, has worked in all major browsers since March 2023, and is "easily the highest OPFS performance" of the options, at the cost of one connection per database ([persistence](https://sqlite.org/wasm/doc/trunk/persistence.md)).
- With a custom file or redb, we write the OPFS layer ourselves. redb's `StorageBackend` (read, write, set_len, sync_data, optional range locks in [backends.rs](https://github.com/cberner/redb/blob/9062a783935b64a4118d02c8c85cd3256b6c9648/src/tree_store/page_store/backends.rs)) makes that possible.
- [Turso](https://github.com/tursodatabase/turso/blob/1300161c058f34a2f7dbbe04984b041182175975/README.md) (MIT) is a Rust database compatible with SQLite at the "file format, and C API levels", runs in browsers, and has "not yet reached 1.0". Choosing SQLite's file format leaves room for a pure-Rust engine later without changing anyone's files.

### Openness, and ground rule 1

- The SQLite file format has been stable since 2004, the US Library of Congress lists SQLite as a [recommended storage format](https://www.sqlite.org/locrsf.html) for datasets, and SQLite promises to keep "the SQLite C-language API and on-disk format fully backwards compatible" and plans support "through the year 2050" ([long-term support](https://www.sqlite.org/lts.html)). The code is [public domain](https://www.sqlite.org/copyright.html), so it meets ground rule 3.
- Anyone can open the file with Python's standard library, list the tables and pull out every record. Each record is SKG, which is deterministic CBOR with a published specification and a second reader ([24.6][k24.6]), so the contents are readable without our code too.
- The header has a 32-bit `application_id` at offset 68, so tools can tell a Spare Construct file from "SQLite3 Database", and a `user_version` at offset 60 that the app owns ([pragmas](https://www.sqlite.org/pragma.html)).
- **Ground rule 1** asks for the file format from scratch. The format that matters is ours, from scratch: SKG records ([24.1][k24.1]), our table layout, the operation encoding and compaction. SQLite supplies only the page store and transactions, in the same way zstd supplies compression. And EXTENDING.md asks for an existing standard unless none will do, which this note finds is not the case.

### Local measurement

These are quick checks, not benchmark-harness results. They used Python 3.10.6's `sqlite3` module (SQLite 3.37.2, from 2021; rusqlite bundles 3.54.0) on Windows 11 with an Intel i7-11370H, 40 GB of RAM and an NVMe SSD (NTFS), with the operating system's cache warm. Python overhead is in every number for both options. The custom file was the simplest honest version: framed records with an in-memory index. The scripts are in [PR #161](https://github.com/spareware-co-op/spareware/pull/161).

| Save: one transaction of 50 operations × 200 bytes | Median | p99 | Worst |
| --- | --- | --- | --- |
| SQLite WAL, EXCLUSIVE, `synchronous=NORMAL`, automatic checkpoints (3,000 saves) | 0.10 ms | 0.67 ms | 75.4 ms |
| SQLite WAL, EXCLUSIVE, `synchronous=FULL`, automatic checkpoints (3,000 saves) | 0.63 ms | 2.94 ms | 25.9 ms |
| SQLite WAL, EXCLUSIVE, `synchronous=FULL`, checkpoints run outside the save (3,000 saves) | 0.62 ms | 1.71 ms | 5.5 ms |
| Custom append file, fsync on every save (400 saves) | 0.57 ms | 1.81 ms | 2.2 ms |
| Whole-file rewrite, temp file then rename: 64 MB / 192 MB | 167 ms / 549 ms | | |

- Most of the slowest SQLite saves came at regular intervals (save numbers 340, 682, 1023, 1705 and 2046 among the six slowest), about every 1,000 log pages. That matches SQLite's automatic checkpoint ([WAL](https://www.sqlite.org/wal.html)). With automatic checkpoints off and a checkpoint every 250 saves outside the save path, the worst save fell to 5.5 ms. The checkpoint itself took 7 to 41 ms. It is real work, but it can wait for idle time.
- In the content-addressed run, inserting one record in its own transaction (`synchronous=FULL`) took a median 0.62 ms for small records, 1.0 ms for table chunks, 8.6 ms for 0.5 to 4 MB meshes and 57 ms for 16 MB meshes. One small-record insert took 84 ms; its cause was not isolated. These are compaction steps in the background, never saves.

## Recommendation

### The container

1. **Use a SQLite container (option A) for SKG records**, with our own published, versioned table layout. This confirms the map's lean and fits G01's [24.5][k24.5]. It meets the save target with room to spare, gives space back after history is pruned without our own allocator, works in the browser through an existing crate, and is an existing standard any tool can open.
2. **Settings to confirm in the Design stage:**
   - `page_size=16384`, WAL, and `locking_mode=EXCLUSIVE` on the desktop. One process owns the file and no `-shm` file is created.
   - `synchronous=FULL`, so a save is on disk when it returns (p99 under 3 ms measured).
   - `wal_autocheckpoint=0`, with the storage thread checkpointing when idle and `journal_size_limit` capping the leftover log.
   - `auto_vacuum=INCREMENTAL`, with `incremental_vacuum` run when idle after a collection (rule 6).
   - Our own `application_id`, and `user_version` set to the layout version.

### The layout

3. **Layout sketch, for the file format draft:**
   - `record`: a rowid table of SKG records, one whole record per blob with its flags and CRC ([24.5][k24.5]), with columns `alg`, `digest`, `codec` and `data`, and a unique index on (`alg`, `digest`). The digest is the record's content ID: **SHA-256 over the uncompressed canonical payload, never over the stored bytes**. Compression is the `codec` column (none, or zstd), applied after hashing, so a new zstd version or level never changes a content ID ([24.2][k24.2], [21][k21]). The map's "compressed (zstd) and content-hashed" means exactly this. Rows are never updated: a changed body is new records.
   - `oplog`: sequence, operation ID, author, time and operation bytes.
   - `version`: name, log position and the content ID of a root record. The head is one more row. A root record lists the content IDs that make up the stored model at that log position, so a version is "a pointer into the operation log plus a compacted snapshot", as the map says. Header and frames are one record per document, and every body index names its frame by content ID ([24.5][k24.5]).
   - `cell` and `cell_member`: one row per cell (building, level, cell, detail level), and its members' content IDs in a stated order. **A cell is an index row, never one blob.** Packing a cell's bodies into one zstd blob would lose per-body content addressing, storing a shared body once ([23.9][k23.9]), and cache hits by content ([23.8][k23.8]).
   - `cell_bounds`: an R*Tree over cells, as a coarse filter only (rule 5).
   - **Every index table can be rebuilt from the records.** Only `record` and `oplog` hold the model; the rest makes it fast. Recovery depends on this (rule 9).
   - The draft must give tables or index entries for every record family in [24.4][k24.4], which this sketch doesn't list one by one: names and history, recipes in their envelope, procedural caches, display and level-of-detail records with deviation, class and edition, attributes, import provenance, and header plus frames. Spare Construct's own element data (components, parameters, relationships) should be stored the same way, as immutable records keyed by content ID; whether it uses the kernel's record registry or its own is for the draft to settle. The draft also states the two promises of [24.5][k24.5] as tests: container to standalone `.skg` and back gives the same records byte for byte, and salvage writes every intact record to `.skg`.
   - **Split large meshes.** Display meshes and levels of detail are stored per tile and per level, so a record stays at a few MB at most. A 16 MB record took 57 to 72 ms to write and about 30 ms to read and verify, and a view rarely needs all of it. The limit belongs in the draft, well under SKG's 64 MiB.

### Rules the container keeps

4. **Every query whose rows feed the model has `ORDER BY` a stable key.** That means the sequence number for the log, the stated member order for a cell, and the content ID or permanent ID otherwise. Never storage order or plan order ([21][k21]). The store's tests also run with `PRAGMA reverse_unordered_selects=ON`, which SQLite provides to expose exactly this mistake. It can't reverse every plan, so it catches most missing `ORDER BY` clauses, not all, and review still checks each query.
5. **R*Tree results are candidates, never answers.** They are sorted by a stable key, then tested exactly against the integer boxes stored in the records. The R*Tree's float bounds are never the authoritative bounds.
6. **What may be deleted.**
   - Saving never prunes ([24.4][k24.4]). Compaction only adds records and moves the head.
   - A record is collected only when nothing reaches it: not the head, not any named version, not any operation still in the log, and not the set of names the program holds outside the kernel. That set is `PruneHistory`'s input, and the operation itself is recorded in the log, so every replica collects alike.
   - Collection deletes unreachable rows; `incremental_vacuum` then only hands their freed pages back. Neither decides what is unreachable.
7. **The log tail is for crash recovery only.**
   - Saving appends operations at once; background compaction folds them into stored records and moves the head ([23.1][k23.1]: at most 10 ms of kernel time for a local edit).
   - A clean close, a write-back to a share, an export and any copy compact first, so the head covers the whole log. The file then opens from stored geometry without replaying anything ([26.2][k26.2], [24.13][k24.13]), even in a reader that lacks the plugin or operation version that made those operations ([24.4][k24.4]).
   - After a crash, the tail is replayed. If a recipe's owner is missing, the file opens read-only at the last head, with the tail kept and listed (*proposal*).
8. **Close cleanly.** On close, and before any copy or export, checkpoint with `TRUNCATE`, so the project file is complete on its own and no `-wal` file is left beside it. A file copied mid-checkpoint can be corrupt ([how to corrupt](https://www.sqlite.org/howtocorrupt.html)).

### Opening, damage and trust

9. **Trust is the program's to state, and nothing scans the whole file on open** ([24.7][k24.7]).
   - The program's own store is *known*: the range checks and the kernel's cheap check run, as [23.1][k23.1] assumes.
   - Anything else is *unknown*: the kernel's full checks run lazily in the background, chunk by chunk, within budget. No `quick_check` runs on the open path.
   - **Damage stays local** ([23.11][k23.11], [24.5][k24.5]). A record whose CRC or content ID fails opens its body read-only, with a structured error; the rest of the file opens normally.
   - If SQLite itself reports damage (`SQLITE_CORRUPT` on a page it reads), the file reopens read-only through the recovery extension (`sqlite3_recover_init` and `sqlite3_recover_run`) into a fresh database. Every recovered record must then pass its CRC and content ID, the index tables are rebuilt from the records that pass, and salvage writes those records to a standalone `.skg` ([24.5][k24.5]).
   - Cheap hardening still runs on every connection ([security](https://www.sqlite.org/security.html)): trusted schema, triggers and views off.

### Sharing and copies

10. **Can we work from a network share? Yes, through a local working copy and a lease.**
    - When a project file sits on a network path, the app takes a lease (a lock file beside it on the share), works on a copy on local disk, and writes back a compacted copy (rule 7) with SQLite's backup API or `VACUUM INTO`.
    - Without the lease, two people's write-backs would silently overwrite each other; with it, the second person opens read-only and is told who holds the file.
    - Teams that need to edit together use the server operation log, as planned.
11. **Live links between programs never read an open project file.** EXCLUSIVE locking shuts out every other process. Models stay linked through packages or the server, by the rules in [20.4][k20.4] and [25][k25].

### Where it lives

12. **The container code belongs in Spare Kit**, in `kit-store`, because Spare Studio shares this decision and the root layering rule moves shared needs down. It never sits in a `geom-*` crate: those are `no_std`, so they open no files, and they forbid unsafe code ([27](../../engines/geometry/architecture/27-crates-folders.md)); [24.5][k24.5] leaves the packing to the program. Rowid tables let incremental blob I/O serve the kernel's `RangeSource` ([24.7][k24.7]).
13. **Browser:** rusqlite with sqlite-wasm-rs and the OPFS sahpool store, from the core's worker.

### Keeping options open

14. **A custom container stays possible.** Records are SKG, independent of the container. If the Phase 0 benchmark harness shows SQLite missing a target, the container can change without changing a record.
    - Reading forever applies to containers too ([24.6][k24.6]): readers for the SQLite container would be kept.
    - After SQLite's 2050 horizon, the published SQLite file format and independent engines such as Turso are the fallback, inside the kernel's fifty-year horizon.
    - Repeat these measurements in Rust, with the kernel's record mix, on a cold cache, on Windows, macOS and Linux, at the reference model size.

## Open risks

- The measurements come from Python on one laptop with a warm cache and a 2021 SQLite. They show the shape of the costs, not final numbers.
- The recovery extension isn't in rusqlite's or sqlite-wasm-rs's builds, so we compile `ext/recover` ourselves, natively and for WebAssembly, and test it with damaged files from the start.
- EXCLUSIVE mode means one connection, so record reads, compaction and saves share it. If parallel paging from several threads is needed, the options are normal locking with a `-shm` file (local disk only) or one storage thread feeding the workers. Measure in Design.
- Collection needs the set of names the program holds outside the kernel. A wrong set loses history, so collection runs only through `PruneHistory`, never on its own.
- sqlite-wasm-rs is a small project (98 stars on 20 October 2026), so there is a bus-factor risk. Fallbacks: SQLite's official WebAssembly build through JavaScript, or Turso once it reaches 1.0.
- Whether WAL works on the OPFS sahpool store under sqlite-wasm-rs is *unverified*. A rollback journal is the fallback in the browser.
- Large attachments (point clouds, PDFs, images) stay in the kit's file store, as Project data management already says, and scans stay in COPC ([24.4][k24.4]).
- The core gains a C dependency, which needs a C compiler in every build, including WebAssembly.
- Spare Construct's map calls a cell a "chunk". The map's owner may want to rename it, so the two meanings stop colliding.
- An `application_id` value has to be chosen. SQLite's source lists known values in [magic.txt](https://github.com/sqlite/sqlite/blob/875b95d4f5759c78f69e70496f84fb797702304d/magic.txt) for the `file` utility (Fossil, MBTiles and GeoPackage among them). How a new value gets added there is *unverified*.

## Sources

Every source is linked where it is used above. Outside sources were read on 20 October 2026, at the commits linked, and the kernel's map (G01, PR #176) on 22 October 2026 at commit b6ca7e2. Blender's code is GPL and was read for its method only; no code from any source is used.

[k20.4]: ../../engines/geometry/architecture/20-naming-history.md#204-how-a-reference-resolves
[k21]: ../../engines/geometry/architecture/21-determinism.md
[k23.1]: ../../engines/geometry/architecture/23-performance.md#231-the-targets-and-the-kernels-share
[k23.8]: ../../engines/geometry/architecture/23-performance.md#238-caching-by-content
[k23.9]: ../../engines/geometry/architecture/23-performance.md#239-memory-layout
[k23.11]: ../../engines/geometry/architecture/23-performance.md#2311-opening-fast-and-safely
[k24.1]: ../../engines/geometry/architecture/24-file-format-exchange.md#241-the-native-format-and-why-this-encoding
[k24.2]: ../../engines/geometry/architecture/24-file-format-exchange.md#242-records
[k24.4]: ../../engines/geometry/architecture/24-file-format-exchange.md#244-what-a-file-holds
[k24.5]: ../../engines/geometry/architecture/24-file-format-exchange.md#245-containers
[k24.6]: ../../engines/geometry/architecture/24-file-format-exchange.md#246-versions-and-reading-forever
[k24.7]: ../../engines/geometry/architecture/24-file-format-exchange.md#247-reading-untrusted-bytes-and-trust-on-open
[k24.13]: ../../engines/geometry/architecture/24-file-format-exchange.md#2413-what-the-kernel-never-does-here
[k25]: ../../engines/geometry/architecture/25-public-interface.md
[k26.2]: ../../engines/geometry/architecture/26-how-it-grows.md#262-versions-so-old-models-never-change-meaning
