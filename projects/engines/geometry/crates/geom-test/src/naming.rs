//! Naming and reference fixtures (chapter 28.3; piece G141): parametric models with the expected
//! resolution of every stored reference after each upstream edit (20.4).
//!
//! What it does: a [`Model`] is a wall with door openings, built from its [`Params`] by the
//! kernel's own operations: the wall and each door a box (`geom_make::make_box`), the openings cut
//! by one `geom_bool::subtract`. Every operation runs under its own path, so a regeneration names
//! what it makes exactly as the first build did, and the model keeps its final body, its name graph
//! (every name each operation reported making, after the names it refers to) and each operation's
//! history. A [`Fixture`] is a model, the references stored against its final body (the way a
//! program stores a dimension's or a hosted element's), and its edits: a [`Edit::Regenerate`] with
//! other parameters, as an upstream feature edited and the model rebuilt, or an operation run after
//! the references were made ([`Edit::Then`]: a cut, a fill, a move, a mirror), or the element
//! deleted, or the history pruned. After each edit every reference is resolved with
//! `geom_name::resolve` and held to its [`Expect`]. [`fixtures`] is the catalogue, and [`report`]
//! writes every resolution as text, whose SHA-256 is a golden: the names compared on every target
//! and mode of the determinism matrix (28.13).
//!
//! The outcomes follow 20.4's steps. A regeneration records no history between the two builds, so
//! a reference resolves `Exact` when the rebuilt body holds its name and `Unresolved` with
//! `NameUnknown` when it does not: names are the contract (20.2), and a name the edit took away is
//! said, never guessed. An operation run after the reference was made records its history, so the
//! walk follows it: a split piece is `Ambiguous` for one entity and every piece for all of them, a
//! filled opening's reveal is `Deleted` by the fill.
//!
//! Guarantees:
//!
//! - **Real names.** Every name comes from the operations themselves, with the name nodes they
//!   report, so a fixture breaks when an operation's naming changes, never when a test's copy of it
//!   does.
//! - **Nothing resolves differently without saying so** (28.3): every reference after every edit has
//!   an expectation, and a run fails on the first that does not hold, naming the fixture, the edit
//!   and the reference.
//! - **Deterministic.** No randomness: the parameters are written in the catalogue.
//!
//! Method: chapter 28.3's naming fixtures and 20.4's resolution
//! (`projects/engines/geometry/architecture/28-testing.md`,
//! `projects/engines/geometry/architecture/20-naming-history.md`), on chapter 11.9's names of
//! booleans (`projects/engines/geometry/architecture/11-booleans.md`).
//!
//! Cost: each fixture builds its model once per regeneration: a few boxes and one boolean.

use core::fmt::{self, Write as _};

use geom_bool::combine::{Combined, subtract, unite};
use geom_bool::operand::{Operand, OperandKey, OperandKind};
use geom_make::{BoxInput, Made, Moved, make_box, mirror, move_by};
use geom_math::Axis;
use geom_name::resolve::Step;
use geom_name::{
    Body as Element, CarrierIx, ElementId, EntityKind, Lookup, NameGraph, Reference, Resolution,
    Signature, Target, UnresolvedReason, Want, resolve,
};
use geom_num::{Exactness, P3, Steps};
use geom_op::history::{History, Origin};
use geom_op::{EdgeOrVertex, PieceKey};
use geom_op::{NameId, NameList, NameSet, OpContext, OpId, OpPath, Role, RoleArg, TopoName};
use geom_run::{Budget, Exec};
use geom_topo::Body;

/// The operation that makes the wall.
const WALL_OP: [u8; 16] = [1; 16];
/// The operation that makes every door, one type: doors of one type share their names until a cut
/// qualifies them by their keys (11.9).
const DOOR_OP: [u8; 16] = [2; 16];
/// The cut of every opening.
const CUT_OP: [u8; 16] = [3; 16];
/// The wall's operand key.
const WALL_KEY: [u8; 16] = [1; 16];

/// A door: its key, where its left jamb stands, its width and its height, in grid steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Door {
    /// The door's key, its element's permanent ID.
    pub key: u8,
    /// Where its left jamb stands along the wall.
    pub x: i64,
    /// Its width.
    pub width: u64,
    /// Its height.
    pub height: u64,
}

/// A wall's parameters: its length, thickness and height, and its doors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Params {
    /// Along x.
    pub length: u64,
    /// Along y.
    pub thickness: u64,
    /// Along z.
    pub height: u64,
    /// The doors, each through the whole thickness, from the floor.
    pub doors: Vec<Door>,
}

/// A built model: its body, the names its operations made, and each operation's history.
#[derive(Clone, Debug)]
pub struct Model {
    /// The final body.
    pub body: Body,
    /// Every name the model's operations made, as they reported them, each after those it refers
    /// to.
    pub names: Vec<TopoName>,
    /// The last operation's history, over the names of its inputs.
    pub history: Option<History>,
}

