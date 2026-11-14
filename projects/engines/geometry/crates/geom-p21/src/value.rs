//! The values of a Part 21 file as a tree: a parameter, a record and an entity instance.
//!
//! What it does: [`Param`] is one parameter value, [`Record`] an entity name with its parameters,
//! and [`Entity`] the records of one instance (one for a simple instance, several for a complex
//! one). A nested list is a [`List`], which knows how deep it nests and cannot be built deeper than
//! [`MAX_DEPTH`].
//!
//! Guarantees:
//!
//! - **Nesting is bounded.** No [`Param`] nests more than [`MAX_DEPTH`] lists, however it was
//!   made: [`List::new`] refuses a deeper one, and a list's items cannot be changed once it is
//!   built. So dropping, cloning, comparing and printing a value never go deeper than that.
//! - **Exact or absent.** [`Param::as_f64`] gives an integer as a real only where the real holds it
//!   exactly, and never gives a value that is not finite.
//!
//! Method: the parameter kinds of ISO 10303-21's exchange structure
//! (<https://www.iso.org/standard/63141.html>), as chapter 24.8 of
//! `projects/engines/geometry/architecture/24-file-format-exchange.md` asks: the encoding, not the
//! entities.
//!
//! Cost: each accessor is constant time, except [`Entity::record`] and [`Entity::has`], linear in
//! the instance's records. [`List::new`] is linear in its items. Dropping or cloning a value is
//! linear in its size and recurses [`MAX_DEPTH`] deep at most.
//!
//! Recursion:
//!
//! - Types: `Param`, `List`.
//!   Bound: 64 (`MAX_DEPTH`).
//!   Why: a list holds parameters, and `List::new`, the only way to make one outside the reader,
//!   refuses a list nested deeper than `MAX_DEPTH`; the reader refuses such a text before it
//!   makes a token.
//!   Test: `deepest_param_drops_clones_compares_and_prints`.
//!
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/a3a8244566081e0777284dc182f77a91bb1a456d/crates/solidcore-record/src/step/p21.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: `Param`, `Record` and `Entity` with their accessors, in `no_std`
//!   Rust. A list is the new `List` type, which carries its depth, so the nesting bound holds for
//!   values a caller builds and not only for parsed ones. `as_f64` gives an integer only where a
//!   double holds it exactly (it rounded beyond 2^53) and looks one typed value deep, without
//!   recursion. `as_i64` no longer reads a real with no fraction as an integer: that is a repair
//!   of one writer's files, which belongs to the reader of the schema.
//! SPDX-License-Identifier: Apache-2.0

use alloc::string::String;
use alloc::vec::Vec;

/// The deepest nesting of lists inside one record's parameter list.
pub const MAX_DEPTH: usize = 64;

/// The largest whole number every double below it holds exactly: 2^53.
const EXACT_INTEGERS: u64 = 1 << 53;

/// A parameter value.
#[derive(Clone, Debug, PartialEq)]
pub enum Param {
    /// An integer.
    Int(i64),
    /// A real (`1.`, `1.5E1`, `1.E-3`).
    Real(f64),
    /// A string, its escapes decoded.
    Str(String),
    /// `.NAME.` (`.T.` and `.F.` included), without its dots, in upper case.
    Enum(String),
    /// An entity instance name, `#n`.
    Ref(u64),
    /// `(a, b, ...)`.
    List(List),
    /// `NAME(params)` (a typed value such as `LENGTH_MEASURE(1.)`).
    Typed(String, List),
    /// `"hex"`, the digits between the quotes as written.
    Binary(String),
    /// `$`
    Null,
    /// `*`
    Derived,
}

impl Param {
    /// How many lists deep the value nests: 0 for a value that is no list.
    #[must_use]
    pub const fn depth(&self) -> usize {
        match self {
            Param::List(l) | Param::Typed(_, l) => l.depth(),
            _ => 0,
        }
    }

