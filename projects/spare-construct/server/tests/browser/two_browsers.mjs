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

// A drawn thing's box on screen. A redraw (after Escape, a claim, an edit, an undo or a redo)
// replaces the drawing, and boundingBox() is null while it is away, so wait until it is drawn
// again (a slow runner caught a wall away once, between Escape and the click on it, and a grid's
// bubble away once, just after a redo). Never read boundingBox() of anything in the drawing
// directly: go through here.
async function boxOf(drawn, what) {
  for (let tries = 0; tries < 20; tries += 1) {
    await drawn.waitFor({ state: 'visible' });
    const found = await drawn.boundingBox();
    if (found) return found;
  }
  throw new Error(`${what} never stayed drawn long enough to find its box`);
}
const box = (page, id) => boxOf(page.locator(`g.element[data-id="${id}"] .body`), id);
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
if (compared.mismatches.length > 0) {
  console.log(`      bodies that differ: ${JSON.stringify(compared.mismatches).slice(0, 4000)}`);
}
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
const doorTypes = async () => {
  const got = await anna.evaluate(
    (url) => fetch(url).then(async (r) => ({ status: r.status, text: await r.text() })),
    `${model}/types?category=300`,
  );
  let body = null;
  try {
    body = JSON.parse(got.text);
  } catch {
    // Not JSON: the message below shows what came back.
  }
  if (!body || !Array.isArray(body.types)) {
    throw new Error(`types?category=300 answered HTTP ${got.status}: ${got.text.slice(0, 500)}`);
  }
  return body.types;
};
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
// An option of a closed select is never visible: it is waited for once it is in the page.
await anna.waitForSelector(`${wallTopField} option[value="2"]`, { state: 'attached' });
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
  const bubble = await boxOf(ben.locator(`${gridGroup(id)} circle.bub`), `grid ${id}'s bubble`);
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
const handle = await boxOf(ben.locator(`${gridGroup(first)} [data-grid-end="1"]`), "A's handle");
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
await anna.waitForSelector(`${floorTypeField} option[value="floor-concrete-150-screed-50"]`, { state: 'attached' });
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

console.log('\n   Views, in the project browser');
// Anna opens Level 1's plan from the browser's list of the model's views and the server draws it;
// Ben draws a wall, and the open plan shows it; Anna makes a plan view and renames it, and Ben's
// list follows each step; a fresh load agrees. A server on the geometry kernel draws the plan (the
// stand-in's has no cut), so a server without it says so in the plan pane instead, in its words.
const withKernel = process.env.STACK_GEOMETRY === 'kernel';
const viewLeaf = (name) => `#browser [data-act="stackview"]:has(span:text-is("${name}"))`;
await toArchitecture(anna);
await toArchitecture(ben);
await ben.click('#browser [data-act="view"][data-a="l1"]');
await anna.waitForSelector(viewLeaf('Level 1 plan'));
await ben.waitForSelector(viewLeaf('Level 1 plan'));
check(
  (await anna.locator('#browser [data-act="stackview"]').allTextContents()).join('|').includes('Level 2 plan'),
  "the browser lists the model's views, one plan for each level",
);
await anna.click(viewLeaf('Level 1 plan'));
if (withKernel) {
  await anna.waitForSelector('#pv-canvas g.drawn path[data-material]', { state: 'attached' });
  await anna.waitForSelector('#pv-canvas g.drawn line[data-element="W-101"]', { state: 'attached' });
  check(
    (await anna.locator('#pv-canvas g.drawn line[data-role]').count()) > 8,
    "Anna opens Level 1's plan and the server's drawing shows cut lines and hatches",
  );
} else {
  await anna.waitForSelector('#pv-canvas .stack-plan-note.bad');
  check(
    /without the geometry kernel/.test(await anna.locator('#pv-canvas .stack-plan-note').textContent()),
    'a server without the kernel says in the plan pane that it cannot draw the plan',
  );
}
await shot(anna, 'anna-open-view');

// Ben draws a wall on Level 1 at a place nothing stands; it appears in Anna's open plan.
const wallsOf = (page) =>
  page.evaluate(() => [...window.spare.elements.values()].filter((p) => p.el.recipe.op === 'wall.layered').map((p) => p.el.id));
const wallsBefore = await wallsOf(ben);
await ben.locator('#ribbon [role="tab"]', { hasText: 'Architecture' }).click();
await ben.click('#ribbon [data-act="dtool"][data-a="architecture:wall"]');
const viewWallStart = await onPlan(ben, 2000, 10000);
const viewWallEnd = await onPlan(ben, 4500, 10000);
await ben.mouse.click(viewWallStart.x, viewWallStart.y);
await ben.mouse.click(viewWallEnd.x, viewWallEnd.y);
await ben.keyboard.press('Escape');
const viewsNewWall = async (page) => {
  for (let tries = 0; tries < 100; tries += 1) {
    const found = (await wallsOf(page)).find((id) => !wallsBefore.includes(id));
    if (found) return found;
    await page.waitForTimeout(100);
  }
  throw new Error("FAILED: Ben's new wall never reached the page");
};
const viewWallId = await viewsNewWall(anna);
if (withKernel) {
  await anna.waitForSelector(`#pv-canvas g.drawn line[data-element="${viewWallId}"]`, { state: 'attached' });
  pass("Ben draws a wall and Anna's open plan shows it, as the server draws it");
} else {
  pass("Ben draws a wall and it reaches Anna's plan");
}
await shot(anna, 'anna-view-with-ben-wall');

// Anna makes a plan view; Ben's list shows it.
await anna.locator('#browser [data-key="new-view-name"]').fill('Ground floor plan');
await anna.locator('#browser [data-act="stacknewview"]').click();
await ben.waitForSelector(viewLeaf('Ground floor plan')).catch(async (error) => {
  const said = await anna.locator('#browser').innerText();
  throw new Error(`${error.message}
Anna's browser: ${said}
views: ${JSON.stringify(await ben.evaluate(() => window.spare.views))}`);
});
await anna.waitForSelector(viewLeaf('Ground floor plan'));
pass("Anna makes a plan view in the browser and Ben's list shows it");
const madeView = await ben.evaluate(() => window.spare.views.find((v) => v.name === 'Ground floor plan'));
check(madeView.kind === 'floor_plan' && madeView.level === '1', 'a floor plan of Level 1, made by the view command');

// Anna opens it and renames it by typing the new name and pressing Enter.
await anna.click(viewLeaf('Ground floor plan'));
const renameField = `#browser input[data-key="view-name:${madeView.id}"]`;
await anna.waitForSelector(renameField);
await anna.locator(renameField).fill('Level 1 working plan');
await anna.locator(renameField).press('Enter');
await ben.waitForSelector(viewLeaf('Level 1 working plan'));
await anna.waitForSelector(viewLeaf('Level 1 working plan'));
check(
  (await ben.locator(viewLeaf('Ground floor plan')).count()) === 0,
  "Anna renames it and Ben sees the new name, not the old",
);

// A fresh load agrees with both.
const fresh = await signIn('anna');
await fresh.waitForSelector(viewLeaf('Level 1 working plan'));
const freshNames = await fresh.evaluate((url) => fetch(url).then((r) => r.json()).then((list) => list.map((v) => v.name)), `${model}/views`);
check(
  freshNames.join('|') === 'Level 1 plan|Level 1 working plan|Level 2 plan',
  'a fresh load lists the same views, by name',
);
await fresh.context().close();
console.log('\n   Roofs, on a rectangle');
// Anna makes a gable roof over the seed building, of a type she chooses in the roof tool's options;
// Ben sees it, its ridge drawn from its record. A roof the server refuses (hips) is turned back in the
// server's words beside the tool, and nothing is drawn wrongly. She picks the roof, reads its area,
// and lets go of it; only then does Ben pick it and change its pitch, and Anna sees the new pitch
// and slope area. Ben lets go; Anna deletes the roof and undoes the delete, and a fresh load of
// Ben's page agrees.
const roofHome = async (page) => {
  await page.click('#b-disc');
  await page.click('#disc-menu [data-a="architecture"]');
  await page.waitForSelector('g.element[data-id="D-101"] .body');
  await page.locator('#ribbon [role="tab"]', { hasText: 'Architecture' }).click();
};
await roofHome(anna);
await roofHome(ben);
await anna.click('#ribbon [data-act="dtool"][data-a="architecture:roof"]');
const roofTypeField = '#pv-side select[data-key="roof-type"]';
await anna.waitForSelector(`${roofTypeField} option[value="roof-steel-rafter-150"]`, { state: 'attached' });
check(
  (await asAnna('roof-types')).types.length === 2 && (await anna.locator(`${roofTypeField} option`).count()) === 2,
  'the roof tool has the server make its default roof types through the log, and offers them',
);
await anna.locator(roofTypeField).selectOption({ label: 'Sheet steel on 150 rafters' });
await anna.waitForSelector('#pv-side [data-roof-layers="roof-steel-rafter-150"]');
check(
  (await anna.locator('#pv-side [data-roof-layers="roof-steel-rafter-150"] tr').count()) === 3 &&
    (await anna.locator('#pv-side select[data-key="roof-form"]').inputValue()) === 'gable',
  'Anna chooses the steel roof type, and the palette lists its three layers, outside first, and a gable',
);
const roofClicks = async (page, corners) => {
  for (const [x, y] of corners) {
    const at = await onPlan(page, x, y);
    await page.mouse.click(at.x, at.y);
  }
};
const roofFootprint = [[0, 0], [12000, 0], [12000, 8000], [0, 8000]];
await roofClicks(anna, [...roofFootprint, roofFootprint[0]]);
await anna.waitForSelector('#pv-side [data-roof-sketch="closed"]');
await anna.keyboard.press('Enter');
const roofOf = () => [...window.spare.elements.values()].find((p) => p.el.kind === 'roof')?.el.id;
await ben.waitForFunction(roofOf);
const roofId = await ben.evaluate(roofOf);
const roofDrawn = (id) =>
  document.querySelector(`g.element[data-id="${id}"] path.roof[data-form="gable"]`) &&
  document.querySelector(`g.element[data-id="${id}"] [data-roof-ridge]`);
await ben.waitForFunction(roofDrawn, roofId);
pass("Anna's gable roof appears in Ben's browser, its ridge drawn from its record");
const roofRecord = await ben.evaluate((id) => window.spare.elements.get(id).el, roofId);
check(
  roofRecord.recipe.op === 'roof' &&
    roofRecord.recipe.args.layers.type === 'layers' &&
    roofRecord.props['@type'] === 'roof-steel-rafter-150' &&
    roofRecord.props['@level'] === '1' &&
    roofRecord.recipe.args.edges.filter((edge) => edge.pitch).length === 2,
  'its record reads its layers from the type she chose, its base tied to Level 1, two edges rising',
);
await shot(ben, 'ben-roof');

const roofRead = (view, id) => view.parameters.find((p) => p.id === id)?.shown;
// A hip roof: Anna chooses the form Hip, leaves the edges it rises from blank (every edge), and
// sketches a second footprint clear of the gable. Both browsers draw its ridge and its four hips from
// the lines the server gave; the server's quantities read it; her undo takes it away from both and her
// redo brings it back; a pitch of its own on one edge, which would meet its neighbours at a hip at two
// pitches, is turned back by name; and a last undo leaves the plan as the gable alone.
await anna.locator('#pv-side select[data-key="roof-form"]').selectOption('hip');
await anna.waitForFunction(() => document.querySelector('#pv-side input[data-key="roof-eaves"]') !== null);
await roofClicks(anna, [[7000, -2300], [10000, -2300], [10000, -500], [7000, -500], [7000, -2300]]);
await anna.waitForSelector('#pv-side [data-roof-sketch="closed"]');
await anna.keyboard.press('Enter');
const roofsOf = () => [...window.spare.elements.values()].filter((p) => p.el.kind === 'roof').map((p) => p.el.id);
await ben.waitForFunction((n) => [...window.spare.elements.values()].filter((p) => p.el.kind === 'roof').length === n, 2);
const hipId = (await ben.evaluate(roofsOf)).find((id) => id !== roofId);
const hipDrawn = (id) => {
  const group = document.querySelector(`g.element[data-id="${id}"]`);
  const count = (kind) => group?.querySelectorAll(`[data-roof-line="${kind}"]`).length;
  return group?.querySelector('path.roof[data-form="solved"]') && count('hip') === 4 && count('ridge') === 1;
};
await ben.waitForFunction(hipDrawn, hipId);
await anna.waitForFunction(hipDrawn, hipId);
pass("Anna's hip roof appears in both browsers, its ridge and four hips drawn from the lines the server gave");
const hipBuilt = (id) => (window.spare.elements.get(id)?.shape?.displays?.[0]?.parts?.length ?? 0) >= 1;
await ben.waitForFunction(hipBuilt, hipId);
await anna.waitForFunction(hipBuilt, hipId);
pass('both browsers built the hip roof for the 3D view, each layer a part of its shape');
await shot(ben, 'ben-roof-hip');

