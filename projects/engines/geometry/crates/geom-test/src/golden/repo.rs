//! The golden store against the repository: every golden is checked where the store says, every
//! golden check in the code is in the store, no golden digest hides outside it, and every pinned
//! file is unchanged.
//!
//! What it does: [`check`] reads the kernel's crates under `projects/engines/geometry/crates`,
//! parses every Rust source of their `src` and `tests` folders as Rust ([`scan`]), and returns every
//! [`Problem`]:
//!
//! - a golden of the current edition whose `check` names a test target with no file, a function the
//!   file does not define, or a function that does not itself call the store's check with the
//!   golden's id;
//! - a golden whose `file` is missing, or whose text no longer has the store's digest (the file was
//!   edited without the store, which would let it change unapproved);
//! - a call of the store's check whose id the current edition does not hold, or whose id cannot be
//!   read (it must be a string literal, or the name of a string `const` of the same file);
//! - the store's `check` imported by itself (`use geom_test::golden::check`, a glob of
//!   `geom_test::golden`, or, inside `geom-test`, a path through `super`), so that a call of it
//!   could not be told from any other function named `check`;
//! - a `const` or `static` whose name holds `GOLDEN` or `DIGEST` and whose value is a string of 64
//!   hexadecimal digits: a golden digest kept outside the store, where no edition, history or
//!   approval rule reaches it;
//! - a source file that is not valid Rust, which this check cannot read and so never passes.
//!
//! A call of the store's check is written `golden::check(<id>, <case>)`, where `golden` is
//! `geom_test::golden` brought into scope by a `use` (under that name or another), or in full,
//! `geom_test::golden::check(..)`, or, inside `geom-test`, `crate::golden::check(..)`. Another
//! crate's own `golden` module, such as `geom-bench`'s, is not the store's, and neither is
//! `golden::check_in`, which takes a store of its own (tests of the store build theirs).
//!
//! Guarantees:
//!
//! - **Code, not text.** Every file is parsed by `syn`, Rust's grammar as the compiler's procedural
//!   macros read it: a call or a constant in a comment, a doc comment or a string literal is never
//!   counted, and a call inside a macro whose arguments are expressions (`assert!`, `println!`,
//!   `vec!`, ...) is.
//! - **The named function checks the golden.** A golden's `check` passes only when its function
//!   (its path through inline modules included) exists in the target's file and calls the store's
//!   check with the golden's id itself, directly or inside a closure, not through a helper.
//! - **It never passes by accident.** A folder it cannot read, a crate with no files, a file it
//!   cannot parse, or a store whose current edition has no golden is a problem, not a pass.
//! - **Every problem names its file**, relative to the workspace root with `/`, and a line where
//!   one applies.
//! - **Deterministic.** Files are read in path order; problems are sorted.
//!
//! Method: a syntax-tree walk with `syn`'s visitor (`syn::visit`), the way `cargo-semver-checks`
//! and clippy read Rust as syntax rather than text; the call and import forms above are this
//! check's written grammar, as the map's headers have theirs (chapter 26.3 and the merge gate of
//! 28.8, `projects/engines/geometry/architecture/28-testing.md`). `syn`: D. Tolnay,
//! <https://github.com/dtolnay/syn> (a dependency, MIT OR Apache-2.0).
//!
//! Cost: linear in the size of the kernel's Rust sources: one parse and one walk per file.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};

use super::sha256::Digest;
use super::store::{Edition, STORE_PATH, Store, text_digest};
use crate::deps::KERNEL_CRATES_DIR;

/// One way the repository and the store disagree.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Problem {
    /// The file concerned, relative to the workspace root with `/`.
    pub file: String,
    /// What is wrong, and what to do.
    pub what: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file, self.what)
    }
}

/// A Rust source file: its path relative to the workspace root with `/`, and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// Its path.
    pub path: String,
    /// Its text.
    pub text: String,
}

/// One call of the store's check in a file.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckCall {
    /// The path, inside the file, of the function the call is in: its inline modules and its name
    /// (`tests::pinned`), or the enclosing function's path and a nested function's name. Empty for
    /// a call outside every function (in a `const` initialiser, say).
    pub function: String,
    /// The golden's id: the string literal, or the value of the string `const` the call names;
    /// `None` when it is neither.
    pub id: Option<String>,
    /// The call's line, from 1.
    pub line: usize,
}

