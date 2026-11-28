//! Reports of problems and feedback (piece A100, note A100-report-a-problem.md): the co-op's
//! private reports store, what the page asks of it, and its routes.
//!
//! A report is kept under keys of its own, apart from every project: `rpt/next` (the last number
//! given), `rpt/report/<number>` (the [`Stored`] report), `rpt/shot/<number>` (its screenshot,
//! when it has one) and `rpt/by/<account>/<number>` (a sender's own list). No project's scan
//! reaches them and no project's reader names them; every route here is behind the sender's
//! sign-in and reads only the sender's own reports. The operators' commands (`spare-server
//! report ...`) read them all.
//!
//! What the page cannot know, the server adds to a draft the person sees before sending
//! ([`Reports::draft`]): the elements the failure is about and those they read (their records:
//! kind, recipe, inputs), every edit of the model's log that touched them, with each author as a
//! stand-in and the stand-ins' names taken out of the words, the program's version and engine,
//! and the server's own lines for the person's own requests ([`crate::recent`]). On receipt
//! ([`Reports::send`]) the report is checked against its shape and caps, the names of every other
//! person the server knows are taken out of its words, it is numbered and kept, and the server
//! replays it ([`crate::replay`]); a replay that ends in its failure marks it reproduced.

use crate::http::{ApiError, Me, Shared};
use crate::recent;
use crate::replay::{self, engine_of};
use crate::service::{Access, Refusal};
use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::Json;
use construct_api::cloud::Target;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{Model, PARAMETER_TABLE, Reads};
use construct_regen::parameters as records;
use construct_report::{
    ANOTHER, Failure, Invalid, Issue, Kind, Listed, MAX_EDITS, MAX_ELEMENTS, MAX_LINES, Part, Program,
    Report, SENDER, Step, Stored, Topic, Where, by_topic, other, public_issue,
};
use kit_accounts::rows::Person;
use kit_core::Id;
use kit_ops::Entry;
use kit_store::{Key, Kv, Place as KitPlace, Reader, Scope, StoreError, Txn, read_row, scan_rows,
    table, table_prefix, to_row};
use kit_sync::{Clock, read_log};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

/// The reports store's own root: apart from `p:` (projects) and `dir` (the directory).
const ROOT: &str = "rpt";
/// Report numbers are padded, so byte order is number order.
const WIDTH: usize = 20;
/// How far from the failure's elements the elements a draft names reach: what they read, and
/// what that reads.
const DEPTH: u8 = 2;
/// How many times a number is tried when two reports arrive at once.
const TRIES: usize = 8;

/// The server's version, as a report names it: the package's, and the build's when one is given
/// (`SPARE_BUILD` when the server is compiled).
#[must_use]
pub fn version() -> String {
    match option_env!("SPARE_BUILD") {
        Some(build) if !build.is_empty() => format!("{}+{build}", env!("CARGO_PKG_VERSION")),
        _ => env!("CARGO_PKG_VERSION").to_owned(),
    }
}

fn padded(number: u64) -> String {
    format!("{number:0WIDTH$}")
}

fn next_key() -> Key {
    table(ROOT, "next", &[])
}

fn report_key(number: u64) -> Key {
    table(ROOT, "report", &[&padded(number)])
}

fn shot_key(number: u64) -> Key {
    table(ROOT, "shot", &[&padded(number)])
}

fn by_key(person: &Id, number: u64) -> Key {
    table(ROOT, "by", &[person.as_str(), &padded(number)])
}

/// What the page asks a draft for: the elements the failure is about, the failing edit by its
/// client and counter, and the IDs of the person's requests whose lines it wants.
#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Ask {
    /// The elements, by ID.
    #[serde(default)]
    pub elements: Vec<String>,
    /// The browser tab that sent the failing edit.
    #[serde(default)]
    pub client: Option<String>,
    /// That edit's counter.
    #[serde(default)]
    pub counter: Option<u64>,
    /// Request IDs.
    #[serde(default)]
    pub requests: Vec<String>,
}

/// What the server adds to a report, for the person to see first: a report holding only the
/// server's parts, the failing edit's number in the log, and how many edits were left out over
/// the cap.
#[derive(Serialize, Debug)]
pub struct Draft {
    /// The server's parts: where, the elements, the edits, the program, the server's lines.
    pub report: Report,
    /// The failing edit's number in the log, when it was found.
    pub failing: Option<u64>,
    /// Edits that touched the elements but are over the cap.
    pub left_out: usize,
}

