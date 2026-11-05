// Every new setting must change the drawing: sets each on a real widget of the UHNWI template and
// compares the widget's SVG before and after. node dev/shots/probe-controls.mjs <out>
import { send, ev, sleep, chrome } from './cdp.mjs';
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); m.setState(await m.call('new_doc', { kind: 'uhnwi_report' })); })()`); await sleep(5000);
const probes = [
  ['waterfall', { style: { fonts: { axis: { size: 11 } } } }, 'axis labels size'],
  ['waterfall', { style: { fonts: { axis: { color: '#ff0000' } } } }, 'axis labels colour'],
  ['waterfall', { style: { fonts: { data_label: { weight: 400, size: 10 } } } }, 'data labels'],
  ['waterfall', { opts: { viz: { legend: 'top-left' } }, style: { fonts: { legend: { family: 'Georgia', size: 10 } } } }, 'legend font (legend shown)'],
  ['waterfall', { style: { fonts: { axis: { upper: true } } } }, 'axis capitals'],
  ['waterfall', { opts: { viz: { grid: true, gridColor: '#ff0000' } } }, 'grid colour'],
  ['waterfall', { opts: { viz: { grid: true, gridWidth: 2 } } }, 'grid width'],
  ['waterfall', { style: { titleGap: 20 } }, 'space under title'],
  ['waterfall', { opts: { viz: { bridge: true, colorStart: '#ff0000' } } }, 'start bar colour'],
  ['waterfall', { opts: { viz: { bridge: true, colorStart: '$accent' } } }, 'start bar colour by name'],
  ['pie', { style: { fonts: { legend: { size: 11 } } } }, 'donut legend size'],
  ['tiles', { opts: { viz: { tileGap: 14 } } }, 'tile gap'],
  ['tiles', { opts: { viz: { tilePad: 14 } } }, 'tile padding'],
  ['kpi', { opts: { viz: { inline: true } } }, 'KPI on the title line'],
  ['kpi', { opts: { viz: { kpiAlign: 'right' } } }, 'KPI right'],
  ['kpi:multi', { opts: { viz: { aside: 1, shareAside: false } } }, 'KPI second value beside'],
  ['kpi:multi', { opts: { viz: { chip: 1, note: 'x {1}' } } }, 'KPI value in a chip'],
  ['kpi:note', { opts: { viz: { noteDecimals: 3 } } }, 'KPI note decimals'],
  ['kpi:note', { opts: { viz: { note: '**a** {1}', noteBoldColor: '#ff0000' } } }, 'KPI bold words colour'],
  ['bar:combo', { opts: { viz: { seriesOpts: { m1: { axis: 'left' } } } } }, 'series: back on the left axis (the template puts it on the right)'],
  ['bar:combo', { opts: { viz: { seriesOpts: { m1: { dash: 'dotted' } } } } }, 'series: line style'],
  ['bar:combo', { opts: { viz: { seriesOpts: { m1: { width: 4 } } } } }, 'series: line width'],
  ['bar:combo', { opts: { viz: { seriesOpts: { m1: { markers: 'none' } } } } }, 'series: markers'],
  ['bar:combo', { opts: { viz: { seriesOpts: { m0: { labels: 'none' } } } } }, 'series: labels off'],
  ['bar:combo', { opts: { viz: { legend: 'top-left', seriesOpts: { m1: { name: 'Итог' } } } } }, 'series: legend name'],
  ['bar:combo', { opts: { viz: { seriesOpts: { m1: { axis: 'right' } }, y2Max: 0.3 } } }, 'right axis maximum'],
  ['bar:combo', { opts: { viz: { axis: true, axisColor: '#ff0000' } } }, 'left axis colour'],
  ['bar:combo', { opts: { viz: { axis: true, yStep: 0.02 } } }, 'left axis step'],
  ['bar:combo', { opts: { viz: { axis: true, seriesOpts: { m1: { axis: 'right' } }, y2Axis: false } } }, 'right axis hidden'],
  ['bar:combo', { opts: { viz: { axis: true, seriesOpts: { m1: { axis: 'right' } }, y2Color: '#ff0000' } } }, 'right axis colour'],
  ['bar:combo', { opts: { viz: { axis: true, seriesOpts: { m1: { axis: 'right' } }, y2Step: 0.02 } } }, 'right axis step'],
  ['bar:combo', { opts: { viz: { axis: true, seriesOpts: { m1: { axis: 'right' } }, y2Ticks: 7 } } }, 'right axis ticks'],
  ['bar:combo', { opts: { viz: { legend: 'top-left', legendSwatch: 'circle' } } }, 'legend: dot swatches'],
  ['bar:combo', { opts: { viz: { legend: 'top-left', legendSize: 13 } } }, 'legend: swatch size'],
  ['bar:combo', { opts: { viz: { legend: 'top-left', legendOrder: 'reverse' } } }, 'legend: reversed order'],
  ['bar:combo', { opts: { viz: { xRotate: '45' } } }, 'category labels turned 45°'],
  ['bar:combo', { opts: { viz: { xEvery: 3 } } }, 'every 3rd category label'],
  ['bar:combo', { opts: { viz: { xDate: 'nY' } } }, 'dates as 01.2026'],
  ['bar:combo', { opts: { viz: { refLine: 'average' } } }, 'reference line at the average'],
  ['bar:combo', { opts: { viz: { refLine: 'value', refValue: 0.01, refLabel: 'Target {value}' } } }, 'reference line at a value, labelled'],
  ['bar:combo', { opts: { viz: { refLine: 'average', refStyle: 'solid', refColor: '#ff0000' } } }, 'reference line style and colour'],
  ['bar:combo', { opts: { viz: { bandFrom: 0, bandTo: 0.01 } } }, 'band'],
  ['bar:combo', { opts: { viz: { bandFrom: 0, bandTo: 0.01, bandLabel: 'Target range', bandColor: '#ff0000' } } }, 'band label and colour'],
  ['bar:ranked', { opts: { viz: { listValues: false } } }, 'list: hide the values'],
  ['bar:ranked', { opts: { viz: { trackFill: true } } }, 'list: bars fill the row'],
  ['bar:ranked', { opts: { viz: { barThick: 14 } } }, 'list: bar thickness'],
  ['bar:ranked', { opts: { viz: { nameBold: true } } }, 'list: names in bold'],
  ['bar:share', { opts: { viz: { insideLabels: true, shareHeight: 30 } } }, '100% bar labels inside'],
  ['treemap', { opts: { viz: { tmLabels: 'top' } } }, 'treemap labels together, top left'],
  ['table', { opts: { table: { cellPad: 12 } } }, 'table cell padding'],
];
for (const [kind, patch, label] of probes) {
  const r = await ev(`(async () => {
    const s = await import('/ui/state.js'); const r = await import('/ui/render.js');
    const norm = (x) => String(x).replace(/ic\\d+/g, 'ic');
    const all = s.S.st.doc.pages.flatMap((p, pi) => p.widgets.map((w) => ({ w, pi })));
    const [k0, cond] = ${JSON.stringify(kind)}.split(':');
    const ok = (w) => !cond ? true : cond === 'multi' ? (w.binding?.measures || []).length > 1 : cond === 'note' ? (w.binding?.measures || []).length > 1 && /\\{\\d+\\}/.test(s.findWidget(w.id)?.vm?.eff?.opts?.viz?.note || '') && typeof s.findWidget(w.id)?.vm?.data?.kpi?.values?.[1] === 'number' : cond === 'combo' ? (w.binding?.measures || []).some((m) => m.chart === 'line') && (w.binding?.measures || [])[1]?.id === 'm1' : cond === 'ranked' ? s.findWidget(w.id)?.vm?.eff?.opts?.viz?.rankStyle === 'ranked' && !s.findWidget(w.id)?.vm?.eff?.opts?.viz?.trackFill : cond === 'share' ? (s.findWidget(w.id)?.vm?.eff?.opts?.viz?.orientation || s.findWidget(w.id)?.vm?.eff?.opts?.orientation) === 'share' : true;
    const hit = all.filter(({ w }) => w.kind === k0 && w.binding && ok(w)).pop();
    if (!hit) return 'no widget';
    const before = norm(r.pageSvg(s.S.st, hit.pi, { exportMode: true }));
    const prev = JSON.parse(JSON.stringify({ style: hit.w.style || {}, opts: hit.w.opts || {} }));
    await s.edit({ op: 'update_widget', id: hit.w.id, patch: ${JSON.stringify(patch)} });
    const after = norm(r.pageSvg(s.S.st, hit.pi, { exportMode: true }));
    await s.api.undo();
    return before === after ? 'NO CHANGE' : 'changes';
  })()`);
  console.log(`${r === 'changes' ? '✓' : '✗'} ${label}: ${r}`);
}
chrome.kill(); process.exit(0);