/// Why a fixture could not run: an operation failed, or a name was not where the fixture says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NamingError {
    /// An operation refused, with the kernel's message.
    Op(String),
    /// A name the fixture picks is not in the model.
    NoSuchName(&'static str),
    /// A name could not be formed or interned.
    Name(String),
}

impl fmt::Display for NamingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamingError::Op(e) => write!(f, "an operation refused: {e}"),
            NamingError::NoSuchName(what) => write!(f, "the model has no {what}"),
            NamingError::Name(e) => write!(f, "a name: {e}"),
        }
    }
}

impl core::error::Error for NamingError {}

fn op_error(e: impl fmt::Display) -> NamingError {
    NamingError::Op(e.to_string())
}

fn ctx(op: [u8; 16]) -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes(op)), Budget::UNLIMITED)
}

fn point(x: i64, y: i64, z: i64) -> Result<P3, NamingError> {
    P3::new(x, y, z).map_err(op_error)
}

fn cuboid(op: [u8; 16], low: [i64; 3], size: [u64; 3]) -> Result<Made, NamingError> {
    let input = BoxInput {
        min: point(low[0], low[1], low[2])?,
        size: size.map(Steps),
    };
    Ok(make_box(&ctx(op), &input).map_err(op_error)?.result)
}

fn solid(key: [u8; 16], body: &Body) -> Operand<'_> {
    Operand {
        key: OperandKey(key),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

/// A door's operand key.
#[must_use]
pub const fn door_key(key: u8) -> [u8; 16] {
    [key; 16]
}

/// The name operation `op` gives what it creates by the plain `role`: a box's faces are `min.x`,
/// `max.x`, `min.y`, `max.y`, `min.z` and `max.z` (`geom_make`).
///
/// # Errors
///
/// [`NamingError::Name`] for a role the scheme refuses.
pub fn created(op: [u8; 16], role: &str) -> Result<TopoName, NamingError> {
    Ok(TopoName::Created {
        op: OpPath::new(OpId::from_bytes(op)),
        role: Role::new(role, []).map_err(|e| NamingError::Name(format!("{e:?}")))?,
        from: NameList::new([]),
    })
}

fn id_of(name: &TopoName) -> Result<NameId, NamingError> {
    name.id().map_err(|e| NamingError::Name(format!("{e:?}")))
}

/// The name of the wall's face `role`.
///
/// # Errors
///
/// As [`created`].
pub fn wall_face(role: &str) -> Result<NameId, NamingError> {
    id_of(&created(WALL_OP, role)?)
}

/// The name of door `key`'s reveal from its face `role`: the door's face, qualified by its key
/// (11.9).
///
/// # Errors
///
/// As [`created`].
pub fn reveal(op: [u8; 16], key: u8, role: &str) -> Result<NameId, NamingError> {
    let door_face = id_of(&created(DOOR_OP, role)?)?;
    id_of(&TopoName::Created {
        op: OpPath::new(OpId::from_bytes(op)),
        role: Role::new("tool", [RoleArg::Name(NameId::from_bytes(door_key(key)))])
            .map_err(|e| NamingError::Name(format!("{e:?}")))?,
        from: NameList::new([door_face]),
    })
}

/// Builds the wall of `params`: the wall, every door, and one cut of all of them.
///
/// # Errors
///
/// [`NamingError::Op`] if an operation refuses.
pub fn build(params: &Params) -> Result<Model, NamingError> {
    let wall = cuboid(
        WALL_OP,
        [0, 0, 0],
        [params.length, params.thickness, params.height],
    )?;
    let mut names = wall.names.clone();
    if params.doors.is_empty() {
        return Ok(Model {
            body: wall.body,
            names,
            history: None,
        });
    }
    let mut doors = Vec::new();
    for door in &params.doors {
        let made = cuboid(
            DOOR_OP,
            [door.x, -5, 0],
            [door.width, params.thickness.saturating_add(10), door.height],
        )?;
        // Doors of one type make the same names: the graph holds each once.
        for name in &made.names {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        doors.push((door.key, made.body));
    }
    let mut operands = vec![solid(WALL_KEY, &wall.body)];
    for (key, body) in &doors {
        operands.push(solid(door_key(*key), body));
    }
    let cut = subtract(
        &ctx(CUT_OP),
        &Exec::serial(),
        OperandKey(WALL_KEY),
        &operands,
    )
    .map_err(op_error)?
    .result;
    names.extend(cut.names.iter().cloned());
    Ok(Model {
        body: cut.body,
        names,
        history: cut.history,
    })
}

/// An operation run on a model after its references were made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum After {
    /// Cut a box made by operation `made_by` from the model, as operation `op` with operand key
    /// `key`.
    Cut {
        /// The operation.
        op: [u8; 16],
        /// The operation that makes the box.
        made_by: [u8; 16],
        /// The tool's key.
        key: u8,
        /// The box's lowest corner.
        low: [i64; 3],
        /// Its size.
        size: [u64; 3],
    },
    /// Fill a box made by operation `made_by` into the model, as operation `op` with operand key
    /// `key`.
    Fill {
        /// The operation.
        op: [u8; 16],
        /// The operation that makes the box.
        made_by: [u8; 16],
        /// The filler's key.
        key: u8,
        /// The box's lowest corner.
        low: [i64; 3],
        /// Its size.
        size: [u64; 3],
    },
    /// Move the model, as operation `op`.
    Move {
        /// The operation.
        op: [u8; 16],
        /// By how much.
        by: [i64; 3],
    },
    /// Mirror the model in the plane x = `at`, as operation `op`.
    Mirror {
        /// The operation.
        op: [u8; 16],
        /// Where the plane stands.
        at: i64,
    },
}

/// The model after `after`, with the step it recorded.
fn run_after(model: &Model, after: &After) -> Result<(Model, Step), NamingError> {
    let step = |op: [u8; 16], history: History| Step::Recorded {
        by: OpPath::new(OpId::from_bytes(op)),
        history,
    };
    let moved = Moved {
        body: &model.body,
        exactness: Exactness::Exact,
        accept: None,
    };
    let made = |op: [u8; 16], made: Made| -> Result<(Model, Step), NamingError> {
        let mut names = model.names.clone();
        names.extend(made.names);
        Ok((
            Model {
                body: made.body,
                names,
                history: Some(made.history.clone()),
            },
            step(op, made.history),
        ))
    };
    let combined =
        |op: [u8; 16], tool: Made, out: Combined| -> Result<(Model, Step), NamingError> {
            let mut names = model.names.clone();
            for name in tool.names.into_iter().chain(out.names) {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            let history = out.history.ok_or(NamingError::NoSuchName("history"))?;
            Ok((
                Model {
                    body: out.body,
                    names,
                    history: Some(history.clone()),
                },
                step(op, history),
            ))
        };
    match after {
        After::Cut {
            op,
            made_by,
            key,
            low,
            size,
        }
        | After::Fill {
            op,
            made_by,
            key,
            low,
            size,
        } => {
            let tool = cuboid(*made_by, *low, *size)?;
            let operands = [
                solid(WALL_KEY, &model.body),
                solid(door_key(*key), &tool.body),
            ];
            let out = if matches!(after, After::Cut { .. }) {
                subtract(&ctx(*op), &Exec::serial(), OperandKey(WALL_KEY), &operands)
            } else {
                unite(&ctx(*op), &Exec::serial(), &operands)
            }
            .map_err(op_error)?
            .result;
            combined(*op, tool, out)
        }
        After::Move { op, by } => made(
            *op,
            move_by(&ctx(*op), moved, *by).map_err(op_error)?.result,
        ),
        After::Mirror { op, at } => made(
            *op,
            mirror(&ctx(*op), moved, Axis::X, point(*at, 0, 0)?)
                .map_err(op_error)?
                .result,
        ),
    }
}

/// An edit after the references were made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    /// An upstream feature edited: the model rebuilt from these parameters, with no history
    /// between the builds.
    Regenerate(Params),
    /// An operation run on the model as it stands.
    Then(After),
    /// The element deleted.
    ElementGone,
    /// The history since the references were made pruned (24.4).
    Pruned,
}

/// What a reference must resolve to after an edit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expect {
    /// Its own name, in the body: `Exact` and not weak.
    Exact,
    /// Its own name, in the body, and weak: the interface asks for a check (20.4 step 3).
    ExactWeak,
    /// One live descendant through the history, not weak: this one, if given.
    Followed(Option<NameId>),
    /// Every piece, through the history: this many.
    Pieces(usize),
    /// The carrier its entity, and every piece, lies on: the surface of the face named, if given.
    Carrier(Option<NameId>),
    /// Several descendants, and the reference wants one: this many.
    Ambiguous(usize),
    /// A weak name gone while its base still names this many entities.
    WeakChanged(usize),
    /// Deleted by this operation.
    Deleted([u8; 16]),
    /// Unresolved, for this reason.
    Unresolved(UnresolvedReason),
    /// The walk needs pruned history.
    Pruned,
    /// A known defect: what resolves today, the outcome wanted, and why they differ. The run holds
    /// it to `today`, so a fix shows as a failure to be recorded, as a fixture's `Known` turns
    /// `Improved` (28.16).
    Known {
        /// What resolves today.
        today: Box<Expect>,
        /// What should.
        wanted: Box<Expect>,
        /// The defect, and where it is.
        why: &'static str,
    },
}

