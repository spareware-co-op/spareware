//! EMBER's pair 827 as the count benchmark `mesh/ember/pair_827` measures it: the batch, the
//! kernel and the golden check, shared by the benchmark (`main.rs`) and its native test
//! (`tests/ember.rs`), which includes this file.
//!
//! The batch is one pair: EMBER's pair 827, its two `Thingi10K` models read from the folder
//! [`FOLDER`] names, each held to its SHA-256 in the corpus register, moved by EMBER's transforms
//! and built into exact meshes on one grid (`geom_test::ember::meshes`). The kernel is EMBER's own
//! operation, the first less the second, through `geom_mesh::boolean::boolean`, serially, under an
//! unlimited budget, as `kernels::mesh::boolean` runs its union. The golden digests of the batch
//! and its result are in `golden.txt` beside this file: the benchmark is outside geom-bench's
//! catalogue (its models are a download, which no other job fetches), so `src/golden.txt`, which
//! lists exactly the catalogue, does not hold them.

use std::path::PathBuf;

use geom_bench::golden;
use geom_bench::harness::{Bench, Bound, Digests, Kernel};
use geom_bench::inputs::mesh::Pair;
use geom_mesh::boolean::{Outcome, boolean};
use geom_mesh::exact::MeshError;
use geom_op::{OpId, OpPath};
use geom_poly::winding::WindingTest;
use geom_run::{Budget, Exec, WorkMeter};

/// The benchmark's name, `group/op/class`.
pub const NAME: &str = "mesh/ember/pair_827";

/// The environment variable naming the folder that holds the pair's two models.
pub const FOLDER: &str = "SPAREKIT_EMBER";

/// The golden digests of the batch and its result, in `src/golden.txt`'s format.
pub const GOLDEN: &str = include_str!("golden.txt");

/// `geom_mesh::boolean::boolean` on each pair: the first less the second, serially, under an
/// unlimited budget.
#[inline(never)]
pub fn subtract(batch: &[Pair], out: &mut Vec<Result<Vec<Outcome>, MeshError>>) {
    let step = OpPath::new(OpId::from_bytes([7; 16]));
    out.clear();
    out.extend(batch.iter().map(|pair| {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        boolean(
            &[&pair.first, &pair.second],
            &[WindingTest::subtract(0)],
            &step,
            &Exec::serial(),
            &mut meter,
        )
    }));
}

/// The benchmark ready to run, from the folder [`FOLDER`] names.
///
/// # Errors
///
/// A message, if [`FOLDER`] is not set or a model is missing, not the register's, or refused.
pub fn prepare() -> Result<Box<dyn Bench>, String> {
    let dir = std::env::var_os(FOLDER)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{FOLDER} names no folder of pair 827's two models"))?;
    let [first, second] = geom_test::ember::meshes(&dir)?;
    let kernel: Kernel<Pair, Result<Vec<Outcome>, MeshError>> = subtract;
    Ok(Box::new(Bound::new(vec![Pair { first, second }], kernel)))
}

/// Whether `actual` are the recorded golden digests; otherwise the line to record.
///
/// # Errors
///
/// A message with the benchmark's line as `golden.txt` would hold it, if they differ or are not
/// recorded.
pub fn check(actual: &Digests) -> Result<(), String> {
    let recorded = golden::parse(GOLDEN)?;
    let line = format!("{NAME} {} {}", actual.inputs, actual.outputs);
    match recorded.as_slice() {
        [entry] if entry.name == NAME && entry.digests == *actual => Ok(()),
        _ => Err(format!(
            "{NAME}: the golden digests differ from golden.txt; if intended, its line is:\n{line}"
        )),
    }
}
