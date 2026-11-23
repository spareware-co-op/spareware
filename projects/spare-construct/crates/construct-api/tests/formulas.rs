//! Formula parameters through the edit log, on the test store (CON-A2-F2-P2): a formula is
//! checked when it is defined, changed or bound; its value is worked out wherever it is read and
//! follows its inputs; a person can't set it; renaming a parameter rewrites the formulas that use
//! it; a result that can't be worked out is refused where a command causes it and shown where it
//! doesn't; undo and redo take each change as one step; and the log replayed gives the same bytes.

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_api::cloud::{Access, ProjectCommands};
use construct_categories::{CategoryTable, WALLS};
use construct_model::element::Element;
use construct_model::geometry::{Geometry, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_params::{Definition, Holder, Kind, ProjectError, ProjectParams};
use construct_regen::parameters::{self as records, ElementView, ParameterView};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use kit_core::Id;
use kit_ops::{Decider, Edit, Entry, Question};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Reader, Scope, Txn, read_row, to_row};
use kit_sync::{Clock, Orderers, Reply, read_log, verify_log};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";
/// How long a claim lasts here, as the server's setting would.
const CLAIM_MS: u64 = 30_000;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// Says yes. Access has tests of its own, in the server, against the real rules.
struct Yes;

impl Decider for Yes {
    fn decide(&self, _: &Question) -> Result<bool, String> {
        Ok(true)
    }
}

/// The orderer's time, set by the test, so a replay can run every edit at its recorded time.
struct Fixed(AtomicU64);

impl Clock for Fixed {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct Stack {
    store: Memory,
    orderers: Orderers<Memory, ProjectCommands<Memory, Yes>>,
    regen: Regenerator<Memory, MemoryFiles>,
    clock: Arc<Fixed>,
    next: AtomicU64,
}

impl Stack {
    /// A store with the rows the directory would have written, and no model yet.
    async fn bare() -> Self {
        let store = Memory::new();
        let files = MemoryFiles::default();
        let access = Arc::new(Access::new(store.clone(), Yes));
        let commands = ProjectCommands::new(store.clone(), access, CLAIM_MS, Boxes.edition());
        let clock = Arc::new(Fixed(AtomicU64::new(1_000_000)));
        let orderers = Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50);
        let regen = Regenerator::new(store.clone(), files, Arc::new(Boxes));
        let mut txn = store.begin().await.unwrap();
        let project = Project {
            id: i("p"),
            name: "P".to_owned(),
            hub: i("h"),
        };
        txn.put(Place::Project(i("p")).key(), to_row(&project).unwrap())
            .await
            .unwrap();
        let team = Team {
            id: i("t"),
            name: "T".to_owned(),
        };
        let place = Place::Team {
            project: i("p"),
            team: i("t"),
        };
        txn.put(place.key(), to_row(&team).unwrap()).await.unwrap();
        txn.commit().await.unwrap();
        Self {
            store,
            orderers,
            regen,
            clock,
            next: AtomicU64::new(0),
        }
    }

    async fn new() -> Self {
        let stack = Self::bare().await;
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        stack
    }

    async fn run(&self, edit: Edit) -> Result<u64, String> {
        match self
            .orderers
            .run(&Scope::Project(i("p")), edit)
            .await
            .unwrap()
        {
            Reply::Ack { seq, .. } => Ok(seq),
            Reply::Rejected { reason, .. } => Err(reason),
            Reply::Retry { .. } => Err("retry".to_owned()),
        }
    }

    async fn edit_as(&self, person: &str, cmd: &str, args: Value) -> Result<u64, String> {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        self.clock.0.fetch_add(10, Ordering::SeqCst);
        let edit = Edit {
            person: i(person),
            client: i(&format!("c{n}")),
            counter: 1,
            base: 0,
            target: i(DOCUMENT),
            cmd: cmd.to_owned(),
            args,
            author_ms: 0,
        };
        self.run(edit).await
    }

    async fn edit(&self, cmd: &str, args: Value) -> Result<u64, String> {
        self.edit_as("anna", cmd, args).await
    }

    /// Lets every claim run out.
    fn later(&self) {
        self.clock.0.fetch_add(CLAIM_MS + 1, Ordering::SeqCst);
    }

