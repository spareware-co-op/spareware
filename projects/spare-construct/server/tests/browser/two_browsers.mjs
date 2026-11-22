// Two real browsers on the real page: separate sign-ins, real mouse drags. Needs the stack running
// on a fresh seed; scripts/run-browser-test.sh sets that up.
//   STACK_URL    the server, default http://localhost:8080
//   BROWSER      a browser channel, msedge or chrome, or bundled for Playwright's own; default msedge
//   STACK_KILL   a command that kills the server the hard way
//   STACK_START  a command that starts it again
//   SHOTS        a folder for screenshots
//   STACK_GEOMETRY  kernel when the server runs on the geometry kernel, as the page must then too
import { execSync } from 'node:child_process';
import { chromium } from 'playwright-core';

const base = process.env.STACK_URL || 'http://localhost:8080';
const shots = process.env.SHOTS;
const channel = process.env.BROWSER || 'msedge';
const browser = await chromium.launch(channel === 'bundled' ? {} : { channel });

function pass(label) {
  console.log(`PASS  ${label}`);
}

function check(ok, label) {
  if (!ok) throw new Error(`FAILED: ${label}`);
  pass(label);
}

async function signIn(person, blocked) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  if (blocked) await context.route(blocked, (route) => route.abort());
  const page = await context.newPage();
  page.on('pageerror', (e) => {
    console.error(`${person}'s page: ${e}`);
    process.exit(1);
  });
  await page.goto(`${base}/auth/login?hint=${person}@example.test`);
  await page.waitForSelector('html[data-stack="ready"]');
  await page.waitForSelector('#saved[data-state="live"]');
  return page;
}

// An element's box on screen. A redraw (after Escape, a claim or an edit) replaces the element's
// drawing, and boundingBox() is null while it is away, so wait until it is drawn again (a slow
// runner caught it away once, between Escape and the click on the wall).
async function box(page, id) {
  const body = page.locator(`g.element[data-id="${id}"] .body`);
  for (let tries = 0; tries < 20; tries += 1) {
    await body.waitFor({ state: 'visible' });
    const found = await body.boundingBox();
    if (found) return found;
  }
  throw new Error(`${id} never stayed drawn long enough to find its box`);
}
const position = (page, id) =>
  page
    .locator(`g.element[data-id="${id}"]`)
    .evaluate((g) => [Number(g.dataset.x), Number(g.dataset.y)]);

async function drag(page, id, dx, dy, steps = 25) {
  const from = await box(page, id);
  const x = from.x + from.width / 2;
  const y = from.y + from.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  for (let i = 1; i <= steps; i += 1) {
    await page.mouse.move(x + (dx * i) / steps, y + (dy * i) / steps);
    await page.waitForTimeout(16);
  }
}

async function shot(page, name) {
  if (shots) await page.screenshot({ path: `${shots}/${name}.png` });
}

function stats(list) {
  const sorted = [...list].sort((a, b) => a - b);
  const at = (share) => sorted[Math.floor((sorted.length - 1) * share)];
  return `${sorted.length} edits, median ${at(0.5)} ms, 95% ${at(0.95)} ms, worst ${sorted.at(-1)} ms`;
}

console.log('\n   Before signing in');
const stranger = await (await browser.newContext()).newPage();
await stranger.goto(base);
await stranger.waitForSelector('#stack-signin a[href="/auth/login"]');
check(!(await stranger.locator('#app').isVisible()), 'the page offers sign-in and shows nothing else');
await shot(stranger, 'sign-in');

console.log('\n1. Anna logs in and lands in Company 1');
const anna = await signIn('anna');
await anna.waitForSelector('g.element[data-id="D-101"]');
check((await anna.locator('#stack-hub').inputValue()) === 'company-1', 'the page opens in Company 1');
check((await anna.locator('#stack-project').textContent()) === 'Example model 1', 'on Example model 1');
check((await anna.locator('#ribbon [role="tab"]').count()) >= 5, "in the demo's own ribbon");
check(await anna.locator('#pal-browser').isVisible(), 'with its project browser palette');
check(await anna.locator('#pal-props').isVisible(), 'and its properties palette');
await shot(anna, 'anna-company-1');

console.log('\n2. Dan switches between Company 2 and Company 1');
const dan = await signIn('dan');
check((await dan.locator('#stack-hub').inputValue()) === 'company-2', 'Dan starts in Company 2');
check((await dan.locator('#stack-project').textContent()).includes('(bridged)'), 'with his own project');
await dan.locator('#stack-hub').selectOption('company-1');
await dan.waitForSelector('[data-hidden-folder="arch"]');
check((await dan.locator('#stack-project').textContent()) === 'Example model 1', 'he switches to Company 1');
const wip = await dan.locator('[data-folder="arch/wip"]').count();
check(wip === 0, "the Architecture team's WIP isn't there for him");
check((await dan.locator('[data-model="arch-model"]').count()) === 0, 'nor is the Arch model');
check((await dan.locator('[data-package]').count()) > 0, 'Shared packages are');
await shot(dan, 'dan-company-1');
await dan.locator('#stack-hub').selectOption('company-2');
await dan.waitForSelector('[data-folder="structure/wip"]');
pass('and back to Company 2, where his WIP is');

