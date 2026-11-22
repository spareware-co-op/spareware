//! The commands that make, rename, move, reshape and delete grid lines and reference planes
//! (CON-A4-F1-P2, P3). They go through the same edit log as `element.*`, with the same access
//! check (`model.edit`) and the same claims: a piece another person is working on is refused, so
//! two people never write one grid at once, and one person's edits land in log order. Each
//! command is validated on the server with [`construct_grids`]' own functions, and writes the one
//! record that changed (the record's `props` other than its category, a parameter value or a
//! type, are kept as they were). A refusal is the command's result, in the data crate's own
//! words, and changes nothing. Nothing here reads a clock or a random number: the claim's time is
//! the orderer's, recorded in the entry; the name a new grid is given when none is asked for is
//! construct-grids' rule, from the document's records.
//!
//! | Command | Arguments | What it does |
//! | --- | --- | --- |
//! | `grid.create` | `grid`; `name` (or null: the next name by the rule); `after` (a name to carry on from); a line: `x0`, `y0`, `x1`, `y1`; an arc: `cx`, `cy`, `radius`, `start`, `end` | A new grid line |
//! | `grid.rename` | `grid`, `name` | Renames it |
//! | `grid.move` | `grid`, `dx`, `dy` | Moves it by that much, as it is now |
//! | `grid.reshape` | `grid`, the same geometry arguments as `grid.create` for its shape | Gives it that geometry: a line stays a line and an arc an arc |
//! | `grid.delete` | `grid` | Deletes it |
//! | `plane.create` | `plane`; `name` (or null); a vertical plane: `x0`, `y0`, `x1`, `y1`; a horizontal one: `elevation` | A new reference plane |
//! | `plane.rename` | `plane`, `name` (null takes its name away) | Renames it |
//! | `plane.move` | `plane`; `dx`, `dy` (vertical) or `dz` (horizontal) | Moves it, as it is now |
//! | `plane.delete` | `plane` | Deletes it |
//!
//! Lengths and angles are typed text, read with construct-params' metric units
//! ([`construct_params::Definition::parse`]): `6000`, `6 m`, `12.5 mm` for a length, `90` for
//! degrees. Anything off the kernel's grid is refused, naming the nearest values. A grid is
//! vertical and has no elevation: there is no argument for one. Which levels it shows on is not
//! stored yet (see construct-grids).
//!
//! The `element.*` commands refuse a grid or a reference plane, and a recipe of theirs
//! ([`is_datum`]), so that each stays one the rules above have checked; undo and redo cover each
//! command as an ordinary step ([`crate::cloud::undo`]) and re-check the names ([`check`]).

use crate::cloud::elements::write_record;
use construct_grids::{
    Datums, Grid, GridError, PlaneShape, ReferencePlane, Shape, datum_of_recipe, is_datum_kind,
};
use construct_model::element::Element;
use construct_model::geometry::Recipe;
use construct_params::{Angle, Definition, Holder, Kind, Length, Units, Value};
use construct_regen::parameters as records;
use kit_core::Id;
use kit_ops::{Change, Claim, Edit};
use kit_store::{Place, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::Deserialize;

/// Every command here.
pub const COMMANDS: [&str; 9] = [
    "grid.create",
    "grid.rename",
    "grid.move",
    "grid.reshape",
    "grid.delete",
    "plane.create",
    "plane.rename",
    "plane.move",
    "plane.delete",
];

/// Why `element.*` refuses a grid or a reference plane.
pub const BY_COMMAND: &str =
    "Grids and reference planes are changed by their own commands, not as elements.";

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// Whether a record of this kind and recipe is a grid line or a reference plane.
#[must_use]
pub fn is_datum(kind: &str, recipe: &Recipe) -> bool {
    is_datum_kind(kind) || datum_of_recipe(recipe).is_some()
}

/// The arguments of every command here; each uses some of them. Lengths and angles are text.
#[derive(Deserialize, Default)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    grid: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    plane: Option<Id>,
    name: Option<String>,
    after: Option<String>,
    x0: Option<String>,
    y0: Option<String>,
    x1: Option<String>,
    y1: Option<String>,
    cx: Option<String>,
    cy: Option<String>,
    radius: Option<String>,
    start: Option<String>,
    end: Option<String>,
    dx: Option<String>,
    dy: Option<String>,
    dz: Option<String>,
    elevation: Option<String>,
}

