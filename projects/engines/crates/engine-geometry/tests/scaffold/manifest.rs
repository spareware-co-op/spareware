//! Reading a cargo manifest's lint tables and a clippy configuration: whether a crate forbids unsafe
//! code through `unsafe_code = "forbid"`, and the settings the lint posture check compares.
//!
//! What it does: reads every key of a TOML document in full dotted form, and from them the level of
//! `unsafe_code` in a manifest's `[lints.rust]` table, or in the workspace's `[workspace.lints.rust]`
//! table when the manifest sets `[lints] workspace = true`; every key under `lints` in a manifest;
//! and every key of a clippy configuration, each value in canonical form (`canonical`). The
//! toolchain check reads `rust-toolchain.toml` with the same reader. The merge gate (G12) reads
//! the items of an array (`array_items`) and the text of a string (`string_text`) from those
//! values: the paths of clippy's disallowed lists, and the lists of the cargo-deny configurations.
//!
//! What it guarantees: a lint counts only as a key of a real table, never as text inside a string,
//! an array or a comment; a table header, a dotted key and an inline table name the same key. A
//! file this reader cannot read forbids nothing and has no settings, so a limit of this reader
//! shows as a violation, never as a pass.
//!
//! Method: a reader after TOML 1.0's grammar of keys, tables, strings, arrays and inline tables
//! (<https://toml.io/en/v1.0.0>), as much as a cargo manifest and a clippy configuration use.
//!
//! Cost: linear in the length of the file. It recurses only into inline tables, at most
//! `MAX_TOML_DEPTH` deep.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The kernel lints deny native arithmetic in kernel code (Cargo.toml). This is test code: its
// readers count positions in a text they hold whole, which tests may do with native arithmetic.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "test code; the rule is for kernel code"
)]

use std::collections::BTreeMap;

/// How deep inline tables may nest in a TOML file this reader accepts.
pub const MAX_TOML_DEPTH: usize = 16;

/// Whether a manifest forbids unsafe code through its `[lints.rust]` table, or through the
/// workspace's `[workspace.lints.rust]` table when it sets `[lints] workspace = true`.
pub fn manifest_forbids_unsafe(manifest: &str, workspace_manifest: &str) -> bool {
    let Some(own) = toml_keys(manifest) else {
        return false;
    };
    if let Some(forbids) = lint_forbids(&own, "lints.rust.unsafe_code") {
        return forbids;
    }
    if own.get("lints.workspace").map(String::as_str) != Some("true") {
        return false;
    }
    toml_keys(workspace_manifest)
        .and_then(|root| lint_forbids(&root, "workspace.lints.rust.unsafe_code"))
        .unwrap_or(false)
}

/// The level a lint key sets, in its plain (`"forbid"`) or table (`{ level = "forbid" }`) form,
/// compared with `forbid`; `None` when the key is not set.
fn lint_forbids(keys: &BTreeMap<String, String>, key: &str) -> Option<bool> {
    let level = keys
        .get(key)
        .or_else(|| keys.get(&format!("{key}.level")))?;
    Some(level == "\"forbid\"" || level == "'forbid'")
}

/// Every key of a TOML document with its value: keys in full dotted form (`lints.rust.unsafe_code`,
/// from a table header, a dotted key or an inline table alike), values as written. An array of
/// tables is entered as `name[]`, so its keys never match a plain table's. `None` when the text is
/// not TOML this reader understands.
pub fn toml_keys(text: &str) -> Option<BTreeMap<String, String>> {
    let chars: Vec<char> = text.chars().collect();
    let mut reader = Toml {
        chars: &chars,
        at: 0,
    };
    let mut keys = BTreeMap::new();
    let mut table = String::new();
    loop {
        reader.skip_trivia();
        let Some(c) = reader.peek() else {
            return Some(keys);
        };
        if c == '[' {
            reader.at += 1;
            let array = reader.peek() == Some('[');
            if array {
                reader.at += 1;
            }
            let name = reader.key()?;
            if !reader.eat(']') || (array && !reader.eat(']')) {
                return None;
            }
            reader.end_of_line()?;
            table = if array { format!("{name}[]") } else { name };
        } else {
            let key = join(&table, &reader.key()?);
            if !reader.eat('=') {
                return None;
            }
            reader.value(&key, &mut keys, 0)?;
            reader.end_of_line()?;
        }
    }
}

