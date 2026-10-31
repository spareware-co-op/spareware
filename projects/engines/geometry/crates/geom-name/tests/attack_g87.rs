//! Attacks on piece G87's claims in `geom-name`: the name graph keeps one encoding per `NameId`,
//! lists names in name order and decides weakness exactly; the ordinal order is 20.2's rule,
//! independent of input order, with every undecided comparison and every tie refused; the
//! duplicate detector finds exactly the repeated names.
//!
//! The truth for the ordinal order is a reference sort written here: the standard library's sort
//! on each entity's (least point, vertices, direction) key, which is the rule's lexicographic order
//! when every comparison is decided and no two keys are equal.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests: native arithmetic and indexing, and a failing test should stop loudly"
)]

use core::cmp::Ordering;
use core::num::NonZeroU64;

use geom_name::{
    DuplicateName, GraphError, NameGraph, OrderError, WeakOrder, expect_unique, first_duplicate,
    ordinals, parents, reference, weak_names,
};
use geom_op::name::{
    EdgeOrVertex, NameList, NameSet, Ordinal, PieceKey, Role, RoleArg, Side, TopoName,
};
use geom_op::{NameId, OpId, OpPath};
use geom_test::rng::SplitMix64;
use geom_test::seed::{Seed, resolve};

fn id(byte: u8) -> NameId {
    NameId::from_bytes([byte; 16])
}

fn set(names: &[NameId]) -> NameSet {
    NameSet::new(names.iter().copied()).unwrap()
}

fn op() -> OpPath {
    OpPath::new(OpId::from_bytes([7; 16]))
}

/// A colliding entity whose three keys the test chooses; `None` is a comparison the operation
/// could not decide.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entity {
    least: Option<i64>,
    vertices: Option<Vec<i64>>,
    direction: Option<i64>,
}

impl Entity {
    fn decided(least: i64, vertices: Vec<i64>, direction: i64) -> Entity {
        Entity {
            least: Some(least),
            vertices: Some(vertices),
            direction: Some(direction),
        }
    }

    fn key(&self) -> (i64, Vec<i64>, i64) {
        (
            self.least.unwrap(),
            self.vertices.clone().unwrap(),
            self.direction.unwrap(),
        )
    }
}

impl WeakOrder for Entity {
    fn least_point(&self, other: &Self) -> Option<Ordering> {
        Some(self.least?.cmp(&other.least?))
    }

    fn vertices(&self, other: &Self) -> Option<Ordering> {
        Some(self.vertices.as_ref()?.cmp(other.vertices.as_ref()?))
    }

    fn direction(&self, other: &Self) -> Option<Ordering> {
        Some(self.direction?.cmp(&other.direction?))
    }
}

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.below(NonZeroU64::new(n).unwrap())
}

/// Random entities with distinct keys drawn from small ranges, so many share a least point or
/// their vertices and the later steps decide.
fn entities(rng: &mut SplitMix64, n: usize) -> Vec<Entity> {
    let mut out: Vec<Entity> = Vec::new();
    while out.len() < n {
        let e = Entity::decided(
            below(rng, 4) as i64,
            (0..below(rng, 3)).map(|_| below(rng, 3) as i64).collect(),
            below(rng, 5) as i64 - 2,
        );
        if !out.iter().any(|o| o.key() == e.key()) {
            out.push(e);
        }
    }
    out
}

#[test]
fn ordinals_follow_the_rule_and_not_the_input_order() {
    let seed = resolve("g87 ordinal order", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    for case in 0..2000 {
        let n = below(&mut rng, 30) as usize;
        let es = entities(&mut rng, n);
        let got = ordinals(&es).unwrap();
        // The reference: each entity's rank by its full key.
        let mut by_key: Vec<usize> = (0..n).collect();
        by_key.sort_by_key(|&i| es[i].key());
        let mut want = vec![0_u32; n];
        for (rank, &i) in by_key.iter().enumerate() {
            want[i] = rank as u32;
        }
        let ns: Vec<u32> = got.iter().map(|o| o.n()).collect();
        assert_eq!(ns, want, "case {case}: {es:?}");
        assert!(got.iter().all(|o| o.of() == n as u32));
        // Shuffled, each entity keeps its ordinal.
        let mut order: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            order.swap(i, below(&mut rng, i as u64 + 1) as usize);
        }
        let shuffled: Vec<Entity> = order.iter().map(|&i| es[i].clone()).collect();
        let again = ordinals(&shuffled).unwrap();
        for (k, &i) in order.iter().enumerate() {
            assert_eq!(again[k], got[i], "case {case}");
        }
    }
}

