//! A name-free entry restamped is the entry a fresh run at that path on those names gives: names,
//! arena order, warnings and bits (G105, 23.8).

#![allow(
    dead_code,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests: native arithmetic and indexing, and a failure stops the test"
)]

#[path = "../../geom-topo/tests/common/mod.rs"]
mod topo;

use geom_format::body::write_body;
use geom_format::cache::{CacheEntry, NamedBody, StampError, restamp};
use geom_num::{Exactness, GridStep, Steps};
use geom_op::{
    BLANK_OP, Diagnostic, EdgeOrVertex, NameError, NameId, NameList, NameSet, OpError,
    OpErrorDetail, OpId, OpOutput, OpPath, Ordinal, Role, RoleArg, RunDetail, Stamp, TopoName,
};
use geom_run::{Budget, BudgetKind, Exceeded, Phase, Usage};
use geom_topo::order::{NameKey, Names};
use geom_topo::{Body, BodyKind};
use proptest::prelude::*;

use topo::{meter, wall, wall_builder, wall_outside, wall_rows};

const BUDGET: Budget = Budget {
    work: 1_000,
    logical_bytes: 1_000,
};

fn key(name: &TopoName) -> NameKey {
    NameKey(name.id().unwrap().to_bytes())
}

/// What a run at `path` on input names `inputs` names a wall: its two regions and six faces in
/// roles, twelve edges between pairs of faces, and eight vertices that collide but for an ordinal.
fn run(path: &OpPath, inputs: &[NameId; 2]) -> NamedBody {
    let r = wall_rows();
    let created = |step: &str, count: usize| TopoName::Created {
        op: path.step(step).unwrap(),
        role: Role::new(
            "side",
            [RoleArg::Name(inputs[0]), RoleArg::Count(count as u64)],
        )
        .unwrap(),
        from: NameList::new(inputs.iter().copied()),
    };
    let regions: Vec<TopoName> = (0..r.regions.len()).map(|i| created("region", i)).collect();
    let faces: Vec<TopoName> = (0..r.faces.len()).map(|i| created("face", i)).collect();
    let pairs = (0..6).flat_map(|a| ((a + 1)..6).map(move |b| (a, b)));
    let edges: Vec<TopoName> = pairs
        .take(r.edges.len())
        .map(|(a, b): (usize, usize)| TopoName::Bounded {
            kind: EdgeOrVertex::Edge,
            by: NameSet::new([faces[a].id().unwrap(), faces[b].id().unwrap()]).unwrap(),
            ends: NameSet::default(),
        })
        .collect();
    let base = TopoName::Bounded {
        kind: EdgeOrVertex::Vertex,
        by: NameSet::new(faces.iter().map(|f| f.id().unwrap())).unwrap(),
        ends: NameSet::default(),
    };
    let vertices: Vec<TopoName> = Ordinal::all(r.vertices.len() as u32)
        .map(|ord| TopoName::Weak {
            base: base.id().unwrap(),
            ord,
        })
        .collect();
    let keys = Names {
        regions: regions.iter().map(key).collect(),
        faces: faces.iter().map(key).collect(),
        edges: edges.iter().map(key).collect(),
        vertices: vertices.iter().map(key).collect(),
    };
    let body = wall_builder(true)
        .publish_named(&keys, BodyKind::Solid, wall_outside(), &mut meter())
        .unwrap();
    let names = regions
        .into_iter()
        .chain(faces)
        .chain(edges)
        .chain([base])
        .chain(vertices)
        .collect();
    NamedBody { names, body }
}

fn blank_inputs() -> [NameId; 2] {
    [
        Stamp::blank_input(0).unwrap(),
        Stamp::blank_input(1).unwrap(),
    ]
}

fn blank_run() -> NamedBody {
    run(&OpPath::new(BLANK_OP), &blank_inputs())
}

fn entry(result: NamedBody) -> CacheEntry<NamedBody> {
    let output = OpOutput::new(result, Exactness::Exact);
    CacheEntry::new(Ok(output), BUDGET, Usage::new(7, 9)).unwrap()
}

fn path(byte: u8) -> OpPath {
    OpPath::new(OpId::from_bytes([byte; 16]))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// The law of 23.8: whatever the path and the input names, and so whatever order the arenas
    /// come out in, a restamped entry is a fresh run's, down to the records' bytes.
    #[test]
    fn a_restamped_entry_is_a_fresh_runs(
        op in any::<[u8; 16]>(),
        step in prop::option::of("[a-z]{1,8}"),
        a in any::<[u8; 16]>(),
        b in any::<[u8; 16]>(),
    ) {
        let mut at = OpPath::new(OpId::from_bytes(op));
        if let Some(step) = step {
            at = at.step(&step).unwrap();
        }
        let inputs = [NameId::from_bytes(a), NameId::from_bytes(b)];
        let got = restamp(&entry(blank_run()), &at, &inputs, &mut meter()).unwrap().unwrap();
        let fresh = entry(run(&at, &inputs));
        let body = |e: &CacheEntry<NamedBody>| {
            let answer = e.answer(BUDGET).unwrap().as_ref().unwrap();
            write_body(&answer.result.body, &mut meter()).unwrap()
        };
        prop_assert_eq!(body(&got), body(&fresh));
        prop_assert_eq!(got, fresh);
    }
}

