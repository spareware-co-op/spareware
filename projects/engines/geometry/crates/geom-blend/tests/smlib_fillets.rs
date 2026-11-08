//! G262a: no case the feasibility plan judges feasible fails on `SMLib`'s own fillet fixtures (the
//! done-when clause of G262, chapter 13.10).
//!
//! The cases are the `my_test_model` calls of `SMLib`'s fillet driver, lifted into
//! `tests/data/smlib_fillets.json` with each case's driver line at usd-brep's pinned commit: the
//! fixture, the radius, the edges by their index in `SmBrep::GetEdges`, include or exclude, and
//! what `SMLib`'s reference build gave for the case, run twice in fresh processes through the oracle
//! harness (status, whether it left a body, its edge count). The fixtures are the corpus
//! `smlib-fillet-fixtures`, fetched at test time from that commit and checked against the register's
//! digest, never stored here (decision 32.44).
//!
//! For each case the test reads the fixture with `geom_test::smb` (`SMLib`'s reader, translated),
//! checks that every selected edge is the edge `SMLib` lists at that index, and reads the plan's facts
//! where it can read them exactly: an edge is read when it is straight and both its faces are
//! planar polygons bounded by straight edges, each decided in exact rationals. Then:
//!
//! - the tangent of the half wedge angle, r·cot(θ/2) being where the ball touches each face, from
//!   the two faces' directions into themselves at the edge;
//! - the room each face has across the edge, from every other boundary segment over the edge's
//!   extent, as an open reach limit;
//! - chains across collinear joins, clusters from shared vertices and overlapping bands, and a
//!   shared face's width between parallel blended edges, crowded by their demands;
//!
//! and asks the plan. A case is judged feasible when every cluster is. A case with an edge it cannot
//! read is not judged, never guessed. Every case judged feasible must be one `SMLib` filleted.
//!
//! The test fetches its fixtures, so it is ignored in the ordinary run; `kernel-nightly.yml`'s
//! corpora job runs it with `--ignored`, and `SPAREKIT_CORPORA` names a cache to fetch into.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    clippy::struct_field_names,
    clippy::struct_excessive_bools,
    clippy::option_option,
    clippy::case_sensitive_file_extension_comparisons,
    reason = "test code: a failure should stop loudly, the vectors' names are the formulas', and the               data's fields are the driver's"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use dashu_ratio::RBig;
use geom_blend::chain::{EdgeId, EdgeRow, Join, Tangency, VertexId, chains};
use geom_blend::cluster::{FaceId, Footprint, Patch, clusters};
use geom_blend::overflow::Policy;
use geom_blend::plan::{ChainFacts, SharedFace, plan};
use geom_blend::radius::{Limit, Verdict, reach_limit};
use geom_num::interval::Interval;
use geom_test::corpus::{Manifest, fetch};
use geom_test::smb::{self, Smb};
use serde::Deserialize;

/// The lifted driver data.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    format: u32,
    source: Source,
    corpus: String,
    oracle: Oracle,
    cases: Vec<Case>,
    not_lifted: Vec<NotLifted>,
}

/// Where the data comes from.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    name: String,
    link: String,
    licence: String,
    copyright: String,
    taken: String,
}

/// How the outcomes were found.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Oracle {
    build: String,
    harness: String,
    runs: u32,
    agreed: bool,
}

/// A driver case that is not a `my_test_model` call, with why it is left out.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NotLifted {
    suite: u32,
    case: u32,
    line: u32,
    why: String,
}

/// One `my_test_model` call.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    suite: u32,
    case: u32,
    line: u32,
    file: String,
    radius: f64,
    edges: Vec<[usize; 2]>,
    include: bool,
    solver: String,
    skipped_for: Vec<String>,
    rebuild_uv: bool,
    global_merge: bool,
    chamfer: bool,
    bevel_corners: bool,
    expected_edges: u64,
    smlib: Outcome,
    fixture_edges: usize,
    selected: Vec<Selected>,
}

/// What `SMLib`'s reference build gave.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    status: i64,
    body: bool,
    edges: i64,
}

