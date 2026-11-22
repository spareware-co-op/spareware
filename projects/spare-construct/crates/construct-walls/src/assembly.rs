//! A wall type's layers and its core, checked whole, and the pure edits that give a new one.
//!
//! The core is set by two boundaries. A boundary is a count of layers: the number that lie on
//! its exterior side. With `n` layers, `0 <= exterior < interior <= n`; layers `0..exterior` are
//! the exterior shell, `exterior..interior` the core, and `interior..n` the interior shell. So
//! there is always exactly one core and it is contiguous; it must hold a layer thicker than 0.
//!
//! Every edit returns a new [`Assembly`] that passes every check, or the refusal; the one it was
//! called on never changes. How each edit moves the boundaries:
//! - insert at `at`: the new layer goes before the layer now at `at` (at `n`, after the last). A
//!   layer inserted at a boundary goes into the shell beside it, never into the core; to put one
//!   in the core, insert it between two core layers or move a boundary after.
//! - remove at `at`: the boundaries close up round the gap.
//! - move from `from` to `to`: the layer is taken out and put back so it ends at index `to`; the
//!   boundaries stay where they are, so a layer's zone is the zone of the place it lands in.
//! - change at `at`: the layer is replaced; the boundaries stay.
//! - move a boundary: it is set to the count given.
//!
//! Depths are measured from the exterior finish face toward the interior. An offset from the
//! reference line is the depth of the location line on it less the depth of the face, so it is
//! positive toward the exterior side; a flipped wall negates every offset.

use crate::error::WallError;
use crate::layer::Layer;
use crate::length::{ZERO, difference, half, negated, sum};
use crate::offsets::{LocationLine, Offsets};
use construct_materials::Materials;
use construct_params::Length;

/// The most a wall type's layers may add up to: 10 m. No real wall comes near it, and it keeps
/// every depth and offset well inside the range of a length.
pub const MOST_TOTAL: Length = Length::from_mm(10_000);

/// One of the two core boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// The boundary between the exterior shell and the core.
    Exterior,
    /// The boundary between the core and the interior shell.
    Interior,
}

impl Side {
    /// Its word in messages.
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Exterior => "exterior",
            Self::Interior => "interior",
        }
    }
}

/// Which part of the wall a layer is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Zone {
    /// Between the exterior finish face and the core.
    ExteriorShell,
    /// The core.
    Core,
    /// Between the core and the interior finish face.
    InteriorShell,
}

/// A wall type's layers in order from the exterior face to the interior face, and its core.
/// Every value of this type has passed every check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assembly {
    layers: Vec<Layer>,
    exterior: usize,
    interior: usize,
    /// The depth of every layer face from the exterior finish face: `n + 1` of them.
    depths: Vec<Length>,
}

impl Assembly {
    /// The layers from the exterior face to the interior face, with the core boundaries.
    ///
    /// # Errors
    /// Checked in this order, the first broken rule refused: [`WallError::NoLayers`]; then layer
    /// by layer from the exterior, [`WallError::NegativeThickness`],
    /// [`WallError::ZeroThickness`] (a layer other than a membrane at 0) and
    /// [`WallError::TooThick`]; [`WallError::BoundaryOutOfRange`] (exterior first);
    /// [`WallError::EmptyCore`]; [`WallError::CoreWithoutThickness`];
    /// [`WallError::CoreLayerWraps`] for the first core layer that wraps.
    pub fn new(layers: Vec<Layer>, exterior: usize, interior: usize) -> Result<Self, WallError> {
        if layers.is_empty() {
            return Err(WallError::NoLayers);
        }
        let mut depths = Vec::with_capacity(layers.len().saturating_add(1));
        let mut depth = ZERO;
        depths.push(depth);
        for (at, layer) in layers.iter().enumerate() {
            if layer.thickness < ZERO {
                return Err(WallError::NegativeThickness { layer: at });
            }
            if layer.thickness == ZERO && !layer.function.may_be_zero() {
                return Err(WallError::ZeroThickness { layer: at });
            }
            depth = sum(depth, layer.thickness)
                .filter(|total| *total <= MOST_TOTAL)
                .ok_or(WallError::TooThick { layer: at })?;
            depths.push(depth);
        }
        let count = layers.len();
        for (side, at) in [(Side::Exterior, exterior), (Side::Interior, interior)] {
            if at > count {
                return Err(WallError::BoundaryOutOfRange {
                    side,
                    at,
                    layers: count,
                });
            }
        }
        if exterior >= interior {
            return Err(WallError::EmptyCore { exterior, interior });
        }
        let core = layers.get(exterior..interior).unwrap_or_default();
        if core.iter().all(|layer| layer.thickness == ZERO) {
            return Err(WallError::CoreWithoutThickness {
                first: exterior,
                last: interior.saturating_sub(1),
            });
        }
        if let Some(at) = core.iter().position(|layer| layer.wraps) {
            return Err(WallError::CoreLayerWraps {
                layer: exterior.saturating_add(at),
            });
        }
        Ok(Self {
            layers,
            exterior,
            interior,
            depths,
        })
    }