    /// A finite real or an integer a double holds exactly.
    fn scalar_f64(&self) -> Option<f64> {
        match self {
            Param::Real(x) => Some(*x).filter(|x| x.is_finite()),
            Param::Int(i) if i.unsigned_abs() <= EXACT_INTEGERS => {
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "exact: the magnitude is at most 2^53, which every double holds"
                )]
                let x = *i as f64;
                Some(x)
            }
            _ => None,
        }
    }

    /// A finite real, an integer a double holds exactly, or a typed value holding just one of
    /// them (`LENGTH_MEASURE(1.)`).
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Param::Typed(_, v) => match v.items() {
                [one] => one.scalar_f64(),
                _ => None,
            },
            _ => self.scalar_f64(),
        }
    }

    /// An integer.
    #[must_use]
    pub const fn as_i64(&self) -> Option<i64> {
        match self {
            Param::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// The id of an entity reference.
    #[must_use]
    pub const fn as_ref_id(&self) -> Option<u64> {
        match self {
            Param::Ref(r) => Some(*r),
            _ => None,
        }
    }

    /// The text of a string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Param::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The name of an enumeration, without its dots.
    #[must_use]
    pub fn as_enum(&self) -> Option<&str> {
        match self {
            Param::Enum(s) => Some(s),
            _ => None,
        }
    }

    /// `.T.` is true and `.F.` false; `.U.` and anything else is neither.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self.as_enum()? {
            "T" => Some(true),
            "F" => Some(false),
            _ => None,
        }
    }

    /// The items of a list.
    #[must_use]
    pub fn as_list(&self) -> Option<&[Param]> {
        match self {
            Param::List(v) => Some(v.items()),
            _ => None,
        }
    }
}

/// A list of parameters that nests at most [`MAX_DEPTH`] deep.
#[derive(Clone, Debug, PartialEq)]
pub struct List {
    items: Vec<Param>,
    /// One more than the deepest item's depth.
    depth: usize,
}

/// A list refused because it would nest deeper than [`MAX_DEPTH`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TooDeepError;

impl core::fmt::Display for TooDeepError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("lists nested too deeply")
    }
}

impl core::error::Error for TooDeepError {}

impl List {
    /// The list of `items`.
    ///
    /// # Errors
    ///
    /// [`TooDeepError`] when an item already nests [`MAX_DEPTH`] deep.
    pub fn new(items: Vec<Param>) -> Result<Self, TooDeepError> {
        let list = Self::within_bound(items);
        if list.depth > MAX_DEPTH {
            return Err(TooDeepError);
        }
        Ok(list)
    }

    /// The list of `items`, which the caller knows nest less than [`MAX_DEPTH`] deep: the reader,
    /// which refused anything deeper before it made a token, and lists of values that are no
    /// lists.
    pub(crate) fn within_bound(items: Vec<Param>) -> Self {
        let mut deepest = 0_usize;
        for item in &items {
            deepest = deepest.max(item.depth());
        }
        Self {
            items,
            depth: deepest.saturating_add(1),
        }
    }

    /// The list with no item.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            items: Vec::new(),
            depth: 1,
        }
    }

    /// The items, in file order.
    #[must_use]
    pub fn items(&self) -> &[Param] {
        &self.items
    }

    /// The items, given back.
    #[must_use]
    pub fn into_items(self) -> Vec<Param> {
        self.items
    }

    /// How many lists deep this one nests, itself counted: at least 1.
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }
}

/// One record of an entity instance: a simple instance has one, a complex one several.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// The entity type name, upper case.
    pub name: String,
    /// The record's parameters, in file order.
    pub params: Vec<Param>,
}

/// An entity instance.
#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    /// Its records, in file order.
    pub records: Vec<Record>,
}

impl Entity {
    /// Whether the instance has more than one record.
    #[must_use]
    pub fn is_complex(&self) -> bool {
        self.records.len() > 1
    }

    /// Name of a simple instance (the first record of a complex one).
    #[must_use]
    pub fn name(&self) -> &str {
        self.records.first().map_or("", |r| r.name.as_str())
    }

    /// Parameters of the record called `name`.
    #[must_use]
    pub fn record(&self, name: &str) -> Option<&[Param]> {
        self.records
            .iter()
            .find(|r| r.name == name)
            .map(|r| r.params.as_slice())
    }

    /// Whether the instance has a record called `name`.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.records.iter().any(|r| r.name == name)
    }

    /// Parameters of a simple instance.
    #[must_use]
    pub fn params(&self) -> &[Param] {
        self.records.first().map_or(&[], |r| r.params.as_slice())
    }
}
