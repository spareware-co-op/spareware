// SPDX-License-Identifier: Apache-2.0
//! The law catalogue's row for names and history (chapter 28.3; piece G155), on the operations
//! that name what they make (`geom-make`'s primitives, transforms and patterns) and `geom-op`'s
//! histories and their composition.
//!
//! - **The rules of section 20.3**, written here from the chapter, not through `geom-op`'s own
//!   checker: every output entity has its history; a deleted input has no modified outputs; an
//!   input's generated and modified outputs do not overlap; every output traces to an input or is
//!   generated; nothing is named as an input that is not one. They hold on every step of seeded
//!   chains of operations, and the kit's chain check (`geom_test::history::check_chain`) passes on
//!   each chain.
//! - **Composition included.** A chain's composed history, by `geom-op`'s relations folded from
//!   the left, is the one its definition gives, computed here path by path: an output is modified
//!   from an input when a path of modifications joins them, generated when another path does, and
//!   made from nothing when none does; an input is deleted when the first step deleted it or its
//!   modified images all die. Checked on the operations' chains and on the kit's generated chains
//!   of every kind of record, splits, merges and deletions included.
//! - **No duplicate names**: in every body, and across a pattern's copies and their source.
//! - **Untouched entities keep their names**: a transform keeps every name; a primitive's names
//!   are its operation's and its roles', so the same primitive drawn elsewhere or another size has
//!   the same names; adding a copy to a pattern renames no other copy.
//! - **The same names on every target**: a fixed corpus of operations, every name and history in
//!   its canonical encoding, has one digest, kept in the golden store
//!   (`geom-test/names-on-every-target`) and checked on every target CI builds.
//!
//! Booleans name what they make with the operations to come (chapter 20.2); their histories join
//! the row then. Seeds: each law prints `seed: <hex>` and replays with `SPAREKIT_SEED`; pull
//! requests run the fixed default seed and the nightly workflow a fresh one. The golden's corpus
//! has a seed of its own, which never changes.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests: native arithmetic on small numbers, and a failure stops the test"
)]

#[path = "catalogue/mod.rs"]
mod catalogue;

use std::collections::{BTreeMap, BTreeSet};

use catalogue::flat::{draw_prism, p};
use catalogue::{Draw, Findings};
use geom_make::{
    BoxInput, Instance, Made, Moved, PrismInput, RoundInput, SphereInput, TorusInput, cone,
    cylinder, make_box, mirror, move_by, pattern, prism, rotate, sphere, torus,
};
use geom_math::{Axis, GridTransform, Rotation, TypedTransform};
use geom_name::first_duplicate;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::history::{History, Relations};
use geom_op::{NameId, NameSet, OpContext, OpError, OpId, OpOutput, OpPath};
use geom_run::Budget;
use geom_test::golden::{self, hex, sha256};
use geom_test::history::{chain, check_chain};
use geom_test::prop;
use geom_test::{Seed, SplitMix64};
use geom_topo::Body;

/// The names-on-every-target corpus's golden in the golden store.
const GOLDEN: &str = "geom-test/names-on-every-target";

/// The context of the `n`th operation of a chain: each operation has its own path, as in a model.
fn ctx_at(n: u8) -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([n; 16])), Budget::UNLIMITED)
}

/// Every name a body carries: its regions', faces', edges' and vertices'.
fn names(body: &Body) -> Vec<NameId> {
    body.names().map_or_else(Vec::new, |n| {
        n.regions
            .iter()
            .chain(&n.faces)
            .chain(&n.edges)
            .chain(&n.vertices)
            .map(|k| NameId::from_bytes(k.0))
            .collect()
    })
}

/// A primitive of a drawn kind and size, made by the operation at `c`.
fn draw_primitive(d: &mut Draw, c: &OpContext<'_>) -> (String, Result<OpOutput<Made>, OpError>) {
    let at = p(d.between(-20, 20), d.between(-20, 20), d.between(-20, 20));
    let size = |d: &mut Draw| Steps(d.between(2, 12) as u64);
    match d.below(6) {
        0 => {
            let input = BoxInput {
                min: at,
                size: [size(d), size(d), size(d)],
            };
            (format!("{input:?}"), make_box(c, &input))
        }
        1 => {
            let shape = draw_prism(d, None);
            let input = PrismInput {
                base: shape.base.iter().map(|b| p(b[0], b[1], shape.z)).collect(),
                height: Steps(shape.height),
            };
            (format!("{input:?}"), prism(c, &input))
        }
        2 | 3 => {
            let input = RoundInput {
                base: at,
                radius: size(d),
                height: size(d),
                step: GridStep::Q0,
            };
            let made = if d.one_in(2) {
                cylinder(c, &input)
            } else {
                cone(c, &input)
            };
            (format!("{input:?}"), made)
        }
        4 => {
            let input = SphereInput {
                center: at,
                radius: size(d),
                step: GridStep::Q0,
            };
            (format!("{input:?}"), sphere(c, &input))
        }
        _ => {
            let minor = d.between(1, 4) as u64;
            let input = TorusInput {
                center: at,
                major: Steps(minor + d.between(2, 8) as u64),
                minor: Steps(minor),
                step: GridStep::Q0,
            };
            (format!("{input:?}"), torus(c, &input))
        }
    }
}

