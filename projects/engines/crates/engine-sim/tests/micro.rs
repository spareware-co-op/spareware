//! Microsimulation tests: the seeded generator, determinism, a free road, a red signal and the lights.
//!
//! The prototype comparison for the traffic wrapper is in construct-transport's tests; these check
//! the generic core on a small network of its own.

use engine_sim::micro::{Control, Kind, Light, MicroArc, MicroNet, SignalPlan};
use engine_sim::{MicroParams, MicroRoute, MicroSim, Mulberry32, Rng};

fn arc(len: f64, to: usize, control: Control) -> MicroArc {
    MicroArc {
        len,
        speed: 50.0,
        ff: len / (50.0 / 3.6),
        lanes: 1,
        bus_lane: false,
        virtual_arc: false,
        to,
        in_bearing: 90.0,
        out_bearing: 90.0,
        control,
        major: false,
        stop: false,
        conflicts: Vec::new(),
    }
}

// Three arcs in a line over nodes 0 to 3; the middle arc ends in the given control
fn line(middle: Control, signals: Vec<SignalPlan>) -> MicroNet {
    MicroNet {
        arcs: vec![
            arc(200.0, 1, Control::Free),
            arc(100.0, 2, middle),
            arc(200.0, 3, Control::Free),
        ],
        node_in: vec![0, 1, 1, 1],
        node_out: vec![1, 1, 1, 0],
        signals,
    }
}

fn cars(rate: f64) -> Vec<MicroRoute> {
    vec![MicroRoute {
        arcs: vec![0, 1, 2],
        rate,
        bus: false,
    }]
}

#[test]
fn mulberry32_matches_the_harness() {
    // First numbers of tools/transport-truth/js-core.mjs `rand`, from Node
    let want: [(u32, [f64; 5]); 3] = [
        (
            1,
            [
                0.6270739405881613,
                0.002735721180215478,
                0.5274470399599522,
                0.9810509674716741,
                0.9683778982143849,
            ],
        ),
        (
            11,
            [
                0.5115870486479253,
                0.5299464082345366,
                0.6081185641232878,
                0.5901576359756291,
                0.8507766961120069,
            ],
        ),
        (
            4294967295,
            [
                0.8964226141106337,
                0.189478256739676,
                0.7156526781618595,
                0.9440599093213677,
                0.8452364315744489,
            ],
        ),
    ];
    for (seed, nums) in want {
        let mut r = Mulberry32::new(seed);
        for (i, w) in nums.iter().enumerate() {
            assert_eq!(r.next_f64(), *w, "seed {seed}, number {i}");
        }
    }
}

#[test]
fn a_closure_is_a_random_source() {
    let mut n = 0.0;
    let mut r = move || {
        n += 0.25;
        n
    };
    assert_eq!(Rng::next_f64(&mut r), 0.25);
    assert_eq!(Rng::next_f64(&mut r), 0.5);
}

#[test]
fn a_run_is_repeatable_and_vehicles_drive_the_route() {
    let run = || {
        let mut sim = MicroSim::new(
            line(Control::Free, Vec::new()),
            cars(0.2),
            MicroParams::new(1.0, 6000.0, 0.1),
            Mulberry32::new(1),
        );
        for _ in 0..1200 {
            sim.step();
        }
        sim
    };
    let a = run();
    let b = run();
    assert_eq!(a.stats(), b.stats());
    assert_eq!(a.arc_passed(), b.arc_passed());
    assert_eq!(a.time(), 600.0);
    assert!(
        a.stats().done > 50,
        "cars finish the route: {}",
        a.stats().done
    );
    assert!(a.vehicles_made() >= a.stats().done);
    assert_eq!(a.stats().series.len(), 120);
    assert_eq!(a.snapshots().len(), 10);
    let mut seen = 0;
    a.for_each_vehicle(|v, arc, lane, lanes| {
        seen += 1;
        assert_eq!(v.arc, arc);
        assert_eq!(v.lane, lane);
        assert_eq!(lanes, 1);
        assert!(v.s >= 0.0 && v.s <= a.net().arcs[arc].len + 20.0);
        assert!(v.v >= 0.0 && v.v <= 25.0);
        assert!(matches!(v.kind, Kind::Car | Kind::Heavy));
    });
    assert_eq!(seen, a.vehicles_on_road());
}

#[test]
fn a_red_signal_holds_traffic_back() {
    // Phase 1 gets no green at all: the light is amber for 3 s of each 60 s cycle, then red
    let red = SignalPlan {
        cycle: 60.0,
        lost: 4.0,
        split: [1.0, 0.0],
        offset: 0.0,
    };
    let run = |control, signals| {
        let mut sim = MicroSim::new(
            line(control, signals),
            cars(0.2),
            MicroParams::new(1.0, 6000.0, 0.0),
            Mulberry32::new(7),
        );
        for _ in 0..1200 {
            sim.step();
        }
        sim
    };
    let free = run(Control::Free, Vec::new());
    let held = run(Control::Signal { plan: 0, phase: 1 }, vec![red]);
    assert!(free.arc_passed()[2] > 0.0);
    assert!(held.arc_passed()[2] < free.arc_passed()[2] / 4.0);
    // a queue stands on the approach
    let mut stopped = 0;
    held.for_each_vehicle(|v, arc, _, _| {
        if arc == 1 && v.v < 0.3 {
            stopped += 1;
        }
    });
    assert!(stopped > 3, "queued vehicles: {stopped}");
}

#[test]
fn lights_follow_the_cycle() {
    // 60 s cycle, 4 s lost: phase 0 is green for 28 s from 0, phase 1 for 28 s from 30
    let plan = SignalPlan {
        cycle: 60.0,
        lost: 4.0,
        split: [0.5, 0.5],
        offset: 0.0,
    };
    let net = MicroNet {
        arcs: vec![
            arc(100.0, 1, Control::Signal { plan: 0, phase: 0 }),
            arc(100.0, 1, Control::Signal { plan: 0, phase: 1 }),
            arc(100.0, 2, Control::Free),
        ],
        node_in: vec![0, 2, 1],
        node_out: vec![2, 1, 0],
        signals: vec![plan],
    };
    let mut sim = MicroSim::new(
        net,
        Vec::new(),
        MicroParams::new(1.0, 6000.0, 0.0),
        Mulberry32::new(1),
    );
    let mut seen = Vec::new();
    for _ in 0..120 {
        sim.step();
        let t = sim.time();
        if [14.0, 28.5, 30.5, 32.0, 45.0, 58.5, 59.5].contains(&t) {
            seen.push((t, sim.light(0), sim.light(1)));
        }
    }
    use Light::{Amber as A, Green as G, Red as R};
    assert_eq!(
        seen,
        vec![
            (14.0, G, R),
            (28.5, A, R),
            (30.5, A, G),
            (32.0, R, G),
            (45.0, R, G),
            (58.5, R, A),
            (59.5, R, A),
        ]
    );
    // an arc with no signal shows green
    assert_eq!(sim.light(2), Light::Green);
}
