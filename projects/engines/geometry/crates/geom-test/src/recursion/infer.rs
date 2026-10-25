//! Inference variables for the recursion check's type checker: one table per body checked.
//!
//! What it does: [`Infer`] holds the inference variables of one reading of one body: general
//! ones, and the integer and floating-point ones an unsuffixed literal gets. [`Infer::unify`]
//! makes two types equal by binding variables, and fails, binding nothing, when they cannot be;
//! [`Infer::resolve`] gives a type with every bound variable replaced. [`Infer::snapshot`] and
//! [`Infer::rollback_to`] let the checker try a unification (does this implementation apply?)
//! and undo it. [`Infer::default_numeric`] gives the literal variables nothing else fixed their
//! defaults, `i32` and `f64`, as the compiler does.
//!
//! Guarantees:
//!
//! - **Unification is all or nothing.** A failed [`Infer::unify`] leaves the table as it was.
//! - **No infinite types.** A variable is never bound to a type that holds it (the occurs check).
//! - **Literal variables stay numeric.** An integer variable binds only to an integer type or a
//!   variable, a floating-point one only to a floating-point type or a variable.
//! - **`!` and unknown types bind nothing.** Unifying with [`Ty::Never`] (which coerces to any
//!   type) or [`Ty::Unknown`] succeeds without binding, so a variable keeps the type its other
//!   uses give it.
//!
//! Method: first-order unification with union-find over variables (J. A. Robinson, "A
//! machine-oriented logic based on the resolution principle", J. ACM 12 (1965),
//! <https://doi.org/10.1145/321250.321253>; R. E. Tarjan, "Efficiency of a good but not linear
//! set union algorithm", J. ACM 22 (1975), <https://doi.org/10.1145/321879.321884>), with an undo
//! log for snapshots, as rustc's `InferCtxt` keeps one
//! (<https://rustc-dev-guide.rust-lang.org/type-inference.html>); integer and float variables and
//! their defaults from The Rust Reference
//! (<https://doc.rust-lang.org/reference/expressions/literal-expr.html>).
//!
//! Cost: a unification is linear in the size of the two types; finding a variable's root follows
//! its chain of unions (no path compression, so undo stays simple).

use crate::recursion::ty::{ClosureKey, Prim, Ty};

/// What a variable may be bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    /// Any type.
    General,
    /// An integer type: an unsuffixed integer literal.
    Int,
    /// A floating-point type: an unsuffixed float literal.
    Float,
}

/// One undoable change.
#[derive(Clone, Debug)]
enum Undo {
    NewVar,
    Parent(usize, u32),
    Value(usize, Option<Ty>),
    Kind(usize, VarKind),
}

/// A point to roll back to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot(usize);

/// The inference table of one body.
#[derive(Clone, Debug, Default)]
pub struct Infer {
    parent: Vec<u32>,
    value: Vec<Option<Ty>>,
    kind: Vec<VarKind>,
    log: Vec<Undo>,
}

impl Infer {
    /// An empty table.
    #[must_use]
    pub fn new() -> Infer {
        Infer::default()
    }

