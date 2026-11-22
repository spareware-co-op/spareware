//! Inputs for the facade, `engine-geometry` (piece G100, decision 32.46): the names a program or an
//! AI tool calls operations by, and the calls it dispatches through the facade.
//!
//! What it does: [`names`] draws, per input, the name of one operation the facade registers
//! (`engine_geometry::operations::OPERATIONS`), each with equal chance, which the benchmark finds
//! in the table and reads the schema of; [`calls`] draws, per input, a call of `make.box` by its
//! name with a box within ±2^20 grid steps of the origin, as `geom-make`'s own benchmark draws them
//! ([`super::make`]), which the benchmark finds by name and runs through the facade's re-export.
//!
//! Guarantees: every typical batch is full, every name is registered and every box is valid, so
//! every lookup finds its operation and every call succeeds (`tests/inputs.rs`), and each input
//! has its own draws. Only the typical class exists: an unknown name is the caller's refusal, not
//! the facade's, and `make.box`'s refusals are tested in `geom-make` itself.
//!
//! Method: the facade's own table, read as `geom-fuzz`'s `contract` target reads it, and the calls
//! of `engine-geometry`'s `tests/attack_g101.rs`, cut to one operation; each is measured as chapter
//! 28.14 sets benchmarks out, `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: one or a few draws of [`crate::rng::SplitMix64`] per input, once per batch, outside every
//! measured run.

use engine_geometry::operations::OPERATIONS;
use geom_make::BoxInput;

use super::{Class, batch_of, make, rng};

/// The name `make.box` is called by.
pub const MAKE_BOX: &str = "make.box";

/// One registered operation's name per lookup.
#[must_use]
pub fn names(class: Class) -> Vec<String> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("facade::lookup", class);
    batch_of(|_| {
        OPERATIONS
            .get(g.index(OPERATIONS.len()))
            .map(|op| op.schema.name.to_owned())
    })
}

/// One call of `make.box` by its name per dispatch.
#[must_use]
pub fn calls(class: Class) -> Vec<(String, BoxInput)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("facade::dispatch", class);
    batch_of(|_| Some((MAKE_BOX.to_owned(), make::a_box(&mut g))))
}
