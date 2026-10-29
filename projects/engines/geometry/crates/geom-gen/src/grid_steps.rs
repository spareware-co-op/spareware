//! Generator `grid-steps`: the grid steps, common units on each step, and the tolerance ladder,
//! exactly, from `geom-num`'s own constants.
//!
//! What it does: [`generate`] writes a Markdown file, [`OUTPUT`], with three tables:
//!
//! 1. each grid step: its size in quarter-nanometres, as a fraction of a millimetre, as exact
//!    decimals in millimetres and nanometres, the steps in a millimetre, how many times finer it is
//!    than the step above, and the frame's range in metres;
//! 2. common units, from the nanometre to the kilometre, inch-based units and the US survey foot:
//!    how many steps each is on each grid step, whether that is a whole number (a unit that is not
//!    whole on a step can put a length between grid points), and `geom-num`'s constant for the
//!    unit where it has one;
//! 3. the tolerance ladder in millimetres on each step, and the angular bound.
//!
//! Why: `geom-num`'s module headers (`grid.rs`, `ladder.rs`) carry hand-written versions of the
//! step and ladder tables. This one is computed from the code: CI fails if a constant changes and
//! the committed file is not regenerated.
//!
//! Guarantees:
//!
//! - **Exact.** Integers and exact fractions only ([`crate::exact`]); a decimal is printed only
//!   when it terminates, with every digit, and a fraction otherwise. Every integer operation is a
//!   checked method or an exact fraction operation, so an overflow is an error, never a wrapped
//!   value.
//! - **From the code.** Every number comes from `GridStep::quarter_nm`, `grid::COORD_MAX`,
//!   `grid::COORD_BITS`, the unit constants (`grid::QUARTER_NM_PER_MM` and the others), the
//!   `ladder` constants and `ladder::quarter_nm`, or from the exact unit definitions below. The
//!   steps are found by walking `GridStep::coarser` to the coarsest step and then `GridStep::finer`
//!   to the finest, so a new step gets its row and column without an edit here. `geom-num` has no
//!   list of its unit and ladder constants, so `UNITS` and [`LADDER`] name them, and
//!   `tests/drift.rs` reads `geom-num`'s `ladder.rs` and `grid.rs` and fails if any public
//!   constant there is missing from the output.
//! - **Checked against the code.** The generator fails rather than print a table that contradicts
//!   `geom-num`: each step's name is `Q<rung>`; each step is 25 times finer than the one above, as
//!   `grid.rs` states; each unit constant is its unit's exact size, and `GridStep::steps_per` gives
//!   exactly the whole counts the table shows, and `None` where the table says "not whole"; each
//!   ladder length agrees with `ladder::quarter_nm`; each commit-distance ceiling bounds the exact
//!   distance committing can move a point, √n half steps in n dimensions; `ladder::ANGULAR` is
//!   exactly 2^`ladder::ANGULAR_LOG2`, bit for bit; and the angular bound is the fit bound divided
//!   by 2^`grid::COORD_BITS`.
//! - **No randomness**, no input, `\n` line ends: the same bytes on every target. A unit test
//!   compares the output with the committed file, built into the test, on every target CI runs,
//!   wasm32 included.
//!
//! Method: the definitions. 1 mm = 10^6 nm = 4,000,000 quarter-nanometres. The inch is 25.4 mm
//! exactly and the US survey foot 1200/3937 m exactly, as NIST Special Publication 811 (2008
//! edition), appendix B, states (<https://www.nist.gov/pml/special-publication-811>). An `f64` is
//! exactly 2^e (for a normal e) when its fraction bits are zero and its biased exponent is
//! e + 1023, by the binary64 format of IEEE 754-2019
//! (<https://standards.ieee.org/ieee/754/6210/>). A point whose n coordinates each move by at most
//! h moves by at most √n · h, by Pythagoras: √(n · h²).
//!
//! Cost: constant; a few hundred exact operations.

use core::fmt::{self, Write as _};

use geom_num::grid::{
    COORD_BITS, COORD_MAX, GridStep, QUARTER_NM_PER_INCH, QUARTER_NM_PER_MICRON,
    QUARTER_NM_PER_MIL, QUARTER_NM_PER_MM, QUARTER_NM_PER_NM, Steps,
};
use geom_num::ladder;

