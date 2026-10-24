//! Macros for the recursion check: what code a macro invocation runs.
//!
//! What it does: [`read`] says what an invocation in a function body means for the call graph.
//! The standard library's formatting macros (`format!`, `write!`, `panic!`, `assert_eq!`, ...)
//! become their argument expressions, each with the formatting trait its placeholder calls
//! (`{}` Display, `{:?}` Debug, `{:x}` `LowerHex`, ...), read from the format string by
//! [`format_traits`]; `vec!`, `matches!`, `assert!`, `debug_assert!`, `dbg!` and `addr_of!` become
//! the expressions they evaluate; macros that only make literals (`concat!`, `stringify!`,
//! `line!`, `cfg!`, `include_str!`, ...) run nothing. A `macro_rules!` macro of the crates read is
//! expanded by its own rules ([`expand`]): its matchers are matched fragment by fragment with
//! `syn`'s parsers, with repetitions, and its transcriber is filled in, `$crate` included.
//! [`expand_items`] does the same for invocations in item position.
//!
//! Guarantees:
//!
//! - **What runs is read, or the invocation is reported.** An invocation is either read as above,
//!   or, for an outside macro the check does not know, read as comma-separated expressions or as
//!   statements when its tokens parse so, or else reported as unread ([`MacroRead::Unread`]); it
//!   is never skipped.
//! - **Expansion is the compiler's.** Rules are tried in order and the first whose matcher matches
//!   the whole input is used; fragments are parsed by the same grammar `syn` gives the compiler's
//!   fragment kinds; a transcription whose repetitions do not agree in length is an error, as in
//!   the compiler.
//! - **Terminates.** Expansion depth is limited to [`EXPANSION_LIMIT`], the compiler's default
//!   recursion limit.
//!
//! Method: The Rust Reference, "Macros By Example": matching, fragment specifiers, repetitions,
//! transcription and `$crate` (<https://doc.rust-lang.org/reference/macros-by-example.html>);
//! `std::fmt`'s format string grammar (<https://doc.rust-lang.org/std/fmt/index.html#syntax>).
//!
//! Cost: matching is linear in the invocation's tokens for each rule tried, times the
//! speculative parses of repetitions; transcription is linear in its output. Recursion follows
//! the nesting of token groups in a macro's rules.

use std::collections::BTreeMap;

use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::ToTokens;
use syn::parse::discouraged::Speculative;
use syn::parse::{ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::{Expr, Token};

use crate::recursion::model::{MacroId, ModId, Program, Scope, SelfRef};
use crate::recursion::resolve::{Ns, Res, Resolver};

/// How deep macro expansion goes: the compiler's default `recursion_limit`.
pub const EXPANSION_LIMIT: usize = 128;

/// The standard macros whose arguments are a format string and its arguments, each with the
/// position of the format string among its comma-separated arguments.
pub const FORMAT_MACROS: [(&str, usize); 16] = [
    ("format", 0),
    ("format_args", 0),
    ("print", 0),
    ("println", 0),
    ("eprint", 0),
    ("eprintln", 0),
    ("panic", 0),
    ("unreachable", 0),
    ("todo", 0),
    ("unimplemented", 0),
    ("write", 1),
    ("writeln", 1),
    ("assert", 1),
    ("debug_assert", 1),
    ("assert_eq", 2),
    ("assert_ne", 2),
];

/// More of the same: the `debug_` forms of the comparing assertions.
pub const FORMAT_MACROS_DEBUG: [(&str, usize); 2] = [("debug_assert_eq", 2), ("debug_assert_ne", 2)];

/// The standard macros that expand to literals or configuration only, and run nothing.
pub const LITERAL_MACROS: [&str; 12] = [
    "concat",
    "stringify",
    "line",
    "column",
    "file",
    "module_path",
    "cfg",
    "env",
    "option_env",
    "include_str",
    "include_bytes",
    "compile_error",
];

/// The standard macros that evaluate their comma-separated arguments as expressions.
pub const EXPRESSION_MACROS: [&str; 4] = ["dbg", "addr_of", "addr_of_mut", "debug_assert_matches"];

/// A short excerpt of tokens, for messages.
#[must_use]
pub fn excerpt(tokens: &TokenStream) -> String {
    let text = tokens.to_string();
    let mut out: String = text.chars().take(80).collect();
    if text.chars().count() > 80 {
        out.push_str("...");
    }
    out
}

/// The formatting trait a placeholder calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FmtTrait {
    /// `{}`.
    Display,
    /// `{:?}`, `{:x?}`, `{:X?}`, `{:#?}`.
    Debug,
    /// `{:x}`.
    LowerHex,
    /// `{:X}`.
    UpperHex,
    /// `{:o}`.
    Octal,
    /// `{:b}`.
    Binary,
    /// `{:e}`.
    LowerExp,
    /// `{:E}`.
    UpperExp,
    /// `{:p}`.
    Pointer,
    /// A width or precision argument (`{:1$}`, `{:.*}`): a `usize`, formatted by no trait.
    Count,
}

