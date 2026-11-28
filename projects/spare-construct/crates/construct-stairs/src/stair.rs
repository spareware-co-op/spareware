//! What a stair is, as a person sets it out: its rule set and category, where it starts and which
//! way it runs, how wide it is, its going and waist, and its runs with the landings between them.
//! Every check here is of the stair's own shape; the code's limits are [`crate::layout`]'s.
//!
//! - **Start and direction.** `start` is the point in plan at the foot of the first riser, on the
//!   stair's right-hand side going up; the first run goes the way `direction` says, along one of
//!   the plan's axes ([`Direction`]), and the stair lies to the left of that line, `width` across.
//!   Runs along the axes keep every corner of the stair on the kernel's grid without rounding; a
//!   stair at another angle is a later version.
//! - **Runs and landings.** A stair is one or more straight runs, with one landing between each
//!   two ([`LandingSpec`]): a landing has a length along the run that comes into it and a
//!   [`Turn`]: straight on, a quarter turn left or right (an L), or a half turn back on the left or
//!   the right (a U). A run may give its count of risers ([`RunSpec`]), at least two; the runs that
//!   do not share what the others leave, as evenly as can be, the earlier ones taking one more.
//! - **Going and waist.** The going is the same on every tread; given none, the stair takes its
//!   category's least going. The waist is the least thickness of the structure under the steps,
//!   measured square to its underside from the steps' inner corners.
//!
//! Stored ([`Stair::to_args`], [`Stair::from_args`]), each length in construct-params' stored
//! spelling: the one spelling FORMAT.md gives, and anything else refused.

use crate::error::StairError;
use construct_params::{Length, Value};
use serde_json::{Map, Value as Json, json};
use std::collections::BTreeMap;

/// The most runs a stair takes.
pub const MOST_RUNS: usize = 64;
/// The most risers a stair takes, in all.
pub const MOST_RISERS: usize = 1000;
/// The longest width, going, waist or landing a stair takes: 100 m.
pub const MOST_LENGTH: Length = Length::from_mm(100_000);
/// The farthest a point may be from the origin along an axis: the kernel's grid, 2^40 - 1 steps.
pub const GRID_LIMIT: i64 = 1_099_511_627_775;

/// Which way the first run goes up, along one of the plan's axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Towards +x.
    PlusX,
    /// Towards -x.
    MinusX,
    /// Towards +y.
    PlusY,
    /// Towards -y.
    MinusY,
}

impl Direction {
    /// Every direction, in a fixed order.
    pub const ALL: [Self; 4] = [Self::PlusX, Self::PlusY, Self::MinusX, Self::MinusY];

    /// Its stored name: `+x`, `-x`, `+y` or `-y`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::PlusX => "+x",
            Self::MinusX => "-x",
            Self::PlusY => "+y",
            Self::MinusY => "-y",
        }
    }

    /// The direction of a stored name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.name() == name)
    }

    /// The unit vector in plan.
    #[must_use]
    pub fn unit(self) -> [i64; 2] {
        match self {
            Self::PlusX => [1, 0],
            Self::MinusX => [-1, 0],
            Self::PlusY => [0, 1],
            Self::MinusY => [0, -1],
        }
    }

    /// A quarter turn to the left (anticlockwise seen from above).
    #[must_use]
    pub fn left(self) -> Self {
        match self {
            Self::PlusX => Self::PlusY,
            Self::PlusY => Self::MinusX,
            Self::MinusX => Self::MinusY,
            Self::MinusY => Self::PlusX,
        }
    }

    /// A quarter turn to the right.
    #[must_use]
    pub fn right(self) -> Self {
        self.left().left().left()
    }

    /// Half a turn.
    #[must_use]
    pub fn back(self) -> Self {
        self.left().left()
    }
}