use crate::exact::Ratio;
use crate::generator::GenError;

/// The generator's name.
pub const NAME: &str = "grid-steps";

/// The file it owns, relative to the workspace root.
pub const OUTPUT: &str = "projects/engines/geometry/crates/geom-gen/generated/grid-steps.md";

/// Quarter-nanometres in a nanometre, by the unit's name.
const QNM_PER_NM: u128 = 4;

/// Quarter-nanometres in a millimetre, by definition: 10^6 nm of 4 quarter-nanometres each.
/// `geom-num`'s `grid::QUARTER_NM_PER_MM` is checked against it ([`UNITS`]).
const QNM_PER_MM: u128 = 4_000_000;

/// Quarter-nanometres in a metre: 1,000 mm. A constant, so the compiler evaluates the product and
/// an overflow would stop the build.
const QNM_PER_M: u128 = QNM_PER_MM * 1_000;

/// How many times finer each grid step is than the one above (`grid.rs`: "each exactly 25 times
/// finer than the one above").
const RUNG_RATIO: u128 = 25;

/// The most steps the walk in [`steps`] visits before it gives up: far more than the ladder will
/// ever have, so a cycle in `coarser` or `finer` is an error, not a hang.
const MAX_STEPS: usize = 64;

/// One unit of the `f64` biased exponent field: bit 52, just above the 52 fraction bits. A
/// constant, so the compiler evaluates the shift.
const EXPONENT_UNIT: u64 = 1 << 52;

/// The bias of the `f64` exponent field.
const EXPONENT_BIAS: i64 = 1_023;

/// The biased exponents of the normal `f64`s: 1 to 2046 (0 is zero and the subnormals, 2047 the
/// infinities and NaN).
const NORMAL_EXPONENTS: core::ops::RangeInclusive<u64> = 1..=2_046;

/// One of `geom-num`'s unit constants: its name in `geom_num::grid` and its value.
#[derive(Clone, Copy)]
struct Constant {
    name: &'static str,
    quarter_nm: u64,
}

/// A unit of length, exactly `mm_num / mm_den` millimetres, with `geom-num`'s constant for it if it
/// has one.
struct Unit {
    name: &'static str,
    mm_num: u128,
    mm_den: u128,
    constant: Option<Constant>,
}

/// The common units, smallest first within each family. Inch-based sizes follow from
/// 1 in = 25.4 mm = 127/5 mm; the US survey foot is 1200/3937 m = 1,200,000/3937 mm. Every unit
/// constant of `geom_num::grid` is named here, against the unit it measures.
const UNITS: [Unit; 14] = [
    Unit {
        name: "1 nm",
        mm_num: 1,
        mm_den: 1_000_000,
        constant: Some(Constant {
            name: "QUARTER_NM_PER_NM",
            quarter_nm: QUARTER_NM_PER_NM,
        }),
    },
    Unit {
        name: "1 µm",
        mm_num: 1,
        mm_den: 1_000,
        constant: Some(Constant {
            name: "QUARTER_NM_PER_MICRON",
            quarter_nm: QUARTER_NM_PER_MICRON,
        }),
    },
    Unit {
        name: "0.01 mm",
        mm_num: 1,
        mm_den: 100,
        constant: None,
    },
    Unit {
        name: "0.1 mm",
        mm_num: 1,
        mm_den: 10,
        constant: None,
    },
    Unit {
        name: "1 mm",
        mm_num: 1,
        mm_den: 1,
        constant: Some(Constant {
            name: "QUARTER_NM_PER_MM",
            quarter_nm: QUARTER_NM_PER_MM,
        }),
    },
    Unit {
        name: "1 m",
        mm_num: 1_000,
        mm_den: 1,
        constant: None,
    },
    Unit {
        name: "1 km",
        mm_num: 1_000_000,
        mm_den: 1,
        constant: None,
    },
    Unit {
        name: "1 mil (1/1000 in)",
        mm_num: 127,
        mm_den: 5_000,
        constant: Some(Constant {
            name: "QUARTER_NM_PER_MIL",
            quarter_nm: QUARTER_NM_PER_MIL,
        }),
    },
    Unit {
        name: "1/256 in",
        mm_num: 127,
        mm_den: 1_280,
        constant: None,
    },
    Unit {
        name: "1/64 in",
        mm_num: 127,
        mm_den: 320,
        constant: None,
    },
    Unit {
        name: "1/16 in",
        mm_num: 127,
        mm_den: 80,
        constant: None,
    },
    Unit {
        name: "1 in",
        mm_num: 127,
        mm_den: 5,
        constant: Some(Constant {
            name: "QUARTER_NM_PER_INCH",
            quarter_nm: QUARTER_NM_PER_INCH,
        }),
    },
    Unit {
        name: "1 ft",
        mm_num: 1_524,
        mm_den: 5,
        constant: None,
    },
    Unit {
        name: "1 US survey ft",
        mm_num: 1_200_000,
        mm_den: 3_937,
        constant: None,
    },
];

