//! IFC 4.3 out, first half (job 12.1, CON-A18-F1-P2): the project, its site, building and storeys,
//! and the walls and slabs standing on them, written in the STEP text form (ISO 10303-21) under
//! the schema `IFC4X3_ADD2` (ISO 16739-1:2024).
//!
//! What a model gives, and what is written:
//! - The project: units in millimetres, square metres, cubic metres and radians, and one model
//!   context with a `Body` and an `Axis` sub-context. One site and one building, which every model
//!   has here, and one `IfcBuildingStorey` for each level, with its elevation.
//! - A layered wall ([`Wall`]): an `IfcWall` whose `Axis` is its reference line and whose `Body` is
//!   one extrusion of its outline, the outline the builders give: the joined outline when it meets
//!   other walls, else the strip between its two finish faces. Its layers are an
//!   `IfcMaterialLayerSet` (an `IfcMaterialLayerSetUsage` for a wall along an axis of the plan),
//!   each layer with its material and thickness, from the exterior face inward.
//! - A floor ([`Slab`]): an `IfcSlab` whose `Body` is the extrusion of its boundary (holes as the
//!   profile's voids) through the thickness of its layers, which are an `IfcMaterialLayerSetUsage`
//!   bottom layer first. A sloped floor is an `IfcPolygonalFaceSet` of exactly its corners, since
//!   no extrusion of a plan polygon has a sloped top.
//!
//! Numbers: every length is written as an exact decimal of millimetres made from grid steps (1 mm
//! = 6,400 steps) by [`crate::decimal::mm`]; no float is read or written. A wall along an axis of
//! the plan is placed along its own line, so its extrusion is in exact local coordinates; a wall at
//! another angle is placed unturned and written in the plan's own coordinates, since a turned frame
//! would need a square root.
//!
//! `GlobalId`s are [`crate::guid::global_id`] of the thing's kind and permanent ID, so they are the
//! same on every export. Nothing in the text depends on a clock but the header's stamp, which the
//! caller gives. Parts are written in ID order, so the same model gives the same bytes.
//!
//! Not written here: slab edges (a thickened or down-turned edge), and everything in
//! `ifc_openings` (job 12.2): openings, doors, windows, roofs, rooms and property sets.
//!
//! Depends on construct-regen (the filled recipes' own readers, [`Layered`] and [`FloorPlan`]),
//! construct-params (the length) and sha2 (through [`crate::guid`]). Nothing here is interim.

use crate::decimal::mm;
use crate::error::IfcError;
use crate::guid::global_id;
use crate::step::{Step, Val};
use construct_params::Length;
use construct_regen::floor_plan::FloorPlan;
use construct_regen::wall_plan::Layered;
use std::collections::{BTreeMap, BTreeSet};

/// The schema every file is written under: IFC 4.3, the second addendum.
pub const SCHEMA: &str = "IFC4X3_ADD2";
/// What the file says made it.
pub const SYSTEM: &str = "Spare Construct";
/// The tolerance of the model's context, in millimetres: the model tolerance, 0.01 mm.
const PRECISION: &str = "0.01";

/// A level, written as a building storey.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Storey {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// How high it is above the project's zero.
    pub elevation: Length,
}

/// A layered wall, as its filled recipe gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wall {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its type's name.
    pub type_name: String,
    /// The level it stands on, by ID; none for a wall on no level.
    pub storey: Option<String>,
    /// How high its base is above the project's zero.
    pub base: Length,
    /// Its line, height, layers and joined outline.
    pub shape: Layered,
}

/// A floor, as its filled recipe gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slab {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its type's name.
    pub type_name: String,
    /// The level it stands on, by ID; none for a floor on no level.
    pub storey: Option<String>,
    /// How high its top face is above the project's zero (where its corners' own heights, a sloped
    /// floor's, are measured from).
    pub top: Length,
    /// Its boundary, layers and slope.
    pub shape: FloorPlan,
}

