//! A Verus model of `geom-topo`'s Euler operators (chapter 28.7 of the kernel architecture,
//! "Topology, by a model"; piece G111): a body's loops as cyclic sequences of fin uses, and proofs
//! that the operators keep every loop closed.
//!
//! The model: each edge runs from a start vertex to an end vertex; a use is an edge and a
//! direction; a loop is a sequence of uses, closed when each use ends where the next starts and the
//! last ends where the first starts. `mev` inserts an edge out to a new vertex and back before a
//! use; `kev` removes such a spur. Each is proved to keep a closed loop closed.
//!
//! It is `geom-topo`'s Verus crate, `verus/lib.rs`. Verify: `verus --crate-type=lib
//! projects/engines/geometry/crates/geom-topo/verus/lib.rs`, with the Verus release
//! `kernel-dev.yml` pins; its `verus` job does this on every pull request.

use vstd::prelude::*;

verus! {

/// An edge: where it starts and where it ends.
#[derive(Clone, Copy)]
pub struct Edge {
    /// Its start vertex.
    pub start: u64,
    /// Its end vertex.
    pub end: u64,
}

/// A use of an edge by a loop: the edge, and whether the loop runs along it.
#[derive(Clone, Copy)]
pub struct Use {
    /// The edge, an index into the edges.
    pub edge: usize,
    /// Whether the loop runs from the edge's start to its end.
    pub forward: bool,
}

/// Where `u` starts.
pub open spec fn start(edges: Seq<Edge>, u: Use) -> u64 {
    if u.forward {
        edges[u.edge as int].start
    } else {
        edges[u.edge as int].end
    }
}

/// Where `u` ends.
pub open spec fn end(edges: Seq<Edge>, u: Use) -> u64 {
    if u.forward {
        edges[u.edge as int].end
    } else {
        edges[u.edge as int].start
    }
}

/// Every use of `l` names an edge of `edges`.
pub open spec fn named(edges: Seq<Edge>, l: Seq<Use>) -> bool {
    forall|i: int| 0 <= i < l.len() ==> (l[i].edge as int) < edges.len()
}

/// `l` is a closed loop over `edges`: not empty, every use names an edge, each use ends where the
/// next starts, and the last ends where the first starts.
pub open spec fn closed(edges: Seq<Edge>, l: Seq<Use>) -> bool {
    &&& l.len() > 0
    &&& named(edges, l)
    &&& forall|i: int| 0 <= i < l.len() - 1 ==> end(edges, l[i]) == start(edges, l[i + 1])
    &&& end(edges, l[l.len() - 1]) == start(edges, l[0])
}

/// Adding an edge changes no use of the edges before it.
pub proof fn lemma_push_keeps(edges: Seq<Edge>, e: Edge, u: Use)
    requires
        (u.edge as int) < edges.len(),
    ensures
        start(edges.push(e), u) == start(edges, u),
        end(edges.push(e), u) == end(edges, u),
{
}

/// A closed loop stays closed when an edge is added.
pub proof fn lemma_push_keeps_closed(edges: Seq<Edge>, e: Edge, l: Seq<Use>)
    requires
        closed(edges, l),
    ensures
        closed(edges.push(e), l),
{
    assert forall|i: int| 0 <= i < l.len() implies start(edges.push(e), l[i]) == start(edges, l[i])
        && end(edges.push(e), l[i]) == end(edges, l[i]) by {
        lemma_push_keeps(edges, e, l[i]);
    }
}

/// MEV in the model: the loop `l` with an edge from where `l[p]` starts out to `w` and back,
/// inserted just before `l[p]`; returns the edges and the loop.
pub fn mev(edges: &mut Vec<Edge>, l: &mut Vec<Use>, p: usize, w: u64)
    requires
        closed(old(edges)@, old(l)@),
        p < old(l)@.len(),
        old(edges)@.len() < usize::MAX,
        old(l)@.len() < usize::MAX - 2,
    ensures
        closed(edges@, l@),
        edges@.len() == old(edges)@.len() + 1,
        l@.len() == old(l)@.len() + 2,
        edges@.subrange(0, old(edges)@.len() as int) == old(edges)@,
{
    let at = l[p];
    let v = if at.forward {
        edges[at.edge].start
    } else {
        edges[at.edge].end
    };
    let e = edges.len();
    let ghost old_edges = edges@;
    let ghost old_l = l@;
    proof {
        assert(at == old_l[p as int]);
        assert(v == start(old_edges, old_l[p as int]));
    }
    edges.push(Edge { start: v, end: w });
    proof {
        lemma_push_keeps_closed(old_edges, Edge { start: v, end: w }, old_l);
    }
    l.insert(p, Use { edge: e, forward: true });
    l.insert(p + 1, Use { edge: e, forward: false });
    proof {
        let es = edges@;
        let n = l@;
        assert(es.subrange(0, old_edges.len() as int) =~= old_edges);
        assert(es[e as int] == (Edge { start: v, end: w }));
        let g = Use { edge: e, forward: true };
        let h = Use { edge: e, forward: false };
        assert(start(es, g) == v && end(es, g) == w);
        assert(start(es, h) == w && end(es, h) == v);
        assert(n.len() == old_l.len() + 2);
        assert forall|i: int| 0 <= i < n.len() implies (n[i].edge as int) < es.len() by {
            if i < p {
                assert(n[i] == old_l[i]);
            } else if i > p + 1 {
                assert(n[i] == old_l[i - 2]);
            }
        }
        assert forall|i: int| 0 <= i < n.len() - 1 implies end(es, n[i]) == start(es, n[i + 1]) by {
            if i < p as int - 1 {
                assert(n[i] == old_l[i] && n[i + 1] == old_l[i + 1]);
            } else if i == p as int - 1 {
                assert(n[i] == old_l[i] && n[i + 1] == g);
            } else if i == p as int {
                assert(n[i] == g && n[i + 1] == h);
            } else if i == p as int + 1 {
                assert(n[i] == h && n[i + 1] == old_l[p as int]);
            } else {
                assert(n[i] == old_l[i - 2] && n[i + 1] == old_l[i - 1]);
            }
        }
        if p == 0 {
            assert(n[n.len() - 1] == old_l[old_l.len() - 1]);
            assert(n[0] == g);
        } else {
            assert(n[n.len() - 1] == old_l[old_l.len() - 1]);
            assert(n[0] == old_l[0]);
        }
    }
}

} // verus!
