//! A view's data: its kinds, scale and detail level, a plan's view range, a section's line, the
//! rules a document of views keeps, and how a view's scale picks a pen.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::CategoryTable;
use construct_levels::{Level, Levels, Tie};
use construct_params::Length;
use construct_phases::{
    DesignOptions, ElementStaging, PhaseError, PhaseFilter, PhaseList, ViewSettings,
};
use construct_styles::default::default_model_pens;
use construct_views::{
    CutLine, DetailLevel, Model, PlanSetup, Point, Scale, Side, View, ViewError, ViewKind,
    ViewRange, Views,
};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

fn levels() -> Levels {
    let mut levels = Levels::new();
    levels
        .add(Level::new(id("l1"), "Ground", mm(0), true).unwrap())
        .unwrap();
    levels
        .add(Level::new(id("l2"), "First", mm(3200), true).unwrap())
        .unwrap();
    levels
}

fn phases() -> PhaseList {
    PhaseList::new()
        .add(id("ph-a"), "Existing")
        .unwrap()
        .add(id("ph-b"), "New works")
        .unwrap()
}

fn staging() -> ElementStaging {
    ElementStaging::new(&phases(), &DesignOptions::new(), id("ph-a"), None, None).unwrap()
}

fn plan(level: &str) -> ViewKind {
    ViewKind::FloorPlan(PlanSetup {
        level: id(level),
        range: ViewRange::on_level(&id(level), mm(2300), mm(1200), mm(0), mm(-500)),
    })
}

fn view(key: &str, name: &str, kind: ViewKind) -> View {
    View::new(
        id(key),
        name,
        Scale::new(100).unwrap(),
        DetailLevel::Medium,
        kind,
        staging(),
    )
    .unwrap()
}

#[test]
fn a_view_holds_its_scale_detail_level_and_kind() {
    let v = view("v1", "  Ground floor plan ", plan("l1"));
    assert_eq!(v.name(), "Ground floor plan");
    assert_eq!(v.scale().denominator(), 100);
    assert_eq!(v.detail(), DetailLevel::Medium);
    assert_eq!(v.kind().key(), "floor_plan");
    assert_eq!(v.kind().plan().unwrap().level, id("l1"));
    assert!(v.kind().cut_line().is_none());
    let changed = v
        .clone()
        .with_scale(Scale::new(50).unwrap())
        .with_detail(DetailLevel::Fine);
    assert_eq!(changed.scale().denominator(), 50);
    assert_eq!(changed.detail(), DetailLevel::Fine);
    assert_eq!(v.scale().denominator(), 100);
}

#[test]
fn detail_levels_are_ordered_and_keyed() {
    assert!(DetailLevel::Coarse < DetailLevel::Medium && DetailLevel::Medium < DetailLevel::Fine);
    for level in [DetailLevel::Coarse, DetailLevel::Medium, DetailLevel::Fine] {
        assert_eq!(DetailLevel::from_key(level.key()), Some(level));
    }
    assert_eq!(DetailLevel::from_key("ultra"), None);
}

#[test]
fn a_scale_is_one_to_a_whole_number_in_range() {
    assert!(Scale::new(0).is_err());
    assert!(Scale::new(1_000_001).is_err());
    assert_eq!(Scale::new(1).unwrap().denominator(), 1);
}

#[test]
fn a_name_must_have_something_in_it() {
    let made = View::new(
        id("v"),
        "   ",
        Scale::new(100).unwrap(),
        DetailLevel::Medium,
        ViewKind::ThreeD,
        staging(),
    );
    assert_eq!(made, Err(ViewError::EmptyName));
    assert_eq!(
        view("v", "x", ViewKind::ThreeD).renamed(" "),
        Err(ViewError::EmptyName)
    );
}

