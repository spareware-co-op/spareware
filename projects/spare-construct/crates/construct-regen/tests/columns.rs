//! A column through the log and the regenerator, on the test store: built from its type's section
//! and its levels, standing at a point or where two grid lines cross, and built again when its
//! type, a level or a grid it reads changes, its record never written; kept and reported when it
//! cannot be built (a top below its base, two grids that do not cross, a grid that is gone) and
//! built again when that is put right. The records go in as they are, through commands of this
//! file's own that only store or delete them: the commands that place columns are construct-api's.
//!
//! The builder is the stand-in, which builds a column as the box around its ring; the kernel's own
//! column is the kernel suite's (`tests/kernel_suite`).

#![cfg(feature = "server")]
// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::CategoryTable;
use construct_columns::{
    Column, ColumnSpec, Position, Section, Top, column_record, new_column_type,
};
use construct_grids::{Grid, Point, Shape};
use construct_levels::{Level, Tie};
use construct_materials::MaterialId;
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Model, Reads};
use construct_params::{Angle, Length, Units, Value};
use construct_regen::columns;
use construct_regen::parameters::{At, ParameterView};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use kit_core::Id;
use kit_ops::{Change, Edit};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Scope, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Clock, Commands, Context as Moment, Orderers, Outcome, Reply};
use serde_json::{Value as Json, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const PROJECT: &str = "p";
const DOCUMENT: &str = "doc";

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(v: i64) -> Length {
    Length::checked_from_mm(v).unwrap()
}

/// The only commands here: `put` stores a record as it is (`{"record": ...}`), `delete` takes
/// one away (`{"id": ...}`), and `model` makes the document on the stand-in's edition.
struct Puts;

impl Commands<Memory> for Puts {
    fn version(&self) -> u32 {
        1
    }

    fn permit(
        &self,
        _: &Scope,
        _: &Edit,
    ) -> impl Future<Output = Result<Result<(), String>, StoreError>> + Send {
        std::future::ready(Ok(Ok(())))
    }

    async fn apply(
        &self,
        txn: &mut <Memory as Kv>::Txn,
        _: &Moment<'_>,
        edit: &Edit,
    ) -> Result<Outcome, StoreError> {
        if edit.cmd == "model" {
            let place = Place::Model {
                project: i(PROJECT),
                model: edit.target.clone(),
            };
            let model = Model {
                id: edit.target.clone(),
                name: "Doc".to_owned(),
                team: i("t"),
                package: "Doc package".to_owned(),
                edition: Boxes.edition(),
            };
            txn.put(place.key(), to_row(&model)?).await?;
            return Ok(Ok(Applied::default()));
        }
        if edit.cmd == "delete" {
            let piece = i(edit.args["id"].as_str().unwrap());
            let pointer = kit_store::Place::Piece {
                project: i(PROJECT),
                document: edit.target.clone(),
                piece: piece.clone(),
            };
            let before: Option<String> = read_row(txn, &pointer.key()).await?;
            txn.delete(pointer.key()).await?;
            return Ok(Ok(Applied {
                changes: vec![Change {
                    piece,
                    before: before.map(|h| i(&h)),
                    after: None,
                }],
                ..Applied::default()
            }));
        }
        let record: Element = serde_json::from_value(edit.args["record"].clone()).unwrap();
        let bytes = to_row(&record)?;
        let hash = i(&kit_ops::sha256_hex(&bytes));
        let place = kit_store::Place::Record {
            project: i(PROJECT),
            hash: hash.clone(),
        };
        txn.put(place.key(), bytes).await?;
        let pointer = kit_store::Place::Piece {
            project: i(PROJECT),
            document: edit.target.clone(),
            piece: record.id.clone(),
        };
        let before: Option<String> = read_row(txn, &pointer.key()).await?;
        txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
        Ok(Ok(Applied {
            changes: vec![Change {
                piece: record.id,
                before: before.map(|h| i(&h)),
                after: Some(hash),
            }],
            ..Applied::default()
        }))
    }
}

/// The orderer's time, fixed.
struct Fixed;

impl Clock for Fixed {
    fn now_ms(&self) -> u64 {
        1_000_000
    }
}

struct Stack {
    store: Memory,
    orderers: Orderers<Memory, Puts>,
    regen: Regenerator<Memory, MemoryFiles>,
    next: AtomicU64,
}

impl Stack {
    /// A document with levels L-0 at 0 and L-1 at 3000 mm, the grid lines g1, g2 (vertical, at
    /// x = 0 and 6000 mm) and ga, gb (horizontal, at y = 0 and 5000 mm), and the column type
    /// `t400`, 400 by 400 mm of concrete.
    async fn new() -> Self {
        let store = Memory::new();
        let orderers = Orderers::new(store.clone(), Arc::new(Puts), Arc::new(Fixed), 50);
        let regen = Regenerator::new(store.clone(), MemoryFiles::default(), Arc::new(Boxes));
        let stack = Self {
            store,
            orderers,
            regen,
            next: AtomicU64::new(0),
        };
        stack.run("model", json!({})).await;
        for (id, elevation) in [("L-0", 0), ("L-1", 3000)] {
            stack
                .put(&level(id, elevation).to_record(&i(DOCUMENT)))
                .await;
        }
        for (id, x) in [("g1", 0), ("g2", 6000)] {
            stack.put(&grid(id, line(x, -1000, x, 6000))).await;
        }
        for (id, y) in [("ga", 0), ("gb", 5000)] {
            stack.put(&grid(id, line(-1000, y, 8000, y))).await;
        }
        stack.put(&column_type("t400", 400, 400)).await;
        stack
    }

    async fn run(&self, cmd: &str, args: Json) {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        let edit = Edit {
            person: i("anna"),
            client: i(&format!("c{n}")),
            counter: 1,
            base: 0,
            target: i(DOCUMENT),
            cmd: cmd.to_owned(),
            args,
            author_ms: 0,
        };
        let reply = self
            .orderers
            .run(&Scope::Project(i(PROJECT)), edit)
            .await
            .unwrap();
        assert!(matches!(reply, Reply::Ack { .. }), "{reply:?}");
    }

    async fn put(&self, record: &Element) {
        self.run("put", json!({ "record": record })).await;
    }

    async fn delete(&self, id: &str) {
        self.run("delete", json!({ "id": id })).await;
    }

    async fn built(&self, id: &str) -> BuiltElement {
        self.regen.catch_up(&i(PROJECT)).await.unwrap();
        let place = Place::Built {
            project: i(PROJECT),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_row(&mut snapshot, &place.key())
            .await
            .unwrap()
            .unwrap()
    }

    /// An element's built bounds, or its failure's code and words.
    async fn bounds(&self, id: &str) -> Result<([i64; 3], [i64; 3]), String> {
        match self.built(id).await.result {
            BuiltResult::Shape { bounds, .. } => Ok((bounds.min, bounds.max)),
            BuiltResult::Failed { code, message } => Err(format!("{code}: {message}")),
        }
    }

    /// What an element's `reads` row lists.
    async fn reads(&self, id: &str) -> Vec<String> {
        self.regen.catch_up(&i(PROJECT)).await.unwrap();
        let place = Place::Reads {
            project: i(PROJECT),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        let row: Option<Reads> = read_row(&mut snapshot, &place.key()).await.unwrap();
        row.unwrap_or_default().records.into_iter().collect()
    }

    /// The read-only parameters a page shows of a column, as the document stands now.
    async fn rows(
        &self,
        record: &Element,
        ty: &construct_types::ElementType,
    ) -> Vec<ParameterView> {
        self.regen.catch_up(&i(PROJECT)).await.unwrap();
        let mut snapshot = self.store.snapshot().await.unwrap();
        columns::view_rows(
            &mut snapshot,
            &i(PROJECT),
            &i(DOCUMENT),
            record,
            Some(ty),
            &Units::default(),
            At::Now,
        )
        .await
        .unwrap()
    }
}

fn level(id: &str, elevation_mm: i64) -> Level {
    Level::new(i(id), id, mm(elevation_mm), true).unwrap()
}

fn line(x0: i64, y0: i64, x1: i64, y1: i64) -> Shape {
    Shape::line(Point::new(mm(x0), mm(y0)), Point::new(mm(x1), mm(y1))).unwrap()
}

fn grid(id: &str, shape: Shape) -> Element {
    Grid::new(i(id), id, shape).unwrap().to_record(&i(DOCUMENT))
}

fn spec(width: i64, depth: i64) -> ColumnSpec {
    ColumnSpec {
        section: Section::rectangular(mm(width), mm(depth)).unwrap(),
        material: MaterialId::parse("concrete-regular").unwrap(),
    }
}

fn column_type_of(id: &str, width: i64, depth: i64) -> construct_types::ElementType {
    new_column_type(
        i(id),
        &format!("Concrete {width} x {depth}"),
        &spec(width, depth),
        &CategoryTable::new(),
    )
    .unwrap()
}

fn column_type(id: &str, width: i64, depth: i64) -> Element {
    column_type_of(id, width, depth).to_record(&i(DOCUMENT))
}

fn tie(level: &str, offset_mm: i64) -> Tie {
    Tie {
        level: i(level),
        offset: mm(offset_mm),
    }
}

fn on_grids(first: &str, second: &str) -> Position {
    Position::Grids {
        first: i(first),
        second: i(second),
        cross: 0,
    }
}

/// A column of type `t400` from L-0 to L-1.
fn column(id: &str, position: Position) -> Element {
    column_record(
        &i(id),
        &i(DOCUMENT),
        id,
        &i("t400"),
        &tie("L-0", 0),
        &Column {
            position,
            turn: Angle::from_units(0),
            top: Top::Level(tie("L-1", 0)),
        },
    )
}

/// A column of 400 by 400 mm at (1000, 2000) mm between the two levels is the box 800 to 1200 mm
/// by 1800 to 2200 mm, 3000 mm tall, its base at the level's elevation (0), with its `reads` row
/// naming its level and its type; its record is never written by the build.
#[tokio::test]
async fn a_column_at_a_point_is_built_from_its_type_and_levels() {
    let stack = Stack::new().await;
    let at = Position::Point {
        x: mm(1000),
        y: mm(2000),
    };
    stack.put(&column("c1", at)).await;
    assert_eq!(
        stack.bounds("c1").await.unwrap(),
        ([800, 1800, 0], [1200, 2200, 3000])
    );
    let built = stack.built("c1").await;
    assert_eq!(built.level, Some(i("L-0")));
    assert_eq!(built.elevation, Some(Value::Length(mm(0)).stored()));
    let reads = stack.reads("c1").await;
    assert!(reads.contains(&"L-0".to_owned()), "{reads:?}");
    assert!(reads.contains(&"L-1".to_owned()), "{reads:?}");
    assert!(reads.contains(&"t400".to_owned()), "{reads:?}");
}

/// A moved level moves its columns: L-1 up 200 mm makes the column 3200 mm tall; L-0 up 500 mm
/// moves its base (the built row's elevation) and makes it 2700 mm tall; no column record changes.
#[tokio::test]
async fn a_moved_level_moves_its_columns() {
    let stack = Stack::new().await;
    let record = column("c1", Position::Point { x: mm(0), y: mm(0) });
    stack.put(&record).await;
    assert_eq!(stack.bounds("c1").await.unwrap().1[2], 3000);
    stack.put(&level("L-1", 3200).to_record(&i(DOCUMENT))).await;
    assert_eq!(stack.bounds("c1").await.unwrap().1[2], 3200);
    stack.put(&level("L-0", 500).to_record(&i(DOCUMENT))).await;
    assert_eq!(stack.bounds("c1").await.unwrap().1[2], 2700);
    assert_eq!(
        stack.built("c1").await.elevation,
        Some(Value::Length(mm(500)).stored())
    );
}

/// A top below its base is kept and reported by name, and the column is built when it is put right:
/// L-1 moved to 0 leaves no height, below 0 is `TopBelowBase` saying how far.
#[tokio::test]
async fn a_top_below_its_base_fails_by_name_and_recovers() {
    let stack = Stack::new().await;
    stack
        .put(&column("c1", Position::Point { x: mm(0), y: mm(0) }))
        .await;
    stack.put(&level("L-1", 0).to_record(&i(DOCUMENT))).await;
    let none = stack.bounds("c1").await.unwrap_err();
    assert!(none.starts_with("NoHeight:"), "{none}");
    stack.put(&level("L-1", -400).to_record(&i(DOCUMENT))).await;
    assert_eq!(
        stack.bounds("c1").await.unwrap_err(),
        "TopBelowBase: the column's top is 400 mm below its base: a column stands up from its base"
    );
    stack.put(&level("L-1", 2400).to_record(&i(DOCUMENT))).await;
    assert_eq!(stack.bounds("c1").await.unwrap().1[2], 2400);
}

/// Columns on the crossings of g1, g2 with ga, gb stand on them, and a grid moved 500 mm moves the
/// two columns on it and no other; a column's `reads` row lists its two grids.
#[tokio::test]
async fn columns_on_grid_intersections_follow_a_moved_grid() {
    let stack = Stack::new().await;
    for (id, v, h) in [
        ("c-1a", "g1", "ga"),
        ("c-1b", "g1", "gb"),
        ("c-2a", "g2", "ga"),
        ("c-2b", "g2", "gb"),
    ] {
        stack.put(&column(id, on_grids(v, h))).await;
    }
    let low = |bounds: ([i64; 3], [i64; 3])| [bounds.0[0] + 200, bounds.0[1] + 200];
    assert_eq!(low(stack.bounds("c-1a").await.unwrap()), [0, 0]);
    assert_eq!(low(stack.bounds("c-1b").await.unwrap()), [0, 5000]);
    assert_eq!(low(stack.bounds("c-2a").await.unwrap()), [6000, 0]);
    assert_eq!(low(stack.bounds("c-2b").await.unwrap()), [6000, 5000]);
    let reads = stack.reads("c-2b").await;
    assert!(reads.contains(&"g2".to_owned()) && reads.contains(&"gb".to_owned()));

    stack.put(&grid("g2", line(6500, -1000, 6500, 6000))).await;
    assert_eq!(low(stack.bounds("c-2a").await.unwrap()), [6500, 0]);
    assert_eq!(low(stack.bounds("c-2b").await.unwrap()), [6500, 5000]);
    assert_eq!(low(stack.bounds("c-1a").await.unwrap()), [0, 0]);
    assert_eq!(low(stack.bounds("c-1b").await.unwrap()), [0, 5000]);
}

/// A grid moved clear of the other leaves the column on them unbuilt, `BadGrid` saying so; moved
/// back it is built again, though it never built in between; a grid that is deleted is the same,
/// naming it.
#[tokio::test]
async fn grids_that_stop_crossing_fail_the_column_and_it_recovers() {
    let stack = Stack::new().await;
    stack.put(&column("c1", on_grids("g1", "ga"))).await;
    assert!(stack.bounds("c1").await.is_ok());
    stack.put(&grid("ga", line(-1000, 9000, 8000, 9000))).await;
    let apart = stack.bounds("c1").await.unwrap_err();
    assert!(
        apart.starts_with("BadGrid: grids g1 and ga do not cross"),
        "{apart}"
    );
    assert!(stack.reads("c1").await.contains(&"ga".to_owned()));
    stack.put(&grid("ga", line(-1000, 0, 8000, 0))).await;
    assert_eq!(stack.bounds("c1").await.unwrap().0[..2], [-200, -200]);
    stack.delete("ga").await;
    let gone = stack.bounds("c1").await.unwrap_err();
    assert!(
        gone.starts_with("BadGrid: the column stands on grid ga, which is not in this model"),
        "{gone}"
    );
    stack.put(&grid("ga", line(-1000, 0, 8000, 0))).await;
    assert!(stack.bounds("c1").await.is_ok());
}

/// A changed type changes every column of it: the type 400 by 400 made 600 by 300 gives the box
/// 700 to 1300 mm by 1850 to 2150 mm about (1000, 2000).
#[tokio::test]
async fn a_changed_type_changes_its_columns() {
    let stack = Stack::new().await;
    stack
        .put(&column(
            "c1",
            Position::Point {
                x: mm(1000),
                y: mm(2000),
            },
        ))
        .await;
    stack.put(&column_type("t400", 600, 300)).await;
    assert_eq!(
        stack.bounds("c1").await.unwrap(),
        ([700, 1850, 0], [1300, 2150, 3000])
    );
}

/// A column that names no type, or one that is not a column type, fails by name; the build says why
/// in construct-columns' words.
#[tokio::test]
async fn a_column_without_a_column_type_fails_by_name() {
    let stack = Stack::new().await;
    let mut record = column("c1", Position::Point { x: mm(0), y: mm(0) });
    record.props.remove("@type");
    stack.put(&record).await;
    let none = stack.bounds("c1").await.unwrap_err();
    assert!(none.starts_with("NoColumnType:"), "{none}");
    let mut record = column("c1", Position::Point { x: mm(0), y: mm(0) });
    record.props.insert("@type".to_owned(), "nope".to_owned());
    stack.put(&record).await;
    assert!(stack.bounds("c1").await.is_err());
}

/// A column turned 90 degrees is the same box turned a quarter, exactly: a 600 by 300 section is
/// 300 by 600 about its centre.
#[tokio::test]
async fn a_turned_column_is_built_turned() {
    let stack = Stack::new().await;
    stack.put(&column_type("t400", 600, 300)).await;
    let mut record = column(
        "c1",
        Position::Point {
            x: mm(1000),
            y: mm(2000),
        },
    );
    record = construct_columns::with_column(
        record,
        &Column {
            position: Position::Point {
                x: mm(1000),
                y: mm(2000),
            },
            turn: Angle::from_units(90 * Angle::UNITS_PER_DEGREE),
            top: Top::Level(tie("L-1", 0)),
        },
    )
    .unwrap();
    stack.put(&record).await;
    assert_eq!(
        stack.bounds("c1").await.unwrap(),
        ([850, 1700, 0], [1150, 2300, 3000])
    );
}

/// The read-only parameters of a column, from its type and levels: height 3000 mm, section area
/// 160,000 mm2, volume 480,000,000 mm3, shown in the document's units and marked read-only; they
/// follow a moved level.
#[tokio::test]
async fn the_read_only_rows_follow_the_levels() {
    let stack = Stack::new().await;
    let record = column("c1", Position::Point { x: mm(0), y: mm(0) });
    stack.put(&record).await;
    let ty = column_type_of("t400", 400, 400);
    let rows = stack.rows(&record, &ty).await;
    let shown: Vec<(&str, Option<&str>)> = rows
        .iter()
        .map(|row| (row.id.as_str(), row.shown.as_deref()))
        .collect();
    assert_eq!(rows.len(), 4);
    assert_eq!(shown[0], ("column.height", Some("3000 mm")));
    assert!(rows.iter().all(|row| row.from == "read-only"));
    assert_eq!(
        rows[2].value.as_deref(),
        Some(
            Value::Volume(construct_params::Volume::from_mm3(480_000_000))
                .stored()
                .as_str()
        )
    );
    stack.put(&level("L-1", 3200).to_record(&i(DOCUMENT))).await;
    let rows = stack.rows(&record, &ty).await;
    assert_eq!(rows[0].shown.as_deref(), Some("3200 mm"));
    assert_eq!(
        rows[2].value.as_deref(),
        Some(
            Value::Volume(construct_params::Volume::from_mm3(512_000_000))
                .stored()
                .as_str()
        )
    );
    // Not a column: none.
    let other = level("L-0", 0).to_record(&i(DOCUMENT));
    assert_eq!(stack.rows(&other, &ty).await.len(), 0);
}

/// An unconnected height needs no top level: 2700 mm up from the base at L-1 (3000 mm) stands from
/// 3000 to 5700 mm in the model; its built row's elevation is 3000 mm and its box 0 to 2700 mm.
#[tokio::test]
async fn an_unconnected_height_stands_from_its_base() {
    let stack = Stack::new().await;
    let record = column_record(
        &i("c1"),
        &i(DOCUMENT),
        "c1",
        &i("t400"),
        &tie("L-1", 0),
        &Column {
            position: Position::Point { x: mm(0), y: mm(0) },
            turn: Angle::from_units(0),
            top: Top::Height(mm(2700)),
        },
    );
    stack.put(&record).await;
    assert_eq!(stack.bounds("c1").await.unwrap().1[2], 2700);
    assert_eq!(
        stack.built("c1").await.elevation,
        Some(Value::Length(mm(3000)).stored())
    );
    assert!(!stack.reads("c1").await.contains(&"L-0".to_owned()));
}