impl FmtTrait {
    /// The trait's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            FmtTrait::Display => "Display",
            FmtTrait::Debug => "Debug",
            FmtTrait::LowerHex => "LowerHex",
            FmtTrait::UpperHex => "UpperHex",
            FmtTrait::Octal => "Octal",
            FmtTrait::Binary => "Binary",
            FmtTrait::LowerExp => "LowerExp",
            FmtTrait::UpperExp => "UpperExp",
            FmtTrait::Pointer => "Pointer",
            FmtTrait::Count => "",
        }
    }
}

/// Which argument a placeholder names.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArgRef {
    /// The positional argument with this index.
    Index(usize),
    /// The named argument, or the variable captured, with this name.
    Name(String),
}

/// Every argument a format string formats, with the trait that formats it, in order; `None` for a
/// string that is malformed.
#[must_use]
pub fn format_traits(fmt: &str) -> Option<Vec<(ArgRef, FmtTrait)>> {
    let mut out = Vec::new();
    let mut next = 0_usize;
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    continue;
                }
                let mut inner = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(ch) => inner.push(ch),
                        None => return None,
                    }
                }
                let (arg, spec) = inner.split_once(':').unwrap_or((inner.as_str(), ""));
                let arg = arg.trim();
                // Width and precision arguments: `name$`, `N$` and `.*`.
                let mut counts: Vec<ArgRef> = Vec::new();
                for part in spec.split('$').take(spec.matches('$').count()) {
                    let word: String = part
                        .chars()
                        .rev()
                        .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    if let Ok(i) = word.parse::<usize>() {
                        counts.push(ArgRef::Index(i));
                    } else if !word.is_empty() {
                        counts.push(ArgRef::Name(word));
                    }
                }
                if spec.contains(".*") {
                    // `.*` takes the precision from the next positional argument, before the value.
                    counts.push(ArgRef::Index(next));
                    next = next.checked_add(1)?;
                }
                let arg_ref = if arg.is_empty() {
                    let a = ArgRef::Index(next);
                    next = next.checked_add(1)?;
                    a
                } else if let Ok(i) = arg.parse::<usize>() {
                    ArgRef::Index(i)
                } else {
                    ArgRef::Name(arg.to_owned())
                };
                out.push((arg_ref, trait_of(spec)));
                out.extend(counts.into_iter().map(|a| (a, FmtTrait::Count)));
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                } else {
                    return None;
                }
            }
            _ => {}
        }
    }
    Some(out)
}

/// The trait a format spec names by its type character.
fn trait_of(spec: &str) -> FmtTrait {
    let spec = spec.trim_end();
    if spec.ends_with('?') {
        FmtTrait::Debug
    } else if spec.ends_with('x') {
        FmtTrait::LowerHex
    } else if spec.ends_with('X') {
        FmtTrait::UpperHex
    } else if spec.ends_with('o') {
        FmtTrait::Octal
    } else if spec.ends_with('b') {
        FmtTrait::Binary
    } else if spec.ends_with('e') {
        FmtTrait::LowerExp
    } else if spec.ends_with('E') {
        FmtTrait::UpperExp
    } else if spec.ends_with('p') {
        FmtTrait::Pointer
    } else {
        FmtTrait::Display
    }
}

/// One formatted argument: its expression, or the variable it captures by name, and its trait.
#[derive(Clone, Debug)]
pub enum Formatted {
    /// An argument expression.
    Expr(Box<Expr>, FmtTrait),
    /// A variable captured by name from the format string (`{x}`).
    Captured(String, FmtTrait),
}

/// What an invocation in a function body means for the call graph.
#[derive(Clone, Debug)]
pub enum MacroRead {
    /// It runs nothing.
    Nothing,
    /// It evaluates these expressions, in order.
    Exprs(Vec<Expr>),
    /// A formatting macro: the expressions evaluated first (a destination, compared values), the
    /// formatted arguments, and for `write!`/`writeln!` the destination whose `write_fmt` runs.
    Format {
        /// Evaluated as they are.
        plain: Vec<Expr>,
        /// Formatted, each by its trait.
        formatted: Vec<Formatted>,
        /// `write!`'s destination.
        destination: Option<Box<Expr>>,
        /// For `assert_eq!` and its kin: the two values compared with `==`.
        compared: Option<(Box<Expr>, Box<Expr>)>,
    },
    /// `vec![x; n]`: `x` is cloned `n` times.
    Repeat(Box<Expr>, Box<Expr>),
    /// A `macro_rules!` expansion, as statements.
    Expanded(Vec<syn::Stmt>),
    /// The invocation could not be read.
    Unread(String),
}

