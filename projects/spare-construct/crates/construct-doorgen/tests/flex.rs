//! The flex test: each template swept across its allowed range of sizes on the grid, at both
//! ends and in the middle of each (the middles half a millimetre off the whole millimetre), and at
//! the edge of each of its rules from the inside. Every result evaluates, passes the recipe
//! checker and the generator's own checks (crate `checks`, run again from the stored rows by
//! `verify`), and keeps its exact quantities, worked out here by hand. Each rule's edge, one
//! even grid step outside (one step for the even rule), is refused by that rule's name.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{Fixture, expected, make, mm, overall, plus, tenths};
use construct_doorgen::{Rule, Spec, Template, verify};
use construct_families::{Limits, Turn, extents};
use construct_params::Length;

/// Both ends of a range and a middle half a millimetre off the whole millimetre.
fn sweep(low: Length, middle: Length, high: Length) -> [Length; 3] {
    [low, middle, high]
}

/// The sizes each template is swept across.
fn sizes(template: Template) -> ([Length; 3], [Length; 3]) {
    let (w, h) = (template.widths(), template.heights());
    match template {
        Template::SingleSwingDoor => (
            sweep(w.low, tenths(9125), w.high),
            sweep(h.low, tenths(24125), h.high),
        ),
        Template::FixedWindow => (
            sweep(w.low, tenths(16505), w.high),
            sweep(h.low, tenths(16505), h.high),
        ),
        Template::SlidingWindow => (
            sweep(w.low, tenths(22505), w.high),
            sweep(h.low, tenths(15005), h.high),
        ),
    }
}

/// Generates the specs as one family and holds every type to its hand-worked quantities, to the
/// stored rows' own check, and to the boxes construct-families' `extents()` gives.
fn holds(specs: &[Spec]) {
    let generated = make(specs).unwrap_or_else(|r| panic!("{r}"));
    let template = specs[0].template;
    let fixture = Fixture::new();
    assert_eq!(generated.recipe.check(&fixture.library()), Ok(()));
    let again = verify(&generated.recipe, template).unwrap();
    assert_eq!(again.len(), specs.len());
    for ((spec, ty), (_, q)) in specs.iter().zip(&generated.types).zip(&again) {
        let w = ty.quantities.width.steps();
        let h = ty.quantities.height.steps();
        assert_eq!(ty.quantities, expected(spec, w, h), "{}", spec.mark);
        assert_eq!(ty.quantities, *q, "{}", spec.mark);
        let boxes = extents(&generated.recipe, &ty.values, Limits::DEFAULT, Turn::None).unwrap();
        let frame = boxes.iter().find(|b| b.step.as_str() == "frame").unwrap();
        assert_eq!(
            frame.high[0].steps() - frame.low[0].steps(),
            w,
            "{}",
            spec.mark
        );
        assert_eq!(
            frame.high[2].steps() - frame.low[2].steps(),
            h,
            "{}",
            spec.mark
        );
        assert_eq!(
            frame.high[1].steps() - frame.low[1].steps(),
            ty.quantities.depth.steps()
        );
    }
}

#[test]
fn every_template_holds_across_its_whole_range() {
    for template in Template::ALL {
        let (widths, heights) = sizes(template);
        let mut specs = Vec::new();
        for (i, w) in widths.iter().enumerate() {
            for (j, h) in heights.iter().enumerate() {
                specs.push(Spec::new(format!("F{i}{j}"), template, overall(*w, *h)));
            }
        }
        holds(&specs);
        // Each size of the sweep, as asked.
        let generated = make(&specs).unwrap();
        for (spec, ty) in specs.iter().zip(&generated.types) {
            assert_eq!(
                overall(ty.quantities.width, ty.quantities.height),
                spec.size
            );
        }
    }
}

/// A spec of `template`, `w` by `h`, changed by `with`.
fn spec(template: Template, w: Length, h: Length, with: impl FnOnce(&mut Spec)) -> Spec {
    let mut spec = Spec::new("E1", template, overall(w, h));
    with(&mut spec);
    spec
}

