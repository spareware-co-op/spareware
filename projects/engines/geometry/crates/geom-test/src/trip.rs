//! Seeded violations for the gates that run an outside tool on the kernel's own code (gate 1,
//! piece G43): each such gate is shown to fail when a violation is planted on purpose.
//!
//! What it does: gate 1 (chapter 30.3) asks that every merge gate fail on a seeded violation, the
//! pattern of the kernel's merge gate (G12) and of the toolchain guard (G20, [`crate::guard`]).
//! Most gates already meet it in their own tests (gate 1's record in chapter 32 lists each). The
//! gates here are the ones whose verdict is an outside tool's: rustfmt, Verus, Kani, libFuzzer,
//! `ClusterFuzzLite`, Gungraun's Callgrind comparison, Wasmtime's fuel, Gappa with Rocq, and
//! HOL Light. [`VIOLATIONS`] holds one planted violation for each: a file of the kernel, one line
//! in it found exactly once ([`Violation::anchor`]), the line it becomes, and the text the tool
//! prints when it catches it.
//! The binary `trip` writes the changed line into the working tree, runs the gate's own command,
//! restores the file byte for byte, and asks [`verdict`] whether the run failed for that reason.
//! Each workflow job runs it with its real check, so a job that could no longer fail (a tool that
//! stopped checking, a command that checks nothing) fails instead. The two count gates run it
//! last, comparing the planted run with the pull request's own run measured just before; the
//! `ClusterFuzzLite` gate runs it in a job of its own, `clusterfuzzlite-seeded`, because its actions
//! build a fresh clone of the pull request, which a planted line never reaches; and HOL Light's
//! runs on orient2d's leg of its matrix alone.
//!
//! Guarantees:
//!
//! - **Each violation is real and small.** One line changes, and the change breaks exactly the
//!   property the gate checks: a line rustfmt would format differently; a sign the Kani harness
//!   for signs asserts; a carry the Verus twin's proof of addition asserts; a bound the limb proofs
//!   assert; orient2d's exact path answering `Positive` whatever its points, which breaks the
//!   antisymmetry every orient2d harness checks (a sign flipped everywhere would not: the fuzz
//!   target holds orient2d to its laws, not to an oracle); the carry the wide-integer fuzz target
//!   compares with big integers under cargo-bolero's libFuzzer, and which `ClusterFuzzLite`'s own
//!   build and fuzzing must report through whichever target meets it first (each target's law
//!   names its own break, "broken law: "; its address sanitizer reports the abort that follows);
//!   `Sign::of_i128` reading its argument through an opaque copy
//!   (`core::hint::black_box`), the same answer for more work, which both count gates must report
//!   as a rise of more than 5% on its benchmark; the error bound orient2d's Gappa problem claims,
//!   halved, which Gappa must fail to prove, so that its Rocq output holds no theorem and Rocq
//!   refuses it; and the bounds of orient2d's `FPTaylor` certificate, every one halved as HOL
//!   Light's session loads it, which HOL Light must refuse to prove.
//! - **An anchor found once, or no run.** [`apply`] refuses a file that holds the anchor line
//!   other than exactly once, so a refactor that moves it fails `tests/attack_g43.rs` (which
//!   checks every anchor in the repository) and the job, loudly, instead of planting nothing.
//! - **A trip counts only for the right reason.** [`verdict`] accepts a run only when the command
//!   failed and printed every line [`Violation::caught`] names: the tool's own words for a found
//!   violation. A build error, a missing tool or any other failure proves nothing and is refused.
//! - **Line ends kept.** The anchor is matched on a line with its line end set aside, so a
//!   Windows checkout's CRLF files work the same, and the line end is kept.
//!
//! Method: the seeded-violation pattern of the kernel's merge gate (chapter 28.8,
//! `projects/engines/geometry/architecture/28-testing.md`; pieces G12 and G20, `guard`): plant one
//! violation, run the real tool as the gate runs it, and require the tool's own report of that
//! violation. Gate 1's condition is chapter 30.3's (`projects/engines/geometry/architecture/30-build-order.md`).
//!
//! Cost: linear in the file's length and the run's output.

use core::fmt;

/// One planted violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Violation {
    /// The name `trip` takes.
    pub name: &'static str,
    /// The workflow and job whose check it proves, as `<file> <job>`.
    pub gate: &'static str,
    /// The file it changes, relative to the repository root.
    pub file: &'static str,
    /// The line it changes, without its line end; found exactly once.
    pub anchor: &'static str,
    /// What the line becomes.
    pub planted: &'static str,
    /// What the tool prints when it catches the violation; every entry must appear.
    pub caught: &'static [&'static str],
}