/// What macro a path names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A `macro_rules!` macro of the crates read.
    Local(MacroId),
    /// A macro from outside, by its last path segment.
    Outside(String),
}

/// Which macro `path` names from `scope`: a `macro_rules!` macro in scope textually (in this
/// module or an enclosing one), by `use`, or by path; otherwise an outside macro.
#[must_use]
pub fn target(resolver: &Resolver<'_>, scope: &Scope, path: &syn::Path) -> Target {
    let program = resolver.program;
    let segs = Resolver::segments(path);
    let name = segs.last().cloned().unwrap_or_default();
    if segs.len() == 1 {
        let mut m = Some(scope.module);
        while let Some(id) = m {
            if let Some(Res::Macro(found)) = resolver.lookup(id, &name, Ns::Macro).first() {
                return Target::Local(*found);
            }
            m = program.module(id).parent;
        }
        // `#[macro_export]` macros of every crate read are at their crate roots.
        for c in &program.crates {
            if let Some(ids) = program.module(c.root).macros.get(&name)
                && let Some(id) = ids.first()
                && program.macros.get(id.0).is_some_and(|d| d.krate == program.module(scope.module).krate)
            {
                return Target::Local(*id);
            }
        }
        return Target::Outside(name);
    }
    for res in resolver.resolve(scope, &segs, path.leading_colon.is_some(), Ns::Macro) {
        if let Res::Macro(id) = res {
            return Target::Local(id);
        }
    }
    Target::Outside(name)
}

/// The comma-separated expressions of an invocation's tokens, if they parse so.
fn exprs(tokens: &TokenStream) -> Option<Vec<Expr>> {
    Punctuated::<Expr, Token![,]>::parse_terminated
        .parse2(tokens.clone())
        .ok()
        .map(|p| p.into_iter().collect())
}

/// The statements of tokens, if they parse so.
fn stmts(tokens: &TokenStream) -> Option<Vec<syn::Stmt>> {
    syn::Block::parse_within.parse2(tokens.clone()).ok()
}

/// Reads a format macro's arguments, the format string at `at`.
fn read_format(name: &str, at: usize, tokens: &TokenStream) -> MacroRead {
    // Named arguments `name = expr` parse as assignment expressions.
    let Some(args) = exprs(tokens) else {
        return MacroRead::Unread(format!("`{name}!` arguments that do not parse: {}", excerpt(tokens)));
    };
    let mut plain: Vec<Expr> = args.iter().take(at).cloned().collect();
    let mut destination = None;
    let mut compared = None;
    if matches!(name, "write" | "writeln") {
        destination = plain.pop().map(Box::new);
    }
    if matches!(
        name,
        "assert_eq" | "assert_ne" | "debug_assert_eq" | "debug_assert_ne"
    ) && let [a, b] = plain.as_slice()
    {
        compared = Some((Box::new(a.clone()), Box::new(b.clone())));
        plain.clear();
    }
    let mut formatted = Vec::new();
    if let Some((a, b)) = &compared {
        // A failing comparison formats both values with Debug.
        formatted.push(Formatted::Expr(a.clone(), FmtTrait::Debug));
        formatted.push(Formatted::Expr(b.clone(), FmtTrait::Debug));
    }
    let rest: Vec<&Expr> = args.iter().skip(at).collect();
    let Some((fmt, values)) = rest.split_first() else {
        return MacroRead::Format {
            plain,
            formatted,
            destination,
            compared,
        };
    };
    let mut positional: Vec<&Expr> = Vec::new();
    let mut named: BTreeMap<String, &Expr> = BTreeMap::new();
    for v in values {
        if let Expr::Assign(assign) = v
            && let Expr::Path(p) = &*assign.left
            && let Some(id) = p.path.get_ident()
        {
            named.insert(id.to_string(), &assign.right);
        } else {
            positional.push(v);
        }
    }
    let traits = match fmt {
        Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s),
            ..
        }) => format_traits(&s.value()),
        _ => None,
    };
    let Some(traits) = traits else {
        // Not a literal format string (or a malformed one): every argument may be formatted by
        // any trait, which the body analysis reads as Display and Debug and every other.
        for v in positional.iter().chain(named.values()) {
            for t in ALL_FMT {
                formatted.push(Formatted::Expr(Box::new((*v).clone()), t));
            }
        }
        plain.push((*fmt).clone());
        return MacroRead::Format {
            plain,
            formatted,
            destination,
            compared,
        };
    };
    let mut used = vec![false; positional.len()];
    for (arg, t) in traits {
        match arg {
            ArgRef::Index(i) => {
                if let Some(e) = positional.get(i) {
                    formatted.push(Formatted::Expr(Box::new((*e).clone()), t));
                    if let Some(u) = used.get_mut(i) {
                        *u = true;
                    }
                }
            }
            ArgRef::Name(n) => match named.get(&n) {
                Some(e) => formatted.push(Formatted::Expr(Box::new((*e).clone()), t)),
                None => formatted.push(Formatted::Captured(n, t)),
            },
        }
    }
    // An argument no placeholder names is still evaluated.
    for (e, u) in positional.iter().zip(used) {
        if !u {
            plain.push((*e).clone());
        }
    }
    MacroRead::Format {
        plain,
        formatted,
        destination,
        compared,
    }
}

