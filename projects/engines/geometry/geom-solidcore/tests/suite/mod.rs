//! The engine suite: the building cases of the engine seam, written over `geom_op::engine::Engine`
//! alone, so that any engine is judged by them. A test file runs an engine through it by
//! implementing [`Checked`] for the engine (how to read the kernel's body of its solid) and
//! calling each case with a way to make a fresh engine. Every solid an engine returns is held to
//! the kernel's checker at its full level here as well, whatever the engine says it checked.
//!
//! The cases: a wall of 4000 by 2700 by 200 mm with a door opening of 900 by 2100 mm, and the
//! envelope cases, strips of a sloped profile (a rise of 3 m, then 45 degrees to 6 m in) lofted
//! along a site's boundaries, united, cut, and set against each other at slivers of 0.5, 1 and
//! 3 mm, on the building grid `Q0`, where one millimetre is 6,400 steps. An exact engine of the kernel's own
//! runs the same file (`#[path]` from its own tests) on the day it takes an operation over.

use std::collections::BTreeMap;

use geom_check::{Level, check};
use geom_num::{GridStep, I256, P3};
use geom_op::engine::{Engine, EngineError, Profile, Section, Volume};
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

// The envelope cases: the permitted volume above a site's boundaries, a sloped profile lofted
// along each boundary line (a vertical rise at the boundary, then a plane at 45 degrees inward),
// the strips united, cut and set against each other at the sliver sizes such work meets.

/// The envelope's vertical rise at the boundary, in millimetres.
pub const RISE: i64 = 3000;

/// How far inward the envelope's profile runs, in millimetres: its 45 degree plane rises as far.
pub const DEPTH: i64 = 6000;

/// The envelope's profile as (run inward from the boundary, height above the ground) in
/// millimetres: up the rise, along the 45 degree plane, and down to the ground at its depth.
pub const ENVELOPE: [(i64, i64); 4] = [(0, 0), (0, RISE), (DEPTH, RISE + DEPTH), (DEPTH, 0)];

/// The area of [`ENVELOPE`] in square millimetres: the rise's rectangle and the slope's triangle.
const ENVELOPE_MM2: i128 = (RISE * DEPTH + DEPTH * DEPTH / 2) as i128;

/// Where two strips at a square corner overlap, in cubic millimetres: under the lower of the two
/// slopes over the square of the depth, `RISE * DEPTH^2 + DEPTH^3 / 3`.
const CORNER_MM3: i128 = (RISE * DEPTH * DEPTH + DEPTH * DEPTH * DEPTH / 3) as i128;

/// The section of the envelope at the boundary point `(x, y, z)` millimetres, its profile running
/// along `inward` (a unit step along x or y) and up.
pub fn section(x: i64, y: i64, z: i64, inward: (i64, i64)) -> Section {
    Section {
        corners: ENVELOPE
            .iter()
            .map(|&(run, h)| at(x + run * inward.0, y + run * inward.1, z + h))
            .collect(),
    }
}

/// The strip of the envelope along the straight boundary from `from` to `to` (plan points in
/// millimetres) at ground height `z`, its profile running `inward`, through `stations` sections
/// spaced evenly along the line (the boundary's length a whole multiple of the spacing).
pub fn strip<E: Checked>(
    e: &mut E,
    from: (i64, i64),
    to: (i64, i64),
    z: i64,
    inward: (i64, i64),
    stations: i64,
) -> Result<E::Solid, EngineError> {
    let gaps = stations - 1;
    let sections: Vec<Section> = (0..stations)
        .map(|k| {
            let x = from.0 + (to.0 - from.0) * k / gaps;
            let y = from.1 + (to.1 - from.1) * k / gaps;
            section(x, y, z, inward)
        })
        .collect();
    e.loft(&sections)
}

/// Six times the volume, in cubic steps, of a solid the kernel's checker passes.
fn six_volume<E: Checked>(e: &mut E, s: &E::Solid) -> I256 {
    cubic_steps(e.volume(s).expect("the volume"))
}