/// A reference a fixture stores: what it is, the name, its kind and what it wants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
    /// What it stands for, in words.
    pub what: &'static str,
    /// The name.
    pub name: NameId,
    /// Its kind.
    pub kind: EntityKind,
    /// What it wants.
    pub want: Want,
}

/// One fixture: its id, the model, the references and the edits, each with what every reference
/// must resolve to after it.
#[derive(Clone, Debug)]
pub struct Fixture {
    /// Its id, `naming/NNNN-slug` (28.16).
    pub id: &'static str,
    /// What it shows.
    pub about: &'static str,
    /// The model's parameters.
    pub params: Params,
    /// The references, made against the model as built.
    pub references: Vec<Stored>,
    /// Each edit, with what each reference resolves to after it, in the references' order.
    pub edits: Vec<(Edit, Vec<Expect>)>,
}

/// The element every reference names.
const ELEMENT: ElementId = ElementId([7; 16]);

/// The carrier of each face of `body`: the surface's row, the index identity 20.1 gives it.
fn carriers(body: &Body) -> Vec<(NameId, CarrierIx)> {
    let mut out = Vec::new();
    if let Some(names) = body.names() {
        let arenas = body.arenas();
        for (row, key) in names.faces.iter().enumerate() {
            let surface = u32::try_from(row)
                .ok()
                .and_then(geom_topo::Ix::new)
                .and_then(|face| arenas.faces.surface.get(face))
                .map(|s| s.row());
            if let Some(surface) = surface {
                out.push((NameId::from_bytes(key.0), CarrierIx(surface)));
            }
        }
    }
    out.sort_unstable();
    out
}

