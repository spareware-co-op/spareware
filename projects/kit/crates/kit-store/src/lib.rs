//! Spare Kit: storage.
//!
//! Chunked storage, the spatial index and paging from disk, so an app only loads what someone is
//! working on. Map: root ARCHITECTURE.md; note K08.
//!
//! Three things live here, and no database:
//!
//! - [`key`]: where every piece is stored. This layout is a published format, FORMAT.md.
//! - [`row`]: how a row is written, with its version, so old rows stay readable forever.
//! - [`kv`]: the small transactional store everything above is written against. TiKV implements
//!   it (`projects/kit/services/store`), and so does [`memory`], which injects faults for tests.
//!
//! Keeping the database behind a trait is what lets a crashed commit or two writers racing for one
//! log number be tested in plain `cargo test`, on any machine, without TiKV.

pub mod key;
pub mod kv;
#[cfg(feature = "memory")]
pub mod memory;
pub mod row;

pub use key::{FORMAT, Key, Place, Prefix, Scope, table, table_prefix};
pub use kv::{FileStore, Kv, Pair, Reader, StoreError, Txn};
pub use row::{from_row, read_row, scan_rows, to_row};
