//! Rings of fins, for `geom-topo`'s Verus model of the Euler operators (chapter 28.7 of the kernel
//! architecture, "Topology, by a model"; piece G111).
//!
//! The model: [`Rings`] holds fins in cycles. Each ring is a key and a sequence, read round and
//! round; `succ` gives the fin after each fin, and `place` gives where each fin is, so a fin is in
//! exactly one place of exactly one ring and `succ` is that place's next ([`Rings::wf`]). Nothing
//! here says what a ring is for: a body's loops are rings, and so are its vertex fans
//! (`euler.rs`).
//!
//! The moves, each proved to keep a ring set well formed, with what it does to `succ`:
//!
//! | Move | What it does |
//! | --- | --- |
//! | [`Rings::fresh`] | a new ring of one fin |
//! | [`Rings::drop_fin`] | removes a ring of one fin |
//! | [`Rings::ins`] | a new fin after a fin |
//! | [`Rings::del`] | removes a fin from a ring of more than one |
//! | [`Rings::split_at`] | swaps the `succ` of two fins of one ring, which cuts it in two |
//! | [`Rings::join_at`] | swaps the `succ` of two fins of two rings, which joins them |
//!
//! The last two are the one step that changes how many rings there are, as a transposition does
//! to the cycles of a permutation. `turn`, `push`, `pop`, `split` and `join` work at a ring's end,
//! where the sequences are simple; the moves first turn the ring to put their fin last.

use vstd::prelude::*;

verus! {

/// A fin: the use of an edge in one direction.
pub ghost struct Fin {
    /// The edge.
    pub edge: nat,
    /// Whether it runs along the edge.
    pub forward: bool,
}

/// The place after place `i` of a ring of `n`.
pub open spec fn after(i: int, n: int) -> int {
    if i + 1 < n {
        i + 1
    } else {
        0
    }
}

/// `l` turned to put place `i` last.
pub open spec fn turned(l: Seq<Fin>, i: int) -> Seq<Fin> {
    l.subrange(i + 1, l.len() as int) + l.subrange(0, i + 1)
}

/// The place of `turned(l, i)[j]` in `l`, for `l` of `n` fins.
pub open spec fn source(j: int, i: int, n: int) -> int {
    if j + i + 1 < n {
        j + i + 1
    } else {
        j + i + 1 - n
    }
}

/// The place of `l[j]` in `turned(l, i)`, for `l` of `n` fins.
pub open spec fn target(j: int, i: int, n: int) -> int {
    if j > i {
        j - i - 1
    } else {
        j + n - i - 1
    }
}

/// `turned`, place by place.
pub proof fn lemma_turned(l: Seq<Fin>, i: int)
    requires
        0 <= i < l.len(),
    ensures
        turned(l, i).len() == l.len(),
        forall|j: int| 0 <= j < l.len() ==> #[trigger] turned(l, i)[j] == l[source(j, i, l.len() as int)],
{
    let n = l.len() as int;
    let t = turned(l, i);
    assert(t.len() == n);
    assert forall|j: int| 0 <= j < n implies #[trigger] t[j] == l[source(j, i, n)] by {
        if j + i + 1 < n {
            assert(t[j] == l.subrange(i + 1, n)[j]);
        } else {
            assert(t[j] == l.subrange(0, i + 1)[j - (n - i - 1)]);
        }
    }
}

/// Fins in rings.
pub ghost struct Rings {
    /// The rings, by key.
    pub rings: Map<nat, Seq<Fin>>,
    /// The fin after each fin.
    pub succ: Map<Fin, Fin>,
    /// Where each fin is: its ring's key, and its place in the ring.
    pub place: Map<Fin, (nat, int)>,
}

impl Rings {
    /// No rings.
    pub open spec fn empty() -> Rings {
        Rings { rings: Map::empty(), succ: Map::empty(), place: Map::empty() }
    }

    /// Ring `k` has a place `i`.
    pub open spec fn at(self, k: nat, i: int) -> bool {
        self.rings.dom().contains(k) && 0 <= i < self.rings[k].len()
    }

