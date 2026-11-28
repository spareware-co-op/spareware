//! The commands that make and change railings (job 14.2, CON-A4-F5-P3, first half). They go
//! through the same edit log as `element.*`, with the same access check (`model.edit`) and the
//! same claims: a railing another person is working on is refused, and one person's edits land in
//! log order. Each is one entry, so one undo takes back all it wrote ([`crate::cloud::undo`]). They
//! read as the stair commands do ([`crate::cloud::stairs`]).
//!
//! A railing runs along a sketched path on its level, or follows a stair on one side or both; its
//! layout (where every baluster stands, its rails' lengths, its openings) is worked out from its
//! path or from its stair as the stair's levels lay it out, and never stored
//! (`crates/construct-railings/FORMAT.md`). So a railing on a stair that changes or whose levels
//! move is laid out again by regeneration and needs no command for it. Each command checks the
//! railing the way the builders read it ([`construct_regen::levels::recipe_at`], which lays it out
//! through [`construct_regen::railings`]), so a railing a command takes is one the builders take,
//! and what construct-railings refuses is refused in its words. A limit of its rule set broken (a
//! barrier too low, an opening a sphere passes) is not a refusal: it is the railing's warning, in
//! the view.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `railing.create` | `element`, `name` (default `Railing`), `level`, `offset` (typed text, default `0`), `path` (`[[x, y], ...]` typed text, at least two points), `type` (fields of the standard type to change, typed text), `category` (default `household-edge`), `openings` (default the category's first), `rules` (default `nz-f4-as1`), `flipped` (default `false`) | A new sketched railing |
//! | `railing.stair` | `element`, `name`, `stair` (a stair's ID), `side` (`left`, `right` or `both`, default `both`), `inset` (default `50 mm`), `type`, `category` (default `household-stair`), `openings`, `rules` | A new railing on a stair, its base tied as the stair's is |
//! | `railing.type` | `element`, and any of `type` (fields to change), `category`, `openings` | The railing: its type and the limits it is checked by |
//! | `railing.path` | `element`; a sketched railing's `path`, or a stair's railing's `stair`, `side` and or `inset` | The railing: where it runs |
//! | `railing.flip` | `element` | The railing: a sketched one's handrail to its other side, a stair's to the stair's other side |
//! | `railing.delete` | `element` | The railing goes |
//!
//! Refused, in words: an element of that ID; a base level not in the model; a blank name; a
//! stair that is not one, or not in the model; a side not among its names; text that is no
//! length; an element that is not a railing; and anything construct-railings or the builders
//! refuse.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a railing,
//! `element.set` or `element.move` of one and `element.delete` of one are refused, naming the
//! command to use.
//!
//! For the page, without the log: [`read_railings`], each railing's quantities and warnings or why
//! it has none, and the rule set's categories and openings rules a person picks from.
//!
//! Typed text is read with construct-params' metric display settings until a project keeps its
//! own.

use crate::cloud::elements::write_record;
use crate::cloud::levels;
use construct_levels::{Length, Tie, base_of, parse_length, show};
use construct_model::element::Element;
use construct_params::{Units, Value};
use construct_railings::{
    NZ_F4_AS1, OpeningLimits, Path, RAILING_KIND, Railing, RailingType, RuleSet, Settings, Side,
    is_railing, is_railing_recipe, railing_of, railing_record, with_railing,
};
use construct_regen::parameters::{self as records, At};
use construct_stairs::is_stair;
use kit_core::Id;
use kit_ops::{Change, Claim, ClaimChange, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Serialize};

