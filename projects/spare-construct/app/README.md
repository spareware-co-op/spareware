# Spare Construct app

The browser app, on the [demo's](../demo/index.html) layout. The server sends the demo's own page unchanged and adds these files, so the ribbon, palettes, view tabs and status bar are the real ones and stay in step with it. Everything the demo shows that isn't connected yet says so, rather than showing sample data.

- [connect.js](connect.js): puts live data into the page: sign-in, hubs, projects, folders, the plan and 3D views, the Wall and Door tools, Share, Bridge, Take in and links.
- [view3d.js](view3d.js): what the 3D view draws: one batched mesh per element type, with elements of the same shape sharing one geometry. The renderer of note A04 replaces this file. [view3d-check.html](view3d-check.html) measures it against one mesh per element.
- [pkg](pkg): the same geometry the server builds with, compiled for the browser. `server/scripts/build-shapes.sh` makes it, from `crates/construct-shapes`.
- `pkg-kernel` (not committed): the geometry kernel's builder over solidcore's engine, compiled for the browser, which the page loads for a document whose edition is the kernel's (`construct.kernel`). `server/scripts/build-shapes-kernel.sh` makes it, from `server/shapes-kernel`; only a server built with its `kernel` feature serves it.
- **Workshop**, the discipline for parts, assemblies and drawings, is the demo's own page ([demo/workshop](../demo/workshop)), which the server serves at `/workshop/`. Its parts are built by the geometry kernel, compiled for the browser (`crates/construct-workshop`, made into `demo/workshop/pkg` by `server/scripts/build-workshop.sh`). Its parts and its assembly are elements of the open model, written through the same edit log as walls and doors (`element.create`, `element.set`, `element.delete`), so a part survives a reload and everyone on the model sees it change. They have no place in the plan, so connect.js finds them by kind (`/api/projects/<project>/models/<model>/kind/workshop.part`) and hands them to the page as `spare.workshop`. The server's stand-in geometry doesn't build them and records so; the page builds them with the kernel.

An element here is its recipe. The page builds it through the package its document's edition names and draws what comes back, an outline and a mesh, so it doesn't change when the geometry does. A browser that can't build a document's edition draws the server's stored shapes and doesn't edit.

The desktop shell and the wgpu viewport come later, on top of `construct-api`.
