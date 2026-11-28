//! A ceiling's grid pattern (a suspended ceiling's tiles, for example 600 by 1200) and the exact
//! plane geometry its count and its placing need: where a point stands against a boundary, an
//! interior point of a boundary, and how many tiles of a grid lie whole or cut within it.
//!
//! The pattern: a tile `width` along the pattern's own x axis and `depth` along its y axis, grid
//! lines through its `origin`, and its `angle` in whole degrees from 0 up to (not including)
//! 180, anticlockwise from the model's x axis. Its stored form is in FORMAT.md beside this
//! crate: `{"width":<steps>,"depth":<steps>,"origin":[x,y],"angle":<degrees>}`.
//!
//! Counting is exact whole-number arithmetic (128 bits over the grid's steps, no float): a tile
//! is whole when no edge of the boundary crosses its open interior and its centre is in the
//! region (the outer loop less its holes), and cut when an edge does cross it. Tiles that only
//! touch the region count for neither.
//!
//! Interim: only an angle that is a multiple of 90 degrees (0 or 90) is counted. A turned grid's
//! lines are not on the length grid's rationals (their direction is a sine and a cosine), so its
//! count needs the kernel's exact rotation; [`Grid::tiles`] refuses it by name
//! ([`SlabError::BadGrid`]) until then, and the pattern is kept, drawn by the page and handed on
//! as it is. This is a row of the interim register: the kernel's exact rotation of a plane
//! pattern replaces it, with the same tests.

use crate::boundary::Boundary;
use crate::error::SlabError;
use construct_params::Length;
use construct_solver::Point;
use serde_json::{Value, json};

/// The most tiles whose bounding grid [`Grid::tiles`] will walk.
pub const MOST_TILES: u64 = 1_000_000;

/// A tile grid on a ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    width: Length,
    depth: Length,
    origin: Point,
    angle: u16,
}

/// How many tiles of a grid lie in a region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tiles {
    /// Tiles inside the region entirely.
    pub whole: u64,
    /// Tiles the region's boundary crosses, which are cut to fit.
    pub cut: u64,
}

impl Tiles {
    /// Every tile that has any part in the region.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.whole.saturating_add(self.cut)
    }
}

impl Grid {
    /// A grid of tiles `width` by `depth`, with a line through `origin`, turned `degrees`.
    ///
    /// # Errors
    /// [`SlabError::BadGrid`] for a tile that is not thicker than nothing or is over 100 m, an
    /// angle outside 0 up to 180, or an origin beyond the kernel's grid.
    pub fn new(
        width: Length,
        depth: Length,
        origin: Point,
        degrees: i64,
    ) -> Result<Self, SlabError> {
        let most = Length::from_mm(100_000).steps();
        for (what, side) in [("width", width), ("depth", depth)] {
            if side.steps() <= 0 || side.steps() > most {
                return Err(SlabError::BadGrid(format!(
                    "a tile's {what} is more than nothing and at most 100 m"
                )));
            }
        }
        let angle = u16::try_from(degrees)
            .ok()
            .filter(|a| *a < 180)
            .ok_or_else(|| {
                SlabError::BadGrid("the angle is a whole number of degrees from 0 up to 180".into())
            })?;
        let limit = 1_099_511_627_775_i64;
        if origin.x.saturating_abs() > limit || origin.y.saturating_abs() > limit {
            return Err(SlabError::BadGrid(
                "the origin is beyond the kernel's grid".into(),
            ));
        }
        Ok(Self {
            width,
            depth,
            origin,
            angle,
        })
    }

    /// A tile's size along the pattern's x axis.
    #[must_use]
    pub fn width(&self) -> Length {
        self.width
    }

    /// A tile's size along the pattern's y axis.
    #[must_use]
    pub fn depth(&self) -> Length {
        self.depth
    }

    /// The point a grid line goes through.
    #[must_use]
    pub fn origin(&self) -> Point {
        self.origin
    }

    /// The angle in whole degrees.
    #[must_use]
    pub fn angle(&self) -> u16 {
        self.angle
    }

    /// The same grid with another origin.
    #[must_use]
    pub fn with_origin(self, origin: Point) -> Self {
        Self { origin, ..self }
    }