/// The planted violations, one for each gate that runs an outside tool on the kernel's code.
pub const VIOLATIONS: [Violation; 13] = [
    Violation {
        name: "fmt",
        gate: "kernel.yml lint",
        file: "projects/engines/geometry/crates/geom-num/src/sign.rs",
        anchor: "    pub const fn of_i128(v: i128) -> Sign {",
        planted: "    pub const fn of_i128( v : i128 )->Sign {",
        caught: &["Diff in", "sign.rs"],
    },
    Violation {
        name: "verus",
        gate: "kernel-dev.yml verus",
        file: "projects/engines/geometry/crates/geom-num/verus/wide.rs",
        anchor: "        let (s2, c2) = s1.overflowing_add(u64::from(carry));",
        planted: "        let (s2, c2) = s1.overflowing_sub(u64::from(carry));",
        caught: &["verification results::", "error: assertion failed"],
    },
    Violation {
        name: "kani-num",
        gate: "kernel-fuzz.yml kani-num",
        file: "projects/engines/geometry/crates/geom-num/src/sign.rs",
        anchor: "        } else if v > 0 {",
        planted: "        } else if v > 1 {",
        caught: &["VERIFICATION:- FAILED"],
    },
    Violation {
        name: "kani-op",
        gate: "kernel-fuzz.yml kani-op",
        file: "projects/engines/geometry/crates/geom-op/src/cbor.rs",
        anchor: "        if fraction.trailing_zeros() < format.dropped {",
        planted: "        if fraction.trailing_zeros() + 1 < format.dropped {",
        caught: &["VERIFICATION:- FAILED"],
    },
    Violation {
        name: "kani-format",
        gate: "kernel-fuzz.yml kani-format",
        file: "projects/engines/geometry/crates/geom-format/src/range.rs",
        anchor: "    v.unsigned_abs().bit_width()",
        planted: "    v.unsigned_abs().bit_width() + 1",
        caught: &["VERIFICATION:- FAILED"],
    },
    Violation {
        name: "kani-limbs",
        gate: "kernel-fuzz.yml kani-limbs",
        file: "projects/engines/geometry/crates/geom-num/src/wide32.rs",
        anchor: "        let dropped_ok = parts.iter().skip(L).all(|&p| p == fill);",
        planted: "        let dropped_ok = parts.iter().skip(L.saturating_sub(1)).all(|&p| p == fill);",
        caught: &["VERIFICATION:- FAILED"],
    },
    Violation {
        name: "kani-fuzz",
        gate: "kernel-fuzz.yml kani",
        file: "projects/engines/geometry/crates/geom-num/src/predicate.rs",
        anchor: "    let exact = generated::orient2d::exact(a, b, c);",
        planted: "    let exact = generated::orient2d::exact(a, b, c).map(|_| Sign::Positive);",
        caught: &["VERIFICATION:- FAILED"],
    },
    Violation {
        name: "libfuzzer",
        gate: "kernel-fuzz.yml libfuzzer",
        file: "projects/engines/geometry/crates/geom-num/src/wide.rs",
        anchor: "        let (s2, c2) = s1.overflowing_add(u64::from(carry));",
        planted: "        let (s2, c2) = s1.overflowing_sub(u64::from(carry));",
        caught: &["libFuzzer: deadly signal"],
    },
    Violation {
        name: "clusterfuzzlite",
        gate: "kernel-fuzz.yml clusterfuzzlite-seeded",
        file: "projects/engines/geometry/crates/geom-num/src/wide.rs",
        anchor: "        let (s2, c2) = s1.overflowing_add(u64::from(carry));",
        planted: "        let (s2, c2) = s1.overflowing_sub(u64::from(carry));",
        caught: &["Detected bug.", "Crash is reproducible.", "broken law: "],
    },
    Violation {
        name: "gungraun",
        gate: "kernel-bench.yml instructions",
        file: "projects/engines/geometry/crates/geom-num/src/sign.rs",
        anchor: "        if v < 0 {",
        planted: "        if core::hint::black_box(v) < 0 {",
        caught: &[
            "Performance has regressed: Instructions",
            "instructions::sign::of_i128",
        ],
    },
    Violation {
        name: "gappa",
        gate: "kernel-certificates.yml gappa",
        file: "projects/engines/geometry/crates/geom-gen/generated/gappa-orient2d.g",
        anchor: concat!(
            "  det - e_det // S in [-40564819207303354358693384683511b-156, ",
            r"40564819207303354358693384683511b-156] /\ bound -/ C * S in [-3b-53, 3b-53]",
        ),
        planted: concat!(
            "  det - e_det // S in [-40564819207303354358693384683511b-157, ",
            r"40564819207303354358693384683511b-157] /\ bound -/ C * S in [-3b-53, 3b-53]",
        ),
        caught: &[
            "Error: some properties were not satisfied:",
            "LIN(det - e_det, S), best: ",
            "The reference l1 was not found in the current environment.",
        ],
    },
    Violation {
        name: "hol-light",
        gate: "kernel-certificates.yml fptaylor",
        file: "projects/engines/geometry/crates/geom-gen/certificates/fptaylor.sh",
        anchor: r#"      echo "let _, bound_th, time = validate 10 \"$expression.proof\";;""#,
        planted: concat!(
            r#"      echo "let _, bound_th, time = let p = load \"$expression.proof\" in "#,
            r#"p.proof_opts <- map (fun q -> {q with opt_bounds = map (fun b -> b /. 2.0) "#,
            r#"q.opt_bounds; total_bound = q.total_bound /. 2.0}) p.proof_opts; "#,
            r#"let f, b = validate_proof 10 p in f, b, 0.0;;""#,
        ),
        caught: &["Exception:", r#"Misc_functions.Error ("prove_bound_cond","#],
    },
    Violation {
        name: "fuel",
        gate: "kernel-bench.yml fuel",
        file: "projects/engines/geometry/crates/geom-num/src/sign.rs",
        anchor: "        if v < 0 {",
        planted: "        if core::hint::black_box(v) < 0 {",
        caught: &["**Failed:** ", "| sign/of_i128/typical | "],
    },
];

/// The violation called `name`.
#[must_use]
pub fn violation(name: &str) -> Option<&'static Violation> {
    VIOLATIONS.iter().find(|v| v.name == name)
}

