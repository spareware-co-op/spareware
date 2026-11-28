//! The dimension style commands: a person's own styles, made, changed and deleted through the edit
//! log as records of the dimensions category with no shape ([`construct_dimensions::DimensionStyle`]),
//! and the styles as a page reads them.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `dimension.style.create` | `style` (the new ID), `name`, optional `from` (the style it starts from; default the first built-in one), and any of the look's fields below | A style record |
//! | `dimension.style.change` | `style`, and any of `name` and the look's fields | The style; nothing at all when it already is so |
//! | `dimension.style.delete` | `style` | Its pointer goes |
//!
//! The look's fields: `marker` (`tick`, `arrow` or `dot`), `text_place` (`above` or `inline`),
//! `length` (`{"unit": "mm" or "m", "places": n}`, or null for the project's), `angle_places` (a
//! number of places in degrees, or null for the project's), `gap`, `extension` and `height`
//! (typed lengths as printed on paper: `1.5`, `2 mm`), `centre_mark` (yes or no).
//!
//! Refused by name: a built-in style (its ID is taken, and it is not changed or deleted: make one
//! of your own from it); an ID that is taken; a name another style has, case aside; a look out of
//! range, in construct-dimensions' words; deleting a style a dimension is in.

use super::{Answer, Run, World, Write, length, need, put_style, style_held, words};
use construct_dimensions::{DimensionStyle, Look, Marker, TextPlace};
use construct_params::{Format, Value};
use kit_core::Id;
use kit_store::Txn;
use serde::{Deserialize, Deserializer, Serialize};

/// A field given, even as null: `None` when it is left out.
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<serde_json::Value>, D::Error> {
    serde_json::Value::deserialize(d).map(Some)
}

/// The arguments of the style commands; each uses some of them.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct StyleArgs {
    #[serde(default, with = "kit_ops::id::option")]
    style: Option<Id>,
    #[serde(default, with = "kit_ops::id::option")]
    from: Option<Id>,
    name: Option<String>,
    marker: Option<String>,
    text_place: Option<String>,
    #[serde(default, deserialize_with = "present")]
    length: Option<serde_json::Value>,
    #[serde(default, deserialize_with = "present")]
    angle_places: Option<serde_json::Value>,
    gap: Option<String>,
    extension: Option<String>,
    height: Option<String>,
    centre_mark: Option<bool>,
}

/// `base` with the fields the arguments give.
fn look_with(base: Look, args: &StyleArgs) -> Result<Look, String> {
    let mut look = base;
    if let Some(marker) = &args.marker {
        look.marker = Marker::from_key(marker)
            .ok_or_else(|| format!("{marker:?} is not a marker: tick, arrow or dot."))?;
    }
    if let Some(place) = &args.text_place {
        look.text = TextPlace::from_key(place)
            .ok_or_else(|| format!("{place:?} is not a place for text: above or inline."))?;
    }
    if let Some(format) = &args.length {
        look.length = if format.is_null() {
            None
        } else {
            Some(serde_json::from_value::<Format>(format.clone()).map_err(|_| {
                "A style's lengths are {\"unit\": \"mm\" or \"m\", \"places\": a number}, or null \
                 for the project's."
                    .to_owned()
            })?)
        };
    }
    if let Some(places) = &args.angle_places {
        look.angle_places = if places.is_null() {
            None
        } else {
            Some(
                places
                    .as_u64()
                    .and_then(|n| u8::try_from(n).ok())
                    .ok_or_else(|| {
                        "A style's angle places are a number, or null for the project's.".to_owned()
                    })?,
            )
        };
    }
    let paper = |typed: &Option<String>, was| typed.as_deref().map_or(Ok(was), length);
    look.gap = paper(&args.gap, look.gap)?;
    look.extension = paper(&args.extension, look.extension)?;
    look.height = paper(&args.height, look.height)?;
    if let Some(mark) = args.centre_mark {
        look.centre_mark = mark;
    }
    Ok(look)
}