/// One strip of the envelope along a straight boundary of 20 m, lofted through its two end
/// sections, through five sections 5 m apart, and with its sections listed the other way round,
/// is a solid whose volume is the profile's area times the boundary's length, exactly.
pub fn a_sloped_strip_along_a_straight_boundary_is_its_profile_times_its_length<E: Checked>(
    mut e: E,
) {
    let expected = I256::from_i128(six_times(ENVELOPE_MM2 * 20_000));
    for stations in [2, 5] {
        let s = checked::<E>(
            strip(&mut e, (0, 0), (20_000, 0), 0, (0, 1), stations).expect("the strip"),
        );
        assert_eq!(six_volume(&mut e, &s), expected, "{stations} sections");
    }
    let mut sections = vec![section(0, 0, 0, (0, 1)), section(20_000, 0, 0, (0, 1))];
    for s in &mut sections {
        s.corners.reverse();
    }
    let s = checked::<E>(e.loft(&sections).expect("the strip wound the other way"));
    assert_eq!(six_volume(&mut e, &s), expected);
}

/// A boundary with one bend: strips along two lines meeting at a square corner, each running the
/// line's whole length, unite into one solid of both strips less their overlap at the corner.
pub fn two_strips_meeting_at_a_corner_unite_into_one_solid<E: Checked>(mut e: E) {
    let a = checked::<E>(strip(&mut e, (0, 0), (20_000, 0), 0, (0, 1), 2).expect("along x"));
    let b = checked::<E>(strip(&mut e, (0, 0), (0, 16_000), 0, (1, 0), 2).expect("along y"));
    let u = checked::<E>(e.unite(&a, &b).expect("the two strips united"));
    let mm3 = ENVELOPE_MM2 * (20_000 + 16_000) - CORNER_MM3;
    assert_eq!(six_volume(&mut e, &u), I256::from_i128(six_times(mm3)));
}

/// A rectangular site of 30 by 24 m: its four strips, each along one boundary and inward, united
/// one after another into one closed solid (a ring round the site), whose volume is the four
/// strips less the four corners where neighbours overlap, exactly.
pub fn four_strips_round_a_rectangle_site_unite_into_one_closed_solid<E: Checked>(mut e: E) {
    let (w, l) = (30_000, 24_000);
    let sides = [
        ((0, 0), (w, 0), (0, 1)),
        ((w, 0), (w, l), (-1, 0)),
        ((w, l), (0, l), (0, -1)),
        ((0, l), (0, 0), (1, 0)),
    ];
    let mut strips = Vec::new();
    for (from, to, inward) in sides {
        strips.push(checked::<E>(
            strip(&mut e, from, to, 0, inward, 2).expect("a strip"),
        ));
    }
    let mut strips = strips.into_iter();
    let mut all = strips.next().expect("the first strip");
    for s in strips {
        all = checked::<E>(e.unite(&all, &s).expect("the strips united"));
    }
    let mm3 = ENVELOPE_MM2 * i128::from(2 * (w + l)) - 4 * CORNER_MM3;
    assert_eq!(six_volume(&mut e, &all), I256::from_i128(six_times(mm3)));
}

/// A strip cut by a box: the box (3 m along the boundary, from 2 m to 7 m inward, from below the
/// ground to above the envelope) takes the strip's share under the envelope from 2 m to the
/// profile's 6 m depth, `3000 * (RISE * 4000 + (6000^2 - 2000^2) / 2)` cubic millimetres.
pub fn a_strip_cut_by_a_box_loses_the_box_s_share<E: Checked>(mut e: E) {
    let s = checked::<E>(strip(&mut e, (0, 0), (20_000, 0), 0, (0, 1), 2).expect("the strip"));
    let profile = Profile {
        outer: rectangle(5000, 2000, 8000, 7000, -1000),
        holes: vec![],
    };
    let cutter = checked::<E>(e.extrude(&profile, [0, 0, mm(21_000)]).expect("the box"));
    let cut = checked::<E>(e.subtract(&s, &cutter).expect("the strip less the box"));
    let taken = 3000 * (i128::from(RISE) * 4000 + (6000 * 6000 - 2000 * 2000) / 2);
    let mm3 = ENVELOPE_MM2 * 20_000 - taken;
    assert_eq!(six_volume(&mut e, &cut), I256::from_i128(six_times(mm3)));
}