/// What [`scan`] finds in one file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// Every function's path inside the file: inline modules and name (`tests::pinned`); a function
    /// nested in another is under the outer one's path, and a method under `<impl>`.
    pub functions: BTreeSet<String>,
    /// Every call of the store's check, in file order.
    pub checks: Vec<CheckCall>,
    /// Every `const` or `static` whose name holds `GOLDEN` or `DIGEST` and whose value is a string
    /// of 64 hexadecimal digits: its name and line.
    pub stray: Vec<(String, usize)>,
    /// Every import of the store's `check` by itself, and every call of it through `super`: what
    /// was written, and its line.
    pub hidden: Vec<(String, usize)>,
}

/// The module path of a source file inside its crate, from its path relative to the crate folder
/// (`src/golden/shuffle.rs` gives `golden::shuffle`), for `geom-test`'s own library files; `None`
/// for a file that is not part of the library (a binary, a test).
fn library_module(in_crate: &str) -> Option<Vec<String>> {
    let rest = in_crate.strip_prefix("src/")?;
    if rest.starts_with("bin/") || rest == "main.rs" {
        return None;
    }
    let stem = rest.strip_suffix(".rs")?;
    let mut parts: Vec<String> = stem.split('/').map(str::to_owned).collect();
    if parts.last().is_some_and(|p| p == "mod" || p == "lib") {
        parts.pop();
    }
    Some(parts)
}

/// Every path a `use` tree imports, as (path, the name it is known by), with `*` for a glob.
fn use_paths(tree: &syn::UseTree, prefix: &mut Vec<String>, out: &mut Vec<(Vec<String>, String)>) {
    match tree {
        syn::UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            use_paths(&p.tree, prefix, out);
            prefix.pop();
        }
        syn::UseTree::Name(n) => {
            let name = n.ident.to_string();
            if name == "self" {
                let local = prefix.last().cloned().unwrap_or_default();
                out.push((prefix.clone(), local));
            } else {
                let mut path = prefix.clone();
                path.push(name.clone());
                out.push((path, name));
            }
        }
        syn::UseTree::Rename(r) => {
            let name = r.ident.to_string();
            let mut path = prefix.clone();
            if name != "self" {
                path.push(name);
            }
            out.push((path, r.rename.to_string()));
        }
        syn::UseTree::Glob(_) => {
            let mut path = prefix.clone();
            path.push("*".to_owned());
            out.push((path, "*".to_owned()));
        }
        syn::UseTree::Group(g) => {
            for item in &g.items {
                use_paths(item, prefix, out);
            }
        }
    }
}

/// The first argument of a check call, as written.
#[derive(Clone, Debug)]
enum Arg {
    /// A string literal's value.
    Literal(String),
    /// A plain name, to look up among the file's string constants.
    Name(String),
    /// Anything else.
    Other,
}

/// A check call before its id is resolved.
#[derive(Clone, Debug)]
struct RawCall {
    function: String,
    arg: Arg,
    line: usize,
}

/// The walk over one file.
struct Scanner {
    /// The file's library module path, for `geom-test`'s own library files.
    module: Option<Vec<String>>,
    /// Whether the file is in `geom-test`, whose library is `crate` to its own files.
    in_geom_test: bool,
    /// The inline modules around the current item.
    modules: Vec<String>,
    /// The functions around the current item, innermost last, each its full path in the file.
    functions: Vec<String>,
    /// Inside an `impl` block, how deep.
    impls: usize,
    /// Names that `use` binds to `geom_test::golden`.
    aliases: BTreeSet<String>,
    /// Every string constant of the file: name to the values it is given.
    consts: BTreeMap<String, BTreeSet<String>>,
    calls: Vec<RawCall>,
    facts: Facts,
}

/// The line, from 1, where an identifier starts.
fn line_of(ident: &syn::Ident) -> usize {
    ident.span().start().line
}

/// A string literal's value, if `expr` is one.
fn string_literal(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s),
            ..
        }) => Some(s.value()),
        syn::Expr::Group(g) => string_literal(&g.expr),
        syn::Expr::Paren(p) => string_literal(&p.expr),
        _ => None,
    }
}

impl Scanner {
    fn new(in_crate: &str, in_geom_test: bool) -> Scanner {
        Scanner {
            module: if in_geom_test {
                library_module(in_crate)
            } else {
                None
            },
            in_geom_test,
            modules: Vec::new(),
            functions: Vec::new(),
            impls: 0,
            aliases: BTreeSet::new(),
            consts: BTreeMap::new(),
            calls: Vec::new(),
            facts: Facts::default(),
        }
    }

