// Two real browsers on the real page: separate sign-ins, real mouse drags. Needs the stack running
// on a fresh seed; scripts/run-browser-test.sh sets that up.
//   STACK_URL    the server, default http://localhost:8080
//   BROWSER      a browser channel, msedge or chrome, or bundled for Playwright's own; default msedge
//   STACK_KILL   a command that kills the server the hard way
//   STACK_START  a command that starts it again
//   SHOTS        a folder for screenshots
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

async function signIn(person) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
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

const box = (page, id) => page.locator(`g.element[data-id="${id}"] rect.body`).boundingBox();
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
}

console.log('\n4. Arch v12 is shared, bridged and consumed, on the page');
await anna.click('[data-act="stackshare"][data-a="arch-model"]');
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
await anna.keyboard.press('Escape');
const wallBoxAgain = await box(anna, wallId);
await anna.mouse.click(wallBoxAgain.x + 20, wallBoxAgain.y + wallBoxAgain.height / 2);
await anna.waitForSelector('#ribbon >> text=Delete');
await anna.keyboard.press('Delete');
await ben.waitForFunction(
  (id) => !document.querySelector(`g.element[data-id="${id}"]`),
  wallId,
);
pass("deleting it takes it off Ben's screen");

console.log('\n   The 3D view');
await anna.click('#browser [data-act="go"][data-a="3d"]');
await anna.waitForSelector('#iso-canvas canvas');
await anna.waitForTimeout(500);
await shot(anna, 'anna-3d');
pass('the boxes draw in 3D');
await browser.close();
console.log('\nThe browser checks passed.');
