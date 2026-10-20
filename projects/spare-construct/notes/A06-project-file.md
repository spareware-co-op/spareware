# A06: Project file: SQLite container or a custom chunk file

## The question

On the desktop, one portable project file holds the chunk blobs, the operation log and the index ([ARCHITECTURE.md](../ARCHITECTURE.md), Storage and scale). Should that file be a SQLite database with our own published schema, or a chunk file format we write ourselves? The piece asks for a recommendation covering crash safety, size and streaming. Spare Studio's project file follows the same decision ([STU-A2-F5-P1](../../spare-studio/FEATURES.md)).

Sources were read on 20 October 2026. "Unverified" marks anything not confirmed in source code or on an official page.

## What the file has to do

From the map:

- **Save in under 100 ms, never blocking.** Operations append to the log at once; chunks compact in the background (Storage and scale; Performance targets).
- **Open a 230,000-element building (1.33 GB as .rvt) to first view in under 3 s**, loading only the chunks in view. A city never has to fit in RAM.
- **Chunks** group by building, level and spatial cell. Chunk blobs are compact binary, versioned, zstd-compressed and content-hashed, so unchanged chunks are never saved or sent again.
- **One core, native and WebAssembly.** The browser core runs in a worker and works offline on small models (Runtime shape).
- **A version is a pointer into the operation log plus a compacted snapshot** (Versions and revisions).
- **Open format:** documented and readable without the app (ground rule 7). [EXTENDING.md](../../../EXTENDING.md) section 3 adds: "Prefer an existing standard to a new format. A new format needs a decision note that says why none will do."
- **From scratch where it decides speed or ownership**, and the file format is on that list (ground rule 1).

## Options

