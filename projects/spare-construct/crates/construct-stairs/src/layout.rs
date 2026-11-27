//! A stair's layout, worked out exactly from its rise (the height from its base to its top) by
//! its rule set's limits: how many risers, how high each is, the going, each run's risers, rise
//! and length, each landing's height, and the total run.
//!
//! The rule, in order ([`layout_with`]):
//!
//! 1. The going is the stair's, or its category's least; it is at least that least and at most
//!    the rule set's most going. The width is at least the category's least, where it has one,
//!    and every landing at least the rule set's least landing.
//! 2. The risers the limits allow on that going: at most the category's most riser and no
//!    steeper than its most pitch, at least the rule set's least riser and no shallower than its
//!    least pitch. In whole grid steps: `highest = min(most riser, floor(going * p / q))` for the
//!    most pitch's slope `p/q`, and `lowest = max(least riser, ceil(going * p' / q'))` for the
//!    least pitch's. A riser `r` is then within every limit exactly when
//!    `lowest <= r <= highest`.
//! 3. The count of risers `n`: the least that lays the rise out with every riser within those
//!    limits and that the runs allow (each run at least two, a run that gives its count exactly
//!    that). A rise `H` in `n` risers is laid out as risers `floor(k H / n) - floor((k - 1) H /
//!    n)` for `k` from 1 to `n`: a rise that does not divide evenly gives risers one grid step
//!    apart (156.25 nm), never a rounded riser, and their sum is the rise exactly. So the
//!    tallest is `ceil(H / n)` and the lowest `floor(H / n)`, and `n` works when `highest` and
//!    `lowest` hold them and they are no further apart than the rule set's uniformity allows
//!    (5 mm in D1/AS1 4.1.3: one grid step always is). When no count works, the refusal names
//!    the two limits and the nearest floor-to-floor heights above and below that would work with
//!    the same settings ([`StairError::NoRiserCount`]).
//! 4. The runs that do not give their count share what is left, as evenly as can be, the earlier
//!    ones taking one more; each run's risers follow on from the run before.
//! 5. Each run rises at most the category's most rise between landings.
//!
//! The total run is the length of the walking line in plan: every tread's going and every
//! landing's length. A run of `m` risers has `m - 1` treads; its top riser rises onto the landing
//! or the floor above.

use crate::error::{Named, NoCount, RiserCount, StairError};
use crate::rules::{Category, LengthLimit, RuleSet};
use crate::stair::{MOST_RISERS, Stair, Turn};
use construct_params::{Length, Slope};

/// One run as laid out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunLayout {
    /// The number (from 0) of its first riser among the stair's.
    pub first: usize,
    /// How many risers it has.
    pub risers: usize,
    /// The height of its foot above the stair's base.
    pub base: Length,
    /// How far it rises: its risers' sum.
    pub rise: Length,
    /// Its length in plan: its treads' goings, `(risers - 1) * going`.
    pub length: Length,
}

/// One landing as laid out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LandingLayout {
    /// Its length along the run that comes into it.
    pub length: Length,
    /// How it turns the stair.
    pub turn: Turn,
    /// The height of its top above the stair's base.
    pub height: Length,
}

/// A stair's layout, as the module header says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The rule set's ID.
    pub rules: String,
    /// The category's ID.
    pub category: String,
    /// The category's name.
    pub category_name: String,
    /// The rise from the base to the top.
    pub rise: Length,
    /// The going of every tread.
    pub going: Length,
    /// The width.
    pub width: Length,
    /// The waist.
    pub waist: Length,
    /// Each riser's height, from the bottom.
    pub risers: Vec<Length>,
    /// The runs, from the bottom.
    pub runs: Vec<RunLayout>,
    /// The landings, from the bottom.
    pub landings: Vec<LandingLayout>,
    /// The least headroom above the pitch line, which the stair's pieces are checked by.
    pub headroom: LengthLimit,
}

impl Layout {
    /// How many risers.
    #[must_use]
    pub fn riser_count(&self) -> usize {
        self.risers.len()
    }

    /// How many treads: one fewer than the risers in each run.
    #[must_use]
    pub fn tread_count(&self) -> usize {
        self.risers.len().saturating_sub(self.runs.len())
    }

