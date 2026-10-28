# Country packs for Transport

Transport is split in two. The **core** is the model and the tools that work the same in any country: the network, trip generation, assignment, junctions, calibration, the simulation and the map. A **country pack** holds what belongs to one country: its official data feeds, its land-use sources and trip rates, its rules and checks, and the report its councils expect.

New Zealand's pack is the first, in [`nz/`](nz/). Spareware does not build other countries' packs. Someone who knows that country's data and rules does, using the same structure.

## What New Zealand's pack holds

| File | What it does for New Zealand |
| --- | --- |
| [`nz/pack.js`](nz/pack.js) | Opens the pack: its code (`nz`), its name and a one-line summary |
| [`nz/feeds.js`](nz/feeds.js) | Official data feeds: NZTA's speed limit register and traffic counts, Auckland Transport's signals and signs, regional public transport feeds, growth and hazard layers |
| [`nz/landuse-plus.js`](nz/landuse-plus.js) | Existing land use from Stats NZ's Census and employee counts, the schools directory and Auckland Unitary Plan zones, with trip rates from AT's ITA Guidelines |
| [`nz/report.js`](nz/report.js) | The Integrated Transport Assessment report, in the chapters of AT's ITA Guidelines, and its data pack |
| [`nz/compliance.js`](nz/compliance.js) | The regulatory checks: the planning regime, E27 trigger screening, NZTA approvals, statutory clocks and the regulatory checklist |
| [`nz/compliance-extra.js`](nz/compliance-extra.js) | Compliance+: precinct staging, conditions, Fast-track and plan change packages, speed limits, parking, policy alignment and the hand-over packs |

Inside Spare Construct, the pack's workspaces, commands and exports appear on a ribbon tab named after the country ("New Zealand").

## Add your country

1. **Make the folder.** Use your country's two-letter code in lower case, for example `js/packs/au/`.
2. **Open the pack.** Copy [`nz/pack.js`](nz/pack.js) to your folder and change it:

   ```js
   Registry.beginPack({ id: 'au', name: 'Australia', summary: "Australia's data feeds, rules and report." });
   ```

3. **Write your country's files.** Each one registers its parts through the same registry the core uses: `Registry.tabs`, `Registry.exports`, `Registry.menu`, `Registry.report`, `Registry.about`, `Registry.settings` and the rest (the kinds are listed at the top of [`../registry.js`](../registry.js)). New Zealand's files are worked examples.
   - Everything a pack registers takes part only on projects in that country, hooks included.
   - To replace a core part for your country, register the same id. Your version shows on your country's projects and the core one shows everywhere else.
4. **Load it.** In [`../../index.html`](../../index.html), add your files after New Zealand's, between your `pack.js` and `Registry.endPack()`:

   ```html
   <script src="js/packs/au/pack.js"></script>
   <script src="js/packs/au/feeds.js"></script>
   <script>Registry.endPack();</script>
   ```

5. **Choose it for a project.** A project's pack is its `settings.country`. A project without one uses the first pack loaded.
6. **State your sources.** Every number, rule and feed names its source, edition and date, and its licence, in an About-the-data row (`Registry.about`). A rule with no source does not merge. Rules and legal content need a reviewer who knows that country, and a lawyer's check before they are switched on ([EXTENDING.md](../../../../../../EXTENDING.md), section 2.4).

## What is still in the core

Some New Zealand values still sit in the core: the left-hand drive side, NZ time and holidays, the standards table (MBCM, IPM, TMDG), the project defaults (value of time, trip rates, road classes), the count sources and their matching rules, the geocoder's region and the Drury sample. The Rust engine the demo runs on is not split yet either. [CON-A22-F5-P1-transport-country-packs.md](../../../../notes/CON-A22-F5-P1-transport-country-packs.md) lists them all and the order to move them, so that a new country never needs a change to the core.
