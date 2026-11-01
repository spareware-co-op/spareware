//! Report truth tests: the ITA report document, its numbering, the data pack and the zip against the prototype.
//!
//! tools/transport-truth/gen-report.mjs ran packs/nz/report.js `Report` on the saved test projects and variants and wrote
//! fixtures/data/report-*.json (project, results, what the stand-ins for the other features answer) and expected/report-*.json
//! (the body, contents, document, pack files); these tests make the same calls on the Rust port.

mod support;

use construct_transport::edit::json::JsonValue;
use construct_transport::junction_tools::Features;
use construct_transport::project::Project;
use construct_transport::report::{
    BuildOpts, ForeignSection, LinkRiskInfo, MapImage, NodeRiskInfo, Regime, Rep, ReportHost,
    crc32, pick, zip,
};
use construct_transport::results::Results;
use serde_json::Value;
use std::collections::HashMap;
use support::read_truth;

const NOW: f64 = 1_792_540_800_000.0; // Date.UTC(2026, 9, 21)

struct Replay {
    host: Value,
    crash: Vec<Value>,
    conflated: HashMap<String, HashMap<String, String>>,
    ids: Vec<String>,
    foreign: Vec<ForeignSection>,
}

impl Features for Replay {
    fn supplementary(&self, _p: &Project) -> String {
        self.host
            .get("supplementary")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }
}

impl ReportHost for Replay {
    fn regime(&self, _p: &Project) -> Option<Regime> {
        let r = self.host.get("regime")?;
        let s = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Some(Regime {
            pathway: s("pathway"),
            pathway_label: s("pathwayLabel"),
            date: s("date"),
            text: s("text"),
            short: s("short"),
        })
    }
    fn stamp_footer(&self, _p: &Project) -> String {
        self.host
            .pointer("/stamp/footer")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }
    fn export_block(&self, _p: &Project) -> Option<String> {
        self.host
            .get("exportBlock")
            .and_then(Value::as_str)
            .map(str::to_string)
    }
    fn conflate(&self, sc: usize, _items: &JsonValue) -> Option<HashMap<String, String>> {
        if self.host.is_null() {
            return None;
        }
        self.conflated.get(&self.ids[sc]).cloned()
    }
    fn foreign_section(&self, id: &str) -> Option<ForeignSection> {
        self.foreign.iter().find(|f| f.id == id).cloned()
    }
    fn foreign_sections(&self) -> Vec<ForeignSection> {
        self.foreign.clone()
    }
    fn junction_of(&self, sc: usize, node: &str) -> Option<String> {
        self.crash
            .get(sc)?
            .pointer(&format!("/junctionOf/{node}"))?
            .as_str()
            .map(str::to_string)
    }
    fn node_risk(&self, sc: usize) -> Option<Vec<(String, NodeRiskInfo)>> {
        let o = self.crash.get(sc)?.get("nodeRisk")?.as_object()?;
        Some(
            o.iter()
                .map(|(k, v)| {
                    let nodes = v["nodes"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|x| x.as_str().unwrap_or("").to_string())
                                .collect()
                        })
                        .unwrap_or_default();
                    (
                        k.clone(),
                        NodeRiskInfo {
                            name: v["name"].as_str().unwrap_or("").to_string(),
                            nodes,
                        },
                    )
                })
                .collect(),
        )
    }
    fn link_risk(&self, sc: usize) -> Option<Vec<(String, LinkRiskInfo)>> {
        let o = self.crash.get(sc)?.get("linkRisk")?.as_object()?;
        Some(
            o.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        LinkRiskInfo {
                            aadt: v["aadt"].as_f64(),
                            aadt_src: v["aadtSrc"].as_str().map(str::to_string),
                        },
                    )
                })
                .collect(),
        )
    }
    fn crashes_on_links(&self, sc: usize) -> Option<f64> {
        self.crash.get(sc)?.get("onLinks")?.as_f64()
    }
}

fn first_diff(a: &str, b: &str) -> String {
    let at = a
        .chars()
        .zip(b.chars())
        .position(|(x, y)| x != y)
        .unwrap_or(a.chars().count().min(b.chars().count()));
    let c = |s: &str| {
        s.chars()
            .skip(at.saturating_sub(80))
            .take(200)
            .collect::<String>()
    };
    format!(
        "differ at char {at} (lengths {} and {})\n  expected ...{:?}\n  actual   ...{:?}",
        b.chars().count(),
        a.chars().count(),
        c(b),
        c(a)
    )
}

