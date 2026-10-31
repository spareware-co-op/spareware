//! Inputs for `geom-format`'s hot paths (piece G105, decision 32.44): calls whose cache keys differ
//! by edition and input, keys to look up in a memo store, and new versions of a body, each one row
//! of its last chunk moved to another surface.
//!
//! What it does: [`cache_key`] gives an edition and an input's seed per call; [`memo_get`] gives
//! the place of a key in the store [`store`] fills; [`next_version`] gives a first version of the
//! body [`faces`] with a body one row changed from it.
//!
//! Guarantees: every typical batch is full and every call succeeds (`tests/inputs.rs`), so each
//! benchmark measures the path that succeeds: every key is found, and a new version hashes one
//! chunk, as an edit in place does. Only the typical class exists: a key's cost does not depend on
//! its bytes.
//!
//! Method: none; seeded draws of calls, as every input of this crate, with no published method to
//! follow.
//!
//! Cost: O(`BATCH`) draws; a batch of versions shares one body's chunks.

use geom_format::body::BodyVersion;
use geom_format::cache::{CacheEntry, CacheKey, MemoStore};
use geom_format::content::ContentId;
use geom_math::Box3;
use geom_num::Exactness;
use geom_num::grid::P3;
use geom_op::{Class as OpClass, Family, OpId, OpOutput, OpPath, OpSchema, Precision};
use geom_run::{Budget, Usage, WorkMeter};
use geom_surface::{Plane, SurfaceKind};
use geom_topo::{Body, BodyKind, Builder, Face, Ix, Region, RegionKind, Sense, Shell};

use super::{Class, batch_of, rng};

/// The schema every benchmarked key is made under.
pub const SCHEMA: OpSchema = OpSchema {
    name: "bench.make",
    version: 1,
    family: Family::Make,
    doc: "The cache benchmarks' operation.",
    inputs: &[],
    outputs: &[],
    roles: &[],
    errors: &[],
    diagnostics: &[],
    classes: &[OpClass::Exact],
    charges: &[],
    batch: false,
    name_free: false,
    example: "none",
};

/// The rows a store holds.
const STORED: u64 = 256;
/// The faces of [`faces`]: past one chunk, so a new version has a chunk to keep.
const FACES: u32 = 5_000;

/// The path every benchmarked key is made at.
#[must_use]
pub fn path() -> OpPath {
    OpPath::new(OpId::from_bytes([7; 16]))
}

/// The input `seed` stands for.
#[must_use]
pub fn input(seed: u64) -> ContentId {
    ContentId::of_payload(&seed.to_le_bytes())
}

/// The key of stored row `row`.
#[must_use]
pub fn key(row: u64) -> Option<CacheKey> {
    CacheKey::new(0, &SCHEMA, &path(), Precision::Final, &[input(row)]).ok()
}

/// An edition and an input's seed per call of `CacheKey::new`.
#[must_use]
pub fn cache_key(class: Class) -> Vec<(u64, u64)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("format::cache_key", class);
    batch_of(|_| Some((g.next_u64() & 3, g.next_u64())))
}

/// A store of [`STORED`] answers, each the row of its key.
#[must_use]
pub fn store() -> MemoStore<u64> {
    let mut held = MemoStore::new(STORED);
    for row in 0..STORED {
        let answer = Ok(OpOutput::new(row, Exactness::Exact));
        if let (Some(key), Some(entry)) = (
            key(row),
            CacheEntry::new(answer, Budget::UNLIMITED, Usage::new(1, 0)),
        ) {
            held.insert(key, entry, 1);
        }
    }
    held
}

/// The key of a row of [`store`] per call of `MemoStore::get`.
#[must_use]
pub fn memo_get(class: Class) -> Vec<CacheKey> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("format::memo_get", class);
    batch_of(|_| key(g.next_u64() % STORED))
}

/// A body of [`FACES`] faces, as geom-topo's arena tests build one: a `next` chain through a shell
/// and two planes, so a face can move to the other.
#[must_use]
pub fn faces() -> Option<Body> {
    let mut b = Builder::new();
    let a = &mut b.arenas;
    a.regions
        .extend(&[
            Region {
                kind: RegionKind::Solid,
                first_shell: Ix::new(0),
            },
            Region {
                kind: RegionKind::Void,
                first_shell: None,
            },
        ])
        .ok()?;
    a.shells
        .push(Shell {
            region: Ix::new(0)?,
            next: None,
            first_face: Ix::new(0),
        })
        .ok()?;
    for origin in [0.0, 1.0] {
        let plane = Plane::new(
            [0.0, 0.0, origin],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .ok()?;
        a.surfaces.push(SurfaceKind::Plane(plane)).ok()?;
    }
    for f in 0..FACES {
        let x = i64::from(f);
        a.faces
            .push(Face {
                shell: Ix::new(0)?,
                next: if f + 1 < FACES { Ix::new(f + 1) } else { None },
                surface: Ix::new(0)?,
                sense: Sense::Forward,
                first_loop: None,
                bound: Box3::new(P3::new(x, 0, 0).ok()?, P3::new(x + 1, 1, 0).ok()?).ok()?,
            })
            .ok()?;
    }
    b.publish(
        BodyKind::Sheet,
        Ix::new(1)?,
        &mut WorkMeter::new(Budget::UNLIMITED),
    )
    .ok()
}

/// A first version of [`faces`] and that body with one row of its last chunk moved to the other
/// surface, per call of `BodyVersion::next`.
#[must_use]
pub fn next_version(class: Class) -> Vec<(BodyVersion, Body)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let Some(body) = faces() else {
        return Vec::new();
    };
    let Ok(first) = BodyVersion::first(&body, &mut WorkMeter::new(Budget::UNLIMITED)) else {
        return Vec::new();
    };
    let mut g = rng("format::next_version", class);
    batch_of(|_| {
        // Any row past the first chunk, which every geom-topo column cuts at 4,096 rows.
        let row = 4_096 + (g.next_u64() % u64::from(FACES - 4_096)) as u32;
        let mut arenas = body.arenas().clone();
        arenas.faces.surface.set(Ix::new(row)?, Ix::new(1)?).ok()?;
        let edited = Builder { arenas }
            .publish(
                body.kind(),
                body.outside(),
                &mut WorkMeter::new(Budget::UNLIMITED),
            )
            .ok()?;
        Some((first.clone(), edited))
    })
}