    /// The tallest riser.
    #[must_use]
    pub fn tallest(&self) -> Length {
        self.risers
            .iter()
            .copied()
            .max_by_key(|r| r.steps())
            .unwrap_or(Length::from_steps(0))
    }

    /// The lowest riser.
    #[must_use]
    pub fn lowest(&self) -> Length {
        self.risers
            .iter()
            .copied()
            .min_by_key(|r| r.steps())
            .unwrap_or(Length::from_steps(0))
    }

    /// The pitch of the tallest riser on the going: the steepest step, as a rise over a run.
    #[must_use]
    pub fn pitch(&self) -> Option<Slope> {
        Slope::new(self.tallest().steps(), self.going.steps())
    }

    /// The total run: the length of the walking line in plan, every tread's going and every
    /// landing's length.
    ///
    /// # Errors
    /// [`StairError::OutOfRange`] if the sum does not fit, which the limits on a stair's size
    /// rule out.
    pub fn total_run(&self) -> Result<Length, StairError> {
        let mut total = Length::from_steps(0);
        for length in self
            .runs
            .iter()
            .map(|r| r.length)
            .chain(self.landings.iter().map(|l| l.length))
        {
            total = total.checked_add(length).ok_or_else(too_long)?;
        }
        Ok(total)
    }
}

fn too_long() -> StairError {
    StairError::OutOfRange {
        what: "the stair's total run".to_owned(),
    }
}

/// `a * b` exactly, as 128 bits.
fn wide(a: i64, b: i64) -> i128 {
    i128::from(a).saturating_mul(i128::from(b))
}

/// The risers the limits allow on a going, as the module header's step 2 says: the lowest and
/// the highest, each named with the limit that sets it.
fn riser_range(rules: &RuleSet, category: &Category, going: Length) -> (Named, Named) {
    let g = going.steps();
    let (p, q) = (
        category.pitch_most.slope.rise(),
        category.pitch_most.slope.run(),
    );
    // floor(g * p / q): g and p are positive, so integer division rounds down.
    let at_most_pitch = wide(g, p).checked_div(i128::from(q)).unwrap_or(0);
    let highest = if at_most_pitch < i128::from(category.riser_most.length.steps()) {
        Named {
            what: format!(
                "the highest riser at its most pitch of {} degrees on that going",
                category.pitch_most.degrees
            ),
            value: Length::from_steps(i64::try_from(at_most_pitch).unwrap_or(i64::MAX)),
            clause: category.pitch_most.clause.clone(),
        }
    } else {
        category.riser_most.named("the highest a riser may be")
    };
    let (p, q) = (
        rules.pitch_least.slope.rise(),
        rules.pitch_least.slope.run(),
    );
    let numerator = wide(g, p);
    let q = i128::from(q);
    let at_least_pitch = numerator
        .checked_div(q)
        .unwrap_or(0)
        .saturating_add(i128::from(numerator.checked_rem(q).unwrap_or(0) != 0));
    let lowest = if at_least_pitch > i128::from(rules.riser_least.length.steps()) {
        Named {
            what: format!(
                "the lowest riser at the least pitch of {} degrees on that going",
                rules.pitch_least.degrees
            ),
            value: Length::from_steps(i64::try_from(at_least_pitch).unwrap_or(i64::MAX)),
            clause: rules.pitch_least.clause.clone(),
        }
    } else {
        rules.riser_least.named("the lowest a riser may be")
    };
    (lowest, highest)
}

/// Whether a rise `h` in `n` risers is within `lowest..=highest` and as uniform as `within`
/// allows, all in grid steps.
fn works(h: i64, n: i64, lowest: i64, highest: i64, within: i64) -> bool {
    let (Some(low), Some(rem)) = (h.checked_div(n), h.checked_rem(n)) else {
        return false;
    };
    let high = low.saturating_add(i64::from(rem != 0));
    low >= lowest && high <= highest && high.saturating_sub(low) <= within
}

