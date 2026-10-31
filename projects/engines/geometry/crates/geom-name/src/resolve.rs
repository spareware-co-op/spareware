//! References and how they resolve (chapter 20.4): a reference names an entity of an element, and
//! resolving it against the element's current body and the history recorded since gives one value
//! of the closed [`Resolution`] type.
//!
//! What it does: a [`Reference`] holds the document it points into ([`DocRef`], or none for the
//! reference's own document), the element, the entity's [`NameId`], its [`EntityKind`], what it
//! wants ([`Want`]: one entity, every piece, or the carrier), and the stored [`Signature`].
//! [`resolve`] answers it from a [`Lookup`]: the element's [`Body`] as the program has it (its
//! current names, the carrier of each face and edge that has one, the history recorded since the
//! reference was made, oldest first, and the name graph), or why the element cannot be read. It
//! takes 20.4's steps in order:
//!
//! 1. The name is in the current body: `Exact`.
//! 2. Otherwise the walk follows the recorded modified images (splits and merges included, as
//!    `geom_op::history` counts them) through each step since. Live descendants resolve as
//!    `FollowedHistory`: the one, or every piece if the reference wants them all; several for a
//!    reference that wants one are `Ambiguous`. A walk left with no descendant is `Deleted` by the
//!    step that removed the last of them; a walk that needs a pruned step is `HistoryPruned`.
//! 3. A weak name resolves with `weak` set. A weak name left with no live descendant while names
//!    that share its base are in the body is `WeakChanged`, with those names as candidates.
//! 4. A reference that wants the carrier resolves to the carrier its entity, or every live
//!    descendant, lies on.
//! 5. The signature is never read (below).
//! 6. Anything else is `Unresolved` with its [`UnresolvedReason`], as model state.
//!
//! A reference into another document resolves the same way inside the linked version the program
//! supplies ([`Lookup::Found`] with that version): none supplied is `Unresolved` with `LinkMissing`,
//! and an older version that does not yet hold the name is `Unresolved` with `LinkOlder`.
//!
//! Guarantees:
//!
//! - **No geometric guessing (20.4 step 5).** [`resolve`] reads a reference's document, name and
//!   want, and never its signature, kind or element: the signature only suggests a repair in the
//!   interface, and an accepted repair is a new operation in the log. `tests/attack_g89.rs` changes
//!   each of them and finds every result unchanged, and checks that this module's code never reads
//!   the signature.
//! - **Model state, the same on every machine.** Every outcome is computed from names and recorded
//!   history alone, every set of names is in name order, and nothing depends on an address, a hash
//!   map's order or the machine.
//! - **Pruned is never Deleted.** A walk that reaches a pruned step while it still has names to
//!   follow answers `HistoryPruned`, whatever the later steps hold (24.4).
//! - **Generated is not followed.** Only modified images are descendants: an entity generated from
//!   the referenced one is a new entity, not the referenced one in a new form.
//! - **Unknown is never guessed.** A name the graph cannot classify, or that no recorded step and
//!   not the body holds, is `Unresolved` with `NameUnknown`.
//! - **No recursion.** The walk is one loop over the steps, holding a sorted vector of names.
//!
//! Decisions here, where 20.4 leaves the detail open:
//!
//! - Each recorded history covers the whole body after its step (every output is listed, an
//!   unchanged one as its own image), so a name with no modified image in a step was deleted there
//!   if the step lists it as deleted, and was not in the body at that step otherwise.
//! - A weak name left with no live descendant is `WeakChanged` when a body name shares its base
//!   (a `Weak` name with the same base, or the base itself); otherwise it resolves as any other
//!   name would. The check follows the walk, so a weak name whose history still leads somewhere
//!   follows it, with `weak` set. A name built on a `Weak` node at depth is weak but has no base of
//!   its own, so it resolves as any other name, with `weak` set.
//! - A carrier reference whose live descendants lie on different carriers is `Ambiguous` with those
//!   descendants; one whose entity, or any live descendant, has no carrier (a vertex) is
//!   `Unresolved` with `NoCarrier`, whatever the others lie on.
//! - A walk whose recorded descendants the body does not hold is `Unresolved` with
//!   `HistoryBroken`: the history and the body disagree, and neither is preferred.
//! - `NoCarrier`, `NameUnknown` and `HistoryBroken` join 20.4's table of reasons.
//! - The carrier is a [`CarrierIx`], an index identity (20.1) into the body version's geometry,
//!   which the caller's body reports; it becomes the topology's typed index when that lands.
//!
//! Method: chapter 20.4's resolution, with the history contract of 20.3 as `geom_op::history`
//! carries it (piece G88; `OpenCascade`'s `BRepTools_History` contract, adopted as a tested
//! invariant), all in `projects/engines/geometry/architecture/20-naming-history.md`.
//!
//! Cost: for s recorded steps, r records per step and m names in the walk's vector,
//! O(s · r log(r m)), searched by bisection, never with hashed sets; the weak checks visit the name graph's reachable nodes
//! once per name asked about, and the base candidates scan the body's names once. Recursion: none.

