//! Temporary probe (not committed): what curve_surface gives on the surface_torus benchmark's inputs.
#![allow(clippy::unwrap_used, missing_docs)]
use geom_bench::inputs::{self, Class};
use geom_intersect::curve_surface::curve_surface;
use geom_intersect::result::Item;
use geom_run::{Budget, WorkMeter};

#[test]
fn probe() {
    let all = inputs::intersect::line_tori(Class::Typical);
    let (mut labels, mut ats) = (std::collections::BTreeMap::new(), Vec::new());
    for (c, s) in all.iter() {
        let r = curve_surface(c, s, &mut WorkMeter::new(Budget::UNLIMITED)).unwrap();
        for i in r.items() {
            match i {
                Item::Point { at, contact, .. } => {
                    *labels.entry(format!("{contact:?}")).or_insert(0) += 1;
                    if ats.len() < 8 {
                        let sp = at.spans();
                        ats.push(format!("{contact:?} t {:.6} u {:.6} v {:.6}", sp[0].lo(), sp[1].lo(), sp[2].lo()));
                    }
                }
                other => *labels.entry(format!("{:?}", std::mem::discriminant(other))).or_insert(0) += 1,
            }
        }
    }
    println!("PROBE inputs {} labels {labels:?}", all.len());
    for a in ats { println!("PROBE {a}"); }
}
