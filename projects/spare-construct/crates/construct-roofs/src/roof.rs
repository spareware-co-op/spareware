//! A roof as data: its footprint, what each edge of it does (how far the roof overhangs it and how
//! steeply, if at all, the roof rises from it), and the form those make.
//!
//! The footprint is a floor's boundary (construct-slabs' [`Boundary`], checked by its own code):
//! one closed loop of straight edges in plan, each edge with its permanent ID, and no holes here
//! ([`RoofError::Holes`]: an opening through a roof is job 4.2's). It is where the roof bears: the
//! roof's bottom face crosses every footprint edge at the roof's base, its level's elevation plus
//! its offset. Each edge has an [`EdgeSpec`]: its overhang, how far the roof runs on past the edge
//! in plan (0 or more), and its pitch, if the roof rises from it ([`Pitch`]).
//!
//! The form ([`Form`]) is worked out from which edges have a pitch, and nothing else is a roof
//! here, each refusal naming the edges and the job that lifts it:
//! - none: **flat**, on any footprint construct-slabs takes; an overhang only on a footprint
//!   whose every edge runs along the plan's x or y axis ([`RoofError::OverhangOffAxis`], job 6.2),
//!   the eaves outline then each edge moved out by its overhang;
//! - one: **mono-pitch**, rising from that edge across to the opposite one;
//! - two opposite edges at one slope: **gable**, rising from both to a ridge midway between them;
//!   at different slopes, refused ([`RoofError::UnevenGable`], job 6.2);
//! - two edges that meet at a corner, three, or all four: hips, refused ([`RoofError::Hips`], job
//!   6.2: "every edge slopes: hips come with job 6.2").
//!
//! A sloped roof's footprint is a rectangle whose edges run along the plan's x and y axes
//! ([`RoofError::NotARectangle`], job 6.2 for the rest), so every corner it makes is exact on the
//! kernel's grid; [`crate::shape`] says where each is.

use crate::error::RoofError;
use crate::pitch::Pitch;
use crate::shape::Plan;
use construct_params::{Length, Value};
use construct_slabs::{Boundary, Segment};
use construct_solver::Point;
use kit_core::Id;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;

/// No length.
const ZERO: Length = Length::from_steps(0);

/// What one footprint edge does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeSpec {
    /// How far the roof runs on past the edge in plan, at or above zero.
    pub overhang: Length,
    /// How steeply the roof rises from the edge; `None` when it does not.
    pub pitch: Option<Pitch>,
}

impl Default for EdgeSpec {
    /// No overhang and no pitch.
    fn default() -> Self {
        Self {
            overhang: ZERO,
            pitch: None,
        }
    }
}

/// A roof's form, worked out from which edges have a pitch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Form {
    /// No edge slopes.
    Flat,
    /// One edge slopes: the roof rises from it to the opposite edge.
    Mono {
        /// The edge it rises from: its eaves.
        eaves: Id,
    },
    /// Two opposite edges slope alike: the roof rises from both to a ridge midway.
    Gable {
        /// The two edges it rises from, in the footprint's order.
        eaves: [Id; 2],
    },
}

impl Form {
    /// The form's stored name: `flat`, `mono` or `gable`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Mono { .. } => "mono",
            Self::Gable { .. } => "gable",
        }
    }

    /// The edges the roof rises from, in the footprint's order: none, one or two.
    #[must_use]
    pub fn eaves(&self) -> Vec<Id> {
        match self {
            Self::Flat => Vec::new(),
            Self::Mono { eaves } => vec![eaves.clone()],
            Self::Gable { eaves } => eaves.to_vec(),
        }
    }
}

/// A roof: its footprint and its edges. Every value of this type has passed every check, so its
/// shape can be worked out for any layers ([`crate::shape`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roof {
    footprint: Boundary,
    edges: Vec<(Id, EdgeSpec)>,
    form: Form,
}

impl Roof {
    /// A roof on `footprint`, each of its edges as `edges` gives it (an edge it does not name has
    /// no overhang and no pitch), checked as the module header says.
    ///
    /// # Errors
    /// In this order: [`RoofError::Holes`]; [`RoofError::UnknownEdge`] for the first edge (by ID)
    /// the footprint lacks; then edge by edge in the footprint's order,
    /// [`RoofError::NegativeOverhang`] and the pitch's refusals ([`Pitch::checked`]); then the
    /// form's refusals ([`RoofError::NotARectangle`], [`RoofError::Hips`],
    /// [`RoofError::UnevenGable`]); then the shape's ([`RoofError::OverhangOffAxis`],
    /// [`RoofError::OverhangFolds`], [`RoofError::TooShallow`], [`RoofError::OutOfRange`],
    /// [`RoofError::NotExact`]).
    pub fn new(footprint: Boundary, edges: &BTreeMap<Id, EdgeSpec>) -> Result<Self, RoofError> {
        if !footprint.holes().is_empty() {
            return Err(RoofError::Holes {
                count: footprint.holes().len(),
            });
        }
        let own: Vec<Id> = footprint
            .outer()
            .iter()
            .map(|s: &Segment| s.id.clone())
            .collect();
        if let Some(edge) = edges.keys().find(|id| !own.contains(id)) {
            return Err(RoofError::UnknownEdge { edge: edge.clone() });
        }
        let mut listed = Vec::with_capacity(own.len());
        for id in own {
            let spec = edges.get(&id).copied().unwrap_or_default();
            if spec.overhang < ZERO {
                return Err(RoofError::NegativeOverhang { edge: id });
            }
            if let Some(pitch) = spec.pitch {
                pitch.checked(&id)?;
            }
            listed.push((id, spec));
        }
        let form = form_of(&footprint, &listed)?;
        let roof = Self {
            footprint,
            edges: listed,
            form,
        };
        Plan::of(&roof)?;
        Ok(roof)
    }