/// Every command here.
pub const COMMANDS: [&str; 6] = [
    "railing.create",
    "railing.stair",
    "railing.type",
    "railing.path",
    "railing.flip",
    "railing.delete",
];

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// The name a railing is given when none is.
const DEFAULT_NAME: &str = "Railing";
/// The category a sketched railing is of when none is given.
const DEFAULT_CATEGORY: &str = "household-edge";
/// The category a railing on a stair is of when none is given.
const DEFAULT_STAIR_CATEGORY: &str = "household-stair";
/// How far in from a stair's edge a railing stands when not given.
const DEFAULT_INSET: Length = Length::from_mm(50);

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    name: Option<String>,
    /// The base level: its ID, or an old page's integer ([`levels::tie_of_new`]).
    level: Option<serde_json::Value>,
    offset: Option<String>,
    path: Option<Vec<[String; 2]>>,
    #[serde(rename = "type")]
    kind: Option<serde_json::Value>,
    category: Option<String>,
    openings: Option<String>,
    rules: Option<String>,
    flipped: Option<bool>,
    #[serde(default, with = "kit_ops::id::option")]
    stair: Option<Id>,
    side: Option<String>,
    inset: Option<String>,
}

/// The part of a person's row a claim needs.
#[derive(Deserialize)]
struct Named {
    name: String,
}

/// A command's answer: a store failure, or the command's own result.
type Answer = Result<Outcome, StoreError>;

fn need<T>(value: Option<T>, what: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("This command needs {what}."))
}

/// Turns a data crate's refusal into the command's result.
fn why(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// How typed lengths are read and shown: construct-params' metric display settings, until a
/// project keeps its own.
fn units() -> Units {
    Units::default()
}

/// A typed length, or construct-levels' refusal of the text.
fn length(text: &str) -> Result<Length, String> {
    parse_length(text, &units()).map_err(why)
}

/// A sketched path from what a person sent.
fn points(sent: Vec<[String; 2]>) -> Result<Vec<[Length; 2]>, String> {
    sent.into_iter()
        .map(|[x, y]| Ok([length(&x)?, length(&y)?]))
        .collect()
}

/// A side from its name.
fn side(name: &str) -> Result<Side, String> {
    Side::from_name(name).ok_or_else(|| {
        format!("A railing follows a stair's left side, its right or both, not {name:?}.")
    })
}

/// A name that is not blank.
fn named(name: Option<String>) -> Result<String, String> {
    match name {
        Some(name) if name.trim().is_empty() => Err("A railing needs a name.".to_owned()),
        Some(name) => Ok(name.trim().to_owned()),
        None => Ok(DEFAULT_NAME.to_owned()),
    }
}

/// The standard type with what a person sent changed.
fn kind_of(sent: Option<&serde_json::Value>, base: &RailingType) -> Result<RailingType, String> {
    match sent {
        Some(patch) => base.patched(patch, &units()).map_err(why),
        None => Ok(base.clone()),
    }
}

/// The openings rule a railing of `category` is checked by: the one sent, or the category's
/// first.
fn openings_of(rules: &str, category: &str, sent: Option<String>) -> Result<String, String> {
    if let Some(sent) = sent {
        return Ok(sent);
    }
    let rule_set = RuleSet::builtin(rules).map_err(why)?;
    let category = rule_set.category(category).map_err(why)?;
    category
        .openings
        .first()
        .cloned()
        .ok_or_else(|| format!("The category {} has no openings rule.", category.id))
}

/// What a command is run against: the store, where, who, and when the orderer says it is.
struct Run<'a, T: Txn> {
    txn: &'a mut T,
    project: &'a Id,
    document: &'a Id,
    edit: &'a Edit,
    now_ms: u64,
    claim_ms: u64,
}

