//! The kernel as a fixture's [`Engine`]: one operation record in, the operation's canonical parts
//! out (chapter 28.16; piece G97).
//!
//! What it does: [`Kernel`] finds an operation by its name and version and runs it. The table is
//! empty until the first operations of the facade are replayable (G101 on), and each piece that
//! adds an operation adds its row here, so until then every log is refused by name.
//!
//! Guarantees:
//!
//! - **An unknown operation is refused, never guessed**: a name or version not in the table gives
//!   [`Ran::Refused`], which no status accepts.
//! - **No state survives [`Engine::start`]**: a fresh run begins from the log's own inputs.
//!
//! Method: chapter 21's replay (`projects/engines/geometry/architecture/21-determinism.md`).
//!
//! Cost: one match per operation, plus the operation.

use geom_format::operation::Operation;
use geom_format::record::Record;
use geom_num::Steps;

use super::run::{Engine, Ran};
use super::{Class, Counts, Measures};

/// The kernel, replaying a log.
#[derive(Clone, Debug, Default)]
pub struct Kernel {
    heal: Option<Steps>,
}

impl Kernel {
    /// A kernel with nothing run.
    #[must_use]
    pub fn new() -> Kernel {
        Kernel::default()
    }
}

impl Engine for Kernel {
    fn start(&mut self, heal: Steps, inputs: &[Record]) -> Result<(), String> {
        self.heal = Some(heal);
        if inputs.is_empty() {
            Ok(())
        } else {
            Err("no operation reads a stored body yet".to_owned())
        }
    }

    fn run(&mut self, operation: &Operation) -> Ran {
        Ran::Refused(format!(
            "the kernel has no operation {} at version {}",
            operation.name, operation.version
        ))
    }

    fn measures(&mut self) -> Measures {
        Measures {
            volume6: None,
            area2: None,
            counts: Counts::default(),
            bounds: None,
            exactness: Class::Exact,
        }
    }
}
