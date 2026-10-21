//! Licences: whether a package's licence expression is permissive, and the licence records every
//! development crate keeps in its header.
//!
//! What it does: [`Expr::parse`] reads an SPDX licence expression as Cargo manifests carry it.
//! [`Expr::permissive`] says whether it can be met with permissive licences alone, and
//! [`Expr::canonical`] writes it in one form per meaning, so two records can be compared. [`records`]
//! reads the licence records of a crate header: the list items after its `Licences` line, each of
//! the form ``- <expression>: `package`, `package` and `package`.``
//!
//! Guarantees:
//!
//! - **Exact grammar, or an error.** An expression with an unknown character, an unbalanced
//!   parenthesis, a missing operand or a stray operator is an [`ExprError`], never a pass.
//! - **Permissive means every licence of one way to comply is permissive.** An `OR` needs one
//!   permissive branch, an `AND` needs both, and `WITH` may add only an exception in
//!   [`EXCEPTIONS`], which grant permissions and take none away. So
//!   `MIT OR Apache-2.0 OR LGPL-2.1-or-later` is permissive, taken under MIT or Apache-2.0, and
//!   `MIT AND GPL-3.0-only` is not. The permissive licences are exactly [`PERMISSIVE`]: the list the
//!   development crates may take (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode). The Unlicense is not on
//!   it (chapter 32.6 takes it only beside another licence), nor is MPL-2.0, which is ours and is
//!   checked separately.
//! - **One form per meaning.** [`Expr::canonical`] flattens nested `OR`s and `AND`s and sorts and
//!   deduplicates their operands, so `MIT OR Apache-2.0`, `Apache-2.0 OR MIT` and Cargo's legacy
//!   `MIT/Apache-2.0` compare equal.
//!
//! Method: the SPDX license expression grammar of the SPDX specification 2.3, annex D
//! (<https://spdx.github.io/spdx-spec/v2.3/SPDX-license-expressions/>), by recursive descent, with
//! its precedence (`WITH` binds tightest, then `AND`, then `OR`) and Cargo's legacy `/` read as
//! `OR` (<https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields>).
//! Recursion is bounded: nesting deeper than [`MAX_DEPTH`] parentheses is an error.
//!
//! Cost: linear in the expression's length to parse; O(n log n) in its operands to canonicalise.

use core::fmt;

/// The licences a development crate may take from an outside package (MIT, Apache-2.0, BSD, ISC,
/// Zlib, Unicode), by SPDX identifier.
pub const PERMISSIVE: [&str; 8] = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "Unicode-3.0",
    "Unicode-DFS-2016",
];

/// The `WITH` exceptions that only add permissions to the licence they modify.
pub const EXCEPTIONS: [&str; 1] = ["LLVM-exception"];

/// The deepest nesting of parentheses [`Expr::parse`] reads.
pub const MAX_DEPTH: usize = 16;

/// A parsed licence expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    /// One licence, by SPDX identifier, with an optional `WITH` exception.
    Licence {
        /// The licence identifier.
        id: String,
        /// The exception identifier after `WITH`, if any.
        exception: Option<String>,
    },
    /// Any one of these.
    Or(Vec<Expr>),
    /// All of these.
    And(Vec<Expr>),
}

/// Why an expression could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExprError {
    /// The expression.
    pub expression: String,
    /// What is wrong with it.
    pub why: String,
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "licence expression {:?} cannot be read: {}",
            self.expression, self.why
        )
    }
}

impl std::error::Error for ExprError {}

/// A token of an expression.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Open,
    Close,
    Or,
    And,
    With,
    Id(String),
}

/// The tokens of `text`: parentheses, `/`, and runs of identifier characters, which are words or
/// operators.
fn tokens(text: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut Vec<Token>| {
        if !word.is_empty() {
            out.push(match word.as_str() {
                "OR" => Token::Or,
                "AND" => Token::And,
                "WITH" => Token::With,
                _ => Token::Id(core::mem::take(word)),
            });
            word.clear();
        }
    };
    for c in text.chars() {
        match c {
            '(' | ')' | '/' => {
                flush(&mut word, &mut out);
                out.push(match c {
                    '(' => Token::Open,
                    ')' => Token::Close,
                    _ => Token::Or,
                });
            }
            ' ' | '\t' => flush(&mut word, &mut out),
            c if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '+' || c == ':' => {
                word.push(c);
            }
            other => return Err(format!("{other:?} is not part of the grammar")),
        }
    }
    flush(&mut word, &mut out);
    Ok(out)
}

