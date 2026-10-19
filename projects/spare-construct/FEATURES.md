# Spare Construct: everything to build

*Stage: Mapping. A readable copy of [features.json](features.json), which is the source; keep the two in step. Every priority and size is a proposal for an owner to accept.*

Spare Construct replaces Revit and the add-ins people buy for it. One open building model for every discipline, built from scratch in Rust, that runs on the desktop, in the browser and headless for AI and scripts. Everything people buy add-ins for today is built in as first-party add-ins on the same public API, free to use, with hosting run at cost. This page maps the whole job top-down: 26 areas, 214 things a person can do, and 687 buildable parts. **Core** means needed for the first usable release (roadmap Gate D: a full council set for a house), **next** follows soon after, and **later** waits. Parts name what they lean on: kit parts by their ID (see the [kit map](../kit/FEATURES.md)), shared engines, or other Spare Construct parts. Where a part is already on the [work board](STATUS.md), its piece is linked.

**Replaces:** Revit, and the add-ins people buy or bolt on to it: Navisworks, Revizto and BIMcollab for clash; Solibri and UpCodes for model checking; Enscape, Twinmotion, Lumion and D5 Render for rendering; Dynamo for visual scripting; pyRevit, DiRoots, Ideate and Naviate toolkits; Ideate BIMLink and DiRoots SheetLink for Excel; AGACAD, StrucSoft MWF and hsbcad for framing; CostX and Assemble for quantities; Autodesk Docs, Newforma and Aconex for document control; Autodesk Design Collaboration for sharing between firms.

## What Spare Construct asks of the kit

Two kit parts that Spare Construct's first release needs are marked next in the kit map. Spare Construct keeps its own parts core and asks the kit to make these core too; both already sit on Spare Construct's work board.

| Kit part | Needed by | Piece |
| --- | --- | --- |
| KIT-A16-F5-P2 Plugin host | CON-A19-F4-P1, and every production tool in CON-A20 | [A08](STATUS.md) |
| KIT-A15-F6-P2 Chunked storage | CON-A2-F4-P1, CON-A2-F4-P2 | [A06](STATUS.md) |

Where the kit part could wait, the Spare Construct part moved instead: office sign-in (now CON-A1-F8-P1, later), updates (now CON-A1-F9-P1, next), the AI data rule (now CON-A1-F7-P6, next), and country packs, which build on the kit's NZ pack (KIT-A19-F9-P1) for the first release.

## Areas

