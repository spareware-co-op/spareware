//! Grid test data: coordinates and points on `geom-num`'s grid, aimed at the hard cases.
//!
//! What it does: a [`GridSampler`] draws coordinates, 2D points and 3D points from any
//! [`Entropy`]: a seeded [`crate::SplitMix64`] in a property test, or a fuzzer's bytes through
//! [`crate::ByteSource`] in a fuzz target. It remembers what it drew, so later draws can repeat or
//! nearly repeat earlier ones.
//!
//! Guarantees:
//!
//! - **Always on the grid.** Every value is a valid [`Coord`], magnitude at most
//!   [`COORD_MAX`] = 2^40 − 1, whatever the entropy, so any input decodes to a legal test case.
//! - **Everything reachable.** Every grid coordinate, and so every grid point, can be drawn.
//! - **The same entropy gives the same values** on every target: integer arithmetic only, one
//!   fixed decision order, and no hash maps.
//! - **One entropy word per decision at most.** No draw loops or rejects, so a fuzzer's bytes map to
//!   a test case in one pass, and a short input simply reads zeros at its end.
//!
//! Method: the generators of section 28.5 of the kernel architecture, "aimed at the hard cases"
//! (`projects/engines/geometry/architecture/28-testing.md`), after `SolidCraft`'s fuzzer
//! (evidence report 11, `projects/kit/notes/G01-evidence/11-vetting-suggested-projects.md`,
//! `SolidCraft`), with Manifold's fuzzer as the counter-example whose coordinates stay between 0.1
//! and 10 (evidence report 01, `projects/kit/notes/G01-evidence/01-manifold-mesh-booleans.md`,
//! section 4). Each coordinate is one of: full range (2 in 8 draws); small, in −16..=16;
//! one of the extremes ±(2^40 − 1), ±(2^40 − 2), 0 and ±1; snapped to a coarse sub-grid (1 mm,
//! 2.5 mm, 0.01 mm at the building step, 1 mm at the mechanical step, 2^20 steps, 625 steps) so
//! coincident faces and shared edges are common; snapped and then moved by exactly one step, so
//! near-coincidences are common; a repeat of an earlier coordinate; or an earlier coordinate moved
//! by one step. Each point is fresh (half the draws), a repeat of an earlier point, an earlier point
//! moved one step along one axis, a point on the line through the two latest points (so the next
//! point of a triangle is often collinear with the first two), or a point on the line through two
//! earlier points. A point on a line is exactly on it while it stays in the frame.
//!
//! Full-range values take one 64-bit word modulo 2^41 − 1, so a value is at most 2^-23 more likely
//! than another; that slight unevenness is harmless for test data, and it keeps every draw to one
//! word, which a fuzzer's bytes need.
//!
//! Arithmetic: every operation on a test value is a saturating `i128` method, and the result is
//! clamped to the frame. No operand comes near `i128`'s limits: a full-range value is a word's
//! remainder, below 2^41; a coordinate has magnitude at most 2^40 − 1; a step is ±1, a line factor
//! at most 3 in magnitude and a sub-grid at most 2^20. So every intermediate value is below 2^44
//! in magnitude (the largest, a + t·(b − a), is at most 2^40 + 3 · 2^41), nothing saturates, and
//! every value before the clamp is exact. The saturating methods need no unreachable branch, and
//! no native operator is used, so nothing depends on overflow checks being on.
//!
//! Cost: constant per draw. The memory of earlier draws is capped at [`HISTORY`] coordinates and
//! [`HISTORY`] points of each dimension, overwritten in a fixed rotation.

use geom_num::grid::{COORD_MAX, Coord, P2, P3};

use crate::rng::Entropy;

/// How many earlier coordinates, and earlier points of each dimension, a sampler remembers.
pub const HISTORY: usize = 64;

/// The extremes a coordinate may take: the edges of the frame, one step inside them, and the
/// smallest values.
pub const EXTREMES: [i64; 7] = [
    COORD_MAX,
    -COORD_MAX,
    COORD_MAX - 1,
    -COORD_MAX + 1,
    0,
    1,
    -1,
];

