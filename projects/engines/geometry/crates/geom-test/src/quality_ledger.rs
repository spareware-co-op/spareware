//! The quality ledger: every surviving mutant the kernel keeps, with why, every `coverage(off)`
//! mark, with the proof it cites, and every foundation line still waiting for its proof.
//!
//! What it does: [`Ledger::parse`] reads the ledger file
//! (`projects/engines/geometry/quality-ledger.json`, [`LEDGER_PATH`]) strictly. The mutation gate
//! ([`crate::mutants`]) matches every surviving mutant against [`Ledger::survivors`]; the coverage
//! gate ([`crate::coverage`]) matches every `coverage(off)` mark against [`Ledger::coverage_off`].
//!
//! A survivor entry names one mutant the way cargo-mutants does, without its line and column,
//! which move whenever code above it changes:
//!
//! - `file`: the source file, relative to the repository root, with `/` separators;
//! - `mutant`: cargo-mutants' description of the change, the part of its mutant name after
//!   `file:line:column: `, for example `replace > with >= in coarsen`;
//! - `line_text`: the trimmed text of the line where the mutated code starts. Editing that line
//!   changes the key, so a changed line is judged again rather than excused by an old reason;
//! - `occurrences`: how many mutants share this key (the same change on identical lines of one
//!   function), at least 1;
//! - `kind`: [`Kind`];
//! - `proof`: for a `proved` entry only, the Kani harness that catches the mutant,
//!   `<module path>::proofs::<name>` in the file's crate, a `#[kani::proof]` function of the
//!   mutated file itself;
//! - `reason`: why, judged through the crate's public interface, at least [`MIN_REASON`]
//!   characters.
//!
//! A `coverage_off` entry names one mark: `file`, `item` (the trimmed first line of the item it
//! marks), `proof` (`<path>::<name>`, the Kani harness or Verus proof function that shows the code
//! cannot be reached, which the mark's comment cites too), `tool` (`kani` or `verus`) and
//! `reason`.
//!
//! An `awaiting_proof` entry names a line of the foundation set the tests do not run yet, kept
//! only until the board piece it names proves the line unreachable or tests it: the coverage gate
//! (piece G21) landed before the Kani proofs of piece G41, which in turn need it.
//! Its fields are `file`, `line_text` (the trimmed text of the line, as for survivors),
//! `occurrences` (how many missed lines of the file have that text, at least 1), `piece` (the
//! board piece that removes it, such as `G41`) and `reason`. The coverage gate
//! ([`crate::coverage::excuse_awaiting`]) accepts exactly those missed lines and branches, and
//! fails an entry that matches any other number of them, so the list can only shrink. The field
//! may be left out when it is empty.
//!
//! Guarantees: the file is read strictly, so a typo cannot drop an entry silently. An unknown or
//! missing field, a wrong type, an empty or short reason, an unknown kind or tool, an absolute or
//! backslashed path, a zero occurrence count and two entries with one key are all
//! [`LedgerError`]s, never a smaller ledger.
//!
//! Method: JSON (RFC 8259, <https://www.rfc-editor.org/rfc/rfc8259>) read with `serde_json` into its
//! value tree and checked field by field.
//!
//! Cost: linear in the size of the file.

use core::fmt;
use std::collections::BTreeSet;

use serde_json::{Map, Value};

/// The ledger's path, relative to the repository root.
pub const LEDGER_PATH: &str = "projects/engines/geometry/quality-ledger.json";

/// The schema name the file's `schema` field must hold.
pub const SCHEMA: &str = "spare-kit-quality-ledger/1";

/// The shortest reason accepted, in characters: long enough that it must say something.
pub const MIN_REASON: usize = 40;

/// Why a survivor is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// No input through the crate's public interface can tell the mutant from the original: the
    /// change cannot alter behaviour. The only kind the foundation set may keep for a missed
    /// mutant (28.8).
    Equivalent,
    /// The mutant changes behaviour only in something the crate does not promise (outside the
    /// foundation set only).
    Harmless,
    /// The mutant makes the tests hang, and cargo-mutants' timeout is how it is caught: the reason
    /// says what never terminates. Allowed everywhere, because a hang is a failure CI reports.
    Hangs,
    /// A Kani proof of the mutated function's own contract fails on the mutant: the mutant is
    /// caught by the proof rather than by a test (28.8; owner decision of 2026-10-30). The entry's
    /// `proof` names the harness, which kernel-fuzz.yml proves on every run; the reason says what
    /// the contract is and that the harness was shown to fail with the mutant's change applied.
    /// Allowed everywhere, for a missed mutant.
    Proved,
    /// A survivor recorded but not yet explained, outside the foundation set only: an old gap the
    /// weekly run found, so that pull requests fail only on new survivors (28.8, "Elsewhere, no new
    /// unexplained survivor may merge").
    Known,
}

