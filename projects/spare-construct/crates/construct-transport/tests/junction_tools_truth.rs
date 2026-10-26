//! Junction tools truth tests: the junction summary, Movement Summary, comparison, flow-scale test, the CSVs and SIDRA exports,
//! the turning diagrams and the report sections against the prototype.
//!
//! tools/transport-truth/gen-junction-tools.mjs ran features/junctions.js `Junctions` on the saved test projects and on variants
//! (approach capacity, a made-up junction site, an extent of effects, runs short of the SIDRA gap, stale runs, counted peak hour
//! factors, banned turns) and wrote fixtures/data/junction-tools-*.json (project and results) and expected/junction-tools-*.json
//! (every call and its answer); these tests make the same calls on the Rust port and compare (text character for character).

mod junction_tools_support;
mod support;

use construct_transport::junction_tools::{
    FigureOpts, J1HtmlOpts, ReportCtx, SidraOpts, Tools, VolumeSource, compare_order, entries, entry_for,
    footer, j1_html, los_table, merge_candidates, node_approaches, node_legs, periods_of_all, upstream_len,
    xid, checks, Entry, Sc,
};
use construct_transport::report::pick;
use junction_tools_support::{Checker, PhfHost, Variant, load_variant, text};
use serde_json::{Value, json};
use std::collections::HashSet;

const VARIANTS: [&str; 12] = [
    "corridor",
    "grid",
    "corridor-arc",
    "grid-arc",
    "grid-site",
    "corridor-extent",
    "corridor-unconverged",
    "grid-stale",
    "grid-phf",
    "corridor-ban",
    "grid-regroup",
    "corridor-short",
];

fn s(v: &Value) -> &str {
    v.as_str().expect("text")
}

fn u(v: &Value) -> usize {
    v.as_u64().expect("index") as usize
}

fn ent_json(e: &Entry) -> Value {
    json!({
        "id": e.id, "kind": e.kind_name(), "name": e.name, "node": e.node, "nodes": e.nodes, "type": e.kind,
        "atSiteNo": e.at_site_no, "ll": e.ll.map_or(Value::Null, |l| json!([l[0], l[1]])), "hasNr": e.nr.is_some(),
    })
}

fn idx_list(v: &[usize]) -> Value {
    json!(v)
}

fn opts_of(c: &Value) -> SidraOpts {
    SidraOpts {
        scope: c.get("scope").and_then(Value::as_str).unwrap_or("auto").to_string(),
        // the recorded export of chosen ids is forced
        force: c.get("force").and_then(Value::as_bool).unwrap_or(c.get("ids").is_some()),
        date_iso: "2026-10-21".to_string(),
        date_long: "21 October 2026".to_string(),
    }
}

fn ids_arg(c: &Value) -> Option<Vec<String>> {
    c.get("ids").and_then(Value::as_array).map(|a| a.iter().map(|x| s(x).to_string()).collect())
}

fn res_of<'a>(t: &Tools<'a>, c: &Value) -> (Sc<'a>, Option<&'a construct_transport::results::Res>) {
    let sc = t.sc(u(&c["sc"]));
    let res = c.get("period").and_then(Value::as_str).and_then(|p| sc.res_of(p));
    (sc, res)
}

fn err_or(r: Result<String, String>) -> Value {
    match r {
        Ok(t) => json!(t),
        Err(e) => json!({ "error": e }),
    }
}

