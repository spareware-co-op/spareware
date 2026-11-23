//! Reader fuzz target, as construct-slabs' are: whatever bytes a roof's recipe holds, reading it
//! answers with a roof or a refusal and never panics, and a roof it reads writes back the same.

#![allow(
    clippy::unwrap_used,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_roofs::Roof;

#[test]
fn reading_stored_edges_never_panics() {
    bolero::check!()
        .with_type::<(String, String)>()
        .for_each(|(footprint, edges)| {
            let (Ok(footprint), Ok(edges)) = (
                serde_json::from_str::<serde_json::Value>(footprint),
                serde_json::from_str::<serde_json::Value>(edges),
            ) else {
                return;
            };
            if let Ok(roof) = Roof::from_values(&footprint, &edges) {
                assert_eq!(roof.edges_value(), edges);
                assert_eq!(roof.footprint().to_value(), footprint);
            }
        });
}