/// The ladder's whole-step tolerances, with how the table names them, in the table's order. The
/// one rung that is not whole steps, `ladder::COMMIT_PER_AXIS`, has its own row before these.
pub const LADDER: [(&str, Steps); 7] = [
    (
        "Commit distance in 2D, certified ceiling (`ladder::COMMIT_DISTANCE_2D`)",
        ladder::COMMIT_DISTANCE_2D,
    ),
    (
        "Commit distance in 3D, certified ceiling (`ladder::COMMIT_DISTANCE_3D`)",
        ladder::COMMIT_DISTANCE_3D,
    ),
    ("Fit bound (`ladder::FIT`)", ladder::FIT),
    (
        "Per-operation bound (`ladder::PER_OPERATION`)",
        ladder::PER_OPERATION,
    ),
    (
        "Import healing, default (`ladder::HEAL_DEFAULT`)",
        ladder::HEAL_DEFAULT,
    ),
    ("Import healing, cap (`ladder::HEAL_CAP`)", ladder::HEAL_CAP),
    (
        "Short-edge warning (`ladder::SHORT_EDGE_WARNING`)",
        ladder::SHORT_EDGE_WARNING,
    ),
];

/// The commit-distance ceilings and the dimension each bounds: committing moves a point of n
/// coordinates by at most √n half steps.
const COMMIT_CEILINGS: [(&str, u128, Steps); 2] = [
    ("COMMIT_DISTANCE_2D", 2, ladder::COMMIT_DISTANCE_2D),
    ("COMMIT_DISTANCE_3D", 3, ladder::COMMIT_DISTANCE_3D),
];

/// The file's bytes.
///
/// # Errors
///
/// [`GenError::Generator`] if an exact operation overflows or `geom-num`'s constants contradict
/// each other or the definitions.
pub fn generate() -> Result<Vec<u8>, GenError> {
    let steps = steps()?;
    let mut out = String::new();
    header(&mut out);
    steps_table(&mut out, &steps)?;
    units_table(&mut out, &steps)?;
    ladder_table(&mut out, &steps)?;
    Ok(out.into_bytes())
}

fn fail(message: impl Into<String>) -> GenError {
    GenError::generator(NAME, message)
}

/// The error for a failed `write!` into a `String`. `String`'s `fmt::Write` never fails (its
/// `write_str` always returns `Ok`), and every value formatted here is an integer, a string or a
/// [`Ratio`], whose `Display` only writes, so this is never called; it is a typed error, not a
/// panic, all the same.
fn formatting(_: fmt::Error) -> GenError {
    fail("formatting into a String failed, which String never does")
}

fn ratio(num: u128, den: u128) -> Result<Ratio, GenError> {
    Ratio::new(num, den).ok_or_else(|| fail(format!("{num}/{den} has a zero denominator")))
}

fn mul(a: Ratio, b: Ratio) -> Result<Ratio, GenError> {
    a.checked_mul(b)
        .ok_or_else(|| fail(format!("{a} × {b} does not fit in u128")))
}

/// `a × b`, or an error naming `what` if the product does not fit.
fn times(a: u128, b: u128, what: &str) -> Result<u128, GenError> {
    a.checked_mul(b)
        .ok_or_else(|| fail(format!("{what}: {a} × {b} does not fit in u128")))
}

fn coord_max() -> Result<u128, GenError> {
    u128::try_from(COORD_MAX).map_err(|_| fail("COORD_MAX is negative"))
}

