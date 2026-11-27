//! The stand-in for the geometry kernel: every shape is a box. This is the only file that knows
//! that. It keeps the kernel's contract, so that the kernel can replace it without anything above
//! noticing: pure, deterministic, whole numbers, canonical bytes, named faces, and a mesh per level
//! of detail.
//!
//! The operations are Spare Construct's, named for what they make: `wall.straight`, not "box".
//! A floor (`floor`) is one box: the box around its outer loop in plan, each side rounded outward
//! to a whole millimetre, from its bottom face to its top face at `z = 0` ([`crate::floor_plan`]);
//! its outline is its outer loop, each corner the nearest millimetre, and its deviation is 0 only
//! when the box is the floor. Its displays draw each layer with a thickness as a part, the box's
//! slice from that layer's bottom face to its top, so a view colours the layers by material on
//! either edition. Interim, as every stand-in shape: its layers as solids and its holes are the
//! kernel's to build (the interim register's row).
//!
//! A roof (`roof`) is one box too: the box around every layer's solid ([`crate::roof_plan`]),
//! rounded outward to whole millimetres, named `box.<role>` for its outer faces. Its displays draw
//! each layer as a part (a solved roof's as each of its pieces drawn as the prism it is): a flat
//! roof's as the box's slice between that layer's faces, as a floor's,
//! a sloped roof's as the layer itself, its section cut into quadrilaterals lofted from verge to
//! verge, each point the nearest millimetre, so a page shows the slopes on either edition.
//! Interim, as every stand-in shape: its layers as solids are the kernel's to build (the interim
//! register's row).
//!
//! A room (`room`) is one box too: the box around its outer loop from its base up to its height
//! ([`crate::room_plan`]), outlined by its outer loop, its deviation 0 only when the box is the
//! room. Interim, as every stand-in shape: its holes and its true outline in 3D are the kernel's
//! to build (the interim register's row).
//!
//! A stair (`stair`) is one box too: the box around every piece ([`crate::stair_plan`]), rounded
//! outward to whole millimetres, named `box.<role>` for its pieces' faces. Its displays draw each
//! piece as a part in the stair's material: a landing as its prism, a run as its side profile cut
//! upright at every corner into quadrilaterals, each a prism across the run, every point the
//! nearest millimetre, so a page shows the steps on either edition. Interim, as every stand-in
//! shape: its pieces as solids are the kernel's to build (the interim register's row).
//!
//! A layered wall (`wall.layered`) is one box of its whole thickness: the prism on its outer
//! outline ([`crate::wall_plan`]), each corner the nearest whole millimetre, named as a box is;
//! its layers are the kernel's to build as solids. Its displays draw each layer as a part, so a
//! view shows the layers and their materials on either edition.
//!
//! A placed family (`family.instance`, job 6.4, [`crate::families`]) is a box per solid: each the
//! box its solid lies in at its type's values, turned as it stands, worked out without an engine
//! (`construct_families::extents`), each side rounded outward to a whole millimetre; each a part
//! of the displays with its solid's material and its step's key, so the glass shows apart from
//! the frame on either edition. Its recipe comes from the library baked in at build time, the
//! source the browser has too (interim until job 7.6). Its body is the box around them all.
//!
//! An opening or a shaft (construct-openings' three operations) is drawn by [`crate::voids`], as on
//! the kernel's edition, with no solid and no face.
//!
//! Interim: it cuts no openings. A wall built with the doors hosted in it
//! ([`Geometry::build_hosting`]) is built alone, as it was before hosting, and says so; the
//! kernel's builder ([`crate::kernel`]) cuts them. The interim register has the row.

use crate::families::{self, Instance};
use crate::floor_plan::{self, FloorPlan};
use crate::roof_plan::{self, PlanSolid, RoofPlan};
use crate::room_plan::{self, RoomPlan};
use crate::stair_plan::{self, PieceSolid, StairPlan};
use crate::wall_plan::{self, Layered};
use construct_families::{Limits, extents};
use construct_model::geometry::{
    Bounds, Built, Display, Edition, FaceName, Geometry, GeometryError, Hosted, Mesh, Part, Recipe,
};
use construct_params::Length;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// The stand-in's edition. A change to what it builds is a new version, as with the kernel.
const VERSION: u32 = 1;

/// The stand-in: builds every recipe as a box, until the geometry kernel can build it.
pub struct Boxes;

/// The six sides of a box, in the kernel's role words (20.2).
const ROLES: [&str; 6] = ["base", "top", "exterior", "interior", "end0", "end1"];
const TRIANGLES: [u32; 36] = [
    0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 2, 3, 7, 2, 7, 6, 0, 4, 7, 0, 7, 3, 1, 2,
    6, 1, 6, 5,
];

/// A body, as the stand-in stores it.
#[derive(Serialize)]
struct BoxBody {
    kind: &'static str,
    size: [i64; 3],
}

fn whole(args: &BTreeMap<String, Value>, name: &str) -> Result<i64, GeometryError> {
    let value = args.get(name).and_then(Value::as_i64);
    let value = value.ok_or_else(|| GeometryError {
        code: "MissingArgument".to_owned(),
        message: format!("{name} must be a whole number of millimetres"),
    })?;
    if value <= 0 {
        return Err(GeometryError {
            code: "EmptyShape".to_owned(),
            message: format!("{name} must be above zero"),
        });
    }
    Ok(value)
}

/// Whether a linear element runs along x, or turns to run along y.
fn runs_along_y(args: &BTreeMap<String, Value>) -> bool {
    args.get("axis").and_then(Value::as_str) == Some("y")
}

/// The box an operation makes: its size along x, y and z.
fn size_of(recipe: &Recipe) -> Result<[i64; 3], GeometryError> {
    let args = &recipe.args;
    let linear = |long: &str, across: &str| -> Result<[i64; 3], GeometryError> {
        let (long, across, height) = (
            whole(args, long)?,
            whole(args, across)?,
            whole(args, "height")?,
        );
        Ok(if runs_along_y(args) {
            [across, long, height]
        } else {
            [long, across, height]
        })
    };
    match recipe.op.as_str() {
        "wall.straight" => linear("length", "thickness"),
        "door.single" => linear("width", "thickness"),
        "block" => Ok([
            whole(args, "width")?,
            whole(args, "depth")?,
            whole(args, "height")?,
        ]),
        other => Err(GeometryError {
            code: "UnknownOperation".to_owned(),
            message: format!("this edition can't build {other}"),
        }),
    }
}

fn mesh(size: [i64; 3]) -> Mesh {
    let [w, d, h] = size;
    let corners = [
        [0, 0, 0],
        [w, 0, 0],
        [w, d, 0],
        [0, d, 0],
        [0, 0, h],
        [w, 0, h],
        [w, d, h],
        [0, d, h],
    ];
    Mesh {
        positions: corners.concat(),
        indices: TRIANGLES.to_vec(),
    }
}

/// A floor's body, as the stand-in stores it: the box it is, in whole millimetres.
#[derive(Serialize)]
struct FloorBody {
    kind: &'static str,
    min: [i64; 3],
    max: [i64; 3],
}

/// The corners of the box from `lo` to `hi`, in [`TRIANGLES`]' order.
fn box_corners(lo: [i64; 3], hi: [i64; 3]) -> [[i64; 3]; 8] {
    let [lx, ly, lz] = lo;
    let [hx, hy, hz] = hi;
    [
        [lx, ly, lz],
        [hx, ly, lz],
        [hx, hy, lz],
        [lx, hy, lz],
        [lx, ly, hz],
        [hx, ly, hz],
        [hx, hy, hz],
        [lx, hy, hz],
    ]
}

