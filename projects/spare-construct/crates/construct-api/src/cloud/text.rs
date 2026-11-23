//! The commands that make and change a document's text styles, text notes and leaders
//! (CON-A9-F3-P1). They go through the same edit log as `element.*`, with the same access check
//! (`model.edit`) and the same claims: a piece another person is working on is refused, and one
//! person's edits land in log order.
//!
//! A text style and a text note are element records of the text notes category, neither with a
//! shape ([`construct_text`]). Each command reads the document's styles, notes and views, makes the
//! change with construct-text's own types, checks it with the rules of [`construct_text::Texts`]
//! and writes the records it changed. A refusal is the command's result, in construct-text's
//! words, and changes nothing. Nothing here reads a clock or a random number.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `text.style.create` | `style` (the new ID), `name`, `font`, `size`; optional `bold`, `italic`, `underline`, `colour`, `spacing` | A style record |
//! | `text.style.set` | `style`, and any of `name`, `font`, `size`, `bold`, `italic`, `underline`, `colour`, `spacing` | The style; every note that reads it reads the change; nothing at all when it already is so |
//! | `text.style.delete` | `style` | Its pointer goes; refused while notes use it |
//! | `text.note.create` | `note` (the new ID), `view`, `style`, `text`, `x`, `y`; optional `width`, `align`, `list`, `rotation` | A note record |
//! | `text.note.text` | `note`, `text` | The note |
//! | `text.note.move` | `note`, and `x` and/or `y` | The note |
//! | `text.note.style` | `note`, `style` | The note |
//! | `text.note.layout` | `note`, and any of `width` (`null` stops wrapping), `align`, `list`, `rotation` | The note |
//! | `text.note.delete` | `note` | Its pointer goes, leaders and all |
//! | `text.leader.add` | `note`, `points` (two or more `[x, y]` pairs); optional `arrow` (default `filled`) | The note, with a leader of the next number |
//! | `text.leader.move` | `note`, `leader` (its number), and `points` and/or `arrow` | The note |
//! | `text.leader.remove` | `note`, `leader` | The note |
//!
//! The values: `size`, `width`, `x`, `y` and a leader's points are typed text read with
//! construct-params' metric display settings (`3.5`, `297.5 mm`); `x` and `y` are in the view's own
//! coordinates, the same ones its elements are placed in, and `size` is the size on paper.
//! `colour` is `[red, green, blue]`, each 0 to 255. `spacing` is a whole percentage from 50 to 300.
//! `text` is the lines separated by line feeds. `align` is `left`, `centre` or `right`; `list` is
//! `none`, `bullet` or `number`; `rotation` is typed degrees, anticlockwise, within one turn; an
//! `arrow` is `none`, `open`, `filled`, `dot` or `slash`.
//!
//! Refused by name: an empty or taken style name (case aside); a size, width or spacing that is not
//! positive or in range; empty text; a note in a view that is not there or reading a style that is
//! not; a leader with fewer than two points; deleting a style notes still use (naming how many);
//! deleting a view that still holds notes (`view.delete` asks, naming how many). A note keeps its
//! view for life. A leader is not tied to an element: it is where it is drawn, so deleting an
//! element leaves every leader as it was.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a note or
//! a style and `element.set`, `element.move` or `element.delete` of one are refused, naming the
//! command to use. Nothing about a note's layout is stored: [`listing_of_view`] lays each note out
//! from the record, its style and font metrics the caller supplies, or leaves the layout out when
//! the caller has no fonts.

