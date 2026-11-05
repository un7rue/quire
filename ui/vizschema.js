// The Visualization settings: one schema describes every option of every
// visual (what it is called, which control edits it, when it applies). The
// same schema builds the widget's "Visualization" section (opts.viz) and the
// template-wide "Chart defaults" (theme.viz[kind]).

import { resolve, PRESETS } from './charts.js';
import { esc, h, resettable } from './util.js';
import { icon } from './icons.js';

export const LABELS = [['auto', 'Auto'], ['all', 'All'], ['last', 'Last'], ['extremes', 'Min & max'], ['none', 'None']];
export const UNITS = [['', 'Automatic'], ['thousands', 'Thousands'], ['millions', 'Millions'], ['billions', 'Billions']];
const FORMATS = [['auto', 'From the data'], ['number', 'Number'], ['currency', 'Currency'], ['percent', 'Percent']];
const DECIMALS = [['', 'Auto'], ['0', '0'], ['1', '1'], ['2', '2'], ['3', '3']];

const isCols = (c) => !c.horizontal;
const hasLines = (c) => c.lines;
const hasBars = (c) => c.bars;


// width and colour of the outline, once it is on
const OUTLINE = [
  { key: 'outlineWidth', label: 'Outline width (pt)', type: 'num', min: 0.25, max: 6, step: 0.25, ph: '0.5', when: (c, S) => S.outline },
  { key: 'outlineColor', label: 'Outline colour', type: 'color', help: 'None: each bar’s own colour, darker.', when: (c, S) => S.outline },
];

// donut slices and split-bar parts: the same controls, their own settings
const PART_MARKS = [
  { key: 'partTint', label: 'Lighten fills', type: 'range', min: 0, max: 0.9, step: 0.05, fmt: (v) => `${Math.round(v * 100)}%` },
  { key: 'partOutline', label: 'Outline', type: 'toggle' },
  ...OUTLINE.map((i) => ({ ...i, when: (c, S) => S.partOutline })),
];

// a waterfall's level bars: where a bridge starts, and the total
const WF_COLOURS = [{ key: 'colorStart', label: 'Start', type: 'color', when: (c, S) => S.bridge }, { key: 'colorTotal', label: 'Total', type: 'color', when: (c, S) => S.total }];

const COLOUR = (extra = []) => ({
  title: 'Colours & marks',
  items: [
    { key: 'colorMode', label: 'Colour by', type: 'select', options: [['auto', 'Series'], ['signOne', 'Up / down, 1 series'], ['sign', 'Up / down']], when: hasBars },
    { key: 'colorUp', label: 'Up / positive', type: 'color' },
    { key: 'colorDown', label: 'Down / negative', type: 'color' },
    ...extra,
    { key: 'tint', label: 'Lighten fills', type: 'range', min: 0, max: 0.9, step: 0.05, fmt: (v) => `${Math.round(v * 100)}%` },
    { key: 'outline', label: 'Outline', type: 'toggle' },
    ...OUTLINE,
    { key: 'radius', label: 'Corner radius', type: 'num', min: 0, max: 20, step: 0.5 },
    { key: 'roundAll', label: 'Round all corners', type: 'toggle' },
    { key: 'barFill', label: 'Bar fill', type: 'seg', options: [['flat', 'Solid'], ['gradient', 'Fade to axis']], when: hasBars },
    { key: 'barWidth', label: 'Bar width', type: 'range', min: 0.2, max: 1, step: 0.05, fmt: (v) => `${Math.round(v * 100)}%`, when: hasBars },
    { key: 'maxBar', label: 'Max bar (pt)', type: 'num', min: 2, max: 120, when: hasBars },
  ],
});

const VALUES = (extra = []) => ({
  title: 'Values',
  items: [
    // units (thousands, millions) replace the format, except for percentages
    { key: 'format', label: 'Format', type: 'select', options: FORMATS, when: (c, S) => !S.units || S.format === 'percent' },
    { key: 'units', label: 'Units', type: 'select', options: UNITS, when: (c, S) => !c.percent && S.format !== 'percent' },
    { key: 'decimals', label: 'Decimals', type: 'select', options: DECIMALS, num: true },
    { key: 'signed', label: 'Show + sign', type: 'toggle' },
    { key: 'suffix', label: 'Suffix', type: 'text', ph: 'e.g.  ₽' },
    ...extra,
  ],
});

// data labels: colour and their own number format (as the values unless set)
const LABEL_COLOR = { key: 'labelColor', label: 'Colour', type: 'select', options: [['auto', 'Auto'], ['series', 'Series colour'], ['sign', 'Up / down'], ['ink', 'Text'], ['muted', 'Muted']], when: (c, S) => S.labels !== 'none' };
const LABEL_FMT = [
  { key: 'labelUnits', label: 'Label units', type: 'select', options: [['', 'As values'], ['none', 'Full number'], ...UNITS.slice(1)], when: (c, S) => S.labels !== 'none' && !c.percent && S.format !== 'percent' },
  { key: 'labelDecimals', label: 'Label decimals', type: 'select', options: [['', 'As values'], ...DECIMALS.slice(1)], num: true, when: (c, S) => S.labels !== 'none' },
  { key: 'labelSuffix', label: 'Label suffix', type: 'text', ph: 'as values', when: (c, S) => S.labels !== 'none' },
];
const LABELS_SHOW = (extra = []) => ({ title: 'Data labels', items: [{ key: 'labels', label: 'Values', type: 'seg', showHide: true, options: [['all', 'Show'], ['none', 'Hide']] }, ...extra, LABEL_COLOR, ...LABEL_FMT] });

const LABELS_G = {
  title: 'Data labels',
  items: [
    { key: 'labels', label: 'Show', type: 'select', options: LABELS },
    { key: 'labelPos', label: 'Position', type: 'select', options: [['auto', 'Auto'], ['outside', 'Outside'], ['inside', 'Inside'], ['below', 'Next to axis']], when: (c, S) => c.bars && S.labels !== 'none' },
    LABEL_COLOR,
    { key: 'labelBold', label: 'Bold', type: 'toggle', when: (c, S) => c.bars && S.labels !== 'none' },
    { key: 'labelBox', label: 'Frame', type: 'seg', options: [['none', 'Plain'], ['box', 'Box'], ['pill', 'Pill']], when: (c, S) => S.labels !== 'none' },
    ...LABEL_FMT,
  ],
};

