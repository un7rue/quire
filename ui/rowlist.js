// A bar chart's "List" layout drawn by the table engine: one row per
// category with its name, a bar and the value (other values under it).
// The same table settings are what "Switch to Table" keeps, so a list and
// a table never need two implementations of bars, marks or highlights.

import { mixHex, textWidth } from './util.js';
import { formatCell } from './table.js';

/** Columns a list adds to the data: the bar (a copy of the first value) and the rank. */
export const LIST_BAR = 'mbar';
export const LIST_RANK = 'mrank';

/**
 * The table a list draws as. `S` are the bar chart's resolved settings,
 * `b` the body box, `font` the chart font. Returns the table options, the
 * data with the bar and rank columns added, the row layout and the cell font.
 */
export function listTable(st, S, data, b, font) {
  const th = st.doc.theme;
  const cols = data?.columns || [];
  const dim = cols.find((c) => c.role === 'dim');
  const ms = cols.filter((c) => c.role === 'measure');
  const m0 = ms[0];
  if (!dim || !m0) return null;
  const j0 = cols.indexOf(m0);
  const rows = data.rows || [];
  const n = Math.max(1, rows.length);
  const nameSize = Math.max(4, font.size + 1.5), valK = (font.size + 1) / nameSize;
  const rowH = Math.min(b.h / n, S.subLine ? 34 : 30);
  const hl = S.rankHighlight === false ? null : S.highlight || (rows[0] ? String(rows[0][cols.indexOf(dim)]) : null);
  // the values at the right can be left out (the bars alone, e.g. when the subtitle gives the figures)
  const showVals = S.listValues !== false;
  const extras = S.subLine && showVals ? ms.slice(1) : [];

  const valueCfg = { signColor: true, signed: S.signed, weight: 800, size: valK, decimals: S.decimals, compact: S.compact !== 'off', sub: extras.length ? extras.map((c) => c.key) : undefined, subWeight: 700 };
  const extraCfg = Object.fromEntries(ms.slice(1).map((c) => [c.key, { hidden: true, signed: c.format === 'percent', decimals: c.format === 'percent' ? (S.decimals ?? 1) : 0, compact: S.compact !== 'off' }]));
  // widths: the value column fits its text, the name its longest name (with marks)
  const valW = !showVals ? 6 : Math.max(...rows.map((r) => textWidth(formatCell(r[j0], m0, valueCfg, th.currency), nameSize * valK, 800, font.family)),
    ...rows.map((r) => (extras.length ? textWidth(extras.map((c) => formatCell(r[cols.indexOf(c)], c, extraCfg[c.key], th.currency)).filter(Boolean).join(' · '), nameSize * valK * 0.82, 400, font.family) : 0))) + 20;
  const rankW = S.rankNumbers ? Math.max(22, nameSize * 2.6) : 0;
  const nameW0 = Math.max(...rows.map((r) => textWidth(String(r[cols.indexOf(dim)] ?? ''), nameSize, 700, font.family))) + (S.markBest ? nameSize * 2 : 0) + 14;
  let barW, nameW;
  if (S.trackFill) {
    nameW = Math.min(b.w * 0.42, nameW0);
    barW = Math.max(20, b.w - rankW - nameW - valW);
  } else {
    barW = Math.min(b.w * 0.36, 160) + 10;
    nameW = Math.max(20, b.w - rankW - barW - valW);
  }
  // the chart's tint (a lighter fill) carries over to the bars
  const tint = (c) => (S.tint > 0 ? mixHex(c, '#ffffff', S.tint) : c);
  const pct = (x) => Math.round((x / b.w) * 1000) / 10;
  const barH = S.barThick > 0 ? Math.min(S.barThick, rowH - 2) : S.trackFill ? Math.min(rowH * 0.3, 9) : Math.min(rowH * 0.42, 12) - 2;

  const table = {
    hideHead: true, totals: false, paginate: false, lines: S.rowLines ? 'rows' : 'none', lineColor: th.grid,
    order: [S.rankNumbers && LIST_RANK, dim.key, LIST_BAR, showVals && m0.key].filter(Boolean),
    cols: {
      [LIST_RANK]: { align: 'center', color: th.muted, weight: 700, size: 0.75, badge: S.colorUp, width: pct(rankW), decimals: 0 },
      [dim.key]: { weight: (S.nameBold ?? S.trackFill) ? 700 : 500, width: pct(nameW) },
      [LIST_BAR]: { label: ' ', width: pct(barW), bar: {
        on: true, only: true, base: S.barBase === 'size' ? null : 'zero', color: tint(S.colorUp), negColor: tint(S.colorDown), h: barH, opacity: 1,
        stroke: S.outline ? S.outlineColor || 'auto' : null, strokeWidth: S.outlineWidth,
        track: S.track ? undefined : false, trackFill: S.track ? mixHex(th.ink, th.page_bg, S.trackFill ? 0.88 : 0.93) : null, ticks: S.track && S.trackLines !== false ? 8 : 0 } },
      [m0.key]: { ...valueCfg, width: pct(valW), hidden: !showVals || undefined },
      ...extraCfg,
    },
    extremes: S.markBest ? [{ on: true, by: m0.key, show: 'icon', rule: S.extremesRule === 'rank' ? 'rank' : null,
      best: { n: 1, icon: 'trend-up', color: S.colorUp, iconFill: mixHex(S.colorUp, th.page_bg, 0.8) },
      worst: { n: 1, icon: 'trend-down', color: S.colorDown, iconFill: mixHex(S.colorDown, th.page_bg, 0.8) } }] : [],
  };
  // the highlighted row: a rounded fill, its name in bold, its rank in a dot
  if (hl) table.cols[dim.key].rules = [{ op: '=', value: hl, rowPill: true, bold: true, fill: mixHex(S.colorUp, th.page_bg, 0.86), stroke: mixHex(S.colorUp, th.page_bg, 0.6) }];

  // the data with the bar (a copy of the first value) and the rank
  const columns = [...cols, { ...m0, key: LIST_BAR, base: undefined, label: ' ' }, { key: LIST_RANK, label: '#', role: 'measure', format: 'number' }];
  const data2 = { ...data, columns, rows: rows.map((r, i) => [...r, r[j0], i + 1]), total_row: null };
  return {
    table, data: data2,
    t: { row_h: rowH, visible: rows.length, offset: 0, head_h: 0, group_h: 0, font: nameSize, totals: false },
    cell: { family: font.family, italic: false, size: nameSize, weight: 500, color: th.ink },
  };
}