/// An edge the driver fillets: its index, and its ends as `SMLib` lists them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selected {
    index: usize,
    start: [f64; 3],
    end: [f64; 3],
}

/// `SM_SUCCESS`.
const SUCCESS: i64 = 1000;

// ---- exact vectors ----------------------------------------------------------------------------

type V = [RBig; 3];

fn rat(x: f64) -> RBig {
    RBig::try_from(x).unwrap()
}

fn point(p: [f64; 3]) -> V {
    p.map(rat)
}

fn sub(a: &V, b: &V) -> V {
    [&a[0] - &b[0], &a[1] - &b[1], &a[2] - &b[2]]
}

fn cross(a: &V, b: &V) -> V {
    [
        &a[1] * &b[2] - &a[2] * &b[1],
        &a[2] * &b[0] - &a[0] * &b[2],
        &a[0] * &b[1] - &a[1] * &b[0],
    ]
}

fn dot(a: &V, b: &V) -> RBig {
    &a[0] * &b[0] + &a[1] * &b[1] + &a[2] * &b[2]
}

fn is_zero(a: &V) -> bool {
    a.iter().all(|x| *x == RBig::ZERO)
}

fn neg(a: &V) -> V {
    [-&a[0], -&a[1], -&a[2]]
}

/// An enclosure of an exact rational.
fn enclose(x: &RBig) -> Interval {
    let near = x.to_f64().value();
    Interval::new(near.next_down(), near.next_up()).unwrap()
}

fn enclose_v(a: &V) -> [Interval; 3] {
    [enclose(&a[0]), enclose(&a[1]), enclose(&a[2])]
}

fn idot(a: &[Interval; 3], b: &[Interval; 3]) -> Interval {
    a[0].mul(b[0])
        .unwrap()
        .add(a[1].mul(b[1]).unwrap())
        .unwrap()
        .add(a[2].mul(b[2]).unwrap())
        .unwrap()
}

// ---- the B-rep's shape, read exactly ----------------------------------------------------------

/// A planar face bounded by straight edges.
struct Flat {
    /// The plane's normal, the one about which the outer loop runs counterclockwise.
    normal: V,
    /// Each loop's directed segments, as vertex indices, and whether that loop runs counterclockwise
    /// about `normal`.
    loops: Vec<(Vec<(u64, u64)>, bool)>,
}

/// Whether the 3D curve of edge `e` is a straight segment: its control points on one line.
fn straight(smb: &Smb, e: usize) -> bool {
    let Some(edge) = smb.edges.get(e) else {
        return false;
    };
    let Some(curve) = smb.curves.get(edge.curve as usize) else {
        return false;
    };
    if curve.dim != 3 || curve.weights.is_some() {
        return false;
    }
    let pts: Vec<V> = curve
        .points
        .iter()
        .map(|p| [rat(p[0]), rat(p[1]), rat(p[2])])
        .collect();
    let Some(a) = pts.first() else {
        return false;
    };
    let Some(far) = pts.iter().map(|p| sub(p, a)).find(|d| !is_zero(d)) else {
        return false;
    };
    pts.iter().all(|p| is_zero(&cross(&sub(p, a), &far)))
}

/// Newell's normal of a closed polygon, twice its vector area.
fn newell(points: &[V]) -> V {
    let mut n = [RBig::ZERO, RBig::ZERO, RBig::ZERO];
    for (i, p) in points.iter().enumerate() {
        let q = &points[(i + 1) % points.len()];
        let c = cross(p, q);
        for k in 0..3 {
            n[k] = &n[k] + &c[k];
        }
    }
    n
}

/// The directed segments of loop `l`: each edge use's edge from its start to its end, or back.
fn segments(smb: &Smb, l: usize) -> Option<Vec<(u64, u64)>> {
    let lp = smb.loops.get(l)?;
    if lp.loop_type != 0 {
        return None;
    }
    let mut out = Vec::new();
    for u in lp.start_edgeuse..lp.start_edgeuse + lp.num_edgeuses {
        let eu = smb.edgeuses.get(u as usize)?;
        if eu.eu_type != 0 {
            return None;
        }
        let edge = smb.edges.get(eu.edge as usize)?;
        if !straight(smb, eu.edge as usize) {
            return None;
        }
        out.push(if eu.orientation == 1 {
            (edge.start, edge.end)
        } else {
            (edge.end, edge.start)
        });
    }
    // A loop's segments follow each other.
    for (i, s) in out.iter().enumerate() {
        if s.1 != out[(i + 1) % out.len()].0 {
            return None;
        }
    }
    Some(out)
}