/// How a landing turns the stair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Turn {
    /// Straight on: the next run goes on the same way.
    Straight,
    /// A quarter turn left: the next run leaves the landing's left side.
    Left,
    /// A quarter turn right: the next run leaves the landing's right side.
    Right,
    /// A half turn: the next run comes back beside this one, on its left.
    BackLeft,
    /// A half turn: the next run comes back beside this one, on its right.
    BackRight,
}

impl Turn {
    /// Every turn, in a fixed order.
    pub const ALL: [Self; 5] = [
        Self::Straight,
        Self::Left,
        Self::Right,
        Self::BackLeft,
        Self::BackRight,
    ];

    /// Its stored name: `straight`, `left`, `right`, `back-left` or `back-right`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Straight => "straight",
            Self::Left => "left",
            Self::Right => "right",
            Self::BackLeft => "back-left",
            Self::BackRight => "back-right",
        }
    }

    /// The turn of a stored name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }

    /// Whether it is a quarter turn, whose landing must be as long as the stair is wide.
    #[must_use]
    pub fn is_quarter(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// One run as set out: how many risers it has, or none to share what the other runs leave.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct RunSpec {
    /// Its risers, at least two; `None` to take its share.
    pub risers: Option<u32>,
}

/// One landing as set out: its length along the run that comes into it, and how it turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LandingSpec {
    /// Its length along the run that comes into it.
    pub length: Length,
    /// How it turns the stair.
    pub turn: Turn,
}

/// A stair as a person sets it out, every check of its own shape passed (the module header).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stair {
    rules: String,
    category: String,
    start: [Length; 2],
    direction: Direction,
    width: Length,
    going: Option<Length>,
    waist: Length,
    runs: Vec<RunSpec>,
    landings: Vec<LandingSpec>,
}

/// The settings a stair is made from, each checked by [`Stair::new`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The rule set's ID.
    pub rules: String,
    /// The category's ID within it.
    pub category: String,
    /// The foot of the first riser, on the stair's right-hand side, in plan.
    pub start: [Length; 2],
    /// Which way the first run goes up.
    pub direction: Direction,
    /// How wide the stair is.
    pub width: Length,
    /// The going, or `None` for the category's least.
    pub going: Option<Length>,
    /// The waist.
    pub waist: Length,
    /// The runs, from the bottom.
    pub runs: Vec<RunSpec>,
    /// The landings, one between each two runs, from the bottom.
    pub landings: Vec<LandingSpec>,
}

fn positive(length: Length, what: &'static str) -> Result<(), StairError> {
    if length.steps() <= 0 {
        return Err(StairError::NotPositive { what });
    }
    if length.steps() > MOST_LENGTH.steps() {
        return Err(StairError::OutOfRange {
            what: format!("{what}, {length}"),
        });
    }
    Ok(())
}