/**
 * List → Table: the same columns, now real ones (the bar and the rank become
 * measures of the binding), so the table looks exactly like the list did.
 */
export function listToTable(st, w, S, vm, look) {
  const L = vm?.data && listTable(st, S, vm.data, vm.body, { size: vm.fonts?.chart?.size || 7, family: vm.fonts?.chart?.family || 'Inter' });
  if (!L) return null;
  const ms = w.binding.measures || [];
  const m0 = ms[0];
  const extra = [{ ...m0, id: LIST_BAR, label: ' ' }];
  if (S.rankNumbers) extra.push({ ...m0, id: LIST_RANK, label: '#', calc: 'rank', format: 'number' });
  return {
    kind: 'table', variant: '',
    binding: { ...w.binding, measures: [...ms.filter((m) => m.id !== LIST_BAR && m.id !== LIST_RANK), ...extra] },
    // the list's design view and settings ride along, so Table → Bars brings them back
    opts: { orientation: null, viz: null, totals: false, table: { ...L.table, row_h: Math.floor(L.t.row_h * 10) / 10, fromList: { variant: w.variant || '', opts: { orientation: w.opts?.orientation ?? null, viz: w.opts?.viz ?? null } } } },
    style: { card: look?.card ?? false, fonts: { table_cell: { size: L.cell.size } } },
  };
}

/**
 * Table → List, for a table that is a ranked bar list: one "Group by", a
 * bar column and a value. Returns the bar chart's patch, or null.
 */
export function tableToList(w, data) {
  const ts = w.opts?.table || {};
  const cols = data?.columns || [];
  if (cols.filter((c) => c.role === 'dim').length !== 1) return null;
  const barKey = Object.keys(ts.cols || {}).find((k) => ts.cols[k]?.bar?.on && !ts.cols[k]?.hidden);
  if (!barKey) return null;
  const ms = (w.binding.measures || []).filter((m) => m.id !== barKey && m.id !== LIST_RANK && m.calc !== 'rank');
  if (!ms.length) return null;
  // only a list's columns: the name, the bar, one value (others may be its line below)
  const shown = ms.filter((m) => !ts.cols?.[m.id]?.hidden);
  if (shown.length > 1) return null;
  if (ts.fromList) {
    // back to the list it was: its design view and settings
    return { kind: 'bar', variant: ts.fromList.variant, binding: { ...w.binding, measures: ms }, opts: { ...ts.fromList.opts, table: null, totals: null }, style: { fonts: { table_cell: null } } };
  }
  const vs = ts.cols[ms[0].id] || {};
  const bar = ts.cols[barKey].bar;
  return {
    kind: 'bar', variant: '',
    binding: { ...w.binding, measures: ms },
    opts: { orientation: 'horizontal', table: null, viz: {
      rankStyle: 'ranked', rankNumbers: (w.binding.measures || []).some((m) => m.calc === 'rank'), rankHighlight: false,
      markBest: [].concat(ts.extremes || []).some((x) => x?.on), rowLines: ts.lines === 'rows', subLine: !!vs.sub,
      track: bar.track !== false, barBase: bar.base === 'zero' ? 'zero' : 'size', trackFill: true, trackLines: !!bar.ticks,
      signed: !!vs.signed, decimals: vs.decimals ?? null, compact: vs.compact ? 'on' : 'off' } },
  };
}
