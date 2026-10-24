//! Count files and the checks the gates make with them: comparing the merge base with a pull
//! request, the benchmarks a pull request removed, and the growth checks.
//!
//! A count file has one line per benchmark: its name and a count (WebAssembly fuel, for the fuel
//! gate), separated by one space. Lines starting with `#` are comments. The checks:
//!
//! - [`compare`]: a rise of more than the gate's percentage on any benchmark fails; every change of
//!   more than the listing percentage either way is listed (chapter 23.13: "every change over 1%
//!   ... is listed"); a benchmark at the merge base that the pull request no longer has fails,
//!   unless the pull request names it in `retired.txt`, so a regression cannot hide behind a
//!   rename; new benchmarks are listed.
//! - [`growth`]: each scale benchmark of [`crate::catalogue::GROWTH`] must grow no faster than its
//!   stated cost from n to 2n and from 2n to 4n (chapter 28.14, "Growth"). A doubling may multiply
//!   the work by at most 2^(k + 1/2) for a stated cost of size^k: the geometric midpoint between
//!   the stated order (2^k) and the next one up (2^(k + 1)), so constant terms and code that
//!   differs between widths pass, and work that is one order up, size^(k + 1), fails.
//!
//!   The counts are whole counts, so a part that does not grow (the loop around the batch, an
//!   operation's fixed cost) damps every ratio: a count a + b · size^m with m > k + 1/2 fails a
//!   doubling from size s only once b · s^m · (2^m − 2^(k + 1/2)) > a · (2^(k + 1/2) − 1). For
//!   m = k + 1 the growing part must exceed about 1.6 times the fixed part when k = 1, and about
//!   2 times when k = 2. Comparison's fixed part is about 2,800 of its 3,814 fuel at 2 limbs, so
//!   a comparison made quadratic could pass both doublings at 2, 4 and 8 limbs; the fuel gate on
//!   its per-width benchmarks fails it, as it fails any rise over 5% on a benchmark the merge base
//!   has (a new operation has only this check until it is merged). Larger widths do not help:
//!   from 16 limbs up the code the compiler emits changes between widths enough to swamp growth
//!   (a linear addition's fuel rises 3.8 times from 16 to 32 limbs, then 2.0 times to 64).
//!
//! Every comparison is exact integer arithmetic on the counts: a percentage limit p is tested as
//! 100 · head > (100 + p) · base, and a growth limit as c(2n)² > 2^(2k + 1) · c(n)². Percentages
//! are only computed, as floating point, for display.
//!
//! Guarantees: every check is exact for every count, percentage and exponent its types hold (a
//! product too large for 128 bits is decided by the bound it exceeds, never wrapped or truncated),
//! so no input makes a check panic or answer wrongly, with overflow checks on or off; and every
//! report lists benchmarks in name order, the same on every run and every platform.
//!
//! Method: the soft-limit regression check Gungraun applies to instruction counts (a percentage of
//! the baseline; <https://github.com/gungraun/gungraun/blob/a11cdc156da6134f281302f889c1d25fa44e00da/README.md>),
//! and the doubling test for empirical growth (Sedgewick and Wayne, *Algorithms*, 4th edition,
//! section 1.4, <https://algs4.cs.princeton.edu/14analysis/>).
//!
//! Cost: O(n log n) for n benchmarks: counts and names are kept in ordered maps and sets, so every
//! report lists them in name order, the same on every run.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::catalogue::{GROWTH, GrowthCase};

/// Counts by benchmark name, in name order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts(pub BTreeMap<String, u64>);

impl Counts {
    /// Parses a count file.
    ///
    /// # Errors
    ///
    /// A message naming the first malformed or repeated line.
    pub fn parse(text: &str) -> Result<Counts, String> {
        let mut counts = BTreeMap::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim_end();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let entry = line.split_once(' ').and_then(|(name, count)| {
                let count = count.parse::<u64>().ok()?;
                (!name.is_empty() && !name.contains(' ')).then_some((name.to_owned(), count))
            });
            let Some((name, count)) = entry else {
                return Err(format!("line {}: malformed: {line}", index + 1));
            };
            if counts.insert(name, count).is_some() {
                return Err(format!("line {}: repeated: {line}", index + 1));
            }
        }
        Ok(Counts(counts))
    }

    /// The count file, one line per benchmark in name order.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for (name, count) in &self.0 {
            let _ = writeln!(out, "{name} {count}");
        }
        out
    }
}

