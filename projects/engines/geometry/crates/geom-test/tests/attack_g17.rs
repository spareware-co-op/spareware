//! Attacks on piece G17: the kernel's README.md, AGENTS.md and GLOSSARY.md, first edition
//! (`projects/engines/geometry/`, chapter 26.3 of the kernel architecture). Each test tries to
//! break a claim the three files make, so that a file moved, a list renamed or a step added
//! without its playbook fails CI instead of misleading the next person or agent who follows them.
//!
//! The claims attacked:
//!
//! - **Every link leads somewhere.** Every relative link in the three files names a file or folder
//!   that exists, and every `#anchor` names a heading of the file it points into, by GitHub's rule
//!   for heading anchors.
//! - **Every path in code is real.** Every repository path the three files write in backticks
//!   exists, read by the rule the files state: from the repository root when it starts with
//!   `projects/` or `.github/`, and from the kernel's folder otherwise.
//! - **The playbooks name what is there.** Each list, constant, field, function, test and rule
//!   that AGENTS.md tells a reader to edit, copy or run is named in AGENTS.md and exists in the
//!   file it lives in; the sentence a predicate's `serves` completes is the generator's own; and
//!   `geom-gen`'s predicates file names and counts every predicate generator in its header, as
//!   AGENTS.md tells the next predicate's author to keep it.
//! - **The checklist is the thirteen principles**, in ARCHITECTURE.md's order and words.
//! - **The workflows are all named.** README.md and AGENTS.md name exactly the kernel's
//!   workflows in `.github/workflows/`, and README.md counts them.
//! - **The glossary defines each term once, in order**: alphabetical, ignoring case and
//!   punctuation, each under its letter, no term or alias defined twice, and every example
//!   chapter 26.3 gives for it defined.
//! - **The randomised-method rule has one home.** `geom-num`'s `tests/rng_ban.rs` reads rule 1 in
//!   AGENTS.md; this file checks that AGENTS.md's section 8 holds all six rules and that
//!   `geom_num::rng`'s header points there and no longer states them.
//!
//! The files are read as GitHub renders them, `CommonMark` with GitHub's tables, by `pulldown-cmark`.
//! The tests read the repository's files, so they run natively only, as `tests/map_repo.rs` does.

// The crate denies native arithmetic in kernel code (Cargo.toml); the reader's counters may use it.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "the reader's counters; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken claim must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![cfg(not(target_family = "wasm"))]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use geom_test::map::repo;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// The kernel's folder, from the repository root.
const KERNEL: &str = "projects/engines/geometry";

/// The three files piece G17 wrote, in the kernel's folder.
const DOCS: [&str; 3] = ["README.md", "AGENTS.md", "GLOSSARY.md"];

/// The extensions of the files the three documents name in backticks. A backticked word with a
/// `/` and one of these, or ending in `/`, is a repository path.
const PATH_EXTENSIONS: [&str; 9] = [
    "json", "lock", "md", "mjs", "rs", "toml", "txt", "yaml", "yml",
];

/// The paths the three documents name before a board piece creates them: the document, the path
/// as written, and the piece. Each must still be missing; once it exists, its row goes.
const PENDING_PATHS: [(&str, &str, &str); 0] = [];