/// Every formatting trait.
pub const ALL_FMT: [FmtTrait; 9] = [
    FmtTrait::Display,
    FmtTrait::Debug,
    FmtTrait::LowerHex,
    FmtTrait::UpperHex,
    FmtTrait::Octal,
    FmtTrait::Binary,
    FmtTrait::LowerExp,
    FmtTrait::UpperExp,
    FmtTrait::Pointer,
];

/// What an invocation of `mac` from `scope` means; `depth` is how many expansions led here.
#[must_use]
pub fn read(resolver: &Resolver<'_>, scope: &Scope, mac: &syn::Macro, depth: usize) -> MacroRead {
    let tokens = &mac.tokens;
    match target(resolver, scope, &mac.path) {
        Target::Local(id) => {
            if depth >= EXPANSION_LIMIT {
                return MacroRead::Unread("macro expansion deeper than the recursion limit".to_owned());
            }
            match expand(resolver.program, scope.module, id, tokens) {
                Ok(out) => match stmts(&out) {
                    Some(s) => MacroRead::Expanded(s),
                    None => match syn::parse2::<Expr>(out.clone()) {
                        Ok(e) => MacroRead::Expanded(vec![syn::Stmt::Expr(e, None)]),
                        Err(_) => MacroRead::Unread(format!(
                            "the expansion of `{}!` is neither statements nor an expression: {}",
                            mac.path.to_token_stream(),
                            excerpt(&out)
                        )),
                    },
                },
                Err(why) => MacroRead::Unread(why),
            }
        }
        Target::Outside(name) => {
            if let Some((_, at)) = FORMAT_MACROS
                .iter()
                .chain(FORMAT_MACROS_DEBUG.iter())
                .find(|(n, _)| *n == name)
            {
                return read_format(&name, *at, tokens);
            }
            if LITERAL_MACROS.contains(&name.as_str()) {
                return MacroRead::Nothing;
            }
            if name == "include" {
                return MacroRead::Unread("`include!` pastes in code the recursion check does not read".to_owned());
            }
            if name == "matches" || name == "assert_matches" {
                return read_matches(tokens);
            }
            if name == "vec" {
                if let Ok((x, n)) = (|s: ParseStream| {
                    let x: Expr = s.parse()?;
                    let _: Token![;] = s.parse()?;
                    let n: Expr = s.parse()?;
                    Ok((x, n))
                })
                .parse2(tokens.clone())
                {
                    return MacroRead::Repeat(Box::new(x), Box::new(n));
                }
            }
            if let Some(e) = exprs(tokens) {
                return MacroRead::Exprs(e);
            }
            if EXPRESSION_MACROS.contains(&name.as_str()) || name == "vec" {
                return MacroRead::Unread(format!("`{name}!` arguments that do not parse"));
            }
            if let Some(s) = stmts(tokens) {
                return MacroRead::Expanded(s);
            }
            MacroRead::Unread(format!(
                "an outside macro `{name}!` whose tokens are neither expressions nor statements: {}",
                excerpt(tokens)
            ))
        }
    }
}