/// The co-op's private reports store, and the replays of what it holds.
pub struct Reports<K: Kv> {
    kv: K,
    access: Arc<Access<K>>,
    geometry: Arc<dyn Geometry>,
    clock: Arc<dyn Clock>,
}

/// The stand-ins for the people of a report's edits: the sender, then each other person in the
/// order they first appear.
struct StandIns<'a> {
    sender: &'a Id,
    others: Vec<Id>,
}

impl StandIns<'_> {
    fn of(&mut self, author: &Id) -> String {
        if author == self.sender {
            return SENDER.to_owned();
        }
        let at = if let Some(at) = self.others.iter().position(|o| o == author) {
            at
        } else {
            self.others.push(author.clone());
            self.others.len().saturating_sub(1)
        };
        other(at.saturating_add(1))
    }
}

/// The edits of `model`'s log that touched `ids` (a change of the element, or of who holds it),
/// up to and including the failing edit when there is one, each author as a stand-in; over the
/// cap, the earliest, and the failing edit last. Also how many were left out.
fn touching(
    log: &[Entry],
    model: &Id,
    ids: &BTreeSet<&str>,
    failing: Option<&Entry>,
    people: &mut StandIns<'_>,
) -> (Vec<Step>, usize) {
    let mut edits = Vec::new();
    for entry in log {
        if failing.is_some_and(|f| entry.seq > f.seq) {
            break;
        }
        let is_failing = failing.is_some_and(|f| f.seq == entry.seq);
        // A claim taken or let go touches its element too: a refusal may be about who holds it.
        let touches = entry.changes.iter().any(|c| ids.contains(c.piece.as_str()))
            || entry.claims.iter().any(|c| ids.contains(c.piece.as_str()));
        if entry.target != *model || !(touches || is_failing) {
            continue;
        }
        edits.push(Step {
            seq: entry.seq,
            by: people.of(&entry.author),
            at_ms: entry.orderer_ms,
            cmd: entry.cmd.clone(),
            args: entry.args.clone(),
            failed: entry.failed.clone(),
        });
    }
    let left_out = edits.len().saturating_sub(MAX_EDITS);
    if left_out > 0 {
        let last = edits.pop();
        edits.truncate(MAX_EDITS.saturating_sub(1));
        edits.extend(last);
    }
    (edits, left_out)
}

/// Every string in `value` that is an ID: its own, an object's values, a list's items, one level
/// deep.
fn ids_in(value: &Value) -> Vec<Id> {
    let one = |v: &Value| v.as_str().and_then(|text| Id::parse(text).ok());
    match value {
        Value::Object(fields) => fields
            .values()
            .flat_map(|v| match v {
                Value::Array(items) => items.iter().filter_map(one).collect(),
                other => one(other).into_iter().collect::<Vec<_>>(),
            })
            .collect(),
        Value::Array(items) => items.iter().filter_map(one).collect(),
        other => one(other).into_iter().collect(),
    }
}

fn bad(invalid: Invalid) -> Refusal {
    Refusal::Bad(invalid.0)
}

fn parse(text: &str) -> Result<Id, Refusal> {
    Id::parse(text).map_err(|_| Refusal::NotFound)
}