    /// The footprint.
    #[must_use]
    pub fn footprint(&self) -> &Boundary {
        &self.footprint
    }

    /// Every footprint edge and what it does, in the footprint's order.
    #[must_use]
    pub fn edges(&self) -> &[(Id, EdgeSpec)] {
        &self.edges
    }

    /// What edge `id` does, if the footprint has it.
    #[must_use]
    pub fn edge(&self, id: &Id) -> Option<&EdgeSpec> {
        self.edges
            .iter()
            .find(|(edge, _)| edge == id)
            .map(|(_, spec)| spec)
    }

    /// Its form.
    #[must_use]
    pub fn form(&self) -> &Form {
        &self.form
    }

    /// A roof from a command's plain form: `{"footprint":<boundary>,"edges":{"<edge ID>":
    /// {"overhang":<grid steps>,"pitch":"<stored slope or angle>"}, ...}}`, the footprint in
    /// construct-slabs' plain form ([`Boundary::from_plain`]), `edges` optional and each field of
    /// an edge optional ([`Roof::new`]).
    ///
    /// # Errors
    /// [`RoofError::Malformed`] for a value not of that form; any refusal of
    /// [`Boundary::from_plain`] or [`Roof::new`].
    pub fn from_plain(value: &Json) -> Result<Self, RoofError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Plain {
            footprint: Json,
            #[serde(default)]
            edges: BTreeMap<String, PlainEdge>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PlainEdge {
            #[serde(default)]
            overhang: i64,
            #[serde(default)]
            pitch: Option<String>,
        }
        let plain: Plain = serde_json::from_value(value.clone())
            .map_err(|e| malformed(format!("a roof is a footprint and its edges: {e}")))?;
        let footprint = Boundary::from_plain(&plain.footprint)?;
        let mut edges = BTreeMap::new();
        for (text, edge) in plain.edges {
            let id = Id::parse(&text).map_err(|_| malformed(format!("{text:?} is not an ID")))?;
            let pitch = match edge.pitch {
                Some(text) => Some(
                    Pitch::from_stored(&text)
                        .ok_or_else(|| malformed(format!("{text:?} is not a slope or an angle")))?,
                ),
                None => None,
            };
            edges.insert(
                id,
                EdgeSpec {
                    overhang: Length::from_steps(edge.overhang),
                    pitch,
                },
            );
        }
        Self::new(footprint, &edges)
    }

    /// The edges in their stored form, as a roof's recipe holds them (FORMAT.md beside this
    /// crate): every footprint edge in the footprint's order, `{"id","overhang"}` and `pitch` when
    /// it slopes.
    #[must_use]
    pub fn edges_value(&self) -> Json {
        let listed: Vec<StoredEdge> = self
            .edges
            .iter()
            .map(|(id, spec)| StoredEdge {
                id: id.as_str().to_owned(),
                overhang: Value::Length(spec.overhang).stored(),
                pitch: spec.pitch.map(Pitch::stored),
            })
            .collect();
        serde_json::to_value(listed).unwrap_or(Json::Null)
    }

