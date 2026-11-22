//! The files outside every crate's folder that the change selections read by what changed in
//! them, `MAP.json` and `quality-ledger.json` (chapter 28.17; the owner's testing principle 2:
//! "the code map and the mutant ledger count by the crates whose rows changed"), so that a change
//! to one of them reaches only the crates its changed entries name and the crates that read it
//! whole, not every crate as any other file outside every crate does.
//!
//! What it does: [`compare`] takes one shared file's text at the change's base and at its head and
//! says which crates its changed entries name ([`Reach::Crates`]), or that the change cannot be
//! placed ([`Reach::Everything`]). [`Readers::read`] finds, from every workspace crate's Rust
//! sources, which crates read a shared file whole, and through which modules of their library.
//! `test_scope` and `fuzz_scope` use both when the base's text is given (`--base`), and treat the
//! file as one outside every crate when it is not.
//!
//! An entry is one element of one of the file's top-level lists: a crate's row of `MAP.json`
//! (`crates`), one of its kinds, operations or sources; a survivor, `coverage_off` or
//! `awaiting_proof` entry of the ledger. The changed entries are the ones in one text and not in
//! the other, compared whole (a moved entry is unchanged). An entry names a crate when one of its
//! strings is a path inside that crate's folder, or the folder itself (a row's `folder`, an
//! entry's `file`, a proof's path, an evidence file).
//!
//! A crate reads a shared file whole when its sources (library, binaries, tests, benchmarks,
//! examples and build script) hold a string literal that names the file (a doc comment does not
//! count), or a constant defined by such a literal, or name a module of a library that reads it
//! (`geom_test::map`, `use geom_test::{map, ..}`, `geom_test::*`, or the library renamed with
//! `as`). A module of a library reads it when its files hold such a literal or constant, or name
//! another library's reading module, or a reading module of their own library (`crate::`,
//! `super::`, `self::` or a bare `module::`); the crate root's `pub use` lines pass on the names
//! they re-export from a reading module, and any other code in the root that names one makes the
//! whole library a reader. This is found until nothing changes.
//!
//! Guarantees:
//!
//! - **Over, never under.** Every crate an entry names is reached, an entry that names none, a
//!   change to a top-level value that is no list, a text that is not JSON and a missing file reach
//!   everything; a source that does not lex as Rust makes its crate a reader of every shared file.
//!   `tests/test_scope.rs` holds the rule with planted changes.
//! - **Names, not text.** A name in a comment, a doc comment or a string is never a module path:
//!   the sources are read as Rust's tokens.
//! - **Every decision has its reason.** Each reached crate comes with the list it was named in.
//!
//! Limits: a reading module is followed by its name, so a call that reaches one without naming it
//! (a trait it implements for a type defined elsewhere, a macro it exports) is not followed; no
//! reading module of the repository does either (`tests/test_scope.rs` checks the repository's
//! readers are the crates that name the files).
//!
//! Method: JSON (RFC 8259, <https://www.rfc-editor.org/rfc/rfc8259>) read with `serde_json` and
//! compared entry by entry as multisets of their serialised text; Rust's tokens read with
//! `proc-macro2`, as `fuzz_scope` reads `geom-fuzz`.
//!
//! Cost: linear in the two texts; for the readers, linear in the size of the workspace's sources
//! times the number of shared files, once per round of the search, which ends within one round
//! per library module.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use serde_json::Value;

use crate::map::MAP_FILE;
use crate::quality_ledger::LEDGER_PATH;
use crate::test_scope::SOURCE_FOLDERS;

/// The shared files read by content, from the repository root.
pub const SHARED: [&str; 2] = [MAP_FILE, LEDGER_PATH];

/// What a change to one shared file reaches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reach {
    /// The crates its changed entries name, each with why.
    Crates(BTreeMap<String, String>),
    /// Every crate, with why.
    Everything(String),
}

/// The changed shared files of a change, each with what it reaches.
pub type Changes = BTreeMap<String, Reach>;