/// Every grid step, coarsest first, with its name: found by walking `coarser` from `Q0` to the
/// coarsest step and then `finer` to the finest, and checked against the rules in the module
/// header.
///
/// # Errors
///
/// [`GenError::Generator`] if the walk does not end within [`MAX_STEPS`] steps, a name is not
/// `Q<rung>`, or a step is not exactly 25 times finer than the one above.
pub fn steps() -> Result<Vec<(String, GridStep)>, GenError> {
    let mut coarsest = GridStep::Q0;
    for _ in 0..MAX_STEPS {
        match coarsest.coarser() {
            Some(c) => coarsest = c,
            None => break,
        }
    }
    if coarsest.coarser().is_some() {
        return Err(fail(format!(
            "GridStep::coarser does not end within {MAX_STEPS} steps"
        )));
    }
    let mut out: Vec<(String, GridStep)> = Vec::new();
    let mut step = Some(coarsest);
    while let Some(s) = step {
        if out.len() == MAX_STEPS {
            return Err(fail(format!(
                "GridStep::finer does not end within {MAX_STEPS} steps"
            )));
        }
        let name = format!("{s:?}");
        if name != format!("Q{}", s.rung()) {
            return Err(fail(format!("GridStep::{name} has rung {}", s.rung())));
        }
        if let Some((above_name, above)) = out.last() {
            let (a, q) = (u128::from(above.quarter_nm()), u128::from(s.quarter_nm()));
            if a != times(RUNG_RATIO, q, "the step above")? {
                return Err(fail(format!(
                    "{name} ({q} quarter-nm) is not {RUNG_RATIO} times finer than {above_name} \
                     ({a} quarter-nm)"
                )));
            }
        }
        out.push((name, s));
        step = s.finer();
    }
    Ok(out)
}

fn header(out: &mut String) {
    out.push_str(
        "<!-- Generated by geom-gen, generator `grid-steps` (seed: none, it uses no randomness). -->\n\
         <!-- Do not edit: change projects/engines/geometry/crates/geom-gen/src/grid_steps.rs and run -->\n\
         <!-- `cargo run -p geom-gen -- write`. CI runs `cargo run -p geom-gen -- check` and fails on any difference. -->\n\
         \n\
         # The grid steps, exactly\n\
         \n\
         The steps of the kernel's coordinate grid (chapter 6.1) and its tolerance ladder (chapter 6.5), \
         computed by exact integer arithmetic from `geom-num`'s own constants: `GridStep::quarter_nm`, \
         `grid::COORD_MAX`, `grid::COORD_BITS`, the unit constants, `GridStep::steps_per` and the \
         `ladder` constants. Nothing is rounded: a value is written as a decimal only when its decimal \
         ends, with every digit, and as a fraction otherwise. 1 mm is 4,000,000 quarter-nanometres; \
         the inch is 25.4 mm and the US survey foot 1200/3937 m exactly (NIST Special Publication 811).\n",
    );
}

fn steps_table(out: &mut String, steps: &[(String, GridStep)]) -> Result<(), GenError> {
    let m = coord_max()?;
    writeln!(
        out,
        "\n## The steps\n\
         \n\
         Every coordinate is a whole number of steps within 2^{COORD_BITS} − 1 = {m} steps \
         (`grid::COORD_MAX`) of its frame's origin.\n\
         \n\
         | Step | Quarter-nm | Size (mm) | Size (mm), decimal | Size (nm), decimal | Steps per mm | Step above ÷ this | Frame range, ± (m) |\n\
         | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
    )
    .map_err(formatting)?;
    let mut above: Option<u128> = None;
    for (name, step) in steps {
        let q = u128::from(step.quarter_nm());
        let size_millimetres = ratio(q, QNM_PER_MM)?;
        let size_nanometres = ratio(q, QNM_PER_NM)?;
        let per_mm = ratio(QNM_PER_MM, q)?;
        let finer = match above {
            Some(a) => ratio(a, q)?.exact(),
            None => "—".to_owned(),
        };
        let range_m = mul(ratio(m, 1)?, ratio(q, QNM_PER_M)?)?;
        writeln!(
            out,
            "| {name} | {q} | {} | {size_millimetres} | {size_nanometres} | {per_mm} | {finer} \
             | {range_m} |",
            size_millimetres.fraction()
        )
        .map_err(formatting)?;
        above = Some(q);
    }
    Ok(())
}