    /// Whether `path` names the golden store's module: `geom_test::golden`, or, inside
    /// `geom-test`, `crate::golden`.
    fn is_store_module(&self, path: &[String]) -> bool {
        match path {
            [a, b] => b == "golden" && (a == "geom_test" || (self.in_geom_test && a == "crate")),
            _ => false,
        }
    }

    /// The full path of a function named `name` here.
    fn function_path(&self, name: &str) -> String {
        let mut parts: Vec<String> = match self.functions.last() {
            Some(outer) => vec![outer.clone()],
            None => self.modules.clone(),
        };
        if self.impls > 0 {
            parts.push("<impl>".to_owned());
        }
        parts.push(name.to_owned());
        parts.join("::")
    }

    fn record_use(&mut self, item: &syn::ItemUse) {
        let mut found = Vec::new();
        use_paths(&item.tree, &mut Vec::new(), &mut found);
        let line = item.use_token.span.start().line;
        for (path, local) in found {
            let leading: Vec<String> = path.iter().filter(|p| *p != "*").cloned().collect();
            if self.is_store_module(&leading) && local != "*" {
                self.aliases.insert(local);
            } else if let [module @ .., last] = path.as_slice() {
                let imports_check = last == "check" || last == "*";
                if imports_check && self.is_store_module(module) {
                    self.facts
                        .hidden
                        .push((format!("use {}", path.join("::")), line));
                }
            }
        }
    }

    fn record_const(&mut self, ident: &syn::Ident, expr: &syn::Expr) {
        let Some(value) = string_literal(expr) else {
            return;
        };
        let name = ident.to_string();
        if (name.contains("GOLDEN") || name.contains("DIGEST"))
            && Digest::from_hex(&value).is_some()
        {
            self.facts.stray.push((name.clone(), line_of(ident)));
        }
        self.consts.entry(name).or_default().insert(value);
    }

    /// Whether a call's path is the store's check, and records a call through `super`.
    fn is_store_check(&mut self, path: &syn::Path) -> bool {
        let segments: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        let Some((last, module)) = segments.split_last() else {
            return false;
        };
        if last != "check" {
            return false;
        }
        if module.first().is_some_and(|s| s == "super") && self.module.is_some() {
            if let Some(first) = path.segments.first() {
                self.facts
                    .hidden
                    .push((segments.join("::"), line_of(&first.ident)));
            }
            return false;
        }
        match module {
            [alias] => self.aliases.contains(alias),
            _ => self.is_store_module(module),
        }
    }
}

impl<'ast> Visit<'ast> for Scanner {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        self.modules.push(item.ident.to_string());
        visit::visit_item_mod(self, item);
        self.modules.pop();
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let path = self.function_path(&item.sig.ident.to_string());
        self.facts.functions.insert(path.clone());
        let impls = core::mem::take(&mut self.impls);
        self.functions.push(path);
        visit::visit_item_fn(self, item);
        self.functions.pop();
        self.impls = impls;
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        self.impls = self.impls.saturating_add(1);
        visit::visit_item_impl(self, item);
        self.impls = self.impls.saturating_sub(1);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let path = self.function_path(&item.sig.ident.to_string());
        self.facts.functions.insert(path.clone());
        let impls = core::mem::take(&mut self.impls);
        self.functions.push(path);
        visit::visit_impl_item_fn(self, item);
        self.functions.pop();
        self.impls = impls;
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.record_use(item);
        visit::visit_item_use(self, item);
    }

    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        self.record_const(&item.ident, &item.expr);
        visit::visit_item_const(self, item);
    }

    fn visit_impl_item_const(&mut self, item: &'ast syn::ImplItemConst) {
        self.record_const(&item.ident, &item.expr);
        visit::visit_impl_item_const(self, item);
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        self.record_const(&item.ident, &item.expr);
        visit::visit_item_static(self, item);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(func) = &*call.func
            && self.is_store_check(&func.path)
        {
            let arg = match call.args.first() {
                Some(first) => match string_literal(first) {
                    Some(value) => Arg::Literal(value),
                    None => match first {
                        syn::Expr::Path(p) => p
                            .path
                            .get_ident()
                            .map_or(Arg::Other, |i| Arg::Name(i.to_string())),
                        _ => Arg::Other,
                    },
                },
                None => Arg::Other,
            };
            let line = func.path.segments.first().map_or(0, |s| line_of(&s.ident));
            self.calls.push(RawCall {
                function: self.functions.last().cloned().unwrap_or_default(),
                arg,
                line,
            });
        }
        visit::visit_expr_call(self, call);
    }

    /// A macro whose arguments are expressions (`assert!(a, "..")`, `vec![..]`, `println!(..)`) is
    /// walked as those expressions; any other macro's tokens are not code this check can read.
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if let Ok(args) =
            mac.parse_body_with(Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated)
        {
            for arg in &args {
                self.visit_expr(arg);
            }
        }
        visit::visit_macro(self, mac);
    }
}

