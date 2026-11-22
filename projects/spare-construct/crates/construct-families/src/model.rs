//! What a recipe holds, as read: every field of the format, in the format's own terms. A recipe
//! here has the format's shape (the reader made sure of that); whether it means anything is the
//! checker's question ([`crate::Recipe::check`]).
//!
//! Text the format keeps as text stays text here: formulas (construct-formulas' language) and
//! values in construct-params' stored spelling. The checker reads them; the writer writes them
//! back exactly, so nothing is respelt on the way through.

use construct_categories::CategoryId;
use construct_params::{Choice, Holder, Kind};
use construct_solver::{Side, Touch};
use kit_core::Id;
use std::fmt;

/// The format version this crate reads and writes.
pub const FORMAT_VERSION: u32 = 1;

/// The longest key.
const MAX_KEY: usize = 64;

/// A name within one recipe: a lowercase ASCII letter, then up to 63 lowercase ASCII letters,
/// digits and underscores. A parameter's key is also its name in formulas.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(String);

impl Key {
    /// The key, if `text` is one.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        is_key(text).then(|| Self(text.to_owned()))
    }

    /// The key as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether `text` is a key: the schema's pattern `^[a-z][a-z0-9_]{0,63}$`.
fn is_key(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && text.len() <= MAX_KEY
}

/// A family recipe: the template its elements are made from. Lists keep the order they were
/// written in; the order of `steps` is the order they are built in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    /// The family's permanent id. Every version of the family keeps it.
    pub id: Id,
    /// The name people see.
    pub name: String,
    /// The family's category (construct-categories).
    pub category: CategoryId,
    /// Its parameters.
    pub parameters: Vec<Parameter>,
    /// The planes its sketches lie on.
    pub planes: Vec<Plane>,
    /// Its sketches.
    pub sketches: Vec<Sketch>,
    /// Its solid steps, in build order.
    pub steps: Vec<Step>,
    /// Its 2D symbols.
    pub symbols: Vec<Symbol>,
    /// What it is placed on, and what it cuts there.
    pub hosting: Hosting,
    /// Its built-in types.
    pub types: Vec<BuiltInType>,
}

/// One parameter: a construct-params definition, the key formulas know it by, and its default
/// or the formula it is worked out from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameter {
    /// Its key, and its name in formulas.
    pub key: Key,
    /// Its definition's fields, as construct-params' own JSON has them.
    pub definition: DefinitionRow,
    /// Its default, or its formula.
    pub value: ParameterValue,
}

/// A construct-params definition as written, before construct-params' own checks (the checker
/// makes the `construct_params::Definition` from it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefinitionRow {
    /// The parameter's permanent id: what values are stored under in an element's record.
    pub id: Id,
    /// The name people see.
    pub name: String,
    /// The kind of value it takes.
    pub kind: Kind,
    /// Whether each instance or the type holds its value.
    pub holder: Holder,
    /// A choice parameter's list; empty for every other kind.
    pub choices: Vec<Choice>,
}

/// Where a parameter's value comes from when nothing sets it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParameterValue {
    /// A default, in construct-params' stored text (`length:q0:5248000`). Types and instances may set
    /// their own value, as its holder allows.
    Default(String),
    /// A formula of other parameters (construct-formulas). Nothing sets it: it is worked out.
    Formula(String),
}

/// The family plane a plane is parallel to. Each names its two in-plane axes, which are a
/// sketch's first and second coordinates, and its normal, the way extrusions run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    /// Coordinates along x and y; normal +z. A plan.
    Xy,
    /// Coordinates along x and z; normal +y. An elevation, parallel to a host wall.
    Xz,
    /// Coordinates along y and z; normal +x. A side elevation.
    Yz,
}

impl Base {
    /// Its spelling in a recipe.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Xy => "xy",
            Self::Xz => "xz",
            Self::Yz => "yz",
        }
    }
}

/// A named plane: parallel to a family plane, at an offset along its normal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plane {
    /// Its key.
    pub key: Key,
    /// The family plane it is parallel to.
    pub base: Base,
    /// How far along the normal, a length formula.
    pub offset: String,
}