// the value axes (a left and a right one once a series is on the right), the category axis and the grid
const VSUB = (c) => (c.right ? 'Left axis' : 'Value axis');
const AXIS = {
  title: 'Axis & grid',
  items: [
    { key: 'axis', label: 'Show', type: 'toggle', sub: VSUB },
    { key: 'axisColor', label: 'Label colour', type: 'color', sub: VSUB, when: (c, S) => S.axis },
    { key: 'ticks', label: 'Ticks', type: 'num', min: 2, max: 8, sub: VSUB, when: (c, S) => !S.yStep },
    { key: 'yStep', label: 'Step', type: 'num', ph: 'auto', nullable: true, sub: VSUB, help: 'A tick every so much (e.g. 0.05 for every 5%); the number of ticks then follows.' },
    // a sign on axis ticks is for percentages (changes), not amounts
    { key: 'signedTicks', label: 'Signed ticks (+5%)', type: 'toggle', sub: VSUB, when: (c) => c.percent },
    { key: 'yMin', label: 'Minimum', type: 'num', ph: 'auto', nullable: true, sub: VSUB },
    { key: 'yMax', label: 'Maximum', type: 'num', ph: 'auto', nullable: true, sub: VSUB },
    // the right axis, once a series is put on it (Series)
    { key: 'y2Axis', label: 'Show', type: 'toggle', sub: 'Right axis', when: (c) => c.right },
    { key: 'y2Color', label: 'Label colour', type: 'color', sub: 'Right axis', help: 'None: as the left axis.', when: (c, S) => c.right && S.y2Axis !== false },
    { key: 'y2Ticks', label: 'Ticks', type: 'num', min: 2, max: 8, ph: 'as the left', sub: 'Right axis', when: (c, S) => c.right && !S.y2Step },
    { key: 'y2Step', label: 'Step', type: 'num', ph: 'auto', nullable: true, sub: 'Right axis', when: (c) => c.right },
    { key: 'y2Signed', label: 'Signed ticks (+5%)', type: 'toggle', sub: 'Right axis', when: (c) => c.right && c.percentRight },
    { key: 'y2Min', label: 'Minimum', type: 'num', ph: 'auto', nullable: true, sub: 'Right axis', when: (c) => c.right },
    { key: 'y2Max', label: 'Maximum', type: 'num', ph: 'auto', nullable: true, sub: 'Right axis', when: (c) => c.right },
    { key: 'y2Units', label: 'Units', type: 'select', options: [['', 'As the left'], ['none', 'Full number'], ...UNITS.slice(1)], sub: 'Right axis', when: (c) => c.right && !c.percentRight },
    { key: 'xLabels', label: 'Labels', type: 'toggle', sub: 'Category axis' },
    { key: 'xEvery', label: 'Every Nth label', type: 'num', min: 0, max: 24, ph: 'as fit', sub: 'Category axis', help: 'Show one label in so many (2: every other one). Empty: every label that fits.', when: (c, S) => S.xLabels && c.kind !== 'waterfall' },
    { key: 'xRotate', label: 'Turn labels', type: 'seg', options: [['0', 'Level'], ['45', '45°'], ['90', '90°']], sub: 'Category axis', when: (c, S) => S.xLabels && c.kind !== 'waterfall' },
    { key: 'xDate', label: 'Dates as', type: 'select', options: [['auto', 'As the data'], ['m', 'Jan'], ['my', 'Jan 26'], ['mY', 'Jan 2026'], ['n', '01.26'], ['nY', '01.2026'], ['y', '2026']], sub: 'Category axis', when: (c, S) => c.dateCats && S.xLabels && c.kind !== 'waterfall' },
    { key: 'grid', label: 'Grid lines', type: 'toggle', sub: 'Grid' },
    { key: 'gridDir', label: 'Direction', type: 'select', options: [['h', 'Horizontal'], ['v', 'Vertical'], ['both', 'Both']], sub: 'Grid', when: (c, S) => S.grid },
    { key: 'gridStyle', label: 'Style', type: 'select', options: [['solid', 'Solid'], ['dashed', 'Dashed'], ['dotted', 'Dotted']], sub: 'Grid', when: (c, S) => S.grid },
    { key: 'gridColor', label: 'Colour', type: 'color', sub: 'Grid', when: (c, S) => S.grid },
    { key: 'gridWidth', label: 'Width (pt)', type: 'num', min: 0.25, max: 4, step: 0.25, ph: 'auto', sub: 'Grid', when: (c, S) => S.grid },
    { key: 'zeroLine', label: 'Zero line', type: 'toggle', sub: 'Grid' },
    { key: 'zeroColor', label: 'Zero line colour', type: 'color', sub: 'Grid', when: (c, S) => S.zeroLine },
  ],
};

const LEGEND = (signed = true) => ({
  title: 'Legend',
  items: [
    { key: 'legend', label: 'Position', type: 'select', options: [['auto', 'Auto'], ['top-right', 'In the title row'], ['top-left', 'Top'], ['bottom', 'Bottom'], ['none', 'Hidden']] },
    { key: 'legendSwatch', label: 'Swatch', type: 'seg', options: [['rounded', 'Rounded'], ['square', 'Square'], ['circle', 'Dot'], ['line', 'Line']], when: (c, S) => S.legend !== 'none' },
    { key: 'legendSize', label: 'Swatch size (pt)', type: 'num', min: 3, max: 16, step: 0.5, ph: '8', when: (c, S) => S.legend !== 'none' },
    { key: 'legendOrder', label: 'Order', type: 'select', options: [['data', 'As the data'], ['reverse', 'Reversed'], ['name', 'By name']], when: (c, S) => S.legend !== 'none' },
    ...(signed ? [
      { key: 'legendUp', label: '“Up” text', type: 'text', ph: 'up' },
      { key: 'legendDown', label: '“Down” text', type: 'text', ph: 'down' },
    ] : []),
  ],
});

