//! A Verus model of `geom-topo`'s arenas and Euler operators (chapter 28.7 of the kernel
//! architecture, "Topology, by a model"; piece G111), proving that every operator keeps every
//! fin's partner, every loop and Euler's formula.
//!
//! The model: a [`Body`] holds its live edges, each with exactly two fins, one each way, as
//! `builder/euler.rs` makes them; its loops, as [`Rings`] of fins by `next`; its vertices, as rings
//! too; which face each loop bounds and which shell each face is in; and a count of holes. A
//! vertex's ring, its fan, runs through the fins that start there: the fin after `x` in its fan is
//! the fin after `x`'s partner in its loop. So the two ring sets are one combinatorial map, a
//! vertex is one cycle of it, and "where a fin starts" needs no row of its own: it is the fin's fan.
//!
//! What is proved: from the empty body ([`Body::lemma_empty`]), each operator with the arguments
//! the code accepts gives a body for which [`Body::wf`] holds, so for all of them, however many:
//!
//! - **Partners.** Every fin of every live edge is in exactly one place of exactly one loop and of
//!   one fan, and no other fin is; a fin's partner is the other fin of its edge.
//! - **Loops.** `next` is a permutation whose cycles are the loops, `prev` its inverse, and each
//!   fin ends where the next starts ([`Body::lemma_closed`]).
//! - **Euler's formula.** V − E + F − R = 2 (S − H), where V, E, F and S count the vertices, edges,
//!   faces and shells the body really holds, R is loops less faces, and H is the count of holes,
//!   which [`Body::mfkrh`] lowers and [`Body::kfmrh`] raises by one.
//!
//! The fans do more than count. `kev` and `kill_wire` remove vertices without looking at the rest
//! of the body, and the code checks nothing else because nothing else can use them: a fin that
//! follows its partner in a loop is its own next in its fan, so it is its vertex's only fin
//! ([`Rings::lemma_alone`]). A closed edge cannot be a lone wire, for the same reason.
//!
//! What is left out: regions ([`Euler::make_region`], `kill_region`, `move_shell`), which touch no
//! count, and geometry and names, which no operator reads. A body must start from these operators:
//! one made by hand and opened with `Euler::edit` is the checker's business (8.3).
//!
//! The tie to the code: `tests/euler_model.rs` drives `Euler` and a plain model of the same lists
//! in lockstep for 100,000 random steps, requiring them to agree at every step, and checks there
//! that `wf`'s facts hold of the real arenas, fans included.
//!
//! It is `geom-topo`'s Verus crate, `verus/lib.rs`. Verify: `verus --crate-type=lib
//! projects/engines/geometry/crates/geom-topo/verus/lib.rs`, with the Verus release
//! `kernel-dev.yml` pins; its `verus` job does this on every pull request.

use crate::rings::{Fin, Rings};
use vstd::prelude::*;

