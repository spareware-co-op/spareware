//! Placed detail components: one at a position, turned by any angle; and one repeated along a line.
//!
//! **A placed component** ([`DetailComponent`]) names a recipe of the library by its key, a
//! position and a turn. The turn is an angle in whole angle units, held modulo a full turn, and any
//! angle is allowed: every point is the exact turn of the recipe's own point rounded once to the
//! grid (the modify tools' rule, main's ruling of 28 Nov 2026), so quarter turns are exact and a
//! 37.5 degree turn is within half a step of the true place on each axis.
//!
//! **A repeating component** ([`DetailRepeat`]) is a component set at every `spacing` along a line
//! from its start, the first at the start, the last at or before the end: along 3 m at 600 mm
//! spacing there are six. Each copy is turned to lie along the line (the component's own x runs
//! along it) and then by an extra angle. A copy's place is the nearest grid step to its exact
//! place along the line, worked out in whole numbers of any size, so copies never drift however
//! long or slanted the line is. No more than [`MOST_REPEATS`] copies are made at once.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "the only unchecked operators here are on num-bigint whole numbers of any size, which cannot overflow, with a divisor above zero; every fixed-width step is a checked method"
)]
use crate::component::{ComponentRecipe, Library, Placed, place};
use crate::error::DetailError;
use crate::num::{add, at, from_steps, point_in_grid, too_big};
use crate::record::{Props, Reader};
use construct_categories::DETAIL_COMPONENTS;
use construct_grids::Point;
use construct_model::element::Element;
use construct_modify::Turn;
use construct_modify::exact::FULL;
use construct_params::{Angle, Length, Value};
use kit_core::Id;
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

/// The `kind` of a placed component record.
pub const COMPONENT_KIND: &str = "detail.component";
/// The `kind` of a repeating component record.
pub const REPEAT_KIND: &str = "detail.repeat";
/// The name a placed component record carries.
pub const COMPONENT_NAME: &str = "Detail component";
/// The name a repeating component record carries.
pub const REPEAT_NAME: &str = "Repeating detail component";
/// The recipe's key.
pub const RECIPE_KEY: &str = "@detail.component.recipe";
/// The position.
pub const POSITION_KEY: &str = "@detail.component.position";
/// The turn, a stored angle.
pub const ROTATION_KEY: &str = "@detail.component.rotation";
/// A repeat's first point.
pub const FROM_KEY: &str = "@detail.repeat.from";
/// A repeat's last possible point.
pub const TO_KEY: &str = "@detail.repeat.to";
/// A repeat's spacing, a stored length.
pub const SPACING_KEY: &str = "@detail.repeat.spacing";
/// The most copies a repeat makes.
pub const MOST_REPEATS: u64 = 2_000;

/// An angle held modulo a full turn: `0 <= units < FULL`.
#[must_use]
pub fn normalised(angle: Angle) -> Angle {
    Angle::from_units(angle.units().rem_euclid(FULL))
}

/// A component placed in a view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailComponent {
    id: Id,
    view: Id,
    recipe: String,
    position: Point,
    rotation: Angle,
}

