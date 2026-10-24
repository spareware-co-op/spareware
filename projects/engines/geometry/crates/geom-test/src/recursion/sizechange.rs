//! Size-change graphs over type terms: which cycles of the generic call graph some finite
//! instantiation of their type parameters can run.
//!
//! What it does: the call graph has one node per function as written, generic or not, while the
//! program that runs has one copy of each generic function per instantiation. A cycle of written
//! functions is a cycle of running ones only if, going round it, every type parameter can come
//! back to the type it started as. Each call edge therefore carries a size-change graph ([`Scg`]):
//! for type parameters of the caller and of the callee, the facts the call states between their
//! instantiations, each one of [`Rel::Eq`] (the callee's parameter is the caller's), [`Rel::Down`]
//! (it is a strict part of it: a call through a parameter `T` that dispatches to an
//! implementation for `Foo<U>` binds `T = Foo<U>`, so `U` is smaller than `T`) or [`Rel::Up`] (it
//! is built around it: `f::<Vec<T>>` from `f::<T>`). [`relate`] reads those facts off the call
//! site by matching the callee's declared types against the caller's types; [`Scg::then`]
//! composes graphs along a walk; [`Scg::closes`] says whether a closed walk's facts allow every
//! parameter to equal itself again. [`Labels`] keeps, for one edge, the weakest graphs of its call
//! sites.
//!
//! Guarantees:
//!
//! - **Facts only.** Every arc [`relate`] writes holds for every instantiation that makes the
//!   call: the callee's parameter is the caller's (`Eq`), lies strictly inside it (`Down`), or
//!   strictly contains it (`Up`), where "inside" is the strict subterm order of type terms, so
//!   sizes, counted in type constructors, strictly fall or rise. Where the types at the call site
//!   are not known, or the match would need more than structure (an associated type, a closure's
//!   or function item's type, a trait object), no arc is written. A call site whose facts
//!   contradict each other (which a program that compiles cannot have, so it means a type was
//!   read wrongly) gets no arcs at all.
//! - **Missing facts never remove a cycle.** A closed walk is unrealisable only when its arcs
//!   force some parameter to be strictly smaller or larger than itself; fewer arcs only make more
//!   walks realisable. Composition keeps only what follows from both graphs, and a set of graphs
//!   keeps only those no other one is a subset of ([`Labels`]), which is the weaker side.
//! - **Deterministic.** Arcs and graphs are ordered sets.
//!
//! Method: the size-change principle (C. S. Lee, N. D. Jones and A. M. Ben-Amram, "The
//! size-change principle for program termination", POPL 2001,
//! <https://doi.org/10.1145/360204.360210>): graphs of strict and non-strict decreases between
//! the parameters of a caller and a callee, composed along walks, with the closure of a strongly
//! connected component computed by a work list; here the measured values are type terms ordered
//! by size, and a walk is kept when its composed graph can be satisfied with the end equal to the
//! start, because Rust generates one copy of a generic function per instantiation
//! (<https://rustc-dev-guide.rust-lang.org/backend/monomorph.html>), so a running cycle returns
//! to the very instantiation it left. Whether the size constraints can be satisfied is decided as
//! for difference constraints: they can unless a cycle of them holds a strict step (T. H. Cormen,
//! C. E. Leiserson, R. L. Rivest and C. Stein, *Introduction to Algorithms*, 3rd edition, section
//! 24.4), found by the Floyd–Warshall closure (section 25.2).
//!
//! Cost: [`relate`] is linear in the size of the types it matches; [`Scg::then`] is quadratic in
//! the number of arcs; [`Scg::closes`] is cubic in the number of the node's type parameters,
//! which is a handful.

use std::collections::BTreeSet;

use crate::recursion::ty::{GenId, Ty};

/// How a callee's type parameter relates to a caller's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rel {
    /// The callee's parameter is the caller's.
    Eq,
    /// The callee's parameter is a strict part of the caller's.
    Down,
    /// The callee's parameter strictly contains the caller's.
    Up,
}

