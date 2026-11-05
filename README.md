# Quire

A desktop report designer and generator built in Rust (Tauri v2), with a plain HTML/JS UI and no build step.

**Template designers** connect data, lay out pages on a grid, configure KPI cards, charts and tables, and decide
what report users may change. **Report users** open a template, pick a period and parameters, add widgets from
the template's gallery, write comments, rearrange, and export to PDF or PowerPoint.

**User guide with screenshots and test steps:** [ui/help/guide.md](ui/help/guide.md), also in the app under Help → User guide (F1).

**What changed:** [RELEASES.md](RELEASES.md) · **How it's tested:** [TESTING.md](TESTING.md) · **Planned:** [backlog/](backlog/README.md)

## Run

```sh
source ~/.cargo/env   # once per terminal, if cargo isn't on PATH yet
cargo run             # the desktop app (dev build)
cargo test            # engine tests
```

**In a browser.** The same app runs as a web server: one core, one UI, two ways in. The desktop app is unchanged.

```sh
cargo run --no-default-features --bin quire-server    # then open http://127.0.0.1:8766/
```

Options: `--port 8766`, `--data DIR` (accounts, settings, themes, logs and saved files; default `~/.quire-server`,
or `QUIRE_DATA`), `--host` (default `127.0.0.1`), `--ui DIR` (or `QUIRE_UI`), `--allow-dir DIR` and `--allow-host HOST`
(repeatable; below), `--secure` (the session cookie only travels over HTTPS), `--dev` (lets pages from other local ports
call it). The server builds without Tauri (`--no-default-features`), so it needs no window system.

