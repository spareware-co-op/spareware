//! Chaining the edges of a blend into tangent chains (G262, chapter 13.10, "Feasibility first",
//! step 1): tangent joins link, sharp ones end a chain, an undecided one is refused, and the answer
//! does not depend on the order the edges were given in.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_blend::chain::{
    Chain, ChainError, EdgeId, EdgeRow, Join, Link, Tangency, VertexId, chains, straight_tangency,
};
use geom_num::Steps;

fn edge(id: u32, start: u32, end: u32) -> EdgeRow {
    EdgeRow {
        id: EdgeId(id),
        start: VertexId(start),
        end: VertexId(end),
    }
}

fn join(at: u32, first: u32, second: u32, tangency: Tangency) -> Join {
    Join {
        at: VertexId(at),
        first: EdgeId(first),
        second: EdgeId(second),
        tangency,
    }
}

fn link(edge: u32, reversed: bool) -> Link {
    Link {
        edge: EdgeId(edge),
        reversed,
    }
}

#[test]
fn tangent_joins_link_edges_into_one_chain_whichever_way_each_is_stored() {
    // Edges 1, 2, 3 run 10 - 11 - 12 - 13, but edge 2 is stored backwards.
    let edges = [edge(1, 10, 11), edge(2, 12, 11), edge(3, 12, 13)];
    let joins = [
        join(11, 1, 2, Tangency::Tangent),
        join(12, 2, 3, Tangency::Tangent),
    ];
    let found = chains(&edges, &joins).unwrap();
    assert_eq!(
        found,
        [Chain {
            links: vec![link(1, false), link(2, true), link(3, false)],
            closed: false
        }]
    );
}

#[test]
fn a_ring_of_tangent_edges_is_one_closed_chain_from_its_lowest_edge() {
    let edges = [edge(4, 3, 0), edge(1, 0, 1), edge(2, 1, 2), edge(3, 2, 3)];
    let joins = [
        join(0, 4, 1, Tangency::Tangent),
        join(1, 1, 2, Tangency::Tangent),
        join(2, 2, 3, Tangency::Tangent),
        join(3, 3, 4, Tangency::Tangent),
    ];
    let found = chains(&edges, &joins).unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].closed);
    assert_eq!(
        found[0].links,
        [
            link(1, false),
            link(2, false),
            link(3, false),
            link(4, false)
        ]
    );
}

#[test]
fn a_sharp_join_ends_the_chain_and_a_vertex_of_three_edges_is_a_corner() {
    // 1 and 2 meet sharply at 11; 2, 3 and 4 meet at 12, where no tangent join chains them.
    let edges = [
        edge(1, 10, 11),
        edge(2, 11, 12),
        edge(3, 12, 13),
        edge(4, 12, 14),
    ];
    let joins = [
        join(11, 1, 2, Tangency::Sharp),
        join(12, 2, 3, Tangency::Tangent),
    ];
    let found = chains(&edges, &joins).unwrap();
    let firsts: Vec<Vec<Link>> = found.iter().map(|c| c.links.clone()).collect();
    assert_eq!(
        firsts,
        [
            vec![link(1, false)],
            vec![link(2, false)],
            vec![link(3, false)],
            vec![link(4, false)]
        ]
    );
    assert!(found.iter().all(|c| !c.closed));
}

#[test]
fn every_edge_is_in_exactly_one_chain() {
    let edges = [
        edge(7, 0, 1),
        edge(5, 1, 2),
        edge(6, 2, 3),
        edge(9, 5, 6),
        edge(8, 6, 7),
    ];
    let joins = [
        join(1, 7, 5, Tangency::Tangent),
        join(2, 5, 6, Tangency::Tangent),
        join(6, 9, 8, Tangency::Tangent),
    ];
    let found = chains(&edges, &joins).unwrap();
    let mut seen: Vec<u32> = found
        .iter()
        .flat_map(|c| c.links.iter().map(|l| l.edge.0))
        .collect();
    seen.sort_unstable();
    assert_eq!(seen, [5, 6, 7, 8, 9]);
    assert_eq!(found.len(), 2);
}