#[test]
fn each_step_decides_only_when_the_ones_before_it_tie() {
    // Least point decides first, whatever the later keys say.
    let es = [
        Entity::decided(1, vec![0], 0),
        Entity::decided(0, vec![9], 9),
    ];
    assert_eq!(
        ordinals(&es)
            .unwrap()
            .iter()
            .map(|o| o.n())
            .collect::<Vec<_>>(),
        [1, 0]
    );
    // Equal least points: the vertex lists, compared like words (a prefix comes first).
    let es = [
        Entity::decided(0, vec![1, 2], 0),
        Entity::decided(0, vec![1], 5),
    ];
    assert_eq!(
        ordinals(&es)
            .unwrap()
            .iter()
            .map(|o| o.n())
            .collect::<Vec<_>>(),
        [1, 0]
    );
    // Equal least points and vertices: the direction.
    let es = [
        Entity::decided(0, vec![1], 3),
        Entity::decided(0, vec![1], -3),
    ];
    assert_eq!(
        ordinals(&es)
            .unwrap()
            .iter()
            .map(|o| o.n())
            .collect::<Vec<_>>(),
        [1, 0]
    );
    // A later step is never asked when an earlier one decides: undecided vertices do not matter.
    let es = [
        Entity {
            least: Some(0),
            vertices: None,
            direction: None,
        },
        Entity {
            least: Some(1),
            vertices: None,
            direction: None,
        },
    ];
    assert_eq!(
        ordinals(&es)
            .unwrap()
            .iter()
            .map(|o| o.n())
            .collect::<Vec<_>>(),
        [0, 1]
    );
}

#[test]
fn a_tie_or_an_undecided_comparison_is_refused_never_guessed() {
    // Entities 1 and 3 tie on every step.
    let es = [
        Entity::decided(0, vec![], 0),
        Entity::decided(5, vec![1], 1),
        Entity::decided(2, vec![], 0),
        Entity::decided(5, vec![1], 1),
    ];
    assert_eq!(ordinals(&es), Err(OrderError::UndecidedOrder(1, 3)));
    // Entity 2's least point cannot be decided.
    let mut es: Vec<Entity> = (0..5).map(|i| Entity::decided(i, vec![], 0)).collect();
    es[2].least = None;
    match ordinals(&es) {
        Err(OrderError::UndecidedOrder(a, b)) => assert!(a == 2 || b == 2, "{a} {b}"),
        other => panic!("{other:?}"),
    }
    // A tie found only at the last step, with the earlier steps equal.
    let es = [
        Entity::decided(0, vec![4], 1),
        Entity::decided(0, vec![4], 1),
    ];
    assert_eq!(ordinals(&es), Err(OrderError::UndecidedOrder(0, 1)));
    // The direction undecided where the earlier steps tie.
    let es = [
        Entity {
            direction: None,
            ..Entity::decided(0, vec![4], 1)
        },
        Entity::decided(0, vec![4], 2),
    ];
    assert_eq!(ordinals(&es), Err(OrderError::UndecidedOrder(0, 1)));
    for e in [OrderError::UndecidedOrder(0, 1), OrderError::TooMany] {
        assert_ne!(e.to_string(), "");
    }
}

