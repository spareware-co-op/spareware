//! What a call into the standard library returns, and what it may call back: the recursion
//! check's model of `core`, `alloc` and `std`.
//!
//! What it does: the kernel's crates call the standard library constantly (`Option::map`,
//! `slice::iter`, `Iterator::fold`, `Vec::push`, ...), and the standard library calls kernel code
//! back through what it is handed: closures, and trait implementations of kernel types. Without
//! its source, the analysis reads such a call by [`method`] (for `value.name(...)`) and
//! [`function`] (for `path::name(...)`), which give a [`Model`]: the call's result where the model
//! knows it (an `Option<T>`'s `map(f)` is an `Option` of what `f` returns), the values each
//! closure argument is called with (`f` gets the `T`), which outside traits the call may invoke on
//! the kernel types it is handed (`sort` invokes `Ord`, `push` none), whether it runs the
//! iterators it is handed, and for the conversions (`collect`, `sum`, `into`, `default`, ...) the
//! trait whose kernel implementation on the result type builds the result. A call it does not
//! model gets [`Model::unknown`]: it may call every closure it is handed, with values built from
//! what it is handed, and every outside-trait implementation of every kernel type it is handed,
//! and returns a value built from what it is handed and what those calls return.
//!
//! An iterator built by a modelled adapter is the outside type `Iter<item, sources>`
//! ([`iter`]): its item type, and a hidden value of everything it was built from, with each
//! closure the model already called wrapped as `Applied<closure>` ([`APPLIED`]), so a call that
//! runs the iterator calls that closure again with no new argument values: the model already
//! gave it the values the iterator hands it.
//!
//! Guarantees:
//!
//! - **What the library may run is never less than the model says.** An unmodelled call runs
//!   everything it may reach, by parametricity: code that does not know the kernel's types can
//!   reach kernel code only through the values it is handed and the trait bounds on them
//!   (J. C. Reynolds, "Types, abstraction and parametric polymorphism", IFIP 1983; P. Wadler,
//!   "Theorems for free!", FPCA 1989, <https://doi.org/10.1145/99370.99404>). A modelled call is
//!   modelled from its documented signature: a method whose bounds name no trait the kernel
//!   implements, and that calls no closure, is listed as calling nothing back; one that takes a
//!   closure calls it with exactly the values its signature passes.
//! - **Its result is never narrower than the library's.** A modelled result is the documented
//!   result type with the input types put in; anything else is a hidden value of the inputs'
//!   parts.
//!
//! Method: the signatures and bounds of the standard library's documentation
//! (<https://doc.rust-lang.org/std/option/enum.Option.html>,
//! <https://doc.rust-lang.org/std/result/enum.Result.html>,
//! <https://doc.rust-lang.org/std/vec/struct.Vec.html>,
//! <https://doc.rust-lang.org/std/primitive.slice.html>,
//! <https://doc.rust-lang.org/std/iter/trait.Iterator.html>,
//! <https://doc.rust-lang.org/std/cmp/index.html>, <https://doc.rust-lang.org/std/mem/index.html>),
//! read under parametricity as above.
//!
//! Cost: each lookup is a match on the method's name and the receiver's head; no recursion.

use std::collections::BTreeSet;

use crate::recursion::ty::{Ty, Val};

/// The name of the outside type that wraps a closure an iterator's adapter already called.
pub const APPLIED: &str = "Applied";

/// The name of the model's iterator type.
pub const ITER: &str = "Iter";

/// The traits of the iterator protocol, which a call that runs an iterator invokes on the kernel
/// iterators it was built from.
pub const ITERATOR_TRAITS: [&str; 4] = [
    "Iterator",
    "IntoIterator",
    "DoubleEndedIterator",
    "ExactSizeIterator",
];

/// The comparison traits.
pub const COMPARE: [&str; 4] = ["PartialEq", "Eq", "PartialOrd", "Ord"];

/// What one call into the standard library does, as far as the call graph needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    /// The result, where the model knows it.
    pub ret: Option<Val>,
    /// For arguments that are closures the library calls: the argument's place (the receiver is
    /// 0 for a method; the first argument is 0 for a function) and the values it is called with.
    pub apply: Vec<(usize, Vec<Val>)>,
    /// The outside traits the call may invoke on kernel types among its inputs' parts: `None`
    /// for every one.
    pub traits: Option<Vec<&'static str>>,
    /// Whether the call runs the iterators among its inputs (their `next`, and the closures they
    /// hold).
    pub consumes: bool,
    /// For a conversion, the trait and method whose kernel implementation on the result type
    /// builds the result.
    pub conversion: Option<(&'static str, &'static str)>,
    /// Whether this is [`Model::unknown`]: every closure among the inputs' parts may be called,
    /// with values built from the inputs.
    pub unknown: bool,
}

