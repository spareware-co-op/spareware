//! The recursion check (piece G13; chapter 28.8): every recursive cycle in the crates that ship,
//! each with its depth bound stated in a module header and tested on a small WebAssembly stack.
//!
//! What it does: [`analyse`] reads every crate that ships (the kernel crates under
//! `projects/engines/geometry/crates` and the facade `engine-geometry` and its binding crates,
//! [`sources`]) with `syn` ([`model`], [`resolve`], [`macros`], [`cfg`]), together with a model of
//! the standard library and `libm` written as Rust ([`stdlib`]), and builds the call graph of the
//! program as the compiler copies it: one instance for each instantiation of each generic
//! function, closures as types of their own ([`mono`]), each body type-checked as rustc does it,
//! with inference variables ([`typeck`], over [`index`], [`ty`] and [`infer`]), so every call
//! reaches what the compiler would call. It finds the graph's recursive cycles, named by the
//! functions they run, and the kernel's recursive types ([`graph`]); reads every `Recursion:`
//! statement of the module headers ([`statement`]); and checks each cycle against them
//! ([`check`]): a cycle without a stated bound, a statement that names no cycle or not exactly
//! one, a bound constant that disagrees, a test that is missing or does not reach its cycle, and
//! code the analysis cannot read are all [`Problem`]s. [`stack`] runs the tests on the small
//! stack. The binary `recursion` lists the cycles, checks them, and runs the tests:
//!
//! ```text
//! cargo run -p geom-test --bin recursion -- list             every cycle and recursive type
//! cargo run -p geom-test --bin recursion -- check            the statements against the code
//! cargo run -p geom-test --bin recursion -- gate [--runner wasmtime|node]
//!                                                            check, then the small-stack runs
//! ```
//!
//! Guarantees:
//!
//! - **Every cycle is listed.** The call graph holds every call Rust can make in a build that
//!   ships ([`typeck`] states the routes, [`mono`] the treatment of generic parameters left open,
//!   and [`stdlib`] what the standard library may call back); a strongly connected component of it
//!   with a cycle is a recursive cycle ([`graph`]). Code the analysis cannot read, or whose types
//!   it cannot infer where a call depends on them, is a problem, never skipped, so the check cannot
//!   pass over code it did not see.
//! - **A cycle without a stated bound fails**, and so does a statement that is not true to the
//!   code ([`check`]); CI runs the check in the merge gate (`.github/workflows/kernel-gate.yml`).
//! - **Each stated bound is run** by its test on the small stack, in debug and in release, and the
//!   stack's limits are shown in force by probes ([`stack`]).
//! - **Deterministic**: the same tree gives the same report, in the same order.
//!
//! Method: a whole-program call graph from source by monomorphisation, as rustc's collector builds
//! it from MIR (<https://rustc-dev-guide.rust-lang.org/backend/monomorph.html>), here from `syn`'s
//! syntax trees with name resolution and type inference as rustc's type checker performs them
//! (<https://rustc-dev-guide.rust-lang.org/type-inference.html>); cycles as strongly connected
//! components (Tarjan, "Depth-first search and linear graph algorithms", SIAM J. Computing 1
//! (1972), <https://doi.org/10.1137/0201010>). The rule that input-driven recursion carries a
//! stated bound because wasm32 has no guard page follows solidcore's (G01 evidence 13,
//! `projects/kit/notes/G01-evidence/13-vetting-smlib.md`).
//!
//! Cost: one parse of every file of the crates that ship and of the model, and one type check of
//! each instance's body, again when what it read changes (see [`mono`]); the analysis runs on a
//! thread with [`ANALYSIS_STACK`] bytes of stack, because the checker recurses over syntax trees.

pub mod cfg;
pub mod check;
pub mod graph;
pub mod index;
pub mod infer;
pub mod macros;
pub mod model;
pub mod mono;
pub mod resolve;
pub mod stack;
pub mod statement;
pub mod stdlib;
pub mod ty;
pub mod typeck;

use core::fmt;
use std::collections::BTreeMap;
use std::path::Path;