    /// Reads a roof from the stored footprint and edges [`Boundary::to_value`] and
    /// [`Roof::edges_value`] write. Anything else is refused, never repaired: an edge out of the
    /// footprint's order or missing, a field added, a length or pitch not in its one spelling, or
    /// a roof [`Roof::new`] refuses.
    ///
    /// # Errors
    /// [`RoofError::Malformed`] for a value not in its one spelling; any refusal of
    /// [`Boundary::from_value`] or [`Roof::new`].
    pub fn from_values(footprint: &Json, edges: &Json) -> Result<Self, RoofError> {
        let footprint = Boundary::from_value(footprint)?;
        let stored: Vec<StoredEdge> = serde_json::from_value(edges.clone())
            .map_err(|e| malformed(format!("a roof's edges are a list of edges: {e}")))?;
        let own: Vec<&Id> = footprint.outer().iter().map(|s| &s.id).collect();
        if stored.len() != own.len()
            || stored
                .iter()
                .zip(&own)
                .any(|(edge, id)| edge.id != id.as_str())
        {
            return Err(malformed(
                "a roof lists every edge of its footprint once, in the footprint's order",
            ));
        }
        let mut specs = BTreeMap::new();
        for (edge, id) in stored.iter().zip(own) {
            let Some(Value::Length(overhang)) = Value::from_stored(&edge.overhang) else {
                return Err(malformed(format!(
                    "edge {id}'s overhang {:?} is not a stored length",
                    edge.overhang
                )));
            };
            let pitch = match &edge.pitch {
                Some(text) => Some(Pitch::from_stored(text).ok_or_else(|| {
                    malformed(format!(
                        "edge {id}'s pitch {text:?} is not a slope or an angle"
                    ))
                })?),
                None => None,
            };
            specs.insert(id.clone(), EdgeSpec { overhang, pitch });
        }
        let read = Self::new(footprint, &specs)?;
        if read.edges_value() != *edges {
            return Err(malformed("the edges are not in their one spelling"));
        }
        Ok(read)
    }

    /// The footprint's corners anticlockwise from above (construct-slabs' profile), each with the
    /// edge that starts there and what that edge does.
    pub(crate) fn corners(&self) -> Vec<(Id, Point, EdgeSpec)> {
        self.footprint
            .profile()
            .into_iter()
            .next()
            .unwrap_or_default()
            .into_iter()
            .map(|(id, at)| {
                let spec = self.edge(&id).copied().unwrap_or_default();
                (id, at, spec)
            })
            .collect()
    }
}

/// One edge as a roof's recipe stores it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredEdge {
    id: String,
    /// construct-params' stored spelling of a length.
    overhang: String,
    /// construct-params' stored spelling of a slope or an angle, when the edge slopes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pitch: Option<String>,
}

fn malformed(why: impl Into<String>) -> RoofError {
    RoofError::Malformed(why.into())
}

/// Whether the edge from `a` to `b` runs along the plan's x or y axis.
pub(crate) fn on_axis(a: Point, b: Point) -> bool {
    a.x == b.x || a.y == b.y
}

/// Whether the loop through `corners` is a rectangle whose edges run along the plan's axes: four
/// corners, each edge along an axis and the next along the other.
fn is_axis_rectangle(corners: &[Point]) -> bool {
    if corners.len() != 4 {
        return false;
    }
    let wrap = |k: usize| k.checked_rem(4).unwrap_or(0);
    let edge = |at: usize| {
        let a = corners.get(wrap(at)).copied();
        let b = corners.get(wrap(at.saturating_add(1))).copied();
        a.zip(b)
    };
    (0..4).all(|at| {
        let (Some((a, b)), Some((c, d))) = (edge(at), edge(at.saturating_add(1))) else {
            return false;
        };
        // Along x then along y, or along y then along x.
        (a.y == b.y && c.x == d.x) || (a.x == b.x && c.y == d.y)
    })
}

/// The form `edges` make on `footprint`, as the module header says.
fn form_of(footprint: &Boundary, edges: &[(Id, EdgeSpec)]) -> Result<Form, RoofError> {
    let sloping: Vec<Id> = edges
        .iter()
        .filter(|(_, spec)| spec.pitch.is_some())
        .map(|(id, _)| id.clone())
        .collect();
    if sloping.is_empty() {
        return Ok(Form::Flat);
    }
    let corners: Vec<Point> = footprint
        .corners(construct_slabs::Ring::Outer)
        .unwrap_or_default()
        .into_iter()
        .map(|(_, at)| at)
        .collect();
    if !is_axis_rectangle(&corners) {
        return Err(RoofError::NotARectangle { edges: sloping });
    }
    let index = |id: &Id| edges.iter().position(|(edge, _)| edge == id);
    match sloping.as_slice() {
        [eaves] => Ok(Form::Mono {
            eaves: eaves.clone(),
        }),
        [first, second] => {
            let apart = index(second)
                .zip(index(first))
                .map(|(b, a)| b.saturating_sub(a));
            if apart != Some(2) {
                return Err(RoofError::Hips {
                    edges: sloping,
                    every: false,
                });
            }
            let slope = |id: &Id| {
                edges
                    .iter()
                    .find(|(edge, _)| edge == id)
                    .and_then(|(_, spec)| spec.pitch)
                    .and_then(Pitch::slope)
            };
            if slope(first) != slope(second) {
                return Err(RoofError::UnevenGable {
                    first: first.clone(),
                    second: second.clone(),
                });
            }
            Ok(Form::Gable {
                eaves: [first.clone(), second.clone()],
            })
        }
        _ => Err(RoofError::Hips {
            every: sloping.len() == edges.len(),
            edges: sloping,
        }),
    }
}