impl Model {
    /// A call the model does not know: it may run everything it is handed.
    #[must_use]
    pub fn unknown() -> Model {
        Model {
            ret: None,
            apply: Vec::new(),
            traits: None,
            consumes: true,
            conversion: None,
            unknown: true,
        }
    }

    /// A call that calls nothing back and returns `ret`.
    fn pure(ret: Val) -> Model {
        Model {
            ret: Some(ret),
            apply: Vec::new(),
            traits: Some(Vec::new()),
            consumes: false,
            conversion: None,
            unknown: false,
        }
    }

    /// The same call, also invoking `traits` on its inputs' kernel types.
    fn invoking(mut self, traits: &[&'static str]) -> Model {
        let mut list = self.traits.take().unwrap_or_default();
        list.extend_from_slice(traits);
        self.traits = Some(list);
        self
    }

    /// The same call, calling the closure at argument place `at` with `params`.
    fn applying(mut self, at: usize, params: Vec<Val>) -> Model {
        self.apply.push((at, params));
        self
    }

    /// The same call, running the iterators among its inputs.
    fn consuming(mut self) -> Model {
        self.consumes = true;
        self
    }
}

/// The model's iterator of `item`, built from `sources` (each closure the adapter called is to be
/// wrapped in [`APPLIED`] by the caller of this function).
#[must_use]
pub fn iter(item: Ty, sources: BTreeSet<Ty>) -> Ty {
    Ty::ext(ITER, vec![item, Ty::Hidden(sources)])
}

/// The item type of iterating over a value of type `ty` (`IntoIterator`), for the outside types
/// the model knows: its iterator, `Vec`, slices, arrays, ranges, `Option`, `Result` and the
/// standard collections; `None` for others.
#[must_use]
pub fn item_of(ty: &Ty) -> Option<Ty> {
    match ty.peel() {
        Ty::Ext(name, args) => match (name.as_str(), args.as_slice()) {
            (ITER, [item, ..]) => Some(item.clone()),
            (
                "Vec" | "VecDeque" | "BTreeSet" | "BinaryHeap" | "LinkedList" | "Range"
                | "RangeInclusive" | "RangeFrom" | "Option" | "IntoIter" | "Box",
                [t, ..],
            ) => Some(t.clone()),
            ("Result", [t, _]) => Some(t.clone()),
            ("BTreeMap", [k, v]) => Some(Ty::Tuple(vec![k.clone(), v.clone()])),
            _ => None,
        },
        Ty::Slice(t) | Ty::Array(t) => Some((**t).clone()),
        _ => None,
    }
}

/// The sources of an iterator value of the model ([`iter`]), or `None` for another type.
#[must_use]
pub fn sources_of(ty: &Ty) -> Option<&BTreeSet<Ty>> {
    match ty.peel() {
        Ty::Ext(name, args) if name == ITER => match args.get(1) {
            Some(Ty::Hidden(s)) => Some(s),
            _ => None,
        },
        _ => None,
    }
}

/// One type for the alternatives of `v`: the single one, or a hidden value of all their parts.
#[must_use]
pub fn one(v: &Val) -> Ty {
    if v.0.len() == 1
        && let Some(t) = v.0.iter().next()
    {
        return t.clone();
    }
    Ty::Hidden(v.parts())
}

/// The inner type of an `Option` or `Result` value, for `filter_map`, `and_then` and kin.
fn inner(v: &Val) -> Ty {
    let mut out = Val::none();
    for t in &v.0 {
        match t.peel() {
            Ty::Ext(name, args) if name == "Option" || name == "Result" => {
                if let Some(a) = args.first() {
                    out.add(a.clone());
                }
            }
            other => {
                out.add(Ty::Hidden(other.parts()));
            }
        }
    }
    one(&out)
}

/// The item type of iterating each alternative of `v`, or a hidden value of its parts.
fn items(v: &Val) -> Ty {
    let mut out = Val::none();
    for t in &v.0 {
        match item_of(t) {
            Some(i) => {
                out.add(i);
            }
            None => {
                out.add(Ty::Hidden(t.parts()));
            }
        }
    }
    one(&out)
}

/// The parts of every value in `vals`.
fn parts_of(vals: &[&Val]) -> BTreeSet<Ty> {
    let mut out = BTreeSet::new();
    for v in vals {
        out.extend(v.parts());
    }
    out
}

/// What `receiver.name(args)` does when `receiver` (references peeled) is an outside type the
/// model knows; `rets` gives what the callables in a value return. `None` when the model does not
/// know the method on that type.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one arm per modelled method, each one line")]
pub fn method(receiver: &Ty, name: &str, args: &[Val], rets: &dyn Fn(&Val) -> Val) -> Option<Model> {
    let empty = Val::none();
    let a0 = args.first().unwrap_or(&empty);
    let a1 = args.get(1).unwrap_or(&empty);
    let r = Val::of(receiver.clone());
    let bool_ = Val::prim("bool");
    let usize_ = Val::prim("usize");
    let unit = Val::unit();
    let option = |t: Ty| Val::of(Ty::ext("Option", vec![t]));
    match receiver.peel() {
        Ty::Ext(kind, targs) if kind == "Option" => {
            let t = targs.first().cloned().unwrap_or_else(Ty::unknown);
            let tv = Val::of(t.clone());
            Some(match name {
                "is_some" | "is_none" => Model::pure(bool_),
                "unwrap" | "expect" | "unwrap_unchecked" => Model::pure(tv),
                "unwrap_or" => Model::pure(tv.or(a0)),
                "unwrap_or_else" => Model::pure(tv.or(&rets(a0))).applying(1, Vec::new()),
                "unwrap_or_default" => Model::pure(tv).invoking(&["Default"]),
                "map" => Model::pure(option(one(&rets(a0)))).applying(1, vec![tv]),
                "and_then" => Model::pure(rets(a0)).applying(1, vec![tv]),
                "filter" => Model::pure(r).applying(1, vec![Val::of(Ty::Ref(Box::new(t)))]),
                "ok_or" => Model::pure(Val::of(Ty::ext("Result", vec![t, one(a0)]))),
                "ok_or_else" => {
                    Model::pure(Val::of(Ty::ext("Result", vec![t, one(&rets(a0))]))).applying(1, Vec::new())
                }
                "as_ref" | "as_mut" => Model::pure(option(Ty::Ref(Box::new(t)))),
                "copied" => Model::pure(option(t.peel().clone())),
                "cloned" => Model::pure(option(t.peel().clone())).invoking(&["Clone"]),
                "take" | "replace" => Model::pure(r),
                "or" | "xor" => Model::pure(r.or(a0)),
                "or_else" => Model::pure(r.or(&rets(a0))).applying(1, Vec::new()),
                "map_or" => Model::pure(a0.or(&rets(a1))).applying(2, vec![tv]),
                "map_or_else" => Model::pure(rets(a0).or(&rets(a1)))
                    .applying(1, Vec::new())
                    .applying(2, vec![tv]),
                "is_some_and" | "is_none_or" => Model::pure(bool_).applying(1, vec![tv]),
                "iter" | "iter_mut" | "into_iter" => {
                    Model::pure(Val::of(iter(t, receiver.parts())))
                }
                "get_or_insert_with" => Model::pure(Val::of(Ty::Ref(Box::new(t)))).applying(1, Vec::new()),
                "zip" => Model::pure(option(Ty::Tuple(vec![t, inner(a0)]))),
                _ => return None,
            })
        }
        Ty::Ext(kind, targs) if kind == "Result" => {
            let t = targs.first().cloned().unwrap_or_else(Ty::unknown);
            let e = targs.get(1).cloned().unwrap_or_else(Ty::unknown);
            let tv = Val::of(t.clone());
            let ev = Val::of(e.clone());
            let result = |a: Ty, b: Ty| Val::of(Ty::ext("Result", vec![a, b]));
            Some(match name {
                "is_ok" | "is_err" => Model::pure(bool_),
                "ok" => Model::pure(option(t)),
                "err" => Model::pure(option(e)),
                "unwrap" | "expect" => Model::pure(tv).invoking(&["Debug"]),
                "unwrap_err" | "expect_err" => Model::pure(ev).invoking(&["Debug"]),
                "unwrap_or" => Model::pure(tv.or(a0)),
                "unwrap_or_else" => Model::pure(tv.or(&rets(a0))).applying(1, vec![ev]),
                "unwrap_or_default" => Model::pure(tv).invoking(&["Default"]),
                "map" => Model::pure(result(one(&rets(a0)), e)).applying(1, vec![tv]),
                "map_err" => Model::pure(result(t, one(&rets(a0)))).applying(1, vec![ev]),
                "and_then" => Model::pure(rets(a0)).applying(1, vec![tv]),
                "or_else" => Model::pure(rets(a0)).applying(1, vec![ev]),
                "as_ref" | "as_mut" => {
                    Model::pure(result(Ty::Ref(Box::new(t)), Ty::Ref(Box::new(e))))
                }
                "map_or" => Model::pure(a0.or(&rets(a1))).applying(2, vec![tv]),
                "map_or_else" => Model::pure(rets(a0).or(&rets(a1)))
                    .applying(1, vec![ev])
                    .applying(2, vec![tv]),
                "is_ok_and" => Model::pure(bool_).applying(1, vec![tv]),
                "is_err_and" => Model::pure(bool_).applying(1, vec![ev]),
                "iter" | "into_iter" => Model::pure(Val::of(iter(t, receiver.parts()))),
                "copied" => Model::pure(result(t.peel().clone(), e)),
                "cloned" => Model::pure(result(t.peel().clone(), e)).invoking(&["Clone"]),
                _ => return None,
            })
        }
        Ty::Ext(kind, _) if kind == ITER || kind.starts_with("Range") => {
            iterator(receiver, name, args, rets)
        }
        peeled => {
            let elem = match peeled {
                Ty::Ext(kind, targs) if matches!(kind.as_str(), "Vec" | "VecDeque") => {
                    targs.first().cloned()
                }
                Ty::Slice(t) | Ty::Array(t) => Some((**t).clone()),
                _ => None,
            }?;
            let ev = Val::of(elem.clone());
            let slice = Ty::Slice(Box::new(elem.clone()));
            let src = receiver.parts();
            Some(match name {
                "len" | "capacity" => Model::pure(usize_),
                "is_empty" => Model::pure(bool_),
                "iter" | "iter_mut" | "into_iter" | "drain" => {
                    Model::pure(Val::of(iter(elem, src)))
                }
                "get" | "get_mut" | "get_unchecked" => {
                    Model::pure(option(elem).or(&option(slice)))
                }
                "first" | "last" | "first_mut" | "last_mut" | "pop" => Model::pure(option(elem)),
                "push" | "insert" | "truncate" | "clear" | "reserve" | "shrink_to_fit"
                | "swap" | "reverse" | "rotate_left" | "rotate_right" => Model::pure(unit),
                "try_reserve" | "try_reserve_exact" => Model::pure(Val::of(Ty::ext(
                    "Result",
                    vec![Ty::unit(), Ty::ext("TryReserveError", Vec::new())],
                ))),
                "remove" | "swap_remove" => Model::pure(ev),
                "split_at" | "split_at_mut" => {
                    Model::pure(Val::of(Ty::Tuple(vec![slice.clone(), slice])))
                }
                "split_first" | "split_last" => Model::pure(option(Ty::Tuple(vec![elem, slice]))),
                "chunks" | "chunks_exact" | "windows" | "rchunks" | "chunks_mut"
                | "chunks_exact_mut" => Model::pure(Val::of(iter(slice, src))),
                "as_slice" | "as_mut_slice" => Model::pure(Val::of(slice)),
                "sort_by" | "sort_unstable_by" => {
                    Model::pure(unit).applying(1, vec![ev.clone(), ev])
                }
                "sort_by_key" | "sort_unstable_by_key" | "sort_by_cached_key" => {
                    Model::pure(unit).applying(1, vec![ev]).invoking(&COMPARE)
                }
                "binary_search_by" => Model::pure(Val::of(Ty::ext(
                    "Result",
                    vec![Ty::prim("usize"), Ty::prim("usize")],
                )))
                .applying(1, vec![ev]),
                "binary_search_by_key" => Model::pure(Val::of(Ty::ext(
                    "Result",
                    vec![Ty::prim("usize"), Ty::prim("usize")],
                )))
                .applying(2, vec![ev])
                .invoking(&COMPARE),
                "retain" | "retain_mut" => Model::pure(unit).applying(1, vec![ev]),
                "dedup_by" => Model::pure(unit).applying(1, vec![ev.clone(), ev]),
                "dedup_by_key" => Model::pure(unit).applying(1, vec![ev]).invoking(&COMPARE),
                "contains" | "starts_with" | "ends_with" => Model::pure(bool_).invoking(&COMPARE),
                "dedup" | "sort" | "sort_unstable" => Model::pure(unit).invoking(&COMPARE),
                "binary_search" => Model::pure(Val::of(Ty::ext(
                    "Result",
                    vec![Ty::prim("usize"), Ty::prim("usize")],
                )))
                .invoking(&COMPARE),
                "to_vec" | "clone" | "to_owned" => {
                    Model::pure(Val::of(Ty::ext("Vec", vec![elem]))).invoking(&["Clone"])
                }
                "extend_from_slice" | "resize" | "fill" | "clone_from_slice" => {
                    Model::pure(unit).invoking(&["Clone"])
                }
                "copy_from_slice" => Model::pure(unit),
                "extend" => Model::pure(unit).consuming(),
                _ => return None,
            })
        }
    }
}

/// The iterator methods, on the model's iterator or a range.
#[allow(clippy::too_many_lines, reason = "one arm per modelled method, each one line")]
fn iterator(receiver: &Ty, name: &str, args: &[Val], rets: &dyn Fn(&Val) -> Val) -> Option<Model> {
    let empty = Val::none();
    let a0 = args.first().unwrap_or(&empty);
    let a1 = args.get(1).unwrap_or(&empty);
    let item = item_of(receiver).unwrap_or_else(|| Ty::Hidden(receiver.parts()));
    let iv = Val::of(item.clone());
    let refd = Val::of(Ty::Ref(Box::new(item.clone())));
    let mut src = receiver.parts();
    let applied = |src: &BTreeSet<Ty>, f: &Val| -> BTreeSet<Ty> {
        let mut s = src.clone();
        for c in f.0.iter().filter(|t| t.is_callable()) {
            s.insert(Ty::ext(APPLIED, vec![c.clone()]));
        }
        s
    };
    let it = |item: Ty, src: BTreeSet<Ty>| Val::of(iter(item, src));
    let option = |t: Ty| Val::of(Ty::ext("Option", vec![t]));
    Some(match name {
        "next" | "next_back" | "last" | "nth" | "nth_back" => Model::pure(option(item)).consuming(),
        "count" => Model::pure(Val::prim("usize")).consuming(),
        "len" | "size_hint" | "is_empty" => Model::pure(Val::unknown()),
        "map" | "inspect" => {
            let out = if name == "map" { one(&rets(a0)) } else { item };
            Model::pure(it(out, applied(&src, a0))).applying(1, vec![iv])
        }
        "map_while" | "filter_map" => {
            Model::pure(it(inner(&rets(a0)), applied(&src, a0))).applying(1, vec![iv])
        }
        "flat_map" => Model::pure(it(items(&rets(a0)), applied(&src, a0))).applying(1, vec![iv]),
        "filter" | "skip_while" | "take_while" => {
            Model::pure(it(item, applied(&src, a0))).applying(1, vec![refd])
        }
        "flatten" => Model::pure(it(items(&iv), src)),
        "enumerate" => Model::pure(it(Ty::Tuple(vec![Ty::prim("usize"), item]), src)),
        "zip" => {
            src.extend(a0.parts());
            Model::pure(it(Ty::Tuple(vec![item, items(a0)]), src))
        }
        "chain" => {
            src.extend(a0.parts());
            let joined = iv.or(&Val::of(items(a0)));
            Model::pure(it(one(&joined), src))
        }
        "rev" | "skip" | "take" | "step_by" | "peekable" | "fuse" | "cycle" | "by_ref"
        | "into_iter" | "iter" => Model::pure(it(item, src)),
        "copied" => Model::pure(it(item.peel().clone(), src)),
        "cloned" => Model::pure(it(item.peel().clone(), src)).invoking(&["Clone"]),
        "fold" => {
            let acc = a0.or(&rets(a1));
            Model::pure(acc.clone()).applying(2, vec![acc, iv]).consuming()
        }
        "for_each" => Model::pure(Val::unit()).applying(1, vec![iv]).consuming(),
        "all" | "any" => Model::pure(Val::prim("bool")).applying(1, vec![iv]).consuming(),
        "find" => Model::pure(option(item)).applying(1, vec![refd]).consuming(),
        "find_map" => Model::pure(rets(a0)).applying(1, vec![iv]).consuming(),
        "position" | "rposition" => {
            Model::pure(option(Ty::prim("usize"))).applying(1, vec![iv]).consuming()
        }
        "min" | "max" => Model::pure(option(item)).invoking(&COMPARE).consuming(),
        "min_by_key" | "max_by_key" => Model::pure(option(item))
            .applying(1, vec![refd])
            .invoking(&COMPARE)
            .consuming(),
        "min_by" | "max_by" => Model::pure(option(item))
            .applying(1, vec![refd.clone(), refd])
            .consuming(),
        "eq" | "ne" | "lt" | "le" | "gt" | "ge" | "cmp" | "partial_cmp" => {
            Model::pure(Val::unknown()).invoking(&COMPARE).consuming()
        }
        "contains" => Model::pure(Val::prim("bool")).invoking(&COMPARE),
        "sum" | "product" => Model {
            ret: None,
            apply: Vec::new(),
            traits: Some(vec!["Sum", "Product", "Add", "Mul"]),
            consumes: true,
            conversion: Some(if name == "sum" { ("Sum", "sum") } else { ("Product", "product") }),
            unknown: false,
        },
        "collect" => Model {
            ret: None,
            apply: Vec::new(),
            traits: Some(vec!["FromIterator", "Extend", "Default"]),
            consumes: true,
            conversion: Some(("FromIterator", "from_iter")),
            unknown: false,
        },
        _ => return None,
    })
}

/// What `qualifier::name(args)` does for an outside function the model knows (`qualifier` is the
/// path segment before the name, if any); `None` when it does not know it.
#[must_use]
pub fn function(qualifier: Option<&str>, name: &str, args: &[Val], rets: &dyn Fn(&Val) -> Val) -> Option<Model> {
    let empty = Val::none();
    let a0 = args.first().unwrap_or(&empty);
    let a1 = args.get(1).unwrap_or(&empty);
    let peel = |v: &Val| -> Val {
        let mut out = Val::none();
        for t in &v.0 {
            out.add(t.peel().clone());
        }
        out
    };
    Some(match (qualifier, name) {
        (_, "Some") => Model::pure(Val::of(Ty::ext("Option", vec![one(a0)]))),
        (_, "Ok") => Model::pure(Val::of(Ty::ext("Result", vec![one(a0), Ty::unknown()]))),
        (_, "Err") => Model::pure(Val::of(Ty::ext("Result", vec![Ty::unknown(), one(a0)]))),
        (Some(b @ ("Box" | "Rc" | "Arc")), "new") => Model::pure(Val::of(Ty::ext(b, vec![one(a0)]))),
        (
            Some(c @ ("Vec" | "VecDeque" | "BTreeMap" | "BTreeSet" | "String")),
            "new" | "with_capacity",
        ) => {
            let n = if c == "BTreeMap" { 2 } else { usize::from(c != "String") };
            Model::pure(Val::of(Ty::ext(c, vec![Ty::unknown(); n])))
        }
        (Some("mem"), "swap") => Model::pure(Val::unit()),
        (Some("mem"), "replace") => Model::pure(peel(a0)),
        (Some("mem"), "take") => Model::pure(peel(a0)).invoking(&["Default"]),
        (Some("mem"), "drop") | (None, "drop") => Model::pure(Val::unit()),
        (Some("cmp"), "max" | "min") => Model::pure(a0.or(a1)).invoking(&COMPARE),
        (Some("cmp"), "max_by" | "min_by") => {
            Model::pure(a0.or(a1)).applying(2, vec![a0.or(a1), a0.or(a1)])
        }
        (Some("cmp"), "max_by_key" | "min_by_key") => Model::pure(a0.or(a1))
            .applying(2, vec![a0.or(a1)])
            .invoking(&COMPARE),
        (Some("iter"), "repeat" | "once") => Model::pure(Val::of(iter(one(a0), a0.parts()))),
        (Some("iter"), "empty") => Model::pure(Val::of(iter(Ty::unknown(), BTreeSet::new()))),
        (Some("iter"), "from_fn") => {
            let mut src = a0.parts();
            for c in a0.0.iter().filter(|t| t.is_callable()) {
                src.insert(Ty::ext(APPLIED, vec![c.clone()]));
            }
            Model::pure(Val::of(iter(inner(&rets(a0)), src))).applying(0, Vec::new())
        }
        (Some("iter"), "zip") => {
            let src = parts_of(&[a0, a1]);
            Model::pure(Val::of(iter(Ty::Tuple(vec![items(a0), items(a1)]), src)))
        }
        (Some("array"), "from_fn") => Model::pure(Val::of(Ty::Array(Box::new(one(&rets(a0))))))
            .applying(0, vec![Val::prim("usize")]),
        (Some("hint"), "black_box") | (Some("convert"), "identity") => Model::pure(a0.clone()),
        _ => return None,
    })
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may use native arithmetic, casts, unwrap and indexing"
)]
mod tests {
    use super::*;
    use crate::recursion::ty::{ClosureKey, ItemId, NodeId};

