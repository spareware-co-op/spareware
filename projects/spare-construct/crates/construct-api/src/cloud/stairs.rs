//! The commands that make and change stairs (job 6.7, CON-A4-F5-P1, first half). They go through
//! the same edit log as `element.*`, with the same access check (`model.edit`) and the same
//! claims: a stair another person is working on is refused, and one person's edits land in log
//! order. Each is one entry, so one undo takes back all it wrote ([`crate::cloud::undo`]). They
//! read as the room and roof commands do ([`crate::cloud::rooms`], [`crate::cloud::roofs`]).
//!
//! A stair runs from its base level to its top level; its layout (the count of risers, each
//! riser's height, its runs and landings) is worked out from the height between them by its rule
//! set and never stored (`crates/construct-stairs/FORMAT.md`). So a stair whose levels move is
//! laid out again by regeneration and needs no command for it. Each command checks the stair the
//! way the builders read it ([`construct_regen::levels::recipe_at`], which lays it out through
//! [`construct_regen::stairs`]), so a stair a command takes is one the builders take, and what
//! construct-stairs refuses is refused in its words: a limit with its clause, and for a height no
//! count of risers lays out, the nearest heights that work.
//!
//! | Command | Arguments | What it writes |
//! | --- | --- | --- |
//! | `stair.create` | `element`, `name` (default `Stair`), `level`, `offset` (typed text, default `0`), `top` (a level's ID), `top_offset` (default `0`), `category` (default `common`), `rules` (default `nz-d1-as1`), `start` (`[x, y]` typed text), `direction` (`+x`, `-x`, `+y` or `-y`, default `+x`), `width` (default `1000 mm`), `going` (default the category's least), `waist` (default `150 mm`), `runs` (default one run, `[{}]`), `landings` (default none) | A new stair |
//! | `stair.width` | `element`, `width` | The stair: its width |
//! | `stair.direction` | `element`, `direction` and or `start` | The stair: which way it runs and where it starts |
//! | `stair.runs` | `element`, `runs`, `landings`, `going` (left as it is when not given) | The stair: its runs and landings |
//! | `stair.category` | `element`, `category` | The stair: the rule set's category it is laid out by |
//! | `stair.top` | `element`, `top` (a level's ID), `top_offset` (typed text, default `0`) | The stair: the level and offset its top is tied to |
//! | `stair.delete` | `element` | The stair goes |
//!
//! `runs` is a list of runs from the bottom, each `{}` to share the risers the others leave or
//! `{"risers": n}`; `landings` one fewer, each `{"length": "<typed text>", "turn": "straight" |
//! "left" | "right" | "back-left" | "back-right"}`. A stair's base level and offset are changed
//! by `level.tie`, as any element's; its top is changed by `stair.top`, which checks that it is above the base and that the stair still lays out.
//!
//! Refused, in words: an element of that ID; a base or top level not in the model, or a top that
//! is not above the base; a blank name; a direction or turn not among its names; text that is no
//! length; an element that is not a stair; and anything construct-stairs or the builders refuse.
//!
//! The generic commands do not go behind these ([`generic_rules`]): `element.create` of a stair,
//! `element.set` or `element.move` of one and `element.delete` of one are refused, naming the
//! command to use.
//!
//! For the page, without the log: [`read_stairs`], each stair's layout or why it has none, and the
//! rule set's categories a person picks from.
//!
//! Typed text is read with construct-params' metric display settings until a project keeps its
//! own.

use crate::cloud::elements::ElementTool;
use crate::cloud::elements::write_record;
use crate::cloud::levels;
use crate::cloud::tool::Tool;
use construct_levels::{Length, Tie, parse_length, show, write_top};
use construct_model::element::Element;
use construct_params::{Units, Value};
use construct_regen::parameters::{self as records, At};
use construct_stairs::{
    Direction, LandingSpec, NZ_D1_AS1, RuleSet, RunSpec, STAIR_KIND, Settings, Stair, Turn,
    is_stair, is_stair_recipe, stair_of, stair_record, with_stair,
};
use kit_core::Id;
use kit_ops::{Change, Claim, ClaimChange, Edit};
use kit_store::{Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context as Ordered, Outcome, claims};
use serde::{Deserialize, Serialize};