/// The part of a person's row a claim needs.
#[derive(Deserialize)]
struct Named {
    name: String,
}

/// What a command is run against: the store, where, who, and when the orderer says it is.
struct Run<'a, T: Txn> {
    txn: &'a mut T,
    project: &'a Id,
    document: &'a Id,
    edit: &'a Edit,
    now_ms: u64,
    claim_ms: u64,
}

/// A command's answer: a store failure, or the command's own result.
type Answer = Result<Outcome, StoreError>;

fn why(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn need<T>(value: Option<T>, what: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("This command needs {what}."))
}

/// A parameter definition to read typed text with: any ID will do, the name is what a refusal says.
fn definition(name: &str, kind: Kind) -> Result<Definition, String> {
    let id = Id::parse("datum").map_err(why)?;
    Definition::new(id, name, kind, Holder::Instance).map_err(why)
}

/// Typed text as a length, with the metric units.
fn length(text: Option<&str>, name: &str) -> Result<Length, String> {
    let text = need(text, &name.to_lowercase())?;
    match definition(name, Kind::Length)?
        .parse(text, &Units::metric())
        .map_err(why)?
    {
        Value::Length(length) => Ok(length),
        _ => Err(format!("{name} is a length.")),
    }
}

/// Typed text as an angle, in degrees.
fn angle(text: Option<&str>, name: &str) -> Result<Angle, String> {
    let text = need(text, &name.to_lowercase())?;
    match definition(name, Kind::Angle)?
        .parse(text, &Units::metric())
        .map_err(why)?
    {
        Value::Angle(angle) => Ok(angle),
        _ => Err(format!("{name} is an angle.")),
    }
}

fn point(x: Option<&str>, y: Option<&str>, which: &str) -> Result<construct_grids::Point, String> {
    Ok(construct_grids::Point::new(
        length(x, &format!("{which} x"))?,
        length(y, &format!("{which} y"))?,
    ))
}

/// Whether any of these arguments is given.
fn given(fields: &[&Option<String>]) -> bool {
    fields.iter().any(|field| field.is_some())
}

impl Args {
    /// A grid's shape, from a line's four arguments or an arc's five.
    fn shape(&self) -> Result<Shape, String> {
        let arc = given(&[&self.cx, &self.cy, &self.radius, &self.start, &self.end]);
        let line = given(&[&self.x0, &self.y0, &self.x1, &self.y1]);
        match (line, arc) {
            (true, false) => {
                let from = point(self.x0.as_deref(), self.y0.as_deref(), "Start")?;
                let to = point(self.x1.as_deref(), self.y1.as_deref(), "End")?;
                Shape::line(from, to).map_err(why)
            }
            (false, true) => {
                let centre = point(self.cx.as_deref(), self.cy.as_deref(), "Centre")?;
                let radius = length(self.radius.as_deref(), "Radius")?;
                let from = angle(self.start.as_deref(), "Start angle")?;
                let to = angle(self.end.as_deref(), "End angle")?;
                Shape::arc(centre, radius, from, to).map_err(why)
            }
            (false, false) => Err(
                "A grid needs its geometry: x0, y0, x1 and y1 for a line, or cx, cy, radius, start and end for an arc."
                    .to_owned(),
            ),
            (true, true) => Err("A grid is a line or an arc, not both.".to_owned()),
        }
    }