/// Each thing AGENTS.md tells a reader to edit, copy or run: as AGENTS.md writes it, the file it
/// lives in (written from the kernel's folder), and the text that declares it there.
const PLAYBOOK: [(&str, &str, &str); 49] = [
    // Section 2, the checklist.
    (
        "`geom_num::Sign`",
        "crates/geom-num/src/lib.rs",
        "pub use sign::Sign;",
    ),
    (
        "`geom_num::interval`",
        "crates/geom-num/src/lib.rs",
        "pub mod interval;",
    ),
    (
        "`geom_num::math::unproven`",
        "crates/geom-num/src/math.rs",
        "pub mod unproven",
    ),
    (
        "`NumError::UnprovenMaths`",
        "crates/geom-num/src/error.rs",
        "UnprovenMaths,",
    ),
    (
        "`NumError::ToleranceExceeded`",
        "crates/geom-num/src/error.rs",
        "ToleranceExceeded,",
    ),
    (
        "`geom_num::Exactness`",
        "crates/geom-num/src/lib.rs",
        "pub use exactness::Exactness;",
    ),
    (
        "`ExactSum`",
        "crates/geom-num/src/exact_sum.rs",
        "pub struct ExactSum",
    ),
    (
        "`geom_num::ladder`",
        "crates/geom-num/src/lib.rs",
        "pub mod ladder;",
    ),
    (
        "`ChunkPlan`",
        "crates/geom-run/src/plan.rs",
        "pub struct ChunkPlan",
    ),
    (
        "`globals` rule",
        "../crates/engine-geometry/tests/scaffold/globals.rs",
        "GlobalState",
    ),
    (
        "`overrides` rule",
        "../crates/engine-geometry/tests/scaffold/overrides.rs",
        "overrides",
    ),
    (
        "`crates/geom-num/tests/determinism.rs`",
        "crates/geom-num/tests/determinism.rs",
        "SHA-256",
    ),
    (
        "`crates/geom-num/tests/rng_determinism.rs`",
        "crates/geom-num/tests/rng_determinism.rs",
        "SHA-256",
    ),
    // Section 3, the gates.
    (
        "`gate_deny`",
        "../crates/engine-geometry/tests/scaffold/deny.rs",
        "fn gate_deny",
    ),
    (
        "`gate_clippy`",
        "../crates/engine-geometry/tests/scaffold/gate.rs",
        "fn gate_clippy",
    ),
    (
        "`gate_build`",
        "../crates/engine-geometry/tests/scaffold/gate.rs",
        "fn gate_build",
    ),
    // Section 4, dependencies.
    (
        "`deny-no-std.toml`'s `allow`",
        "deny-no-std.toml",
        "\nallow = [\n  { crate = \"libm@",
    ),
    (
        "`registry` list",
        "../crates/engine-geometry/tests/scaffold/table.rs",
        "pub registry:",
    ),
    (
        "`LICENCE_CHOICES`",
        "../crates/engine-geometry/tests/scaffold/licence.rs",
        "pub const LICENCE_CHOICES",
    ),
    // Section 5, a module.
    (
        "`GEOM_NUM`",
        "crates/geom-bench/tests/coverage.rs",
        "const GEOM_NUM",
    ),
    (
        "`geom_num::math::fma`",
        "crates/geom-num/src/math.rs",
        "pub fn fma(",
    ),
    (
        "`geom_num::math::sqrt`",
        "crates/geom-num/src/math.rs",
        "pub fn sqrt(",
    ),
    (
        "`geom_test::seed`",
        "crates/geom-test/src/lib.rs",
        "pub mod seed;",
    ),
    (
        "`geom_bench::catalogue::UNMEASURED`",
        "crates/geom-bench/src/catalogue.rs",
        "pub static UNMEASURED",
    ),
    (
        "\"Changing the benchmarks\"",
        "crates/geom-bench/src/lib.rs",
        "Changing the benchmarks",
    ),
    // Section 6, a predicate.
    (
        "`ORIENT2D`",
        "crates/geom-gen/src/predicates.rs",
        "pub const ORIENT2D",
    ),
    (
        "`PREDICATES`",
        "crates/geom-gen/src/predicates.rs",
        "pub const PREDICATES",
    ),
    (
        "`generator`",
        "crates/geom-gen/src/predicates.rs",
        "pub generator: &'static str",
    ),
    (
        "`constant`",
        "crates/geom-gen/src/predicates.rs",
        "pub constant: &'static str",
    ),
    (
        "`text`",
        "crates/geom-gen/src/predicates.rs",
        "pub text: &'static str",
    ),
    (
        "`output`",
        "crates/geom-gen/src/predicates.rs",
        "pub output: &'static str",
    ),
    (
        "`Comparison`",
        "crates/geom-gen/src/predicate_emit.rs",
        "pub struct Comparison",
    ),
    (
        "`name`",
        "crates/geom-gen/src/predicate_emit.rs",
        "pub name: &'static str",
    ),
    (
        "`serves`",
        "crates/geom-gen/src/predicate_emit.rs",
        "pub serves: &'static str",
    ),
    (
        "`shewchuk`",
        "crates/geom-gen/src/predicate_emit.rs",
        "pub shewchuk: &'static str",
    ),
    (
        "`fpg`",
        "crates/geom-gen/src/predicate_emit.rs",
        "pub fpg: &'static str",
    ),
    (
        "`generate_orient2d`",
        "crates/geom-gen/src/predicates.rs",
        "pub fn generate_orient2d",
    ),
    (
        "`COMMITTED` list of that file's tests",
        "crates/geom-gen/src/predicates.rs",
        "const COMMITTED",
    ),
    (
        "`GENERATORS`",
        "crates/geom-gen/src/registry.rs",
        "pub const GENERATORS",
    ),
    (
        "`COMMITTED` in `crates/geom-gen/tests/predicate_checker.rs`",
        "crates/geom-gen/tests/predicate_checker.rs",
        "const COMMITTED",
    ),
    (
        "`geom_num::predicate::orient2d`",
        "crates/geom-num/src/predicate.rs",
        "pub fn orient2d(",
    ),
    (
        "`generated_predicates.rs`",
        "crates/geom-num/tests/generated_predicates.rs",
        "#![allow(",
    ),
    (
        "`crates/geom-num/tests/attack_g28.rs`",
        "crates/geom-num/tests/attack_g28.rs",
        "robust::orient2d",
    ),
    (
        "`GENERATED_PARTS`",
        "crates/geom-bench/tests/coverage.rs",
        "const GENERATED_PARTS",
    ),
    (
        "(module `generated`)",
        "crates/geom-bench/src/kernels.rs",
        "pub mod generated",
    ),
    (
        "`Op`",
        "crates/geom-bench/src/catalogue.rs",
        "pub struct Op",
    ),
    (
        "`#[bench::...]`",
        "crates/geom-bench/benches/instructions/gate.rs",
        "#[bench::",
    ),
    // Section 7, a crate.
    (
        "(`present.len()`)",
        "../crates/engine-geometry/tests/attack_g09.rs",
        "present.len()",
    ),
    // Section 10, when something does not fit.
    (
        "`header_gaps` and `pending` lists",
        "../crates/engine-geometry/tests/scaffold/table.rs",
        "pub header_gaps:",
    ),
];

