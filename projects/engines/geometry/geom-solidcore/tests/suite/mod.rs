//! The engine suite: the building cases of the engine seam, written over `geom_op::engine::Engine`
//! alone, so that any engine is judged by them. A test file runs an engine through it by
//! implementing [`Checked`] for the engine (how to read the kernel's body of its solid) and
//! calling each case with a way to make a fresh engine. Every solid an engine returns is held to
//! the kernel's checker at its full level here as well, whatever the engine says it checked.
//!
//! The cases: a wall of 4000 by 2700 by 200 mm with a door opening of 900 by 2100 mm, on the
//! building grid `Q0`, where one millimetre is 6,400 steps. An exact engine of the kernel's own
//! runs the same file (`#[path]` from its own tests) on the day it takes an operation over.

use std::collections::BTreeMap;

use geom_check::{Level, check};
use geom_num::{GridStep, I256, P3};
use geom_op::engine::{Engine, EngineError, Profile, Volume};
use geom_run::{Budget, WorkMeter};
use geom_topo::{Body, BodyKind};

/// The building grid.
pub const STEP: GridStep = GridStep::Q0;

/// One millimetre in quarter-nanometres.
const MM: u64 = 4_000_000;

/// How the suite reads the kernel's body of an engine's solid, to check it.
pub trait Checked: Engine {
    /// The kernel's body of `solid` and the exactness it carries.
    fn body(solid: &Self::Solid) -> (&Body, geom_num::Exactness);
}

/// `mm` whole millimetres in steps of [`STEP`]: the one conversion a program makes at its seam.
pub fn mm(mm: i64) -> i64 {
    let per = STEP
        .steps_per(MM)
        .expect("a millimetre is a whole number of steps");
    mm * i64::try_from(per).expect("a small number")
}

/// The grid point `(x, y, z)` millimetres.
pub fn at(x: i64, y: i64, z: i64) -> P3 {
    P3::new(mm(x), mm(y), mm(z)).expect("a grid point")
}

/// The rectangle of corners `(x0, y0)` to `(x1, y1)` millimetres at height `z`.
fn rectangle(x0: i64, y0: i64, x1: i64, y1: i64, z: i64) -> Vec<P3> {
    vec![at(x0, y0, z), at(x1, y0, z), at(x1, y1, z), at(x0, y1, z)]
}

/// `solid`, once the kernel's checker has passed its body as a solid.
fn checked<E: Checked>(solid: E::Solid) -> E::Solid {
    let (body, exactness) = E::body(&solid);
    assert_eq!(body.kind(), BodyKind::Solid);
    let report = check(
        body,
        exactness,
        Level::Full,
        &mut WorkMeter::new(Budget::UNLIMITED),
    )
    .expect("an unlimited meter");
    assert!(report.passed(), "{:?}", report.faults().first());
    solid
}

/// How the wall is drawn: in plan and swept up, or in elevation and swept through its thickness.
#[derive(Clone, Copy, Debug)]
pub enum Drawn {
    /// Its plan, 4000 by 200 mm, swept 2700 mm up.
    InPlan,
    /// Its elevation, 4000 by 2700 mm on y = 0, swept 200 mm along y.
    InElevation,
}

/// Both ways of drawing the wall.
pub const BOTH: [Drawn; 2] = [Drawn::InPlan, Drawn::InElevation];

/// The wall: 4000 mm along x, 200 mm thick along y, 2700 mm high, from the origin, drawn `drawn`.
pub fn wall<E: Checked>(e: &mut E, drawn: Drawn) -> E::Solid {
    let (profile, by) = match drawn {
        Drawn::InPlan => (rectangle(0, 0, 4000, 200, 0), [0, 0, mm(2700)]),
        Drawn::InElevation => (
            vec![
                at(0, 0, 0),
                at(4000, 0, 0),
                at(4000, 0, 2700),
                at(0, 0, 2700),
            ],
            [0, mm(200), 0],
        ),
    };
    let profile = Profile {
        outer: profile,
        holes: vec![],
    };
    checked::<E>(e.extrude(&profile, by).expect("the wall"))
}

