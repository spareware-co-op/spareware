//! Openings: what a door hosted in a wall cuts from it, decided before the engine is asked.
//!
//! What it does: [`plan`] turns the doors hosted in a straight wall ([`Hosted`]) into the cutters
//! the builder subtracts from the wall's extrusion, one after another in the order of the doors'
//! keys, and refuses every case it does not build with a [`GeometryError`] whose message starts
//! with the door's key and says why. A door's opening is its width along the wall and its height,
//! through the wall's whole thickness whatever the door's own thickness, its cutter reaching
//! [`PAST_MM`] past both sides and, for a door to the floor, below the base, as the engine seam's
//! own suite cuts a door (geom-solidcore's `tests/suite`): no face of the cutter lies on a face of
//! the wall it passes through. [`plan_in`] asks the same rules of a wall given by its [`Frame`]
//! alone, each door's place already in that frame: a layered wall's openings are asked so (the
//! parent's `layered` module), through every layer at once.
//!
//! A placed family's opening (job 6.4, [`crate::families`]) is cut on the same path: its outline,
//! solved at its type's values and placed in the wall's frame where the family stands, is a
//! [`Span`] along the wall and up it, and every rule below is asked of it in the same words, the
//! same cutter made (through the wall's whole thickness, the format's depth rule for a wall),
//! the same names given. A box door and a family differ only in how their span is found.
//!
//! A plain opening (construct-openings' `opening.wall`, job 4.2) is cut on this same path, by the
//! same rules and in the same words, named `opening <key>` where a door is `door <key>`: its
//! width along the wall and its height, always through the wall's whole thickness, its place
//! along the wall and its sill height given as a door's are. It is a [`Span`] like the rest, and
//! `cut_span` is the one cutter of all three.
//!
//! What it builds, in the wall's frame (along its axis from `end0` to `end1`, up from its base):
//!
//! - an opening wholly inside the wall, its foot above the base: four new faces, its jambs, its
//!   head and its sill;
//! - an opening whose foot is on the base, a door to the floor (the common case): its jambs and
//!   its head; the base is cut in two and keeps its one name.
//!
//! What it refuses, each with its code: `NotAHost` (only a straight wall has openings cut),
//! `NotAnOpening` (only `door.single` and `opening.wall` cut one), `DuplicateOpening` (one key
//! twice), `OpeningOutsideHost` (an opening wholly beside, above or below its wall),
//! `OpeningAcrossWall` (a door not along its wall's axis), `OpeningNotInWall` (a door whose plan
//! misses the wall's thickness), `OpeningNearFace` (a face of the opening within the band of the
//! wall's matching face, or of another opening, without being on it: named fault SC-4's case),
//! `OpeningPastEnd` and `OpeningAtEnd` (an opening crossing or touching an end: it would be a
//! shorter wall, not an opening), `OpeningBelowBase`, `OpeningTooTall` (a head at or above the
//! wall's top, which would cut the wall in two or leave no head), `OpeningsOverlap` (two openings
//! that overlap or touch), and `NotWholeMillimetres` (an opening whose corners are not whole
//! millimetres, which a display mesh in whole millimetres cannot hold).
//!
//! Names: each face an opening makes is named as chapter 11.9 names a tool's faces,
//! `Created { op: the wall's operation, role: tool(key), from: [the opening's own face] }`, where
//! the key is the first 16 bytes of the SHA-256 of the door's key and the opening's own face is
//! `Created { op: the door's operation, role }` for the roles `jamb.start` (nearer `end0`),
//! `jamb.end`, `head` and `sill`. No length goes in, so the names stay when the wall is lengthened
//! or another door is added, and two doors of one type name their faces apart.
//!
//! Why each of these: `projects/spare-construct/notes/CON-A3-F1-P2-door-openings.md`.
//!
//! Depends on: the parent module's [`Units`], [`Extrusion`] and naming, and `engine-geometry`'s
//! names. Interim: the case of SC-4 is refused here, not built, until lengths and meshes are on
//! the kernel's grid and the engine is the kernel's exact one (the interim register's row).

