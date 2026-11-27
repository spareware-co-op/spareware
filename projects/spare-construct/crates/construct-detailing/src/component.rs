//! Detail components: 2D content that belongs to one view, made from a recipe of lines and fills.
//!
//! A **component recipe** is a small JSON document of its own kind (FORMAT.md beside this crate,
//! `schema/detail-component-1.schema.json`): a key, a name, straight lines each with a built-in
//! line style, and fills each a closed outline with a fill pattern and a colour, all in the
//! component's own frame (its origin at its position, its x along its turn). It has no 3D, no
//! parameters, no host and no type table: that is what makes it view-only, and why it is not a
//! family recipe. The library holds our own designs ([`Library::built_in`]): a timber section, a
//! blocking cross and an insulation batt.
//!
//! Lengths are typed text read by construct-params (`45 mm`, `-22.5 mm`, `0.045 m`) and written back as the exact millimetres
//! `Length::to_string` gives, so [`ComponentRecipe::to_json`] is the one canonical written form
//! and reading what it wrote gives the same recipe.
//!
//! [`place`] puts a component at a position turned by any angle (the one rule of the modify tools:
//! exact, each point rounded once to the grid), giving the lines and fills in the view's steps.

use crate::error::DetailError;
use crate::region::check_outline;
use crate::style::{BuiltIn, colour_of, colour_spelled};
use construct_grids::Point;
use construct_modify::{Motion, Turn};
use construct_params::{Definition, Holder, Kind, Length, Units, Value};
use construct_styles::Colour;
use kit_core::Id;
use serde::{Deserialize, Serialize};

/// The version of the recipe format this crate reads and writes.
pub const FORMAT_VERSION: u32 = 1;
/// The published schema of a component recipe.
pub const SCHEMA: &str = include_str!("../schema/detail-component-1.schema.json");
/// The farthest a component's frame extends from its origin, in grid steps: 10 m.
pub const REACH_STEPS: i64 = 64_000_000;
/// The most lines or fills one component holds.
pub const MOST_PARTS: usize = 64;

/// A line of a component, in its frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentLine {
    /// Where it starts.
    pub from: [Length; 2],
    /// Where it ends.
    pub to: [Length; 2],
    /// Its built-in line style.
    pub style: BuiltIn,
}

/// A fill of a component, in its frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentFill {
    /// Its corners, a simple closed loop.
    pub outline: Vec<[Length; 2]>,
    /// A fill pattern, by name.
    pub pattern: String,
    /// The colour of the pattern's lines.
    pub colour: Colour,
}

