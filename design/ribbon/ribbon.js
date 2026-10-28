/* Spareware ribbon: one ribbon, quick access bar, menus and icon family for every desktop-style Spareware demo.
   Read design/ribbon/README.md first. The layout follows the ribbon pattern people already know from desktop
   design software (tabs of labelled panels, large and small buttons, contextual tabs); the look, the icons and the
   words are Spareware's own, and every colour, size and space comes from design/tokens.css through ribbon.css.

   A page loads ribbon.css and this file, installs the icons, creates one ribbon and updates it with its tabs:
     SpareIcons.install();
     const rb = SpareRibbon.create(document.getElementById('ribbon'), {app:'construct', quick:document.getElementById('qat')});
     rb.update({file:{...}, quick:[...], tabs:[...], contextual:[...]});
   Buttons carry data-act and data-a like the rest of each demo, so the page's own click handler runs them;
   an item can instead carry run(), which the ribbon calls itself. */
(function (global) {
'use strict';
const esc = s => String(s == null ? '' : s).replace(/[&<>"']/g, c => ({'&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'}[c]));

/* ---------- the icon family ----------
   Drawn by Spareware on a 24-unit grid with 2 units clear at each edge, shown at 16 px (small) and 32 px (large).
   Lines are currentColor with round ends and joins; rectangles have 1-unit corners. Each icon may have one accent
   area: class "f" (filled and outlined) or "fo" (filled, no outline). Large icons fill it with the app colour,
   small icons leave it empty. Class "fp" fills with the panel colour to hide lines behind a shape. */
const ICONS = {
  // general
  search: '<circle cx="10.5" cy="10.5" r="6"/><path d="M15 15l5.5 5.5"/>',
  zoomin: '<circle class="f" cx="10.5" cy="10.5" r="6"/><path d="M15 15l5.5 5.5M8 10.5h5M10.5 8v5"/>',
  zoomout: '<circle class="f" cx="10.5" cy="10.5" r="6"/><path d="M15 15l5.5 5.5M8 10.5h5"/>',
  undo: '<path d="M9 13.5L4.5 9 9 4.5"/><path d="M4.5 9H15a5 5 0 0 1 0 10h-3"/>',
  redo: '<path d="M15 13.5L19.5 9 15 4.5"/><path d="M19.5 9H9a5 5 0 0 0 0 10h3"/>',
  check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
  x: '<path d="M6 6l12 12M18 6L6 18"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  minus: '<path d="M5 12h14"/>',
  fit: '<path d="M4 9V5a1 1 0 0 1 1-1h4M15 4h4a1 1 0 0 1 1 1v4M20 15v4a1 1 0 0 1-1 1h-4M9 20H5a1 1 0 0 1-1-1v-4"/><rect class="fo" x="8" y="8" width="8" height="8" rx="1"/>',
  chev: '<path d="M9 6l6 6-6 6"/>',
  chevd: '<path d="M6 9l6 6 6-6"/>',
  chevu: '<path d="M6 15l6-6 6 6"/>',
  start: '<path class="f" d="M5 10.5V19a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-8.5L12 5z"/><path d="M3 12l9-7.5 9 7.5M10 20v-5h4v5"/>',
  file: '<path class="f" d="M6 3h8l5 5v12a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z"/><path d="M14 3v5h5M8.5 13h7M8.5 16.5h7"/>',
  folder: '<path class="f" d="M3 6a1 1 0 0 1 1-1h5l2 2h9a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z"/>',
  open: '<path d="M3 18V6a1 1 0 0 1 1-1h5l2 2h7a1 1 0 0 1 1 1v2"/><path class="f" d="M3 19l3.2-8.3a1 1 0 0 1 .9-.7H21l-3.4 8.4a1 1 0 0 1-.9.6z"/>',
  save: '<path class="f" d="M5 4h11l3 3v12a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z"/><path d="M8 4v5h7V4M8 20v-6h8v6"/>',
  print: '<path d="M7 9V4h10v5"/><path class="f" d="M7 17H4a1 1 0 0 1-1-1v-6a1 1 0 0 1 1-1h16a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-3"/><rect class="fp" x="7" y="14" width="10" height="7" rx="1"/>',
  send: '<path class="f" d="M3 11l18-8-8 18-2-8z"/><path d="M11 13l10-10"/>',
  down: '<path d="M12 3v12M7 10l5 5 5-5"/><path class="f" d="M4 17v2a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-2"/>',
  up: '<path d="M12 15V3M7 8l5-5 5 5"/><path class="f" d="M4 17v2a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-2"/>',
  play: '<path class="f" d="M7 4.5v15a.5.5 0 0 0 .8.4l11.4-7.5a.5.5 0 0 0 0-.8L7.8 4.1a.5.5 0 0 0-.8.4z"/>',
  warn: '<path class="f" d="M12 3.5l9.5 16.5h-19z"/><path d="M12 10v4.5M12 17.5v.5"/>',
  info: '<circle class="f" cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.5"/>',
  help: '<circle class="f" cx="12" cy="12" r="9"/><path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17v.5"/>',
  gear: '<circle class="f" cx="12" cy="12" r="3.5"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>',
  book: '<path class="f" d="M4 4h6a2 2 0 0 1 2 2v14a2 2 0 0 0-2-2H4z"/><path d="M20 4h-6a2 2 0 0 0-2 2v14a2 2 0 0 1 2-2h6z"/>',
  user: '<circle class="f" cx="12" cy="8" r="4"/><path d="M4 21c1-4 4-6 8-6s7 2 8 6"/>',
  team: '<circle class="f" cx="9" cy="8" r="3.5"/><path d="M3 20c.8-3.5 3-5 6-5s5.2 1.5 6 5"/><circle cx="17" cy="9" r="2.5"/><path d="M16 14.5c2.6 0 4.3 1.4 5 4.5"/>',
  lock: '<rect class="f" x="5" y="11" width="14" height="10" rx="1"/><path d="M8 11V7a4 4 0 0 1 8 0v4M12 15v2"/>',
  cloud: '<path class="f" d="M7 18h10a4 4 0 0 0 .4-8 6 6 0 0 0-11.6 1.6A3.3 3.3 0 0 0 7 18z"/>',
  globe: '<circle class="f" cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
  flag: '<path class="f" d="M5 4h12l-2 4 2 4H5"/><path d="M5 21V4"/>',
  pin: '<path class="f" d="M12 21s-7-6-7-11a7 7 0 0 1 14 0c0 5-7 11-7 11z"/><circle cx="12" cy="10" r="2.5"/>',
  eye: '<path class="f" d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6S2 12 2 12z"/><circle class="fp" cx="12" cy="12" r="2.5"/>',
  eyeoff: '<path class="f" d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6S2 12 2 12z"/><path d="M4 20L20 4"/>',
  bot: '<rect class="f" x="4" y="8" width="16" height="12" rx="1"/><path d="M12 4v4M9 13v1M15 13v1M9 17h6"/>',
  term: '<rect class="f" x="3" y="5" width="18" height="14" rx="1"/><path d="M7 10l3 2.5L7 15M12 15h5"/>',
  screen: '<rect class="f" x="3" y="4" width="18" height="12" rx="1"/><path d="M8 20h8M12 16v4"/>',
  link: '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>',
  layers: '<path class="f" d="M12 3l9 5-9 5-9-5z"/><path d="M3 13l9 5 9-5"/>',
  timeline: '<path d="M3 7h3M10 7h4M18 7h3M3 17h7M14 17h7"/><circle class="f" cx="8" cy="7" r="2"/><circle class="f" cx="16" cy="7" r="2"/><circle class="f" cx="12" cy="17" r="2"/>',
  shield: '<path class="f" d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z"/><path d="M8.5 12l2.5 2.5 4.5-5"/>',
  compare: '<rect class="f" x="3" y="4" width="8" height="16" rx="1"/><rect x="13" y="4" width="8" height="16" rx="1"/><path d="M5.5 9h3M5.5 13h3M15.5 9h3M15.5 13h3"/>',
  clash: '<path class="fo" d="M12 7.4A5.5 5.5 0 0 1 12 16.6A5.5 5.5 0 0 1 12 7.4z"/><circle cx="9" cy="12" r="5.5"/><circle cx="15" cy="12" r="5.5"/>',
  issue: '<path class="f" d="M4 5a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-8l-4 4v-4H5a1 1 0 0 1-1-1z"/><path d="M12 7v4.5M12 13.5v.5"/>',
  energy: '<path class="f" d="M13 2.5L5 14h6l-1 7.5 8-11.5h-6z"/>',
  sun: '<circle class="f" cx="12" cy="12" r="4"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9L7 7M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1"/>',
  shadow: '<path class="fo" d="M11 17h7.5l3-4H14z"/><rect x="4" y="7" width="7" height="10" rx="1"/><path d="M2 17h20"/>',
  vstyle: '<path class="fo" d="M12 4a8 8 0 0 1 0 16z"/><circle cx="12" cy="12" r="8"/>',
  detail: '<rect class="f" x="4" y="4" width="16" height="16" rx="1"/><path d="M4 9.5h16M4 14.5h16M10 4v16"/>',
  crop: '<rect class="fo" x="7" y="7" width="10" height="10"/><path d="M7 3v13a1 1 0 0 0 1 1h13M3 7h13a1 1 0 0 1 1 1v13"/>',
  windows: '<rect class="f" x="8" y="3" width="13" height="11" rx="1"/><rect class="fp" x="3" y="9" width="13" height="11" rx="1"/><path d="M3 12.5h13"/>',
  closeviews: '<rect class="f" x="3" y="4" width="18" height="16" rx="1"/><path d="M3 8h18M9.5 11.5l5 5M14.5 11.5l-5 5"/>',
  ui: '<rect x="3" y="4" width="18" height="16" rx="1"/><path class="fo" d="M4 5h5v14H4z"/><path d="M9 4v16M12 9h6M12 13h6"/>',
  props: '<rect x="4" y="3" width="16" height="18" rx="1"/><path class="fo" d="M5 4h14v4H5z"/><path d="M4 8h16M7 12h4M13 12h4M7 16h4M13 16h4"/>',
  tree: '<path d="M5 4v13a1 1 0 0 0 1 1h4M5 8.5h5"/><rect class="f" x="10" y="6" width="10" height="5" rx="1"/><rect x="10" y="15.5" width="10" height="5" rx="1"/>',
  template: '<rect x="4" y="3" width="13" height="18" rx="1"/><path d="M7 7h7M7 10.5h4"/><path class="f" d="M13.5 17.5l6-6 2 2-6 6-3 1z"/>',
  measure: '<rect class="f" x="2.5" y="8" width="19" height="8" rx="1"/><path d="M6.5 8v3M10.5 8v4M14.5 8v3M18.5 8v4"/>',
  select: '<path class="f" d="M6 3l12.5 9.5-5.6 1.1 3.1 6.2-2.6 1.2-3-6.2L6 18.8z"/>',
  table: '<rect x="3" y="5" width="18" height="14" rx="1"/><path class="fo" d="M4 6h16v4H4z"/><path d="M3 10h18M3 14.5h18M9 5v14"/>',
  sheet: '<rect x="3" y="5" width="18" height="14" rx="1"/><path class="fo" d="M16.5 15.5h3.5v2.5h-3.5z"/><path d="M16 5v14M16 15h5M6 8.5h7v7H6z"/>',
  plan: '<path class="fo" d="M4 5h7v7H4z"/><rect x="3" y="4" width="18" height="16" rx="1"/><path d="M11 4v8H3M11 16v4M15 12h6"/>',
  section: '<path class="fo" d="M6 12h12v7H6z"/><path d="M4 11l8-6 8 6M6 9.5V19a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1V9.5"/><path d="M2 12h20" stroke-dasharray="2.5 2"/>',
  elev: '<path class="f" d="M5 11.5L12 6l7 5.5V20H5z"/><path d="M3 13l9-7.5 9 7.5M2 20h20M8 13.5h2.5v2.5H8zM13.5 20v-6h2.5v6"/>',
  cube: '<path class="fo" d="M4 7.5L12 3l8 4.5-8 4.5z"/><path d="M12 3l8 4.5v9L12 21l-8-4.5v-9zM4 7.5l8 4.5 8-4.5M12 12v9"/>',
  box: '<path class="fo" d="M4 8l8 4v8l-8-4z"/><path d="M4 8l8-4 8 4v8l-8 4-8-4z" stroke-dasharray="3 2"/><path d="M4 8l8 4 8-4M12 12v8"/>',
  grid: '<circle class="f" cx="12" cy="5.5" r="3"/><path d="M12 8.5V21" stroke-dasharray="4 2 1 2"/>',
  turnl: '<path d="M4 12a8 8 0 1 0 2.3-5.7"/><path d="M4 4v5h5"/>',
  turnr: '<path d="M20 12a8 8 0 1 1-2.3-5.7"/><path d="M20 4v5h-5"/>',
  // modify
  move: '<path d="M12 3v18M3 12h18M9 6l3-3 3 3M9 18l3 3 3-3M6 9l-3 3 3 3M18 9l3 3-3 3"/>',
  copy: '<path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3"/><rect class="f" x="8" y="8" width="12" height="12" rx="1"/>',
  rotate: '<path d="M20 12a8 8 0 1 1-2.3-5.7"/><path d="M20 4v5h-5"/><circle class="f" cx="12" cy="12" r="2"/>',
  mirror: '<path d="M12 3v18" stroke-dasharray="2 2"/><path class="f" d="M9 7L4 12l5 5z"/><path d="M15 7l5 5-5 5z"/>',
  array: '<rect class="f" x="3" y="9" width="5" height="6" rx="1"/><rect x="10" y="9" width="4" height="6" rx="1"/><rect x="16" y="9" width="5" height="6" rx="1"/>',
  align: '<path d="M4 3v18"/><rect class="f" x="8" y="6" width="12" height="4.5" rx="1"/><rect x="8" y="13.5" width="7" height="4.5" rx="1"/>',
  offset: '<path d="M4 7h16"/><path d="M4 17h16" stroke-dasharray="3 2"/><path d="M12 9v6M10 13l2 2 2-2"/>',
  trim: '<path d="M4 20L20 4M4 12h7"/><path d="M15 12h5" stroke-dasharray="2 2"/><circle class="f" cx="12" cy="12" r="1.8"/>',
  split: '<path d="M3 12h7M14 12h7M12 6v12"/><path class="fo" d="M10 10.5h4v3h-4z"/>',
  scale: '<rect class="fo" x="4" y="14" width="6" height="6"/><path d="M4 14v6h6M4 20L20 4M14 4h6v6"/>',
  del: '<path d="M4 7h16M9 7V4h6v3"/><path class="f" d="M6 7l1 13a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1l1-13"/><path d="M10 11v6M14 11v6"/>',
  group: '<rect class="f" x="3" y="3" width="8" height="8" rx="1"/><rect x="13" y="13" width="8" height="8" rx="1"/><path d="M11 7h6v6" stroke-dasharray="2 2"/>',
  // building and annotation
  wall: '<rect class="f" x="3" y="9" width="18" height="6" rx="1"/><path d="M7.5 9l-3 6M12.5 9l-3 6M17.5 9l-3 6M21 10.5L18.8 15"/>',
  door: '<path class="fo" d="M7 20V6a14 14 0 0 1 14 14z"/><path d="M3 20h4M17 20h4M7 20V6"/><path d="M7 6a14 14 0 0 1 14 14" stroke-dasharray="2 2"/>',
  window: '<rect class="f" x="3" y="9" width="18" height="6" rx="1"/><path d="M3 12h18M8 9v6M16 9v6"/>',
  room: '<rect class="f" x="4" y="4" width="16" height="16" rx="1" stroke-dasharray="3 2"/><path d="M8 11h8M9.5 14h5"/>',
  family: '<rect class="f" x="3" y="3" width="8" height="8" rx="1"/><circle cx="17" cy="7" r="4"/><path d="M7 13.5l4 7H3z"/><rect x="13" y="13" width="8" height="8" rx="1"/>',
  dim: '<path d="M3 7v10M21 7v10M3 12h18M5.5 10l-2.5 2 2.5 2M18.5 10l2.5 2-2.5 2"/>',
  tag: '<path class="f" d="M3 8h12l5 4-5 4H3z"/><path d="M7 12h5"/>',
  num: '<path d="M5 9h15M4 15h15M10 4L8 20M16 4l-2 16"/>',
  key: '<rect class="f" x="3" y="3" width="10" height="10" rx="1"/><path d="M13 13l7 7M6 8h4"/>',
  column: '<rect class="f" x="6" y="6" width="12" height="12" rx="1"/><path d="M6 6l12 12M18 6L6 18"/>',
  beam: '<rect class="f" x="3" y="10" width="18" height="4" rx="1"/><path d="M3 6.5v11M21 6.5v11"/>',
  lintel: '<rect class="f" x="4" y="5" width="16" height="4" rx="1"/><path d="M7 9v11M17 9v11M2 20h5M17 20h5"/>',
  bracing: '<rect x="4" y="4" width="16" height="16" rx="1"/><path class="fo" d="M5 5.5V19h13.5z"/><path d="M4 4l16 16"/>',
  foundation: '<rect class="f" x="3" y="7" width="18" height="4" rx="1"/><path d="M7 11v9M12 11v9M17 11v9M2 4h20"/>',
  pipe: '<path class="f" d="M3 8h9a6 6 0 0 1 6 6v7h-5v-7a1 1 0 0 0-1-1H3z"/>',
  duct: '<rect class="f" x="3" y="7" width="18" height="10" rx="1"/><path d="M7 12h9M13 9l3 3-3 3"/>',
  light: '<circle class="f" cx="12" cy="12" r="6.5"/><path d="M7.4 7.4l9.2 9.2M16.6 7.4l-9.2 9.2"/>',
  power: '<circle class="f" cx="12" cy="12" r="8.5"/><path d="M9.5 8.5v4.5M14.5 8.5v4.5M12 16v.5"/>',
  drain: '<path class="f" d="M12 3c3.5 4.5 6 8 6 11a6 6 0 0 1-12 0c0-3 2.5-6.5 6-11z"/>',
  spot: '<path class="f" d="M7 9h10l-5 7z"/><path d="M3 16h18M12 3v6"/>',
  pad: '<rect class="f" x="6" y="10" width="12" height="4" rx="1"/><path d="M2 19l4-5M22 19l-4-5M2 19h20"/>',
  storm: '<circle class="f" cx="6" cy="12" r="3"/><circle class="f" cx="18" cy="12" r="3"/><path d="M9 12h6"/>',
  alignment: '<path d="M3 19c6 0 8-14 18-14"/><path d="M8.2 15.4l1.9 1.6M12.4 10.2l2 1.3M16.8 6.6l1.4 1.8"/><circle class="f" cx="3.5" cy="19" r="1.8"/><circle class="f" cx="20.5" cy="5" r="1.8"/>',
  road: '<path class="f" d="M9 3h6l5 18H4z"/><path d="M12 5v2M12 10v3M12 16v3"/>',
  phase: '<rect x="3" y="3" width="11" height="11" rx="1" stroke-dasharray="2.5 2"/><rect class="f" x="10" y="10" width="11" height="11" rx="1"/>',
  option: '<circle class="f" cx="6" cy="6" r="2.5"/><circle cx="18" cy="6" r="2.5"/><circle cx="12" cy="19" r="2.5"/><path d="M6 8.5V10a3 3 0 0 0 3 3h6a3 3 0 0 0 3-3V8.5M12 13v3.5"/>',
  // documents, sheets and slides
  bold: '<path class="f" d="M7 4.5h5.5a3.5 3.5 0 0 1 0 7H7z"/><path d="M7 11.5h6.5a3.75 3.75 0 0 1 0 7.5H7zM7 4.5V19"/>',
  italic: '<path d="M10 4.5h8M6 19.5h8M14 4.5l-4 15"/>',
  underline: '<path d="M7 4v7a5 5 0 0 0 10 0V4M5 20h14"/>',
  highlight: '<path class="f" d="M14.5 4.5l5 5-8 8H6.5v-5z"/><path d="M4 20.5h16"/>',
  alignl: '<path d="M4 6h16M4 10h10M4 14h16M4 18h10"/>',
  alignc: '<path d="M4 6h16M7 10h10M4 14h16M7 18h10"/>',
  bullets: '<circle class="f" cx="5.5" cy="7" r="1.5"/><circle cx="5.5" cy="12" r="1.5"/><circle cx="5.5" cy="17" r="1.5"/><path d="M10 7h10M10 12h10M10 17h10"/>',
  numbering: '<path d="M4 5.5l1.5-1v5M4 9.5h3M4 14.5a1.5 1.5 0 0 1 3 0c0 1-3 2.5-3 4h3M10 7h10M10 12h10M10 17h10"/>',
  comment: '<path class="f" d="M4 5a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-8l-4 4v-4H5a1 1 0 0 1-1-1z"/><path d="M8 8.5h8M8 12h5"/>',
  suggest: '<path class="f" d="M15 4l5 5-10 10H5v-5z"/><path d="M13 6l5 5"/><path d="M3 21.5h9" stroke-dasharray="2 2"/>',
  spell: '<path d="M3.5 15l3.5-9 3.5 9M4.8 12h4.4M12.5 16l3 3 6-7.5"/>',
  history: '<path d="M4 12a8 8 0 1 0 2.3-5.7"/><path d="M4 4v5h5M12 8v4l3 2"/>',
  share: '<circle class="f" cx="18" cy="5.5" r="2.5"/><circle cx="6" cy="12" r="2.5"/><circle cx="18" cy="18.5" r="2.5"/><path d="M8.2 10.8l7.6-4.1M8.2 13.2l7.6 4.1"/>',
  sort: '<path d="M7 4v16M3.5 16.5L7 20l3.5-3.5M13 6h8M13 11h6M13 16h4"/>',
  filter: '<path class="f" d="M4 5h16l-6 7.5V19l-4 2v-8.5z"/>',
  filldown: '<rect class="f" x="5" y="3" width="14" height="5" rx="1"/><path d="M12 10v10M8 16l4 4 4-4"/>',
  freeze: '<rect x="3" y="4" width="18" height="16" rx="1"/><path class="fo" d="M4 5h16v3H4zM4 8h4v11H4z"/><path d="M3 8h18M8 4v16"/>',
  chart: '<path d="M4 20h16"/><rect x="5.5" y="11" width="3" height="9" rx=".5"/><rect class="f" x="10.5" y="6" width="3" height="14" rx=".5"/><rect x="15.5" y="14" width="3" height="6" rx=".5"/>',
  pivot: '<rect x="3" y="3" width="18" height="18" rx="1"/><path class="fo" d="M4 4h16v4H4z"/><path d="M3 8h18M8 3v18M12 17h5v-5M17 17l-4-4"/>',
  money: '<rect class="f" x="3" y="6" width="18" height="12" rx="1"/><circle cx="12" cy="12" r="2.5"/><path d="M6.5 9v.5M17.5 14.5v.5"/>',
  percent: '<path d="M18 6L6 18"/><circle class="f" cx="7.5" cy="7.5" r="2.5"/><circle cx="16.5" cy="16.5" r="2.5"/>',
  slide: '<rect class="f" x="3" y="4" width="18" height="12" rx="1"/><path d="M12 16v4M8 20h8M7 8h6M7 11h10"/>',
  theme: '<path class="f" d="M12 3a9 9 0 1 0 0 18c1.2 0 1.8-.8 1.8-1.7 0-1.3-1.1-1.8-1.1-3 0-1 .8-1.8 1.9-1.8H17a4 4 0 0 0 4-4C21 6.3 17 3 12 3z"/><circle cx="7.5" cy="11" r="1"/><circle cx="10" cy="7" r="1"/><circle cx="15" cy="7.5" r="1"/>',
  transition: '<rect class="f" x="3" y="6" width="9" height="12" rx="1"/><rect x="12" y="6" width="9" height="12" rx="1" stroke-dasharray="2.5 2"/><path d="M6 12h9M12.5 9.5L15 12l-2.5 2.5"/>',
  present: '<rect x="3" y="4" width="18" height="12" rx="1"/><path class="f" d="M10 7.5v5l4.5-2.5z"/><path d="M12 16v4M8 20h8"/>',
  image: '<rect x="3" y="5" width="18" height="14" rx="1"/><circle cx="8.5" cy="9.5" r="1.5"/><path class="f" d="M4 18l5.5-5.5 3.5 3.5 2.5-2.5 4.5 4.5z"/>',
  textbox: '<rect class="f" x="3" y="5" width="18" height="14" rx="1" stroke-dasharray="3 2"/><path d="M8 9h8M12 9v7"/>',
  shape: '<circle class="f" cx="9" cy="9" r="5"/><rect class="fp" x="11" y="11" width="9" height="9" rx="1"/>',
  footnote: '<path d="M4 6h12M4 10h12M4 14h8M18 13.5l1.5-1v5"/>',
  pagebreak: '<path class="f" d="M6 3h12v6H6z"/><path d="M6 15h12v6H6z"/><path d="M3 12h18" stroke-dasharray="2.5 2"/>',
  toc: '<path d="M4 6h10M4 11h10M4 16h10M17 6h3M17 11h3M17 16h3"/>'
};
const EXTRA = {};
let PFX = 'i-';   // symbol ids are PFX + name; a page that already has its own i- icons installs with another prefix
const SVGNS = 'http://www.w3.org/2000/svg';
function symbol(n, body) {
  return `<symbol id="${PFX}${n}" viewBox="0 0 24 24">${body
    .replace(/class="fo"/g, 'style="fill:var(--ic-fill,none);stroke:none"')
    .replace(/class="fp"/g, 'style="fill:var(--panel)"')
    .replace(/class="f"/g, 'style="fill:var(--ic-fill,none)"')}</symbol>`;
}
/* Puts the family (and a page's own icons, drawn to the same rules) into the page once, as <symbol id="i-name">. */
function install(more, opts) {
  Object.assign(EXTRA, more || {});
  if (opts && opts.prefix) PFX = opts.prefix;
  let s = document.getElementById('rb-icons');
  if (!s) {
    s = document.createElementNS(SVGNS, 'svg');
    s.id = 'rb-icons';
    s.setAttribute('aria-hidden', 'true');
    s.setAttribute('focusable', 'false');
    s.setAttribute('width', '0');
    s.setAttribute('height', '0');
    s.style.position = 'absolute';
    document.body.prepend(s);
  }
  s.innerHTML = Object.entries(Object.assign({}, ICONS, EXTRA)).map(([n, b]) => symbol(n, b)).join('');
}
const icon = (n, cls = '') => `<svg class="rb-ic ${cls}" aria-hidden="true" focusable="false"><use href="#${PFX}${esc(n)}"/></svg>`;

/* ---------- small helpers ---------- */
const store = {
  get(k) { try { return localStorage.getItem(k); } catch (e) { return null; } },
  set(k, v) { try { localStorage.setItem(k, v); } catch (e) { /* private window: the choice lasts for this visit */ } }
};
const keyOf = it => it.id || it.domId || [it.act || 'run', it.a == null ? '' : it.a, it.label].join('|');
/* Extra data attributes a page's own handlers read, for example {cmd: 'bold'} becomes data-cmd="bold". */
const dataAttrs = d => Object.entries(d).map(([k, v]) => ` data-${k.replace(/[A-Z]/g, c => '-' + c.toLowerCase())}="${esc(v)}"`).join('');
const focusables = el => Array.from(el.querySelectorAll('button:not([disabled]),[tabindex="0"]')).filter(b => b.offsetParent !== null);

/* ---------- the ribbon ---------- */
class Ribbon {
  constructor(host, opts) {
    this.host = host;
    this.o = opts || {};
    this.cfg = {tabs: [], contextual: [], quick: []};
    this.active = null;
    this.lastStd = null;
    this.ctxSeen = [];
    this.min = store.get('sw-ribbon-min:' + (this.o.app || '')) === '1';
    this.peek = false;
    this.pop = null;              // the open menu, flyout or File menu: {el, trigger}
    this.items = [];              // items of the last render, by index
    this.uid = 'rb' + (++Ribbon.n);
    host.classList.add('rb');
    host.addEventListener('click', e => this.onClick(e));
    host.addEventListener('keydown', e => this.onKey(e));
    this.tipOn(host);
    if (this.o.quick) {
      this.o.quick.classList.add('rb-qat');
      this.o.quick.addEventListener('click', e => this.onClick(e));
      this.tipOn(this.o.quick);
    }
    document.addEventListener('pointerdown', e => {
      if (this.pop && !this.pop.el.contains(e.target) && !(this.pop.trigger && this.pop.trigger.contains(e.target))) this.closePop();
      if (this.peek && !this.host.contains(e.target) && !(this.pop && this.pop.el.contains(e.target))) this.setPeek(false);
    }, true);
    window.addEventListener('resize', () => { this.closePop(); this.fit(); });
    window.addEventListener('blur', () => { this.closePop(); if (this.peek) this.setPeek(false); });   // a click into a frame, such as a map
    if (global.ResizeObserver) new ResizeObserver(() => this.fit()).observe(host);
  }

  /* cfg: {file, quick, tabs, contextual}. A contextual tab with a new key (for example a new selection) comes to the front;
     when it goes away the ribbon goes back to the tab you were on. */
  update(cfg) {
    Object.assign(this.cfg, cfg);
    const ctx = this.cfg.contextual || [];
    const fresh = ctx.filter(t => !this.ctxSeen.includes(t.key || t.id));
    const isCtx = id => ctx.some(t => t.id === id);
    if (fresh.length) {
      if (!this.ctxIds || !this.ctxIds.includes(this.active)) this.lastStd = this.active;
      this.active = fresh[fresh.length - 1].id;
    } else if (this.ctxIds && this.ctxIds.includes(this.active) && !isCtx(this.active)) {
      this.active = this.lastStd;
    }
    this.ctxSeen = ctx.map(t => t.key || t.id);
    this.ctxIds = ctx.map(t => t.id);
    if (!this.allTabs().some(t => t.id === this.active)) this.active = (this.cfg.tabs[0] || {}).id;
    this.render();
  }
  allTabs() { return (this.cfg.tabs || []).concat(this.cfg.contextual || []); }
  /* Brings a tab to the front, for example the discipline the page just switched to. */
  select(id) { if (this.allTabs().some(t => t.id === id)) { if (!(this.ctxIds || []).includes(id)) this.lastStd = id; this.show(id); } }

  render() {
    const fa = document.activeElement, fk = fa && (this.host.contains(fa) || (this.o.quick && this.o.quick.contains(fa))) ? fa.dataset.rk : null;
    this.closePop(true);
    this.hideTip();
    this.items = [];
    const tabs = this.allTabs(), cur = tabs.find(t => t.id === this.active) || tabs[0];
    if (cur) this.active = cur.id;
    const ctxIds = this.ctxIds || [], u = this.uid;
    const tabBtn = t => {
      const on = cur && t.id === cur.id, c = ctxIds.includes(t.id);
      return `<button type="button" class="rb-tab${c ? ' ctx' : ''}" role="tab" id="${u}-t-${esc(t.id)}" data-rb-tab="${esc(t.id)}" data-rk="tab:${esc(t.id)}" aria-selected="${on}" aria-controls="${u}-b-${esc(t.id)}" tabindex="${on ? 0 : -1}">${c && t.group ? `<span class="rb-ctxg">${esc(t.group)}</span>` : ''}<span class="rb-tl">${esc(t.label)}</span></button>`;
    };
    const file = this.cfg.file, fdata = file && file.data ? dataAttrs(file.data) : ' data-rb-file="1" aria-haspopup="menu" aria-expanded="false"';
    this.host.dataset.min = String(this.isMin());
    this.host.classList.toggle('peek', this.peek);
    this.host.innerHTML = `<div class="rb-bar">${file ? `<button type="button" class="rb-file"${fdata} data-rk="file">${esc(file.label || 'File')}</button>` : ''}<div class="rb-tabs" role="tablist" aria-label="${esc(this.o.label || 'Ribbon')}">${tabs.map(tabBtn).join('')}</div><span class="rb-sp"></span>${this.cfg.end ? `<div class="rb-end">${this.cfg.end}</div>` : ''}<button type="button" class="rb-mini" data-rb-min="1" data-rk="min" aria-pressed="${this.min}" data-tl="${this.min ? 'Show the whole ribbon' : 'Show tabs only'}" aria-label="${this.min ? 'Show the whole ribbon' : 'Show tabs only'}">${icon(this.min ? 'chevd' : 'chevu')}</button></div>`
      + tabs.map(t => `<div class="rb-body${ctxIds.includes(t.id) ? ' ctx' : ''}" id="${u}-b-${esc(t.id)}" data-rb-body="${esc(t.id)}" role="tabpanel" aria-labelledby="${u}-t-${esc(t.id)}"${cur && t.id === cur.id ? '' : ' hidden'}>${(t.panels || []).map((p, i) => this.panelHTML(p, i)).join('')}</div>`).join('');
    this.body = cur ? this.host.querySelector(`[data-rb-body="${CSS.escape(cur.id)}"]`) : null;
    if (this.o.quick) this.o.quick.innerHTML = (this.cfg.quick || []).map(it => this.btn(it, 'q')).join('');
    this.fit();
    if (fk) { const el = this.find(fk); if (el) el.focus(); }
  }
  /* Shows one tab's panels without rebuilding anything. */
  show(id) {
    const tabs = this.allTabs();
    if (!tabs.some(t => t.id === id)) return;
    this.closePop(true);
    this.hideTip();
    this.active = id;
    this.host.querySelectorAll('.rb-tab').forEach(b => { const on = b.dataset.rbTab === id; b.setAttribute('aria-selected', String(on)); b.tabIndex = on ? 0 : -1; });
    this.host.querySelectorAll('.rb-body').forEach(b => { b.hidden = b.dataset.rbBody !== id; });
    this.body = this.host.querySelector(`[data-rb-body="${CSS.escape(id)}"]`);
    this.fit();
  }
  find(rk) {
    const sel = `[data-rk="${CSS.escape(rk)}"]`;
    return (this.body && this.body.querySelector(sel)) || this.host.querySelector('.rb-bar ' + sel) || this.host.querySelector(sel) || (this.o.quick && this.o.quick.querySelector(sel));
  }

  panelHTML(p, i, open) {
    const body = (p.items || []).map(it => it.stack ? `<div class="rb-stack">${it.stack.map(s => this.btn(s, 'sm')).join('')}</div>` : this.btn(it, it.size || 'lg')).join('');
    const pi = open ? '' : ` data-pi="${i}"`;
    return `<div class="rb-panel${open ? ' open' : ''}" role="group" aria-label="${esc(p.label)}"${pi}><div class="rb-pc">${body}</div><div class="rb-pl" aria-hidden="true">${esc(p.label)}</div>${open ? '' : `<button type="button" class="rb-pbtn" data-rb-panel="${i}" data-rk="panel:${esc(p.label)}" aria-haspopup="true" aria-expanded="false">${icon(p.icon || ((p.items || []).map(x => x.stack ? x.stack[0] : x)[0] || {}).icon || 'chevd', 'lg')}<span>${esc(p.label)}${icon('chevd', 'dd')}</span></button>`}</div>`;
  }

  /* One button. size: lg (icon over label), sm (icon beside label), q (quick access: icon only), m (menu row). */
  btn(it, size) {
    if (it.sep) return size === 'm' ? '<div class="rb-msep" role="separator"></div>' : '<span class="rb-vsep" aria-hidden="true"></span>';
    if (it.head) return `<div class="rb-mh">${esc(it.head)}</div>`;
    const k = this.items.push(it) - 1;
    const rk = keyOf(it);
    const dis = it.disabled ? ' aria-disabled="true"' : '';
    const act = (it.disabled ? '' : it.act ? ` data-act="${esc(it.act)}"${it.a != null ? ` data-a="${esc(it.a)}"` : ''}` : it.run ? ` data-rb-run="${k}"` : '') + (it.data && !it.disabled ? dataAttrs(it.data) : '');
    const tip = ` data-rb-tip="${esc(it.tip || '')}" data-tl="${esc(it.label)}"${it.kbd ? ` data-rb-kbd="${esc(it.kbd)}"` : ''}`;
    const pressed = it.pressed != null && size !== 'm' ? ` aria-pressed="${!!it.pressed}"` : '';
    const ic = it.icon ? icon(it.icon, size === 'lg' ? 'lg' : '') : '';
    const lbl = size === 'q' ? '' : `<span class="rb-l">${esc(it.label)}${it.menu && !it.split ? icon('chevd', 'dd') : ''}</span>`;
    const id = it.domId ? ` id="${esc(it.domId)}"` : '';
    if (it.options) {
      const sel = `<select${id}${act}${dis ? ' disabled' : ''} aria-label="${esc(it.label)}"${tip}>${it.options.map(o => { const [v, l] = Array.isArray(o) ? o : [o, o]; return `<option value="${esc(v)}"${String(v) === String(it.value) ? ' selected' : ''}>${esc(l)}</option>`; }).join('')}</select>`;
      return `<label class="rb-sel ${size === 'lg' ? 'rb-sel-lg' : ''}">${it.icon ? icon(it.icon) : ''}${it.show ? `<span class="rb-l">${esc(it.label)}</span>` : ''}${sel}</label>`;
    }
    if (size === 'm') {
      const role = it.pressed != null ? `menuitemcheckbox" aria-checked="${!!it.pressed}` : 'menuitem';
      return `<button type="button" class="rb-mi" role="${role}" data-rk="${esc(rk)}"${act}${dis}${it.tip ? ` title="${esc(it.tip)}"` : ''}>${it.pressed != null ? `<span class="rb-mck">${it.pressed ? icon('check') : ''}</span>` : ''}${ic}<span class="rb-l">${esc(it.label)}${it.sub ? `<small>${esc(it.sub)}</small>` : ''}</span>${it.kbd ? `<kbd>${esc(it.kbd)}</kbd>` : ''}</button>`;
    }
    const cls = size === 'q' ? 'rb-q' : 'rb-' + size;
    if (it.menu && it.split) {
      return `<div class="rb-sb ${cls}"><button type="button" class="${cls} rb-main"${id} data-rk="${esc(rk)}"${act}${dis}${pressed}${tip} aria-label="${esc(it.label)}">${ic}${lbl}</button><button type="button" class="rb-arrow" data-rb-menu="${k}" data-rk="${esc(rk)}|more" aria-haspopup="menu" aria-expanded="false" aria-label="More: ${esc(it.label)}"${dis}>${icon('chevd', 'dd')}</button></div>`;
    }
    const menu = it.menu ? ` data-rb-menu="${k}" aria-haspopup="menu" aria-expanded="false"` : act;
    return `<button type="button" class="${cls}"${id} data-rk="${esc(rk)}"${menu}${dis}${pressed}${tip} aria-label="${esc(it.label)}">${ic}${lbl}</button>`;
  }

  /* Narrow windows: first the small buttons lose their words, then whole panels fold into one button, from the right. */
  fit() {
    const b = this.body;
    if (!b || (this.isMin() && !this.peek)) return;
    const ps = Array.from(b.querySelectorAll(':scope>.rb-panel'));
    ps.forEach(p => p.classList.remove('s1', 's2'));
    const fits = () => b.scrollWidth <= b.clientWidth + 1;
    if (fits()) return;
    for (let i = ps.length - 1; i >= 0; i--) { ps[i].classList.add('s1'); if (fits()) return; }
    for (let i = ps.length - 1; i >= 0; i--) { ps[i].classList.replace('s1', 's2'); if (fits()) return; }
  }

  onClick(e) {
    const t = e.target.closest('button');
    if (!t) return;
    if (t.getAttribute('aria-disabled') === 'true') { e.preventDefault(); e.stopPropagation(); return; }
    if (t.dataset.rbTab) { this.tabClick(t.dataset.rbTab); return; }
    if (t.dataset.rbMin) { this.setMin(!this.min); return; }
    if (t.dataset.rbFile) { this.toggleFile(t); return; }
    if (t.dataset.rbPanel != null) { this.toggleFlyout(t); return; }
    if (t.dataset.rbMenu != null) { this.toggleMenu(t, this.items[+t.dataset.rbMenu].menu); return; }
    if (t.dataset.rbRun != null) { const it = this.items[+t.dataset.rbRun]; this.done(); if (it && it.run) it.run(t, e); return; }
    if (this.host.contains(t) || (this.pop && this.pop.el.contains(t))) this.done();   // the page's own click handler runs it
  }
  done() { this.closePop(); this.hideTip(); if (this.peek) this.setPeek(false); }
  tabClick(id) {
    if (!(this.ctxIds || []).includes(id)) this.lastStd = id;
    if (this.isMin()) { if (this.peek && id === this.active) { this.setPeek(false); return; } this.show(id); this.setPeek(true); }
    else this.show(id);
  }
  isMin() { return this.min || !!this.folded; }
  fold(v) { if (!!this.folded === !!v) return; this.folded = !!v; this.applyMin(); }
  setMin(v) { this.min = v; store.set('sw-ribbon-min:' + (this.o.app || ''), v ? '1' : '0'); this.applyMin(); }
  /* Tabs only, or the whole ribbon: switched in place, so nothing on the ribbon is rebuilt. */
  applyMin() {
    this.peek = false;
    this.host.classList.remove('peek');
    this.host.dataset.min = String(this.isMin());
    const b = this.host.querySelector('.rb-mini');
    if (b) { const t = this.min ? 'Show the whole ribbon' : 'Show tabs only'; b.setAttribute('aria-pressed', String(this.min)); b.setAttribute('aria-label', t); b.dataset.tl = t; b.innerHTML = icon(this.min ? 'chevd' : 'chevu'); }
    this.fit();
  }
  setPeek(v) { if (this.peek === v) return; this.peek = v; this.host.classList.toggle('peek', v); this.fit(); }

  onKey(e) {
    const t = e.target;
    if (t.classList && t.classList.contains('rb-tab') && ['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) {
      const L = Array.from(this.host.querySelectorAll('.rb-tab')), i = L.indexOf(t);
      const j = e.key === 'Home' ? 0 : e.key === 'End' ? L.length - 1 : (i + (e.key === 'ArrowRight' ? 1 : -1) + L.length) % L.length;
      e.preventDefault();
      if (!(this.ctxIds || []).includes(L[j].dataset.rbTab)) this.lastStd = L[j].dataset.rbTab;
      this.show(L[j].dataset.rbTab);
      L[j].focus();
      return;
    }
    if (t.classList && t.classList.contains('rb-tab') && e.key === 'ArrowDown' && !(this.isMin() && !this.peek)) {
      const f = focusables(this.body)[0]; if (f) { e.preventDefault(); f.focus(); }
      return;
    }
    if (e.key === 'Escape' && this.peek) { e.stopPropagation(); this.setPeek(false); const n = this.find('tab:' + this.active); if (n) n.focus(); }
  }

  /* ---------- menus, flyouts and the File menu: one open at a time ---------- */
  place(el, trigger, alignRight) {
    const r = trigger.getBoundingClientRect();
    el.style.top = Math.round(r.bottom + 2) + 'px';
    document.body.appendChild(el);
    const w = el.offsetWidth, h = el.offsetHeight;
    let x = alignRight ? r.right - w : r.left;
    x = Math.max(8, Math.min(x, innerWidth - w - 8));
    el.style.left = Math.round(x) + 'px';
    if (r.bottom + 2 + h > innerHeight - 8 && r.top - h - 2 > 8) el.style.top = Math.round(r.top - h - 2) + 'px';
  }
  openPop(el, trigger, first) {
    this.closePop();
    this.hideTip();
    el.addEventListener('click', e => this.onClick(e));
    el.addEventListener('keydown', e => this.popKey(e));
    this.pop = {el, trigger};
    if (trigger) trigger.setAttribute('aria-expanded', 'true');
    if (first !== false) { const f = focusables(el)[0]; if (f) f.focus(); }
  }
  closePop(quiet) {
    const p = this.pop;
    if (!p) return;
    this.pop = null;
    p.el.hidden = true;
    setTimeout(() => p.el.remove(), 0);
    if (p.trigger) {
      p.trigger.setAttribute('aria-expanded', 'false');
      if (!quiet && p.el.contains(document.activeElement)) p.trigger.focus();
    }
  }
  popKey(e) {
    const L = focusables(this.pop ? this.pop.el : e.currentTarget), i = L.indexOf(document.activeElement);
    if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); const tr = this.pop && this.pop.trigger; this.closePop(true); if (tr && document.body.contains(tr)) tr.focus(); return; }
    if (e.key === 'Tab') { this.closePop(true); return; }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); const j = i < 0 ? 0 : (i + (e.key === 'ArrowDown' ? 1 : -1) + L.length) % L.length; if (L[j]) L[j].focus(); }
    if (e.key === 'Home' || e.key === 'End') { e.preventDefault(); const f = e.key === 'Home' ? L[0] : L[L.length - 1]; if (f) f.focus(); }
  }
  toggleMenu(trigger, items) {
    if (this.pop && this.pop.trigger === trigger) { this.closePop(); return; }
    this.openMenu(trigger, items);
  }
  /* A dropdown menu under any button, also usable by the page (for example the view control bar). */
  openMenu(trigger, items, opts) {
    const el = document.createElement('div');
    el.className = 'rb-menu';
    el.setAttribute('role', 'menu');
    el.setAttribute('aria-label', trigger.getAttribute('aria-label') || trigger.textContent.trim());
    el.innerHTML = (items || []).map(it => this.btn(it, 'm')).join('');
    this.place(el, trigger, opts && opts.right);
    this.openPop(el, trigger);
    const c = el.querySelector('[aria-checked="true"]'); if (c) c.focus();
  }
  toggleFlyout(trigger) {
    if (this.pop && this.pop.trigger === trigger) { this.closePop(); return; }
    const tab = this.allTabs().find(t => t.id === this.active), p = tab && tab.panels[+trigger.dataset.rbPanel];
    if (!p) return;
    const el = document.createElement('div');
    el.className = 'rb-flyout';
    el.setAttribute('role', 'group');
    el.setAttribute('aria-label', p.label);
    el.innerHTML = this.panelHTML(p, 0, true);
    this.place(el, trigger);
    this.openPop(el, trigger);
  }
  toggleFile(trigger) {
    if (this.pop && this.pop.trigger === trigger) { this.closePop(); return; }
    const f = this.cfg.file;
    const el = document.createElement('div');
    el.className = 'rb-filemenu';
    el.setAttribute('role', 'menu');
    el.setAttribute('aria-label', f.label || 'File');
    const side = f.side ? `<div class="rb-fside"><div class="rb-mh">${esc(f.side.head || 'Recent')}</div>${(f.side.items || []).map(it => this.btn(it, 'm')).join('')}</div>` : '';
    el.innerHTML = `<div class="rb-fmain">${(f.items || []).map(it => this.btn(it, 'm')).join('')}</div>${side}`;
    this.place(el, trigger);
    this.openPop(el, trigger);
  }

  /* ---------- tooltips: instant, white, with a 1px ink border ---------- */
  tipOn(root) {
    root.addEventListener('mouseover', e => { const b = e.target.closest('[data-tl]'); if (b && root.contains(b)) this.showTip(b); });
    root.addEventListener('mouseout', e => { const b = e.target.closest('[data-tl]'); if (b && !b.contains(e.relatedTarget)) this.hideTip(); });
    root.addEventListener('focusin', e => { const b = e.target.closest('[data-tl]'); if (b && b.matches(':focus-visible')) this.showTip(b); });
    root.addEventListener('focusout', () => this.hideTip());
  }
  showTip(b) {
    if (this.pop && this.pop.el.contains(b)) return;
    const label = b.dataset.tl || '', tip = b.dataset.rbTip || '', kbd = b.dataset.rbKbd || '';
    if (!tip && !kbd && b.querySelector('.rb-l') && b.offsetWidth > 0 && getComputedStyle(b.querySelector('.rb-l')).display !== 'none') return;
    let t = this.tip;
    if (!t) { t = this.tip = document.createElement('div'); t.className = 'rb-tip'; t.setAttribute('role', 'tooltip'); document.body.appendChild(t); }
    t.innerHTML = `${label ? `<b>${esc(label)}</b>` : ''}${kbd ? ` <kbd>${esc(kbd)}</kbd>` : ''}${tip ? `<div>${esc(tip)}</div>` : ''}`;
    t.hidden = false;
    const r = b.getBoundingClientRect();
    t.style.left = '0px'; t.style.top = '0px';
    const w = t.offsetWidth, h = t.offsetHeight;
    t.style.left = Math.round(Math.max(8, Math.min(r.left, innerWidth - w - 8))) + 'px';
    t.style.top = Math.round(r.bottom + h + 10 > innerHeight ? r.top - h - 6 : r.bottom + 6) + 'px';
  }
  hideTip() { if (this.tip) this.tip.hidden = true; }
}

/* ---------- docked palettes ----------
   A work area holds two docks, <aside class="rb-dock" data-side="left"> and data-side="right", either side of the view.
   Each palette is <section class="rb-pal" data-pal="id"> with a .rb-pal-h title. Palettes start on the side the page asks
   for (opts.sides, for example {props: 'left', browser: 'right'}; the right if it does not say). People move them by
   dragging the title, or with the move button on the title, which works from the keyboard; the layout is remembered for that viewer. */
class Dock {
  constructor(work, opts) {
    this.work = work;
    this.o = opts || {};
    this.sides = {left: work.querySelector('.rb-dock[data-side="left"]'), right: work.querySelector('.rb-dock[data-side="right"]')};
    this.pals = Array.from(work.querySelectorAll('.rb-pal[data-pal]'));
    this.key = 'sw-dock:' + (this.o.app || '');
    this.sig = {};
    const want = this.o.sides || {}, grow = this.o.grow || {};
    this.defaults = this.pals.map((p, i) => ({id: p.dataset.pal, side: want[p.dataset.pal] || 'right', i, grow: grow[p.dataset.pal] || 1}));
    let saved = null;
    try { saved = JSON.parse(store.get(this.key) || 'null'); } catch (e) { saved = null; }
    this.at = {};
    this.defaults.forEach(d => {
      const s = saved && saved[d.id];
      this.at[d.id] = s && this.sides[s.side] ? {side: s.side, i: +s.i || 0, grow: +s.grow || d.grow} : {side: d.side, i: d.i, grow: d.grow};
    });
    this.pals.forEach(p => this.decorate(p));
    this.layout();
  }
  pal(id) { return this.pals.find(p => p.dataset.pal === id); }
  order(side) { return Object.keys(this.at).filter(id => this.at[id].side === side).sort((a, b) => this.at[a].i - this.at[b].i); }
  save() { store.set(this.key, JSON.stringify(this.at)); }
  decorate(p) {
    const h = p.querySelector('.rb-pal-h'), t = h.querySelector('.t');
    const name = (t ? t.childNodes[0].textContent : h.textContent).trim();
    p.dataset.name = name;
    const mv = document.createElement('button');
    mv.type = 'button';
    mv.className = 'rb-x';
    mv.dataset.rbDockMenu = p.dataset.pal;
    mv.setAttribute('aria-label', 'Move ' + name);
    mv.setAttribute('aria-haspopup', 'menu');
    mv.dataset.tl = 'Move ' + name;
    mv.dataset.rbTip = 'Drag the title to dock it on either side, or pick a place here.';
    mv.innerHTML = icon('move');
    h.insertBefore(mv, h.querySelector('.rb-x'));
    mv.addEventListener('click', () => this.menu(mv, p.dataset.pal));
    h.addEventListener('pointerdown', e => this.drag(e, p));
    if (this.o.ribbon) this.o.ribbon.tipOn(h);
  }
  menu(btn, id) {
    const a = this.at[id], same = this.order(a.side), i = same.indexOf(id);
    this.o.ribbon.openMenu(btn, [
      {label: 'Dock on the left', pressed: a.side === 'left', run: () => this.move(id, 'left')},
      {label: 'Dock on the right', pressed: a.side === 'right', run: () => this.move(id, 'right')},
      {sep: 1},
      {label: 'Move up', disabled: i <= 0, run: () => this.move(id, a.side, i - 1)},
      {label: 'Move down', disabled: i >= same.length - 1, run: () => this.move(id, a.side, i + 1)},
      {sep: 1},
      {label: 'Put every palette back', run: () => this.reset()}
    ], {right: true});
  }
  move(id, side, index) {
    this.order(this.at[id].side).filter(x => x !== id).forEach((x, k) => { this.at[x].i = k; });
    const to = this.order(side).filter(x => x !== id);
    to.splice(index == null ? to.length : Math.max(0, Math.min(index, to.length)), 0, id);
    this.at[id].side = side;
    to.forEach((x, k) => { this.at[x].i = k; });
    this.save();
    this.layout();
    const mv = this.pal(id).querySelector('[data-rb-dock-menu]');
    if (mv) mv.focus();
  }
  reset() { this.defaults.forEach(d => { this.at[d.id] = {side: d.side, i: d.i, grow: d.grow}; }); this.save(); this.layout(); }
  /* Puts each palette in its dock and order. Nodes move only when the order changed, so typing in a palette is never interrupted. */
  layout() {
    for (const side of ['left', 'right']) {
      const d = this.sides[side];
      if (!d) continue;
      const want = this.order(side).map(id => this.pal(id));
      const cur = Array.from(d.children).filter(c => c.classList.contains('rb-pal'));
      if (want.length !== cur.length || want.some((p, k) => p !== cur[k])) want.forEach(p => d.appendChild(p));
      const vis = want.filter(p => !p.hidden), sig = vis.map(p => p.dataset.pal).join();
      vis.forEach(p => { p.style.flexGrow = String(this.at[p.dataset.pal].grow); });
      if (this.sig[side] !== sig) {
        this.sig[side] = sig;
        d.querySelectorAll(':scope>.rb-split').forEach(s => s.remove());
        vis.forEach((p, k) => { if (k) d.insertBefore(this.splitter(vis[k - 1], p), p); });
      }
      d.hidden = !vis.length;
      this.work.classList.toggle('dock-' + side, vis.length > 0);
    }
  }
  splitter(a, b) {
    const s = document.createElement('div');
    s.className = 'rb-split';
    s.tabIndex = 0;
    s.setAttribute('role', 'separator');
    s.setAttribute('aria-orientation', 'horizontal');
    s.setAttribute('aria-label', 'Resize ' + a.dataset.name + ' and ' + b.dataset.name);
    const A = () => this.at[a.dataset.pal], B = () => this.at[b.dataset.pal];
    const set = f => { const t = A().grow + B().grow; A().grow = t * f; B().grow = t * (1 - f); a.style.flexGrow = String(A().grow); b.style.flexGrow = String(B().grow); };
    let box = null;
    s.addEventListener('pointerdown', e => { box = {top: a.getBoundingClientRect().top, h: a.offsetHeight + b.offsetHeight}; s.setPointerCapture(e.pointerId); e.preventDefault(); });
    s.addEventListener('pointermove', e => { if (box) set(Math.max(.12, Math.min(.88, (e.clientY - box.top) / box.h))); });
    s.addEventListener('pointerup', () => { if (box) { box = null; this.save(); } });
    s.addEventListener('keydown', e => {
      if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
      e.preventDefault();
      set(Math.max(.12, Math.min(.88, A().grow / (A().grow + B().grow) + (e.key === 'ArrowUp' ? -.05 : .05))));
      this.save();
    });
    return s;
  }
  /* Dragging a title: a dashed outline shows where the palette will land, on the nearer side, above or below the others there. */
  drag(e, p) {
    if (e.button !== 0 || e.target.closest('button,input,select,a')) return;
    const id = p.dataset.pal, x0 = e.clientX, y0 = e.clientY;
    let on = false, ghost = null, target = null;
    const move = ev => {
      if (!on && Math.hypot(ev.clientX - x0, ev.clientY - y0) < 6) return;
      if (!on) { on = true; ghost = document.createElement('div'); ghost.className = 'rb-drop'; document.body.appendChild(ghost); p.classList.add('dragging'); }
      target = this.dropAt(ev.clientX, ev.clientY, id);
      const r = target.rect;
      Object.assign(ghost.style, {left: r.left + 'px', top: r.top + 'px', width: r.width + 'px', height: r.height + 'px'});
    };
    const up = () => {
      document.removeEventListener('pointermove', move);
      document.removeEventListener('pointerup', up);
      if (!on) return;
      ghost.remove();
      p.classList.remove('dragging');
      if (target) this.move(id, target.side, target.i);
    };
    document.addEventListener('pointermove', move);
    document.addEventListener('pointerup', up);
  }
  dropAt(x, y, id) {
    const w = this.work.getBoundingClientRect(), side = x < w.left + w.width / 2 ? 'left' : 'right';
    const W = this.pal(id).offsetWidth || 280;
    const full = this.order(side).filter(k => k !== id), vis = full.map(k => this.pal(k)).filter(q => !q.hidden);
    let i = vis.length;
    for (let k = 0; k < vis.length; k++) { const r = vis[k].getBoundingClientRect(); if (y < r.top + r.height / 2) { i = k; break; } }
    let top = w.top, h = w.height;
    if (vis.length) {
      const r = vis[Math.min(i, vis.length - 1)].getBoundingClientRect();
      top = i < vis.length ? r.top : r.top + r.height / 2;
      h = r.height / 2;
    }
    const at = !vis.length ? full.length : i < vis.length ? full.indexOf(vis[i].dataset.pal) : full.indexOf(vis[vis.length - 1].dataset.pal) + 1;
    return {side, i: at, rect: {left: side === 'left' ? w.left : w.right - W, top, width: W, height: h}};
  }
}

Ribbon.n = 0;

global.SpareIcons = {install, icon, names: () => Object.keys(Object.assign({}, ICONS, EXTRA))};
global.SpareRibbon = {create: (host, opts) => new Ribbon(host, opts), dock: (work, opts) => new Dock(work, opts), icon};
})(window);