    fn closure(i: usize) -> Ty {
        Ty::Closure(Box::new(ClosureKey {
            owner: NodeId(0),
            subst: Vec::new(),
            index: i,
        }))
    }

    fn foo() -> Ty {
        Ty::Adt(ItemId(1), Vec::new())
    }

    fn bar() -> Ty {
        Ty::Adt(ItemId(2), Vec::new())
    }

    #[test]
    fn option_map_calls_its_closure_with_the_item_and_returns_its_result() {
        let rets = |_: &Val| Val::of(bar());
        let opt = Ty::ext("Option", vec![foo()]);
        let m = method(&opt, "map", &[Val::of(closure(0))], &rets).unwrap();
        assert_eq!(m.apply, vec![(1, vec![Val::of(foo())])]);
        assert_eq!(m.ret, Some(Val::of(Ty::ext("Option", vec![bar()]))));
        assert_eq!(m.traits, Some(Vec::new()), "calls no trait implementation");
    }

    #[test]
    fn iterator_chains_keep_their_items_and_mark_applied_closures() {
        let rets = |_: &Val| Val::of(bar());
        let v = Ty::ext("Vec", vec![foo()]);
        let it = method(&v, "iter", &[], &rets).unwrap().ret.unwrap();
        let it_ty = it.0.iter().next().unwrap().clone();
        assert_eq!(item_of(&it_ty), Some(foo()));
        let mapped = method(&it_ty, "map", &[Val::of(closure(3))], &rets).unwrap();
        assert_eq!(mapped.apply, vec![(1, vec![Val::of(foo())])]);
        let m_ty = mapped.ret.unwrap().0.into_iter().next().unwrap();
        assert_eq!(item_of(&m_ty), Some(bar()));
        let src = sources_of(&m_ty).unwrap();
        assert!(src.contains(&Ty::ext(APPLIED, vec![closure(3)])));
        assert!(src.contains(&foo()), "the iterator remembers what it was built from");
        let collected = method(&m_ty, "collect", &[], &rets).unwrap();
        assert!(collected.consumes);
        assert_eq!(collected.conversion, Some(("FromIterator", "from_iter")));
    }

