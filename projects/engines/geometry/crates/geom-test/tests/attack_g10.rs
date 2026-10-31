//! Attacks on piece G10, the CI matrix and check tiers (kernel architecture, chapters 21 and
//! 28.17): the workflows do what 28.17's "As built (G10)" table says they do. The workflows are
//! read as YAML (yaml-rust2) and the chapter as Markdown (pulldown-cmark), so a check sees the
//! structure GitHub sees, not a line that looks right:
//!
//! - `every_tier_names_workflows_that_run_on_its_trigger`: the as-built table has 28.17's tiers in
//!   28.17's order; each workflow a row names exists and runs on that tier's trigger (a pull
//!   request; a daily schedule; a weekly schedule; a pull request that changes
//!   `rust-toolchain.toml` or `Cargo.lock`; a `geom-v` tag); and every kernel workflow is named.
//! - `the_release_runs_every_kernel_workflow_whole`: `kernel-release.yml` runs on `geom-v` tags
//!   and nothing else, calls every other kernel workflow once (each takes `workflow_call`),
//!   records the golden store only after every call has passed, and only that job may write.
//! - `the_nightly_campaign_fuzzes_every_target`: `kernel-fuzz-campaign.yml` runs every night, and
//!   its nightly list of targets is `kernel-fuzz.yml`'s libFuzzer matrix, which geom-fuzz's
//!   `tests/attack_g14.rs` holds to the fuzz targets themselves.
//! - `the_matrix_has_every_runner_in_both_modes`: `kernel.yml` tests on the six native runners
//!   G10's done-when names (x86-64 and arm64 Linux, Windows and macOS) in debug and release, with
//!   a `target-cpu=native` leg in release, and on wasm32 under wasmtime and Node.
//! - `one_pinned_cargo_deny_runs_the_facade_tests`: `kernel-build.yml`'s scaffold job, which runs
//!   every test file of the facade (`attack_g12`'s licence-gate attacks start cargo-deny), installs
//!   cargo-deny checked against its SHA-256 first, and every kernel workflow that pins cargo-deny
//!   pins `kernel-gate.yml`'s version and SHA-256.
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

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use yaml_rust2::{Yaml, YamlLoader};

const WORKFLOWS: &str = ".github/workflows";
const RELEASE: &str = "kernel-release.yml";
const CHAPTER: &str = "projects/engines/geometry/architecture/28-testing.md";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .to_path_buf()
}

/// A file's text with LF line ends, however the checkout wrote them, so the seeded edits below,
/// written with LF, apply.
fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel))
        .unwrap()
        .replace("\r\n", "\n")
}

fn parse(text: &str) -> Yaml {
    let mut docs = YamlLoader::load_from_str(text).unwrap();
    assert_eq!(docs.len(), 1, "one document");
    docs.remove(0)
}

/// Whether a file name is a workflow's: GitHub reads workflows from `.yml` and `.yaml` files.
fn is_workflow(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("yml") || e.eq_ignore_ascii_case("yaml"))
}

/// Every kernel workflow, by file name, as text.
fn kernel_workflows() -> BTreeMap<String, String> {
    std::fs::read_dir(root().join(WORKFLOWS))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("kernel") && is_workflow(n))
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

