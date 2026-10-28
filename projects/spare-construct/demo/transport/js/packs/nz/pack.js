'use strict';
/* Spare Construct · Transport · the New Zealand country pack. Everything in js/packs/nz/ is New Zealand's own: the
   official data feeds (feeds.js: NZTA speed limits and traffic counts, Auckland Transport signals and signs), the
   land-use sources and trip rates (landuse-plus.js: Stats NZ Census and employee counts, the schools directory,
   Auckland Unitary Plan zones, AT ITA Guidelines trip rates), the Integrated Transport Assessment report and data
   pack (report.js) and the regulatory checks (compliance.js and compliance-extra.js: E27 screening, NZTA approvals,
   statutory clocks and the hand-over packs).
   This file opens the pack; index.html loads the pack's files after it and closes it with Registry.endPack(), so
   everything they register belongs to New Zealand and shows only on New Zealand projects. To add your country, read
   js/packs/README.md: copy this file into js/packs/<code>/ and register your country's own feeds, checks and report.
   Parts of the core still hold New Zealand values; the list, and the order to move them, is in
   projects/spare-construct/notes/CON-A22-F5-P1-transport-country-packs.md. */
Registry.beginPack({
  id: 'nz',
  name: 'New Zealand',
  summary: "New Zealand's official data feeds, land-use sources and trip rates, regulatory checks and the Integrated Transport Assessment report.",
});
