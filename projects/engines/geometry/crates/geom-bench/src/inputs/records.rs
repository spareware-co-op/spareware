//! Inputs for `geom-format`'s file readers and writers (pieces G92, G94, G95, G96 and G108,
//! decision 32.46): records and files, a body's records, names and history records, operation
//! records, and a container to open at either trust.
//!
//! What it does: [`record`] makes a record holding a short `f64` column, as the record layer's own
//! benchmark (`benches/records.rs` in `geom-format`) does; [`file`] writes a header and two such
//! records as a file. [`boxes`] builds a box with `geom-make`, a small whole solid; [`stored`] is
//! its records. [`graph`] interns three names of three kinds and [`history`] is one operation's
//! history over a few names, as `tests/names_history.rs` writes them; [`operation`] is one authored
//! operation, as `tests/operation.rs` writes it. [`container`] holds a box's records and its
//! validation stamp, read from a file as `benches/open.rs` does, and [`Pass`] is the checker it is
//! opened with. Each input draws its own coordinates, names and values, so no two are alike.
//!
//! Guarantees: every typical batch is full and every call succeeds (`tests/inputs.rs`), so each
//! benchmark measures the path that accepts: every record and file reads back, every body
//! rebuilds, every name's ID matches its bytes, and every root of a container opens as its trust
//! asks. Only the typical class exists: the refusals are tested in `geom-format` itself.
//!
//! Method: the cases of `geom-format`'s own tests and wall-clock benchmarks of the pieces
//! (`tests/record_layer.rs`, `tests/body_records.rs`, `tests/names_history.rs`,
//! `tests/operation.rs`, `tests/trust.rs`; `benches/records.rs`, `benches/open.rs`), cut small: a
//! short column, a box (the smallest whole solid) and a few names. A batch of records, names or
//! operations costs a few hundred thousand to a few million instructions, and one of bodies or
//! containers some tens of millions, as a box's records are; each is measured as chapter 28.14 sets
//! benchmarks out, `projects/engines/geometry/architecture/28-testing.md`. The checker passes every
//! body for one unit of work, as `benches/open.rs`'s does, so an open measures reading, rebuilding
//! and the open's own bookkeeping; the checker's cost is `geom-check`'s to measure.
//!
//! Cost: O([`BATCH`](super::BATCH)) draws, and for the bodies and containers one box built and
//! written per input, once per batch, outside every measured run.

use geom_format::body::{StoredBody, write_body};
use geom_format::container::Container;
use geom_format::content::ContentId;
use geom_format::file::{ReadFile, read, write};
use geom_format::header::{Header, PRE_EDITION};
use geom_format::history::history_record;
use geom_format::names::names_record;
use geom_format::operation::{Operation, operation_record};
use geom_format::recipe::RecipeInput;
use geom_format::record::{REQUIRED, Record};
use geom_format::registry::knows;
use geom_format::source::SliceSource;
use geom_format::stamp::Stamp;
use geom_format::trust::{Checked, Checker, Level};
use geom_make::{BoxInput, make_box};
use geom_name::NameGraph;
use geom_num::grid::{P3, Steps};
use geom_op::cbor::Encoder;
use geom_op::{
    History, NameId, NameList, NameSet, OpContext, OpId, OpPath, Ordinal, Origin, Record as Entry,
    Role, RoleArg, TopoName, TopologyChange, Vanish,
};
use geom_run::{Budget, Stopped, WorkMeter};
use geom_topo::Body;

use super::{Class, batch_of, p3, rng, within};
use crate::rng::SplitMix64;

/// The rows of a record's column.
pub const ROWS: usize = 16;
/// The validity checker's version and groups the stamps record.
pub const CHECKER: u64 = 1;
/// The groups [`Pass`] runs, by chapter 8.3's numbers.
pub const GROUPS: [u64; 2] = [1, 4];

/// A meter under no limit.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// Sixteen drawn bytes.
fn bytes16(g: &mut SplitMix64) -> [u8; 16] {
    let (a, b) = (g.next_u64().to_le_bytes(), g.next_u64().to_le_bytes());
    let mut out = [0; 16];
    for (o, v) in out.iter_mut().zip(a.iter().chain(&b)) {
        *o = *v;
    }
    out
}

/// A drawn name.
fn name(g: &mut SplitMix64) -> NameId {
    NameId::from_bytes(bytes16(g))
}

