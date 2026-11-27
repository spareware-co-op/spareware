# A08: Plugins: WebAssembly components on wasmtime

## The question

Add-ins run as sandboxed WebAssembly components with declared permissions; they can add commands, panels, element types and checks, and a faulty plugin cannot crash or corrupt the model ([ARCHITECTURE.md](../ARCHITECTURE.md), Headless API, AI and add-ins). The host is wasmtime natively and the browser's own engine on the web, and the interface is written in WIT, the component model's interface language ([EXTENDING.md](../../../EXTENDING.md), 2.3). The map says Rust is strongest today and C# and Go "still maturing", and asks: "Is WebAssembly-only too strict for plugins?" The piece asks: what can plugin authors in Rust, C#, TypeScript and Python use today?

Sources were read on 12 November 2026, from each project's repository at the commit named: wasmtime at [`266d794`](https://github.com/bytecodealliance/wasmtime/tree/266d7946f7783be62864cad5e553764b4c0a0f58) (52.0.0 in development; latest release 50.0.1), wit-bindgen at [`4db5baa`](https://github.com/bytecodealliance/wit-bindgen/tree/4db5baa5b99c432508042f4887cdc6ee167523a7) (latest 0.63.0), jco at [`e401805`](https://github.com/bytecodealliance/jco/tree/e40180510a1c0ed22f41e1febd5527c4f99e604c) (1.41.0), ComponentizeJS at [`7df33dd`](https://github.com/bytecodealliance/ComponentizeJS/tree/7df33ddcdaa5f598027962bd876939d36b1552de) (0.23.0), componentize-py at [`5382383`](https://github.com/bytecodealliance/componentize-py/tree/5382383be90e05b75c88d327de83be5c347379eb) (0.25.1), componentize-dotnet at [`f6bfc5a`](https://github.com/bytecodealliance/componentize-dotnet/tree/f6bfc5a9a5a25728aeec959be60c8f737278a183) (no release), cargo-component at [`1495f61`](https://github.com/bytecodealliance/cargo-component/tree/1495f6141420528dfb745dd7da94a618e58d84b2), and the [WASI releases](https://github.com/WebAssembly/WASI/releases). Every project named here is Apache-2.0 as GitHub reports it; wasmtime's licence file adds the LLVM exception. This note reads READMEs and wasmtime's configuration source; no plugin was built for it, so sizes and limits quoted are the projects' own, marked where not checked.

## What a plugin has to do

From the map, EXTENDING.md and features:

- **Add** commands, panels, element types and checks, and subscribe to changes (Headless API; EXTENDING 2.3).
- **Get nothing it did not declare.** Permissions such as read, edit or network are shown in plain words before install; the sandbox starts with no access (EXTENDING 2.3).
- **Never crash, hang or corrupt the model.** A plugin reaches the model only through the command API (EXTENDING, "Plugins reach a program only through its command API or plugin interface"), and a command runs in a transaction that applies whole or not at all (Regeneration and constraints).
- **Run where people work,** which is the browser ([K08](../../kit/notes/K08-cloud-foundation.md)), and on the server for headless and agent work.
- **Stay stable.** A published plugin uses only the stable interface, which follows semantic versioning (EXTENDING 2.3 and 4).
- **Stay deterministic where it feeds the model.** Commands a plugin issues go into the operation log like any other, so replay never re-runs the plugin. But a plugin-defined element type whose geometry the plugin builds is in the regeneration graph, and then the plugin's code must give the same bits on every machine (ground rule 6).

## What each language can use today

| Language | How a component is made | State | Runs in the browser through |
| --- | --- | --- | --- |
| Rust | `wit-bindgen` and the upstream `wasm32-wasip2` target, with plain `cargo build` | Mature; developed in the wit-bindgen repo | jco transpile |
| C# | NativeAOT-LLVM from .NET's experimental package feed, with wit-bindgen's C# generator or componentize-dotnet | Experimental; prerelease packages | jco transpile (*unverified* for .NET output) |
| TypeScript and JavaScript | ComponentizeJS (through `jco componentize`): the SpiderMonkey engine embedded in each component | Experimental, by its own README; about 8 MB of engine per component | jco transpile |
| Python | componentize-py: CPython embedded in the component | Working, with known limits; native packages only from unofficial WASI builds | jco transpile (*unverified* for its size) |
| Go, C, C++, MoonBit | wit-bindgen's own generators | C and C++ mature; Go and MoonBit newer | jco transpile |

### Rust

