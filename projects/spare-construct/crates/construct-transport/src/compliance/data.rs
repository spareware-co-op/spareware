//! The rule tables the compliance checks read: thresholds, source texts, disclaimers, the regulatory checklist, the standards
//! register and the speed, parking and ANOP tables. They are the prototype's own tables, written out of the scripts by
//! tools/transport-truth/gen-compliance.mjs (`data`) into src/compliance/data and embedded here, so every wording is exactly the
//! prototype's. A regular expression in a table is `{ "$re": source, "$flags": flags }`; an infinite number is the text "Infinity".

use super::jv::{Jx, V};
use super::rx::{Regex, rx};
use std::rc::Rc;
use std::sync::OnceLock;

static CORE: OnceLock<V> = OnceLock::new();
static EXTRA: OnceLock<V> = OnceLock::new();
static MODEL: OnceLock<V> = OnceLock::new();

/// The tables of packs/nz/compliance.js (SRC, REGIME, PATHWAYS, E27, ACCESS, SH, CLOCKS, DISCLAIMERS, STANDARDS, CHECKLIST ...).
pub fn core() -> &'static V {
    CORE.get_or_init(|| V::parse(include_str!("data/compliance.json")).expect("compliance.json"))
}

/// The tables of packs/nz/compliance-extra.js (STD, SRC, D, ASSUME, LINT, PRECINCTS, PARK_ACT, PREF ...).
pub fn extra() -> &'static V {
    EXTRA.get_or_init(|| V::parse(include_str!("data/extra.json")).expect("extra.json"))
}

/// The labels of model.js the checks print (CLASSES, LANDUSES, CONTROLS, STATUS, CYCLE, SCENARIO_ROLES, FUNDING, REGISTER_TYPES).
pub fn model() -> &'static V {
    MODEL.get_or_init(|| V::parse(include_str!("data/model.json")).expect("model.json"))
}

/// A value of the compliance tables by dotted path ("E27.rows", "SRC.rma.url"); null when missing.
pub fn c(p: &str) -> &'static V {
    super::jv::path(core(), p)
}

/// A value of the compliance-extra tables by dotted path.
pub fn x(p: &str) -> &'static V {
    super::jv::path(extra(), p)
}

/// A value of the model.js labels by dotted path.
pub fn m(p: &str) -> &'static V {
    super::jv::path(model(), p)
}

/// The expression a table holds (`{ "$re", "$flags" }`).
pub fn re_of(v: &V) -> Rc<Regex> {
    rx(&v.g("$re").s(), &v.g("$flags").sn())
}
