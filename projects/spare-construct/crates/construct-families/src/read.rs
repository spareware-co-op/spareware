//! Reading a recipe's shape: JSON to [`Recipe`], taking exactly what the published schema takes
//! and refusing exactly what it refuses. What the shape means is not asked here (that is
//! [`crate::check`]), so the two can be held to agree, case by case, by the tests.
//!
//! The rules that make it agree with the schema (JSON Schema draft 2020-12):
//!
//! - Every object names its members; one it does not name is refused.
//! - An integer is any JSON number with no fractional part, however it is written (`1`, `1.0`
//!   and `1e0` are all 1), within the member's range.
//! - A string's length counts characters, and every pattern is ASCII, as the schema's are.
//! - A member written twice in one object is read as JSON values are: the last one counts.
//!   A recipe written by this crate never has one.

use crate::model::{
    Base, BuiltInType, Condition, Constraint, DefinitionRow, FORMAT_VERSION, Hosting, Key,
    MaterialRef, Opening, Operation, Parameter, ParameterValue, Plane, Recipe, Sketch, SketchArc,
    SketchCircle, SketchLine, SketchPoint, Step, Symbol, TypeValue, View, Visibility,
};
use construct_categories::CategoryId;
use construct_params::{Choice, Holder, Kind};
use construct_solver::{Side, Touch};
use kit_core::Id;
use serde_json::{Map, Value};
use std::fmt;

/// Why a text is not a recipe: where, as a JSON Pointer (RFC 6901; empty for the whole text),
/// and what was wanted there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadError {
    at: String,
    reason: String,
}

impl ReadError {
    pub(crate) fn new(at: &str, reason: impl Into<String>) -> Self {
        Self {
            at: at.to_owned(),
            reason: reason.into(),
        }
    }

    /// Where, as a JSON Pointer.
    #[must_use]
    pub fn at(&self) -> &str {
        &self.at
    }

    /// What was wanted there, for people.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.at.is_empty() {
            write!(f, "{}", self.reason)
        } else {
            write!(f, "at {}: {}", self.at, self.reason)
        }
    }
}

impl std::error::Error for ReadError {}

type Read<T> = Result<T, ReadError>;

/// A JSON Pointer reference token: `~` as `~0`, `/` as `~1`.
pub(crate) fn token(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

/// A value and where it is.
#[derive(Clone)]
struct Node<'a> {
    value: &'a Value,
    at: String,
}

/// An object whose members have been checked against the names it may have.
struct Object<'a> {
    map: &'a Map<String, Value>,
    at: String,
}

impl<'a> Object<'a> {
    fn get(&self, name: &str) -> Option<Node<'a>> {
        self.map.get(name).map(|value| Node {
            value,
            at: format!("{}/{}", self.at, token(name)),
        })
    }

    fn has(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    fn req(&self, name: &str) -> Read<Node<'a>> {
        self.get(name).ok_or_else(|| {
            ReadError::new(
                &format!("{}/{}", self.at, token(name)),
                format!("the member \"{name}\" is missing"),
            )
        })
    }
}

impl<'a> Node<'a> {
    fn fail<T>(&self, wanted: &str) -> Read<T> {
        Err(ReadError::new(&self.at, format!("wanted {wanted}")))
    }

