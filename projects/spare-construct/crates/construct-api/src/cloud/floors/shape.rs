//! The commands that shape-edit a floor's top face (job 14.15, CON-A4-F4): points with heights,
//! split lines, a wall picked as a support, and a reset, through the same edit log, access check
//! and claims as the other floor commands ([`super`]). Each is one entry, so one undo takes back
//! all it wrote ([`crate::cloud::undo`]). The rules are construct-slabs' ([`construct_slabs::top`]):
//! a refusal is its words after the floor's name, and changes nothing.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `floor.shape.point` | `element`, `at` (`[x, y]` typed), `height` (typed, from the floor's top; left out: the top's height there now), `point` (its ID; left out: `p<n>`, the first free) | The floor: a point on its top, or the point of that ID moved and set |
//! | `floor.shape.height` | `element`, `point` or `split`, `height` (typed) | The floor: that point's height, or both ends of that split line |
//! | `floor.shape.split` | `element`, `from` and `to` (each `[x, y]` typed), `split` (its ID; left out: `s<n>`) | The floor: a split line between the points at those places, each end a new point at the top's height there when no point is there yet |
//! | `floor.shape.support` | `element`, `wall` (a wall's ID) | The floor: a split line along the wall's location line where it crosses the floor, its ends at the top's height there, so the top folds over the wall |
//! | `floor.shape.remove` | `element`, `point` or `split` | The floor: that point, with every split line ending at it, or that split line, taken away |
//! | `floor.shape.reset` | `element` | The floor: every point and split line taken away, its top plain again |
//!
//! A new point's height, left out, is the top face's height there now, to the nearest grid step,
//! a half away from zero (the one rounding), so a point or a split line added changes nothing
//! until a height is set. A floor that slopes (A72's arrow) or has slab edges is refused by name:
//! one or the other. Beams are not in the model yet, so only a wall is picked as a support.

use super::{Answer, Run, floor_of, length, need, put_floor};
use construct_levels::Length;
use construct_model::element::Element;
use construct_regen::wall_plan::line_of;
use construct_slabs::{Edits, Point, SplitLine, TopPoint, shape_of, with_top};
use kit_core::Id;
use kit_store::{StoreError, Txn};
use serde::Deserialize;

/// Every command here.
pub const COMMANDS: [&str; 6] = [
    "floor.shape.point",
    "floor.shape.height",
    "floor.shape.split",
    "floor.shape.support",
    "floor.shape.remove",
    "floor.shape.reset",
];

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    /// A point's ID.
    point: Option<String>,
    /// A split line's ID.
    split: Option<String>,
    /// A place in plan, typed.
    at: Option<[String; 2]>,
    /// A height from the floor's top, typed.
    height: Option<String>,
    /// A split line's ends, typed.
    from: Option<[String; 2]>,
    to: Option<[String; 2]>,
    /// A wall picked as a support: its ID.
    #[serde(default, with = "kit_ops::id::option")]
    wall: Option<Id>,
}