/// What one export writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Export {
    /// The model's permanent ID: every `GlobalId` is made from it, with the thing's own ID.
    pub document: String,
    /// The model's name: the project's, and the file's.
    pub name: String,
    /// The header's time, as ISO 8601 text (`2026-11-28T09:30:00`): the only value that varies
    /// between exports of one model, and the caller's to give.
    pub stamp: String,
    /// The levels.
    pub storeys: Vec<Storey>,
    /// The walls.
    pub walls: Vec<Wall>,
    /// The floors.
    pub slabs: Vec<Slab>,
}

fn add(a: i64, b: i64) -> Result<i64, IfcError> {
    a.checked_add(b).ok_or(IfcError::TooBig)
}

fn sub(a: i64, b: i64) -> Result<i64, IfcError> {
    a.checked_sub(b).ok_or(IfcError::TooBig)
}

fn neg(a: i64) -> Result<i64, IfcError> {
    a.checked_neg().ok_or(IfcError::TooBig)
}

/// The text of a list of reals, each a length in steps.
fn lengths(steps: &[i64]) -> Result<Val, IfcError> {
    Ok(Val::reals(
        &steps
            .iter()
            .map(|&s| mm(s))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn real(steps: i64) -> Result<Val, IfcError> {
    Ok(Val::Real(mm(steps)?))
}

/// A wall's frame: along its own line for a wall on an axis of the plan, else the plan's own.
#[derive(Clone, Copy)]
struct Frame {
    /// Where its origin is in the plan (the line's start, or the plan's origin).
    origin: [i64; 2],
    /// The unit step its x runs along, for a wall on an axis; none for a wall at another angle.
    along: Option<[i64; 2]>,
}

impl Frame {
    fn of(wall: &Layered) -> Frame {
        let along = wall.plan.axis().map(construct_regen::wall_plan::Axis::unit);
        Frame {
            origin: if along.is_some() {
                wall.plan.start()
            } else {
                [0, 0]
            },
            along,
        }
    }

    /// A point of the plan in this frame: along the line and to its left, or itself.
    fn local(&self, p: [i64; 2]) -> Result<[i64; 2], IfcError> {
        let Some([ux, uy]) = self.along else {
            return Ok(p);
        };
        let times = |a: i64, b: i64| a.checked_mul(b).ok_or(IfcError::TooBig);
        let r = [sub(p[0], self.origin[0])?, sub(p[1], self.origin[1])?];
        let u = add(times(ux, r[0])?, times(uy, r[1])?)?;
        let v = add(times(neg(uy)?, r[0])?, times(ux, r[1])?)?;
        Ok([u, v])
    }
}

/// A wall's body in plan: its joined outline when it meets other walls, else the strip between its
/// two finish faces.
fn wall_outline(wall: &Layered) -> Result<Vec<[i64; 2]>, IfcError> {
    if let Some(joined) = &wall.joined {
        return Ok(joined.outline.clone());
    }
    let (a, b) = wall.outer();
    let (strip, _) = wall
        .plan
        .strip(a, b)
        .map_err(|e| IfcError::Shape(e.message))?;
    Ok(strip.to_vec())
}

/// The file as it is built.
struct Writer<'a> {
    step: Step,
    export: &'a Export,
    world: u64,
    z_axis: u64,
    body: u64,
    axis: u64,
    /// The materials made, by the ID the model gives them.
    materials: BTreeMap<String, u64>,
    /// The layer sets made, by their type and layers.
    sets: BTreeMap<String, u64>,
    /// Each storey: its instance, its placement and its elevation in steps.
    storeys: BTreeMap<String, (u64, u64, i64)>,
    building: u64,
    building_placement: u64,
    /// What each storey (and the building, for none) contains.
    contained: BTreeMap<Option<String>, Vec<u64>>,
}

impl Writer<'_> {
    fn guid(&self, kind: &str, id: &str) -> Val {
        Val::Text(global_id(&format!(
            "construct:{kind}:{}:{id}",
            self.export.document
        )))
    }

    fn point2(&mut self, x: i64, y: i64) -> Result<u64, IfcError> {
        self.step.add("IFCCARTESIANPOINT", &[lengths(&[x, y])?])
    }

    fn point3(&mut self, x: i64, y: i64, z: i64) -> Result<u64, IfcError> {
        self.step.add("IFCCARTESIANPOINT", &[lengths(&[x, y, z])?])
    }

    fn direction(&mut self, x: i64, y: i64, z: i64) -> Result<u64, IfcError> {
        // Direction ratios, not lengths: whole numbers written as reals.
        let ratio = |n: i64| Val::Real(format!("{n}."));
        self.step.add(
            "IFCDIRECTION",
            &[Val::List(vec![ratio(x), ratio(y), ratio(z)])],
        )
    }

    /// An axis placement at a point, turned about z so its x runs along `along` (a unit step along
    /// an axis of the plan), or unturned.
    fn axis3(&mut self, at: [i64; 3], along: Option<[i64; 2]>) -> Result<u64, IfcError> {
        let location = self.point3(at[0], at[1], at[2])?;
        match along {
            Some([x, y]) => {
                let reference = self.direction(x, y, 0)?;
                self.step.add(
                    "IFCAXIS2PLACEMENT3D",
                    &[
                        Val::Ref(location),
                        Val::Ref(self.z_axis),
                        Val::Ref(reference),
                    ],
                )
            }
            None => self.step.add(
                "IFCAXIS2PLACEMENT3D",
                &[Val::Ref(location), Val::Null, Val::Null],
            ),
        }
    }

    fn placement(&mut self, relative_to: Option<u64>, axis: u64) -> Result<u64, IfcError> {
        self.step.add(
            "IFCLOCALPLACEMENT",
            &[relative_to.map_or(Val::Null, Val::Ref), Val::Ref(axis)],
        )
    }

    /// A rel that aggregates `parts` into `whole`.
    fn aggregate(&mut self, whole: u64, parts: &[u64], seed: &str) -> Result<u64, IfcError> {
        let guid = self.guid("aggregates", seed);
        self.step.add(
            "IFCRELAGGREGATES",
            &[
                guid,
                Val::Null,
                Val::Null,
                Val::Null,
                Val::Ref(whole),
                Val::refs(parts),
            ],
        )
    }

    /// The units and the model context with its `Body` and `Axis` sub-contexts: gives the context
    /// and the unit assignment.
    fn units_and_context(&mut self) -> Result<(u64, u64), IfcError> {
        let si = |step: &mut Step,
                  unit: &'static str,
                  prefix: Option<&'static str>,
                  name: &'static str| {
            step.add(
                "IFCSIUNIT",
                &[
                    Val::Derived,
                    Val::Enum(unit),
                    prefix.map_or(Val::Null, Val::Enum),
                    Val::Enum(name),
                ],
            )
        };
        let units = [
            si(&mut self.step, "LENGTHUNIT", Some("MILLI"), "METRE")?,
            si(&mut self.step, "AREAUNIT", None, "SQUARE_METRE")?,
            si(&mut self.step, "VOLUMEUNIT", None, "CUBIC_METRE")?,
            si(&mut self.step, "PLANEANGLEUNIT", None, "RADIAN")?,
        ];
        let assignment = self.step.add("IFCUNITASSIGNMENT", &[Val::refs(&units)])?;
        let context = self.step.add(
            "IFCGEOMETRICREPRESENTATIONCONTEXT",
            &[
                Val::Null,
                Val::text("Model"),
                Val::Int(3),
                Val::Real(PRECISION.to_owned()),
                Val::Ref(self.world),
                Val::Null,
            ],
        )?;
        let sub_context = |step: &mut Step, name: &str, view: &'static str| {
            step.add(
                "IFCGEOMETRICREPRESENTATIONSUBCONTEXT",
                &[
                    Val::text(name),
                    Val::text("Model"),
                    Val::Derived,
                    Val::Derived,
                    Val::Derived,
                    Val::Derived,
                    Val::Ref(context),
                    Val::Null,
                    Val::Enum(view),
                    Val::Null,
                ],
            )
        };
        self.body = sub_context(&mut self.step, "Body", "MODEL_VIEW")?;
        self.axis = sub_context(&mut self.step, "Axis", "GRAPH_VIEW")?;
        Ok((context, assignment))
    }

    /// The project, and the site and building with their rels. The storeys follow in
    /// [`Writer::storeys_and_rels`].
    fn spatial(&mut self) -> Result<u64, IfcError> {
        let (context, assignment) = self.units_and_context()?;
        let project_id = self.export.document.clone();
        let project = self.step.add(
            "IFCPROJECT",
            &[
                self.guid("project", &project_id),
                Val::Null,
                Val::text(&self.export.name),
                Val::Null,
                Val::Null,
                Val::Null,
                Val::Null,
                Val::refs(&[context]),
                Val::Ref(assignment),
            ],
        )?;
        let site_placement = self.placement(None, self.world)?;
        let site = self.step.add(
            "IFCSITE",
            &[
                self.guid("site", &project_id),
                Val::Null,
                Val::text("Site"),
                Val::Null,
                Val::Null,
                Val::Ref(site_placement),
                Val::Null,
                Val::Null,
                Val::Enum("ELEMENT"),
                Val::Null,
                Val::Null,
                Val::Null,
                Val::Null,
                Val::Null,
            ],
        )?;
        self.building_placement = self.placement(Some(site_placement), self.world)?;
        self.building = self.step.add(
            "IFCBUILDING",
            &[
                self.guid("building", &project_id),
                Val::Null,
                Val::text("Building"),
                Val::Null,
                Val::Null,
                Val::Ref(self.building_placement),
                Val::Null,
                Val::Null,
                Val::Enum("ELEMENT"),
                Val::Null,
                Val::Null,
                Val::Null,
            ],
        )?;
        self.aggregate(project, &[site], "project>site")?;
        self.aggregate(site, &[self.building], "site>building")?;
        Ok(project)
    }

    fn storeys_and_rels(&mut self) -> Result<(), IfcError> {
        let mut sorted: Vec<&Storey> = self.export.storeys.iter().collect();
        sorted.sort_by(|a, b| (a.elevation, &a.id).cmp(&(b.elevation, &b.id)));
        let mut made = Vec::new();
        for storey in sorted {
            if self.storeys.contains_key(&storey.id) {
                return Err(IfcError::Duplicate(storey.id.clone()));
            }
            let elevation = storey.elevation.steps();
            let axis = self.axis3([0, 0, elevation], None)?;
            let placement = self.placement(Some(self.building_placement), axis)?;
            let instance = self.step.add(
                "IFCBUILDINGSTOREY",
                &[
                    self.guid("storey", &storey.id),
                    Val::Null,
                    Val::text(&storey.name),
                    Val::Null,
                    Val::Null,
                    Val::Ref(placement),
                    Val::Null,
                    Val::Null,
                    Val::Enum("ELEMENT"),
                    real(elevation)?,
                ],
            )?;
            self.storeys
                .insert(storey.id.clone(), (instance, placement, elevation));
            made.push(instance);
        }
        if !made.is_empty() {
            self.aggregate(self.building, &made, "building>storeys")?;
        }
        Ok(())
    }

    /// Where an element's placement hangs: the storey's, or the building's. Gives that placement
    /// and the elevation its own z is measured from.
    fn parent(&self, storey: Option<&String>) -> Result<(u64, i64), IfcError> {
        match storey {
            None => Ok((self.building_placement, 0)),
            Some(id) => self
                .storeys
                .get(id)
                .map(|&(_, placement, elevation)| (placement, elevation))
                .ok_or_else(|| IfcError::UnknownStorey(id.clone())),
        }
    }

    fn material(&mut self, id: &str) -> Result<u64, IfcError> {
        if let Some(&made) = self.materials.get(id) {
            return Ok(made);
        }
        let made = self
            .step
            .add("IFCMATERIAL", &[Val::text(id), Val::Null, Val::Null])?;
        self.materials.insert(id.to_owned(), made);
        Ok(made)
    }

    /// An `IfcMaterialLayerSet` of `layers` (material, function, thickness in steps), made once
    /// for each type and layers.
    fn layer_set(
        &mut self,
        type_name: &str,
        layers: &[(String, String, i64)],
    ) -> Result<u64, IfcError> {
        let key = format!("{type_name}|{layers:?}");
        if let Some(&made) = self.sets.get(&key) {
            return Ok(made);
        }
        let mut made = Vec::with_capacity(layers.len());
        for (material, function, thickness) in layers {
            let material = self.material(material)?;
            made.push(self.step.add(
                "IFCMATERIALLAYER",
                &[
                    Val::Ref(material),
                    real(*thickness)?,
                    Val::Bool(false),
                    Val::Null,
                    Val::Null,
                    Val::text(function),
                    Val::Null,
                ],
            )?);
        }
        let set = self.step.add(
            "IFCMATERIALLAYERSET",
            &[Val::refs(&made), Val::text(type_name), Val::Null],
        )?;
        self.sets.insert(key, set);
        Ok(set)
    }

    fn associate(&mut self, element: u64, material: u64, seed: &str) -> Result<(), IfcError> {
        let guid = self.guid("material", seed);
        self.step.add(
            "IFCRELASSOCIATESMATERIAL",
            &[
                guid,
                Val::Null,
                Val::Null,
                Val::Null,
                Val::refs(&[element]),
                Val::Ref(material),
            ],
        )?;
        Ok(())
    }

    /// The closed polyline of `corners`, as a profile outline (the first corner again at the end).
    fn closed(&mut self, corners: &[[i64; 2]]) -> Result<u64, IfcError> {
        let mut points = Vec::with_capacity(corners.len().saturating_add(1));
        for corner in corners.iter().chain(corners.first()) {
            points.push(self.point2(corner[0], corner[1])?);
        }
        self.step.add("IFCPOLYLINE", &[Val::refs(&points)])
    }

    /// An extrusion of `outer` (with `holes`) in the element's own frame, straight up through
    /// `depth`.
    fn extrusion(
        &mut self,
        outer: &[[i64; 2]],
        holes: &[Vec<[i64; 2]>],
        depth: i64,
    ) -> Result<u64, IfcError> {
        let outline = self.closed(outer)?;
        let profile = if holes.is_empty() {
            self.step.add(
                "IFCARBITRARYCLOSEDPROFILEDEF",
                &[Val::Enum("AREA"), Val::Null, Val::Ref(outline)],
            )?
        } else {
            let mut inner = Vec::with_capacity(holes.len());
            for hole in holes {
                inner.push(self.closed(hole)?);
            }
            self.step.add(
                "IFCARBITRARYPROFILEDEFWITHVOIDS",
                &[
                    Val::Enum("AREA"),
                    Val::Null,
                    Val::Ref(outline),
                    Val::refs(&inner),
                ],
            )?
        };
        self.step.add(
            "IFCEXTRUDEDAREASOLID",
            &[
                Val::Ref(profile),
                Val::Ref(self.world),
                Val::Ref(self.z_axis),
                real(depth)?,
            ],
        )
    }

    fn shape(
        &mut self,
        context: u64,
        identifier: &str,
        kind: &str,
        items: &[u64],
    ) -> Result<u64, IfcError> {
        self.step.add(
            "IFCSHAPEREPRESENTATION",
            &[
                Val::Ref(context),
                Val::text(identifier),
                Val::text(kind),
                Val::refs(items),
            ],
        )
    }

    fn contain(&mut self, storey: Option<&String>, element: u64) {
        self.contained
            .entry(storey.cloned())
            .or_default()
            .push(element);
    }

    fn wall(&mut self, wall: &Wall) -> Result<(), IfcError> {
        let (parent, floor) = self.parent(wall.storey.as_ref())?;
        let frame = Frame::of(&wall.shape);
        let local = wall_outline(&wall.shape)?
            .iter()
            .map(|&p| frame.local(p))
            .collect::<Result<Vec<_>, _>>()?;
        let solid = self.extrusion(&local, &[], wall.shape.height)?;
        let body = self.shape(self.body, "Body", "SweptSolid", &[solid])?;
        let plan = wall.shape.plan;
        let [dx, dy] = plan.direction();
        let [sx, sy] = plan.start();
        let from = frame.local([sx, sy])?;
        let to = frame.local([add(sx, dx)?, add(sy, dy)?])?;
        let line = {
            let a = self.point2(from[0], from[1])?;
            let b = self.point2(to[0], to[1])?;
            self.step.add("IFCPOLYLINE", &[Val::refs(&[a, b])])?
        };
        let axis = self.shape(self.axis, "Axis", "Curve2D", &[line])?;
        let definition = self.step.add(
            "IFCPRODUCTDEFINITIONSHAPE",
            &[Val::Null, Val::Null, Val::refs(&[axis, body])],
        )?;
        let origin = [
            frame.origin[0],
            frame.origin[1],
            sub(wall.base.steps(), floor)?,
        ];
        let placed = self.axis3(origin, frame.along)?;
        let placement = self.placement(Some(parent), placed)?;
        let element = self.step.add(
            "IFCWALL",
            &[
                self.guid("wall", &wall.id),
                Val::Null,
                Val::text(&wall.name),
                Val::Null,
                Val::text(&wall.type_name),
                Val::Ref(placement),
                Val::Ref(definition),
                Val::text(&wall.id),
                Val::Enum("STANDARD"),
            ],
        )?;
        self.contain(wall.storey.as_ref(), element);
        // The layers, exterior face first, so across the wall from its right to its left.
        let layers = wall
            .shape
            .layers
            .iter()
            .map(|l| {
                Ok((
                    l.material.clone(),
                    l.function.clone(),
                    i64::try_from(l.thickness()).map_err(|_| IfcError::TooBig)?,
                ))
            })
            .collect::<Result<Vec<_>, IfcError>>()?;
        let set = self.layer_set(&wall.type_name, &layers)?;
        let first = wall.shape.layers.first().map_or(0, |l| l.exterior);
        let last = wall.shape.layers.last().map_or(0, |l| l.interior);
        // Offsets run to the right (toward the exterior); the wall's own y runs to the left. The
        // layers lie from y = -first, toward the interior: up in y when the exterior is on the
        // right of the line's faces, down otherwise.
        let outward = first > last;
        let material = if frame.along.is_some() {
            self.step.add(
                "IFCMATERIALLAYERSETUSAGE",
                &[
                    Val::Ref(set),
                    Val::Enum("AXIS2"),
                    Val::Enum(if outward { "POSITIVE" } else { "NEGATIVE" }),
                    real(neg(first)?)?,
                    Val::Null,
                ],
            )?
        } else {
            set
        };
        self.associate(element, material, &format!("wall:{}", wall.id))
    }

    fn slab(&mut self, slab: &Slab) -> Result<(), IfcError> {
        let plan = &slab.shape;
        let (parent, floor) = self.parent(slab.storey.as_ref())?;
        let total = plan.total().ok_or(IfcError::TooBig)?;
        let flat = plan.top_range().0 == plan.top_range().1;
        let outer: Vec<[i64; 2]> = plan.outer().iter().map(|c| c.at).collect();
        let (items, z) = if flat {
            // The solid stands on its underside: the top less the thickness.
            let top = add(slab.top.steps(), plan.top_range().0)?;
            let base = sub(top, total)?;
            let holes: Vec<Vec<[i64; 2]>> = plan
                .holes()
                .iter()
                .map(|ring| ring.iter().map(|c| c.at).collect())
                .collect();
            (
                vec![self.extrusion(&outer, &holes, total)?],
                sub(base, floor)?,
            )
        } else {
            (vec![self.face_set(slab, floor, total)?], 0)
        };
        let kind = if flat { "SweptSolid" } else { "Tessellation" };
        let body = self.shape(self.body, "Body", kind, &items)?;
        let definition = self.step.add(
            "IFCPRODUCTDEFINITIONSHAPE",
            &[Val::Null, Val::Null, Val::refs(&[body])],
        )?;
        let frame = self.axis3([0, 0, z], None)?;
        let placement = self.placement(Some(parent), frame)?;
        let element = self.step.add(
            "IFCSLAB",
            &[
                self.guid("slab", &slab.id),
                Val::Null,
                Val::text(&slab.name),
                Val::Null,
                Val::text(&slab.type_name),
                Val::Ref(placement),
                Val::Ref(definition),
                Val::text(&slab.id),
                Val::Enum("FLOOR"),
            ],
        )?;
        self.contain(slab.storey.as_ref(), element);
        // The recipe lists layers from the top face down; a layer set runs up from the base.
        let layers = plan
            .layers
            .iter()
            .rev()
            .map(|l| (l.material.clone(), l.function.clone(), l.thickness))
            .collect::<Vec<_>>();
        let set = self.layer_set(&slab.type_name, &layers)?;
        let material = if flat {
            self.step.add(
                "IFCMATERIALLAYERSETUSAGE",
                &[
                    Val::Ref(set),
                    Val::Enum("AXIS3"),
                    Val::Enum("POSITIVE"),
                    real(0)?,
                    Val::Null,
                ],
            )?
        } else {
            set
        };
        self.associate(element, material, &format!("slab:{}", slab.id))
    }

    /// A sloped floor as a closed polygonal face set: its top face at each corner's height, its
    /// underside `total` below that, and a side face along every edge. Coordinates are in the
    /// element's frame, whose z is measured from `floor`.
    fn face_set(&mut self, slab: &Slab, floor: i64, total: i64) -> Result<u64, IfcError> {
        let plan = &slab.shape;
        let mut coordinates = Vec::new();
        // The 1-based index of the top vertex of each corner of each ring; the underside's is the
        // next one.
        let mut rings: Vec<Vec<u64>> = Vec::new();
        for (r, ring) in plan.loops.iter().enumerate() {
            let mut indices = Vec::with_capacity(ring.len());
            for (k, corner) in ring.iter().enumerate() {
                let top = sub(add(slab.top.steps(), plan.top_at(r, k))?, floor)?;
                let at = u64::try_from(coordinates.len())
                    .map_err(|_| IfcError::TooBig)?
                    .checked_add(1)
                    .ok_or(IfcError::TooBig)?;
                indices.push(at);
                coordinates.push(lengths(&[corner.at[0], corner.at[1], top])?);
                coordinates.push(lengths(&[corner.at[0], corner.at[1], sub(top, total)?])?);
            }
            rings.push(indices);
        }
        let ints = |v: &[u64]| {
            Val::List(
                v.iter()
                    .map(|&i| Val::Int(i64::try_from(i).unwrap_or(0)))
                    .collect(),
            )
        };
        let next = |v: &[u64], k: usize| -> Option<u64> {
            v.get(k.checked_add(1)?.checked_rem(v.len())?).copied()
        };
        let mut faces = Vec::new();
        let (outer, holes) = rings
            .split_first()
            .ok_or_else(|| IfcError::Shape("a floor has no boundary".into()))?;
        // The top face, then the underside the other way round.
        let under =
            |v: &[u64]| -> Vec<u64> { v.iter().rev().map(|&i| i.saturating_add(1)).collect() };
        let top_face = if holes.is_empty() {
            self.step.add("IFCINDEXEDPOLYGONALFACE", &[ints(outer)])?
        } else {
            let inner = Val::List(holes.iter().map(|h| ints(h)).collect());
            self.step
                .add("IFCINDEXEDPOLYGONALFACEWITHVOIDS", &[ints(outer), inner])?
        };
        faces.push(top_face);
        let bottom_face = if holes.is_empty() {
            self.step
                .add("IFCINDEXEDPOLYGONALFACE", &[ints(&under(outer))])?
        } else {
            let inner = Val::List(holes.iter().map(|h| ints(&under(h))).collect());
            self.step.add(
                "IFCINDEXEDPOLYGONALFACEWITHVOIDS",
                &[ints(&under(outer)), inner],
            )?
        };
        faces.push(bottom_face);
        // A side face along each edge, outward: the outer loop runs anticlockwise, so its sides
        // run top, bottom, bottom, top; a hole runs clockwise and its sides the other way.
        for (r, ring) in rings.iter().enumerate() {
            for (k, &a) in ring.iter().enumerate() {
                let b = next(ring, k).ok_or(IfcError::TooBig)?;
                let quad = if r == 0 {
                    [a, a.saturating_add(1), b.saturating_add(1), b]
                } else {
                    [a, b, b.saturating_add(1), a.saturating_add(1)]
                };
                faces.push(self.step.add("IFCINDEXEDPOLYGONALFACE", &[ints(&quad)])?);
            }
        }
        let points = self.step.add(
            "IFCCARTESIANPOINTLIST3D",
            &[Val::List(coordinates), Val::Null],
        )?;
        self.step.add(
            "IFCPOLYGONALFACESET",
            &[
                Val::Ref(points),
                Val::Bool(true),
                Val::refs(&faces),
                Val::Null,
            ],
        )
    }

    fn containment(&mut self) -> Result<(), IfcError> {
        let groups: Vec<(Option<String>, Vec<u64>)> =
            std::mem::take(&mut self.contained).into_iter().collect();
        for (storey, elements) in groups {
            let (whole, seed) = match &storey {
                Some(id) => (
                    self.storeys
                        .get(id)
                        .map(|&(instance, _, _)| instance)
                        .ok_or_else(|| IfcError::UnknownStorey(id.clone()))?,
                    format!("storey:{id}"),
                ),
                None => (self.building, "building".to_owned()),
            };
            let guid = self.guid("contains", &seed);
            self.step.add(
                "IFCRELCONTAINEDINSPATIALSTRUCTURE",
                &[
                    guid,
                    Val::Null,
                    Val::Null,
                    Val::Null,
                    Val::refs(&elements),
                    Val::Ref(whole),
                ],
            )?;
        }
        Ok(())
    }
}