/// Scans one Rust source: its functions, its calls of the store's check, its stray digests and its
/// hidden imports. `in_crate` is the file's path inside its crate folder (`src/golden/shuffle.rs`,
/// `tests/golden.rs`), and `in_geom_test` says whether that crate is `geom-test`.
///
/// # Errors
///
/// `syn`'s message, with its line and column, when the text is not valid Rust.
pub fn scan(text: &str, in_crate: &str, in_geom_test: bool) -> Result<Facts, String> {
    let file = syn::parse_file(text).map_err(|e| {
        let at = e.span().start();
        format!(
            "line {}, column {}: {e}",
            at.line,
            at.column.saturating_add(1)
        )
    })?;
    let mut scanner = Scanner::new(in_crate, in_geom_test);
    scanner.visit_file(&file);
    let Scanner {
        consts,
        calls,
        mut facts,
        ..
    } = scanner;
    for call in calls {
        let id = match call.arg {
            Arg::Literal(value) => Some(value),
            Arg::Name(name) => consts.get(&name).and_then(|values| {
                let mut it = values.iter();
                match (it.next(), it.next()) {
                    (Some(only), None) => Some(only.clone()),
                    _ => None,
                }
            }),
            Arg::Other => None,
        };
        facts.checks.push(CheckCall {
            function: call.function,
            id,
            line: call.line,
        });
    }
    Ok(facts)
}

/// Every `.rs` file under `dir`, in path order, recursively; `Err` with the folder that could not
/// be read.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        paths.push(entry.path());
    }
    paths.sort();
    for path in paths {
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            rust_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// The path of `path` relative to `root`, with `/`.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Reads every Rust source of every kernel crate (its `src` and `tests` folders) under `root`.
///
/// # Errors
///
/// A message naming the folder or file that could not be read.
pub fn kernel_sources(root: &Path) -> Result<Vec<Source>, String> {
    let crates_dir = KERNEL_CRATES_DIR
        .iter()
        .fold(root.to_path_buf(), |p, c| p.join(c));
    let mut crates: Vec<PathBuf> = std::fs::read_dir(&crates_dir)
        .map_err(|e| format!("{}: {e}", crates_dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.join("Cargo.toml").is_file())
        .collect();
    crates.sort();
    if crates.is_empty() {
        return Err(format!("{}: no crate found", crates_dir.display()));
    }
    let mut files = Vec::new();
    for krate in &crates {
        for folder in ["src", "tests"] {
            let dir = krate.join(folder);
            if dir.is_dir() {
                rust_files(&dir, &mut files)?;
            }
        }
    }
    let mut out = Vec::new();
    for path in files {
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        out.push(Source {
            path: relative(root, &path),
            text,
        });
    }
    Ok(out)
}

/// A source's crate and its path inside the crate folder, or `None` for a file outside the
/// kernel's crates folder.
fn crate_and_rest(path: &str) -> Option<(&str, &str)> {
    let base = format!("{}/", KERNEL_CRATES_DIR.join("/"));
    path.strip_prefix(&base)?.split_once('/')
}

/// One source, scanned: its path, and what [`scan`] found in it or why it could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scanned {
    /// The source's path, relative to the workspace root with `/`.
    pub path: String,
    /// What the scan found, or `syn`'s message for a file that is not valid Rust.
    pub facts: Result<Facts, String>,
}

/// Scans one kernel source, telling `geom-test`'s own files (whose library is `crate` to them) from
/// the other crates' by the source's path.
#[must_use]
pub fn scan_source(src: &Source) -> Scanned {
    let (krate, in_crate) = crate_and_rest(&src.path).unwrap_or(("", src.path.as_str()));
    Scanned {
        path: src.path.clone(),
        facts: scan(&src.text, in_crate, krate == "geom-test"),
    }
}

/// The sources that scanned, with their facts; each that did not is a problem.
fn readable<'s>(
    scanned: &'s [Scanned],
    add: &mut impl FnMut(&str, String),
) -> Vec<(&'s str, &'s Facts)> {
    let mut out = Vec::new();
    for s in scanned {
        match &s.facts {
            Ok(facts) => out.push((s.path.as_str(), facts)),
            Err(e) => add(
                &s.path,
                format!("is not Rust this check can parse ({e}), so its goldens cannot be checked"),
            ),
        }
    }
    out
}

