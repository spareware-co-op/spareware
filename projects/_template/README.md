# Template for a new program

Copy this folder to `projects/spare-<name>/` when a program moves from Idea to Mapping.

| Path | Holds |
| --- | --- |
| `README.md` | What it is, what it replaces, its stage |
| `ARCHITECTURE.md` | Its map: high level and low level, discuss points, open decisions |
| `STATUS.md` | Its work board |
| `notes/` | Research and decision notes, `<ID>-<slug>.md` |
| `crates/` | Its Rust libraries, named `<short>-<part>` (added at the Design stage) |
| `app/` | Its interface and shell |

Rule: a program's crates may depend on `projects/kit` and `projects/engines`, never on another program. Anything two programs both need moves into the kit or an engine.
