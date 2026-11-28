//! View templates, per-element overrides and view filters (CON-A8-F3-P4, CON-A8-F3-P6): a template
//! tied to two views changes both, a tied view's change to an included setting is refused, a
//! template applied once copies, an override is cleared, two filters decide in order, and every
//! stored form round-trips and refuses any other spelling.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::{CategoryId, CategoryTable, DOORS, WALLS};
use construct_params::{Kind, Length, Value};
use construct_phases::{DesignOptions, ElementStaging, PhaseList};
use construct_styles::patterns::{DASHED, DOTTED};
use construct_styles::{Colour, LineStyles, ObjectStyles};
use construct_views::{
    CutLine, DetailLevel, Edge, ElementStyle, FILTERS_KEY, Filter, Filters, HIDDEN_KEY,
    OVERRIDES_KEY, Operator, Overrides, PlanSetup, Point, RuleStyle, Scale, Setting, Side,
    TEMPLATE_KIND, TEMPLATE_TIE_KEY, View, ViewError, ViewKind, ViewRange, ViewTemplate,
    ViewTemplates, Views, adjust, resolve,
};
use kit_core::Id;
use std::collections::BTreeSet;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(n: i32) -> Length {
    Length::from_mm(n)
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

fn view(key: &str, name: &str, kind: ViewKind, scale: u32) -> View {
    View::new(
        id(key),
        name,
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

fn pen(at: usize) -> u16 {
    lines().model_pens.pens().get(at).unwrap().number
}

fn walls_rule() -> RuleStyle {
    RuleStyle {
        pen: pen(2),
        pattern: DASHED,
        colour: Colour(200, 10, 20),
        fill: None,
        halftone: true,
        transparency: 30,
        shown: true,
    }
}

/// A template of scale 1:50 and fine detail with one graphics rule, every setting included.
fn template() -> ViewTemplate {
    let source = view("v0", "Source", plan(), 50)
        .with_detail(DetailLevel::Fine)
        .with_graphics(construct_views::Graphics::new().with_style(WALLS, Edge::Cut, walls_rule()));
    ViewTemplate::from_view(id("t1"), "Working plan", &source).unwrap()
}

fn two_views() -> Views {
    let mut views = Views::new();
    views
        .add(view("v1", "Level 1", plan(), 100).with_template(Some(id("t1"))))
        .unwrap();
    views
        .add(view("v2", "Section A", section(), 200).with_template(Some(id("t1"))))
        .unwrap();
    views
}

fn templates_of(template: ViewTemplate) -> ViewTemplates {
    let mut templates = ViewTemplates::new();
    templates.add(template).unwrap();
    templates
}

#[test]
fn a_template_tied_to_two_views_changes_both() {
    let views = two_views();
    let mut templates = templates_of(template());
    let shown = templates.effective(&views);
    for key in ["v1", "v2"] {
        let one = shown.get(&id(key)).unwrap();
        assert_eq!(one.scale().denominator(), 50, "{key}");
        assert_eq!(one.detail(), DetailLevel::Fine);
        assert_eq!(
            one.graphics().style(WALLS, Edge::Cut),
            Some(&walls_rule()),
            "{key}"
        );
    }
    // What the views are called and what kind they are is their own.
    assert_eq!(shown.get(&id("v1")).unwrap().name(), "Level 1");
    assert_eq!(shown.get(&id("v2")).unwrap().kind().key(), "section");

    // The template changes: both views change with it, and the views were not touched.
    let mut settings = templates.get(&id("t1")).unwrap().settings().clone();
    settings.scale = Scale::new(20).unwrap();
    settings.graphics = settings.graphics.without(WALLS, None);
    let changed = templates
        .get(&id("t1"))
        .unwrap()
        .clone()
        .with_settings(settings);
    templates.replace(changed).unwrap();
    let again = templates.effective(&views);
    for key in ["v1", "v2"] {
        let one = again.get(&id(key)).unwrap();
        assert_eq!(one.scale().denominator(), 20, "{key}");
        assert!(one.graphics().is_empty(), "{key}");
    }
    assert_eq!(views.get(&id("v1")).unwrap().scale().denominator(), 100);
    assert_eq!(templates.tied_views(&id("t1"), &views).len(), 2);
}

#[test]
fn only_the_included_settings_reach_a_tied_view() {
    let views = two_views();
    let included: BTreeSet<Setting> = [Setting::Graphics].into();
    let templates = templates_of(template().with_included(included));
    let shown = templates.effective(&views);
    let one = shown.get(&id("v1")).unwrap();
    // Graphics are the template's; scale and detail are still the view's own.
    assert_eq!(one.graphics().style(WALLS, Edge::Cut), Some(&walls_rule()));
    assert_eq!(one.scale().denominator(), 100);
    assert_eq!(one.detail(), DetailLevel::Medium);
}

#[test]
fn a_tied_view_cannot_change_an_included_setting_and_says_so() {
    let views = two_views();
    let templates = templates_of(template());
    let tied = views.get(&id("v1")).unwrap();
    for setting in Setting::ALL {
        let refusal = templates.tied_refusal(tied, setting).unwrap_err();
        assert!(matches!(refusal, ViewError::TiedSetting { .. }));
    }
    assert_eq!(
        templates
            .tied_refusal(tied, Setting::Scale)
            .unwrap_err()
            .to_string(),
        "Level 1 follows the view template Working plan, which sets its scale: change it in \
         the template, or untie the view first."
    );
    // A setting the template does not include is the view's own.
    let templates = templates_of(template().with_included([Setting::Graphics].into()));
    assert!(templates.tied_refusal(tied, Setting::Scale).is_ok());
    assert!(templates.tied_refusal(tied, Setting::Graphics).is_err());
    // A view tied to nothing is free.
    let free = view("v3", "Free", plan(), 100);
    assert!(templates.tied_refusal(&free, Setting::Graphics).is_ok());
}

#[test]
fn a_template_applied_once_copies_and_lets_go() {
    let templates = templates_of(template());
    let free = view("v3", "Free", plan(), 100);
    let copied = templates.get(&id("t1")).unwrap().copy_into(&free);
    assert_eq!(copied.scale().denominator(), 50);
    assert_eq!(
        copied.graphics().style(WALLS, Edge::Cut),
        Some(&walls_rule())
    );
    assert_eq!(copied.template(), None);
    // The view is free to change what it copied, and a later template change does not reach it.
    let moved = copied.clone().with_scale(Scale::new(10).unwrap());
    assert_eq!(moved.scale().denominator(), 10);
    let mut views = Views::new();
    views.add(moved).unwrap();
    let mut settings = templates.get(&id("t1")).unwrap().settings().clone();
    settings.scale = Scale::new(5).unwrap();
    let mut later = templates.clone();
    later
        .replace(
            templates
                .get(&id("t1"))
                .unwrap()
                .clone()
                .with_settings(settings),
        )
        .unwrap();
    assert_eq!(
        later
            .effective(&views)
            .get(&id("v3"))
            .unwrap()
            .scale()
            .denominator(),
        10
    );
}

#[test]
fn a_template_fits_each_kind_of_view_and_refuses_drafting_and_legend() {
    let three_d = view("v4", "Model", ViewKind::ThreeD, 100);
    let applied = template().apply_to(&three_d);
    // A cut rule is not one a three-dimensional view can hold: it is left out.
    assert!(applied.graphics().is_empty());
    let mut both = template().settings().clone();
    both.graphics = both
        .graphics
        .with_style(WALLS, Edge::Projected, walls_rule());
    let applied = template().with_settings(both).apply_to(&three_d);
    assert_eq!(
        applied.graphics().style(WALLS, Edge::Projected),
        Some(&walls_rule())
    );
    assert_eq!(applied.graphics().style(WALLS, Edge::Cut), None);
    assert!(applied.graphics().check_kind(&ViewKind::ThreeD).is_ok());
    for kind in [ViewKind::Drafting, ViewKind::Legend] {
        let refusal = ViewTemplates::can_tie(&view("v5", "Draft", kind, 50)).unwrap_err();
        assert!(
            matches!(refusal, ViewError::NoTemplateOnKind { .. }),
            "{refusal}"
        );
    }
    assert!(ViewTemplates::can_tie(&three_d).is_ok());
}

#[test]
fn template_names_are_unique_and_a_missing_template_leaves_the_view_alone() {
    let mut templates = templates_of(template());
    let other = ViewTemplate::new(
        id("t2"),
        "  WORKING PLAN ",
        template().settings().clone(),
        BTreeSet::new(),
    )
    .unwrap();
    assert_eq!(
        templates.add(other.clone()).unwrap_err(),
        ViewError::DuplicateTemplateName("WORKING PLAN".to_owned())
    );
    assert_eq!(
        templates.replace(other).unwrap_err(),
        ViewError::UnknownTemplate(id("t2"))
    );
    assert_eq!(
        ViewTemplate::new(
            id("t3"),
            "   ",
            template().settings().clone(),
            BTreeSet::new()
        )
        .unwrap_err(),
        ViewError::EmptyTemplateName
    );
    // A view tied to a template this document lacks shows its own settings.
    let views = two_views();
    let empty = ViewTemplates::new();
    assert_eq!(empty.effective(&views), views);
}

#[test]
fn a_template_record_round_trips_and_refuses_any_other_spelling() {
    let doc = id("doc");
    let mut settings = template().settings().clone();
    settings.hidden = [CategoryId(300), CategoryId(10)].into();
    let (filters, _) = Filters::new()
        .added(filter("Fire", WALLS, Operator::Equals, "1h"))
        .unwrap();
    settings.filters = filters;
    let made = template()
        .with_settings(settings)
        .with_included([Setting::Scale, Setting::Filters].into());
    let record = made.to_record(&doc);
    assert_eq!(record.kind, TEMPLATE_KIND);
    assert_eq!(record.props.get("@vt.included").unwrap(), "scale,filters");
    assert_eq!(record.props.get(HIDDEN_KEY).unwrap(), "10,300");
    assert_eq!(ViewTemplate::from_record(&record).unwrap(), made);
    assert_eq!(made.to_record(&doc), record, "the same bytes");

    // Another spelling of the same template is refused.
    let mut wrong = record.clone();
    wrong
        .props
        .insert("@vt.included".to_owned(), "filters,scale".to_owned());
    assert!(ViewTemplate::from_record(&wrong).is_err());
    let mut wrong = record.clone();
    wrong
        .props
        .insert(HIDDEN_KEY.to_owned(), "300,10".to_owned());
    assert!(ViewTemplate::from_record(&wrong).is_err());
    let mut wrong = record.clone();
    wrong.props.insert("@vt.extra".to_owned(), "x".to_owned());
    assert!(ViewTemplate::from_record(&wrong).is_err());
    let mut wrong = record;
    wrong.kind = "view".to_owned();
    assert!(ViewTemplate::from_record(&wrong).is_err());
}

#[test]
fn a_view_keeps_its_tie_hidden_categories_filters_and_overrides() {
    let doc = id("doc");
    let (filters, _) = Filters::new()
        .added(filter("Fire", WALLS, Operator::Equals, "1h"))
        .unwrap();
    let overrides = Overrides::new().with(
        &id("w-1"),
        ElementStyle {
            colour: Some(Colour(1, 2, 3)),
            shown: Some(false),
            ..ElementStyle::default()
        },
    );
    let one = view("v1", "Level 1", plan(), 100)
        .with_template(Some(id("t1")))
        .with_hidden([DOORS].into())
        .with_filters(filters)
        .with_overrides(overrides);
    let record = one.to_record(&doc);
    assert_eq!(record.props.get(TEMPLATE_TIE_KEY).unwrap(), "t1");
    assert!(record.props.contains_key(FILTERS_KEY));
    assert!(record.props.contains_key(OVERRIDES_KEY));
    assert_eq!(View::from_record(&record).unwrap(), one);
    // A view with none of them writes none of the keys: every view stored before reads as free.
    let plain = view("v1", "Level 1", plan(), 100).to_record(&doc);
    for key in [TEMPLATE_TIE_KEY, FILTERS_KEY, OVERRIDES_KEY, HIDDEN_KEY] {
        assert!(!plain.props.contains_key(key), "{key}");
    }
    // Other spellings are refused.
    let mut wrong = record.clone();
    wrong.props.insert(
        OVERRIDES_KEY.to_owned(),
        r#"{"format":2,"overrides":[]}"#.to_owned(),
    );
    assert!(View::from_record(&wrong).is_err());
    let mut wrong = record;
    wrong.props.insert(
        OVERRIDES_KEY.to_owned(),
        r#"{"format":1,"overrides":[{"element":"w-1","style":{}}]}"#.to_owned(),
    );
    assert!(View::from_record(&wrong).is_err());
}

#[test]
fn an_override_is_set_merged_and_cleared() {
    let wall = id("w-1");
    let door = id("d-1");
    let hide = ElementStyle {
        shown: Some(false),
        ..ElementStyle::default()
    };
    let dash = ElementStyle {
        pattern: Some(DOTTED),
        halftone: Some(true),
        ..ElementStyle::default()
    };
    let set = Overrides::new()
        .with(&wall, hide.clone())
        .with(&door, dash.clone());
    assert_eq!(set.get(&wall), Some(&hide));
    assert_eq!(set.iter().count(), 2);
    // Clearing one element's leaves the other's; clearing with none takes them all.
    let one_left = set.without(Some(&wall));
    assert_eq!(one_left.get(&wall), None);
    assert_eq!(one_left.get(&door), Some(&dash));
    assert!(set.without(None).is_empty());
    // An empty style is no override at all.
    assert!(
        set.with(&wall, ElementStyle::default())
            .get(&wall)
            .is_none()
    );
    // Overrides are the view's own: a tied view keeps them.
    let views = {
        let mut views = Views::new();
        views
            .add(
                view("v1", "Level 1", plan(), 100)
                    .with_template(Some(id("t1")))
                    .with_overrides(set.clone()),
            )
            .unwrap();
        views
    };
    let shown = templates_of(template()).effective(&views);
    assert_eq!(shown.get(&id("v1")).unwrap().overrides(), &set);
}

fn filter(name: &str, category: CategoryId, op: Operator, text: &str) -> Filter {
    Filter {
        id: 0,
        name: name.to_owned(),
        category,
        parameter: id("p-fire"),
        op,
        value: Value::Text(text.to_owned()),
        style: ElementStyle::default(),
    }
}

/// A reader of one element's parameters: the fire rating as text.
fn reads(rating: &'static str) -> impl Fn(&Id) -> Option<Value> {
    move |parameter| (parameter == &id("p-fire")).then(|| Value::Text(rating.to_owned()))
}

#[test]
fn two_filters_decide_in_order_and_the_order_can_change() {
    let categories = CategoryTable::new();
    let red = ElementStyle {
        colour: Some(Colour(255, 0, 0)),
        pen: Some(pen(4)),
        ..ElementStyle::default()
    };
    let hidden = ElementStyle {
        colour: Some(Colour(0, 0, 255)),
        shown: Some(false),
        ..ElementStyle::default()
    };
    let first = Filter {
        style: red.clone(),
        ..filter("Rated", WALLS, Operator::Contains, "h")
    };
    let second = Filter {
        style: hidden.clone(),
        ..filter("One hour", WALLS, Operator::Equals, "1h")
    };
    let (filters, a) = Filters::new().added(first).unwrap();
    let (filters, b) = filters.added(second).unwrap();
    assert_eq!((a, b), (1, 2));

    let one_hour = reads("1h");
    // Both match: the first wins the colour and the pen it names, the second adds what it alone
    // names (hiding).
    let both = filters.style_for(&categories, WALLS, &one_hour);
    assert_eq!(both.colour, Some(Colour(255, 0, 0)));
    assert_eq!(both.pen, Some(pen(4)));
    assert_eq!(both.shown, Some(false));

    // Move the second to the top: now it wins the colour.
    let flipped = filters.moved(b, 0).unwrap();
    let both = flipped.style_for(&categories, WALLS, &one_hour);
    assert_eq!(both.colour, Some(Colour(0, 0, 255)));
    assert_eq!(both.pen, Some(pen(4)));
    assert_eq!(flipped.iter().next().unwrap().id, b);

    // Only the first matches a 2h wall; neither a door; the parameter change is seen at once.
    let wall_2h = filters.style_for(&categories, WALLS, &reads("2h"));
    assert_eq!(wall_2h.colour, Some(Colour(255, 0, 0)));
    assert_eq!(wall_2h.shown, None);
    assert!(filters.style_for(&categories, DOORS, &one_hour).is_empty());
    assert!(
        filters
            .style_for(&categories, WALLS, &reads("none"))
            .is_empty()
    );
    assert!(
        filters
            .style_for(&categories, WALLS, &|_: &Id| None)
            .is_empty()
    );
    // A subcategory of walls matches a filter on walls.
    let sub = CategoryId(101);
    assert_eq!(categories.get(sub).unwrap().parent, Some(WALLS));
    assert_eq!(
        filters.style_for(&categories, sub, &one_hour).colour,
        Some(Colour(255, 0, 0))
    );
}

#[test]
fn filters_compare_by_their_operators_and_check_the_kind_they_can_read() {
    let categories = CategoryTable::new();
    let number = |op: Operator, bound: Value| Filter {
        value: bound,
        ..filter("Over", WALLS, op, "")
    };
    let read_len = |n: i32| move |_: &Id| Some(Value::Length(mm(n)));
    let above = number(Operator::GreaterThan, Value::Length(mm(200)));
    assert!(above.matches(&categories, WALLS, &read_len(300)));
    assert!(!above.matches(&categories, WALLS, &read_len(200)));
    assert!(!above.matches(&categories, WALLS, &read_len(100)));
    // The same kind only: a whole number does not stand against a length.
    assert!(!above.matches(&categories, WALLS, &|_: &Id| Some(Value::Whole(900))));
    let whole = number(Operator::GreaterThan, Value::Whole(2));
    assert!(whole.matches(&categories, WALLS, &|_: &Id| Some(Value::Whole(3))));
    assert!(!whole.matches(&categories, WALLS, &|_: &Id| Some(Value::Whole(2))));
    // Contains is text, without regard to case.
    let has = filter("Has", WALLS, Operator::Contains, "FIRE");
    assert!(has.matches(&categories, WALLS, &reads("Fire rated 1h")));
    assert!(!has.matches(&categories, WALLS, &reads("Acoustic")));
    assert!(!has.matches(&categories, WALLS, &|_: &Id| Some(Value::Whole(1))));
    // Equals is exact.
    let eq = filter("Eq", WALLS, Operator::Equals, "1h");
    assert!(eq.matches(&categories, WALLS, &reads("1h")));
    assert!(!eq.matches(&categories, WALLS, &reads("1H")));
    // What an operator can be given.
    assert!(Operator::Equals.fits(Kind::YesNo));
    assert!(Operator::Contains.fits(Kind::Text) && !Operator::Contains.fits(Kind::Length));
    assert!(Operator::GreaterThan.fits(Kind::Length) && !Operator::GreaterThan.fits(Kind::Text));
    assert_eq!(
        Operator::from_key("greater_than"),
        Some(Operator::GreaterThan)
    );
    assert_eq!(Operator::from_key("below"), None);
}

#[test]
fn the_list_of_filters_refuses_what_it_cannot_hold_and_stores_one_way() {
    let (filters, one) = Filters::new()
        .added(filter("A", WALLS, Operator::Equals, "x"))
        .unwrap();
    assert_eq!(
        filters
            .added(filter(" a ", WALLS, Operator::Equals, "y"))
            .unwrap_err(),
        ViewError::DuplicateFilterName("a".to_owned())
    );
    assert_eq!(
        filters
            .added(filter(" ", WALLS, Operator::Equals, "y"))
            .unwrap_err(),
        ViewError::EmptyFilterName
    );
    assert_eq!(filters.removed(9).unwrap_err(), ViewError::UnknownFilter(9));
    assert_eq!(
        filters.moved(one, 1).unwrap_err(),
        ViewError::FilterPosition { to: 1, len: 1 }
    );
    let renamed = filters
        .replaced(Filter {
            id: one,
            ..filter("Renamed", WALLS, Operator::Contains, "y")
        })
        .unwrap();
    assert_eq!(renamed.get(one).unwrap().name, "Renamed");
    assert_eq!(filters.removed(one).unwrap().len(), 0);
    // A removed number is not reused while a higher one is in the list.
    let (two, b) = filters
        .added(filter("B", WALLS, Operator::Equals, "z"))
        .unwrap();
    let (three, c) = two
        .removed(one)
        .unwrap()
        .added(filter("C", WALLS, Operator::Equals, "w"))
        .unwrap();
    assert_eq!((b, c), (2, 3));
    assert_eq!(three.len(), 2);
    // The stored text round-trips through a view record.
    let doc = id("doc");
    let record = view("v1", "V", plan(), 100)
        .with_filters(two)
        .to_record(&doc);
    assert!(View::from_record(&record).is_ok());
    let mut wrong = record;
    wrong.props.insert(
        FILTERS_KEY.to_owned(),
        r#"{"format":1,"filters":[{"id":0,"name":"A","category":100,"parameter":"p","op":"equals","value":"text:x","style":{}}]}"#
            .to_owned(),
    );
    assert!(View::from_record(&wrong).is_err());
    // A filter of a category that does not exist is asked of the table.
    let bad = Filters::new()
        .added(filter("Bad", CategoryId(31), Operator::Equals, "x"))
        .unwrap()
        .0;
    assert_eq!(
        bad.check(&CategoryTable::new()),
        Err(ViewError::UnknownCategory(CategoryId(31)))
    );
}

#[test]
fn a_hidden_category_is_not_shown_and_an_element_style_goes_over_the_answer() {
    let categories = CategoryTable::new();
    let objects = ObjectStyles::default_metric();
    let lines = lines();
    let open = view("v1", "V", plan(), 100);
    let hidden = open.clone().with_hidden([WALLS].into());
    let ask =
        |v: &View, edge| resolve(v, &categories, &objects, &lines, WALLS, None, edge).unwrap();
    assert!(ask(&open, Edge::Cut).style.shown);
    for edge in [Edge::Cut, Edge::Projected] {
        assert!(!ask(&hidden, edge).style.shown);
    }
    // A hidden subcategory hides itself only.
    let sub_hidden = open.with_hidden([CategoryId(101)].into());
    let sub = |v: &View| {
        resolve(
            v,
            &categories,
            &objects,
            &lines,
            WALLS,
            Some(CategoryId(101)),
            Edge::Cut,
        )
        .unwrap()
        .style
        .shown
    };
    assert!(!sub(&sub_hidden));
    assert!(ask(&sub_hidden, Edge::Cut).style.shown);

    // An element's style: the pen, colour, pattern, halftone and shown are put over the answer
    // and the printed width follows the pen.
    let base = ask(&view("v1", "V", plan(), 100), Edge::Cut);
    let style = ElementStyle {
        pen: Some(pen(5)),
        colour: Some(Colour(9, 9, 9)),
        pattern: Some(DOTTED),
        halftone: Some(true),
        shown: Some(false),
    };
    let kind = categories.get(WALLS).unwrap().kind;
    let done = adjust(&base, &style, &lines, kind, Scale::new(100).unwrap()).unwrap();
    assert_eq!(done.style.pen, pen(5));
    assert_eq!(done.style.colour, Colour(9, 9, 9));
    assert_eq!(done.style.pattern, DOTTED);
    assert!(done.style.halftone && !done.style.shown);
    assert_eq!(
        done.width,
        lines
            .model_pens
            .width(pen(5), Scale::new(100).unwrap())
            .unwrap()
    );
    // A pen that is not in the table is left out rather than stopping a drawing.
    let missing = ElementStyle {
        pen: Some(9999),
        ..ElementStyle::default()
    };
    let same = adjust(
        &base,
        &missing,
        &lines,
        kind,
        Scale::new(100).unwrap(),
    )
    .unwrap();
    assert_eq!(same, base);
    assert_eq!(
        missing.check(&lines, kind),
        Err(ViewError::NoSuchPen { pen: 9999 })
    );
}