/// Face `f` as a flat polygon, or `None` unless its surface's control points lie in one plane and
/// every edge of its loops is straight.
fn flat(smb: &Smb, f: usize) -> Option<Flat> {
    let face = smb.faces.get(f)?;
    let s = smb.surfaces.get(face.surface as usize)?;
    if s.weights.is_some() {
        return None;
    }
    let pts: Vec<V> = s.points.iter().map(|p| point(*p)).collect();
    let a = pts.first()?;
    let b = pts.iter().find(|p| !is_zero(&sub(p, a)))?;
    let ab = sub(b, a);
    let normal = pts
        .iter()
        .map(|p| cross(&ab, &sub(p, a)))
        .find(|n| !is_zero(n))?;
    if !pts.iter().all(|p| dot(&normal, &sub(p, a)) == RBig::ZERO) {
        return None;
    }
    let mut loops = Vec::new();
    for l in face.start_loop..face.start_loop + face.num_loops {
        let segs = segments(smb, l as usize)?;
        let ring: Vec<V> = segs
            .iter()
            .map(|(v, _)| point(smb.vertices[*v as usize].point))
            .collect();
        loops.push((segs, dot(&newell(&ring), &normal)));
    }
    // Orient the normal so that the outer loop runs counterclockwise about it.
    let outer = loops.first()?.1.clone();
    if outer == RBig::ZERO {
        return None;
    }
    let normal = if outer > RBig::ZERO {
        normal
    } else {
        neg(&normal)
    };
    let outer_sign = outer > RBig::ZERO;
    let loops = loops
        .into_iter()
        .map(|(segs, area)| {
            let ccw = (area > RBig::ZERO) == outer_sign;
            (segs, ccw)
        })
        .collect();
    Some(Flat { normal, loops })
}

/// One side of a blended edge: its face, the direction into the face, perpendicular to the edge,
/// and the room the face has across it, both unnormalised.
struct Side {
    face: usize,
    into: V,
    room: Option<RBig>,
}

/// The two sides of edge `e`, or `None` if it is not read.
fn sides(smb: &Smb, flats: &BTreeMap<usize, Option<Flat>>, e: usize) -> Option<[Side; 2]> {
    let edge = smb.edges.get(e)?;
    let a = point(smb.vertices.get(edge.start as usize)?.point);
    let b = point(smb.vertices.get(edge.end as usize)?.point);
    let t = sub(&b, &a);
    if is_zero(&t) || !straight(smb, e) {
        return None;
    }
    let mut out = Vec::new();
    for u in smb.edges_of(e as u64) {
        let eu = &smb.edgeuses[u];
        let f = smb.face_of_loop(eu.loop_index)?;
        let flat = flats.get(&f)?.as_ref()?;
        // The loop's direction along this edge, and the face on its left when the loop runs
        // counterclockwise about the normal.
        let ccw = flat
            .loops
            .iter()
            .find(|(segs, _)| {
                segs.iter()
                    .any(|s| *s == (edge.start, edge.end) || *s == (edge.end, edge.start))
            })?
            .1;
        let along = if eu.orientation == 1 {
            t.clone()
        } else {
            neg(&t)
        };
        let left = cross(&flat.normal, &along);
        let into = if ccw { left } else { neg(&left) };
        let room = room(smb, flat, edge.start, edge.end, &a, &t, &into)?;
        out.push(Side {
            face: f,
            into,
            room,
        });
    }
    let [s1, s2]: [Side; 2] = out.try_into().ok()?;
    if s1.face == s2.face {
        return None;
    }
    Some([s1, s2])
}

