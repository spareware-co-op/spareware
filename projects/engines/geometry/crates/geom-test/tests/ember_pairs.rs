// SPDX-License-Identifier: Apache-2.0
//! EMBER's case list, as far as its models' own licences allow, held exactly to the rational
//! reference (Gate 4, G156, conditions 1 and 4 under reading R1's A and R2's A).
//!
//! - **The pairs.** Of EMBER's 1,000 pairs (Manifold's `extras/ember_tests`), the 92 whose two
//!   `Thingi10K` models both carry a licence rule (i) of 32.11 admits ([`CLEARED`]): their 171
//!   models and EMBER's case list are the fetched corpus `ember-cleared-pairs`, each file pinned
//!   by its SHA-256 with its own licence and credit. Nothing of it is in the repository.
//! - **Each pair as EMBER runs it.** Both models read (`geom_test::stl`), each moved by its own
//!   transform from the case list exactly, the two put on one grid at one stated scale and rounded
//!   once, built by `geom-mesh`'s builder, and combined by `geom_mesh::boolean`: the first less the
//!   second (EMBER's own operation, Manifold's `man_bench`), and beside it their union and their
//!   intersection, as the polyhedral corpus runs its models. A pair whose meshes the builder
//!   refuses on the grid is named with its typed refusal in [`REFUSED`], as
//!   `tests/corpus_polyhedral.rs` names Manifold's.
//! - **Both checkers and the reference.** Every mesh built or made passes both mesh checkers of
//!   28.4, `geom-mesh`'s own and the second (`geom_test::agree::judge_mesh`), which agree, and
//!   `geom-mesh`'s volume holds the second's exact one; each result's exact volume is the flat
//!   reference's (`flat_reference::six_volumes_where`, by the operands' winding numbers), except
//!   for a pair named in [`NAMED_OUT`], with the reason the reference is left out for it.
//!
//! Tiers: the corpus is a download, so the pairs' test is ignored in the suite and runs in
//! `kernel-oracles.yml`'s job `ember-pairs` on a push to main, after `corpus fetch`, a few pairs to
//! a job (`SPAREKIT_PAIRS`) so that each job takes minutes. The guard tests run everywhere: every
//! cleared pair is in exactly one of those jobs, and the corpus holds exactly the pairs' models
//! and the case list; a planted gap is found.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic, the wall clock for the jobs' balance, and a failure stops the test"
)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;

use geom_mesh::boolean::boolean;
use geom_mesh::exact::ExactMesh;
use geom_mesh::exact::build::build;
use geom_op::{OpId, OpPath};
use geom_poly::winding::WindingTest;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::agree::judge_mesh;
use geom_test::corpus::{Fetch, Manifest, tree_digest};
use geom_test::flat_reference::six_volumes_where;
use geom_test::obj::Grid;
use geom_test::polyhedral::{mesh_polygons, mesh_solid};
use geom_test::stl::{self, Case};

/// The fetched corpus.
const CORPUS: &str = "ember-cleared-pairs";

/// The case list's file in it.
const CASES: &str = "ember-benchmark-cases.json";