    /// An object with no member but these.
    fn object(&self, names: &[&str]) -> Read<Object<'a>> {
        let Value::Object(map) = self.value else {
            return self.fail("an object");
        };
        if let Some(other) = map.keys().find(|name| !names.contains(&name.as_str())) {
            return Err(ReadError::new(
                &format!("{}/{}", self.at, token(other)),
                format!("no member \"{other}\" belongs here"),
            ));
        }
        Ok(Object {
            map,
            at: self.at.clone(),
        })
    }

    fn array(&self) -> Read<Vec<Node<'a>>> {
        let Value::Array(items) = self.value else {
            return self.fail("an array");
        };
        Ok(items
            .iter()
            .enumerate()
            .map(|(n, value)| Node {
                value,
                at: format!("{}/{n}", self.at),
            })
            .collect())
    }

    fn list<T>(&self, each: impl Fn(&Node<'a>) -> Read<T>) -> Read<Vec<T>> {
        self.array()?.iter().map(each).collect()
    }

    fn string(&self) -> Read<&'a str> {
        match self.value {
            Value::String(text) => Ok(text),
            _ => self.fail("a string"),
        }
    }

    /// A string with something in it.
    fn text(&self) -> Read<String> {
        let text = self.string()?;
        if text.is_empty() {
            return self.fail("a string that is not empty");
        }
        Ok(text.to_owned())
    }

    fn key(&self) -> Read<Key> {
        Key::parse(self.string()?).map_or_else(
            || {
                self.fail(
                    "a key: a lowercase letter, then up to 63 lowercase letters, digits and _",
                )
            },
            Ok,
        )
    }

    fn keys(&self) -> Read<Vec<Key>> {
        self.list(Node::key)
    }

    fn id(&self) -> Read<Id> {
        Id::parse(self.string()?).or_else(|_| {
            self.fail("an id: a letter or digit, then up to 63 letters, digits, ., _ and -")
        })
    }

    fn boolean(&self) -> Read<bool> {
        match self.value {
            Value::Bool(b) => Ok(*b),
            _ => self.fail("true or false"),
        }
    }

    /// An integer from `low` to `high`: any JSON number with no fractional part.
    fn whole(&self, low: u32, high: u32) -> Read<u32> {
        let wanted = format!("a whole number from {low} to {high}");
        let Value::Number(number) = self.value else {
            return self.fail(&wanted);
        };
        let whole = if let Some(n) = number.as_u64() {
            u32::try_from(n).ok()
        } else if number.as_i64().is_some() {
            None
        } else {
            number.as_f64().and_then(integral_u32)
        };
        match whole {
            Some(n) if (low..=high).contains(&n) => Ok(n),
            _ => self.fail(&wanted),
        }
    }

    /// One of these words.
    fn word(&self, words: &[&'static str]) -> Read<&'static str> {
        let text = self.string()?;
        match words.iter().find(|word| **word == text) {
            Some(word) => Ok(word),
            None => self.fail(&format!("one of {}", words.join(", "))),
        }
    }

    fn category(&self) -> Read<CategoryId> {
        self.whole(0, u32::MAX).map(CategoryId)
    }
}

/// A double with no fractional part, as a `u32`, if it is one.
fn integral_u32(f: f64) -> Option<u32> {
    if f.fract() != 0.0 || !(0.0..=f64::from(u32::MAX)).contains(&f) {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "f is whole and from 0 to u32::MAX, checked above, so the cast is exact"
    )]
    Some(f as u32)
}

/// A recipe from JSON text.
pub(crate) fn from_json(text: &str) -> Read<Recipe> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| ReadError::new("", format!("not JSON: {e}")))?;
    from_value(&value)
}

/// A recipe from a JSON value.
pub(crate) fn from_value(value: &Value) -> Read<Recipe> {
    let root = Node {
        value,
        at: String::new(),
    };
    let o = root.object(&[
        "format",
        "id",
        "name",
        "category",
        "parameters",
        "planes",
        "sketches",
        "steps",
        "symbols",
        "hosting",
        "types",
    ])?;
    o.req("format")?.whole(FORMAT_VERSION, FORMAT_VERSION)?;
    Ok(Recipe {
        id: o.req("id")?.id()?,
        name: o.req("name")?.text()?,
        category: o.req("category")?.category()?,
        parameters: o.req("parameters")?.list(parameter)?,
        planes: o.req("planes")?.list(plane)?,
        sketches: o.req("sketches")?.list(sketch)?,
        steps: o.req("steps")?.list(step)?,
        symbols: o.req("symbols")?.list(symbol)?,
        hosting: hosting(&o.req("hosting")?)?,
        types: o.req("types")?.list(built_in_type)?,
    })
}

/// The kinds, by the tag construct-params gives each.
const KINDS: [&str; 13] = [
    "length", "area", "volume", "angle", "slope", "number", "whole", "text", "yesno", "choice",
    "element", "material", "link",
];