/// Runs one recorded call on the Rust port and gives its answer as the recorder wrote it.
fn run(t: &Tools, v: &Variant, c: &Value) -> Value {
    let f = s(&c["fn"]);
    let st = t.st();
    match f {
        "periodsOf" => json!(t.sc(u(&c["sc"])).periods()),
        "entries" => {
            let (sc, res) = res_of(t, c);
            let all = c["all"].as_bool().expect("flag");
            Value::Array(entries(sc.sc, res.expect("result"), all).iter().map(ent_json).collect())
        }
        "footer" => json!(footer(res_of(t, c).1.expect("result"))),
        "entryFor" => {
            let (sc, res) = res_of(t, c);
            entry_for(sc.sc, res.expect("result"), s(&c["id"])).map_or(Value::Null, |e| ent_json(&e))
        }
        "nodeLegs" => {
            let (sc, res) = res_of(t, c);
            Value::Array(
                node_legs(sc.sc, res, s(&c["node"]))
                    .iter()
                    .map(|l| {
                        let a = &l.arm;
                        json!({ "id": a.id, "o": a.o, "leg": a.leg, "legLabel": a.leg_label, "road": a.road, "b": a.b,
                            "isIn": a.is_in, "isOut": a.is_out, "inKey": a.in_key, "outKey": a.out_key, "conn": a.conn, "link": a.link })
                    })
                    .collect(),
            )
        }
        "j1" => {
            let (sc, res) = res_of(t, c);
            t.j1(sc.sc, res.expect("result"), s(&c["id"]), None).map_or(Value::Null, |d| d.to_value())
        }
        "j1Html" => {
            let (sc, res) = res_of(t, c);
            let d = t.j1(sc.sc, res.expect("result"), s(&c["id"]), None);
            let head = c["head"].as_bool().expect("flag");
            json!(j1_html(t, d.as_ref(), J1HtmlOpts { head }))
        }
        "turnCountFile" => {
            let (sc, res) = res_of(t, c);
            let r = t.turn_count_file(sc.sc, res, s(&c["id"]));
            json!({ "text": r.text, "nc": r.nc })
        }
        "summary" | "checks" | "xid" | "moves" | "banSvg" | "approaches" | "upstreamLen" => {
            let (sc, res) = res_of(t, c);
            match f {
                "approaches" => {
                    return Value::Array(
                        node_approaches(sc.sc, &entries(sc.sc, res.expect("result"), true).iter().find(|e| e.id == s(&c["id"])).expect("entry").node)
                            .iter()
                            .map(|a| a.to_value())
                            .collect(),
                    )
                }
                _ => {}
            }
            let res = res.expect("result");
            let es = entries(sc.sc, res, true);
            let e = es.iter().find(|e| e.id == s(&c["id"])).unwrap_or_else(|| panic!("no entry for {c}"));
            match f {
                "summary" => t.summary(sc.sc, res, e).map_or(Value::Null, |x| x.to_value()),
                "checks" => checks(sc.sc, st, res, e).to_value(),
                "xid" => json!(xid(e)),
                "moves" => Value::Array(t.moves(sc.sc, Some(res), &e.node).iter().map(|m| m.to_value()).collect()),
                "banSvg" => {
                    let list = t.moves(sc.sc, Some(res), &e.node);
                    json!(t.ban_svg(&e.node, &list, c.get("size").and_then(Value::as_f64)))
                }
                _ => {
                    // upstreamLen
                    let side: HashSet<String> = e.nodes.iter().cloned().collect();
                    let d = t.j1(sc.sc, res, &e.id, None);
                    match d {
                        None => Value::Null,
                        Some(d) => Value::Array(
                            d.nr.appr
                                .iter()
                                .map(|a| json!([a.key, upstream_len(sc.sc, res, &a.key, Some(&side))]))
                                .collect(),
                        ),
                    }
                }
            }
        }
        "figure" => {
            let (sc, res) = res_of(t, c);
            let id = s(&c["id"]);
            let layer = c.get("layer").and_then(Value::as_str);
            let size = c.get("size").and_then(Value::as_f64);
            let o = FigureOpts { size, layer };
            if size.is_some() {
                // figure(sc, null, res, id, { size })
                json!(t.figure(sc.sc, None, res, id, o))
            } else {
                json!(t.figure(sc.sc, res, None, id, o))
            }
        }
        "mergeCandidates" => {
            let sc = t.sc(u(&c["sc"]));
            Value::Array(merge_candidates(sc.sc, s(&c["id"])).iter().map(|m| m.to_value()).collect())
        }
        "flowScale" => {
            let (sc, res) = res_of(t, c);
            let f = c["f"].as_f64().unwrap_or(construct_transport::junction_tools::FLOW_SCALE_FACTOR);
            t.flow_scale(sc.sc, res.expect("result"), f).map_or(Value::Null, |r| r.to_value())
        }
        "scaleHtml" => {
            let (sc, res) = res_of(t, c);
            json!(t.scale_html(sc.sc, res))
        }
        "losTable" => json!(los_table(s(&c["method"]))),
        "j2" => {
            let sc = t.sc(u(&c["sc"]));
            Value::Array(t.j2(sc, &sc.periods()).iter().map(|r| r.to_value()).collect())
        }
        "j2Html" => {
            let sc = t.sc(u(&c["sc"]));
            let fly = c.get("fly").and_then(Value::as_bool).unwrap_or(true);
            json!(t.j2_html(sc, &sc.periods(), fly))
        }
        "junctionMovementsCSV" => json!(t.junction_movements_csv(Some(&[t.sc(u(&c["sc"]))]), None)),
        "keyJunctions" => {
            let n = c.get("n").and_then(Value::as_u64).map_or(8, |n| n as usize);
            json!(t.key_junctions(t.sc(u(&c["sc"])), n))
        }
        "turningMovementsCSV" => err_or(t.turning_movements_csv(Some(&[t.sc(u(&c["sc"]))]), None, c["force"].as_bool().expect("flag"))),
        "volumesCSV" => {
            let sc = t.sc(u(&c["sc"]));
            err_or(t.volumes_csv(sc, None, ids_arg(c).as_deref(), &opts_of(c)))
        }
        "loaderPy" => {
            let sc = t.sc(u(&c["sc"]));
            err_or(t.loader_py(sc, None, ids_arg(c).as_deref(), &opts_of(c)))
        }
        "gridsHtml" => {
            let sc = t.sc(u(&c["sc"]));
            json!(t.grids_html(sc, None, ids_arg(c).as_deref(), &opts_of(c)))
        }
        "volumeRows" => {
            let sc = t.sc(u(&c["sc"]));
            let o = opts_of(c);
            let list = t.both_res(sc);
            Value::Array(t.volume_rows(sc.sc, &list, None, &o.scope).iter().map(|r| r.to_value()).collect())
        }
        "asCountCSV" => {
            let sc = t.sc(u(&c["sc"]));
            let date = s(&c["date"]);
            if s(&c["src"]) == "rows" {
                let rows = t.volume_rows(sc.sc, &t.both_res(sc), None, "all");
                json!(t.as_count_csv(&VolumeSource::Rows(&rows), date))
            } else {
                let o = SidraOpts { scope: "all".into(), force: true, ..opts_of(c) };
                let csv = t.volumes_csv(sc, None, None, &o).expect("forced");
                json!(t.as_count_csv(&VolumeSource::Text(&csv), date))
            }
        }
        "parseVolumesCSV" => {
            let sc = t.sc(u(&c["sc"]));
            let o = SidraOpts { scope: "all".into(), force: true, ..opts_of(c) };
            let csv = t.volumes_csv(sc, None, None, &o).expect("forced");
            Value::Array(
                construct_transport::junction_tools::parse_volumes_csv(&csv)
                    .iter()
                    .map(|row| {
                        let mut m = serde_json::Map::new();
                        for (k, v) in row {
                            m.insert(
                                k.clone(),
                                match v {
                                    construct_transport::junction_tools::CsvValue::Num(x) => construct_transport::jsnum::plain_number(*x),
                                    construct_transport::junction_tools::CsvValue::Text(t) => json!(t),
                                },
                            );
                        }
                        Value::Object(m)
                    })
                    .collect(),
            )
        }
        "phfOf" => {
            let sc = t.sc(u(&c["sc"]));
            // the periods in the order the results were stored (Object.keys(sc.res))
            let ps: Vec<String> = sc.res.keys().map(str::to_string).collect();
            let first = ps.first().and_then(|p| sc.res_of(p));
            match first {
                None => json!([]),
                Some(r) => {
                    let mut out = Vec::new();
                    for e in entries(sc.sc, r, true) {
                        for p in &ps {
                            out.push(json!([e.id, p, t.phf_of(sc.sc, &e, p)]));
                        }
                    }
                    Value::Array(out)
                }
            }
        }
        "j3" => json!(t.j3(t.sc(u(&c["from"])), t.sc(u(&c["to"])), &periods_of_all(&[t.sc(u(&c["from"])), t.sc(u(&c["to"]))])).to_value()),
        "j3Html" => {
            let to = t.sc(u(&c["to"]));
            let from = c["from"].as_u64().map(|i| t.sc(i as usize));
            json!(t.j3_html(from, Some(to), None))
        }
        "compareOrder" => {
            let a = t.sc(u(&c["active"]));
            let o = t.sc(u(&c["other"]));
            let (x, y) = compare_order(&v.project, &a, &o, c["swap"].as_bool().expect("flag"));
            json!([x, y])
        }
        "reportSel" => {
            let ctx = if s(&c["ctx"]) == "scenarios" {
                ReportCtx { sel: None, scenarios: Some((0..v.project.scenarios.len()).rev().collect()) }
            } else {
                ReportCtx::default()
            };
            let r = t.report_sel(&ctx);
            json!({ "list": idx_list(&r.list), "pairs": r.pairs.iter().map(|(a, b, h, k)| json!([a, b, h, k])).collect::<Vec<_>>() })
        }
        "section" => {
            let ctx = if c.get("ctx").and_then(Value::as_str) == Some("scenarios") {
                ReportCtx { sel: None, scenarios: Some(vec![0]) }
            } else {
                ReportCtx { sel: Some(pick(t)), scenarios: None }
            };
            json!(t.report_section(s(&c["id"]), &ctx).expect("a junctions section"))
        }
        other => panic!("call {other} is not replayed"),
    }
}

