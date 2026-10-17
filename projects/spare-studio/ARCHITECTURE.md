# Spare Studio: architecture map

*Working name. Stage: Mapping. Every choice below is open until an owner accepts a decision note.*

Spare Studio is one creative suite that replaces Adobe's apps: photo, vector, layout, video, motion, raw photos, PDF, audio and more. Every app shares one studio core (layers, non-destructive edits, colour, text, GPU image pipeline) on top of the Spareware shared kit, so each app is a thin layer of its own tools. Partner first: [ArtCraft](https://github.com/storytold) already builds most of these from scratch in Rust under Apache-2.0, and we want to invite it under the Spareware umbrella before building our own.

## Why not just use what's free already

| Option | What it is | Why Spare Studio is still needed |
| --- | --- | --- |
| Affinity (Canva) | Photo, vector and layout in one app; free since 2025-10-30 | Proprietary, needs a Canva login, AI features need a paid Canva plan, no video, audio or PDF suite |
| ArtCraft | Apache-2.0 Rust apps for most Adobe products, created in 2026 | Very young (most apps alpha or in development); its backend service uses a non-open "fair source" licence and its AI studio sells credit plans. Best partner candidate |
| GIMP, Krita, Inkscape, Scribus, Kdenlive, darktable | Mature open tools | GPL, separate apps with no shared core, no shared AI access or co-op ownership |

## The apps

Our products use their own names. Adobe's names appear only to say what each replaces.

| Spare app | Replaces | Open work to build on or learn from (licence) |
| --- | --- | --- |
| Spare Photo | Photoshop | ArtCraft PhotoCraft (Apache-2.0), Graphite (Apache-2.0); GIMP and Krita to learn from only (GPL) |
| Spare Draw | Illustrator | ArtCraft VectorCraft (Apache-2.0), Graphite (Apache-2.0) |
| Spare Layout | InDesign, InCopy | ArtCraft DesignCraft (Apache-2.0); Scribus to learn from only (GPL) |
| Spare Video | Premiere, Media Encoder | ArtCraft FilmCraft (Apache-2.0); Kdenlive and Shotcut to learn from only (GPL) |
| Spare Motion | After Effects | ArtCraft EffectCraft (Apache-2.0); Natron to learn from only (GPL) |
| Spare Raw | Lightroom, Lightroom Classic | ArtCraft LightCraft (Apache-2.0); darktable and RawTherapee to learn from only (GPL) |
| Spare PDF | Acrobat | ArtCraft PdfCraft (Apache-2.0), pdf.js (Apache-2.0); MuPDF is AGPL, not usable |
| Spare Sound | Audition | ArtCraft SoundCraft (Apache-2.0); Audacity and Ardour to learn from only (GPL) |
| Spare Screens | XD (maintenance mode), Figma | Penpot (MPL-2.0), ArtCraft FigCraft (Apache-2.0) |
| Spare Animate | Animate, Character Animator | OpenToonz (BSD-3-Clause); Synfig to learn from only (GPL) |
| Spare Paint | Fresco | Shares Spare Photo's brush engine |
| Spare 3D | Substance 3D, Dimension | Shares Spare Arch's renderer; Blender's Cycles renderer is Apache-2.0 |
| Spare Library | Bridge | Shared asset browser in the studio core |

"Learn from only" means reading how a GPL project solves a problem and writing our own code in our own words, never copying it, per the legal rules.

## The studio core (shared by every app)

| Part | What it does | Shared with |
| --- | --- | --- |
| Document model | Layers, groups, masks, artboards and pages; every edit is a non-destructive operation, so undo, history, sync and AI use one path | Same operation log as the shared kit |
| GPU image pipeline | Filters, adjustments, blending, raw development and video effects as GPU compute (wgpu) | Spare Arch's renderer |
| Colour management | ICC profiles for print and photo, OpenColorIO (BSD-3) for video and film | All studio apps |
| Vector engine | Paths, strokes, 2D booleans, text on paths | Spare Arch's 2D drawing engine |
| Text engine | Font loading, shaping, line breaking, paragraph and page layout | Spare Layout, Spare Draw, Spare Arch annotation |
| Media engine | Decoding and encoding video and audio, playback, timelines | Spare Video, Spare Motion, Spare Sound |
| Asset library | Shared browser for files, fonts, colours, brushes and templates | All studio apps |
| PDF engine | Reading, editing and writing PDF to ISO 32000-2 | Spare PDF, print output, Spare Arch sheets |
| AI tools | Optional; members choose their model; run at cost; never trained on members' work without consent | All |

## File formats

Reading other companies' formats follows the README's legal rules: published specifications only, or an owner's clearance after legal advice.

| Format | Status | Plan |
| --- | --- | --- |
| PSD and PSB | Adobe publishes a specification | Read and write from the spec |
| PDF | ISO 32000-2, free via the PDF Association; Adobe's public patent licence covers ISO 32000-1 | Full read and write |
| DNG | Published specification and SDK | Read; patent terms to check |
| XMP | Partly ISO (paid), partly free; SDK is BSD-3 | Read and write metadata |
| IDML | Old public specification withdrawn; status unclear | Check before relying on it |
| AI (Illustrator) | No current specification; modern files embed a PDF | Open the embedded PDF only |
| INDD (InDesign) | Undocumented; ArtCraft's indd-utils reverse-engineers it | Not used unless an owner clears it after legal advice |
| Our own | Open, documented, layered | The default for every Spare Studio app |

## How it fits with ArtCraft

1. **Invite first.** Ask ArtCraft's author whether ArtCraft would come under the Spareware umbrella, as a partner project or by sharing one studio core.
2. **If yes:** shared core, shared roadmap, contributors' spare tokens go to the same code, and each side keeps credit.
3. **If no:** their Apache-2.0 app code can still be used with credit, but not their fair-source backend (artcraft-services), their trademarked name or logo, or anything that conflicts with our legal rules.

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Name | Spare Studio; another name | Spare Studio |
| ArtCraft | Partner under the umbrella; build on its Apache-2.0 code; build separately | Partner, if the author agrees |
| Video codecs | FFmpeg (LGPL build, linked separately); platform codecs; pure-Rust decoders | Research first (piece S04) |
| First app | Spare Photo, Spare PDF or Spare Draw | Research first (piece S08) |
| AI policy | Bring your own model; co-op-hosted at cost; both | Both, never trained on members' work without consent |

The work board is [STATUS.md](STATUS.md).