impl<T: Txn> Run<'_, T> {
    /// The claim this person takes on a piece, or who else holds it.
    async fn claim(&mut self, piece: &Id) -> Result<Result<Claim, String>, StoreError> {
        let holder = claims::held_by_another(
            self.txn,
            self.project,
            self.document,
            piece,
            &self.edit.person,
            self.now_ms,
        )
        .await?;
        if let Some(claim) = holder {
            return Ok(Err(format!("{} is working on this.", claim.name)));
        }
        let person: Option<Named> =
            read_row(self.txn, &Place::Person(self.edit.person.clone()).key()).await?;
        let name = person.map_or_else(|| self.edit.person.to_string(), |person| person.name);
        Ok(Ok(Claim {
            person: self.edit.person.clone(),
            name,
            // milliseconds since 1970 plus a claim's length: far below u64::MAX
            until_ms: self.now_ms.saturating_add(self.claim_ms),
        }))
    }

    /// The document's current record of a piece, and its hash.
    async fn current(&mut self, id: &Id) -> Result<Option<(Id, Element)>, StoreError> {
        records::read_element(self.txn, self.project, self.document, id).await
    }

    /// Writes a record and points its piece at it; the change, from what it was.
    async fn put(&mut self, record: &Element) -> Result<Change, StoreError> {
        let before = self.current(&record.id).await?.map(|(hash, _)| hash);
        let hash = write_record(self.txn, self.project, record).await?;
        let pointer = Place::Piece {
            project: self.project.clone(),
            document: self.document.clone(),
            piece: record.id.clone(),
        };
        self.txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
        Ok(Change {
            piece: record.id.clone(),
            before,
            after: Some(hash),
        })
    }

    /// Writes `record`, whose piece this person claims, once the builders would take it: read the
    /// way they read it, laid out along its path or its stair as the document is now.
    async fn checked(&mut self, record: &Element) -> Answer {
        let built = construct_regen::levels::recipe_at(
            self.txn,
            self.project,
            self.document,
            record,
            At::Now,
        )
        .await?;
        if let Err(failed) = built {
            return Ok(Err(failed.message));
        }
        let claim = match self.claim(&record.id).await? {
            Ok(claim) => claim,
            Err(refusal) => return Ok(Err(refusal)),
        };
        let change = self.put(record).await?;
        let claim = claims::take(self.txn, self.project, self.document, &record.id, claim).await?;
        Ok(Ok(Applied {
            changes: vec![change],
            claims: vec![claim],
            ..Applied::default()
        }))
    }
}