console.log("\n3. Two browsers each see the other's edit in under 200 ms");
const ben = await signIn('ben');
await ben.waitForSelector('g.element[data-id="D-101"]');
const start = await position(ben, 'D-101');
await drag(anna, 'D-101', 120, 40);
await ben.waitForSelector('g.element[data-id="D-101"] rect.claim.theirs');
pass("Anna's claim shows on Ben's screen while she works");
const tag = await ben.locator('g.element[data-id="D-101"] text.claimt').textContent();
check(tag === 'Anna editing', 'with her name on it');
await shot(ben, 'ben-sees-anna-claim');
const blocked = await box(ben, 'D-101');
await ben.mouse.click(blocked.x + blocked.width / 2, blocked.y + blocked.height / 2);
await ben.waitForFunction(() =>
  document.getElementById('toast-m').textContent.includes('Anna is working on this'),
);
pass('Ben clicking it is told Anna is working on it');
await anna.mouse.up();
await ben.waitForFunction(
  ([x]) => Number(document.querySelector('g.element[data-id="D-101"]').dataset.x) !== x,
  start,
);
await anna.waitForTimeout(300);
const [atAnna, atBen] = [await position(anna, 'D-101'), await position(ben, 'D-101')];
check(atAnna.join() === atBen.join(), `both browsers show the door at ${atAnna}`);

await drag(ben, 'D-103', -100, -30);
await ben.mouse.up();
await ben.waitForTimeout(300);
const seen = await position(anna, 'D-103');
check(seen.join() === (await position(ben, 'D-103')).join(), "Anna's browser follows Ben's drag");

const toBen = await ben.evaluate(() => window.spare.latencies);
const toAnna = await anna.evaluate(() => window.spare.latencies);
console.log(`      Anna to Ben: ${stats(toBen)}`);
console.log(`      Ben to Anna: ${stats(toAnna)}`);
// Where the slowest edits spent their time, so a miss says which part to look at.
const journeys = [
  ...(await ben.evaluate(() => window.spare.journeys)),
  ...(await anna.evaluate(() => window.spare.journeys)),
];
const took = (j) => j.to_server + j.to_page + j.to_screen;
for (const journey of journeys.sort((a, b) => took(b) - took(a)).slice(0, 3)) {
  console.log(`      slowest: ${took(journey)} ms, ${JSON.stringify(journey)}`);
}
check(
  Math.max(...toBen, ...toAnna) < 200,
  'every edit was drawn in the other browser in under 200 ms',
);

if (process.env.STACK_KILL) {
  console.log('\n5. Killing and restarting the server mid-edit loses nothing');
  await drag(anna, 'D-102', 60, 40);
  execSync(process.env.STACK_KILL);
  await anna.waitForSelector('#saved[data-state="down"]');
  pass('Anna sees the connection drop, mid-drag');
  const target = await box(anna, 'D-102');
  await anna.mouse.move(target.x + 40, target.y + 40);
  await anna.mouse.up();
  execSync(process.env.STACK_START);
  await anna.waitForSelector('#saved[data-state="live"]');
  await ben.waitForSelector('#saved[data-state="live"]');
  await anna.waitForFunction(() => window.spare.pending.length === 0);
  pass("her page reconnects and sends what wasn't confirmed");
  await ben.waitForTimeout(500);
  const [a, b] = [await position(anna, 'D-102'), await position(ben, 'D-102')];
  check(a.join() === b.join(), `Ben's browser ends with the door where Anna left it, ${a}`);
  await anna.reload();
  await anna.waitForSelector('g.element[data-id="D-102"]');
  check((await position(anna, 'D-102')).join() === a.join(), 'and a fresh load agrees');
  // Anna's claim on the door outlives her reload: a claim lasts 30 s after her last edit, and the
  // server turns back anyone else's change to the door until then. Ben changes this door's type
  // further down, so Anna lets go of it the way a person does: she picks it, then clicks off it.
  const held = await box(anna, 'D-102');
  await anna.mouse.click(held.x + held.width / 2, held.y + held.height / 2);
  await anna.waitForFunction(() => window.spare.claims.get('D-102')?.person === 'anna');
  await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
  await ben.waitForFunction(() => !window.spare.claims.has('D-102'));
  pass('Anna lets go of the door, and Ben sees it free');
}

