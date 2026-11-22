# CON-A3-F1-P1 (used), CON-A2-F9-P4 (in part): the geometry kernel's engine in the browser

Part of CON-A3-F1-P1 (the solid kernel, as Spare Construct uses it through the engine seam) and a first piece of CON-A2-F9-P4 (the same results natively and in the browser) ([FEATURES.md](../FEATURES.md)). Piece A51. Status: built (`server/shapes-kernel`, `server/scripts/build-shapes-kernel.sh`, `app/connect.js`, the checklist's kernel job); the server's default builder is still the stand-in.

## The question

The server can build a document's shapes with the kernel builder over solidcore's engine (`STACK_GEOMETRY=kernel`), but the page could only build the stand-in's boxes, so a document made on the kernel was a viewer in the browser. The page must build those shapes too, with the same bytes as the server, behind the interface it already reads. Where does that package live, is it committed, and how does the page choose it?

## Decisions

1. **A package of its own, in its own workspace.** `projects/spare-construct/server/shapes-kernel` (crate `construct-shapes-kernel`) links construct-regen's `kernel::Kernel` and geom-solidcore's `SolidcoreEngine`, made as the server's `src/geometry.rs` makes them, and compiles to WebAssembly. It cannot be a root-workspace crate beside `construct-shapes`: solidcore never enters the root workspace (decision 32.70). It is not a member of the server's workspace either: it needs wasm-bindgen at 0.2.118, the version `construct-shapes` and wasm-bindgen-cli are pinned to, and the server's lock holds wasm-bindgen at 0.2.130 for its own browser-only dependencies, so joining would move existing pins. So it is its own workspace, as geom-solidcore is, with its own Cargo.lock.
2. **The server's versions, kept.** Its lock was started from a copy of the server's, so every crate both build natively, solidcore and its tree among them, is at the server's version; only wasm-bindgen's four crates differ, which the server never builds natively. `tests/package.rs` checks it on every run (`every_crate_the_server_builds_is_at_the_servers_version`): the browser builds with the very engine the server's `kernel` feature does.
3. **The same interface as the stand-in's package.** `edition()` and `build(recipe)`, JSON in and JSON out, byte for byte the answers `construct-shapes` gives for the same builder (a test runs both on the stand-in). One function more, `build_hosting(host, hosted)`, cuts a wall's doors as the kernel builder's `build_hosting` does, each door placed by the difference of the two placements; the page does not call it until the hosting job hands a wall its doors (CON-A4-F3-P1, the interim register's row).
4. **The same bytes, shown without a browser.** `examples/answers.rs` prints, natively, the edition and the answer to each recipe of `tests/cases.json` (a wall, a wall with two doors, a block); `tests/browser/same_bytes.mjs` loads the very package the page loads in Node, builds the same recipes through the page's functions, and compares every line byte for byte. The answers carry every display mesh, so this catches a float or an order that differs between native code and WebAssembly, which the page's check against the server's checkpoints cannot: a body's name is made of what the engine was asked, not of what it gave (the interim register's row). The comparison is never loosened: a difference is found at its cause.
5. **How the page chooses.** By the engine a document's edition names: `construct.kernel` loads `/shapes/kernel/construct_shapes_kernel.js`, anything else the stand-in's package, as before. The stand-in's loads with the page; the kernel's, far larger, the first time a document names it. A package builds a document only when its edition is the document's in every field, the engine's report included (construct-model's FORMAT.md, "A document"); otherwise, or when the package can't load, the page is a viewer of the stored shapes, as today.
6. **Built by the checks, not committed.** `app/pkg` is committed so the server builds without a WebAssembly toolchain. That reason is the default build's, and it still holds: the kernel's package is needed only by a server built with the `kernel` feature, which already takes solidcore's long build. And the package is large: about 29 MB, 7.2 MB gzipped, against the stand-in's 192 KB (figures below), which every change to the engine or the builder would add again to the repository's history. So it is built by `scripts/build-shapes-kernel.sh` into `app/pkg-kernel` (ignored by git), and a server built with `kernel` serves it from there at run time (the `shapes_kernel` setting, `STACK_SHAPES_KERNEL`), so that build needs no package at compile time. A default server does not serve it: it can't build a kernel document, so neither may its page.
7. **8 MiB of stack.** solidcore's README asks for it, and geom-solidcore's `.cargo/config.toml` sets it; the package's own does the same, for its native tests and, at link time, for the browser's build.

## The figures (23 Nov 2026, a release build, 4 build jobs)

| | Stand-in (`app/pkg`) | Kernel (`app/pkg-kernel`) |
| --- | --- | --- |
| `.wasm`, raw | 192,079 bytes | 29,035,575 bytes |
| `.wasm`, gzipped (`gzip -9`) | 71,962 bytes | 7,214,179 bytes |
| `.js`, raw | 7,029 bytes | 8,250 bytes |
| Release build from clean | | 3 min 26 s (the wasm-bindgen step included) |

That is large for a web page, 100 times the stand-in's download, so the page loads it only for a document on the kernel. In Node on a desktop it loads in about 0.2 s and builds a wall in milliseconds; a page on a slow line waits for the 7.2 MB first. What is large, by section (read from the file's own sections and its name section): code 24.6 MB (85%), the function names 3.9 MB (13%), data 0.5 MB. Of the code, solidcore is 20.4 MB (83%), Rust's own library 2.4 MB, and the kernel's own crates (geom-poly, geom-bool, geom-check, geom-topo and the rest) about 1 MB. Much of solidcore's share is one generic function compiled once per type it is used with (`refine_large_gap`, about 27 KB a copy).

What could be cut, none of it cut here: the name section (3.9 MB), by stripping symbols in the release build; a build optimised for size, with link-time optimisation and one code unit (not measured here); and most of solidcore, which is linked because `SolidcoreEngine::extrude` falls back to solidcore's linear sweep (through geom-solidcore's `Modeller::extrude`) for a profile make.prism does not take, though every wall, door and block today is a level rectangle that make.prism answers. That last cut changes the engine's edition, so it is a change to the engine on both sides, server and browser, not to this package.

## The default

The server's default builder stays the stand-in until the checklist's kernel job is green on main. The switch is the default of the `geometry` setting, `default_value_t = Builder::StandIn` in `server/src/settings.rs`, to `Builder::Kernel`; a server built without the `kernel` feature then refuses to start, so the same change makes the feature a default one (`default = ["kernel"]` in the server's `[features]`), and the deployment then builds the kernel's package beside the server.

## Not done here

- The page builds each element alone: a wall it builds has no openings until the hosting job (CON-A4-F3-P1).
- Nothing is cut from the package yet (above).
- The determinism checks of CON-A2-F9-P4 proper (a whole log replayed natively and in the browser) are job 13.5; this covers the recipes of `tests/cases.json`.