impl DetailComponent {
    /// A component of the library's recipe `recipe`, at `position`, turned anticlockwise by
    /// `rotation` (any angle).
    ///
    /// # Errors
    /// [`DetailError::UnknownComponent`] for a key the library lacks; [`DetailError::OutOfRange`]
    /// for a position, or a component placed so that a point passes the kernel's grid.
    pub fn new(
        id: Id,
        view: Id,
        library: &Library,
        recipe: &str,
        position: Point,
        rotation: Angle,
    ) -> Result<Self, DetailError> {
        let found = library
            .get(recipe)
            .ok_or_else(|| DetailError::UnknownComponent(recipe.to_owned()))?;
        point_in_grid(position)?;
        let placed = Self {
            id,
            view,
            recipe: recipe.to_owned(),
            position,
            rotation: normalised(rotation),
        };
        placed.draw(found)?;
        Ok(placed)
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The view it is in.
    #[must_use]
    pub fn view(&self) -> &Id {
        &self.view
    }

    /// The key of its recipe.
    #[must_use]
    pub fn recipe(&self) -> &str {
        &self.recipe
    }

    /// Where its origin is.
    #[must_use]
    pub fn position(&self) -> Point {
        self.position
    }

    /// How far it is turned anticlockwise, in `[0, 360)` degrees.
    #[must_use]
    pub fn rotation(&self) -> Angle {
        self.rotation
    }

    /// The component moved and turned.
    ///
    /// # Errors
    /// As [`DetailComponent::new`].
    pub fn with_placement(
        self,
        library: &Library,
        position: Point,
        rotation: Angle,
    ) -> Result<Self, DetailError> {
        Self::new(
            self.id,
            self.view,
            library,
            &self.recipe,
            position,
            rotation,
        )
    }

    /// What it draws: its recipe placed.
    ///
    /// # Errors
    /// [`DetailError::OutOfRange`] when a point would pass the kernel's grid.
    pub fn draw(&self, recipe: &ComponentRecipe) -> Result<Placed, DetailError> {
        place(recipe, self.position, &Turn::of_angle(self.rotation))
    }

    /// The component as an element record of `document` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = Props::new(DETAIL_COMPONENTS.0, &self.view);
        props.text(RECIPE_KEY, &self.recipe);
        props.point(POSITION_KEY, self.position);
        props.text(ROTATION_KEY, &Value::Angle(self.rotation).stored());
        props.finish(
            &self.id,
            document,
            COMPONENT_KIND,
            COMPONENT_NAME,
            Vec::new(),
        )
    }

    /// Reads a component from its record; anything [`DetailComponent::to_record`] would not write
    /// is refused.
    ///
    /// # Errors
    /// [`DetailError::Malformed`], naming the record.
    pub fn from_record(record: &Element, library: &Library) -> Result<Self, DetailError> {
        let read = Reader::new(record, COMPONENT_KIND, "detail component")?;
        let angle = read.text(ROTATION_KEY)?;
        let Some(Value::Angle(rotation)) = Value::from_stored(angle) else {
            return Err(read.bad("holds a rotation that is not a stored angle"));
        };
        let component = Self::new(
            record.id.clone(),
            read.view()?,
            library,
            read.text(RECIPE_KEY)?,
            read.point(POSITION_KEY)?,
            rotation,
        )
        .map_err(|e| read.bad(&e.to_string()))?;
        read.same_as(&component.to_record(&record.document))?;
        Ok(component)
    }
}

/// A component repeated along a line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailRepeat {
    id: Id,
    view: Id,
    recipe: String,
    from: Point,
    to: Point,
    spacing: Length,
    rotation: Angle,
}

/// The nearest whole number to `k_s * d / sqrt(n)`, a half away from zero: how far along one axis
/// a copy `k_s` steps along a line lies, where `d` is that axis's part of the line's vector and `n`
/// its squared length. Whole numbers of any size throughout. With `a = k_s * d` and `x = |a| /
/// sqrt(n)`, the nearest whole number is `floor(x + 1/2) = floor((floor(2x) + 1) / 2)`, and
/// `floor(2x) = floor(sqrt(4 a^2 / n)) = isqrt(floor(4 a^2 / n))`. A square root of a fraction
/// exact to the last step is what no library gives, so it is written here.
pub(crate) fn along(k_s: &BigInt, d: i128, n: &BigInt) -> Result<i64, DetailError> {
    if n.is_zero() {
        return Err(DetailError::BadSpacing(
            "The line has no length.".to_owned(),
        ));
    }
    let a: BigInt = k_s * BigInt::from(d);
    let four_a2: BigInt = &a * &a * 4;
    let q = four_a2 / n;
    let root = q.sqrt();
    let nearest: BigInt = (root + 1) / 2;
    let signed = if a.is_negative() { -nearest } else { nearest };
    signed.to_i64().ok_or_else(too_big)
}