#[test]
fn the_answer_does_not_depend_on_the_order_of_the_edges_or_the_joins() {
    let edges = [
        edge(1, 10, 11),
        edge(2, 12, 11),
        edge(3, 12, 13),
        edge(4, 20, 21),
    ];
    let joins = [
        join(11, 1, 2, Tangency::Tangent),
        join(12, 3, 2, Tangency::Tangent),
    ];
    let base = chains(&edges, &joins).unwrap();
    let mut reversed_edges = edges;
    reversed_edges.reverse();
    let mut reversed_joins = joins;
    reversed_joins.reverse();
    assert_eq!(chains(&reversed_edges, &reversed_joins).unwrap(), base);
    // And with a join named the other way round.
    let swapped = [
        join(11, 2, 1, Tangency::Tangent),
        join(12, 2, 3, Tangency::Tangent),
    ];
    assert_eq!(chains(&edges, &swapped).unwrap(), base);
}

#[test]
fn an_undecided_join_is_refused_with_its_band_and_never_treated_as_smooth() {
    let edges = [edge(1, 10, 11), edge(2, 11, 12), edge(3, 12, 13)];
    let joins = [
        join(12, 2, 3, Tangency::Undecided(Steps(40))),
        join(11, 1, 2, Tangency::Undecided(Steps(7))),
    ];
    assert_eq!(
        chains(&edges, &joins),
        Err(ChainError::IllConditioned {
            vertex: VertexId(11),
            band: Steps(7)
        })
    );
}

#[test]
fn joins_and_edges_that_do_not_match_are_refused() {
    let edges = [edge(1, 10, 11), edge(2, 11, 12)];
    // An edge that was not given.
    assert_eq!(
        chains(&edges, &[join(11, 1, 9, Tangency::Tangent)]),
        Err(ChainError::UnknownJoin {
            vertex: VertexId(11)
        })
    );
    // A vertex neither edge has.
    assert_eq!(
        chains(&edges, &[join(99, 1, 2, Tangency::Tangent)]),
        Err(ChainError::UnknownJoin {
            vertex: VertexId(99)
        })
    );
    // An edge given twice.
    assert_eq!(
        chains(&[edge(1, 10, 11), edge(1, 11, 12)], &[]),
        Err(ChainError::DuplicateEdge(EdgeId(1)))
    );
}

#[test]
fn an_edge_that_closes_on_itself_is_a_chain_of_its_own() {
    let edges = [edge(1, 5, 5), edge(2, 5, 6)];
    let found = chains(&edges, &[join(5, 1, 2, Tangency::Tangent)]).unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].links, [link(1, false)]);
    assert_eq!(found[1].links, [link(2, false)]);
}

#[test]
fn straight_edges_continue_each_other_only_in_the_same_direction() {
    let tangent = straight_tangency([1, 0, 0], [5, 0, 0]).unwrap();
    assert_eq!(tangent, Tangency::Tangent);
    // A hairpin, parallel and opposite, is sharp.
    assert_eq!(
        straight_tangency([1, 0, 0], [-3, 0, 0]).unwrap(),
        Tangency::Sharp
    );
    // A turn, however slight, is sharp: the cross product is exact.
    let slight = straight_tangency([1_000_000_000_000, 0, 0], [1_000_000_000_000, 1, 0]).unwrap();
    assert_eq!(slight, Tangency::Sharp);
    assert_eq!(
        straight_tangency([2, 4, 6], [1, 2, 3]).unwrap(),
        Tangency::Tangent
    );
    // Sizes past the budget are refused, not answered.
    assert!(straight_tangency([i128::MAX, 0, 0], [i128::MAX, 1, 0]).is_err());
}
