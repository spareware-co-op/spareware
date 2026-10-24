//! The `Recursion` field of a module header: each recursive cycle's stated depth bound.
//!
//! What it does: [`read`] finds the `Recursion:` paragraph of a file's `//!` header and reads the
//! list after it. Each item states one cycle (or one group of recursive types) in four labelled
//! parts, the first on the item's first line and each other at the start of a line of its own:
//!
//! ```text
//! Recursion:
//!
//! - Cycle: `Tree::walk`, `walk_children`.
//!   Bound: 64 (`MAX_DEPTH`).
//!   Why: each call goes one level down the tree, and `Tree::walk` refuses past `MAX_DEPTH`.
//!   Test: `walk_at_its_bound`.
//! - Types: `Expr`.
//!   Bound: 32 (`MAX_NESTING`).
//!   Why: ...
//!   Test: `deepest_expr_drops_and_clones`.
//! ```
//!
//! `Cycle:` names the cycle's functions (closures are named by the function they are written
//! in), relative to the module or from the crate (`crate::...`); `Types:` names recursive
//! structs, enums or unions, whose drop glue and derived implementations recurse over them.
//! `Bound:` is the most frames of the cycle that are ever on the stack at once (for types: the
//! deepest nesting of their values), a positive whole number, with the constant item that
//! enforces it in backticks when there is one (`const NAME: T = <integer literal>;` in the crate,
//! whose value must be the bound). `Why:` says why the bound holds. `Test:` names the test, in
//! the crate's `tests/recursion.rs`, that runs the cycle at its bound on the small WebAssembly
//! stack ([`crate::recursion::stack`]).
//!
//! Guarantees:
//!
//! - **Read as written, or reported.** A part that is missing, written twice, empty, a
//!   placeholder ([`crate::map::header::PLACEHOLDERS`]) or malformed is a [`Problem`] naming the
//!   file, the line and what to write; a `Recursion:` paragraph without a list, or written twice,
//!   is one too. A statement is never read from text that is not in this grammar.
//! - **Header only.** Only the `//!` lines that open the file are read, as the map reads
//!   headers (`projects/engines/geometry/HEADERS.md`).
//!
//! Method: the header grammar of `projects/engines/geometry/HEADERS.md` (paragraphs, `- ` list
//! items with indented continuation lines, labels before a colon), which this field joins.
//!
//! Cost: linear in the length of the header.

use crate::map::header::{Header, states};
use crate::recursion::{Problem, Rule};

/// The label of the field.
pub const FIELD: &str = "Recursion:";

/// The labels of a statement's parts, in order.
pub const PARTS: [&str; 5] = ["Cycle: ", "Types: ", "Bound: ", "Why: ", "Test: "];

/// What a statement is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A cycle of functions.
    Cycle,
    /// A group of recursive types.
    Types,
}

/// One stated cycle and its bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Statement {
    /// The file it is in, relative to the workspace root.
    pub file: String,
    /// The line its item starts on.
    pub line: usize,
    /// What it is about.
    pub kind: Kind,
    /// The names it lists, as written.
    pub names: Vec<String>,
    /// The bound.
    pub bound: u64,
    /// The constant that enforces it, as written, if named.
    pub constant: Option<String>,
    /// Why the bound holds.
    pub why: String,
    /// The test's name.
    pub test: String,
}

/// The text between each pair of backticks.
#[must_use]
pub fn backticked(text: &str) -> Vec<String> {
    text.split('`')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.trim().to_owned())
        .collect()
}

