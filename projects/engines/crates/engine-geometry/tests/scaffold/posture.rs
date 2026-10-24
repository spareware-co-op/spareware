//! The merge gate's lint rules (piece G12): every crate that ships is built with chapter 21's
//! disallowed methods and types and chapter 27's denials, and nothing in its settings or around
//! them switches those off.
//!
//! What it does: `lints` holds every crate that ships to exactly `geom-num`'s settings, and leaves
//! "`geom-num`'s settings themselves ... to chapter 28.8's merge gate (G12) to check against
//! chapters 21 and 27". This module is that check. For every crate of the table whose kind ships
//! and that is in the workspace it requires, in the settings cargo and clippy read for it:
//!
//! - in its manifest's `[lints.clippy]`, `unwrap_used`, `expect_used`, `panic` and
//!   `indexing_slicing` at `deny` or `forbid` (chapter 27: "`clippy` denies `unwrap`, `expect`,
//!   `panic` and unchecked indexing in kernel code"); `todo`, `unimplemented` and `unreachable`
//!   too, the panics `clippy::panic` does not name (principle 13: "no panics in kernel paths");
//!   and `disallowed_methods` and `disallowed_types`, so chapter 21's lists are errors in every
//!   clippy run, not only in one given `-D warnings`;
//! - that nothing else in `[lints.clippy]` lowers one of those: cargo passes the table to clippy
//!   sorted by priority, a later flag overriding an earlier one, so a lint group (`restriction`,
//!   `style`, `all` and the rest) set to `allow` or `warn` at a priority not below a denial's
//!   switches the denial off, and so does the same lint spelt again with hyphens (`unwrap-used`),
//!   which rustc reads as the same name. A group may lower lints only below every denial, as
//!   `geom-num`'s `pedantic` at priority -1 does;
//! - in its clippy configuration's `disallowed-methods`, every method of `required_methods`, and in
//!   `disallowed-types`, every type of `REQUIRED_TYPES` (chapter 21's table, "Enforced by"), none
//!   of them with `allow-invalid`, which would silence the message that a path no longer resolves;
//! - and, through `cargo_configs`, that no cargo configuration file sits in the repository on the
//!   way from the workspace root to a crate of the kernel's family: cargo reads `.cargo/config.toml`
//!   from the folder it runs in and every folder above, and its `[env]` table can point clippy at
//!   another configuration (`CLIPPY_CONF_DIR`), its `[build]` table can wrap the compiler, and its
//!   `[source]` and `[patch]` tables can replace crates.io's code, all without changing a manifest
//!   or a `clippy.toml` this check reads. None exists today.
//!
//! The lists, from chapter 21's table, row by row:
//!
//! - "Maths functions (`sin`, `cos`, `atan2`, `exp`, `powi` and so on) differ by platform": every
//!   method of `f64` and `f32` the standard library computes with the platform's maths library,
//!   `PLATFORM_MATHS` (the transcendental and power functions, `cbrt` and `hypot`). Not listed,
//!   because IEEE 754 defines one correctly rounded result for each, which chapter 21's first
//!   paragraph says Rust gives on every target: `sqrt`, `mul_add`, `abs`, `copysign`, rounding,
//!   `rem_euclid` and comparisons.
//! - "`max` and `min` may return either zero sign": `f64::max`, `f64::min`, `f32::max`, `f32::min`.
//! - "Relaxed and 'algebraic' operations ... Forbidden": the `algebraic_*` methods of both widths
//!   (`ALGEBRAIC`). They are stable since Rust 1.98.0 and defined in `core`
//!   (`#[stable(feature = "float_algebraic", since = "1.98.0")]` in `library/core/src/num/f64.rs`
//!   at the 1.99.0 tag), so `no_std` kernel code on the pinned 1.99.0 can call them, as it can
//!   `max` and `min`; the platform's maths functions are `std`-only there (`core::f64::math` is
//!   unstable), so in kernel code only tests could reach them.
//! - "Parallel float reductions change order": `Iterator::sum` and `Iterator::product`, whose
//!   order of operations is not written out at the call, and the trait functions they call,
//!   `Sum::sum` and `Product::product`, which `<f64 as Sum<&f64>>::sum(it)` or `f64::sum(it)` name
//!   directly with the order just as unstated (`REDUCTIONS`).
//! - "Hash map iteration order is random": `std::collections::HashMap` and `HashSet`.
//! - "Random numbers, clocks, and process or thread identity": std's clocks, process id, running
//!   thread and randomly seeded hasher (`CLOCKS_AND_IDENTITY`, and `RandomState` and `ThreadId`
//!   among `REQUIRED_TYPES`), which the facade and `geom-pool`, the `std` crates that ship, could
//!   otherwise call.
//!
//! What it does not cover: relaxed SIMD, the rest of the fourth row, which exists only as
//! WebAssembly intrinsics behind the `relaxed-simd` target feature, which no kernel build enables;
//! the rest of the sixth row (std's unstable random module, the x86 RDRAND, RDSEED and RDTSC
//! intrinsics and the random-number crates' entry points), which the piece that builds
//! `geom_num::rng` (G32) adds with its generator; panicking indexing that no clippy lint reports:
//! slicing a string (`clippy::string_slice` would report it, but `geom-num`'s own tests slice
//! strings and no test exemption exists for that lint, so denying it waits for the piece that may
//! edit them) and indexing a map (`map[&key]`), which no clippy lint reads, so it stays a review
//! rule; and whether clippy resolves each path, which a configuration file cannot show. The gate's
//! end-to-end check (`gate`, `gate_clippy_fails_on_each_seeded_violation`) runs clippy 1.99.0 with
//! `geom-num`'s own files on a probe that calls every listed method and names every listed type
//! once, so a misspelt path fails there, and the gate fails on clippy's "does not refer to a
//! reachable" message. Lint attributes in source (`#![allow(...)]`) are `overrides`'.
//!
//! What it guarantees: every crate that ships and lacks any of these is reported, with exactly
//! what it lacks; settings the reader cannot read are reported too, never passed.
//!
//! Method: set comparison over the settings `manifest` reads with the `toml` crate, the items of
//! each disallowed list read as TOML values; cargo's order of lint flags, from `lints_to_rustflags` in
//! cargo's `src/workspace/parser/mod.rs` (MIT OR Apache-2.0, read at commit
//! `ce886c7f2994c7cdfa455786ada6d52a383bca3a`: sorted by priority, then by name in reverse) and
//! the manifest reference's `priority` ("lower ... numbers have lower priority, being overridden
//! by higher numbers"); cargo's configuration discovery
//! (<https://doc.rust-lang.org/cargo/reference/config.html#hierarchical-structure>).
//!
//! Cost: linear in the size of each crate's settings, and two file lookups per folder on the way
//! to each crate.

// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken check must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};

use crate::check::{Observed, Rule, Violation};
use crate::lints::{Lints, carries_the_posture};
use crate::manifest::{Keys, Value};
use crate::table::Table;

/// The clippy lints chapter 27 denies in kernel code, the other panic macros (principle 13), and
/// the two that make chapter 21's lists errors.
pub const DENIED_LINTS: [&str; 9] = [
    "unwrap_used",
    "expect_used",
    "panic",
    "indexing_slicing",
    "todo",
    "unimplemented",
    "unreachable",
    "disallowed_methods",
    "disallowed_types",
];

/// Clippy's lint groups (<https://doc.rust-lang.org/clippy/lints.html>). Which group holds which
/// lint can change between releases, so the rule treats each as holding every denied lint.
pub const CLIPPY_GROUPS: [&str; 10] = [
    "all",
    "cargo",
    "complexity",
    "correctness",
    "nursery",
    "pedantic",
    "perf",
    "restriction",
    "style",
    "suspicious",
];

/// The methods of `f64` and `f32` that the standard library computes with the platform's maths
/// library (chapter 21's first row).
pub const PLATFORM_MATHS: [&str; 26] = [
    "sin", "cos", "tan", "sin_cos", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh",
    "asinh", "acosh", "atanh", "exp", "exp2", "exp_m1", "ln", "log", "log2", "log10", "ln_1p",
    "powf", "powi", "cbrt", "hypot",
];

/// The relaxed ("algebraic") float operations (chapter 21's fourth row).
pub const ALGEBRAIC: [&str; 5] = [
    "algebraic_add",
    "algebraic_sub",
    "algebraic_mul",
    "algebraic_div",
    "algebraic_rem",
];

/// The reductions whose order is not written out (chapter 21's fifth row): the iterator methods
/// and the trait functions they call.
pub const REDUCTIONS: [&str; 4] = [
    "core::iter::Iterator::sum",
    "core::iter::Iterator::product",
    "core::iter::Sum::sum",
    "core::iter::Product::product",
];

/// std's clocks, process id, running thread and randomly seeded hasher (chapter 21's sixth row).
pub const CLOCKS_AND_IDENTITY: [&str; 8] = [
    "std::time::SystemTime::now",
    "std::time::SystemTime::elapsed",
    "std::time::Instant::now",
    "std::time::Instant::elapsed",
    "std::process::id",
    "std::thread::current",
    "std::thread::Thread::id",
    "std::hash::RandomState::new",
];

/// The types chapter 21 disallows: the hash maps (fifth row), std's randomly seeded hasher and the
/// running thread's identity (sixth row).
pub const REQUIRED_TYPES: [&str; 4] = [
    "std::collections::HashMap",
    "std::collections::HashSet",
    "std::hash::RandomState",
    "std::thread::ThreadId",
];

