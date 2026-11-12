//! Fuzz target: reference resolution (piece G89). On any element history, body and reference,
//! `resolve` gives the one outcome chapter 20.4's steps give, twice alike, and never reads the
//! reference's signature, kind or element.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: an [`Element`], a chain of up
//! to [`MAX_STEPS`] operation histories over up to [`MAX_FACES`] first faces, some of them weak,
//! made by `geom-test`'s history kit from the fates the bytes choose (every input unchanged,
//! modified, split, merged, deleted or deleted while generating, and outputs from nothing), every
//! name interned in the element's name graph, with the body after it (one name sometimes dropped,
//! so the history and the body disagree), carriers for some of its faces, and sometimes a pruned
//! stretch; and a reference to any of its names or to a stranger, from any step, for any want,
//! into its own document or a linked one at any version, with a lookup that finds the element,
//! finds it gone or misses the link. [`check`] resolves it, twice, and again with its signature,
//! kind and element changed, and compares the outcome with [`expected`], 20.4's steps written
//! here from the text over the records themselves. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included; [`decode`] reads at most
//!   [`MAX_INPUT_BYTES`] bytes, derived below, and a unit test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case. The
//!   laws: the same outcome twice; the same outcome whatever the signature, kind and element (20.4
//!   step 5); and the outcome 20.4's table gives: a name in the body is `Exact`; otherwise the walk
//!   follows modified images (never generated ones) step by step, and live descendants are
//!   `FollowedHistory`, several for one wanted `Ambiguous`, none `Deleted` by the step that removed
//!   the last, a pruned stretch on the way `HistoryPruned`, descendants the body lacks
//!   `HistoryBroken`, a name nothing holds `NameUnknown`; a weak name with no live descendant is
//!   `WeakChanged` when names of its base are in the body; a carrier is the one every piece lies
//!   on, `NoCarrier` without one; and a link is `LinkMissing`, or `LinkOlder` for a version before
//!   the reference's that lacks the name.
//!
//! Method: chapter 20.4's resolution (`projects/engines/geometry/architecture/20-naming-history.md`)
//! as `geom-name`'s `resolve` module header decides its open details, the oracle written here
//! from that text over the records rather than through `geom-op`'s relations, and chapter 20.3's
//! histories from `geom-test`'s kit, driven by coverage-guided fuzzing (28.5).
//!
//! Cost: at most [`MAX_STEPS`] steps of at most [`MAX_FACES`] inputs and their outputs, four
//! resolutions and one oracle walk; at most [`MAX_INPUT_BYTES`] bytes read.

use std::collections::{BTreeMap, BTreeSet};

use geom_name::resolve::Step;
use geom_name::{
    Body, CarrierIx, DocRef, ElementId, EntityKind, Lookup, NameGraph, Reference, Resolution,
    Signature, Target, UnresolvedReason, Want, resolve,
};
use geom_op::{
    History, NameId, NameList, NameSet, OpId, OpPath, Ordinal, Origin, PieceKey, Record, Role,
    TopoName,
};
use geom_test::history::{Fate, built};
use geom_test::{ByteSource, Entropy};

use crate::Finding;
use crate::cbor::hex;

/// The most first faces of an element, and the most inputs a step takes.
pub const MAX_FACES: u8 = 8;

/// The most first faces the bytes choose; up to [`MAX_FACES`] come from the steps.
pub const MAX_FIRST: u8 = 6;

/// The most operation steps in an element's history.
pub const MAX_STEPS: u8 = 4;

/// Bytes [`element`] reads: the face count, the weak mask, the step count, each step's new-output
/// count and [`MAX_FACES`] fates, the body's drop, a carrier byte per body name and the pruned
/// stretch: 2 + 1 + 4 · 9 + 1 + 8 + 1 = 49.
pub const ELEMENT_BYTES: usize =
    2 + 1 + MAX_STEPS as usize * (1 + MAX_FACES as usize) + 1 + MAX_FACES as usize + 1;

/// The most bytes [`decode`] reads: an element, then the reference's name, want, start, link,
/// two versions, lookup, four signature bytes, element and kind: 49 + 13 = 62. The test
/// `the_input_bound_is_reached_and_never_passed` builds that input.
pub const MAX_INPUT_BYTES: usize = ELEMENT_BYTES + 13;