    /// How many variables the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// Whether the table holds no variable.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    fn new_var(&mut self, kind: VarKind) -> Ty {
        let id = u32::try_from(self.parent.len()).unwrap_or(u32::MAX);
        self.parent.push(id);
        self.value.push(None);
        self.kind.push(kind);
        self.log.push(Undo::NewVar);
        Ty::Var(id)
    }

    /// A fresh general variable.
    pub fn fresh(&mut self) -> Ty {
        self.new_var(VarKind::General)
    }

    /// A fresh integer variable.
    pub fn fresh_int(&mut self) -> Ty {
        self.new_var(VarKind::Int)
    }

    /// A fresh floating-point variable.
    pub fn fresh_float(&mut self) -> Ty {
        self.new_var(VarKind::Float)
    }

    fn root(&self, v: u32) -> u32 {
        let mut cur = v;
        // Each union points a root at another root, so the chain is at most the number of
        // variables long.
        for _ in 0..=self.parent.len() {
            match self.parent.get(cur as usize) {
                Some(&p) if p != cur => cur = p,
                _ => break,
            }
        }
        cur
    }

    /// The kind of variable `v`'s class.
    #[must_use]
    pub fn kind_of(&self, v: u32) -> VarKind {
        self.kind
            .get(self.root(v) as usize)
            .copied()
            .unwrap_or(VarKind::General)
    }

    /// The type with its outermost variables replaced by what they are bound to (a variable's
    /// root when unbound).
    #[must_use]
    pub fn shallow(&self, t: &Ty) -> Ty {
        let mut cur = t.clone();
        for _ in 0..=self.parent.len() {
            let Ty::Var(v) = cur else {
                return cur;
            };
            let r = self.root(v);
            match self.value.get(r as usize).and_then(Option::as_ref) {
                Some(bound) => cur = bound.clone(),
                None => return Ty::Var(r),
            }
        }
        cur
    }

    /// The type with every bound variable replaced, deeply; unbound ones by their roots.
    #[must_use]
    pub fn resolve(&self, t: &Ty) -> Ty {
        self.resolve_at(t, 0)
    }

    fn resolve_at(&self, t: &Ty, depth: usize) -> Ty {
        if depth > self.parent.len().saturating_add(2) {
            return Ty::Unknown;
        }
        t.fold(&mut |x| match x {
            Ty::Var(_) => {
                let s = self.shallow(x);
                if matches!(s, Ty::Var(_)) {
                    Some(s)
                } else {
                    Some(self.resolve_at(&s, depth.saturating_add(1)))
                }
            }
            _ => None,
        })
    }

    /// Whether variable class `v` occurs in `t`.
    fn occurs(&self, v: u32, t: &Ty) -> bool {
        let r = self.root(v);
        self.resolve(t).any(&|x| matches!(x, Ty::Var(y) if self.root(*y) == r))
    }

    /// A point to roll back to.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot(self.log.len())
    }

    /// Undoes every change since `s`.
    pub fn rollback_to(&mut self, s: Snapshot) {
        while self.log.len() > s.0 {
            let Some(u) = self.log.pop() else {
                break;
            };
            match u {
                Undo::NewVar => {
                    self.parent.pop();
                    self.value.pop();
                    self.kind.pop();
                }
                Undo::Parent(i, old) => {
                    if let Some(p) = self.parent.get_mut(i) {
                        *p = old;
                    }
                }
                Undo::Value(i, old) => {
                    if let Some(v) = self.value.get_mut(i) {
                        *v = old;
                    }
                }
                Undo::Kind(i, old) => {
                    if let Some(k) = self.kind.get_mut(i) {
                        *k = old;
                    }
                }
            }
        }
    }

    fn set_value(&mut self, r: u32, t: Ty) {
        let i = r as usize;
        if let Some(slot) = self.value.get_mut(i) {
            let old = slot.replace(t);
            self.log.push(Undo::Value(i, old));
        }
    }

    fn set_parent(&mut self, child: u32, to: u32) {
        let i = child as usize;
        if let Some(slot) = self.parent.get_mut(i) {
            let old = *slot;
            *slot = to;
            self.log.push(Undo::Parent(i, old));
        }
    }

    fn set_kind(&mut self, r: u32, k: VarKind) {
        let i = r as usize;
        if let Some(slot) = self.kind.get_mut(i) {
            let old = *slot;
            *slot = k;
            self.log.push(Undo::Kind(i, old));
        }
    }

    /// Makes `a` and `b` equal; on failure nothing changes.
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> bool {
        let s = self.snapshot();
        let ok = self.unify_at(a, b, 0);
        if !ok {
            self.rollback_to(s);
        }
        ok
    }

    /// Whether `a` and `b` can be made equal, changing nothing.
    pub fn can_unify(&mut self, a: &Ty, b: &Ty) -> bool {
        let s = self.snapshot();
        let ok = self.unify_at(a, b, 0);
        self.rollback_to(s);
        ok
    }

    fn bind(&mut self, v: u32, t: &Ty) -> bool {
        let r = self.root(v);
        match self.kind_of(r) {
            VarKind::Int if !matches!(t, Ty::Prim(p) if p.is_int()) => return false,
            VarKind::Float if !matches!(t, Ty::Prim(p) if p.is_float()) => return false,
            _ => {}
        }
        if self.occurs(r, t) {
            return false;
        }
        self.set_value(r, t.clone());
        true
    }

    fn unify_all(&mut self, xs: &[Ty], ys: &[Ty], depth: usize) -> bool {
        xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| self.unify_at(x, y, depth))
    }

    fn unify_at(&mut self, a: &Ty, b: &Ty, depth: usize) -> bool {
        if depth > crate::recursion::ty::MAX_DEPTH.saturating_mul(4) {
            return true;
        }
        let d = depth.saturating_add(1);
        let a = self.shallow(a);
        let b = self.shallow(b);
        match (&a, &b) {
            (Ty::Var(x), Ty::Var(y)) => {
                let (rx, ry) = (self.root(*x), self.root(*y));
                if rx == ry {
                    return true;
                }
                let kind = match (self.kind_of(rx), self.kind_of(ry)) {
                    (VarKind::General, k) | (k, VarKind::General) => k,
                    (VarKind::Int, VarKind::Int) => VarKind::Int,
                    (VarKind::Float, VarKind::Float) => VarKind::Float,
                    _ => return false,
                };
                self.set_parent(rx, ry);
                self.set_kind(ry, kind);
                true
            }
            (Ty::Never | Ty::Unknown, _) | (_, Ty::Never | Ty::Unknown) => true,
            (Ty::Var(x), t) | (t, Ty::Var(x)) => self.bind(*x, t),
            (Ty::Infer, _) | (_, Ty::Infer) => true,
            (Ty::Prim(p), Ty::Prim(q)) => p == q,
            (Ty::Adt(i, xs), Ty::Adt(j, ys)) => i == j && self.unify_all(xs, ys, d),
            (Ty::Ref(x, m), Ty::Ref(y, n)) => m == n && self.unify_at(x, y, d),
            (Ty::Slice(x), Ty::Slice(y)) | (Ty::Array(x), Ty::Array(y)) => self.unify_at(x, y, d),
            (Ty::Tuple(xs), Ty::Tuple(ys)) => self.unify_all(xs, ys, d),
            (Ty::FnPtr(xs, x), Ty::FnPtr(ys, y)) => self.unify_all(xs, ys, d) && self.unify_at(x, y, d),
            (Ty::Closure(k), Ty::Closure(l)) => same_closure(k, l) && self.unify_all(&k.subst, &l.subst, d),
            (Ty::FnDef(n, xs), Ty::FnDef(m, ys)) => n == m && self.unify_all(xs, ys, d),
            (Ty::Param(g), Ty::Param(h)) => g == h,
            (Ty::Dyn(xs), Ty::Dyn(ys)) | (Ty::Opaque(xs), Ty::Opaque(ys)) => {
                xs.len() == ys.len()
                    && xs.iter().zip(ys).all(|(x, y)| {
                        x.id == y.id
                            && self.unify_all(&x.args, &y.args, d)
                            && x.bindings.iter().all(|(n, t)| {
                                y.binding(n).is_none_or(|u| self.unify_at(t, u, d))
                            })
                    })
            }
            // A projection not resolved yet may turn out to be anything.
            (Ty::Assoc(..), _) | (_, Ty::Assoc(..)) => true,
            _ => false,
        }
    }

    /// Gives every unbound integer variable `i32` and every unbound float variable `f64`.
    pub fn default_numeric(&mut self) {
        for i in 0..self.parent.len() {
            let Ok(v) = u32::try_from(i) else {
                break;
            };
            let r = self.root(v);
            if self.value.get(r as usize).is_some_and(Option::is_some) {
                continue;
            }
            match self.kind_of(r) {
                VarKind::Int => self.set_value(r, Ty::Prim(Prim::I32)),
                VarKind::Float => self.set_value(r, Ty::Prim(Prim::F64)),
                VarKind::General => {}
            }
        }
    }
}