impl Kind {
    /// The kind's name in the ledger.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Kind::Equivalent => "equivalent",
            Kind::Harmless => "harmless",
            Kind::Hangs => "hangs",
            Kind::Known => "known",
            Kind::Proved => "proved",
        }
    }

    fn parse(text: &str) -> Option<Kind> {
        [
            Kind::Equivalent,
            Kind::Harmless,
            Kind::Hangs,
            Kind::Known,
            Kind::Proved,
        ]
        .into_iter()
        .find(|k| k.name() == text)
    }
}

/// One kept survivor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Survivor {
    /// The source file, relative to the repository root.
    pub file: String,
    /// cargo-mutants' description of the change.
    pub mutant: String,
    /// The trimmed text of the line where the mutated code starts.
    pub line_text: String,
    /// How many mutants share this key.
    pub occurrences: u64,
    /// Why it is kept.
    pub kind: Kind,
    /// For [`Kind::Proved`] only: the Kani harness that catches it, `<path>::<name>`.
    pub proof: Option<String>,
    /// The written reason.
    pub reason: String,
}

impl Survivor {
    /// The key a mutant is matched by: (file, mutant, line text).
    #[must_use]
    pub fn key(&self) -> (String, String, String) {
        (
            self.file.clone(),
            self.mutant.clone(),
            self.line_text.clone(),
        )
    }
}

/// The tool a proof is checked with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tool {
    /// A Kani bounded proof: `#[kani::proof]` on the function.
    Kani,
    /// A Verus proof function: `proof fn`.
    Verus,
}

impl Tool {
    /// The tool's name in the ledger.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Tool::Kani => "kani",
            Tool::Verus => "verus",
        }
    }
}

/// One `coverage(off)` mark and the proof it cites.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageOffEntry {
    /// The source file, relative to the repository root.
    pub file: String,
    /// The trimmed first line of the item the mark applies to.
    pub item: String,
    /// The proof, `<path>::<name>`.
    pub proof: String,
    /// The tool that checks the proof.
    pub tool: Tool,
    /// Why the code cannot be reached, in words.
    pub reason: String,
}

/// One kind of foundation line the tests do not run yet, waiting for the piece that proves it
/// unreachable or tests it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwaitingProof {
    /// The source file, relative to the repository root.
    pub file: String,
    /// The trimmed text of the line.
    pub line_text: String,
    /// How many missed lines of the file have this text.
    pub occurrences: u64,
    /// The board piece that removes the entry, such as `G41`.
    pub piece: String,
    /// Why the tests do not run the line, in words.
    pub reason: String,
}

/// The whole ledger.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ledger {
    /// Kept survivors, in file order.
    pub survivors: Vec<Survivor>,
    /// `coverage(off)` marks, in file order.
    pub coverage_off: Vec<CoverageOffEntry>,
    /// Foundation lines waiting for their proofs, in file order.
    pub awaiting_proof: Vec<AwaitingProof>,
}

/// Why the ledger could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerError {
    /// Where in the file: `survivors, entry 3.reason` and so on.
    pub at: String,
    /// What is wrong.
    pub why: String,
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ledger {}: {}", self.at, self.why)
    }
}

impl std::error::Error for LedgerError {}

fn err(at: &str, why: impl Into<String>) -> LedgerError {
    LedgerError {
        at: at.to_owned(),
        why: why.into(),
    }
}

/// An object's fields, refusing any field not in `allowed`.
fn object<'a>(
    value: &'a Value,
    at: &str,
    allowed: &[&str],
) -> Result<&'a Map<String, Value>, LedgerError> {
    let map = value
        .as_object()
        .ok_or_else(|| err(at, "is not an object"))?;
    if let Some(extra) = map.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(err(at, format!("unknown field {extra:?}")));
    }
    Ok(map)
}

fn string(map: &Map<String, Value>, at: &str, field: &str) -> Result<String, LedgerError> {
    let here = format!("{at}.{field}");
    let text = map
        .get(field)
        .ok_or_else(|| err(&here, "is missing"))?
        .as_str()
        .ok_or_else(|| err(&here, "is not a string"))?;
    if text.trim().is_empty() {
        return Err(err(&here, "is empty"));
    }
    if text.trim() != text {
        return Err(err(&here, "has leading or trailing spaces"));
    }
    Ok(text.to_owned())
}

