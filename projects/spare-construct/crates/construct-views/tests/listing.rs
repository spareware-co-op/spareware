//! What a view shows: the category rule, the phase rule, a plan's cut and seen elements by its view
//! range, a section's and an elevation's region, and the refusals. Written against this crate's own
//! types, so the same tests judge a replacement of any engine behind them.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::{
    ANALYTICAL_MEMBERS, CategoryId, CategoryTable, DOORS, FLOORS, FOUNDATIONS, GRIDS,
    IMPORTED_DRAWINGS, SHEETS, WALLS,
};
use construct_levels::{Level, Levels};
use construct_model::geometry::Bounds;
use construct_params::Length;
use construct_phases::{
    DesignOptions, ElementStaging, OptionRef, PhaseError, PhaseFilter, PhaseList, ViewSettings,
};
use construct_views::{
    CutLine, DetailLevel, How, Model, PlanSetup, Point, Scale, Side, View, ViewElement, ViewError,
    ViewKind, ViewRange, list,
};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

struct World {
    levels: Levels,
    categories: CategoryTable,
    phases: PhaseList,
    options: DesignOptions,
}

impl World {
    fn new() -> Self {
        let mut levels = Levels::new();
        levels
            .add(Level::new(id("l1"), "Ground", mm(0), true).unwrap())
            .unwrap();
        levels
            .add(Level::new(id("l2"), "First", mm(3200), true).unwrap())
            .unwrap();
        Self {
            levels,
            categories: CategoryTable::new(),
            phases: PhaseList::new()
                .add(id("ph-a"), "Existing")
                .unwrap()
                .add(id("ph-b"), "New works")
                .unwrap(),
            options: DesignOptions::new(),
        }
    }

    fn model(&self) -> Model<'_> {
        Model {
            levels: &self.levels,
            categories: &self.categories,
            phases: &self.phases,
            options: &self.options,
        }
    }

    fn staging(&self, created: &str, demolished: Option<&str>) -> ElementStaging {
        ElementStaging::new(
            &self.phases,
            &self.options,
            id(created),
            demolished.map(id),
            None,
        )
        .unwrap()
    }

    fn element(
        &self,
        key: &str,
        category: CategoryId,
        min: [i64; 3],
        max: [i64; 3],
    ) -> ViewElement {
        ViewElement {
            id: id(key),
            category,
            bounds: Bounds { min, max },
            staging: self.staging("ph-a", None),
        }
    }

    fn view(&self, kind: ViewKind) -> View {
        View::new(
            id("v"),
            "A view",
            Scale::new(100).unwrap(),
            DetailLevel::Medium,
            kind,
            self.staging("ph-a", None),
        )
        .unwrap()
    }
}

/// Top 2300 mm, cut plane 1200, bottom 0, view depth -500, all from the ground level.
fn range() -> ViewRange {
    ViewRange::on_level(&id("l1"), mm(2300), mm(1200), mm(0), mm(-500))
}

fn floor_plan(world: &World) -> View {
    world.view(ViewKind::FloorPlan(PlanSetup {
        level: id("l1"),
        range: range(),
    }))
}

fn ids(of: &[&Id]) -> Vec<String> {
    of.iter().map(|id| id.as_str().to_owned()).collect()
}

fn the_building(world: &World) -> Vec<ViewElement> {
    vec![
        world.element("wall", WALLS, [0, 0, 0], [5000, 200, 3000]),
        world.element("door", DOORS, [1000, 0, 0], [1900, 200, 2100]),
        world.element("slab", FLOORS, [0, 0, -200], [5000, 5000, 0]),
        world.element("footing", FOUNDATIONS, [0, 0, -700], [500, 500, -300]),
        world.element("deep", FOUNDATIONS, [0, 0, -2000], [500, 500, -1000]),
        world.element("beam", WALLS, [0, 0, 2000], [5000, 200, 2200]),
        world.element("roof", WALLS, [0, 0, 2400], [5000, 5000, 2600]),
    ]
}

#[test]
fn a_floor_plan_cuts_what_the_plane_crosses_and_sees_what_is_below() {
    let world = World::new();
    let listing = list(&floor_plan(&world), &world.model(), &the_building(&world)).unwrap();
    assert_eq!(ids(&listing.cut()), ["door", "wall"]);
    assert_eq!(ids(&listing.seen()), ["slab"]);
    assert_eq!(ids(&listing.beyond()), ["footing"]);
    assert_eq!(listing.len(), 4);
    assert_eq!(listing.how(&id("wall")), Some(How::Cut));
    assert_eq!(listing.how(&id("beam")), None);
    assert_eq!(listing.how(&id("deep")), None);
    assert_eq!(listing.how(&id("roof")), None);
}

