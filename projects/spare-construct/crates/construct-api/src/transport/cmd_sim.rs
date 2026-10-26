//! The traffic microsimulation: create a run from the assignment, step it, read the HUD, the flow comparison and the vehicles.

use super::Session;
use super::registry::{Registry, Sink};
use crate::error::{ApiError, ApiResult};
use crate::schema::*;
use construct_transport::jsnum::{Maybe, json_number};
use construct_transport::sim::{Hud, Sim, SimOptions, SimResults};
use construct_transport::util::{along, fmt};
use engine_sim::Mulberry32;
use serde_json::{Value, json};

/// A simulation in progress: which scenario and period it runs, and the run itself.
pub(crate) struct SimState {
    /// Id of the scenario being simulated.
    pub scenario: String,
    /// "AM" or "PM".
    pub period: String,
    /// The seed of its random source.
    pub seed: u32,
    /// The scale the run was started with (None: the settings').
    pub scale: Option<f64>,
    /// The run.
    pub sim: Sim<Mulberry32>,
}

pub(super) fn register(r: &mut Registry) {
    r.add_streaming(
        "transport.sim.create",
        "Start a microsimulation from a run of a scenario and period (the model is run first when there is no current result with paths). Vehicles drive the assigned routes.",
        false,
        params(
            vec![
                ("scenario", string("Scenario id or name (default: the active scenario).")),
                ("period", choice(&["AM", "PM"], "Peak hour (default the project's current period).")),
                ("scale", number("Demand scale for this run (default the setting sim.scale).")),
                ("seed", integer("Seed of the random source, for a repeatable run (default drawn from the session).")),
            ],
            &[],
        ),
        sim_create,
    );
    r.add(
        "transport.sim.restart",
        "Start the current simulation again from the beginning, with the same seed.",
        false,
        none(),
        sim_restart,
    );
    r.add(
        "transport.sim.step",
        "Advance the simulation by simulated seconds (steps of 0.5 s) and give the HUD numbers.",
        false,
        params(
            vec![
                (
                    "seconds",
                    number("Simulated seconds to advance (default 60, at most 7200)."),
                ),
                ("steps", integer("Steps of 0.5 s instead of seconds.")),
            ],
            &[],
        ),
        sim_step,
    );
    r.add(
        "transport.sim.hud",
        "The HUD numbers now: clock, vehicles on the road, queued, average speed, trips done, delay per trip, waiting to enter, released and moved on.",
        false,
        none(),
        |s, _| {
            let st = state(s)?;
            Ok(hud_json(&st.sim))
        },
    );
    r.add(
        "transport.sim.series",
        "The running series sampled every 5 simulated seconds: vehicles on the road, queued, average speed, trips done and waiting.",
        false,
        params(vec![("last", integer("Only the latest this many samples (default 120)."))], &[]),
        sim_series,
    );
    r.add(
        "transport.sim.validation",
        "Compare the simulated flow over the last 5 to 10 minutes with the assignment, by road, with the GEH statistic (available after 7 simulated minutes).",
        false,
        params(vec![("limit", integer("Most roads to list (default 50)."))], &[]),
        sim_validation,
    );
    r.add(
        "transport.sim.vehicles",
        "The vehicles on the road now with their position, speed, lane and the road they are on.",
        false,
        params(
            vec![
                ("limit", integer("Most vehicles to list (default 500).")),
                ("offset", integer("Vehicles to skip.")),
            ],
            &[],
        ),
        sim_vehicles,
    );
    r.add(
        "transport.sim.stop",
        "Stop and forget the simulation.",
        false,
        none(),
        |s, _| {
            let had = s.sim.take().is_some();
            Ok(json!({ "stopped": had }))
        },
    );
}

fn state(s: &Session) -> ApiResult<&SimState> {
    s.sim.as_ref().ok_or_else(|| {
        ApiError::invalid_state(
            "There is no simulation running. Start one with transport.sim.create.",
        )
    })
}

fn hud_json(sim: &Sim<Mulberry32>) -> Value {
    let h: Hud = sim.hud();
    json!({
        "time": json_number(sim.time()), "clock": json_number(h.clock), "clockText": h.clock_text(), "onRoad": h.on_road, "queued": h.queued,
        "speedKmh": json_number(h.speed), "tripsDone": h.trips_done, "delayPerTrip": json_number(h.delay_per_trip),
        "waiting": h.waiting, "released": h.released, "teleports": h.teleports,
    })
}

fn build(
    s: &mut Session,
    sid: &str,
    period: &str,
    scale: Option<f64>,
    seed: u32,
) -> ApiResult<Sim<Mulberry32>> {
    let i = s.scenario_index(sid)?;
    let res = s.result(sid, period).ok_or_else(|| {
        ApiError::invalid_state(format!("There is no {period} result for this scenario."))
    })?;
    let sr = SimResults::from_res(res);
    let sim = Sim::new(
        &s.project.scenarios[i],
        &s.project.settings,
        Some(&sr),
        SimOptions { scale },
        Mulberry32::new(seed),
    );
    if sim.routes().is_empty() {
        return Err(ApiError::invalid_state(
            "No traffic to simulate. Check zones and gateways.",
        ));
    }
    Ok(sim)
}