/// The file and the function path inside it that a golden's check names: `tests/<test>.rs` or
/// `tests/<test>/main.rs` with the whole function path; or, for `lib`, the deepest module file
/// whose remaining path is a function the file defines.
fn check_target<'f>(
    scanned: &[(&'f str, &'f Facts)],
    krate: &str,
    test: &str,
    function: &str,
) -> Result<(&'f str, &'f Facts, String), String> {
    let base = format!("{}/{krate}", KERNEL_CRATES_DIR.join("/"));
    let find = |path: &str| scanned.iter().find(|(p, _)| *p == path).copied();
    if test == "lib" {
        let parts: Vec<&str> = function.split("::").collect();
        let mut any_file = false;
        for k in (0..parts.len()).rev() {
            let module = parts.get(..k).unwrap_or(&[]);
            let inside = parts.get(k..).unwrap_or(&[]).join("::");
            let candidates: Vec<String> = if module.is_empty() {
                vec![format!("{base}/src/lib.rs")]
            } else {
                let stem = format!("{base}/src/{}", module.join("/"));
                vec![format!("{stem}.rs"), format!("{stem}/mod.rs")]
            };
            for candidate in &candidates {
                if let Some((path, facts)) = find(candidate) {
                    any_file = true;
                    if facts.functions.contains(&inside) {
                        return Ok((path, facts, inside));
                    }
                }
            }
        }
        Err(if any_file {
            format!("no module file of {krate}'s library defines a test function {function}")
        } else {
            format!("{krate} has no library source")
        })
    } else {
        let candidates = [
            format!("{base}/tests/{test}.rs"),
            format!("{base}/tests/{test}/main.rs"),
        ];
        let Some((path, facts)) = candidates.iter().find_map(|c| find(c)) else {
            return Err(format!("{krate} has no test target {test:?}"));
        };
        if facts.functions.contains(function) {
            Ok((path, facts, function.to_owned()))
        } else {
            Err(format!(
                "{path} does not define the test function {function}"
            ))
        }
    }
}

/// Every disagreement between `store` and the kernel's `sources`, with `file_text` reading a pinned
/// file by its path relative to the workspace root.
#[must_use]
pub fn problems(
    store: &Store,
    sources: &[Source],
    file_text: &dyn Fn(&str) -> Result<String, String>,
) -> Vec<Problem> {
    let scanned: Vec<Scanned> = sources.iter().map(scan_source).collect();
    problems_of(store, &scanned, file_text)
}

/// [`problems`] over sources already scanned ([`scan_source`]), so a caller that changes one file
/// rescans only that one.
#[must_use]
pub fn problems_of(
    store: &Store,
    scanned: &[Scanned],
    file_text: &dyn Fn(&str) -> Result<String, String>,
) -> Vec<Problem> {
    let mut found = BTreeSet::new();
    let store_file = STORE_PATH.join("/");
    let mut add = |file: &str, what: String| {
        found.insert(Problem {
            file: file.to_owned(),
            what,
        });
    };
    let Ok(edition) = store.current() else {
        add(&store_file, "the store has no edition".to_owned());
        return found.into_iter().collect();
    };
    if edition.goldens.is_empty() {
        add(&store_file, "the current edition has no golden".to_owned());
    }
    let readable = readable(scanned, &mut add);
    store_side(edition, &readable, file_text, &mut add);
    source_side(edition, &readable, &mut add);
    found.into_iter().collect()
}