fn path(map: &Map<String, Value>, at: &str, field: &str) -> Result<String, LedgerError> {
    let text = string(map, at, field)?;
    let here = format!("{at}.{field}");
    if text.contains('\\') {
        return Err(err(&here, "uses \\; write paths with /"));
    }
    if text.starts_with('/')
        || text.contains(':')
        || text
            .split('/')
            .any(|c| c == ".." || c == "." || c.is_empty())
    {
        return Err(err(
            &here,
            "is not a plain path relative to the repository root",
        ));
    }
    Ok(text)
}

fn reason(map: &Map<String, Value>, at: &str) -> Result<String, LedgerError> {
    let text = string(map, at, "reason")?;
    if text.chars().count() < MIN_REASON {
        return Err(err(
            &format!("{at}.reason"),
            format!("is shorter than {MIN_REASON} characters: say why"),
        ));
    }
    Ok(text)
}

fn occurrences(map: &Map<String, Value>, at: &str) -> Result<u64, LedgerError> {
    map.get("occurrences")
        .ok_or_else(|| err(&format!("{at}.occurrences"), "is missing"))?
        .as_u64()
        .filter(|&n| n >= 1)
        .ok_or_else(|| {
            err(
                &format!("{at}.occurrences"),
                "is not a whole number of at least 1",
            )
        })
}

fn awaiting(value: &Value, at: &str) -> Result<AwaitingProof, LedgerError> {
    let map = object(
        value,
        at,
        &["file", "line_text", "occurrences", "piece", "reason"],
    )?;
    let piece = string(map, at, "piece")?;
    let number = piece
        .strip_prefix('G')
        .map(|rest| rest.trim_end_matches(|c: char| c.is_ascii_lowercase()));
    if !number.is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
        return Err(err(
            &format!("{at}.piece"),
            format!("{piece:?} is not a board piece such as G41"),
        ));
    }
    Ok(AwaitingProof {
        file: path(map, at, "file")?,
        line_text: string(map, at, "line_text")?,
        occurrences: occurrences(map, at)?,
        piece,
        reason: reason(map, at)?,
    })
}

fn survivor(value: &Value, at: &str) -> Result<Survivor, LedgerError> {
    let map = object(
        value,
        at,
        &[
            "file",
            "mutant",
            "line_text",
            "occurrences",
            "kind",
            "proof",
            "reason",
        ],
    )?;
    let occurrences = occurrences(map, at)?;
    let kind_text = string(map, at, "kind")?;
    let kind = Kind::parse(&kind_text).ok_or_else(|| {
        err(
            &format!("{at}.kind"),
            format!("{kind_text:?} is not equivalent, harmless, hangs, known or proved"),
        )
    })?;
    let proof = match (kind, map.contains_key("proof")) {
        (Kind::Proved, true) => Some(proof_path(map, at)?),
        (Kind::Proved, false) => return Err(err(&format!("{at}.proof"), "is missing")),
        (_, true) => {
            return Err(err(
                &format!("{at}.proof"),
                "is given, but only a proved entry names a proof",
            ));
        }
        (_, false) => None,
    };
    Ok(Survivor {
        file: path(map, at, "file")?,
        mutant: string(map, at, "mutant")?,
        line_text: string(map, at, "line_text")?,
        occurrences,
        kind,
        proof,
        reason: reason(map, at)?,
    })
}

/// A `proof` field: `<path>::<name>`, the name an identifier.
fn proof_path(map: &Map<String, Value>, at: &str) -> Result<String, LedgerError> {
    let proof = string(map, at, "proof")?;
    let Some((file, name)) = proof.rsplit_once("::") else {
        return Err(err(&format!("{at}.proof"), "is not <path>::<name>"));
    };
    if file.is_empty() || name.is_empty() || !name.chars().all(|c| c == '_' || c.is_alphanumeric())
    {
        return Err(err(&format!("{at}.proof"), "is not <path>::<name>"));
    }
    Ok(proof)
}

