//! The replay hash of placed instances (piece G145): `geom_topo::placed`'s queries on exact
//! placements, `geom_make::placed`'s materialisation and its library's answers on general ones, and
//! every refusal, written out and hashed with SHA-256. The digest must be the same on every target
//! (chapters 19.12, 21, 23.9 and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed `SplitMix64` stream, integers only, so the inputs are the same on every
//! target by construction: three seeded boxes, three seeded skewed prisms and a seeded cylinder as
//! definitions, each in its own frame.
//!
//! - **Exact placements:** four for each definition, each one of the 48 signed permutations by its
//!   packed number with a seeded shift. Written: the packed placement and its box, every face row
//!   and twice its area vector, six times the volume, and for seeded points round the box, half of
//!   them on the plane of a face, and the image of a corner of the shared box, the point in the
//!   shared frame and its side of the shell; then the materialised body's canonical output.
//! - **General placements:** for each flat definition, two rotations drawn from a side table of
//!   six (about each axis, by Pythagorean directions), answered through a `Library` that accepts a
//!   certified body within 64 steps: the box, the materialised body's canonical output, the library's
//!   answers to the same queries and how many bodies it holds; one more placement that the library
//!   may not round (refused, nothing kept) and one past the table (refused).
//! - **Refused:** packed permutations past 47, a shift that sends a box out of the grid's range,
//!   and a scale as a general placement.
//!
//! Each answer of `geom_topo::placed` on an exact placement is held to the same query on its
//! materialisation and to the library's answer, and a general placement's own queries all answer
//! that it is general. A body is written by the SHA-256 of its native format (every array, its
//! geometry and its names), with the names the operation made, its history, its exactness and its
//! diagnostics (`geom_test::canonical`); a refusal by its kind, entities and evidence and its debug
//! text. A target that permutes, rounds, orders, names or refuses differently changes the digest.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::fmt::Debug;
use std::num::NonZeroU64;

