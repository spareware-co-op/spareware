//! Licence expressions and the record of licence choices (piece G12; chapter 29.2; 29.20, rule 3).
//!
//! What it does: reads a crate's SPDX licence expression (`MIT OR Apache-2.0`,
//! `(MIT OR Apache-2.0) AND Unicode-3.0`, `Apache-2.0 WITH LLVM-exception`) into the ways one may
//! comply with it (`ways`): each way a set of licences that must all be kept, the expression
//! offering a choice when it has more than one. And it holds `LICENCE_CHOICES`, the gate's record
//! of the choice taken for every crate in the kernel family's graph whose licence offers one.
//! 29.20, rule 3: "Where a crate offers a choice, such as `Apache-2.0 OR GPL-2.0-only`, we take
//! the permissive option and record the choice in the gate." cargo-deny accepts such a crate when
//! one way is allowed, and records nothing; the gate's own rule (`deny::rules`) refuses every crate
//! with a choice the record does not hold, under its exact expression, or whose recorded choice is
//! not one of its ways or not on 29.2's allow-list, and every row of the record no crate needs.
//!
//! Each row records the first of 29.2's identifiers, in the chapter's order, that the expression
//! offers (so `MIT` where `MIT` is offered): a permissive licence, as the rule asks, chosen the same
//! way every time. Changing a row is a decision a kernel owner makes; the notices a shipped
//! artefact carries (rule 4, piece "Third-party notices") follow the row.
//!
//! What it guarantees: an expression this reader cannot read has no ways, so the rules refuse it,
//! never pass it. The old crates.io form `MIT/Apache-2.0` reads as `MIT OR Apache-2.0`, as
//! cargo-deny reads it.
//!
//! Method: the SPDX License Expression grammar (SPDX specification 2.3, annex D,
//! <https://spdx.github.io/spdx-spec/v2.3/SPDX-license-expressions/>): `WITH` binds tighter than
//! `AND`, which binds tighter than `OR`; parsed by Dijkstra's shunting-yard into postfix
//! (Dijkstra 1961, <https://www.cs.utexas.edu/~EWD/MCReps/MR35.PDF>) and evaluated with a stack,
//! `AND` the product of its sides' ways and `OR` their union. No recursion.
//!
//! Cost: linear in the expression for the parse; the ways of `AND` multiply, and a crate's
//! expression holds a handful of terms.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

use crate::deny::ALLOW_LIST_29_2;

/// One way to comply with an expression: the licences (each an identifier, or `id WITH exception`)
/// that must all be kept.
pub type Way = BTreeSet<String>;

/// A token of an expression.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Term(String),
    With,
    And,
    Or,
    Open,
    Close,
}

/// The tokens of an expression; `None` when a word is not one SPDX allows.
fn tokens(expression: &str) -> Option<Vec<Token>> {
    let spaced = expression
        .replace('(', " ( ")
        .replace(')', " ) ")
        .replace('/', " OR ");
    spaced
        .split_whitespace()
        .map(|word| match word {
            "(" => Some(Token::Open),
            ")" => Some(Token::Close),
            "AND" => Some(Token::And),
            "OR" => Some(Token::Or),
            "WITH" => Some(Token::With),
            id if id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '+' | ':')) =>
            {
                Some(Token::Term(id.to_owned()))
            }
            _ => None,
        })
        .collect()
}

/// How tightly an operator binds.
fn precedence(token: &Token) -> u8 {
    match token {
        Token::With => 3,
        Token::And => 2,
        Token::Or => 1,
        _ => 0,
    }
}

