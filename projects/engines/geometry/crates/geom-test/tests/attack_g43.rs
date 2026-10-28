//! Attacks on piece G43, gate 1 (kernel architecture, chapters 30.3 and 32.43): the gate's record
//! holds what the board asks, and everything it cites exists. The record is read as Markdown
//! (pulldown-cmark), the workflows as YAML (yaml-rust2) and the cited tests as Rust
//! (`geom_test::rust_source`), so a check sees the structure, not a line that looks right:
//!
//! - `the_record_states_every_condition_of_the_board`: the record's first table has one row per
//!   condition of G43's done-when in `30-build-order.json`, in its words and order, each with its
//!   evidence and its CI run.
//! - `every_pull_request_job_has_its_seeded_violation`: the second table lists every job of every
//!   kernel workflow that runs on a pull request, once. Each row cites a test that exists (a
//!   `<file>.rs::<name>` code span whose file defines `name`), a file that exists, a planted
//!   violation (`trip <name>`), the row of another job of its workflow ("The tests of `<job>`"),
//!   or says why it is not a gate ("Not a gate:") or that it is open, with its issue.
//! - `every_planted_violation_runs_in_its_own_job`: each of `geom_test::trip`'s violations is found
//!   once in the tree, and the job it names runs it with the command of one of its own steps: the
//!   same words, a matrix value where the step has the matrix's variable, or a harness filter
//!   narrowed.
//! - `a_passed_gate_has_nothing_open`: while the result is not "passed" anything may be pending;
//!   once it is, no CI run may be pending and no row open.
//! - `the_thread_counts_are_the_boards`: `geom_test::golden::THREAD_COUNTS` is the done-when's
//!   "1, 2, 3 and 8 threads", and every test the record cites for them reaches it.
//!
//! Each check is also shown to fail on a seeded violation.

// Tests may use native arithmetic.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "indices; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use geom_test::golden::THREAD_COUNTS;
use geom_test::rust_source::references;
use geom_test::trip::{VIOLATIONS, apply, violation};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use yaml_rust2::{Yaml, YamlLoader};

const WORKFLOWS: &str = ".github/workflows";
const RECORD: &str = "projects/engines/geometry/architecture/32-decisions.md";
const BOARD: &str = "projects/engines/geometry/architecture/30-build-order.json";
const SECTION: &str = "32.43 ";
const PIECE: &str = "G43";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .to_path_buf()
}

/// A file's text with LF line ends, however the checkout wrote them; `None` when it is missing.
fn read_opt(rel: &str) -> Option<String> {
    std::fs::read_to_string(root().join(rel))
        .ok()
        .map(|t| t.replace("\r\n", "\n"))
}

fn read(rel: &str) -> String {
    read_opt(rel).unwrap_or_else(|| panic!("{rel} is missing"))
}

fn parse(text: &str) -> Yaml {
    let mut docs = YamlLoader::load_from_str(text).unwrap();
    assert_eq!(docs.len(), 1, "one document");
    docs.remove(0)
}

/// Every kernel workflow, by file name, as text.
fn kernel_workflows() -> BTreeMap<String, String> {
    std::fs::read_dir(root().join(WORKFLOWS))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.starts_with("kernel")
                && Path::new(n).extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("yml") || e.eq_ignore_ascii_case("yaml")
                })
        })
        .map(|n| {
            let text = read(&format!("{WORKFLOWS}/{n}"));
            (n, text)
        })
        .collect()
}

