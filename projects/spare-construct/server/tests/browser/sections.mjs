// Runs one section of two_browsers.mjs, or saves the state each section leaves. Used only when
// SAVE_SECTIONS or START_AT is set; with neither set, two_browsers.mjs runs exactly as it is
// written and never loads this file.
//
//   SAVE_SECTIONS=<folder>  after each section, save what is needed to rebuild the state it leaves
//   START_AT=<label>        run from that section, on the state saved after the section before it
//   ONLY=1                  with START_AT: run that section alone, then stop
//   START_AT=?              list the section labels and stop
//
// A section is what follows a heading line, console.log('\n<label>'), at the top level of the
// test. The test is read here as source and cut into sections with a parser (acorn, MIT); the
// sections themselves are never edited. For the sections before the start, the new copy of the
// test (written beside the test and run in its place, line numbers unchanged) keeps their helper
// functions, signs the people in again where a later section uses their page, and sets every other
// top-level value from what the saved state holds; their actions are not run.
//
// The server's state is saved and restored through its own backup and restore commands
// (spare-server backup, restore: every key, so the edit logs with them, and every file; see
// src/backup.rs), through STACK_SAVE and STACK_RESTORE, which the runner sets. Those commands take
// a state's name last.
//
// A saved state carries a stamp: the text of the test up to the end of its section, and the code
// it was made from (SECTION_CODE, which the runner sets: the commit the branch last took from main,
// and the engine). A state whose stamp is not the current one is refused, in words.
import { execSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parse } from 'acorn';

const fail = (words) => {
  throw new Error(`FAILED: ${words}`);
};
const sha = (text) => createHash('sha256').update(text).digest('hex').slice(0, 16);
const stateName = (index) => `s${String(index).padStart(2, '0')}`;
const lines = (text) => (text.match(/\n/g) || []).length;
const keepLines = (text, count = lines(text)) => '\n'.repeat(count);

const isLog = (node) =>
  node.type === 'ExpressionStatement' &&
  node.expression.type === 'CallExpression' &&
  node.expression.callee.type === 'MemberExpression' &&
  node.expression.callee.object.name === 'console' &&
  node.expression.callee.property.name === 'log';

// The heading's words when this statement is a heading, else null.
function heading(node) {
  if (!isLog(node)) return null;
  const first = node.expression.arguments[0];
  let text = null;
  if (first?.type === 'Literal' && typeof first.value === 'string') text = first.value;
  if (first?.type === 'TemplateLiteral' && first.expressions.length === 0) text = first.quasis[0].value.cooked;
  return text !== null && text.startsWith('\n') ? text.trim() : null;
}

const isFunction = (init) => init && (init.type === 'ArrowFunctionExpression' || init.type === 'FunctionExpression');
const isSignIn = (init) =>
  init?.type === 'AwaitExpression' &&
  init.argument.type === 'CallExpression' &&
  init.argument.callee.type === 'Identifier' &&
  init.argument.callee.name === 'signIn';

function bindings(pattern, into = []) {
  if (pattern.type === 'Identifier') into.push(pattern.name);
  else if (pattern.type === 'ObjectPattern') for (const p of pattern.properties) bindings(p.value ?? p.argument, into);
  else if (pattern.type === 'ArrayPattern') for (const p of pattern.elements) if (p) bindings(p, into);
  else if (pattern.type === 'AssignmentPattern') bindings(pattern.left, into);
  else if (pattern.type === 'RestElement') bindings(pattern.argument, into);
  return into;
}

// What a state keeps of a value: plain data only (numbers, text, lists and plain objects of
// those). Pages, locators, maps and the like are left out: a section that needs one sets it up.
function plain(value, depth = 0) {
  if (value === null || ['string', 'number', 'boolean'].includes(typeof value)) return { value };
  if (depth > 12) return null;
  if (Array.isArray(value)) {
    const items = value.map((v) => plain(v, depth + 1));
    return items.every(Boolean) ? { value: items.map((i) => i.value) } : null;
  }
  if (typeof value === 'object' && Object.getPrototypeOf(value) === Object.prototype) {
    const out = {};
    for (const [k, v] of Object.entries(value)) {
      const kept = plain(v, depth + 1);
      if (!kept) return null;
      out[k] = kept.value;
    }
    return { value: out };
  }
  return null;
}

