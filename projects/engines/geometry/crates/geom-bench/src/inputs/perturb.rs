//! Inputs for the perturbed predicates (G30): [`orient2d`], [`orient3d`], [`incircle`] and
//! [`insphere`] of `geom_num::perturb`, each point with a distinct key.
//!
//! Classes:
//!
//! - `typical`: the exact predicates' typical inputs, whose exact sign is not zero, so the
//!   perturbed predicate costs the exact one and a check of the keys.
//! - `degenerate`: the exact predicates' degenerate inputs (collinear, coplanar, cocircular and
//!   cospherical points), exact ties, so the perturbation's walk over masks decides.
//!
//! Guarantees: every batch is full, the same on every target, and its classes are what they
//! claim (`tests/inputs.rs`); the keys within an input are distinct and their order rotates from
//! input to input, so every argument position holds the smallest key in turn.
//!
//! Method: the exact predicates' own input classes ([`super::predicate`], after Shewchuk,
//! <https://doi.org/10.1007/PL00009321>, section 6), drawn with this operation's seed, with keys
//! attached.
//!
//! Cost: the exact predicates' inputs and N keys per input, once per batch.

use geom_num::grid::{P2, P3};

use super::{Class, predicate};

/// Attaches distinct keys to each input: input n's argument i gets key N·n + ((i + n) mod N).
fn keyed<P: Copy, const N: usize>(batch: Vec<[P; N]>) -> Vec<[(P, u64); N]> {
    batch
        .into_iter()
        .enumerate()
        .map(|(n, points)| {
            let mut i = 0;
            points.map(|p| {
                let key = (N * n + (i + n) % N) as u64;
                i += 1;
                (p, key)
            })
        })
        .collect()
}

/// Keyed point triples for `perturb::orient2d`.
#[must_use]
pub fn orient2d(class: Class) -> Vec<[(P2, u64); 3]> {
    keyed(predicate::orient2d_from("perturb::orient2d", class))
}

/// Keyed point quadruples for `perturb::orient3d`.
#[must_use]
pub fn orient3d(class: Class) -> Vec<[(P3, u64); 4]> {
    keyed(predicate::orient3d_from("perturb::orient3d", class))
}

/// Keyed point quadruples for `perturb::incircle`.
#[must_use]
pub fn incircle(class: Class) -> Vec<[(P2, u64); 4]> {
    keyed(predicate::incircle_from("perturb::incircle", class))
}

/// Keyed point quintuples for `perturb::insphere`.
#[must_use]
pub fn insphere(class: Class) -> Vec<[(P3, u64); 5]> {
    keyed(predicate::insphere_from("perturb::insphere", class))
}