/// Every statement in the header of `text`, the file `file`, and every problem with them.
#[must_use]
pub fn read(file: &str, text: &str) -> (Vec<Statement>, Vec<Problem>) {
    let header = Header::parse(text);
    let lines = &header.lines;
    let mut statements = Vec::new();
    let mut problems = Vec::new();
    let line_no = |i: usize| i.saturating_add(1);
    let opens: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(i, l)| {
            l.starts_with("Recursion")
                && (*i == 0 || lines.get(i.saturating_sub(1)).is_some_and(|p| p.trim().is_empty()))
        })
        .map(|(i, _)| i)
        .collect();
    if opens.len() > 1
        && let Some(second) = opens.get(1)
    {
        problems.push(Problem::new(
            file,
            line_no(*second),
            Rule::Statement,
            "the Recursion field is written twice; state every cycle in one list".to_owned(),
        ));
    }
    let Some(&open) = opens.first() else {
        return (statements, problems);
    };
    let first = lines.get(open).map_or("", String::as_str).trim_end();
    if first != FIELD {
        problems.push(Problem::new(
            file,
            line_no(open),
            Rule::Statement,
            format!("write the field as a line `{FIELD}` alone, followed by a `- ` list of cycles"),
        ));
        return (statements, problems);
    }
    // The list after it: items and their continuation lines.
    let mut items: Vec<(usize, Vec<String>)> = Vec::new();
    let mut i = open.saturating_add(1);
    let mut started = false;
    while let Some(l) = lines.get(i) {
        if l.trim().is_empty() {
            if started {
                // A blank line inside the list: the next line decides.
                let next = lines.get(i.saturating_add(1));
                if !next.is_some_and(|n| n.starts_with("- ") || n.starts_with(' ')) {
                    break;
                }
            }
        } else if let Some(rest) = l.strip_prefix("- ") {
            items.push((i, vec![rest.to_owned()]));
            started = true;
        } else if l.starts_with(' ') && started {
            if let Some((_, item)) = items.last_mut() {
                item.push(l.trim().to_owned());
            }
        } else {
            break;
        }
        i = i.saturating_add(1);
    }
    if items.is_empty() {
        problems.push(Problem::new(
            file,
            line_no(open),
            Rule::Statement,
            "the Recursion field has no `- Cycle:` or `- Types:` item".to_owned(),
        ));
    }
    for (at, item_lines) in items {
        match item(file, line_no(at), &item_lines) {
            Ok(s) => statements.push(s),
            Err(p) => problems.extend(p),
        }
    }
    (statements, problems)
}

/// Reads one item.
fn item(file: &str, line: usize, lines: &[String]) -> Result<Statement, Vec<Problem>> {
    let mut parts: Vec<(&str, String)> = Vec::new();
    let mut problems = Vec::new();
    let mut bad = |what: String| problems.push(Problem::new(file, line, Rule::Statement, what));
    for l in lines {
        if let Some(label) = PARTS.iter().find(|p| l.starts_with(*p)) {
            let rest = l.get(label.len()..).unwrap_or_default().to_owned();
            parts.push((label, rest));
        } else if let Some((_, text)) = parts.last_mut() {
            text.push(' ');
            text.push_str(l);
        } else {
            bad(format!(
                "an item that does not open with `Cycle: ` or `Types: `: {l:?}"
            ));
        }
    }
    let get = |label: &str| -> Vec<&String> {
        parts.iter().filter(|(l, _)| *l == label).map(|(_, t)| t).collect()
    };
    let kind = match parts.first().map(|(l, _)| *l) {
        Some("Cycle: ") => Kind::Cycle,
        Some("Types: ") => Kind::Types,
        _ => {
            bad("an item must open with `Cycle: ` (functions) or `Types: ` (recursive types)".to_owned());
            Kind::Cycle
        }
    };
    let head = match kind {
        Kind::Cycle => "Cycle: ",
        Kind::Types => "Types: ",
    };
    for label in PARTS {
        let n = get(label).len();
        if n > 1 {
            bad(format!("`{}` is written {n} times in one item", label.trim_end()));
        }
    }
    if !get("Cycle: ").is_empty() && !get("Types: ").is_empty() {
        bad("an item states either a `Cycle:` or `Types:`, not both".to_owned());
    }
    let names = get(head).first().map(|t| backticked(t)).unwrap_or_default();
    if names.is_empty() || names.iter().any(String::is_empty) {
        bad(format!("`{}` names nothing: list its members in backticks", head.trim_end()));
    }
    let mut bound = 0_u64;
    let mut constant = None;
    match get("Bound: ").first() {
        None => bad("no `Bound: ` line: state the most frames of the cycle on the stack at once".to_owned()),
        Some(text) => {
            let digits: String = text
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '_')
                .filter(|c| *c != '_')
                .collect();
            match digits.parse::<u64>() {
                Ok(n) if n > 0 => bound = n,
                _ => bad(format!("`Bound: {text}` does not open with a positive whole number")),
            }
            let named = backticked(text);
            match named.as_slice() {
                [] => {}
                [one] if !one.is_empty() => constant = Some(one.clone()),
                _ => bad(format!("`Bound: {text}` names more than one constant")),
            }
        }
    }
    let why = get("Why: ").first().map(|t| t.trim().to_owned()).unwrap_or_default();
    if !states(&why) {
        bad("no `Why: ` that states why the bound holds".to_owned());
    }
    let test = match get("Test: ").first().map(|t| backticked(t)) {
        Some(names) => match names.as_slice() {
            [one] if is_ident(one) => one.clone(),
            _ => {
                bad("`Test: ` names one test function of `tests/recursion.rs`, in backticks".to_owned());
                String::new()
            }
        },
        None => {
            bad("no `Test: ` line naming the test that runs the cycle at its bound".to_owned());
            String::new()
        }
    };
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(Statement {
        file: file.to_owned(),
        line,
        kind,
        names,
        bound,
        constant,
        why,
        test,
    })
}

