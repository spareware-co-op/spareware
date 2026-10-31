//! The `ssi` and `ssi-runner` binaries (G160): the contract the corpus's scorer calls, the catalogue
//! as files, and the gate's exit codes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output as Ran};

use geom_test::ssi::case::Case;
use geom_test::ssi::catalogue;
use geom_test::ssi::cover::{compare, effective};
use geom_test::ssi::gate::{Report, Scored, Verdict};
use geom_test::ssi::output::Output;

const SSI: &str = env!("CARGO_BIN_EXE_ssi");
const RUNNER: &str = env!("CARGO_BIN_EXE_ssi-runner");

/// A folder of its own: `cases` and `raw` inside.
fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("geom-test-ssi-tool-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("cases")).expect("made");
    fs::create_dir_all(dir.join("raw")).expect("made");
    dir
}

/// Runs the tool on `args`.
fn ssi(args: &[&str]) -> Ran {
    Command::new(SSI).args(args).output().expect("it runs")
}

/// `path` as an argument.
fn arg(path: &Path) -> &str {
    path.to_str().expect("a plain path")
}

/// What `ran` printed.
fn printed(ran: &Ran) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    )
}

/// Runs the runner on the catalogue case `id` as `stand_in`, and writes the scorer's line.
fn answered(dir: &Path, id: &str, stand_in: &str) {
    let case_path = dir.join("cases").join(format!("{id}.json"));
    let out_path = dir.join("raw").join(format!("{id}.json"));
    let ran = Command::new(RUNNER)
        .env("SSI_STAND_IN", stand_in)
        .args([arg(&case_path), "default", "1e-6", arg(&out_path)])
        .output()
        .expect("it runs");
    assert!(ran.status.success(), "{}", printed(&ran));
    let case = Case::read(&case_path).expect("read");
    let text = fs::read_to_string(&out_path).expect("written");
    let out: Output = serde_json::from_str(&text).expect("read");
    let seen = compare(&case, &out, effective(&case, Some(1e-5)));
    let verdict = if !seen.missing.is_empty() {
        Verdict::Incomplete
    } else if seen.spurious > 0 {
        Verdict::WrongTopology
    } else {
        Verdict::Pass
    };
    let line = Scored {
        id: id.to_owned(),
        verdict,
        flags: Vec::new(),
        time_ms: Some(out.time_ms),
    };
    let scores = dir.join("scores.json");
    fs::write(scores, serde_json::to_string(&[line]).expect("written")).expect("wrote");
}

/// `ssi gate` on the folder, with `more` arguments.
fn gate(dir: &Path, engine: &str, more: &[&str]) -> Ran {
    let (cases, raw, scores) = (dir.join("cases"), dir.join("raw"), dir.join("scores.json"));
    let mut args = vec![
        "gate",
        "--cases",
        arg(&cases),
        "--raw",
        arg(&raw),
        "--scores",
        arg(&scores),
        "--engine",
        engine,
        "--mode",
        "default",
    ];
    args.extend(more);
    ssi(&args)
}

#[test]
fn the_tool_checks_the_ceilings_and_the_catalogue() {
    let ran = ssi(&["check"]);
    assert!(ran.status.success(), "{}", printed(&ran));
    assert!(
        printed(&ran).contains("15 catalogue cases, 0 failed"),
        "{}",
        printed(&ran)
    );
}

#[test]
fn the_catalogue_is_written_lists_and_places() {
    let dir = scratch("catalogue");
    let cases = dir.join("cases");
    assert!(ssi(&["catalogue", "--out", arg(&cases)]).status.success());
    assert_eq!(
        fs::read_dir(&cases).expect("listed").count(),
        catalogue::all().len()
    );
    // Anything that is not a case is not read.
    fs::write(cases.join("README.txt"), "not a case").expect("wrote");
    let listed = ssi(&["list", "--cases", arg(&cases)]);
    assert!(listed.status.success());
    let text = printed(&listed);
    assert!(text.contains("TangentCurve 3"), "{text}");
    assert!(text.contains("Transversal 5"), "{text}");
    let placed = ssi(&["place", "--cases", arg(&cases)]);
    assert!(placed.status.success(), "{}", printed(&placed));
    let text = printed(&placed);
    assert!(text.contains("15 cases, 0 failed"), "{text}");
    assert!(text.contains("f_seam_line 2^23 moved 0"), "{text}");
    fs::remove_dir_all(&dir).expect("removed");
}

#[test]
fn a_case_that_cannot_be_placed_fails_the_tool_and_the_runner() {
    let dir = scratch("unplaced");
    let mut case = catalogue::all().remove(0);
    case.cov_tol = 1e-13;
    let path = dir.join("cases").join(format!("{}.json", case.id));
    fs::write(&path, serde_json::to_string(&case).expect("written")).expect("wrote");
    let placed = ssi(&["place", "--cases", arg(&dir.join("cases"))]);
    assert_eq!(placed.status.code(), Some(1));
    assert!(
        printed(&placed).contains("cannot be placed"),
        "{}",
        printed(&placed)
    );
    let out = dir.join("out.json");
    let ran = Command::new(RUNNER)
        .args([arg(&path), "default", "default", arg(&out)])
        .output()
        .expect("it runs");
    assert_eq!(ran.status.code(), Some(1));
    assert!(!out.exists());
    fs::remove_dir_all(&dir).expect("removed");
}

