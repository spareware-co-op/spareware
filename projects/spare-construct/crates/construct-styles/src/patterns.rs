//! Line patterns: how a line is broken into dashes, gaps and dots.
//!
//! A pattern is a cycle of segments that repeats along the line. Every length is an exact printed
//! length in micrometres and does not change with the view's scale (a dash is as long on paper at
//! 1:20 as at 1:200). The six built-in patterns are always present and cannot be removed or
//! changed; a project adds its own, which get IDs from 100 up.
//!
//! A pattern's name and lengths here are our own. The family of line types they follow (a
//! continuous line, a dashed line, a dotted line, a long dash with a dot, a long dash with two
//! dots) is the one every drawing standard uses, such as ISO 128-20, "Technical drawings: general
//! principles of presentation: basic conventions for lines".

use crate::error::StyleError;
use crate::units::{Micrometres, check_name};
use serde::{Deserialize, Serialize};

/// The first ID given to a pattern a project adds. IDs below it are the built-ins' and reserved.
pub const FIRST_USER_PATTERN_ID: u32 = 100;

/// The most segments a pattern may have.
const MOST_SEGMENTS: usize = 64;

/// A pattern's permanent number.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct PatternId(pub u32);

/// The built-in continuous line.
pub const SOLID: PatternId = PatternId(1);
/// The built-in dashed line.
pub const DASHED: PatternId = PatternId(2);
/// The built-in dotted line.
pub const DOTTED: PatternId = PatternId(3);
/// The built-in dash-dot line, the centre line.
pub const DASH_DOT: PatternId = PatternId(4);
/// The built-in dash-dot-dot line.
pub const DASH_DOT_DOT: PatternId = PatternId(5);
/// The built-in hidden-edge line.
pub const HIDDEN: PatternId = PatternId(6);

/// One part of a pattern's cycle.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Segment {
    /// A drawn stretch of this length.
    Dash(Micrometres),
    /// An undrawn stretch of this length.
    Gap(Micrometres),
    /// A dot: a mark as wide as the line and of no length along it.
    Dot,
}

/// A line pattern: an ID, a name and the cycle of segments. An empty cycle is the continuous
/// line, which only the built-in [`SOLID`] is.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LinePattern {
    /// Its permanent number.
    pub id: PatternId,
    /// The name people see.
    pub name: String,
    /// The cycle, drawn start to end and then again.
    pub segments: Vec<Segment>,
}

impl LinePattern {
    /// The length of one cycle in micrometres: its dashes and gaps added up (a dot has none). It
    /// is zero for the continuous line.
    #[must_use]
    pub fn cycle_micrometres(&self) -> u32 {
        cycle_of(&self.segments)
    }
}

/// A built-in length. The built-in lengths are fixed and inside the allowed range.
const fn mm(micrometres: u32) -> Micrometres {
    Micrometres::built_in(micrometres)
}

fn cycle_of(segments: &[Segment]) -> u32 {
    segments.iter().fold(0_u32, |total, segment| match segment {
        Segment::Dash(length) | Segment::Gap(length) => total.saturating_add(length.get()),
        Segment::Dot => total,
    })
}

fn built_in(id: PatternId, name: &str, segments: Vec<Segment>) -> LinePattern {
    LinePattern {
        id,
        name: name.to_owned(),
        segments,
    }
}

/// The six built-in patterns, as this crate defines them.
fn built_ins() -> Vec<LinePattern> {
    use Segment::{Dash, Dot, Gap};
    vec![
        built_in(SOLID, "Solid", Vec::new()),
        built_in(DASHED, "Dashed", vec![Dash(mm(6000)), Gap(mm(2000))]),
        built_in(DOTTED, "Dotted", vec![Dot, Gap(mm(1500))]),
        built_in(
            DASH_DOT,
            "Dash-dot (centre)",
            vec![Dash(mm(12_000)), Gap(mm(2000)), Dot, Gap(mm(2000))],
        ),
        built_in(
            DASH_DOT_DOT,
            "Dash-dot-dot",
            vec![
                Dash(mm(12_000)),
                Gap(mm(2000)),
                Dot,
                Gap(mm(2000)),
                Dot,
                Gap(mm(2000)),
            ],
        ),
        built_in(HIDDEN, "Hidden", vec![Dash(mm(3000)), Gap(mm(1000))]),
    ]
}

/// The table of line patterns: the built-ins and the project's own.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(try_from = "RawPatternTable", into = "RawPatternTable")]
pub struct PatternTable {
    patterns: Vec<LinePattern>,
    next_id: u32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct RawPatternTable {
    patterns: Vec<LinePattern>,
    next_id: u32,
}

impl TryFrom<RawPatternTable> for PatternTable {
    type Error = StyleError;