/// The 92 pairs whose two models' own licences rule (i) of 32.11 admits: each pair's number in
/// EMBER's list (its place, from 0) and its two `Thingi10K` file ids, as the list gives them.
/// Each model's licence is `Thingi10K`'s own record of it (its dataset's `metadata/input_summary.csv`
/// at tag v1.5.0, which its `contextual_data.csv` agrees with), and the register carries it.
const CLEARED: [(usize, u64, u64); 92] = [
    (1, 80604, 500107),
    (11, 94140, 70831),
    (68, 89420, 500110),
    (73, 1458682, 65282),
    (79, 92119, 132440),
    (98, 1356640, 60916),
    (111, 45513, 472047),
    (134, 74126, 59230),
    (164, 129908, 472011),
    (169, 72100, 206495),
    (173, 75359, 472037),
    (189, 232909, 75986),
    (208, 38741, 100815),
    (213, 472056, 107910),
    (216, 101864, 91013),
    (221, 1273629, 96777),
    (224, 96776, 267664),
    (238, 472133, 472041),
    (248, 472147, 129915),
    (269, 57942, 224504),
    (277, 548867, 1087135),
    (291, 64957, 472078),
    (295, 48940, 217030),
    (303, 60927, 88147),
    (304, 500100, 90359),
    (310, 387871, 472059),
    (317, 206494, 105171),
    (321, 84737, 46780),
    (324, 99776, 472038),
    (337, 65004, 70066),
    (349, 908460, 50881),
    (350, 96772, 1458674),
    (356, 106681, 84032),
    (360, 124036, 124373),
    (377, 1458692, 45515),
    (379, 178193, 129891),
    (387, 225958, 104968),
    (389, 1458694, 95487),
    (395, 132434, 472201),
    (399, 375245, 472039),
    (427, 472155, 75986),
    (432, 235726, 69930),
    (437, 548859, 94492),
    (438, 78738, 109459),
    (447, 110699, 108619),
    (459, 85886, 95500),
    (461, 98625, 472080),
    (523, 59757, 548864),
    (530, 213388, 124034),
    (543, 70834, 45517),
    (553, 472103, 110699),
    (562, 520647, 225957),
    (589, 98660, 77017),
    (594, 472179, 472050),
    (608, 100887, 472000),
    (615, 84918, 94149),
    (618, 58933, 70830),
    (640, 95500, 375281),
    (644, 80760, 84736),
    (654, 107586, 75976),
    (659, 472117, 472061),
    (667, 472042, 99865),
    (675, 80942, 85495),
    (676, 94990, 111011),
    (688, 225957, 500110),
    (689, 95503, 59757),
    (698, 78733, 60880),
    (703, 95487, 226160),
    (720, 95663, 232912),
    (726, 178192, 104640),
    (727, 109772, 669963),
    (738, 94152, 81366),
    (774, 500107, 136639),
    (777, 45513, 500120),
    (788, 43850, 107587),
    (799, 472047, 123787),
    (804, 44950, 225966),
    (815, 472137, 495000),
    (827, 375267, 251402),
    (836, 44948, 213384),
    (837, 90358, 232909),
    (839, 75988, 104640),
    (848, 53048, 95500),
    (852, 472152, 104301),
    (915, 69883, 55963),
    (929, 76907, 196187),
    (930, 939893, 669979),
    (933, 278458, 375278),
    (934, 39498, 94146),
    (947, 104738, 77185),
    (951, 167110, 67501),
    (998, 237737, 1458703),
];

/// The cleared pairs refused before any boolean, with the refusal: a file `stl_io` cannot read, or
/// meshes `geom-mesh`'s builder refuses once on the grid (not closed and manifold there, whose
/// remedy is healing, 19.2). Measured: model 123787 of pair 799 is an ASCII STL (its first line
/// `solid`, its facets' normals `0 0 0`) that `stl_io` 0.11.0 stops reading partway.
const REFUSED: [(usize, &str); 1] = [(799, "123787.stl: Read(\"failed to fill whole buffer\")")];

/// The cleared pairs held to both checkers and the kernel's own exact volume but not to the
/// reference, each with why.
const NAMED_OUT: [(usize, &str); 2] = [
    (
        79,
        "the reference's rationals took 957 s on one core of a development machine (27,362 and          85,010 faces), past a job of minutes (the testing principle's), though they agreed          exactly there; the kernel and both checkers take about 67 s",
    ),
    (
        930,
        "the reference refuses the pair's meshes on the grid as not closed (NotClosed) where      geom-mesh's builder and both mesh checkers accept them; Thingi10K's own flags say a model      of this pair is not closed, so it is not a solid the reference's winding numbers can read",
    ),
];