use alloc::vec::Vec;

use geom_op::history::History;
use geom_op::name::{NameSet, TopoName};
use geom_op::{NameId, OpPath};

use crate::graph::NameGraph;

/// A linked document and the version a reference was made against (chapter 20.1): the document's
/// permanent 128-bit ID and the version number the program gave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocRef {
    /// The linked document's permanent ID.
    pub id: [u8; 16],
    /// The version the reference was made against.
    pub version: u64,
}

/// The program's element an entity belongs to: its permanent 128-bit ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementId(pub [u8; 16]);

/// The kind of entity a reference names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    /// A whole body.
    Body,
    /// A face.
    Face,
    /// An edge.
    Edge,
    /// A vertex.
    Vertex,
}

/// What a reference asks for (20.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Want {
    /// One entity: several candidates are `Ambiguous`.
    One,
    /// Every piece the entity became.
    AllPieces,
    /// The plane, axis or surface the entity lies on, which its pieces share; the default for
    /// dimensions and hosting.
    Carrier,
}

/// The geometric signature stored with a reference: bytes the interface may use to suggest a
/// repair, which resolution never reads (20.4 step 5).
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Signature(pub Vec<u8>);

/// A reference to an entity (20.4).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Reference {
    /// The linked document, or `None` for the reference's own document.
    pub doc: Option<DocRef>,
    /// The element the entity belongs to.
    pub element: ElementId,
    /// The entity's persistent name.
    pub name: NameId,
    /// The kind of entity.
    pub kind: EntityKind,
    /// What the reference asks for.
    pub want: Want,
    /// The signature stored with it, for repair suggestions only.
    pub signature: Signature,
}

/// The index of a carrier (a plane, axis or surface) in a body version's geometry: an index
/// identity (20.1), in memory only, as the caller's body reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CarrierIx(pub u32);

/// What a resolved reference gives: what its `want` asked for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    /// One entity.
    One(NameId),
    /// Every piece, in name order.
    Pieces(NameSet),
    /// The carrier every piece lies on.
    Carrier(CarrierIx),
}

/// Why a reference is unresolved: the reasons of 20.4's table. Other chapters add reasons, never
/// outcomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnresolvedReason {
    /// The element itself no longer exists (20.4).
    ElementGone,
    /// The linked document or file is not available, or this kernel cannot read its version
    /// (20.4).
    LinkMissing,
    /// The linked version supplied is older than the one the reference was made against, and does
    /// not yet hold the name (20.4).
    LinkOlder,
    /// The target lies outside the drawing's view (18.8).
    NotInView,
    /// A plan position falls outside a surface's boundary, or in a hole or void (17.12).
    OffSurface,
    /// A station lies beyond the end of the alignment (17.12).
    PastEnd,
    /// The reference wants a carrier, and its entity has none, such as a vertex (20.4, G89).
    NoCarrier,
    /// The name is not in the body, in any recorded step or (where asked) in the name graph
    /// (20.4, G89).
    NameUnknown,
    /// The recorded history leads to names the current body does not hold (20.4, G89).
    HistoryBroken,
}

