//! A spec judged against its template's rules before anything is built, and its values worked
//! out: every part the spec leaves unset reads the template's default, and a clear size becomes
//! the overall size whose clear opening it is.
//!
//! The rules, in the order a spec is judged (each refusal names its [`Rule`] and the field):
//!
//! 1. [`Rule::NotInTemplate`], [`Rule::Hand`]: the spec sets only what its template has.
//! 2. [`Rule::Positive`]: every size and part is more than zero; a sill height is at or above the
//!    floor; a wall is thicker than nothing.
//! 3. [`Rule::Width`], [`Rule::Height`]: the overall size is in the template's range.
//! 4. [`Rule::Fits`]: a door leaf, a window's glass, a sliding window's two sashes and their track
//!    gap fit within the frame's depth; a sash's glass within its thickness.
//! 5. [`Rule::Clear`]: what is left inside the frame is an opening (more than zero each way); for
//!    a sliding window, the sliding sash's glass is at least 300 mm wide and 200 mm high, the room
//!    its slide mark is drawn in.
//! 6. [`Rule::Even`]: every length the template halves (its formulas say `/ 2`) is an even number
//!    of grid steps, so the half lands on the grid exactly and no quantity is rounded.
//! 7. [`Rule::Wall`]: the frame's depth is no more than the wall's thickness.
//!
//! Everything is exact: lengths are grid steps and every step is a checked one.

use crate::refusal::{Refusal, Rule};
use crate::spec::{Hand, Size, Spec};
use crate::template::{Part, Template};
use construct_families::{ParameterValue, Recipe};
use construct_params::{Length, Value};
use kit_core::Id;

/// A spec's values, judged and worked out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Resolved {
    /// The overall width.
    pub(crate) width: Length,
    /// The overall height.
    pub(crate) height: Length,
    /// Every part the template has, the spec's or the default, in the template's order.
    pub(crate) parts: Vec<(Part, Length)>,
    /// The materials the spec sets, by the template's parameter key.
    pub(crate) materials: Vec<(&'static str, Id)>,
    /// The hand, if the template draws one.
    pub(crate) hand: Option<Hand>,
    /// The sill height, if the spec sets one.
    pub(crate) sill: Option<Length>,
    /// The clear size the spec asks for, if it gives its size as clear.
    pub(crate) clear: Option<(Length, Length)>,
}

impl Resolved {
    /// A part's value (every template part is in `parts`).
    pub(crate) fn part(&self, part: Part) -> Length {
        self.parts
            .iter()
            .find(|(p, _)| *p == part)
            .map_or(Length::from_steps(0), |(_, l)| *l)
    }
}

/// The value a spec sets for a part.
fn given(spec: &Spec, part: Part) -> Option<Length> {
    match part {
        Part::Frame => spec.frame,
        Part::FrameDepth => spec.frame_depth,
        Part::Leaf => spec.leaf,
        Part::Sash => spec.sash,
        Part::SashDepth => spec.sash_depth,
        Part::TrackGap => spec.track_gap,
        Part::Glass => spec.glass,
    }
}

const ALL_PARTS: [Part; 7] = [
    Part::Frame,
    Part::FrameDepth,
    Part::Leaf,
    Part::Sash,
    Part::SashDepth,
    Part::TrackGap,
    Part::Glass,
];

/// A length parameter's default in `recipe`.
fn default_length(recipe: &Recipe, key: &str) -> Result<Length, Refusal> {
    let text = recipe
        .parameters
        .iter()
        .find(|p| p.key.as_str() == key)
        .and_then(|p| match &p.value {
            ParameterValue::Default(text) => Some(text.as_str()),
            ParameterValue::Formula(_) => None,
        });
    match text.and_then(Value::from_stored) {
        Some(Value::Length(length)) => Ok(length),
        _ => Err(Refusal::new(
            Rule::Template,
            key,
            "the template has no length parameter with a default by this key",
        )),
    }
}

/// `a + b`, refused as too large past i64 steps.
fn add(a: Length, b: Length, what: &str) -> Result<Length, Refusal> {
    a.checked_add(b).ok_or_else(|| too_large(what))
}