impl Rel {
    /// The relation along two steps, when one follows: equal then equal is equal; a strict step
    /// and equal or a step the same way is strict; opposite strict steps say nothing.
    #[must_use]
    pub fn then(self, next: Rel) -> Option<Rel> {
        match (self, next) {
            (Rel::Eq, r) | (r, Rel::Eq) => Some(r),
            (Rel::Down, Rel::Down) => Some(Rel::Down),
            (Rel::Up, Rel::Up) => Some(Rel::Up),
            (Rel::Down, Rel::Up) | (Rel::Up, Rel::Down) => None,
        }
    }
}

/// A size-change graph: arcs from a caller's type parameters to a callee's, by their positions
/// in each node's list of type parameters.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Scg {
    arcs: BTreeSet<(usize, usize, Rel)>,
}

impl Scg {
    /// The graph that states nothing.
    #[must_use]
    pub fn empty() -> Scg {
        Scg::default()
    }

    /// The graph of a step that keeps all `n` parameters as they are.
    #[must_use]
    pub fn identity(n: usize) -> Scg {
        Scg {
            arcs: (0..n).map(|i| (i, i, Rel::Eq)).collect(),
        }
    }

    /// A graph of the given arcs: (caller's parameter, callee's parameter, relation).
    #[must_use]
    pub fn of(arcs: impl IntoIterator<Item = (usize, usize, Rel)>) -> Scg {
        Scg {
            arcs: arcs.into_iter().collect(),
        }
    }

    /// Whether it states nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.arcs.is_empty()
    }

    /// Its arcs.
    pub fn arcs(&self) -> impl Iterator<Item = &(usize, usize, Rel)> {
        self.arcs.iter()
    }

    /// Whether every arc of `self` is an arc of `other`, so `self` is the weaker.
    #[must_use]
    pub fn is_subset(&self, other: &Scg) -> bool {
        self.arcs.is_subset(&other.arcs)
    }

    /// The graph of `self` followed by `next`: every arc that follows from an arc of each.
    #[must_use]
    pub fn then(&self, next: &Scg) -> Scg {
        let mut arcs = BTreeSet::new();
        for &(x, y, r1) in &self.arcs {
            for &(_, z, r2) in next.arcs.range((y, 0, Rel::Eq)..).take_while(|(y2, _, _)| *y2 == y) {
                if let Some(r) = r1.then(r2) {
                    arcs.insert((x, z, r));
                }
            }
        }
        Scg { arcs }
    }

    /// Whether a closed walk with this graph, at a node with `n` type parameters, can end with
    /// every parameter equal to where it started: false only when the arcs, with each end
    /// parameter taken as its start, force some parameter strictly below or above itself.
    #[must_use]
    pub fn closes(&self, n: usize) -> bool {
        let mut m = Matrix::new(n);
        for &(x, y, r) in &self.arcs {
            m.add(x, y, r);
        }
        !m.strict_cycle()
    }
}

/// Size constraints between type parameters: `le[i][j]` says size(i) <= size(j) and `lt[i][j]`
/// says size(i) < size(j).
struct Matrix {
    n: usize,
    le: Vec<bool>,
    lt: Vec<bool>,
}

impl Matrix {
    fn new(n: usize) -> Matrix {
        let cells = n.saturating_mul(n);
        Matrix {
            n,
            le: vec![false; cells],
            lt: vec![false; cells],
        }
    }

    fn cell(&self, i: usize, j: usize) -> Option<usize> {
        (i < self.n && j < self.n).then(|| i.saturating_mul(self.n).saturating_add(j))
    }

    fn set(&mut self, i: usize, j: usize, strict: bool) {
        if let Some(c) = self.cell(i, j) {
            if let Some(v) = self.le.get_mut(c) {
                *v = true;
            }
            if strict && let Some(v) = self.lt.get_mut(c) {
                *v = true;
            }
        }
    }