/// A recursive-descent reader over the tokens.
struct Reader {
    tokens: Vec<Token>,
    at: usize,
}

impl Reader {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.at).cloned();
        if t.is_some() {
            // `get` found a token at `at`, so at < tokens.len() and at + 1 fits: `saturating_add`
            // never saturates.
            self.at = self.at.saturating_add(1);
        }
        t
    }

    /// or := and ("OR" and)*
    fn or(&mut self, depth: usize) -> Result<Expr, String> {
        let mut terms = vec![self.and(depth)?];
        while self.peek() == Some(&Token::Or) {
            self.next();
            terms.push(self.and(depth)?);
        }
        Ok(one_or(terms, Expr::Or))
    }

    /// and := with ("AND" with)*
    fn and(&mut self, depth: usize) -> Result<Expr, String> {
        let mut terms = vec![self.with(depth)?];
        while self.peek() == Some(&Token::And) {
            self.next();
            terms.push(self.with(depth)?);
        }
        Ok(one_or(terms, Expr::And))
    }

    /// with := "(" or ")" | id ("WITH" id)?
    fn with(&mut self, depth: usize) -> Result<Expr, String> {
        match self.next() {
            Some(Token::Open) => {
                if depth >= MAX_DEPTH {
                    return Err(format!("parentheses nested deeper than {MAX_DEPTH}"));
                }
                // depth < MAX_DEPTH, so depth + 1 fits: `saturating_add` never saturates.
                let inner = self.or(depth.saturating_add(1))?;
                match self.next() {
                    Some(Token::Close) => Ok(inner),
                    _ => Err("a parenthesis is not closed".to_owned()),
                }
            }
            Some(Token::Id(id)) => {
                let exception = if self.peek() == Some(&Token::With) {
                    self.next();
                    match self.next() {
                        Some(Token::Id(e)) => Some(e),
                        _ => return Err(format!("WITH after {id} names no exception")),
                    }
                } else {
                    None
                };
                Ok(Expr::Licence { id, exception })
            }
            Some(t) => Err(format!("{t:?} where a licence was expected")),
            None => Err("the expression ends where a licence was expected".to_owned()),
        }
    }
}

/// The single term, or the terms joined by `join`.
fn one_or(mut terms: Vec<Expr>, join: fn(Vec<Expr>) -> Expr) -> Expr {
    if terms.len() == 1 {
        if let Some(t) = terms.pop() {
            return t;
        }
    }
    join(terms)
}

impl Expr {
    /// Reads an SPDX licence expression (with Cargo's legacy `/` as `OR`).
    ///
    /// # Errors
    ///
    /// [`ExprError`] for anything outside the grammar.
    pub fn parse(text: &str) -> Result<Expr, ExprError> {
        let error = |why: String| ExprError {
            expression: text.to_owned(),
            why,
        };
        let mut reader = Reader {
            tokens: tokens(text).map_err(error)?,
            at: 0,
        };
        let expr = reader.or(0).map_err(error)?;
        match reader.peek() {
            None => Ok(expr),
            Some(t) => Err(error(format!("{t:?} after a complete expression"))),
        }
    }

    /// Whether the expression can be met with licences in [`PERMISSIVE`] alone, each with no
    /// exception or one in [`EXCEPTIONS`].
    #[must_use]
    pub fn permissive(&self) -> bool {
        match self {
            Expr::Licence { id, exception } => {
                PERMISSIVE.contains(&id.as_str())
                    && exception.as_deref().is_none_or(|e| EXCEPTIONS.contains(&e))
            }
            Expr::Or(terms) => terms.iter().any(Expr::permissive),
            Expr::And(terms) => terms.iter().all(Expr::permissive),
        }
    }

    /// The expression in one form per meaning: nested `OR`s and `AND`s flattened, their operands
    /// sorted and deduplicated, and parentheses only around an operand that needs them.
    #[must_use]
    pub fn canonical(&self) -> String {
        match self {
            Expr::Licence { id, exception } => match exception {
                Some(e) => format!("{id} WITH {e}"),
                None => id.clone(),
            },
            Expr::Or(_) => join(self.flat(true), " OR ", false),
            Expr::And(_) => join(self.flat(false), " AND ", true),
        }
    }

