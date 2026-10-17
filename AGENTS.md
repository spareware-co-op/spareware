# Instructions for agents

You are working on Spareware with a contributor's spare tokens. Your job: show them where things stand, let them choose a piece of work that fits their tokens, do it to the standard below, and leave it so the next agent can carry on.

## 0. Check the model first

The minimum for building Spareware is **Claude Opus 5.5 at Extra high effort**. A newer, more capable model is also allowed.

- If you are a smaller or older model than Claude Opus 5.5 (for example a Sonnet or Haiku model, or an earlier Opus), stop and tell the contributor. Do no work.
- Ask the contributor to confirm the effort level is Extra high or Max. If it is lower, ask them to switch before you start.

## 1. Show where every project is

Read [STATUS.md](STATUS.md). Tell the contributor, in a short table, each project, its stage and how many pieces are open.

| Stage | What it means | What the work looks like |
| --- | --- | --- |
| Idea | Named, not mapped yet | Research and a first architecture map |
| Mapping | The whole architecture is being mapped and decisions settled | Research notes and decision notes; no product code |
| Design | Each part gets a short design note naming the proven method to follow | Design notes and small spikes that prove a method |
| Build | Code is written against agreed design notes | Code with tests and benchmarks |
| Harden | Speed, robustness and truth tests | Benchmarks, fuzzing, fixes |
| Live | In use | Fixes and add-ins |

## 2. Offer options

1. Ask which project they want, or recommend the one with the most open work.
2. Read the root [ARCHITECTURE.md](ARCHITECTURE.md) (how all the programs fit together), then that project's `projects/<name>/STATUS.md` and `ARCHITECTURE.md`. The shared kit's own map is `projects/kit/ARCHITECTURE.md`.
3. Ask roughly how many tokens they have left: a little, a fair amount, or a lot. These match the Small, Medium and Large sizes on the work board.
4. Offer 3 to 5 open pieces that fit the stage and their size. Give each one line on what it is and what "done" looks like.
5. Let them choose. Never start a piece they have not picked.

## 3. Claim it

Claim the piece before starting so two agents never do the same work. On GitHub, comment on the issue carrying the piece ID (for example A01) and assign the contributor. If someone has claimed it and worked on it in the last 7 days, offer another piece.

## 4. Do the work

**Mapping and Design stages:** write the note at `projects/<name>/notes/<ID>-<slug>.md` with: the question, the options, the evidence, a recommendation and the open risks. Evidence means reading the actual source of the projects you cite, not only their docs. Link every source.

**Build and Harden stages:** code, tests and a benchmark, following the project's `ARCHITECTURE.md`. Code goes in the matching crate under `projects/<name>/crates/` (each crate's `lib.rs` says what it is for). Keep the layering rule in the root `ARCHITECTURE.md`: a program never depends on another program; shared needs move into `projects/kit` or `projects/engines`.

**Always:**

- Read how the best open project does it before designing; name the method you are following.
- Research before stating anything you are unsure of, and mark anything unverified.
- Stay inside the piece. If something else looks broken, note it on the pull request; do not fix it.
- Work on a branch named `<ID>-<slug>`. Never push to `main`.

## 5. Stay legal (no exceptions)

- Never copy proprietary code, assets, icons, artwork, documentation or interface designs from any company's software, including from memory.
- Never decompile, disassemble or reverse-engineer proprietary software. Never use leaked or stolen source code.
- Use open-source code only when its licence allows it. Keep its licence and credit, and list its source in the pull request. No GPL or AGPL code in the core.
- Our products use their own names. Never use another company's product name for ours.
- If a method might be patented, a data source's terms forbid our use, or a file format can only be read with inside knowledge, stop and raise it with an owner.
- If you are unsure whether something is allowed, stop and ask the contributor to raise it. Do not guess.

## 6. Finish or hand over

**Done:** open a pull request with the piece ID, what changed, how it was checked, and every outside source used. Set the piece's row on the work board to "In review".

**Tokens running low:** stop at a clean point, commit, and open a draft pull request with a **Handover** section: what is done, what is left, the next step, the files touched and anything learned. Set the row to "Handed over".

## 7. Never

- Merge or approve pull requests.
- Change `ARCHITECTURE.md` decisions without a decision note an owner has accepted.
- Change another project's work board.
- Put secrets, personal data or client project data in the repo.
