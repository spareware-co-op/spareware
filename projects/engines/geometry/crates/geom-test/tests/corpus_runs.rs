// SPDX-License-Identifier: Apache-2.0
//! The tests that hold the kernel to the rational reference run in every optimised leg of the
//! workflows (Gate 4, G156): none is compiled out or ignored there.
//!
//! From #887 (decision 32.53) every test leg of `kernel.yml` turned debug assertions on in its
//! optimised build too, so the five tests of `corpus_polyhedral.rs` ignored under
//! `debug_assertions` ran in no job at all, and every check stayed green. This file reads, for each
//! leg, the configuration its build has, and each such test's attributes in that configuration:
//!
//! - **The tests of this kind**: every test file of a kernel crate that reads `geom_test`'s flat
//!   reference, its polyhedral corpus or its second mesh checker (`flat_reference`, `polyhedral`,
//!   `second_mesh`), found by its `use` paths, so a new file of the kind is held at once.
//! - **The legs**: every workflow step that runs the suite optimised (`cargo nextest run ...
//!   --release` for the host), with the environment the workflow, its job and the step give it, and
//!   the workspace's release profile: whether debug assertions are on, the optimisation level
//!   (`cfg(unoptimised)` at level 0, geom-test's `build.rs`), and any `--cfg` in `RUSTFLAGS`.
//! - **A test is skipped in a leg** when a `#[cfg]` in force on it, its module's or its file's is
//!   not true there, or an `#[ignore]` is in force (a `cfg_attr` whose predicate is not false).
//!   An option the leg does not settle counts as possibly true, so an unknown gate is a skip. The
//!   one exception is a test ignored outright that an optimised workflow step runs by name with
//!   `--ignored` (the large tier, `kernel.yml`'s `large-models`).
//!
//! A planted skip of each kind must be found: a test's gate changed back to `debug_assertions`, a
//! test or a whole file compiled out, a plain `#[ignore]`, and a leg built at `opt-level` 0 or with
//! `--cfg unoptimised`.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native counts, and a failure stops the test"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use geom_test::recursion::cfg::{Truth, effective_under, predicate_under};
use syn::visit::Visit;
use syn::{Attribute, Item, Meta, UseTree};
use yaml_rust2::{Yaml, YamlLoader};

/// The modules of `geom_test` whose readers are tests of this kind.
const KINDS: [&str; 3] = ["flat_reference", "polyhedral", "second_mesh"];

/// The repository's root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .to_path_buf()
}

/// The `geom_test` modules a file's `use` declarations name, anywhere in it.
#[derive(Default)]
struct Uses(Vec<String>);

impl Uses {
    fn tree(&mut self, prefix: &[String], tree: &UseTree) {
        match tree {
            UseTree::Path(p) => {
                let mut longer = prefix.to_vec();
                longer.push(p.ident.to_string());
                self.tree(&longer, &p.tree);
            }
            UseTree::Name(n) => self.end(prefix, &n.ident.to_string()),
            UseTree::Rename(r) => self.end(prefix, &r.ident.to_string()),
            UseTree::Glob(_) => self.end(prefix, "*"),
            UseTree::Group(g) => {
                for t in &g.items {
                    self.tree(prefix, t);
                }
            }
        }
    }

    fn end(&mut self, prefix: &[String], last: &str) {
        let mut path = prefix.to_vec();
        path.push(last.to_owned());
        if path.first().map(String::as_str) == Some("geom_test") {
            self.0.extend(path.get(1).cloned());
        }
    }
}

impl<'ast> Visit<'ast> for Uses {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.tree(&[], &item.tree);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        let names: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        if names.len() > 1 && names[0] == "geom_test" {
            self.0.push(names[1].clone());
        }
        syn::visit::visit_path(self, path);
    }
}

