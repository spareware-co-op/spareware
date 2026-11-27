//! A modify asked of a selection: which records it writes, as [`Plan`]. The elements named, every
//! element hosted on one of them (it follows its host: [`crate::kinds::follower`]), and each named
//! element whose host is not named (it moves alone, along its host, or is refused by name).
//!
//! A copy is the same motion applied to a copy of each: a new ID made from the key the command
//! names (`<key>.<n>`, `n` counting the copied elements from 1 in the order of their IDs, hosted
//! ones included; an array's copy `k` is `<key>.<k>.<n>`), the same type, parameters and name,
//! except where a name must stay unique: a grid's or a named reference plane's is the next by
//! construct-grids' rule, and a room's number the next free one. A hosted element copied with its
//! host is hosted on the host's copy; one copied alone stays on its own host.
//!
//! Everything here is a pure function of the document's records and the request, so the same
//! request on the same model gives the same records on every machine.

use crate::error::ModifyError;
use crate::exact::Motion;
use crate::kinds::{self, Sense, What};
use construct_model::element::Element;
use construct_regen::hosting::{self, Hosting};
use kit_core::Id;
use std::collections::{BTreeMap, BTreeSet};

/// The most records one command makes.
pub const MOST_MADE: usize = 10_000;

/// What is asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// The selection moved by one motion.
    Move(Motion),
    /// Copies of the selection.
    Copy(Copies),
}

/// Copies of a selection: one for each motion, each made from the originals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Copies {
    /// What the copies' IDs are made from.
    pub key: String,
    /// One motion per copy; more than one is an array, whose IDs count the copies.
    pub motions: Vec<Motion>,
}

/// What a request writes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// Records rewritten, in the order of their IDs.
    pub changed: Vec<Element>,
    /// Records made (the copies), in the order made.
    pub made: Vec<Element>,
    /// The hosts whose hosted elements must still fit them: every host moved with what it hosts,
    /// every copy of one, and the host of each element moved or copied alone.
    pub hosts: BTreeSet<Id>,
}

/// How a piece of the selection takes part.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Role {
    /// Moved as itself.
    Own,
    /// Hosted on an element that moves: it follows.
    Follows(Hosting),
    /// Hosted on an element that does not move: it moves alone.
    Alone(Hosting),
}

/// The record of `id`, or the refusal.
fn record<'a>(document: &'a BTreeMap<Id, Element>, id: &Id) -> Result<&'a Element, ModifyError> {
    document.get(id).ok_or_else(|| ModifyError::Missing(id.clone()))
}

/// Every piece the request moves and how, in the order of their IDs.
fn pieces(
    document: &BTreeMap<Id, Element>,
    selection: &[Id],
) -> Result<BTreeMap<Id, Role>, ModifyError> {
    if selection.is_empty() {
        return Err(ModifyError::Nothing);
    }
    let named: BTreeSet<&Id> = selection.iter().collect();
    let mut roles = BTreeMap::new();
    for id in &named {
        let found = record(document, id)?;
        let role = match kinds::what(found)? {
            What::OnFace(hosting) | What::InPlan(hosting) => {
                if named.contains(hosting.host()) {
                    Role::Follows(hosting)
                } else {
                    Role::Alone(hosting)
                }
            }
            _ => Role::Own,
        };
        roles.insert((*id).clone(), role);
    }
    let owners: BTreeSet<Id> = roles
        .iter()
        .filter(|(_, role)| **role == Role::Own)
        .map(|(id, _)| id.clone())
        .collect();
    for (id, other) in document {
        if roles.contains_key(id) {
            continue;
        }
        let Ok(Some(hosting)) = hosting::read(other) else {
            continue;
        };
        if !owners.contains(hosting.host()) {
            continue;
        }
        if matches!(hosting, Hosting::ByPosition { .. }) {
            return Err(ModifyError::ByPosition {
                element: other.name.clone(),
            });
        }
        roles.insert(id.clone(), Role::Follows(hosting));
    }
    Ok(roles)
}

/// The host's name, for a refusal.
fn host_name(document: &BTreeMap<Id, Element>, host: &Id) -> String {
    document
        .get(host)
        .map_or_else(|| host.to_string(), |record| record.name.clone())
}

/// What `piece` becomes under `motion`, given how it takes part: `None` when its record does
/// not change.
fn under(
    document: &BTreeMap<Id, Element>,
    piece: &Element,
    role: &Role,
    motion: &Motion,
    sense: &dyn Sense,
) -> Result<Option<Element>, ModifyError> {
    match role {
        Role::Own | Role::Follows(Hosting::InPlan { .. }) | Role::Alone(Hosting::InPlan { .. }) => {
            kinds::moved(piece, motion, sense).map(Some)
        }
        Role::Follows(_) => kinds::follower(piece, motion, sense),
        Role::Alone(hosting) => {
            let host = record(document, hosting.host())?;
            let Hosting::OnFace { along, .. } = hosting else {
                return Err(ModifyError::ByPosition {
                    element: piece.name.clone(),
                });
            };
            let Some(by) = motion.translation() else {
                return Err(ModifyError::TurnsWithHost {
                    element: piece.name.clone(),
                    host: host_name(document, hosting.host()),
                });
            };
            kinds::along_host(piece, host, *along, by).map(Some)
        }
    }
}

/// What `request` writes in `document` (every record of the model, by ID) when it is asked of
/// the elements `selection`.
///
/// # Errors
/// The first refusal, naming its element: one not in the model, one the modify tools do not move,
/// a point off the grid, a hosted element moved off its host or turned alone, and the refusals of
/// the crates that own each kind's form; for copies, a key that makes no ID, an ID already taken,
/// copies that would lie on their originals, and more than [`MOST_MADE`] records.
pub fn plan(
    document: &BTreeMap<Id, Element>,
    selection: &[Id],
    request: &Request,
    sense: &dyn Sense,
) -> Result<Plan, ModifyError> {
    let roles = pieces(document, selection)?;
    match request {
        Request::Move(motion) => moved(document, &roles, motion, sense),
        Request::Copy(copies) => copied(document, &roles, copies, sense),
    }
}

