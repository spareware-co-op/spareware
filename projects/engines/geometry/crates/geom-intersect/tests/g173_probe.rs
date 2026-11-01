#![allow(clippy::all, clippy::pedantic, clippy::restriction, unused)]
use geom_curve::Frame;
use geom_intersect::contact::{Reading, analyse};
use geom_intersect::general::subdivide;
use geom_intersect::march::march;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind, Torus};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 23))
}

fn plane(o: [f64; 3], u: [f64; 3], v: [f64; 3], r: f64) -> SurfaceKind {
    SurfaceKind::Plane(Plane::new(o, u, v, ((-r, r), (-r, r))).unwrap())
}

fn show(name: &str, a: &SurfaceKind, b: &SurfaceKind, floor: f64) {
    let mut m = meter();
    let found = match subdivide(a, b, floor, &mut m) {
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
    match march(a, b, &found, floor, 1.0, &mut meter()) {
        Ok(t) => {
            let s: Vec<_> = t.branches.iter().map(|b| (b.vertices.len(), b.ends)).collect();
            println!("  branches {s:?} unproven {}", t.unproven.len());
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
fn scratch() {
    let frame = Frame::standard().unwrap();
    let floor = 1e-2;
    let flat = plane([0.0625, 0.03125, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 8.0);
    // E1 a whole sphere, a plane across the seam.
    let ball = SurfaceKind::Sphere(Sphere::new([0.0; 3], 5.0, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap());
    show("sphere-z=1", &ball, &flat, floor);
    // E2 a plane through both poles.
    let upright = plane([0.25, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 8.0);
    show("sphere-x=.25", &ball, &upright, floor);
    let through = plane([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 8.0);
    show("sphere-through-poles", &ball, &through, floor);
    // E3 a whole cylinder.
    let can = SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 5.0, frame, ((0.0, 2.0), (-8.0, 8.0))).unwrap());
    show("cylinder-z=1", &can, &flat, floor);
    // E4 a crease.
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
    let crease = SurfaceKind::Nurbs(Surface::new(basis, across, &net).unwrap());
    show("crease-z=1", &crease, &flat, floor);
    // E5 a cone's apex.
    let cone = SurfaceKind::Cone(Cone::new([0.0; 3], 0.0, 1.0, frame, ((0.0, 2.0), (0.0, 4.0))).unwrap());
    show("cone-through-apex", &cone, &through, floor);
    show("cone-z=1", &cone, &flat, floor);
    // E6 lemon.
    match Torus::new([0.0; 3], 2.0, 3.0, frame, ((0.0, 2.0), (-1.0, 1.0))) {
        Ok(t) => show("lemon-apple", &SurfaceKind::Torus(t), &flat, floor),
        Err(e) => println!("lemon: {e:?}"),
    }
}