fn run(name: &str) -> usize {
    let file = JsonValue::parse(&read_truth(&format!("fixtures/data/report-{name}.json")))
        .expect("fixture");
    let raw = file.get("project").expect("project").clone();
    let project: Project = serde_json::from_str(&raw.stringify()).expect("project reads");
    let res = file.get("results").expect("results");
    let results: Vec<Results> = project
        .scenarios
        .iter()
        .map(|s| {
            serde_json::from_str(&res.get(&s.id).expect("res").stringify()).expect("results read")
        })
        .collect();
    let fx = |k: &str| file.get(k).map(JsonValue::to_value).unwrap_or(Value::Null);
    let foreign: Vec<ForeignSection> = fx("foreign")
        .as_array()
        .map(|a| {
            a.iter()
                .map(|f| ForeignSection {
                    id: f["id"].as_str().unwrap_or("").to_string(),
                    chapter: f["chapter"].as_str().unwrap_or("").to_string(),
                    order: f["order"].as_f64().unwrap_or(1000.0),
                    title: f["title"].as_str().map(str::to_string),
                    html: f["html"].as_str().unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    let conflated: HashMap<String, HashMap<String, String>> = fx("conflated")
        .as_object()
        .map(|o| {
            o.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.as_object()
                            .unwrap()
                            .iter()
                            .map(|(a, b)| (a.clone(), b.as_str().unwrap().to_string()))
                            .collect(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let host = Replay {
        host: fx("host"),
        crash: fx("crash").as_array().cloned().unwrap_or_default(),
        conflated,
        ids: project.scenarios.iter().map(|s| s.id.clone()).collect(),
        foreign,
    };
    let rep = Rep::new(&project, &raw, &results, &host, NOW);
    let exp = serde_json::from_str::<Value>(&read_truth(&format!("expected/report-{name}.json")))
        .expect("expected");
    let mut checks = 0;
    let mut fails: Vec<String> = Vec::new();
    let mut same = |what: &str, a: &str, b: &str| {
        checks += 1;
        if a != b {
            fails.push(format!("{name} {what}: {}", first_diff(a, b)));
        }
    };
    let mut opts = BuildOpts::default();
    let o = fx("opts");
    opts.period = o["period"].as_str().map(str::to_string);
    if let Some(m) = o["mapImages"].as_object() {
        for (k, v) in m {
            let img = match v {
                Value::String(u) => MapImage::Url(u.clone()),
                _ => MapImage::Object {
                    src: v["src"].as_str().map(str::to_string),
                    attribution: v["attribution"].as_str().map(str::to_string),
                    error: v["error"].as_str().map(str::to_string),
                },
            };
            opts.map_images.insert(k.clone(), img);
        }
    }
    let built = rep.build(&opts);
    same("body", &built.body, exp["body"].as_str().unwrap());
    same("toc", &serde_json::to_string(&built.toc.iter().map(|t| serde_json::json!({"ch": t.ch, "num": t.num, "title": t.title, "id": t.id, "level": t.level})).collect::<Vec<_>>()).unwrap(), &serde_json::to_string(&exp["toc"]).unwrap());
    same("html", &rep.doc(&built), exp["html"].as_str().unwrap());
    let pack = rep.pack_files();
    let want = exp["pack"].as_array().unwrap();
    same(
        "pack names",
        &pack
            .files
            .iter()
            .map(|f| f.name.clone())
            .collect::<Vec<_>>()
            .join("|"),
        &want
            .iter()
            .map(|f| f["name"].as_str().unwrap())
            .collect::<Vec<_>>()
            .join("|"),
    );
    for w in want {
        let n = w["name"].as_str().unwrap();
        if let Some(f) = pack.files.iter().find(|f| f.name == n) {
            same(&format!("pack {n}"), &f.data, w["data"].as_str().unwrap());
        }
    }
    let z = zip(&pack.files, 1_792_587_910_000.0);
    let zb = {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        z
    };
    same(
        "zip crc",
        &crc32(&zb).to_string(),
        &crc32(&base64_decode(exp["zipB64"].as_str().unwrap())).to_string(),
    );
    let sel = pick(&rep.tools());
    same(
        "pick",
        &format!(
            "{:?}",
            (
                sel.with,
                sel.without,
                sel.mitigation,
                sel.existing,
                sel.base
            )
        ),
        &{
            let g = |k: &str| exp["pick"][k].as_u64().map(|x| x as usize);
            format!(
                "{:?}",
                (
                    g("with"),
                    g("without"),
                    g("mitigation"),
                    g("existing"),
                    g("base")
                )
            )
        },
    );
    if !fails.is_empty() {
        panic!(
            "{} of {checks} checks differ:\n{}",
            fails.len(),
            fails.join("\n")
        );
    }
    checks
}

fn base64_decode(s: &str) -> Vec<u8> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let b: Vec<u8> = s.bytes().filter(|c| *c != b'=').collect();
    let mut out = Vec::new();
    for ch in b.chunks(4) {
        let v: Vec<u32> = ch.iter().map(|c| val(*c) as u32).collect();
        let n = v.iter().fold(0u32, |a, x| (a << 6) | x) << (6 * (4 - v.len()));
        for i in 0..(v.len() * 6 / 8) {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    out
}

#[test]
fn report_matches_the_prototype() {
    let mut total = 0;
    for v in [
        "corridor",
        "grid",
        "corridor-full",
        "grid-full",
        "corridor-comp",
        "grid-maps",
    ] {
        total += run(v);
    }
    println!("report: {total} checks match the JavaScript");
}
