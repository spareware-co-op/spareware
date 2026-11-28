//! The server's replay of a report (piece A100, note A100-report-a-problem.md): the report's edits,
//! in log order, into an empty project, so that a fault reproduces without the person's project.
//!
//! Everything happens in one transaction of the store that is never committed: the empty project's
//! rows, its model (`model.add`), and then each edit run through the same commands the orderer
//! runs ([`ProjectCommands`], `Commands::apply`), at the log's own time for it, with its entry
//! written into the transaction as the orderer writes it, so undo, claims and every rule read the
//! replay's own log. The transaction is then rolled back: the kit's `Txn` promises that dropping
//! one leaves nothing behind, so nothing of a report ever lands outside the reports store.
//!
//! Access is not what a report is about, so the replay's commands are asked through a decider that
//! says yes, and `permit` is not asked at all; the people are the report's stand-ins (`sender`,
//! `other-1`), each a person row of the replay's own. A refusal is reproduced when the sender's
//! failing edit is turned back with the same words; a build failure when the element's recipe,
//! filled as the server fills it (`construct_regen::joins::recipe_at`), fails to build with the
//! same code and words. A body mismatch, a server failure or a page error happened outside what
//! the edits decide: the replay says so, and the report's lines are what a fixer reads.
//!
//! Interim: a host is built here on its own, not with what it hosts (the regenerator's
//! `build_hosting`), since that pass reads committed rows; a fault only a host's cut shows is not
//! reproduced yet. Job 13.2's history of an element replaces the scan of the whole log in
//! `report.rs`.

use construct_api::cloud::{Access, ProjectCommands};
use construct_model::geometry::{Edition, Geometry};
use construct_model::keys::Place;
use construct_model::rows::{Project, Team};
use construct_regen::parameters::{self as records, At};
use construct_report::{Replay, Replayed, Report, SENDER, Source};
use kit_accounts::rows::Person;
use kit_core::Id;
use kit_ops::{Decider, Edit, Entry, Question};
use kit_store::key::list as kit_list;
use kit_store::{Kv, Place as KitPlace, Reader, Scope, StoreError, Txn, to_row};
use kit_sync::{Applied, Commands, Context};
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;

/// How long a claim lasts in a replay: the server's own default.
const CLAIM_MS: u64 = 30_000;
/// The team and hub of the empty project.
const REPLAY: &str = "replay";

/// Says yes: a replay asks only the commands' own rules.
struct Yes;

impl Decider for Yes {
    fn decide(&self, _: &Question) -> Result<bool, String> {
        Ok(true)
    }
}

/// An engine and its version, as a report names it: `stand-in 1`.
#[must_use]
pub fn engine_of(edition: &Edition) -> String {
    format!("{} {}", edition.engine, edition.version)
}

fn not_replayed(engine: String, note: &str) -> Replay {
    Replay {
        engine,
        reproduced: false,
        code: None,
        words: None,
        steps: Vec::new(),
        note: Some(note.to_owned()),
    }
}

fn id(text: &str) -> Result<Id, StoreError> {
    Id::parse(text).map_err(|e| StoreError::Damaged(format!("{text:?}: {e}")))
}