/// What a change to a shared file reaches, from its text at the base and at the head (`None`:
/// the file is missing there), with `folders` the workspace crates' folders (from the root, each
/// ending in `/`).
#[must_use]
pub fn compare(
    base: Option<&str>,
    head: Option<&str>,
    folders: &BTreeMap<String, String>,
) -> Reach {
    let (Some(base), Some(head)) = (base, head) else {
        return Reach::Everything("it is missing on one side of the change".to_owned());
    };
    let parse = |text: &str| serde_json::from_str::<Value>(text);
    let (Ok(Value::Object(base)), Ok(Value::Object(head))) = (parse(base), parse(head)) else {
        return Reach::Everything("it is not a JSON object on both sides".to_owned());
    };
    let keys: BTreeSet<&String> = base.keys().chain(head.keys()).collect();
    let mut reached = BTreeMap::new();
    for key in keys {
        let (b, h) = (base.get(key), head.get(key));
        if b == h {
            continue;
        }
        let (Some(Value::Array(b)), Some(Value::Array(h))) = (b, h) else {
            return Reach::Everything(format!("its `{key}` changed, and it is no list of entries"));
        };
        for entry in changed_entries(b, h) {
            let named = named_crates(entry, folders);
            if named.is_empty() {
                return Reach::Everything(format!(
                    "a changed entry of its `{key}` names no crate's file"
                ));
            }
            for krate in named {
                reached
                    .entry(krate.clone())
                    .or_insert_with(|| format!("a changed entry of its `{key}` names {krate}"));
            }
        }
    }
    Reach::Crates(reached)
}

/// The entries in one list and not in the other, compared whole, counting repeats.
fn changed_entries<'a>(base: &'a [Value], head: &'a [Value]) -> Vec<&'a Value> {
    let mut count: BTreeMap<String, (Vec<&Value>, Vec<&Value>)> = BTreeMap::new();
    for v in base {
        count.entry(v.to_string()).or_default().0.push(v);
    }
    for v in head {
        count.entry(v.to_string()).or_default().1.push(v);
    }
    let mut out = Vec::new();
    for (mut b, mut h) in count.into_values() {
        while !b.is_empty() && !h.is_empty() {
            b.pop();
            h.pop();
        }
        out.extend(b);
        out.extend(h);
    }
    out
}

/// The crates whose folder holds a path among `entry`'s strings, or is one.
fn named_crates(entry: &Value, folders: &BTreeMap<String, String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![entry];
    while let Some(v) = stack.pop() {
        match v {
            Value::String(s) => {
                let path = s.replace('\\', "/");
                let folder_form = format!("{path}/");
                let owner = folders
                    .iter()
                    .filter(|(_, f)| path.starts_with(f.as_str()) || folder_form == **f)
                    .max_by_key(|(_, f)| f.len());
                if let Some((krate, _)) = owner {
                    out.insert(krate.clone());
                }
            }
            Value::Array(items) => stack.extend(items),
            Value::Object(fields) => stack.extend(fields.values()),
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }
    out
}

/// The reach of every shared file among `changed` (paths from the repository root), comparing its
/// text at the base (`base_text`, `None` when it is missing there) with the text in `repo`.
#[must_use]
pub fn changes(
    changed: &[String],
    base_text: &dyn Fn(&str) -> Option<String>,
    repo: &Path,
    folders: &BTreeMap<String, String>,
) -> Changes {
    let mut out = Changes::new();
    for file in SHARED {
        if !changed.iter().any(|c| c.trim().replace('\\', "/") == file) {
            continue;
        }
        let head = std::fs::read_to_string(repo.join(file)).ok();
        let base = base_text(file);
        out.insert(
            file.to_owned(),
            compare(base.as_deref(), head.as_deref(), folders),
        );
    }
    out
}

/// One crate's sources, as the readers' search needs them.
#[derive(Clone, Debug, Default)]
pub struct CrateSources {
    /// The name its library is used by in code (`geom_test`), if it has one.
    pub lib: Option<String>,
    /// Its Rust files, by path inside its folder (`src/map/repo.rs`, `tests/a.rs`), with their
    /// text.
    pub files: BTreeMap<String, String>,
}

/// Which crates read each shared file whole, and through which names of their library.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Readers {
    /// For each shared file, the crates whose sources read it, any of them.
    all: BTreeMap<String, BTreeSet<String>>,
    /// For each shared file, the crates whose library reads it.
    lib: BTreeMap<String, BTreeSet<String>>,
    /// For each shared file and crate, the files of the crate that read it, by path inside its
    /// folder.
    files: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    /// For each shared file and crate, the library module names (and the root's re-exported
    /// names) that read it; `*` for the whole library.
    names: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
}

