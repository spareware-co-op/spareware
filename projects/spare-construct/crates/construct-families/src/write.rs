//! The one written form of a recipe, and the content id named by it.
//!
//! The form: JSON laid out with two spaces of indent, one member or item to a line, `": "`
//! between a member's name and its value, an empty array as `[]`, and one newline at the end
//! (`serde_json`'s pretty layout); members in the fixed order FORMAT.md gives; integers in plain
//! decimal; strings escaped only where JSON must (`"`, `\` and the control characters, with
//! `\b`, `\f`, `\n`, `\r` and `\t` for those that have them and `\u00XX` in lowercase for the
//! rest), everything else written as it is in UTF-8; and an optional member written only when
//! present. The same recipe is therefore always the same bytes, and reading what was written
//! gives the same recipe back.

use crate::model::{
    BuiltInType, Constraint, FORMAT_VERSION, Hosting, MaterialRef, Operation, Parameter,
    ParameterValue, Plane, Recipe, Sketch, Step, Symbol, TypeTable, View, Visibility,
};
use construct_params::Holder;
use construct_solver::{Side, Touch};
use sha2::{Digest, Sha256};
use std::fmt;

/// An ordered JSON tree: what is written, in the order it is written.
enum Tree {
    Text(String),
    Whole(u64),
    Bool(bool),
    List(Vec<Tree>),
    Members(Vec<(&'static str, Tree)>),
}

fn text(s: &str) -> Tree {
    Tree::Text(s.to_owned())
}

fn texts<T: AsRef<str>>(items: &[T]) -> Tree {
    Tree::List(items.iter().map(|item| text(item.as_ref())).collect())
}

/// Writes `s` as a JSON string.
fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                out.push_str("\\u00");
                for nibble in [u32::from(c) >> 4, u32::from(c) & 0xf] {
                    out.push(char::from_digit(nibble, 16).unwrap_or('0'));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn write_tree(out: &mut String, tree: &Tree, depth: usize) {
    let inner = depth.saturating_add(1);
    match tree {
        Tree::Text(s) => string(out, s),
        Tree::Whole(n) => out.push_str(&n.to_string()),
        Tree::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Tree::List(items) if items.is_empty() => out.push_str("[]"),
        Tree::Members(members) if members.is_empty() => out.push_str("{}"),
        Tree::List(items) => {
            out.push('[');
            for (n, item) in items.iter().enumerate() {
                out.push_str(if n == 0 { "\n" } else { ",\n" });
                indent(out, inner);
                write_tree(out, item, inner);
            }
            out.push('\n');
            indent(out, depth);
            out.push(']');
        }
        Tree::Members(members) => {
            out.push('{');
            for (n, (name, value)) in members.iter().enumerate() {
                out.push_str(if n == 0 { "\n" } else { ",\n" });
                indent(out, inner);
                string(out, name);
                out.push_str(": ");
                write_tree(out, value, inner);
            }
            out.push('\n');
            indent(out, depth);
            out.push('}');
        }
    }
}

/// The canonical text of a recipe.
pub(crate) fn to_json(recipe: &Recipe) -> String {
    let mut out = String::new();
    write_tree(&mut out, &tree(recipe), 0);
    out.push('\n');
    out
}

fn tree(recipe: &Recipe) -> Tree {
    let mut members = vec![
        ("format", Tree::Whole(u64::from(FORMAT_VERSION))),
        ("id", text(recipe.id.as_str())),
        ("name", text(&recipe.name)),
        ("category", Tree::Whole(u64::from(recipe.category.0))),
        (
            "parameters",
            Tree::List(recipe.parameters.iter().map(parameter).collect()),
        ),
        (
            "planes",
            Tree::List(recipe.planes.iter().map(plane).collect()),
        ),
        (
            "sketches",
            Tree::List(recipe.sketches.iter().map(sketch).collect()),
        ),
        ("steps", Tree::List(recipe.steps.iter().map(step).collect())),
        (
            "symbols",
            Tree::List(recipe.symbols.iter().map(symbol).collect()),
        ),
        ("hosting", hosting(&recipe.hosting)),
        (
            "types",
            Tree::List(recipe.types.iter().map(built_in_type).collect()),
        ),
    ];
    if let Some(table) = &recipe.type_table {
        members.push(("type_table", type_table(table)));
    }
    Tree::Members(members)
}

fn parameter(p: &Parameter) -> Tree {
    let d = &p.definition;
    let definition = Tree::Members(vec![
        ("id", text(d.id.as_str())),
        ("name", text(&d.name)),
        ("kind", text(d.kind.tag())),
        (
            "holder",
            text(match d.holder {
                Holder::Instance => "instance",
                Holder::Type => "type",
            }),
        ),
        (
            "choices",
            Tree::List(
                d.choices
                    .iter()
                    .map(|c| Tree::Members(vec![("key", text(&c.key)), ("label", text(&c.label))]))
                    .collect(),
            ),
        ),
    ]);
    let value = match &p.value {
        ParameterValue::Default(stored) => ("default", text(stored)),
        ParameterValue::Formula(formula) => ("formula", text(formula)),
    };
    Tree::Members(vec![
        ("key", text(p.key.as_str())),
        ("definition", definition),
        value,
    ])
}

fn plane(p: &Plane) -> Tree {
    Tree::Members(vec![
        ("key", text(p.key.as_str())),
        ("base", text(p.base.name())),
        ("offset", text(&p.offset)),
    ])
}

fn sketch(s: &Sketch) -> Tree {
    let points = s
        .points
        .iter()
        .map(|p| {
            Tree::Members(vec![
                ("key", text(p.key.as_str())),
                ("x", text(&p.x)),
                ("y", text(&p.y)),
            ])
        })
        .collect();
    let lines = s
        .lines
        .iter()
        .map(|l| {
            Tree::Members(vec![
                ("key", text(l.key.as_str())),
                ("start", text(l.start.as_str())),
                ("end", text(l.end.as_str())),
            ])
        })
        .collect();
    let arcs = s
        .arcs
        .iter()
        .map(|a| {
            Tree::Members(vec![
                ("key", text(a.key.as_str())),
                ("centre", text(a.centre.as_str())),
                ("start", text(a.start.as_str())),
                ("end", text(a.end.as_str())),
                ("radius", text(&a.radius)),
            ])
        })
        .collect();
    let circles = s
        .circles
        .iter()
        .map(|c| {
            Tree::Members(vec![
                ("key", text(c.key.as_str())),
                ("centre", text(c.centre.as_str())),
                ("radius", text(&c.radius)),
            ])
        })
        .collect();
    Tree::Members(vec![
        ("key", text(s.key.as_str())),
        ("plane", text(s.plane.as_str())),
        ("points", Tree::List(points)),
        ("lines", Tree::List(lines)),
        ("arcs", Tree::List(arcs)),
        ("circles", Tree::List(circles)),
        (
            "constraints",
            Tree::List(s.constraints.iter().map(constraint).collect()),
        ),
    ])
}

fn side(side: Side) -> Tree {
    text(match side {
        Side::Left => "left",
        Side::Right => "right",
    })
}

fn constraint(c: &Constraint) -> Tree {
    let k = |key: &crate::model::Key| text(key.as_str());
    let members = match c {
        Constraint::Coincident { a, b } => {
            vec![("kind", text("coincident")), ("a", k(a)), ("b", k(b))]
        }
        Constraint::Horizontal { line } => vec![("kind", text("horizontal")), ("line", k(line))],
        Constraint::Vertical { line } => vec![("kind", text("vertical")), ("line", k(line))],
        Constraint::Parallel { a, b } => vec![("kind", text("parallel")), ("a", k(a)), ("b", k(b))],
        Constraint::Perpendicular { a, b } => {
            vec![("kind", text("perpendicular")), ("a", k(a)), ("b", k(b))]
        }
        Constraint::EqualLength { a, b } => {
            vec![("kind", text("equal_length")), ("a", k(a)), ("b", k(b))]
        }
        Constraint::Distance { a, b, length } => vec![
            ("kind", text("distance")),
            ("a", k(a)),
            ("b", k(b)),
            ("length", text(length)),
        ],
        Constraint::PointLineDistance {
            point,
            line,
            length,
            side: s,
        } => vec![
            ("kind", text("point_line_distance")),
            ("point", k(point)),
            ("line", k(line)),
            ("length", text(length)),
            ("side", side(*s)),
        ],
        Constraint::Fixed { point } => vec![("kind", text("fixed")), ("point", k(point))],
        Constraint::PointOnLine { point, line } => vec![
            ("kind", text("point_on_line")),
            ("point", k(point)),
            ("line", k(line)),
        ],
        Constraint::PointOnCurve { point, curve } => vec![
            ("kind", text("point_on_curve")),
            ("point", k(point)),
            ("curve", k(curve)),
        ],
        Constraint::Concentric { a, b } => {
            vec![("kind", text("concentric")), ("a", k(a)), ("b", k(b))]
        }
        Constraint::EqualRadius { a, b } => {
            vec![("kind", text("equal_radius")), ("a", k(a)), ("b", k(b))]
        }
        Constraint::Radius { curve, length } => vec![
            ("kind", text("radius")),
            ("curve", k(curve)),
            ("length", text(length)),
        ],
        Constraint::Diameter { curve, length } => vec![
            ("kind", text("diameter")),
            ("curve", k(curve)),
            ("length", text(length)),
        ],
        Constraint::TangentLine {
            line,
            curve,
            side: s,
        } => vec![
            ("kind", text("tangent_line")),
            ("line", k(line)),
            ("curve", k(curve)),
            ("side", side(*s)),
        ],
        Constraint::TangentCurves { a, b, touch } => vec![
            ("kind", text("tangent_curves")),
            ("a", k(a)),
            ("b", k(b)),
            (
                "touch",
                text(match touch {
                    Touch::Outside => "outside",
                    Touch::FirstInside => "first_inside",
                    Touch::SecondInside => "second_inside",
                }),
            ),
        ],
    };
    Tree::Members(members)
}

fn visibility(v: Visibility) -> Tree {
    Tree::Members(vec![
        ("coarse", Tree::Bool(v.coarse)),
        ("medium", Tree::Bool(v.medium)),
        ("fine", Tree::Bool(v.fine)),
    ])
}

fn material(m: &MaterialRef) -> Tree {
    Tree::Members(vec![match m {
        MaterialRef::Parameter(key) => ("parameter", text(key.as_str())),
        MaterialRef::Fixed(id) => ("fixed", text(id.as_str())),
    }])
}

fn step(s: &Step) -> Tree {
    let mut members = vec![("key", text(s.key.as_str()))];
    match &s.operation {
        Operation::Extrude {
            sketch,
            profile,
            holes,
            start,
            end,
        } => {
            members.push(("op", text("extrude")));
            members.push(("sketch", text(sketch.as_str())));
            members.push(("profile", keys(profile)));
            members.push((
                "holes",
                Tree::List(holes.iter().map(|h| keys(h.as_slice())).collect()),
            ));
            members.push(("start", text(start)));
            members.push(("end", text(end)));
        }
        Operation::Subtract { target, tool } => {
            members.push(("op", text("subtract")));
            members.push(("target", text(target.as_str())));
            members.push(("tool", text(tool.as_str())));
        }
        Operation::Unite { a, b } => {
            members.push(("op", text("unite")));
            members.push(("a", text(a.as_str())));
            members.push(("b", text(b.as_str())));
        }
    }
    members.push(("subcategory", Tree::Whole(u64::from(s.subcategory.0))));
    members.push(("material", material(&s.material)));
    members.push(("visibility", visibility(s.visibility)));
    Tree::Members(members)
}

fn keys(keys: &[crate::model::Key]) -> Tree {
    let names: Vec<&str> = keys.iter().map(crate::model::Key::as_str).collect();
    texts(&names)
}

fn symbol(s: &Symbol) -> Tree {
    let mut members = vec![
        ("key", text(s.key.as_str())),
        (
            "view",
            text(match s.view {
                View::Plan => "plan",
                View::Elevation => "elevation",
            }),
        ),
        ("sketch", text(s.sketch.as_str())),
        ("edges", keys(&s.edges)),
        ("subcategory", Tree::Whole(u64::from(s.subcategory.0))),
        ("visibility", visibility(s.visibility)),
    ];
    if let Some(when) = &s.when {
        members.push((
            "when",
            Tree::Members(vec![
                ("parameter", text(when.parameter.as_str())),
                ("is", text(&when.is)),
            ]),
        ));
    }
    Tree::Members(members)
}

fn hosting(h: &Hosting) -> Tree {
    match h {
        Hosting::Unhosted => Tree::Members(vec![("host", text("unhosted"))]),
        Hosting::Floor => Tree::Members(vec![("host", text("floor"))]),
        Hosting::Face => Tree::Members(vec![("host", text("face"))]),
        Hosting::Wall { opening } => Tree::Members(vec![
            ("host", text("wall")),
            (
                "opening",
                Tree::Members(vec![
                    ("sketch", text(opening.sketch.as_str())),
                    ("profile", keys(&opening.profile)),
                ]),
            ),
        ]),
    }
}

fn built_in_type(t: &BuiltInType) -> Tree {
    Tree::Members(vec![
        ("key", text(t.key.as_str())),
        ("name", text(&t.name)),
        (
            "values",
            Tree::List(
                t.values
                    .iter()
                    .map(|v| {
                        Tree::Members(vec![
                            ("parameter", text(v.parameter.as_str())),
                            ("value", text(&v.value)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn type_table(t: &TypeTable) -> Tree {
    Tree::Members(vec![
        ("columns", keys(&t.columns)),
        (
            "rows",
            Tree::List(
                t.rows
                    .iter()
                    .map(|row| {
                        Tree::Members(vec![
                            ("key", text(row.key.as_str())),
                            ("name", text(&row.name)),
                            ("values", texts(&row.values)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

/// The name of one version of a recipe: `sha256:` and the SHA-256 of its canonical bytes in
/// lowercase hexadecimal. Two recipes with one content id are the same recipe, byte for byte.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentId(String);

/// How a content id starts.
const SHA256: &str = "sha256:";

impl ContentId {
    /// The content id of these canonical bytes.
    pub(crate) fn of(bytes: &[u8]) -> Self {
        Self(format!("{SHA256}{}", hex::encode(Sha256::digest(bytes))))
    }

    /// A content id from its text, if the text is one: `sha256:` and 64 lowercase hexadecimal
    /// digits.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix(SHA256)?;
        let hex = |c: char| c.is_ascii_digit() || ('a'..='f').contains(&c);
        (digits.len() == 64 && digits.chars().all(hex)).then(|| Self(text.to_owned()))
    }

    /// The content id as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
