//! The path sets of an assignment: Maps in insertion order, as the JavaScript's `Map` keeps them.
//!
//! A deleted key that is added again goes last, as it does in a `Map`, and iteration visits the
//! keys in the order they were added. The sets are `od -> path -> flow`, where a path is the
//! list of arcs it uses.

use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

/// A map that iterates in insertion order and lets entries be deleted while it is walked.
pub(super) struct OMap<K, V> {
    slots: Vec<Option<(K, V)>>,
    index: HashMap<K, usize>,
}

impl<K: Hash + Eq + Clone, V> OMap<K, V> {
    /// An empty map.
    pub fn new() -> Self {
        OMap {
            slots: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// The value for a key.
    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        match self.index.get(k) {
            Some(&i) => self.slots[i].as_mut().map(|e| &mut e.1),
            None => None,
        }
    }

    /// Adds a new key last (the key must not be in the map).
    pub fn push(&mut self, k: K, v: V) {
        self.index.insert(k.clone(), self.slots.len());
        self.slots.push(Some((k, v)));
    }

    /// The entries in order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.slots.iter().filter_map(|e| e.as_ref().map(|e| (&e.0, &e.1)))
    }

    /// The entries in order, with values for changing.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        self.slots.iter_mut().filter_map(|e| e.as_mut().map(|e| (&e.0, &mut e.1)))
    }

    /// Visits every entry in order and deletes those for which f returns false.
    pub fn retain(&mut self, mut f: impl FnMut(&K, &mut V) -> bool) {
        for slot in &mut self.slots {
            let keep = match slot {
                Some(e) => f(&e.0, &mut e.1),
                None => true,
            };
            if !keep && let Some(e) = slot.take() {
                self.index.remove(&e.0);
            }
        }
        // Dropping the empty slots keeps the order and the walk short
        if self.slots.iter().filter(|s| s.is_none()).count() * 2 > self.slots.len() {
            self.slots.retain(|s| s.is_some());
            for (i, s) in self.slots.iter().enumerate() {
                if let Some(e) = s {
                    self.index.insert(e.0.clone(), i);
                }
            }
        }
    }
}

/// The paths of one OD pair: the arcs a path uses, and its flow (pcu/h).
pub(super) type PairPaths = OMap<Rc<[usize]>, f64>;

/// Paths by OD pair (`od = i * Z + j`) with their flows.
pub(super) type PathSet = OMap<usize, PairPaths>;

/// One all-or-nothing path: its OD pair, its arcs in order and the trips on it (pcu/h).
pub(super) type NewPath = (usize, Vec<usize>, f64);

/// `addPaths(np, w, into)`: adds the all-or-nothing paths, each scaled by w, to a set.
pub(super) fn add_paths(np: Vec<NewPath>, w: f64, into: &mut PathSet) {
    for (od, arcs, q) in np {
        if into.get_mut(&od).is_none() {
            into.push(od, PairPaths::new());
        }
        let m = into.get_mut(&od).expect("just added");
        let key: Rc<[usize]> = Rc::from(arcs);
        match m.get_mut(&key) {
            Some(p) => *p += w * q,
            None => m.push(key, w * q),
        }
    }
}

/// `scalePaths(set, f)`: multiplies every flow by f and drops the paths left under 0.002 pcu/h.
pub(super) fn scale_paths(set: &mut PathSet, f: f64) {
    for (_, m) in set.iter_mut() {
        m.retain(|_, w| {
            *w *= f;
            !(*w < 0.002)
        });
    }
}

/// Adds lam times the flows of `from` to `to` (paths in `from` that `to` lacks are added last).
pub(super) fn merge_paths(to: &mut PathSet, from: &PathSet, lam: f64) {
    for (od, m) in from.iter() {
        if to.get_mut(od).is_none() {
            to.push(*od, PairPaths::new());
        }
        let pm = to.get_mut(od).expect("just added");
        for (k, w) in m.iter() {
            match pm.get_mut(k) {
                Some(e) => *e += lam * *w,
                None => pm.push(k.clone(), lam * *w),
            }
        }
    }
}