/// The face's room across the edge from `a` along `t`, in units of |into|: the least height, along
/// `into`, of the face's boundary over the edge's extent. A segment that meets the edge at a vertex
/// counts by its far end alone, and only where that end is over the edge: where the segments leave
/// the edge at its ends is the corner's business. `Some(None)` when nothing bounds it; `None` for a
/// boundary that comes down to the edge.
fn room(
    smb: &Smb,
    flat: &Flat,
    start: u64,
    end: u64,
    a: &V,
    t: &V,
    into: &V,
) -> Option<Option<RBig>> {
    let tt = dot(t, t);
    let at = |v: u64| point(smb.vertices[v as usize].point);
    let s_of = |p: &V| dot(&sub(p, a), t) / &tt;
    let h_of = |p: &V| dot(&sub(p, a), into);
    let mut least: Option<RBig> = None;
    let take = |h: RBig, least: &mut Option<RBig>| {
        if least.as_ref().is_none_or(|l| h < *l) {
            *least = Some(h);
        }
    };
    let zero = RBig::ZERO;
    let one = RBig::ONE;
    for (segs, _) in &flat.loops {
        for &(p, q) in segs {
            if (p, q) == (start, end) || (p, q) == (end, start) {
                continue;
            }
            let touches = [p, q].iter().any(|v| *v == start || *v == end);
            let (pp, qq) = (at(p), at(q));
            if touches {
                let far = if p == start || p == end { &qq } else { &pp };
                let s = s_of(far);
                if s >= zero && s <= one {
                    take(h_of(far), &mut least);
                }
                continue;
            }
            let (sp, sq) = (s_of(&pp), s_of(&qq));
            let (hp, hq) = (h_of(&pp), h_of(&qq));
            if (sp < zero && sq < zero) || (sp > one && sq > one) {
                continue;
            }
            // The segment clipped to the edge's extent: its least height is at a clipped end.
            for (s, h, so, ho) in [(&sp, &hp, &sq, &hq), (&sq, &hq, &sp, &hp)] {
                if *s >= zero && *s <= one {
                    take(h.clone(), &mut least);
                } else {
                    let bound = if *s < zero { &zero } else { &one };
                    let w = (bound - s) / (so - s);
                    take(h + &(w * (ho - h)), &mut least);
                }
            }
        }
    }
    match least {
        Some(h) if h <= RBig::ZERO => None,
        other => Some(other),
    }
}

/// What a case reads to.
enum Judged {
    /// An edge could not be read; the reason.
    Unread(String),
    /// The plan's answer: feasible or not, and why not.
    Plan { feasible: bool, why: String },
}