console.log('\n4. Arch v12 is shared, bridged and consumed, on the page');
await anna.click('#browser [data-act="stackshare"][data-a="arch-model"]');
await anna.waitForSelector('[data-package="arch-package-v12"]');
pass('Anna clicks Share and Arch package v12 appears in Shared');
await anna.click('[data-act="stackbridge"][data-a="arch-package-v12"]');
await anna.waitForSelector('#dlg-b [data-act="stackpick"]');
const choices = anna.locator('#dlg-b li');
check(
  await choices.filter({ hasText: 'Company 3' }).locator('button').isDisabled(),
  "Company 3 can't be chosen: it hasn't permitted the bridge",
);
await shot(anna, 'anna-bridge');
await choices.filter({ hasText: 'Company 2' }).locator('button').click();
await anna.waitForFunction(() =>
  document.getElementById('toast-m').textContent.includes('Bridged to Company 2'),
);
pass('Anna bridges it to Company 2');
await dan.reload();
await dan.waitForSelector('[data-incoming] [data-act="stacktake"]');
await dan.click('[data-act="stacktake"]');
const consumed = '[data-package="company-1.arch-package-v12"]';
await dan.waitForSelector(consumed);
pass('Dan takes it into Consumed');
await dan.click(consumed);
await dan.waitForSelector('g.element[data-id="D-101"]');
pass('and opens it in the plan as a fixed version');
await shot(dan, 'dan-consumed');

console.log('\n   Drawing and deleting');
const newWall = () =>
  [...document.querySelectorAll('g.element')].find((g) => /^W-[0-9a-f]{8}$/.test(g.dataset.id))
    ?.dataset.id;
await anna.click('#ribbon [data-act="dtool"][data-a="architecture:wall"]');
const area = await anna.locator('#pv-canvas').boundingBox();
await anna.mouse.click(area.x + area.width * 0.3, area.y + area.height * 0.55);
await anna.mouse.click(area.x + area.width * 0.6, area.y + area.height * 0.55);
await ben.waitForFunction(newWall);
pass("Anna's new wall appears in Ben's browser");
const wallId = await ben.evaluate(newWall);
await anna.keyboard.press('Escape');
await anna.click('#ribbon [data-act="dtool"][data-a="architecture:door"]');
const wallBox = await box(anna, wallId);
await anna.mouse.click(wallBox.x + wallBox.width / 2, wallBox.y + wallBox.height / 2);
await ben.waitForFunction(() =>
  [...document.querySelectorAll('g.element')].some((g) => /^D-[0-9a-f]{8}$/.test(g.dataset.id)),
);
pass('and so does a door placed on it');
const hosted = await ben.evaluate(() => {
  const door = [...window.spare.elements.values()].find((p) => /^D-[0-9a-f]{8}$/.test(p.el.id));
  return { id: door.el.id, face: door.el.references[0], along: door.el.props['@host.along'] };
});
check(
  hosted.face.element === wallId && hosted.face.kind === 'face' && /^length:q0:/.test(hosted.along),
  'hosted on a named face of that wall at an offset along it, not on the wall',
);
await anna.keyboard.press('Escape');
const wallBoxAgain = await box(anna, wallId);
await anna.mouse.click(wallBoxAgain.x + 20, wallBoxAgain.y + wallBoxAgain.height / 2);
await anna.waitForSelector('#ribbon >> text=Delete');
await anna.keyboard.press('Delete');
await anna.waitForFunction(() =>
  document.getElementById('toast-m').textContent.includes('still hosts 1 element'),
);
pass('a wall with a door on it is not deleted, and the server says why');
const doorBox = await box(anna, hosted.id);
await anna.mouse.click(doorBox.x + doorBox.width / 2, doorBox.y + doorBox.height / 2);
await anna.waitForSelector('#ribbon >> text=Delete');
await anna.keyboard.press('Delete');
await ben.waitForFunction(
  (id) => !document.querySelector(`g.element[data-id="${id}"]`),
  hosted.id,
);
const wallBoxLast = await box(anna, wallId);
await anna.mouse.click(wallBoxLast.x + 20, wallBoxLast.y + wallBoxLast.height / 2);
await anna.waitForSelector('#ribbon >> text=Delete');
await anna.keyboard.press('Delete');
await ben.waitForFunction(
  (id) => !document.querySelector(`g.element[data-id="${id}"]`),
  wallId,
);
pass("deleting the door and then the wall takes them off Ben's screen");

console.log('\n   The 3D view');
await anna.click('#browser [data-act="go"][data-a="3d"]');
await anna.waitForSelector('#iso-canvas canvas');
await anna.waitForTimeout(500);
await shot(anna, 'anna-3d');
pass('the shapes draw in 3D');