const hipShown = await until(
  () => asAnna(`parameters/${hipId}`),
  (view) => ['roof.area', 'roof.slope-area', 'roof.ridge-height'].every((id) => view.parameters?.some((p) => p.id === id && p.from === 'read-only')),
  'the server works out the plan area, the slope area and the ridge height of the hip roof',
);
const hipPlanArea = roofRead(hipShown, 'roof.area');
const hipSlopeArea = roofRead(hipShown, 'roof.slope-area');
const hipRidge = roofRead(hipShown, 'roof.ridge-height');
check(
  /^[0-9.]+ m²$/.test(hipPlanArea) && /^[0-9.]+ m²$/.test(hipSlopeArea) && Number.parseFloat(hipSlopeArea) > Number.parseFloat(hipPlanArea) && !!hipRidge,
  `the server gives the hip roof's plan area ${hipPlanArea}, slope area ${hipSlopeArea} and ridge height ${hipRidge}`,
);
await anna.keyboard.press('Escape');
const hipAt = await onPlan(anna, 8500, -1400);
await anna.mouse.click(hipAt.x, hipAt.y);
await anna.waitForFunction(
  ([area, slope, ridge]) =>
    document.querySelector('#pv-side [data-read-only="roof.area"]')?.textContent === area &&
    document.querySelector('#pv-side [data-read-only="roof.slope-area"]')?.textContent === slope &&
    document.querySelector('#pv-side [data-read-only="roof.ridge-height"]')?.textContent === ridge,
  [hipPlanArea, hipSlopeArea, hipRidge],
);
check(
  (await anna.locator('#pv-side select[data-key="roof-own-form"]').inputValue()) === 'hip' &&
    (await anna.locator('#pv-side input[data-key^="roof-edge-pitch:"]').count()) === 4,
  "Anna's palette reads the roof as a hip roof with a pitch for each of its four edges, and the server's quantities read-only",
);
await shot(anna, 'anna-roof-hip');

// Refused by name: one edge at another pitch would meet its neighbours at a hip at two pitches.
const hipEdge = anna.locator('#pv-side input[data-key="roof-edge-pitch:1"]');
await hipEdge.fill('1:2');
await hipEdge.press('Enter');
await anna.waitForSelector('#pv-side [data-problem="roof-edit"]');
const roofRefusal = await anna.locator('#pv-side [data-problem="roof-edit"]').textContent();
check(
  /slope at different pitches where their planes meet/.test(roofRefusal) &&
    (await anna.evaluate((id) => window.spare.elements.get(id).el.recipe.args.edges.every((e) => e.pitch === 'slope:1/4' || !e.pitch), hipId)),
  "a pitch of its own on one edge of a hip roof is turned back in the server's words, and the roof is as it was",
);
await shot(anna, 'anna-roof-refused');

// Anna's undo takes the hip roof away from both browsers and her redo brings it back, drawn as before.
await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await anna.waitForFunction(() => window.spare.steps.undo?.cmd === 'roof.create');
await anna.click('#qat [data-act="stackundo"]:not([aria-disabled="true"])');
await ben.waitForFunction((id) => !document.querySelector(`g.element[data-id="${id}"]`), hipId);
await anna.waitForFunction((id) => !document.querySelector(`g.element[data-id="${id}"]`), hipId);
pass("Anna's undo takes the hip roof away from both browsers");
await anna.waitForSelector('#qat [data-act="stackredo"]:not([aria-disabled="true"])');
await anna.click('#qat [data-act="stackredo"]:not([aria-disabled="true"])');
await ben.waitForFunction(hipDrawn, hipId);
await anna.waitForFunction(hipDrawn, hipId);
pass("Anna's redo brings the hip roof back, drawn as before in both browsers");
await ben.waitForFunction((id) => document.querySelector(`g.element[data-id="${id}"]`) !== null, hipId);
const hipAgain = await asAnna(`parameters/${hipId}`);
check(
  roofRead(hipAgain, 'roof.area') === hipPlanArea && roofRead(hipAgain, 'roof.slope-area') === hipSlopeArea,
  'the quantities of the redone hip roof are the same',
);

// A last undo leaves the plan as it was before the hip roof, the gable alone.
await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await anna.waitForSelector('#qat [data-act="stackundo"]:not([aria-disabled="true"])');
await anna.click('#qat [data-act="stackundo"]:not([aria-disabled="true"])');
await ben.waitForFunction((id) => !document.querySelector(`g.element[data-id="${id}"]`), hipId);
check(
  (await ben.evaluate(roofsOf)).length === 1 && (await anna.evaluate(roofsOf)).length === 1,
  'a last undo leaves the gable roof alone in both browsers',
);
await anna.keyboard.press('Escape');

// Its area and what else the server works out of it, in units, apart from what a person sets.
const roofShown = await until(
  () => asAnna(`parameters/${roofId}`),
  (view) => view.parameters?.some((p) => p.id === 'roof.area' && p.from === 'read-only'),
  'the server works out the area of the roof',
);
const roofArea = roofRead(roofShown, 'roof.area');
const roofSlopeArea = roofRead(roofShown, 'roof.slope-area');
check(/^[0-9.]+ m²$/.test(roofArea) && roofRead(roofShown, 'roof.pitch') !== undefined, `the server gives its area in the document's units, ${roofArea}`);
const roofAreaIs = (shown) => document.querySelector('#pv-side [data-read-only="roof.area"]')?.textContent === shown;
// Picked in its overhang, where no wall is: the roof alone is there.
const roofAtAnna = await onPlan(anna, -150, 4000);
await anna.mouse.click(roofAtAnna.x, roofAtAnna.y);
await anna.waitForFunction(roofAreaIs, roofArea);
check(
  (await anna.locator('#pv-side input[data-key="roof-own-pitch"]').inputValue()) === '1:4' &&
    (await anna.locator('#pv-side input[data-key="roof-own-overhang"]').inputValue()) === '300 mm',
  "Anna's palette shows its pitch and overhang as typed, and its area read-only",
);
// An uneven gable: edge 1 at 1:3 against edge 3 at 1:4; Ben sees both pitches and the same gable drawn.
const gableEdge = anna.locator('#pv-side input[data-key="roof-edge-pitch:1"]');
await gableEdge.fill('1:3');
await gableEdge.press('Enter');
await ben.waitForFunction(
  (id) => {
    const edges = window.spare.elements.get(id)?.el.recipe.args.edges;
    return edges?.[0].pitch === 'slope:1/3' && edges[2].pitch === 'slope:1/4';
  },
  roofId,
);
const roofUneven = await until(
  () => asAnna(`parameters/${roofId}`),
  (view) => roofRead(view, 'roof.slope-area') !== roofSlopeArea,
  "the server works out the uneven gable's slope area",
);
await ben.waitForFunction(roofDrawn, roofId);
check(
  roofRead(roofUneven, 'roof.area') === roofArea,
  "Anna's pitch of its own on one face makes an uneven gable in Ben's browser, with the same area in plan",
);
const roofUnevenArea = roofRead(roofUneven, 'roof.slope-area');
await shot(anna, 'anna-roof-area');
await ben.waitForFunction((id) => window.spare.claims.get(id)?.person === 'anna', roofId);
await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await ben.waitForFunction((id) => !window.spare.claims.has(id), roofId);
pass('Anna lets go of the roof, and Ben sees it free');

// Ben changes its pitch; Anna sees the new pitch and the new slope area, and the same area in plan.
const roofAtBen = await onPlan(ben, -150, 4000);
await ben.mouse.click(roofAtBen.x, roofAtBen.y);
await ben.waitForFunction(roofAreaIs, roofArea);
const roofPitch = ben.locator('#pv-side input[data-key="roof-own-pitch"]');
await roofPitch.fill('1:2');
await roofPitch.press('Enter');
await anna.waitForFunction(
  (id) => window.spare.elements.get(id)?.el.recipe.args.edges[0].pitch === 'slope:1/2',
  roofId,
);
const roofSteeper = await until(
  () => asAnna(`parameters/${roofId}`),
  (view) => roofRead(view, 'roof.slope-area') !== roofUnevenArea,
  'the server works out the new slope area',
);
check(
  roofRead(roofSteeper, 'roof.area') === roofArea && /^[0-9.]+ m²$/.test(roofRead(roofSteeper, 'roof.slope-area')),
  "Ben's new pitch reaches Anna's browser, with a new slope area and the same area in plan",
);
await shot(ben, 'ben-roof-pitch');

// Ben lets go; Anna deletes the roof, and her undo puts it back.
await ben.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await anna.waitForFunction((id) => !window.spare.claims.has(id), roofId);
const roofAtAnnaAgain = await onPlan(anna, -150, 4000);
await anna.mouse.click(roofAtAnnaAgain.x, roofAtAnnaAgain.y);
await anna.waitForFunction(roofAreaIs, roofArea);
await anna.waitForSelector('#ribbon >> text=Delete');
await anna.keyboard.press('Delete');
await ben.waitForFunction((id) => !document.querySelector(`g.element[data-id="${id}"]`), roofId);
pass("Anna deletes the roof, and it is gone from Ben's plan");
await anna.waitForFunction(() => window.spare.steps.undo?.cmd === 'roof.delete');
await anna.click('#qat [data-act="stackundo"]:not([aria-disabled="true"])');
await ben.waitForFunction(roofDrawn, roofId);
await anna.waitForFunction(roofDrawn, roofId);
pass("Anna's undo puts the roof back, in both browsers");

// A fresh load agrees: the same roof, pitch and ridge, drawn from the server's records.
await ben.reload();
await ben.waitForSelector('html[data-stack="ready"]');
await roofHome(ben);
await ben.waitForFunction(roofDrawn, roofId);
const roofReloaded = await ben.evaluate((id) => window.spare.elements.get(id).el, roofId);
check(
  roofReloaded.recipe.args.edges[0].pitch === 'slope:1/2' && roofReloaded.recipe.args.edges[2].pitch === 'slope:1/2',
  'a fresh load of Ben’s page shows the same roof, at the pitch Ben gave it',
);
await ben.waitForTimeout(1500);
for (const [name, page] of [['Anna', anna], ['Ben', ben]]) {
  const mismatched = await page.evaluate(() => window.spare.mismatches);
  check(mismatched.length === 0, `every body ${name}'s page built, the roof's included, matches the server's`);
}

console.log('\n   Placed families');
// Anna draws a wall of her own in clear ground above the building, where the floor was, and puts a window of the
// library on it from the family tool; Ben sees the window, and the wall cut for it where this run's
// geometry cuts. Ben changes its type after Anna lets go of it, and Anna sees the new size. Anna
// deletes it and undoes the delete; a position past the wall's end is refused in words under the
// field; a fresh load agrees. Nobody changes what the other touched in the last 30 s without the
// other letting go first, and an undo claims the element it writes too.
const famKnown = await anna.evaluate(() => [...window.spare.elements.keys()]);
await anna.keyboard.press('Escape');
await anna.locator('#ribbon [role="tab"]', { hasText: 'Architecture' }).click();
await anna.click('#ribbon [data-act="dtool"][data-a="architecture:wall"]');
await anna.waitForSelector('#pv-side select[data-key="wall-type"]');
const famWallFrom = await onPlan(anna, 6500, -1200);
const famWallTo = await onPlan(anna, 11000, -1200);
await anna.mouse.click(famWallFrom.x, famWallFrom.y);
await anna.mouse.click(famWallTo.x, famWallTo.y);
const famWallOf = (known) =>
  [...window.spare.elements.values()].find((p) => p.el.recipe.op === 'wall.layered' && !known.includes(p.el.id))?.el.id;
