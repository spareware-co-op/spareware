//! How a curtain wall's recipe reads its grid and parts from its curtain wall type (job 14.1,
//! CON-A4-F6-P1, piece A84). The regenerator fills a curtain wall's recipe through this one module
//! before it is built, so a curtain wall is built from what construct-curtain says its type is.
//!
//! What it promises:
//! - A curtain wall's recipe argument written `{"type":"curtain"}` (construct-curtain's
//!   `type_arg`) is filled, before the shape is built, with the grid and parts of the element's
//!   curtain wall type (its `@system.curtain` block, construct-curtain's `spec_of`). A recipe that
//!   reads no type this way is returned as it is.
//! - A recipe that reads its type this way reads the document's parameter table and the type
//!   ([`reads_type`]): the regenerator lists both in its `reads` row, so a change to the type's
//!   grid or parts builds every curtain wall of it again, and a curtain wall changed to another
//!   type is built with the new type's. Its levels are read as a wall's are ([`crate::levels`]):
//!   its base at its level's elevation plus its offset, its height from its top tie when it has
//!   one.
//! - What a page shows of a curtain wall ([`read_only_view`]), after the parameters its category
//!   has: its length, height, area, panels and their area, glazed area, and mullions and their
//!   length (construct-curtain's `read_only`), worked out each time from its filled recipe, never
//!   stored, each shown in the document's units and marked `read-only`, as a floor's are. An
//!   embedded curtain wall's length is its own; its line is its host's.
//! - What a page is told of a curtain wall's grid ([`CurtainView`]): every line and every panel,
//!   each place as the exact typed text a command takes and in the document's units, worked out
//!   from the filled recipe each time and never stored.
//!
//! Depends on construct-model (the recipe), construct-types (the type), construct-curtain (the
//! grid and parts, the layout and the quantities) and construct-params (units). Nothing here is
//! interim.

use crate::parameters::ParameterView;
use crate::slabs::shown;
use construct_curtain::{
    Curtain, CurtainError, Filled, GridLine, Panel, PanelPlace, edits_of, embedded_length,
    height_of, is_curtain, is_embedded, quantities, read_only, spec_of,
};
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::{Definition, Holder, Kind, Length, Units, Value};
use construct_types::ElementType;
use kit_core::Id;
use serde::Serialize;

fn failure(code: &str, message: String) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message,
    }
}

/// Whether a recipe is a curtain wall's that reads its curtain wall type's grid and parts.
#[must_use]
pub fn reads_type(recipe: &Recipe) -> bool {
    construct_curtain::reads_type(recipe)
}

/// The recipe a curtain wall is built from: its grid and parts read from its curtain wall type
/// `ty`. A recipe that reads no type ([`reads_type`]) is returned as it is.
///
/// # Errors
/// A build failure, which is model state like any other: `NoCurtainType` for a curtain wall that
/// names no type, `BadCurtainType` for a type that is not a curtain wall type or whose grid and
/// parts do not read, each in construct-curtain's words.
pub fn filled(recipe: &Recipe, ty: Option<&ElementType>) -> Result<Recipe, GeometryError> {
    if !reads_type(recipe) {
        return Ok(recipe.clone());
    }
    let Some(ty) = ty else {
        return Err(failure(
            "NoCurtainType",
            "its grid and parts are read from its curtain wall type, and it names none".to_owned(),
        ));
    };
    let spec =
        spec_of(ty).map_err(|e| failure("BadCurtainType", format!("its type {}: {e}", ty.id())))?;
    construct_curtain::filled(recipe, &spec).map_err(|e| failure("BadCurtain", e.to_string()))
}

/// The curtain wall a filled recipe lays out: a free one's from its line, an embedded one's from
/// its own length, before it is seated on its host.
///
/// # Errors
/// construct-curtain's refusals of the recipe, in its words.
pub fn curtain_of(recipe: &Recipe) -> Result<Curtain, CurtainError> {
    if !is_embedded(recipe) {
        return Ok(Filled::of(recipe)?.curtain);
    }
    let text = recipe
        .args
        .get(construct_curtain::SPEC_ARG)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CurtainError::Malformed("its type's grid is not read in".to_owned()))?;
    Curtain::new(
        embedded_length(recipe)?,
        height_of(recipe)?,
        construct_curtain::CurtainSpec::from_text(text)?,
        edits_of(recipe)?,
    )
}