#[test]
fn a_kernel_kind_off_its_closed_form_fails_the_tool() {
    let dir = scratch("kinds");
    let mut case = catalogue::all().remove(0);
    let moved = serde_json::to_string(&case.truth[0])
        .expect("written")
        .replace("-1.0", "-1.4");
    case.truth[0] = serde_json::from_str(&moved).expect("read");
    let path = dir.join("cases").join(format!("{}.json", case.id));
    fs::write(&path, serde_json::to_string(&case).expect("written")).expect("wrote");
    let placed = ssi(&["place", "--cases", arg(&dir.join("cases"))]);
    assert_eq!(placed.status.code(), Some(1), "{}", printed(&placed));
    assert!(
        printed(&placed).contains("off its closed forms"),
        "{}",
        printed(&placed)
    );
    fs::remove_dir_all(&dir).expect("removed");
}

#[test]
fn the_runner_keeps_the_contract() {
    let dir = scratch("runner");
    catalogue::write(&dir.join("cases")).expect("wrote");
    let id = "f16_cylinders_external";
    answered(&dir, id, "truth");
    let text = fs::read_to_string(dir.join("raw").join(format!("{id}.json"))).expect("written");
    let json: serde_json::Value = serde_json::from_str(&text).expect("read");
    assert_eq!(json["status"], 1000);
    assert!(json["time_ms"].is_number());
    assert_eq!(json["curves"].as_array().expect("curves").len(), 1);
    assert_eq!(json["curves"][0]["type"], "TANGENT");
    assert_eq!(json["log2"], 23);
    // With no stand-in named, it is the truth.
    let (case_path, out) = (
        dir.join("cases").join(format!("{id}.json")),
        dir.join("out.json"),
    );
    let ran = Command::new(RUNNER)
        .env_remove("SSI_STAND_IN")
        .args([arg(&case_path), "default", "default", arg(&out)])
        .output()
        .expect("it runs");
    assert!(ran.status.success());
    let read: Output =
        serde_json::from_str(&fs::read_to_string(&out).expect("written")).expect("read");
    // At the default tolerance the scale follows the case's own tolerance.
    assert_eq!(read.log2, 13);
    assert_eq!(read.curves.len(), 1);
    fs::remove_dir_all(&dir).expect("removed");
}

#[test]
fn the_runner_refuses_what_it_cannot_do() {
    let dir = scratch("refuse");
    catalogue::write(&dir.join("cases")).expect("wrote");
    let case = dir.join("cases").join("f_seam_line.json");
    let out = dir.join("out.json");
    let run = |stand_in: &str, args: &[&str]| {
        Command::new(RUNNER)
            .env("SSI_STAND_IN", stand_in)
            .args(args)
            .output()
            .expect("it runs")
            .status
            .code()
    };
    assert_eq!(run("truth", &[]), Some(2));
    assert_eq!(
        run("truth", &[arg(&case), "default", "soon", arg(&out)]),
        Some(2)
    );
    assert_eq!(
        run("psychic", &[arg(&case), "default", "1e-6", arg(&out)]),
        Some(2)
    );
    assert_eq!(
        run("truth", &["no-such.json", "default", "1e-6", arg(&out)]),
        Some(2)
    );
    let nowhere = dir.join("no-such-folder").join("out.json");
    assert_eq!(
        run("truth", &[arg(&case), "default", "1e-6", arg(&nowhere)]),
        Some(2)
    );
    fs::remove_dir_all(&dir).expect("removed");
}

#[test]
fn the_gate_passes_a_right_answer_and_writes_its_report() {
    let dir = scratch("pass");
    catalogue::write(&dir.join("cases")).expect("wrote");
    answered(&dir, "f_seam_line", "truth");
    let report = dir.join("report.json");
    let ran = gate(&dir, "truth", &["--report", arg(&report)]);
    assert!(ran.status.success(), "{}", printed(&ran));
    assert!(
        printed(&ran).contains("truth: 1 cases, 0 failed"),
        "{}",
        printed(&ran)
    );
    let read: Report =
        serde_json::from_str(&fs::read_to_string(&report).expect("written")).expect("read");
    assert!(
        read.credit.contains("Yang, Jia and Yan 2023"),
        "{}",
        read.credit
    );
    assert_eq!(read.rows.len(), 1);
    // Asked to break a rule it did not, it fails.
    let held = gate(&dir, "truth", &["--must-fail", "verdict"]);
    assert_eq!(held.status.code(), Some(1));
    assert!(
        printed(&held).contains("expected to break verdict"),
        "{}",
        printed(&held)
    );
    fs::remove_dir_all(&dir).expect("removed");
}

