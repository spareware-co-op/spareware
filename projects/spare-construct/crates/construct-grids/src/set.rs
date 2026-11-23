//! A document's grid lines and reference planes together: the rules that need to see them all.
//!
//! - A grid's name is unique among grids, and a reference plane's among reference planes (when it
//!   has one), compared without regard to case ([`crate::same_name`]). The two are separate:
//!   grid `A` and a plane named `A` can both exist.
//! - An ID is a grid's or a plane's, never both.
//! - Reading a document ([`Datums::from_records`]) is strict about each record and does not
//!   refuse two of the same name: rules bind what commands write, and a document that somehow
//!   holds two keeps working; [`Datums::check_names`] says so.

use crate::GridError;
use crate::cross::{Crossing, crossings, nearest_on};
use crate::datum::{Grid, PlaneShape, ReferencePlane};
use crate::name::{clean_name, same_name, suggest};
use crate::record::{GRID_KIND, PLANE_KIND};
use crate::shape::{Point, Shape};
use construct_model::element::Element;
use construct_params::Length;
use kit_core::Id;
use std::collections::BTreeMap;

/// The grid line nearest to a point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nearest {
    /// The grid's ID.
    pub id: Id,
    /// Its name.
    pub name: String,
    /// The point on the grid nearest to the one asked about.
    pub foot: Point,
    /// How far that is from the point asked about, the nearest step.
    pub distance: Length,
}

/// The grid lines and reference planes of one document, by ID.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Datums {
    grids: BTreeMap<Id, Grid>,
    planes: BTreeMap<Id, ReferencePlane>,
}