/// Every test file of the kind, by its path from the repository's root, with its source.
fn files_of_the_kind() -> BTreeMap<String, String> {
    let crates = root().join("projects/engines/geometry/crates");
    let mut out = BTreeMap::new();
    for krate in std::fs::read_dir(&crates).unwrap() {
        let tests = krate.unwrap().path().join("tests");
        let Ok(entries) = std::fs::read_dir(&tests) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path)
                .unwrap()
                .replace("\r\n", "\n");
            let mut uses = Uses::default();
            uses.visit_file(&syn::parse_file(&source).unwrap());
            if uses.0.iter().any(|m| KINDS.contains(&m.as_str())) {
                let name = path.strip_prefix(root()).unwrap();
                out.insert(name.to_string_lossy().replace('\\', "/"), source);
            }
        }
    }
    out
}

/// One optimised leg: where it is, the step it runs, and the options its build settles.
#[derive(Clone, Debug)]
struct Leg {
    place: String,
    run: String,
    debug_assertions: bool,
    unoptimised: bool,
    /// Every `--cfg` name `RUSTFLAGS` gives it.
    cfgs: Vec<String>,
}

impl Leg {
    /// An option's value in this leg's build of the suite's tests for its host.
    fn option(&self, option: &Meta) -> Truth {
        let known = |yes: bool| if yes { Truth::True } else { Truth::False };
        match option {
            Meta::Path(p) => {
                let name = p.get_ident().map(ToString::to_string).unwrap_or_default();
                if self.cfgs.contains(&name) {
                    return Truth::True;
                }
                match name.as_str() {
                    "test" => Truth::True,
                    "debug_assertions" => known(self.debug_assertions),
                    "unoptimised" => known(self.unoptimised),
                    // Interpreter, proof, documentation and coverage builds are other jobs'.
                    "miri" | "kani" | "doc" | "doctest" | "coverage_nightly" => Truth::False,
                    _ => Truth::Unknown,
                }
            }
            Meta::NameValue(nv) => {
                let value = match &nv.value {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) => s.value(),
                    _ => return Truth::Unknown,
                };
                let name = nv.path.get_ident().map(ToString::to_string);
                // The suite's legs build for their host, never for WebAssembly.
                match (name.as_deref(), value.as_str()) {
                    (Some("target_family"), "wasm")
                    | (Some("target_os"), "wasi")
                    | (Some("target_arch"), "wasm32") => Truth::False,
                    _ => Truth::Unknown,
                }
            }
            Meta::List(_) => Truth::Unknown,
        }
    }
}

/// A YAML mapping's string values.
fn strings(map: &Yaml) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(hash) = map.as_hash() {
        for (k, v) in hash {
            let value = match v {
                Yaml::String(s) | Yaml::Real(s) => s.clone(),
                Yaml::Integer(i) => i.to_string(),
                Yaml::Boolean(b) => b.to_string(),
                _ => continue,
            };
            if let Some(key) = k.as_str() {
                out.insert(key.to_owned(), value);
            }
        }
    }
    out
}

/// The workspace's release profile: whether it turns debug assertions on, and its `opt-level`.
fn release_profile(manifest: &str) -> (bool, String) {
    let table: toml::Table = toml::from_str(manifest).unwrap();
    let release = table
        .get("profile")
        .and_then(|p| p.get("release"))
        .cloned()
        .unwrap_or(toml::Value::Table(toml::Table::new()));
    let debug = release
        .get("debug-assertions")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    let level = match release.get("opt-level") {
        Some(toml::Value::Integer(i)) => i.to_string(),
        Some(toml::Value::String(s)) => s.clone(),
        _ => "3".to_owned(),
    };
    (debug, level)
}

