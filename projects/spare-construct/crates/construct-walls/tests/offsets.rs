//! Total thickness, the depth of every layer face, and the offsets of every face and every
//! location line from the reference line, for every default type and every location line.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{finish, hmm, hmms, id, mm, structure};
use construct_materials::MaterialId;
use construct_params::{Definition, Holder, Kind, Length, Units, Value};
use construct_walls::{Assembly, Function, Layer, LocationLine, WallError, default_walls};

use LocationLine::{
    ExteriorCoreFace, ExteriorFinishFace, InteriorCoreFace, InteriorFinishFace, WallCentre,
};

/// The default type with this ID.
fn default(wall: &str) -> Assembly {
    default_walls()
        .unwrap()
        .into_iter()
        .find(|d| d.id == wall)
        .unwrap()
        .assembly
}

/// Each default type: its face depths, its six location line depths in the order of
/// `LocationLine::ALL`, and the face offsets with each location line on the reference line.
/// Every number is in half millimetres, because the partition's linings are 12.5 mm.
struct Expected {
    wall: &'static str,
    total: i64,
    depths: &'static [i64],
    lines: [i64; 6],
    faces: [&'static [i64]; 6],
}

const EXPECTED: [Expected; 3] = [
    Expected {
        wall: "wall-partition-stud-90",
        total: 230,
        depths: &[0, 25, 205, 230],
        lines: [115, 115, 0, 230, 25, 205],
        faces: [
            &[115, 90, -90, -115],
            &[115, 90, -90, -115],
            &[0, -25, -205, -230],
            &[230, 205, 25, 0],
            &[25, 0, -180, -205],
            &[205, 180, 0, -25],
        ],
    },
    Expected {
        wall: "wall-concrete-200",
        total: 400,
        depths: &[0, 400],
        lines: [200, 200, 0, 400, 0, 400],
        faces: [
            &[200, -200],
            &[200, -200],
            &[0, -400],
            &[400, 0],
            &[0, -400],
            &[400, 0],
        ],
    },
    Expected {
        wall: "wall-brick-veneer-90",
        total: 500,
        depths: &[0, 220, 300, 300, 480, 500],
        lines: [250, 390, 0, 500, 300, 480],
        faces: [
            &[250, 30, -50, -50, -230, -250],
            &[390, 170, 90, 90, -90, -110],
            &[0, -220, -300, -300, -480, -500],
            &[500, 280, 200, 200, 20, 0],
            &[300, 80, 0, 0, -180, -200],
            &[480, 260, 180, 180, 0, -20],
        ],
    },
];

/// Every default type's total, face depths and location line depths.
#[test]
fn default_depths() {
    for expected in &EXPECTED {
        let wall = default(expected.wall);
        assert_eq!(wall.total(), hmm(expected.total), "{}", expected.wall);
        assert_eq!(wall.face_depths(), hmms(expected.depths).as_slice());
        for (line, depth) in LocationLine::ALL.iter().zip(expected.lines) {
            assert_eq!(
                wall.depth(*line),
                Some(hmm(depth)),
                "{} {line:?}",
                expected.wall
            );
        }
    }
}

/// Every default type, every location line on the reference line: every face offset, and every
/// location line's offset (the chosen line's depth less the line's depth).
#[test]
fn default_offsets_every_line() {
    for expected in &EXPECTED {
        let wall = default(expected.wall);
        for (on, faces) in LocationLine::ALL.iter().zip(expected.faces) {
            let offsets = wall.offsets(*on, false).unwrap();
            assert_eq!(
                offsets.faces(),
                hmms(faces).as_slice(),
                "{} on {on:?}",
                expected.wall
            );
            let on_depth = wall.depth(*on).unwrap();
            assert_eq!(offsets.line(*on), Some(mm(0)));
            for line in LocationLine::ALL {
                let want = on_depth.checked_sub(wall.depth(line).unwrap());
                assert_eq!(offsets.line(line), want, "{} {line:?}", expected.wall);
            }
        }
    }
}

/// Flipped puts the exterior on the other side: every offset is negated.
#[test]
fn flipped_negates_every_offset() {
    for expected in &EXPECTED {
        let wall = default(expected.wall);
        for on in LocationLine::ALL {
            let plain = wall.offsets(on, false).unwrap();
            let flipped = wall.offsets(on, true).unwrap();
            let negated: Vec<Length> = plain
                .faces()
                .iter()
                .map(|f| Length::from_mm(0).checked_sub(*f).unwrap())
                .collect();
            assert_eq!(flipped.faces(), negated.as_slice());
            for line in LocationLine::ALL {
                let want = plain
                    .line(line)
                    .and_then(|l| Length::from_mm(0).checked_sub(l));
                assert_eq!(flipped.line(line), want);
            }
        }
    }
}

/// The two faces of the reference line's side: with the exterior finish face on the line, every
/// other face lies toward the interior (negative); with the interior finish face, toward the
/// exterior (positive).
#[test]
fn sign_convention() {
    let wall = default("wall-brick-veneer-90");
    let exterior = wall.offsets(ExteriorFinishFace, false).unwrap();
    assert!(exterior.faces().iter().skip(1).all(|f| *f < mm(0)));
    let interior = wall.offsets(InteriorFinishFace, false).unwrap();
    assert!(interior.faces().iter().rev().skip(1).all(|f| *f > mm(0)));
    assert_eq!(exterior.line(InteriorCoreFace), Some(mm(-240)));
    assert_eq!(interior.line(ExteriorCoreFace), Some(mm(100)));
}

/// One step over 90 mm is an odd number of steps: its wall centreline is refused, naming it, and
/// never rounded; every other line is exact. (An odd whole millimetre halves exactly: see
/// `a39_odd_millimetre_centre_is_exact`.)
#[test]
fn odd_step_centre_refused() {
    let step = Length::from_steps(1);
    let wall = Assembly::new(
        vec![
            Layer::new(
                Function::Finish,
                step,
                MaterialId::parse("board").unwrap(),
                false,
            ),
            structure(90),
        ],
        1,
        2,
    )
    .unwrap();
    assert_eq!(wall.depth(WallCentre), None);
    assert_eq!(
        wall.offsets(WallCentre, false),
        Err(WallError::HalfNotExact { line: WallCentre })
    );
    let offsets = wall.offsets(ExteriorFinishFace, false).unwrap();
    assert_eq!(offsets.line(WallCentre), None);
    assert_eq!(
        offsets.line(ExteriorCoreFace),
        Some(mm(0).checked_sub(step).unwrap())
    );
    assert_eq!(
        WallError::HalfNotExact { line: WallCentre }.to_string(),
        "the wall centreline falls between two steps of the length unit"
    );
}

/// A length typed as people type it, read through construct-params.
fn typed(text: &str) -> Option<Length> {
    let def = Definition::new(id("t"), "t", Kind::Length, Holder::Type).unwrap();
    match def.parse(text, &Units::default()).ok()? {
        Value::Length(length) => Some(length),
        _ => None,
    }
}

/// With piece A39's length unit (the kernel's grid), half of an odd millimetre is exact: the
/// centreline of 113 mm is 56.5 mm, with no rule of this crate's own.
#[test]
fn a39_odd_millimetre_centre_is_exact() {
    let wall = Assembly::new(vec![finish(13), structure(90), finish(10)], 1, 2).unwrap();
    assert!(typed("56.5 mm").is_some());
    assert_eq!(wall.depth(WallCentre), typed("56.5 mm"));
    let offsets = wall.offsets(WallCentre, false).unwrap();
    assert_eq!(offsets.faces().first().copied(), typed("56.5 mm"));
    assert_eq!(offsets.faces().last().copied(), typed("-56.5 mm"));
}

/// With piece A39's length unit, the default partition's linings become 12.5 mm plasterboard:
/// 115 mm in all, its centreline 57.5 mm from either face. Switch on with that change made to
/// the defaults.
#[test]
fn a39_partition_lining_is_12_5_mm() {
    let wall = default("wall-partition-stud-90");
    assert!(typed("12.5 mm").is_some() && typed("57.5 mm").is_some());
    assert_eq!(Some(wall.layer(0).unwrap().thickness), typed("12.5 mm"));
    assert_eq!(Some(wall.layer(2).unwrap().thickness), typed("12.5 mm"));
    assert_eq!(Some(wall.total()), typed("115 mm"));
    assert_eq!(wall.depth(WallCentre), typed("57.5 mm"));
    assert_eq!(wall.depth(ExteriorCoreFace), typed("12.5 mm"));
}