/// The names in a list file (`retired.txt`, or a list of benchmark names): one per line, `#`
/// comments and blank lines ignored, anything after the first space ignored (a reason may follow).
#[must_use]
pub fn parse_names(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split(' ').next())
        .map(str::to_owned)
        .collect()
}

/// One benchmark's change between the merge base and the pull request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// The benchmark.
    pub name: String,
    /// Its count at the merge base.
    pub base: u64,
    /// Its count in the pull request.
    pub head: u64,
}

impl Change {
    /// The change in percent, for display only (no check uses it): +∞ for a rise from zero.
    /// Counts below 2^53 convert exactly; larger ones round to the nearest double, the same on
    /// every platform.
    #[must_use]
    pub fn percent(&self) -> f64 {
        if self.base == self.head {
            0.0
        } else if self.base == 0 {
            f64::INFINITY
        } else {
            (self.head as f64 - self.base as f64) / self.base as f64 * 100.0
        }
    }

    /// Whether the count rose by more than `percent` percent: 100 · head > (100 + p) · base,
    /// exactly, for every count and every percentage.
    #[must_use]
    pub fn rose_over(&self, percent: u64) -> bool {
        // 100 · head < 2^71 always fits. The limit, base · (100 + p), can pass 2^128 only when it
        // is far above 100 · head; saturating then keeps the comparison's answer, no.
        let limit = u128::from(self.base).saturating_mul(u128::from(percent) + 100);
        u128::from(self.head) * 100 > limit
    }

    /// Whether the count fell by more than `percent` percent: 100 · head < (100 − p) · base.
    #[must_use]
    pub fn fell_over(&self, percent: u64) -> bool {
        let kept = u128::from(100u64.saturating_sub(percent));
        u128::from(self.head) * 100 < u128::from(self.base) * kept
    }
}

/// What [`compare`] found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Comparison {
    /// Rises over the gate's percentage: each one fails the gate.
    pub regressions: Vec<Change>,
    /// Every change over the listing percentage, either way, rises first.
    pub listed: Vec<Change>,
    /// Benchmarks at the merge base that the pull request removed without retiring them: each one
    /// fails the gate.
    pub removed: Vec<String>,
    /// Benchmarks the pull request removed and named in `retired.txt`.
    pub retired: Vec<String>,
    /// Benchmarks new in the pull request.
    pub added: Vec<String>,
}

impl Comparison {
    /// Whether the gate passes: no regression and no unretired removal.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.regressions.is_empty() && self.removed.is_empty()
    }

    /// A Markdown report of everything found, under the heading `title`.
    #[must_use]
    pub fn report(&self, title: &str, gate: u64, list: u64) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "### {title}\n");
        if self.passed() {
            let _ = writeln!(out, "**Passed.** No benchmark rose by more than {gate}%.\n");
        } else {
            let _ = writeln!(
                out,
                "**Failed:** {} rise(s) over {gate}% and {} benchmark(s) removed without being \
                 named in retired.txt.\n",
                self.regressions.len(),
                self.removed.len()
            );
        }
        table(
            &mut out,
            &format!("Rises over {gate}% (they block the merge)"),
            &self.regressions,
        );
        table(
            &mut out,
            &format!("Every change over {list}%"),
            &self.listed,
        );
        names(
            &mut out,
            "Removed without being retired (they block the merge)",
            &self.removed,
        );
        names(&mut out, "Removed and retired", &self.retired);
        names(&mut out, "New", &self.added);
        out
    }
}

/// Appends a table of changes, or "None."
fn table(out: &mut String, heading: &str, changes: &[Change]) {
    let _ = writeln!(out, "#### {heading}\n");
    if changes.is_empty() {
        let _ = writeln!(out, "None.\n");
        return;
    }
    let _ = writeln!(out, "| Benchmark | Merge base | Pull request | Change |");
    let _ = writeln!(out, "| --- | ---: | ---: | ---: |");
    for c in changes {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {:+.2}% |",
            c.name,
            c.base,
            c.head,
            c.percent()
        );
    }
    out.push('\n');
}

