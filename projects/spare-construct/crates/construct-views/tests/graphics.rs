//! A view's graphics rules: the resolution rule at each level, every refusal, the stored form and
//! its round trip, and old views that have no rules.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::{
    BUILT_IN, CategoryId, CategoryKind, CategoryTable, DOORS, VIEWS, WALLS,
};
use construct_params::Length;
use construct_phases::{DesignOptions, ElementStaging, PhaseList};
use construct_styles::patterns::{DASHED, DOTTED, SOLID};
use construct_styles::{Colour, LineStyles, PatternId};
use construct_styles::{ObjectStyles, Scale};
use construct_views::{
    CutLine, DetailLevel, Edge, FILL_PATTERNS, Fill, GRAPHICS_KEY, Graphics, Origin, PlanSetup,
    Point, RuleStyle, Side, View, ViewError, ViewKind, ViewRange, edge_from_key, edge_key, resolve,
};
use kit_core::Id;

const CORE_CUT_LINE: CategoryId = CategoryId(101);
const FINISH_CUT_LINE: CategoryId = CategoryId(102);
const DOOR_SWING: CategoryId = CategoryId(304);
const DOOR_PANEL: CategoryId = CategoryId(301);

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i32) -> Length {
    Length::from_mm(mm)
}

fn staging() -> ElementStaging {
    let phases = PhaseList::new().add(id("ph-a"), "Existing").unwrap();
    ElementStaging::new(&phases, &DesignOptions::new(), id("ph-a"), None, None).unwrap()
}

fn plan() -> ViewKind {
    ViewKind::FloorPlan(PlanSetup {
        level: id("l1"),
        range: ViewRange::on_level(&id("l1"), mm(2300), mm(1200), mm(0), mm(-500)),
    })
}

fn section() -> ViewKind {
    let line = CutLine::new(
        Point { x: mm(0), y: mm(0) },
        Point {
            x: mm(6000),
            y: mm(0),
        },
        Side::Left,
        mm(5000),
    )
    .unwrap();
    ViewKind::Section(line)
}

fn view_of(kind: ViewKind, scale: u32) -> View {
    View::new(
        id("v1"),
        "View",
        Scale::new(scale).unwrap(),
        DetailLevel::Medium,
        kind,
        staging(),
    )
    .unwrap()
}

fn lines() -> LineStyles {
    LineStyles::default_metric()
}

fn a_pen(lines: &LineStyles, at: usize) -> u16 {
    lines.model_pens.pens().get(at).unwrap().number
}

/// A style that differs from every project default in each of its values.
fn red(lines: &LineStyles) -> RuleStyle {
    RuleStyle {
        pen: a_pen(lines, 0),
        pattern: DOTTED,
        colour: Colour(200, 10, 20),
        fill: None,
        halftone: true,
        transparency: 40,
        shown: true,
    }
}

fn blue(lines: &LineStyles) -> RuleStyle {
    RuleStyle {
        pen: a_pen(lines, 1),
        pattern: DASHED,
        colour: Colour(10, 20, 200),
        fill: Some(Fill {
            pattern: "concrete".to_owned(),
            colour: Colour(1, 2, 3),
        }),
        halftone: false,
        transparency: 0,
        shown: false,
    }
}

/// What a view says of an element of walls, in a subcategory or not, at one edge.
fn answer(
    view: &View,
    category: CategoryId,
    sub: Option<CategoryId>,
    edge: Edge,
) -> Result<(RuleStyle, Origin), ViewError> {
    let categories = CategoryTable::new();
    resolve(
        view,
        &categories,
        &ObjectStyles::default_metric(),
        &lines(),
        category,
        sub,
        edge,
    )
    .map(|found| (found.style, found.origin))
}

