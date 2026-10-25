//! The golden store and the determinism matrix: every golden digest of the kernel in one store
//! keyed by edition, the one SHA-256 every golden is hashed with, the perturbations every golden
//! must survive, and the runner that checks every golden on every target and build mode.
//!
//! What it does: chapter 28.1 puts "the golden store" among `geom-test`'s CI tools, and chapter
//! 28.13 says what it holds and what runs through it. The parts:
//!
//! - [`sha256`]: SHA-256 (the `RustCrypto` project's `sha2`), [`Sha256`], [`hex`] and
//!   [`Digest`]: the one implementation every golden test hashes with, replacing the copies each
//!   crate's tests carried.
//! - [`store`]: the store, `golden/store.json` in this crate's folder, keyed by kernel edition,
//!   golden id and operation versions, with every change's record; compiled into this crate, so a
//!   golden test reads it on every target, WebAssembly included.
//! - [`check()`]: checks one golden: runs its case twice in one process, each time on a heap the
//!   perturbing allocator ([`perturb`]) has shifted and fragmented from its own seed, and requires
//!   one digest, the one the store holds in the current edition.
//! - [`shuffle`]: the input-shuffle hook, for cases whose contract says input order does not
//!   matter.
//! - [`diff`]: the owner-approval rule for edits to the store, and the review check behind it.
//! - [`repo`]: the store against the repository: every golden checked where the store says, every
//!   golden check in the code in the store, no golden digest kept outside it, every pinned golden
//!   file unchanged.
//! - [`matrix`]: the runner the binary `golden` uses to run every golden of the store in one CI
//!   leg's configuration and prove that each ran and agreed.
//!
//! The binary `golden` (`cargo run -p geom-test --bin golden -- <command>`) has the commands
//! `list`, `check`, `run` and `diff` (its own header says what each does).
//!
//! Guarantees: as each part's header states; together they make these promises.
//!
//! - **One store, one hash.** Every golden digest of the kernel's crates is in the store; [`repo`]
//!   fails on a golden digest constant anywhere else, and every one is SHA-256 from [`sha256`].
//! - **Every golden on every leg.** `kernel.yml` runs the store's whole current edition through
//!   [`matrix`] on each of its targets and build modes, so a golden added to the store is checked
//!   everywhere the day it lands.
//! - **Perturbations change nothing, or the golden fails.** Every golden runs twice on two
//!   differently perturbed heaps; order-free cases also run in shuffled orders.
//! - **Edits are visible and approved.** A changed digest without its history record, a removed
//!   golden, or any change to a frozen edition is refused; any other edit to an existing golden
//!   needs a kernel owner's approval on the pull request's head commit.
//!
//! Method: golden hashes of canonical outputs per kernel edition and operation version, with
//! perturbations that must change nothing, chapter 28.13 of the kernel architecture
//! (`projects/engines/geometry/architecture/28-testing.md`); the determinism matrix of chapter 21
//! (`projects/engines/geometry/architecture/21-determinism.md`), which follows Rapier's and
//! `Box2D`'s determinism tests
//! (`projects/kit/notes/G01-evidence/05-exact-arithmetic-determinism.md`, section 7).
//!
//! Cost: as each part's header states.

pub mod check;
pub mod diff;
pub mod matrix;
pub mod perturb;
pub mod repo;
pub mod sha256;
pub mod shuffle;
pub mod store;

pub use check::{Checked, GoldenError, check, check_in};
pub use sha256::{Digest, Sha256, hex, sha256};
pub use store::Store;