/// Reads the facts of `case` from `smb` and asks the plan.
fn judge(smb: &Smb, case: &Case) -> Judged {
    let mut flats: BTreeMap<usize, Option<Flat>> = BTreeMap::new();
    for f in 0..smb.faces.len() {
        flats.insert(f, flat(smb, f));
    }
    let picked: Vec<usize> = case.selected.iter().map(|s| s.index).collect();
    let mut rows = Vec::new();
    let mut per_edge: BTreeMap<usize, ([Side; 2], Interval)> = BTreeMap::new();
    for &e in &picked {
        let Some(sides) = sides(smb, &flats, e) else {
            return Judged::Unread(format!("edge {e}: not straight between two flat faces"));
        };
        // cot(θ/2) = (d1·d2 + |d1||d2|) / |d1 × d2|.
        let (d1, d2) = (enclose_v(&sides[0].into), enclose_v(&sides[1].into));
        let c = cross(&sides[0].into, &sides[1].into);
        if is_zero(&c) {
            return Judged::Unread(format!("edge {e}: its faces meet flat"));
        }
        let top = idot(&d1, &d2)
            .add(
                Interval::norm(&d1)
                    .unwrap()
                    .mul(Interval::norm(&d2).unwrap())
                    .unwrap(),
            )
            .unwrap();
        let bottom = Interval::norm(&enclose_v(&c)).unwrap();
        let k = top.div(bottom).unwrap();
        if k.lo() <= 0.0 {
            return Judged::Unread(format!("edge {e}: its faces are too near tangent to bound"));
        }
        let edge = &smb.edges[e];
        rows.push(EdgeRow {
            id: EdgeId(e as u32),
            start: VertexId(edge.start as u32),
            end: VertexId(edge.end as u32),
        });
        per_edge.insert(e, (sides, k));
    }
    // Joins: picked edges meeting at a vertex, tangent where they are collinear and go on.
    let mut joins = Vec::new();
    for (i, &e1) in picked.iter().enumerate() {
        for &e2 in &picked[i + 1..] {
            let (x, y) = (&smb.edges[e1], &smb.edges[e2]);
            for v in [x.start, x.end] {
                if v != y.start && v != y.end {
                    continue;
                }
                let dir = |edge: &smb::records::Edge| {
                    sub(
                        &point(smb.vertices[edge.end as usize].point),
                        &point(smb.vertices[edge.start as usize].point),
                    )
                };
                let arriving = if x.end == v { dir(x) } else { neg(&dir(x)) };
                let leaving = if y.start == v { dir(y) } else { neg(&dir(y)) };
                let tangent =
                    is_zero(&cross(&arriving, &leaving)) && dot(&arriving, &leaving) > RBig::ZERO;
                joins.push(Join {
                    at: VertexId(v as u32),
                    first: EdgeId(e1 as u32),
                    second: EdgeId(e2 as u32),
                    tangency: if tangent {
                        Tangency::Tangent
                    } else {
                        Tangency::Sharp
                    },
                });
            }
        }
    }
    let chained = match chains(&rows, &joins) {
        Ok(c) => c,
        Err(e) => return Judged::Unread(format!("chains: {e:?}")),
    };
    let r = Interval::point(case.radius).unwrap();
    let mut facts = Vec::new();
    let mut feet = Vec::new();
    let mut radii = Vec::new();
    let mut on_face: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
    for (ci, chain) in chained.iter().enumerate() {
        let mut limits: Vec<Limit> = Vec::new();
        let mut vertices = Vec::new();
        let mut patches = Vec::new();
        for link in &chain.links {
            let e = link.edge.0 as usize;
            let (sides, k) = &per_edge[&e];
            let edge = &smb.edges[e];
            vertices.push(VertexId(edge.start as u32));
            vertices.push(VertexId(edge.end as u32));
            let a = enclose_v(&point(smb.vertices[edge.start as usize].point));
            let b = enclose_v(&point(smb.vertices[edge.end as usize].point));
            for side in sides {
                let into = enclose_v(&side.into);
                let len = Interval::norm(&into).unwrap();
                if let Some(h) = &side.room {
                    let width = enclose(h).div(len).unwrap();
                    limits.push(reach_limit(width, *k, true).unwrap());
                }
                // The band on this face, as a box of its corners in the plane's two coordinates
                // least along its normal.
                let reach = r.mul(*k).unwrap();
                let n = &plane_axes(&flats[&side.face].as_ref().unwrap().normal);
                let mut lo = [f64::INFINITY; 2];
                let mut hi = [f64::NEG_INFINITY; 2];
                for p in [a, b] {
                    for far in [false, true] {
                        for (slot, &axis) in n.iter().enumerate() {
                            let mut x = p[axis];
                            if far {
                                x = x
                                    .add(reach.mul(into[axis].div(len).unwrap()).unwrap())
                                    .unwrap();
                            }
                            lo[slot] = if x.lo() < lo[slot] { x.lo() } else { lo[slot] };
                            hi[slot] = if x.hi() > hi[slot] { x.hi() } else { hi[slot] };
                        }
                    }
                }
                patches.push(Patch {
                    face: FaceId(side.face as u32),
                    reach: [
                        Interval::new(lo[0], hi[0]).unwrap(),
                        Interval::new(lo[1], hi[1]).unwrap(),
                    ],
                });
                on_face.entry(side.face).or_default().push((ci, e));
            }
        }
        facts.push(ChainFacts {
            limits,
            sites: Vec::new(),
        });
        feet.push(Footprint { vertices, patches });
        radii.push(case.radius);
    }
    // A face that several chains share: the width between parallel blended edges on it.
    let mut shared = Vec::new();
    for (&f, users) in &on_face {
        let chains_here: Vec<usize> = {
            let mut c: Vec<usize> = users.iter().map(|(c, _)| *c).collect();
            c.sort_unstable();
            c.dedup();
            c
        };
        if chains_here.len() < 2 {
            continue;
        }
        let mut width: Option<Interval> = None;
        for (i, &(c1, e1)) in users.iter().enumerate() {
            for &(c2, e2) in &users[i + 1..] {
                if c1 == c2 {
                    continue;
                }
                let (x, y) = (&smb.edges[e1], &smb.edges[e2]);
                let ax = point(smb.vertices[x.start as usize].point);
                let tx = sub(&point(smb.vertices[x.end as usize].point), &ax);
                let ay = point(smb.vertices[y.start as usize].point);
                let ty = sub(&point(smb.vertices[y.end as usize].point), &ay);
                if !is_zero(&cross(&tx, &ty)) {
                    continue;
                }
                let side = per_edge[&e1].0.iter().find(|s| s.face == f).unwrap();
                let h = dot(&sub(&ay, &ax), &side.into);
                let len = Interval::norm(&enclose_v(&side.into)).unwrap();
                let w = enclose(&h).div(len).unwrap();
                width = Some(match width {
                    Some(old) if old.lo() <= w.lo() => old,
                    _ => w,
                });
            }
        }
        if let Some(width) = width {
            let demands = chains_here
                .iter()
                .map(|&c| {
                    let e = users.iter().find(|(cc, _)| *cc == c).unwrap().1;
                    (c, per_edge[&e].1)
                })
                .collect();
            shared.push(SharedFace {
                face: FaceId(f as u32),
                width,
                demands,
            });
        }
    }
    let groups = clusters(&feet);
    match plan(&facts, &radii, &groups, &shared, &Policy::default()) {
        Ok(p) => {
            let feasible = p.clusters.iter().all(|c| c.feasible);
            let why = p
                .chains
                .iter()
                .filter(|c| c.verdict != Verdict::Feasible)
                .map(|c| format!("{:?}", c.verdict))
                .chain(
                    p.clusters
                        .iter()
                        .filter(|c| !c.crowded.is_empty())
                        .map(|c| format!("crowded {:?}", c.crowded)),
                )
                .collect::<Vec<_>>()
                .join("; ");
            Judged::Plan { feasible, why }
        }
        Err(e) => Judged::Plan {
            feasible: false,
            why: format!("{e}"),
        },
    }
}