| | A. SQLite container | B. Custom chunk file, from scratch | C. Custom file on a Rust embedded store |
| --- | --- | --- | --- |
| What it is | One SQLite database: our tables for the operation log, chunk blobs, chunk index and versions | Our own byte layout: framed, checksummed records appended to a file, with an index we maintain | Our own tables on [redb](https://github.com/cberner/redb), a pure-Rust B-tree store |
| Crash safety | Built in: atomic commits, checksummed write-ahead log, 590 times as much test code as library code | Ours to build and prove | Built in: copy-on-write pages, checksums, two commit slots |
| Space after edits | Free pages reused inside the file | Grows until we rewrite it, unless we also build a page allocator | Free pages reused (buddy allocator) |
| Read one chunk | B-tree lookup by key | Seek to an offset from our index | B-tree lookup by key |
| Browser | rusqlite uses sqlite-wasm-rs with an OPFS backend | We write an OPFS backend | We write a `StorageBackend` for OPFS (*unverified* that anyone has) |
| Readable with other tools | Yes: any SQLite tool or language lists the tables and reads the log | Only with our reader | Only through redb |
| Licence | Public domain | Ours | MIT or Apache-2.0 |

A fourth family, formats that rewrite the whole file on every save (a zip of files, or Blender's write-then-rename), is ruled out by the measurement below: rewriting 192 MB took 549 ms, more than five times the save budget.

## Evidence

### Crash safety

**SQLite.** "Writes to an SQLite database are atomic. They either happen completely or not at all, even during system crashes or power failures" ([application file format](https://www.sqlite.org/appfileformat.html)). In write-ahead log (WAL) mode, every frame of the log carries two salts and a checksum. Recovery accepts a frame only if its salts match the log header and its checksum is valid, and the salts change after each checkpoint so stale frames can never be replayed (header comment of [src/wal.c](https://github.com/sqlite/sqlite/blob/master/src/wal.c)). Testing: the library is about 155.8 KSLOC with 590 times as much test code, 100% branch and MC/DC coverage of the core, and crash tests in which a special file system "randomly reorders and corrupts the unsynchronized write operations" before checking that each transaction either fully happened or fully rolled back ([how SQLite is tested](https://www.sqlite.org/testing.html)).

SQLite lists the ways a database still gets corrupted ([how to corrupt](https://www.sqlite.org/howtocorrupt.html)). The ones that matter to us:

- Network file systems with broken locking, and drives that "lie about syncing" (USB sticks especially).
- Copying the file while a transaction is open, or separating it from its `-wal` file. A safe copy uses the backup API or `VACUUM INTO`.
- `PRAGMA synchronous=OFF`.

WAL needs shared memory, so "WAL does not work over a network filesystem" ([WAL](https://www.sqlite.org/wal.html)). The exception: in `locking_mode=EXCLUSIVE`, set before the first access, "SQLite never attempts to call any of the shared-memory methods" (same page; the source turns the lock routines into no-ops in that mode). For network shares in general, SQLite advises a client/server database and treats exclusive one-at-a-time access as a mitigation, not a guarantee ([SQLite over a network](https://www.sqlite.org/useovernet.html)). Many firms keep project files on office file servers (*unverified as a general claim*), so the app has to handle this case on purpose.

**A store we write ourselves** has to solve the same problem. redb shows what that takes ([design.md](https://github.com/cberner/redb/blob/master/docs/design.md)). Every page is copy-on-write. Two commit slots are chosen by one "god byte", and flipping that byte is the commit. Pages carry XXH3-128 checksums in a Merkle tree, and the default commit (1PC+C) uses one fsync. If a commit is torn, "the checksum verification will fail, and it will be rolled back." It also depends on assumptions: single-byte writes are atomic, and writes never touch bytes outside their range. And because XXH3 is not collision-resistant, files from someone you don't trust need the slower two-fsync commit. Getting a design like this right and proven is years of work. SQLite and redb have already spent those years.

**Whole-file save.** Blender writes the new file to `<name>@`, renames it over the original, and shifts older saves to `.blend1`, `.blend2` and so on (`BLO_write_file_impl` and `do_history` in [writefile.cc](https://github.com/blender/blender/blob/main/source/blender/blenloader/intern/writefile.cc). It is GPL-2.0-or-later, so this is the method only, and no code is used). It is safe, but each save rewrites everything, which fails the save target for large models (measured below).

### Size

| Chunk size | SQLite, 4 KB pages | SQLite, 16 KB pages | SQLite, 64 KB pages | Custom file |
| --- | --- | --- | --- | --- |
| 64 KB | +0.9% | +3.2% | +14.4% | 0% |
| 256 KB | +0.3% | +0.8% | +3.7% | 0% |
| 1 MB | +0.3% | +0.2% | +1.0% | 0% |

*File size over the raw bytes stored, 192 MB of incompressible chunks (measured, see below).*

- After replacing a quarter of the chunks one at a time (what background compaction does), the SQLite file stayed the same size, because freed pages were reused. The append-only custom file grew by 25% (192 MB to 240 MB) and stays that way until a full rewrite. Avoiding that means building a page allocator, which is what redb's buddy allocator is.
- 64 KB pages are the wrong choice for 64 KB chunks: a blob plus its row header no longer fits in one page and spills onto an overflow page.
- SQLite's own measurements say blobs under about 100 KB read faster from inside the database than from separate files, and that "a database page size of 8192 or 16384 gives the best performance for large BLOB I/O" ([internal versus external BLOBs](https://www.sqlite.org/intern-v-extern-blob.html), measured on a 2011 machine).
- Limits are far above our needs: 1,000,000,000 bytes per blob by default (hard limit 2,147,483,645), and about 281 TB per file at 64 KB pages ([limits](https://www.sqlite.org/limits.html)).
- Chunk blobs belong in an ordinary rowid table with a unique index on the hash, not a `WITHOUT ROWID` table keyed by hash. `WITHOUT ROWID` rows should be under about 1/20th of a page, and incremental blob I/O "does not work for WITHOUT ROWID tables" ([WITHOUT ROWID](https://www.sqlite.org/withoutrowid.html)). Speckle's local cache uses `objects(hash TEXT PRIMARY KEY, content TEXT) WITHOUT ROWID` with `INSERT OR IGNORE` ([sqlite.py](https://github.com/specklesystems/specklepy/blob/main/src/specklepy/transports/sqlite.py), Apache-2.0). That suits its small JSON objects, but not our larger chunks.

### Streaming

- **Inside the desktop core**, streaming means paging chunks from disk by key. Reading one chunk took 0.07 ms (64 KB) to 2.4 ms (1 MB) from SQLite, and 0.03 to 1.4 ms from a plain file (median, warm cache). Both leave the 3 s first-view budget almost untouched. As an estimate, 200 MB of 256 KB chunks would take about 0.25 s to read from SQLite and 0.09 s from a plain file. Decompression and GPU upload are more likely to be the bottleneck (*unverified*).
- **Between machines**, the map already sends chunks, not files. Chunks are content-addressed (Speckle's method, Sharing with consultants), the server keeps them in an object store, and a client asks only for hashes it lacks. If the file's chunk table is keyed by the same hash, a sync is "send the rows the other side doesn't have". The file itself never has to stream over HTTP.
- **If we ever want to stream the file itself**, both approaches have been proven:
  - SQLite: [Cloud Backed SQLite](https://sqlite.org/cloudsqlite/doc/trunk/www/index.wiki) splits a database into fixed-size, never-modified blocks in Azure or Google Cloud Storage, with a local block cache. Bentley's iModel engine ([imodel-native](https://github.com/iTwin/imodel-native), Apache-2.0) uses it: [CloudSqlite.cpp](https://github.com/iTwin/imodel-native/blob/main/iModelCore/BeSQLite/CloudSqlite.cpp) calls `sqlite3_bcvfs_attach`, `sqlite3_bcvfs_upload`, `sqlite3_bcvfs_poll` and `sqlite3_bcvfs_prefetch_new`. [sql.js-httpvfs](https://github.com/phiresky/sql.js-httpvfs) (Apache-2.0) reads a SQLite file over HTTP range requests, read-only.
  - Custom: [PMTiles](https://github.com/protomaps/PMTiles/blob/main/spec/v3/spec.md) (spec public domain or CC0, code BSD-3-Clause) is a single file with a 127-byte header and a root directory inside the first 16 KiB, made for static hosting. Its spec says nothing about updating a file in place, so it fits write-once archives, not a file being edited.
- **Precedents in our own field.** iModels are SQLite files ("SQLite · .bim iModel files" in the imodel-native README), and their changesets come from SQLite's session extension ([ChangeSet.cpp](https://github.com/iTwin/imodel-native/blob/main/iModelCore/BeSQLite/ChangeSet.cpp) calls `sqlite3session_create` and `sqlite3changeset_apply_v3_strm`). [GeoPackage](https://www.geopackage.org/spec/) (OGC 12-128r19) is "a platform-independent SQLite database file" for map tiles and features, with R-tree spatial indexes kept up to date by triggers.

### Browser

- rusqlite 0.40.1 uses [sqlite-wasm-rs](https://github.com/Spxg/sqlite-wasm-rs) (MIT) on `wasm32-unknown-unknown` by default (`ffi-sqlite-wasm-rs` in its [Cargo.toml](https://github.com/rusqlite/rusqlite/blob/master/Cargo.toml)). sqlite-wasm-rs offers an in-memory store and an OPFS "sahpool" store, and builds SQLite with `-DSQLITE_THREADSAFE=0`. That fits a core that owns its storage from one worker. On native targets rusqlite bundles SQLite 3.54.0.
- SQLite's own notes on the OPFS sahpool store say it needs no COOP or COEP headers, has worked in all major browsers since March 2023, and is "easily the highest OPFS performance" of the options, at the cost of one connection per database ([persistence](https://sqlite.org/wasm/doc/trunk/persistence.md)).
- With a custom file or redb, we write the OPFS layer ourselves. redb's `StorageBackend` (read, write, set_len, sync_data, optional range locks in [backends.rs](https://github.com/cberner/redb/blob/master/src/tree_store/page_store/backends.rs)) makes that possible.
- [Turso](https://github.com/tursodatabase/turso) (MIT) is a Rust database compatible with SQLite at the "file format, and C API levels", runs in browsers, and has "not yet reached 1.0". Choosing SQLite's file format leaves room for a pure-Rust engine later without changing anyone's files.

### Openness, and ground rule 1

- The SQLite file format has been stable since 2004, the US Library of Congress lists SQLite as a [recommended storage format](https://www.sqlite.org/locrsf.html) for datasets, and SQLite plans [support through 2050](https://www.sqlite.org/lts.html). The code is [public domain](https://www.sqlite.org/copyright.html), so it meets ground rule 3.
- Anyone can open the file with Python's standard library and read the operation log and chunk tables. The chunk blob encoding still needs our published specification and reader library. A container makes the file easy to open; it doesn't make our encodings self-explanatory.
- The header has a 32-bit `application_id` at offset 68, so tools can tell a Spare Construct file from "SQLite3 Database", and a `user_version` at offset 60 that the app owns ([pragmas](https://www.sqlite.org/pragma.html)).
- **Ground rule 1** asks for the file format from scratch. Everything that makes the format ours is still built from scratch: the schema, the operation encoding, the chunk blob encoding, the chunk layout and compaction. SQLite supplies only the page store and transactions, in the same way zstd supplies compression. And EXTENDING.md asks for an existing standard unless none will do, which this note finds is not the case.

### Local measurement

This is a quick check, not a benchmark-harness result. It used Python 3.10.6's `sqlite3` module (SQLite 3.37.2, from 2021; rusqlite bundles 3.54.0) on Windows 11 with an Intel i7-11370H, 40 GB of RAM and an NVMe SSD (NTFS), with the operating system's cache warm. Python overhead is in every number for both options. The custom file was the simplest honest version: length, CRC-32 and payload per record, with an in-memory index.

| Save: one transaction of 50 operations × 200 bytes | Median | p99 | Worst |
| --- | --- | --- | --- |
| SQLite WAL, EXCLUSIVE, `synchronous=NORMAL`, automatic checkpoints (3,000 saves) | 0.10 ms | 0.67 ms | 75.4 ms |
| SQLite WAL, EXCLUSIVE, `synchronous=FULL`, automatic checkpoints (3,000 saves) | 0.63 ms | 2.94 ms | 25.9 ms |
| SQLite WAL, EXCLUSIVE, `synchronous=FULL`, checkpoints run outside the save (3,000 saves) | 0.62 ms | 1.71 ms | 5.5 ms |
| Custom append file, fsync on every save (400 saves) | 0.57 ms | 1.81 ms | 2.2 ms |
| Whole-file rewrite, temp file then rename: 64 MB / 192 MB | 167 ms / 549 ms | | |

- Most of the slowest SQLite saves came at regular intervals (save numbers 340, 682, 1023, 1705 and 2046 among the six slowest), about every 1,000 log pages. That matches SQLite's automatic checkpoint ([WAL](https://www.sqlite.org/wal.html)). With automatic checkpoints off and a checkpoint every 250 saves outside the save path, the worst save fell to 5.5 ms. The checkpoint itself took 7 to 41 ms. It is real work, but it can wait for idle time.
- Bulk writing 192 MB in one transaction took 2.1 to 3.7 s in SQLite (16 KB pages) against 0.85 to 1.2 s for the plain file. That matters for imports, not for saves.
- Replacing one chunk in its own transaction took 0.2 ms (64 KB) to 6 ms (1 MB), so compaction can work one chunk at a time between saves.

## Recommendation

1. **Use a SQLite container (option A)** with our own published, versioned schema. This confirms the map's lean. It meets the save target with room to spare, needs no space management of our own, works in the browser through an existing crate, and is an existing standard any tool can open.
2. **Settings to confirm in the Design stage:**
   - `page_size=16384`, WAL, and `locking_mode=EXCLUSIVE` on the desktop. One process owns the file and no `-shm` file is created.
   - `synchronous=FULL`, so a save is on disk when it returns (p99 under 3 ms measured).
   - `wal_autocheckpoint=0`, with the storage thread checkpointing when idle and `journal_size_limit` capping the leftover log.
   - `auto_vacuum=INCREMENTAL`, with `incremental_vacuum` run when idle.
   - Our own `application_id`, and `user_version` set to the schema version.
3. **Layout sketch, for the file format draft:** `oplog` (sequence, operation ID, author, time, operation bytes); `chunk_blob` (rowid table, unique hash, codec, bytes); `chunk` (building, level, cell, detail level, current hash); `chunk_bounds` (an R*Tree in building-local coordinates); `version` (name, log position, snapshot); `meta`. Compaction writes one chunk per transaction. Replay always reads the log `ORDER BY seq`, never in storage order, to keep ground rule 6.
4. **Can we work from a network share? Yes, through a local working copy.** When a project file sits on a network path, the app works on a copy on local disk and writes it back with SQLite's backup API or `VACUUM INTO` at save points and on close. Teams that need to edit together use the server operation log, as planned.
5. **Treat files from other people as untrusted** ([security](https://www.sqlite.org/security.html)): turn off trusted schema, triggers and views on every connection, run `PRAGMA quick_check` before anything else, and refuse files that fail it.
6. **Browser:** rusqlite with sqlite-wasm-rs and the OPFS sahpool store, from the core's worker.
7. **Keep a custom container possible.** The operation records and chunk blobs use our own encodings, independent of the container. If the Phase 0 benchmark harness shows SQLite missing a target, the container can change without changing the encodings. Repeat this measurement in Rust, on a cold cache, on Windows, macOS and Linux, with a model at the reference size.

## Open risks

- The measurements come from Python on one laptop with a warm cache and a 2021 SQLite. They show the shape of the costs, not final numbers.
- EXCLUSIVE mode means one connection, so chunk reads and saves share it. If parallel paging from several threads is needed, the options are normal locking with a `-shm` file (local disk only) or one storage thread feeding the workers. Measure in Design.
- sqlite-wasm-rs is a small project (98 stars on 20 October 2026), so there is a bus-factor risk. Fallbacks: SQLite's official WebAssembly build through JavaScript, or Turso once it reaches 1.0.
- Whether WAL works on the OPFS sahpool store under sqlite-wasm-rs is *unverified*. A rollback journal is the fallback in the browser.
- Large attachments (point clouds, PDFs, images) should live in the kit's file store, as Project data management already says, not as blobs in the project file. Above about 1 MB a blob reads slower than a plain file, and the default blob limit is 1,000,000,000 bytes.
- The core gains a C dependency, which needs a C compiler in every build, including WebAssembly.
- Content hashing needs a deterministic chunk encoding. That doesn't depend on the container, but it has to be specified.
- An `application_id` value has to be chosen. SQLite's source lists known values in [magic.txt](https://github.com/sqlite/sqlite/blob/master/magic.txt) for the `file` utility (Fossil, MBTiles and GeoPackage among them). How a new value gets added there is *unverified*.

## Sources

Every source is linked where it is used above. All were read on 20 October 2026. Blender's code is GPL and was read for its method only; no code from any source is used.