/// The ways to comply with `expression`, each sorted, sorted; `None` when it cannot be read.
pub fn ways(expression: &str) -> Option<Vec<Way>> {
    // Shunting-yard into postfix.
    let mut postfix = Vec::new();
    let mut operators: Vec<Token> = Vec::new();
    for token in tokens(expression)? {
        match token {
            Token::Term(_) => postfix.push(token),
            Token::Open => operators.push(token),
            Token::Close => loop {
                match operators.pop()? {
                    Token::Open => break,
                    op => postfix.push(op),
                }
            },
            op => {
                while operators
                    .last()
                    .is_some_and(|top| *top != Token::Open && precedence(top) >= precedence(&op))
                {
                    postfix.push(operators.pop()?);
                }
                operators.push(op);
            }
        }
    }
    while let Some(op) = operators.pop() {
        if op == Token::Open {
            return None;
        }
        postfix.push(op);
    }
    // Evaluation: a `WITH` joins two terms into one; `AND` and `OR` combine ways.
    let mut stack: Vec<Vec<Way>> = Vec::new();
    let mut terms: Vec<Option<String>> = Vec::new();
    for token in postfix {
        match token {
            Token::Term(id) => {
                terms.push(Some(id.clone()));
                stack.push(vec![BTreeSet::from([id])]);
            }
            Token::With => {
                let exception = terms.pop()??;
                let licence = terms.pop()??;
                stack.pop()?;
                stack.pop()?;
                let term = format!("{licence} WITH {exception}");
                stack.push(vec![BTreeSet::from([term])]);
                terms.push(None);
            }
            op => {
                let right = stack.pop()?;
                let left = stack.pop()?;
                terms.pop()?;
                terms.pop()?;
                terms.push(None);
                let combined = if op == Token::Or {
                    left.into_iter().chain(right).collect()
                } else {
                    let mut product = Vec::new();
                    for l in &left {
                        for r in &right {
                            product.push(l.union(r).cloned().collect());
                        }
                    }
                    product
                };
                stack.push(combined);
            }
        }
    }
    let mut result = stack.pop()?;
    if !stack.is_empty() {
        return None;
    }
    result.sort();
    result.dedup();
    Some(result)
}

/// Whether every licence of a way is on 29.2's allow-list.
pub fn allowed(way: &Way) -> bool {
    way.iter()
        .all(|term| ALLOW_LIST_29_2.contains(&term.as_str()))
}

/// The way a recorded choice names: its licences joined by `AND`.
pub fn chosen_way(chosen: &str) -> Option<Way> {
    let mut found = ways(chosen)?;
    (found.len() == 1).then(|| found.remove(0))
}

