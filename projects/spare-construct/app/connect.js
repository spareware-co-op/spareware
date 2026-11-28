// Connects the Spare Construct demo page to its server. The server sends the demo's own file
// unchanged, so its ribbon, docks, view tabs and status bar are the real ones; this script runs
// after it and puts live data where the sample was: the plan and 3D views, the project browser,
// the properties palette, the Types and Project parameters dialogs and the status bar. The page keeps only what is in view; whatever it
// fetched by hash stays in the browser's cache. Edits go up the socket and are drawn at once, then
// confirmed or turned back by the log.
//
// An element here is its recipe. Shapes come from the same geometry the server builds with,
// compiled for the browser, and this file draws what comes back: an outline, a mesh, a box around
// it. It never reads a shape's size from anywhere else, so it doesn't change when the geometry does.
// The server's checkpoints name the bodies it built; this page compares them with its own.
//
// The page is ES modules (note A96-tool-architecture.md): the core in core/, each tool in tools/<discipline>/,
// the tools in the order of tools/index.js. This file loads them and starts; it is written by the split
// of the one file this page was, so its header above still describes the whole page.

import './core/state.js';
import './core/frame.js';
import './core/hub.js';
import './core/browser.js';
import './core/open.js';
import './core/shapes.js';
import './core/channel.js';
import './core/undo.js';
import './core/parameters.js';
import './core/problems.js';
import './core/dialogs.js';
import './core/plan.js';
import './core/viewport3d.js';
import './tools/index.js';
import { boot } from './core/hub.js';

boot();