/// `a - b`.
fn sub(a: Length, b: Length, what: &str) -> Result<Length, Refusal> {
    a.checked_sub(b).ok_or_else(|| too_large(what))
}

/// `a + a`.
fn twice(a: Length, what: &str) -> Result<Length, Refusal> {
    add(a, a, what)
}

fn too_large(what: &str) -> Refusal {
    Refusal::new(Rule::Positive, what, "the size is too large to hold")
}

/// Judges `spec` against its template (`recipe` is the template's own) and works its values out.
///
/// # Errors
/// The first rule broken, in the order of this module's header.
pub(crate) fn judge(spec: &Spec, recipe: &Recipe) -> Result<Resolved, Refusal> {
    let template = spec.template;
    only_what_it_has(spec)?;
    let mut resolved = positive(spec, recipe)?;
    if let Some((w, h)) = resolved.clear {
        let (w, h) = overall_from_clear(template, &resolved, w, h)?;
        resolved.width = w;
        resolved.height = h;
    }
    in_range(template, &resolved)?;
    fits(template, &resolved)?;
    clear_opening(template, &resolved)?;
    even(template, &resolved)?;
    if let Some(wall) = spec.wall {
        let depth = resolved.part(Part::FrameDepth);
        if depth > wall {
            return Err(Refusal::new(
                Rule::Wall,
                "wall",
                format!("the frame is {depth} deep and the wall only {wall} thick"),
            ));
        }
    }
    Ok(resolved)
}

/// Rule 1: the spec sets only what its template has.
fn only_what_it_has(spec: &Spec) -> Result<(), Refusal> {
    let template = spec.template;
    for part in ALL_PARTS {
        if given(spec, part).is_some() && !template.parts().contains(&part) {
            return Err(Refusal::new(
                Rule::NotInTemplate,
                part.key(),
                format!("the {} template has no {}", template.stem(), part.key()),
            ));
        }
    }
    if spec.sill_height.is_some() && !template.has_sill() {
        return Err(Refusal::new(
            Rule::NotInTemplate,
            "sill_height",
            format!(
                "the {} template stands on the floor and has no sill height",
                template.stem()
            ),
        ));
    }
    if spec.leaf_material.is_some() && template.leaf_material().is_none() {
        return Err(Refusal::new(
            Rule::NotInTemplate,
            "leaf_material",
            format!("the {} template has no leaf or sash", template.stem()),
        ));
    }
    if let Some(hand) = spec.hand {
        if template.hands().is_empty() {
            return Err(Refusal::new(
                Rule::NotInTemplate,
                "hand",
                format!(
                    "the {} template does not open, so has no hand",
                    template.stem()
                ),
            ));
        }
        if !template.hands().contains(&hand) {
            return Err(Refusal::new(
                Rule::Hand,
                "hand",
                format!(
                    "the {} template draws its sliding sash on the right only; the {} hand is the \
                     placed window mirrored",
                    template.stem(),
                    hand.name()
                ),
            ));
        }
    }
    Ok(())
}