/// `matches!(expr, pattern)` and `matches!(expr, pattern if guard)`: what the standard library
/// expands them to, `match expr { pattern if guard => true, _ => false }`, so the guard sees the
/// names the pattern binds.
fn read_matches(tokens: &TokenStream) -> MacroRead {
    let parsed = (|s: ParseStream| {
        let e: Expr = s.parse()?;
        let _: Token![,] = s.parse()?;
        let pat = syn::Pat::parse_multi_with_leading_vert(s)?;
        let guard = if s.peek(Token![if]) {
            let _: Token![if] = s.parse()?;
            Some(s.parse::<Expr>()?)
        } else {
            None
        };
        let _: Option<Token![,]> = s.parse()?;
        Ok((e, pat, guard))
    })
    .parse2(tokens.clone());
    let Ok((e, pat, guard)) = parsed else {
        return MacroRead::Unread(format!("`matches!` arguments that do not parse: {}", excerpt(tokens)));
    };
    let guard = guard.map(|g| quote::quote! { if #g });
    let expanded = quote::quote! {
        match #e {
            #pat #guard => true,
            _ => false,
        }
    };
    match syn::parse2::<Expr>(expanded) {
        Ok(m) => MacroRead::Exprs(vec![m]),
        Err(_) => MacroRead::Unread(format!("`matches!` arguments that do not parse: {}", excerpt(tokens))),
    }
}

/// Expands an item-position invocation from `module` into items: `None` for an outside macro
/// that writes no item the check needs (none is known), an error for one it cannot read.
///
/// # Errors
///
/// A macro that cannot be expanded, or whose expansion is not items.
pub fn expand_items(
    program: &Program,
    module: ModId,
    mac: &syn::Macro,
) -> Result<Option<Vec<syn::Item>>, String> {
    let resolver = Resolver::new(program);
    let scope = Scope {
        module,
        generics: Vec::new(),
        consts: Vec::new(),
        self_ref: SelfRef::None,
    };
    match target(&resolver, &scope, &mac.path) {
        Target::Local(id) => {
            let out = expand(program, module, id, &mac.tokens)?;
            (|s: ParseStream| {
                let mut items = Vec::new();
                while !s.is_empty() {
                    items.push(s.parse::<syn::Item>()?);
                }
                Ok(items)
            })
            .parse2(out)
            .map(Some)
            .map_err(|e| format!("the expansion of `{}!` is not items: {e}", mac.path.to_token_stream()))
        }
        Target::Outside(name) if LITERAL_MACROS.contains(&name.as_str()) => Ok(None),
        Target::Outside(name) => Err(format!(
            "an outside macro `{name}!` in item position, which the recursion check cannot expand"
        )),
    }
}

// ------------------------------------------------------------------ macro_rules

/// One element of a matcher.
#[derive(Clone, Debug)]
enum Matcher {
    /// A token to match exactly.
    Token(TokenTree),
    /// A delimited group and what it holds.
    Group(Delimiter, Vec<Matcher>),
    /// `$name:kind`.
    Fragment(String, String),
    /// `$( ... ) sep op`.
    Repeat(Vec<Matcher>, Option<TokenTree>, char),
}

/// What a matcher's fragment bound.
#[derive(Clone, Debug)]
enum Binding {
    /// One fragment's tokens.
    One(TokenStream),
    /// One binding per repetition.
    Many(Vec<Binding>),
}

/// The rules of a `macro_rules!` body: each matcher and transcriber.
fn rules(body: &TokenStream) -> Result<Vec<(Vec<Matcher>, TokenStream)>, String> {
    let mut out = Vec::new();
    let tokens: Vec<TokenTree> = body.clone().into_iter().collect();
    let mut i = 0_usize;
    while i < tokens.len() {
        let (Some(TokenTree::Group(m)), Some(TokenTree::Punct(eq)), Some(TokenTree::Punct(gt)), Some(TokenTree::Group(t))) = (
            tokens.get(i),
            tokens.get(i.saturating_add(1)),
            tokens.get(i.saturating_add(2)),
            tokens.get(i.saturating_add(3)),
        ) else {
            return Err("a macro_rules! rule that is not `(matcher) => {transcriber}`".to_owned());
        };
        if eq.as_char() != '=' || gt.as_char() != '>' {
            return Err("a macro_rules! rule without `=>`".to_owned());
        }
        out.push((matchers(&m.stream())?, t.stream()));
        i = i.saturating_add(4);
        if let Some(TokenTree::Punct(p)) = tokens.get(i)
            && p.as_char() == ';'
        {
            i = i.saturating_add(1);
        }
    }
    Ok(out)
}

/// The matcher elements of a matcher's tokens.
fn matchers(tokens: &TokenStream) -> Result<Vec<Matcher>, String> {
    let tt: Vec<TokenTree> = tokens.clone().into_iter().collect();
    let mut out = Vec::new();
    let mut i = 0_usize;
    while let Some(t) = tt.get(i) {
        i = i.saturating_add(1);
        match t {
            TokenTree::Punct(p) if p.as_char() == '$' => match tt.get(i) {
                Some(TokenTree::Ident(name)) => {
                    let (Some(TokenTree::Punct(colon)), Some(TokenTree::Ident(kind))) =
                        (tt.get(i.saturating_add(1)), tt.get(i.saturating_add(2)))
                    else {
                        return Err(format!("a matcher `${name}` without a fragment kind"));
                    };
                    if colon.as_char() != ':' {
                        return Err(format!("a matcher `${name}` without a fragment kind"));
                    }
                    out.push(Matcher::Fragment(name.to_string(), kind.to_string()));
                    i = i.saturating_add(3);
                }
                Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                    let inner = matchers(&g.stream())?;
                    i = i.saturating_add(1);
                    let (sep, op) = repetition_tail(&tt, &mut i)?;
                    out.push(Matcher::Repeat(inner, sep, op));
                }
                _ => return Err("a `$` in a matcher that starts nothing".to_owned()),
            },
            TokenTree::Group(g) => out.push(Matcher::Group(g.delimiter(), matchers(&g.stream())?)),
            other => out.push(Matcher::Token(other.clone())),
        }
    }
    Ok(out)
}

