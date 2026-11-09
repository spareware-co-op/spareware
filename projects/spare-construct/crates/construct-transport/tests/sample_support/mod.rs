//! Shared by the Drury sample truth tests: the recorded answers to every request, and the stand-ins for the features other ports cover (feeds, compliance,
//! compliance extras, report). tools/transport-truth/gen-sample.mjs wrote the stand-ins' JavaScript twins and recorded every call into them and every answer
//! of `Junctions.approaches` (which the Rust side replays, as that feature is another port's).
#![allow(dead_code)] // each test file uses its own part of this module

use construct_transport::census::Bx;
use construct_transport::jsnum::{Maybe, Num, value_to_number, value_truthy};
use construct_transport::project::{Project, Route, Scenario};
use construct_transport::sample::{Hazards, LaneApproach, SampleError, SampleHooks};
use construct_transport::studyarea::counts_of;
use serde_json::{Map, Value, json};

/// The answers every recorded request got, by request.
pub fn exchanges() -> Value {
    crate::support::load_json("fixtures/data/sample-exchanges.json")
}

fn obj<'a>(m: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    let v = m
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !v.is_object() {
        *v = Value::Object(Map::new());
    }
    v.as_object_mut().expect("an object")
}

/// The Rust twins of the JavaScript stand-ins in gen-sample.mjs (FAKES), steered by the same `fake` settings, with the same call log.
pub struct Fakes {
    /// What the stand-ins were asked to do (the case's `fake`).
    pub fake: Value,
    /// Every call, as the JavaScript logged it.
    pub calls: Vec<Value>,
    /// The recorded answers of `Junctions.approaches`, in order.
    pub approaches: Vec<Value>,
    at: usize,
}

impl Fakes {
    /// Stand-ins for a case.
    pub fn new(case: &Value) -> Fakes {
        Fakes {
            fake: case["fake"].clone(),
            calls: Vec::new(),
            approaches: case["approaches"].as_array().cloned().unwrap_or_default(),
            at: 0,
        }
    }
    /// True when every recorded answer of `approaches` was asked for.
    pub fn approaches_done(&self) -> bool {
        self.at == self.approaches.len()
    }
    fn log(&mut self, v: Value) {
        self.calls.push(v);
    }
    fn setting(&self, k: &str) -> Option<String> {
        self.fake.get(k).and_then(Value::as_str).map(str::to_string)
    }
    fn cx<'a>(p: &'a mut Project) -> &'a mut Map<String, Value> {
        let cx = obj(&mut p.extra, "cx");
        if !cx.contains_key("precinct") {
            cx.insert(
                "precinct".into(),
                json!({ "id": null, "register": [], "proposal": null }),
            );
            cx.insert("mitigation".into(), json!([]));
            cx.insert("conditions".into(), Value::Null);
        }
        cx
    }
    fn scenario_code(p: &Project, i: usize) -> Value {
        json!(p.scenarios[i].code.as_str())
    }
}