/// The two axes a plane is drawn on: all but its normal's largest component.
fn plane_axes(n: &V) -> [usize; 2] {
    let abs: Vec<RBig> = n
        .iter()
        .map(|x| if *x < RBig::ZERO { -x } else { x.clone() })
        .collect();
    let big = if abs[0] >= abs[1] && abs[0] >= abs[2] {
        0
    } else if abs[1] >= abs[2] {
        1
    } else {
        2
    };
    match big {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    }
}

fn data() -> Data {
    let text = include_str!("data/smlib_fillets.json");
    let d: Data = serde_json::from_str(text).unwrap();
    assert_eq!(d.format, 1);
    d
}

/// The lifted data is whole: every case names its line, an edge selection that is a list of
/// intervals, and the outcome of two runs that agreed; and the cases left out say why.
#[test]
fn the_lifted_cases_are_whole() {
    let d = data();
    assert!(
        d.source
            .link
            .contains("c9c5979f04cea8c38213004d4f04df0dcfb40d05")
    );
    assert_eq!(d.source.licence, "Apache-2.0");
    assert!(d.source.copyright.contains("NVIDIA"));
    assert!(!d.source.name.is_empty() && !d.source.taken.is_empty());
    assert!(!d.oracle.build.is_empty() && !d.oracle.harness.is_empty());
    assert_eq!((d.oracle.runs, d.oracle.agreed), (2, true));
    assert!(d.cases.len() >= 80, "{}", d.cases.len());
    let mut seen = std::collections::BTreeSet::new();
    for c in &d.cases {
        assert!(seen.insert((c.suite, c.case)), "{} {}", c.suite, c.case);
        assert!(c.line > 0 && c.radius > 0.0 && c.file.ends_with(".smb"));
        assert!(c.edges.iter().all(|[a, b]| a <= b));
        assert!(["driver", "constant-distance"].contains(&c.solver.as_str()));
        assert!(c.selected.iter().all(|s| s.index < c.fixture_edges));
        // An include list names the edges filleted; every one filleted is in it.
        if c.include {
            assert!(
                c.selected
                    .iter()
                    .all(|s| c.edges.iter().any(|[a, b]| (*a..=*b).contains(&s.index)))
            );
        }
        let _ = (
            &c.skipped_for,
            c.rebuild_uv,
            c.global_merge,
            c.chamfer,
            c.bevel_corners,
            c.expected_edges,
        );
    }
    for n in &d.not_lifted {
        assert!(n.line > 0 && !n.why.is_empty(), "{} {}", n.suite, n.case);
    }
}