await ben.waitForFunction(famWallOf, famKnown);
const famWall = await ben.evaluate(famWallOf, famKnown);
await anna.keyboard.press('Escape');
pass("Anna's new wall for the family appears in Ben's browser");

await anna.click('#ribbon [data-act="dtool"][data-a="architecture:family"]');
await anna.waitForSelector('#pv-side select[data-key="family-choice"] option', { state: 'attached' });
const famChoices = await anna.locator('#pv-side select[data-key="family-choice"] option').allTextContents();
check(
  ['Fixed window', 'Single swing door', 'Sliding window'].every((name) => famChoices.includes(name)),
  `the family tool lists the library's door and window families: ${famChoices.join(', ')}`,
);
await anna.locator('#pv-side select[data-key="family-choice"]').selectOption({ label: 'Fixed window' });
await anna.waitForSelector('#pv-side select[data-key="family-type"]');
const famTypes = await anna.locator('#pv-side select[data-key="family-type"] option').allTextContents();
check(famTypes.includes('1200 x 1200') && famTypes.includes('1800 x 1200'), "and the family's types");
await anna.locator('#pv-side select[data-key="family-type"]').selectOption({ label: '1200 x 1200' });
const famWallBox = await box(anna, famWall);
await anna.mouse.click(famWallBox.x + famWallBox.width * 0.3, famWallBox.y + famWallBox.height / 2 - 1);
const famOn = (wall) =>
  [...window.spare.elements.values()].find((p) => p.el.recipe.op === 'family.instance' && p.el.references?.[0]?.element === wall)?.el.id;
await ben.waitForFunction(famOn, famWall);
const famId = await ben.evaluate(famOn, famWall);
await anna.keyboard.press('Escape');
const famRecord = await ben.evaluate((id) => window.spare.elements.get(id).el, famId);
check(
  famRecord.kind === 'window' &&
    famRecord.props['@category'] === '400' &&
    famRecord.references[0].kind === 'face' &&
    /^length:q0:/.test(famRecord.props['@host.along']) &&
    famRecord.recipe.args.type === 'w1200x1200',
  "the window Anna places is in Ben's browser: a window of its family's category, hosted on a named face of her wall at an offset along it",
);
const famParts = ([id]) => document.querySelectorAll(`g.element[data-id="${id}"] polygon.part`).length > 0;
await ben.waitForFunction(famParts, [famId]);
await anna.waitForFunction(famParts, [famId]);
pass('and both browsers draw its frame and glass, each in its material');
if (process.env.STACK_GEOMETRY === 'kernel') {
  const famCut = (names) => names.some((n) => n.role === `tool(${famId}).head`);
  await until(async () => (await asAnna(`shape/${famWall}`)).built.result.shape?.names ?? [], famCut, 'the server cuts the wall for the window');
  await ben.waitForFunction(
    ([wall, cut]) => window.spare.elements.get(wall)?.shape?.names?.some((n) => n.role === `tool(${cut}).head`),
    [famWall, famId],
  );
  pass("the window cuts the wall, on the server and as Ben's page builds it");
} else {
  pass("(the stand-in cuts no openings: the kernel's run checks the wall's cut)");
}
await shot(ben, 'ben-window');

// The size of what is drawn, along the wall: its shape's bounds as a page holds them.
const famSize = (page) =>
  page.evaluate((id) => {
    const { min, max } = window.spare.elements.get(id).shape.bounds;
    return Math.max(max[0] - min[0], max[1] - min[1]);
  }, famId);
const famPick = async (page, id) => {
  await page.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
  const at = await box(page, id);
  await page.mouse.click(at.x + at.width / 2, at.y + at.height / 2);
  await page.waitForSelector(`#pv-side [data-family-properties="${id}"] select[data-key="family-retype"]`);
};
const famLetGo = async (page, other) => {
  await page.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
  await other.waitForFunction((id) => !window.spare.claims.has(id), famId);
};
const famSizeBefore = await famSize(anna);
await famPick(anna, famId);
check((await anna.locator('#pv-side').textContent()).includes('Windows'), "Anna's palette gives the window its family's category, Windows");
await ben.waitForFunction((id) => window.spare.claims.get(id)?.person === 'anna', famId);
await famLetGo(anna, ben);
pass('Anna lets go of the window, and Ben sees it free');
await famPick(ben, famId);
await ben.locator('#pv-side select[data-key="family-retype"]').selectOption({ label: '1800 x 1200' });
await anna.waitForFunction((id) => window.spare.elements.get(id)?.el.recipe.args.type === 'w1800x1200', famId);
await anna.waitForFunction(
  ([id, before]) => {
    const { min, max } = window.spare.elements.get(id).shape.bounds;
    return Math.max(max[0] - min[0], max[1] - min[1]) === before + 600;
  },
  [famId, famSizeBefore],
);
pass("Ben changes its type to 1800 x 1200, and Anna's browser draws the new size");
await famLetGo(ben, anna);

// Anna deletes it and undoes the delete.
await famPick(anna, famId);
await anna.keyboard.press('Delete');
await ben.waitForFunction((id) => !document.querySelector(`g.element[data-id="${id}"]`), famId);
pass("Anna deletes the window, and it is gone from Ben's plan");
await anna.waitForFunction(() => window.spare.steps.undo?.cmd === 'family.delete');
await anna.click('#qat [data-act="stackundo"]:not([aria-disabled="true"])');
await ben.waitForFunction(famParts, [famId]);
await anna.waitForFunction(famParts, [famId]);
pass('and her undo puts it back, in both browsers');
// The undo wrote the window, so it is Anna's for 30 s: she works on it, and Ben leaves it.
await ben.waitForFunction((id) => window.spare.claims.get(id)?.person === 'anna', famId);

// A position past the wall's end is refused in words, under the field, and nothing changes.
await famPick(anna, famId);
const famAlong = '#pv-side input[data-key="family-along"]';
await anna.locator(famAlong).fill('4900 mm');
await anna.locator(famAlong).press('Enter');
await anna.waitForSelector(`#pv-side [data-problem="family:${famId}"]`);
const famWords = await anna.locator(`#pv-side [data-problem="family:${famId}"]`).textContent();
check(famWords === `family ${famId}: it crosses the wall's end`, `the refusal reads: ${famWords}`);
check(
  (await anna.evaluate((id) => window.spare.elements.get(id).el.props['@host.along'], famId)) ===
    famRecord.props['@host.along'],
  'and the window is where it was',
);
await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });

// A fresh load agrees: same record, same type, nothing built differently.
const famFresh = await signIn('anna');
await famFresh.waitForFunction(famParts, [famId]);
const famHere = await anna.evaluate((id) => window.spare.elements.get(id).hash, famId);
check(
  (await famFresh.evaluate((id) => window.spare.elements.get(id).hash, famId)) === famHere,
  "a fresh load holds the window's record as Anna's page does",
);
await famFresh.waitForFunction((id) => window.spare.elements.get(id).el.recipe.args.type === 'w1800x1200', famId);
pass('at the type Ben gave it');
await ben.waitForTimeout(1500);
for (const [name, page] of [['Anna', anna], ['Ben', ben], ['the fresh load', famFresh]]) {
  const mismatched = await page.evaluate(() => window.spare.mismatches);
  check(mismatched.length === 0, `every body ${name}'s page built, the window's included, matches the server's`);
}

console.log('\n   Openings and shafts');
// Anna draws a wall of her own below the building and cuts an opening in it from the opening tool;
// Ben sees the opening, and the wall cut for it where this run's geometry cuts. She draws a floor
// above the building, with an opening in it; Ben, after clicking off whatever he had, makes a shaft
// through it from the shaft tool, and Anna's browser draws the floor with the shaft's hole and
// reads its area, which has dropped by exactly the shaft's. She deletes the shaft and undoes the
// delete; a position past the wall's end is refused in words under the field; Ben resizes the
// opening once she has let go of it; a fresh load agrees. Nobody changes what the other touched in
// the last 30 s without the other letting go first, and an undo claims the element it writes too.
const opKnown = await anna.evaluate(() => [...window.spare.elements.keys()]);
await anna.keyboard.press('Escape');
await anna.locator('#ribbon [role="tab"]', { hasText: 'Architecture' }).click();
await anna.click('#ribbon [data-act="dtool"][data-a="architecture:wall"]');
await anna.waitForSelector('#pv-side select[data-key="wall-type"]');
const opWallFrom = await onPlan(anna, 7000, 9000);
const opWallTo = await onPlan(anna, 11000, 9000);
await anna.mouse.click(opWallFrom.x, opWallFrom.y);
await anna.mouse.click(opWallTo.x, opWallTo.y);
const opWallOf = (known) =>
  [...window.spare.elements.values()].find((p) => p.el.recipe.op === 'wall.layered' && !known.includes(p.el.id))?.el.id;
await ben.waitForFunction(opWallOf, opKnown);
const opWall = await ben.evaluate(opWallOf, opKnown);
await anna.keyboard.press('Escape');
pass("Anna's wall for the opening appears in Ben's browser");

// The opening tool: its size is typed in the palette, and the click is the middle of the opening.
await anna.click('#ribbon [data-act="dtool"][data-a="architecture:opening"]');
const opWidthField = '#pv-side input[data-key="opening-tool-width"]';
await anna.waitForSelector(opWidthField);
check(
  (await anna.locator(opWidthField).inputValue()) === '1000 mm' &&
    (await anna.locator('#pv-side input[data-key="opening-tool-sill"]').inputValue()) === '900 mm',
  "the opening tool's palette offers a width, a height and a sill height",
);
await anna.locator(opWidthField).fill('1200 mm');
await anna.locator(opWidthField).press('Enter');
const opWallBox = await box(anna, opWall);
await anna.mouse.click(opWallBox.x + opWallBox.width * 0.3, opWallBox.y + opWallBox.height / 2 - 1);
const opOn = (wall) =>
  [...window.spare.elements.values()].find((p) => p.el.recipe.op === 'opening.wall' && p.el.references?.[0]?.element === wall)?.el.id;
await ben.waitForFunction(opOn, opWall);
const opId = await ben.evaluate(opOn, opWall);
await anna.keyboard.press('Escape');
const opRecord = await ben.evaluate((id) => window.spare.elements.get(id).el, opId);
check(
  opRecord.kind === 'opening' &&
    opRecord.props['@category'] === '1600' &&
    opRecord.references[0].kind === 'face' &&
    /^length:q0:/.test(opRecord.props['@host.along']) &&
    opRecord.props['@host.up'] === `length:q0:${900 * 6400}` &&
    opRecord.recipe.args.width === `length:q0:${1200 * 6400}`,
  "the opening Anna cuts is in Ben's browser: in the openings category, hosted on a named face of her wall, 1200 wide with its sill at 900",
);
const opDrawn = (id) => document.querySelector(`g.element[data-id="${id}"] .body.opening`);
await ben.waitForFunction(opDrawn, opId);
await anna.waitForFunction(opDrawn, opId);
pass("and both browsers draw it where it opens through the wall");
if (process.env.STACK_GEOMETRY === 'kernel') {
  const opCut = (names) => names.some((n) => n.role === `tool(${opId}).head`);
  await until(async () => (await asAnna(`shape/${opWall}`)).built.result.shape?.names ?? [], opCut, 'the server cuts the wall for the opening');
  await ben.waitForFunction(
    ([wall, cut]) => window.spare.elements.get(wall)?.shape?.names?.some((n) => n.role === `tool(${cut}).head`),
    [opWall, opId],
  );
  pass("the opening cuts the wall, on the server and as Ben's page builds it");
} else {
  pass("(the stand-in cuts no openings: the kernel's run checks the wall's cut)");
}
await shot(ben, 'ben-opening');