/// How a reference resolved (20.4): one value of a closed type, so programs and AI agents can
/// handle each case.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Resolution {
    /// Step 1: the name is in the current body. `weak` if the name is weak.
    Exact {
        /// What the reference asked for.
        target: Target,
        /// Whether the name, or a result, is weak.
        weak: bool,
    },
    /// Step 2: the recorded history leads to one live descendant, or to every piece.
    FollowedHistory {
        /// What the reference asked for.
        target: Target,
        /// Whether the name, or a result, is weak.
        weak: bool,
    },
    /// Step 2: several live descendants, and the reference wants one.
    Ambiguous {
        /// The descendants, in name order.
        candidates: NameSet,
    },
    /// Step 3: a weak name is gone while its base still names entities; a person or AI confirms.
    WeakChanged {
        /// The body's names that share the base, in name order.
        candidates: NameSet,
    },
    /// Step 2: the history leaves no descendant.
    Deleted {
        /// The step that removed the last descendant.
        by: OpPath,
    },
    /// Step 2: the walk needs history that was pruned (24.4).
    HistoryPruned {
        /// The `PruneHistory` operation, or `None` for a linked version shared without its
        /// history.
        by: Option<OpPath>,
    },
    /// None of the above: the reason says why.
    Unresolved {
        /// Why.
        reason: UnresolvedReason,
    },
}

/// One step of the history recorded since a reference was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// An operation step and the history it recorded.
    Recorded {
        /// The step.
        by: OpPath,
        /// What it did to every entity of the body.
        history: History,
    },
    /// A stretch of history that was pruned (24.4).
    Pruned {
        /// The `PruneHistory` operation, or `None` for a version shared without its history.
        by: Option<OpPath>,
    },
}

/// The element as the program has it: the body a reference resolves against.
#[derive(Clone, Copy, Debug)]
pub struct Body<'a> {
    /// The names in the element's current body.
    pub names: &'a NameSet,
    /// The carrier of each face and edge that has one, in name order, each name once.
    pub carriers: &'a [(NameId, CarrierIx)],
    /// The history recorded since the reference was made, oldest first.
    pub history: &'a [Step],
    /// The name graph that holds the body's names and the referenced name.
    pub graph: &'a NameGraph,
}

/// What the program can supply for a reference's element.
#[derive(Clone, Copy, Debug)]
pub enum Lookup<'a> {
    /// The element's body: in the reference's own document, or in the linked document at
    /// `version` (for a reference with a [`DocRef`]; `None` when the program does not say).
    Found {
        /// The body.
        body: Body<'a>,
        /// The linked version supplied.
        version: Option<u64>,
    },
    /// The element no longer exists.
    ElementGone,
    /// The linked document, or a version of it this kernel can read, is not available.
    LinkMissing,
}

/// Resolves `reference` against `lookup` (20.4; see the module documentation).
#[must_use]
pub fn resolve(reference: &Reference, lookup: &Lookup<'_>) -> Resolution {
    let body = match lookup {
        Lookup::ElementGone => return unresolved(UnresolvedReason::ElementGone),
        Lookup::LinkMissing => return unresolved(UnresolvedReason::LinkMissing),
        Lookup::Found { body, version } => {
            let present = holds(body.names.names(), reference.name);
            if let (Some(doc), Some(supplied)) = (reference.doc, version)
                && *supplied < doc.version
                && !present
            {
                return unresolved(UnresolvedReason::LinkOlder);
            }
            *body
        }
    };
    let Ok(weak) = body.graph.is_weak(reference.name) else {
        return unresolved(UnresolvedReason::NameUnknown);
    };
    // Step 1.
    if holds(body.names.names(), reference.name) {
        return found(reference.want, &[reference.name], weak, &body, true);
    }
    // Step 2.
    let live = match walk(reference.name, body.history) {
        Walk::Live(names) => names,
        Walk::Pruned(by) => return Resolution::HistoryPruned { by },
        Walk::Deleted(by) => {
            return weak_changed(reference.name, &body).unwrap_or(Resolution::Deleted { by });
        }
        Walk::Unknown => {
            return weak_changed(reference.name, &body)
                .unwrap_or(unresolved(UnresolvedReason::NameUnknown));
        }
    };
    if live.iter().any(|&n| !holds(body.names.names(), n)) {
        return weak_changed(reference.name, &body)
            .unwrap_or(unresolved(UnresolvedReason::HistoryBroken));
    }
    found(reference.want, &live, weak, &body, false)
}

/// An `Unresolved` outcome.
const fn unresolved(reason: UnresolvedReason) -> Resolution {
    Resolution::Unresolved { reason }
}

/// Whether the sorted `names` hold `name`.
fn holds(names: &[NameId], name: NameId) -> bool {
    names.binary_search(&name).is_ok()
}