/// Runs one of these commands.
///
/// # Errors
///
/// A store error; a command that cannot be made is the outcome's error instead.
pub async fn apply<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Ordered<'_>,
    edit: &Edit,
    claim_ms: u64,
) -> Result<Outcome, StoreError> {
    let args: Args = match serde_json::from_value(edit.args.clone()) {
        Ok(args) => args,
        Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
    };
    let mut run = Run {
        txn,
        project,
        document: &edit.target,
        edit,
        now_ms: context.now_ms,
        claim_ms,
    };
    match edit.cmd.as_str() {
        "railing.create" => create(&mut run, args).await,
        "railing.stair" => on_stair(&mut run, args).await,
        "railing.type" => retype(&mut run, args).await,
        "railing.path" => repath(&mut run, args).await,
        "railing.flip" => flip(&mut run, args).await,
        "railing.delete" => delete(&mut run, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// An element ID not yet in the document, or the refusal.
async fn fresh<T: Txn>(
    run: &mut Run<'_, T>,
    element: Option<Id>,
) -> Result<Result<Id, String>, StoreError> {
    let id = match need(element, "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    Ok(Ok(id))
}

async fn create<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match fresh(run, args.element.clone()).await? {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let base =
        match levels::tie_of_new(run.txn, run.project, run.document, args.level.as_ref()).await? {
            Ok(Some(base)) => base,
            Ok(None) => {
                return Ok(Err(
                    "A railing stands on a level: this command needs a level.".to_owned(),
                ));
            }
            Err(refusal) => return Ok(Err(refusal)),
        };
    let made = (|| {
        let base = match args.offset.as_deref().map(length) {
            None => base,
            Some(offset) => Tie {
                offset: offset?,
                ..base
            },
        };
        let name = named(args.name)?;
        let rules = args.rules.unwrap_or_else(|| NZ_F4_AS1.to_owned());
        let category = args.category.unwrap_or_else(|| DEFAULT_CATEGORY.to_owned());
        let openings = openings_of(&rules, &category, args.openings)?;
        let railing = Railing::new(Settings {
            kind: kind_of(args.kind.as_ref(), &RailingType::standard())?,
            path: Path::Sketched {
                points: points(need(args.path, "a path, `path`")?)?,
            },
            rules,
            category,
            openings,
            flipped: args.flipped.unwrap_or(false),
        })
        .map_err(why)?;
        Ok::<_, String>(railing_record(&id, run.document, &name, &base, &railing))
    })();
    match made {
        Ok(record) => run.checked(&record).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

async fn on_stair<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let id = match fresh(run, args.element.clone()).await? {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let stair_id = match need(args.stair.clone(), "a stair, `stair`") {
        Ok(stair) => stair,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((_, stair)) = run.current(&stair_id).await? else {
        return Ok(Err(format!("There is no stair {stair_id} in this model.")));
    };
    if !is_stair(&stair) {
        return Ok(Err(format!("{} is not a stair.", stair.name)));
    }
    let base = match base_of(&stair) {
        Ok(Some(base)) => base,
        Ok(None) => return Ok(Err(format!("{} stands on no level.", stair.name))),
        Err(e) => return Ok(Err(why(e))),
    };
    let made = (|| {
        let name = named(args.name)?;
        let rules = args.rules.unwrap_or_else(|| NZ_F4_AS1.to_owned());
        let category = args
            .category
            .unwrap_or_else(|| DEFAULT_STAIR_CATEGORY.to_owned());
        let openings = openings_of(&rules, &category, args.openings)?;
        let railing = Railing::new(Settings {
            kind: kind_of(args.kind.as_ref(), &RailingType::standard())?,
            path: Path::OnStair {
                stair: stair_id.clone(),
                side: args.side.as_deref().map_or(Ok(Side::Both), side)?,
                inset: args.inset.as_deref().map_or(Ok(DEFAULT_INSET), length)?,
            },
            rules,
            category,
            openings,
            flipped: false,
        })
        .map_err(why)?;
        Ok::<_, String>(railing_record(&id, run.document, &name, &base, &railing))
    })();
    match made {
        Ok(record) => run.checked(&record).await,
        Err(refusal) => Ok(Err(refusal)),
    }
}

/// The railing a command is about: its record and hash, and the railing it holds; or the refusal.
async fn target<T: Txn>(
    run: &mut Run<'_, T>,
    element: Option<Id>,
) -> Result<Result<(Id, Element, Railing), String>, StoreError> {
    let id = match need(element, "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((hash, record)) = run.current(&id).await? else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    if !is_railing(&record) {
        return Ok(Err(format!("{} is not a railing.", record.name)));
    }
    let railing = match railing_of(&record.recipe) {
        Ok(railing) => railing,
        Err(refusal) => return Ok(Err(why(refusal))),
    };
    Ok(Ok((hash, record, railing)))
}

/// `settings` made a railing and put in `record`, checked and written; or the refusal.
async fn rewrite<T: Txn>(run: &mut Run<'_, T>, record: Element, settings: Settings) -> Answer {
    let made = Railing::new(settings).and_then(|railing| with_railing(record, &railing));
    match made {
        Ok(record) => run.checked(&record).await,
        Err(refusal) => Ok(Err(why(refusal))),
    }
}

async fn retype<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, record, railing) = match target(run, args.element.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if args.kind.is_none() && args.category.is_none() && args.openings.is_none() {
        return Ok(Err(
            "This command needs a type, a category or an openings rule.".to_owned(),
        ));
    }
    let mut settings = railing.settings().clone();
    settings.kind = match kind_of(args.kind.as_ref(), &settings.kind) {
        Ok(kind) => kind,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Some(category) = args.category {
        settings.category = category;
        // A new category's openings rule is its first unless one is sent.
        if args.openings.is_none() {
            settings.openings = match openings_of(&settings.rules, &settings.category, None) {
                Ok(openings) => openings,
                Err(refusal) => return Ok(Err(refusal)),
            };
        }
    }
    if let Some(openings) = args.openings {
        settings.openings = openings;
    }
    rewrite(run, record, settings).await
}

async fn repath<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, record, railing) = match target(run, args.element.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut settings = railing.settings().clone();
    let changed = match &mut settings.path {
        Path::Sketched { points: held } => {
            if args.stair.is_some() || args.side.is_some() || args.inset.is_some() {
                return Ok(Err(
                    "A sketched railing is moved by its path, `path`.".to_owned()
                ));
            }
            need(args.path, "a path, `path`")
                .and_then(points)
                .map(|sent| *held = sent)
        }
        Path::OnStair {
            stair,
            side: held_side,
            inset,
        } => {
            if args.path.is_some() {
                return Ok(Err(
                    "A railing on a stair is moved by its stair, its side and its inset."
                        .to_owned(),
                ));
            }
            if args.stair.is_none() && args.side.is_none() && args.inset.is_none() {
                return Ok(Err(
                    "This command needs a stair, a side, an inset, or more than one.".to_owned(),
                ));
            }
            let mut set = || -> Result<(), String> {
                if let Some(id) = &args.stair {
                    stair.clone_from(id);
                }
                if let Some(name) = &args.side {
                    *held_side = side(name)?;
                }
                if let Some(text) = &args.inset {
                    *inset = length(text)?;
                }
                Ok(())
            };
            set()
        }
    };
    if let Err(refusal) = changed {
        return Ok(Err(refusal));
    }
    // A railing moved to another stair stands where that stair's base does.
    let mut record = record;
    if let (Some(id), Path::OnStair { .. }) = (&args.stair, &settings.path) {
        match run.current(id).await? {
            Some((_, stair)) if is_stair(&stair) => match base_of(&stair) {
                Ok(Some(base)) => construct_levels::write_base(&mut record.props, Some(&base)),
                Ok(None) => return Ok(Err(format!("{} stands on no level.", stair.name))),
                Err(e) => return Ok(Err(why(e))),
            },
            Some((_, other)) => return Ok(Err(format!("{} is not a stair.", other.name))),
            None => return Ok(Err(format!("There is no stair {id} in this model."))),
        }
    }
    rewrite(run, record, settings).await
}

async fn flip<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, record, railing) = match target(run, args.element.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    match railing.flipped() {
        Ok(flipped) => rewrite(run, record, flipped.settings().clone()).await,
        Err(refusal) => Ok(Err(why(refusal))),
    }
}

async fn delete<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (hash, record, _) = match target(run, args.element).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(refusal) = run.claim(&record.id).await? {
        return Ok(Err(refusal));
    }
    let pointer = Place::Piece {
        project: run.project.clone(),
        document: run.document.clone(),
        piece: record.id.clone(),
    };
    run.txn.delete(pointer.key()).await?;
    let claim: ClaimChange =
        claims::release(run.txn, run.project, run.document, &record.id).await?;
    Ok(Ok(Applied {
        changes: vec![Change {
            piece: record.id,
            before: Some(hash),
            after: None,
        }],
        claims: vec![claim],
        ..Applied::default()
    }))
}

/// The words for a railing command that should have been used.
fn use_instead(what: &str, command: &str) -> String {
    format!("{what} with {command}, which checks it as a railing.")
}

/// The part of a generic command's arguments these rules read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    recipe: Option<construct_model::geometry::Recipe>,
}

/// The railing rules of the generic commands, asked before one runs: a railing is made by
/// `railing.create` or `railing.stair`, not `element.create`; changed by `railing.type`,
/// `railing.path` and `railing.flip`, not `element.set` or `element.move`; and deleted by
/// `railing.delete`, not `element.delete`. The inner error is the refusal, naming the command to
/// use. An edit these rules have nothing to say of is left to the command. Picking a railing
/// (`element.claim`, `element.release`) is allowed.
///
/// # Errors
///
/// A [`StoreError`] when the store fails.
pub async fn generic_rules<T: Txn>(
    txn: &mut T,
    project: &Id,
    edit: &Edit,
) -> Result<Result<(), String>, StoreError> {
    let Ok(args) = serde_json::from_value::<GenericArgs>(edit.args.clone()) else {
        return Ok(Ok(()));
    };
    let a_railing_recipe = args.recipe.as_ref().is_some_and(is_railing_recipe);
    let made = a_railing_recipe || args.kind.as_deref() == Some(RAILING_KIND);
    let cmd = edit.cmd.as_str();
    if cmd == "element.create" && made {
        return Ok(Err(use_instead(
            "A railing is made",
            "railing.create or railing.stair",
        )));
    }
    if !matches!(cmd, "element.set" | "element.move" | "element.delete") {
        return Ok(Ok(()));
    }
    let Some(id) = &args.element else {
        return Ok(Ok(()));
    };
    Ok(
        match records::read_element(txn, project, &edit.target, id).await? {
            Some((_, record)) if is_railing(&record) => Err(match cmd {
                "element.delete" => {
                    use_instead(&format!("{} is deleted", record.name), "railing.delete")
                }
                "element.move" => use_instead(&format!("{} is moved", record.name), "railing.path"),
                _ => format!(
                    "{} is changed with railing.type, railing.path or railing.flip, which check it as a railing.",
                    record.name
                ),
            }),
            Some(_) if a_railing_recipe && cmd == "element.set" => Err(use_instead(
                "A railing is made",
                "railing.create or railing.stair",
            )),
            _ => Ok(()),
        },
    )
}

/// What every railing of a document reports now: each is laid out as the document stands and its
/// warnings are gathered, for the warnings view
/// ([`construct_warnings::Model::railings`]). A railing that cannot be laid out has none here (its
/// own listing says why).
///
/// # Errors
/// A [`StoreError`] when the store fails.
pub async fn measured_warnings<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Vec<construct_railings::Warning>, StoreError> {
    let all = records::read_document(reader, project, document).await?;
    let mut found = Vec::new();
    for (_, record) in all.into_values().filter(|(_, record)| is_railing(record)) {
        found.extend(
            construct_regen::railings::warnings_at(reader, project, document, &record, At::Now)
                .await?,
        );
    }
    Ok(found)
}

/// A length as the page reads it: its stored text and the same shown in units.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct LengthView {
    /// construct-params' stored text.
    pub value: String,
    /// The same shown in the document's units, such as `1000 mm`.
    pub shown: String,
}

impl LengthView {
    fn of(length: Length) -> Self {
        Self {
            value: Value::Length(length).stored(),
            shown: show(length, &units()),
        }
    }
}

/// One warning of a railing, as the page shows it (construct-warnings' shape).
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct WarningView {
    /// Its kind and its elements, `kind|id`.
    pub identity: String,
    /// Its kind's stable name.
    pub kind: &'static str,
    /// `notice`, `warning` or `error`.
    pub severity: &'static str,
    /// A plain sentence.
    pub words: String,
}

/// What a railing reads back, as the page shows it.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct LaidView {
    /// Its top rail's length.
    pub rail_length: LengthView,
    /// Its handrail's length, if it has one.
    pub handrail_length: Option<LengthView>,
    /// How many balusters.
    pub balusters: usize,
    /// How many posts.
    pub posts: usize,
    /// Its warnings, in kind order.
    pub warnings: Vec<WarningView>,
}

/// One railing as the page reads it.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct RailingView {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// The stair it follows, if it is on one.
    pub stair: Option<String>,
    /// Which side of its stair: `left`, `right` or `both`.
    pub side: Option<&'static str>,
    /// Its category's ID.
    pub category: String,
    /// Its openings rule's ID.
    pub openings: String,
    /// Its top rail's height.
    pub height: LengthView,
    /// What it reads back as the document stands; none where it cannot be laid out.
    pub laid: Option<LaidView>,
    /// Why it cannot be laid out, in words; none where it can.
    pub refusal: Option<String>,
}