/// Replays report `number` into an empty project of its own, built with `geometry`.
///
/// # Errors
///
/// A [`StoreError`] when the store fails. What the replay found, reproduced or not, is the
/// answer, never an error.
pub async fn replay<K: Kv>(
    kv: &K,
    geometry: &dyn Geometry,
    number: u64,
    report: &Report,
) -> Result<Replay, StoreError> {
    let engine = engine_of(&geometry.edition());
    let Some(failure) = &report.failure else {
        return Ok(not_replayed(engine, "There is no failure to replay."));
    };
    let Some(source) = failure.source() else {
        return Ok(not_replayed(engine, "Its failure code names no source."));
    };
    if !source.replayed() {
        return Ok(not_replayed(
            engine,
            "This failure happened outside what the edits decide: read the report's lines.",
        ));
    }
    let Some(place) = &report.place else {
        return Ok(not_replayed(engine, "It names no model to replay."));
    };
    let Ok(model) = Id::parse(&place.model) else {
        return Ok(not_replayed(engine, "Its model isn't an ID."));
    };
    let project = id(&format!("{REPLAY}.{number}"))?;
    let mut snapshot = kv.snapshot().await?;
    let taken = !snapshot.scan(&kit_list::project(&project)).await?.is_empty()
        || snapshot
            .get(&Place::Project(project.clone()).key())
            .await?
            .is_some();
    if taken {
        return Ok(not_replayed(
            engine,
            "A project with the replay's name is in the store, so it was not replayed.",
        ));
    }
    let mut txn = kv.begin().await?;
    let found = run(&mut txn, kv, geometry, &project, &model, report, source).await;
    // Whatever happened, nothing of the replay lands.
    let rolled = txn.rollback().await;
    let mut found = found?;
    rolled?;
    found.engine = engine;
    Ok(found)
}

/// The empty project: its rows, and a person row for each stand-in, named as the stand-in.
async fn empty<T: Txn>(txn: &mut T, project: &Id, report: &Report) -> Result<(), StoreError> {
    let replay = id(REPLAY)?;
    let row = Project {
        id: project.clone(),
        name: "Replay".to_owned(),
        hub: replay.clone(),
    };
    txn.put(Place::Project(project.clone()).key(), to_row(&row)?)
        .await?;
    let team = Team {
        id: replay.clone(),
        name: "Replay".to_owned(),
    };
    let place = Place::Team {
        project: project.clone(),
        team: replay,
    };
    txn.put(place.key(), to_row(&team)?).await?;
    let people: BTreeSet<&str> = std::iter::once(SENDER)
        .chain(report.edits.iter().map(|step| step.by.as_str()))
        .collect();
    for by in people {
        let person = Person {
            id: id(by)?,
            name: by.to_owned(),
        };
        txn.put(KitPlace::Person(person.id.clone()).key(), to_row(&person)?)
            .await?;
    }
    Ok(())
}

/// One edit, as the orderer runs it: the command, then its entry and the head.
async fn step<K: Kv>(
    commands: &ProjectCommands<K, Yes>,
    txn: &mut K::Txn,
    scope: &Scope,
    head: &mut u64,
    edit: &Edit,
    now_ms: u64,
) -> Result<Option<String>, StoreError> {
    let context = Context {
        scope,
        now_ms,
        head: *head,
    };
    let (applied, failed) = match commands.apply(txn, &context, edit).await? {
        Ok(applied) => (applied, None),
        Err(reason) => (Applied::default(), Some(reason)),
    };
    *head = head
        .checked_add(1)
        .ok_or_else(|| StoreError::Damaged("the replay's log is full".to_owned()))?;
    let entry = Entry::new(
        *head,
        edit,
        commands.version(),
        now_ms,
        applied.changes,
        applied.claims,
        failed.clone(),
    );
    txn.put(scope.log(*head), entry.encode()).await?;
    txn.put(scope.head(), to_row(head)?).await?;
    Ok(failed)
}

