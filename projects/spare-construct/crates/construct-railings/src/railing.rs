//! What a railing is, as a person sets it out: the rule set and the category and openings rule it
//! is checked by, its type, its path, and for a sketched path which side its handrail is on.
//!
//! Stored ([`Railing::to_args`], [`Railing::from_args`]), each length in construct-params' stored
//! spelling: the one spelling FORMAT.md gives, and anything else refused.

use crate::error::RailingError;
use crate::kind::RailingType;
use crate::path::{Line, MOST_INSET, Path, Side, check_points, sketched_line, stair_lines};
use crate::rules::RuleSet;
use construct_params::{Length, Value};
use construct_stairs::{Layout as StairLayout, Stair};
use kit_core::Id;
use serde_json::{Map, Value as Json, json};
use std::collections::BTreeMap;

/// The settings a railing is made from, each checked by [`Railing::new`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The rule set's ID.
    pub rules: String,
    /// Its category's ID: where the barrier stands.
    pub category: String,
    /// Its openings rule's ID: who uses the place.
    pub openings: String,
    /// Its type.
    pub kind: RailingType,
    /// Where it runs.
    pub path: Path,
    /// For a sketched path, its handrail on the right of the path's direction, not the left.
    pub flipped: bool,
}

/// A railing as a person sets it out, every check passed ([`Railing::new`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Railing {
    settings: Settings,
}

/// The stair a railing on a stair follows, laid out, and how far the stair's base stands above
/// the railing's.
#[derive(Clone, Copy, Debug)]
pub struct StairHost<'a> {
    /// The stair.
    pub stair: &'a Stair,
    /// Its layout.
    pub layout: &'a StairLayout,
    /// Its base above the railing's base.
    pub shift: Length,
}

fn bad(why: &str) -> RailingError {
    RailingError::Malformed(format!("A railing's {why}."))
}

/// A stored length.
fn stored(v: &Json, what: &str) -> Result<Length, RailingError> {
    v.as_str()
        .and_then(Value::from_stored)
        .and_then(|value| match value {
            Value::Length(l) => Some(l),
            _ => None,
        })
        .ok_or_else(|| bad(&format!("{what} is not a stored length")))
}

impl Railing {
    /// A railing from its settings, every check passed: its rule set held, its category in it,
    /// its openings rule one the category takes, its type's checks, a sketched path's checks
    /// ([`check_points`]), a stair's inset at least nothing and at most [`MOST_INSET`], and only
    /// a sketched path flipped (a railing on a stair flips by its side).
    ///
    /// # Errors
    /// The refusal of the first check that fails, in that order.
    pub fn new(settings: Settings) -> Result<Self, RailingError> {
        let rules = RuleSet::builtin(&settings.rules)?;
        let category = rules.category(&settings.category)?;
        rules.openings_for(category, &settings.openings)?;
        settings.kind.check()?;
        match &settings.path {
            Path::Sketched { points } => check_points(points)?,
            Path::OnStair { inset, .. } => {
                if inset.steps() < 0 {
                    return Err(RailingError::Negative {
                        what: "inset from the stair's edge",
                    });
                }
                if inset.steps() > MOST_INSET.steps() {
                    return Err(RailingError::OutOfRange {
                        what: format!("inset from the stair's edge, {inset}"),
                    });
                }
                if settings.flipped {
                    return Err(bad(
                        "handrail on a stair is on the stair's side: flip its side",
                    ));
                }
            }
        }
        Ok(Self { settings })
    }

    /// Its settings, to change one and make the railing again with [`Railing::new`].
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Its type.
    #[must_use]
    pub fn kind(&self) -> &RailingType {
        &self.settings.kind
    }

