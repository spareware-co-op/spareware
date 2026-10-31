//! A Verus model of `geom-topo`'s Euler operators (chapter 28.7 of the kernel architecture,
//! "Topology, by a model"; piece G111): a body's loops as cyclic sequences of fin uses, and proofs
//! that the operators keep every loop closed.
//!
//! The model: each edge runs from a start vertex to an end vertex; a use is an edge and a
//! direction; a loop is a sequence of uses, closed when each use ends where the next starts and the
//! last ends where the first starts. `mev` inserts an edge out to a new vertex and back before a
//! use; `kev` removes such a spur; `mef` splits a loop in two by a new edge between two of its
//! vertices. Each is proved to keep closed loops closed; turning a loop to start anywhere
//! (`rot`, `rotate`) keeps it closed too.
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

/// `l` turned to start at `i`.
pub open spec fn rot(l: Seq<Use>, i: int) -> Seq<Use> {
    l.subrange(i, l.len() as int) + l.subrange(0, i)
}

/// A closed loop turned to start anywhere is closed.
pub proof fn lemma_rot_closed(edges: Seq<Edge>, l: Seq<Use>, i: int)
    requires
        closed(edges, l),
        0 <= i < l.len(),
    ensures
        closed(edges, rot(l, i)),
        rot(l, i).len() == l.len(),
        rot(l, i)[0] == l[i],
{
    let n = l.len() as int;
    let r = rot(l, i);
    assert(r.len() == n);
    assert forall|t: int| 0 <= t < n implies r[t] == (if t < n - i { l[i + t] } else { l[t - (n - i)] }) by {
    }
    assert forall|t: int| 0 <= t < n implies (r[t].edge as int) < edges.len() by {
    }
    assert forall|t: int| 0 <= t < n - 1 implies end(edges, r[t]) == start(edges, r[t + 1]) by {
        if t < n - i - 1 {
            assert(r[t] == l[i + t] && r[t + 1] == l[i + t + 1]);
        } else if t == n - i - 1 {
            assert(r[t] == l[n - 1] && r[t + 1] == l[0]);
        } else {
            assert(r[t] == l[t - (n - i)] && r[t + 1] == l[t + 1 - (n - i)]);
        }
    }
    if i == 0 {
        assert(r[n - 1] == l[n - 1] && r[0] == l[0]);
    } else {
        assert(r[n - 1] == l[i - 1] && r[0] == l[i]);
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

/// KEV in the model: the loop `l` without the spur at `p`: `l[p]` and `l[p + 1]`, one edge out
/// and back.
pub fn kev(edges: &Vec<Edge>, l: &mut Vec<Use>, p: usize)
    requires
        closed(edges@, old(l)@),
        old(l)@.len() >= 3,
        p + 1 < old(l)@.len(),
        old(l)@[p as int].edge == old(l)@[p + 1].edge,
        old(l)@[p as int].forward != old(l)@[p + 1].forward,
    ensures
        closed(edges@, l@),
        l@.len() == old(l)@.len() - 2,
{
    let ghost o = l@;
    l.remove(p);
    l.remove(p);
    proof {
        let es = edges@;
        let n = l@;
        let k = o.len();
        // The spur goes out and back: it starts where it ends.
        assert(start(es, o[p as int]) == end(es, o[p + 1]));
        assert(n.len() == k - 2);
        assert forall|i: int| 0 <= i < n.len() implies (n[i].edge as int) < es.len() by {
            if i < p {
                assert(n[i] == o[i]);
            } else {
                assert(n[i] == o[i + 2]);
            }
        }
        assert forall|i: int| 0 <= i < n.len() - 1 implies end(es, n[i]) == start(es, n[i + 1]) by {
            if i < p as int - 1 {
                assert(n[i] == o[i] && n[i + 1] == o[i + 1]);
            } else if i == p as int - 1 {
                assert(n[i] == o[i] && n[i + 1] == o[i + 3]);
                assert(end(es, o[i]) == start(es, o[p as int]));
                assert(end(es, o[p + 1]) == start(es, o[p + 2]));
            } else {
                assert(n[i] == o[i + 2] && n[i + 1] == o[i + 3]);
            }
        }
        if p == 0 {
            assert(n[0] == o[2]);
            assert(n[n.len() - 1] == o[k - 1]);
            assert(end(es, o[1]) == start(es, o[2]));
        } else if p + 2 == k {
            assert(n[0] == o[0]);
            assert(n[n.len() - 1] == o[p - 1]);
            assert(end(es, o[p - 1]) == start(es, o[p as int]));
        } else {
            assert(n[0] == o[0]);
            assert(n[n.len() - 1] == o[k - 1]);
        }
    }
}

/// `l` turned to start at `i`, as a new vector.
pub fn rotate(l: &Vec<Use>, i: usize) -> (r: Vec<Use>)
    requires
        i < l@.len(),
    ensures
        r@ == rot(l@, i as int),
{
    let mut r: Vec<Use> = Vec::new();
    let mut t = i;
    while t < l.len()
        invariant
            i <= t <= l@.len(),
            i < l@.len(),
            r@ == l@.subrange(i as int, t as int),
        decreases l@.len() - t,
    {
        r.push(l[t]);
        proof {
            assert(l@.subrange(i as int, t + 1) =~= l@.subrange(i as int, t as int).push(l@[t as int]));
        }
        t = t + 1;
    }
    let mut u: usize = 0;
    while u < i
        invariant
            u <= i,
            i < l@.len(),
            r@ == l@.subrange(i as int, l@.len() as int) + l@.subrange(0, u as int),
        decreases i - u,
    {
        r.push(l[u]);
        proof {
            assert(l@.subrange(i as int, l@.len() as int) + l@.subrange(0, u + 1) =~= (l@.subrange(
                i as int,
                l@.len() as int,
            ) + l@.subrange(0, u as int)).push(l@[u as int]));
        }
        u = u + 1;
    }
    r
}

/// `l[from..to]`, as a new vector.
pub fn slice(l: &Vec<Use>, from: usize, to: usize) -> (r: Vec<Use>)
    requires
        from <= to <= l@.len(),
    ensures
        r@ == l@.subrange(from as int, to as int),
{
    let mut r: Vec<Use> = Vec::new();
    let mut t = from;
    while t < to
        invariant
            from <= t <= to,
            to <= l@.len(),
            r@ == l@.subrange(from as int, t as int),
        decreases to - t,
    {
        r.push(l[t]);
        proof {
            assert(l@.subrange(from as int, t + 1) =~= l@.subrange(from as int, t as int).push(l@[t as int]));
        }
        t = t + 1;
    }
    r
}

/// MEF in the model: an edge from where `l[i]` starts to where `l[j]` starts, splitting `l` in
/// two: the uses from `l[i]` up to `l[j]` (the whole loop when `j` is `i`), closed by the edge
/// back, and the rest, closed by the edge along. Returns the rest, then the part split off.
pub fn mef(edges: &mut Vec<Edge>, l: &Vec<Use>, i: usize, j: usize) -> (out: (Vec<Use>, Vec<Use>))
    requires
        closed(old(edges)@, l@),
        i < l@.len(),
        j < l@.len(),
        l@.len() < usize::MAX / 2,
        old(edges)@.len() < usize::MAX,
    ensures
        closed(edges@, out.0@),
        closed(edges@, out.1@),
        out.0@.len() + out.1@.len() == l@.len() + 2,
        edges@.len() == old(edges)@.len() + 1,
        edges@.subrange(0, old(edges)@.len() as int) == old(edges)@,
{
    let n = l.len();
    let ui = l[i];
    let uj = l[j];
    let v1 = if ui.forward { edges[ui.edge].start } else { edges[ui.edge].end };
    let v2 = if uj.forward { edges[uj.edge].start } else { edges[uj.edge].end };
    let e = edges.len();
    let ghost old_edges = edges@;
    edges.push(Edge { start: v1, end: v2 });
    let r = rotate(l, i);
    let k: usize = if j > i {
        j - i
    } else if j == i {
        n
    } else {
        j + n - i
    };
    let mut moved = slice(&r, 0, k);
    let mut kept = slice(&r, k, n);
    let g = Use { edge: e, forward: true };
    let h = Use { edge: e, forward: false };
    moved.push(h);
    kept.push(g);
    proof {
        let es = edges@;
        let rs = r@;
        lemma_rot_closed(old_edges, l@, i as int);
        lemma_push_keeps_closed(old_edges, Edge { start: v1, end: v2 }, rs);
        assert(es.subrange(0, old_edges.len() as int) =~= old_edges);
        assert(start(es, g) == v1 && end(es, g) == v2);
        assert(start(es, h) == v2 && end(es, h) == v1);
        assert(rs[0] == l@[i as int]);
        assert(v1 == start(old_edges, l@[i as int]));
        assert(v2 == start(old_edges, l@[j as int]));
        assert(start(es, rs[0]) == v1);
        if k < n {
            assert(rs[k as int] == l@[j as int]);
            assert(start(es, rs[k as int]) == v2);
        } else {
            assert(i == j && v1 == v2);
        }
        let m = moved@;
        let p = kept@;
        assert(m =~= rs.subrange(0, k as int).push(h));
        assert(p =~= rs.subrange(k as int, n as int).push(g));
        assert forall|t: int| 0 <= t < m.len() implies (m[t].edge as int) < es.len() by {
            if t < k {
                assert(m[t] == rs[t]);
            }
        }
        assert forall|t: int| 0 <= t < m.len() - 1 implies end(es, m[t]) == start(es, m[t + 1]) by {
            if t < k - 1 {
                assert(m[t] == rs[t] && m[t + 1] == rs[t + 1]);
            } else {
                assert(m[t] == rs[k - 1] && m[t + 1] == h);
                if k < n {
                    assert(end(es, rs[k - 1]) == start(es, rs[k as int]));
                } else {
                    assert(end(es, rs[n - 1]) == start(es, rs[0]));
                }
            }
        }
        assert(m[m.len() - 1] == h && m[0] == rs[0]);
        assert forall|t: int| 0 <= t < p.len() implies (p[t].edge as int) < es.len() by {
            if t < n - k {
                assert(p[t] == rs[k + t]);
            }
        }
        assert forall|t: int| 0 <= t < p.len() - 1 implies end(es, p[t]) == start(es, p[t + 1]) by {
            if t < n - k - 1 {
                assert(p[t] == rs[k + t] && p[t + 1] == rs[k + t + 1]);
            } else {
                assert(p[t] == rs[n - 1] && p[t + 1] == g);
                assert(end(es, rs[n - 1]) == start(es, rs[0]));
            }
        }
        assert(p[p.len() - 1] == g);
        if k < n {
            assert(p[0] == rs[k as int]);
        } else {
            assert(p[0] == g);
        }
    }
    (kept, moved)
}

} // verus!