/// What a path names after `root ::`, or that `root` is renamed (`root as`).
#[derive(Clone, Debug)]
enum Next {
    /// `root :: name`.
    Name(String),
    /// `root :: { ... }`: every identifier in the group, at any depth.
    Group(BTreeSet<String>),
    /// `root :: *`, or `root as`: every name.
    Every,
}

/// What the search needs of one file, or of one statement of a crate root, read from its tokens
/// once.
#[derive(Clone, Debug, Default)]
struct Summary {
    /// It does not lex as Rust.
    broken: bool,
    /// Its string literals that are not a doc comment's text (`doc = "..."`).
    literals: Vec<String>,
    /// Its identifiers.
    idents: BTreeSet<String>,
    /// Every `root ::` (and `root as`) in it, with what follows.
    paths: Vec<(String, Next)>,
    /// Every constant or static it defines, with the literals of its statement.
    consts: Vec<(String, String)>,
    /// Its first keyword after `pub` and attributes: `use`, `mod` or another item.
    kind: Option<&'static str>,
}

impl Summary {
    /// The summary of `stream`.
    fn of(stream: &TokenStream) -> Self {
        let mut out = Self::default();
        for trees in levels(stream) {
            out.literals.extend(literals(&trees));
            // The name of the constant or static whose statement this is, until its `;`.
            let mut name: Option<String> = None;
            let mut after_keyword = false;
            for (at, t) in trees.iter().enumerate() {
                match t {
                    TokenTree::Ident(i) => {
                        let word = i.to_string();
                        if word == "const" || word == "static" {
                            name = None;
                            after_keyword = true;
                        } else if after_keyword && word != "mut" {
                            // A constant's name is followed by its type: `NAME:` (not `const fn`).
                            let typed = at
                                .checked_add(1)
                                .and_then(|n| trees.get(n))
                                .is_some_and(|n| is_punct(n, ':'));
                            name = typed.then(|| word.clone());
                            after_keyword = false;
                        }
                        if let Some(next) = path_after(&trees, at) {
                            out.paths.push((word.clone(), next));
                        }
                        out.idents.insert(word);
                    }
                    TokenTree::Punct(p) if p.as_char() == ';' => {
                        name = None;
                        after_keyword = false;
                    }
                    TokenTree::Literal(l) => {
                        if let Some(n) = &name {
                            out.consts.push((n.clone(), l.to_string()));
                        }
                    }
                    TokenTree::Punct(_) | TokenTree::Group(_) => {}
                }
            }
        }
        out
    }

    /// The summary of a file's text; a text that does not lex is `broken`.
    fn of_text(text: &str) -> Self {
        text.parse::<TokenStream>().map_or_else(
            |_| Self {
                broken: true,
                ..Self::default()
            },
            |s| Self::of(&s),
        )
    }

    /// Whether it holds a literal naming `base` or names one of `consts`.
    fn names_file(&self, base: &str, consts: &BTreeSet<String>) -> bool {
        self.broken
            || self.literals.iter().any(|l| l.contains(base))
            || self.idents.iter().any(|i| consts.contains(i))
    }

    /// Whether it names a reading name of a library (`libs`: library name to its reading names).
    fn names_reader(&self, libs: &BTreeMap<&str, &BTreeSet<String>>) -> bool {
        self.paths.iter().any(|(root, next)| {
            libs.get(root.as_str())
                .is_some_and(|names| !names.is_empty() && next.names(names))
        })
    }

    /// Whether it names one of its own library's reading `names`: through `crate::`, `super::` or
    /// `self::`, or a bare `name ::`.
    fn names_own(&self, names: &BTreeSet<String>) -> bool {
        self.paths.iter().any(|(root, next)| {
            if ["crate", "super", "self"].contains(&root.as_str()) {
                // A glob of its own crate (`use super::*;` in a tests module) brings names into
                // scope; a call through one is named again where it is made.
                !matches!(next, Next::Every) && next.names(names)
            } else {
                names.contains(root)
            }
        })
    }
}