/// The separator and operator after a repetition's group, advancing `i` past them.
fn repetition_tail(tt: &[TokenTree], i: &mut usize) -> Result<(Option<TokenTree>, char), String> {
    let op_of = |t: Option<&TokenTree>| match t {
        Some(TokenTree::Punct(p)) if matches!(p.as_char(), '*' | '+' | '?') => Some(p.as_char()),
        _ => None,
    };
    if let Some(op) = op_of(tt.get(*i)) {
        *i = i.saturating_add(1);
        return Ok((None, op));
    }
    let sep = tt.get(*i).cloned();
    if let Some(op) = op_of(tt.get(i.saturating_add(1))) {
        *i = i.saturating_add(2);
        return Ok((sep, op));
    }
    Err("a repetition without `*`, `+` or `?`".to_owned())
}

/// Whether two tokens match as macro tokens.
fn same_token(a: &TokenTree, b: &TokenTree) -> bool {
    match (a, b) {
        (TokenTree::Ident(x), TokenTree::Ident(y)) => x == y,
        (TokenTree::Punct(x), TokenTree::Punct(y)) => x.as_char() == y.as_char(),
        (TokenTree::Literal(x), TokenTree::Literal(y)) => x.to_string() == y.to_string(),
        _ => false,
    }
}

/// Parses one fragment of `kind` from `input`, as its tokens.
fn fragment(kind: &str, input: ParseStream) -> syn::Result<TokenStream> {
    Ok(match kind {
        "expr" | "expr_2021" => input.parse::<Expr>()?.to_token_stream(),
        "ty" => input.parse::<syn::Type>()?.to_token_stream(),
        "ident" => {
            use syn::ext::IdentExt;
            Ident::parse_any(input)?.to_token_stream()
        }
        "path" => input.parse::<syn::Path>()?.to_token_stream(),
        "pat" => syn::Pat::parse_multi_with_leading_vert(input)?.to_token_stream(),
        "pat_param" => syn::Pat::parse_single(input)?.to_token_stream(),
        "block" => input.parse::<syn::Block>()?.to_token_stream(),
        "stmt" => input.parse::<syn::Stmt>()?.to_token_stream(),
        "item" => input.parse::<syn::Item>()?.to_token_stream(),
        "lifetime" => input.parse::<syn::Lifetime>()?.to_token_stream(),
        "literal" => {
            let mut ts = TokenStream::new();
            if input.peek(Token![-]) {
                let minus: Token![-] = input.parse()?;
                ts.extend(minus.to_token_stream());
            }
            ts.extend(input.parse::<syn::Lit>()?.to_token_stream());
            ts
        }
        "vis" => input.parse::<syn::Visibility>()?.to_token_stream(),
        "meta" => input.parse::<syn::Meta>()?.to_token_stream(),
        "tt" => input.parse::<TokenTree>()?.to_token_stream(),
        other => return Err(input.error(format!("unknown fragment kind `{other}`"))),
    })
}

/// Matches `ms` against `input`, adding bindings.
fn match_seq(ms: &[Matcher], input: ParseStream, b: &mut BTreeMap<String, Binding>) -> syn::Result<()> {
    for m in ms {
        match m {
            Matcher::Token(t) => {
                let got: TokenTree = input.parse()?;
                if !same_token(t, &got) {
                    return Err(input.error("token mismatch"));
                }
            }
            Matcher::Group(d, inner) => {
                let got: TokenTree = input.parse()?;
                let TokenTree::Group(g) = got else {
                    return Err(input.error("expected a group"));
                };
                if g.delimiter() != *d {
                    return Err(input.error("delimiter mismatch"));
                }
                (|s: ParseStream| {
                    match_seq(inner, s, b)?;
                    if s.is_empty() { Ok(()) } else { Err(s.error("tokens left in group")) }
                })
                .parse2(g.stream())?;
            }
            Matcher::Fragment(name, kind) => {
                let ts = fragment(kind, input)?;
                b.insert(name.clone(), Binding::One(ts));
            }
            Matcher::Repeat(inner, sep, op) => {
                let mut iterations: Vec<BTreeMap<String, Binding>> = Vec::new();
                loop {
                    if *op == '?' && !iterations.is_empty() {
                        break;
                    }
                    let fork = input.fork();
                    if let Some(sep) = sep
                        && !iterations.is_empty()
                    {
                        let Ok(got) = fork.parse::<TokenTree>() else {
                            break;
                        };
                        if !same_token(sep, &got) {
                            break;
                        }
                    }
                    let mut one = BTreeMap::new();
                    if fork.is_empty() || match_seq(inner, &fork, &mut one).is_err() {
                        break;
                    }
                    input.advance_to(&fork);
                    iterations.push(one);
                }
                if *op == '+' && iterations.is_empty() {
                    return Err(input.error("a `+` repetition matched nothing"));
                }
                let mut names: Vec<String> = Vec::new();
                names_in(inner, &mut names);
                for name in names {
                    let seq = iterations
                        .iter()
                        .map(|it| it.get(&name).cloned().unwrap_or(Binding::Many(Vec::new())))
                        .collect();
                    b.insert(name, Binding::Many(seq));
                }
            }
        }
    }
    Ok(())
}

