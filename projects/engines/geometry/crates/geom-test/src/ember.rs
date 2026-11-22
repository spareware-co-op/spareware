//! EMBER's pair 827 as the count gate's benchmark `mesh/ember` takes it (Gate 4, G156): its two
//! `Thingi10K` models, each held to its SHA-256 in the corpus register, moved by EMBER's transforms
//! and built into exact meshes on one grid, exactly as `tests/ember_pairs.rs` runs every cleared
//! pair.
//!
//! What it does: [`model`] reads one model of the fetched corpus `ember-cleared-pairs` from a
//! folder and refuses bytes that are not the register's; [`case`] is the pair's entry of EMBER's
//! case list, written here as the list writes it ([`CASE`]), so the benchmark's job fetches the two
//! models alone and never the list or the rest of the corpus (`tests/ember_pairs.rs` holds this
//! entry to the list's own wherever the whole corpus is fetched); [`meshes`] reads both models,
//! moves each by its transform exactly, puts the two on one grid (`stl::on_one_grid`) and builds
//! each with `geom-mesh`'s builder, serially.
//!
//! Guarantees:
//!
//! - **The register's bytes or nothing.** A model is read only after its SHA-256 equals the
//!   register's; a missing file, another file's bytes, or a name the corpus does not hold is an
//!   error naming the file, never a mesh.
//! - **EMBER's pair.** [`MODELS`] are the pair's two models in EMBER's order, the first less the
//!   second being EMBER's own operation.
//! - **The same meshes on every target.** The reading, the transforms and the grid are
//!   `geom_test::stl`'s exact arithmetic; the builder runs serially.
//!
//! Method: EMBER's case list and its transforms as Manifold's `extras/ember_tests/man_bench.cpp`
//! reads them (<https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/extras/ember_tests/man_bench.cpp>), through
//! [`crate::stl`] and [`crate::polyhedral::mesh_polygons`]; SHA-256 by [`crate::golden::sha256`].
//! Consulted: Manifold, Apache-2.0,
//!   <https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/extras/ember_tests/man_bench.cpp>,
//!   for how EMBER moves and combines a pair. No code copied.
//!
//! Cost: reading and hashing the two files (about 125 KB), [`crate::stl::placed`]'s rationals for
//! their 2,504 faces' corners, and two builds.

use std::path::Path;

use geom_mesh::exact::ExactMesh;
use geom_mesh::exact::build::build;
use geom_run::{Budget, Exec, WorkMeter};

use crate::corpus::{Fetch, Manifest};
use crate::golden::sha256;
use crate::polyhedral::mesh_polygons;
use crate::stl::{self, Case};

/// The fetched corpus the models belong to.
pub const CORPUS: &str = "ember-cleared-pairs";

/// The pair's number in EMBER's case list (its place, from 0).
pub const PAIR: usize = 827;

/// The pair's two models, the first and the second, as the case list names them.
pub const MODELS: [&str; 2] = ["375267.stl", "251402.stl"];

/// The pair's entry in EMBER's case list (`ember-benchmark-cases.json` at Manifold's
/// d056b8fb70de8c8e6d6bee2e639b71c20aa08d01), its numbers written exactly as the list writes them.
pub const CASE: &str = r#"{"id_a": 375267, "id_b": 251402, "transform_a": {"col0": {"x": -0.0208335, "y": 0.00750836, "z": -0.00153649, "w": 0}, "col1": {"x": -0.00536656, "y": -0.0174694, "z": -0.0126013, "w": 0}, "col2": {"x": -0.0054714, "y": -0.011455, "z": 0.0182104, "w": 0}, "col3": {"x": 0.906526, "y": 0.210694, "z": 0.317925, "w": 1}}, "transform_b": {"col0": {"x": -0.018701, "y": 0.00439637, "z": 0.00436959, "w": 0}, "col1": {"x": -0.00475247, "y": -0.0190859, "z": -0.00113681, "w": 0}, "col2": {"x": 0.00397937, "y": -0.00213313, "z": 0.0191772, "w": 0}, "col3": {"x": 2.02604, "y": 6.79692, "z": 0.174242, "w": 1}}}"#;

/// The pair's entry of the case list.
///
/// # Errors
///
/// serde's message, if [`CASE`] is not a case (a test checks it is).
pub fn case() -> Result<Case, String> {
    let mut read = stl::cases(&format!("[{CASE}]"))?;
    read.pop().ok_or_else(|| "the case is empty".to_owned())
}

