//! Shared by the census truth tests: the made-up downloaded answers, the replay of recorded requests and the seeded id generator.
//! tools/transport-truth/gen-census.mjs wrote the answers (fixtures/data/census-answers.json) and ran the prototype on them.
#![allow(dead_code)] // each test file uses its own part of this module

use construct_transport::census::{
    AupData, CensusData, CensusLayers, EmployeeData, Failure, FillData, Layer, Net, parse_aup,
    parse_census, parse_employees, parse_moe,
};
use construct_transport::edit::Ids;
use engine_sim::{Mulberry32, Rng};
use serde_json::Value;

/// The made-up downloaded answers (fixtures/data/census-answers.json).
pub fn answers() -> Value {
    crate::support::load_json("fixtures/data/census-answers.json")
}

/// The seeded id generator a recorded case used: the seed, how many numbers loading the scripts drew first, the id counter and the clock.
pub fn ids_for(file: &Value) -> Ids<Mulberry32> {
    let seed = file["seed"].as_u64().expect("seed") as u32;
    let mut rng = Mulberry32::new(seed);
    for _ in 0..file["rngSkip"].as_u64().expect("rngSkip") {
        rng.next_f64();
    }
    Ids::new(rng, file["now"].as_f64().expect("now")).with_seq(file["seq0"].as_u64().expect("seq0"))
}

/// Replays the requests the prototype made, in order: each request must be the next recorded one, and gets the answer it got.
pub struct Replay<'a> {
    log: &'a [Value],
    table: &'a Value,
    at: usize,
}

/// The answers every recorded request got, by request (fixtures/data/census-exchanges.json).
pub fn exchanges() -> Value {
    crate::support::load_json("fixtures/data/census-exchanges.json")
}

impl<'a> Replay<'a> {
    /// A replay of a recorded request log, with the table of answers.
    pub fn new(log: &'a Value, table: &'a Value) -> Replay<'a> {
        Replay {
            log: log.as_array().expect("a log"),
            table,
            at: 0,
        }
    }
    /// The requests made so far.
    pub fn used(&self) -> usize {
        self.at
    }
    /// True when every recorded request was made.
    pub fn done(&self) -> bool {
        self.at == self.log.len()
    }
    fn next(&mut self, method: &str, url: &str, body: Option<&str>) -> Result<Value, Failure> {
        let e = self.log.get(self.at).unwrap_or_else(|| {
            panic!(
                "request {} ({method} {url}) was not made by the prototype",
                self.at
            )
        });
        assert_eq!(e["method"].as_str(), Some(method), "request {}", self.at);
        assert_eq!(e["url"].as_str(), Some(url), "request {}", self.at);
        assert_eq!(
            e.get("body").and_then(Value::as_str),
            body,
            "body of request {}",
            self.at
        );
        self.at += 1;
        if let Some(f) = e.get("failure").and_then(Value::as_str) {
            return Err(match f {
                "network" => Failure::Unreachable("fetch failed".into()),
                "timeout" => Failure::Timeout,
                "cancelled" => Failure::Cancelled("The operation was aborted".into()),
                "readtimeout" => Failure::ReadTimeout,
                _ => Failure::Unreadable("Unexpected token < in JSON at position 0".into()),
            });
        }
        let status = e["status"].as_u64().expect("status") as u16;
        if !(200..300).contains(&status) {
            return Err(Failure::Http(status));
        }
        match e.get("answer") {
            Some(a) => Ok(a.clone()),
            None => Ok(self.table[e["ans"].as_str().expect("an answer key")].clone()),
        }
    }
}

impl Net for Replay<'_> {
    fn get_json(&mut self, url: &str) -> Result<Value, Failure> {
        self.next("GET", url, None)
    }
    fn post_form(&mut self, url: &str, body: &str) -> Result<Value, Failure> {
        self.next("POST", url, Some(body))
    }
}

/// The OpenStreetMap elements of the made-up suburb (fixtures/landuse-papatoetoe.json).
pub fn suburb_elements() -> Vec<Value> {
    crate::support::load_json("fixtures/landuse-papatoetoe.json")["elements"]
        .as_array()
        .expect("elements")
        .clone()
}

fn layer(fields: &Value, features: &Value) -> Layer {
    Layer {
        fields: fields.as_array().cloned().unwrap_or_default(),
        features: features.as_array().cloned().unwrap_or_default(),
    }
}

/// The Census layers of the made-up answers; `no_dw` leaves the dwellings layer out.
pub fn census_of(ans: &Value, no_dw: bool) -> CensusData {
    let c = &ans["census"];
    let l = |k: &str| Some(layer(&c[k]["fields"], &c[k]["features"]));
    parse_census(&CensusLayers {
        dw: if no_dw { None } else { l("dw") },
        hh: l("hh"),
        ind0: l("ind0"),
        ind2: l("ind2"),
    })
}

/// The employees of the made-up answers.
pub fn employees_of(ans: &Value) -> EmployeeData {
    parse_employees(&layer(
        &ans["employees"]["fields"],
        &ans["employees"]["features"],
    ))
}

/// The Unitary Plan zones of the made-up answers.
pub fn aup_of(ans: &Value) -> AupData {
    parse_aup(&layer(&ans["aup"]["fields"], &ans["aup"]["features"]))
}

/// What a fill is given: the made-up answers, each left out when its flag is false (`which` is the case's `data` object; the Census can be "noDw").
pub fn data_for(ans: &Value, which: &Value) -> FillData {
    let on = |k: &str| which.get(k).is_none_or(|v| v != &Value::Bool(false));
    let mut d = FillData::default();
    if on("elements") {
        d.elements = Some(suburb_elements());
    }
    if on("census") {
        d.census = Some(census_of(
            ans,
            which.get("census").and_then(Value::as_str) == Some("noDw"),
        ));
    }
    if on("employees") {
        d.employees = Some(employees_of(ans));
    }
    if on("schools") {
        d.schools = Some(parse_moe(ans["schools"].as_array().unwrap(), "schools"));
        d.ece = Some(parse_moe(ans["ece"].as_array().unwrap(), "ece"));
    }
    if on("aup") {
        d.aup = Some(aup_of(ans));
    }
    d
}