/// Every name `body` holds.
fn held(body: &Body) -> Result<NameSet, NamingError> {
    let names = body.names().ok_or(NamingError::NoSuchName("names"))?;
    NameSet::new(
        [&names.regions, &names.faces, &names.edges, &names.vertices]
            .into_iter()
            .flatten()
            .map(|key| NameId::from_bytes(key.0)),
    )
    .map_err(|e| NamingError::Name(format!("{e:?}")))
}

/// A graph of `names`.
fn graph_of(names: &[TopoName]) -> Result<NameGraph, NamingError> {
    let mut graph = NameGraph::new();
    for name in names {
        graph
            .intern(name.clone())
            .map_err(|e| NamingError::Name(format!("{e:?}")))?;
    }
    Ok(graph)
}

/// Each reference of `fixture` resolved after each of its edits, in order.
///
/// # Errors
///
/// [`NamingError`] if the model cannot be built or an edit cannot run.
pub fn resolutions(fixture: &Fixture) -> Result<Vec<Vec<Resolution>>, NamingError> {
    let base = build(&fixture.params)?;
    let mut out = Vec::new();
    let mut model = base.clone();
    let mut steps: Vec<Step> = Vec::new();
    for (edit, _) in &fixture.edits {
        let (now, lookup_steps, gone) = match edit {
            Edit::Regenerate(params) => (build(params)?, Vec::new(), false),
            Edit::Then(after) => {
                let (next, step) = run_after(&model, after)?;
                steps.push(step);
                model = next.clone();
                (next, steps.clone(), false)
            }
            Edit::ElementGone => (model.clone(), Vec::new(), true),
            Edit::Pruned => (model.clone(), vec![Step::Pruned { by: None }], false),
        };
        // The document keeps every name it has held: those the references were made against too.
        let graph = graph_of(&[base.names.as_slice(), now.names.as_slice()].concat())?;
        let names = held(&now.body)?;
        let carriers = carriers(&now.body);
        let element = Element {
            names: &names,
            carriers: &carriers,
            history: &lookup_steps,
            graph: &graph,
        };
        let lookup = if gone {
            Lookup::ElementGone
        } else {
            Lookup::Found {
                body: element,
                version: None,
            }
        };
        out.push(
            fixture
                .references
                .iter()
                .map(|r| {
                    resolve(
                        &Reference {
                            doc: None,
                            element: ELEMENT,
                            name: r.name,
                            kind: r.kind,
                            want: r.want,
                            signature: Signature::default(),
                        },
                        &lookup,
                    )
                })
                .collect(),
        );
    }
    Ok(out)
}

/// Whether `got` is what `expect` says, `carriers` being the body's after the edit.
fn holds(expect: &Expect, got: &Resolution, carriers: &[(NameId, CarrierIx)]) -> bool {
    let carrier_of = |name: &NameId| carriers.iter().find(|(n, _)| n == name).map(|(_, c)| *c);
    match (expect, got) {
        (Expect::Exact, Resolution::Exact { weak: false, .. })
        | (Expect::ExactWeak, Resolution::Exact { weak: true, .. })
        | (Expect::Pruned, Resolution::HistoryPruned { .. }) => true,
        (
            Expect::Followed(name),
            Resolution::FollowedHistory {
                target: Target::One(n),
                weak: false,
            },
        ) => name.is_none_or(|name| *n == name),
        (
            Expect::Pieces(count),
            Resolution::FollowedHistory {
                target: Target::Pieces(set),
                ..
            },
        ) => set.names().len() == *count,
        (
            Expect::Carrier(face),
            Resolution::Exact {
                target: Target::Carrier(c),
                ..
            }
            | Resolution::FollowedHistory {
                target: Target::Carrier(c),
                ..
            },
        ) => face.is_none_or(|face| carrier_of(&face) == Some(*c)),
        (Expect::Ambiguous(count), Resolution::Ambiguous { candidates })
        | (Expect::WeakChanged(count), Resolution::WeakChanged { candidates }) => {
            candidates.names().len() == *count
        }
        (Expect::Deleted(op), Resolution::Deleted { by }) => {
            *by == OpPath::new(OpId::from_bytes(*op))
        }
        (Expect::Unresolved(reason), Resolution::Unresolved { reason: r }) => reason == r,
        (Expect::Known { today, .. }, got) => holds(today, got, carriers),
        _ => false,
    }
}