#[test]
fn one_entity_none_and_weak_names() {
    assert_eq!(ordinals::<Entity>(&[]), Ok(vec![]));
    let one = [Entity::decided(0, vec![], 0)];
    assert_eq!(ordinals(&one), Ok(vec![Ordinal::new(0, 1).unwrap()]));
    let es = [Entity::decided(3, vec![], 0), Entity::decided(1, vec![], 0)];
    let names = weak_names(id(9), &es).unwrap();
    assert_eq!(
        names,
        vec![
            TopoName::Weak {
                base: id(9),
                ord: Ordinal::new(1, 2).unwrap()
            },
            TopoName::Weak {
                base: id(9),
                ord: Ordinal::new(0, 2).unwrap()
            },
        ]
    );
    let tied = [Entity::decided(0, vec![], 0), Entity::decided(0, vec![], 0)];
    assert_eq!(
        weak_names(id(9), &tied),
        Err(OrderError::UndecidedOrder(0, 1))
    );
}

/// An entity with no size, so a slice longer than a `u32` counts costs no memory.
#[derive(Clone, Copy)]
struct Point;

impl WeakOrder for Point {
    fn least_point(&self, _: &Self) -> Option<Ordering> {
        Some(Ordering::Equal)
    }
    fn vertices(&self, _: &Self) -> Option<Ordering> {
        Some(Ordering::Equal)
    }
    fn direction(&self, _: &Self) -> Option<Ordering> {
        Some(Ordering::Equal)
    }
}

#[cfg(target_pointer_width = "64")]
#[test]
fn more_entities_than_a_u32_counts_are_refused_before_sorting() {
    // 2^32 entities of no size: the array occupies no memory, and nothing is compared.
    let many = [Point; 1 << 32];
    assert_eq!(ordinals(&many[..]), Err(OrderError::TooMany));
}

#[test]
fn the_reference_is_the_lowest_name_every_entity_touches() {
    assert_eq!(reference(&[]), None);
    assert_eq!(reference(&[set(&[id(5), id(2)])]), Some(id(2)));
    assert_eq!(
        reference(&[
            set(&[id(5), id(2), id(9)]),
            set(&[id(9), id(5)]),
            set(&[id(5), id(9), id(1)])
        ]),
        Some(id(5))
    );
    assert_eq!(reference(&[set(&[id(1)]), set(&[id(2)])]), None);
    assert_eq!(reference(&[set(&[]), set(&[id(2)])]), None);
}

fn created(role: &str, args: Vec<RoleArg>, from: &[NameId]) -> TopoName {
    TopoName::Created {
        op: op(),
        role: Role::new(role, args).unwrap(),
        from: NameList::new(from.iter().copied()),
    }
}

#[test]
fn interning_keeps_one_node_per_name_in_name_order() {
    let mut graph = NameGraph::new();
    assert!(graph.is_empty());
    let names = [
        created("cap.start", vec![], &[]),
        created("cap.end", vec![], &[]),
        created("side", vec![RoleArg::Count(2)], &[]),
        TopoName::Merged {
            op: op(),
            of: set(&[id(1), id(2)]),
        },
    ];
    let ids: Vec<NameId> = names
        .iter()
        .map(|n| graph.intern(n.clone()).unwrap())
        .collect();
    for (name, &got) in names.iter().zip(&ids) {
        assert_eq!(got, name.id().unwrap());
        assert_eq!(graph.intern(name.clone()), Ok(got));
        assert_eq!(graph.name(got), Some(name));
        assert_eq!(graph.bytes(got), Some(name.encode().unwrap().as_bytes()));
    }
    assert_eq!(graph.len(), 4);
    assert!(!graph.is_empty());
    let listed: Vec<NameId> = graph.ids().collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(listed, sorted);
    assert_eq!(graph.name(id(0)), None);
    assert_eq!(graph.bytes(id(0)), None);
}