    fn get(&self, i: usize, j: usize) -> (bool, bool) {
        self.cell(i, j).map_or((false, false), |c| {
            (
                self.le.get(c).copied().unwrap_or(false),
                self.lt.get(c).copied().unwrap_or(false),
            )
        })
    }

    /// Adds the arc from parameter `x` to parameter `y` with relation `r`: the callee's `y`
    /// against the caller's `x`.
    fn add(&mut self, x: usize, y: usize, r: Rel) {
        match r {
            Rel::Eq => {
                self.set(x, y, false);
                self.set(y, x, false);
            }
            Rel::Down => self.set(y, x, true),
            Rel::Up => self.set(x, y, true),
        }
    }

    /// Whether the constraints hold a cycle with a strict step: Floyd–Warshall over (<=, <).
    fn strict_cycle(mut self) -> bool {
        for k in 0..self.n {
            for i in 0..self.n {
                let (ik_le, ik_lt) = self.get(i, k);
                if !ik_le {
                    continue;
                }
                for j in 0..self.n {
                    let (kj_le, kj_lt) = self.get(k, j);
                    if kj_le {
                        self.set(i, j, ik_lt || kj_lt);
                    }
                }
            }
        }
        (0..self.n).any(|i| self.get(i, i).1)
    }
}

/// The weakest size-change graphs of one edge's call sites: no graph kept is a superset of
/// another, so a graph that states nothing, once added, is the only one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Labels {
    graphs: BTreeSet<Scg>,
}

impl Labels {
    /// Adds `g`; returns whether the set changed.
    pub fn insert(&mut self, g: Scg) -> bool {
        if self.graphs.iter().any(|h| h.is_subset(&g)) {
            return false;
        }
        self.graphs.retain(|h| !g.is_subset(h));
        self.graphs.insert(g);
        true
    }

    /// The graphs kept.
    pub fn iter(&self) -> impl Iterator<Item = &Scg> {
        self.graphs.iter()
    }

    /// Whether the only graph kept, if any, states nothing.
    #[must_use]
    pub fn states_nothing(&self) -> bool {
        self.graphs.iter().all(Scg::is_empty)
    }
}

/// The type parameters written in `ty` whose size `ty`'s size includes: not inside an associated
/// type projection (which names another type), a closure's or function item's type, a function
/// pointer, or an unknown part.
fn params_in(ty: &Ty, out: &mut BTreeSet<GenId>) {
    let mut work: Vec<&Ty> = vec![ty];
    while let Some(t) = work.pop() {
        match t {
            Ty::Param(g) => {
                out.insert(*g);
            }
            Ty::Adt(_, args) | Ty::Ext(_, args) | Ty::Tuple(args) => work.extend(args.iter()),
            Ty::Ref(inner) | Ty::Slice(inner) | Ty::Array(inner) => work.push(inner),
            Ty::Dyn(traits) => {
                for tr in traits {
                    work.extend(tr.types());
                }
            }
            Ty::Assoc(..)
            | Ty::Prim(_)
            | Ty::FnPtr(_)
            | Ty::Closure(_)
            | Ty::FnDef(_)
            | Ty::Never
            | Ty::Unknown => {}
        }
    }
}