use crate::cloud::sheets::{Answer, Run, Write, length, need};
use construct_model::element::Element;
use construct_params::{Angle, Definition, Holder, Kind, Length, Units, Value};
use construct_regen::parameters as records;
use construct_text::{
    Align, Arrow, Leader, ListKind, NOTE_KIND, STYLE_KIND, TextNote, TextStyle, Texts, lay_out,
};
pub use construct_text::{Face, FontMetrics, TextError};
use kit_core::Id;
use kit_ops::Edit;
use kit_store::{Reader, StoreError, Txn};
use kit_sync::{Applied, Context as Ordered, Outcome};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Every command here.
pub const COMMANDS: [&str; 12] = [
    "text.style.create",
    "text.style.set",
    "text.style.delete",
    "text.note.create",
    "text.note.text",
    "text.note.move",
    "text.note.style",
    "text.note.layout",
    "text.note.delete",
    "text.leader.add",
    "text.leader.move",
    "text.leader.remove",
];

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// A point of a leader as a person sends it.
type PointArgs = (String, String);

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    style: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    note: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    view: Option<Id>,
    leader: Option<u32>,
    name: Option<String>,
    font: Option<String>,
    size: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    colour: Option<Vec<i64>>,
    spacing: Option<u16>,
    text: Option<String>,
    x: Option<String>,
    y: Option<String>,
    /// The wrap width; `null` stops wrapping, so `null` is kept apart from none.
    #[serde(default, deserialize_with = "present")]
    width: Option<WidthChange>,
    align: Option<String>,
    list: Option<String>,
    rotation: Option<String>,
    points: Option<Vec<PointArgs>>,
    arrow: Option<String>,
}

/// An argument that is there, `null` included.
fn present<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<WidthChange>, D::Error> {
    Option::<String>::deserialize(deserializer)
        .map(|width| Some(width.map_or(WidthChange::Away, WidthChange::To)))
}

/// What a width argument asks for.
enum WidthChange {
    /// `null`: the note stops wrapping.
    Away,
    /// A width to wrap at, as typed.
    To(String),
}

/// A document as the text commands read it: its styles and notes, its views, and each record's
/// hash.
pub struct World {
    texts: Texts,
    views: construct_views::Views,
    all: BTreeMap<Id, (Id, Element)>,
}

impl World {
    /// The document's styles and notes.
    #[must_use]
    pub fn texts(&self) -> &Texts {
        &self.texts
    }

    /// The document's views.
    #[must_use]
    pub fn views(&self) -> &construct_views::Views {
        &self.views
    }

    fn view_exists(&self, id: &Id) -> bool {
        self.views.get(id).is_some()
    }

    fn record(&self, id: &Id) -> Option<&(Id, Element)> {
        self.all.get(id)
    }
}

/// A document's text styles, notes and views as they are now. The inner error is a record that is
/// not as written, in words.
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn read_world<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Result<World, String>, StoreError> {
    let all = records::read_document(reader, project, document).await?;
    let found: Vec<&Element> = all.values().map(|(_, record)| record).collect();
    let texts = match Texts::from_records(found.iter().copied()) {
        Ok(texts) => texts,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let views = match construct_views::Views::from_records(found.iter().copied()) {
        Ok(views) => views,
        Err(e) => return Ok(Err(e.to_string())),
    };
    Ok(Ok(World { texts, views, all }))
}

fn stored(length: Length) -> String {
    Value::Length(length).stored()
}

/// A text style as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StyleView {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// The font, by name.
    pub font: String,
    /// The size on paper, in construct-params' stored length text.
    pub size: String,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Underline.
    pub underline: bool,
    /// Red, green, blue.
    pub colour: [u8; 3],
    /// The line spacing in percent.
    pub spacing: u16,
    /// How many notes read it.
    pub notes: usize,
}

/// A leader as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LeaderView {
    /// Its number in its note.
    pub number: u32,
    /// The arrowhead at the last point.
    pub arrow: String,
    /// The points, each `[x, y]` in stored length text.
    pub points: Vec<[String; 2]>,
}

/// One laid-out line, every length in stored length text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LineView {
    /// The list marker on the first line of an item, or empty.
    pub marker: String,
    /// The text.
    pub text: String,
    /// Where the line's block starts, from the box's left edge.
    pub x: String,
    /// How far the text sits right of `x`.
    pub indent: String,
    /// The block's width.
    pub width: String,
    /// The line's top, down from the box's top.
    pub top: String,
}

/// A note's box, laid out, in the view's coordinates before rotation: the top left corner is the
/// note's anchor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LaidView {
    /// The lines, top to bottom.
    pub lines: Vec<LineView>,
    /// How far one line takes.
    pub line_height: String,
    /// The box's width.
    pub width: String,
    /// The box's height.
    pub height: String,
    /// Left edge.
    pub min_x: String,
    /// Bottom edge.
    pub min_y: String,
    /// Right edge.
    pub max_x: String,
    /// Top edge.
    pub max_y: String,
}