/// The goldens' side: each is checked where the store says, and each pinned file keeps its digest.
fn store_side(
    edition: &Edition,
    scanned: &[(&str, &Facts)],
    file_text: &dyn Fn(&str) -> Result<String, String>,
    add: &mut impl FnMut(&str, String),
) {
    let store_file = STORE_PATH.join("/");
    for golden in &edition.goldens {
        if let Some(check) = &golden.check {
            match check_target(scanned, &check.krate, &check.test, &check.function) {
                Err(e) => add(
                    &store_file,
                    format!("golden {}: its check cannot be found: {e}", golden.id),
                ),
                Ok((path, facts, inside)) => {
                    let checks_it = facts
                        .checks
                        .iter()
                        .any(|c| c.function == inside && c.id.as_deref() == Some(&golden.id));
                    if !checks_it {
                        add(
                            path,
                            format!(
                                "the store checks golden {} in function {}, which does not call \
                                 golden::check with that id itself",
                                golden.id, check.function
                            ),
                        );
                    }
                }
            }
        }
        if let Some(file) = &golden.file {
            match file_text(file) {
                Err(e) => add(file, format!("golden {}: cannot read it: {e}", golden.id)),
                Ok(text) => {
                    let digest = text_digest(&text).to_string();
                    if digest != golden.sha256 {
                        add(
                            file,
                            format!(
                                "its text has digest {digest} but golden {} pins {}: this golden \
                                 file changed without the store. Record the change in {store_file} \
                                 (the old digest in the golden's history, with its kind, piece and \
                                 why); a kernel owner approves it",
                                golden.id, golden.sha256
                            ),
                        );
                    }
                }
            }
        }
    }
}

/// The sources' side: every golden check names a golden of the store, no check hides behind an
/// import, and no digest hides outside the store.
fn source_side(edition: &Edition, scanned: &[(&str, &Facts)], add: &mut impl FnMut(&str, String)) {
    let store_file = STORE_PATH.join("/");
    let known: BTreeSet<&str> = edition.goldens.iter().map(|g| g.id.as_str()).collect();
    for (path, facts) in scanned {
        for call in &facts.checks {
            match &call.id {
                None => add(
                    path,
                    format!(
                        "line {}: a golden check whose id is neither a string literal nor the name \
                         of a string const of this file; name the id so the store can be checked \
                         against it",
                        call.line
                    ),
                ),
                Some(id) if !known.contains(id.as_str()) => add(
                    path,
                    format!(
                        "line {}: checks golden {id:?}, which the store's current edition does not \
                         hold; add it to {store_file}",
                        call.line
                    ),
                ),
                Some(_) => {}
            }
        }
        for (written, line) in &facts.hidden {
            add(
                path,
                format!(
                    "line {line}: `{written}` reaches the golden store's check by another path; \
                     write the call `golden::check(..)` with `use geom_test::golden;` (inside \
                     geom-test, `crate::golden::check(..)`), so the store can be checked against it"
                ),
            );
        }
        for (name, line) in &facts.stray {
            add(
                path,
                format!(
                    "line {line}: {name} holds a 64-digit digest; a golden digest lives in the \
                     store ({store_file}), where its edition, history and approval rule reach it"
                ),
            );
        }
    }
}

