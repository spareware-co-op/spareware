# CON-A2-F2-P3: Binding a parameter to categories

**Status, 2026-11-22:** Decided for the build. The crate `construct-params` follows it (modules `shared`, `project` and `info`); the stored forms are in its [FORMAT.md](../crates/construct-params/FORMAT.md).

## The rules

- A shared parameter's ID is a UUID the caller gives, in one spelling (lower case, hyphenated, not all zero). The same ID with another kind, name, group, holder or choices is refused, so one ID is one meaning in every project and family.
- A parameter is bound to categories as instance or type. A subcategory cannot be bound: an element is in a top-level category, and a subcategory only names a part a drawing draws differently. An element of a subcategory has its parent's parameters.
- A type parameter needs elements that have types; imported files and analytical stand-ins have none, so they hold instance parameters only.
- A binding may carry a default value of the parameter's kind. The resolution order for a value is instance, then type, then this default; the types job feeds its parameter list from the bindings.
- Unbinding reports how many holders had a value that goes; this crate keeps no index, so the caller gives the holders.
- Project information is a fixed set of text parameters of the project itself (name, number, client, address, stage, issue date) with pinned IDs, plus extras the project defines. The stage is text because offices name stages differently; the issue date is a calendar date kept as `YYYY-MM-DD`.
