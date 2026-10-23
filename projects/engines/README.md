# Spare Kit engines

The heavy engines of Spare Kit, which any program can use. Each is a Rust crate in `crates/`, depends only on the kit crates (or another engine), and never on a program. A program builds in only the engines it uses, so each app stays as small as its job.

| Engine | What it does | Used by |
| --- | --- | --- |
| `engine-geometry` | The one geometry kernel, 2D and 3D: exact flat-faced solids and certified curved ones (analytic and NURBS), subdivision surfaces, fields, meshes, terrain, alignments, 2D drafting, drawings, and the spatial index for "what is near what". Mapped in [G01](../kit/notes/G01-geometry-kernel.md) | Spare Construct (every discipline, survey and civil included), Spare Studio's 3D app, the simulation engine, and future programs such as mechanical design, manufacturing, circuit boards and games |
| `engine-render` | GPU renderer for 2D and 3D views, from live working views to photo-real rendering | Any program that shows a model or a picture |
| `engine-sim` | Moves many agents through networks over time: vehicles, buses and trains, and people walking | Spare Construct (traffic, public transport, people movement), open to any program |
| `engine-draw2d` | 2D drawing and vectors: lines with true weights, fills and patterns, paths and 2D booleans | Spare Construct, Spare Studio |
| `engine-text` | Fonts, shaping, line breaking and paragraph layout | Spare Construct, Spare Studio, Spare Office |
| `engine-pdf` | Reads, edits and writes PDF | Spare Construct, Spare Studio |
| `engine-media` | Video and audio: decoding, encoding, playback and timelines | Spare Studio, Spare Social, Spare Music, Spare Movies |

Layers stay separate on purpose: modelling needs exact solids, the renderer needs fast meshes, and simulation needs simple paths and collision shapes. The geometry core can produce all three, and each engine on top does one job well.
