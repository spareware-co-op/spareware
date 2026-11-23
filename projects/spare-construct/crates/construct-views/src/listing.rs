//! What a view shows: a pure function of the view and the model's elements.
//!
//! An element is shown when, in this order, the view draws its kind of category, the view's phase
//! rule lets it through ([`construct_phases::visibility`], unchanged) and it lies in the view's
//! reach. The reach and the way it is shown depend on the kind of view:
//!
//! - **Floor plan.** Between the view depth and the top. An element the cut plane crosses (its
//!   lowest point at or below the plane, its highest at or above) is [`How::Cut`]; one wholly
//!   below the plane is [`How::Seen`] down to the bottom and [`How::Beyond`] below it, to the
//!   view depth; one wholly above the plane is not shown.
//! - **Ceiling plan.** Between the cut plane and the top, looking up: an element the plane crosses
//!   is cut, one wholly above it up to the top is seen, one wholly below is not shown.
//! - **Section and elevation.** Within the region the line and its depth make, at every height.
//!   An element the line crosses is cut; one wholly in front of it, within the depth and within the
//!   line's two ends, is seen. The region's far corners are exact to one grid step.
//! - **Three-dimensional.** Everything the category and phase rules let through, seen.
//! - **Drafting and legend.** Nothing: they hold their own drawing, not the model.
//!
//! Which categories a view draws: the model's (kind model) and the imported ones (a linked model,
//! a drawing). Datums, annotation, other views and analysis stand-ins are not elements a view
//! draws from the model: datums and annotation have drawing rules of their own (the next jobs).
//!
//! Every box is the element's bounds in the project's frame, in whole millimetres, as construct-model
//! stores them. They are compared in grid steps with the view's heights and line (construct-params'
//! lengths), so no length is rounded to whole millimetres to answer. construct-spatial finds the
//! candidates in the view's box; the exact test judges each.

use crate::error::ViewError;
use crate::shape::{Rect, bounding, ceil_mm, floor_mm, meets, mm_to_steps, region};
use crate::view::View;
use construct_categories::{CategoryId, CategoryKind, CategoryTable};
use construct_levels::Levels;
use construct_model::geometry::Bounds;
use construct_phases::{DesignOptions, ElementStaging, PhaseList, Visibility, visibility};
use construct_spatial::{Entry, MAX_COORDINATE, SpatialIndex};
use kit_core::Id;
use std::collections::{BTreeMap, BTreeSet};

/// The parts of a model a view's question reads: its levels, categories, phases and design
/// options. All four are the document's own.
#[derive(Clone, Copy, Debug)]
pub struct Model<'a> {
    /// The document's levels.
    pub levels: &'a Levels,
    /// The category table.
    pub categories: &'a CategoryTable,
    /// The document's phases.
    pub phases: &'a PhaseList,
    /// The document's design options.
    pub options: &'a DesignOptions,
}

/// What a view needs to know of one element: its ID, its category, its box and its staging.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewElement {
    /// The element's permanent ID.
    pub id: Id,
    /// Its category (a subcategory is read as the category it is a part of, by its kind).
    pub category: CategoryId,
    /// Its box in the project's frame, in whole millimetres, as regeneration stores it.
    pub bounds: Bounds,
    /// Its phases and design option.
    pub staging: ElementStaging,
}

/// How a view shows an element it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum How {
    /// The view's cut plane or line crosses it.
    Cut,
    /// It is in the view beyond the cut, or in a three-dimensional view.
    Seen,
    /// A plan's element below the bottom, down to the view depth.
    Beyond,
}

/// One element a view shows, and how.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    /// The element's ID.
    pub id: Id,
    /// How it is shown.
    pub how: How,
}

/// What a view shows: the elements, sorted by ID.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    shown: Vec<Shown>,
}

impl Listing {
    /// Every element shown, sorted by ID.
    #[must_use]
    pub fn shown(&self) -> &[Shown] {
        &self.shown
    }

    /// How many elements are shown.
    #[must_use]
    pub fn len(&self) -> usize {
        self.shown.len()
    }