fn parameter(node: &Node<'_>) -> Read<Parameter> {
    let o = node.object(&["key", "definition", "default", "formula"])?;
    let key = o.req("key")?.key()?;
    let definition = definition(&o.req("definition")?)?;
    let value = match (o.get("default"), o.get("formula")) {
        (Some(default), None) => ParameterValue::Default(default.text()?),
        (None, Some(formula)) => ParameterValue::Formula(formula.text()?),
        _ => return node.fail("a parameter with either a default or a formula, not both"),
    };
    Ok(Parameter {
        key,
        definition,
        value,
    })
}

fn definition(node: &Node<'_>) -> Read<DefinitionRow> {
    let o = node.object(&["id", "name", "kind", "holder", "choices"])?;
    let id = o.req("id")?.id()?;
    let name = o.req("name")?.text()?;
    let tag = o.req("kind")?.word(&KINDS)?;
    let kind = Kind::from_tag(tag).map_or_else(|| node.fail("a kind"), Ok)?;
    let holder = match o.req("holder")?.word(&["instance", "type"])? {
        "instance" => Holder::Instance,
        _ => Holder::Type,
    };
    let choices = o.req("choices")?.list(|item| {
        let c = item.object(&["key", "label"])?;
        Ok(Choice::new(
            c.req("key")?.string()?,
            c.req("label")?.string()?,
        ))
    })?;
    Ok(DefinitionRow {
        id,
        name,
        kind,
        holder,
        choices,
    })
}

fn plane(node: &Node<'_>) -> Read<Plane> {
    let o = node.object(&["key", "base", "offset"])?;
    Ok(Plane {
        key: o.req("key")?.key()?,
        base: match o.req("base")?.word(&["xy", "xz", "yz"])? {
            "xy" => Base::Xy,
            "xz" => Base::Xz,
            _ => Base::Yz,
        },
        offset: o.req("offset")?.text()?,
    })
}

fn sketch(node: &Node<'_>) -> Read<Sketch> {
    let o = node.object(&[
        "key",
        "plane",
        "points",
        "lines",
        "arcs",
        "circles",
        "constraints",
    ])?;
    Ok(Sketch {
        key: o.req("key")?.key()?,
        plane: o.req("plane")?.key()?,
        points: o.req("points")?.list(|item| {
            let p = item.object(&["key", "x", "y"])?;
            Ok(SketchPoint {
                key: p.req("key")?.key()?,
                x: p.req("x")?.text()?,
                y: p.req("y")?.text()?,
            })
        })?,
        lines: o.req("lines")?.list(|item| {
            let l = item.object(&["key", "start", "end"])?;
            Ok(SketchLine {
                key: l.req("key")?.key()?,
                start: l.req("start")?.key()?,
                end: l.req("end")?.key()?,
            })
        })?,
        arcs: o.req("arcs")?.list(|item| {
            let a = item.object(&["key", "centre", "start", "end", "radius"])?;
            Ok(SketchArc {
                key: a.req("key")?.key()?,
                centre: a.req("centre")?.key()?,
                start: a.req("start")?.key()?,
                end: a.req("end")?.key()?,
                radius: a.req("radius")?.text()?,
            })
        })?,
        circles: o.req("circles")?.list(|item| {
            let c = item.object(&["key", "centre", "radius"])?;
            Ok(SketchCircle {
                key: c.req("key")?.key()?,
                centre: c.req("centre")?.key()?,
                radius: c.req("radius")?.text()?,
            })
        })?,
        constraints: o.req("constraints")?.list(constraint)?,
    })
}

/// Every constraint's name, in the solver's order.
pub(crate) const CONSTRAINTS: [&str; 17] = [
    "coincident",
    "horizontal",
    "vertical",
    "parallel",
    "perpendicular",
    "equal_length",
    "distance",
    "point_line_distance",
    "fixed",
    "point_on_line",
    "point_on_curve",
    "concentric",
    "equal_radius",
    "radius",
    "diameter",
    "tangent_line",
    "tangent_curves",
];