    /// The operands of a flattened `OR` (`or` true) or `AND`.
    fn flat(&self, or: bool) -> Vec<&Expr> {
        match (self, or) {
            (Expr::Or(terms), true) | (Expr::And(terms), false) => {
                terms.iter().flat_map(|t| t.flat(or)).collect()
            }
            _ => vec![self],
        }
    }
}

/// Operands, canonical, sorted, deduplicated and joined; an `OR` inside an `AND` is parenthesised.
fn join(terms: Vec<&Expr>, by: &str, wrap_or: bool) -> String {
    let mut parts: Vec<String> = terms
        .into_iter()
        .map(|t| match t {
            Expr::Or(_) if wrap_or => format!("({})", t.canonical()),
            _ => t.canonical(),
        })
        .collect();
    parts.sort();
    parts.dedup();
    parts.join(by)
}

/// One licence record of a crate header: an expression and the packages recorded under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// The expression, as written.
    pub expression: Expr,
    /// The packages, by name, in the order written.
    pub packages: Vec<String>,
}

/// Why a header's licence records could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordError {
    /// The header has no line starting `//! Licences`.
    NoSection,
    /// A list item in the section is not ``- <expression>: `package`, ...``.
    Item(String),
    /// A record's expression cannot be read.
    Expr(ExprError),
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordError::NoSection => write!(f, "the header has no `//! Licences` section"),
            RecordError::Item(item) => write!(
                f,
                "licence record {item:?} is not `- <expression>: `package`, `package``"
            ),
            RecordError::Expr(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RecordError {}

/// The licence records of a source file's `//!` header: every list item after the line that starts
/// `//! Licences`, up to the end of the header. An item runs from its `- ` line to the next item or
/// blank `//!` line.
///
/// # Errors
///
/// [`RecordError`] if there is no such section or an item is not a record.
pub fn records(source: &str) -> Result<Vec<Record>, RecordError> {
    let header: Vec<&str> = source
        .lines()
        .take_while(|l| l.starts_with("//!"))
        .map(|l| l.trim_start_matches("//!").trim())
        .collect();
    let start = header
        .iter()
        .position(|l| l.starts_with("Licences"))
        .ok_or(RecordError::NoSection)?;
    let mut items: Vec<String> = Vec::new();
    let mut open = false;
    // The lines after the `Licences` line.
    for line in header.iter().skip(start).skip(1) {
        if let Some(rest) = line.strip_prefix("- ") {
            items.push(rest.to_owned());
            open = true;
        } else if line.is_empty() {
            open = false;
        } else if open {
            if let Some(last) = items.last_mut() {
                last.push(' ');
                last.push_str(line);
            }
        }
    }
    items
        .into_iter()
        .map(|item| {
            let (expression, names) = item
                .split_once(": ")
                .ok_or_else(|| RecordError::Item(item.clone()))?;
            let packages: Vec<String> = names
                .split('`')
                .skip(1)
                .step_by(2)
                .map(str::to_owned)
                .collect();
            if packages.is_empty() {
                return Err(RecordError::Item(item.clone()));
            }
            Ok(Record {
                expression: Expr::parse(expression).map_err(RecordError::Expr)?,
                packages,
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    fn p(text: &str) -> Expr {
        Expr::parse(text).unwrap()
    }

    #[test]
    fn permissive_needs_one_permissive_way_to_comply() {
        for yes in [
            "MIT",
            "MIT OR Apache-2.0",
            "MIT/Apache-2.0",
            "Unlicense OR MIT",
            "(MIT OR Apache-2.0) AND Unicode-3.0",
            "MIT OR Apache-2.0 OR LGPL-2.1-or-later",
            "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
            "BSD-2-Clause OR Apache-2.0 OR MIT",
            "Apache-2.0 WITH LLVM-exception",
        ] {
            assert!(p(yes).permissive(), "{yes}");
        }
        for no in [
            "GPL-3.0-only",
            "MIT AND GPL-3.0-only",
            "LGPL-2.1-or-later",
            "Unlicense",
            "MPL-2.0",
            "(MIT OR Apache-2.0) AND AGPL-3.0-only",
            "GPL-2.0-only WITH Classpath-exception-2.0",
            "MIT WITH Some-unknown-exception",
        ] {
            assert!(!p(no).permissive(), "{no}");
        }
    }

    #[test]
    fn precedence_is_with_then_and_then_or() {
        assert_eq!(
            p("A OR B AND C"),
            Expr::Or(vec![p("A"), Expr::And(vec![p("B"), p("C")]),])
        );
        assert_eq!(
            p("A WITH E AND B"),
            Expr::And(vec![
                Expr::Licence {
                    id: "A".into(),
                    exception: Some("E".into())
                },
                p("B"),
            ])
        );
        // So "MIT OR GPL AND X" is permissive through MIT alone.
        assert!(p("MIT OR GPL-3.0-only AND Zlib").permissive());
        assert!(!p("(MIT OR GPL-3.0-only) AND BSD-4-Clause").permissive());
    }

    #[test]
    fn malformed_expressions_are_errors() {
        for bad in [
            "",
            "MIT OR",
            "OR MIT",
            "(MIT",
            "MIT)",
            "MIT Apache-2.0",
            "MIT WITH",
            "MIT, Apache-2.0",
            "MIT AND AND Zlib",
            "MIT; Zlib",
        ] {
            assert!(Expr::parse(bad).is_err(), "{bad:?}");
        }
        let deep = format!(
            "{}MIT{}",
            "(".repeat(MAX_DEPTH + 1),
            ")".repeat(MAX_DEPTH + 1)
        );
        assert!(Expr::parse(&deep).is_err());
        let ok = format!("{}MIT{}", "(".repeat(MAX_DEPTH), ")".repeat(MAX_DEPTH));
        assert_eq!(p(&ok), p("MIT"));
    }

    #[test]
    fn canonical_forms_compare_by_meaning() {
        let same = [
            "MIT OR Apache-2.0",
            "Apache-2.0 OR MIT",
            "MIT/Apache-2.0",
            "(MIT OR Apache-2.0)",
            "MIT OR (Apache-2.0 OR MIT)",
        ];
        for s in same {
            assert_eq!(p(s).canonical(), "Apache-2.0 OR MIT", "{s}");
        }
        assert_eq!(
            p("Unicode-3.0 AND (MIT OR Apache-2.0)").canonical(),
            p("(MIT OR Apache-2.0) AND Unicode-3.0").canonical()
        );
        assert_eq!(
            p("(MIT OR Apache-2.0) AND Unicode-3.0").canonical(),
            "(Apache-2.0 OR MIT) AND Unicode-3.0"
        );
        assert_ne!(p("MIT AND Zlib").canonical(), p("MIT OR Zlib").canonical());
        assert_eq!(
            p("Apache-2.0 WITH LLVM-exception OR MIT").canonical(),
            "Apache-2.0 WITH LLVM-exception OR MIT"
        );
    }

    #[test]
    fn records_are_read_from_the_licences_section_only() {
        let header = "//! Header.\n\
                      //!\n\
                      //! - MIT: `not-a-record` (before the section)\n\
                      //!\n\
                      //! Licences of every outside package:\n\
                      //!\n\
                      //! - MIT OR Apache-2.0: `a`, `b` and\n\
                      //!   `c`.\n\
                      //! - MIT: `d`.\n\
                      //!\n\
                      //! A closing paragraph.\n\
                      #![forbid(unsafe_code)]\n\
                      //! - MIT: `after-the-header`\n";
        let r = records(header).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].packages, ["a", "b", "c"]);
        assert_eq!(r[0].expression.canonical(), "Apache-2.0 OR MIT");
        assert_eq!(r[1].packages, ["d"]);
        assert_eq!(records("//! No section.\n"), Err(RecordError::NoSection));
        assert!(matches!(
            records("//! Licences:\n//! - MIT `a`\n"),
            Err(RecordError::Item(_))
        ));
        assert!(matches!(
            records("//! Licences:\n//! - MIT: no names\n"),
            Err(RecordError::Item(_))
        ));
        assert!(matches!(
            records("//! Licences:\n//! - MIT OR: `a`\n"),
            Err(RecordError::Expr(_))
        ));
        assert_eq!(records("//! Licences: none.\n"), Ok(vec![]));
    }
}