/// G262's done-when clause: on `SMLib`'s fixtures, no case judged feasible fails.
#[test]
#[ignore = "fetches SMLib's fixtures from usd-brep's pinned commit; kernel-nightly.yml's corpora job runs it"]
fn no_case_judged_feasible_fails_on_smlib_fixtures() {
    let d = data();
    let manifest = Manifest::embedded().unwrap();
    let corpus = manifest.get(&d.corpus).expect("the corpus is registered");
    let cache = std::env::var_os("SPAREKIT_CORPORA").map_or_else(
        || PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("corpora"),
        PathBuf::from,
    );
    fetch(corpus, &cache).unwrap();
    let root = cache.join(&d.corpus).join("TestFiles").join("pt_TestFiles");
    let mut files: BTreeMap<String, Result<Smb, smb::SmbError>> = BTreeMap::new();
    let (mut feasible, mut refused, mut unread, mut unreadable) = (0, 0, 0, 0);
    let mut failures = Vec::new();
    for case in &d.cases {
        let smb = files
            .entry(case.file.clone())
            .or_insert_with(|| smb::read(&std::fs::read(root.join(&case.file)).unwrap()));
        let name = format!(
            "{}/{} {} (line {})",
            case.suite, case.case, case.file, case.line
        );
        let Ok(smb) = smb else {
            unreadable += 1;
            println!(
                "{name}: the reader refuses it: {}",
                smb.as_ref().unwrap_err()
            );
            continue;
        };
        // The reader sees the edges SMLib sees, in its order.
        assert_eq!(smb.edges.len(), case.fixture_edges, "{name}");
        for s in &case.selected {
            let edge = &smb.edges[s.index];
            assert_eq!(
                smb.vertices[edge.start as usize].point, s.start,
                "{name}: edge {}",
                s.index
            );
            assert_eq!(
                smb.vertices[edge.end as usize].point, s.end,
                "{name}: edge {}",
                s.index
            );
        }
        let filleted = case.smlib.status == SUCCESS && case.smlib.body;
        match judge(smb, case) {
            Judged::Unread(why) => {
                unread += 1;
                println!("{name}: not judged: {why}");
            }
            Judged::Plan { feasible: true, .. } => {
                feasible += 1;
                println!(
                    "{name}: feasible; SMLib {}",
                    if filleted { "filleted it" } else { "failed" }
                );
                if !filleted {
                    failures.push(format!(
                        "{name}: judged feasible, and SMLib failed (status {}, edges {})",
                        case.smlib.status, case.smlib.edges
                    ));
                }
            }
            Judged::Plan {
                feasible: false,
                why,
            } => {
                refused += 1;
                println!(
                    "{name}: refused ({why}); SMLib {}",
                    if filleted { "filleted it" } else { "failed" }
                );
            }
        }
    }
    println!(
        "{} cases: {feasible} judged feasible, {refused} refused, {unread} not judged, {unreadable} fixtures the reader refuses",
        d.cases.len()
    );
    assert!(
        feasible > 0,
        "no case was judged feasible, so the clause was not tested"
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