/// A drawn operation path with one step, as the names tests write one.
fn path(g: &mut SplitMix64) -> Option<OpPath> {
    OpPath::new(OpId::from_bytes(bytes16(g))).step("cut")
}

/// A record of a column of [`ROWS`] doubles in [1, 2).
fn a_record(g: &mut SplitMix64) -> Option<Record> {
    let column: Vec<f64> = (0..ROWS)
        .map(|_| f64::from_bits(0x3ff0_0000_0000_0000 | (g.next_u64() >> 12)))
        .collect();
    let mut e = Encoder::new();
    e.f64_column(&column).ok()?;
    Some(Record {
        tag: 20 + g.below(2),
        flags: REQUIRED,
        schema: 1,
        depends: Vec::new(),
        payload: e.finish().ok()?.into_bytes(),
    })
}

/// The header every file here is written with.
fn header() -> Option<Record> {
    Header::v1("geom-bench", Steps(640)).record().ok()
}

/// One record per call of `Record::encode`.
#[must_use]
pub fn record(class: Class) -> Vec<Record> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("record::encode", class);
    batch_of(|_| a_record(&mut g))
}

/// The bytes of one record per call of `Record::decode`.
#[must_use]
pub fn record_bytes(class: Class) -> Vec<Vec<u8>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("record::decode", class);
    batch_of(|_| a_record(&mut g)?.encode(&mut meter()).ok())
}

/// A header and two records per call of `file::write`.
#[must_use]
pub fn file_parts(class: Class) -> Vec<(Record, Vec<Record>)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("record::file_write", class);
    batch_of(|_| Some((header()?, vec![a_record(&mut g)?, a_record(&mut g)?])))
}

/// The bytes of one file of a header and two records per call of `file::read`.
#[must_use]
pub fn file(class: Class) -> Vec<Vec<u8>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("record::file_read", class);
    batch_of(|_| {
        let records = [a_record(&mut g)?, a_record(&mut g)?];
        write(&header()?, &records, &mut meter()).ok()
    })
}

/// A size of 1 to 2^16 steps.
fn size(g: &mut SplitMix64) -> Steps {
    Steps(g.range(1, 1 << 16).unsigned_abs())
}

/// A box within ±2^20 grid steps of the origin, its sizes from 1 to 2^16 steps, built and checked
/// by `geom-make`.
fn a_box(g: &mut SplitMix64) -> Option<Body> {
    let reach = 1 << 20;
    let min: P3 = p3(within(g, reach), within(g, reach), within(g, reach));
    let input = BoxInput {
        min,
        size: [size(g), size(g), size(g)],
    };
    let ctx = OpContext::new(OpPath::new(OpId::from_bytes([1; 16])), Budget::UNLIMITED);
    Some(make_box(&ctx, &input).ok()?.result.body)
}

/// One box per call of `write_body`.
#[must_use]
pub fn boxes(class: Class) -> Vec<Body> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("body_record::write", class);
    batch_of(|_| a_box(&mut g))
}

/// One box's records per call of `read_body`.
#[must_use]
pub fn stored(class: Class) -> Vec<StoredBody> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("body_record::read", class);
    batch_of(|_| write_body(&a_box(&mut g)?, &mut meter()).ok())
}

/// A name graph of three names: one made by an operation's role from two others, one merged from
/// two, and one weak.
fn a_graph(g: &mut SplitMix64) -> Option<NameGraph> {
    let op = path(g)?;
    let role = Role::new("side", [RoleArg::Count(g.below(8))]).ok()?;
    let names = [
        TopoName::Created {
            op: op.clone(),
            role,
            from: NameList::new([name(g), name(g)]),
        },
        TopoName::Merged {
            op,
            of: NameSet::new([name(g), name(g)]).ok()?,
        },
        TopoName::Weak {
            base: name(g),
            ord: Ordinal::new(1, 2).ok()?,
        },
    ];
    let mut graph = NameGraph::new();
    for n in names {
        graph.intern(n).ok()?;
    }
    Some(graph)
}

/// One name graph per call of `names_record`.
#[must_use]
pub fn graph(class: Class) -> Vec<NameGraph> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("name_record::names_write", class);
    batch_of(|_| a_graph(&mut g))
}