/// Two strips end to end along one boundary, sharing the section between them exactly, unite into
/// one solid of the whole strip's volume, with the face count of the whole strip lofted at once:
/// the shared face is gone and the walls either side of it are one face each.
pub fn two_strips_sharing_an_exactly_coincident_face_unite_into_one<E: Checked>(mut e: E) {
    let a = checked::<E>(strip(&mut e, (0, 0), (10_000, 0), 0, (0, 1), 2).expect("the first"));
    let b =
        checked::<E>(strip(&mut e, (10_000, 0), (20_000, 0), 0, (0, 1), 2).expect("the second"));
    let u = checked::<E>(e.unite(&a, &b).expect("the strips united"));
    let whole = checked::<E>(strip(&mut e, (0, 0), (20_000, 0), 0, (0, 1), 2).expect("whole"));
    assert_eq!(
        six_volume(&mut e, &u),
        I256::from_i128(six_times(ENVELOPE_MM2 * 20_000))
    );
    let faces = |s: &E::Solid| E::body(s).0.arenas().faces.len();
    assert_eq!(faces(&u), faces(&whole));
}

/// How two strips end to end along one boundary are set against each other at a sliver's size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sliver {
    /// The second strip raised by the sliver, still sharing the plane of the section between them:
    /// how a join is nudged when the strips' union fails.
    Raised,
    /// The second strip starting the sliver before the first ends: a thin overlap.
    Overlapping,
    /// The second strip starting the sliver after the first ends: a thin gap.
    Apart,
}

/// What uniting two strips at a sliver gave: a closed solid, whose volume the suite has checked
/// against the closed form, or the engine's error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A body the kernel's checker passes as a solid, of the closed form's volume, bounded by
    /// `shells` shells: one for a single piece, two for two pieces with a gap between them.
    Closed {
        /// How many shells bound the body's material.
        shells: u32,
    },
    /// The engine's error.
    Failed(EngineError),
}

/// The sliver sizes, in tenths of a millimetre: 0.5, 1 and 3 mm.
pub const SLIVERS: [i64; 3] = [5, 10, 30];

/// The two strips of 10 m along x set against each other by `sliver` of `tenths` tenths of a
/// millimetre, united. A result the engine returns must be one closed solid of the closed form's
/// volume (the raised and overlapping pairs fill 20 m of profile; the pair apart would fill 20 m
/// less the gap); an error is returned as what happened, never hidden.
pub fn united_at_a_sliver<E: Checked>(e: &mut E, sliver: Sliver, tenths: i64) -> Outcome {
    let d = mm(1) * tenths / 10;
    let first = checked::<E>(strip(e, (0, 0), (10_000, 0), 0, (0, 1), 2).expect("the first strip"));
    let p = |x: i64, y: i64, z: i64| P3::new(x, y, z).expect("a grid point");
    let (x0, x1, lift) = match sliver {
        Sliver::Raised => (mm(10_000), mm(20_000), d),
        Sliver::Overlapping => (mm(10_000) - d, mm(20_000), 0),
        Sliver::Apart => (mm(10_000) + d, mm(20_000), 0),
    };
    let at_x = |x: i64| Section {
        corners: ENVELOPE
            .iter()
            .map(|&(run, h)| p(x, mm(run), mm(h) + lift))
            .collect(),
    };
    let second = checked::<E>(e.loft(&[at_x(x0), at_x(x1)]).expect("the second strip"));
    match e.unite(&first, &second) {
        Ok(u) => {
            let u = checked::<E>(u);
            let area = ENVELOPE_MM2 * i128::from(mm(1)) * i128::from(mm(1));
            let length = match sliver {
                Sliver::Raised | Sliver::Overlapping => mm(20_000),
                Sliver::Apart => mm(20_000) - d,
            };
            let six = 6 * area * i128::from(length);
            assert_eq!(
                six_volume(e, &u),
                I256::from_i128(six),
                "{sliver:?} {tenths}"
            );
            Outcome::Closed {
                shells: E::body(&u).0.arenas().shells.len(),
            }
        }
        Err(err) => Outcome::Failed(err),
    }
}