// A floor of Anna's, above the building, drawn from the floor tool as in the floors section, with an
// opening in it from the floor opening tool: two corners inside the floor.
const opFloorTypeField = '#pv-side select[data-key="floor-type"]';
await anna.click('#ribbon [data-act="dtool"][data-a="architecture:floor"]');
await anna.waitForSelector(`${opFloorTypeField} option[value="floor-concrete-150-screed-50"]`, { state: 'attached' });
await anna.locator(opFloorTypeField).selectOption({ label: 'Concrete 150, 50 screed' });
const opClicks = async (page, corners) => {
  for (const [x, y] of corners) {
    const at = await onPlan(page, x, y);
    await page.mouse.click(at.x, at.y);
  }
};
const opFloorEdge = [[500, -2200], [5500, -2200], [5500, -400], [500, -400]];
await opClicks(anna, [...opFloorEdge, opFloorEdge[0]]);
await anna.waitForSelector('#pv-side [data-floor-sketch="closed"]');
await anna.keyboard.press('Enter');
const opFloorOf = () => [...window.spare.elements.values()].find((p) => p.el.kind === 'floor')?.el.id;
await ben.waitForFunction(opFloorOf);
const opFloor = await ben.evaluate(opFloorOf);
const opFloorDrawn = ([id, count]) => document.querySelector(`g.element[data-id="${id}"] path.floor[data-holes="${count}"]`);
const opFloorHoles = (page, count) => page.waitForFunction(opFloorDrawn, [opFloor, count]);
await anna.keyboard.press('Escape');
const opAreaShown = async () => (await asAnna(`parameters/${opFloor}`)).parameters.find((p) => p.id === 'floor.area')?.shown;
await until(opAreaShown, (shown) => shown === '9.00 m²', 'the server works out the area of the floor, 9 m2');
pass("Anna's floor, 5 m by 1.8 m, is in the plan with its area, 9.00 m²");

await anna.click('#ribbon [data-act="dtool"][data-a="architecture:floor-opening"]');
await anna.waitForSelector('#pv-side [data-opening-sketch="empty"]');
await opClicks(anna, [[1000, -2000], [2000, -1500]]);
const opFloorOpening = () => [...window.spare.elements.values()].find((p) => p.el.recipe.op === 'opening.floor')?.el.id;
await ben.waitForFunction(opFloorOpening);
await anna.keyboard.press('Escape');
await opFloorHoles(ben, 1);
await opFloorHoles(anna, 1);
await until(opAreaShown, (shown) => shown === '8.50 m²', 'the floor reads its area after the opening');
pass("Anna's opening in the floor is a hole in it in both browsers, and the floor's area is 8.50 m²");
await shot(anna, 'anna-floor-opening');

// Ben clicks off whatever he had and makes a shaft through the floor from the shaft tool; Anna has
// the floor picked, and her palette's area drops by exactly the shaft's 0.64 m².
const opFloorAt = await onPlan(anna, 4800, -1000);
await anna.mouse.click(opFloorAt.x, opFloorAt.y);
const opAreaIs = (shown) => document.querySelector('#pv-side [data-read-only="floor.area"]')?.textContent === shown;
await anna.waitForFunction(opAreaIs, '8.50 m²');
await ben.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await ben.locator('#ribbon [role="tab"]', { hasText: 'Architecture' }).click();
await ben.click('#ribbon [data-act="dtool"][data-a="architecture:shaft"]');
const opStartField = '#pv-side select[data-key="shaft-tool-level"]';
await ben.waitForSelector(`${opStartField} option[value="2"]`, { state: 'attached' });
check(
  (await ben.locator(opStartField).inputValue()) === '1' &&
    (await ben.locator('#pv-side select[data-key="shaft-tool-top"]').inputValue()) === '2',
  "the shaft tool offers the plan's level to start on and the level above to end on",
);
await ben.locator('#pv-side input[data-key="shaft-tool-offset"]').fill('-500 mm');
await ben.locator('#pv-side input[data-key="shaft-tool-offset"]').press('Enter');
await opClicks(ben, [[3500, -1900], [4300, -1100]]);
const opShaftOf = () => [...window.spare.elements.values()].find((p) => p.el.kind === 'shaft')?.el.id;
await anna.waitForFunction(opShaftOf);
const opShaft = await anna.evaluate(opShaftOf);
await ben.keyboard.press('Escape');
const opShaftRecord = await anna.evaluate((id) => window.spare.elements.get(id).el, opShaft);
check(
  opShaftRecord.recipe.op === 'shaft' &&
    opShaftRecord.props['@category'] === '1600' &&
    opShaftRecord.props['@level'] === '1' &&
    opShaftRecord.props['@top.level'] === '2' &&
    opShaftRecord.props['@level.offset'] === `length:q0:${-500 * 6400}` &&
    opShaftRecord.references.length === 0,
  "the shaft Ben draws is in Anna's browser: tied to Level 1, 500 below it, and to Level 2, hosted on nothing",
);
await opFloorHoles(anna, 2);
await opFloorHoles(ben, 2);
pass("and both browsers draw the floor with the shaft's hole as well as the opening's");
await anna.waitForFunction(opAreaIs, '7.86 m²');
await until(opAreaShown, (shown) => shown === '7.86 m²', 'the server works out the floor after the shaft');
pass("the floor's area has dropped by exactly the shaft's 0.64 m², to 7.86 m², on the server and in Anna's palette");
await shot(anna, 'anna-shaft');
await ben.waitForFunction((id) => window.spare.elements.get(id)?.fill?.cut_by?.shafts?.length === 1, opFloor);
pass("Ben's page builds the floor with the shaft the server says passes through it");
await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await ben.waitForFunction((id) => !window.spare.claims.has(id), opFloor);

// Anna picks the shaft and deletes it, and her undo puts it back.
const opPick = async (page, id, field) => {
  await page.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
  const at = await box(page, id);
  await page.mouse.click(at.x + at.width / 2, at.y + at.height / 2);
  await page.waitForSelector(`#pv-side [data-opening-properties="${id}"] ${field}`);
};
await opPick(anna, opShaft, 'select[data-key="shaft-level"]');
check(
  (await anna.locator('#pv-side input[data-key="shaft-w"]').inputValue()) === '800 mm' &&
    (await anna.locator('#pv-side input[data-key="shaft-top-offset"]').inputValue()) === '0 mm',
  "Anna's palette shows the shaft's size and where it starts and ends",
);
await anna.waitForFunction(() => !document.activeElement?.closest('input,textarea,select'));
await anna.keyboard.press('Delete');
await ben.waitForFunction((id) => !document.querySelector(`g.element[data-id="${id}"]`), opShaft);
await opFloorHoles(ben, 1);
await until(opAreaShown, (shown) => shown === '8.50 m²', 'the floor is whole again without the shaft');
pass("Anna deletes the shaft, and Ben's browser draws the floor without its hole");
await anna.waitForFunction(() => window.spare.steps.undo?.cmd === 'shaft.delete');
await anna.click('#qat [data-act="stackundo"]:not([aria-disabled="true"])');
await ben.waitForFunction((id) => document.querySelector(`g.element[data-id="${id}"] .body.shaft`), opShaft);
await opFloorHoles(ben, 2);
await opFloorHoles(anna, 2);
await until(opAreaShown, (shown) => shown === '7.86 m²', 'her undo cuts the floor again');
pass("and her undo puts the shaft back, the floor cut again, in both browsers");
// The undo wrote the shaft, so it is Anna's for 30 s: Ben leaves it.
await ben.waitForFunction((id) => window.spare.claims.get(id)?.person === 'anna', opShaft);
await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });

// A position past the wall's end is refused in words, under the field, and nothing changes.
await opPick(anna, opId, 'input[data-key="opening-along"]');
const opAlong = '#pv-side input[data-key="opening-along"]';
await anna.locator(opAlong).fill('3900 mm');
await anna.locator(opAlong).press('Enter');
await anna.waitForSelector(`#pv-side [data-problem="opening:${opId}"]`);
const opWords = await anna.locator(`#pv-side [data-problem="opening:${opId}"]`).textContent();
check(opWords === `opening ${opId}: it crosses the wall's end`, `the refusal reads: ${opWords}`);
check(
  (await anna.evaluate((id) => window.spare.elements.get(id).el.props['@host.along'], opId)) === opRecord.props['@host.along'],
  'and the opening is where it was',
);
await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await ben.waitForFunction((id) => !window.spare.claims.has(id), opId);

// Ben widens it from his palette once Anna has let go; Anna's browser draws the new width.
await opPick(ben, opId, 'input[data-key="opening-width"]');
const opWide = ben.locator('#pv-side input[data-key="opening-width"]');
check((await opWide.inputValue()) === '1200 mm', "Ben's palette shows the opening's width");
await opWide.fill('1500 mm');
await opWide.press('Enter');
await anna.waitForFunction((id) => window.spare.elements.get(id)?.el.recipe.args.width === `length:q0:${1500 * 6400}`, opId);
pass("Ben changes its width to 1500 mm, and Anna's browser holds the new record");
await ben.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await anna.waitForFunction((id) => !window.spare.claims.has(id), opId);

// A fresh load agrees: the same records, the floor cut as before, nothing built differently.
const opFresh = await signIn('anna');
await opFresh.waitForFunction(opDrawn, opId);
await opFresh.waitForFunction((id) => document.querySelector(`g.element[data-id="${id}"] .body.shaft`), opShaft);
await opFresh.waitForFunction(opFloorDrawn, [opFloor, 2]);
for (const id of [opId, opShaft]) {
  const here = await anna.evaluate((one) => window.spare.elements.get(one).hash, id);
  check(
    (await opFresh.evaluate((one) => window.spare.elements.get(one).hash, id)) === here,
    `a fresh load holds the record of ${id} as Anna's page does`,
  );
}
await ben.waitForTimeout(1500);
for (const [name, page] of [['Anna', anna], ['Ben', ben], ['the fresh load', opFresh]]) {
  const mismatched = await page.evaluate(() => window.spare.mismatches);
  check(mismatched.length === 0, `every body ${name}'s page built, the opening's and the shaft's included, matches the server's`);
}

console.log('\n   Rooms, found from the walls round a point');
// Anna places a room by a click inside the seed building's west half; Ben sees its tag and its area.
// After Anna lets go, Ben moves the wall on its west side and both see the area the server gives
// for the new loop; Ben deletes that wall and both see the room not enclosed, with no area; his undo
// puts the wall back and the room is enclosed again. Anna renames the room, picked in the plan, and
// a fresh load agrees. Anna touches only the room and Ben only the wall.
await toArchitecture(anna);
await toArchitecture(ben);
for (const page of [anna, ben]) await page.click('#browser [data-act="view"][data-a="l1"]');
await anna.locator('#ribbon [role="tab"]', { hasText: 'Architecture' }).click();
await anna.click('#ribbon [data-act="dtool"][data-a="architecture:room"]');
const roomNameField = '#pv-side input[data-key="room-tool-name"]';
const roomNumberField = '#pv-side input[data-key="room-tool-number"]';
await anna.waitForSelector(`${roomNumberField}[value="1"]`);
pass('the Room tool offers the next free number, 1, from the server');
await anna.locator(roomNameField).fill('Studio');
await anna.locator(roomNameField).press('Enter');
const roomClick = await onPlan(anna, 3000, 4000);
await anna.mouse.click(roomClick.x, roomClick.y);
const roomOf = () => [...window.spare.elements.values()].find((p) => p.el.kind === 'room')?.el.id;
await ben.waitForFunction(roomOf);
const roomId = await ben.evaluate(roomOf);
const roomTag = (id, part) => `#pv-canvas g.room-tag[data-room-tag="${id}"] text.${part}`;
const roomTagIs = (page, part, text) =>
  page.waitForFunction(([selector, want]) => document.querySelector(selector)?.textContent === want, [roomTag(roomId, part), text]);
const roomArea = async (before) =>
  (
    await until(
      () => asAnna(`parameters/${roomId}`),
      (view) => {
        const shown = view.parameters?.find((p) => p.id === 'room.area')?.shown;
        return shown !== undefined && shown !== before;
      },
      'the server works out the room area',
    )
  ).parameters.find((p) => p.id === 'room.area').shown;
const roomFirst = await roomArea(null);
check(/^[0-9.]+ m²$/.test(roomFirst), `the server gives the room's area in the document's units, ${roomFirst}`);
await roomTagIs(ben, 'room-area', roomFirst);
await roomTagIs(ben, 'room-name', 'Studio');
await roomTagIs(ben, 'room-number', '1');
pass("Anna places a room with a click inside a loop of walls, and Ben's browser tags it with its name, number and area");
await roomTagIs(anna, 'room-area', roomFirst);
await ben.waitForSelector(`#pv-canvas g.element[data-id="${roomId}"] path.room-fill`);
await shot(ben, 'ben-room');