use super::{Extrusion, Units, fault, op_id, too_big};
use construct_model::geometry::{FaceName, GeometryError, Recipe};
use engine_geometry::value::engine::Profile;
use engine_geometry::value::grid::P3;
use engine_geometry::value::name::{NameId, NameList, Role, RoleArg, TopoName};
use engine_geometry::value::op::{OpId, OpPath};
use kit_ops::sha256_hex;

/// How far an opening's cutter reaches past each side of its wall, and below the base for a door
/// to the floor, in millimetres.
pub const PAST_MM: i64 = 100;

/// The roles of the faces an opening's own solid gives the wall, in the order they are named.
pub const OPENING_ROLES: [&str; 4] = ["jamb.start", "jamb.end", "head", "sill"];

/// The operation of a plain opening through a wall (construct-openings' `opening.wall`).
pub(super) const PLAIN_OP: &str = construct_openings::WALL_OP;

/// The operation of a door, which cuts an opening.
pub(super) const OPENING_OP: &str = "door.single";

/// What a refusal calls the hosted element of `recipe`: `door` or `opening`.
fn noun_of(recipe: &Recipe) -> &'static str {
    if recipe.op == PLAIN_OP {
        "opening"
    } else {
        "door"
    }
}

/// What a refusal calls the hosted element of `recipe` with the permanent ID `key`: `door D-1`,
/// or `opening O-1` for a plain opening.
pub(super) fn who(recipe: &Recipe, key: &str) -> String {
    format!("{} {key}", noun_of(recipe))
}

/// An element hosted in a wall that cuts an opening through it.
#[derive(Clone, Copy, Debug)]
pub struct Hosted<'a> {
    /// The hosted element's permanent ID: the order the openings are cut in, and the key their
    /// faces are named by.
    pub key: &'a str,
    /// Its recipe: `door.single`.
    pub recipe: &'a Recipe,
    /// Where its shape's origin sits in the wall's frame, as a grid point ([`Units::offset`]).
    pub at: P3,
}

/// One opening's cutter, in the wall's frame, in grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Cut {
    key: String,
    /// What the hosted element is called in a refusal.
    noun: &'static str,
    door: OpId,
    /// The cutter's lowest and highest corners.
    lo: [i64; 3],
    hi: [i64; 3],
    /// The opening's own foot and head, along start and end.
    foot: i64,
    head: i64,
    start: i64,
    end: i64,
    /// How far the extrusion without this opening strays from the wall with it, at most.
    stray: i64,
}

/// A refusal of the hosted element `key`, called `noun` (a door, an opening, a family).
fn refuse_as(noun: &str, key: &str, code: &str, why: impl std::fmt::Display) -> GeometryError {
    fault(code, format!("{noun} {key}: {why}"))
}

/// The first 16 bytes of the SHA-256 of `text`.
fn sixteen(text: &str) -> Result<[u8; 16], GeometryError> {
    sha256_hex(text.as_bytes())
        .get(..32)
        .and_then(|half| hex::decode(half).ok())
        .and_then(|bytes| <[u8; 16]>::try_from(bytes).ok())
        .ok_or_else(|| fault("Naming", "a key's hash is not 16 bytes"))
}

fn add(a: i64, b: i64) -> Result<i64, GeometryError> {
    a.checked_add(b).ok_or_else(too_big)
}

fn less(a: i64, b: i64) -> Result<i64, GeometryError> {
    a.checked_sub(b).ok_or_else(too_big)
}

/// Half of `steps` at or above zero, rounded up.
fn half_up(steps: i64) -> i64 {
    let half = steps.checked_div(2).unwrap_or(0);
    if steps.checked_rem(2) == Some(0) {
        half
    } else {
        half.saturating_add(1)
    }
}