/// The coarse sub-grids, in grid steps: 1 mm, 2.5 mm and 0.01 mm (the fit bound) on the building
/// step Q0 (1/6400 mm); 1 mm on the mechanical step Q1 (1/160,000 mm); a power of two; and 625
/// steps, which is 25 × 25, so coarsening by two rungs is exact.
pub const SUBGRIDS: [i64; 6] = [6_400, 16_000, 64, 160_000, 1 << 20, 625];

/// Whether every value is positive: a sub-grid snaps a value by truncation to a multiple of it,
/// which needs a positive divisor.
const fn all_positive(values: &[i64]) -> bool {
    match values {
        [] => true,
        [first, rest @ ..] => *first > 0 && all_positive(rest),
    }
}

// Every sub-grid is positive, so [`snapped`]'s remainder always exists.
const _: () = assert!(all_positive(&SUBGRIDS));

/// How many values the frame holds, 2^41 − 1: every coordinate from −(2^40 − 1) to 2^40 − 1. A
/// constant, so the compiler evaluates it, and an overflow would stop the build.
const FRAME_VALUES: u64 = 2 * COORD_MAX.unsigned_abs() + 1;

/// Draws grid coordinates and points aimed at the hard cases, remembering what it drew.
#[derive(Clone, Debug, Default)]
pub struct GridSampler {
    coords: Vec<i64>,
    coords_drawn: usize,
    points2: Vec<P2>,
    points2_drawn: usize,
    points3: Vec<P3>,
    points3_drawn: usize,
}

impl GridSampler {
    /// A sampler with no memory of earlier draws.
    #[must_use]
    pub fn new() -> GridSampler {
        GridSampler::default()
    }

    /// A coordinate, by one of the eight rules in the module header.
    pub fn coord(&mut self, e: &mut dyn Entropy) -> Coord {
        let rule = e.byte() % 8;
        let v: i128 = match rule {
            0 | 1 => full_range(e),
            2 => i128::from(e.byte() % 33).saturating_sub(16),
            3 => i128::from(pick(&EXTREMES, e.byte()).unwrap_or(0)),
            4 => snapped(e),
            5 => snapped(e).saturating_add(step(e)),
            6 => i128::from(pick(&self.coords, e.byte()).unwrap_or(0)),
            _ => i128::from(pick(&self.coords, e.byte()).unwrap_or(0)).saturating_add(step(e)),
        };
        let c = clamp(v);
        remember(&mut self.coords, &mut self.coords_drawn, c.get());
        c
    }

    /// A 2D point, by one of the point rules in the module header.
    pub fn p2(&mut self, e: &mut dyn Entropy) -> P2 {
        let rule = e.byte() % 8;
        let p = match rule {
            4 => pick(&self.points2, e.byte()),
            5 => pick(&self.points2, e.byte()).map(|p| {
                let d = step(e);
                if e.byte() % 2 == 0 {
                    P2 {
                        x: moved(p.x, d),
                        y: p.y,
                    }
                } else {
                    P2 {
                        x: p.x,
                        y: moved(p.y, d),
                    }
                }
            }),
            6 | 7 => {
                let (a, b) = if rule == 6 {
                    (
                        recent(&self.points2, self.points2_drawn, 1),
                        recent(&self.points2, self.points2_drawn, 0),
                    )
                } else {
                    (pick(&self.points2, e.byte()), pick(&self.points2, e.byte()))
                };
                let t = factor(e);
                a.zip(b).map(|(a, b)| P2 {
                    x: along(a.x, b.x, t),
                    y: along(a.y, b.y, t),
                })
            }
            _ => None,
        };
        // A rule that needs earlier points draws a fresh point until there are enough.
        let p = match p {
            Some(p) => p,
            None => P2 {
                x: self.coord(e),
                y: self.coord(e),
            },
        };
        remember(&mut self.points2, &mut self.points2_drawn, p);
        p
    }