/// A floor's displayed mesh and its parts: for each layer with a thickness, top first, the slice of
/// the box from `min` to `max` between that layer's faces (each the nearest millimetre, the
/// lowest the box's bottom), a part with the outline `outline`.
fn floor_layers(
    plan: &FloorPlan,
    min: [i64; 3],
    max: [i64; 3],
    outline: &[[i64; 2]],
) -> Result<(Mesh, Vec<Part>), GeometryError> {
    let mut mesh = Mesh {
        positions: Vec::new(),
        indices: Vec::new(),
    };
    let mut parts = Vec::new();
    let slabs = plan.slabs();
    let last = slabs.len().saturating_sub(1);
    for (n, (_, layer, bottom, top)) in slabs.iter().enumerate() {
        let low = if n == last {
            min[2]
        } else {
            floor_plan::nearest_mm(*bottom).0
        };
        let high = floor_plan::nearest_mm(*top).0.min(max[2]);
        let (first, base) = (
            part_count(mesh.indices.len() / 3)?,
            part_count(mesh.positions.len() / 3)?,
        );
        let corners = box_corners([min[0], min[1], low], [max[0], max[1], high]);
        mesh.positions.extend(corners.concat());
        for index in TRIANGLES {
            mesh.indices
                .push(base.checked_add(index).ok_or_else(too_many)?);
        }
        parts.push(Part {
            material: layer.material.clone(),
            function: layer.function.clone(),
            triangles: [first, part_count(TRIANGLES.len() / 3)?],
            outline: outline.to_vec(),
        });
    }
    Ok((mesh, parts))
}

/// How far a box from `min` to `max` strays from the shape it stands for: by nothing when it is
/// the shape (`exact`); otherwise by at most its plan's diagonal (every point of one is within that
/// of every point of the other); and by one millimetre more where anything was `rounded`.
fn straying(min: [i64; 3], max: [i64; 3], exact: bool, rounded: bool) -> i64 {
    if exact {
        return i64::from(rounded);
    }
    let [w, d] = [
        i128::from(max[0].saturating_sub(min[0])),
        i128::from(max[1].saturating_sub(min[1])),
    ];
    let square = w.saturating_mul(w).saturating_add(d.saturating_mul(d));
    let mut root = square.isqrt();
    if root.saturating_mul(root) < square {
        root = root.saturating_add(1);
    }
    i64::try_from(root)
        .unwrap_or(i64::MAX)
        .saturating_add(i64::from(rounded))
}

/// A floor as one box ([`crate::floor_plan`]): the box around its outer loop, its sides rounded
/// outward to whole millimetres, from its bottom face (rounded down) to its top face at 0. Named
/// `box.top`, `box.bottom` and `box.side.<edge ID>` for each edge of each loop, in loop order. Its
/// displays draw each layer as a part ([`floor_layers`]); its body is the one box.
fn floor(recipe: &Recipe) -> Result<Built, GeometryError> {
    let plan = FloorPlan::of(recipe)?;
    let total = plan.total().unwrap_or_default();
    let ([x0, y0], [x1, y1]) = plan.extent();
    let min = [
        floor_plan::floor_mm(x0),
        floor_plan::floor_mm(y0),
        floor_plan::floor_mm(total.saturating_neg()),
    ];
    let max = [floor_plan::ceil_mm(x1), floor_plan::ceil_mm(y1), 0];
    let mut rounded = false;
    let mut outline = Vec::with_capacity(plan.outer().len());
    for corner in plan.outer() {
        let [x, y] = corner.at;
        let ((x, a), (y, b)) = (floor_plan::nearest_mm(x), floor_plan::nearest_mm(y));
        rounded |= !(a && b);
        outline.push([x, y]);
    }
    rounded |= !floor_plan::nearest_mm(total).1;
    let deviation = straying(min, max, plan.is_box(), rounded);
    let (mesh, parts) = floor_layers(&plan, min, max, &outline)?;
    let names = floor_names(&plan);
    let body = FloorBody {
        kind: "floor",
        min,
        max,
    };
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation,
            mesh: mesh.clone(),
            outline: outline.clone(),
            parts: parts.clone(),
        })
        .collect();
    Ok(Built {
        body: serde_json::to_vec(&body).unwrap_or_default(),
        bounds: Bounds { min, max },
        names,
        displays,
    })
}

/// A sloped roof's displayed mesh and its parts: each layer's loft drawn as it is, its section (a
/// lower chain of corners and the upper chain back) cut into quadrilaterals between the two
/// chains, each a prism from one verge to the other, every point the nearest millimetre.
fn sloped_layers(
    plan: &RoofPlan,
    outline: &[[i64; 2]],
    rounded: &mut bool,
) -> Result<(Mesh, Vec<Part>), GeometryError> {
    let mut mesh = Mesh {
        positions: Vec::new(),
        indices: Vec::new(),
    };
    let mut parts = Vec::new();
    let mut near = |p: [i64; 3]| {
        p.map(|c| {
            let (mm, exact) = floor_plan::nearest_mm(c);
            *rounded |= !exact;
            mm
        })
    };
    let corner = |list: &[[i64; 3]], k: usize| list.get(k).copied().unwrap_or_default();
    for layer in &plan.layers {
        if let Some(PlanSolid::Unite { profiles, by }) = &layer.solid {
            let start = part_count(mesh.indices.len().checked_div(3).unwrap_or(0))?;
            let mut triangles = 0_usize;
            for profile in profiles {
                let low: Vec<[i64; 3]> = profile.iter().map(|p| near(*p)).collect();
                let high: Vec<[i64; 3]> = profile
                    .iter()
                    .map(|p| {
                        near([
                            p[0].saturating_add(by[0]),
                            p[1].saturating_add(by[1]),
                            p[2].saturating_add(by[2]),
                        ])
                    })
                    .collect();
                triangles = triangles.saturating_add(prism(&mut mesh, &low, &high)?);
            }
            parts.push(Part {
                material: layer.material.clone(),
                function: layer.function.clone(),
                triangles: [start, part_count(triangles)?],
                outline: outline.to_vec(),
            });
            continue;
        }
        let Some(PlanSolid::Loft {
            sections: [first, second],
        }) = &layer.solid
        else {
            continue;
        };
        let count = first.len();
        if count.checked_rem(2) != Some(0) {
            return Err(GeometryError {
                code: "MissingArgument".to_owned(),
                message: "a sloped layer's section is a lower and an upper chain of corners"
                    .to_owned(),
            });
        }
        let a: Vec<[i64; 3]> = first.iter().map(|p| near(*p)).collect();
        let b: Vec<[i64; 3]> = second.iter().map(|p| near(*p)).collect();
        let start = part_count(mesh.indices.len().checked_div(3).unwrap_or(0))?;
        // The box's triangles face out when its first ring turns anticlockwise seen from the side
        // its second ring is on.
        let towards = {
            let (p, q) = (corner(&a, 0), corner(&b, 0));
            [
                q[0].saturating_sub(p[0]),
                q[1].saturating_sub(p[1]),
                q[2].saturating_sub(p[2]),
            ]
        };
        let mut quads = 0_usize;
        for i in 0..count.checked_div(2).unwrap_or(0).saturating_sub(1) {
            let mut quad = [
                i,
                i.saturating_add(1),
                count.saturating_sub(2).saturating_sub(i),
                count.saturating_sub(1).saturating_sub(i),
            ];
            let ring: Vec<[i64; 3]> = quad.iter().map(|&k| corner(&a, k)).collect();
            if dot(newell(&ring), towards) < 0 {
                quad.reverse();
            }
            let at = part_count(mesh.positions.len().checked_div(3).unwrap_or(0))?;
            for list in [&a, &b] {
                for &k in &quad {
                    mesh.positions.extend(corner(list, k));
                }
            }
            for index in TRIANGLES {
                mesh.indices
                    .push(at.checked_add(index).ok_or_else(too_many)?);
            }
            quads = quads.saturating_add(1);
        }
        let triangles = quads
            .saturating_mul(TRIANGLES.len())
            .checked_div(3)
            .unwrap_or(0);
        parts.push(Part {
            material: layer.material.clone(),
            function: layer.function.clone(),
            triangles: [start, part_count(triangles)?],
            outline: outline.to_vec(),
        });
    }
    Ok((mesh, parts))
}