/// Every optimised leg of the suite in these workflows (file name, text), built under `manifest`.
fn legs(workflows: &[(String, String)], manifest: &str) -> Vec<Leg> {
    let (profile_debug, profile_level) = release_profile(manifest);
    let mut out = Vec::new();
    for (file, text) in workflows {
        let Ok(docs) = YamlLoader::load_from_str(text) else {
            continue;
        };
        let Some(doc) = docs.first() else {
            continue;
        };
        let Some(jobs) = doc["jobs"].as_hash() else {
            continue;
        };
        for (name, job) in jobs {
            let Some(steps) = job["steps"].as_vec() else {
                continue;
            };
            for step in steps {
                let Some(run) = step["run"].as_str() else {
                    continue;
                };
                let run = run.trim().to_owned();
                if !(run.contains("cargo nextest run") && run.contains("--release"))
                    || run.contains("--target")
                {
                    continue;
                }
                let mut env = strings(&doc["env"]);
                env.extend(strings(&job["env"]));
                env.extend(strings(&step["env"]));
                let flags = env.get("RUSTFLAGS").cloned().unwrap_or_default();
                let words: Vec<&str> = flags.split_whitespace().collect();
                let cfgs = words
                    .windows(2)
                    .filter(|w| w[0] == "--cfg")
                    .map(|w| w[1].to_owned())
                    .collect();
                let debug_assertions = env
                    .get("CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS")
                    .map_or(profile_debug, |v| v == "true");
                let level = env
                    .get("CARGO_PROFILE_RELEASE_OPT_LEVEL")
                    .cloned()
                    .unwrap_or_else(|| profile_level.clone());
                out.push(Leg {
                    place: format!(
                        "{file}, job {}, step {:?}",
                        name.as_str().unwrap_or("?"),
                        step["name"].as_str().unwrap_or("?")
                    ),
                    run,
                    debug_assertions,
                    unoptimised: level == "0",
                    cfgs,
                });
            }
        }
    }
    out
}

/// Every workflow, by file name, with its text.
fn workflows() -> Vec<(String, String)> {
    let dir = root().join(".github/workflows");
    let mut out: Vec<(String, String)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "yml" || e == "yaml"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (
                name,
                std::fs::read_to_string(&p).unwrap().replace("\r\n", "\n"),
            )
        })
        .collect();
    out.sort();
    out
}

/// Whether some optimised workflow step runs `test`, ignored, by name (`--ignored`).
fn run_by_name(test: &str, workflows: &[(String, String)]) -> bool {
    workflows.iter().any(|(_, text)| {
        let docs = YamlLoader::load_from_str(text).unwrap_or_default();
        docs.first()
            .and_then(|d| d["jobs"].as_hash().cloned())
            .into_iter()
            .flat_map(|jobs| jobs.into_iter().map(|(_, job)| job))
            .flat_map(|job| job["steps"].as_vec().cloned().unwrap_or_default())
            .filter_map(|step| step["run"].as_str().map(str::to_owned))
            .any(|run| run.contains("--release") && run.contains("--ignored") && run.contains(test))
    })
}

/// What skips a test with these attributes (its file's, its modules' and its own) in `leg`, if
/// anything does.
fn skip(attrs: &[Attribute], leg: &Leg) -> Option<String> {
    let option = |m: &Meta| leg.option(m);
    for meta in effective_under(attrs, &option) {
        match &meta {
            Meta::List(list) if list.path.is_ident("cfg") => {
                let Ok(p) = list.parse_args::<Meta>() else {
                    return Some("a cfg it cannot read".to_owned());
                };
                let value = predicate_under(&p, &option);
                if value != Truth::True {
                    return Some(format!("compiled out: cfg({}) is {value:?}", quote(&p)));
                }
            }
            _ if meta.path().is_ident("ignore") => {
                return Some(format!("ignored: {}", quote(&meta)));
            }
            _ => {}
        }
    }
    None
}

fn quote(meta: &Meta) -> String {
    quote::ToTokens::to_token_stream(meta).to_string()
}