console.log('\n   Shapes built here and on the server');
await ben.waitForFunction(() => window.spare.checked > 0, null, { timeout: 15000 });
const compared = await ben.evaluate(() => ({
  checked: window.spare.checked,
  mismatches: window.spare.mismatches,
}));
console.log(`      Ben's page compared ${compared.checked} bodies with the server's`);
check(compared.mismatches.length === 0, "every body built in the browser matches the server's");
const building = await ben.evaluate(() => ({
  viewing: window.spare.viewing,
  engine: window.spare.model.edition.engine,
}));
check(!building.viewing, `Ben's page builds the model's shapes itself, on the edition ${building.engine}`);
if (process.env.STACK_GEOMETRY === 'kernel') {
  check(building.engine === 'construct.kernel', "on the geometry kernel's edition, with the kernel's own package");
}

console.log('\n   Parameters, through the edit log');
// No page yet defines parameters, so Ben's page sends the two edits a parameters dialog will.
await ben.evaluate(() => {
  const definition = { id: 'P-sill', name: 'Sill height', kind: 'length', holder: 'instance', choices: [] };
  window.spare.edit('parameter.define', { definition });
  window.spare.edit('parameter.bind', { parameter: 'P-sill', categories: [300], default: '0' });
});
await ben.waitForFunction(() => window.spare.pending.length === 0);
const doorAt = await box(ben, 'D-103');
await ben.mouse.click(doorAt.x + doorAt.width / 2, doorAt.y + doorAt.height / 2);
const sill = '#pv-side input[data-chg="stackparam"][data-a="P-sill"]';
await ben.waitForSelector(sill);
check((await ben.locator('#pv-side').textContent()).includes('Doors'), 'a selected door shows its category');
check((await ben.locator(sill).inputValue()) === '0 mm', 'and its parameters in units, the default until one is set');
// Typed and committed with Enter, the way a person does it: the field sends one change, and the page
// then lets go of the field. (A made-up change event on top would send a second one when the field
// loses focus, and an undo would then only take back that copy.)
const setSill = async (value) => {
  await ben.locator(sill).fill(value);
  await ben.locator(sill).press('Enter');
};
await setSill('3 kg');
await ben.waitForFunction(() => document.getElementById('toast-m').textContent.includes('Sill height'));
pass("a wrong unit is turned back with the server's reason");
await setSill('0.15 m');
await anna.waitForFunction(() => window.spare.elements.get('D-103')?.el.props['P-sill'] === 'length:q0:960000');
pass("Anna's browser has Ben's value through the edit log, stored as its parameter's text");
await ben.waitForFunction((field) => document.querySelector(field)?.value === '150 mm', sill);
pass('and Ben sees it in units');
await shot(ben, 'ben-parameters');

console.log('\n   Undo and redo, through the edit log');
await ben.waitForSelector('#qat [data-act="stackundo"]');
pass("Ben's Undo is ready: his last change is his to undo");
// Enter committed the value and the page let go of the field, so Ctrl+Z is the page's, not the field's.
await ben.waitForFunction(() => !document.activeElement?.closest('input,textarea'));
await ben.keyboard.press('Control+z');
await ben.waitForFunction(() => window.spare.pending.length === 0);
// What Ben's page saw of its undo: the state, the last toast and the newest lines of the edit log.
console.log(
  `      Ben's page after Ctrl+Z: ${JSON.stringify(
    await ben.evaluate(() => ({
      steps: window.spare.steps,
      toast: document.getElementById('toast-m').textContent,
      focus: document.activeElement?.tagName,
      log: window.spare.log.slice(0, 3),
    })),
  )}`,
);
await anna.waitForFunction(() => window.spare.elements.get('D-103')?.el.props['P-sill'] === undefined);
pass("Ctrl+Z undoes his value, and Anna's browser follows");
await ben.waitForSelector('#qat [data-act="stackredo"]');
await ben.click('#qat [data-act="stackredo"]');
await anna.waitForFunction(() => window.spare.elements.get('D-103')?.el.props['P-sill'] === 'length:q0:960000');
pass('his Redo button puts it back, for her too');
await ben.waitForSelector('#qat [data-tl="Redo"][aria-disabled="true"]');
pass('and then there is nothing to redo');
// A second change, taken back with the quick bar's Undo button this time.
await setSill('0.2 m');
await anna.waitForFunction(() => {
  const value = window.spare.elements.get('D-103')?.el.props['P-sill'];
  return value !== undefined && value !== 'length:q0:960000';
});
await ben.click('#qat [data-act="stackundo"]');
await anna.waitForFunction(() => window.spare.elements.get('D-103')?.el.props['P-sill'] === 'length:q0:960000');
pass("his Undo button takes back a second change, and Anna's browser follows");
// Anna's own last change was deleting the wall she drew: her undo brings it back, for Ben too.
await anna.click('#qat [data-act="stackundo"]');
await ben.waitForFunction((id) => document.querySelector(`g.element[data-id="${id}"]`), wallId);
pass("Anna's undo is her own: the wall she deleted is back on Ben's screen");
// Her next is deleting the door she had placed on it: undone, it is back on its wall.
await anna.click('#qat [data-act="stackundo"]');
await ben.waitForFunction(() =>
  [...window.spare.elements.keys()].some((id) => /^D-[0-9a-f]{8}$/.test(id)),
);
pass('her next undo puts the door back on its wall');
// Her next is placing the door. Ben takes the door first, so that undo is turned back.
const newDoor = await ben.evaluate(() =>
  [...window.spare.elements.keys()].find((id) => /^D-[0-9a-f]{8}$/.test(id)),
);
const claimedBox = await box(ben, newDoor);
await ben.mouse.click(claimedBox.x + claimedBox.width / 2, claimedBox.y + claimedBox.height / 2);
await anna.waitForFunction((id) => window.spare.claims.get(id)?.person === 'ben', newDoor);
await anna.click('#qat [data-act="stackundo"]');
await anna.waitForFunction(() =>
  document.getElementById('toast-m').textContent.includes('Ben is working on this'),
);
check(
  await anna.evaluate((id) => window.spare.elements.has(id), newDoor),
  "an undo of what someone else is working on is turned back with the server's reason, and changes nothing",
);
await shot(anna, 'anna-undo-turned-back');