/// The nearest heights below and above `h` that some allowed count of risers lays out.
fn nearest(
    h: i64,
    counts: &[i64],
    lowest: i64,
    highest: i64,
    within: i64,
) -> (Option<Length>, Option<Length>) {
    let mut below: Option<i64> = None;
    let mut above: Option<i64> = None;
    for &n in counts {
        // Heights `n` lays out: from n * lowest to n * highest; with risers that must be equal,
        // only the multiples of n among them.
        let (Some(least), Some(most)) = (n.checked_mul(lowest), n.checked_mul(highest)) else {
            continue;
        };
        if least > most {
            continue;
        }
        let step = if within >= 1 { 1 } else { n };
        // The largest height of n's below h.
        let top = most.min(h.saturating_sub(1));
        if top >= least {
            let candidate = top.saturating_sub(top.checked_rem(step).unwrap_or(0));
            if candidate >= least && works(candidate, n, lowest, highest, within) {
                below = Some(below.map_or(candidate, |b| b.max(candidate)));
            }
        }
        // The smallest height of n's above h.
        let bottom = least.max(h.saturating_add(1));
        if bottom <= most {
            let rem = bottom.checked_rem(step).unwrap_or(0);
            let candidate = if rem == 0 {
                bottom
            } else {
                bottom.saturating_add(step.saturating_sub(rem))
            };
            if candidate <= most && works(candidate, n, lowest, highest, within) {
                above = Some(above.map_or(candidate, |a| a.min(candidate)));
            }
        }
    }
    (below.map(Length::from_steps), above.map(Length::from_steps))
}

/// A stair's layout by its own rule set ([`RuleSet::builtin`] of its `rules`), for a rise.
///
/// # Errors
/// [`StairError::UnknownRules`] for a rule set this program does not hold; any refusal of
/// [`layout_with`].
pub fn layout(stair: &Stair, rise: Length) -> Result<Layout, StairError> {
    layout_with(&RuleSet::builtin(stair.rules())?, stair, rise)
}

