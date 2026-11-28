//! How an element looks in a view: the one resolution rule (CON-A8-F3-P2).
//!
//! For an element of a category, and of a subcategory of it or of none, seen where a view cuts it
//! or beyond the cut plane, the answer comes from the first of these that has a style for that
//! edge:
//!
//! 1. the subcategory's rule in the view ([`Origin::Subcategory`]);
//! 2. the category's rule in the view ([`Origin::Category`]);
//! 3. the project's object style for the subcategory, which is its parent's where it has none of
//!    its own, or for the category ([`Origin::Project`]): its pen for the edge, colour and pattern,
//!    with no fill of its own (the element's material fills a cut), no halftone, no transparency,
//!    shown.
//!
//! The two edges are resolved apart: a subcategory's rule for its cut does not stand in for its
//! projection. Every answer says which level gave it and carries the pen's printed width at the
//! view's scale. The drawing calls [`resolve`] for each element; nothing here draws.

use crate::error::ViewError;
use crate::graphics::{Graphics, RuleStyle, pen_exists};
use crate::style::ElementStyle;
use crate::view::View;
use construct_categories::{CategoryId, CategoryKind, CategoryTable};
use construct_styles::{Edge, LineStyles, Micrometres, ObjectStyles, Scale};
use std::collections::BTreeSet;

/// Which level of the rule gave an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The subcategory's rule in the view.
    Subcategory,
    /// The category's rule in the view.
    Category,
    /// The project's object style: the view has no rule for it.
    Project,
}

impl Origin {
    /// The word an answer to a person uses: `subcategory`, `category` or `project`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Subcategory => "subcategory",
            Self::Category => "category",
            Self::Project => "project",
        }
    }
}

/// How one edge of one element looks in one view, and where that came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The style.
    pub style: RuleStyle,
    /// The printed width of the style's pen at the view's scale.
    pub width: Micrometres,
    /// The level that gave it.
    pub origin: Origin,
}

fn width(
    lines: &LineStyles,
    kind: CategoryKind,
    pen: u16,
    scale: Scale,
) -> Result<Micrometres, ViewError> {
    if matches!(kind, CategoryKind::Annotation | CategoryKind::View) {
        lines.annotation_pens.width(pen)
    } else {
        lines.model_pens.width(pen, scale)
    }
    .map_err(|why| ViewError::Style(why.to_string()))
}

/// How an element of `category`, and of `subcategory` when it is of one, looks in `view` on `edge`.
///
/// `category` is a top-level category and `subcategory`, when given, one of its subcategories.
///
/// # Errors
/// [`ViewError::UnknownCategory`] for a category or subcategory `categories` lacks,
/// [`ViewError::NotTopLevel`] for a `category` that is itself a subcategory,
/// [`ViewError::NotASubcategory`] for a subcategory of another category,
/// [`ViewError::CutOnUncuttable`] for the cut edge of one that cannot be cut, and
/// [`ViewError::Style`] when a pen is not in `lines` (a view checked against the same `lines`
/// never hits this).
pub fn resolve(
    view: &View,
    categories: &CategoryTable,
    objects: &ObjectStyles,
    lines: &LineStyles,
    category: CategoryId,
    subcategory: Option<CategoryId>,
    edge: Edge,
) -> Result<Resolved, ViewError> {
    resolve_in(
        view.graphics(),
        view.hidden(),
        view.scale(),
        categories,
        objects,
        lines,
        category,
        subcategory,
        edge,
    )
}