fn same_closure(a: &ClosureKey, b: &ClosureKey) -> bool {
    a.owner == b.owner && a.index == b.index
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

    fn adt(i: usize, args: Vec<Ty>) -> Ty {
        Ty::Adt(ItemId(i), args)
    }

    #[test]
    fn variables_bind_and_resolve_deeply() {
        let mut t = Infer::new();
        let a = t.fresh();
        let b = t.fresh();
        assert!(t.unify(&adt(1, vec![a.clone()]), &adt(1, vec![b.clone()])));
        assert!(t.unify(&b, &Ty::Prim(Prim::U64)));
        assert_eq!(t.resolve(&adt(2, vec![a])), adt(2, vec![Ty::Prim(Prim::U64)]));
    }

    #[test]
    fn a_failed_unification_changes_nothing() {
        let mut t = Infer::new();
        let a = t.fresh();
        let x = Ty::Tuple(vec![a.clone(), Ty::Prim(Prim::Bool)]);
        let y = Ty::Tuple(vec![Ty::Prim(Prim::U8), Ty::Prim(Prim::Char)]);
        assert!(!t.unify(&x, &y));
        assert_eq!(t.shallow(&a), a, "the first element's binding was undone");
    }

    #[test]
    fn the_occurs_check_refuses_infinite_types() {
        let mut t = Infer::new();
        let a = t.fresh();
        assert!(!t.unify(&a, &adt(1, vec![a.clone()])));
    }

    #[test]
    fn literal_variables_stay_numeric_and_default() {
        let mut t = Infer::new();
        let i = t.fresh_int();
        let f = t.fresh_float();
        assert!(!t.unify(&i, &Ty::Prim(Prim::F64)));
        assert!(!t.unify(&f, &Ty::Prim(Prim::U8)));
        assert!(!t.unify(&i, &f));
        let g = t.fresh();
        assert!(t.unify(&g, &i));
        assert!(!t.unify(&g, &Ty::Prim(Prim::Bool)), "the general variable joined the integer class");
        t.default_numeric();
        assert_eq!(t.resolve(&g), Ty::Prim(Prim::I32));
        assert_eq!(t.resolve(&f), Ty::Prim(Prim::F64));
    }

    #[test]
    fn never_and_unknown_bind_nothing() {
        let mut t = Infer::new();
        let a = t.fresh();
        assert!(t.unify(&a, &Ty::Never));
        assert!(t.unify(&a, &Ty::Unknown));
        assert!(t.unify(&a, &Ty::Prim(Prim::U16)));
        assert_eq!(t.resolve(&a), Ty::Prim(Prim::U16));
    }

    #[test]
    fn snapshots_roll_back_new_variables_and_bindings() {
        let mut t = Infer::new();
        let a = t.fresh();
        let s = t.snapshot();
        let b = t.fresh();
        assert!(t.unify(&a, &b));
        assert!(t.unify(&b, &Ty::Prim(Prim::U32)));
        t.rollback_to(s);
        assert_eq!(t.len(), 1);
        assert_eq!(t.shallow(&a), a);
        assert!(t.can_unify(&a, &Ty::Prim(Prim::U8)));
        assert_eq!(t.shallow(&a), a, "can_unify binds nothing");
    }
}