/// A stair's layout by the rule set `rules`, for a rise, as the module header says.
///
/// # Errors
/// [`StairError::UnknownRules`] when the stair names another rule set;
/// [`StairError::UnknownCategory`]; [`StairError::NotRising`] for a rise of nothing or less; and
/// each limit's refusal, in the module header's order: [`StairError::GoingTooShort`],
/// [`StairError::GoingTooLong`], [`StairError::TooNarrow`], [`StairError::LandingTooShort`],
/// [`StairError::NotUniform`] (a count that works but for the uniformity a rule set asks),
/// [`StairError::NoRiserCount`], [`StairError::RunRisesTooFar`].
#[allow(
    clippy::too_many_lines,
    reason = "the module header's five steps, in order, in one place"
)]
pub fn layout_with(rules: &RuleSet, stair: &Stair, rise: Length) -> Result<Layout, StairError> {
    if stair.rules() != rules.id {
        return Err(StairError::UnknownRules(stair.rules().to_owned()));
    }
    let category = rules.category(stair.category())?;
    if rise.steps() <= 0 {
        return Err(StairError::NotRising { rise });
    }
    // Step 1.
    let going = stair.going().unwrap_or(category.going_least.length);
    if going.steps() < category.going_least.length.steps() {
        return Err(StairError::GoingTooShort {
            going,
            category: category.name.clone(),
            least: category.going_least.named("the shortest a going may be"),
        });
    }
    if going.steps() > rules.going_most.length.steps() {
        return Err(StairError::GoingTooLong {
            going,
            most: rules.going_most.named("the longest a going may be"),
        });
    }
    if let Some(least) = &category.width_least
        && stair.width().steps() < least.length.steps()
    {
        return Err(StairError::TooNarrow {
            width: stair.width(),
            category: category.name.clone(),
            least: least.named("the narrowest it may be"),
        });
    }
    for (at, landing) in stair.landings().iter().enumerate() {
        if landing.length.steps() < rules.landing_least.length.steps() {
            return Err(StairError::LandingTooShort {
                landing: at.saturating_add(1),
                length: landing.length,
                least: rules.landing_least.named("the shortest a landing may be"),
            });
        }
    }
    // Step 2.
    let (lowest, highest) = riser_range(rules, category, going);
    let (lo, hi) = (lowest.value.steps(), highest.value.steps());
    let within = rules.uniform_within.length.steps();
    // Step 3.
    let fixed: usize = stair
        .runs()
        .iter()
        .filter_map(|r| r.risers)
        .map(|n| usize::try_from(n).unwrap_or(usize::MAX))
        .fold(0, usize::saturating_add);
    let free = stair.runs().iter().filter(|r| r.risers.is_none()).count();
    let least_count = fixed.saturating_add(free.saturating_mul(2));
    let count_rule = if free == 0 {
        RiserCount::Exactly(fixed)
    } else {
        RiserCount::AtLeast(least_count)
    };
    let counts: Vec<i64> = if free == 0 {
        vec![i64::try_from(fixed).unwrap_or(i64::MAX)]
    } else {
        (least_count..=MOST_RISERS)
            .filter_map(|n| i64::try_from(n).ok())
            .collect()
    };
    let h = rise.steps();
    let Some(n) = counts
        .iter()
        .copied()
        .find(|&n| works(h, n, lo, hi, within))
    else {
        // A count whose risers would be within every limit but one step apart, where the rule
        // set asks for less.
        if let Some(n) = counts
            .iter()
            .copied()
            .find(|&n| works(h, n, lo, hi, within.max(1)))
        {
            return Err(StairError::NotUniform {
                rise,
                risers: usize::try_from(n).unwrap_or(usize::MAX),
                within: rules
                    .uniform_within
                    .named("the most two risers of a flight may differ"),
            });
        }
        let (below, above) = nearest(h, &counts, lo, hi, within);
        return Err(StairError::NoRiserCount(Box::new(NoCount {
            rise,
            category: category.name.clone(),
            lowest,
            highest,
            going,
            risers: count_rule,
            below,
            above,
        })));
    };
    let mut risers = Vec::with_capacity(usize::try_from(n).unwrap_or(0));
    let mut before = 0_i128;
    for k in 1..=n {
        let at = wide(k, h)
            .checked_div(i128::from(n))
            .ok_or_else(|| StairError::OutOfRange {
                what: "a riser".to_owned(),
            })?;
        let riser =
            i64::try_from(at.saturating_sub(before)).map_err(|_| StairError::OutOfRange {
                what: "a riser".to_owned(),
            })?;
        risers.push(Length::from_steps(riser));
        before = at;
    }
    // Step 4.
    let n = risers.len();
    let left = n.saturating_sub(fixed);
    let (share, extra) = (
        left.checked_div(free).unwrap_or(0),
        left.checked_rem(free).unwrap_or(0),
    );
    let mut runs = Vec::with_capacity(stair.runs().len());
    let mut landings = Vec::with_capacity(stair.landings().len());
    let (mut first, mut free_seen, mut base) = (0_usize, 0_usize, Length::from_steps(0));
    for (at, spec) in stair.runs().iter().enumerate() {
        let count = if let Some(count) = spec.risers {
            usize::try_from(count).unwrap_or(usize::MAX)
        } else {
            let count = share.saturating_add(usize::from(free_seen < extra));
            free_seen = free_seen.saturating_add(1);
            count
        };
        let mut run_rise = Length::from_steps(0);
        for riser in risers
            .get(first..first.saturating_add(count))
            .unwrap_or_default()
        {
            run_rise = run_rise.checked_add(*riser).ok_or_else(too_long)?;
        }
        let treads = i64::try_from(count.saturating_sub(1)).map_err(|_| too_long())?;
        let length = Length::from_steps(going.steps().checked_mul(treads).ok_or_else(too_long)?);
        // Step 5.
        if run_rise.steps() > category.rise_between_landings_most.length.steps() {
            return Err(StairError::RunRisesTooFar {
                run: at.saturating_add(1),
                rise: run_rise,
                category: category.name.clone(),
                most: category
                    .rise_between_landings_most
                    .named("the most a stair may rise between landings"),
            });
        }
        runs.push(RunLayout {
            first,
            risers: count,
            base,
            rise: run_rise,
            length,
        });
        first = first.saturating_add(count);
        base = base.checked_add(run_rise).ok_or_else(too_long)?;
        if let Some(spec) = stair.landings().get(at) {
            landings.push(LandingLayout {
                length: spec.length,
                turn: spec.turn,
                height: base,
            });
        }
    }
    Ok(Layout {
        rules: rules.id.clone(),
        category: category.id.clone(),
        category_name: category.name.clone(),
        rise,
        going,
        width: stair.width(),
        waist: stair.waist(),
        risers,
        runs,
        landings,
        headroom: rules.headroom_least.clone(),
    })
}
