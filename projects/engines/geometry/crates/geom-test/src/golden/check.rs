//! Checking a golden: run its case twice in this process, each time on a differently perturbed
//! heap, require one digest, and require the digest the store holds for it in the current edition.
//!
//! What it does: [`check`] takes a golden's id and its case, a closure that computes the case's
//! canonical output and returns its SHA-256 (and anything else the test wants to look at, such as
//! per-section digests or tallies). It runs the case [`REPEATS`] times, each under
//! [`Heap::shift_and_fragment`] with one of [`PERTURBATION_SEEDS`], then compares the digests with
//! each other and with [`Store::expected`]. It prints one line, `golden <id> (edition <label>):
//! <digest> ...`, which `golden run` reads to know the check really ran, and returns a [`Checked`]
//! holding the first run's value and the verdict, so a test can print its diagnostics before it
//! fails.
//!
//! Chapter 28.13's "perturbations that must change nothing" are "a test allocator that shifts and
//! fragments memory", "each case run twice in one process, to catch hidden caches", and shuffled
//! inputs; the first two are here for every golden, and the third is [`super::shuffle`], which an
//! order-free case runs inside its closure.
//!
//! Guarantees:
//!
//! - **Twice, on two heaps.** The case runs exactly [`REPEATS`] times, each run under a different
//!   seed's perturbation held for the whole run; a case whose output depends on an address, on
//!   leftover state from an earlier run, or on anything else that differs between two runs in one
//!   process fails with [`GoldenError::Repeat`], naming both digests.
//! - **The store decides.** The digest must equal the store's for the id in the current edition;
//!   otherwise [`GoldenError::Mismatch`] names the edition, the id, both digests and the two things
//!   a mismatch can mean (a bug on this target, or a deliberate change that must be recorded in the
//!   store and approved by a kernel owner). A golden the store does not hold is
//!   [`GoldenError::Store`]. Nothing is ever written to the store from here.
//! - **The repeat is checked before the store**, so a case that is not even repeatable is reported
//!   as such, not as a changed golden.
//! - **No panic**: the verdict is returned, and the test decides how to fail.
//!
//! Method: golden hashes of canonical outputs with in-process repeats and heap perturbation,
//! chapter 28.13 of the kernel architecture
//! (`projects/engines/geometry/architecture/28-testing.md`), the method chapter 21 takes from
//! Rapier's and `Box2D`'s determinism tests
//! (`projects/kit/notes/G01-evidence/05-exact-arithmetic-determinism.md`, section 7).
//!
//! Cost: the case's cost twice, plus two perturbations ([`super::perturb`]) and one parse of the
//! store.

use core::fmt;

use super::perturb::Heap;
use super::sha256::Digest;
use super::store::{Store, StoreError};
use crate::seed::Seed;

/// How many times [`check`] runs a case.
pub const REPEATS: usize = 2;

/// The heap perturbation of each run: "PERTURB1" and "PERTURB2" in ASCII.
pub const PERTURBATION_SEEDS: [Seed; REPEATS] =
    [Seed(0x5045_5254_5552_4231), Seed(0x5045_5254_5552_4232)];

/// What went wrong with a golden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GoldenError {
    /// Two runs of the case in this process gave different digests.
    Repeat {
        /// The golden's id.
        id: String,
        /// The first run's digest.
        first: Digest,
        /// The run that differed: its number (from 1) and digest.
        run: usize,
        /// That run's digest.
        other: Digest,
    },
    /// The case's digest is not the store's.
    Mismatch {
        /// The golden's id.
        id: String,
        /// The current edition.
        edition: String,
        /// What this target computed.
        got: Digest,
        /// What the store holds.
        want: Digest,
    },
    /// The store cannot be read, or does not hold the golden.
    Store(StoreError),
}