console.log('\n   Types and parameters, on the page');
// Ben manages them through the page's own dialogs and palette; Anna reads what the server says, from
// her own sign-in, to see that it went through the log. Each step is then undone and redone once.
const model = '/api/projects/example-1/models/arch-model';
const asAnna = (path) => anna.evaluate((url) => fetch(url).then((r) => r.json()), `${model}/${path}`);
async function until(read, ok, label) {
  for (let tries = 0; tries < 60; tries += 1) {
    const found = await read();
    if (ok(found)) return found;
    await ben.waitForTimeout(100);
  }
  throw new Error(`FAILED: ${label}`);
}
const doorTypes = async () => (await asAnna('types?category=300')).types;
const leafWidth = (view) => view.parameters.find((p) => p.name === 'Leaf width');
const manager = '#stack-manage';
const closeDialog = async () => {
  await ben.locator('#dlg .dlg-f [data-act="dgclose"]').click();
  await ben.waitForSelector('#dlg', { state: 'hidden' });
};
const pick = async (id) => {
  await ben.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
  const at = await box(ben, id);
  await ben.mouse.click(at.x + at.width / 2, at.y + at.height / 2);
  await ben.waitForSelector('#pv-side [data-act="stacktypes"]');
};
const openTypes = async () => {
  await ben.locator('#pv-side [data-act="stacktypes"]').click();
  await ben.waitForSelector(`${manager} select[data-key="type-category"]`);
};
const typeSection = (name) =>
  ben.locator(`${manager} section[data-type]`).filter({ has: ben.locator(`input[value="${name}"]`) });
const problemIs = async (where, label) => {
  const shownText = await ben.locator(`${manager} [data-problem="${where}"]`).textContent();
  const toast = await ben.locator('#toast-m').textContent();
  check(shownText.length > 0 && toast === shownText, label);
};
// One undo and one redo of the step just made: `read` reads it from Anna's side.
async function undoRedo(label, read, without, withIt) {
  await ben.locator('#qat [data-act="stackundo"]:not([aria-disabled="true"])').click();
  await until(read, without, `${label}: the undo`);
  await ben.locator('#qat [data-act="stackredo"]:not([aria-disabled="true"])').click();
  await until(read, withIt, `${label}: the redo`);
  pass(`${label}: undo takes it back and redo puts it back, as Anna reads it`);
}

// Define a parameter and bind it to doors, from the Manage tab.
await ben.locator('#ribbon [role="tab"]', { hasText: 'Manage' }).click();
await ben.locator('#ribbon [data-act="stackparams"]').click();
await ben.waitForSelector(`${manager} [data-key="def-name"]`);
const catalog = await asAnna('catalog');
check(
  (await ben.locator(`${manager} [data-key="def-kind"] option`).count()) === catalog.kinds.length &&
    (await ben.locator(`${manager} [data-key^="def-cat:"]`).count()) ===
      catalog.categories.filter((c) => c.parent === null).length,
  "the kinds and the categories to bind to are the server's, not the page's",
);
await ben.locator(`${manager} [data-key="def-name"]`).fill('Leaf width');
await ben.locator(`${manager} [data-key="def-kind"]`).selectOption('length');
await ben.locator(`${manager} [data-key="def-holder"]`).selectOption('type');
await ben.locator(`${manager} [data-key="def-default"]`).fill('900');
await ben.locator(`${manager} [data-key="def-cat:300"]`).check();
await ben.locator(`${manager} [data-key="def-go"]`).click();
await ben.waitForFunction(
  (root) => document.querySelector(root).textContent.includes('Bound to Doors. Default: 900 mm.'),
  manager,
);
pass('Leaf width is defined and bound to doors with a default, and the list says so in units');
await ben.locator(`${manager} [data-key="def-name"]`).fill('Leaf width');
await ben.locator(`${manager} [data-key="def-go"]`).click();
await ben.waitForSelector(`${manager} [data-problem="define"]`);
await problemIs('define', "a second parameter of that name is refused, in the server's words, beside the form");
await closeDialog();
await ben.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
const leaf = async () =>
  (await asAnna('project-parameters')).parameters.find((p) => p.name === 'Leaf width');