/// Runs a style command.
pub(super) async fn apply<T: Txn>(run: &mut Run<'_, T>, world: &World) -> Answer {
    let args: StyleArgs = match serde_json::from_value(run.edit.args.clone()) {
        Ok(args) => args,
        Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
    };
    match run.edit.cmd.as_str() {
        "dimension.style.create" => create(run, world, &args).await,
        "dimension.style.change" => change(run, world, &args).await,
        "dimension.style.delete" => delete(run, world, &args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

async fn create<T: Txn>(run: &mut Run<'_, T>, world: &World, args: &StyleArgs) -> Answer {
    let made = (|| {
        let id = need(args.style.clone(), "a style")?;
        let name = need(args.name.as_deref(), "a name")?;
        let base = match &args.from {
            Some(from) => world
                .styles
                .get(from)
                .map(|style| *style.look())
                .ok_or_else(|| format!("There is no dimension style {from} in this model."))?,
            None => Look::ticks(),
        };
        if world.record(&id).is_some() {
            return Err("That element already exists.".to_owned());
        }
        let style = DimensionStyle::new(id, name, look_with(base, args)?).map_err(|e| words(&e))?;
        let mut styles = world.styles.clone();
        styles.add(style.clone()).map_err(|e| words(&e))?;
        Ok(style)
    })();
    match made {
        Ok(style) => {
            run.write_all(vec![Write::Put(
                Box::new(style.to_record(run.document)),
                None,
            )])
            .await
        }
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// The document's own style a command is about, with its record's hash.
fn own<'a>(world: &'a World, args: &StyleArgs) -> Result<(&'a Id, &'a DimensionStyle), String> {
    let id = need(args.style.clone(), "a style")?;
    let style = world
        .styles
        .get(&id)
        .ok_or_else(|| format!("There is no dimension style {id} in this model."))?;
    if style.is_builtin() {
        return Err(format!(
            "{} is a built-in style: it is not changed or deleted. Make a style of your own from \
             it with dimension.style.create.",
            style.name()
        ));
    }
    let (hash, _) = world
        .record(&id)
        .ok_or_else(|| format!("There is no dimension style {id} in this model."))?;
    Ok((hash, style))
}

async fn change<T: Txn>(run: &mut Run<'_, T>, world: &World, args: &StyleArgs) -> Answer {
    let changed = (|| {
        if args.from.is_some() {
            return Err(
                "A style is changed field by field: from is for dimension.style.create.".to_owned(),
            );
        }
        let (hash, style) = own(world, args)?;
        let renamed = match &args.name {
            Some(name) => style.renamed(name).map_err(|e| words(&e))?,
            None => style.clone(),
        };
        let after = renamed
            .with_look(look_with(*style.look(), args)?)
            .map_err(|e| words(&e))?;
        let mut styles = world.styles.clone();
        styles.replace(after.clone()).map_err(|e| words(&e))?;
        Ok((hash.clone(), style.clone(), after))
    })();
    match changed {
        Ok((hash, before, after)) => put_style(run, &hash, &before, after).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn delete<T: Txn>(run: &mut Run<'_, T>, world: &World, args: &StyleArgs) -> Answer {
    let found = own(world, args).and_then(|(hash, style)| match style_held(world, style.id()) {
        Some(refusal) => Err(refusal),
        None => Ok((hash.clone(), style.id().clone())),
    });
    match found {
        Ok((hash, id)) => run.write_all(vec![Write::Remove(id, hash)]).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// Whether no two styles share a name, case aside (an undo checks it).
pub(super) fn names_unique(world: &World) -> Result<(), String> {
    let all = world.styles.all();
    for (at, style) in all.iter().enumerate() {
        let name = style.name().to_lowercase();
        if let Some(other) = all
            .iter()
            .skip(at.saturating_add(1))
            .find(|other| other.name().to_lowercase() == name)
        {
            return Err(format!(
                "the dimension styles {} and {} would share the name {}",
                style.id(),
                other.id(),
                other.name()
            ));
        }
    }
    Ok(())
}

/// How lengths are shown, as a page reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FormatView {
    /// `mm` or `m`.
    pub unit: String,
    /// The places shown.
    pub places: u8,
}

/// A dimension style as a page reads it. Sizes are as printed on paper, in stored length text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StyleView {
    /// Its ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Whether it is a built-in one, which is not changed or deleted.
    pub builtin: bool,
    /// `tick`, `arrow` or `dot`.
    pub marker: String,
    /// `above` or `inline`.
    pub text_place: String,
    /// How lengths are shown; `None`: as the project shows them.
    pub length: Option<FormatView>,
    /// How many places angles are shown to, in degrees; `None`: as the project shows them.
    pub angle_places: Option<u8>,
    /// How far a witness line stops short of its reference.
    pub gap: String,
    /// How far a witness line runs past the dimension's line.
    pub extension: String,
    /// How high the text is.
    pub height: String,
    /// Whether a radius or a diameter marks its arc's centre.
    pub centre_mark: bool,
}

/// A style as a page reads it.
#[must_use]
pub fn style_view(style: &DimensionStyle) -> StyleView {
    let look = style.look();
    StyleView {
        id: style.id().to_string(),
        name: style.name().to_owned(),
        builtin: style.is_builtin(),
        marker: look.marker.key().to_owned(),
        text_place: look.text.key().to_owned(),
        length: look.length.map(|format| FormatView {
            unit: format.unit.symbol().to_owned(),
            places: format.places,
        }),
        angle_places: look.angle_places,
        gap: Value::Length(look.gap).stored(),
        extension: Value::Length(look.extension).stored(),
        height: Value::Length(look.height).stored(),
        centre_mark: look.centre_mark,
    }
}
