#![allow(clippy::all, clippy::pedantic, warnings)]
use geom_num::interval::Interval;
fn det(m: [[Interval; 3]; 3]) -> Option<Interval> {
    let [[a, b, c], [d, e, f], [g, h, k]] = m;
    let minor = |w: Interval, x: Interval, y: Interval, z: Interval| -> Option<Interval> { w.mul(x).ok()?.sub(y.mul(z).ok()?).ok() };
    let (m1, m2, m3) = (minor(e, k, f, h)?, minor(d, k, f, g)?, minor(d, h, e, g)?);
    a.mul(m1).ok()?.sub(b.mul(m2).ok()?).ok()?.add(c.mul(m3).ok()?).ok()
}
#[test]
fn probe() {
    for k in 44..=53 {
        for m in [1.0, 3.0, 5.0, 7.0] {
            let eps = m * f64::from_bits(((1023 - k) as u64) << 52);
            let rows = [[1.0, 1.0, 0.0], [1.0, 1.0 + eps, 0.0], [0.0, 0.0, 1.0]];
            let p = rows.map(|r| r.map(|x| Interval::point(x).unwrap()));
            let w = rows.map(|r| r.map(|x| Interval::new(x.next_down(), x.next_up()).unwrap()));
            let (dp, dw) = (det(p).unwrap(), det(w).unwrap());
            println!("PROBE k {k} m {m} eps {eps:e} orig {:?} [{:e},{:e}] widened {:?}", dp.certain_sign(), dp.lo(), dp.hi(), dw.certain_sign());
        }
    }
}