#[test]
fn the_gate_fails_a_wrong_answer_unless_that_is_what_it_was_asked() {
    let dir = scratch("fail");
    catalogue::write(&dir.join("cases")).expect("wrote");
    answered(&dir, "f_seam_line", "shifted");
    let ran = gate(&dir, "shifted", &[]);
    assert_eq!(ran.status.code(), Some(1));
    assert!(
        printed(&ran).contains("f_seam_line: Incomplete: verdict"),
        "{}",
        printed(&ran)
    );
    assert!(
        gate(&dir, "shifted", &["--must-fail", "verdict"])
            .status
            .success()
    );
    assert_eq!(
        gate(&dir, "shifted", &["--must-fail", "too-wide"])
            .status
            .code(),
        Some(1)
    );
    fs::remove_dir_all(&dir).expect("removed");
}

#[test]
fn a_flagged_run_is_judged_by_its_measured_ceilings() {
    let dir = scratch("measure");
    catalogue::write(&dir.join("cases")).expect("wrote");
    let id = "f16_cylinders_external";
    answered(&dir, id, "flagged");
    // `measure` reads every case of its folder, so leave it this one.
    let keep = dir.join("kept");
    fs::create_dir_all(&keep).expect("made");
    fs::copy(
        dir.join("cases").join(format!("{id}.json")),
        keep.join(format!("{id}.json")),
    )
    .expect("copied");
    let measured = dir.join("measured.json");
    let (raw, out) = (dir.join("raw"), arg(&measured));
    let ran = ssi(&[
        "measure",
        "--cases",
        arg(&keep),
        "--raw",
        arg(&raw),
        "--out",
        out,
        "--why",
        "a test",
    ]);
    assert!(ran.status.success(), "{}", printed(&ran));
    assert!(gate(&dir, "flagged", &["--ceilings", out]).status.success());
    // The register has nothing flagged, so over it the same run fails.
    assert!(
        gate(&dir, "flagged", &["--must-fail", "over-ceiling"])
            .status
            .success()
    );
    answered(&dir, id, "enlarged");
    let wide = gate(
        &dir,
        "enlarged",
        &["--ceilings", out, "--must-fail", "too-wide"],
    );
    assert!(wide.status.success(), "{}", printed(&wide));
    assert!(
        gate(
            &dir,
            "enlarged",
            &["--ceilings", out, "--must-fail", "over-ceiling"]
        )
        .status
        .success()
    );
    answered(&dir, id, "misplaced");
    assert!(
        gate(
            &dir,
            "misplaced",
            &["--ceilings", out, "--must-fail", "no-feature"]
        )
        .status
        .success()
    );
    answered(&dir, id, "wrong-reason");
    assert!(
        gate(
            &dir,
            "wrong-reason",
            &["--ceilings", out, "--must-fail", "wrong-kind"]
        )
        .status
        .success()
    );
    fs::remove_dir_all(&dir).expect("removed");
}

#[test]
fn two_reports_are_compared_bit_for_bit() {
    let dir = scratch("same");
    catalogue::write(&dir.join("cases")).expect("wrote");
    let id = "f_seam_line";
    let (first, second, third) = (dir.join("a.json"), dir.join("b.json"), dir.join("c.json"));
    answered(&dir, id, "truth");
    assert!(
        gate(&dir, "truth", &["--report", arg(&first)])
            .status
            .success()
    );
    answered(&dir, id, "truth");
    assert!(
        gate(&dir, "truth", &["--report", arg(&second)])
            .status
            .success()
    );
    answered(&dir, id, "shifted");
    assert_eq!(
        gate(&dir, "shifted", &["--report", arg(&third)])
            .status
            .code(),
        Some(1)
    );
    assert!(ssi(&["same", arg(&first), arg(&second)]).status.success());
    let ran = ssi(&["same", arg(&first), arg(&third)]);
    assert_eq!(ran.status.code(), Some(1));
    assert!(
        printed(&ran).contains("f_seam_line differs"),
        "{}",
        printed(&ran)
    );
    fs::remove_dir_all(&dir).expect("removed");
}

#[test]
fn what_cannot_be_read_is_a_usage_error() {
    let dir = scratch("usage");
    let missing = dir.join("nothing");
    let code = |args: &[&str]| ssi(args).status.code();
    assert_eq!(code(&["list", "--cases", arg(&missing)]), Some(2));
    assert_eq!(code(&["place", "--cases", arg(&missing)]), Some(2));
    assert_eq!(code(&["catalogue", "--out", arg(&missing)]), Some(2));
    assert_eq!(code(&["same", arg(&missing), arg(&missing)]), Some(2));
    assert_eq!(
        gate(&dir, "x", &["--ceilings", arg(&missing)])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(gate(&dir, "x", &[]).status.code(), Some(2));
    assert_eq!(code(&["nonsense"]), Some(2));
    fs::remove_dir_all(&dir).expect("removed");
}