    /// Whether nothing is shown.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shown.is_empty()
    }

    /// How an element is shown, or `None` if the view does not show it.
    #[must_use]
    pub fn how(&self, id: &Id) -> Option<How> {
        self.shown
            .binary_search_by(|shown| shown.id.cmp(id))
            .ok()
            .and_then(|index| self.shown.get(index))
            .map(|shown| shown.how)
    }

    fn with(&self, how: How) -> Vec<&Id> {
        self.shown
            .iter()
            .filter(|shown| shown.how == how)
            .map(|shown| &shown.id)
            .collect()
    }

    /// The IDs of the elements cut, sorted.
    #[must_use]
    pub fn cut(&self) -> Vec<&Id> {
        self.with(How::Cut)
    }

    /// The IDs of the elements seen, sorted.
    #[must_use]
    pub fn seen(&self) -> Vec<&Id> {
        self.with(How::Seen)
    }

    /// The IDs of the elements beyond, sorted.
    #[must_use]
    pub fn beyond(&self) -> Vec<&Id> {
        self.with(How::Beyond)
    }
}

/// Whether a view draws elements of this kind of category.
fn drawn(kind: CategoryKind) -> bool {
    matches!(kind, CategoryKind::Model | CategoryKind::Import)
}

/// The elements `view` shows, from `elements`, and how.
///
/// # Errors
/// [`ViewError::DuplicateElement`] if two elements share an ID; [`ViewError::UnknownCategory`] for
/// a category the table lacks; every refusal of [`View::check`] about the view; [`ViewError::Phase`]
/// for a staging the model's phases or options do not hold; [`ViewError::Spatial`] for a box the
/// index cannot hold; [`ViewError::OutOfRange`] for a view and an element too far apart to test
/// exactly.
pub fn list(
    view: &View,
    model: &Model<'_>,
    elements: &[ViewElement],
) -> Result<Listing, ViewError> {
    view.check(model)?;
    let mut ids = BTreeSet::new();
    for element in elements {
        if !ids.insert(&element.id) {
            return Err(ViewError::DuplicateElement(element.id.clone()));
        }
    }
    if !view.kind().shows_model() {
        return Ok(Listing::default());
    }
    let eligible = eligible(view, model, elements)?;
    let mut shown = match region_of(view, model)? {
        Region::Everything => eligible
            .iter()
            .map(|element| Shown {
                id: element.id.clone(),
                how: How::Seen,
            })
            .collect(),
        region => classify(&region, &eligible)?,
    };
    shown.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Listing { shown })
}

/// The elements whose category the view draws and which its phase rule lets through.
fn eligible<'a>(
    view: &View,
    model: &Model<'_>,
    elements: &'a [ViewElement],
) -> Result<Vec<&'a ViewElement>, ViewError> {
    let mut kept = Vec::new();
    for element in elements {
        let category = model
            .categories
            .get(element.category)
            .ok_or(ViewError::UnknownCategory(element.category))?;
        if !drawn(category.kind) {
            continue;
        }
        if let Some(settings) = view.phasing() {
            if visibility(model.phases, model.options, settings, &element.staging)?
                == Visibility::Hidden
            {
                continue;
            }
        } else {
            element.staging.check(model.phases, model.options)?;
        }
        kept.push(element);
    }
    Ok(kept)
}

/// The part of space a view reaches, in grid steps.
enum Region {
    /// A three-dimensional view: no limit.
    Everything,
    /// A floor plan: the heights of its range.
    Floor(crate::range::Heights),
    /// A ceiling plan: the heights of its range.
    Ceiling(crate::range::Heights),
    /// A section or an elevation: the line, and the quadrilateral it sees.
    Cut {
        line: [(i128, i128); 2],
        quad: [(i128, i128); 4],
    },
}

fn region_of(view: &View, model: &Model<'_>) -> Result<Region, ViewError> {
    use crate::kind::ViewKind;
    Ok(match view.kind() {
        ViewKind::FloorPlan(plan) => Region::Floor(plan.range.resolve(model.levels)?),
        ViewKind::CeilingPlan(plan) => Region::Ceiling(plan.range.resolve(model.levels)?),
        ViewKind::Section(line) | ViewKind::Elevation(line) => {
            let point = |p: crate::cut::Point| (i128::from(p.x.steps()), i128::from(p.y.steps()));
            let (from, to) = (point(line.from()), point(line.to()));
            let quad = region(from, to, line.look(), i128::from(line.far().steps()))?;
            Region::Cut {
                line: [from, to],
                quad,
            }
        }
        ViewKind::ThreeD | ViewKind::Drafting | ViewKind::Legend => Region::Everything,
    })
}