/// [`resolve`] over a view's three inputs (its graphics rules, its hidden categories and its
/// scale) instead of a view, so a view template, which is not a view, is asked the same way. A
/// category the view hides, or whose parent it hides, is not shown, whatever its rules say.
///
/// # Errors
/// As [`resolve`].
#[allow(
    clippy::too_many_arguments,
    reason = "the three inputs of a view and the five of the question; a struct would only rename them"
)]
pub fn resolve_in(
    graphics: &Graphics,
    hidden: &BTreeSet<CategoryId>,
    scale: Scale,
    categories: &CategoryTable,
    objects: &ObjectStyles,
    lines: &LineStyles,
    category: CategoryId,
    subcategory: Option<CategoryId>,
    edge: Edge,
) -> Result<Resolved, ViewError> {
    let found = resolve_rules(
        graphics,
        scale,
        categories,
        objects,
        lines,
        category,
        subcategory,
        edge,
    )?;
    if hidden.contains(&category) || subcategory.is_some_and(|sub| hidden.contains(&sub)) {
        return Ok(Resolved {
            style: RuleStyle {
                shown: false,
                ..found.style
            },
            ..found
        });
    }
    Ok(found)
}

/// `resolved` with the parts `style` names put over it, as an element's own look in a view: the
/// pen's printed width is worked out again at the view's `scale`. A pen the pen table of the
/// category's `kind` does not have is left out, so an override never stops a drawing.
///
/// # Errors
/// [`ViewError::Style`] when the pen table cannot give the pen's width.
pub fn adjust(
    resolved: &Resolved,
    style: &ElementStyle,
    lines: &LineStyles,
    kind: CategoryKind,
    scale: Scale,
) -> Result<Resolved, ViewError> {
    let pen = style.pen.filter(|number| pen_exists(lines, kind, *number));
    let own = ElementStyle {
        pen,
        pattern: style
            .pattern
            .filter(|pattern| lines.patterns.get(*pattern).is_some()),
        ..style.clone()
    };
    let changed = own.over(resolved.style.clone());
    let width = if changed.pen == resolved.style.pen {
        resolved.width
    } else {
        width(lines, kind, changed.pen, scale)?
    };
    Ok(Resolved {
        style: changed,
        width,
        origin: resolved.origin,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "the inputs of the one question, as resolve asks it"
)]
fn resolve_rules(
    graphics: &Graphics,
    scale: Scale,
    categories: &CategoryTable,
    objects: &ObjectStyles,
    lines: &LineStyles,
    category: CategoryId,
    subcategory: Option<CategoryId>,
    edge: Edge,
) -> Result<Resolved, ViewError> {
    let top = categories
        .get(category)
        .ok_or(ViewError::UnknownCategory(category))?;
    if top.parent.is_some() {
        return Err(ViewError::NotTopLevel(category));
    }
    let target = match subcategory {
        Some(id) => {
            let sub = categories.get(id).ok_or(ViewError::UnknownCategory(id))?;
            if sub.parent != Some(category) {
                return Err(ViewError::NotASubcategory {
                    subcategory: id,
                    category,
                });
            }
            sub
        }
        None => top,
    };
    if edge == Edge::Cut && !target.cuttable {
        return Err(ViewError::CutOnUncuttable(target.id));
    }
    let ruled = subcategory
        .and_then(|id| graphics.style(id, edge).map(|s| (s, Origin::Subcategory)))
        .or_else(|| {
            graphics
                .style(category, edge)
                .map(|s| (s, Origin::Category))
        });
    if let Some((style, origin)) = ruled {
        return Ok(Resolved {
            width: width(lines, target.kind, style.pen, scale)?,
            style: style.clone(),
            origin,
        });
    }
    let own = objects
        .style_of(categories, target.id)
        .map_err(|why| ViewError::Style(why.to_string()))?;
    let pen = match edge {
        Edge::Projected => own.projection_pen,
        Edge::Cut => own.cut_pen.ok_or(ViewError::CutOnUncuttable(target.id))?,
    };
    Ok(Resolved {
        width: width(lines, target.kind, pen, scale)?,
        style: RuleStyle {
            pen,
            pattern: own.pattern,
            colour: own.colour,
            fill: None,
            halftone: false,
            transparency: 0,
            shown: true,
        },
        origin: Origin::Project,
    })
}
