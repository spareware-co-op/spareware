# The SKG record registry

*Every record type of the native format, by tag (architecture 24.2; piece G93). A tag is never reused; a new layout of a type is a new schema version of the same tag. The table between the `generated` markers is written from `geom-format`'s `registry::RECORDS` and checked against it by `crates/geom-format/tests/registry.rs`; add a type there, then run that test's `write_the_tables` to update this file.*

<!-- generated: registry -->
| Tag | Name | Schemas | Flags | CDDL | Defined in |
| --- | --- | --- | --- | --- | --- |
| 1 | `header` | 1 | required | `cddl/header.cddl` | SPEC.md, section 6 |
| 2 | `end` | 1 | required | `cddl/end.cddl` | SPEC.md, section 7 |
| 3 | `body` | 1 | required | `cddl/body.cddl` | SPEC.md, section 10 |
| 4 | `chunk` | 1 | required | `cddl/body.cddl` | SPEC.md, section 10 |
<!-- end generated -->

Format 1 is a draft (see [SPEC.md](SPEC.md)): the pieces that store each family add their record types here as they land (body records, G94; names, history, recipes and attributes, G95). The protocol tag 0x534B4746 is requested from IANA (32.18).