    /// Its stored form.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({
            "width": self.width.steps(),
            "depth": self.depth.steps(),
            "origin": [self.origin.x, self.origin.y],
            "angle": self.angle,
        })
    }

    /// A grid from its stored form.
    ///
    /// # Errors
    /// [`SlabError::Malformed`] for anything not as written above, then any refusal of
    /// [`Grid::new`].
    pub fn from_value(value: &Value) -> Result<Self, SlabError> {
        let bad = |why: &str| SlabError::Malformed(format!("a ceiling's grid {why}"));
        let object = value.as_object().ok_or_else(|| bad("is an object"))?;
        if object.len() != 4 {
            return Err(bad("holds its width, depth, origin and angle"));
        }
        let number = |key: &str| {
            object
                .get(key)
                .and_then(Value::as_i64)
                .ok_or_else(|| bad("holds whole numbers"))
        };
        let origin = match object
            .get("origin")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
        {
            Some([x, y]) => match (x.as_i64(), y.as_i64()) {
                (Some(x), Some(y)) => Point { x, y },
                _ => return Err(bad("has an origin of two whole numbers")),
            },
            _ => return Err(bad("has an origin of two whole numbers")),
        };
        Self::new(
            Length::from_steps(number("width")?),
            Length::from_steps(number("depth")?),
            origin,
            number("angle")?,
        )
    }

    /// How many of its tiles lie in the region of `boundary`: whole, and cut.
    ///
    /// # Errors
    /// [`SlabError::BadGrid`] for a turned grid (see the module header) and for a region with
    /// more than [`MOST_TILES`] tiles in its bounding box; [`SlabError::OutOfRange`] when a
    /// number does not fit.
    pub fn tiles(&self, boundary: &Boundary) -> Result<Tiles, SlabError> {
        if !self.angle.is_multiple_of(90) {
            return Err(SlabError::BadGrid(
                "a turned grid's tiles are not counted exactly yet: only 0 and 90 degrees are"
                    .into(),
            ));
        }
        let (across, up) = if self.angle == 0 {
            (self.width.steps(), self.depth.steps())
        } else {
            (self.depth.steps(), self.width.steps())
        };
        let rings = rings_of(boundary);
        let Some(outer) = rings.first() else {
            return Ok(Tiles { whole: 0, cut: 0 });
        };
        let (low, high) = bounds_of(outer);
        let (across, up) = (i128::from(across), i128::from(up));
        let (ox, oy) = (i128::from(self.origin.x), i128::from(self.origin.y));
        let first =
            |from: i128, origin: i128, step: i128| from.saturating_sub(origin).div_euclid(step);
        let last = |to: i128, origin: i128, step: i128| {
            // The index of the last tile starting before `to`.
            to.saturating_sub(origin).saturating_sub(1).div_euclid(step)
        };
        let (i0, i1) = (first(low[0], ox, across), last(high[0], ox, across));
        let (j0, j1) = (first(low[1], oy, up), last(high[1], oy, up));
        let columns = i1.saturating_sub(i0).saturating_add(1).max(0);
        let rows = j1.saturating_sub(j0).saturating_add(1).max(0);
        if columns.saturating_mul(rows) > i128::from(MOST_TILES) {
            return Err(SlabError::BadGrid(format!(
                "its bounding box holds more than {MOST_TILES} tiles"
            )));
        }
        let edges: Vec<([i128; 2], [i128; 2])> =
            rings.iter().flat_map(|ring| edges_of(ring)).collect();
        let doubled: Vec<Vec<[i128; 2]>> = rings
            .iter()
            .map(|ring| {
                ring.iter()
                    .map(|p| [p[0].saturating_mul(2), p[1].saturating_mul(2)])
                    .collect()
            })
            .collect();
        let (mut whole, mut cut) = (0_u64, 0_u64);
        let mut i = i0;
        while i <= i1 {
            let x0 = ox.saturating_add(i.saturating_mul(across));
            let x1 = x0.saturating_add(across);
            let mut j = j0;
            while j <= j1 {
                let y0 = oy.saturating_add(j.saturating_mul(up));
                let y1 = y0.saturating_add(up);
                if edges
                    .iter()
                    .any(|(a, b)| crosses_open(*a, *b, [x0, y0], [x1, y1]))
                {
                    cut = cut.saturating_add(1);
                } else if region_of(&doubled, [x0.saturating_add(x1), y0.saturating_add(y1)])
                    == Where::Inside
                {
                    whole = whole.saturating_add(1);
                }
                j = j.saturating_add(1);
            }
            i = i.saturating_add(1);
        }
        Ok(Tiles { whole, cut })
    }
}

/// Where a point stands against a loop or a region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Where {
    Inside,
    Outside,
    On,
}

fn rings_of(boundary: &Boundary) -> Vec<Vec<[i128; 2]>> {
    boundary
        .profile()
        .into_iter()
        .map(|ring| {
            ring.into_iter()
                .map(|(_, p)| [i128::from(p.x), i128::from(p.y)])
                .collect()
        })
        .collect()
}