/// The box construct-spatial is asked for, in whole millimetres, rounded outward.
fn query_box(region: &Region) -> Bounds {
    let (limit, floor) = (i128::from(MAX_COORDINATE), i128::from(-MAX_COORDINATE));
    let clamp = |steps_low: i128, steps_high: i128| -> (i64, i64) {
        let low = floor_mm(steps_low).clamp(floor, limit);
        let high = ceil_mm(steps_high).clamp(floor, limit);
        (
            i64::try_from(low).unwrap_or(-MAX_COORDINATE),
            i64::try_from(high).unwrap_or(MAX_COORDINATE),
        )
    };
    let whole = (-MAX_COORDINATE, MAX_COORDINATE);
    let ((x_low, x_high), (y_low, y_high), (z_low, z_high)) = match region {
        Region::Everything => (whole, whole, whole),
        Region::Floor(heights) => (
            whole,
            whole,
            clamp(
                i128::from(heights.depth.steps()),
                i128::from(heights.top.steps()),
            ),
        ),
        Region::Ceiling(heights) => (
            whole,
            whole,
            clamp(
                i128::from(heights.cut.steps()),
                i128::from(heights.top.steps()),
            ),
        ),
        Region::Cut { quad, .. } => {
            let rect = bounding(quad);
            (
                clamp(rect.min.0, rect.max.0),
                clamp(rect.min.1, rect.max.1),
                whole,
            )
        }
    };
    Bounds {
        min: [x_low, y_low, z_low],
        max: [x_high, y_high, z_high],
    }
}

/// The elements of `eligible` in the region, each with how it is shown.
fn classify(region: &Region, eligible: &[&ViewElement]) -> Result<Vec<Shown>, ViewError> {
    let index = SpatialIndex::from_entries(eligible.iter().map(|element| Entry {
        id: element.id.clone(),
        bounds: element.bounds,
        scope: None,
    }))?;
    let by_id: BTreeMap<&Id, &ViewElement> = eligible
        .iter()
        .map(|element| (&element.id, *element))
        .collect();
    let mut shown = Vec::new();
    for id in index.meeting(&query_box(region), None)? {
        let Some(element) = by_id.get(id) else {
            continue;
        };
        if let Some(how) = how_shown(region, &element.bounds)? {
            shown.push(Shown {
                id: id.clone(),
                how,
            });
        }
    }
    Ok(shown)
}

/// How one box is shown in the region, or `None` if it is not in it.
fn how_shown(region: &Region, bounds: &Bounds) -> Result<Option<How>, ViewError> {
    let [x_low, y_low, z_low] = bounds.min;
    let [x_high, y_high, z_high] = bounds.max;
    let (z_low, z_high) = (mm_to_steps(z_low)?, mm_to_steps(z_high)?);
    Ok(match region {
        Region::Everything => Some(How::Seen),
        Region::Floor(heights) => {
            let (top, cut, bottom, depth) = (
                i128::from(heights.top.steps()),
                i128::from(heights.cut.steps()),
                i128::from(heights.bottom.steps()),
                i128::from(heights.depth.steps()),
            );
            if z_high < depth || z_low > top {
                None
            } else if z_low <= cut && cut <= z_high {
                Some(How::Cut)
            } else if z_high < cut {
                Some(if z_high >= bottom {
                    How::Seen
                } else {
                    How::Beyond
                })
            } else {
                None
            }
        }
        Region::Ceiling(heights) => {
            let (top, cut) = (
                i128::from(heights.top.steps()),
                i128::from(heights.cut.steps()),
            );
            if z_low <= cut && cut <= z_high {
                Some(How::Cut)
            } else if z_low > cut && z_low <= top {
                Some(How::Seen)
            } else {
                None
            }
        }
        Region::Cut { line, quad } => {
            let rect = Rect {
                min: (mm_to_steps(x_low)?, mm_to_steps(y_low)?),
                max: (mm_to_steps(x_high)?, mm_to_steps(y_high)?),
            };
            if meets(line, &rect)? {
                Some(How::Cut)
            } else if meets(quad, &rect)? {
                Some(How::Seen)
            } else {
                None
            }
        }
    })
}