/// A prism between two rings of one count, `low` anticlockwise from above and `high` the same
/// ring moved up, appended to `mesh` with every face outward: its two caps fanned from their first
/// corner and each wall two triangles. The count of its triangles.
fn prism(mesh: &mut Mesh, low: &[[i64; 3]], high: &[[i64; 3]]) -> Result<usize, GeometryError> {
    let n = low.len();
    let at = part_count(mesh.positions.len().checked_div(3).unwrap_or(0))?;
    for p in low.iter().chain(high) {
        mesh.positions.extend(*p);
    }
    let mut corners: Vec<[usize; 3]> = Vec::new();
    for k in 1..n.saturating_sub(1) {
        let next = k.saturating_add(1);
        corners.push([0, next, k]);
        corners.push([n, n.saturating_add(k), n.saturating_add(next)]);
    }
    for k in 0..n {
        let next = k.saturating_add(1).checked_rem(n).unwrap_or(0);
        let (top, top_next) = (n.saturating_add(k), n.saturating_add(next));
        corners.push([k, next, top_next]);
        corners.push([k, top_next, top]);
    }
    for triangle in &corners {
        for &k in triangle {
            mesh.indices
                .push(at.checked_add(part_count(k)?).ok_or_else(too_many)?);
        }
    }
    Ok(corners.len())
}

/// The normal of a flat polygon by Newell's sum, its length twice the polygon's area, pointing to
/// the side it turns anticlockwise seen from. Every coordinate is a whole millimetre within the
/// kernel's grid (under 2^28), so no sum comes near the range of an `i128`.
fn newell(ring: &[[i64; 3]]) -> [i128; 3] {
    let mut n = [0_i128; 3];
    for (at, p) in ring.iter().enumerate() {
        let next = at.saturating_add(1).checked_rem(ring.len()).unwrap_or(0);
        let q = ring.get(next).copied().unwrap_or(*p);
        let [px, py, pz] = p.map(i128::from);
        let [qx, qy, qz] = q.map(i128::from);
        let terms = [
            (py.saturating_sub(qy)).saturating_mul(pz.saturating_add(qz)),
            (pz.saturating_sub(qz)).saturating_mul(px.saturating_add(qx)),
            (px.saturating_sub(qx)).saturating_mul(py.saturating_add(qy)),
        ];
        for (sum, term) in n.iter_mut().zip(terms) {
            *sum = sum.saturating_add(term);
        }
    }
    n
}

/// `n . d`.
fn dot(n: [i128; 3], d: [i64; 3]) -> i128 {
    n.iter().zip(d).fold(0_i128, |sum, (a, b)| {
        sum.saturating_add(a.saturating_mul(i128::from(b)))
    })
}

/// A flat roof's displayed mesh and its parts: for each layer with a thickness, top first, the
/// slice of the box from `min` to `max` between that layer's faces, each the nearest millimetre.
fn flat_layers(
    plan: &RoofPlan,
    min: [i64; 3],
    max: [i64; 3],
    outline: &[[i64; 2]],
    rounded: &mut bool,
) -> Result<(Mesh, Vec<Part>), GeometryError> {
    let mut mesh = Mesh {
        positions: Vec::new(),
        indices: Vec::new(),
    };
    let mut parts = Vec::new();
    for layer in &plan.layers {
        let Some(PlanSolid::Extrude { profile, by }) = &layer.solid else {
            continue;
        };
        let z = profile.first().map_or(0, |p| p[2]);
        let (low, a) = floor_plan::nearest_mm(z);
        let (high, b) = floor_plan::nearest_mm(z.saturating_add(by[2]));
        *rounded |= !(a && b);
        let (first, base) = (
            part_count(mesh.indices.len().checked_div(3).unwrap_or(0))?,
            part_count(mesh.positions.len().checked_div(3).unwrap_or(0))?,
        );
        let corners = box_corners([min[0], min[1], low], [max[0], max[1], high]);
        mesh.positions.extend(corners.concat());
        for index in TRIANGLES {
            mesh.indices
                .push(base.checked_add(index).ok_or_else(too_many)?);
        }
        parts.push(Part {
            material: layer.material.clone(),
            function: layer.function.clone(),
            triangles: [
                first,
                part_count(TRIANGLES.len().checked_div(3).unwrap_or(0))?,
            ],
            outline: outline.to_vec(),
        });
    }
    Ok((mesh, parts))
}

/// A roof as one box ([`crate::roof_plan`]): the box around every layer's solid, rounded outward
/// to whole millimetres, named `box.<role>` for its outer faces (its tops, bottoms and sides). Its
/// displays draw each layer as a part: a flat roof's as the box's slice between that layer's
/// faces ([`flat_layers`], as a floor's), a sloped roof's as its loft itself
/// ([`sloped_layers`]); its body is the one box. A flat roof strays as a floor's box does; a
/// sloped roof's displays are its layers, straying by 1 mm where a point was rounded.
fn roof(recipe: &Recipe) -> Result<Built, GeometryError> {
    let plan = RoofPlan::of(recipe)?;
    let (lo, hi) = plan.extent();
    let min = lo.map(floor_plan::floor_mm);
    let max = hi.map(floor_plan::ceil_mm);
    let mut rounded = false;
    let mut outline = Vec::with_capacity(plan.outline.len());
    for corner in &plan.outline {
        let [x, y] = corner.at;
        let ((x, a), (y, b)) = (floor_plan::nearest_mm(x), floor_plan::nearest_mm(y));
        rounded |= !(a && b);
        outline.push([x, y]);
    }
    let (mesh, parts, deviation) = if plan.slopes.is_empty() {
        let (mesh, parts) = flat_layers(&plan, min, max, &outline, &mut rounded)?;
        let upright = plan.outline.len() == 4
            && plan
                .outline
                .iter()
                .zip(plan.outline.iter().cycle().skip(1))
                .all(|(a, b)| a.at[0] == b.at[0] || a.at[1] == b.at[1]);
        (mesh, parts, straying(min, max, upright, rounded))
    } else {
        let (mesh, parts) = sloped_layers(&plan, &outline, &mut rounded)?;
        (mesh, parts, i64::from(rounded))
    };
    let body = FloorBody {
        kind: "roof",
        min,
        max,
    };
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation,
            mesh: mesh.clone(),
            outline: outline.clone(),
            parts: parts.clone(),
        })
        .collect();
    Ok(Built {
        body: serde_json::to_vec(&body).unwrap_or_default(),
        bounds: Bounds { min, max },
        names: roof_names(&plan),
        displays,
    })
}

/// A roof's face names: `box.<role>` for each of its outer roles, in order.
fn roof_names(plan: &RoofPlan) -> Vec<FaceName> {
    plan.outer_roles()
        .into_iter()
        .map(|role| FaceName {
            name: format!("box.{role}"),
            role,
        })
        .collect()
}

/// The corners of a run's side profile in its own plane: `(a, z)` along the plan axis it runs on
/// (`x` when every corner has one `y`, else `y`), in grid steps.
fn profile_in_plane(section: &[[i64; 3]]) -> Result<Vec<[i64; 2]>, GeometryError> {
    let first = section.first().copied().unwrap_or_default();
    let along_x = section.iter().all(|p| p[1] == first[1]);
    let along_y = section.iter().all(|p| p[0] == first[0]);
    if !(along_x || along_y) {
        return Err(GeometryError {
            code: "MissingArgument".to_owned(),
            message: "a stair's run is profiled in an upright plane along x or y".to_owned(),
        });
    }
    Ok(section
        .iter()
        .map(|p| [if along_x { p[0] } else { p[1] }, p[2]])
        .collect())
}

/// The height at `a` of the edge from `p` to `q` (`p[0] != q[0]`), in grid steps, to the nearest
/// step: a display's point, never stored.
fn height_at(p: [i64; 2], q: [i64; 2], a: i64) -> i64 {
    let (da, dz) = (
        i128::from(q[0]).saturating_sub(i128::from(p[0])),
        i128::from(q[1]).saturating_sub(i128::from(p[1])),
    );
    let along = i128::from(a).saturating_sub(i128::from(p[0]));
    // p.z + dz * along / da, rounded to the nearest step a half up.
    let num = dz
        .saturating_mul(along)
        .saturating_mul(2)
        .saturating_add(da);
    let step = num.checked_div_euclid(da.saturating_mul(2)).unwrap_or(0);
    i64::try_from(i128::from(p[1]).saturating_add(step)).unwrap_or(p[1])
}