const _: () = assert!(ELEMENT_BYTES == 49 && MAX_INPUT_BYTES == 62);

/// An element as the program holds it: its name graph, its history since its first faces, its
/// body and carriers, and every name it ever had.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    /// Every name of the element, interned.
    pub graph: NameGraph,
    /// The weak names among them.
    pub weak: BTreeSet<NameId>,
    /// Each step's history, oldest first, with a pruned stretch where the bytes put one.
    pub history: Vec<Step>,
    /// Each operation step's history and the inputs it took, oldest first, without the pruned
    /// stretch.
    pub steps: Vec<(OpPath, History, NameSet)>,
    /// The names of the body.
    pub body: NameSet,
    /// The carrier of each body name that has one, in name order.
    pub carriers: Vec<(NameId, CarrierIx)>,
    /// Every name the element ever had, in name order.
    pub names: Vec<NameId>,
}

/// One case: an element and a reference into it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The element.
    pub element: Element,
    /// The reference.
    pub reference: Reference,
    /// The step the reference was made at: it walks `element.history[from..]`.
    pub from: usize,
    /// What the lookup finds: the element with the linked version supplied, gone, or a missing
    /// link.
    pub lookup: LookupKind,
}

/// What the program's lookup gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupKind {
    /// The element, with the linked version supplied, if any.
    Found(Option<u64>),
    /// The element is gone.
    ElementGone,
    /// The link is missing.
    LinkMissing,
}

/// An operation path from a byte.
fn op(byte: u8) -> OpPath {
    OpPath::new(OpId::from_bytes([byte; 16]))
}

/// The element's names: each name the history kit makes, interned as a face of its own, so the
/// graph classifies every name.
struct Interned {
    graph: NameGraph,
    names: BTreeMap<NameId, NameId>,
}

impl Interned {
    /// The interned name standing for the kit's name `n`; a name already in the graph stands for
    /// itself.
    fn name(&mut self, n: NameId) -> Option<NameId> {
        if self.graph.name(n).is_some() {
            return Some(n);
        }
        if let Some(&id) = self.names.get(&n) {
            return Some(id);
        }
        let id = self
            .graph
            .intern(TopoName::Created {
                op: OpPath::new(OpId::from_bytes(n.to_bytes())),
                role: Role::new("face", []).ok()?,
                from: NameList::new([]),
            })
            .ok()?;
        self.names.insert(n, id);
        Some(id)
    }

    fn set(&mut self, s: &NameSet) -> Option<NameSet> {
        let names: Option<Vec<NameId>> = s.names().iter().map(|&n| self.name(n)).collect();
        NameSet::new(names?).ok()
    }

    fn origin(&mut self, origin: &Origin) -> Option<Origin> {
        Some(match origin {
            Origin::Unchanged => Origin::Unchanged,
            Origin::Modified { from } => Origin::Modified {
                from: self.set(from)?,
            },
            Origin::Generated { from, role } => Origin::Generated {
                from: self.set(from)?,
                role: role.clone(),
            },
            Origin::Split {
                of,
                piece: PieceKey::ByTools { tools, side },
            } => Origin::Split {
                of: self.name(*of)?,
                piece: PieceKey::ByTools {
                    tools: self.set(tools)?,
                    side: *side,
                },
            },
            Origin::Merged { of } => Origin::Merged { of: self.set(of)? },
        })
    }

    fn history(&mut self, history: &History) -> Option<History> {
        let mut records = Vec::new();
        for r in history.records() {
            records.push(Record {
                output: self.name(r.output)?,
                origin: self.origin(&r.origin)?,
            });
        }
        let deleted = self.set(history.deleted())?;
        History::new(records, deleted, []).ok()
    }
}

