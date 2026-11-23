//! A recipe's 2D symbols drawn at one type's values: each symbol's sketch solved exactly as a
//! step's sketch is (`sketch.rs`: from where it is stored, the least change, on the grid, the
//! same on every run), and each edge it names read back as the solved line, arc or circle.
//!
//! The geometry is in the sketch's own two coordinates (a plan symbol's x and y, an elevation
//! symbol's x and z: FORMAT.md, "Coordinates"), as `construct_params::Length` grid steps; nothing
//! is held as millimetres or as a double. Every symbol is given, with its condition (`when`) as
//! written: whether it is drawn is the view's question, since a condition is usually on an
//! instance parameter (a door's swing), which a type does not set. Each sketch is solved once
//! however many symbols draw from it, within the same [`Limits`] as a solid evaluation (solver
//! steps and formula text are the counts a symbol uses).

use super::error::{EvalError, Place, Reason};
use super::limits::{Limits, Meter, Used};
use super::sketch::{Drawn, resolve};
use super::values::{TypeValues, work_out};
use crate::check::Sort;
use crate::model::{Condition, Key, Recipe, View, Visibility};
use construct_categories::CategoryId;
use construct_params::Length;
use std::collections::BTreeMap;

/// A point of a symbol, in its sketch's two coordinates.
pub type Point2 = [Length; 2];

/// One edge of a symbol, solved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edge {
    /// A straight line.
    Line {
        /// The sketch line's key.
        key: Key,
        /// Where it starts.
        start: Point2,
        /// Where it ends.
        end: Point2,
    },
    /// Part of a circle, anticlockwise about its centre from its start to its end.
    Arc {
        /// The sketch arc's key.
        key: Key,
        /// Its centre.
        centre: Point2,
        /// Where it starts.
        start: Point2,
        /// Where it ends.
        end: Point2,
        /// Its radius.
        radius: Length,
    },
    /// A whole circle.
    Circle {
        /// The sketch circle's key.
        key: Key,
        /// Its centre.
        centre: Point2,
        /// Its radius.
        radius: Length,
    },
}

impl Edge {
    /// The sketch item's key.
    #[must_use]
    pub fn key(&self) -> &Key {
        match self {
            Self::Line { key, .. } | Self::Arc { key, .. } | Self::Circle { key, .. } => key,
        }
    }
}

/// One of a family's symbols, drawn at one type's values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawnSymbol {
    /// The symbol's key.
    pub symbol: Key,
    /// The kind of view it is drawn in.
    pub view: View,
    /// The subcategory it is drawn with.
    pub subcategory: CategoryId,
    /// Which detail levels show it.
    pub visibility: Visibility,
    /// Drawn only while this holds; always when `None`.
    pub when: Option<Condition>,
    /// Its edges, in the order the symbol names them.
    pub edges: Vec<Edge>,
}

/// A recipe's symbols drawn at one type's values, with what drawing them used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawnSymbols {
    /// Every symbol, in recipe order.
    pub symbols: Vec<DrawnSymbol>,
    /// What drawing them used, counted as evaluation's limits count.
    pub used: Used,
}

/// Draws every symbol of `recipe` for the type whose values are `values`, within the default
/// limits ([`Limits::DEFAULT`]).
///
/// # Errors
///
/// An [`EvalError`] naming the place and the [`Reason`]: a value the type may not set or that is
/// not its parameter's, a formula refused, a sketch the solver refuses at these values, an edge
/// the sketch does not have, or a limit reached.
pub fn symbols(recipe: &Recipe, values: &TypeValues) -> Result<DrawnSymbols, EvalError> {
    symbols_within(recipe, values, Limits::DEFAULT)
}

/// As [`symbols()`], within `limits`.
///
/// # Errors
///
/// As [`symbols()`].
pub fn symbols_within(
    recipe: &Recipe,
    values: &TypeValues,
    limits: Limits,
) -> Result<DrawnSymbols, EvalError> {
    let mut meter = Meter::new(limits);
    let worked = work_out(recipe, values, &mut meter)?;
    let mut sketches: BTreeMap<Key, Drawn> = BTreeMap::new();
    let mut out = Vec::with_capacity(recipe.symbols.len());
    for symbol in &recipe.symbols {
        if !sketches.contains_key(&symbol.sketch) {
            let drawn = resolve(recipe, &symbol.sketch, &worked, &mut meter)?;
            sketches.insert(symbol.sketch.clone(), drawn);
        }
        let drawn = sketches.get(&symbol.sketch).ok_or_else(|| {
            EvalError::new(
                Place::Sketch(symbol.sketch.clone()),
                Reason::Missing {
                    sort: Sort::Sketch,
                    key: symbol.sketch.as_str().to_owned(),
                },
            )
        })?;
        let edges = symbol
            .edges
            .iter()
            .map(|key| {
                drawn.edge(key).map_err(|reason| {
                    EvalError::new(
                        Place::Item {
                            sketch: symbol.sketch.clone(),
                            item: key.clone(),
                        },
                        reason,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        out.push(DrawnSymbol {
            symbol: symbol.key.clone(),
            view: symbol.view,
            subcategory: symbol.subcategory,
            visibility: symbol.visibility,
            when: symbol.when.clone(),
            edges,
        });
    }
    Ok(DrawnSymbols {
        symbols: out,
        used: meter.used(),
    })
}