/// The spec is built, and keeps its quantities.
fn inside(spec: Spec) {
    holds(&[spec]);
}

/// The spec is refused under `rule`, about `what`.
fn outside(spec: Spec, rule: Rule, what: &str) {
    let refusal = make(&[spec]).unwrap_err();
    assert_eq!(
        (refusal.rule, refusal.what.as_str()),
        (rule, what),
        "{refusal}"
    );
    assert_eq!(refusal.mark.as_deref(), Some("E1"));
}

#[test]
fn each_range_is_built_to_its_ends_and_refused_one_even_step_past_them() {
    for template in Template::ALL {
        let (w, h) = (template.widths(), template.heights());
        let mid_h = sizes(template).1[1];
        let mid_w = sizes(template).0[1];
        inside(spec(template, w.low, mid_h, |_| {}));
        inside(spec(template, w.high, mid_h, |_| {}));
        outside(
            spec(template, plus(w.low, -2), mid_h, |_| {}),
            Rule::Width,
            "width",
        );
        outside(
            spec(template, plus(w.high, 2), mid_h, |_| {}),
            Rule::Width,
            "width",
        );
        outside(
            spec(template, mid_w, plus(h.low, -2), |_| {}),
            Rule::Height,
            "height",
        );
        outside(
            spec(template, mid_w, plus(h.high, 2), |_| {}),
            Rule::Height,
            "height",
        );
    }
}

#[test]
fn each_halved_length_is_built_on_an_even_step_and_refused_on_an_odd_one() {
    for template in Template::ALL {
        let mid = sizes(template);
        let (w, h) = (mid.0[1], mid.1[1]);
        inside(spec(template, plus(w, 2), h, |_| {}));
        outside(spec(template, plus(w, 1), h, |_| {}), Rule::Even, "width");
        let depth = match template {
            Template::FixedWindow => mm(70),
            _ => mm(100),
        };
        outside(
            spec(template, w, h, |s| {
                s.frame_depth = Some(plus(depth, 1));
            }),
            Rule::Even,
            "frame_depth",
        );
    }
    let door = Template::SingleSwingDoor;
    // The leaf's height (height less the head) is halved: an odd head on an odd height is even.
    inside(spec(door, mm(900), plus(mm(2100), 1), |s| {
        s.frame = Some(plus(mm(40), 1));
    }));
    outside(
        spec(door, mm(900), plus(mm(2100), 1), |_| {}),
        Rule::Even,
        "leaf_height",
    );
    let fixed = Template::FixedWindow;
    outside(
        spec(fixed, mm(1200), mm(1200), |s| {
            s.glass = Some(plus(mm(24), 1));
        }),
        Rule::Even,
        "glass",
    );
    let sliding = Template::SlidingWindow;
    outside(
        spec(sliding, mm(1800), plus(mm(1200), 1), |_| {}),
        Rule::Even,
        "height",
    );
    for (what, set) in [
        (
            "sash",
            (|s: &mut Spec| {
                s.sash = Some(plus(mm(50), 1));
            }) as fn(&mut Spec),
        ),
        ("sash_depth", |s| {
            s.sash_depth = Some(plus(mm(40), 1));
        }),
        ("track_gap", |s| {
            s.track_gap = Some(plus(mm(10), 1));
        }),
        ("glass", |s| {
            s.glass = Some(plus(mm(20), 1));
        }),
    ] {
        outside(spec(sliding, mm(1800), mm(1200), set), Rule::Even, what);
    }
}