impl Next {
    /// Whether this names one of `names`, or `names` holds `*` (the whole library); never when
    /// `names` is empty (a glob of a library that reads nothing reads nothing).
    fn names(&self, names: &BTreeSet<String>) -> bool {
        if names.is_empty() {
            return false;
        }
        names.contains("*")
            || match self {
                Self::Name(n) => names.contains(n),
                Self::Group(g) => g.iter().any(|n| names.contains(n)),
                Self::Every => true,
            }
    }
}

/// What follows the identifier at `at` in `trees`, when it starts a path (`at ::`) or is renamed
/// (`at as`).
fn path_after(trees: &[TokenTree], at: usize) -> Option<Next> {
    let after = at.checked_add(1).and_then(|a| trees.get(a..))?;
    if after.first().is_some_and(|t| is_ident(t, "as")) {
        return Some(Next::Every);
    }
    let [a, b, next, ..] = after else {
        return None;
    };
    if !is_punct(a, ':') || !is_punct(b, ':') {
        return None;
    }
    Some(match next {
        TokenTree::Ident(m) => Next::Name(m.to_string()),
        TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => Next::Group(
            levels(&g.stream())
                .into_iter()
                .flatten()
                .filter_map(|t| match t {
                    TokenTree::Ident(i) => Some(i.to_string()),
                    _ => None,
                })
                .collect(),
        ),
        TokenTree::Punct(_) | TokenTree::Group(_) | TokenTree::Literal(_) => Next::Every,
    })
}

/// One crate's files, summarised: each file, and a crate root's statements.
struct Summarised {
    /// Its library name.
    lib: Option<String>,
    /// Each file, by path inside the crate's folder.
    files: BTreeMap<String, Summary>,
    /// The crate root's statements (`src/lib.rs`).
    root: Vec<Summary>,
}

impl Readers {
    /// Reads every workspace crate's sources from `root` (`folders`: crate to folder, from the
    /// root; `libs`: crate to library name) and finds the readers of every shared file.
    ///
    /// # Errors
    ///
    /// A folder or file that cannot be read.
    pub fn read(
        root: &Path,
        folders: &BTreeMap<String, String>,
        libs: &BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let mut sources = BTreeMap::new();
        for (krate, folder) in folders {
            sources.insert(
                krate.clone(),
                CrateSources {
                    lib: libs.get(krate).cloned(),
                    files: read_rust(&root.join(folder))?,
                },
            );
        }
        Ok(Self::new(&sources))
    }