// Anna leaves the tool, picks the room she placed and lets go of it by clicking off.
await anna.keyboard.press('Escape');
const roomFirstPick = await onPlan(anna, 2000, 6000);
await anna.mouse.click(roomFirstPick.x, roomFirstPick.y);
await anna.waitForSelector('#pv-side input[data-key="room-name"]');
await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await ben.waitForFunction((id) => !window.spare.claims.has(id), roomId);
pass('Anna lets go of the room, and Ben sees it free');

// Ben drags the wall on the room's west side east; the loop is smaller.
await ben.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
await drag(ben, 'W-103', 25, 0);
await ben.mouse.up();
const roomMoved = await roomArea(roomFirst);
check(roomMoved !== roomFirst, `a wall of the room moved: the server gives a new area, ${roomMoved}`);
await roomTagIs(ben, 'room-area', roomMoved);
await roomTagIs(anna, 'room-area', roomMoved);
pass("Ben moves a wall of the room, and both browsers tag it with the server's new area");

// Ben deletes that wall: the loop is open, the room stays, with its state in words and no area.
await ben.waitForSelector('#ribbon >> text=Delete');
await ben.waitForFunction(() => !document.activeElement?.closest('input,textarea,select'));
await ben.keyboard.press('Delete');
for (const page of [anna, ben]) {
  await roomTagIs(page, 'room-state', 'Not enclosed');
  check((await page.locator(`#pv-canvas g.room-tag[data-room-tag="${roomId}"] text.room-area`).count()) === 0, 'and shows no area');
}
const roomOpen = await until(
  () => asAnna(`filled/${roomId}`),
  (view) => view.room?.state === 'not enclosed',
  'the server says the room is not enclosed',
);
check(
  roomOpen.room.area === null || roomOpen.room.area === undefined,
  `Ben deletes the wall: the server says "${roomOpen.room.words}", and gives no area`,
);
await shot(anna, 'anna-room-not-enclosed');

// His undo puts the wall back, and the loop closes on the same area.
await ben.waitForFunction(() => window.spare.pending.length === 0 && window.spare.steps.undo?.cmd === 'element.delete');
await ben.locator('#qat [data-act="stackundo"]:not([aria-disabled="true"])').click();
await roomTagIs(ben, 'room-area', roomMoved);
await roomTagIs(anna, 'room-area', roomMoved);
pass("Ben's undo puts the wall back, and both browsers show the room enclosed again, as it was");

// Anna picks the room in the plan and renames it in the properties palette.
const roomPick = await onPlan(anna, 2000, 6000);
await anna.mouse.click(roomPick.x, roomPick.y);
const roomRename = '#pv-side input[data-key="room-name"]';
await anna.waitForSelector(roomRename);
await anna.waitForFunction(
  (want) => document.querySelector('#pv-side [data-read-only="room.area"]')?.textContent === want,
  roomMoved,
);
pass("Anna's palette shows its area read-only, the same as the server's");
await anna.locator(roomRename).fill('Studio A');
await anna.locator(roomRename).press('Enter');
await roomTagIs(ben, 'room-name', 'Studio A');
await roomTagIs(anna, 'room-name', 'Studio A');
pass("Anna renames the room, and Ben's browser tags it with the new name");

// A fresh load agrees.
const roomsFresh = await signIn('anna');
await roomsFresh.waitForFunction(
  ([selector, want]) => document.querySelector(selector)?.textContent === want,
  [roomTag(roomId, 'room-name'), 'Studio A'],
);
await roomsFresh.waitForFunction(
  ([selector, want]) => document.querySelector(selector)?.textContent === want,
  [roomTag(roomId, 'room-area'), roomMoved],
);
check(
  (await roomsFresh.locator(roomTag(roomId, 'room-number')).textContent()) === '1',
  'a fresh load tags the room with the same name, number and area',
);
await roomsFresh.context().close();
await ben.waitForTimeout(1500);
for (const [name, page] of [['Anna', anna], ['Ben', ben]]) {
  const mismatched = await page.evaluate(() => window.spare.mismatches);
  check(mismatched.length === 0, `every body ${name}'s page built, the room's included, matches the server's`);
}


console.log('\n   Graphics rules, in a view');
// Anna opens Level 1's plan and its Graphics from the view's line in the browser. She gives what
// walls cut a heavier pen, a colour, another fill in another colour and halftone: Ben's page and
// hers both read the rule, and on a server with the geometry kernel both draw the plan with it. A
// value the server refuses (transparency over 100) is turned back in its words beside the
// controls. She hides what walls cut and the drawing goes in both; she clears the rule and it
// comes back as it was; her undo puts the hiding back, then the rule; her redo clears it again.
const gfxView = 'VP-1';
const gfxWall = '#pv-canvas g.drawn line[data-element="W-101"]';
const gfxHatch = '#pv-canvas g.drawn path[data-material]';
const gfxDialog = '#stack-manage';
const gfxControl = (key) => `${gfxDialog} [data-key="${key}"]`;
const gfxRow = `${gfxDialog} [data-gfx-rule="100"]`;
const gfxCut = (page) =>
  page.evaluate(
    (url) => fetch(url).then((r) => r.json()).then((l) => l.rules.find((r) => r.category === 100)?.cut ?? null),
    `${model}/views/${gfxView}/graphics`,
  );
// Waits until the server's list of rules says what the call names: 'none' (no rule for walls), 'hidden'
// or 'ruled' (shown, in the colour set below).
const gfxWait = (page, what) =>
  page.waitForFunction(
    async ([url, want]) => {
      const list = await fetch(url).then((r) => r.json());
      const cut = list.rules.find((r) => r.category === 100)?.cut ?? null;
      if (want === 'none') return cut === null;
      if (want === 'hidden') return cut !== null && cut.shown === false;
      return cut !== null && cut.shown === true && cut.colour.join() === '200,16,32';
    },
    [`${model}/views/${gfxView}/graphics`, what],
    { polling: 250 },
  );
await anna.click(viewLeaf('Level 1 plan'));
await ben.click(viewLeaf('Level 1 plan'));
let gfxPlain = null;
let gfxHatches = 0;
if (withKernel) {
  await anna.waitForSelector(gfxWall, { state: 'attached' });
  await ben.waitForSelector(gfxWall, { state: 'attached' });
  gfxPlain = await anna.locator(gfxWall).first().getAttribute('stroke-width');
  await anna.waitForSelector(gfxHatch, { state: 'attached' });
  gfxHatches = await anna.locator(gfxHatch).count();
}
await anna.click('#browser [data-act="stackviewgraphics"]');
await anna.waitForSelector(gfxControl('gfx-category'));
check(
  (await anna.locator(`${gfxDialog} [data-gfx-none]`).count()) === 1,
  'a view with no rules lists none, and says every category looks as the project says',
);
await anna.locator(gfxControl('gfx-category')).selectOption('100');
await anna.waitForSelector(`${gfxDialog} [data-gfx-origin="project"]`);
const gfxHeaviest = await anna.locator(`${gfxControl('gfx-pen')} option`).last().getAttribute('value');
await anna.locator(gfxControl('gfx-pen')).selectOption(gfxHeaviest);
await anna.waitForSelector(`${gfxRow}:has-text("pen ${gfxHeaviest}")`);
await anna.waitForSelector(`${gfxDialog} [data-gfx-origin="category"]`);
await anna.locator(gfxControl('gfx-colour')).fill('#c81020');
await anna.waitForSelector(`${gfxRow}:has-text("#c81020")`);
await anna.locator(gfxControl('gfx-fill')).selectOption('earth');
await anna.waitForSelector(`${gfxRow}:has-text("earth fill")`);
await anna.locator(gfxControl('gfx-fill-colour')).fill('#0a5a14');
await anna.waitForSelector(`${gfxRow}:has-text("#0a5a14")`);
await anna.locator(gfxControl('gfx-halftone')).check();
await anna.waitForSelector(`${gfxRow}:has-text("halftone")`);
await gfxWait(ben, 'ruled');
check(
  (await gfxCut(ben)).pen === Number(gfxHeaviest) && (await gfxCut(ben)).fill.pattern === 'earth',
  "walls cut in the heaviest pen with another fill: Ben's page reads the rule Anna set",
);
if (withKernel) {
  for (const page of [anna, ben]) {
    await page.waitForSelector(`${gfxWall}[stroke="#c81020"][opacity="0.5"]`, { state: 'attached' });
    await page.waitForSelector(`${gfxHatch}[stroke="#0a5a14"][opacity="0.5"]`, { state: 'attached' });
  }
  const heavy = await ben.locator(gfxWall).first().getAttribute('stroke-width');
  check(Number(heavy) > Number(gfxPlain), "the plan is drawn in both browsers with the heavier pen, the colour, the fill and halftone");
}
await shot(anna, 'anna-graphics-rule');

// A value the server refuses is turned back in its words, and the rule stays as it was.
await anna.locator(gfxControl('gfx-transparency')).fill('150');
await anna.locator(gfxControl('gfx-transparency')).press('Enter');
await anna.waitForSelector(`${gfxDialog} [data-problem="graphics:100:cut"]`);
const gfxWords = await anna.locator(`${gfxDialog} [data-problem="graphics:100:cut"]`).textContent();
check(/Transparency is a percentage from 0 to 100, not 150/.test(gfxWords), `the refusal reads: ${gfxWords}`);
check((await gfxCut(ben)).transparency === 0, 'and the rule is as it was');

// Hiding what walls cut takes the drawing away in both browsers.
await anna.locator(gfxControl('gfx-shown')).uncheck();
await anna.waitForSelector(`${gfxRow}:has-text("hidden")`);
await gfxWait(ben, 'hidden');
if (withKernel) {
  for (const page of [anna, ben]) {
    await page.waitForSelector(gfxWall, { state: 'detached' });
    await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length < n, [gfxHatch, gfxHatches]);
  }
  pass('walls hidden: no cut edge and no hatch of them is drawn, in either browser (the doors still are)');
} else {
  pass("walls hidden: Ben's page reads the rule");
}

// Clearing the rule gives the first drawing back.
await anna.click(gfxControl('gfx-clear'));
await anna.waitForSelector(`${gfxDialog} [data-gfx-none]`);
await gfxWait(ben, 'none');
if (withKernel) {
  for (const page of [anna, ben]) {
    await page.waitForSelector(`${gfxWall}:not([stroke="#c81020"])`, { state: 'attached' });
    await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [gfxHatch, gfxHatches]);
    check(
      (await page.locator(gfxWall).first().getAttribute('stroke-width')) === gfxPlain &&
        (await page.locator(`${gfxWall}[stroke="#c81020"]`).count()) === 0,
      'the rule cleared, the plan is drawn as it was before it',
    );
  }
} else {
  pass("the rule cleared, Ben's page lists none");
}

// Her undo is her own: first the clear (the hiding is back), then the hiding (the rule is shown).
await anna.locator('#dlg .dlg-f [data-act="dgclose"]').click();
await anna.waitForSelector('#dlg', { state: 'hidden' });
await anna.click('#qat [data-act="stackundo"]');
await gfxWait(ben, 'hidden');
if (withKernel) {
  for (const page of [anna, ben]) await page.waitForSelector(gfxWall, { state: 'detached' });
}
pass('her undo puts the hiding back, and Ben follows');
await anna.click('#qat [data-act="stackundo"]');
await gfxWait(ben, 'ruled');
if (withKernel) {
  for (const page of [anna, ben]) await page.waitForSelector(`${gfxWall}[stroke="#c81020"]`, { state: 'attached' });
}
pass('her next undo shows the walls again in the rule, and Ben follows');
await anna.click('#qat [data-act="stackredo"]');
await gfxWait(ben, 'hidden');
await anna.click('#qat [data-act="stackredo"]');
await gfxWait(ben, 'none');
if (withKernel) {
  for (const page of [anna, ben]) {
    await page.waitForSelector(`${gfxWall}:not([stroke="#c81020"])`, { state: 'attached' });
  }
}
pass('her redos hide the walls and clear the rule again, and the plan is as it was in both browsers');
console.log('\n   Floors, sloped and with slab edges');
// Anna draws two floors south of the building, of the 200 mm concrete type. She picks the first,
// types its two heights, draws its slope arrow in the plan with two clicks, and both browsers draw
// the arrow with its heights and hold the floor's body as the server built it (40 mm lower at one
// end); the server's read-only rows give its lowest and highest points and its top area. Ben picks
// the second, chooses a downturn and clicks one of its sides: both browsers draw the slab edge, and
// the rows give its length and volume. Ben then tries a slope on that floor, and the server's words
// say why not. Her undo and redo of the slope, and his remove and undo of the edge, are seen in
// both browsers. Each lets go of the other's floor, and each deletes their own, so the plan ends as
// it began.
await floorHome(anna);
await floorHome(ben);
await anna.click('#ribbon [data-act="dtool"][data-a="architecture:floor"]');
await anna.locator(floorTypeField).selectOption({ label: 'Concrete 200' });
await anna.waitForSelector('#pv-side [data-floor-layers="floor-concrete-200"]');
const slFloorA = [[7000, -2300], [10000, -2300], [10000, -500], [7000, -500]];
const slFloorB = [[11000, -2300], [13000, -2300], [13000, -500], [11000, -500]];
const slFloorWith = (page, x) =>
  page.evaluate(
    (steps) =>
      [...window.spare.elements.values()].find(
        (p) => p.el.kind === 'floor' && p.el.recipe.args.boundary?.sketch?.points?.[0]?.x === steps,
      )?.el.id,
    x * 6400,
  );