/// One step after a primitive: a grid move, or one copy of a pattern.
#[derive(Clone, Copy, Debug)]
enum Step {
    Move([i64; 3]),
    Turn(Axis, i32, P3),
    Mirror(Axis, P3),
    Copy([u8; 16], [i64; 3]),
}

impl Step {
    fn draw(d: &mut Draw) -> Step {
        let axis = [Axis::X, Axis::Y, Axis::Z][d.pick(3)];
        let centre = p(d.between(-5, 5), d.between(-5, 5), d.between(-5, 5));
        let by = [0; 3].map(|_: i64| d.between(-40, 40));
        match d.below(4) {
            0 => Step::Move(by),
            1 => Step::Turn(axis, i32::try_from(d.between(1, 3)).unwrap(), centre),
            2 => Step::Mirror(axis, centre),
            _ => Step::Copy([u8::try_from(d.below(200)).unwrap() + 1; 16], by),
        }
    }

    /// The step run by the operation at `c` on `body`.
    fn run(self, c: &OpContext<'_>, body: &Body, exactness: Exactness) -> Result<OpOutput<Made>, String> {
        let moved = Moved {
            body,
            exactness,
            accept: None,
        };
        let out = match self {
            Step::Move(by) => move_by(c, moved, by),
            Step::Turn(axis, turns, centre) => {
                rotate(c, moved, axis, Rotation::quarter_turns(turns), centre)
            }
            Step::Mirror(axis, centre) => mirror(c, moved, axis, centre),
            Step::Copy(key, by) => {
                let copies = pattern(c, moved, &[instance(key, by)]);
                return copies
                    .map_err(|e| format!("{self:?}: {e:?}"))
                    .and_then(|out| {
                        let exactness = out.exactness;
                        out.result
                            .into_iter()
                            .next()
                            .map(|(_, made)| OpOutput::new(made, exactness))
                            .ok_or_else(|| format!("{self:?}: no copy"))
                    });
            }
        };
        out.map_err(|e| format!("{self:?}: {e:?}"))
    }
}

fn instance(key: [u8; 16], by: [i64; 3]) -> Instance {
    Instance {
        key,
        transform: TypedTransform::Exact(GridTransform::translation(by).unwrap()),
    }
}

/// The rules of chapter 20.3 for one history, read from its relations.
fn rules(f: &mut Findings, what: &str, history: &History, inputs: &[NameId], outputs: &[NameId]) {
    let r = history.relations();
    let ins: BTreeSet<NameId> = inputs.iter().copied().collect();
    let outs: BTreeSet<NameId> = outputs.iter().copied().collect();
    let modified: BTreeSet<(NameId, NameId)> = r.modified().collect();
    let generated: BTreeSet<(NameId, NameId)> = r.generated().collect();
    let deleted: BTreeSet<NameId> = r.deleted().collect();
    let nothing: BTreeSet<NameId> = r.from_nothing().collect();
    f.check(
        r.outputs().collect::<BTreeSet<NameId>>() == outs,
        "every output entity has its history",
        || what.to_owned(),
    );
    f.check(
        modified.iter().all(|(i, _)| !deleted.contains(i)),
        "a deleted input has no modified outputs",
        || what.to_owned(),
    );
    f.check(
        modified.is_disjoint(&generated),
        "an input's generated and modified outputs do not overlap",
        || what.to_owned(),
    );
    f.check(
        outs.iter().all(|o| {
            nothing.contains(o) || modified.iter().chain(&generated).any(|(_, x)| x == o)
        }),
        "every output traces to an input or is generated",
        || what.to_owned(),
    );
    f.check(
        modified
            .iter()
            .chain(&generated)
            .map(|(i, _)| i)
            .chain(&deleted)
            .all(|i| ins.contains(i)),
        "nothing is named as an input that is not one",
        || what.to_owned(),
    );
}

