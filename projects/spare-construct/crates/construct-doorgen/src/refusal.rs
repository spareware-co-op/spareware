//! Why a spec is not built: the rule it breaks, by name, and the reason in words, as a value a
//! person can read. Nothing here panics; every refusal is one of these.

use std::fmt;

/// A rule a spec or a generated type must keep. Each has a stable name ([`Rule::name`]) that a
/// refusal is reported under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    /// A family is generated from one spec or more.
    NoSpecs,
    /// Every spec of a family names the family's template.
    OtherTemplate,
    /// A type mark makes a type key: a letter, then letters, digits, spaces, `-` or `_`.
    Mark,
    /// No two specs of one family share a type mark (case, spaces, `-` and `_` aside).
    DuplicateMark,
    /// A spec sets only what its template has (a door has no glass, a window no leaf).
    NotInTemplate,
    /// A spec's hand is one its template draws.
    Hand,
    /// Every size and section is greater than zero (a sill height at or above the floor).
    Positive,
    /// The overall width is within the template's range.
    Width,
    /// The overall height is within the template's range.
    Height,
    /// The parts fit inside one another: a leaf, sashes and glass within the frame's depth.
    Fits,
    /// What is left inside the frame is an opening: a clear width and height greater than zero,
    /// and room for a sliding sash's mark on its glass.
    Clear,
    /// A length the template halves is a whole even number of grid steps, so its half lands on
    /// the grid exactly and nothing is rounded.
    Even,
    /// The unit's depth fits the wall it is for.
    Wall,
    /// The template itself reads (a fault of this crate's build, never of a spec).
    Template,
    /// The generated recipe passes the recipe checker.
    Checker,
    /// The generated type evaluates through the family evaluator.
    Evaluation,
    /// The opening the wall is cut by is clean: one closed rectangle through the wall's whole
    /// thickness, exactly the frame's outside, with every solid inside it.
    Opening,
    /// Every plan and elevation symbol lies within the opening.
    Symbol,
    /// The measured quantities are the ones the spec asks for.
    Quantity,
    /// A type's tags are the values its solids measure.
    Tag,
    /// The generated recipe reads back from its written form unchanged.
    RoundTrip,
}

impl Rule {
    /// Every rule, in the order a spec is judged by them.
    pub const ALL: [Self; 21] = [
        Self::NoSpecs,
        Self::OtherTemplate,
        Self::Mark,
        Self::DuplicateMark,
        Self::NotInTemplate,
        Self::Hand,
        Self::Positive,
        Self::Width,
        Self::Height,
        Self::Fits,
        Self::Clear,
        Self::Even,
        Self::Wall,
        Self::Template,
        Self::Checker,
        Self::Evaluation,
        Self::Opening,
        Self::Symbol,
        Self::Quantity,
        Self::Tag,
        Self::RoundTrip,
    ];

    /// The rule's stable name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::NoSpecs => "no-specs",
            Self::OtherTemplate => "other-template",
            Self::Mark => "mark",
            Self::DuplicateMark => "duplicate-mark",
            Self::NotInTemplate => "not-in-template",
            Self::Hand => "hand",
            Self::Positive => "positive",
            Self::Width => "width",
            Self::Height => "height",
            Self::Fits => "fits",
            Self::Clear => "clear",
            Self::Even => "even",
            Self::Wall => "wall",
            Self::Template => "template",
            Self::Checker => "checker",
            Self::Evaluation => "evaluation",
            Self::Opening => "opening",
            Self::Symbol => "symbol",
            Self::Quantity => "quantity",
            Self::Tag => "tag",
            Self::RoundTrip => "round-trip",
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A spec, or a generated type, refused: the rule, the field or part it is about, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The type mark of the spec refused, if the refusal is about one spec.
    pub mark: Option<String>,
    /// The rule broken.
    pub rule: Rule,
    /// The field, parameter or part it is about (`width`, `frame_depth`, `leaf`, `opening`).
    pub what: String,
    /// Why, in words, with the numbers.
    pub reason: String,
}

impl Refusal {
    /// A refusal under `rule` about `what`.
    pub(crate) fn new(rule: Rule, what: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            mark: None,
            rule,
            what: what.into(),
            reason: reason.into(),
        }
    }

    /// The same refusal, about the spec with this mark.
    #[must_use]
    pub(crate) fn of(mut self, mark: &str) -> Self {
        self.mark = Some(mark.to_owned());
        self
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(mark) = &self.mark {
            write!(f, "{mark}: ")?;
        }
        write!(f, "{} ({}): {}", self.rule, self.what, self.reason)
    }
}

impl std::error::Error for Refusal {}
