//! Results and queries on results from engine.js.
//!
//! [`Res`] is the typed result of one assignment (what `Engine.assign` returns for a
//! scenario and period): convergence, link (arc) results, junction results, turning
//! movements, bus routes, development tracking, junction sites, the model metadata, KPIs,
//! the demand vectors and the stored paths. It is the contract the assignment port fills,
//! and it reads and writes the JSON the prototype gives (key order, keys left out, nulls,
//! and non-finite numbers as tools/transport-truth writes them; see [`plain`]).
//! The queries port engine.js "queries on results" and "results by site, and the model
//! status metadata": travel between nodes and isochrones (on the graph port's graph, or any
//! [`RouteGraph`]), select-link from stored paths, export readiness, cycling level of traffic
//! stress, the model status and the metadata every result carries. Junction-site results
//! (siteResults) need the graph's controllers and are left to the assignment port.
//!
//! Results are passed explicitly: where engine.js reads `sc.res[period]` (`Store.res`),
//! the Rust functions take a [`Results`] holder (a scenario's results by period) or a
//! `&Res`, beside the scenario and the settings they read (`Store.S`).

mod queries;
mod types;

pub use queries::*;
pub use types::*;

use crate::jsnum::plain_number;
use crate::project::OrderedMap;
use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};
use std::fmt;

/// A scenario's results by period ("AM", "PM"), as `sc.res` holds them in engine.js
/// (`Store.res(sc, period)` is `results.get(period)`).
pub type Results = OrderedMap<Res>;

/* ---------------- numbers in results ---------------- */

/// Serde for numbers in results (`#[serde(with = "plain")]`): finite numbers as JSON numbers,
/// NaN and infinities as the text "NaN", "Infinity" and "-Infinity" (as tools/transport-truth
/// writes them); reading accepts both. Works for `f64`, `Option<f64>` (null), `Vec`,
/// `OrderedMap` and `[f64; 2]` of those.
pub mod plain {
    use super::*;

    /// A value whose numbers are written and read the plain way.
    pub trait Plain: Sized {
        /// Writes the value.
        fn ser<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error>;
        /// Reads the value.
        fn de<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error>;
    }

    /// Serde `serialize_with` entry point.
    pub fn serialize<T: Plain, S: Serializer>(v: &T, s: S) -> Result<S::Ok, S::Error> {
        v.ser(s)
    }

    /// Serde `deserialize_with` entry point.
    pub fn deserialize<'de, T: Plain, D: Deserializer<'de>>(d: D) -> Result<T, D::Error> {
        T::de(d)
    }

    /// A reference that serializes through [`Plain`].
    pub struct Ser<'a, T>(pub &'a T);

    impl<T: Plain> Serialize for Ser<'_, T> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            self.0.ser(s)
        }
    }

    /// A value that deserializes through [`Plain`].
    pub struct De<T>(pub T);

    impl<'de, T: Plain> Deserialize<'de> for De<T> {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            T::de(d).map(De)
        }
    }

    struct NumVisitor;

    impl Visitor<'_> for NumVisitor {
        type Value = f64;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a number, or \"NaN\", \"Infinity\" or \"-Infinity\"")
        }
        fn visit_f64<E: de::Error>(self, x: f64) -> Result<f64, E> {
            Ok(x)
        }
        fn visit_i64<E: de::Error>(self, x: i64) -> Result<f64, E> {
            Ok(x as f64)
        }
        fn visit_u64<E: de::Error>(self, x: u64) -> Result<f64, E> {
            Ok(x as f64)
        }
        fn visit_str<E: de::Error>(self, s: &str) -> Result<f64, E> {
            match s {
                "NaN" => Ok(f64::NAN),
                "Infinity" => Ok(f64::INFINITY),
                "-Infinity" => Ok(f64::NEG_INFINITY),
                _ => Err(E::invalid_value(de::Unexpected::Str(s), &self)),
            }
        }
    }

    impl Plain for f64 {
        fn ser<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            plain_number(*self).serialize(s)
        }
        fn de<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            d.deserialize_any(NumVisitor)
        }
    }

    impl<T: Plain> Plain for Option<T> {
        fn ser<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            match self {
                Some(v) => v.ser(s),
                None => s.serialize_unit(),
            }
        }
        fn de<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            Ok(Option::<De<T>>::deserialize(d)?.map(|x| x.0))
        }
    }

    impl<T: Plain> Plain for Vec<T> {
        fn ser<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.collect_seq(self.iter().map(Ser))
        }
        fn de<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            Ok(Vec::<De<T>>::deserialize(d)?
                .into_iter()
                .map(|x| x.0)
                .collect())
        }
    }

    impl Plain for [f64; 2] {
        fn ser<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.collect_seq(self.iter().map(Ser))
        }
        fn de<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let v = Vec::<f64>::de(d)?;
            if v.len() != 2 {
                return Err(de::Error::invalid_length(v.len(), &"a [lat, lng] pair"));
            }
            Ok([v[0], v[1]])
        }
    }

    impl<T: Plain> Plain for OrderedMap<T> {
        fn ser<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.collect_map(self.iter().map(|(k, v)| (k, Ser(v))))
        }
        fn de<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            Ok(OrderedMap::<De<T>>::deserialize(d)?
                .into_iter()
                .map(|(k, v)| (k, v.0))
                .collect())
        }
    }
}

/* ---------------- JavaScript helpers the queries share ---------------- */

/// `arr.sort((a, b) => cmp(a, b))` with a comparator returning a JavaScript number: a stable
/// merge sort that reads NaN as 0 (equal), as `Array.prototype.sort` does.
///
/// For a consistent comparator the order is the one V8 gives (every stable sort agrees);
/// unlike `slice::sort_by` it never panics on an inconsistent one.
pub fn js_sort_by<T>(v: &mut Vec<T>, mut cmp: impl FnMut(&T, &T) -> f64) {
    let n = v.len();
    if n < 2 {
        return;
    }
    // bottom-up merge sort of the positions, then the items moved into that order
    let mut order: Vec<usize> = (0..n).collect();
    let mut buf = vec![0usize; n];
    let mut width = 1;
    while width < n {
        let mut lo = 0;
        while lo < n {
            let mid = (lo + width).min(n);
            let hi = (lo + 2 * width).min(n);
            let (mut i, mut j, mut k) = (lo, mid, lo);
            while i < mid && j < hi {
                // take from the right only when the left sorts strictly after it
                if cmp(&v[order[i]], &v[order[j]]) > 0.0 {
                    buf[k] = order[j];
                    j += 1;
                } else {
                    buf[k] = order[i];
                    i += 1;
                }
                k += 1;
            }
            buf[k..k + mid - i].copy_from_slice(&order[i..mid]);
            k += mid - i;
            buf[k..k + hi - j].copy_from_slice(&order[j..hi]);
            lo = hi;
        }
        std::mem::swap(&mut order, &mut buf);
        width *= 2;
    }
    let mut items: Vec<Option<T>> = v.drain(..).map(Some).collect();
    v.extend(
        order
            .into_iter()
            .map(|i| items[i].take().expect("each item once")),
    );
}