/// The size-change graph of a call: for each pair (`pattern`, `actual`), the callee's declared
/// type in its own type parameters `callee` and the caller's type at the call site in its type
/// parameters `caller`, matched structurally with references looked through.
#[must_use]
pub fn relate(pairs: &[(Ty, Ty)], caller: &[GenId], callee: &[GenId]) -> Scg {
    if caller.is_empty() || callee.is_empty() {
        return Scg::empty();
    }
    let at = |list: &[GenId], g: &GenId| list.iter().position(|x| x == g);
    let mut arcs: BTreeSet<(usize, usize, Rel)> = BTreeSet::new();
    let mut work: Vec<(&Ty, &Ty)> = pairs.iter().map(|(p, a)| (p, a)).collect();
    while let Some((p, a)) = work.pop() {
        let (p, a) = (p.peel(), a.peel());
        if let Ty::Param(y) = p
            && let Some(yi) = at(callee, y)
        {
            match a {
                Ty::Param(x) => {
                    if let Some(xi) = at(caller, x) {
                        arcs.insert((xi, yi, Rel::Eq));
                    }
                }
                other => {
                    let mut inside = BTreeSet::new();
                    params_in(other, &mut inside);
                    for x in &inside {
                        if let Some(xi) = at(caller, x) {
                            arcs.insert((xi, yi, Rel::Up));
                        }
                    }
                }
            }
            continue;
        }
        if let Ty::Param(x) = a
            && let Some(xi) = at(caller, x)
        {
            let mut inside = BTreeSet::new();
            params_in(p, &mut inside);
            for y in &inside {
                if let Some(yi) = at(callee, y) {
                    arcs.insert((xi, yi, Rel::Down));
                }
            }
            continue;
        }
        match (p, a) {
            (Ty::Adt(i, ps), Ty::Adt(j, xs)) if i == j && ps.len() == xs.len() => {
                work.extend(ps.iter().zip(xs.iter()));
            }
            (Ty::Ext(n, ps), Ty::Ext(m, xs)) if n == m && ps.len() == xs.len() => {
                work.extend(ps.iter().zip(xs.iter()));
            }
            (Ty::Tuple(ps), Ty::Tuple(xs)) if ps.len() == xs.len() => work.extend(ps.iter().zip(xs.iter())),
            (Ty::Slice(p) | Ty::Array(p), Ty::Slice(x) | Ty::Array(x)) => work.push((p, x)),
            _ => {}
        }
    }
    let scg = Scg { arcs };
    if consistent(&scg, caller.len(), callee.len()) {
        scg
    } else {
        Scg::empty()
    }
}

/// Whether one call site's facts can all hold together: the caller's and the callee's parameters
/// as separate unknowns, with no cycle of constraints through a strict step.
fn consistent(scg: &Scg, caller: usize, callee: usize) -> bool {
    let mut m = Matrix::new(caller.saturating_add(callee));
    for &(x, y, r) in &scg.arcs {
        m.add(x, caller.saturating_add(y), r);
    }
    !m.strict_cycle()
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "tests may use native arithmetic and casts"
)]
mod tests {
    use super::*;
    use crate::recursion::ty::ItemId;

    const T: GenId = GenId(0);
    const U: GenId = GenId(1);
    const V: GenId = GenId(2);

    fn p(g: GenId) -> Ty {
        Ty::Param(g)
    }

    fn adt(i: usize, args: Vec<Ty>) -> Ty {
        Ty::Adt(ItemId(i), args)
    }

    #[test]
    fn dispatch_through_a_parameter_to_a_wrapper_is_a_strict_decrease() {
        // `impl<T: Clone> Clone for W<T>` calls `T::clone`, which may be `<W<U> as Clone>`.
        let g = relate(&[(adt(1, vec![p(U)]), p(T))], &[T], &[U]);
        assert_eq!(g, Scg::of([(0, 0, Rel::Down)]));
        assert!(!g.closes(1), "W<T> -> W<U> with U inside T cannot come back to T");
        assert!(!g.then(&g).closes(1));
    }

    #[test]
    fn the_same_instantiation_closes() {
        // `fn walk<V>(v: &mut V)` called from `V::visit` with the same `V`.
        let g = relate(&[(Ty::Ref(Box::new(p(U))), Ty::Ref(Box::new(p(T))))], &[T], &[U]);
        assert_eq!(g, Scg::of([(0, 0, Rel::Eq)]));
        assert!(g.closes(1));
        // A concrete instantiation says nothing about the caller's parameter.
        let c = relate(&[(p(U), adt(3, vec![]))], &[T], &[U]);
        assert!(c.is_empty() && c.closes(1));
    }