    /// The readers of every shared file in `sources`.
    #[must_use]
    pub fn new(sources: &BTreeMap<String, CrateSources>) -> Self {
        let crates: BTreeMap<String, Summarised> = sources
            .iter()
            .map(|(krate, s)| {
                let root = s
                    .files
                    .get("src/lib.rs")
                    .and_then(|t| t.parse::<TokenStream>().ok())
                    .map(|stream| {
                        statements(&stream)
                            .into_iter()
                            .map(|statement| {
                                let piece: TokenStream = statement.iter().cloned().collect();
                                Summary {
                                    kind: statement_kind(&statement),
                                    ..Summary::of(&piece)
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let files = s
                    .files
                    .iter()
                    .map(|(path, text)| (path.clone(), Summary::of_text(text)))
                    .collect();
                let summarised = Summarised {
                    lib: s.lib.clone(),
                    files,
                    root,
                };
                (krate.clone(), summarised)
            })
            .collect();
        let mut out = Self::default();
        for shared in SHARED {
            let base = shared.rsplit('/').next().unwrap_or(shared);
            let consts = library_constants(&crates, base);
            let names = reading_names(&crates, base, &consts);
            let libs: BTreeMap<&str, &BTreeSet<String>> = crates
                .iter()
                .filter_map(|(k, c)| Some((c.lib.as_deref()?, names.get(k)?)))
                .collect();
            let mut all = BTreeSet::new();
            let mut files_of = BTreeMap::new();
            for (krate, c) in &crates {
                let own = names.get(krate);
                let mine = consts.get(krate).cloned().unwrap_or_default();
                let mut reading = BTreeSet::new();
                for (path, summary) in &c.files {
                    let in_module = module_of(path)
                        .is_some_and(|m| own.is_some_and(|n| n.contains(m) || n.contains("*")));
                    if in_module || summary.names_file(base, &mine) || summary.names_reader(&libs) {
                        reading.insert(path.clone());
                    }
                }
                let lib_reads = own.is_some_and(|n| !n.is_empty());
                if lib_reads || !reading.is_empty() {
                    all.insert(krate.clone());
                }
                files_of.insert(krate.clone(), reading);
            }
            let lib: BTreeSet<String> = names
                .iter()
                .filter(|(_, n)| !n.is_empty())
                .map(|(k, _)| k.clone())
                .collect();
            out.all.insert(shared.to_owned(), all);
            out.lib.insert(shared.to_owned(), lib);
            out.files.insert(shared.to_owned(), files_of);
            out.names.insert(shared.to_owned(), names);
        }
        out
    }

    /// The crates whose sources read `shared`, any of them.
    #[must_use]
    pub fn crates(&self, shared: &str) -> BTreeSet<String> {
        self.all.get(shared).cloned().unwrap_or_default()
    }

    /// Whether `krate`'s library reads `shared`.
    #[must_use]
    pub fn library_reads(&self, krate: &str, shared: &str) -> bool {
        self.lib.get(shared).is_some_and(|l| l.contains(krate))
    }

    /// Whether the file `path` of `krate` (inside its folder) reads `shared` itself, by a literal,
    /// a constant or a reading module it names.
    #[must_use]
    pub fn file_reads(&self, krate: &str, path: &str, shared: &str) -> bool {
        self.files
            .get(shared)
            .and_then(|f| f.get(krate))
            .is_some_and(|f| f.contains(path))
    }

    /// The names of `krate`'s library that read `shared`: its first module names under `src/`,
    /// the names its root re-exports from them, and `*` for the whole library.
    #[must_use]
    pub fn library_names(&self, krate: &str, shared: &str) -> BTreeSet<String> {
        self.names
            .get(shared)
            .and_then(|n| n.get(krate))
            .cloned()
            .unwrap_or_default()
    }

    /// Whether the library module `module` of `krate` (the first name under `src/`) reads
    /// `shared`, or the whole library does.
    #[must_use]
    pub fn module_reads(&self, krate: &str, module: &str, shared: &str) -> bool {
        self.names
            .get(shared)
            .and_then(|n| n.get(krate))
            .is_some_and(|n| n.contains(module) || n.contains("*"))
    }
}

/// Reads a crate's Rust files (its `src/`, `tests/`, `benches/`, `examples/` and `build.rs`), by
/// path inside `folder`.
fn read_rust(folder: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    let mut stack: Vec<std::path::PathBuf> =
        SOURCE_FOLDERS.iter().map(|s| folder.join(s)).collect();
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            let listing =
                std::fs::read_dir(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            for entry in listing {
                stack.push(
                    entry
                        .map_err(|e| format!("{}: {e}", path.display()))?
                        .path(),
                );
            }
        } else if path.extension().is_some_and(|e| e == "rs") && path.is_file() {
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let inside = path
                .strip_prefix(folder)
                .map_err(|e| format!("{}: {e}", path.display()))?
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(inside, text);
        }
    }
    Ok(out)
}

/// Every token of a stream, at every level, in order within each level.
fn levels(stream: &TokenStream) -> Vec<Vec<TokenTree>> {
    let mut out = Vec::new();
    let mut stack = vec![stream.clone()];
    while let Some(s) = stack.pop() {
        let trees: Vec<TokenTree> = s.into_iter().collect();
        for t in &trees {
            if let TokenTree::Group(g) = t {
                stack.push(g.stream());
            }
        }
        out.push(trees);
    }
    out
}

/// Whether a token is the identifier `name`.
fn is_ident(t: &TokenTree, name: &str) -> bool {
    matches!(t, TokenTree::Ident(i) if *i == name)
}

/// Whether a token is the punctuation `c`.
fn is_punct(t: &TokenTree, c: char) -> bool {
    matches!(t, TokenTree::Punct(p) if p.as_char() == c)
}

/// The string literals of one level that are not a doc comment's text (`doc = "..."`).
fn literals(trees: &[TokenTree]) -> impl Iterator<Item = String> + '_ {
    trees.iter().enumerate().filter_map(|(at, t)| {
        let TokenTree::Literal(l) = t else {
            return None;
        };
        let before = at.checked_sub(2).and_then(|b| trees.get(b..at));
        let is_doc = matches!(before, Some([d, eq]) if is_ident(d, "doc") && is_punct(eq, '='));
        (!is_doc).then(|| l.to_string())
    })
}

/// The first module name of a library file (`src/map/repo.rs` and `src/map.rs`: `map`), `lib` for
/// the crate root, or `None` for a file outside the library (a binary, a test, the build script).
fn module_of(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("src/")?;
    if rest.starts_with("bin/") || rest == "main.rs" {
        return None;
    }
    let first = rest.split('/').next()?;
    Some(first.strip_suffix(".rs").unwrap_or(first))
}

/// The crate root's statements: its top-level tokens split at `;`.
fn statements(stream: &TokenStream) -> Vec<Vec<TokenTree>> {
    let mut out = vec![Vec::new()];
    for t in stream.clone() {
        let end = is_punct(&t, ';');
        if let Some(last) = out.last_mut() {
            last.push(t);
        }
        if end {
            out.push(Vec::new());
        }
    }
    out
}

/// Whether a root statement is a `use` or `mod` line (with `pub`, `pub(crate)` and attributes
/// before it), or another item.
fn statement_kind(statement: &[TokenTree]) -> Option<&'static str> {
    statement.iter().find_map(|t| match t {
        TokenTree::Ident(i) if *i == "use" => Some("use"),
        TokenTree::Ident(i) if *i == "mod" => Some("mod"),
        TokenTree::Ident(i) if *i == "pub" => None,
        TokenTree::Punct(p) if p.as_char() == '#' => None,
        TokenTree::Group(_) => None,
        _ => Some("item"),
    })
}

/// The words of a `use` line that name no item: they are never a re-exported name.
const PATH_WORDS: [&str; 10] = [
    "pub", "use", "crate", "self", "super", "as", "in", "std", "core", "alloc",
];

/// The constants and statics each crate's library defines by a literal naming the file `base`,
/// by crate: a name is visible only in its own crate (another crate reaches it by a path through
/// the module that defines it, which then reads the file).
fn library_constants(
    crates: &BTreeMap<String, Summarised>,
    base: &str,
) -> BTreeMap<String, BTreeSet<String>> {
    crates
        .iter()
        .map(|(krate, c)| {
            let names = c
                .files
                .iter()
                .filter(|(path, _)| module_of(path).is_some())
                .flat_map(|(_, f)| &f.consts)
                .filter(|(_, literal)| literal.contains(base))
                .map(|(name, _)| name.clone())
                .collect();
            (krate.clone(), names)
        })
        .collect()
}

/// Every library's reading names for the file `base`, found until nothing changes: by crate, the
/// first module names under `src/` whose files read it, the names the root re-exports from them,
/// and `*` when the root's own code reads it.
fn reading_names(
    crates: &BTreeMap<String, Summarised>,
    base: &str,
    consts: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut names: BTreeMap<String, BTreeSet<String>> = crates
        .keys()
        .map(|k| (k.clone(), BTreeSet::new()))
        .collect();
    loop {
        let libs: BTreeMap<&str, BTreeSet<String>> = crates
            .iter()
            .filter_map(|(k, c)| Some((c.lib.as_deref()?, names.get(k)?.clone())))
            .collect();
        let lib_refs: BTreeMap<&str, &BTreeSet<String>> =
            libs.iter().map(|(k, v)| (*k, v)).collect();
        let mut grew = false;
        for (krate, c) in crates {
            let own = names.get(krate).cloned().unwrap_or_default();
            let mine = consts.get(krate).cloned().unwrap_or_default();
            let reads = |s: &Summary| {
                s.names_file(base, &mine) || s.names_reader(&lib_refs) || s.names_own(&own)
            };
            let mut add = BTreeSet::new();
            for (path, summary) in &c.files {
                // The crate root is read by its statements below, unless it does not lex.
                let module = match module_of(path) {
                    Some("lib") if summary.broken => "*",
                    Some("lib") | None => continue,
                    Some(module) => module,
                };
                if reads(summary) {
                    add.insert(module.to_owned());
                }
            }
            for statement in &c.root {
                if !reads(statement) {
                    continue;
                }
                match statement.kind {
                    // The names it re-exports: its identifiers but the keywords of a path.
                    Some("use") => add.extend(
                        statement
                            .idents
                            .iter()
                            .filter(|i| !PATH_WORDS.contains(&i.as_str()))
                            .cloned(),
                    ),
                    Some("mod") => {}
                    _ => {
                        add.insert("*".to_owned());
                    }
                }
            }
            if let Some(n) = names.get_mut(krate) {
                for a in add {
                    grew |= n.insert(a);
                }
            }
        }
        if !grew {
            return names;
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests may unwrap and index"
)]
mod tests {
    use super::*;

    fn folders() -> BTreeMap<String, String> {
        [
            ("a", "crates/a/"),
            ("b", "crates/b/"),
            ("ab", "crates/a/b/"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into()
    }

    fn crates(reach: &Reach) -> Vec<&str> {
        match reach {
            Reach::Crates(c) => c.keys().map(String::as_str).collect(),
            Reach::Everything(why) => panic!("everything: {why}"),
        }
    }

    #[test]
    fn a_changed_entry_reaches_the_crates_it_names() {
        let base = r#"{"crates": [{"folder": "crates/a", "job": "x"}, {"folder": "crates/b"}],
                       "about": "same"}"#;
        let head = r#"{"crates": [{"folder": "crates/b"}, {"folder": "crates/a", "job": "y"}],
                       "about": "same"}"#;
        assert_eq!(crates(&compare(Some(base), Some(head), &folders())), ["a"]);
        // A moved entry is unchanged.
        let moved = r#"{"crates": [{"folder": "crates/b"}, {"folder": "crates/a", "job": "x"}],
                        "about": "same"}"#;
        assert_eq!(
            crates(&compare(Some(base), Some(moved), &folders())),
            Vec::<&str>::new()
        );
        // The longest folder owns a path; an entry's paths name every crate they are in.
        let added = r#"{"crates": [{"folder": "crates/a", "job": "x"}, {"folder": "crates/b"}],
                        "about": "same", "s": [{"file": "crates/a/b/src/x.rs", "p": ["crates/b/t.rs"]}]}"#;
        let less = r#"{"crates": [{"folder": "crates/a", "job": "x"}, {"folder": "crates/b"}],
                       "about": "same", "s": []}"#;
        assert_eq!(
            crates(&compare(Some(less), Some(added), &folders())),
            ["ab", "b"]
        );
        // A repeated entry counts twice.
        let twice = r#"{"s": [{"file": "crates/a/x"}, {"file": "crates/a/x"}]}"#;
        let once = r#"{"s": [{"file": "crates/a/x"}]}"#;
        assert_eq!(crates(&compare(Some(once), Some(twice), &folders())), ["a"]);
    }