    /// Its path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.settings.path
    }

    /// The stair it follows, if it is on one.
    #[must_use]
    pub fn stair(&self) -> Option<&Id> {
        match &self.settings.path {
            Path::OnStair { stair, .. } => Some(stair),
            Path::Sketched { .. } => None,
        }
    }

    /// Its rule set.
    ///
    /// # Errors
    /// As [`RuleSet::builtin`]; [`Railing::new`] has read it already.
    pub fn rule_set(&self) -> Result<RuleSet, RailingError> {
        RuleSet::builtin(&self.settings.rules)
    }

    /// The same railing on the other side: a sketched path's handrail to its other side, a
    /// stair's railing to the stair's other side.
    ///
    /// # Errors
    /// [`RailingError::BothSides`] for a railing on both sides of a stair.
    pub fn flipped(&self) -> Result<Self, RailingError> {
        let mut s = self.settings.clone();
        match &mut s.path {
            Path::Sketched { .. } => s.flipped = !s.flipped,
            Path::OnStair { side, .. } => *side = side.flipped().ok_or(RailingError::BothSides)?,
        }
        Self::new(s)
    }

    /// Its lines in its own frame ([`crate::path`]): a sketched path's one, or the stair's sides
    /// as `host` lays them out.
    ///
    /// # Errors
    /// [`RailingError::Host`] for a railing on a stair given no stair, or a sketched one given
    /// one; any refusal of [`sketched_line`] or [`stair_lines`].
    pub fn lines(&self, host: Option<StairHost<'_>>) -> Result<Vec<Line>, RailingError> {
        match (&self.settings.path, host) {
            (Path::Sketched { points }, None) => {
                Ok(vec![sketched_line(points, self.settings.flipped)?])
            }
            (Path::OnStair { side, inset, .. }, Some(host)) => {
                stair_lines(host.stair, host.layout, *side, *inset, host.shift)
            }
            (Path::OnStair { stair, .. }, None) => Err(RailingError::Host(format!(
                "A railing on stair {stair} is laid out from that stair, and it is not in the model."
            ))),
            (Path::Sketched { .. }, Some(_)) => Err(RailingError::Host(
                "A sketched railing follows no stair.".to_owned(),
            )),
        }
    }

    /// Its stored arguments, as FORMAT.md writes them: `category`, `flipped`, `openings`,
    /// `path`, `rules` and `type`.
    #[must_use]
    pub fn to_args(&self) -> BTreeMap<String, Json> {
        let s = &self.settings;
        let mut args = BTreeMap::new();
        args.insert("category".to_owned(), json!(s.category));
        args.insert("flipped".to_owned(), json!(s.flipped));
        args.insert("openings".to_owned(), json!(s.openings));
        let path = match &s.path {
            Path::Sketched { points } => json!({
                "points": points
                    .iter()
                    .map(|p| json!([Value::Length(p[0]).stored(), Value::Length(p[1]).stored()]))
                    .collect::<Vec<_>>()
            }),
            Path::OnStair { stair, side, inset } => json!({
                "inset": Value::Length(*inset).stored(),
                "side": side.name(),
                "stair": stair.as_str(),
            }),
        };
        args.insert("path".to_owned(), path);
        args.insert("rules".to_owned(), json!(s.rules));
        args.insert("type".to_owned(), s.kind.to_json());
        args
    }

    /// A railing from its stored arguments, read strictly: as [`Railing::to_args`] writes them,
    /// and the same railing written back gives the same arguments. Arguments not named there are
    /// refused.
    ///
    /// # Errors
    /// [`RailingError::Malformed`] for an argument missing, added or not in its one spelling; any
    /// refusal of [`Railing::new`].
    pub fn from_args(args: &BTreeMap<String, Json>) -> Result<Self, RailingError> {
        let get = |name: &str| {
            args.get(name)
                .ok_or_else(|| bad(&format!("{name} is missing")))
        };
        let text = |name: &str| -> Result<String, RailingError> {
            get(name)?
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| bad(&format!("{name} is not text")))
        };
        let path = get("path")?
            .as_object()
            .ok_or_else(|| bad("path is an object"))?;
        let path = read_path(path)?;
        let railing = Self::new(Settings {
            rules: text("rules")?,
            category: text("category")?,
            openings: text("openings")?,
            kind: RailingType::from_stored(get("type")?)?,
            path,
            flipped: get("flipped")?
                .as_bool()
                .ok_or_else(|| bad("flipped is true or false"))?,
        })?;
        if railing.to_args() != *args {
            return Err(bad("arguments are not in their one spelling"));
        }
        Ok(railing)
    }
}

fn read_path(path: &Map<String, Json>) -> Result<Path, RailingError> {
    if let Some(points) = path.get("points") {
        if path.len() != 1 {
            return Err(bad("sketched path holds its points only"));
        }
        let points = points
            .as_array()
            .ok_or_else(|| bad("path's points are a list"))?
            .iter()
            .map(|p| match p.as_array().map(Vec::as_slice) {
                Some([x, y]) => Ok([stored(x, "point")?, stored(y, "point")?]),
                _ => Err(bad("point is two lengths")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Path::Sketched { points });
    }
    if path.len() != 3 {
        return Err(bad(
            "path is {\"points\"} or {\"stair\",\"side\",\"inset\"}",
        ));
    }
    let stair = path
        .get("stair")
        .and_then(Json::as_str)
        .and_then(|s| Id::parse(s).ok())
        .ok_or_else(|| bad("path's stair is an element's ID"))?;
    let side = path
        .get("side")
        .and_then(Json::as_str)
        .and_then(Side::from_name)
        .ok_or_else(|| bad("path's side is left, right or both"))?;
    let inset = stored(
        path.get("inset")
            .ok_or_else(|| bad("path's inset is missing"))?,
        "path's inset",
    )?;
    Ok(Path::OnStair { stair, side, inset })
}