/// A loft whose sections fold back on themselves, the third section back between the first two,
/// is reported as an invalid input (a section not wholly ahead of the one before it), never
/// returned as a solid.
pub fn a_loft_that_folds_back_is_a_fault_not_a_solid<E: Checked>(mut e: E) {
    let sections = [
        section(0, 0, 0, (0, 1)),
        section(10_000, 0, 0, (0, 1)),
        section(5000, 0, 0, (0, 1)),
    ];
    let found = e.loft(&sections);
    assert!(
        matches!(
            &found,
            Err(EngineError::Invalid {
                operation: "loft",
                what,
            }) if what.contains("folds back")
        ),
        "a folded loft was not refused: {:?}",
        found.as_ref().err()
    );
}

/// A loft whose walls cross, the second section's corners listed half a turn round from the
/// first's so that every wall passes through the strip's axis (each wall still flat), is reported
/// as a fault, never returned as a solid.
pub fn a_loft_whose_walls_cross_is_a_fault_not_a_solid<E: Checked>(mut e: E) {
    let square = |x: i64| {
        vec![
            at(x, 0, 0),
            at(x, 2000, 0),
            at(x, 2000, 2000),
            at(x, 0, 2000),
        ]
    };
    let first = Section { corners: square(0) };
    let mut turned = square(10_000);
    turned.rotate_left(2);
    let found = e.loft(&[first, Section { corners: turned }]);
    assert!(
        matches!(
            found,
            Err(EngineError::Refused {
                operation: "loft",
                ..
            })
        ),
        "a crossed loft was not refused: {:?}",
        found.as_ref().err()
    );
}

/// Sections of different corner counts, and a single section, are not inputs a loft takes.
pub fn sections_of_different_counts_are_an_invalid_input<E: Checked>(mut e: E) {
    let mut short = section(10_000, 0, 0, (0, 1));
    short.corners.pop();
    let found = e.loft(&[section(0, 0, 0, (0, 1)), short]);
    assert!(
        matches!(
            found,
            Err(EngineError::Invalid {
                operation: "loft",
                ..
            })
        ),
        "{:?}",
        found.as_ref().err()
    );
    let one = e.loft(&[section(0, 0, 0, (0, 1))]);
    assert!(
        matches!(
            one,
            Err(EngineError::Invalid {
                operation: "loft",
                ..
            })
        ),
        "{:?}",
        one.as_ref().err()
    );
}

/// The face count of a loft of `stations` sections of `corners` corners each, one face for each
/// wall between two sections and one for each end.
fn loft_faces(stations: u32, corners: u32) -> u32 {
    2 + (stations - 1) * corners
}

/// The envelope's section over ground that rises by `raised` millimetres at the middle of three
/// stations 500 mm apart, lofted through all three at once: the walls bend between the stations
/// by under half a degree at 2 mm (0.46 degrees), not at all at 0 mm, and by 1.15 degrees at
/// 5 mm. Each loft is one closed solid with one flat face for each wall between two stations and
/// one for each end however little the walls turn, its volume the profile's area times its
/// length, as a shear keeps volume.
pub fn a_loft_whose_walls_bend_less_than_half_a_degree_is_one_face_per_wall<E: Checked>(mut e: E) {
    let expected = I256::from_i128(six_times(ENVELOPE_MM2 * 1000));
    for raised in [2, 0, 5] {
        let s = checked::<E>(
            e.loft(&[
                section(0, 0, 0, (0, 1)),
                section(500, 0, raised, (0, 1)),
                section(1000, 0, 0, (0, 1)),
            ])
            .unwrap_or_else(|err| panic!("raised {raised} mm: {err:?}")),
        );
        assert_eq!(six_volume(&mut e, &s), expected, "raised {raised} mm");
        assert_eq!(E::body(&s).0.arenas().faces.len(), loft_faces(3, 4));
    }
}