/// Rule 2: every size and part is more than zero. Gives the spec's values, every part the
/// template has, unset ones at its defaults.
fn positive(spec: &Spec, recipe: &Recipe) -> Result<Resolved, Refusal> {
    let template = spec.template;
    let mut parts = Vec::with_capacity(template.parts().len());
    for part in template.parts() {
        let value = match given(spec, *part) {
            Some(value) => value,
            None => default_length(recipe, part.key())?,
        };
        if value <= Length::from_steps(0) {
            return Err(Refusal::new(
                Rule::Positive,
                part.key(),
                format!("{} is {value}; it must be more than 0 mm", part.key()),
            ));
        }
        parts.push((*part, value));
    }
    if let Some(sill) = spec.sill_height
        && sill < Length::from_steps(0)
    {
        return Err(Refusal::new(
            Rule::Positive,
            "sill_height",
            format!("the sill height is {sill}; a sill is at or above the floor"),
        ));
    }
    if let Some(wall) = spec.wall
        && wall <= Length::from_steps(0)
    {
        return Err(Refusal::new(
            Rule::Positive,
            "wall",
            format!("the wall is {wall} thick; it must be more than 0 mm"),
        ));
    }
    let (asked_w, asked_h, clear) = match spec.size {
        Size::Overall { width, height } => (width, height, false),
        Size::Clear { width, height } => (width, height, true),
    };
    for (what, value) in [("width", asked_w), ("height", asked_h)] {
        if value <= Length::from_steps(0) {
            return Err(Refusal::new(
                Rule::Positive,
                what,
                format!("the {what} is {value}; it must be more than 0 mm"),
            ));
        }
    }
    let mut resolved = Resolved {
        width: asked_w,
        height: asked_h,
        parts,
        materials: Vec::new(),
        hand: spec.hand,
        sill: spec.sill_height,
        clear: clear.then_some((asked_w, asked_h)),
    };
    if let Some(id) = &spec.frame_material {
        resolved.materials.push(("frame_material", id.clone()));
    }
    if let (Some(id), Some(key)) = (&spec.leaf_material, template.leaf_material()) {
        resolved.materials.push((key, id.clone()));
    }
    if template == Template::SingleSwingDoor && resolved.hand.is_none() {
        resolved.hand = Some(Hand::Left);
    }
    if template == Template::SlidingWindow {
        resolved.hand = Some(Hand::Right);
    }
    Ok(resolved)
}

/// Rule 3: the overall size is in the template's range.
fn in_range(template: Template, resolved: &Resolved) -> Result<(), Refusal> {
    let worked = if resolved.clear.is_some() {
        " (worked out from the clear size asked for)"
    } else {
        ""
    };
    let (widths, heights) = (template.widths(), template.heights());
    if !widths.holds(resolved.width) {
        return Err(Refusal::new(
            Rule::Width,
            "width",
            format!(
                "the overall width is {}{worked}; the {} template is made from {} to {}",
                resolved.width,
                template.stem(),
                widths.low,
                widths.high
            ),
        ));
    }
    if !heights.holds(resolved.height) {
        return Err(Refusal::new(
            Rule::Height,
            "height",
            format!(
                "the overall height is {}{worked}; the {} template is made from {} to {}",
                resolved.height,
                template.stem(),
                heights.low,
                heights.high
            ),
        ));
    }
    Ok(())
}

/// The overall size whose clear opening is `w` by `h` (spec, `Size::Clear`):
/// a swing door's clear width is its width less two frames and the open leaf's thickness, its
/// clear height its height less the head; a fixed light's clear size is its size less two frames
/// each way; a sliding window's clear width is half of its width less two frames and one sash,
/// its clear height its height less two frames.
fn overall_from_clear(
    template: Template,
    r: &Resolved,
    w: Length,
    h: Length,
) -> Result<(Length, Length), Refusal> {
    let frame = r.part(Part::Frame);
    let frames = twice(frame, "width")?;
    Ok(match template {
        Template::SingleSwingDoor => (
            add(add(w, frames, "width")?, r.part(Part::Leaf), "width")?,
            add(h, frame, "height")?,
        ),
        Template::FixedWindow => (add(w, frames, "width")?, add(h, frames, "height")?),
        Template::SlidingWindow => (
            add(
                add(twice(w, "width")?, frames, "width")?,
                r.part(Part::Sash),
                "width",
            )?,
            add(h, frames, "height")?,
        ),
    })
}

/// Rule 4: the parts fit inside one another.
fn fits(template: Template, r: &Resolved) -> Result<(), Refusal> {
    let depth = r.part(Part::FrameDepth);
    let refuse = |what: &str, reason: String| Err(Refusal::new(Rule::Fits, what, reason));
    match template {
        Template::SingleSwingDoor => {
            let leaf = r.part(Part::Leaf);
            if leaf > depth {
                return refuse(
                    "leaf",
                    format!("the leaf is {leaf} thick and the frame only {depth} deep"),
                );
            }
        }
        Template::FixedWindow => {
            let glass = r.part(Part::Glass);
            if glass > depth {
                return refuse(
                    "glass",
                    format!("the glass is {glass} thick and the frame only {depth} deep"),
                );
            }
        }
        Template::SlidingWindow => {
            let sash = r.part(Part::SashDepth);
            let needed = add(
                twice(sash, "sash_depth")?,
                r.part(Part::TrackGap),
                "sash_depth",
            )?;
            if needed > depth {
                return refuse(
                    "sash_depth",
                    format!(
                        "two sashes and their track gap need {needed} and the frame is only \
                         {depth} deep"
                    ),
                );
            }
            let glass = r.part(Part::Glass);
            if glass > sash {
                return refuse(
                    "glass",
                    format!("the glass is {glass} thick and the sash only {sash}"),
                );
            }
        }
    }
    Ok(())
}

