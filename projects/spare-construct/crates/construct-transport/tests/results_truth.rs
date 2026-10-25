//! Queries on results against the JavaScript's recorded answers: travel, isochrone, selectLink,
//! lts, exportReady, modelMeta (and every result's own meta), modelStatus, yearOf. The inputs are
//! the recorded results themselves (expected/<name>.json res), read as the typed `Res`.

mod recorded;
mod support;

use construct_transport::results::{
    Convergence, Res, RouteGraph, SearchTree, Travel, export_ready, isochrone, isochrone_on,
    js_sort_by, lts, model_meta, model_status, select_link, travel, travel_on,
};
use construct_transport::standards::year_of;
use construct_transport::util::LatLng;
use recorded::{json, project_and_record};
use serde_json::{Value, json};
use support::{PROJECTS, assert_close};

/// What gen-expected.mjs passes to modelMeta as c: the result itself.
fn conv_of(res: &Res) -> Convergence {
    Convergence {
        converged: res.converged,
        hit_max: res.hit_max,
        gap: res.gap,
        gap_tol: res.gap_tol,
        it: res.it,
        note: res.conv_note.clone(),
    }
}

#[test]
fn travel_and_isochrone_as_recorded() {
    // gen-expected.mjs asks with zone ids; buildGraph indexes zones as "z:<id>", so the JavaScript returns null
    // (analysis_cases.rs has routes between nodes and zone centroids)
    let mut n = 0;
    for name in PROJECTS {
        let (p, rec) = project_and_record(name);
        for sc in &p.scenarios {
            let zl: Vec<&str> = sc
                .zones
                .values()
                .filter(|z| !z.kind.is("gateway"))
                .map(|z| z.id.as_str())
                .collect();
            for (period, r) in rec.scenarios[&sc.id].periods.iter() {
                let path = format!("{name}.{}.{period}", sc.id);
                if zl.len() >= 2 {
                    let t = travel(sc, &p.settings, Some(&r.res), zl[0], zl[1]);
                    assert_close(
                        &json(&t),
                        r.travel.as_ref().unwrap_or(&Value::Null),
                        &format!("{path}.travel"),
                    );
                    n += 1;
                }
                if !zl.is_empty() {
                    let i = isochrone(sc, &p.settings, Some(&r.res), zl[0]);
                    assert_close(
                        &json(&i),
                        r.isochrone.as_ref().unwrap_or(&Value::Null),
                        &format!("{path}.isochrone"),
                    );
                    n += 1;
                }
            }
        }
    }
    assert_eq!(n, 20);
}

#[test]
fn select_link_lts_export_ready_and_meta_as_recorded() {
    let mut n = 0;
    for name in PROJECTS {
        let (p, rec) = project_and_record(name);
        let s = &p.settings;
        for sc in &p.scenarios {
            for (period, r) in rec.scenarios[&sc.id].periods.iter() {
                let path = format!("{name}.{}.{period}", sc.id);
                let res = &r.res;
                // the busiest arc, as gen-expected.mjs picks it
                let mut arcs: Vec<(&str, f64)> = res.arcs.iter().map(|(k, a)| (k, a.v)).collect();
                js_sort_by(&mut arcs, |a, b| {
                    construct_transport::jsnum::or_default(b.1, 0.0)
                        - construct_transport::jsnum::or_default(a.1, 0.0)
                });
                let key = arcs.first().map(|a| a.0.to_string());
                assert_eq!(key, r.select_link_key, "{path}.selectLinkKey");
                let sl = select_link(Some(res), key.as_deref().unwrap());
                assert_close(
                    &json(&sl),
                    r.select_link.as_ref().unwrap(),
                    &format!("{path}.selectLink"),
                );
                let lts_all: serde_json::Map<String, Value> = sc
                    .links
                    .values()
                    .map(|l| (l.id.clone(), json!(lts(l, Some(res), s))))
                    .collect();
                assert_close(&Value::Object(lts_all), &r.lts, &format!("{path}.lts"));
                assert_close(
                    &json(&export_ready(Some(res))),
                    &r.export_ready,
                    &format!("{path}.exportReady"),
                );
                let meta = model_meta(&p, Some(sc), period, s, conv_of(res));
                assert_close(&json(&meta), &r.model_meta, &format!("{path}.modelMeta"));
                // and the meta every result carries, made the same way inside assign
                assert_close(&json(&meta), &json(&res.meta), &format!("{path}.res.meta"));
                n += 5;
            }
        }
    }
    assert_eq!(n, 50);
}

#[test]
fn model_status_and_year_as_recorded() {
    for name in PROJECTS {
        let (p, rec) = project_and_record(name);
        for (period, v) in rec.model_status.iter() {
            assert_close(
                &json(&model_status(&p, period)),
                v,
                &format!("{name}.modelStatus.{period}"),
            );
        }
        for sc in &p.scenarios {
            assert_close(
                &json!(year_of(Some(sc), &p.settings)),
                &rec.scenarios[&sc.id].year_of,
                &format!("{name}.{}.yearOf", sc.id),
            );
        }
    }
}

