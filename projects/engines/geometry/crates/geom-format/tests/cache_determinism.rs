//! Cache keys, content IDs and restamped names are the same on every target and at any number of
//! threads (principle 5, chapter 21; G105): keys of every kind, the IDs of bodies and of a chain of
//! versions, and a restamped entry's names and records, hashed into one golden digest.

#![allow(
    dead_code,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test corpus, and chapter 21's thread-count test spawns threads"
)]

#[path = "../../geom-topo/tests/common/mod.rs"]
mod topo;

use geom_format::body::{BodyVersion, write_body};
use geom_format::cache::{CacheEntry, CacheKey, NamedBody, restamp};
use geom_format::content::ContentId;
use geom_num::{Exactness, GridStep, Steps};
use geom_op::{
    BLANK_OP, Class, EdgeOrVertex, Family, NameId, NameList, NameSet, OpId, OpOutput, OpPath,
    OpSchema, Precision, Role, RoleArg, Stamp, TopoName,
};
use geom_run::{Budget, Usage};
use geom_test::golden::{self, Sha256, hex};
use geom_topo::Builder;
use geom_topo::order::{NameKey, Names};

use topo::{ix, many_faces, meter, wall, wall_builder, wall_outside, wall_rows};

const GOLDEN: &str = "geom-format/cache-keys";

const SCHEMA: OpSchema = OpSchema {
    name: "make.box",
    version: 3,
    family: Family::Make,
    doc: "A box.",
    inputs: &[],
    outputs: &[],
    roles: &[],
    errors: &[],
    diagnostics: &[],
    classes: &[Class::Exact],
    charges: &[],
    batch: false,
    name_free: true,
    example: "a box of 1 m",
};

const BUDGET: Budget = Budget {
    work: 1_000,
    logical_bytes: 1_000,
};

fn put(total: &mut Sha256, section: &mut Sha256, bytes: &[u8]) {
    for h in [total, section] {
        h.write(&(bytes.len() as u64).to_le_bytes());
        h.write(bytes);
    }
}

fn path(byte: u8) -> OpPath {
    OpPath::new(OpId::from_bytes([byte; 16]))
}

fn id(c: &ContentId) -> Vec<u8> {
    c.item().unwrap().into_bytes()
}

fn key(name: &TopoName) -> NameKey {
    NameKey(name.id().unwrap().to_bytes())
}

/// A wall named by a run at `at` on `input`: its faces in roles, the rest bounded by them.
fn run(at: &OpPath, input: NameId) -> NamedBody {
    let r = wall_rows();
    let created = |step: &str, count: usize| TopoName::Created {
        op: at.step(step).unwrap(),
        role: Role::new("side", [RoleArg::Name(input), RoleArg::Count(count as u64)]).unwrap(),
        from: NameList::new([input]),
    };
    let regions: Vec<TopoName> = (0..r.regions.len()).map(|i| created("region", i)).collect();
    let faces: Vec<TopoName> = (0..r.faces.len()).map(|i| created("face", i)).collect();
    let bounded = |kind, a: usize, b: usize| TopoName::Bounded {
        kind,
        by: NameSet::new([faces[a].id().unwrap(), faces[b].id().unwrap()]).unwrap(),
        ends: NameSet::default(),
    };
    let pairs: Vec<(usize, usize)> = (0..6)
        .flat_map(|a| ((a + 1)..6).map(move |b| (a, b)))
        .collect();
    let edges: Vec<TopoName> = pairs[..r.edges.len()]
        .iter()
        .map(|&(a, b)| bounded(EdgeOrVertex::Edge, a, b))
        .collect();
    let vertices: Vec<TopoName> = pairs[..r.vertices.len()]
        .iter()
        .map(|&(a, b)| bounded(EdgeOrVertex::Vertex, a, b))
        .collect();
    let keys = Names {
        regions: regions.iter().map(key).collect(),
        faces: faces.iter().map(key).collect(),
        edges: edges.iter().map(key).collect(),
        vertices: vertices.iter().map(key).collect(),
    };
    let body = wall_builder(true)
        .publish_named(
            &keys,
            geom_topo::BodyKind::Solid,
            wall_outside(),
            &mut meter(),
        )
        .unwrap();
    let names = regions
        .into_iter()
        .chain(faces)
        .chain(edges)
        .chain(vertices)
        .collect();
    NamedBody { names, body }
}