const HIGHLIGHT = (callouts) => ({
  title: 'Highlights',
  items: [
    { key: 'extremes', label: 'Mark best & worst', type: 'toggle', when: hasBars },
    { key: 'extremesRule', label: 'Best / worst', type: 'select', options: [['sign', 'Gains & losses only'], ['rank', 'Always highest & lowest']], hint: 'Gains & losses only: best only if above 0, worst only if below 0; a zero is neither.', when: (c, S) => c.bars && S.extremes },
    { key: 'bands', label: 'Background bands', type: 'toggle', when: (c, S) => c.bars && S.extremes },
    { key: 'bestLabel', label: 'Best label', type: 'text', ph: 'BEST', when: (c, S) => c.bars && S.extremes },
    { key: 'worstLabel', label: 'Worst label', type: 'text', ph: 'WORST', when: (c, S) => c.bars && S.extremes },
    { key: 'callout', label: 'Callout', type: 'select', options: callouts },
    { key: 'calloutBg', label: 'Callout fill', type: 'color', when: (c, S) => S.callout !== 'none' },
    { key: 'calloutColor', label: 'Callout text', type: 'color', when: (c, S) => S.callout !== 'none' },
    // a reference line: a target, or the first series' average, median, highest or lowest
    { key: 'refLine', label: 'Line at', type: 'select', options: [['none', 'None'], ['value', 'A value'], ['average', 'Average'], ['median', 'Median'], ['max', 'Highest'], ['min', 'Lowest']], sub: 'Reference line', help: 'Average, median, highest and lowest are the first series’ (on the left axis).' },
    { key: 'refValue', label: 'Value', type: 'num', ph: 'e.g. 1000000', nullable: true, sub: 'Reference line', when: (c, S) => S.refLine === 'value' },
    { key: 'refLabel', label: 'Label', type: 'text', ph: 'e.g. Target {value}', sub: 'Reference line', help: '{value} is the line’s value. Empty: its name and value.', when: (c, S) => S.refLine && S.refLine !== 'none' },
    { key: 'refColor', label: 'Colour', type: 'color', sub: 'Reference line', when: (c, S) => S.refLine && S.refLine !== 'none' },
    { key: 'refStyle', label: 'Style', type: 'seg', options: [['solid', 'Solid'], ['dashed', 'Dashed'], ['dotted', 'Dotted']], sub: 'Reference line', when: (c, S) => S.refLine && S.refLine !== 'none' },
    // a band: a range shown behind the marks (a target range, a corridor)
    { key: 'bandFrom', label: 'From', type: 'num', ph: 'none', nullable: true, sub: 'Band', help: 'A range shaded behind the marks, e.g. the target range. Set both ends.' },
    { key: 'bandTo', label: 'To', type: 'num', ph: 'none', nullable: true, sub: 'Band' },
    { key: 'bandLabel', label: 'Label', type: 'text', ph: 'e.g. Target range', sub: 'Band', when: (c, S) => S.bandFrom != null && S.bandTo != null },
    { key: 'bandColor', label: 'Colour', type: 'color', sub: 'Band', when: (c, S) => S.bandFrom != null && S.bandTo != null },
  ],
});

const LINES = {
  title: 'Lines',
  when: hasLines,
  items: [
    { key: 'lineColor', label: 'Colour', type: 'color' },
    { key: 'lineWidth', label: 'Width', type: 'num', min: 0.5, max: 6, step: 0.25 },
    { key: 'lineStyle', label: 'Style', type: 'seg', options: [['solid', 'Solid'], ['dashed', 'Dashed'], ['dotted', 'Dotted']] },
    { key: 'curve', label: 'Shape', type: 'seg', options: [['straight', 'Straight'], ['smooth', 'Smooth']] },
    { key: 'markers', label: 'Markers', type: 'seg', options: [['none', 'None'], ['last', 'Last'], ['all', 'All']] },
    { key: 'markerSize', label: 'Marker size', type: 'num', min: 1, max: 8, step: 0.2, when: (c, S) => S.markers !== 'none' },
    { key: 'markerFill', label: 'Marker fill', type: 'seg', options: [['solid', 'Solid'], ['hollow', 'Hollow']], when: (c, S) => S.markers !== 'none' },
    { key: 'areaFill', label: 'Area under', type: 'seg', options: [['none', 'None'], ['flat', 'Flat'], ['gradient', 'Fading']], when: (c) => c.kind === 'line' || c.kind === 'area' },
    { key: 'areaOpacity', label: 'Area opacity', type: 'range', min: 0, max: 0.8, step: 0.02, fmt: (v) => `${Math.round(v * 100)}%`, when: (c, S) => c.kind === 'area' || S.areaFill !== 'none' },
    { key: 'guides', label: 'Guides to axis', type: 'toggle', when: (c) => !c.bars },
  ],
};

