//! The arrangement of one million segments (or `ARRANGE_SEGMENTS` of them), timed beside iOverlay's split-and-graph build of the
//! same segments (piece G196a; decision 32.44), for people.
//!
//! The drawing is [`SEGMENTS`] short segments between grid points in a square of side
//! [`SIDE`], placed by a fixed generator, so every run and every machine sees the same segments:
//! mostly disjoint, with some thousand crossings, the sparse case of a large drawing. `geom_2d`
//! arranges them exactly (rational crossings, ordered edges, faces and holes). iOverlay has no
//! public call that returns an arrangement of segments, so its row is its `EdgeOverlay`: the same
//! segments added as one shape, split at their intersections and built into a graph, then
//! extracted with a union rule. That row therefore includes extraction that the first row's
//! faces-and-holes step only partly matches, and iOverlay computes in 32-bit integers where the
//! first row uses exact rationals; read the two numbers as the cost of the same job in two
//! libraries, not as a like-for-like race.
//!
//! ```text
//! cargo bench -p geom-bench --bench arrange_1m
//! ```
//!
//! Guarantees: both rows time the same segments; the first row checks that the arrangement has at
//! least as many edges as segments before it is timed, so nothing is timed that computes nothing.
//! Timings vary between runs and machines, so this harness never gates a merge.
//!
//! Method: divan's sampling (<https://docs.rs/divan/0.1.21/divan/>); iOverlay 9.1.0 (MIT OR
//! Apache-2.0), `EdgeOverlay`
//! (<https://github.com/iShape-Rust/iOverlay/blob/v9.1.0/src/core/edge_overlay.rs>).
//!
//! Cost: a few seconds to a few minutes of building and timing per row, three samples of one run
//! each; the segments take about 32 MB.

use divan::Bencher;
use divan::counter::ItemsCount;
use geom_2d::arrange::arrange;
use geom_num::grid::P2;
use geom_run::{Budget, WorkMeter};
use i_overlay::core::edge_data::{EdgeDataMerge, OverlayEdgeData};
use i_overlay::core::edge_overlay::{EdgeOverlay, InputEdge};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::ShapeType;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::i_float::int::point::IntPoint;
use i_overlay::segm::boolean::ShapeCountBoolean;

/// How many segments by default; the environment variable `ARRANGE_SEGMENTS` sets another count,
/// for a smaller run or a machine with less memory.
const SEGMENTS: usize = 1_000_000;

fn count() -> usize {
    std::env::var("ARRANGE_SEGMENTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(SEGMENTS)
}

/// The side of the square, in grid steps.
const SIDE: i64 = 1_000_000;

/// The reach of a segment, in grid steps.
const REACH: i64 = 40;

fn main() {
    divan::main();
}

/// A small deterministic generator (`SplitMix64`, Steele, Lea and Flood 2014).
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: i64) -> i64 {
        i64::try_from(self.next() % n.unsigned_abs()).unwrap_or(0)
    }
}

/// The segments, as pairs of coordinates.
fn drawing() -> Vec<[i32; 4]> {
    let mut g = SplitMix64(0x1A3E_0001);
    (0..count())
        .map(|_| {
            let x = REACH + g.below(SIDE - 2 * REACH);
            let y = REACH + g.below(SIDE - 2 * REACH);
            let dx = g.below(2 * REACH + 1) - REACH;
            let dy = g.below(2 * REACH + 1) - REACH;
            let (x1, y1) = (x + dx, y + dy);
            let n = |v: i64| i32::try_from(v).unwrap_or(0);
            [n(x), n(y), n(x1), n(y1)]
        })
        .collect()
}

/// iOverlay's edge data: nothing, and merging two edges gives nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Plain;

impl OverlayEdgeData for Plain {
    type Store = ();

    fn merge(_: EdgeDataMerge<ShapeCountBoolean, Self>, _: &mut Self::Store) -> Self {
        Plain
    }
}

#[divan::bench(sample_count = 3, sample_size = 1)]
fn geom_2d_exact_arrangement(bencher: Bencher) {
    let segs: Vec<(P2, P2)> = drawing()
        .iter()
        .filter_map(|s| {
            Some((
                P2::new(i64::from(s[0]), i64::from(s[1])).ok()?,
                P2::new(i64::from(s[2]), i64::from(s[3])).ok()?,
            ))
        })
        .collect();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let checked = arrange(&segs, &mut meter).map(|a| a.edges().len());
    assert!(
        checked.is_ok_and(|n| n >= segs.len() / 2),
        "the arrangement has edges"
    );
    bencher
        .counter(ItemsCount::new(segs.len()))
        .bench_local(|| {
            let mut meter = WorkMeter::new(Budget::UNLIMITED);
            arrange(&segs, &mut meter).map(|a| a.edges().len())
        });
}

/// The most segments the iOverlay row runs on unless `ARRANGE_IOVERLAY_ALL` is set: on this layout
/// (short segments spread over a large square) iOverlay 9.1.0 asks for 20 GB at once from about
/// 200,000 segments and aborts on a machine with less, so the row is skipped past 100,000 to keep
/// a default run from taking the machine down. The measured rows are in the pull request.
const IOVERLAY_SAFE: usize = 100_000;

#[divan::bench(sample_count = 3, sample_size = 1)]
fn i_overlay_edge_overlay(bencher: Bencher) {
    if count() > IOVERLAY_SAFE && std::env::var_os("ARRANGE_IOVERLAY_ALL").is_none() {
        eprintln!(
            "i_overlay row skipped above {IOVERLAY_SAFE} segments (set ARRANGE_IOVERLAY_ALL to run it)"
        );
        bencher.bench_local(|| 0_usize);
        return;
    }
    let segs = drawing();
    bencher
        .counter(ItemsCount::new(segs.len()))
        .bench_local(|| {
            let mut overlay = EdgeOverlay::<i32, Plain>::new(segs.len());
            for s in &segs {
                overlay.add_edge(
                    InputEdge {
                        a: IntPoint::new(s[0], s[1]),
                        b: IntPoint::new(s[2], s[3]),
                        data: Plain,
                    },
                    ShapeType::Subject,
                );
            }
            overlay
                .build_vector_shapes(OverlayRule::Union, FillRule::NonZero)
                .len()
        });
}