/// Appends a list of names, or "None."
fn names(out: &mut String, heading: &str, list: &[String]) {
    let _ = writeln!(out, "#### {heading}\n");
    if list.is_empty() {
        let _ = writeln!(out, "None.\n");
        return;
    }
    for name in list {
        let _ = writeln!(out, "- {name}");
    }
    out.push('\n');
}

/// Compares the merge base's counts with the pull request's: rises over `gate` percent fail,
/// changes over `list` percent are listed, and benchmarks removed without being in `retired` fail.
#[must_use]
pub fn compare(
    base: &Counts,
    head: &Counts,
    gate: u64,
    list: u64,
    retired: &BTreeSet<String>,
) -> Comparison {
    let mut out = Comparison::default();
    let mut falls = Vec::new();
    for (name, &b) in &base.0 {
        match head.0.get(name) {
            Some(&h) => {
                let change = Change {
                    name: name.clone(),
                    base: b,
                    head: h,
                };
                if change.rose_over(gate) {
                    out.regressions.push(change.clone());
                }
                if change.rose_over(list) {
                    out.listed.push(change);
                } else if change.fell_over(list) {
                    falls.push(change);
                }
            }
            None if retired.contains(name) => out.retired.push(name.clone()),
            None => out.removed.push(name.clone()),
        }
    }
    out.listed.extend(falls);
    out.added = head
        .0
        .keys()
        .filter(|name| !base.0.contains_key(*name))
        .cloned()
        .collect();
    out
}

/// Names in `base` that `head` lacks and `retired` does not name: the removals that fail a gate.
#[must_use]
pub fn unretired_removals(
    base: &BTreeSet<String>,
    head: &BTreeSet<String>,
    retired: &BTreeSet<String>,
) -> Vec<String> {
    base.iter()
        .filter(|name| !head.contains(*name) && !retired.contains(*name))
        .cloned()
        .collect()
}

/// The growth check of one scale benchmark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrowthCheck {
    /// The scale benchmark's name in [`GROWTH`].
    pub name: &'static str,
    /// Its stated cost exponent k (work grows as size^k).
    pub exponent: u32,
    /// Its sizes, n, 2n and 4n.
    pub sizes: [usize; 3],
    /// Its counts at those sizes, where present.
    pub counts: [Option<u64>; 3],
}

impl GrowthCheck {
    /// Whether every count is present and each doubling multiplies the work by at most
    /// 2^(k + 1/2): c(2s)² ≤ 2^(2k + 1) · c(s)², exactly, for every count and every exponent.
    #[must_use]
    pub fn passed(&self) -> bool {
        let [Some(c1), Some(c2), Some(c4)] = self.counts else {
            return false;
        };
        let exponent = self.exponent;
        let within = |small: u64, large: u64| {
            let (s, l) = (u128::from(small), u128::from(large));
            // l² < 2^128 always fits.
            let large_squared = l * l;
            if s == 0 {
                // No work at the smaller size: only no work at the larger one is within a factor.
                return large_squared == 0;
            }
            // 2^(2k + 1) · s², or `None` when it passes 2^128 − 1, which only happens when it is
            // above every l² (s ≥ 1, so the product is at least 2^(2k + 1), and l² < 2^128).
            let limit = exponent
                .checked_mul(2)
                .and_then(|e| e.checked_add(1))
                .and_then(|shift| 1u128.checked_shl(shift))
                .and_then(|factor| factor.checked_mul(s))
                .and_then(|scaled| scaled.checked_mul(s));
            limit.is_none_or(|limit| large_squared <= limit)
        };
        within(c1, c2) && within(c2, c4)
    }

    /// The two doubling ratios, for display.
    #[must_use]
    pub fn ratios(&self) -> [Option<f64>; 2] {
        let ratio = |a: Option<u64>, b: Option<u64>| match (a, b) {
            (Some(a), Some(b)) if a > 0 => Some(b as f64 / a as f64),
            _ => None,
        };
        let [c1, c2, c4] = self.counts;
        [ratio(c1, c2), ratio(c2, c4)]
    }

    /// The largest ratio a doubling may show, 2^(k + 1/2), for display: 2^k is exact (built from
    /// its bits), times √2 rounded once; +∞ when 2^k is beyond every double (k > 1023).
    #[must_use]
    pub fn limit(&self) -> f64 {
        // The biased exponent of 2^k is k + 1023, at most 2046 for a normal double.
        let power = match u64::from(self.exponent) + 1023 {
            biased @ ..=2046 => f64::from_bits(biased << 52),
            _ => f64::INFINITY,
        };
        power * core::f64::consts::SQRT_2
    }
}