/// Every test of a file's items, with the attributes in force around it, under `outer`.
fn tests_in(items: &[Item], outer: &[Attribute], out: &mut Vec<(String, Vec<Attribute>)>) {
    for item in items {
        match item {
            Item::Fn(f) if f.attrs.iter().any(|a| a.path().is_ident("test")) => {
                let mut attrs = outer.to_vec();
                attrs.extend(f.attrs.iter().cloned());
                out.push((f.sig.ident.to_string(), attrs));
            }
            Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    let mut attrs = outer.to_vec();
                    attrs.extend(m.attrs.iter().cloned());
                    tests_in(inner, &attrs, out);
                }
            }
            Item::Macro(m) if m.mac.tokens.to_string().contains("# [test]") => {
                out.push((
                    "a test inside a macro, which this reader cannot judge".to_owned(),
                    vec![syn::parse_quote!(#[ignore])],
                ));
            }
            _ => {}
        }
    }
}

/// Every skip of a test of these files in these legs, as `file::test in leg: why`; with how many
/// tests were read.
fn skips(
    files: &BTreeMap<String, String>,
    legs: &[Leg],
    workflows: &[(String, String)],
) -> (Vec<String>, usize) {
    let mut out = Vec::new();
    let mut count = 0;
    for (name, source) in files {
        let file = syn::parse_file(source).unwrap();
        let mut tests = Vec::new();
        tests_in(&file.items, &file.attrs, &mut tests);
        count += tests.len();
        // The crate the file tests, and the legs whose step runs it.
        let krate = name.split('/').rev().nth(2).unwrap();
        let reach: Vec<&Leg> = legs
            .iter()
            .filter(|l| {
                l.run.contains("$KERNEL_CRATES")
                    || l.run.contains("--workspace")
                    || l.run.contains(&format!("-p {krate}"))
            })
            .collect();
        for (test, attrs) in &tests {
            for leg in &reach {
                let Some(why) = skip(attrs, leg) else {
                    continue;
                };
                let by_name = attrs.iter().any(|a| a.path().is_ident("ignore"))
                    && why.starts_with("ignored")
                    && run_by_name(test, workflows);
                if !by_name {
                    out.push(format!("{name}::{test} in {}: {why}", leg.place));
                }
            }
        }
    }
    (out, count)
}

fn manifest() -> String {
    std::fs::read_to_string(root().join("Cargo.toml"))
        .unwrap()
        .replace("\r\n", "\n")
}

#[test]
#[cfg_attr(
    target_os = "wasi",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
fn every_reference_test_runs_in_every_optimised_leg() {
    let workflows = workflows();
    let legs = legs(&workflows, &manifest());
    // The pull request's suite and the merge queue's, so a renamed step cannot leave nothing read.
    for job in ["kernel.yml, job linux,", "kernel.yml, job suite,"] {
        assert!(
            legs.iter().any(|l| l.place.starts_with(job)),
            "no optimised leg of the suite in {job} {legs:#?}"
        );
    }
    let files = files_of_the_kind();
    assert!(
        files.contains_key("projects/engines/geometry/crates/geom-test/tests/corpus_polyhedral.rs"),
        "{:?}",
        files.keys()
    );
    let (skipped, count) = skips(&files, &legs, &workflows);
    println!(
        "{count} tests in {} files read in {} optimised legs: {:?}",
        files.len(),
        legs.len(),
        files.keys()
    );
    assert_eq!(skipped, Vec::<String>::new());
}

/// `text` with `from` replaced by `to` `times` times (every time for 0), which must change it.
fn plant(text: &str, from: &str, to: &str, times: usize) -> String {
    assert!(text.contains(from), "nothing to plant in: {from}");
    if times == 0 {
        text.replace(from, to)
    } else {
        text.replacen(from, to, times)
    }
}

/// The skips of `corpus_polyhedral.rs` as changed by `source`, in the workflows with `kernel.yml`
/// changed by `workflow`.
fn planted(source: impl Fn(&str) -> String, workflow: impl Fn(&str) -> String) -> Vec<String> {
    let workflows: Vec<(String, String)> = workflows()
        .into_iter()
        .map(|(name, text)| {
            let changed = if name == "kernel.yml" {
                workflow(&text)
            } else {
                text
            };
            (name, changed)
        })
        .collect();
    let legs = legs(&workflows, &manifest());
    let path = "projects/engines/geometry/crates/geom-test/tests/corpus_polyhedral.rs";
    let text = std::fs::read_to_string(root().join(path))
        .unwrap()
        .replace("\r\n", "\n");
    let files = BTreeMap::from([(path.to_owned(), source(&text))]);
    skips(&files, &legs, &workflows).0
}

