//! The lint of chapter 11.17 item 18 (G244): no topology module of the curved builder calls an
//! angle function. Orders round a vertex and the senses of loops are decided by exact signs and
//! certified enclosures (11.4 step 5: "Floating-point angles are never compared"). The platform's
//! angle functions are already refused in every kernel crate by its clippy.toml; this also refuses
//! the kernel's own (`geom_num::math`'s `atan2` and the like, and conversions to angles), which
//! the shared lint configuration cannot, as geom-num defines them. The sources are read at compile
//! time, so the lint runs on every target.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "test code: a failure should stop loudly"
)]

/// The curved builder's topology modules: where the arrangement's order and loops are decided.
const MODULES: [(&str, &str); 5] = [
    ("interfere.rs", include_str!("../src/interfere.rs")),
    ("pave.rs", include_str!("../src/pave.rs")),
    ("face.rs", include_str!("../src/face.rs")),
    ("inside.rs", include_str!("../src/inside.rs")),
    ("split.rs", include_str!("../src/split.rs")),
];

/// What an angle is computed with: inverse trigonometry and conversions to and from angles.
const ANGLES: [&str; 9] = [
    "atan2",
    "atan(",
    "acos(",
    "asin(",
    "atan2pi",
    "acospi",
    "asinpi",
    "to_degrees",
    "to_radians",
];

/// The angle functions a source calls, outside its comments: (line, function).
fn angle_calls(source: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for (n, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for name in ANGLES {
            if code.contains(name) {
                found.push((n + 1, name));
            }
        }
    }
    found
}

#[test]
fn no_topology_module_calls_an_angle_function() {
    for (name, source) in MODULES {
        assert!(source.len() > 1000, "{name} was read");
        let calls = angle_calls(source);
        assert_eq!(calls.len(), 0, "{name} calls an angle function: {calls:?}");
    }
}

#[test]
fn the_lint_finds_a_planted_angle() {
    let planted = "let turn = geom_num::math::atan2(y, x)?;\n// atan2 in a comment is fine\n";
    assert_eq!(angle_calls(planted), vec![(1, "atan2")]);
    assert_eq!(angle_calls("let a = x.acos();").len(), 1);
    assert_eq!(angle_calls("let speed = norm(v);").len(), 0);
}