    async fn record(&self, id: &str) -> Element {
        let mut snapshot = self.store.snapshot().await.unwrap();
        records::read_element(&mut snapshot, &i("p"), &i(DOCUMENT), &i(id))
            .await
            .unwrap()
            .unwrap()
            .1
    }

    /// The document's parameter table, as stored.
    async fn table(&self) -> ProjectParams {
        let record = self.record("parameters").await;
        records::read_table(&record, &CategoryTable::new()).unwrap()
    }

    /// What the properties panel shows of an element: the server's view, read the server's way.
    async fn view(&self, id: &str) -> ElementView {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let (p, doc) = (i("p"), i(DOCUMENT));
        let (_, record) = records::read_element(&mut snapshot, &p, &doc, &i(id))
            .await
            .unwrap()
            .unwrap();
        let context = records::read_context(&mut snapshot, &p, &doc)
            .await
            .unwrap()
            .unwrap();
        let ty = match records::type_of(&record) {
            None => None,
            Some(ty) => Some(
                records::read_type(&mut snapshot, &p, &doc, &ty, &context)
                    .await
                    .unwrap()
                    .unwrap(),
            ),
        };
        context.view(&record, ty.as_ref()).unwrap()
    }

    async fn param(&self, element: &str, name: &str) -> ParameterView {
        self.view(element)
            .await
            .parameters
            .into_iter()
            .find(|p| p.name == name)
            .unwrap()
    }

    async fn built(&self, id: &str) -> BuiltElement {
        self.regen.catch_up(&i("p")).await.unwrap();
        let place = Place::Built {
            project: i("p"),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_row(&mut snapshot, &place.key())
            .await
            .unwrap()
            .unwrap()
    }

    /// How wide an element's shape is along x, once regenerated.
    async fn width(&self, id: &str) -> i64 {
        match self.built(id).await.result {
            BuiltResult::Shape { bounds, .. } => bounds.max[0] - bounds.min[0],
            failed @ BuiltResult::Failed { .. } => panic!("{id}: {failed:?}"),
        }
    }

    /// Every row under a table of the project, as bytes.
    async fn rows(&self, table: &str) -> BTreeMap<String, Vec<u8>> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let prefix = Scope::Project(i("p")).table_prefix(table, &[]);
        snapshot
            .scan(&prefix)
            .await
            .unwrap()
            .into_iter()
            .map(|(key, value)| (key.as_str().to_owned(), value))
            .collect()
    }

    async fn derived(&self) -> BTreeMap<String, Vec<u8>> {
        let mut rows = BTreeMap::new();
        for table in ["body", "display", "built", "area", "kind", "reads"] {
            rows.extend(self.rows(table).await);
        }
        rows
    }

    async fn log(&self) -> Vec<Entry> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let head: u64 = read_row(&mut snapshot, &Scope::Project(i("p")).head())
            .await
            .unwrap()
            .unwrap_or(0);
        read_log(&mut snapshot, &Scope::Project(i("p")), 0, head)
            .await
            .unwrap()
    }
}

fn recipe(op: &str, args: Value) -> Value {
    serde_json::to_value(Recipe::construct(op, serde_json::from_value(args).unwrap())).unwrap()
}

/// A door as wide as its leaf, which its type's formula gives.
fn door(id: &str, ty: &str) -> Value {
    let args = json!({ "width": { "param": "P-leaf" }, "thickness": { "param": "P-thick" },
                       "height": 2100 });
    json!({ "element": id, "kind": "door", "level": 1, "type": ty,
            "placement": { "x": 0, "y": 0, "z": 0 }, "recipe": recipe("door.single", args) })
}

fn def(id: &str, name: &str, kind: &str, holder: &str) -> Value {
    json!({ "definition": { "id": id, "name": name, "kind": kind, "holder": holder,
                            "choices": [] } })
}

fn formula(id: &str, name: &str, kind: &str, holder: &str, formula: &str) -> Value {
    let mut args = def(id, name, kind, holder);
    args["definition"]["formula"] = json!(formula);
    args
}

fn bind(id: &str, default: &Value) -> Value {
    json!({ "parameter": id, "categories": [300], "default": default })
}