#[allow(clippy::too_many_arguments, reason = "one replay's parts, each read once")]
async fn run<K: Kv>(
    txn: &mut K::Txn,
    kv: &K,
    geometry: &dyn Geometry,
    project: &Id,
    model: &Id,
    report: &Report,
    source: Source,
) -> Result<Replay, StoreError> {
    empty(txn, project, report).await?;
    let access = Arc::new(Access::new(kv.clone(), Yes));
    let commands = ProjectCommands::new(kv.clone(), access, CLAIM_MS, geometry.edition());
    let scope = Scope::Project(project.clone());
    let mut head = 0_u64;
    let first_ms = report.edits.first().map_or(0, |step| step.at_ms);
    let sender = id(SENDER)?;
    let add = Edit {
        person: sender.clone(),
        client: id(&format!("{SENDER}.client"))?,
        counter: 0,
        base: 0,
        target: model.clone(),
        cmd: "model.add".to_owned(),
        args: json!({ "team": REPLAY, "name": "Replay", "package": "Replay package" }),
        author_ms: first_ms,
    };
    if let Some(why) = step(&commands, txn, &scope, &mut head, &add, first_ms).await? {
        return Ok(not_replayed(
            String::new(),
            &format!("The empty project's model could not be made: {why}"),
        ));
    }
    let mut steps = Vec::with_capacity(report.edits.len());
    for (counter, one) in (1_u64..).zip(&report.edits) {
        let edit = Edit {
            person: id(&one.by)?,
            client: id(&format!("{}.client", one.by))?,
            counter,
            base: head,
            target: model.clone(),
            cmd: one.cmd.clone(),
            args: one.args.clone(),
            author_ms: one.at_ms,
        };
        let now = step(&commands, txn, &scope, &mut head, &edit, one.at_ms).await?;
        steps.push(Replayed {
            seq: one.seq,
            cmd: one.cmd.clone(),
            then: one.failed.clone(),
            now,
        });
    }
    let (code, words, note) = match source {
        Source::Build => built(txn, geometry, project, model, report).await?,
        _ => refused(report, &steps),
    };
    let reproduced = report
        .failure
        .as_ref()
        .is_some_and(|f| code.as_deref() == Some(f.code.as_str()) && words.as_deref() == Some(f.words.as_str()));
    let differing = steps.iter().filter(|s| s.then != s.now).count();
    let note = match (note, differing) {
        (note, 0) => note,
        (Some(note), n) => Some(format!("{note} {n} edits ended otherwise than when they were made.")),
        (None, n) => Some(format!("{n} edits ended otherwise than when they were made.")),
    };
    Ok(Replay {
        engine: String::new(),
        reproduced,
        code,
        words,
        steps,
        note,
    })
}

type Found = (Option<String>, Option<String>, Option<String>);

/// How the sender's failing edit ended in the replay.
fn refused(report: &Report, steps: &[Replayed]) -> Found {
    let Some(failing) = report.failing() else {
        return (
            None,
            None,
            Some("The report holds no edit of the sender's that was turned back.".to_owned()),
        );
    };
    let Some(replayed) = steps.iter().find(|s| s.seq == failing.seq) else {
        return (None, None, None);
    };
    match &replayed.now {
        Some(words) => (
            Some(format!("{}.{}", Source::Refused.prefix(), replayed.cmd)),
            Some(words.clone()),
            None,
        ),
        None => (
            None,
            None,
            Some(format!("Edit {} applied in the replay.", failing.seq)),
        ),
    }
}

/// How the failure's element builds after the replay: filled as the server fills it, then built.
async fn built<R: Reader>(
    reader: &mut R,
    geometry: &dyn Geometry,
    project: &Id,
    model: &Id,
    report: &Report,
) -> Result<Found, StoreError> {
    let Some(element) = report
        .failure
        .as_ref()
        .and_then(|f| f.element.as_deref())
        .and_then(|e| Id::parse(e).ok())
    else {
        return Ok((None, None, Some("The failure names no element.".to_owned())));
    };
    let Some((_, record)) = records::read_element(reader, project, model, &element).await? else {
        return Ok((
            None,
            None,
            Some(format!("{element} is not there after the replay.")),
        ));
    };
    let at = At::Now;
    let recipe = if construct_regen::rooms::is_room(&record) {
        match construct_regen::rooms::worked_at(reader, project, model, &record, at).await? {
            Ok(worked) => worked.0,
            Err(failed) => Err(failed),
        }
    } else {
        construct_regen::joins::recipe_at(reader, project, model, &record, at).await?
    };
    let failed = match recipe {
        Ok(recipe) => geometry.build(&recipe).err(),
        Err(failed) => Some(failed),
    };
    Ok(match failed {
        Some(failed) => (
            Some(format!("{}.{}", Source::Build.prefix(), failed.code)),
            Some(failed.message),
            None,
        ),
        None => (None, None, Some(format!("{element} builds in the replay."))),
    })
}