| ID | Area | Features | Parts | Core features |
| --- | --- | --- | --- | --- |
| CON-A1 | [Getting started, accounts and settings](#con-a1-getting-started-accounts-and-settings) | 9 | 29 | 5 |
| CON-A2 | [Model core, storage and speed](#con-a2-model-core-storage-and-speed) | 10 | 42 | 10 |
| CON-A3 | [Geometry kernel and modelling tools](#con-a3-geometry-kernel-and-modelling-tools) | 7 | 31 | 5 |
| CON-A4 | [Architectural elements](#con-a4-architectural-elements) | 10 | 33 | 8 |
| CON-A5 | [Structure](#con-a5-structure) | 7 | 20 | 1 |
| CON-A6 | [Building services](#con-a6-building-services) | 9 | 27 | 1 |
| CON-A7 | [Families and content library](#con-a7-families-and-content-library) | 9 | 35 | 6 |
| CON-A8 | [Views and graphics](#con-a8-views-and-graphics) | 8 | 33 | 6 |
| CON-A9 | [Annotation and detailing](#con-a9-annotation-and-detailing) | 7 | 18 | 6 |
| CON-A10 | [Schedules, quantities and cost](#con-a10-schedules-quantities-and-cost) | 8 | 22 | 3 |
| CON-A11 | [Sheets, printing and issuing](#con-a11-sheets-printing-and-issuing) | 11 | 33 | 5 |
| CON-A12 | [Teamwork, history and versions](#con-a12-teamwork-history-and-versions) | 10 | 29 | 4 |
| CON-A13 | [Coordination and clash](#con-a13-coordination-and-clash) | 6 | 13 | 0 |
| CON-A14 | [Model checking, QA and building code](#con-a14-model-checking-qa-and-building-code) | 6 | 18 | 2 |
| CON-A15 | [Analysis: energy, daylight and carbon](#con-a15-analysis-energy-daylight-and-carbon) | 7 | 17 | 2 |
| CON-A16 | [Visualisation, rendering and VR](#con-a16-visualisation-rendering-and-vr) | 6 | 16 | 0 |
| CON-A17 | [Site, survey and context](#con-a17-site-survey-and-context) | 8 | 25 | 3 |
| CON-A18 | [File exchange](#con-a18-file-exchange) | 8 | 29 | 6 |
| CON-A19 | [API, scripting, AI and add-ins](#con-a19-api-scripting-ai-and-add-ins) | 8 | 40 | 4 |
| CON-A20 | [Production tools](#con-a20-production-tools) | 10 | 20 | 1 |
| CON-A21 | [Construction, fabrication and prefab](#con-a21-construction-fabrication-and-prefab) | 8 | 22 | 1 |
| CON-A22 | [Interface, accessibility and languages](#con-a22-interface-accessibility-and-languages) | 8 | 30 | 3 |
| CON-A23 | [Hosting, offices, security and running costs](#con-a23-hosting-offices-security-and-running-costs) | 7 | 26 | 0 |
| CON-A24 | [Legal, licence, community and support](#con-a24-legal-licence-community-and-support) | 7 | 28 | 4 |
| CON-A25 | [Sharing between firms](#con-a25-sharing-between-firms) | 7 | 22 | 7 |
| CON-A26 | [Project files and document control](#con-a26-project-files-and-document-control) | 13 | 29 | 12 |

## CON-A1 Getting started, accounts and settings

How a person finds, installs and starts Spare Construct, signs in only when they need to, sets preferences and learns the tool. No licence keys and no paywall.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A1-F1 Download and install | Install Spare Construct on Windows, macOS or Linux, or open it in a browser, with no licence key. | core | P1 Desktop installers; P3 Hardware check; P4 Browser start; P5 Offline installers |
| CON-A1-F2 Sign in, or work without an account | Use the desktop app fully offline with no account, and sign in only to share, sync or publish. | core | P1 Spareware sign-in; P2 Local profile |
| CON-A1-F3 Start a first project | Create a project from a template for their country and project type, or open a sample, in under a minute. | core | P1 New project flow; P2 Starter templates; P3 Sample projects; P4 Home screen |
| CON-A1-F4 Bring my old work across | Move from Revit by bringing models, families, templates and office standards across in one guided step. | next | P1 Move from Revit guide; P2 Standards mapper; P3 Move report |
| CON-A1-F5 Learn as I go | Learn the tool through short lessons, tips in context and searchable help, with no paid training needed. | core | P1 Help library; P2 Interactive lessons; P3 Tips in context; P4 Revit terms finder; P5 Offline help |
| CON-A1-F6 Set my preferences | Set units, snaps, shortcuts, colours, language and file locations once and carry them to every computer. | core | P1 Preferences store; P2 Preference sync; P3 Shortcut editor |
| CON-A1-F7 Get help when stuck | Report a problem, see known issues and service status, get support as a paying office, and get an answer from the community or an AI helper. | next | P1 Problem report; P2 Crash recovery; P3 AI helper; P4 Support for paying offices; P5 Service status; P6 AI choice and project data |
| CON-A1-F8 Sign in with my office's own login | Sign in to Spare Construct through an office's own identity provider, as a member of the office's team. | later | P1 Office sign-in |
| CON-A1-F9 Stay up to date on my chosen channel | Get updates in the background on a stable, preview or pinned channel, and roll back if a release causes trouble. | next | P1 Updates and channels |

**Parts in Getting started, accounts and settings**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A1-F1-P1 | **Desktop installers**. Signed installers for Windows, macOS and Linux built from the desktop shell, signed through the kit's release signing (KIT-A22-F2-P3). | integration | medium | CON-A22-F1-P1, KIT-A22-F2-P3 |  |
| CON-A1-F1-P3 | **Hardware check**. Checks the graphics card, memory and browser support on first run and explains any limit in plain words. | library | small | CON-A8-F4-P1 |  |
| CON-A1-F1-P4 | **Browser start**. Opens the app in a browser with the core in a web worker and the cross-origin headers that threads need. | integration | medium | CON-A2-F10-P1 |  |
| CON-A1-F1-P5 | **Offline installers**. Full installers an office can copy to machines with no internet, with the help library and starter content included. | integration | small | CON-A1-F1-P1, CON-A1-F5-P5, CON-A7-F6-P7 |  |
| CON-A1-F2-P1 | **Spareware sign-in**. One Spareware login, passkey first, through the kit's account service (KIT-A1-F1-P1, KIT-A1-F2-P1), used only for sharing and hosted features. | integration | small | KIT-A1-F1-P1, KIT-A1-F2-P1 |  |
| CON-A1-F2-P2 | **Local profile**. A local profile that holds preferences and recent projects with no account at all. | data | small | KIT-A15-F1-P1 |  |
| CON-A1-F3-P1 | **New project flow**. Picks country, units, template and project location, then opens the first view. | interface | small | CON-A2-F4-P1, CON-A1-F3-P2 |  |
| CON-A1-F3-P2 | **Starter templates**. Open templates per country and discipline: levels, view types, title blocks, parameters and styles. | data | large | CON-A22-F5-P1, CON-A8-F3-P1 |  |
| CON-A1-F3-P3 | **Sample projects**. A house, a small commercial building and a multi-storey building to learn and test with. | data | medium | CON-A2-F4-P1 |  |
| CON-A1-F3-P4 | **Home screen**. Recent projects, pinned projects, templates and learning links on one start page. | interface | small | CON-A1-F2-P2 |  |
| CON-A1-F4-P1 | **Move from Revit guide**. A guided import that takes exported models, parameter lists and office standards and maps them to Spare Construct. | interface | medium | CON-A18-F1-P1, CON-A18-F2-P1 |  |
| CON-A1-F4-P2 | **Standards mapper**. Maps categories, line styles, fill patterns and parameter names to ours and reports what did not map. | library | medium | CON-A8-F3-P1, CON-A2-F2-P1 |  |
| CON-A1-F4-P3 | **Move report**. A plain report of what came across, what changed and what needs a person to check. | interface | small | CON-A1-F4-P1 |  |
| CON-A1-F5-P1 | **Help library**. Searchable help pages for every command, generated from the command schema plus written guides, and published through the kit's help centre (KIT-A21-F1-P1). The same source builds the public documentation site (A24-F5-P2). | data | medium | CON-A19-F1-P1, KIT-A21-F1-P1 |  |
| CON-A1-F5-P2 | **Interactive lessons**. Step-by-step lessons that run inside a sample project and check each step. | interface | medium | CON-A1-F3-P3 |  |
| CON-A1-F5-P3 | **Tips in context**. Short hints on the first use of a tool, which can be turned off. | interface | small | CON-A22-F1-P2 |  |
| CON-A1-F5-P4 | **Revit terms finder**. Type a Revit term and find the matching Spare Construct command or idea. | data | small | CON-A1-F5-P1 |  |
| CON-A1-F5-P5 | **Offline help**. The help library and lessons installed with the app, so help works with no connection. | data | small | CON-A1-F5-P1, CON-A1-F5-P2 |  |
| CON-A1-F6-P1 | **Preferences store**. Typed preferences per person, per office and per project, with a clear order of precedence. | library | small | KIT-A15-F1-P1 |  |
| CON-A1-F6-P2 | **Preference sync**. Carries a signed-in person's Spare Construct preferences to every device through the kit's settings (KIT-A20-F5-P1) and sync (KIT-A15-F4-P2). | service | small | CON-A1-F6-P1, KIT-A20-F5-P1, KIT-A15-F4-P2 |  |
| CON-A1-F6-P3 | **Shortcut editor**. Rebind any command to keys or a two-key sequence, and import an existing keyboard file. | interface | small | CON-A19-F1-P1 |  |
| CON-A1-F7-P1 | **Problem report**. Sends a report with the log, the operations leading up to it and an optional anonymised piece of model, only with consent, through the kit's crash reports (KIT-A22-F9-P1). Report storage is metered so its cost reaches the ledger (KIT-A13-F2-P2). | service | medium | KIT-A15-F1-P2, KIT-A22-F9-P1, KIT-A15-F7-P1, KIT-A13-F2-P2 |  |
| CON-A1-F7-P2 | **Crash recovery**. Reopens after a crash with every operation up to the crash replayed. | library | medium | CON-A2-F5-P1 |  |
| CON-A1-F7-P3 | **AI helper**. One in-app AI helper, built on the kit's help assistant (KIT-A17-F7-P1). It answers how-to questions from the help library and, when asked, proposes commands the person sees before they run. It replaces the separate in-app assistant that was CON-A19-F3-P3. | integration | medium | KIT-A17-F7-P1, CON-A1-F5-P1, CON-A19-F3-P1, CON-A1-F7-P6 |  |
| CON-A1-F7-P4 | **Support for paying offices**. Support cases for offices that pay for hosting, through the kit's support cases (KIT-A21-F2-P1), with the project and version attached when the office agrees. | integration | small | CON-A1-F7-P1, KIT-A21-F2-P1 |  |
| CON-A1-F7-P5 | **Service status**. Hosted sync, library and sharing shown on the kit's status page (KIT-A21-F5-P1), linked from the app. | integration | small | CON-A23-F1-P1, KIT-A21-F5-P1 |  |
| CON-A1-F7-P6 | **AI choice and project data**. Spare Construct's rule on what model and client data an in-app AI may receive, set by the office per project. Choosing the model and paying at cost follow the kit's AI tools policy (KIT-A17-F6-P1). Next, with the AI helper, because the first release sends nothing to a co-op AI. | legal | small | KIT-A17-F6-P1 |  |
| CON-A1-F8-P1 | **Office sign-in**. Sign in through an office's own identity provider, as a member of the office's organisation, using the kit's organisation model (KIT-A1-F7-P1) and outside sign-in links (KIT-A1-F10-P1). Later, not core, because both kit parts come after the kit's first release. | integration | medium | CON-A1-F2-P1, KIT-A1-F7-P1, KIT-A1-F10-P1 |  |
| CON-A1-F9-P1 | **Updates and channels**. Spare Construct's update rules (stable, preview and pinned channels, versions pinned per project, and roll back) delivered through the kit's update delivery (KIT-A20-F7-P1) and signed releases (KIT-A22-F2-P3), with update packages an office can copy to machines with no internet. Next, not core, because the kit's update delivery is next; the first release ships signed installers only. | service | medium | CON-A1-F1-P1, KIT-A20-F7-P1, KIT-A22-F2-P3 |  |

## CON-A2 Model core, storage and speed

The database every element lives in, the project file, the operation log behind undo and sync, and the speed and truth tests that keep it fast and correct.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A2-F1 Keep the whole building in one model | Model architecture, structure and services in one model rather than a stack of linked files. | core | P1 Element store; P2 Permanent IDs; P3 Relationship tables; P4 Discipline ownership |
| CON-A2-F2 Give elements data | Give any element typed parameters, formulas, shared parameters and project-wide values. | core | P1 Typed parameters; P2 Formula language; P3 Shared parameters; P4 Project-wide values; P5 Project information |
| CON-A2-F3 Work in real units and real places | Model in metres or feet with survey-accurate coordinates and true north. | core | P1 Units and display; P2 Coordinates and survey points; P3 Coordinate systems; P4 Vertical datums |
| CON-A2-F4 Save and reopen without waiting | Save instantly and reopen a large project to its first view in seconds. | core | P1 Project file; P2 Chunked storage; P3 Paging from disk; P4 Spatial index; P5 Never-blocking save; P6 Published file format; P7 No forced upgrades; P8 Storage layout decision |
| CON-A2-F5 Undo anything | Undo and redo any change, including big batch changes, in one step. | core | P1 Operation log; P2 Transactions; P3 Undo and redo |
| CON-A2-F6 See edits update at once | Change one thing and see everything that depends on it update straight away, using every CPU core. | core | P1 Regeneration graph; P2 Parallel rebuild; P3 Progress and cancel |
| CON-A2-F7 See problems without being stopped | See modelling problems as a list linked to the elements, never as a dialog that stops work. | core | P1 Warnings as data; P2 Warnings panel |
| CON-A2-F8 Use phases, options and groups | Show what is existing, new and demolished, try design options, and repeat sets of elements as groups. | core | P1 Phases; P2 Design options; P3 Groups; P4 Assemblies |
| CON-A2-F9 Stay fast on any size of project | Open, move around and edit a whole multi-storey building faster than Revit does. | core | P1 Benchmark harness; P2 Large test models; P3 Truth tests; P4 Determinism checks; P5 Drawing truth tests |
| CON-A2-F10 Run the same core everywhere | Run the same model core in the desktop app, in a browser and on a server. | core | P1 Browser core; P2 Core to screen protocol; P3 Headless core service; P4 Offline in the browser |

**Parts in Model core, storage and speed**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A2-F1-P1 | **Element store**. A column-by-column element store for millions of elements, with types stored once and instances storing only what differs. | library | large | KIT-A15-F1-P1 | [A02](STATUS.md) |
| CON-A2-F1-P2 | **Permanent IDs**. 128-bit time-ordered IDs on disk and over the wire, with a compact index in memory, so offline work never collides. | library | small | KIT-A15-F1-P1 |  |
| CON-A2-F1-P3 | **Relationship tables**. Hosted by, joined to, bounded by, in room, on level and in group, stored as tables and queryable both ways. | library | medium | CON-A2-F1-P1 |  |
| CON-A2-F1-P4 | **Discipline ownership**. Every element has an owning discipline that views, permissions and exports can filter on. | library | small | CON-A2-F1-P1 |  |
| CON-A2-F2-P1 | **Typed parameters**. Length, area, volume, angle, number, text, yes or no, choice, element link, material and web link values. | library | medium | CON-A2-F1-P1 |  |
| CON-A2-F2-P2 | **Formula language**. A small, safe expression language for parameter formulas, with units checked. | library | medium | CON-A2-F2-P1 |  |
| CON-A2-F2-P3 | **Shared parameters**. Parameters keyed by GUID so they match across projects, families and schedules. | data | small | CON-A2-F2-P1 |  |
| CON-A2-F2-P4 | **Project-wide values**. Named values that drive dimensions and parameters across the whole project. | library | small | CON-A2-F2-P1, CON-A2-F6-P1 |  |
| CON-A2-F2-P5 | **Project information**. Client, address, project number, stage and other project data that title blocks and exports read. | data | small | CON-A2-F2-P1 |  |
| CON-A2-F3-P1 | **Units and display**. SI units inside in f64, with display units, rounding and formats as view and project settings. | library | small | KIT-A15-F1-P1 |  |
| CON-A2-F3-P2 | **Coordinates and survey points**. Local building origin, project base point, survey point, true north and NZTM or any other projected system. | library | medium | CON-A2-F3-P1 |  |
| CON-A2-F3-P3 | **Coordinate systems**. Coordinate system definitions and transforms for any country, from a database whose terms are checked first (A24-F1-P7). A candidate for the kit's maps, since other apps place things on maps too. | data | medium | CON-A2-F3-P2, CON-A24-F1-P7 |  |
| CON-A2-F3-P4 | **Vertical datums**. NZVD2016 and the older local vertical datums, with the published transforms between them. Every level, spot height and drainage invert states its datum, so survey, flood and drainage levels line up. | library | medium | CON-A2-F3-P3, CON-A24-F1-P7 |  |
| CON-A2-F4-P1 | **Project file**. One portable project file holding chunk blobs, the operation log and the index, on the kit's chunked storage (KIT-A15-F6-P2), which Spare Construct needs in its first release. | library | medium | KIT-A15-F6-P2 | [A06](STATUS.md) |
| CON-A2-F4-P2 | **Chunked storage**. Elements grouped by building, level and spatial cell, compressed and content-hashed so unchanged chunks are never re-saved. Spare Construct needs the kit's chunked storage (KIT-A15-F6-P2, piece A06) in its first release, so it asks the kit to make that part core. | library | large | CON-A2-F1-P1, CON-A2-F4-P8, KIT-A15-F6-P2 |  |
| CON-A2-F4-P3 | **Paging from disk**. Keeps recently used chunks in memory and pages the rest, so a city never has to fit in memory. | library | large | CON-A2-F4-P2 |  |
| CON-A2-F4-P4 | **Spatial index**. A bounding-volume hierarchy over chunks and elements that answers what is in a view or box in microseconds. | library | medium | CON-A2-F1-P1, CON-A2-F4-P2 |  |
| CON-A2-F4-P5 | **Never-blocking save**. Operations append to the log at once and chunks compact in the background. | library | medium | CON-A2-F5-P1, CON-A2-F4-P1 |  |
| CON-A2-F4-P6 | **Published file format**. A documented, versioned schema and a standalone reader library in Rust and Python, so files outlive the app. | data | medium | CON-A2-F4-P1 |  |
| CON-A2-F4-P7 | **No forced upgrades**. Files open in older and newer releases without a one-way upgrade, using schema versions and migrations. | library | medium | CON-A2-F4-P6 |  |
| CON-A2-F4-P8 | **Storage layout decision**. A decision note on chunks by building, level and cell or one flat file, the cell size, and levels or pure 3D cells. One of the six decisions that gate the Design stage; it has no work-board piece yet. | research | small |  |  |
| CON-A2-F5-P1 | **Operation log**. Every change is an operation in one log that drives undo, history and sync. | library | large | KIT-A15-F1-P2 |  |
| CON-A2-F5-P2 | **Transactions**. Every command fully applies or fully rolls back. | library | medium | KIT-A15-F1-P2, CON-A2-F5-P1 |  |
| CON-A2-F5-P3 | **Undo and redo**. Steps through the operation log, grouped the way a person thinks of the change. | library | small | CON-A2-F5-P2 |  |
| CON-A2-F6-P1 | **Regeneration graph**. Incremental recomputation of only what changed, with early cut-off when a value comes out the same. | library | large | CON-A2-F1-P1 | [A03](STATUS.md) |
| CON-A2-F6-P2 | **Parallel rebuild**. Independent changed elements rebuild at the same time on a work-stealing thread pool. | library | medium | CON-A2-F6-P1 |  |
| CON-A2-F6-P3 | **Progress and cancel**. Long global edits show progress and can be cancelled without harm. | library | small | CON-A2-F6-P1, CON-A2-F5-P2 |  |
| CON-A2-F7-P1 | **Warnings as data**. Problems stored with the elements involved, readable by people, AI and scripts. | library | medium | CON-A2-F1-P1, CON-A2-F6-P1 |  |
| CON-A2-F7-P2 | **Warnings panel**. Lists, filters and zooms to warnings, and offers a fix where one is clear. | interface | small | CON-A2-F7-P1 |  |
| CON-A2-F8-P1 | **Phases**. Created and demolished phases on every element, with phase filters for views. | library | medium | CON-A2-F1-P1 |  |
| CON-A2-F8-P2 | **Design options**. Option sets and options as fields on elements, with views that choose which option to show. | library | medium | CON-A2-F1-P1 |  |
| CON-A2-F8-P3 | **Groups**. Model and detail groups that repeat a set of elements and update every copy. | library | medium | CON-A2-F1-P1, CON-A2-F6-P1 |  |
| CON-A2-F8-P4 | **Assemblies**. Named sets of elements treated as one item for schedules and shop drawings. | library | small | CON-A2-F1-P1 |  |
| CON-A2-F9-P1 | **Benchmark harness**. Benchmarks in CI for every target in the map; a regression over 5% blocks the merge. | service | medium |  |  |
| CON-A2-F9-P2 | **Large test models**. Anonymised real projects and generated cities used as test fixtures. | data | medium | CON-A2-F4-P6 |  |
| CON-A2-F9-P3 | **Truth tests**. The same element built in Revit and here, compared within tolerance. Revit's geometry comes out through the exporter add-in, which waits on the Revit API terms check (A18-F2-P2). | service | large | CON-A18-F2-P1, CON-A18-F2-P2 |  |
| CON-A2-F9-P4 | **Determinism checks**. Replays the same log natively and in the browser and requires identical results. | service | medium | CON-A2-F5-P1, CON-A2-F10-P1 |  |
| CON-A2-F9-P5 | **Drawing truth tests**. Plans and sections of the same model compared with Revit's output within a set tolerance: the test behind Gate C. | service | medium | CON-A2-F9-P3, CON-A8-F2-P2 |  |
| CON-A2-F10-P1 | **Browser core**. The core compiled to WebAssembly in a web worker, holding only what the person has open. | library | large | CON-A2-F1-P1, CON-A2-F4-P2 |  |
| CON-A2-F10-P2 | **Core to screen protocol**. A versioned binary protocol: commands in, changes and ready-to-upload geometry chunks out. | library | medium | KIT-A15-F1-P2 |  |
| CON-A2-F10-P3 | **Headless core service**. The native core running as a service for AI agents, scripts and multi-user servers. | service | medium | CON-A2-F10-P2 |  |
| CON-A2-F10-P4 | **Offline in the browser**. Small models saved in the browser's own storage, so they open and save with no connection. | library | small | CON-A2-F10-P1 |  |

## CON-A3 Geometry kernel and modelling tools

The exact solid geometry under every element, the sketch solver, snapping and editing tools, and massing and free-form shapes.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A3-F1 Make solid shapes that never break | Build walls, roofs and components from solids whose cuts and joins always succeed. | core | P1 Solid kernel v1; P2 Booleans; P3 Exact predicates; P4 Solid steps; P5 Kernel fuzzing; P6 Tessellation; P7 Kernel start decision; P8 Model tolerance decision |
| CON-A3-F2 Sketch with constraints | Draw outlines and lock them with dimensions, parallels, equal lengths and alignments. | core | P1 Sketch solver; P2 Sketch mode; P3 Project locks; P4 Project locks decision |
| CON-A3-F3 Snap and pick precisely | Pick and snap to exact ends, middles, centres, intersections and faces. | core | P1 Snapping; P2 Picking; P3 Work planes; P4 Measure |
| CON-A3-F4 Edit elements directly | Move, copy, rotate, mirror, array, align, offset, trim, split and scale elements. | core | P1 Modify tools; P2 Grips and handles; P3 Pin and lock; P4 Paste aligned; P5 Copy between projects; P6 Match type and create similar |
| CON-A3-F5 Mass a building early | Shape a building as masses, get floor areas from them, and turn mass faces into walls, floors and roofs. | next | P1 Conceptual masses; P2 Elements from faces |
| CON-A3-F6 Model free-form shapes | Model curved roofs, facades and organic shapes with curved surfaces. | later | P1 Kernel v2 curved surfaces; P2 Curved kernel study; P3 Adaptive components; P4 Subdivision shapes; P5 Curved surfaces timing decision |
| CON-A3-F7 Place imported shapes as elements | Place shapes from other tools as real elements with a category and data. | core | P1 Direct shapes; P2 Mesh repair |

**Parts in Geometry kernel and modelling tools**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A3-F1-P1 | **Solid kernel v1**. Flat-faced solids on a half-edge structure that always returns a valid solid or a clear error. | library | large | KIT-A15-F1-P1, CON-A3-F1-P7, CON-A3-F1-P8 |  |
| CON-A3-F1-P2 | **Booleans**. Cut, join and intersect ported from Manifold's published method, batched so 300 openings cut as one. | library | large | CON-A3-F1-P1, CON-A3-F1-P7 | [A01](STATUS.md) |
| CON-A3-F1-P3 | **Exact predicates**. Adaptive exact arithmetic for orientation and in-circle tests, so which side of a plane is never wrong. | library | medium | KIT-A15-F1-P1 |  |
| CON-A3-F1-P4 | **Solid steps**. Extrude, sweep, revolve, blend and swept blend along lines and arcs. | library | medium | CON-A3-F1-P1 |  |
| CON-A3-F1-P5 | **Kernel fuzzing**. Random solids and boolean sequences; any invalid result is a bug. | service | small | CON-A3-F1-P2 |  |
| CON-A3-F1-P6 | **Tessellation**. Turns exact solids into meshes split by on-screen error, cached per type and detail level. | library | medium | CON-A3-F1-P1 |  |
| CON-A3-F1-P7 | **Kernel start decision**. A decision note on wrapping Manifold's C++ for the first stretch or writing Rust from day one, with Manifold as the reference answer in tests. One of the six decisions that gate the Design stage; piece A01 studies Manifold's method but does not make this decision. | research | small |  |  |
| CON-A3-F1-P8 | **Model tolerance decision**. A decision note on the one model-wide tolerance (lean: 0.01 mm), set against Revit's short-curve limit of about 0.8 mm. | research | small |  |  |
| CON-A3-F2-P1 | **Sketch solver**. A 2D constraint solver following the planegcs method, written as our own code. | library | large | CON-A3-F1-P1 | [A11](STATUS.md) |
| CON-A3-F2-P2 | **Sketch mode**. Draw lines, arcs, splines, circles and rectangles with snaps and live dimensions. | interface | medium | CON-A3-F2-P1, CON-A3-F3-P1 |  |
| CON-A3-F2-P3 | **Project locks**. Locked dimensions and alignments between elements in the project, not only inside families. | library | large | CON-A3-F2-P1, CON-A2-F6-P1, CON-A3-F2-P4 |  |
| CON-A3-F2-P4 | **Project locks decision**. A decision note on Revit-style locks between elements in the project, or constraints inside families only at first (lean: families first). | research | small |  |  |
| CON-A3-F3-P1 | **Snapping**. Snapping against exact kernel geometry, with temporary dimensions while drawing. | library | medium | CON-A3-F1-P1, CON-A2-F4-P4 |  |
| CON-A3-F3-P2 | **Picking**. Fast picking through a GPU ID buffer, with cycling through overlapping elements. | library | small | CON-A8-F4-P1 |  |
| CON-A3-F3-P3 | **Work planes**. Set a work plane from a level, face, reference plane or line. | library | small | CON-A3-F1-P1 |  |
| CON-A3-F3-P4 | **Measure**. Measure distances, angles, areas and volumes between picked points, edges and faces, without placing anything. | interface | small | CON-A3-F3-P1 |  |
| CON-A3-F4-P1 | **Modify tools**. The standard modify commands as operations, each with a headless form. | library | medium | KIT-A15-F1-P2, CON-A2-F6-P1 |  |
| CON-A3-F4-P2 | **Grips and handles**. Drag ends, shape handles and arrows on screen, with live feedback. | interface | medium | CON-A3-F4-P1, CON-A8-F4-P1 |  |
| CON-A3-F4-P3 | **Pin and lock**. Pin elements so nothing moves them by mistake. | library | small | CON-A2-F1-P1 |  |
| CON-A3-F4-P4 | **Paste aligned**. Paste to the same place, to chosen levels or to chosen views, keeping hosts where they exist. | library | small | CON-A3-F4-P1 |  |
| CON-A3-F4-P5 | **Copy between projects**. Copy elements with their types between open projects, with a choice when a type of the same name differs. | library | medium | CON-A3-F4-P1, CON-A2-F1-P2 |  |
| CON-A3-F4-P6 | **Match type and create similar**. Give chosen elements another element's type, or start drawing a new element of a picked element's type. | interface | small | CON-A3-F4-P1 |  |
| CON-A3-F5-P1 | **Conceptual masses**. Massing solids with mass floors and gross area reports. | library | medium | CON-A3-F1-P4 |  |
| CON-A3-F5-P2 | **Elements from faces**. Walls, floors and roofs made from mass faces and kept linked. Curtain systems on faces come from A4-F6-P2. | library | large | CON-A3-F5-P1, CON-A4-F2-P1 |  |
| CON-A3-F6-P1 | **Kernel v2 curved surfaces**. NURBS surfaces and solids behind the same kernel interface. | library | large | CON-A3-F1-P1, CON-A3-F6-P5 |  |
| CON-A3-F6-P2 | **Curved kernel study**. How OpenCascade and truck handle curved booleans, learning the method only. | research | medium |  |  |
| CON-A3-F6-P3 | **Adaptive components**. Point-driven components that repeat across divided surfaces. | library | large | CON-A3-F6-P1, CON-A7-F1-P1 |  |
| CON-A3-F6-P4 | **Subdivision shapes**. Soft shapes edited by a control cage, then turned into solids. | library | medium | CON-A3-F6-P1 |  |
| CON-A3-F6-P5 | **Curved surfaces timing decision**. A decision note on when curved surfaces become necessary: kernel v1 or v2 (lean: v2). | research | small |  |  |
| CON-A3-F7-P1 | **Direct shapes**. Solids and meshes placed as elements with a category and parameters. | library | small | CON-A3-F1-P1, CON-A2-F1-P1 |  |
| CON-A3-F7-P2 | **Mesh repair**. Closes holes and fixes normals so imported meshes can be cut and measured. | library | medium | CON-A3-F1-P1 |  |

## CON-A4 Architectural elements

The building elements an architect models every day. Each is a rule set for how it is shaped, joined, hosted and drawn.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A4-F1 Set up levels and grids | Add levels, grids and reference planes, with extents and bubbles set per view. | core | P1 Levels; P2 Grids; P3 Reference planes; P4 Scope boxes |
| CON-A4-F2 Draw walls | Draw layered walls that join cleanly, attach to roofs and floors, and wrap at ends and openings. | core | P1 Wall element; P2 Wall joins; P3 Wall profiles and sweeps; P4 Layer wrapping |
| CON-A4-F3 Place doors, windows and openings | Place doors and windows in walls, and cut shafts and openings through floors and roofs. | core | P1 Hosting; P2 Openings and shafts; P3 Flip and swap; P4 Dormer openings |
| CON-A4-F4 Model floors, ceilings and roofs | Sketch floors, ceilings and roofs with layers, slopes and edges. | core | P1 Floors; P2 Ceilings; P3 Roofs; P4 Roof solving |
| CON-A4-F5 Design stairs, ramps and railings | Lay out stairs, ramps and railings by rules that check code limits as they are drawn. | core | P1 Stairs; P2 Stair code limits; P3 Railings; P4 Ramps |
| CON-A4-F6 Model curtain walls and glazing | Draw curtain walls, set their grids and swap panels and mullions. | next | P1 Curtain walls; P2 Curtain systems; P3 Doors in curtain walls |
| CON-A4-F7 Find rooms and areas | Place rooms that find their own boundaries, and measure areas the way each country's rules ask. | core | P1 Rooms; P2 Area plans; P3 Area measurement standards; P4 Colour fills |
| CON-A4-F8 Place furniture, fittings and equipment | Place furniture, joinery, fixtures and equipment from the library. | core | P1 Placed components; P2 Joinery runs |
| CON-A4-F9 Add model text, lines and parts | Add 3D text and model lines, and break layered walls and floors into parts. | next | P1 Model text and lines; P2 Parts |
| CON-A4-F10 Show existing and demolished work | Model an existing building, mark what is demolished and show each phase in its own style. | core | P1 Demolition tools; P2 Phase graphics |

**Parts in Architectural elements**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A4-F1-P1 | **Levels**. Named datum planes with elevations, storey flags and building ownership. | library | small | CON-A2-F1-P1 |  |
| CON-A4-F1-P2 | **Grids**. Straight, arc and multi-segment grids with extents set per view. | library | medium | CON-A2-F1-P1 |  |
| CON-A4-F1-P3 | **Reference planes**. Named reference planes for hosting and alignment. | library | small | CON-A2-F1-P1 |  |
| CON-A4-F1-P4 | **Scope boxes**. Boxes that set the extent of datums and view crops together. | library | small | CON-A4-F1-P1, CON-A4-F1-P2 |  |
| CON-A4-F2-P1 | **Wall element**. Layered wall types with a location line, base and top constraints and attached tops. | library | large | CON-A3-F1-P1, CON-A2-F6-P1 |  |
| CON-A4-F2-P2 | **Wall joins**. L, T, X and mitre joins solved in plan first, with layer priority and clean-up. | library | large | CON-A4-F2-P1 |  |
| CON-A4-F2-P3 | **Wall profiles and sweeps**. Edit wall outlines, add sweeps and reveals, and stack walls. | library | medium | CON-A4-F2-P1, CON-A3-F2-P1 |  |
| CON-A4-F2-P4 | **Layer wrapping**. Layers wrap at ends and around inserts by type rules. | library | medium | CON-A4-F2-P1 |  |
| CON-A4-F3-P1 | **Hosting**. Hosted elements store their host and offset and move with the host without rebuilding. | library | medium | CON-A2-F1-P1, CON-A2-F6-P1 |  |
| CON-A4-F3-P2 | **Openings and shafts**. Plain wall openings, shafts through many floors and openings by face. | library | medium | CON-A3-F1-P2, CON-A4-F3-P1 |  |
| CON-A4-F3-P3 | **Flip and swap**. Flip hand and facing, swap types, and keep tags and data. | library | small | CON-A4-F3-P1 |  |
| CON-A4-F3-P4 | **Dormer openings**. Cut a roof for a dormer from the dormer's walls and roof, kept in step as they move. | library | small | CON-A4-F4-P3, CON-A4-F3-P2 |  |
| CON-A4-F4-P1 | **Floors**. Sketched layered floors with slope arrows, shape editing and slab edges. | library | large | CON-A3-F2-P1, CON-A3-F1-P1 |  |
| CON-A4-F4-P2 | **Ceilings**. Ceilings found from room boundaries or sketched, with grid patterns. | library | medium | CON-A4-F4-P1 |  |
| CON-A4-F4-P3 | **Roofs**. Footprint and extrusion roofs with slopes, soffits, fascias and gutters. | library | large | CON-A4-F4-P1, CON-A3-F1-P2 |  |
| CON-A4-F4-P4 | **Roof solving**. Solves ridges, hips and valleys and joins between roofs. | library | large | CON-A4-F4-P3 |  |
| CON-A4-F5-P1 | **Stairs**. Runs, landings and supports laid out by rules, by component or by sketch. | library | large | CON-A3-F2-P1, CON-A3-F1-P1 |  |
| CON-A4-F5-P2 | **Stair code limits**. Riser, going, pitch and headroom limits per country as data, starting with NZ Building Code clause D1, with the source's terms checked (A24-F1-P6). | data | medium | CON-A4-F5-P1, CON-A24-F1-P6 |  |
| CON-A4-F5-P3 | **Railings**. Railings following paths, with balusters, rails, posts and handrail extensions. | library | large | CON-A3-F1-P1 |  |
| CON-A4-F5-P4 | **Ramps**. Ramps with slope limits and landings. | library | medium | CON-A4-F5-P1 |  |
| CON-A4-F6-P1 | **Curtain walls**. Curtain grids, panels and mullions, with panel sizes driven by the grids. | library | large | CON-A4-F2-P1 |  |
| CON-A4-F6-P2 | **Curtain systems**. Curtain systems on faces and sloped glazing on roofs. | library | medium | CON-A4-F6-P1, CON-A3-F5-P2 |  |
| CON-A4-F6-P3 | **Doors in curtain walls**. Doors and windows placed as curtain panels. | library | small | CON-A4-F6-P1 |  |
| CON-A4-F7-P1 | **Rooms**. Room boundaries from bounding elements and separation lines, with area, perimeter and volume. | library | large | CON-A4-F2-P1, CON-A3-F1-P3 |  |
| CON-A4-F7-P2 | **Area plans**. Gross, rentable and custom area schemes with boundary rules per scheme. | library | medium | CON-A4-F7-P1 |  |
| CON-A4-F7-P3 | **Area measurement standards**. Which measurement rules each country uses and how to set them as data, with sources. | research | small | CON-A4-F7-P2 |  |
| CON-A4-F7-P4 | **Colour fills**. Rooms and areas coloured by a parameter, with a legend. The same colour scheme engine colours any element by value in A20-F6-P1. | library | medium | CON-A4-F7-P1, CON-A8-F3-P2 |  |
| CON-A4-F8-P1 | **Placed components**. Placed instances of family types, hosted or free. | library | medium | CON-A7-F1-P1, CON-A4-F3-P1 |  |
| CON-A4-F8-P2 | **Joinery runs**. Kitchen and joinery runs laid out along a wall from a spec. | library | medium | CON-A4-F8-P1 |  |
| CON-A4-F9-P1 | **Model text and lines**. 3D text and model lines as elements. | library | small | CON-A3-F1-P1, engine-text |  |
| CON-A4-F9-P2 | **Parts**. Layered walls and floors broken into parts that can be split, shown and scheduled. | library | medium | CON-A4-F2-P1, CON-A4-F4-P1 |  |
| CON-A4-F10-P1 | **Demolition tools**. Demolish whole elements, or split off and demolish only the removed piece. | library | medium | CON-A2-F8-P1 |  |
| CON-A4-F10-P2 | **Phase graphics**. Existing, new, demolished and temporary styles per view. | library | small | CON-A2-F8-P1, CON-A8-F3-P2 |  |

## CON-A5 Structure

Structural elements, the analytical model and links to analysis software, so engineers work in the same model as the architect.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A5-F1 Place columns, beams and bracing | Place columns, beams, bracing and trusses that join and cut back correctly. | next | P1 Structural columns; P2 Beams and bracing; P3 Trusses; P4 Section libraries; P5 Discipline order decision |
| CON-A5-F2 Lay out structure from grids | Place columns at grid crossings and beams along grids in one step. | next | P1 Structure by grid |
| CON-A5-F3 Model foundations and slabs | Model footings, piles, slabs and retaining walls, and draw the foundation plan a house consent needs. | core | P1 Foundations; P2 Structural floors and walls; P3 House foundations to NZS 3604 |
| CON-A5-F4 Keep an analytical model | Keep a linked analytical model with nodes, members, panels, supports and loads. | next | P1 Analytical model; P2 Loads and combinations; P3 Supports and releases |
| CON-A5-F5 Send to analysis and bring results back | Send the analytical model to analysis software and see results back in the model. | later | P1 Analysis export; P2 Analysis links; P3 Results in the model |
| CON-A5-F6 Detail reinforcement | Place rebar, mesh and cover in concrete, and get bending schedules. | later | P1 Rebar; P2 Shape codes; P3 Bending schedules |
| CON-A5-F7 Detail steel connections | Add connections, plates, bolts and welds to steel framing. | later | P1 Connections; P2 Steel fabrication output |

**Parts in Structure**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A5-F1-P1 | **Structural columns**. Vertical and slanted columns between levels, attached to beams and floors. | library | medium | CON-A4-F8-P1, CON-A4-F1-P1 |  |
| CON-A5-F1-P2 | **Beams and bracing**. Beams, beam systems and bracing with cut-backs, justification and joins. | library | large | CON-A3-F1-P1, CON-A4-F8-P1 |  |
| CON-A5-F1-P3 | **Trusses**. Trusses laid out from a profile and member types. | library | medium | CON-A5-F1-P2 |  |
| CON-A5-F1-P4 | **Section libraries**. Steel, timber and concrete section sizes per country as data, from sources whose terms are checked first (A24-F1-P6). | data | medium | CON-A7-F1-P1, CON-A24-F1-P6 |  |
| CON-A5-F1-P5 | **Discipline order decision**. A decision note on which discipline follows the core, structure or services (lean: structure, which has fewer element types). | research | small |  |  |
| CON-A5-F2-P1 | **Structure by grid**. Places columns and beams on chosen grid lines and levels. | library | small | CON-A4-F1-P2, CON-A5-F1-P2 |  |
| CON-A5-F3-P1 | **Foundations**. Isolated, strip and pile foundations hosted to walls, and to structural columns once they arrive (A5-F1-P1). | library | medium | CON-A4-F2-P1, CON-A7-F3-P2 |  |
| CON-A5-F3-P2 | **Structural floors and walls**. Structural use on floors and walls, with span direction and core layers. | library | small | CON-A4-F4-P1, CON-A4-F2-P1 |  |
| CON-A5-F3-P3 | **House foundations to NZS 3604**. Piles, foundation walls and slab-on-ground edges and thickenings laid out by NZS 3604 rules, with a foundation plan view template. | library | medium | CON-A5-F3-P1, CON-A21-F1-P2, CON-A17-F8-P1 |  |
| CON-A5-F4-P1 | **Analytical model**. Analytical members and panels linked to physical elements, kept in step by regeneration. | library | large | CON-A5-F1-P2, CON-A2-F6-P1 |  |
| CON-A5-F4-P2 | **Loads and combinations**. Point, line and area loads, load cases and combinations per country's loading standard. | library | medium | CON-A5-F4-P1, CON-A24-F1-P6 |  |
| CON-A5-F4-P3 | **Supports and releases**. Boundary conditions and end releases on analytical members. | library | small | CON-A5-F4-P1 |  |
| CON-A5-F5-P1 | **Analysis export**. Exports the analytical model in open formats such as the structural parts of IFC. | integration | medium | CON-A5-F4-P1, CON-A18-F1-P2 |  |
| CON-A5-F5-P2 | **Analysis links**. Links to analysis programs through their published formats or APIs, with their terms checked first (A24-F1-P7). | integration | large | CON-A5-F5-P1, CON-A24-F1-P7 |  |
| CON-A5-F5-P3 | **Results in the model**. Forces, deflections and use ratios shown as colours and tags. | interface | medium | CON-A5-F4-P1, CON-A8-F3-P2 |  |
| CON-A5-F6-P1 | **Rebar**. Rebar shapes, sets and mesh hosted in concrete, with cover and hooks, including along 3D paths. | library | large | CON-A3-F1-P1, CON-A4-F3-P1 |  |
| CON-A5-F6-P2 | **Shape codes**. Bending shape codes per country's standard as data, with the standard's terms checked first (A24-F1-P6). | data | medium | CON-A5-F6-P1, CON-A24-F1-P6 |  |
| CON-A5-F6-P3 | **Bending schedules**. Bending schedules and cut lists from placed rebar. | library | small | CON-A5-F6-P1, CON-A10-F1-P1 |  |
| CON-A5-F7-P1 | **Connections**. Parametric connection recipes for common joints. | library | large | CON-A5-F1-P2, CON-A7-F1-P1 |  |
| CON-A5-F7-P2 | **Steel fabrication output**. Shop drawings and part lists for fabricators, in open steel exchange formats. | integration | large | CON-A5-F7-P1, CON-A21-F3-P1 |  |

## CON-A6 Building services

Ducts, pipes, cable trays, electrical circuits and their systems, with sizing and calculations, in the same model as the building.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A6-F1 Route ducts and pipes | Draw ducts and pipes that add their own fittings at bends, tees and size changes. | next | P1 Ducts; P2 Pipes; P3 Automatic fittings; P4 Connectors; P5 Route suggestions; P6 Flexible ducts and pipes; P7 Placeholder runs |
| CON-A6-F2 Build systems | Group equipment and terminals into systems and see flow and direction. | next | P1 Systems; P2 System browser; P3 Gas and hot water |
| CON-A6-F3 Size and calculate | Size ducts and pipes, and check pressure loss, flow and velocity. | next | P1 Sizing; P2 Pressure loss |
| CON-A6-F4 Design electrical | Place panels, circuits, lighting, devices and cable trays, and check loads. | next | P1 Electrical elements; P2 Circuits and panels; P3 Cable trays and conduits; P4 Lighting layouts; P5 Emergency lighting |
| CON-A6-F5 Model spaces and zones for loads | Place spaces and zones and work out heating and cooling loads. | next | P1 Spaces and zones; P2 Heating and cooling loads |
| CON-A6-F6 Draw schematics | Draw single-line and riser diagrams linked to the model. | later | P1 Schematics |
| CON-A6-F7 Prepare services for fabrication | Turn ducts and pipes into fabrication parts with spool drawings. | later | P1 Fabrication parts; P2 Spool drawings |
| CON-A6-F8 Draw house drainage and plumbing plans | Draw a house's drainage and plumbing layout with pipe sizes, falls and inverts worked out, ready for a consent set. | core | P1 Drainage runs; P2 Drainage symbols and labels; P3 Plumbing and drainage loads |
| CON-A6-F9 Design fire protection | Lay out fire sprinklers and fire alarms and check their coverage. | next | P1 Sprinkler systems; P2 Fire alarm systems |

**Parts in Building services**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A6-F1-P1 | **Ducts**. Round, rectangular and oval ducts with insulation and lining. | library | large | CON-A3-F1-P1, CON-A2-F6-P1 |  |
| CON-A6-F1-P2 | **Pipes**. Pipes with slope, insulation and sizes by material. | library | large | CON-A3-F1-P1, CON-A2-F6-P1 |  |
| CON-A6-F1-P3 | **Automatic fittings**. Routing preferences that choose fittings and sizes as ducts and pipes are drawn. | library | large | CON-A6-F1-P1, CON-A6-F1-P2, CON-A7-F1-P1 |  |
| CON-A6-F1-P4 | **Connectors**. Typed connectors on families that join to ducts, pipes, conduits and circuits. | library | medium | CON-A7-F1-P1 |  |
| CON-A6-F1-P5 | **Route suggestions**. Suggests routes between connectors that avoid structure and clashes. | library | large | CON-A6-F1-P4, CON-A13-F1-P1 |  |
| CON-A6-F1-P6 | **Flexible ducts and pipes**. Flexible ducts and pipes that follow a drawn path, with their own pressure loss. | library | small | CON-A6-F1-P1, CON-A6-F1-P2 |  |
| CON-A6-F1-P7 | **Placeholder runs**. Single-line duct and pipe placeholders for early design that turn into full ducts and pipes with fittings later. | library | medium | CON-A6-F1-P3 |  |
| CON-A6-F2-P1 | **Systems**. Supply, return, exhaust, water, waste and other systems with flow direction. | library | medium | CON-A6-F1-P4 |  |
| CON-A6-F2-P2 | **System browser**. A tree of systems that shows what is not connected. | interface | small | CON-A6-F2-P1 |  |
| CON-A6-F2-P3 | **Gas and hot water**. Gas supply and hot water as system types, with each country's sizing rules as data. | data | small | CON-A6-F2-P1, CON-A24-F1-P6 |  |
| CON-A6-F3-P1 | **Sizing**. Duct and pipe sizing by friction, velocity or equal friction. | library | medium | CON-A6-F2-P1, CON-A6-F8-P3 |  |
| CON-A6-F3-P2 | **Pressure loss**. Pressure drop reports along the critical path. | library | medium | CON-A6-F2-P1 |  |
| CON-A6-F4-P1 | **Electrical elements**. Panels, devices, fixtures, conduits and cable trays as elements. | library | medium | CON-A4-F8-P1, CON-A6-F1-P4 |  |
| CON-A6-F4-P2 | **Circuits and panels**. Circuits, panel schedules, load summaries and voltage drop. | library | large | CON-A6-F4-P1 |  |
| CON-A6-F4-P3 | **Cable trays and conduits**. Trays and conduits with fittings and fill. | library | medium | CON-A6-F1-P3 |  |
| CON-A6-F4-P4 | **Lighting layouts**. Lighting layouts by spacing and target light level, with simple lighting calculations. | library | medium | CON-A6-F4-P1 |  |
| CON-A6-F4-P5 | **Emergency lighting**. Emergency lights and exit signs, with simple coverage checks along escape routes. | library | small | CON-A6-F4-P4, CON-A24-F1-P6 |  |
| CON-A6-F5-P1 | **Spaces and zones**. Spaces bounded like rooms, grouped into zones. | library | medium | CON-A4-F7-P1 |  |
| CON-A6-F5-P2 | **Heating and cooling loads**. Load calculations by an openly published method, with weather data for the place from a source whose terms are checked (A24-F1-P7). | library | large | CON-A6-F5-P1, CON-A15-F6-P1, CON-A24-F1-P7 |  |
| CON-A6-F6-P1 | **Schematics**. Single-line diagrams, risers and schematic views made from systems. | library | large | CON-A6-F2-P1, engine-draw2d |  |
| CON-A6-F7-P1 | **Fabrication parts**. Ducts and pipes split into lengths that can be made, with hanger positions. | library | large | CON-A6-F1-P1, CON-A6-F1-P2 |  |
| CON-A6-F7-P2 | **Spool drawings**. Spool sheets and cut lists for workshops. | library | medium | CON-A6-F7-P1, CON-A21-F3-P1 |  |
| CON-A6-F8-P1 | **Drainage runs**. Drain and waste lines drawn on site and floor plans with size, material and fall. Inverts are worked out along each run from a starting level on a stated datum. The 3D pipe network (A6-F1) can take the runs over later. | library | medium | CON-A9-F4-P1, CON-A9-F1-P2, CON-A2-F3-P4 |  |
| CON-A6-F8-P2 | **Drainage symbols and labels**. Gully traps, inspection points, vents and connections as symbols, with invert, fall and size labels. | data | small | CON-A6-F8-P1, CON-A9-F5-P1, CON-A9-F2-P1 |  |
| CON-A6-F8-P3 | **Plumbing and drainage loads**. Fixture units and drain sizes from the fixtures placed, per the Acceptable Solutions for NZ Building Code clause G13 first, as data, with their terms checked (A24-F1-P6). | data | medium | CON-A4-F8-P1, CON-A24-F1-P6 |  |
| CON-A6-F9-P1 | **Sprinkler systems**. Sprinkler pipes and heads laid out and checked for coverage by hazard class, to NZS 4541 first. | library | large | CON-A6-F1-P2, CON-A6-F2-P1, CON-A24-F1-P6 |  |
| CON-A6-F9-P2 | **Fire alarm systems**. Detectors, call points, sounders and alarm zones, to NZS 4512 first. | library | medium | CON-A6-F4-P1, CON-A24-F1-P6 |  |

## CON-A7 Families and content library

How parametric components are defined, edited, generated by rule or AI, and shared through one versioned library, plus materials.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A7-F1 Make a parametric component | Build a door, window or fitting with parameters, constraints, solid steps and plan symbols. | core | P1 Family recipe format; P2 Recipe evaluation; P3 Types and type tables; P4 Visibility by view; P5 Family templates; P6 Recipe limits |
| CON-A7-F2 Edit families in an editor | Open a family in an editor, flex it, and load it back into the project. | core | P1 Family editor; P2 Flex test; P3 Load and reload |
| CON-A7-F3 Nest and host components | Put components inside components with linked parameters, and make components that cut their host. | core | P1 Nested families; P2 Hosted families; P3 In-place components |
| CON-A7-F4 Generate doors and windows from a spec | Make door and window families from a spreadsheet row or a supplier catalogue. | core | P1 Door and window generator; P2 Configuration templates; P3 Generator checks |
| CON-A7-F5 Write families with AI | Describe a component in words or a photo and get a family to check and load. | next | P1 AI family writer; P2 Recipe schema for AI |
| CON-A7-F6 Find components in a shared library | Search, preview and load components from an office library and a public library. | core | P1 Library service; P2 Library search; P3 Previews; P4 Library versions; P5 Contribution terms; P6 Safety claims guard; P7 Offline library |
| CON-A7-F7 Get manufacturers' products | Load real products from manufacturers who publish their data openly. | later | P1 Manufacturer feeds; P2 Product data; P3 Manufacturer terms; P4 Verified publishers; P5 Product information |
| CON-A7-F8 Bring Revit families across | Convert Revit families into recipes where the shapes map cleanly. | next | P1 Family conversion; P2 Family reading clearance |
| CON-A7-F9 Use materials | Pick materials with drawing graphics, physical and thermal data, carbon data and render looks. | core | P1 Materials; P2 Material library; P3 Fill patterns; P4 Face paint |

**Parts in Families and content library**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A7-F1-P1 | **Family recipe format**. A text recipe holding parameters, sketches, solid steps, symbols, visibility, materials and connectors. | data | medium | CON-A2-F2-P1, CON-A3-F2-P1 | [A07](STATUS.md) |
| CON-A7-F1-P2 | **Recipe evaluation**. Recipe to solver to solid steps to kernel, inside the regeneration graph, cached per type. | library | large | CON-A7-F1-P1, CON-A3-F1-P4, CON-A2-F6-P1 |  |
| CON-A7-F1-P3 | **Types and type tables**. Named types inside a recipe, and type tables loaded from a spreadsheet. | data | small | CON-A7-F1-P1 |  |
| CON-A7-F1-P4 | **Visibility by view**. Geometry and symbols shown by view type, cut or projection and detail level. | library | medium | CON-A7-F1-P1, CON-A8-F3-P2 |  |
| CON-A7-F1-P5 | **Family templates**. Starting templates for every family kind: generic, face-hosted, wall-, floor- and ceiling-hosted, profile, annotation, tag, title block, detail item and services fitting. | data | medium | CON-A7-F1-P1, CON-A7-F3-P2 |  |
| CON-A7-F1-P6 | **Recipe limits**. Time, memory and size limits when a recipe or formula is evaluated, so a shared family cannot hang the app or fill its memory. | library | small | CON-A7-F1-P2, CON-A2-F2-P2 |  |
| CON-A7-F2-P1 | **Family editor**. An editor for reference planes, sketches, solids, parameters and types. | interface | large | CON-A7-F1-P1, CON-A3-F2-P2 |  |
| CON-A7-F2-P2 | **Flex test**. Runs every type and parameter range and reports broken geometry. | library | small | CON-A7-F1-P2 |  |
| CON-A7-F2-P3 | **Load and reload**. Load, reload and swap families in a project, keeping placed instances. | library | small | CON-A7-F1-P1 |  |
| CON-A7-F3-P1 | **Nested families**. Recipes inside recipes with linked parameters and shared nested elements. | library | medium | CON-A7-F1-P2 |  |
| CON-A7-F3-P2 | **Hosted families**. Families that declare their host and the void they cut. | library | medium | CON-A7-F1-P2, CON-A4-F3-P1 |  |
| CON-A7-F3-P3 | **In-place components**. One-off components modelled in the project, with voids that cut their hosts. | library | medium | CON-A7-F1-P2, CON-A3-F1-P2 |  |
| CON-A7-F4-P1 | **Door and window generator**. Template recipes for each configuration, filled from a spec. | library | large | CON-A7-F1-P1, CON-A7-F1-P3 |  |
| CON-A7-F4-P2 | **Configuration templates**. Single swing, double, sliding, bifold, casement, awning and fixed templates with plan and elevation symbols. | data | large | CON-A7-F1-P1 |  |
| CON-A7-F4-P3 | **Generator checks**. Valid solids, clearances, a clean opening cut and tags reading the right data. | library | small | CON-A7-F4-P1, CON-A7-F2-P2 |  |
| CON-A7-F5-P1 | **AI family writer**. An AI writes recipes through the MCP server, then runs the flex test. | integration | medium | CON-A19-F3-P1, CON-A7-F1-P1 |  |
| CON-A7-F5-P2 | **Recipe schema for AI**. A published schema with examples so any AI can write valid recipes. | data | small | CON-A7-F1-P1 |  |
| CON-A7-F6-P1 | **Library service**. One shared, versioned library loaded by reference, with office and public collections, stored on the kit's files (KIT-A15-F7-P1) and versions (KIT-A15-F3-P1). Hosting and bandwidth are metered so the cost reaches the ledger (KIT-A13-F2-P2). A candidate kit content library, since Spare Studio needs one for fonts and brand kits. | service | large | KIT-A15-F7-P1, KIT-A15-F3-P1, KIT-A13-F2-P2, CON-A7-F6-P5 |  |
| CON-A7-F6-P2 | **Library search**. Search by name, category, parameter and picture, with previews. | interface | medium | CON-A7-F6-P1 |  |
| CON-A7-F6-P3 | **Previews**. Preview images and small 3D previews made when a family is published. | service | small | CON-A7-F6-P1, CON-A8-F4-P1 |  |
| CON-A7-F6-P4 | **Library versions**. Versions per family, and a way to update projects to a new version. | library | medium | CON-A7-F6-P1, KIT-A15-F3-P1 |  |
| CON-A7-F6-P5 | **Contribution terms**. Terms anyone accepts when they publish to the public library, with a licence chooser per item, so only content the publisher may share goes in. | legal | small | KIT-A18-F4-P2 |  |
| CON-A7-F6-P6 | **Safety claims guard**. In the public library, fire ratings, CodeMark and appraisal claims show only from verified publishers (A7-F7-P4); until then they are marked as unverified. | library | small | CON-A7-F6-P1 |  |
| CON-A7-F6-P7 | **Offline library**. A local copy of the office library and chosen public collections, so components load with no connection. | library | small | CON-A7-F6-P1 |  |
| CON-A7-F7-P1 | **Manufacturer feeds**. Manufacturers publish products as recipes or product data through an open feed. | integration | medium | CON-A7-F6-P1, CON-A7-F7-P3 |  |
| CON-A7-F7-P2 | **Product data**. Dimensions, fire ratings, warranties and declarations attached to types. | data | medium | CON-A2-F2-P1 |  |
| CON-A7-F7-P3 | **Manufacturer terms**. Terms for manufacturer feeds: who answers for product data, how updates and withdrawals reach projects, and that only content the manufacturer owns goes in. | legal | small | CON-A7-F6-P5 |  |
| CON-A7-F7-P4 | **Verified publishers**. Publishers verified as real businesses through the kit (KIT-A2-F7-P1) before their types may claim a fire rating, CodeMark or appraisal. | integration | small | CON-A7-F6-P6, KIT-A2-F7-P1 |  |
| CON-A7-F7-P5 | **Product information**. Holds the product information NZ's building product information requirements make manufacturers publish, so designers can cite it. | data | small | CON-A7-F7-P2 |  |
| CON-A7-F8-P1 | **Family conversion**. Converts families from IFC or the Revit exporter into recipes, or into fixed shapes when they do not map. | library | large | CON-A18-F1-P1, CON-A18-F2-P1, CON-A7-F1-P1 |  |
| CON-A7-F8-P2 | **Family reading clearance**. Legal advice on reading family files directly, before any such work starts. | legal | small |  |  |
| CON-A7-F9-P1 | **Materials**. Materials with drawing graphics and physical and thermal data, kept apart from render appearance. | data | medium | CON-A2-F2-P1 |  |
| CON-A7-F9-P2 | **Material library**. A shared material library with patterns, textures and carbon data, from sources whose terms are checked (A24-F1-P7). | data | medium | CON-A7-F9-P1, CON-A7-F6-P1, CON-A24-F1-P7 |  |
| CON-A7-F9-P3 | **Fill patterns**. Drafting and model fill patterns, with rotation, in an open pattern format. | data | small | engine-draw2d |  |
| CON-A7-F9-P4 | **Face paint**. Paint a material onto one face, or split a face and paint part of it, kept through regeneration. | library | medium | CON-A7-F9-P1, CON-A3-F1-P1, CON-A3-F2-P1 |  |

## CON-A8 Views and graphics

Every kind of view, the drawing engine that turns 3D into correct 2D, and the graphics rules that let office standards carry across.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A8-F1 Open plans, sections and elevations | Create floor plans, ceiling plans, sections, elevations, callouts, legends and 3D views. | core | P1 View types; P2 View range; P3 Crop regions; P4 Project browser; P5 Plan regions; P6 Dependent views; P7 Legend components |
| CON-A8-F2 Get correct drawings from the model | See cut and projected lines, hidden lines, patterns and line weights drawn correctly at any scale. | core | P1 Section cut; P2 Hidden line removal; P3 Line clean-up; P4 2D output |
| CON-A8-F3 Control how things look | Set graphics by category, filter, phase and design option, and save them as view templates. | core | P1 Object and line styles; P2 Graphics rules; P3 View filters; P4 View templates; P5 Design option visibility; P6 Element overrides; P7 Linework; P8 Cut profile; P9 Linked model display |
| CON-A8-F4 Look around in 3D | Orbit, walk, cut with a section box and isolate elements in 3D views. | core | P1 Renderer; P2 3D navigation; P3 Section boxes and isolate; P4 Visual styles |
| CON-A8-F5 Show sun and shadows | Show the sun's position and shadows for a place, date and time. | core | P1 Sun position; P2 Shadows in views |
| CON-A8-F6 Work fast in huge views | Open a plan or 3D view of a very large model and keep it smooth. | next | P1 View streaming; P2 Simplified meshes; P3 Drawing cache |
| CON-A8-F7 Compare and present views | Show plans and 3D side by side, and present from saved viewpoints. | next | P1 Tiled windows; P2 Saved viewpoints |
| CON-A8-F8 Use underlays and references | Show another level, a PDF or an image under a view to trace or check against. | core | P1 Underlays; P2 PDF and image placing |

**Parts in Views and graphics**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A8-F1-P1 | **View types**. Plan, ceiling plan, section, elevation, callout, 3D, detail, drafting and legend views as elements. | library | medium | CON-A2-F1-P1 |  |
| CON-A8-F1-P2 | **View range**. Cut plane, top, bottom and view depth for each plan. | library | small | CON-A8-F1-P1 |  |
| CON-A8-F1-P3 | **Crop regions**. Crop regions, shaped crops and annotation crops. | library | small | CON-A8-F1-P1 |  |
| CON-A8-F1-P4 | **Project browser**. A tree of views, sheets, schedules and families, sorted by rules a person can set. | interface | medium | CON-A8-F1-P1 |  |
| CON-A8-F1-P5 | **Plan regions**. A local view range for part of a plan, such as a split level or a high window. | library | medium | CON-A8-F1-P2 |  |
| CON-A8-F1-P6 | **Dependent views**. Copies of a view that share its settings, used to split a large plan across sheets. | library | small | CON-A8-F1-P3 |  |
| CON-A8-F1-P7 | **Legend components**. Family types drawn in legend views, such as window and door type legends, updating when the type changes. | library | small | CON-A8-F1-P1, CON-A7-F1-P4 |  |
| CON-A8-F2-P1 | **Section cut**. Intersects solids with the cut plane to get closed regions, then clips patterns to them. | library | large | CON-A3-F1-P1, engine-draw2d |  |
| CON-A8-F2-P2 | **Hidden line removal**. Removes or dashes hidden edges per view, worked out in tiles and cached per element. | library | large | CON-A8-F2-P1, engine-draw2d | [A12](STATUS.md) |
| CON-A8-F2-P3 | **Line clean-up**. Joined elements draw without seams between layers of the same material. | library | medium | CON-A8-F2-P1 |  |
| CON-A8-F2-P4 | **2D output**. Lines at true weights at any zoom and patterns as GPU fills, from the drawing engine. | library | medium | engine-draw2d, CON-A8-F4-P1 |  |
| CON-A8-F3-P1 | **Object and line styles**. Line weights by scale, line patterns, object styles and colours as project data. | data | small | CON-A2-F1-P1 |  |
| CON-A8-F3-P2 | **Graphics rules**. Category and subcategory styles, cut and projection, halftone and transparency, per view. | library | medium | CON-A8-F3-P1 |  |
| CON-A8-F3-P3 | **View filters**. Rule-based and selection filters that change graphics or hide elements. | library | medium | CON-A8-F3-P2, CON-A2-F2-P1 |  |
| CON-A8-F3-P4 | **View templates**. Saved sets of view settings applied to many views at once. | library | small | CON-A8-F3-P2 |  |
| CON-A8-F3-P5 | **Design option visibility**. Set which option each view shows, from the interface and from the API. | library | small | CON-A2-F8-P2, CON-A8-F3-P2 |  |
| CON-A8-F3-P6 | **Element overrides**. Override lines, patterns and colour for chosen elements in one view. | library | small | CON-A8-F3-P2 |  |
| CON-A8-F3-P7 | **Linework**. Restyle single edges in one view, such as hiding a line or giving it another weight. | library | small | CON-A8-F2-P4, CON-A8-F3-P1 |  |
| CON-A8-F3-P8 | **Cut profile**. Change the cut shape of an element in one view only, without changing the model. | library | small | CON-A8-F2-P1 |  |
| CON-A8-F3-P9 | **Linked model display**. Show a linked model by the host view, by a view in the link or by custom settings, with its own line weights. | library | medium | CON-A12-F5-P1, CON-A8-F3-P2 |  |
| CON-A8-F4-P1 | **Renderer**. GPU renderer with instancing, culling and positions relative to the camera. | library | large | engine-render | [A04](STATUS.md) |
| CON-A8-F4-P2 | **3D navigation**. Orbit, pan, zoom, walk, a view orientation cube of our own design, and saved views. | interface | medium | CON-A8-F4-P1 |  |
| CON-A8-F4-P3 | **Section boxes and isolate**. Section boxes and temporary isolate and hide. | library | small | CON-A8-F4-P1, CON-A2-F4-P4 |  |
| CON-A8-F4-P4 | **Visual styles**. Wireframe, hidden line, shaded, consistent colours and realistic styles. | library | medium | CON-A8-F4-P1 |  |
| CON-A8-F5-P1 | **Sun position**. Solar position for any place and time from a published method. | library | small | CON-A2-F3-P2 |  |
| CON-A8-F5-P2 | **Shadows in views**. Shadows in 3D and plan views, with ambient light. | library | medium | CON-A8-F4-P1, CON-A8-F5-P1 |  |
| CON-A8-F6-P1 | **View streaming**. The client asks for what is in view; near chunks arrive in full detail and far ones simplified. | library | large | CON-A2-F4-P2, CON-A2-F10-P2 |  |
| CON-A8-F6-P2 | **Simplified meshes**. Simplified meshes for each chunk, made in the background. | library | medium | CON-A2-F4-P2, CON-A3-F1-P6 |  |
| CON-A8-F6-P3 | **Drawing cache**. Hidden-line results cached per element and cleared through the regeneration graph. | library | medium | CON-A8-F2-P2, CON-A2-F6-P1 |  |
| CON-A8-F7-P1 | **Tiled windows**. Several views side by side, kept in step if wanted, across screens. | interface | small | CON-A22-F1-P2 |  |
| CON-A8-F7-P2 | **Saved viewpoints**. Saved camera, section box and visibility, shareable as a link. | library | small | CON-A8-F4-P2 |  |
| CON-A8-F8-P1 | **Underlays**. Another level, a linked model, a PDF or an image shown halftone under a view. | library | small | CON-A8-F3-P2 |  |
| CON-A8-F8-P2 | **PDF and image placing**. Places PDF pages and images to scale in views. | library | medium | engine-pdf |  |

## CON-A9 Annotation and detailing

Dimensions, tags, text, keynotes, symbols and detail drawing tools that stay bound to the model.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A9-F1 Dimension the model | Place aligned, linear, angular, radial, arc and ordinate dimensions that update when the model moves. | core | P1 Dimensions; P2 Spot levels and slopes; P3 Auto dimensions |
| CON-A9-F2 Tag and label | Tag any element by category, tag everything in a view, and tag by example. | core | P1 Tags; P2 Tag all and tag by example; P3 Label formats |
| CON-A9-F3 Write notes and keynotes | Add text, general notes and keynotes. | core | P1 Text; P2 Keynotes; P3 Spelling |
| CON-A9-F4 Draw details | Draw 2D details with detail lines, filled regions, components, insulation and repeating details. | core | P1 Detail tools; P2 Detail components; P3 Detail library |
| CON-A9-F5 Add symbols and markers | Place section marks, elevation marks, north arrows, matchlines and other symbols. | core | P1 Symbols and marks; P2 Matchlines and view references |
| CON-A9-F6 Number things by rules | Number doors, rooms, sheets and views by rules, not by the order they were made. | core | P1 Rule numbering; P2 Renumber tools; P3 Naming rules engine |
| CON-A9-F7 Annotate in 3D | Dimension and tag in 3D views for presentations and site work. | later | P1 3D annotation |

**Parts in Annotation and detailing**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A9-F1-P1 | **Dimensions**. Dimensions bound to exact references, with styles, overrides and equality. | library | large | CON-A3-F1-P1, engine-draw2d |  |
| CON-A9-F1-P2 | **Spot levels and slopes**. Spot elevations, coordinates and slopes read from geometry. | library | medium | CON-A9-F1-P1, CON-A2-F3-P2 |  |
| CON-A9-F1-P3 | **Auto dimensions**. Dimension strings for walls, openings and grids made in one step. | library | medium | CON-A9-F1-P1 |  |
| CON-A9-F2-P1 | **Tags**. Tags reading parameters, with leaders, that follow their element. | library | medium | CON-A2-F2-P1, engine-text |  |
| CON-A9-F2-P2 | **Tag all and tag by example**. Tags every untagged element, or copies one tag's placement to others. | library | small | CON-A9-F2-P1 |  |
| CON-A9-F2-P3 | **Label formats**. Tag labels built from parameters, prefixes, units and formulas. | data | small | CON-A9-F2-P1 |  |
| CON-A9-F3-P1 | **Text**. Multi-line text with styles, lists, fonts and leaders. | library | medium | engine-text |  |
| CON-A9-F3-P2 | **Keynotes**. Keynotes by element, material or note, with keynote legends. | library | medium | CON-A9-F2-P1 |  |
| CON-A9-F3-P3 | **Spelling**. Spell check through the shared text engine (engine-text), which Spare Office and Spare Studio use too, with NZ English first and Spare Construct's own list of building terms. | library | small | CON-A9-F3-P1, engine-text |  |
| CON-A9-F4-P1 | **Detail tools**. Detail lines, filled and masking regions, insulation and break lines. | library | medium | engine-draw2d |  |
| CON-A9-F4-P2 | **Detail components**. 2D detail components and repeating details from the library. | library | medium | CON-A7-F1-P1, CON-A9-F4-P1 |  |
| CON-A9-F4-P3 | **Detail library**. Standard details as a collection in the shared library (A7-F6-P1), office and public, searched and dropped into a view. | data | medium | CON-A7-F6-P1, CON-A9-F4-P2 |  |
| CON-A9-F5-P1 | **Symbols and marks**. Section heads, elevation marks, view titles, north arrows and other annotation families of our own design. | library | medium | CON-A7-F1-P1 |  |
| CON-A9-F5-P2 | **Matchlines and view references**. Matchlines and view references between dependent views. | library | small | CON-A8-F1-P1 |  |
| CON-A9-F6-P1 | **Rule numbering**. Numbers by level, room, path or grid, with prefixes and gaps. | library | medium | CON-A2-F2-P1 |  |
| CON-A9-F6-P2 | **Renumber tools**. Renumber by clicking along a path, swap numbers and fix duplicates. | interface | small | CON-A9-F6-P1 |  |
| CON-A9-F6-P3 | **Naming rules engine**. One rule format for the names and numbers of sheets, views, files and documents, set per office and client, used by A11-F2-P2, A26-F3-P1 and A20-F2-P2. | library | small | CON-A9-F6-P1 |  |
| CON-A9-F7-P1 | **3D annotation**. Dimensions, tags and text on a work plane in 3D views. | library | medium | CON-A9-F1-P1, CON-A9-F2-P1, CON-A8-F4-P1 |  |

## CON-A10 Schedules, quantities and cost

Live tables over the model, quantities, spreadsheet round-trip, cost plans, classification, dashboards and handover data.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A10-F1 Make schedules | Make live tables of any element with grouping, totals, filters and formulas, and edit values in them. | core | P1 Schedule engine; P2 Schedule editor; P3 Key schedules and note blocks; P4 Sheet, view and revision lists; P5 Schedule layout |
| CON-A10-F2 Take off quantities | Get counts, lengths, areas, volumes and material quantities live from the model. | core | P1 Material takeoff; P2 Measurement rules |
| CON-A10-F3 Round-trip with spreadsheets | Export any schedule or parameter set to a spreadsheet, edit it, and bring changes back with a preview. | core | P1 Spreadsheet export and import; P2 Change preview; P3 Online spreadsheets |
| CON-A10-F4 Price the model | Link cost rates to elements and see a live cost plan by element and stage. | next | P1 Cost rates; P2 Cost plan; P3 Cost changes; P4 Takeoff from PDF drawings |
| CON-A10-F5 Classify elements | Code elements with classification systems such as the NZ one and Uniclass. | next | P1 Classification; P2 Classification licences |
| CON-A10-F6 See project data on dashboards | Send live model data to dashboards and see model health at a glance. | next | P1 Data feed; P2 Model health page; P3 Database export |
| CON-A10-F7 Hand over asset data | Produce asset registers and handover data such as COBie for facility managers. | later | P1 Handover data; P2 Asset fields |
| CON-A10-F8 Show spreadsheets on sheets | Place a live spreadsheet table on a sheet that updates when the file changes. | later | P1 Spreadsheet tables |

**Parts in Schedules, quantities and cost**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A10-F1-P1 | **Schedule engine**. Live queries over elements with fields, filters, sorting, grouping and totals. | library | large | CON-A2-F2-P1, CON-A2-F6-P1 |  |
| CON-A10-F1-P2 | **Schedule editor**. A table editor that edits model values both ways. | interface | medium | CON-A10-F1-P1 |  |
| CON-A10-F1-P3 | **Key schedules and note blocks**. Key schedules that set many values from one key, and note-block schedules. | library | small | CON-A10-F1-P1 |  |
| CON-A10-F1-P4 | **Sheet, view and revision lists**. Sheet lists, view lists and revision schedules. The drawing register in an issue (A11-F4-P2) is written from the sheet list, so there is one register. | library | small | CON-A10-F1-P1 |  |
| CON-A10-F1-P5 | **Schedule layout**. Long schedules split across sheets, with conditional formatting and images in cells. | library | medium | CON-A10-F1-P1, CON-A11-F1-P1 |  |
| CON-A10-F2-P1 | **Material takeoff**. Quantities by material and layer. | library | medium | CON-A10-F1-P1, CON-A7-F9-P1 |  |
| CON-A10-F2-P2 | **Measurement rules**. Which measurement rules each country uses for quantities (NZS 4202 in NZ) and how to set them as data, with their terms checked (A24-F1-P6). | research | medium | CON-A10-F2-P1, CON-A24-F1-P6 |  |
| CON-A10-F3-P1 | **Spreadsheet export and import**. Writes and reads xlsx and csv, keeping element IDs for the return trip. The xlsx reader and writer is a candidate shared crate, since Spare Office reads and writes xlsx too. | integration | medium | CON-A10-F1-P1 |  |
| CON-A10-F3-P2 | **Change preview**. Shows what will change before an import applies, as one undoable step. | interface | small | CON-A10-F3-P1, CON-A2-F5-P2 |  |
| CON-A10-F3-P3 | **Online spreadsheets**. Links to online spreadsheets through their published APIs and open formats, with their terms checked (A24-F1-P7). | integration | small | CON-A10-F3-P1, CON-A24-F1-P7 |  |
| CON-A10-F4-P1 | **Cost rates**. Rate tables linked to types, materials or classification codes. | data | medium | CON-A2-F2-P1 |  |
| CON-A10-F4-P2 | **Cost plan**. A live cost plan with totals by element, level, trade and stage. | library | medium | CON-A10-F4-P1, CON-A10-F2-P1 |  |
| CON-A10-F4-P3 | **Cost changes**. Shows how cost moved between two versions. | library | small | CON-A10-F4-P2, CON-A12-F4-P2 |  |
| CON-A10-F4-P4 | **Takeoff from PDF drawings**. Measure lengths, areas and counts on PDF drawings for quantity surveyors, feeding the cost plan. | library | medium | CON-A8-F8-P2, CON-A10-F4-P2 |  |
| CON-A10-F5-P1 | **Classification**. Classification codes on types and elements, with tables loaded as data. | data | small | CON-A2-F2-P1, CON-A10-F5-P2 |  |
| CON-A10-F5-P2 | **Classification licences**. Check the licence of each classification table before shipping it. | legal | small | CON-A24-F1-P7 |  |
| CON-A10-F6-P1 | **Data feed**. A read-only feed of elements and parameters for dashboard tools. | integration | medium | CON-A19-F1-P2 |  |
| CON-A10-F6-P2 | **Model health page**. A page showing the model audit's figures (A14-F4-P2) over time: warnings, file size, unused items and view counts. | interface | medium | CON-A14-F4-P2 |  |
| CON-A10-F6-P3 | **Database export**. Model data as related tables per category, for dashboard tools. | integration | small | CON-A10-F6-P1 |  |
| CON-A10-F7-P1 | **Handover data**. COBie and asset register export from the model. | integration | medium | CON-A18-F1-P2, CON-A10-F1-P1 |  |
| CON-A10-F7-P2 | **Asset fields**. Standard asset fields such as warranty, serial number and maintenance as a parameter set. | data | small | CON-A2-F2-P1 |  |
| CON-A10-F8-P1 | **Spreadsheet tables**. A spreadsheet range placed on a sheet as a table, refreshed when the file changes. | library | small | CON-A10-F3-P1, CON-A11-F1-P1 |  |

## CON-A11 Sheets, printing and issuing

Title blocks, sheets, print sets, PDF output, revisions, issue records, transmittals, markups and sharing drawings with people outside the project.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A11-F1 Lay out sheets | Place views on sheets with title blocks, guide grids and positions that line up across sheets. | core | P1 Sheets; P2 Viewports; P3 Title blocks; P4 Sheet collections |
| CON-A11-F2 Print and export to PDF | Print any set of sheets to vector PDF, paper or images, in parallel. | core | P1 Vector PDF; P2 Print sets; P3 Printers and plotters; P4 Image export; P5 Archive PDF |
| CON-A11-F3 Record revisions | Track revisions on sheets, with clouds, tags and revision schedules. | core | P1 Revisions; P2 Automatic revision clouds |
| CON-A11-F4 Issue drawing sets | Issue a set and get the issue record, transmittal and drawing register written for them. | core | P1 Issue records; P2 Transmittals and registers; P3 Send an issue; P4 Issue fingerprints; P5 Records for the longstop |
| CON-A11-F5 Lodge with councils | Package a consent set in MBIE's standard order, with forms filled from project data and supporting documents gathered, ready to lodge. | core | P1 Consent pack; P2 Council portals; P3 Forms from project data; P4 House consent checklist; P5 Supporting documents |
| CON-A11-F6 Assign and track markups | Assign markups to people and track them to done, as issues. | next | P2 Review flow |
| CON-A11-F7 Share drawings online | Share sheets and the model with anyone through a link that opens in a browser. | next | P1 Web viewer; P2 Share links; P3 Listing floor plans; P4 Link controls; P5 Redaction |
| CON-A11-F8 Answer council requests and lodge amendments | Track a council's requests for further information to their answers, and lodge amendments to a consented set. | next | P1 Council requests; P2 Amendments |
| CON-A11-F9 Review a lodged set as a council officer | As a council officer, open a lodged set in a browser, mark it up and send requests for further information. | later | P1 Officer review |
| CON-A11-F10 Follow a consent to completion | Track consent conditions and inspections, and gather the papers for a code compliance certificate. | next | P1 Conditions and inspections; P2 Completion papers |
| CON-A11-F11 Sign drawings and forms | Sign drawings and statutory forms electronically, with a record of who signed and when. | next | P1 Electronic signatures |

**Parts in Sheets, printing and issuing**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A11-F1-P1 | **Sheets**. Sheet elements with title blocks, sheet parameters and placed viewports. | library | medium | CON-A8-F1-P1, CON-A7-F1-P1 |  |
| CON-A11-F1-P2 | **Viewports**. Viewports with titles, alignment, guide grids and saved positions. | library | medium | CON-A11-F1-P1 |  |
| CON-A11-F1-P3 | **Title blocks**. Title block recipes with labels reading project and sheet data. | data | medium | CON-A7-F1-P1, CON-A2-F2-P5 |  |
| CON-A11-F1-P4 | **Sheet collections**. Groups of sheets that share parameters and print together. | library | small | CON-A11-F1-P1 |  |
| CON-A11-F2-P1 | **Vector PDF**. PDF with true line weights, embedded fonts, layers and bookmarks. | library | medium | engine-pdf, CON-A8-F2-P4 |  |
| CON-A11-F2-P2 | **Print sets**. Saved sheet sets with file names from the naming rules (A9-F6-P3), printed in parallel. | library | small | CON-A11-F2-P1, CON-A9-F6-P3 |  |
| CON-A11-F2-P3 | **Printers and plotters**. Paper sizes, scale and colour or black and white, through the system print service. | integration | small | CON-A11-F2-P1 |  |
| CON-A11-F2-P4 | **Image export**. Views and sheets saved as PNG and other image formats. | library | small | CON-A8-F4-P1 |  |
| CON-A11-F2-P5 | **Archive PDF**. Issued sets written as PDF/A, so the records still open at the end of the Building Act's 10-year longstop. | library | small | CON-A11-F2-P1, engine-pdf |  |
| CON-A11-F3-P1 | **Revisions**. Revision sequences, clouds, tags and per-sheet revision schedules. | library | medium | CON-A11-F1-P1 |  |
| CON-A11-F3-P2 | **Automatic revision clouds**. Compares each sheet with its last issue, clusters the changes and suggests clouds. | library | large | CON-A12-F4-P2, CON-A8-F2-P4 |  |
| CON-A11-F4-P1 | **Issue records**. Each issue stores the version it came from, so any issued drawing can be made again exactly. | library | medium | CON-A12-F4-P1, CON-A11-F1-P1 |  |
| CON-A11-F4-P2 | **Transmittals and registers**. Transmittals and the drawing register written on issue, with the register taken from the sheet list (A10-F1-P4). | library | small | CON-A11-F4-P1, CON-A10-F1-P4 |  |
| CON-A11-F4-P3 | **Send an issue**. Sends the issued set to recipients by email through the kit (KIT-A8-F4-P1), with a download link from the kit's file sharing (KIT-A15-F8-P1). | integration | small | CON-A11-F4-P1, KIT-A8-F4-P1, KIT-A15-F8-P1 |  |
| CON-A11-F4-P4 | **Issue fingerprints**. A hash of every issued file stored with the issue record, so it can be proven exactly what was issued. | library | small | CON-A11-F4-P1 |  |
| CON-A11-F4-P5 | **Records for the longstop**. Issue records, forms and supporting documents kept unchanged for at least the Building Act's 10-year longstop, even after an office stops paying for hosting (A23-F7-P2). | library | small | CON-A11-F4-P1, CON-A11-F4-P4, CON-A11-F2-P5 |  |
| CON-A11-F5-P1 | **Consent pack**. Drawings, specification, calculations, forms and supporting documents bundled in MBIE's standard order as searchable PDFs with bookmarks, to each council's lodging rules. | library | medium | CON-A11-F4-P1, CON-A11-F5-P4, CON-A11-F5-P5 |  |
| CON-A11-F5-P2 | **Council portals**. Which council lodging portals publish an API that allows a link, and on what terms. | research | medium | CON-A11-F5-P1 |  |
| CON-A11-F5-P3 | **Forms from project data**. Statutory forms filled from project information and design records. | data | small | CON-A2-F2-P5, CON-A24-F4-P2 |  |
| CON-A11-F5-P4 | **House consent checklist**. MBIE's standard order of documents as a checklist. Each item is tied to the core part that makes it, or marked as supplied from outside at Gate D: truss and frame designs from the plant, geotechnical reports, engineers' producer statements and the specification. | data | small | CON-A11-F4-P1 |  |
| CON-A11-F5-P5 | **Supporting documents**. Producer statements PS1 to PS4, reports, and BRANZ appraisals and CodeMark certificates linked from the types used, gathered in the order of the checklist. | library | small | CON-A11-F5-P4, CON-A2-F2-P1 |  |
| CON-A11-F6-P2 | **Review flow**. Assign, answer and close markups as issues in the one issue model (A12-F8-P3), with a record of who did what. | interface | small | CON-A26-F8-P1, CON-A12-F8-P3, KIT-A8-F3-P2 |  |
| CON-A11-F7-P1 | **Web viewer**. A read-only browser viewer for sheets and 3D. | interface | medium | CON-A2-F10-P1, CON-A8-F4-P1 |  |
| CON-A11-F7-P2 | **Share links**. Spare Construct's share rules (which sheets, views and versions) on the kit's sharing (KIT-A15-F8-P1). Link bandwidth is metered so its cost reaches the ledger (KIT-A13-F2-P2). | service | small | CON-A11-F7-P1, KIT-A15-F8-P1, KIT-A13-F2-P2 |  |
| CON-A11-F7-P3 | **Listing floor plans**. Clean marketing floor plans made from the model and exported in an open format that Spare Home imports, never through a shared crate. | library | small | CON-A8-F1-P1, CON-A11-F2-P1 |  |
| CON-A11-F7-P4 | **Link controls**. Revoke a link at any time, protect it with a password, and see who opened it. | library | small | CON-A11-F7-P2 |  |
| CON-A11-F7-P5 | **Redaction**. Withhold chosen categories or areas of the model before sharing or exporting, for prisons, banks and other secure projects. | library | medium | CON-A11-F7-P2, CON-A8-F3-P3 |  |
| CON-A11-F8-P1 | **Council requests**. Requests for further information logged against sheets and elements with due dates, and answered with revised sheets, in the one issue model. | library | medium | CON-A12-F8-P3, CON-A11-F5-P1 |  |
| CON-A11-F8-P2 | **Amendments**. An amendment lodged as the changes against the consented set, clouded and listed automatically. | library | small | CON-A11-F3-P2, CON-A11-F5-P1 |  |
| CON-A11-F9-P1 | **Officer review**. A council officer opens the lodged set in the web viewer, marks it up and sends requests that arrive in the designer's register. | interface | medium | CON-A11-F7-P1, CON-A26-F8-P1, CON-A11-F8-P1 |  |
| CON-A11-F10-P1 | **Conditions and inspections**. Consent conditions and inspections tracked against the set, with dates and results. | library | small | CON-A11-F5-P1, CON-A12-F8-P3 |  |
| CON-A11-F10-P2 | **Completion papers**. As-built drawings, the builder's record of work, producer statements PS3 and PS4 and the other papers for the code compliance certificate, gathered in one place. | library | small | CON-A11-F10-P1, CON-A11-F5-P5 |  |
| CON-A11-F11-P1 | **Electronic signatures**. Signatures on drawings, certificates of design work and producer statements through the kit's signatures (KIT-A25-F1-P2), stored with the issue fingerprint. | integration | small | CON-A11-F4-P4, CON-A24-F4-P4, KIT-A25-F1-P2 |  |

## CON-A12 Teamwork, history and versions

Many people editing one model at once, offline work, full history, versions and compare, consultant models, permissions and project documents.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A12-F1 Work together live | Edit the same model with others at once and see their changes within a moment. | next | P1 Sync server; P2 Element claims; P3 Presence; P4 Shared text; P5 Editing requests |
| CON-A12-F2 Work offline and merge | Work offline, then merge back with an element-by-element review of any conflicts. | next | P1 Branches; P2 Merge review |
| CON-A12-F3 See full history | See who changed what, when and why, and open any past state. | core | P1 History browser; P2 Element history; P3 History keeping; P4 Personal data in history |
| CON-A12-F4 Save and compare versions | Save named versions at milestones and compare any two in colour. | core | P1 Versions; P2 Version compare; P3 Compare view |
| CON-A12-F5 Link consultant models | Link consultants' models, keep each version they send and see what changed. | core | P1 Linked models; P2 Received versions; P3 Copy and monitor |
| CON-A12-F6 Control who can do what | Set who can view, edit or approve by project, discipline and category. | next | P1 Permissions; P2 Approvals; P3 Project transfer; P4 Role end; P5 Project control rules |
| CON-A12-F7 Ask questions and track submittals | Send requests for information and submittals to the right firm, with due dates, and track the answers. | next | P3 Questions and submittals |
| CON-A12-F8 Get told what matters | Get notified about mentions, assigned issues, new consultant versions and drawing issues. | next | P1 Notifications; P2 Comments on elements; P3 One issue model |
| CON-A12-F9 Take turns on a shared project | Check out a shared project to edit it while others open it read-only, until live sync arrives. | core | P1 Project check-out; P2 Check in and hand over |
| CON-A12-F10 Place one design many times | Place one linked design at several named positions on a site, such as repeated houses or terraces. | later | P1 Named positions |

**Parts in Teamwork, history and versions**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A12-F1-P1 | **Sync server**. A server-ordered operation log per project that broadcasts operations to every client. | service | large | CON-A2-F5-P1, KIT-A15-F4-P2, KIT-A15-F5-P1 | [A05](STATUS.md) |
| CON-A12-F1-P2 | **Element claims**. Light automatic claims on edited elements, with conflicts caught per element, not per file. | library | medium | CON-A12-F1-P1 |  |
| CON-A12-F1-P3 | **Presence**. Which level, view and elements each person is on, using the kit's presence (KIT-A15-F5-P2). | interface | small | CON-A12-F1-P1, KIT-A15-F5-P2 |  |
| CON-A12-F1-P4 | **Shared text**. Comments and notes edited together through the kit's text co-editing (KIT-A15-F10-P1). | library | small | KIT-A15-F10-P1 |  |
| CON-A12-F1-P5 | **Editing requests**. Ask whoever holds a claim to release it, get a reply and a notification, and let an admin release the claims of someone who has left. | library | small | CON-A12-F1-P2, CON-A12-F8-P1, CON-A23-F3-P1 |  |
| CON-A12-F2-P1 | **Branches**. Branch a project, work on it, then merge or discard it. | library | large | CON-A2-F5-P1, KIT-A15-F11-P1 |  |
| CON-A12-F2-P2 | **Merge review**. Shows conflicts per element and lets a person choose. | interface | medium | CON-A12-F2-P1 |  |
| CON-A12-F3-P1 | **History browser**. Browse the log by person, time, element or command, and open any past state. | interface | medium | CON-A2-F5-P1 |  |
| CON-A12-F3-P2 | **Element history**. Trace every change to one element. | library | small | CON-A2-F5-P1 |  |
| CON-A12-F3-P3 | **History keeping**. History kept for the life of the project unless the owner prunes it, folded into snapshots now and then. Personal data in it is handled as in A12-F3-P4. | library | small | CON-A2-F5-P1, CON-A2-F4-P2 |  |
| CON-A12-F3-P4 | **Personal data in history**. Names in the operation log, client details, photo locations and the history of staff who have left can be corrected or erased without breaking replay, using the kit's method (KIT-A15-F1-P6), retention schedules (KIT-A18-F6-P1) and photo metadata stripping (KIT-A15-F7-P3). | library | medium | CON-A2-F5-P1, KIT-A15-F1-P6, KIT-A18-F6-P1, KIT-A15-F7-P3 |  |
| CON-A12-F4-P1 | **Versions**. Named snapshots made by hand, on a schedule or on issue, costing almost nothing. | library | small | CON-A2-F5-P1 |  |
| CON-A12-F4-P2 | **Version compare**. Added, removed, moved and changed elements found from the log, with a parameter change list. | library | large | CON-A12-F4-P1 |  |
| CON-A12-F4-P3 | **Compare view**. Colour overlays in 3D and plans, and a change list with filters. | interface | medium | CON-A12-F4-P2 |  |
| CON-A12-F5-P1 | **Linked models**. Link other projects or IFC files, placed by shared coordinates. | library | medium | CON-A2-F3-P2, CON-A18-F1-P1 |  |
| CON-A12-F5-P2 | **Received versions**. Each received version kept and compared with the last. | library | small | CON-A12-F5-P1, CON-A12-F4-P2 |  |
| CON-A12-F5-P3 | **Copy and monitor**. Copy levels and grids from a link and get a warning when they move. | library | medium | CON-A12-F5-P1 |  |
| CON-A12-F6-P1 | **Permissions**. Permissions per project, per discipline and per element category. | service | medium | CON-A12-F1-P1, KIT-A15-F1-P4, KIT-A1-F7-P1 |  |
| CON-A12-F6-P2 | **Approvals**. Sign-off steps for issues and protected changes. | library | small | CON-A12-F6-P1 |  |
| CON-A12-F6-P3 | **Project transfer**. Hand a hosted project, with its history, to the client or to another office. | library | small | CON-A12-F6-P1, CON-A23-F1-P1 |  |
| CON-A12-F6-P4 | **Role end**. Remove a consultant's access when their role ends, keeping their work and history. | library | small | CON-A12-F6-P1 |  |
| CON-A12-F6-P5 | **Project control rules**. Who controls a shared project and its model at handover, or when an office closes, set out in the hosting terms. | legal | small | CON-A24-F4-P3 |  |
| CON-A12-F7-P3 | **Questions and submittals**. Requests for information and submittals with due dates and answers, as issues in the one issue model (A12-F8-P3). | library | medium | CON-A26-F1-P1, CON-A12-F8-P3, KIT-A8-F3-P2 |  |
| CON-A12-F8-P1 | **Notifications**. In-app, email and push notifications through the kit's notification router (KIT-A8-F3-P2), each one chosen by the person. | integration | small | KIT-A8-F3-P2, KIT-A8-F5-P1 |  |
| CON-A12-F8-P2 | **Comments on elements**. Comment threads pinned to elements and viewpoints, as issues in the one issue model (A12-F8-P3). | library | small | CON-A12-F1-P4, CON-A12-F8-P3 |  |
| CON-A12-F8-P3 | **One issue model**. One BCF-compatible model for issues and comments, with viewpoint, element links, status, owner and history. Clash issues, markups, element comments, RFIs and council requests all use it, and it is exported as BCF. | library | medium | CON-A2-F5-P1, CON-A8-F7-P2 |  |
| CON-A12-F9-P1 | **Project check-out**. One person checks out a project from a shared folder or server and edits a local copy; others open the last checked-in version read-only. A project file is never edited live over a network share. | library | medium | CON-A2-F4-P1, CON-A2-F5-P1 |  |
| CON-A12-F9-P2 | **Check in and hand over**. Check the project back in with a note, and tell whoever asked for it next. | library | small | CON-A12-F9-P1, KIT-A8-F3-P2 |  |
| CON-A12-F10-P1 | **Named positions**. Named positions for a linked model, so one design can be placed several times on a site. | library | small | CON-A12-F5-P1, CON-A2-F3-P2 |  |

## CON-A13 Coordination and clash

Live and batch clash across disciplines and consultant models, grouped into issues and exchanged with other coordination tools through BCF.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A13-F1 See clashes as I model | See a clash the moment a duct meets a beam, without running a test. | next | P1 Live clash; P2 Exact narrow phase |
| CON-A13-F2 Run full clash tests | Run tests between any two sets of elements or models for coordination meetings. | next | P1 Batch tests; P2 Rules and tolerances; P3 Clash sets |
| CON-A13-F3 Group clashes into issues | Turn thousands of hits into a short list of issues with owners. | next | P1 Grouping; P2 Issue tracker; P3 Clash reports |
| CON-A13-F4 Exchange issues with other tools | Send and receive issues with people on other coordination tools. | next | P1 BCF in and out |
| CON-A13-F5 Coordinate models from other software | Bring in consultants' IFC models and test them like native elements. | next | P1 IFC model clash; P2 Federated view |
| CON-A13-F6 Check access and maintenance space | Check clearance zones around plant, doors and access routes. | later | P1 Clearance zones; P2 Standard clearances |

**Parts in Coordination and clash**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A13-F1-P1 | **Live clash**. Re-tests only changed elements against their neighbours through the spatial index. | library | large | CON-A2-F4-P4, CON-A3-F1-P1 | [A13](STATUS.md) |
| CON-A13-F1-P2 | **Exact narrow phase**. Overlap and true minimum distance on exact geometry, so clearance has no false hits. | library | large | CON-A3-F1-P1 | [A13](STATUS.md) |
| CON-A13-F2-P1 | **Batch tests**. Tests between sets, categories, levels or models, run in parallel and cached per pair. | library | medium | CON-A13-F1-P2 |  |
| CON-A13-F2-P2 | **Rules and tolerances**. Ignore conditions, tolerances and clearance gaps per test. | data | small | CON-A13-F2-P1 |  |
| CON-A13-F2-P3 | **Clash sets**. Clash tests take their sets from saved selections (A20-F3-P3) or from rules. | library | small | CON-A20-F3-P3, CON-A8-F3-P3 |  |
| CON-A13-F3-P1 | **Grouping**. Groups hits by element, level, zone or system. | library | medium | CON-A13-F2-P1 |  |
| CON-A13-F3-P2 | **Issue tracker**. Clash issues in the one issue model (A12-F8-P3), with viewpoint and history, that close when the clash is gone. | library | medium | CON-A13-F3-P1, CON-A12-F8-P3, CON-A8-F7-P2 |  |
| CON-A13-F3-P3 | **Clash reports**. Meeting reports with images and owners. | library | small | CON-A13-F3-P2 |  |
| CON-A13-F4-P1 | **BCF in and out**. BCF files and the BCF API, from the published standard. | integration | medium | CON-A13-F3-P2, CON-A12-F8-P3 |  |
| CON-A13-F5-P1 | **IFC model clash**. IFC models placed by shared coordinates and tested like native elements. | library | small | CON-A18-F1-P1, CON-A13-F2-P1 |  |
| CON-A13-F5-P2 | **Federated view**. Many models in one view with visibility and colour per model. | interface | medium | CON-A12-F5-P1 |  |
| CON-A13-F6-P1 | **Clearance zones**. Maintenance and access zones in families, tested as clearance. | library | small | CON-A7-F1-P1, CON-A13-F1-P1 |  |
| CON-A13-F6-P2 | **Standard clearances**. Standard service clearances as data, NZ first, with their sources' terms checked (A24-F1-P6). | data | small | CON-A13-F6-P1, CON-A24-F1-P6 |  |

## CON-A14 Model checking, QA and building code

Rule checks on the model, building code checks with each clause cited, stage QA checklists, audits and safe clean-up.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A14-F1 Check the model against rules | Run rule sets that find errors, missing data and bad modelling. | next | P1 Rule engine; P2 Standard rule sets; P3 Rule editor; P4 Information requirements (IDS) |
| CON-A14-F2 Check against the building code | Check stairs, barriers, access, escape routes and energy against the building code, with each clause cited. | next | P1 Building code rules; P2 Clause citations; P3 Other countries' codes; P4 Support, not certify |
| CON-A14-F3 Run stage QA checklists | Run an office's checklist for a project stage and see what passes and what fails. | next | P1 QA checklists; P2 QA report; P3 AI review |
| CON-A14-F4 Audit and clean up | Find unused items, heavy content and bad modelling, and purge them safely. | core | P1 Purge; P2 Model audit |
| CON-A14-F5 Check accessible design | Check doors, ramps, toilets and routes against access rules. | later | P1 Access rules; P2 Travel routes |
| CON-A14-F6 Score weathertightness risk | Score each elevation's weathertightness risk from the model and show the risk matrix on the elevation sheets. | core | P1 Risk matrix scoring; P2 Risk matrix on elevations; P3 E2/AS1 risk factors |

**Parts in Model checking, QA and building code**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A14-F1-P1 | **Rule engine**. Rules as data over queries and geometry, with results linked to elements. | library | large | CON-A19-F1-P2 |  |
| CON-A14-F1-P2 | **Standard rule sets**. Rule sets for data completeness, naming, duplicates and geometry. | data | medium | CON-A14-F1-P1 |  |
| CON-A14-F1-P3 | **Rule editor**. Build and share rules without writing code. | interface | medium | CON-A14-F1-P1 |  |
| CON-A14-F1-P4 | **Information requirements (IDS)**. Read buildingSMART IDS files and check the model against them, and write IDS from our own rule sets. | library | medium | CON-A14-F1-P1, CON-A18-F1-P3 |  |
| CON-A14-F2-P1 | **Building code rules**. Machine-readable checks per clause, NZ Building Code first. | data | large | CON-A14-F1-P1, CON-A4-F5-P2, CON-A14-F2-P2, CON-A14-F2-P4 |  |
| CON-A14-F2-P2 | **Clause citations**. Check results cite and link the clause they test instead of quoting copyrighted standards text, as the standards clearance allows (A24-F1-P6). | legal | small | CON-A24-F1-P6 |  |
| CON-A14-F2-P3 | **Other countries' codes**. Rule packs for Australia, the UK, the US and others, added by members. | data | large | CON-A14-F2-P1, CON-A24-F1-P6 |  |
| CON-A14-F2-P4 | **Support, not certify**. Results state that they support, and do not replace, a designer's or council's own check. | legal | small | CON-A24-F4-P1 |  |
| CON-A14-F3-P1 | **QA checklists**. Office checklists per stage, with automatic and manual items. | data | small | CON-A14-F1-P1 |  |
| CON-A14-F3-P2 | **QA report**. An issues-only report with a picture of each problem. | interface | small | CON-A14-F3-P1 |  |
| CON-A14-F3-P3 | **AI review**. An AI reviews sheets and model against the checklist and reports issues. | integration | medium | CON-A19-F3-P1, CON-A14-F3-P1 |  |
| CON-A14-F4-P1 | **Purge**. Removes unused types, families, materials and styles, after a preview. | library | small | CON-A2-F1-P1 |  |
| CON-A14-F4-P2 | **Model audit**. Element counts, weight by family, imported content and warnings, recorded at each save so the model health page (A10-F6-P2) can show trends. | library | medium | CON-A2-F1-P1, CON-A2-F7-P1 |  |
| CON-A14-F5-P1 | **Access rules**. Accessible design rules per country, the NZ access standard (NZS 4121) first, with its terms checked (A24-F1-P6). | data | medium | CON-A14-F2-P1, CON-A24-F1-P6 |  |
| CON-A14-F5-P2 | **Travel routes**. Travel distance and route checks for escape and access. | library | large | CON-A4-F7-P1 |  |
| CON-A14-F6-P1 | **Risk matrix scoring**. Scores each elevation on the E2/AS1 risk factors (wind zone, storeys, roof and wall junctions, eaves, envelope complexity and decks) from the model's cladding, roofs and decks. | library | medium | CON-A14-F6-P3, CON-A8-F1-P1, CON-A4-F2-P1, CON-A4-F4-P3, CON-A17-F8-P1 |  |
| CON-A14-F6-P2 | **Risk matrix on elevations**. Places the scored risk matrix table on each elevation sheet, updating as the design changes. | library | small | CON-A14-F6-P1, CON-A11-F1-P1, CON-A10-F1-P1 |  |
| CON-A14-F6-P3 | **E2/AS1 risk factors**. The E2/AS1 risk factors and scores as data, shipped in the NZ pack. | data | small | CON-A24-F1-P6, CON-A24-F4-P1 |  |

## CON-A15 Analysis: energy, daylight and carbon

Energy, sun, daylight, embodied carbon, ventilation, acoustic and fire checks from the same model, partnering with open tools where they already do the job.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A15-F1 Check energy performance | Check a house's insulation against NZ Building Code clause H1 by the schedule or calculation method, from the R-values of its constructions. | core | P2 H1 calculation; P3 Construction R-values; P5 H1 report |
| CON-A15-F2 Study sun and shadow | Make shadow studies for chosen dates and times and place them on sheets. | core | P1 Sun studies; P2 Shadow diagrams |
| CON-A15-F3 Check daylight | Check daylight levels and glare in rooms. | later | P1 Daylight; P2 Open tool partners |
| CON-A15-F4 Count embodied carbon | See embodied carbon live by element, material and stage. | next | P1 Embodied carbon; P2 Product declarations; P3 NZ carbon rules |
| CON-A15-F5 Check ventilation, acoustics and fire | Run simple ventilation, acoustic and fire separation checks. | later | P1 Ventilation; P2 Acoustics; P3 Fire separations |
| CON-A15-F6 Simulate energy use | Simulate a building's energy use through an open engine, or send the model to other energy tools. | next | P1 Energy model; P2 Energy exchange |
| CON-A15-F7 Study wind and microclimate | Check wind comfort and microclimate around a building at concept stage. | later | P1 Open wind tools; P2 Concept wind study |

**Parts in Analysis: energy, daylight and carbon**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A15-F1-P2 | **H1 calculation**. The H1 schedule and calculation methods for houses as data and code, using the site's H1 climate zone (A17-F8-P1) and areas from the model. | data | medium | CON-A15-F1-P3, CON-A17-F8-P1, CON-A4-F7-P1, CON-A24-F1-P6, CON-A24-F4-P1 |  |
| CON-A15-F1-P3 | **Construction R-values**. R-values from layers and materials, allowing for thermal bridging. | library | medium | CON-A7-F9-P1 |  |
| CON-A15-F1-P5 | **H1 report**. The H1 calculation set out as a report for the consent set. | library | small | CON-A15-F1-P2, CON-A11-F5-P4 |  |
| CON-A15-F2-P1 | **Sun studies**. Sets of shadow plans and 3D views for chosen dates and times, built on the view shadows (A8-F5-P2). | library | small | CON-A8-F5-P2 |  |
| CON-A15-F2-P2 | **Shadow diagrams**. Shadow diagrams over neighbouring sites for planning consents. | library | small | CON-A15-F2-P1, CON-A17-F1-P3 |  |
| CON-A15-F3-P1 | **Daylight**. Daylight factor and annual daylight by an openly published simulation method. | library | large | CON-A4-F7-P1, CON-A7-F9-P1 |  |
| CON-A15-F3-P2 | **Open tool partners**. Where open tools such as Ladybug Tools already do the job, partner rather than rebuild. | research | small |  |  |
| CON-A15-F4-P1 | **Embodied carbon**. Carbon per material quantity from declared data, with totals by element and stage. | library | medium | CON-A10-F2-P1, CON-A7-F9-P2 |  |
| CON-A15-F4-P2 | **Product declarations**. Environmental product declarations as data from open sources, with their terms checked (A24-F1-P7). | data | medium | CON-A15-F4-P1, CON-A24-F1-P7 |  |
| CON-A15-F4-P3 | **NZ carbon rules**. The NZ government's building carbon rules and reporting format, kept current. | research | small | CON-A15-F4-P1 |  |
| CON-A15-F5-P1 | **Ventilation**. Natural ventilation openings checked against floor areas. | library | small | CON-A4-F7-P1, CON-A14-F2-P1 |  |
| CON-A15-F5-P2 | **Acoustics**. Sound ratings of constructions and simple checks between rooms. | library | medium | CON-A7-F9-P1 |  |
| CON-A15-F5-P3 | **Fire separations**. Fire ratings shown and checked across walls, floors and openings. | library | medium | CON-A2-F2-P1, CON-A14-F1-P1 |  |
| CON-A15-F6-P1 | **Energy model**. Thermal zones, envelope and loads from the model, run through an open simulation engine. | library | large | CON-A6-F5-P1, CON-A7-F9-P1, CON-A15-F1-P3 |  |
| CON-A15-F6-P2 | **Energy exchange**. Export to energy tools through open formats such as gbXML. | integration | small | CON-A15-F6-P1 |  |
| CON-A15-F7-P1 | **Open wind tools**. Which open tools do concept wind and comfort studies, and on what licences; GPL tools only ever run as a separate program. | research | small |  |  |
| CON-A15-F7-P2 | **Concept wind study**. Simple wind flow and comfort maps around the massing at concept stage. | library | large | CON-A15-F7-P1, CON-A3-F5-P1, CON-A17-F2-P3 |  |

## CON-A16 Visualisation, rendering and VR

Photo-quality images, walkthroughs, VR and client presentations from the same renderer, with no second product to buy.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A16-F1 Render photo-real images | Render a realistic image from any 3D view in seconds. | next | P1 Real-time rendering; P2 Final render; P3 Render looks; P4 Lights and sky; P5 Decals |
| CON-A16-F2 Add people, plants and cars | Place people, plants, cars and other items from a free library. | next | P1 Entourage library; P2 Scatter |
| CON-A16-F3 Make walkthroughs and videos | Record walkthroughs and animations and export them as video. | later | P1 Camera paths; P2 Video export; P3 Sun and build animation |
| CON-A16-F4 Walk through in VR and AR | Step into the model in a VR headset, or see it on site on a tablet. | later | P1 VR; P2 AR on site |
| CON-A16-F5 Share interactive presentations | Send a client a link to look through the model in a browser or on a phone. | next | P1 Panoramas; P2 Presentation viewer |
| CON-A16-F6 Render with AI | Turn a view into a styled image with an AI image model the member chooses. | later | P1 AI rendering; P2 AI image rules |

**Parts in Visualisation, rendering and VR**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A16-F1-P1 | **Real-time rendering**. Physically based lighting, reflections and global illumination in the renderer. | library | large | CON-A8-F4-P1 |  |
| CON-A16-F1-P2 | **Final render**. Path-traced still images for final quality. | library | large | CON-A16-F1-P1 |  |
| CON-A16-F1-P3 | **Render looks**. Physically based material looks linked to model materials. | data | medium | CON-A7-F9-P1 |  |
| CON-A16-F1-P4 | **Lights and sky**. Sun, sky, lights from light fixtures and HDR skies. | library | medium | CON-A8-F5-P1, CON-A16-F1-P1 |  |
| CON-A16-F1-P5 | **Decals**. Images placed on faces, such as signs and artwork, shown in renders. | library | small | CON-A16-F1-P3 |  |
| CON-A16-F2-P1 | **Entourage library**. People, plants, vehicles and furniture models with clear open licences, checked before they go in (A24-F1-P7). | data | medium | CON-A7-F6-P1, CON-A24-F1-P7 |  |
| CON-A16-F2-P2 | **Scatter**. Spread grass, trees and objects over terrain as light render content. Scheduled planting with sizes comes from A17-F6-P2. | library | medium | CON-A16-F2-P1, CON-A17-F1-P1 |  |
| CON-A16-F3-P1 | **Camera paths**. Camera paths and key frames. | library | small | CON-A8-F4-P2 |  |
| CON-A16-F3-P2 | **Video export**. Video export through the shared media engine. | library | medium | engine-media, CON-A16-F3-P1 |  |
| CON-A16-F3-P3 | **Sun and build animation**. Animate the sun and construction stages. | library | small | CON-A16-F3-P2, CON-A21-F4-P1 |  |
| CON-A16-F4-P1 | **VR**. OpenXR support for headsets, on the desktop and in the browser. | integration | large | CON-A16-F1-P1 |  |
| CON-A16-F4-P2 | **AR on site**. The model shown in place on a phone or tablet. | integration | large | CON-A2-F10-P1 |  |
| CON-A16-F5-P1 | **Panoramas**. 360 panoramas for any browser or phone. | library | small | CON-A16-F1-P1 |  |
| CON-A16-F5-P2 | **Presentation viewer**. A browser viewer with chosen views, design options and notes. | interface | medium | CON-A11-F7-P1 |  |
| CON-A16-F6-P1 | **AI rendering**. Sends a view and its depth map to the chosen AI image model and places the result. Use of a co-op AI is metered so its cost reaches the ledger (KIT-A13-F2-P2). | integration | medium | CON-A8-F4-P1, CON-A16-F6-P2, KIT-A13-F2-P2 |  |
| CON-A16-F6-P2 | **AI image rules**. Spare Construct's consent and labelling rules for AI images, under the kit's AI tools policy (KIT-A17-F6-P1). | legal | small | KIT-A17-F6-P1 |  |

## CON-A17 Site, survey and context

Terrain, property boundaries, open survey and aerial data, planning rules, point clouds and city context. New Zealand first, any country by the same design.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A17-F1 Model the site | Model terrain, building pads, cut and fill, and property boundaries. | core | P1 Terrain solids; P2 Pads and cut and fill; P3 Property boundaries; P4 LandXML in; P5 Contour labels |
| CON-A17-F2 Bring in the real site | Pull the real terrain, aerial photos, parcels and neighbouring buildings for an address. | next | P1 NZ open data; P2 Other countries' data; P3 Context buildings; P4 Site data licences |
| CON-A17-F3 Look up planning rules for my site | Pull a site's zone, overlays and planning rules from council data, so envelopes and checks set themselves. | next | P1 Zoning data; P3 Planning rules as data; P4 Sites of significance |
| CON-A17-F4 Use point clouds and scans | Load laser scans, model against them, and fit walls, floors and pipes to the scan. | later | P1 Point clouds; P2 Scan to model |
| CON-A17-F5 Work at city scale | Open a district of buildings, each loading in full only when someone opens it. | later | P1 District model; P2 3D Tiles out |
| CON-A17-F6 Model civil and landscape work | Model paths, kerbs, paving, site drainage and planting. | later | P1 Hardscape; P2 Planting; P3 Site drainage; P4 Parking |
| CON-A17-F7 Show planning envelopes and site coverage | Show height in relation to boundary and other envelopes on plans, elevations and sections, and report site coverage, from rules typed in or looked up. | core | P1 Planning envelopes; P2 Site coverage and planning areas; P3 Envelope lines on drawings |
| CON-A17-F8 Know my site's design zones and hazards | Record a site's wind, earthquake, snow, exposure and climate zones and its hazards, so framing, energy and weathertightness checks have their inputs. | core | P1 Site design zones; P2 Zone and hazard lookup |

**Parts in Site, survey and context**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A17-F1-P1 | **Terrain solids**. Terrain from points, contours or surfaces, as a solid with sub-regions. | library | large | CON-A3-F1-P1 |  |
| CON-A17-F1-P2 | **Pads and cut and fill**. Building pads and cut and fill volumes. | library | medium | CON-A17-F1-P1 |  |
| CON-A17-F1-P3 | **Property boundaries**. Boundaries from bearings and distances or survey data, with areas. | library | small | CON-A2-F3-P2 |  |
| CON-A17-F1-P4 | **LandXML in**. Reads survey surfaces, boundaries and points from LandXML, the format NZ surveyors deliver. | library | medium | CON-A17-F1-P1, CON-A17-F1-P3 |  |
| CON-A17-F1-P5 | **Contour labels**. Labelled contours on terrain in plans, at a chosen interval. | library | small | CON-A17-F1-P1, CON-A9-F2-P1 |  |
| CON-A17-F2-P1 | **NZ open data**. LINZ terrain, aerials, parcels and building outlines, used under their open licences. A candidate kit land data part, since Spare Home reads the same data. | integration | medium | CON-A2-F3-P3, CON-A17-F2-P4, KIT-A7-F2-P2, KIT-A7-F2-P3 |  |
| CON-A17-F2-P2 | **Other countries' data**. The same for other countries' open data, added country by country by members. | integration | large | CON-A17-F2-P1 |  |
| CON-A17-F2-P3 | **Context buildings**. Neighbouring buildings and terrain kept as light context until opened. | library | medium | CON-A17-F2-P1, CON-A8-F6-P1 |  |
| CON-A17-F2-P4 | **Site data licences**. LINZ, council and hazard data sources entered in the data licence register (A24-F1-P7), with their attribution shown on drawings. | legal | small | CON-A24-F1-P7 |  |
| CON-A17-F3-P1 | **Zoning data**. District plan zones and overlays from councils' open data, under their licences. A candidate kit land data part, shared with Spare Home. | integration | medium | CON-A17-F2-P1, CON-A17-F2-P4 |  |
| CON-A17-F3-P3 | **Planning rules as data**. Each council's rules as data, kept current by members. | data | large | CON-A17-F3-P1 |  |
| CON-A17-F3-P4 | **Sites of significance**. Overlays for sites of significance to mana whenua from district plans, cared for under the kit's Māori data principles (KIT-A18-F16-P1). | data | small | CON-A17-F3-P1, KIT-A18-F16-P1 |  |
| CON-A17-F4-P1 | **Point clouds**. E57 and LAS clouds streamed as an underlay. | library | large | CON-A8-F6-P1 |  |
| CON-A17-F4-P2 | **Scan to model**. Fit walls, floors, pipes and columns to the scan. | library | large | CON-A17-F4-P1 |  |
| CON-A17-F5-P1 | **District model**. Many buildings in one project, each with its own set of chunks. | library | large | CON-A2-F4-P2, CON-A2-F4-P3 |  |
| CON-A17-F5-P2 | **3D Tiles out**. Writes 3D Tiles for city-scale web viewers. | integration | medium | CON-A17-F5-P1 |  |
| CON-A17-F6-P1 | **Hardscape**. Paths, kerbs, roads and paving that follow the terrain. | library | medium | CON-A17-F1-P1 |  |
| CON-A17-F6-P2 | **Planting**. Plant families with sizes at planting and at maturity. | data | small | CON-A16-F2-P1 |  |
| CON-A17-F6-P3 | **Site drainage**. Site drainage layout with simple flow checks. | library | medium | CON-A6-F1-P2, CON-A17-F1-P1 |  |
| CON-A17-F6-P4 | **Parking**. Parking spaces laid out along a path or in rows, and counted in schedules. | library | small | CON-A17-F6-P1 |  |
| CON-A17-F7-P1 | **Planning envelopes**. Height in relation to boundary and other envelopes as 3D shapes from the zone's rules, typed in for the site or looked up later (A17-F3-P3), and checked against the model. | library | medium | CON-A3-F1-P1, CON-A17-F1-P3 |  |
| CON-A17-F7-P2 | **Site coverage and planning areas**. Site coverage, impermeable surface, landscaped area and outdoor living space worked out from the model and the site, with the figures on the site plan. | library | small | CON-A17-F1-P3, CON-A4-F7-P2 |  |
| CON-A17-F7-P3 | **Envelope lines on drawings**. Recession planes and height limits drawn on elevations and sections. | library | small | CON-A17-F7-P1, CON-A8-F2-P1 |  |
| CON-A17-F8-P1 | **Site design zones**. Wind, earthquake, snow and exposure zones for NZS 3604 and the H1 climate zone, stored as project data with their source and date. | data | small | CON-A2-F2-P5 |  |
| CON-A17-F8-P2 | **Zone and hazard lookup**. Looks up zones and flood, liquefaction and coastal hazard overlays for an address from sources whose licence allows it. Where none does (BRANZ Maps is not open data), the designer types the zone and cites the source. | integration | medium | CON-A17-F8-P1, CON-A24-F1-P7, KIT-A7-F2-P2 |  |

## CON-A18 File exchange

Reading and writing the formats every project already uses, built only from published specifications or work an owner has cleared after legal advice.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A18-F1 Exchange IFC | Open and save IFC 4.3 and 2x3 files that other tools and councils read cleanly. | core | P1 IFC reader; P2 IFC writer; P3 IFC mapping; P4 IFC checks; P5 IFC5 watch; P6 Data dictionary links; P7 Way-back profiles |
| CON-A18-F2 Bring Revit models in | Bring Revit projects in with their geometry and data. | core | P1 Revit exporter add-in; P2 Revit API terms; P3 Direct read clearance; P4 Editable import; P5 Revit import depth decision |
| CON-A18-F3 Exchange CAD drawings | Import, link and export DXF, and DWG once it is decided how. | core | P1 DXF in and out; P2 DWG decision; P3 CAD import and link; P4 Layer mapping; P5 DWG detail libraries |
| CON-A18-F4 Exchange 3D models | Export glTF, STL and OBJ, and read and write Rhino files, for other tools. | next | P1 glTF and GLB; P2 Rhino files; P3 STL and OBJ; P4 SketchUp import; P5 STEP and IGES in; P6 Other coordination formats |
| CON-A18-F5 Keep a live link to Rhino and Grasshopper | Send geometry and parameters between Spare Construct and Rhino as they change. | later | P1 Rhino live link |
| CON-A18-F6 Exchange data | Exchange schedules and parameters as CSV, xlsx and documented JSON. | core | P1 JSON export; P2 Parameter tables |
| CON-A18-F7 Take everything with me | Export a whole project and its history in open formats at any time. | core | P1 Full export; P3 Project record export |
| CON-A18-F8 Open outside files safely | Open consultants' files and shared content knowing a bad file cannot crash the app or take over the computer. | core | P1 Reader fuzzing |

**Parts in File exchange**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A18-F1-P1 | **IFC reader**. Reads IFC 4.3 and 2x3 into elements with their properties. | library | large | CON-A2-F1-P1, CON-A3-F1-P1 | [A09](STATUS.md) |
| CON-A18-F1-P2 | **IFC writer**. Writes IFC with mapping per category and property set. | library | large | CON-A2-F1-P1 |  |
| CON-A18-F1-P3 | **IFC mapping**. Category, property set and classification mapping tables people can edit. | data | small | CON-A18-F1-P2 |  |
| CON-A18-F1-P4 | **IFC checks**. Our files tested with the published IFC validation tools on every release. | service | small | CON-A18-F1-P2 |  |
| CON-A18-F1-P5 | **IFC5 watch**. Track IFC5 and plan support once it settles. | research | small |  |  |
| CON-A18-F1-P6 | **Data dictionary links**. Parameters linked to buildingSMART Data Dictionary entries, under its terms (A24-F1-P7). | library | small | CON-A2-F2-P3, CON-A24-F1-P7 |  |
| CON-A18-F1-P7 | **Way-back profiles**. IFC export profiles tested by re-importing into Revit and Archicad each release, so an office can always get its work back out. | service | medium | CON-A18-F1-P2, CON-A18-F1-P4 |  |
| CON-A18-F2-P1 | **Revit exporter add-in**. An add-in on Revit's published API that writes our format with exact geometry and parameters. | integration | large | CON-A2-F4-P6, CON-A18-F2-P2 |  |
| CON-A18-F2-P2 | **Revit API terms**. Check that Revit's API licence terms allow the exporter. | legal | small |  |  |
| CON-A18-F2-P3 | **Direct read clearance**. Legal advice before any direct reading of .rvt files. | legal | small |  |  |
| CON-A18-F2-P4 | **Editable import**. Imported walls, floors and doors turned into editable elements. | library | large | CON-A18-F2-P1, CON-A4-F2-P1, CON-A4-F4-P1, CON-A18-F2-P5 |  |
| CON-A18-F2-P5 | **Revit import depth decision**. A decision note on whether Revit models come in as reference geometry first or as editable elements (lean: reference first, then editable walls, floors and doors). | research | small |  |  |
| CON-A18-F3-P1 | **DXF in and out**. Our own DXF reader and writer with layers and blocks. | library | medium | engine-draw2d |  |
| CON-A18-F3-P2 | **DWG decision**. Compare an ODA licence, our own writer from public documentation, or DXF only. | research | small | CON-A18-F3-P1 |  |
| CON-A18-F3-P3 | **CAD import and link**. CAD placed or linked in views, or as 3D. | library | medium | CON-A18-F3-P1 |  |
| CON-A18-F3-P4 | **Layer mapping**. Export layer and colour rules per office or client. | data | small | CON-A18-F3-P1 |  |
| CON-A18-F3-P5 | **DWG detail libraries**. Batch import of a folder of DXF details into drafting views and the detail collection, with a guide to saving an office's DWG library as DXF in the CAD software it already licenses. The first-release path while the DWG decision is open. | library | small | CON-A18-F3-P1, CON-A9-F4-P3 |  |
| CON-A18-F4-P1 | **glTF and GLB**. glTF out for visualisation tools and the web. | library | medium | CON-A3-F1-P6 |  |
| CON-A18-F4-P2 | **Rhino files**. Reads and writes 3dm through Rhino's openly licensed file library, as meshes and flat-faced solids until curved surfaces arrive. | integration | medium | CON-A3-F1-P6, CON-A3-F7-P1, CON-A24-F1-P7 |  |
| CON-A18-F4-P3 | **STL and OBJ**. Mesh export for 3D printing and other tools. | library | small | CON-A3-F1-P6 |  |
| CON-A18-F4-P4 | **SketchUp import**. Whether SketchUp files can be read through a published SDK whose terms allow it. | research | small |  |  |
| CON-A18-F4-P5 | **STEP and IGES in**. Manufacturers' equipment models read from STEP and IGES, as meshes and flat-faced solids until curved surfaces arrive. | library | medium | CON-A3-F7-P1, CON-A3-F7-P2 |  |
| CON-A18-F4-P6 | **Other coordination formats**. For Navisworks, DGN and FBX models: a legal research note, or a published 'not supported, use IFC'. | research | small | CON-A24-F1-P7 |  |
| CON-A18-F5-P1 | **Rhino live link**. A Rhino plugin on its published SDK that sends geometry and parameters live. | integration | large | CON-A18-F4-P2, CON-A19-F1-P3 |  |
| CON-A18-F6-P1 | **JSON export**. Every element and parameter as documented JSON. | library | small | CON-A2-F4-P6 |  |
| CON-A18-F6-P2 | **Parameter tables**. Import and export parameter tables by element ID. | library | small | CON-A10-F3-P1 |  |
| CON-A18-F7-P1 | **Full export**. Project file, IFC, PDFs, history and the project record exported together in one step. | library | small | CON-A2-F4-P6, CON-A18-F1-P2, CON-A11-F2-P1, CON-A18-F7-P3 |  |
| CON-A18-F7-P3 | **Project record export**. The full export also carries the issue records, transmittal history, document register, and the office library and standards. Issues, markups, comments and RFIs join it as BCF once the issue model arrives (A12-F8-P3). | library | small | CON-A11-F4-P2, CON-A7-F6-P1 |  |
| CON-A18-F8-P1 | **Reader fuzzing**. Every file reader (IFC, DXF, PDF underlays, E57, 3dm, xlsx and family recipes) fuzzed in CI, not only the kernel. | service | medium | CON-A18-F1-P1, CON-A18-F3-P1, CON-A7-F1-P2 |  |

## CON-A19 API, scripting, AI and add-ins

One command layer for people, scripts, AI agents and add-ins, plus visual scripting, a script console and a directory of add-ins.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A19-F1 Drive everything by commands | Run any action from a script, the command line or another program. | core | P1 Command schema; P2 Query API; P3 Bindings; P4 Events; P5 API versioning; P6 Dry run; P7 Paged results; P8 Headless coverage test; P9 Local API token; P10 Python bindings decision |
| CON-A19-F2 Use the command line | Batch-convert files, print sets and run checks from a terminal or a build server. | core | P1 Command line tool; P2 Batch runs |
| CON-A19-F3 Work with AI agents | Let an AI agent model, check and document through an MCP server. | core | P1 MCP server; P2 AI guide; P5 Agent scopes; P6 Approval for binding commands; P7 Agent edits labelled; P8 Agent limits; P9 Hidden instruction guards; P10 People-only commands; P11 View snapshots for agents; P12 Long jobs over MCP |
| CON-A19-F4 Build add-ins | Build an add-in that adds commands, panels, element types and checks, without forking. | core | P1 Plugin host; P2 Plugin panels; P3 SDK and templates; P4 Plugin decision |
| CON-A19-F5 Find and install add-ins | Find, install and update add-ins from a directory, with their permissions shown first. | next | P1 Add-in directory; P2 Add-in checks; P3 Paid add-ins; P4 Add-in sales tax and payouts; P5 Add-in tax rules |
| CON-A19-F6 Script visually | Build scripts by joining nodes, without writing code. | next | P1 Node editor; P2 Node library; P3 Script conversion; P4 Script player; P5 Node packages |
| CON-A19-F7 Write quick scripts | Write and run short scripts in Python or TypeScript inside the app. | next | P1 Script console; P2 Recorded actions; P3 Script limits |
| CON-A19-F8 Explore design options by script | Run a script over ranges of inputs and compare the results against several goals. | later | P1 Optimisation studies |

**Parts in API, scripting, AI and add-ins**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A19-F1-P1 | **Command schema**. Typed commands generated from the Rust types, with docs, TypeScript types and MCP tools that never drift apart. | library | medium | KIT-A15-F1-P3 |  |
| CON-A19-F1-P2 | **Query API**. Read by category, parameter, location and relationship, with geometry on request. | library | medium | CON-A2-F1-P1, CON-A2-F4-P4 |  |
| CON-A19-F1-P3 | **Bindings**. Rust, a C interface for C# and Python, WebAssembly and TypeScript, and JSON-RPC over a socket or HTTP. | library | large | CON-A19-F1-P1, CON-A19-F1-P10 |  |
| CON-A19-F1-P4 | **Events**. Subscribe to changes such as element added or parameter changed. | library | small | KIT-A15-F1-P2 |  |
| CON-A19-F1-P5 | **API versioning**. Semantic versioning so add-ins keep working across minor releases. | library | small | CON-A19-F1-P1 |  |
| CON-A19-F1-P6 | **Dry run**. Run any command as a dry run that returns the change list without committing it. | library | small | CON-A19-F1-P1, CON-A2-F5-P2 |  |
| CON-A19-F1-P7 | **Paged results**. Large query results returned in pages, with a count first. | library | small | CON-A19-F1-P2 |  |
| CON-A19-F1-P8 | **Headless coverage test**. A CI test that fails if any interface action has no command. | service | small | CON-A19-F1-P1, CON-A22-F1-P2 |  |
| CON-A19-F1-P9 | **Local API token**. The local socket and HTTP port need a token, so a web page or another program cannot drive the model. | library | small | CON-A19-F1-P3 |  |
| CON-A19-F1-P10 | **Python bindings decision**. A decision note on Python bindings at launch or later (lean: later, since the C interface makes them cheap). | research | small |  |  |
| CON-A19-F2-P1 | **Command line tool**. Scripts, batch conversion, printing and checks from the terminal. | interface | medium | CON-A2-F10-P3 |  |
| CON-A19-F2-P2 | **Batch runs**. Runs a script over many projects in parallel. | service | small | CON-A19-F2-P1 |  |
| CON-A19-F3-P1 | **MCP server**. An MCP server on the command API with a built-in guide, on the kit's MCP server (KIT-A17-F1-P1). | integration | medium | CON-A19-F1-P1, KIT-A17-F1-P1 |  |
| CON-A19-F3-P2 | **AI guide**. A guide for agents covering commands, patterns and safe use. | data | small | CON-A19-F3-P1 |  |
| CON-A19-F3-P5 | **Agent scopes**. Each agent gets a scope per project (read only, edit, or issue) through the kit's agent sign-in and MCP authorisation (KIT-A17-F1-P2, KIT-A17-F1-P3). | library | medium | CON-A19-F3-P1, KIT-A17-F1-P2, KIT-A17-F1-P3 |  |
| CON-A19-F3-P6 | **Approval for binding commands**. Destructive or binding commands, such as deleting many elements, purging or issuing, wait for a person's approval (KIT-A17-F2-P1). | library | small | CON-A19-F3-P5, KIT-A17-F2-P1 |  |
| CON-A19-F3-P7 | **Agent edits labelled**. Every agent edit is marked as agent-made in history and in design records, and can be undone (KIT-A17-F3-P1, KIT-A17-F9-P1). | library | small | CON-A19-F3-P1, CON-A12-F3-P2, KIT-A17-F3-P1, KIT-A17-F9-P1 |  |
| CON-A19-F3-P8 | **Agent limits**. Spend and rate limits per agent, using the kit's agent limits (KIT-A17-F4-P1). | library | small | CON-A19-F3-P5, KIT-A17-F4-P1 |  |
| CON-A19-F3-P9 | **Hidden instruction guards**. Model content an agent reads (parameter text, family names and descriptions from the public library, consultants' IFC, PDF underlays) is passed as untrusted data, never as instructions, using the kit's guards (KIT-A17-F5-P1). | library | medium | CON-A19-F3-P1, KIT-A17-F5-P1 |  |
| CON-A19-F3-P10 | **People-only commands**. A declared list of commands only a person may run: issue for consent, sign a certificate of design work or a producer statement, publish to the public library, delete a project and accept a merge (KIT-A17-F9-P2). | library | small | CON-A19-F3-P5, KIT-A17-F9-P2 |  |
| CON-A19-F3-P11 | **View snapshots for agents**. Agents can render any view to an image to check their own work. | library | small | CON-A19-F3-P1, CON-A11-F2-P4 |  |
| CON-A19-F3-P12 | **Long jobs over MCP**. Long commands report progress and can be cancelled over MCP. | library | small | CON-A19-F3-P1, CON-A2-F6-P3 |  |
| CON-A19-F4-P1 | **Plugin host**. Sandboxed plugins with declared permissions, as the plugin decision settles (lean: WebAssembly components, on wasmtime natively and the browser's engine on the web), built on the kit's plugin host (KIT-A16-F5-P2). Every A20 tool sits on it, so Spare Construct asks the kit to make its plugin host core. | library | large | KIT-A16-F5-P2, CON-A19-F4-P4 | [A08](STATUS.md) |
| CON-A19-F4-P2 | **Plugin panels**. Add-ins add HTML panels and ribbon buttons. | interface | medium | CON-A19-F4-P1, CON-A22-F1-P2 |  |
| CON-A19-F4-P3 | **SDK and templates**. Templates and examples in Rust, TypeScript, C# and Python. | data | medium | CON-A19-F1-P3 |  |
| CON-A19-F4-P4 | **Plugin decision**. A decision note on WebAssembly-only plugins or a native path as well (lean: WebAssembly only). Research note A08 informs it. | research | small |  |  |
| CON-A19-F5-P1 | **Add-in directory**. Spare Construct's section of the kit's plugin directory (KIT-A16-F6-P1), filtered by API version and showing each add-in's permissions. | service | medium | CON-A19-F4-P1, KIT-A16-F6-P1 |  |
| CON-A19-F5-P2 | **Add-in checks**. Spare Construct's own checks on top of the kit's plugin review and signing (KIT-A16-F6-P2): API version, model-safety tests and a member review. | service | medium | CON-A19-F5-P1, KIT-A16-F6-P2 |  |
| CON-A19-F5-P3 | **Paid add-ins**. Optional paid add-ins sold through the kit's paid add-ins (KIT-A16-F7-P2), with only the at-cost fee taken. | integration | medium | CON-A19-F5-P1, KIT-A16-F7-P2 |  |
| CON-A19-F5-P4 | **Add-in sales tax and payouts**. NZ GST on add-ins sold through the directory, with the co-op as the supplier where NZ's rules for electronic marketplaces say so; refunds; disputes between buyer and publisher; and publisher payouts and income reporting, through the kit's tax, refunds, disputes and payouts. Sales to other countries wait for the kit's tax rules per country (KIT-A4-F7-P1). | integration | medium | CON-A19-F5-P3, CON-A19-F5-P5, KIT-A4-F1-P2, KIT-A3-F5-P1, KIT-A11-F1-P1, KIT-A3-F3-P2, KIT-A4-F3-P3 |  |
| CON-A19-F5-P5 | **Add-in tax rules**. What NZ GST on remote services, EU VAT and Australian GST require of the co-op when it sells add-ins as an electronic marketplace. The kit's tax research covers only listed services. | legal | small | KIT-A4-F1-P1 |  |
| CON-A19-F6-P1 | **Node editor**. A node graph editor over the command and query API. | interface | large | CON-A19-F1-P1, CON-A19-F1-P2 |  |
| CON-A19-F6-P2 | **Node library**. Nodes for geometry, lists, model access and maths. | library | large | CON-A19-F6-P1 |  |
| CON-A19-F6-P3 | **Script conversion**. Turns common Dynamo graphs into node graphs from their open file format, with the format's terms checked (A24-F1-P7). | library | medium | CON-A19-F6-P1, CON-A24-F1-P7 |  |
| CON-A19-F6-P4 | **Script player**. Run office scripts and node graphs from a simple panel that asks for inputs, with no editing. | interface | small | CON-A19-F6-P1 |  |
| CON-A19-F6-P5 | **Node packages**. Share node packages through the add-in directory. | library | small | CON-A19-F6-P2, CON-A19-F5-P1 |  |
| CON-A19-F7-P1 | **Script console**. A console that runs Python or TypeScript against the open model. | interface | medium | CON-A19-F1-P3 |  |
| CON-A19-F7-P2 | **Recorded actions**. Record a series of actions and replay it. | library | small | CON-A2-F5-P1 |  |
| CON-A19-F7-P3 | **Script limits**. Time and memory limits on scripts run in the console or by batch runs. | library | small | CON-A19-F7-P1 |  |
| CON-A19-F8-P1 | **Optimisation studies**. Parameter sweeps and studies with several goals over a node graph or script, with the results compared side by side. | library | medium | CON-A19-F6-P1, CON-A19-F2-P2 |  |

## CON-A20 Production tools

The productivity tools people buy add-ins for today, built as first-party add-ins on the public API so each one is also an example for add-in authors.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A20-F1 Make room plans automatically | Make room plans, elevations, crops, sheets and schedules for every room in one step. | next | P1 Room plans tool; P2 Room plan settings |
| CON-A20-F2 Build sheet sets from a template | Create the sheets, views and title blocks for a whole set from a template. | next | P1 Sheet set builder; P2 Naming rules |
| CON-A20-F3 Edit many elements at once | Batch edit parameters, rename, find and replace, and select by rule across the model. | core | P1 Batch edit; P2 Find and replace; P3 Selection tools |
| CON-A20-F4 Manage parameters across families | Add, map, copy and audit parameters across many families and projects at once. | next | P1 Parameter manager; P2 Parameter audit |
| CON-A20-F5 Manage views and sheets in bulk | Duplicate, rename, apply templates, align and clean up views and sheets in bulk. | next | P1 View tools; P2 View clean-up |
| CON-A20-F6 Colour the model by data | Colour elements in a view by any parameter value, with a legend. | next | P1 Colour by value |
| CON-A20-F7 Join and tidy geometry in bulk | Join walls, floors and columns, and fix common modelling faults across a whole level. | next | P1 Bulk join; P2 Fix common faults |
| CON-A20-F8 Match office standards | Copy standards between projects and check a project against the office standard. | next | P1 Standards transfer; P2 Standards check |
| CON-A20-F9 Link keynotes to the specification | Keep keynotes, types and the specification in step. | next | P1 Specification links; P2 Specification licences |
| CON-A20-F10 Get concept designs from a site | Get design options for a site from its planning rules, then open one as a model to develop. | later | P1 Concept generator; P2 AI design options |

**Parts in Production tools**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A20-F1-P1 | **Room plans tool**. Creates plans, elevations and sheets per room from a template. | library | medium | CON-A4-F7-P1, CON-A11-F1-P1, CON-A19-F4-P1 |  |
| CON-A20-F1-P2 | **Room plan settings**. Settings files for crops, scales, view templates and title blocks, not hard-coded to one firm. | data | small | CON-A20-F1-P1 |  |
| CON-A20-F2-P1 | **Sheet set builder**. Sheets and views made and placed by rules. | library | medium | CON-A11-F1-P1, CON-A11-F1-P2 |  |
| CON-A20-F2-P2 | **Naming rules**. Sheet and view names and numbers, as rule sets for the one naming rules engine (A9-F6-P3). | data | small | CON-A20-F2-P1, CON-A9-F6-P3 |  |
| CON-A20-F3-P1 | **Batch edit**. Change parameters on a filtered set in one step. | library | small | CON-A2-F2-P1, CON-A2-F5-P2 |  |
| CON-A20-F3-P2 | **Find and replace**. Find and replace across names, parameters and text. | library | small | CON-A19-F1-P2 |  |
| CON-A20-F3-P3 | **Selection tools**. Select by type, parameter, view or rule, and save selections that clash tests and other tools reuse. | interface | small | CON-A19-F1-P2 |  |
| CON-A20-F4-P1 | **Parameter manager**. Adds shared parameters to many families at once and copies values. | library | medium | CON-A2-F2-P3, CON-A7-F1-P1 |  |
| CON-A20-F4-P2 | **Parameter audit**. Reports missing and inconsistent values. | library | small | CON-A14-F1-P1 |  |
| CON-A20-F5-P1 | **View tools**. Bulk duplicate, rename, apply templates and align views across sheets. | library | small | CON-A8-F3-P4, CON-A11-F1-P2 |  |
| CON-A20-F5-P2 | **View clean-up**. Finds and removes unused views and empty sheets. | library | small | CON-A14-F4-P1 |  |
| CON-A20-F6-P1 | **Colour by value**. Temporary or saved overrides that colour any element by parameter value, using the colour scheme engine of room colour fills (A4-F7-P4). | library | small | CON-A8-F3-P3, CON-A4-F7-P4 |  |
| CON-A20-F7-P1 | **Bulk join**. Joins and unjoins geometry by category rules across a selection. | library | small | CON-A4-F2-P2, CON-A3-F1-P2 |  |
| CON-A20-F7-P2 | **Fix common faults**. Fixes duplicates, tiny gaps and walls that just miss each other, with a preview. | library | medium | CON-A2-F7-P1 |  |
| CON-A20-F8-P1 | **Standards transfer**. Copies types, styles, templates and parameters between projects. | library | medium | CON-A1-F3-P2 |  |
| CON-A20-F8-P2 | **Standards check**. Compares a project with the office standard and fixes the differences. | library | medium | CON-A20-F8-P1, CON-A14-F1-P1 |  |
| CON-A20-F9-P1 | **Specification links**. Specification clauses linked to types, materials and keynotes. | library | medium | CON-A2-F2-P1, CON-A9-F3-P2, CON-A20-F9-P2 |  |
| CON-A20-F9-P2 | **Specification licences**. Use specification systems only through their published formats and licences. | legal | small | CON-A24-F1-P7 |  |
| CON-A20-F10-P1 | **Concept generator**. Generates massing options that fit the site's planning envelope and brief. | library | large | CON-A17-F7-P1, CON-A3-F5-P1 |  |
| CON-A20-F10-P2 | **AI design options**. An AI proposes and explains options through the MCP server. | integration | medium | CON-A19-F3-P1, CON-A20-F10-P1 |  |

## CON-A21 Construction, fabrication and prefab

Timber framing, prefab panels, shop drawings, building timelines and site tracking for the people who build it.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A21-F1 Design house framing and bracing | Size framing members from NZS 3604's tables, design bracing, and draw the framing and bracing plans a house consent needs. | core | P2 NZS 3604 rules; P3 NZS 3604 tables clearance; P4 Bracing calculation; P6 Member sizing; P7 Bracing elements; P8 Framing and bracing plans |
| CON-A21-F2 Make prefab panels | Split walls and floors into panels within lifting and transport limits. | later | P1 Panel splitting; P2 Machine output |
| CON-A21-F3 Make shop drawings | Make assembly and shop drawings with cut lists. | later | P1 Shop drawings; P2 Cut lists |
| CON-A21-F4 Plan the build over time | Link elements to a construction programme and play the build in 3D. | later | P1 Construction timeline; P2 Programme import |
| CON-A21-F5 Track work on site | Mark elements as installed or checked on site from a tablet, even with no signal. | later | P1 Site status; P2 Offline on site |
| CON-A21-F6 Generate framing in 3D | Generate every stud, plate, nog, lintel, joist and rafter in 3D from the walls, floors and roofs. | next | P1 Framing generator; P2 Light steel framing |
| CON-A21-F7 Work with frame and truss plants | Send truss and frame layouts to the plant, and bring their designs and statements back into the project. | next | P1 Layouts to the plant; P2 Plant designs back; P3 Plant formats |
| CON-A21-F8 Run the build phase | Log defects against elements and rooms, issue site instructions and variations from the model, and record practical completion. | later | P1 Defects and snags; P2 Site instructions and variations; P3 Practical completion |

**Parts in Construction, fabrication and prefab**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A21-F1-P2 | **NZS 3604 rules**. Span tables, bracing and fixing rules as data. | data | large | CON-A21-F1-P3 |  |
| CON-A21-F1-P3 | **NZS 3604 tables clearance**. The terms for putting NZS 3604's span, bracing and fixing tables into data, under the standards clearance (A24-F1-P6). | legal | small | CON-A24-F1-P6 |  |
| CON-A21-F1-P4 | **Bracing calculation**. Bracing demand from the site's wind and earthquake zones, and bracing achieved per level and direction from the bracing elements placed. | library | medium | CON-A21-F1-P2, CON-A21-F1-P7, CON-A17-F8-P1 |  |
| CON-A21-F1-P6 | **Member sizing**. Joists, bearers, rafters, purlins and lintels sized from NZS 3604's span tables, with each size shown in tags and schedules. | library | medium | CON-A21-F1-P2, CON-A9-F2-P1 |  |
| CON-A21-F1-P7 | **Bracing elements**. Bracing elements placed on wall lines, each with its type and rating. | library | small | CON-A4-F2-P1, CON-A21-F1-P2 |  |
| CON-A21-F1-P8 | **Framing and bracing plans**. Subfloor, floor and roof framing plans and a bracing plan with its calculation sheet, drawn as views with framing lines, tags and schedules until the 3D generator (A21-F6-P1) arrives. | library | medium | CON-A21-F1-P4, CON-A21-F1-P6, CON-A8-F1-P1, CON-A9-F4-P1 |  |
| CON-A21-F2-P1 | **Panel splitting**. Panels by size, weight and joint rules. | library | large | CON-A21-F6-P1, CON-A4-F9-P2 |  |
| CON-A21-F2-P2 | **Machine output**. Output for framing and CNC machines in open formats, BTLx (a published timber machining format) first, with its terms checked (A24-F1-P7). | integration | large | CON-A21-F2-P1, CON-A24-F1-P7 |  |
| CON-A21-F3-P1 | **Shop drawings**. Assembly views, dimensions and part lists made by rules. | library | medium | CON-A2-F8-P4, CON-A11-F1-P1 |  |
| CON-A21-F3-P2 | **Cut lists**. Cut lists with stock length optimisation. | library | small | CON-A21-F3-P1 |  |
| CON-A21-F4-P1 | **Construction timeline**. Tasks linked to elements, with dates and playback. | library | medium | CON-A2-F8-P1 |  |
| CON-A21-F4-P2 | **Programme import**. Imports tasks from open project schedule formats, with their terms checked (A24-F1-P7). | integration | small | CON-A21-F4-P1, CON-A24-F1-P7 |  |
| CON-A21-F5-P1 | **Site status**. Status, photos and checks on elements from a tablet. | interface | medium | CON-A2-F10-P1 |  |
| CON-A21-F5-P2 | **Offline on site**. Works without a signal and syncs later. | library | small | CON-A12-F2-P1 |  |
| CON-A21-F6-P1 | **Framing generator**. Studs, plates, nogs, lintels, joists and rafters made by rules from the walls, floors and roofs. | library | large | CON-A4-F2-P1, CON-A4-F4-P1, CON-A4-F4-P3, CON-A21-F1-P2, CON-A21-F1-P6 |  |
| CON-A21-F6-P2 | **Light steel framing**. Light steel framing by profile and rules. | library | large | CON-A21-F6-P1, CON-A24-F1-P6 |  |
| CON-A21-F7-P1 | **Layouts to the plant**. Roof truss and wall frame layouts sent to the frame and truss plant as IFC, PDF and open data. | integration | medium | CON-A21-F1-P8, CON-A18-F1-P2, CON-A21-F7-P3 |  |
| CON-A21-F7-P2 | **Plant designs back**. Truss and frame designs and the fabricator's producer statement brought back, placed in the model and filed with the supporting documents. | integration | medium | CON-A21-F7-P1, CON-A11-F5-P5, CON-A3-F7-P1 |  |
| CON-A21-F7-P3 | **Plant formats**. Which formats NZ frame and truss plants accept and return, with their terms checked. | research | small | CON-A24-F1-P7 |  |
| CON-A21-F8-P1 | **Defects and snags**. Defects linked to elements and rooms, with photos, owners and close-out, in the one issue model. | library | medium | CON-A12-F8-P3, CON-A21-F5-P1 |  |
| CON-A21-F8-P2 | **Site instructions and variations**. Site instructions and variations issued from the model, with their record kept. | library | small | CON-A11-F4-P1, CON-A12-F8-P3 |  |
| CON-A21-F8-P3 | **Practical completion**. Practical completion records with the outstanding defects attached. | library | small | CON-A21-F8-P1 |  |

## CON-A22 Interface, accessibility and languages

The app shell and panels, use by everyone including people with disabilities, and every language and country.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A22-F1 Use a clear, fast interface | Find any tool from a ribbon, a command search or a shortcut, with properties and browser panels. | core | P1 Desktop shell; P2 Interface panels; P3 Command search; P4 Look and feel; P5 Interface study; P6 Desktop shell decision |
| CON-A22-F2 Work across screens and devices | Spread views across monitors, and use a tablet, touch screen or pen. | next | P1 Many screens; P2 Touch and pen |
| CON-A22-F3 Use it by keyboard or screen reader | Use every panel by keyboard and screen reader, with high contrast and large text. | core | P1 Accessible panels; P2 Accessible views; P3 Colour and contrast; P4 Accessibility testing; P5 Typed input |
| CON-A22-F4 Use it in my language | Use the app in English, te reo Māori and other languages, including right-to-left scripts. | next | P1 Translation system; P2 Te reo Māori; P3 More languages; P4 Scripts and fonts; P5 Local number formats; P6 Library content per language; P7 Bilingual annotation |
| CON-A22-F5 Work to my country's standards | Get the units, paper sizes, codes, templates and rules for the country a project is in. | core | P1 Country packs; P2 NZ pack; P3 Imperial units |
| CON-A22-F6 Arrange my workspace | Rearrange panels, make custom tool tabs and save workspaces by role. | next | P1 Workspaces; P2 Custom tabs |
| CON-A22-F7 Get accessible outputs and viewers | Read schedules, registers and reports as tagged PDFs, use the web viewers by keyboard or screen reader, and follow lessons with captions. | next | P1 Tagged PDFs; P2 Accessible viewers; P3 Captions |
| CON-A22-F8 Work to Australian rules | Use Spare Construct on an Australian project with its energy rules, ratings and framing standard. | later | P1 Australian pack; P2 NatHERS route |

**Parts in Interface, accessibility and languages**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A22-F1-P1 | **Desktop shell**. The desktop shell the decision note picks (lean: Tauri), with the native core in the same process. | integration | medium | CON-A2-F10-P3, CON-A22-F1-P6 |  |
| CON-A22-F1-P2 | **Interface panels**. HTML and TypeScript panels: ribbon, properties, project browser, status bar and options bar. | interface | large | CON-A19-F1-P1 |  |
| CON-A22-F1-P3 | **Command search**. Type to find and run any command. | interface | small | CON-A19-F1-P1 |  |
| CON-A22-F1-P4 | **Look and feel**. Follows the co-op design guide and tokens, with Spare Construct's own colour and icons of our own. | interface | small |  |  |
| CON-A22-F1-P5 | **Interface study**. Study open editors and watch people work, without copying any product's interface. | research | medium |  |  |
| CON-A22-F1-P6 | **Desktop shell decision**. A decision note on Tauri or our own WebView2 host. | research | small |  |  |
| CON-A22-F2-P1 | **Many screens**. Floating panels and views on any monitor. | interface | small | CON-A22-F1-P1 |  |
| CON-A22-F2-P2 | **Touch and pen**. Touch and pen input for markups and site use. | interface | medium | CON-A22-F1-P2 |  |
| CON-A22-F3-P1 | **Accessible panels**. Panels meet WCAG 2.2 AA, with full keyboard use. | interface | medium | CON-A22-F1-P2 |  |
| CON-A22-F3-P2 | **Accessible views**. Keyboard selection in views and spoken descriptions of selected elements. | interface | medium | CON-A8-F4-P1 |  |
| CON-A22-F3-P3 | **Colour and contrast**. Colour-blind-safe highlights, dark mode, high contrast and reduced motion. | interface | small | CON-A22-F1-P4 |  |
| CON-A22-F3-P4 | **Accessibility testing**. Spare Construct's own tests for views and drawing tools on top of the kit's accessibility testing (KIT-A19-F2-P2), with members with disabilities testing each release. | service | small | CON-A22-F3-P1, KIT-A19-F2-P2 |  |
| CON-A22-F3-P5 | **Typed input**. Every drawing tool takes typed values, such as lengths and angles, not only a precise drag. | interface | small | CON-A3-F4-P1, CON-A3-F2-P2 |  |
| CON-A22-F4-P1 | **Translation system**. Spare Construct's strings and help through the kit's translation framework (KIT-A19-F1-P1). Category, parameter and view names stay keyed by ID, so a translation never breaks a model. | library | medium | CON-A22-F1-P2, KIT-A19-F1-P1 |  |
| CON-A22-F4-P2 | **Te reo Māori**. A te reo Māori interface made with fluent speakers. | data | medium | CON-A22-F4-P1 |  |
| CON-A22-F4-P3 | **More languages**. Members add languages; right-to-left layout works. | data | large | CON-A22-F4-P1 |  |
| CON-A22-F4-P4 | **Scripts and fonts**. Annotation text in any script through the shared text engine. | library | medium | engine-text |  |
| CON-A22-F4-P5 | **Local number formats**. Decimal commas and other local number formats in dimensions and schedules, from the kit's local formats (KIT-A19-F1-P2). | library | small | CON-A2-F3-P1, KIT-A19-F1-P2 |  |
| CON-A22-F4-P6 | **Library content per language**. Library content and shared parameter names shown in each person's language. | data | small | CON-A22-F4-P1, CON-A7-F6-P1 |  |
| CON-A22-F4-P7 | **Bilingual annotation**. Text and tags in two languages, such as English and te reo Māori room names. | library | small | CON-A9-F2-P3, CON-A22-F4-P2 |  |
| CON-A22-F5-P1 | **Country packs**. Units, paper, templates, codes, classification and data sources bundled per country. The format has slots for each country's energy method and framing standard, tested against Australia's so it is not shaped only by NZ. The first release builds on the kit's NZ pack (KIT-A19-F9-P1); other countries wait for the kit's country packs (KIT-A19-F6-P1). | data | large | CON-A8-F3-P1, KIT-A19-F9-P1 |  |
| CON-A22-F5-P2 | **NZ pack**. The first full pack: NZ codes, templates, LINZ data, NZS 3604, the E2/AS1 risk factors and council rules. | data | large | CON-A22-F5-P1, CON-A14-F6-P3 |  |
| CON-A22-F5-P3 | **Imperial units**. Feet, inches and fractions as display units. | library | small | CON-A2-F3-P1 |  |
| CON-A22-F6-P1 | **Workspaces**. Saved layouts per person and per role. | interface | small | CON-A22-F1-P2 |  |
| CON-A22-F6-P2 | **Custom tabs**. Custom tabs holding any commands or scripts. | interface | small | CON-A22-F1-P2, CON-A19-F1-P1 |  |
| CON-A22-F7-P1 | **Tagged PDFs**. Schedules, registers, transmittals and reports written as tagged, accessible PDFs through the kit's accessible PDFs (KIT-A19-F11-P1). | library | small | CON-A11-F2-P1, KIT-A19-F11-P1 |  |
| CON-A22-F7-P2 | **Accessible viewers**. The web and presentation viewers usable by keyboard and screen reader. | interface | medium | CON-A11-F7-P1, CON-A16-F5-P2, KIT-A19-F2-P1 |  |
| CON-A22-F7-P3 | **Captions**. Captions on lessons and videos, through the kit's captions (KIT-A19-F4-P1). | library | small | CON-A1-F5-P2, KIT-A19-F4-P1 |  |
| CON-A22-F8-P1 | **Australian pack**. NCC energy rules and AS 1684 framing as a country pack, on the kit's country packs (KIT-A19-F6-P1). | data | large | CON-A22-F5-P1, CON-A24-F1-P6, KIT-A19-F6-P1 |  |
| CON-A22-F8-P2 | **NatHERS route**. Whether to link to accredited NatHERS rating tools or to seek accreditation. | research | small |  |  |

## CON-A23 Hosting, offices, security and running costs

Servers run by the co-op at cost or by an office itself, office administration, backups and security, and paying only real running costs.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A23-F1 Host projects on co-op servers | Store and sync projects on co-op servers and pay only the published running cost. | next | P1 Hosted service; P2 Storage charges; P3 Costs on the ledger; P4 What stays free; P5 Metering; P6 Hosting decision |
| CON-A23-F2 Run our own server | Run the server on an office's own machine or cloud account. | next | P1 Self-host package; P2 Server upgrades |
| CON-A23-F3 Manage an office | Add staff, groups and projects, set standards and see how the tools are used. | next | P1 Office admin console; P2 Standards roll-out; P3 Usage data; P4 Many machines |
| CON-A23-F4 Keep work safe | Keep projects backed up, encrypted and logged. | next | P1 Backups; P2 Encryption; P3 Admin audit log; P4 Security review |
| CON-A23-F5 Keep data where the law asks | Keep project data in the region a client or the law requires. | later | P1 Data regions; P2 Data processing terms |
| CON-A23-F6 Pay for paid extras | Pay at cost for cloud rendering, large storage or paid add-ins from one account. | later | P1 Payment flow; P2 Tax invoices; P3 Cloud computing; P4 Spend caps and alerts; P5 Project cost codes; P6 Pay on account |
| CON-A23-F7 Move or leave with my whole office | Move a whole office between co-op hosting and self-hosting, and keep read-only access and export after stopping paying. | next | P1 Office move; P2 No hostage rule |

**Parts in Hosting, offices, security and running costs**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A23-F1-P1 | **Hosted service**. Sync, storage and the library run by the co-op at cost on the kit's hosting (KIT-A22-F4-P1). | service | large | CON-A12-F1-P1, CON-A23-F1-P6, CON-A23-F7-P2, KIT-A22-F4-P1, KIT-A15-F7-P2 |  |
| CON-A23-F1-P2 | **Storage charges**. Charges only for storage and computing used, at cost, through the kit's regular payments (KIT-A3-F12-P1). | integration | medium | CON-A23-F1-P4, CON-A23-F1-P5, KIT-A3-F12-P1 | [K02](../kit/STATUS.md) |
| CON-A23-F1-P3 | **Costs on the ledger**. Running costs published every month on the kit's open cost ledger (KIT-A13-F1-P2). | integration | small | CON-A23-F1-P5, KIT-A13-F1-P2 | [K06](../kit/STATUS.md) |
| CON-A23-F1-P4 | **What stays free**. Desktop use, local files and small shared projects stay free; where the line sits is published and voted on. | legal | small | CON-A24-F6-P3, KIT-A12-F3-P1 |  |
| CON-A23-F1-P5 | **Metering**. Storage, sync, library and share-link bandwidth, AI rendering and problem-report storage metered through the kit (KIT-A13-F2-P2), so each cost reaches the ledger. | service | small | CON-A23-F1-P1, KIT-A13-F2-P2 |  |
| CON-A23-F1-P6 | **Hosting decision**. A decision note on self-hosting first, or a co-op hosted service as well (lean: self-hosted first). | research | small |  |  |
| CON-A23-F2-P1 | **Self-host package**. Spare Construct's sync server, library and storage added to the kit's self-host package (KIT-A22-F7-P1), with Spare Construct's own sizing guide. | service | medium | CON-A12-F1-P1, KIT-A22-F7-P1 |  |
| CON-A23-F2-P2 | **Server upgrades**. Upgrades without downtime, with a way back. | service | medium | CON-A23-F2-P1 |  |
| CON-A23-F3-P1 | **Office admin console**. People, groups, projects, standards and add-in policy in one place, on the kit's organisations and team management (KIT-A1-F7-P1, KIT-A1-F7-P2). | interface | medium | CON-A12-F6-P1, KIT-A1-F7-P1, KIT-A1-F7-P2 |  |
| CON-A23-F3-P2 | **Standards roll-out**. Pushes templates, libraries and settings to every desk. | service | small | CON-A23-F3-P1, CON-A7-F6-P1 |  |
| CON-A23-F3-P3 | **Usage data**. Opt-in usage data per office, for training and support. | service | small | CON-A23-F3-P1, CON-A24-F2-P2 |  |
| CON-A23-F3-P4 | **Many machines**. Install and update on many machines at once with the office's own device tools. A candidate kit part, since Spare OS manages machines too. | integration | small | CON-A1-F1-P1, CON-A1-F1-P5 |  |
| CON-A23-F4-P1 | **Backups**. Spare Construct's restore test on top of the kit's backups and restore drills (KIT-A22-F3-P1): a restored project must replay to the identical model. | service | medium | CON-A23-F1-P1, CON-A2-F9-P4, KIT-A22-F3-P1 |  |
| CON-A23-F4-P2 | **Encryption**. Encryption in transit and at rest, with keys held by the office as an option. | service | medium | CON-A23-F1-P1, KIT-A22-F5-P5 |  |
| CON-A23-F4-P3 | **Admin audit log**. Office admin actions in Spare Construct (permissions, transfers, released claims) recorded as operations (KIT-A15-F1-P2) and shown with the kit's admin log (KIT-A22-F6-P1). | library | small | KIT-A15-F1-P2, KIT-A22-F6-P1 |  |
| CON-A23-F4-P4 | **Security review**. Spare Construct's scope for the kit's outside security reviews (KIT-A22-F5-P3): the local API, the add-in sandbox, file readers and the sync server, before any hosted release. | research | medium | CON-A23-F1-P1, KIT-A22-F5-P3 |  |
| CON-A23-F5-P1 | **Data regions**. Spare Construct projects stored and synced in the region set per project, through the kit's data location (KIT-A18-F9-P1). | service | medium | CON-A23-F1-P1, CON-A23-F5-P2, KIT-A18-F9-P1 |  |
| CON-A23-F5-P2 | **Data processing terms**. Data processing terms for offices and their clients. | legal | small |  |  |
| CON-A23-F6-P1 | **Payment flow**. Spare Construct's charges sent through the kit's checkout (KIT-A3-F1-P3) and regular payments (KIT-A3-F12-P1). | integration | small | KIT-A3-F1-P3, KIT-A3-F12-P1 | [K02](../kit/STATUS.md) |
| CON-A23-F6-P2 | **Tax invoices**. Spare Construct's invoice lines, with project cost codes, on the kit's tax invoices (KIT-A4-F2-P1). | library | small | CON-A23-F6-P1, CON-A23-F6-P5, KIT-A4-F2-P1 |  |
| CON-A23-F6-P3 | **Cloud computing**. Rendering, analysis and batch runs on co-op servers, charged at cost. | service | medium | CON-A23-F1-P1, CON-A2-F10-P3 |  |
| CON-A23-F6-P4 | **Spend caps and alerts**. Spend caps and alerts per office and per project for cloud rendering and storage. | service | small | CON-A23-F1-P5 |  |
| CON-A23-F6-P5 | **Project cost codes**. Charges tagged per project, so offices can recharge clients for disbursements. | library | small | CON-A23-F1-P5 |  |
| CON-A23-F6-P6 | **Pay on account**. Offices pay on account with one invoice a month, through the kit's business accounts (KIT-A3-F17-P1). | integration | small | CON-A23-F6-P1, KIT-A3-F17-P1 |  |
| CON-A23-F7-P1 | **Office move**. Projects, library, standards and people moved between co-op hosting and an office's own server, using the kit's account move (KIT-A22-F7-P3). | service | medium | CON-A23-F2-P1, CON-A18-F7-P1, KIT-A22-F7-P3 |  |
| CON-A23-F7-P2 | **No hostage rule**. Read-only access and full export guaranteed after an office stops paying, written into the hosting terms. | legal | small | CON-A24-F4-P1 |  |

## CON-A24 Legal, licence, community and support

The licence, proof that it is built legally, privacy, fair moderation of shared content, professional duties, community help and how members steer the work.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A24-F1 Trust that it is built legally | See the licence, where every method came from, and that no proprietary code or design was used. | core | P1 Licence choice; P2 Contribution sign-off; P3 Dependency licence check; P4 Provenance records; P5 Name and trademark check; P6 Standards clearance; P7 Data and format licences |
| CON-A24-F2 Keep my data private | Know what data leaves their machine, control it, and export or delete their account and data. | core | P1 Privacy terms; P2 Opt-in telemetry; P3 Export and delete account; P4 Security reports |
| CON-A24-F3 Report and remove bad content | Report a shared family, add-in or public page that breaks the rules, and see a fair, published decision. | next | P1 Reporting; P2 Copyright notices; P3 Public removal log; P4 Malware checks; P5 Reasons and appeals |
| CON-A24-F4 Meet professional duties | Know what the software does and does not certify, and keep the records designers need. | core | P1 Limits and duties; P2 Design records; P3 Design copyright; P4 Certificate of design work |
| CON-A24-F5 Get community help | Ask questions, share tips and find answers from other members. | next | P1 Community forum; P2 Documentation site; P3 Free courses |
| CON-A24-F6 Have a say in what gets built | Propose features and vote on priorities as a member. | next | P1 Roadmap votes; P2 Feature proposals; P3 Spare Construct's electorate |
| CON-A24-F7 Build Spare Construct with spare tokens | Pick a Spare Construct piece sized to spare tokens and have an agent do it to Spare Construct's own standard of benchmarks, truth tests and determinism checks; the repo-wide workflow lives in AGENTS.md. | core | P1 Agent workflow; P2 Agent-ready issues |

**Parts in Legal, licence, community and support**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A24-F1-P1 | **Licence choice**. MPL-2.0 core with an Apache-2.0 SDK, or an alternative, decided by an owner. | legal | small |  | [A14](STATUS.md) |
| CON-A24-F1-P2 | **Contribution sign-off**. Spare Construct follows the kit's contributor sign-off (KIT-A14-F16-P1). It adds only that kernel, file format and API changes need their code owners' review. | legal | small | CON-A24-F1-P1, KIT-A14-F16-P1 |  |
| CON-A24-F1-P3 | **Dependency licence check**. Spare Construct's ground rule 3 as a rule in the kit's dependency checks (KIT-A22-F5-P2): no GPL or AGPL code in the kernel, file format or core crates. | service | small | KIT-A22-F5-P2 |  |
| CON-A24-F1-P4 | **Provenance records**. Each Spare Construct method note records the open project studied, its licence, and whether code was reused or only the method, as for planegcs (LGPL, method only). | data | small |  |  |
| CON-A24-F1-P5 | **Name and trademark check**. Spare Construct's interface, icons and help are checked before each release so other companies' names appear only to say what we replace, under the README's legal rules for the whole repo. | legal | small |  |  |
| CON-A24-F1-P6 | **Standards clearance**. One register of every standard and Acceptable Solution whose tables or text the app encodes (NZS 3604, NZS 4121, NZS 4202, NZS 4541, NZS 4512, AS/NZS 1170, E2/AS1, the G13 and H1 documents, and section and rebar standards), with what its terms allow: cite, link, encode values, or licence. Free to read is not free to copy. Each part that encodes one waits on its entry. | legal | medium |  |  |
| CON-A24-F1-P7 | **Data and format licences**. One register of every outside dataset and format the app reads (coordinate systems, weather files, product declarations, classification tables, council and hazard data, the buildingSMART Data Dictionary, Rhino, Dynamo and programme files, BTLx), with its licence and attribution checked before the part that reads it is built. | legal | medium |  |  |
| CON-A24-F2-P1 | **Privacy terms**. Spare Construct's own privacy notes on the kit's terms (KIT-A18-F4-P2): project models hold clients' personal data, so for hosted projects the office is responsible for that data and the co-op handles it for them. | legal | small | KIT-A18-F4-P2, KIT-A18-F15-P1 |  |
| CON-A24-F2-P2 | **Opt-in telemetry**. Nothing leaves the machine without consent, and a viewer shows exactly what would be sent. | library | small | CON-A1-F2-P2 |  |
| CON-A24-F2-P3 | **Export and delete account**. Leaving through the kit's export and deletion (KIT-A18-F1-P1, KIT-A18-F2-P1) never deletes an office's projects; the person's name in project history is handled as in A12-F3-P4. | integration | small | CON-A12-F3-P4, KIT-A18-F1-P1, KIT-A18-F2-P1 |  |
| CON-A24-F2-P4 | **Security reports**. A published way to report security holes in Spare Construct, with response times. The kit has no such part yet, so this is a candidate kit part. | service | small | KIT-A22-F5-P3 |  |
| CON-A24-F3-P1 | **Reporting**. Report content in the library, the add-in directory or public shares, through the kit's reports service (KIT-A10-F3-P1). | interface | small | KIT-A10-F3-P1 |  |
| CON-A24-F3-P2 | **Copyright notices**. Notices about families, details and add-ins shared without the right to share them, through the kit's copyright notices (KIT-A10-F21-P2). A removed family keeps working in projects that already use it. | legal | small | KIT-A10-F21-P2 |  |
| CON-A24-F3-P3 | **Public removal log**. Removals from the library, the add-in directory and public shares, each with its rule, in the kit's public removal log (KIT-A10-F7-P1). | service | small | KIT-A10-F7-P1 |  |
| CON-A24-F3-P4 | **Malware checks**. Scanning of add-ins and shared files before they are listed. | service | medium | CON-A19-F5-P2 |  |
| CON-A24-F3-P5 | **Reasons and appeals**. A statement of reasons and a right of appeal when a family, add-in or share is removed (KIT-A10-F8-P1, KIT-A11-F5-P1). | integration | small | CON-A24-F3-P1, KIT-A10-F8-P1, KIT-A11-F5-P1 |  |
| CON-A24-F4-P1 | **Limits and duties**. Terms on limits, warranty and professional use, written with a lawyer. | legal | small | CON-A24-F1-P1 |  |
| CON-A24-F4-P2 | **Design records**. Who designed what and when, with people and agents marked apart, for statutory forms such as a certificate of design work. | data | small | CON-A12-F3-P1, CON-A19-F3-P7 |  |
| CON-A24-F4-P3 | **Design copyright**. Who owns models, drawings and shared content made with the app. | legal | small |  |  |
| CON-A24-F4-P4 | **Certificate of design work**. The certificate of design work filled from the design records for the licensed designer to sign: by hand at first, and electronically once kit signatures arrive (A11-F11-P1). | library | small | CON-A24-F4-P2, CON-A11-F5-P3 |  |
| CON-A24-F5-P1 | **Community forum**. A Spare Construct section in the kit's member forum (KIT-A21-F6-P1), linked from the app. | service | small | KIT-A21-F6-P1 |  |
| CON-A24-F5-P2 | **Documentation site**. Public docs, API reference and guides, generated from the same source as the in-app help (A1-F5-P1). | data | medium | CON-A1-F5-P1, CON-A19-F1-P1 |  |
| CON-A24-F5-P3 | **Free courses**. Spare Construct courses and certificates for students and staff, at no cost, on the kit's training (KIT-A25-F2-P1). | data | medium | CON-A1-F5-P2, KIT-A25-F2-P1 |  |
| CON-A24-F6-P1 | **Roadmap votes**. Members vote on Spare Construct's priorities through the kit's consultations and ballots (KIT-A12-F11-P2, KIT-A12-F3-P1). | integration | small | CON-A24-F6-P3, KIT-A12-F11-P2, KIT-A12-F3-P1 |  |
| CON-A24-F6-P2 | **Feature proposals**. Proposals with designs and token estimates, linked to work-board pieces, through the kit's suggestions and proposals (KIT-A21-F7-P1, KIT-A12-F1-P1). | interface | small | KIT-A21-F7-P1, KIT-A12-F1-P1 |  |
| CON-A24-F6-P3 | **Spare Construct's electorate**. Who votes on Spare Construct's hosting fee and free line (hosting payers, all users, contributors or add-in publishers), set through the kit's electorates (KIT-A12-F5-P2). | legal | small | KIT-A12-F5-P2 |  |
| CON-A24-F7-P1 | **Agent workflow**. What Spare Construct can copy from ArtCraft's Rust repo layout and agent workflow. | research | small |  | [A10](STATUS.md) |
| CON-A24-F7-P2 | **Agent-ready issues**. Spare Construct pieces name the benchmark, truth test or determinism check that proves them done, on top of the repo-wide piece format. | data | small |  |  |

## CON-A25 Sharing between firms

Each firm works in its own team space and shares packages of models, views, sheets and files with the other firms when it is ready. A sharing timeline shows every package and who has taken it in, and each team takes in other teams' packages when it chooses.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A25-F1 Work in my firm's team space | Work with my own firm in a team space inside the project, where our work in progress stays ours until we publish it. | core | P1 Project space; P2 Team spaces; P3 Team boundary; P4 Join a project; P5 Shared space hosting decision; P6 Sharing running costs |
| CON-A25-F2 Publish a package | Choose models, views, sheets and files and publish them to the other firms as one numbered package when they are ready. | core | P1 Packages; P2 Package contents; P3 Publish flow; P4 Saved package sets; P5 Packages from other software |
| CON-A25-F3 Follow the sharing timeline | See every team's packages on one timeline, who has taken each one in, and what changed. | core | P1 Sharing timeline; P2 Change summary; P3 New package alerts |
| CON-A25-F4 Take in a package when I'm ready | Take in another team's package when my team is ready, so nobody's work changes under them. | core | P1 Take in; P2 Before you take in; P3 Stay on or go back |
| CON-A25-F5 Compare any two packages | Compare any two packages and see what changed in 3D, in plans, on sheets and in a list. | core | P1 Package compare; P2 Compare from the timeline |
| CON-A25-F6 Control what each firm and team sees | Set who can see, publish and take in, per firm, team and package. | core | P1 Sharing permissions; P2 Firm sharing policies |
| CON-A25-F7 See every publish and take-in | See a full record of every publish and take-in: who, when, which package and which version. | core | P1 Sharing record |

**Parts in Sharing between firms**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A25-F1-P1 | **Project space**. The shared space every firm on a job joins. It holds the teams, packages, project files and the sharing record, on the kit's operation log, file service and hosting at cost (KIT-A15-F1-P2, KIT-A15-F7-P1, KIT-A22-F4-P1). A firm's own server joins it once self-hosting arrives (A23-F2). | service | large | CON-A2-F5-P1, CON-A25-F1-P5, KIT-A15-F1-P2, KIT-A15-F7-P1, KIT-A22-F4-P1 |  |
| CON-A25-F1-P2 | **Team spaces**. A team space for each firm or discipline, such as Architecture, Structure, Services and Client, with its own members, models, views, sheets and folders. Work in progress in it is seen only by the team. Members and roles come from the kit's organisations (KIT-A1-F7-P1, KIT-A1-F7-P2). | library | medium | CON-A25-F1-P1, KIT-A1-F7-P1, KIT-A1-F7-P2 |  |
| CON-A25-F1-P3 | **Team boundary**. A team's operation log and check-outs stay inside the team. In the first release people in a team take turns through project check-out (A12-F9-P1); once live sync arrives (A12-F1-P1), edits inside a team sync live. Nothing reaches another team until it is published. | library | small | CON-A25-F1-P2, CON-A12-F9-P1 |  |
| CON-A25-F1-P4 | **Join a project**. Invite a firm to a project, or ask to join one, with the lead firm accepting. Each firm brings its own people and keeps its own models. A client or other guest can join read-only through the kit's guest access (KIT-A15-F8-P2). | interface | small | CON-A25-F1-P2, KIT-A8-F4-P1, KIT-A15-F8-P2 |  |
| CON-A25-F1-P5 | **Shared space hosting decision**. A decision note on who hosts a project's shared space by default (the lead firm, the client or co-op servers) and how a firm's own server joins it. It settles the sharing side of the hosting decision (A23-F1-P6). | research | small |  |  |
| CON-A25-F1-P6 | **Sharing running costs**. Storage and transfer for team spaces, packages and project files metered per project and firm through the kit's per-account metering (KIT-A13-F2-P3), charged at cost (KIT-A15-F7-P2) and shown on the open cost ledger. | service | small | CON-A25-F1-P1, KIT-A13-F2-P3, KIT-A15-F7-P2 |  |
| CON-A25-F2-P1 | **Packages**. A package is a named, numbered version of a team's chosen models, views, sheets and files: a signed pointer into the team's operation log plus the chunks and files it covers. Chunks and files are content-addressed, the way Speckle stores objects, so packages share what did not change and publishing or taking in moves only what did. | library | large | CON-A25-F1-P2, CON-A12-F4-P1, CON-A2-F4-P2, KIT-A15-F7-P1 |  |
| CON-A25-F2-P2 | **Package contents**. Pick what goes in: whole models or chosen levels, views, sheets and folders. Contents are filtered by discipline ownership, view filters (A8-F3-P3) and permissions (A25-F6-P1), so design options and work in progress never leave unless the team chooses. | library | medium | CON-A25-F2-P1, CON-A25-F6-P1, CON-A8-F3-P3 |  |
| CON-A25-F2-P3 | **Publish flow**. Publish from one dialog that shows what is chosen, what changed since the team's last package (A25-F3-P2) and a note. A team can require an approval before a package leaves (A26-F6-P3). | interface | medium | CON-A25-F2-P2, CON-A25-F3-P2, CON-A26-F6-P3 |  |
| CON-A25-F2-P4 | **Saved package sets**. Saved choices, such as weekly coordination for Structure or a monthly set for the client, so a regular publish is one click, or runs on a schedule through the kit's durable timers (KIT-A15-F14-P1). | library | small | CON-A25-F2-P2, KIT-A15-F14-P1 |  |
| CON-A25-F2-P5 | **Packages from other software**. Firms on other software publish IFC files and PDFs as packages into their own team space. They are kept and compared like every other package (A12-F5-P2). | integration | medium | CON-A25-F2-P1, CON-A18-F1-P1, CON-A12-F5-P2 |  |
| CON-A25-F3-P1 | **Sharing timeline**. The main view of the shared space, open from the start: a row for each team, each package as a mark along it, and for each mark which teams have taken it in. Choose a mark to see its contents, note and changes, and filter by team and date. | interface | medium | CON-A25-F2-P1, CON-A25-F4-P1, CON-A22-F1-P2 |  |
| CON-A25-F3-P2 | **Change summary**. A plain summary of what changed since a team's last package: elements added, removed and changed by category, sheets changed and files added. Built from version compare (A12-F4-P2). | library | medium | CON-A25-F2-P1, CON-A12-F4-P2 |  |
| CON-A25-F3-P3 | **New package alerts**. Members are told when another team publishes, through the kit's notification router and settings (KIT-A8-F3-P2, KIT-A8-F5-P1). | integration | small | CON-A25-F2-P1, KIT-A8-F3-P2, KIT-A8-F5-P1 |  |
| CON-A25-F4-P1 | **Take in**. Each team chooses when to take in another team's package. Until then, its linked copy stays on the last package it took in. Taking in downloads only the changed chunks and updates the linked model (A12-F5-P1). | library | medium | CON-A25-F2-P1, CON-A12-F5-P1 |  |
| CON-A25-F4-P2 | **Before you take in**. Before taking in, see the change summary (A25-F3-P2) and which of the team's own views, sheets and clash tests the change touches. | interface | small | CON-A25-F4-P1, CON-A25-F3-P2 |  |
| CON-A25-F4-P3 | **Stay on or go back**. Stay on an earlier package as long as needed, or go back to one, with the reason recorded. | library | small | CON-A25-F4-P1 |  |
| CON-A25-F5-P1 | **Package compare**. Differences between any two packages: model elements through version compare (A12-F4-P2), drawings through sheet comparison (A11-F3-P2) and other files through file compare (A26-F2-P2). Two packages from different teams compare by what each holds. | library | medium | CON-A25-F2-P1, CON-A12-F4-P2, CON-A11-F3-P2, CON-A26-F2-P2 |  |
| CON-A25-F5-P2 | **Compare from the timeline**. Pick two marks on the timeline to open them in the compare view (A12-F4-P3), with the change list grouped by category and sheet. | interface | small | CON-A25-F5-P1, CON-A25-F3-P1, CON-A12-F4-P3 |  |
| CON-A25-F6-P1 | **Sharing permissions**. Who may publish for a team, who may take in, and which teams may see each package. Rights are relationships (this firm is a member of that team; that team may view this package), checked by the kit (KIT-A15-F1-P4, KIT-A15-F8-P1), so a team's rights pass down to its packages and folders. Element permissions inside a live model stay with A12-F6-P1. | service | medium | CON-A25-F1-P2, KIT-A15-F1-P4, KIT-A15-F8-P1, KIT-A1-F7-P1 |  |
| CON-A25-F6-P2 | **Firm sharing policies**. A firm's admin can lock what may leave the firm, such as no design options and no work in progress, through the kit's organisation policies (KIT-A1-F7-P5). | library | small | CON-A25-F6-P1, KIT-A1-F7-P5 |  |
| CON-A25-F7-P1 | **Sharing record**. Every publish, take-in, go-back and permission change is an operation on the kit's tamper-evident log (KIT-A15-F1-P2), searchable and exportable in the kit's organisation audit log (KIT-A1-F7-P7). The timeline is drawn from this record. | library | small | CON-A25-F2-P1, CON-A25-F4-P1, KIT-A15-F1-P2, KIT-A1-F7-P7 |  |

## CON-A26 Project files and document control

Every project file in one versioned store with the models: folders, versions and compare, naming checks, status from work in progress to archived, permissions, review and approval, issue sets with transmittals and a register, markups, search, desktop sync, retention and an audit log.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| CON-A26-F1 Keep every project file in folders | Keep models, drawings, PDFs, specifications, photos, reports and letters in project folders, in one store with the model. | core | P1 Project file store; P2 Folder templates; P3 File previews |
| CON-A26-F2 Keep every version of every file | Keep every version of every file, see who made it, compare any two and go back to one. | core | P1 File versions; P2 File compare; P3 Restore a version |
| CON-A26-F3 Name files to the standard | Have every file name checked against the project's naming standard when it is uploaded, with a fix suggested. | core | P1 Naming rules; P2 Upload check |
| CON-A26-F4 Track status and attributes | Give every file attributes such as discipline, type and revision, and a status that moves from work in progress to shared, published and archived. | core | P1 File attributes; P2 Information states; P3 Status codes |
| CON-A26-F5 Control who sees each folder and file | Set who can see, add, edit and approve in each folder and file, per firm, team and role. | core | P1 Folder and file permissions; P2 Who has access |
| CON-A26-F6 Review and approve | Send files and packages for review, get comments and decisions from the right people by a due date, and approve them before they are published. | core | P1 Review steps; P2 Review panel; P3 Approval gates |
| CON-A26-F7 Issue sets with transmittals | Gather files and sheets into an issue set, send it with a transmittal, and keep a register of everything issued and received. | core | P1 Issue sets; P2 Transmittals for any file; P3 Received transmittals; P4 Document register |
| CON-A26-F8 Mark up files | Mark up drawings, PDFs and images, with each markup kept on the file version it was made on. | core | P1 Markups |
| CON-A26-F9 Track issues on files | Raise an issue on a file, a markup or a package, assign it to a firm or a person, and track it to closed. | next | P1 Issues on files |
| CON-A26-F10 Find any file | Search file names, attributes and the text inside files, and see only what I'm allowed to see. | core | P1 File search; P2 Search panel |
| CON-A26-F11 Work on files from my desktop and offline | Keep chosen project folders on my computer, work on them offline, and have changes come back as new versions. | core | P1 Desktop folder sync; P2 Offline files |
| CON-A26-F12 Keep records and archive at project close | Keep project records as long as the law and the contract ask, and archive the whole project read-only when it closes. | core | P1 Retention rules; P2 Project close |
| CON-A26-F13 See who did what to every file | See a full record of who opened, added, changed, moved, shared, approved and deleted every file. | core | P1 File audit log |

**Parts in Project files and document control**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| CON-A26-F1-P1 | **Project file store**. Project folders for any file type (models, drawings, PDFs, specifications, photos, reports and letters) on the kit's file service, sharing and version history (KIT-A15-F7-P1, KIT-A15-F8-P1, KIT-A15-F3-P1), in the same project space as the models (A25-F1-P1). A candidate kit document store, since Spare Office needs the same. | service | large | KIT-A15-F7-P1, KIT-A15-F8-P1, KIT-A15-F3-P1, CON-A25-F1-P1 |  |
| CON-A26-F1-P2 | **Folder templates**. Folder structures per office, client and project type, set up with a new project and kept as data. | data | small | CON-A26-F1-P1 |  |
| CON-A26-F1-P3 | **File previews**. Thumbnails and quick previews of every file without opening its program, through the kit's preview service (KIT-A15-F16-P1). | integration | small | CON-A26-F1-P1, KIT-A15-F16-P1 |  |
| CON-A26-F2-P1 | **File versions**. Uploading a file over an existing one makes its next version, with who, when and a note. Versions are split into content-defined chunks, as restic does, so a new version stores and sends only what changed. Models are versioned by their operation log. | library | medium | CON-A26-F1-P1, KIT-A15-F3-P1 |  |
| CON-A26-F2-P2 | **File compare**. Compare two versions of a file: models element by element (A12-F4-P2), drawings and PDFs as an overlay of changed areas, images side by side, and text documents word by word. | library | medium | CON-A26-F2-P1, CON-A12-F4-P2, KIT-A27-F6-P1 |  |
| CON-A26-F2-P3 | **Restore a version**. Make an earlier version current again. The restore is saved as a new version, so nothing is lost. | library | small | CON-A26-F2-P1 |  |
| CON-A26-F3-P1 | **Naming rules**. Document naming rules per office and per client, as rule sets for the one naming rules engine (A9-F6-P3). | data | small | CON-A26-F1-P1, CON-A9-F6-P3 |  |
| CON-A26-F3-P2 | **Upload check**. Each upload is checked against the naming rules. A name that fails shows which field is wrong and a suggested name, and the project chooses whether a failing name blocks the upload or only warns. | library | small | CON-A26-F3-P1, CON-A26-F4-P1 |  |
| CON-A26-F4-P1 | **File attributes**. Typed attributes on every file and folder, such as originator, discipline, type, level, revision and description, filled in from the file name where the naming rules allow, and set per office and client. | data | small | CON-A26-F1-P1, CON-A26-F3-P1 |  |
| CON-A26-F4-P2 | **Information states**. Every file and model version has one state: work in progress (seen by its team), shared (seen by other teams for coordination), published (approved for use) or archived (kept read-only). These follow the states ISO 19650 describes for a common data environment, without claiming compliance. Publishing a package shares what it holds. | library | medium | CON-A26-F4-P1 |  |
| CON-A26-F4-P3 | **Status codes**. Status and revision codes kept as data per country, client or office, so a project uses the codes its contract asks for. No national code table is copied without clearance (A24-F1-P6). | data | small | CON-A26-F4-P2, CON-A24-F1-P6 |  |
| CON-A26-F5-P1 | **Folder and file permissions**. View, add, edit, approve and manage rights on folders and files, given to a firm, team, role or person and passed down to sub-folders unless changed, using the same relationship checks as sharing (A25-F6-P1). | service | medium | CON-A25-F6-P1, CON-A26-F1-P1 |  |
| CON-A26-F5-P2 | **Who has access**. One panel that shows who can open a folder or file and why, and changes it. | interface | small | CON-A26-F5-P1 |  |
| CON-A26-F6-P1 | **Review steps**. Review steps kept as data: who reviews, in what order, by when, and which decisions they can give (approved, approved with comments, rejected). Each decision is an operation with its reason, and due dates and reminders run on the kit's durable timers and notifications (KIT-A15-F14-P1, KIT-A8-F3-P2). A candidate kit part, since Spare Studio's review and approval needs the same. | library | medium | CON-A26-F4-P2, KIT-A15-F1-P2, KIT-A15-F14-P1, KIT-A8-F3-P2 |  |
| CON-A26-F6-P2 | **Review panel**. A panel beside the file or package with its markups (A26-F8-P1), the decision buttons and the history of every step. | interface | medium | CON-A26-F6-P1, CON-A26-F8-P1 |  |
| CON-A26-F6-P3 | **Approval gates**. Rules such as a file needing an approved review before it is published, or a package needing the lead's approval before it leaves, checked on every change of state. | library | small | CON-A26-F6-P1, CON-A26-F4-P2 |  |
| CON-A26-F7-P1 | **Issue sets**. Pick sheets, models and any project files into an issue set with a reason and recipients. Issuing fixes the versions it holds, marks them published, and keeps the issue records and fingerprints that drawing issues use (A11-F4-P1, A11-F4-P4). | library | medium | CON-A26-F2-P1, CON-A26-F4-P2, CON-A11-F4-P1, CON-A11-F4-P4 |  |
| CON-A26-F7-P2 | **Transmittals for any file**. The transmittal and the document register cover every file in a set, not only sheets, extending the drawing transmittals and register (A11-F4-P2), and are sent by the issue sender (A11-F4-P3). | library | small | CON-A26-F7-P1, CON-A11-F4-P2, CON-A11-F4-P3 |  |
| CON-A26-F7-P3 | **Received transmittals**. Log what other firms and the client send, file what came into the right folders, and record the acknowledgement. | library | small | CON-A26-F7-P2, CON-A26-F1-P1 |  |
| CON-A26-F7-P4 | **Document register**. One register of every document with its versions, state and every issue it went out in, filtered and exported to a spreadsheet (A10-F3-P1). | interface | small | CON-A26-F7-P2, CON-A10-F3-P1 |  |
| CON-A26-F8-P1 | **Markups**. Clouds, text, arrows and stamps on sheets, PDFs and images, using the kit's PDF markup (KIT-A27-F6-P4) and anchored comments (KIT-A8-F12-P2), which Spare Studio needs too. Markups are operations kept on the file version they were made on, and carried to the next version where they still apply. | library | medium | CON-A8-F2-P4, engine-pdf, KIT-A27-F6-P4, KIT-A8-F12-P2, CON-A26-F2-P1 |  |
| CON-A26-F9-P1 | **Issues on files**. Issues linked to a file version, a markup or a package, in the one issue model (A12-F8-P3), so model, sheet and file issues sit in one list. | library | small | CON-A12-F8-P3, CON-A26-F8-P1, CON-A25-F2-P1 |  |
| CON-A26-F10-P1 | **File search**. Names, attributes and contents (text in PDFs and documents, scanned drawings read by the kit's text recognition, and model properties) indexed in the kit's search service, with results filtered by permission (KIT-A6-F1-P2, KIT-A6-F1-P4, KIT-A27-F12-P3). | integration | medium | CON-A26-F1-P1, CON-A26-F4-P1, KIT-A6-F1-P2, KIT-A6-F1-P4, KIT-A27-F12-P3 |  |
| CON-A26-F10-P2 | **Search panel**. A search box with filters by folder, state, discipline, type and date, on the kit's search screens (KIT-A6-F1-P5). | interface | small | CON-A26-F10-P1, KIT-A6-F1-P5 |  |
| CON-A26-F11-P1 | **Desktop folder sync**. Chosen project folders kept on the computer through the kit's desktop sync client (KIT-A15-F15-P1). Each saved change becomes a new version, and the naming check runs as it syncs. | integration | medium | CON-A26-F2-P1, CON-A26-F3-P2, KIT-A15-F15-P1 |  |
| CON-A26-F11-P2 | **Offline files**. Files kept for offline use are encrypted on the device (KIT-A15-F4-P4). If two people change one file offline, both versions are kept and a person chooses (KIT-A15-F4-P3). | library | small | CON-A26-F11-P1, KIT-A15-F4-P3, KIT-A15-F4-P4 |  |
| CON-A26-F12-P1 | **Retention rules**. Retention per folder and kind of record, with legal holds, through the kit's retention schedules (KIT-A18-F6-P1). Issue records keep the longstop period of the country pack (A11-F4-P5). | library | small | CON-A26-F1-P1, KIT-A18-F6-P1, CON-A11-F4-P5 |  |
| CON-A26-F12-P2 | **Project close**. Closing a project moves everything to archived and read-only, and writes an archive copy in open formats (PDF/A, IFC and the project file) through the full export (A18-F7-P1, A11-F2-P5). | library | medium | CON-A26-F12-P1, CON-A26-F4-P2, CON-A18-F7-P1, CON-A11-F2-P5 |  |
| CON-A26-F13-P1 | **File audit log**. Every file action is an operation, and opens and downloads are logged too, searchable and exportable in the kit's organisation audit log (KIT-A1-F7-P7). | library | small | CON-A26-F1-P1, KIT-A1-F7-P7 |  |

## Moved and merged parts

IDs are never reused. A part that changed priority moved to a feature of that priority under a new ID; a duplicate was merged into the part that stays.

| Old ID | Now | Why |
| --- | --- | --- |
| CON-A1-F1-P2 | CON-A1-F9-P1 | The kit's update delivery is next, so updates are next too. |
| CON-A1-F2-P3 | CON-A1-F8-P1 | Needs the kit's organisation model (next) and outside sign-in (later). |
| CON-A19-F3-P4 | CON-A1-F7-P6 | Goes with the AI helper; the first release sends nothing to a co-op AI. |
| CON-A6-F3-P3 | CON-A6-F8-P3 | Drainage loads are needed at core for a house consent. |
| CON-A15-F1-P1 | CON-A15-F6-P1 | H1 for houses is core; full energy simulation stays next. |
| CON-A15-F1-P4 | CON-A15-F6-P2 | Goes with the energy simulation, which stays next. |
| CON-A17-F3-P2 | CON-A17-F7-P1 | Planning envelopes are needed at core; the council data lookup stays next. |
| CON-A21-F1-P1 | CON-A21-F6-P1 | Framing and bracing design is core; the 3D generator stays next. |
| CON-A21-F1-P5 | CON-A21-F6-P2 | Goes with the 3D framing generator, which stays next. |
| CON-A19-F3-P3 | merged into CON-A1-F7-P3 | Same feature as the AI helper (CON-A1-F7-P3), and both repeated the kit help assistant (KIT-A17-F7). |
| CON-A18-F7-P2 | merged into CON-A2-F4-P6 | Repeated the reader library already in the published file format (CON-A2-F4-P6). |
| CON-A12-F7-P1 | merged into CON-A26-F1-P1 | Project files are core for the first release; the document store is now the project file store. |
| CON-A12-F7-P2 | CON-A26-F3-P1 | Naming checks on upload are core for the first release. |
| CON-A11-F6-P1 | CON-A26-F8-P1 | Markups on any file are core for the first release; tracking them as issues stays next. |

## Questions for assessing this map and the demo

- Does the core set produce a full council set for a house (roadmap Gate D), and is anything in it that a house set does not need?
- Could someone open the demo and do a whole day's work they do in Revit today: model, document, schedule, sheet and issue, without hitting a gap?
- Which add-ins does our own office pay for today, and does each one appear here as a feature with a priority that matches how much we use it?
- Is clash, multi-user sync and version compare right as "next", or does a real office need one of them in the first release?
- Are the kernel, regeneration graph, hidden line and sketch solver parts sized realistically, and do their work-board pieces (A01, A03, A11, A12) cover the research they need first?
- Does every part that reads another company's format or data have a legal part in front of it (Revit exporter terms, direct .rvt reading, DWG, families, standards tables, classification tables)?
- Is the Revit exporter add-in the right way in for a first release, and is moving from Revit easy enough that an office would switch?
- Are New Zealand rules (D1 stairs, H1 energy, G13 drainage, NZS 3604, height in relation to boundary, LINZ data) covered first, and is the country pack design ready for Australia and others?
- Can a person with a disability use the panels and views, and can someone use the app fully offline with no account?
- Is every running cost (hosting, cloud rendering, AI use) paid at cost through the kit and shown on the open cost ledger, with what stays free clearly published?
- Is the demo's interface our own design, with our own names and icons, and nothing that copies another product's look?
- Which parts can start now in the Mapping stage as research and decision notes, and which must wait for the first six decisions?
- Are there features missing that a structural engineer, a services engineer, a builder or a council officer would expect to see?
- Do the parts lean on the shared kit and engines where they should, rather than building their own accounts, payments, messaging or PDF output?
- Has the kit made core the two parts Spare Construct's first release needs (its plugin host KIT-A16-F5-P2 and chunked storage KIT-A15-F6-P2), or should Spare Construct's first release wait for them?
- Does the house consent checklist (CON-A11-F5-P4) match what councils ask for, and are the items it marks as coming from outside acceptable at Gate D?
- Do the decisions that gate the Design stage but have no work-board piece yet (storage layout CON-A2-F4-P8, kernel start CON-A3-F1-P7) now have pieces, and does every other decision part have one before its build part starts?