/// Rule 5: what is left inside the frame is an opening.
fn clear_opening(template: Template, r: &Resolved) -> Result<(), Refusal> {
    let zero = Length::from_steps(0);
    let frame = r.part(Part::Frame);
    let frames = twice(frame, "frame")?;
    let refuse = |what: &str, reason: String| Err(Refusal::new(Rule::Clear, what, reason));
    match template {
        Template::SingleSwingDoor => {
            let clear = sub(sub(r.width, frames, "width")?, r.part(Part::Leaf), "width")?;
            if clear <= zero {
                return refuse(
                    "clear_width",
                    format!(
                        "the width less two frames and the open leaf's thickness is {clear}; \
                         it must be more than 0 mm"
                    ),
                );
            }
            // The clear height (height less the head) is more than zero whenever the clear width
            // is: two frames are less than a width of at most 1,200 mm, which is less than any
            // height of the range.
        }
        Template::FixedWindow => {
            let w = sub(r.width, frames, "width")?;
            if w <= zero {
                return refuse(
                    "clear_width",
                    format!("the width less two frames is {w}; it must be more than 0 mm"),
                );
            }
            let h = sub(r.height, frames, "height")?;
            if h <= zero {
                return refuse(
                    "clear_height",
                    format!("the height less two frames is {h}; it must be more than 0 mm"),
                );
            }
        }
        Template::SlidingWindow => {
            let sash = r.part(Part::Sash);
            let sashes = add(twice(sash, "sash")?, sash, "sash")?;
            let left = sub(sub(r.width, frames, "width")?, sashes, "width")?;
            let needed = Length::from_mm(600);
            if left < needed {
                return refuse(
                    "clear_width",
                    format!(
                        "the width less two frames and three sash widths is {left}; at least \
                         {needed} is needed, so each sash's glass is 300 mm wide for the slide \
                         mark"
                    ),
                );
            }
            let high = sub(
                sub(r.height, frames, "height")?,
                twice(sash, "sash")?,
                "height",
            )?;
            let needed = Length::from_mm(200);
            if high < needed {
                return refuse(
                    "clear_height",
                    format!(
                        "the height less two frames and two sash widths is {high}; at least \
                         {needed} is needed for the slide mark"
                    ),
                );
            }
        }
    }
    Ok(())
}

/// Rule 6: every length the template halves is a whole even number of grid steps.
fn even(template: Template, r: &Resolved) -> Result<(), Refusal> {
    let mut halved: Vec<(&str, Length)> = vec![("width", r.width)];
    match template {
        Template::SingleSwingDoor => {
            halved.push(("frame_depth", r.part(Part::FrameDepth)));
            // The leaf's height: the elevation mark meets the hinge side half way up it.
            halved.push((
                "leaf_height",
                sub(r.height, r.part(Part::Frame), "leaf_height")?,
            ));
        }
        Template::FixedWindow => {
            halved.push(("frame_depth", r.part(Part::FrameDepth)));
            halved.push(("glass", r.part(Part::Glass)));
        }
        Template::SlidingWindow => {
            halved.push(("height", r.height));
            for part in [
                Part::FrameDepth,
                Part::Sash,
                Part::SashDepth,
                Part::TrackGap,
                Part::Glass,
            ] {
                halved.push((part.key(), r.part(part)));
            }
        }
    }
    for (what, length) in halved {
        if length.steps().checked_rem(2) != Some(0) {
            return Err(Refusal::new(
                Rule::Even,
                what,
                format!(
                    "{what} is {length}, {} grid steps; the template halves it, so it must be an \
                     even number of steps for its half to land on the grid exactly",
                    length.steps()
                ),
            ));
        }
    }
    Ok(())
}
