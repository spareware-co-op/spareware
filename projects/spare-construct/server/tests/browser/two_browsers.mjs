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
// The seed made the Arch model's wall types as Anna, and that wrote its parameters record (the
// walls' location line and flip). A claim lasts 30 s after a person's last edit, and nobody can let
// go of a parameters record, so Ben waits until hers has run out, as a person told "Anna is working
// on this" would, before he changes the record.
for (let tries = 0; ; tries += 1) {
  const left = await ben.evaluate(() =>
    fetch('/api/projects/example-1/models/arch-model/claims')
      .then((r) => r.json())
      .then((claims) => (claims.parameters ? claims.parameters.until_ms - Date.now() : 0)),
  );
  if (left <= 0) break;
  if (tries > 80) throw new Error("FAILED: Anna's claim on the parameters record never ran out");
  await ben.waitForTimeout(Math.min(left + 100, 1000));
}
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
// The undo that put the door back was Anna's edit, so the door is hers for 30 s and the server
// would turn Ben away. She lets go of it the way a person does: back in the plan she picks it,
// then clicks off it.
await anna.click('#browser [data-act="view"][data-a="l1"]');
const restored = await box(anna, newDoor);
await anna.mouse.click(restored.x + restored.width / 2, restored.y + restored.height / 2);
await anna.waitForFunction((id) => window.spare.claims.get(id)?.person === 'anna', newDoor);
await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await ben.waitForFunction((id) => !window.spare.claims.has(id), newDoor);
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

console.log('\n   Formulas, on the page');
// A parameter given by a formula of others: defined in the Project parameters dialog, worked out by
// the server wherever it is read, shown read-only with its formula, and following its inputs.
const openParams = async () => {
  await ben.locator('#ribbon [role="tab"]', { hasText: 'Manage' }).click();
  await ben.locator('#ribbon [data-act="stackparams"]').click();
  await ben.waitForSelector(`${manager} [data-key="def-name"]`);
};
const define = async (name, holder, { standard = '', formula = '' } = {}) => {
  await ben.locator(`${manager} [data-key="def-name"]`).fill(name);
  await ben.locator(`${manager} [data-key="def-kind"]`).selectOption('length');
  await ben.locator(`${manager} [data-key="def-holder"]`).selectOption(holder);
  await ben.locator(`${manager} [data-key="def-default"]`).fill(standard);
  await ben.locator(`${manager} [data-key="def-formula"]`).fill(formula);
  await ben.locator(`${manager} [data-key="def-cat:300"]`).check();
  await ben.locator(`${manager} [data-key="def-go"]`).click();
};
const parameterNamed = async (name) => (await asAnna('project-parameters')).parameters.find((p) => p.name === name);
// Once it is defined and bound, the page empties the form for the next one.
const formEmpty = () => ben.waitForFunction((root) => document.querySelector(`${root} [data-key="def-name"]`)?.value === '', manager);
await openParams();
await define('Jamb', 'type', { standard: '40' });
await until(() => parameterNamed('Jamb'), (found) => found && found.categories.length === 1, 'Jamb is bound');
await formEmpty();
await define('Clear', 'instance', { formula: '2*Jamb+820mm' });
const clearDef = await until(() => parameterNamed('Clear'), (found) => found && found.categories.length === 1, 'Clear is bound');
await formEmpty();
check(clearDef.formula === '2 * Jamb + 820 mm', 'a formula typed in the dialog is stored in its one spelling, by the server');
await ben.waitForFunction(
  ([root, id]) => document.querySelector(`${root} [data-formula="${id}"]`)?.textContent === 'Formula: 2 * Jamb + 820 mm',
  [manager, clearDef.id],
);
pass('and the dialog shows it');
await define('Bad', 'instance', { formula: 'Nope + 1 mm' });
await ben.waitForSelector(`${manager} [data-problem="define"]`);
check((await ben.locator(`${manager} [data-problem="define"]`).textContent()) === 'There is no parameter called Nope.', "a formula naming no parameter is refused in the server's words");
await problemIs('define', 'beside the form');
await closeDialog();

