# Designer: full control over visuals

> **Status:** ✅ done 28 Oct 2026 — category axis (every Nth, turned, date format), legend swatch shape / size / order, reference lines and bands, table cell padding, where each value comes from (ring: view, square: chart defaults) and *Changed only*. Per-series settings were done before. Earlier: 24 Oct 2026 — ✅ 1: one order and name for every part, duplicates merged, hidden settings shown (`dev/checks/inspector.mjs`); ✅ 2: click a part on the canvas to open its settings; ◐ 3: text roles for axis labels, data labels and legends, gridline / zero-line colour and width, tile gap and padding, space under titles done — legend swatch shape and order, table cell padding still to do. Step 4 not started.
> **Goal:** a designer can set every visible detail of a visual, and finds each setting where they expect it.

## What was found (24 Oct 2026)

- Chart text (axis labels, data labels, legend, callouts, a donut's centre) all comes from one "Charts" text role at
  fixed offsets (axis −0.5 pt, labels +0.5 pt, callouts +1 pt); its colour is the muted grey in 34 places in `ui/charts.js`.
- Fixed spacing: tile gap 6 pt and padding 7 pt (`tiles()`), table cell padding 5 pt (`ui/table.js`), title → chart
  and legend gaps.
- Category axis is on / off only (no rotation, every Nth label, date format); gridline and axis-line colour / width
  can't be set; the legend has no text style, swatch shape or order.
- Per series only the colour can be set; no reference lines (target, average, benchmark).
- Clicking a part of a chart on the canvas doesn't open its settings (tables already do this for columns).

## Steps

2. ✅ **Done 24 Oct 2026 — Click a part, get its settings.** Legend, axis, data labels, a series, title, donut centre: clicking one on the
   canvas opens that part of the Design tab and outlines the part. The renderers tag their pieces (`data-part`).
3. ◐ **Mostly done 24 Oct 2026 — Text and spacing for each part.** Left: legend swatch shape / size / order, table cell padding (the core lays out column widths with the 5 pt padding too). Text roles *Axis labels*, *Data labels*, *Legend*, *Callout*, *Centre value*
   (each starts as today's "Charts" role, so nothing moves until set), also in Typography; their rows sit in each
   part's section. Spacing: title → chart, legend gap, tile gap and padding, cell padding. Gridline / axis-line colour
   and width.
4. **Series, reference lines, category axis.** Per series: colour, bar or line, dash, markers, labels on/off, legend
   name (under *Series*). Reference lines and bands: a constant, the average or a field, with a label (under
   *Highlights*). Category axis: rotation, every Nth label, date format.
   - Also: show where each value comes from (theme · design view · this widget) with a reset, and a "Changed only"
     filter.

## Rules to keep

- New options go into an existing part (`KEY_SEC` / `GROUP_SEC` in `ui/vizschema.js`); `dev/checks/inspector.mjs`
  must pass.
- Every control must visibly change the output (see the *controls must work* rule).