/// The set of `names`, which are distinct and in name order already (so `NameSet::new` refuses
/// nothing).
fn set_of(names: &[NameId]) -> NameSet {
    NameSet::new(names.iter().copied()).unwrap_or_default()
}

/// What the walk through the recorded history found.
enum Walk {
    /// Names still descended from the referenced one after every step, in name order, at least
    /// one.
    Live(Vec<NameId>),
    /// The last descendants were deleted by this step.
    Deleted(OpPath),
    /// A pruned step was needed.
    Pruned(Option<OpPath>),
    /// No recorded step held the name, and it is not in the body.
    Unknown,
}

/// Follows `name`'s modified images through `history`, oldest step first (module documentation).
fn walk(name: NameId, history: &[Step]) -> Walk {
    if history.is_empty() {
        // No step at all, and the name is not in the body.
        return Walk::Unknown;
    }
    let mut current: Vec<NameId> = Vec::from([name]);
    let mut deleted_by: Option<OpPath> = None;
    for step in history {
        if current.is_empty() {
            break;
        }
        match step {
            Step::Pruned { by } => return Walk::Pruned(by.clone()),
            Step::Recorded { by, history } => {
                let relations = history.relations();
                let mut next: Vec<NameId> = Vec::new();
                for (from, to) in relations.modified() {
                    if holds(&current, from) {
                        next.push(to);
                    }
                }
                next.sort_unstable();
                next.dedup();
                let deleted_here = relations.deleted().any(|d| holds(&current, d));
                if next.is_empty() && deleted_here {
                    deleted_by = Some(by.clone());
                }
                current = next;
            }
        }
    }
    if current.is_empty() {
        return deleted_by.map_or(Walk::Unknown, Walk::Deleted);
    }
    Walk::Live(current)
}

/// The resolution of `want` over the live names `names` (one or more, in name order): `Exact` for
/// step 1, `FollowedHistory` for step 2, or the outcome the carrier or the count calls for.
fn found(want: Want, names: &[NameId], weak: bool, body: &Body<'_>, exact: bool) -> Resolution {
    let mut weak = weak;
    for &n in names {
        match body.graph.is_weak(n) {
            Ok(w) => weak |= w,
            Err(_) => return unresolved(UnresolvedReason::NameUnknown),
        }
    }
    let target = match want {
        Want::One => match names {
            [one] => Target::One(*one),
            _ => {
                return Resolution::Ambiguous {
                    candidates: set_of(names),
                };
            }
        },
        Want::AllPieces => Target::Pieces(set_of(names)),
        Want::Carrier => {
            // Every name needs a carrier before any two are compared, so the outcome never
            // depends on the names' order.
            let mut on: Vec<CarrierIx> = Vec::new();
            for &n in names {
                let Some(c) = carrier_of(body.carriers, n) else {
                    return unresolved(UnresolvedReason::NoCarrier);
                };
                on.push(c);
            }
            on.sort_unstable();
            on.dedup();
            match on.as_slice() {
                [c] => Target::Carrier(*c),
                _ => {
                    return Resolution::Ambiguous {
                        candidates: set_of(names),
                    };
                }
            }
        }
    };
    if exact {
        Resolution::Exact { target, weak }
    } else {
        Resolution::FollowedHistory { target, weak }
    }
}

/// The carrier `carriers` gives `name`.
fn carrier_of(carriers: &[(NameId, CarrierIx)], name: NameId) -> Option<CarrierIx> {
    carriers
        .binary_search_by_key(&name, |&(n, _)| n)
        .ok()
        .and_then(|i| carriers.get(i))
        .map(|&(_, c)| c)
}

/// `WeakChanged` with the body's names that share `name`'s base, if `name` is a `Weak` node and any
/// do (module documentation).
fn weak_changed(name: NameId, body: &Body<'_>) -> Option<Resolution> {
    let Some(TopoName::Weak { base, .. }) = body.graph.name(name) else {
        return None;
    };
    let candidates: Vec<NameId> = body
        .names
        .names()
        .iter()
        .copied()
        .filter(|&n| {
            n == *base
                || matches!(body.graph.name(n), Some(TopoName::Weak { base: b, .. }) if b == base)
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    Some(Resolution::WeakChanged {
        candidates: set_of(&candidates),
    })
}