const clear = (view) => view.parameters.find((p) => p.name === 'Clear');
await until(() => door('D-103'), (view) => clear(view)?.shown === '900 mm' && clear(view).from === 'formula', 'D-103 reads its clear width');
pass("the server works the formula out for a door, as Anna reads it: 2 * 40 mm + 820 mm");
await pick('D-103');
await ben.waitForFunction((id) => document.querySelector(`#pv-side [data-formula="${id}"]`)?.textContent === '= 2 * Jamb + 820 mm', clearDef.id);
check((await ben.locator(`#pv-side input[data-a="${clearDef.id}"]`).count()) === 0, "Ben's palette shows the formula's value read-only, with its formula");

// An input the formula reads changes on the type: the value follows, for both of them.
const jambDef = await parameterNamed('Jamb');
await openTypes();
const jamb = typeSection('Flush door').locator(`input[data-key$=":${jambDef.id}"]`);
await jamb.fill('50');
await jamb.press('Enter');
await until(() => door('D-103'), (view) => clear(view).shown === '920 mm', 'the clear width follows the jamb');
pass('a value the formula reads, set on the type, reaches the formula of each door of it');
await closeDialog();
await ben.waitForFunction(() => /Clear\s*920 mm/.test(document.querySelector('#pv-side').textContent));
pass("and Ben's palette shows the new value without being asked");
await undoRedo('Setting a value a formula reads', () => door('D-103'), (view) => clear(view).shown === '900 mm', (view) => clear(view).shown === '920 mm');

// The formula itself changes in the dialog; one the server can't work out is refused beside it.
await openParams();
const formulaField = ben.locator(`${manager} [data-key="formula:${clearDef.id}"]`);
await formulaField.fill('2 * Jamb + 1 m');
await formulaField.press('Enter');
await until(() => door('D-103'), (view) => clear(view).shown === '1100 mm', 'the new formula reaches the door');
pass('a changed formula reaches every door that reads it');
await formulaField.fill('Jamb / 0');
await formulaField.press('Enter');
await ben.waitForSelector(`${manager} [data-problem="formula:${clearDef.id}"]`);
await problemIs(`formula:${clearDef.id}`, "a formula that can't be worked out is refused in the server's words, beside the formula");
check((await door('D-103').then(clear)).shown === '1100 mm', 'and changes nothing');
await closeDialog();
await undoRedo('Changing a formula', () => door('D-103'), (view) => clear(view).shown === '920 mm', (view) => clear(view).shown === '1100 mm');
await shot(ben, 'ben-formulas');

console.log('\n   Layered walls, of a chosen type');
// Anna draws a wall of a type she chooses in the wall tool's options, below the seed's floor where
// nothing else is; Ben sees it built of its type's layers, each in its material's colour. Her undo
// and redo of the draw take it away and back. Then Ben places a door on it, which cuts it on the
// kernel's edition, and changes its type's layers: both browsers build it again. Anna touches
// nothing of Ben's after that, and Ben nothing of hers before.
const onPlan = (page, x, y) =>
  page.evaluate(([mx, my]) => {
    const svg = document.querySelector('#pv-canvas svg');
    const at = svg.createSVGPoint();
    at.x = mx;
    at.y = my;
    const screen = at.matrixTransform(svg.getScreenCTM());
    return { x: screen.x, y: screen.y };
  }, [x, y]);
const layeredTypes = await asAnna('wall-types');
check(
  ['wall-brick-veneer-90', 'wall-concrete-200', 'wall-partition-stud-90'].every((id) => layeredTypes.types.some((t) => t.id === id)),
  "the seed's model has the default wall types, made through the log",
);
await anna.click('#ribbon [data-act="dtool"][data-a="architecture:wall"]');
const wallTypeField = '#pv-side select[data-key="wall-type"]';
const wallTopField = '#pv-side select[data-key="wall-top"]';
await anna.waitForSelector(wallTypeField);
await anna.waitForSelector(`${wallTopField} option[value="2"]`);
check(
  (await anna.locator(`${wallTypeField} option`).count()) === layeredTypes.types.length,
  "the wall tool offers the model's wall types, as the server lists them",
);
check((await anna.locator(wallTopField).inputValue()) === '2', 'and its top is the level above the plan, Level 2');
await anna.locator(wallTypeField).selectOption({ label: 'Concrete 200' });
await anna.waitForSelector('#pv-side [data-layers="wall-concrete-200"]');
check(
  (await anna.locator('#pv-side [data-layers="wall-concrete-200"] tr').count()) === 1,
  "Anna chooses Concrete 200, and the palette lists its one layer",
);
const layeredStart = await onPlan(anna, 2000, 9500);
const layeredEnd = await onPlan(anna, 5000, 9500);
await anna.mouse.click(layeredStart.x, layeredStart.y);
await anna.mouse.click(layeredEnd.x, layeredEnd.y);
const concreteWall = () =>
  [...window.spare.elements.values()].find((p) => p.el.recipe.op === 'wall.layered' && p.el.props['@type'] === 'wall-concrete-200')
    ?.el.id;
