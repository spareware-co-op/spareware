# A97 and A98: the command layer's tools, as built

**Status, 2026-11-28:** Decided for the build (pieces A97 and A98; main accepted both changes below). Part: CON-A8 (interface), the command layer. It amends note A96's section "The command layer (the next job)", which it follows in everything it does not say here. The contract itself is construct-api's `cloud/tool.rs`, whose header says the same in the code.

## What is built

Every module of construct-api's `cloud` that runs commands is a tool: a unit type implementing `Tool` (`NAME`, `COMMANDS`, `STEPS`, `GUARDS`, `UNDO_ORDER`, `RULES`, `FIELDS`, `apply`, `guard`, `undo_rules`), named once in the ordered `tools!` list in `cloud/mod.rs`. The list expands to `Tools`, which the orderer's commands, undo, the warnings and the schedules ask; none of them names a tool.

## Where it differs from note A96

1. **Two orders, not one.** The hand-written chains were not in one order. The guards before an element command ran view, schedule, dimension, sheet, family, opening, hosting, room, floor, wall, roof, stair, curtain wall (those before a parameter command and before a hosting command were parts of that order). The undo rules ran hosting, family loading, level, view, schedule, sheet, dimension: hosting after the family among the guards (an `element.move` of a placed family is answered with `family.move`'s words) but before it among the undo rules, and dimension before sheet among the guards but after it among the undo rules (an undo of a view with a viewport and a dimension on it is refused by both). No single order keeps every refusal's words, so the list's order is the guards' order, and a tool gives its undo rule's place itself, `UNDO_ORDER` (10 hosting, 20 family loading, 30 level, 40 view, 50 schedule, 60 sheet, 70 dimension; equal places keep the list's order). A test pins both orders for the tools that were in the chains.
2. **`GUARDS`.** A guard is asked only before the commands of the tools it names (the element tool, the parameter tool, the hosting tool), so every guard runs exactly where it ran, and reads nothing more.
3. **The `pub mod` lines stay plain.** The macro does not declare the modules: rustfmt does not reach a module declared inside a macro, so its file would never be formatted or checked. A new tool adds two lines in `cloud/mod.rs`: its `pub mod` line and its line at the end of `tools!`.
4. **Family loading is its own tool**, beside the family tool, whose guard still asks loading's first, as it did.

## Step two: warnings and schedule fields (A98)

Each tool registers its own warning rules (`RULES`, construct-warnings' `Rule`) and schedule fields (`FIELDS`, construct-schedules' `Quantity`). The warnings are worked out from ONE model, read once for each check (`cloud/warnings.rs`), and every tool's rules are asked of that same model, so no rule reads the store. A schedule is read against every tool's fields.

construct-warnings and construct-schedules keep their own types and their tables (`RULES`, `QUANTITIES`), each entry now a constant of its own, which the tools register by name; the tables are what the crates' own plain forms (`warnings`, `Basis::definition` and the rest) and their tests read. The api layer hands them the tools' ones (`warnings_by`, the `_among` forms). A new tool's warnings and fields are its own constants, so adding them touches no shared list. The rule a door or window crossing a joined wall is the hosting tool's.

## How a lane adds a tool

1. Its module `cloud/<tool>.rs`, with `pub struct XTool; impl Tool for XTool { NAME, COMMANDS, ... }`: `GUARDS` names the generic tools whose commands could reach its records; `UNDO_ORDER` places its undo rule (after 70 unless it must come before one); `RULES` and `FIELDS` are its warning rules and schedule fields.
2. In `cloud/mod.rs`: its `pub mod` line, and its line at the end of `tools!`. Nothing in `commands.rs`, `undo.rs`, `warnings.rs` or `schedules.rs`.
3. A new command of an existing tool goes in that tool's `COMMANDS`; a new warning kind or quantity in its `RULES` or `FIELDS`.