/// The gate's record of licence choices (29.20, rule 3): every crate in the kernel family's graph
/// whose licence offers a choice, its expression exactly as its manifest gives it, and the licence
/// (or licences, joined by `AND`) taken: the first of 29.2's identifiers, in its order, offered.
pub const LICENCE_CHOICES: [(&str, &str, &str); 82] = [
    ("anstyle", "MIT OR Apache-2.0", "MIT"),
    ("anyhow", "MIT OR Apache-2.0", "MIT"),
    ("autocfg", "Apache-2.0 OR MIT", "MIT"),
    ("bit-set", "Apache-2.0 OR MIT", "MIT"),
    ("bit-vec", "Apache-2.0 OR MIT", "MIT"),
    ("bitflags", "MIT OR Apache-2.0", "MIT"),
    ("borsh", "MIT OR Apache-2.0", "MIT"),
    ("cc", "MIT OR Apache-2.0", "MIT"),
    ("cfg-if", "MIT OR Apache-2.0", "MIT"),
    ("chacha20", "MIT OR Apache-2.0", "MIT"),
    ("clap", "MIT OR Apache-2.0", "MIT"),
    ("clap_builder", "MIT OR Apache-2.0", "MIT"),
    ("clap_lex", "MIT OR Apache-2.0", "MIT"),
    ("condtype", "MIT OR Apache-2.0", "MIT"),
    ("core_detect", "MIT/Apache-2.0", "MIT"),
    ("cpufeatures", "MIT OR Apache-2.0", "MIT"),
    ("dashu-base", "MIT OR Apache-2.0", "MIT"),
    ("dashu-int", "MIT OR Apache-2.0", "MIT"),
    ("dashu-ratio", "MIT OR Apache-2.0", "MIT"),
    ("divan", "MIT OR Apache-2.0", "MIT"),
    ("divan-macros", "MIT OR Apache-2.0", "MIT"),
    ("either", "MIT OR Apache-2.0", "MIT"),
    ("either-or-both", "Apache-2.0 OR MIT", "MIT"),
    ("equivalent", "Apache-2.0 OR MIT", "MIT"),
    ("errno", "MIT OR Apache-2.0", "MIT"),
    ("find-msvc-tools", "MIT OR Apache-2.0", "MIT"),
    ("getrandom", "MIT OR Apache-2.0", "MIT"),
    ("gungraun", "Apache-2.0 OR MIT", "MIT"),
    ("gungraun-common", "Apache-2.0 OR MIT", "MIT"),
    ("gungraun-macros", "Apache-2.0 OR MIT", "MIT"),
    ("gungraun-runner", "Apache-2.0 OR MIT", "MIT"),
    ("hashbrown", "MIT OR Apache-2.0", "MIT"),
    ("indexmap", "Apache-2.0 OR MIT", "MIT"),
    ("itoa", "MIT OR Apache-2.0", "MIT"),
    ("lazy_static", "MIT OR Apache-2.0", "MIT"),
    ("libc", "MIT OR Apache-2.0", "MIT"),
    (
        "linux-raw-sys",
        "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
        "MIT",
    ),
    ("memchr", "Unlicense OR MIT", "MIT"),
    ("num-bigint", "MIT OR Apache-2.0", "MIT"),
    ("num-integer", "MIT OR Apache-2.0", "MIT"),
    ("num-rational", "MIT OR Apache-2.0", "MIT"),
    ("num-traits", "MIT OR Apache-2.0", "MIT"),
    ("once_cell", "MIT OR Apache-2.0", "MIT"),
    ("ppv-lite86", "MIT OR Apache-2.0", "MIT"),
    ("proc-macro-crate", "MIT OR Apache-2.0", "MIT"),
    ("proc-macro-error-attr3", "MIT OR Apache-2.0", "MIT"),
    ("proc-macro-error3", "MIT OR Apache-2.0", "MIT"),
    ("proc-macro2", "MIT OR Apache-2.0", "MIT"),
    ("proptest", "MIT OR Apache-2.0", "MIT"),
    ("quote", "MIT OR Apache-2.0", "MIT"),
    ("r-efi", "MIT OR Apache-2.0 OR LGPL-2.1-or-later", "MIT"),
    ("rand", "MIT OR Apache-2.0", "MIT"),
    ("rand_chacha", "MIT OR Apache-2.0", "MIT"),
    ("rand_core", "MIT OR Apache-2.0", "MIT"),
    ("rand_xorshift", "MIT OR Apache-2.0", "MIT"),
    ("rand_xoshiro", "MIT OR Apache-2.0", "MIT"),
    ("regex-lite", "MIT OR Apache-2.0", "MIT"),
    ("regex-syntax", "MIT OR Apache-2.0", "MIT"),
    ("robust", "MIT OR Apache-2.0", "MIT"),
    ("rustc_version", "MIT OR Apache-2.0", "MIT"),
    (
        "rustix",
        "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
        "MIT",
    ),
    ("semver", "MIT OR Apache-2.0", "MIT"),
    ("serde", "MIT OR Apache-2.0", "MIT"),
    ("serde_core", "MIT OR Apache-2.0", "MIT"),
    ("serde_derive", "MIT OR Apache-2.0", "MIT"),
    ("serde_json", "MIT OR Apache-2.0", "MIT"),
    ("shlex", "MIT OR Apache-2.0", "MIT"),
    ("static_assertions", "MIT OR Apache-2.0", "MIT"),
    ("syn", "MIT OR Apache-2.0", "MIT"),
    ("terminal_size", "MIT OR Apache-2.0", "MIT"),
    ("toml_datetime", "MIT OR Apache-2.0", "MIT"),
    ("toml_edit", "MIT OR Apache-2.0", "MIT"),
    ("toml_parser", "MIT OR Apache-2.0", "MIT"),
    ("unarray", "MIT OR Apache-2.0", "MIT"),
    (
        "unicode-ident",
        "(MIT OR Apache-2.0) AND Unicode-3.0",
        "MIT AND Unicode-3.0",
    ),
    ("unty-next", "MIT OR Apache-2.0", "MIT"),
    (
        "wasip2",
        "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
        "MIT",
    ),
    ("windows-link", "MIT OR Apache-2.0", "MIT"),
    ("windows-sys", "MIT OR Apache-2.0", "MIT"),
    (
        "wit-bindgen",
        "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
        "MIT",
    ),
    ("zerocopy", "BSD-2-Clause OR Apache-2.0 OR MIT", "MIT"),
    (
        "zerocopy-derive",
        "BSD-2-Clause OR Apache-2.0 OR MIT",
        "MIT",
    ),
];