export const SCHEMA = {
  // a range bar: points on one scale (benchmarks, a peer group)
  bar: (c) => (c.scale ? SCHEMA.scale() : c.share ? [
    { title: 'Bar', items: [
      { key: 'shareHeight', label: 'Height (pt)', type: 'num', min: 0, max: 60, ph: 'auto' },
      { key: 'shareGap', label: 'Gap between parts', type: 'num', min: 0, max: 6, step: 0.5 },
      { key: 'radius', label: 'End radius', type: 'num', min: 0, max: 30, step: 0.5 },
      ...PART_MARKS,
      { key: 'insideLabels', label: 'Labels inside parts', type: 'toggle', sec: 'Data labels' },
      { key: 'insideValue', label: 'They show', type: 'select', options: [['first', 'Amount'], ['share', 'Share'], ['second', 'Second value']], sec: 'Data labels', when: (c, S) => S.insideLabels },
    ] },
    { title: 'Legend', items: [
      { key: 'legend', label: 'Show', type: 'seg', showHide: true, options: [['all', 'Show'], ['none', 'Hide']] },
      { key: 'legendText', label: 'Text after names', type: 'text', ph: '{value} · {share}' },
      { key: 'legendShare', label: 'Shares', type: 'toggle', when: (c, S) => !S.legendText },
      { key: 'legendAmount', label: 'Amounts', type: 'toggle', when: (c, S) => !S.legendText },
      { key: 'legendCols', label: 'Columns', type: 'num', min: 0, max: 8, ph: 'flow' },
      { key: 'decimals', label: 'Share decimals', type: 'select', options: DECIMALS, num: true },
      { key: 'labelDecimals', label: 'Amount decimals', type: 'select', options: DECIMALS, num: true },
    ] },
  ] : c.horizontal ? [
    { title: 'Style', items: [{ key: 'rankStyle', label: 'Layout', type: 'seg', options: [['bars', 'Chart'], ['ranked', 'List']] }, { key: 'stacked', label: 'Stacked', type: 'toggle', when: (cc, S) => cc.barSeries > 1 && S.rankStyle !== 'ranked' }] },
    { title: 'List', when: (cc, S) => S.rankStyle === 'ranked', items: [
      { type: 'hint', tone: 'info', text: 'A row per category: name, bar and value. Need more columns? Chart type → Table keeps this look and lets you add them.' },
      { key: 'rankHighlight', label: 'Highlight a row', type: 'toggle' },
      { key: 'highlight', label: 'Row to highlight', type: 'text', ph: 'First row', when: (cc, S) => S.rankHighlight !== false },
      { key: 'rankNumbers', label: 'Rank numbers', type: 'toggle' },
      { key: 'markBest', label: 'Best & worst marks', type: 'toggle' },
      { key: 'extremesRule', label: 'Best / worst', type: 'select', options: [['sign', 'Gains & losses only'], ['rank', 'Always highest & lowest']], hint: 'Gains & losses only: best only if above 0, worst only if below 0; a zero is neither.', when: (cc, S) => S.markBest },
      { key: 'rowLines', label: 'Lines between rows', type: 'toggle' },
      { key: 'track', label: 'Scale track', type: 'toggle' },
      { key: 'listValues', label: 'Values at the right', type: 'toggle' },
      { key: 'trackFill', label: 'Bars fill the row', type: 'toggle', help: 'On: the bars take all the width the names and values leave. Off: bars up to 160 pt, the names get the rest.' },
      { key: 'barThick', label: 'Bar thickness (pt)', type: 'num', min: 0, max: 30, step: 0.5, ph: 'auto' },
      { key: 'nameBold', label: 'Names in bold', type: 'toggle' },
      { key: 'trackLines', label: 'Track divisions', type: 'toggle', when: (cc, S) => S.track },
      { key: 'barBase', label: 'Bar length', type: 'seg', options: [['zero', 'From zero'], ['size', 'By size']] },
      { key: 'subLine', label: 'Other values below', type: 'toggle', when: (cc) => cc.multi },
    ] },
    // a ranked list draws every bar by sign, with its own row style
    { ...COLOUR(), items: COLOUR().items.filter((i) => i.key !== 'colorMode').concat([{ key: 'colorMode', label: 'Colour by', type: 'select', options: [['auto', 'Series'], ['sign', 'Up / down']] }])
      .map((i) => (['colorMode', 'radius', 'roundAll', 'barWidth', 'maxBar'].includes(i.key) ? { ...i, when: (cc, S) => S.rankStyle !== 'ranked' && (!i.when || i.when(cc, S)) } : i)) },
    { ...LABELS_SHOW([{ key: 'highlight', label: 'Highlight row', type: 'text', ph: 'none', when: (cc, S) => S.rankStyle !== 'ranked' }]), when: (cc, S) => S.rankStyle !== 'ranked' },
    VALUES(),
    { title: 'Scale', when: (cc, S) => S.rankStyle !== 'ranked', items: [{ key: 'yMin', label: 'Minimum', type: 'num', ph: 'auto', nullable: true }, { key: 'yMax', label: 'Maximum', type: 'num', ph: 'auto', nullable: true }] },
    { ...LEGEND(false), when: (cc, S) => S.rankStyle !== 'ranked' },
  ] : [
    { title: 'Style', items: [{ key: 'stacked', label: 'Stacked', type: 'toggle', when: (cc) => cc.barSeries > 1 }] },
    COLOUR(), LABELS_G, VALUES(), AXIS, LEGEND(), HIGHLIGHT([['none', 'None'], ['last', 'Last line value'], ['max', 'Largest bar']]), LINES,
  ]),
  line: () => [LINES, LABELS_G, VALUES(), AXIS, LEGEND(false), HIGHLIGHT([['none', 'None'], ['last', 'Last value']])],
  area: () => [LINES, LABELS_G, VALUES(), AXIS, LEGEND(false), HIGHLIGHT([['none', 'None'], ['last', 'Last value']])],
  waterfall: () => [
    { title: 'Style', items: [{ key: 'bridge', label: 'Starts from a level', type: 'toggle', help: 'The first value is where it starts (e.g. the capital at the start); the axis begins near it so small changes show.' }, { key: 'total', label: 'Total bar', type: 'toggle' }, { key: 'totalLabel', label: 'Total label', type: 'text', ph: 'Total', when: (c, S) => S.total }, { key: 'connectors', label: 'Connectors', type: 'seg', options: [['dashed', 'Dashed'], ['solid', 'Solid'], ['none', 'None']] }] },
    // a waterfall always colours steps by direction
    { ...COLOUR(WF_COLOURS), items: COLOUR(WF_COLOURS).items.filter((i) => i.key !== 'colorMode') },
    LABELS_SHOW(),
    VALUES(), AXIS,
    { ...LEGEND(), items: [...LEGEND().items, { key: 'legendTotal', label: '“Total” text', type: 'text', ph: 'total' }] },
    { title: 'Highlights', items: [
      { key: 'extremes', label: 'Mark best & worst', type: 'toggle' },
      { key: 'extremesRule', label: 'Best / worst', type: 'select', options: [['sign', 'Gains & losses only'], ['rank', 'Always highest & lowest']], hint: 'Gains & losses only: best only if above 0, worst only if below 0; a zero is neither.', when: (c, S) => S.extremes },
      { key: 'bands', label: 'Background bands', type: 'toggle', when: (c, S) => S.extremes },
      { key: 'bestLabel', label: 'Best label', type: 'text', ph: 'BEST', when: (c, S) => S.extremes },
      { key: 'worstLabel', label: 'Worst label', type: 'text', ph: 'WORST', when: (c, S) => S.extremes },
      { key: 'callout', label: 'Total as a pill', type: 'select', options: [['none', 'No'], ['last', 'Yes']] },
      { key: 'calloutBg', label: 'Pill fill', type: 'color', when: (c, S) => S.callout !== 'none' },
      { key: 'calloutColor', label: 'Pill text', type: 'color', when: (c, S) => S.callout !== 'none' },
    ] },
  ],
  pie: () => [
    { title: 'Style', items: [
      { key: 'inner', label: 'Hole', type: 'range', min: 0, max: 0.9, step: 0.02, fmt: (v) => (v ? `${Math.round(v * 100)}%` : 'Pie') },
      { key: 'sliceGap', label: 'Gap between slices', type: 'num', min: 0, max: 6, step: 0.2 },
      { key: 'maxSlices', label: 'Max slices', type: 'num', min: 2, max: 20 },
    ] },
    { title: 'Colours', items: [
      { key: 'mono', label: 'Shades of one colour', type: 'toggle' },
      { key: 'sortSlices', label: 'Largest first', type: 'toggle' },
      { key: 'baseColor', label: 'Base colour', type: 'color', when: (c, S) => S.mono },
      ...PART_MARKS,
    ] },
    { title: 'Centre', when: (c, S) => S.inner > 0.2, items: [
      { key: 'center', label: 'Shows', type: 'select', options: [['total', 'Total'], ['top', 'Largest share'], ['text', 'Custom text'], ['none', 'Nothing']] },
      { key: 'centerText', label: 'Text', type: 'text', when: (c, S) => S.center === 'text' },
      { key: 'centerLabel', label: 'Caption', type: 'text', ph: 'Total', when: (c, S) => S.center !== 'none' },
      { key: 'centerUpper', label: 'Caption in capitals', type: 'toggle', when: (c, S) => S.center !== 'none' },
      { key: 'centerSize', label: 'Size (pt)', type: 'num', min: 0, max: 40, ph: 'auto', when: (c, S) => S.center !== 'none' },
      { key: 'signed', label: 'Show + sign', type: 'toggle' },
      { key: 'suffix', label: 'Suffix', type: 'text', ph: 'e.g. п.п.' },
    ] },
    { title: 'Legend', items: [
      { key: 'legendStyle', label: 'Style', type: 'seg', options: [['simple', 'Simple'], ['table', 'Table'], ['none', 'Hidden']] },
      { key: 'legendAmount', label: 'Amounts', type: 'toggle', when: (c, S) => S.legendStyle !== 'none' },
      ...[LABEL_COLOR, ...LABEL_FMT].map((i) => ({ ...i, label: i.label.replace('Label', 'Amount').replace('Colour', 'Amount colour'), when: (c, S) => S.legendStyle !== 'none' && S.legendAmount })),
      { key: 'legendShare', label: 'Shares', type: 'toggle', when: (c, S) => S.legendStyle !== 'none' },
      { key: 'decimals', label: 'Decimals', type: 'select', options: DECIMALS, num: true },
    ] },
  ],
  treemap: () => [
    { title: 'Style', items: [
      { key: 'tmLayout', label: 'Layout', type: 'select', options: [['squarify', 'Tiles'], ['split', 'Largest left, rest tiled'], ['auto', 'Automatic']] },
      { key: 'chipBelow', label: 'Group below (%)', type: 'num', min: 0, max: 30, step: 0.5, help: 'Categories smaller than this share become one “Other” tile.' },
      { key: 'tmGap', label: 'Gap (pt)', type: 'num', min: 0, max: 10, step: 0.5, ph: '2' },
      { key: 'tmOutline', label: 'Outline', type: 'toggle' },
      ...OUTLINE.map((i) => ({ ...i, when: (c, S) => S.tmOutline })),
    ] },
    { title: 'Colours', items: [
      { key: 'colorMode', label: 'Colour by', type: 'select', options: [['auto', 'Category'], ['sign', 'Result (up / down)'], ['value', 'Value (one colour)']], help: 'Result uses the second value of the data, e.g. the return: the larger it is, the stronger the colour. Value: darker for larger tiles.' },
      { key: 'baseColor', label: 'Colour', type: 'color', when: (c, S) => S.colorMode === 'value' },
      { key: 'colorUp', label: 'Up', type: 'color', when: (c, S) => S.colorMode === 'sign' && c.multi },
      { key: 'colorDown', label: 'Down', type: 'color', when: (c, S) => S.colorMode === 'sign' && c.multi },
      { key: 'tmFill', label: 'Fill strength', type: 'range', min: 0.05, max: 1, step: 0.05, def: () => 0.24, fmt: (v) => `${Math.round(v * 100)}%`, when: (c, S) => !['sign', 'value'].includes(S.colorMode) },
    ] },
    { title: 'Values', items: [
      { key: 'compact', label: 'Compact numbers', type: 'seg', options: [['auto', 'Off'], ['on', 'On']] },
      { key: 'decimals', label: 'Decimals', type: 'select', options: DECIMALS, num: true },
      { key: 'shareText', label: 'Share caption', type: 'text', ph: 'of total' },
      { key: 'tmLabels', label: 'Labels', type: 'seg', options: [['split', 'Name top, value below'], ['top', 'Together, top left']], help: 'Together: the name with its value and share under it in the top left corner, as data journalism often sets them.' },
    ] },
  ],
  tiles: () => [
    { title: 'Layout', items: [
      { key: 'cols', label: 'Columns', type: 'num', min: 0, max: 12, ph: 'auto' },
      { key: 'tileGap', label: 'Gap between tiles (pt)', type: 'num', min: 0, max: 30, ph: '6' },
      { key: 'tilePad', label: 'Padding in a tile (pt)', type: 'num', min: 0, max: 24, ph: '7' },
      { key: 'tileBg', label: 'Tile fill', type: 'color' },
      { key: 'tileBorder', label: 'Border', type: 'toggle' },
      { key: 'tileTone', label: 'Tint by sign', type: 'seg', options: [['none', 'Off'], ['sign', 'On']] },
      { key: 'radius', label: 'Corner radius', type: 'num', min: 0, max: 20, step: 0.5 },
      { key: 'markExtremes', label: 'Mark best & worst', type: 'toggle' },
      { key: 'extremesRule', label: 'Best / worst', type: 'select', options: [['sign', 'Gains & losses only'], ['rank', 'Always highest & lowest']], hint: 'Gains & losses only: best only if above 0, worst only if below 0; a zero is neither.', when: (c, S) => S.markExtremes },
    ] },
    { title: 'Values', items: [
      { key: 'valueColor', label: 'Value colour', type: 'seg', options: [['sign', 'Up / down'], ['ink', 'Text'], ['font', 'From font']] },
      { key: 'colorUp', label: 'Up', type: 'color' },
      { key: 'colorDown', label: 'Down', type: 'color' },
      ...VALUES().items.filter((i) => i.key !== 'units'),
    ] },
  ],
  kpi: () => [
    // a KPI with a "Group by" shows one category, or every category's share
    { title: 'Categories', when: (c) => c.dims > 0, items: [
      { key: 'kpiGroup', label: 'Shows', type: 'select', options: [['name', 'Top category'], ['value', 'Top category’s value'], ['shares', 'Shares of all']] },
      { type: 'hint', tone: 'info', text: 'The top category comes first in the Data tab’s sort (largest value by default). In the note, {name} is its name, {names} the categories shown, {share} its share.' },
      { key: 'shareAside', label: 'Its share beside it', type: 'toggle', when: (c, S) => (S.kpiGroup || 'name') !== 'shares' },
    ] },
    { title: 'Value', items: [
      { key: 'kpiAlign', label: 'Align', type: 'seg', options: [['left', 'Left'], ['center', 'Centre'], ['right', 'Right']] },
      { key: 'inline', label: 'On the title’s line', type: 'toggle', help: 'The value sits at the right of the title instead of under it.' },
      { key: 'valueColor', label: 'Colour', type: 'seg', options: [['font', 'From font'], ['ink', 'Text'], ['sign', 'Up / down']] },
      { key: 'format', label: 'Format', type: 'select', options: FORMATS },
      { key: 'compact', label: 'Compact (1.2M)', type: 'seg', options: [['auto', 'Auto'], ['on', 'On'], ['off', 'Off']] },
      { key: 'decimals', label: 'Decimals', type: 'select', options: DECIMALS, num: true },
      { key: 'signed', label: 'Show + sign', type: 'toggle' },
      { key: 'suffix', label: 'Suffix', type: 'text' },
      { key: 'arrow', label: 'Arrow ↗ ↘', type: 'toggle' },
      { key: 'valueJoin', label: 'All values on one line', type: 'toggle', when: (c) => c.multi || c.dims > 0 },
      { key: 'aside', label: 'Second value beside it', type: 'select', options: [['', 'None'], ['1', 'Value 2'], ['2', 'Value 3'], ['3', 'Value 4']], num: true, when: (c, S) => c.multi && !S.shareAside },
      { key: 'asideDecimals', label: 'Its decimals', type: 'select', options: DECIMALS, num: true, when: (c, S) => S.aside || S.shareAside },
    ] },
    { title: 'Card', items: [
      { key: 'tone', label: 'Colour the card', type: 'select', options: [['none', 'No'], ['sign', 'By result (tinted)'], ['solid', 'By result (solid)'], ['good', 'Good colour'], ['bad', 'Bad colour'], ['accent', 'Accent'], ['ink', 'Dark']] },
    ] },
    { title: 'Note & breakdown', items: [
      { key: 'note', label: 'Note', type: 'text', ph: '{1} profit since January' },
      { key: 'noteColor', label: 'Values in note', type: 'seg', options: [['sign', 'Up / down'], ['ink', 'Text']], when: (c, S) => !!S.note },
      { key: 'noteDecimals', label: 'Their decimals', type: 'select', options: DECIMALS, num: true, when: (c, S) => !!S.note },
      { key: 'noteBoldColor', label: '**Bold** words colour', type: 'color', when: (c, S) => /\*\*/.test(S.note || '') },
      { key: 'chip', label: 'Value in a chip', type: 'select', options: [['', 'None'], ['1', 'Value 2'], ['2', 'Value 3'], ['3', 'Value 4']], num: true, when: (c) => c.multi, help: 'Another value in a coloured chip at the right of the note line.' },
      { key: 'breakdown', label: 'Other values as rows', type: 'toggle' },
      { key: 'breakdownTone', label: 'Rows coloured', type: 'seg', options: [['sign', 'By sign'], ['neutral', 'Neutral']], when: (c, S) => S.breakdown },
    ] },
    { title: 'Change', items: [
      { type: 'hint', text: 'Nothing to compare with yet: choose a comparison period in the toolbar (e.g. previous period) to show the change.', when: (c) => c.noDelta },
      { key: 'delta', label: 'Show change', type: 'toggle' },
      { key: 'deltaFormat', label: 'As', type: 'select', options: [['auto', 'Automatic'], ['pct', 'Percent change'], ['pts', 'Points'], ['abs', 'Amount']], when: (c, S) => S.delta },
      { key: 'deltaText', label: 'Comparison text', type: 'text', ph: 'vs {{compare}}', when: (c, S) => S.delta },
      { key: 'upIsGood', label: 'Up is good', type: 'toggle' },
      { key: 'colorUp', label: 'Good', type: 'color' },
      { key: 'colorDown', label: 'Bad', type: 'color' },
    ] },
    { title: 'Trend', items: [
      { type: 'hint', text: 'No trend to draw: this figure’s data has no dates in the period. A sparkline needs a dataset with a date field and at least three points.', when: (c) => c.noSpark },
      { key: 'spark', label: 'Sparkline', type: 'seg', options: [['line', 'Line'], ['bars', 'Bars'], ['none', 'None']] },
      { key: 'sparkPos', label: 'Position', type: 'seg', options: [['auto', 'Auto'], ['right', 'Right'], ['bottom', 'Below']], when: (c, S) => S.spark !== 'none' },
      { key: 'sparkColor', label: 'Colour', type: 'color', when: (c, S) => S.spark !== 'none' },
    ] },
  ],
};