/// A chain's composed history by its definition: its modified, generated, from-nothing, deleted
/// and output names.
#[derive(Debug, PartialEq, Eq)]
struct Composed {
    modified: BTreeSet<(NameId, NameId)>,
    generated: BTreeSet<(NameId, NameId)>,
    from_nothing: BTreeSet<NameId>,
    deleted: BTreeSet<NameId>,
    outputs: BTreeSet<NameId>,
}

impl Composed {
    /// The definition, path by path: for each output, every input a path reaches it from, and
    /// whether every step of some such path is a modification.
    fn of(steps: &[Relations]) -> Composed {
        let Some((first, rest)) = steps.split_first() else {
            panic!("an empty chain");
        };
        let mut reach: BTreeMap<NameId, BTreeSet<(NameId, bool)>> = BTreeMap::new();
        for (i, o) in first.modified() {
            reach.entry(o).or_default().insert((i, true));
        }
        for (i, o) in first.generated() {
            reach.entry(o).or_default().insert((i, false));
        }
        let mut outputs: BTreeSet<NameId> = first.outputs().collect();
        for next in rest {
            let mut after: BTreeMap<NameId, BTreeSet<(NameId, bool)>> = BTreeMap::new();
            for (m, o) in next.modified() {
                if let Some(from) = reach.get(&m) {
                    after.entry(o).or_default().extend(from.iter().copied());
                }
            }
            for (m, o) in next.generated() {
                if let Some(from) = reach.get(&m) {
                    after
                        .entry(o)
                        .or_default()
                        .extend(from.iter().map(|&(i, _)| (i, false)));
                }
            }
            reach = after;
            outputs = next.outputs().collect();
        }
        let mut modified = BTreeSet::new();
        let mut generated = BTreeSet::new();
        for (o, from) in &reach {
            for &(i, all_modified) in from {
                if all_modified {
                    modified.insert((i, *o));
                }
            }
        }
        for (o, from) in &reach {
            for &(i, _) in from {
                if !modified.contains(&(i, *o)) {
                    generated.insert((i, *o));
                }
            }
        }
        let from_nothing = outputs
            .iter()
            .filter(|o| reach.get(o).is_none_or(BTreeSet::is_empty))
            .copied()
            .collect();
        let kept: BTreeSet<NameId> = modified.iter().map(|&(i, _)| i).collect();
        let deleted = first
            .deleted()
            .chain(first.modified().map(|(i, _)| i).filter(|i| !kept.contains(i)))
            .collect();
        Composed {
            modified,
            generated,
            from_nothing,
            deleted,
            outputs,
        }
    }

    /// What `geom-op`'s relations say.
    fn read(r: &Relations) -> Composed {
        Composed {
            modified: r.modified().collect(),
            generated: r.generated().collect(),
            from_nothing: r.from_nothing().collect(),
            deleted: r.deleted().collect(),
            outputs: r.outputs().collect(),
        }
    }
}

/// `geom-op`'s composition of a chain, folded from the left.
fn folded(steps: &[Relations]) -> Option<Relations> {
    let (first, rest) = steps.split_first()?;
    Some(rest.iter().fold(first.clone(), |acc, next| acc.then(next)))
}

#[test]
fn every_operation_keeps_the_rules_of_20_3_and_its_chain_composes() {
    let mut d = Draw::new("names: operations and chains", 0x9001);
    let mut f = Findings::default();
    for _ in 0..40 {
        let (what, made) = draw_primitive(&mut d, &ctx_at(1));
        let Ok(out) = made else {
            f.add("a primitive the kernel would not build", || what);
            continue;
        };
        f.ran();
        let mut body = out.result.body;
        let mut exactness = out.exactness;
        rules(&mut f, &what, &out.result.history, &[], &names(&body));
        let mut steps: Vec<(History, NameSet)> = Vec::new();
        let mut done = what.clone();
        for k in 0..d.between(1, 4) {
            let step = Step::draw(&mut d);
            let inputs = names(&body);
            let made = match step.run(&ctx_at(2 + u8::try_from(k).unwrap()), &body, exactness) {
                Ok(m) => m,
                Err(why) => {
                    f.add("a step the kernel refused", || format!("{done} then {why}"));
                    break;
                }
            };
            done = format!("{done} then {step:?}");
            rules(&mut f, &done, &made.result.history, &inputs, &names(&made.result.body));
            steps.push((made.result.history, NameSet::new(inputs).unwrap()));
            body = made.result.body;
            exactness = made.exactness;
        }
        if steps.is_empty() {
            continue;
        }
        f.check(check_chain(&steps).is_ok(), "the kit's chain check passes", || {
            format!("{done}: {:?}", check_chain(&steps))
        });
        let relations: Vec<Relations> = steps.iter().map(|(h, _)| h.relations()).collect();
        let by_definition = Composed::of(&relations);
        let by_fold = folded(&relations).map(|r| Composed::read(&r));
        f.check(by_fold.as_ref() == Some(&by_definition), "a chain composes as its definition says", || {
            format!("{done}: {by_fold:?} against {by_definition:?}")
        });
        f.check(
            by_definition.outputs == names(&body).into_iter().collect(),
            "a chain's outputs are its last body's names",
            || done.clone(),
        );
    }
    f.assert_none("names: operations and chains", 30);
}