    /// Every ring holds a fin, every place holds a fin that `place` names, every fin `place` names
    /// is at that place, and `succ` is the next place, round the ring.
    pub open spec fn wf(self) -> bool {
        &&& forall|k: nat| #[trigger] self.rings.dom().contains(k) ==> self.rings[k].len() > 0
        &&& forall|k: nat, i: int|
            #![trigger self.rings[k][i]]
            self.at(k, i) ==> {
                &&& self.place.dom().contains(self.rings[k][i])
                &&& self.place[self.rings[k][i]] == (k, i)
            }
        &&& forall|x: Fin|
            #[trigger] self.place.dom().contains(x) ==> {
                &&& self.at(self.place[x].0, self.place[x].1)
                &&& self.rings[self.place[x].0][self.place[x].1] == x
            }
        &&& self.succ.dom() == self.place.dom()
        &&& forall|k: nat, i: int|
            #![trigger self.succ[self.rings[k][i]]]
            self.at(k, i) ==> self.succ[self.rings[k][i]] == self.rings[k][after(
                i,
                self.rings[k].len() as int,
            )]
    }

    /// The empty set of rings is well formed.
    pub proof fn lemma_empty()
        ensures
            Rings::empty().wf(),
    {
        let e = Rings::empty();
        assert(e.succ.dom() == e.place.dom());
    }

    /// What `wf` says of one fin's `succ`.
    pub proof fn lemma_succ(self, x: Fin)
        requires
            self.wf(),
            self.place.dom().contains(x),
        ensures
            self.succ[x] == self.rings[self.place[x].0][after(
                self.place[x].1,
                self.rings[self.place[x].0].len() as int,
            )],
            self.place.dom().contains(self.succ[x]),
            self.place[self.succ[x]] == (
                self.place[x].0,
                after(self.place[x].1, self.rings[self.place[x].0].len() as int),
            ),
    {
        let k = self.place[x].0;
        let i = self.place[x].1;
        let n = self.rings[k].len() as int;
        assert(self.at(k, i));
        assert(self.rings[k][i] == x);
        assert(self.succ[self.rings[k][i]] == self.rings[k][after(i, n)]);
        assert(self.at(k, after(i, n)));
        assert(self.place[self.rings[k][after(i, n)]] == (k, after(i, n)));
    }

    /// Two fins with one `succ` are one fin.
    pub proof fn lemma_succ_injective(self, a: Fin, b: Fin)
        requires
            self.wf(),
            self.place.dom().contains(a),
            self.place.dom().contains(b),
            self.succ[a] == self.succ[b],
        ensures
            a == b,
    {
        self.lemma_succ(a);
        self.lemma_succ(b);
        assert(self.at(self.place[a].0, self.place[a].1));
        assert(self.at(self.place[b].0, self.place[b].1));
        assert(self.place[a] == self.place[b]);
    }

    /// A fin that is its own `succ` is the only fin of its ring.
    pub proof fn lemma_alone(self, x: Fin, y: Fin)
        requires
            self.wf(),
            self.place.dom().contains(x),
            self.place.dom().contains(y),
            self.succ[x] == x,
            x != y,
        ensures
            self.place[x].0 != self.place[y].0,
    {
        let k = self.place[x].0;
        let i = self.place[x].1;
        self.lemma_succ(x);
        assert(self.at(k, i));
        assert(self.rings[k][i] == x);
        assert(after(i, self.rings[k].len() as int) == i);
        assert(self.rings[k].len() == 1);
        assert(self.at(self.place[y].0, self.place[y].1));
        if self.place[y].0 == k {
            assert(self.place[y].1 == 0);
            assert(i == 0);
            assert(self.rings[k][0] == y);
        }
    }

    /// The fin before `x`.
    pub open spec fn pred(self, x: Fin) -> Fin {
        let k = self.place[x].0;
        let i = self.place[x].1;
        self.rings[k][if i > 0 {
            i - 1
        } else {
            self.rings[k].len() - 1
        }]
    }

    /// `pred` is before `x`, in its ring.
    pub proof fn lemma_pred(self, x: Fin)
        requires
            self.wf(),
            self.place.dom().contains(x),
        ensures
            self.place.dom().contains(self.pred(x)),
            self.succ[self.pred(x)] == x,
            self.place[self.pred(x)].0 == self.place[x].0,
    {
        let k = self.place[x].0;
        let i = self.place[x].1;
        let n = self.rings[k].len() as int;
        assert(self.at(k, i));
        let j = if i > 0 {
            i - 1
        } else {
            n - 1
        };
        assert(self.at(k, j));
        assert(self.pred(x) == self.rings[k][j]);
        assert(after(j, n) == i);
        assert(self.succ[self.rings[k][j]] == self.rings[k][after(j, n)]);
        assert(self.place[self.rings[k][j]] == (k, j));
    }

    /// Ring `k` turned to put place `i` last.
    pub open spec fn turn(self, k: nat, i: int) -> Rings {
        let n = self.rings[k].len() as int;
        Rings {
            rings: self.rings.insert(k, turned(self.rings[k], i)),
            succ: self.succ,
            place: Map::new(
                self.place.dom(),
                |x: Fin|
                    if self.place[x].0 == k {
                        (k, target(self.place[x].1, i, n))
                    } else {
                        self.place[x]
                    },
            ),
        }
    }

    /// Turning a ring keeps everything but where it starts.
    pub proof fn lemma_turn(self, k: nat, i: int)
        requires
            self.wf(),
            self.at(k, i),
        ensures
            self.turn(k, i).wf(),
            self.turn(k, i).rings.dom() == self.rings.dom(),
            self.turn(k, i).place.dom() == self.place.dom(),
            self.turn(k, i).succ == self.succ,
            self.turn(k, i).rings[k].len() == self.rings[k].len(),
            self.turn(k, i).rings[k][self.rings[k].len() - 1] == self.rings[k][i],
            forall|x: Fin|
                self.place.dom().contains(x) ==> #[trigger] self.turn(k, i).place[x].0
                    == self.place[x].0,
    {
        let t = self.turn(k, i);
        let l = self.rings[k];
        let n = l.len() as int;
        lemma_turned(l, i);
        assert(t.rings[k] == turned(l, i));
        assert(t.rings.dom() =~= self.rings.dom());
        assert(source(n - 1, i, n) == i);
        assert forall|m: nat| #[trigger] t.rings.dom().contains(m) implies t.rings[m].len() > 0 by {
            assert(self.rings.dom().contains(m));
            if m != k {
                assert(t.rings[m] == self.rings[m]);
            }
        }
        assert forall|m: nat, j: int|
            #![trigger t.rings[m][j]]
            t.at(m, j) implies t.place.dom().contains(t.rings[m][j]) && t.place[t.rings[m][j]] == (
            m,
            j,
        ) by {
            if m == k {
                let s = source(j, i, n);
                assert(t.rings[m][j] == l[s]);
                assert(self.at(k, s));
                assert(self.place[l[s]] == (k, s));
                assert(target(s, i, n) == j);
            } else {
                assert(t.rings[m] == self.rings[m]);
                assert(self.at(m, j));
                assert(self.place[self.rings[m][j]] == (m, j));
            }
        }
        assert forall|x: Fin| #[trigger] t.place.dom().contains(x) implies t.at(
            t.place[x].0,
            t.place[x].1,
        ) && t.rings[t.place[x].0][t.place[x].1] == x by {
            let p = self.place[x];
            assert(self.at(p.0, p.1));
            assert(self.rings[p.0][p.1] == x);
            if p.0 == k {
                let d = target(p.1, i, n);
                assert(t.place[x] == (k, d));
                assert(source(d, i, n) == p.1);
                assert(t.rings[k][d] == l[p.1]);
            } else {
                assert(t.place[x] == p);
                assert(t.rings[p.0] == self.rings[p.0]);
            }
        }
        assert forall|m: nat, j: int|
            #![trigger t.succ[t.rings[m][j]]]
            t.at(m, j) implies t.succ[t.rings[m][j]] == t.rings[m][after(
            j,
            t.rings[m].len() as int,
        )] by {
            if m == k {
                let s = source(j, i, n);
                let a = after(j, n);
                assert(t.rings[k][j] == l[s]);
                assert(self.at(k, s));
                assert(self.succ[l[s]] == l[after(s, n)]);
                assert(source(a, i, n) == after(s, n));
                assert(t.rings[k][a] == l[source(a, i, n)]);
            } else {
                assert(t.rings[m] == self.rings[m]);
                assert(self.at(m, j));
                assert(self.succ[self.rings[m][j]] == self.rings[m][after(
                    j,
                    self.rings[m].len() as int,
                )]);
            }
        }
        assert(t.succ.dom() =~= t.place.dom());
        assert forall|x: Fin| self.place.dom().contains(x) implies #[trigger] t.place[x].0
            == self.place[x].0 by {
        }
    }

    /// `x` added to ring `k`, after its last fin.
    pub open spec fn push(self, k: nat, x: Fin) -> Rings {
        let l = self.rings[k];
        let last = l[l.len() - 1];
        Rings {
            rings: self.rings.insert(k, l.push(x)),
            succ: self.succ.insert(last, x).insert(x, self.succ[last]),
            place: self.place.insert(x, (k, l.len() as int)),
        }
    }

    /// Adding a fin at a ring's end.
    pub proof fn lemma_push(self, k: nat, x: Fin)
        requires
            self.wf(),
            self.rings.dom().contains(k),
            !self.place.dom().contains(x),
        ensures
            self.push(k, x).wf(),
            self.push(k, x).rings.dom() == self.rings.dom(),
            self.push(k, x).place.dom() == self.place.dom().insert(x),
            self.push(k, x).place[x].0 == k,
            forall|y: Fin|
                self.place.dom().contains(y) ==> #[trigger] self.push(k, x).place[y] == self.place[y],
    {
        let r = self.push(k, x);
        let l = self.rings[k];
        let n = l.len() as int;
        let last = l[n - 1];
        assert(n > 0);
        assert(self.at(k, n - 1));
        assert(self.place[last] == (k, n - 1));
        assert(self.place.dom().contains(last));
        assert(self.succ[last] == l[0]) by {
            assert(self.succ[self.rings[k][n - 1]] == self.rings[k][after(n - 1, n)]);
        }
        assert(last != x);
        assert(r.rings[k] == l.push(x));
        assert(r.rings.dom() =~= self.rings.dom());
        assert forall|m: nat| #[trigger] r.rings.dom().contains(m) implies r.rings[m].len() > 0 by {
            assert(self.rings.dom().contains(m));
            if m != k {
                assert(r.rings[m] == self.rings[m]);
            }
        }
        assert forall|m: nat, j: int|
            #![trigger r.rings[m][j]]
            r.at(m, j) implies r.place.dom().contains(r.rings[m][j]) && r.place[r.rings[m][j]] == (
            m,
            j,
        ) by {
            if m == k {
                if j == n {
                    assert(r.rings[k][j] == x);
                } else {
                    assert(r.rings[k][j] == l[j]);
                    assert(self.at(k, j));
                    assert(self.place.dom().contains(l[j]));
                    assert(self.place[l[j]] == (k, j));
                }
            } else {
                assert(r.rings[m] == self.rings[m]);
                assert(self.at(m, j));
                assert(self.place.dom().contains(self.rings[m][j]));
                assert(self.place[self.rings[m][j]] == (m, j));
            }
        }
        assert forall|y: Fin| #[trigger] r.place.dom().contains(y) implies r.at(
            r.place[y].0,
            r.place[y].1,
        ) && r.rings[r.place[y].0][r.place[y].1] == y by {
            if y != x {
                let p = self.place[y];
                assert(self.place.dom().contains(y));
                assert(self.at(p.0, p.1));
                assert(self.rings[p.0][p.1] == y);
                assert(r.place[y] == p);
                if p.0 == k {
                    assert(r.rings[k][p.1] == l[p.1]);
                } else {
                    assert(r.rings[p.0] == self.rings[p.0]);
                }
            }
        }
        assert(r.succ.dom() =~= r.place.dom());
        assert forall|m: nat, j: int|
            #![trigger r.succ[r.rings[m][j]]]
            r.at(m, j) implies r.succ[r.rings[m][j]] == r.rings[m][after(
            j,
            r.rings[m].len() as int,
        )] by {
            if m == k {
                if j == n {
                    assert(r.rings[k][j] == x);
                    assert(r.rings[k][0] == l[0]);
                } else if j == n - 1 {
                    assert(r.rings[k][j] == last);
                    assert(r.rings[k][n] == x);
                } else {
                    assert(r.rings[k][j] == l[j]);
                    assert(self.at(k, j));
                    assert(self.at(k, j + 1));
                    assert(self.place[l[j]] == (k, j));
                    assert(l[j] != last);
                    assert(l[j] != x);
                    assert(self.succ[self.rings[k][j]] == self.rings[k][after(j, n)]);
                    assert(r.rings[k][j + 1] == l[j + 1]);
                }
            } else {
                let y = self.rings[m][j];
                assert(r.rings[m] == self.rings[m]);
                assert(self.at(m, j));
                assert(self.place.dom().contains(y));
                assert(self.place[y] == (m, j));
                assert(y != last);
                assert(y != x);
                assert(self.succ[self.rings[m][j]] == self.rings[m][after(
                    j,
                    self.rings[m].len() as int,
                )]);
            }
        }
        assert forall|y: Fin| self.place.dom().contains(y) implies #[trigger] r.place[y]
            == self.place[y] by {
            assert(y != x);
        }
    }

    /// Ring `k` without its last fin.
    pub open spec fn pop(self, k: nat) -> Rings {
        let l = self.rings[k];
        let n = l.len() as int;
        let last = l[n - 1];
        Rings {
            rings: self.rings.insert(k, l.subrange(0, n - 1)),
            succ: self.succ.remove(last).insert(l[n - 2], self.succ[last]),
            place: self.place.remove(last),
        }
    }

    /// Removing a fin from a ring's end.
    pub proof fn lemma_pop(self, k: nat)
        requires
            self.wf(),
            self.rings.dom().contains(k),
            self.rings[k].len() >= 2,
        ensures
            self.pop(k).wf(),
            self.pop(k).rings.dom() == self.rings.dom(),
            self.pop(k).place.dom() == self.place.dom().remove(
                self.rings[k][self.rings[k].len() - 1],
            ),
            forall|y: Fin|
                self.pop(k).place.dom().contains(y) ==> #[trigger] self.pop(k).place[y]
                    == self.place[y],
    {
        let r = self.pop(k);
        let l = self.rings[k];
        let n = l.len() as int;
        let last = l[n - 1];
        let before = l[n - 2];
        assert(self.at(k, n - 1));
        assert(self.at(k, n - 2));
        assert(self.place[last] == (k, n - 1));
        assert(self.place[before] == (k, n - 2));
        assert(self.place.dom().contains(before));
        assert(before != last);
        assert(self.succ[last] == l[0]) by {
            assert(self.succ[self.rings[k][n - 1]] == self.rings[k][after(n - 1, n)]);
        }
        assert(r.rings[k] == l.subrange(0, n - 1));
        assert(r.rings.dom() =~= self.rings.dom());
        assert forall|m: nat| #[trigger] r.rings.dom().contains(m) implies r.rings[m].len() > 0 by {
            assert(self.rings.dom().contains(m));
            if m != k {
                assert(r.rings[m] == self.rings[m]);
            }
        }
        assert forall|m: nat, j: int|
            #![trigger r.rings[m][j]]
            r.at(m, j) implies r.place.dom().contains(r.rings[m][j]) && r.place[r.rings[m][j]] == (
            m,
            j,
        ) by {
            if m == k {
                assert(r.rings[k][j] == l[j]);
                assert(self.at(k, j));
                assert(self.place.dom().contains(l[j]));
                assert(self.place[l[j]] == (k, j));
                assert(l[j] != last);
            } else {
                let y = self.rings[m][j];
                assert(r.rings[m] == self.rings[m]);
                assert(self.at(m, j));
                assert(self.place.dom().contains(y));
                assert(self.place[y] == (m, j));
                assert(y != last);
            }
        }
        assert forall|y: Fin| #[trigger] r.place.dom().contains(y) implies r.at(
            r.place[y].0,
            r.place[y].1,
        ) && r.rings[r.place[y].0][r.place[y].1] == y by {
            let p = self.place[y];
            assert(self.place.dom().contains(y));
            assert(y != last);
            assert(self.at(p.0, p.1));
            assert(self.rings[p.0][p.1] == y);
            assert(r.place[y] == p);
            if p.0 == k {
                assert(p.1 != n - 1);
                assert(r.rings[k][p.1] == l[p.1]);
            } else {
                assert(r.rings[p.0] == self.rings[p.0]);
            }
        }
        assert(r.succ.dom() =~= r.place.dom());
        assert forall|m: nat, j: int|
            #![trigger r.succ[r.rings[m][j]]]
            r.at(m, j) implies r.succ[r.rings[m][j]] == r.rings[m][after(
            j,
            r.rings[m].len() as int,
        )] by {
            if m == k {
                assert(r.rings[k][j] == l[j]);
                assert(self.at(k, j));
                assert(self.place[l[j]] == (k, j));
                assert(l[j] != last);
                if j == n - 2 {
                    assert(r.rings[k][0] == l[0]);
                } else {
                    assert(self.at(k, j + 1));
                    assert(l[j] != before);
                    assert(self.succ[self.rings[k][j]] == self.rings[k][after(j, n)]);
                    assert(r.rings[k][j + 1] == l[j + 1]);
                }
            } else {
                let y = self.rings[m][j];
                assert(r.rings[m] == self.rings[m]);
                assert(self.at(m, j));
                assert(self.place[y] == (m, j));
                assert(y != last);
                assert(y != before);
                assert(self.succ[self.rings[m][j]] == self.rings[m][after(
                    j,
                    self.rings[m].len() as int,
                )]);
            }
        }
        assert forall|y: Fin| r.place.dom().contains(y) implies #[trigger] r.place[y]
            == self.place[y] by {
            assert(y != last);
        }
    }

    /// Ring `k` cut after place `i`, with places past it in a new ring `k2`; the ring's last fin
    /// and the fin at `i` swap their `succ`.
    pub open spec fn split(self, k: nat, i: int, k2: nat) -> Rings {
        let l = self.rings[k];
        let n = l.len() as int;
        let (a, b) = (l[i], l[n - 1]);
        Rings {
            rings: self.rings.insert(k, l.subrange(0, i + 1)).insert(k2, l.subrange(i + 1, n)),
            succ: self.succ.insert(a, self.succ[b]).insert(b, self.succ[a]),
            place: Map::new(
                self.place.dom(),
                |x: Fin|
                    if self.place[x].0 == k && self.place[x].1 > i {
                        (k2, self.place[x].1 - i - 1)
                    } else {
                        self.place[x]
                    },
            ),
        }
    }

    /// Cutting a ring in two.
    pub proof fn lemma_split(self, k: nat, i: int, k2: nat)
        requires
            self.wf(),
            self.at(k, i),
            i < self.rings[k].len() - 1,
            !self.rings.dom().contains(k2),
        ensures
            self.split(k, i, k2).wf(),
            self.split(k, i, k2).rings.dom() == self.rings.dom().insert(k2),
            self.split(k, i, k2).place.dom() == self.place.dom(),
    {
        let r = self.split(k, i, k2);
        let l = self.rings[k];
        let n = l.len() as int;
        let a = l[i];
        let b = l[n - 1];
        assert(k != k2);
        assert(self.at(k, n - 1));
        assert(self.place[a] == (k, i));
        assert(self.place[b] == (k, n - 1));
        assert(self.place.dom().contains(a));
        assert(self.place.dom().contains(b));
        assert(a != b);
        assert(self.succ[b] == l[0]) by {
            assert(self.succ[self.rings[k][n - 1]] == self.rings[k][after(n - 1, n)]);
        }
        assert(self.succ[a] == l[i + 1]) by {
            assert(self.succ[self.rings[k][i]] == self.rings[k][after(i, n)]);
        }
        assert(r.rings[k] == l.subrange(0, i + 1));
        assert(r.rings[k2] == l.subrange(i + 1, n));
        assert(r.rings.dom() =~= self.rings.dom().insert(k2));
        assert forall|m: nat| #[trigger] r.rings.dom().contains(m) implies r.rings[m].len() > 0 by {
            if m != k && m != k2 {
                assert(self.rings.dom().contains(m));
                assert(r.rings[m] == self.rings[m]);
            }
        }
        assert forall|m: nat, j: int|
            #![trigger r.rings[m][j]]
            r.at(m, j) implies r.place.dom().contains(r.rings[m][j]) && r.place[r.rings[m][j]] == (
            m,
            j,
        ) by {
            if m == k {
                assert(r.rings[k][j] == l[j]);
                assert(self.at(k, j));
                assert(self.place.dom().contains(l[j]));
                assert(self.place[l[j]] == (k, j));
            } else if m == k2 {
                assert(r.rings[k2][j] == l[i + 1 + j]);
                assert(self.at(k, i + 1 + j));
                assert(self.place.dom().contains(l[i + 1 + j]));
                assert(self.place[l[i + 1 + j]] == (k, i + 1 + j));
            } else {
                assert(r.rings[m] == self.rings[m]);
                assert(self.at(m, j));
                assert(self.place.dom().contains(self.rings[m][j]));
                assert(self.place[self.rings[m][j]] == (m, j));
            }
        }
        assert forall|y: Fin| #[trigger] r.place.dom().contains(y) implies r.at(
            r.place[y].0,
            r.place[y].1,
        ) && r.rings[r.place[y].0][r.place[y].1] == y by {
            let p = self.place[y];
            assert(self.place.dom().contains(y));
            assert(self.at(p.0, p.1));
            assert(self.rings[p.0][p.1] == y);
            if p.0 == k && p.1 > i {
                assert(r.place[y] == (k2, p.1 - i - 1));
                assert(r.rings[k2][p.1 - i - 1] == l[p.1]);
            } else if p.0 == k {
                assert(r.place[y] == p);
                assert(r.rings[k][p.1] == l[p.1]);
            } else {
                assert(p.0 != k2);
                assert(r.place[y] == p);
                assert(r.rings[p.0] == self.rings[p.0]);
            }
        }
        assert(r.succ.dom() =~= r.place.dom());
        assert forall|m: nat, j: int|
            #![trigger r.succ[r.rings[m][j]]]
            r.at(m, j) implies r.succ[r.rings[m][j]] == r.rings[m][after(
            j,
            r.rings[m].len() as int,
        )] by {
            if m == k {
                assert(r.rings[k][j] == l[j]);
                assert(self.at(k, j));
                assert(self.place[l[j]] == (k, j));
                assert(l[j] != b);
                if j == i {
                    assert(r.rings[k][0] == l[0]);
                } else {
                    assert(l[j] != a);
                    assert(self.succ[self.rings[k][j]] == self.rings[k][after(j, n)]);
                    assert(r.rings[k][j + 1] == l[j + 1]);
                }
            } else if m == k2 {
                let s = i + 1 + j;
                assert(r.rings[k2][j] == l[s]);
                assert(self.at(k, s));
                assert(self.place[l[s]] == (k, s));
                assert(l[s] != a);
                assert(r.rings[k2][0] == l[i + 1]);
                if s == n - 1 {
                    assert(l[s] == b);
                } else {
                    assert(l[s] != b);
                    assert(self.succ[self.rings[k][s]] == self.rings[k][after(s, n)]);
                    assert(r.rings[k2][j + 1] == l[s + 1]);
                }
            } else {
                let y = self.rings[m][j];
                assert(r.rings[m] == self.rings[m]);
                assert(self.at(m, j));
                assert(self.place[y] == (m, j));
                assert(y != a);
                assert(y != b);
                assert(self.succ[self.rings[m][j]] == self.rings[m][after(
                    j,
                    self.rings[m].len() as int,
                )]);
            }
        }
    }

    /// Ring `k1` followed by ring `k2`, under `k1`; their last fins swap their `succ`.
    pub open spec fn join(self, k1: nat, k2: nat) -> Rings {
        let (a, b) = (self.rings[k1], self.rings[k2]);
        let (p, q) = (a[a.len() - 1], b[b.len() - 1]);
        Rings {
            rings: self.rings.remove(k2).insert(k1, a + b),
            succ: self.succ.insert(p, self.succ[q]).insert(q, self.succ[p]),
            place: Map::new(
                self.place.dom(),
                |x: Fin|
                    if self.place[x].0 == k2 {
                        (k1, a.len() + self.place[x].1)
                    } else {
                        self.place[x]
                    },
            ),
        }
    }

    /// Joining two rings.
    pub proof fn lemma_join(self, k1: nat, k2: nat)
        requires
            self.wf(),
            self.rings.dom().contains(k1),
            self.rings.dom().contains(k2),
            k1 != k2,
        ensures
            self.join(k1, k2).wf(),
            self.join(k1, k2).rings.dom() == self.rings.dom().remove(k2),
            self.join(k1, k2).place.dom() == self.place.dom(),
    {
        let r = self.join(k1, k2);
        let a = self.rings[k1];
        let b = self.rings[k2];
        let na = a.len() as int;
        let nb = b.len() as int;
        let p = a[na - 1];
        let q = b[nb - 1];
        assert(self.at(k1, na - 1));
        assert(self.at(k2, nb - 1));
        assert(self.place[p] == (k1, na - 1));
        assert(self.place[q] == (k2, nb - 1));
        assert(self.place.dom().contains(p));
        assert(self.place.dom().contains(q));
        assert(p != q);
        assert(self.succ[p] == a[0]) by {
            assert(self.succ[self.rings[k1][na - 1]] == self.rings[k1][after(na - 1, na)]);
        }
        assert(self.succ[q] == b[0]) by {
            assert(self.succ[self.rings[k2][nb - 1]] == self.rings[k2][after(nb - 1, nb)]);
        }
        assert(r.rings[k1] == a + b);
        assert(r.rings.dom() =~= self.rings.dom().remove(k2));
        assert forall|m: nat| #[trigger] r.rings.dom().contains(m) implies r.rings[m].len() > 0 by {
            assert(self.rings.dom().contains(m));
            if m != k1 {
                assert(r.rings[m] == self.rings[m]);
            }
        }
        assert forall|m: nat, j: int|
            #![trigger r.rings[m][j]]
            r.at(m, j) implies r.place.dom().contains(r.rings[m][j]) && r.place[r.rings[m][j]] == (
            m,
            j,
        ) by {
            if m == k1 {
                if j < na {
                    assert(r.rings[k1][j] == a[j]);
                    assert(self.at(k1, j));
                    assert(self.place.dom().contains(a[j]));
                    assert(self.place[a[j]] == (k1, j));
                } else {
                    assert(r.rings[k1][j] == b[j - na]);
                    assert(self.at(k2, j - na));
                    assert(self.place.dom().contains(b[j - na]));
                    assert(self.place[b[j - na]] == (k2, j - na));
                }
            } else {
                assert(m != k2);
                assert(r.rings[m] == self.rings[m]);
                assert(self.at(m, j));
                assert(self.place.dom().contains(self.rings[m][j]));
                assert(self.place[self.rings[m][j]] == (m, j));
            }
        }
        assert forall|y: Fin| #[trigger] r.place.dom().contains(y) implies r.at(
            r.place[y].0,
            r.place[y].1,
        ) && r.rings[r.place[y].0][r.place[y].1] == y by {
            let t = self.place[y];
            assert(self.place.dom().contains(y));
            assert(self.at(t.0, t.1));
            assert(self.rings[t.0][t.1] == y);
            if t.0 == k2 {
                assert(r.place[y] == (k1, na + t.1));
                assert(r.rings[k1][na + t.1] == b[t.1]);
            } else if t.0 == k1 {
                assert(r.place[y] == t);
                assert(r.rings[k1][t.1] == a[t.1]);
            } else {
                assert(r.place[y] == t);
                assert(r.rings[t.0] == self.rings[t.0]);
            }
        }
        assert(r.succ.dom() =~= r.place.dom());
        assert forall|m: nat, j: int|
            #![trigger r.succ[r.rings[m][j]]]
            r.at(m, j) implies r.succ[r.rings[m][j]] == r.rings[m][after(
            j,
            r.rings[m].len() as int,
        )] by {
            if m == k1 {
                assert(r.rings[k1][0] == a[0]);
                assert(r.rings[k1][na] == b[0]);
                if j < na {
                    assert(r.rings[k1][j] == a[j]);
                    assert(self.at(k1, j));
                    assert(self.place[a[j]] == (k1, j));
                    assert(a[j] != q);
                    if j < na - 1 {
                        assert(a[j] != p);
                        assert(self.succ[self.rings[k1][j]] == self.rings[k1][after(j, na)]);
                        assert(r.rings[k1][j + 1] == a[j + 1]);
                    }
                } else {
                    let s = j - na;
                    assert(r.rings[k1][j] == b[s]);
                    assert(self.at(k2, s));
                    assert(self.place[b[s]] == (k2, s));
                    assert(b[s] != p);
                    if s < nb - 1 {
                        assert(b[s] != q);
                        assert(self.succ[self.rings[k2][s]] == self.rings[k2][after(s, nb)]);
                        assert(r.rings[k1][j + 1] == b[s + 1]);
                    }
                }
            } else {
                let y = self.rings[m][j];
                assert(m != k2);
                assert(r.rings[m] == self.rings[m]);
                assert(self.at(m, j));
                assert(self.place[y] == (m, j));
                assert(y != p);
                assert(y != q);
                assert(self.succ[self.rings[m][j]] == self.rings[m][after(
                    j,
                    self.rings[m].len() as int,
                )]);
            }
        }
    }

    /// A new ring `k` of the one fin `x`.
    pub open spec fn fresh(self, k: nat, x: Fin) -> Rings {
        Rings {
            rings: self.rings.insert(k, seq![x]),
            succ: self.succ.insert(x, x),
            place: self.place.insert(x, (k, 0int)),
        }
    }

    /// Adding a ring of one fin.
    pub proof fn lemma_fresh(self, k: nat, x: Fin)
        requires
            self.wf(),
            !self.rings.dom().contains(k),
            !self.place.dom().contains(x),
        ensures
            self.fresh(k, x).wf(),
            self.fresh(k, x).rings.dom() == self.rings.dom().insert(k),
            self.fresh(k, x).place.dom() == self.place.dom().insert(x),
            self.fresh(k, x).place[x].0 == k,
            forall|y: Fin|
                self.place.dom().contains(y) ==> #[trigger] self.fresh(k, x).place[y] == self.place[y],
    {
        let r = self.fresh(k, x);
        assert(r.rings[k] == seq![x]);
        assert(r.rings[k].len() == 1);
        assert(r.rings.dom() =~= self.rings.dom().insert(k));
        assert forall|m: nat| #[trigger] r.rings.dom().contains(m) implies r.rings[m].len() > 0 by {
            if m != k {
                assert(self.rings.dom().contains(m));
                assert(r.rings[m] == self.rings[m]);
            }
        }
        assert forall|m: nat, j: int|
            #![trigger r.rings[m][j]]
            r.at(m, j) implies r.place.dom().contains(r.rings[m][j]) && r.place[r.rings[m][j]] == (
            m,
            j,
        ) by {
            if m == k {
                assert(j == 0);
                assert(r.rings[k][0] == x);
            } else {
                let y = self.rings[m][j];
                assert(r.rings[m] == self.rings[m]);
                assert(self.at(m, j));
                assert(self.place.dom().contains(y));
                assert(self.place[y] == (m, j));
                assert(y != x);
            }
        }
        assert forall|y: Fin| #[trigger] r.place.dom().contains(y) implies r.at(
            r.place[y].0,
            r.place[y].1,
        ) && r.rings[r.place[y].0][r.place[y].1] == y by {
            if y == x {
                assert(r.rings[k][0] == x);
            } else {
                let p = self.place[y];
                assert(self.place.dom().contains(y));
                assert(self.at(p.0, p.1));
                assert(self.rings[p.0][p.1] == y);
                assert(p.0 != k);
                assert(r.rings[p.0] == self.rings[p.0]);
            }
        }
        assert(r.succ.dom() =~= r.place.dom());
        assert forall|m: nat, j: int|
            #![trigger r.succ[r.rings[m][j]]]
            r.at(m, j) implies r.succ[r.rings[m][j]] == r.rings[m][after(
            j,
            r.rings[m].len() as int,
        )] by {
            if m == k {
                assert(j == 0);
                assert(r.rings[k][0] == x);
            } else {
                let y = self.rings[m][j];
                assert(r.rings[m] == self.rings[m]);
                assert(self.at(m, j));
                assert(self.place.dom().contains(y));
                assert(y != x);
                assert(self.succ[self.rings[m][j]] == self.rings[m][after(
                    j,
                    self.rings[m].len() as int,
                )]);
            }
        }
        assert forall|y: Fin| self.place.dom().contains(y) implies #[trigger] r.place[y]
            == self.place[y] by {
            assert(y != x);
        }
    }

    /// Without the ring of the one fin `x`.
    pub open spec fn drop_fin(self, x: Fin) -> Rings {
        Rings {
            rings: self.rings.remove(self.place[x].0),
            succ: self.succ.remove(x),
            place: self.place.remove(x),
        }
    }

    /// Removing a ring of one fin.
    pub proof fn lemma_drop_fin(self, x: Fin)
        requires
            self.wf(),
            self.place.dom().contains(x),
            self.succ[x] == x,
        ensures
            self.drop_fin(x).wf(),
            self.drop_fin(x).rings.dom() == self.rings.dom().remove(self.place[x].0),
            self.rings.dom().contains(self.place[x].0),
            self.drop_fin(x).place.dom() == self.place.dom().remove(x),
            forall|y: Fin|
                self.place.dom().contains(y) && y != x ==> #[trigger] self.drop_fin(x).succ[y]
                    == self.succ[y],
            forall|y: Fin|
                self.place.dom().contains(y) && y != x ==> #[trigger] self.drop_fin(x).place[y]
                    == self.place[y],
    {
        let r = self.drop_fin(x);
        let k = self.place[x].0;
        assert(self.at(k, self.place[x].1));
        assert(r.rings.dom() =~= self.rings.dom().remove(k));
        assert forall|y: Fin| #[trigger] self.place.dom().contains(y) && y != x implies self.place[y].0 != k by {
            self.lemma_alone(x, y);
        }
        assert forall|m: nat| #[trigger] r.rings.dom().contains(m) implies r.rings[m].len() > 0 by {
            assert(self.rings.dom().contains(m));
            assert(r.rings[m] == self.rings[m]);
        }
        assert forall|m: nat, j: int|
            #![trigger r.rings[m][j]]
            r.at(m, j) implies r.place.dom().contains(r.rings[m][j]) && r.place[r.rings[m][j]] == (
            m,
            j,
        ) by {
            let y = self.rings[m][j];
            assert(r.rings[m] == self.rings[m]);
            assert(self.at(m, j));
            assert(self.place.dom().contains(y));
            assert(self.place[y] == (m, j));
            assert(y != x);
        }
        assert forall|y: Fin| #[trigger] r.place.dom().contains(y) implies r.at(
            r.place[y].0,
            r.place[y].1,
        ) && r.rings[r.place[y].0][r.place[y].1] == y by {
            let p = self.place[y];
            assert(self.place.dom().contains(y));
            assert(self.at(p.0, p.1));
            assert(self.rings[p.0][p.1] == y);
            assert(p.0 != k);
            assert(r.rings[p.0] == self.rings[p.0]);
        }
        assert(r.succ.dom() =~= r.place.dom());
        assert forall|m: nat, j: int|
            #![trigger r.succ[r.rings[m][j]]]
            r.at(m, j) implies r.succ[r.rings[m][j]] == r.rings[m][after(
            j,
            r.rings[m].len() as int,
        )] by {
            let y = self.rings[m][j];
            assert(r.rings[m] == self.rings[m]);
            assert(self.at(m, j));
            assert(self.place.dom().contains(y));
            assert(self.place[y] == (m, j));
            assert(y != x);
            assert(self.succ[self.rings[m][j]] == self.rings[m][after(
                j,
                self.rings[m].len() as int,
            )]);
        }
    }

    /// A new fin `x` after the fin `p`.
    pub open spec fn ins(self, p: Fin, x: Fin) -> Rings {
        let t = self.place[p];
        self.turn(t.0, t.1).push(t.0, x)
    }

    /// Adding a fin after a fin.
    pub proof fn lemma_ins(self, p: Fin, x: Fin)
        requires
            self.wf(),
            self.place.dom().contains(p),
            !self.place.dom().contains(x),
        ensures
            self.ins(p, x).wf(),
            self.ins(p, x).rings.dom() == self.rings.dom(),
            self.ins(p, x).place.dom() == self.place.dom().insert(x),
            self.ins(p, x).succ == self.succ.insert(p, x).insert(x, self.succ[p]),
            self.ins(p, x).place[x].0 == self.place[p].0,
            forall|y: Fin|
                self.place.dom().contains(y) ==> #[trigger] self.ins(p, x).place[y].0
                    == self.place[y].0,
    {
        let t = self.place[p];
        assert(self.at(t.0, t.1));
        assert(self.rings[t.0][t.1] == p);
        self.lemma_turn(t.0, t.1);
        let r = self.turn(t.0, t.1);
        assert(r.place.dom() == self.place.dom());
        assert(r.rings.dom().contains(t.0));
        r.lemma_push(t.0, x);
        let n = self.rings[t.0].len() as int;
        assert(r.rings[t.0].len() == n);
        assert(r.rings[t.0][n - 1] == p);
        assert forall|y: Fin| self.place.dom().contains(y) implies #[trigger] self.ins(p, x).place[y].0
            == self.place[y].0 by {
            assert(r.push(t.0, x).place[y] == r.place[y]);
            assert(r.place[y].0 == self.place[y].0);
        }
    }

    /// Without the fin `x`.
    pub open spec fn del(self, x: Fin) -> Rings {
        let t = self.place[x];
        self.turn(t.0, t.1).pop(t.0)
    }

    /// Removing a fin from a ring that holds another.
    pub proof fn lemma_del(self, x: Fin)
        requires
            self.wf(),
            self.place.dom().contains(x),
            self.succ[x] != x,
        ensures
            self.del(x).wf(),
            self.del(x).rings.dom() == self.rings.dom(),
            self.del(x).place.dom() == self.place.dom().remove(x),
            forall|y: Fin|
                self.place.dom().contains(y) && y != x ==> #[trigger] self.del(x).succ[y] == (
                if self.succ[y] == x {
                    self.succ[x]
                } else {
                    self.succ[y]
                }),
            forall|y: Fin|
                self.place.dom().contains(y) && y != x ==> #[trigger] self.del(x).place[y].0
                    == self.place[y].0,
    {
        let t = self.place[x];
        let k = t.0;
        let n = self.rings[k].len() as int;
        assert(self.at(k, t.1));
        assert(self.rings[k][t.1] == x);
        self.lemma_succ(x);
        if n == 1 {
            assert(t.1 == 0);
            assert(after(t.1, n) == 0);
        }
        assert(n >= 2);
        self.lemma_turn(k, t.1);
        let r = self.turn(k, t.1);
        assert(r.rings.dom().contains(k));
        r.lemma_pop(k);
        assert(r.rings[k][n - 1] == x);
        let before = r.rings[k][n - 2];
        assert(r.at(k, n - 2));
        assert(r.place.dom().contains(before));
        assert(r.place[before] == (k, n - 2));
        assert(self.place.dom().contains(before));
        assert(before != x) by {
            assert(r.at(k, n - 1));
            assert(r.place[r.rings[k][n - 1]] == (k, n - 1));
        }
        assert(self.succ[before] == x) by {
            assert(r.succ[r.rings[k][n - 2]] == r.rings[k][after(n - 2, n)]);
        }
        assert forall|y: Fin| self.place.dom().contains(y) && y != x implies #[trigger] self.del(x).succ[y]
            == (if self.succ[y] == x {
            self.succ[x]
        } else {
            self.succ[y]
        }) by {
            if self.succ[y] == x {
                self.lemma_succ_injective(y, before);
            }
        }
        assert forall|y: Fin| self.place.dom().contains(y) && y != x implies #[trigger] self.del(x).place[y].0
            == self.place[y].0 by {
            assert(r.pop(k).place.dom().contains(y));
            assert(r.pop(k).place[y] == r.place[y]);
            assert(r.place[y].0 == self.place[y].0);
        }
    }

    /// The two fins `a` and `b` of one ring swap their `succ`, which cuts the ring in two:
    /// a new ring `k2` holds `b`.
    pub open spec fn split_at(self, a: Fin, b: Fin, k2: nat) -> Rings {
        let t = self.place[b];
        let r = self.turn(t.0, t.1);
        r.split(t.0, r.place[a].1, k2)
    }

    /// Cutting a ring at two fins.
    pub proof fn lemma_split_at(self, a: Fin, b: Fin, k2: nat)
        requires
            self.wf(),
            self.place.dom().contains(a),
            self.place.dom().contains(b),
            self.place[a].0 == self.place[b].0,
            a != b,
            !self.rings.dom().contains(k2),
        ensures
            self.split_at(a, b, k2).wf(),
            self.split_at(a, b, k2).rings.dom() == self.rings.dom().insert(k2),
            self.split_at(a, b, k2).place.dom() == self.place.dom(),
            self.split_at(a, b, k2).succ == self.succ.insert(a, self.succ[b]).insert(b, self.succ[a]),
    {
        let t = self.place[b];
        let k = t.0;
        let n = self.rings[k].len() as int;
        assert(self.at(k, t.1));
        assert(self.rings[k][t.1] == b);
        self.lemma_turn(k, t.1);
        let r = self.turn(k, t.1);
        assert(r.place.dom().contains(a));
        assert(r.place.dom().contains(b));
        assert(r.place[a].0 == k);
        let i = r.place[a].1;
        assert(r.at(k, i));
        assert(r.rings[k][i] == a);
        assert(r.rings[k][n - 1] == b);
        assert(i != n - 1);
        assert(r.rings.dom() == self.rings.dom());
        r.lemma_split(k, i, k2);
    }

    /// The two fins `a` and `b` of two rings swap their `succ`, which joins the rings: `b`'s ring
    /// goes, and `a`'s holds them both.
    pub open spec fn join_at(self, a: Fin, b: Fin) -> Rings {
        let (s, t) = (self.place[a], self.place[b]);
        let r = self.turn(s.0, s.1);
        let u = r.turn(t.0, t.1);
        u.join(s.0, t.0)
    }

    /// Joining two rings at two fins.
    pub proof fn lemma_join_at(self, a: Fin, b: Fin)
        requires
            self.wf(),
            self.place.dom().contains(a),
            self.place.dom().contains(b),
            self.place[a].0 != self.place[b].0,
        ensures
            self.join_at(a, b).wf(),
            self.join_at(a, b).rings.dom() == self.rings.dom().remove(self.place[b].0),
            self.rings.dom().contains(self.place[b].0),
            self.join_at(a, b).place.dom() == self.place.dom(),
            self.join_at(a, b).succ == self.succ.insert(a, self.succ[b]).insert(b, self.succ[a]),
    {
        let s = self.place[a];
        let t = self.place[b];
        let (k1, k2) = (s.0, t.0);
        let (na, nb) = (self.rings[k1].len() as int, self.rings[k2].len() as int);
        assert(self.at(k1, s.1));
        assert(self.at(k2, t.1));
        assert(self.rings[k1][s.1] == a);
        assert(self.rings[k2][t.1] == b);
        self.lemma_turn(k1, s.1);
        let r = self.turn(k1, s.1);
        assert(r.rings[k2] == self.rings[k2]);
        assert(r.at(k2, t.1));
        r.lemma_turn(k2, t.1);
        let u = r.turn(k2, t.1);
        assert(u.rings[k1] == r.rings[k1]);
        assert(u.rings.dom() == self.rings.dom());
        assert(u.rings[k1].len() == na);
        assert(u.rings[k2].len() == nb);
        assert(u.rings[k1][na - 1] == a);
        assert(u.rings[k2][nb - 1] == b);
        u.lemma_join(k1, k2);
    }
}

} // verus!