/// Without the new sort a restamped body would keep the blank run's order, so this is the test
/// that shows a restamped body takes the order its new names give.
#[test]
fn a_restamped_body_takes_its_new_names_order() {
    let blank = blank_run();
    let inputs = [NameId::from_bytes([3; 16]), NameId::from_bytes([4; 16])];
    let got = restamp(&entry(blank.clone()), &path(9), &inputs, &mut meter())
        .unwrap()
        .unwrap();
    let answer = got.answer(BUDGET).unwrap().as_ref().unwrap();
    assert_ne!(
        answer.result.body.arenas().faces.bound,
        blank.body.arenas().faces.bound
    );
}

#[test]
fn a_restamped_entry_keeps_its_class_budget_and_use() {
    let class = Exactness::Certified {
        bound: Steps(3),
        step: GridStep::Q0,
    };
    let output = OpOutput::new(blank_run(), class);
    let e = CacheEntry::new(Ok(output), BUDGET, Usage::new(7, 9)).unwrap();
    let got = restamp(
        &e,
        &path(9),
        &[NameId::from_bytes([3; 16]); 2],
        &mut meter(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(got.used(), Usage::new(7, 9));
    let answer = got.answer(BUDGET).unwrap().as_ref().unwrap();
    assert_eq!(answer.exactness, class);
    assert!(got.answer(Budget { work: 6, ..BUDGET }).is_none());
    // The stored budget matters for a stored budget failure, and restamping keeps it.
    assert!(got.answer(Budget { work: 7, ..BUDGET }).is_some());
}

#[test]
fn a_warning_names_what_the_run_with_those_names_would_name() {
    let blank = blank_run();
    let inputs = [NameId::from_bytes([3; 16]), NameId::from_bytes([4; 16])];
    let warned = |result: NamedBody, entities: [NameId; 2]| {
        let warning = Diagnostic::new("make.small", "a small face")
            .unwrap()
            .with_entities(entities);
        let output = OpOutput::new(result, Exactness::Exact).with_diagnostic(warning);
        CacheEntry::new(Ok(output), BUDGET, Usage::new(7, 9)).unwrap()
    };
    let face = |r: &NamedBody| r.names[3].id().unwrap();
    let fresh_run = run(&path(9), &inputs);
    let e = warned(blank.clone(), [face(&blank), blank_inputs()[1]]);
    let got = restamp(&e, &path(9), &inputs, &mut meter())
        .unwrap()
        .unwrap();
    assert_eq!(
        got,
        warned(fresh_run.clone(), [face(&fresh_run), inputs[1]])
    );
}

#[test]
fn a_warning_about_a_name_nobody_stamped_is_refused() {
    let warning = Diagnostic::new("make.small", "a small face")
        .unwrap()
        .with_entities([NameId::from_bytes([1; 16])]);
    let output = OpOutput::new(blank_run(), Exactness::Exact).with_diagnostic(warning);
    let e = CacheEntry::new(Ok(output), BUDGET, Usage::new(7, 9)).unwrap();
    assert_eq!(
        restamp(
            &e,
            &path(9),
            &[NameId::from_bytes([3; 16]); 2],
            &mut meter()
        ),
        Err(StampError::Name(NameError::NotBlank))
    );
}

#[test]
fn a_stored_error_is_not_restamped() {
    let error = OpError::new(
        path(1),
        OpErrorDetail::Run(RunDetail::BudgetExceeded(Exceeded {
            budget: BUDGET,
            kind: BudgetKind::Work,
            phase: Phase::new("faces"),
            used: Usage::new(1_001, 0),
        })),
    );
    let e: CacheEntry<NamedBody> =
        CacheEntry::new(Err(error), BUDGET, Usage::new(1_001, 0)).unwrap();
    assert_eq!(restamp(&e, &path(9), &[], &mut meter()), Ok(None));
}

#[test]
fn names_that_are_not_blank_are_refused() {
    let inputs = [NameId::from_bytes([3; 16]), NameId::from_bytes([4; 16])];
    let already = entry(run(&path(5), &inputs));
    assert_eq!(
        restamp(&already, &path(9), &inputs, &mut meter()),
        Err(StampError::Name(NameError::NotBlank))
    );
}

#[test]
fn a_body_without_its_names_is_refused_by_the_builder() {
    let result = NamedBody {
        names: Vec::new(),
        body: wall(),
    };
    let got = restamp(&entry(result), &path(9), &[], &mut meter());
    assert!(matches!(got, Err(StampError::Topo(_))), "{got:?}");
}

#[test]
fn a_stamp_error_says_what_was_wrong() {
    let topo: Body = wall();
    let unnamed = NamedBody {
        names: Vec::new(),
        body: topo,
    };
    let got = restamp(&entry(unnamed), &path(9), &[], &mut meter()).unwrap_err();
    assert!(
        got.to_string()
            .starts_with("a restamped body could not be published")
    );
    let name = StampError::from(NameError::NotBlank);
    assert_eq!(
        name.to_string(),
        "a name could not be stamped: a name to stamp holds something not blank"
    );
}