/// Whether `a` lies within `band` of `b` without being on it.
fn near(a: i64, b: i64, band: i64) -> bool {
    let d = i128::from(a).abs_diff(i128::from(b));
    d > 0 && d <= u128::from(band.unsigned_abs())
}

/// The cutters of the openings `hosted` cuts from `wall`, the extrusion of `host`, sorted by key;
/// none when nothing is hosted.
///
/// # Errors
///
/// The refusals of this module's header, each naming the door and why.
pub(super) fn plan(
    host: &Recipe,
    wall: &Extrusion,
    hosted: &[Hosted<'_>],
    units: Units,
    band: i64,
) -> Result<Vec<Cut>, GeometryError> {
    let mut order: Vec<&Hosted<'_>> = hosted.iter().collect();
    order.sort_by(|a, b| a.key.cmp(b.key));
    let Some(first) = order.first() else {
        return Ok(Vec::new());
    };
    if let Some(family) = order
        .iter()
        .find(|h| crate::families::is_instance(h.recipe))
    {
        return Err(refuse_as(
            "family",
            family.key,
            "NotAHost",
            format!(
                "a family is hosted only on a {}; this is a {}",
                crate::wall_plan::OP,
                host.op
            ),
        ));
    }
    if host.op != "wall.straight" {
        return Err(refuse_as(
            noun_of(first.recipe),
            first.key,
            "NotAHost",
            format!(
                "a {} has no openings cut; only wall.straight and wall.layered do",
                host.op
            ),
        ));
    }
    let (long, across) = wall.long_and_across();
    let frame = Frame {
        along: wall.along,
        size: [
            units.steps(long)?,
            units.steps(across)?,
            units.steps(wall.height)?,
        ],
    };
    let cuts = plan_in(frame, hosted, &[], units, band)?;
    // Last, so a face near another is refused as that, not as a fraction of a millimetre.
    for c in &cuts {
        for corner in [c.start, c.end, c.foot, c.head] {
            units
                .millimetres(corner)
                .map_err(|e| refuse_as(c.noun, &c.key, &e.code, e.message))?;
        }
    }
    Ok(cuts)
}

/// A wall's frame, as its openings are asked in it: the plan axis the frame's along runs on (0 for
/// x, 1 for y; across is the other), and the wall's length along it from `end0`, its thickness
/// across it from its exterior, and its height up from its base, in grid steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Frame {
    /// The plan axis along: 0 for x, 1 for y.
    pub(super) along: usize,
    /// `[length, thickness, height]`, in steps.
    pub(super) size: [i64; 3],
}

/// Where one opening lies in its wall's frame, in grid steps: along from `end0`, from `start` to
/// `end`, and up from the base, from `foot` to `head`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Span {
    pub(super) start: i64,
    pub(super) end: i64,
    pub(super) foot: i64,
    pub(super) head: i64,
}

/// The opening of a hosted element that is not a box door, already placed in its wall's frame:
/// a placed family's.
#[derive(Clone, Copy, Debug)]
pub(super) struct Placed<'a> {
    /// The hosted element's permanent ID.
    pub(super) key: &'a str,
    /// What it is called in a refusal.
    pub(super) noun: &'static str,
    /// The operation that names the opening's own faces.
    pub(super) op: OpId,
    /// Where its opening lies.
    pub(super) span: Span,
}

