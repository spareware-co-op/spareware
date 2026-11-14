//! The mesh oracles' jobs and comparison (chapter 28.10; piece G154): a night's jobs are one seed's,
//! the kernel's boolean holds to the exact reference on each of them, a correct answer agrees, and
//! a wrong one, a refusal or a broken harness each ends as it should.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests: native arithmetic and indexing, and a failure stops the test"
)]

use std::collections::BTreeSet;

use geom_test::oracle::exact::float;
use geom_test::oracle::meshes::{
    Answer, CGAL, CHERCHI, Compared, Job, MANIFOLD, MANIFOLD_RUST, Op, cuboid, hausdorff, holds,
    jobs, judge, operation, own_answer, own_volume, prism, reference, replay, run, tetrahedron,
    volume,
};
use geom_test::seed::Seed;

fn night() -> Vec<Job> {
    jobs(Seed(0x6d65_7368), 150)
}

/// A job whose answer is not 0: two boxes that overlap.
fn overlapping() -> Job {
    Job {
        id: 7,
        family: "boxes".to_owned(),
        op: Op::Union,
        a: cuboid([0, 0, 0], [4, 4, 4]),
        b: cuboid([2, 2, 2], [6, 6, 6]),
    }
}

fn kind_and_settled(c: Compared) -> (String, String) {
    match c {
        Compared::Differ { signature, settled } => (signature.kind, settled),
        Compared::Agree => panic!("no difference"),
    }
}

#[test]
fn a_seed_gives_one_night() {
    assert_eq!(night(), night());
    assert_ne!(night(), jobs(Seed(1), 150));
    let families: BTreeSet<String> = night().into_iter().map(|j| j.family).collect();
    let all = ["boxes", "flush", "nested", "prisms", "tetrahedra"];
    assert_eq!(families, all.map(str::to_owned).into_iter().collect());
    let ops: BTreeSet<Op> = night().into_iter().map(|j| j.op).collect();
    assert_eq!(ops.len(), 3);
}

#[test]
fn only_this_modules_oracles_are_held() {
    for id in [MANIFOLD, MANIFOLD_RUST, CGAL, CHERCHI] {
        assert!(holds(id));
    }
    assert!(!holds("geomdl"));
}

#[test]
fn the_solids_are_as_drawn() {
    let boxed = cuboid([0, 0, 0], [2, 3, 4]);
    assert_eq!((boxed.vertices.len(), boxed.triangles.len()), (8, 12));
    let triangle = prism(&[[0, 0], [4, 0], [0, 4]], 1, 3);
    assert_eq!((triangle.vertices.len(), triangle.triangles.len()), (6, 8));
    let flat = [[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0]];
    assert_eq!(tetrahedron(flat), None);
    let both_ways = [
        [[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]],
        [[0, 0, 0], [0, 1, 0], [1, 0, 0], [0, 0, 1]],
    ];
    for corners in both_ways {
        let solid = tetrahedron(corners).unwrap();
        let job = Job {
            id: 0,
            family: "tetrahedra".to_owned(),
            op: Op::Intersection,
            a: solid,
            b: cuboid([-1, -1, -1], [2, 2, 2]),
        };
        assert_eq!(
            volume(&run(&job).unwrap()).unwrap(),
            reference(&job).unwrap()
        );
    }
}

#[test]
fn the_kernel_holds_to_the_exact_reference() {
    for job in night() {
        let mesh = run(&job).unwrap_or_else(|why| panic!("job {}: {why}", job.id));
        assert_eq!(
            volume(&mesh).unwrap(),
            reference(&job).unwrap(),
            "job {}: {} {:?}",
            job.id,
            job.family,
            job.op
        );
    }
}

#[test]
fn a_right_answer_agrees() {
    for job in night().into_iter().take(60) {
        assert_eq!(
            judge(MANIFOLD, &job, &own_answer(&job).unwrap()).unwrap(),
            Compared::Agree
        );
        let exact = Answer {
            id: job.id,
            volume: Some(own_volume(&job).unwrap()),
            ..Answer::default()
        };
        assert_eq!(judge(CGAL, &job, &exact).unwrap(), Compared::Agree);
    }
}

