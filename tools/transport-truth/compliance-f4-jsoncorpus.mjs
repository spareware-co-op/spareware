// A corpus of JSON texts, good and bad, with what the prototype's JSON.parse made of each (the value as JSON text, or the error message
// V8 gives): the Rust port reads the other party's file with its own JSON reader and must give the same value and the same message.
// Deterministic: a seeded stream picks the mutations. Texts with a lone surrogate half are left out (a Rust string cannot hold one).
const seeds = [
  '{"a":1,"b":[1,2,{"c":"x"}],"d":null,"e":true,"f":false,"g":-1.5e3}',
  '[1,2,3]',
  '{"settings":{"growth":3,"hvPct":8,"rates":{"office":{"AM":{"rate":3,"inPct":80}}}}}',
  '{"key":"value with \\"escapes\\" and \\u00e9 and \\n and \\/ and \\\\"}',
  '[ ]',
  '{ }',
  '[[[]]]',
  '  {"a" : 1 , "b" : [ 1 , 2 ] }  ',
  '{"x":"line1\\nline2\\ttab"}',
  '[0, -0, 0.5, 10, 1e5, 1E-5, 1.0, 123456789012, 0e0, -1E+2]',
  '{"unicode":"é😀 text"}',
  '{\r\n  "growth": 3,\r\n  "hvPct": 8\r\n}',
  '{\n  "growth": 3,\n  "nested": {\n    "a": [true, false, null]\n  }\n}',
  '{"a":{"b":{"c":{"d":{"e":1}}}},"list":[{"x":1},{"y":[2,3]}]}',
  '[{"growth":1},{"hvPct":2},{"cycle":3}]',
  '"just a string"',
  '12.5',
  'true',
  '{"empty":"","e2":{},"e3":[]}',
  '{"settings":{"growth":1.5,"hvPct":6,"cycle":100,"satFlow":1800,"baseYear":2026,"pcuHV":2.3,"peakToDaily":10}}',
  '{"1":1,"0":2,"b":3,"2":4,"a":5,"b":6,"__proto__":7}',
  '{"a":"\\ud83d\\ude00 pair","b":"\\u0041\\u00e9\\u4e2d"}',
  '[1e999,-1e999,1e-999,0.1e1,123456789012345678901234567890,1.7976931348623157e308,5e-324]',
];
const tokens = ['{', '}', '[', ']', ':', ',', '"', '\\', '-', '+', '.', 'e', 'E', '0', '1', '9', ' ', '\n', '\r', '\r\n', '\t', 'x', 't', 'f', 'n', 'u', '/', 'é', '😀', ' ', ' ', '\u0001', '\u001f', 'true', 'false', 'null',
  'tru', 'nul', 'fals', 'NaN', 'Infinity', '-Infinity', 'undefined', '01', '1.', '1e', '1e+', '.5', '"\\u12"', '"\\uZZZZ"', '"\\q"', '\\u0041', '\\"', "'", '/*', '//', '﻿', '00', '-0', '0x1F',
  '\u0000', '\u007f', '\u0080', 'ÿ', 'Ā', '\\u', '\\u0', 'ab', 'abcdefghijklmnopqrstuvwxyz'];

const mulberry = a => () => { a |= 0; a = (a + 0x6D2B79F5) | 0; let t = Math.imul(a ^ (a >>> 15), 1 | a); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
// a lone surrogate half cannot be held in a Rust string
const lone = s => /[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/.test(s);

// whether a parsed value holds a lone surrogate half in a text or a key
const loneIn = v => (typeof v === 'string' ? lone(v) : Array.isArray(v) ? v.some(loneIn) : v && typeof v === 'object' ? Object.entries(v).some(([k, x]) => lone(k) || loneIn(x)) : false);

export function jsonCorpus() {
  const rnd = mulberry(20261021);
  const pick = a => a[Math.floor(rnd() * a.length)];
  const texts = new Set();
  const add = s => { if (typeof s === 'string' && !lone(s)) texts.add(s); };
  for (const s of seeds) {
    add(s);
    for (let i = 0; i <= s.length; i++) add(s.slice(0, i));                 // every truncation
    for (let i = 0; i < s.length; i++) add(s.slice(0, i) + s.slice(i + 1));  // every single deletion
  }
  for (let n = 0; n < 6000; n++) {
    let s = pick(seeds);
    const ops = 1 + Math.floor(rnd() * 3);
    for (let k = 0; k < ops; k++) {
      const i = Math.floor(rnd() * (s.length + 1));
      switch (Math.floor(rnd() * 6)) {
        case 0: s = s.slice(0, i) + s.slice(i + 1); break;
        case 1: s = s.slice(0, i) + pick(tokens) + s.slice(i); break;
        case 2: s = s.slice(0, i) + pick(tokens) + s.slice(i + 1); break;
        case 3: s = s.slice(0, i); break;
        case 4: s = s.slice(0, i) + s.charAt(i) + s.slice(i); break;
        case 5: s = s.slice(0, i) + s.slice(i + 1, i + 2) + s.charAt(i) + s.slice(i + 2); break;
      }
    }
    add(s);
  }
  // a backslash before a character beyond Latin-1, and before the end of the text
  for (const ch of ['Ā', ' ', '﻿', '中', 'é', 'ÿ', '\u007f', '\u0000']) {
    add('{"a":"x\\' + ch + 'y"}');
    add('["\\' + ch + '"]');
    add('["a very long string of text before \\' + ch + ' and after it"]');
  }
  add('["\\'); add('["abc\\'); add('{"a":"b\\');
  // padded, so the 21-character edge of the message context is crossed at every position
  for (let len = 15; len <= 26; len++) for (const bad of ['x', '}', ']', ',', ':', 't', '1x']) {
    const body = '"' + 'a'.repeat(Math.max(0, len - 4)) + '":';
    add('{' + body + bad + '}');
    add('[' + 'a'.repeat(len - 3) + bad);
  }
  const cases = [];
  for (const s of [...texts].sort()) {
    try { const v = JSON.parse(s); if (!loneIn(v)) cases.push({ s, ok: JSON.stringify(v) }); } catch (e) { if (!lone(e.message)) cases.push({ s, err: e.message }); }
  }
  return cases;
}