/// Every method chapter 21 disallows, as clippy paths, sorted.
pub fn required_methods() -> BTreeSet<String> {
    let mut methods: BTreeSet<String> = REDUCTIONS
        .iter()
        .chain(CLOCKS_AND_IDENTITY.iter())
        .map(|m| (*m).to_owned())
        .collect();
    for width in ["f64", "f32"] {
        for method in ["max", "min"]
            .iter()
            .chain(PLATFORM_MATHS.iter())
            .chain(ALGEBRAIC.iter())
        {
            methods.insert(format!("{width}::{method}"));
        }
    }
    methods
}

/// One entry of a disallowed list: whether it carries `allow-invalid = true`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Listed {
    /// `allow-invalid = true`: clippy says nothing when the path does not resolve.
    pub allow_invalid: bool,
}

/// The paths a disallowed list names: each item is a path string or a table with a `path` key
/// (clippy's two forms), with whether it allows an invalid path. `None` when the value cannot be
/// read so: not an array, an item of another type, a path that is not a string, or an
/// `allow-invalid` that is not a boolean.
pub fn listed_paths(value: &Value) -> Option<BTreeMap<String, Listed>> {
    let mut paths = BTreeMap::new();
    for item in value.as_array()? {
        let (path, allow_invalid) = match item {
            Value::String(path) => (path.clone(), false),
            Value::Table(table) => {
                let allow_invalid = match table.get("allow-invalid") {
                    None => false,
                    Some(flag) => flag.as_bool()?,
                };
                (table.get("path")?.as_str()?.to_owned(), allow_invalid)
            }
            _ => return None,
        };
        paths.insert(path, Listed { allow_invalid });
    }
    Some(paths)
}

/// One entry of a manifest's `[lints.clippy]`, as cargo reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    /// The key as written.
    key: String,
    /// The name rustc reads: the key with each `-` read as `_`.
    name: String,
    /// `allow`, `warn`, `deny` or `forbid`.
    level: String,
    /// The priority, 0 when not written.
    priority: i64,
}

impl Entry {
    /// Whether this entry makes its lints errors.
    fn denies(&self) -> bool {
        self.level == "deny" || self.level == "forbid"
    }

    /// The entry as a violation shows it.
    fn shown(&self) -> String {
        format!(
            "{} = {{ level = \"{}\", priority = {} }}",
            self.key, self.level, self.priority
        )
    }
}

/// The entries of `[lints.clippy]` among a manifest's lint keys, by key; `None` when one cannot be
/// read: a level that is not one of rustc's four, a priority that is not an integer, or a field
/// other than `level` and `priority`.
fn clippy_entries(tables: &Keys) -> Option<BTreeMap<String, Entry>> {
    let mut fields: BTreeMap<String, (Option<String>, Option<i64>)> = BTreeMap::new();
    for (key, value) in tables {
        let Some(rest) = key.strip_prefix("lints.clippy.") else {
            continue;
        };
        let (lint, field) = match rest.rsplit_once('.') {
            Some((lint, field @ ("level" | "priority"))) => (lint, field),
            Some(_) => return None,
            None => (rest, "level"),
        };
        let entry = fields.entry(lint.to_owned()).or_default();
        if field == "level" {
            let level = value.as_str()?.to_owned();
            if !["allow", "warn", "deny", "forbid"].contains(&level.as_str()) {
                return None;
            }
            entry.0 = Some(level);
        } else {
            entry.1 = Some(value.as_integer()?);
        }
    }
    fields
        .into_iter()
        .map(|(key, (level, priority))| {
            let entry = Entry {
                name: key.replace('-', "_"),
                level: level?,
                priority: priority.unwrap_or(0),
                key: key.clone(),
            };
            Some((key, entry))
        })
        .collect()
}

/// What lowers the denials of `[lints.clippy]`, in words, into `wrong`: a lint group set below
/// `deny` at a priority not under a denial's, and a denied lint spelt again at a lower level.
fn lowered(entries: &BTreeMap<String, Entry>, wrong: &mut Vec<String>) {
    let denials: Vec<&Entry> = DENIED_LINTS
        .iter()
        .filter_map(|lint| entries.get(*lint))
        .filter(|e| e.denies())
        .collect();
    for entry in entries.values().filter(|e| !e.denies()) {
        let group = CLIPPY_GROUPS.contains(&entry.name.as_str());
        let overridden: Vec<String> = denials
            .iter()
            .filter(|d| d.key != entry.key)
            .filter(|d| (group && entry.priority >= d.priority) || entry.name == d.name)
            .map(|d| format!("clippy::{} (priority {})", d.name, d.priority))
            .collect();
        if overridden.is_empty() {
            continue;
        }
        let why = if group {
            "a lint group cargo passes after a denial (priority not lower) overrides it, so a group \
             may lower lints only at a priority below every denial"
        } else {
            "rustc reads `-` as `_`, so this is the same lint set again, and cargo passes it after \
             or beside the denial"
        };
        wrong.push(format!(
            "its manifest sets {}, which lowers {} (chapters 21 and 27): {why}",
            entry.shown(),
            overridden.join(", ")
        ));
    }
}

