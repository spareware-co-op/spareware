//! The mesh oracles' jobs and comparison (chapter 28.10; piece G154): a night's jobs are one seed's,
//! the kernel's boolean holds to the exact reference on each of them, a correct answer agrees, and
//! a wrong one, a refusal or a broken harness each ends as it should.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "tests: native arithmetic, casts of small counts and indexing, and a failure stops \
              the test"
)]

use std::collections::BTreeSet;

use core::num::NonZeroU64;

use dashu_ratio::RBig;
use geom_mesh::exact::{MeshError, MeshFault};
use geom_test::oracle::exact::float;
use geom_test::oracle::meshes::{
    Answer, CASE_SETS, CGAL, CHERCHI, Compared, Job, Judge, MANIFOLD, MANIFOLD_MODELS,
    MANIFOLD_RUST, MODEL_FACES, Op, REFUSED_MODELS, SEEDED, Solid, asked, case_jobs, crosses,
    cuboid, hausdorff, held_refusal, holds, jobs, judge, judge_by_reference, model_jobs,
    of_case_set, operation, own_answer, own_volume, prism, recorded_refusal, recorded_refusals,
    reference, refusal, replay, run, stated_input, tetrahedron, triangle_distance,
    triangle_distance_squared, volume,
};
use geom_test::polyhedral::MODELS;
use geom_test::rng::SplitMix64;
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
        other => panic!("no difference: {other:?}"),
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
fn manifold_s_smaller_models_are_a_case_set() {
    let jobs = model_jobs().unwrap();
    // The smaller tier, counted from the files' own face lines.
    let smaller: Vec<&str> = MODELS
        .iter()
        .filter(|(_, text)| text.lines().filter(|l| l.starts_with("f ")).count() <= MODEL_FACES)
        .map(|(name, _)| *name)
        .collect();
    assert_eq!(smaller.len(), 23);
    assert_eq!(jobs.len(), 3 * smaller.len());
    for (k, job) in jobs.iter().enumerate() {
        assert_eq!(job.id, k as u64);
        assert_eq!(job.family, format!("{MANIFOLD_MODELS}/{}", smaller[k / 3]));
        assert_eq!(job.op, [Op::Union, Op::Intersection, Op::Difference][k % 3]);
        // The copy is the model moved by one vector, forward on every axis.
        assert_eq!(job.a.triangles, job.b.triangles);
        let by: BTreeSet<[i64; 3]> = job
            .a
            .vertices
            .iter()
            .zip(&job.b.vertices)
            .map(|(a, b)| [0, 1, 2].map(|i| b[i] - a[i]))
            .collect();
        assert_eq!(by.len(), 1, "{}", job.family);
        assert!(by.iter().all(|d| d.iter().all(|x| *x > 0)), "{by:?}");
        for t in &job.a.triangles {
            assert!(t.iter().all(|&v| (v as usize) < job.a.vertices.len()));
        }
    }
    assert_eq!(model_jobs().unwrap(), jobs, "the same every run");
}