verus! {

/// The other fin of `x`'s edge.
pub open spec fn partner(x: Fin) -> Fin {
    Fin { edge: x.edge, forward: !x.forward }
}

/// The fin along edge `e`.
pub open spec fn along(e: nat) -> Fin {
    Fin { edge: e, forward: true }
}

/// The fin back along edge `e`.
pub open spec fn back(e: nat) -> Fin {
    Fin { edge: e, forward: false }
}

/// A body's topology.
pub ghost struct Body {
    /// The live edges.
    pub edges: Set<nat>,
    /// The loops, by key; `succ` is `next`.
    pub loops: Rings,
    /// The vertices' fans, by vertex.
    pub fans: Rings,
    /// The face each loop bounds.
    pub face: Map<nat, nat>,
    /// The shell each face is in.
    pub shell: Map<nat, nat>,
    /// The live shells.
    pub shells: Set<nat>,
    /// The holes.
    pub holes: int,
}

impl Body {
    /// Euler's formula, V − E + F − R = 2 (S − H), with R the loops less the faces.
    pub open spec fn euler(self) -> bool {
        self.fans.rings.dom().len() - self.edges.len() + self.shell.dom().len() - (
        self.loops.rings.dom().len() - self.shell.dom().len()) == 2 * (self.shells.len()
            - self.holes)
    }

    /// Everything the operators keep (see the module header).
    pub open spec fn wf(self) -> bool {
        &&& self.loops.wf()
        &&& self.fans.wf()
        &&& forall|x: Fin|
            #![trigger self.loops.place.dom().contains(x)]
            self.loops.place.dom().contains(x) <==> self.edges.contains(x.edge)
        &&& forall|x: Fin|
            #![trigger self.fans.place.dom().contains(x)]
            self.fans.place.dom().contains(x) <==> self.edges.contains(x.edge)
        &&& forall|x: Fin|
            #![trigger self.fans.succ[x]]
            self.edges.contains(x.edge) ==> self.fans.succ[x] == self.loops.succ[partner(x)]
        &&& self.face.dom() == self.loops.rings.dom()
        &&& forall|k: nat| #[trigger]
            self.face.dom().contains(k) ==> self.shell.dom().contains(self.face[k])
        &&& forall|f: nat| #[trigger]
            self.shell.dom().contains(f) ==> self.shells.contains(self.shell[f])
        &&& self.euler()
    }

    /// The body with nothing in it.
    pub open spec fn empty() -> Body {
        Body {
            edges: Set::empty(),
            loops: Rings::empty(),
            fans: Rings::empty(),
            face: Map::empty(),
            shell: Map::empty(),
            shells: Set::empty(),
            holes: 0,
        }
    }

    /// The empty body keeps every rule.
    pub proof fn lemma_empty()
        ensures
            Body::empty().wf(),
    {
        Rings::lemma_empty();
        let b = Body::empty();
        assert(b.face.dom() == b.loops.rings.dom());
    }

    /// Each fin ends where the next starts: the next fin is in the fan of the fin's partner.
    pub proof fn lemma_closed(self, x: Fin)
        requires
            self.wf(),
            self.edges.contains(x.edge),
        ensures
            self.fans.place[self.loops.succ[x]].0 == self.fans.place[partner(x)].0,
    {
        let y = partner(x);
        assert(partner(y) == x);
        assert(self.edges.contains(y.edge));
        assert(self.fans.place.dom().contains(y));
        assert(self.fans.succ[y] == self.loops.succ[x]);
        self.fans.lemma_succ(y);
    }

    /// The fin before `at` in its loop, with its partner, whose fan `succ` is `at`.
    pub proof fn lemma_before(self, at: Fin)
        requires
            self.wf(),
            self.edges.contains(at.edge),
        ensures
            self.loops.place.dom().contains(self.loops.pred(at)),
            self.loops.succ[self.loops.pred(at)] == at,
            self.loops.place[self.loops.pred(at)].0 == self.loops.place[at].0,
            self.edges.contains(self.loops.pred(at).edge),
            self.fans.place.dom().contains(partner(self.loops.pred(at))),
            self.fans.succ[partner(self.loops.pred(at))] == at,
    {
        assert(self.loops.place.dom().contains(at));
        self.loops.lemma_pred(at);
        let p = self.loops.pred(at);
        let q = partner(p);
        assert(partner(q) == p);
        assert(self.edges.contains(q.edge));
        assert(self.fans.place.dom().contains(q));
        assert(self.fans.succ[q] == self.loops.succ[p]);
    }

    /// MAKE_WIRE: a new shell of one face, whose one loop runs along one new edge between two new
    /// vertices and back.
    pub open spec fn make_wire(self, e: nat, k: nat, f: nat, s: nat, v0: nat, v1: nat) -> Body {
        let (g, h) = (along(e), back(e));
        Body {
            edges: self.edges.insert(e),
            loops: self.loops.fresh(k, g).ins(g, h),
            fans: self.fans.fresh(v0, g).fresh(v1, h),
            face: self.face.insert(k, f),
            shell: self.shell.insert(f, s),
            shells: self.shells.insert(s),
            holes: self.holes,
        }
    }

    /// `make_wire` keeps every rule.
    pub proof fn lemma_make_wire(self, e: nat, k: nat, f: nat, s: nat, v0: nat, v1: nat)
        requires
            self.wf(),
            !self.edges.contains(e),
            !self.loops.rings.dom().contains(k),
            !self.shell.dom().contains(f),
            !self.shells.contains(s),
            !self.fans.rings.dom().contains(v0),
            !self.fans.rings.dom().contains(v1),
            v0 != v1,
        ensures
            self.make_wire(e, k, f, s, v0, v1).wf(),
    {
        let (g, h) = (along(e), back(e));
        let b = self.make_wire(e, k, f, s, v0, v1);
        assert(!self.loops.place.dom().contains(g));
        assert(!self.loops.place.dom().contains(h));
        assert(!self.fans.place.dom().contains(g));
        assert(!self.fans.place.dom().contains(h));
        self.loops.lemma_fresh(k, g);
        let l1 = self.loops.fresh(k, g);
        assert(l1.place.dom().contains(g));
        assert(!l1.place.dom().contains(h));
        l1.lemma_ins(g, h);
        self.fans.lemma_fresh(v0, g);
        let f1 = self.fans.fresh(v0, g);
        assert(f1.rings.dom() == self.fans.rings.dom().insert(v0));
        assert(!f1.place.dom().contains(h));
        f1.lemma_fresh(v1, h);
        assert forall|x: Fin| #[trigger] b.loops.place.dom().contains(x) <==> b.edges.contains(x.edge) by {
            assert(self.loops.place.dom().contains(x) <==> self.edges.contains(x.edge));
            if x.edge == e {
                assert(x == g || x == h);
            }
        }
        assert forall|x: Fin| #[trigger] b.fans.place.dom().contains(x) <==> b.edges.contains(x.edge) by {
            assert(self.fans.place.dom().contains(x) <==> self.edges.contains(x.edge));
            if x.edge == e {
                assert(x == g || x == h);
            }
        }
        assert forall|x: Fin| #[trigger] b.edges.contains(x.edge) implies b.fans.succ[x]
            == b.loops.succ[partner(x)] by {
            assert(partner(g) == h);
            assert(partner(h) == g);
            if x.edge != e {
                assert(self.edges.contains(x.edge));
                assert(self.fans.succ[x] == self.loops.succ[partner(x)]);
                assert(partner(x).edge != e);
            } else {
                assert(x == g || x == h);
            }
        }
        assert(b.face.dom() =~= b.loops.rings.dom());
        assert forall|m: nat| #[trigger] b.face.dom().contains(m) implies b.shell.dom().contains(
            b.face[m],
        ) by {
            if m != k {
                assert(self.face.dom().contains(m));
            }
        }
        assert forall|m: nat| #[trigger] b.shell.dom().contains(m) implies b.shells.contains(
            b.shell[m],
        ) by {
            if m != f {
                assert(self.shell.dom().contains(m));
            }
        }
        assert(b.fans.rings.dom() == self.fans.rings.dom().insert(v0).insert(v1));
    }

    /// KILL_WIRE: removes the edge `e`, whose two fins are the whole of a loop that is its face's
    /// only loop, on a face that is its shell's only face, with the shell and the two vertices.
    pub open spec fn kill_wire(self, e: nat) -> Body {
        let (g, h) = (along(e), back(e));
        let k = self.loops.place[g].0;
        let f = self.face[k];
        Body {
            edges: self.edges.remove(e),
            loops: self.loops.del(h).drop_fin(g),
            fans: self.fans.drop_fin(g).drop_fin(h),
            face: self.face.remove(k),
            shell: self.shell.remove(f),
            shells: self.shells.remove(self.shell[f]),
            holes: self.holes,
        }
    }

    /// `kill_wire` keeps every rule.
    pub proof fn lemma_kill_wire(self, e: nat)
        requires
            self.wf(),
            self.edges.contains(e),
            self.loops.succ[along(e)] == back(e),
            self.loops.succ[back(e)] == along(e),
            forall|m: nat|
                #[trigger] self.face.dom().contains(m) && self.face[m]
                    == self.face[self.loops.place[along(e)].0] ==> m == self.loops.place[along(e)].0,
            forall|m: nat|
                #[trigger] self.shell.dom().contains(m) && self.shell[m]
                    == self.shell[self.face[self.loops.place[along(e)].0]] ==> m
                    == self.face[self.loops.place[along(e)].0],
        ensures
            self.kill_wire(e).wf(),
    {
        let (g, h) = (along(e), back(e));
        let k = self.loops.place[g].0;
        let f = self.face[k];
        let s = self.shell[f];
        let b = self.kill_wire(e);
        assert(partner(g) == h);
        assert(partner(h) == g);
        assert(self.loops.place.dom().contains(g));
        assert(self.loops.place.dom().contains(h));
        assert(self.fans.place.dom().contains(g));
        assert(self.fans.place.dom().contains(h));
        assert(self.fans.succ[g] == g);
        assert(self.fans.succ[h] == h);
        self.loops.lemma_del(h);
        let l1 = self.loops.del(h);
        assert(l1.place.dom().contains(g));
        assert(l1.succ[g] == g);
        self.loops.lemma_succ(g);
        self.loops.lemma_succ(h);
        l1.lemma_drop_fin(g);
        self.fans.lemma_drop_fin(g);
        self.fans.lemma_alone(g, h);
        let f1 = self.fans.drop_fin(g);
        assert(f1.place.dom().contains(h));
        assert(f1.succ[h] == h);
        f1.lemma_drop_fin(h);
        assert(l1.place[g].0 == k);
        assert(self.face.dom().contains(k));
        assert(self.shell.dom().contains(f));
        assert(self.shells.contains(s));
        assert forall|x: Fin| #[trigger] b.loops.place.dom().contains(x) <==> b.edges.contains(x.edge) by {
            assert(self.loops.place.dom().contains(x) <==> self.edges.contains(x.edge));
            if x.edge == e {
                assert(x == g || x == h);
            }
        }
        assert forall|x: Fin| #[trigger] b.fans.place.dom().contains(x) <==> b.edges.contains(x.edge) by {
            assert(self.fans.place.dom().contains(x) <==> self.edges.contains(x.edge));
            if x.edge == e {
                assert(x == g || x == h);
            }
        }
        assert forall|x: Fin| #[trigger] b.edges.contains(x.edge) implies b.fans.succ[x]
            == b.loops.succ[partner(x)] by {
            let y = partner(x);
            assert(self.edges.contains(x.edge));
            assert(self.edges.contains(y.edge));
            assert(x != g && x != h && y != g && y != h);
            assert(self.fans.succ[x] == self.loops.succ[y]);
            assert(self.fans.place.dom().contains(x));
            assert(self.loops.place.dom().contains(y));
            if self.loops.succ[y] == h {
                self.loops.lemma_succ_injective(y, g);
            }
        }
        assert(b.face.dom() =~= b.loops.rings.dom());
        assert forall|m: nat| #[trigger] b.face.dom().contains(m) implies b.shell.dom().contains(
            b.face[m],
        ) by {
            assert(self.face.dom().contains(m));
            assert(m != k);
        }
        assert forall|m: nat| #[trigger] b.shell.dom().contains(m) implies b.shells.contains(
            b.shell[m],
        ) by {
            assert(self.shell.dom().contains(m));
            assert(m != f);
        }
    }

    /// MEV: an edge `e` from the vertex `at` starts at to a new vertex `w`, and back, in `at`'s
    /// loop just before `at`.
    pub open spec fn mev(self, at: Fin, e: nat, w: nat) -> Body {
        let p = self.loops.pred(at);
        let (g, h) = (along(e), back(e));
        Body {
            edges: self.edges.insert(e),
            loops: self.loops.ins(p, g).ins(g, h),
            fans: self.fans.ins(partner(p), g).fresh(w, h),
            face: self.face,
            shell: self.shell,
            shells: self.shells,
            holes: self.holes,
        }
    }

    /// `mev` keeps every rule.
    pub proof fn lemma_mev(self, at: Fin, e: nat, w: nat)
        requires
            self.wf(),
            self.edges.contains(at.edge),
            !self.edges.contains(e),
            !self.fans.rings.dom().contains(w),
        ensures
            self.mev(at, e, w).wf(),
    {
        self.lemma_before(at);
        let p = self.loops.pred(at);
        let q = partner(p);
        let (g, h) = (along(e), back(e));
        let b = self.mev(at, e, w);
        assert(partner(q) == p);
        assert(partner(g) == h);
        assert(partner(h) == g);
        assert(!self.loops.place.dom().contains(g));
        assert(!self.loops.place.dom().contains(h));
        assert(!self.fans.place.dom().contains(g));
        assert(!self.fans.place.dom().contains(h));
        self.loops.lemma_ins(p, g);
        let l1 = self.loops.ins(p, g);
        l1.lemma_ins(g, h);
        self.fans.lemma_ins(q, g);
        let f1 = self.fans.ins(q, g);
        f1.lemma_fresh(w, h);
        assert forall|x: Fin| #[trigger] b.loops.place.dom().contains(x) <==> b.edges.contains(x.edge) by {
            assert(self.loops.place.dom().contains(x) <==> self.edges.contains(x.edge));
            if x.edge == e {
                assert(x == g || x == h);
            }
        }
        assert forall|x: Fin| #[trigger] b.fans.place.dom().contains(x) <==> b.edges.contains(x.edge) by {
            assert(self.fans.place.dom().contains(x) <==> self.edges.contains(x.edge));
            if x.edge == e {
                assert(x == g || x == h);
            }
        }
        assert forall|x: Fin| #[trigger] b.edges.contains(x.edge) implies b.fans.succ[x]
            == b.loops.succ[partner(x)] by {
            if x.edge != e {
                assert(self.edges.contains(x.edge));
                assert(self.fans.succ[x] == self.loops.succ[partner(x)]);
                assert(partner(x).edge != e);
            } else {
                assert(x == g || x == h);
            }
        }
        assert(b.fans.rings.dom() == self.fans.rings.dom().insert(w));
    }

    /// KEV: removes the edge of `x` and `partner(x)`, which follows `x` in its loop without `x`
    /// also following it, with the far vertex.
    pub open spec fn kev(self, x: Fin) -> Body {
        let y = partner(x);
        Body {
            edges: self.edges.remove(x.edge),
            loops: self.loops.del(y).del(x),
            fans: self.fans.drop_fin(y).del(x),
            face: self.face,
            shell: self.shell,
            shells: self.shells,
            holes: self.holes,
        }
    }

    /// `kev` keeps every rule.
    pub proof fn lemma_kev(self, x: Fin)
        requires
            self.wf(),
            self.edges.contains(x.edge),
            self.loops.succ[x] == partner(x),
            self.loops.succ[partner(x)] != x,
        ensures
            self.kev(x).wf(),
    {
        let y = partner(x);
        let e = x.edge;
        let b = self.kev(x);
        assert(partner(y) == x);
        assert(x != y);
        assert(self.loops.place.dom().contains(x));
        assert(self.loops.place.dom().contains(y));
        assert(self.fans.place.dom().contains(x));
        assert(self.fans.place.dom().contains(y));
        assert(self.fans.succ[y] == y);
        assert(self.fans.succ[x] == self.loops.succ[y]);
        if self.loops.succ[y] == y {
            self.loops.lemma_succ_injective(x, y);
        }
        self.loops.lemma_del(y);
        let l1 = self.loops.del(y);
        assert(l1.place.dom().contains(x));
        assert(l1.succ[x] == self.loops.succ[y]);
        l1.lemma_del(x);
        self.fans.lemma_drop_fin(y);
        let f1 = self.fans.drop_fin(y);
        assert(f1.place.dom().contains(x));
        assert(f1.succ[x] == self.fans.succ[x]);
        f1.lemma_del(x);
        assert forall|u: Fin| #[trigger] b.loops.place.dom().contains(u) <==> b.edges.contains(u.edge) by {
            assert(self.loops.place.dom().contains(u) <==> self.edges.contains(u.edge));
            if u.edge == e {
                assert(u == x || u == y);
            }
        }
        assert forall|u: Fin| #[trigger] b.fans.place.dom().contains(u) <==> b.edges.contains(u.edge) by {
            assert(self.fans.place.dom().contains(u) <==> self.edges.contains(u.edge));
            if u.edge == e {
                assert(u == x || u == y);
            }
        }
        assert forall|u: Fin| #[trigger] b.edges.contains(u.edge) implies b.fans.succ[u]
            == b.loops.succ[partner(u)] by {
            let v = partner(u);
            assert(self.edges.contains(u.edge));
            assert(self.edges.contains(v.edge));
            assert(u != x && u != y && v != x && v != y);
            assert(self.loops.place.dom().contains(v));
            assert(self.fans.place.dom().contains(u));
            assert(self.fans.succ[u] == self.loops.succ[v]);
            if self.loops.succ[v] == y {
                self.loops.lemma_succ_injective(v, x);
            }
        }
        assert(b.fans.rings.dom() == self.fans.rings.dom().remove(self.fans.place[y].0));
    }

    /// MEF: an edge `e` from the vertex `from` starts at to the vertex `to` starts at, across
    /// their one loop, and a new face `f` on the same shell. A new loop `k2` holds the back fin
    /// and the fins from `from` up to `to` (the whole loop when `to` is `from`); the rest, with the
    /// fin along the edge, stay.
    pub open spec fn mef(self, from: Fin, to: Fin, e: nat, k2: nat, f: nat) -> Body {
        let p = self.loops.pred(from);
        let r = self.loops.pred(to);
        let (g, h) = (along(e), back(e));
        Body {
            edges: self.edges.insert(e),
            loops: self.loops.ins(p, g).ins(r, h).split_at(g, h, k2),
            fans: self.fans.ins(partner(p), g).ins(partner(r), h),
            face: self.face.insert(k2, f),
            shell: self.shell.insert(f, self.shell[self.face[self.loops.place[from].0]]),
            shells: self.shells,
            holes: self.holes,
        }
    }

    /// `mef` keeps every rule.
    pub proof fn lemma_mef(self, from: Fin, to: Fin, e: nat, k2: nat, f: nat)
        requires
            self.wf(),
            self.edges.contains(from.edge),
            self.edges.contains(to.edge),
            self.loops.place[from].0 == self.loops.place[to].0,
            !self.edges.contains(e),
            !self.loops.rings.dom().contains(k2),
            !self.shell.dom().contains(f),
        ensures
            self.mef(from, to, e, k2, f).wf(),
    {
        self.lemma_before(from);
        self.lemma_before(to);
        let p = self.loops.pred(from);
        let r = self.loops.pred(to);
        let (q1, q2) = (partner(p), partner(r));
        let (g, h) = (along(e), back(e));
        let k = self.loops.place[from].0;
        let b = self.mef(from, to, e, k2, f);
        assert(partner(q1) == p);
        assert(partner(q2) == r);
        assert(partner(g) == h);
        assert(partner(h) == g);
        assert(!self.loops.place.dom().contains(g));
        assert(!self.loops.place.dom().contains(h));
        assert(!self.fans.place.dom().contains(g));
        assert(!self.fans.place.dom().contains(h));
        if p == r {
            self.loops.lemma_succ_injective(p, r);
        } else {
            assert(from != to);
        }
        self.loops.lemma_ins(p, g);
        let l1 = self.loops.ins(p, g);
        assert(l1.place.dom().contains(r));
        assert(!l1.place.dom().contains(h));
        l1.lemma_ins(r, h);
        let l2 = l1.ins(r, h);
        assert(l2.place.dom().contains(g));
        assert(l2.place.dom().contains(h));
        assert(l2.place[g].0 == k);
        assert(l2.place[h].0 == k);
        assert(l2.rings.dom() == self.loops.rings.dom());
        l2.lemma_split_at(g, h, k2);
        self.fans.lemma_ins(q1, g);
        let f1 = self.fans.ins(q1, g);
        assert(f1.place.dom().contains(q2));
        assert(!f1.place.dom().contains(h));
        f1.lemma_ins(q2, h);
        assert forall|x: Fin| #[trigger] b.loops.place.dom().contains(x) <==> b.edges.contains(x.edge) by {
            assert(self.loops.place.dom().contains(x) <==> self.edges.contains(x.edge));
            if x.edge == e {
                assert(x == g || x == h);
            }
        }
        assert forall|x: Fin| #[trigger] b.fans.place.dom().contains(x) <==> b.edges.contains(x.edge) by {
            assert(self.fans.place.dom().contains(x) <==> self.edges.contains(x.edge));
            if x.edge == e {
                assert(x == g || x == h);
            }
        }
        assert forall|x: Fin| #[trigger] b.edges.contains(x.edge) implies b.fans.succ[x]
            == b.loops.succ[partner(x)] by {
            if x.edge != e {
                assert(self.edges.contains(x.edge));
                assert(self.fans.succ[x] == self.loops.succ[partner(x)]);
                assert(partner(x).edge != e);
            } else {
                assert(x == g || x == h);
            }
        }
        assert(self.loops.place.dom().contains(from));
        assert(self.loops.at(k, self.loops.place[from].1));
        assert(self.face.dom().contains(k));
        assert(self.shell.dom().contains(self.face[k]));
        assert(b.face.dom() =~= b.loops.rings.dom());
        assert forall|m: nat| #[trigger] b.face.dom().contains(m) implies b.shell.dom().contains(
            b.face[m],
        ) by {
            if m != k2 {
                assert(self.face.dom().contains(m));
            }
        }
        assert forall|m: nat| #[trigger] b.shell.dom().contains(m) implies b.shells.contains(
            b.shell[m],
        ) by {
            if m != f {
                assert(self.shell.dom().contains(m));
            }
        }
    }

    /// KEF: removes the edge of `gone` and the loop and face of `gone`, the only loop of that face;
    /// the loop's other fins join the loop of `gone`'s partner.
    pub open spec fn kef(self, gone: Fin) -> Body {
        let stay = partner(gone);
        let lg = self.loops.place[gone].0;
        Body {
            edges: self.edges.remove(gone.edge),
            loops: self.loops.join_at(stay, gone).del(gone).del(stay),
            fans: self.fans.del(gone).del(stay),
            face: self.face.remove(lg),
            shell: self.shell.remove(self.face[lg]),
            shells: self.shells,
            holes: self.holes,
        }
    }

    /// `kef` keeps every rule.
    pub proof fn lemma_kef(self, gone: Fin)
        requires
            self.wf(),
            self.edges.contains(gone.edge),
            self.face[self.loops.place[gone].0] != self.face[self.loops.place[partner(gone)].0],
            forall|m: nat|
                #[trigger] self.face.dom().contains(m) && self.face[m]
                    == self.face[self.loops.place[gone].0] ==> m == self.loops.place[gone].0,
            !(self.loops.succ[gone] == gone && self.loops.succ[partner(gone)] == partner(gone)),
        ensures
            self.kef(gone).wf(),
    {
        let stay = partner(gone);
        let e = gone.edge;
        let lg = self.loops.place[gone].0;
        let ls = self.loops.place[stay].0;
        let b = self.kef(gone);
        let s = self.loops.succ;
        let t = self.fans.succ;
        assert(partner(stay) == gone);
        assert(gone != stay);
        assert(lg != ls);
        assert(self.loops.place.dom().contains(gone));
        assert(self.loops.place.dom().contains(stay));
        assert(self.fans.place.dom().contains(gone));
        assert(self.fans.place.dom().contains(stay));
        assert(t[gone] == s[stay]);
        assert(t[stay] == s[gone]);
        self.loops.lemma_succ(gone);
        self.loops.lemma_succ(stay);
        // The fins of two loops are never each other's `succ`.
        assert(s[stay] != gone);
        assert(s[gone] != stay);
        self.loops.lemma_join_at(stay, gone);
        let l1 = self.loops.join_at(stay, gone);
        assert(l1.succ == s.insert(stay, s[gone]).insert(gone, s[stay]));
        assert(l1.succ[gone] == s[stay]);
        assert(l1.succ[stay] == s[gone]);
        assert(l1.place.dom().contains(gone));
        l1.lemma_del(gone);
        let l2 = l1.del(gone);
        assert(l2.place.dom().contains(stay));
        l2.lemma_del(stay);
        self.fans.lemma_del(gone);
        let f1 = self.fans.del(gone);
        assert(f1.place.dom().contains(stay));
        f1.lemma_del(stay);
        assert forall|u: Fin| #[trigger] b.loops.place.dom().contains(u) <==> b.edges.contains(u.edge) by {
            assert(self.loops.place.dom().contains(u) <==> self.edges.contains(u.edge));
            if u.edge == e {
                assert(u == gone || u == stay);
            }
        }
        assert forall|u: Fin| #[trigger] b.fans.place.dom().contains(u) <==> b.edges.contains(u.edge) by {
            assert(self.fans.place.dom().contains(u) <==> self.edges.contains(u.edge));
            if u.edge == e {
                assert(u == gone || u == stay);
            }
        }
        assert forall|u: Fin| #[trigger] b.edges.contains(u.edge) implies b.fans.succ[u]
            == b.loops.succ[partner(u)] by {
            let v = partner(u);
            assert(self.edges.contains(u.edge));
            assert(self.edges.contains(v.edge));
            assert(u != gone && u != stay && v != gone && v != stay);
            assert(self.loops.place.dom().contains(v));
            assert(self.fans.place.dom().contains(u));
            assert(t[u] == s[v]);
            assert(l1.succ[v] == s[v]);
        }
        assert(self.loops.at(lg, self.loops.place[gone].1));
        assert(self.face.dom().contains(lg));
        assert(self.shell.dom().contains(self.face[lg]));
        assert(b.face.dom() =~= b.loops.rings.dom());
        assert forall|m: nat| #[trigger] b.face.dom().contains(m) implies b.shell.dom().contains(
            b.face[m],
        ) by {
            assert(self.face.dom().contains(m));
            assert(m != lg);
            assert(self.face[m] != self.face[lg]);
            assert(self.shell.dom().contains(self.face[m]));
        }
        assert forall|m: nat| #[trigger] b.shell.dom().contains(m) implies b.shells.contains(
            b.shell[m],
        ) by {
            assert(self.shell.dom().contains(m));
        }
    }

    /// MEKR: an edge `e` from the vertex `from` starts at to the vertex `to` starts at, joining
    /// their two loops of one face into `from`'s loop.
    pub open spec fn mekr(self, from: Fin, to: Fin, e: nat) -> Body {
        let p = self.loops.pred(from);
        let r = self.loops.pred(to);
        let (g, h) = (along(e), back(e));
        Body {
            edges: self.edges.insert(e),
            loops: self.loops.ins(p, g).ins(r, h).join_at(g, h),
            fans: self.fans.ins(partner(p), g).ins(partner(r), h),
            face: self.face.remove(self.loops.place[to].0),
            shell: self.shell,
            shells: self.shells,
            holes: self.holes,
        }
    }

    /// `mekr` keeps every rule.
    pub proof fn lemma_mekr(self, from: Fin, to: Fin, e: nat)
        requires
            self.wf(),
            self.edges.contains(from.edge),
            self.edges.contains(to.edge),
            self.loops.place[from].0 != self.loops.place[to].0,
            self.face[self.loops.place[from].0] == self.face[self.loops.place[to].0],
            !self.edges.contains(e),
        ensures
            self.mekr(from, to, e).wf(),
    {
        self.lemma_before(from);
        self.lemma_before(to);
        let p = self.loops.pred(from);
        let r = self.loops.pred(to);
        let (q1, q2) = (partner(p), partner(r));
        let (g, h) = (along(e), back(e));
        let k2 = self.loops.place[to].0;
        let b = self.mekr(from, to, e);
        assert(partner(q1) == p);
        assert(partner(q2) == r);
        assert(partner(g) == h);
        assert(partner(h) == g);
        assert(!self.loops.place.dom().contains(g));
        assert(!self.loops.place.dom().contains(h));
        assert(!self.fans.place.dom().contains(g));
        assert(!self.fans.place.dom().contains(h));
        assert(p != r);
        assert(q1 != q2);
        self.loops.lemma_ins(p, g);
        let l1 = self.loops.ins(p, g);
        assert(l1.place.dom().contains(r));
        assert(!l1.place.dom().contains(h));
        l1.lemma_ins(r, h);
        let l2 = l1.ins(r, h);
        assert(l2.place.dom().contains(g));
        assert(l2.place.dom().contains(h));
        assert(l2.place[g].0 == self.loops.place[from].0);
        assert(l2.place[h].0 == k2);
        assert(l2.rings.dom() == self.loops.rings.dom());
        l2.lemma_join_at(g, h);
        self.fans.lemma_ins(q1, g);
        let f1 = self.fans.ins(q1, g);
        assert(f1.place.dom().contains(q2));
        assert(!f1.place.dom().contains(h));
        f1.lemma_ins(q2, h);
        assert forall|x: Fin| #[trigger] b.loops.place.dom().contains(x) <==> b.edges.contains(x.edge) by {
            assert(self.loops.place.dom().contains(x) <==> self.edges.contains(x.edge));
            if x.edge == e {
                assert(x == g || x == h);
            }
        }
        assert forall|x: Fin| #[trigger] b.fans.place.dom().contains(x) <==> b.edges.contains(x.edge) by {
            assert(self.fans.place.dom().contains(x) <==> self.edges.contains(x.edge));
            if x.edge == e {
                assert(x == g || x == h);
            }
        }
        assert forall|x: Fin| #[trigger] b.edges.contains(x.edge) implies b.fans.succ[x]
            == b.loops.succ[partner(x)] by {
            if x.edge != e {
                assert(self.edges.contains(x.edge));
                assert(self.fans.succ[x] == self.loops.succ[partner(x)]);
                assert(partner(x).edge != e);
            } else {
                assert(x == g || x == h);
            }
        }
        assert(self.loops.place.dom().contains(to));
        assert(self.loops.at(k2, self.loops.place[to].1));
        assert(b.face.dom() =~= b.loops.rings.dom());
        assert forall|m: nat| #[trigger] b.face.dom().contains(m) implies b.shell.dom().contains(
            b.face[m],
        ) by {
            assert(self.face.dom().contains(m));
        }
    }

    /// KEMR: removes the edge of `x`, whose two fins are in one loop and not next to each other.
    /// The fins after `x` up to its partner become a new loop `ring` of the same face.
    pub open spec fn kemr(self, x: Fin, ring: nat) -> Body {
        let y = partner(x);
        Body {
            edges: self.edges.remove(x.edge),
            loops: self.loops.split_at(x, y, ring).del(x).del(y),
            fans: self.fans.del(x).del(y),
            face: self.face.insert(ring, self.face[self.loops.place[x].0]),
            shell: self.shell,
            shells: self.shells,
            holes: self.holes,
        }
    }

    /// `kemr` keeps every rule.
    pub proof fn lemma_kemr(self, x: Fin, ring: nat)
        requires
            self.wf(),
            self.edges.contains(x.edge),
            self.loops.place[x].0 == self.loops.place[partner(x)].0,
            self.loops.succ[x] != partner(x),
            self.loops.succ[partner(x)] != x,
            !self.loops.rings.dom().contains(ring),
        ensures
            self.kemr(x, ring).wf(),
    {
        let y = partner(x);
        let e = x.edge;
        let k = self.loops.place[x].0;
        let b = self.kemr(x, ring);
        let s = self.loops.succ;
        let t = self.fans.succ;
        assert(partner(y) == x);
        assert(x != y);
        assert(self.loops.place.dom().contains(x));
        assert(self.loops.place.dom().contains(y));
        assert(self.fans.place.dom().contains(x));
        assert(self.fans.place.dom().contains(y));
        assert(t[x] == s[y]);
        assert(t[y] == s[x]);
        // Two fins in one loop: neither is alone in it.
        if s[x] == x {
            self.loops.lemma_alone(x, y);
        }
        if s[y] == y {
            self.loops.lemma_alone(y, x);
        }
        self.loops.lemma_split_at(x, y, ring);
        let l1 = self.loops.split_at(x, y, ring);
        assert(l1.succ == s.insert(x, s[y]).insert(y, s[x]));
        assert(l1.succ[x] == s[y]);
        assert(l1.succ[y] == s[x]);
        assert(l1.place.dom().contains(x));
        l1.lemma_del(x);
        let l2 = l1.del(x);
        assert(l2.place.dom().contains(y));
        l2.lemma_del(y);
        self.fans.lemma_del(x);
        let f1 = self.fans.del(x);
        assert(f1.place.dom().contains(y));
        f1.lemma_del(y);
        assert forall|u: Fin| #[trigger] b.loops.place.dom().contains(u) <==> b.edges.contains(u.edge) by {
            assert(self.loops.place.dom().contains(u) <==> self.edges.contains(u.edge));
            if u.edge == e {
                assert(u == x || u == y);
            }
        }
        assert forall|u: Fin| #[trigger] b.fans.place.dom().contains(u) <==> b.edges.contains(u.edge) by {
            assert(self.fans.place.dom().contains(u) <==> self.edges.contains(u.edge));
            if u.edge == e {
                assert(u == x || u == y);
            }
        }
        assert forall|u: Fin| #[trigger] b.edges.contains(u.edge) implies b.fans.succ[u]
            == b.loops.succ[partner(u)] by {
            let v = partner(u);
            assert(self.edges.contains(u.edge));
            assert(self.edges.contains(v.edge));
            assert(u != x && u != y && v != x && v != y);
            assert(self.loops.place.dom().contains(v));
            assert(self.fans.place.dom().contains(u));
            assert(t[u] == s[v]);
            assert(l1.succ[v] == s[v]);
        }
        assert(self.loops.at(k, self.loops.place[x].1));
        assert(self.face.dom().contains(k));
        assert(b.face.dom() =~= b.loops.rings.dom());
        assert forall|m: nat| #[trigger] b.face.dom().contains(m) implies b.shell.dom().contains(
            b.face[m],
        ) by {
            if m != ring {
                assert(self.face.dom().contains(m));
            }
        }
    }

    /// KFMRH: removes the face of loop `ring`, that face's only loop, making `ring` a loop of
    /// `keep`, another face of the same shell. One hole more.
    pub open spec fn kfmrh(self, keep: nat, ring: nat) -> Body {
        Body {
            edges: self.edges,
            loops: self.loops,
            fans: self.fans,
            face: self.face.insert(ring, keep),
            shell: self.shell.remove(self.face[ring]),
            shells: self.shells,
            holes: self.holes + 1,
        }
    }

    /// `kfmrh` keeps every rule.
    pub proof fn lemma_kfmrh(self, keep: nat, ring: nat)
        requires
            self.wf(),
            self.shell.dom().contains(keep),
            self.face.dom().contains(ring),
            self.face[ring] != keep,
            self.shell[self.face[ring]] == self.shell[keep],
            forall|m: nat|
                #[trigger] self.face.dom().contains(m) && self.face[m] == self.face[ring] ==> m == ring,
        ensures
            self.kfmrh(keep, ring).wf(),
    {
        let b = self.kfmrh(keep, ring);
        let gone = self.face[ring];
        assert(self.shell.dom().contains(gone));
        assert(b.face.dom() =~= b.loops.rings.dom());
        assert forall|m: nat| #[trigger] b.face.dom().contains(m) implies b.shell.dom().contains(
            b.face[m],
        ) by {
            if m != ring {
                assert(self.face.dom().contains(m));
                assert(self.face[m] != gone);
            }
        }
        assert forall|m: nat| #[trigger] b.shell.dom().contains(m) implies b.shells.contains(
            b.shell[m],
        ) by {
            assert(self.shell.dom().contains(m));
        }
    }

    /// MFKRH: makes `ring`, a loop of a face that keeps another loop, the one loop of a new face
    /// `f` on the same shell. One hole fewer.
    pub open spec fn mfkrh(self, ring: nat, f: nat) -> Body {
        Body {
            edges: self.edges,
            loops: self.loops,
            fans: self.fans,
            face: self.face.insert(ring, f),
            shell: self.shell.insert(f, self.shell[self.face[ring]]),
            shells: self.shells,
            holes: self.holes - 1,
        }
    }

    /// `mfkrh` keeps every rule.
    pub proof fn lemma_mfkrh(self, ring: nat, f: nat)
        requires
            self.wf(),
            self.face.dom().contains(ring),
            !self.shell.dom().contains(f),
        ensures
            self.mfkrh(ring, f).wf(),
    {
        let b = self.mfkrh(ring, f);
        let old = self.face[ring];
        assert(self.shell.dom().contains(old));
        assert(self.shells.contains(self.shell[old]));
        assert(b.face.dom() =~= b.loops.rings.dom());
        assert forall|m: nat| #[trigger] b.face.dom().contains(m) implies b.shell.dom().contains(
            b.face[m],
        ) by {
            if m != ring {
                assert(self.face.dom().contains(m));
            }
        }
        assert forall|m: nat| #[trigger] b.shell.dom().contains(m) implies b.shells.contains(
            b.shell[m],
        ) by {
            if m != f {
                assert(self.shell.dom().contains(m));
            }
        }
    }
}

} // verus!