/// Writes `export` as an IFC 4.3 file in the STEP text form.
///
/// # Errors
///
/// [`IfcError::Duplicate`] for two levels, walls or floors of one ID, [`IfcError::UnknownStorey`]
/// for an element on a level that was not given, [`IfcError::Shape`] for a wall too thin to have an
/// outline on the grid, and [`IfcError::TooBig`] for a length or count that does not fit.
pub fn write(export: &Export) -> Result<String, IfcError> {
    let mut step = Step::new();
    let origin = step.add(
        "IFCCARTESIANPOINT",
        &[Val::reals(&[mm(0)?, mm(0)?, mm(0)?])],
    )?;
    let z_axis = step.add(
        "IFCDIRECTION",
        &[Val::List(vec![
            Val::Real("0.".into()),
            Val::Real("0.".into()),
            Val::Real("1.".into()),
        ])],
    )?;
    let world = step.add(
        "IFCAXIS2PLACEMENT3D",
        &[Val::Ref(origin), Val::Null, Val::Null],
    )?;
    let mut writer = Writer {
        step,
        export,
        world,
        z_axis,
        body: 0,
        axis: 0,
        materials: BTreeMap::new(),
        sets: BTreeMap::new(),
        storeys: BTreeMap::new(),
        building: 0,
        building_placement: 0,
        contained: BTreeMap::new(),
    };
    writer.spatial()?;
    writer.storeys_and_rels()?;
    let mut walls: Vec<&Wall> = export.walls.iter().collect();
    walls.sort_by(|a, b| a.id.cmp(&b.id));
    let mut seen = BTreeSet::new();
    for wall in walls {
        if !seen.insert(("wall", wall.id.clone())) {
            return Err(IfcError::Duplicate(wall.id.clone()));
        }
        writer.wall(wall)?;
    }
    let mut slabs: Vec<&Slab> = export.slabs.iter().collect();
    slabs.sort_by(|a, b| a.id.cmp(&b.id));
    for slab in slabs {
        if !seen.insert(("slab", slab.id.clone())) {
            return Err(IfcError::Duplicate(slab.id.clone()));
        }
        writer.slab(slab)?;
    }
    writer.containment()?;
    Ok(writer.step.finish(
        "ViewDefinition [DesignTransferView_V1.0]",
        &format!("{}.ifc", export.name),
        &export.stamp,
        SYSTEM,
        SCHEMA,
    ))
}
