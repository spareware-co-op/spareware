//! The recursion check's model of the standard library, of `libm`, of the part of `rayon`
//! that `geom-pool` calls and of the `unicode-normalization` function `geom-op` calls: Rust source the check reads in place of theirs, so a call into them is
//! checked as any other call is.
//!
//! What it does: the kernel's crates call `core`, `alloc` (and `std` in tests) and `libm`
//! constantly, and the standard library calls kernel code back through what it is handed:
//! closures (`Option::map`, `Iterator::fold`, `sort_by`) and trait implementations of kernel types
//! (`sort` runs `Ord::cmp`, `Vec::clone` runs `Clone::clone`, formatting runs `Display::fmt`).
//! Their real source is not read; instead [`CORE`], [`LIBM`] and [`RAYON`] are Rust written from
//! their documented signatures, which [`sources`] gives the loader as three crates of role
//! [`Role::Model`]: `core` (also answering to `alloc` and `std`,
//! [`crate::recursion::resolve::MODEL_ALIASES`]), `libm` and `rayon` (`geom-pool`, the one
//! kernel crate that depends on it, runs its tasks on a rayon pool: `in_place_scope` runs its
//! closure and `Scope::spawn` its body). A function of the model that calls
//! back has a body that makes exactly those calls (`Option::map` calls `f`; `Map`'s `next` calls
//! its closure; `[T]::sort` calls `T::cmp`); one that calls nothing back has the body `loop {}`.
//! The type checker reads the model's bodies as it reads the kernel's, instance by instance, so a
//! callback reaches exactly the closure or implementation the call was handed.
//!
//! Guarantees:
//!
//! - **Never less than the library calls.** Each modelled function's body calls every closure it
//!   is handed and every trait method of its type parameters that the documented function may
//!   call; adapters that defer a call (an iterator's `map`) keep the closure in their result and
//!   call it where the library does (in `next`). By parametricity, code that does not know the
//!   kernel's types reaches kernel code only through the values and bounds it is handed (J. C.
//!   Reynolds, "Types, abstraction and parametric polymorphism", IFIP 1983; P. Wadler, "Theorems
//!   for free!", FPCA 1989, <https://doi.org/10.1145/99370.99404>), so these are all the calls it
//!   can make into the kernel.
//! - **What is not modelled is reported.** A path into the standard library that names nothing in
//!   the model resolves to nothing, and the type checker reports it as code it cannot read; the
//!   model is extended rather than guessed.
//!
//! Method: the standard library's documented signatures and behaviour
//! (<https://doc.rust-lang.org/std/>, <https://doc.rust-lang.org/core/>,
//! <https://docs.rs/libm/0.2.16/libm/>, <https://docs.rs/rayon/1.12.0/rayon/>), written as stubs: a library model, which the type
//! checker reads as ordinary code, under parametricity as above.
//!
//! Cost: three more crates for the loader to parse, about 5 000 lines; their instances are checked
//! only when kernel code calls them.

use crate::recursion::model::{CrateSource, Role};

/// The model of `core`, `alloc` and `std`.
pub const CORE: &str = include_str!("stdlib/core.rs.txt");

/// The model of `libm`.
pub const LIBM: &str = include_str!("stdlib/libm.rs.txt");

/// The model of the part of `rayon` that `geom-pool` calls.
pub const RAYON: &str = include_str!("stdlib/rayon.rs.txt");

/// The model of `unicode-normalization`'s `is_nfc`, which `geom-op` calls.
pub const UNICODE_NORMALIZATION: &str = include_str!("stdlib/unicode_normalization.rs.txt");

/// Where the model of `core` is, relative to the workspace root (its text is [`CORE`]).
pub const CORE_PATH: &str =
    "projects/engines/geometry/crates/geom-test/src/recursion/stdlib/core.rs.txt";

/// Where the model of `libm` is, relative to the workspace root (its text is [`LIBM`]).
pub const LIBM_PATH: &str =
    "projects/engines/geometry/crates/geom-test/src/recursion/stdlib/libm.rs.txt";

/// Where the model of `rayon` is, relative to the workspace root (its text is [`RAYON`]).
pub const RAYON_PATH: &str =
    "projects/engines/geometry/crates/geom-test/src/recursion/stdlib/rayon.rs.txt";

/// Where the model of `unicode-normalization` is, relative to the workspace root (its text is
/// [`UNICODE_NORMALIZATION`]).
pub const UNICODE_NORMALIZATION_PATH: &str =
    "projects/engines/geometry/crates/geom-test/src/recursion/stdlib/unicode_normalization.rs.txt";

/// The folder of the model's files.
pub const FOLDER: &str = "projects/engines/geometry/crates/geom-test/src/recursion/stdlib";

/// The model's crates, for the loader: `core`, `libm`, `rayon` and `unicode-normalization`.
#[must_use]
pub fn sources() -> Vec<CrateSource> {
    vec![
        CrateSource {
            package: "core".to_owned(),
            lib: "core".to_owned(),
            folder: FOLDER.to_owned(),
            root: CORE_PATH.to_owned(),
            role: Role::Model,
        },
        CrateSource {
            package: "libm".to_owned(),
            lib: "libm".to_owned(),
            folder: FOLDER.to_owned(),
            root: LIBM_PATH.to_owned(),
            role: Role::Model,
        },
        CrateSource {
            package: "rayon".to_owned(),
            lib: "rayon".to_owned(),
            folder: FOLDER.to_owned(),
            root: RAYON_PATH.to_owned(),
            role: Role::Model,
        },
        CrateSource {
            package: "unicode-normalization".to_owned(),
            lib: "unicode_normalization".to_owned(),
            folder: FOLDER.to_owned(),
            root: UNICODE_NORMALIZATION_PATH.to_owned(),
            role: Role::Model,
        },
    ]
}

/// The text of a model file by its path, or `None` for any other path.
#[must_use]
pub fn read_model(path: &str) -> Option<String> {
    match path {
        CORE_PATH => Some(CORE.to_owned()),
        LIBM_PATH => Some(LIBM.to_owned()),
        RAYON_PATH => Some(RAYON.to_owned()),
        UNICODE_NORMALIZATION_PATH => Some(UNICODE_NORMALIZATION.to_owned()),
        _ => None,
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may use native arithmetic, casts, unwrap and indexing"
)]
mod tests {
    use super::*;
    use crate::recursion::model::load;

    #[test]
    fn the_model_parses_and_loads_without_problems() {
        let read = |p: &str| read_model(p).ok_or_else(|| "no such file".to_owned());
        let p = load(&sources(), &read);
        assert!(p.problems.is_empty(), "{:#?}", p.problems);
        assert!(p.nodes.len() > 1000, "{}", p.nodes.len());
        let has = |path: &str| p.items.iter().any(|i| i.path == path);
        for path in [
            "core::option::Option",
            "core::result::Result",
            "core::vec::Vec",
            "core::iter::Iterator",
            "core::cmp::Ordering",
            "core::fmt::Formatter",
            "core::num::NonZeroU64",
            "core::sync::atomic::AtomicU64",
        ] {
            assert!(has(path), "{path}");
        }
    }

    #[test]
    fn model_paths_are_the_files_on_disk() {
        assert!(
            CORE_PATH.starts_with(FOLDER)
                && LIBM_PATH.starts_with(FOLDER)
                && RAYON_PATH.starts_with(FOLDER)
                && UNICODE_NORMALIZATION_PATH.starts_with(FOLDER)
        );
        assert_eq!(read_model(CORE_PATH).as_deref(), Some(CORE));
        assert!(read_model("elsewhere.rs").is_none());
    }
}