    /// A reference plane's shape: a vertical one from its four, a horizontal one from its
    /// elevation.
    fn plane_shape(&self) -> Result<PlaneShape, String> {
        let line = given(&[&self.x0, &self.y0, &self.x1, &self.y1]);
        match (line, self.elevation.as_deref()) {
            (true, None) => {
                let from = point(self.x0.as_deref(), self.y0.as_deref(), "Start")?;
                let to = point(self.x1.as_deref(), self.y1.as_deref(), "End")?;
                let shape = PlaneShape::Vertical { from, to };
                shape.check().map_err(why)?;
                Ok(shape)
            }
            (false, Some(text)) => {
                let elevation = length(Some(text), "Elevation")?;
                let shape = PlaneShape::Horizontal { elevation };
                shape.check().map_err(why)?;
                Ok(shape)
            }
            (false, None) => Err(
                "A reference plane needs its geometry: x0, y0, x1 and y1 for a vertical one, or an elevation for a horizontal one."
                    .to_owned(),
            ),
            (true, Some(_)) => Err(
                "A reference plane is vertical or horizontal, not both.".to_owned(),
            ),
        }
    }
}

impl<T: Txn> Run<'_, T> {
    /// The claim this person takes on a piece, or who else holds it.
    async fn claim(&mut self, piece: &Id) -> Result<Result<Claim, String>, StoreError> {
        let holder = claims::held_by_another(
            self.txn,
            self.project,
            self.document,
            piece,
            &self.edit.person,
            self.now_ms,
        )
        .await?;
        if let Some(claim) = holder {
            return Ok(Err(format!("{} is working on this.", claim.name)));
        }
        let person: Option<Named> =
            read_row(self.txn, &Place::Person(self.edit.person.clone()).key()).await?;
        let name = person.map_or_else(|| self.edit.person.to_string(), |person| person.name);
        Ok(Ok(Claim {
            person: self.edit.person.clone(),
            name,
            // milliseconds since 1970 plus a claim's length: far below u64::MAX
            until_ms: self.now_ms.saturating_add(self.claim_ms),
        }))
    }

    /// Writes one record whose piece this person claims, as one command's whole result.
    async fn put_claimed(&mut self, record: &Element, before: Option<Id>) -> Answer {
        let claim = match self.claim(&record.id).await? {
            Ok(claim) => claim,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let hash = write_record(self.txn, self.project, record).await?;
        let pointer = Place::Piece {
            project: self.project.clone(),
            document: self.document.clone(),
            piece: record.id.clone(),
        };
        self.txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
        let claim = claims::take(self.txn, self.project, self.document, &record.id, claim).await?;
        Ok(Ok(Applied {
            changes: vec![Change {
                piece: record.id.clone(),
                before,
                after: Some(hash),
            }],
            claims: vec![claim],
            ..Applied::default()
        }))
    }

    /// Deletes one piece this person claims: its pointer goes and its claim is let go.
    async fn delete_claimed(&mut self, id: &Id, hash: Id) -> Answer {
        if let Err(refusal) = self.claim(id).await? {
            return Ok(Err(refusal));
        }
        let pointer = Place::Piece {
            project: self.project.clone(),
            document: self.document.clone(),
            piece: id.clone(),
        };
        self.txn.delete(pointer.key()).await?;
        let released = claims::release(self.txn, self.project, self.document, id).await?;
        Ok(Ok(Applied {
            changes: vec![Change {
                piece: id.clone(),
                before: Some(hash),
                after: None,
            }],
            claims: vec![released],
            ..Applied::default()
        }))
    }

    /// The document's grids and reference planes, and its records of them by ID.
    async fn datums(&mut self) -> Result<Result<(Datums, Vec<(Id, Element)>), String>, StoreError> {
        let all = records::read_document(self.txn, self.project, self.document).await?;
        let found: Vec<(Id, Element)> = all
            .into_values()
            .filter(|(_, record)| is_datum_kind(&record.kind))
            .collect();
        Ok(Datums::from_records(found.iter().map(|(_, record)| record))
            .map(|datums| (datums, found))
            .map_err(why))
    }
}