for (const corners of [slFloorA, slFloorB]) {
  await floorClicks(anna, [...corners, corners[0]]);
  await anna.waitForSelector('#pv-side [data-floor-sketch="closed"]');
  await anna.keyboard.press('Enter');
  await anna.waitForSelector('#pv-side [data-floor-sketch="empty"]');
}
await ben.waitForFunction((x) => [...window.spare.elements.values()].some((p) => p.el.kind === 'floor' && p.el.recipe.args.boundary?.sketch?.points?.[0]?.x === x), 11000 * 6400);
const slIdA = await slFloorWith(ben, 7000);
const slIdB = await slFloorWith(ben, 11000);
check(slIdA && slIdB && slIdA !== slIdB, 'Anna draws two plain floors, and Ben has both');
await anna.keyboard.press('Escape');

// The first floor: heights typed, the arrow drawn with two clicks.
const slPick = async (page, id, x, y) => {
  const at = await onPlan(page, x, y);
  await page.mouse.click(at.x, at.y);
  await page.waitForSelector(`#pv-side [data-floor-slope-part]`);
  await page.waitForFunction((who) => window.spare.claims.get(who)?.person === window.spare.me.person.id, id);
};
await slPick(anna, slIdA, 8500, -1400);
await anna.locator('#pv-side input[data-key="floor-slope-atTail"]').fill('0 mm');
await anna.locator('#pv-side input[data-key="floor-slope-atHead"]').fill('-40 mm');
await anna.locator('#pv-side input[data-key="floor-slope-atHead"]').press('Enter');
await anna.click('#pv-side [data-key="floor-slope-pick"]');
await anna.waitForSelector('#pv-side [data-floor-pick="tail"]');
const slTail = await onPlan(anna, 7000, -1400);
await anna.mouse.click(slTail.x, slTail.y);
await anna.waitForSelector('#pv-side [data-floor-pick="head"]');
const slHead = await onPlan(anna, 10000, -1400);
await anna.mouse.click(slHead.x, slHead.y);
const slArrow = (id) => `g.element[data-id="${id}"] [data-floor-slope="${id}"]`;
const slBodyLow = (id) => window.spare.elements.get(id)?.shape?.bounds?.min?.[2] === -240 && window.spare.elements.get(id).shape.bounds.max[2] === 0;
for (const [who, page] of [['Anna', anna], ['Ben', ben]]) {
  await page.waitForSelector(slArrow(slIdA));
  await page.waitForFunction(slBodyLow, slIdA);
  const heights = await page.locator(`${slArrow(slIdA)} text`).allTextContents();
  check(heights.join('|') === '0 mm|-40 mm', `${who}'s plan draws the slope arrow with its two heights, ${heights.join(' and ')}`);
}
pass("Anna's slope arrow is drawn in both plans, and both browsers hold the body 240 mm deep at its low end");
const slAsBen = (path) => ben.evaluate((url) => fetch(url).then((r) => r.json()), `${model}/${path}`);
const slRead = (view, id) => view.parameters.find((p) => p.id === id)?.shown;
const slRowsA = await until(
  () => slAsBen(`parameters/${slIdA}`),
  (view) => slRead(view, 'floor.lowest') !== undefined,
  'the server works out the slope',
);
check(
  slRead(slRowsA, 'floor.lowest') === '-240 mm' && slRead(slRowsA, 'floor.highest') === '0 mm' && /^5\.40 m²$/.test(slRead(slRowsA, 'floor.top-area')),
  `the server gives the floor's lowest and highest points and its sloped top area, ${slRead(slRowsA, 'floor.lowest')}, ${slRead(slRowsA, 'floor.highest')} and ${slRead(slRowsA, 'floor.top-area')}`,
);
await anna.waitForFunction(() => document.querySelector('#pv-side [data-read-only="floor.lowest"]')?.textContent === '-240 mm');
check(
  (await anna.locator('#pv-side [data-floor-slope-now]').textContent()) === '0 mm to -40 mm',
  "Anna's palette says what the slope is, read from the record",
);
await shot(anna, 'anna-floor-slope');

// The second floor: a downturn on one side, picked in the plan.
const slAtBen = async (x, y) => onPlan(ben, x, y);
const slPickBen = await slAtBen(12000, -1400);
await ben.mouse.click(slPickBen.x, slPickBen.y);
await ben.waitForSelector('#pv-side [data-floor-slope-part]');
await ben.waitForFunction((id) => window.spare.claims.get(id)?.person === 'ben', slIdB);
await ben.locator('#pv-side select[data-key="floor-edge-kind"]').selectOption('downturn');
await ben.locator('#pv-side input[data-key="floor-edge-width"]').fill('300 mm');
await ben.locator('#pv-side input[data-key="floor-edge-depth"]').fill('400 mm');
await ben.locator('#pv-side input[data-key="floor-edge-depth"]').press('Enter');
await ben.click('#pv-side [data-key="floor-edge-pick"]');
await ben.waitForSelector('#pv-side [data-floor-pick="edge"]');
// A side is a line with no height, which the page never counts as visible: wait for it to be drawn,
// then click where the plan puts its middle.
await ben.waitForSelector('#pv-canvas line[data-floor-edge="o0"]', { state: 'attached' });
const slSide = await onPlan(ben, 12000, -2300);
await ben.mouse.click(slSide.x, slSide.y);
const slEdgeDrawn = (id) => `g.element[data-id="${id}"] path[data-slab-edge="o0"][data-kind="downturn"]`;
await ben.waitForSelector('#pv-side [data-slab-edge-row="o0"]');
await anna.waitForSelector(slEdgeDrawn(slIdB));
await ben.waitForSelector(slEdgeDrawn(slIdB));
await anna.waitForFunction((id) => window.spare.elements.get(id)?.shape?.bounds?.min?.[2] === -600, slIdB);
pass("Ben's downturn on the floor's first side is drawn in both plans, and the body goes 400 mm deeper");
const slRowsB = await until(
  () => slAsBen(`parameters/${slIdB}`),
  (view) => slRead(view, 'floor.edge-o0.length') !== undefined,
  'the server works out the slab edge',
);
check(
  slRead(slRowsB, 'floor.edge-o0.length') === '2000 mm' && slRead(slRowsB, 'floor.edge-o0.volume') === '0.24 m³',
  `the server gives the slab edge's length and volume, ${slRead(slRowsB, 'floor.edge-o0.length')} and ${slRead(slRowsB, 'floor.edge-o0.volume')}`,
);
await ben.waitForFunction(() => document.querySelector('#pv-side [data-read-only="floor.edge-o0.length"]')?.textContent === '2000 mm');
pass("and Ben's palette shows them beside the floor's");
await shot(ben, 'ben-floor-edge');

// A slope on a floor with slab edges: refused in the server's words, and nothing changes.
await ben.click('#pv-side [data-key="floor-slope-pick"]');
await ben.waitForSelector('#pv-side [data-floor-pick="tail"]');
const slBadTail = await slAtBen(11500, -1400);
await ben.mouse.click(slBadTail.x, slBadTail.y);
await ben.waitForSelector('#pv-side [data-floor-pick="head"]');
const slBadHead = await slAtBen(12500, -1400);
await ben.mouse.click(slBadHead.x, slBadHead.y);
await ben.waitForSelector('#pv-side [data-problem="floor-slope"]');
const slRefusal = await ben.locator('#pv-side [data-problem="floor-slope"]').textContent();
check(
  slRefusal === `${slIdB}'s slope: a floor cannot have both a slope and slab edges yet: slab edges under a sloped floor are not built`,
  `a slope on the floor with a slab edge is turned back in the server's words: ${slRefusal}`,
);
check(
  (await ben.evaluate((id) => window.spare.elements.get(id).el.recipe.args.slope, slIdB)) === undefined,
  'and the floor has no slope',
);
await ben.keyboard.press('Escape');

// Her undo and redo of the slope.
await anna.waitForFunction(() => window.spare.steps.undo?.cmd === 'floor.slope');
await anna.click('#qat [data-act="stackundo"]:not([aria-disabled="true"])');
for (const page of [anna, ben]) {
  await page.waitForFunction((id) => !window.spare.elements.get(id)?.el.recipe.args.slope && !document.querySelector(`g.element[data-id="${id}"] [data-floor-slope]`), slIdA);
}
pass("Anna's undo takes the slope off, in both browsers");
await anna.click('#qat [data-act="stackredo"]:not([aria-disabled="true"])');
for (const page of [anna, ben]) await page.waitForSelector(slArrow(slIdA));
pass('and her redo puts the arrow back, in both browsers');

// His remove of the slab edge, and his undo of it.
await ben.click('#pv-side [data-key="floor-edge-remove-o0"]');
await anna.waitForFunction((id) => !document.querySelector(`g.element[data-id="${id}"] path[data-slab-edge]`), slIdB);
await ben.waitForFunction(() => window.spare.steps.undo?.cmd === 'floor.edge.remove');
await ben.click('#qat [data-act="stackundo"]:not([aria-disabled="true"])');
await anna.waitForSelector(slEdgeDrawn(slIdB));
pass("Ben removes the slab edge and undoes that, and Anna's plan follows both");
await ben.waitForTimeout(1500);
for (const [name, page] of [['Anna', anna], ['Ben', ben]]) {
  const mismatched = await page.evaluate(() => window.spare.mismatches);
  if (mismatched.length) console.log(JSON.stringify(mismatched).slice(0, 3000));
  check(mismatched.length === 0, `every body ${name}'s page built, the shaped floors included, matches the server's`);
}

// Each deletes their own floor; the plan ends as it began.
await anna.waitForFunction(() => !document.activeElement?.closest('input,textarea,select'));
await anna.keyboard.press('Delete');
await ben.waitForFunction((id) => !document.querySelector(`g.element[data-id="${id}"]`), slIdA);
await ben.waitForFunction(() => !document.activeElement?.closest('input,textarea,select'));
await ben.keyboard.press('Delete');
await anna.waitForFunction((id) => !document.querySelector(`g.element[data-id="${id}"]`), slIdB);
pass('each deletes their own floor, and both are gone from both plans');

console.log('\n   Warnings, listed from the server');
// The page keeps no rules: the count in the status bar and the list in the project browser are what
// the server's /warnings gives, read again after every entry of the log. On the sound model both
// browsers say there are none; Ben deletes the wall on the room's west side, so the room is not
// enclosed and both browsers list it with the server's own sentence; Anna chooses it and the room is
// selected. Ben draws a wall over the gap and both lists are empty again; his undo and redo bring the
// warning back and take it away. Then Ben draws two walls along one line, so they overlap and the
// other kind appears, naming both; his deleting one clears it, and undo and redo bring it back and
// away. Anna only reads and chooses; Ben changes everything, so no claim of one is in the other's way.
// Both browsers go back to the architecture plan of Level 1, as the Rooms section left it.
for (const page of [anna, ben]) {
  await toArchitecture(page);
  await page.click('#browser [data-act="view"][data-a="l1"]');
}
const warnCount = (page, n) =>
  page.waitForFunction((want) => document.querySelector('#st-warnings')?.dataset.count === want, String(n));
