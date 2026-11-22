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
const MODULES: [(&str, &str); 6] = [
    ("interfere.rs", include_str!("../src/interfere.rs")),
    ("pave.rs", include_str!("../src/pave.rs")),
    ("face.rs", include_str!("../src/face.rs")),
    ("inside.rs", include_str!("../src/inside.rs")),
    ("picture.rs", include_str!("../src/picture.rs")),
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

/// What each of [`ANGLES`] starts with.
const STEMS: [&str; 4] = ["atan", "acos", "asin", "to_"];

/// The calls [`angle_calls`] finds, found by searching the whole source once for each of
/// [`STEMS`] rather than each line for each function: a function named where a stem is found
/// counts where no `//` comes before it on its line (no function's name holds a `/`, so a name
/// starting before a line's first `//` lies wholly in its code).
fn angle_calls_whole(source: &str) -> Vec<(usize, &'static str)> {
    assert!(
        ANGLES
            .iter()
            .all(|name| STEMS.iter().any(|stem| name.starts_with(stem)))
    );
    let mut found = Vec::new();
    for stem in STEMS {
        for (at, _) in source.match_indices(stem) {
            for (k, name) in ANGLES.iter().enumerate() {
                if !source[at..].starts_with(name) {
                    continue;
                }
                let start = source[..at].rfind('\n').map_or(0, |n| n + 1);
                if !source[start..at].contains("//") {
                    found.push((source[..at].matches('\n').count() + 1, k));
                }
            }
        }
    }
    found.sort_unstable();
    found.dedup();
    found
        .into_iter()
        .map(|(line, k)| (line, ANGLES[k]))
        .collect()
}

#[test]
fn no_topology_module_calls_an_angle_function() {
    for (name, source) in MODULES {
        assert!(source.len() > 1000, "{name} was read");
        // Under Miri a search set up anew for every line and function takes minutes: each source
        // is searched whole instead, which finds the same calls, as the planted ones show.
        let calls = if cfg!(miri) {
            angle_calls_whole(source)
        } else {
            angle_calls(source)
        };
        assert_eq!(calls.len(), 0, "{name} calls an angle function: {calls:?}");
    }
    if cfg!(miri) {
        let planted = "let turn = geom_num::math::atan2(y, x)?;\n// atan2 in a comment is fine\n\
                       let a = x.acos(); // atan2pi\nlet d = x.to_degrees().atan2pi(y);\n";
        assert_eq!(angle_calls_whole(planted), angle_calls(planted));
        assert_eq!(
            angle_calls_whole(planted),
            vec![
                (1, "atan2"),
                (3, "acos("),
                (4, "atan2"),
                (4, "atan2pi"),
                (4, "to_degrees")
            ]
        );
    }
}

#[test]
fn the_lint_finds_a_planted_angle() {
    let planted = "let turn = geom_num::math::atan2(y, x)?;\n// atan2 in a comment is fine\n";
    assert_eq!(angle_calls(planted), vec![(1, "atan2")]);
    assert_eq!(angle_calls("let a = x.acos();").len(), 1);
    assert_eq!(angle_calls("let speed = norm(v);").len(), 0);
}