/// Every command here.
pub const COMMANDS: [&str; 7] = [
    "stair.create",
    "stair.width",
    "stair.direction",
    "stair.runs",
    "stair.category",
    "stair.top",
    "stair.delete",
];

/// Whether a command is one of these.
#[must_use]
pub fn is_command(cmd: &str) -> bool {
    COMMANDS.contains(&cmd)
}

/// The stair tool ([`Tool`]): stairs laid out by rules.
pub struct StairTool;

impl Tool for StairTool {
    const NAME: &'static str = "stair";
    const COMMANDS: &'static [&'static str] = &COMMANDS;
    const GUARDS: &'static [&'static str] = &[ElementTool::NAME];

    async fn apply<T: Txn>(
        txn: &mut T,
        project: &Id,
        context: &kit_sync::Context<'_>,
        edit: &Edit,
        claim_ms: u64,
    ) -> Result<Outcome, StoreError> {
        apply(txn, project, context, edit, claim_ms).await
    }

    async fn guard<T: Txn>(
        txn: &mut T,
        project: &Id,
        edit: &Edit,
    ) -> Result<Result<(), String>, StoreError> {
        generic_rules(txn, project, edit).await
    }
}

/// The name a stair is given when none is.
const DEFAULT_NAME: &str = "Stair";
/// The category a stair is of when none is given.
const DEFAULT_CATEGORY: &str = "common";
/// The width a stair is given when none is.
const DEFAULT_WIDTH: Length = Length::from_mm(1000);
/// The waist a stair is given when none is.
const DEFAULT_WAIST: Length = Length::from_mm(150);

/// One run as a person sends it.
#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
struct RunArgs {
    risers: Option<u32>,
}

/// One landing as a person sends it.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct LandingArgs {
    length: String,
    turn: String,
}