    #[test]
    fn comparisons_and_clones_invoke_their_traits() {
        let rets = |_: &Val| Val::none();
        let v = Ty::Slice(Box::new(foo()));
        assert_eq!(method(&v, "sort", &[], &rets).unwrap().traits, Some(COMPARE.to_vec()));
        assert_eq!(method(&v, "to_vec", &[], &rets).unwrap().traits, Some(vec!["Clone"]));
        let r = Ty::ext("Result", vec![foo(), bar()]);
        assert_eq!(method(&r, "unwrap", &[], &rets).unwrap().traits, Some(vec!["Debug"]));
        assert!(method(&foo(), "anything", &[], &rets).is_none());
        assert!(Model::unknown().traits.is_none());
    }

    #[test]
    fn functions_build_their_results() {
        let rets = |_: &Val| Val::of(bar());
        let m = function(Some("Box"), "new", &[Val::of(foo())], &rets).unwrap();
        assert_eq!(m.ret, Some(Val::of(Ty::ext("Box", vec![foo()]))));
        let m = function(None, "Some", &[Val::of(foo())], &rets).unwrap();
        assert_eq!(m.ret, Some(Val::of(Ty::ext("Option", vec![foo()]))));
        let m = function(Some("array"), "from_fn", &[Val::of(closure(1))], &rets).unwrap();
        assert_eq!(m.apply, vec![(0, vec![Val::prim("usize")])]);
        assert!(function(Some("somewhere"), "else", &[], &rets).is_none());
    }
}