#[test]
fn a_view_with_no_rules_looks_as_the_project_says() {
    let view = view_of(plan(), 100);
    assert!(view.graphics().is_empty());
    let objects = ObjectStyles::default_metric();
    let categories = CategoryTable::new();
    for (category, sub) in [(WALLS, None), (WALLS, Some(CORE_CUT_LINE)), (DOORS, None)] {
        for edge in [Edge::Cut, Edge::Projected] {
            let found =
                resolve(&view, &categories, &objects, &lines(), category, sub, edge).unwrap();
            assert_eq!(found.origin, Origin::Project);
            let want = objects.lookup(
                &categories,
                &lines(),
                sub.unwrap_or(category),
                edge,
                view.scale(),
            );
            let want = want.unwrap();
            assert_eq!(found.width, want.width);
            assert_eq!(found.style.colour, want.colour);
            assert_eq!(found.style.pattern, want.pattern);
            assert_eq!(found.style.fill, None);
            assert!(!found.style.halftone && found.style.shown);
            assert_eq!(found.style.transparency, 0);
        }
    }
}

#[test]
fn the_subcategory_beats_the_category_beats_the_project() {
    let lines = lines();
    let (red, blue) = (red(&lines), blue(&lines));
    let graphics = Graphics::new()
        .with_style(WALLS, Edge::Cut, red.clone())
        .with_style(CORE_CUT_LINE, Edge::Cut, blue.clone());
    let view = view_of(plan(), 100).with_graphics(graphics);
    // The subcategory's own rule.
    assert_eq!(
        answer(&view, WALLS, Some(CORE_CUT_LINE), Edge::Cut).unwrap(),
        (blue, Origin::Subcategory)
    );
    // A subcategory with no rule of its own takes the category's.
    assert_eq!(
        answer(&view, WALLS, Some(FINISH_CUT_LINE), Edge::Cut).unwrap(),
        (red.clone(), Origin::Category)
    );
    assert_eq!(
        answer(&view, WALLS, None, Edge::Cut).unwrap(),
        (red, Origin::Category)
    );
    // The edges are apart: no projection rule, so the project answers, whatever the cut says.
    for sub in [None, Some(CORE_CUT_LINE)] {
        assert_eq!(
            answer(&view, WALLS, sub, Edge::Projected).unwrap().1,
            Origin::Project
        );
    }
    // Another category is not touched by walls' rules.
    assert_eq!(
        answer(&view, DOORS, None, Edge::Cut).unwrap().1,
        Origin::Project
    );
}

#[test]
fn a_subcategorys_projection_rule_leaves_its_cut_to_the_category() {
    let lines = lines();
    let (red, blue) = (red(&lines), blue(&lines));
    let mut seen = blue;
    seen.fill = None;
    let graphics = Graphics::new()
        .with_style(WALLS, Edge::Cut, red.clone())
        .with_style(CORE_CUT_LINE, Edge::Projected, seen.clone());
    let view = view_of(plan(), 100).with_graphics(graphics);
    assert_eq!(
        answer(&view, WALLS, Some(CORE_CUT_LINE), Edge::Projected).unwrap(),
        (seen, Origin::Subcategory)
    );
    assert_eq!(
        answer(&view, WALLS, Some(CORE_CUT_LINE), Edge::Cut).unwrap(),
        (red, Origin::Category)
    );
}

#[test]
fn a_hidden_subcategory_under_a_shown_category_stays_hidden() {
    let lines = lines();
    let mut hidden = red(&lines);
    hidden.shown = false;
    let graphics = Graphics::new().with_style(DOOR_PANEL, Edge::Projected, hidden);
    let view = view_of(plan(), 100).with_graphics(graphics);
    assert!(
        !answer(&view, DOORS, Some(DOOR_PANEL), Edge::Projected)
            .unwrap()
            .0
            .shown
    );
    assert!(answer(&view, DOORS, None, Edge::Projected).unwrap().0.shown);
}

