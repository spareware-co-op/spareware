//! The recursion check's rules: every recursive cycle has a stated depth bound, and every
//! statement is true to the code.
//!
//! What it does: [`check`] matches each statement ([`crate::recursion::statement`]) to the cycle
//! or recursive type it names, and reports:
//!
//! - **Unbounded**: a cycle, or recursive type, that no statement names, with a shortest path
//!   round it and the line of each call, and the statement to write.
//! - **Stale**: a statement whose names are not exactly one cycle's members (a member missing, an
//!   extra name, a name that is no function, or no cycle at all), with the cycle's actual
//!   members when the names reach one; a second statement of one cycle.
//! - **Statement**: a statement in a file that holds none of its cycle's functions (or types).
//! - **Bound**: a constant named in `Bound:` that is not a constant of the crate, or whose value
//!   is not the bound.
//! - **Test**: a test that is not a `#[test]` function of the crate's `tests/recursion.rs`, is
//!   `#[ignore]`d, or does not reach the cycle in the call graph.
//!
//! A cycle whose members are all implementations `#[derive]` writes for the types of a stated
//! recursive type is covered by that type's statement.
//!
//! Guarantees:
//!
//! - **Every cycle is bounded or reported**, and every statement is checked against the code it
//!   names, so a bound cannot outlive its cycle or silently cover a cycle that grew.
//! - **Names are resolved as the code resolves them**: relative to the statement's module, from
//!   `crate::`, or through its `use` declarations ([`crate::recursion::resolve`]).
//! - **Deterministic.** Problems come in statement and cycle order.
//!
//! Method: matching of named sets against strongly connected components; reachability by
//! breadth-first search (Cormen, Leiserson, Rivest and Stein, *Introduction to Algorithms*, 3rd
//! edition, section 22.2); the module-header grammar of `projects/engines/geometry/HEADERS.md`.
//!
//! Cost: linear in statements times cycles, plus one breadth-first search of the call graph per
//! test.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::recursion::graph::{Cycle, TypeCycle};
use crate::recursion::index::Index;
use crate::recursion::model::{ItemKind, ModId, NodeKind, Role};
use crate::recursion::resolve::{Ns, Res};
use crate::recursion::statement::{Kind, Statement};
use crate::recursion::mono::{InstId, InstKind, Mono};
use crate::recursion::ty::NodeId;
use crate::recursion::{Problem, Rule};

/// What [`check`] finds.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    /// Every problem.
    pub problems: Vec<Problem>,
    /// For each cycle (by index), the statement that bounds it, if any.
    pub bounded: BTreeMap<usize, usize>,
    /// For each recursive type group (by index), its statement, if any.
    pub typed: BTreeMap<usize, usize>,
}

/// The module whose header a file holds: the first module read from it.
fn module_of_file(ix: &Index<'_>, file: &str) -> Option<ModId> {
    ix.program
        .modules
        .iter()
        .position(|m| m.file == file && m.block_of.is_none())
        .map(ModId)
}