    /// The layers, from the exterior face to the interior face.
    #[must_use]
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// The layer at index `at` from the exterior face.
    #[must_use]
    pub fn layer(&self, at: usize) -> Option<&Layer> {
        self.layers.get(at)
    }

    /// A core boundary: the count of layers on its exterior side.
    #[must_use]
    pub fn boundary(&self, side: Side) -> usize {
        match side {
            Side::Exterior => self.exterior,
            Side::Interior => self.interior,
        }
    }

    /// The zone of the layer at `at`, if there is one.
    #[must_use]
    pub fn zone(&self, at: usize) -> Option<Zone> {
        if at >= self.layers.len() {
            None
        } else if at < self.exterior {
            Some(Zone::ExteriorShell)
        } else if at < self.interior {
            Some(Zone::Core)
        } else {
            Some(Zone::InteriorShell)
        }
    }

    /// Whether the layer at `at` is in the core.
    #[must_use]
    pub fn in_core(&self, at: usize) -> bool {
        self.zone(at) == Some(Zone::Core)
    }

    /// The indices of the layers that wrap at free ends and opening sides, ascending.
    #[must_use]
    pub fn wrapping(&self) -> Vec<usize> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.wraps)
            .map(|(at, _)| at)
            .collect()
    }

    /// The indices of the layers whose material `materials` does not hold, ascending.
    #[must_use]
    pub fn missing_materials(&self, materials: &Materials) -> Vec<usize> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| materials.get(&layer.material).is_none())
            .map(|(at, _)| at)
            .collect()
    }

    /// The whole wall's thickness.
    #[must_use]
    pub fn total(&self) -> Length {
        self.face_depth(self.layers.len())
    }

    /// The depth of every layer face from the exterior finish face: `n + 1` faces, the first 0
    /// and the last the total.
    #[must_use]
    pub fn face_depths(&self) -> &[Length] {
        &self.depths
    }

    /// The depth of face `face` (`0..=n`).
    fn face_depth(&self, face: usize) -> Length {
        self.depths.get(face).copied().unwrap_or(ZERO)
    }

    /// The depth of a location line from the exterior finish face. A centreline is half the sum
    /// of two depths; `None` if the length unit cannot hold that half.
    #[must_use]
    pub fn depth(&self, line: LocationLine) -> Option<Length> {
        let core_exterior = self.face_depth(self.exterior);
        let core_interior = self.face_depth(self.interior);
        match line {
            LocationLine::WallCentre => half(self.total()),
            LocationLine::CoreCentre => sum(core_exterior, core_interior).and_then(half),
            LocationLine::ExteriorFinishFace => Some(ZERO),
            LocationLine::InteriorFinishFace => Some(self.total()),
            LocationLine::ExteriorCoreFace => Some(core_exterior),
            LocationLine::InteriorCoreFace => Some(core_interior),
        }
    }

    /// Where every layer face and every location line lies from the wall's reference line, when
    /// location line `on` lies on it. An offset is positive toward the wall's exterior side;
    /// `flipped` puts the exterior on the other side and so negates every offset.
    ///
    /// # Errors
    /// [`WallError::HalfNotExact`] if `on` is a centreline whose half the length unit cannot hold.
    pub fn offsets(&self, on: LocationLine, flipped: bool) -> Result<Offsets, WallError> {
        let reference = self.depth(on).ok_or(WallError::HalfNotExact { line: on })?;
        let offset = |depth: Length| {
            let toward_exterior = difference(reference, depth)?;
            if flipped {
                negated(toward_exterior)
            } else {
                Some(toward_exterior)
            }
        };
        // Depths are at most MOST_TOTAL, so no difference or negation falls outside a length;
        // were one to, it would be refused as too thick at the layer before that face.
        let faces = self
            .depths
            .iter()
            .enumerate()
            .map(|(face, depth)| {
                offset(*depth).ok_or(WallError::TooThick {
                    layer: face.saturating_sub(1),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let lines = LocationLine::ALL.map(|line| self.depth(line).and_then(offset));
        Ok(Offsets::new(faces, lines))
    }

    /// The same type with `layer` inserted at `at` (`0..=n`).
    ///
    /// # Errors
    /// [`WallError::NoSuchLayer`] if `at` is past `n`, or any refusal of [`Assembly::new`].
    pub fn with_inserted(&self, at: usize, layer: Layer) -> Result<Self, WallError> {
        let count = self.layers.len();
        if at > count {
            return Err(WallError::NoSuchLayer {
                layer: at,
                layers: count,
            });
        }
        let mut layers = self.layers.clone();
        layers.insert(at, layer);
        let shift = |boundary: usize, moves: bool| {
            if moves {
                boundary.saturating_add(1)
            } else {
                boundary
            }
        };
        Self::new(
            layers,
            shift(self.exterior, self.exterior >= at),
            shift(self.interior, self.interior > at),
        )
    }

    /// The same type without the layer at `at`.
    ///
    /// # Errors
    /// [`WallError::NoSuchLayer`], or any refusal of [`Assembly::new`] (removing the last core
    /// layer gives [`WallError::EmptyCore`]).
    pub fn without_layer(&self, at: usize) -> Result<Self, WallError> {
        self.require(at)?;
        let mut layers = self.layers.clone();
        layers.remove(at);
        let close = |boundary: usize| {
            if boundary > at {
                boundary.saturating_sub(1)
            } else {
                boundary
            }
        };
        Self::new(layers, close(self.exterior), close(self.interior))
    }

    /// The same type with the layer at `from` moved so it lies at index `to`; the boundaries
    /// stay where they are.
    ///
    /// # Errors
    /// [`WallError::NoSuchLayer`] for either index, or any refusal of [`Assembly::new`].
    pub fn with_moved(&self, from: usize, to: usize) -> Result<Self, WallError> {
        self.require(from)?;
        self.require(to)?;
        let mut layers = self.layers.clone();
        let layer = layers.remove(from);
        layers.insert(to, layer);
        Self::new(layers, self.exterior, self.interior)
    }

    /// The same type with the layer at `at` replaced by `layer`.
    ///
    /// # Errors
    /// [`WallError::NoSuchLayer`], or any refusal of [`Assembly::new`].
    pub fn with_changed(&self, at: usize, layer: Layer) -> Result<Self, WallError> {
        self.require(at)?;
        let mut layers = self.layers.clone();
        if let Some(slot) = layers.get_mut(at) {
            *slot = layer;
        }
        Self::new(layers, self.exterior, self.interior)
    }

    /// The same type with one core boundary set to `at`, a count of layers on its exterior side.
    ///
    /// # Errors
    /// Any refusal of [`Assembly::new`].
    pub fn with_boundary(&self, side: Side, at: usize) -> Result<Self, WallError> {
        let (exterior, interior) = match side {
            Side::Exterior => (at, self.interior),
            Side::Interior => (self.exterior, at),
        };
        Self::new(self.layers.clone(), exterior, interior)
    }

    /// Refuses an index with no layer.
    fn require(&self, at: usize) -> Result<(), WallError> {
        if at < self.layers.len() {
            Ok(())
        } else {
            Err(WallError::NoSuchLayer {
                layer: at,
                layers: self.layers.len(),
            })
        }
    }
}