/// The repository root.
fn root() -> PathBuf {
    repo::workspace_root().unwrap()
}

/// The text of a file, with Windows line ends made `\n`.
fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
        .replace("\r\n", "\n")
}

/// A file of the kernel's folder, by its path from there.
fn kernel_file(relative: &str) -> String {
    read(&root().join(KERNEL).join(relative))
}

/// One heading: its level and its text, markup removed.
struct Heading {
    level: HeadingLevel,
    text: String,
}

/// One paragraph, or one list item of a list whose items are not paragraphs.
struct Block {
    /// The last level-2 heading above it.
    section: String,
    /// The text of the strong emphasis it opens with, if it opens with one.
    lead: Option<String>,
    /// Its text, markup removed: a code span's text without its backticks.
    text: String,
}

/// What a Markdown file holds, as GitHub reads it.
#[derive(Default)]
struct Markdown {
    /// Every link's destination, in order.
    links: Vec<String>,
    /// Every code span's text, in order.
    code: Vec<String>,
    /// Every heading, in order.
    headings: Vec<Heading>,
    /// Every paragraph and every item of a tight list, in order.
    blocks: Vec<Block>,
}

/// Reads Markdown as GitHub renders it: `CommonMark`, with GitHub's tables.
fn markdown(text: &str) -> Markdown {
    let mut out = Markdown::default();
    let mut section = String::new();
    let mut heading: Option<String> = None;
    // The block being read, and how many elements deep inside it the parser is.
    let mut block: Option<Block> = None;
    let mut depth = 0_usize;
    let mut in_lead = false;
    for event in Parser::new_ext(text, Options::ENABLE_TABLES) {
        let words = match event {
            Event::Start(tag) => {
                match &tag {
                    Tag::Heading { .. } => heading = Some(String::new()),
                    Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                        out.links.push(dest_url.to_string());
                    }
                    _ => {}
                }
                if let Some(b) = &mut block {
                    depth += 1;
                    if matches!(tag, Tag::Strong) && b.lead.is_none() && b.text.trim().is_empty() {
                        b.lead = Some(String::new());
                        in_lead = true;
                    }
                } else if matches!(tag, Tag::Paragraph | Tag::Item) {
                    block = Some(Block {
                        section: section.clone(),
                        lead: None,
                        text: String::new(),
                    });
                    depth = 1;
                }
                None
            }
            Event::End(end) => {
                if block.is_some() {
                    depth -= 1;
                    if matches!(end, TagEnd::Strong) {
                        in_lead = false;
                    }
                    if depth == 0 {
                        out.blocks.push(block.take().unwrap());
                    }
                }
                if let TagEnd::Heading(level) = end {
                    let text = heading.take().unwrap().trim().to_owned();
                    if level == HeadingLevel::H2 {
                        section.clone_from(&text);
                    }
                    out.headings.push(Heading { level, text });
                }
                None
            }
            Event::Text(t) => Some(t.to_string()),
            Event::Code(c) => {
                out.code.push(c.to_string());
                Some(c.to_string())
            }
            Event::SoftBreak | Event::HardBreak => Some(" ".to_owned()),
            _ => None,
        };
        if let Some(words) = words {
            if let Some(h) = &mut heading {
                h.push_str(&words);
            }
            if let Some(b) = &mut block {
                b.text.push_str(&words);
                if in_lead {
                    b.lead.as_mut().unwrap().push_str(&words);
                }
            }
        }
    }
    out
}

