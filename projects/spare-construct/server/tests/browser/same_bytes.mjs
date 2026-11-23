// The kernel's package for the browser gives the same bytes as the server's own build. It loads the
// package the page loads (app/pkg-kernel, wasm-bindgen's web target), with the few lines Node needs
// in place of the page's fetch, builds every case of shapes-kernel/tests/cases.json through the
// functions the page calls, and compares each answer, byte for byte, with what
// shapes-kernel/examples/answers.rs printed natively for the same cases. No browser needed.
//
//   node same_bytes.mjs <native answers file> [cases.json] [package folder]
//
// The answers include every display mesh, so a float that differs between native and WebAssembly,
// or an order that does, shows here even though a body's name would not show it (the name is made
// of what the engine was asked, not of what it gave: the interim register's row).
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const server = resolve(here, '../..');
const [nativeFile, casesFile = `${server}/shapes-kernel/tests/cases.json`, pkg = `${server}/../app/pkg-kernel`] =
  process.argv.slice(2);
if (!nativeFile) {
  console.error('usage: node same_bytes.mjs <native answers file> [cases.json] [package folder]');
  process.exit(2);
}

const shapes = await import(pathToFileURL(`${pkg}/construct_shapes_kernel.js`).href);
await shapes.default({ module_or_path: readFileSync(`${pkg}/construct_shapes_kernel_bg.wasm`) });

// The same lines, in the same order, as examples/answers.rs writes them.
const { cases } = JSON.parse(readFileSync(casesFile, 'utf8'));
let browser = `edition\t${shapes.edition()}\n`;
for (const c of cases) {
  const answer = c.build
    ? shapes.build(JSON.stringify(c.build))
    : shapes.build_hosting(JSON.stringify(c.host), JSON.stringify(c.hosted));
  browser += `${c.name}\t${answer}\n`;
}

const native = readFileSync(nativeFile, 'utf8');
const lines = (text) => text.split('\n').filter((line) => line !== '');
const [ours, theirs] = [lines(browser), lines(native)];
let failed = ours.length !== theirs.length;
if (failed) console.error(`${ours.length} answers here, ${theirs.length} natively`);
for (let i = 0; i < Math.max(ours.length, theirs.length); i += 1) {
  const [a = '', b = ''] = [ours[i], theirs[i]];
  const name = (a || b).split('\t')[0];
  if (a === b) {
    console.log(`  ok    ${name}: ${Buffer.byteLength(a)} bytes the same`);
    continue;
  }
  failed = true;
  let at = 0;
  while (at < a.length && a[at] === b[at]) at += 1;
  console.error(`  FAIL  ${name}: differs from character ${at}`);
  console.error(`        here:     …${a.slice(Math.max(0, at - 40), at + 80)}`);
  console.error(`        natively: …${b.slice(Math.max(0, at - 40), at + 80)}`);
}
if (failed || browser !== native) {
  console.error('The browser’s build differs from the native build.');
  process.exit(1);
}
console.log(`Every answer is the same bytes in the browser’s build as natively (${Buffer.byteLength(native)} bytes).`);