/// Runs one of these commands.
pub(super) async fn apply<T: Txn>(run: &mut Run<'_, T>) -> Answer {
    let args: Args = match serde_json::from_value(run.edit.args.clone()) {
        Ok(args) => args,
        Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
    };
    let before = match floor_of(run, args.element.clone()).await? {
        Ok(before) => before,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let name = before.name.clone();
    let cmd = run.edit.cmd.clone();
    let changed = match cmd.as_str() {
        "floor.shape.reset" => Ok(None),
        "floor.shape.support" => match support(run, &before, &args).await? {
            Ok(edits) => Ok(Some(edits)),
            Err(refusal) => Err(refusal),
        },
        _ => edit(&before, &cmd, &args).map(Some),
    };
    let edits = match changed {
        Ok(edits) => edits,
        Err(refusal) => return Ok(Err(format!("{name}'s shape: {refusal}"))),
    };
    put_floor(run, with_top(before, edits), "shape", &name).await
}

/// An ID from what a person sent.
fn id_of(text: &str) -> Result<Id, String> {
    Id::parse(text).map_err(|_| format!("{text:?} is not an ID."))
}

/// A place in plan from typed text, in grid steps.
fn place(at: &[String; 2]) -> Result<Point, String> {
    Ok(Point {
        x: length(&at[0])?.steps(),
        y: length(&at[1])?.steps(),
    })
}

/// A floor's edits as its record holds them: none on a plain floor.
fn edits_of(record: &Element) -> Result<(Vec<TopPoint>, Vec<SplitLine>), String> {
    let shape = shape_of(&record.recipe).map_err(|e| e.to_string())?;
    Ok(shape.edits().map_or_else(
        || (Vec::new(), Vec::new()),
        |e| (e.points().to_vec(), e.splits().to_vec()),
    ))
}

/// The first free ID `<tag><n>` among the points' and split lines' IDs.
fn free_id(tag: &str, points: &[TopPoint], splits: &[SplitLine]) -> Result<Id, String> {
    let taken = |id: &Id| points.iter().any(|p| p.id == *id) || splits.iter().any(|s| s.id == *id);
    for n in 0_u32.. {
        let id = id_of(&format!("{tag}{n}"))?;
        if !taken(&id) {
            return Ok(id);
        }
    }
    Err("No free ID is left.".to_owned())
}

/// The top face's height at `p` now, to the nearest grid step (a half away from zero); 0 on a
/// plain floor or off it.
fn height_now(record: &Element, p: Point) -> Length {
    let top = shape_of(&record.recipe).ok();
    let exact = top
        .as_ref()
        .and_then(|s| s.top())
        .and_then(|t| t.height_at(p));
    let steps = exact.map_or(0, |(num, den)| {
        // den is above zero; the nearest whole number, a half away from zero.
        let twice = num.saturating_mul(2);
        let rounded = if num >= 0 {
            twice.saturating_add(den)
        } else {
            twice.saturating_sub(den)
        };
        let whole = rounded.checked_div(den.saturating_mul(2)).unwrap_or(0);
        i64::try_from(whole).unwrap_or(0)
    });
    Length::from_steps(steps)
}

/// The point at `p`, or a new one there at the top's height now, named `<split><end>`.
fn end_at(
    record: &Element,
    p: Point,
    points: &mut Vec<TopPoint>,
    splits: &[SplitLine],
) -> Result<Id, String> {
    if let Some(found) = points.iter().find(|q| q.at == p) {
        return Ok(found.id.clone());
    }
    let id = free_id("p", points, splits)?;
    points.push(TopPoint {
        id: id.clone(),
        at: p,
        height: height_now(record, p),
    });
    Ok(id)
}

/// The edits a command other than a reset or a support makes of `record`'s.
fn edit(record: &Element, cmd: &str, args: &Args) -> Result<Edits, String> {
    let (mut points, mut splits) = edits_of(record)?;
    match cmd {
        "floor.shape.point" => {
            let at = place(need(args.at.as_ref(), "where the point is")?)?;
            let id = match &args.point {
                Some(text) => id_of(text)?,
                None => free_id("p", &points, &splits)?,
            };
            let height = match &args.height {
                Some(text) => length(text)?,
                None => height_now(record, at),
            };
            points.retain(|p| p.id != id);
            points.push(TopPoint { id, at, height });
        }
        "floor.shape.height" => {
            let height = length(need(args.height.as_deref(), "a height")?)?;
            let ends: Vec<Id> = match (&args.point, &args.split) {
                (Some(point), None) => vec![id_of(point)?],
                (None, Some(split)) => {
                    let id = id_of(split)?;
                    let line = splits
                        .iter()
                        .find(|s| s.id == id)
                        .ok_or_else(|| format!("there is no split line {id}"))?;
                    vec![line.from.clone(), line.to.clone()]
                }
                _ => return Err("This command needs a point or a split line.".to_owned()),
            };
            for end in ends {
                let point = points
                    .iter_mut()
                    .find(|p| p.id == end)
                    .ok_or_else(|| format!("there is no point {end}"))?;
                point.height = height;
            }
        }
        "floor.shape.split" => {
            let from = place(need(args.from.as_ref(), "where the line starts")?)?;
            let to = place(need(args.to.as_ref(), "where the line ends")?)?;
            let id = match &args.split {
                Some(text) => id_of(text)?,
                None => free_id("s", &points, &splits)?,
            };
            let a = end_at(record, from, &mut points, &splits)?;
            let b = end_at(record, to, &mut points, &splits)?;
            splits.retain(|s| s.id != id);
            splits.push(SplitLine { id, from: a, to: b });
        }
        "floor.shape.remove" => match (&args.point, &args.split) {
            (Some(point), None) => {
                let id = id_of(point)?;
                if !points.iter().any(|p| p.id == id) {
                    return Err(format!("there is no point {id}"));
                }
                points.retain(|p| p.id != id);
                splits.retain(|s| s.from != id && s.to != id);
            }
            (None, Some(split)) => {
                let id = id_of(split)?;
                if !splits.iter().any(|s| s.id == id) {
                    return Err(format!("there is no split line {id}"));
                }
                splits.retain(|s| s.id != id);
            }
            _ => return Err("This command needs a point or a split line.".to_owned()),
        },
        other => return Err(format!("Unknown command {other}.")),
    }
    Edits::new(points, splits).map_err(|e| e.to_string())
}

/// Where the segment `a`-`b` crosses the line through `p` and `q`, as a parameter along `a`-`b`
/// `(num, den)`, `den` above zero; `None` when they are parallel.
fn crossing(a: Point, b: Point, p: Point, q: Point) -> Option<(i128, i128)> {
    let span = |from: Point, to: Point| {
        [
            i128::from(to.x).saturating_sub(i128::from(from.x)),
            i128::from(to.y).saturating_sub(i128::from(from.y)),
        ]
    };
    let cross = |first: [i128; 2], second: [i128; 2]| {
        first[0]
            .saturating_mul(second[1])
            .saturating_sub(first[1].saturating_mul(second[0]))
    };
    let (along, other) = (span(a, b), span(p, q));
    let den = cross(along, other);
    if den == 0 {
        return None;
    }
    let num = cross(span(a, p), other);
    Some(if den < 0 {
        (num.saturating_neg(), den.saturating_neg())
    } else {
        (num, den)
    })
}

/// `a + (b - a) * t` for `t = num / den` in `[0, 1]`, to the nearest grid step.
fn at_fraction(a: Point, b: Point, (num, den): (i128, i128)) -> Option<Point> {
    let one = |from: i64, to: i64| {
        let span = i128::from(to).checked_sub(i128::from(from))?;
        let twice = span.checked_mul(num)?.checked_mul(2)?;
        let rounded = if twice >= 0 {
            twice.checked_add(den)?
        } else {
            twice.checked_sub(den)?
        };
        let step = rounded.checked_div(den.checked_mul(2)?)?;
        i64::try_from(i128::from(from).checked_add(step)?).ok()
    };
    Some(Point {
        x: one(a.x, b.x)?,
        y: one(a.y, b.y)?,
    })
}

/// A wall picked as a support: a split line along its location line over the floor, between the
/// two places it crosses the floor's outer loop, or from one of them to the wall's end on the
/// floor, or the whole line when the wall stands wholly on the floor. Each end where the line
/// crosses an edge is rounded once to the nearest grid step (on an edge along x or y it lands on
/// the edge exactly); a line the floor's rules refuse (it leaves the floor) is refused in their
/// words.
async fn support<T: Txn>(
    run: &mut Run<'_, T>,
    floor: &Element,
    args: &Args,
) -> Result<Result<Edits, String>, StoreError> {
    let wall_id = match need(args.wall.clone(), "a wall") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((_, wall)) = run.current(&wall_id).await? else {
        return Ok(Err("That wall no longer exists.".to_owned()));
    };
    let Some(([sx, sy], [ex, ey])) = line_of(&wall.recipe) else {
        return Ok(Err(format!(
            "{} is not a layered wall: only a wall is picked as a support (beams are not in the \
             model yet)",
            wall.name
        )));
    };
    // The line is in the wall's frame: the model's plan less its placement (whole millimetres).
    let (Some(px), Some(py)) = (
        Length::checked_from_mm(wall.placement.x),
        Length::checked_from_mm(wall.placement.y),
    ) else {
        return Ok(Err("the wall's place is beyond the grid".to_owned()));
    };
    let moved = |x: i64, y: i64| Point {
        x: x.saturating_add(px.steps()),
        y: y.saturating_add(py.steps()),
    };
    let line = (moved(sx, sy), moved(ex, ey));
    let mut last = Err("the wall does not cross the floor".to_owned());
    for (from, to) in support_lines(floor, line) {
        let made = (|| {
            let (mut points, mut splits) = edits_of(floor)?;
            let id = free_id("s", &points, &splits)?;
            let a = end_at(floor, from, &mut points, &splits)?;
            let b = end_at(floor, to, &mut points, &splits)?;
            splits.push(SplitLine { id, from: a, to: b });
            let edits = Edits::new(points, splits).map_err(|e| e.to_string())?;
            with_top(floor.clone(), Some(edits.clone())).map_err(|e| e.to_string())?;
            Ok::<_, String>(edits)
        })();
        match made {
            Ok(edits) => return Ok(Ok(edits)),
            Err(refusal) => last = Err(refusal),
        }
    }
    Ok(last)
}

/// The candidate parts of a wall's location line `a`-`b` (grid steps in the model's plan) over the
/// floor, as [`support`] says, the likeliest first.
fn support_lines(floor: &Element, (a, b): (Point, Point)) -> Vec<(Point, Point)> {
    let Ok(shape) = shape_of(&floor.recipe) else {
        return Vec::new();
    };
    let outer: Vec<Point> = shape
        .boundary()
        .profile()
        .first()
        .map(|ring| ring.iter().map(|(_, p)| *p).collect())
        .unwrap_or_default();
    let within = |(num, den): (i128, i128)| num >= 0 && num <= den;
    let mut hits: Vec<(i128, i128)> = Vec::new();
    for (k, p) in outer.iter().enumerate() {
        let q = outer
            .get(k.saturating_add(1))
            .or_else(|| outer.first())
            .copied()
            .unwrap_or(*p);
        if let (Some(t), Some(u)) = (crossing(a, b, *p, q), crossing(*p, q, a, b))
            && within(t)
            && within(u)
        {
            hits.push(t);
        }
    }
    hits.sort_by(|x, y| x.0.saturating_mul(y.1).cmp(&y.0.saturating_mul(x.1)));
    let ends: Vec<Point> = hits.iter().filter_map(|t| at_fraction(a, b, *t)).collect();
    let mut out = match (ends.first(), ends.last()) {
        (Some(first), Some(last)) if first != last => vec![(*first, *last)],
        (Some(only), _) => vec![(a, *only), (*only, b)],
        _ => vec![(a, b)],
    };
    out.retain(|(from, to)| from != to);
    out
}