/// The door's cutter: 900 mm wide from x = 1550 mm, through the wall and 100 mm past each face,
/// from 100 mm below the floor to 2100 mm.
pub fn door<E: Checked>(e: &mut E) -> E::Solid {
    let profile = Profile {
        outer: rectangle(1550, -100, 2450, 300, -100),
        holes: vec![],
    };
    checked::<E>(
        e.extrude(&profile, [0, 0, mm(2200)])
            .expect("the door's cutter"),
    )
}

/// The wall, drawn `drawn`, with the door cut out of it.
pub fn wall_with_door<E: Checked>(e: &mut E, drawn: Drawn) -> E::Solid {
    let (w, d) = (wall(e, drawn), door(e));
    checked::<E>(e.subtract(&w, &d).expect("the wall less the door"))
}

/// Six times the volume of `mm3` cubic millimetres, in cubic steps.
fn six_times(mm3: i128) -> i128 {
    let s = i128::from(mm(1));
    6 * mm3 * s * s * s
}

/// The wall's volume less the door's, in cubic millimetres.
const EXPECTED_MM3: i128 = 4000 * 2700 * 200 - 900 * 2100 * 200;

/// Six times the volume in cubic steps, when the engine gives it so.
fn cubic_steps(v: Volume) -> I256 {
    match v {
        Volume::SixTimes { cubic_steps, .. } => cubic_steps,
        other => panic!("{other:?}"),
    }
}

/// The volume of the wall with its door is the box less the opening, and the wall's alone the
/// box, whichever way the wall is drawn. Six times the volume of a flat body's rows is exact, so
/// an engine that gives it is within any tolerance by being equal.
pub fn a_wall_with_a_door_is_the_box_less_the_opening<E: Checked>(mut e: E) {
    for drawn in BOTH {
        let s = wall_with_door(&mut e, drawn);
        let v = e.volume(&s).expect("the volume");
        assert_eq!(
            cubic_steps(v),
            I256::from_i128(six_times(EXPECTED_MM3)),
            "{drawn:?}"
        );
        let w = wall(&mut e, drawn);
        let whole = e.volume(&w).expect("the wall's volume");
        assert_eq!(
            cubic_steps(whole),
            I256::from_i128(six_times(4000 * 2700 * 200)),
            "{drawn:?}"
        );
    }
}

/// The same wall drawn in elevation, its opening a hole of the profile (raised 300 mm off the
/// floor so that it is a hole), extruded 200 mm through its thickness: the box less the hole.
pub fn a_profile_with_a_hole_extrudes_to_the_box_less_the_hole<E: Checked>(mut e: E) {
    let xz = |x0: i64, z0: i64, x1: i64, z1: i64| {
        vec![at(x0, 0, z0), at(x1, 0, z0), at(x1, 0, z1), at(x0, 0, z1)]
    };
    let profile = Profile {
        outer: xz(0, 0, 4000, 2700),
        holes: vec![xz(1550, 300, 2450, 2400)],
    };
    let s = checked::<E>(e.extrude(&profile, [0, mm(200), 0]).expect("the wall"));
    let v = e.volume(&s).expect("the volume");
    assert_eq!(cubic_steps(v), I256::from_i128(six_times(EXPECTED_MM3)));
}

/// A vector in the profile's plane is refused as an input.
pub fn a_vector_in_the_profile_s_plane_is_an_invalid_input<E: Checked>(mut e: E) {
    let profile = Profile {
        outer: rectangle(0, 0, 4000, 200, 0),
        holes: vec![],
    };
    let found = e.extrude(&profile, [mm(10), 0, 0]);
    assert!(
        matches!(
            found,
            Err(EngineError::Invalid {
                operation: "extrude",
                ..
            })
        ),
        "an extrusion along its own plane was not refused as an input"
    );
}

/// The display mesh is closed: with its vertices joined where their positions are equal bit for
/// bit, every edge is used by exactly two triangles, once each way. Its bounds are the wall's,
/// every normal is a unit vector on its triangle's outward side, and the triangles' signed volume
/// is the wall's, whichever way the wall is drawn.
pub fn the_mesh_is_closed_and_bounded_by_the_wall<E: Checked>(mut e: E) {
    for drawn in BOTH {
        closed_mesh(&mut e, drawn);
    }
}

