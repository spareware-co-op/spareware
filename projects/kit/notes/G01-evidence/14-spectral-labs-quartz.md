# 14. Spectral Labs' Quartz: CAD built for AI agents

*Evidence for [G01](../G01-geometry-kernel.md). Read on 2026-10-22 from the company's public research post, [Quartz: CAD Software Built for Machine Intelligence](https://www.spectrallabs.ai/research/building-cad-for-machine-intelligence) (dated 2026-10-19). Everything below is the post's own claim unless marked inferred; no code, paper, data or benchmark method is public, and nothing has been independently checked.*

## What it is

- A commercial CAD product (web app with agent chat and a 3D viewer). No licence, repository, SDK or paper is published; engagement is through the company's contact page. Inferred: proprietary.
- Its core is an unnamed "proven commercial geometry kernel". The company wrote the layer above it: the data model, translation to and from other CAD programs, and building and checking many designs at once. So Quartz is not a kernel, and nothing in it can be ported.
- A part is units, an ordered list of parametric operations (extrudes, revolves, sweeps, lofts, patterns, fillets, chamfers, shells, holes, drafts, sheet metal, reference geometry), constrained 2D and 3D sketches, and face attributes; assemblies add components, instances and mates.

## Claims that bear on Spare Kit

- **Open-source kernels return wrong geometry while reporting success.** The post's examples: a crane hook from three guided lofts, and a bin whose shell is as thick as its fillets, which open-source CAD built wrongly or could not build. This is the failure principle 4 forbids (never an invalid result; every result passes the checker or is an error).
- **Speed for agents.** Session start under 100 ms (SOLIDWORKS over 10 s); a part rebuilds in tens of milliseconds, of which the geometry engine is a few; a dimension change updates a couple-hundred-part assembly in under half a second; about 1,000 assemblies a minute across a few thousand cloud containers. SOLIDWORKS jobs began failing above 8 copies per machine.
- **Isolated parallel sessions.** Each worker has a private copy of the design and its own session; dependencies are explicit; if a part fails, its dependents stop and independent parts carry on.
- **Agents propose, engineers decide.** Changes return as candidates; two candidates that change the same thing are not merged; after an engineer accepts one, the others are revalidated against it. Humans review each change in the assembly and keep or reject it.
- **Feature trees across programs.** Designs move between Quartz, SOLIDWORKS and Siemens NX with feature trees intact (two examples, no corpus-wide rate). The mechanism (vendor APIs, neutral formats) is not stated.

## What Spare Kit takes

| What | Where it goes | How |
| --- | --- | --- |
| The two failure cases (crane hook from three guided lofts; a bin shelled as thick as its fillets) | The test corpus (chapter 28) and the lofts, shells and fillets pieces (chapter 13) | Rebuilt as our own fixtures from the plain description; no geometry copied |
| Speed targets for agent use: start well under 100 ms, a part rebuild in milliseconds, thousands of isolated sessions | Chapter 23's targets and the benchmark gates (geom-bench) | Compared with chapter 23 when its pieces are built |
| Isolated sessions, explicit dependencies, failure stops only dependents | Already the design: pure operations on immutable data (chapter 10), batch results per item (10.1), headless (principle 7) | Confirms the design; nothing to change |
| Candidates, no merge of conflicting candidates, revalidation after acceptance, engineer keeps or rejects each change | Spare Kit's command layer, with the kernel's previews and model diff (chapter 22) | Matches the owner's AI-mode idea; for the command-layer pieces, not the kernel |
| Feature trees across CAD programs | Programs' exchange pieces (chapter 24 for geometry; feature trees are program data) | Only through vendors' licensed APIs or published formats; never by reverse engineering (README legal rules) |

## Verdict

**STUDY** (claims only). Nothing to depend on or port: the product is closed and its kernel commercial. Its public claims confirm three things Spare Kit already does: honest errors rather than silent wrong geometry, pure operations that run in isolated parallel sessions, and agent proposals that an engineer reviews. Its two named failure cases become our test fixtures.