/// Whether `s` is a Rust identifier.
fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
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

    const GOOD: &str = "//! A module.\n//!\n//! Recursion:\n//!\n//! - Cycle: `Tree::walk`, `walk_children`.\n//!   Bound: 1_024 (`MAX_DEPTH`).\n//!   Why: one level per call,\n//!   refused past the limit.\n//!   Test: `walk_at_its_bound`.\n//! - Types: `Expr`.\n//!   Bound: 32.\n//!   Why: parsing refuses deeper.\n//!   Test: `deep_expr`.\n//!\n//! Cost: linear.\nfn f() {}\n";

    #[test]
    fn statements_read_as_written() {
        let (s, p) = read("a.rs", GOOD);
        assert!(p.is_empty(), "{p:?}");
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].names, ["Tree::walk", "walk_children"]);
        assert_eq!(s[0].bound, 1024);
        assert_eq!(s[0].constant.as_deref(), Some("MAX_DEPTH"));
        assert_eq!(s[0].why, "one level per call, refused past the limit.");
        assert_eq!(s[0].test, "walk_at_its_bound");
        assert_eq!(s[0].line, 5);
        assert_eq!(s[1].kind, Kind::Types);
        assert_eq!(s[1].constant, None);
    }

    #[test]
    fn every_malformed_part_is_reported() {
        for (from, to, what) in [
            ("Bound: 1_024 (`MAX_DEPTH`).", "Bound: none.", "positive whole number"),
            ("Bound: 1_024 (`MAX_DEPTH`).", "Bound: 0.", "positive whole number"),
            ("//!   Bound: 1_024 (`MAX_DEPTH`).\n", "", "no `Bound: `"),
            ("Why: one level per call,\n//!   refused past the limit.", "Why: TBD.", "no `Why: `"),
            ("Test: `walk_at_its_bound`.", "Test: walk_at_its_bound.", "names one test"),
            ("Test: `walk_at_its_bound`.", "Test: `a`, `b`.", "names one test"),
            ("Cycle: `Tree::walk`, `walk_children`.", "Cycle: walk.", "names nothing"),
            ("- Cycle:", "- Loop:", "must open with"),
            ("Bound: 32.", "Bound: 32.\n//!   Bound: 33.", "written 2 times"),
            ("//! Recursion:\n//!\n", "//! Recursion: none.\n//!\n", "alone"),
        ] {
            assert!(GOOD.contains(from), "{from}");
            let text = GOOD.replacen(from, to, 1);
            let (_, p) = read("a.rs", &text);
            assert!(p.iter().any(|x| x.what.contains(what)), "{to:?}: {p:?}");
        }
        let (_, p) = read("a.rs", "//! A.\n//!\n//! Recursion:\n//!\n//! Cost: x.\n");
        assert!(p.iter().any(|x| x.what.contains("no `- Cycle:`")), "{p:?}");
        let twice = format!("{}//!\n//! Recursion:\n//!\n//! - Cycle: `x`.\n", GOOD.replace("fn f() {}\n", ""));
        let (_, p) = read("a.rs", &twice);
        assert!(p.iter().any(|x| x.what.contains("twice")), "{p:?}");
    }

    #[test]
    fn only_the_header_is_read() {
        let (s, p) = read("a.rs", "//! A.\nfn f() {}\n//! Recursion:\n//!\n//! - Cycle: `f`.\n");
        assert!(s.is_empty() && p.is_empty());
    }
}