/// One names record's payload per call of `read_names`.
#[must_use]
pub fn names_payload(class: Class) -> Vec<Vec<u8>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("name_record::names_read", class);
    batch_of(|_| Some(names_record(&a_graph(&mut g)?).ok()?.payload))
}

/// One operation's history: one output unchanged, one modified from an input, and a face that
/// vanished.
fn a_history(g: &mut SplitMix64) -> Option<(OpPath, History)> {
    let history = History::new(
        [
            Entry {
                output: name(g),
                origin: Origin::Unchanged,
            },
            Entry {
                output: name(g),
                origin: Origin::Modified {
                    from: NameSet::new([name(g)]).ok()?,
                },
            },
        ],
        NameSet::default(),
        [TopologyChange::FaceVanished {
            face: name(g),
            why: Vanish::Collapsed,
        }],
    )
    .ok()?;
    Some((path(g)?, history))
}

/// One path and history per call of `history_record`.
#[must_use]
pub fn history(class: Class) -> Vec<(OpPath, History)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("name_record::history_write", class);
    batch_of(|_| a_history(&mut g))
}

/// One history record's payload per call of `read_history`.
#[must_use]
pub fn history_payload(class: Class) -> Vec<Vec<u8>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("name_record::history_read", class);
    batch_of(|_| {
        let (op, history) = a_history(&mut g)?;
        Some(history_record(&op, &history).ok()?.payload)
    })
}

/// One authored operation: a subtraction of a body by an entity, with two parameters.
fn an_operation(g: &mut SplitMix64) -> Option<Operation> {
    let small = u8::try_from(g.below(24)).ok()?;
    Some(Operation {
        op: path(g)?,
        name: "combine.subtract".to_owned(),
        version: 3,
        inputs: vec![
            RecipeInput::Body(ContentId::of_payload(&bytes16(g))),
            RecipeInput::Entity(name(g)),
        ],
        // [small, "deep"]: one canonical item.
        params: vec![0x82, small, 0x64, b'd', b'e', b'e', b'p'],
        budget: Budget::new(10_000, 1 << 20),
    })
}

/// One operation per call of `operation_record`.
#[must_use]
pub fn operation(class: Class) -> Vec<Operation> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("op_record::write", class);
    batch_of(|_| an_operation(&mut g))
}

/// One operation record's payload per call of `read_operation`.
#[must_use]
pub fn operation_payload(class: Class) -> Vec<Vec<u8>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("op_record::read", class);
    batch_of(|_| {
        Some(
            operation_record(&an_operation(&mut g)?, &mut meter())
                .ok()?
                .payload,
        )
    })
}

/// A file of one box's records and its validation stamp, as read.
fn a_file(g: &mut SplitMix64) -> Option<ReadFile> {
    let stored = write_body(&a_box(g)?, &mut meter()).ok()?;
    let stamp = Stamp {
        body: stored.content_id(),
        checker: CHECKER,
        groups: GROUPS.to_vec(),
        edition: PRE_EDITION,
    };
    let mut records = stored.chunks;
    records.push(stored.index);
    records.push(stamp.record().ok()?);
    let bytes = write(&header()?, &records, &mut meter()).ok()?;
    read(&mut SliceSource::new(&bytes), knows, &mut meter()).ok()
}

/// One file as read per call of `Container::from_file`.
#[must_use]
pub fn read_file(class: Class) -> Vec<ReadFile> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("open::from_file", class);
    batch_of(|_| a_file(&mut g))
}

/// One container of a stamped box per call of `open`, for the trust `op` names.
#[must_use]
pub fn container(op: &str, class: Class) -> Vec<Container> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng(op, class);
    batch_of(|_| Container::from_file(&a_file(&mut g)?, &mut meter()).ok())
}

/// A checker that passes every body for one unit of work, as `benches/open.rs` in `geom-format`
/// has one.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pass;

impl Checker for Pass {
    type Report = ();

    fn version(&self) -> u64 {
        CHECKER
    }

    fn groups(&self) -> Vec<u64> {
        GROUPS.to_vec()
    }

    fn check(
        &mut self,
        _: &Body,
        _: Level,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Checked<()>, Stopped> {
        meter.charge(1)?;
        Ok(Checked {
            ran: GROUPS.to_vec(),
            passed: true,
            report: (),
        })
    }
}