/// The members each constraint has after `kind`, in the order they are written.
pub(crate) fn constraint_members(kind: &str) -> &'static [&'static str] {
    match kind {
        "horizontal" | "vertical" => &["kind", "line"],
        "distance" => &["kind", "a", "b", "length"],
        "point_line_distance" => &["kind", "point", "line", "length", "side"],
        "fixed" => &["kind", "point"],
        "point_on_line" => &["kind", "point", "line"],
        "point_on_curve" => &["kind", "point", "curve"],
        "radius" | "diameter" => &["kind", "curve", "length"],
        "tangent_line" => &["kind", "line", "curve", "side"],
        "tangent_curves" => &["kind", "a", "b", "touch"],
        _ => &["kind", "a", "b"],
    }
}

fn side(node: &Node<'_>) -> Read<Side> {
    Ok(match node.word(&["left", "right"])? {
        "left" => Side::Left,
        _ => Side::Right,
    })
}

fn constraint(node: &Node<'_>) -> Read<Constraint> {
    let Value::Object(map) = node.value else {
        return node.fail("an object");
    };
    let kind_node = Node {
        value: map.get("kind").unwrap_or(&Value::Null),
        at: format!("{}/kind", node.at),
    };
    let kind = kind_node.word(&CONSTRAINTS)?;
    let o = node.object(constraint_members(kind))?;
    let k = |name: &str| o.req(name)?.key();
    let length = || o.req("length")?.text();
    Ok(match kind {
        "coincident" => Constraint::Coincident {
            a: k("a")?,
            b: k("b")?,
        },
        "horizontal" => Constraint::Horizontal { line: k("line")? },
        "vertical" => Constraint::Vertical { line: k("line")? },
        "parallel" => Constraint::Parallel {
            a: k("a")?,
            b: k("b")?,
        },
        "perpendicular" => Constraint::Perpendicular {
            a: k("a")?,
            b: k("b")?,
        },
        "equal_length" => Constraint::EqualLength {
            a: k("a")?,
            b: k("b")?,
        },
        "distance" => Constraint::Distance {
            a: k("a")?,
            b: k("b")?,
            length: length()?,
        },
        "point_line_distance" => Constraint::PointLineDistance {
            point: k("point")?,
            line: k("line")?,
            length: length()?,
            side: side(&o.req("side")?)?,
        },
        "fixed" => Constraint::Fixed { point: k("point")? },
        "point_on_line" => Constraint::PointOnLine {
            point: k("point")?,
            line: k("line")?,
        },
        "point_on_curve" => Constraint::PointOnCurve {
            point: k("point")?,
            curve: k("curve")?,
        },
        "concentric" => Constraint::Concentric {
            a: k("a")?,
            b: k("b")?,
        },
        "equal_radius" => Constraint::EqualRadius {
            a: k("a")?,
            b: k("b")?,
        },
        "radius" => Constraint::Radius {
            curve: k("curve")?,
            length: length()?,
        },
        "diameter" => Constraint::Diameter {
            curve: k("curve")?,
            length: length()?,
        },
        "tangent_line" => Constraint::TangentLine {
            line: k("line")?,
            curve: k("curve")?,
            side: side(&o.req("side")?)?,
        },
        _ => Constraint::TangentCurves {
            a: k("a")?,
            b: k("b")?,
            touch: match o
                .req("touch")?
                .word(&["outside", "first_inside", "second_inside"])?
            {
                "outside" => Touch::Outside,
                "first_inside" => Touch::FirstInside,
                _ => Touch::SecondInside,
            },
        },
    })
}

fn visibility(node: &Node<'_>) -> Read<Visibility> {
    let o = node.object(&["coarse", "medium", "fine"])?;
    Ok(Visibility {
        coarse: o.req("coarse")?.boolean()?,
        medium: o.req("medium")?.boolean()?,
        fine: o.req("fine")?.boolean()?,
    })
}

fn material(node: &Node<'_>) -> Read<MaterialRef> {
    let o = node.object(&["parameter", "fixed"])?;
    match (o.has("parameter"), o.has("fixed")) {
        (true, false) => Ok(MaterialRef::Parameter(o.req("parameter")?.key()?)),
        (false, true) => Ok(MaterialRef::Fixed(o.req("fixed")?.id()?)),
        _ => node.fail("a material: either a parameter or a fixed material"),
    }
}

