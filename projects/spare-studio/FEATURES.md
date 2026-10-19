# Spare Studio: what needs to be built

*Working name. Stage: Mapping. Mapped top-down on 2026-10-18, then checked on 2026-10-19 against an independent review and the kit's own map. Every choice is open until an owner accepts a decision note.*

Spare Studio replaces Adobe Creative Cloud: Photoshop, Illustrator, InDesign, InCopy, Premiere Pro, Media Encoder, After Effects, Lightroom, Lightroom Classic, Acrobat, Acrobat Sign, Audition, XD, Animate, Character Animator, Fresco, Bridge, Dreamweaver, Express, Firefly, Substance 3D, Dimension, Adobe Fonts, Adobe Stock, Frame.io and Behance. One creative suite for photos, vectors, layout, video, motion, raw photos, PDF, audio, screens, animation and 3D, built on one shared studio core and the Spareware kit. Free to use, open formats, AI optional and at cost, and owned by its members. This map lists everything a full replacement needs, top down: 21 areas, 202 features a person can use, and 562 buildable parts. It covers the apps themselves and every journey around them: first sign-up, daily use, edge cases, teams and admin, paying at cost, AI, sharing, safety, accessibility, offline work, many languages and countries, taking your data with you, and legal duties. Adobe's product names appear only to say what each area replaces. Where a part rests on the shared kit, it names the kit part it uses (93 kit parts in all, listed in the [kit's feature map](../kit/FEATURES.md)) and holds only Studio's own rules; parts that two programs need say which shared engine or kit part they belong in. Selling assets lives in Spare Market, slides in Spare Office and hiring in Spare Work, so Studio links to them instead of building its own. The map is [ARCHITECTURE.md](ARCHITECTURE.md), the same list in machine-readable form is [features.json](features.json), and the open pieces of work are on the [work board](STATUS.md).

**Priority:** core is needed for that app's first usable release, and the shared core areas are needed before any app ships (93 features); next comes soon after (71); later can wait (38). Which app ships first is piece [S08](STATUS.md). **Kinds of part:** library, service, interface, data, integration, research, legal. **Depends on** names kit crates (kit-...), kit services (svc-...), shared engines (engine-...), kit parts by ID (KIT-...) or other Studio parts by ID. **Piece** links a part to an existing work-board piece.

## Areas