    /// A 3D point, by one of the point rules in the module header.
    pub fn p3(&mut self, e: &mut dyn Entropy) -> P3 {
        let rule = e.byte() % 8;
        let p = match rule {
            4 => pick(&self.points3, e.byte()),
            5 => pick(&self.points3, e.byte()).map(|p| {
                let d = step(e);
                match e.byte() % 3 {
                    0 => P3 {
                        x: moved(p.x, d),
                        ..p
                    },
                    1 => P3 {
                        y: moved(p.y, d),
                        ..p
                    },
                    _ => P3 {
                        z: moved(p.z, d),
                        ..p
                    },
                }
            }),
            6 | 7 => {
                let (a, b) = if rule == 6 {
                    (
                        recent(&self.points3, self.points3_drawn, 1),
                        recent(&self.points3, self.points3_drawn, 0),
                    )
                } else {
                    (pick(&self.points3, e.byte()), pick(&self.points3, e.byte()))
                };
                let t = factor(e);
                a.zip(b).map(|(a, b)| P3 {
                    x: along(a.x, b.x, t),
                    y: along(a.y, b.y, t),
                    z: along(a.z, b.z, t),
                })
            }
            _ => None,
        };
        let p = match p {
            Some(p) => p,
            None => P3 {
                x: self.coord(e),
                y: self.coord(e),
                z: self.coord(e),
            },
        };
        remember(&mut self.points3, &mut self.points3_drawn, p);
        p
    }
}

/// A value anywhere in −(2^40 − 1)..=2^40 − 1, from one word. The remainder r of the word modulo
/// 2^41 − 1 maps to r for r ≤ 2^40 − 1 and to 2^40 − 1 − r above, a bijection onto the frame that
/// sends small words to small values, so a fuzzer's zero bytes decode to zero.
fn full_range(e: &mut dyn Entropy) -> i128 {
    let m = i128::from(COORD_MAX);
    // The remainder modulo the constant 2^41 − 1 is at most 2^41 − 2, so m − r ≥ −m.
    let r = i128::from(e.word() % FRAME_VALUES);
    if r <= m { r } else { m.saturating_sub(r) }
}

/// A full-range value truncated towards zero to a multiple of one of [`SUBGRIDS`]. Truncation
/// never increases a magnitude, so the result stays in range.
fn snapped(e: &mut dyn Entropy) -> i128 {
    let s = i128::from(pick(&SUBGRIDS, e.byte()).unwrap_or(1));
    let v = full_range(e);
    // Every sub-grid is positive (checked when the crate compiles), so the remainder exists and
    // `checked_rem` is never `None`; were it, `v` would stay unsnapped, still a value in the frame.
    v.checked_rem(s).map_or(v, |rest| v.saturating_sub(rest))
}

/// One grid step, up or down.
fn step(e: &mut dyn Entropy) -> i128 {
    if e.byte() % 2 == 0 { 1 } else { -1 }
}

/// A line factor t in −3..=3 for [`along`].
fn factor(e: &mut dyn Entropy) -> i128 {
    i128::from(e.byte() % 7).saturating_sub(3)
}

/// `c` moved by `d` steps, clamped to the frame.
fn moved(c: Coord, d: i128) -> Coord {
    clamp(i128::from(c.get()).saturating_add(d))
}

/// a + t·(b − a), exactly (module header, "Arithmetic"), clamped to the frame. While it is in
/// range it lies exactly on the line through `a` and `b`.
fn along(a: Coord, b: Coord, t: i128) -> Coord {
    let (a, b) = (i128::from(a.get()), i128::from(b.get()));
    clamp(a.saturating_add(t.saturating_mul(b.saturating_sub(a))))
}