/// A checked component recipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentRecipe {
    key: String,
    name: String,
    lines: Vec<ComponentLine>,
    fills: Vec<ComponentFill>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Doc {
    schema: u32,
    key: String,
    name: String,
    lines: Vec<DocLine>,
    fills: Vec<DocFill>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocLine {
    from: [String; 2],
    to: [String; 2],
    style: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocFill {
    outline: Vec<[String; 2]>,
    pattern: String,
    colour: String,
}

fn bad(why: impl Into<String>) -> DetailError {
    DetailError::BadRecipe(why.into())
}

fn length_of(text: &str, at: &str) -> Result<Length, DetailError> {
    let id = Id::parse("detail-length").map_err(|e| bad(e.to_string()))?;
    let definition = Definition::new(id, "Length", Kind::Length, Holder::Instance)
        .map_err(|e| bad(e.to_string()))?;
    match definition.parse(text, &Units::metric()) {
        Ok(Value::Length(length)) => Ok(length),
        Ok(_) => Err(bad(format!("{at}: {text:?} is not a length"))),
        Err(e) => Err(bad(format!("{at}: {e}"))),
    }
}

fn pair(texts: &[String; 2], at: &str) -> Result<[Length; 2], DetailError> {
    let [x, y] = texts;
    Ok([length_of(x, at)?, length_of(y, at)?])
}

fn point_text(p: [Length; 2]) -> [String; 2] {
    [p[0].to_string(), p[1].to_string()]
}

fn key_ok(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn reaches(p: [Length; 2]) -> bool {
    p.iter()
        .all(|l| l.steps().unsigned_abs() <= REACH_STEPS.unsigned_abs())
}

impl ComponentRecipe {
    /// A recipe, checked.
    ///
    /// # Errors
    /// [`DetailError::BadRecipe`] for a key that is not lower-case letters, digits and hyphens
    /// (1 to 64), a name that is empty or over 64 characters, no lines and no fills, more than
    /// [`MOST_PARTS`] of either, a line whose ends are one point, a fill whose outline is not a
    /// simple closed loop (the region rules), or a point further than 10 m from the origin.
    pub fn new(
        key: &str,
        name: &str,
        lines: Vec<ComponentLine>,
        fills: Vec<ComponentFill>,
    ) -> Result<Self, DetailError> {
        if !key_ok(key) {
            return Err(bad(format!(
                "{key:?} is not a key: lower-case letters, digits and hyphens, 1 to 64."
            )));
        }
        if name.trim().is_empty() || name.chars().count() > 64 || name != name.trim() {
            return Err(bad(
                "A component's name is 1 to 64 characters, with no space at either end.",
            ));
        }
        if lines.is_empty() && fills.is_empty() {
            return Err(bad("A component needs at least a line or a fill."));
        }
        if lines.len() > MOST_PARTS || fills.len() > MOST_PARTS {
            return Err(bad(format!(
                "A component holds at most {MOST_PARTS} lines and {MOST_PARTS} fills."
            )));
        }
        for (n, line) in lines.iter().enumerate() {
            if line.from == line.to {
                return Err(bad(format!(
                    "Line {} has two ends that are one point.",
                    n.saturating_add(1)
                )));
            }
            if !reaches(line.from) || !reaches(line.to) {
                return Err(bad(format!(
                    "Line {} reaches further than 10 m from the origin.",
                    n.saturating_add(1)
                )));
            }
        }
        for (n, fill) in fills.iter().enumerate() {
            let corners: Vec<Point> = fill
                .outline
                .iter()
                .map(|p| Point::new(p[0], p[1]))
                .collect();
            check_outline(&corners)
                .map_err(|e| bad(format!("Fill {}: {e}", n.saturating_add(1))))?;
            if !fill.outline.iter().copied().all(reaches) {
                return Err(bad(format!(
                    "Fill {} reaches further than 10 m from the origin.",
                    n.saturating_add(1)
                )));
            }
        }
        Ok(Self {
            key: key.to_owned(),
            name: name.to_owned(),
            lines,
            fills,
        })
    }

    /// Its key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Its name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its lines.
    #[must_use]
    pub fn lines(&self) -> &[ComponentLine] {
        &self.lines
    }

    /// Its fills.
    #[must_use]
    pub fn fills(&self) -> &[ComponentFill] {
        &self.fills
    }

    /// The recipe as JSON, in its one canonical written form.
    ///
    /// # Errors
    /// [`DetailError::BadRecipe`] if the writer fails, which it does not for these types.
    pub fn to_json(&self) -> Result<String, DetailError> {
        let doc = Doc {
            schema: FORMAT_VERSION,
            key: self.key.clone(),
            name: self.name.clone(),
            lines: self
                .lines
                .iter()
                .map(|l| DocLine {
                    from: point_text(l.from),
                    to: point_text(l.to),
                    style: l.style.key().to_owned(),
                })
                .collect(),
            fills: self
                .fills
                .iter()
                .map(|f| DocFill {
                    outline: f.outline.iter().copied().map(point_text).collect(),
                    pattern: f.pattern.clone(),
                    colour: colour_spelled(f.colour),
                })
                .collect(),
        };
        serde_json::to_string_pretty(&doc).map_err(|e| bad(e.to_string()))
    }

    /// Reads a recipe: exactly what the schema takes, then every rule of [`ComponentRecipe::new`];
    /// a document that is not in its canonical written form is refused.
    ///
    /// # Errors
    /// [`DetailError::BadRecipe`], naming the place.
    pub fn from_json(text: &str) -> Result<Self, DetailError> {
        let doc: Doc =
            serde_json::from_str(text).map_err(|e| bad(format!("Not a component recipe: {e}")))?;
        if doc.schema != FORMAT_VERSION {
            return Err(bad(format!(
                "The recipe is format {}: this reads format {FORMAT_VERSION}.",
                doc.schema
            )));
        }
        let mut lines = Vec::with_capacity(doc.lines.len());
        for (n, line) in doc.lines.iter().enumerate() {
            let at = format!("/lines/{n}");
            lines.push(ComponentLine {
                from: pair(&line.from, &format!("{at}/from"))?,
                to: pair(&line.to, &format!("{at}/to"))?,
                style: BuiltIn::from_key(&line.style).ok_or_else(|| {
                    bad(format!(
                        "{at}/style: {:?} is not a built-in style",
                        line.style
                    ))
                })?,
            });
        }
        let mut fills = Vec::with_capacity(doc.fills.len());
        for (n, fill) in doc.fills.iter().enumerate() {
            let at = format!("/fills/{n}");
            let mut outline = Vec::with_capacity(fill.outline.len());
            for (c, corner) in fill.outline.iter().enumerate() {
                outline.push(pair(corner, &format!("{at}/outline/{c}"))?);
            }
            fills.push(ComponentFill {
                outline,
                pattern: fill.pattern.clone(),
                colour: colour_of(&fill.colour).map_err(|e| bad(format!("{at}/colour: {e}")))?,
            });
        }
        Self::new(&doc.key, &doc.name, lines, fills)
    }
}

/// The components a project can place, by key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
    recipes: Vec<ComponentRecipe>,
}

const BUILT_IN: [&str; 3] = [
    include_str!("../../../library/detail-components/timber-section-90x45.json"),
    include_str!("../../../library/detail-components/blocking-cross-90x45.json"),
    include_str!("../../../library/detail-components/insulation-batt-90x600.json"),
];

impl Library {
    /// The library of `recipes`, in order of key.
    ///
    /// # Errors
    /// [`DetailError::BadRecipe`] for two recipes of one key.
    pub fn new(mut recipes: Vec<ComponentRecipe>) -> Result<Self, DetailError> {
        recipes.sort_by(|a, b| a.key.cmp(&b.key));
        for pair in recipes.windows(2) {
            if let [a, b] = pair
                && a.key == b.key
            {
                return Err(bad(format!("Two components have the key {}.", a.key)));
            }
        }
        Ok(Self { recipes })
    }

    /// Our own designs: a timber section, a blocking cross and an insulation batt.
    ///
    /// # Errors
    /// [`DetailError::BadRecipe`] if one of the library's files is not a recipe, which a test holds
    /// it never is.
    pub fn built_in() -> Result<Self, DetailError> {
        Self::new(
            BUILT_IN
                .iter()
                .map(|text| ComponentRecipe::from_json(text))
                .collect::<Result<Vec<_>, _>>()?,
        )
    }

    /// The recipe with this key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&ComponentRecipe> {
        self.recipes.iter().find(|r| r.key == key)
    }

    /// Every recipe, in order of key.
    #[must_use]
    pub fn all(&self) -> &[ComponentRecipe] {
        &self.recipes
    }
}

/// A line of a placed component, in the view's grid steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedLine {
    /// Where it starts.
    pub from: [i64; 2],
    /// Where it ends.
    pub to: [i64; 2],
    /// Its built-in line style.
    pub style: BuiltIn,
}

