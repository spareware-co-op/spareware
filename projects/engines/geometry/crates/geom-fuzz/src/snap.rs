//! Fuzz target: `geom-2d`'s snap index never misses, never panics and answers the same twice.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: up to [`MAX_ENTITIES`] points,
//! segments and circles on a small lattice, so that shared ends, shared lines and tangent circles
//! are all common, a cursor, an aperture and the kinds to ask for. [`check`] builds the drawing and
//! its index and asks the snap query. What it answers must be what a scan of every entity answers
//! (so the tree misses nothing), the same when asked twice, a point that can be read, and within
//! the aperture.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case.
//!
//! Method: differential testing of the tree against a scan, the laws of chapter 14.8
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`): the closest wins, a tie goes to the
//! lower name, and nothing outside the aperture is returned.
//!
//! Cost: at most [`MAX_ENTITIES`] entities, two queries and a scan.

#![allow(
    clippy::many_single_char_names,
    reason = "x, y and a to d are lattice coordinates and p and q points, as in the case's definition"
)]

use geom_2d::carrier::{Carriers, Circle2};
use geom_2d::entity::{Attr, Ends, Entity, Shape};
use geom_2d::index::{Found, SnapIndex};
use geom_2d::point::Pt2;
use geom_2d::snap::{Drawing, SnapKind};
use geom_num::arc::Circle2 as GridCircle;
use geom_num::carrier::Carrier;
use geom_num::grid::{P2, Steps};
use geom_op::NameId;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most entities a case holds.
pub const MAX_ENTITIES: u8 = 8;

/// The lattice's points on each axis, and its spacing in steps.
pub const SIDE: u8 = 6;

/// The spacing of the lattice.
pub const SPACING: i64 = 10;

/// The most bytes [`decode`] reads from any input: 4 for the cursor, the aperture and the kinds, a
/// count, and 5 for each of 8 entities.
pub const MAX_INPUT_BYTES: usize = 4 + 1 + 5 * 8;

/// The kinds a case asks for, by bit.
const KINDS: [SnapKind; 7] = [
    SnapKind::End,
    SnapKind::Start,
    SnapKind::Mid,
    SnapKind::Center,
    SnapKind::Node,
    SnapKind::Quadrant,
    SnapKind::Intersection,
];

/// One entity of a case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    /// A point at lattice point (x, y).
    Point(u8, u8),
    /// A segment between two lattice points.
    Segment(u8, u8, u8, u8),
    /// A circle about a lattice point, of this many spacings.
    Circle(u8, u8, u8),
}

/// One decoded input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The cursor, as lattice coordinates in quarter spacings.
    pub cursor: (u8, u8),
    /// The aperture, in steps.
    pub aperture: u8,
    /// The kinds asked for, one bit each.
    pub kinds: u8,
    /// The entities.
    pub items: Vec<Item>,
}

/// The case `data` holds, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let lattice = SIDE.saturating_mul(4);
    let cursor = (
        src.byte().checked_rem(lattice).unwrap_or(0),
        src.byte().checked_rem(lattice).unwrap_or(0),
    );
    let aperture = src.byte().checked_rem(40).unwrap_or(0).saturating_add(1);
    let kinds = src.byte();
    let count = src
        .byte()
        .checked_rem(MAX_ENTITIES.saturating_add(1))
        .unwrap_or(0);
    let mut items = Vec::new();
    for _ in 0..count {
        let mut at = || src.byte().checked_rem(SIDE).unwrap_or(0);
        let pick = at();
        let (a, b, c, d) = (at(), at(), at(), at());
        items.push(match pick % 3 {
            0 => Item::Point(a, b),
            1 => Item::Segment(a, b, c, d),
            _ => Item::Circle(a, b, c.saturating_add(1)),
        });
    }
    let read = data.len().saturating_sub(src.remaining());
    (
        Case {
            cursor,
            aperture,
            kinds,
            items,
        },
        read,
    )
}

fn point(x: u8, y: u8) -> Result<P2, Finding> {
    P2::new(
        i64::from(x).saturating_mul(SPACING),
        i64::from(y).saturating_mul(SPACING),
    )
    .map_err(|e| Finding::new("a lattice point is in the frame", || format!("{e:?}")))
}

fn shape(item: Item) -> Result<Option<Shape>, Finding> {
    Ok(match item {
        Item::Point(x, y) => Some(Shape::Point(point(x, y)?)),
        Item::Segment(a, b, c, d) => {
            let (p, q) = (point(a, b)?, point(c, d)?);
            if p == q {
                None
            } else {
                Some(Shape::Line {
                    carrier: Carrier::through(p, q).map_err(|e| {
                        Finding::new("a segment has a carrier", || format!("{e:?}"))
                    })?,
                    ends: Ends::Segment(Pt2::Grid(p), Pt2::Grid(q)),
                })
            }
        }
        Item::Circle(x, y, r) => Some(Shape::Circle(Circle2::Grid(
            GridCircle::new(
                point(x, y)?,
                i128::from(r).saturating_mul(i128::from(SPACING)),
            )
            .map_err(|e| Finding::new("a circle has a radius", || format!("{e:?}")))?,
        ))),
    })
}

fn name(n: u8) -> NameId {
    let mut b = [0_u8; 16];
    b[15] = n;
    NameId::from_bytes(b)
}

/// What an answer is, without its carriers.
type Key = Option<(SnapKind, NameId, Option<NameId>, Option<u32>, u8, [u64; 2])>;

fn key(f: Option<&Found>) -> Key {
    f.map(|f| {
        (
            f.kind,
            f.name,
            f.with,
            f.sub,
            f.pick,
            [
                f.distance_squared[0].to_bits(),
                f.distance_squared[1].to_bits(),
            ],
        )
    })
}

/// Builds the case's drawing and index and holds the answer to every law.
///
/// # Errors
///
/// A [`Finding`] for the first law broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    let broken = |law: &'static str, detail: String| {
        Finding::new(law, move || format!("{case:?}: {detail}"))
    };
    let mut items = Vec::new();
    for (n, item) in (0_u8..).zip(&case.items) {
        if let Some(s) = shape(*item)? {
            items.push((name(n), Entity::new(Attr(0), s)));
        }
    }
    let drawing = Drawing::new(items, Vec::new())
        .map_err(|e| broken("a drawing of distinct names is made", format!("{e:?}")))?;
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let index = SnapIndex::build(&drawing, &Exec::serial(), &mut meter)
        .map_err(|e| broken("an index is built", format!("{e:?}")))?;
    let kinds: Vec<SnapKind> = KINDS
        .iter()
        .enumerate()
        .filter(|(i, _)| case.kinds >> i & 1 == 1)
        .map(|(_, k)| *k)
        .collect();
    let cursor = P2::new(
        i64::from(case.cursor.0)
            .saturating_mul(SPACING)
            .saturating_div(4),
        i64::from(case.cursor.1)
            .saturating_mul(SPACING)
            .saturating_div(4),
    )
    .map_err(|e| broken("the cursor is in the frame", format!("{e:?}")))?;
    let aperture = Steps(u64::from(case.aperture));
    let ask = |m: &mut WorkMeter<'_>| index.snap(cursor, aperture, &kinds, &mut Carriers::new(), m);
    let tree = ask(&mut meter).map_err(|e| broken("a snap is answered", format!("{e:?}")))?;
    let again = ask(&mut meter).map_err(|e| broken("a snap is answered", format!("{e:?}")))?;
    let scan = index
        .scan(cursor, aperture, &kinds, &mut Carriers::new())
        .map_err(|e| broken("a scan is answered", format!("{e:?}")))?;
    if key(tree.as_ref()) != key(scan.as_ref()) {
        return Err(broken(
            "the tree gives the scan's answer",
            format!("{:?} against {:?}", key(tree.as_ref()), key(scan.as_ref())),
        ));
    }
    if key(tree.as_ref()) != key(again.as_ref()) {
        return Err(broken("one answer when asked twice", String::new()));
    }
    if let Some(found) = &tree {
        let limit = f64::from(case.aperture) * f64::from(case.aperture);
        if found.distance_squared[0] > limit {
            return Err(broken(
                "the answer is within the aperture",
                format!("{:?}", found.distance_squared),
            ));
        }
    }
    Ok(())
}

/// The harness every fuzzing engine drives: [`decode`] then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
