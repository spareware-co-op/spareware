#![allow(clippy::all, clippy::pedantic, clippy::restriction, unused, missing_docs)]
use geom_curve::Frame;
use geom_intersect::contact::{Reading, analyse};
use geom_intersect::general::subdivide;
use geom_intersect::level::{Silhouette, trace};
use geom_intersect::march::march;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Plane, Sphere, SurfaceKind, Torus};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

fn plane(o: [f64; 3], u: [f64; 3], v: [f64; 3], r: f64) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(o, u, v, ((-r, r), (-r, r))).unwrap())
}

fn show(name: &str, a: &SurfaceKind, b: &SurfaceKind, floor: f64) {
    let found = match subdivide(a, b, floor, &mut meter()) {
        Ok(f) => f,
        Err(e) => {
            println!("{name}: subdivide {e:?}");
            return;
        }
    };
    let starts: usize = found.settled.iter().map(|s| s.starts.len()).sum();
    let mut kinds = std::collections::BTreeMap::new();
    for f in &found.flagged {
        *kinds.entry(format!("{:?}", f.why)).or_insert(0) += 1;
    }
    println!("{name}: settled {} starts {starts} flagged {kinds:?}", found.settled.len());
    for f in found.flagged.iter().take(3) {
        println!("    flag {:?} {:?}", f.first, f.second);
    }
    match march(a, b, &found, floor, 1.0, &mut meter()) {
        Ok(t) => {
            for b in &t.branches {
                let f = b.vertices.first().map(|v| v.map(|x| (x.lo() * 1e3).round() / 1e3));
                let l = b.vertices.last().map(|v| v.map(|x| (x.lo() * 1e3).round() / 1e3));
                println!("  branch {} {:?} {:?} .. {:?}", b.vertices.len(), b.ends, f, l);
            }
            println!("  unproven {}", t.unproven.len());
        }
        Err(e) => println!("  march {e:?}"),
    }
    match analyse(a, b, &found, floor, 1.0, &mut meter()) {
        Ok(r) => {
            let mut k = std::collections::BTreeMap::new();
            for x in &r {
                let name = match x {
                    Reading::Point { contact, .. } => format!("Point {contact:?}"),
                    Reading::Crossing(_) => "Crossing".to_string(),
                    Reading::Curve { .. } => "Curve".to_string(),
                    Reading::Overlap { .. } => "Overlap".to_string(),
                    Reading::Undecided { why, .. } => format!("Undecided {why:?}"),
                };
                *k.entry(name).or_insert(0) += 1;
            }
            println!("  readings {k:?}");
        }
        Err(e) => println!("  analyse {e:?}"),
    }
}

#[test]
fn scratch2() {
    let frame = Frame::standard().unwrap();
    let floor = 1e-2;
    // A tent whose ridge (u = 0.5, a knot of a degree-1 basis) a plane across it must cross.
    let basis = Basis::new(1, vec![0.0, 0.0, 0.5, 1.0, 1.0]).unwrap();
    let across = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let net = [
        [-4.0, -4.0, 0.0],
        [-4.0, 4.0, 0.0],
        [0.0625, -4.0, 4.0],
        [0.0625, 4.0, 4.0],
        [4.0, -4.0, 0.0],
        [4.0, 4.0, 0.0],
    ];
    let tent = SurfaceKind::Nurbs(Surface::new(basis, across, &net).unwrap());
    let cut = plane([0.0, 0.03125, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 8.0);
    show("tent-y=0.03", &tent, &cut, floor);
    // A sphere and the plane tangent at its north pole.
    let ball = SurfaceKind::Sphere(Sphere::new([0.0; 3], 5.0, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap());
    let top = plane([0.0625, 0.03125, 5.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 8.0);
    show("sphere-tangent-pole", &ball, &top, floor);
    // A double cone through its apex.
    let through = plane([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 8.0);
    let double = SurfaceKind::Cone(Cone::new([0.0; 3], 0.0, 1.0, frame, ((0.0, 2.0), (-4.0, 4.0))).unwrap());
    show("double-cone-through-apex", &double, &through, floor);
    // A ring torus across both seams.
    let ring = SurfaceKind::Torus(Torus::new([0.0; 3], 5.0, 1.0, frame, ((0.0, 2.0), (0.0, 2.0))).unwrap());
    let side = plane([0.0, 0.03125, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 8.0);
    show("ring-y=0.03", &ring, &side, floor);
    // A spindle through its axis points.
    let spindle = SurfaceKind::Torus(Torus::new([0.0; 3], 2.0, 3.0, frame, ((0.0, 2.0), (-1.0, 1.0))).unwrap());
    show("spindle-through-axis", &spindle, &through, floor);
    // A closed rational cylinder.
    let w = core::f64::consts::FRAC_1_SQRT_2;
    let ku = Basis::new(2, vec![0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0]).unwrap();
    let kv = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let ring2 = [
        [5.0, 0.0],
        [5.0, 5.0],
        [0.0, 5.0],
        [-5.0, 5.0],
        [-5.0, 0.0],
        [-5.0, -5.0],
        [0.0, -5.0],
        [5.0, -5.0],
        [5.0, 0.0],
    ];
    let mut pts = Vec::new();
    let mut ws = Vec::new();
    for (i, [x, y]) in ring2.iter().enumerate() {
        for z in [-8.0, 8.0] {
            pts.push([*x, *y, z]);
            ws.push(if i % 2 == 1 { w } else { 1.0 });
        }
    }
    match Surface::new_rational(ku, kv, &pts, &ws) {
        Ok(s) => {
            let can = SurfaceKind::Nurbs(s);
            let flat = plane([0.0625, 0.03125, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 8.0);
            show("nurbs-cylinder-z=1", &can, &flat, floor);
        }
        Err(e) => println!("nurbs cylinder: {e:?}"),
    }
    // A silhouette across the sphere's seam.
    match trace(&ball, &Silhouette { direction: [0.0625, 0.03125, 1.0] }, floor, 1.0, &mut meter()) {
        Ok(c) => {
            for b in &c.branches {
                println!("  level {} {:?}", b.vertices.len(), b.ends);
            }
            println!("level silhouette: {} branches, {} flagged", c.branches.len(), c.flagged.len());
        }
        Err(e) => println!("level: {e:?}"),
    }
}