#[test]
fn the_width_is_the_pens_at_the_views_scale() {
    let lines = lines();
    let pen = a_pen(&lines, 3);
    let mut style = red(&lines);
    style.pen = pen;
    for scale in [20, 100, 500] {
        let view = view_of(plan(), scale).with_graphics(Graphics::new().with_style(
            WALLS,
            Edge::Projected,
            style.clone(),
        ));
        let found = resolve(
            &view,
            &CategoryTable::new(),
            &ObjectStyles::default_metric(),
            &lines,
            WALLS,
            None,
            Edge::Projected,
        )
        .unwrap();
        assert_eq!(
            found.width,
            lines
                .model_pens
                .width(pen, Scale::new(scale).unwrap())
                .unwrap()
        );
    }
}

#[test]
fn resolution_refuses_what_it_cannot_answer() {
    let view = view_of(plan(), 100);
    // A cut of a subcategory that cannot be cut, and of a category with none.
    assert_eq!(
        answer(&view, DOORS, Some(DOOR_SWING), Edge::Cut).unwrap_err(),
        ViewError::CutOnUncuttable(DOOR_SWING)
    );
    assert!(answer(&view, DOORS, Some(DOOR_SWING), Edge::Projected).is_ok());
    assert_eq!(
        answer(&view, CategoryId(31), None, Edge::Projected).unwrap_err(),
        ViewError::UnknownCategory(CategoryId(31))
    );
    assert_eq!(
        answer(&view, WALLS, Some(CategoryId(31)), Edge::Projected).unwrap_err(),
        ViewError::UnknownCategory(CategoryId(31))
    );
    assert_eq!(
        answer(&view, CORE_CUT_LINE, None, Edge::Projected).unwrap_err(),
        ViewError::NotTopLevel(CORE_CUT_LINE)
    );
    assert_eq!(
        answer(&view, DOORS, Some(CORE_CUT_LINE), Edge::Projected).unwrap_err(),
        ViewError::NotASubcategory {
            subcategory: CORE_CUT_LINE,
            category: DOORS
        }
    );
}

/// The one kind of each: a category that is annotation, and one that cannot be cut.
fn an_annotation() -> CategoryId {
    BUILT_IN
        .iter()
        .find(|c| c.kind == CategoryKind::Annotation && c.parent.is_none())
        .unwrap()
        .id
}