/// Every expectation of `fixture` that does not hold, as text naming the edit and the reference;
/// empty when every one holds.
///
/// # Errors
///
/// As [`resolutions`].
pub fn misses(fixture: &Fixture) -> Result<Vec<String>, NamingError> {
    let got = resolutions(fixture)?;
    let mut out = Vec::new();
    let mut model = build(&fixture.params)?;
    for ((at, (edit, expects)), resolved) in fixture.edits.iter().enumerate().zip(&got) {
        let now = match edit {
            Edit::Regenerate(params) => build(params)?,
            Edit::Then(after) => {
                model = run_after(&model, after)?.0;
                model.clone()
            }
            Edit::ElementGone | Edit::Pruned => model.clone(),
        };
        let carriers = carriers(&now.body);
        for ((reference, expect), resolution) in
            fixture.references.iter().zip(expects).zip(resolved)
        {
            if let Expect::Known { wanted, .. } = expect
                && holds(wanted, resolution, &carriers)
            {
                out.push(format!(
                    "{} edit {at}: {} resolved as wanted, {resolution:?}: the known defect is \
                     fixed, so record it",
                    fixture.id, reference.what
                ));
            } else if !holds(expect, resolution, &carriers) {
                out.push(format!(
                    "{} edit {at}: {} resolved {resolution:?}, not {expect:?}",
                    fixture.id, reference.what
                ));
            }
        }
        if expects.len() != fixture.references.len() {
            out.push(format!(
                "{} edit {at}: {} expectations for {} references",
                fixture.id,
                expects.len(),
                fixture.references.len()
            ));
        }
    }
    Ok(out)
}

/// Every fixture's every resolution as text, one line each: the bytes whose SHA-256 the golden
/// `geom-test/naming-fixtures` holds, so every target of the determinism matrix gives the same
/// names (28.13).
///
/// # Errors
///
/// As [`resolutions`].
pub fn report(fixtures: &[Fixture]) -> Result<String, NamingError> {
    let mut out = String::new();
    for fixture in fixtures {
        for (at, resolved) in resolutions(fixture)?.iter().enumerate() {
            for (reference, resolution) in fixture.references.iter().zip(resolved) {
                writeln!(
                    out,
                    "{} {at} {}: {resolution:?}",
                    fixture.id, reference.what
                )
                .map_err(|e| NamingError::Name(e.to_string()))?;
            }
        }
    }
    Ok(out)
}

/// The name of the face of a door of the one type by `role`.
fn door_face(role: &str) -> Result<NameId, NamingError> {
    id_of(&created(DOOR_OP, role)?)
}

/// What operation `op` makes in role `role` of the names `args`, sorted as `geom_make` sorts them.
fn named(op: [u8; 16], role: &str, args: &[NameId]) -> Result<NameId, NamingError> {
    let mut args = args.to_vec();
    args.sort_unstable();
    id_of(&TopoName::Created {
        op: OpPath::new(OpId::from_bytes(op)),
        role: Role::new(role, args.into_iter().map(RoleArg::Name))
            .map_err(|e| NamingError::Name(format!("{e:?}")))?,
        from: NameList::new([]),
    })
}

/// Whichever of `names` the body of `model` holds: the box's own name for an entity the cut kept,
/// or the cut's for one it bounded anew.
fn held_of(model: &Model, names: &[NameId], what: &'static str) -> Result<NameId, NamingError> {
    let body = held(&model.body)?;
    names
        .iter()
        .copied()
        .find(|n| body.names().contains(n))
        .ok_or(NamingError::NoSuchName(what))
}

/// The wall's edge between its faces `a` and `b`, ended by the faces `ends`, as `model` names it:
/// the box's `edge` if the cut kept it, or `Bounded` by those faces if it named it anew.
fn wall_edge(
    model: &Model,
    a: &str,
    b: &str,
    ends: &[&str],
    what: &'static str,
) -> Result<NameId, NamingError> {
    let between = [wall_face(a)?, wall_face(b)?];
    let ends: Vec<NameId> = ends
        .iter()
        .map(|r| wall_face(r))
        .collect::<Result<_, _>>()?;
    let set = |names: &[NameId]| {
        NameSet::new(names.iter().copied()).map_err(|e| NamingError::Name(format!("{e:?}")))
    };
    let bounded = id_of(&TopoName::Bounded {
        kind: EdgeOrVertex::Edge,
        by: set(&between)?,
        ends: set(&ends)?,
    })?;
    held_of(model, &[named(WALL_OP, "edge", &between)?, bounded], what)
}