fn bounds_of(ring: &[[i128; 2]]) -> ([i128; 2], [i128; 2]) {
    let mut low = [i128::MAX; 2];
    let mut high = [i128::MIN; 2];
    for p in ring {
        for axis in 0..2 {
            if let (Some(l), Some(h), Some(v)) =
                (low.get_mut(axis), high.get_mut(axis), p.get(axis))
            {
                *l = (*l).min(*v);
                *h = (*h).max(*v);
            }
        }
    }
    (low, high)
}

fn edges_of(ring: &[[i128; 2]]) -> impl Iterator<Item = ([i128; 2], [i128; 2])> + '_ {
    ring.iter().enumerate().filter_map(move |(at, a)| {
        let b = ring.get(at.saturating_add(1)).or_else(|| ring.first())?;
        Some((*a, *b))
    })
}

/// `(b - a) x (p - a)`: positive when `p` is left of the way from `a` to `b`.
fn cross(a: [i128; 2], b: [i128; 2], p: [i128; 2]) -> i128 {
    let ux = b[0].saturating_sub(a[0]);
    let uy = b[1].saturating_sub(a[1]);
    let vx = p[0].saturating_sub(a[0]);
    let vy = p[1].saturating_sub(a[1]);
    ux.saturating_mul(vy).saturating_sub(uy.saturating_mul(vx))
}

/// Where `p` stands against one loop, exactly.
fn locate(ring: &[[i128; 2]], p: [i128; 2]) -> Where {
    let mut inside = false;
    for (a, b) in edges_of(ring) {
        let c = cross(a, b, p);
        let within = p[0] >= a[0].min(b[0])
            && p[0] <= a[0].max(b[0])
            && p[1] >= a[1].min(b[1])
            && p[1] <= a[1].max(b[1]);
        if c == 0 && within {
            return Where::On;
        }
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let left = if b[1] > a[1] { c > 0 } else { c < 0 };
            if left {
                inside = !inside;
            }
        }
    }
    if inside {
        Where::Inside
    } else {
        Where::Outside
    }
}

/// Where `p` stands against a region: its loops are the outer loop first, then its holes.
fn region_of(rings: &[Vec<[i128; 2]>], p: [i128; 2]) -> Where {
    let mut state = Where::Outside;
    for (at, ring) in rings.iter().enumerate() {
        match (at, locate(ring, p)) {
            (_, Where::On) => return Where::On,
            (0, found) => state = found,
            (_, Where::Inside) => return Where::Outside,
            _ => {}
        }
    }
    state
}

/// Whether the segment from `a` to `b` crosses the open rectangle `low` to `high`, by the
/// separating-axis test with the rectangle's two axes and the segment's normal, in whole numbers.
fn crosses_open(a: [i128; 2], b: [i128; 2], low: [i128; 2], high: [i128; 2]) -> bool {
    if a[0].max(b[0]) <= low[0] || a[0].min(b[0]) >= high[0] {
        return false;
    }
    if a[1].max(b[1]) <= low[1] || a[1].min(b[1]) >= high[1] {
        return false;
    }
    let corners = [low, [high[0], low[1]], high, [low[0], high[1]]];
    let signs: Vec<i128> = corners.iter().map(|c| cross(a, b, *c).signum()).collect();
    !(signs.iter().all(|s| *s >= 0) || signs.iter().all(|s| *s <= 0))
}

/// Whether `p`, a point in grid steps, is in the region of `boundary` (its outer loop less its
/// holes) and on none of its edges.
#[must_use]
pub fn contains(boundary: &Boundary, p: [i64; 2]) -> bool {
    region_of(&rings_of(boundary), [i128::from(p[0]), i128::from(p[1])]) == Where::Inside
}

/// A point of the grid strictly inside the region of `boundary`, the same for the same boundary:
/// the centre (to a whole step, rounded down) of the first ear of its outer loop that holds
/// inside the region. `None` for a region too thin to hold a grid point.
#[must_use]
pub fn interior_point(boundary: &Boundary) -> Option<[i64; 2]> {
    let rings = rings_of(boundary);
    let outer = rings.first()?;
    let count = outer.len();
    for at in 0..count {
        let previous = at.checked_add(count)?.checked_sub(1)?.checked_rem(count)?;
        let next = at.checked_add(1)?.checked_rem(count)?;
        let (a, b, c) = (*outer.get(previous)?, *outer.get(at)?, *outer.get(next)?);
        if cross(a, b, c) <= 0 {
            continue;
        }
        let centre = [
            a[0].saturating_add(b[0]).saturating_add(c[0]).div_euclid(3),
            a[1].saturating_add(b[1]).saturating_add(c[1]).div_euclid(3),
        ];
        if region_of(&rings, centre) == Where::Inside {
            return Some([
                i64::try_from(centre[0]).ok()?,
                i64::try_from(centre[1]).ok()?,
            ]);
        }
    }
    None
}
