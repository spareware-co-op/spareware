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
2. Read the root [ARCHITECTURE.md](ARCHITECTURE.md) (how all the programs fit together), then that project's `projects/<name>/STATUS.md` and `ARCHITECTURE.md`. Spare Kit's own map is `projects/kit/ARCHITECTURE.md`.
3. Ask roughly how many tokens they have left: a little, a fair amount, or a lot. These match the Small, Medium and Large sizes on the work board.
4. Offer 3 to 5 open pieces that fit the stage and their size. Give each one line on what it is and what "done" looks like.
5. Let them choose. Never start a piece they have not picked.

## 3. Check it out (so nobody works over anyone else)

Every piece has a GitHub issue titled `<ID>: <piece>`. The lock is the branch `piece/<ID>`: GitHub lets only one person create a branch name, so two agents can never hold the same piece. Use the tool; it needs the GitHub CLI (`gh`) signed in as the contributor.

```bash
python tools/piece.py list                 # open pieces (add a program, e.g. spare-construct)
python tools/piece.py status A01           # who has it and its state
python tools/piece.py checkout A01         # check it out, or take over a handed-over piece
git fetch origin && git switch piece/A01   # all work for the piece happens on this branch
```

1. **Check out before touching anything.** If `checkout` says someone has it, pick another piece. Never work on a piece you haven't checked out.
2. **Only work inside your piece.** If you need a part of a big piece, split it first: `python tools/piece.py split A01 "Part title"` makes `A01a`, which you then check out on its own.
3. **Finished:** open a pull request from `piece/<ID>` that says `Closes #<issue number>`, then `python tools/piece.py release <ID> --review`.
4. **Out of tokens:** commit and push to `piece/<ID>`, open a draft pull request with a **Handover** section, then `python tools/piece.py release <ID> --handover`. The next agent's `checkout` takes over from your branch.
5. **Changed your mind before doing anything:** `python tools/piece.py release <ID>`.

GitHub enforces it: a pull request from `piece/<ID>` fails its check unless its author has that piece checked out. Pieces idle for 7 days are released automatically, and any work stays on the branch.

Who has what, at a glance: [checked out](https://github.com/spareware-co-op/spareware/issues?q=is%3Aissue+is%3Aopen+label%3Aclaimed) · [open to take](https://github.com/spareware-co-op/spareware/issues?q=is%3Aissue+is%3Aopen+label%3Apiece+-label%3Aclaimed+-label%3A%22in+review%22) · [in review](https://github.com/spareware-co-op/spareware/issues?q=is%3Aissue+is%3Aopen+label%3A%22in+review%22) · [handed over](https://github.com/spareware-co-op/spareware/issues?q=is%3Aissue+is%3Aopen+label%3A%22handed+over%22)

## 4. Do the work

**Mapping and Design stages:** write the note at `projects/<name>/notes/<ID>-<slug>.md` with: the question, the options, the evidence, a recommendation and the open risks. Evidence means reading the actual source of the projects you cite, not only their docs. Link every source.

**Build and Harden stages:** code, tests and a benchmark, following the project's `ARCHITECTURE.md`. Any interface follows [design/DESIGN.md](design/DESIGN.md) and uses [design/tokens.css](design/tokens.css) for every colour, font and spacing. Never change the shared look itself unless a member vote agreed it, or a unique situation truly needs it; then say why in the pull request. Code goes in the matching crate under `projects/<name>/crates/` (each crate's `lib.rs` says what it is for). Keep the layering rule in the root `ARCHITECTURE.md`: a program never depends on another program; shared needs move into `projects/kit` or `projects/engines`.

**Always:**

- Always start with "Can we": when something looks unsupported or hard, answer with how it could be done and what it would take, never just "it doesn't" or "we can't". Only the legal rules in step 5 are a hard no.
- Read how the best open project does it before designing; name the method you are following.
- Research before stating anything you are unsure of, and mark anything unverified.
- Stay inside the piece. If something else looks broken, note it on the pull request; do not fix it.
- Work only on your piece's branch, `piece/<ID>`. Never push to `main`.

## 5. Stay legal (no exceptions)

- Never copy proprietary code, assets, icons, artwork, documentation or interface designs from any company's software, including from memory.
- Never decompile, disassemble or reverse-engineer proprietary software. Never use leaked or stolen source code.
- Use open-source code only when its licence allows it. Keep its licence and credit, and list its source in the pull request. No GPL or AGPL code in the core.
- Our products use their own names. Never use another company's product name for ours.
- If a method might be patented, a data source's terms forbid our use, or a file format can only be read with inside knowledge, stop and raise it with an owner.
- If you are unsure whether something is allowed, stop and ask the contributor to raise it. Do not guess.

## 6. Finish or hand over

**Every commit is signed off** with the Developer Certificate of Origin: use `git commit -s`. The contributor keeps the copyright; the work is licensed MPL-2.0 (core) or Apache-2.0 (SDK and plugins), and Spareware is never relicensed.

**Done:** a pull request from `piece/<ID>` with the piece ID, what changed, how it was checked, every outside source used, and `Closes #<issue number>`. Then `python tools/piece.py release <ID> --review`.

**Tokens running low:** stop at a clean point, commit and push to `piece/<ID>`, open a draft pull request with a **Handover** section (what is done, what is left, the next step, files touched, anything learned), then `python tools/piece.py release <ID> --handover`.

## 7. Never

- Merge or approve pull requests.
- Change `ARCHITECTURE.md` decisions without a decision note an owner has accepted.
- Change another project's work board.
- Put secrets, personal data or client project data in the repo.