/// A curtain wall's read-only parameters as a page shows them, as the module header says, from its
/// filled recipe `recipe` (its type's grid read in, its top tie read). Not a curtain wall, or one
/// that does not lay out (its built row says why): none.
#[must_use]
pub fn read_only_view(recipe: &Recipe, units: &Units) -> Vec<ParameterView> {
    if !is_curtain(recipe) {
        return Vec::new();
    }
    let rows = (|| {
        let grid = curtain_of(recipe)?.grid()?;
        read_only(&quantities(&grid)?)
    })();
    rows.map_or_else(
        |_| Vec::new(),
        |rows| {
            rows.into_iter()
                .map(|(def, value)| shown(&def, value, units))
                .collect()
        },
    )
}

/// One segment of a grid line as a page is told of it: between two crossings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SegmentView {
    /// The crossing it starts at, as typed text a command takes (`curtain.segment.omit`'s `from`).
    pub from: String,
    /// The crossing it ends at, in the document's units.
    pub to: String,
    /// Whether the wall leaves it out.
    pub omitted: bool,
}

/// One grid line as a page is told of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LineView {
    /// `vertical` or `horizontal`.
    pub direction: &'static str,
    /// Where it stands, as typed text a command takes (`at`): exact on the length grid.
    pub at: String,
    /// Where it stands, in the document's units.
    pub shown: String,
    /// Whether it is one of the wall's borders (a border is never added or removed).
    pub border: bool,
    /// Whether the wall adds it, rather than its type laying it out.
    pub added: bool,
    /// Its segments, in order.
    pub segments: Vec<SegmentView>,
}

/// One panel as a page is told of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PanelView {
    /// The corner its cell starts at, along then up, as typed text a command takes (`at`).
    pub at: [String; 2],
    /// The same, in the document's units.
    pub shown: [String; 2],
    /// Its width, in the document's units.
    pub width: String,
    /// Its height, in the document's units.
    pub height: String,
    /// `glazed`, `solid`, `empty` or `door`.
    pub kind: &'static str,
    /// For a door, its family's content id and its type's key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub door: Option<[String; 2]>,
    /// Whether the wall swaps it in, rather than its type giving it.
    pub swapped: bool,
}

/// What a page is told of a curtain wall's grid: every line and every panel, where each stands,
/// worked out each time it is read from the filled recipe and never stored, so a page edits a grid
/// it was shown and never works one out. Every length a command takes comes both as the exact
/// typed text of the length grid and as the document's units for a person to read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CurtainView {
    /// Its vertical lines from its start, then its horizontal lines from its base, borders
    /// included.
    pub lines: Vec<LineView>,
    /// Its panels, by the corner their cell starts at (along, then up).
    pub panels: Vec<PanelView>,
    /// The edits that have no place in the grid, in words.
    pub unused: Vec<String>,
}

fn looks(units: &Units, length: Length) -> String {
    Id::parse("curtain.length")
        .ok()
        .and_then(|id| Definition::new(id, "Length", Kind::Length, Holder::Instance).ok())
        .and_then(|def| def.format(&Value::Length(length), units).ok())
        .unwrap_or_else(|| length.to_string())
}

fn line_view(line: &GridLine, units: &Units) -> LineView {
    LineView {
        direction: line.direction.word(),
        at: line.at.to_string(),
        shown: looks(units, line.at),
        border: line.border,
        added: line.added,
        segments: line
            .segments
            .iter()
            .map(|s| SegmentView {
                from: s.from.to_string(),
                to: looks(units, s.to),
                omitted: s.omitted,
            })
            .collect(),
    }
}

fn panel_view(place: &PanelPlace, units: &Units) -> PanelView {
    let [along, up] = place.at;
    PanelView {
        at: [along.to_string(), up.to_string()],
        shown: [looks(units, along), looks(units, up)],
        width: looks(units, place.width()),
        height: looks(units, place.height()),
        kind: place.panel.kind().word(),
        door: match &place.panel {
            Panel::Door(door) => Some([
                door.family().as_str().to_owned(),
                door.ty().as_str().to_owned(),
            ]),
            _ => None,
        },
        swapped: place.swapped,
    }
}

impl CurtainView {
    /// The view of a curtain wall from its filled recipe `recipe` (its type's grid read in, its top
    /// tie read), in `units`. Not a curtain wall, or one that does not lay out: none.
    #[must_use]
    pub fn of(recipe: &Recipe, units: &Units) -> Option<Self> {
        if !is_curtain(recipe) {
            return None;
        }
        let grid = curtain_of(recipe).ok()?.grid().ok()?;
        let lines = grid
            .vertical
            .iter()
            .chain(&grid.horizontal)
            .map(|line| line_view(line, units))
            .collect();
        Some(Self {
            lines,
            panels: grid.panels.iter().map(|p| panel_view(p, units)).collect(),
            unused: grid.unused,
        })
    }
}