const warnRowsAre = (page, kind, n) =>
  page.waitForFunction(
    ([selector, want]) => document.querySelectorAll(selector).length === want,
    [`#browser [data-warning][data-kind="${kind}"]`, n],
  );
const warnEmptyWords = (page) =>
  page.waitForFunction(() =>
    document.querySelector('#browser [data-warnings-empty]')?.textContent.startsWith('No warnings'),
  );
const serverWarnings = async () => (await asAnna('warnings')).warnings;
const severityWords = (word) => word.charAt(0).toUpperCase() + word.slice(1);

for (const [name, page] of [['Anna', anna], ['Ben', ben]]) {
  await warnCount(page, 0);
  await warnEmptyWords(page);
  check(
    (await page.locator('#st-warnings').textContent()).includes('No warnings'),
    `${name}'s status bar says there are no warnings, and the list says so in words`,
  );
}
check((await serverWarnings()).length === 0, 'the server lists none for the sound model, so the page shows the server');
await anna.locator('#st-warnings').click();
check(
  await anna.locator('#browser details[data-k="Warnings"]').evaluate((d) => d.open),
  'the count in the status bar opens the list',
);

// Ben deletes the wall on the room's west side: the loop is open.
const gap = await ben.evaluate(() => {
  const at = document.querySelector('#pv-canvas g.element[data-id="W-103"] .body').getBBox();
  return { x: at.x, y: at.y, width: at.width, height: at.height };
});
await ben.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
const westWall = await box(ben, 'W-103');
await ben.mouse.click(westWall.x + westWall.width / 2, westWall.y + westWall.height / 2);
await ben.waitForSelector('#ribbon >> text=Delete');
await ben.waitForFunction(() => !document.activeElement?.closest('input,textarea,select'));
await ben.keyboard.press('Delete');
for (const page of [anna, ben]) {
  await warnCount(page, 1);
  await warnRowsAre(page, 'room.not_enclosed', 1);
}
const [open] = await serverWarnings();
check(
  open.kind === 'room.not_enclosed' && open.elements.length === 1 && open.elements[0] === roomId,
  "Ben deletes the room's west wall: the server lists the room as not enclosed",
);
for (const [name, page] of [['Anna', anna], ['Ben', ben]]) {
  const row = page.locator(`#browser [data-warning="${open.identity}"]`);
  check(
    (await row.getAttribute('data-severity')) === open.severity &&
      (await row.locator('.leaf').textContent()) === `${severityWords(open.severity)}: ${open.words}`,
    `${name}'s list shows it with how serious it is and the server's own sentence`,
  );
  check((await page.locator('#st-warnings').textContent()).includes('1 warning'), `and ${name}'s count reads 1 warning`);
}
await shot(anna, 'anna-warning-room');

// Anna chooses the warning: the room is selected, and she lets go of it by clicking off.
await anna.locator(`#browser [data-warning="${open.identity}"] .leaf`).click();
await anna.waitForSelector('#pv-side input[data-key="room-name"]');
check(
  (await anna.locator('#pv-side input[data-key="room-name"]').inputValue()) === 'Studio A',
  "Anna chooses the warning and the room it names is selected, in the plan's properties",
);
await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });

// Ben draws a wall where the deleted one stood, a little past each end, and the loop is closed.
const warnWallAt = async (page, from, to) => {
  await page.locator('#ribbon [role="tab"]', { hasText: 'Architecture' }).click();
  await page.click('#ribbon [data-act="dtool"][data-a="architecture:wall"]');
  const a = await onPlan(page, from[0], from[1]);
  const b = await onPlan(page, to[0], to[1]);
  await page.mouse.click(a.x, a.y);
  await page.mouse.click(b.x, b.y);
  await page.keyboard.press('Escape');
};
const lengthwiseY = gap.height > gap.width;
const middle = lengthwiseY ? gap.x + gap.width / 2 : gap.y + gap.height / 2;
const past = 40;
const closeFrom = lengthwiseY ? [middle, gap.y - past] : [gap.x - past, middle];
const closeTo = lengthwiseY ? [middle, gap.y + gap.height + past] : [gap.x + gap.width + past, middle];
await warnWallAt(ben, closeFrom, closeTo);
for (const page of [anna, ben]) {
  await warnCount(page, 0);
  await warnEmptyWords(page);
}
check((await serverWarnings()).length === 0, "Ben draws a wall across the gap: both lists are empty and so is the server's");
await ben.waitForFunction(() => window.spare.pending.length === 0 && window.spare.steps.undo?.cmd === 'wall.draw');
await ben.locator('#qat [data-act="stackundo"]:not([aria-disabled="true"])').click();
for (const page of [anna, ben]) await warnCount(page, 1);
check((await serverWarnings()).length === 1, "Ben's undo takes the wall away: the warning is back in both browsers");
await ben.locator('#qat [data-act="stackredo"]:not([aria-disabled="true"])').click();
for (const page of [anna, ben]) await warnCount(page, 0);
check((await serverWarnings()).length === 0, 'his redo puts the wall back: the warning is gone from both');

// Two walls along one line: the other kind, naming both.
const warnWalls = (page) =>
  page.evaluate(() => [...window.spare.elements.values()].filter((p) => p.el.recipe.op === 'wall.layered').map((p) => p.el.id));
const warnBefore = await warnWalls(ben);
const newWallAfter = async (known) => {
  for (let tries = 0; tries < 100; tries += 1) {
    const found = (await warnWalls(ben)).find((id) => !known.includes(id));
    if (found) return found;
    await ben.waitForTimeout(100);
  }
  throw new Error('FAILED: the wall Ben drew never came');
};
await warnWallAt(ben, [6000, 9750], [9000, 9750]);
const warnFirst = await newWallAfter(warnBefore);
for (const page of [anna, ben]) await page.waitForSelector(`#pv-canvas g.element[data-id="${warnFirst}"]`);
for (const page of [anna, ben]) await warnCount(page, 0);
pass('one new wall alone gives no warning');
await warnWallAt(ben, [7500, 9750], [10500, 9750]);
const warnSecond = await newWallAfter([...warnBefore, warnFirst]);
for (const page of [anna, ben]) {
  await warnCount(page, 1);
  await warnRowsAre(page, 'wall.overlap', 1);
}
const [overlap] = await serverWarnings();
check(
  overlap.kind === 'wall.overlap' && [...overlap.elements].sort().join() === [warnFirst, warnSecond].sort().join(),
  'Ben draws a second wall over the first: the server lists the two as overlapping',
);
for (const [name, page] of [['Anna', anna], ['Ben', ben]]) {
  const row = page.locator(`#browser [data-warning="${overlap.identity}"]`);
  check(
    (await row.locator('.leaf').textContent()) === `${severityWords(overlap.severity)}: ${overlap.words}` &&
      (await row.locator('.stack-warning-at [data-act="stackwarning"]').count()) === 2,
    `${name}'s list shows its sentence and offers each of the two walls`,
  );
}
await anna
  .locator(`#browser [data-warning="${overlap.identity}"] [data-act="stackwarning"][data-a="${overlap.elements[1]}"]`)
  .click();
await anna.waitForSelector(`#pv-canvas g.element[data-id="${overlap.elements[1]}"] .body.sel`);
pass('Anna chooses one of the two walls and it is selected in the plan');
await anna.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });

// Ben deletes the second wall: the warning goes from both; undo and redo bring it back and away.
await ben.locator('#pv-canvas').click({ position: { x: 5, y: 5 } });
const overBox = await box(ben, warnSecond);
await ben.mouse.click(overBox.x + overBox.width - 6, overBox.y + overBox.height / 2);
await ben.waitForSelector('#ribbon >> text=Delete');
await ben.waitForFunction(() => !document.activeElement?.closest('input,textarea,select'));
await ben.keyboard.press('Delete');
for (const page of [anna, ben]) {
  await warnCount(page, 0);
  await warnEmptyWords(page);
}
check((await serverWarnings()).length === 0, 'Ben deletes one of the two: the warning is gone from both browsers');
await ben.waitForFunction(() => window.spare.pending.length === 0 && window.spare.steps.undo?.cmd === 'element.delete');
await ben.locator('#qat [data-act="stackundo"]:not([aria-disabled="true"])').click();
for (const page of [anna, ben]) await warnRowsAre(page, 'wall.overlap', 1);
pass("Ben's undo puts the wall back, and the overlap is listed again in both browsers");
await ben.locator('#qat [data-act="stackredo"]:not([aria-disabled="true"])').click();
for (const page of [anna, ben]) await warnCount(page, 0);
pass('his redo takes it away again, in both');

// A fresh load agrees with the server.
const warnFresh = await signIn('anna');
await warnFresh.waitForFunction(() => document.querySelector('#st-warnings')?.dataset.count === '0');
await warnFresh.context().close();
pass('a fresh load reads the same list, none');

console.log('\n   Schedules, in the project browser');
// Ben makes a schedule of rooms from the browser's Schedules group and Anna's list shows it. Its
// table is the server's: a heading for each field, a row for each room, each value as the server
// shows it, and it is asked for again after every entry of the log, so a room Anna places, renames
// or deletes changes the rows in both browsers. Ben sorts it, refuses himself a sort key twice,
// filters it, undoes and redoes the filter; choosing a row selects that room in Anna's plan. A
// fresh load agrees. Ben touches only the schedule and Anna only the rooms, so neither takes
// what the other touched within 30 s.
const scheduleLeaf = (name) => `#browser [data-act="stackschedule"]:has(span:text-is("${name}"))`;
const table = '#stack-sched table[data-schedule-table]';
const roomIdsOf = (page) =>
  page.evaluate(() => [...window.spare.elements.values()].filter((p) => p.el.kind === 'room').map((p) => p.el.id).sort());
const rowIdsOf = (page) => page.locator(`${table} tr[data-row]`).evaluateAll((rows) => rows.map((r) => r.dataset.row));
const closeScheduleOf = async (page) => {
  await page.locator('#dlg .dlg-f [data-act="dgclose"]').click();
  await page.waitForSelector('#dlg', { state: 'hidden' });
};
// The rows of the open table, in the order shown, as [element, ...cells], once they are `want`.
const schedRows = async (page, want, label) => {
  const same = (found) => JSON.stringify(found) === JSON.stringify(want);
  const read = () =>
    page
      .locator(`${table} tbody tr`)
      .evaluateAll((rows) => rows.map((r) => [r.dataset.row, ...[...r.querySelectorAll('td')].map((td) => td.textContent)]))
      .catch(() => null);
  for (let tries = 0; tries < 100; tries += 1) {
    const found = await read();
    if (found && same(found)) return found;
    await page.waitForTimeout(100);
  }
  throw new Error(`FAILED: ${label}: the rows were ${JSON.stringify(await read())}, wanted ${JSON.stringify(want)}`);
};
const schedIds = async (page, ids, label) => {
  for (let tries = 0; tries < 100; tries += 1) {
    const found = [...(await rowIdsOf(page).catch(() => []))].sort();
    if (JSON.stringify(found) === JSON.stringify([...ids].sort())) return;
    await page.waitForTimeout(100);
  }
  throw new Error(`FAILED: ${label}: the rows were ${JSON.stringify(await rowIdsOf(page).catch(() => null))}, wanted ${JSON.stringify(ids)}`);
};
const areaOf = async (id) =>
  (await asAnna(`parameters/${id}`)).parameters?.find((p) => p.id === 'room.area')?.shown ?? '';
const stateOf = async (id) =>
  (await asAnna(`parameters/${id}`)).parameters?.find((p) => p.id === 'room.state')?.shown ?? '';

await toArchitecture(anna);
await toArchitecture(ben);
for (const page of [anna, ben]) await page.click('#browser [data-act="view"][data-a="l1"]');
check((await asAnna('schedules')).length === 0, 'the model has no schedules yet');

// Ben makes a schedule of rooms: a name and a category.
await ben.locator('#browser [data-key="new-schedule-name"]').fill('Room list');
await ben.locator('#browser [data-key="new-schedule-category"]').selectOption({ label: 'Rooms' });
await ben.locator('#browser [data-act="stacknewschedule"]').click();
await anna.waitForSelector(scheduleLeaf('Room list'));
await ben.waitForSelector(table);
const made = (await asAnna('schedules'))[0];
check(
  made.name === 'Room list' && made.category === 3300 && made.fields.join() === 'name',
  "Ben makes a schedule of rooms from the browser and the server holds it as a schedule record, listing the rooms' names",
);
pass("Anna's list shows the schedule Ben made, and Ben's browser opens its table");
const scheduleId = made.id;