/// A sketch: construct-solver's points, lines, arcs, circles and constraints on one plane, each
/// named by a key that is unique within the sketch, with a formula wherever the solver takes a
/// length. Points and radii are where they are drawn, the solver's starting place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sketch {
    /// Its key.
    pub key: Key,
    /// The plane it lies on.
    pub plane: Key,
    /// Its points.
    pub points: Vec<SketchPoint>,
    /// Its straight lines.
    pub lines: Vec<SketchLine>,
    /// Its arcs.
    pub arcs: Vec<SketchArc>,
    /// Its circles.
    pub circles: Vec<SketchCircle>,
    /// Its constraints, in the solver's order.
    pub constraints: Vec<Constraint>,
}

/// A point, where it is drawn: two lengths in stored text, along the plane's first and second
/// axes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SketchPoint {
    /// Its key.
    pub key: Key,
    /// Along the plane's first axis.
    pub x: String,
    /// Along the plane's second axis.
    pub y: String,
}

/// A straight line between two points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SketchLine {
    /// Its key.
    pub key: Key,
    /// The point it starts at.
    pub start: Key,
    /// The point it ends at.
    pub end: Key,
}

/// Part of a circle, anticlockwise about its centre from its start to its end, as the solver's
/// arc is; its radius as drawn in stored text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SketchArc {
    /// Its key.
    pub key: Key,
    /// Its centre point.
    pub centre: Key,
    /// The point it starts at.
    pub start: Key,
    /// The point it ends at.
    pub end: Key,
    /// Its radius as drawn.
    pub radius: String,
}

/// A whole circle; its radius as drawn in stored text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SketchCircle {
    /// Its key.
    pub key: Key,
    /// Its centre point.
    pub centre: Key,
    /// Its radius as drawn.
    pub radius: String,
}

/// One of construct-solver's constraints, by the same name, naming points, lines and curves (arcs
/// or circles) by key; a length is a formula.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Constraint {
    /// Two points are one place.
    Coincident {
        /// A point.
        a: Key,
        /// A point.
        b: Key,
    },
    /// A line runs along the plane's first axis.
    Horizontal {
        /// The line.
        line: Key,
    },
    /// A line runs along the plane's second axis.
    Vertical {
        /// The line.
        line: Key,
    },
    /// Two lines run the same way.
    Parallel {
        /// A line.
        a: Key,
        /// A line.
        b: Key,
    },
    /// Two lines meet at a right angle.
    Perpendicular {
        /// A line.
        a: Key,
        /// A line.
        b: Key,
    },
    /// Two lines are the same length.
    EqualLength {
        /// A line.
        a: Key,
        /// A line.
        b: Key,
    },
    /// Two points are this far apart.
    Distance {
        /// A point.
        a: Key,
        /// A point.
        b: Key,
        /// The distance, a length formula.
        length: String,
    },
    /// A point is this far from a line's carrier, on the side given.
    PointLineDistance {
        /// The point.
        point: Key,
        /// The line.
        line: Key,
        /// The distance, a length formula.
        length: String,
        /// Which side of the line, looking from its start to its end.
        side: Side,
    },
    /// A point stays where it is drawn.
    Fixed {
        /// The point.
        point: Key,
    },
    /// A point lies on a line's carrier.
    PointOnLine {
        /// The point.
        point: Key,
        /// The line.
        line: Key,
    },
    /// A point lies on a curve's circle.
    PointOnCurve {
        /// The point.
        point: Key,
        /// The arc or circle.
        curve: Key,
    },
    /// Two curves share a centre.
    Concentric {
        /// An arc or circle.
        a: Key,
        /// An arc or circle.
        b: Key,
    },
    /// Two curves have the same radius.
    EqualRadius {
        /// An arc or circle.
        a: Key,
        /// An arc or circle.
        b: Key,
    },
    /// A curve's radius.
    Radius {
        /// The arc or circle.
        curve: Key,
        /// The radius, a length formula.
        length: String,
    },
    /// A curve's diameter.
    Diameter {
        /// The arc or circle.
        curve: Key,
        /// The diameter, a length formula.
        length: String,
    },
    /// A line's carrier touches a curve's circle, the curve's centre on the side given.
    TangentLine {
        /// The line.
        line: Key,
        /// The arc or circle.
        curve: Key,
        /// Which side of the line the curve's centre is on.
        side: Side,
    },
    /// Two curves' circles touch, the way given.
    TangentCurves {
        /// An arc or circle.
        a: Key,
        /// An arc or circle.
        b: Key,
        /// How they touch.
        touch: Touch,
    },
}