await undoRedo(
  'Binding it',
  leaf,
  (found) => found && found.categories.length === 0,
  (found) => found && found.categories.length === 1 && found.default === '900 mm',
);
await ben.locator('#qat [data-act="stackundo"]:not([aria-disabled="true"])').click();
await until(leaf, (found) => found && found.categories.length === 0, 'the bind is back before its define');
await ben.locator('#qat [data-act="stackundo"]:not([aria-disabled="true"])').click();
await until(leaf, (found) => found === undefined, 'the define goes');
await ben.locator('#qat [data-act="stackredo"]:not([aria-disabled="true"])').click();
await ben.locator('#qat [data-act="stackredo"]:not([aria-disabled="true"])').click();
await until(leaf, (found) => found && found.categories.length === 1, 'both are back');
pass('Defining it: undo takes it away and redo brings it back, bound again');

// Types of doors: make, refuse a clash, set a value, rename, copy, delete.
await pick('D-103');
await openTypes();
check((await ben.locator(`${manager} [data-key="type-category"]`).inputValue()) === '300', "the dialog opens on the selected door's category");
const make = async (name) => {
  await ben.locator(`${manager} [data-key="new-type-name"]`).fill(name);
  await ben.locator(`${manager} [data-key="new-type-make"]`).click();
};
await make('Flush');
await ben.waitForSelector(`${manager} section[data-type]`);
pass('a type of doors is made from a name');
await make('Flush');
await ben.waitForSelector(`${manager} [data-problem="new-type"]`);
await problemIs('new-type', "a second type of that name is refused in the server's words, under the field");
await closeDialog();
await undoRedo('Making a type', doorTypes, (types) => types.length === 0, (types) => types.length === 1);

await openTypes();
const value = ben.locator(`${manager} section[data-type] input[data-key^="type-value:"]`).first();
check((await value.inputValue()) === '900 mm', "a new type shows the parameter's default, in units");
await value.fill('3 kg');
await value.press('Tab');
await ben.waitForSelector(`${manager} [data-problem^="type:"]`);
await problemIs(await ben.locator(`${manager} [data-problem^="type:"]`).getAttribute('data-problem'), "a wrong unit on a type is refused in the server's words, beside the value");
await value.fill('2.4 m');
await value.press('Tab');
await ben.waitForFunction((root) => document.querySelector(`${root} [data-key^="type-value:"]`)?.value === '2400 mm', manager);
pass('a value typed in metres is shown as the server formats it');
await closeDialog();
const flushValue = async () => leafWidth((await doorTypes())[0]).shown;
await undoRedo('Setting a value on a type', flushValue, (shown) => shown === '900 mm', (shown) => shown === '2400 mm');

await openTypes();
await ben.locator(`${manager} section[data-type] input[data-key^="type-name:"]`).fill('Flush door');
await ben.locator(`${manager} section[data-type] input[data-key^="type-name:"]`).press('Tab');
await ben.waitForFunction((root) => document.querySelector(`${root} [data-key^="type-name:"]`)?.value === 'Flush door', manager);
await closeDialog();
const names = async () => (await doorTypes()).map((t) => t.name);
await undoRedo('Renaming a type', names, (found) => found.join() === 'Flush', (found) => found.join() === 'Flush door');

await openTypes();
await ben.locator(`${manager} section[data-type] [data-key^="type-copy:"]`).click();
await ben.waitForFunction((root) => document.querySelectorAll(`${root} section[data-type]`).length === 2, manager);
await closeDialog();
await undoRedo('Copying a type', names, (found) => found.length === 1, (found) => found.length === 2);

await openTypes();
await typeSection('Flush door copy').locator('[data-key^="type-delete:"]').click();
await ben.waitForFunction((root) => document.querySelectorAll(`${root} section[data-type]`).length === 1, manager);
await closeDialog();
await undoRedo('Deleting a type', names, (found) => found.length === 2, (found) => found.length === 1);