| ID | Area | Features | Parts | What it covers |
| --- | --- | --- | --- | --- |
| STU-A1 | [Accounts, membership, teams and admin](#stu-a1-accounts-membership-teams-and-admin) | 9 | 37 | How a person or an organisation starts, signs in, joins the co-op, pays for the few things that cost money and manages people and machines. Replaces the Creative Cloud account, plans and Admin Console. Every app works without an account; an account adds sync, sharing, AI at cost and buying from Spare Market. |
| STU-A2 | [Studio core: workspace, documents and history](#stu-a2-studio-core-workspace-documents-and-history) | 13 | 43 | The shared base every app stands on: one workspace, one document model where every edit is a non-destructive operation, an open file format, smooth handling of huge files and history that never loses work. Built once, used by every Spare Studio app. |
| STU-A3 | [Type, colour and output](#stu-a3-type-colour-and-output) | 8 | 31 | The shared core for text in every app, colour that stays true from screen to print and video, and output to printers, files and the web, with proof of where a file came from. Replaces the type, colour and export features spread across every Adobe app. |
| STU-A4 | [Spare Library: assets, fonts, colours and templates](#stu-a4-spare-library-assets-fonts-colours-and-templates) | 8 | 22 | One browser for every file, shared libraries that work in every app, open fonts, open stock and templates, and a door into Spare Market for the assets members sell. Replaces Bridge, Creative Cloud Libraries, Adobe Fonts, Adobe Stock and Adobe Color. Selling, orders, tax and payouts for assets live in Spare Market on the shared marketplace engine. |
| STU-A5 | [Spare Photo and Spare Paint: photo editing, compositing and painting](#stu-a5-spare-photo-and-spare-paint-photo-editing-compositing-and-painting) | 15 | 48 | Edit and retouch photos, combine images, design graphics and paint by hand, all without harming the original. Replaces Photoshop and Fresco. Spare Paint is Spare Photo's brush engine in a touch-first layout. |
| STU-A6 | [Spare Draw: vector illustration](#stu-a6-spare-draw-vector-illustration) | 12 | 32 | Logos, illustrations, icons, packaging, patterns and type art as sharp vectors at any size. Replaces Illustrator. Shares its 2D drawing engine with Spare Construct. |
| STU-A7 | [Spare Layout: page layout, long documents and quick designs](#stu-a7-spare-layout-page-layout-long-documents-and-quick-designs) | 11 | 34 | Magazines, books, reports, catalogues, flyers, social posts and e-books, from a one-minute template edit to a 500-page book. Replaces InDesign, InCopy and the template side of Express. |
| STU-A8 | [Spare Raw: photo library and raw development](#stu-a8-spare-raw-photo-library-and-raw-development) | 15 | 37 | Bring photos in, organise tens of thousands of them, develop raw files without touching the originals, and share, print or make books from them. Replaces Lightroom and Lightroom Classic. |
| STU-A9 | [Spare Video: video editing and encoding](#stu-a9-spare-video-video-editing-and-encoding) | 14 | 36 | Edit video from the first rough cut to a finished film, with captions, colour, sound and export to every screen. Replaces Premiere Pro, Media Encoder and the phone video editors. Shares its media engine with Spare Social. |
| STU-A10 | [Spare Motion: motion graphics and visual effects](#stu-a10-spare-motion-motion-graphics-and-visual-effects) | 11 | 22 | Animated titles and graphics, compositing, keying, tracking and visual effects, plus templates that Spare Video uses. Replaces After Effects. |
| STU-A11 | [Spare Sound: audio editing, mixing and podcasts](#stu-a11-spare-sound-audio-editing-mixing-and-podcasts) | 9 | 20 | Record, edit, repair and mix audio for podcasts, video, radio and music. Replaces Audition. Shares its audio engine with Spare Video and Spare Motion. |
| STU-A12 | [Spare PDF: read, edit, review, fill and sign](#stu-a12-spare-pdf-read-edit-review-fill-and-sign) | 18 | 41 | Read any PDF, mark it up, edit it, organise pages, fill and build forms, sign and send for signature, scan paper, protect, redact and make PDFs accessible. Replaces Acrobat, Acrobat Reader, Acrobat Sign and the scan and fill-and-sign phone apps. Shares its PDF engine with Spare Construct and Spare Office. |
| STU-A13 | [Spare Screens: interface design, prototypes and websites](#stu-a13-spare-screens-interface-design-prototypes-and-websites) | 8 | 17 | Design app and web screens, build clickable prototypes, keep design systems, hand off to developers and publish simple websites. Replaces XD and Dreamweaver, and the screen-design tools people moved to after XD. Penpot (MPL-2.0) is the first project to learn from or partner with. |
| STU-A14 | [Spare Animate: 2D and character animation](#stu-a14-spare-animate-2d-and-character-animation) | 5 | 13 | Frame-by-frame and rigged 2D animation for cartoons, explainers, games and the web, and puppets that follow a webcam. Replaces Animate and Character Animator. OpenToonz (BSD-3-Clause) is the first project to learn from or build on. |
| STU-A15 | [Spare 3D: materials, texturing and 3D scenes](#stu-a15-spare-3d-materials-texturing-and-3d-scenes) | 6 | 15 | Paint textures onto 3D models, build materials, stage product shots and render realistic images. Replaces Substance 3D and Dimension. Shares its renderer with Spare Construct; Blender's Cycles renderer (Apache-2.0) is the first to learn from. |
| STU-A16 | [AI tools: optional, at cost, with consent](#stu-a16-ai-tools-optional-at-cost-with-consent) | 10 | 28 | AI across the suite that a member can switch off. Members choose the model, AI runs on the device or at cost, every AI edit is labelled, and nothing is trained on members' work without their consent. Replaces Firefly and the generative features in every Adobe app. |
| STU-A17 | [Sharing, review and teamwork](#stu-a17-sharing-review-and-teamwork) | 7 | 20 | Keep files in co-op storage at cost, share links, collect feedback pinned to the exact spot, page, frame or text, approve versions, work together live and show a portfolio. Replaces Creative Cloud storage and cloud documents, Frame.io review, Behance and Adobe Portfolio; hiring from a portfolio happens in Spare Work. |
| STU-A18 | [Automation, plugins and AI agents](#stu-a18-automation-plugins-and-ai-agents) | 5 | 19 | Every change is an operation, so scripts, plugins and AI agents can do anything a person can, through one command API, safely and in the open. Replaces each Adobe app's own scripting, plugin systems and automation. |
| STU-A19 | [Accessibility, languages, devices and offline](#stu-a19-accessibility-languages-devices-and-offline) | 7 | 25 | Every app works for everyone, in their own language, on the computer or tablet they already have, with or without the internet. New Zealand first, international by design. |
| STU-A20 | [Trust, safety, privacy and legal duties](#stu-a20-trust-safety-privacy-and-legal-duties) | 8 | 17 | Studio's own duties on top of the kit's: face data, client work in cloud files, member websites, active content in files, the LGPL question and proof that the suite is built legally. Privacy, reports, moderation, notices, consumer law, terms and security reviews come from the kit, and Studio only adds its own rules. Not legal advice; every legal part needs checking with a lawyer. |
| STU-A21 | [Plan, partners and names](#stu-a21-plan-partners-and-names) | 3 | 5 | The work that decides what is built first and with whom: the ArtCraft partnership, the order of apps and the names. Moved here from the core and legal areas, since none of it is a feature a person uses. |

## STU-A1 Accounts, membership, teams and admin

How a person or an organisation starts, signs in, joins the co-op, pays for the few things that cost money and manages people and machines. Replaces the Creative Cloud account, plans and Admin Console. Every app works without an account; an account adds sync, sharing, AI at cost and buying from Spare Market.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A1-F1 Install the suite and start without an account | Download one installer, pick the apps they want and start working offline with no sign-in. | core | P1 Installers on the shared desktop shell; P2 Browser build study; P3 Studio update channels; P4 Local profile without an account |
| STU-A1-F2 Make a Spareware account and sign in | Create one Spareware login with a passkey or password, use it in every app and on every device, and get back in if they lose access. | core | P1 Studio sign-in; P2 The studio as a sign-in client |
| STU-A1-F3 Find their way on the first day | See a short welcome, say what they make, and get starter templates and a guided first edit. | core | P1 Suite home screen; P2 Studio first-run tours; P3 Guides for people moving from other tools; P4 Learning centre; P5 Sample files the co-op owns |
| STU-A1-F4 Join the co-op and have a say | Become a member, read Spare Studio's proposals and vote on the matters Studio's members decide. | next | P1 Join from the studio; P2 Studio proposals and votes; P3 Spare Studio's electorate |
| STU-A1-F5 Pay only for what costs money | Set a monthly cap for cloud storage, co-op-hosted AI and video processing, pay after use, and see what each dollar paid for. | next | P1 Per-member usage meter; P2 Monthly cap and pay after use; P3 Studio tax invoices; P4 Studio lines on the open ledger; P5 Free allowance and hardship rule; P6 Tax on services sold overseas; P7 Failed jobs and charge disputes |
| STU-A1-F6 Manage their account and data | Change details, see signed-in devices, download all their data and delete the account. | core | P1 Studio settings and privacy choices; P2 Studio files in the data export; P3 Delete the account and its files |
| STU-A1-F7 Set up a team, school or business | Create an organisation, invite people, give them roles, share libraries and storage, and keep the work when people leave. | next | P1 Studio roles for organisations; P3 Single sign-on for organisations; P4 Studio usage on organisation bills; P5 Schools and classes; P6 Hand-over when someone leaves; P7 Domain claiming and account provisioning |
| STU-A1-F8 Run the suite on many computers | Install, set up and update the apps on many machines and set policies such as AI off. | later | P1 Silent install and managed updates; P2 Organisation policies; P3 Admin audit log; P4 AI, agent and plugin policies |
| STU-A1-F9 Use the apps in a browser | Open and edit files in a web browser with nothing to install, without losing work kept there. | next | P1 Browser version; P2 Browser storage safety; P3 Browser accessibility mirror |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A1-F1-P1 | **Installers on the shared desktop shell.** Studio's app picker on signed installers for Windows, macOS and Linux that add or remove single apps; the installer belongs in the kit's shared desktop shell, used by Spare Construct and Spare Office too. | integration | medium | KIT-A20-F1-P2, KIT-A22-F2-P3 |  |
| STU-A1-F1-P2 | **Browser build study.** Whether the heavy apps can run in a browser on WebAssembly and WebGPU: memory limits, speed, file access, and which apps are worth it. | research | medium |  |  |
| STU-A1-F1-P3 | **Studio update channels.** Studio's update channels with pause, skip and roll back, on the kit's update delivery. | service | medium | KIT-A20-F7-P1, STU-A1-F1-P1 |  |
| STU-A1-F1-P4 | **Local profile without an account.** Preferences, recent files and libraries kept on the device until the person chooses to sign in. | data | small | kit-store |  |
| STU-A1-F2-P1 | **Studio sign-in.** The kit's passkey-first sign-in, recovery and new sign-in alerts inside every Studio app, with device sign-in codes for shared and lab machines. | interface | small | KIT-A1-F2-P4, KIT-A1-F4-P1, KIT-A1-F3-P3 |  |
| STU-A1-F2-P2 | **The studio as a sign-in client.** The apps sign in through the accounts service's OpenID Connect flow and keep tokens in the system keychain. | integration | small | svc-accounts, KIT-A1-F6-P1 |  |
| STU-A1-F3-P1 | **Suite home screen.** One start screen with recent files, new-document presets, templates and the installed apps. | interface | medium | STU-A1-F1-P4 |  |
| STU-A1-F3-P2 | **Studio first-run tours.** A skippable tour per app on a real sample file, on the kit's first-run guide. | interface | medium | KIT-A20-F6-P1, STU-A1-F3-P1, STU-A1-F3-P5 |  |
| STU-A1-F3-P3 | **Guides for people moving from other tools.** Plain notes and optional shortcut sets that map common habits onto our tools, written in our own words. | data | medium | STU-A2-F1-P4 |  |
| STU-A1-F3-P4 | **Learning centre.** Searchable how-to articles and short videos inside each app, also available offline. | interface | medium | STU-A1-F3-P1 |  |
| STU-A1-F3-P5 | **Sample files the co-op owns.** Photos, footage, sound and layouts made or licensed by the co-op for tutorials and templates. | data | medium |  |  |
| STU-A1-F4-P1 | **Join from the studio.** A link from every Studio app into the kit's join flow, which ties the account to a co-op membership and its share. | interface | small | KIT-A14-F1-P3 |  |
| STU-A1-F4-P2 | **Studio proposals and votes.** Studio's proposal types, such as the free allowance, the AI policy, at-cost prices and the order of apps, on the kit's proposals and ballots, with results applied as operations. | interface | small | KIT-A12-F1-P1, KIT-A12-F3-P1, KIT-A12-F4-P1, STU-A1-F4-P3 |  |
| STU-A1-F4-P3 | **Spare Studio's electorate.** States who votes on Studio matters; lean: every co-op member who uses Spare Studio votes on the free allowance, the AI policy, at-cost prices and the order of apps, since Studio has no workers in the marketplace sense, and asset sellers vote on the market fee in Spare Market. | data | small | KIT-A12-F5-P2, KIT-A12-F5-P1 |  |
| STU-A1-F5-P1 | **Per-member usage meter.** Counts storage, AI compute, video processing and server jobs per member against their cap; it belongs in the kit beside the per-app usage metering, since Spare Office, Spare Social and Spare Construct need the same. | service | medium | KIT-A13-F2-P2, svc-ledger, kit-ops |  |
| STU-A1-F5-P2 | **Monthly cap and pay after use.** Lets a person set a monthly cap and pay for use after each month through the payment provider, in their own currency; the co-op never holds a prepaid balance. | interface | medium | svc-payments, STU-A1-F5-P1, KIT-A3-F12-P1, KIT-A3-F8-P1 | [K02](../kit/STATUS.md) |
| STU-A1-F5-P3 | **Studio tax invoices.** Monthly tax invoices and receipts for at-cost Studio charges, issued through the kit's tax invoices. | service | small | KIT-A4-F2-P1, STU-A1-F5-P6 |  |
| STU-A1-F5-P4 | **Studio lines on the open ledger.** Studio's costs for storage, AI compute and video processing, and how each at-cost price was set, on the kit's ledger pages. | interface | small | KIT-A13-F1-P3, KIT-A13-F3-P1 | [K06](../kit/STATUS.md) |
| STU-A1-F5-P5 | **Free allowance and hardship rule.** A free storage and AI allowance per member, with a published rule for people who cannot pay. | data | small | svc-voting, STU-A1-F4-P3 |  |
| STU-A1-F5-P6 | **Tax on services sold overseas.** Which GST, VAT and digital services tax the co-op owes on at-cost storage and AI sold to people overseas, when it must register in each country, and how remote services rules treat marketplaces; the rules belong in the kit's tax rules per country. | legal | medium | KIT-A4-F7-P1, KIT-A4-F9-P1 |  |
| STU-A1-F5-P7 | **Failed jobs and charge disputes.** Charges nothing, or refunds automatically, for paid jobs that fail, are cancelled or are blocked by safety checks, and lets a member dispute any usage charge through the kit's support cases. | service | small | STU-A1-F5-P1, KIT-A21-F2-P1, KIT-A3-F5-P1 |  |
| STU-A1-F6-P1 | **Studio settings and privacy choices.** Studio's own choices, such as AI tools, training consent and cloud sync, on the kit's settings and connected apps pages. | interface | medium | KIT-A20-F5-P1, KIT-A1-F6-P2 |  |
| STU-A1-F6-P2 | **Studio files in the data export.** Adds every cloud file, library, comment and version history, in open formats, to the kit's data export. | service | medium | KIT-A18-F1-P1, STU-A2-F5-P2 |  |
| STU-A1-F6-P3 | **Delete the account and its files.** Deletes the account and its cloud files through the kit's account deletion, keeping only the records the law requires, such as tax invoices, signed documents and market sales, and keeping buyers' licences to assets they bought. | service | small | KIT-A18-F2-P1, KIT-A18-F6-P1 |  |
| STU-A1-F7-P1 | **Studio roles for organisations.** Studio's permissions, such as editing shared libraries, publishing and paid AI, as roles on the kit's organisation model. | service | small | KIT-A1-F7-P1 |  |
| STU-A1-F7-P3 | **Single sign-on for organisations.** Sign-in through an organisation's own identity system using OpenID Connect or SAML; it belongs in the kit beside the organisation model, since Spare Office needs it too. | integration | medium | svc-accounts, KIT-A1-F7-P1 |  |
| STU-A1-F7-P4 | **Studio usage on organisation bills.** Splits an organisation's at-cost Studio usage by team on the kit's business accounts, with purchase orders and invoices. | service | medium | KIT-A3-F17-P1, STU-A1-F5-P1 |  |
| STU-A1-F7-P5 | **Schools and classes.** Class spaces where teachers hand out and collect work, with student accounts the school manages. | interface | large | STU-A1-F7-P1, KIT-A1-F7-P2 |  |
| STU-A1-F7-P6 | **Hand-over when someone leaves.** When a person leaves an organisation, the organisation's cloud files, libraries and share links they owned pass to the organisation, and their personal files stay with them. | service | medium | STU-A1-F7-P1, KIT-A1-F7-P2, STU-A17-F1-P1 |  |
| STU-A1-F7-P7 | **Domain claiming and account provisioning.** Lets an organisation claim its email domain and create, change and remove accounts automatically through the SCIM standard; it belongs in the kit with directory sync. | integration | medium | KIT-A1-F7-P3, STU-A1-F7-P3 |  |
| STU-A1-F8-P1 | **Silent install and managed updates.** Installer options and update channels that IT staff push with standard device-management tools. | integration | medium | STU-A1-F1-P1 | [OS06](../spare-os/STATUS.md) |
| STU-A1-F8-P2 | **Organisation policies.** Settings an admin can lock, such as AI tools off, no sharing outside the organisation, or a storage region. | service | medium | STU-A1-F7-P1 |  |
| STU-A1-F8-P3 | **Admin audit log.** Every admin change recorded as an operation the organisation can read; it belongs in the kit beside the organisation model, since Spare Office needs it too. | service | small | kit-ops, KIT-A1-F7-P1 |  |
| STU-A1-F8-P4 | **AI, agent and plugin policies.** Lets an admin allow only listed plugins, allow or block outside AI agents and MCP access, and choose which AI providers people may use. | service | medium | STU-A1-F8-P2, STU-A18-F3-P2, STU-A18-F4-P4, STU-A16-F2-P1 |  |
| STU-A1-F9-P1 | **Browser version.** The apps the study finds workable, compiled to WebAssembly and served as an installable web app. | interface | large | kit-core, engine-render, STU-A1-F1-P2 |  |
| STU-A1-F9-P2 | **Browser storage safety.** Handles storage quotas and eviction, file access that only some browsers allow, phone memory limits and an offline service worker, and reminds people to export work kept only in the browser. | library | medium | STU-A1-F9-P1, kit-store |  |
| STU-A1-F9-P3 | **Browser accessibility mirror.** A hidden page structure that mirrors canvas content for screen readers in the browser build, in the kit's shared interface components with Spare Office's canvas mirror. | library | medium | STU-A1-F9-P1, STU-A19-F1-P4, KIT-A20-F1-P1 |  |

## STU-A2 Studio core: workspace, documents and history

The shared base every app stands on: one workspace, one document model where every edit is a non-destructive operation, an open file format, smooth handling of huge files and history that never loses work. Built once, used by every Spare Studio app.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A2-F1 Work in one familiar workspace | Arrange panels, tools and saved workspaces, and move between apps without learning a new layout. | core | P1 App shell; P2 Panels and saved workspaces; P3 Tool system; P4 Shortcuts and command search; P5 Preferences; P6 Canvas view |
| STU-A2-F2 Start a document the right size | Pick a preset for print, screen, social, video or phone, or set their own size, units and resolution. | core | P1 Document presets; P2 New document screen |
| STU-A2-F3 Build with layers, groups, masks and artboards | Stack content in layers and groups, mask it, and keep several artboards or pages in one file. | core | P2 Studio document model; P3 Non-destructive edit graph; P4 Linked and embedded content; P5 Layers panel |
| STU-A2-F4 Undo anything and step through history | Undo and redo every change, jump back through the history list and branch from an older state. | core | P1 Undo and redo on the operation log; P2 History panel and snapshots |
| STU-A2-F5 Save in an open format and never lose work | Save to a documented open file any tool can read, with autosave and recovery after a crash or power cut. | core | P1 Project file container; P2 Spare Studio file format; P3 Format reader and writer library; P4 Public format specification; P5 Autosave and crash recovery |
| STU-A2-F6 Open and save common image files | Open and export PNG, JPEG, WebP, AVIF, JPEG XL, TIFF, GIF, HEIF, BMP, OpenRaster and SVG files, including HDR photos from phones. | core | P1 Image codecs; P2 HEIF and HEVC patent check; P3 SVG import and export; P4 Metadata kept through every edit; P5 HDR photos with gain maps |
| STU-A2-F7 Open files from the tools we replace | Open layered PSD and PSB files and illustration files through the PDF inside them. | core | P1 What we may legally read and write; P2 PSD and PSB reader and writer; P3 Illustration files through their PDF; P4 EPS and PostScript study; P5 Import report; P6 PSD fidelity study |
| STU-A2-F8 Edit huge files smoothly | Work on very large images, long documents and long timelines without waiting. | core | P1 Renderer approach; P2 GPU image pipeline; P3 Tile and page cache; P4 Speed benchmarks |
| STU-A2-F9 Keep and compare versions | Name versions, see what changed between them and restore any of them. | next | P1 Named versions; P2 Visual compare |
| STU-A2-F10 Get help in context | Hover any tool for a short tip, read the manual offline, ask other members and report a problem from inside the app. | core | P1 Instant tips; P2 Offline manual; P3 Studio in the member forum; P4 Studio crash and bug reports |
| STU-A2-F11 Run long jobs in the background | Export, render and batch-process in the background while they keep working, and see progress, pause or cancel. | core | P1 Shared job queue |
| STU-A2-F12 Record and repeat steps | Record a set of steps once and replay it on other files in any app. | next | P1 Action recorder |
| STU-A2-F13 Open EPS and PostScript files | Open old EPS and PostScript artwork and edit it. | later | P1 EPS and PostScript import |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A2-F1-P1 | **App shell.** The frame every app shares: header, tool bar, panels, canvas and status line, following the Spareware design guide. | interface | large | kit-core |  |
| STU-A2-F1-P2 | **Panels and saved workspaces.** Docked, floating and tabbed panels, with saved layouts for each kind of task. | interface | medium | STU-A2-F1-P1 |  |
| STU-A2-F1-P3 | **Tool system.** One way to define tools, their options and their on-canvas handles, shared by every app. | library | medium | kit-ops |  |
| STU-A2-F1-P4 | **Shortcuts and command search.** Every command searchable by name, with shortcut sets a person can edit, export and share. | interface | medium | kit-ops, STU-A2-F1-P1 |  |
| STU-A2-F1-P5 | **Preferences.** Settings per app and for the whole suite, stored as data the person can export. | data | small | STU-A1-F1-P4 |  |
| STU-A2-F1-P6 | **Canvas view.** Zoom, pan, rotate, rulers, guides, grids and several views of one document. | library | medium | engine-render |  |
| STU-A2-F2-P1 | **Document presets.** Sizes for paper (ISO and US), screens, social formats and video, with A4 and metric units first in New Zealand. | data | small |  |  |
| STU-A2-F2-P2 | **New document screen.** Choose a preset or a custom size, colour mode, bit depth and background. | interface | small | STU-A2-F2-P1, STU-A1-F3-P1 |  |
| STU-A2-F3-P2 | **Studio document model.** One model for layers, groups, masks, artboards, pages and timelines that every app uses. | library | large | kit-core, kit-ops | [S02](STATUS.md) |
| STU-A2-F3-P3 | **Non-destructive edit graph.** Adjustments, filters and effects stored as live operations that can be changed or removed at any time. | library | large | STU-A2-F3-P2 | [S02](STATUS.md) |
| STU-A2-F3-P4 | **Linked and embedded content.** Place other files as live links or embedded copies, update or relink them when they move, and replace them. | library | medium | STU-A2-F3-P2, kit-store |  |
| STU-A2-F3-P5 | **Layers panel.** Shows, orders, groups, locks, hides, colour-labels and searches layers in every app. | interface | medium | STU-A2-F3-P2 |  |
| STU-A2-F4-P1 | **Undo and redo on the operation log.** Undo built on the kit's operation log, so people, scripts and AI share one history. | library | medium | kit-ops, STU-A2-F3-P2 | [S02](STATUS.md) |
| STU-A2-F4-P2 | **History panel and snapshots.** A list of every step, named snapshots, and branching from any earlier step. | interface | medium | STU-A2-F4-P1 |  |
| STU-A2-F5-P1 | **Project file container.** Whether studio files use a SQLite container or a custom chunk file, shared with Spare Construct's decision. | research | small |  | [A06](../spare-construct/STATUS.md) |
| STU-A2-F5-P2 | **Spare Studio file format.** A documented, layered, open file format with a published schema and test files. | data | large | kit-store, STU-A2-F3-P2, STU-A2-F5-P1 |  |
| STU-A2-F5-P3 | **Format reader and writer library.** A library that reads and writes the format, published so other tools can use it too. | library | medium | STU-A2-F5-P2 |  |
| STU-A2-F5-P4 | **Public format specification.** Plain documentation of the format with examples, version rules and a validator. | data | medium | STU-A2-F5-P2 |  |
| STU-A2-F5-P5 | **Autosave and crash recovery.** Saves work continuously in the background and restores it after a crash or power cut. | library | medium | kit-store, kit-ops |  |
| STU-A2-F6-P1 | **Image codecs.** Readers and writers for common raster formats using permissively licensed libraries. | library | medium | kit-core |  |
| STU-A2-F6-P2 | **HEIF and HEVC patent check.** Whether reading and writing HEIF photos needs a patent licence and whether platform codecs avoid it, as part of the one engine-media codec study. | legal | small |  | [S04](STATUS.md) |
| STU-A2-F6-P3 | **SVG import and export.** Reads and writes SVG with layers, text and gradients kept editable where possible. | library | medium | engine-draw2d |  |
| STU-A2-F6-P4 | **Metadata kept through every edit.** Reads and writes EXIF, IPTC and XMP metadata and keeps it through edits and exports. | library | medium | kit-core |  |
| STU-A2-F6-P5 | **HDR photos with gain maps.** Opens phone HDR photos with their gain maps (ISO 21496-1) and exports HDR JPEG, AVIF and JPEG XL with gain maps. | library | medium | STU-A2-F6-P1, STU-A3-F3-P3 |  |
| STU-A2-F7-P1 | **What we may legally read and write.** Each Adobe format checked against a published specification, with links. | research | small |  | [S06](STATUS.md) |
| STU-A2-F7-P2 | **PSD and PSB reader and writer.** Reads and writes layered PSD and PSB files from Adobe's published specification, keeping layers, masks and text. | library | large | STU-A2-F3-P2, STU-A2-F7-P1, STU-A2-F7-P6 |  |
| STU-A2-F7-P3 | **Illustration files through their PDF.** Opens illustration files by reading only the PDF they embed. | library | small | engine-pdf, STU-A2-F7-P1 |  |
| STU-A2-F7-P4 | **EPS and PostScript study.** Whether to write a PostScript interpreter from the published specification or read only the preview and PDF inside EPS files, since Ghostscript is AGPL. | research | medium | STU-A2-F7-P1 |  |
| STU-A2-F7-P5 | **Import report.** Lists anything that could not be brought across exactly and what it became instead. | interface | small | STU-A2-F7-P2 |  |
| STU-A2-F7-P6 | **PSD fidelity study.** How exactly layers, masks, effects and text layers can round-trip through PSD and PSB from the published specification, tested on real files. | research | medium | STU-A2-F7-P1 |  |
| STU-A2-F8-P1 | **Renderer approach.** Our own renderer on wgpu or a web renderer first, shared with Spare Construct's decision. | research | medium |  | [A04](../spare-construct/STATUS.md) |
| STU-A2-F8-P2 | **GPU image pipeline.** Tiles, filters, blending and adjustments as GPU compute on wgpu with a CPU fallback, built on engine-render as one image pipeline that the photo editors in Spare Social, Spare Market and Spare OS can use too. | library | large | engine-render, STU-A2-F8-P1 |  |
| STU-A2-F8-P3 | **Tile and page cache.** Loads only what is on screen at the zoom in use and pages the rest from disk. | library | medium | kit-store |  |
| STU-A2-F8-P4 | **Speed benchmarks.** A public set of very large test files with timings checked on every change. | data | medium |  |  |
| STU-A2-F9-P1 | **Named versions.** Versions kept on the operation log, locally and in sync. | library | medium | kit-sync, STU-A2-F4-P1 |  |
| STU-A2-F9-P2 | **Visual compare.** Side-by-side and overlay compare of two versions with the changed areas marked. | interface | medium | STU-A2-F9-P1, engine-render |  |
| STU-A2-F10-P1 | **Instant tips.** A short tip on every tool and setting that shows at once on hover. | interface | small | STU-A2-F1-P1 |  |
| STU-A2-F10-P2 | **Offline manual.** The full manual for every app, shipped with the app and searchable. | data | medium |  |  |
| STU-A2-F10-P3 | **Studio in the member forum.** A link from every app to a Spare Studio section of the kit's member forum. | interface | small | KIT-A21-F6-P1 |  |
| STU-A2-F10-P4 | **Studio crash and bug reports.** Sends crash and bug reports with consent through the kit's crash reports, with the person's file content stripped out. | service | small | KIT-A22-F9-P1 |  |
| STU-A2-F11-P1 | **Shared job queue.** One background queue for exports, renders and batch runs in every app, with progress, cancel, watch folders and limits on processor and graphics use. | library | medium | kit-ops, kit-store |  |
| STU-A2-F12-P1 | **Action recorder.** Records operations as a replayable script in the studio core, shared by every app, and runs it on many files through the job queue. | library | medium | kit-ops, STU-A18-F1-P1, STU-A2-F11-P1 |  |
| STU-A2-F13-P1 | **EPS and PostScript import.** Reads EPS and PostScript using the approach the study chose, without GPL or AGPL interpreters. | library | large | engine-pdf, STU-A2-F7-P4 |  |

## STU-A3 Type, colour and output

The shared core for text in every app, colour that stays true from screen to print and video, and output to printers, files and the web, with proof of where a file came from. Replaces the type, colour and export features spread across every Adobe app.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A3-F1 Set type in any app | Add and style text with any installed or open font, including OpenType features and variable fonts. | core | P1 Text engine libraries; P2 Text engine; P3 Text layers and frames; P4 Character and paragraph panels; P5 Missing font handling; P6 Colour and emoji fonts |
| STU-A3-F2 Write well in many languages | Type in any script with any input method, with spelling checks, hyphenation and the right line breaks for that language. | core | P1 Complex scripts; P2 Spelling and hyphenation dictionaries; P3 Glyphs, macrons and smart quotes; P4 Input methods for canvas text |
| STU-A3-F3 See true colour on screen | Work in sRGB, Display P3, wide-gamut RGB, CMYK, Lab and HDR, with colour-managed screens. | core | P1 Colour management approach; P2 Colour management engine; P3 Screen profiles and HDR display; P4 Colour modes and bit depths; P5 Screen calibration options |
| STU-A3-F4 Pick, mix and keep colours | Pick colours, build palettes and harmonies, sample from images and check contrast. | core | P1 Colour picker and mixer; P2 Palettes and harmonies; P3 Contrast checker; P4 Spot colour books |
| STU-A3-F5 Proof for print before printing | Soft-proof against a press or printer profile and see the colours that will not print. | next | P1 Soft proof and gamut warning; P2 Separations and ink preview |
| STU-A3-F6 Print from any app | Print to home, office and photo printers with colour management, bleed, marks and tiling. | core | P1 Print screen; P2 Printer colour handling |
| STU-A3-F7 Export for screen, print and social | Export a document, page, artboard, layer or selection in many formats and sizes at once. | core | P1 Export pipeline; P2 Export screen; P3 Export many at once; P4 Print-ready PDF |
| STU-A3-F8 Prove where a file came from | Attach content credentials that record how a file was made and edited, including any AI use, and choose what they reveal. | next | P1 Content credentials; P2 Credentials viewer; P3 Signing certificates and conformance; P4 Credential privacy choices |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A3-F1-P1 | **Text engine libraries.** Recommended open libraries for fonts, shaping and layout, with licences, and what Spare Layout needs on top. | research | medium |  | [S05](STATUS.md) |
| STU-A3-F1-P2 | **Text engine.** Font loading, shaping, bidirectional text, line breaking and paragraph layout, built in engine-text and shared with Spare Construct and Spare Office. | library | large | engine-text, STU-A3-F1-P1 |  |
| STU-A3-F1-P3 | **Text layers and frames.** Point text, area text, text on a path and threaded frames as one shared text object. | library | medium | STU-A3-F1-P2, STU-A2-F3-P2 |  |
| STU-A3-F1-P4 | **Character and paragraph panels.** Size, leading, tracking, kerning, OpenType features and variable font axes. | interface | medium | STU-A3-F1-P3 |  |
| STU-A3-F1-P5 | **Missing font handling.** Finds missing fonts, offers open replacements and keeps the original font names in the file. | library | small | STU-A3-F1-P2 |  |
| STU-A3-F1-P6 | **Colour and emoji fonts.** Draws COLR v1, SVG-in-OpenType and sbix colour fonts, so emoji and colour type work in every app. | library | medium | engine-text, STU-A3-F1-P2 |  |
| STU-A3-F2-P1 | **Complex scripts.** Arabic, Hebrew, Indic, Thai and CJK shaping, vertical text, ruby and Japanese spacing rules. | library | large | STU-A3-F1-P2 |  |
| STU-A3-F2-P2 | **Spelling and hyphenation dictionaries.** Open dictionaries per language, te reo Māori and English (New Zealand) first, each licence checked, kept in engine-text so Spare Office, Spare OS and Spare Construct use the same ones. | data | medium | engine-text |  |
| STU-A3-F2-P3 | **Glyphs, macrons and smart quotes.** A glyph panel, easy macron input and smart quotes that follow each language's rules. | interface | small | STU-A3-F1-P4, STU-A3-F2-P4 |  |
| STU-A3-F2-P4 | **Input methods for canvas text.** IME composition and candidate windows, dead keys and compose keys for text drawn on the GPU canvas, on desktop and in the browser; built in engine-text with Spare Office and Spare Construct. | library | large | engine-text, STU-A3-F1-P2, STU-A19-F5-P1 |  |
| STU-A3-F3-P1 | **Colour management approach.** How open tools manage colour, their licences, and a recommended approach for print, photo and video. | research | medium |  | [S03](STATUS.md) |
| STU-A3-F3-P2 | **Colour management engine.** ICC profile transforms for photo and print, and OpenColorIO for video and film, on the GPU. | library | large | STU-A2-F8-P2, STU-A3-F3-P1 |  |
| STU-A3-F3-P3 | **Screen profiles and HDR display.** Reads each screen's profile and shows HDR content correctly where the screen supports it. | library | medium | STU-A3-F3-P2, engine-render |  |
| STU-A3-F3-P4 | **Colour modes and bit depths.** RGB, CMYK, greyscale, Lab and multichannel at 8, 16 and 32 bits per channel. | library | medium | STU-A3-F3-P2, STU-A2-F3-P2 |  |
| STU-A3-F3-P5 | **Screen calibration options.** How members can calibrate screens with common measuring devices without GPL or AGPL code. | research | small |  |  |
| STU-A3-F4-P1 | **Colour picker and mixer.** Wheel, sliders and number fields in every colour mode, with an eyedropper that samples anywhere on screen. | interface | medium | STU-A3-F3-P2 |  |
| STU-A3-F4-P2 | **Palettes and harmonies.** Builds colour harmonies and pulls palettes from images. | library | small | STU-A3-F3-P2 |  |
| STU-A3-F4-P3 | **Contrast checker.** Checks text and background colours against accessibility contrast rules. | library | small | STU-A3-F3-P2 |  |
| STU-A3-F4-P4 | **Spot colour books.** Lets people use spot colour books they hold a licence for, and checks which books the co-op may ship. | legal | small |  |  |
| STU-A3-F5-P1 | **Soft proof and gamut warning.** Simulates the output profile on screen and marks areas that fall outside it. | library | medium | STU-A3-F3-P2 |  |
| STU-A3-F5-P2 | **Separations and ink preview.** Shows each ink plate, total ink coverage and overprints. | interface | medium | STU-A3-F3-P2 |  |
| STU-A3-F6-P1 | **Print screen.** System printing with page setup, scaling, tiling, crop marks and bleed. | interface | medium | engine-pdf |  |
| STU-A3-F6-P2 | **Printer colour handling.** Sends colour-managed output or lets the printer manage colour, with the choice explained in plain words. | library | small | STU-A3-F3-P2 |  |
| STU-A3-F7-P1 | **Export pipeline.** One export engine for images, PDF, SVG and stills, with saved presets. | library | medium | STU-A2-F6-P1, engine-pdf |  |
| STU-A3-F7-P2 | **Export screen.** Choose formats, sizes, quality and file names, with a preview of quality and file size. | interface | medium | STU-A3-F7-P1 |  |
| STU-A3-F7-P3 | **Export many at once.** Exports every artboard, page or layer in several sizes in one go. | library | small | STU-A3-F7-P1 |  |
| STU-A3-F7-P4 | **Print-ready PDF.** PDF/X output with bleed, marks and embedded fonts for commercial printers. | library | medium | engine-pdf, STU-A3-F3-P2 |  |
| STU-A3-F8-P1 | **Content credentials.** Signs exported files with C2PA content credentials, using a signer that belongs in engine-media and is shared with Spare Social. | library | medium | engine-media, kit-ops, STU-A3-F7-P1 |  |
| STU-A3-F8-P2 | **Credentials viewer.** Shows the content credentials on any file opened in the studio, using the shared reader in engine-media. | interface | small | engine-media, STU-A3-F8-P1 |  |
| STU-A3-F8-P3 | **Signing certificates and conformance.** How the co-op gets, stores and rotates the certificates that sign content credentials, and joins the C2PA conformance programme and trust list, once for Spare Studio and Spare Social. | research | small |  |  |
| STU-A3-F8-P4 | **Credential privacy choices.** A choice on each export to leave the name, the edit history or the location out of content credentials. | interface | small | STU-A3-F8-P1 |  |

## STU-A4 Spare Library: assets, fonts, colours and templates

One browser for every file, shared libraries that work in every app, open fonts, open stock and templates, and a door into Spare Market for the assets members sell. Replaces Bridge, Creative Cloud Libraries, Adobe Fonts, Adobe Stock and Adobe Color. Selling, orders, tax and payouts for assets live in Spare Market on the shared marketplace engine.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A4-F1 Browse and find any file | Browse folders, preview every supported format, filter by metadata and search by content. | core | P1 Asset browser; P2 Thumbnail and preview builder; P3 Search and filters; P4 Batch rename and metadata editing |
| STU-A4-F2 Keep reusable libraries | Save colours, text styles, graphics, brushes and components to a library and use them in every app. | core | P1 Library data model; P2 Libraries panel; P3 Shared and synced libraries |
| STU-A4-F3 Keep work on brand | Lock an organisation's logos, colours, fonts and templates so everyone's work stays on brand. | next | P1 Brand kits |
| STU-A4-F4 Use and manage fonts | Find open-licence fonts, turn them on in every app and see what each font's licence allows. | core | P1 Open font catalogue; P2 Font manager; P3 Font licence rules |
| STU-A4-F5 Start from a template | Start any kind of project from a co-op-made template and make it their own. | core | P1 Template gallery; P2 Licence for templates and samples |
| STU-A4-F6 Find free images, footage and sound | Search open-licence stock from inside every app, with licence and credit added automatically. | next | P1 Open media search; P2 Stock source terms; P3 Licence and credit tracking |
| STU-A4-F7 Buy and install assets from Spare Market | Browse brushes, fonts, templates, photos, footage, sounds and plugins that members sell in Spare Market, and install what they buy straight into a library. | later | P1 Spare Market in the library; P2 Asset licences kept with files; P6 Sell from a library; P7 Bought assets work offline |
| STU-A4-F8 Make colour themes | Build, save and share colour themes and gradients from images, harmony rules or a phone camera. | next | P1 Colour theme maker; P2 Capture from a phone camera |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A4-F1-P1 | **Asset browser.** A fast browser with thumbnails and previews for images, raw photos, vectors, video, audio, PDF and 3D. | interface | large | STU-A4-F1-P2 |  |
| STU-A4-F1-P2 | **Thumbnail and preview builder.** Builds and caches previews for every supported format in the background. | library | medium | STU-A2-F6-P1, engine-media, engine-pdf |  |
| STU-A4-F1-P3 | **Search and filters.** Search by name, metadata, colour and keywords, and optionally by what is in the picture. | library | medium | kit-store, STU-A2-F6-P4 |  |
| STU-A4-F1-P4 | **Batch rename and metadata editing.** Renames many files by pattern and edits metadata in bulk. | library | small | STU-A2-F6-P4 |  |
| STU-A4-F2-P1 | **Library data model.** One open format for colours, styles, graphics, brushes, components and templates. | data | medium | STU-A2-F5-P2 |  |
| STU-A4-F2-P2 | **Libraries panel.** Drag items into and out of libraries in every app. | interface | medium | STU-A4-F2-P1 |  |
| STU-A4-F2-P3 | **Shared and synced libraries.** Libraries synced across devices and shared with a team, updating every place an item is used. | service | medium | kit-sync, STU-A4-F2-P1 |  |
| STU-A4-F3-P1 | **Brand kits.** Logos, colours, fonts and templates an organisation locks so everyone's work stays on brand. | interface | medium | STU-A4-F2-P1, STU-A1-F7-P1 |  |
| STU-A4-F4-P1 | **Open font catalogue.** A catalogue of fonts under open licences such as the SIL Open Font License, with previews and pairings, kept in engine-text so Spare Office, Spare OS and Spare Construct share one. | data | medium | engine-text |  |
| STU-A4-F4-P2 | **Font manager.** Turns fonts on and off across apps, groups them and shows their licence terms. | interface | medium | STU-A3-F1-P2 |  |
| STU-A4-F4-P3 | **Font licence rules.** What each font licence allows for embedding in PDF, packaging for print and use on the web. | legal | small |  |  |
| STU-A4-F5-P1 | **Template gallery.** Co-op-made templates for posters, social posts, documents, slides, video and more. | data | large | STU-A1-F3-P5 |  |
| STU-A4-F5-P2 | **Licence for templates and samples.** A stated licence that lets members use co-op templates and sample files in commercial work. | legal | small | STU-A4-F5-P1, STU-A1-F3-P5 |  |
| STU-A4-F6-P1 | **Open media search.** Searches open-licence collections whose terms allow it, recording each item's licence and credit. | integration | medium | STU-A4-F6-P2 |  |
| STU-A4-F6-P2 | **Stock source terms.** Each source's licence and API terms checked before it is connected. | legal | small |  |  |
| STU-A4-F6-P3 | **Licence and credit tracking.** Keeps each placed asset's licence and credit with the file and lists them on export. | library | small | STU-A2-F6-P4 |  |
| STU-A4-F7-P1 | **Spare Market in the library.** Browses Spare Market's digital goods and installs bought assets into a library through Market's public API; listings, orders, tax and payouts stay in Spare Market on kit-market. | integration | medium | kit-market, STU-A4-F2-P1 |  |
| STU-A4-F7-P2 | **Asset licences kept with files.** Keeps each bought asset's licence and the buyer's licence certificate with the file and lists them on export. | library | small | STU-A4-F7-P1, STU-A4-F6-P3 |  |
| STU-A4-F7-P6 | **Sell from a library.** Sends an asset from a Studio library to a Spare Market listing with watermarked previews, version updates and any model and property releases attached; trader details, ratings and payouts stay in Spare Market. | integration | medium | STU-A4-F7-P1, STU-A2-F6-P4 |  |
| STU-A4-F7-P7 | **Bought assets work offline.** Fonts and assets bought in Spare Market are plain files that work offline for good, with no licence check-ins. | library | small | STU-A4-F7-P1, kit-store |  |
| STU-A4-F8-P1 | **Colour theme maker.** A tool to build themes and gradients and save them to a library. | interface | small | STU-A3-F4-P2, STU-A4-F2-P1 |  |
| STU-A4-F8-P2 | **Capture from a phone camera.** Pulls colours, patterns and shapes from the phone camera into a library. | interface | medium | STU-A4-F2-P1 | [K04](../kit/STATUS.md) |

## STU-A5 Spare Photo and Spare Paint: photo editing, compositing and painting

Edit and retouch photos, combine images, design graphics and paint by hand, all without harming the original. Replaces Photoshop and Fresco. Spare Paint is Spare Photo's brush engine in a touch-first layout.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A5-F1 Select exactly what they want | Select by shape, lasso, colour, edges, subject, sky or what is in focus, then refine hair and soft edges. | core | P1 Shape, lasso and wand selections; P2 Colour range and focus area; P3 Subject, sky and object selection; P4 Refine edges; P5 Quick mask and channels |
| STU-A5-F2 Adjust colour and tone without harm | Use adjustment layers for levels, curves, exposure, hue, black and white, colour balance, gradient maps and more. | core | P1 Adjustment set; P2 Adjustment panels; P3 Colour lookup tables; P4 Raw-style adjustments on any layer |
| STU-A5-F3 Retouch and repair | Remove spots and objects, heal, clone and patch, with content-aware fill that needs no AI. | core | P1 Healing, clone and patch; P2 Image method patent register; P3 Content-aware fill; P4 Red eye, dust and scratches |
| STU-A5-F4 Combine images | Blend layers with blend modes and opacity, mask and clip them, and align and blend photos automatically. | core | P1 Blend modes and layer styles; P2 Layer, vector and clipping masks; P3 Auto-align and auto-blend; P4 Smart containers |
| STU-A5-F5 Use filters and effects | Apply blur, sharpen, noise, distort, stylise, liquify and lens correction as editable filters. | core | P1 Filter set; P2 Liquify and face-aware warping; P3 Lens and perspective correction; P4 Open lens profiles |
| STU-A5-F6 Transform and warp | Scale, rotate, skew, distort, warp, puppet-warp and resize content while keeping key parts unstretched. | core | P1 Free transform and warp; P2 Puppet and perspective warp; P3 Content-aware scale; P4 Crop, straighten and canvas size |
| STU-A5-F7 Paint and draw by hand | Paint with natural brushes that respond to pressure and tilt, mix wet colour and paint on a tablet. | core | P1 Brush engine; P2 Brush format and import; P3 Mixer and wet media; P4 Symmetry, rulers and perspective guides; P5 Spare Paint touch layout |
| STU-A5-F8 Add shapes and text to designs | Draw vector shapes and add styled, warped text inside a photo document. | core | P1 Shape and pen tools in Photo; P2 Text in Photo |
| STU-A5-F9 Make many images at once | Record steps as an action, run it on a folder and make variations from a spreadsheet. | next | P1 Photo actions panel; P2 Photo batch processing; P3 Variations from data |
| STU-A5-F10 Merge photos into panoramas and HDR | Merge photos into panoramas, HDR images and focus stacks. | next | P1 Panorama merge; P2 HDR merge and tone mapping |
| STU-A5-F11 Edit HDR, 32-bit and scientific images | Edit 16 and 32-bit images for HDR screens, and open scientific and medical image formats. | later | P1 32-bit float editing; P2 Scientific image formats |
| STU-A5-F12 Animate frames and use video layers | Make short frame animations and put video clips in layers. | later | P1 Frame animation timeline; P2 Video layers |
| STU-A5-F13 Make mock-ups | Place a design onto a photo of a product, screen or sign so it follows the surface. | later | P1 Mock-up placement |
| STU-A5-F14 Show options from one file | Save layer comps that record which layers show, where they sit and how they look, to present alternatives from one file. | next | P1 Layer comps |
| STU-A5-F15 Use newer photo tools | Replace a sky, blur by depth, clone in perspective, find a font from a picture and relight a subject to match a composite. | later | P1 Sky replacement; P2 Depth-based lens blur; P3 Perspective cloning; P4 Font finder; P5 Relight to match |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A5-F1-P1 | **Shape, lasso and wand selections.** Rectangle, ellipse, lasso, polygon, magnetic lasso and flood-fill selection tools. | library | medium | STU-A2-F1-P3, STU-A2-F8-P2 |  |
| STU-A5-F1-P2 | **Colour range and focus area.** Selects by colour, tone, skin tones or the parts of a photo that are in focus. | library | medium | STU-A2-F8-P2 |  |
| STU-A5-F1-P3 | **Subject, sky and object selection.** One-click selection of the subject, sky or a clicked object, using an open model that runs on the device. | library | large | STU-A16-F1-P2, STU-A16-F1-P7 |  |
| STU-A5-F1-P4 | **Refine edges.** A view to refine edges and hair, clean colour fringes and output the result as a mask. | interface | medium | STU-A2-F8-P2 |  |
| STU-A5-F1-P5 | **Quick mask and channels.** Paint a selection as a mask and save selections as channels. | library | small | STU-A2-F3-P2 |  |
| STU-A5-F2-P1 | **Adjustment set.** The full set of tone and colour adjustments as GPU operations that stay editable. | library | large | STU-A2-F8-P2, STU-A2-F3-P3 |  |
| STU-A5-F2-P2 | **Adjustment panels.** Panels with live histograms, curves and adjusting by dragging on the image. | interface | medium | STU-A5-F2-P1 |  |
| STU-A5-F2-P3 | **Colour lookup tables.** Loads and saves LUTs in open formats such as .cube. | library | small | STU-A5-F2-P1 |  |
| STU-A5-F2-P4 | **Raw-style adjustments on any layer.** Applies Spare Raw's development controls to any layer as an editable filter. | library | small | STU-A8-F3-P3 |  |
| STU-A5-F3-P1 | **Healing, clone and patch.** Spot healing, healing brush, clone and patch tools with sampling from any layer. | library | large | STU-A2-F8-P2, STU-A5-F7-P1, STU-A5-F3-P2 |  |
| STU-A5-F3-P2 | **Image method patent register.** Which published methods for fill, healing, patch matching, content-aware scale, auto-blend, image trace and denoise are free to use and which may be patented, kept as one register for every app. | research | medium |  |  |
| STU-A5-F3-P3 | **Content-aware fill.** Fills a selection from the rest of the image using a published patch-matching method. | library | large | STU-A2-F8-P2, STU-A5-F3-P2 |  |
| STU-A5-F3-P4 | **Red eye, dust and scratches.** Small automatic repairs for red eye, dust and scratches. | library | small | STU-A2-F8-P2 |  |
| STU-A5-F4-P1 | **Blend modes and layer styles.** Every common blend mode and editable layer styles: shadow, glow, stroke, bevel and overlay. | library | medium | STU-A2-F8-P2 |  |
| STU-A5-F4-P2 | **Layer, vector and clipping masks.** Pixel masks, vector masks and clipping groups with density and feather. | library | medium | STU-A2-F3-P2 |  |
| STU-A5-F4-P3 | **Auto-align and auto-blend.** Lines up layers by their content and blends exposures or focus stacks. | library | large | STU-A2-F8-P2, STU-A5-F3-P2 |  |
| STU-A5-F4-P4 | **Smart containers.** Linked or embedded containers that keep their source, so they can be scaled and filtered without loss. | library | medium | STU-A2-F3-P4, STU-A2-F3-P3 |  |
| STU-A5-F5-P1 | **Filter set.** Blur, sharpen, noise, distort, render and stylise filters on the GPU. | library | large | STU-A2-F8-P2 |  |
| STU-A5-F5-P2 | **Liquify and face-aware warping.** Push, pull, bloat and pucker, with optional face detection on the device. | library | medium | STU-A2-F8-P2, STU-A20-F1-P5 |  |
| STU-A5-F5-P3 | **Lens and perspective correction.** Fixes distortion, vignetting, colour fringes and perspective. | library | medium | STU-A2-F8-P2, STU-A5-F5-P4 |  |
| STU-A5-F5-P4 | **Open lens profiles.** Lens correction data under an open licence, and a way for members to add their own lenses. | data | medium |  |  |
| STU-A5-F6-P1 | **Free transform and warp.** Transforms with handles or numbers and mesh warps, resampled at a chosen quality. | library | medium | STU-A2-F8-P2 |  |
| STU-A5-F6-P2 | **Puppet and perspective warp.** Pin-based mesh warping and plane-based perspective warping. | library | medium | STU-A5-F6-P1 |  |
| STU-A5-F6-P3 | **Content-aware scale.** Resizes an image while protecting people and chosen areas from stretching. | library | medium | STU-A5-F6-P1, STU-A5-F3-P2 |  |
| STU-A5-F6-P4 | **Crop, straighten and canvas size.** Crop to ratios, straighten by the horizon and extend the canvas. | library | small | STU-A5-F6-P1 |  |
| STU-A5-F7-P1 | **Brush engine.** A fast GPU brush engine with dynamics, textures, pressure, tilt and smoothing, in the studio core and shared by every app. | library | large | STU-A2-F8-P2 |  |
| STU-A5-F7-P2 | **Brush format and import.** Our open brush format, plus import of brush formats that have a published specification. | data | medium | STU-A4-F2-P1, STU-A2-F7-P1 |  |
| STU-A5-F7-P3 | **Mixer and wet media.** Colour mixing, watercolour and oil paint simulation. | library | large | STU-A5-F7-P1 |  |
| STU-A5-F7-P4 | **Symmetry, rulers and perspective guides.** Painting aids for symmetry, straight lines and perspective. | interface | small | STU-A5-F7-P1 |  |
| STU-A5-F7-P5 | **Spare Paint touch layout.** A touch-first painting layout for pen displays and tablets. | interface | medium | STU-A5-F7-P1, STU-A19-F6-P1 |  |
| STU-A5-F8-P1 | **Shape and pen tools in Photo.** Uses the shared vector engine for shapes and paths in pixel documents. | library | small | STU-A6-F1-P1 |  |
| STU-A5-F8-P2 | **Text in Photo.** Uses the shared text layers, with warp and layer styles. | library | small | STU-A3-F1-P3 |  |
| STU-A5-F9-P1 | **Photo actions panel.** Spare Photo's panel for recording and playing actions on the shared action recorder. | interface | small | STU-A2-F12-P1 |  |
| STU-A5-F9-P2 | **Photo batch processing.** Runs an action on folders with naming and format rules, on the shared job queue. | library | small | STU-A2-F12-P1, STU-A2-F11-P1 |  |
| STU-A5-F9-P3 | **Variations from data.** Fills text and image layers from a spreadsheet using the shared data merge engine. | library | small | STU-A7-F7-P1 |  |
| STU-A5-F10-P1 | **Panorama merge.** Finds matching points, then warps and blends photos into one panorama. | library | large | STU-A2-F8-P2 |  |
| STU-A5-F10-P2 | **HDR merge and tone mapping.** Merges bracketed photos and tone-maps them for ordinary screens and print. | library | medium | STU-A2-F8-P2 |  |
| STU-A5-F11-P1 | **32-bit float editing.** A floating-point pipeline for every adjustment and filter. | library | medium | STU-A2-F8-P2, STU-A3-F3-P4 |  |
| STU-A5-F11-P2 | **Scientific image formats.** Reads DICOM, FITS and multi-page TIFF stacks. | library | medium | STU-A2-F6-P1 |  |
| STU-A5-F12-P1 | **Frame animation timeline.** A simple frame timeline on the shared timeline model, with GIF, WebP and APNG export. | interface | medium | STU-A9-F2-P1, STU-A2-F3-P2 |  |
| STU-A5-F12-P2 | **Video layers.** Video clips as layers, trimmed and filtered with the media engine. | library | medium | engine-media |  |
| STU-A5-F13-P1 | **Mock-up placement.** Wraps a design onto a chosen surface with perspective, folds and lighting. | library | medium | STU-A5-F6-P2, STU-A5-F4-P4 |  |
| STU-A5-F14-P1 | **Layer comps.** Saved states of layer visibility, position and style that can be switched, updated and exported one by one. | library | medium | STU-A2-F3-P2 |  |
| STU-A5-F15-P1 | **Sky replacement.** Swaps the sky using sky selection and matches the foreground's colour and light. | library | medium | STU-A5-F1-P3, STU-A5-F2-P1 |  |
| STU-A5-F15-P2 | **Depth-based lens blur.** Blurs by distance using a depth map from an on-device model, with a chosen focus point and lens shape. | library | medium | STU-A16-F1-P2, STU-A2-F8-P2 |  |
| STU-A5-F15-P3 | **Perspective cloning.** Clones and paints along flat surfaces drawn in perspective, such as walls and floors. | library | medium | STU-A5-F3-P1, STU-A5-F6-P2 |  |
| STU-A5-F15-P4 | **Font finder.** Suggests open fonts that match the type in a picture, using an on-device model. | library | medium | STU-A16-F1-P2, STU-A4-F4-P1 |  |
| STU-A5-F15-P5 | **Relight to match.** Relights a placed subject to match the scene's light and colour, with the member's chosen model. | integration | medium | STU-A16-F2-P1, STU-A5-F4-P1 |  |

## STU-A6 Spare Draw: vector illustration

Logos, illustrations, icons, packaging, patterns and type art as sharp vectors at any size. Replaces Illustrator. Shares its 2D drawing engine with Spare Construct.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A6-F1 Draw paths and shapes | Draw with pen, pencil, curvature and shape tools and edit every point precisely. | core | P1 Vector engine; P2 Pen, curvature and pencil tools; P3 Live shapes; P4 GPU vector drawing |
| STU-A6-F2 Combine and cut shapes | Unite, subtract, intersect and divide shapes, build shapes by dragging, and cut with knife and scissors. | core | P1 Path booleans; P2 Shape builder; P3 Paint regions; P4 Knife, scissors and eraser |
| STU-A6-F3 Style strokes and fills | Use gradients, patterns, variable-width strokes, brushes, several fills on one object and blends between shapes. | core | P1 Appearance stack; P2 Gradients and gradient mesh; P3 Variable-width and brush strokes; P4 Pattern maker; P5 Object blends |
| STU-A6-F4 Arrange, align and lay out | Align, distribute, group and lock objects, use artboards, guides, grids and smart snapping. | core | P1 Align, distribute and smart guides; P2 Artboards |
| STU-A6-F5 Trace and convert images | Turn photos and sketches into editable vectors. | next | P1 Tracing methods; P2 Image trace |
| STU-A6-F6 Use type as art | Set display type, outline text, put text on paths and in shapes, and warp it. | core | P1 Text on paths and outlines; P2 Envelope and warp distortion |
| STU-A6-F7 Reuse symbols and recolour | Use symbols and repeats, recolour whole artworks at once and keep colours that update everywhere. | next | P1 Symbols and repeats; P2 Recolour artwork; P3 Global swatches |
| STU-A6-F8 Add effects, perspective and 3D looks | Use shadows, blurs and distortions, draw on a perspective grid and extrude or revolve shapes into 3D. | next | P1 Vector effects; P2 Perspective grid; P3 3D extrude and revolve |
| STU-A6-F9 Design packaging and print artwork | Set up dielines, spot colours and overprint, and make print-ready files. | next | P1 Dielines and print marks; P2 Technical drawing aids |
| STU-A6-F10 Export icons, logos and web graphics | Export SVG, PNG sets, icon sizes and graphics ready for developers. | core | P1 Asset export |
| STU-A6-F11 Make charts | Make bar, line, pie and other charts from data that stay editable as vectors. | later | P1 Chart tool |
| STU-A6-F12 Bend, edit in bulk and draw from data | Bend vector art with pins, change every similar object at once and make many graphics from a spreadsheet. | later | P1 Puppet warp for vectors; P2 Edit similar objects; P3 Graphics from data |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A6-F1-P1 | **Vector engine.** Paths, curves, strokes and fills built on the shared 2D drawing engine. | library | large | engine-draw2d, STU-A2-F3-P2 |  |
| STU-A6-F1-P2 | **Pen, curvature and pencil tools.** Tools for drawing and editing points and handles, with snapping and smoothing. | library | medium | STU-A6-F1-P1, STU-A2-F1-P3 |  |
| STU-A6-F1-P3 | **Live shapes.** Rectangles, ellipses, polygons, stars and lines that keep editable corners and sides. | library | small | STU-A6-F1-P1 |  |
| STU-A6-F1-P4 | **GPU vector drawing.** Fast, accurate drawing of complex vector art on the GPU, with exact preview at any zoom. | library | large | engine-render, engine-draw2d |  |
| STU-A6-F2-P1 | **Path booleans.** Robust 2D boolean operations on curves, built in engine-draw2d and shared with Spare Construct. | library | large | engine-draw2d |  |
| STU-A6-F2-P2 | **Shape builder.** Merges or removes regions by dragging across overlapping shapes. | library | medium | STU-A6-F2-P1 |  |
| STU-A6-F2-P3 | **Paint regions.** Fills the regions formed by overlapping paths as if they were separate shapes. | library | medium | STU-A6-F2-P1 |  |
| STU-A6-F2-P4 | **Knife, scissors and eraser.** Tools to cut, split and erase parts of paths. | library | small | STU-A6-F2-P1 |  |
| STU-A6-F3-P1 | **Appearance stack.** Several fills, strokes and effects on one object, each editable and reorderable. | library | medium | STU-A6-F1-P1, STU-A2-F3-P3 |  |
| STU-A6-F3-P2 | **Gradients and gradient mesh.** Linear, radial and freeform gradients and gradient meshes. | library | large | STU-A6-F1-P4 |  |
| STU-A6-F3-P3 | **Variable-width and brush strokes.** Width profiles, dashes, arrowheads and art, scatter and pattern brushes. | library | medium | STU-A6-F1-P1 |  |
| STU-A6-F3-P4 | **Pattern maker.** Make and edit seamless repeating patterns. | library | small | STU-A6-F1-P1 |  |
| STU-A6-F3-P5 | **Object blends.** Blends shapes and colours in steps or smoothly along a path. | library | medium | STU-A6-F1-P1 |  |
| STU-A6-F4-P1 | **Align, distribute and smart guides.** Alignment, spacing and snapping to points, pixels and other objects. | library | medium | STU-A6-F1-P1 |  |
| STU-A6-F4-P2 | **Artboards.** Many artboards in one file, with export per artboard. | library | small | STU-A2-F3-P2 |  |
| STU-A6-F5-P1 | **Tracing methods.** Published tracing methods and permissive libraries, learned from without copying GPL code. | research | small |  |  |
| STU-A6-F5-P2 | **Image trace.** Converts pixel images to vector paths, with presets for line art, logos and photos. | library | large | STU-A6-F1-P1, STU-A6-F5-P1, STU-A5-F3-P2 |  |
| STU-A6-F6-P1 | **Text on paths and outlines.** Text along paths and inside shapes, and text turned into outlines. | library | medium | STU-A3-F1-P3, STU-A6-F1-P1 |  |
| STU-A6-F6-P2 | **Envelope and warp distortion.** Warps objects and text through meshes and presets. | library | medium | STU-A6-F1-P1 |  |
| STU-A6-F7-P1 | **Symbols and repeats.** Reusable symbols and radial, grid and mirror repeats. | library | medium | STU-A2-F3-P2 |  |
| STU-A6-F7-P2 | **Recolour artwork.** Remaps every colour in a selection using harmony rules. | library | medium | STU-A3-F4-P2 |  |
| STU-A6-F7-P3 | **Global swatches.** Colours that update everywhere they are used. | library | small | STU-A4-F2-P1 |  |
| STU-A6-F8-P1 | **Vector effects.** Live effects such as offset path, roughen, zig zag, shadows and blurs. | library | medium | STU-A6-F3-P1 |  |
| STU-A6-F8-P2 | **Perspective grid.** Draw and place art on a one, two or three-point perspective grid. | library | medium | STU-A6-F1-P1 |  |
| STU-A6-F8-P3 | **3D extrude and revolve.** Extrudes and revolves shapes with lighting and materials, staying editable. | library | large | engine-render |  |
| STU-A6-F9-P1 | **Dielines and print marks.** Dieline layers, spot colours, overprint settings and printer's marks. | library | medium | STU-A3-F5-P2 |  |
| STU-A6-F9-P2 | **Technical drawing aids.** Dimensions, scale and measuring tools for technical illustration. | library | small | engine-draw2d |  |
| STU-A6-F10-P1 | **Asset export.** Exports chosen objects as SVG, PNG or PDF at several sizes in one step. | library | small | STU-A3-F7-P1, STU-A2-F6-P3 |  |
| STU-A6-F11-P1 | **Chart tool.** Charts linked to data and styled as vectors. | library | medium | STU-A6-F1-P1 |  |
| STU-A6-F12-P1 | **Puppet warp for vectors.** Pin-based bending of vector art that keeps paths editable. | library | medium | STU-A6-F1-P1, STU-A14-F2-P2 |  |
| STU-A6-F12-P2 | **Edit similar objects.** Finds objects that share a shape or style and edits them all at once. | library | small | STU-A6-F1-P1 |  |
| STU-A6-F12-P3 | **Graphics from data.** Fills text, images and visibility in vector art from a spreadsheet using the shared data merge engine. | library | small | STU-A7-F7-P1, STU-A6-F1-P1 |  |

## STU-A7 Spare Layout: page layout, long documents and quick designs

Magazines, books, reports, catalogues, flyers, social posts and e-books, from a one-minute template edit to a 500-page book. Replaces InDesign, InCopy and the template side of Express.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A7-F1 Lay out pages | Make pages and spreads with margins, columns, grids, bleed and parent pages. | core | P1 Pages, spreads and parent pages; P2 Frames and text wrap; P3 Pages panel; P4 Image fitting |
| STU-A7-F2 Set long text well | Flow text through threaded frames with paragraph and character styles, good justification and hyphenation. | core | P1 Paragraph composer; P2 Paragraph, character and object styles; P3 Baseline grid and optical alignment; P4 Find and change; P5 Story editor |
| STU-A7-F3 Build long documents | Make books with contents pages, indexes, footnotes, cross-references, tables and numbered lists. | next | P1 Books from many files; P2 Contents, index and cross-references; P3 Footnotes and endnotes; P4 Tables; P5 Text variables and running heads; P6 Conditional text |
| STU-A7-F4 Write and edit as a team | Let writers edit the text while designers lay out the pages, with tracked changes and comments. | next | P1 Shared stories; P2 Tracked changes |
| STU-A7-F5 Make quick designs from templates | Pick a template for a social post, poster, flyer or card, change the words and pictures and share it in minutes. | core | P1 Quick design editor; P2 Resize to other formats; P3 Quick actions |
| STU-A7-F6 Post and schedule designs | Post finished designs to Spare Social and other services now or at a set time. | later | P1 Post and schedule |
| STU-A7-F7 Fill pages from data | Merge spreadsheet data into layouts for catalogues, name cards and certificates, with the same engine Photo and Draw use. | next | P1 Data merge engine |
| STU-A7-F8 Check and package for print | Find errors as they work, gather fonts and images, and export print-ready PDF. | core | P1 Live preflight; P2 Package for print; P3 Imposition |
| STU-A7-F9 Publish e-books and digital documents | Export EPUB, interactive PDF and web pages from the same layout. | next | P1 EPUB export; P2 Web publishing; P3 Interactive documents |
| STU-A7-F10 Open layouts and text from other tools | Turn PDFs into editable layouts, place word-processor files, and open and save layout exchange files if they are cleared. | next | P1 PDF to editable layout; P2 Layout exchange format check; P3 Layout exchange import; P4 Word-processor text import; P5 Layout exchange export |
| STU-A7-F11 Turn layouts into slides | Export layout pages as an open slide file to present in Spare Office. | later | P1 Slide export |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A7-F1-P1 | **Pages, spreads and parent pages.** A page and spread model with parent pages, sections and page numbering. | library | large | STU-A2-F3-P2 |  |
| STU-A7-F1-P2 | **Frames and text wrap.** Text and image frames with text wrap, anchored objects and layout grids. | library | large | STU-A3-F1-P3 |  |
| STU-A7-F1-P3 | **Pages panel.** Add, move and duplicate pages and apply parent pages. | interface | medium | STU-A7-F1-P1 |  |
| STU-A7-F1-P4 | **Image fitting.** Fits, fills and crops placed images in frames, with links shown in a links panel. | library | small | STU-A7-F1-P2, STU-A2-F3-P4 |  |
| STU-A7-F2-P1 | **Paragraph composer.** Line breaking across whole paragraphs, with hyphenation and justification settings. | library | large | STU-A3-F1-P2 |  |
| STU-A7-F2-P2 | **Paragraph, character and object styles.** Styles with inheritance, nested styles and styles applied by text patterns. | library | medium | STU-A3-F1-P3 |  |
| STU-A7-F2-P3 | **Baseline grid and optical alignment.** Aligns text to a baseline grid, with optical margin alignment. | library | medium | STU-A7-F2-P1 |  |
| STU-A7-F2-P4 | **Find and change.** Finds and changes text, styles and objects, including by patterns. | library | medium | STU-A3-F1-P3 |  |
| STU-A7-F2-P5 | **Story editor.** A plain text view for writing and editing long stories. | interface | small | STU-A3-F1-P3 |  |
| STU-A7-F3-P1 | **Books from many files.** Combines documents into a book with shared styles and continuous numbering. | library | medium | STU-A7-F1-P1 |  |
| STU-A7-F3-P2 | **Contents, index and cross-references.** Tables of contents, indexes and cross-references that update themselves. | library | medium | STU-A7-F2-P2 |  |
| STU-A7-F3-P3 | **Footnotes and endnotes.** Footnotes and endnotes that flow with the text. | library | medium | STU-A7-F2-P1 |  |
| STU-A7-F3-P4 | **Tables.** Tables with styles, header rows and import from spreadsheets. | library | large | STU-A7-F1-P2 |  |
| STU-A7-F3-P5 | **Text variables and running heads.** Variables such as chapter title, date and file name, and running heads. | library | small | STU-A3-F1-P3 |  |
| STU-A7-F3-P6 | **Conditional text.** Marks text and objects for versions such as regions or audiences, and shows or hides each version from one document. | library | medium | STU-A3-F1-P3, STU-A7-F2-P2 |  |
| STU-A7-F4-P1 | **Shared stories.** Writers check out stories and edit them while the layout stays locked. | service | medium | kit-sync | [OF03](../spare-office/STATUS.md) |
| STU-A7-F4-P2 | **Tracked changes.** Tracks, accepts and rejects text changes, with comments on the shared comment model. | library | medium | kit-ops, STU-A3-F1-P3, STU-A17-F3-P1 |  |
| STU-A7-F5-P1 | **Quick design editor.** A simple editor with templates, one-click styles and plain controls for people who are not designers. | interface | large | STU-A4-F5-P1, STU-A7-F1-P1 |  |
| STU-A7-F5-P2 | **Resize to other formats.** Resizes a design to other sizes and keeps the layout sensible. | library | medium | STU-A7-F1-P1 |  |
| STU-A7-F5-P3 | **Quick actions.** One-click jobs such as remove a background, resize, convert a file or trim a video. | interface | medium | STU-A16-F5-P2, STU-A9-F10-P1 |  |
| STU-A7-F6-P1 | **Post and schedule.** Posts finished designs to Spare Social and to other services through their public APIs, now or later. | integration | medium | STU-A18-F5-P1, STU-A17-F5-P2 |  |
| STU-A7-F7-P1 | **Data merge engine.** One merge engine in the studio core that makes a page, record or variation per row of CSV or spreadsheet data for Layout, Photo and Draw; a candidate to share with Spare Office's mail merge through an engine. | library | medium | STU-A2-F3-P2, kit-ops |  |
| STU-A7-F8-P1 | **Live preflight.** Checks for missing fonts, low-resolution images, overset text and colour problems while they work. | library | medium | STU-A7-F1-P1 |  |
| STU-A7-F8-P2 | **Package for print.** Gathers the file, linked images and the fonts their licences allow into one folder. | library | small | STU-A4-F4-P3 |  |
| STU-A7-F8-P3 | **Imposition.** Arranges pages on sheets for printing booklets and signatures. | library | medium | engine-pdf |  |
| STU-A7-F9-P1 | **EPUB export.** EPUB 3 export, reflowable and fixed-layout, with accessibility metadata. | library | large | STU-A7-F1-P1 |  |
| STU-A7-F9-P2 | **Web publishing.** Publishes a document as a web page that fits phones and computers. | library | medium | STU-A7-F1-P1 |  |
| STU-A7-F9-P3 | **Interactive documents.** Buttons, links, forms and media in exported PDF and web documents. | library | medium | engine-pdf |  |
| STU-A7-F10-P1 | **PDF to editable layout.** Turns PDF pages into Layout frames, text and images using the shared page recovery engine. | library | large | engine-pdf, STU-A12-F7-P3 |  |
| STU-A7-F10-P2 | **Layout exchange format check.** Whether the old public layout exchange specification can still be used. | legal | small |  | [S06](STATUS.md) |
| STU-A7-F10-P3 | **Layout exchange import.** Imports layout exchange files, only if an owner clears it. | library | large | STU-A7-F10-P2 |  |
| STU-A7-F10-P4 | **Word-processor text import.** Places text from DOCX, ODT, RTF and Markdown with styles mapped, through the shared office formats engine. | library | medium | STU-A12-F7-P4 |  |
| STU-A7-F10-P5 | **Layout exchange export.** Writes layout exchange files for people who still use the tool we replace, only if an owner clears the format. | library | large | STU-A7-F10-P2, STU-A7-F10-P3 |  |
| STU-A7-F11-P1 | **Slide export.** Exports Layout pages as an ODP slide file through the shared office formats engine; Studio builds no slide model or player of its own, since Spare Office has them. | library | medium | STU-A7-F1-P1, STU-A12-F7-P4 |  |

## STU-A8 Spare Raw: photo library and raw development

Bring photos in, organise tens of thousands of them, develop raw files without touching the originals, and share, print or make books from them. Replaces Lightroom and Lightroom Classic.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A8-F1 Import photos | Import from cards, cameras, phones and folders, copying, renaming, backing up and applying presets on the way in. | core | P1 Photo importer; P2 Photo catalogue |
| STU-A8-F2 Organise and find photos | Rate, flag, colour-label and keyword photos, group them into collections and search by any detail. | core | P1 Grid, loupe and compare views; P2 Collections and smart collections; P3 Keywords and people; P4 Duplicates and similar shots; P5 Stacks and virtual copies; P6 XMP sidecars |
| STU-A8-F3 Develop raw photos | Set white balance, exposure, tone curve, colour, detail, lens corrections and geometry on raw files. | core | P1 Raw decoding licences; P2 Raw decoder; P3 Raw development pipeline; P4 Camera colour profiles; P5 Develop panels |
| STU-A8-F4 Make local adjustments | Adjust parts of a photo with brushes, gradients, colour and brightness ranges, and automatic subject, sky and people masks. | core | P1 Local adjustments; P2 Automatic masks |
| STU-A8-F5 Remove noise and blemishes | Clean noise from raw files and remove dust spots and distractions. | next | P1 Raw noise reduction; P2 Spot and distraction removal |
| STU-A8-F6 Apply presets and sync edits | Apply presets and copy edits across many photos at once. | core | P1 Develop presets; P2 Copy and sync settings |
| STU-A8-F7 Merge raw photos | Merge raw photos into HDR images and panoramas that stay raw. | next | P1 Raw HDR and panorama merge |
| STU-A8-F8 Shoot tethered | Shoot with the camera linked to the computer and see each photo arrive as it is taken. | later | P1 Tethered capture |
| STU-A8-F9 Place photos on a map | See where photos were taken and add locations from a map or a GPS track. | later | P1 Map and geotagging |
| STU-A8-F10 Make books, slideshows, prints and galleries | Lay out photo books, play slideshows, print contact sheets and publish web galleries. | later | P1 Photo books; P2 Slideshows; P3 Print layouts; P4 Web galleries |
| STU-A8-F11 Edit photos on a phone | Pick, edit and share photos on a phone, synced with the computer. | next | P1 Phone app |
| STU-A8-F12 Export and share | Export sized copies and DNG files with sharpening, watermarks, HDR gain maps and choices about which metadata to keep. | core | P1 Export recipes; P2 DNG export |
| STU-A8-F13 Bring a catalogue from another tool | Move photos, ratings, keywords and collections from another catalogue; edits stay behind, since other tools' develop settings have no published specification. | next | P1 Catalogue import |
| STU-A8-F14 Work with photos on other drives | Keep editing when originals sit on a disconnected drive, relink folders that moved and back up the catalogue safely. | next | P1 Offline previews; P2 Relink moved folders; P3 Catalogue backup and integrity checks |
| STU-A8-F15 Cull, proof and order prints | Pick the best shots faster, share proofing galleries with clients, collect photos in shared albums and order prints from a lab. | later | P1 Assisted culling; P2 Client proofing galleries; P3 Shared albums; P4 Print lab orders |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A8-F1-P1 | **Photo importer.** Finds cards and devices, copies and renames files, makes a backup copy and applies presets. | library | medium | kit-store, STU-A2-F6-P4 |  |
| STU-A8-F1-P2 | **Photo catalogue.** A fast local catalogue of photos, edits and metadata that syncs only if the person chooses. | data | large | kit-store, kit-sync |  |
| STU-A8-F2-P1 | **Grid, loupe and compare views.** Grid, single, compare and survey views for picking the best shots fast. | interface | medium | STU-A8-F1-P2 |  |
| STU-A8-F2-P2 | **Collections and smart collections.** Hand-picked collections and collections built from rules. | library | small | STU-A8-F1-P2 |  |
| STU-A8-F2-P3 | **Keywords and people.** Keyword lists and optional face grouping that runs only on the device by default; face groups sync to the cloud only under the face data rules. | library | medium | STU-A8-F1-P2, STU-A16-F1-P2, STU-A20-F1-P5 |  |
| STU-A8-F2-P4 | **Duplicates and similar shots.** Groups bursts and near-duplicates so they can be culled together. | library | small | STU-A8-F1-P2 |  |
| STU-A8-F2-P5 | **Stacks and virtual copies.** Stacks related shots and makes virtual copies with their own edits. | library | small | STU-A8-F1-P2 |  |
| STU-A8-F2-P6 | **XMP sidecars.** Writes ratings, labels, keywords and collections to standard XMP sidecar files beside the photos, so years of organising work leave with the member in a form any tool reads. | library | small | STU-A2-F6-P4, STU-A8-F1-P2 |  |
| STU-A8-F3-P1 | **Raw decoding licences.** Licences of open raw libraries (rawler and rawloader are LGPL-2.1; LibRaw is LGPL-2.1 or CDDL-1.0), DNG patent terms, and whether to write our own decoder. | legal | small | STU-A20-F4-P5 | [S06](STATUS.md) |
| STU-A8-F3-P2 | **Raw decoder.** Reads raw files from many cameras, and DNG, from published specifications; an LGPL raw library is used only in the way the co-op LGPL decision allows, or we write our own decoder. | library | large | kit-core, STU-A8-F3-P1, STU-A20-F4-P5 |  |
| STU-A8-F3-P3 | **Raw development pipeline.** Demosaic, white balance, tone and colour on the GPU in a scene-referred pipeline. | library | large | STU-A2-F8-P2, STU-A3-F3-P2, STU-A8-F3-P2 |  |
| STU-A8-F3-P4 | **Camera colour profiles.** Open camera colour profiles, and a way to make new ones from colour charts. | data | medium | STU-A3-F3-P2 |  |
| STU-A8-F3-P5 | **Develop panels.** Panels for every development control, with before and after views. | interface | medium | STU-A8-F3-P3 |  |
| STU-A8-F4-P1 | **Local adjustments.** Brush, linear, radial, colour range and brightness range masks. | library | medium | STU-A8-F3-P3 |  |
| STU-A8-F4-P2 | **Automatic masks.** Subject, sky, background and people masks from models that run on the device. | library | medium | STU-A16-F1-P2, STU-A8-F4-P1, STU-A16-F1-P7 |  |
| STU-A8-F5-P1 | **Raw noise reduction.** Noise reduction on raw data, with an optional learned model on the device. | library | large | STU-A8-F3-P3, STU-A5-F3-P2 |  |
| STU-A8-F5-P2 | **Spot and distraction removal.** Healing and removal inside the develop view. | library | medium | STU-A5-F3-P1 |  |
| STU-A8-F6-P1 | **Develop presets.** Our open preset format, and import of preset formats that have a published specification. | data | small | STU-A8-F3-P3 |  |
| STU-A8-F6-P2 | **Copy and sync settings.** Copies chosen settings to many photos, with automatic syncing while editing. | library | small | STU-A8-F3-P3 |  |
| STU-A8-F7-P1 | **Raw HDR and panorama merge.** Merges raw files into a single raw-like file using Spare Photo's merge methods. | library | medium | STU-A5-F10-P1, STU-A5-F10-P2 |  |
| STU-A8-F8-P1 | **Tethered capture.** Camera control over USB through open camera protocols. | integration | large |  |  |
| STU-A8-F9-P1 | **Map and geotagging.** A map of photos and tagging from GPS tracks, on open map data. | interface | medium | svc-maps, STU-A8-F1-P2 |  |
| STU-A8-F10-P1 | **Photo books.** Book layouts exported as PDF for any printer. | interface | medium | STU-A7-F1-P1 |  |
| STU-A8-F10-P2 | **Slideshows.** Slideshows with music, exported as video. | interface | small | STU-A9-F10-P1 |  |
| STU-A8-F10-P3 | **Print layouts.** Contact sheets and picture packages. | interface | small | STU-A3-F6-P1 |  |
| STU-A8-F10-P4 | **Web galleries.** Publishes a gallery through a share link or Spare Social. | integration | small | STU-A17-F2-P1, STU-A17-F2-P3 |  |
| STU-A8-F11-P1 | **Phone app.** A phone version of the library and develop views. | interface | large | STU-A8-F3-P3, kit-sync, STU-A19-F6-P3 | [K04](../kit/STATUS.md) |
| STU-A8-F12-P1 | **Export recipes.** Export recipes with size, sharpening, watermark, metadata and location-removal options. | library | small | STU-A3-F7-P1, STU-A2-F6-P5 |  |
| STU-A8-F12-P2 | **DNG export.** Exports raw photos as DNG from the published specification, with edits kept in XMP, so originals and edits can move to any tool. | library | medium | STU-A8-F3-P2, STU-A8-F2-P6, STU-A8-F3-P1 |  |
| STU-A8-F13-P1 | **Catalogue import.** Reads ratings, keywords and collections from XMP sidecars and open catalogue formats. | library | medium | STU-A2-F6-P4, STU-A8-F1-P2, STU-A2-F7-P1 |  |
| STU-A8-F14-P1 | **Offline previews.** Keeps light, editable previews of photos whose originals are on a disconnected drive and applies the edits to the originals when the drive returns. | library | medium | STU-A8-F1-P2, STU-A8-F3-P3 |  |
| STU-A8-F14-P2 | **Relink moved folders.** Finds photos whose folders were moved or renamed and relinks them in one step. | library | small | STU-A8-F1-P2 |  |
| STU-A8-F14-P3 | **Catalogue backup and integrity checks.** Backs up the catalogue on a schedule, checks the catalogue and its backups for damage and restores from a chosen backup. | library | medium | STU-A8-F1-P2, kit-store |  |
| STU-A8-F15-P1 | **Assisted culling.** Flags blinks, missed focus and near-duplicates with on-device models so the person picks faster; it suggests and never deletes. | library | medium | STU-A8-F2-P4, STU-A16-F1-P2 |  |
| STU-A8-F15-P2 | **Client proofing galleries.** A share link where clients pick and comment on photos, with their picks and comments synced back to the catalogue as flags. | service | medium | STU-A8-F10-P4, STU-A17-F3-P1, STU-A17-F2-P1 |  |
| STU-A8-F15-P3 | **Shared albums.** Albums several people can add photos to, with the owner deciding who may add and who may see. | service | medium | STU-A17-F2-P1, STU-A8-F2-P2 |  |
| STU-A8-F15-P4 | **Print lab orders.** Sends print orders to labs through their public ordering APIs where their terms allow it; the lab sells and is paid directly, never through the co-op. | integration | medium | STU-A18-F5-P1, STU-A8-F10-P3 |  |

## STU-A9 Spare Video: video editing and encoding

Edit video from the first rough cut to a finished film, with captions, colour, sound and export to every screen. Replaces Premiere Pro, Media Encoder and the phone video editors. Shares its media engine with Spare Social.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A9-F1 Bring in and organise footage | Import clips, make light editing copies, sort them into bins and search by content or by what was said. | core | P1 Codec and patent approach; P3 Video and audio decoding; P4 Proxies and media cache; P5 Bins, markers and metadata; P6 Camera and HDR video format terms |
| STU-A9-F2 Cut on a timeline | Cut, trim, ripple, roll, slip and slide clips on many video and audio tracks. | core | P1 Timeline model; P2 Timeline interface; P3 Real-time playback; P4 Source and program monitors |
| STU-A9-F3 Edit by text | Edit a video by editing its transcript, and find any moment by searching what was said. | next | P1 Speech to text; P2 Transcript editing |
| STU-A9-F4 Add captions and subtitles | Make, style, translate and export captions, subtitles and audio description, in web and broadcast formats. | core | P1 Caption tracks; P2 Subtitle formats; P3 Caption translation; P4 Audio description tracks; P5 Broadcast caption formats |
| STU-A9-F5 Grade colour | Correct and grade colour with wheels, curves, LUTs, scopes and HDR. | core | P1 Colour grading tools; P2 Video scopes; P3 HDR video |
| STU-A9-F6 Add effects, transitions and titles | Use transitions, effects, keyframes, titles and motion templates made in Spare Motion. | core | P1 Video effects and transitions; P2 Titles and graphics |
| STU-A9-F7 Mix sound for video | Mix tracks, clean up dialogue, lower music under speech and add audio effects. | core | P1 Audio mixer; P2 Dialogue clean-up and ducking |
| STU-A9-F8 Edit several cameras at once | Sync camera angles by sound or timecode and switch between them while playing. | next | P1 Multi-camera editing |
| STU-A9-F9 Stabilise, reframe and change speed | Smooth shaky footage, reframe for vertical and square screens, and speed clips up or down smoothly. | next | P1 Stabiliser; P2 Automatic reframing; P3 Speed changes and time remapping |
| STU-A9-F10 Export and encode | Export to delivery formats and presets, queue jobs and use watch folders. | core | P1 Encoder; P2 Video export queue; P3 Delivery presets |
| STU-A9-F11 Exchange projects with other editors | Move timelines to and from other editing tools through open interchange formats. | next | P1 Timeline interchange |
| STU-A9-F12 Edit on a phone or tablet | Make quick edits on a phone and finish on a computer. | later | P1 Phone and tablet editor |
| STU-A9-F13 Manage media and re-edit finished films | Find the cuts in a finished video, relink missing media, and consolidate, transcode and archive a project. | next | P1 Scene-cut detection; P2 Relink offline media; P3 Consolidate, transcode and archive |
| STU-A9-F14 Edit 360-degree and VR video | Edit 360-degree and VR video and look around it while editing. | later | P1 360 and VR video |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A9-F1-P1 | **Codec and patent approach.** One engine-media study of which codecs to decode and encode and how to avoid patent and licence risk, merging Spare Social's codec and pipeline studies and Spare OS's codec research. | research | medium | STU-A20-F4-P5 | [S04](STATUS.md) |
| STU-A9-F1-P3 | **Video and audio decoding.** Decodes common camera and phone formats, using hardware decoding where the platform offers it. | library | large | engine-media, STU-A9-F1-P1, STU-A9-F1-P6 |  |
| STU-A9-F1-P4 | **Proxies and media cache.** Makes light editing copies and caches previews and audio waveforms. | library | medium | engine-media |  |
| STU-A9-F1-P5 | **Bins, markers and metadata.** Project bins, labels, markers and searchable clip details. | interface | medium | STU-A2-F3-P2 |  |
| STU-A9-F1-P6 | **Camera and HDR video format terms.** Licence terms for the ProRes, Blackmagic RAW, R3D and ARRIRAW SDKs, CinemaDNG, DNxHR (published as SMPTE VC-3), Dolby Vision and HDR10+, as part of the codec study. | legal | medium | STU-A9-F1-P1 | [S04](STATUS.md) |
| STU-A9-F2-P1 | **Timeline model.** One timeline model for sequences, tracks, clips and edits as operations, built in engine-media and shared by Video, Sound, Motion, Animate, Photo's frame timeline and Spare Social's short video editor. | library | large | engine-media, STU-A2-F3-P2, kit-ops | [S02](STATUS.md) |
| STU-A9-F2-P2 | **Timeline interface.** A fast timeline with trimming tools, snapping and keyboard editing. | interface | large | STU-A9-F2-P1 |  |
| STU-A9-F2-P3 | **Real-time playback.** Smooth playback with effects, using the GPU and render caching. | library | large | engine-media, STU-A2-F8-P2 |  |
| STU-A9-F2-P4 | **Source and program monitors.** Viewers with in and out points, safe-area overlays and timecode. | interface | medium | STU-A9-F2-P3 |  |
| STU-A9-F3-P1 | **Speech to text.** Speech recognition with an open model on the device, from the shared on-device model engine that Spare Social, Spare OS and Spare Office's captions also use. | library | medium | STU-A16-F1-P2, STU-A16-F1-P8 |  |
| STU-A9-F3-P2 | **Transcript editing.** Cuts the timeline when the person deletes or moves words in the transcript. | interface | medium | STU-A9-F3-P1, STU-A9-F2-P1 |  |
| STU-A9-F4-P1 | **Caption tracks.** Caption tracks with styles, timing and burn-in. | library | medium | STU-A9-F2-P1 |  |
| STU-A9-F4-P2 | **Subtitle formats.** Reads and writes SRT, WebVTT and other open subtitle formats. | library | small |  |  |
| STU-A9-F4-P3 | **Caption translation.** Translates captions with an on-device translation model. | library | small | STU-A16-F1-P2, STU-A16-F1-P8, KIT-A19-F10-P3 |  |
| STU-A9-F4-P4 | **Audio description tracks.** Audio description tracks on the timeline, recorded or read by a chosen voice, exported as a separate or mixed track. | library | medium | STU-A9-F2-P1, STU-A11-F1-P1 |  |
| STU-A9-F4-P5 | **Broadcast caption formats.** Reads and writes TTML and IMSC, SCC, and CEA-608 and 708 captions from their published specifications, for broadcasters who reject SRT and WebVTT. | library | medium | STU-A9-F4-P1, STU-A9-F4-P2 |  |
| STU-A9-F5-P1 | **Colour grading tools.** Colour wheels, curves, qualifiers and LUTs on the GPU. | library | large | STU-A2-F8-P2, STU-A3-F3-P2 |  |
| STU-A9-F5-P2 | **Video scopes.** Waveform, vectorscope, histogram and RGB parade. | interface | medium | STU-A9-F2-P3 |  |
| STU-A9-F5-P3 | **HDR video.** HDR grading and output with the right HDR metadata. | library | medium | STU-A3-F3-P2, STU-A9-F1-P6 |  |
| STU-A9-F6-P1 | **Video effects and transitions.** Effects and transitions with keyframes, on the GPU. | library | large | STU-A2-F8-P2, STU-A10-F1-P1 |  |
| STU-A9-F6-P2 | **Titles and graphics.** Titles, lower thirds and motion templates. | library | medium | STU-A3-F1-P3 |  |
| STU-A9-F7-P1 | **Audio mixer.** Track and clip mixing with meters and automation. | library | medium | STU-A11-F1-P1 |  |
| STU-A9-F7-P2 | **Dialogue clean-up and ducking.** Noise reduction, level matching for speech and automatic music ducking. | library | medium | STU-A11-F2-P1 |  |
| STU-A9-F8-P1 | **Multi-camera editing.** Syncs clips by sound or timecode and cuts between angles live. | library | medium | STU-A9-F2-P1 |  |
| STU-A9-F9-P1 | **Stabiliser.** Smooths camera shake from the footage itself. | library | large | STU-A2-F8-P2 |  |
| STU-A9-F9-P2 | **Automatic reframing.** Keeps the subject in frame when changing the shape of the picture. | library | medium | STU-A16-F1-P2, STU-A16-F1-P7 |  |
| STU-A9-F9-P3 | **Speed changes and time remapping.** Speed ramps, freeze frames and frame blending. | library | medium | STU-A9-F2-P1 |  |
| STU-A9-F10-P1 | **Encoder.** Encodes to delivery formats, using hardware encoding where the platform offers it. | library | large | engine-media, STU-A9-F1-P1 |  |
| STU-A9-F10-P2 | **Video export queue.** Spare Video's export settings and watch folders on the shared job queue. | library | small | STU-A2-F11-P1, STU-A9-F10-P1 |  |
| STU-A9-F10-P3 | **Delivery presets.** Presets for the web, broadcast, phones and social services. | data | small |  |  |
| STU-A9-F11-P1 | **Timeline interchange.** Reads and writes OpenTimelineIO, EDL and other published timeline formats. | library | medium | STU-A9-F2-P1 |  |
| STU-A9-F12-P1 | **Phone and tablet editor.** A simple touch editor synced with the desktop project. | interface | large | STU-A9-F2-P1, kit-sync, STU-A19-F6-P3 | [K04](../kit/STATUS.md) |
| STU-A9-F13-P1 | **Scene-cut detection.** Finds the cuts in a finished video and splits it into clips on the timeline. | library | medium | STU-A9-F1-P3, STU-A9-F2-P1 |  |
| STU-A9-F13-P2 | **Relink offline media.** Finds media that moved or went offline and relinks it by name, timecode and length. | library | small | STU-A9-F1-P5, STU-A2-F3-P4 |  |
| STU-A9-F13-P3 | **Consolidate, transcode and archive.** Copies only the media a project uses, with handles, can transcode it, and packs the project for hand-over or archive. | library | medium | STU-A9-F1-P4, STU-A2-F11-P1 |  |
| STU-A9-F14-P1 | **360 and VR video.** Edits 360-degree and stereo VR video with a look-around viewer and keeps the published spherical video metadata on export. | library | large | STU-A9-F2-P3, engine-render |  |

## STU-A10 Spare Motion: motion graphics and visual effects

Animated titles and graphics, compositing, keying, tracking and visual effects, plus templates that Spare Video uses. Replaces After Effects.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A10-F1 Animate anything with keyframes | Animate any property with keyframes and easing, and shape the motion in a graph editor. | core | P1 Keyframe engine; P2 Graph editor; P3 Compositions; P4 Motion workspace |
| STU-A10-F2 Composite and key | Combine footage using keying, masks, track mattes and blend modes. | core | P1 Keying; P2 Masks and track mattes; P3 Rotoscoping |
| STU-A10-F3 Track motion | Track points, planes and the camera through footage, and attach text or graphics to them. | next | P1 Point and plane tracking; P2 3D camera tracking |
| STU-A10-F4 Work in 3D space | Place layers and 3D models in 3D space with cameras, lights and depth of field. | next | P1 3D layers, cameras and lights |
| STU-A10-F5 Drive animation with expressions | Link properties with short expressions so things move together. | next | P1 Expression language |
| STU-A10-F6 Make motion templates | Package an animation with simple controls so others can change it in Spare Video. | next | P1 Motion templates |
| STU-A10-F7 Animate text and shapes | Animate text by letter, word or line, and animate shape layers with trim paths and repeaters. | core | P1 Text animators; P2 Shape layers |
| STU-A10-F8 Add effects and particles | Use blurs, glows, distortions, noise, particles and generators. | next | P1 Effects set; P2 Effect plugins |
| STU-A10-F9 Render and export | Render to video, image sequences and web animation files. | core | P1 Motion render settings; P2 Web animation export; P3 Image sequences |
| STU-A10-F10 Remove objects from video | Remove an object or person from a shot and fill the gap across every frame. | next | P1 Content-aware fill for video |
| STU-A10-F11 Bend and blur motion | Bend layers with pins over time and add motion blur to anything that moves. | later | P1 Puppet pins; P2 Motion blur |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A10-F1-P1 | **Keyframe engine.** Keyframes and easing for every property, built in engine-media beside the shared timeline model, so every Studio app and Spare Office's slide animation can use them. | library | large | engine-media, STU-A2-F3-P2, STU-A9-F2-P1 |  |
| STU-A10-F1-P2 | **Graph editor.** Edit speed and value curves between keyframes. | interface | medium | STU-A10-F1-P1 |  |
| STU-A10-F1-P3 | **Compositions.** Nested compositions with layers, timing, blending and parenting, on the shared timeline model. | library | large | STU-A9-F2-P1 |  |
| STU-A10-F1-P4 | **Motion workspace.** Composition viewer, layer timeline and effect controls. | interface | large | STU-A10-F1-P3 |  |
| STU-A10-F2-P1 | **Keying.** Colour and brightness keying with spill removal and edge controls. | library | medium | STU-A2-F8-P2 |  |
| STU-A10-F2-P2 | **Masks and track mattes.** Animated masks with feather and expansion, and track mattes. | library | medium | STU-A5-F4-P2, STU-A10-F1-P1 |  |
| STU-A10-F2-P3 | **Rotoscoping.** Assisted rotoscoping that follows an object through a shot, with an on-device model. | library | large | STU-A16-F1-P2, STU-A10-F2-P2 |  |
| STU-A10-F3-P1 | **Point and plane tracking.** Tracks points and flat surfaces through footage. | library | large | STU-A2-F8-P2 |  |
| STU-A10-F3-P2 | **3D camera tracking.** Works out the camera's movement from footage so 3D layers sit in the shot. | library | large | STU-A10-F3-P1 |  |
| STU-A10-F4-P1 | **3D layers, cameras and lights.** A 3D scene for layers and imported models, drawn by the shared renderer. | library | large | engine-render, STU-A10-F1-P3 |  |
| STU-A10-F5-P1 | **Expression language.** A sandboxed expression language with a library of common behaviours. | library | large | kit-plugin, STU-A10-F1-P1, STU-A20-F5-P4 |  |
| STU-A10-F6-P1 | **Motion templates.** Exposes chosen properties as simple controls inside Spare Video. | library | medium | STU-A10-F1-P1, STU-A9-F6-P2 |  |
| STU-A10-F7-P1 | **Text animators.** Per-letter, per-word and per-line animators for text. | library | medium | STU-A3-F1-P3, STU-A10-F1-P1 |  |
| STU-A10-F7-P2 | **Shape layers.** Vector shape layers with trim paths, repeaters and path morphing. | library | medium | STU-A6-F1-P1, STU-A10-F1-P1 |  |
| STU-A10-F8-P1 | **Effects set.** GPU effects including particles, noise, glows and distortions. | library | large | STU-A2-F8-P2 |  |
| STU-A10-F8-P2 | **Effect plugins.** Effect plugins from other makers written to the open OpenFX standard, run in the native plugin host on desktop only. | integration | medium | STU-A18-F3-P6 |  |
| STU-A10-F9-P1 | **Motion render settings.** Motion's render settings on the shared job queue, using many processor cores and the GPU. | library | medium | STU-A2-F11-P1, STU-A10-F1-P3 |  |
| STU-A10-F9-P2 | **Web animation export.** Exports to the open Lottie animation format. | library | medium | STU-A10-F1-P1 |  |
| STU-A10-F9-P3 | **Image sequences.** OpenEXR and PNG image sequence import and export. | library | small | STU-A2-F6-P1 |  |
| STU-A10-F10-P1 | **Content-aware fill for video.** Fills a tracked mask across frames from nearby frames using published methods, with an optional on-device model. | library | large | STU-A10-F2-P3, STU-A10-F3-P1, STU-A5-F3-P2 |  |
| STU-A10-F11-P1 | **Puppet pins.** Pin-based mesh deformation of layers over time, sharing its mesh deformation with Spare Animate's bones. | library | medium | STU-A14-F2-P2, STU-A10-F1-P1 |  |
| STU-A10-F11-P2 | **Motion blur.** Motion blur for moving layers, shapes and 3D layers, with shutter angle and sample settings. | library | medium | STU-A10-F1-P3, STU-A2-F8-P2 |  |

## STU-A11 Spare Sound: audio editing, mixing and podcasts

Record, edit, repair and mix audio for podcasts, video, radio and music. Replaces Audition. Shares its audio engine with Spare Video and Spare Motion.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A11-F1 Record and edit audio | Record from microphones and edit waveforms precisely. | core | P1 Audio engine; P2 Waveform editor; P3 Audio formats; P4 Audio latency study; P5 Low-latency driver choice |
| STU-A11-F2 Repair and clean sound | Remove noise, hum, clicks, clipping and room echo. | core | P1 Audio effects; P2 Spectral editor; P3 Speech enhancement |
| STU-A11-F3 Mix many tracks | Mix multitrack sessions with buses, sends, automation and meters. | core | P1 Multitrack sessions; P2 Mixer view |
| STU-A11-F4 Use audio plugins | Load effect and instrument plugins made to open plugin standards, and Audio Unit plugins on macOS, on the desktop apps. | next | P1 Audio plugin hosting; P2 Audio Unit hosting on macOS |
| STU-A11-F5 Make podcasts | Record guests, edit by transcript, match loudness and publish episodes. | next | P1 Podcast workflow; P2 Loudness matching; P3 Remote guest recording |
| STU-A11-F6 Analyse and batch process | Analyse frequency, phase and loudness and process many files at once. | later | P1 Analysis tools; P2 Batch processing |
| STU-A11-F7 Edit sound to picture | Edit and mix audio against video with timecode. | later | P1 Video in sessions |
| STU-A11-F8 Fit sound to length | Make music and voice longer or shorter without changing pitch, and change pitch without changing length. | next | P1 Time stretch and pitch shift |
| STU-A11-F9 Mix in surround | Mix for 5.1, 7.1 and Ambisonics with surround panning and monitoring. | later | P1 Surround and Ambisonics mixing |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A11-F1-P1 | **Audio engine.** Low-latency playback, recording and mixing on every platform, built in engine-media and shared with Spare Video, Spare Motion and Spare Social. | library | large | engine-media, STU-A11-F1-P4, STU-A11-F1-P5 |  |
| STU-A11-F1-P2 | **Waveform editor.** Cut, copy, fade, normalise and edit on zero crossings. | interface | medium | STU-A11-F1-P1 |  |
| STU-A11-F1-P3 | **Audio formats.** Reads and writes WAV, FLAC, Ogg, Opus, MP3 and AAC where licences allow. | library | medium | engine-media, STU-A9-F1-P1 |  |
| STU-A11-F1-P4 | **Audio latency study.** How low the audio latency can go on Windows, macOS, Linux and in the browser, measured against professional tools, before the audio engine is built. | research | medium |  |  |
| STU-A11-F1-P5 | **Low-latency driver choice.** On Windows, WASAPI, or Steinberg's proprietary ASIO licence, since ASIO's other licence is GPLv3; Core Audio on macOS and PipeWire on Linux. The VST 3 SDK has been MIT since October 2025, so it raises no licence problem. | legal | small |  |  |
| STU-A11-F2-P1 | **Audio effects.** EQ, compression, limiting, noise reduction, de-click, de-hum and de-reverb. | library | large | STU-A11-F1-P1 |  |
| STU-A11-F2-P2 | **Spectral editor.** Shows sound as a spectrogram so unwanted sounds can be painted out. | interface | medium | STU-A11-F1-P1 |  |
| STU-A11-F2-P3 | **Speech enhancement.** Makes voices clearer with an open model that runs on the device. | library | medium | STU-A16-F1-P2 |  |
| STU-A11-F3-P1 | **Multitrack sessions.** Tracks, buses, sends, automation and clip editing on the shared timeline model. | library | large | STU-A11-F1-P1, STU-A2-F3-P2, STU-A9-F2-P1 |  |
| STU-A11-F3-P2 | **Mixer view.** A mixing desk view with meters, plugins and routing. | interface | medium | STU-A11-F3-P1 |  |
| STU-A11-F4-P1 | **Audio plugin hosting.** Hosts CLAP, LV2 and VST 3 effect and instrument plugins in the native plugin host, each in its own process so a crash cannot lose work; desktop only. | integration | large | STU-A11-F1-P1, STU-A18-F3-P7 |  |
| STU-A11-F4-P2 | **Audio Unit hosting on macOS.** Hosts Audio Unit plugins on macOS through Apple's public plugin interfaces, in the native plugin host. | integration | medium | STU-A18-F3-P7, STU-A11-F1-P1 |  |
| STU-A11-F5-P1 | **Podcast workflow.** Templates, chapter markers, show notes and transcript editing for episodes. | interface | medium | STU-A11-F3-P1, STU-A9-F3-P1, STU-A16-F1-P8 |  |
| STU-A11-F5-P2 | **Loudness matching.** Measures loudness and matches it to the targets each platform asks for. | library | small | STU-A11-F2-P1 |  |
| STU-A11-F5-P3 | **Remote guest recording.** Records each guest's voice locally and uploads it, for clean remote interviews. | service | large | svc-messaging, STU-A17-F1-P1 |  |
| STU-A11-F6-P1 | **Analysis tools.** Frequency, phase and loudness analysis views. | library | medium | STU-A11-F1-P1 |  |
| STU-A11-F6-P2 | **Batch processing.** Applies effect chains to many files with the shared action recorder and job queue. | library | small | STU-A2-F12-P1, STU-A2-F11-P1, STU-A11-F2-P1 |  |
| STU-A11-F7-P1 | **Video in sessions.** Video playback locked to the session's timecode. | library | small | engine-media, STU-A11-F3-P1 |  |
| STU-A11-F8-P1 | **Time stretch and pitch shift.** Time stretching and pitch shifting for music and voice using published methods, with quality settings for each. | library | medium | STU-A11-F1-P1 |  |
| STU-A11-F9-P1 | **Surround and Ambisonics mixing.** Surround buses, panning and monitoring for 5.1, 7.1 and Ambisonics; immersive formats with their own licence programmes are added only after a licence check. | library | large | STU-A11-F3-P1, STU-A9-F1-P1 |  |

## STU-A12 Spare PDF: read, edit, review, fill and sign

Read any PDF, mark it up, edit it, organise pages, fill and build forms, sign and send for signature, scan paper, protect, redact and make PDFs accessible. Replaces Acrobat, Acrobat Reader, Acrobat Sign and the scan and fill-and-sign phone apps. Shares its PDF engine with Spare Construct and Spare Office.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A12-F1 Read PDFs | Open, search, zoom and read any PDF quickly on any device. | core | P1 PDF parser and writer; P2 PDF viewer; P3 Reading view and read aloud |
| STU-A12-F2 Comment and mark up | Highlight, comment, draw, stamp and reply to other people's comments. | core | P1 Annotations; P2 Comment tools and list |
| STU-A12-F3 Edit text and images | Change text, images and links inside a PDF. | core | P1 Content editing; P2 Edit tools; P3 PDF content editing study |
| STU-A12-F4 Organise pages | Insert, delete, rotate, reorder, split, merge and crop pages, and add headers, footers, watermarks and page numbers. | core | P1 Page operations; P2 Organise view; P3 Headers, footers, watermarks and numbering |
| STU-A12-F5 Fill in and build forms | Fill in any form, and build forms with fields, checks and calculations. | core | P1 Form fields; P2 Field detection; P3 Form builder; P4 Form data collection; P5 XFA form reading |
| STU-A12-F6 Sign PDFs | Sign a PDF themselves with a record of who signed and when, and check the signatures on any PDF. | core | P1 Signature law overseas; P2 Sign a PDF yourself; P3 Digital certificate signatures; P5 Signature trust store |
| STU-A12-F7 Convert to and from PDF | Make PDFs from any file and turn PDFs into editable office files and images. | core | P1 Create PDF; P2 Export from PDF; P3 Page recovery engine; P4 Office formats engine; P5 HTML renderer for web pages |
| STU-A12-F8 Scan paper and recognise text | Scan paper with a phone and make the text searchable and editable. | next | P1 Text recognition; P2 Phone scanning |
| STU-A12-F9 Compare and redact | Compare two versions and permanently remove sensitive content and hidden data. | next | P1 Compare documents; P2 Redaction; P3 Remove hidden data |
| STU-A12-F10 Protect documents | Add passwords, set what others may do and encrypt files. | core | P1 Encryption and permissions |
| STU-A12-F11 Make PDFs accessible | Tag PDFs, set the reading order and alt text, and check against the PDF/UA standard. | next | P1 Tags and reading order; P2 Accessibility checker |
| STU-A12-F12 Meet archive and print standards | Save and check PDF/A for archives, PDF/X for print and PDF/UA for accessibility. | next | P1 Standards conversion and checking; P2 Print production tools |
| STU-A12-F13 Measure drawings | Measure distances, areas and perimeters on scaled drawings. | next | P1 Measuring tools |
| STU-A12-F14 Ask questions about a document | Summarise a PDF and ask questions about it, with answers that point to the page. | later | P1 Document assistant |
| STU-A12-F15 Process many PDFs | Run saved steps on many files and watched folders. | later | P1 PDF actions |
| STU-A12-F16 Bundle files | Put many files into one PDF portfolio or attach files to a PDF. | later | P1 Attachments and portfolios |
| STU-A12-F17 Ask others to sign | Send a PDF for signature, set the signing order, remind people and collect the signed copy. | next | P1 Send for signature |
| STU-A12-F18 Make PDFs smaller | Shrink a PDF for email or the web and see the size and quality before saving. | next | P1 PDF optimiser |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A12-F1-P1 | **PDF parser and writer.** Reads and writes PDF to ISO 32000-2, tolerant of broken files. | library | large | engine-pdf |  |
| STU-A12-F1-P2 | **PDF viewer.** Fast, accurate drawing of any PDF with search, bookmarks, layers and thumbnails. | interface | large | STU-A12-F1-P1, engine-render |  |
| STU-A12-F1-P3 | **Reading view and read aloud.** Reflowed text for small screens, comfortable colours and reading aloud with system voices. | interface | small | STU-A12-F1-P2 |  |
| STU-A12-F2-P1 | **Annotations.** Every standard PDF annotation type, read and written to the specification. | library | medium | STU-A12-F1-P1 |  |
| STU-A12-F2-P2 | **Comment tools and list.** Comment tools and a comment list with replies, status, filters and summaries. | interface | medium | STU-A12-F2-P1, STU-A17-F3-P1 |  |
| STU-A12-F3-P1 | **Content editing.** Edits text runs and images in place, reflowing within paragraphs and keeping fonts where licences allow. | library | large | STU-A12-F1-P1, STU-A3-F1-P2, STU-A12-F3-P3 |  |
| STU-A12-F3-P2 | **Edit tools.** Tools to edit text, images and links, with font matching. | interface | medium | STU-A12-F3-P1 |  |
| STU-A12-F3-P3 | **PDF content editing study.** How far text can be edited in place in real PDFs, with subset fonts, broken text runs and reflow, tested on a set of real files before the editor is built. | research | medium | STU-A12-F1-P1 |  |
| STU-A12-F4-P1 | **Page operations.** Page operations as undoable operations, including merging many files. | library | medium | STU-A12-F1-P1 |  |
| STU-A12-F4-P2 | **Organise view.** A thumbnail view for dragging pages within and between files. | interface | small | STU-A12-F4-P1 |  |
| STU-A12-F4-P3 | **Headers, footers, watermarks and numbering.** Adds headers, footers, watermarks, backgrounds and legal page numbering. | library | small | STU-A12-F1-P1 |  |
| STU-A12-F5-P1 | **Form fields.** Form fields with formats, checks and calculations, to the PDF specification; form scripts run only in the sandbox for active content. | library | large | STU-A12-F1-P1, STU-A20-F5-P4 |  |
| STU-A12-F5-P2 | **Field detection.** Finds the fields in a flat or scanned form automatically. | library | medium | STU-A16-F1-P2, STU-A12-F5-P1 |  |
| STU-A12-F5-P3 | **Form builder.** Tools to add, arrange and test fields. | interface | medium | STU-A12-F5-P1 |  |
| STU-A12-F5-P4 | **Form data collection.** Collects returned forms into a table and exports the data. | library | small | STU-A12-F5-P1 |  |
| STU-A12-F5-P5 | **XFA form reading.** Reads and fills the XFA forms some government agencies still use, from the published XFA specification, if the format check clears it. | library | large | STU-A12-F5-P1, STU-A2-F7-P1 |  |
| STU-A12-F6-P1 | **Signature law overseas.** What Australia, the EU and the US add to the kit's New Zealand signature rules, for members who sign with people overseas. | legal | small | KIT-A25-F1-P1 |  |
| STU-A12-F6-P2 | **Sign a PDF yourself.** Places a drawn, typed or image signature on a PDF with a tamper-evident record, on the kit's signature records. | library | small | STU-A12-F1-P1, KIT-A25-F1-P2, STU-A12-F6-P1 |  |
| STU-A12-F6-P3 | **Digital certificate signatures.** Certificate signatures and checking, including long-term validation, built in engine-pdf so Spare Office and the kit's signing flow use them too. | library | large | engine-pdf, STU-A12-F6-P5 |  |
| STU-A12-F6-P5 | **Signature trust store.** Checks signatures against the EU trust list and the system's certificate stores, since the list Acrobat uses is Adobe's own. | data | medium | engine-pdf |  |
| STU-A12-F7-P1 | **Create PDF.** Print to PDF from any program, and convert office files, web pages and images to PDF through the shared office formats engine and the chosen HTML renderer. | integration | medium | STU-A12-F1-P1, STU-A12-F7-P4, STU-A12-F7-P5 |  |
| STU-A12-F7-P2 | **Export from PDF.** Converts PDF to DOCX, ODT, XLSX, images and plain text with the shared page recovery and office formats engines, the same ones Spare Office uses. | library | medium | STU-A12-F7-P3, STU-A12-F7-P4 |  |
| STU-A12-F7-P3 | **Page recovery engine.** Rebuilds paragraphs, columns, tables and images from PDF pages, built in engine-pdf as one engine for PDF to layout, PDF to office files and Spare Office's PDF conversion. | library | large | engine-pdf, STU-A12-F1-P1 |  |
| STU-A12-F7-P4 | **Office formats engine.** Reads and writes DOCX, XLSX, PPTX, ODT, ODS, ODP and RTF as a shared engine below both Spare Studio and Spare Office, so neither program depends on the other's code. | library | large | engine-text | [OF02](../spare-office/STATUS.md) |
| STU-A12-F7-P5 | **HTML renderer for web pages.** Which renderer turns web pages into PDF: the platform's own web view or an open engine such as Servo (MPL-2.0), since nothing else in the map renders HTML. | research | small |  |  |
| STU-A12-F8-P1 | **Text recognition.** Text recognition on the device with open models and many languages, from the shared on-device model engine that Spare OS and Spare Office also use. | library | medium | STU-A16-F1-P2 |  |
| STU-A12-F8-P2 | **Phone scanning.** Edge finding, perspective fixing and clean-up for photos of paper, built once in engine-pdf as scan to PDF and shared with Spare Office's and Spare OS's scanners. | interface | medium | STU-A12-F8-P1, engine-pdf, STU-A19-F6-P3 | [K04](../kit/STATUS.md) |
| STU-A12-F9-P1 | **Compare documents.** Shows text and visual differences between two PDFs. | library | medium | STU-A12-F1-P1 |  |
| STU-A12-F9-P2 | **Redaction.** Removes chosen content for good, including text under it and matching search terms. | library | medium | STU-A12-F1-P1 |  |
| STU-A12-F9-P3 | **Remove hidden data.** Removes hidden data, scripts, attachments and metadata. | library | small | STU-A12-F1-P1 |  |
| STU-A12-F10-P1 | **Encryption and permissions.** AES encryption and permission settings to the specification. | library | small | STU-A12-F1-P1 |  |
| STU-A12-F11-P1 | **Tags and reading order.** Builds and edits the structure tree and reading order. | library | large | STU-A12-F1-P1 |  |
| STU-A12-F11-P2 | **Accessibility checker.** Checks against PDF/UA and WCAG and helps fix what fails. | library | medium | STU-A12-F11-P1 |  |
| STU-A12-F12-P1 | **Standards conversion and checking.** Converts to and checks PDF/A, PDF/X and PDF/UA. | library | large | STU-A12-F1-P1, STU-A3-F3-P2 |  |
| STU-A12-F12-P2 | **Print production tools.** Preflight, output preview and fixes for print. | library | medium | STU-A3-F5-P2, STU-A12-F12-P1 |  |
| STU-A12-F13-P1 | **Measuring tools.** Calibrated measuring on scaled drawings, with results kept as comments. | library | small | STU-A12-F2-P1 |  |
| STU-A12-F14-P1 | **Document assistant.** Summaries and answers with page references, using a model the member chooses. | integration | medium | STU-A16-F2-P1, STU-A12-F1-P1, KIT-A17-F5-P1 |  |
| STU-A12-F15-P1 | **PDF actions.** Saved steps such as convert, stamp, compress and recognise text, run on many files with the shared action recorder and job queue. | library | small | STU-A2-F12-P1, STU-A2-F11-P1, STU-A12-F4-P1, STU-A12-F18-P1 |  |
| STU-A12-F16-P1 | **Attachments and portfolios.** Attachments and PDF portfolios read and written to the specification. | library | small | STU-A12-F1-P1 |  |
| STU-A12-F17-P1 | **Send for signature.** Spare PDF's door into the kit's signing flow, which handles signing order, reminders, the audit trail and a signed copy for every party. | integration | small | KIT-A25-F1-P2, STU-A12-F6-P2 |  |
| STU-A12-F18-P1 | **PDF optimiser.** Compresses PDFs by resampling images, subsetting fonts and removing unused objects, with a preview of size and quality. | library | medium | STU-A12-F1-P1 |  |

## STU-A13 Spare Screens: interface design, prototypes and websites

Design app and web screens, build clickable prototypes, keep design systems, hand off to developers and publish simple websites. Replaces XD and Dreamweaver, and the screen-design tools people moved to after XD. Penpot (MPL-2.0) is the first project to learn from or partner with.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A13-F1 Design screens | Lay out screens with frames, automatic layout, constraints and grids that resize like real apps. | core | P1 Frames and automatic layout; P2 Screen design workspace; P3 Study Penpot |
| STU-A13-F2 Build components and design systems | Make components with variants and properties, share them as libraries and use design tokens. | core | P1 Components and variants; P2 Design tokens |
| STU-A13-F3 Prototype interactions | Link screens, add transitions, overlays, scrolling and simple logic, and try it on a phone. | core | P1 Prototype engine; P2 Prototype player |
| STU-A13-F5 Hand off to developers | Inspect sizes and spacing, export assets and copy code for each element. | core | P1 Inspect view; P2 Code export; P3 Accessible HTML |
| STU-A13-F6 Bring in work from other screen tools | Import files from open screen-design tools, formats with a published specification, and the main screen-design tool through its public API. | next | P1 Screen-design format check; P2 Open format import; P3 Import through a design tool's public API |
| STU-A13-F7 Build simple websites | Design and publish a simple website, and edit its HTML and CSS by hand. | later | P1 Site publisher; P2 Code editor |
| STU-A13-F8 Plan on a whiteboard | Brainstorm and gather mood boards on an open board, the same whiteboard Spare Office uses. | later | P1 Mood boards on the shared whiteboard |
| STU-A13-F9 Branch a design | Branch a screen design, try changes and merge them back after review. | later | P1 Design branches |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A13-F1-P1 | **Frames and automatic layout.** Frames with flex and grid layout, constraints and responsive resizing. | library | large | STU-A2-F3-P2, STU-A6-F1-P1 |  |
| STU-A13-F1-P2 | **Screen design workspace.** Canvas and panels for frames, layout, styles and device previews. | interface | large | STU-A13-F1-P1 |  |
| STU-A13-F1-P3 | **Study Penpot.** How Penpot is built and what could be shared or partnered on under its licence. | research | medium |  |  |
| STU-A13-F2-P1 | **Components and variants.** Main components, instances, variants and overrides. | library | large | STU-A13-F1-P1 |  |
| STU-A13-F2-P2 | **Design tokens.** Colour, type and spacing tokens in the open design tokens format. | data | medium | STU-A4-F2-P1 |  |
| STU-A13-F3-P1 | **Prototype engine.** Interactions, transitions, states and variables. | library | large | STU-A13-F1-P1 |  |
| STU-A13-F3-P2 | **Prototype player.** Plays prototypes in a browser and on phones, with comments on the shared comment model. | interface | medium | STU-A13-F3-P1, STU-A17-F3-P1 |  |
| STU-A13-F5-P1 | **Inspect view.** Measurements, styles, tokens and assets for developers. | interface | medium | STU-A13-F1-P1 |  |
| STU-A13-F5-P2 | **Code export.** Exports layouts and components to HTML and CSS. | library | medium | STU-A13-F1-P1 |  |
| STU-A13-F5-P3 | **Accessible HTML.** Code export and published sites use semantic HTML with landmarks, alt text, labels and a sensible focus order, checked before export. | library | medium | STU-A13-F5-P2, STU-A19-F2-P2 |  |
| STU-A13-F6-P1 | **Screen-design format check.** Which screen-design formats have published specifications we may use. | legal | small |  | [S06](STATUS.md) |
| STU-A13-F6-P2 | **Open format import.** Imports from open formats such as Penpot's, and SVG. | integration | medium | STU-A13-F6-P1, STU-A2-F6-P3 |  |
| STU-A13-F6-P3 | **Import through a design tool's public API.** Imports files from the main screen-design tool through its public API with the member's own token, only after its developer terms are checked. | integration | medium | STU-A13-F6-P1, STU-A13-F6-P2 |  |
| STU-A13-F7-P1 | **Site publisher.** Publishes screens as a static site on co-op hosting or anywhere else. | service | medium | STU-A13-F5-P2, STU-A13-F5-P3, STU-A20-F2-P5 |  |
| STU-A13-F7-P2 | **Code editor.** An editor for HTML, CSS and JavaScript with live preview. | interface | medium |  |  |
| STU-A13-F8-P1 | **Mood boards on the shared whiteboard.** Studio's mood-board tools, such as placed images, swatches and library links, on one whiteboard canvas built in engine-draw2d and shared with Spare Office's whiteboard. | interface | medium | engine-draw2d, STU-A4-F2-P1 |  |
| STU-A13-F9-P1 | **Design branches.** Branches, review and merge for screen designs on the kit's branches of the operation log. | library | medium | KIT-A15-F11-P1, STU-A13-F1-P1 |  |

## STU-A14 Spare Animate: 2D and character animation

Frame-by-frame and rigged 2D animation for cartoons, explainers, games and the web, and puppets that follow a webcam. Replaces Animate and Character Animator. OpenToonz (BSD-3-Clause) is the first project to learn from or build on.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A14-F1 Animate frame by frame | Draw each frame with onion skin, an exposure sheet and a timeline. | core | P1 Study OpenToonz; P2 Frame timeline and onion skin; P3 Drawing tools for animation; P4 Animation workspace |
| STU-A14-F2 Tween and rig characters | Tween shapes and motion between key poses, and rig characters with bones. | next | P1 Tweening; P2 Bones and mesh deformation |
| STU-A14-F3 Animate a puppet with a webcam | Make a puppet follow their face and body from a webcam and their voice from a microphone. | later | P1 Face and body tracking; P2 Puppet rigs; P3 Lip sync |
| STU-A14-F4 Add sound and export | Sync sound, then export video, GIF, SVG, Lottie and HTML canvas animations. | core | P1 Sound sync; P2 Animation export |
| STU-A14-F5 Make interactive animations and game art | Add buttons and simple scripts, and export sprite sheets for games. | later | P1 Interactive scripts; P2 Game export |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A14-F1-P1 | **Study OpenToonz.** What we can reuse from OpenToonz under its licence, and what to learn from. | research | medium |  |  |
| STU-A14-F1-P2 | **Frame timeline and onion skin.** A frame timeline, exposure sheet, onion skin and light table on the shared timeline model. | library | large | STU-A2-F3-P2, STU-A9-F2-P1 |  |
| STU-A14-F1-P3 | **Drawing tools for animation.** Vector and pixel brushes tuned for clean animation lines and fills. | library | medium | STU-A5-F7-P1, STU-A6-F1-P1 |  |
| STU-A14-F1-P4 | **Animation workspace.** Stage, timeline and library panels for animators. | interface | large | STU-A14-F1-P2 |  |
| STU-A14-F2-P1 | **Tweening.** Shape and motion tweens with easing. | library | medium | STU-A10-F1-P1 |  |
| STU-A14-F2-P2 | **Bones and mesh deformation.** Bone rigs that bend artwork through a mesh. | library | large | STU-A10-F1-P1 |  |
| STU-A14-F3-P1 | **Face and body tracking.** Face and body tracking from a webcam with a model that runs only on the device, so no face data leaves it. | library | large | STU-A16-F1-P2, STU-A20-F1-P5, STU-A16-F1-P7 |  |
| STU-A14-F3-P2 | **Puppet rigs.** Puppets with triggers, swaps and physics. | library | large | STU-A14-F2-P2 |  |
| STU-A14-F3-P3 | **Lip sync.** Mouth shapes chosen from speech. | library | medium | STU-A9-F3-P1 |  |
| STU-A14-F4-P1 | **Sound sync.** Audio on the timeline with scrubbing. | library | small | STU-A11-F1-P1 |  |
| STU-A14-F4-P2 | **Animation export.** Exports to video, GIF, SVG, Lottie and HTML canvas. | library | medium | STU-A9-F10-P1, STU-A10-F9-P2 |  |
| STU-A14-F5-P1 | **Interactive scripts.** Sandboxed scripts for buttons and simple interactions. | library | medium | kit-plugin, STU-A20-F5-P4 |  |
| STU-A14-F5-P2 | **Game export.** Sprite sheets and open runtime formats for game engines. | library | medium | STU-A14-F4-P2 |  |

## STU-A15 Spare 3D: materials, texturing and 3D scenes

Paint textures onto 3D models, build materials, stage product shots and render realistic images. Replaces Substance 3D and Dimension. Shares its renderer with Spare Construct; Blender's Cycles renderer (Apache-2.0) is the first to learn from.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A15-F1 Open and view 3D models | Open glTF, OBJ, STL, PLY and OpenUSD models and look around them. | core | P1 3D import and export; P2 3D viewport |
| STU-A15-F2 Paint textures on models | Paint colour, roughness, height and other channels straight onto 3D models. | core | P1 UV unwrapping; P2 3D texture painting; P3 Map baking; P4 Texture export presets |
| STU-A15-F3 Build materials | Make materials from node graphs and from photos of real surfaces. | next | P1 Material graph; P2 Materials from photos; P3 Open material library; P4 Smart materials |
| STU-A15-F4 Stage and render scenes | Place models, lights, cameras and backdrops and render realistic images. | core | P1 Scene staging; P2 Path-traced rendering |
| STU-A15-F5 Use 3D in other apps and in the room | Place 3D models in Photo, Layout and Motion, and view them in augmented reality on a phone. | later | P1 3D layers in other apps; P2 Augmented reality preview |
| STU-A15-F6 Model simple objects | Model or sculpt simple objects and props. | later | P1 Simple modelling and sculpting |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A15-F1-P1 | **3D import and export.** Reads and writes glTF, OBJ, STL, PLY and OpenUSD from their published specifications and open libraries. | library | large | engine-render |  |
| STU-A15-F1-P2 | **3D viewport.** A fast viewport with lighting, materials and camera controls, built in engine-render and shared with Spare Construct. | interface | medium | engine-render |  |
| STU-A15-F2-P1 | **UV unwrapping.** Automatic and guided UV unwrapping. | library | large | STU-A15-F1-P1 |  |
| STU-A15-F2-P2 | **3D texture painting.** Projection painting across texture sets with the shared brush engine. | library | large | STU-A5-F7-P1, STU-A15-F2-P1, engine-render |  |
| STU-A15-F2-P3 | **Map baking.** Bakes normal, ambient occlusion and curvature maps from detailed models. | library | large | engine-render |  |
| STU-A15-F2-P4 | **Texture export presets.** Exports texture sets with channel packing and naming presets for common game engines and glTF. | library | small | STU-A15-F2-P2 |  |
| STU-A15-F3-P1 | **Material graph.** Node-based procedural materials saved as MaterialX. | library | large | STU-A2-F8-P2 |  |
| STU-A15-F3-P2 | **Materials from photos.** Makes tileable materials from photos of real surfaces. | library | medium | STU-A2-F8-P2 |  |
| STU-A15-F3-P3 | **Open material library.** A library of materials under open licences, with credit kept. | data | medium | STU-A4-F2-P1 |  |
| STU-A15-F3-P4 | **Smart materials.** Materials that place wear, dirt and edge highlights from a model's baked curvature and occlusion maps, and update when the model changes. | library | medium | STU-A15-F3-P1, STU-A15-F2-P3 |  |
| STU-A15-F4-P1 | **Scene staging.** Place models, cameras, lights and backdrops, with ground snapping and simple presets. | interface | medium | STU-A15-F1-P2 |  |
| STU-A15-F4-P2 | **Path-traced rendering.** Realistic path-traced rendering with an open denoiser, built in engine-render and shared with Spare Construct. | library | large | engine-render |  |
| STU-A15-F5-P1 | **3D layers in other apps.** Linked 3D content rendered in place in other documents. | library | medium | STU-A2-F3-P4, STU-A15-F4-P2 |  |
| STU-A15-F5-P2 | **Augmented reality preview.** Shows a model in the room through a phone's camera, using the phone's own AR support. | integration | medium | STU-A15-F1-P1 | [K04](../kit/STATUS.md) |
| STU-A15-F6-P1 | **Simple modelling and sculpting.** Basic mesh modelling and sculpting tools. | library | large | STU-A15-F1-P1 |  |

## STU-A16 AI tools: optional, at cost, with consent

AI across the suite that a member can switch off. Members choose the model, AI runs on the device or at cost, every AI edit is labelled, and nothing is trained on members' work without their consent. Replaces Firefly and the generative features in every Adobe app.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A16-F1 Use smart tools that run on their own computer | Use AI selection, masking, text and speech tools that run on the device, free, private and offline. | core | P1 AI tools policy decision; P2 On-device model runtime; P3 Model licence checks; P4 AI off switch; P5 On-device model runtime study; P6 Model supply chain checks; P7 Fairness tests for people models; P8 Te reo Māori model rules |
| STU-A16-F2 Choose a cloud AI model and pay at cost | Pick a co-op-hosted or own-key model for heavier jobs and see the price before running anything. | next | P1 Model gateway; P2 AI settings and price preview; P3 Provider keys and data rules |
| STU-A16-F3 Fill, extend and remove with AI | Fill a selection from a written prompt, extend an image past its edges and remove objects or people. | next | P1 Generative fill and expand; P2 AI remove |
| STU-A16-F4 Make images, vectors, video and sound from a prompt | Generate images, patterns, vectors, video clips, sound effects and voices from words or reference pictures. | later | P1 Image and pattern generation; P2 Vector generation; P3 Video generation and clip extension; P4 Sound, music and voice |
| STU-A16-F5 Improve images with AI | Upscale, sharpen, restore old photos and remove backgrounds in one step. | core | P1 Upscaling and restoration; P2 Background removal |
| STU-A16-F6 Ask the suite to do a job | Describe a job in words and let an assistant carry it out through the same commands as the interface. | later | P1 Studio assistant |
| STU-A16-F7 Know what AI made | See which parts of a file came from AI, and label them on export and when posting. | next | P1 AI labelling; P2 AI labelling law; P3 Durable AI marking |
| STU-A16-F8 Keep their work out of AI training | Choose whether their work may ever be used to train AI, with the answer no unless they change it, and tell outside AI companies not to train on what they export. | core | P1 Consent records; P2 No keeping or training by providers; P3 Do-not-train marks on exports |
| STU-A16-F9 Use AI safely | Use AI that refuses illegal content and intimate or deceptive images of real people. | next | P1 Generation safety checks |
| STU-A16-F10 Train a model on their own work | Train a private model on their own or their organisation's work for output that stays on brand. | later | P1 Custom models from own work |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A16-F1-P1 | **AI tools policy decision.** The decision note on models, cost and consent that an owner decides, the same piece as the kit's AI tools policy. | research | small |  | [S07](STATUS.md) |
| STU-A16-F1-P2 | **On-device model runtime.** Runs open models on the device with the GPU; it belongs in a new shared on-device model engine with speech to text, text recognition, translation and segmentation, used by Spare OS, Spare Social and Spare Office too. | library | large | STU-A2-F8-P2, STU-A16-F1-P5, STU-A16-F1-P6 |  |
| STU-A16-F1-P3 | **Model licence checks.** Each model's licence and training-data terms checked before it is offered. | legal | medium |  |  |
| STU-A16-F1-P4 | **AI off switch.** One setting that turns every AI tool off in every app and keeps it off. | interface | small | STU-A2-F1-P5 |  |
| STU-A16-F1-P5 | **On-device model runtime study.** Which runtime runs open models on the device across Windows, macOS, Linux and the browser, compared on speed, GPU support and licence. | research | medium |  |  |
| STU-A16-F1-P6 | **Model supply chain checks.** Signed model downloads, safe weight formats with no executable pickles, and a provenance record for each model the apps offer. | library | medium | STU-A16-F1-P3, KIT-A22-F2-P3 |  |
| STU-A16-F1-P7 | **Fairness tests for people models.** Tests selection, masking, face and skin models across skin tones and hair types before release, and holds back any that fail unevenly. | research | medium | STU-A16-F1-P3 |  |
| STU-A16-F1-P8 | **Te reo Māori model rules.** Rules for te reo Māori speech and translation models: Māori data sovereignty, model terms such as Te Hiku Media's Kaitiakitanga licence, and review by fluent speakers. | legal | medium | KIT-A18-F16-P1, KIT-A19-F10-P3, STU-A16-F1-P3 |  |
| STU-A16-F2-P1 | **Model gateway.** Studio's model choices on the kit's co-op AI at cost: sends requests to on-device, co-op-hosted or member-chosen providers, metered per member, with no charge for failed or blocked jobs. | service | medium | KIT-A17-F6-P2, STU-A1-F5-P1, STU-A1-F5-P7, STU-A16-F1-P1 |  |
| STU-A16-F2-P2 | **AI settings and price preview.** Settings to switch AI off, choose models and see the price before a paid job runs. | interface | small | STU-A16-F2-P1 |  |
| STU-A16-F2-P3 | **Provider keys and data rules.** Keeps keys for member-chosen AI providers in the system keychain and never syncs them in plain text, runs the overseas disclosure check before images go to an overseas provider, and sets how long prompts and outputs are kept. | service | medium | STU-A16-F2-P1, KIT-A18-F15-P1, KIT-A18-F6-P1 |  |
| STU-A16-F3-P1 | **Generative fill and expand.** Fills and extends images with the chosen model, as a layer that can be changed later. | integration | medium | STU-A16-F2-P1, STU-A2-F3-P3 |  |
| STU-A16-F3-P2 | **AI remove.** Removes objects and people with a fill model. | integration | small | STU-A16-F3-P1 |  |
| STU-A16-F4-P1 | **Image and pattern generation.** Makes images and patterns from words and reference pictures. | integration | medium | STU-A16-F2-P1 |  |
| STU-A16-F4-P2 | **Vector generation.** Makes editable vectors and icons from words. | integration | medium | STU-A16-F2-P1, STU-A6-F1-P1 |  |
| STU-A16-F4-P3 | **Video generation and clip extension.** Makes short clips and extends existing clips by a few seconds. | integration | medium | STU-A16-F2-P1, STU-A9-F2-P1 |  |
| STU-A16-F4-P4 | **Sound, music and voice.** Makes sound effects, music and voices, with a consent rule for copying any real person's voice. | integration | medium | STU-A16-F2-P1 |  |
| STU-A16-F5-P1 | **Upscaling and restoration.** On-device models to upscale, sharpen and restore photos. | library | medium | STU-A16-F1-P2 |  |
| STU-A16-F5-P2 | **Background removal.** One-click background removal built on subject selection. | library | small | STU-A5-F1-P3 |  |
| STU-A16-F6-P1 | **Studio assistant.** A chat panel that plans a job and runs commands only with the person's approval. | interface | large | kit-mcp, STU-A16-F2-P1, KIT-A17-F5-P1 |  |
| STU-A16-F7-P1 | **AI labelling.** Records AI operations in the file and adds labels and content credentials on export. | library | small | STU-A3-F8-P1 | [M09](../spare-social/STATUS.md) |
| STU-A16-F7-P2 | **AI labelling law.** What the EU AI Act and other laws require when AI-made content is marked and published. | legal | small |  |  |
| STU-A16-F7-P3 | **Durable AI marking.** A machine-readable mark on AI output that survives metadata stripping, such as a watermark, if EU AI Act Article 50 covers co-op-hosted models; whether its dates have moved is unverified. | library | medium | STU-A16-F7-P1, STU-A16-F7-P2 |  |
| STU-A16-F8-P1 | **Consent records.** Each member's AI training choice kept once in the kit's accounts service as a consent register that every app and AI service checks, since Spare Social, Spare Office and Spare Construct hold members' work too. | service | small | svc-accounts, KIT-A1-F6-P2, STU-A16-F1-P1 | [S07](STATUS.md) |
| STU-A16-F8-P2 | **No keeping or training by providers.** Contracts and settings so co-op-hosted and partner providers neither keep nor train on members' data. | legal | small |  |  |
| STU-A16-F8-P3 | **Do-not-train marks on exports.** Marks exports 'do not train' with a C2PA training and mining assertion and a text-and-data-mining reservation, so outside AI companies are told. | library | small | STU-A3-F8-P1, STU-A16-F8-P1 |  |
| STU-A16-F9-P1 | **Generation safety checks.** Blocks illegal and abusive prompts and outputs before anything is shown or saved. | service | medium | svc-trust, STU-A16-F2-P1 | [M07](../spare-social/STATUS.md) |
| STU-A16-F10-P1 | **Custom models from own work.** Trains a style or brand model only on a member's or an organisation's own work, with their consent, kept private to them. | integration | large | STU-A16-F2-P1, STU-A16-F8-P1, STU-A16-F1-P3 |  |

## STU-A17 Sharing, review and teamwork

Keep files in co-op storage at cost, share links, collect feedback pinned to the exact spot, page, frame or text, approve versions, work together live and show a portfolio. Replaces Creative Cloud storage and cloud documents, Frame.io review, Behance and Adobe Portfolio; hiring from a portfolio happens in Spare Work.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A17-F1 Store and sync files in the cloud | Keep files and their history in co-op storage at cost and open them on any device. | next | P1 Studio files in co-op storage; P3 Studio sync client; P4 Studio files on member servers; P5 Encryption and private projects; P6 Staff access to private files |
| STU-A17-F2 Share a link | Share a view, comment or edit link with an expiry date, password and download control. | next | P1 Studio share links; P2 Web viewer; P3 Hidden data removed from shared files |
| STU-A17-F3 Get feedback and approval | Collect comments pinned to a point, a page or a video frame, and approve or reject each version. | next | P1 One comment model; P2 Review and approval; P3 Review alerts |
| STU-A17-F4 Edit together live | Edit the same file at the same time as others in any app, including screen designs, seeing their cursors and changes. | next | P1 Live co-editing |
| STU-A17-F5 Show a portfolio | Publish a portfolio of projects, share work to Spare Social and let people hire them through Spare Work. | later | P1 Portfolio pages; P2 Post to Spare Social; P3 Hire me link |
| STU-A17-F6 Send large files | Send big files and print packages to printers and clients. | next | P1 Large file transfer |
| STU-A17-F7 Review footage from the set | Get camera footage into a project as it is shot, protect review links with watermarks, see who watched and read comments on the timeline. | later | P1 Camera-to-cloud upload; P2 Watermarks on review links; P3 View logs; P4 Review comments on the timeline |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A17-F1-P1 | **Studio files in co-op storage.** Studio files and their history in the kit's file service and storage at cost, kept in New Zealand first and in other regions where the law asks. | service | medium | KIT-A15-F7-P1, KIT-A15-F7-P2, kit-store |  |
| STU-A17-F1-P3 | **Studio sync client.** Background sync of Studio files and history on the kit's sync engine, carrying on offline. | library | small | KIT-A15-F4-P2, STU-A17-F1-P1 |  |
| STU-A17-F1-P4 | **Studio files on member servers.** Studio file storage in the kit's self-host package, so a member group's own server can hold Studio files. | service | small | KIT-A22-F7-P1, STU-A17-F1-P1 |  |
| STU-A17-F1-P5 | **Encryption and private projects.** Encrypted storage and transfer on the kit's encryption and keys, with an end-to-end option for private projects that states what it switches off: the web viewer, server AI, known-content matching and recovery. | service | small | KIT-A15-F12-P1, KIT-A22-F5-P5, STU-A17-F1-P1 |  |
| STU-A17-F1-P6 | **Staff access to private files.** A published policy that co-op staff open private files only in a break-glass case, with a stated reason and a log entry the member can see. | legal | small | KIT-A22-F5-P1 |  |
| STU-A17-F2-P1 | **Studio share links.** View, comment and edit links for Studio files on the kit's sharing, with expiry, passwords and download control. | service | medium | KIT-A15-F8-P1, STU-A17-F1-P1 |  |
| STU-A17-F2-P2 | **Web viewer.** Views images, PDFs, video, audio and layouts in a browser with no account or install. | interface | large | engine-render, engine-pdf, engine-media |  |
| STU-A17-F2-P3 | **Hidden data removed from shared files.** Strips location and other hidden metadata by default from files shared by link, web galleries and portfolios, and scans shared files for malware, on the kit's upload checks and metadata stripping. | library | small | KIT-A15-F7-P1, KIT-A15-F7-P3, STU-A17-F2-P1 |  |
| STU-A17-F3-P1 | **One comment model.** Comments anchored to points, regions, pages, video frames and text ranges, with replies and mentions, that map to and from PDF annotations; it belongs in the kit beside messaging, since Spare Office and Spare Construct markups need the same. | service | medium | svc-messaging, STU-A17-F2-P1, STU-A12-F2-P1 |  |
| STU-A17-F3-P2 | **Review and approval.** Version stacks, reviewers, due dates and an approval status for each version. | service | medium | STU-A17-F3-P1, STU-A2-F9-P1 |  |
| STU-A17-F3-P3 | **Review alerts.** Comment, mention and approval alerts on the kit's notification router and settings. | integration | small | KIT-A8-F3-P2, KIT-A8-F5-P1 |  |
| STU-A17-F4-P1 | **Live co-editing.** Co-editing of canvases, pages and timelines on the kit's live updates and presence, with follow mode. | service | large | KIT-A15-F5-P1, KIT-A15-F5-P2, KIT-A15-F4-P2 | [A05](../spare-construct/STATUS.md) |
| STU-A17-F5-P1 | **Portfolio pages.** A portfolio site built from chosen projects, on co-op hosting or a member's own domain. | interface | medium | STU-A17-F2-P1, STU-A17-F2-P3, STU-A20-F2-P5 |  |
| STU-A17-F5-P2 | **Post to Spare Social.** Posts work with its content credentials and human or AI label. | integration | small | STU-A3-F8-P1, STU-A16-F7-P1 | [M09](../spare-social/STATUS.md) |
| STU-A17-F5-P3 | **Hire me link.** A 'hire me' link from a portfolio to the member's Spare Work profile, so hiring happens in Spare Work and Studio builds none of it. | integration | small | STU-A17-F5-P1 |  |
| STU-A17-F6-P1 | **Large file transfer.** Sends large files and folders with an expiring link, on the kit's file service. | service | small | KIT-A15-F7-P1, STU-A17-F2-P1 |  |
| STU-A17-F7-P1 | **Camera-to-cloud upload.** Uploads proxies from cameras and recorders that support it straight into a project, through their makers' published interfaces. | integration | large | STU-A17-F1-P1, STU-A9-F1-P4 |  |
| STU-A17-F7-P2 | **Watermarks on review links.** Visible watermarks with the viewer's name on review links, and forensic watermarks once their patent risk is checked. | library | medium | STU-A17-F2-P1, STU-A5-F3-P2 |  |
| STU-A17-F7-P3 | **View logs.** Shows the owner who opened a review link and when. | service | small | STU-A17-F2-P1 |  |
| STU-A17-F7-P4 | **Review comments on the timeline.** Shows review comments on video frames as markers on Spare Video's timeline, with replies synced back. | library | small | STU-A17-F3-P1, STU-A9-F2-P1 |  |

## STU-A18 Automation, plugins and AI agents

Every change is an operation, so scripts, plugins and AI agents can do anything a person can, through one command API, safely and in the open. Replaces each Adobe app's own scripting, plugin systems and automation.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A18-F1 Script any app | Write scripts in common languages that drive any app. | next | P1 Studio command schemas; P2 Studio API reference; P3 Script console |
| STU-A18-F2 Run jobs without opening an app | Convert, export and process files from the command line or on a server. | next | P1 Command-line tools; P2 Server jobs; P3 Server job safety |
| STU-A18-F3 Add plugins | Install plugins that add tools, filters, panels and file formats, each showing what it may access, and native media plugins in the desktop apps. | next | P2 Studio plugin points; P3 Plugin panels; P4 Studio plugins in the directory; P5 Filter plugins on our own API; P6 Open effect plugins; P7 Native plugin host; P8 Native plugin exception |
| STU-A18-F4 Let AI agents use the suite | Connect an AI agent through MCP and let it work on files with their approval. | next | P1 Studio on the MCP server; P2 Agent permissions; P3 Agent previews and long jobs; P4 Local and remote MCP links |
| STU-A18-F5 Connect other services | Link cloud drives, publishing services and other tools. | later | P1 Studio connectors; P2 Events and webhooks |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A18-F1-P1 | **Studio command schemas.** Every Studio app's commands and queries described once on the kit's command schemas, so the kit's command API, bindings, plugins and AI all use them. | library | large | KIT-A15-F1-P3, KIT-A16-F1-P1, kit-ops |  |
| STU-A18-F1-P2 | **Studio API reference.** Studio's command reference and worked examples in the kit's developer docs. | data | small | KIT-A16-F3-P2, STU-A18-F1-P1 |  |
| STU-A18-F1-P3 | **Script console.** A console inside each app to run and debug scripts. | interface | small | STU-A18-F1-P1 |  |
| STU-A18-F2-P1 | **Command-line tools.** Headless tools for converting, exporting and batch jobs, for every app. | library | medium | STU-A18-F1-P1 |  |
| STU-A18-F2-P2 | **Server jobs.** Runs headless jobs on co-op or member servers, charged at cost. | service | medium | STU-A18-F2-P1, STU-A1-F5-P1, STU-A18-F2-P3 |  |
| STU-A18-F2-P3 | **Server job safety.** Runs headless jobs in a sandbox with time, processor and storage quotas, and checks for crypto mining and spam, so members never pay for misuse. | service | medium | KIT-A22-F5-P4, STU-A18-F2-P1 |  |
| STU-A18-F3-P2 | **Studio plugin points.** Studio's plugin points, such as tools, filters, panels and file formats, on the kit's WebAssembly plugin host. | library | medium | KIT-A16-F5-P2, KIT-A16-F5-P1 |  |
| STU-A18-F3-P3 | **Plugin panels.** Lets plugins add panels built with HTML and TypeScript. | interface | medium | STU-A18-F3-P2 |  |
| STU-A18-F3-P4 | **Studio plugins in the directory.** Studio plugins listed in the kit's plugin directory, reviewed and signed before they are listed. | service | small | KIT-A16-F6-P1, KIT-A16-F6-P2, STU-A18-F3-P2 |  |
| STU-A18-F3-P5 | **Filter plugins on our own API.** Filters from other makers written to the studio's own plugin API, run as sandboxed plugins in Spare Photo; Photoshop-format plugins are not hosted unless an owner clears it after legal advice. | integration | medium | STU-A18-F3-P2, STU-A2-F7-P1 |  |
| STU-A18-F3-P6 | **Open effect plugins.** Spare Video and Spare Motion host effect plugins written to the open OpenFX standard in the native plugin host, on desktop only. | integration | medium | STU-A18-F3-P7 |  |
| STU-A18-F3-P7 | **Native plugin host.** Runs native OpenFX, CLAP, LV2, VST 3 and Audio Unit plugins each in its own process, signed and with stated permissions; desktop only, never in the browser build. | integration | large | STU-A18-F3-P8, KIT-A16-F6-P2 |  |
| STU-A18-F3-P8 | **Native plugin exception.** The written exception to the kit's WebAssembly-only plugin rule for native media plugins: why it is needed, the process isolation and signing it requires, and that it applies on desktop only. | research | small | KIT-A16-F5-P1 |  |
| STU-A18-F4-P1 | **Studio on the MCP server.** Every Studio app's commands offered to AI agents through the kit's MCP server, built from the Studio command schemas. | integration | small | KIT-A17-F1-P1, STU-A18-F1-P1 |  |
| STU-A18-F4-P2 | **Agent permissions.** Studio's risky and costly steps, such as deleting, publishing and paid AI, marked for the kit's approval steps and agent limits. | library | small | KIT-A17-F2-P1, KIT-A17-F4-P1, STU-A18-F4-P1 |  |
| STU-A18-F4-P3 | **Agent previews and long jobs.** Lets an agent render a preview of a canvas, page, frame or waveform to check its work, follow long jobs with progress and cancel them, and move large files in and out. | library | medium | STU-A18-F4-P1, STU-A2-F11-P1, KIT-A15-F7-P1 |  |
| STU-A18-F4-P4 | **Local and remote MCP links.** A local link to a running desktop app with origin checks, DNS-rebinding protection and a limit on which documents the agent can reach, and a remote MCP to cloud files using the kit's OAuth. | integration | medium | STU-A18-F4-P1, KIT-A17-F1-P3 |  |
| STU-A18-F5-P1 | **Studio connectors.** Links to storage and publishing services on the kit's common connectors, through their public APIs and terms. | integration | medium | KIT-A16-F8-P1 |  |
| STU-A18-F5-P2 | **Events and webhooks.** Tells other systems when Studio files change or versions are approved, on the kit's signed webhooks. | service | small | KIT-A16-F2-P1, kit-ops |  |

## STU-A19 Accessibility, languages, devices and offline

Every app works for everyone, in their own language, on the computer or tablet they already have, with or without the internet. New Zealand first, international by design.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A19-F1 Use the apps with assistive technology | Use screen readers, the keyboard alone, high contrast, larger interface text, reduced motion, and a click or number for every drag. | core | P1 Accessibility tree; P2 Full keyboard use; P3 High contrast, scaling and reduced motion; P4 Describing the canvas; P5 Testing with disabled people; P6 Alternatives to dragging; P7 Accessibility conformance reports |
| STU-A19-F2 Make accessible work | Check and fix the accessibility of what they make in every app, including audio description and broadcast captions. | next | P1 Colour-blindness proofing; P2 Alt text and reading order; P3 Accessibility checker for designs |
| STU-A19-F3 Use the apps in their language | Use the apps in te reo Māori, English and many other languages, including right-to-left languages. | core | P1 Studio strings for translation; P2 Right-to-left interface; P3 Studio in member translation; P4 Paper sizes and units per country; P5 Help in every language |
| STU-A19-F4 Work fully offline | Use every app with no internet and sync when they are back online. | core | P1 Offline first; P2 Offline AI |
| STU-A19-F5 Use the computer they have | Run on Windows, macOS, Linux and in a browser, from older laptops to workstations. | core | P1 Platform layer; P2 Low-spec mode; P3 Ready for Spare OS; P4 Clipboard and file previews |
| STU-A19-F6 Use pens and touch | Draw with pressure and tilt and use touch gestures on tablets and pen displays. | core | P1 Pen and touch input; P2 Tablet apps; P3 Phone and tablet plan |
| STU-A19-F7 Use hardware control surfaces | Work with colour panels, MIDI controllers and button decks in Video, Sound and Raw. | later | P1 Control surfaces |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A19-F1-P1 | **Accessibility tree.** Exposes every control to screen readers through the AccessKit library on every platform. | library | medium | STU-A2-F1-P1 |  |
| STU-A19-F1-P2 | **Full keyboard use.** Every command and panel reachable by keyboard, with visible focus. | interface | medium | STU-A2-F1-P4 |  |
| STU-A19-F1-P3 | **High contrast, scaling and reduced motion.** High-contrast canvas themes, interface scaling and reduced motion, following the kit's display settings. | interface | small | STU-A2-F1-P1, KIT-A19-F3-P1 |  |
| STU-A19-F1-P4 | **Describing the canvas.** Announces layers, objects, selections and timeline positions to screen-reader users through one canvas mirror in the kit's shared interface components, shared with Spare Office's canvas mirror. | library | medium | STU-A19-F1-P1, STU-A2-F3-P2, KIT-A20-F1-P1 |  |
| STU-A19-F1-P5 | **Testing with disabled people.** Testing with disabled creative workers before each app's first release, on the kit's accessibility testing. | research | medium | KIT-A19-F2-P2 |  |
| STU-A19-F1-P6 | **Alternatives to dragging.** A click or number-entry way to do every drag on canvases, timelines and curve editors, and minimum target sizes, to WCAG 2.2 criteria 2.5.7 and 2.5.8. | interface | medium | STU-A2-F1-P3, STU-A2-F1-P1 |  |
| STU-A19-F1-P7 | **Accessibility conformance reports.** A published accessibility conformance report for each app against WCAG 2.2 and EN 301 549, updated each release, for schools and government buyers. | data | small | STU-A19-F1-P5 |  |
| STU-A19-F2-P1 | **Colour-blindness proofing.** Shows how a design looks with each common colour vision difference. | library | small | STU-A3-F3-P2 |  |
| STU-A19-F2-P2 | **Alt text and reading order.** Alt text and reading order kept with images and frames across Layout, PDF and Screens. | library | medium | STU-A12-F11-P1, STU-A2-F3-P2 |  |
| STU-A19-F2-P3 | **Accessibility checker for designs.** Checks contrast, text size, missing alt text and caption tracks before export. | library | medium | STU-A3-F4-P3, STU-A9-F4-P1, STU-A9-F4-P4 |  |
| STU-A19-F3-P1 | **Studio strings for translation.** Studio's interface strings on the kit's translation framework. | library | small | KIT-A19-F1-P1 |  |
| STU-A19-F3-P2 | **Right-to-left interface.** Studio's panels and canvases mirrored on the kit's right-to-left layout. | interface | small | KIT-A19-F7-P1 |  |
| STU-A19-F3-P3 | **Studio in member translation.** Studio's strings in the kit's member translation workflow, te reo Māori first. | service | small | KIT-A19-F1-P3, STU-A19-F3-P1 |  |
| STU-A19-F3-P4 | **Paper sizes and units per country.** Studio's units, paper sizes and print defaults per country, on the kit's local formats. | data | small | KIT-A19-F1-P2 |  |
| STU-A19-F3-P5 | **Help in every language.** Translates manuals, the learning centre, templates and tutorial video captions, te reo Māori first, with New Zealand Sign Language for key steps. | data | large | KIT-A21-F1-P1, STU-A2-F10-P2, STU-A1-F3-P4, STU-A19-F3-P3 |  |
| STU-A19-F4-P1 | **Offline first.** All editing and saving happen on the device, and sync, when used, waits for a connection; in the browser build, the browser storage rules apply. | library | medium | kit-sync, STU-A2-F5-P5, STU-A1-F9-P2 |  |
| STU-A19-F4-P2 | **Offline AI.** On-device models downloaded once and used with no connection. | library | small | STU-A16-F1-P2, STU-A4-F7-P7 |  |
| STU-A19-F5-P1 | **Platform layer.** Windows, macOS, Linux and browser builds with native menus, file dialogs and drag and drop. | library | large | kit-core |  |
| STU-A19-F5-P2 | **Low-spec mode.** Lighter previews and CPU fallbacks so older machines stay usable. | library | medium | STU-A2-F8-P2 |  |
| STU-A19-F5-P3 | **Ready for Spare OS.** Packaged and tested for Spare OS. | integration | small | STU-A1-F1-P1 |  |
| STU-A19-F5-P4 | **Clipboard and file previews.** Copies SVG, PDF and PNG to the clipboard for other apps, and installs thumbnail and preview handlers for the open Studio format on Windows and macOS. | integration | medium | STU-A19-F5-P1, STU-A2-F5-P3 |  |
| STU-A19-F6-P1 | **Pen and touch input.** Pressure, tilt, barrel rotation and gestures on every platform. | library | medium | STU-A19-F5-P1 |  |
| STU-A19-F6-P2 | **Tablet apps.** Tablet layouts for Photo, Paint, Draw, Raw and PDF. | interface | large | STU-A19-F6-P1, STU-A19-F6-P3 | [K04](../kit/STATUS.md) |
| STU-A19-F6-P3 | **Phone and tablet plan.** Whether Studio ships one phone and tablet app or several, and how app store billing rules, with fees of 15 to 30 percent, apply to at-cost charges and Spare Market purchases. | research | medium | KIT-A20-F8-P1 | [K04](../kit/STATUS.md) |
| STU-A19-F7-P1 | **Control surfaces.** Supports colour panels, MIDI controllers and button decks through MIDI, HID and makers' published interfaces, with mappings people can edit. | integration | medium | STU-A19-F5-P1, STU-A2-F1-P4 |  |

## STU-A20 Trust, safety, privacy and legal duties

Studio's own duties on top of the kit's: face data, client work in cloud files, member websites, active content in files, the LGPL question and proof that the suite is built legally. Privacy, reports, moderation, notices, consumer law, terms and security reviews come from the kit, and Studio only adds its own rules. Not legal advice; every legal part needs checking with a lawyer.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A20-F1 Keep their data private | Know what data is kept, where and why, and control it. | core | P1 Studio privacy duties; P2 Studio's privacy notice lines; P3 Opt-in diagnostics only; P5 Face data rules |
| STU-A20-F2 Report content and see what was removed | Report shared files, links, portfolios and member websites, and see every removal in the kit's public log. | next | P1 Report buttons on shared work; P5 Website hosting safety |
| STU-A20-F3 Handle copyright claims fairly | Send and answer copyright notices and counter-notices, with work restored when a claim is wrong. | next | P2 Copyright notices for shared work |
| STU-A20-F4 Use the apps under clear, fair terms | Read short, fair terms that leave their work theirs, and see every open-source licence and credit. | core | P1 Studio terms; P3 Open-source credits; P4 Studio's licence rules in every build; P5 LGPL decision; P6 Your work stays yours |
| STU-A20-F5 Keep the apps secure | Open untrusted files and the scripts they carry safely, and get security fixes fast. | core | P1 Sandboxed file readers; P4 Active content in a sandbox |
| STU-A20-F6 Protect children | Use the studio with age-appropriate settings, and use it at school safely. | next | P2 Safer settings for young members |
| STU-A20-F8 Trust that the suite is built legally and in the open | See where every part came from, under which licence and whom it credits. | core | P4 Where-it-came-from records |
| STU-A20-F9 Meet accessibility law | Know that e-books and reading software made with Studio meet accessibility law where they are sold. | next | P1 European Accessibility Act duties |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A20-F1-P1 | **Studio privacy duties.** What the Privacy Act 2020 and the GDPR add for Studio's own data, such as face groups, client work in cloud files and AI processing, on top of the kit's privacy duties. | legal | medium | KIT-A18-F5-P1, KIT-A18-F15-P1 |  |
| STU-A20-F1-P2 | **Studio's privacy notice lines.** Studio's lines in the kit's collection notices, shown before anything leaves the device. | data | small | KIT-A18-F5-P1, STU-A20-F1-P1 |  |
| STU-A20-F1-P3 | **Opt-in diagnostics only.** Studio's opt-in usage counts on the kit's private usage counts, with no tracking. | library | small | KIT-A22-F8-P1 |  |
| STU-A20-F1-P5 | **Face data rules.** Face grouping and face tracking run on the device by default; if face groups ever sync, the Biometric Processing Privacy Code 2025 and the GDPR's special-category rules apply. | legal | medium | STU-A20-F1-P1 |  |
| STU-A20-F2-P1 | **Report buttons on shared work.** Report buttons on Studio's shared files, links, portfolios and websites, using the kit's report forms. | interface | small | KIT-A10-F3-P2 |  |
| STU-A20-F2-P5 | **Website hosting safety.** Checks custom domains, issues TLS certificates, and finds and takes down phishing and malware pages on member websites and portfolios. | service | medium | KIT-A10-F3-P1, KIT-A15-F7-P1, KIT-A10-F9-P2 |  |
| STU-A20-F3-P2 | **Copyright notices for shared work.** Studio's shared files, links and portfolios on the kit's copyright notice workflow, with work restored when a claim is wrong. | service | small | KIT-A10-F21-P2, KIT-A10-F21-P1 |  |
| STU-A20-F4-P1 | **Studio terms.** Studio's own terms, such as cloud files, AI use and published sites, in the kit's plain terms. | legal | small | KIT-A18-F4-P2 |  |
| STU-A20-F4-P3 | **Open-source credits.** Lists every library and asset used, with its licence and source. | data | small |  |  |
| STU-A20-F4-P4 | **Studio's licence rules in every build.** Studio's licence rules, including the LGPL decision and WebAssembly builds, in the kit's dependency checks on every change. | service | small | KIT-A22-F5-P2, STU-A20-F4-P5 |  |
| STU-A20-F4-P5 | **LGPL decision.** Asks the co-op for one decision on LGPL code: allowed when dynamically linked, allowed only as a separate process, or study only, including WebAssembly builds where linking is static; Studio's FFmpeg build and raw decoding wait on it. | legal | small | KIT-A16-F4-P1 |  |
| STU-A20-F4-P6 | **Your work stays yours.** Terms that take no licence to members' work beyond running the service, and say plainly who owns AI output. | legal | small | KIT-A18-F4-P2, STU-A20-F4-P1 |  |
| STU-A20-F5-P1 | **Sandboxed file readers.** File readers for untrusted files run sandboxed and are fuzz-tested. | library | medium | kit-plugin |  |
| STU-A20-F5-P4 | **Active content in a sandbox.** PDF JavaScript and form calculations, expressions, animation scripts, and SVG and Lottie scripts run sandboxed and only with the person's consent. | library | medium | kit-plugin, STU-A20-F5-P1 |  |
| STU-A20-F6-P2 | **Safer settings for young members.** Sharing, AI and buying limits switched on by default for young members. | service | small | KIT-A1-F1-P5, STU-A1-F7-P1, KIT-A2-F8-P1 |  |
| STU-A20-F8-P4 | **Where-it-came-from records.** Each pull request records the sources, licences and methods it used. | data | small | KIT-A14-F16-P1, KIT-A14-F16-P2 |  |
| STU-A20-F9-P1 | **European Accessibility Act duties.** What the European Accessibility Act asks of e-book output and reading software sold into the EU, and how Studio meets it. | legal | medium | STU-A19-F1-P7, STU-A7-F9-P1 |  |

## STU-A21 Plan, partners and names

The work that decides what is built first and with whom: the ArtCraft partnership, the order of apps and the names. Moved here from the core and legal areas, since none of it is a feature a person uses.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STU-A21-F1 Build with ArtCraft | See how ArtCraft is built, and know what a partnership would share and credit. | core | P1 ArtCraft's architecture compared with ours; P2 ArtCraft's repo layout and agent workflow; P3 Partner agreement with ArtCraft |
| STU-A21-F2 Know which app comes first | See the order the apps will be built in and why. | core | P1 First app decision |
| STU-A21-F3 Know the names are safe to use | Use app names that have been checked against existing trade marks. | next | P1 Name clearance |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STU-A21-F1-P1 | **ArtCraft's architecture compared with ours.** How ArtCraft's apps are built and what of them the studio core could share, from reading their source. | research | medium |  | [S01](STATUS.md) |
| STU-A21-F1-P2 | **ArtCraft's repo layout and agent workflow.** How ArtCraft builds from scratch in Rust with agents, and what to copy, shared with Spare Construct. | research | small |  | [A10](../spare-construct/STATUS.md) |
| STU-A21-F1-P3 | **Partner agreement with ArtCraft.** A written agreement if ArtCraft comes under the umbrella, covering credit, licences and names. | legal | medium | STU-A21-F1-P1 |  |
| STU-A21-F2-P1 | **First app decision.** The decision note an owner decides: Spare Photo, PDF and Draw ranked on demand, effort and what ArtCraft already covers. | research | small | STU-A21-F1-P1 | [S08](STATUS.md) |
| STU-A21-F3-P1 | **Name clearance.** Trade mark searches for Spare Studio and each app name in New Zealand, Australia, the EU and the US before release, with other names ready where one clashes. | legal | medium |  |  |

## Work-board pieces this map uses

| Piece | Parts that use it |
| --- | --- |
| [S01](STATUS.md) | STU-A21-F1-P1 |
| [S02](STATUS.md) | STU-A2-F3-P2, STU-A2-F3-P3, STU-A2-F4-P1, STU-A9-F2-P1 |
| [S03](STATUS.md) | STU-A3-F3-P1 |
| [S04](STATUS.md) | STU-A2-F6-P2, STU-A9-F1-P1, STU-A9-F1-P6 |
| [S05](STATUS.md) | STU-A3-F1-P1 |
| [S06](STATUS.md) | STU-A2-F7-P1, STU-A7-F10-P2, STU-A8-F3-P1, STU-A13-F6-P1 |
| [S07](STATUS.md) | STU-A16-F1-P1, STU-A16-F8-P1 |
| [S08](STATUS.md) | STU-A21-F2-P1 |
| [K02](../kit/STATUS.md) | STU-A1-F5-P2 |
| [K04](../kit/STATUS.md) | STU-A4-F8-P2, STU-A8-F11-P1, STU-A9-F12-P1, STU-A12-F8-P2, STU-A15-F5-P2, STU-A19-F6-P2, STU-A19-F6-P3 |
| [K06](../kit/STATUS.md) | STU-A1-F5-P4 |
| [A04](../spare-construct/STATUS.md) | STU-A2-F8-P1 |
| [A05](../spare-construct/STATUS.md) | STU-A17-F4-P1 |
| [A06](../spare-construct/STATUS.md) | STU-A2-F5-P1 |
| [A10](../spare-construct/STATUS.md) | STU-A21-F1-P2 |
| [M07](../spare-social/STATUS.md) | STU-A16-F9-P1 |
| [M09](../spare-social/STATUS.md) | STU-A16-F7-P1, STU-A17-F5-P2 |
| [OF02](../spare-office/STATUS.md) | STU-A12-F7-P4 |
| [OF03](../spare-office/STATUS.md) | STU-A7-F4-P1 |
| [OS06](../spare-os/STATUS.md) | STU-A1-F8-P1 |

## Questions for assessing this map and its demo

1. Does each app's area cover what you do every day in the Adobe app it replaces? What is missing from your own work?
2. Is "core" small enough that the first app (piece S08) could reach a usable release with the co-op's spare tokens?
3. Which parts could come from ArtCraft, Graphite, Penpot, OpenToonz or other permissively licensed projects instead of being built (piece S01)?
4. Is every Adobe file format handled only from a published specification, and which legal parts need a lawyer before work starts (piece S06)?
5. Is the studio core (document model, GPU pipeline, colour, text) truly shared by every app, or does any app quietly need its own?
6. Are the shared engines (render, 2D drawing, text, PDF, media) shared with Spare Construct, Spare Office and Spare Social rather than copied?
7. Does the at-cost model add up: what stays free, what do storage, AI and video processing cost a member each month, and does paying after use against a monthly cap keep the co-op from ever holding members' money?
8. Is the AI policy clear enough to build on: off by default, the member's choice of model, labelled, and never trained on members' work without consent (piece S07)?
9. Are accessibility, te reo Māori and offline work treated as core, not as extras for later?
10. Does anything in the demo or the map copy Adobe's interface designs, icons or names beyond saying what we replace?
11. Is the split with Spare Market right: Studio browses, installs and sends assets to sell, while listings, orders, tax, payouts and seller rules live in Spare Market?
12. Which large parts are the riskiest, and does each have a research or decision piece before building starts?
13. Are the hosting duties (illegal content, copyright notices, privacy) right-sized for a suite that is mostly local, with sharing on top?
14. Is the order of apps right: which three ship first, and which (3D, character animation) can wait?
15. Is anything here better left to an existing free tool than built by the co-op?
16. Does the demo show clearly what a person can do in each app on its first day?
17. The riskiest new research parts (browser build STU-A1-F1-P2, PSD fidelity STU-A2-F7-P6, EPS STU-A2-F7-P4, PDF content editing STU-A12-F3-P3, on-device model runtime STU-A16-F1-P5, audio latency STU-A11-F1-P4) have no work-board piece yet: which should become pieces first?
18. Several parts say they belong in the kit or a shared engine (per-member metering, the AI training consent register, organisation sign-on and audit log, the comment model, the office formats engine, the on-device model engine): should they move there now, before any program builds its own copy?
19. Is the native plugin host's exception to the WebAssembly-only plugin rule safe enough, and is desktop only acceptable for OpenFX, CLAP, LV2, VST 3 and Audio Unit plugins?
20. Is the co-op LGPL decision (STU-A20-F4-P5) made before any FFmpeg or raw decoding work starts?
