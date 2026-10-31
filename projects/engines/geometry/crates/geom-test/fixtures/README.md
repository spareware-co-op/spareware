# Fixtures

Every failure ends here as a test that runs forever (chapter 28.16 of the kernel architecture). One JSON file per fixture, at `<family>/<number>-<slug>.json`: its source, its operation log as the hexadecimal bytes of a standalone `.skgf` file, one expectation per kernel edition and set of operation versions, and its status. `geom_test::fixture` holds the rules, and `fixture` (`cargo run -p geom-test --bin fixture -- --help`) lists, checks and runs them.

Fixtures are never deleted, and a file holds its one spelling, which `Fixture::to_json` writes.