await ben.waitForFunction(concreteWall);
const layeredId = await ben.evaluate(concreteWall);
// How many parts a wall is drawn in: handed to the page as it is, so it takes its values as arguments.
const layeredParts = ([id, count]) =>
  document.querySelectorAll(`g.element[data-id="${id}"] polygon.part`).length === count;
await ben.waitForFunction(layeredParts, [layeredId, 1]);
pass("Anna's wall appears in Ben's browser, built of its type's one layer");
const layeredRecord = await ben.evaluate((id) => window.spare.elements.get(id).el, layeredId);
check(
  layeredRecord.recipe.args.layers.wall === 'type' &&
    layeredRecord.props['@level'] === '1' &&
    layeredRecord.props['@top.level'] === '2',
  'its record reads its layers from its type, its base tied to Level 1 and its top to Level 2',
);
const concreteMaterial = layeredTypes.materials.find((m) => m.id === 'concrete-regular');
check(
  (await ben.locator(`g.element[data-id="${layeredId}"] polygon.part`).getAttribute('style')) === `fill:${concreteMaterial.cut}`,
  "in its material's colour where the plan cuts it, as the server lists the material",
);
await shot(ben, 'ben-layered-wall');
await anna.keyboard.press('Escape');
await anna.waitForFunction(() => window.spare.steps.undo?.cmd === 'wall.draw');
await anna.click('#qat [data-act="stackundo"]:not([aria-disabled="true"])');
await ben.waitForFunction((id) => !document.querySelector(`g.element[data-id="${id}"]`), layeredId);
pass("Anna's undo takes the wall she drew away, in Ben's browser too");
await anna.click('#qat [data-act="stackredo"]:not([aria-disabled="true"])');
await ben.waitForFunction(layeredParts, [layeredId, 1]);
await anna.waitForFunction(layeredParts, [layeredId, 1]);
pass('and her redo puts it back, built again of its type, in both browsers');

// Ben places a door on it from his own plan.
await ben.locator('#ribbon [role="tab"]', { hasText: 'Architecture' }).click();
await ben.click('#ribbon [data-act="dtool"][data-a="architecture:door"]');
const layeredBox = await box(ben, layeredId);
await ben.mouse.click(layeredBox.x + layeredBox.width / 2, layeredBox.y + layeredBox.height / 2);
const doorOnIt = (id) =>
  [...window.spare.elements.values()].find((p) => p.el.references?.[0]?.element === id)?.el.id;
await ben.waitForFunction(doorOnIt, layeredId);
const hostedDoor = await ben.evaluate(doorOnIt, layeredId);
await ben.keyboard.press('Escape');
await anna.waitForFunction((id) => document.querySelector(`g.element[data-id="${id}"] .body`), hostedDoor);
pass('a door Ben places on the layered wall is hosted on it, in both browsers');
if (process.env.STACK_GEOMETRY === 'kernel') {
  const cutBy = (names) => names.some((n) => n.role === `tool(${hostedDoor}).head`);
  await until(
    async () => (await asAnna(`shape/${layeredId}`)).built.result.shape?.names ?? [],
    cutBy,
    'the server cuts the wall for the door',
  );
  await anna.waitForFunction(
    ([wall, cut]) => window.spare.elements.get(wall)?.shape?.names?.some((n) => n.role === `tool(${cut}).head`),
    [layeredId, hostedDoor],
  );
  pass("the door cuts the wall's layers, on the server and as Anna's page builds it");
} else {
  pass("(the stand-in cuts no openings: the kernel's run checks the door's cut)");
}

