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

use common::{finish, id, mm, mms, structure};
use construct_params::{Definition, Holder, Kind, Length, Units, Value};
use construct_walls::{Assembly, LocationLine, WallError, default_walls};

use LocationLine::{
    CoreCentre, ExteriorCoreFace, ExteriorFinishFace, InteriorCoreFace, InteriorFinishFace,
    WallCentre,
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
        total: 116,
        depths: &[0, 13, 103, 116],
        lines: [58, 58, 0, 116, 13, 103],
        faces: [
            &[58, 45, -45, -58],
            &[58, 45, -45, -58],
            &[0, -13, -103, -116],
            &[116, 103, 13, 0],
            &[13, 0, -90, -103],
            &[103, 90, 0, -13],
        ],
    },
    Expected {
        wall: "wall-concrete-200",
        total: 200,
        depths: &[0, 200],
        lines: [100, 100, 0, 200, 0, 200],
        faces: [
            &[100, -100],
            &[100, -100],
            &[0, -200],
            &[200, 0],
            &[0, -200],
            &[200, 0],
        ],
    },
    Expected {
        wall: "wall-brick-veneer-90",
        total: 250,
        depths: &[0, 110, 150, 150, 240, 250],
        lines: [125, 195, 0, 250, 150, 240],
        faces: [
            &[125, 15, -25, -25, -115, -125],
            &[195, 85, 45, 45, -45, -55],
            &[0, -110, -150, -150, -240, -250],
            &[250, 140, 100, 100, 10, 0],
            &[150, 40, 0, 0, -90, -100],
            &[240, 130, 90, 90, 0, -10],
        ],
    },
];

/// Every default type's total, face depths and location line depths.
#[test]
fn default_depths() {
    for expected in &EXPECTED {
        let wall = default(expected.wall);
        assert_eq!(wall.total(), mm(expected.total), "{}", expected.wall);
        assert_eq!(wall.face_depths(), mms(expected.depths).as_slice());
        for (line, depth) in LocationLine::ALL.iter().zip(expected.lines) {
            assert_eq!(
                wall.depth(*line),
                Some(mm(depth)),
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
                mms(faces).as_slice(),
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

/// 13 + 90 + 10 = 113 mm. Today's length unit is a whole millimetre, so half of 113 mm is not a
/// length: the wall centreline is refused, naming it, and never rounded; every other line is
/// exact. Delete this test when piece A39 lands: its twin below then switches on.
#[test]
fn odd_millimetre_centre_refused_today() {
    let wall = Assembly::new(vec![finish(13), structure(90), finish(10)], 1, 2).unwrap();
    assert_eq!(wall.depth(WallCentre), None);
    assert_eq!(
        wall.offsets(WallCentre, false),
        Err(WallError::HalfNotExact { line: WallCentre })
    );
    let offsets = wall.offsets(ExteriorFinishFace, false).unwrap();
    assert_eq!(offsets.faces(), mms(&[0, -13, -103, -113]).as_slice());
    assert_eq!(offsets.line(WallCentre), None);
    assert_eq!(offsets.line(CoreCentre), Some(mm(-58)));
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
#[ignore = "switch on when piece A39 lands: Length in the kernel's grid unit"]
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
#[ignore = "switch on when piece A39 lands: Length in the kernel's grid unit"]
fn a39_partition_lining_is_12_5_mm() {
    let wall = default("wall-partition-stud-90");
    assert!(typed("12.5 mm").is_some() && typed("57.5 mm").is_some());
    assert_eq!(Some(wall.layer(0).unwrap().thickness), typed("12.5 mm"));
    assert_eq!(Some(wall.layer(2).unwrap().thickness), typed("12.5 mm"));
    assert_eq!(Some(wall.total()), typed("115 mm"));
    assert_eq!(wall.depth(WallCentre), typed("57.5 mm"));
    assert_eq!(wall.depth(ExteriorCoreFace), typed("12.5 mm"));
}
