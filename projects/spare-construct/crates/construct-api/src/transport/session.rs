//! The open project and everything around it: the active scenario, results per scenario and period, the id source and the undo history.

use super::cmd_calibrate::CalibState;
use super::cmd_sim::SimState;
use super::registry::{Handler, Sink, registry};
use crate::error::{ApiError, ApiResult};
use crate::schema::{Params, validate};
use construct_transport::edit;
use construct_transport::edit::history::{History, MAX_STEPS, mark_stale};
use construct_transport::edit::ids::Ids;
use construct_transport::jsnum::Maybe;
use construct_transport::project::{Project, Settings};
use construct_transport::results::{Res, Results};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// Writes every number with no fraction as a whole number (12, not 12.0), as `JSON.stringify` does.
fn whole_numbers(v: &mut Value) {
    match v {
        Value::Number(n) => {
            if let Some(x) = n.as_f64()
                && n.is_f64()
                && x.fract() == 0.0
                && x.abs() < 9_007_199_254_740_992.0
            {
                *v = Value::from(x as i64);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(whole_numbers),
        Value::Object(m) => m.values_mut().for_each(whole_numbers),
        _ => {}
    }
}

/// A function that receives progress events while long commands run.
pub type Listener = Box<dyn FnMut(&Value)>;

/// A random source of numbers in [0, 1), as `Math.random` gives.
pub type BoxRng = Box<dyn FnMut() -> f64>;

/// What one undo step restores.
#[derive(Clone)]
enum StepKind {
    /// The active scenario's network, demand and assessment fields (kept in that scenario's [`History`]).
    Scenario,
    /// The project settings.
    Settings(Box<Settings>),
}

/// One entry of the undo or redo list: what it was, which scenario and the label that names it.
#[derive(Clone)]
struct Step {
    sid: String,
    label: String,
    kind: StepKind,
}

/// The order of undo and redo steps across scenarios and settings, as the prototype's single undo stack keeps it.
#[derive(Default)]
struct StepLog {
    undo: Vec<Step>,
    redo: Vec<Step>,
}

/// The open Spare Construct transport project and everything commands need around it.
pub struct Session {
    pub(crate) project: Project,
    pub(crate) ids: Ids<BoxRng>,
    clock: Box<dyn FnMut() -> f64>,
    histories: HashMap<String, History>,
    log: StepLog,
    pub(crate) results: HashMap<String, Results>,
    pub(crate) sim: Option<SimState>,
    pub(crate) calib: CalibState,
    progress: Option<Listener>,
    revision: u64,
}

impl Session {
    /// A session with a new empty project. `rng` is `Math.random`, `clock` is `Date.now` (milliseconds): both are injected so a
    /// browser, a test and the command line each bring their own.
    pub fn new(
        rng: impl FnMut() -> f64 + 'static,
        clock: impl FnMut() -> f64 + 'static,
    ) -> Session {
        let mut clock: Box<dyn FnMut() -> f64> = Box::new(clock);
        let now = clock();
        let mut ids = Ids::new(Box::new(rng) as BoxRng, now);
        let project = edit::new_project(&mut ids, "Untitled project");
        Session {
            project,
            ids,
            clock,
            histories: HashMap::new(),
            log: StepLog::default(),
            results: HashMap::new(),
            sim: None,
            calib: CalibState::default(),
            progress: None,
            revision: 0,
        }
    }

    /// A session whose ids come from the operating system's clock and a time-seeded generator (not in the browser build).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn system() -> Session {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now_ms = || {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0.0, |d| d.as_millis() as f64)
        };
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64)
            ^ (u64::from(std::process::id()) << 32);
        let mut s = seed;
        let rng = move || {
            // splitmix64: a small, well mixed generator is enough for ids
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            (z >> 11) as f64 / (1u64 << 53) as f64
        };
        Session::new(rng, now_ms)
    }

    /// A session with the truth-test generator at this seed and a clock that stays at `now_ms`: the same ids as the prototype's harness.
    pub fn seeded(seed: u32, now_ms: f64) -> Session {
        let mut r = engine_sim::Mulberry32::new(seed);
        Session::new(move || engine_sim::Rng::next_f64(&mut r), move || now_ms)
    }

    /// A session whose ids and clock replay the truth-test harness (tools/transport-truth): `rng_skip` numbers of the seeded generator
    /// are used up first and the id counter starts at `seq0`, as they stand in the prototype after it has loaded.
    pub fn harness(seed: u32, now_ms: f64, rng_skip: usize, seq0: u64) -> Session {
        let mut s = Session::new(|| 0.5, move || now_ms);
        let mut r = engine_sim::Mulberry32::new(seed);
        for _ in 0..rng_skip {
            engine_sim::Rng::next_f64(&mut r);
        }
        s.ids = Ids::new(
            Box::new(move || engine_sim::Rng::next_f64(&mut r)) as BoxRng,
            now_ms,
        )
        .with_seq(seq0);
        s
    }

    /// Sets a function that receives progress events (`{ event, ... }`) while long commands run.
    pub fn set_progress(&mut self, f: impl FnMut(&Value) + 'static) {
        self.progress = Some(Box::new(f));
    }

    /// Stops sending progress events.
    pub fn clear_progress(&mut self) {
        self.progress = None;
    }

    /// A number that goes up each time the project changes (an edit, an undo, a run, opening a project).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The open project.
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// The results held for a scenario and period (a run, or a result read from a project file).
    pub fn result(&self, scenario_id: &str, period: &str) -> Option<&Res> {
        self.results.get(scenario_id).and_then(|r| r.get(period))
    }

    /// Runs one command by name with JSON parameters and gives its JSON result, or a structured error. Progress events go to the
    /// listener set with [`Session::set_progress`].
    pub fn run(&mut self, name: &str, params: &Value) -> ApiResult {
        let mut listener = self.progress.take();
        let out = {
            let mut sink = |ev: &Value| {
                if let Some(f) = listener.as_mut() {
                    f(ev);
                }
            };
            self.run_with(name, params, &mut sink)
        };
        self.progress = listener;
        out
    }

    /// Like [`Session::run`], with the progress events sent to this sink instead.
    pub fn run_with(&mut self, name: &str, params: &Value, sink: Sink<'_>) -> ApiResult {
        let reg = registry();
        let cmd = reg
            .get(name)
            .ok_or_else(|| ApiError::unknown_command(name))?;
        let empty = json!({});
        let params = if params.is_null() { &empty } else { params };
        if !params.is_object() {
            return Err(ApiError::bad_params("the parameters must be a JSON object"));
        }
        validate(&cmd.schema, params, "").map_err(ApiError::bad_params)?;
        let now = (self.clock)();
        self.ids.set_now_ms(now);
        let p = Params::new(params);
        let handler = cmd.handler;
        let call = |s: &mut Session, sink: Sink<'_>| {
            let mut r = match handler {
                Handler::Plain(h) => h(s, &p),
                Handler::Streaming(h) => h(s, &p, sink),
            };
            if let Ok(v) = &mut r {
                whole_numbers(v);
            }
            r
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::panic::{AssertUnwindSafe, catch_unwind};
            match catch_unwind(AssertUnwindSafe(|| call(self, sink))) {
                Ok(r) => r,
                Err(e) => {
                    let msg = e
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| e.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "unknown failure".to_string());
                    Err(ApiError::new(
                        "internal",
                        format!("The command stopped unexpectedly: {msg}"),
                    ))
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            call(self, sink)
        }
    }

    /// Runs a command given its parameters as JSON text (empty text means none) and gives one line of JSON:
    /// `{"ok":true,"result":...}` or `{"ok":false,"error":{"code","message"}}`.
    pub fn execute(&mut self, name: &str, params_text: &str) -> String {
        let params: Result<Value, _> = if params_text.trim().is_empty() {
            Ok(json!({}))
        } else {
            serde_json::from_str(params_text)
        };
        let out = match params {
            Err(e) => Err(ApiError::bad_params(format!(
                "the parameters are not valid JSON: {e}"
            ))),
            Ok(p) => self.run(name, &p),
        };
        match out {
            Ok(v) => json!({ "ok": true, "result": v }).to_string(),
            Err(e) => json!({ "ok": false, "error": e.to_value() }).to_string(),
        }
    }

    /// Every command as JSON text: name, description, whether it changes the project and its parameter schema.
    pub fn commands_json() -> String {
        registry().describe().to_string()
    }

    // ---------------------------------------------------------------- scenarios

    /// Index of the active scenario (`Store.sc`): the one named by `active`, otherwise the first.
    pub(crate) fn active_index(&self) -> ApiResult<usize> {
        let p = &self.project;
        p.active
            .as_str()
            .and_then(|id| p.scenarios.iter().position(|s| s.id == id))
            .or(if p.scenarios.is_empty() {
                None
            } else {
                Some(0)
            })
            .ok_or_else(|| ApiError::invalid_state("The project has no scenarios."))
    }

    /// Index of the scenario with this id or name (names match without regard to case).
    pub(crate) fn scenario_index(&self, key: &str) -> ApiResult<usize> {
        let p = &self.project;
        if let Some(i) = p.scenarios.iter().position(|s| s.id == key) {
            return Ok(i);
        }
        let k = key.trim().to_lowercase();
        let hits: Vec<usize> = p
            .scenarios
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.name
                    .as_str()
                    .is_some_and(|n| n.trim().to_lowercase() == k)
            })
            .map(|(i, _)| i)
            .collect();
        match hits.len() {
            1 => Ok(hits[0]),
            0 => Err(ApiError::not_found(format!(
                "There is no scenario with the id or name \"{key}\"."
            ))
            .with_details(json!({ "scenarios": self.scenario_names() }))),
            _ => Err(ApiError::bad_params(format!(
                "More than one scenario is called \"{key}\"; use its id."
            ))),
        }
    }

    fn scenario_names(&self) -> Vec<Value> {
        self.project
            .scenarios
            .iter()
            .map(|s| json!({ "id": s.id, "name": s.name.as_str().unwrap_or("") }))
            .collect()
    }

    /// The scenario a read-only command asks about: the `scenario` parameter (id or name), otherwise the active one.
    pub(crate) fn pick_scenario(&self, p: &Params, key: &str) -> ApiResult<usize> {
        match p.opt_str(key)? {
            Some(k) => self.scenario_index(k),
            None => self.active_index(),
        }
    }

    /// The scenario an editing command works on: the `scenario` parameter (id or name) is made the active one first, as picking it in the top bar does.
    pub(crate) fn target(&mut self, p: &Params) -> ApiResult<usize> {
        let i = self.pick_scenario(p, "scenario")?;
        let id = self.project.scenarios[i].id.clone();
        if self.active_index()? != i || !self.project.active.is(&id) {
            self.project.active = Maybe::Set(id);
            self.revision += 1;
        }
        Ok(i)
    }

    /// The period a command asks about: the `period` parameter, otherwise the project's current period.
    pub(crate) fn period_of(&self, p: &Params) -> ApiResult<String> {
        match p.opt_str("period")? {
            Some(x) => {
                if x == "AM" || x == "PM" {
                    Ok(x.to_string())
                } else {
                    Err(ApiError::bad_params("`period` must be AM or PM"))
                }
            }
            None => Ok(self.current_period()),
        }
    }

    /// The project's current period (`Store.S.period`): "AM" unless set to PM.
    pub(crate) fn current_period(&self) -> String {
        match self.project.settings.period.as_str() {
            Some("PM") => "PM".to_string(),
            _ => "AM".to_string(),
        }
    }

    /// Id of a scenario by index.
    pub(crate) fn sid(&self, i: usize) -> String {
        self.project.scenarios[i].id.clone()
    }

    // ---------------------------------------------------------------- undo

    /// `Kit.snap(label)`: takes an undo step for the active scenario before an edit, named so undo can say what it undid.
    pub(crate) fn snap(&mut self, label: &str) {
        let Ok(i) = self.active_index() else { return };
        let sid = self.project.scenarios[i].id.clone();
        self.histories
            .entry(sid.clone())
            .or_default()
            .snapshot(&self.project);
        self.log.undo.push(Step {
            sid,
            label: label.to_string(),
            kind: StepKind::Scenario,
        });
        if self.log.undo.len() > MAX_STEPS {
            self.log.undo.remove(0);
        }
        self.log.redo.clear();
    }

    /// `Kit.snapSettings(label)`: an undo step that restores the project settings.
    pub(crate) fn snap_settings(&mut self, label: &str) {
        self.log.undo.push(Step {
            sid: "settings".to_string(),
            label: label.to_string(),
            kind: StepKind::Settings(Box::new(self.project.settings.clone())),
        });
        if self.log.undo.len() > MAX_STEPS {
            self.log.undo.remove(0);
        }
        self.log.redo.clear();
    }

    /// Drops the step `snap` just took, as the prototype does when an action turned out to change nothing (`Store.undoStack.pop()`).
    pub(crate) fn discard_snap(&mut self) {
        let Some(step) = self.log.undo.pop() else {
            return;
        };
        if !matches!(step.kind, StepKind::Scenario) {
            return;
        }
        let Some(h) = self.histories.get_mut(&step.sid) else {
            return;
        };
        // History has no way to drop a step, so the step is taken back on a copy of the project that holds only its scenario
        let all = std::mem::take(&mut self.project.scenarios);
        let mut scratch = self.project.clone();
        scratch.scenarios = all.iter().filter(|s| s.id == step.sid).cloned().collect();
        self.project.scenarios = all;
        h.undo(&mut scratch);
    }

    /// Steps that can be undone, and redone.
    pub(crate) fn step_counts(&self) -> (usize, usize) {
        (self.log.undo.len(), self.log.redo.len())
    }

    /// The labels of the steps that can be undone (oldest first) and redone (next to redo last).
    pub fn step_labels(&self) -> (Vec<String>, Vec<String>) {
        (
            self.log.undo.iter().map(|s| s.label.clone()).collect(),
            self.log.redo.iter().map(|s| s.label.clone()).collect(),
        )
    }

    /// Forgets every undo and redo step (opening a project does this).
    pub(crate) fn clear_history(&mut self) {
        self.histories.clear();
        self.log = StepLog::default();
    }

    /// Forgets the undo steps of a scenario that was deleted (`Store.undoStack.filter`).
    pub(crate) fn forget_scenario_steps(&mut self, sid: &str) {
        self.log.undo.retain(|s| s.sid != sid);
        self.histories.remove(sid);
    }

    /// `undo(redo)` of ui.js: undoes or redoes the latest step and says what it was.
    pub(crate) fn step_back(&mut self, redo: bool) -> ApiResult {
        let what = if redo { "redo" } else { "undo" };
        let top = if redo {
            self.log.redo.pop()
        } else {
            self.log.undo.pop()
        };
        let Some(step) = top else {
            return Ok(
                json!({ "done": false, "action": what, "message": format!("Nothing to {what}.") }),
            );
        };
        let before = self.project.active.as_str().map(str::to_string);
        let ok = match &step.kind {
            StepKind::Settings(data) => {
                let cur = std::mem::replace(&mut self.project.settings, (**data).clone());
                for i in 0..self.project.scenarios.len() {
                    self.mark_stale_at(i);
                }
                let back = Step {
                    sid: step.sid.clone(),
                    label: step.label.clone(),
                    kind: StepKind::Settings(Box::new(cur)),
                };
                if redo {
                    self.log.undo.push(back)
                } else {
                    self.log.redo.push(back)
                }
                true
            }
            StepKind::Scenario => {
                let done = match self.histories.get_mut(&step.sid) {
                    Some(h) if redo => h.redo(&mut self.project),
                    Some(h) => h.undo(&mut self.project),
                    None => false,
                };
                if done {
                    self.mark_cache_stale(&step.sid);
                    let back = Step {
                        sid: step.sid.clone(),
                        label: step.label.clone(),
                        kind: StepKind::Scenario,
                    };
                    if redo {
                        self.log.undo.push(back)
                    } else {
                        self.log.redo.push(back)
                    }
                }
                done
            }
        };
        if !ok {
            return Ok(json!({
                "done": false, "action": what,
                "message": "That step can no longer be undone."
            }));
        }
        self.revision += 1;
        let sc = self.project.scenarios.iter().find(|s| s.id == step.sid);
        let sc_name = sc.map(|s| s.name.as_str().unwrap_or("").to_string());
        let switched = sc.is_some() && before.as_deref() != Some(step.sid.as_str());
        let label = if step.label.is_empty() {
            if step.sid == "settings" {
                "settings".to_string()
            } else {
                "the last edit".to_string()
            }
        } else {
            step.label.clone()
        };
        let mut msg = format!("{}: {}", if redo { "Redid" } else { "Undid" }, label);
        if let Some(n) = &sc_name {
            msg.push_str(&format!(" ({n})"));
        }
        msg.push('.');
        if switched && let Some(n) = &sc_name {
            msg.push_str(&format!(" Switched to {n}."));
        }
        Ok(json!({
            "done": true, "action": what, "label": label, "scenario": sc_name, "switched": switched, "message": msg,
            "undo": self.log.undo.len(), "redo": self.log.redo.len(),
        }))
    }

    // ---------------------------------------------------------------- results and staleness

    /// `Store.edited()`: an edit to the active scenario makes its results out of date.
    pub(crate) fn edited(&mut self) {
        if let Ok(i) = self.active_index() {
            self.mark_stale_at(i);
        }
        self.revision += 1;
    }

    /// Marks the results of one scenario (by index) out of date, in the project and in the held results.
    pub(crate) fn mark_stale_at(&mut self, i: usize) {
        mark_stale(&mut self.project.scenarios[i]);
        let sid = self.project.scenarios[i].id.clone();
        self.mark_cache_stale(&sid);
        self.revision += 1;
    }

    fn mark_cache_stale(&mut self, sid: &str) {
        if let Some(r) = self.results.get_mut(sid) {
            for res in r.values_mut() {
                res.stale = true;
            }
        }
    }

    /// Marks every scenario's results out of date (a change to the project settings does this).
    pub(crate) fn stale_all(&mut self) {
        for i in 0..self.project.scenarios.len() {
            self.mark_stale_at(i);
        }
    }

    /// Counts a change to the project that is not an edit to a scenario (a name, a setting, the period).
    pub(crate) fn touch(&mut self) {
        self.revision += 1;
    }

    /// Keeps a result for a scenario and period, in the held results and in the project's `res`.
    pub(crate) fn store_res(&mut self, i: usize, period: &str, res: Res) -> ApiResult<()> {
        // the project's copy keeps the paths too (saving strips them), as the prototype's `sc.res` does
        let v = serde_json::to_value(&res).map_err(|e| ApiError::new("internal", e.to_string()))?;
        let sid = self.project.scenarios[i].id.clone();
        let sc = &mut self.project.scenarios[i];
        match &mut sc.res {
            Maybe::Set(Value::Object(m)) => {
                m.insert(period.to_string(), v);
            }
            other => {
                let mut m = Map::new();
                m.insert(period.to_string(), v);
                *other = Maybe::Set(Value::Object(m));
            }
        }
        self.results
            .entry(sid)
            .or_default()
            .insert(period.to_string(), res);
        self.revision += 1;
        Ok(())
    }

    /// Forgets the results of a scenario (`sc.res = {}`).
    pub(crate) fn clear_res(&mut self, i: usize) {
        self.project.scenarios[i].res = Maybe::Set(Value::Object(Map::new()));
        let sid = self.sid(i);
        self.results.remove(&sid);
        self.revision += 1;
    }

    /// Reads every result stored in the project's scenarios into the held results; gives the ones that could not be read.
    pub(crate) fn load_results(&mut self) -> Vec<String> {
        self.results.clear();
        let mut bad = Vec::new();
        for sc in &self.project.scenarios {
            let Maybe::Set(Value::Object(m)) = &sc.res else {
                continue;
            };
            for (period, v) in m {
                if v.is_null() {
                    continue;
                }
                match serde_json::from_value::<Res>(v.clone()) {
                    Ok(r) => {
                        self.results
                            .entry(sc.id.clone())
                            .or_default()
                            .insert(period.clone(), r);
                    }
                    Err(e) => bad.push(format!(
                        "{} {period}: {e}",
                        sc.name.as_str().unwrap_or(&sc.id)
                    )),
                }
            }
        }
        bad
    }

    /// Replaces the open project (new or opened) and forgets everything that belonged to the old one.
    pub(crate) fn set_project(&mut self, p: Project) -> Vec<String> {
        self.project = p;
        self.clear_history();
        self.sim = None;
        self.calib = CalibState::default();
        self.revision += 1;
        self.load_results()
    }

    /// A short description of a scenario for lists.
    pub(crate) fn scenario_brief(&self, i: usize) -> Value {
        let sc = &self.project.scenarios[i];
        let mut results = Map::new();
        if let Some(r) = self.results.get(&sc.id) {
            for (period, res) in r.iter() {
                results.insert(
                    period.to_string(),
                    json!({ "stale": res.stale, "converged": res.converged, "iterations": res.it, "year": res.year }),
                );
            }
        }
        json!({
            "id": sc.id,
            "name": sc.name.as_str().unwrap_or(""),
            "role": sc.role.as_str(),
            "year": construct_transport::standards::year_of(Some(sc), &self.project.settings),
            "active": self.project.active.is(&sc.id),
            "base": self.project.base.is(&sc.id),
            "nodes": sc.nodes.len(), "links": sc.links.len(), "zones": sc.zones.len(), "routes": sc.routes.len(),
            "results": results,
        })
    }

    /// The project's own details: name, scenarios, period, undo steps and any notice about changed defaults.
    pub(crate) fn project_info(&self) -> Value {
        let p = &self.project;
        let (u, r) = self.step_counts();
        json!({
            "id": p.id.as_str(),
            "name": p.name.as_str().unwrap_or(""),
            "revision": self.revision,
            "active": p.active.as_str(),
            "base": p.base.as_str(),
            "period": self.current_period(),
            "scenarios": (0..p.scenarios.len()).map(|i| self.scenario_brief(i)).collect::<Vec<_>>(),
            "undo": u, "redo": r,
            "settingsNotice": edit::migrate::settings_notice(p).cloned(),
        })
    }
}