/// A section two of whose sides turn by under half a degree: a rise of 3 m, then 45 degrees to
/// 6 m in, then on to 12 m in, turning by 0.05 degrees (the last corner 10 mm above the slope's
/// line), lofted 500 mm. The loft is one closed solid with each of the two sloped walls its own
/// flat face, its volume the section's area times its length; with the sides exactly in line it
/// is the same, the trapezoid's area times the length.
pub fn a_section_whose_sides_turn_less_than_half_a_degree_lofts_to_one_face_per_side<E: Checked>(
    mut e: E,
) {
    // The section's area in square millimetres: the trapezoid under the line to 15 m, and the
    // triangle of the last corner's 10 mm above it over 6 m.
    for (top, mm2) in [(15_010, 108_030_000), (15_000, 108_000_000)] {
        let section = |x: i64| Section {
            corners: [(0, 0), (0, 3000), (6000, 9000), (12_000, top), (12_000, 0)]
                .iter()
                .map(|&(r, h)| at(x, r, h))
                .collect(),
        };
        let s = checked::<E>(
            e.loft(&[section(0), section(500)])
                .unwrap_or_else(|err| panic!("top {top}: {err:?}")),
        );
        assert_eq!(
            six_volume(&mut e, &s),
            I256::from_i128(six_times(mm2 * 500)),
            "top {top}"
        );
        assert_eq!(E::body(&s).0.arenas().faces.len(), loft_faces(2, 5));
    }
}

/// The envelope's strip of 20 m along x, lofted through eleven sections 2 m apart over ground that
/// rises and falls by a few millimetres between them (every wall turning by under half a degree
/// from the next), is one closed solid of one face per wall between two sections and per end,
/// the profile's area times its length.
pub fn a_loft_over_uneven_ground_is_its_profile_times_its_length<E: Checked>(mut e: E) {
    let ground = [0, 3, 1, 4, 1, 5, 9, 2, 6, 5, 3];
    let sections: Vec<Section> = ground
        .iter()
        .zip(0_i64..)
        .map(|(&z, k)| section(2000 * k, 0, z, (0, 1)))
        .collect();
    let s = checked::<E>(e.loft(&sections).expect("the strip over uneven ground"));
    assert_eq!(
        six_volume(&mut e, &s),
        I256::from_i128(six_times(ENVELOPE_MM2 * 20_000))
    );
    assert_eq!(E::body(&s).0.arenas().faces.len(), loft_faces(11, 4));
}

/// The same loft gives the same bytes: two fresh engines, each lofting the strip over uneven
/// ground, give the same volume and the same mesh, bit for bit.
pub fn the_same_loft_gives_the_same_bytes<E: Checked>(make: impl Fn() -> E) {
    let bytes = || {
        let mut e = make();
        let sections: Vec<Section> = [0, 2, 0, 5, 0]
            .iter()
            .zip(0_i64..)
            .map(|(&z, k)| section(500 * k, 0, z, (0, 1)))
            .collect();
        let s = e.loft(&sections).expect("the loft");
        let v = e.volume(&s).expect("the volume");
        let m = e.mesh(&s).expect("the mesh");
        let mut out = format!("{v:?}").into_bytes();
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
        out
    };
    let first = bytes();
    assert!(!first.is_empty());
    assert_eq!(first, bytes());
}

/// The box that cuts the strip of 20 m along x across its whole width and height from x = 8 m to
/// x = 12 m: from 1 m outside the boundary to 1 m past the profile's depth, from 1 m under the
/// ground to 1 m over the envelope.
fn across<E: Checked>(e: &mut E, x0: i64, x1: i64) -> E::Solid {
    let profile = Profile {
        outer: rectangle(x0, -1000, x1, DEPTH + 1000, -1000),
        holes: vec![],
    };
    checked::<E>(
        e.extrude(&profile, [0, 0, mm(RISE + DEPTH + 2000)])
            .expect("the box across"),
    )
}