/// A fill of a placed component, in the view's grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedFill {
    /// Its corners.
    pub outline: Vec<[i64; 2]>,
    /// The fill pattern's name.
    pub pattern: String,
    /// The colour of its lines.
    pub colour: Colour,
}

/// What a placed component draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    /// Its lines.
    pub lines: Vec<PlacedLine>,
    /// Its fills.
    pub fills: Vec<PlacedFill>,
}

fn grid_error() -> DetailError {
    DetailError::OutOfRange("The component would reach past the kernel's grid.".to_owned())
}

/// `recipe` placed with its origin at `position`, turned by `turn` about it. Every point is the
/// exact turn of the component's own point, rounded once to the nearest grid step (the modify
/// tools' rule), so a quarter turn and an unturned component are exact.
///
/// # Errors
/// [`DetailError::OutOfRange`] when a point would pass the kernel's grid.
pub fn place(
    recipe: &ComponentRecipe,
    position: Point,
    turn: &Turn,
) -> Result<Placed, DetailError> {
    let origin = [position.x.steps(), position.y.steps()];
    let motion = Motion::turn_about(origin, turn);
    let put = |p: [Length; 2]| -> Result<[i64; 2], DetailError> {
        let moved = [
            origin[0].checked_add(p[0].steps()).ok_or_else(grid_error)?,
            origin[1].checked_add(p[1].steps()).ok_or_else(grid_error)?,
        ];
        motion.apply(moved).ok_or_else(grid_error)
    };
    let lines = recipe
        .lines
        .iter()
        .map(|l| {
            Ok(PlacedLine {
                from: put(l.from)?,
                to: put(l.to)?,
                style: l.style,
            })
        })
        .collect::<Result<Vec<_>, DetailError>>()?;
    let fills = recipe
        .fills
        .iter()
        .map(|f| {
            Ok(PlacedFill {
                outline: f
                    .outline
                    .iter()
                    .map(|p| put(*p))
                    .collect::<Result<Vec<_>, DetailError>>()?,
                pattern: f.pattern.clone(),
                colour: f.colour,
            })
        })
        .collect::<Result<Vec<_>, DetailError>>()?;
    Ok(Placed { lines, fills })
}
