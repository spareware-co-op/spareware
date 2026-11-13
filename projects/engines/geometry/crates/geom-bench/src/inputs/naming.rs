//! Inputs for persistent names, history records, reference resolution and `PruneHistory` (pieces
//! G87, G88, G89 and G109, decision 32.46): names of every kind, an element's history over two
//! operations, references into it, and that history pruned.
//!
//! What it does: [`a_chain`] builds one element's history, as an operation log records it: five
//! faces made by a first operation, then a cut that keeps one face, modifies one, splits one in
//! two by a tool, deletes one while generating a cap from it and keeps the last, then a step that
//! modifies the modified face again and deletes the last; every name is interned in the element's
//! name graph, and the body after it holds a carrier for each face. From it, [`names`] are the
//! names of each kind, [`groups`] the names a graph interns, [`checks`] and [`compositions`] its
//! steps for the history laws and for composing them, [`scenes`] references to its faces, in turn
//! one in the body, one modified, one split and wanted whole, one split and wanted by its carrier,
//! one deleted and one split and wanted alone, and [`prunes`], [`pruned`] and [`pruned_payloads`]
//! its steps pruned for a held face. Each input draws its own operation paths, roles and tools, so
//! no two are alike.
//!
//! Guarantees: every typical batch is full and every call succeeds (`tests/inputs.rs`): each name
//! has an ID, each graph interns its names, each history keeps its laws, each prune runs inside
//! its unlimited budget, and each pruned record reads back. Only the typical class exists: the
//! refusals are tested in `geom-op`, `geom-name` and `geom-format` themselves.
//!
//! Method: the cases of `geom-name`'s and `geom-op`'s own tests (`tests/attack_g87.rs`,
//! `tests/attack_g89.rs`, `tests/prune.rs`; `geom-op`'s `tests/history.rs`) and `geom-format`'s
//! `tests/pruned.rs`, cut to one small element; each is measured as chapter 28.14 sets benchmarks
//! out, `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: one chain of a dozen names built per input, once per batch, outside every measured run.

use geom_format::pruned::{Pruned, encode_pruned};
use geom_name::NameGraph;
use geom_name::resolve::{CarrierIx, ElementId, EntityKind, Reference, Signature, Step, Want};
use geom_op::{
    EdgeOrVertex, History, NameId, NameList, NameSet, OpContext, OpId, OpPath, Ordinal, Origin,
    PieceKey, Record, Relations, Role, RoleArg, Side, TopoName, TopologyChange, Vanish,
};
use geom_run::Budget;

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// Sixteen drawn bytes.
fn bytes16(g: &mut SplitMix64) -> [u8; 16] {
    let (a, b) = (g.next_u64().to_le_bytes(), g.next_u64().to_le_bytes());
    let mut out = [0; 16];
    for (o, v) in out.iter_mut().zip(a.iter().chain(&b)) {
        *o = *v;
    }
    out
}

/// A drawn operation path with one step.
fn path(g: &mut SplitMix64, step: &str) -> Option<OpPath> {
    OpPath::new(OpId::from_bytes(bytes16(g))).step(step)
}

/// A role with one drawn count.
fn role(g: &mut SplitMix64, name: &str) -> Option<Role> {
    Role::new(name, [RoleArg::Count(g.below(64))]).ok()
}

/// One element's history over two operations, its name graph, the body after it and the faces
/// it started with (see the module documentation).
#[derive(Clone, Debug)]
pub struct Chain {
    /// Every name of the element.
    pub graph: NameGraph,
    /// Each operation's path, history and inputs, oldest first.
    pub steps: Vec<(OpPath, History, NameSet)>,
    /// The names of the body after the last step.
    pub body: NameSet,
    /// The carrier of each face of the body, in name order.
    pub carriers: Vec<(NameId, CarrierIx)>,
    /// The faces the first operation made: kept, modified, split, deleted, deleted later.
    pub faces: [NameId; 5],
}

impl Chain {
    /// The steps as a reference walks them.
    #[must_use]
    pub fn walk(&self) -> Vec<Step> {
        self.steps
            .iter()
            .map(|(by, history, _)| Step::Recorded {
                by: by.clone(),
                history: history.clone(),
            })
            .collect()
    }
}

