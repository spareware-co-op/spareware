//! Fuzz target: persistent names (piece G87). Every name of any shape gets the one `NameId` its
//! canonical bytes hash to, the name graph keeps each once and knows exactly which are weak, the
//! duplicate detector and the reference pick are exact, and colliding entities get the ordinals of
//! 20.2's order whatever order they come in.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: up to [`MAX_NAMES`] names of
//! every kind of `TopoName`, each built on earlier names of the case or on a stranger the graph
//! never holds, through every slot a name has (a role's name argument, a keyed role's key, the
//! names it is made from, a split's input and tools, a merge's inputs, an edge's faces, a weak
//! name's base); a list of the names with repeats; up to [`MAX_SETS`] sets of them; and up to
//! [`MAX_ENTITIES`] colliding entities, each with the three keys of 20.2's rule, any of which may
//! be undecided. [`check`] interns the names in a `NameGraph` and holds each to its laws, asks the
//! graph which are weak, runs `first_duplicate`, `expect_unique`, `reference`, `ordinals` and
//! `weak_names`, and compares every answer with an oracle written here from the rule. [`harness`]
//! is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included; [`decode`] reads at most
//!   [`MAX_INPUT_BYTES`] bytes, derived below, and a unit test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case. The laws:
//!   a name's ID is the first 16 bytes of SHA-256 over `NameScheme v1`'s prefix and its canonical
//!   bytes, and the graph interns it under that ID with those bytes, once, however often it is
//!   given; the graph lists its IDs in byte order, each once; a name is weak exactly when a `Weak`
//!   node is reachable through its parents, and a parent the graph does not hold is `Missing`,
//!   never assumed strong (when both a weak node and a stranger are reachable, the walk may meet
//!   either first, and only those two answers are allowed); the lowest repeated name is the
//!   duplicate, and `expect_unique` refuses exactly then in a build with debug assertions; the
//!   reference is the lowest name every set holds; and the ordinals are each entity's rank under
//!   the rule's three steps, the same for the entities reversed, unless some pair is undecided or
//!   ties on every step, when the answer is `UndecidedOrder` naming such a pair.
//!
//! Method: chapter 20.1 and 20.2's names, the name graph and the ordinal order
//! (`projects/engines/geometry/architecture/20-naming-history.md`) and decision 32.19's hashing
//! (`32-decisions.md`), each oracle written here from the text, driven by coverage-guided
//! fuzzing (28.5).
//!
//! Cost: at most [`MAX_NAMES`] names, each encoded and hashed twice and interned twice; a walk
//! of the graph per name; an O(n²) oracle over at most [`MAX_ENTITIES`] entities, sorted twice;
//! at most [`MAX_INPUT_BYTES`] bytes read.

use core::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use geom_name::{
    GraphError, NameGraph, OrderError, WeakOrder, first_duplicate, ordinals, reference, weak_names,
};
use geom_op::hash::sha256_128;
use geom_op::{
    EdgeOrVertex, NAME_SCHEME_V1_PREFIX, NameId, NameList, NameSet, OpId, OpPath, Ordinal,
    PieceKey, Role, RoleArg, Side, TopoName,
};
use geom_test::{ByteSource, Entropy};

use crate::Finding;
use crate::cbor::hex;

/// The most names a case interns.
pub const MAX_NAMES: u8 = 8;

/// The most names in the duplicate list.
pub const MAX_LIST: u8 = 12;

/// The most sets the reference is picked from.
pub const MAX_SETS: u8 = 3;

/// The most colliding entities.
pub const MAX_ENTITIES: u8 = 8;

/// Bytes per name: its kind, its operation, one argument and two parents.
const NAME_BYTES: usize = 5;

/// Bytes per entity: its least point, its vertices and its direction.
const ENTITY_BYTES: usize = 3;

/// The most bytes [`decode`] reads: 1 + 8 · 5 + 1 + 12 + 1 + 3 + 1 + 8 · 3 = 83.
///
/// The name count, each name's five bytes; the list's length and its entries; the set count and
/// each set's mask; the entity count and each entity's three bytes. The test
/// `the_input_bound_is_reached_and_never_passed` builds that input.
pub const MAX_INPUT_BYTES: usize = 1
    + MAX_NAMES as usize * NAME_BYTES
    + 1
    + MAX_LIST as usize
    + 1
    + MAX_SETS as usize
    + 1
    + MAX_ENTITIES as usize * ENTITY_BYTES;

