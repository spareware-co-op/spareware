//! Attack tests from solidcore's kernel-case corpus: primitives and transforms, `geom-make`.
//!
//! The corpus is `crates/solidcore/tests/kernel_cases/` in solidcore (tangency-labs/solidcore,
//! Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70). The entries here are the ones
//! whose operation is a kernel primitive or transform and whose answer is a closed-form value or
//! a refusal: a cube scaled twice about its far corner, a scale factor of zero, a cone with no
//! height. Each test cites its entry as `<area>/<case>.json`. One millimetre is
//! [`QUARTER_NM_PER_MM`] grid steps.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::doc_markdown,
    reason = "test code: a failure should stop loudly"
)]

use geom_check::{Level, check};
use geom_make::{BoxInput, Moved, RoundInput, cone, make_box, scale};
use geom_math::Scale;
use geom_num::grid::QUARTER_NM_PER_MM;
use geom_num::{Exactness, GridStep, NumError, P3, Steps};
use geom_op::{MakeDetail, OpContext, OpErrorDetail, OpId, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_topo::Body;
use geom_topo::query::shell_volume;

const MM: i64 = QUARTER_NM_PER_MM as i64;

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn sixfold(body: &Body) -> i128 {
    let mut total = 0i128;
    for row in 0..body.arenas().shells.len() {
        let shell = geom_topo::ShellIx::new(row).unwrap();
        total += shell_volume(body, shell, &mut meter())
            .unwrap()
            .to_i128()
            .unwrap();
    }
    total
}

/// solids/cov-scale-uniform-about-a-corner.json: a 10 mm cube scaled 2x about its far corner
/// (10, 10, 10) is the cube from (-10, -10, -10) to (10, 10, 10): volume 8000 mm^3 and still
/// 6 faces, 12 edges and 8 vertices, the same as a cube made there, and passing the full check.
#[test]
fn solids_a_cube_scaled_twice_about_its_far_corner_is_the_cube_of_twice_the_size() {
    let cube = make_box(
        &ctx(),
        &BoxInput {
            min: P3::new(0, 0, 0).unwrap(),
            size: [Steps((10 * MM) as u64); 3],
        },
    )
    .unwrap();
    let doubled = scale(
        &ctx(),
        Moved {
            body: &cube.result.body,
            exactness: cube.exactness,
            accept: None,
        },
        [Scale::new(2, 1).unwrap(); 3],
        P3::new(10 * MM, 10 * MM, 10 * MM).unwrap(),
    )
    .unwrap()
    .result;
    let body = &doubled.body;
    let report = check(body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{report:?}");
    // 6 x 8000 mm^3, in cubic steps.
    assert_eq!(sixfold(body), 6 * 8000 * i128::from(MM).pow(3));
    let a = body.arenas();
    assert_eq!(
        (
            a.vertices.len() as usize,
            a.edges.len() as usize,
            a.faces.len() as usize
        ),
        (8, 12, 6)
    );
    // The same body as a cube made at (-10, -10, -10), 20 on a side.
    let made = make_box(
        &ctx(),
        &BoxInput {
            min: P3::new(-10 * MM, -10 * MM, -10 * MM).unwrap(),
            size: [Steps((20 * MM) as u64); 3],
        },
    )
    .unwrap();
    assert_eq!(sixfold(&made.result.body), sixfold(body));
}

/// solids/cov-scale-zero-factor-refused.json: a scale factor of 0 would flatten the body to a
/// sheet; the corpus entry expects a refusal ("scale factors must be positive"). Ours refuses
/// when the factor is made, with `NumError::Degenerate`, for a zero numerator or denominator,
/// so no scale operation can be asked to flatten a body.
#[test]
fn solids_a_scale_factor_of_zero_is_refused_when_it_is_made() {
    assert_eq!(Scale::new(0, 1).unwrap_err(), NumError::Degenerate);
    assert_eq!(Scale::new(1, 0).unwrap_err(), NumError::Degenerate);
    assert_eq!(Scale::new(0, 0).unwrap_err(), NumError::Degenerate);
}

/// solids/cov-cone-zero-height-refused.json: a cone of height 0 has no volume; the entry expects a
/// refusal ("height must not be zero"). Ours refuses with `MakeDetail::ZeroLength` naming the
/// height, and the same for a radius of zero.
#[test]
fn solids_a_cone_of_height_zero_is_refused() {
    let input = |radius: u64, height: u64| RoundInput {
        base: P3::new(0, 0, 0).unwrap(),
        radius: Steps(radius),
        height: Steps(height),
        step: GridStep::Q0,
    };
    let e = cone(&ctx(), &input(10 * MM as u64, 0)).unwrap_err();
    assert!(matches!(
        e.detail(),
        OpErrorDetail::Make(MakeDetail::ZeroLength { input: "height" })
    ));
    let e = cone(&ctx(), &input(0, 30 * MM as u64)).unwrap_err();
    assert!(matches!(
        e.detail(),
        OpErrorDetail::Make(MakeDetail::ZeroLength { input: "radius" })
    ));
}