/// Doors: a width on the type (900 unless set) and a frame (30), the leaf worked out from them on
/// the type, a thickness (40) and a number of panels (2) on each door, and each panel's gap worked
/// out on each door from the leaf. Type `T-820` of width 820 and door `D-1` of it.
async fn doors(stack: &Stack) {
    for (args, default) in [
        (def("P-width", "Width", "length", "type"), json!("900")),
        (def("P-frame", "Frame", "length", "type"), json!("30")),
        (
            formula("P-leaf", "Leaf", "length", "type", "Width-2*Frame"),
            Value::Null,
        ),
        (
            def("P-thick", "Thickness", "length", "instance"),
            json!("40"),
        ),
        (def("P-panels", "Panels", "whole", "instance"), json!("2")),
        (
            formula("P-gap", "Gap", "length", "instance", "Leaf / Panels"),
            Value::Null,
        ),
    ] {
        let id = args["definition"]["id"].as_str().unwrap().to_owned();
        stack.edit("parameter.define", args).await.unwrap();
        stack
            .edit("parameter.bind", bind(&id, &default))
            .await
            .unwrap();
    }
    stack
        .edit(
            "type.create",
            json!({ "type": "T-820", "name": "Single 820", "category": 300 }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "type.set",
            json!({ "type": "T-820", "parameter": "P-width", "value": "820" }),
        )
        .await
        .unwrap();
    stack
        .edit("element.create", door("D-1", "T-820"))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_formula_is_checked_with_construct_formulas_when_it_is_defined_changed_or_bound() {
    let stack = Stack::new().await;
    doors(&stack).await;

    // It is stored in its one spelling.
    let table = stack.table().await;
    assert_eq!(
        table.definition(&i("P-leaf")).unwrap().formula(),
        Some("Width - 2 * Frame")
    );

    // A name it uses that is not a parameter, and kinds that do not fit, in the library's words.
    assert_eq!(
        stack
            .edit(
                "parameter.define",
                formula("P-x", "X", "length", "type", "Nope * 2"),
            )
            .await,
        Err("There is no parameter called Nope.".to_owned())
    );
    let width = table.definition(&i("P-width")).unwrap().clone();
    let frame = table.definition(&i("P-frame")).unwrap().clone();
    let area = construct_formulas::define(
        Definition::new(i("P-x"), "X", Kind::Length, Holder::Type).unwrap(),
        "Width * Frame",
    )
    .unwrap();
    let units = construct_formulas::check(&[width, frame, area])[&i("P-x")]
        .message
        .clone();
    assert_eq!(
        stack
            .edit(
                "parameter.define",
                formula("P-x", "X", "length", "type", "Width * Frame"),
            )
            .await,
        Err(units)
    );

    // A type's formula uses only the type's parameters.
    let refusal = stack
        .edit(
            "parameter.define",
            formula("P-x", "X", "length", "type", "Thickness * 2"),
        )
        .await
        .unwrap_err();
    assert!(
        refusal.starts_with("Thickness is on each instance"),
        "{refusal}"
    );

    // A loop, named, when a change would close one.
    stack
        .edit(
            "parameter.define",
            def("P-a", "Alpha", "length", "instance"),
        )
        .await
        .unwrap();
    stack
        .edit(
            "parameter.define",
            formula("P-b", "Beta", "length", "instance", "Alpha + 1 mm"),
        )
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit(
                "parameter.change",
                json!({ "parameter": "P-a", "formula": "Beta - 1 mm" }),
            )
            .await,
        Err("Alpha, Beta are defined from each other.".to_owned())
    );

    // On an element a formula sees only its category's parameters: bound to walls, Beta would
    // use an Alpha no wall has.
    let walls = CategoryTable::new().get(WALLS).unwrap().name;
    assert_eq!(
        stack
            .edit(
                "parameter.bind",
                json!({ "parameter": "P-b", "categories": [WALLS.0] }),
            )
            .await,
        Err(format!("On {walls}: There is no parameter called Alpha."))
    );

    // The same parameter again changes nothing; another formula is parameter.change's.
    stack
        .edit(
            "parameter.define",
            formula("P-leaf", "Leaf", "length", "type", "Width - 2 * Frame"),
        )
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit(
                "parameter.define",
                formula("P-leaf", "Leaf", "length", "type", "Width - Frame"),
            )
            .await,
        Err(ProjectError::Conflict {
            id: i("P-leaf"),
            name: "Leaf".to_owned(),
            field: "formula"
        }
        .to_string())
    );

    // A kind no formula gives can't carry one, in construct-params' words.
    let text = Definition::new(i("P-x"), "X", Kind::Text, Holder::Type).unwrap();
    assert_eq!(
        stack
            .edit(
                "parameter.define",
                formula("P-x", "X", "text", "type", "Width"),
            )
            .await,
        Err(construct_formulas::define(text, "Width")
            .unwrap_err()
            .message)
    );
    // Nothing refused reached the table.
    assert!(stack.table().await.definition(&i("P-x")).is_none());
    assert_eq!(
        stack.table().await.definition(&i("P-a")).unwrap().formula(),
        None
    );
}