fn coverage_off(value: &Value, at: &str) -> Result<CoverageOffEntry, LedgerError> {
    let map = object(value, at, &["file", "item", "proof", "tool", "reason"])?;
    let tool_text = string(map, at, "tool")?;
    let tool = match tool_text.as_str() {
        "kani" => Tool::Kani,
        "verus" => Tool::Verus,
        other => {
            return Err(err(
                &format!("{at}.tool"),
                format!("{other:?} is not kani or verus"),
            ));
        }
    };
    let proof = proof_path(map, at)?;
    Ok(CoverageOffEntry {
        file: path(map, at, "file")?,
        item: string(map, at, "item")?,
        proof,
        tool,
        reason: reason(map, at)?,
    })
}

impl Ledger {
    /// Reads a ledger from its JSON text.
    ///
    /// # Errors
    ///
    /// [`LedgerError`] for anything but a well-formed ledger (see the module documentation).
    pub fn parse(text: &str) -> Result<Ledger, LedgerError> {
        let root: Value =
            serde_json::from_str(text).map_err(|e| err("(file)", format!("is not JSON: {e}")))?;
        let map = object(
            &root,
            "(file)",
            &[
                "schema",
                "about",
                "survivors",
                "coverage_off",
                "awaiting_proof",
            ],
        )?;
        let schema = string(map, "(file)", "schema")?;
        if schema != SCHEMA {
            return Err(err("schema", format!("is {schema:?}, not {SCHEMA:?}")));
        }
        let list = |field: &str| -> Result<&Vec<Value>, LedgerError> {
            map.get(field)
                .ok_or_else(|| err(field, "is missing"))?
                .as_array()
                .ok_or_else(|| err(field, "is not a list"))
        };
        let mut ledger = Ledger::default();
        for (i, value) in list("survivors")?.iter().enumerate() {
            ledger
                .survivors
                .push(survivor(value, &format!("survivors, entry {i}"))?);
        }
        for (i, value) in list("coverage_off")?.iter().enumerate() {
            ledger
                .coverage_off
                .push(coverage_off(value, &format!("coverage_off, entry {i}"))?);
        }
        if let Some(value) = map.get("awaiting_proof") {
            let entries = value
                .as_array()
                .ok_or_else(|| err("awaiting_proof", "is not a list"))?;
            for (i, value) in entries.iter().enumerate() {
                ledger
                    .awaiting_proof
                    .push(awaiting(value, &format!("awaiting_proof, entry {i}"))?);
            }
        }
        let mut keys = BTreeSet::new();
        for (i, s) in ledger.survivors.iter().enumerate() {
            if !keys.insert(s.key()) {
                return Err(err(
                    &format!("survivors, entry {i}"),
                    "has the same file, mutant and line_text as an earlier entry: merge them and \
                     count occurrences",
                ));
            }
        }
        let mut marks = BTreeSet::new();
        for (i, c) in ledger.coverage_off.iter().enumerate() {
            if !marks.insert((c.file.clone(), c.item.clone())) {
                return Err(err(
                    &format!("coverage_off, entry {i}"),
                    "has the same file and item as an earlier entry",
                ));
            }
        }
        let mut waiting = BTreeSet::new();
        for (i, a) in ledger.awaiting_proof.iter().enumerate() {
            if !waiting.insert((a.file.clone(), a.line_text.clone())) {
                return Err(err(
                    &format!("awaiting_proof, entry {i}"),
                    "has the same file and line_text as an earlier entry: merge them and count \
                     occurrences",
                ));
            }
        }
        Ok(ledger)
    }