#[test]
fn a_ceiling_plan_cuts_the_plane_and_sees_what_is_above_it_up_to_the_top() {
    let world = World::new();
    let view = world.view(ViewKind::CeilingPlan(PlanSetup {
        level: id("l1"),
        range: range(),
    }));
    let listing = list(&view, &world.model(), &the_building(&world)).unwrap();
    assert_eq!(ids(&listing.cut()), ["door", "wall"]);
    assert_eq!(ids(&listing.seen()), ["beam"]);
    assert_eq!(ids(&listing.beyond()), Vec::<String>::new());
    assert_eq!(listing.how(&id("roof")), None);
    assert_eq!(listing.how(&id("slab")), None);
}

#[test]
fn touching_the_cut_plane_is_cut() {
    let world = World::new();
    let elements = vec![
        world.element("rail-top", WALLS, [0, 0, 900], [100, 100, 1200]),
        world.element("rail-bottom", WALLS, [0, 0, 1200], [100, 100, 1500]),
        world.element("under", WALLS, [0, 0, 900], [100, 100, 1199]),
    ];
    let listing = list(&floor_plan(&world), &world.model(), &elements).unwrap();
    assert_eq!(ids(&listing.cut()), ["rail-bottom", "rail-top"]);
    assert_eq!(ids(&listing.seen()), ["under"]);
}

#[test]
fn a_cut_plane_between_whole_millimetres_is_not_rounded() {
    let world = World::new();
    // The cut plane is 12.5 mm above the level: 80,000 grid steps, between 12 and 13 mm.
    let range = ViewRange {
        top: construct_levels::Tie {
            level: id("l1"),
            offset: mm(2300),
        },
        cut: construct_levels::Tie {
            level: id("l1"),
            offset: Length::from_steps(80_000),
        },
        bottom: construct_levels::Tie {
            level: id("l1"),
            offset: mm(0),
        },
        depth: construct_levels::Tie {
            level: id("l1"),
            offset: mm(-500),
        },
    };
    let view = world.view(ViewKind::FloorPlan(PlanSetup {
        level: id("l1"),
        range,
    }));
    let elements = vec![
        world.element("twelve", WALLS, [0, 0, 0], [100, 100, 12]),
        world.element("thirteen", WALLS, [0, 0, 0], [100, 100, 13]),
    ];
    let listing = list(&view, &world.model(), &elements).unwrap();
    assert_eq!(ids(&listing.cut()), ["thirteen"]);
    assert_eq!(ids(&listing.seen()), ["twelve"]);
}

#[test]
fn moving_a_level_moves_the_plan_with_it() {
    let mut world = World::new();
    let wall = world.element("wall", WALLS, [0, 0, 6400], [5000, 200, 9400]);
    let plan_one = floor_plan(&world);
    let first = list(&plan_one, &world.model(), std::slice::from_ref(&wall)).unwrap();
    assert!(first.is_empty());
    world.levels.set_elevation(&id("l1"), mm(6400)).unwrap();
    let moved = list(&plan_one, &world.model(), &[wall]).unwrap();
    assert_eq!(ids(&moved.cut()), ["wall"]);
}

#[test]
fn only_the_categories_a_view_draws_are_shown() {
    let world = World::new();
    let elements = vec![
        world.element("wall", WALLS, [0, 0, 0], [100, 100, 3000]),
        world.element("grid", GRIDS, [0, 0, 0], [100, 100, 3000]),
        world.element("sheet", SHEETS, [0, 0, 0], [100, 100, 3000]),
        world.element("analysis", ANALYTICAL_MEMBERS, [0, 0, 0], [100, 100, 3000]),
        world.element("drawing", IMPORTED_DRAWINGS, [0, 0, 0], [100, 100, 3000]),
    ];
    let listing = list(&floor_plan(&world), &world.model(), &elements).unwrap();
    assert_eq!(ids(&listing.cut()), ["drawing", "wall"]);
}