#[tokio::test]
async fn a_formulas_value_follows_its_inputs_everywhere_it_is_read() {
    let stack = Stack::new().await;
    doors(&stack).await;
    assert_eq!(stack.width("D-1").await, 760, "820 - 2 * 30");

    // The properties panel shows the value the formula gives, and the formula.
    let leaf = stack.param("D-1", "Leaf").await;
    assert_eq!(leaf.from, "formula");
    assert_eq!(leaf.formula.as_deref(), Some("Width - 2 * Frame"));
    assert_eq!(leaf.shown.as_deref(), Some("760 mm"));
    assert_eq!(leaf.reason, None);
    assert_eq!(
        stack.param("D-1", "Gap").await.shown.as_deref(),
        Some("380 mm")
    );
    let width = stack.param("D-1", "Width").await;
    assert_eq!((width.from, width.formula), ("type", None));

    // A type's value, a default and an element's own value each reach every formula that
    // depends on them, in order: the shape through the regenerator's reads table.
    stack
        .edit(
            "type.set",
            json!({ "type": "T-820", "parameter": "P-width", "value": "1 m" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.width("D-1").await, 940);
    stack
        .edit("parameter.bind", bind("P-frame", &json!("40")))
        .await
        .unwrap();
    assert_eq!(stack.width("D-1").await, 920);
    stack
        .edit(
            "element.param",
            json!({ "element": "D-1", "parameter": "P-panels", "value": "4" }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack.param("D-1", "Gap").await.shown.as_deref(),
        Some("230 mm")
    );

    // A second type reads the defaults; changing the formula itself reaches both.
    stack
        .edit(
            "type.create",
            json!({ "type": "T-std", "name": "Standard", "category": 300 }),
        )
        .await
        .unwrap();
    stack
        .edit("element.create", door("D-2", "T-std"))
        .await
        .unwrap();
    assert_eq!(stack.width("D-2").await, 820, "900 - 2 * 40");
    stack
        .edit(
            "parameter.change",
            json!({ "parameter": "P-leaf", "formula": "Width - Frame" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.width("D-1").await, 960);
    assert_eq!(stack.width("D-2").await, 860);
    assert_eq!(
        stack.param("D-2", "Gap").await.shown.as_deref(),
        Some("430 mm"),
        "860 / 2 panels"
    );

    // The types dialog shows a type parameter's formula and the value it gives the type.
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let types =
        construct_regen::catalog::read_types_view(&mut snapshot, &i("p"), &i(DOCUMENT), None)
            .await
            .unwrap()
            .unwrap();
    let single = types.types.iter().find(|t| t.id == "T-820").unwrap();
    let leaf = single.parameters.iter().find(|p| p.name == "Leaf").unwrap();
    assert_eq!(
        (leaf.from, leaf.formula.as_deref(), leaf.shown.as_deref()),
        ("formula", Some("Width - Frame"), Some("960 mm"))
    );
    let listed = construct_regen::catalog::read_table_view(&mut snapshot, &i("p"), &i(DOCUMENT))
        .await
        .unwrap()
        .unwrap();
    let gap = listed.parameters.iter().find(|p| p.name == "Gap").unwrap();
    assert_eq!(gap.formula.as_deref(), Some("Leaf / Panels"));

    // Nothing worked out is stored: every derived row comes back the same from the log.
    let before = stack.derived().await;
    stack.regen.rebuild(&i("p")).await.unwrap();
    assert_eq!(stack.derived().await, before);
    assert_eq!(stack.regen.verify(&i("p")).await.unwrap(), 2);
    assert!(
        !stack.record("T-820").await.props.contains_key("P-leaf"),
        "a formula's value is never a record's"
    );
}

#[tokio::test]
async fn a_formula_parameter_takes_no_value_until_its_formula_is_taken_away() {
    let stack = Stack::new().await;
    doors(&stack).await;
    stack
        .edit(
            "type.create",
            json!({ "type": "T-std", "name": "Standard", "category": 300 }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit(
                "element.param",
                json!({ "element": "D-1", "parameter": "P-gap", "value": "100" }),
            )
            .await,
        Err(
            "Gap is given by its formula, Leaf / Panels; take the formula away to set it."
                .to_owned()
        )
    );
    assert_eq!(
        stack
            .edit(
                "type.set",
                json!({ "type": "T-820", "parameter": "P-leaf", "value": "700" }),
            )
            .await,
        Err(
            "Leaf is given by its formula, Width - 2 * Frame; take the formula away to set it."
                .to_owned()
        )
    );
    assert_eq!(
        stack
            .edit("parameter.bind", bind("P-leaf", &json!("700")))
            .await,
        Err("Leaf is given by its formula; it takes no default.".to_owned())
    );

    // Taking it away writes every holder's last value as its own, so it is refused while
    // another person works on one of them, as any edit is.
    stack.later();
    stack
        .edit_as(
            "ben",
            "type.rename",
            json!({ "type": "T-std", "name": "Plain" }),
        )
        .await
        .unwrap();
    let away = json!({ "parameter": "P-leaf", "formula": null });
    assert_eq!(
        stack.edit("parameter.change", away.clone()).await,
        Err("ben is working on this.".to_owned())
    );
    stack.later();
    stack.edit("parameter.change", away).await.unwrap();
    assert_eq!(
        stack
            .table()
            .await
            .definition(&i("P-leaf"))
            .unwrap()
            .formula(),
        None
    );
    assert_eq!(
        stack.record("T-820").await.props["P-leaf"],
        format!("length:q0:{}", 760 * 6400),
        "820 - 2 * 30, kept"
    );
    assert_eq!(
        stack.record("T-std").await.props["P-leaf"],
        format!("length:q0:{}", 840 * 6400),
        "900 - 2 * 30, kept"
    );
    let leaf = stack.param("D-1", "Leaf").await;
    assert_eq!((leaf.from, leaf.formula), ("type", None));
    assert_eq!(stack.width("D-1").await, 760);

    // Settable again, and what depends on it follows.
    stack
        .edit(
            "type.set",
            json!({ "type": "T-820", "parameter": "P-leaf", "value": "700" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.width("D-1").await, 700);
    assert_eq!(
        stack.param("D-1", "Gap").await.shown.as_deref(),
        Some("350 mm")
    );

    // An instance formula taken away leaves each element its own last value.
    stack
        .edit(
            "parameter.change",
            json!({ "parameter": "P-gap", "formula": null }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack.record("D-1").await.props["P-gap"],
        format!("length:q0:{}", 350 * 6400)
    );
}

#[tokio::test]
async fn renaming_a_parameter_rewrites_the_formulas_that_use_it_in_one_step() {
    let stack = Stack::new().await;
    doors(&stack).await;
    let seq = stack
        .edit(
            "parameter.change",
            json!({ "parameter": "P-frame", "name": "Jamb" }),
        )
        .await
        .unwrap();
    let table = stack.table().await;
    assert_eq!(table.definition(&i("P-frame")).unwrap().name(), "Jamb");
    assert_eq!(
        table.definition(&i("P-leaf")).unwrap().formula(),
        Some("Width - 2 * Jamb")
    );
    assert_eq!(stack.width("D-1").await, 760, "the same value");
    let entry = stack
        .log()
        .await
        .into_iter()
        .find(|e| e.seq == seq)
        .unwrap();
    assert_eq!(entry.changes.len(), 1, "the table alone, in one entry");

    // One undo puts the name and the formula back; one redo again.
    stack.edit("edit.undo", json!({})).await.unwrap();
    let table = stack.table().await;
    assert_eq!(table.definition(&i("P-frame")).unwrap().name(), "Frame");
    assert_eq!(
        table.definition(&i("P-leaf")).unwrap().formula(),
        Some("Width - 2 * Frame")
    );
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(
        stack
            .table()
            .await
            .definition(&i("P-leaf"))
            .unwrap()
            .formula(),
        Some("Width - 2 * Jamb")
    );

    // A name a formula can't use is refused, naming the formulas that use the parameter; one no
    // formula uses may take any name.
    assert_eq!(
        stack
            .edit(
                "parameter.change",
                json!({ "parameter": "P-frame", "name": "Door frame" }),
            )
            .await,
        Err(
            "Door frame can't be used in a formula: a name there is letters, digits and _. The \
             formulas of Leaf use Jamb."
                .to_owned()
        )
    );
    stack
        .edit(
            "parameter.change",
            json!({ "parameter": "P-thick", "name": "Leaf thickness" }),
        )
        .await
        .unwrap();
    // A name is still the project's only one of its spelling.
    assert_eq!(
        stack
            .edit(
                "parameter.change",
                json!({ "parameter": "P-frame", "name": "width" }),
            )
            .await,
        Err(ProjectError::DuplicateName("width".to_owned()).to_string())
    );
}

#[tokio::test]
async fn a_result_that_cannot_be_worked_out_is_refused_where_a_command_causes_it_and_shown_elsewhere()
 {
    let stack = Stack::new().await;
    doors(&stack).await;
    stack
        .edit(
            "parameter.define",
            formula("P-half", "HalfGap", "length", "instance", "Gap / 2"),
        )
        .await
        .unwrap();
    stack
        .edit("parameter.bind", bind("P-half", &Value::Null))
        .await
        .unwrap();

    // The command that would make a formula fail is the one refused, with the library's reason.
    let none = json!({ "element": "D-1", "parameter": "P-panels", "value": "0" });
    assert_eq!(
        stack.edit("element.param", none).await,
        Err("Gap can't be worked out for D-1: That divides by zero.".to_owned())
    );
    assert_eq!(
        stack.param("D-1", "Gap").await.shown.as_deref(),
        Some("380 mm")
    );

    // A default no element reads yet can't fail anything.
    stack
        .edit(
            "element.param",
            json!({ "element": "D-1", "parameter": "P-panels", "value": "4" }),
        )
        .await
        .unwrap();
    stack
        .edit("parameter.bind", bind("P-panels", &json!("0")))
        .await
        .unwrap();

    // A new door reads it: creating an element is not refused for it; the failure is shown.
    let mut args = door("D-3", "T-820");
    args["recipe"]["args"]["width"] = json!({ "param": "P-gap" });
    stack.edit("element.create", args).await.unwrap();
    let gap = stack.param("D-3", "Gap").await;
    assert_eq!(
        (gap.from, gap.value, gap.reason.as_deref()),
        ("formula", None, Some("That divides by zero."))
    );
    assert_eq!(
        stack.param("D-3", "HalfGap").await.reason.as_deref(),
        Some("Gap fails: That divides by zero."),
        "it waits on the one that fails"
    );
    let BuiltResult::Failed { code, message } = stack.built("D-3").await.result else {
        panic!("its shape reads the gap");
    };
    assert_eq!(
        (code.as_str(), message.as_str()),
        ("BadParameter", "That divides by zero.")
    );
    // The document still works: another edit of that door stands, and the fix builds it.
    stack
        .edit(
            "element.param",
            json!({ "element": "D-3", "parameter": "P-thick", "value": "50" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "element.param",
            json!({ "element": "D-3", "parameter": "P-panels", "value": "5" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.width("D-3").await, 152, "760 / 5");

    // A type value, a binding and a formula change are refused the same way.
    stack
        .edit(
            "parameter.change",
            json!({ "parameter": "P-gap", "formula": "Leaf / (Panels - 4)" }),
        )
        .await
        .map(|_| ())
        .unwrap_err();
    for (args, default) in [
        (def("P-mull", "Mullions", "whole", "type"), json!("1")),
        (
            formula("P-pane", "Pane", "length", "type", "Leaf / Mullions"),
            Value::Null,
        ),
    ] {
        let id = args["definition"]["id"].as_str().unwrap().to_owned();
        stack.edit("parameter.define", args).await.unwrap();
        stack
            .edit("parameter.bind", bind(&id, &default))
            .await
            .unwrap();
    }
    assert_eq!(
        stack
            .edit(
                "type.set",
                json!({ "type": "T-820", "parameter": "P-mull", "value": "0" }),
            )
            .await,
        Err("Pane can't be worked out for Single 820: That divides by zero.".to_owned())
    );
    stack
        .edit(
            "parameter.define",
            formula(
                "P-rail",
                "Rail",
                "length",
                "instance",
                "Leaf / (Panels - Panels)",
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        stack
            .edit("parameter.bind", bind("P-rail", &Value::Null))
            .await,
        Err("Rail can't be worked out for D-1: That divides by zero.".to_owned())
    );
}

#[tokio::test]
async fn undo_and_redo_take_each_formula_change_as_one_step() {
    let stack = Stack::new().await;
    doors(&stack).await;
    stack
        .edit(
            "type.create",
            json!({ "type": "T-std", "name": "Standard", "category": 300 }),
        )
        .await
        .unwrap();
    for (cmd, args) in [
        (
            "parameter.define",
            formula("P-x", "Extra", "length", "instance", "Gap * 2"),
        ),
        (
            "parameter.change",
            json!({ "parameter": "P-leaf", "formula": "Width - Frame" }),
        ),
        (
            "parameter.change",
            json!({ "parameter": "P-leaf", "formula": null }),
        ),
    ] {
        let before = stack.rows("piece").await;
        let width = stack.width("D-1").await;
        stack
            .edit(cmd, args)
            .await
            .unwrap_or_else(|why| panic!("{cmd}: {why}"));
        let after = stack.rows("piece").await;
        let changed = stack.width("D-1").await;
        stack.edit("edit.undo", json!({})).await.unwrap();
        assert_eq!(stack.rows("piece").await, before, "undo of {cmd}");
        assert_eq!(stack.width("D-1").await, width, "undo of {cmd}");
        stack.edit("edit.redo", json!({})).await.unwrap();
        assert_eq!(stack.rows("piece").await, after, "redo of {cmd}");
        assert_eq!(stack.width("D-1").await, changed, "redo of {cmd}");
    }
    // The formula taken away wrote the table and both types in that one step.
    let log = stack.log().await;
    let away = log
        .iter()
        .rev()
        .find(|e| e.cmd == "parameter.change" && e.failed.is_none())
        .unwrap();
    assert_eq!(away.changes.len(), 3);
    let mut snapshot = stack.store.snapshot().await.unwrap();
    verify_log(&mut snapshot, &Scope::Project(i("p")))
        .await
        .unwrap();
}

/// The log is the truth: every edit of it run again, in order, at its recorded time, on an empty
/// store gives the same records and pointers, and regenerating gives the same derived rows, byte
/// for byte.
#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    doors(&stack).await;
    for (cmd, args) in [
        (
            "type.set",
            json!({ "type": "T-820", "parameter": "P-width", "value": "1 m" }),
        ),
        (
            "element.param",
            json!({ "element": "D-1", "parameter": "P-panels", "value": "0" }),
        ),
        (
            "parameter.change",
            json!({ "parameter": "P-frame", "name": "Jamb" }),
        ),
        (
            "parameter.change",
            json!({ "parameter": "P-leaf", "formula": "Width - 3 * Jamb" }),
        ),
        ("edit.undo", json!({})),
        (
            "parameter.change",
            json!({ "parameter": "P-gap", "formula": null }),
        ),
    ] {
        let _ = stack.edit(cmd, args).await;
    }
    stack.regen.catch_up(&i("p")).await.unwrap();
    let entries = stack.log().await;
    assert!(entries.iter().any(|entry| entry.failed.is_some()));

    let again = Stack::bare().await;
    for entry in &entries {
        again.clock.0.store(entry.orderer_ms, Ordering::SeqCst);
        let edit = Edit {
            person: entry.author.clone(),
            client: entry.client.clone(),
            counter: entry.counter,
            base: entry.base,
            target: entry.target.clone(),
            cmd: entry.cmd.clone(),
            args: entry.args.clone(),
            author_ms: entry.author_ms,
        };
        let _ = again.run(edit).await;
    }
    for table in ["record", "piece"] {
        assert_eq!(again.rows(table).await, stack.rows(table).await, "{table}");
    }
    assert_eq!(again.log().await, entries);
    again.regen.catch_up(&i("p")).await.unwrap();
    assert_eq!(again.derived().await, stack.derived().await);
}