#[test]
fn an_exact_oracle_is_held_to_every_bit_and_the_reference_settles_it() {
    let job = overlapping();
    let wrong = Answer {
        id: job.id,
        volume: Some("120000001/1000000".to_owned()),
        ..Answer::default()
    };
    let (kind, settled) = kind_and_settled(judge(CGAL, &job, &wrong).unwrap());
    assert_eq!(kind, "volume");
    assert!(settled.starts_with("theirs"), "{settled}");
}

#[test]
fn a_floating_oracle_is_held_to_what_it_states() {
    let job = overlapping();
    let right = own_answer(&job).unwrap();
    let text = |x: f64| float(x).unwrap();
    // 600 of area at 64 steps of fit is the cap.
    let volume_off = |by: f64, tolerance: f64| Answer {
        volume: Some(text(120.0 + by)),
        tolerance: Some(text(tolerance)),
        ..right.clone()
    };
    assert_eq!(
        judge(MANIFOLD, &job, &volume_off(1.0, 0.01)).unwrap(),
        Compared::Agree
    );
    let (kind, settled) = kind_and_settled(judge(MANIFOLD, &job, &volume_off(10.0, 0.01)).unwrap());
    assert_eq!(kind, "volume");
    assert!(settled.starts_with("theirs"), "{settled}");
    let beyond_the_cap = volume_off(1e6, 1e6);
    assert_eq!(
        kind_and_settled(judge(MANIFOLD, &job, &beyond_the_cap).unwrap()).0,
        "volume"
    );
    let area_off = Answer {
        area: Some(text(1000.0)),
        ..right.clone()
    };
    assert_eq!(
        kind_and_settled(judge(MANIFOLD, &job, &area_off).unwrap()).0,
        "area"
    );
    let mut moved = right.clone();
    let triangles = moved.triangles.as_mut().unwrap();
    triangles[0][0][0] = text(40.0);
    assert_eq!(
        kind_and_settled(judge(MANIFOLD, &job, &moved).unwrap()).0,
        "hausdorff"
    );
}

#[test]
fn a_refusal_is_a_difference() {
    let job = overlapping();
    let refused = Answer {
        id: job.id,
        ..Answer::default()
    };
    assert_eq!(
        kind_and_settled(judge(CGAL, &job, &refused).unwrap()).0,
        "refused"
    );
    // Corners past the frame: the kernel cannot read the job.
    let mut far = overlapping();
    far.a.vertices[0][0] = 1 << 50;
    assert!(run(&far).is_err());
    assert_eq!(judge(CGAL, &far, &refused).unwrap(), Compared::Agree);
    let answered = Answer {
        volume: Some("1".to_owned()),
        ..refused
    };
    assert_eq!(
        kind_and_settled(judge(CGAL, &far, &answered).unwrap()).0,
        "refused"
    );
}

/// One of Manifold's first answers that differed, with its job and night.
#[derive(serde::Deserialize)]
struct Night {
    run: String,
    job: Job,
    answer: Answer,
}