// An element's type, from the palette; the type's value reaches it.
await ben.locator('#pv-side select[data-key="element-type"]').selectOption({ label: 'Flush door' });
const door = async (id) => asAnna(`parameters/${id}`);
await until(() => door('D-103'), (view) => view.type?.name === 'Flush door' && leafWidth(view).shown === '2400 mm' && leafWidth(view).from === 'type', 'D-103 has the type');
pass("a door's type is changed from the properties palette, and it reads the type's value");
await ben.waitForFunction(() => /Leaf width\s*2400 mm/.test(document.querySelector('#pv-side').textContent));
await ben.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await undoRedo('Changing a door’s type', () => door('D-103'), (view) => view.type === null, (view) => view.type?.name === 'Flush door');

// A type in use can't be deleted: the server says so. Then a change of value reaches every element of it.
await pick('D-102');
await ben.locator('#pv-side select[data-key="element-type"]').selectOption({ label: 'Flush door' });
await until(() => door('D-102'), (view) => view.type?.name === 'Flush door', 'D-102 has the type');
await openTypes();
check((await typeSection('Flush door').locator('.small.faint').first().textContent()) === '2 elements', 'the type says how many elements are of it');
await typeSection('Flush door').locator('[data-key^="type-delete:"]').click();
await ben.waitForSelector(`${manager} [data-problem^="type:"]`);
await problemIs(await ben.locator(`${manager} [data-problem^="type:"]`).getAttribute('data-problem'), "deleting a type that is in use is refused in the server's words, beside the type");
check((await doorTypes()).length === 1, 'and the type is still there');
const flush = ben.locator(`${manager} section[data-type] input[data-key^="type-value:"]`).first();
await flush.fill('2.1 m');
await flush.press('Tab');
await until(() => door('D-103'), (view) => leafWidth(view).shown === '2100 mm', 'D-103 reads the new value');
await until(() => door('D-102'), (view) => leafWidth(view).shown === '2100 mm', 'D-102 reads the new value');
pass('one value changed on the type, and both doors of that type read it');
await closeDialog();
await ben.waitForFunction(() => /Leaf width\s*2100 mm/.test(document.querySelector('#pv-side').textContent));
pass("and Ben's properties palette shows it without being asked");
await shot(ben, 'ben-types');

console.log('\n   Levels, on the page');
// Ben makes and changes levels in the Manage tab's dialog and draws on one; Anna reads what the server says.
const levels = async () => asAnna('levels');
const levelSection = (name) =>
  ben.locator(`${manager} section[data-level]`).filter({ has: ben.locator(`input[value="${name}"]`) });
await ben.locator('#ribbon [role="tab"]', { hasText: 'Manage' }).click();
await ben.locator('#ribbon [data-act="stacklevels"]').click();
await ben.waitForSelector(`${manager} [data-key="new-level-name"]`);
const listed = await levels();
check(
  (await ben.locator(`${manager} section[data-level]`).count()) === listed.length &&
    (await ben.locator(`${manager} [data-key="level-elevation:1"]`).inputValue()) === listed.find((l) => l.id === '1').shown,
  "the levels the old drawings stand on are listed, each elevation as the server formats it",
);
const makeLevel = async (name, elevation) => {
  await ben.locator(`${manager} [data-key="new-level-name"]`).fill(name);
  await ben.locator(`${manager} [data-key="new-level-elevation"]`).fill(elevation);
  await ben.locator(`${manager} [data-key="new-level-make"]`).click();
};
await makeLevel('Roof', '6.2 m');
await levelSection('Roof').waitFor();
check(
  (await levelSection('Roof').locator('[data-key^="level-elevation:"]').inputValue()) === '6200 mm',
  'a level is made from a name and an elevation typed in metres, shown in millimetres',
);
await makeLevel('roof', '7 m');
await ben.waitForSelector(`${manager} [data-problem="new-level"]`);
await problemIs('new-level', "a second level of that name, whatever its case, is refused in the server's words");
const roof = (await levels()).find((l) => l.name === 'Roof');
const roofIs = (ok) => async () => ok((await levels()).find((l) => l.id === roof.id));
const roofElevation = ben.locator(`${manager} [data-key="level-elevation:${roof.id}"]`);
await roofElevation.fill('7 m');
await roofElevation.press('Tab');
await until(roofIs((found) => found.shown === '7000 mm'), (moved) => moved, 'the roof moves up');
pass('a level moves by typing its elevation beside it');
await ben.locator(`${manager} [data-key="level-delete:1"]`).click();
await ben.waitForSelector(`${manager} [data-problem="level:1"]`);
await problemIs('level:1', "deleting a level that elements are tied to is refused with the server's count");
await levelSection('Roof').locator('[data-key^="level-show:"]').click();
await closeDialog();
await ben.waitForFunction(() => document.getElementById('h-view').textContent === 'Roof plan');
await ben.waitForFunction(() => !document.querySelector('g.element[data-id="D-101"]'));
pass("showing the roof shows its plan, without Level 1's doors");
await ben.locator('#ribbon [role="tab"]', { hasText: 'Architecture' }).click();
await ben.click('#ribbon [data-act="dtool"][data-a="architecture:wall"]');
const roofPlan = await ben.locator('#pv-canvas').boundingBox();
await ben.mouse.click(roofPlan.x + roofPlan.width * 0.3, roofPlan.y + roofPlan.height * 0.4);
await ben.mouse.click(roofPlan.x + roofPlan.width * 0.6, roofPlan.y + roofPlan.height * 0.4);
await ben.keyboard.press('Escape');
await until(roofIs((found) => found.tied === 1), (tied) => tied, 'the wall is on the roof');
pass("a wall drawn on the roof's plan is tied to the roof, as Anna reads it");
await undoRedo('Drawing on the roof', roofIs((found) => found.tied === 1), (tied) => !tied, (tied) => tied);
await ben.click('#browser [data-act="view"][data-a="l1"]');
await ben.waitForSelector('g.element[data-id="D-101"]');
check((await ben.locator('#h-view').textContent()) === 'Level 1 plan', 'and Level 1 plan shows Level 1 again');
await shot(ben, 'ben-levels');