/// The growth checks of every scale benchmark on `counts`.
#[must_use]
pub fn growth(counts: &Counts) -> Vec<GrowthCheck> {
    GROWTH
        .iter()
        .map(|g| {
            let count = |size: usize| {
                counts
                    .0
                    .get(&GrowthCase { growth: g, size }.name())
                    .copied()
            };
            let [n, n2, n4] = g.sizes;
            GrowthCheck {
                name: g.name,
                exponent: g.exponent,
                sizes: g.sizes,
                counts: [count(n), count(n2), count(n4)],
            }
        })
        .collect()
}

/// A Markdown report of the growth checks.
#[must_use]
pub fn growth_report(checks: &[GrowthCheck]) -> String {
    let mut out = String::from("### Growth at n, 2n and 4n\n\n");
    let failed = checks.iter().filter(|c| !c.passed()).count();
    if failed == 0 {
        out.push_str("**Passed.** Every scale benchmark grows no faster than its stated cost.\n\n");
    } else {
        let _ = writeln!(
            out,
            "**Failed:** {failed} scale benchmark(s) grow faster than stated.\n"
        );
    }
    out.push_str("| Benchmark | Stated cost | Sizes (limbs) | Counts | Ratios | Limit |");
    out.push_str(" Result |\n");
    out.push_str("| --- | --- | --- | --- | --- | ---: | --- |\n");
    for c in checks {
        let counts: Vec<String> = c
            .counts
            .iter()
            .map(|n| n.map_or_else(|| "missing".to_owned(), |n| n.to_string()))
            .collect();
        let ratios: Vec<String> = c
            .ratios()
            .iter()
            .map(|r| r.map_or_else(|| "-".to_owned(), |r| format!("{r:.3}")))
            .collect();
        let [s1, s2, s4] = c.sizes;
        let _ = writeln!(
            out,
            "| {} | size^{} | {s1}, {s2}, {s4} | {} | {} | {:.3} | {} |",
            c.name,
            c.exponent,
            counts.join(", "),
            ratios.join(", "),
            c.limit(),
            if c.passed() { "ok" } else { "FAILED" }
        );
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(pairs: &[(&str, u64)]) -> Counts {
        Counts(pairs.iter().map(|&(n, c)| (n.to_owned(), c)).collect())
    }

    #[test]
    fn parse_and_render_round_trip() {
        let c = Counts::parse("# fuel\nb 20\na 10\n\n").unwrap();
        assert_eq!(c, counts(&[("a", 10), ("b", 20)]));
        assert_eq!(c.render(), "a 10\nb 20\n");
        assert!(Counts::parse("a").is_err());
        assert!(Counts::parse("a x").is_err());
        assert!(Counts::parse("a 1\na 2").is_err());
        assert!(Counts::parse("a b 3").is_err());
    }

    #[test]
    fn the_gate_is_exact_at_its_limit() {
        let c = |base, head| Change {
            name: "x".into(),
            base,
            head,
        };
        // 5% of 100 is 5: 105 passes, 106 fails.
        assert!(!c(100, 105).rose_over(5));
        assert!(c(100, 106).rose_over(5));
        // From zero, any rise is over every limit; zero to zero is no change.
        assert!(c(0, 1).rose_over(5));
        assert!(!c(0, 0).rose_over(0));
        assert!(c(100, 98).fell_over(1));
        assert!(!c(100, 99).fell_over(1));
        assert_eq!(c(0, 3).percent(), f64::INFINITY);
        assert_eq!(c(200, 210).percent(), 5.0);
    }

    #[test]
    fn the_gate_is_exact_at_the_extremes_of_its_types() {
        let c = |base, head| Change {
            name: "x".into(),
            base,
            head,
        };
        let max = u64::MAX;
        // Counts at the top of u64: the smallest base from which u64::MAX is not a rise over 5% is
        // ⌈100 · u64::MAX / 105⌉ (computed in u128 here), and one less is.
        let least = u64::try_from((u128::from(max) * 100).div_ceil(105)).unwrap();
        assert!(!c(least, max).rose_over(5));
        assert!(c(least - 1, max).rose_over(5));
        assert!(!c(max, max).rose_over(0));
        assert!(c(max - 1, max).rose_over(0));
        // Percentages that would overflow 100 + p, or base · (100 + p) (which passes 2^128 for
        // base = u64::MAX): exact all the same.
        for percent in [max - 99, max - 50, max] {
            assert!(!c(max, max).rose_over(percent));
            assert!(!c(max / 2, max).rose_over(percent), "a rise of 100%");
            // From 1 to u64::MAX is a rise of (2^64 − 2) · 100%, over every u64 percentage.
            assert!(c(1, max).rose_over(percent));
            assert!(
                c(0, 1).rose_over(percent),
                "a rise from zero is over every limit"
            );
        }
        // A fall over 100% or more cannot happen; to zero is a fall of exactly 100%.
        assert!(!c(max, 0).fell_over(100));
        assert!(c(max, 0).fell_over(99));
        assert!(!c(max, 0).fell_over(max));
        assert_eq!(c(max, max).percent(), 0.0);
        assert!(c(1, max).percent() > 1e20);
        // compare and its report take the same extremes.
        let base = counts(&[("a", max), ("b", max / 2)]);
        let head = counts(&[("a", max), ("b", max)]);
        let r = compare(&base, &head, max, max, &BTreeSet::new());
        assert!(r.passed());
        assert!(r.listed.is_empty());
        let r = compare(&base, &head, 5, 1, &BTreeSet::new());
        assert_eq!(r.regressions.len(), 1);
        assert_eq!(r.listed.len(), 1);
        let report = r.report("x", 5, 1);
        assert!(report.contains("| b | 9223372036854775807 | 18446744073709551615 | +100.00% |"));
    }

    #[test]
    fn compare_finds_rises_listings_removals_and_additions() {
        let base = counts(&[("a", 100), ("b", 100), ("c", 100), ("gone", 7), ("old", 9)]);
        let head = counts(&[("a", 110), ("b", 98), ("c", 100), ("new", 3)]);
        let retired: BTreeSet<String> = ["old".to_owned()].into();
        let r = compare(&base, &head, 5, 1, &retired);
        assert_eq!(
            r.regressions
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["a"]
        );
        assert_eq!(
            r.listed.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(r.removed, ["gone"]);
        assert_eq!(r.retired, ["old"]);
        assert_eq!(r.added, ["new"]);
        assert!(!r.passed());
        let report = r.report("Fuel", 5, 1);
        assert!(report.contains("| a | 100 | 110 | +10.00% |"));
        assert!(report.contains("- gone"));
        // Retiring the removed one and fixing the rise passes.
        let head = counts(&[("a", 105), ("b", 98), ("c", 100), ("new", 3)]);
        let retired: BTreeSet<String> = ["old".to_owned(), "gone".to_owned()].into();
        assert!(compare(&base, &head, 5, 1, &retired).passed());
    }

    #[test]
    fn names_lists_ignore_comments_and_reasons() {
        let names = parse_names("# retired\npredicate/x/typical renamed to y\n\n  a/b/c\n");
        assert_eq!(
            names,
            ["a/b/c".to_owned(), "predicate/x/typical".to_owned()].into()
        );
        let base: BTreeSet<String> = ["a".into(), "b".into(), "c".into()].into();
        let head: BTreeSet<String> = ["a".into()].into();
        let retired: BTreeSet<String> = ["b".into()].into();
        assert_eq!(unretired_removals(&base, &head, &retired), ["c"]);
    }

    #[test]
    fn growth_passes_the_stated_order_and_fails_the_next() {
        let check = |exponent, counts| GrowthCheck {
            name: "t",
            exponent,
            sizes: [2, 4, 8],
            counts,
        };
        // Linear with a constant term: ratios below 2.
        assert!(check(1, [Some(100), Some(180), Some(340)]).passed());
        // Linear code a little slower per limb at the larger width: still within 2^1.5 ≈ 2.83.
        assert!(check(1, [Some(100), Some(250), Some(700)]).passed());
        // Quadratic work stated as linear fails.
        assert!(!check(1, [Some(100), Some(400), Some(1600)]).passed());
        // Quadratic stated as quadratic passes, cubic fails.
        assert!(check(2, [Some(100), Some(400), Some(1600)]).passed());
        assert!(!check(2, [Some(100), Some(800), Some(6400)]).passed());
        // A missing size fails.
        assert!(!check(1, [Some(100), None, Some(400)]).passed());
        // The limit is exact: a ratio of exactly 2^1.5 cannot occur in integers, so test both
        // sides of it: 2.828 passes, 2.829 fails.
        assert!(check(1, [Some(1000), Some(2828), Some(7997)]).passed());
        assert!(!check(1, [Some(1000), Some(2829), Some(7997)]).passed());
    }

    #[test]
    fn a_fixed_part_damps_growth_as_documented() {
        let check = |counts: [u64; 3]| GrowthCheck {
            name: "t",
            exponent: 1,
            sizes: [2, 4, 8],
            counts: counts.map(Some),
        };
        let quadratic = |a: u64, b: u64| [2u64, 4, 8].map(|s| a + b * s * s);
        // Quadratic work stated as linear: caught with no fixed part ...
        assert!(!check(quadratic(0, 256)).passed());
        // ... but not behind comparison's fixed part of about 2,800 (the module documentation).
        assert!(check(quadratic(2_790, 256)).passed());
        // The documented threshold, b · s^2 · (4 − 2^1.5) > a · (2^1.5 − 1), about
        // b · s^2 > 1.56 a, at the last doubling (s = 4): a = 1000 needs b · 16 above about 1,560.
        assert!(!check(quadratic(1_000, 98)).passed());
        assert!(check(quadratic(1_000, 97)).passed());
    }

    #[test]
    fn growth_is_exact_at_the_extremes_of_its_types() {
        let check = |exponent, counts| GrowthCheck {
            name: "t",
            exponent,
            sizes: [2, 4, 8],
            counts,
        };
        let max = u64::MAX;
        // Counts at the top of u64, where 2^(2k + 1) · c² passes 2^128.
        assert!(check(2, [Some(max), Some(max), Some(max)]).passed());
        assert!(check(1, [Some(max / 2), Some(max), Some(max)]).passed());
        assert!(!check(1, [Some(max / 4), Some(max), Some(max)]).passed());
        assert!(!check(0, [Some(max / 2), Some(max), Some(max)]).passed());
        // k = 63: the factor is 2^127, the largest that fits, and the limit 2^63.5 lies between
        // 2^63 and 2^64.
        let at_63 = |counts| check(63, counts).passed();
        assert!(at_63([Some(1), Some(1 << 63), Some(1 << 63)]));
        assert!(!at_63([Some(1), Some(max), Some(max)]));
        // Exponents whose factor 2^(2k + 1) passes 2^128, or whose 2k + 1 passes u32: every
        // nonzero growth is within them, and growth from zero work never is.
        for exponent in [64, 1000, u32::MAX / 2, u32::MAX] {
            assert!(check(exponent, [Some(1), Some(max), Some(max)]).passed());
            assert!(!check(exponent, [Some(0), Some(1), Some(1)]).passed());
            assert!(check(exponent, [Some(0), Some(0), Some(0)]).passed());
            let limit = check(exponent, [None; 3]).limit();
            assert!(limit > 1e18, "{exponent}: {limit}");
        }
        // The displayed limit is 2^(k + 1/2), with 2^k exact.
        assert_eq!(check(1, [None; 3]).limit(), 2.0 * core::f64::consts::SQRT_2);
        assert_eq!(
            check(40, [None; 3]).limit(),
            1_099_511_627_776.0 * core::f64::consts::SQRT_2
        );
        assert_eq!(check(1024, [None; 3]).limit(), f64::INFINITY);
        assert!(growth_report(&[check(u32::MAX, [Some(1), Some(max), Some(max)])]).contains("ok"));
    }

    #[test]
    fn growth_reads_the_catalogue_names() {
        let mut c = Counts::default();
        for g in GROWTH {
            for (i, &size) in g.sizes.iter().enumerate() {
                let work = 10u64 * (1 << (u32::try_from(i).unwrap() * g.exponent));
                c.0.insert(GrowthCase { growth: g, size }.name(), 100 + work);
            }
        }
        let checks = growth(&c);
        assert_eq!(checks.len(), GROWTH.len());
        assert!(checks.iter().all(GrowthCheck::passed), "{checks:?}");
        assert!(growth_report(&checks).contains("**Passed.**"));
    }
}
