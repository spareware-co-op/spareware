//! One layer of a wall, what its function means, and the join rule that the function's priority
//! gives.
//!
//! A layered wall is read from the outside in: a finish that meets the weather or the room, often
//! a cavity or insulation, a sheet that keeps water or vapour out, the part that carries the load
//! (the core: a frame, blockwork, concrete), then the inner lining. Each layer's function says
//! how it behaves where two walls meet and how a cut through it is drawn.
//!
//! | Function | What it is | Priority | At a join | In a cut drawing |
//! | --- | --- | --- | --- | --- |
//! | [`Function::Structure`] | What carries the load: frame, block, concrete | 1 | Runs through and cuts every weaker layer | Its material's cut pattern; where it bounds the core, the core's (heavier) cut line |
//! | [`Function::Substrate`] | A board or sheathing that backs a finish | 2 | Cuts insulation, finishes and membranes | Its material's cut pattern, the finish cut line |
//! | [`Function::Thermal`] | Insulation, or an air space | 3 | Cuts finishes and membranes | Insulation: its material's pattern; an air space (material of class air gap): no fill |
//! | [`Function::Finish`] | The face people see, inside or out | 4 | Cuts only membranes | Its material's pattern, the finish cut line |
//! | [`Function::Membrane`] | A sheet: a wrap, a vapour or damp barrier | none | Never cuts; stops at the first layer it meets | One line (it may be 0 thick) |
//!
//! The join rule ([`join`]), which the plan join solver (job 5.1) applies to each pair of layers
//! that meet: the layer with the stronger priority (the lower number) runs through the join and
//! cuts the weaker one. A tie is not broken by order or by side: neither layer cuts, the two meet
//! on the join line, and when their materials are the same they are drawn as one, with no line
//! between them. A membrane has no priority: it never cuts, and two membranes meet as a tie.

use construct_materials::MaterialId;
use construct_params::Length;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// What a layer does in the wall. The stored names are the variant names in snake case.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Function {
    /// What carries the load: a timber or steel frame, blockwork, concrete.
    Structure,
    /// A board or sheathing that a finish is fixed to.
    Substrate,
    /// Insulation, or an air space.
    Thermal,
    /// The face people see, inside or out: cladding, a lining, plaster, brick veneer.
    Finish,
    /// A sheet that keeps water, air or vapour out: a wrap or a barrier. It may be 0 thick.
    Membrane,
}

impl Function {
    /// Every function, strongest first.
    pub const ALL: [Self; 5] = [
        Self::Structure,
        Self::Substrate,
        Self::Thermal,
        Self::Finish,
        Self::Membrane,
    ];

    /// Its join priority: 1 is the strongest; a membrane has none.
    #[must_use]
    pub const fn priority(self) -> Option<u8> {
        match self {
            Self::Structure => Some(1),
            Self::Substrate => Some(2),
            Self::Thermal => Some(3),
            Self::Finish => Some(4),
            Self::Membrane => None,
        }
    }

    /// Whether a layer of this function may be 0 thick: only a membrane.
    #[must_use]
    pub const fn may_be_zero(self) -> bool {
        matches!(self, Self::Membrane)
    }

    /// Its priority as a rank where a lower number is stronger and a membrane is weakest.
    const fn rank(self) -> u8 {
        match self.priority() {
            Some(priority) => priority,
            None => u8::MAX,
        }
    }
}

/// One layer: what it does, how thick it is, what it is made of, and whether it wraps at the
/// wall's free ends and at the sides of openings. Whether it is in the core is not the layer's
/// own: the type's core boundaries say it ([`crate::Assembly::zone`]).
///
/// Wrapping: a wrapping layer turns the corner at a free end or an opening side and runs across
/// the end of the core to the core's far face; a layer that does not wrap stops flush with the
/// end. Where it turns is the type's [`crate::Wrapping`] (or a wall's own over it): its free ends
/// and its inserts each wrap one side, the other, or (at inserts) both ([`crate::turned`]).
/// Only a shell layer may wrap; [`crate::Assembly`] refuses a core layer that wraps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    /// What it does.
    pub function: Function,
    /// Its thickness, never below 0, and 0 only for a membrane.
    pub thickness: Length,
    /// What it is made of: a material in the project's materials (construct-materials).
    pub material: MaterialId,
    /// Whether it wraps where its wall wraps its side, at free ends and at the sides of openings.
    pub wraps: bool,
}

impl Layer {
    /// A layer.
    #[must_use]
    pub fn new(function: Function, thickness: Length, material: MaterialId, wraps: bool) -> Self {
        Self {
            function,
            thickness,
            material,
            wraps,
        }
    }
}

/// What happens where two layers of two joined walls meet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Join {
    /// The first runs through the join and cuts the second.
    FirstCuts,
    /// The second runs through the join and cuts the first.
    SecondCuts,
    /// A tie: neither cuts; they meet on the join line.
    Meet {
        /// Whether they are of one material, and so drawn as one with no line between.
        drawn_as_one: bool,
    },
}

/// The join rule for two layers of two walls that meet: the stronger priority cuts the weaker;
/// on a tie neither cuts, and layers of one material are drawn as one.
#[must_use]
pub fn join(first: &Layer, second: &Layer) -> Join {
    match first.function.rank().cmp(&second.function.rank()) {
        Ordering::Less => Join::FirstCuts,
        Ordering::Greater => Join::SecondCuts,
        Ordering::Equal => Join::Meet {
            drawn_as_one: first.material == second.material,
        },
    }
}