const viewer = await signIn('anna', '**/shapes/**');
await viewer.waitForSelector('g.element[data-id="D-101"] .body');
check(await viewer.evaluate(() => window.spare.viewing), "a browser that can't build still draws the stored shapes");
await drag(viewer, 'D-101', 80, 0, 5);
await viewer.mouse.up();
check(
  (await viewer.evaluate(() => window.spare.pending.length)) === 0,
  'and sends no edits: it is a viewer',
);

console.log('\n   Workshop');
await anna.click('#b-disc');
await anna.click('#disc-menu [data-a="workshop"]');
await anna.frameLocator('#sc-workshop iframe').locator('#engine[data-engine="kernel"]').waitFor({ timeout: 30000 });
const workshop = anna.frames().find((f) => f.url().endsWith('/workshop/'));
const built = await workshop.evaluate(() => document.querySelector('#vinfo').textContent);
console.log(`      ${built}`);
check(/^6 bodies, 87,549\.28 mm³\. 2 features wait for the kernel\./.test(built), 'Workshop opens, and the geometry kernel builds its sample part in the browser');
await shot(anna, 'anna-workshop');

// Workshop's parts are elements of the model, through the same edit log as walls and doors.
const openWorkshop = async (page) => {
  await page.click('#b-disc');
  await page.click('#disc-menu [data-a="workshop"]');
  await page.frameLocator('#sc-workshop iframe').locator('#engine[data-engine="kernel"]').waitFor({ timeout: 30000 });
  return page.frames().find((f) => f.url().endsWith('/workshop/'));
};
const bracketWidth = (frame) =>
  frame.evaluate(() => window.WorkshopFrame.project().parts.find((p) => p.name === 'Bracket').params[0].value);
const storedWidth = (page) =>
  page.evaluate(() => {
    const part = window.spare.workshop.elements().find((e) => e.name === 'Bracket');
    return part ? part.recipe.args.params[0].value : null;
  });
const setWidth = async (frame, value) => {
  await frame.click('#left button.leaf:has-text("Bracket")');
  const field = frame.locator('#left input[data-chg="param"][data-a="0"]');
  await field.fill(String(value));
  await field.evaluate((el) => el.dispatchEvent(new Event('change', { bubbles: true })));
};
check(
  (await workshop.evaluate(() => window.WorkshopFrame.state().kept)) === 'sample',
  'a model with no parts shows the sample, kept nowhere until it is changed',
);
await setWidth(workshop, 150);
await anna.waitForFunction(() => window.spare.pending.length === 0 && window.spare.workshop.elements().length === 4);
check((await storedWidth(anna)) === 150, "the first change keeps the sample's three parts and its assembly in the model");
const benWorkshop = await openWorkshop(ben);
await ben.waitForFunction(() => window.spare.workshop.state().loaded);
check((await bracketWidth(benWorkshop)) === 150, "Ben's Workshop opens the parts Anna kept in the model");
await setWidth(workshop, 135.5);
await benWorkshop.waitForFunction(
  () => window.WorkshopFrame.project().parts.find((p) => p.name === 'Bracket').params[0].value === 135.5,
);
check((await storedWidth(ben)) === '135.5', 'and sees her next change live; a fraction is kept as decimal text');
await shot(ben, 'ben-workshop');
await anna.reload();
await anna.waitForSelector('#saved[data-state="live"]');
const again = await openWorkshop(anna);
await anna.waitForFunction(() => window.spare.workshop.state().loaded);
await again.waitForFunction(() => window.WorkshopFrame.state().kept === 'model');
check((await bracketWidth(again)) === 135.5, 'and after a reload the part comes back from the model');
await browser.close();
console.log('\nThe browser checks passed.');