/// Builds one chain from `g` (see the module documentation).
fn a_chain(g: &mut SplitMix64) -> Option<Chain> {
    let mut graph = NameGraph::new();
    let made = path(g, "make")?;
    let mut faces = [NameId::from_bytes([0; 16]); 5];
    for (k, face) in (0..).zip(faces.iter_mut()) {
        *face = graph
            .intern(TopoName::Created {
                op: made.clone(),
                role: Role::new("side", [RoleArg::Count(k)]).ok()?,
                from: NameList::new([]),
            })
            .ok()?;
    }
    let [a, b, c, d, e] = faces;
    // The cut: keeps a and e, modifies b, splits c by a tool, deletes d while generating a cap.
    let cut = path(g, "cut")?;
    let b1 = graph
        .intern(TopoName::Created {
            op: cut.clone(),
            role: role(g, "side")?,
            from: NameList::new([b]),
        })
        .ok()?;
    // The tool is a face of another body, in the graph as every name a walk can reach must be.
    let tool = graph
        .intern(TopoName::Created {
            op: path(g, "tool")?,
            role: role(g, "side")?,
            from: NameList::new([]),
        })
        .ok()?;
    let tools = NameSet::new([tool]).ok()?;
    let piece = |side| PieceKey::ByTools {
        tools: tools.clone(),
        side,
    };
    let mut pieces = [a; 2];
    for (p, side) in pieces.iter_mut().zip([Side::Back, Side::Front]) {
        *p = graph
            .intern(TopoName::Split {
                op: cut.clone(),
                of: c,
                piece: piece(side),
            })
            .ok()?;
    }
    let [c1, c2] = pieces;
    let cap = role(g, "cap")?;
    let cap_face = graph
        .intern(TopoName::Created {
            op: cut.clone(),
            role: cap.clone(),
            from: NameList::new([d]),
        })
        .ok()?;
    let first = History::new(
        [
            Record {
                output: a,
                origin: Origin::Unchanged,
            },
            Record {
                output: b1,
                origin: Origin::Modified {
                    from: NameSet::new([b]).ok()?,
                },
            },
            Record {
                output: c1,
                origin: Origin::Split {
                    of: c,
                    piece: piece(Side::Back),
                },
            },
            Record {
                output: c2,
                origin: Origin::Split {
                    of: c,
                    piece: piece(Side::Front),
                },
            },
            Record {
                output: cap_face,
                origin: Origin::Generated {
                    from: NameSet::new([d]).ok()?,
                    role: cap.clone(),
                },
            },
            Record {
                output: e,
                origin: Origin::Unchanged,
            },
        ],
        NameSet::new([d]).ok()?,
        [
            TopologyChange::FaceVanished {
                face: d,
                why: Vanish::Collapsed,
            },
            TopologyChange::FaceAdded {
                face: cap_face,
                role: cap,
            },
        ],
    )
    .ok()?;
    // The next step: modifies b1 again and deletes e; keeps the rest.
    let next = path(g, "offset")?;
    let b2 = graph
        .intern(TopoName::Created {
            op: next.clone(),
            role: role(g, "side")?,
            from: NameList::new([b1]),
        })
        .ok()?;
    let kept = |output| Record {
        output,
        origin: Origin::Unchanged,
    };
    let second = History::new(
        [
            kept(a),
            Record {
                output: b2,
                origin: Origin::Modified {
                    from: NameSet::new([b1]).ok()?,
                },
            },
            kept(c1),
            kept(c2),
            kept(cap_face),
        ],
        NameSet::new([e]).ok()?,
        [TopologyChange::FaceVanished {
            face: e,
            why: Vanish::InsideOut,
        }],
    )
    .ok()?;
    let body = NameSet::new([a, b2, c1, c2, cap_face]).ok()?;
    // The two pieces of c lie on c's carrier; every other face on its own.
    let mut carriers = vec![
        (a, CarrierIx(0)),
        (b2, CarrierIx(1)),
        (c1, CarrierIx(2)),
        (c2, CarrierIx(2)),
        (cap_face, CarrierIx(3)),
    ];
    carriers.sort_unstable();
    Some(Chain {
        graph,
        steps: vec![
            (cut, first, NameSet::new(faces).ok()?),
            (
                next,
                second,
                NameSet::new([a, b1, c1, c2, cap_face, e]).ok()?,
            ),
        ],
        body,
        carriers,
        faces,
    })
}

/// The `n`-th name of a chain's kind in turn: made by a role, split, merged, an edge between two
/// faces, or weak.
fn a_name(g: &mut SplitMix64, n: usize) -> Option<TopoName> {
    let chain = a_chain(g)?;
    let [a, b, c, ..] = chain.faces;
    let op = path(g, "fillet")?;
    Some(match n % 5 {
        0 => TopoName::Created {
            op,
            role: role(g, "blend")?,
            from: NameList::new([a, b]),
        },
        1 => TopoName::Split {
            op,
            of: c,
            piece: PieceKey::ByTools {
                tools: NameSet::new([a, b]).ok()?,
                side: Side::Front,
            },
        },
        2 => TopoName::Merged {
            op,
            of: NameSet::new([a, b, c]).ok()?,
        },
        3 => TopoName::Bounded {
            kind: EdgeOrVertex::Edge,
            by: NameSet::new([a, b]).ok()?,
            ends: NameSet::new([c]).ok()?,
        },
        _ => TopoName::Weak {
            base: a,
            ord: Ordinal::new(2, 3).ok()?,
        },
    })
}

