//! JavaScript's Array.prototype.sort and string order, as V8 does them.

use std::cmp::Ordering;

/// `arr.sort(cmp)` as V8 sorts: the comparator returns a number (below 0: a first; NaN counts as 0) and the sort is stable.
///
/// Arrays shorter than 64 take V8's exact path (TimSort's first run, then binary insertion), so
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
        // CountAndMakeRun(0, n)
        let mut run = 2;
        let descending = order(&v[1], &v[0]) < 0.0;
        let mut prev = 1;
        for idx in 2..n {
            let o = order(&v[idx], &v[prev]);
            if descending {
                if o >= 0.0 {
                    break;
                }
            } else if o < 0.0 {
                break;
            }
            prev = idx;
            run += 1;
        }
        if descending {
            v[..run].reverse();
        }
        run
    } else {
        1
    };
    // BinaryInsertionSort(0, start, n)
    for s in start..n {
        let (mut left, mut right) = (0, s);
        while left < right {
            let mid = left + ((right - left) >> 1);
            if order(&v[s], &v[mid]) < 0.0 {
                right = mid;
            } else {
                left = mid + 1;
            }
        }
        v[left..=s].rotate_right(1);
    }
}

/// `a < b` for JavaScript strings: compares UTF-16 code units.
pub fn js_str_lt(a: &str, b: &str) -> bool {
    a.encode_utf16().cmp(b.encode_utf16()) == Ordering::Less
}