impl Datums {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads every grid and reference plane out of a document's records, by their kinds; any other
    /// record is not this crate's and is passed over.
    ///
    /// # Errors
    ///
    /// The first record of these kinds that is not one as written ([`Grid::from_record`],
    /// [`ReferencePlane::from_record`]), or whose ID another has.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a Element>,
    ) -> Result<Self, GridError> {
        let mut set = Self::new();
        for record in records {
            if record.kind == GRID_KIND {
                let grid = Grid::from_record(record)?;
                set.free_id(grid.id())?;
                set.grids.insert(grid.id().clone(), grid);
            } else if record.kind == PLANE_KIND {
                let plane = ReferencePlane::from_record(record)?;
                set.free_id(plane.id())?;
                set.planes.insert(plane.id().clone(), plane);
            }
        }
        Ok(set)
    }

    /// Every grid and reference plane as records in `document`, grids first, each in ID order.
    #[must_use]
    pub fn to_records(&self, document: &Id) -> Vec<Element> {
        let grids = self.grids.values().map(|g| g.to_record(document));
        let planes = self.planes.values().map(|p| p.to_record(document));
        grids.chain(planes).collect()
    }

    fn free_id(&self, id: &Id) -> Result<(), GridError> {
        if self.grids.contains_key(id) || self.planes.contains_key(id) {
            return Err(GridError::DuplicateId(id.clone()));
        }
        Ok(())
    }

    /// Every grid, by name (case aside) and then ID.
    #[must_use]
    pub fn grids(&self) -> Vec<&Grid> {
        let mut all: Vec<&Grid> = self.grids.values().collect();
        all.sort_by_key(|g| (g.name().to_lowercase(), g.id().clone()));
        all
    }

    /// Every reference plane, named ones by name (case aside) first, then the unnamed, each by ID.
    #[must_use]
    pub fn planes(&self) -> Vec<&ReferencePlane> {
        let mut all: Vec<&ReferencePlane> = self.planes.values().collect();
        all.sort_by_key(|p| {
            (
                p.name().is_none(),
                p.name().map(str::to_lowercase),
                p.id().clone(),
            )
        });
        all
    }

    /// The grid with an ID.
    #[must_use]
    pub fn grid(&self, id: &Id) -> Option<&Grid> {
        self.grids.get(id)
    }

    /// The reference plane with an ID.
    #[must_use]
    pub fn plane(&self, id: &Id) -> Option<&ReferencePlane> {
        self.planes.get(id)
    }

    /// The grid of that name, case aside.
    #[must_use]
    pub fn grid_named(&self, name: &str) -> Option<&Grid> {
        self.grids().into_iter().find(|g| same_name(g.name(), name))
    }

    /// The reference plane of that name, case aside: how families and sketches will find a plane
    /// to snap to.
    #[must_use]
    pub fn plane_named(&self, name: &str) -> Option<&ReferencePlane> {
        self.planes()
            .into_iter()
            .find(|p| p.name().is_some_and(|n| same_name(n, name)))
    }

    fn grid_name_free(&self, name: &str, except: Option<&Id>) -> Result<(), GridError> {
        let name = clean_name(name)?;
        let clash = self
            .grids
            .values()
            .any(|g| Some(g.id()) != except && same_name(g.name(), &name));
        if clash {
            return Err(GridError::DuplicateGridName(name));
        }
        Ok(())
    }

    fn plane_name_free(&self, name: Option<&str>, except: Option<&Id>) -> Result<(), GridError> {
        let Some(name) = name else { return Ok(()) };
        let name = clean_name(name)?;
        let clash = self
            .planes
            .values()
            .any(|p| Some(p.id()) != except && p.name().is_some_and(|n| same_name(n, &name)));
        if clash {
            return Err(GridError::DuplicatePlaneName(name));
        }
        Ok(())
    }

    /// Whether no two grids share a name and no two planes do.
    ///
    /// # Errors
    ///
    /// The first clash found.
    pub fn check_names(&self) -> Result<(), GridError> {
        for grid in self.grids.values() {
            self.grid_name_free(grid.name(), Some(grid.id()))?;
        }
        for plane in self.planes.values() {
            self.plane_name_free(plane.name(), Some(plane.id()))?;
        }
        Ok(())
    }

    /// The name to offer for the next grid: the one after `after` ([`crate::next_name`]), and on
    /// from there while it is taken; `1` with no `after`.
    #[must_use]
    pub fn suggest_grid_name(&self, after: Option<&str>) -> String {
        suggest(after, |name| self.grid_named(name).is_some())
    }

    /// Adds a grid.
    ///
    /// # Errors
    ///
    /// [`GridError::DuplicateId`] or [`GridError::DuplicateGridName`]. Nothing changes.
    pub fn add_grid(&mut self, grid: Grid) -> Result<(), GridError> {
        self.free_id(grid.id())?;
        self.grid_name_free(grid.name(), None)?;
        self.grids.insert(grid.id().clone(), grid);
        Ok(())
    }

    /// Renames a grid.
    ///
    /// # Errors
    ///
    /// [`GridError::UnknownGrid`], a name refusal, or [`GridError::DuplicateGridName`].
    pub fn rename_grid(&mut self, id: &Id, name: &str) -> Result<(), GridError> {
        let grid = self
            .grids
            .get(id)
            .ok_or_else(|| GridError::UnknownGrid(id.clone()))?;
        let renamed = grid.renamed(name)?;
        self.grid_name_free(renamed.name(), Some(id))?;
        self.grids.insert(id.clone(), renamed);
        Ok(())
    }

    /// Gives a grid another shape.
    ///
    /// # Errors
    ///
    /// [`GridError::UnknownGrid`] or the shape's refusal.
    pub fn reshape_grid(&mut self, id: &Id, shape: Shape) -> Result<(), GridError> {
        let grid = self
            .grids
            .get(id)
            .ok_or_else(|| GridError::UnknownGrid(id.clone()))?;
        let reshaped = grid.reshaped(shape)?;
        self.grids.insert(id.clone(), reshaped);
        Ok(())
    }

    /// Takes a grid out and gives it back.
    ///
    /// # Errors
    ///
    /// [`GridError::UnknownGrid`].
    pub fn remove_grid(&mut self, id: &Id) -> Result<Grid, GridError> {
        self.grids
            .remove(id)
            .ok_or_else(|| GridError::UnknownGrid(id.clone()))
    }

    /// Adds a reference plane.
    ///
    /// # Errors
    ///
    /// [`GridError::DuplicateId`] or [`GridError::DuplicatePlaneName`]. Nothing changes.
    pub fn add_plane(&mut self, plane: ReferencePlane) -> Result<(), GridError> {
        self.free_id(plane.id())?;
        self.plane_name_free(plane.name(), None)?;
        self.planes.insert(plane.id().clone(), plane);
        Ok(())
    }

    /// Renames a reference plane, or takes its name away (`None`).
    ///
    /// # Errors
    ///
    /// [`GridError::UnknownPlane`], a name refusal, or [`GridError::DuplicatePlaneName`].
    pub fn rename_plane(&mut self, id: &Id, name: Option<&str>) -> Result<(), GridError> {
        let plane = self
            .planes
            .get(id)
            .ok_or_else(|| GridError::UnknownPlane(id.clone()))?;
        let renamed = plane.renamed(name)?;
        self.plane_name_free(renamed.name(), Some(id))?;
        self.planes.insert(id.clone(), renamed);
        Ok(())
    }

    /// Gives a reference plane another shape.
    ///
    /// # Errors
    ///
    /// [`GridError::UnknownPlane`] or the shape's refusal.
    pub fn reshape_plane(&mut self, id: &Id, shape: PlaneShape) -> Result<(), GridError> {
        let plane = self
            .planes
            .get(id)
            .ok_or_else(|| GridError::UnknownPlane(id.clone()))?;
        let reshaped = plane.reshaped(shape)?;
        self.planes.insert(id.clone(), reshaped);
        Ok(())
    }

    /// Takes a reference plane out and gives it back.
    ///
    /// # Errors
    ///
    /// [`GridError::UnknownPlane`].
    pub fn remove_plane(&mut self, id: &Id) -> Result<ReferencePlane, GridError> {
        self.planes
            .remove(id)
            .ok_or_else(|| GridError::UnknownPlane(id.clone()))
    }

    /// Where two grids cross ([`crossings`]).
    ///
    /// # Errors
    ///
    /// [`GridError::UnknownGrid`].
    pub fn crossings_of(&self, a: &Id, b: &Id) -> Result<Vec<Crossing>, GridError> {
        let one = self
            .grids
            .get(a)
            .ok_or_else(|| GridError::UnknownGrid(a.clone()))?;
        let other = self
            .grids
            .get(b)
            .ok_or_else(|| GridError::UnknownGrid(b.clone()))?;
        Ok(crossings(one.shape(), other.shape()))
    }

    /// The grid nearest to a point, with the point of it nearest: by distance, then by name (case
    /// aside), then by ID. `None` if there are no grids.
    #[must_use]
    pub fn nearest_grid(&self, point: Point) -> Option<Nearest> {
        let mut best: Option<(i128, String, &Grid, Point, Length)> = None;
        for grid in self.grids() {
            let Some(foot) = nearest_on(grid.shape(), point) else {
                continue;
            };
            let key = (foot.squared, grid.name().to_lowercase());
            let better = best
                .as_ref()
                .is_none_or(|(squared, name, ..)| key < (*squared, name.clone()));
            if better {
                best = Some((foot.squared, key.1, grid, foot.point, foot.distance));
            }
        }
        best.map(|(_, _, grid, foot, distance)| Nearest {
            id: grid.id().clone(),
            name: grid.name().to_owned(),
            foot,
            distance,
        })
    }
}
