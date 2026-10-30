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
//!   once in the tree, and the job it names runs it with the command of one of its own steps,
//!   compared as shell commands (continued lines joined, redirections and pipes set aside; a
//!   script's last command when it runs one): the same words, a matrix value where the step has
//!   the matrix's variable, a harness filter narrowed, and the step's `--save-…` options left out.
//!   `ClusterFuzzLite`'s violation runs through `.clusterfuzzlite/seeded.sh` instead, which must run
//!   the image of each of the `clusterfuzzlite` job's actions, at the commit it was read against,
//!   with every input the job gives it.
//! - `the_wasm_legs_prove_the_guard_through_golden_run`: `kernel.yml`'s `wasm` job runs `guard
//!   prove --golden-run` with the arguments of one of its own `golden run` steps.
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

/// The commands a shell script runs, each as its words: lines continued with `\` joined, comment
/// lines left out, split at `&&`, `||`, `;`, `|` and line ends, with output redirections (`>`,
/// `>>` and the file each names, `2>&1`) and a subshell's parentheses set aside.
fn commands(script: &str) -> Vec<Vec<String>> {
    let joined = script.replace("\\\r\n", " ").replace("\\\n", " ");
    let mut found = Vec::new();
    for line in joined.lines() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        let parts = line
            .split("&&")
            .flat_map(|p| p.split("||"))
            .flat_map(|p| p.split(';'))
            .flat_map(|p| p.split('|'));
        for part in parts {
            let mut command = Vec::new();
            let mut skip = false;
            for word in words(part) {
                let word = word
                    .trim_start_matches('(')
                    .trim_end_matches(')')
                    .trim_matches(['"', '\''])
                    .to_owned();
                if std::mem::take(&mut skip) {
                    continue;
                }
                if word == ">" || word == ">>" {
                    skip = true;
                } else if !(word.is_empty() || word.starts_with('>') || word.starts_with("2>")) {
                    command.push(word);
                }
            }
            if !command.is_empty() {
                found.push(command);
            }
        }
    }
    found
}