/// A note as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NoteView {
    /// Its permanent ID.
    pub id: String,
    /// The style it reads.
    pub style: String,
    /// The anchor across the view, stored length text.
    pub x: String,
    /// The anchor up the view.
    pub y: String,
    /// The lines of text.
    pub lines: Vec<String>,
    /// The wrap width, if it wraps.
    pub width: Option<String>,
    /// `left`, `centre` or `right`.
    pub align: String,
    /// `none`, `bullet` or `number`.
    pub list: String,
    /// The rotation, in construct-params' stored angle text.
    pub rotation: String,
    /// The leaders, by number.
    pub leaders: Vec<LeaderView>,
    /// The layout, worked from the record, the style and the caller's font metrics; `None` when
    /// the caller has no fonts to measure with.
    pub laid: Option<LaidView>,
}

/// A view's notes, and the document's styles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TextListing {
    /// The view.
    pub view: String,
    /// Every style of the document, by name.
    pub styles: Vec<StyleView>,
    /// The view's notes, by ID.
    pub notes: Vec<NoteView>,
    /// Whether the notes carry a layout.
    pub laid_out: bool,
}

fn style_view(style: &TextStyle, texts: &Texts) -> StyleView {
    StyleView {
        id: style.id().to_string(),
        name: style.name().to_owned(),
        font: style.font().to_owned(),
        size: stored(style.size()),
        bold: style.bold(),
        italic: style.italic(),
        underline: style.underline(),
        colour: style.colour(),
        spacing: style.spacing(),
        notes: texts.uses_of(style.id()),
    }
}

fn laid_view(
    note: &TextNote,
    style: &TextStyle,
    metrics: &dyn FontMetrics,
) -> Result<LaidView, TextError> {
    let laid = lay_out(note, style, metrics)?;
    let [min_x, min_y, max_x, max_y] = laid.bounds(note)?;
    Ok(LaidView {
        lines: laid
            .lines
            .iter()
            .map(|line| LineView {
                marker: line.marker.clone(),
                text: line.text.clone(),
                x: stored(line.x),
                indent: stored(line.indent),
                width: stored(line.width),
                top: stored(line.top),
            })
            .collect(),
        line_height: stored(laid.line_height),
        width: stored(laid.width),
        height: stored(laid.height),
        min_x: stored(min_x),
        min_y: stored(min_y),
        max_x: stored(max_x),
        max_y: stored(max_y),
    })
}

