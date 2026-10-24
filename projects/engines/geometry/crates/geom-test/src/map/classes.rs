//! Chapter 29.2's licence classes: what the kernel may do with code under a licence, read from a
//! provenance record's licence expression.
//!
//! What it does: [`of`] reads a licence as a provenance record writes it (an SPDX expression, or
//! `public domain`) and returns its [`Class`]: on the allow-list (code may be ported), readable but
//! never ported, readable only under the method-only procedure, readable only with an owner's yes,
//! or never read. A licence identifier the table does not classify is an error naming it, so a
//! record cannot pass with a word that is no licence (`TBD`, `unknown`, a file name).
//!
//! Guarantees:
//!
//! - **Chapter 29.2's table, exactly.** [`ALLOWED`] is 29.2's allow-list ("these SPDX identifiers,
//!   and no others, may appear ... in a ported file"), with `Apache-2.0 WITH LLVM-exception` its
//!   one exception form. LGPL in any version, with or without exceptions, is read only under the
//!   method-only procedure (rule 6); GPL in any version, with or without exceptions, is not read
//!   unless a kernel owner allowed it, and then under rule 6; AGPL, non-commercial terms and custom
//!   terms (any `LicenseRef-`) are never read. [`READABLE`] are the documentation and data
//!   licences 29.2 and rule 9 name, and `public domain`, which 29.2 takes as PORT METHOD only.
//! - **Real identifiers only.** Every identifier in the tables is on the SPDX License List 3.29.0,
//!   deprecated forms included, as consulted below; an identifier outside the tables is
//!   [`Unclassified`], never a pass, and adding one needs a row in 29.2 first (rule 1: a source not
//!   in the ledger gets a row; rule 11: an unclear licence is a reason to stop and ask).
//! - **A choice takes the most permissive way; a conjunction the strictest.** An `OR` is read as
//!   its most permissive operand (rule 3: "we take the permissive option"), an `AND` as its
//!   strictest (rule 1: "the strictest applies"), and an unclassified identifier anywhere makes
//!   the whole expression unclassified.
//!
//! Method: chapter 29.2's table and allow-list and 29.20's rules 1, 3, 4 and 6
//! (`projects/engines/geometry/architecture/29-sources-licences.md`), applied to the parse tree of
//! an SPDX license expression ([`crate::licence::Expr`], SPDX specification 2.3, annex D,
//! <https://spdx.github.io/spdx-spec/v2.3/SPDX-license-expressions/>).
//!
//! Cost: linear in the expression's length, times a search of the tables, which are fixed.
//!
//! Consulted: SPDX License List 3.29.0, CC-BY-3.0,
//!   <https://github.com/spdx/license-list-data/blob/31ba1a50e5397e00a304dbadc76531740e89ee48/json/licenses.json>,
//!   to check that every identifier in the tables below is a real SPDX licence identifier, and to
//!   list the deprecated GPL, LGPL and AGPL forms and the non-commercial licences.

use crate::licence::Expr;

/// What chapter 29.2 lets the kernel do with code under a licence, from the most permissive class
/// to the strictest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    /// On 29.2's allow-list: read it, port it with its notice, depend on it.
    Allowed,
    /// Read it, cite it, never port it: public-domain code (taken as PORT METHOD) and the
    /// documentation and data licences 29.2 and rule 9 name.
    Readable,
    /// LGPL: read only under the method-only procedure (29.20, rule 6), never taken.
    MethodOnly,
    /// GPL: not read unless a kernel owner allowed it in chapter 29's row; then rule 6 applies.
    OwnerOnly,
    /// AGPL, non-commercial or custom terms: never read.
    Never,
}

impl Class {
    /// The class's name, as `HEADERS.md` writes it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Class::Allowed => "allowed",
            Class::Readable => "readable",
            Class::MethodOnly => "method only",
            Class::OwnerOnly => "owner only",
            Class::Never => "never read",
        }
    }
}

/// A licence identifier, or an identifier and exception, that the tables do not classify.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unclassified {
    /// The identifier as written, with ` WITH <exception>` when it has one.
    pub id: String,
}

/// How a record writes a public-domain dedication, which has no SPDX identifier (29.2).
pub const PUBLIC_DOMAIN: &str = "public domain";

/// Chapter 29.2's allow-list, by SPDX identifier.
pub const ALLOWED: [&str; 14] = [
    "MIT",
    "MIT-0",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSL-1.0",
    "ISC",
    "Zlib",
    "0BSD",
    "Unlicense",
    "CC0-1.0",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "MPL-2.0",
];