use geom_make::placed::{Library, Shared, materialise};
use geom_make::{BoxInput, Made, Moved, PrismInput, RoundInput, cylinder, make_box, prism};
use geom_math::{Axis, Box3, RationalTransform, Rotation, Scale, TypedTransform};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{NameSet, OpContext, OpError, OpId, OpOutput, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_test::canonical::{Outcome, canonical_bytes};
use geom_test::fixture::kernel::id_of;
use geom_test::golden::{self, hex, sha256};
use geom_test::{Seed, SplitMix64};
use geom_topo::Ix;
use geom_topo::placed::{self as topo, Definition, Placed, PlacedError, Placement};
use geom_topo::query::{self, QueryError};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-make/placed";

/// The bodies made, the refusals met, and the answers held to their materialisation.
const MADE: u32 = 47;
const REFUSED: u32 = 12;
const HELD: u32 = 28;

/// The exact placements of each definition, and the general ones of each flat definition.
const EXACT: usize = 4;
const GENERAL: usize = 2;

/// The seeded points asked of each placement.
const POINTS: usize = 6;

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

/// A whole number in `low..=high`.
fn between(rng: &mut SplitMix64, low: i64, high: i64) -> i64 {
    low + below(rng, (high - low + 1) as u64) as i64
}

/// The path of operation `n`.
fn op(n: u32) -> OpPath {
    let mut bytes = [0x45; 16];
    bytes[..4].copy_from_slice(&n.to_le_bytes());
    OpPath::new(OpId::from_bytes(bytes))
}

/// Operation `n`'s context, its budget far above what the corpus needs: a step that stops
/// advancing fails, not hangs.
fn ctx(n: u32) -> OpContext<'static> {
    OpContext::new(op(n), Budget::of_work(1 << 31))
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

/// The certified class and bound a library accepts for a rounded body.
const ACCEPT: Exactness = Exactness::Certified {
    bound: Steps(64),
    step: GridStep::Q0,
};

/// The document's side table of general transforms: rotations about each axis.
fn table() -> Vec<TypedTransform> {
    [
        (Axis::Z, 3, 4),
        (Axis::X, 5, 12),
        (Axis::Y, 8, 15),
        (Axis::Z, -4, 3),
        (Axis::Y, 12, -5),
        (Axis::X, 7, 24),
    ]
    .into_iter()
    .map(|(axis, c, s)| TypedTransform::rotation(axis, Rotation::direction(c, s).unwrap()).unwrap())
    .collect()
}

/// A definition: its body and its box.
struct Def {
    made: OpOutput<Made>,
    bound: Box3,
    /// Whether every face is flat, so it can be placed by a general rotation.
    flat: bool,
    /// The planes of its flat faces, each as `n` and `c` of n·p = c: half the points asked lie on
    /// one, where a side is decided by the query's tie, which must fall as on the materialised body.
    planes: Vec<([i64; 3], i64)>,
}

impl Def {
    /// Whether `q`, in the definition's frame, lies on the plane of a flat face.
    fn on_a_face(&self, q: P3) -> bool {
        let q = [q.x.get(), q.y.get(), q.z.get()];
        self.planes
            .iter()
            .any(|(n, c)| n[0] * q[0] + n[1] * q[1] + n[2] * q[2] == *c)
    }
}

/// The planes of the faces of the box from `lo` to `hi`.
fn box_planes(lo: [i64; 3], hi: [i64; 3]) -> Vec<([i64; 3], i64)> {
    let mut planes = Vec::new();
    for (i, n) in [[1, 0, 0], [0, 1, 0], [0, 0, 1]].into_iter().enumerate() {
        planes.push((n, lo[i]));
        planes.push((n, hi[i]));
    }
    planes
}

/// The definitions (the module documentation's list), each made by its own operation.
fn definitions(rng: &mut SplitMix64, next: &mut u32) -> Vec<Def> {
    let mut made = Vec::new();
    let mut fresh = || {
        *next += 1;
        ctx(*next)
    };
    for _ in 0..3 {
        let min = p(
            between(rng, -200, 200),
            between(rng, -200, 200),
            between(rng, -200, 200),
        );
        let size = [0, 1, 2].map(|_| between(rng, 1, 60));
        let lo = [min.x.get(), min.y.get(), min.z.get()];
        let planes = box_planes(lo, [0, 1, 2].map(|i| lo[i] + size[i]));
        let size = size.map(|s| Steps(s as u64));
        let made_box = make_box(&fresh(), &BoxInput { min, size }).unwrap();
        made.push((made_box, true, planes));
    }
    for _ in 0..3 {
        let (x, y, z) = (
            between(rng, -200, 200),
            between(rng, -200, 200),
            between(rng, -200, 200),
        );
        let (w, d, skew) = (
            between(rng, 2, 60),
            between(rng, 2, 60),
            between(rng, 1, 40),
        );
        let base = vec![
            p(x, y, z),
            p(x + w, y, z),
            p(x + w + skew, y + d, z),
            p(x + skew, y + d, z),
        ];
        let h = between(rng, 1, 60);
        let mut planes = box_planes([0, y, z], [0, y + d, z + h]);
        planes.drain(..2);
        planes.push(([d, -skew, 0], d * x - skew * y));
        planes.push(([d, -skew, 0], d * (x + w) - skew * y));
        let height = Steps(h as u64);
        made.push((
            prism(&fresh(), &PrismInput { base, height }).unwrap(),
            true,
            planes,
        ));
    }
    let base = p(
        between(rng, -100, 100),
        between(rng, -100, 100),
        between(rng, -100, 100),
    );
    let h = between(rng, 10, 400);
    let round = RoundInput {
        base,
        radius: Steps(between(rng, 100, 400) as u64),
        height: Steps(h as u64),
        step: GridStep::Q0,
    };
    let z = base.z.get();
    let planes = vec![([0, 0, 1], z), ([0, 0, 1], z + h)];
    made.push((cylinder(&fresh(), &round).unwrap(), false, planes));
    made.into_iter()
        .map(|(made, flat, planes)| {
            let bound = query::bounding_box(&made.result.body, &mut meter())
                .unwrap()
                .unwrap()
                .0;
            Def {
                made,
                bound,
                flat,
                planes,
            }
        })
        .collect()
}

/// A query's answer on an exact placement, as the shared query's error if it failed there.
fn shared<T>(r: Result<T, PlacedError>) -> Result<T, QueryError> {
    r.map_err(|e| match e {
        PlacedError::Query(q) => q,
        other => panic!("an exact placement answered {other:?}"),
    })
}

fn corners(b: &Box3) -> [P3; 2] {
    [b.min(), b.max()]
}

/// What the corpus gave.
struct Record {
    bytes: Vec<u8>,
    made: u32,
    refused: u32,
    held: u32,
}

impl Record {
    fn line(&mut self, text: &str) {
        self.bytes.extend((text.len() as u64).to_le_bytes());
        self.bytes.extend(text.as_bytes());
    }

    fn answer<T: Debug>(&mut self, what: &str, r: &T) {
        self.line(&format!("{what} {r:?}"));
    }

    fn placed(&mut self, what: &str, placed: &Placed) {
        let [lo, hi] = corners(placed.bound());
        self.line(&format!(
            "{what} {:?} {:?} box {:?} {:?}",
            placed.definition(),
            placed.placement(),
            [lo.x.get(), lo.y.get(), lo.z.get()],
            [hi.x.get(), hi.y.get(), hi.z.get()],
        ));
    }

    /// A materialisation's canonical output, its history under `path`.
    fn output(&mut self, path: &OpPath, made: &OpOutput<Made>) {
        self.made += 1;
        let body = &made.result.body;
        let digest = match id_of(body) {
            Ok(id) => <[u8; 32]>::try_from(id.digest()).unwrap(),
            Err(why) => panic!("a body the native format refuses: {why}"),
        };
        let names = NameSet::new(made.result.names.iter().map(|name| name.id().unwrap())).unwrap();
        let output = Outcome::Output {
            bodies: &[digest],
            names: &names,
            history: Some((path, &made.result.history)),
            exactness: &made.exactness,
            diagnostics: &made.diagnostics,
        };
        let bytes = canonical_bytes(&output).unwrap();
        self.line(&hex(sha256(&bytes)));
    }

    /// A refusal: its canonical encoding and its debug text.
    fn error(&mut self, e: &OpError) {
        self.refused += 1;
        let bytes = canonical_bytes(&Outcome::Error(e)).unwrap();
        self.line(&hex(sha256(&bytes)));
        self.line(&format!("{e:?}"));
    }

    fn outcome(&mut self, path: &OpPath, made: &Result<OpOutput<Made>, OpError>) {
        match made {
            Ok(out) => self.output(path, out),
            Err(e) => self.error(e),
        }
    }
}

/// A seeded point in `b` or up to two steps round it.
fn point(rng: &mut SplitMix64, b: &Box3) -> P3 {
    let [lo, hi] = corners(b);
    let mut axis = |l: i64, h: i64| between(rng, l - 2, h + 2);
    p(
        axis(lo.x.get(), hi.x.get()),
        axis(lo.y.get(), hi.y.get()),
        axis(lo.z.get(), hi.z.get()),
    )
}

/// One exact placement of definition `d`, packed as `axes` with `shift`.
fn exact(rec: &mut Record, rng: &mut SplitMix64, next: &mut u32, d: u32, def: &Def) {
    let shift = [0, 1, 2].map(|_| between(rng, -500, 500));
    let axes = below(rng, 48) as u8;
    let packed = Placement::Exact { shift, axes };
    let t = packed.grid().unwrap();
    let instance = Placed::exact(Definition(d), &t, &def.bound).unwrap();
    assert_eq!(instance.placement(), packed);
    rec.placed("exact", &instance);
    rec.line(&format!("mirror {}", t.reverses_orientation()));
    let body = &def.made.result.body;
    let s = Ix::new(0).unwrap();

    // The answers in the shared frame.
    let rows = shared(topo::faces(body, &instance, &mut meter()));
    rec.answer("faces", &rows);
    let areas: Vec<_> = rows
        .iter()
        .flatten()
        .map(|row| shared(topo::face_area(body, &instance, row.face, &mut meter())))
        .collect();
    rec.answer("areas", &areas);
    let six = shared(topo::six_volume(body, &instance, &mut meter()));
    rec.answer("volume", &six.as_ref().map(|v| v.to_le_limbs()));
    // Half on the plane of a face, half off every one.
    let mut asked = Vec::new();
    let mut on = 0;
    while asked.len() < POINTS {
        let q = point(rng, instance.bound());
        let face = def.on_a_face(topo::to_shared(&instance, q).unwrap());
        if face == (on < POINTS / 2) {
            on += usize::from(face);
            asked.push(q);
        }
    }
    // The image of the shared box's low corner, a vertex of each flat definition.
    asked.push(topo::from_shared(&instance, def.bound.min()).unwrap());
    let mut sides = Vec::new();
    for q in &asked {
        let back = topo::to_shared(&instance, *q).unwrap();
        assert_eq!(topo::from_shared(&instance, back).unwrap(), *q);
        let side = shared(topo::side_of_shell(body, &instance, s, *q, &mut meter()));
        rec.answer(&format!("point {q:?} shared {back:?} side"), &side);
        sides.push(side);
    }

    // The library answers the same and makes nothing.
    let key = Shared {
        key: d,
        body,
        exactness: def.made.exactness,
        accept: None,
    };
    let none: [TypedTransform; 0] = [];
    let mut library = Library::new(&none);
    let c = ctx(0);
    if let Ok(six) = &six {
        assert_eq!(library.six_volume(&c, &key, &instance).unwrap(), *six);
    }
    if let Ok(rows) = &rows {
        assert_eq!(library.faces(&c, &key, &instance).unwrap(), *rows);
    }
    assert!(library.is_empty());

    // The materialisation, and the same queries on it.
    *next += 1;
    let moved = Moved {
        body,
        exactness: def.made.exactness,
        accept: None,
    };
    let made = materialise(&ctx(*next), moved, &TypedTransform::Exact(t));
    rec.outcome(&op(*next), &made);
    if let Ok(made) = &made {
        let b = &made.result.body;
        assert_eq!(rows, query::faces(b, &mut meter()), "{d} {axes}");
        for (row, area) in rows.iter().flatten().zip(&areas) {
            assert_eq!(
                *area,
                query::face_area(b, row.face, &mut meter()),
                "{d} {axes}"
            );
        }
        assert_eq!(six, query::volume(b, &mut meter()), "{d} {axes}");
        for (q, side) in asked.iter().zip(&sides) {
            assert_eq!(
                *side,
                query::side_of_shell(b, s, *q, &mut meter()),
                "{d} {axes} {shift:?} {q:?} shared {:?} def {:?} placed {:?} body {:?}",
                topo::to_shared(&instance, *q),
                def.bound,
                instance.bound(),
                query::bounding_box(b, &mut meter())
            );
        }
        assert_eq!(
            query::bounding_box(b, &mut meter()).unwrap().unwrap().0,
            *instance.bound()
        );
        rec.held += 1;
    }
}

/// A general placement of definition `d` by entry `index` of `table`, through `library`: whether
/// its body was made.
fn general(
    rec: &mut Record,
    rng: &mut SplitMix64,
    next: &mut u32,
    (d, def): (u32, &Def),
    library: &mut Library<'_, u32>,
    (index, t): (u32, &TypedTransform),
    accept: Option<Exactness>,
) -> bool {
    let body = &def.made.result.body;
    let instance = Placed::general(Definition(d), index, t, &def.bound).unwrap();
    rec.placed("general", &instance);
    assert_eq!(
        topo::six_volume(body, &instance, &mut meter()),
        Err(PlacedError::General)
    );
    assert_eq!(
        topo::to_shared(&instance, def.bound.min()),
        Err(PlacedError::General)
    );
    let key = Shared {
        key: d,
        body,
        exactness: def.made.exactness,
        accept,
    };
    let before = library.len();
    *next += 1;
    let n = *next;
    let made = library.body(&ctx(n), &key, instance.placement()).cloned();
    rec.outcome(&op(n), &made);
    let Ok(made) = made else {
        assert_eq!(library.len(), before, "a refused body is not kept");
        return false;
    };
    let c = ctx(n);
    let rows = library.faces(&c, &key, &instance);
    rec.answer("faces", &rows);
    let areas: Vec<_> = rows
        .iter()
        .flatten()
        .map(|row| library.face_area(&c, &key, &instance, row.face))
        .collect();
    rec.answer("areas", &areas);
    let six = library.six_volume(&c, &key, &instance);
    rec.answer("volume", &six.as_ref().map(|v| v.to_le_limbs()));
    let s = Ix::new(0).unwrap();
    for q in (0..POINTS)
        .map(|_| point(rng, instance.bound()))
        .collect::<Vec<_>>()
    {
        let side = library.side_of_shell(&c, &key, &instance, s, q);
        rec.answer(&format!("point {q:?} side"), &side);
    }
    // The body's box lies inside the instance's, and the questions made nothing more.
    let [lo, hi] = corners(
        &query::bounding_box(&made.result.body, &mut meter())
            .unwrap()
            .unwrap()
            .0,
    );
    assert!(instance.bound().contains(lo) && instance.bound().contains(hi));
    rec.line(&format!("bodies {}", library.len()));
    true
}

/// The corpus's record.
fn corpus() -> Record {
    let mut rng = SplitMix64::new(Seed(0x0145));
    let mut rec = Record {
        bytes: Vec::new(),
        made: 0,
        refused: 0,
        held: 0,
    };
    let mut next = 1000;
    let defs = definitions(&mut rng, &mut next);
    let table = table();
    for (d, def) in (0_u32..).zip(&defs) {
        rec.line(&format!("definition {d}"));
        rec.output(&op(1001 + d), &def.made);
        for _ in 0..EXACT {
            exact(&mut rec, &mut rng, &mut next, d, def);
        }
        if !def.flat {
            continue;
        }
        let mut library = Library::new(&table);
        // One the library may not round, first, so no body of that placement is kept yet; then
        // the rounded ones, and one past the table.
        let mut accepts = vec![None];
        accepts.extend([Some(ACCEPT); GENERAL]);
        let mut t = &table[0];
        for accept in accepts {
            let i = below(&mut rng, table.len() as u64) as u32;
            t = &table[i as usize];
            let made = general(
                &mut rec,
                &mut rng,
                &mut next,
                (d, def),
                &mut library,
                (i, t),
                accept,
            );
            assert_eq!(made, accept.is_some(), "definition {d}, entry {i}");
        }
        let past = table.len() as u32 + below(&mut rng, 5) as u32;
        let made = general(
            &mut rec,
            &mut rng,
            &mut next,
            (d, def),
            &mut library,
            (past, t),
            Some(ACCEPT),
        );
        assert!(!made, "past the table");
    }

    // Refused where the instance is made.
    for _ in 0..3 {
        let axes = 48 + below(&mut rng, 208) as u8;
        let packed = Placement::Exact {
            shift: [0; 3],
            axes,
        }
        .grid();
        assert_eq!(packed, Err(PlacedError::Permutation(axes)));
        rec.answer("packed", &packed);
    }
    // A box of positive corners, its axes permuted with no sign turned, shifted to the grid's
    // last step: its image is past the range.
    let side = between(&mut rng, 1, 9);
    let small = Box3::new(p(1, 1, 1), p(side, side, side)).unwrap();
    let far = Placement::Exact {
        shift: [(1 << 40) - 1, 0, 0],
        axes: 8 * below(&mut rng, 6) as u8,
    }
    .grid()
    .unwrap();
    let far = Placed::exact(Definition(0), &far, &small);
    assert!(matches!(far, Err(PlacedError::Num(_))), "{far:?}");
    rec.answer("far", &far);
    let factor = [2, 3, 5][below(&mut rng, 3) as usize];
    let scale = TypedTransform::from(
        RationalTransform::uniform_scale(Scale::new(factor, 1).unwrap()).unwrap(),
    );
    let scaled = Placed::general(Definition(0), 0, &scale, &defs[0].bound);
    assert_eq!(scaled, Err(PlacedError::NotRigid));
    rec.answer("scale", &scaled);
    rec
}

#[test]
fn placed_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-make placed corpus: {} bodies made, {} refused, {} held to their materialisation, \
         {} bytes, golden hash {}",
        rec.made,
        rec.refused,
        rec.held,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert_eq!(
        (rec.made, rec.refused, rec.held),
        (MADE, REFUSED, HELD),
        "every placement ran"
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target permutes, rounds, orders, names or \
             refuses differently (compare this log's digest with another target's), or a \
             definition changed in place, which chapter 26.2 forbids"
        )
    });
}