/// Manifold's answers to the four jobs of its first nights that differed (chapter 28.10): job 742
/// of run 43542140594, 1623 of run 43548038439, and 1763 and 1891 of run 43548473272. Each was
/// measured against the export's triangles of the kernel's result, whose corners are rounded to
/// the grid: on 1763 they folded, and the other three will not triangulate.
fn first_nights() -> Vec<Night> {
    include_str!("oracle_meshes/manifold.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn manifolds_first_differences_were_the_rounding_of_our_measure() {
    let nights = first_nights();
    let ids: Vec<u64> = nights.iter().map(|n| n.job.id).collect();
    assert_eq!(ids, [742, 1623, 1763, 1891]);
    for night in &nights {
        let job = &night.job;
        // The kernel's exact volume is the reference's, and Manifold's answer, measured from the
        // kernel's exact faces, agrees with it.
        assert_eq!(
            volume(&run(job).unwrap()).unwrap(),
            reference(job).unwrap(),
            "job {}",
            job.id
        );
        assert_eq!(
            judge(MANIFOLD, job, &night.answer).unwrap(),
            Compared::Agree,
            "job {} of run {}",
            job.id,
            night.run
        );
        // An exact oracle needs only the volume, so it agrees with the right one too.
        let exact = Answer {
            id: job.id,
            volume: Some(own_volume(job).unwrap()),
            ..Answer::default()
        };
        assert_eq!(judge(CGAL, job, &exact).unwrap(), Compared::Agree);
    }
    // The export still refuses the three whose rounded sides cross, and covers 1763's faces.
    let refused: Vec<u64> = nights
        .iter()
        .filter(|n| own_answer(&n.job).is_err())
        .map(|n| n.job.id)
        .collect();
    assert_eq!(refused, [742, 1623, 1891]);
}

#[test]
fn an_answer_off_the_exact_faces_is_a_hausdorff_difference() {
    // Job 1763 with one of Manifold's corners moved far off, its volume ours and its stated
    // tolerance a step: the volume and area pass, so the distance from the exact faces must catch
    // it.
    let night = first_nights().swap_remove(2);
    let mut moved = night.answer.clone();
    let triangles = moved.triangles.as_mut().unwrap();
    let x: f64 = triangles[0][1][0].parse().unwrap();
    triangles[0][1][0] = float(x + 30.0).unwrap();
    moved.tolerance = Some(float(1.0).unwrap());
    let ours = volume(&run(&night.job).unwrap()).unwrap();
    moved.volume = Some(float(ours.to_f64().value()).unwrap());
    let (kind, settled) = kind_and_settled(judge(MANIFOLD, &night.job, &moved).unwrap());
    assert_eq!(kind, "hausdorff");
    assert!(settled.starts_with("theirs"), "{settled}");
}

#[test]
fn a_broken_harness_is_never_a_pass() {
    let job = overlapping();
    let other = Answer {
        id: job.id + 1,
        ..Answer::default()
    };
    assert!(judge(CGAL, &job, &other).is_err());
    let bad = Answer {
        id: job.id,
        volume: Some("a third".to_owned()),
        ..Answer::default()
    };
    assert!(judge(CGAL, &job, &bad).is_err());
    let right = own_answer(&job).unwrap();
    for broken in [
        Answer {
            area: None,
            ..right.clone()
        },
        Answer {
            tolerance: None,
            ..right.clone()
        },
        Answer {
            triangles: None,
            ..right.clone()
        },
        Answer {
            volume: Some("x".to_owned()),
            ..right.clone()
        },
    ] {
        assert!(judge(MANIFOLD, &job, &broken).is_err());
    }
}

#[test]
fn distances_are_measured() {
    let flat = [[[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 3.0, 0.0]]];
    let above = [[[0.0, 0.0, 2.0], [4.0, 0.0, 2.0], [0.0, 3.0, 2.0]]];
    assert_eq!(hausdorff(&flat, &above), 2.0);
    assert_eq!(hausdorff(&flat, &flat), 0.0);
    assert_eq!(hausdorff(&[], &[]), 0.0);
    assert_eq!(hausdorff(&flat, &[]), f64::INFINITY);
    // The sides of a target: its corners, edges and face each the nearest.
    let target = [[[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0]]];
    let near = |p: [f64; 3]| hausdorff(&[[p, p, p]], &target);
    assert!(near([0.0, 0.0, 0.0]) > 3.9);
    let lone = [[[1.0, 1.0, 1.0]; 3]];
    assert!(hausdorff(&lone, &lone) < 1e-12);
}

#[test]
fn a_job_replays_through_the_fixtures() {
    let job = overlapping();
    let logged = operation(&job).unwrap();
    assert!(replay(&logged).is_some());
    let mut other = logged.clone();
    other.name = "nurbs.point".to_owned();
    assert!(replay(&other).is_none());
}