/// The wall's corner where its faces `a`, `b` and `c` meet, as `model` names it.
fn wall_corner(model: &Model, faces: [&str; 3], what: &'static str) -> Result<NameId, NamingError> {
    let by: Vec<NameId> = faces
        .iter()
        .map(|r| wall_face(r))
        .collect::<Result<_, _>>()?;
    let set = |names: &[NameId]| {
        NameSet::new(names.iter().copied()).map_err(|e| NamingError::Name(format!("{e:?}")))
    };
    let bounded = id_of(&TopoName::Bounded {
        kind: EdgeOrVertex::Vertex,
        by: set(&by)?,
        ends: set(&[])?,
    })?;
    held_of(model, &[named(WALL_OP, "corner", &by)?, bounded], what)
}

/// The pieces the last cut of `model` split the wall's face `role` into: each name, and the names
/// of the door faces that bound it.
fn pieces(model: &Model, role: &str) -> Result<Vec<(NameId, Vec<NameId>)>, NamingError> {
    let of = wall_face(role)?;
    let history = model
        .history
        .as_ref()
        .ok_or(NamingError::NoSuchName("history"))?;
    Ok(history
        .records()
        .iter()
        .filter_map(|r| match &r.origin {
            Origin::Split {
                of: from,
                piece: PieceKey::ByTools { tools, .. },
            } if *from == of => Some((r.output, tools.names().to_vec())),
            _ => None,
        })
        .collect())
}

/// The piece of the wall's face `role` bounded by door faces of every role in `with` and of none in
/// `without`: the first in name order where several are (weak names told apart).
fn piece(
    model: &Model,
    role: &str,
    with: &[&str],
    without: &[&str],
    what: &'static str,
) -> Result<NameId, NamingError> {
    let with: Vec<NameId> = with
        .iter()
        .map(|r| door_face(r))
        .collect::<Result<_, _>>()?;
    let without: Vec<NameId> = without
        .iter()
        .map(|r| door_face(r))
        .collect::<Result<_, _>>()?;
    let mut found: Vec<NameId> = pieces(model, role)?
        .into_iter()
        .filter(|(_, tools)| {
            with.iter().all(|t| tools.contains(t)) && !without.iter().any(|t| tools.contains(t))
        })
        .map(|(name, _)| name)
        .collect();
    found.sort_unstable();
    found.first().copied().ok_or(NamingError::NoSuchName(what))
}

const fn door(key: u8, x: i64, height: u64) -> Door {
    Door {
        key,
        x,
        width: 20,
        height,
    }
}

/// A wall 300 long, 10 thick and 60 high, with two doors 20 wide and 40 high: its exterior face
/// keeps its name, notched; its floor face is split three ways.
fn doors() -> Params {
    Params {
        length: 300,
        thickness: 10,
        height: 60,
        doors: vec![door(11, 20, 40), door(12, 120, 40)],
    }
}

/// The same wall with three openings taller than it: every long face is split four ways, and the
/// two pieces between openings are bounded alike, so they take weak names.
fn openings() -> Params {
    Params {
        doors: vec![door(11, 20, 80), door(12, 120, 80), door(13, 220, 80)],
        ..doors()
    }
}

const fn stored(what: &'static str, name: NameId, kind: EntityKind, want: Want) -> Stored {
    Stored {
        what,
        name,
        kind,
        want,
    }
}

/// The references stored against [`doors`]: faces, a reveal of each door, a floor piece, an edge
/// and a corner.
fn door_references(model: &Model) -> Result<Vec<Stored>, NamingError> {
    use EntityKind::{Edge, Face, Vertex};
    Ok(vec![
        stored("the exterior face", wall_face("min.y")?, Face, Want::One),
        stored(
            "the exterior face's plane",
            wall_face("min.y")?,
            Face,
            Want::Carrier,
        ),
        stored(
            "door 11's left reveal",
            reveal(CUT_OP, 11, "min.x")?,
            Face,
            Want::One,
        ),
        stored(
            "door 12's head",
            reveal(CUT_OP, 12, "max.z")?,
            Face,
            Want::One,
        ),
        stored("the top face", wall_face("max.z")?, Face, Want::One),
        stored("the far end", wall_face("max.x")?, Face, Want::One),
        stored(
            "the floor left of the first door",
            piece(model, "min.z", &["min.x"], &["max.x"], "left floor piece")?,
            Face,
            Want::One,
        ),
        stored(
            "the top back edge",
            wall_edge(
                model,
                "max.y",
                "max.z",
                &["max.x", "min.x"],
                "top back edge",
            )?,
            Edge,
            Want::One,
        ),
        stored(
            "the far top back corner",
            wall_corner(model, ["max.x", "max.y", "max.z"], "far top back corner")?,
            Vertex,
            Want::One,
        ),
    ])
}