Each browser gets a workspace of its own (a cookie): its own sign-in, open documents and undo, kept across page
reloads and closed after 12 idle hours. In the data folder, the libraries are shared as in the desktop app
(`users.json`, `templates/`, `reports/`, `themes/`, `uploads/`), and each person has their own settings — their Python
too — in `users/<login>/settings.json` and their saved reports and exports in `files/<login>/`. The Reports area lists
the person's saved reports (*Saved*). Exports, briefs, CSVs and batch reports download; files picked with *Browse…* or
*Open* are uploaded. Only documents inside the data folder can be opened. *Show in Finder* and the screen eyedropper
(except Chrome and Edge's own) are desktop-only.

**On a server** (plan and what's left: [backlog/web-server-mode.md](backlog/web-server-mode.md)):

- **Secrets.** AI keys and database passwords are kept in `secrets.json`, sealed (ChaCha20-Poly1305) with
  `QUIRE_SECRET_KEY` — any passphrase of 16+ characters, kept outside the data folder (without it the server makes
  `master.key` in the data folder and says so). Connections are saved without their passwords; the page sees `••••••`.
  The desktop app keeps using the macOS Keychain.
- **What data may be read.** Files only from the data folder (uploads), the demo database's folder and `--allow-dir`
  folders — data sources, DuckDB's own file functions (`read_csv`, `'x.parquet'`, symlinks too) and images alike; no
  SQLite `ATTACH`. With `--allow-host`, database servers only those hosts.
- **Python is off** on a server until an administrator switches it on (**Admin → Access**): a script runs as the server,
  outside the folders above. While it's off, Python data sources, the SQL workspace's Python and Settings → Python
  don't run and aren't offered. The desktop app always has Python.
- **Sign-in.** Five wrong passwords in a row rest the login for 15 minutes.
- **HTTP.** Plain HTTP: run it behind an HTTPS proxy with `--secure`. Requests over 300 MB, slower than 2 minutes or past
  512 at once are refused; pages can't be framed by other sites.
- **Docker.** `docker build -t quire-server .` then
  `docker run -d -p 8766:8766 -v quire-data:/data -e QUIRE_SECRET_KEY='…' quire-server` (Python 3 with pandas and curl
  inside; the demo database at `QUIRE_DEMO_DB`).
- **Load.** `node dev/load/reports.mjs <people> <url>`: that many bankers at once, each signing in and opening a
  client's report. A person's sign-in, report and one period change cost about 3 CPU-seconds, nearly all of it the
  report's SQL over the bank data, so the server serves about as many people at once as it has cores (TESTING.md § 6).

Python data sources use your local `python3` (pandas optional). Set `QUIRE_PYTHON=/path/to/python` to pick one.

**A client's typeface.** The Сбер themes can use a typeface the client supplies, which isn't in the repository. Put
its font files in `ui/fonts/private/` (kept out of git) with `families.txt` (`green = Display family, Text family`,
the same for `graphite`) and `fonts.css` (its `@font-face` rules for the screen); the build embeds them (`build.rs`).
Without them those themes use IBM Plex Sans and Inter.

**Demo database.** `data/demo.duckdb` is in the repository; `python3 data/make_demo_db.py` rebuilds it (needs `duckdb`).

## Versions and licence

The version is in `Cargo.toml` only (MAJOR.MINOR.PATCH); the app shows it on the sign-in screen and
`quire-server --version` prints it. `dev/version.sh 0.2.0` sets a new one and tags it `v0.2.0` once its
[RELEASES.md](RELEASES.md) entry is written.

Quire is licensed under the [GNU Affero General Public License v3.0](LICENSE) (`AGPL-3.0-only`): anyone may use, change
and share it, and whoever shares a changed version — or runs one as a server for others — must publish its source under
the same licence. Bundled third-party parts keep their own licences (below).

## Files

| Extension | What |
|---|---|
| `.rtpl` | Template: data sources, pages, widgets, gallery, theme, end-user permissions |
| `.rpt` | Report: a self-contained snapshot of its template plus the user's period, layout and comments |

Both are pretty-printed JSON. Relative data paths resolve against the template's folder.

## Layout

| Path | What |
|---|---|
| `src/lib.rs` / `src/app.rs` | The core as a library: the app's state, every command, and `dispatch` — the one command layer the desktop app and the web server share |
| `src/host.rs` | What differs between them (`Host`): the app's folder, file dialogs, eyedropper, Finder |
| `src/main.rs` | The desktop app: Tauri window, native dialogs, one `call` command into `dispatch` |
| `src/bin/quire-server.rs` | The web server: serves `ui/` and `POST /api/call` |
| `ui/platform.js` | Desktop or browser: how the UI reaches the core, and what only the desktop app has |
| `src/model.rs` | Document model shared by templates and reports |
| `src/layout.rs` | Grid fit engine: collisions, compaction, shrink-to-fit, fill, issues & one-click suggestions |
| `src/vm.rs` | View model: page geometry, Rust-wrapped text, table metrics, per-page fit analysis |
| `src/fonts.rs` | Font store: bundled Inter, system fonts, embedded document fonts |
| `src/text.rs` | Text measurement and wrapping with the report's fonts |
| `src/query.rs` | Query engine: period windows, filters, group-by, aggregations, top-N + Other, KPI comparisons |
| `src/period.rs` | Reporting periods, comparison ranges, trailing windows |
| `src/data.rs` | Sources: CSV, Excel, SQLite, PostgreSQL, ClickHouse (over HTTP, through the system's curl), DuckDB (bundled), Python, inline |
| `src/export.rs` | Vector PDF (svg2pdf, fonts embedded) and native PPTX (text boxes, tables, SVG charts + PNG fallback) |
| `src/sample.rs` | The "Sales Performance Review" sample template |
| `src/app.rs` | Commands, edits & end-user permission checks, undo/redo, files |
| `ui/render.js` | One renderer for screen, PDF and PPTX (charts, KPIs, text) |
| `ui/fontui.js` | Font picker, role editors, font loading |
| `ui/table.js` | Table styling: formats, rules, pills, bars, scales; SVG and PPTX output |
| `ui/canvas.js` | Pages, selection, drag/resize on the grid, gallery drops, inline text, fit advisor |
| `ui/panels.js` | Gallery / widgets / data / pages rail and the inspector |
| `ui/app.js` | Title bar, period & parameter controls, home screen, shortcuts, export |
| `src/ai.rs` | AI providers (Claude, ChatGPT, DeepSeek, Qwen, GigaChat) over system curl, Keychain keys, request log, the SQL assistant's prompt |
| `ui/ai.js` | The top bar's AI button (⌘I): the assistant of the screen in front, or what AI offers each role |
| `src/ai_design.rs` | The design assistant: what it is told of a template (structure only), and its page plans checked against the template |
| `src/ai_restyle.rs` | Restyling after a picture: the theme look a vision model reads and writes, checked (colours, installed typefaces, contrast) |
| `ui/aichat.js` | What the AI panes share: the provider chip, saved conversations, the history menu |
| `ui/aiassist.js` | The SQL workspace's AI pane: write, explain, fix, make a report source, questions about the database |
| `ui/aidesign.js` | The designer's AI pane (right of the inspector): build a page, change the page in front; one undo step each |
| `src/ai_admin.rs` | The administrator's assistant: the activity and access as the model reads them (people, bankers and clients as codes), its tasks (ask the log, explain a failure, access in words, review), and a proposal made exact |
| `src/ai_report.rs` | The report assistant: the report's figures page by page with the client and codes masked, and its tasks (brief & pitch, check, page note) |
| `ui/aireport.js` | The report user's AI pane: brief & pitch, check before sending, a draft note added to the page on request |
| `src/audit.rs` | The activity log: sign-ins, administration, documents, edits, data work and AI settings in `activity.jsonl`, read back with the AI requests for Admin → Activity |
| `src/auth.rs` | Sign-in, roles, client access (bankers' coverage × products), groups; accounts in `users.json` in the app data folder |
| `src/dbconn.rs` | Database connections: their tables, read-only SQL with a preview, `:parameters` filled in, edits written back from the workspace |
| `src/python.rs` | Python data sources: a persistent `python3` worker spoken to over JSON lines |
| `src/looks.rs` | Chart presets ("clean", "soft") folded into the theme's chart defaults |
| `src/theme_studio.rs` | Themes, the theme studio document (one page per design view) and the theme library |
| `src/uhnwi_theme.rs` | The built-in themes (Quire, Modern UHNWI, Alpine, Сбер, Сбер PB, …): a look + a shape language, and the shared design views |
| `src/uhnwi_sample.rs` / `src/uhnwi_sql.rs` | The "Modern UHNWI" example report and its data sources as SQL on the demo bank database |
| `src/client_sample.rs` | A second private-banking example template with generated data |
| `ui/state.js` | Shared UI state, `call()` to the Rust core, edits, undo/redo, documents |
| `ui/rail.js` / `ui/areas.js` | The rail (areas, open documents) and each area's page: Reports, Design, Data, Admin |
| `ui/brief.js` | The report assistant's brief three ways: highlighted in the pane, formatted for an e-mail, and as an A4 PDF in the report's style |
| `ui/aiadmin.js` | Admin's AI drawer: questions about the log with links, access in words shown as before/after and applied on confirmation |
| `ui/admin.js` | Admin's Access overview, AI settings (policy, bank keys, who may send figures, use) and the Activity log |
| `ui/auth.js` | The sign-in screen, the account menu, what the person's roles allow |
| `ui/charts.js` / `ui/vizschema.js` | Charts, KPIs and tiles from one resolved settings object; the schema that builds their *Visualization* settings |
| `ui/rowlist.js` | A bar chart's *List* layout, drawn by the table engine |
| `ui/richtext.js` | Rich text in notes and text blocks: icons, sizes, colours, and its editor |
| `ui/repeat.js` | Repeating pages: the dialog, the page badges and the row picker |
| `ui/studio.js` | The theme studio's workspace: object types on the left, the overview or a gallery of design views |
| `ui/dbx.js` / `ui/sqled.js` / `ui/pylang.js` | The SQL workspace, its editor (highlighting, completion, formatting) and Python highlighting/completion |
| `ui/colorpick.js` | The colour picker in a popover: shade square, hue, hex field, screen eyedropper |
| `ui/icons.js` / `ui/glyphs.js` / `ui/logolib.js` | Interface icons; report icons and flags (SVG, exact in PDF and PowerPoint); company logos |
| `ui/help.js` | The in-app user guide (renders `ui/help/guide.md`) |
| `ui/ui.js` / `ui/util.js` | Menus, popovers, modals, toasts; text measurement and number formatting |
| `dev/shots/` | Headless-Chrome screenshots of the real UI for the user guide (see its README) |

## Concepts

- **Grid.** Pages are a 12 × 12 grid by default (configurable). Widgets snap to cells.
- **Fit to page.** After each edit the fit engine pushes overlapping widgets down, closes gaps, shrinks flexible
  charts so the page fits (never below a readable minimum or below what text needs), and stretches widgets into
  spare space. Whatever it can't solve becomes an issue in the page's fit pill with concrete fixes (fit
  everything, move to the next or a new page, show top N + Other, grow to fit text, remove…). Hover a fix to
  preview it.
- **Periods & parameters.** Month / quarter / year / YTD / custom, compared with the previous period or the same
  period last year. Widgets choose a window (selected period, YTD, last 3/6/12/24 months, all). Template
  parameters (e.g. Region) feed widget filters via `{{region}}` and SQL via `:region`; SQL also gets `:start`
  and `:end`.
- **Tables.** Summary mode groups by up to three fields; detail mode lists rows as they are (e.g. a trade
  blotter). Values can show this period, the comparison period, the change or the % change. Every column can
  be styled: number/date format, decimals, prefix/suffix, width, alignment, bold/colour/fill, green-red by
  sign, ▲▼ arrows, data bars, colour scales, pills (badges such as BUY / SELL) and conditional rules that
  colour a cell or the whole row. Click a column in the table's **Columns** list to open its settings in the
  inspector (the table stays visible while you edit; arrows step to the neighbouring column). Only settings
  that can show are offered: a number column is shown as **Number**, **Bar** or **Both** (a bar alone has no
  number to format; bars have colour, negative colour, thickness, round ends and a background track); a text
  column gets **Pills** or a **Marker** (an icon per value, chosen from a grid, or a series-colour swatch).
  **Best & worst** (table Design tab) marks the best and worst rows by any value, shown or hidden: pick the
  column it ranks by, whether lower is better, how many rows of each, and whether to show an icon after the
  row's name, a tinted row with a stripe, or both, with their icons and colours. **Add another ranking** marks
  by a second value too (e.g. return and result). Icons say which value a mark is about (arrows for returns,
  coins for amounts; colour says best or worst). A row hit by several rankings shows every icon in list order;
  only one fill can show, so the first ranking set to Row or Both decides it. Optional labels ("best return")
  appear under the name, joined with "·".
  **Highlights & rules** can put an icon after the row's name (e.g. **Mark best and worst rows**), add a
  label or note, and colour the cell, row, stripe or pill. **Row height** offers Compact, Normal and Roomy
  presets or exact points. Tables with pills or bars export to PowerPoint as an exact vector picture;
  others as editable native tables (switchable per table).
- **More table layouts.** Pivot a field across the columns ("Columns by", with row totals); subtotals with
  indented group rows, or only the group totals; long tables continue onto automatically added pages with the
  header repeated; header groups spanning columns; per-column totals (auto, sum, average, min, max, count,
  none); icon sets (traffic lights, arrows, checks) by threshold; wrapping text cells; value columns for share
  of total, rank, running total and a 12-month sparkline per row.
- **SQL.** Per data source: calculated fields (`name = SQL expression`) and a transform query over `data` that
  can join other sources by name. Per widget: a SQL filter (`WHERE` condition). Runs in a sandboxed DuckDB
  with no file or network access.
- **Empty widgets explain themselves** (no rows in the period, filters, unreadable dates) and offer a fix. Adding a
  data source moves the template's period onto the data if needed.
- **Typography.** Any installed font, or a font file embedded in the template (so it renders the same
  anywhere). Eleven roles — headings, heading subtitles, widget titles/subtitles, text & comments, captions,
  KPI values, table headers, table cells, chart labels, footer — each with family, size, weight, italic,
  colour and capitals. The theme sets them; any widget can override any role; table columns can change
  family and italic. Rust measures text with the real font files, and PDFs embed them.
- **Report structure.** Pages belong to sections; cover pages (plain) hide the header and footer.
- **Contents.** The Contents widget lists sections and pages with page numbers. Its Content tab shows the
  entries; click one to edit that page's section, title and description (the description appears under the
  entry). The Design tab sets the **Layout** (**List**: section labels in small capitals; **Chapters**: a block
  per section with a number, icon and serif name), what to **Show** (sections and pages, sections only, pages
  only), 1–3 **Columns**, **Spacing**, page numbers with dotted, solid or no **Leader**, descriptions, lines
  between entries, section **Numbering** (I, II / 1, 2 / A, B), a **Label** such as "Chapter", the section
  colour, and for chapters the icon chip and an icon per section. The theme studio has a **Contents** page
  with List and Chapters views, previewed on sample entries.
  - **Smart entries.** Titles can come from **Page titles** or each page's main **Heading** (the largest heading with
    words in it), and descriptions from the page notes or the heading's subtitle, so the contents follows the pages
    as they are edited; page numbers always do. **Pages** lists those in a section, or all of them.
  - The Content tab shows every entry with a switch to leave a page out, and a **Left out** list (cover page, no
    section, hidden) with **+** to add one back. **Fill in from the report…** proposes missing sections (from the page
    before), titles (from the page heading) and descriptions (heading subtitle or the first sentence of the page's
    text) for review; applying them is one undo step.
  - The contents fits its box: spacing tightens first, then descriptions are dropped if needed.
- **Header & footer.** Each is a band with its own height, background (bleeding to the page edge), divider
  line and colour, and section tabs placed left, centre, right or full width. Left, centre and right slots
  each hold multi-line text, a logo image and their own font. Text can mix anything: `{{page}}`, `{{pages}}`,
  `{{section}}`, `{{title}}`, `{{period}}`, report parameters, any field or calculated field
  (`{{data:Dataset.field|sum}}`) and SQL over the datasets (`{{sql: select count(*) from positions where
  date between :start and :end}}`). **Insert** in the slot editor offers all of these, and the editor shows
  the resolved text.
- **Language.** English, Russian or German formatting per template: `21 649 000 ₽`, `+2,34%`, `10,0 млн ₽`,
  `15.06.2026`, month and quarter names.
- **Personalised text.** `{{start}}`, `{{end}}`, `{{today}}`, and `{{data:Source.field}}` (optionally `|first`,
  `|sum`, `|count`, `|list`…), which follows report parameters such as the client.
- **More visuals.** Treemap, waterfall, bar + line combo, tiles, donut legends with amounts; tables with a second
  line per cell (e.g. ISIN), bar-only share columns, row stripes, + signs and "—" for zero.
- **Configurable visuals.** Each bar, column, line, area, waterfall, donut, treemap, tiles and KPI widget has a
  **Visualization** section that sets every visual choice:
  - **Colours and marks:** colour by series or by sign, the up, down and total colours, lightening and
    outlines, corner radius, bar width.
  - **Data labels:** which values, where, in which colour; units, decimals, + sign, suffix and format.
  - **Axis and grid:** axis, ticks, min and max, grid, zero line, category labels.
  - **Legend:** position and texts.
  - **Highlights:** best and worst bands and labels, callout pills.
  - **Lines:** width, dash, smoothing, markers, area opacity.
  - **Per-kind options:**
    - waterfall: total bar, label and connectors
    - ranked bars: highlight row, rank numbers, track
    - donut: hole size, slice gap, single-hue shades, centre content, table legend with amounts and shares, max slices
    - treemap: layout, chips, share caption
    - tiles: columns, fill, border, best/worst marks
    - KPI: alignment, size, colour, format, change as %/points/amount, "up is good", sparkline as line or bars and its position

  **Chart preset** (theme → Charts) is the one starting point for every chart in a theme. *Clean* means solid marks and few labels. *Soft* means tinted, outlined and rounded marks, every value labelled, best and worst marked, the last value as a pill, and donuts in shades of one colour. Everything a preset does is also a visible setting (e.g. *Colour by: Up / down, 1 series*, *Signed ticks*, *Bold* labels, *Caption in capitals*), so design views and widgets override it setting by setting. Older files that set a look on a widget, design view or chart default are converted on load into the settings it stood for, and look exactly the same. **Theme → Chart defaults** sets the template-wide
  defaults per chart kind, and **Use as default** promotes a widget's settings to them.
- **Table styling extras:** fixed row and header heights, row and header line colours, totals row fill, text
  colour and line (single, thick, double underline), group row text colour.
- **Themes and design views.**
  - **What a theme holds:** colours, fonts and *design views*, which are named looks for each widget kind. For example, KPI: "Big number", "Toned card", "With breakdown"; table: "Grouped", "List"; bar: "100% bar".
  - **How templates use it:** a template picks a theme; each widget picks a design view (inspector → **Design** → Design view) and keeps only its own content and overrides. New widgets take the kind's default view. **+** next to the picker saves a widget's look as a new view.
  - **Theme studio** (top bar → **Theme**, or Home → Themes) is its own workspace:
    - **Left rail:** *Overview* plus one entry per object type (KPI cards, tables, bar/line/area/donut charts, waterfalls, treemaps, tiles, headings, text, comments, dividers) with its number of views.
    - **Sample data** follows the theme's language (English, Russian, German), including measure names, titles, the category column and the totals row. Changing the language in **General** relabels the previews at once.
    - **Overview:** a specimen of the theme: colour tokens, series colours, every type role in its font, and a tile per object type.
    - **Object type:** a gallery with one card per design view. Each card shows the view on its own with sample data, sized to what it draws, so nothing is cut off. It also shows the view's name, description and the *Default* badge. Hover a card to duplicate it, or open its menu to rename, make default, reorder or delete.
    - **New design view** (button or the last card): name it, say when to use it, and pick what it starts from: *Blank* or any existing view, shown as thumbnails.
    - Selecting a card edits that view on the right, under **Design** (chart type, each visualization group, table columns/header/lines/totals, card & frame, colour), **Text** (type roles) and **About** (name, description, preview data).
    - With nothing selected, the right panel edits the theme: **Colours**, **Type**, **Charts** (cards, chart defaults) and **General** (name, language, currency).
    - **Save theme** writes a `.qtheme` file and adds it to your library.
    - **Back to template** can apply the theme on the way out.
  - **Theme library** (Home → Themes → *Manage themes…*, or Theme → *Manage themes…*): rename, duplicate, delete or show in Finder the themes you've saved, open any of them in the studio, and duplicate a built-in theme into your library as a starting point. Deleting a theme doesn't change templates that use it, because each template keeps its own copy.
  - **Applying a theme:** Theme → *Apply a theme* puts a library or built-in theme on a template. Widgets keep their design-view ids, so a theme that uses the same ids restyles the whole report.
  - **Built-in themes:** nine looks that share the same ~45 design views and differ in colour, type and shape, so any of them restyles the UHNWI report in one click:
    | Theme | Look | Type |
    |---|---|---|
    | *Quire* | the house dashboard: white paper, grey cards, blue first, pastel marks, result cards filled green or red | IBM Plex Sans |
    | *Modern UHNWI* | warm paper, brown-black ink, oxblood red | Spectral + Manrope |
    | *Alpine* | white, graphite, one Swiss red — precise, minimal | IBM Plex Sans (light headings) |
    | *Сбер*, *Сбер Private Banking* | the banks' own sites | the client's typeface (IBM Plex Sans and Inter without it) |
    | *Изумруд* | white, emerald, green header bands | Raleway |
    | *Инфографика* | flat infographic: navy, teal, sunflower | Montserrat |
    | *Bold Blocks* | figures on solid brand panels, black type | Montserrat + Inter |
    | *Data Atlas* | data journalism: pastel tiles, condensed type, one red | Roboto Condensed |
  - **Colour tokens:** any colour in a view, widget, rule or text markup can be `$name` (`$accent`, `$good`, `$bad`, `$muted`, `$class1`…, or any custom token in the theme's `tokens`). Tokens resolve against the template's theme, so swapping themes recolours everything, including `{$accent|words}` in text and the page frame.
- **Inspector:** the right panel is split into categories with icons, each colour-coded: **Content**, **Data** (teal), **Design** (blue), **Text** (amber) and **Layout** (slate) for a widget; **Document**, **Page**, **Theme** and **Access** for the template. Inside, every group is a collapsible section with its own icon and a badge counting the settings changed at this level; Quire remembers which sections you keep open. Delete, Replace and Save to gallery stay pinned at the bottom.
- **Chart type** (Design → Chart type): a gallery of icon tiles: columns, bars, 100% bar, line, area, donut, treemap, tiles and waterfall. Switching keeps the data and takes the new type's default design view, and ⌘Z undoes it. Types that draw one series over one set of categories are greyed out until the chart has exactly one “Group by” field.
- **Design view picker:** shows the current view's name and description. It opens a list of the theme's views for that kind, with descriptions and the default marked, plus *Own look* and a shortcut to the theme studio.
- **Totals rows** default to the report language (Total / Итого / Gesamt).
- **Chart colours:** bar, line, area, donut, treemap and tiles widgets list their series or categories under **Design → Series/Category colours**, each with its own colour. Colours are kept by name (`viz.colors`), so a category keeps its colour in every period. Treemaps also have **Colours**: colour by category or by result (up/down), the up and down colours, and fill strength. When a chart colours by result (or a donut uses shades of one colour), the per-name list is hidden: only the colours that apply are shown. Text switches to light on strong fills.
- **Icons and flags:** small line icons (cash, trend, layers, diamond, briefcase, percent, grid, document, calendar, swap…) plus RU/US/CN flags and buy/sell/in/out badges. They appear in table cells (a value → icon map), in chips before widget titles, after row names (from rules), and in tile, donut and share-bar legends. All export as vectors.
- **More visuals:**
  - treemaps coloured by result
  - share bars with labels inside, colour by result and row legends
  - KPI value chips ("+2,1" next to a median)
  - a contents page by section (Roman numerals, icons, page descriptions, two columns)
  - card padding per widget
- **Design-system pages** (built for the *Modern UHNWI* report design):
  - **Fonts:** Manrope, Spectral, Cormorant Garamond, Playfair Display and IBM Plex Sans are bundled (OFL, in `ui/fonts/`), and every text role has letter spacing.
  - **Text styling:** text and headings accept `*italic*`, `**bold**` and `{#b1542f|coloured}` words, and can be aligned left, centre or right.
  - **Headings** can have:
    - an eyebrow line of small capitals
    - a dark badge chip (e.g. `№ 400PX0F`)
    - a note on the right
    - a rule underneath
    - page navigation across a page group (Обзор · Состав 1 · Состав 2; set each page's group and label in its Page dialog)
  - **Cards:** any widget can have its own outline colour and width, fill, divider lines on chosen sides, and a ▲/▼ chip before its title. Dividers can be vertical, and pages can have an inset frame.
  - **KPIs:**
    - Cards can take the colour of their result, tinted or solid.
    - An ↗/↘ arrow can sit before the value, and the value can be right-aligned or placed on the title line.
    - A second figure can sit beside the value.
    - A note line can embed other values (`{1} profit since January`).
    - The other values can be listed as rows, e.g. top-ups and withdrawals.
  - **New visuals:**
    - a *100% bar*, with a legend of shares and amounts
    - a *Range* bar (a bar chart type) for benchmarks or a percentile: points on one scale, a gradient track, min/max ends and a highlighted point
    - bar charts have two layouts: *Chart* (axis) and *List* (a row per category: name, bar, value, the line under it); the List is drawn by the table engine, so Bars (List) ↔ Table keeps the look both ways
    - donuts of signed contributions, with the signed total in the centre
  - **Tables:**
    - colour swatches per category (square, dot or tile)
    - a tag after a cell from another column, or from a rule (`{v} days`)
    - rules that add a note under the row name (e.g. ▲ best result) or show the value as a pill
    - group rows in caps with a count and a rule underneath, values optionally hidden
    - mini bars under numbers, scaled to the row's total
    - column bands and vertical rules
    - two-line headers
    - a totals row with its own font, and `{n}` in its label for the row count
    - a hidden header row
- **Sample:** *Modern UHNWI report (RU)* — the 18-page Test Private Banking design built from the Modern UHNWI theme's design views:
  - cover and contents
  - portfolio summary, overview, results and benchmarks
  - broker, trust and bank account pages, with a holdings table that continues across pages
  - monthly cash-flow forecast and its detail
  - trades and operations, trades by instrument
  - legal page
- **Sample:** *Client portfolio report (RU)* — a private-banking report built only from these features (no longer on the
  home screen; the home screen's **Examples** shows the Modern UHNWI report, opened as a template or as a report).
- **Snapshot data.** For values recorded per date that don't add up over time (balances, headcount, prices),
  use **Last value** (the value at the latest date in the period, per group), **First value** or **Average
  across dates**. Mark a source as snapshots and new values default to Last value.
- **DuckDB.** Query a `.duckdb` file, or leave the database empty to read Parquet/CSV files with SQL
  (`select * from 'sales.parquet'`); relative file names resolve next to the document.
- **Gallery.** Designers save configured widgets to the gallery; report users add or swap them in.
- **Permissions.** Per template: add, remove, move & resize, restyle, edit text, comment, pages. Individual
  widgets can be locked.
- **Placeholders** in text: `{{period}}`, `{{compare}}`, `{{title}}`, `{{subtitle}}`, `{{organization}}`,
  `{{author}}`, `{{today}}` and any parameter name.

## Keyboard

`⌘S` save · `⇧⌘S` save as · `⌘O` open · `⌘Z` / `⇧⌘Z` undo/redo · `⌘E` export PDF · `⌫` delete widget ·
`⌘D` duplicate · arrows move (⇧ resizes) · `↩` edit text · `Esc` deselect · `⌘0` fit zoom · `⌘[` / `⌘]` left panel (on area pages: their menu or list) /
inspector · `⌘I` AI assistant · `⌘,` settings · `F1` user guide.

## Dev checks

Full list and when each runs: [TESTING.md](TESTING.md). `cargo test` covers periods, SQL binding, type inference, text wrapping, queries, the fit engine, permissions,
and PDF/PPTX packaging. Ignored helpers: `QUIRE_DUMP=dir cargo test dump -- --ignored` writes UI state
snapshots. The UI is driven in a headless browser through the web server (`cargo run --no-default-features --bin quire-server -- --dev`, :8766).
`QUIRE_TRACE=1 cargo run` prints each command the desktop window sends.

## App menu and fitting

- **Menu bar.** The title bar has a **Home** button and **File, Edit, Insert, Page, View, Theme, Help** menus (new,
  open, save, export, document setup; undo and the selected widget's actions; add any widget to the current page;
  page actions and "Fit every page"; panels, grid and zoom; themes; keyboard shortcuts). With one menu open, pointing
  at another opens it. Menus follow the mode: report users see only what the template allows; the theme studio shows
  file, edit, view and help.
- **Padding.** Card & frame has **Padding** (None, S, M, L or exact points). It applies to widgets without a card or
  title too.
- **Content that doesn't fit.** Charts fit their box: a KPI's value shrinks so the note and change below it stay
  inside, a share bar's legend rows tighten, then shrink, then fold the smallest into "Other". Anything still drawn
  past its frame gets a dashed outline and a **Cut off · Fit** badge that grows the widget by the rows it needs.
- **Find a setting.** The inspector has a search box above its tabs (⌘F). Typing searches every tab at once: setting
  names, section names and the choices inside them (so "dots" finds Leader, "roman" finds Numbering), with both
  spellings (colour/color, centre/center) and a few synonyms. Matches appear as live controls grouped by
  "Tab › Section"; click a group's heading to open that section in its tab. Esc clears.
- **Right-click menus.** Widgets: edit text, duplicate, move to page, lock, save to gallery, delete. An empty spot on a
  page: add a KPI, chart, table or text right there, and the page's actions and settings. The page title above a page,
  pages in the Pages list, widget types and gallery or data items in the left panel, the inspector (find a setting,
  expand or collapse all), text fields (cut, copy, paste, select all) and the area around the pages (zoom, grid, add a
  page, save, export) each get their own. Hold ⌥ while right-clicking for **Inspect element** (developer tools).

## Data, parameters, formats and icons

- **SQL over other sources.** A dataset can be one DuckDB query over the other sources by name (CTEs, joins, window
  functions). `:start`, `:end`, `:prev_start`, `:prev_end` and `:param` are filled in there and in transforms,
  calculated fields and widget SQL filters; a multi-choice parameter becomes a list (`client in (:client)`).
- **Level of detail.** The calculated-field dialog has a builder that writes window-function SQL: the group's value on
  every row (FIXED), once per group (INCLUDE, for averages across groups) or each row's share of its group, over all
  rows, the period, the comparison or year to date.
- **Parameters.** Choice lists typed in or read from a dataset column, several values at once, text/number/date
  types, and a column the parameter filters in every source that has it. Report users pick values in the toolbar
  (searchable multi-select, reset) and the report redraws.
- **Settings (⌘,).** Language, currency, date format, separators, negative style, currency position, and defaults for
  new documents (author, organisation, page, period). Documents carry their own formats (Document → Formats).
- **Icons.** 1,539 line icons in categories, 79 flags and 68 brand marks, plus logos uploaded into the document, in a
  searchable picker; circle, rounded, square or no chip. *By name* finds a flag or logo from a value (country, currency
  code or name, in English or Russian). Sources: Lucide (ISC), flag-icons (MIT), Simple Icons (CC0); the library is
  generated into `ui/iconlib.js`.
- **Menus.** A menu bar (File, Edit, Insert, Page, View, Theme, Help), a Home button and right-click menus everywhere.
- **Variables.** The Variables tab lists every `{{placeholder}}` with its current value and how it is filled; designers
  add their own (text, an SQL query or a data value, with a format) and use them anywhere.
- **Settings follow through.** App settings (⌘,) are the default for every open and new document; a document's own
  formats (Document → Formats) win. Engine-written numbers (`{{data:…}}`, `{{sql:…}}`) follow them too.
- **App language.** English or Russian for the interface (`ui/i18n.js`, dictionary `ui/i18n-ru.js`): text, tooltips and
  placeholders are translated as they render; report content never is.
- **KPI values by role.** The Data tab names the main value and lets every other value sit beside it, in the note, in a
  chip, as rows, or nowhere.
- **Filters for report users.** Document → Access sets, for the period, the comparison and each parameter, whether
  report users can change it, only see it, or don't see it; the engine enforces it.
- **Filters reach everything.** A parameter filters each source by its column (found by name, chosen per source, or
  left out), before SQL datasets, data and SQL values, variables and level-of-detail fields read it.

## Third-party code

- `ui/fonts/` — Inter, Manrope, IBM Plex Sans, Spectral, Cormorant Garamond, Playfair Display, Raleway, Montserrat,
  Roboto Condensed (SIL Open Font License, licences alongside).
- `ui/iconlib.js` — line icons from [Lucide](https://lucide.dev) (ISC), flags from flag-icons (MIT), brand marks from
  Simple Icons (CC0; the brands are their owners' trademarks).
- `ui/logolib.js` — company logos from TradingView's symbol logos and Wikimedia Commons; the logos are their owners'
  trademarks.
- `ui/vendor/pdfjs/` — [pdf.js](https://github.com/mozilla/pdf.js) 4.10.38 (Mozilla, Apache 2.0, licence alongside): draws PDF pages as pictures for the design assistant's *Look* (restyle after a reference).