/// What one crate's lint settings lack, in words; empty when they hold everything.
pub fn lacks(lints: &Lints) -> Vec<String> {
    let mut wrong = Vec::new();
    match lints.tables.as_ref().map(|t| (t, clippy_entries(t))) {
        None => wrong.push("its manifest's lint tables cannot be read".to_owned()),
        Some((_, None)) => wrong.push(
            "its manifest's [lints.clippy] cannot be read: a level that is not allow, warn, deny \
             or forbid, a priority that is not an integer, or another field"
                .to_owned(),
        ),
        Some((_, Some(entries))) => {
            let missing: Vec<&str> = DENIED_LINTS
                .iter()
                .copied()
                .filter(|lint| !entries.get(*lint).is_some_and(Entry::denies))
                .collect();
            if !missing.is_empty() {
                wrong.push(format!(
                    "its manifest does not deny clippy::{} (chapters 21 and 27; principle 13)",
                    missing.join(", clippy::")
                ));
            }
            lowered(&entries, &mut wrong);
        }
    }
    let settings = lints.clippy.as_ref().and_then(|c| c.settings.as_ref());
    let Some(settings) = settings else {
        wrong.push("clippy reads no configuration for it that this check can read".to_owned());
        return wrong;
    };
    for (key, required) in [
        ("disallowed-methods", required_methods()),
        (
            "disallowed-types",
            REQUIRED_TYPES.iter().map(|t| (*t).to_owned()).collect(),
        ),
    ] {
        match settings.get(key).map(listed_paths) {
            None => wrong.push(format!("its clippy configuration has no {key}")),
            Some(None) => wrong.push(format!("its clippy configuration's {key} cannot be read")),
            Some(Some(listed)) => {
                let missing: Vec<&str> = required
                    .iter()
                    .filter(|path| !listed.contains_key(*path))
                    .map(String::as_str)
                    .collect();
                if !missing.is_empty() {
                    wrong.push(format!(
                        "its clippy configuration's {key} lacks {} (chapter 21)",
                        missing.join(", ")
                    ));
                }
                let silenced: Vec<&str> = required
                    .iter()
                    .filter(|path| listed.get(*path).is_some_and(|l| l.allow_invalid))
                    .map(String::as_str)
                    .collect();
                if !silenced.is_empty() {
                    wrong.push(format!(
                        "its clippy configuration's {key} lists {} with allow-invalid = true, so \
                         the ban could lapse in silence (chapter 21)",
                        silenced.join(", ")
                    ));
                }
            }
        }
    }
    wrong
}

/// Checks every crate of the table that ships and is in the workspace.
pub fn check(table: &Table<'_>, observed: &Observed) -> Vec<Violation> {
    let mut found = BTreeSet::new();
    for row in table
        .rows
        .iter()
        .filter(|row| carries_the_posture(row.kind))
    {
        let Some(package) = observed.packages.get(row.name) else {
            continue;
        };
        for detail in lacks(&package.lints) {
            found.insert(Violation {
                krate: row.name.to_owned(),
                rule: Rule::PostureLacks,
                detail,
            });
        }
    }
    found.into_iter().collect()
}

/// The two names cargo gives a configuration file inside a `.cargo` folder, the second the older.
pub const CARGO_CONFIG_FILES: [&str; 2] = ["config.toml", "config"];

/// Reports every cargo configuration file found on the way to a crate of the kernel's family
/// (each path relative to the workspace root, one component per entry).
pub fn cargo_configs(found: &BTreeSet<Vec<String>>) -> Vec<Violation> {
    found
        .iter()
        .map(|file| Violation {
            krate: file.join("/"),
            rule: Rule::CargoConfig,
            detail: "cargo reads this file whenever it runs in or below its folder, which holds \
                     crates of the kernel's family; its [env] can point clippy at another \
                     configuration (CLIPPY_CONF_DIR), its [build] can wrap or replace the \
                     compiler and its [source] and [patch] can replace crates.io's code, none of \
                     which a manifest or clippy.toml shows. The kernel is built with what its \
                     manifests and clippy.toml files say: remove it, or move what it sets into \
                     them"
                .to_owned(),
        })
        .collect()
}