// Its rows are the model's rooms: one row for each, the same IDs, the same names.
const firstRooms = await roomIdsOf(ben);
check(firstRooms.length === 1 && firstRooms[0] === roomId, "the model has the room Anna placed earlier, and only it");
await schedRows(ben, [[roomId, 'Studio A']], "Ben's table has a row for the room, with its name");
check(
  (await ben.locator(`${table} thead th`).allTextContents()).join('|') === 'Name',
  'a heading for the one field, from the server',
);

// Ben adds the room's area and state from the fields the server offers for rooms.
const offeredByServer = (await asAnna(`schedules/${scheduleId}`)).offered.map((f) => f.name);
check(
  ['Name', 'Type', 'State', 'Area', 'Perimeter', 'Volume'].every((name) => offeredByServer.includes(name)),
  `the fields the server offers for rooms include its name, type, state, area, perimeter and volume`,
);
const addField = async (name) => {
  await ben.locator('#stack-sched [data-key="sched-add-field"]').selectOption({ label: name });
  await ben.locator('#stack-sched [data-act="stackschedfieldadd"]').click();
};
// Each add sends the whole list of fields the table showed, so the next waits for the new heading.
const headings = (page, want) =>
  page.waitForFunction(
    (list) => [...document.querySelectorAll('#stack-sched table thead th')].map((th) => th.textContent).join('|') === list,
    want,
  );
await addField('Area');
await headings(ben, 'Name|Area');
await addField('State');
await headings(ben, 'Name|Area|State');
const studioArea = await areaOf(roomId);
check(/^[0-9.]+ m²$/.test(studioArea), `the server gives the room's area in the document's units, ${studioArea}`);
await schedRows(ben, [[roomId, 'Studio A', studioArea, await stateOf(roomId)]], "the row's values are the server's, with their units");
check((await stateOf(roomId)) === 'enclosed', 'the room is enclosed');

// Anna opens it from her list and sees the same table.
await anna.click(scheduleLeaf('Room list'));
await anna.waitForSelector(table);
await schedRows(anna, [[roomId, 'Studio A', studioArea, 'enclosed']], "Anna's table is the same");
await shot(anna, 'anna-schedule');
await closeScheduleOf(anna);
await closeScheduleOf(ben);

// Ben renames it in the browser: type the new name and press Enter.
await ben.click(scheduleLeaf('Room list'));
await ben.waitForSelector(table);
await closeScheduleOf(ben);
const scheduleName = `#browser input[data-key="schedule-name:${scheduleId}"]`;
await ben.waitForSelector(scheduleName);
await ben.locator(scheduleName).fill('Rooms by name');
await ben.locator(scheduleName).press('Enter');
await anna.waitForSelector(scheduleLeaf('Rooms by name'));
await ben.waitForSelector(scheduleLeaf('Rooms by name'));
check((await anna.locator(scheduleLeaf('Room list')).count()) === 0, "Ben renames the schedule and Anna's list shows the new name, not the old");
await ben.click(scheduleLeaf('Rooms by name'));
await ben.waitForSelector(table);

// Anna places a room in the other half of the building with the Room tool; Ben's table gains a row.
await anna.locator('#ribbon [role="tab"]', { hasText: 'Architecture' }).click();
await anna.click('#ribbon [data-act="dtool"][data-a="architecture:room"]');
await anna.locator('#pv-side input[data-key="room-tool-name"]').fill('Store');
await anna.locator('#pv-side input[data-key="room-tool-name"]').press('Enter');
const storeClick = await onPlan(anna, 7000, 4000);
await anna.mouse.click(storeClick.x, storeClick.y);
await anna.keyboard.press('Escape');
await ben.waitForFunction((before) => [...window.spare.elements.values()].filter((p) => p.el.kind === 'room').length === before + 1, firstRooms.length);
const storeId = (await roomIdsOf(ben)).find((id) => id !== roomId);
await schedIds(ben, [roomId, storeId], "Anna places a room: Ben's table has a row for it");
check(
  (await ben.locator(`${table} tr[data-row="${storeId}"] td`).first().textContent()) === 'Store',
  "the new row is the room's name, as the server gives it",
);
const storeState = await stateOf(storeId);
const storeArea = await areaOf(storeId);
await schedRows(
  ben,
  [[roomId, 'Studio A', studioArea, 'enclosed'], [storeId, 'Store', storeArea, storeState]].sort((a, b) => (a[0] < b[0] ? -1 : 1)),
  'the rows are in the order of the elements when nothing is sorted, with the server\'s values',
);
check(JSON.stringify(await roomIdsOf(ben)) === JSON.stringify([roomId, storeId].sort()), 'the rows are the rooms of the model, none more');
await anna.click(scheduleLeaf('Rooms by name'));
await anna.waitForSelector(table);
await schedIds(anna, [roomId, storeId], "Anna's own table has both rooms");
await shot(ben, 'ben-schedule-two-rooms');

// Ben sorts by name, up: Store comes before Studio A; then down: the other way round.
const sortBy = async (name, order) => {
  await ben.locator('#stack-sched [data-key="sched-sort-field"]').selectOption({ label: name });
  await ben.locator('#stack-sched [data-key="sched-sort-order"]').selectOption({ value: order });
  await ben.locator('#stack-sched [data-act="stackschedsortadd"]').click();
};
const namesInOrder = (page, want, label) =>
  page.waitForFunction(
    (list) => [...document.querySelectorAll('#stack-sched table tbody tr')].map((r) => r.querySelector('td').textContent).join('|') === list,
    want.join('|'),
    { timeout: 15000 },
  ).then(() => pass(label));
await sortBy('Name', 'ascending');
await namesInOrder(ben, ['Store', 'Studio A'], "Ben sorts by name, up: Store, then Studio A");
await namesInOrder(anna, ['Store', 'Studio A'], "Anna's table follows the sorting");
await ben.locator('#stack-sched [data-act="stackschedsortflip"]').click();
await namesInOrder(ben, ['Studio A', 'Store'], 'Ben turns the sorting down: Studio A, then Store');
await namesInOrder(anna, ['Studio A', 'Store'], "Anna's table follows");

// A sort key given twice is refused, in the server's words, in the Sorting part.
await sortBy('Name', 'ascending');
const sortWhere = `#stack-sched [data-problem="sched-sorting:${scheduleId}"]`;
await ben.waitForSelector(sortWhere);
const sortWords = await ben.locator(sortWhere).textContent();
check(/twice/.test(sortWords), `a sort key given twice is refused where it was made: "${sortWords}"`);
check((await ben.locator('#toast-m').textContent()) === sortWords, 'and the toast says the same');
check((await asAnna(`schedules/${scheduleId}`)).schedule.sorting.length === 1, 'the refused sorting changed nothing');
await namesInOrder(ben, ['Studio A', 'Store'], 'the rows stay as they were');

// Ben filters by name: only Studio A passes. The filter is the server's, so Anna's table follows.
await ben.locator('#stack-sched [data-key="sched-filter-field"]').selectOption({ label: 'Name' });
await ben.locator('#stack-sched [data-key="sched-filter-is"]').selectOption({ value: 'contains' });
await ben.locator('#stack-sched [data-key="sched-filter-value"]').fill('stud');
await ben.locator('#stack-sched [data-key="sched-filter-value"]').press('Enter');
await namesInOrder(ben, ['Studio A'], 'Ben filters the names to those containing "stud": one row');
await namesInOrder(anna, ['Studio A'], "Anna's table has the one row");
const heldFilters = (await asAnna(`schedules/${scheduleId}`)).schedule.filters;
check(
  heldFilters.length === 1 && heldFilters[0].field === 'name' && heldFilters[0].is === 'contains' && heldFilters[0].to.includes('stud'),
  `the server holds the filter as a field, a comparison and a value: ${JSON.stringify(heldFilters)}`,
);

// His undo takes the filter back and his redo puts it back; both browsers follow. The undo and redo
// buttons are behind the dialog's scrim, so he closes the table, presses one and opens it again.
const stepAs = async (page, act) => {
  await closeScheduleOf(page);
  await page.locator(`#qat [data-act="${act}"]:not([aria-disabled="true"])`).click();
  await page.click(scheduleLeaf('Rooms by name'));
  await page.waitForSelector(table);
};
await ben.waitForFunction(() => window.spare.pending.length === 0 && window.spare.steps.undo?.cmd === 'schedule.filters');
await stepAs(ben, 'stackundo');
await namesInOrder(ben, ['Studio A', 'Store'], "Ben's undo takes the filter back: both rooms");
await namesInOrder(anna, ['Studio A', 'Store'], "Anna's table follows the undo");
await ben.waitForFunction(() => window.spare.pending.length === 0 && window.spare.steps.redo);
await stepAs(ben, 'stackredo');
await namesInOrder(ben, ['Studio A'], "Ben's redo puts the filter back: one row");
await namesInOrder(anna, ['Studio A'], "Anna's table follows the redo");

// Ben takes the filter off with its Remove button.
await ben.locator('#stack-sched [data-filter-row] [data-act="stackschedfilteroff"]').click();
await namesInOrder(ben, ['Studio A', 'Store'], 'Ben removes the filter: both rooms again');
await namesInOrder(anna, ['Studio A', 'Store'], "Anna's table follows");

// Anna chooses the Store row: the room is selected in her plan, and the palette shows it.
await anna.locator(`${table} tr[data-row="${storeId}"]`).click();
await anna.waitForSelector('#pv-side input[data-key="room-name"][value="Store"]', { state: 'attached' });
await anna.waitForSelector(`${table} tr[data-row="${storeId}"][aria-selected="true"]`);
pass('Anna chooses the Store row and the room is selected: her properties palette shows it');
await closeScheduleOf(anna);

// Anna renames the selected room in the palette; the name changes in Ben's table, and the
// order with it (Ben's sorting is down).
await anna.locator('#pv-side input[data-key="room-name"]').fill('Pantry');
await anna.locator('#pv-side input[data-key="room-name"]').press('Enter');
await namesInOrder(ben, ['Studio A', 'Pantry'], "Anna renames the room: Ben's table has the new name");

// Anna deletes the room; the row goes from Ben's table.
await anna.locator('#pv-side [data-key="room-delete"]').click();
await schedIds(ben, [roomId], "Anna deletes the room: Ben's table has one row again");
await schedRows(ben, [[roomId, 'Studio A', studioArea, 'enclosed']], 'and it is the first room, unchanged');
check((await roomIdsOf(ben)).join() === roomId, "the rows are the model's rooms");

// A fresh load agrees: the same schedule in the list, the same rows.
const scheduleFresh = await signIn('anna');
await scheduleFresh.waitForSelector(scheduleLeaf('Rooms by name'));
await scheduleFresh.click(scheduleLeaf('Rooms by name'));
await scheduleFresh.waitForSelector(table);
await schedRows(scheduleFresh, [[roomId, 'Studio A', studioArea, 'enclosed']], 'a fresh load lists the schedule and gives the same row');
await scheduleFresh.context().close();

// Anna has the table open; Ben deletes the schedule from the browser, and Anna's list and her open
// table follow.
await anna.click(scheduleLeaf('Rooms by name'));
await anna.waitForSelector(table);
await closeScheduleOf(ben);
await ben.locator(`#browser [data-act="stackscheddelete"][data-a="${scheduleId}"]`).click();
await anna.waitForFunction(() => /deleted/.test(document.querySelector('#stack-sched')?.textContent ?? ''));
await closeScheduleOf(anna);
await anna.waitForFunction((name) => ![...document.querySelectorAll('#browser [data-act="stackschedule"] span')].some((s) => s.textContent === name), 'Rooms by name');
check((await asAnna('schedules')).length === 0, 'Ben deletes the schedule: Anna\'s list is empty again and the server holds none');
for (const [name, page] of [['Anna', anna], ['Ben', ben]]) {
  const mismatched = await page.evaluate(() => window.spare.mismatches);
  check(mismatched.length === 0, `every body ${name}'s page built, after the schedules, matches the server's`);
}

await browser.close();
console.log('\nThe browser checks passed.');
