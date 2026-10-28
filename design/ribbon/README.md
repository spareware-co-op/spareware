# Spareware ribbon

One ribbon and desktop frame for every Spareware demo that works like desktop design software: Spare Construct, Spare Kit, Spare Office, Spare Studio and the others where a ribbon fits. Each demo brings its own tabs, panels and words; the ribbon, the frame and the icons are shared, so they look and behave the same everywhere.

It is a component built only from [tokens.css](../tokens.css) and [DESIGN.md](../DESIGN.md). It adds no new colours, fonts, sizes or shapes, so it does not change the shared look.

## What is in it

- `ribbon.js`: the ribbon (tabs of labelled panels; large, small, split and dropdown buttons; contextual tabs; a File menu; a quick access bar; a button to show tabs only), menus, instant tooltips and the icon family.
- `ribbon.css`: the ribbon, plus the frame parts around it: title bar, docked palettes that people can move between the left and right, tabs for open views or documents, the bar under a view, navigation buttons over a drawing, and the status bar.

## Using it

The page defines the tokens first (the usual "copied from design/tokens.css" block), then loads the two files. The path below works both in the repository and on the published site, where the site workflow copies this folder to `/design/ribbon/` (a path that climbs past the site root stops at the root):

```html
<link rel="stylesheet" href="../../../design/ribbon/ribbon.css">
<script src="../../../design/ribbon/ribbon.js"></script>
```

Then, in the page's own script:

```js
SpareIcons.install({myicon: '<path d="..."/>'});    // the family, plus any icons only this app needs
const rb = SpareRibbon.create(document.getElementById('ribbon'), {app: 'construct', label: 'Spare Construct tools', quick: document.getElementById('qat')});
rb.update({
  file: {label: 'File', items: [{label: 'Save', icon: 'save', act: 'save', kbd: 'Ctrl S'}], side: {head: 'Recent', items: []}},
  quick: [{label: 'Undo', icon: 'undo', act: 'undo', disabled: !canUndo}],
  tabs: [{id: 'home', label: 'Home', panels: [{label: 'Create', items: [
    {label: 'Wall', icon: 'wall', act: 'tool', a: 'wall', pressed: tool === 'wall'},
    {stack: [{label: 'Copy', icon: 'copy', act: 'copy'}, {label: 'Move', icon: 'move', act: 'move'}]},
    {label: 'Views', icon: 'plan', menu: [{label: 'Ground floor', act: 'view', a: 'ground'}]},
    {label: 'Print', icon: 'print', act: 'print', split: true, menu: [{label: 'This sheet', act: 'print', a: 'one'}]}]}]}],
  contextual: selection ? [{id: 'sel', key: 'sel:' + selection, group: 'Modify', label: 'Doors', panels: []}] : []
});
```

- Items carry `act` and `a`, which become `data-act` and `data-a`, so the page's usual click handler runs them. An item can instead have `run(button, event)`.
- `pressed` makes a toggle, `disabled` greys it out (say why in `tip`), `tip` and `kbd` fill the tooltip, `menu` adds a dropdown, and `split: true` makes the top run the item while the arrow opens the menu.
- A contextual tab comes to the front when its `key` is new (a new selection, a tool starting, a view opening) and the ribbon goes back to the previous tab when it goes away. `rb.select(id)` brings any tab to the front.
- `rb.openMenu(button, items)` opens the same dropdown menu under any button on the page, for example in the bar under a view.
- In narrow windows small buttons drop their words first, then whole panels fold into one button that opens them, starting from the right.
- Docked palettes: put `<aside class="rb-dock" data-side="left">` and `data-side="right"` either side of the view and the palettes (`<section class="rb-pal" data-pal="props">` with a `.rb-pal-h` title) inside, then `SpareRibbon.dock(workArea, {app, ribbon: rb, sides: {props: 'left', browser: 'right'}})`. People drag a palette's title to dock it on either side, above or below another, or use the move button on its title; the split between two palettes drags too, and the layout is remembered for that viewer. `dock.reset()` puts them back.
- Keyboard: Tab reaches every button, arrow keys move along the tabs and through menus, Down goes from a tab into its panels, and Esc closes menus. The focus ring is `--app-strong`.

## The icons

Spareware draws every icon itself. Never copy, trace or recreate another program's icons, even from memory; draw the thing itself, as it looks in a plan or as a plain object.

- A 24-unit grid with 2 units clear at each edge, shown at 16 px (small) and 32 px (large).
- Lines only, in `currentColor`, with round ends and joins; rectangles have 1-unit corners.
- At most one accent area per icon (class `f`, or `fo` with no outline). Large icons fill it with the app colour; small icons leave it empty, so small buttons stay calm.
- Class `fp` fills with the panel colour, to hide lines behind a shape.
- An app adds icons it alone needs with `SpareIcons.install({...})`, drawn to the same rules.

## Where the layout comes from

The structure follows the ribbon layout people already know from desktop design software (File menu and quick access bar at the top, tabs of labelled panels, a contextual Modify tab, docked palettes, view tabs, a bar of view controls, a status bar), which the co-op's legal rules allow ([AGENTS.md](../../AGENTS.md), section 5). The look, icons and words are our own.
