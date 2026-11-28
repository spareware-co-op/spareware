//! The IFC 4.3 writer's first half, on small models made by hand: a wall along an axis and one
//! at another angle, floors flat, with a hole and sloped, levels, and the file's own syntax.
//! Every number is worked out by hand in a comment beside it. The house itself, through the edit
//! log, is construct-api's `tests/house_ifc.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_interop::IfcError;
use construct_interop::ifc_spatial::{Export, Slab, Storey, Wall, write};
use construct_model::geometry::Recipe;
use construct_params::{Length, Value};
use construct_regen::floor_plan::FloorPlan;
use construct_regen::wall_plan::Layered;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

/// 12.5 mm in grid steps.
const HALF_CM: i64 = 80_000;

fn mm(n: i64) -> i64 {
    Length::checked_from_mm(n).unwrap().steps()
}

fn stored(steps: i64) -> Json {
    Json::String(Value::Length(Length::from_steps(steps)).stored())
}

/// A layered wall's filled recipe: `layers` are (function, material, exterior, interior) in steps.
fn layered(
    start: [i64; 2],
    end: [i64; 2],
    height: i64,
    layers: &[(&str, &str, i64, i64)],
) -> Layered {
    let mut args = BTreeMap::new();
    args.insert(
        "start".to_owned(),
        json!([stored(start[0]), stored(start[1])]),
    );
    args.insert("end".to_owned(), json!([stored(end[0]), stored(end[1])]));
    args.insert("height".to_owned(), stored(height));
    let layers: Vec<Json> = layers
        .iter()
        .map(|&(function, material, exterior, interior)| {
            json!({
                "function": function,
                "material": material,
                "exterior": stored(exterior),
                "interior": stored(interior),
            })
        })
        .collect();
    args.insert("layers".to_owned(), Json::Array(layers));
    Layered::of(&Recipe::construct("wall.layered", args)).unwrap()
}

/// A floor's filled recipe: loops of corners in steps, each layer (function, material, thickness)
/// in steps from the top face down, and the top heights of a sloped floor.
fn floor(
    loops: &[Vec<[i64; 2]>],
    layers: &[(&str, &str, i64)],
    tops: Option<Vec<Vec<i64>>>,
) -> FloorPlan {
    let mut args = BTreeMap::new();
    let mut at = 0;
    let rings: Vec<Json> = loops
        .iter()
        .map(|ring| {
            Json::Array(
                ring.iter()
                    .map(|c| {
                        at += 1;
                        json!({ "id": format!("e{at}"), "at": [c[0], c[1]] })
                    })
                    .collect(),
            )
        })
        .collect();
    args.insert("loops".to_owned(), Json::Array(rings));
    args.insert(
        "layers".to_owned(),
        Json::Array(
            layers
                .iter()
                .map(|&(function, material, thickness)| {
                    json!({ "function": function, "material": material, "thickness": thickness })
                })
                .collect(),
        ),
    );
    let mut recipe = Recipe::construct("floor", args);
    if let Some(tops) = tops {
        recipe.op_version = 2;
        recipe.args.insert("tops".to_owned(), json!(tops));
    }
    FloorPlan::of(&recipe).unwrap()
}

fn storey(id: &str, name: &str, elevation_mm: i64) -> Storey {
    Storey {
        id: id.to_owned(),
        name: name.to_owned(),
        elevation: Length::checked_from_mm(elevation_mm).unwrap(),
    }
}

fn export(storeys: Vec<Storey>, walls: Vec<Wall>, slabs: Vec<Slab>) -> Export {
    Export {
        document: "doc".to_owned(),
        name: "Small".to_owned(),
        stamp: "2026-01-01T00:00:00".to_owned(),
        storeys,
        walls,
        slabs,
    }
}

/// A south wall along +x: brick 100 (150 to 50 right of the line), frame 100 (50 to -50), lining
/// 12.5 (-50 to -62.5); 2700 high, 8000 long.
fn south(storey: Option<&str>) -> Wall {
    let layers = [
        ("finish", "brick", mm(150), mm(50)),
        ("structure", "timber", mm(50), mm(-50)),
        ("finish", "gypsum", mm(-50), mm(-50) - HALF_CM),
    ];
    Wall {
        id: "W-1".to_owned(),
        name: "South".to_owned(),
        type_name: "Brick veneer".to_owned(),
        storey: storey.map(str::to_owned),
        base: Length::from_mm(0),
        shape: layered([0, 0], [mm(8000), 0], mm(2700), &layers),
    }
}

fn count(text: &str, entity: &str) -> usize {
    text.matches(&format!("={entity}(")).count()
}