/// Reads an element from `s` (see the module documentation); `None` only if the history kit or
/// the graph refuses what the bytes chose, which is a defect of one of them.
pub fn element(s: &mut ByteSource<'_>) -> Option<Element> {
    let first = (s.byte() % MAX_FIRST).saturating_add(1);
    let weak_mask = s.byte();
    let mut graph = NameGraph::new();
    let mut weak = BTreeSet::new();
    let mut current = Vec::new();
    for i in 0..first {
        let made = TopoName::Created {
            op: op(0),
            role: Role::new("side", [geom_op::RoleArg::Count(u64::from(i))]).ok()?,
            from: NameList::new([]),
        };
        // Faces after the first may be weak: told apart from the first by their ordinal.
        let name = match current.first() {
            Some(&base) if weak_mask.checked_shr(u32::from(i)).is_some_and(|m| m & 1 == 1) => {
                TopoName::Weak {
                    base,
                    ord: Ordinal::new(u32::from(i), u32::from(first)).ok()?,
                }
            }
            _ => made,
        };
        let is_weak = matches!(name, TopoName::Weak { .. });
        let id = graph.intern(name).ok()?;
        if is_weak {
            weak.insert(id);
        }
        current.push(id);
    }
    let mut interned = Interned {
        graph,
        names: BTreeMap::new(),
    };
    let mut names: BTreeSet<NameId> = current.iter().copied().collect();
    let count = s.byte() % MAX_STEPS.saturating_add(1);
    let mut steps = Vec::new();
    for k in 0..count {
        let new = s.byte() % 3;
        let fates: Vec<Fate> = (0..MAX_FACES)
            .map(|_| *Fate::ALL.get(usize::from(s.byte() % 6)).unwrap_or(&Fate::Unchanged))
            .collect();
        let (raw, _) = built(k, &current, fates.get(..current.len())?, new)?;
        let history = interned.history(&raw)?;
        let inputs = NameSet::new(current.iter().copied()).ok()?;
        current = history
            .records()
            .iter()
            .map(|r| r.output)
            .take(usize::from(MAX_FACES))
            .collect();
        names.extend(history.records().iter().map(|r| r.output));
        steps.push((op(k.saturating_add(1)), history, inputs));
    }
    let drop = s.byte();
    let mut body = current;
    if drop & 15 == 15
        && let Some(at) = usize::from(drop >> 4).checked_rem(body.len())
    {
        body.remove(at);
    }
    let mut carriers = Vec::new();
    for _ in 0..MAX_FACES {
        carriers.push(s.byte());
    }
    let mut body_set: Vec<NameId> = body;
    body_set.sort_unstable();
    let carriers: Vec<(NameId, CarrierIx)> = body_set
        .iter()
        .zip(carriers)
        .filter(|&(_, c)| c % 4 != 3)
        .map(|(&n, c)| (n, CarrierIx(u32::from(c % 3))))
        .collect();
    let mut history: Vec<Step> = steps
        .iter()
        .map(|(by, history, _)| Step::Recorded {
            by: by.clone(),
            history: history.clone(),
        })
        .collect();
    let pruned = s.byte();
    if pruned & 7 == 7
        && let Some(at) = usize::from(pruned >> 4).checked_rem(history.len().saturating_add(1))
    {
        let by = (pruned & 8 == 8).then(|| op(0xF0));
        history.insert(at, Step::Pruned { by });
    }
    Some(Element {
        graph: interned.graph,
        weak,
        history,
        steps,
        body: NameSet::new(body_set).ok()?,
        carriers,
        names: names.into_iter().collect(),
    })
}

/// A name no graph holds.
fn stranger(byte: u8) -> NameId {
    NameId::from_bytes([0xEE, byte, 0xEE, 0xEE, 0xEE, 0xEE, 0xEE, 0xEE, 0xEE, 0xEE, 0xEE, 0xEE, 0xEE, 0xEE, 0xEE, 0xEE])
}