const _: () = assert!(MAX_INPUT_BYTES == 83);

/// A name's parent: an earlier name of the case, or a stranger no graph holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parent {
    /// The name at this position, before the one that refers to it.
    Earlier(u8),
    /// A name the graph is never given, by its byte.
    Stranger(u8),
}

/// One name to build: its kind, its operation's byte, one argument byte and two parents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Draw {
    /// Which `TopoName` it is.
    pub kind: u8,
    /// The byte its operation's ID is made from.
    pub op: u8,
    /// A count, a side, an ordinal or a choice of role, by kind.
    pub arg: u8,
    /// The names it is built on.
    pub parents: [Parent; 2],
}

/// A colliding entity's three keys under 20.2's rule; `None` is undecided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    /// Its least point on the reference.
    pub least: Option<(u8, u8)>,
    /// Its vertex list, sorted.
    pub vertices: Option<Vec<u8>>,
    /// The direction it leaves a shared least point in.
    pub direction: Option<u8>,
}

impl WeakOrder for Key {
    fn least_point(&self, other: &Self) -> Option<Ordering> {
        Some(self.least?.cmp(&other.least?))
    }

    fn vertices(&self, other: &Self) -> Option<Ordering> {
        Some(self.vertices.as_ref()?.cmp(other.vertices.as_ref()?))
    }

    fn direction(&self, other: &Self) -> Option<Ordering> {
        Some(self.direction?.cmp(&other.direction?))
    }
}

/// One case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The names, in the order they are interned.
    pub names: Vec<Draw>,
    /// The duplicate list, by position in `names`.
    pub list: Vec<u8>,
    /// The sets the reference is picked from, each a mask over `names`.
    pub sets: Vec<u8>,
    /// The colliding entities.
    pub keys: Vec<Key>,
}

/// A parent from its byte, for the name at position `at`: one in eight is a stranger.
fn parent(byte: u8, at: u8) -> Parent {
    match byte.checked_rem(at) {
        Some(earlier) if byte >= 32 => Parent::Earlier(earlier),
        _ => Parent::Stranger(byte),
    }
}

/// A least point from its byte: one in sixteen is undecided, and the rest take six values, so
/// ties are common.
fn least(byte: u8) -> Option<(u8, u8)> {
    (byte & 15 != 15).then_some(((byte >> 4) % 3, byte >> 6))
}

/// The vertex list of a byte: none, one or two vertices of three, or undecided.
fn vertices(byte: u8) -> Option<Vec<u8>> {
    if byte & 15 == 15 {
        return None;
    }
    let mut v: Vec<u8> = [(byte >> 4) % 3, byte >> 6]
        .into_iter()
        .take(usize::from(byte % 3))
        .collect();
    v.sort_unstable();
    Some(v)
}

/// The direction of a byte: one of three, or undecided.
fn direction(byte: u8) -> Option<u8> {
    (byte & 15 != 15).then_some((byte >> 4) % 3)
}

/// The fuzzer's bytes as a case, and how many bytes were read (at most [`MAX_INPUT_BYTES`]).
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    const NAME_COUNTS: u8 = MAX_NAMES + 1;
    const LIST_LENGTHS: u8 = MAX_LIST + 1;
    const SET_COUNTS: u8 = MAX_SETS + 1;
    const ENTITY_COUNTS: u8 = MAX_ENTITIES + 1;
    let mut src = ByteSource::new(data);
    let count = src.byte() % NAME_COUNTS;
    let names = (0..count)
        .map(|at| {
            let (kind, op, arg) = (src.byte(), src.byte(), src.byte());
            let parents = [parent(src.byte(), at), parent(src.byte(), at)];
            Draw {
                kind,
                op,
                arg,
                parents,
            }
        })
        .collect();
    let len = src.byte() % LIST_LENGTHS;
    let list = (0..len).map(|_| src.byte()).collect();
    let count = src.byte() % SET_COUNTS;
    let sets = (0..count).map(|_| src.byte()).collect();
    let count = src.byte() % ENTITY_COUNTS;
    let keys = (0..count)
        .map(|_| {
            let (point, list, leaving) = (src.byte(), src.byte(), src.byte());
            Key {
                least: least(point),
                vertices: vertices(list),
                direction: direction(leaving),
            }
        })
        .collect();
    let read = data.len().saturating_sub(src.remaining());
    (
        Case {
            names,
            list,
            sets,
            keys,
        },
        read,
    )
}