    #[test]
    fn what_cannot_be_placed_reaches_everything() {
        let f = folders();
        let ok = r#"{"about": "x", "s": [{"file": "crates/a/x"}]}"#;
        for (base, head) in [
            (Some(ok), None),
            (None, Some(ok)),
            (Some(ok), Some("not json")),
            (Some(ok), Some("[1]")),
            (
                Some(ok),
                Some(r#"{"about": "y", "s": [{"file": "crates/a/x"}]}"#),
            ),
            (
                Some(ok),
                Some(r#"{"about": "x", "s": [{"file": "elsewhere/x"}]}"#),
            ),
            (
                Some(ok),
                Some(r#"{"about": "x", "s": {"file": "crates/a/x"}}"#),
            ),
        ] {
            assert!(
                matches!(compare(base, head, &f), Reach::Everything(_)),
                "{base:?} -> {head:?}"
            );
        }
    }

    /// A planted crate: its name, its library's name and its files.
    type Planted<'a> = (&'a str, Option<&'a str>, &'a [(&'a str, &'a str)]);

    fn sources(crates: &[Planted<'_>]) -> BTreeMap<String, CrateSources> {
        crates
            .iter()
            .map(|(k, lib, files)| {
                (
                    (*k).to_owned(),
                    CrateSources {
                        lib: lib.map(str::to_owned),
                        files: files
                            .iter()
                            .map(|(p, t)| ((*p).to_owned(), (*t).to_owned()))
                            .collect(),
                    },
                )
            })
            .collect()
    }

    const LEDGER: &str = "projects/engines/geometry/quality-ledger.json";
    const MAP: &str = "projects/engines/geometry/MAP.json";

    #[test]
    fn readers_are_found_by_name_through_modules() {
        let s = sources(&[
            (
                "t",
                Some("t_lib"),
                &[
                    (
                        "src/lib.rs",
                        "pub mod led; pub mod other; pub mod user; pub use led::Ledger;",
                    ),
                    (
                        "src/led.rs",
                        "pub const PATH: &str = \"x/quality-ledger.json\"; pub struct Ledger;",
                    ),
                    (
                        "src/other.rs",
                        "//! Reads quality-ledger.json? No.\n/// Not \"quality-ledger.json\".\npub fn f() { let _ = \"a\"; }",
                    ),
                    ("src/user.rs", "use crate::led::PATH; pub fn g() {}"),
                ],
            ),
            (
                "direct",
                Some("direct"),
                &[("tests/a.rs", "fn main() { let p = \"../../MAP.json\"; }")],
            ),
            (
                "byconst",
                None,
                &[(
                    "tests/a.rs",
                    "use t_lib::led::PATH as P; fn f() { let _ = PATH; }",
                )],
            ),
            (
                "bymodule",
                None,
                &[("tests/a.rs", "use t_lib::{other, user::g};")],
            ),
            (
                "byexport",
                None,
                &[("benches/a.rs", "fn f(_: t_lib::Ledger) {}")],
            ),
            ("byglob", None, &[("tests/a.rs", "use t_lib::*;")]),
            ("renamed", None, &[("tests/a.rs", "use t_lib as tl;")]),
            (
                "nothing",
                None,
                &[(
                    "tests/a.rs",
                    "use t_lib::other::f; // quality-ledger.json\n",
                )],
            ),
            ("broken", None, &[("tests/a.rs", "fn f() { (")]),
            (
                "chain",
                Some("chain"),
                &[
                    ("src/lib.rs", "pub mod via;"),
                    ("src/via.rs", "pub fn h() { t_lib::user::g() }"),
                ],
            ),
            (
                "chained",
                None,
                &[("tests/a.rs", "fn f() { chain::via::h() }")],
            ),
        ]);
        let r = Readers::new(&s);
        let want: BTreeSet<String> = [
            "t", "byconst", "bymodule", "byexport", "byglob", "renamed", "broken", "chain",
            "chained",
        ]
        .map(str::to_owned)
        .into();
        assert_eq!(r.crates(LEDGER), want);
        let map: BTreeSet<String> = ["direct", "broken"].map(str::to_owned).into();
        assert_eq!(r.crates(MAP), map);
        assert!(r.library_reads("t", LEDGER));
        assert!(r.module_reads("t", "led", LEDGER));
        assert!(r.module_reads("t", "user", LEDGER));
        assert!(!r.module_reads("t", "other", LEDGER));
        assert!(r.module_reads("chain", "via", LEDGER));
        assert!(!r.library_reads("direct", MAP), "a test is not the library");
        assert!(r.file_reads("direct", "tests/a.rs", MAP));
        // The root's own code that names a reading module makes the whole library a reader.
        let s = sources(&[(
            "r",
            Some("r"),
            &[
                ("src/lib.rs", "pub mod led; pub fn f() { led::g() }"),
                ("src/led.rs", "pub fn g() { let _ = \"MAP.json\"; }"),
            ],
        )]);
        let r = Readers::new(&s);
        assert!(r.module_reads("r", "anything", MAP));
    }
}