    /// Reads the ledger at `repo`/[`LEDGER_PATH`] or any other path.
    ///
    /// # Errors
    ///
    /// [`LedgerError`] if the file cannot be read or is not a well-formed ledger.
    pub fn read(path: &std::path::Path) -> Result<Ledger, LedgerError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| err("(file)", format!("{} cannot be read: {e}", path.display())))?;
        Ledger::parse(&text)
    }
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

    const REASON: &str = "Both arms return the same value for every input the type allows.";

    fn with_survivor(fields: &str) -> String {
        format!("{{\"schema\": \"{SCHEMA}\", \"survivors\": [{{{fields}}}], \"coverage_off\": []}}")
    }

    fn good_fields() -> String {
        format!(
            "\"file\": \"a/src/b.rs\", \"mutant\": \"replace > with >= in f\", \"line_text\": \
             \"if x > 1 {{\", \"occurrences\": 1, \"kind\": \"equivalent\", \"reason\": \"{REASON}\""
        )
    }

    #[test]
    fn a_good_ledger_reads() {
        let ledger = Ledger::parse(&with_survivor(&good_fields())).unwrap();
        assert_eq!(ledger.survivors.len(), 1);
        assert_eq!(ledger.survivors[0].kind, Kind::Equivalent);
    }

    #[test]
    fn a_proved_entry_names_its_harness() {
        let fields = good_fields().replace(
            "\"kind\": \"equivalent\"",
            "\"kind\": \"proved\", \"proof\": \"math::f::proofs::f_is_exact\"",
        );
        let ledger = Ledger::parse(&with_survivor(&fields)).unwrap();
        assert_eq!(ledger.survivors[0].kind, Kind::Proved);
        assert_eq!(
            ledger.survivors[0].proof.as_deref(),
            Some("math::f::proofs::f_is_exact")
        );
    }

    #[test]
    fn every_malformation_is_an_error() {
        let good = good_fields();
        let cases = [
            good.replace("\"occurrences\": 1", "\"occurrences\": 0"),
            good.replace("\"occurrences\": 1", "\"occurrences\": -1"),
            good.replace("equivalent", "fine"),
            good.replace(REASON, "short"),
            good.replace("a/src/b.rs", "a\\\\src\\\\b.rs"),
            good.replace("a/src/b.rs", "/a/src/b.rs"),
            good.replace("a/src/b.rs", "a/../b.rs"),
            good.replace("\"line_text\"", "\"line\""),
            format!("{good}, \"extra\": 1"),
            good.replace("\"mutant\": \"replace > with >= in f\", ", ""),
            // A proof only on a proved entry, and always on one.
            good.replace(
                "\"kind\": \"equivalent\"",
                "\"kind\": \"equivalent\", \"proof\": \"m::proofs::p\"",
            ),
            good.replace("equivalent", "proved"),
            good.replace(
                "\"kind\": \"equivalent\"",
                "\"kind\": \"proved\", \"proof\": \"p\"",
            ),
            good.replace(
                "\"kind\": \"equivalent\"",
                "\"kind\": \"proved\", \"proof\": \"m::proofs::\"",
            ),
        ];
        for case in cases {
            assert!(Ledger::parse(&with_survivor(&case)).is_err(), "{case}");
        }
        let dup = format!(
            "{{\"schema\": \"{SCHEMA}\", \"survivors\": [{{{good}}}, {{{good}}}], \"coverage_off\": []}}"
        );
        assert!(Ledger::parse(&dup).is_err());
        assert!(
            Ledger::parse("{\"schema\": \"x\", \"survivors\": [], \"coverage_off\": []}").is_err()
        );
        assert!(
            Ledger::parse(&format!("{{\"schema\": \"{SCHEMA}\", \"survivors\": []}}")).is_err()
        );
    }

    fn with_awaiting(entries: &str) -> String {
        format!(
            "{{\"schema\": \"{SCHEMA}\", \"survivors\": [], \"coverage_off\": [], \
             \"awaiting_proof\": [{entries}]}}"
        )
    }

    fn awaiting_fields() -> String {
        format!(
            "{{\"file\": \"a/src/b.rs\", \"line_text\": \"return None;\", \"occurrences\": 2, \
             \"piece\": \"G41\", \"reason\": \"{REASON}\"}}"
        )
    }

    #[test]
    fn awaiting_entries_read_strictly() {
        let good = awaiting_fields();
        let ledger = Ledger::parse(&with_awaiting(&good)).unwrap();
        assert_eq!(ledger.awaiting_proof.len(), 1);
        assert_eq!(ledger.awaiting_proof[0].occurrences, 2);
        assert_eq!(ledger.awaiting_proof[0].piece, "G41");
        assert!(Ledger::parse(&with_awaiting(&good.replace("\"G41\"", "\"G41a\""))).is_ok());
        for bad in [
            good.replace("\"G41\"", "\"41\""),
            good.replace("\"G41\"", "\"G\""),
            good.replace("\"G41\"", "\"G4x1\""),
            good.replace("\"occurrences\": 2", "\"occurrences\": 0"),
            good.replace(REASON, "short"),
            good.replace("a/src/b.rs", "/a/src/b.rs"),
            good.replace("\"piece\"", "\"pieces\""),
            format!("{good}, {good}"),
        ] {
            assert!(Ledger::parse(&with_awaiting(&bad)).is_err(), "{bad}");
        }
        // Left out, it is empty.
        let none = format!("{{\"schema\": \"{SCHEMA}\", \"survivors\": [], \"coverage_off\": []}}");
        assert_eq!(Ledger::parse(&none).unwrap().awaiting_proof, Vec::new());
    }
}