/// A strip cut in two: the box across it from 8 m to 12 m leaves two pieces, the strip's first
/// 8 m and its last 8 m, each one closed solid of the profile's area times 8 m, the first piece
/// first (its least corner, the boundary's start, is the least); a box across the whole strip
/// leaves none. The one-solid subtraction gives the cut in two as one solid of two shells, of
/// both pieces' volume.
pub fn a_strip_cut_in_two_leaves_two_pieces_in_order<E: Checked>(mut e: E) {
    let s = checked::<E>(strip(&mut e, (0, 0), (20_000, 0), 0, (0, 1), 2).expect("the strip"));
    let cutter = across(&mut e, 8000, 12_000);
    let pieces = e.subtract_pieces(&s, &cutter).expect("the pieces");
    assert_eq!(pieces.len(), 2);
    let piece = I256::from_i128(six_times(ENVELOPE_MM2 * 8000));
    let least = |p: &E::Solid| {
        let b = E::body(p).0;
        (0..b.arenas().vertices.len())
            .filter_map(geom_topo::VertexIx::new)
            .filter_map(|v| geom_topo::query::point(b, v).ok())
            .map(|q| (q.x.get(), q.y.get(), q.z.get()))
            .min()
    };
    let mut leasts = Vec::new();
    for p in pieces {
        let p = checked::<E>(p);
        assert_eq!(six_volume(&mut e, &p), piece);
        leasts.push(least(&p));
    }
    assert_eq!(
        leasts,
        [Some((0, 0, 0)), Some((mm(12_000), 0, 0))],
        "the pieces in order of their corners"
    );
    let one = checked::<E>(e.subtract(&s, &cutter).expect("the strip less the box"));
    assert_eq!(E::body(&one).0.arenas().shells.len(), 2);
    assert_eq!(
        six_volume(&mut e, &one),
        I256::from_i128(six_times(ENVELOPE_MM2 * 16_000))
    );
    let all = across(&mut e, -1000, 21_000);
    assert_eq!(e.subtract_pieces(&s, &all).map(|p| p.len()), Ok(0));
}

/// How two boxes of 1 m are set against each other: sharing only an edge, or only a corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Touch {
    /// The second box beside the first along x and y at once: they share one vertical edge.
    AlongAnEdge,
    /// The second box beside the first along x, y and z at once: they share one corner.
    AtAPoint,
}

/// Two boxes of 1 m that touch only as `touch` says, united: as two envelopes cut at a corner
/// meet along the corner's vertical line, the union is non-manifold there, so it has no solid
/// answer and the engine's error is returned, never a solid. Two boxes sharing a face, the same
/// boxes moved, unite into one solid of both volumes.
pub fn united_touching<E: Checked>(e: &mut E, touch: Touch) -> EngineError {
    let cube = |e: &mut E, x: i64, y: i64, z: i64| {
        let profile = Profile {
            outer: rectangle(x, y, x + 1000, y + 1000, z),
            holes: vec![],
        };
        checked::<E>(e.extrude(&profile, [0, 0, mm(1000)]).expect("a box"))
    };
    let first = cube(e, 0, 0, 0);
    let beside = cube(e, 1000, 0, 0);
    let both = checked::<E>(e.unite(&first, &beside).expect("boxes sharing a face"));
    assert_eq!(
        six_volume(e, &both),
        I256::from_i128(six_times(2 * 1000 * 1000 * 1000))
    );
    let second = match touch {
        Touch::AlongAnEdge => cube(e, 1000, 1000, 0),
        Touch::AtAPoint => cube(e, 1000, 1000, 1000),
    };
    match e.unite(&first, &second) {
        Ok(u) => panic!("{touch:?}: a solid of {:?}", E::body(&u).0.kind()),
        Err(err) => err,
    }
}