SCHEMA.scale = () => [
  { title: 'Scale', items: [
    { key: 'scaleMin', label: 'Minimum', type: 'num', ph: 'auto', nullable: true },
    { key: 'scaleMax', label: 'Maximum', type: 'num', ph: 'auto', nullable: true },
    { key: 'scaleTicks', label: 'Tick labels', type: 'text', ph: 'below median|median|top 10%' },
    { key: 'track', label: 'Track', type: 'toggle' },
    { key: 'trackStyle', label: 'Track colour', type: 'seg', options: [['gradient', 'Gradient'], ['split', 'Split at 0'], ['plain', 'Plain']], when: (c, S) => S.track },
    { key: 'shareHeight', label: 'Track height', type: 'num', min: 0, max: 20, ph: '5' },
    { key: 'axisTicks', label: 'Axis ticks', type: 'toggle', when: (c, S) => !S.scaleTicks },
    { key: 'scaleZero', label: 'Zero line', type: 'toggle' },
    { key: 'scaleEnds', label: 'Lowest & highest at the ends', type: 'toggle' },
    { type: 'hint', tone: 'info', text: 'The lowest and highest points close the scale, with their values outside it (e.g. min and max of a peer group).', when: (c, S) => S.scaleEnds },
  ] },
  { title: 'Points', items: [
    { key: 'highlight', label: 'Highlight', type: 'text', ph: 'name of the point' },
    { key: 'hlDecimals', label: 'Its decimals', type: 'num', min: 0, max: 4, ph: 'as others', nullable: true, when: (c, S) => S.valueLabels },
    { key: 'markerStyle', label: 'Markers', type: 'seg', options: [['dots', 'Dots'], ['ring', 'Rings'], ['solid', 'Solid']] },
    { key: 'valueLabels', label: 'Values above', type: 'toggle' },
    { key: 'nameLabels', label: 'Names below', type: 'toggle' },
    { key: 'colorUp', label: 'Up colour', type: 'color' },
    { key: 'colorDown', label: 'Down colour', type: 'color' },
  ] },
  VALUES(),
];

