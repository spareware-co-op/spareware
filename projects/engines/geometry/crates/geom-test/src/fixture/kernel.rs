//! The kernel as a fixture's [`Engine`]: one operation record in, the operation's canonical parts
//! out (chapter 28.16; pieces G97 and G116).
//!
//! What it does: [`Kernel`] finds an operation by its name and version ([`Call::read`]), takes the
//! bodies it names from the results so far, runs it, and keeps each result under its content ID, so
//! a later operation of the log can name it. Each piece that adds an operation adds its row to
//! [`Call`].
//!
//! Guarantees:
//!
//! - **An unknown operation is refused, never guessed**: a name or version not in the table, or
//!   parameters that are not its one writing, give [`Ran::Refused`], which no status accepts. So
//!   does an operation that names a body the log has not made.
//! - **No state survives [`Engine::start`]**: a fresh run begins from the log's own inputs.
//! - **A body is its stored bytes.** A result's hash in the step is its content ID (24.2).
//!
//! Method: chapter 21's replay (`projects/engines/geometry/architecture/21-determinism.md`).
//!
//! Cost: one match per operation, plus the operation, plus one write of each result to hash it.

use std::collections::BTreeMap;

use geom_format::body::write_body;
use geom_format::content::ContentId;
use geom_format::operation::Operation;
use geom_format::recipe::RecipeInput;
use geom_format::record::Record;
use geom_make::Made;
use geom_num::Steps;
use geom_op::history::Record as HistoryRecord;
use geom_op::{Exactness, History, NameId, NameSet, OpContext};
use geom_run::{Budget, WorkMeter};
use geom_topo::Body;

use super::calls::Call;
use super::run::{Engine, Ran, Step};
use super::{Class, Counts, Measures};

/// The kernel, replaying a log.
#[derive(Clone, Debug, Default)]
pub struct Kernel {
    heal: Option<Steps>,
    bodies: BTreeMap<ContentId, Body>,
    last: Vec<Body>,
    exactness: Option<Exactness>,
}

impl Kernel {
    /// A kernel with nothing run.
    #[must_use]
    pub fn new() -> Kernel {
        Kernel::default()
    }

    /// The results of the last operation that succeeded, in the operation's order.
    #[must_use]
    pub fn last(&self) -> &[Body] {
        &self.last
    }

    /// A body the log has made, by its content ID.
    #[must_use]
    pub fn body(&self, id: &ContentId) -> Option<&Body> {
        self.bodies.get(id)
    }

    /// Keeps the results of an operation and gives its step.
    fn keep(
        &mut self,
        taken: Option<&Body>,
        made: Vec<Made>,
        exactness: Exactness,
        diagnostics: Vec<geom_op::Diagnostic>,
    ) -> Result<Step, String> {
        let mut hashes = Vec::with_capacity(made.len());
        let mut last = Vec::with_capacity(made.len());
        let mut records: Vec<HistoryRecord> = Vec::new();
        let mut changes = Vec::new();
        let mut deleted: Vec<NameId> = Vec::new();
        for Made { body, history, .. } in made {
            let id = id_of(&body)?;
            let digest = <[u8; 32]>::try_from(id.digest())
                .map_err(|_| "a content ID that is not 32 bytes".to_owned())?;
            hashes.push(digest);
            last.push(body.clone());
            self.bodies.insert(id, body);
            records.extend_from_slice(history.records());
            changes.extend_from_slice(history.changes());
            deleted.extend_from_slice(history.deleted().names());
        }
        deleted.sort_unstable();
        deleted.dedup();
        let deleted = NameSet::new(deleted).map_err(|e| e.to_string())?;
        let kept = taken.and_then(Body::names);
        let new = records.iter().map(|r| r.output).filter(|name| {
            kept.is_none_or(|names| {
                let key = geom_topo::order::NameKey(name.to_bytes());
                !(names.regions.contains(&key)
                    || names.faces.contains(&key)
                    || names.edges.contains(&key)
                    || names.vertices.contains(&key))
            })
        });
        let names = NameSet::new(new).map_err(|e| e.to_string())?;
        let history = History::new(records, deleted, changes).map_err(|e| e.to_string())?;
        self.last = last;
        self.exactness = Some(exactness);
        Ok(Step {
            bodies: hashes,
            names,
            history: Some(history),
            exactness,
            diagnostics,
        })
    }
}

/// `body`'s content ID.
///
/// # Errors
///
/// Why it has no stored form.
pub fn id_of(body: &Body) -> Result<ContentId, String> {
    write_body(body, &mut WorkMeter::new(Budget::UNLIMITED))
        .map(|stored| stored.content_id())
        .map_err(|e| e.to_string())
}

impl Engine for Kernel {
    fn start(&mut self, heal: Steps, inputs: &[Record]) -> Result<(), String> {
        *self = Kernel::new();
        self.heal = Some(heal);
        if inputs.is_empty() {
            Ok(())
        } else {
            Err("no operation reads a stored body yet".to_owned())
        }
    }

    fn run(&mut self, operation: &Operation) -> Ran {
        let call = match Call::read(&operation.name, operation.version, &operation.params) {
            Ok(call) => call,
            Err(why) => {
                return Ran::Refused(format!(
                    "the kernel has no operation {} at version {}: {why:?}",
                    operation.name, operation.version
                ));
            }
        };
        let body = match operation.inputs.as_slice() {
            [] => None,
            [RecipeInput::Body(id)] => match self.bodies.get(id) {
                Some(body) => Some(body.clone()),
                None => return Ran::Refused("a body the log has not made".to_owned()),
            },
            _ => return Ran::Refused("inputs its schema does not take".to_owned()),
        };
        if call.takes_a_body() != body.is_some() {
            return Ran::Refused("inputs its schema does not take".to_owned());
        }
        let ctx = OpContext::new(operation.op.clone(), operation.budget);
        let Some(answer) = call.run(&ctx, body.as_ref()) else {
            return Ran::Refused("parameters no input can hold".to_owned());
        };
        match answer {
            Ok(output) => {
                let (exactness, diagnostics) = (output.exactness, output.diagnostics);
                match self.keep(body.as_ref(), output.result, exactness, diagnostics) {
                    Ok(step) => Ran::Done(step),
                    Err(why) => Ran::Refused(why),
                }
            }
            Err(error) => Ran::Failed(error),
        }
    }

    fn measures(&mut self) -> Measures {
        let mut counts = Counts::default();
        let mut bounds: Option<[[i64; 3]; 2]> = None;
        for body in &self.last {
            let a = body.arenas();
            counts.bodies = counts.bodies.saturating_add(1);
            counts.shells = counts.shells.saturating_add(u64::from(a.shells.len()));
            counts.faces = counts.faces.saturating_add(u64::from(a.faces.len()));
            counts.edges = counts.edges.saturating_add(u64::from(a.edges.len()));
            counts.vertices = counts.vertices.saturating_add(u64::from(a.vertices.len()));
            for point in a.points.iter() {
                let p = [point.x.get(), point.y.get(), point.z.get()];
                let [low, high] = bounds.get_or_insert([p, p]);
                for ((l, h), v) in low.iter_mut().zip(high.iter_mut()).zip(p) {
                    *l = (*l).min(v);
                    *h = (*h).max(v);
                }
            }
        }
        // Volume and area come with the core queries (G102).
        Measures {
            volume6: None,
            area2: None,
            counts,
            bounds,
            exactness: self.exactness.as_ref().map_or(Class::Exact, Class::from),
        }
    }
}