    fn try_from(raw: RawPatternTable) -> Result<Self, StyleError> {
        let table = Self {
            patterns: raw.patterns,
            next_id: raw.next_id,
        };
        table.check()?;
        Ok(table)
    }
}

impl From<PatternTable> for RawPatternTable {
    fn from(table: PatternTable) -> Self {
        Self {
            patterns: table.patterns,
            next_id: table.next_id,
        }
    }
}

/// Refuses a user pattern's segments that would draw nothing, or draw on a point.
fn check_segments(segments: &[Segment]) -> Result<(), StyleError> {
    if segments.len() > MOST_SEGMENTS {
        return Err(StyleError::TooManySegments);
    }
    if !segments
        .iter()
        .any(|segment| matches!(segment, Segment::Dash(_) | Segment::Dot))
    {
        return Err(StyleError::NoMarks);
    }
    if cycle_of(segments) == 0 {
        return Err(StyleError::ZeroCycle);
    }
    Ok(())
}

impl PatternTable {
    /// The table with only the six built-in patterns.
    #[must_use]
    pub fn built_in() -> Self {
        Self {
            patterns: built_ins(),
            next_id: FIRST_USER_PATTERN_ID,
        }
    }

    /// Checks a table read from storage.
    fn check(&self) -> Result<(), StyleError> {
        let built = built_ins();
        if self.patterns.get(..built.len()) != Some(built.as_slice()) {
            return Err(StyleError::BuiltInsChanged);
        }
        let mut last = FIRST_USER_PATTERN_ID.saturating_sub(1);
        let mut names: Vec<&str> = built.iter().map(|pattern| pattern.name.as_str()).collect();
        for pattern in self.patterns.iter().skip(built.len()) {
            if pattern.id.0 <= last {
                return Err(StyleError::PatternsNotAscending);
            }
            last = pattern.id.0;
            check_name(&pattern.name)?;
            if names.contains(&pattern.name.as_str()) {
                return Err(StyleError::DuplicateName);
            }
            names.push(&pattern.name);
            check_segments(&pattern.segments)?;
        }
        if self.next_id <= last || self.next_id < FIRST_USER_PATTERN_ID {
            return Err(StyleError::PatternsNotAscending);
        }
        Ok(())
    }

    /// Every pattern, built-ins first, then the project's in ID order.
    #[must_use]
    pub fn patterns(&self) -> &[LinePattern] {
        &self.patterns
    }

    /// The pattern with this ID.
    #[must_use]
    pub fn get(&self, id: PatternId) -> Option<&LinePattern> {
        self.patterns.iter().find(|pattern| pattern.id == id)
    }

    /// The ID the next added pattern will get. An ID is never given out twice, even after the
    /// pattern that had it is removed.
    #[must_use]
    pub fn next_id(&self) -> PatternId {
        PatternId(self.next_id)
    }

    /// This table with a new pattern added, and the new pattern's ID.
    ///
    /// # Errors
    /// A [`StyleError`] when the name is bad or already used, the segments are too many, have no
    /// dash or dot, or add up to a cycle of no length, or no more IDs are left. The table itself
    /// is unchanged.
    pub fn with_pattern(
        &self,
        name: &str,
        segments: Vec<Segment>,
    ) -> Result<(Self, PatternId), StyleError> {
        check_name(name)?;
        if self.patterns.iter().any(|pattern| pattern.name == name) {
            return Err(StyleError::DuplicateName);
        }
        check_segments(&segments)?;
        let id = PatternId(self.next_id);
        let next_id = self
            .next_id
            .checked_add(1)
            .ok_or(StyleError::IdsExhausted)?;
        let mut patterns = self.patterns.clone();
        patterns.push(LinePattern {
            id,
            name: name.to_owned(),
            segments,
        });
        Ok((Self { patterns, next_id }, id))
    }

    /// This table without the pattern `id`.
    ///
    /// # Errors
    /// [`StyleError::BuiltIn`] for a built-in pattern, [`StyleError::NoSuchPattern`] when no
    /// pattern has the ID. The table itself is unchanged.
    pub fn without_pattern(&self, id: PatternId) -> Result<Self, StyleError> {
        if id.0 < FIRST_USER_PATTERN_ID {
            return Err(StyleError::BuiltIn { id: id.0 });
        }
        if self.get(id).is_none() {
            return Err(StyleError::NoSuchPattern { id: id.0 });
        }
        Ok(Self {
            patterns: self
                .patterns
                .iter()
                .filter(|pattern| pattern.id != id)
                .cloned()
                .collect(),
            next_id: self.next_id,
        })
    }
}