/// `toggleSim()` of ui.js: runs the model when the result is missing, out of date or has no paths, then starts the run.
fn sim_create(s: &mut Session, p: &Params, sink: Sink<'_>) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let period = s.period_of(p)?;
    let sid = s.sid(i);
    let need_run = match s.result(&sid, &period) {
        None => true,
        Some(r) => r.stale || r.paths.is_missing(),
    };
    if need_run {
        s.run_with(
            "transport.run",
            &json!({ "scenario": sid, "periods": [period] }),
            sink,
        )?;
    }
    match s.result(&sid, &period) {
        None => {
            return Err(ApiError::invalid_state(
                "The model could not be run for this scenario and period (it needs roads and zones).",
            ));
        }
        Some(r) if matches!(r.paths, Maybe::Null) => {
            return Err(ApiError::invalid_state(
                "This model has too many zones to simulate (limit 300).",
            ));
        }
        _ => {}
    }
    let scale = p.opt_f64("scale")?;
    let seed = match p.opt_f64("seed")? {
        Some(x) => x as u32,
        None => (engine_sim::Rng::next_f64(s.ids.rng_mut()) * 4294967296.0) as u32,
    };
    let sim = build(s, &sid, &period, scale, seed)?;
    let routes = sim.routes().len();
    let buses = sim.routes().iter().filter(|r| r.bus).count();
    s.sim = Some(SimState {
        scenario: sid.clone(),
        period: period.clone(),
        seed,
        scale,
        sim,
    });
    let st = state(s)?;
    Ok(
        json!({ "scenario": sid, "period": period, "seed": seed, "routes": routes, "busRoutes": buses, "hud": hud_json(&st.sim) }),
    )
}

fn sim_restart(s: &mut Session, _: &Params) -> ApiResult {
    let (sid, period, seed, scale) = {
        let st = state(s)?;
        (st.scenario.clone(), st.period.clone(), st.seed, st.scale)
    };
    let sim = build(s, &sid, &period, scale, seed)?;
    s.sim = Some(SimState {
        scenario: sid,
        period,
        seed,
        scale,
        sim,
    });
    Ok(hud_json(&state(s)?.sim))
}

fn sim_step(s: &mut Session, p: &Params) -> ApiResult {
    let steps = match p.opt_f64("steps")? {
        Some(n) => n.max(0.0) as u64,
        None => {
            let secs = p.opt_f64("seconds")?.unwrap_or(60.0).clamp(0.0, 7200.0);
            (secs / 0.5).ceil() as u64
        }
    };
    if steps > 144_000 {
        return Err(ApiError::bad_params(
            "That is more than 72,000 simulated seconds in one call; step in smaller pieces.",
        ));
    }
    let st = s.sim.as_mut().ok_or_else(|| {
        ApiError::invalid_state(
            "There is no simulation running. Start one with transport.sim.create.",
        )
    })?;
    for _ in 0..steps {
        st.sim.step();
    }
    Ok(hud_json(&st.sim))
}

fn sim_series(s: &mut Session, p: &Params) -> ApiResult {
    let st = state(s)?;
    let last = p.opt_f64("last")?.map_or(120, |x| x.max(0.0) as usize);
    let series = &st.sim.stats().series;
    let from = series.len().saturating_sub(last);
    let rows: Vec<Value> = series[from..]
        .iter()
        .map(|x| json!({ "t": json_number(x.t), "onRoad": x.n, "queued": x.q, "speedKmh": json_number(x.speed), "done": x.done, "waiting": x.waiting }))
        .collect();
    Ok(json!({ "total": series.len(), "samples": rows }))
}

fn sim_validation(s: &mut Session, p: &Params) -> ApiResult {
    let st = state(s)?;
    let limit = p.opt_f64("limit")?.map_or(50, |x| x.max(0.0) as usize);
    match st.sim.validation() {
        None => Ok(
            json!({ "available": false, "message": "Run the simulation for at least 7 minutes to compare it with the model." }),
        ),
        Some(v) => Ok(json!({
            "available": true, "pass": json_number(v.pass), "roads": v.rows.len(),
            "message": format!("{} of simulated roads have a GEH under 5.", fmt::pct(v.pass, 0)),
            "rows": v.rows.iter().take(limit).map(|r| json!({
                "key": r.key, "link": r.link, "name": r.name, "model": json_number(r.model), "sim": json_number(r.sim), "geh": json_number(r.geh),
            })).collect::<Vec<_>>(),
        })),
    }
}

fn sim_vehicles(s: &mut Session, p: &Params) -> ApiResult {
    let st = state(s)?;
    let limit = p.opt_f64("limit")?.map_or(500, |x| x.max(0.0) as usize);
    let offset = p.opt_f64("offset")?.map_or(0, |x| x.max(0.0) as usize);
    let mut out: Vec<Value> = Vec::new();
    let mut total = 0usize;
    st.sim.for_each_vehicle(|v, arc, lane, lanes| {
        if total >= offset && out.len() < limit {
            let at = if arc.pts.is_empty() { [f64::NAN, f64::NAN] } else { along(&arc.pts, &arc.cum, v.s).p };
            out.push(json!({
                "id": v.id, "kind": v.kind.as_str(), "route": v.route, "arc": arc.key, "link": arc.link, "lane": lane, "lanes": lanes,
                "s": json_number(v.s), "speedKmh": json_number(v.v * 3.6), "acc": json_number(v.acc), "length": json_number(v.len),
                "lat": json_number(at[0]), "lng": json_number(at[1]), "stopped": v.stopped, "stuck": v.stuck,
            }));
        }
        total += 1;
    });
    Ok(json!({ "total": total, "offset": offset, "count": out.len(), "vehicles": out }))
}
