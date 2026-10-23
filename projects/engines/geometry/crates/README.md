# Kernel crates

One folder per crate, folder name = crate name (chapter 27 of the kernel architecture,
`projects/engines/geometry/architecture/27-crates-folders.md`). Each crate's `src/lib.rs` opens with
its one job, its layer, the invariants it keeps, the sections it implements and its status, and
names the build pieces that fill it in.

The crate table lives in one place: `projects/engines/crates/engine-geometry/tests/scaffold/table.rs`.
It holds every crate's layer and exactly its direct kernel dependencies, the edges allowed inside a
layer, the dependencies still waiting for a crate under way, the dependency behind the facade's
`threads` feature, and the one registry crate a `no_std` kernel crate may name (`libm`, in
`geom-num` only; 29.1, 21). The scaffold test fails when a `Cargo.toml` differs from the table, an
edge breaks the layer rule, the graph has a cycle, a kernel crate is not `no_std` or does not
forbid unsafe code, or a header leaves out its job, layer, invariants or sections in the grammar of
`tests/scaffold/header.rs`. It also fails when `rust-toolchain.toml`, `kernel.yml` and
`kernel-build.yml` do not pin one compiler. Run it with
`cargo test -p engine-geometry --test scaffold`.

CI runs it in `.github/workflows/kernel-build.yml`, which also builds every kernel crate on its own
natively, for `wasm32-unknown-unknown` and for `wasm32-wasip1` with warnings denied; builds every
`no_std` kernel crate for `wasm32v1-none`, a target with no `std` at all, which is what proves
`no_std` (the scaffold test reads text; the build cannot be fooled); and runs `rustfmt` and
`clippy -D warnings` over them. The workflow takes its list of crates from the table, through the
scaffold test's ignored `kernel_build_list`, so it holds no copy of the table.

## The table is the proposed one

G09 depends on G08, the owner's decision on chapter 27's crate table, and G08 is not recorded yet.
So the table here is chapter 27's proposed crate table with chapter 30.5's amendments (`geom-make`,
and the `geom-p21`, `geom-exchange` and `geom-heal` edges), not only G01's accepted rows: its
dependency lists are the proposed ones. Crates that only the amendments define say so in their
headers ("Proposed in chapter 27" or, for `geom-make`, "Proposed in chapter 30", pending G08). When
the owner decides, the table, chapter 27 and these crates change in one pull request.

## Crates not here, and why

| Crates | Layer | Why not yet | Built by |
| --- | --- | --- | --- |
| `geom-math` | 2 | Under way in another piece | G03 |
| `geom-run` | 1 | Under way in another piece | G04 |
| `geom-bench` | Development | Under way in another piece | G06 |
| `geom-test`, `geom-fuzz`, `geom-gen` | Development | Under way in another piece | G11 |
| `geom-sheet`, `geom-thread`, `geom-board`, `geom-stair`, `geom-civil`, `geom-assembly` | 11 | Domain toolkits enter the kernel only when a second program needs them (section 26.1); until then each lives in the program that does | G472, G258, G473, G281, G378, G479 |
| `geom-gerber`, `geom-ipc2581` | 12 | Board exchange follows the same two-program rule (section 24.11, and chapter 27's toolkit rule), and waits for the owner's terms check (G447) | G474, G475 |
| `engine-geometry-c`, `engine-geometry-wasm`, `engine-geometry-py` | 13 | Binding crates beside the facade in `projects/engines/crates/`, built in phase 13 | G453, G454, G455 |

## When a crate under way merges

A crate written in another piece is checked like any other once it is on the branch, so its pull
request meets the scaffold check whichever merges first:

- Its header states the four fields in the interim grammar of `tests/scaffold/header.rs`: a
  sentence opening "One job: " (or "Job: ", "Job (", "Its one job is "), one opening "Layer: " with
  its layer from chapter 27's layer table, a paragraph opening "Invariants" followed by a "- " list,
  and a sentence opening "Sections: " or "Map: " that names a chapter. Each is a line or two in its
  own `lib.rs`.
- Its `Cargo.toml` names exactly its row of the table.
- The crates that wait for it gain it. `geom-run` (G04) and `geom-math` (G03) are not on this branch
  yet, so the crates that depend on them cannot name them as path dependencies: each lists them in
  its header under "Direct dependencies still to add when ... merges", and the table's `PENDING`
  list holds the same. When one merges, the scaffold test fails until the path dependency is added
  to every crate waiting for it, its `PENDING` entry is deleted and those header lines are deleted.
  `geom-pool`, scaffolded here because G04 built only `geom-run`, waits for `geom-run` the same
  way; `engine-geometry` names it behind its `threads` feature, off by default.

The last step touches files outside the merging piece (43 manifests for `geom-run`, `geom-num`'s
among them, and 26 for `geom-math`), which AGENTS.md's "stay inside the piece" does not foresee. It is a mechanical edit the
check spells out crate by crate. If G09 merges first, the owner either allows that edit in the G03
and G04 pull requests or merges them first and has G09 rebase and wire them itself; the check
holds either way.

## The toolchain

`rust-toolchain.toml` pins Rust 1.99.0 with rustfmt, clippy and the two WebAssembly targets. The
first cargo or rustup command inside the repository makes rustup install that toolchain, once, if it
is missing.