/// A mapping's string keys, in file order; none for anything else.
fn keys(y: &Yaml) -> Vec<String> {
    y.as_hash()
        .map(|h| {
            h.keys()
                .filter_map(|k| k.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// A table cell: its text and its code spans.
#[derive(Clone, Debug, Default)]
struct Cell {
    text: String,
    code: Vec<String>,
}

/// The tables of the section whose heading starts with `heading`, each as rows of cells, header
/// row first.
fn tables_under(markdown: &str, heading: &str) -> Vec<Vec<Vec<Cell>>> {
    let mut tables = Vec::new();
    let mut in_section = false;
    let mut title: Option<String> = None;
    let mut table: Option<Vec<Vec<Cell>>> = None;
    let mut cell: Option<Cell> = None;
    for event in Parser::new_ext(markdown, Options::ENABLE_TABLES) {
        match event {
            Event::Start(Tag::Heading { level, .. }) if level <= HeadingLevel::H2 => {
                title = Some(String::new());
            }
            Event::End(TagEnd::Heading(level)) if level <= HeadingLevel::H2 => {
                in_section = title.take().unwrap().starts_with(heading);
            }
            Event::Start(Tag::Table(_)) if in_section => table = Some(Vec::new()),
            Event::End(TagEnd::Table) => {
                if let Some(t) = table.take() {
                    tables.push(t);
                }
            }
            Event::Start(Tag::TableHead | Tag::TableRow) => {
                if let Some(t) = table.as_mut() {
                    t.push(Vec::new());
                }
            }
            Event::Start(Tag::TableCell) if table.is_some() => cell = Some(Cell::default()),
            Event::End(TagEnd::TableCell) => {
                if let (Some(t), Some(c)) = (table.as_mut(), cell.take()) {
                    t.last_mut().unwrap().push(c);
                }
            }
            Event::Text(s) => {
                if let Some(t) = title.as_mut() {
                    t.push_str(&s);
                }
                if let Some(c) = cell.as_mut() {
                    c.text.push_str(&s);
                }
            }
            Event::Code(s) => {
                if let Some(t) = title.as_mut() {
                    t.push_str(&s);
                }
                if let Some(c) = cell.as_mut() {
                    c.text.push_str(&s);
                    c.code.push(s.to_string());
                }
            }
            _ => {}
        }
    }
    tables
}

/// The section's text, from its heading to the next heading of its level.
fn section(markdown: &str) -> String {
    let start = markdown
        .find(&format!("\n## {SECTION}"))
        .expect("the record's section");
    let rest = &markdown[start + 1..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |e| e + 3);
    rest[..end].to_owned()
}

/// The conditions of G43's done-when: its clauses before the sentence on the record, first
/// letter capitalised as the record's rows write them.
fn conditions(board: &str) -> Vec<String> {
    let pieces: serde_json::Value = serde_json::from_str(board).unwrap();
    let done = pieces
        .as_array()
        .or_else(|| pieces["pieces"].as_array())
        .unwrap()
        .iter()
        .find(|p| p["id"] == PIECE)
        .unwrap()["done_when"]
        .as_str()
        .unwrap()
        .to_owned();
    let gate = done
        .split(". A kernel owner records")
        .next()
        .unwrap()
        .to_owned();
    gate.split("; ")
        .map(|c| {
            let mut chars = c.chars();
            let first = chars.next().unwrap().to_uppercase();
            format!("{first}{}", chars.as_str())
        })
        .collect()
}

/// What is wrong with the record's first table against the board.
fn condition_problems(record: &str, board: &str) -> Vec<String> {
    let tables = tables_under(record, SECTION);
    let Some(first) = tables.first() else {
        return vec!["the record has no table of conditions".to_owned()];
    };
    let mut found = Vec::new();
    let rows: Vec<String> = first
        .iter()
        .skip(1)
        .map(|r| r[0].text.trim().to_owned())
        .collect();
    let want = conditions(board);
    if rows != want {
        found.push(format!(
            "the conditions {rows:#?} are not the done-when's {want:#?}"
        ));
    }
    for row in first.iter().skip(1) {
        if row.len() != 3 || row[1].text.trim().is_empty() || row[2].text.trim().is_empty() {
            found.push(format!(
                "{:?} lacks its evidence or its CI run",
                row[0].text
            ));
        }
    }
    found
}

/// A code span's file, resolved from the record's short form: `geom-…` under the kernel's crates,
/// `engine-geometry/…` under the engines' crates.
fn resolve(span: &str) -> Option<String> {
    if span.starts_with("geom-") {
        Some(format!("projects/engines/geometry/crates/{span}"))
    } else if span.starts_with("engine-geometry/") {
        Some(format!("projects/engines/crates/{span}"))
    } else {
        None
    }
}

/// What a code span cites, when it cites something: `Some(Ok(()))` when it exists, `Some(Err)`
/// when it does not, `None` when the span is not a citation.
fn citation(span: &str, files: &dyn Fn(&str) -> Option<String>) -> Option<Result<(), String>> {
    if let Some(name) = span.strip_prefix("trip ") {
        return Some(
            violation(name)
                .map(|_| ())
                .ok_or_else(|| format!("no planted violation {name}")),
        );
    }
    let (file, item) = match span.split_once(".rs::") {
        Some((file, item)) => (format!("{file}.rs"), Some(item)),
        None if Path::new(span)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("rs")) =>
        {
            (span.to_owned(), None)
        }
        None => return None,
    };
    let path = resolve(&file)?;
    let Some(text) = files(&path) else {
        return Some(Err(format!("{path} does not exist")));
    };
    let Some(item) = item else {
        return Some(Ok(()));
    };
    let name = item.rsplit("::").next().unwrap();
    Some(match references(&text, name) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(format!("{path} defines no `{name}`")),
        Err(e) => Err(format!("{path} is not Rust: {e}")),
    })
}

/// Whether a row's text says it is open, with an issue.
fn is_open(text: &str) -> bool {
    text.split(['O', 'o'])
        .skip(1)
        .any(|rest| rest.starts_with("pen: #"))
}

/// The jobs of every kernel workflow that runs on a pull request.
fn pull_request_jobs(workflows: &BTreeMap<String, String>) -> BTreeSet<(String, String)> {
    let mut jobs = BTreeSet::new();
    for (file, text) in workflows {
        let w = parse(text);
        if w["on"]["pull_request"].is_badvalue() {
            continue;
        }
        for job in keys(&w["jobs"]) {
            jobs.insert((file.clone(), job));
        }
    }
    jobs
}

/// What is wrong with the record's second table against the workflows and the tree.
fn gate_problems(
    record: &str,
    workflows: &BTreeMap<String, String>,
    files: &dyn Fn(&str) -> Option<String>,
) -> Vec<String> {
    let tables = tables_under(record, SECTION);
    let Some(gates) = tables.get(1) else {
        return vec!["the record has no table of merge gates".to_owned()];
    };
    let mut found = Vec::new();
    let mut rows: BTreeMap<(String, String), &Cell> = BTreeMap::new();
    for row in gates.iter().skip(1) {
        let (Some(file), Some(job)) = (row[0].code.first(), row[1].code.first()) else {
            found.push(format!("a row names no workflow or job: {:?}", row[0].text));
            continue;
        };
        if rows.insert((file.clone(), job.clone()), &row[2]).is_some() {
            found.push(format!("{file} {job} has two rows"));
        }
    }
    let jobs = pull_request_jobs(workflows);
    for (file, job) in &jobs {
        if !rows.contains_key(&(file.clone(), job.clone())) {
            found.push(format!(
                "{file} {job} runs on a pull request and has no row"
            ));
        }
    }
    for ((file, job), cell) in &rows {
        if !jobs.contains(&(file.clone(), job.clone())) {
            found.push(format!(
                "{file} {job} has a row but no such job runs on a pull request"
            ));
        }
        let mut cited = false;
        for span in &cell.code {
            match citation(span, files) {
                Some(Ok(())) => cited = true,
                Some(Err(e)) => found.push(format!("{file} {job}: {e}")),
                None => {}
            }
        }
        let text = cell.text.trim();
        let delegated = text
            .strip_prefix("The tests of ")
            .and_then(|_| cell.code.first())
            .and_then(|other| rows.get(&(file.clone(), other.clone())))
            .is_some();
        if !(cited || delegated || text.starts_with("Not a gate:") || is_open(text)) {
            found.push(format!(
                "{file} {job} cites no seeded violation and says neither why it is not a gate \
                 nor that it is open"
            ));
        }
    }
    found
}

/// The words of a command, quotes set aside.
fn words(command: &str) -> Vec<String> {
    command
        .replace("${{ ", "${{")
        .replace(" }}", "}}")
        .split_whitespace()
        .map(|w| w.trim_matches(['"', '\'']).to_owned())
        .collect()
}

/// The values a job's matrix gives the variable `name`.
fn matrix_values(job: &Yaml, name: &str) -> Vec<String> {
    let matrix = &job["strategy"]["matrix"];
    let mut values: Vec<String> = matrix[name]
        .as_vec()
        .map(|v| {
            v.iter()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    for entry in matrix["include"].as_vec().into_iter().flatten() {
        if let Some(v) = entry[name].as_str() {
            values.push(v.to_owned());
        }
    }
    values
}

/// Whether `planted`, the command a trip step runs, is `step`, a command of the job's own steps:
/// word for word, where a `${{ matrix.<name> }}` word is one of the matrix's values and a harness
/// filter may be narrowed (the planted word extends the step's).
fn same_command(planted: &[String], step: &[String], job: &Yaml) -> bool {
    planted.len() == step.len()
        && planted.iter().zip(step).all(|(p, s)| {
            if let Some(name) = s
                .strip_prefix("${{")
                .and_then(|r| r.strip_suffix("}}"))
                .map(str::trim)
                .and_then(|r| r.strip_prefix("matrix."))
            {
                matrix_values(job, name).contains(p)
            } else {
                p.starts_with(s.as_str())
            }
        })
}

/// What is wrong with the planted violations against the tree and the jobs they name.
fn trip_problems(
    workflows: &BTreeMap<String, String>,
    files: &dyn Fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut found = Vec::new();
    for v in &VIOLATIONS {
        match files(v.file).map(|text| apply(v, &text)) {
            None => found.push(format!("{}: {} does not exist", v.name, v.file)),
            Some(Err(e)) => found.push(format!("{}: {}: {e}", v.name, v.file)),
            Some(Ok(_)) => {}
        }
        let (file, job) = v.gate.split_once(' ').unwrap();
        let Some(text) = workflows.get(file) else {
            found.push(format!("{}: no workflow {file}", v.name));
            continue;
        };
        let w = parse(text);
        let j = &w["jobs"][job];
        let runs: Vec<String> = j["steps"]
            .as_vec()
            .into_iter()
            .flatten()
            .filter_map(|s| s["run"].as_str().map(str::to_owned))
            .collect();
        let marker = format!("--bin trip -- {} --", v.name);
        let planted: Vec<Vec<String>> = runs
            .iter()
            .filter_map(|r| {
                r.split_once(&marker)
                    .map(|(_, cmd)| words(cmd.lines().next().unwrap_or_default()))
            })
            .collect();
        let [planted] = planted.as_slice() else {
            found.push(format!(
                "{}: {file} {job} runs `trip {}` {} times, not once",
                v.name,
                v.name,
                planted.len()
            ));
            continue;
        };
        let own = runs
            .iter()
            .filter(|r| !r.contains("--bin trip"))
            .flat_map(|r| r.lines().map(str::to_owned).collect::<Vec<_>>())
            .chain(
                runs.iter()
                    .filter(|r| !r.contains("--bin trip"))
                    .map(|r| r.replace('\n', " ")),
            )
            .any(|line| same_command(planted, &words(&line), j));
        if !own {
            found.push(format!(
                "{}: {file} {job} plants it under {planted:?}, which is none of its own steps' \
                 commands",
                v.name
            ));
        }
    }
    found
}

/// What is wrong with a passed result.
fn passed_problems(record: &str) -> Vec<String> {
    let text = section(record);
    let passed = text.contains("**Result: passed");
    if !passed {
        return Vec::new();
    }
    let mut found = Vec::new();
    let tables = tables_under(record, SECTION);
    for row in tables.first().into_iter().flatten().skip(1) {
        if row[2].text.contains("pending") {
            found.push(format!("passed, but {:?} is pending", row[0].text));
        }
    }
    for row in tables.get(1).into_iter().flatten().skip(1) {
        if is_open(&row[2].text) {
            found.push(format!("passed, but {} is open", row[1].text));
        }
    }
    found
}

/// What is wrong with the thread counts: the constant against the done-when, and every test the
/// record's determinism row cites against the constant.
fn thread_problems(
    record: &str,
    board: &str,
    counts: [usize; 4],
    files: &dyn Fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut found = Vec::new();
    let determinism = conditions(board)
        .into_iter()
        .find(|c| c.contains(" threads"))
        .unwrap();
    let listed = determinism
        .split(" modes and ")
        .nth(1)
        .and_then(|r| r.split(" threads").next())
        .unwrap();
    let want: Vec<usize> = listed
        .split([',', ' '])
        .filter_map(|w| w.parse().ok())
        .collect();
    if counts.to_vec() != want {
        found.push(format!(
            "THREAD_COUNTS is {counts:?}, the done-when's are {want:?}"
        ));
    }
    let tables = tables_under(record, SECTION);
    let row = tables
        .first()
        .and_then(|t| t.iter().find(|r| r[0].text == determinism));
    let mut cited = 0;
    for span in row.map(|r| r[1].code.clone()).unwrap_or_default() {
        let Some((file, name)) = span.split_once(".rs::") else {
            continue;
        };
        if !span.contains("thread_count") {
            continue;
        }
        cited += 1;
        let path = resolve(&format!("{file}.rs")).unwrap();
        let reach = files(&path)
            .and_then(|t| references(&t, name).ok().flatten())
            .unwrap_or_default();
        if !reach.iter().any(|p| p.ends_with("THREAD_COUNTS")) {
            found.push(format!("{span} does not run THREAD_COUNTS"));
        }
    }
    if cited < 4 {
        found.push(format!(
            "the determinism row cites {cited} thread-count tests, not the four"
        ));
    }
    found
}

fn tree(path: &str) -> Option<String> {
    read_opt(path)
}

#[test]
fn the_record_states_every_condition_of_the_board() {
    let (record, board) = (read(RECORD), read(BOARD));
    let found = condition_problems(&record, &board);
    assert!(found.is_empty(), "{found:#?}");

    // Seeded: a condition dropped, and one reworded.
    let first = conditions(&board)[1].clone();
    let dropped = record.replacen(&format!("| {first} |"), "| |", 1);
    assert!(
        condition_problems(&dropped, &board)
            .iter()
            .any(|p| p.contains("are not the done-when's"))
    );
    let reworded = record.replacen(&format!("| {first} |"), "| Wide integers are tested |", 1);
    assert!(
        condition_problems(&reworded, &board)
            .iter()
            .any(|p| p.contains("Wide integers are tested"))
    );
}

#[test]
fn every_pull_request_job_has_its_seeded_violation() {
    let (record, workflows) = (read(RECORD), kernel_workflows());
    let found = gate_problems(&record, &workflows, &tree);
    assert!(found.is_empty(), "{found:#?}");

    // Seeded: a cited test renamed, a job with no row, and a row that says nothing.
    let renamed = record.replacen(
        "::gate_build_fails_on_seeded_violations`",
        "::gate_build_fails_on_nothing`",
        1,
    );
    assert!(
        gate_problems(&renamed, &workflows, &tree)
            .iter()
            .any(|p| p.contains("defines no `gate_build_fails_on_nothing`"))
    );
    let mut added = workflows.clone();
    let gate = added.get_mut("kernel-gate.yml").unwrap();
    *gate =
        format!("{gate}  unseeded:\n    runs-on: ubuntu-latest\n    steps:\n      - run: true\n");
    assert!(
        gate_problems(&record, &added, &tree)
            .iter()
            .any(|p| p.contains("kernel-gate.yml unseeded runs on a pull request and has no row"))
    );
    let silent = record.replacen(
        "| `kernel-quality.yml` | `coverage` | R: `geom-test/tests/seeded_gates.rs::seeded_uncovered_code_fails_the_coverage_gate_until_tests_reach_it` |",
        "| `kernel-quality.yml` | `coverage` | It is checked |",
        1,
    );
    assert_ne!(silent, record, "the seeded edit applies");
    assert!(
        gate_problems(&silent, &workflows, &tree)
            .iter()
            .any(|p| p.contains("kernel-quality.yml coverage cites no seeded violation"))
    );
}

#[test]
fn every_planted_violation_runs_in_its_own_job() {
    let workflows = kernel_workflows();
    let found = trip_problems(&workflows, &tree);
    assert!(found.is_empty(), "{found:#?}");

    // Seeded: the trip step removed, its command changed, and its anchor line gone.
    let mut removed = workflows.clone();
    let dev = removed.get_mut("kernel-dev.yml").unwrap();
    *dev = dev.replace("--bin trip -- verus --", "--bin trip-not -- verus --");
    assert!(
        trip_problems(&removed, &tree)
            .iter()
            .any(|p| p.contains("runs `trip verus` 0 times"))
    );
    let mut changed = workflows.clone();
    let lint = changed.get_mut("kernel.yml").unwrap();
    *lint = lint.replacen(
        "--bin trip -- fmt -- cargo fmt --check $KERNEL_CRATES",
        "--bin trip -- fmt -- cargo fmt --check -p geom-num",
        1,
    );
    assert!(
        trip_problems(&changed, &tree)
            .iter()
            .any(|p| p.contains("none of its own steps' commands"))
    );
    let moved = |path: &str| {
        let text = tree(path)?;
        Some(if path.ends_with("sign.rs") {
            text.replace("} else if v > 0 {", "} else if 0 < v {")
        } else {
            text
        })
    };
    assert!(
        trip_problems(&workflows, &moved)
            .iter()
            .any(|p| p.starts_with("kani-num:") && p.contains("0 times"))
    );
}

#[test]
fn a_passed_gate_has_nothing_open() {
    let record = read(RECORD);
    let found = passed_problems(&record);
    assert!(found.is_empty(), "{found:#?}");

    // Seeded: the result says passed while runs are pending and rows open.
    let start = record.find("**Result: ").unwrap();
    let passed = format!(
        "{}**Result: passed{}",
        &record[..start],
        &record[start + 10..]
    );
    let found = passed_problems(&passed);
    assert!(found.iter().any(|p| p.contains("pending")), "{found:#?}");
    assert!(found.iter().any(|p| p.contains("is open")), "{found:#?}");
}

#[test]
fn the_thread_counts_are_the_boards() {
    let (record, board) = (read(RECORD), read(BOARD));
    let found = thread_problems(&record, &board, THREAD_COUNTS, &tree);
    assert!(found.is_empty(), "{found:#?}");

    // Seeded: a count dropped from the constant, and a test going back to its own list.
    assert!(
        thread_problems(&record, &board, [1, 2, 3, 4], &tree)
            .iter()
            .any(|p| p.starts_with("THREAD_COUNTS is [1, 2, 3, 4]"))
    );
    let own_list = |path: &str| {
        let text = tree(path)?;
        Some(if path.ends_with("geom-math/tests/determinism.rs") {
            text.replace(
                "for threads in geom_test::golden::THREAD_COUNTS {",
                "for threads in [1usize, 2, 3] {",
            )
        } else {
            text
        })
    };
    assert!(
        thread_problems(&record, &board, THREAD_COUNTS, &own_list)
            .iter()
            .any(|p| p.contains("does not run THREAD_COUNTS"))
    );
}