/// The file is read back as ISO 10303-21 by another implementation, every instance is where the
/// writer says, and no reference points past the last instance or at the instance itself.
#[test]
fn the_text_is_well_formed_step_and_its_references_are_inside_it() {
    let model = export(
        vec![storey("L-0", "Ground", 0)],
        vec![south(Some("L-0"))],
        Vec::new(),
    );
    let text = write(&model).unwrap();
    let parsed: step_p21::ast::Exchange = text.parse().unwrap();
    assert_eq!(parsed.header.len(), 3);
    let entities = &parsed.data[0].entities;
    let lines = text.lines().filter(|l| l.starts_with('#')).count();
    assert_eq!(entities.len(), lines);
    for (n, line) in text.lines().filter(|l| l.starts_with('#')).enumerate() {
        let number = n + 1;
        assert!(line.starts_with(&format!("#{number}=")), "{line}");
        let body = line.split_once('=').unwrap().1;
        let mut rest = body;
        while let Some(at) = rest.find('#') {
            let digits: String = rest[at + 1..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            let target: usize = digits.parse().unwrap();
            assert!(target >= 1 && target <= lines && target != number, "{line}");
            rest = &rest[at + 1..];
        }
    }
    assert!(text.starts_with("ISO-10303-21;\nHEADER;\n"));
    assert!(text.contains("FILE_SCHEMA(('IFC4X3_ADD2'));"));
    assert!(!text.contains('\r'));
}

/// The same model gives the same bytes; only the header's stamp is the caller's.
#[test]
fn the_same_model_gives_the_same_bytes() {
    let model = export(
        vec![storey("L-0", "Ground", 0)],
        vec![south(Some("L-0"))],
        Vec::new(),
    );
    let one = write(&model).unwrap();
    assert_eq!(one, write(&model).unwrap());
    let mut later = model.clone();
    later.stamp = "2030-12-31T23:59:59".to_owned();
    let two = write(&later).unwrap();
    assert_ne!(one, two);
    assert_eq!(
        one.replace("2026-01-01T00:00:00", "X"),
        two.replace("2030-12-31T23:59:59", "X")
    );
}

/// A wall along +x is placed along its own line, and its body is the strip between its finish
/// faces, in exact local coordinates: the exterior is on the right, so the 150 mm face is at
/// y = -150, and the lining's inner face is at y = +62.5 (12.5 mm = 80,000 steps, exact).
#[test]
fn a_wall_along_an_axis_is_exact() {
    let model = export(
        vec![storey("L-0", "Ground", 0)],
        vec![south(Some("L-0"))],
        Vec::new(),
    );
    let text = write(&model).unwrap();
    // Strip corners anticlockwise: (0,-150), (8000,-150), (8000,62.5), (0,62.5), closed.
    for point in ["(0.,-150.)", "(8000.,-150.)", "(8000.,62.5)", "(0.,62.5)"] {
        assert!(
            text.contains(&format!("=IFCCARTESIANPOINT({point});")),
            "{point}"
        );
    }
    // 2700 high, and the axis from 0 to 8000 along the wall.
    assert!(text.contains(",2700.);"));
    assert!(text.contains("=IFCCARTESIANPOINT((8000.,0.));"));
    // Layers: 100, 100 and 12.5, as exact decimals; the set is used with the offset of the first
    // face to the right of the line (150 mm), so -150 along the wall's own y.
    assert!(text.contains("=IFCMATERIALLAYER(#"));
    assert!(text.contains(",100.,.F.,$,$,'finish',$);"));
    assert!(text.contains(",12.5,.F.,$,$,'finish',$);"));
    assert_eq!(count(&text, "IFCMATERIALLAYERSETUSAGE"), 1);
    assert!(text.contains("=IFCMATERIALLAYERSETUSAGE(#"));
    assert!(text.contains(",.AXIS2.,.POSITIVE.,-150.,$);"));
    assert_eq!(count(&text, "IFCWALL"), 1);
}

/// A wall along -y is turned about z: its x runs down the plan's -y, its left is the plan's +x, so
/// a point is written as (distance along, distance to the left).
#[test]
fn a_wall_along_minus_y_is_turned_and_exact() {
    let layers = [("structure", "timber", mm(50), mm(-50))];
    let wall = Wall {
        id: "W-2".to_owned(),
        name: "West".to_owned(),
        type_name: "T".to_owned(),
        storey: None,
        base: Length::from_mm(0),
        shape: layered([0, mm(6000)], [0, 0], mm(3000), &layers),
    };
    let text = write(&export(Vec::new(), vec![wall], Vec::new())).unwrap();
    // Walking -y, the right is -x: the 50 mm face is at x = -50 and the -50 face at x = +50. In the
    // wall's frame the left is the plan's +x, so v = x, and u = 6000 - y runs 0 to 6000 along the
    // line. The strip is (0,-50), (6000,-50), (6000,50), (0,50).
    for point in ["(0.,-50.)", "(6000.,-50.)", "(6000.,50.)", "(0.,50.)"] {
        assert!(
            text.contains(&format!("=IFCCARTESIANPOINT({point});")),
            "{point}"
        );
    }
    assert!(text.contains("=IFCDIRECTION((0.,-1.,0.));"));
    assert!(text.contains("=IFCCARTESIANPOINT((0.,6000.,0.));"));
    // A wall on no level hangs from the building: one containment, in the building.
    assert_eq!(count(&text, "IFCRELCONTAINEDINSPATIALSTRUCTURE"), 1);
    assert_eq!(count(&text, "IFCBUILDINGSTOREY"), 0);
}

/// A 3-4-5 wall is at no axis: it is placed unturned, written in the plan's own coordinates, and
/// its layers are a layer set with no usage. Its faces are exact because 3-4-5 is whole: the face
/// 50 mm to the right of the line from (0,0) to (3000,4000) is moved by (40, -30) mm, and the one
/// 50 mm to the left by (-40, 30).
#[test]
fn a_wall_off_the_axes_is_written_in_the_plan() {
    let layers = [("structure", "timber", mm(50), mm(-50))];
    let wall = Wall {
        id: "W-3".to_owned(),
        name: "Diagonal".to_owned(),
        type_name: "T".to_owned(),
        storey: None,
        base: Length::from_mm(0),
        shape: layered([0, 0], [mm(3000), mm(4000)], mm(3000), &layers),
    };
    let text = write(&export(Vec::new(), vec![wall], Vec::new())).unwrap();
    for point in ["(40.,-30.)", "(3040.,3970.)", "(2960.,4030.)", "(-40.,30.)"] {
        assert!(
            text.contains(&format!("=IFCCARTESIANPOINT({point});")),
            "{point}"
        );
    }
    assert_eq!(count(&text, "IFCMATERIALLAYERSETUSAGE"), 0);
    assert_eq!(count(&text, "IFCMATERIALLAYERSET"), 1);
    assert_eq!(count(&text, "IFCRELASSOCIATESMATERIAL"), 1);
}

/// A flat floor with a hole is a profile with a void, extruded through its thickness, standing on
/// its underside: top at 3000 on the 3000 storey, 150 thick, so 150 below the storey's own
/// origin. Its layer set runs up from the base.
#[test]
fn a_floor_with_a_hole_is_a_profile_with_a_void() {
    let outer = vec![[0, 0], [mm(5000), 0], [mm(5000), mm(4000)], [0, mm(4000)]];
    // The hole runs clockwise.
    let hole = vec![
        [mm(1000), mm(1000)],
        [mm(1000), mm(2000)],
        [mm(2000), mm(2000)],
        [mm(2000), mm(1000)],
    ];
    let plan = floor(
        &[outer, hole],
        &[
            ("finish", "tile", mm(10)),
            ("structure", "concrete", mm(140)),
        ],
        None,
    );
    let slab = Slab {
        id: "F-1".to_owned(),
        name: "Floor".to_owned(),
        type_name: "Concrete 150".to_owned(),
        storey: Some("L-1".to_owned()),
        top: Length::from_mm(3000),
        shape: plan,
    };
    let text = write(&export(
        vec![storey("L-1", "First", 3000)],
        Vec::new(),
        vec![slab],
    ))
    .unwrap();
    assert_eq!(count(&text, "IFCARBITRARYPROFILEDEFWITHVOIDS"), 1);
    assert_eq!(count(&text, "IFCARBITRARYCLOSEDPROFILEDEF"), 0);
    assert!(text.contains(",150.);"), "the extrusion is 10 + 140 deep");
    assert!(text.contains("=IFCCARTESIANPOINT((0.,0.,-150.));"));
    assert!(text.contains(",.FLOOR.);"));
    // The set lists the concrete first (the layers are listed from the top down in the recipe).
    let set = text
        .lines()
        .find(|l| l.contains("=IFCMATERIALLAYERSET("))
        .unwrap();
    let layer_lines: Vec<&str> = text
        .lines()
        .filter(|l| l.contains("=IFCMATERIALLAYER("))
        .collect();
    assert_eq!(layer_lines.len(), 2);
    assert!(layer_lines[0].contains(",140.,") && layer_lines[1].contains(",10.,"));
    assert!(set.contains("'Concrete 150'"));
    assert!(text.contains(",.AXIS3.,.POSITIVE.,0.,$);"));
}

/// A sloped floor cannot be an extrusion of its plan, so it is a closed face set of its eight
/// corners: its top at 0, 0, 600 and 600 mm over the four corners and its underside 150 below
/// each, a top face, a bottom face the other way round, and four sides.
#[test]
fn a_sloped_floor_is_a_closed_face_set() {
    let outer = vec![[0, 0], [mm(5000), 0], [mm(5000), mm(4000)], [0, mm(4000)]];
    let plan = floor(
        &[outer],
        &[("structure", "concrete", mm(150))],
        Some(vec![vec![0, 0, mm(600), mm(600)]]),
    );
    let slab = Slab {
        id: "F-2".to_owned(),
        name: "Ramp".to_owned(),
        type_name: "Concrete 150".to_owned(),
        storey: None,
        top: Length::from_mm(0),
        shape: plan,
    };
    let text = write(&export(Vec::new(), Vec::new(), vec![slab])).unwrap();
    assert_eq!(count(&text, "IFCPOLYGONALFACESET"), 1);
    assert_eq!(count(&text, "IFCEXTRUDEDAREASOLID"), 0);
    assert_eq!(count(&text, "IFCINDEXEDPOLYGONALFACE"), 6);
    // Points 1 and 2 are the first corner's top and underside; 5 and 6 the third's.
    assert!(text.contains(
        "=IFCCARTESIANPOINTLIST3D(((0.,0.,0.),(0.,0.,-150.),(5000.,0.,0.),(5000.,0.,-150.),\
         (5000.,4000.,600.),(5000.,4000.,450.),(0.,4000.,600.),(0.,4000.,450.)),$);"
    ));
    // The top face runs through the top points, the bottom face back through the undersides, and
    // the first side from the first corner's top down and along.
    assert!(text.contains("=IFCINDEXEDPOLYGONALFACE((1,3,5,7));"));
    assert!(text.contains("=IFCINDEXEDPOLYGONALFACE((8,6,4,2));"));
    assert!(text.contains("=IFCINDEXEDPOLYGONALFACE((1,2,4,3));"));
    assert!(text.contains("'Tessellation'"));
}

/// An element on a level that was not given, and two things of one ID, are refused, in words.
#[test]
fn what_cannot_be_written_is_refused() {
    let lost = export(Vec::new(), vec![south(Some("L-9"))], Vec::new());
    assert_eq!(write(&lost), Err(IfcError::UnknownStorey("L-9".to_owned())));
    assert!(write(&lost).unwrap_err().to_string().contains("L-9"));
    let twice = export(Vec::new(), vec![south(None), south(None)], Vec::new());
    assert_eq!(write(&twice), Err(IfcError::Duplicate("W-1".to_owned())));
    let levels = export(
        vec![storey("L-0", "A", 0), storey("L-0", "B", 1000)],
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(write(&levels), Err(IfcError::Duplicate("L-0".to_owned())));
}

/// Names are escaped as the standard says: an apostrophe is doubled and a letter outside ASCII is
/// written in the `\X2\` form, so no byte of the file is outside printable ASCII.
#[test]
fn names_are_escaped() {
    let mut model = export(
        vec![storey("L-0", "Rez-de-chauss\u{e9}e", 0)],
        Vec::new(),
        Vec::new(),
    );
    model.name = "Smith's house".to_owned();
    let text = write(&model).unwrap();
    assert!(text.contains("'Smith''s house'"));
    assert!(text.contains("'Rez-de-chauss\\X2\\00E9\\X0\\e'"));
    assert!(
        text.bytes()
            .all(|b| b == b'\n' || (b' '..=b'~').contains(&b))
    );
    let _: step_p21::ast::Exchange = text.parse().unwrap();
}

/// A model with nothing in it is still a project, a site and a building.
#[test]
fn an_empty_model_is_a_project_with_a_site_and_a_building() {
    let text = write(&export(Vec::new(), Vec::new(), Vec::new())).unwrap();
    assert_eq!(count(&text, "IFCPROJECT"), 1);
    assert_eq!(count(&text, "IFCSITE"), 1);
    assert_eq!(count(&text, "IFCBUILDING"), 1);
    assert_eq!(count(&text, "IFCRELAGGREGATES"), 2);
    assert_eq!(count(&text, "IFCRELCONTAINEDINSPATIALSTRUCTURE"), 0);
}