/// The fragment names in a matcher.
fn names_in(ms: &[Matcher], out: &mut Vec<String>) {
    for m in ms {
        match m {
            Matcher::Fragment(n, _) => out.push(n.clone()),
            Matcher::Group(_, inner) | Matcher::Repeat(inner, _, _) => names_in(inner, out),
            Matcher::Token(_) => {}
        }
    }
}

/// Expands the macro `id`, invoked from `module` with `tokens`.
///
/// # Errors
///
/// No rule matches, or a transcription is malformed.
pub fn expand(program: &Program, module: ModId, id: MacroId, tokens: &TokenStream) -> Result<TokenStream, String> {
    let Some(def) = program.macros.get(id.0) else {
        return Err("an unknown macro".to_owned());
    };
    let crate_word = if def.krate == program.module(module).krate {
        "crate".to_owned()
    } else {
        program
            .crates
            .get(def.krate)
            .map_or_else(|| "crate".to_owned(), |c| c.source.lib.clone())
    };
    for (matcher, transcriber) in rules(&def.body)? {
        let mut bindings = BTreeMap::new();
        let matched = (|s: ParseStream| {
            match_seq(&matcher, s, &mut bindings)?;
            if s.is_empty() { Ok(()) } else { Err(s.error("tokens left")) }
        })
        .parse2(tokens.clone());
        if matched.is_ok() {
            return transcribe(&transcriber, &bindings, &[], &crate_word);
        }
    }
    Err(format!(
        "no rule of `{}!` matches its invocation: {}",
        def.name,
        excerpt(tokens)
    ))
}

/// The binding of `name` at the repetition indices `at`.
fn binding_at<'b>(b: &'b BTreeMap<String, Binding>, name: &str, at: &[usize]) -> Option<&'b Binding> {
    let mut cur = b.get(name)?;
    for i in at {
        match cur {
            Binding::Many(items) => cur = items.get(*i)?,
            Binding::One(_) => return Some(cur),
        }
    }
    Some(cur)
}

/// Fills in a transcriber.
fn transcribe(
    tokens: &TokenStream,
    b: &BTreeMap<String, Binding>,
    at: &[usize],
    crate_word: &str,
) -> Result<TokenStream, String> {
    let tt: Vec<TokenTree> = tokens.clone().into_iter().collect();
    let mut out = TokenStream::new();
    let mut i = 0_usize;
    while let Some(t) = tt.get(i) {
        i = i.saturating_add(1);
        match t {
            TokenTree::Punct(p) if p.as_char() == '$' => match tt.get(i) {
                Some(TokenTree::Ident(name)) if name == "crate" => {
                    out.extend([TokenTree::Ident(Ident::new(crate_word, Span::call_site()))]);
                    i = i.saturating_add(1);
                }
                Some(TokenTree::Ident(name)) => {
                    match binding_at(b, &name.to_string(), at) {
                        Some(Binding::One(ts)) => {
                            // A fragment is one token tree when substituted.
                            out.extend([TokenTree::Group(Group::new(Delimiter::None, ts.clone()))]);
                        }
                        Some(Binding::Many(_)) => {
                            return Err(format!("`${name}` is still repeating here"));
                        }
                        None => {
                            out.extend([
                                TokenTree::Punct(Punct::new('$', Spacing::Alone)),
                                TokenTree::Ident(name.clone()),
                            ]);
                        }
                    }
                    i = i.saturating_add(1);
                }
                Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                    i = i.saturating_add(1);
                    let (sep, _op) = repetition_tail(&tt, &mut i)?;
                    let count = repeat_count(&g.stream(), b, at)?;
                    for k in 0..count {
                        if k > 0
                            && let Some(sep) = &sep
                        {
                            out.extend([sep.clone()]);
                        }
                        let mut inner_at = at.to_vec();
                        inner_at.push(k);
                        out.extend(transcribe(&g.stream(), b, &inner_at, crate_word)?);
                    }
                }
                _ => out.extend([t.clone()]),
            },
            TokenTree::Group(g) => {
                let inner = transcribe(&g.stream(), b, at, crate_word)?;
                let mut ng = Group::new(g.delimiter(), inner);
                ng.set_span(g.span());
                out.extend([TokenTree::Group(ng)]);
            }
            other => out.extend([other.clone()]),
        }
    }
    Ok(out)
}

