//! The sketch oracles' jobs and judge (`geom_test::oracle::sketches`, piece G222).
//!
//! Every job is drawn from a known truth, so the kernel's own solve of each must come back to it:
//! an oracle that answers the truth agrees, one that answers anything else, or refuses, differs, and
//! the difference names the oracle as the side that is off. fiksi's jobs hold only the kinds fiksi
//! can state.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "tests; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_test::Seed;
use geom_test::oracle::geomdl::Compared;
use geom_test::oracle::sketches::{
    Answer, EZPZ, FIKSI, Job, NEAR, Rule, jobs, judge, kernel, near, sketch_text,
};
use geom_test::seed::resolve;

fn seed() -> Seed {
    resolve("oracle_sketches", Seed::DEFAULT).unwrap()
}

fn truth(job: &Job) -> Answer {
    Answer {
        id: job.id,
        points: Some(job.truth.clone()),
    }
}

#[test]
fn a_night_of_jobs_is_the_same_from_its_seed() {
    assert_eq!(jobs(EZPZ, seed(), 50), jobs(EZPZ, seed(), 50));
    assert_ne!(jobs(EZPZ, seed(), 50), jobs(EZPZ, Seed(seed().0 ^ 1), 50));
    assert_eq!(jobs("geomdl", seed(), 5), None);
    let night = jobs(EZPZ, seed(), 40).unwrap();
    assert_eq!(night.len(), 40);
    assert!(night.iter().enumerate().all(|(i, j)| j.id == i as u64));
    // Three to twelve points, the first fixed, every rule on points the job has.
    for job in &night {
        assert!((3..=12).contains(&job.points.len()), "{}", job.points.len());
        assert_eq!(job.rules[0], Rule::Fix { point: 0 });
        assert_eq!(job.truth.len(), job.points.len());
    }
}

#[test]
fn fiksi_sees_only_what_it_can_state() {
    let night = jobs(FIKSI, seed(), 300).unwrap();
    let mut kinds = std::collections::BTreeSet::new();
    for job in &night {
        for rule in &job.rules {
            assert!(
                !matches!(rule, Rule::Midpoint { .. } | Rule::Mirror { .. }),
                "job {}: {rule:?}",
                job.id
            );
            kinds.insert(job.family.clone());
        }
    }
    // ezpz's nights hold every kind.
    let families: String = jobs(EZPZ, seed(), 300)
        .unwrap()
        .iter()
        .map(|j| j.family.clone())
        .collect::<Vec<_>>()
        .join("+");
    for kind in [
        "fix",
        "distance",
        "level",
        "plumb",
        "parallel",
        "perpendicular",
        "angle",
        "from-line",
        "midpoint",
        "mirror",
        "equal-length",
    ] {
        assert!(families.contains(kind), "{kind}");
    }
    assert!(kinds.len() > 10);
}

#[test]
fn the_kernel_solves_every_job_onto_its_truth() {
    for oracle in [EZPZ, FIKSI] {
        for job in jobs(oracle, seed(), 200).unwrap() {
            assert!(
                kernel(&job).unwrap().is_some(),
                "{oracle} job {}: refused\n{}",
                job.id,
                sketch_text(&job).unwrap()
            );
            assert_eq!(
                judge(oracle, &job, &truth(&job)).unwrap(),
                Compared::Agree,
                "{oracle} job {}\n{}",
                job.id,
                sketch_text(&job).unwrap()
            );
        }
    }
}

#[test]
fn an_answer_off_the_truth_or_missing_is_the_oracles() {
    let job = &jobs(EZPZ, seed(), 3).unwrap()[2];
    let differs = |answer: &Answer| match judge(EZPZ, job, answer).unwrap() {
        Compared::Differ { signature, .. } => {
            assert_eq!(signature.oracle, EZPZ);
            assert_eq!(signature.operation, "solve");
            assert_eq!(signature.family, job.family);
            signature.kind
        }
        Compared::Agree => "agree".to_owned(),
    };
    // Two grid steps off on one point is off; a hundredth of one is on.
    let mut moved = truth(job);
    let x: f64 = moved.points.as_ref().unwrap()[1][0].parse().unwrap();
    moved.points.as_mut().unwrap()[1][0] = format!("{:e}", x + 2.0 / 1024.0);
    assert_eq!(differs(&moved), "oracle");
    moved.points.as_mut().unwrap()[1][0] = format!("{:e}", x + 0.01 / 1024.0);
    assert_eq!(differs(&moved), "agree");
    let refused = Answer {
        id: job.id,
        points: None,
    };
    assert_eq!(differs(&refused), "oracle");
    let short = Answer {
        id: job.id,
        points: Some(job.truth[..1].to_vec()),
    };
    assert_eq!(differs(&short), "oracle");
    // An answer to another job, or one that cannot be read, is the harness's fault.
    let other = Answer {
        id: job.id + 1,
        points: Some(job.truth.clone()),
    };
    assert!(judge(EZPZ, job, &other).is_err());
    let unreadable = Answer {
        id: job.id,
        points: Some(vec![["x".to_owned(), "1".to_owned()]; job.truth.len()]),
    };
    assert!(judge(EZPZ, job, &unreadable).is_err());
}

#[test]
fn fiksi_is_held_to_its_own_stopping_rule_and_no_further() {
    let job = &jobs(FIKSI, seed(), 3).unwrap()[2];
    let wide = near(FIKSI, job).unwrap();
    assert_eq!(near(EZPZ, job).unwrap(), NEAR);
    assert!(wide > 4.0 * NEAR && wide < 0.5, "{wide}");
    let moved = |by: f64| {
        let mut answer = truth(job);
        let x: f64 = answer.points.as_ref().unwrap()[1][0].parse().unwrap();
        answer.points.as_mut().unwrap()[1][0] = format!("{:e}", x + by);
        answer
    };
    // Inside fiksi's reach it agrees; past it, or on another branch two units away, it does not.
    assert_eq!(
        judge(FIKSI, job, &moved(0.9 * wide)).unwrap(),
        Compared::Agree
    );
    assert_ne!(
        judge(FIKSI, job, &moved(1.1 * wide)).unwrap(),
        Compared::Agree
    );
    assert_ne!(judge(FIKSI, job, &moved(2.0)).unwrap(), Compared::Agree);
}

#[test]
fn a_job_reads_back_as_the_same_line() {
    for job in jobs(FIKSI, seed(), 20).unwrap() {
        let line = serde_json::to_string(&job).unwrap();
        assert_eq!(serde_json::from_str::<Job>(&line).unwrap(), job);
    }
}
