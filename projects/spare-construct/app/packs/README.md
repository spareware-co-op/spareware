# Packs for the Construct page

The page is a small core and tools that plug into it through one registry (`../core/registry.js`; note [A96](../../notes/A96-tool-architecture.md)). A **pack** plugs in the same way: a country pack holds what belongs to one country (its wall, floor and roof types, its sheet borders, its rules and checks), a content pack a firm's or a supplier's own tools and types. No pack exists yet; this is how one is added. It is the Transport page's way ([`../../demo/transport/js/packs/README.md`](../../demo/transport/js/packs/README.md)), so a pack for one Spareware page reads like a pack for another.

1. **Make the folder**, `packs/<code>/`: a country's two-letter code in lower case (`au`), or a short name for a content pack.
2. **Open and close the pack.** Its first file calls `beginPack({ id: '<code>', name: '<Name>' })` and its last `endPack()`, both from `../core/registry.js`. Everything registered between them carries the pack's id.
3. **Register its parts** with the same calls the tools use (`register.tools`, `register.ribbons`, `register.palettes` and the rest; the kinds and their fields are at the top of `../core/registry.js`). What a pack registers takes part only on a project of that pack. To replace a core part for the pack's projects, register the same id: the pack's shows on its projects, the core's everywhere else.
4. **Load it** from `../tools/index.js`, after the tools, in the order its files need.
5. **State its sources.** Every number, rule and type names its source, edition and date, and its licence; rules and legal content need a reviewer who knows that country ([EXTENDING.md](../../../../EXTENDING.md), section 2.4).

Which pack a project is of is the core's to say (`usePack` in the registry). Until a project carries one, the first pack loaded is every project's, as on the Transport page.