#[test]
fn generated_chains_compose_as_their_definition_says() {
    let inputs: Vec<NameId> = (1..=12_u8).map(|i| NameId::from_bytes([i; 16])).collect();
    let result = prop::run(
        "names: generated chains",
        Seed(0x9002),
        128,
        &chain(inputs, 4),
        |steps| {
            let relations: Vec<Relations> = steps.iter().map(|(h, _)| h.relations()).collect();
            let by_definition = Composed::of(&relations);
            let by_fold = folded(&relations).map(|r| Composed::read(&r));
            proptest::prop_assert_eq!(by_fold, Some(by_definition));
            proptest::prop_assert!(check_chain(&steps).is_ok());
            Ok(())
        },
    );
    if let Err(e) = result {
        panic!("{e}");
    }
}

#[test]
fn no_two_entities_share_a_name() {
    let mut d = Draw::new("names: no duplicates", 0x9003);
    let mut f = Findings::default();
    for _ in 0..40 {
        let (what, made) = draw_primitive(&mut d, &ctx_at(1));
        let Ok(out) = made else {
            f.add("a primitive the kernel would not build", || what);
            continue;
        };
        f.ran();
        let source = names(&out.result.body);
        f.check(first_duplicate(&source).is_none(), "a primitive names each entity once", || {
            what.clone()
        });
        let moved = Moved {
            body: &out.result.body,
            exactness: out.exactness,
            accept: None,
        };
        let count = d.between(1, 4);
        let instances: Vec<Instance> = (0..count)
            .map(|k| instance([u8::try_from(k).unwrap() + 1; 16], [40 * (k + 1), 0, 0]))
            .collect();
        match pattern(&ctx_at(2), moved, &instances) {
            Ok(copies) => {
                let mut all = source.clone();
                for (_, copy) in &copies.result {
                    all.extend(names(&copy.body));
                }
                f.check(
                    first_duplicate(&all).is_none(),
                    "a pattern's copies and their source share no name",
                    || format!("{what}, {count} copies"),
                );
            }
            Err(e) => f.add("a pattern the kernel refused", || format!("{what}: {e:?}")),
        }
    }
    f.assert_none("names: no duplicates", 30);
}

#[test]
fn untouched_entities_keep_their_names() {
    let mut d = Draw::new("names: untouched entities", 0x9004);
    let mut f = Findings::default();
    for _ in 0..40 {
        let (what, made) = draw_primitive(&mut d, &ctx_at(1));
        let Ok(out) = made else {
            f.add("a primitive the kernel would not build", || what);
            continue;
        };
        f.ran();
        let body = &out.result.body;
        // A transform keeps every name.
        let step = loop {
            let s = Step::draw(&mut d);
            if !matches!(s, Step::Copy(..)) {
                break s;
            }
        };
        match step.run(&ctx_at(2), body, out.exactness) {
            Ok(moved) => f.check(
                names(&moved.result.body) == names(body),
                "a transform keeps every name",
                || format!("{what} then {step:?}"),
            ),
            Err(why) => f.add("a transform the kernel refused", || format!("{what}: {why}")),
        }
        // Adding a copy to a pattern renames no other copy.
        let moved = Moved {
            body,
            exactness: out.exactness,
            accept: None,
        };
        let one = pattern(&ctx_at(3), moved, &[instance([7; 16], [50, 0, 0])]);
        let two = pattern(
            &ctx_at(3),
            moved,
            &[instance([7; 16], [50, 0, 0]), instance([9; 16], [0, 50, 0])],
        );
        match (one, two) {
            (Ok(one), Ok(two)) => {
                let first = |copies: &[([u8; 16], Made)]| {
                    copies
                        .iter()
                        .find(|(k, _)| *k == [7; 16])
                        .map(|(_, m)| names(&m.body))
                };
                f.check(
                    first(&one.result).is_some() && first(&one.result) == first(&two.result),
                    "adding a copy renames no other copy",
                    || what.clone(),
                );
            }
            (a, b) => f.add("a pattern the kernel refused", || format!("{what}: {:?} {:?}", a.err(), b.err())),
        }
    }
    f.assert_none("names: untouched entities", 30);
}