/// The value clamped to the frame, ±(2^40 − 1), as a coordinate.
fn clamp(v: i128) -> Coord {
    let (lowest, highest) = (i128::from(Coord::MIN.get()), i128::from(Coord::MAX.get()));
    // The clamp puts the value in range, so `from_i128` cannot fail; library code may not panic, so
    // the impossible error maps to zero rather than to an `expect`.
    Coord::from_i128(v.clamp(lowest, highest)).unwrap_or(Coord::ZERO)
}

/// The entry of `items` that `selector` names (modulo the length), or `None` if it is empty.
fn pick<T: Copy>(items: &[T], selector: u8) -> Option<T> {
    // `checked_rem` is `None` exactly when the length is zero: an empty memory has nothing to pick.
    items
        .get(usize::from(selector).checked_rem(items.len())?)
        .copied()
}

/// The `k`-th most recent entry of a memory [`remember`] fills (`k` = 0 is the latest), or `None`
/// if fewer than `k + 1` entries were drawn. Draw number j is stored in slot j mod [`HISTORY`].
fn recent<T: Copy>(memory: &[T], drawn: usize, k: usize) -> Option<T> {
    if k >= HISTORY {
        return None;
    }
    // The latest draw is number drawn − 1, so the k-th most recent is drawn − 1 − k: `None`
    // exactly when fewer than k + 1 were drawn.
    let number = drawn.checked_sub(k)?.checked_sub(1)?;
    memory.get(number % HISTORY).copied()
}