/// How many times a transcriber repetition repeats: the length of the repeating bindings it
/// uses, which must agree.
fn repeat_count(tokens: &TokenStream, b: &BTreeMap<String, Binding>, at: &[usize]) -> Result<usize, String> {
    let mut names = Vec::new();
    vars_in(tokens, &mut names);
    let mut count: Option<usize> = None;
    for name in names {
        if let Some(Binding::Many(items)) = binding_at(b, &name, at) {
            match count {
                None => count = Some(items.len()),
                Some(c) if c != items.len() => {
                    return Err(format!("repetitions of different lengths meet at `${name}`"));
                }
                Some(_) => {}
            }
        }
    }
    count.ok_or_else(|| "a repetition in a transcriber that repeats no fragment".to_owned())
}

/// The `$name` variables in transcriber tokens.
fn vars_in(tokens: &TokenStream, out: &mut Vec<String>) {
    let tt: Vec<TokenTree> = tokens.clone().into_iter().collect();
    for (k, t) in tt.iter().enumerate() {
        match t {
            TokenTree::Punct(p) if p.as_char() == '$' => {
                if let Some(TokenTree::Ident(name)) = tt.get(k.saturating_add(1)) {
                    out.push(name.to_string());
                }
            }
            TokenTree::Group(g) => vars_in(&g.stream(), out),
            _ => {}
        }
    }
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
    use crate::recursion::model::{CrateSource, Role, load};

    #[test]
    fn format_strings_name_each_argument_and_trait() {
        let got = format_traits("{} {:?} {x} {0:#x} {name:>width$} {:.*} {{lit}} {:e}").unwrap();
        assert_eq!(
            got,
            [
                (ArgRef::Index(0), FmtTrait::Display),
                (ArgRef::Index(1), FmtTrait::Debug),
                (ArgRef::Name("x".to_owned()), FmtTrait::Display),
                (ArgRef::Index(0), FmtTrait::LowerHex),
                (ArgRef::Name("name".to_owned()), FmtTrait::Display),
                (ArgRef::Name("width".to_owned()), FmtTrait::Count),
                (ArgRef::Index(3), FmtTrait::Display),
                (ArgRef::Index(2), FmtTrait::Count),
                (ArgRef::Index(4), FmtTrait::LowerExp),
            ]
        );
        assert!(format_traits("{").is_none());
        assert!(format_traits("}").is_none());
    }

    fn program(lib: &str) -> Program {
        let text = lib.to_owned();
        let read = move |p: &str| {
            if p == "c/a/src/lib.rs" { Ok(text.clone()) } else { Err("none".to_owned()) }
        };
        load(
            &[CrateSource {
                package: "a".to_owned(),
                lib: "a".to_owned(),
                folder: "c/a".to_owned(),
                root: "c/a/src/lib.rs".to_owned(),
                role: Role::Ships,
            }],
            &read,
        )
    }

    #[test]
    fn macro_rules_expand_with_repetitions_and_crate() {
        let p = program(
            "macro_rules! calls { ($($f:ident),* ; $x:expr) => { $( $crate::$f($x); )* }; }\n\
             macro_rules! make { ($n:ident) => { fn $n() { helper() } }; }\n\
             make!(generated);\n\
             fn helper() {}",
        );
        assert!(p.problems.is_empty(), "{:?}", p.problems);
        assert!(p.nodes.iter().any(|n| n.name == "a::generated"), "item macros expand");
        let calls = MacroId(0);
        let out = expand(&p, ModId(0), calls, &"f, g; 1 + 2".parse().unwrap()).unwrap();
        let text = out.to_string().replace(' ', "");
        assert_eq!(text, "crate::f(1+2);crate::g(1+2);");
        assert!(expand(&p, ModId(0), calls, &"; ;".parse().unwrap()).is_err());
    }

    #[test]
    fn outside_macros_are_read_or_reported() {
        let p = program("fn f() {}");
        let r = Resolver::new(&p);
        let scope = p.nodes[0].scope.clone();
        let mac = |s: &str| -> syn::Macro { syn::parse_str(s).unwrap() };
        assert!(matches!(read(&r, &scope, &mac("concat!(\"a\", \"b\")"), 0), MacroRead::Nothing));
        let MacroRead::Format { formatted, destination, .. } =
            read(&r, &scope, &mac("write!(f, \"{} {:?}\", a, b)"), 0)
        else {
            panic!("write! is a format macro");
        };
        assert!(destination.is_some());
        assert_eq!(formatted.len(), 2);
        assert!(matches!(read(&r, &scope, &mac("matches!(x, Some(_) if y)"), 0), MacroRead::Exprs(e) if e.len() == 2));
        assert!(matches!(read(&r, &scope, &mac("vec![x; 3]"), 0), MacroRead::Repeat(..)));
        assert!(matches!(read(&r, &scope, &mac("include!(\"x.rs\")"), 0), MacroRead::Unread(_)));
        assert!(matches!(read(&r, &scope, &mac("weird!(=> =>)"), 0), MacroRead::Unread(_)));
    }
}