#[test]
fn weakness_is_found_at_any_depth_and_a_missing_parent_is_an_error() {
    let mut graph = NameGraph::new();
    let face = graph.intern(created("top", vec![], &[])).unwrap();
    let other = graph.intern(created("base", vec![], &[])).unwrap();
    let weak = graph
        .intern(TopoName::Weak {
            base: face,
            ord: Ordinal::new(0, 2).unwrap(),
        })
        .unwrap();
    // Built on the weak name through a role argument, then a split, then a bound.
    let on_weak = graph
        .intern(created("roll", vec![RoleArg::Name(weak)], &[]))
        .unwrap();
    let split = graph
        .intern(TopoName::Split {
            op: op(),
            of: other,
            piece: PieceKey::ByTools {
                tools: set(&[on_weak]),
                side: Side::Back,
            },
        })
        .unwrap();
    let bounded = graph
        .intern(TopoName::Bounded {
            kind: EdgeOrVertex::Edge,
            by: set(&[face, other]),
            ends: set(&[split]),
        })
        .unwrap();
    let strong = graph
        .intern(TopoName::Bounded {
            kind: EdgeOrVertex::Vertex,
            by: set(&[face, other]),
            ends: set(&[]),
        })
        .unwrap();
    for (name, is) in [
        (face, false),
        (weak, true),
        (on_weak, true),
        (split, true),
        (bounded, true),
        (strong, false),
    ] {
        assert_eq!(graph.is_weak(name), Ok(is), "{name}");
    }
    // A shared parent is visited once: a name reaching `face` twice is still strong.
    let twice = graph
        .intern(TopoName::Merged {
            op: op(),
            of: set(&[strong, face]),
        })
        .unwrap();
    assert_eq!(graph.is_weak(twice), Ok(false));
    let dangling = graph.intern(created("x", vec![], &[id(0xee)])).unwrap();
    assert_eq!(graph.is_weak(dangling), Err(GraphError::Missing(id(0xee))));
    assert_eq!(graph.is_weak(id(0xdd)), Err(GraphError::Missing(id(0xdd))));
    for e in [
        GraphError::Missing(id(1)),
        GraphError::Collision(id(1)),
        GraphError::Name(geom_op::name::NameError::NotNfc),
    ] {
        assert_ne!(e.to_string(), "");
    }
}

#[test]
fn parents_are_every_name_a_name_refers_to_in_encoding_order() {
    assert_eq!(
        parents(&created(
            "side",
            vec![
                RoleArg::Name(id(5)),
                RoleArg::Count(1),
                RoleArg::Name(id(3))
            ],
            &[id(2), id(1)]
        )),
        vec![id(5), id(3), id(1), id(2)]
    );
    assert_eq!(
        parents(&TopoName::Split {
            op: op(),
            of: id(4),
            piece: PieceKey::ByTools {
                tools: set(&[id(3), id(1)]),
                side: Side::Front
            }
        }),
        vec![id(4), id(1), id(3)]
    );
    assert_eq!(
        parents(&TopoName::Merged {
            op: op(),
            of: set(&[id(2), id(1)])
        }),
        vec![id(1), id(2)]
    );
    assert_eq!(
        parents(&TopoName::Bounded {
            kind: EdgeOrVertex::Edge,
            by: set(&[id(2), id(1)]),
            ends: set(&[id(3)])
        }),
        vec![id(1), id(2), id(3)]
    );
    assert_eq!(
        parents(&TopoName::Weak {
            base: id(6),
            ord: Ordinal::new(0, 1).unwrap()
        }),
        vec![id(6)]
    );
}

#[test]
fn the_duplicate_detector_finds_exactly_the_repeated_names() {
    assert_eq!(first_duplicate(&[]), None);
    assert_eq!(first_duplicate(&[id(1), id(2), id(3)]), None);
    assert_eq!(
        first_duplicate(&[id(5), id(2), id(5), id(2), id(9)]),
        Some(id(2))
    );
    assert_eq!(first_duplicate(&[id(9), id(9)]), Some(id(9)));
    if cfg!(debug_assertions) {
        assert_eq!(
            expect_unique(&[id(3), id(1), id(3)]),
            Err(DuplicateName(id(3)))
        );
    } else {
        assert_eq!(expect_unique(&[id(3), id(1), id(3)]), Ok(()));
    }
    assert_eq!(expect_unique(&[id(3), id(1)]), Ok(()));
    assert_ne!(DuplicateName(id(1)).to_string(), "");
}