#[test]
fn the_phase_rule_is_the_phases_crates_own() {
    let world = World::new();
    let mut existing = world.element("old", WALLS, [0, 0, 0], [100, 100, 3000]);
    existing.staging = world.staging("ph-a", None);
    let mut built = world.element("new", WALLS, [0, 0, 0], [100, 100, 3000]);
    built.staging = world.staging("ph-b", None);
    let mut removed = world.element("gone", WALLS, [0, 0, 0], [100, 100, 3000]);
    removed.staging = world.staging("ph-a", Some("ph-b"));
    let elements = [existing, built, removed];
    let settings = |filter| ViewSettings {
        phase: id("ph-b"),
        filter,
        choices: Vec::new(),
    };
    let plan = floor_plan(&world);
    // No phase settings: every element, whatever its phase.
    let all = list(&plan, &world.model(), &elements).unwrap();
    assert_eq!(ids(&all.cut()), ["gone", "new", "old"]);
    let new_only = plan
        .clone()
        .with_phasing(Some(settings(PhaseFilter::ShowNew)));
    assert_eq!(
        ids(&list(&new_only, &world.model(), &elements).unwrap().cut()),
        ["new"]
    );
    let work = plan
        .clone()
        .with_phasing(Some(settings(PhaseFilter::ShowNewAndDemolished)));
    assert_eq!(
        ids(&list(&work, &world.model(), &elements).unwrap().cut()),
        ["gone", "new"]
    );
    let standing = plan.with_phasing(Some(settings(PhaseFilter::ShowExistingAndNew)));
    assert_eq!(
        ids(&list(&standing, &world.model(), &elements).unwrap().cut()),
        ["new", "old"]
    );
}

#[test]
fn design_options_follow_the_views_choice() {
    let mut world = World::new();
    world.options = DesignOptions::new()
        .add_set(id("set"), "Entrance", id("opt-1"), "Glazed")
        .unwrap()
        .add_option(&id("set"), id("opt-2"), "Solid")
        .unwrap();
    let option = |key: &str, option: &str| {
        let mut element = world.element(key, WALLS, [0, 0, 0], [100, 100, 3000]);
        element.staging = ElementStaging::new(
            &world.phases,
            &world.options,
            id("ph-a"),
            None,
            Some(OptionRef {
                set: id("set"),
                option: id(option),
            }),
        )
        .unwrap();
        element
    };
    let elements = [option("glazed", "opt-1"), option("solid", "opt-2")];
    let settings = |choices| ViewSettings {
        phase: id("ph-a"),
        filter: PhaseFilter::ShowAll,
        choices,
    };
    let primary = floor_plan(&world).with_phasing(Some(settings(Vec::new())));
    assert_eq!(
        ids(&list(&primary, &world.model(), &elements).unwrap().cut()),
        ["glazed"]
    );
    let chosen =
        floor_plan(&world).with_phasing(Some(settings(vec![construct_phases::OptionChoice {
            set: id("set"),
            option: id("opt-2"),
        }])));
    assert_eq!(
        ids(&list(&chosen, &world.model(), &elements).unwrap().cut()),
        ["solid"]
    );
}

fn line(from: (i32, i32), to: (i32, i32), look: Side, far: i32) -> CutLine {
    CutLine::new(
        Point {
            x: mm(from.0),
            y: mm(from.1),
        },
        Point {
            x: mm(to.0),
            y: mm(to.1),
        },
        look,
        mm(far),
    )
    .unwrap()
}

fn tall(world: &World, key: &str, x: (i64, i64), y: (i64, i64)) -> ViewElement {
    world.element(key, WALLS, [x.0, y.0, 0], [x.1, y.1, 3000])
}

#[test]
fn a_section_cuts_what_the_line_crosses_and_sees_what_is_in_front_within_its_depth() {
    let world = World::new();
    let view = world.view(ViewKind::Section(line(
        (0, 0),
        (10_000, 0),
        Side::Left,
        5000,
    )));
    let elements = vec![
        tall(&world, "crossed", (2000, 3000), (-100, 100)),
        tall(&world, "in-front", (2000, 3000), (1000, 2000)),
        tall(&world, "at-the-depth", (2000, 3000), (4000, 5000)),
        tall(&world, "too-far", (2000, 3000), (5001, 6000)),
        tall(&world, "behind", (2000, 3000), (-2000, -1000)),
        tall(&world, "past-the-end", (11_000, 12_000), (1000, 2000)),
        tall(&world, "at-the-end", (-500, 0), (1000, 2000)),
    ];
    let listing = list(&view, &world.model(), &elements).unwrap();
    assert_eq!(ids(&listing.cut()), ["crossed"]);
    assert_eq!(
        ids(&listing.seen()),
        ["at-the-depth", "at-the-end", "in-front"]
    );
    // Looking the other way sees what was behind.
    let other = world.view(ViewKind::Section(line(
        (0, 0),
        (10_000, 0),
        Side::Right,
        5000,
    )));
    let listing = list(&other, &world.model(), &elements).unwrap();
    assert_eq!(ids(&listing.cut()), ["crossed"]);
    assert_eq!(ids(&listing.seen()), ["behind"]);
}

#[test]
fn a_section_sees_every_height() {
    let world = World::new();
    let view = world.view(ViewKind::Section(line(
        (0, 0),
        (10_000, 0),
        Side::Left,
        5000,
    )));
    let elements = vec![
        world.element("deep", WALLS, [2000, 1000, -9000], [3000, 2000, -8000]),
        world.element("high", WALLS, [2000, 1000, 80_000], [3000, 2000, 90_000]),
    ];
    let listing = list(&view, &world.model(), &elements).unwrap();
    assert_eq!(ids(&listing.seen()), ["deep", "high"]);
}