    #[test]
    fn growth_is_a_strict_increase_and_opposite_steps_cancel() {
        let up = relate(&[(p(U), Ty::Ext("Vec".to_owned(), vec![p(T)]))], &[T], &[U]);
        assert_eq!(up, Scg::of([(0, 0, Rel::Up)]));
        assert!(!up.closes(1));
        let down = Scg::of([(0, 0, Rel::Down)]);
        let both = up.then(&down);
        assert!(both.is_empty(), "a rise then a fall says nothing: {both:?}");
        assert!(both.closes(1));
    }

    #[test]
    fn swaps_close_and_contradictions_are_dropped() {
        // f<T, U> -> f<U, T>: a swap returns after two steps and is realisable at once.
        let swap = Scg::of([(0, 1, Rel::Eq), (1, 0, Rel::Eq)]);
        assert!(swap.closes(2));
        assert!(swap.then(&swap).closes(2));
        // T' < U and U' = T: going round, T < U = T, which cannot hold.
        let g = Scg::of([(1, 0, Rel::Down), (0, 1, Rel::Eq)]);
        assert!(!g.closes(2));
        // One call site claiming U = T and U = Vec<T> at once is a misread type: no facts.
        let bad = relate(
            &[(p(U), p(T)), (p(U), Ty::Ext("Vec".to_owned(), vec![p(T)]))],
            &[T],
            &[U],
        );
        assert!(bad.is_empty());
    }

    #[test]
    fn projections_closures_and_unknowns_state_nothing() {
        for actual in [
            Ty::Assoc(Box::new(p(T)), "Item".to_owned()),
            Ty::Unknown,
            Ty::FnPtr(1),
        ] {
            let g = relate(&[(p(U), actual.clone())], &[T], &[U]);
            assert!(g.is_empty(), "{actual:?}: {g:?}");
        }
        // A mismatch of constructors states nothing either.
        let g = relate(&[(adt(1, vec![p(U)]), adt(2, vec![p(T)]))], &[T], &[U]);
        assert!(g.is_empty());
        // Nested structure is matched: W<(U, V)> against W<(T, Vec<T>)>.
        let g = relate(
            &[(
                adt(1, vec![Ty::Tuple(vec![p(U), p(V)])]),
                adt(1, vec![Ty::Tuple(vec![p(T), Ty::Ext("Vec".to_owned(), vec![p(T)])])]),
            )],
            &[T],
            &[U, V],
        );
        assert_eq!(g, Scg::of([(0, 0, Rel::Eq), (0, 1, Rel::Up)]));
    }

    #[test]
    fn labels_keep_the_weakest_graphs() {
        let mut l = Labels::default();
        assert!(l.insert(Scg::of([(0, 0, Rel::Down), (1, 1, Rel::Eq)])));
        assert!(l.insert(Scg::of([(0, 0, Rel::Up)])));
        assert!(!l.insert(Scg::of([(0, 0, Rel::Down), (1, 1, Rel::Eq), (1, 0, Rel::Eq)])));
        assert!(l.insert(Scg::of([(0, 0, Rel::Down)])));
        assert_eq!(l.iter().count(), 2, "the superset of (0,0,Down) went");
        assert!(l.insert(Scg::empty()));
        assert!(l.states_nothing());
        assert_eq!(l.iter().count(), 1);
    }

    #[test]
    fn composition_follows_shared_parameters_only() {
        let a = Scg::of([(0, 1, Rel::Eq), (1, 0, Rel::Down)]);
        let b = Scg::of([(1, 0, Rel::Down), (0, 0, Rel::Up)]);
        // 0 -Eq-> 1 -Down-> 0 gives (0, 0, Down); 1 -Down-> 0 -Up-> 0 gives nothing.
        assert_eq!(a.then(&b), Scg::of([(0, 0, Rel::Down)]));
        assert_eq!(Scg::identity(2).then(&a), a);
        assert_eq!(a.then(&Scg::identity(2)), a);
    }
}