#[test]
fn a_primitives_names_do_not_depend_on_its_place_or_size() {
    let mut d = Draw::new("names: roles, not places", 0x9005);
    let mut f = Findings::default();
    for _ in 0..40 {
        f.ran();
        let low = |d: &mut Draw| [0; 3].map(|_: i64| d.between(-30, 30));
        let size = |d: &mut Draw| [0; 3].map(|_: u64| d.between(1, 20) as u64);
        let boxes = [(low(&mut d), size(&mut d)), (low(&mut d), size(&mut d))].map(|(l, s)| {
            make_box(
                &ctx_at(1),
                &BoxInput {
                    min: p(l[0], l[1], l[2]),
                    size: s.map(Steps),
                },
            )
            .map(|o| names(&o.result.body))
        });
        let [a, b] = boxes;
        f.check(a.is_ok() && a == b, "two boxes of one operation have the same names", || {
            format!("{a:?} {b:?}")
        });
        let radius = |d: &mut Draw| Steps(d.between(2, 12) as u64);
        let round = |d: &mut Draw| RoundInput {
            base: p(d.between(-30, 30), d.between(-30, 30), d.between(-30, 30)),
            radius: radius(d),
            height: radius(d),
            step: GridStep::Q0,
        };
        let (r1, r2) = (round(&mut d), round(&mut d));
        let (c1, c2) = (cylinder(&ctx_at(1), &r1), cylinder(&ctx_at(1), &r2));
        let (n1, n2) = (
            c1.map(|o| names(&o.result.body)),
            c2.map(|o| names(&o.result.body)),
        );
        f.check(n1.is_ok() && n1 == n2, "two cylinders of one operation have the same names", || {
            format!("{r1:?} {r2:?}")
        });
    }
    f.assert_none("names: roles, not places", 40);
}

/// The corpus of the names golden: from a fixed stream, 60 primitives, each followed by up to
/// three steps, every body's names and every history in its canonical encoding, in order.
fn corpus() -> (Vec<u8>, usize) {
    let mut d = Draw(SplitMix64::new(geom_test::seed::fixed(
        "names on every target",
        Seed(0x9006),
    )));
    let mut bytes = Vec::new();
    let mut count = 0_usize;
    for _ in 0..60 {
        let (_, made) = draw_primitive(&mut d, &ctx_at(1));
        let Ok(out) = made else {
            bytes.push(0);
            continue;
        };
        let mut body = out.result.body;
        let mut exactness = out.exactness;
        let mut record = |bytes: &mut Vec<u8>, at: u8, body: &Body, history: &History| {
            for name in names(body) {
                bytes.extend(name.to_bytes());
                count += 1;
            }
            match history.encode(ctx_at(at).path()) {
                Ok(item) => bytes.extend(item.into_bytes()),
                Err(_) => bytes.push(1),
            }
        };
        record(&mut bytes, 1, &body, &out.result.history);
        for k in 0..d.between(0, 3) {
            let at = 2 + u8::try_from(k).unwrap();
            match Step::draw(&mut d).run(&ctx_at(at), &body, exactness) {
                Ok(made) => {
                    record(&mut bytes, at, &made.result.body, &made.result.history);
                    body = made.result.body;
                    exactness = made.exactness;
                }
                Err(_) => bytes.push(2),
            }
        }
    }
    (bytes, count)
}

#[test]
fn the_same_names_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let (bytes, count) = corpus();
        (sha256(&bytes), (bytes, count))
    });
    let (bytes, count) = &checked.value;
    println!(
        "names corpus: {count} names, {} bytes, golden hash {}",
        bytes.len(),
        hex(sha256(bytes))
    );
    assert!(*count > 1000, "only {count} names");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target names an entity, orders a history or \
             encodes a name differently (compare this log's digest with another target's), or a \
             name's definition changed in place, which chapter 20.2 forbids"
        )
    });
}