/// Stores `v` in a memory of at most [`HISTORY`] entries, overwriting the oldest once it is full.
fn remember<T>(memory: &mut Vec<T>, drawn: &mut usize, v: T) {
    if memory.len() < HISTORY {
        memory.push(v);
    } else if let Some(slot) = memory.get_mut(*drawn % HISTORY) {
        *slot = v;
    }
    *drawn = drawn.wrapping_add(1);
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;
    use crate::rng::{ByteSource, SplitMix64};
    use crate::seed::{Seed, fixed};
    use geom_num::predicate::orient2d;

    #[test]
    fn every_rule_is_reached_and_every_value_is_on_the_grid() {
        let mut rng = SplitMix64::new(fixed("grid.rs unit test", Seed::DEFAULT));
        let mut s = GridSampler::new();
        let (mut extremes, mut small, mut big, mut repeats) = (0, 0, 0, 0);
        let mut seen = Vec::new();
        for _ in 0..20_000 {
            let c = s.coord(&mut rng).get();
            assert!(c.abs() <= COORD_MAX);
            extremes += usize::from(c.abs() >= COORD_MAX - 1);
            small += usize::from(c.abs() <= 16);
            big += usize::from(c.abs() > 1 << 39);
            repeats += usize::from(seen.contains(&c));
            seen.push(c);
            if seen.len() > 64 {
                seen.remove(0);
            }
        }
        assert!(extremes > 500, "{extremes}");
        assert!(small > 2_000, "{small}");
        assert!(big > 2_000, "{big}");
        assert!(repeats > 2_000, "{repeats}");
    }

    #[test]
    fn points_repeat_move_and_line_up() {
        let mut rng = SplitMix64::new(fixed("grid.rs unit test", Seed(42)));
        let mut s = GridSampler::new();
        let mut pts = Vec::new();
        for _ in 0..5_000 {
            pts.push(s.p2(&mut rng));
        }
        let repeats = (1..pts.len())
            .filter(|&i| pts[i.saturating_sub(HISTORY)..i].contains(&pts[i]))
            .count();
        assert!(repeats > 300, "{repeats}");
        let one_step = (1..pts.len())
            .filter(|&i| {
                pts[i.saturating_sub(HISTORY)..i].iter().any(|q| {
                    let dx = (q.x.get() - pts[i].x.get()).abs();
                    let dy = (q.y.get() - pts[i].y.get()).abs();
                    dx + dy == 1
                })
            })
            .count();
        assert!(one_step > 300, "{one_step}");
        // Collinear triples among consecutive draws show the line rule is live.
        let collinear = pts
            .windows(3)
            .filter(|w| orient2d(w[0], w[1], w[2]).unwrap().is_zero())
            .count();
        assert!(collinear > 100, "{collinear}");
        let mut s3 = GridSampler::new();
        for _ in 0..5_000 {
            let p = s3.p3(&mut rng);
            for c in [p.x, p.y, p.z] {
                assert!(c.get().abs() <= COORD_MAX);
            }
        }
    }

    /// The golden hash of 3,000 draws from the default seed, over every coordinate's little-endian
    /// bytes: SHA-256, in the golden store as `geom-test/grid-sampler` (piece G19), and the 64-bit
    /// FNV-1a digest G11 first pinned, `FNV_GOLDEN`, kept to show the bytes are the ones it was
    /// fixed on. CI runs this on every platform and build mode and under WebAssembly (the golden
    /// matrix), so a seed gives the same test data on every target. A change here changes every
    /// seeded test's inputs: it must be deliberate and recorded in the store.
    #[test]
    fn the_same_seed_gives_the_same_data_on_every_target() {
        const FNV_GOLDEN: u64 = 0x478c_1099_7c52_44b6;
        let draws = || {
            let mut rng = SplitMix64::new(fixed("grid.rs unit test", Seed::DEFAULT));
            let mut s = GridSampler::new();
            let mut bytes = Vec::new();
            let mut eat = |c: Coord| bytes.extend_from_slice(&c.get().to_le_bytes());
            for _ in 0..1_000 {
                eat(s.coord(&mut rng));
                let p = s.p2(&mut rng);
                eat(p.x);
                eat(p.y);
                let q = s.p3(&mut rng);
                eat(q.x);
                eat(q.y);
                eat(q.z);
            }
            bytes
        };
        let checked = crate::golden::check("geom-test/grid-sampler", || {
            let bytes = draws();
            (crate::golden::sha256(&bytes), bytes)
        });
        let fnv = checked.value.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
        });
        assert_eq!(fnv, FNV_GOLDEN, "{fnv:#018x}");
        checked.verdict().unwrap_or_else(|e| panic!("{e}"));
    }

    #[test]
    fn the_same_entropy_gives_the_same_values() {
        let draw = |seed| {
            let mut rng = SplitMix64::new(fixed("grid.rs unit test", Seed(seed)));
            let mut s = GridSampler::new();
            (0..1_000).map(|_| s.p3(&mut rng)).collect::<Vec<_>>()
        };
        assert_eq!(draw(9), draw(9));
        assert_ne!(draw(9), draw(10));
    }

    #[test]
    fn any_bytes_decode_including_none() {
        let mut s = GridSampler::new();
        let p = s.p2(&mut ByteSource::new(&[]));
        assert_eq!(p, P2::new(0, 0).unwrap());
        let all_ff = [0xffu8; 64];
        let mut src = ByteSource::new(&all_ff);
        for _ in 0..10 {
            let p = s.p3(&mut src);
            assert!(p.x.get().abs() <= COORD_MAX);
        }
    }

    #[test]
    fn clamp_and_along_stay_in_the_frame() {
        assert_eq!(clamp(i128::MAX).get(), COORD_MAX);
        assert_eq!(clamp(i128::MIN).get(), -COORD_MAX);
        assert_eq!(clamp(5).get(), 5);
        let (a, b) = (Coord::MAX, Coord::MIN);
        assert_eq!(along(a, b, 3).get(), -COORD_MAX);
        assert_eq!(along(a, b, -3).get(), COORD_MAX);
        assert_eq!(
            along(Coord::new(2).unwrap(), Coord::new(5).unwrap(), 2).get(),
            8
        );
    }

    #[test]
    fn memory_is_capped_and_rotates() {
        let mut memory = Vec::new();
        let mut drawn = 0;
        for v in 0..(HISTORY + 3) {
            remember(&mut memory, &mut drawn, v);
        }
        assert_eq!(memory.len(), HISTORY);
        assert_eq!(&memory[..3], &[HISTORY, HISTORY + 1, HISTORY + 2]);
        assert_eq!(memory[3], 3);
    }
}