// No page edits a wall type's layers yet, so Ben's page sends the edit a layers editor will: a
// 12.5 mm lining either side of the concrete.
const layeredHash = await anna.evaluate((id) => window.spare.elements.get(id).hash, layeredId);
const lining = { function: 'finish', thickness: '12.5 mm', material: 'plasterboard-gypsum', wraps: true };
const concreteLayers = layeredTypes.types.find((t) => t.id === 'wall-concrete-200').assembly.layers;
const linedConcrete = { core: { exterior: 1, interior: 2 }, layers: [lining, ...concreteLayers, lining] };
await ben.evaluate((assembly) => window.spare.edit('wall.type.layers', { type: 'wall-concrete-200', assembly }), linedConcrete);
await ben.waitForFunction(layeredParts, [layeredId, 3]);
await anna.waitForFunction(layeredParts, [layeredId, 3]);
pass("Ben changes the type's layers, and both browsers build the wall again of its three");
check(
  (await anna.evaluate((id) => window.spare.elements.get(id).hash, layeredId)) === layeredHash,
  'and no wall was written: it reads its type',
);
await until(
  async () => (await asAnna('wall-types')).types.find((t) => t.id === 'wall-concrete-200').thickness,
  (thickness) => thickness === '225 mm',
  'the type is 225 mm thick',
);
await ben.waitForTimeout(1500);
for (const [name, page] of [['Anna', anna], ['Ben', ben]]) {
  const mismatched = await page.evaluate(() => window.spare.mismatches);
  check(mismatched.length === 0, `every body ${name}'s page built, the layered wall's included, matches the server's`);
}
await shot(anna, 'anna-layered-wall');

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

console.log('\n   Grids, on the page');
// The seeded grids stand in the site setout, which the Arch model links as a shared package: both
// browsers draw them, seen and never picked. Ben then draws, undoes and redoes, renames, moves and
// deletes grids of the Arch model itself, and Anna's browser follows each step. Anna changes none of
// them, so no claim of Ben's is ever in her way.
const toArchitecture = async (page) => {
  await page.click('#b-disc');
  await page.click('#disc-menu [data-a="architecture"]');
  await page.waitForSelector('#pv-canvas g.element[data-id="D-101"]');
};
await toArchitecture(anna);
await toArchitecture(ben);
const seededGrids = '#pv-canvas g.datum.linked[data-kind="grid"]';
const seededNames = (page) =>
  page.locator(seededGrids).evaluateAll((groups) => groups.map((g) => g.dataset.name).sort().join());
for (const page of [anna, ben]) {
  await page.waitForFunction((at) => document.querySelectorAll(at).length === 5, seededGrids);
}
check(
  (await seededNames(anna)) === '1,2,A,B,C' && (await seededNames(ben)) === '1,2,A,B,C',
  "the site setout's five grids are drawn in both browsers",
);
check(
  (await anna.locator(`${seededGrids}[data-name="A"] circle.bub`).count()) === 1 &&
    (await anna.locator(`${seededGrids}[data-name="A"] text.tn`).textContent()) === 'A',
  'each a chain line with its name in a bubble',
);
await shot(anna, 'anna-grids');

// A point of the model on a page's screen, through the plan's own drawing.
const onScreen = (page, x, y) =>
  page.evaluate(([mx, my]) => {
    const svg = document.querySelector('#pv-canvas svg');
    const point = svg.createSVGPoint();
    point.x = mx;
    point.y = my;
    const at = point.matrixTransform(svg.getScreenCTM());
    return [at.x, at.y];
  }, [x, y]);
const clickModel = async (page, x, y) => {
  const [sx, sy] = await onScreen(page, x, y);
  await page.mouse.click(sx, sy);
};
const ownGrids = '#pv-canvas g.datum.own[data-kind="grid"]';
const ownGrid = (name) => `${ownGrids}[data-name="${name}"]`;
const gridGroup = (id) => `#pv-canvas g.datum[data-datum="${id}"]`;
const gridName = '#pv-side input[data-key="datum-name"]';
const drawGrid = async (from, to) => {
  await ben.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
  await ben.locator('#ribbon [role="tab"]', { hasText: 'Architecture' }).click();
  await ben.click('#ribbon [data-act="dtool"][data-a="architecture:grid"]');
  await clickModel(ben, ...from);
  await clickModel(ben, ...to);
  await ben.keyboard.press('Escape');
};
// Ben picks a grid by its bubble, the way a person does, after clicking off whatever he had.
const pickGrid = async (id) => {
  await ben.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
  await ben.waitForSelector(`${gridGroup(id)} circle.bub`);
  const bubble = await ben.locator(`${gridGroup(id)} circle.bub`).boundingBox();
  await ben.mouse.click(bubble.x + bubble.width / 2, bubble.y + bubble.height / 2);
  await ben.waitForSelector(`${gridGroup(id)}[data-picked="yes"]`);
  await ben.waitForSelector(gridName);
};