#[test]
fn every_kind_has_its_own_key_and_only_the_model_views_show_the_model() {
    let line = CutLine::new(
        Point { x: mm(0), y: mm(0) },
        Point {
            x: mm(1000),
            y: mm(0),
        },
        Side::Left,
        mm(500),
    )
    .unwrap();
    let kinds = [
        plan("l1"),
        ViewKind::CeilingPlan(PlanSetup {
            level: id("l1"),
            range: ViewRange::on_level(&id("l1"), mm(2300), mm(1200), mm(0), mm(-500)),
        }),
        ViewKind::Section(line),
        ViewKind::Elevation(line),
        ViewKind::ThreeD,
        ViewKind::Drafting,
        ViewKind::Legend,
    ];
    let keys: Vec<&str> = kinds.iter().map(ViewKind::key).collect();
    assert_eq!(keys, ViewKind::KEYS);
    let shows: Vec<bool> = kinds.iter().map(ViewKind::shows_model).collect();
    assert_eq!(shows, [true, true, true, true, true, false, false]);
}

#[test]
fn a_view_range_resolves_to_heights_through_the_levels() {
    let levels = levels();
    let range = ViewRange {
        top: Tie {
            level: id("l2"),
            offset: mm(-900),
        },
        cut: Tie {
            level: id("l1"),
            offset: mm(1200),
        },
        bottom: Tie {
            level: id("l1"),
            offset: mm(0),
        },
        depth: Tie {
            level: id("l1"),
            offset: Length::from_steps(-3_200_000),
        },
    };
    let heights = range.resolve(&levels).unwrap();
    assert_eq!(heights.top, mm(2300));
    assert_eq!(heights.cut, mm(1200));
    assert_eq!(heights.bottom, mm(0));
    assert_eq!(heights.depth.to_string(), "-500 mm");
    assert_eq!(range.levels(), [&id("l2"), &id("l1"), &id("l1"), &id("l1")]);
}

#[test]
fn a_view_range_is_refused_out_of_order_or_with_a_level_that_is_gone() {
    let levels = levels();
    let upside_down = ViewRange::on_level(&id("l1"), mm(0), mm(1200), mm(0), mm(-500));
    assert!(matches!(
        upside_down.resolve(&levels),
        Err(ViewError::RangeOrder { .. })
    ));
    let flat = ViewRange::on_level(&id("l1"), mm(1200), mm(1200), mm(1200), mm(1200));
    assert!(flat.resolve(&levels).is_ok());
    let gone = ViewRange::on_level(&id("l9"), mm(2300), mm(1200), mm(0), mm(-500));
    assert_eq!(
        gone.resolve(&levels),
        Err(ViewError::UnknownLevel(id("l9")))
    );
    let huge = ViewRange::on_level(&id("l1"), Length::from_steps(i64::MAX), mm(0), mm(0), mm(0));
    let mut high = Levels::new();
    high.add(Level::new(id("l1"), "High", Length::from_steps(1), true).unwrap())
        .unwrap();
    assert!(matches!(huge.resolve(&high), Err(ViewError::OutOfRange(_))));
}

#[test]
fn a_cut_line_needs_two_ends_and_a_depth() {
    let at = |x, y| Point { x: mm(x), y: mm(y) };
    assert_eq!(
        CutLine::new(at(5, 5), at(5, 5), Side::Left, mm(100)),
        Err(ViewError::EmptyLine)
    );
    assert_eq!(
        CutLine::new(at(0, 0), at(5, 5), Side::Left, mm(0)),
        Err(ViewError::NoDepth)
    );
    assert_eq!(
        CutLine::new(at(0, 0), at(5, 5), Side::Left, mm(-1)),
        Err(ViewError::NoDepth)
    );
    let line = CutLine::new(at(0, 0), at(5, 5), Side::Right, Length::from_steps(80_000)).unwrap();
    assert_eq!(line.far().to_string(), "12.5 mm");
    assert_eq!(line.look(), Side::Right);
    assert_eq!((line.from(), line.to()), (at(0, 0), at(5, 5)));
    assert_eq!(Side::from_key(Side::Left.key()), Some(Side::Left));
    assert_eq!(Side::from_key("up"), None);
}