/// Why a violation cannot be planted, or why a run does not prove its gate trips.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TripError {
    /// The file holds the anchor line this many times, not once.
    Anchor(usize),
    /// The run passed: the gate did not see the violation.
    NotTripped,
    /// The run failed without the tool's report of the violation: these lines were missing.
    WrongFailure(Vec<&'static str>),
}

impl fmt::Display for TripError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anchor(n) => write!(
                f,
                "the file holds the anchor line {n} times, not once: move the violation to the \
                 line's new place in geom-test's trip module"
            ),
            Self::NotTripped => write!(f, "the run passed with the violation planted"),
            Self::WrongFailure(missing) => write!(
                f,
                "the run failed, but not by catching the violation: it never printed {missing:?}"
            ),
        }
    }
}

impl std::error::Error for TripError {}

/// `source` with the violation planted: its one anchor line replaced, the line end kept.
///
/// # Errors
///
/// [`TripError::Anchor`] when the anchor line is not in `source` exactly once.
pub fn apply(v: &Violation, source: &str) -> Result<String, TripError> {
    let found = source
        .split_inclusive('\n')
        .filter(|line| line.trim_end_matches(['\r', '\n']) == v.anchor)
        .count();
    if found != 1 {
        return Err(TripError::Anchor(found));
    }
    Ok(source
        .split_inclusive('\n')
        .map(|line| {
            let text = line.trim_end_matches(['\r', '\n']);
            if text == v.anchor {
                let end = line.get(text.len()..).unwrap_or_default();
                format!("{}{end}", v.planted)
            } else {
                line.to_owned()
            }
        })
        .collect())
}

/// Whether a run with the violation planted proves its gate trips: it failed, and printed every
/// line of [`Violation::caught`].
///
/// # Errors
///
/// [`TripError::NotTripped`] when the run passed; [`TripError::WrongFailure`] when it failed
/// without the tool's report.
pub fn verdict(v: &Violation, succeeded: bool, output: &str) -> Result<(), TripError> {
    if succeeded {
        return Err(TripError::NotTripped);
    }
    let missing: Vec<&'static str> = v
        .caught
        .iter()
        .copied()
        .filter(|c| !output.contains(c))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(TripError::WrongFailure(missing))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_changes_and_its_line_end_stays() {
        let v = violation("kani-num").unwrap();
        let lf = "a\n        } else if v > 0 {\nb\n";
        assert_eq!(apply(v, lf).unwrap(), "a\n        } else if v > 1 {\nb\n");
        let crlf = lf.replace('\n', "\r\n");
        assert_eq!(
            apply(v, &crlf).unwrap(),
            "a\r\n        } else if v > 1 {\r\nb\r\n"
        );
        assert_eq!(apply(v, "a\n"), Err(TripError::Anchor(0)));
        let twice = format!("{lf}{lf}");
        assert_eq!(apply(v, &twice), Err(TripError::Anchor(2)));
    }

    #[test]
    fn a_trip_counts_only_for_the_right_reason() {
        let v = violation("kani-num").unwrap();
        assert_eq!(
            verdict(v, true, "VERIFICATION:- FAILED"),
            Err(TripError::NotTripped)
        );
        assert_eq!(
            verdict(v, false, "error[E0308]: mismatched types"),
            Err(TripError::WrongFailure(vec!["VERIFICATION:- FAILED"]))
        );
        assert_eq!(verdict(v, false, "...\nVERIFICATION:- FAILED\n"), Ok(()));
    }

    #[test]
    fn every_violation_has_a_name_a_gate_and_a_change() {
        for (i, v) in VIOLATIONS.iter().enumerate() {
            assert!(!v.caught.is_empty() && v.anchor != v.planted, "{}", v.name);
            assert!(
                VIOLATIONS.iter().skip(i + 1).all(|w| w.name != v.name),
                "{} twice",
                v.name
            );
        }
        assert!(violation("none").is_none());
    }
}