use crate::map::{CrateKind, crate_kind, repo};
use crate::recursion::graph::{Cycle, TypeCycle};
use crate::recursion::index::Index;
use crate::recursion::model::{CrateSource, Role};
use crate::recursion::statement::Statement;

/// The stack of the thread the analysis runs on: 256 MiB, so no syntax tree of the kernel's is
/// deep enough to overflow it.
pub const ANALYSIS_STACK: usize = 256 * 1024 * 1024;

/// The bound tests' file in each crate.
pub const TESTS_FILE: &str = "tests/recursion.rs";

/// What kind of problem a problem is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    /// Code the analysis cannot read.
    Unread,
    /// A cycle or recursive type without a stated bound.
    Unbounded,
    /// A statement that is malformed or misplaced.
    Statement,
    /// A statement that does not name exactly one cycle.
    Stale,
    /// A bound constant that disagrees with the bound.
    Bound,
    /// A bound's test that is missing, ignored or does not reach its cycle.
    Test,
    /// A failure on the small stack.
    Stack,
}

impl Rule {
    /// The rule's name, as problems print it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Rule::Unread => "Unread",
            Rule::Unbounded => "Unbounded",
            Rule::Statement => "Statement",
            Rule::Stale => "Stale",
            Rule::Bound => "Bound",
            Rule::Test => "Test",
            Rule::Stack => "Stack",
        }
    }
}

/// One problem: where, which rule, and what to do.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Problem {
    /// The file, relative to the workspace root.
    pub file: String,
    /// The line.
    pub line: usize,
    /// The rule.
    pub rule: Rule,
    /// What is wrong and what to write.
    pub what: String,
}

impl Problem {
    /// A problem.
    #[must_use]
    pub fn new(file: &str, line: usize, rule: Rule, what: String) -> Problem {
        Problem {
            file: file.to_owned(),
            line,
            rule,
            what,
        }
    }

    /// Code the analysis cannot read.
    #[must_use]
    pub fn unread(file: &str, line: usize, what: &str) -> Problem {
        Problem::new(file, line, Rule::Unread, what.to_owned())
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {}: {}",
            self.file,
            self.line,
            self.rule.label(),
            self.what
        )
    }
}

/// Everything the check found.
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// Every recursive cycle.
    pub cycles: Vec<Cycle>,
    /// Every recursive type group.
    pub types: Vec<TypeCycle>,
    /// Every statement read.
    pub statements: Vec<Statement>,
    /// For each cycle (by index), the statement bounding it.
    pub bounded: BTreeMap<usize, usize>,
    /// For each recursive type group (by index), its statement.
    pub typed: BTreeMap<usize, usize>,
    /// Every problem, sorted.
    pub problems: Vec<Problem>,
    /// For each crate with bound tests: its package, its tests file and the tests its statements
    /// name.
    pub bound_tests: Vec<(String, String, Vec<String>)>,
    /// How many functions, methods, trait default bodies and derived implementations were read.
    pub functions: usize,
    /// How many instances the call graph holds.
    pub instances: usize,
    /// How many edges it holds.
    pub edges: usize,
}

/// The file reader [`analyse`] uses: owned, so the analysis can run on its own thread.
pub type OwnedReader = Box<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

/// Analyses `sources` read by `read` (with the standard library's model, which [`stdlib`]
/// serves): on a thread with [`ANALYSIS_STACK`] bytes of stack where threads exist, else on this
/// one.
#[must_use]
pub fn analyse(sources: Vec<CrateSource>, read: OwnedReader) -> Report {
    let shared = std::sync::Arc::new((sources, read));
    let inner = std::sync::Arc::clone(&shared);
    #[allow(
        clippy::disallowed_methods,
        reason = "a development tool, not kernel code: the thread only gives the analysis a bigger \
                  stack, and its result does not depend on scheduling"
    )]
    let spawned = std::thread::Builder::new()
        .name("recursion-check".to_owned())
        .stack_size(ANALYSIS_STACK)
        .spawn(move || analyse_here(&inner.0, &*inner.1));
    match spawned {
        Ok(handle) => handle.join().unwrap_or_else(|_| Report {
            problems: vec![Problem::unread("", 0, "the analysis stopped with a panic")],
            ..Report::default()
        }),
        Err(_) => analyse_here(&shared.0, &*shared.1),
    }
}

