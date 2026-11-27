//! A curtain wall in plan (job 14.1, CON-A4-F6-P1, piece A84): every solid of a filled
//! `wall.curtain` recipe as a quadrilateral on the kernel's grid and the heights it rises between,
//! and every door panel where its family stands. Both builders ([`crate::kernel`] and
//! [`crate::stand_in`]) read a curtain wall through this one module, so they cannot disagree.
//!
//! The layout is construct-curtain's ([`construct_curtain::Grid`]): each solid a box in the wall's
//! frame, along its line from its start, across it toward its exterior (the right walking from the
//! start to the end) and up from its base, exactly. Here a frame point `(u, w)` goes into the plan
//! as `start + A(u) + R(w)`: `A(u)` is `(end - start) * u / L`, `L` the line's length to the nearest
//! step (so `A(L)` is exactly `end - start`), and `R(w)` is [`crate::wall_plan`]'s offset across,
//! each coordinate of each rounded to the nearest step, a half away from zero. That is each corner
//! rounded once along and once across: two solids that share a place along the wall, or a face
//! across it, share its corners bit for bit, and every box's plan is a parallelogram on grid
//! points. A wall along an axis has no rounding at all. Each solid rises straight up between two
//! heights, exactly; the base is the element's frame's `z = 0`.
//!
//! A door panel ([`PlanDoor`]): its family stands on its panel's foot, its origin at the panel's
//! middle on the wall's line, turned by quarter turns so that its `+y` points out of the wall's
//! exterior and its `+x` a quarter turn clockwise from that (a turn, never a mirror), as a family
//! hosted in a wall stands ([`crate::hosting`]); its family's `width` and `height` parameters are
//! set to its panel's. Only a curtain wall along x or y holds one: a family is turned by quarter
//! turns only (`DoorAtAngle`).
//!
//! Depends on construct-model (recipes), construct-params (lengths), construct-curtain (the layout)
//! and construct-families (a door's turn). Nothing here is interim: the rounding rule is the
//! kernel's grid's, and an exact kernel takes the same corners.

use crate::wall_plan::Plan;
use construct_curtain::{DoorPanel, Filled, Grid, Solid};
use construct_families::Turn;
use construct_model::geometry::{GeometryError, Recipe};
use construct_params::Length;

/// The operation of a curtain wall.
pub const OP: &str = construct_curtain::OP;

/// The key of the family parameter a door panel's width is set to.
pub const DOOR_WIDTH: &str = "width";

/// The key of the family parameter a door panel's height is set to.
pub const DOOR_HEIGHT: &str = "height";

fn fault(code: &str, message: impl Into<String>) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn too_big() -> GeometryError {
    fault("TooBig", "a length is beyond the kernel's grid")
}

/// Whether `recipe` is a curtain wall's.
#[must_use]
pub fn is_curtain(recipe: &Recipe) -> bool {
    construct_curtain::is_curtain(recipe)
}

/// One solid in plan: its key, what it does, its material, its four corners anticlockwise seen
/// from above (grid steps, the element's frame) and the heights it rises between (grid steps).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanSolid {
    /// Its key within the wall.
    pub key: String,
    /// What it is: `panel.glazed`, `panel.solid` or `mullion`.
    pub function: &'static str,
    /// Its material's ID.
    pub material: String,
    /// Its plan, anticlockwise from above.
    pub corners: [[i64; 2]; 4],
    /// From and to, up.
    pub z: [i64; 2],
}

/// One door panel, as its family is placed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanDoor {
    /// Its key within the wall.
    pub key: String,
    /// The family and type.
    pub door: DoorPanel,
    /// The width its family is set to.
    pub width: Length,
    /// The height its family is set to.
    pub height: Length,
    /// Where its family's origin stands: grid steps, the element's frame.
    pub origin: [i64; 3],
    /// How it is turned.
    pub turn: Turn,
}

/// A curtain wall in plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurtainPlan {
    /// Its line.
    pub line: Plan,
    /// Its layout.
    pub grid: Grid,
    /// Its solids, in the layout's order.
    pub solids: Vec<PlanSolid>,
    /// Its door panels.
    pub doors: Vec<PlanDoor>,
    /// The box round everything it holds in its frame, in plan, anticlockwise from above.
    pub outline: [[i64; 2]; 4],
    /// Its height, in grid steps.
    pub height: i64,
}

/// The nearest whole number to `a / b`, a half away from zero, for `b` above zero.
fn nearest(a: i128, b: i128) -> Option<i64> {
    let twice = a.checked_mul(2)?;
    let q = if a >= 0 {
        twice.checked_add(b)?.checked_div(b.checked_mul(2)?)?
    } else {
        twice.checked_sub(b)?.checked_div(b.checked_mul(2)?)?
    };
    i64::try_from(q).ok()
}

/// How the wall's frame goes into the plan.
struct Frame {
    line: Plan,
    length: i64,
}

impl Frame {
    /// `A(u)`: the point `u` along the line, from its start, before the start is added.
    fn along(&self, u: Length) -> Result<[i64; 2], GeometryError> {
        let [dx, dy] = self.line.direction();
        let (u, l) = (i128::from(u.steps()), i128::from(self.length));
        let a = |d: i64| {
            i128::from(d)
                .checked_mul(u)
                .and_then(|n| nearest(n, l))
                .ok_or_else(too_big)
        };
        Ok([a(dx)?, a(dy)?])
    }