function listSections(sections) {
  return sections.map((s, i) => `   ${String(i).padStart(2)}  ${s.label}`).join('\n');
}

function choose(sections, wanted) {
  if (/^#\d+$/.test(wanted)) {
    const found = sections[Number(wanted.slice(1))];
    if (!found) fail(`START_AT ${wanted}: there is no such section. The sections are:\n${listSections(sections)}`);
    return found.index;
  }
  const lower = wanted.toLowerCase();
  const exact = sections.filter((s) => s.label.toLowerCase() === lower);
  const found = exact.length ? exact : sections.filter((s) => s.label.toLowerCase().includes(lower));
  if (found.length === 1) return found[0].index;
  const words = found.length ? 'matches several sections, name one more closely (or use #<number>)' : 'matches no section';
  fail(`START_AT "${wanted}" ${words}. The sections are:\n${listSections(found.length ? found : sections)}`);
}

export async function runSections(testUrl) {
  const testFile = fileURLToPath(testUrl);
  const source = fs.readFileSync(testFile, 'utf8');
  const body = parse(source, { ecmaVersion: 'latest', sourceType: 'module' }).body;

  // Headings cut the test into sections; the last heading is the closing line, not a section.
  const marks = [];
  body.forEach((node, at) => {
    const label = heading(node);
    if (label !== null) marks.push({ at, label, start: node.start });
  });
  const closing = marks.at(-1);
  if (!closing || !closing.label.startsWith('The browser checks passed')) fail('the test has lost its closing line');
  const sections = marks.slice(0, -1).map((m, index) => ({
    index,
    label: m.label,
    start: m.start,
    end: marks[index + 1].start,
    nodes: body.slice(m.at, marks[index + 1].at),
  }));

  const wanted = process.env.START_AT;
  if (wanted === '?') {
    console.log(`The sections of the test (START_AT=<words of the label> or START_AT=#<number>):\n${listSections(sections)}`);
    return;
  }
  const first = wanted ? choose(sections, wanted) : 0;
  const last = wanted && process.env.ONLY ? first : sections.length - 1;
  const saveTo = process.env.SECTION_STATES;
  const saving = Boolean(process.env.SAVE_SECTIONS);
  if ((saving || wanted) && !saveTo) fail('the runner did not say where the states are kept (SECTION_STATES)');
  const code = process.env.SECTION_CODE || 'unknown';

  // The state to start from, and whether it is the right one.
  let kept = {};
  if (first > 0) {
    const before = sections[first - 1];
    const folder = path.join(saveTo, stateName(before.index));
    const metaFile = path.join(folder, 'meta.json');
    const say = (why) =>
      fail(
        `START_AT "${sections[first].label}" needs the state saved after "${before.label}", ${why}. ` +
          'Run the whole test once with SAVE_SECTIONS set to save the states (a full run is the only way to make them).',
      );
    if (!fs.existsSync(metaFile)) say(`and there is none in ${saveTo}`);
    const meta = JSON.parse(fs.readFileSync(metaFile, 'utf8'));
    if (meta.code !== code) say(`but it was saved from other code (${meta.code}; this run is ${code})`);
    if (meta.testHash !== sha(source.slice(0, before.end))) {
      say('but the test up to and including that section has changed since it was saved');
    }
    kept = JSON.parse(fs.readFileSync(path.join(folder, 'vars.json'), 'utf8'));
    for (const command of ['STACK_RESTORE', 'STACK_START']) if (!process.env[command]) fail(`${command} is not set`);
    console.log(`-- restoring the state saved after "${before.label}" (${meta.savedAt})`);
    execSync(`${process.env.STACK_RESTORE} ${stateName(before.index)}`, { stdio: 'inherit' });
    execSync(process.env.STACK_START);
    const until = Date.now() + 90_000;
    for (;;) {
      const ok = await fetch(`${process.env.STACK_URL || 'http://localhost:8080'}/health`).then((r) => r.ok, () => false);
      if (ok) break;
      if (Date.now() > until) fail('the server did not answer after the state was restored');
      await new Promise((r) => setTimeout(r, 500));
    }
  } else if (saving) {
    fs.mkdirSync(saveTo, { recursive: true });
    for (const old of fs.readdirSync(saveTo)) if (/^s\d\d$/.test(old)) fs.rmSync(path.join(saveTo, old), { recursive: true });
  }
  if (saving && !process.env.STACK_SAVE) fail('STACK_SAVE is not set');

  // What a later section may use from the ones before the start: a name that a kept section
  // declared, and that the sections to run mention.
  const runText = sections.slice(first, last + 1).map((s) => source.slice(s.start, s.end)).join('\n');
  const mentioned = (name) => new RegExp(`(?<![\\w$.])${name.replace(/\$/g, '\\$')}(?![\\w$])`).test(runText);

  // A section before the start, cut down to what later sections stand on.
  const cutDown = (section) => {
    let out = '';
    let at = section.start;
    for (const node of section.nodes) {
      out += keepLines(source.slice(at, node.start));
      const text = source.slice(node.start, node.end);
      at = node.end;
      let kept = '';
      if (node.type === 'FunctionDeclaration' || node.type === 'ClassDeclaration') kept = text;
      else if (node.type === 'VariableDeclaration') {
        const parts = [];
        for (const d of node.declarations) {
          const own = source.slice(d.start, d.end);
          if (isFunction(d.init) || (isSignIn(d.init) && bindings(d.id).every(mentioned))) kept += `${node.kind} ${own};`;
          else for (const name of bindings(d.id)) parts.push(`${name} = __sections.kept[${JSON.stringify(name)}]`);
        }
        if (parts.length) kept += `let ${parts.join(', ')};`;
      }
      out += kept + keepLines(text, Math.max(0, lines(text) - lines(kept)));
    }
    return out + keepLines(source.slice(at, section.end));
  };

  // The names a section leaves, for a state: every top-level value declared so far that is not a function.
  const declared = (upTo) => {
    const names = [];
    for (const s of sections.slice(0, upTo + 1)) {
      for (const node of s.nodes) {
        if (node.type !== 'VariableDeclaration') continue;
        for (const d of node.declarations) if (!isFunction(d.init)) names.push(...bindings(d.id));
      }
    }
    return [...new Set(names)];
  };

  let text = source.slice(0, sections[0].start);
  for (const s of sections) {
    if (s.index < first) text += cutDown(s);
    else if (s.index <= last) text += source.slice(s.start, s.end);
    else text += keepLines(source.slice(s.start, s.end));
    if (s.index >= first && s.index <= last) {
      text += `await __sections.end(${s.index}, ${JSON.stringify(s.label)}, () => ({ ${declared(s.index).join(', ')} }));`;
    }
  }
  text += source.slice(closing.start);

  let clock = Date.now();
  globalThis.__sectionsChild = true;
  globalThis.__sections = {
    kept,
    async end(index, label, values) {
      const took = ((Date.now() - clock) / 1000).toFixed(1);
      console.log(`      -- section "${label}" took ${took} s`);
      if (saving) {
        const name = stateName(index);
        const folder = path.join(saveTo, name);
        fs.rmSync(folder, { recursive: true, force: true });
        fs.mkdirSync(folder, { recursive: true });
        const vars = {};
        for (const [k, v] of Object.entries(values())) {
          const keep = plain(v);
          if (keep) vars[k] = keep.value;
        }
        fs.writeFileSync(path.join(folder, 'vars.json'), JSON.stringify(vars));
        execSync(`${process.env.STACK_SAVE} ${name}`, { stdio: 'inherit' });
        const meta = { index, label, code, testHash: sha(source.slice(0, sections[index].end)), savedAt: new Date().toISOString() };
        fs.writeFileSync(path.join(folder, 'meta.json'), JSON.stringify(meta, null, 1));
      }
      clock = Date.now();
    },
  };
  // Beside the test, so Node finds the same modules; the same name with ".sections" keeps the
  // line numbers of a failure the same as the test's.
  const copy = testFile.replace(/\.mjs$/, '.sections.mjs');
  fs.writeFileSync(copy, text);
  try {
    await import(pathToFileURL(copy).href);
  } finally {
    fs.rmSync(copy, { force: true });
  }
}