struct Corpus {
    total: Sha256,
    report: Vec<(&'static str, [u8; 32])>,
}

fn corpus() -> Corpus {
    let mut total = Sha256::new();
    let mut report = Vec::new();

    let mut s = Sha256::new();
    let inputs = [ContentId::of_payload(b"a"), ContentId::of_payload(b"b")];
    let preview = Precision::Preview {
        bound: Steps(3),
        step: GridStep::Q0,
    };
    for precision in [Precision::Final, preview] {
        for edition in [0, 1] {
            let at = path(7).step("cut").unwrap();
            let k = CacheKey::new(edition, &SCHEMA, &at, precision, &inputs).unwrap();
            put(&mut total, &mut s, &k.to_bytes());
            let free = CacheKey::name_free(edition, &SCHEMA, precision, &inputs)
                .unwrap()
                .unwrap();
            put(&mut total, &mut s, &free.to_bytes());
        }
    }
    report.push(("keys", s.finish()));

    let mut s = Sha256::new();
    let named = run(&path(2), NameId::from_bytes([5; 16]));
    for body in [wall(), many_faces(9_000), named.body.clone()] {
        let version = BodyVersion::first(&body, &mut meter()).unwrap();
        put(&mut total, &mut s, &id(&version.content_id()));
        put(&mut total, &mut s, &id(version.geometry_id()));
    }
    report.push(("bodies", s.finish()));

    let mut s = Sha256::new();
    let mut body = many_faces(9_000);
    let mut version = BodyVersion::first(&body, &mut meter()).unwrap();
    for step in 0..8 {
        let mut arenas = body.arenas().clone();
        arenas.faces.surface.set(ix(step * 1_001), ix(1)).unwrap();
        body = Builder { arenas }
            .publish(body.kind(), body.outside(), &mut meter())
            .unwrap();
        version = version.next(&body, &mut meter()).unwrap();
        put(&mut total, &mut s, &id(&version.content_id()));
    }
    report.push(("versions", s.finish()));

    let mut s = Sha256::new();
    let blank = run(&OpPath::new(BLANK_OP), Stamp::blank_input(0).unwrap());
    let entry = CacheEntry::new(
        Ok(OpOutput::new(blank, Exactness::Exact)),
        BUDGET,
        Usage::new(7, 9),
    )
    .unwrap();
    let got = restamp(
        &entry,
        &path(4),
        &[NameId::from_bytes([6; 16])],
        &mut meter(),
    )
    .unwrap()
    .unwrap();
    let answer = got.answer(BUDGET).unwrap().as_ref().unwrap();
    for name in &answer.result.names {
        put(&mut total, &mut s, &name.id().unwrap().to_bytes());
    }
    let stored = write_body(&answer.result.body, &mut meter()).unwrap();
    for r in stored.chunks.iter().chain([&stored.index]) {
        put(&mut total, &mut s, &r.encode(&mut meter()).unwrap());
    }
    report.push(("restamp", s.finish()));

    Corpus { total, report }
}

#[test]
fn golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let c = corpus();
        (c.total.clone().finish(), c)
    });
    for (name, digest) in &checked.value.report {
        println!("  {name:<8} {}", hex(*digest));
    }
    println!(
        "geom-format cache-keys golden hash: {}",
        hex(checked.value.total.clone().finish())
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!("{e}. Compare the section digests above with another target's log to find the part that differs")
    });
}

/// Chapter 21's thread-count test: the same digest on 1, 2, 3 and 8 threads at once.
#[test]
#[cfg_attr(
    all(target_family = "wasm", not(target_feature = "atomics")),
    ignore = "WebAssembly without the atomics feature (wasm32-wasip1) has no threads: \
              std::thread::scope cannot spawn"
)]
fn golden_hash_is_the_same_on_every_thread_count() {
    let one = hex(corpus().total.finish());
    for threads in golden::THREAD_COUNTS {
        let digests: Vec<String> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|_| s.spawn(|| hex(corpus().total.finish())))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(
            digests.iter().all(|d| *d == one),
            "{threads} threads: {digests:?}"
        );
    }
}