/// A sequence of strings; none for anything else.
fn strings(y: &Yaml) -> Vec<String> {
    y.as_vec()
        .map(|v| {
            v.iter()
                .filter_map(|s| s.as_str().map(str::to_owned))
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

/// The tables under the heading that starts with `heading`, each as rows of cells, header row
/// first.
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

/// 28.17's tiers, from its first table, and the as-built table's rows: each tier and the
/// workflows it names.
fn tiers(chapter: &str) -> (Vec<String>, Vec<(String, Vec<String>)>) {
    let tables = tables_under(chapter, "28.17 ");
    assert!(
        tables.len() >= 2,
        "28.17 has its tier table and the as-built table"
    );
    let tiers = tables[0]
        .iter()
        .skip(1)
        .map(|row| row[0].text.trim().to_owned())
        .collect();
    let built = tables[1]
        .iter()
        .skip(1)
        .map(|row| {
            let named = row[1]
                .code
                .iter()
                .filter(|c| is_workflow(c))
                .cloned()
                .collect();
            (row[0].text.trim().to_owned(), named)
        })
        .collect();
    (tiers, built)
}

/// The cron schedules of a workflow, each as its five fields.
fn crons(w: &Yaml) -> Vec<Vec<String>> {
    w["on"]["schedule"]
        .as_vec()
        .map(|v| {
            v.iter()
                .filter_map(|s| s["cron"].as_str())
                .map(|c| c.split_whitespace().map(str::to_owned).collect())
                .collect()
        })
        .unwrap_or_default()
}

/// Whether a workflow runs on a tier's trigger; `None` for a tier this check does not know.
fn runs_on_tier(tier: &str, w: &Yaml) -> Option<bool> {
    let day_fields = |c: &Vec<String>| c.len() == 5 && c[2] == "*" && c[3] == "*";
    Some(match tier {
        "Every pull request" => !w["on"]["pull_request"].is_badvalue(),
        "Nightly" => crons(w).iter().any(|c| day_fields(c) && c[4] == "*"),
        "Weekly" => crons(w).iter().any(|c| day_fields(c) && c[4] != "*"),
        "Toolchain or dependency change" => {
            let paths = strings(&w["on"]["pull_request"]["paths"]);
            ["rust-toolchain.toml", "Cargo.lock"]
                .iter()
                .all(|p| paths.iter().any(|q| q == p))
        }
        "Each release and edition" => strings(&w["on"]["push"]["tags"])
            .iter()
            .any(|t| t == "geom-v*"),
        _ => return None,
    })
}

/// What is wrong with the as-built table against the workflows.
fn tier_violations(chapter: &str, workflows: &BTreeMap<String, String>) -> Vec<String> {
    let mut found = Vec::new();
    let (tiers, built) = tiers(chapter);
    let built_tiers: Vec<String> = built.iter().map(|(t, _)| t.clone()).collect();
    if built_tiers != tiers {
        found.push(format!(
            "the as-built table's tiers {built_tiers:?} are not 28.17's {tiers:?}"
        ));
    }
    let mut named = BTreeSet::new();
    for (tier, files) in &built {
        if files.is_empty() {
            found.push(format!("the tier {tier:?} names no workflow"));
        }
        for file in files {
            named.insert(file.clone());
            let Some(text) = workflows.get(file) else {
                found.push(format!("{tier:?} names {file}, which does not exist"));
                continue;
            };
            match runs_on_tier(tier, &parse(text)) {
                Some(true) => {}
                Some(false) => found.push(format!("{file} does not run on the tier {tier:?}")),
                None => found.push(format!("no trigger is known for the tier {tier:?}")),
            }
        }
    }
    for file in workflows.keys().filter(|f| !named.contains(*f)) {
        found.push(format!("{file} is in no tier of the as-built table"));
    }
    found
}

/// What is wrong with the release workflow against the other kernel workflows.
fn release_violations(workflows: &BTreeMap<String, String>) -> Vec<String> {
    let mut found = Vec::new();
    let Some(text) = workflows.get(RELEASE) else {
        return vec![format!("there is no {RELEASE}")];
    };
    let release = parse(text);
    let on = &release["on"];
    if keys(on) != ["push"]
        || keys(&on["push"]) != ["tags"]
        || strings(&on["push"]["tags"]) != ["geom-v*"]
    {
        found.push(format!(
            "{RELEASE} must run on a geom-v tag and nothing else: {on:?}"
        ));
    }
    let jobs = &release["jobs"];
    let mut calls: BTreeMap<String, String> = BTreeMap::new();
    for job in keys(jobs) {
        if let Some(uses) = jobs[job.as_str()]["uses"].as_str() {
            let file = uses.trim_start_matches("./.github/workflows/").to_owned();
            if calls.insert(file.clone(), job.clone()).is_some() {
                found.push(format!("{RELEASE} calls {file} twice"));
            }
            if !keys(&jobs[job.as_str()]["permissions"]).is_empty() {
                found.push(format!("{RELEASE}'s job {job} asks for permissions"));
            }
        }
    }
    let others: BTreeSet<String> = workflows
        .keys()
        .filter(|f| *f != RELEASE)
        .cloned()
        .collect();
    let called: BTreeSet<String> = calls.keys().cloned().collect();
    if called != others {
        found.push(format!(
            "{RELEASE} calls {called:?}, not every other kernel workflow {others:?}"
        ));
    }
    for file in &others {
        let w = parse(&workflows[file]);
        if !keys(&w["on"]).iter().any(|k| k == "workflow_call") {
            found.push(format!(
                "{file} cannot be called: it has no workflow_call trigger"
            ));
        }
    }
    let freeze = &jobs["freeze"];
    let needs: BTreeSet<String> = strings(&freeze["needs"]).into_iter().collect();
    let callers: BTreeSet<String> = calls.values().cloned().collect();
    if needs != callers {
        found.push(format!(
            "the freeze job needs {needs:?}, not every call {callers:?}"
        ));
    }
    if freeze["permissions"]["contents"].as_str() != Some("write") {
        found.push("the freeze job cannot attach the store to the release".to_owned());
    }
    let top = &release["permissions"];
    if keys(top)
        .iter()
        .any(|k| top[k.as_str()].as_str() == Some("write"))
    {
        found.push(format!("{RELEASE} lets every job write"));
    }
    let runs: String = freeze["steps"]
        .as_vec()
        .map(|s| {
            s.iter()
                .filter_map(|step| step["run"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    for needle in ["golden -- check", "gh release upload", "cmp -s"] {
        if !runs.contains(needle) {
            found.push(format!("the freeze job does not run {needle:?}"));
        }
    }
    found
}

/// The JSON list a `fromJSON(... && '<list>' || ...)` expression gives when its condition holds.
fn first_json_list(expression: &str) -> Vec<String> {
    let start = expression.find("'[").expect("a quoted JSON list");
    let rest = &expression[start + 1..];
    let end = rest.find("]'").expect("the list's end");
    serde_json::from_str(&rest[..=end]).unwrap()
}

/// What is wrong with the nightly campaign against kernel-fuzz.yml's libFuzzer matrix.
fn campaign_violations(campaign: &str, fuzz: &str) -> Vec<String> {
    let mut found = Vec::new();
    let c = parse(campaign);
    if !crons(&c)
        .iter()
        .any(|f| f.len() == 5 && f[2] == "*" && f[3] == "*" && f[4] == "*")
    {
        found.push("the campaign does not run every night".to_owned());
    }
    let job = &c["jobs"]["campaign"];
    let condition = job["if"].as_str().unwrap_or_default();
    if !condition.contains("github.event_name == 'schedule'") {
        found.push(format!("the campaign skips the nightly run: {condition}"));
    }
    let target = job["strategy"]["matrix"]["target"]
        .as_str()
        .unwrap_or_default();
    if !target.contains("github.event_name == 'schedule'") {
        found.push(format!(
            "the nightly list is not chosen by the schedule: {target}"
        ));
    }
    let nightly: BTreeSet<String> = first_json_list(target).into_iter().collect();
    let f = parse(fuzz);
    let all: BTreeSet<String> = strings(&f["jobs"]["libfuzzer"]["strategy"]["matrix"]["target"])
        .into_iter()
        .collect();
    if all.is_empty() || nightly != all {
        found.push(format!(
            "the nightly campaign fuzzes {nightly:?}, not kernel-fuzz.yml's targets {all:?}"
        ));
    }
    found
}

/// What is wrong with kernel.yml's matrix.
fn matrix_violations(kernel: &str) -> Vec<String> {
    let mut found = Vec::new();
    let k = parse(kernel);
    let test = &k["jobs"]["test"];
    let modes: BTreeSet<String> = strings(&test["strategy"]["matrix"]["mode"])
        .into_iter()
        .collect();
    if modes != BTreeSet::from(["debug".to_owned(), "release".to_owned()]) {
        found.push(format!(
            "the native legs run in {modes:?}, not debug and release"
        ));
    }
    let runners: BTreeSet<String> = test["strategy"]["matrix"]["target"]
        .as_vec()
        .map(|v| {
            v.iter()
                .filter_map(|t| t["os"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let wanted: BTreeSet<String> = [
        "ubuntu-latest",
        "ubuntu-24.04-arm",
        "windows-latest",
        "windows-11-arm",
        "macos-latest",
        "macos-15-intel",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    if runners != wanted {
        found.push(format!(
            "the native runners are {runners:?}, not {wanted:?}"
        ));
    }
    let native_in_release = test["steps"].as_vec().is_some_and(|steps| {
        steps.iter().any(|s| {
            s["env"]["RUSTFLAGS"]
                .as_str()
                .is_some_and(|f| f.contains("target-cpu=native"))
                && s["if"]
                    .as_str()
                    .is_some_and(|c| c.contains("matrix.mode == 'release'"))
        })
    });
    if !native_in_release {
        found.push("no release step builds for the runner's own CPU".to_owned());
    }
    let wasm = kernel.contains("wasmtime") && kernel.contains("node");
    if k["jobs"]["wasm"].is_badvalue() || !wasm {
        found.push("no wasm32 job runs under wasmtime and Node".to_owned());
    }
    found
}

/// What breaks "one pinned cargo-deny": a kernel workflow that names cargo-deny's version or its
/// archive's SHA-256 without both, or with values other than `kernel-gate.yml`'s; and the scaffold
/// job of `kernel-build.yml`, which runs `attack_g12`'s cargo-deny tests, not installing it before
/// it runs the facade's tests.
fn deny_pin_violations(workflows: &BTreeMap<String, String>) -> Vec<String> {
    const PINS: [&str; 2] = ["CARGO_DENY_VERSION", "CARGO_DENY_SHA256"];
    let mut found = Vec::new();
    let pins_of = |text: &str| -> Vec<Option<String>> {
        let w = parse(text);
        PINS.iter()
            .map(|p| w["env"][*p].as_str().map(str::to_owned))
            .collect()
    };
    let Some(gate) = workflows.get("kernel-gate.yml") else {
        return vec!["kernel-gate.yml is missing".to_owned()];
    };
    let wanted = pins_of(gate);
    if wanted.iter().any(Option::is_none) {
        found.push("kernel-gate.yml does not pin cargo-deny's version and SHA-256".to_owned());
    }
    for (name, text) in workflows {
        let pins = pins_of(text);
        if pins.iter().any(Option::is_some) && pins != wanted {
            found.push(format!(
                "{name} pins cargo-deny as {pins:?}, not kernel-gate.yml's {wanted:?}"
            ));
        }
    }
    let Some(build) = workflows.get("kernel-build.yml") else {
        found.push("kernel-build.yml is missing".to_owned());
        return found;
    };
    let steps = parse(build)["jobs"]["scaffold"]["steps"]
        .as_vec()
        .cloned()
        .unwrap_or_default();
    let runs: Vec<&str> = steps.iter().filter_map(|s| s["run"].as_str()).collect();
    let install = runs
        .iter()
        .position(|r| r.contains("sha256sum --check --strict") && r.contains("cargo-deny-"));
    let facade = runs
        .iter()
        .position(|r| r.contains("engine-geometry/tests/*.rs"));
    match (install, facade) {
        (Some(i), Some(f)) if i < f => {}
        _ => found.push(
            "kernel-build.yml's scaffold job does not install the pinned cargo-deny before it runs \
             the facade's tests"
                .to_owned(),
        ),
    }
    if !pins_of(build).iter().all(Option::is_some) {
        found.push("kernel-build.yml does not pin cargo-deny".to_owned());
    }
    found
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn one_pinned_cargo_deny_runs_the_facade_tests() {
    let workflows = kernel_workflows();
    assert_eq!(deny_pin_violations(&workflows), Vec::<String>::new());

    // Seeded violations: each must be reported.
    let build = &workflows["kernel-build.yml"];
    let seeded = |text: String| {
        assert_ne!(&text, build);
        let mut w = workflows.clone();
        w.insert("kernel-build.yml".to_owned(), text);
        deny_pin_violations(&w)
    };
    let other_version = build.replacen(
        "CARGO_DENY_VERSION: \"0.20.2\"",
        "CARGO_DENY_VERSION: \"0.20.3\"",
        1,
    );
    assert!(
        seeded(other_version)
            .iter()
            .any(|v| v.contains("not kernel-gate.yml's"))
    );
    let unchecked = build.replacen(
        "echo \"${CARGO_DENY_SHA256}  $name.tar.gz\" | sha256sum --check --strict\n",
        "",
        1,
    );
    assert!(
        seeded(unchecked)
            .iter()
            .any(|v| v.contains("before it runs"))
    );
    let no_pin = build.replacen("  CARGO_DENY_SHA256:", "  NOT_THE_PIN:", 1);
    assert!(seeded(no_pin).iter().any(|v| v.contains("not kernel-gate")));
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn every_tier_names_workflows_that_run_on_its_trigger() {
    let chapter = read(CHAPTER);
    let workflows = kernel_workflows();
    assert_eq!(tier_violations(&chapter, &workflows), Vec::<String>::new());

    // Seeded violations: each must be reported.
    let mut no_nightly = workflows.clone();
    let text = no_nightly.get_mut("kernel-nightly.yml").unwrap();
    assert!(text.contains("  schedule:\n"));
    *text = text.replacen("  schedule:\n", "  schedulx:\n", 1);
    assert!(
        tier_violations(&chapter, &no_nightly)
            .iter()
            .any(|v| v.contains("kernel-nightly.yml does not run on the tier \"Nightly\""))
    );
    let unnamed = chapter.replacen("`kernel-gate.yml`, ", "", 1);
    assert_ne!(unnamed, chapter);
    assert!(
        tier_violations(&unnamed, &workflows)
            .iter()
            .any(|v| v.contains("kernel-gate.yml is in no tier"))
    );
    let wrong_tier = chapter.replacen(
        "| Weekly | `kernel-quality.yml` |",
        "| Weekly | `kernel.yml` |",
        1,
    );
    assert_ne!(wrong_tier, chapter);
    assert!(
        tier_violations(&wrong_tier, &workflows)
            .iter()
            .any(|v| v.contains("kernel.yml does not run on the tier \"Weekly\""))
    );
    let missing = chapter.replacen(
        "`kernel-release.yml` | The format",
        "`kernel-relaese.yml` | The format",
        1,
    );
    assert_ne!(missing, chapter);
    assert!(
        tier_violations(&missing, &workflows)
            .iter()
            .any(|v| v.contains("kernel-relaese.yml, which does not exist"))
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_release_runs_every_kernel_workflow_whole() {
    let workflows = kernel_workflows();
    assert_eq!(release_violations(&workflows), Vec::<String>::new());

    // Seeded violations: each must be reported.
    let seeded = |file: &str, from: &str, to: &str| {
        let mut w = workflows.clone();
        let text = w.get_mut(file).unwrap();
        assert!(text.contains(from), "{file} lacks {from:?}");
        *text = text.replacen(from, to, 1);
        release_violations(&w)
    };
    assert!(
        seeded(
            RELEASE,
            "  nightly:\n    uses: ./.github/workflows/kernel-nightly.yml\n",
            ""
        )
        .iter()
        .any(|v| v.contains("not every other kernel workflow"))
    );
    assert!(
        seeded("kernel-dev.yml", "  workflow_call:\n", "")
            .iter()
            .any(|v| v.contains("kernel-dev.yml cannot be called"))
    );
    assert!(
        seeded(
            RELEASE,
            "    tags: [\"geom-v*\"]\n",
            "    branches: [main]\n"
        )
        .iter()
        .any(|v| v.contains("must run on a geom-v tag"))
    );
    assert!(
        seeded(
            RELEASE,
            " fuzz-campaign, quality, nightly]",
            " fuzz-campaign, quality]"
        )
        .iter()
        .any(|v| v.contains("the freeze job needs"))
    );
    assert!(
        seeded(
            RELEASE,
            "  pull-requests: read\n",
            "  pull-requests: write\n"
        )
        .iter()
        .any(|v| v.contains("lets every job write"))
    );
    assert!(
        seeded(RELEASE, "cmp -s", "true")
            .iter()
            .any(|v| v.contains("cmp -s"))
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_nightly_campaign_fuzzes_every_target() {
    let campaign = read(".github/workflows/kernel-fuzz-campaign.yml");
    let fuzz = read(".github/workflows/kernel-fuzz.yml");
    assert_eq!(campaign_violations(&campaign, &fuzz), Vec::<String>::new());

    // Seeded violations: each must be reported.
    let dropped = campaign.replacen("'[\"curves_fuzz\", ", "'[", 1);
    assert_ne!(dropped, campaign);
    assert!(
        campaign_violations(&dropped, &fuzz)
            .iter()
            .any(|v| v.contains("not kernel-fuzz.yml's targets"))
    );
    let added = fuzz.replacen("[curves_fuzz, ", "[curves_fuzz, extra_fuzz, ", 1);
    assert_ne!(added, fuzz);
    assert!(
        campaign_violations(&campaign, &added)
            .iter()
            .any(|v| v.contains("not kernel-fuzz.yml's targets"))
    );
    let weekly = campaign.replacen("\"47 12 * * *\"", "\"47 12 * * 6\"", 1);
    assert_ne!(weekly, campaign);
    assert!(
        campaign_violations(&weekly, &fuzz)
            .iter()
            .any(|v| v.contains("every night"))
    );
}

#[cfg_attr(
    target_family = "wasm",
    ignore = "reads the repository's files, which a WebAssembly test run cannot"
)]
#[test]
fn the_matrix_has_every_runner_in_both_modes() {
    let kernel = read(".github/workflows/kernel.yml");
    assert_eq!(matrix_violations(&kernel), Vec::<String>::new());

    // Seeded violations: each must be reported.
    let no_debug = kernel.replacen("mode: [release, debug]", "mode: [release]", 1);
    assert_ne!(no_debug, kernel);
    assert!(
        matrix_violations(&no_debug)
            .iter()
            .any(|v| v.contains("not debug and release"))
    );
    let no_intel_mac = kernel.replacen("os: macos-15-intel", "os: macos-latest", 1);
    assert_ne!(no_intel_mac, kernel);
    assert!(
        matrix_violations(&no_intel_mac)
            .iter()
            .any(|v| v.contains("native runners"))
    );
    let no_native = kernel.replace("-C target-cpu=native", "");
    assert_ne!(no_native, kernel);
    assert!(
        matrix_violations(&no_native)
            .iter()
            .any(|v| v.contains("own CPU"))
    );
}