fn units_table(out: &mut String, steps: &[(String, GridStep)]) -> Result<(), GenError> {
    let mut head = String::from("| Unit | Size (mm) |");
    let mut rule = String::from("| --- | ---: |");
    for (name, _) in steps {
        write!(head, " {name} |").map_err(formatting)?;
        rule.push_str(" ---: |");
    }
    head.push_str(" `geom-num` constant (quarter-nm) |");
    rule.push_str(" --- |");
    writeln!(
        out,
        "\n## Common units on each step\n\
         \n\
         Steps in one unit. \"Not whole\" means the unit is not a whole number of steps, so a length \
         measured in it can fall between grid points; `GridStep::steps_per` gives `None` for it. The \
         last column names `geom-num`'s constant for the unit, where it has one.\n\
         \n\
         {head}\n\
         {rule}"
    )
    .map_err(formatting)?;
    for u in &UNITS {
        let size = ratio(u.mm_num, u.mm_den)?;
        if let Some(c) = u.constant {
            let by_definition = mul(size, Ratio::whole(QNM_PER_MM))?;
            if by_definition != Ratio::whole(u128::from(c.quarter_nm)) {
                return Err(fail(format!(
                    "grid::{} is {} quarter-nm, but {} is {by_definition} quarter-nm by definition",
                    c.name, c.quarter_nm, u.name
                )));
            }
        }
        let mut row = format!("| {} | {size} |", u.name);
        for (step_name, step) in steps {
            let per_unit = mul(size, ratio(QNM_PER_MM, u128::from(step.quarter_nm()))?)?;
            if let Some(c) = u.constant {
                // In lowest terms, so whole exactly when the denominator is 1, and then the count
                // is the numerator.
                let shown = per_unit.is_whole().then(|| per_unit.numerator());
                let from_code = step.steps_per(c.quarter_nm).map(u128::from);
                if from_code != shown {
                    return Err(fail(format!(
                        "GridStep::{step_name}.steps_per(grid::{}) is {from_code:?}, but {} is \
                         {per_unit} steps of {step_name}",
                        c.name, u.name
                    )));
                }
            }
            if per_unit.is_whole() {
                write!(row, " {per_unit} |").map_err(formatting)?;
            } else {
                write!(row, " not whole: {per_unit} |").map_err(formatting)?;
            }
        }
        match u.constant {
            Some(c) => {
                write!(row, " `grid::{}` = {} |", c.name, c.quarter_nm).map_err(formatting)?;
            }
            None => row.push_str(" — |"),
        }
        row.push('\n');
        out.push_str(&row);
    }
    Ok(())
}

/// 2^`exponent` as the bits of an `f64`, for a normal exponent (−1022..=1023): a zero fraction and
/// the exponent biased by 1023. `None` for any other exponent.
fn power_of_two_bits(exponent: i32) -> Option<u64> {
    let biased = u64::try_from(i64::from(exponent).checked_add(EXPONENT_BIAS)?).ok()?;
    if !NORMAL_EXPONENTS.contains(&biased) {
        return None;
    }
    // At most 2046 < 2^11 units of 2^52: below 2^63, so the product fits; checked all the same.
    biased.checked_mul(EXPONENT_UNIT)
}

/// Checks that each commit-distance ceiling bounds the exact distance: committing moves each of a
/// point's n coordinates by at most h = `ladder::COMMIT_PER_AXIS` half steps, so the point by at
/// most √n · h half steps, which is at most c steps exactly when n · h² ≤ 4 · c², all in whole
/// numbers.
///
/// # Errors
///
/// [`GenError::Generator`] for a ceiling below the exact distance, or an overflow.
fn check_commit_ceilings() -> Result<(), GenError> {
    let h = u128::from(ladder::COMMIT_PER_AXIS.0);
    for (name, n, ceiling) in COMMIT_CEILINGS {
        let c = u128::from(ceiling.get());
        let moved = times(n, times(h, h, name)?, name)?;
        let bound = times(4, times(c, c, name)?, name)?;
        if moved > bound {
            return Err(fail(format!(
                "ladder::{name} ({c} steps) is below the exact commit distance in {n} dimensions, \
                 √{n} × {h} half steps: {n} × {h}² > 4 × {c}²"
            )));
        }
    }
    Ok(())
}