/// The host each piece is checked against, if it is hosted, and whether it hosts what follows it.
fn host_of(role: &Role) -> Option<&Id> {
    match role {
        Role::Own => None,
        Role::Follows(hosting) | Role::Alone(hosting) => Some(hosting.host()),
    }
}

fn moved(
    document: &BTreeMap<Id, Element>,
    roles: &BTreeMap<Id, Role>,
    motion: &Motion,
    sense: &dyn Sense,
) -> Result<Plan, ModifyError> {
    let mut plan = Plan::default();
    for (id, role) in roles {
        let piece = record(document, id)?;
        if let Some(host) = host_of(role) {
            plan.hosts.insert(host.clone());
        }
        if let Some(after) = under(document, piece, role, motion, sense)?
            && after != *piece
        {
            plan.changed.push(after);
        }
    }
    Ok(plan)
}

/// The ID of copy `k` (from 1) of the piece numbered `n` (from 1).
fn copy_id(key: &str, k: usize, n: usize, array: bool) -> Result<Id, ModifyError> {
    let text = if array {
        format!("{key}.{k}.{n}")
    } else {
        format!("{key}.{n}")
    };
    Id::parse(&text).map_err(|_| ModifyError::CopyKey {
        key: key.to_owned(),
    })
}

/// Names that must stay unique, as copies take them.
struct Unique {
    grids: Vec<String>,
    planes: Vec<String>,
    numbers: Vec<String>,
}

impl Unique {
    fn of(document: &BTreeMap<Id, Element>) -> Unique {
        let named = |kind: &str| {
            document
                .values()
                .filter(|record| record.kind == kind && !record.name.is_empty())
                .map(|record| record.name.clone())
                .collect()
        };
        Unique {
            grids: named(construct_grids::GRID_KIND),
            planes: named(construct_grids::PLANE_KIND),
            numbers: document
                .values()
                .filter(|record| construct_rooms::is_room(record))
                .filter_map(construct_rooms::number_of)
                .map(str::to_owned)
                .collect(),
        }
    }

    /// `copy` with the name or number it must have to stay unique, taken.
    fn take(&mut self, copy: &mut Element) {
        let next = |taken: &[String], after: &str| {
            construct_grids::suggest(Some(after), |name| {
                taken
                    .iter()
                    .any(|other| construct_grids::same_name(other, name))
            })
        };
        if copy.kind == construct_grids::GRID_KIND {
            copy.name = next(&self.grids, &copy.name);
            self.grids.push(copy.name.clone());
        } else if copy.kind == construct_grids::PLANE_KIND && !copy.name.is_empty() {
            copy.name = next(&self.planes, &copy.name);
            self.planes.push(copy.name.clone());
        } else if construct_rooms::is_room(copy) {
            let number = construct_rooms::next_number(self.numbers.iter().map(String::as_str));
            copy.props
                .insert(construct_rooms::NUMBER_KEY.to_owned(), number.clone());
            self.numbers.push(number);
        }
    }
}

/// `copy` hosted on `host` instead of on `was`: its face reference names the host's copy.
fn hosted_on(copy: &mut Element, was: &Id, host: &Id) {
    for reference in &mut copy.references {
        if reference.kind == hosting::FACE_KIND && reference.doc.is_none() && reference.element == *was
        {
            reference.element = host.clone();
        }
    }
}

fn copied(
    document: &BTreeMap<Id, Element>,
    roles: &BTreeMap<Id, Role>,
    copies: &Copies,
    sense: &dyn Sense,
) -> Result<Plan, ModifyError> {
    if copies.motions.iter().any(Motion::is_identity) {
        return Err(ModifyError::InPlace);
    }
    let count = roles
        .len()
        .checked_mul(copies.motions.len())
        .ok_or(ModifyError::TooMany { most: MOST_MADE })?;
    if count > MOST_MADE {
        return Err(ModifyError::TooMany { most: MOST_MADE });
    }
    let array = copies.motions.len() > 1;
    let mut unique = Unique::of(document);
    let mut plan = Plan::default();
    for (k, motion) in copies.motions.iter().enumerate() {
        let k = k.saturating_add(1);
        let mut ids = BTreeMap::new();
        for (n, id) in roles.keys().enumerate() {
            let new = copy_id(&copies.key, k, n.saturating_add(1), array)?;
            if document.contains_key(&new) {
                return Err(ModifyError::Taken { id: new });
            }
            ids.insert(id.clone(), new);
        }
        for (id, role) in roles {
            let piece = record(document, id)?;
            let mut copy =
                under(document, piece, role, motion, sense)?.unwrap_or_else(|| piece.clone());
            let Some(new) = ids.get(id) else {
                continue;
            };
            copy.id = new.clone();
            match role {
                Role::Own => {
                    if roles
                        .values()
                        .any(|other| matches!(other, Role::Follows(h) if h.host() == id))
                    {
                        plan.hosts.insert(new.clone());
                    }
                }
                Role::Follows(hosting) => {
                    if let Some(host) = ids.get(hosting.host()) {
                        hosted_on(&mut copy, hosting.host(), host);
                    }
                }
                Role::Alone(hosting) => {
                    plan.hosts.insert(hosting.host().clone());
                }
            }
            unique.take(&mut copy);
            plan.made.push(copy);
        }
    }
    Ok(plan)
}