/// The expectations for [`door_references`] where nothing the references name changes.
fn door_kept() -> Result<Vec<Expect>, NamingError> {
    Ok(vec![
        Expect::Exact,
        Expect::Carrier(Some(wall_face("min.y")?)),
        Expect::Exact,
        Expect::Exact,
        Expect::Exact,
        Expect::Exact,
        Expect::Exact,
        Expect::Exact,
        Expect::Exact,
    ])
}

/// [`doors`] with door `key` changed by `change`.
fn with_door(key: u8, change: impl Fn(&mut Door)) -> Params {
    let mut params = doors();
    for door in &mut params.doors {
        if door.key == key {
            change(door);
        }
    }
    params
}

/// A cut, as operation `op` with key `key`, of a box its own operation makes.
fn cut(op: u8, key: u8, low: [i64; 3], size: [u64; 3]) -> Edit {
    Edit::Then(After::Cut {
        op: [op; 16],
        made_by: [op.wrapping_add(100); 16],
        key,
        low,
        size,
    })
}

/// The references stored against [`openings`]: the exterior piece left of the first opening, three
/// ways, and a piece between two openings, whose name is weak.
fn opening_references(model: &Model) -> Result<Vec<Stored>, NamingError> {
    let left = piece(
        model,
        "min.y",
        &["min.x"],
        &["max.x"],
        "left exterior piece",
    )?;
    let between = piece(
        model,
        "min.y",
        &["min.x", "max.x"],
        &[],
        "middle exterior piece",
    )?;
    Ok(vec![
        stored(
            "the exterior left of opening 11",
            left,
            EntityKind::Face,
            Want::One,
        ),
        stored("every piece of it", left, EntityKind::Face, Want::AllPieces),
        stored("its plane", left, EntityKind::Face, Want::Carrier),
        stored(
            "the exterior between two openings",
            between,
            EntityKind::Face,
            Want::One,
        ),
    ])
}

/// `base` with the entries of `changes` put in their places.
fn changed(base: &[Expect], changes: &[(usize, Expect)]) -> Vec<Expect> {
    let mut out = base.to_vec();
    for (at, expect) in changes {
        if let Some(slot) = out.get_mut(*at) {
            *slot = expect.clone();
        }
    }
    out
}