export const VIZ_KINDS = Object.keys(SCHEMA);
export const KIND_NAMES = { scale: 'Dot scales', bar: 'Bar & column charts', line: 'Line charts', area: 'Area charts', waterfall: 'Waterfalls', pie: 'Donuts & pies', treemap: 'Treemaps', tiles: 'Tiles', kpi: 'KPIs' };

/** Facts about the widget that decide which settings apply. */
export function context(w, kind = w.kind) {
  const ms = w.binding?.measures || [];
  const lineMs = ms.filter((m) => m.chart === 'line').length;
  const orient = w.opts?.viz?.orientation || w.opts?.orientation;
  const horizontal = orient === 'horizontal', share = orient === 'share', scale = orient === 'scale';
  const multi = ms.length > 1 || (w.binding?.dims?.length || 0) > 1;
  // bar series: bar measures, or one measure split by a second field
  const barSeries = (w.binding?.dims?.length || 0) > 1 ? 2 : ms.length - lineMs;
  // a series on the right axis (Design → Series)
  const so = w.opts?.viz?.seriesOpts || {};
  const rightKeys = Object.keys(so).filter((k) => so[k]?.axis === 'right');
  const right = (kind === 'bar' || kind === 'line' || kind === 'area') && !horizontal && !share && !scale && rightKeys.length > 0 && (ms.length > 1 || (w.binding?.dims?.length || 0) > 1);
  const percentRight = right && rightKeys.every((k) => ms.find((m) => m.id === k)?.format === 'percent');
  return {
    kind, horizontal, share, scale, multi, barSeries, dims: w.binding?.dims?.length || 0, right, percentRight,
    // categories that are dates (month, quarter…): they can be written another way
    dateCats: /:(day|week|month|quarter|year)$/.test(w.binding?.dims?.[0] || ''),
    // values that are percentages have no thousands or millions
    percent: ms.length > 0 && ms.every((m) => m.format === 'percent'),
    lines: kind === 'line' || kind === 'area' || lineMs > 0,
    bars: kind === 'bar' || kind === 'waterfall' ? !(lineMs && lineMs === ms.length) : false,
  };
}