/// The anchors GitHub gives a file's headings, by the rules it documents for section links
/// (<https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#section-links>):
/// markup removed, leading and trailing spaces removed, letters made lower case, each space made a
/// hyphen, and every other character that is not a letter, a number, a hyphen or an underscore
/// removed; an anchor equal to an earlier one gets `-1`, `-2` and so on. pulldown-cmark makes no
/// anchors, and the rules are four, so they are written out here.
fn anchors(headings: &[Heading]) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    for h in headings {
        let base: String = h
            .text
            .trim()
            .chars()
            .flat_map(char::to_lowercase)
            .filter_map(|c| match c {
                ' ' => Some('-'),
                '-' | '_' => Some(c),
                c if c.is_alphanumeric() => Some(c),
                _ => None,
            })
            .collect();
        let mut anchor = base.clone();
        let mut n = 0_u32;
        while seen.contains(&anchor) {
            n += 1;
            anchor = format!("{base}-{n}");
        }
        seen.insert(anchor);
    }
    seen
}

/// Whether a code span is a repository path: no spaces or placeholders, a `/`, and a file
/// extension the documents name or a closing `/`.
fn is_repository_path(span: &str) -> bool {
    let plain = !span.contains(|c: char| c.is_whitespace() || "<>*${}:=|,;()[]#\"'`".contains(c));
    let shaped = span.ends_with('/')
        || Path::new(span)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| PATH_EXTENSIONS.contains(&e));
    plain && shaped && span.contains('/') && !span.starts_with(['/', '-'])
}

/// Where a repository path in backticks points, by the rule the three files state.
fn resolve_code_path(root: &Path, span: &str) -> PathBuf {
    if span.starts_with("projects/") || span.starts_with(".github/") {
        root.join(span)
    } else {
        root.join(KERNEL).join(span)
    }
}

/// Whether a file name is a workflow's: GitHub reads workflows from `.yml` and `.yaml` files.
fn is_workflow(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("yml") || e.eq_ignore_ascii_case("yaml"))
}

/// English number words, for the counts the files write out.
fn number_word(n: usize) -> String {
    const WORDS: [&str; 21] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ];
    // Past twenty, a ten's word and a unit's, joined by a hyphen, as the headers write them.
    const TENS: [&str; 10] = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    match (WORDS.get(n), TENS.get(n / 10)) {
        (Some(word), _) => (*word).to_owned(),
        (None, Some(tens)) if n % 10 == 0 => (*tens).to_owned(),
        (None, Some(tens)) => format!("{tens}-{}", WORDS[n % 10]),
        (None, None) => "many".to_owned(),
    }
}