/// Every key of a TOML document with its value in canonical form (`canonical`); `None` when the text
/// is not TOML this reader understands.
pub fn settings(text: &str) -> Option<BTreeMap<String, String>> {
    toml_keys(text)?
        .into_iter()
        .map(|(key, value)| Some((key, canonical(&value)?)))
        .collect()
}

/// Every key under `lints` of a manifest (`lints.rust.unsafe_code`, `lints.clippy.pedantic.level`
/// and so on), each value in canonical form; `None` when the manifest is not TOML this reader
/// understands.
pub fn lint_settings(manifest: &str) -> Option<BTreeMap<String, String>> {
    let mut keys = settings(manifest)?;
    keys.retain(|key, _| key == "lints" || key.starts_with("lints."));
    Some(keys)
}

/// A value as `toml_keys` gives it, without its comments, without whitespace outside strings and
/// without a comma just before a closing bracket, so two values that differ only in layout read the
/// same. Everything else is kept as written, quotes included: `"deny"` and `'deny'`, or an inline
/// table with its keys in another order, still differ. `None` when the value does not lex.
pub fn canonical(value: &str) -> Option<String> {
    let chars: Vec<char> = value.chars().collect();
    let mut reader = Toml {
        chars: &chars,
        at: 0,
    };
    let mut out = String::new();
    loop {
        reader.skip_trivia();
        let Some(c) = reader.peek() else {
            return Some(out);
        };
        match c {
            '"' | '\'' => {
                let start = reader.at;
                reader.string()?;
                out.extend(chars.get(start..reader.at)?);
            }
            ']' | '}' => {
                if out.ends_with(',') {
                    out.pop();
                }
                out.push(c);
                reader.at += 1;
            }
            _ => {
                out.push(c);
                reader.at += 1;
            }
        }
    }
}

/// The items of an array value as `canonical` writes it (`[a,"b",{k="v"}]`), each as written, in
/// order; `None` when the value is not one array this reader reads. Commas inside strings, inline
/// tables and nested arrays do not split items.
pub fn array_items(value: &str) -> Option<Vec<String>> {
    let chars: Vec<char> = value.chars().collect();
    if chars.first() != Some(&'[') || chars.last() != Some(&']') {
        return None;
    }
    let inner = chars.get(1..chars.len() - 1)?;
    let mut items = Vec::new();
    let mut item = String::new();
    let mut depth = 0usize;
    let mut i = 0;
    while let Some(&c) = inner.get(i) {
        match c {
            '"' | '\'' => {
                // A string runs to its closing quote; a basic string's backslash escapes the
                // next character.
                item.push(c);
                i += 1;
                loop {
                    let d = *inner.get(i)?;
                    item.push(d);
                    i += 1;
                    if d == '\\' && c == '"' {
                        item.push(*inner.get(i)?);
                        i += 1;
                    } else if d == c {
                        break;
                    }
                }
                continue;
            }
            '[' | '{' => depth += 1,
            ']' | '}' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                items.push(std::mem::take(&mut item));
                i += 1;
                continue;
            }
            _ => {}
        }
        item.push(c);
        i += 1;
    }
    if depth != 0 {
        return None;
    }
    if !item.is_empty() {
        items.push(item);
    }
    if items.iter().any(String::is_empty) {
        return None;
    }
    Some(items)
}

/// The text of a one-line string value, either quote: a literal string as it is, a basic string
/// with its `\"` and `\\` escapes undone; `None` for anything else, a basic string with any other
/// escape included, so an unusual value is never read as a name it does not hold.
pub fn string_text(value: &str) -> Option<String> {
    if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        return (!inner.contains('\'')).then(|| inner.to_owned());
    }
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                e @ ('"' | '\\') => out.push(e),
                _ => return None,
            },
            '"' => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