/// Reads the kernel's sources under `root` and checks them against `store`.
///
/// # Errors
///
/// A message naming a folder or file that could not be read.
pub fn check(root: &Path, store: &Store) -> Result<Vec<Problem>, String> {
    let sources = kernel_sources(root)?;
    let read = |relative: &str| -> Result<String, String> {
        let path = relative
            .split('/')
            .fold(root.to_path_buf(), |p, c| p.join(c));
        std::fs::read_to_string(&path).map_err(|e| e.to_string())
    };
    Ok(problems(store, &sources, &read))
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

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn store(extra: &str) -> Store {
        Store::parse(&format!(
            r#"{{"format": 1, "about": "t", "editions": [{{"edition": "pre-edition",
            "state": "open", "goldens": [{{"id": "geom-x/a", "what": "w", "sha256": "{A}",
            "check": {{"crate": "geom-x", "test": "golden", "function": "it_holds"}}}}{extra}]}}]}}"#
        ))
        .unwrap()
    }

    fn src(path: &str, text: &str) -> Source {
        Source {
            path: format!("projects/engines/geometry/crates/{path}"),
            text: text.to_owned(),
        }
    }

    fn no_files(_: &str) -> Result<String, String> {
        Err("no files here".to_owned())
    }

    const GOOD: &str = "use geom_test::golden;\n#[test]\nfn it_holds() { \
                        golden::check(\"geom-x/a\", || ([0; 32], ())); }";

    fn ids(text: &str) -> Vec<Option<String>> {
        scan(text, "tests/t.rs", false)
            .unwrap()
            .checks
            .into_iter()
            .map(|c| c.id)
            .collect()
    }

    #[test]
    fn ids_are_read_from_literals_and_consts_and_only_code_counts() {
        let text = "use geom_test::golden;\nconst ID: &str = \"geom-x/b\";\n\
                    fn f() { golden::check(\"geom-x/a\", g); golden::check(ID, g); \
                    golden::check(make_id(), g); golden::check_in(&s, \"geom-x/c\", g); }\n\
                    // golden::check(\"geom-x/comment\", g);\n\
                    /// golden::check(\"geom-x/doc\", g);\nfn h() { let s = \"golden::check(\\\"geom-x/s\\\", g)\"; }";
        assert_eq!(
            ids(text),
            [
                Some("geom-x/a".to_owned()),
                Some("geom-x/b".to_owned()),
                None
            ]
        );
    }

    #[test]
    fn only_the_stores_golden_module_counts() {
        // geom-bench's own `golden` module is not the store's.
        assert!(ids("use crate::golden;\nfn f() { golden::check(name, d); }").is_empty());
        assert!(ids("use geom_bench::golden;\nfn f() { golden::check(\"x/y\", d); }").is_empty());
        // In full, renamed, and inside geom-test.
        assert_eq!(
            ids("fn f() { geom_test::golden::check(\"x/y\", d); }").len(),
            1
        );
        assert_eq!(
            ids("use geom_test::golden as g;\nfn f() { g::check(\"x/y\", d); }").len(),
            1
        );
        let inside = scan(
            "fn f() { crate::golden::check(\"x/y\", d); }",
            "src/grid.rs",
            true,
        )
        .unwrap();
        assert_eq!(inside.checks.len(), 1);
        // `crate::golden` outside geom-test is that crate's own module.
        assert!(ids("fn f() { crate::golden::check(\"x/y\", d); }").is_empty());
    }

    #[test]
    fn a_check_inside_an_expression_macro_counts() {
        let text = "use geom_test::golden;\nfn f() { assert!(golden::check(\"x/y\", d).verdict().is_ok()); }";
        assert_eq!(ids(text), [Some("x/y".to_owned())]);
    }

    #[test]
    fn hidden_imports_and_super_paths_are_found() {
        for text in [
            "use geom_test::golden::check;",
            "use geom_test::golden::{check as c, Sha256};",
            "use geom_test::golden::*;",
        ] {
            let facts = scan(text, "tests/t.rs", false).unwrap();
            assert_eq!(facts.hidden.len(), 1, "{text}");
        }
        let via_super = scan(
            "mod tests { fn f() { super::super::check(\"x/y\", d); } }",
            "src/golden/shuffle.rs",
            true,
        )
        .unwrap();
        assert_eq!(via_super.hidden.len(), 1);
        assert!(via_super.checks.is_empty());
        let fine = scan(
            "use geom_test::golden::{self, Sha256, check_in};",
            "tests/t.rs",
            false,
        )
        .unwrap();
        assert!(fine.hidden.is_empty());
    }

    #[test]
    fn functions_are_found_with_their_module_paths() {
        let facts = scan(
            "fn a() { fn inner() {} }\nmod tests { fn b() {} mod deep { fn c() {} } }\n\
             impl X { fn m() {} }",
            "src/x.rs",
            true,
        )
        .unwrap();
        let want: BTreeSet<String> = ["a", "a::inner", "tests::b", "tests::deep::c", "<impl>::m"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(facts.functions, want);
    }

    #[test]
    fn stray_digests_are_found_by_name_and_value() {
        let text = format!(
            "const GOLDEN: &str = \"{A}\";\nconst OTHER: &str = \"{A}\";\n\
             const FNV_GOLDEN: u64 = 0x1;\npub const WORKLOAD_DIGEST: &str = \"{A}\";\n\
             const GOLDEN_NAME: &str = \"geom-x/a\";\nfn f() {{ const INNER_DIGEST: &str = \"{A}\"; }}\n\
             // const COMMENT_GOLDEN: &str = \"{A}\";"
        );
        let facts = scan(&text, "tests/t.rs", false).unwrap();
        assert_eq!(
            facts.stray,
            [
                ("GOLDEN".to_owned(), 1),
                ("WORKLOAD_DIGEST".to_owned(), 4),
                ("INNER_DIGEST".to_owned(), 6)
            ]
        );
    }

    #[test]
    fn a_consistent_tree_has_no_problem() {
        let sources = [src("geom-x/tests/golden.rs", GOOD)];
        assert_eq!(problems(&store(""), &sources, &no_files), []);
    }

    #[test]
    fn every_disagreement_is_found() {
        let cases: Vec<(&str, Vec<Source>)> = vec![
            ("missing test file", vec![]),
            (
                "missing function",
                vec![src(
                    "geom-x/tests/golden.rs",
                    "use geom_test::golden;\nfn other() { golden::check(\"geom-x/a\", f); }",
                )],
            ),
            (
                "function does not check it",
                vec![src(
                    "geom-x/tests/golden.rs",
                    "use geom_test::golden;\nfn it_holds() {}\nfn other() { golden::check(\"geom-x/a\", f); }",
                )],
            ),
            (
                "only a comment checks it",
                vec![src(
                    "geom-x/tests/golden.rs",
                    "fn it_holds() { /* golden::check(\"geom-x/a\", f) */ }",
                )],
            ),
            (
                "unknown id",
                vec![
                    src("geom-x/tests/golden.rs", GOOD),
                    src(
                        "geom-y/tests/t.rs",
                        "use geom_test::golden;\nfn f() { golden::check(\"geom-y/zz\", f); }",
                    ),
                ],
            ),
            (
                "unreadable id",
                vec![
                    src("geom-x/tests/golden.rs", GOOD),
                    src(
                        "geom-y/tests/t.rs",
                        "use geom_test::golden;\nfn f() { golden::check(id(), f); }",
                    ),
                ],
            ),
            (
                "stray digest",
                vec![
                    src("geom-x/tests/golden.rs", GOOD),
                    src(
                        "geom-y/src/lib.rs",
                        &format!("const GOLDEN: &str = \"{A}\";"),
                    ),
                ],
            ),
            (
                "hidden import",
                vec![
                    src("geom-x/tests/golden.rs", GOOD),
                    src("geom-y/tests/t.rs", "use geom_test::golden::check;"),
                ],
            ),
            (
                "not Rust",
                vec![
                    src("geom-x/tests/golden.rs", GOOD),
                    src("geom-y/tests/t.rs", "fn f( {"),
                ],
            ),
        ];
        for (name, sources) in cases {
            let p = problems(&store(""), &sources, &no_files);
            assert!(!p.is_empty(), "{name}: no problem found");
        }
    }

    #[test]
    fn a_pinned_file_must_keep_its_digest() {
        let text = "line one\nline two\n";
        let digest = text_digest(text).to_string();
        let extra = format!(
            r#",{{"id": "geom-z/file", "what": "w", "sha256": "{digest}", "file": "a/golden.txt"}}"#
        );
        let sources = [src("geom-x/tests/golden.rs", GOOD)];
        let same = |_: &str| Ok("line one\r\nline two\r\n".to_owned());
        assert_eq!(problems(&store(&extra), &sources, &same), []);
        let edited = |_: &str| Ok("line one\nline 2\n".to_owned());
        assert_eq!(problems(&store(&extra), &sources, &edited).len(), 1);
        assert_eq!(problems(&store(&extra), &sources, &no_files).len(), 1);
    }

    #[test]
    fn lib_checks_find_their_module_file_and_inline_module() {
        let lib = scan("mod tests { fn pinned() {} }", "src/lib.rs", false).unwrap();
        let shuffle = scan(
            "mod tests { fn pinned() {} }",
            "src/golden/shuffle.rs",
            false,
        )
        .unwrap();
        let scanned = vec![
            ("projects/engines/geometry/crates/geom-x/src/lib.rs", &lib),
            (
                "projects/engines/geometry/crates/geom-x/src/golden/shuffle.rs",
                &shuffle,
            ),
        ];
        let (path, _, inside) =
            check_target(&scanned, "geom-x", "lib", "golden::shuffle::tests::pinned").unwrap();
        assert_eq!(
            path,
            "projects/engines/geometry/crates/geom-x/src/golden/shuffle.rs"
        );
        assert_eq!(inside, "tests::pinned");
        let (root, _, inside) = check_target(&scanned, "geom-x", "lib", "tests::pinned").unwrap();
        assert_eq!(root, "projects/engines/geometry/crates/geom-x/src/lib.rs");
        assert_eq!(inside, "tests::pinned");
        assert!(check_target(&scanned, "geom-x", "lib", "tests::missing").is_err());
        assert!(check_target(&scanned, "geom-x", "det", "f").is_err());
    }

    #[test]
    fn library_modules_follow_cargos_layout() {
        let m = |p: &str| library_module(p).map(|v| v.join("::"));
        assert_eq!(m("src/lib.rs"), Some(String::new()));
        assert_eq!(m("src/golden.rs"), Some("golden".to_owned()));
        assert_eq!(
            m("src/golden/shuffle.rs"),
            Some("golden::shuffle".to_owned())
        );
        assert_eq!(m("src/map/mod.rs"), Some("map".to_owned()));
        assert_eq!(m("src/bin/golden.rs"), None);
        assert_eq!(m("tests/t.rs"), None);
    }
}
