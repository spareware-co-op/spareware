//! The default metric set: what each type is, its core and wrapping layers, and which of its
//! materials construct-materials' default library does not hold yet.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::CategoryTable;
use construct_materials::Materials;
use construct_types::Parameters;
use construct_walls::{
    Function, Side, add_parameters, assembly_of, default_wall_types, default_walls,
};
use std::collections::BTreeSet;

#[test]
fn default_set() {
    use Function::{Finish, Membrane, Structure, Thermal};
    let walls = default_walls().unwrap();
    let ids: Vec<&str> = walls.iter().map(|w| w.id).collect();
    assert_eq!(
        ids,
        vec![
            "wall-partition-stud-90",
            "wall-concrete-200",
            "wall-brick-veneer-90"
        ]
    );
    let cores: Vec<(usize, usize)> = walls
        .iter()
        .map(|w| {
            (
                w.assembly.boundary(Side::Exterior),
                w.assembly.boundary(Side::Interior),
            )
        })
        .collect();
    assert_eq!(cores, vec![(1, 2), (0, 1), (3, 4)]);
    let wrapping: Vec<Vec<usize>> = walls.iter().map(|w| w.assembly.wrapping()).collect();
    assert_eq!(wrapping, vec![vec![0, 2], vec![], vec![2, 4]]);
    let functions: Vec<Vec<Function>> = walls
        .iter()
        .map(|w| w.assembly.layers().iter().map(|l| l.function).collect())
        .collect();
    assert_eq!(
        functions,
        vec![
            vec![Finish, Structure, Finish],
            vec![Structure],
            vec![Finish, Thermal, Membrane, Structure, Finish],
        ]
    );
}

/// Plasterboard and clay brick are not in construct-materials' defaults yet; every other layer's
/// material is. When the materials crate adds them, this test says so.
#[test]
fn missing_materials_pinned() {
    let library = Materials::default_metric();
    let walls = default_walls().unwrap();
    let missing: Vec<Vec<usize>> = walls
        .iter()
        .map(|w| w.assembly.missing_materials(&library))
        .collect();
    assert_eq!(missing, vec![vec![0, 2], vec![], vec![0, 4]]);
    let names: BTreeSet<&str> = walls
        .iter()
        .flat_map(|w| {
            w.assembly
                .missing_materials(&library)
                .into_iter()
                .map(|at| w.assembly.layer(at).unwrap().material.as_str())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(names, BTreeSet::from(["brick-clay", "plasterboard-gypsum"]));
}

/// As element types: walls, named, each holding its default layers.
#[test]
fn default_element_types() {
    let mut params = Parameters::new();
    add_parameters(&mut params).unwrap();
    let types = default_wall_types(&params, &CategoryTable::new()).unwrap();
    let walls = default_walls().unwrap();
    assert_eq!(types.len(), walls.len());
    for (ty, wall) in types.iter().zip(&walls) {
        assert_eq!(ty.id().as_str(), wall.id);
        assert_eq!(ty.name(), wall.name);
        assert_eq!(assembly_of(ty).unwrap(), wall.assembly);
    }
}