/// A stranger's name: no `TopoName` hashes to it.
fn stranger(byte: u8) -> NameId {
    let mut bytes = [0xEE; 16];
    if let Some(last) = bytes.last_mut() {
        *last = byte;
    }
    NameId::from_bytes(bytes)
}

/// The roles a created name takes: one with a count, one with a name argument, and a keyed one
/// whose argument is an operand's key, not a parent.
const ROLES: [&str; 3] = ["side", "blend", "tool"];

/// The `TopoName` of `draw`, on the parents' IDs, with the parents a walk of the graph follows
/// from it, as 20.2 defines them.
fn build(draw: Draw, ids: &[NameId]) -> Option<(TopoName, Vec<NameId>)> {
    let id = |p: Parent| match p {
        Parent::Earlier(i) => ids.get(usize::from(i)).copied(),
        Parent::Stranger(b) => Some(stranger(b)),
    };
    let [first, second] = draw.parents;
    let (p, q) = (id(first)?, id(second)?);
    let set = |names: &[NameId]| -> Option<NameSet> {
        let distinct: BTreeSet<NameId> = names.iter().copied().collect();
        NameSet::new(distinct).ok()
    };
    let op = OpPath::new(OpId::from_bytes([draw.op; 16]));
    let side = if draw.arg % 2 == 0 {
        Side::Back
    } else {
        Side::Front
    };
    Some(match draw.kind % 5 {
        0 => {
            let role = *ROLES.get(usize::from(draw.arg % 3))?;
            let (role, walked) = match role {
                "side" => (
                    Role::new(role, [RoleArg::Count(u64::from(draw.arg))]),
                    vec![q],
                ),
                "blend" => (Role::new(role, [RoleArg::Name(p)]), vec![p, q]),
                // A keyed role's argument is an operand's key, never a parent
                // (`geom_op::KEYED_ROLES`).
                _ => (Role::new(role, [RoleArg::Name(p)]), vec![q]),
            };
            (
                TopoName::Created {
                    op,
                    role: role.ok()?,
                    from: NameList::new([q]),
                },
                walked,
            )
        }
        1 => (
            TopoName::Split {
                op,
                of: p,
                piece: PieceKey::ByTools {
                    tools: set(&[q])?,
                    side,
                },
            },
            vec![p, q],
        ),
        2 => (
            TopoName::Merged {
                op,
                of: set(&[p, q])?,
            },
            vec![p, q],
        ),
        3 => (
            TopoName::Bounded {
                kind: if side == Side::Back {
                    EdgeOrVertex::Edge
                } else {
                    EdgeOrVertex::Vertex
                },
                by: set(&[p])?,
                ends: set(&[q])?,
            },
            vec![p, q],
        ),
        _ => {
            let of = u32::from(draw.arg % 7).saturating_add(1);
            let n = u32::from(draw.op).checked_rem(of)?;
            (
                TopoName::Weak {
                    base: p,
                    ord: Ordinal::new(n, of).ok()?,
                },
                vec![p],
            )
        }
    })
}

/// What the graph's walk may answer for `id`, from the oracle's map of each held name to whether
/// it is a `Weak` node and the parents it is walked to: `Ok` of its weakness, or `Missing` of a
/// stranger it can reach; both when a weak node and a stranger are reachable.
fn weak_answers(
    id: NameId,
    held: &BTreeMap<NameId, (bool, Vec<NameId>)>,
) -> Vec<Result<bool, GraphError>> {
    let mut stack = vec![id];
    let mut seen = BTreeSet::from([id]);
    let (mut weak, mut missing) = (false, BTreeSet::new());
    while let Some(next) = stack.pop() {
        match held.get(&next) {
            None => {
                missing.insert(next);
            }
            Some((true, _)) => weak = true,
            Some((false, parents)) => {
                for &p in parents {
                    if seen.insert(p) {
                        stack.push(p);
                    }
                }
            }
        }
    }
    let mut answers: Vec<Result<bool, GraphError>> = missing
        .into_iter()
        .map(|m| Err(GraphError::Missing(m)))
        .collect();
    if answers.is_empty() || weak {
        answers.push(Ok(weak));
    }
    answers
}