#[test]
fn a_view_stands_in_a_model_when_its_levels_and_phases_do() {
    let (levels, categories, phases, options) = (
        levels(),
        CategoryTable::new(),
        phases(),
        DesignOptions::new(),
    );
    let model = Model {
        levels: &levels,
        categories: &categories,
        phases: &phases,
        options: &options,
    };
    assert_eq!(view("v", "Plan", plan("l1")).check(&model), Ok(()));
    assert_eq!(
        view("v", "Plan", plan("l9")).check(&model),
        Err(ViewError::UnknownLevel(id("l9")))
    );
    let lost_range = ViewKind::FloorPlan(PlanSetup {
        level: id("l1"),
        range: ViewRange::on_level(&id("l7"), mm(2300), mm(1200), mm(0), mm(-500)),
    });
    assert_eq!(
        view("v", "Plan", lost_range).check(&model),
        Err(ViewError::UnknownLevel(id("l7")))
    );
    let settings = ViewSettings {
        phase: id("ph-none"),
        filter: PhaseFilter::ShowAll,
        choices: Vec::new(),
    };
    assert_eq!(
        view("v", "3D", ViewKind::ThreeD)
            .with_phasing(Some(settings))
            .check(&model),
        Err(ViewError::Phase(PhaseError::UnknownPhase(id("ph-none"))))
    );
    let mut lost = staging();
    lost.created = id("ph-none");
    assert!(matches!(
        view("v", "3D", ViewKind::ThreeD)
            .with_staging(lost)
            .check(&model),
        Err(ViewError::Phase(_))
    ));
}

#[test]
fn the_views_scale_picks_the_pen_width() {
    let pens = default_model_pens();
    let heaviest = pens.pens().last().unwrap().number;
    let at = |denominator| {
        view("v", "x", ViewKind::ThreeD)
            .with_scale(Scale::new(denominator).unwrap())
            .pen_width(&pens, heaviest)
            .unwrap()
    };
    assert!(at(20) > at(500));
    assert_eq!(
        at(100),
        pens.width(heaviest, Scale::new(100).unwrap()).unwrap()
    );
    assert!(matches!(
        view("v", "x", ViewKind::ThreeD).pen_width(&pens, 60_000),
        Err(ViewError::Style(_))
    ));
}

#[test]
fn a_document_of_views_keeps_names_unique_without_regard_to_case() {
    let mut views = Views::new();
    views.add(view("v1", "Level 1", plan("l1"))).unwrap();
    assert_eq!(
        views.add(view("v2", "LEVEL 1", ViewKind::ThreeD)),
        Err(ViewError::DuplicateName("LEVEL 1".to_owned()))
    );
    assert_eq!(
        views.add(view("v1", "Another", ViewKind::ThreeD)),
        Err(ViewError::DuplicateView(id("v1")))
    );
    views.add(view("v2", "3D", ViewKind::ThreeD)).unwrap();
    // A view may change the case of its own name, and not take another's.
    views.replace(view("v1", "LEVEL 1", plan("l1"))).unwrap();
    assert_eq!(
        views.replace(view("v2", "level 1", ViewKind::ThreeD)),
        Err(ViewError::DuplicateName("level 1".to_owned()))
    );
    assert_eq!(
        views.replace(view("v9", "Nine", ViewKind::ThreeD)),
        Err(ViewError::UnknownView(id("v9")))
    );
    let order: Vec<&str> = views.ordered().iter().map(|v| v.name()).collect();
    assert_eq!(order, ["3D", "LEVEL 1"]);
    assert_eq!(views.len(), 2);
    assert_eq!(views.remove(&id("v2")).unwrap().name(), "3D");
    assert_eq!(
        views.remove(&id("v2")),
        Err(ViewError::UnknownView(id("v2")))
    );
    assert!(views.get(&id("v1")).is_some());
    assert!(!views.is_empty());
}

#[test]
fn option_choices_are_kept_in_the_order_of_their_sets() {
    let choice = |set: &str| construct_phases::OptionChoice {
        set: id(set),
        option: id("o"),
    };
    let settings = |choices| ViewSettings {
        phase: id("ph-a"),
        filter: PhaseFilter::ShowAll,
        choices,
    };
    let one = view("v", "x", ViewKind::ThreeD)
        .with_phasing(Some(settings(vec![choice("b"), choice("a")])));
    let two = view("v", "x", ViewKind::ThreeD)
        .with_phasing(Some(settings(vec![choice("a"), choice("b")])));
    assert_eq!(one, two);
}
