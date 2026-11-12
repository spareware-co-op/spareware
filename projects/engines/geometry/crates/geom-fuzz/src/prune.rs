//! Fuzz target: `PruneHistory` and the pruned history record (piece G109). On any element history
//! and any set of held names, pruning keeps every step's place, every held reference resolves as
//! before from every step and for every want, a reference no one held meets `HistoryPruned`,
//! pruning again cuts only what is new, the budget stops a run exactly where its charges say, and
//! the pruned record reads back to what was written and, from any bytes, reads only its one
//! canonical encoding.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: an element as the
//! [`references`](crate::references) target reads one (a chain of histories over first faces,
//! some weak, its body and carriers, sometimes a stretch pruned earlier in a linked version), two
//! sets of held names drawn from every name it had, a step to prune again from, and an edit to
//! make to the record's payload: one byte replaced and the rest cut. The bytes after the case are
//! a pruned record's payload as a file holds it. [`check`] prunes the history, holds the result
//! and every reference into it to the laws, prunes again, runs the prune under its exact budget
//! and one unit less, writes the steps as a pruned record and reads it back, and reads the edited
//! payload and the trailing bytes. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included; [`decode`] reads at most
//!   [`MAX_INPUT_BYTES`] bytes of the case, derived below (a unit test builds the input that
//!   reaches it), and hands every byte after them to the record's reader.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case. The laws
//!   are the module header's of `geom_name::prune`: each step keeps its place and its path, an
//!   operation step becomes a kept step naming the operation and the held set and keeping only its
//!   own records and deletions, and a step pruned before stays as it was; every held name, and every
//!   name in the body, resolves as it did from every step and for every want; any other name with
//!   steps to walk meets `HistoryPruned` at once, with the step's pruning operation; pruning again
//!   leaves the steps the first prune kept as they were, and every name the second holds resolves
//!   as after the first; a run charges one unit a step and a record, so a budget of exactly that
//!   succeeds with the same steps and one unit less is `BudgetExceeded`; and of the pruned record's
//!   (`geom_format::pruned`): the steps written read back as written, and a payload read twice gives
//!   the same answer and is accepted only if it is the one encoding of what it holds.
//!
//! Method: chapter 24.4's `PruneHistory` and pruned history record
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`) with chapter 20.4's
//! resolution as the observer, `tests/prune.rs`'s laws in `geom-name` driven by coverage-guided
//! fuzzing (28.5), and the reader's laws as the record targets hold theirs.
//!
//! Cost: three prunes and a budget pair over at most [`references::MAX_STEPS`] steps and a stretch;
//! about 2 · 3 · (s + 1) resolutions for each of the element's names; a record written once and
//! read three times; at most [`MAX_INPUT_BYTES`] bytes of the case read.

use geom_format::pruned::{encode_pruned, pruned_record, read_pruned};
use geom_name::resolve::Step;
use geom_name::{Body, Lookup, Resolution, Want, resolve};
use geom_op::{ErrorKind, History, NameId, NameSet, OpContext, OpId, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;
use crate::cbor::hex;
use crate::references::{self, ELEMENT_BYTES, Element};

/// The most bytes [`decode`] reads of the case: an element, two held masks of four bytes, the step
/// to prune again from, and the payload's edit (its place in two bytes, the byte put there and
/// where it is cut): 49 + 4 + 4 + 1 + 4 = 62. The test `the_input_bound_is_reached_and_never_passed`
/// builds that input.
pub const MAX_INPUT_BYTES: usize = ELEMENT_BYTES + 4 + 4 + 1 + 4;

const _: () = assert!(MAX_INPUT_BYTES == 62);

/// The `PruneHistory` operations the case runs.
const FIRST: u8 = 0xC0;
const SECOND: u8 = 0xC1;

/// One case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The element whose history is pruned.
    pub element: Element,
    /// The names the first prune holds.
    pub held: NameSet,
    /// The names the second prune holds.
    pub again: NameSet,
    /// The step the second prune starts from: it prunes the first's steps before it and the
    /// original steps from it.
    pub at: usize,
    /// The edit made to the written record's payload: the place, the byte put there, and the
    /// length it is cut to.
    pub edit: (u16, u8, u8),
    /// The bytes after the case, read as a payload.
    pub tail: Vec<u8>,
}

/// The names of `names` whose bit in `mask` is set, bit `i % 32` for the `i`-th.
fn chosen(names: &[NameId], mask: u32) -> NameSet {
    let picked = names.iter().enumerate().filter(|&(i, _)| {
        u32::try_from(i % 32).is_ok_and(|b| mask.checked_shr(b).is_some_and(|m| m & 1 == 1))
    });
    NameSet::new(picked.map(|(_, &n)| n)).unwrap_or_default()
}

/// A mask from four bytes.
fn mask(s: &mut ByteSource<'_>) -> u32 {
    u32::from_le_bytes([s.byte(), s.byte(), s.byte(), s.byte()])
}

/// The fuzzer's bytes as a case, and how many bytes of the case were read (at most
/// [`MAX_INPUT_BYTES`]); `None` only if the history kit or the graph refuses what the bytes
/// chose.
#[must_use]
pub fn decode(data: &[u8]) -> Option<(Case, usize)> {
    let mut s = ByteSource::new(data);
    let element = references::element(&mut s)?;
    let held = chosen(&element.names, mask(&mut s));
    let again = chosen(&element.names, mask(&mut s));
    let at = usize::from(s.byte()) % element.history.len().saturating_add(1);
    let place = u16::from_le_bytes([s.byte(), s.byte()]);
    let edit = (place, s.byte(), s.byte());
    let read = data.len().saturating_sub(s.remaining());
    let tail = data.get(read..).unwrap_or_default().to_vec();
    Some((
        Case {
            element,
            held,
            again,
            at,
            edit,
            tail,
        },
        read,
    ))
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

/// A `PruneHistory` operation's path.
fn op(byte: u8) -> OpPath {
    OpPath::new(OpId::from_bytes([byte; 16]))
}

/// `history` pruned for `held` by the operation at `by`, under `budget`.
fn pruned(
    history: &[Step],
    held: &NameSet,
    by: u8,
    budget: Budget,
) -> Result<Vec<Step>, geom_op::OpError> {
    geom_name::prune(&OpContext::new(op(by), budget), history, held).map(|o| o.result)
}

/// `name` resolved for `want` against `element`'s body, with `history` since the reference was
/// made.
fn resolved(element: &Element, history: &[Step], name: NameId, want: Want) -> Resolution {
    let reference = geom_name::Reference {
        doc: None,
        element: geom_name::ElementId([0xE1; 16]),
        name,
        kind: geom_name::EntityKind::Face,
        want,
        signature: geom_name::Signature::default(),
    };
    let lookup = Lookup::Found {
        body: Body {
            names: &element.body,
            carriers: &element.carriers,
            history,
            graph: &element.graph,
        },
        version: None,
    };
    resolve(&reference, &lookup)
}

const WANTS: [Want; 3] = [Want::One, Want::AllPieces, Want::Carrier];

/// Holds the case to every law of the module header.
///
/// # Errors
///
/// A [`Finding`] naming the law the case broke.
pub fn check(case: &Case) -> Result<(), Finding> {
    let law = |law: &str, what: String| Finding::new(law, || format!("{case:?}: {what}"));
    let e = &case.element;
    let history = &e.history;
    let Ok(once) = pruned(history, &case.held, FIRST, Budget::UNLIMITED) else {
        return Err(law("a prune under no limit succeeds", String::new()));
    };
    check_steps(history, &once, &case.held, &law)?;
    // Every held reference, and every body name, resolves as before; any other meets the cut.
    for from in 0..=history.len() {
        let (before, after) = (
            history.get(from..).unwrap_or_default(),
            once.get(from..).unwrap_or_default(),
        );
        for &name in &e.names {
            let kept = case.held.names().contains(&name) || e.body.names().contains(&name);
            for want in WANTS {
                let (b, a) = (
                    resolved(e, before, name, want),
                    resolved(e, after, name, want),
                );
                let expected = match after.first() {
                    _ if kept => b.clone(),
                    None => b.clone(),
                    Some(Step::Pruned { by }) => Resolution::HistoryPruned { by: by.clone() },
                    Some(_) => Resolution::HistoryPruned {
                        by: Some(op(FIRST)),
                    },
                };
                if a != expected {
                    return Err(law(
                        "a held reference resolves as before, and any other meets HistoryPruned",
                        format!("{name} from {from} for {want:?}: {a:?}, not {expected:?}; before {b:?}"),
                    ));
                }
            }
        }
    }
    check_again(case, &once, &law)?;
    check_budget(history, &case.held, &once, &law)?;
    check_record(case, &once, &law)
}

/// Each step's place, path and kept part.
fn check_steps(
    history: &[Step],
    once: &[Step],
    held: &NameSet,
    law: &impl Fn(&str, String) -> Finding,
) -> Result<(), Finding> {
    if once.len() != history.len() {
        return Err(law(
            "every step keeps its place",
            format!("{} steps, then {}", history.len(), once.len()),
        ));
    }
    for (before, after) in history.iter().zip(once) {
        let ok = match (before, after) {
            (
                Step::Recorded { by, history },
                Step::Kept {
                    by: kept_by,
                    history: kept,
                    pruned_by,
                    held: kept_for,
                },
            ) => {
                by == kept_by
                    && *pruned_by == op(FIRST)
                    && kept_for == held
                    && kept.changes().is_empty()
                    && kept.records().iter().all(|r| history.records().contains(r))
                    && kept
                        .deleted()
                        .names()
                        .iter()
                        .all(|d| history.deleted().names().contains(d))
            }
            (Step::Pruned { .. } | Step::Kept { .. }, _) => before == after,
            _ => false,
        };
        if !ok {
            return Err(law(
                "a step becomes a kept step of its own records, or stays as an earlier prune left it",
                format!("{before:?} became {after:?}"),
            ));
        }
    }
    Ok(())
}

/// Pruning again, from the case's step on.
fn check_again(
    case: &Case,
    once: &[Step],
    law: &impl Fn(&str, String) -> Finding,
) -> Result<(), Finding> {
    let e = &case.element;
    let mut mixed: Vec<Step> = once.get(..case.at).unwrap_or_default().to_vec();
    mixed.extend_from_slice(e.history.get(case.at..).unwrap_or_default());
    let Ok(twice) = pruned(&mixed, &case.again, SECOND, Budget::UNLIMITED) else {
        return Err(law("a second prune under no limit succeeds", String::new()));
    };
    if twice.get(..case.at) != mixed.get(..case.at) {
        return Err(law(
            "pruning again leaves the steps an earlier prune kept",
            format!("{mixed:?} became {twice:?}"),
        ));
    }
    for from in 0..=mixed.len() {
        for &name in case.again.names() {
            for want in WANTS {
                let (b, a) = (
                    resolved(e, mixed.get(from..).unwrap_or_default(), name, want),
                    resolved(e, twice.get(from..).unwrap_or_default(), name, want),
                );
                if a != b {
                    return Err(law(
                        "every name the second prune holds resolves as after the first",
                        format!("{name} from {from} for {want:?}: {a:?}, not {b:?}"),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// The budget: one unit a step and a record.
fn check_budget(
    history: &[Step],
    held: &NameSet,
    once: &[Step],
    law: &impl Fn(&str, String) -> Finding,
) -> Result<(), Finding> {
    let charges: u64 = history
        .iter()
        .map(|step| {
            let records = match step {
                Step::Recorded { history, .. } | Step::Kept { history, .. } => {
                    history.records().len()
                }
                Step::Pruned { .. } => 0,
            };
            u64::try_from(records).unwrap_or(u64::MAX).saturating_add(1)
        })
        .fold(0, u64::saturating_add);
    let exact = Budget {
        work: charges,
        logical_bytes: u64::MAX,
    };
    if pruned(history, held, FIRST, exact).as_deref() != Ok(once) {
        return Err(law(
            "a prune within its charges gives the same steps",
            format!("{charges} units"),
        ));
    }
    if let Some(work) = charges.checked_sub(1) {
        let short = pruned(history, held, FIRST, Budget { work, ..exact });
        if short.as_ref().err().map(geom_op::OpError::kind) != Some(ErrorKind::BudgetExceeded) {
            return Err(law(
                "a prune one unit short of its charges stops with BudgetExceeded",
                format!("{work} units: {short:?}"),
            ));
        }
    }
    Ok(())
}

/// A payload read twice, and accepted only as its one encoding.
fn check_payload(payload: &[u8], law: &impl Fn(&str, String) -> Finding) -> Result<(), Finding> {
    let meter = || WorkMeter::new(Budget::UNLIMITED);
    let first = read_pruned(payload, &mut meter());
    if read_pruned(payload, &mut meter()) != first {
        return Err(law(
            "a pruned record reads the same twice",
            format!("{}: {first:?}", hex(payload)),
        ));
    }
    if let Ok((by, held, steps)) = &first
        && encode_pruned(by, held, steps).as_deref() != Ok(payload)
    {
        return Err(law(
            "a pruned record read is its one encoding",
            format!("{}: {first:?}", hex(payload)),
        ));
    }
    Ok(())
}

/// The pruned record: written and read back, edited, and the trailing bytes.
fn check_record(
    case: &Case,
    once: &[Step],
    law: &impl Fn(&str, String) -> Finding,
) -> Result<(), Finding> {
    let steps: Vec<(OpPath, History)> = once
        .iter()
        .filter_map(|step| match step {
            Step::Kept {
                by,
                history,
                pruned_by,
                ..
            } if *pruned_by == op(FIRST) => Some((by.clone(), history.clone())),
            _ => None,
        })
        .collect();
    let Ok(record) = pruned_record(&op(FIRST), &case.held, &steps) else {
        return Err(law("a pruned record can be written", format!("{steps:?}")));
    };
    let back = read_pruned(&record.payload, &mut WorkMeter::new(Budget::UNLIMITED));
    if back.as_ref().ok() != Some(&(op(FIRST), case.held.clone(), steps.clone())) {
        return Err(law(
            "a pruned record reads back as written",
            format!("{steps:?}: {back:?}"),
        ));
    }
    check_payload(&record.payload, law)?;
    let (place, byte, cut) = case.edit;
    let mut edited = record.payload;
    if let Some(at) = usize::from(place).checked_rem(edited.len())
        && let Some(b) = edited.get_mut(at)
    {
        *b = byte;
    }
    // One cut in four leaves the payload whole.
    if cut % 4 != 0 {
        edited.truncate(usize::from(cut));
    }
    check_payload(&edited, law)?;
    check_payload(&case.tail, law)
}