/// One name per call of `TopoName::id`, each kind in turn.
#[must_use]
pub fn names(class: Class) -> Vec<TopoName> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("names::name_id", class);
    batch_of(|n| a_name(&mut g, n))
}

/// Five names, one of each kind, per call of `NameGraph::intern` on each.
#[must_use]
pub fn groups(class: Class) -> Vec<Vec<TopoName>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("names::intern", class);
    batch_of(|n| (0..5).map(|k| a_name(&mut g, n + k)).collect())
}

/// One step's history and inputs per call of `History::check`, the cut and the step after it in
/// turn.
#[must_use]
pub fn checks(class: Class) -> Vec<(History, NameSet)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("history::check", class);
    batch_of(|n| {
        let (_, history, inputs) = a_chain(&mut g)?.steps.into_iter().nth(n % 2)?;
        Some((history, inputs))
    })
}

/// The relations of a chain's two steps per call of `Relations::then`.
#[must_use]
pub fn compositions(class: Class) -> Vec<(Relations, Relations)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("history::compose", class);
    batch_of(|_| {
        let chain = a_chain(&mut g)?;
        let [(_, first, _), (_, second, _)] = chain.steps.as_slice() else {
            return None;
        };
        Some((first.relations(), second.relations()))
    })
}

/// A reference into an element and what the program holds of it.
#[derive(Clone, Debug)]
pub struct Scene {
    /// The element's history, names and body.
    pub chain: Chain,
    /// The steps since the reference was made, as a reference walks them.
    pub history: Vec<Step>,
    /// The reference.
    pub reference: Reference,
}

/// A reference to `name` for `want`.
fn reference(g: &mut SplitMix64, name: NameId, want: Want) -> Reference {
    Reference {
        doc: None,
        element: ElementId(bytes16(g)),
        name,
        kind: EntityKind::Face,
        want,
        signature: Signature(bytes16(g).to_vec()),
    }
}

/// One reference per call of `resolve`, each of its paths in turn (see the module
/// documentation).
#[must_use]
pub fn scenes(class: Class) -> Vec<Scene> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("resolve::reference", class);
    batch_of(|n| {
        let chain = a_chain(&mut g)?;
        let [a, b, c, d, _] = chain.faces;
        let (name, want) = match n % 6 {
            0 => (a, Want::One),
            1 => (b, Want::One),
            2 => (c, Want::AllPieces),
            3 => (c, Want::Carrier),
            4 => (d, Want::One),
            _ => (c, Want::One),
        };
        let reference = reference(&mut g, name, want);
        Some(Scene {
            history: chain.walk(),
            chain,
            reference,
        })
    })
}

/// A chain's steps and the names held, one face of the first operation's in turn.
fn held_chain(g: &mut SplitMix64, n: usize) -> Option<(Chain, NameSet)> {
    let chain = a_chain(g)?;
    let held = NameSet::new([*chain.faces.get(n % 5)?]).ok()?;
    Some((chain, held))
}

/// One element's steps and the names held per call of `prune`.
#[must_use]
pub fn prunes(class: Class) -> Vec<(Vec<Step>, NameSet)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("prune::prune", class);
    batch_of(|n| {
        let (chain, held) = held_chain(&mut g, n)?;
        Some((chain.walk(), held))
    })
}

/// What one `PruneHistory` operation cut: its path, the set held, and each step it kept.
fn cut_down(g: &mut SplitMix64, n: usize) -> Option<Pruned> {
    let (chain, held) = held_chain(g, n)?;
    let by = path(g, "prune")?;
    let ctx = OpContext::new(by.clone(), Budget::UNLIMITED);
    let steps = geom_name::prune(&ctx, &chain.walk(), &held)
        .ok()?
        .result
        .into_iter()
        .filter_map(|step| match step {
            Step::Kept { by, history, .. } => Some((by, history)),
            _ => None,
        })
        .collect();
    Some((by, held, steps))
}

/// One pruned element per call of `pruned_record`.
#[must_use]
pub fn pruned(class: Class) -> Vec<Pruned> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("prune::record_write", class);
    batch_of(|n| cut_down(&mut g, n))
}

/// One pruned history record's payload per call of `read_pruned`.
#[must_use]
pub fn pruned_payloads(class: Class) -> Vec<Vec<u8>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("prune::record_read", class);
    batch_of(|n| {
        let (by, held, steps) = cut_down(&mut g, n)?;
        encode_pruned(&by, &held, &steps).ok()
    })
}