/// Analyses on the calling thread.
#[must_use]
pub fn analyse_here(
    sources: &[CrateSource],
    read: &(dyn Fn(&str) -> Result<String, String> + Send + Sync),
) -> Report {
    let mut all = stdlib::sources();
    all.extend(sources.iter().cloned());
    let program = model::load(&all, &|p| stdlib::read_model(p).map_or_else(|| read(p), Ok));
    let ix = Index::new(&program);
    let mut problems = program.problems.clone();
    let mono = mono::analyse(&ix);
    problems.extend(mono.all_problems(&ix));
    let cycles = graph::cycles(&ix, &mono);
    let types = graph::type_cycles(&ix);
    let mut statements = Vec::new();
    for c in program
        .crates
        .iter()
        .filter(|c| c.source.role == Role::Ships)
    {
        let prefix = format!("{}/src/", c.source.folder);
        for (file, text) in program.files.iter().filter(|(f, _)| f.starts_with(&prefix)) {
            let (s, p) = statement::read(file, text);
            statements.extend(s);
            problems.extend(p);
        }
    }
    let outcome = check::check(&ix, &mono, &cycles, &types, &statements);
    problems.extend(outcome.problems);
    problems.sort();
    problems.dedup();
    let mut bound_tests: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    for c in program
        .crates
        .iter()
        .filter(|c| c.source.role == Role::BoundTests)
    {
        bound_tests.insert(
            c.source.package.clone(),
            (c.source.root.clone(), Vec::new()),
        );
    }
    for s in &statements {
        let krate = program.crates.iter().find(|c| {
            c.source.role == Role::Ships && s.file.starts_with(&format!("{}/", c.source.folder))
        });
        if let Some(c) = krate
            && let Some((_, tests)) = bound_tests.get_mut(&c.source.package)
        {
            tests.push(s.test.clone());
        }
    }
    let edges = mono
        .edges
        .values()
        .map(BTreeMap::len)
        .fold(0_usize, usize::saturating_add);
    Report {
        cycles,
        types,
        statements,
        bounded: outcome.bounded,
        typed: outcome.typed,
        problems,
        bound_tests: bound_tests
            .into_iter()
            .map(|(package, (file, tests))| (package, file, tests))
            .collect(),
        functions: program.nodes.len(),
        instances: mono.insts.len(),
        edges,
    }
}

/// The crates the check reads in the workspace at `root`: every crate that ships, and each one's
/// `tests/recursion.rs` when it has one.
///
/// # Errors
///
/// A folder or file that cannot be read.
pub fn sources(root: &Path) -> Result<Vec<CrateSource>, String> {
    let input = repo::read(root).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut tests = Vec::new();
    for c in &input.crates {
        if crate_kind(&c.name, &c.folder) == CrateKind::Development {
            continue;
        }
        let lib = format!("{}/src/lib.rs", c.folder);
        let main = format!("{}/src/main.rs", c.folder);
        let root_file = if root.join(&lib).is_file() {
            lib
        } else if root.join(&main).is_file() {
            main
        } else {
            continue;
        };
        out.push(CrateSource {
            package: c.name.clone(),
            lib: c.name.replace('-', "_"),
            folder: c.folder.clone(),
            root: root_file,
            role: Role::Ships,
        });
        let test_file = format!("{}/{TESTS_FILE}", c.folder);
        if root.join(&test_file).is_file() {
            tests.push(CrateSource {
                package: c.name.clone(),
                lib: "recursion".to_owned(),
                folder: c.folder.clone(),
                root: test_file,
                role: Role::BoundTests,
            });
        }
    }
    out.extend(tests);
    Ok(out)
}

/// Analyses the workspace at `root`.
///
/// # Errors
///
/// A folder or file that cannot be read to list the crates.
pub fn analyse_repo(root: &Path) -> Result<Report, String> {
    let sources = sources(root)?;
    let base = root.to_path_buf();
    let read: OwnedReader =
        Box::new(move |p: &str| std::fs::read_to_string(base.join(p)).map_err(|e| e.to_string()));
    Ok(analyse(sources, read))
}