/// The arguments of every command here; each uses some of them.
#[derive(Deserialize, Default)]
struct Args {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    name: Option<String>,
    /// The base level: its ID, or an old page's integer ([`levels::tie_of_new`]).
    level: Option<serde_json::Value>,
    offset: Option<String>,
    #[serde(default, with = "kit_ops::id::option")]
    top: Option<Id>,
    top_offset: Option<String>,
    category: Option<String>,
    rules: Option<String>,
    start: Option<[String; 2]>,
    direction: Option<String>,
    width: Option<String>,
    going: Option<String>,
    waist: Option<String>,
    runs: Option<Vec<RunArgs>>,
    landings: Option<Vec<LandingArgs>>,
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

/// A direction from its name.
fn direction(name: &str) -> Result<Direction, String> {
    Direction::from_name(name)
        .ok_or_else(|| format!("A stair runs +x, -x, +y or -y, not {name:?}."))
}

/// A stair's start from what a person sent.
fn start(sent: [String; 2]) -> Result<[Length; 2], String> {
    let [x, y] = sent;
    Ok([length(&x)?, length(&y)?])
}

/// Runs and landings from what a person sent.
fn runs_of(runs: Vec<RunArgs>, landings: Vec<LandingArgs>) -> Result<RunsAndLandings, String> {
    let runs = runs
        .into_iter()
        .map(|r| RunSpec { risers: r.risers })
        .collect();
    let landings = landings
        .into_iter()
        .map(|l| {
            Ok(LandingSpec {
                length: length(&l.length)?,
                turn: Turn::from_name(&l.turn).ok_or_else(|| {
                    format!(
                        "A landing turns straight, left, right, back-left or back-right, not {:?}.",
                        l.turn
                    )
                })?,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok((runs, landings))
}

/// A stair's runs and the landings between them.
type RunsAndLandings = (Vec<RunSpec>, Vec<LandingSpec>);

/// A name that is not blank.
fn named(name: Option<String>, default: Option<&str>) -> Result<String, String> {
    match name {
        Some(name) if name.trim().is_empty() => Err("A stair needs a name.".to_owned()),
        Some(name) => Ok(name.trim().to_owned()),
        None => default
            .map(str::to_owned)
            .ok_or_else(|| "This command needs a name.".to_owned()),
    }
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

    /// Writes one record whose piece this person claims, as the command's whole result.
    async fn put_claimed(&mut self, record: &Element) -> Answer {
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

    /// What the builders would make of `record`, read the way they read it: its rise from its
    /// levels as they are now, laid out and shaped by construct-stairs; the refusal in its words
    /// when they would refuse it. Its top must be above its base first.
    async fn buildable(&mut self, record: &Element) -> Result<Result<(), String>, StoreError> {
        if let Err(refusal) =
            levels::top_is_above_base(self.txn, self.project, self.document, record).await?
        {
            return Ok(Err(refusal));
        }
        let built = construct_regen::levels::recipe_at(
            self.txn,
            self.project,
            self.document,
            record,
            At::Now,
        )
        .await?;
        Ok(built.map(|_| ()).map_err(|failed| failed.message))
    }

    /// Writes `record` once the builders would take it.
    async fn checked(&mut self, record: &Element) -> Answer {
        if let Err(refusal) = self.buildable(record).await? {
            return Ok(Err(refusal));
        }
        self.put_claimed(record).await
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
        "stair.create" => create(&mut run, args).await,
        "stair.width" => width(&mut run, args).await,
        "stair.direction" => turn(&mut run, args).await,
        "stair.runs" => runs(&mut run, args).await,
        "stair.category" => retype(&mut run, args).await,
        "stair.top" => retop(&mut run, args).await,
        "stair.delete" => delete(&mut run, args).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}

/// The stair a command is about: its record and hash, and the stair it holds; or the refusal.
async fn target<T: Txn>(
    run: &mut Run<'_, T>,
    element: Option<Id>,
) -> Result<Result<(Id, Element, Stair), String>, StoreError> {
    let id = match need(element, "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some((hash, record)) = run.current(&id).await? else {
        return Ok(Err("That element no longer exists.".to_owned()));
    };
    if !is_stair(&record) {
        return Ok(Err(format!("{} is not a stair.", record.name)));
    }
    let stair = match stair_of(&record.recipe) {
        Ok(stair) => stair,
        Err(refusal) => return Ok(Err(why(refusal))),
    };
    Ok(Ok((hash, record, stair)))
}

/// `settings` made a stair and put in `record`, checked and written; or the refusal.
async fn rewrite<T: Txn>(run: &mut Run<'_, T>, record: Element, settings: Settings) -> Answer {
    let stair = match Stair::new(settings) {
        Ok(stair) => stair,
        Err(refusal) => return Ok(Err(why(refusal))),
    };
    match with_stair(record, &stair) {
        Ok(record) => run.checked(&record).await,
        Err(refusal) => Ok(Err(why(refusal))),
    }
}

/// A stair's settings from what `stair.create` was sent.
fn new_settings(args: &mut Args) -> Result<Settings, String> {
    let start = start(need(args.start.take(), "a start, `start`")?)?;
    let (runs, landings) = runs_of(
        args.runs.take().unwrap_or_else(|| vec![RunArgs::default()]),
        args.landings.take().unwrap_or_default(),
    )?;
    Ok(Settings {
        rules: args.rules.take().unwrap_or_else(|| NZ_D1_AS1.to_owned()),
        category: args
            .category
            .take()
            .unwrap_or_else(|| DEFAULT_CATEGORY.to_owned()),
        start,
        direction: args
            .direction
            .take()
            .map_or(Ok(Direction::PlusX), |d| direction(&d))?,
        width: args
            .width
            .take()
            .map_or(Ok(DEFAULT_WIDTH), |w| length(&w))?,
        going: args.going.take().map(|g| length(&g)).transpose()?,
        waist: args
            .waist
            .take()
            .map_or(Ok(DEFAULT_WAIST), |w| length(&w))?,
        runs,
        landings,
    })
}

async fn create<T: Txn>(run: &mut Run<'_, T>, mut args: Args) -> Answer {
    let id = match need(args.element.clone(), "an element") {
        Ok(id) => id,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if run.current(&id).await?.is_some() {
        return Ok(Err("That element already exists.".to_owned()));
    }
    let name = match named(args.name.take(), Some(DEFAULT_NAME)) {
        Ok(name) => name,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let base =
        match levels::tie_of_new(run.txn, run.project, run.document, args.level.as_ref()).await? {
            Ok(Some(base)) => base,
            Ok(None) => {
                return Ok(Err(
                    "A stair stands on a level: this command needs a level.".to_owned(),
                ));
            }
            Err(refusal) => return Ok(Err(refusal)),
        };
    let base = match args.offset.as_deref().map(length) {
        None => base,
        Some(Ok(offset)) => Tie { offset, ..base },
        Some(Err(refusal)) => return Ok(Err(refusal)),
    };
    let top = match need(args.top.clone(), "a top level, `top`") {
        Ok(top) => top,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(refusal) = levels::level_now(run.txn, run.project, run.document, &top).await? {
        return Ok(Err(refusal));
    }
    let top = Tie {
        level: top,
        offset: match args
            .top_offset
            .as_deref()
            .map_or(Ok(Length::from_steps(0)), length)
        {
            Ok(offset) => offset,
            Err(refusal) => return Ok(Err(refusal)),
        },
    };
    let stair = match new_settings(&mut args).and_then(|s| Stair::new(s).map_err(why)) {
        Ok(stair) => stair,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let record = stair_record(&id, run.document, &name, &base, &top, &stair);
    run.checked(&record).await
}

async fn width<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, record, stair) = match target(run, args.element.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut settings = stair.settings();
    settings.width = match need(args.width.as_deref(), "a width").and_then(length) {
        Ok(width) => width,
        Err(refusal) => return Ok(Err(refusal)),
    };
    rewrite(run, record, settings).await
}

async fn turn<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, record, stair) = match target(run, args.element.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if args.direction.is_none() && args.start.is_none() {
        return Ok(Err(
            "This command needs a direction, a start, or both.".to_owned()
        ));
    }
    let mut settings = stair.settings();
    if let Some(name) = &args.direction {
        settings.direction = match direction(name) {
            Ok(d) => d,
            Err(refusal) => return Ok(Err(refusal)),
        };
    }
    if let Some(sent) = args.start {
        settings.start = match start(sent) {
            Ok(at) => at,
            Err(refusal) => return Ok(Err(refusal)),
        };
    }
    rewrite(run, record, settings).await
}

async fn runs<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, record, stair) = match target(run, args.element.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut settings = stair.settings();
    let sent = need(args.runs, "runs, `runs`")
        .and_then(|runs| runs_of(runs, args.landings.unwrap_or_default()));
    (settings.runs, settings.landings) = match sent {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Some(text) = &args.going {
        settings.going = match length(text) {
            Ok(going) => Some(going),
            Err(refusal) => return Ok(Err(refusal)),
        };
    }
    rewrite(run, record, settings).await
}

async fn retype<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, record, stair) = match target(run, args.element.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut settings = stair.settings();
    settings.category = match need(args.category, "a category, `category`") {
        Ok(category) => category,
        Err(refusal) => return Ok(Err(refusal)),
    };
    rewrite(run, record, settings).await
}

async fn retop<T: Txn>(run: &mut Run<'_, T>, args: Args) -> Answer {
    let (_, mut record, _) = match target(run, args.element.clone()).await? {
        Ok(found) => found,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let level = match need(args.top, "a top level, `top`") {
        Ok(level) => level,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(refusal) = levels::level_now(run.txn, run.project, run.document, &level).await? {
        return Ok(Err(refusal));
    }
    let offset = match args
        .top_offset
        .as_deref()
        .map_or(Ok(Length::from_steps(0)), length)
    {
        Ok(offset) => offset,
        Err(refusal) => return Ok(Err(refusal)),
    };
    write_top(&mut record.props, Some(&Tie { level, offset }));
    run.checked(&record).await
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

/// The words for a stair command that should have been used.
fn use_instead(what: &str, command: &str) -> String {
    format!("{what} with {command}, which checks it as a stair.")
}

/// The part of a generic command's arguments these rules read.
#[derive(Deserialize, Default)]
struct GenericArgs {
    #[serde(default, with = "kit_ops::id::option")]
    element: Option<Id>,
    kind: Option<String>,
    recipe: Option<construct_model::geometry::Recipe>,
}

/// The stair rules of the generic commands, asked before one runs: a stair is made by
/// `stair.create`, not `element.create`; it is changed by `stair.width`, `stair.direction` and
/// `stair.runs`, not `element.set` or `element.move`; and deleted by `stair.delete`, not
/// `element.delete`. The inner error is the refusal, naming the command to use. An edit these
/// rules have nothing to say of (another command, arguments that do not read, an element that is
/// not there or not a stair) is left to the command, which answers it as it always has. Picking a
/// stair (`element.claim`, `element.release`) is allowed.
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
    let a_stair_recipe = args.recipe.as_ref().is_some_and(is_stair_recipe);
    let made = a_stair_recipe || args.kind.as_deref() == Some(STAIR_KIND);
    let cmd = edit.cmd.as_str();
    if cmd == "element.create" && made {
        return Ok(Err(use_instead("A stair is made", "stair.create")));
    }
    if !matches!(cmd, "element.set" | "element.move" | "element.delete") {
        return Ok(Ok(()));
    }
    let Some(id) = &args.element else {
        return Ok(Ok(()));
    };
    Ok(
        match records::read_element(txn, project, &edit.target, id).await? {
            Some((_, record)) if is_stair(&record) => Err(match cmd {
                "element.delete" => {
                    use_instead(&format!("{} is deleted", record.name), "stair.delete")
                }
                "element.move" => {
                    use_instead(&format!("{} is moved", record.name), "stair.direction")
                }
                _ => format!(
                    "{} is changed with stair.width, stair.direction, stair.runs, stair.category or stair.top, which check it as a stair.",
                    record.name
                ),
            }),
            Some(_) if a_stair_recipe && cmd == "element.set" => {
                Err(use_instead("A stair is made", "stair.create"))
            }
            _ => Ok(()),
        },
    )
}

/// A length as the page reads it: its stored text and the same shown in units.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct LengthView {
    /// construct-params' stored text.
    pub value: String,
    /// The same shown in the document's units, such as `280 mm`.
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

/// One run of a stair's layout.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct RunView {
    /// Its risers.
    pub risers: usize,
    /// How far it rises.
    pub rise: LengthView,
    /// Its length in plan.
    pub length: LengthView,
}

/// One landing of a stair's layout.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct LandingView {
    /// Its length along the run that comes into it.
    pub length: LengthView,
    /// How it turns: `straight`, `left`, `right`, `back-left` or `back-right`.
    pub turn: &'static str,
    /// Its top's height above the stair's base.
    pub height: LengthView,
}

/// A stair's layout as the page shows it.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct LayoutView {
    /// The rise from base to top.
    pub rise: LengthView,
    /// How many risers.
    pub risers: usize,
    /// The tallest riser.
    pub riser_height: LengthView,
    /// The lowest riser: one grid step lower than the tallest where the rise does not divide
    /// evenly, else the same.
    pub lowest_riser: LengthView,
    /// How many treads.
    pub treads: usize,
    /// The going.
    pub going: LengthView,
    /// The total run: the walking line in plan.
    pub total_run: LengthView,
    /// The runs, from the bottom.
    pub runs: Vec<RunView>,
    /// The landings, from the bottom.
    pub landings: Vec<LandingView>,
}

/// One stair as the page reads it.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct StairView {
    /// Its permanent ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its rule set's ID.
    pub rules: String,
    /// Its category's ID.
    pub category: String,
    /// Which way its first run goes up.
    pub direction: &'static str,
    /// Its width.
    pub width: LengthView,
    /// How high its base sits above the project's zero; none where its level cannot be read.
    pub base: Option<LengthView>,
    /// Its layout as the document stands; none where it cannot be laid out.
    pub layout: Option<LayoutView>,
    /// Why it cannot be laid out, in construct-stairs' words; none where it can.
    pub refusal: Option<String>,
}

/// One category of a rule set, as a person picks it.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct CategoryView {
    /// Its ID.
    pub id: String,
    /// Its name.
    pub name: String,
}

/// Where a floor plan cuts, for drawing what a stair shows above and below it.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct PlanCutView {
    /// The plan's permanent ID.
    pub view: String,
    /// The level the plan is of.
    pub level: String,
    /// The height of its cut plane above the project's zero.
    pub cut: LengthView,
}

/// What the page asks of a document's stairs.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct StairsView {
    /// The rule set new stairs are laid out by.
    pub rules: String,
    /// That rule set's code and edition, in words.
    pub rules_name: String,
    /// Its categories of stairway.
    pub categories: Vec<CategoryView>,
    /// Every floor plan's cut plane, by the plan's name, so the page can draw a stair cut where
    /// the plan cuts it.
    pub plans: Vec<PlanCutView>,
    /// Every stair of the document, by ID.
    pub stairs: Vec<StairView>,
}

fn layout_view(layout: &construct_stairs::Layout) -> Result<LayoutView, String> {
    Ok(LayoutView {
        rise: LengthView::of(layout.rise),
        risers: layout.riser_count(),
        riser_height: LengthView::of(layout.tallest()),
        lowest_riser: LengthView::of(layout.lowest()),
        treads: layout.tread_count(),
        going: LengthView::of(layout.going),
        total_run: LengthView::of(layout.total_run().map_err(why)?),
        runs: layout
            .runs
            .iter()
            .map(|r| RunView {
                risers: r.risers,
                rise: LengthView::of(r.rise),
                length: LengthView::of(r.length),
            })
            .collect(),
        landings: layout
            .landings
            .iter()
            .map(|l| LandingView {
                length: LengthView::of(l.length),
                turn: l.turn.name(),
                height: LengthView::of(l.height),
            })
            .collect(),
    })
}

/// A document's stairs as the page needs them, as the module header says. A stair that cannot be
/// laid out as the document stands (a level moved too far, say) is listed with why.
///
/// # Errors
/// A store error.
pub async fn read_stairs<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<StairsView, StoreError> {
    let all = records::read_document(reader, project, document).await?;
    let mut stairs = Vec::new();
    for (_, record) in all.into_values().filter(|(_, record)| is_stair(record)) {
        let base =
            match construct_regen::levels::placed(reader, project, document, &record, At::Now)
                .await?
            {
                Ok(placed) => placed.elevation.map(LengthView::of),
                Err(_) => None,
            };
        let laid =
            construct_regen::stairs::layout_at(reader, project, document, &record, At::Now).await?;
        let (layout, refusal) = match laid.and_then(|(_, l)| layout_view(&l)) {
            Ok(view) => (Some(view), None),
            Err(why) => (None, Some(why)),
        };
        let (rules, category, direction, width) = match stair_of(&record.recipe) {
            Ok(stair) => (
                stair.rules().to_owned(),
                stair.category().to_owned(),
                stair.direction().name(),
                LengthView::of(stair.width()),
            ),
            Err(_) => (
                String::new(),
                String::new(),
                "",
                LengthView::of(Length::from_steps(0)),
            ),
        };
        stairs.push(StairView {
            id: record.id.to_string(),
            name: record.name.clone(),
            rules,
            category,
            direction,
            width,
            base,
            layout,
            refusal,
        });
    }
    let (rules_name, categories) = match RuleSet::builtin(NZ_D1_AS1) {
        Ok(set) => (
            format!("{}, {}", set.name, set.edition),
            set.categories
                .iter()
                .map(|c| CategoryView {
                    id: c.id.clone(),
                    name: c.name.clone(),
                })
                .collect(),
        ),
        Err(e) => (why(e), Vec::new()),
    };
    Ok(StairsView {
        rules: NZ_D1_AS1.to_owned(),
        rules_name,
        categories,
        plans: plan_cuts(reader, project, document).await?,
        stairs,
    })
}

/// Every floor plan's cut plane, by the plans' names; a plan whose range does not resolve against
/// the levels is left out (the views list says why).
async fn plan_cuts<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Vec<PlanCutView>, StoreError> {
    let Ok(world) = crate::cloud::views::read_world(reader, project, document).await? else {
        return Ok(Vec::new());
    };
    let mut plans = Vec::new();
    for view in world.views().ordered() {
        let Some(setup) = view.kind().plan() else {
            continue;
        };
        if let Ok(Some(cut)) = crate::cloud::views::plan_cut(&world, view.id()) {
            plans.push(PlanCutView {
                view: view.id().to_string(),
                level: setup.level.to_string(),
                cut: LengthView::of(cut.cut),
            });
        }
    }
    Ok(plans)
}