fn is_text(f: &str) -> bool {
    matches!(
        f,
        "footer" | "j1Html" | "banSvg" | "figure" | "scaleHtml" | "losTable" | "j2Html" | "junctionMovementsCSV" | "turningMovementsCSV" | "volumesCSV"
            | "loaderPy" | "gridsHtml" | "asCountCSV" | "j3Html" | "section"
    )
}

#[test]
fn every_recorded_call_matches_the_prototype() {
    let mut ck = Checker::default();
    for name in VARIANTS {
        let v = load_variant("junction-tools", name);
        let host = PhfHost { phf: v.phf.clone() };
        let tools = Tools::new(&v.project, &v.results, &host).with_raw(&v.raw);
        let ex = support::load_json(&format!("expected/junction-tools-{name}.json"));
        let calls = ex["calls"].as_array().expect("calls");
        let mut n = 0;
        for (i, c) in calls.iter().enumerate() {
            let got = run(&tools, &v, c);
            let at = format!("{name}[{i}] {}", c["fn"]);
            let f = s(&c["fn"]);
            let want = &c["out"];
            if is_text(f) || want.is_string() {
                if let (Some(g), Some(w)) = (got.as_str(), want.as_str()) {
                    ck.same_text(&at, g, w);
                    n += 1;
                    continue;
                }
            }
            ck.check(&at, &got, want);
            n += 1;
        }
        println!("{name}: {n} calls replayed");
    }
    let _ = text("expected/junction-tools-corridor.json").len();
    ck.finish("junction tools");
}