/// The bytes of the corpus's file `name` in `dir`, once their SHA-256 is the register's.
///
/// # Errors
///
/// A message naming the file, if the register does not hold it, it cannot be read, or its bytes
/// are not the register's.
pub fn model(dir: &Path, name: &str) -> Result<Vec<u8>, String> {
    let manifest = Manifest::embedded().map_err(|e| format!("the corpus register: {e:?}"))?;
    let corpus = manifest
        .get(CORPUS)
        .ok_or_else(|| format!("the register has no corpus {CORPUS}"))?;
    let Some(Fetch::Files(files)) = &corpus.fetch else {
        return Err(format!("{CORPUS} is not fetched file by file"));
    };
    let want = files
        .iter()
        .find(|f| f.name == name)
        .ok_or_else(|| format!("{name} is not a file of {CORPUS}"))?;
    let bytes = std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
    if sha256(&bytes) != want.sha256.0 {
        return Err(format!("{name}: not the register's bytes"));
    }
    Ok(bytes)
}

/// The pair's two models from `dir`, each moved by its transform, on one grid and built: the
/// first and the second of EMBER's operation.
///
/// # Errors
///
/// A message naming the step: a model [`model`] refuses, a file `stl_io` cannot read, a transform
/// `stl::placed` refuses, a grid that cannot be made, or a mesh the builder refuses.
pub fn meshes(dir: &Path) -> Result<[ExactMesh; 2], String> {
    let case = case()?;
    let [first, second] = MODELS;
    let placed = |name: &str, by: &stl::Transform| {
        let read = stl::read(&model(dir, name)?).map_err(|e| format!("{name}: {e}"))?;
        stl::placed(&read, by).map_err(|e| format!("{name}: {e}"))
    };
    let read = [
        placed(first, &case.transform_a)?,
        placed(second, &case.transform_b)?,
    ];
    let grids = stl::on_one_grid(&read).ok_or("the pair has no grid")?;
    let exec = Exec::serial();
    let mut built = Vec::with_capacity(2);
    for (grid, name) in grids.iter().zip([1_u8, 2]) {
        let polygons = mesh_polygons(grid, [0; 3], name)?;
        let mesh = build(&polygons, &exec, &mut WorkMeter::new(Budget::UNLIMITED))
            .map_err(|e| format!("the builder refused model {name}: {e:?}"))?;
        built.push(mesh);
    }
    built.try_into().map_err(|_| "not two meshes".to_owned())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        reason = "test code: a failure should stop loudly"
    )]

    use super::*;

    #[test]
    fn the_case_is_the_pairs_and_its_models_are_the_registers() {
        let case = case().unwrap();
        assert_eq!(
            [format!("{}.stl", case.id_a), format!("{}.stl", case.id_b)],
            MODELS.map(str::to_owned)
        );
        assert_eq!(case.transform_a.col3.w, 1.0);
        let manifest = Manifest::embedded().unwrap();
        let Some(Fetch::Files(files)) = &manifest.get(CORPUS).unwrap().fetch else {
            panic!("{CORPUS} is not fetched file by file")
        };
        for name in MODELS {
            assert!(files.iter().any(|f| f.name == name), "{name}");
        }
    }

    #[cfg_attr(
        target_family = "wasm",
        ignore = "writes a file, which a WebAssembly test run cannot"
    )]
    #[test]
    fn a_file_that_is_not_the_registers_is_refused() {
        // One folder per test binary's name; nothing else writes it.
        let dir = std::env::temp_dir().join("geom-test-ember-refusal");
        std::fs::create_dir_all(&dir).unwrap();
        // Planted: another file's bytes under the model's name, a name the corpus does not hold,
        // and a model missing.
        std::fs::write(dir.join(MODELS[0]), b"solid x\nendsolid x\n").unwrap();
        let wrong = model(&dir, MODELS[0]).unwrap_err();
        assert!(wrong.contains("not the register's bytes"), "{wrong}");
        assert!(model(&dir, "1.stl").unwrap_err().contains("not a file of"));
        assert!(model(&dir, MODELS[1]).unwrap_err().starts_with(MODELS[1]));
        assert!(meshes(&dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
