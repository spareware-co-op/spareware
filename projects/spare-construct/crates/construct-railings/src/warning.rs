//! A railing that breaks a limit of its rule set, as warnings: by name, naming the limit, the
//! value that breaks it and its clause. A limit is not a refusal: a railing that breaks one is
//! drawn and built, and the warning says what to fix.
//!
//! A railing's warning is construct-warnings' own [`Warning`], and [`Severity`] is its too, as a
//! dimension's is: this crate adds the three kinds' names and the function that finds them. The
//! same three names are entries of construct-warnings' `RULES` table, which lists what
//! [`warnings`] reported for each railing of a model (construct-api's `cloud::warnings` hands it
//! over). The kinds:
//!
//! - [`LOW`]: the top rail is lower than the least barrier height of the railing's category,
//!   on a flight (measured from the pitch line) or on the level.
//! - [`OPENING`]: an opening a sphere of the openings rule passes (`children`: one at least the
//!   sphere's diameter; under the balusters on a flight, the stair sphere's), or one wider than the
//!   rule's most (`no-children`).
//! - [`HANDRAIL`]: a handrail on a stair's flight whose top is not within the rule set's handrail
//!   heights above the pitch line.
//!
//! Each kind gives at most one warning per railing, for the first place that breaks it, line by
//! line and piece by piece from the start, so a warning's identity lasts while the problem does.

use crate::error::RailingError;
use crate::layout::{Between, Layout, Opening};
use crate::railing::Railing;
use crate::rules::{LengthLimit, OpeningLimits};
use construct_levels::show;
use construct_params::{Length, Units};
use kit_core::Id;

pub use construct_warnings::{Severity, Warning};

/// A top rail too low.
pub const LOW: &str = "railing.low";
/// An opening too wide.
pub const OPENING: &str = "railing.opening";
/// A handrail too low or too high.
pub const HANDRAIL: &str = "railing.handrail";

fn place(o: &Opening) -> String {
    let at = format!("piece {} of line {}", o.segment, o.line);
    match o.between {
        Between::Members => format!("between upright members on {at}"),
        Between::Floor => format!("under the balusters on {at}"),
        Between::Flight => format!("under the balusters on the flight, {at}"),
    }
}

/// A length in words: construct-params' metric display settings (mm, at most 2 places).
fn words(length: Length) -> String {
    show(length, &Units::default())
}

fn cite(limit: &LengthLimit) -> String {
    format!("{} ({})", words(limit.length), limit.clause)
}

/// The openings of a layout that break its openings rule's limits: the first, in words.
fn opening(name: &str, limits: &OpeningLimits, layout: &Layout) -> Option<String> {
    for line in &layout.lines {
        for seg in &line.segments {
            let words = match limits {
                OpeningLimits::Sphere {
                    sphere,
                    stair_sphere,
                } => {
                    let under = if seg.rise == 0 { sphere } else { stair_sphere };
                    [(&seg.widest, sphere), (&seg.under_disc, under)]
                        .into_iter()
                        .find(|(o, limit)| o.at_least(limit.length))
                        .map(|(o, limit)| {
                            format!(
                                "{name}: the opening {} is {}, and a sphere of {} must not pass it.",
                                place(o),
                                words(o.shown),
                                cite(limit)
                            )
                        })
                }
                OpeningLimits::Most {
                    between_verticals,
                    between_rails,
                } => [
                    (&seg.widest, between_verticals),
                    (&seg.under_rise, between_rails),
                ]
                .into_iter()
                .find(|(o, limit)| o.wider_than(limit.length))
                .map(|(o, limit)| {
                    format!(
                        "{name}: the opening {} is {}, wider than the most allowed, {}.",
                        place(o),
                        words(o.shown),
                        cite(limit)
                    )
                }),
            };
            if words.is_some() {
                return words;
            }
        }
    }
    None
}

/// The warnings of railing `id`, called `name` in words, laid out as `layout`, in kind order.
///
/// # Errors
/// As [`Railing::rule_set`].
pub fn warnings(
    id: &Id,
    name: &str,
    railing: &Railing,
    layout: &Layout,
) -> Result<Vec<Warning>, RailingError> {
    let rules = railing.rule_set()?;
    let settings = railing.settings();
    let category = rules.category(&settings.category)?;
    let openings = rules.openings_for(category, &settings.openings)?;
    let warn = |kind: &'static str, words: String| Warning {
        kind,
        severity: Severity::Warning,
        elements: vec![id.clone()],
        words,
    };
    let kind = railing.kind();
    let height = kind.top_rail.height;
    let on_flight = layout
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .any(|s| s.rise != 0);
    let on_level = layout
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .any(|s| s.rise == 0);
    let mut out = Vec::new();
    if let Some(handrail) = &kind.handrail
        && on_flight
        && (handrail.height.steps() < rules.handrail_least.length.steps()
            || handrail.height.steps() > rules.handrail_most.length.steps())
    {
        out.push(warn(
            HANDRAIL,
            format!(
                "{name}: its handrail's top is {} above the pitch line; it stands from {} to {}.",
                words(handrail.height),
                cite(&rules.handrail_least),
                cite(&rules.handrail_most)
            ),
        ));
    }
    let low = [
        (
            on_flight,
            category.flight(),
            "on a flight, above the pitch line",
        ),
        (on_level, &category.level_least, "on the level"),
    ]
    .into_iter()
    .find(|(there, limit, _)| *there && height.steps() < limit.length.steps());
    if let Some((_, limit, place)) = low {
        out.push(warn(
            LOW,
            format!(
                "{name}: its top rail is {} high {place}, below the least barrier height there, {}.",
                words(height),
                cite(limit)
            ),
        ));
    }
    if let Some(words) = opening(name, &openings.limits, layout) {
        out.push(warn(OPENING, words));
    }
    out.sort_by(|a, b| a.kind.cmp(b.kind));
    Ok(out)
}
