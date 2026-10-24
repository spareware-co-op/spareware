//! The golden digests: for every benchmark, the SHA-256 of its inputs and of its results, recorded
//! in `golden.txt` beside this file.
//!
//! Every harness checks them (chapter 23.13, "Output hash: each benchmark checks its output's
//! content hash against the golden value, so no benchmark gets faster by being wrong"):
//!
//! - the instruction-count gate, after each measured run, outside the counted region;
//! - the WebAssembly fuel gate, for every benchmark, on wasm32;
//! - the wall-clock harness, before timing each benchmark;
//! - `tests/golden.rs`, on every platform and build profile `cargo test` runs.
//!
//! The inputs are pinned as well as the results, so a batch cannot change unnoticed, and a digest
//! recorded on one platform must come out the same on every other (principle 5).
//!
//! The file has one line per benchmark: its name, the hexadecimal SHA-256 of its inputs and of its
//! results, separated by single spaces. Lines starting with `#` are comments.
//!
//! Guarantees: [`check`] passes only when a benchmark has an entry and both of its digests equal
//! the recorded ones, and otherwise says which differs; a malformed line, or a second entry for a
//! name, fails [`parse`] and names the line, so no benchmark can have two recorded values.
//!
//! Method: golden hashes of canonical outputs, as chapters 21 and 28.13 of the kernel architecture
//! specify them, with SHA-256 (NIST FIPS 180-4, <https://csrc.nist.gov/pubs/fips/180-4/upd1/final>).
//!
//! Cost: [`parse`] is O(n log n) for a file of n lines (each name is checked against those before
//! it in an ordered set); [`check`] parses it and scans its entries, once per benchmark, always
//! outside the measured region.

use std::collections::BTreeSet;

use crate::harness::Digests;
use crate::sha256::Digest;

/// The recorded golden digests (see the module documentation for the format).
pub const GOLDEN: &str = include_str!("golden.txt");

/// The header of `golden.txt`, written by [`render`].
const HEADER: &str = "\
# Golden digests of every geom-bench benchmark (chapter 23.13, \"Output hash\").
# One line per benchmark: its name, the SHA-256 of its inputs and the SHA-256 of its results, each
# over the canonical encoding of geom_bench::canonical. tests/golden.rs prints the whole file when a
# digest changes; replace this file with it only when the change is intended, and say why in the
# pull request.
";

/// One recorded line: a benchmark's name and digests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry<'a> {
    /// The benchmark's name, `group/op/class`.
    pub name: &'a str,
    /// Its recorded digests.
    pub digests: Digests,
}

/// Parses a golden file. Fails, naming the line, on anything but comments, blank lines and
/// well-formed entries, and on a second entry for a name already recorded.
///
/// # Errors
///
/// A message naming the first malformed or repeated line.
pub fn parse(text: &str) -> Result<Vec<Entry<'_>>, String> {
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split(' ');
        let parsed = match (fields.next(), fields.next(), fields.next(), fields.next()) {
            (Some(name), Some(inputs), Some(outputs), None) if !name.is_empty() => {
                Digest::from_hex(inputs)
                    .zip(Digest::from_hex(outputs))
                    .map(|(inputs, outputs)| Entry {
                        name,
                        digests: Digests { inputs, outputs },
                    })
            }
            _ => None,
        };
        let Some(entry) = parsed else {
            return Err(format!("golden.txt line {}: malformed: {line}", index + 1));
        };
        if !seen.insert(entry.name) {
            return Err(format!("golden.txt line {}: repeated: {line}", index + 1));
        }
        entries.push(entry);
    }
    Ok(entries)
}

/// The recorded digests of the benchmark `name`, if it has an entry.
#[must_use]
pub fn lookup(name: &str) -> Option<Digests> {
    let entries = parse(GOLDEN).ok()?;
    entries
        .into_iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.digests)
}

/// Checks a benchmark's digests against its recorded ones.
///
/// # Errors
///
/// A message saying what differs: no entry, the inputs, or the results.
pub fn check(name: &str, actual: &Digests) -> Result<(), String> {
    let recorded = parse(GOLDEN)?
        .into_iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.digests)
        .ok_or_else(|| format!("{name}: no golden digests recorded in golden.txt"))?;
    if recorded.inputs != actual.inputs {
        return Err(format!(
            "{name}: the inputs changed (recorded {}, now {})",
            recorded.inputs, actual.inputs
        ));
    }
    if recorded.outputs != actual.outputs {
        return Err(format!(
            "{name}: the results changed (recorded {}, now {}): the operation computes something \
             else",
            recorded.outputs, actual.outputs
        ));
    }
    Ok(())
}

/// A whole golden file for these benchmarks, header included.
#[must_use]
pub fn render<'a>(rows: impl IntoIterator<Item = (&'a str, Digests)>) -> String {
    let mut out = String::from(HEADER);
    for (name, digests) in rows {
        out.push_str(&format!("{name} {} {}\n", digests.inputs, digests.outputs));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sha256::Sha256;

    #[test]
    fn rendering_and_parsing_agree() {
        let d = Digests {
            inputs: Sha256::digest(b"in"),
            outputs: Sha256::digest(b"out"),
        };
        let text = render([("a/b/typical", d), ("growth/x/l2", d)]);
        let parsed = parse(&text).unwrap();
        assert_eq!(
            parsed,
            vec![
                Entry {
                    name: "a/b/typical",
                    digests: d
                },
                Entry {
                    name: "growth/x/l2",
                    digests: d
                }
            ]
        );
    }

    #[test]
    fn malformed_lines_are_refused() {
        let d = Sha256::digest(b"x");
        assert!(parse(&format!("a {d}")).is_err());
        assert!(parse(&format!("a {d} {d} extra")).is_err());
        assert!(parse(&format!("a {d} 1234")).is_err());
        assert!(parse(&format!(" {d} {d}")).is_err());
        assert!(parse("# only a comment\n\n").unwrap().is_empty());
        // A second entry for one name, with the same digests or others, is refused.
        let e = Sha256::digest(b"y");
        assert!(parse(&format!("a {d} {d}\na {d} {d}")).is_err());
        let text = format!("a {d} {d}\nb {d} {d}\na {e} {e}");
        assert_eq!(
            parse(&text),
            Err(format!("golden.txt line 3: repeated: a {e} {e}"))
        );
    }

    #[test]
    fn the_recorded_file_parses() {
        assert!(!parse(GOLDEN).unwrap().is_empty());
    }
}