impl fmt::Display for GoldenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GoldenError::Repeat {
                id,
                first,
                run,
                other,
            } => write!(
                f,
                "golden {id}: run 1 gave {first} but run {run} gave {other} in the same process, \
                 on a differently perturbed heap. The case depends on something that changes \
                 between runs: a memory address (an ordering or a tie broken by address), a hidden \
                 cache or other state left by the first run. That is a bug in the code under test \
                 (chapter 21: no tie-breaks by memory address; principle 7: no global state); never \
                 change the golden for it"
            ),
            GoldenError::Mismatch {
                id,
                edition,
                got,
                want,
            } => write!(
                f,
                "golden {id} (edition {edition}): this target computes {got} but the store holds \
                 {want}. Either this target computes different bits, a bug (compare this run's \
                 diagnostics with another target's log), or the results were changed on purpose: \
                 then record the change in {} (the old digest in the golden's history, with its \
                 kind, piece and why) and a kernel owner approves the pull request (chapter 28.13). \
                 Never change a golden to make one target pass",
                super::store::STORE_PATH.join("/")
            ),
            GoldenError::Store(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for GoldenError {}

/// The outcome of [`check`]: the first run's value and digest, and the verdict.
#[derive(Debug)]
pub struct Checked<T> {
    /// What the first run's closure returned besides its digest.
    pub value: T,
    /// The first run's digest.
    pub digest: Digest,
    verdict: Result<(), GoldenError>,
}

impl<T> Checked<T> {
    /// Whether every run agreed and matched the store.
    ///
    /// # Errors
    ///
    /// The [`GoldenError`] found, the repeat checked before the store.
    pub fn verdict(&self) -> Result<(), GoldenError> {
        self.verdict.clone()
    }

    /// The value, once the verdict is good.
    ///
    /// # Errors
    ///
    /// As [`Checked::verdict`].
    pub fn into_value(self) -> Result<T, GoldenError> {
        self.verdict.map(|()| self.value)
    }
}

/// Checks the golden `id` (with no operation versions) against the store compiled into this crate.
pub fn check<T>(id: &str, case: impl FnMut() -> ([u8; 32], T)) -> Checked<T> {
    match Store::embedded() {
        Ok(store) => check_in(&store, id, case),
        Err(e) => {
            let mut case = case;
            let (digest, value) = case();
            Checked {
                value,
                digest: Digest(digest),
                verdict: Err(GoldenError::Store(e)),
            }
        }
    }
}

/// Checks the golden `id` against `store`: [`check`] with the store given.
pub fn check_in<T>(store: &Store, id: &str, mut case: impl FnMut() -> ([u8; 32], T)) -> Checked<T> {
    let mut first: Option<(Digest, T)> = None;
    let mut repeat: Result<(), GoldenError> = Ok(());
    for (index, seed) in PERTURBATION_SEEDS.iter().enumerate() {
        let heap = Heap::shift_and_fragment(*seed);
        let (digest, value) = case();
        drop(heap);
        let digest = Digest(digest);
        match &first {
            None => first = Some((digest, value)),
            Some((d, _)) => {
                if *d != digest && repeat.is_ok() {
                    repeat = Err(GoldenError::Repeat {
                        id: id.to_owned(),
                        first: *d,
                        run: index.saturating_add(1),
                        other: digest,
                    });
                }
            }
        }
    }
    let Some((digest, value)) = first else {
        // REPEATS is at least one, so the loop ran; this keeps the function total.
        let (digest, value) = case();
        return Checked {
            value,
            digest: Digest(digest),
            verdict: repeat,
        };
    };
    let edition = store
        .current()
        .map_or_else(|_| String::new(), |e| e.edition.clone());
    let verdict = repeat.and_then(|()| match store.expected(id) {
        Ok(want) if want == digest => Ok(()),
        Ok(want) => Err(GoldenError::Mismatch {
            id: id.to_owned(),
            edition: edition.clone(),
            got: digest,
            want,
        }),
        Err(e) => Err(GoldenError::Store(e)),
    });
    println!(
        "golden {id} (edition {edition}): {digest} {}",
        match &verdict {
            Ok(()) => format!(
                "agrees with the store; {REPEATS} runs agree on perturbed heaps (seeds {} and {})",
                PERTURBATION_SEEDS[0], PERTURBATION_SEEDS[1]
            ),
            Err(GoldenError::Repeat { .. }) => "differs between runs".to_owned(),
            Err(GoldenError::Mismatch { want, .. }) => format!("differs from the store's {want}"),
            Err(GoldenError::Store(_)) => "is not in the store".to_owned(),
        }
    );
    Checked {
        value,
        digest,
        verdict,
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use core::cell::Cell;

    use super::super::sha256::{hex, sha256};
    use super::*;

    fn store_with(id: &str, digest: [u8; 32]) -> Store {
        let (krate, _) = id.split_once('/').unwrap();
        Store::parse(&format!(
            r#"{{"format": 1, "about": "t", "editions": [{{"edition": "pre-edition",
            "state": "open", "goldens": [{{"id": "{id}", "what": "w", "sha256": "{}",
            "check": {{"crate": "{krate}", "test": "t", "function": "f"}}}}]}}]}}"#,
            hex(digest)
        ))
        .unwrap()
    }

    #[test]
    fn a_repeatable_case_that_matches_passes_and_runs_twice() {
        let store = store_with("geom-x/a", sha256(b"x"));
        let runs = Cell::new(0);
        let c = check_in(&store, "geom-x/a", || {
            runs.set(runs.get() + 1);
            (sha256(b"x"), 7)
        });
        assert_eq!(runs.get(), REPEATS);
        assert_eq!(c.verdict(), Ok(()));
        assert_eq!(c.into_value(), Ok(7));
    }

    #[test]
    fn a_changed_digest_is_a_mismatch_naming_both() {
        let store = store_with("geom-x/a", sha256(b"x"));
        let c = check_in(&store, "geom-x/a", || (sha256(b"y"), ()));
        let e = c.verdict().unwrap_err();
        assert!(matches!(e, GoldenError::Mismatch { .. }));
        let text = e.to_string();
        assert!(text.contains(&hex(sha256(b"x"))) && text.contains(&hex(sha256(b"y"))));
        assert!(text.contains("kernel owner"));
    }

    /// A case with state left over from its first run (a hidden cache) fails as a repeat, before
    /// the store is consulted, even when its first run matches the store.
    #[test]
    fn a_hidden_cache_fails_as_a_repeat() {
        let store = store_with("geom-x/a", sha256(b"0"));
        let calls = Cell::new(0u8);
        let c = check_in(&store, "geom-x/a", || {
            let n = calls.get();
            calls.set(n + 1);
            (sha256(&[b'0' + n]), ())
        });
        assert!(matches!(
            c.verdict(),
            Err(GoldenError::Repeat { run: 2, .. })
        ));
    }

    /// Code that orders values by their addresses is caught by the perturbed repeat: over a few
    /// stores, the two runs disagree.
    #[test]
    fn address_ordered_code_fails_as_a_repeat() {
        let store = store_with("geom-x/a", [0; 32]);
        let c = check_in(&store, "geom-x/a", || {
            (super::super::perturb::address_order_digest(64), ())
        });
        assert!(
            matches!(c.verdict(), Err(GoldenError::Repeat { .. })),
            "{:?}",
            c.verdict()
        );
    }

    #[test]
    fn a_golden_the_store_lacks_is_reported() {
        let store = store_with("geom-x/a", sha256(b"x"));
        let c = check_in(&store, "geom-x/b", || (sha256(b"x"), ()));
        assert!(matches!(
            c.verdict(),
            Err(GoldenError::Store(StoreError::NotFound { .. }))
        ));
    }
}
