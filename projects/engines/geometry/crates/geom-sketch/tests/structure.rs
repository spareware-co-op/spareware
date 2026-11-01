//! Tests of `geom_sketch::structure` (piece G221): substitution, components, matching and the
//! Dulmage–Mendelsohn decomposition.
//!
//! The board's done-when: "Variable merging, components, Hopcroft-Karp matching, the
//! Dulmage-Mendelsohn block triangular form and exact structural counts, iterative with budgeted
//! stacks; identical plans under shuffled input; 100,000 equations in 50 ms." The benchmark is
//! `benches/structure.rs`.
//!
//! What this file proves: on worked examples, each part is what the definition says; on random
//! graphs, the matching's size equals a plain augmenting search's, the parts partition the graph,
//! each block's equations read only what comes before them, and each square block has a perfect
//! matching; a chain long enough to overflow any recursion is walked; a sketch authored in any order
//! gets one plan; and a budget too small refuses.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::range_plus_one,
    clippy::too_many_lines,
    reason = "tests, written as the definitions read; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_num::{GridStep, P3};
use geom_run::{Budget, WorkMeter};
use geom_sketch::model::{
    Constraint, ConstraintKind, Entity, EntityKind, Extent, ItemId, Sketch, SketchError, Standing,
};
use geom_sketch::plane::{Lattice, PlaneFrame, SketchPlane};
use geom_sketch::structure::{Analysis, Component, Graph, Part, analyse, substitute};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn graph(variables: usize, rows: &[&[usize]]) -> Graph {
    Graph::new(variables, rows.iter().copied()).unwrap()
}

fn plan(variables: usize, rows: &[&[usize]]) -> Analysis {
    analyse(&graph(variables, rows), &mut meter()).unwrap()
}