/// The `WITH` forms on 29.2's allow-list.
pub const ALLOWED_WITH: [(&str, &str); 1] = [("Apache-2.0", "LLVM-exception")];

/// Documentation and data licences that 29.2 and rule 9 let the kernel read and cite: CC BY
/// (rule 9's data, and the SPDX specification's), CC BY-SA (`CommonMark`'s text) and CC BY-ND
/// (IFC 4.3's documentation).
pub const READABLE: [&str; 4] = ["CC-BY-3.0", "CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-ND-4.0"];

/// LGPL in every version, deprecated identifiers included.
pub const LGPL: [&str; 12] = [
    "LGPL-2.0-only",
    "LGPL-2.0-or-later",
    "LGPL-2.1-only",
    "LGPL-2.1-or-later",
    "LGPL-3.0-only",
    "LGPL-3.0-or-later",
    "LGPL-2.0",
    "LGPL-2.0+",
    "LGPL-2.1",
    "LGPL-2.1+",
    "LGPL-3.0",
    "LGPL-3.0+",
];

/// GPL in every version, deprecated identifiers and the deprecated forms with an exception
/// included.
pub const GPL: [&str; 19] = [
    "GPL-1.0-only",
    "GPL-1.0-or-later",
    "GPL-2.0-only",
    "GPL-2.0-or-later",
    "GPL-3.0-only",
    "GPL-3.0-or-later",
    "GPL-1.0",
    "GPL-1.0+",
    "GPL-2.0",
    "GPL-2.0+",
    "GPL-3.0",
    "GPL-3.0+",
    "GPL-2.0-with-GCC-exception",
    "GPL-2.0-with-autoconf-exception",
    "GPL-2.0-with-bison-exception",
    "GPL-2.0-with-classpath-exception",
    "GPL-2.0-with-font-exception",
    "GPL-3.0-with-GCC-exception",
    "GPL-3.0-with-autoconf-exception",
];

/// Licences whose code is never read: AGPL in every version, and non-commercial terms (every
/// Creative Commons non-commercial licence, and `PolyForm`'s).
pub const NEVER: [&str; 32] = [
    "AGPL-1.0",
    "AGPL-1.0-only",
    "AGPL-1.0-or-later",
    "AGPL-3.0",
    "AGPL-3.0-only",
    "AGPL-3.0-or-later",
    "CC-BY-NC-1.0",
    "CC-BY-NC-2.0",
    "CC-BY-NC-2.5",
    "CC-BY-NC-3.0",
    "CC-BY-NC-3.0-DE",
    "CC-BY-NC-3.0-IGO",
    "CC-BY-NC-4.0",
    "CC-BY-NC-ND-1.0",
    "CC-BY-NC-ND-2.0",
    "CC-BY-NC-ND-2.5",
    "CC-BY-NC-ND-3.0",
    "CC-BY-NC-ND-3.0-DE",
    "CC-BY-NC-ND-3.0-IGO",
    "CC-BY-NC-ND-4.0",
    "CC-BY-NC-SA-1.0",
    "CC-BY-NC-SA-2.0",
    "CC-BY-NC-SA-2.0-DE",
    "CC-BY-NC-SA-2.0-FR",
    "CC-BY-NC-SA-2.0-UK",
    "CC-BY-NC-SA-2.5",
    "CC-BY-NC-SA-3.0",
    "CC-BY-NC-SA-3.0-DE",
    "CC-BY-NC-SA-3.0-IGO",
    "CC-BY-NC-SA-4.0",
    "PolyForm-Noncommercial-1.0.0",
    "PolyForm-Small-Business-1.0.0",
];

/// The prefix of a licence with custom terms (SPDX's user-defined reference), which 29.2 never
/// reads unless a choice offers another licence.
pub const CUSTOM_PREFIX: &str = "LicenseRef-";

/// The class of one identifier, with its exception.
fn one(id: &str, exception: Option<&str>) -> Result<Class, Unclassified> {
    let unclassified = || Unclassified {
        id: match exception {
            Some(e) => format!("{id} WITH {e}"),
            None => id.to_owned(),
        },
    };
    let custom = id.starts_with(CUSTOM_PREFIX)
        || id.split_once(':').is_some_and(|(doc, rest)| {
            doc.starts_with("DocumentRef-") && rest.starts_with(CUSTOM_PREFIX)
        });
    let base = if ALLOWED.contains(&id) {
        Class::Allowed
    } else if READABLE.contains(&id) {
        Class::Readable
    } else if LGPL.contains(&id) {
        Class::MethodOnly
    } else if GPL.contains(&id) {
        Class::OwnerOnly
    } else if NEVER.contains(&id) || custom {
        Class::Never
    } else {
        return Err(unclassified());
    };
    match (base, exception) {
        // "With or without (linking) exceptions": an exception changes neither class (29.2).
        (_, None) | (Class::MethodOnly | Class::OwnerOnly | Class::Never, Some(_)) => Ok(base),
        (_, Some(e)) if ALLOWED_WITH.contains(&(id, e)) => Ok(Class::Allowed),
        (_, Some(_)) => Err(unclassified()),
    }
}