impl Stair {
    /// A stair from its settings, every check of its own shape passed: a rule set and category
    /// named; at least one run and at most [`MOST_RUNS`]; one landing between each two runs; a
    /// run's count of risers at least two, and all of them at most [`MOST_RISERS`]; the width,
    /// waist, every landing and any going more than nothing and at most [`MOST_LENGTH`]; the start
    /// on the kernel's grid; and a landing that turns a quarter at least as long as the stair is
    /// wide.
    ///
    /// # Errors
    /// The refusal of the first check that fails, in that order.
    pub fn new(s: Settings) -> Result<Self, StairError> {
        if s.rules.trim().is_empty() || s.category.trim().is_empty() {
            return Err(StairError::Malformed(
                "A stair names its rule set and its category.".to_owned(),
            ));
        }
        if s.runs.is_empty() {
            return Err(StairError::NoRuns);
        }
        if s.runs.len() > MOST_RUNS {
            return Err(StairError::TooManyRuns { runs: s.runs.len() });
        }
        if s.landings.len().saturating_add(1) != s.runs.len() {
            return Err(StairError::LandingCount {
                runs: s.runs.len(),
                landings: s.landings.len(),
            });
        }
        let mut fixed = 0_usize;
        for (at, run) in s.runs.iter().enumerate() {
            if let Some(risers) = run.risers {
                if risers < 2 {
                    return Err(StairError::FewRisers {
                        run: at.saturating_add(1),
                        risers,
                    });
                }
                fixed = fixed.saturating_add(usize::try_from(risers).unwrap_or(usize::MAX));
            } else {
                fixed = fixed.saturating_add(2);
            }
        }
        if fixed > MOST_RISERS {
            return Err(StairError::OutOfRange {
                what: format!("a stair of {fixed} risers or more"),
            });
        }
        positive(s.width, "the width")?;
        positive(s.waist, "the waist")?;
        if let Some(going) = s.going {
            positive(going, "the going")?;
        }
        for (at, landing) in s.landings.iter().enumerate() {
            positive(landing.length, "a landing's length")?;
            if landing.turn.is_quarter() && landing.length.steps() < s.width.steps() {
                return Err(StairError::TurnTooShort {
                    landing: at.saturating_add(1),
                    length: landing.length,
                    width: s.width,
                });
            }
        }
        if s.start
            .iter()
            .any(|c| c.steps().unsigned_abs() > GRID_LIMIT.unsigned_abs())
        {
            return Err(StairError::OutOfRange {
                what: "the start, beyond the kernel's grid".to_owned(),
            });
        }
        Ok(Self {
            rules: s.rules,
            category: s.category,
            start: s.start,
            direction: s.direction,
            width: s.width,
            going: s.going,
            waist: s.waist,
            runs: s.runs,
            landings: s.landings,
        })
    }

    /// Its settings, to change one and make the stair again with [`Stair::new`].
    #[must_use]
    pub fn settings(&self) -> Settings {
        Settings {
            rules: self.rules.clone(),
            category: self.category.clone(),
            start: self.start,
            direction: self.direction,
            width: self.width,
            going: self.going,
            waist: self.waist,
            runs: self.runs.clone(),
            landings: self.landings.clone(),
        }
    }

    /// The rule set's ID.
    #[must_use]
    pub fn rules(&self) -> &str {
        &self.rules
    }

    /// The category's ID.
    #[must_use]
    pub fn category(&self) -> &str {
        &self.category
    }

    /// The foot of the first riser, on the stair's right-hand side.
    #[must_use]
    pub fn start(&self) -> [Length; 2] {
        self.start
    }

    /// Which way the first run goes up.
    #[must_use]
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Its width.
    #[must_use]
    pub fn width(&self) -> Length {
        self.width
    }

    /// Its going as set, or `None` for its category's least.
    #[must_use]
    pub fn going(&self) -> Option<Length> {
        self.going
    }

    /// Its waist.
    #[must_use]
    pub fn waist(&self) -> Length {
        self.waist
    }

    /// Its runs, from the bottom.
    #[must_use]
    pub fn runs(&self) -> &[RunSpec] {
        &self.runs
    }

    /// Its landings, from the bottom.
    #[must_use]
    pub fn landings(&self) -> &[LandingSpec] {
        &self.landings
    }

    /// Its stored arguments, as FORMAT.md writes them: `category`, `direction`, `going` (only
    /// when set), `landings`, `rules`, `runs`, `start`, `waist` and `width`.
    #[must_use]
    pub fn to_args(&self) -> BTreeMap<String, Json> {
        let stored = |l: Length| Json::from(Value::Length(l).stored());
        let mut args = BTreeMap::new();
        args.insert("category".to_owned(), Json::from(self.category.clone()));
        args.insert("direction".to_owned(), Json::from(self.direction.name()));
        if let Some(going) = self.going {
            args.insert("going".to_owned(), stored(going));
        }
        let landings: Vec<Json> = self
            .landings
            .iter()
            .map(|l| json!({ "length": Value::Length(l.length).stored(), "turn": l.turn.name() }))
            .collect();
        args.insert("landings".to_owned(), Json::Array(landings));
        args.insert("rules".to_owned(), Json::from(self.rules.clone()));
        let runs: Vec<Json> = self
            .runs
            .iter()
            .map(|r| match r.risers {
                Some(n) => json!({ "risers": n }),
                None => Json::Object(Map::new()),
            })
            .collect();
        args.insert("runs".to_owned(), Json::Array(runs));
        args.insert(
            "start".to_owned(),
            json!([
                Value::Length(self.start[0]).stored(),
                Value::Length(self.start[1]).stored()
            ]),
        );
        args.insert("waist".to_owned(), stored(self.waist));
        args.insert("width".to_owned(), stored(self.width));
        args
    }

