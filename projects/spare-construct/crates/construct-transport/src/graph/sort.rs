//! JavaScript's Array.prototype.sort and string order, as V8 does them.

use std::cmp::Ordering;

/// `arr.sort(cmp)` as V8 sorts: the comparator returns a number (below 0: a first; NaN counts as 0) and the sort is stable.
///
/// Arrays shorter than 64 take V8's exact path (`TimSort`'s first run, then binary insertion), so
/// even a comparator that is not a consistent order (NaN keys) gives V8's order. Longer arrays
/// use a binary insertion sort over the whole array, which equals V8 for consistent comparators.
pub fn js_sort_by<T>(v: &mut [T], mut cmp: impl FnMut(&T, &T) -> f64) {
    let n = v.len();
    if n < 2 {
        return;
    }
    let mut order = |a: &T, b: &T| {
        let x = cmp(a, b);
        if x.is_nan() { 0.0 } else { x }
    };
    let start = if n < 64 {
        // CountAndMakeRun(0, n): each element after the first two against the one before it
        let (Some(v0), Some(v1)) = (v.first(), v.get(1)) else {
            return;
        };
        let descending = order(v1, v0) < 0.0;
        let mut run = 2usize;
        for pair in v.windows(2).skip(1) {
            let [prev, cur] = pair else {
                break;
            };
            let o = order(cur, prev);
            if descending {
                if o >= 0.0 {
                    break;
                }
            } else if o < 0.0 {
                break;
            }
            run = run.saturating_add(1);
        }
        if descending && let Some(head) = v.get_mut(..run) {
            head.reverse();
        }
        run
    } else {
        1
    };
    // BinaryInsertionSort(0, start, n)
    for s in start..n {
        let (mut left, mut right) = (0, s);
        while left < right {
            let mid = usize::midpoint(left, right);
            let less = match (v.get(s), v.get(mid)) {
                (Some(a), Some(b)) => order(a, b) < 0.0,
                _ => false,
            };
            if less {
                right = mid;
            } else {
                left = mid.saturating_add(1);
            }
        }
        if let Some(w) = v.get_mut(left..=s) {
            w.rotate_right(1);
        }
    }
}

/// `a < b` for JavaScript strings: compares UTF-16 code units.
#[must_use]
pub fn js_str_lt(a: &str, b: &str) -> bool {
    a.encode_utf16().cmp(b.encode_utf16()) == Ordering::Less
}