/// One solid step. A step a later step uses is that step's input and is not drawn itself; the
/// subcategory, material and visibility drawn are those of the step that uses it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// Its key.
    pub key: Key,
    /// What it makes.
    pub operation: Operation,
    /// The subcategory it is drawn with: the family's category or one of its subcategories.
    pub subcategory: CategoryId,
    /// Its material.
    pub material: MaterialRef,
    /// Which detail levels show it.
    pub visibility: Visibility,
}

/// What a step makes: only the operations the kernel's `Engine` interface names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    /// A sketch's closed profile of lines, less its holes, swept along its plane's normal from
    /// `start` to `end` (length formulas, measured from the plane).
    Extrude {
        /// The sketch.
        sketch: Key,
        /// The outer loop: lines of the sketch, each sharing a point with the next, the last
        /// with the first.
        profile: Vec<Key>,
        /// Loops inside it, each as `profile`.
        holes: Vec<Vec<Key>>,
        /// Where it starts along the normal.
        start: String,
        /// Where it ends along the normal.
        end: String,
    },
    /// An earlier step less another.
    Subtract {
        /// What is cut.
        target: Key,
        /// What cuts it.
        tool: Key,
    },
    /// Two earlier steps as one solid.
    Unite {
        /// One step.
        a: Key,
        /// The other.
        b: Key,
    },
}

/// A material: fixed, or the value of a material parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MaterialRef {
    /// The value of this material parameter.
    Parameter(Key),
    /// This material (construct-materials' permanent id).
    Fixed(Id),
}

/// Which of a view's detail levels show something.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one yes or no per detail level, as the format writes them"
)]
pub struct Visibility {
    /// Coarse.
    pub coarse: bool,
    /// Medium.
    pub medium: bool,
    /// Fine.
    pub fine: bool,
}

/// The kind of view a symbol is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// Plans: drawn from a sketch on an xy plane.
    Plan,
    /// Front elevations: drawn from a sketch on an xz plane.
    Elevation,
}

/// 2D lines drawn in one kind of view in place of, or beside, what the solids show there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    /// Its key.
    pub key: Key,
    /// The kind of view.
    pub view: View,
    /// The sketch its lines come from.
    pub sketch: Key,
    /// The lines, arcs and circles of the sketch it draws.
    pub edges: Vec<Key>,
    /// The subcategory it is drawn with (its line style comes from construct-styles' object
    /// styles for that subcategory).
    pub subcategory: CategoryId,
    /// Which detail levels show it.
    pub visibility: Visibility,
    /// Drawn only while this holds; always when `None`.
    pub when: Option<Condition>,
}

/// A yes or no, or choice, parameter having one value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    /// The parameter.
    pub parameter: Key,
    /// The value, in stored text.
    pub is: String,
}

/// What a family is placed on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hosting {
    /// Nothing: it stands on its own.
    Unhosted,
    /// A wall, cutting the opening given through the wall's whole thickness.
    Wall {
        /// The opening.
        opening: Opening,
    },
    /// A floor.
    Floor,
    /// Any face.
    Face,
}

/// An opening a hosted family cuts: a closed profile of lines in a sketch on an xz plane,
/// parallel to the wall.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opening {
    /// The sketch.
    pub sketch: Key,
    /// The loop, as an extrusion's profile.
    pub profile: Vec<Key>,
}

/// A built-in type: a name and its values of parameters the type holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuiltInType {
    /// Its key.
    pub key: Key,
    /// The name people see.
    pub name: String,
    /// Its values.
    pub values: Vec<TypeValue>,
}

/// One value a built-in type sets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeValue {
    /// The parameter.
    pub parameter: Key,
    /// The value, in stored text.
    pub value: String,
}