/// A run's side profile cut upright at every corner into strips, each a quadrilateral (its top
/// and bottom straight across it), as `[(a0, bottom), (a1, bottom), (a1, top), (a0, top)]`.
fn strips(profile: &[[i64; 2]]) -> Result<Vec<[[i64; 2]; 4]>, GeometryError> {
    let mut stops: Vec<i64> = profile.iter().map(|p| p[0]).collect();
    stops.sort_unstable();
    stops.dedup();
    let edges: Vec<([i64; 2], [i64; 2])> = profile
        .iter()
        .zip(profile.iter().cycle().skip(1))
        .map(|(p, q)| if p[0] <= q[0] { (*p, *q) } else { (*q, *p) })
        .filter(|(p, q)| p[0] < q[0])
        .collect();
    let mut out = Vec::new();
    for pair in stops.windows(2) {
        let (Some(&a0), Some(&a1)) = (pair.first(), pair.get(1)) else {
            continue;
        };
        let across: Vec<([i64; 2], [i64; 2])> = edges
            .iter()
            .filter(|(p, q)| p[0] <= a0 && q[0] >= a1)
            .copied()
            .collect();
        let [(p0, q0), (p1, q1)] = across.as_slice() else {
            return Err(GeometryError {
                code: "MissingArgument".to_owned(),
                message: "a stair's run is one strip from its foot to its end".to_owned(),
            });
        };
        let (mut bottom, mut top) = (
            [height_at(*p0, *q0, a0), height_at(*p0, *q0, a1)],
            [height_at(*p1, *q1, a0), height_at(*p1, *q1, a1)],
        );
        if bottom[0].saturating_add(bottom[1]) > top[0].saturating_add(top[1]) {
            std::mem::swap(&mut bottom, &mut top);
        }
        out.push([[a0, bottom[0]], [a1, bottom[1]], [a1, top[1]], [a0, top[0]]]);
    }
    Ok(out)
}

/// A run's loft drawn as its side profile `right` cut upright at every corner into quadrilaterals
/// ([`strips`]), each a prism across to `left`, every point put through `near`. The count of its
/// triangles.
fn run_strips(
    mesh: &mut Mesh,
    right: &[[i64; 3]],
    left: &[[i64; 3]],
    near: &mut impl FnMut([i64; 3]) -> [i64; 3],
) -> Result<usize, GeometryError> {
    let (Some(r0), Some(l0)) = (right.first(), left.first()) else {
        return Ok(0);
    };
    let across = [l0[0].saturating_sub(r0[0]), l0[1].saturating_sub(r0[1]), 0];
    let shift = [
        floor_plan::nearest_mm(across[0]).0,
        floor_plan::nearest_mm(across[1]).0,
    ];
    let along_x = right.iter().all(|p| p[1] == r0[1]);
    let fixed = if along_x { r0[1] } else { r0[0] };
    let mut triangles = 0_usize;
    for strip in strips(&profile_in_plane(right)?)? {
        let place = |[a, z]: [i64; 2]| {
            if along_x {
                [a, fixed, z]
            } else {
                [fixed, a, z]
            }
        };
        let mut ring: Vec<[i64; 3]> = strip.iter().map(|c| near(place(*c))).collect();
        // Faces outward: the ring turns anticlockwise seen from the side across.
        if dot(newell(&ring), across) < 0 {
            ring.reverse();
        }
        let other: Vec<[i64; 3]> = ring
            .iter()
            .map(|p| {
                [
                    p[0].saturating_add(shift[0]),
                    p[1].saturating_add(shift[1]),
                    p[2],
                ]
            })
            .collect();
        triangles = triangles.saturating_add(prism(mesh, &ring, &other)?);
    }
    Ok(triangles)
}

/// A stair as one box ([`crate::stair_plan`]): the box around every piece, rounded outward to
/// whole millimetres, named `box.<role>` for every role its pieces have. Its displays draw each
/// piece as a part in the stair's material: a landing as the prism it is, a run as its side
/// profile cut upright at every corner into quadrilaterals, each a prism across its width, every
/// point the nearest millimetre, so a page shows the steps on either edition; its body is the one
/// box. Its displays stray by 1 mm where a point was rounded.
fn stair(recipe: &Recipe) -> Result<Built, GeometryError> {
    let plan = StairPlan::of(recipe)?;
    let (lo, hi) = plan.extent();
    let min = lo.map(floor_plan::floor_mm);
    let max = hi.map(floor_plan::ceil_mm);
    let mut rounded = false;
    let mut near = |p: [i64; 3]| {
        p.map(|c| {
            let (mm, exact) = floor_plan::nearest_mm(c);
            rounded |= !exact;
            mm
        })
    };
    let mut mesh = Mesh {
        positions: Vec::new(),
        indices: Vec::new(),
    };
    let mut parts = Vec::new();
    let mut first_outline = Vec::new();
    for piece in &plan.pieces {
        let outline: Vec<[i64; 2]> = piece
            .plan
            .iter()
            .map(|&[x, y]| {
                let [x, y, _] = near([x, y, 0]);
                [x, y]
            })
            .collect();
        if first_outline.is_empty() {
            first_outline.clone_from(&outline);
        }
        let start = part_count(mesh.indices.len().checked_div(3).unwrap_or(0))?;
        let triangles = match &piece.solid {
            PieceSolid::Extrude { profile, by } => {
                let low: Vec<[i64; 3]> = profile.iter().map(|p| near(*p)).collect();
                let high: Vec<[i64; 3]> = profile
                    .iter()
                    .map(|p| {
                        near([
                            p[0].saturating_add(by[0]),
                            p[1].saturating_add(by[1]),
                            p[2].saturating_add(by[2]),
                        ])
                    })
                    .collect();
                prism(&mut mesh, &low, &high)?
            }
            PieceSolid::Loft {
                sections: [right, left],
            } => run_strips(&mut mesh, right, left, &mut near)?,
        };
        parts.push(Part {
            material: plan.material.clone(),
            function: "structure".to_owned(),
            triangles: [start, part_count(triangles)?],
            outline,
        });
    }
    let body = FloorBody {
        kind: "stair",
        min,
        max,
    };
    let deviation = i64::from(rounded);
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation,
            mesh: mesh.clone(),
            outline: first_outline.clone(),
            parts: parts.clone(),
        })
        .collect();
    Ok(Built {
        body: serde_json::to_vec(&body).unwrap_or_default(),
        bounds: Bounds { min, max },
        names: stair_names(&plan),
        displays,
    })
}

/// A stair's face names: `box.<role>` for each of its pieces' roles, in order.
fn stair_names(plan: &StairPlan) -> Vec<FaceName> {
    plan.roles()
        .into_iter()
        .map(|role| FaceName {
            name: format!("box.{role}"),
            role,
        })
        .collect()
}

/// A floor's face names: `box.top`, `box.bottom` and `box.side.<edge ID>` for each edge of each
/// loop, in loop order.
fn floor_names(plan: &FloorPlan) -> Vec<FaceName> {
    let mut names = vec![
        FaceName {
            name: "box.top".to_owned(),
            role: "top".to_owned(),
        },
        FaceName {
            name: "box.bottom".to_owned(),
            role: "bottom".to_owned(),
        },
    ];
    for corner in plan.loops.iter().flatten() {
        let role = format!("side.{}", corner.id);
        names.push(FaceName {
            name: format!("box.{role}"),
            role,
        });
    }
    names
}

/// A placed family's body, as the stand-in stores it: the family, its type and turn, and the box
/// around its solids' boxes, in whole millimetres.
#[derive(Serialize)]
struct FamilyBody<'a> {
    kind: &'static str,
    family: &'a str,
    #[serde(rename = "type")]
    ty: &'a str,
    turn: u8,
    min: [i64; 3],
    max: [i64; 3],
}

