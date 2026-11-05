// Checks the designer's Design tab stays in order: every option of every visual lands in one of the
// agreed parts (SECTION_ORDER, the same names for every kind), no setting shows in two parts, and
// every setting the chart drawing reads has a control. node dev/checks/inspector.mjs
import { readFileSync } from 'node:fs';

// the schema module pulls in text measuring, which wants a canvas: a stand-in is enough here
globalThis.document = { createElement: () => ({ getContext: () => ({ measureText: (t) => ({ width: String(t).length * 5 }), font: '' }) }) };
globalThis.localStorage = { getItem: () => null, setItem: () => {} };

const root = new URL('../../', import.meta.url);
const { SCHEMA, SECTION_ORDER, sectionOf } = await import(new URL('ui/vizschema.js', root));

const problems = [];
// every combination of the facts that decide which options apply
const flags = ['horizontal', 'share', 'scale', 'multi', 'percent', 'lines', 'bars'];
const contexts = [];
for (let m = 0; m < 1 << flags.length; m++) {
  const c = { dims: m & 1 ? 1 : 0, barSeries: m & 2 ? 2 : 1, noSpark: false, noDelta: false };
  flags.forEach((f, i) => { c[f] = !!(m & (1 << i)); });
  contexts.push(c);
}

const keysInSchema = new Set();
for (const kind of Object.keys(SCHEMA)) {
  const where = new Map();
  for (const c of contexts) {
    // a chart type of its own (a range bar, a ranked list, a 100% bar) may place a key apart
    const type = c.scale ? 'range' : c.share ? '100%' : c.horizontal ? 'horizontal' : 'columns';
    for (const g of SCHEMA[kind]({ ...c, kind })) {
      for (const it of g.items) {
        if (!it.key) continue;
        keysInSchema.add(it.key);
        const sec = sectionOf(it, g.title);
        if (!SECTION_ORDER.includes(sec)) problems.push(`${kind}: “${it.label}” (${it.key}) goes to “${sec}”, which isn't one of the agreed parts`);
        const before = where.get(`${type}/${it.key}`);
        if (before && before !== sec) problems.push(`${kind} (${type}): ${it.key} shows in both “${before}” and “${sec}”`);
        where.set(`${type}/${it.key}`, sec);
      }
    }
  }
}

// settings the drawing reads (S.key) that no control sets; those set elsewhere in the inspector are listed
const drawn = new Set([...readFileSync(new URL('ui/charts.js', root), 'utf8').matchAll(/\bS\.([a-zA-Z_][a-zA-Z0-9_]*)/g)].map((m) => m[1]));
const elsewhere = new Set(['icons', 'iconsAuto', 'iconsOff', 'iconShape', 'iconFill', 'iconColor', 'iconStroke', 'iconStrokeWidth', 'colors', 'palette', 'look', 'orientation', 'valueSize', 'kpiGroup', 'seriesOpts']);
for (const k of drawn) if (!keysInSchema.has(k) && !elsewhere.has(k)) problems.push(`charts.js reads S.${k}, but the inspector has no control for it`);

[...new Set(problems)].forEach((p) => console.log('✗', p));
if (problems.length) process.exit(1);
console.log(`✓ ${Object.keys(SCHEMA).length} kinds: every option in its part, none twice, every drawn setting has a control`);