/// A small hand-made graph: a line of nodes a - b - c - d with one arc each way, and zone z on a;
/// edge-based search with a 1 s penalty for every turn.
struct Line {
    ids: Vec<&'static str>,
    arcs: Vec<(&'static str, usize, usize, f64, Vec<LatLng>)>,
}

fn line() -> Line {
    let p = |x: f64| [x, 174.0];
    Line {
        ids: vec!["a", "b", "c", "d", "z:z1"],
        arcs: vec![
            ("l1:ab", 0, 1, 100.0, vec![p(0.0), p(0.5), p(1.0)]),
            ("l2:ab", 1, 2, 200.0, vec![p(1.0), p(2.0)]),
            ("l3:ab", 2, 3, 300.0, vec![p(2.0), p(3.0)]),
            ("l1:ba", 1, 0, 100.0, vec![p(1.0), p(0.5), p(0.0)]),
            ("c:z1:a:o", 4, 0, 50.0, vec![p(-1.0), p(0.0)]),
        ],
    }
}

impl RouteGraph for Line {
    fn node_index(&self, id: &str) -> Option<usize> {
        self.ids.iter().position(|x| *x == id)
    }
    fn node_count(&self) -> usize {
        self.ids.len()
    }
    fn node_id(&self, i: usize) -> &str {
        self.ids[i]
    }
    fn is_centroid(&self, i: usize) -> bool {
        self.ids[i].starts_with("z:")
    }
    fn arc_count(&self) -> usize {
        self.arcs.len()
    }
    fn arc_key(&self, a: usize) -> &str {
        self.arcs[a].0
    }
    fn arc_len(&self, a: usize) -> f64 {
        self.arcs[a].3
    }
    fn arc_ff(&self, a: usize) -> f64 {
        self.arcs[a].3 / 10.0
    }
    fn arc_pts(&self, a: usize) -> &[LatLng] {
        &self.arcs[a].4
    }
    fn empty_time(&self, a: usize) -> f64 {
        self.arcs[a].3 / 5.0
    }
    fn shortest(&self, cost: &[f64], src: usize, _target: Option<usize>) -> SearchTree {
        let (n, m) = (self.ids.len(), self.arcs.len());
        let mut dist = vec![f64::INFINITY; m];
        let mut pred = vec![-1i32; m];
        let mut nd = vec![f64::INFINITY; n];
        let mut na = vec![-1i32; n];
        nd[src] = 0.0;
        for (b, arc) in self.arcs.iter().enumerate() {
            if arc.1 == src {
                dist[b] = cost[b];
            }
        }
        let mut done = vec![false; m];
        loop {
            let next = (0..m)
                .filter(|&a| !done[a] && dist[a].is_finite())
                .min_by(|&x, &y| dist[x].total_cmp(&dist[y]));
            let Some(a) = next else { break };
            done[a] = true;
            let v = self.arcs[a].2;
            if dist[a] < nd[v] {
                nd[v] = dist[a];
                na[v] = a as i32;
            }
            if self.is_centroid(v) {
                continue;
            }
            for (b, arc) in self.arcs.iter().enumerate() {
                // no U-turns; a turn costs 1 s
                if arc.1 == v && arc.2 != self.arcs[a].1 {
                    let d = dist[a] + 1.0 + cost[b];
                    if d < dist[b] {
                        dist[b] = d;
                        pred[b] = a as i32;
                    }
                }
            }
        }
        SearchTree { src, nd, na, pred }
    }
}

#[test]
fn travel_and_isochrone_on_a_small_graph() {
    let g = line();
    // without a result every arc takes its empty time (len / 5): 20, 40, 60 s, plus two turns
    let Some(Travel::Route(t)) = travel_on(&g, None, "a", "d") else {
        panic!("a route")
    };
    assert_eq!(t.arcs, ["l1:ab", "l2:ab", "l3:ab"]);
    assert_eq!(
        t.pts,
        vec![
            [0.0, 174.0],
            [0.5, 174.0],
            [1.0, 174.0],
            [2.0, 174.0],
            [3.0, 174.0]
        ]
    );
    assert_eq!((t.time, t.ff, t.len, t.pen), (122.0, 60.0, 600.0, 2.0));
    // d cannot be left, so a is unreachable from it; unknown ids give None
    assert_eq!(
        travel_on(&g, None, "d", "a"),
        Some(Travel::Unreached { none: true })
    );
    assert_eq!(
        serde_json::to_value(travel_on(&g, None, "d", "a")).unwrap(),
        json!({ "none": true })
    );
    assert!(travel_on(&g, None, "a", "x").is_none());
    assert_eq!(
        travel_on(&g, None, "b", "b").map(|t| serde_json::to_value(t).unwrap()),
        Some(json!({ "arcs": [], "pts": [], "time": 0, "ff": 0, "len": 0, "pen": 0 }))
    );
    // times from z: the centroid itself is left out
    let iso = isochrone_on(&g, None, "z:z1").unwrap();
    assert_eq!(
        serde_json::to_value(&iso).unwrap(),
        json!({ "from": "z:z1", "times": { "a": 10, "b": 31, "c": 72, "d": 133 } })
    );
}

#[test]
fn js_sort_is_stable_and_reads_nan_as_equal() {
    // a stable sort with a numeric comparator agrees with slice::sort_by for any consistent comparator
    let mut seed = 7u64;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) % 13
    };
    for n in [0usize, 1, 2, 3, 7, 16, 33, 100] {
        let items: Vec<(u64, usize)> = (0..n).map(|i| (next(), i)).collect();
        let mut a = items.clone();
        js_sort_by(&mut a, |x, y| x.0 as f64 - y.0 as f64);
        let mut b = items.clone();
        b.sort_by_key(|x| x.0);
        assert_eq!(a, b, "n = {n}");
    }
    // NaN compares as equal, so those items keep their places relative to each other
    let mut v = vec![(f64::NAN, 0), (1.0, 1), (f64::NAN, 2), (0.0, 3)];
    js_sort_by(&mut v, |x, y| x.0 - y.0);
    let order: Vec<usize> = v.iter().map(|x| x.1).collect();
    assert_eq!(order, [0, 1, 2, 3]);
}