impl<K: Kv> Reports<K> {
    /// The store over `kv`, checking access with `access`, replaying with `geometry`, stamping
    /// what arrives by `clock`.
    pub fn new(
        kv: K,
        access: Arc<Access<K>>,
        geometry: Arc<dyn Geometry>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            kv,
            access,
            geometry,
            clock,
        }
    }

    async fn may_read(&self, person: &Id, project: &Id, model: &Id) -> Result<(), Refusal> {
        if self
            .access
            .allowed(person, "model.read", Target::Model(project, model), None)
            .await?
        {
            Ok(())
        } else {
            Err(Refusal::Refused("You don't have access to this.".to_owned()))
        }
    }

    /// The elements a draft names: `roots` that are in the model, then what each reads (its reads
    /// row's records, host, hosted elements and joined walls, and the elements its properties and
    /// references name), [`DEPTH`] steps out, at most [`MAX_ELEMENTS`].
    async fn closure<R: Reader>(
        reader: &mut R,
        project: &Id,
        model: &Id,
        roots: Vec<Id>,
    ) -> Result<Vec<Part>, StoreError> {
        let mut queue: VecDeque<(Id, u8)> = roots.into_iter().map(|id| (id, 0)).collect();
        let mut seen = BTreeSet::new();
        let mut parts = Vec::new();
        while let Some((id, depth)) = queue.pop_front() {
            if parts.len() >= MAX_ELEMENTS {
                break;
            }
            if !seen.insert(id.clone()) {
                continue;
            }
            let Some((_, record)) = records::read_element(reader, project, model, &id).await?
            else {
                continue;
            };
            let value = serde_json::to_value(&record)
                .map_err(|e| StoreError::Damaged(e.to_string()))?;
            if depth < DEPTH {
                let next = depth.saturating_add(1);
                let place = Place::Reads {
                    project: project.clone(),
                    document: model.clone(),
                    element: id.clone(),
                };
                let mut named: Vec<Id> = Vec::new();
                if let Some(reads) = read_row::<Reads, _>(reader, &place.key()).await? {
                    let texts = reads
                        .records
                        .iter()
                        .chain(reads.host.iter())
                        .chain(reads.hosted.iter())
                        .chain(reads.joins.iter());
                    named.extend(texts.filter_map(|text| Id::parse(text).ok()));
                }
                if let Some(props) = value.get("props") {
                    named.extend(ids_in(props));
                }
                if let Some(references) = value.get("references").and_then(Value::as_array) {
                    let elements = references.iter().filter_map(|r| r.get("element"));
                    named.extend(elements.flat_map(ids_in));
                }
                queue.extend(named.into_iter().map(|id| (id, next)));
            }
            parts.push(Part {
                id: id.to_string(),
                record: value,
            });
        }
        Ok(parts)
    }

    /// What the server adds to `person`'s report about `model`, for them to see first.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model, or the store fails.
    pub async fn draft(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        ask: &Ask,
    ) -> Result<Draft, Refusal> {
        self.may_read(person, project, model).await?;
        let mut snapshot = self.kv.snapshot().await?;
        let scope = Scope::Project(project.clone());
        let head: u64 = read_row(&mut snapshot, &scope.head()).await?.unwrap_or(0);
        let log = read_log(&mut snapshot, &scope, 0, head).await?;
        let client = ask.client.as_deref().and_then(|c| Id::parse(c).ok());
        let failing = log.iter().find(|entry| {
            entry.target == *model
                && entry.author == *person
                && entry.failed.is_some()
                && client.as_ref() == Some(&entry.client)
                && ask.counter == Some(entry.counter)
        });
        let mut roots: Vec<Id> = ask.elements.iter().filter_map(|e| Id::parse(e).ok()).collect();
        if let Some(entry) = failing {
            roots.extend(ids_in(&entry.args));
        }
        // Every value a parameter holds is read through the table of the document's parameters.
        roots.extend(Id::parse(PARAMETER_TABLE).ok());
        let elements = Self::closure(&mut snapshot, project, model, roots).await?;
        let ids: BTreeSet<&str> = elements.iter().map(|p| p.id.as_str()).collect();
        let mut people = StandIns {
            sender: person,
            others: Vec::new(),
        };
        let (edits, left_out) = touching(&log, model, &ids, failing, &mut people);
        let mut names = Vec::new();
        for (at, other_person) in people.others.iter().enumerate() {
            let row: Option<Person> =
                read_row(&mut snapshot, &KitPlace::Person(other_person.clone()).key()).await?;
            if let Some(row) = row {
                names.push((row.name, other(at.saturating_add(1))));
            }
        }
        let header: Option<Model> = read_row(
            &mut snapshot,
            &Place::Model {
                project: project.clone(),
                model: model.clone(),
            }
            .key(),
        )
        .await?;
        let engine = header.map_or_else(
            || engine_of(&self.geometry.edition()),
            |m| engine_of(&m.edition),
        );
        let mut report = Report {
            format: construct_report::FORMAT,
            kind: Kind::Problem,
            summary: String::new(),
            words: String::new(),
            topic: None,
            failure: None,
            place: Some(Where {
                project: project.to_string(),
                model: model.to_string(),
            }),
            elements,
            edits,
            program: Some(Program {
                version: version(),
                engine,
            }),
            browser: None,
            page_errors: Vec::new(),
            server_log: recent::lines_for(person.as_str(), &ask.requests, MAX_LINES),
            screenshot: None,
        };
        report.scrub(&names).map_err(bad)?;
        // The failure is the failing edit's, in its words with the stand-ins in: the words the
        // replay gives back when it reproduces it.
        report.failure = report
            .failing()
            .filter(|step| failing.is_some_and(|f| f.seq == step.seq))
            .and_then(|step| {
                let words = step.failed.clone()?;
                Some(Failure::refused(&step.cmd, words))
            });
        Ok(Draft {
            report,
            failing: failing.map(|f| f.seq),
            left_out,
        })
    }

    /// The names of everyone the server knows but `person`, each to read as [`ANOTHER`].
    async fn others(&self, person: &Id) -> Result<Vec<(String, String)>, StoreError> {
        let mut snapshot = self.kv.snapshot().await?;
        let people: Vec<Person> =
            scan_rows(&mut snapshot, &table_prefix("dir", "person", &[])).await?;
        Ok(people
            .into_iter()
            .filter(|p| p.id != *person)
            .map(|p| (p.name, ANOTHER.to_owned()))
            .collect())
    }

    /// Numbers and keeps a report.
    async fn keep(
        &self,
        person: &Id,
        received_ms: u64,
        report: &Report,
    ) -> Result<Stored, Refusal> {
        for _ in 0..TRIES {
            let mut txn = self.kv.begin().await?;
            let last: u64 = read_row(&mut txn, &next_key()).await?.unwrap_or(0);
            let Some(number) = last.checked_add(1) else {
                let _ = txn.rollback().await;
                return Err(Refusal::Failed(StoreError::Damaged(
                    "report numbers are used up".to_owned(),
                )));
            };
            let (stored, shot) =
                Stored::received(number, person.as_str(), received_ms, report.clone());
            txn.put(report_key(number), to_row(&stored)?).await?;
            if let Some(shot) = &shot {
                txn.put(shot_key(number), to_row(shot)?).await?;
            }
            txn.put(by_key(person, number), to_row(&number)?).await?;
            txn.put(next_key(), to_row(&number)?).await?;
            match txn.commit().await {
                Ok(()) => return Ok(stored),
                // Another report took the number: try the next.
                Err(StoreError::Conflict) => {}
                Err(e) => return Err(e.into()),
            }
        }
        Err(Refusal::Failed(StoreError::Unavailable(
            "too many reports arrived at once".to_owned(),
        )))
    }

    async fn write(&self, stored: &Stored) -> Result<(), StoreError> {
        let mut txn = self.kv.begin().await?;
        txn.put(report_key(stored.number), to_row(stored)?).await?;
        txn.commit().await
    }

    /// Receives `person`'s report: checks it, takes other people's names out of its words, numbers
    /// and keeps it, and replays a problem. The answer is the report as their list shows it.
    ///
    /// # Errors
    ///
    /// [`Refusal::Bad`] with the words when it is not a report this server keeps; a [`Refusal`]
    /// when it is about a model this person may not read, or the store fails.
    pub async fn send(&self, person: &Id, mut report: Report) -> Result<Listed, Refusal> {
        report.check().map_err(bad)?;
        if let Some(place) = &report.place {
            let (project, model) = (parse(&place.project)?, parse(&place.model)?);
            self.may_read(person, &project, &model).await?;
        }
        let names = self.others(person).await?;
        report.scrub(&names).map_err(bad)?;
        let mut stored = self.keep(person, self.clock.now_ms(), &report).await?;
        if stored.report.kind == Kind::Problem {
            let found =
                replay::replay(&self.kv, self.geometry.as_ref(), stored.number, &stored.report)
                    .await;
            match found {
                Ok(found) => {
                    stored.replayed(found);
                    self.write(&stored).await?;
                }
                // The report is kept as received; an operator replays it again.
                Err(e) => tracing::warn!(number = stored.number, "a report's replay failed: {e}"),
            }
        }
        tracing::info!(number = stored.number, "a report was received");
        Ok(stored.listed())
    }

    /// `person`'s own reports, oldest first.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when the store fails.
    pub async fn mine(&self, person: &Id) -> Result<Vec<Listed>, Refusal> {
        let mut snapshot = self.kv.snapshot().await?;
        let numbers: Vec<u64> =
            scan_rows(&mut snapshot, &table_prefix(ROOT, "by", &[person.as_str()])).await?;
        let mut listed = Vec::with_capacity(numbers.len());
        for number in numbers {
            let stored: Option<Stored> = read_row(&mut snapshot, &report_key(number)).await?;
            listed.extend(stored.map(|s| s.listed()));
        }
        Ok(listed)
    }

    /// One of `person`'s own reports, whole but for its screenshot.
    ///
    /// # Errors
    ///
    /// [`Refusal::NotFound`] when there is no such report of theirs; a [`Refusal`] when the store
    /// fails.
    pub async fn one(&self, person: &Id, number: u64) -> Result<Stored, Refusal> {
        match self.get(number).await? {
            Some(stored) if stored.sender == person.as_str() => Ok(stored),
            _ => Err(Refusal::NotFound),
        }
    }

    // --- The operators' side: `spare-server report ...`, never a route ---

    /// Report `number`, whoever sent it.
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the store fails.
    pub async fn get(&self, number: u64) -> Result<Option<Stored>, StoreError> {
        let mut snapshot = self.kv.snapshot().await?;
        read_row(&mut snapshot, &report_key(number)).await
    }

    async fn change(
        &self,
        number: u64,
        how: impl FnOnce(&mut Stored) -> Result<(), Invalid>,
    ) -> Result<Stored, String> {
        let mut stored = self
            .get(number)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("There is no report {number}."))?;
        how(&mut stored).map_err(|e| e.0)?;
        self.write(&stored).await.map_err(|e| e.to_string())?;
        Ok(stored)
    }

    /// Marks report `number` fixed in `version`.
    ///
    /// # Errors
    ///
    /// Why not, in words.
    pub async fn fixed(&self, number: u64, version: &str) -> Result<Stored, String> {
        self.change(number, |stored| stored.fixed(version)).await
    }

    /// Records the public issue report `number` became.
    ///
    /// # Errors
    ///
    /// Why not, in words.
    pub async fn linked(&self, number: u64, issue: &str) -> Result<Stored, String> {
        self.change(number, |stored| {
            stored.issue = Some(issue.to_owned());
            Ok(())
        })
        .await
    }

    /// Replays report `number` again, as it is kept.
    ///
    /// # Errors
    ///
    /// Why not, in words.
    pub async fn replay_again(&self, number: u64) -> Result<Stored, String> {
        let stored = self
            .get(number)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("There is no report {number}."))?;
        let found = replay::replay(&self.kv, self.geometry.as_ref(), number, &stored.report)
            .await
            .map_err(|e| e.to_string())?;
        self.change(number, |stored| {
            stored.replayed(found);
            Ok(())
        })
        .await
    }

    /// The generic public issue report `number` becomes.
    ///
    /// # Errors
    ///
    /// Why not, in words.
    pub async fn issue(&self, number: u64) -> Result<Issue, String> {
        let stored = self
            .get(number)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("There is no report {number}."))?;
        public_issue(&stored).map_err(|e| e.0)
    }

    /// Every report, oldest first.
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the store fails.
    pub async fn all(&self) -> Result<Vec<Stored>, StoreError> {
        let mut snapshot = self.kv.snapshot().await?;
        scan_rows(&mut snapshot, &table_prefix(ROOT, "report", &[])).await
    }

    /// Every piece of feedback, grouped by its topic.
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the store fails.
    pub async fn feedback(&self) -> Result<BTreeMap<Topic, Vec<u64>>, StoreError> {
        Ok(by_topic(&self.all().await?))
    }
}