/// 20.2's rule on two entities: least point, then vertices, then direction; `None` if a step is
/// undecided or they tie on all three.
fn rule(a: &Key, b: &Key) -> Option<Ordering> {
    for step in [
        Key::least_point as fn(&Key, &Key) -> Option<Ordering>,
        Key::vertices,
        Key::direction,
    ] {
        match step(a, b)? {
            Ordering::Equal => {}
            decided => return Some(decided),
        }
    }
    None
}

/// The oracle's ordinals of `keys`: each entity's rank, or `None` if a pair is undecided or tied.
fn ranks(keys: &[Key]) -> Option<Vec<Ordinal>> {
    let of = u32::try_from(keys.len()).ok()?;
    let mut out = Vec::new();
    for (i, a) in keys.iter().enumerate() {
        let mut below = 0_u32;
        for (j, b) in keys.iter().enumerate() {
            if i == j {
                continue;
            }
            if rule(b, a)? == Ordering::Less {
                below = below.saturating_add(1);
            }
        }
        out.push(Ordinal::new(below, of).ok()?);
    }
    Some(out)
}

/// The fuzz harness: the laws on `data` decoded as a case.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0).map_err(|f| Finding {
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
    let mut graph = NameGraph::new();
    let mut ids: Vec<NameId> = Vec::new();
    let mut held: BTreeMap<NameId, (bool, Vec<NameId>)> = BTreeMap::new();
    for draw in &case.names {
        let Some((name, walked)) = build(*draw, &ids) else {
            return Err(law("the decoder builds every name", format!("{draw:?}")));
        };
        let Ok(bytes) = name.encode().map(geom_op::cbor::Item::into_bytes) else {
            return Err(law(
                "every name has a canonical encoding",
                format!("{name:?}"),
            ));
        };
        let mut message = NAME_SCHEME_V1_PREFIX.to_vec();
        message.extend_from_slice(&bytes);
        let expected = NameId::from_bytes(sha256_128(&message));
        if name.id() != Ok(expected) {
            return Err(law(
                "a name's ID is SHA-256 of the scheme's prefix and its bytes, cut to 16",
                format!("{name:?}: {:?}, not {expected}", name.id()),
            ));
        }
        let before = graph.len();
        let interned = graph.intern(name.clone());
        if interned != Ok(expected)
            || graph.bytes(expected) != Some(bytes.as_slice())
            || graph.name(expected) != Some(&name)
        {
            return Err(law(
                "the graph interns a name under its ID with its bytes",
                format!("{name:?}: {interned:?}"),
            ));
        }
        let fresh = !held.contains_key(&expected);
        if graph.len() != before.saturating_add(usize::from(fresh))
            || graph.intern(name.clone()) != Ok(expected)
            || graph.len() != before.saturating_add(usize::from(fresh))
        {
            return Err(law(
                "a name is held once however often it is given",
                format!("{name:?}: {before} names, then {}", graph.len()),
            ));
        }
        held.insert(expected, (matches!(name, TopoName::Weak { .. }), walked));
        ids.push(expected);
    }
    let listed: Vec<NameId> = graph.ids().collect();
    let distinct: Vec<NameId> = held.keys().copied().collect();
    if listed != distinct {
        return Err(law(
            "the graph lists its names in byte order, each once",
            format!("{listed:?}, not {distinct:?}"),
        ));
    }
    for &id in &ids {
        let got = graph.is_weak(id);
        let allowed = weak_answers(id, &held);
        if !allowed.contains(&got) {
            return Err(law(
                "a name is weak exactly when a weak node is reachable, and a stranger is Missing",
                format!("{id}: {got:?}, not one of {allowed:?}"),
            ));
        }
    }
    check_lists(case, &ids, &law)?;
    check_order(
        case,
        ids.first().copied().unwrap_or_else(|| stranger(0)),
        &law,
    )
}

