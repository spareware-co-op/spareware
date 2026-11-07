//! The step mix of chapter 23.2: which step of the predicate ladder settled each call, per corpus.
//!
//! What it does: [`mix`] runs the `_step` twins of `geom-num`'s predicates over three corpora and
//! tallies the steps: buildings (a plan grid of bays and storeys, with axis planes and a few at 45
//! degrees), mechanical parts (tilted planes through points within a metre) and near-degenerate
//! fuzz input (points one step off a line, and vertices on planes that are not their own).
//!
//! Guarantees: the corpora come from fixed seeds and the twins are pure, so the counts are the same
//! on every target and in every build profile, except that `orient2d` skips its filter on some
//! targets ([`geom_num::predicate::orient2d`]), which moves its calls between the filter and the
//! exact step. Counts are statistics only, not the work units of 23.10.
//!
//! Method: the counts chapter 23.2 asks for, over `geom-num`'s `_step` twins
//! (`projects/engines/geometry/architecture/23-performance.md`); SplitMix64 draws the corpora
//! ([`crate::rng`]), from Steele, Lea and Flood 2014, <https://doi.org/10.1145/2660193.2660195>.
//!
//! Cost: a few thousand predicate calls per corpus.

use geom_num::plane::{Plane, Tpi};
use geom_num::predicate::{orient2d_step, orient3d_step};
use geom_num::{P2, P3, Sign, Step};
use geom_poly::plane::PlaneTable;

use crate::rng::{SplitMix64, seed_for};

/// Grid steps in a metre at q0.
const METRE: i64 = 6_400_000;

/// A bay of a building's plan: 3.6 m.
const BAY: i64 = 3 * METRE + 3 * METRE / 5;

/// A storey: 3 m.
const STOREY: i64 = 3 * METRE;

/// Calls of each kind in a corpus.
const CALLS: usize = 2_000;

/// A corpus, and how many of its calls each step settled, in the order of [`Step::ALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mix {
    /// The corpus's name.
    pub corpus: &'static str,
    /// The calls each step settled.
    pub counts: [u64; 5],
}

impl Mix {
    /// All the calls of the corpus.
    #[must_use]
    pub fn calls(&self) -> u64 {
        self.counts.iter().sum()
    }

    /// The calls `step` settled.
    #[must_use]
    pub fn settled_by(&self, step: Step) -> u64 {
        Step::ALL
            .iter()
            .zip(self.counts)
            .find_map(|(s, count)| (*s == step).then_some(count))
            .unwrap_or(0)
    }

    /// Adds one call to `step`'s count, if the call was answered.
    fn add<E>(&mut self, answer: Result<(Sign, Step), E>) {
        let Ok((_, step)) = answer else {
            return;
        };
        for (s, count) in Step::ALL.iter().zip(&mut self.counts) {
            if *s == step {
                *count = count.saturating_add(1);
            }
        }
    }
}

/// The plane where coordinate `axis` is `at`, in canonical form.
fn axis_plane(axis: usize, at: i64) -> Option<Plane> {
    let n = match axis {
        0 => [1, 0, 0],
        1 => [0, 1, 0],
        _ => [0, 0, 1],
    };
    Plane::new(n, i128::from(at).checked_neg()?).ok()
}

/// A plan corner: `bays` along and `rows` across.
fn plan(g: &mut SplitMix64) -> Option<P2> {
    P2::new(
        g.range(0, 8).checked_mul(BAY)?,
        g.range(0, 6).checked_mul(BAY)?,
    )
    .ok()
}

/// A corner of the frame of bays and storeys.
fn frame(g: &mut SplitMix64) -> Option<P3> {
    P3::new(
        g.range(0, 8).checked_mul(BAY)?,
        g.range(0, 6).checked_mul(BAY)?,
        g.range(0, 5).checked_mul(STOREY)?,
    )
    .ok()
}

/// The plane of a wall or a floor, one in eight of them at 45 degrees.
fn building_plane(g: &mut SplitMix64) -> Option<Plane> {
    let at = g.range(0, 8).checked_mul(BAY)?;
    match g.below(8) {
        0 => Plane::new([1, 1, 0], i128::from(at).checked_neg()?).ok(),
        1..=3 => axis_plane(0, at),
        4..=6 => axis_plane(1, g.range(0, 6).checked_mul(BAY)?),
        _ => axis_plane(2, g.range(0, 5).checked_mul(STOREY)?),
    }
}

/// A vertex of the frame, where a wall, a cross wall and a floor meet.
fn building_vertex(g: &mut SplitMix64) -> Option<Tpi> {
    Tpi::new(
        axis_plane(0, g.range(0, 8).checked_mul(BAY)?)?,
        axis_plane(1, g.range(0, 6).checked_mul(BAY)?)?,
        axis_plane(2, g.range(0, 5).checked_mul(STOREY)?)?,
    )
    .ok()
}