fn ladder_table(out: &mut String, steps: &[(String, GridStep)]) -> Result<(), GenError> {
    let mut head = String::from("| Tolerance | Steps |");
    let mut rule = String::from("| --- | ---: |");
    for (name, _) in steps {
        write!(head, " {name} (mm) |").map_err(formatting)?;
        rule.push_str(" ---: |");
    }
    writeln!(
        out,
        "\n## The tolerance ladder\n\
         \n\
         Every tolerance in the kernel, in steps of the frame and in millimetres on each step.\n\
         \n\
         {head}\n\
         {rule}"
    )
    .map_err(formatting)?;

    // Committing an implicit point moves each coordinate by at most COMMIT_PER_AXIS half steps.
    let half_steps = ladder::COMMIT_PER_AXIS.0;
    let commit = ratio(u128::from(half_steps), 2)?;
    let mut row = format!(
        "| Commit rounding per axis, at most (`ladder::COMMIT_PER_AXIS`, in half steps: \
         {half_steps}) | {} |",
        commit.fraction()
    );
    for (_, step) in steps {
        let mm = mul(commit, ratio(u128::from(step.quarter_nm()), QNM_PER_MM)?)?;
        write!(row, " {mm} |").map_err(formatting)?;
    }
    row.push('\n');
    out.push_str(&row);

    check_commit_ceilings()?;
    for (name, steps_count) in LADDER {
        let n = u128::from(steps_count.get());
        let mut row = format!("| {name} | {n} |");
        for (step_name, step) in steps {
            let quarter_nm = ladder::quarter_nm(steps_count, *step).ok_or_else(|| {
                fail(format!(
                    "ladder::quarter_nm reports no value for {name} on {step_name}"
                ))
            })?;
            let from_ladder = ratio(quarter_nm, QNM_PER_MM)?;
            let from_step = mul(
                ratio(n, 1)?,
                ratio(u128::from(step.quarter_nm()), QNM_PER_MM)?,
            )?;
            if from_ladder != from_step {
                return Err(fail(format!(
                    "{name} on {step_name}: ladder::quarter_nm gives {from_ladder} mm, \
                     the step size gives {from_step} mm"
                )));
            }
            write!(row, " {from_ladder} |").map_err(formatting)?;
        }
        row.push('\n');
        out.push_str(&row);
    }

    // The angular bound: ladder::ANGULAR must be exactly 2^ANGULAR_LOG2, and 2^ANGULAR_LOG2 must
    // be the fit bound divided by 2^COORD_BITS.
    let log2 = ladder::ANGULAR_LOG2;
    let angular_bits = ladder::ANGULAR.get().to_bits();
    if power_of_two_bits(log2) != Some(angular_bits) {
        return Err(fail(format!(
            "ladder::ANGULAR (bits {angular_bits:#018x}) is not exactly 2^{log2}"
        )));
    }
    let neg_log2 = i64::from(log2)
        .checked_neg()
        .and_then(|e| u32::try_from(e).ok())
        .ok_or_else(|| fail("ladder::ANGULAR_LOG2 is not negative"))?;
    let two_pow = |e: u32| {
        2u128
            .checked_pow(e)
            .ok_or_else(|| fail(format!("2^{e} does not fit in u128")))
    };
    let angular = ratio(1, two_pow(neg_log2)?)?;
    let fit_over_range = ratio(u128::from(ladder::FIT.get()), two_pow(COORD_BITS)?)?;
    if angular != fit_over_range {
        return Err(fail(format!(
            "the angular bound {angular} is not the fit bound over 2^{COORD_BITS}, \
             {fit_over_range}"
        )));
    }
    // Every number in this sentence is `ladder::COMMIT_PER_AXIS` in steps, the bound
    // `check_commit_ceilings` checked above, so the sentence cannot contradict the check.
    let per_axis = commit.fraction();
    writeln!(
        out,
        "\nCommitting rounds each coordinate to the nearest step, moving it by at most {per_axis} \
         step (`ladder::COMMIT_PER_AXIS`), so a point of n coordinates moves by at most \
         √n × {per_axis} steps (Pythagoras). Each commit-distance ceiling c is checked to bound \
         that exactly: n × ({per_axis})² ≤ c²."
    )
    .map_err(formatting)?;
    writeln!(
        out,
        "\nThe angular bound (`ladder::ANGULAR`) is 2^{log2} rad (`ladder::ANGULAR_LOG2`) = {} rad \
         = {angular} rad, exactly, as an `f64` too: the fit bound ({} steps) divided by 2^{COORD_BITS} \
         steps (2^`grid::COORD_BITS`, one step more than the largest coordinate magnitude, \
         2^{COORD_BITS} − 1).",
        angular.fraction(),
        ladder::FIT.get(),
    )
    .map_err(formatting)?;
    Ok(())
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

    fn text() -> String {
        String::from_utf8(generate().unwrap()).unwrap()
    }

    #[test]
    fn the_same_bytes_every_time_with_unix_line_ends() {
        let a = generate().unwrap();
        assert_eq!(a, generate().unwrap());
        assert!(!a.contains(&b'\r'));
        assert!(a.ends_with(b"\n"));
        assert!(!a.ends_with(b"\n\n"));
    }

    /// The committed file, built into this test, is exactly what the generator gives on the
    /// target the test runs on. CI runs it natively on every platform and under wasm32-wasip1, so
    /// the generator gives the same bytes on every target, and `check` means the same everywhere.
    #[test]
    fn the_committed_output_is_what_this_target_generates() {
        let committed: &[u8] = include_bytes!("../generated/grid-steps.md");
        let generated = generate().unwrap();
        assert!(
            committed == generated.as_slice(),
            "the committed grid-steps.md differs from the generator's output on this target, \
             first on line {}; run `cargo run -p geom-gen -- write`",
            crate::files::first_difference_line(committed, &generated)
        );
    }

    #[test]
    fn steps_are_walked_from_the_code() {
        let s = steps().unwrap();
        let names: Vec<&str> = s.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["Q0", "Q1", "Q2"]);
        assert_eq!(s.first().unwrap().1.coarser(), None);
        assert_eq!(s.last().unwrap().1.finer(), None);
    }

    #[test]
    fn power_of_two_bits_are_exact() {
        assert_eq!(power_of_two_bits(0), Some(1.0f64.to_bits()));
        assert_eq!(power_of_two_bits(-1), Some(0.5f64.to_bits()));
        assert_eq!(power_of_two_bits(10), Some(1024.0f64.to_bits()));
        assert_eq!(
            power_of_two_bits(-34),
            Some(ladder::ANGULAR.get().to_bits())
        );
        assert_eq!(power_of_two_bits(-1022), Some(f64::MIN_POSITIVE.to_bits()));
        assert_eq!(power_of_two_bits(1023), Some(2f64.powi(1023).to_bits()));
        assert_eq!(power_of_two_bits(-1023), None);
        assert_eq!(power_of_two_bits(1024), None);
        assert_eq!(power_of_two_bits(i32::MIN), None);
        assert_eq!(power_of_two_bits(i32::MAX), None);
    }

    /// The commit ceilings hold for geom-num's constants, and the check would refuse a ceiling
    /// below the exact distance: with h half steps per axis, √n · h half steps is at most c steps
    /// exactly when n · h² ≤ 4c². One half step per axis and a ceiling of zero steps would be
    /// refused in every dimension; two half steps (one whole step) per axis and one step would be
    /// refused in 2D (√2 > 1) and 3D.
    #[test]
    fn commit_ceilings_bound_the_exact_distance() {
        check_commit_ceilings().unwrap();
        let holds = |n: u128, h: u128, c: u128| n * h * h <= 4 * c * c;
        assert!(holds(2, 1, 1) && holds(3, 1, 1) && holds(4, 1, 1));
        assert!(!holds(5, 1, 1));
        assert!(!holds(2, 1, 0) && !holds(3, 1, 0));
        assert!(!holds(2, 2, 1) && !holds(3, 2, 1));
    }

    /// Each unit constant of geom-num is named in its row, with its value, and the units without
    /// one say so.
    #[test]
    fn unit_constants_are_named_in_their_rows() {
        let t = text();
        for row in [
            "| 1 nm | 0.000001 | not whole: 0.0064 | not whole: 0.16 | 4 | `grid::QUARTER_NM_PER_NM` = 4 |",
            "| 1 µm | 0.001 | not whole: 6.4 | 160 | 4000 | `grid::QUARTER_NM_PER_MICRON` = 4000 |",
            "| 1 mm | 1 | 6400 | 160000 | 4000000 | `grid::QUARTER_NM_PER_MM` = 4000000 |",
            "| 1 mil (1/1000 in) | 0.0254 | not whole: 162.56 | 4064 | 101600 | `grid::QUARTER_NM_PER_MIL` = 101600 |",
            "| 1 in | 25.4 | 162560 | 4064000 | 101600000 | `grid::QUARTER_NM_PER_INCH` = 101600000 |",
            "| 0.01 mm | 0.01 | 64 | 1600 | 40000 | — |",
        ] {
            assert!(t.contains(row), "missing row: {row}\n{t}");
        }
    }

    /// The values `geom-num`'s hand-written headers state, found in the generated tables.
    #[test]
    fn agrees_with_geom_num_s_headers() {
        let t = text();
        for row in [
            "| Q0 | 625 | 1/6400 | 0.00015625 | 156.25 | 6400 | — | 171798.69183984375 |",
            "| Q1 | 25 | 1/160000 | 0.00000625 | 6.25 | 160000 | 25 | 6871.94767359375 |",
            "| Q2 | 1 | 1/4000000 | 0.00000025 | 0.25 | 4000000 | 25 | 274.87790694375 |",
            // grid.rs: Q0 holds 0.01 mm, 1 mm and 1/256 inch exactly; Q1 adds 1 µm and 1 mil.
            "| 0.01 mm | 0.01 | 64 | 1600 | 40000 |",
            "| 1/256 in | 0.09921875 | 635 | 15875 | 396875 |",
            "| 1 mil (1/1000 in) | 0.0254 | not whole: 162.56 | 4064 | 101600 |",
            "| 1 µm | 0.001 | not whole: 6.4 | 160 | 4000 |",
            "| 1 nm | 0.000001 | not whole: 0.0064 | not whole: 0.16 | 4 |",
            "| 1 US survey ft | 1200000/3937 | not whole: 7680000000/3937 | not whole: 192000000000/3937 | not whole: 4800000000000/3937 |",
            // ladder.rs: the fit bound is 0.01 mm at Q0 and 0.4 µm at Q1; healing 0.1 mm, cap 1 mm.
            "| Fit bound (`ladder::FIT`) | 64 | 0.01 | 0.0004 | 0.000016 |",
            "| Per-operation bound (`ladder::PER_OPERATION`) | 16 | 0.0025 | 0.0001 | 0.000004 |",
            "| Import healing, default (`ladder::HEAL_DEFAULT`) | 640 | 0.1 | 0.004 | 0.00016 |",
            "| Import healing, cap (`ladder::HEAL_CAP`) | 6400 | 1 | 0.04 | 0.0016 |",
            "| Short-edge warning (`ladder::SHORT_EDGE_WARNING`) | 3200 | 0.5 | 0.02 | 0.0008 |",
            // ladder.rs: commit rounding is half a step per axis (78.125 nm at Q0, 3.125 nm at Q1),
            // and one whole step bounds the distance in 2D and in 3D.
            "| Commit rounding per axis, at most (`ladder::COMMIT_PER_AXIS`, in half steps: 1) | 1/2 | 0.000078125 | 0.000003125 | 0.000000125 |",
            "| Commit distance in 2D, certified ceiling (`ladder::COMMIT_DISTANCE_2D`) | 1 | 0.00015625 | 0.00000625 | 0.00000025 |",
            "| Commit distance in 3D, certified ceiling (`ladder::COMMIT_DISTANCE_3D`) | 1 | 0.00015625 | 0.00000625 | 0.00000025 |",
        ] {
            assert!(t.contains(row), "missing row: {row}\n{t}");
        }
        // ladder.rs: half a step per axis, and n · (1/2)² ≤ 1² for the one-step ceilings.
        assert!(
            t.contains("moving it by at most 1/2 step (`ladder::COMMIT_PER_AXIS`)"),
            "{t}"
        );
        assert!(t.contains("n × (1/2)² ≤ c²."), "{t}");
        assert!(
            t.contains("2^-34 rad (`ladder::ANGULAR_LOG2`) = 1/17179869184 rad"),
            "{t}"
        );
        assert!(t.contains("2^40 − 1 = 1099511627775 steps"), "{t}");
        // One description of the range: the angular sentence divides by 2^40 and says how that
        // relates to the largest magnitude, rather than calling 2^40 the range.
        assert!(!t.contains("range (2^40 steps)"), "{t}");
    }
}
