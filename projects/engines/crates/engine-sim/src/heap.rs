//! The binary min-heap behind the network searches.
//!
//! Port of the transport prototype's util.js `U.Heap`: (number key, integer value) pairs, lazy
//! deletion (callers skip stale entries), and the same sift steps comparison for comparison, so
//! entries with equal keys come out in the same order and shortest-path ties break the same way.

/// Binary min-heap of (key, value) pairs, ordered by key; ties come out in the order the sift steps leave them.
#[derive(Clone, Debug, Default)]
pub struct Heap {
    k: Vec<f64>,
    v: Vec<i32>,
    n: usize,
    /// Key of the entry the last `pop` returned (0 before any pop).
    pub top: f64,
}

impl Heap {
    /// An empty heap.
    pub fn new() -> Self {
        Self::with_capacity(1024)
    }
    /// An empty heap with room for cap entries before it grows.
    pub fn with_capacity(cap: usize) -> Self {
        Heap {
            k: Vec::with_capacity(cap),
            v: Vec::with_capacity(cap),
            n: 0,
            top: 0.0,
        }
    }
    /// Removes every entry.
    pub fn clear(&mut self) {
        self.n = 0;
    }
    /// Number of entries.
    pub fn len(&self) -> usize {
        self.n
    }
    /// True when there are no entries.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    /// Adds an entry.
    pub fn push(&mut self, key: f64, val: i32) {
        if self.n == self.k.len() {
            self.k.push(0.0);
            self.v.push(0);
        }
        let mut i = self.n;
        self.n += 1;
        while i > 0 {
            let p = (i - 1) >> 1;
            if self.k[p] <= key {
                break;
            }
            self.k[i] = self.k[p];
            self.v[i] = self.v[p];
            i = p;
        }
        self.k[i] = key;
        self.v[i] = val;
    }
    /// Removes the entry with the smallest key and returns its value; sets `top` to its key. The heap must not be empty.
    pub fn pop(&mut self) -> i32 {
        assert!(self.n > 0, "pop from an empty heap");
        let rv = self.v[0];
        self.top = self.k[0];
        self.n -= 1;
        let n = self.n;
        if n > 0 {
            let key = self.k[n];
            let val = self.v[n];
            let mut i = 0;
            loop {
                let mut c = 2 * i + 1;
                if c >= n {
                    break;
                }
                if c + 1 < n && self.k[c + 1] < self.k[c] {
                    c += 1;
                }
                if self.k[c] >= key {
                    break;
                }
                self.k[i] = self.k[c];
                self.v[i] = self.v[c];
                i = c;
            }
            self.k[i] = key;
            self.v[i] = val;
        }
        rv
    }
}