/// The tests a list of skips names, once each.
fn named(skipped: &[String]) -> Vec<String> {
    let mut names: Vec<String> = skipped
        .iter()
        .map(|s| {
            s.split(" in ")
                .next()
                .unwrap()
                .rsplit("::")
                .next()
                .unwrap()
                .to_owned()
        })
        .collect();
    names.dedup();
    names
}

/// The tests of `corpus_polyhedral.rs` ignored unoptimised: the five the optimised legs dropped
/// from #887 to this change, the figures' test of every selection now in its four parts.
const GATED: [&str; 8] = [
    "ember_s_figure_24_general_fuse_is_the_unit_cells_and_the_reference",
    "ember_s_figure_24_inside_exactly_one_is_the_unit_cells_and_the_reference",
    "ember_s_figures_commute_with_exact_transforms",
    "every_other_selection_of_ember_s_figure_24_is_the_unit_cells_and_the_reference",
    "every_selection_of_ember_s_figure_25_is_the_unit_cells_and_the_reference",
    "manifold_s_models_commute_with_exact_transforms",
    "manifold_s_models_unite_alone_and_with_a_moved_copy_to_the_reference",
    "the_polyhedral_corpus_is_the_same_on_every_thread_count",
];

#[test]
#[cfg_attr(
    target_os = "wasi",
    ignore = "reads the repository's files, which WASI cannot without a preopened folder"
)]
fn a_planted_skip_is_found() {
    let same = |t: &str| t.to_owned();
    // As built, nothing.
    assert_eq!(planted(same, same), Vec::<String>::new());
    // A gate put back on debug assertions, which every optimised leg turns on.
    let back = planted(
        |s| plant(s, "    unoptimised,\n", "    debug_assertions,\n", 1),
        same,
    );
    assert_eq!(
        named(&back),
        ["every_selection_of_ember_s_figure_25_is_the_unit_cells_and_the_reference"]
    );
    // A test compiled out in those legs, and one ignored outright.
    let test = "fn the_polyhedral_corpus_is_the_same_on_every_target()";
    for skip in ["#[cfg(not(debug_assertions))]", "#[ignore = \"planted\"]"] {
        let skipped = planted(|s| plant(s, test, &format!("{skip}\n{test}"), 1), same);
        assert_eq!(
            named(&skipped),
            ["the_polyhedral_corpus_is_the_same_on_every_target"],
            "{skip}"
        );
    }
    // The whole file compiled out: every test of it.
    let whole = planted(|s| format!("#![cfg(not(debug_assertions))]\n{s}"), same);
    assert!(named(&whole).len() > GATED.len(), "{whole:?}");
    // The pull request's leg built unoptimised, or told it is: every gated test there.
    let step = "        env:\n          RUSTFLAGS: -C overflow-checks=on\n          \
                CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS: \"true\"\n        run: cargo nextest run \
                $KERNEL_CRATES --release --locked --no-fail-fast --partition count:${{ matrix.k }}/2";
    for (from, to) in [
        (
            "        run: cargo",
            "          CARGO_PROFILE_RELEASE_OPT_LEVEL: \"0\"\n        run: cargo",
        ),
        (
            "overflow-checks=on\n",
            "overflow-checks=on --cfg unoptimised\n",
        ),
    ] {
        let skipped = planted(same, |w| plant(w, step, &plant(step, from, to, 1), 1));
        assert!(
            skipped.iter().all(|s| s.contains("job linux")),
            "{skipped:#?}"
        );
        let mut got = named(&skipped);
        got.sort();
        got.dedup();
        assert_eq!(got, GATED, "{to}");
    }
}