/// The smallest whole number of millimetres at or above the diagonal of a box of `size`.
fn diagonal(size: [i64; 3]) -> i64 {
    let square = size.iter().fold(0_i128, |sum, &length| {
        let length = i128::from(length);
        sum.saturating_add(length.saturating_mul(length))
    });
    let mut root = square.isqrt();
    if root.saturating_mul(root) < square {
        root = root.saturating_add(1);
    }
    i64::try_from(root).unwrap_or(i64::MAX)
}

/// A placed family as a box per solid ([`construct_families::extents`]): each solid's box, its
/// sides rounded outward to whole millimetres, a part of both displays with its material and its
/// step's key; the bounds and the body the box around them all. Each box strays from its solid by
/// at most its diagonal (a door's frame is not its box), so that is the displays' deviation. It
/// names no face.
fn family(recipe: &Recipe) -> Result<Built, GeometryError> {
    let instance = Instance::of(recipe)?;
    let library = families::baked();
    let resolved = families::resolve(&*library, &instance)?;
    let boxes = extents(
        &resolved.recipe,
        &resolved.values,
        Limits::DEFAULT,
        instance.turn,
    )
    .map_err(|e| families::evaluation_refused(&resolved.recipe, &e))?;
    let mut mesh = Mesh {
        positions: Vec::new(),
        indices: Vec::new(),
    };
    let mut parts = Vec::with_capacity(boxes.len());
    let mut reach: Option<([i64; 3], [i64; 3])> = None;
    let mut deviation = 0_i64;
    for solid in &boxes {
        let lo = solid.low.map(|l| floor_plan::floor_mm(l.steps()));
        let hi = solid.high.map(|l| floor_plan::ceil_mm(l.steps()));
        let size = [
            hi[0].saturating_sub(lo[0]),
            hi[1].saturating_sub(lo[1]),
            hi[2].saturating_sub(lo[2]),
        ];
        deviation = deviation.max(diagonal(size));
        reach = Some(match reach {
            None => (lo, hi),
            Some((mut min, mut max)) => {
                for (m, l) in min.iter_mut().zip(lo) {
                    *m = (*m).min(l);
                }
                for (m, h) in max.iter_mut().zip(hi) {
                    *m = (*m).max(h);
                }
                (min, max)
            }
        });
        let (first, base) = (
            part_count(mesh.indices.len() / 3)?,
            part_count(mesh.positions.len() / 3)?,
        );
        mesh.positions.extend(box_corners(lo, hi).concat());
        for index in TRIANGLES {
            mesh.indices
                .push(base.checked_add(index).ok_or_else(too_many)?);
        }
        parts.push(Part {
            material: solid.material.as_str().to_owned(),
            function: solid.step.as_str().to_owned(),
            triangles: [first, part_count(TRIANGLES.len() / 3)?],
            outline: vec![
                [lo[0], lo[1]],
                [hi[0], lo[1]],
                [hi[0], hi[1]],
                [lo[0], hi[1]],
            ],
        });
    }
    let (min, max) = reach.ok_or_else(|| GeometryError {
        code: "EmptyShape".to_owned(),
        message: format!("family {}: it has no solid to draw", resolved.recipe.name),
    })?;
    let body = FamilyBody {
        kind: "family",
        family: instance.family.as_str(),
        ty: instance.ty.as_str(),
        turn: instance.turn.quarters(),
        min,
        max,
    };
    let outline = vec![
        [min[0], min[1]],
        [max[0], min[1]],
        [max[0], max[1]],
        [min[0], max[1]],
    ];
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation,
            mesh: mesh.clone(),
            outline: outline.clone(),
            parts: parts.clone(),
        })
        .collect();
    Ok(Built {
        body: serde_json::to_vec(&body).unwrap_or_default(),
        bounds: Bounds { min, max },
        names: Vec::new(),
        displays,
    })
}

/// A room as one box ([`crate::room_plan`]): the box around its outer loop, its sides rounded
/// outward to whole millimetres, from its base at 0 up to its height (rounded up). Named
/// `box.bottom`, `box.top` and `box.side.<k>` for each edge of each loop, in loop order. Its
/// outline is its outer loop, each corner the nearest millimetre; its deviation is 0 only when the
/// box is the room, as a floor's; it has no parts (a room is space, of no material).
fn room(recipe: &Recipe) -> Result<Built, GeometryError> {
    let plan = RoomPlan::of(recipe)?;
    let ([x0, y0], [x1, y1]) = plan.extent();
    let min = [floor_plan::floor_mm(x0), floor_plan::floor_mm(y0), 0];
    let max = [
        floor_plan::ceil_mm(x1),
        floor_plan::ceil_mm(y1),
        floor_plan::ceil_mm(plan.height),
    ];
    let mut rounded = !floor_plan::nearest_mm(plan.height).1;
    let mut outline = Vec::with_capacity(plan.outer().len());
    for corner in plan.outer() {
        let [x, y] = corner.at;
        let ((x, a), (y, b)) = (floor_plan::nearest_mm(x), floor_plan::nearest_mm(y));
        rounded |= !(a && b);
        outline.push([x, y]);
    }
    // As a floor's box: it strays by nothing when it is the room, else by at most its plan's
    // diagonal, and by one millimetre more where a corner or the height was rounded.
    let deviation = if plan.is_box() {
        i64::from(rounded)
    } else {
        let [w, d] = [
            i128::from(max[0].saturating_sub(min[0])),
            i128::from(max[1].saturating_sub(min[1])),
        ];
        let square = w.saturating_mul(w).saturating_add(d.saturating_mul(d));
        let mut root = square.isqrt();
        if root.saturating_mul(root) < square {
            root = root.saturating_add(1);
        }
        i64::try_from(root)
            .unwrap_or(i64::MAX)
            .saturating_add(i64::from(rounded))
    };
    let body = FloorBody {
        kind: "room",
        min,
        max,
    };
    let mesh = Mesh {
        positions: box_corners(min, max).concat(),
        indices: TRIANGLES.to_vec(),
    };
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation,
            mesh: mesh.clone(),
            outline: outline.clone(),
            parts: Vec::new(),
        })
        .collect();
    Ok(Built {
        body: serde_json::to_vec(&body).unwrap_or_default(),
        bounds: Bounds { min, max },
        names: room_names(&plan),
        displays,
    })
}

/// A room's face names: `box.<role>` for each of [`RoomPlan::roles`].
fn room_names(plan: &RoomPlan) -> Vec<FaceName> {
    plan.roles()
        .into_iter()
        .map(|role| FaceName {
            name: format!("box.{role}"),
            role,
        })
        .collect()
}

/// The persistent names of a box's faces, one per role: the same six for every box.
fn box_names() -> Vec<FaceName> {
    ROLES
        .iter()
        .map(|role| FaceName {
            name: format!("box.{role}"),
            role: (*role).to_owned(),
        })
        .collect()
}

/// The persistent names of the faces `recipe` builds, without building it: what a hosted element
/// names its host's face by.
///
/// # Errors
///
/// As [`Geometry::build`]: a recipe it cannot build has no faces.
pub fn names(recipe: &Recipe) -> Result<Vec<FaceName>, GeometryError> {
    if construct_openings::is_void(recipe) {
        return Ok(Vec::new());
    }
    if families::is_instance(recipe) {
        Instance::of(recipe)?;
        return Ok(Vec::new());
    }
    if floor_plan::is_floor(recipe) {
        return Ok(floor_names(&FloorPlan::of(recipe)?));
    }
    if roof_plan::is_roof(recipe) {
        return Ok(roof_names(&RoofPlan::of(recipe)?));
    }
    if room_plan::is_room(recipe) {
        return Ok(room_names(&RoomPlan::of(recipe)?));
    }
    if stair_plan::is_stair(recipe) {
        return Ok(stair_names(&StairPlan::of(recipe)?));
    }
    if wall_plan::is_layered(recipe) {
        Layered::of(recipe)?;
    } else {
        size_of(recipe)?;
    }
    Ok(box_names())
}