/// The names a statement's `name` may mean, as the call graph names nodes.
fn candidates(ix: &Index<'_>, module: ModId, name: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let r = &ix.resolver;
    let program = ix.program;
    let scope = crate::recursion::model::Scope {
        module,
        generics: Vec::new(),
        consts: Vec::new(),
        self_ref: crate::recursion::model::SelfRef::None,
    };
    if name.starts_with('<') {
        if let Ok(p) = syn::parse_str::<syn::ExprPath>(name)
            && let Some(q) = &p.qself
        {
            let ty = r.ty(&scope, &q.ty);
            let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
            if let (Some(tr), Some(m)) = (segs.get(q.position.saturating_sub(1)), segs.last()) {
                out.insert(format!("<{} as {tr}>::{m}", ix.show(&ty)));
            }
        }
        return out;
    }
    let segs: Vec<String> = name.split("::").map(str::to_owned).collect();
    let krate = program.module(module).krate;
    let lib = program
        .crates
        .get(krate)
        .map(|c| c.source.lib.clone())
        .unwrap_or_default();
    out.insert(name.to_owned());
    if let Some(rest) = name.strip_prefix("crate::") {
        out.insert(format!("{lib}::{rest}"));
    }
    out.insert(format!("{}::{name}", program.module(module).path));
    if let Some((last, prefix)) = segs.split_last() {
        if prefix.is_empty() {
            for res in r.resolve(&scope, &segs, false, Ns::Value) {
                if let Res::Fn(n) = res {
                    out.insert(ix.name(n).to_owned());
                }
            }
        } else {
            let mut found = r.resolve(&scope, prefix, false, Ns::Type);
            found.extend(r.resolve(&scope, prefix, false, Ns::Value));
            for res in found {
                match res {
                    Res::Mod(m) => {
                        out.insert(format!("{}::{last}", program.module(m).path));
                    }
                    Res::Item(id) => {
                        let ty = r.item_ty(id);
                        let shown = if matches!(program.item(id).kind, ItemKind::Alias { .. }) {
                            ix.show(&ty)
                        } else {
                            program.item(id).path.clone()
                        };
                        out.insert(format!("{shown}::{last}"));
                    }
                    Res::Fn(n) => {
                        out.insert(format!("{}::{last}", ix.name(n)));
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

/// The type paths a `Types:` name may mean.
fn type_candidates(ix: &Index<'_>, module: ModId, name: &str) -> BTreeSet<String> {
    let scope = crate::recursion::model::Scope {
        module,
        generics: Vec::new(),
        consts: Vec::new(),
        self_ref: crate::recursion::model::SelfRef::None,
    };
    let segs: Vec<String> = name.split("::").map(str::to_owned).collect();
    let mut out: BTreeSet<String> = ix
        .resolver
        .resolve(&scope, &segs, false, Ns::Type)
        .into_iter()
        .filter_map(|res| match res {
            Res::Item(id) => Some(ix.program.item(id).path.clone()),
            _ => None,
        })
        .collect();
    out.insert(name.to_owned());
    out
}

/// Whether `names`, each with its candidates, are exactly `members`.
fn names_exactly(names: &[BTreeSet<String>], members: &BTreeSet<String>) -> bool {
    let mut covered = BTreeSet::new();
    for c in names {
        let hit: Vec<&String> = c.intersection(members).collect();
        if hit.is_empty() {
            return false;
        }
        covered.extend(hit);
    }
    covered.len() == members.len()
}

/// The value of an integer constant's initialiser, when it is a literal (in parentheses or
/// cast with `as`, at most).
fn literal_value(e: &syn::Expr) -> Option<u128> {
    match e {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(i),
            ..
        }) => i.base10_parse::<u128>().ok(),
        syn::Expr::Paren(p) => literal_value(&p.expr),
        syn::Expr::Group(g) => literal_value(&g.expr),
        syn::Expr::Cast(c) => literal_value(&c.expr),
        _ => None,
    }
}

/// The `#[test]` functions of a test file, and which of them are `#[ignore]`d.
fn tests_in(text: &str) -> Option<BTreeMap<String, bool>> {
    let file = syn::parse_file(text).ok()?;
    let mut out = BTreeMap::new();
    for item in &file.items {
        if let syn::Item::Fn(f) = item {
            let attrs = crate::recursion::cfg::effective(&f.attrs);
            let is_test = attrs.iter().any(|m| m.path().is_ident("test"));
            let ignored = attrs.iter().any(|m| m.path().is_ident("ignore"));
            if is_test && crate::recursion::cfg::ships(&f.attrs) {
                out.insert(f.sig.ident.to_string(), ignored);
            }
        }
    }
    Some(out)
}

/// Whether the call graph leads from `from` to any of `to`.
fn reaches(mono: &Mono, from: InstId, to: &BTreeSet<InstId>) -> bool {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([from]);
    while let Some(n) = queue.pop_front() {
        if to.contains(&n) {
            return true;
        }
        for next in mono.edges.get(&n).into_iter().flat_map(BTreeMap::keys) {
            if seen.insert(*next) {
                queue.push_back(*next);
            }
        }
    }
    false
}

/// Writes how to state a cycle, for a problem message.
fn suggestion(members: &BTreeSet<String>, kind: Kind) -> String {
    let label = match kind {
        Kind::Cycle => "Cycle",
        Kind::Types => "Types",
    };
    let names: Vec<String> = members.iter().map(|m| format!("`{m}`")).collect();
    format!(
        "state its depth bound in the module header: `Recursion:` then `- {label}: {}.`, \
         `Bound: <n>`, `Why: <why the bound holds>`, `Test: <a test in tests/recursion.rs>`",
        names.join(", ")
    )
}

/// Checks every statement against the cycles and recursive types of the code.
#[must_use]
#[allow(clippy::too_many_lines, reason = "the rules in their written order; each is short")]
pub fn check(
    ix: &Index<'_>,
    mono: &Mono,
    cycles: &[Cycle],
    types: &[TypeCycle],
    statements: &[Statement],
) -> Outcome {
    let mut out = Outcome::default();
    let program = ix.program;
    for (si, s) in statements.iter().enumerate() {
        let problem = |rule: Rule, what: String| Problem::new(&s.file, s.line, rule, what);
        let Some(module) = module_of_file(ix, &s.file) else {
            out.problems.push(problem(Rule::Statement, "a statement in a file the check did not read".to_owned()));
            continue;
        };
        match s.kind {
            Kind::Cycle => {
                let names: Vec<BTreeSet<String>> = s.names.iter().map(|n| candidates(ix, module, n)).collect();
                let matched = cycles.iter().position(|c| names_exactly(&names, &c.members));
                let Some(ci) = matched else {
                    let near: Vec<&Cycle> = cycles
                        .iter()
                        .filter(|c| names.iter().any(|n| n.intersection(&c.members).next().is_some()))
                        .collect();
                    let what = match near.first() {
                        Some(c) => format!(
                            "these names are not exactly the cycle's members; the cycle is: {}",
                            c.members.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", ")
                        ),
                        None => "these names are no recursive cycle: delete the statement, or fix the names \
                                 (functions as the module names them, closures by their function)"
                            .to_owned(),
                    };
                    out.problems.push(problem(Rule::Stale, what));
                    continue;
                };
                if let Some(first) = out.bounded.insert(ci, si)
                    && let Some(f) = statements.get(first)
                {
                    out.problems.push(problem(
                        Rule::Stale,
                        format!("this cycle is stated already, at {}:{}", f.file, f.line),
                    ));
                }
                let Some(cycle) = cycles.get(ci) else {
                    continue;
                };
                let files: BTreeSet<&str> = cycle
                    .member_nodes
                    .iter()
                    .filter_map(|n| program.node(*n).map(|g| g.file.as_str()))
                    .collect();
                if !files.contains(s.file.as_str()) {
                    out.problems.push(problem(
                        Rule::Statement,
                        format!(
                            "state the cycle in the header of a module that holds one of its functions: {}",
                            files.into_iter().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                check_test(ix, mono, s, Some(&cycle.insts), &mut out.problems);
            }
            Kind::Types => {
                let names: Vec<BTreeSet<String>> = s.names.iter().map(|n| type_candidates(ix, module, n)).collect();
                let matched = types.iter().position(|t| names_exactly(&names, &t.members));
                let Some(ti) = matched else {
                    let near = types
                        .iter()
                        .find(|t| names.iter().any(|n| n.intersection(&t.members).next().is_some()));
                    let what = match near {
                        Some(t) => format!(
                            "these names are not exactly the recursive types; they are: {}",
                            t.members.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", ")
                        ),
                        None => "these names are no recursive types: delete the statement or fix the names".to_owned(),
                    };
                    out.problems.push(problem(Rule::Stale, what));
                    continue;
                };
                if let Some(first) = out.typed.insert(ti, si)
                    && let Some(f) = statements.get(first)
                {
                    out.problems.push(problem(
                        Rule::Stale,
                        format!("these types are stated already, at {}:{}", f.file, f.line),
                    ));
                }
                let files: BTreeSet<String> = types
                    .get(ti)
                    .map(|t| {
                        t.items
                            .iter()
                            .map(|i| program.module(program.item(*i).module).file.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                if !files.contains(&s.file) {
                    out.problems.push(problem(
                        Rule::Statement,
                        format!(
                            "state the types in the header of a module that defines one of them: {}",
                            files.into_iter().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                check_test(ix, mono, s, None, &mut out.problems);
            }
        }
        if let Some(c) = &s.constant {
            let scope = crate::recursion::model::Scope {
                module,
                generics: Vec::new(),
                consts: Vec::new(),
                self_ref: crate::recursion::model::SelfRef::None,
            };
            let segs: Vec<String> = c.split("::").map(str::to_owned).collect();
            let mut value = None;
            for res in ix.resolver.resolve(&scope, &segs, false, Ns::Value) {
                if let Res::Item(id) = res
                    && let ItemKind::Const { init: Some(init), .. } = &program.item(id).kind
                {
                    value = literal_value(init);
                }
            }
            match value {
                Some(v) if v == u128::from(s.bound) => {}
                Some(v) => out.problems.push(problem(
                    Rule::Bound,
                    format!("`{c}` is {v}, not the stated bound {}", s.bound),
                )),
                None => out.problems.push(problem(
                    Rule::Bound,
                    format!(
                        "`{c}` is not a constant item of the crate (`const NAME: T = <integer literal>;`)"
                    ),
                )),
            }
        }
    }
    // Every cycle and recursive type must be stated.
    let stated_types: BTreeSet<String> = out
        .typed
        .keys()
        .filter_map(|t| types.get(*t))
        .flat_map(|t| t.members.iter().cloned())
        .collect();
    for (ci, c) in cycles.iter().enumerate() {
        if out.bounded.contains_key(&ci) {
            continue;
        }
        let derived_of_stated = c.member_nodes.iter().all(|n| match program.node(*n).map(|x| &x.kind) {
            Some(NodeKind::Derived { item, .. }) => stated_types.contains(&program.item(*item).path),
            _ => false,
        });
        if derived_of_stated && !c.member_nodes.is_empty() {
            continue;
        }
        let (file, line) = c
            .member_nodes
            .iter()
            .filter_map(|n| program.node(*n))
            .map(|g| (g.file.clone(), g.line))
            .next()
            .unwrap_or_default();
        let path: Vec<String> = c
            .path
            .iter()
            .map(|(name, site)| format!("{name} (calls on at {}:{})", site.file, site.line))
            .collect();
        out.problems.push(Problem::new(
            &file,
            line,
            Rule::Unbounded,
            format!(
                "a recursive cycle without a stated depth bound: {}; round it: {}; {}",
                c.members.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", "),
                path.join(" -> "),
                suggestion(&c.members, Kind::Cycle)
            ),
        ));
    }
    for (ti, t) in types.iter().enumerate() {
        if out.typed.contains_key(&ti) {
            continue;
        }
        let (file, line) = t
            .items
            .iter()
            .next()
            .map(|i| {
                let item = program.item(*i);
                (program.module(item.module).file.clone(), item.line)
            })
            .unwrap_or_default();
        out.problems.push(Problem::new(
            &file,
            line,
            Rule::Unbounded,
            format!(
                "recursive types without a stated nesting bound (their drop glue and derived \
                 implementations recurse over them): {}; {}",
                t.members.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", "),
                suggestion(&t.members, Kind::Types)
            ),
        ));
    }
    out
}

/// Checks a statement's test: a `#[test]` of its crate's `tests/recursion.rs`, not ignored,
/// reaching `nodes` when given.
fn check_test(
    ix: &Index<'_>,
    mono: &Mono,
    s: &Statement,
    nodes: Option<&BTreeSet<InstId>>,
    problems: &mut Vec<Problem>,
) {
    let program = ix.program;
    let problem = |what: String| Problem::new(&s.file, s.line, Rule::Test, what);
    let Some(home) = module_of_file(ix, &s.file) else {
        return;
    };
    let krate = program.module(home).krate;
    let Some(package) = program.crates.get(krate).map(|c| c.source.package.clone()) else {
        return;
    };
    let tests = program
        .crates
        .iter()
        .enumerate()
        .find(|(_, c)| c.source.role == Role::BoundTests && c.source.package == package);
    let Some((test_crate, tc)) = tests else {
        problems.push(problem(format!(
            "the crate {package} has no tests/recursion.rs: write `{}` there",
            s.test
        )));
        return;
    };
    let text = program.files.get(&tc.source.root).cloned().unwrap_or_default();
    let Some(found) = tests_in(&text) else {
        problems.push(problem(format!("{} does not parse", tc.source.root)));
        return;
    };
    match found.get(&s.test) {
        None => {
            problems.push(problem(format!(
                "{} has no `#[test] fn {}`",
                tc.source.root, s.test
            )));
            return;
        }
        Some(true) => {
            problems.push(problem(format!(
                "`{}` is `#[ignore]`d, so it never runs on the small stack",
                s.test
            )));
            return;
        }
        Some(false) => {}
    }
    let Some(nodes) = nodes else {
        return;
    };
    let test_node = program
        .nodes
        .iter()
        .position(|n| n.krate == test_crate && n.ident == s.test && matches!(n.kind, NodeKind::Fn))
        .map(NodeId)
        .and_then(|t| mono.find(&InstKind::Node(t, Vec::new())));
    match test_node {
        Some(t) if reaches(mono, t, nodes) => {}
        Some(_) => problems.push(problem(format!(
            "`{}` does not reach the cycle: it must run the cycle at its bound",
            s.test
        ))),
        None => problems.push(problem(format!("`{}` was not read as a function", s.test))),
    }
}
