//! Mesh booleans against manifold-rust's rational engine, in process (chapter 28.10; piece G154). A
//! seed's jobs run through both, and every difference is either triaged or fails the test: the same
//! rule the nightly run holds an out-of-process oracle to.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests: small coordinates and vertex numbers, and a failure stops the test"
)]

use geom_test::oracle::exact::float;
use geom_test::oracle::meshes::{Answer, Compared, Job, MANIFOLD_RUST, Op, Solid, jobs, judge};
use geom_test::oracle::triage::Register;
use geom_test::seed::Seed;
use manifold_rust::manifold::Manifold;
use manifold_rust::types::{BooleanEngine, MeshGL64, OpType};

fn solid(s: &Solid) -> Manifold {
    let mesh = MeshGL64 {
        num_prop: 3,
        vert_properties: s.vertices.iter().flatten().map(|v| *v as f64).collect(),
        tri_verts: s
            .triangles
            .iter()
            .flatten()
            .map(|v| u64::from(*v))
            .collect(),
        ..MeshGL64::default()
    };
    Manifold::from_mesh_gl64_robust(&mesh)
}

fn answer(job: &Job) -> Answer {
    let op = match job.op {
        Op::Union => OpType::Add,
        Op::Intersection => OpType::Intersect,
        Op::Difference => OpType::Subtract,
    };
    let made = solid(&job.a).boolean_with_engine(&solid(&job.b), op, BooleanEngine::Robust);
    let mesh = made.get_mesh_gl64(-1);
    let text = |x: f64| float(x).unwrap();
    let triangles = (0..mesh.num_tri())
        .map(|t| {
            let [a, b, c] = mesh.get_tri_verts(t);
            [a, b, c].map(|v| mesh.get_vert_pos(v as usize).map(text))
        })
        .collect();
    Answer {
        id: job.id,
        volume: Some(text(made.volume())),
        area: Some(text(made.surface_area())),
        tolerance: Some(text(made.get_tolerance())),
        triangles: Some(triangles),
    }
}

#[test]
fn a_nights_differences_are_all_triaged() {
    let mut differences = Vec::new();
    for job in jobs(Seed(0x6d61_6e69), 300) {
        if let Compared::Differ { signature, settled } =
            judge(MANIFOLD_RUST, &job, &answer(&job)).unwrap()
        {
            println!("job {}: {signature}: {settled}", job.id);
            assert!(!settled.starts_with("ours"), "{signature}: {settled}");
            differences.push(signature);
        }
    }
    let new = Register::embedded().unwrap().new_signatures(&differences);
    assert!(new.is_empty(), "new signatures: {new:?}");
}