fn join(table: &str, key: &str) -> String {
    if table.is_empty() {
        key.to_owned()
    } else {
        format!("{table}.{key}")
    }
}

struct Toml<'c> {
    chars: &'c [char],
    at: usize,
}

impl Toml<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn starts_with(&self, text: &str) -> bool {
        text.chars()
            .enumerate()
            .all(|(k, c)| self.chars.get(self.at + k) == Some(&c))
    }

    fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.at += 1;
        }
    }

    fn skip_comment(&mut self) {
        if self.peek() == Some('#') {
            while self.peek().is_some_and(|c| c != '\n') {
                self.at += 1;
            }
        }
    }

    /// Skips spaces, line ends and comments.
    fn skip_trivia(&mut self) {
        loop {
            self.skip_spaces();
            self.skip_comment();
            if matches!(self.peek(), Some('\r' | '\n')) {
                self.at += 1;
            } else {
                return;
            }
        }
    }

    /// Skips spaces, then takes `c` if it comes next.
    fn eat(&mut self, c: char) -> bool {
        self.skip_spaces();
        if self.peek() == Some(c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// Only spaces and a comment may follow a key's value or a table header on its line.
    fn end_of_line(&mut self) -> Option<()> {
        self.skip_spaces();
        self.skip_comment();
        if self.peek() == Some('\r') {
            self.at += 1;
        }
        match self.peek() {
            None => Some(()),
            Some('\n') => {
                self.at += 1;
                Some(())
            }
            Some(_) => None,
        }
    }

    /// A dotted key, its parts joined by dots, quotes removed.
    fn key(&mut self) -> Option<String> {
        let mut parts = Vec::new();
        loop {
            self.skip_spaces();
            let part = match self.peek()? {
                '"' | '\'' => {
                    let start = self.at;
                    self.string()?;
                    self.chars[start + 1..self.at - 1]
                        .iter()
                        .collect::<String>()
                }
                _ => {
                    let start = self.at;
                    while self
                        .peek()
                        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    {
                        self.at += 1;
                    }
                    if self.at == start {
                        return None;
                    }
                    self.chars[start..self.at].iter().collect()
                }
            };
            parts.push(part);
            self.skip_spaces();
            if self.peek() == Some('.') {
                self.at += 1;
            } else {
                return Some(parts.join("."));
            }
        }
    }

    /// Skips one string of any of TOML's four kinds, starting at its first quote.
    fn string(&mut self) -> Option<()> {
        let quote = self.peek()?;
        let escapes = quote == '"';
        let triple: String = [quote; 3].iter().collect();
        if self.starts_with(&triple) {
            self.at += 3;
            loop {
                if escapes && self.peek()? == '\\' {
                    self.at += 2;
                } else if self.starts_with(&triple) {
                    self.at += 3;
                    // Up to two more quotes belong to the string's content.
                    for _ in 0..2 {
                        if self.peek() == Some(quote) {
                            self.at += 1;
                        }
                    }
                    return Some(());
                } else {
                    self.at += 1;
                }
            }
        }
        self.at += 1;
        loop {
            match self.peek()? {
                '\\' if escapes => self.at += 2,
                '\n' => return None,
                c if c == quote => {
                    self.at += 1;
                    return Some(());
                }
                _ => self.at += 1,
            }
        }
    }

    /// Reads one value for `key` into `keys`: an inline table key by key, anything else as
    /// written.
    fn value(
        &mut self,
        key: &str,
        keys: &mut BTreeMap<String, String>,
        depth: usize,
    ) -> Option<()> {
        self.skip_spaces();
        let start = self.at;
        match self.peek()? {
            '"' | '\'' => self.string()?,
            '[' => {
                let mut level = 0usize;
                loop {
                    self.skip_trivia();
                    match self.peek()? {
                        '[' => {
                            level += 1;
                            self.at += 1;
                        }
                        ']' => {
                            level -= 1;
                            self.at += 1;
                            if level == 0 {
                                break;
                            }
                        }
                        '"' | '\'' => self.string()?,
                        _ => self.at += 1,
                    }
                }
            }
            '{' => {
                if depth >= MAX_TOML_DEPTH {
                    return None;
                }
                self.at += 1;
                loop {
                    self.skip_trivia();
                    if self.eat('}') {
                        return Some(());
                    }
                    let inner = join(key, &self.key()?);
                    if !self.eat('=') {
                        return None;
                    }
                    self.value(&inner, keys, depth + 1)?;
                    self.skip_trivia();
                    if !self.eat(',') && self.peek() != Some('}') {
                        return None;
                    }
                }
            }
            _ => {
                while self
                    .peek()
                    .is_some_and(|c| !matches!(c, '\r' | '\n' | '#' | ',' | ']' | '}'))
                {
                    self.at += 1;
                }
            }
        }
        let written: String = self.chars[start..self.at].iter().collect();
        let written = written.trim();
        if written.is_empty() {
            return None;
        }
        keys.insert(key.to_owned(), written.to_owned());
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_lints() {
        let own = "[package]\nname = \"a\"\n\n[lints.rust]\nunsafe_code = \"forbid\" # never\n";
        assert!(manifest_forbids_unsafe(own, ""));
        let table_form = "[lints.rust]\nunsafe_code = { level = \"forbid\", priority = 1 }\n";
        assert!(manifest_forbids_unsafe(table_form, ""));
        let dotted = "[lints]\nrust.unsafe_code = \"forbid\"\n";
        assert!(manifest_forbids_unsafe(dotted, ""));
        let inline = "lints = { rust = { unsafe_code = 'forbid' } }\n";
        assert!(manifest_forbids_unsafe(inline, ""));
        let deny = "[lints.rust]\nunsafe_code = \"deny\"\n";
        assert!(!manifest_forbids_unsafe(deny, ""));
        let inherited = "[lints]\nworkspace = true\n";
        let root = "[workspace.lints.rust]\nunsafe_code = \"forbid\"\n";
        assert!(manifest_forbids_unsafe(inherited, root));
        assert!(!manifest_forbids_unsafe(inherited, "[workspace]\n"));
        let other_table = "[lints.clippy]\nunsafe_code = \"forbid\"\n";
        assert!(!manifest_forbids_unsafe(other_table, ""));
    }

    #[test]
    fn a_lint_table_written_inside_a_string_is_text() {
        for quotes in ["\"\"\"", "'''"] {
            let manifest = format!(
                "[package]\nname = \"a\"\ndescription = {quotes}\n[lints.rust]\n\
                 unsafe_code = \"forbid\"\n{quotes}\n"
            );
            assert!(!manifest_forbids_unsafe(&manifest, ""), "{manifest}");
        }
        let in_array = "[package]\nkeywords = [\n  \"[lints.rust]\", # a comment ]\n  \
                        \"unsafe_code = 'forbid'\",\n]\n";
        assert!(!manifest_forbids_unsafe(in_array, ""));
        // What follows the string is read again: the real table counts.
        let after = "[package]\ndescription = \"\"\"\n[x]\n\"\"\"\n\n[lints.rust]\n\
                     unsafe_code = \"forbid\"\n";
        assert!(manifest_forbids_unsafe(after, ""));
        // A manifest this reader cannot read forbids nothing.
        assert!(!manifest_forbids_unsafe(
            "[lints.rust]\nunsafe_code = \"forbid\n",
            ""
        ));
    }

    #[test]
    fn toml_keys_of_a_manifest() {
        let keys = toml_keys(
            "[package]\nname = \"geom-a\" # name\nedition.workspace = true\n\
             [dependencies]\ngeom-num = { path = \"../geom-num\" }\n\
             [[bench]]\nname = \"b\"\n",
        )
        .unwrap();
        assert_eq!(keys["package.name"], "\"geom-a\"");
        assert_eq!(keys["package.edition.workspace"], "true");
        assert_eq!(keys["dependencies.geom-num.path"], "\"../geom-num\"");
        assert_eq!(keys["bench[].name"], "\"b\"");
        let deep = format!("a = {}1{}\n", "{ b = ".repeat(20), " }".repeat(20));
        assert_eq!(toml_keys(&deep), None);
    }

    #[test]
    fn canonical_drops_layout_and_comments_only() {
        let laid_out = "[\n    # max may return either zero.\n    { path = \"f64::max\", \
                        reason = \"# not a comment\" },\n    { path = 'f64::min' },\n]";
        assert_eq!(
            canonical(laid_out).as_deref(),
            Some("[{path=\"f64::max\",reason=\"# not a comment\"},{path='f64::min'}]")
        );
        assert_eq!(
            canonical("{ level = \"warn\", priority = -1 }").as_deref(),
            Some("{level=\"warn\",priority=-1}")
        );
        assert_eq!(canonical("-1").as_deref(), Some("-1"));
        // Quotes, key order and text inside strings are kept.
        assert_ne!(canonical("\"deny\""), canonical("'deny'"));
        assert_ne!(canonical("{ a = 1, b = 2 }"), canonical("{ b = 2, a = 1 }"));
        assert_ne!(canonical("\"a b\""), canonical("\"ab\""));
        // A value that does not lex has no canonical form.
        assert_eq!(canonical("\"open"), None);
    }

    #[test]
    fn array_items_and_string_text() {
        let value = canonical(
            "[\n  \"MIT\", # a comment, with a comma\n  'Apache-2.0 WITH LLVM-exception',\n  \
             { crate = \"rayon\", wrappers = [\"geom-pool\", \"a,b\"] },\n  \"x\\\"]\",\n]",
        )
        .unwrap();
        assert_eq!(
            array_items(&value).unwrap(),
            [
                "\"MIT\"",
                "'Apache-2.0 WITH LLVM-exception'",
                "{crate=\"rayon\",wrappers=[\"geom-pool\",\"a,b\"]}",
                "\"x\\\"]\"",
            ]
        );
        assert_eq!(array_items("[]").unwrap(), Vec::<String>::new());
        for bad in ["", "MIT", "[\"open]", "[a,,b]", "[{a=1]", "[a]]"] {
            assert_eq!(array_items(bad), None, "{bad}");
        }
        assert_eq!(string_text("\"MIT\"").as_deref(), Some("MIT"));
        assert_eq!(string_text("'f64::max'").as_deref(), Some("f64::max"));
        assert_eq!(string_text("\"a\\\"b\\\\\"").as_deref(), Some("a\"b\\"));
        for bad in ["MIT", "\"a\\u0041\"", "\"a", "'a'b'", "true", "\"a\"b\""] {
            assert_eq!(string_text(bad), None, "{bad}");
        }
    }

    #[test]
    fn lint_settings_of_a_manifest() {
        let tables = "[package]\nname = \"a\"\n\n[lints.rust]\nunsafe_code = \"forbid\"\n\n\
                      [lints.clippy]\n# pedantic, below the others\n\
                      pedantic = { level = \"warn\", priority = -1 }\nfloat_cmp = \"allow\"\n";
        let dotted = "lints.rust.unsafe_code = 'forbid'\n[package]\nname = \"b\"\n";
        let got = lint_settings(tables).unwrap();
        let expected: BTreeMap<String, String> = [
            ("lints.rust.unsafe_code", "\"forbid\""),
            ("lints.clippy.pedantic.level", "\"warn\""),
            ("lints.clippy.pedantic.priority", "-1"),
            ("lints.clippy.float_cmp", "\"allow\""),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        assert_eq!(got, expected);
        // Only keys under `lints`; the same key from a dotted key, its quotes as written.
        assert_eq!(
            lint_settings(dotted)
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            [("lints.rust.unsafe_code".to_owned(), "'forbid'".to_owned())]
        );
        // The same settings in another layout, with other comments, read the same.
        let relaid = "[lints]\nclippy.float_cmp = \"allow\" # exact\nrust = { unsafe_code = \"forbid\" }\n\
                      [lints.clippy.pedantic]\npriority = -1\nlevel = \"warn\"\n";
        assert_eq!(lint_settings(relaid).unwrap(), expected);
        // A manifest this reader cannot read has no settings.
        assert_eq!(lint_settings("[lints.rust\n"), None);
        assert_eq!(settings("a = \"open\n"), None);
    }
}