impl DetailRepeat {
    /// A repeat of `recipe` along the line from `from` to `to`, every `spacing`, each copy then
    /// turned by `rotation` more than the line's direction.
    ///
    /// # Errors
    /// [`DetailError::UnknownComponent`]; [`DetailError::ZeroLength`] for a line of no length;
    /// [`DetailError::BadSpacing`] for a spacing that is not above zero; [`DetailError::TooMany`]
    /// above [`MOST_REPEATS`] copies; [`DetailError::OutOfRange`] past the kernel's grid.
    pub fn new(
        id: Id,
        view: Id,
        library: &Library,
        recipe: &str,
        (from, to): (Point, Point),
        spacing: Length,
        rotation: Angle,
    ) -> Result<Self, DetailError> {
        if library.get(recipe).is_none() {
            return Err(DetailError::UnknownComponent(recipe.to_owned()));
        }
        point_in_grid(from)?;
        point_in_grid(to)?;
        if from == to {
            return Err(DetailError::ZeroLength);
        }
        if spacing.steps() <= 0 {
            return Err(DetailError::BadSpacing(
                "A repeating component's spacing must be more than zero.".to_owned(),
            ));
        }
        let repeat = Self {
            id,
            view,
            recipe: recipe.to_owned(),
            from,
            to,
            spacing,
            rotation: normalised(rotation),
        };
        let count = repeat.count()?;
        if count > MOST_REPEATS {
            return Err(DetailError::TooMany(count));
        }
        Ok(repeat)
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The view it is in.
    #[must_use]
    pub fn view(&self) -> &Id {
        &self.view
    }

    /// The key of its recipe.
    #[must_use]
    pub fn recipe(&self) -> &str {
        &self.recipe
    }

    /// The line it runs along: its first point and its far end.
    #[must_use]
    pub fn line(&self) -> (Point, Point) {
        (self.from, self.to)
    }

    /// The distance between copies, along the line.
    #[must_use]
    pub fn spacing(&self) -> Length {
        self.spacing
    }

    /// The extra turn of every copy, beyond the line's direction.
    #[must_use]
    pub fn rotation(&self) -> Angle {
        self.rotation
    }

    /// The repeat along another line.
    ///
    /// # Errors
    /// As [`DetailRepeat::new`].
    pub fn with_line(self, library: &Library, line: (Point, Point)) -> Result<Self, DetailError> {
        Self::new(
            self.id,
            self.view,
            library,
            &self.recipe,
            line,
            self.spacing,
            self.rotation,
        )
    }

    /// The repeat with another spacing and extra turn.
    ///
    /// # Errors
    /// As [`DetailRepeat::new`].
    pub fn with_spacing(
        self,
        library: &Library,
        spacing: Length,
        rotation: Angle,
    ) -> Result<Self, DetailError> {
        let line = (self.from, self.to);
        Self::new(
            self.id,
            self.view,
            library,
            &self.recipe,
            line,
            spacing,
            rotation,
        )
    }

    fn vector(&self) -> Result<[i128; 2], DetailError> {
        let (a, b) = (at(self.from), at(self.to));
        Ok([
            b[0].checked_sub(a[0]).ok_or_else(too_big)?,
            b[1].checked_sub(a[1]).ok_or_else(too_big)?,
        ])
    }

    fn length_squared(&self) -> Result<BigInt, DetailError> {
        let [dx, dy] = self.vector()?;
        Ok(BigInt::from(dx) * BigInt::from(dx) + BigInt::from(dy) * BigInt::from(dy))
    }

    /// How many copies it makes: one at the start and one every spacing along the line while the
    /// line is long enough, `floor(length / spacing) + 1`, exactly (the floor of a square root is
    /// the square root of the floor).
    ///
    /// # Errors
    /// [`DetailError::OutOfRange`] for a count that does not fit.
    pub fn count(&self) -> Result<u64, DetailError> {
        let whole = self.length_squared()?.sqrt();
        let per = BigInt::from(self.spacing.steps());
        let n: BigInt = whole / per + 1;
        n.to_u64().ok_or_else(too_big)
    }

    /// The nearest grid points to the copies' exact places, in order from the start.
    ///
    /// # Errors
    /// [`DetailError::OutOfRange`] or [`DetailError::TooMany`].
    pub fn positions(&self) -> Result<Vec<Point>, DetailError> {
        let count = self.count()?;
        if count > MOST_REPEATS {
            return Err(DetailError::TooMany(count));
        }
        let n = self.length_squared()?;
        let [dx, dy] = self.vector()?;
        let origin = at(self.from);
        let mut out = Vec::new();
        for k in 0..count {
            let k_s = BigInt::from(k) * BigInt::from(self.spacing.steps());
            let x = add(origin[0], i128::from(along(&k_s, dx, &n)?))?;
            let y = add(origin[1], i128::from(along(&k_s, dy, &n)?))?;
            out.push(from_steps([
                i64::try_from(x).map_err(|_| too_big())?,
                i64::try_from(y).map_err(|_| too_big())?,
            ]));
        }
        Ok(out)
    }

    /// What every copy draws, in order from the start.
    ///
    /// # Errors
    /// [`DetailError::UnknownComponent`] when the library lacks the recipe; what
    /// [`DetailRepeat::positions`] refuses; [`DetailError::OutOfRange`] past the grid.
    pub fn draw(&self, library: &Library) -> Result<Vec<Placed>, DetailError> {
        let recipe = library
            .get(&self.recipe)
            .ok_or_else(|| DetailError::UnknownComponent(self.recipe.clone()))?;
        let [dx, dy] = self.vector()?;
        let to_i64 = |v: i128| i64::try_from(v).map_err(|_| too_big());
        let along_line = Turn::between([1, 0], [to_i64(dx)?, to_i64(dy)?])
            .map_err(|e| DetailError::OutOfRange(e.to_string()))?;
        let turn = along_line.then(&Turn::of_angle(self.rotation));
        self.positions()?
            .into_iter()
            .map(|position| place(recipe, position, &turn))
            .collect()
    }

    /// The repeat as an element record of `document` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = Props::new(DETAIL_COMPONENTS.0, &self.view);
        props.text(RECIPE_KEY, &self.recipe);
        props.point(FROM_KEY, self.from);
        props.point(TO_KEY, self.to);
        props.length(SPACING_KEY, self.spacing);
        props.text(ROTATION_KEY, &Value::Angle(self.rotation).stored());
        props.finish(&self.id, document, REPEAT_KIND, REPEAT_NAME, Vec::new())
    }

    /// Reads a repeat from its record; anything [`DetailRepeat::to_record`] would not write is
    /// refused.
    ///
    /// # Errors
    /// [`DetailError::Malformed`], naming the record.
    pub fn from_record(record: &Element, library: &Library) -> Result<Self, DetailError> {
        let read = Reader::new(record, REPEAT_KIND, "repeating detail component")?;
        let angle = read.text(ROTATION_KEY)?;
        let Some(Value::Angle(rotation)) = Value::from_stored(angle) else {
            return Err(read.bad("holds a rotation that is not a stored angle"));
        };
        let repeat = Self::new(
            record.id.clone(),
            read.view()?,
            library,
            read.text(RECIPE_KEY)?,
            (read.point(FROM_KEY)?, read.point(TO_KEY)?),
            read.length(SPACING_KEY)?,
            rotation,
        )
        .map_err(|e| read.bad(&e.to_string()))?;
        read.same_as(&repeat.to_record(&record.document))?;
        Ok(repeat)
    }
}