/// The corpus of a building.
fn buildings() -> Mix {
    let mut g = SplitMix64::new(seed_for("steps", "buildings"));
    let mut mix = Mix {
        corpus: "buildings",
        counts: [0; 5],
    };
    let mut table = PlaneTable::new();
    for _ in 0..CALLS {
        if let (Some(a), Some(b), Some(c)) = (plan(&mut g), plan(&mut g), plan(&mut g)) {
            mix.add(orient2d_step(a, b, c));
        }
        if let (Some(p), Some(q)) = (building_plane(&mut g), frame(&mut g))
            && let Ok(held) = table.insert(&p)
        {
            mix.add(table.side_of_point(held.plane, q));
        }
        if let Some(v) = building_vertex(&mut g) {
            let own = v.planes().get(g.index(3)).copied();
            if let Some(s) = own {
                mix.add(s.side_of_tpi_step(&v));
            }
            if let Some(s) = building_plane(&mut g) {
                mix.add(s.side_of_tpi_step(&v));
            }
        }
        if let (Some(a), Some(b), Some(c), Some(d)) =
            (frame(&mut g), frame(&mut g), frame(&mut g), frame(&mut g))
        {
            mix.add(orient3d_step(a, b, c, d));
        }
    }
    mix
}

/// A point within a metre of the origin.
fn near(g: &mut SplitMix64) -> Option<P3> {
    P3::new(
        g.range(-METRE, METRE),
        g.range(-METRE, METRE),
        g.range(-METRE, METRE),
    )
    .ok()
}

/// A plane of a part, through three points within a metre.
fn part_plane(g: &mut SplitMix64) -> Option<Plane> {
    Plane::through(near(g)?, near(g)?, near(g)?).ok()
}

/// The corpus of mechanical parts.
fn parts() -> Mix {
    let mut g = SplitMix64::new(seed_for("steps", "parts"));
    let mut mix = Mix {
        corpus: "mechanical parts",
        counts: [0; 5],
    };
    let mut table = PlaneTable::new();
    for _ in 0..CALLS {
        if let (Some(p), Some(q)) = (part_plane(&mut g), near(&mut g))
            && let Ok(held) = table.insert(&p)
        {
            mix.add(table.side_of_point(held.plane, q));
        }
        if let (Some(a), Some(b), Some(c), Some(d)) =
            (near(&mut g), near(&mut g), near(&mut g), near(&mut g))
        {
            mix.add(orient3d_step(a, b, c, d));
        }
        let planes = (
            part_plane(&mut g),
            part_plane(&mut g),
            part_plane(&mut g),
            part_plane(&mut g),
        );
        if let (Some(p), Some(q), Some(r), Some(s)) = planes
            && let Ok(v) = Tpi::new(p, q, r)
        {
            mix.add(s.side_of_tpi_step(&v));
            mix.add(p.side_of_tpi_step(&v));
        }
    }
    mix
}

/// Three steep planes through `at` and a fourth plane of `axis` a step or none from it.
fn near_vertex(g: &mut SplitMix64, at: [i64; 3]) -> Option<(Plane, Tpi)> {
    let [x, y, z] = at;
    let plane = |g: &mut SplitMix64| {
        let mut far = || {
            let [dx, dy, dz] = [(); 3].map(|()| g.range(-METRE, METRE));
            P3::new(x.checked_add(dx)?, y.checked_add(dy)?, z.checked_add(dz)?).ok()
        };
        Plane::through(P3::new(x, y, z).ok()?, far()?, far()?).ok()
    };
    let vertex = Tpi::new(plane(g)?, plane(g)?, plane(g)?).ok()?;
    let off = g.range(-1, 1);
    let s = match g.below(3) {
        0 => axis_plane(0, x.checked_add(off)?),
        1 => axis_plane(1, y.checked_add(off)?),
        _ => axis_plane(2, z.checked_add(off)?),
    }?;
    Some((s, vertex))
}

/// The corpus of near-degenerate fuzz input.
fn near_degenerate() -> Mix {
    let mut g = SplitMix64::new(seed_for("steps", "near-degenerate"));
    let mut mix = Mix {
        corpus: "near-degenerate",
        counts: [0; 5],
    };
    let wide = 1 << 38;
    for _ in 0..CALLS {
        let (x, y) = (g.range(-wide, wide), g.range(-wide, wide));
        let t = g.range(1, 7);
        let off = g.range(-1, 1);
        // On the line from the origin through (x, y) / 8, or a step off it.
        if let (Ok(a), Ok(b), Ok(c)) = (
            P2::new(0, 0),
            P2::new(x, y),
            P2::new((x / 8) * t, ((y / 8) * t).saturating_add(off)),
        ) {
            mix.add(orient2d_step(a, b, c));
        }
        let at = [
            g.range(-METRE, METRE),
            g.range(-METRE, METRE),
            g.range(-METRE, METRE),
        ];
        if let Some((s, v)) = near_vertex(&mut g, at) {
            mix.add(s.side_of_tpi_step(&v));
        }
    }
    mix
}

/// The step mix of every corpus.
#[must_use]
pub fn mix() -> Vec<Mix> {
    vec![buildings(), parts(), near_degenerate()]
}