#[test]
fn a_case_set_is_named_and_given_what_it_needs() {
    assert_eq!(CASE_SETS, [SEEDED, MANIFOLD_MODELS]);
    let seed = Some((Seed(0x6d65_7368), 150));
    assert_eq!(case_jobs(SEEDED, seed).unwrap(), night());
    assert_eq!(
        case_jobs(MANIFOLD_MODELS, None).unwrap(),
        model_jobs().unwrap()
    );
    for (cases, given, why) in [
        (SEEDED, None, "needs --seed and --count"),
        (MANIFOLD_MODELS, seed, "takes no --seed or --count"),
        ("ember-pairs", None, "no case set ember-pairs"),
    ] {
        let e = case_jobs(cases, given).unwrap_err();
        assert!(e.contains(why), "{e}");
    }
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
    // A certified lower bound, within a few doubles of 2.
    let two = hausdorff(&flat, &above);
    assert!(two <= 2.0 && two > 2.0 - 1e-12, "{two}");
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

/// The models recorded as refused: each job's model, and the refusal.
#[test]
fn the_models_recorded_as_refused_are_refused_exactly_and_not_asked() {
    let all = model_jobs().unwrap();
    let recorded = recorded_refusals(MANIFOLD_MODELS).unwrap();
    assert_eq!(recorded.len(), 3 * REFUSED_MODELS.len());
    for (job, expected) in &recorded {
        assert_eq!(held_refusal(job, *expected), Ok(()), "{}", job.family);
        assert!(!asked(job));
    }
    // Every other model of the set is asked, and the kernel builds it.
    let asked_jobs: Vec<&Job> = all.iter().filter(|j| asked(j)).collect();
    assert_eq!(asked_jobs.len(), all.len() - recorded.len());
    for job in asked_jobs.iter().step_by(3) {
        assert_eq!(refusal(job).unwrap(), None, "{}", job.family);
    }
    assert_eq!(recorded_refusals(SEEDED).unwrap().len(), 0);
    assert!(recorded_refusals("ember-pairs").is_err());
    assert!(night().iter().all(asked));
    assert!(night().iter().all(|j| of_case_set(SEEDED, j)));
    assert!(all.iter().all(|j| of_case_set(MANIFOLD_MODELS, j)));
    assert!(!of_case_set(SEEDED, &all[0]));
    assert!(!of_case_set(MANIFOLD_MODELS, &night()[0]));
}

#[test]
fn a_recorded_refusal_fails_when_the_kernel_accepts_or_refuses_otherwise() {
    // Planted: a valid model marked as refused.
    let degenerate = MeshError::Fault(MeshFault::DegenerateFace { face: 0 });
    let accepted = held_refusal(&overlapping(), degenerate).unwrap_err();
    assert!(accepted.contains("accepts"), "{accepted}");
    // Planted: a refused model marked with another refusal.
    let (job, _) = recorded_refusals(MANIFOLD_MODELS)
        .unwrap()
        .into_iter()
        .find(|(j, _)| j.family.ends_with("/dedupe_stale_duplicate"))
        .unwrap();
    assert_eq!(recorded_refusal(&job), Some(degenerate));
    let uneven = MeshError::Fault(MeshFault::UnevenEdge { half_edge: 0 });
    let otherwise = held_refusal(&job, uneven).unwrap_err();
    assert!(otherwise.contains("not the recorded"), "{otherwise}");
    let elsewhere = MeshError::Fault(MeshFault::DegenerateFace { face: 1 });
    assert!(held_refusal(&job, elsewhere).is_err());
}

/// Two boxes as one solid that overlap from `[2, 2, 2]` to `[4, 4, 4]`: winding number 2 there.
fn overlapping_boxes() -> Solid {
    let (first, second) = (cuboid([0, 0, 0], [4, 4, 4]), cuboid([2, 2, 2], [6, 6, 6]));
    let mut both = first.clone();
    both.vertices.extend(&second.vertices);
    let shift = first.vertices.len() as u32;
    both.triangles
        .extend(second.triangles.iter().map(|t| t.map(|v| v + shift)));
    both
}

/// A box with a box-shaped cavity: the inner shell faces in, and the winding numbers are 0 and 1.
fn hollow_box() -> Solid {
    let (outer, mut inner) = (cuboid([0, 0, 0], [6, 6, 6]), cuboid([2, 2, 2], [4, 4, 4]));
    for t in &mut inner.triangles {
        t.swap(1, 2);
    }
    let mut both = outer.clone();
    both.vertices.extend(&inner.vertices);
    let shift = outer.vertices.len() as u32;
    both.triangles
        .extend(inner.triangles.iter().map(|t| t.map(|v| v + shift)));
    both
}

fn with(a: Solid, op: Op) -> Job {
    Job {
        id: 3,
        family: "crossing".to_owned(),
        op,
        a,
        b: cuboid([10, 10, 10], [12, 12, 12]),
    }
}

/// An answer of a floating oracle that is the kernel's but for its volume, far off.
fn off_by_volume(job: &Job) -> Answer {
    let right = own_answer(job).unwrap();
    Answer {
        volume: Some(float(1e6).unwrap()),
        ..right
    }
}

#[test]
fn a_model_crosses_itself_by_its_winding_numbers_alone() {
    assert!(crosses(&overlapping_boxes()).unwrap());
    let mut inside_out = cuboid([0, 0, 0], [2, 2, 2]);
    for t in &mut inside_out.triangles {
        t.swap(1, 2);
    }
    assert!(crosses(&inside_out).unwrap(), "a pocket of winding -1");
    assert!(!crosses(&cuboid([0, 0, 0], [2, 2, 2])).unwrap());
    assert!(
        !crosses(&hollow_box()).unwrap(),
        "a cavity is not a crossing"
    );
    // Two boxes apart, as one solid, do not cross either.
    let mut apart = cuboid([0, 0, 0], [2, 2, 2]);
    let other = cuboid([3, 0, 0], [5, 2, 2]);
    apart.vertices.extend(&other.vertices);
    apart
        .triangles
        .extend(other.triangles.iter().map(|t| t.map(|v| v + 8)));
    assert!(!crosses(&apart).unwrap());
    // Moved, the same.
    let mut moved = overlapping_boxes();
    for v in &mut moved.vertices {
        v[0] += 1000;
    }
    assert!(crosses(&moved).unwrap());
}

#[test]
fn a_crossing_model_is_judged_by_the_exact_reference_alone() {
    for op in [Op::Union, Op::Intersection, Op::Difference] {
        let job = with(overlapping_boxes(), op);
        // The kernel reads the crossing by its winding numbers, as the reference does.
        assert_eq!(
            volume(&run(&job).unwrap()).unwrap(),
            reference(&job).unwrap()
        );
        // The rule is found from the model, never a mark: an answer that would differ is
        // outside the oracle's stated input, and the reason cites the oracle's documentation.
        for oracle in [MANIFOLD, CGAL] {
            let wrong = if oracle == CGAL {
                Answer {
                    id: job.id,
                    volume: Some("1".to_owned()),
                    ..Answer::default()
                }
            } else {
                off_by_volume(&job)
            };
            let Compared::Outside { why } = judge(oracle, &job, &wrong).unwrap() else {
                panic!("{oracle}: a crossing model compared with the oracle");
            };
            assert!(why.contains(stated_input(oracle).unwrap()), "{why}");
            // A refusal, too, is not the oracle's to give here.
            let refused = Answer {
                id: job.id,
                ..Answer::default()
            };
            assert!(matches!(
                judge(oracle, &job, &refused).unwrap(),
                Compared::Outside { .. }
            ));
        }
        // An oracle whose documentation says nothing of it is held to the comparison.
        assert_eq!(
            kind_and_settled(judge(MANIFOLD_RUST, &job, &off_by_volume(&job)).unwrap()).0,
            "volume"
        );
    }
    assert_eq!(stated_input(MANIFOLD_RUST), None);
    // Planted: a model that does not cross is never treated as crossing, a cavity included.
    for solid in [cuboid([0, 0, 0], [4, 4, 4]), hollow_box()] {
        let job = with(solid, Op::Union);
        assert_eq!(
            kind_and_settled(judge(MANIFOLD, &job, &off_by_volume(&job)).unwrap()).0,
            "volume"
        );
    }
    // A judge remembers what it read, and gives the same.
    let mut judge_twice = Judge::new();
    let job = with(overlapping_boxes(), Op::Union);
    let first = judge_twice
        .judge(MANIFOLD, &job, &off_by_volume(&job))
        .unwrap();
    let second = judge_twice
        .judge(MANIFOLD, &job, &off_by_volume(&job))
        .unwrap();
    assert_eq!(first, second);
}

#[test]
fn a_crossing_model_whose_kernel_answer_is_off_still_fails() {
    let job = with(overlapping_boxes(), Op::Union);
    let stated = stated_input(MANIFOLD).unwrap();
    let right = volume(&run(&job).unwrap()).unwrap();
    assert!(matches!(
        judge_by_reference(MANIFOLD, &job, stated, Ok(right.clone())).unwrap(),
        Compared::Outside { .. }
    ));
    // Planted: the kernel's volume off by one cubic step, and the kernel refusing.
    let off = right + RBig::ONE;
    let (kind, settled) =
        kind_and_settled(judge_by_reference(MANIFOLD, &job, stated, Ok(off)).unwrap());
    assert_eq!(kind, "reference");
    assert!(settled.starts_with("ours"), "{settled}");
    let (kind, settled) = kind_and_settled(
        judge_by_reference(CGAL, &job, stated, Err("refused".to_owned())).unwrap(),
    );
    assert_eq!(kind, "reference");
    assert!(settled.starts_with("ours"), "{settled}");
}

/// Whether `lo` is a certified lower bound on the distance whose exact square is `square`, and
/// within the comparison's stated closeness of it: 10^-9 steps and 2^-40 of the distance.
fn certified(lo: f64, square: &RBig) -> bool {
    let exact = |x: f64| RBig::try_from(x).unwrap();
    let up = lo + 2e-9 + lo * 2e-12;
    lo >= 0.0 && exact(lo) * exact(lo) <= *square && exact(up) * exact(up) >= *square
}

/// The sliver of issue #951 and a sample on it, from Manifold's model `r1` (its face 5 is the
/// model's own triangle 119).
const SLIVER: [[f64; 3]; 3] = [
    [905_032_541.0, -722_592_755.0, 100_000_200.0],
    [905_032_546.0, -722_592_760.0, 100_000_200.0],
    [907_908_570.0, -719_603_208.0, 101_037_291.0],
];
const ON_IT: [f64; 3] = [905_392_044.625, -722_219_066.0, 100_129_835.601_457_37];

#[test]
fn a_sample_on_a_sliver_far_from_the_origin_is_on_it() {
    let square = triangle_distance_squared(ON_IT, &SLIVER).unwrap();
    let d = triangle_distance(ON_IT, &SLIVER);
    assert!(certified(d, &square), "{d}");
    // On it, but for the rounding of the sample's own coordinates (was 3.03).
    assert!(d < 1e-7, "{d}");
    // The same triangle moved to the origin: the same answer. Each move is exact in doubles.
    let a = SLIVER[0];
    let moved = SLIVER.map(|c| [0, 1, 2].map(|k| c[k] - a[k]));
    let p = [0, 1, 2].map(|k| ON_IT[k] - a[k]);
    assert_eq!(triangle_distance_squared(p, &moved).unwrap(), square);
    assert_eq!(triangle_distance(p, &moved), d);
}

#[test]
#[allow(
    clippy::many_single_char_names,
    reason = "a triangle's corners a, b and c, a sample p and its distance d, as in the formulas"
)]
fn slivers_at_the_grids_far_corner_measure_as_exact_arithmetic_does() {
    let mut rng = SplitMix64::new(Seed(0x951));
    let mut draw = |n: u64| rng.below(NonZeroU64::new(n).unwrap()) as f64;
    // The far corner of the grid, 2^40 - 1 steps on each axis.
    let far = ((1_u64 << 40) - 1) as f64;
    for _ in 0..300 {
        let a = [far - draw(1000), far - draw(1000), far - draw(1000)];
        let thin = [draw(8) + 1.0, -draw(8) - 1.0, draw(3)];
        let long = [-draw(1 << 30) - 1.0, -draw(1 << 30) - 1.0, -draw(1 << 20)];
        let b = [0, 1, 2].map(|k| a[k] - thin[k].abs());
        let c = [0, 1, 2].map(|k| a[k] + long[k]);
        let triangle = [a, b, c];
        for _ in 0..12 {
            // On the triangle as doubles place it, a little off it, and past its sides.
            let (s, t) = (
                draw(1 << 20) / f64::from(1 << 20),
                draw(1 << 20) / f64::from(1 << 20),
            );
            let (s, t) = if s + t > 1.0 {
                (1.0 - s, 1.0 - t)
            } else {
                (s, t)
            };
            let lift = [draw(5) - 2.0, draw(5) - 2.0, draw(5) - 2.0].map(|x| x / 4.0);
            let past = (draw(3) - 1.0) * (1.0 + draw(4));
            let p = [0, 1, 2]
                .map(|k| a[k] + s * (b[k] - a[k]) + t * (c[k] - a[k]) + lift[k] + past * thin[k]);
            let square = triangle_distance_squared(p, &triangle).unwrap();
            let d = triangle_distance(p, &triangle);
            assert!(certified(d, &square), "{triangle:?} {p:?}: {d}");
        }
    }
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