/// The class of a parsed expression: an `OR` its most permissive operand, an `AND` its strictest.
///
/// # Errors
///
/// [`Unclassified`] for the first identifier the tables do not classify.
pub fn of_expr(expr: &Expr) -> Result<Class, Unclassified> {
    match expr {
        Expr::Licence { id, exception } => one(id, exception.as_deref()),
        Expr::Or(terms) => {
            let classes = terms.iter().map(of_expr).collect::<Result<Vec<_>, _>>()?;
            Ok(classes.into_iter().min().unwrap_or(Class::Never))
        }
        Expr::And(terms) => {
            let classes = terms.iter().map(of_expr).collect::<Result<Vec<_>, _>>()?;
            Ok(classes.into_iter().max().unwrap_or(Class::Never))
        }
    }
}

/// Why a licence as a record writes it cannot be classified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LicenceError {
    /// It is neither `public domain` nor an SPDX expression.
    Syntax(String),
    /// It names an identifier the tables do not classify.
    Unclassified(Unclassified),
}

/// The class of a licence as a provenance record writes it: `public domain`, or an SPDX
/// expression.
///
/// # Errors
///
/// [`LicenceError`] when it does not parse or names an identifier the tables do not classify.
pub fn of(licence: &str) -> Result<Class, LicenceError> {
    if licence == PUBLIC_DOMAIN {
        return Ok(Class::Readable);
    }
    let expr = Expr::parse(licence).map_err(|e| LicenceError::Syntax(e.why))?;
    of_expr(&expr).map_err(LicenceError::Unclassified)
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

    #[test]
    fn the_table_of_29_2() {
        for (licence, class) in [
            ("MIT", Class::Allowed),
            ("MIT OR Apache-2.0", Class::Allowed),
            ("Apache-2.0 WITH LLVM-exception", Class::Allowed),
            ("MPL-2.0 AND BSD-3-Clause", Class::Allowed),
            ("MIT OR GPL-2.0-only", Class::Allowed),
            ("public domain", Class::Readable),
            ("CC-BY-4.0", Class::Readable),
            ("MIT AND CC-BY-4.0", Class::Readable),
            (
                "LGPL-3.0-or-later OR LicenseRef-Commercial",
                Class::MethodOnly,
            ),
            ("LGPL-2.1-or-later", Class::MethodOnly),
            ("GPL-3.0-only", Class::OwnerOnly),
            (
                "GPL-2.0-only WITH Classpath-exception-2.0",
                Class::OwnerOnly,
            ),
            ("MIT AND GPL-3.0-only", Class::OwnerOnly),
            ("AGPL-3.0-only", Class::Never),
            ("PolyForm-Noncommercial-1.0.0", Class::Never),
            ("CC-BY-NC-4.0", Class::Never),
            ("LicenseRef-Proprietary", Class::Never),
            ("GPL-3.0-only AND AGPL-3.0-only", Class::Never),
        ] {
            assert_eq!(of(licence), Ok(class), "{licence}");
        }
    }

    #[test]
    fn words_that_are_no_licence_are_errors() {
        for (licence, id) in [
            ("FPU.h", "FPU.h"),
            ("unknown", "unknown"),
            ("TBD", "TBD"),
            ("MIT OR TBD", "TBD"),
            ("MIT WITH Some-exception", "MIT WITH Some-exception"),
            ("NOASSERTION", "NOASSERTION"),
        ] {
            assert_eq!(
                of(licence),
                Err(LicenceError::Unclassified(Unclassified { id: id.into() })),
                "{licence}"
            );
        }
        assert!(matches!(of("not a licence!"), Err(LicenceError::Syntax(_))));
        assert!(matches!(of(""), Err(LicenceError::Syntax(_))));
    }

    #[test]
    fn the_tables_do_not_overlap() {
        let all: Vec<&str> = ALLOWED
            .iter()
            .chain(&READABLE)
            .chain(&LGPL)
            .chain(&GPL)
            .chain(&NEVER)
            .copied()
            .collect();
        let mut sorted = all.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), all.len());
    }
}