/// The command whose failure a trip step's verdict reads, from the text after its marker: the
/// first command of that line, or, when it runs a script with `bash ... -c '<script>'`, the
/// script's last command.
fn planted_command(text: &str) -> Vec<String> {
    let line = text.lines().next().unwrap_or_default();
    let script = line
        .trim_start()
        .starts_with("bash ")
        .then(|| line.split_once(" -c '"))
        .flatten()
        .and_then(|(_, script)| script.rsplit_once('\''))
        .map(|(script, _)| script);
    match script {
        Some(script) => commands(script).pop().unwrap_or_default(),
        None => commands(line).into_iter().next().unwrap_or_default(),
    }
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
/// filter may be narrowed (the planted word extends the step's), and where the step's options
/// that save what it measured (`--save-…`) are left out: a planted run is never saved.
fn same_command(planted: &[String], step: &[String], job: &Yaml) -> bool {
    let step: Vec<&String> = step.iter().filter(|w| !w.starts_with("--save-")).collect();
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
        let planted: Vec<&str> = runs
            .iter()
            .filter_map(|r| r.split_once(&marker).map(|(_, cmd)| cmd))
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
        if let Some(&(_, script, proved)) = ACTION_SEEDS.iter().find(|s| s.0 == v.name) {
            let run = words(planted.lines().next().unwrap_or_default());
            if run != ["bash", script] {
                found.push(format!(
                    "{}: {file} {job} plants it under {run:?}, not `bash {script}`",
                    v.name
                ));
            }
            match files(script) {
                None => found.push(format!("{}: {script} does not exist", v.name)),
                Some(text) => found.extend(
                    action_seed_problems(&text, &w["jobs"][proved])
                        .into_iter()
                        .map(|p| format!("{}: {script}: {p}", v.name)),
                ),
            }
            continue;
        }
        let planted = planted_command(planted);
        let own = runs
            .iter()
            .filter(|r| !r.contains("--bin trip"))
            .flat_map(|r| commands(r))
            .any(|command| same_command(&planted, &command, j));
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

/// The violations a job runs through a script that stands in for another job's actions, because
/// those actions build a fresh clone, which a planted line never reaches: the violation, the
/// script, and the job (of the same workflow) whose actions it stands in for.
const ACTION_SEEDS: [(&str, &str, &str); 1] = [(
    "clusterfuzzlite",
    ".clusterfuzzlite/seeded.sh",
    "clusterfuzzlite",
)];

/// `ClusterFuzzLite`'s actions, the commit whose `action.yml` files the script was read against,
/// and the image each runs there (`runs.image`, `docker://` set aside).
const CLUSTERFUZZLITE: &str = "google/clusterfuzzlite/actions/";
const CLUSTERFUZZLITE_COMMIT: &str = "52ecc61cb587ee99c26825a112a21abf19c7448c";
const CLUSTERFUZZLITE_IMAGES: [(&str, &str); 2] = [
    (
        "build_fuzzers",
        "gcr.io/oss-fuzz-base/clusterfuzzlite-build-fuzzers:v1",
    ),
    (
        "run_fuzzers",
        "gcr.io/oss-fuzz-base/clusterfuzzlite-run-fuzzers:v1",
    ),
];

/// What is wrong with a script that stands in for `job`'s `ClusterFuzzLite` actions: each action
/// must be pinned to the commit the script was read against, and the script must run its image
/// once, passing every input the job gives it (but the token) as its `action.yml` does, an
/// environment variable of the input's name in capitals with `_` for `-`.
fn action_seed_problems(script: &str, job: &Yaml) -> Vec<String> {
    let runs = commands(script);
    let mut found = Vec::new();
    let mut actions = 0;
    for step in job["steps"].as_vec().into_iter().flatten() {
        let Some(action) = step["uses"]
            .as_str()
            .and_then(|u| u.strip_prefix(CLUSTERFUZZLITE))
        else {
            continue;
        };
        actions += 1;
        let (name, commit) = action.split_once('@').unwrap_or((action, ""));
        if commit != CLUSTERFUZZLITE_COMMIT {
            found.push(format!(
                "{name} is pinned to {commit:?}, not {CLUSTERFUZZLITE_COMMIT}, whose image the \
                 script runs: read the new action.yml and update both"
            ));
            continue;
        }
        let Some(&(_, image)) = CLUSTERFUZZLITE_IMAGES.iter().find(|i| i.0 == name) else {
            found.push(format!("no image is known for the action {name}"));
            continue;
        };
        let docker: Vec<&Vec<String>> = runs
            .iter()
            .filter(|c| c.starts_with(&["docker".to_owned(), "run".to_owned()]))
            .filter(|c| c.last().is_some_and(|last| last == image))
            .collect();
        let [run] = docker.as_slice() else {
            found.push(format!("it runs {image} {} times, not once", docker.len()));
            continue;
        };
        for (key, value) in step["with"].as_hash().into_iter().flatten() {
            let Some(key) = key.as_str() else { continue };
            if key == "github-token" {
                continue;
            }
            let value = match value {
                Yaml::String(s) => s.clone(),
                Yaml::Integer(i) => i.to_string(),
                Yaml::Boolean(b) => b.to_string(),
                other => format!("{other:?}"),
            };
            let pair = format!("{}={value}", key.to_uppercase().replace('-', "_"));
            if !run.windows(2).any(|w| w[0] == "-e" && w[1] == pair) {
                found.push(format!(
                    "{name}'s input {key}: {value} is not passed to {image} as `-e {pair}`"
                ));
            }
        }
    }
    if actions == 0 {
        found.push("the job it stands in for runs no ClusterFuzzLite action".to_owned());
    }
    found
}

fn tree(path: &str) -> Option<String> {
    read_opt(path)
}

/// What is wrong with the WebAssembly legs' proof of the toolchain guard: `kernel.yml`'s `wasm`
/// job must run `guard prove --golden-run` once, with the arguments of one of its own `golden run`
/// steps.
fn guard_problems(workflows: &BTreeMap<String, String>) -> Vec<String> {
    let w = parse(&workflows["kernel.yml"]);
    let job = &w["jobs"]["wasm"];
    let runs: Vec<String> = job["steps"]
        .as_vec()
        .into_iter()
        .flatten()
        .filter_map(|s| s["run"].as_str().map(str::to_owned))
        .collect();
    let marker = "--bin guard -- prove --golden-run";
    let proved: Vec<Vec<String>> = runs
        .iter()
        .filter_map(|r| {
            r.split_once(marker)
                .map(|(_, rest)| words(rest.lines().next().unwrap_or_default()))
        })
        .collect();
    let [proved] = proved.as_slice() else {
        return vec![format!(
            "kernel.yml wasm runs `guard prove --golden-run` {} times, not once",
            proved.len()
        )];
    };
    let golden = ["--bin", "golden", "--", "run"].map(str::to_owned);
    let own = runs.iter().flat_map(|r| commands(r)).any(|c| {
        c.windows(4)
            .position(|w| w == golden)
            .is_some_and(|at| c[at + 4..] == proved[..])
    });
    if own {
        Vec::new()
    } else {
        vec![format!(
            "kernel.yml wasm proves the guard with `golden run {}`, which none of its own golden \
             runs is",
            proved.join(" ")
        )]
    }
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
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

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
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

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
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

    // Seeded: a count gate's seed under a looser limit than its gate, and a fuel seed comparing
    // other files than the gate does.
    let mut looser = workflows.clone();
    let bench = looser.get_mut("kernel-bench.yml").unwrap();
    *bench = bench.replacen(
        "\"${compare[@]}\" --callgrind-limits=\"$CALLGRIND_LIMITS\" --dhat",
        "\"${compare[@]}\" --callgrind-limits=ir=50% --dhat",
        1,
    );
    assert_ne!(&looser, &workflows, "the seeded edit applies");
    assert!(
        trip_problems(&looser, &tree)
            .iter()
            .any(|p| p.starts_with("gungraun:") && p.contains("none of its own steps' commands"))
    );
    let mut other = workflows.clone();
    let bench = other.get_mut("kernel-bench.yml").unwrap();
    *bench = bench.replacen(
        "&& \"$COUNTS\" compare \"$RUNNER_TEMP/base-fuel.txt\"",
        "&& \"$COUNTS\" compare \"$RUNNER_TEMP/head-fuel.txt\"",
        1,
    );
    assert_ne!(&other, &workflows, "the seeded edit applies");
    assert!(
        trip_problems(&other, &tree)
            .iter()
            .any(|p| p.starts_with("fuel:") && p.contains("none of its own steps' commands"))
    );

    // Seeded: `ClusterFuzzLite`'s script fuzzing in another mode than the job's, and the job's
    // action moved to a commit the script was not read against.
    let batch = |path: &str| {
        let text = tree(path)?;
        Some(if path == ".clusterfuzzlite/seeded.sh" {
            text.replace("-e MODE=code-change", "-e MODE=batch")
        } else {
            text
        })
    };
    assert!(
        trip_problems(&workflows, &batch)
            .iter()
            .any(|p| p.contains("input mode: code-change is not passed"))
    );
    let mut moved_action = workflows.clone();
    let fuzz = moved_action.get_mut("kernel-fuzz.yml").unwrap();
    *fuzz = fuzz.replacen(
        "run_fuzzers@52ecc61cb587ee99c26825a112a21abf19c7448c",
        "run_fuzzers@0000000000000000000000000000000000000000",
        1,
    );
    assert!(
        trip_problems(&moved_action, &tree)
            .iter()
            .any(|p| p.contains("run_fuzzers is pinned to"))
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
#[test]
fn the_wasm_legs_prove_the_guard_through_golden_run() {
    let workflows = kernel_workflows();
    let found = guard_problems(&workflows);
    assert!(found.is_empty(), "{found:#?}");

    // Seeded: the proof dropped, and the proof run with arguments no golden step uses.
    let mut dropped = workflows.clone();
    let kernel = dropped.get_mut("kernel.yml").unwrap();
    *kernel = kernel.replace("--bin guard -- prove --golden-run", "--bin guard -- prove");
    assert!(
        guard_problems(&dropped)
            .iter()
            .any(|p| p.contains("0 times"))
    );
    let mut other = workflows.clone();
    let kernel = other.get_mut("kernel.yml").unwrap();
    *kernel = kernel.replace(
        "prove --golden-run --target wasm32-wasip1 --release",
        "prove --golden-run --target wasm32-wasip1 --profile guard",
    );
    assert!(
        guard_problems(&other)
            .iter()
            .any(|p| p.contains("none of its own golden runs"))
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
#[test]
fn a_passed_gate_has_nothing_open() {
    let record = read(RECORD);
    let found = passed_problems(&record);
    assert!(found.is_empty(), "{found:#?}");

    // Seeded: the result says passed while a condition's run is pending and a gate's row open.
    let start = record.find("**Result: ").unwrap();
    let passed = format!(
        "{}**Result: passed{}",
        &record[..start],
        &record[start + 10..]
    );
    // The first condition's row is the section's third table line; the first gate's is
    // `kernel.yml`'s `test`. Each gets a new last cell.
    let text = section(&passed);
    let rows: Vec<&str> = text.lines().filter(|l| l.starts_with("| ")).collect();
    let gate = rows
        .iter()
        .find(|l| l.starts_with("| `kernel.yml` | `test` |"))
        .unwrap();
    let last_cell =
        |row: &str, cell: &str| format!("{} | {cell} |", row.rsplit_once(" | ").unwrap().0);
    let seeded = text
        .replacen(rows[2], &last_cell(rows[2], "pending"), 1)
        .replacen(gate, &last_cell(gate, "Open: #1"), 1);
    let found = passed_problems(&passed.replacen(&text, &seeded, 1));
    assert!(found.iter().any(|p| p.contains("pending")), "{found:#?}");
    assert!(found.iter().any(|p| p.contains("is open")), "{found:#?}");
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
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
