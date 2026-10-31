# Spare Construct app

The browser app, on the [demo's](../demo/index.html) layout. The server sends the demo's own page unchanged and adds these files, so the ribbon, palettes, view tabs and status bar are the real ones and stay in step with it. Everything the demo shows that isn't connected yet says so, rather than showing sample data.

- [connect.js](connect.js): puts live data into the page: sign-in, hubs, projects, folders, the plan and 3D views, the Wall and Door tools, Share, Bridge, Take in and links.
- [pkg](pkg): the same geometry the server builds with, compiled for the browser. `server/scripts/build-shapes.sh` makes it, from `crates/construct-shapes`.

An element here is its recipe. The page builds it through that package and draws what comes back, an outline and a mesh, so it doesn't change when the geometry does. A browser that can't build a document's edition draws the server's stored shapes and doesn't edit.

The desktop shell and the wgpu viewport come later, on top of `construct-api`.