// ---- where settings live: one order and one name for each part of a visual, whatever its kind.
// Every option goes to the part it changes (by its key, else by its group); a test checks that
// each kind's options land in these sections and nowhere else (dev/checks/inspector.mjs).
export const SECTION_ORDER = ['Type & view', 'Layout', 'Series', 'Value', 'Colours', 'Marks', 'Numbers', 'Data labels', 'Centre',
  'Axes & grid', 'List', 'Legend', 'Note & breakdown', 'Change', 'Trend', 'Highlights', 'Icons', 'Card & title'];

const KEY_SEC = {
  // layout: arrangement and sizes of the marks
  stacked: 'Layout', rankStyle: 'Layout', barWidth: 'Layout', maxBar: 'Layout', inner: 'Layout', sliceGap: 'Layout', maxSlices: 'Layout',
  sortSlices: 'Layout', tmLayout: 'Layout', chipBelow: 'Layout', tmGap: 'Layout', cols: 'Layout', bridge: 'Layout', total: 'Layout',
  totalLabel: 'Layout', shareHeight: 'Layout', shareGap: 'Layout',
  // colours of the marks
  colorMode: 'Colours', colorUp: 'Colours', colorDown: 'Colours', colorTotal: 'Colours', colorStart: 'Colours', mono: 'Colours', baseColor: 'Colours',
  tmFill: 'Colours', tileBg: 'Colours', tileTone: 'Colours',
  // how the marks are drawn
  tint: 'Marks', outline: 'Marks', outlineWidth: 'Marks', outlineColor: 'Marks', radius: 'Marks', roundAll: 'Marks', barFill: 'Marks',
  partTint: 'Marks', partOutline: 'Marks', tmOutline: 'Marks', tileBorder: 'Marks', connectors: 'Marks', markerStyle: 'Marks',
  // numbers
  format: 'Numbers', units: 'Numbers', decimals: 'Numbers', signed: 'Numbers', suffix: 'Numbers', compact: 'Numbers',
  // labels on the marks
  labels: 'Data labels', labelPos: 'Data labels', labelColor: 'Data labels', labelBold: 'Data labels', labelBox: 'Data labels',
  labelUnits: 'Data labels', labelDecimals: 'Data labels', labelSuffix: 'Data labels', shareText: 'Data labels', tmLabels: 'Data labels',
  valueLabels: 'Data labels', nameLabels: 'Data labels', hlDecimals: 'Data labels',
  // scales
  axis: 'Axes & grid', ticks: 'Axes & grid', signedTicks: 'Axes & grid', yMin: 'Axes & grid', yMax: 'Axes & grid', grid: 'Axes & grid',
  y2Min: 'Axes & grid', y2Max: 'Axes & grid', y2Units: 'Axes & grid', y2Axis: 'Axes & grid', y2Color: 'Axes & grid', y2Ticks: 'Axes & grid',
  y2Step: 'Axes & grid', y2Signed: 'Axes & grid', axisColor: 'Axes & grid', yStep: 'Axes & grid', gridDir: 'Axes & grid', gridStyle: 'Axes & grid', zeroLine: 'Axes & grid', xLabels: 'Axes & grid', gridColor: 'Axes & grid', gridWidth: 'Axes & grid',
  zeroColor: 'Axes & grid', tileGap: 'Layout', tilePad: 'Layout',
  // what stands out
  extremes: 'Highlights', extremesRule: 'Highlights', bands: 'Highlights', bestLabel: 'Highlights', worstLabel: 'Highlights',
  callout: 'Highlights', calloutBg: 'Highlights', calloutColor: 'Highlights', markExtremes: 'Highlights', markBest: 'Highlights',
  rankHighlight: 'Highlights', highlight: 'Highlights',
  // a KPI's card colour belongs with its card
  tone: 'Card & title',
};
// a group's options that aren't listed above go to its part
const GROUP_SEC = { Style: 'Layout', Layout: 'Layout', Bar: 'Layout', 'Colours & marks': 'Colours', Colours: 'Colours', Values: 'Numbers',
  'Data labels': 'Data labels', 'Axis & grid': 'Axes & grid', Scale: 'Axes & grid', Legend: 'Legend', Highlights: 'Highlights',
  Lines: 'Marks', List: 'List', Centre: 'Centre', Points: 'Data labels', Categories: 'Value', Value: 'Value', Card: 'Card & title',
  'Note & breakdown': 'Note & breakdown', Change: 'Change', Trend: 'Trend' };
// these keep their group's part: the same key means something else there
const KEEP = { Centre: ['signed', 'suffix'], Change: ['colorUp', 'colorDown'],
  // a donut's and a 100% bar's amounts are in the legend (there are no labels on the marks)
  Legend: ['labelColor', 'labelUnits', 'labelDecimals', 'labelSuffix', 'decimals'] };
// a heading inside a part, where its options come from a group of their own
const SUBHEAD = { Lines: 'Lines', Categories: 'Categories', Points: 'Points', Scale: 'Scale' };

