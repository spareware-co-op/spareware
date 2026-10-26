// Records Node's Math.exp, sin, cos, asin and pow on a spread of arguments, as the exact bits of each answer, so
// the Rust port of V8's maths (projects/spare-construct/crates/construct-transport/src/jsmath.rs) can be checked
// bit for bit (tests/jsmath_truth.rs). Writes expected/jsmath-cases.json: { exp: [[x, result], ...], sin, cos,
// asin, pow: [[x, y, result], ...] } with every number as 16 hex digits (the IEEE 754 bit pattern).
import { writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const dv = new DataView(new ArrayBuffer(8));
const hex = x => { dv.setFloat64(0, x); return dv.getBigUint64(0).toString(16).padStart(16, '0'); };
let seed = 20261021;
const rnd = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed / 4294967296; };

const out = { exp: [], sin: [], cos: [], asin: [], pow: [] };
const edge = [0, -0, 1, -1, 0.5, -0.5, 2, 1e-300, -1e-300, 5e-324, 1e-10, 709, 709.78, 710, -745, -746, 1e300, -1e300,
  Math.PI, Math.PI / 2, Math.PI / 4, 3 * Math.PI / 4, 1e6, 8.2e5, NaN, Infinity, -Infinity];
const add = (name, fn, xs) => { for (const x of xs) out[name].push([hex(x), hex(fn(x))]); };

// the arguments the engine uses: gravity weights and gap-acceptance capacities (exp of small negatives), angles of a
// map (radians of latitudes and half the difference of two), haversine roots (asin of small numbers)
const expArgs = [...edge];
for (let i = 0; i < 700; i++) expArgs.push(-rnd() * (i % 4 === 0 ? 30 : i % 4 === 1 ? 700 : 3), rnd() * (i % 5 === 0 ? 5 : 0.7));
add('exp', Math.exp, expArgs);
const trigArgs = [...edge];
for (let i = 0; i < 700; i++) trigArgs.push((rnd() - 0.5) * (i % 3 === 0 ? 0.01 : i % 3 === 1 ? 6.4 : 400), -(0.5 + rnd() * 0.4), (rnd() - 0.5) * 1e-4);
add('sin', Math.sin, trigArgs);
add('cos', Math.cos, trigArgs);
const asinArgs = [...edge.filter(x => Math.abs(x) <= 1.0000001), 0.975, 0.9751, 0.5, 0.4999999, 0.9999999];
for (let i = 0; i < 700; i++) asinArgs.push(Math.sqrt(rnd() * 0.0003), (rnd() * 2 - 1) * (i % 2 ? 1 : 0.4), Math.sqrt(rnd()));
add('asin', Math.asin, asinArgs);
// pow: the engine's BPR term (a ratio times 4), the junction formulas' exponents, special exponents and bases
const exps = [4, 4, 4, 3.7, 0.7, 1.5, 2, 1, 0.5, 0, -1, -2, 6, 10, -0.5, 4.5, 3, 5];
for (let i = 0; i < 2700; i++) {
  const x = rnd() * (i % 5 === 0 ? 20 : 3), y = exps[i % exps.length];
  out.pow.push([hex(x), hex(y), hex(Math.pow(x, y))]);
}
for (const x of edge) for (const y of [0, 1, 2, 0.5, -1, NaN, Infinity, -Infinity, 3, 4, 0.7, -2.5]) out.pow.push([hex(x), hex(y), hex(Math.pow(x, y))]);
writeFileSync(join(here, 'expected', 'jsmath-cases.json'), JSON.stringify(out));
console.log('jsmath-cases written', Object.fromEntries(Object.entries(out).map(([k, v]) => [k, v.length])));