/// One view's notes with their laid-out boxes, and the document's styles. `None` when the document
/// has no such view. With no `metrics` the notes carry no layout (nothing is guessed).
///
/// # Errors
/// The crates' words for a note whose layout cannot be worked (a font the metrics do not know).
pub fn listing_of_view(
    world: &World,
    view: &Id,
    metrics: Option<&dyn FontMetrics>,
) -> Result<Option<TextListing>, String> {
    if !world.view_exists(view) {
        return Ok(None);
    }
    let mut notes = Vec::new();
    for note in world.texts.notes_of(view) {
        let laid = match (metrics, world.texts.style(note.style())) {
            (Some(metrics), Some(style)) => {
                Some(laid_view(note, style, metrics).map_err(|e| e.to_string())?)
            }
            _ => None,
        };
        notes.push(NoteView {
            id: note.id().to_string(),
            style: note.style().to_string(),
            x: stored(note.x()),
            y: stored(note.y()),
            lines: note.lines().to_vec(),
            width: note.width().map(stored),
            align: note.align().key().to_owned(),
            list: note.list().key().to_owned(),
            rotation: Value::Angle(note.rotation()).stored(),
            leaders: note
                .leaders()
                .iter()
                .map(|(number, leader)| LeaderView {
                    number: *number,
                    arrow: leader.arrow().key().to_owned(),
                    points: leader
                        .points()
                        .iter()
                        .map(|(x, y)| [stored(*x), stored(*y)])
                        .collect(),
                })
                .collect(),
            laid,
        });
    }
    Ok(Some(TextListing {
        view: view.to_string(),
        styles: world
            .texts
            .styles()
            .into_iter()
            .map(|style| style_view(style, &world.texts))
            .collect(),
        notes,
        laid_out: metrics.is_some(),
    }))
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
    let world = match read_world(run.txn, project, &edit.target).await? {
        Ok(world) => world,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match edit.cmd.as_str() {
        "text.style.create" => create_style(&mut run, &world, args).await,
        "text.style.set" => set_style(&mut run, &world, args).await,
        "text.style.delete" => delete_style(&mut run, &world, args).await,
        "text.note.create" => create_note(&mut run, &world, args).await,
        "text.note.text" | "text.note.move" | "text.note.style" | "text.note.layout" => {
            change_note(&mut run, &world, args).await
        }
        "text.note.delete" => delete_note(&mut run, &world, args).await,
        "text.leader.add" | "text.leader.move" | "text.leader.remove" => {
            change_leader(&mut run, &world, args).await
        }
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

fn colour_of(parts: &[i64]) -> Result<[u8; 3], String> {
    let words = || "A colour is [red, green, blue], each a whole number from 0 to 255.".to_owned();
    let part = |at: usize| {
        parts
            .get(at)
            .and_then(|n| u8::try_from(*n).ok())
            .ok_or_else(words)
    };
    if parts.len() != 3 {
        return Err(words());
    }
    Ok([part(0)?, part(1)?, part(2)?])
}

/// A rotation typed by a person, in degrees: `45`, `-22.5`, `90 deg`.
fn angle(text: &str) -> Result<Angle, String> {
    let id = Id::parse("rotation").map_err(|e| e.to_string())?;
    let definition = Definition::new(id, "Rotation", Kind::Angle, Holder::Instance)
        .map_err(|e| e.to_string())?;
    match definition
        .parse(text, &Units::default())
        .map_err(|e| e.to_string())?
    {
        Value::Angle(angle) => Ok(angle),
        _ => Err(format!("{text:?} is not an angle")),
    }
}

fn choice<T>(
    text: &str,
    read: fn(&str) -> Option<T>,
    what: &str,
    keys: &[&str],
) -> Result<T, String> {
    read(text).ok_or_else(|| format!("{text:?} is not {what}: {}.", keys.join(", ")))
}

fn point(pair: &PointArgs) -> Result<(Length, Length), String> {
    Ok((length(&pair.0)?, length(&pair.1)?))
}

/// The style a command is about, with its record's hash.
fn style_of(world: &World, id: Option<Id>) -> Result<(Id, TextStyle), String> {
    let id = need(id, "a style")?;
    let missing = || TextError::UnknownStyle(id.clone()).to_string();
    let (hash, record) = world.record(&id).ok_or_else(missing)?;
    if record.kind != STYLE_KIND {
        return Err(missing());
    }
    let style = TextStyle::from_record(record).map_err(|e| e.to_string())?;
    Ok((hash.clone(), style))
}

/// The note a command is about, with its record's hash.
fn note_of(world: &World, id: Option<Id>) -> Result<(Id, TextNote), String> {
    let id = need(id, "a note")?;
    let missing = || TextError::UnknownNote(id.clone()).to_string();
    let (hash, record) = world.record(&id).ok_or_else(missing)?;
    if record.kind != NOTE_KIND {
        return Err(missing());
    }
    let note = TextNote::from_record(record).map_err(|e| e.to_string())?;
    Ok((hash.clone(), note))
}

/// What a style takes from the arguments over `style`.
fn styled(mut style: TextStyle, args: &Args) -> Result<TextStyle, String> {
    let e = |e: TextError| e.to_string();
    if let Some(name) = &args.name {
        style = style.renamed(name).map_err(e)?;
    }
    if let Some(font) = &args.font {
        style = style.with_font(font).map_err(e)?;
    }
    if let Some(size) = &args.size {
        style = style.with_size(length(size)?).map_err(e)?;
    }
    if let Some(bold) = args.bold {
        style = style.with_bold(bold);
    }
    if let Some(italic) = args.italic {
        style = style.with_italic(italic);
    }
    if let Some(underline) = args.underline {
        style = style.with_underline(underline);
    }
    if let Some(parts) = &args.colour {
        style = style.with_colour(colour_of(parts)?);
    }
    if let Some(percent) = args.spacing {
        style = style.with_spacing(percent).map_err(e)?;
    }
    Ok(style)
}

async fn create_style<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let made = (|| {
        let id = need(args.style.clone(), "a style")?;
        let name = need(args.name.as_deref(), "a name")?;
        let font = need(args.font.as_deref(), "a font")?;
        let size = length(need(args.size.as_deref(), "a size")?)?;
        let style = TextStyle::new(id, name, font, size).map_err(|e| e.to_string())?;
        // The rest of the arguments over the new style, without the name, font and size again.
        let rest = Args {
            name: None,
            font: None,
            size: None,
            ..args_of(&args)
        };
        styled(style, &rest)
    })();
    let style = match made {
        Ok(style) => style,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if world.record(style.id()).is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let mut texts = world.texts.clone();
    if let Err(e) = texts.add_style(style.clone()) {
        return Ok(Err(e.to_string()));
    }
    run.write_all(vec![Write::Put(
        Box::new(style.to_record(run.document)),
        None,
    )])
    .await
}

/// The style arguments of `args`, copied.
fn args_of(args: &Args) -> Args {
    Args {
        bold: args.bold,
        italic: args.italic,
        underline: args.underline,
        colour: args.colour.clone(),
        spacing: args.spacing,
        ..Args::default()
    }
}

async fn set_style<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, old) = match style_of(world, args.style.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let changed = match styled(old.clone(), &args) {
        Ok(style) => style,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if changed == old {
        // Nothing changed: no record is written and no step is made.
        return Ok(Ok(Applied::default()));
    }
    let mut texts = world.texts.clone();
    if let Err(e) = texts.replace_style(changed.clone()) {
        return Ok(Err(e.to_string()));
    }
    run.write_all(vec![Write::Put(
        Box::new(changed.to_record(run.document)),
        Some(hash),
    )])
    .await
}

async fn delete_style<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, style) = match style_of(world, args.style.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut texts = world.texts.clone();
    if let Err(e) = texts.remove_style(style.id()) {
        return Ok(Err(e.to_string()));
    }
    run.write_all(vec![Write::Remove(style.id().clone(), hash)])
        .await
}

async fn create_note<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let made = (|| {
        let id = need(args.note.clone(), "a note")?;
        let view = need(args.view.clone(), "a view")?;
        let style = need(args.style.clone(), "a style")?;
        let text = need(args.text.as_deref(), "text")?;
        let x = length(need(args.x.as_deref(), "an x")?)?;
        let y = length(need(args.y.as_deref(), "a y")?)?;
        let note = TextNote::new(id, view, style, (x, y), text).map_err(|e| e.to_string())?;
        laid_args(note, &args)
    })();
    let note = match made {
        Ok(note) => note,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if world.record(note.id()).is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let mut texts = world.texts.clone();
    if let Err(e) = texts.add_note(note.clone(), |view| world.view_exists(view)) {
        return Ok(Err(e.to_string()));
    }
    run.write_all(vec![Write::Put(
        Box::new(note.to_record(run.document)),
        None,
    )])
    .await
}

/// A note's width, alignment, list and rotation from the arguments.
fn laid_args(mut note: TextNote, args: &Args) -> Result<TextNote, String> {
    let e = |e: TextError| e.to_string();
    if let Some(width) = &args.width {
        let width = match width {
            WidthChange::To(text) => Some(length(text)?),
            WidthChange::Away => None,
        };
        note = note.with_width(width).map_err(e)?;
    }
    if let Some(align) = &args.align {
        let keys = ["left", "centre", "right"];
        note = note.with_align(choice(align, Align::from_key, "an alignment", &keys)?);
    }
    if let Some(list) = &args.list {
        let keys = ["none", "bullet", "number"];
        note = note.with_list(choice(list, ListKind::from_key, "a list kind", &keys)?);
    }
    if let Some(rotation) = &args.rotation {
        note = note.with_rotation(angle(rotation)?).map_err(e)?;
    }
    Ok(note)
}

async fn put_note<T: Txn>(
    run: &mut Run<'_, T>,
    world: &World,
    before: (Id, TextNote),
    changed: TextNote,
) -> Answer {
    let (hash, old) = before;
    if changed == old {
        return Ok(Ok(Applied::default()));
    }
    let mut texts = world.texts.clone();
    if let Err(e) = texts.replace_note(changed.clone()) {
        return Ok(Err(e.to_string()));
    }
    run.write_all(vec![Write::Put(
        Box::new(changed.to_record(run.document)),
        Some(hash),
    )])
    .await
}

/// `text.note.text`, `.move`, `.style` and `.layout`: each changes one note and refuses the
/// arguments of the others.
async fn change_note<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, old) = match note_of(world, args.note.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let cmd = run.edit.cmd.clone();
    let changed = (|| {
        let note = old.clone();
        match cmd.as_str() {
            "text.note.text" => {
                let text = need(args.text.as_deref(), "text")?;
                note.with_text(text).map_err(|e| e.to_string())
            }
            "text.note.move" => {
                if args.x.is_none() && args.y.is_none() {
                    return Err("This command needs x, y or both.".to_owned());
                }
                let x = match &args.x {
                    Some(text) => length(text)?,
                    None => old.x(),
                };
                let y = match &args.y {
                    Some(text) => length(text)?,
                    None => old.y(),
                };
                Ok(note.at(x, y))
            }
            "text.note.style" => Ok(note.with_style(need(args.style.clone(), "a style")?)),
            _ => {
                if args.width.is_none()
                    && args.align.is_none()
                    && args.list.is_none()
                    && args.rotation.is_none()
                {
                    return Err(
                        "This command needs a width, an align, a list or a rotation.".to_owned(),
                    );
                }
                laid_args(note, &args)
            }
        }
    })();
    match changed {
        Ok(changed) => put_note(run, world, (hash, old), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn delete_note<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, note) = match note_of(world, args.note.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    run.write_all(vec![Write::Remove(note.id().clone(), hash)])
        .await
}

/// `text.leader.add`, `.move` and `.remove`.
async fn change_leader<T: Txn>(run: &mut Run<'_, T>, world: &World, args: Args) -> Answer {
    let (hash, old) = match note_of(world, args.note.clone()) {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let cmd = run.edit.cmd.clone();
    let changed = (|| {
        let e = |e: TextError| e.to_string();
        let arrow = |text: &str| {
            let keys: Vec<&str> = Arrow::ALL.iter().map(|a| a.key()).collect();
            choice(text, Arrow::from_key, "an arrowhead", &keys)
        };
        let points = |pairs: &[PointArgs]| -> Result<Vec<(Length, Length)>, String> {
            pairs.iter().map(point).collect()
        };
        let note = old.clone();
        match cmd.as_str() {
            "text.leader.add" => {
                let pairs = need(args.points.as_deref(), "points")?;
                let head = match &args.arrow {
                    Some(text) => arrow(text)?,
                    None => Arrow::Filled,
                };
                let leader = Leader::new(points(pairs)?, head).map_err(e)?;
                note.with_leader(leader).map(|(note, _)| note).map_err(e)
            }
            "text.leader.move" => {
                let number = need(args.leader, "a leader number")?;
                let Some(current) = old.leaders().get(&number) else {
                    return Err(TextError::UnknownLeader {
                        note: old.id().clone(),
                        leader: number,
                    }
                    .to_string());
                };
                if args.points.is_none() && args.arrow.is_none() {
                    return Err("This command needs points, an arrow or both.".to_owned());
                }
                let head = match &args.arrow {
                    Some(text) => arrow(text)?,
                    None => current.arrow(),
                };
                let line = match &args.points {
                    Some(pairs) => points(pairs)?,
                    None => current.points().to_vec(),
                };
                let leader = Leader::new(line, head).map_err(e)?;
                note.with_leader_replaced(number, leader).map_err(e)
            }
            _ => {
                let number = need(args.leader, "a leader number")?;
                note.without_leader(number).map_err(e)
            }
        }
    })();
    match changed {
        Ok(changed) => put_note(run, world, (hash, old), changed).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// Why a view cannot be deleted: it holds text notes. `None` when it holds none (or no text record
/// reads, which the text commands report).
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn notes_refusal<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    view: &Id,
) -> Result<Option<String>, StoreError> {
    let Ok(world) = read_world(reader, project, document).await? else {
        return Ok(None);
    };
    Ok(world.texts.view_free(view).err().map(|e| e.to_string()))
}

/// The part of a generic command the rules below read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    recipe: Option<construct_model::geometry::Recipe>,
}

/// The generic commands do not go behind the text commands: `element.create` of a note or a style
/// and `element.set`, `element.move` or `element.delete` of one are refused, naming the command to
/// use. Anything else is left to the command.
///
/// # Errors
///
/// A [`StoreError`] when the store fails.
pub async fn generic_rules<T: Txn>(
    txn: &mut T,
    project: &Id,
    edit: &Edit,
) -> Result<Result<(), String>, StoreError> {
    let Ok(args) = serde_json::from_value::<GenericArgs>(edit.args.clone()) else {
        return Ok(Ok(()));
    };
    let text_kind = |kind: &str| matches!(kind, NOTE_KIND | STYLE_KIND);
    let by_recipe = args
        .recipe
        .as_ref()
        .is_some_and(|recipe| text_kind(&records::kind_name(recipe)));
    let by_kind = args.kind.as_deref().is_some_and(text_kind);
    let use_instead = |what: &str, command: &str| format!("{what}: use {command}.");
    Ok(match edit.cmd.as_str() {
        "element.create" if by_recipe || by_kind => Err(use_instead(
            "A text note or a text style is made",
            "text.note.create or text.style.create",
        )),
        "element.set" | "element.move" | "element.delete" => {
            let Some(id) = &args.element else {
                return Ok(Ok(()));
            };
            match records::read_element(txn, project, &edit.target, id).await? {
                Some((_, record)) if text_kind(&record.kind) => {
                    let note = record.kind == NOTE_KIND;
                    Err(match (edit.cmd.as_str(), note) {
                        ("element.delete", true) => {
                            use_instead("A text note is deleted", "text.note.delete")
                        }
                        ("element.delete", false) => {
                            use_instead("A text style is deleted", "text.style.delete")
                        }
                        (_, true) => use_instead(
                            "A text note is changed",
                            "text.note.text, .move, .style or .layout, or text.leader.*",
                        ),
                        (_, false) => use_instead("A text style is changed", "text.style.set"),
                    })
                }
                Some(_) if by_recipe => Err(use_instead(
                    "A text note or a text style is made",
                    "text.note.create or text.style.create",
                )),
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    })
}

/// Whether the text an undo wrote still stands: every record reads, every style name is free, every
/// note is in a view that is there and reads a style that is there. The whole document is checked
/// whenever a text record or a view was written, or any piece was taken away.
///
/// # Errors
/// A store error; the inner error names what would not stand.
pub async fn undo_rules<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    written: &[Element],
    deleted: &[Id],
) -> Result<Result<(), String>, StoreError> {
    let touched = written
        .iter()
        .any(|record| matches!(record.kind.as_str(), NOTE_KIND | STYLE_KIND | "view"));
    // A piece taken away may be a view, a style or a note: its kind is not known any more.
    if !touched && deleted.is_empty() {
        return Ok(Ok(()));
    }
    let world = match read_world(txn, project, document).await? {
        Ok(world) => world,
        Err(why) => return Ok(Err(why)),
    };
    let mut rebuilt = Texts::new();
    for style in world.texts.styles() {
        if let Err(e) = rebuilt.add_style(style.clone()) {
            return Ok(Err(e.to_string()));
        }
    }
    for note in world.texts.notes() {
        if let Err(e) = rebuilt.add_note(note.clone(), |view| world.view_exists(view)) {
            return Ok(Err(e.to_string()));
        }
    }
    Ok(Ok(()))
}