/// The fuzzer's bytes as a case, and how many bytes were read (at most [`MAX_INPUT_BYTES`]);
/// `None` only if the history kit or the graph refuses what the bytes chose.
#[must_use]
pub fn decode(data: &[u8]) -> Option<(Case, usize)> {
    let mut s = ByteSource::new(data);
    let element = element(&mut s)?;
    let pick = s.byte();
    let count = u8::try_from(element.names.len()).unwrap_or(u8::MAX);
    let name = match pick.checked_rem(count) {
        Some(i) if pick < 240 => *element.names.get(usize::from(i))?,
        _ => stranger(pick),
    };
    let want = match s.byte() % 3 {
        0 => Want::One,
        1 => Want::AllPieces,
        _ => Want::Carrier,
    };
    let from = usize::from(s.byte())
        .checked_rem(element.history.len().saturating_add(1))
        .unwrap_or(0);
    let link = s.byte();
    let (made_at, supplied) = (u64::from(s.byte() % 4), u64::from(s.byte() % 4));
    let doc = (link % 2 == 1).then_some(DocRef {
        id: [link; 16],
        version: made_at,
    });
    let lookup = match s.byte() % 8 {
        0 => LookupKind::ElementGone,
        1 => LookupKind::LinkMissing,
        2 | 3 => LookupKind::Found(None),
        _ => LookupKind::Found(Some(supplied)),
    };
    let signature = Signature((0..4).map(|_| s.byte()).collect());
    let element_id = ElementId([s.byte(); 16]);
    let kind = match s.byte() % 4 {
        0 => EntityKind::Body,
        1 => EntityKind::Face,
        2 => EntityKind::Edge,
        _ => EntityKind::Vertex,
    };
    let read = data.len().saturating_sub(s.remaining());
    Some((
        Case {
            element,
            reference: Reference {
                doc,
                element: element_id,
                name,
                kind,
                want,
                signature,
            },
            from,
            lookup,
        },
        read,
    ))
}

/// `case`'s reference resolved, against its element as the lookup finds it.
fn resolved(case: &Case, reference: &Reference) -> Resolution {
    let e = &case.element;
    let history = e.history.get(case.from..).unwrap_or_default();
    let lookup = match case.lookup {
        LookupKind::ElementGone => Lookup::ElementGone,
        LookupKind::LinkMissing => Lookup::LinkMissing,
        LookupKind::Found(version) => Lookup::Found {
            body: Body {
                names: &e.body,
                carriers: &e.carriers,
                history,
                graph: &e.graph,
            },
            version,
        },
    };
    resolve(reference, &lookup)
}

/// The fuzz harness: the laws on `data` decoded as a case.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    let Some((case, _)) = decode(data) else {
        return Err(Finding::new("any bytes are a case", || hex(data)));
    };
    check(&case).map_err(|f| Finding {
        law: f.law,
        detail: format!("{}: {}", hex(data), f.detail),
    })
}

/// Holds the case to every law of the module header.
///
/// # Errors
///
/// A [`Finding`] naming the law the case broke.
pub fn check(case: &Case) -> Result<(), Finding> {
    let law = |law: &str, what: String| Finding::new(law, || format!("{case:?}: {what}"));
    let got = resolved(case, &case.reference);
    if resolved(case, &case.reference) != got {
        return Err(law("a reference resolves the same twice", format!("{got:?}")));
    }
    let mut other = case.reference.clone();
    other.signature = Signature(vec![0xA5; 9]);
    other.element = ElementId([0x5A; 16]);
    other.kind = match other.kind {
        EntityKind::Face => EntityKind::Vertex,
        _ => EntityKind::Face,
    };
    let changed = resolved(case, &other);
    if changed != got {
        return Err(law(
            "resolution never reads the signature, kind or element (20.4 step 5)",
            format!("{got:?}, then {changed:?}"),
        ));
    }
    let want = expected(case);
    if got != want {
        return Err(law(
            "a reference resolves as 20.4's steps give",
            format!("{got:?}, not {want:?}"),
        ));
    }
    Ok(())
}

/// An `Unresolved` outcome.
const fn unresolved(reason: UnresolvedReason) -> Resolution {
    Resolution::Unresolved { reason }
}