fn too_many() -> GeometryError {
    GeometryError {
        code: "TooBig".to_owned(),
        message: "a wall with more layers than a display can count".to_owned(),
    }
}

/// A count of a display's corners or triangles, as its mesh numbers them.
fn part_count(n: usize) -> Result<u32, GeometryError> {
    u32::try_from(n).map_err(|_| too_many())
}

/// A layered wall's body, as the stand-in stores it: its outer outline and its height, in whole
/// millimetres.
#[derive(Serialize)]
struct WallBody {
    kind: &'static str,
    outline: Vec<[i64; 2]>,
    height: i64,
}

/// A layered wall as one box of its whole thickness: the prism on its outer outline, the strip
/// from its exterior face to its interior face ([`crate::wall_plan`]), each corner the nearest
/// whole millimetre, from its base to its top. Its bounds are rounded outward. Its displays draw
/// each layer with a thickness as a part of its own, the prism on that layer's strip, so a view
/// shows the layers and their materials as the kernel's displays do; its body is the one box.
fn layered(recipe: &Recipe) -> Result<Built, GeometryError> {
    let wall = Layered::of(recipe)?;
    let (exterior, interior) = wall.outer();
    let (corners, _) = wall.plan.strip(exterior, interior)?;
    let mut rounded = false;
    let mut near = |steps: i64| {
        let (mm, exact) = wall_plan::nearest_mm(steps);
        rounded |= !exact;
        mm
    };
    let outline: Vec<[i64; 2]> = corners.iter().map(|&[x, y]| [near(x), near(y)]).collect();
    let height = near(wall.height);
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    let mut parts = Vec::new();
    for layer in wall.layers.iter().filter(|layer| layer.thickness() > 0) {
        let (strip, _) = wall.plan.strip(layer.exterior, layer.interior)?;
        let shape: Vec<[i64; 2]> = strip.iter().map(|&[x, y]| [near(x), near(y)]).collect();
        let (first, base) = (
            part_count(indices.len() / 3)?,
            part_count(positions.len() / 3)?,
        );
        for z in [0, height] {
            for &[x, y] in &shape {
                positions.extend([x, y, z]);
            }
        }
        for index in TRIANGLES {
            indices.push(base.checked_add(index).ok_or_else(too_many)?);
        }
        parts.push(Part {
            material: layer.material.clone(),
            function: layer.function.clone(),
            triangles: [first, part_count(TRIANGLES.len() / 3)?],
            outline: shape,
        });
    }
    let deviation = i64::from(rounded);
    let (mut min, mut max) = ([0, 0, 0], [0, 0, wall.height]);
    for axis in 0..2 {
        let values = corners.iter().filter_map(|c| c.get(axis).copied());
        if let (Some(lo), Some(hi)) = (values.clone().min(), values.max())
            && let (Some(l), Some(h)) = (min.get_mut(axis), max.get_mut(axis))
        {
            (*l, *h) = (lo, hi);
        }
    }
    let per = Length::STEPS_PER_MM;
    let floor = |s: i64| s.checked_div_euclid(per).unwrap_or(0);
    let ceil = |s: i64| match s.checked_rem_euclid(per) {
        Some(0) => floor(s),
        _ => floor(s).saturating_add(1),
    };
    let body = WallBody {
        kind: "wall",
        outline: outline.clone(),
        height,
    };
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation,
            mesh: Mesh {
                positions: positions.clone(),
                indices: indices.clone(),
            },
            outline: outline.clone(),
            parts: parts.clone(),
        })
        .collect();
    Ok(Built {
        body: serde_json::to_vec(&body).unwrap_or_default(),
        bounds: Bounds {
            min: min.map(floor),
            max: max.map(ceil),
        },
        names: box_names(),
        displays,
    })
}

impl Geometry for Boxes {
    fn edition(&self) -> Edition {
        Edition {
            engine: "construct.stand-in".to_owned(),
            version: VERSION,
            name_scheme: 0,
            report: None,
        }
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        if let Some(datum) = crate::datum::build(recipe) {
            return datum;
        }
        if let Some(void) = crate::voids::build(recipe) {
            return void;
        }
        if floor_plan::is_floor(recipe) {
            return floor(recipe);
        }
        if roof_plan::is_roof(recipe) {
            return roof(recipe);
        }
        if families::is_instance(recipe) {
            return family(recipe);
        }
        if room_plan::is_room(recipe) {
            return room(recipe);
        }
        if stair_plan::is_stair(recipe) {
            return stair(recipe);
        }
        if wall_plan::is_layered(recipe) {
            return layered(recipe);
        }
        let size = size_of(recipe)?;
        let [w, d, _] = size;
        let body = serde_json::to_vec(&BoxBody { kind: "box", size }).unwrap_or_default();
        let names = box_names();
        let outline = vec![[0, 0], [w, 0], [w, d], [0, d]];
        // A box has no finer or coarser form, so both levels are the same shape. The kernel's
        // levels will differ: their bytes do here too, so the records already stand apart.
        let displays = (0..2)
            .map(|lod| Display {
                lod,
                deviation: 0,
                mesh: mesh(size),
                outline: outline.clone(),
                parts: Vec::new(),
            })
            .collect();
        Ok(Built {
            body,
            bounds: Bounds {
                min: [0, 0, 0],
                max: size,
            },
            names,
            displays,
        })
    }

