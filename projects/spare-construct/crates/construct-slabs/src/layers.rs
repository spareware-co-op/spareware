//! A floor type's layers, from the top face down, what each layer's function means, and the rule
//! for where a floor sits on its level.
//!
//! A floor is read from the top down: the finish people walk on (a screed, a topping, a board),
//! sometimes a sheet under it (a damp-proof membrane), the part that carries the load (a concrete
//! slab, a timber or steel frame of joists), and sometimes insulation or a lining below. The five
//! functions are a wall layer's (construct-walls' `Function`), word for word and priority for
//! priority, so where a floor meets a wall each pair of layers is settled by one join rule; the
//! two lists are kept equal by a test until they share one crate (the decision note,
//! notes/CON-A4-F4-P1-floor-from-boundary.md).
//!
//! **Where a floor sits.** The floor's top face sits at its level's elevation plus its own offset
//! ([`Layers::face_heights`]), and its layers hang below in order, so a floor's shape has `z = 0`
//! at its top face and `z = -T` at its bottom, `T` its total thickness. A floor whose structure's
//! top, not its finish's, sits at the level is a later version of the type.

use crate::error::SlabError;
use construct_materials::{MaterialId, Materials};
use construct_params::Length;
use serde::{Deserialize, Serialize};

/// The most a floor type's layers may add up to: 10 m, as a wall's. No real floor comes near it,
/// and it keeps every depth well inside the range of a length.
pub const MOST_TOTAL: Length = Length::from_mm(10_000);

/// No length.
const ZERO: Length = Length::from_steps(0);

/// What a layer does in the floor: a wall layer's five functions. The stored names are the
/// variant names in snake case.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Function {
    /// What carries the load: a concrete slab, a frame of joists.
    Structure,
    /// A board or sheet a finish is laid on: sheet flooring, plywood.
    Substrate,
    /// Insulation, or an air space.
    Thermal,
    /// The face people see: a screed, a topping, a board, a ceiling lining below.
    Finish,
    /// A sheet that keeps water, air or vapour out: a damp-proof membrane. It may be 0 thick.
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

    /// Its join priority: 1 is the strongest; a membrane has none. A wall layer's, the same.
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
}

/// One layer: what it does, how thick it is and what it is made of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FloorLayer {
    /// What it does.
    pub function: Function,
    /// Its thickness, never below 0, and 0 only for a membrane.
    pub thickness: Length,
    /// What it is made of: a material in the project's materials (construct-materials).
    pub material: MaterialId,
}

impl FloorLayer {
    /// A layer.
    #[must_use]
    pub fn new(function: Function, thickness: Length, material: MaterialId) -> Self {
        Self {
            function,
            thickness,
            material,
        }
    }
}

/// A floor type's layers from the top face down. Every value of this type has passed every check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layers {
    layers: Vec<FloorLayer>,
    /// The depth of every layer face below the top face: `n + 1` of them, 0 first.
    depths: Vec<Length>,
}

impl Layers {
    /// The layers from the top face down.
    ///
    /// # Errors
    /// Checked in this order, the first broken rule refused: [`SlabError::NoLayers`]; then layer
    /// by layer from the top, [`SlabError::NegativeThickness`], [`SlabError::ZeroThickness`] (a
    /// layer other than a membrane at 0) and [`SlabError::TooThick`]; then
    /// [`SlabError::NoThickness`] when every layer is 0 thick.
    pub fn new(layers: Vec<FloorLayer>) -> Result<Self, SlabError> {
        if layers.is_empty() {
            return Err(SlabError::NoLayers);
        }
        let mut depths = Vec::with_capacity(layers.len().saturating_add(1));
        let mut depth = ZERO;
        depths.push(depth);
        for (at, layer) in layers.iter().enumerate() {
            if layer.thickness < ZERO {
                return Err(SlabError::NegativeThickness { layer: at });
            }
            if layer.thickness == ZERO && !layer.function.may_be_zero() {
                return Err(SlabError::ZeroThickness { layer: at });
            }
            depth = depth
                .checked_add(layer.thickness)
                .filter(|total| *total <= MOST_TOTAL)
                .ok_or(SlabError::TooThick { layer: at })?;
            depths.push(depth);
        }
        if depth == ZERO {
            return Err(SlabError::NoThickness);
        }
        Ok(Self { layers, depths })
    }

    /// The layers, from the top face down.
    #[must_use]
    pub fn layers(&self) -> &[FloorLayer] {
        &self.layers
    }

    /// The layer at `at` from the top, if there is one.
    #[must_use]
    pub fn layer(&self, at: usize) -> Option<&FloorLayer> {
        self.layers.get(at)
    }

    /// The total thickness.
    #[must_use]
    pub fn total(&self) -> Length {
        self.depths.last().copied().unwrap_or(ZERO)
    }

    /// The depth of every layer face below the top face, from the top face (0) to the bottom
    /// face (the total): face `k` is the top of layer `k` and the bottom of layer `k - 1`.
    #[must_use]
    pub fn depths(&self) -> &[Length] {
        &self.depths
    }

    /// The height of every layer face when the floor's top face stands at `top`: its level's
    /// elevation plus its offset (construct-levels' `height_of`). The level rule in one place.
    ///
    /// # Errors
    /// [`SlabError::OutOfRange`] when a height does not fit a length.
    pub fn face_heights(&self, top: Length) -> Result<Vec<Length>, SlabError> {
        self.depths
            .iter()
            .map(|depth| top.checked_sub(*depth).ok_or(SlabError::OutOfRange))
            .collect()
    }

    /// The layers whose material `library` does not hold, by index from the top.
    #[must_use]
    pub fn missing_materials(&self, library: &Materials) -> Vec<usize> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| library.get(&layer.material).is_none())
            .map(|(at, _)| at)
            .collect()
    }
}
