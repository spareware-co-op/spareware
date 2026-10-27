//! Appraisal and assessment against the JavaScript's recorded answers: capital and econ per scenario
//! pair and period, difference, withWithout, extentTest and devDistribution, with the recorded
//! results as inputs.

mod recorded;
mod support;

use construct_transport::analysis::{
    DiffOpts, ExtentOpts, dev_distribution, difference, extent_test, with_without,
};
use construct_transport::econ::{capital, econ};
use recorded::{json, project_and_record};
use support::{PROJECTS, assert_close};

#[test]
fn capital_as_recorded() {
    let mut n = 0;
    for name in PROJECTS {
        let (p, rec) = project_and_record(name);
        for sc in &p.scenarios {
            assert_close(
                &json(&capital(sc, &p.settings)),
                &rec.scenarios[&sc.id].capital,
                &format!("{name}.{}.capital", sc.id),
            );
            n += 1;
        }
    }
    assert_eq!(n, 5);
}

#[test]
fn pairs_as_recorded() {
    let mut n = 0;
    for name in PROJECTS {
        let (p, rec) = project_and_record(name);
        for (pair, periods) in rec.pairs.iter() {
            let (a, b) = pair.split_once('|').expect("a|b");
            let (sa, sb) = (p.scenario(a).unwrap(), p.scenario(b).unwrap());
            let (ra, rb) = (rec.results(a), rec.results(b));
            for (period, want) in periods.iter() {
                let path = format!("{name}.pairs[{pair}].{period}");
                let (ea, eb) = (ra.get(period), rb.get(period));
                let ec = econ(&p.settings, sa, &ra, sb, &rb, period);
                assert_close(&json(&ec), &want.econ, &format!("{path}.econ"));
                assert_close(
                    &json(&difference(ea, eb, DiffOpts::default())),
                    &want.difference,
                    &format!("{path}.difference"),
                );
                assert_close(
                    &json(&with_without(ea, eb, None)),
                    &want.with_without,
                    &format!("{path}.withWithout"),
                );
                assert_close(
                    &json(&extent_test(sb, ea, eb, ExtentOpts::default())),
                    &want.extent_test,
                    &format!("{path}.extentTest"),
                );
                n += 4;
            }
        }
    }
    assert_eq!(n, 16);
}

#[test]
fn dev_distribution_as_recorded() {
    let mut n = 0;
    for name in PROJECTS {
        let (p, rec) = project_and_record(name);
        for sc in &p.scenarios {
            let res = rec.results(&sc.id);
            let list: Vec<_> = ["AM", "PM"].iter().filter_map(|x| res.get(x)).collect();
            assert_close(
                &json(&dev_distribution(Some(sc), &list)),
                &rec.scenarios[&sc.id].dev_distribution,
                &format!("{name}.{}.devDistribution", sc.id),
            );
            n += 1;
        }
    }
    assert_eq!(n, 5);
}