/// The record of a piece in a document's datum records, and its hash.
fn record_of(found: &[(Id, Element)], id: &Id) -> Option<(Id, Element)> {
    found
        .iter()
        .find(|(_, record)| &record.id == id)
        .map(|(hash, record)| (hash.clone(), record.clone()))
}

/// Runs one of these commands.
///
/// # Errors
///
/// A store error; a command that cannot be made is the outcome's error instead.
pub async fn apply<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Ordered<'_>,
    edit: &Edit,
    claim_ms: u64,
) -> Result<Outcome, StoreError> {
    let args: Args = match serde_json::from_value(edit.args.clone()) {
        Ok(args) => args,
        Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
    };
    let mut run = Run {
        txn,
        project,
        document: &edit.target,
        edit,
        now_ms: context.now_ms,
        claim_ms,
    };
    let (datums, found) = match run.datums().await? {
        Ok(read) => read,
        Err(refusal) => return Ok(Err(refusal)),
    };
    // Whether the arguments say `name` at all: a plane's `name: null` takes its name away.
    let names_it = edit.args.get("name").is_some();
    match edit.cmd.as_str() {
        "grid.create" => create_grid(&mut run, datums, args).await,
        "plane.create" => create_plane(&mut run, datums, args).await,
        "grid.rename" | "grid.move" | "grid.reshape" | "grid.delete" => {
            change_grid(&mut run, datums, &found, args).await
        }
        "plane.rename" | "plane.move" | "plane.delete" => {
            change_plane(&mut run, datums, &found, args, names_it).await
        }
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// A new piece's ID must not be an element's already.
async fn free<T: Txn>(run: &mut Run<'_, T>, id: &Id) -> Result<Result<(), String>, StoreError> {
    if records::read_element(run.txn, run.project, run.document, id)
        .await?
        .is_some()
    {
        return Ok(Err("That element already exists.".to_owned()));
    }
    Ok(Ok(()))
}

async fn create_grid<T: Txn>(run: &mut Run<'_, T>, mut datums: Datums, args: Args) -> Answer {
    let made = (|| {
        let id = need(args.grid.clone(), "a grid")?;
        let shape = args.shape()?;
        let name = args
            .name
            .clone()
            .unwrap_or_else(|| datums.suggest_grid_name(args.after.as_deref()));
        let grid = Grid::new(id, &name, shape).map_err(why)?;
        datums.add_grid(grid.clone()).map_err(why)?;
        Ok::<_, String>(grid)
    })();
    let grid = match made {
        Ok(grid) => grid,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(refusal) = free(run, grid.id()).await? {
        return Ok(Err(refusal));
    }
    let record = grid.to_record(run.document);
    run.put_claimed(&record, None).await
}

async fn create_plane<T: Txn>(run: &mut Run<'_, T>, mut datums: Datums, args: Args) -> Answer {
    let made = (|| {
        let id = need(args.plane.clone(), "a reference plane")?;
        let shape = args.plane_shape()?;
        let plane = ReferencePlane::new(id, args.name.as_deref(), shape).map_err(why)?;
        datums.add_plane(plane.clone()).map_err(why)?;
        Ok::<_, String>(plane)
    })();
    let plane = match made {
        Ok(plane) => plane,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(refusal) = free(run, plane.id()).await? {
        return Ok(Err(refusal));
    }
    let record = plane.to_record(run.document);
    run.put_claimed(&record, None).await
}

/// `grid.rename`, `grid.move`, `grid.reshape` and `grid.delete`: each acts on one grid.
async fn change_grid<T: Txn>(
    run: &mut Run<'_, T>,
    mut datums: Datums,
    found: &[(Id, Element)],
    args: Args,
) -> Answer {
    let id = match need(args.grid.clone(), "a grid") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((hash, record)) = record_of(found, &id).filter(|(_, r)| r.kind == "grid") else {
        return Ok(Err(why(GridError::UnknownGrid(id))));
    };
    let changed = (|| {
        let grid = datums
            .grid(&id)
            .ok_or_else(|| why(GridError::UnknownGrid(id.clone())))?;
        match run.edit.cmd.as_str() {
            "grid.rename" => {
                let name = need(args.name.as_deref(), "a name")?;
                datums.rename_grid(&id, name).map_err(why)?;
            }
            "grid.move" => {
                let dx = length(args.dx.as_deref(), "dx")?;
                let dy = length(args.dy.as_deref(), "dy")?;
                let shape = grid.shape().moved(dx, dy).map_err(why)?;
                datums.reshape_grid(&id, shape).map_err(why)?;
            }
            "grid.reshape" => {
                let shape = args.shape()?;
                if std::mem::discriminant(&shape) != std::mem::discriminant(grid.shape()) {
                    return Err(
                        "A line stays a line and an arc an arc: delete it and draw the other."
                            .to_owned(),
                    );
                }
                datums.reshape_grid(&id, shape).map_err(why)?;
            }
            _ => {}
        }
        Ok::<_, String>(())
    })();
    if let Err(refusal) = changed {
        return Ok(Err(refusal));
    }
    if run.edit.cmd == "grid.delete" {
        return run.delete_claimed(&id, hash).await;
    }
    let Some(grid) = datums.grid(&id) else {
        return Ok(Err(why(GridError::UnknownGrid(id))));
    };
    let after = grid.update_record(&record);
    run.put_claimed(&after, Some(hash)).await
}

/// `plane.rename`, `plane.move` and `plane.delete`: each acts on one reference plane.
async fn change_plane<T: Txn>(
    run: &mut Run<'_, T>,
    mut datums: Datums,
    found: &[(Id, Element)],
    args: Args,
    names_it: bool,
) -> Answer {
    let id = match need(args.plane.clone(), "a reference plane") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((hash, record)) = record_of(found, &id).filter(|(_, r)| r.kind == "refplane") else {
        return Ok(Err(why(GridError::UnknownPlane(id))));
    };
    let changed = (|| {
        let plane = datums
            .plane(&id)
            .ok_or_else(|| why(GridError::UnknownPlane(id.clone())))?;
        match run.edit.cmd.as_str() {
            "plane.rename" => {
                if !names_it {
                    return Err("This command needs a name, or null to take it away.".to_owned());
                }
                datums
                    .rename_plane(&id, args.name.as_deref())
                    .map_err(why)?;
            }
            "plane.move" => {
                let zero = "0";
                let dx = length(args.dx.as_deref().or(Some(zero)), "dx")?;
                let dy = length(args.dy.as_deref().or(Some(zero)), "dy")?;
                let dz = length(args.dz.as_deref().or(Some(zero)), "dz")?;
                let shape = plane.shape().moved(dx, dy, dz).map_err(why)?;
                datums.reshape_plane(&id, shape).map_err(why)?;
            }
            _ => {}
        }
        Ok::<_, String>(())
    })();
    if let Err(refusal) = changed {
        return Ok(Err(refusal));
    }
    if run.edit.cmd == "plane.delete" {
        return run.delete_claimed(&id, hash).await;
    }
    let Some(plane) = datums.plane(&id) else {
        return Ok(Err(why(GridError::UnknownPlane(id))));
    };
    let after = plane.update_record(&record);
    run.put_claimed(&after, Some(hash)).await
}

/// After an undo or a redo wrote these records: if any is a grid or a reference plane, every
/// grid and plane of the document must read as written, and no two of a kind share a name. The
/// reason, in words, if not.
///
/// # Errors
///
/// A store error; a document that fails the rules is the inner error.
pub async fn check<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    written: &[Element],
) -> Result<Result<(), String>, StoreError> {
    if !written.iter().any(|record| is_datum_kind(&record.kind)) {
        return Ok(Ok(()));
    }
    let all = records::read_document(txn, project, document).await?;
    let read = Datums::from_records(all.values().map(|(_, record)| record))
        .and_then(|datums| datums.check_names());
    Ok(read.map_err(why))
}