// Left of the building, top to bottom, clear of the zoom buttons at the top right: the bubble
// goes at the top, where it starts.
await drawGrid([-1000, -1000], [-1000, 9000]);
await anna.waitForSelector(ownGrid('1'));
pass("Ben draws a grid with the grid tool, and Anna's browser draws it with the server's first name, 1");
const first = await anna.locator(ownGrid('1')).getAttribute('data-datum');
await ben.waitForSelector(ownGrid('1'));
check(
  (await ben.locator(ownGrid('1')).getAttribute('data-datum')) === first,
  'the same grid in both browsers',
);

await ben.waitForFunction(() => window.spare.pending.length === 0 && window.spare.steps.undo?.cmd === 'grid.create');
await ben.locator('#qat [data-act="stackundo"]:not([aria-disabled="true"])').click();
await anna.waitForFunction((at) => !document.querySelector(at), gridGroup(first));
pass("Ben's undo takes the grid away, and Anna's browser follows");
await ben.locator('#qat [data-act="stackredo"]:not([aria-disabled="true"])').click();
await anna.waitForSelector(`${gridGroup(first)}[data-name="1"]`);
pass('his redo puts it back, with its name, for her too');

await pickGrid(first);
await ben.locator(gridName).fill('A');
await ben.locator(gridName).press('Enter');
await anna.waitForSelector(`${gridGroup(first)}[data-name="A"]`);
pass("Ben renames it A in the properties palette, and Anna's browser shows the new name");
// Ben's page names the next grid after the one he last named, once the server's answer is in hand.
await ben.waitForSelector(`${gridGroup(first)}[data-name="A"]`);

// Along the bottom, left to right: the server names it after the grid Ben last named.
await drawGrid([0, 9800], [12000, 9800]);
await anna.waitForSelector(ownGrid('B'));
pass('his next grid is given the next name by the server, B, in both browsers');
const second = await anna.locator(ownGrid('B')).getAttribute('data-datum');

await pickGrid(second);
await ben.locator(gridName).fill('A');
await ben.locator(gridName).press('Enter');
const refusal = `#pv-side [data-problem="datum:${second}"]`;
await ben.waitForSelector(refusal);
const refused = await ben.locator(refusal).textContent();
check(
  refused.length > 0 && refused === (await ben.locator('#toast-m').textContent()),
  "a second grid named A is refused in the server's words, beside its name",
);
check((await anna.locator(`${gridGroup(second)}[data-name="B"]`).count()) === 1, 'and B keeps its name');

// A's far end, dragged by its handle from 9000 up to 8000.
await pickGrid(first);
const handle = await ben.locator(`${gridGroup(first)} [data-grid-end="1"]`).boundingBox();
const [hx, hy] = [handle.x + handle.width / 2, handle.y + handle.height / 2];
const [tx, ty] = await onScreen(ben, -1000, 8000);
await ben.mouse.move(hx, hy);
await ben.mouse.down();
for (let i = 1; i <= 10; i += 1) {
  await ben.mouse.move(hx + ((tx - hx) * i) / 10, hy + ((ty - hy) * i) / 10);
  await ben.waitForTimeout(16);
}
await ben.mouse.up();
await anna.waitForFunction((at) => document.querySelector(at)?.dataset.y1 === '8000', gridGroup(first));
const ends = (page) => page.locator(gridGroup(first)).evaluate((g) => [g.dataset.x0, g.dataset.y0, g.dataset.x1, g.dataset.y1].join());
check((await ends(anna)) === '-1000,-1000,-1000,8000', "Ben drags A's end, and Anna's browser draws it where he left it");
await ben.waitForFunction((at) => document.querySelector(at)?.dataset.y1 === '8000', gridGroup(first));
check((await ends(ben)) === (await ends(anna)), "and Ben's shows the server's answer, the same");

