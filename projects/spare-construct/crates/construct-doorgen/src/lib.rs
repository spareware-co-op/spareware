//! Spare Construct: the door and window generator (CON-A7-F4-P1) and its checks (CON-A7-F4-P3).
//!
//! A spec (a schedule's row, a supplier catalogue's line) fills one of the library's door and
//! window templates (`library/doors-windows`, CON-A7-F4-P2) and gives a family type that is
//! valid: its solids evaluate through construct-families' evaluator, the opening its wall is cut
//! by is clean, and its tags (the type mark, the clear opening, the leaf and glass areas) are
//! what its solids measure. The choices are note CON-A7-F4-P1-door-window-generator; the map is
//! projects/spare-construct/ARCHITECTURE.md, "Families".
//!
//! - [`Spec`], [`Size`], [`Hand`]: what is asked for.
//! - [`Template`], [`Range`], [`Part`]: the templates, baked in, the range of sizes each is made
//!   across and the parts a spec may set.
//! - [`generate()`] and [`Family`]: specs made into one family recipe, a type table row each
//!   ([`Generated`], [`GeneratedType`]), every spec judged by its template's rules first (crate
//!   module `rules`), each type then built, measured and checked ([`Quantities`]).
//! - [`verify`]: every type of a generated recipe checked again from its stored rows.
//! - [`Refusal`] and [`Rule`]: a spec outside its template's rules, refused by the rule's name
//!   with the reason, never built wrongly.
//! - [`measure`]: the measuring engine behind the kernel's `Engine` trait that the checks build
//!   through, so every quantity is exact in grid steps.
//!
//! What it may not do: write geometry. The generator sets a template's parameters and writes type
//! rows and tags; every solid, opening and symbol comes from the template through the evaluator.
//!
//! Promises: exact (lengths in grid steps, areas in square steps, every step checked); the same
//! specs give the same recipe bytes on every run; no clock, no random number, no hash-map order;
//! nothing panics. Depends on kit-core, construct-families, construct-params and engine-geometry
//! (the kernel's interface; no engine is linked). Nothing here is interim.
//!
//! Not here, with its place: commands through the edit log and the page (later jobs), and storing
//! generated family types in a project's document (job 7.6).

pub mod checks;
pub mod generate;
pub mod measure;
pub mod refusal;
mod rules;
pub mod spec;
pub mod template;

pub use checks::Quantities;
pub use generate::{Family, Generated, GeneratedType, generate, mark_key, verify};
pub use refusal::{Refusal, Rule};
pub use spec::{Hand, Size, Spec};
pub use template::{Part, Range, Template};