/// The outcome 20.4's steps give `case` (see the module documentation), computed from the records
/// themselves.
#[must_use]
pub fn expected(case: &Case) -> Resolution {
    let e = &case.element;
    let r = &case.reference;
    let version = match case.lookup {
        LookupKind::ElementGone => return unresolved(UnresolvedReason::ElementGone),
        LookupKind::LinkMissing => return unresolved(UnresolvedReason::LinkMissing),
        LookupKind::Found(version) => version,
    };
    let in_body = |n: NameId| e.body.names().contains(&n);
    let known = |n: NameId| e.names.contains(&n);
    if let (Some(doc), Some(v)) = (r.doc, version)
        && v < doc.version
        && !in_body(r.name)
    {
        return unresolved(UnresolvedReason::LinkOlder);
    }
    if !known(r.name) {
        return unresolved(UnresolvedReason::NameUnknown);
    }
    let weak = e.weak.contains(&r.name);
    if in_body(r.name) {
        return found(e, r.want, &[r.name], weak, true);
    }
    let steps = e.history.get(case.from..).unwrap_or_default();
    // The weak name's outcome when its walk leaves nothing live, or something the body lacks.
    let weak_or = |otherwise: Resolution| weak_changed(e, r.name).unwrap_or(otherwise);
    if steps.is_empty() {
        return weak_or(unresolved(UnresolvedReason::NameUnknown));
    }
    let mut current: BTreeSet<NameId> = BTreeSet::from([r.name]);
    let mut deleted_by = None;
    for step in steps {
        if current.is_empty() {
            break;
        }
        let (by, history) = match step {
            Step::Pruned { by } => return Resolution::HistoryPruned { by: by.clone() },
            Step::Kept {
                pruned_by, held, ..
            } if !held.names().contains(&r.name) => {
                return Resolution::HistoryPruned {
                    by: Some(pruned_by.clone()),
                };
            }
            Step::Recorded { by, history } | Step::Kept { by, history, .. } => (by, history),
        };
        let next: BTreeSet<NameId> = history
            .records()
            .iter()
            .filter(|rec| match &rec.origin {
                Origin::Unchanged => current.contains(&rec.output),
                Origin::Modified { from } | Origin::Merged { of: from } => {
                    from.names().iter().any(|n| current.contains(n))
                }
                Origin::Split { of, .. } => current.contains(of),
                Origin::Generated { .. } => false,
            })
            .map(|rec| rec.output)
            .collect();
        if next.is_empty() && history.deleted().names().iter().any(|n| current.contains(n)) {
            deleted_by = Some(by.clone());
        }
        current = next;
    }
    if current.is_empty() {
        return weak_or(match deleted_by {
            Some(by) => Resolution::Deleted { by },
            None => unresolved(UnresolvedReason::NameUnknown),
        });
    }
    let live: Vec<NameId> = current.into_iter().collect();
    if live.iter().any(|&n| !in_body(n)) {
        return weak_or(unresolved(UnresolvedReason::HistoryBroken));
    }
    found(e, r.want, &live, weak, false)
}

/// What a reference wanting `want` gets from the live names `names`, in name order.
fn found(e: &Element, want: Want, names: &[NameId], weak: bool, exact: bool) -> Resolution {
    let weak = weak || names.iter().any(|n| e.weak.contains(n));
    let set = || NameSet::new(names.iter().copied()).unwrap_or_default();
    let target = match want {
        Want::One => match names {
            [one] => Target::One(*one),
            _ => return Resolution::Ambiguous { candidates: set() },
        },
        Want::AllPieces => Target::Pieces(set()),
        Want::Carrier => {
            let mut on = BTreeSet::new();
            for n in names {
                match e.carriers.iter().find(|(c, _)| c == n) {
                    Some(&(_, c)) => on.insert(c),
                    None => return unresolved(UnresolvedReason::NoCarrier),
                };
            }
            match on.into_iter().collect::<Vec<_>>().as_slice() {
                [c] => Target::Carrier(*c),
                _ => return Resolution::Ambiguous { candidates: set() },
            }
        }
    };
    if exact {
        Resolution::Exact { target, weak }
    } else {
        Resolution::FollowedHistory { target, weak }
    }
}

/// `WeakChanged` with the body's names sharing `name`'s base, if `name` is weak and any do.
fn weak_changed(e: &Element, name: NameId) -> Option<Resolution> {
    let TopoName::Weak { base, .. } = e.graph.name(name)? else {
        return None;
    };
    let candidates: Vec<NameId> = e
        .body
        .names()
        .iter()
        .copied()
        .filter(|&n| {
            n == *base
                || matches!(e.graph.name(n), Some(TopoName::Weak { base: b, .. }) if b == base)
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    Some(Resolution::WeakChanged {
        candidates: NameSet::new(candidates).ok()?,
    })
}