wit-bindgen's repository develops bindings for "Rust, C, C++, C#, and Go" ([README L32-L35](https://github.com/bytecodealliance/wit-bindgen/blob/4db5baa5b99c432508042f4887cdc6ee167523a7/README.md#L32-L35)). A Rust component is a crate with `cargo add wit-bindgen`, the WIT files in a `wit/` folder, a `generate!` call and a `Guest` trait implementation, built with `cargo build --target wasm32-wasip2` ([L189-L250](https://github.com/bytecodealliance/wit-bindgen/blob/4db5baa5b99c432508042f4887cdc6ee167523a7/README.md#L189-L250)). The `wasm32-wasip2` target has been in upstream Rust since 1.82 ([cargo-component README L186-L190](https://github.com/bytecodealliance/cargo-component/blob/1495f6141420528dfb745dd7da94a618e58d84b2/README.md#L186-L190)); cargo-component, which older guides use, has not been pushed to since July 2025 and its own README says the plain target is "not a complete replacement" yet (L192-L200), while wit-bindgen's README now builds custom worlds with the plain target. So Rust authors need only cargo and two crates. This is also the language our own first-party plugins are written in, so it is the path we test hardest.

### C\#

C# has two routes, both on the same compiler: wit-bindgen's C# generator (`wit-bindgen csharp -w command -r native-aot`) and componentize-dotnet, a simpler MSBuild package. Both need .NET 10 ([componentize-dotnet README L27](https://github.com/bytecodealliance/componentize-dotnet/blob/f6bfc5a9a5a25728aeec959be60c8f737278a183/README.md#L27)) and the NativeAOT-LLVM compiler from .NET's `dotnet-experimental` package feed, added as prerelease packages ([L270-L293](https://github.com/bytecodealliance/componentize-dotnet/blob/f6bfc5a9a5a25728aeec959be60c8f737278a183/README.md#L270-L293); [wit-bindgen README L317-L365](https://github.com/bytecodealliance/wit-bindgen/blob/4db5baa5b99c432508042f4887cdc6ee167523a7/README.md#L317-L365)). componentize-dotnet has no release and was last pushed to in June 2026. A C# author can write a plugin today, but on an experimental compiler with no support promise. That matters for Spare Construct more than for most: most Revit add-ins are written in C#, so C# is the language many of the people we want to bring across already know (*unverified* as a share).

### TypeScript and JavaScript

ComponentizeJS takes an ES module and a WIT world and outputs a component, by embedding Mozilla's SpiderMonkey engine (through StarlingMonkey) in the component; "the total embedding size is around 8MB" ([README L17-L18, L44](https://github.com/bytecodealliance/ComponentizeJS/blob/7df33ddcdaa5f598027962bd876939d36b1552de/README.md#L17-L44)). It says of itself: "This is an experimental project, no guarantees are provided for stability or support and breaking changes may be made in future" (L20). Exports may be async, but "imported functions can only be synchronous pending component-model-level async support" (L144). TypeScript is compiled to JavaScript first; jco runs both steps ([jco README L30](https://github.com/bytecodealliance/jco/blob/e40180510a1c0ed22f41e1febd5527c4f99e604c/README.md#L30)), and also lists a QuickJS-based alternative (`componentize-qjs`) and an experimental backend. Every TypeScript plugin therefore carries its own JavaScript engine: about 8 MB to download and instantiate per plugin, until the plan to share one engine between components (L48) lands.

### Python