/// The routes of the reports store: a draft of what the server adds, sending a report, and the
/// sender's own list and reports. Each is behind the sender's sign-in.
pub fn routes() -> Router<Shared> {
    Router::new()
        .route(
            "/api/projects/{project}/models/{model}/report-draft",
            post(draft),
        )
        .route("/api/reports", get(mine).post(send))
        .route("/api/reports/{number}", get(one))
}

fn unread(rejection: &JsonRejection) -> ApiError {
    ApiError(Refusal::Bad(format!(
        "That isn't a report this server reads: {}",
        rejection.body_text()
    )))
}

async fn draft(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
    body: Result<Json<Ask>, JsonRejection>,
) -> Result<Json<Draft>, ApiError> {
    let Json(ask) = body.map_err(|e| unread(&e))?;
    let (project, model) = (parse(&project)?, parse(&model)?);
    Ok(Json(
        app.service
            .reports
            .draft(&person, &project, &model, &ask)
            .await?,
    ))
}

async fn send(
    State(app): State<Shared>,
    Me(person): Me,
    body: Result<Json<Report>, JsonRejection>,
) -> Result<Json<Listed>, ApiError> {
    let Json(report) = body.map_err(|e| unread(&e))?;
    Ok(Json(app.service.reports.send(&person, report).await?))
}

async fn mine(State(app): State<Shared>, Me(person): Me) -> Result<Json<Vec<Listed>>, ApiError> {
    Ok(Json(app.service.reports.mine(&person).await?))
}

async fn one(
    State(app): State<Shared>,
    Me(person): Me,
    Path(number): Path<String>,
) -> Result<Json<Stored>, ApiError> {
    let number = number.parse::<u64>().map_err(|_| ApiError(Refusal::NotFound))?;
    Ok(Json(app.service.reports.one(&person, number).await?))
}