/// The cutters of the openings `hosted` (box doors) and `placed` (openings already placed, a
/// family's) cut from a wall in `frame`, each door's place given in that frame (along the frame's
/// along axis from `end0`, across from the exterior, up from the base), sorted by key; none when
/// nothing is hosted. The host is a wall: the caller has said so.
///
/// # Errors
///
/// The refusals of this module's header but `NotAHost` and `NotWholeMillimetres`, each naming the
/// door or family and why.
pub(super) fn plan_in(
    frame: Frame,
    hosted: &[Hosted<'_>],
    placed: &[Placed<'_>],
    units: Units,
    band: i64,
) -> Result<Vec<Cut>, GeometryError> {
    let mut keys: Vec<(&str, &str)> = hosted
        .iter()
        .map(|h| (h.key, noun_of(h.recipe)))
        .chain(placed.iter().map(|p| (p.key, p.noun)))
        .collect();
    keys.sort_unstable();
    if let Some(pair) = keys.windows(2).find(|w| matches!(w, [a, b] if a.0 == b.0)) {
        let (key, noun) = pair.first().copied().unwrap_or(("", "door"));
        return Err(refuse_as(
            noun,
            key,
            "DuplicateOpening",
            "hosted twice in one wall",
        ));
    }
    // Each opening in the order of its key, so the first refused is the same whatever order
    // they come in.
    let mut order: Vec<(&str, Result<&Hosted<'_>, &Placed<'_>>)> = hosted
        .iter()
        .map(|h| (h.key, Ok(h)))
        .chain(placed.iter().map(|p| (p.key, Err(p))))
        .collect();
    order.sort_by(|a, b| a.0.cmp(b.0));
    let mut cuts = Vec::with_capacity(order.len());
    for (_, opening) in order {
        cuts.push(match opening {
            Ok(h) => cut(h, frame.along, frame.size, units, band)?,
            Err(p) => cut_span(p, frame.along, frame.size, units, band)?,
        });
    }
    for (i, a) in cuts.iter().enumerate() {
        for b in cuts.iter().skip(i.saturating_add(1)) {
            apart(a, b, band)?;
        }
    }
    Ok(cuts)
}

/// The cutter of one door or plain opening in a wall along `along` of `[length, thickness,
/// height]` steps.
fn cut(
    h: &Hosted<'_>,
    along: usize,
    [length, thickness, height]: [i64; 3],
    units: Units,
    band: i64,
) -> Result<Cut, GeometryError> {
    let (key, noun) = (h.key, noun_of(h.recipe));
    let refuse = |code: &str, why: &str| refuse_as(noun, key, code, why);
    // What it opens: its width along the wall, its depth across it (none for a plain opening,
    // which goes through all of the wall), its height, and the plan axis it runs along.
    let (width, depth, tall, runs) = if h.recipe.op == PLAIN_OP {
        let opening = construct_openings::WallOpening::of(key, h.recipe)
            .map_err(|e| fault(e.code(), e.to_string()))?;
        (
            units.from_q0(opening.width.steps())?,
            None,
            units.from_q0(opening.height.steps())?,
            usize::from(opening.along_y),
        )
    } else if h.recipe.op == OPENING_OP {
        let door = Extrusion::door(&h.recipe.args).map_err(|e| refuse(&e.code, &e.message))?;
        let (width, depth) = door.long_and_across();
        (
            units.steps(width)?,
            Some(units.steps(depth)?),
            units.steps(door.height)?,
            door.along,
        )
    } else {
        return Err(refuse(
            "NotAnOpening",
            &format!(
                "a {} cuts no opening; only {OPENING_OP} and {PLAIN_OP} do",
                h.recipe.op
            ),
        ));
    };
    if runs != along {
        return Err(refuse(
            "OpeningAcrossWall",
            "it does not run along its wall's axis",
        ));
    }
    let at = [h.at.x.get(), h.at.y.get(), h.at.z.get()];
    let (a, c) = if along == 1 { (1, 0) } else { (0, 1) };
    let start = at.get(a).copied().unwrap_or_default();
    let foot = h.at.z.get();
    let (end, head) = (add(start, width)?, add(foot, tall)?);
    // A door's box must meet the wall's thickness; a plain opening goes through all of it.
    if let Some(depth) = depth {
        let side = at.get(c).copied().unwrap_or_default();
        let back = add(side, depth)?;
        if side >= thickness || back <= 0 {
            return Err(refuse(
                "OpeningNotInWall",
                "its plan misses the wall's thickness",
            ));
        }
    }
    if end <= 0 || start >= length || head <= 0 || foot >= height {
        return Err(refuse(
            "OpeningOutsideHost",
            "it lies outside its wall: an opening is cut inside the wall that hosts it",
        ));
    }
    let placed = Placed {
        key,
        noun,
        op: op_id(h.recipe)?,
        span: Span {
            start,
            end,
            foot,
            head,
        },
    };
    cut_span(&placed, along, [length, thickness, height], units, band)
}

/// The cutter of one opening placed in a wall along `along` of `[length, thickness, height]`
/// steps, by the rules
/// of this module's header: through the wall's whole thickness.
fn cut_span(
    placed: &Placed<'_>,
    along: usize,
    [length, thickness, height]: [i64; 3],
    units: Units,
    band: i64,
) -> Result<Cut, GeometryError> {
    let (key, noun) = (placed.key, placed.noun);
    let refuse = |key: &str, code: &str, why: String| refuse_as(noun, key, code, why);
    let Span {
        start,
        end,
        foot,
        head,
    } = placed.span;
    let width = less(end, start)?;
    for (face, at, wall_face, wall_at) in [
        ("jamb.start", start, "end0", 0),
        ("jamb.end", end, "end1", length),
        ("head", head, "top", height),
        ("sill", foot, "base", 0),
    ] {
        if near(at, wall_at, band) {
            return Err(refuse(
                key,
                "OpeningNearFace",
                format!(
                    "its {face} lies within the engine's tolerance ({band} steps) of the wall's \
                     {wall_face} without being on it"
                ),
            ));
        }
    }
    if start < 0 || end > length {
        return Err(refuse(
            key,
            "OpeningPastEnd",
            "it crosses the wall's end".to_owned(),
        ));
    }
    if start == 0 || end == length {
        return Err(refuse(
            key,
            "OpeningAtEnd",
            "it touches the wall's end".to_owned(),
        ));
    }
    if foot < 0 {
        return Err(refuse(
            key,
            "OpeningBelowBase",
            "it reaches below the wall's base".to_owned(),
        ));
    }
    if head >= height {
        return Err(refuse(
            key,
            "OpeningTooTall",
            "its head is at or above the wall's top, which leaves no wall over it".to_owned(),
        ));
    }
    let past = units.steps(PAST_MM)?;
    let (a, c) = if along == 1 { (1, 0) } else { (0, 1) };
    let mut lo = [0; 3];
    let mut hi = [0; 3];
    for (axis, (l, u)) in [
        (a, (start, end)),
        (c, (less(0, past)?, add(thickness, past)?)),
        (2, (if foot == 0 { less(0, past)? } else { foot }, head)),
    ] {
        if let (Some(lo), Some(hi)) = (lo.get_mut(axis), hi.get_mut(axis)) {
            (*lo, *hi) = (l, u);
        }
    }
    Ok(Cut {
        key: key.to_owned(),
        noun,
        door: placed.op,
        lo,
        hi,
        foot,
        head,
        start,
        end,
        stray: half_up(width).max(half_up(thickness)),
    })
}

/// Refuses two openings that overlap or touch, or come within `band` of each other.
fn apart(a: &Cut, b: &Cut, band: i64) -> Result<(), GeometryError> {
    let gap = |p: i64, q: i64| i128::from(p).saturating_sub(i128::from(q));
    let along = gap(b.start, a.end).max(gap(a.start, b.end));
    let up = gap(b.foot, a.head).max(gap(a.foot, b.head));
    let both = format!("{} and {} {}", a.key, b.noun, b.key);
    if along <= 0 && up <= 0 {
        let how = if along == 0 || up == 0 {
            "touch"
        } else {
            "overlap"
        };
        return Err(refuse_as(
            a.noun,
            &both,
            "OpeningsOverlap",
            format!("their openings {how}"),
        ));
    }
    if along.max(up) <= i128::from(band) {
        return Err(refuse_as(
            a.noun,
            &both,
            "OpeningNearFace",
            format!(
                "their openings come within the engine's tolerance ({band} steps) of each other \
                 without meeting"
            ),
        ));
    }
    Ok(())
}

impl Cut {
    /// The door's key.
    pub(super) fn key(&self) -> &str {
        &self.key
    }

    /// The cutter as the engine is asked for it: its plan at its foot, counter-clockwise from
    /// above, and the vector up to its head.
    pub(super) fn profile(&self) -> Result<(Profile, [i64; 3]), GeometryError> {
        let ([x0, y0, z0], [x1, y1, z1]) = (self.lo, self.hi);
        let p = |x: i64, y: i64| P3::new(x, y, z0).map_err(|_| too_big());
        let outer = vec![p(x0, y0)?, p(x1, y0)?, p(x1, y1)?, p(x0, y1)?];
        Ok((
            Profile {
                outer,
                holes: Vec::new(),
            },
            [0, 0, less(z1, z0)?],
        ))
    }

    /// The cutter's lowest and highest corners in its wall's frame, in steps: along the frame's
    /// along axis and across it in their plan axes' places, and up.
    pub(super) fn frame_box(&self) -> ([i64; 3], [i64; 3]) {
        (self.lo, self.hi)
    }

    /// The engine's error on this door's opening, with the door's key in front.
    pub(super) fn engine_error(&self, error: GeometryError) -> GeometryError {
        refuse_as(self.noun, &self.key, &error.code, error.message)
    }

    /// The roles of the faces this opening makes in its wall: no sill for a door to the floor.
    fn roles(&self) -> impl Iterator<Item = &'static str> {
        let sill = self.foot > 0;
        OPENING_ROLES
            .into_iter()
            .filter(move |role| sill || *role != "sill")
    }

    /// The persistent names of the faces this opening makes in the wall made by `wall`.
    pub(super) fn names(&self, wall: OpId) -> Result<Vec<FaceName>, GeometryError> {
        tool_names(
            &format!("{} {}", self.noun, self.key),
            &self.key,
            self.door,
            wall,
            self.roles(),
        )
    }
}

/// The persistent names of the faces the element `key` (`who` in a refusal), built by the
/// operation `own`, makes in the host built by `host`, one for each of `roles`, as chapter 11.9
/// names a tool's faces: `Created { op: host, role: tool(key), from: [Created { op: own, role }] }`,
/// shown `tool(<key>).<role>`. A door's opening and a floor's cut outlines are named so.
pub(super) fn tool_names<R: AsRef<str>>(
    who: &str,
    key: &str,
    own: OpId,
    host: OpId,
    roles: impl Iterator<Item = R>,
) -> Result<Vec<FaceName>, GeometryError> {
    let naming = |e: &dyn std::fmt::Debug| fault("Naming", format!("{who}: {e:?}"));
    let id = NameId::from_bytes(sixteen(key)?);
    let tool = Role::new("tool", [RoleArg::Name(id)]).map_err(|e| naming(&e))?;
    roles
        .map(|role| {
            let role = role.as_ref();
            let made_by = TopoName::Created {
                op: OpPath::new(own),
                role: Role::new(role, std::iter::empty()).map_err(|e| naming(&e))?,
                from: NameList::new([]),
            }
            .id()
            .map_err(|e| naming(&e))?;
            let made = TopoName::Created {
                op: OpPath::new(host),
                role: tool.clone(),
                from: NameList::new([made_by]),
            };
            Ok(FaceName {
                name: made.id().map_err(|e| naming(&e))?.to_string(),
                role: format!("tool({key}).{role}"),
            })
        })
        .collect()
}

/// How far the coarse level, the extrusion before `cuts` were made, strays from the wall with
/// them, at most, in whole millimetres rounded up: for each opening, half its width (no point of
/// the filled opening is farther than that from a jamb) or half the wall's thickness (no point of
/// a jamb, head or sill is farther than that from a side), whichever is the larger; none for no
/// openings.
pub(super) fn deviation(cuts: &[Cut], units: Units) -> i64 {
    cuts.iter()
        .map(|c| units.millimetres_up(c.stray))
        .max()
        .unwrap_or(0)
}