    /// A stair from its stored arguments, read strictly: each as [`Stair::to_args`] writes it,
    /// and the same stair written back gives the same arguments. Arguments not named there are
    /// left alone (a recipe holds its rise beside them).
    ///
    /// # Errors
    /// [`StairError::Malformed`] for an argument missing or not in its one spelling; any refusal
    /// of [`Stair::new`].
    pub fn from_args(args: &BTreeMap<String, Json>) -> Result<Self, StairError> {
        let bad = |why: &str| StairError::Malformed(format!("A stair's {why}."));
        let get = |name: &str| {
            args.get(name)
                .ok_or_else(|| bad(&format!("{name} is missing")))
        };
        let text = |name: &str| -> Result<String, StairError> {
            get(name)?
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| bad(&format!("{name} is not text")))
        };
        let length = |v: &Json, what: &str| -> Result<Length, StairError> {
            v.as_str()
                .and_then(Value::from_stored)
                .and_then(|value| match value {
                    Value::Length(l) => Some(l),
                    _ => None,
                })
                .ok_or_else(|| bad(&format!("{what} is not a stored length")))
        };
        let direction = Direction::from_name(&text("direction")?)
            .ok_or_else(|| bad("direction is not +x, -x, +y or -y"))?;
        let start = match get("start")?.as_array().map(Vec::as_slice) {
            Some([x, y]) => [length(x, "start")?, length(y, "start")?],
            _ => return Err(bad("start is not two lengths")),
        };
        let going = args.get("going").map(|g| length(g, "going")).transpose()?;
        let mut runs = Vec::new();
        for run in get("runs")?
            .as_array()
            .ok_or_else(|| bad("runs is not a list"))?
        {
            let risers = match run.as_object().map(|o| (o.len(), o.get("risers"))) {
                Some((0, None)) => None,
                Some((1, Some(n))) => Some(
                    n.as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or_else(|| bad("run's risers is not a whole number"))?,
                ),
                _ => return Err(bad("run is not {} or {\"risers\":<n>}")),
            };
            runs.push(RunSpec { risers });
        }
        let mut landings = Vec::new();
        for landing in get("landings")?
            .as_array()
            .ok_or_else(|| bad("landings is not a list"))?
        {
            let object = landing
                .as_object()
                .filter(|o| o.len() == 2)
                .ok_or_else(|| bad("landing is not {\"length\",\"turn\"}"))?;
            let turn = object
                .get("turn")
                .and_then(Json::as_str)
                .and_then(Turn::from_name)
                .ok_or_else(|| bad("landing's turn is not one of its five"))?;
            let length = length(
                object
                    .get("length")
                    .ok_or_else(|| bad("landing has no length"))?,
                "landing's length",
            )?;
            landings.push(LandingSpec { length, turn });
        }
        let stair = Self::new(Settings {
            rules: text("rules")?,
            category: text("category")?,
            start,
            direction,
            width: length(get("width")?, "width")?,
            going,
            waist: length(get("waist")?, "waist")?,
            runs,
            landings,
        })?;
        let written = stair.to_args();
        let same = written.iter().all(|(k, v)| args.get(k) == Some(v))
            && (args.contains_key("going") == written.contains_key("going"));
        if !same {
            return Err(bad("arguments are not in their one spelling"));
        }
        Ok(stair)
    }
}
