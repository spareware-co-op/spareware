//! How long healing takes, by the standard library's clock (piece G298): a thousand boxes 1 mm
//! apart as a soup, every face with its own copies of its corners, each copy moved up to 100 steps,
//! as an export with near-duplicate vertices gives. The weld alone, then the whole heal through the
//! resolve, on 1 and 8 threads. Prints one line per run.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::print_stdout,
    reason = "a benchmark program; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::{NonZeroU64, NonZeroUsize};
use std::time::Instant;

use geom_mesh::exact::Role;
use geom_mesh::exact::build::Source;
use geom_mesh::exact::raw::RawMesh;
use geom_mesh::repair::{Heal, heal, weld};
use geom_num::grid::{P3, Steps};
use geom_op::{NameId, OpId, OpPath};
use geom_pool::native::Rayon;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::{Seed, SplitMix64};

const SIDES: [[usize; 4]; 6] = [
    [0, 4, 6, 2],
    [1, 3, 7, 5],
    [0, 1, 5, 4],
    [2, 6, 7, 3],
    [0, 2, 3, 1],
    [4, 5, 7, 6],
];

fn main() {
    let mut rng = SplitMix64::new(Seed(0x298));
    let jitter = NonZeroU64::new(101).unwrap();
    let (side, gap) = (6400_i64, 12_800_i64);
    let (mut vertices, mut corners, mut face_start) = (Vec::new(), Vec::new(), vec![0_u32]);
    for b in 0..1000_i64 {
        let low = [b % 10, b / 10 % 10, b / 100].map(|c| c * gap);
        for s in SIDES {
            for v in s {
                let at = [0, 1, 2].map(|a| {
                    low[a] + if v >> a & 1 == 1 { side } else { 0 } + rng.below(jitter) as i64
                });
                corners.push(vertices.len() as u32);
                vertices.push(P3::new(at[0], at[1], at[2]).unwrap());
            }
            face_start.push(corners.len() as u32);
        }
    }
    let faces = face_start.len() - 1;
    let raw = RawMesh::new(vertices, corners, face_start).unwrap();
    let source = Source {
        name: NameId::from_bytes([2; 16]),
        role: Role::FaceSet,
        flags: 0,
    };
    let op = OpPath::new(OpId::from_bytes([8; 16]));
    let rayon = Rayon::new(NonZeroUsize::new(8).unwrap()).unwrap();
    for (name, exec) in [
        ("1 thread", Exec::serial()),
        ("8 threads", Exec::with_pool(&rayon)),
    ] {
        let start = Instant::now();
        let welded = weld(
            &raw,
            Steps(640),
            &exec,
            &mut WorkMeter::new(Budget::UNLIMITED),
        )
        .unwrap();
        let weld_time = start.elapsed();
        let start = Instant::now();
        let healed = heal(
            &raw,
            Heal::default(),
            source,
            &op,
            &exec,
            &mut WorkMeter::new(Budget::UNLIMITED),
        )
        .unwrap();
        println!(
            "{name}: {faces} faces, {} corners welded into {} vertices in {weld_time:?}; healed into {} faces in {:?}",
            raw.vertices().len(),
            welded.mesh.vertices().len(),
            healed.mesh.faces(),
            start.elapsed()
        );
    }
}