#[test]
fn a_sloping_section_line_is_tested_exactly_not_by_its_bounding_box() {
    let world = World::new();
    // From the corner up and across at 45 degrees; walking along it, right is down and to the east.
    let view = world.view(ViewKind::Section(line(
        (0, 0),
        (10_000, 10_000),
        Side::Right,
        1000,
    )));
    let elements = vec![
        tall(&world, "crossed", (5000, 5100), (4900, 5100)),
        // 700 mm from the line, on the right: seen.
        tall(&world, "near", (5500, 5600), (4800, 4900)),
        // On the left of the line.
        tall(&world, "left", (4800, 4900), (5500, 5600)),
        // Inside the region's bounding box, but 6.3 m from the line.
        tall(&world, "far", (9000, 9100), (0, 100)),
        // On the right, 2.1 m from the line: past the depth.
        tall(&world, "past", (6000, 6200), (3000, 3200)),
    ];
    let listing = list(&view, &world.model(), &elements).unwrap();
    assert_eq!(ids(&listing.cut()), ["crossed"]);
    assert_eq!(ids(&listing.seen()), ["near"]);
}

#[test]
fn an_elevation_is_a_picture_plane_with_a_depth() {
    let world = World::new();
    let view = world.view(ViewKind::Elevation(line(
        (0, -3000),
        (10_000, -3000),
        Side::Left,
        20_000,
    )));
    let elements = vec![
        tall(&world, "building", (1000, 9000), (0, 8000)),
        tall(&world, "behind", (1000, 9000), (-9000, -4000)),
    ];
    let listing = list(&view, &world.model(), &elements).unwrap();
    assert_eq!(ids(&listing.seen()), ["building"]);
    assert_eq!(ids(&listing.cut()), Vec::<String>::new());
}

#[test]
fn a_three_d_view_shows_everything_the_rules_let_through() {
    let world = World::new();
    let listing = list(
        &world.view(ViewKind::ThreeD),
        &world.model(),
        &the_building(&world),
    )
    .unwrap();
    assert_eq!(listing.len(), 7);
    assert_eq!(listing.seen().len(), 7);
}

#[test]
fn a_drafting_view_and_a_legend_show_no_model() {
    let world = World::new();
    for kind in [ViewKind::Drafting, ViewKind::Legend] {
        let listing = list(&world.view(kind), &world.model(), &the_building(&world)).unwrap();
        assert!(listing.is_empty());
    }
}

#[test]
fn the_answer_does_not_depend_on_the_order_of_the_elements() {
    let world = World::new();
    let mut elements = the_building(&world);
    let forward = list(&floor_plan(&world), &world.model(), &elements).unwrap();
    elements.reverse();
    let backward = list(&floor_plan(&world), &world.model(), &elements).unwrap();
    assert_eq!(forward, backward);
    let order: Vec<&str> = forward.shown().iter().map(|s| s.id.as_str()).collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(order, sorted);
}

#[test]
fn refusals_are_values() {
    let world = World::new();
    let plan = floor_plan(&world);
    let twice = vec![
        world.element("a", WALLS, [0, 0, 0], [1, 1, 1]),
        world.element("a", WALLS, [0, 0, 0], [1, 1, 1]),
    ];
    assert_eq!(
        list(&plan, &world.model(), &twice),
        Err(ViewError::DuplicateElement(id("a")))
    );
    let nowhere = vec![world.element("a", CategoryId(123_456), [0, 0, 0], [1, 1, 1])];
    assert_eq!(
        list(&plan, &world.model(), &nowhere),
        Err(ViewError::UnknownCategory(CategoryId(123_456)))
    );
    let mut lost = world.element("a", WALLS, [0, 0, 0], [1, 1, 1]);
    lost.staging.created = id("ph-none");
    assert_eq!(
        list(&plan, &world.model(), &[lost]),
        Err(ViewError::Phase(PhaseError::UnknownPhase(id("ph-none"))))
    );
    let inverted = vec![world.element("a", WALLS, [5, 0, 0], [1, 1, 1])];
    assert!(matches!(
        list(&plan, &world.model(), &inverted),
        Err(ViewError::Spatial(_))
    ));
    let unknown_level = world.view(ViewKind::FloorPlan(PlanSetup {
        level: id("nowhere"),
        range: range(),
    }));
    assert_eq!(
        list(&unknown_level, &world.model(), &[]),
        Err(ViewError::UnknownLevel(id("nowhere")))
    );
}