/// The duplicate detector and the reference pick, against brute force.
fn check_lists(
    case: &Case,
    ids: &[NameId],
    law: &impl Fn(&str, String) -> Finding,
) -> Result<(), Finding> {
    let pick = |b: u8| -> Option<NameId> {
        let n = u8::try_from(ids.len()).ok()?;
        ids.get(usize::from(b.checked_rem(n)?)).copied()
    };
    let list: Vec<NameId> = case.list.iter().filter_map(|&b| pick(b)).collect();
    let mut counts: BTreeMap<NameId, u32> = BTreeMap::new();
    for &id in &list {
        let c = counts.entry(id).or_insert(0);
        *c = c.saturating_add(1);
    }
    let lowest = counts.iter().find(|&(_, &c)| c >= 2).map(|(&id, _)| id);
    if first_duplicate(&list) != lowest {
        return Err(law(
            "the duplicate is the lowest name given twice",
            format!("{list:?}: {:?}, not {lowest:?}", first_duplicate(&list)),
        ));
    }
    // A release build checks nothing there (the module header of `geom_name::unique`).
    if cfg!(debug_assertions) && geom_name::expect_unique(&list).err().map(|d| d.0) != lowest {
        return Err(law(
            "expect_unique refuses exactly a list with a duplicate",
            format!("{list:?}"),
        ));
    }
    let sets: Vec<NameSet> = case
        .sets
        .iter()
        .filter_map(|&mask| {
            let chosen: BTreeSet<NameId> = ids
                .iter()
                .enumerate()
                .filter(|&(i, _)| {
                    u32::try_from(i).is_ok_and(|i| mask.checked_shr(i).is_some_and(|m| m & 1 == 1))
                })
                .map(|(_, &id)| id)
                .collect();
            NameSet::new(chosen).ok()
        })
        .collect();
    let common = sets.split_first().and_then(|(first, rest)| {
        first
            .names()
            .iter()
            .copied()
            .filter(|n| rest.iter().all(|s| s.names().contains(n)))
            .min()
    });
    if reference(&sets) != common {
        return Err(law(
            "the reference is the lowest name every set holds",
            format!("{sets:?}: {:?}, not {common:?}", reference(&sets)),
        ));
    }
    Ok(())
}

/// The ordinals and weak names, against the rule's ranks, and the same entities reversed.
fn check_order(
    case: &Case,
    base: NameId,
    law: &impl Fn(&str, String) -> Finding,
) -> Result<(), Finding> {
    let keys = &case.keys;
    let got = ordinals(keys);
    match (ranks(keys), &got) {
        (Some(expected), Ok(ords)) if *ords == expected => {}
        (None, Err(OrderError::UndecidedOrder(a, b)))
            if a != b
                && keys
                    .get(*a)
                    .zip(keys.get(*b))
                    .is_some_and(|(x, y)| rule(x, y).is_none()) => {}
        (expected, _) => {
            return Err(law(
                "ordinals are ranks under 20.2's rule, or UndecidedOrder naming a pair it cannot order",
                format!("{got:?}, not {expected:?}"),
            ));
        }
    }
    let reversed: Vec<Key> = keys.iter().rev().cloned().collect();
    let back = ordinals(&reversed).map(|mut o| {
        o.reverse();
        o
    });
    if back.is_ok() != got.is_ok() || got.as_ref().is_ok_and(|o| Ok(o) != back.as_ref()) {
        return Err(law(
            "the order is the entities', not the input's",
            format!("{got:?}, reversed {back:?}"),
        ));
    }
    let weak = weak_names(base, keys);
    let expected: Result<Vec<TopoName>, OrderError> = got.map(|ords| {
        ords.into_iter()
            .map(|ord| TopoName::Weak { base, ord })
            .collect()
    });
    if weak != expected {
        return Err(law(
            "weak names are the base and each entity's ordinal",
            format!("{weak:?}, not {expected:?}"),
        ));
    }
    Ok(())
}