await pickGrid(second);
await ben.waitForSelector('#ribbon >> text=Delete');
await ben.waitForFunction(() => !document.activeElement?.closest('input,textarea,select'));
await ben.keyboard.press('Delete');
await anna.waitForFunction((at) => !document.querySelector(at), gridGroup(second));
pass("Ben deletes B with the Delete key, and it goes from Anna's browser");
check(
  (await anna.locator(ownGrids).count()) === 1 && (await anna.locator(seededGrids).count()) === 5,
  'A stays, and so do the seeded grids',
);
await shot(ben, 'ben-grids');

console.log('\n   Floors, from a boundary');
// Anna draws an L-shaped floor with a hole, of a type she chooses in the floor tool's options, south
// of the building where nothing else is; Ben sees it, its hole drawn from its record. A boundary
// that crosses itself is turned back with the server's reason, beside the tool. Her undo and redo
// of the draw take it away and put it back. Then she picks it, reads its area in units, and lets go
// of it; only then does Ben pick it, read the same, and delete it, so the plan ends as it began.
const floorHome = async (page) => {
  await page.click('#b-disc');
  await page.click('#disc-menu [data-a="architecture"]');
  await page.waitForSelector('g.element[data-id="D-101"] .body');
  await page.locator('#ribbon [role="tab"]', { hasText: 'Architecture' }).click();
};
await floorHome(anna);
await floorHome(ben);
check((await asAnna('floor-types')).types.length === 0, 'the seed has no floor types yet');
await anna.click('#ribbon [data-act="dtool"][data-a="architecture:floor"]');
const floorTypeField = '#pv-side select[data-key="floor-type"]';
await anna.waitForSelector(`${floorTypeField} option[value="floor-concrete-150-screed-50"]`);
const floorTypesMade = await asAnna('floor-types');
check(
  floorTypesMade.types.length === 3 && (await anna.locator(`${floorTypeField} option`).count()) === 3,
  'the floor tool has the server make its default floor types through the log, and offers them',
);
await anna.locator(floorTypeField).selectOption({ label: 'Concrete 150, 50 screed' });
await anna.waitForSelector('#pv-side [data-floor-layers="floor-concrete-150-screed-50"]');
check(
  (await anna.locator('#pv-side [data-floor-layers="floor-concrete-150-screed-50"] tr').count()) === 2,
  'Anna chooses Concrete 150, 50 screed, and the palette lists its two layers, top first',
);
// Corners in the model's millimetres, clicked where the plan draws them.
const floorClicks = async (page, corners) => {
  for (const [x, y] of corners) {
    const at = await onPlan(page, x, y);
    await page.mouse.click(at.x, at.y);
  }
};
const floorEdge = [[7000, -2300], [10000, -2300], [10000, -1300], [8500, -1300], [8500, -500], [7000, -500]];
const floorHole = [[7500, -2000], [8000, -2000], [8000, -1600], [7500, -1600]];
await floorClicks(anna, [...floorEdge, floorEdge[0]]);
await anna.waitForSelector('#pv-side [data-floor-sketch="closed"]');
await floorClicks(anna, [...floorHole, floorHole[0]]);
await anna.waitForFunction(() => /1 hole/.test(document.querySelector('#pv-side [data-floor-sketch="closed"]')?.textContent ?? ''));
pass('her edge closes on its first corner, and a hole inside it on its own');
await anna.keyboard.press('Enter');
const floorOf = () => [...window.spare.elements.values()].find((p) => p.el.kind === 'floor')?.el.id;
await ben.waitForFunction(floorOf);
const floorId = await ben.evaluate(floorOf);
const floorDrawn = (id) => document.querySelector(`g.element[data-id="${id}"] path.floor[data-holes="1"]`);
await ben.waitForFunction(floorDrawn, floorId);
pass("Anna's floor appears in Ben's browser, its hole drawn from its record");
const floorRecord = await ben.evaluate((id) => window.spare.elements.get(id).el, floorId);
check(
  floorRecord.recipe.op === 'floor' &&
    floorRecord.recipe.args.layers.type === 'layers' &&
    floorRecord.props['@type'] === 'floor-concrete-150-screed-50' &&
    floorRecord.props['@level'] === '1',
  'its record reads its layers from the type she chose, its top tied to Level 1',
);
await shot(ben, 'ben-floor');