fn part(equations: &[usize], variables: &[usize]) -> Part {
    Part {
        equations: equations.to_vec(),
        variables: variables.to_vec(),
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 20
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[test]
fn a_graph_keeps_each_row_sorted_and_without_repeats() {
    let g = graph(4, &[&[3, 1, 3], &[], &[0]]);
    assert_eq!((g.equations(), g.variables()), (3, 4));
    assert_eq!(g.row(0).unwrap(), [1, 3]);
    assert_eq!(g.row(1).unwrap(), [] as [usize; 0]);
    assert_eq!(g.row(2).unwrap(), [0]);
    assert_eq!(g.row(3), Err(SketchError::Internal));
    assert_eq!(
        Graph::new(2, [&[2_usize][..]].into_iter()),
        Err(SketchError::Internal)
    );
}

#[test]
fn a_triangular_system_is_a_chain_of_blocks_in_solve_order() {
    // x0 alone, then x1 from x0, then x2 from both.
    let a = plan(3, &[&[0, 1, 2], &[0, 1], &[0]]);
    assert_eq!(a.freedoms, 0);
    assert_eq!(a.components.len(), 1);
    let c = &a.components[0];
    assert_eq!(
        c.blocks,
        [part(&[2], &[0]), part(&[1], &[1]), part(&[0], &[2])]
    );
    assert_eq!((c.freedoms, c.excess), (0, 0));
    assert_eq!(c.over, Part::default());
    assert_eq!(c.under, Part::default());
}

#[test]
fn a_coupled_pair_is_one_block() {
    let a = plan(3, &[&[0, 1], &[0, 1], &[1, 2]]);
    let c = &a.components[0];
    assert_eq!(c.blocks, [part(&[0, 1], &[0, 1]), part(&[2], &[2])]);
}

#[test]
fn extra_equations_are_over_determined() {
    // Three equations on two variables, and a third variable solved after them.
    let a = plan(3, &[&[0, 1], &[0, 1], &[0, 1], &[1, 2]]);
    let c = &a.components[0];
    assert_eq!(c.over, part(&[0, 1, 2], &[0, 1]));
    assert_eq!(c.excess, 1);
    assert_eq!(c.blocks, [part(&[3], &[2])]);
    assert_eq!(c.freedoms, 0);
}

#[test]
fn extra_variables_are_under_determined() {
    let a = plan(3, &[&[0, 1, 2]]);
    let c = &a.components[0];
    assert_eq!(c.under, part(&[0], &[0, 1, 2]));
    assert_eq!(c.freedoms, 2);
    assert_eq!(a.freedoms, 2);
    assert!(c.blocks.is_empty(), "{:?}", c.blocks);
    // A square equation on what the under-determined part leaves.
    let a = plan(3, &[&[0], &[0, 1, 2]]);
    let c = &a.components[0];
    assert_eq!(c.blocks, [part(&[0], &[0])]);
    assert_eq!(c.under, part(&[1], &[1, 2]));
    assert_eq!(c.freedoms, 1);
}

#[test]
fn all_three_parts_at_once() {
    // Over: e0, e1 on x0. Square: e2 on x0 and x1. Under: e3 on x1, x2, x3.
    let a = plan(4, &[&[0], &[0], &[0, 1], &[1, 2, 3]]);
    let c = &a.components[0];
    assert_eq!(c.over, part(&[0, 1], &[0]));
    assert_eq!(c.blocks, [part(&[2], &[1])]);
    assert_eq!(c.under, part(&[3], &[2, 3]));
    assert_eq!((c.excess, c.freedoms), (1, 1));
}

#[test]
fn parts_that_do_not_connect_are_components() {
    let a = plan(5, &[&[3, 4], &[0], &[3]]);
    assert_eq!(a.components.len(), 4);
    assert_eq!(a.components[0].blocks, [part(&[1], &[0])]);
    // A variable in no equation is a component of its own, one freedom.
    assert_eq!(a.components[1].under, part(&[], &[1]));
    assert_eq!(a.components[2].under, part(&[], &[2]));
    assert_eq!(a.components[3].blocks, [part(&[2], &[3]), part(&[0], &[4])]);
    assert_eq!(a.freedoms, 2);
    // An equation of no variable is a component of its own, and over-determined.
    let a = plan(1, &[&[0], &[]]);
    assert_eq!(a.components.len(), 2);
    assert_eq!(
        a.components[1],
        Component {
            over: part(&[1], &[]),
            excess: 1,
            ..Component::default()
        }
    );
    assert_eq!(plan(0, &[]).components, []);
}

/// The size of a largest matching by the plain augmenting search of the definition.
fn plain_matching(g: &Graph) -> usize {
    fn reach(g: &Graph, e: usize, seen: &mut [bool], owner: &mut [Option<usize>]) -> bool {
        for &v in g.row(e).unwrap() {
            if !seen[v] {
                seen[v] = true;
                if owner[v].is_none_or(|w| reach(g, w, seen, owner)) {
                    owner[v] = Some(e);
                    return true;
                }
            }
        }
        false
    }
    let mut owner = vec![None; g.variables()];
    (0..g.equations())
        .filter(|&e| reach(g, e, &mut vec![false; g.variables()], &mut owner))
        .count()
}

#[test]
fn the_laws_hold_on_random_graphs() {
    let mut rng = Lcg(0x221);
    for round in 0..3000 {
        let (m, n) = (1 + rng.below(14), 1 + rng.below(14));
        let rows: Vec<Vec<usize>> = (0..m)
            .map(|_| {
                (0..1 + rng.below(1 + round % 4))
                    .map(|_| rng.below(n))
                    .collect()
            })
            .collect();
        let g = Graph::new(n, rows.iter().map(Vec::as_slice)).unwrap();
        let a = analyse(&g, &mut meter()).unwrap();
        // The parts partition the graph.
        let (mut es, mut vs) = (vec![0; m], vec![0; n]);
        let mut matched = 0;
        for c in &a.components {
            let mut seen = vec![false; n];
            for p in [&c.over, &c.under].into_iter().chain(&c.blocks) {
                for &e in &p.equations {
                    es[e] += 1;
                }
                for &v in &p.variables {
                    vs[v] += 1;
                }
            }
            // The over part reads only its own variables, and the blocks only what comes before
            // them or the under part: the solve order.
            for &e in &c.over.equations {
                for v in g.row(e).unwrap() {
                    assert!(c.over.variables.contains(v), "round {round}");
                }
            }
            for &v in &c.under.variables {
                seen[v] = true;
            }
            for &v in &c.over.variables {
                seen[v] = true;
            }
            for block in &c.blocks {
                assert_eq!(
                    block.equations.len(),
                    block.variables.len(),
                    "round {round}"
                );
                matched += block.equations.len();
                for &v in &block.variables {
                    seen[v] = true;
                }
                for &e in &block.equations {
                    for &v in g.row(e).unwrap() {
                        assert!(seen[v], "round {round}: {block:?} reads {v} too soon");
                    }
                }
            }
            matched += c.over.variables.len() + c.under.equations.len();
        }
        assert!(es.iter().all(|&k| k == 1), "round {round}");
        assert!(vs.iter().all(|&k| k == 1), "round {round}");
        // Every largest matching has the size the parts add up to.
        assert_eq!(matched, plain_matching(&g), "round {round}");
    }
}

#[test]
fn a_long_chain_is_walked_without_recursion() {
    // Each equation reads the one variable before it: one block a link, in order.
    let n = 300_000;
    let rows: Vec<Vec<usize>> = (0..n)
        .map(|i| if i == 0 { vec![0] } else { vec![i - 1, i] })
        .collect();
    let g = Graph::new(n, rows.iter().map(Vec::as_slice)).unwrap();
    let a = analyse(&g, &mut meter()).unwrap();
    assert_eq!(a.components.len(), 1);
    assert_eq!(a.components[0].blocks.len(), n);
    assert_eq!(a.components[0].blocks[n - 1], part(&[n - 1], &[n - 1]));
    // The same chain the other way round, so each augmenting path and each walk is as long as it
    // can be: every link was matched to the wrong variable by the first pass.
    let rows: Vec<Vec<usize>> = (0..n)
        .map(|i| if i + 1 == n { vec![i] } else { vec![i, i + 1] })
        .rev()
        .collect();
    let g = Graph::new(n, rows.iter().map(Vec::as_slice)).unwrap();
    let a = analyse(&g, &mut meter()).unwrap();
    assert_eq!(a.components[0].blocks.len(), n);
    // One long cycle is one block.
    let rows: Vec<Vec<usize>> = (0..n).map(|i| vec![i, (i + 1) % n]).collect();
    let g = Graph::new(n, rows.iter().map(Vec::as_slice)).unwrap();
    let a = analyse(&g, &mut meter()).unwrap();
    assert_eq!(a.components[0].blocks.len(), 1);
    assert_eq!(a.components[0].blocks[0].equations.len(), n);
}

#[test]
fn a_budget_too_small_stops_it() {
    let g = graph(2, &[&[0], &[0, 1]]);
    let mut none = WorkMeter::new(Budget::of_work(1));
    assert_eq!(analyse(&g, &mut none), Err(SketchError::Stopped));
    let mut enough = WorkMeter::new(Budget::of_work(1_000));
    assert!(analyse(&g, &mut enough).is_ok());
}

fn sketch() -> Sketch {
    let lattice = Lattice::new(P3::new(0, 0, 0).unwrap(), [1, 0, 0], [0, 1, 0]).unwrap();
    Sketch::new(SketchPlane::new(PlaneFrame::Lattice(lattice), GridStep::Q0))
}

fn point(key: u64, x: f64, y: f64) -> Entity {
    Entity {
        id: ItemId::new(1, key),
        kind: EntityKind::Point { x, y },
        construction: false,
    }
}

fn line(key: u64, a: u64, b: u64) -> Entity {
    Entity {
        id: ItemId::new(2, key),
        kind: EntityKind::Line {
            start: ItemId::new(1, a),
            end: ItemId::new(1, b),
            extent: Extent::Segment,
        },
        construction: false,
    }
}

fn constraint(key: u64, kind: ConstraintKind) -> Constraint {
    Constraint {
        id: ItemId::new(3, key),
        kind,
        standing: Standing::Driving,
        priority: 0,
    }
}

/// A rectangle: four points, four lines, horizontals and verticals, two sizes, and a fixed corner.
fn rectangle() -> (Vec<Entity>, Vec<Constraint>) {
    let p = |k| ItemId::new(1, k);
    let entities = vec![
        point(0, 0.0, 0.0),
        point(1, 40.0, 1.0),
        point(2, 41.0, 30.0),
        point(3, 1.0, 29.0),
        line(0, 0, 1),
        line(1, 1, 2),
        line(2, 2, 3),
        line(3, 3, 0),
    ];
    let constraints = vec![
        constraint(
            0,
            ConstraintKind::Horizontal {
                line: ItemId::new(2, 0),
            },
        ),
        constraint(
            1,
            ConstraintKind::Vertical {
                line: ItemId::new(2, 1),
            },
        ),
        constraint(
            2,
            ConstraintKind::Horizontal {
                line: ItemId::new(2, 2),
            },
        ),
        constraint(
            3,
            ConstraintKind::Vertical {
                line: ItemId::new(2, 3),
            },
        ),
        constraint(
            4,
            ConstraintKind::Distance {
                a: p(0),
                b: p(1),
                distance: 40.0,
            },
        ),
        constraint(
            5,
            ConstraintKind::Distance {
                a: p(1),
                b: p(2),
                distance: 30.0,
            },
        ),
        constraint(
            6,
            ConstraintKind::Fix {
                entity: p(0),
                slot: 0,
                value: 0.0,
            },
        ),
        constraint(
            7,
            ConstraintKind::Fix {
                entity: p(0),
                slot: 1,
                value: 0.0,
            },
        ),
    ];
    (entities, constraints)
}

#[test]
fn a_rectangle_merges_to_four_variables_in_order() {
    let (entities, constraints) = rectangle();
    let mut s = sketch();
    for e in entities {
        s.add_entity(e).unwrap();
    }
    for c in constraints {
        s.add_constraint(c).unwrap();
    }
    let sub = substitute(&s).unwrap();
    // Eight coordinates, merged by the four equalities: x0, y0, x1, y2.
    assert_eq!(sub.graph.variables(), 4);
    assert_eq!(sub.merged, [0, 1, 2, 1, 2, 3, 0, 3]);
    assert_eq!(sub.graph.equations(), 4);
    assert_eq!(
        sub.owners,
        [
            ItemId::new(3, 4),
            ItemId::new(3, 5),
            ItemId::new(3, 6),
            ItemId::new(3, 7)
        ]
    );
    assert_eq!(sub.graph.row(0).unwrap(), [0, 1, 2]);
    let a = analyse(&sub.graph, &mut meter()).unwrap();
    assert_eq!(a.freedoms, 0);
    // The two fixes, then each size from its fix.
    let order: Vec<&[usize]> = a
        .components
        .iter()
        .flat_map(|c| &c.blocks)
        .map(|b| b.equations.as_slice())
        .collect();
    assert_eq!(a.components.iter().map(|c| c.excess).sum::<usize>(), 0);
    assert_eq!(order.iter().map(|b| b.len()).sum::<usize>(), 4);
}

#[test]
fn a_reference_dimension_adds_no_equation_and_a_coincidence_merges_all_of_it() {
    let p = |k| ItemId::new(1, k);
    let mut s = sketch();
    s.add_entity(point(0, 0.0, 0.0)).unwrap();
    s.add_entity(point(1, 3.0, 4.0)).unwrap();
    s.add_entity(point(2, 9.0, 9.0)).unwrap();
    s.add_constraint(constraint(
        0,
        ConstraintKind::Coincident { a: p(0), b: p(2) },
    ))
    .unwrap();
    s.add_constraint(Constraint {
        standing: Standing::Reference,
        ..constraint(
            1,
            ConstraintKind::Distance {
                a: p(0),
                b: p(1),
                distance: 5.0,
            },
        )
    })
    .unwrap();
    let sub = substitute(&s).unwrap();
    assert_eq!(sub.graph.equations(), 0);
    assert_eq!(sub.graph.variables(), 4);
    assert_eq!(sub.merged, [0, 1, 2, 3, 0, 1]);
    assert_eq!(sub.owners, []);
    assert_eq!(analyse(&sub.graph, &mut meter()).unwrap().freedoms, 4);
}

#[test]
fn a_sketch_authored_in_any_order_has_one_plan() {
    let (entities, constraints) = rectangle();
    let mut first = sketch();
    for e in entities.clone() {
        first.add_entity(e).unwrap();
    }
    for c in &constraints {
        first.add_constraint(*c).unwrap();
    }
    let want = substitute(&first).unwrap();
    let plan_of =
        |sub: &geom_sketch::structure::Substituted| analyse(&sub.graph, &mut meter()).unwrap();
    let mut rng = Lcg(0x222);
    for _ in 0..200 {
        // Points first, since lines name them; each kind shuffled.
        let mut points: Vec<Entity> = entities[..4].to_vec();
        let mut lines: Vec<Entity> = entities[4..].to_vec();
        let mut rules = constraints.clone();
        for i in (1..4).rev() {
            points.swap(i, rng.below(i + 1));
            lines.swap(i, rng.below(i + 1));
        }
        for i in (1..rules.len()).rev() {
            rules.swap(i, rng.below(i + 1));
        }
        let mut s = sketch();
        for e in points.into_iter().chain(lines) {
            s.add_entity(e).unwrap();
        }
        for c in rules {
            s.add_constraint(c).unwrap();
        }
        let sub = substitute(&s).unwrap();
        assert_eq!(sub, want);
        assert_eq!(plan_of(&sub), plan_of(&want));
    }
}