#[test]
fn every_rule_is_checked_against_the_categories_and_the_pens() {
    let lines = lines();
    let categories = CategoryTable::new();
    let ok = red(&lines);
    let with =
        |id: CategoryId, edge: Edge, style: RuleStyle| Graphics::new().with_style(id, edge, style);
    let check = |g: &Graphics, kind: &ViewKind| g.check(kind, &categories, &lines);

    let fine = with(WALLS, Edge::Cut, ok.clone());
    assert_eq!(check(&fine, &plan()), Ok(()));
    assert_eq!(check(&fine, &section()), Ok(()));
    assert_eq!(check(&Graphics::new(), &ViewKind::Legend), Ok(()));

    // A kind that cannot hold a rule.
    for kind in [ViewKind::Drafting, ViewKind::Legend] {
        let refusal = check(&fine, &kind).unwrap_err();
        assert!(
            matches!(refusal, ViewError::NoGraphicsOnKind { .. }),
            "{refusal}"
        );
    }
    assert_eq!(
        check(&fine, &ViewKind::ThreeD).unwrap_err().to_string(),
        "A three d view cuts nothing, so it has no cut rules."
    );
    assert_eq!(
        check(&with(WALLS, Edge::Projected, ok.clone()), &ViewKind::ThreeD),
        Ok(())
    );

    // A category that is not there, one that is not drawn, one that cannot be cut.
    assert_eq!(
        check(&with(CategoryId(31), Edge::Projected, ok.clone()), &plan()),
        Err(ViewError::UnknownCategory(CategoryId(31)))
    );
    assert_eq!(
        check(&with(VIEWS, Edge::Projected, ok.clone()), &plan()),
        Err(ViewError::NotStyled(VIEWS))
    );
    assert_eq!(
        check(&with(DOOR_SWING, Edge::Cut, ok.clone()), &plan()),
        Err(ViewError::CutOnUncuttable(DOOR_SWING))
    );
    assert_eq!(
        check(&with(DOOR_SWING, Edge::Projected, ok.clone()), &plan()),
        Ok(())
    );

    // Values out of range.
    let mut bad = ok.clone();
    bad.pen = 65_000;
    assert_eq!(
        check(&with(WALLS, Edge::Cut, bad), &plan()),
        Err(ViewError::NoSuchPen { pen: 65_000 })
    );
    let mut bad = ok.clone();
    bad.pattern = PatternId(9999);
    assert_eq!(
        check(&with(WALLS, Edge::Cut, bad), &plan()),
        Err(ViewError::NoSuchPattern { pattern: 9999 })
    );
    let mut bad = ok.clone();
    bad.transparency = 101;
    assert_eq!(
        check(&with(WALLS, Edge::Cut, bad.clone()), &plan()),
        Err(ViewError::TransparencyAbove(101))
    );
    bad.transparency = 100;
    assert_eq!(check(&with(WALLS, Edge::Cut, bad), &plan()), Ok(()));
    let fill = |name: &str| Fill {
        pattern: name.to_owned(),
        colour: Colour(0, 0, 0),
    };
    let mut bad = ok.clone();
    bad.fill = Some(fill("nonsense"));
    let refusal = check(&with(WALLS, Edge::Cut, bad), &plan()).unwrap_err();
    assert_eq!(refusal, ViewError::NoSuchFill("nonsense".to_owned()));
    assert!(refusal.to_string().contains("concrete"));
    for name in FILL_PATTERNS {
        let mut good = ok.clone();
        good.fill = Some(fill(name));
        assert_eq!(check(&with(WALLS, Edge::Cut, good), &plan()), Ok(()));
    }
    let mut bad = ok.clone();
    bad.fill = Some(fill("concrete"));
    assert_eq!(
        check(&with(WALLS, Edge::Projected, bad), &plan()),
        Err(ViewError::FillOnProjection)
    );

    // An annotation category takes the annotation pens: a model pen number may not exist there.
    let note = an_annotation();
    let annotation_pen = lines.annotation_pens.pens().first().unwrap().number;
    let mut style = ok;
    style.pen = annotation_pen;
    assert_eq!(
        check(&with(note, Edge::Projected, style.clone()), &plan()),
        Ok(())
    );
    style.pen = 65_000;
    assert_eq!(
        check(&with(note, Edge::Projected, style), &plan()),
        Err(ViewError::NoSuchPen { pen: 65_000 })
    );
}

#[test]
fn changes_are_pure_and_an_empty_rule_is_no_rule() {
    let lines = lines();
    let one = Graphics::new().with_style(WALLS, Edge::Cut, red(&lines));
    let two = one.with_style(WALLS, Edge::Projected, blue(&lines));
    assert_eq!(one.rule(WALLS).unwrap().projection, None);
    assert!(two.style(WALLS, Edge::Projected).is_some());
    let back = two.without(WALLS, Some(Edge::Projected));
    assert_eq!(back, one);
    assert!(two.without(WALLS, None).is_empty());
    assert!(one.without(WALLS, Some(Edge::Cut)).is_empty());
    assert_eq!(one.without(DOORS, None), one);
    assert_eq!(edge_from_key(edge_key(Edge::Cut)), Some(Edge::Cut));
    assert_eq!(
        edge_from_key(edge_key(Edge::Projected)),
        Some(Edge::Projected)
    );
    assert_eq!(edge_from_key("both"), None);
}