componentize-py takes a WIT world and a Python module and outputs a component that embeds CPython ([README L5-L15](https://github.com/bytecodealliance/componentize-py/blob/5382383be90e05b75c88d327de83be5c347379eb/README.md#L5-L15)). Its known limit is that imports are resolved at build time, so a module must import what it uses at the top level (L111-L137). Packages with native extensions work only when built for WASI: its NumPy example uses "an unofficial build of NumPy since the upstream project does not yet publish WASI builds" ([matrix-math example](https://github.com/bytecodealliance/componentize-py/tree/5382383be90e05b75c88d327de83be5c347379eb/examples/matrix-math)). So pure-Python plugins work today; plugins that need the scientific stack depend on unofficial builds. The component's size, with CPython inside, was not measured here (*unverified*).

### The host, natively

wasmtime runs any WIT world from Rust, with a `bindgen!` macro that generates the host side ([wit-bindgen README L471-L495](https://github.com/bytecodealliance/wit-bindgen/blob/4db5baa5b99c432508042f4887cdc6ee167523a7/README.md#L471-L495)). Its configuration has what the plugin rules need:

- **Deterministic budgets:** fuel "can be used to deterministically prevent infinitely-executing WebAssembly code" ([config.rs L625-L654](https://github.com/bytecodealliance/wasmtime/blob/266d7946f7783be62864cad5e553764b4c0a0f58/crates/wasmtime/src/config.rs#L625-L654)), the same idea as the kernel's work units (kernel 23.10). Fuel used is not exact after a trap, so a store is not reused after one.
- **Deadlines:** epoch interruption, which wasmtime measured at up to 2 to 3 times faster than fuel ([L770-L799](https://github.com/bytecodealliance/wasmtime/blob/266d7946f7783be62864cad5e553764b4c0a0f58/crates/wasmtime/src/config.rs#L770-L799)). Like the kernel's `Cancel`, it depends on time, so it may stop a plugin but never decide a stored result.
- **Memory:** `StoreLimits` caps a store's memories and tables ([limits.rs L318-L325](https://github.com/bytecodealliance/wasmtime/blob/266d7946f7783be62864cad5e553764b4c0a0f58/crates/wasmtime/src/runtime/limits.rs#L318-L325)).
- **Same bits on every machine:** NaN canonicalisation replaces NaNs "with a single canonical value. This is useful for users requiring entirely deterministic WebAssembly computation" ([L1745-L1762](https://github.com/bytecodealliance/wasmtime/blob/266d7946f7783be62864cad5e553764b4c0a0f58/crates/wasmtime/src/config.rs#L1745-L1762)), and `relaxed_simd_deterministic` fixes the relaxed SIMD instructions, which otherwise "may execute differently on one host than another" ([L1265-L1287](https://github.com/bytecodealliance/wasmtime/blob/266d7946f7783be62864cad5e553764b4c0a0f58/crates/wasmtime/src/config.rs#L1265-L1287)). Both are off by default.
- **Async:** the component model's async ABI, streams and futures sit behind `wasm_component_model_async` ([L1395-L1418](https://github.com/bytecodealliance/wasmtime/blob/266d7946f7783be62864cad5e553764b4c0a0f58/crates/wasmtime/src/config.rs#L1395-L1418)). WASI 0.3, which brings async, was released in June 2026 (0.3.0) and is at 0.3.2 (October 2026); WASI 0.2 continues alongside it (0.2.12, June 2026), from the WASI releases page.

### The host, in the browser

No browser runs components directly. jco "transpiles" a component into ES modules that use the browser's own WebAssembly engine for the core modules, with generated JavaScript glue for the component's interfaces ([jco README L31](https://github.com/bytecodealliance/jco/blob/e40180510a1c0ed22f41e1febd5527c4f99e604c/README.md#L31); [wit-bindgen README L487-L493](https://github.com/bytecodealliance/wit-bindgen/blob/4db5baa5b99c432508042f4887cdc6ee167523a7/README.md#L487-L493)). Its WASI 0.2 shim works in Node and browsers; its WASI 0.3 shim is Node only ([jco README L56-L57](https://github.com/bytecodealliance/jco/blob/e40180510a1c0ed22f41e1febd5527c4f99e604c/README.md#L56-L57)). So in the browser today a plugin is a WASI 0.2 component, transpiled once (at publish time, by the directory, so the browser never runs a compiler), and best run in its own Web Worker so a busy plugin cannot freeze the page. Browser engines have no fuel, so a deterministic budget in the browser needs metering written into the module before it is published (*unverified* tooling; a Design question), and NaN bits are not canonicalised by browsers, so plugin code that feeds geometry must avoid depending on NaN payloads (*derived*).

### Is WebAssembly-only too strict?

For code that runs inside the app, no: it is the only way a person's browser can run a stranger's code safely, and every language above can target it. But the map also has a second path that needs no WebAssembly: scripts and programs that drive the model from outside through the command API, over JSON-RPC, with the C interface for C# and Python, and the command line (Headless API, bindings). An external script runs on the author's own machine or server with the author's credentials and the same per-project permissions; it cannot crash the app because it is not in it. That path serves C# and Python authors today, with their full ecosystems (NuGet, NumPy, pandas), while their WebAssembly toolchains mature.

## Recommendation

1. **Keep WebAssembly components as the only in-app plugin form,** as EXTENDING.md says, and name the second path plainly: **external scripts** over the command API (JSON-RPC, the C interface, the command line) for anything that needs a native ecosystem or runs outside the app. Both go through the same commands and the same permission checks, so an external script can do nothing a plugin with the same permissions could not.
2. **One WIT world, versioned,** for example `spareware:construct/plugin@1.0.0`, generated from the command schema (KIT-A15-F1-P3) so plugins, TypeScript types and MCP tools never drift apart. Target WASI 0.2 for the first stable world, because jco's browser shim is 0.2; move to 0.3's async when jco runs it in browsers.
3. **Tell authors what is ready, per language, in our own docs:**
   - **Rust: supported.** wit-bindgen on `wasm32-wasip2`; our first-party plugins are the examples.
   - **TypeScript: supported, with its cost stated.** ComponentizeJS through jco; about 8 MB of engine per plugin until engines are shared. Experimental by its own words, so pin versions.
   - **Python: supported for pure Python.** componentize-py; native packages only where WASI builds exist.
   - **C#: preview.** NativeAOT-LLVM is experimental; meanwhile C# authors use external scripts through the C interface or JSON-RPC, which need no WebAssembly at all.
4. **Host rules, natively (wasmtime) and in the browser (jco):**
   - Fuel for every call that can affect the model, so a budget fails the same way on every machine; epochs for deadlines; `StoreLimits` for memory; a fresh store after any trap.
   - For plugins that build geometry: NaN canonicalisation and deterministic relaxed SIMD on, no clock and no randomness imported (a world without `wasi:clocks` and `wasi:random`), so their results can be cached and compared like the kernel's (kernel 23.8). In the browser, metering injected at publish time; to settle in Design.
   - One plugin per store natively and per Web Worker in the browser; a plugin's commands go through the same transaction as the person's, so a failure changes nothing.
   - Panels: a plugin describes its panel as data the app draws with its own components, so plugin UI follows `design/DESIGN.md` and needs no access to the page; a sandboxed frame for free-form HTML is a later question.
5. **Measure before the first stable release:** component size, instantiation time and call overhead for a hello-world plugin in each of the four languages, native and transpiled in the browser; this note quotes the projects' own figures only.

## Open risks

- **Three of the four language toolchains call themselves experimental or lack releases.** Pin versions, test each in CI against the stable world, and say plainly in the docs which are supported.
- **8 MB per TypeScript plugin** adds up in a browser that loads several. Shared engines are planned upstream (ComponentizeJS README L48), not shipped.
- **No fuel in the browser.** Until metering is injected at publish time, a plugin's budget in the browser is a deadline, not deterministic; geometry from plugins then cannot be cached across machines with the same guarantee as the kernel's.
- **jco is the browser's only component host,** and its WASI 0.3 support is Node only today. If it stalls, the browser plugin path stalls with it.
- **Panels.** Declarative panels limit what plugin authors can draw; how far that goes before people want HTML is untested.
- **Nothing was built for this note.** The figures are the projects' own; item 5 measures them.

## Sources

| Source | Licence | Used for |
| --- | --- | --- |
| [wasmtime](https://github.com/bytecodealliance/wasmtime/tree/266d7946f7783be62864cad5e553764b4c0a0f58) | Apache-2.0 with LLVM exception | Native host: fuel, epochs, limits, determinism, async |
| [wit-bindgen](https://github.com/bytecodealliance/wit-bindgen/tree/4db5baa5b99c432508042f4887cdc6ee167523a7) | Apache-2.0 (as GitHub reports it) | Guest languages, Rust and C# routes, hosts |
| [jco](https://github.com/bytecodealliance/jco/tree/e40180510a1c0ed22f41e1febd5527c4f99e604c) | Apache-2.0 | Browser host by transpiling; WASI shims |
| [ComponentizeJS](https://github.com/bytecodealliance/ComponentizeJS/tree/7df33ddcdaa5f598027962bd876939d36b1552de) | Apache-2.0 | JavaScript and TypeScript plugins |
| [componentize-py](https://github.com/bytecodealliance/componentize-py/tree/5382383be90e05b75c88d327de83be5c347379eb) | Apache-2.0 | Python plugins |
| [componentize-dotnet](https://github.com/bytecodealliance/componentize-dotnet/tree/f6bfc5a9a5a25728aeec959be60c8f737278a183) | Apache-2.0 | C# plugins |
| [cargo-component](https://github.com/bytecodealliance/cargo-component/tree/1495f6141420528dfb745dd7da94a618e58d84b2) | Apache-2.0 | Older Rust route |
| [WASI releases](https://github.com/WebAssembly/WASI/releases) | W3C Community Group | 0.2 and 0.3 status |
| [EXTENDING.md](../../../EXTENDING.md) | This repo | Plugin rules already set |