#[test]
fn every_link_leads_to_a_file_and_every_anchor_to_a_heading() {
    let root = root();
    let mut problems = Vec::new();
    let mut checked = 0_usize;
    for doc in DOCS {
        let path = root.join(KERNEL).join(doc);
        let parsed = markdown(&read(&path));
        for link in &parsed.links {
            if link.contains("://") || link.starts_with("mailto:") {
                continue;
            }
            checked += 1;
            let (file, anchor) = link.split_once('#').unwrap_or((link.as_str(), ""));
            if file.starts_with('/') {
                problems.push(format!(
                    "{doc}: {link} is absolute; write it from the file's folder"
                ));
                continue;
            }
            let target = if file.is_empty() {
                path.clone()
            } else {
                root.join(KERNEL).join(file)
            };
            if !target.exists() {
                problems.push(format!(
                    "{doc}: {link} names {}, which does not exist",
                    target.display()
                ));
                continue;
            }
            if anchor.is_empty() {
                continue;
            }
            if target.extension().is_none_or(|e| e != "md") || !target.is_file() {
                problems.push(format!(
                    "{doc}: {link} has an anchor into a file that is not Markdown"
                ));
                continue;
            }
            let headings = markdown(&read(&target)).headings;
            if !anchors(&headings).contains(anchor) {
                problems.push(format!(
                    "{doc}: {link}: no heading of {file} has the anchor #{anchor}"
                ));
            }
        }
    }
    println!("{checked} relative links checked");
    assert!(checked > 50, "too few links found: the reader is broken");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_path_in_backticks_exists() {
    let root = root();
    let mut problems = Vec::new();
    let mut checked = 0_usize;
    for doc in DOCS {
        let parsed = markdown(&kernel_file(doc));
        for span in parsed.code.iter().filter(|s| is_repository_path(s)) {
            checked += 1;
            let target = resolve_code_path(&root, span);
            let pending = PENDING_PATHS
                .iter()
                .find(|(d, path, _)| *d == doc && path == span);
            match (target.exists(), pending) {
                (true, Some((_, _, piece))) => problems.push(format!(
                    "{doc}: `{span}` exists now; remove its row, waiting for {piece}, from PENDING_PATHS"
                )),
                (false, None) => problems.push(format!(
                    "{doc}: `{span}` names {}, which does not exist",
                    target.display()
                )),
                _ => {}
            }
        }
    }
    println!("{checked} paths in backticks checked");
    assert!(checked > 40, "too few paths found: the reader is broken");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The reader itself, on text whose answers are known, so the tests above cannot pass by reading
/// nothing.
#[test]
fn the_reader_finds_links_code_headings_and_leads() {
    let text = "# Top\n\n## A b\n\n**Lead** (also: `x_y`): text [a](b.md#c) and `d/e.rs`.\n\n\
                - **One.** first\n- two\n\n| `k.yml` | [t](u/) |\n| --- | --- |\n| 1 | 2 |\n\n\
                ## A b\n\n```\n`not/code.rs`\n```\n";
    let m = markdown(text);
    assert_eq!(m.links, ["b.md#c", "u/"]);
    assert_eq!(m.code, ["x_y", "d/e.rs", "k.yml"]);
    let texts: Vec<&str> = m.headings.iter().map(|h| h.text.as_str()).collect();
    assert_eq!(texts, ["Top", "A b", "A b"]);
    let a = anchors(&m.headings);
    assert!(a.contains("top") && a.contains("a-b") && a.contains("a-b-1"));
    let leads: Vec<Option<&str>> = m.blocks.iter().map(|b| b.lead.as_deref()).collect();
    assert_eq!(leads, [Some("Lead"), Some("One."), None]);
    assert_eq!(m.blocks[0].section, "A b");
    assert_eq!(m.blocks[0].text, "Lead (also: x_y): text a and d/e.rs.");
    let h = |text: &str| Heading {
        level: HeadingLevel::H2,
        text: text.to_owned(),
    };
    let a = anchors(&[
        h("8. The randomised-method rule"),
        h("`geom_num::rng` (G32)"),
    ]);
    assert!(a.contains("8-the-randomised-method-rule"), "{a:?}");
    assert!(a.contains("geom_numrng-g32"), "{a:?}");
    assert!(is_repository_path("crates/geom-num/src/rng.rs"));
    assert!(is_repository_path("crates/"));
    assert!(is_repository_path(".github/workflows/"));
    assert!(!is_repository_path("src/<name>.rs"));
    assert!(!is_repository_path("geom_num::rng"));
    assert!(!is_repository_path("MIT/Apache-2.0"));
    assert!(!is_repository_path("doi:10.1145/2063384.2063405"));
    // The glossary's order: word by word, case, accents and punctuation set aside.
    assert!(sort_key("Batch runner") < sort_key("`BatchBudget`"));
    assert!(sort_key("Bernstein bounds") < sort_key("Bézier extraction"));
    assert!(sort_key("Bézier extraction") < sort_key("Big-endian"));
    assert!(sort_key("Filter-bound certificate") < sort_key("Filter boundary"));
    assert!(sort_key("`I128`") < sort_key("`i64`"));
    assert_eq!(sort_key("`no_std`"), ["no", "std"]);
}

#[test]
fn the_playbooks_name_what_exists() {
    let agents = kernel_file("AGENTS.md");
    let mut problems = Vec::new();
    for (named, file, declared) in PLAYBOOK {
        if !agents.contains(named) {
            problems.push(format!("AGENTS.md no longer says {named}; update PLAYBOOK"));
        }
        if !kernel_file(file).contains(declared) {
            problems.push(format!(
                "AGENTS.md names {named}, but {file} no longer holds {declared:?}"
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// AGENTS.md tells a predicate's author that `serves` completes a sentence of the generated
/// header; that sentence must be the generator's own.
#[test]
fn the_serves_sentence_is_the_generators() {
    let sentence = "The `f64` filter and the exact path of {serves}.";
    assert!(kernel_file("AGENTS.md").contains(&format!("\"{sentence}\"")));
    assert!(
        kernel_file("crates/geom-gen/src/predicate_emit.rs").contains(&format!("\"{sentence}")),
        "the emitter no longer writes {sentence:?}; update AGENTS.md section 6"
    );
}

/// AGENTS.md tells a predicate's author to name the new generator in `predicates.rs`'s header and
/// to update the count in its `Cost` line; every predicate there now must already be named and
/// counted.
#[test]
fn the_predicates_header_names_and_counts_every_generator() {
    let source = kernel_file("crates/geom-gen/src/predicates.rs");
    let header: Vec<&str> = source
        .lines()
        .take_while(|l| l.starts_with("//!"))
        .map(|l| l.trim_start_matches("//!").trim())
        .collect();
    let first = header.split(|l| l.is_empty()).next().unwrap().join(" ");
    let cost = header
        .split(|l| l.is_empty())
        .find(|p| p.first().is_some_and(|l| l.starts_with("Cost:")))
        .expect("predicates.rs's header has no Cost paragraph")
        .join(" ");
    let generators: Vec<&str> = source
        .lines()
        .filter_map(|l| l.trim().strip_prefix("generator: \""))
        .filter_map(|l| l.strip_suffix("\","))
        .collect();
    assert!(
        generators.len() >= 5,
        "too few predicates found: {generators:?}"
    );
    for g in &generators {
        assert!(
            first.contains(&format!("`{g}`")),
            "predicates.rs's header does not name `{g}`: {first}"
        );
    }
    let count = number_word(generators.len());
    assert!(
        cost.contains(&format!(" {count} ")),
        "predicates.rs's Cost line does not count {count} predicates: {cost}"
    );
}

#[test]
fn the_checklist_is_the_thirteen_principles() {
    let architecture = markdown(&kernel_file("ARCHITECTURE.md"));
    let principles: Vec<String> = architecture
        .blocks
        .iter()
        .filter(|b| b.section == "Principles")
        .filter_map(|b| b.lead.as_deref())
        .map(|l| l.trim_end_matches('.').to_owned())
        .collect();
    assert_eq!(
        principles.len(),
        13,
        "ARCHITECTURE.md's principles: {principles:?}"
    );
    let agents = markdown(&kernel_file("AGENTS.md"));
    let start = agents
        .headings
        .iter()
        .position(|h| h.level == HeadingLevel::H2 && h.text == "2. The pull-request checklist")
        .expect("AGENTS.md has no section 2, the checklist");
    let items: Vec<&str> = agents.headings[start + 1..]
        .iter()
        .take_while(|h| h.level == HeadingLevel::H3)
        .map(|h| h.text.as_str())
        .collect();
    let expected: Vec<String> = principles
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{}. {p}", i + 1))
        .chain(["And the process".to_owned()])
        .collect();
    assert_eq!(items, expected);
    // Each principle's subsection holds at least one item to tick.
    let section: Vec<&Block> = agents
        .blocks
        .iter()
        .filter(|b| b.section == "2. The pull-request checklist")
        .collect();
    assert!(
        section.len() > 30,
        "too few checklist items: {}",
        section.len()
    );
}

#[test]
fn the_workflows_are_all_named_and_counted() {
    let folder = root().join(".github/workflows");
    let workflows: BTreeSet<String> = fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("kernel") && is_workflow(n))
        .collect();
    assert!(workflows.len() >= 5, "{workflows:?}");
    for doc in ["README.md", "AGENTS.md"] {
        let named: BTreeSet<String> = markdown(&kernel_file(doc))
            .code
            .into_iter()
            .filter(|c| is_workflow(c))
            .map(|c| c.trim_start_matches(".github/workflows/").to_owned())
            .collect();
        assert_eq!(named, workflows, "{doc} names these workflows");
    }
    let mut count = number_word(workflows.len());
    count[..1].make_ascii_uppercase();
    assert!(
        kernel_file("README.md").contains(&format!("{count} workflows in `.github/workflows/`")),
        "README.md does not say {count} workflows"
    );
}

/// A glossary headword's sorting key, by the order GLOSSARY.md states: word by word, ignoring
/// case, accents and punctuation. Any character that is not a letter or a digit ends a word, so
/// "Batch runner" (the words `batch`, `runner`) comes before "`BatchBudget`" (`batchbudget`), and
/// an accent is set aside by Unicode's canonical decomposition, so "Bézier" sorts as "Bezier".
fn sort_key(term: &str) -> Vec<String> {
    let bare: String = term.nfd().filter(|c| !is_combining_mark(*c)).collect();
    bare.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// One glossary entry.
struct Entry {
    /// The letter heading it is under.
    letter: String,
    /// Its headword.
    head: String,
    /// The sense it defines, written in brackets after the headword, as in **Class** (exactness).
    sense: Option<String>,
    /// The other names it goes by, written "(also: ...)" after the headword.
    also: Vec<String>,
}

/// The glossary's entries, in order. Every paragraph under a letter heading is one, written
/// `**Term**: definition`, `**Term** (also: name, name): definition` or
/// `**Term** (sense): definition`.
fn glossary() -> Vec<Entry> {
    let parsed = markdown(&kernel_file("GLOSSARY.md"));
    parsed
        .blocks
        .iter()
        .filter(|b| b.section.chars().count() == 1)
        .map(|b| {
            let head = b.lead.clone().unwrap_or_else(|| {
                panic!("a paragraph under {} defines no term: {}", b.section, b.text)
            });
            let rest = b.text[head.len()..].trim_start();
            let bracket = if rest.starts_with(':') {
                None
            } else {
                let inner = rest
                    .strip_prefix('(')
                    .and_then(|r| r.split_once("):"))
                    .map(|(inner, _)| inner);
                Some(inner.unwrap_or_else(|| {
                    panic!(
                        "{head}: an entry is **Term**:, **Term** (also: ...): or **Term** (sense):                          then its definition"
                    )
                }))
            };
            let (sense, also) = match bracket.map(|b| (b, b.strip_prefix("also: "))) {
                None => (None, Vec::new()),
                Some((_, Some(names))) => (None, names.split(", ").map(str::to_owned).collect()),
                Some((sense, None)) => (Some(sense.to_owned()), Vec::new()),
            };
            Entry {
                letter: b.section.clone(),
                head,
                sense,
                also,
            }
        })
        .collect()
}

#[test]
fn the_glossary_defines_each_term_once_in_order() {
    let entries = glossary();
    assert!(entries.len() > 300, "too few entries: {}", entries.len());
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    let mut previous: Option<&Entry> = None;
    for e in &entries {
        let key = sort_key(&e.head);
        if !key
            .first()
            .is_some_and(|w| w.starts_with(&e.letter.to_lowercase()))
        {
            problems.push(format!("{} is under {}", e.head, e.letter));
        }
        if let Some(p) = previous {
            let earlier = (sort_key(&p.head), p.sense.as_deref().map(sort_key));
            if earlier >= (key, e.sense.as_deref().map(sort_key)) {
                problems.push(format!("{} comes after {}", e.head, p.head));
            }
        }
        previous = Some(e);
        // A name belongs to one entry. Within an entry a name may repeat in another case or as
        // code (**Budget** (also: `Budget`)); across entries even case tells names apart
        // (`I128` and `i128`).
        let headword = match &e.sense {
            Some(sense) => format!("{} ({sense})", e.head),
            None => e.head.clone(),
        };
        let names: BTreeSet<&String> = std::iter::once(&headword).chain(&e.also).collect();
        for name in names {
            if !seen.insert(name.clone()) {
                problems.push(format!("{name} is defined by two entries"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{}",
        problems.join(
            "
"
        )
    );
}

/// Chapter 26.3's row for the glossary lists examples of the terms it defines; each is a headword
/// or a name an entry also goes by.
#[test]
fn the_glossary_defines_chapter_26_3s_examples() {
    let chapter = kernel_file("architecture/26-how-it-grows.md");
    let row = chapter
        .lines()
        .find(|l| l.starts_with("| `geometry/GLOSSARY.md` |"))
        .expect("chapter 26.3 has no row for GLOSSARY.md");
    let examples: Vec<&str> = row
        .split_once("plain words: ")
        .expect("chapter 26.3's glossary row lists no examples")
        .1
        .trim_end_matches(['|', ' '])
        .split(", ")
        .collect();
    assert!(examples.len() >= 8, "{examples:?}");
    let names: BTreeSet<String> = glossary()
        .into_iter()
        .flat_map(|e| std::iter::once(e.head).chain(e.also))
        .map(|n| n.to_lowercase())
        .collect();
    for example in examples {
        assert!(
            names.contains(&example.to_lowercase()),
            "GLOSSARY.md does not define {example}, which chapter 26.3 names"
        );
    }
}

#[test]
fn the_randomised_method_rule_lives_in_agents_md() {
    let agents = markdown(&kernel_file("AGENTS.md"));
    let leads: Vec<&str> = agents
        .blocks
        .iter()
        .filter(|b| b.section == "8. The randomised-method rule")
        .filter_map(|b| b.lead.as_deref())
        .collect();
    for rule in [
        "One generator.",
        "The seed is an input.",
        "One stream per consumer, numbered by position.",
        "Honest results.",
        "Tests are separate.",
    ] {
        assert!(
            leads.contains(&rule),
            "AGENTS.md section 8 lacks {rule:?}: {leads:?}"
        );
    }
    assert!(
        leads
            .iter()
            .any(|l| l.starts_with("The draws are part of the version")),
        "AGENTS.md section 8 lacks rule 4: {leads:?}"
    );
    let rng = kernel_file("crates/geom-num/src/rng.rs");
    assert!(rng.contains("`projects/engines/geometry/AGENTS.md`, section 8"));
    for stated in [
        "1. **One generator.**",
        "2. **The seed is an input.**",
        "6. **Tests are separate.**",
    ] {
        assert!(
            !rng.contains(stated),
            "rng.rs states the rule again: {stated}"
        );
    }
}