#[test]
fn each_part_fits_its_frame_to_the_edge_and_is_refused_past_it() {
    let door = Template::SingleSwingDoor;
    inside(spec(door, mm(900), mm(2100), |s| {
        s.leaf = Some(mm(100));
    }));
    outside(
        spec(door, mm(900), mm(2100), |s| {
            s.leaf = Some(plus(mm(100), 2));
        }),
        Rule::Fits,
        "leaf",
    );
    let fixed = Template::FixedWindow;
    inside(spec(fixed, mm(1200), mm(1200), |s| {
        s.glass = Some(mm(70));
    }));
    outside(
        spec(fixed, mm(1200), mm(1200), |s| {
            s.glass = Some(plus(mm(70), 2));
        }),
        Rule::Fits,
        "glass",
    );
    let sliding = Template::SlidingWindow;
    // Two 45 mm sashes and the 10 mm gap fill the 100 mm frame exactly.
    inside(spec(sliding, mm(1800), mm(1200), |s| {
        s.sash_depth = Some(mm(45));
    }));
    outside(
        spec(sliding, mm(1800), mm(1200), |s| {
            s.sash_depth = Some(plus(mm(45), 2));
        }),
        Rule::Fits,
        "sash_depth",
    );
    inside(spec(sliding, mm(1800), mm(1200), |s| {
        s.glass = Some(mm(40));
    }));
    outside(
        spec(sliding, mm(1800), mm(1200), |s| {
            s.glass = Some(plus(mm(40), 2));
        }),
        Rule::Fits,
        "glass",
    );
}

#[test]
fn each_clear_opening_is_built_to_its_last_two_steps_and_refused_past_it() {
    let door = Template::SingleSwingDoor;
    // 600 less two frames of 280 mm less a step each, less the 40 mm leaf: two steps clear. The
    // height is a step over 1,800 so the leaf's height stays even.
    let h = plus(mm(1800), 1);
    let edge = spec(door, mm(600), h, |s| {
        s.frame = Some(plus(mm(280), -1));
    });
    let generated = make(std::slice::from_ref(&edge)).unwrap();
    assert_eq!(
        generated.types[0].quantities.clear_width,
        Length::from_steps(2)
    );
    inside(edge);
    outside(
        spec(door, mm(600), h, |s| {
            s.frame = Some(mm(280));
        }),
        Rule::Clear,
        "clear_width",
    );
    let fixed = Template::FixedWindow;
    let edge = spec(fixed, mm(300), mm(300), |s| {
        s.frame = Some(plus(mm(150), -1));
    });
    let q = make(std::slice::from_ref(&edge)).unwrap().types[0].quantities;
    assert_eq!(
        (q.clear_width, q.clear_height),
        (Length::from_steps(2), Length::from_steps(2))
    );
    inside(edge);
    outside(
        spec(fixed, mm(300), mm(300), |s| {
            s.frame = Some(mm(150));
        }),
        Rule::Clear,
        "clear_width",
    );
    outside(
        spec(fixed, mm(600), mm(300), |s| {
            s.frame = Some(mm(150));
        }),
        Rule::Clear,
        "clear_height",
    );
    let sliding = Template::SlidingWindow;
    // 900 less two 75 mm frames less three 50 mm sashes leaves the 600 mm the slide mark needs.
    let edge = spec(sliding, mm(900), mm(1200), |s| {
        s.frame = Some(mm(75));
    });
    let q = make(std::slice::from_ref(&edge)).unwrap().types[0].quantities;
    assert_eq!(q.clear_width, mm(350));
    inside(edge);
    outside(
        spec(sliding, mm(900), mm(1200), |s| {
            s.frame = Some(plus(mm(75), 1));
        }),
        Rule::Clear,
        "clear_width",
    );
    // 600 less two 50 mm frames less two 150 mm sashes leaves the 200 mm the mark needs.
    inside(spec(sliding, mm(1200), mm(600), |s| {
        s.sash = Some(mm(150));
    }));
    outside(
        spec(sliding, mm(1200), mm(600), |s| {
            s.sash = Some(plus(mm(150), 2));
        }),
        Rule::Clear,
        "clear_height",
    );
}

#[test]
fn a_unit_fits_a_wall_as_thick_as_its_frame_and_no_thinner() {
    for template in Template::ALL {
        let depth = match template {
            Template::FixedWindow => mm(70),
            _ => mm(100),
        };
        let mid = sizes(template);
        inside(spec(template, mid.0[1], mid.1[1], |s| {
            s.wall = Some(depth);
        }));
        outside(
            spec(template, mid.0[1], mid.1[1], |s| {
                s.wall = Some(plus(depth, -2));
            }),
            Rule::Wall,
            "wall",
        );
    }
}