/// [`the_mesh_is_closed_and_bounded_by_the_wall`] for the wall drawn `drawn`.
#[allow(
    clippy::cast_precision_loss,
    reason = "test arithmetic on grid coordinates under 2^40 and a volume compared to 1e-12"
)]
fn closed_mesh<E: Checked>(e: &mut E, drawn: Drawn) {
    let s = wall_with_door(e, drawn);
    let m = e.mesh(&s).expect("the mesh");
    assert_eq!(m.positions.len(), m.normals.len());
    assert!(!m.triangles.is_empty());
    let mut ids: BTreeMap<[u64; 3], usize> = BTreeMap::new();
    let id: Vec<usize> = m
        .positions
        .iter()
        .map(|&p| {
            let n = ids.len();
            *ids.entry(p.map(f64::to_bits)).or_insert(n)
        })
        .collect();
    let mut counts: BTreeMap<(usize, usize), (u32, i32)> = BTreeMap::new();
    let mut six = 0.0_f64;
    for t in &m.triangles {
        let [a, b, c] = t.map(|i| usize::try_from(i).expect("an index"));
        for (u, v) in [(a, b), (b, c), (c, a)] {
            let (u, v) = (id[u], id[v]);
            assert_ne!(u, v, "a triangle with two corners at one point");
            let entry = counts.entry((u.min(v), u.max(v))).or_default();
            entry.0 += 1;
            entry.1 += if u < v { 1 } else { -1 };
        }
        let [p, q, r] = [a, b, c].map(|i| m.positions[i]);
        let cross = [
            (q[1] - p[1]) * (r[2] - p[2]) - (q[2] - p[2]) * (r[1] - p[1]),
            (q[2] - p[2]) * (r[0] - p[0]) - (q[0] - p[0]) * (r[2] - p[2]),
            (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]),
        ];
        for i in [a, b, c] {
            let n = m.normals[i];
            let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((length - 1.0).abs() < 1e-12, "{n:?}");
            let dot: f64 = n.iter().zip(cross).map(|(x, y)| x * y).sum();
            assert!(dot > 0.0, "a normal against its triangle");
        }
        six += p[0] * cross[0] + p[1] * cross[1] + p[2] * cross[2];
    }
    assert!(
        counts.values().all(|&(n, _)| n == 2),
        "an edge not used by two triangles"
    );
    assert!(
        counts.values().all(|&(_, w)| w == 0),
        "an edge used twice the same way"
    );
    let bound = |pick: fn(f64, f64) -> f64, from: f64| {
        (0..3)
            .map(|i| m.positions.iter().map(|p| p[i]).fold(from, pick))
            .collect::<Vec<_>>()
    };
    let to = |p: P3| vec![p.x.get() as f64, p.y.get() as f64, p.z.get() as f64];
    assert_eq!(bound(f64::min, f64::MAX), to(at(0, 0, 0)));
    assert_eq!(bound(f64::max, f64::MIN), to(at(4000, 200, 2700)));
    let expected = six_times(EXPECTED_MM3) as f64;
    assert!(
        (six - expected).abs() <= expected * 1e-12,
        "{six} {expected}"
    );
}

/// The same input gives the same bytes: two fresh engines, each making the wall with its door,
/// give the same volume and the same mesh, bit for bit.
pub fn the_same_input_gives_the_same_bytes<E: Checked>(make: impl Fn() -> E) {
    let bytes = || {
        let mut e = make();
        let mut out = Vec::new();
        for drawn in BOTH {
            let s = wall_with_door(&mut e, drawn);
            let v = e.volume(&s).expect("the volume");
            let m = e.mesh(&s).expect("the mesh");
            out.extend(format!("{v:?}").into_bytes());
            for p in m.positions.iter().chain(&m.normals) {
                for c in p {
                    out.extend(c.to_bits().to_le_bytes());
                }
            }
            for t in &m.triangles {
                for i in t {
                    out.extend(i.to_le_bytes());
                }
            }
        }
        out
    };
    let first = bytes();
    assert!(!first.is_empty());
    assert_eq!(first, bytes());
}