impl SampleHooks for Fakes {
    fn nslr_all(
        &mut self,
        p: &mut Project,
        _near: &Bx,
        scenario: usize,
        status: &mut dyn FnMut(&str),
    ) -> Result<(), SampleError> {
        self.log(json!(["nslr", 1]));
        if let Some(m) = self.setting("nslr") {
            return Err(SampleError::msg(m));
        }
        status("Reading speed limits");
        for l in p.scenarios[scenario].links.values_mut() {
            if l.cls.is("arterial") && l.speed.num() == 60.0 {
                l.speed = Num::number(50.0);
            }
        }
        Ok(())
    }
    fn tms_load(
        &mut self,
        p: &mut Project,
        status: &mut dyn FnMut(&str),
    ) -> Result<(), SampleError> {
        self.log(json!(["tms", 2]));
        if let Some(m) = self.setting("tms") {
            return Err(SampleError::msg(m));
        }
        status("Reading TMS");
        for c in counts_of(p).iter_mut() {
            if c.get("src").and_then(Value::as_str) == Some("nzta") {
                c["tms"] = json!({ "n": 1 });
            }
        }
        Ok(())
    }
    fn hazards_load(
        &mut self,
        _p: &mut Project,
        _near: &Bx,
        status: &mut dyn FnMut(&str),
    ) -> Result<Hazards, SampleError> {
        self.log(json!(["hazards"]));
        match self.setting("hazards").as_deref() {
            Some("none") => {
                return Ok(Hazards {
                    layers: vec![],
                    errors: vec!["flood service said no".to_string()],
                });
            }
            Some("none-silent") => return Ok(Hazards::default()),
            Some(m) => return Err(SampleError::msg(m)),
            None => {}
        }
        status("Reading flood layers");
        Ok(Hazards {
            layers: vec![
                json!({ "id": "flood", "name": "Flood plains" }),
                json!({ "id": "coast", "name": "Coastal inundation" }),
            ],
            errors: vec![],
        })
    }
    fn hazard_rows(&mut self, p: &Project, scenario: usize) -> Value {
        self.log(json!(["hazard-links"]));
        Value::Array(
            p.scenarios[scenario]
                .links
                .values()
                .take(3)
                .map(|l| json!({ "link": l.id, "flood": 120, "coast": 0 }))
                .collect(),
        )
    }
    fn gtfs_first_feed(&mut self, _near: &Bx) -> Result<Option<Value>, SampleError> {
        self.log(json!(["gtfs-catalogue"]));
        match self.setting("gtfs").as_deref() {
            Some("throw") => Err(SampleError::msg("catalogue down")),
            Some("none") => Ok(None),
            _ => Ok(Some(json!({ "id": "at-gtfs" }))),
        }
    }
    fn gtfs_apply(
        &mut self,
        p: &mut Project,
        feed: &Value,
        _near: &Bx,
        scenario: usize,
        status: &mut dyn FnMut(&str),
    ) -> Result<usize, SampleError> {
        self.log(json!(["gtfs-load", feed["id"]]));
        status("Reading timetable");
        self.log(json!(["gtfs-routes"]));
        self.log(json!(["gtfs-apply", 3]));
        let r: Route = serde_json::from_value(
            json!({ "id": "rGtfs", "name": "Bus 3", "color": "#7048e8", "freq": 4, "wp": [] }),
        )
        .expect("a route");
        p.scenarios[scenario].routes.insert("rGtfs", r);
        self.log(json!(["gtfs-keep", 3]));
        Ok(3)
    }
    fn approaches(&mut self, _sc: &Scenario, node_id: &str) -> Vec<LaneApproach> {
        let rec = self
            .approaches
            .get(self.at)
            .unwrap_or_else(|| panic!("approaches({node_id}) was not asked for by the prototype"));
        assert_eq!(
            rec["node"].as_str(),
            Some(node_id),
            "approaches call {}",
            self.at
        );
        self.at += 1;
        rec["out"]
            .as_array()
            .expect("a list")
            .iter()
            .map(|a| LaneApproach {
                link: a["link"].as_str().expect("link").to_string(),
                gl: a["gl"].as_f64().expect("gl"),
            })
            .collect()
    }
    fn compliance_context(
        &mut self,
        p: &mut Project,
        ctx: &Map<String, Value>,
        site: Option<&Map<String, Value>>,
    ) {
        let c = obj(&mut p.extra, "compliance");
        obj(c, "ctx");
        obj(c, "site");
        for (k, v) in ctx {
            obj(c, "ctx").insert(k.clone(), v.clone());
        }
        if let Some(s) = site {
            for (k, v) in s {
                obj(c, "site").insert(k.clone(), v.clone());
            }
        }
    }
    fn lookup_site(&mut self, p: &mut Project) -> Result<(), SampleError> {
        self.log(json!(["lookup-site"]));
        if self.setting("co").as_deref() == Some("lookup") {
            return Err(SampleError::msg("council lookup down"));
        }
        obj(obj(&mut p.extra, "compliance"), "site").insert("zone".into(), json!("Future Urban"));
        Ok(())
    }
    fn load_sh(&mut self, p: &mut Project, scenario: usize) -> Result<(), SampleError> {
        self.log(json!(["load-sh", Self::scenario_code(p, scenario)]));
        if self.setting("co").as_deref() == Some("sh") {
            return Err(SampleError::msg("state highway layer down"));
        }
        let id = p.scenarios[scenario].id.clone();
        obj(&mut p.extra, "compliance").insert("sh".into(), json!(id));
        Ok(())
    }
    fn load_overlays(&mut self, p: &mut Project, scenario: usize) -> Result<(), SampleError> {
        self.log(json!(["load-overlays", Self::scenario_code(p, scenario)]));
        if self.setting("co").as_deref() == Some("overlays") {
            return Err(SampleError::msg(""));
        }
        obj(&mut p.extra, "compliance").insert("overlays".into(), json!(2));
        Ok(())
    }
    fn record_outputs(&mut self, p: &mut Project) {
        self.log(json!(["record-outputs"]));
        let n = p.scenarios.len();
        obj(&mut p.extra, "compliance").insert("outputs".into(), json!(n));
    }
    fn precinct(&mut self, p: &mut Project, id: &str, register: Vec<Value>, proposal: Value) {
        let cx = Self::cx(p);
        let pr = obj(cx, "precinct");
        pr.insert("id".into(), json!(id));
        pr.insert("register".into(), Value::Array(register));
        pr.insert("proposal".into(), proposal);
    }
    fn seed_mitigation(&mut self, p: &mut Project) -> Result<(), SampleError> {
        self.log(json!(["seed-mitigation"]));
        if self.setting("cx").as_deref() == Some("seed") {
            return Err(SampleError::msg("no effects yet"));
        }
        let n = self.fake.get("cxN").and_then(Value::as_u64).unwrap_or(14);
        let mut list = vec![
            json!({ "id": "m1", "seed": "diff:1", "effect": "e0" }),
            json!({ "id": "m2", "seed": "diff:2" }),
            json!({ "id": "m3", "seed": "hand" }),
        ];
        for k in 0..n {
            list.push(json!({ "id": format!("x{k}"), "seed": format!("eff:{k}"), "effect": format!("e{}", k % 12) }));
        }
        Self::cx(p).insert("mitigation".into(), Value::Array(list));
        Ok(())
    }
    fn mitigation(&mut self, p: &mut Project) -> Vec<Value> {
        Self::cx(p)
            .get("mitigation")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    }
    fn set_mitigation(&mut self, p: &mut Project, list: Vec<Value>) {
        Self::cx(p).insert("mitigation".into(), Value::Array(list));
    }
    fn effects(&mut self, _p: &mut Project) -> Result<Vec<Value>, SampleError> {
        self.log(json!(["effects"]));
        Ok((0..12u32)
            .map(|k| match k % 3 {
                0 => json!({ "id": format!("e{k}"), "dos": 0.4 + f64::from(k % 5) * 0.15 }),
                1 => json!({ "id": format!("e{k}"), "vc": 0.6 + f64::from(k % 4) * 0.1 }),
                _ => json!({ "id": format!("e{k}"), "dos": 0, "vc": 0 }),
            })
            .collect())
    }
    fn seed_conditions(&mut self, p: &mut Project) -> Result<(), SampleError> {
        self.log(json!(["seed-conditions"]));
        if self.setting("cx").as_deref() == Some("conditions") {
            return Err(SampleError::msg("conditions table missing"));
        }
        let cx = Self::cx(p);
        let n = cx
            .get("mitigation")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        cx.insert("conditions".into(), json!({ "n": n }));
        Ok(())
    }
    fn staging(
        &mut self,
        p: &mut Project,
        steps: &[f64],
        progress: &mut dyn FnMut(usize, usize, &str) -> Result<(), SampleError>,
    ) -> Result<(), SampleError> {
        let list = steps
            .iter()
            .map(|s| construct_transport::jsnum::number_to_js_string(*s))
            .collect::<Vec<_>>();
        self.log(json!(["staging", list.join(",")]));
        if self.setting("rp").as_deref() == Some("staging") {
            return Err(SampleError::msg("staging failed"));
        }
        for (k, s) in list.iter().enumerate() {
            progress(k + 1, list.len(), &format!("{s}% built"))?;
        }
        obj(&mut p.extra, "report").insert("staging".into(), json!(steps));
        Ok(())
    }
    fn sensitivity(
        &mut self,
        p: &mut Project,
        tests: &[&str],
        progress: &mut dyn FnMut(usize, usize, &str) -> Result<(), SampleError>,
    ) -> Result<(), SampleError> {
        self.log(json!(["sensitivity", tests.join(",")]));
        if self.setting("rp").as_deref() == Some("sens") {
            return Err(SampleError::msg("sensitivity failed"));
        }
        for (k, t) in tests.iter().enumerate() {
            progress(k + 1, tests.len(), t)?;
        }
        obj(&mut p.extra, "report").insert("sens".into(), json!(tests));
        Ok(())
    }
}

/// `value_truthy` and `value_to_number` re-used by the tests.
pub fn truthy(v: &Value) -> bool {
    value_truthy(v)
}

/// A number read the JavaScript way.
pub fn number(v: &Value) -> f64 {
    value_to_number(v)
}

/// `Maybe` text.
pub fn text_of(m: &Maybe<String>) -> Option<&str> {
    m.as_str()
}