// A boundary that crosses itself: the server's reason, beside the tool, and the loop it turned back.
await floorClicks(anna, [[11000, -2300], [12500, -800], [12500, -2300], [11000, -800]]);
await anna.keyboard.press('Enter');
await anna.waitForSelector('#pv-side [data-floor-sketch="closed"]');
await anna.keyboard.press('Enter');
await anna.waitForSelector('#pv-side [data-problem="floor-draw"]');
const floorRefusal = await anna.locator('#pv-side [data-problem="floor-draw"]').textContent();
check(
  /'s boundary: the outer loop crosses or touches itself/.test(floorRefusal) &&
    floorRefusal === (await anna.locator('#toast-m').textContent()),
  "a boundary whose edges cross is turned back in the server's words, beside the floor tool",
);
await anna.waitForSelector('#pv-canvas [data-floor-refused]');
check(
  (await anna.evaluate(() => [...window.spare.elements.values()].filter((p) => p.el.kind === 'floor').length)) === 1,
  'drawn where it was turned back, and no second floor made',
);
await shot(anna, 'anna-floor-refused');

await anna.keyboard.press('Escape');
await anna.waitForFunction(() => window.spare.steps.undo?.cmd === 'floor.create');
await anna.click('#qat [data-act="stackundo"]:not([aria-disabled="true"])');
await ben.waitForFunction((id) => !document.querySelector(`g.element[data-id="${id}"]`), floorId);
pass("Anna's undo takes the floor she drew away, in Ben's browser too");
await anna.click('#qat [data-act="stackredo"]:not([aria-disabled="true"])');
await ben.waitForFunction(floorDrawn, floorId);
await anna.waitForFunction(floorDrawn, floorId);
pass('and her redo puts it back, in both browsers');

// Its area and what else the server works out of it, in units, apart from what a person sets.
const floorInside = async (page) => onPlan(page, 9500, -1800);
const floorShown = await until(
  () => asAnna(`parameters/${floorId}`),
  (view) => view.parameters?.some((p) => p.id === 'floor.area' && p.from === 'read-only'),
  'the server works out the area of the floor',
);
const floorArea = floorShown.parameters.find((p) => p.id === 'floor.area').shown;
check(/^[0-9.]+ m²$/.test(floorArea), `the server gives its area in the document's units, ${floorArea}`);
const floorAreaIs = (shown) => document.querySelector('#pv-side [data-read-only="floor.area"]')?.textContent === shown;
const floorAtAnna = await floorInside(anna);
await anna.mouse.click(floorAtAnna.x, floorAtAnna.y);
await anna.waitForFunction(floorAreaIs, floorArea);
check(
  (await anna.locator('#pv-side [data-read-only-rows] input').count()) === 0 &&
    (await anna.locator('#pv-side [data-floor-offset]').textContent()) === '0 mm',
  "Anna's palette shows it read-only, apart from the floor's type, level and offset",
);
await shot(anna, 'anna-floor-area');
// Picking it claimed it for her, as her redo had: Ben waits until she lets go.
await ben.waitForFunction((id) => window.spare.claims.get(id)?.person === 'anna', floorId);
await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await ben.waitForFunction((id) => !window.spare.claims.has(id), floorId);
pass('Anna lets go of the floor, and Ben sees it free');
const floorAtBen = await floorInside(ben);
await ben.mouse.click(floorAtBen.x, floorAtBen.y);
await ben.waitForFunction(floorAreaIs, floorArea);
pass('Ben picks it and reads the same area in his palette');
await ben.waitForSelector('#ribbon >> text=Delete');
await ben.keyboard.press('Delete');
await anna.waitForFunction((id) => !document.querySelector(`g.element[data-id="${id}"]`), floorId);
pass("Ben deletes it, and it is gone from Anna's plan");
await ben.waitForTimeout(1500);
for (const [name, page] of [['Anna', anna], ['Ben', ben]]) {
  const mismatched = await page.evaluate(() => window.spare.mismatches);
  check(mismatched.length === 0, `every body ${name}'s page built, the floor's included, matches the server's`);
}
await browser.close();
console.log('\nThe browser checks passed.');