/// One category of a rule set, as a person picks it.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct CategoryView {
    /// Its ID.
    pub id: String,
    /// Where the barrier stands, in the code's words.
    pub name: String,
    /// Its least height on the level.
    pub level_least: LengthView,
    /// Its least height on a flight, where the code sets one.
    pub flight_least: Option<LengthView>,
    /// The openings rules it may be checked by, its default first.
    pub openings: Vec<String>,
}

/// One openings rule, as a person picks it.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct OpeningsView {
    /// Its ID.
    pub id: String,
    /// Who uses the place, in the code's words.
    pub name: String,
    /// `sphere` (no sphere of its size passes) or `most` (the widest openings allowed).
    pub by: &'static str,
}

/// What the page asks of a document's railings.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct RailingsView {
    /// The rule set new railings are checked by.
    pub rules: String,
    /// That rule set's code and edition, in words.
    pub rules_name: String,
    /// Its categories: where a barrier stands.
    pub categories: Vec<CategoryView>,
    /// Its openings rules.
    pub openings: Vec<OpeningsView>,
    /// Every railing of the document, by ID.
    pub railings: Vec<RailingView>,
}

/// A document's railings as the page needs them, as the module header says. A railing that
/// cannot be laid out as the document stands (its stair gone, say) is listed with why.
///
/// # Errors
/// A store error.
pub async fn read_railings<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<RailingsView, StoreError> {
    let all = records::read_document(reader, project, document).await?;
    let mut railings = Vec::new();
    for (_, record) in all.into_values().filter(|(_, record)| is_railing(record)) {
        let laid =
            construct_regen::railings::layout_at(reader, project, document, &record, At::Now)
                .await?;
        let warnings =
            construct_regen::railings::warnings_at(reader, project, document, &record, At::Now)
                .await?;
        let (laid, refusal) = match laid {
            Ok((_, layout)) => (
                Some(LaidView {
                    rail_length: LengthView::of(layout.rail_length),
                    handrail_length: layout.handrail_length.map(LengthView::of),
                    balusters: layout.balusters,
                    posts: layout.posts,
                    warnings: warnings
                        .into_iter()
                        .map(|w| WarningView {
                            identity: w.identity(),
                            kind: w.kind,
                            severity: w.severity.word(),
                            words: w.words,
                        })
                        .collect(),
                }),
                None,
            ),
            Err(why) => (None, Some(why)),
        };
        let held = railing_of(&record.recipe).ok();
        let settings = held.as_ref().map(Railing::settings);
        let (stair, side) = match settings.map(|s| &s.path) {
            Some(Path::OnStair { stair, side, .. }) => (Some(stair.to_string()), Some(side.name())),
            _ => (None, None),
        };
        railings.push(RailingView {
            id: record.id.to_string(),
            name: record.name.clone(),
            stair,
            side,
            category: settings.map(|s| s.category.clone()).unwrap_or_default(),
            openings: settings.map(|s| s.openings.clone()).unwrap_or_default(),
            height: LengthView::of(
                settings.map_or(Length::from_steps(0), |s| s.kind.top_rail.height),
            ),
            laid,
            refusal,
        });
    }
    let (rules_name, categories, openings) = match RuleSet::builtin(NZ_F4_AS1) {
        Ok(set) => (
            format!("{}, {}", set.name, set.edition),
            set.categories
                .iter()
                .map(|c| CategoryView {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    level_least: LengthView::of(c.level_least.length),
                    flight_least: c.flight_least.as_ref().map(|l| LengthView::of(l.length)),
                    openings: c.openings.clone(),
                })
                .collect(),
            set.openings
                .iter()
                .map(|o| OpeningsView {
                    id: o.id.clone(),
                    name: o.name.clone(),
                    by: match o.limits {
                        OpeningLimits::Sphere { .. } => "sphere",
                        OpeningLimits::Most { .. } => "most",
                    },
                })
                .collect(),
        ),
        Err(e) => (why(e), Vec::new(), Vec::new()),
    };
    Ok(RailingsView {
        rules: NZ_F4_AS1.to_owned(),
        rules_name,
        categories,
        openings,
        railings,
    })
}
