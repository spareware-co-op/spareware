//! Attacks on G100 (issue #428): the facade holds every operation's schema as data, re-exports the
//! stable value types, and holds no kernel logic.
//!
//! Each test tries to break one claim: the facade's source defines no function and no
//! implementation (27: "no operation, query, dispatch, hashing, cache or trait implementation that
//! computes anything"); every item it exports is a re-export of a kernel crate it depends on, or a
//! data type of its own, and every kernel crate it depends on is re-exported; and the operation
//! list keeps its rules, shown on seeded lists that break each one.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a failure should stop loudly"
)]

use std::collections::BTreeSet;
use std::path::Path;

use engine_geometry::operations::OPERATIONS;
use engine_geometry::schema::{Class, Family, OpSchema, Operation, Tier};
use syn::visit::Visit;

const SOURCES: [(&str, &str); 4] = [
    ("lib.rs", include_str!("../src/lib.rs")),
    ("operations.rs", include_str!("../src/operations.rs")),
    ("schema.rs", include_str!("../src/schema.rs")),
    ("value.rs", include_str!("../src/value.rs")),
];
const MANIFEST: &str = include_str!("../Cargo.toml");

/// What a source file defines that could compute something, and the crates its `use`s name.
#[derive(Default)]
struct Items {
    computes: Vec<String>,
    used: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for Items {
    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        self.computes.push(format!("fn {}", f.sig.ident));
    }
    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        self.computes
            .push(format!("impl at line {}", i.impl_token.span.start().line));
    }
    fn visit_item_macro(&mut self, m: &'ast syn::ItemMacro) {
        self.computes
            .push(format!("macro {:?}", m.mac.path.get_ident()));
    }
    fn visit_item_trait(&mut self, t: &'ast syn::ItemTrait) {
        self.computes.push(format!("trait {}", t.ident));
    }
    fn visit_item_use(&mut self, u: &'ast syn::ItemUse) {
        if let syn::UseTree::Path(p) = &u.tree {
            self.used.insert(p.ident.to_string());
        }
    }
}

/// Every source file of the facade, parsed.
fn items() -> Items {
    let mut all = Items::default();
    for (name, text) in SOURCES {
        let file = syn::parse_file(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        all.visit_file(&file);
    }
    all
}

#[test]
fn the_facade_holds_no_logic() {
    let items = items();
    assert!(items.computes.is_empty(), "{:?}", items.computes);
    // Every source file is listed here, so a new file cannot hide logic.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files, SOURCES.map(|(n, _)| n.to_string()));
}

#[test]
fn every_kernel_dependency_is_re_exported_and_nothing_else_is_used() {
    let manifest: toml::Table = toml::from_str(MANIFEST).unwrap();
    let deps = manifest["dependencies"].as_table().unwrap();
    let kernel: BTreeSet<String> = deps
        .keys()
        .filter(|k| k.starts_with("geom-") && *k != "geom-pool")
        .map(|k| k.replace('-', "_"))
        .collect();
    let used: BTreeSet<String> = items().used.into_iter().filter(|u| u != "crate").collect();
    assert_eq!(
        used, kernel,
        "the facade re-exports exactly the kernel crates it depends on"
    );
}

/// The rules every list of operations keeps, or the first it breaks.
fn rules(list: &[Operation]) -> Result<(), String> {
    for pair in list.windows(2) {
        if pair[0].schema.name >= pair[1].schema.name {
            return Err(format!(
                "{} is not before {}",
                pair[0].schema.name, pair[1].schema.name
            ));
        }
    }
    for op in list {
        let name = op.schema.name;
        let valid = !name.is_empty()
            && name.len() <= 128
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'));
        if !valid {
            return Err(format!("{name:?} is not a valid MCP tool name"));
        }
        if op.schema.version == 0 {
            return Err(format!("{name}: version 0"));
        }
        if let Tier::Preview { since } = op.tier {
            let parts: Vec<&str> = since.split('-').collect();
            let date = parts.len() == 3
                && parts[0].len() == 4
                && parts[1].len() == 2
                && parts[2].len() == 2
                && parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit()))
                && (1..=12).contains(&parts[1].parse::<u32>().unwrap())
                && (1..=31).contains(&parts[2].parse::<u32>().unwrap());
            if !date {
                return Err(format!("{name}: preview since {since:?} is not a date"));
            }
        }
    }
    Ok(())
}

/// A schema named `name`, of `version`.
const fn schema(name: &'static str, version: u32) -> OpSchema {
    OpSchema {
        name,
        version,
        family: Family::Make,
        doc: "An example.",
        inputs: &[],
        outputs: &[],
        roles: &[],
        errors: &[],
        diagnostics: &[],
        classes: &[Class::Exact],
        batch: false,
        example: "",
    }
}

static BOX: OpSchema = schema("box", 1);
static CYLINDER: OpSchema = schema("cylinder", 1);
static SPACE: OpSchema = schema("a box", 1);
static ZERO: OpSchema = schema("zero", 0);

#[test]
fn the_operation_list_keeps_its_rules() {
    assert_eq!(rules(OPERATIONS), Ok(()));
    let stable = |schema| Operation {
        schema,
        tier: Tier::Stable,
    };
    let preview = |schema, since| Operation {
        schema,
        tier: Tier::Preview { since },
    };
    assert_eq!(
        rules(&[stable(&BOX), preview(&CYLINDER, "2026-10-31")]),
        Ok(())
    );
    assert!(
        rules(&[stable(&CYLINDER), stable(&BOX)]).is_err(),
        "out of order"
    );
    assert!(rules(&[stable(&BOX), stable(&BOX)]).is_err(), "twice");
    assert!(rules(&[stable(&SPACE)]).is_err(), "a space in a tool name");
    assert!(rules(&[stable(&ZERO)]).is_err(), "version 0");
    for bad in ["2026-13-01", "2026-10-32", "26-10-31", "2026-1-31", "soon"] {
        assert!(rules(&[preview(&BOX, bad)]).is_err(), "{bad}");
    }
}