/** The part an option belongs to. */
export function sectionOf(it, groupTitle) {
  if (it.sec) return it.sec;
  if ((KEEP[groupTitle] || []).includes(it.key)) return GROUP_SEC[groupTitle];
  return KEY_SEC[it.key] || GROUP_SEC[groupTitle] || groupTitle;
}

const OPEN = new Set(['Layout', 'Value']);

/**
 * The settings of a kind as parts: { title → { count, rows } } in SECTION_ORDER. `own` is what is
 * set at this level, `eff` the effective settings, `set(key, value|null)` writes one (null resets it).
 */
export function vizParts(kind, ctx, own, eff, set, ui) {
  const { row, seg, toggle, selectInput, textInput, colorField } = ui;
  const parts = new Map();
  const seen = new Set();
  const lastSub = new Map();
  for (const g of SCHEMA[kind](ctx)) {
    if (g.when && !g.when(ctx, eff)) continue;
    for (const it of g.items) {
      if (it.when && !it.when(ctx, eff)) continue;
      // one control per setting: the first place it appears
      if (it.key) { if (seen.has(it.key)) continue; seen.add(it.key); }
      const title = sectionOf(it, g.title);
      if (!parts.has(title)) parts.set(title, { count: 0, rows: [] });
      const part = parts.get(title);
      // a heading inside the part: the option's own (e.g. "Right axis"), or its group's
      const subOwn = typeof it.sub === 'function' ? it.sub(ctx) : it.sub;
      const sub = subOwn || (SUBHEAD[g.title] && SUBHEAD[g.title] !== title ? SUBHEAD[g.title] : '');
      if (sub && lastSub.get(title) !== sub) part.rows.push(h(`<div class="vz-sub">${esc(sub)}</div>`));
      lastSub.set(title, sub);
      if (it.type === 'hint') { part.rows.push(h(`<div class="hint${it.tone === 'info' ? '' : ' warn-hint'}">${esc(it.text)}</div>`)); continue; }
      const key = `viz.${it.key}`;
      const mine = own[it.key];
      const v = eff[it.key];
      let ctl;
      switch (it.type) {
        case 'seg': ctl = seg(key, it.showHide ? (v === 'none' ? 'none' : 'all') : String(v), it.options, (x) => set(it.key, x)); break;
        case 'select': ctl = selectInput(key, v == null ? '' : String(v), it.options, (x) => set(it.key, it.num ? (x === '' ? null : +x) : x)); break;
        case 'toggle': ctl = toggle(key, !!v, (x) => set(it.key, x)); break;
        // a colour set by name shows the name (what's drawn has it resolved)
        case 'color': ctl = colorField(key, (typeof mine === 'string' && mine[0] === '$' ? mine : v) || null, (c) => set(it.key, c)); break;
        case 'text': ctl = textInput(key, mine ?? '', (x) => set(it.key, x === '' ? null : x), { placeholder: it.ph || (v != null ? String(v) : '') }); break;
        case 'num': {
          // "auto": what it came to when last drawn (the arrows start from it)
          const dv = ctx.drawn?.[it.key];
          const ph = (it.ph === 'auto' || !it.ph) && dv != null && (v == null || v === 0 || it.ph === 'auto') ? `auto · ${dv}` : it.ph || (v != null ? String(+(+v).toFixed(2)) : 'auto');
          // an axis's ends and step follow the size of the numbers (0.15 steps by 0.01, 400 000 000 by 10 000 000)
          const scaleKeys = ['yMin', 'yMax', 'y2Min', 'y2Max', 'yStep', 'y2Step'];
          const ref = Math.abs(+(mine ?? dv ?? v ?? 0)) || (it.key.startsWith('y2') ? Math.abs(+(ctx.drawn?.y2Max ?? 0)) : Math.abs(+(ctx.drawn?.yMax ?? 0)));
          const step = it.step ?? (scaleKeys.includes(it.key) && ref > 0 ? 10 ** (Math.floor(Math.log10(ref)) - 1) : undefined);
          ctl = textInput(key, mine ?? '', (x) => set(it.key, x), { type: 'number', min: it.min, max: it.max, step: step != null ? +step.toPrecision(1) : undefined, placeholder: ph });
          break;
        }
        case 'range': {
          const cur = v ?? (it.def ? it.def(eff) : it.min);
          ctl = h(`<div class="vz-range"><input type="range" data-key="${key}" min="${it.min}" max="${it.max}" step="${it.step}" value="${cur}"><span>${esc(it.fmt ? it.fmt(+cur) : String(cur))}</span></div>`);
          const inp = ctl.querySelector('input'), lab = ctl.querySelector('span');
          inp.addEventListener('input', () => { lab.textContent = it.fmt ? it.fmt(+inp.value) : inp.value; });
          inp.addEventListener('change', () => set(it.key, +inp.value));
          break;
        }
        default: continue;
      }
      const r = row(it.label, ctl);
      if (mine != null && mine !== '') {
        part.count++;
        resettable(r, r.querySelector('label'), () => set(it.key, null));
      } else {
        // not set here: where the value comes from, when it isn't the built-in default
        const from = ui.srcOf?.(it.key);
        if (from) {
          r.classList.add(from.kind === 'view' ? 'vz-from-view' : 'vz-from-theme');
          const lab = r.querySelector('label');
          if (lab) lab.title = `From ${from.what}`;
        }
      }
      if (it.help) r.title = it.help;
      part.rows.push(r);
    }
  }
  // a part of hints alone has nothing to set
  for (const [t, p] of parts) if (!p.rows.some((r) => r.classList?.contains('f-row') || r.querySelector?.('.f-ctl'))) parts.delete(t);
  return parts;
}

/** Parts in SECTION_ORDER (any not in it go last, in the order they came). */
export function ordered(parts) {
  const rank = (t) => { const i = SECTION_ORDER.indexOf(t); return i < 0 ? 999 : i; };
  return [...parts.entries()].sort((a, b) => rank(a[0]) - rank(b[0]));
}

/** The settings as sections, in order (`ui.group(title, count, rows, key, open)`). */
export function vizGroups(kind, ctx, own, eff, set, ui) {
  return ordered(vizParts(kind, ctx, own, eff, set, ui)).map(([title, p], i) => ui.group(title, p.count, p.rows, `${kind}.${title}`, i === 0 || OPEN.has(title) || p.count > 0));
}

/** The effective settings a template-wide editor shows for a kind. */
export function defaultsFor(st, kind) {
  return resolve(st, { kind, opts: {} });
}

export { PRESETS };
