# The SKG record registry

*Every record type of the native format, by tag (architecture 24.2; piece G93). A tag is never reused; a new layout of a type is a new schema version of the same tag. The table between the `generated` markers is written from `geom-format`'s `registry::RECORDS` and checked against it by `crates/geom-format/tests/registry.rs`; add a type there, then run that test's `write_the_tables` to update this file.*

<!-- generated: registry -->
| Tag | Name | Schemas | Flags | CDDL | Defined in |
| --- | --- | --- | --- | --- | --- |
| 1 | `header` | 1 | required | `cddl/header.cddl` | SPEC.md, section 6 |
| 2 | `end` | 1 | required | `cddl/end.cddl` | SPEC.md, section 7 |
| 16 | `names` | 1 | required | `cddl/names.cddl` | SPEC.md, section 10 |
| 17 | `history` | 1 | required | `cddl/history.cddl` | SPEC.md, section 11 |
| 18 | `recipe` | 1 | required | `cddl/recipe.cddl` | SPEC.md, section 12 |
| 19 | `attributes` | 1 | required | `cddl/attributes.cddl` | SPEC.md, section 13 |
| 20 | `operation` | 1 | required | `cddl/operation.cddl` | SPEC.md, section 14 |
| 21 | `pruned` | 1 | required | `cddl/pruned.cddl` | SPEC.md, section 15 |
<!-- end generated -->

Format 1 is a draft (see [SPEC.md](SPEC.md)): the pieces that store each family add their record types here as they land (body records, G94; names, history, recipes and attributes, G95). The protocol tag 0x534B4746 is requested from IANA (32.18).