/// The choice the record's rule would take for `expression`: the first of 29.2's identifiers, in
/// its order, that each part of a way offers, for the first way all of whose licences are allowed.
/// Used to propose a row; the record itself is what the gate checks.
pub fn proposed_choice(expression: &str) -> Option<String> {
    let rank = |way: &Way| {
        let mut ranks: Vec<usize> = way
            .iter()
            .map(|t| {
                ALLOW_LIST_29_2
                    .iter()
                    .position(|a| *a == t.as_str())
                    .unwrap_or(usize::MAX)
            })
            .collect();
        ranks.sort_unstable();
        ranks
    };
    let ways = ways(expression)?;
    let best = ways.iter().filter(|w| allowed(w)).min_by_key(|w| rank(w))?;
    Some(best.iter().cloned().collect::<Vec<_>>().join(" AND "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(expression: &str) -> Vec<String> {
        ways(expression)
            .unwrap()
            .iter()
            .map(|w| w.iter().cloned().collect::<Vec<_>>().join(" AND "))
            .collect()
    }

    #[test]
    fn expressions_read_as_the_ways_to_comply() {
        assert_eq!(shown("MIT"), ["MIT"]);
        assert_eq!(shown("MIT OR Apache-2.0"), ["Apache-2.0", "MIT"]);
        assert_eq!(shown("MIT/Apache-2.0"), ["Apache-2.0", "MIT"]);
        assert_eq!(
            shown("(MIT OR Apache-2.0) AND Unicode-3.0"),
            ["Apache-2.0 AND Unicode-3.0", "MIT AND Unicode-3.0"]
        );
        assert_eq!(
            shown("Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT"),
            ["Apache-2.0", "Apache-2.0 WITH LLVM-exception", "MIT"]
        );
        // AND binds tighter than OR.
        assert_eq!(shown("MIT OR ISC AND Zlib"), ["ISC AND Zlib", "MIT"]);
        for unreadable in [
            "",
            "MIT OR",
            "(MIT",
            "MIT)",
            "MIT AND AND ISC",
            "MIT, ISC",
            "WITH",
        ] {
            assert_eq!(ways(unreadable), None, "{unreadable}");
        }
    }

    #[test]
    fn every_recorded_choice_is_one_way_on_the_allow_list() {
        for (krate, expression, chosen) in LICENCE_CHOICES {
            let ways = ways(expression).unwrap_or_else(|| panic!("{krate}: {expression}"));
            assert!(ways.len() > 1, "{krate}: {expression} offers no choice");
            let way = chosen_way(chosen).unwrap();
            assert!(ways.contains(&way) && allowed(&way), "{krate}: {chosen}");
            assert_eq!(
                proposed_choice(expression).as_deref(),
                Some(chosen),
                "{krate}"
            );
        }
        let names: BTreeSet<&str> = LICENCE_CHOICES.iter().map(|(k, _, _)| *k).collect();
        assert_eq!(names.len(), LICENCE_CHOICES.len(), "a crate recorded twice");
        // A choice the allow-list leaves no way through has no proposal.
        assert_eq!(proposed_choice("GPL-2.0-only OR AGPL-3.0-only"), None);
        assert_eq!(
            proposed_choice("Apache-2.0 OR GPL-2.0-only").as_deref(),
            Some("Apache-2.0")
        );
    }
}