    /// Interim (the module header): the stand-in cuts no openings, so a host is built alone,
    /// whatever is hosted in it, exactly as [`Geometry::build`] builds it.
    fn build_hosting(&self, host: &Recipe, hosted: &[Hosted<'_>]) -> Result<Built, GeometryError> {
        let _ = hosted;
        self.build(host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn recipe(op: &str, args: Value) -> Recipe {
        let args = serde_json::from_value(args).unwrap();
        Recipe::construct(op, args)
    }

    #[test]
    fn a_wall_is_as_long_as_it_says_along_its_axis() {
        let args = json!({ "length": 3000, "thickness": 200, "height": 2700 });
        let along_x = Boxes.build(&recipe("wall.straight", args)).unwrap();
        assert_eq!(along_x.bounds.max, [3000, 200, 2700]);
        let args = json!({ "length": 3000, "thickness": 200, "height": 2700, "axis": "y" });
        let along_y = Boxes.build(&recipe("wall.straight", args)).unwrap();
        assert_eq!(along_y.bounds.max, [200, 3000, 2700]);
    }

    #[test]
    fn the_same_recipe_always_gives_the_same_bytes() {
        let args = json!({ "width": 900, "thickness": 300, "height": 2100 });
        let one = Boxes.build(&recipe("door.single", args.clone())).unwrap();
        let two = Boxes.build(&recipe("door.single", args)).unwrap();
        assert_eq!(one.body_id(), two.body_id());
        assert_eq!(one.displays[0].id(), two.displays[0].id());
    }

    #[test]
    fn each_level_of_detail_is_a_record_of_its_own() {
        let args = json!({ "width": 900, "depth": 900, "height": 900 });
        let built = Boxes.build(&recipe("block", args)).unwrap();
        assert_ne!(built.displays[0].id(), built.displays[1].id());
    }

    #[test]
    fn a_recipe_that_cant_build_says_why() {
        let bad = Boxes.build(&recipe("stair.spiral", json!({}))).unwrap_err();
        assert_eq!(bad.code, "UnknownOperation");
        let bad = Boxes
            .build(&recipe("block", json!({ "width": 1 })))
            .unwrap_err();
        assert_eq!(bad.code, "MissingArgument");
        let args = json!({ "width": 0, "depth": 1, "height": 1 });
        assert_eq!(
            Boxes.build(&recipe("block", args)).unwrap_err().code,
            "EmptyShape"
        );
    }

    /// The stand-in cuts no openings: a wall with a door hosted in it is the wall alone, byte for
    /// byte, and its faces are named without building it as building names them.
    #[test]
    fn a_wall_with_a_door_is_the_wall_alone() {
        let wall = recipe(
            "wall.straight",
            json!({ "length": 3000, "thickness": 200, "height": 2700 }),
        );
        let door = recipe(
            "door.single",
            json!({ "width": 900, "thickness": 200, "height": 2100 }),
        );
        let at = construct_model::element::Placement {
            x: 1000,
            y: 0,
            z: 0,
        };
        let hosted = [Hosted {
            key: "D-1",
            recipe: &door,
            at,
        }];
        let alone = Boxes.build(&wall).unwrap();
        assert_eq!(Boxes.build_hosting(&wall, &hosted).unwrap(), alone);
        assert_eq!(names(&wall).unwrap(), alone.names);
        assert_eq!(
            names(&recipe("stair.spiral", json!({}))).unwrap_err().code,
            "UnknownOperation"
        );
    }

    fn q0(steps: i64) -> String {
        construct_params::Value::Length(Length::from_steps(steps)).stored()
    }

    /// The default partition (12.5, 90, 12.5 mm) on a line from `start` to `end` in steps, 2700
    /// mm high, centred on its line.
    fn partition(start: [i64; 2], end: [i64; 2]) -> Recipe {
        let layer = |f: &str, a: i64, b: i64| json!({ "function": f, "material": "m", "exterior": q0(a), "interior": q0(b) });
        recipe(
            "wall.layered",
            json!({
                "start": [q0(start[0]), q0(start[1])],
                "end": [q0(end[0]), q0(end[1])],
                "height": q0(17_280_000),
                "layers": [
                    layer("finish", 368_000, 288_000),
                    layer("structure", 288_000, -288_000),
                    layer("finish", -288_000, -368_000),
                ],
            }),
        )
    }

    /// A layered wall is one box of its whole thickness: the prism on its outer outline, each
    /// corner the nearest millimetre (57.5 mm rounds away from zero), its bounds rounded outward,
    /// named as a box is; a layered wall at an angle is its outer parallelogram. Its recorded
    /// bytes never change within an edition.
    #[test]
    fn a_layered_wall_is_one_box_of_its_whole_thickness() {
        let wall = partition([0, 0], [25_600_000, 0]);
        let built = Boxes.build(&wall).unwrap();
        let outline = vec![[0, -58], [4000, -58], [4000, 58], [0, 58]];
        assert_eq!(built.displays[0].outline, outline);
        assert_eq!(built.displays[0].deviation, 1);
        assert_eq!(built.bounds.min, [0, -58, 0]);
        assert_eq!(built.bounds.max, [4000, 58, 2700]);
        assert_eq!(built.names, box_names());
        assert_eq!(names(&wall).unwrap(), built.names);
        assert_eq!(
            built.body,
            br#"{"kind":"wall","outline":[[0,-58],[4000,-58],[4000,58],[0,58]],"height":2700}"#
        );
        // Each layer is a part of the displays: its strip's prism, its material and function.
        let parts = &built.displays[0].parts;
        let outlines: Vec<&Vec<[i64; 2]>> = parts.iter().map(|part| &part.outline).collect();
        assert_eq!(
            outlines,
            [
                &vec![[0, -58], [4000, -58], [4000, -45], [0, -45]],
                &vec![[0, -45], [4000, -45], [4000, 45], [0, 45]],
                &vec![[0, 45], [4000, 45], [4000, 58], [0, 58]],
            ]
        );
        let triangles: Vec<[u32; 2]> = parts.iter().map(|part| part.triangles).collect();
        assert_eq!(triangles, [[0, 12], [12, 12], [24, 12]]);
        let functions: Vec<&str> = parts.iter().map(|part| part.function.as_str()).collect();
        assert_eq!(functions, ["finish", "structure", "finish"]);
        assert!(parts.iter().all(|part| part.material == "m"));
        assert_eq!(built.displays[0].mesh.positions.len(), 72);
        assert_eq!(built.displays[0].mesh.indices.len(), 108);
        assert_eq!(built.displays[0].mesh.indices[36], 8);
        assert_eq!(built.displays[1].mesh, built.displays[0].mesh);
        assert_eq!(built.displays[1].parts, built.displays[0].parts);
        // A 3-4-5 wall from the origin: its exterior face is (46, -34.5) mm off its line.
        let angled = Boxes
            .build(&partition([0, 0], [15_360_000, 20_480_000]))
            .unwrap();
        assert_eq!(
            angled.displays[0].outline,
            vec![[46, -35], [2446, 3166], [2354, 3235], [-46, 35]]
        );
        assert_eq!(angled.bounds.min, [-46, -35, 0]);
        assert_eq!(angled.bounds.max, [2446, 3235, 2700]);
        let mut unfilled = wall;
        unfilled
            .args
            .insert("layers".to_owned(), json!({ "wall": "type" }));
        assert_eq!(Boxes.build(&unfilled).unwrap_err().code, "Unfilled");
        assert_eq!(names(&unfilled).unwrap_err().code, "Unfilled");
    }

    /// A filled floor: the L-shaped floor of 8 m by 8 m less its notch, with a 2 m square hole,
    /// its two layers 200 mm in all; or, with `rectangle`, a 4 m by 3 m rectangle.
    #[allow(clippy::arithmetic_side_effects, reason = "test code")]
    fn floor_of(rectangle: bool) -> Recipe {
        let mm = |v: i64| v * 6400;
        let corner = |id: &str, x: i64, y: i64| json!({ "id": id, "at": [mm(x), mm(y)] });
        let loops = if rectangle {
            json!([[
                corner("e0", 0, 0),
                corner("e1", 4000, 0),
                corner("e2", 4000, 3000),
                corner("e3", 0, 3000)
            ]])
        } else {
            json!([
                [
                    corner("e0", 0, 0),
                    corner("e1", 8000, 0),
                    corner("e2", 8000, 4000),
                    corner("e3", 4000, 4000),
                    corner("e4", 4000, 8000),
                    corner("e5", 0, 8000)
                ],
                [
                    corner("h3", 1000, 1000),
                    corner("h2", 1000, 3000),
                    corner("h1", 3000, 3000),
                    corner("h0", 3000, 1000)
                ]
            ])
        };
        let layer = |f: &str, t: i64| json!({ "function": f, "material": "m", "thickness": mm(t) });
        recipe(
            "floor",
            json!({ "loops": loops, "layers": [layer("finish", 50), layer("structure", 150)] }),
        )
    }

    /// A floor is one box: the box around its outer loop from its bottom face to its top face at
    /// 0, its outline its outer loop, named top, bottom and a side per edge; it strays by nothing
    /// when it is a rectangle, and by the box's plan diagonal otherwise. Its bytes are pinned, and
    /// a floor whose layers are not filled in does not build.
    #[test]
    fn a_floor_is_the_box_around_its_outer_loop() {
        let rectangle = Boxes.build(&floor_of(true)).unwrap();
        assert_eq!(rectangle.bounds.min, [0, 0, -200]);
        assert_eq!(rectangle.bounds.max, [4000, 3000, 0]);
        assert_eq!(rectangle.displays[0].deviation, 0);
        assert_eq!(
            rectangle.body,
            br#"{"kind":"floor","min":[0,0,-200],"max":[4000,3000,0]}"#
        );
        let l = Boxes.build(&floor_of(false)).unwrap();
        assert_eq!(l.bounds.min, [0, 0, -200]);
        assert_eq!(l.bounds.max, [8000, 8000, 0]);
        assert_eq!(
            l.displays[0].outline,
            vec![
                [0, 0],
                [8000, 0],
                [8000, 4000],
                [4000, 4000],
                [4000, 8000],
                [0, 8000]
            ]
        );
        // The diagonal of 8 m by 8 m is 11,313.7 mm.
        assert_eq!(l.displays[0].deviation, 11_314);
        // Each layer is a part: the box's slice between its faces, top first, in its material.
        let parts = &l.displays[0].parts;
        let functions: Vec<&str> = parts.iter().map(|p| p.function.as_str()).collect();
        assert_eq!(functions, ["finish", "structure"]);
        let triangles: Vec<[u32; 2]> = parts.iter().map(|p| p.triangles).collect();
        assert_eq!(triangles, [[0, 12], [12, 12]]);
        assert!(parts.iter().all(|p| p.outline == l.displays[0].outline));
        assert!(parts.iter().all(|p| p.material == "m"));
        let heights: Vec<i64> = l.displays[0]
            .mesh
            .positions
            .iter()
            .skip(2)
            .step_by(3)
            .copied()
            .collect();
        let (a, b, c) = (-50, 0, -200);
        assert_eq!(
            heights,
            [a, a, a, a, b, b, b, b, c, c, c, c, a, a, a, a],
            "finish from -50 to 0, structure from -200 to -50"
        );
        assert_eq!(l.displays[1].parts, *parts);
        let roles: Vec<&str> = l.names.iter().map(|n| n.role.as_str()).collect();
        assert_eq!(roles.len(), 2 + 6 + 4);
        assert_eq!(&roles[..3], ["top", "bottom", "side.e0"]);
        assert_eq!(l.names[2].name, "box.side.e0");
        assert_eq!(names(&floor_of(false)).unwrap(), l.names);
        let mut unfilled = floor_of(true);
        unfilled
            .args
            .insert("layers".to_owned(), json!({ "type": "layers" }));
        assert_eq!(Boxes.build(&unfilled).unwrap_err().code, "Unfilled");
    }

    /// A filled 6 m by 4 m roof (construct-roofs' `filled` form), every point in whole
    /// millimetres: with `gable`, rising at 1 in 4 from o0 and o2 to a ridge at y = 2 m, 500 mm
    /// over its eaves and 300 mm over its verges, one layer 206 mm upright; otherwise flat, two
    /// layers of 100 and 200 mm over the same outline.
    #[allow(clippy::arithmetic_side_effects, reason = "test code")]
    fn roof_of(gable: bool) -> Recipe {
        let mm = |v: i64| v * 6400;
        let outline = json!([
            { "id": "o0", "at": [mm(-300), mm(-500)] },
            { "id": "o1", "at": [mm(6300), mm(-500)] },
            { "id": "o2", "at": [mm(6300), mm(4500)] },
            { "id": "o3", "at": [mm(-300), mm(4500)] },
        ]);
        if gable {
            let section = |x: i64| {
                json!([
                    [mm(x), mm(-500), mm(-125)],
                    [mm(x), mm(2000), mm(500)],
                    [mm(x), mm(4500), mm(-125)],
                    [mm(x), mm(4500), mm(81)],
                    [mm(x), mm(2000), mm(706)],
                    [mm(x), mm(-500), mm(81)],
                ])
            };
            let layer = json!({ "function": "structure", "material": "m", "thickness": mm(200),
                                "solid": { "loft": [section(-300), section(6300)] } });
            return recipe(
                "roof",
                json!({ "form": "gable", "slopes": ["o0", "o2"], "outline": outline, "layers": [layer] }),
            );
        }
        let profile = |z: i64| {
            json!([
                [mm(-300), mm(-500), mm(z)],
                [mm(6300), mm(-500), mm(z)],
                [mm(6300), mm(4500), mm(z)],
                [mm(-300), mm(4500), mm(z)]
            ])
        };
        let layer = |f: &str, z: i64, t: i64| {
            json!({ "function": f, "material": "m", "thickness": mm(t),
                    "solid": { "extrude": profile(z), "by": [0, 0, mm(t)] } })
        };
        recipe(
            "roof",
            json!({ "form": "flat", "slopes": [], "outline": outline,
                    "layers": [layer("thermal", 200, 100), layer("structure", 0, 200)] }),
        )
    }

    /// Six times the signed volume a mesh holds, in cubic millimetres.
    #[allow(clippy::arithmetic_side_effects, reason = "test code")]
    fn six_volume(mesh: &Mesh) -> i128 {
        let p = |i: u32| {
            let i = i as usize * 3;
            [
                i128::from(mesh.positions[i]),
                i128::from(mesh.positions[i + 1]),
                i128::from(mesh.positions[i + 2]),
            ]
        };
        mesh.indices
            .chunks(3)
            .map(|t| {
                let [a, b, c] = [p(t[0]), p(t[1]), p(t[2])];
                a[0] * (b[1] * c[2] - b[2] * c[1])
                    + a[1] * (b[2] * c[0] - b[0] * c[2])
                    + a[2] * (b[0] * c[1] - b[1] * c[0])
            })
            .sum()
    }

    /// A roof is one box: the box around its layers, named by its outer faces. A gable's displays
    /// draw its layer as it is, its chevron cut at the ridge into two quadrilaterals lofted from
    /// verge to verge, facing out, holding the layer's volume exactly (5 m across by 206 mm
    /// upright, 6.6 m long), straying by nothing; a flat roof's draw each layer as the box's slice.
    #[test]
    #[allow(clippy::arithmetic_side_effects, reason = "test code")]
    fn a_roof_is_its_layers_drawn_on_the_stand_in() {
        let gable = Boxes.build(&roof_of(true)).unwrap();
        assert_eq!(gable.bounds.min, [-300, -500, -125]);
        assert_eq!(gable.bounds.max, [6300, 4500, 706]);
        assert_eq!(
            gable.body,
            br#"{"kind":"roof","min":[-300,-500,-125],"max":[6300,4500,706]}"#
        );
        let display = &gable.displays[0];
        assert_eq!(display.deviation, 0);
        assert_eq!(
            display.outline,
            vec![[-300, -500], [6300, -500], [6300, 4500], [-300, 4500]]
        );
        assert_eq!(display.parts.len(), 1);
        assert_eq!(display.parts[0].triangles, [0, 24]);
        assert_eq!(display.parts[0].function, "structure");
        assert_eq!(six_volume(&display.mesh), 6 * 5000 * 206 * 6600);
        assert_eq!(
            gable.displays[1],
            Display {
                lod: 1,
                ..display.clone()
            }
        );
        let roles: Vec<&str> = gable.names.iter().map(|n| n.role.as_str()).collect();
        assert_eq!(
            roles,
            [
                "top.o0",
                "bottom.o0",
                "top.o2",
                "bottom.o2",
                "side.o0",
                "side.o1",
                "side.o2",
                "side.o3"
            ]
        );
        assert_eq!(gable.names[0].name, "box.top.o0");
        assert_eq!(names(&roof_of(true)).unwrap(), gable.names);

        let flat = Boxes.build(&roof_of(false)).unwrap();
        assert_eq!(flat.bounds.min, [-300, -500, 0]);
        assert_eq!(flat.bounds.max, [6300, 4500, 300]);
        assert_eq!(flat.displays[0].deviation, 0);
        let parts = &flat.displays[0].parts;
        let functions: Vec<&str> = parts.iter().map(|p| p.function.as_str()).collect();
        assert_eq!(functions, ["thermal", "structure"]);
        assert_eq!(six_volume(&flat.displays[0].mesh), 6 * 6600 * 5000 * 300);
        let roles: Vec<&str> = flat.names.iter().map(|n| n.role.as_str()).collect();
        assert_eq!(
            roles,
            ["top", "bottom", "side.o0", "side.o1", "side.o2", "side.o3"]
        );

        let mut unfilled = roof_of(true);
        unfilled
            .args
            .insert("layers".to_owned(), json!({ "type": "layers" }));
        assert_eq!(Boxes.build(&unfilled).unwrap_err().code, "Unfilled");
    }

    /// Recorded: a document's stored shapes must keep their meaning. A change here is a new
    /// edition, never an edit.
    #[test]
    fn the_stand_ins_bytes_never_change_within_an_edition() {
        let args = json!({ "width": 900, "thickness": 300, "height": 2100 });
        let built = Boxes.build(&recipe("door.single", args)).unwrap();
        assert_eq!(built.body, br#"{"kind":"box","size":[900,300,2100]}"#);
    }
}