fn step(node: &Node<'_>) -> Read<Step> {
    let Value::Object(map) = node.value else {
        return node.fail("an object");
    };
    let op_node = Node {
        value: map.get("op").unwrap_or(&Value::Null),
        at: format!("{}/op", node.at),
    };
    let op = op_node.word(&["extrude", "subtract", "unite"])?;
    let members: &[&str] = match op {
        "extrude" => &[
            "key",
            "op",
            "sketch",
            "profile",
            "holes",
            "start",
            "end",
            "subcategory",
            "material",
            "visibility",
        ],
        "subtract" => &[
            "key",
            "op",
            "target",
            "tool",
            "subcategory",
            "material",
            "visibility",
        ],
        _ => &[
            "key",
            "op",
            "a",
            "b",
            "subcategory",
            "material",
            "visibility",
        ],
    };
    let o = node.object(members)?;
    let key = o.req("key")?.key()?;
    let operation = match op {
        "extrude" => Operation::Extrude {
            sketch: o.req("sketch")?.key()?,
            profile: o.req("profile")?.keys()?,
            holes: o.req("holes")?.list(Node::keys)?,
            start: o.req("start")?.text()?,
            end: o.req("end")?.text()?,
        },
        "subtract" => Operation::Subtract {
            target: o.req("target")?.key()?,
            tool: o.req("tool")?.key()?,
        },
        _ => Operation::Unite {
            a: o.req("a")?.key()?,
            b: o.req("b")?.key()?,
        },
    };
    Ok(Step {
        key,
        operation,
        subcategory: o.req("subcategory")?.category()?,
        material: material(&o.req("material")?)?,
        visibility: visibility(&o.req("visibility")?)?,
    })
}

fn symbol(node: &Node<'_>) -> Read<Symbol> {
    let o = node.object(&[
        "key",
        "view",
        "sketch",
        "edges",
        "subcategory",
        "visibility",
        "when",
    ])?;
    Ok(Symbol {
        key: o.req("key")?.key()?,
        view: match o.req("view")?.word(&["plan", "elevation"])? {
            "plan" => View::Plan,
            _ => View::Elevation,
        },
        sketch: o.req("sketch")?.key()?,
        edges: o.req("edges")?.keys()?,
        subcategory: o.req("subcategory")?.category()?,
        visibility: visibility(&o.req("visibility")?)?,
        when: match o.get("when") {
            None => None,
            Some(when) => {
                let w = when.object(&["parameter", "is"])?;
                Some(Condition {
                    parameter: w.req("parameter")?.key()?,
                    is: w.req("is")?.text()?,
                })
            }
        },
    })
}

fn hosting(node: &Node<'_>) -> Read<Hosting> {
    let Value::Object(map) = node.value else {
        return node.fail("an object");
    };
    let host_node = Node {
        value: map.get("host").unwrap_or(&Value::Null),
        at: format!("{}/host", node.at),
    };
    let host = host_node.word(&["unhosted", "wall", "floor", "face"])?;
    if host != "wall" {
        node.object(&["host"])?;
        return Ok(match host {
            "unhosted" => Hosting::Unhosted,
            "floor" => Hosting::Floor,
            _ => Hosting::Face,
        });
    }
    let o = node.object(&["host", "opening"])?;
    let opening = o.req("opening")?;
    let p = opening.object(&["sketch", "profile"])?;
    Ok(Hosting::Wall {
        opening: Opening {
            sketch: p.req("sketch")?.key()?,
            profile: p.req("profile")?.keys()?,
        },
    })
}

fn built_in_type(node: &Node<'_>) -> Read<BuiltInType> {
    let o = node.object(&["key", "name", "values"])?;
    Ok(BuiltInType {
        key: o.req("key")?.key()?,
        name: o.req("name")?.text()?,
        values: o.req("values")?.list(|item| {
            let v = item.object(&["parameter", "value"])?;
            Ok(TypeValue {
                parameter: v.req("parameter")?.key()?,
                value: v.req("value")?.text()?,
            })
        })?,
    })
}
