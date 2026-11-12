# A10: Study ArtCraft's repo layout and agent workflow

## The question

The ArtCraft team builds desktop apps from scratch in Rust with coding agents, across a family of "Craft" repositories. The piece asks how they do it, and what Spare Construct should copy.

Sources were read on 12 November 2026: ArtCraft at [`84a6a7d`](https://github.com/storytold/artcraft/tree/84a6a7dcf5ccd5f8f577e5dafe84494532cc14fb) (MIT or Apache-2.0, at the user's option), its shared house rules in [storytold/craft-contrib](https://github.com/storytold/craft-contrib/tree/666a6dcb5b6b4640a71ca8c395d4f4807e0bbf9a) at `666a6dc` and the shared CI in [storytold/craft-common-actions](https://github.com/storytold/craft-common-actions/tree/293de72429d3ed5d9fa92aded5e500544c0fa28f) at `293de72`, and the earlier reading of their CADCraft app for the kernel ([G01 evidence 12](../../kit/notes/G01-evidence/12-vetting-cadcraft.md)). craft-contrib and craft-common-actions carry no licence file, so they are read for method only and nothing is quoted at length from them.

## What ArtCraft is and how it is laid out

ArtCraft is "a public-facing Tauri desktop app for generating AI images and video", in Rust and TypeScript, with its backend in a separate repository ([AGENTS.md L29-L31](https://github.com/storytold/artcraft/blob/84a6a7dcf5ccd5f8f577e5dafe84494532cc14fb/AGENTS.md#L29-L31)). GitHub counts about 10 MB of Rust and 6.6 MB of TypeScript. Its layout ([AGENTS.md L35-L53](https://github.com/storytold/artcraft/blob/84a6a7dcf5ccd5f8f577e5dafe84494532cc14fb/AGENTS.md#L35-L53)):

- `crates/` grouped by role, not flat: `desktop/` (the app), `api_clients/` (one crate per outside service), `lib/` (small support crates, one job each), `schema/` (database and public wire types), `testing/` and `vendor/` (forks of outside crates, kept apart).
- `frontend/` as its own workspace of apps and libraries.
- `script/`, `test_data/`, `_database/` migrations, and `.sqlx/` query metadata for offline builds.

Its product is far from Spare Construct's, and parts of it, such as clients for consumer services, were not reviewed. What matters here is how its agents are told to work.

## How their agents work

**One rule set for every repository, copied in and checked.** Each Craft repository's `AGENTS.md` starts with a block of house rules between markers that record the source commit and a SHA-256 of the text ([AGENTS.md L1-L21](https://github.com/storytold/artcraft/blob/84a6a7dcf5ccd5f8f577e5dafe84494532cc14fb/AGENTS.md#L1-L21)). The source is `standards/house-rules-core.md` in craft-contrib, and a repository "may be stricter, not weaker" (L4). A shared CI workflow checks out craft-contrib and fails when the copy differs ([house-rules.yml](https://github.com/storytold/artcraft/blob/84a6a7dcf5ccd5f8f577e5dafe84494532cc14fb/.github/workflows/house-rules.yml); craft-common-actions' `house-rules.yml`). `CLAUDE.md` is one line, `@AGENTS.md`, so every agent reads the same file, as Spareware's root `CLAUDE.md` already does.

**The core rules,** in our words (L2-L20):

1. A person is accountable for what an agent opens.
2. Check, then claim: read the issue, search open pull requests and recent commits, and comment on the issue before writing code; release a claim you will not finish.
3. Verify before using a definition or naming a cause.
4. Keep fixes narrow, with a test that fails without the fix.
5. Review the diff before opening a pull request.
6. Close your pull request when another one fixes the same thing as well or better.
7. Report only what ran.
8. `main` changes only through pull requests.
9. Stop and ask only about licences outside the allowlist, publishing, releases, new remotes, secrets, spending, destructive git and anything hard to undo; otherwise keep working.
10. Keep secrets and personal details out.
11. CI is shared; a feature pull request does not change it.
12. A guard says what it prevents.
13. Instructions are readable by everyone, with no notes about one machine.
14. A clock on review feedback: respond within 7 days, a reminder at 14, closure possible at 30, counted only while an actionable request waits on the author.
15. No agent attribution in pull requests.

**Rust-specific rules** (craft-contrib `standards/house-rules-rust.md`): never crash (production crate roots deny `unwrap_used`, `expect_used`, `panic`, `unimplemented`, `todo` and `unreachable`, and each crash fix comes with a test that panicked before it); clean room (learn from behaviour and public specifications, copy no product's code, assets, strings or artwork); every asset recorded in an attribution file in the same change; and an SPDX licence allowlist that a repository may shorten but not extend.

**Rules split by topic, and by folder.** `.claude/rules/` holds `api-conventions.md`, `code-style.md` and `testing.md`, and subsystems carry their own `AGENTS.md` (for example for the app's command modules, linked from [AGENTS.md L69](https://github.com/storytold/artcraft/blob/84a6a7dcf5ccd5f8f577e5dafe84494532cc14fb/AGENTS.md#L69)). A style rule that agents need and people forget is written down, such as file layout read top to bottom and space-padded Markdown tables (L71-L98).

**From CADCraft** (the kernel's reading, [evidence 12](../../kit/notes/G01-evidence/12-vetting-cadcraft.md)): crate layers enforced in CI by `cargo xtask layers`, one command registry shared by the interface, the command line and MCP, the never-crash discipline above, hostile-input tests and the project's own honest parity audits. The same reading found CADCraft's chain of title in doubt (it was built by observing AutoCAD), which is why the kernel takes its practices only, never its code or data (kernel [29](../../engines/geometry/architecture/29-sources-licences.md), CADCraft row).

## Against what Spareware already does

| ArtCraft practice | Spareware today | Gap |
| --- | --- | --- |
| Check, then claim, by issue comment | `tools/piece.py checkout`: a branch name as a lock, enforced on every pull request (AGENTS.md, 3) | None; ours is stronger |
| Report only what ran | Every pull request says how it was checked (AGENTS.md, 6) | None |
| Never-crash lints at crate roots | The kernel's crates deny unwrap, expect, panic and unchecked indexing in each `clippy.toml` ([geometry AGENTS.md](../../engines/geometry/AGENTS.md)) | Spare Construct's crates and server do not (none of their `lib.rs` files or manifests set these lints, checked at main `21d1067`) |
| Rules per subsystem | The kernel has its own `AGENTS.md` | Spare Construct has none; the map's Community section says each repo carries its build and test commands |
| Guards say why | Workflow files explain each check at length | None |
| Licence allowlist | Kernel 29's allow-list; the server's `deny.toml` | None for crates; assets have no attribution file rule (*unverified* beyond the kernel) |
| Review clock | Pieces idle for 7 days are released (AGENTS.md, 3) | Nothing on how long review feedback may wait |
| No agent attribution | Commits and pull requests name the agent | A different choice, not a gap |
| One rule block synced across repositories | One monorepo, one AGENTS.md | Not needed while Spareware is one repository |

## Recommendation

1. **Add a Spare Construct `AGENTS.md`** under `projects/spare-construct/`, as the kernel has: what to read first, the crates and what each owns, the commands to build and test the server, the app and the WebAssembly package (`server/scripts/build-shapes.sh`), and the rules particular to the program (the stand-in geometry line in `construct-regen`, the app's demo page served unchanged). It adds to the root file, never weakens it. This is a docs piece that waits on nothing.
2. **Adopt never-crash lints for Spare Construct's crates and server,** with the kernel's wording: deny `unwrap_used`, `expect_used`, `panic`, `unimplemented`, `todo` and `unreachable` outside tests, and every crash fix with a test that panicked before it. It is a code piece of its own, because existing code will need changes to pass.
3. **Write down the rules agents keep getting wrong, by topic,** in that file rather than in reviews: for Spare Construct, the plain-words rule for interface text, design tokens for every colour, the clean-room rule for Revit-like interface structure (AGENTS.md, 5), and how notes cite sources.
4. **Propose a review clock to the owners:** a time after which an unanswered actionable review comment lets an owner close the pull request, counted only while it waits on the author, in the manner of ArtCraft's rule 14. It changes the root `AGENTS.md`, so an owner decides.
5. **Do not copy** the shared-block mechanism while Spareware is one repository, the "no agent attribution" rule (Spareware's own practice differs and is the owners' choice), or anything from CADCraft beyond its practices, as the kernel already rules.

## Open risks

- **Rules files grow until nobody reads them.** ArtCraft keeps its root file short and pushes detail into topic and folder files; Spare Construct's file should do the same.
- **Never-crash lints cost a pass over existing code.** The server and `construct-api` are about 13,000 lines; how many `unwrap` calls they hold was not counted here.
- **The sources move.** craft-contrib's rules change by pull request in their repository; links here are pinned.

## Sources

| Source | Licence | Used for |
| --- | --- | --- |
| [ArtCraft](https://github.com/storytold/artcraft/tree/84a6a7dcf5ccd5f8f577e5dafe84494532cc14fb) | MIT or Apache-2.0 | Layout, AGENTS.md, topic rules, CI |
| [craft-contrib](https://github.com/storytold/craft-contrib/tree/666a6dcb5b6b4640a71ca8c395d4f4807e0bbf9a) | No licence file; method only | Shared core and Rust house rules |
| [craft-common-actions](https://github.com/storytold/craft-common-actions/tree/293de72429d3ed5d9fa92aded5e500544c0fa28f) | No licence file; method only | The check that keeps the copies in step |
| [G01 evidence 12](../../kit/notes/G01-evidence/12-vetting-cadcraft.md) | This repo | CADCraft practices and chain of title |