/// The catalogue: every fixture, its names taken from the models as first built.
///
/// # Errors
///
/// [`NamingError`] if a model cannot be built or holds no name a fixture picks.
#[allow(
    clippy::too_many_lines,
    reason = "the catalogue: one entry per fixture, each with its edits and expectations"
)]
pub fn fixtures() -> Result<Vec<Fixture>, NamingError> {
    let refs_a = door_references(&build(&doors())?)?;
    let refs_b = opening_references(&build(&openings())?)?;
    let kept = door_kept()?;
    let unknown = Expect::Unresolved(UnresolvedReason::NameUnknown);
    let fixture = |id, about, params: Params, references: &Vec<Stored>, edits| Fixture {
        id,
        about,
        params,
        references: references.clone(),
        edits,
    };
    let slot = cut(26, 33, [8, -5, -5], [4, 20, 90]);
    let split = vec![
        Expect::Ambiguous(2),
        Expect::Pieces(2),
        Expect::Carrier(None),
        Expect::ExactWeak,
    ];
    Ok(vec![
        fixture(
            "naming/0001-a-door-moves-along-the-wall",
            "Moving a door upstream renames nothing: its reveals are named by its key and its \
             face, the wall's faces by the wall.",
            doors(),
            &refs_a,
            vec![(Edit::Regenerate(with_door(11, |d| d.x = 40)), kept.clone())],
        ),
        fixture(
            "naming/0002-the-wall-grows-longer",
            "A longer wall keeps every name: its faces are named by role, not by size.",
            doors(),
            &refs_a,
            vec![(
                Edit::Regenerate(Params {
                    length: 360,
                    ..doors()
                }),
                kept.clone(),
            )],
        ),
        fixture(
            "naming/0003-a-door-grows-taller",
            "A taller door that stays under the top keeps every name.",
            doors(),
            &refs_a,
            vec![(
                Edit::Regenerate(with_door(12, |d| d.height = 50)),
                kept.clone(),
            )],
        ),
        fixture(
            "naming/0004-a-door-removed",
            "A door taken out upstream takes its reveals with it, said as an unknown name; the \
             floor piece left of the first door keeps its name, now left of the other, since a \
             piece is named by the faces that bound it, not by which door they belong to. The \
             wall's untouched edge and corner keep the box's names with two doors of one type or \
             one (G132's names are per name).",
            doors(),
            &refs_a,
            vec![(
                Edit::Regenerate(Params {
                    doors: vec![door(12, 120, 40)],
                    ..doors()
                }),
                changed(&kept, &[(2, unknown.clone())]),
            )],
        ),
        fixture(
            "naming/0005-every-door-removed",
            "With no doors there is no cut, and the wall's names are the box's own: every wall \
             reference resolves, every reveal and floor piece is unknown.",
            doors(),
            &refs_a,
            vec![(
                Edit::Regenerate(Params {
                    doors: Vec::new(),
                    ..doors()
                }),
                changed(
                    &kept,
                    &[
                        (2, unknown.clone()),
                        (3, unknown.clone()),
                        (6, unknown.clone()),
                    ],
                ),
            )],
        ),
        fixture(
            "naming/0006-a-third-door-renames-nothing",
            "A third door renames nothing it does not cut (11.9, G132).",
            doors(),
            &refs_a,
            vec![(
                Edit::Regenerate(Params {
                    doors: vec![door(11, 20, 40), door(12, 120, 40), door(13, 220, 40)],
                    ..doors()
                }),
                kept.clone(),
            )],
        ),
        fixture(
            "naming/0007-a-window-is-a-hole-not-a-split",
            "A window cut after the references holes the exterior face, which stays one face and \
             keeps its name.",
            doors(),
            &refs_a,
            vec![(cut(20, 30, [60, -5, 45], [30, 20, 10]), kept.clone())],
        ),
        fixture(
            "naming/0008-a-filled-opening-deletes-its-reveals",
            "Filling door 11's opening after the references deletes its reveals by the fill, the \
             faces the filler joins follow the history to the merged face, and the edge along the \
             merged back face, bounded anew, is deleted by the fill.",
            doors(),
            &refs_a,
            vec![(
                Edit::Then(After::Fill {
                    op: [22; 16],
                    made_by: [122; 16],
                    key: 31,
                    low: [20, 0, 0],
                    size: [20, 10, 40],
                }),
                changed(
                    &kept,
                    &[
                        (0, Expect::Followed(None)),
                        (1, Expect::Carrier(None)),
                        (2, Expect::Deleted([22; 16])),
                        (6, Expect::Followed(None)),
                        // The back face merges with the filler's, so the edge between it and the
                        // top is bounded anew: a renamed edge is generated (20.2, 20.3).
                        (7, Expect::Deleted([22; 16])),
                    ],
                ),
            )],
        ),
        fixture(
            "naming/0009-moving-and-mirroring-keep-every-name",
            "A move and then a mirror of the element keep every name: a transform makes none.",
            doors(),
            &refs_a,
            vec![
                (
                    Edit::Then(After::Move {
                        op: [40; 16],
                        by: [100, 0, 0],
                    }),
                    kept.clone(),
                ),
                (
                    Edit::Then(After::Mirror {
                        op: [41; 16],
                        at: 500,
                    }),
                    kept.clone(),
                ),
            ],
        ),
        fixture(
            "naming/0010-an-end-cut-off-deletes-the-end",
            "Cutting the far end off after the references deletes the end face and its corner by \
             the cut; the shortened faces and edge keep their names.",
            doors(),
            &refs_a,
            vec![(
                cut(24, 32, [280, -5, -5], [100, 20, 80]),
                changed(
                    &kept,
                    &[
                        (5, Expect::Deleted([24; 16])),
                        (8, Expect::Deleted([24; 16])),
                    ],
                ),
            )],
        ),
        fixture(
            "naming/0011-a-later-slot-splits-a-piece",
            "A slot cut after the references splits the exterior piece left of the first opening: \
             one wanted is ambiguous between the two, every piece gives both, and the plane is the \
             one they share. A weak name elsewhere stays weak.",
            openings(),
            &refs_b,
            vec![(slot.clone(), split.clone())],
        ),
        fixture(
            "naming/0012-a-weak-name-changes-with-its-set",
            "Taking the third opening out upstream leaves one piece between openings, no longer \
             told apart: the weak name is gone while its base names it, so a person or AI \
             confirms.",
            openings(),
            &refs_b,
            vec![(
                Edit::Regenerate(Params {
                    doors: vec![door(11, 20, 80), door(12, 120, 80)],
                    ..openings()
                }),
                vec![
                    Expect::Exact,
                    Expect::Exact,
                    Expect::Carrier(None),
                    Expect::WeakChanged(1),
                ],
            )],
        ),
        fixture(
            "naming/0013-a-deleted-element-and-pruned-history",
            "After a slot splits a piece, the element deleted leaves every reference unresolved, \
             and a pruned history leaves the split piece's walk pruned, never deleted (24.4).",
            openings(),
            &refs_b,
            vec![
                (slot, split),
                (
                    Edit::ElementGone,
                    vec![Expect::Unresolved(UnresolvedReason::ElementGone); 4],
                ),
                (
                    Edit::Pruned,
                    vec![
                        Expect::Pruned,
                        Expect::Pruned,
                        Expect::Pruned,
                        Expect::ExactWeak,
                    ],
                ),
            ],
        ),
    ])
}