#[test]
fn rules_round_trip_through_the_stored_record() {
    let lines = lines();
    let graphics = Graphics::new()
        .with_style(CORE_CUT_LINE, Edge::Cut, blue(&lines))
        .with_style(WALLS, Edge::Cut, red(&lines))
        .with_style(WALLS, Edge::Projected, {
            let mut s = red(&lines);
            s.pattern = SOLID;
            s
        });
    for kind in [plan(), section()] {
        let view = view_of(kind, 50).with_graphics(graphics.clone());
        let record = view.to_record(&id("doc"));
        let text = record.props.get(GRAPHICS_KEY).unwrap();
        // Ascending category, one spelling, no spare space.
        assert!(text.starts_with(r#"{"format":1,"rules":[{"category":100,"cut":{"pen":"#));
        assert!(!text.contains(' '));
        assert_eq!(View::from_record(&record).unwrap(), view);
        assert_eq!(view.to_record(&id("doc")), record);
    }
    // No rules, no key: a view stored before rules existed reads with none.
    let plain = view_of(plan(), 100);
    let record = plain.to_record(&id("doc"));
    assert!(!record.props.contains_key(GRAPHICS_KEY));
    assert!(View::from_record(&record).unwrap().graphics().is_empty());
}

#[test]
fn a_record_that_is_not_as_written_is_refused() {
    let lines = lines();
    let view = view_of(plan(), 100).with_graphics(Graphics::new().with_style(
        WALLS,
        Edge::Cut,
        red(&lines),
    ));
    let record = view.to_record(&id("doc"));
    let good = record.props.get(GRAPHICS_KEY).unwrap().clone();
    let refuse = |text: String| {
        let mut bad = record.clone();
        bad.props.insert(GRAPHICS_KEY.to_owned(), text);
        let refusal = View::from_record(&bad).unwrap_err();
        assert!(matches!(refusal, ViewError::Malformed(_)), "{refusal}");
        refusal.to_string()
    };
    // Spare space, another version, an unknown field, nothing in it, a colour past 255.
    refuse(good.replace(":[", ": ["));
    refuse(good.replace("\"format\":1", "\"format\":2"));
    refuse(good.replace("\"shown\":true", "\"shown\":true,\"bold\":true"));
    refuse(r#"{"format":1,"rules":[]}"#.to_owned());
    refuse(r#"{"format":1,"rules":[{"category":100,"cut":null,"projection":null}]}"#.to_owned());
    refuse(good.replace("200,10,20", "256,10,20"));
    refuse("rules".to_owned());
    // Two rules not in ascending order, and one rule twice.
    let rule_text = |category: CategoryId| {
        let one = view_of(plan(), 100)
            .with_graphics(Graphics::new().with_style(category, Edge::Cut, red(&lines)))
            .to_record(&id("doc"));
        one.props
            .get(GRAPHICS_KEY)
            .unwrap()
            .trim_start_matches(r#"{"format":1,"rules":["#)
            .trim_end_matches("]}")
            .to_owned()
    };
    let (walls, doors) = (rule_text(WALLS), rule_text(DOORS));
    let both = |first: &str, second: &str| format!(r#"{{"format":1,"rules":[{first},{second}]}}"#);
    let ascending = both(&walls, &doors);
    let mut fine = record.clone();
    fine.props
        .insert(GRAPHICS_KEY.to_owned(), ascending.clone());
    assert!(View::from_record(&fine).is_ok());
    refuse(both(&doors, &walls));
    refuse(both(&walls, &walls));
    // Rules on a drafting view, or a cut rule on a three-dimensional one.
    for kind in ["drafting", "three_d"] {
        let mut other = view_of(ViewKind::Drafting, 100).to_record(&id("doc"));
        other
            .props
            .insert("@view.kind".to_owned(), format!("choice:{kind}"));
        other.props.insert(GRAPHICS_KEY.to_owned(), good.clone());
        let refusal = View::from_record(&other).unwrap_err().to_string();
        assert!(
            refusal.contains("graphics rules it cannot hold"),
            "{refusal}"
        );
    }
}