    /// The plan point of the frame point `u` along and `w` across.
    fn point(&self, u: Length, w: Length) -> Result<[i64; 2], GeometryError> {
        let [ax, ay] = self.along(u)?;
        let [rx, ry] = self.line.across(w.steps())?;
        let [sx, sy] = self.line.start();
        let add3 = |s: i64, a: i64, r: i64| s.checked_add(a)?.checked_add(r);
        Ok([
            add3(sx, ax, rx).ok_or_else(too_big)?,
            add3(sy, ay, ry).ok_or_else(too_big)?,
        ])
    }

    /// The plan of a box from `along` and `across`, anticlockwise seen from above.
    fn quad(
        &self,
        along: [Length; 2],
        across: [Length; 2],
    ) -> Result<[[i64; 2]; 4], GeometryError> {
        let [u0, u1] = along;
        let [w0, w1] = across;
        let corners = [
            self.point(u0, w0)?,
            self.point(u1, w0)?,
            self.point(u1, w1)?,
            self.point(u0, w1)?,
        ];
        Ok(anticlockwise(corners))
    }
}

/// Four corners in the order that turns anticlockwise seen from above.
fn anticlockwise(corners: [[i64; 2]; 4]) -> [[i64; 2]; 4] {
    let [p0, p1, _, p3] = corners;
    let (ax, ay) = (
        i128::from(p1[0]).saturating_sub(i128::from(p0[0])),
        i128::from(p1[1]).saturating_sub(i128::from(p0[1])),
    );
    let (bx, by) = (
        i128::from(p3[0]).saturating_sub(i128::from(p0[0])),
        i128::from(p3[1]).saturating_sub(i128::from(p0[1])),
    );
    let turn = ax.saturating_mul(by).saturating_sub(ay.saturating_mul(bx));
    if turn >= 0 {
        corners
    } else {
        let [c0, c1, c2, c3] = corners;
        [c0, c3, c2, c1]
    }
}

/// A grid point's coordinate from a length.
fn steps(l: Length) -> i64 {
    l.steps()
}

impl CurtainPlan {
    /// A curtain wall's plan, read from its filled recipe.
    ///
    /// # Errors
    ///
    /// `BadCurtain` for a recipe or a layout construct-curtain refuses, in its words; `DoorAtAngle`
    /// for a door panel in a wall that runs at an angle; `ZeroLength` and `TooBig` for a line the
    /// grid cannot hold.
    pub fn of(recipe: &Recipe) -> Result<CurtainPlan, GeometryError> {
        let bad = |e: construct_curtain::CurtainError| fault("BadCurtain", e.to_string());
        let filled = Filled::of(recipe).map_err(bad)?;
        let grid = filled.curtain.grid().map_err(bad)?;
        let [start, end] = [filled.start, filled.end].map(|[x, y]| [steps(x), steps(y)]);
        let line = Plan::new(start, end)?;
        let frame = Frame {
            line,
            length: steps(filled.curtain.length()),
        };
        let solids = grid
            .solids()
            .map_err(bad)?
            .iter()
            .map(|solid: &Solid| {
                Ok(PlanSolid {
                    key: solid.key.clone(),
                    function: solid.part.word(),
                    material: solid.material.as_str().to_owned(),
                    corners: frame.quad(solid.frame.along, solid.frame.across)?,
                    z: solid.frame.up.map(steps),
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let places = grid.doors().map_err(bad)?;
        let mut doors = Vec::with_capacity(places.len());
        if !places.is_empty() {
            let axis = line.axis().ok_or_else(|| {
                fault(
                    "DoorAtAngle",
                    "a door panel stands only in a curtain wall along x or y, until a family \
                     can be turned by any angle",
                )
            })?;
            let [ux, uy] = axis.unit();
            // +y out of the exterior, the line's right: (uy, -ux); +x then takes (-ux, -uy).
            let neg = |v: i64| v.checked_neg().ok_or_else(too_big);
            let turn = Turn::taking_x_to([neg(ux)?, neg(uy)?]).ok_or_else(too_big)?;
            for place in places {
                let [x, y] = frame.point(place.middle, Length::from_steps(0))?;
                doors.push(PlanDoor {
                    key: place.key,
                    door: place.door,
                    width: place.width,
                    height: place.height,
                    origin: [x, y, steps(place.foot)],
                    turn,
                });
            }
        }
        // The box round every solid and door across, for the whole length.
        let mut low = Length::from_steps(0);
        let mut high = Length::from_steps(0);
        for solid in grid.solids().map_err(bad)? {
            let [a, b] = solid.frame.across;
            low = low.min(a);
            high = high.max(b);
        }
        let outline = frame.quad(
            [Length::from_steps(0), filled.curtain.length()],
            [low, high],
        )?;
        Ok(CurtainPlan {
            line,
            grid,
            solids,
            doors,
            outline,
            height: steps(filled.curtain.height()),
        })
    }
}