/// What EMBER's operation and the two beside it keep of the pair's winding numbers: the first
/// less the second, their union and their intersection.
const OPERATIONS: [&str; 3] = ["the first less the second", "union", "intersection"];

fn tests() -> [WindingTest; 3] {
    [
        WindingTest::subtract(0),
        WindingTest::unite(),
        WindingTest::intersect(2),
    ]
}

fn keep(operation: usize, w: &[i64]) -> bool {
    let (a, b) = (w[0] > 0, w[1] > 0);
    match operation {
        0 => a && !b,
        1 => a || b,
        _ => a && b,
    }
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The register's entry for the corpus.
fn entry() -> geom_test::corpus::Corpus {
    Manifest::embedded().unwrap().get(CORPUS).unwrap().clone()
}

/// The fetched corpus's folder, `SPAREKIT_CORPORA/ember-cleared-pairs`, checked against the
/// register's digest before anything reads it.
fn folder() -> PathBuf {
    let cache = std::env::var("SPAREKIT_CORPORA").unwrap_or_else(|_| {
        panic!(
            "SPAREKIT_CORPORA names no cache: fetch the corpus first, `cargo run -p geom-test \
             --bin corpus -- fetch --cache <dir> {CORPUS}`, and set SPAREKIT_CORPORA=<dir>"
        )
    });
    let dir = PathBuf::from(cache).join(CORPUS);
    assert_eq!(
        Some(tree_digest(&dir).unwrap()),
        entry().sha256,
        "{}: not the register's bytes",
        dir.display()
    );
    dir
}

/// The pairs this run takes: every cleared one, or those `SPAREKIT_PAIRS` names by number,
/// separated by commas. A number that is not a cleared pair's stops the test, so a job cannot test
/// nothing.
fn chosen() -> Vec<(usize, u64, u64)> {
    let Ok(names) = std::env::var("SPAREKIT_PAIRS") else {
        return CLEARED.to_vec();
    };
    let names: BTreeSet<usize> = names
        .split(',')
        .map(|n| n.trim().parse().unwrap())
        .collect();
    let known: BTreeSet<usize> = CLEARED.iter().map(|(n, _, _)| *n).collect();
    let unknown: Vec<&usize> = names.difference(&known).collect();
    assert!(
        unknown.is_empty(),
        "SPAREKIT_PAIRS names {unknown:?}, which are not cleared pairs"
    );
    CLEARED
        .into_iter()
        .filter(|(n, _, _)| names.contains(n))
        .collect()
}

/// Both mesh checkers on `mesh`: both pass, so they agree, and `geom-mesh`'s volume holds the
/// second's exact one, which is returned.
fn both_meshes(what: &str, mesh: &ExactMesh) -> dashu_ratio::RBig {
    let v = judge_mesh(mesh);
    assert!(
        v.agree() && v.passed(),
        "{what}: the mesh checkers {:?} and {:?}, volumes held {:?}",
        v.first,
        v.second,
        v.volume_held
    );
    v.six_volume.unwrap()
}

/// One pair's two models moved by their transforms and put on one grid, or the file `stl_io`
/// refuses, with its refusal.
fn grids(dir: &std::path::Path, case: &Case) -> Result<[Grid; 2], String> {
    let model = |id: u64, by| {
        let bytes = std::fs::read(dir.join(format!("{id}.stl"))).unwrap();
        let read = stl::read(&bytes).map_err(|e| format!("{id}.stl: {e:?}"))?;
        Ok::<_, String>(stl::placed(&read, by).unwrap())
    };
    let read = [
        model(case.id_a, &case.transform_a)?,
        model(case.id_b, &case.transform_b)?,
    ];
    let grids: [Grid; 2] = stl::on_one_grid(&read).unwrap().try_into().unwrap();
    Ok(grids)
}

/// A pair run as EMBER runs it, held to both checkers and the reference: its reader's or its
/// builder's refusal, or nothing.
fn run(pair: usize, case: &Case, dir: &std::path::Path) -> Option<String> {
    let started = Instant::now();
    let [a, b] = match grids(dir, case) {
        Ok(grids) => grids,
        Err(why) => return Some(why),
    };
    let exec = Exec::serial();
    let mesh = |grid: &Grid, name: u8| {
        build(
            &mesh_polygons(grid, [0; 3], name).unwrap(),
            &exec,
            &mut meter(),
        )
    };
    let (first, second) = match (mesh(&a, 1), mesh(&b, 2)) {
        (Ok(first), Ok(second)) => (first, second),
        (Err(e), _) | (_, Err(e)) => return Some(format!("{e:?}")),
    };
    both_meshes(&format!("pair {pair}, the first built"), &first);
    both_meshes(&format!("pair {pair}, the second built"), &second);
    let step = OpPath::new(OpId::from_bytes([9; 16]));
    let made = boolean(&[&first, &second], &tests(), &step, &exec, &mut meter()).unwrap();
    let kernel = started.elapsed();
    let volumes: Vec<_> = made
        .iter()
        .zip(OPERATIONS)
        .map(|(one, what)| both_meshes(&format!("pair {pair}, {what}"), &one.mesh))
        .collect();
    let checked = started.elapsed();
    if let Some((_, why)) = NAMED_OUT.iter().find(|(n, _)| *n == pair) {
        println!("pair {pair}: the reference is named out: {why}");
        return None;
    }
    let solids = [
        mesh_solid(&a, [0; 3]).unwrap(),
        mesh_solid(&b, [0; 3]).unwrap(),
    ];
    let keeps: Vec<Box<dyn Fn(&[i64]) -> bool>> = (0..OPERATIONS.len())
        .map(|k| Box::new(move |w: &[i64]| keep(k, w)) as Box<dyn Fn(&[i64]) -> bool>)
        .collect();
    let borrowed: Vec<&dyn Fn(&[i64]) -> bool> = keeps.iter().map(|k| k.as_ref()).collect();
    let reference = six_volumes_where(&[&solids[0], &solids[1]], &borrowed).unwrap();
    for ((got, want), what) in volumes.iter().zip(&reference).zip(OPERATIONS) {
        assert_eq!(
            got, want,
            "pair {pair}, {what}: the kernel against the reference"
        );
    }
    println!(
        "pair {pair}: {} and {} faces, built and made in {:.1} s, checked by {:.1} s, the \
         reference by {:.1} s",
        a.faces.len(),
        b.faces.len(),
        kernel.as_secs_f64(),
        checked.as_secs_f64(),
        started.elapsed().as_secs_f64()
    );
    None
}

/// Every cleared pair this run takes ([`chosen`]), each as EMBER runs it, through both mesh
/// checkers, its results' exact volumes the reference's; the pairs the builder refuses are those
/// [`REFUSED`] names.
#[test]
#[ignore = "reads the fetched corpus ember-cleared-pairs; kernel-oracles.yml's job ember-pairs \
            runs it on a push to main, a few pairs to a job"]
fn every_cleared_pair_is_the_reference() {
    let dir = folder();
    let cases = stl::cases(&std::fs::read_to_string(dir.join(CASES)).unwrap()).unwrap();
    let pairs = chosen();
    let mut refused = Vec::new();
    for (pair, id_a, id_b) in &pairs {
        let case = &cases[*pair];
        assert_eq!((case.id_a, case.id_b), (*id_a, *id_b), "pair {pair}");
        if let Some(why) = run(*pair, case, &dir) {
            refused.push((*pair, why));
        }
    }
    let expected: Vec<(usize, &str)> = REFUSED
        .into_iter()
        .filter(|(n, _)| pairs.iter().any(|(p, _, _)| p == n))
        .collect();
    let got: Vec<(usize, &str)> = refused.iter().map(|(n, w)| (*n, w.as_str())).collect();
    assert_eq!(got, expected);
}

/// The groups of pairs `kernel-oracles.yml`'s job `ember-pairs` runs, one job each, as its matrix
/// lists them.
fn job_groups(workflow: &str) -> Vec<Vec<usize>> {
    let docs = yaml_rust2::YamlLoader::load_from_str(workflow).unwrap();
    let job = &docs[0]["jobs"]["ember-pairs"];
    assert!(
        !job.is_badvalue(),
        "kernel-oracles.yml has no job ember-pairs"
    );
    job["strategy"]["matrix"]["pairs"]
        .as_vec()
        .unwrap_or_else(|| panic!("ember-pairs has no matrix of pairs"))
        .iter()
        .map(|group| {
            group
                .as_str()
                .unwrap()
                .split(',')
                .map(|n| n.trim().parse().unwrap())
                .collect()
        })
        .collect()
}

/// Every pair of `groups`, once each, in order of number.
fn every_once(groups: &[Vec<usize>]) -> Result<Vec<usize>, String> {
    let mut all: Vec<usize> = groups.iter().flatten().copied().collect();
    all.sort_unstable();
    let before = all.len();
    all.dedup();
    if all.len() == before {
        Ok(all)
    } else {
        Err("a pair is in two jobs".to_owned())
    }
}

/// Every cleared pair runs in exactly one job of `ember-pairs`, no job names anything else, and
/// every pair [`REFUSED`] or [`NAMED_OUT`] names is a cleared one; a planted gap, the matrix
/// without its last group, and a pair planted twice are each found.
#[test]
#[cfg_attr(target_os = "wasi", ignore = "reads the workflow from the tree")]
fn every_cleared_pair_runs_in_one_job() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .join(".github/workflows/kernel-oracles.yml");
    let workflow = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let groups = job_groups(&workflow);
    let cleared: Vec<usize> = CLEARED.iter().map(|(n, _, _)| *n).collect();
    assert_eq!(every_once(&groups), Ok(cleared.clone()));
    for n in REFUSED
        .iter()
        .map(|(n, _)| n)
        .chain(NAMED_OUT.iter().map(|(n, _)| n))
    {
        assert!(cleared.contains(n), "pair {n} is not a cleared pair");
    }
    // The planted gap: the last group left out.
    let mut dropped = groups.clone();
    dropped.pop();
    assert_ne!(
        every_once(&dropped),
        Ok(cleared.clone()),
        "the dropped group was not seen"
    );
    // A pair planted in a second job.
    let mut twice = groups.clone();
    twice[0].push(groups[1][0]);
    assert!(
        every_once(&twice).is_err(),
        "the pair in two jobs was not seen"
    );
}

/// The register's corpus holds exactly the cleared pairs' models and the case list, each model
/// under a licence of its own with its credit, and the pairs are EMBER's own numbers in order.
#[test]
fn the_corpus_is_the_cleared_pairs_models() {
    let corpus = entry();
    assert_eq!(corpus.storage, geom_test::corpus::Storage::Fetched);
    let Some(Fetch::Files(files)) = &corpus.fetch else {
        panic!("{CORPUS} is not fetched file by file")
    };
    let names: BTreeSet<String> = files.iter().map(|f| f.name.clone()).collect();
    let mut wanted: BTreeSet<String> = CLEARED
        .iter()
        .flat_map(|(_, a, b)| [format!("{a}.stl"), format!("{b}.stl")])
        .collect();
    assert_eq!(wanted.len(), 171);
    wanted.insert(CASES.to_owned());
    assert_eq!(names, wanted);
    for f in files {
        assert!(f.licence.is_some() && f.credit.is_some(), "{}", f.name);
    }
    let numbers: Vec<usize> = CLEARED.iter().map(|(n, _, _)| *n).collect();
    assert!(numbers.windows(2).all(|w| w[0] < w[1]) && numbers.iter().all(|n| *n < 1_000));
}
