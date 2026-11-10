# Releases

What changed in Quire, newest first. Each entry is one working day's release: **New**, **Improved** and **Fixed**,
written for the people who use the app; file names point developers to the code.

From **0.1.0** (5 November 2026) each entry is a numbered version (`Cargo.toml`; `dev/version.sh` sets it and tags it);
the dated entries before it were working days on the way to 0.1.0. Entries before 22 October 2026 were written afterwards from the project notes, so
they are shorter than the later ones. Ideas planned but not started are in [`backlog/`](backlog/README.md).

---

## Unreleased — 10 November 2026

### New

- **The report API** (on the web server, built with `--features render`). Other systems — a CRM, a client portal, a
  scheduler — ask Quire for a client's report, or a batch of them, with their API account's key, and download the
  PDFs (one report) or a zip (a batch). `POST /api/v1/reports` answers at once with a job, or refuses a request it
  can't serve with the reason (a client outside the account's access, an unknown client, a parameter the report
  doesn't take, a bad period); `GET /api/v1/jobs/{id}` shows each report's status, pages, layout issues and the
  values used. A retried request with the same `Idempotency-Key` gets the first job; 60 new requests a minute per
  account. Jobs survive a restart; their files are deleted after the days set in Admin → API. Each request and report
  is in Activity under the account, with a new *API* filter there. One 18-page report takes about a second
  (`src/api_jobs.rs`; README *On a server*).

- **Admin → API** (on the web server): an account for each system that will ask for reports — a CRM, a client portal,
  a scheduler. Each has its own key, the clients and products it may report on, and the reports it may ask for. The key
  is shown once, when made; Quire keeps only a fingerprint of it. *New key…* replaces it at once. Below, for each
  published report: whether it is offered over the API, which parameters a request may set, and their defaults for API
  requests; and how many days finished files are kept (7 to start). Every change is in Activity, never a key
  (`ui/apiadmin.js`, `src/api_accounts.rs`).

- **The core makes a report for another system's request**: a published report (UHNWI when none is named), a client
  by the bank's id, a period as dates or by name (`ytd`, `last_month`, `last_quarter`, `last_year`) and the report's
  other parameters — those Admin lets the API set, checked against the report's own; Admin's defaults fill in the rest.
  The caller's access applies, and a request it can't serve is refused with the reason (`src/app/report_api.rs`; plan in [`backlog/report-api.md`](backlog/report-api.md)).
- **Report pages drawn without a browser.** The web server can run the app's own drawing code in V8 (Chrome's
  engine), measuring text with Quire's fonts, and write the pages as a PDF. The pages match the browser's word for
  word; positions differ by under 2 pt. Built with `--features render` (`src/render_js.rs`).

### Improved

- **Client access preview in Russian.** *Coverage* and the "Sees 5 clients" line under a person's access are translated.
- `ui/util.js` loads outside a browser (no canvas or `navigator` needed at start), so the drawing code runs on the server.

---

## 0.2.0 — 5 November 2026

### New

- **Quire on Linux.** The desktop app and the web server build and run on Linux (checked on Ubuntu 24.04), and are
  packaged as `.deb` and AppImage. AI keys and database passwords are kept sealed in the app's data folder, as on the
  server, since Linux has no Keychain; *Show in folder* opens the file manager; the screen eyedropper, macOS-only, isn't
  offered there (`src/main.rs`, `ui/platform.js`).
- **Every push is built and checked on Linux** by GitHub Actions: the engine tests, the web server and its checks, the
  desktop app and its packages (`.github/workflows/linux.yml`).

### Fixed

- **In a browser, the AI key hints said keys were kept in the Keychain.** They now say where they really are: on the
  server, sealed (`KEYS_AT` in `ui/platform.js`).

---

## 0.1.0 — 5 November 2026

The first numbered version, and the first one with its source in a GitHub repository.

### New

- **A version number.** The sign-in screen shows it (*Quire 0.1.0*), and `quire-server --version` prints it. It is
  kept in one place, `Cargo.toml` (the desktop app's `tauri.conf.json` reads it from there); `dev/version.sh 0.2.0`
  sets the next one and tags it in git (`ui/auth.js`, `src/bin/quire-server.rs`).
- **A licence.** Quire is open source under the GNU Affero General Public License v3.0 (`LICENSE`); the bundled fonts,
  icons and pdf.js keep their own licences (README → *Third-party code*).

---

## 30 October 2026

### New

- **A guide to rebuilding Quire** on another computer (Linux) with another coding model: what the app is, the rules
  it follows, its architecture and data model, 19 build phases each with checks that say when it's done, the
  macOS-only parts to replace on Linux, and lessons already learned (`REBUILD.md`).

### Fixed

- **A parameter card's warning was squeezed into narrow columns, in capitals.** In *Data → Report parameters*, the
  line under *Check* ("1 source doesn't follow it: …") took the small capital style of the card's labels, and each
  part of it (the count, the sources, the token) became a column of its own. It now reads as plain text under its
  label, as does the green *Reach* line (`.pm-line`, `.pm-reach` in `ui/styles.css`; `ui/areas.js`).

---

## 29 October 2026

### New

- **The browser version, ready for a bank's network (web phase 3).**
  - **Secrets sealed on the server.** AI keys and database passwords are kept in `secrets.json`, sealed with
    `QUIRE_SECRET_KEY` (ChaCha20-Poly1305), not in the macOS Keychain or in `users.json`. A connection's password is
    shown as `••••••` and kept when the connection is saved again (`src/secrets.rs`; the desktop app keeps the Keychain).
  - **What data may be read.** On the server, data sources read files only from its data folder (uploads), the demo
    database's folder and folders allowed with `--allow-dir` — DuckDB's own file functions and symlinks included;
    SQLite can't `ATTACH` other files. `--allow-host` limits the database servers it connects to (`data::fence`).
  - **Python off on a server by default.** A Python script runs as the server and can read whatever it can, outside
    the data fence, so the server runs none until an administrator switches it on in **Admin → Access** (with a warning
    first; logged in the activity log; kept across restarts). While off, Python data sources, the SQL workspace's
    Python tab and Settings → Python aren't offered. The desktop app always has Python (`python::check`,
    `python_policy` in `src/app.rs`, `ui/admin.js`).
  - **Sign-in.** Five wrong passwords in a row rest the login for 15 minutes — on the desktop too.
  - **HTTP.** `--secure` for running behind HTTPS (the cookie then only travels over it); security headers; requests
    over 300 MB, stalled for 2 minutes or past 512 at once refused. The accounts file is written whole and readable by
    the server's user only.
  - **Docker.** A `Dockerfile` for the server (Python 3 with pandas and curl inside); not built here (no Docker on
    this Mac) — build and try it on a Linux machine first.
  - **Checks.** `dev/web/checks.mjs` (25 checks, all pass) and the load check `dev/load/reports.mjs` (TESTING.md § 6).

### Improved

- **Reports open faster, and many at once.** A DuckDB database file is opened once and shared by every query and
  person, instead of being opened afresh — reading its whole catalogue — for each of a report's dozens of queries.
  The UHNWI report opens in 0.6 s instead of 0.9 s; 50 people at once in 11 s instead of 17 s (dev build). Writing
  cells from the SQL workspace lets the shared copy go first (`duck_shared`, `release_db` in `src/data.rs`).

- **The browser version for a team (web phase 2).** Each browser now has a workspace of its own — its own sign-in,
  open documents and undo — instead of one for the whole server; it survives a page reload and closes after 12 idle
  hours. Each person's **settings** are their own, including **their Python** (Settings → Python), and their **saved
  reports and exports** are kept in a folder of their own: nobody else can list, download or delete them. A report
  is saved under its title and client (*Отчёт по активам и доходности — LeBron Jameson.rpt*, also on the desktop), and
  never replaces another one of the same name. The **Reports** area lists your saved reports (**Saved**: client, when,
  which report) to open again or delete. The server only opens documents inside its own folder
  (`src/bin/quire-server.rs`; `Host::signed_in`, `settings_file`, `files_dir`, `may_open` in `src/host.rs`;
  `saved_documents`, `delete_saved` in `src/app.rs`; `ui/areas.js`).

- **Quire in a browser (first step of the web version).** `cargo run --no-default-features --bin quire-server`, then
  open `http://127.0.0.1:8766/`. It is the whole app — sign-in, the Reports, Design, Data and Admin areas, the editor,
  themes, the AI assistants, export — from the same core and the same interface as the desktop app, which is
  unchanged. For now the server has one workspace and one sign-in, so it listens on this computer only. In the
  browser, exports and *Save as* go to the server's `files` folder. Opening files from your computer, *Show in Finder*
  and the screen eyedropper stay in the desktop app (Chrome and Edge have their own eyedropper, which Quire uses). The
  title bar leaves no room for window buttons in a browser
  (`src/bin/quire-server.rs`, `ui/platform.js`; plan and next steps in
  [backlog/web-server-mode.md](backlog/web-server-mode.md)).
- **Files from your computer in the browser.** *Browse…* opens the browser's own file picker and uploads the file to
  the server's `uploads` folder: CSV, Excel, SQLite and DuckDB data sources, images and fonts. *File → Open* uploads a
  template, report or theme and opens it, and *Open* in the SQL workspace reads a script. The same file picked again
  reuses its copy (`upload_file` in `src/app.rs`, `ui/platform.js`).

- **Downloads in the browser.** Exports (PDF, PowerPoint), the brief's PDF, the activity log as CSV, batch reports and
  saved scripts now come to your computer as downloads, as any website's files do (`saved_file` in `src/app.rs`, only
  files the app saved; `ui/platform.js`).

### Fixed

- **Exported in the browser, but nowhere on the computer.** The file was written to the server's `files` folder and
  not downloaded; it is now (above).
- **New data source didn't work in the browser** for files (CSV, Excel, SQLite, DuckDB): *Browse…* asked the server
  for a file dialog and was refused. It now uploads the file (above).

### Improved

- **The Design start page no longer shows the Python status line** (“Python 3.13 · pandas ready for data scripts”). It
  said nothing a designer acts on there; Python is chosen and checked in **Settings → Python** (`ui/app.js`).

- For developers: **one command layer.** The core is now a library (`src/lib.rs`, `src/app.rs`), and every command —
  from the desktop window or the browser — goes through `quire::dispatch`, with the same role checks. The desktop app
  sends them all through one Tauri command, `call` (`src/main.rs`); what differs between desktop and browser (the app's
  folder, file dialogs, eyedropper, Finder) is behind `Host` (`src/host.rs`). The server builds without Tauri. The dev
  bridge, which kept its own copy of the commands and role checks, is gone: the screenshot and render checks in
  `dev/shots/` run against the server (`--dev`), unchanged apart from the command that starts it.
  `QUIRE_TRACE=1 cargo run` prints each command the desktop window sends.

---

## 28 October 2026

### New

- **Visuals, full control.** Legend swatch shape, size and order; category labels every Nth, turned 45° or 90°, dates
  written another way (01.26, Jan 2026…); a **reference line** (a value, average, median, highest, lowest) and a
  **band**, each labelled; **table cell padding** (also in PowerPoint). The Design tab marks where a value comes from
  (a ring: the design view; a square: the template's chart defaults) and has **Changed only** (`ui/charts.js`,
  `ui/vizschema.js`, `ui/panels.js`, `ui/table.js`).
- **Advanced settings…** (a widget's ⋯ menu): its own settings as JSON, checked against the schema as typed; errors
  block Apply, unknown names warn; one undo step (`ui/advanced.js`).
- **Composite widgets** (Insert → Composite): several parts in one card — a figure, charts, text, a table, an
  image — on a small grid, reading the composite's data source and shared filters. With the composite selected,
  click a part to edit it with its full inspector; drag to move, handles to resize. Parts export as native
  PowerPoint elements (`Widget.parts`, `Edit::AddPart` in `src/main.rs`, `vm::part_vms`, `ui/canvas.js`).
- **The Quire theme names each built-in widget once** (no "big" beside "hero"); applying a theme moves widgets to that
  theme's name for the same widget (`theme_studio::same_views`).

- **ClickHouse connections.** *Connect a database* now offers **ClickHouse** next to DuckDB, SQLite and PostgreSQL:
  `clickhouse://user:password@host:8123/database` (`clickhouses://` for TLS). Its tables come with their row
  counts, the SQL workspace and the SQL assistant work on it (the assistant is told it writes ClickHouse SQL), and
  data sources on it load with the report's period and parameters. It is read over ClickHouse's HTTP interface
  through the system's curl — the user and password travel as headers on curl's input, never on a command line
  (`src/data.rs` `load_clickhouse`, `src/dbconn.rs`, `ui/dbx.js`, `ui/panels.js`).

### Fixed

- **A card turned on kept its title on the edge.** Views made without a card (e.g. *Лидеры роста*, the ranked list
  *Доходность против ориентиров*) set no padding; with **Card** switched on, the title and its icon sat on the card's
  border. A card now always has at least S (6 pt) inside, as the guide says (`vm::frame_pad`).
- **KPI cards flagged as cut off when they weren't.** The fit check counted a KPI's lines as overflowing as soon as
  they reached the card's bottom padding, though they were still inside the card; Bold Blocks' breakdown cards and
  Сбер's hero figures were flagged as built (layout issues 36 → 26 and 35 → 26). A KPI now counts as cut off only when
  its lines would cross the card's edge, and its figure is sized to the card's width first, as it is drawn (`src/vm.rs`).
- **A long KPI note ran past the card's right edge.** It now takes slightly smaller type (to 85%), then ends with “…”
  (`ui/charts.js`).
- **New data source did nothing at first.** The Data area wired the button only after loading every report's
  data, so a click in the first seconds was lost and the page then opened on the first shared source. It now
  answers at once and opens its page when the lists arrive (`ui/areas.js`).

### Improved

- **Connections are easier to find and change.** **Connect a database** sits at the top of the Data area and is
  the last choice on the *New data source* page; a connection's page shows its type and where it points, and has
  **Edit connection** in view (it was only in the … menu). A designer can now edit a connection that belongs only
  to the open template, and connect one for it (`ui/areas.js`).

- **The user guide's pictures are new**: all 58 are taken again from the app as it is now, with the templates in
  the **Quire** theme: the Design start page with its template list and icon buttons, the inspector, charts with
  their per-series parts, table columns with pills and letter badges, the theme studio, the AI assistants and
  Admin. The guide's table of the Templates area now names *Your templates* and *Start from*, as on screen
  (`ui/help/guide.md`, `ui/help/img/`).
- For developers: **`dev/shots/guide-all.mjs`** takes pictures 01–48 in one run (`node dev/shots/guide-all.mjs <out>
  [prefixes]`), each document opened in Quire; `design-ai.mjs` and `restyle-ai.mjs` open theirs in Quire too. The dev
  bridge keeps its data in **`QUIRE_BRIDGE_DIR`** when that is set (otherwise the temp folder
  `quire_bridge_themes`), so pictures can be taken on clean data (`src/main.rs`).

### Fixed

- `dev/shots/restyle-ai.mjs` failed at its stored answer (it read one picture, the assistant now sends several);
  `admin.mjs` switches Activity to *All events* before opening an AI request.

---

## 27 October 2026

### New
- **Quire, the house theme, as a dashboard** — the theme the guides, screenshots and decks are made in. It was the
  app's plain default (no design views); it is now a full look with every design view: white paper; **cards on a soft
  cool grey** with a hairline; **IBM Plex Sans** throughout — light page headings, **bold figures**, even figures in
  tables; **blue leads** (the first series, share bars, the donut, accents) through teal, lavender, amber, slate and
  sky, and **red is only for what is negative**; marks are **pastel** (columns a light tint of their colour, their
  figures at full strength); **KPI cards that colour a result by its sign sit on a soft fill of it**, green or red,
  with its arrow — the page's accents come from its figures. It keeps every widget the theme studio offers; the UHNWI
  template fits in it as in Modern UHNWI (8 layout issues, the same ones). The old plain theme stays as the app's
  starting point, out of the theme list. (`QUIRE` and the new `Shape.kpi_tone` in `src/uhnwi_theme.rs`; `builtin` in
  `src/theme_studio.rs`)
- **Letter badges on a pale chip are tonal**: their letters a deep shade of the chip's own colour (deep teal on pale
  teal, deep red on pale red) instead of black — readable on any chip; on a dark chip, white. (`letter` in
  `ui/glyphs.js`)
- **Removed themes: Heritage, Терракота and Дашборд.** Templates already in one keep their copy of it.
- **A waterfall's start bar has its own colour** (*Design → Colours → Start*, shown when *Starts from a level* is
  on). It was always the theme's muted colour; that stays the default. (`colorStart` in `ui/charts.js`,
  `ui/vizschema.js`)
- **Transparent in every colour picker**, the first swatch of **Custom colours**, which replaces *Fixed colours*
  (twelve swatches written into the code, tied to nothing). Custom colours are the colours this template already
  uses, most used first, so one picked once is a click away. (`colorField` and `usedColours` in `ui/panels.js`)
- **Select several widgets with ⇧-click** and change them together. The inspector shows the settings they all have;
  one they have set differently reads **Mixed** (an empty field, a striped swatch or switch, a *Mixed* tag by its
  name) until a value is set for all. A change goes to all of them as one undo step; dragging, the arrow keys, ⌘D and
  ⌫ act on all of them. Each keeps its own data, title and words, and a design view or chart type goes only to
  widgets of the same kind. On one page, in templates (not in reports or the theme studio). (`selection`,
  `toggleSelected` and `forSelection` in `ui/state.js`; `multiInspector` and the common-settings filter in
  `tabbed`, `ui/panels.js`; ⇧-click in `ui/canvas.js`; check `dev/shots/multi-select.mjs`). *Find a setting* searches
  the shared settings (at first it found none: the search ran inside each widget's own inspector too). While
  dragging, every selected widget shows where it will go. *Layout*'s Column, Row, Width and Height set each selected
  widget to the number typed (lining them up), and a *Mixed* field takes any value typed — typing the first widget's
  own value used to do nothing.

### Improved
- **Themes use their colours by name.** Wherever a theme repeats one of its named colours — Emerald's accent line,
  share bar and green table headers are its Accent; white header text is its Page — it now points to that name, so
  changing Accent (or Text, Page, a series colour…) carries everything that was in it along. This holds for the
  built-in themes, a theme made by AI restyle, and themes and templates saved earlier (linked when opened; nothing
  draws differently). Table header bands, their text and striped rows are now theme colours too (*Table header*,
  *Table header text*, *Striped rows*). (`Theme::link_colours` and `Theme::token` in `src/model.rs`, called from
  `theme_from` in `src/uhnwi_theme.rs`, the restyle and opening a document in `src/main.rs`)
- **The templates list: one button where an action is due, icons for the rest, and Delete.** A draft shows
  *Publish…* and a changed template *Publish changes…*; then, as icons with a tooltip each: open, audience, preview,
  duplicate and **delete**. The template's name opens it too. Delete always asks first, saying what goes — the
  template file and, if it was published, its report, with how many people lose it — and nothing is erased: both go to
  a *.deleted* folder in the templates library. (`delete_template` in `src/main.rs`, test
  `deleting_a_template_puts_it_and_its_report_away`; the list in `ui/app.js`)
- **Templates sorted by status** on the Design page's Templates list: published first, then unpublished, drafts
  last. (`ui/app.js`)
- **No dividers between the halves of a page.** The template's pages split in two had a vertical divider in a column
  of its own, so the halves were 11 and 12 columns wide. The dividers are gone and the halves are 12 and 12, told apart
  by the gap between them (the forecast page's three blocks are 8, 8 and 8). Templates and reports saved earlier lose
  them when opened: the side with fewer columns takes the divider's column, nothing else moves, and the pages' layout
  issues are the same as before. The grid stays at 24 columns, so rows of four and six cards stay even. A vertical
  divider you add yourself stays, and can be drawn *In the gap* (Design → *Place*), where it takes no room.
  (`Doc::remove_split_dividers` and `in_gap` in `src/model.rs`; in-gap dividers skipped by the fit engine in
  `rect_items`, `src/main.rs`, and `src/vm.rs`; drawn in `ui/render.js`; test `halves_meet_without_a_divider`;
  check `dev/shots/dividers.mjs`)
- **Padding is S for every widget unless set, and None is there for any widget.** The default was 10 pt for a widget
  with a card or title and 0 without, which left content at different distances from the edges (the right column of
  *Портфель — кратко* sat on its edge while the left was inset). Now every widget starts at S (6 pt) — a bare one at
  its sides only, so tables keep their rows, and none on the cover and other plain pages; Bold Blocks keeps its
  roomier colour blocks. *None* shows as an icon, and the row reads *Padding* (no "(pt)"). Fewer figures are cut off:
  the template's layout issues went from 27 to 8, none new. (`frame_pad` and `PAD_S` in `src/vm.rs`; the Padding row
  in `ui/panels.js`)
- **A part per series in Colours, Marks, Highlights and Data labels.** On a chart with two or more series, each of
  these sections has what the series share first (*All series*: corners, best/worst words, the callout's colours;
  *Text · all series* for labels), then a part under each series' name — as *Axes & grid* has one per axis:
  - **Colours**: *Colour by* its own colour or up / down, and those colours;
  - **Marks**: for columns, lighten fills, outline (width, colour) and solid or fade to axis; for a line, width,
    style, straight or smooth, markers (size, fill) and the area under it;
  - **Highlights**: best & worst with background bands (columns side by side), and a callout on the last or the
    largest value;
  - **Data labels**: show, position, colour, bold, frame, units, decimals and suffix.
  The **Series** cards keep what makes a series: its name, columns or line, and axis (its colour shows as a dot).
  Unset, a series draws as the chart did — best & worst and callouts, which used to apply only to a lone series,
  stay off until a series asks for them — so existing charts don't change. Stacked columns are labelled with their
  totals and keep those settings common. (`seriesParts`, `commonRows` and `PER_SERIES` in `ui/panels.js`; `seriesS`
  and `SERIES_KEYS` in `ui/charts.js`; check `dev/shots/series-parts.mjs`)
- **Title align** (Design → *Card & title*): a widget's title, and the subtitle under it, on the left, centre or
  right. Right-aligned titles used a fixed 10 pt inset; they now follow the widget's padding. (`ui/render.js`)
- **Icon chip settings show what is drawn.** *Chip background* showed "none" while a tint was drawn, and *Chip outline*
  read off while a quiet outline was drawn (switching it on added a second, bolder one). On charts the background now
  shows its automatic colour (transparent on tiles) and the outline is on, in a dimmed icon colour; switching it off
  removes it. Colour fields can show an automatic colour (`auto` in `colorField`, `chipAuto` in `ui/panels.js`).
- **Tiles' icons have no chip background by default**, only the icon and a faint outline in its colour; the
  icons' background colour setting still gives them one. (`chip` in `ui/charts.js`; a see-through chip in `glyph`, `ui/glyphs.js`)

- **Data labels "Next to axis"** (was *Below axis*): the labels run in one row along the axis, each on the side away
  from its column — under the axis for a rise, over it for a fall (a fall's label used to go under the column's far
  end, away from the row). (`columns` in `ui/charts.js`)

- **Charts of monthly values and their running total use two axes** in every built-in template (Modern UHNWI's
  overview and account pages, the client template, the theme studio's sample): the total on the right axis,
  labelled at its last value only; each month's column labelled outside it. Saved templates and reports get the
  same when opened, where the chart's series aren't set by hand. (`Doc::split_series_axes` in `src/model.rs`; test
  `monthly_and_running_totals_use_two_axes`)
- **The legend says which axis each series reads against.** With a right axis every series is tagged *(LHS)* or
  *(RHS)* — in Russian *(лев.)* / *(прав.)* — and the legend shows even where the design view hides it (a widget that
  hides it itself keeps it hidden). A lone column series coloured by sign is named, with its axis, instead of
  *rise / fall*. The right axis's numbers are the colour of the left axis's by default (they took the series'
  colour); *Right axis → Label colour* still sets one. (`columns` in `ui/charts.js`)
- **Labels no longer overlap on a chart of columns and a line.** A column's label steps clear of the line's points
  (over the point; where there's no room over it, the label would sit on its own column and is left out); a line's label takes the other side of its point where a
  column's label is in the way, and gives way where both sides are taken. (`columns` in `ui/charts.js`)

### Fixed
- **Opening a template kept only as its published report left the templates list on screen**: it opened in the
  sidebar's *Open* list but the Design home stayed in front. Its name, the open icon and *Publish changes…* now show
  the template. (`ui/app.js`)
- **A letter badge without a chip colour of its own swapped its colours**: it was drawn on the mark's colour with white
  letters, while its settings showed the chip's. A letter badge now always sits on its chip's colour with its letters
  in the mark's (white or ink where that wouldn't read), as the settings show; only a see-through chip keeps the
  mark's colour as the badge. (`glyph` in `ui/glyphs.js`)
- **"By name" was hard to find**: the switch that finds flags and logos from a value is now called **Flags & logos —
  Find them by name**, with its explanation right under it (it sat below *Size*, apart from the switch). (`ui/panels.js`)
- **A value with both a marker and a pill drew them on top of each other** (in a centred or right-aligned column):
  the badge now comes first and the pill after it, the two centred together. A **+** or **−** badge is drawn, not
  set as text, so it sits in the middle of its chip. (`ui/table.js`, `letter` in `ui/glyphs.js`)
- **A table column of words (a direction, a type) had no Pills or Icons settings.** A value shown as text was taken
  for a number, so its settings had *Number format* instead — the deals' *Тип* pills and the *S / B*, *IN / OUT*
  badges couldn't be changed. They now open as text columns, and each value in *Icons* has its own chip colour
  beside its icon (the red *S*, the green *B*). (`columnInspector` in `ui/panels.js`)
- **A legend shown because of a right axis read "Hidden"** in the inspector. It reads *Auto* (as drawn) with a note;
  *Hidden* hides it. (`vizSections` in `ui/panels.js`)
- **Pointing at a part of a chart on a repeated page outlined far too much**, down to the same chart on the next
  account's page: the outline gathered the part from every copy of the widget. It now covers the copy under the
  pointer. (`onHover` in `ui/canvas.js`)
- **Find a setting didn't search the Text tab.** Text roles (*KPI values*, *Widget subtitles*, *Title*…) are now
  found by their name and what they are set to ("bold", "Inter", "13 pt"); a matching role opens. With several
  widgets selected, a search that finds nothing says only their shared settings were searched. (`rowText` and
  `searchResults` in `ui/panels.js`)
- **Card views showed no card in Data Atlas and Bold Blocks.** Their cards were the page colour with no frame, so a
  number set to *Card* (and the figure rows, cohort bars and stat tiles on a card) looked as if bare. A theme whose
  cards wouldn't show now puts them on its second card colour with a hairline in its card outline; Bold Blocks'
  colour blocks stay unframed. Every built-in theme is checked so that each card view draws a visible card.
  Templates already in one of these themes get it when the theme is applied again. (`card_shows` and `theme_from`
  in `src/uhnwi_theme.rs`, `on_block` in `src/vm.rs`; test `every_card_view_shows_a_card`)
- **A colour field set to a theme colour stretched across the inspector** and showed a stray circle (on the
  waterfall's *Down / negative*, for one). It stays a swatch; the name is in its tooltip. (`ui/styles.css`)

## 26 October 2026

### New
- **Data Atlas, a built-in theme** after a data-journalism infographic (a GDP treemap): Roboto Condensed headlines
  and labels, flat muted pastels, solid treemap tiles with white gutters, square corners, no grid or card frames, red
  for what must stand out. Roboto Condensed (400–800, with Cyrillic, SIL OFL) is now bundled, so it exports to PDF
  as drawn. (`ATLAS` in `src/uhnwi_theme.rs`, `ui/fonts/RobotoCondensed-*.ttf`)
- **Treemap labels "together, top left"** (Design → Data labels → *Labels*): the name, its value and share stacked in
  the tile's corner; thin tiles put the name and value on one line. The default stays *Name top, value below*.
  (`treemap` in `ui/charts.js`)
- **Themes set their treemaps:** a theme's Shape now has the tiles' colour strength, gap and label layout, as it has
  for bars and donuts; the restyle can set them too. (`Shape.tile_fill`, `tile_gap`, `tile_labels`)
- **Theme colours by name in every colour picker.** The popover lists the theme's colours by the names its
  *Colours* section gives them (*Accent*, *Text*, *Positive*, *Cards*, *Lines & grid*…), then the series colours and,
  under *More theme colours*, the rest. A colour picked by name is kept as that name and follows the theme: change
  Accent and everything set to *Accent* changes with it; the field's tooltip names it. Plain swatches are now under *Fixed
  colours*. Works for widget, series, text and page-band colours. (`NAMED_COLOURS`, `tokenColor`, `themeColours` in
  `ui/util.js`; `colorField` in `ui/panels.js`; band colours resolved by `Theme::color` in `src/vm.rs`)

### Fixed
- **A published template's button read "Publishing…"**, as if publishing were under way. It now reads *Audience…*
  (see or change who gets the report); *Publish…* and *Publish changes…* are unchanged. (`ui/app.js`)

## 24 October 2026

### New
- **The Widgets tab is a table of widgets grouped by kind**, each widget once (not per theme): a preview, name,
  description, source (built-in or yours, with version), **the themes that include it** and **the templates that use
  it**. Click a row to work on it.
- **The widget studio has its own pane**: About (name and description), Preview size (small, medium, large), Used by
  (themes and templates) and the save actions, in place of the template's palette, data and pages.
- **The widget studio.** Click any widget on the Design page's Widgets tab to open it, drawn large on sample data
  with the full inspector. *Save widget* updates one of yours in the library (a new version); a built-in one is saved
  with *Save as my widget…* as your own. The sample data never goes into the widget. (`widget_open` in
  `src/main.rs`; the canvas zooms onto the widget)
- **A Widgets tab on the Design page.** Every widget drawn with sample data: your own widget types, then each
  built-in kind in all its design views, in a theme you pick (*Drawn in*). (`widget_gallery` in `src/main.rs`, drawn
  apart from the open documents)
- **Your own widget types.** Any configured widget can be saved as a widget type (⋯ → *Save as widget type…*, with a
  name, description and icon) in a library every template uses. Types appear under *Your widget types* in the
  Widgets panel. Widgets of a type follow it: *Update the type* passes one widget's changes to all of them, and
  *Detach* lets a widget go its own way. Each template keeps a copy, picks up newer versions when opened, and bankers
  get changes when it is published again. (`WidgetType` and `Widget.type` in `src/model.rs`, merged in
  `model::effective`; the `widget_types*` commands in `src/main.rs`; the Widgets panel and the save dialog in
  `ui/panels.js`)
- **Each value axis has its own controls.** *Axes & grid* has headings: *Left axis* and *Right axis* (or *Value
  axis*), *Category axis* and *Grid*. Each value axis can be shown or hidden and has its own label colour, number of
  ticks or a fixed **step**, signed ticks, minimum and maximum; the right axis also has units.
- **Several series, each under control, and a right axis.** On column, line and area charts, *Design → Series* has
  a card per series: its legend name, colour, columns or line, axis, line style, width, markers and labels. Any series
  can go on a **right axis** with its own scale (minimum, maximum and units under *Axes & grid*); its labels take the
  series' colour and the legend marks it. This replaces the old "Values as lines" rows. (`seriesOpts`, `seriesKey`
  and the second scale in `ui/charts.js`; `seriesSection` in `ui/panels.js`)
- **Click a part of a visual to get its settings.** With a widget selected, the part under the pointer is outlined
  and named: legend, axis & grid, data labels, a series or category, a donut's centre, a KPI's value, note, change or
  trend, the title, the card. A click opens the Design tab at that part, and a series opens *Colours* at its own
  colour. A table cell opens its column's settings, and a ranked list's row opens *List*. (Charts tag their pieces with
  `data-part` in `ui/charts.js`; `focusPart` in `ui/panels.js`; hover and click in `ui/canvas.js`)
- **Text styles for each part of a chart.** Axis labels, data labels and legends each have their own text style
  (font, size, weight, colour, capitals, spacing): a **Text** row in *Axes & grid*, *Data labels* and *Legend*, also in
  the Text tab and the theme's Typography. Each starts as the chart text, so existing templates look the same until
  one is set. (New text roles `axis`, `data_label`, `legend` in `src/vm.rs`; `pf` in `ui/charts.js`)
- **Spacing and lines:** gridline colour and width, zero-line colour, the gap between tiles and the padding inside
  them, and the space under a widget's title.

- **The Design area has two views: Templates and Themes.** *Templates* is the main view and opens by default; *Themes* is
  a gallery of your library and the built-in themes, each with its colours, typefaces and the published reports
  that use it.
- **A theme used by many templates.** *Used by* shows the count; clicking it lists every template using the theme
  (published first, searchable) to open one, or **Show in Templates** for the Templates tab filtered to that theme.
- **Themes as a table.** Each row shows the theme's colours (paper, text, accent, series, gains and losses), its
  typefaces written in themselves, the card style, corners and chart look, its design views, and the templates that
  use it (which are published, a click opens one). Filter by *All*, *Your library*, *Built-in* or *In use*. Open,
  Duplicate…, Rename and Delete are on each row. (`theme_list` gained card, corners, chart look, gains / losses and
  the save date)
- **Templates library and one list of templates.** Templates are saved in a library (a first *Save* goes there, and
  *Save as…* still picks a place). The Design page lists every template on one line with its published report,
  status (*Draft*, *Published*, *Unpublished*, *changed since*), version, theme, audience and use, with **Open**,
  **Publish…**, **Preview** and **Duplicate…**. *Needs attention* now flags templates changed since they were
  published, and has a *Publish…* button on each item. (`templates_dir`, `save_template_here` and the `design_home`
  templates list in `src/main.rs`)
- **Publishing is one clear step.** A **Publish** button in the template's toolbar (*Publish changes* once it's
  published) opens a dialog. It explains template → publish → report users, then asks who gets it; a new report
  starts as *Everyone with Reports*. Publishing saves the template, so the link to its report is kept; there's no
  "save to keep the link" step any more.
- **Duplicate a template into another theme.** *Duplicate…* on a published report, the example or a recent template
  asks for a name and a theme (picked from swatches). The copy is saved in the templates library, opens, and is listed
  as a draft until you publish it. The original stays as it is. (`duplicate_template` in `src/main.rs`)
- Published reports have a **Version** column and a **Theme** chip that opens the theme. Theme cards list the
  templates that use the theme, both published reports and recent template files.
- **A start page for designers (Design area).** It shows:
  - **Needs attention:** data changes waiting for a published report, drafts, reports published to nobody, and
    reports unused for 30 days;
  - **Published reports:** status, version, audience, and use in the last 30 days (opened, exported, clients, last
    used), with **Edit** (opens the report's template) and **Preview**;
  - your **recent templates**;
  - the examples;
  - **themes as colour swatches**;
  - links to the guide.

  (`design_home` and `edit_published` commands in `src/main.rs`, `design_usage` in `src/audit.rs`; `designHome` in
  `ui/app.js`)
- **Double-click a setting's name to reset it.** The reset buttons are gone. This works in the designer's Design tab
  (charts, colours of each series or category, Card & title, the widget's name), in every text style (each property,
  or the whole style from its name), and in report users' settings (Fit, Numbers, Chart, Table, title). Report users
  had no way to reset these settings before. (`resettable` in `ui/util.js`, `mark` in `ui/panels.js`)

### Improved
- **The same four tabs for every widget: Content · Design · Text · Layout.** *Content* holds a *Title* section and the
  *Data* section for charts, tables, KPIs and tiles. A chart's, table's,
  KPI's or tiles' title and subtitle are at the top of **Data**, above the data settings. Text, headings, notes and
  contents lists keep their words or entries in **Data** too.
- **The Design tab has one order for every visual.** Settings are grouped by the part of the visual they change, with
  the same names and order for every chart, KPI, tile and table: *Type & view, Layout, Series, Value, Colours, Marks,
  Numbers, Data labels, Centre, Axes & grid, List, Legend, Note & breakdown, Change, Trend, Highlights, Icons, Card &
  title*. Only the parts that apply show, and parts with changes open by themselves. (`ui/vizschema.js`
  `SECTION_ORDER`, `designSections` in `ui/panels.js`)
- **Duplicate settings merged:**
  - all mark colours, including each series' or category's colour, are under *Colours*;
  - best & worst is under *Highlights* for every kind;
  - a table's totals row (on/off and its look) is under *Totals & groups*;
  - *Values as lines* moved to *Series*, so there are no longer two sections called "Lines";
  - a KPI's value size under *Value* is the same setting as Text → *KPI values*.
- **Settings that had no control now have one:**
  - a KPI's value on the title's line;
  - a second value beside it, and its decimals;
  - a value in a chip;
  - the note's decimals and the colour of its bold words;
  - labels inside a 100% bar's parts;
  - right alignment for KPIs.
- **Ranked lists:** the values at the right can be hidden (*List → Values at the right*). The misnamed *Track fills
  the row* is now **Bars fill the row**: it only makes the bars take the free width. It no longer also makes the names
  bold and the bars thinner; those are now their own settings, *Names in bold* and *Bar thickness*. Lists that had it
  on look the same as before. (`ui/rowlist.js`)
- A check (`node dev/checks/inspector.mjs`) keeps it this way: every option of every visual lands in one of the
  agreed parts, none appears twice, and every setting the drawing code reads has a control.

### Fixed
- **The Widgets tab listed some built-in widgets twice, in two languages** (Waterfall and Водопад, Treemap and
  Плитки · крупнейший слева…). The Quire theme gave 12 shared widgets its own ids; they are now one row each, listed
  with all their themes, and the Quire theme's remaining widgets are named in the preview theme's language.
  (`theme_studio::same_widget`, `widget_gallery` in `src/main.rs`)
- **Opening a widget from the Widgets tab** added it to the open documents but left the Design page in front, and it
  could show a different widget: the studio used a default sample instead of the widget's own (monthly results,
  a split by class…), and widgets of only some themes opened without their design view. The studio now comes to the
  front and shows exactly the widget its row previews (checked for all 56 built-in widgets; contents lists show
  sample sections).
- **"Auto" sizes now say what they came to.** A KPI figure, a donut's centre, a 100% bar's height, tile columns and
  an axis's ends (left and right) show "auto · 18" with the value they were drawn at, and the arrows start from it.
  An axis's ends step by an amount that fits their numbers (0.15 → 0.16, 400 000 000 → 410 000 000).
- **The arrows on an empty number box started from 1.** A setting that isn't set shows the value in force (e.g. 18)
  in grey; ↑ / ↓ and the spinner now start from that value (18 → 19), and the box keeps its focus between steps. Text
  sizes step by whole points (typing a half point still works).
- **Wrap on a table column did nothing and hid the icons.** Turning it on now keeps the column's width and breaks
  long text onto more lines instead of cutting it with "…". Icons stay beside the lines; the column keeps its width
  when the table is tight, and the text never runs into the next column. Wrap isn't offered on a grouped table's
  first column, where it can't work.
- **A KPI too short for its figure and the rows under it was cut off without a warning.** It now gets the same
  "cut off" warning as text, with **Grow to fit the figures**, and the canvas offers to make it taller. (KPI height
  in `src/vm.rs`; warning in `src/layout.rs`)
- **Padding "None" (and "S") on a card did nothing.** A card always keeps at least 6 pt inside it, or the content
  would sit on its outline. On a card the control now offers S (6 pt), M and L, and shows what is actually drawn.
  This applies to the designer's *Card & title* and report users' *Spacing*.

## 23 October 2026

### New
- **Proposals in reports.** A banker adds a proposal page from their own notes, an uploaded Word, PowerPoint or text
  file, or an AI draft (*Suggest from the report*). The page goes after the facts and before the legal text. It shows
  the main idea in large type, a highlighted effect card, buy/sell actions in colour, and *Why now*, *Risks* and
  *Next steps* cards.
  - A long proposal takes a second page instead of being cut off; a long actions table continues onto further pages.
  - A report can hold several proposals.
  - The brief & pitch, the check before sending, and the AI's *Proposal ideas* all take the proposals into account.
- **Proposal register.** Every proposal is remembered per client: its versions and its status (*Draft*, *Presented*,
  *Accepted*, *Partly accepted*, *Declined*, *Withdrawn*). The client list shows a client's proposals and reminds
  the banker to follow up after 14 days. Exporting the report to PDF marks its proposals *Presented*. Admins see
  every proposal in **Admin → Proposals**, with a support summary. (`src/proposal.rs`, `src/proposals.rs`,
  `ui/proposal.js`)
- **PDF as a restyle reference.** *Designer → Look* accepts a PDF as well as pictures and GIFs. Quire also reads the
  PDF's own typefaces (names and sizes) and exact colours, and passes them to the AI as facts.
  (`ui/pdfpages.js`, bundled pdf.js)
- **Bold Blocks theme.** Heavy black headlines, figures on solid yellow, orange, pink and black blocks, a thick yellow
  bar beside the main figure, black table headers and a yellow closing page. It follows the look of
  BlackRock-style booklets.
- **Theme settings for poster-style designs:**
  - headline letter spacing, and headlines up to 60 pt;
  - up to four block colours that KPI cards take in turn;
  - their own paper colour for the cover and the closing page.
  - Montserrat Black and Inter ExtraBold/Black are now bundled.
- **Backlog folder** with plans for later: web-server mode for 500 users, a report API, acting on *Needs attention*,
  and proposal campaigns.

### Improved
- **Faster client list for thousands of clients:**
  - the figures are cached;
  - the activity index is read incrementally;
  - the list shows 100 rows at a time and loads more as you scroll;
  - search waits until you stop typing.
- **The AI is told the restyle rules:** heading weight comes first, how to use block colours, and that the PDF's facts
  win over guesses from the pictures. The list of fonts it sees now shows the weights each one has.
- **Typefaces belong to text roles.** Changing a role's font or the base font in *Typography* now changes every
  design view. A restyle's typefaces are stored on the role, where you can see and change them.
- **Show icons** switch for charts with category icons (tiles, donut, treemap, 100% bar).

### Fixed
- The header and footer kept a restyled theme's colours after switching to another theme. They now belong to the
  theme, are kept when you save it in the studio, and go back to the template's own when the theme changes.
- Icons disappeared on chips of their own colour, pill text vanished on dark pills, and figures vanished on dark
  tiles. Text and icons now always stay readable on their background.
- A restyle could make the "gain" tint pink. Gain and loss tints now keep the hue of the green and red they sit
  behind; broken themes are corrected when they load, and the restyle is checked for it.
- The theme couldn't be changed after adding a proposal.
- A widget couldn't be moved down a cell after being made smaller. Widgets placed by hand now stay where they are put.

## 22 October 2026

### New
- **Client list for bankers (Reports area):**
  - per client: value, year-to-date return, net flow, a 12-month sparkline, return against similar clients, fees,
    risk profile (as a meter), products and the last report;
  - sorting, a column picker and recently opened clients;
  - **Needs attention**: payments due, policy renewals, birthdays, withdrawals, idle cash, and shares above the risk
    profile. A flag on the client's row; details open inline under it, and ⌘] closes them.
  - *Create report* as a small button in the last column.

  (`src/overview.rs`, `ui/areas.js`)
- **Batch reports.** Reports for many clients and one period in one go, saved as PDFs to a chosen folder, built apart
  from the open documents. (`ui/batch.js`)
- **New template page:** *Движение средств, налоги и комиссии*, covering inflows and outflows, taxes and fees.

### Improved
- *Сделки по инструментам*: the instrument type as a pill column, the account type before the account number, and
  each deal's result at the end of the period.
- Template display: currency tiles no longer overlap; the contribution donut has no number inside; the leaders list
  is tighter; the donut's unit sits right under its number; card title icons are aligned.
- The *Time* and *No comparison* options are hidden where they do nothing.
- Report users can switch *Fit to page* on and off.

## 21 October 2026

### New
- **Report assistant (AI):**
  - *Brief & pitch*, with the main ideas and figures highlighted;
  - a ready-to-send **e-mail to the client** (edit, copy, open in Mail);
  - *Check before sending*;
  - a note for a page;
  - saving as PDF.

  Client, account and instrument names and numbers are always masked before anything is sent. (`src/ai_report.rs`,
  `ui/aireport.js`, `ui/brief.js`)
- **Admin assistant:**
  - *Ask the log* (the last 30 days);
  - *Change access*: proposes the change and shows it before and after; you apply it;
  - *Explain* on failed events;
  - an access review and AI cost.
- **Admin areas:** Users & roles, Groups, Access overview, AI (bank policy, keys, level per person, 30-day use) and
  Activity (an audit log of everything, exportable to CSV).
- **AI data levels:** *Structure only* or *Figures*, set for the bank, per group and per person.
- **Products table** in the demo database. Admin shows product names from it.

## 20 October 2026

### New
- **SQL assistant (AI) for the Data role:** writes, fixes and explains SQL, turns a query into a report source, and
  answers questions about the schema. Only the structure is sent; the app runs each attempt itself.
- **Design assistant (AI):** builds a new page or changes the current one from a request, checked for data errors and
  fit, in one undo step. (`src/ai_design.rs`)
- **Restyle from a picture (*Look*):**
  - one answer gives a whole look, drawing changes per design view, and the page header and footer;
  - GIFs are read frame by frame;
  - the AI gets back any text cut off and tries again.

  (`src/ai_restyle.rs`)

## 17 October 2026

### New
- **AI providers:** Claude, ChatGPT, DeepSeek, Qwen and GigaChat. Keys are kept in the macOS Keychain, personal or
  bank-wide. An admin sets the policy (allowed providers, off switch), and every request is logged.

## 15 October 2026

### New
- A **simpler report view** for bankers: a theme picker, replace or delete widgets, and a basic look per widget.
  Designers keep the full inspector.
- *Получено* (payments received) column in the holdings tables.

### Improved
- Totals reconcile across pages: taxes, fees and currency revaluation have their own row, so products, positions and
  the headline agree.
- Near-duplicate design views were merged; the differences are now options on the widget.

## 14 October 2026

### New
- **Sign-in and the left rail:** areas Reports, Templates (Design), Data and Admin, with open documents and SQL
  workspaces kept while you move between areas.
- **Data owns connections and data sources:** shared sources live in the Data area and are reused by templates.
- **Access = clients × products:** coverage by banker or a list of clients, and products by account type. Reports
  are published to an audience.
- **Themes change shapes, not just colours:** corners, grid, fills, labels and cards. The Сбер theme follows its annual
  report.

### Improved
- The build uses far less disk space (slimmer dev profile).

## 13 October 2026

### New
- **Demo bank database** (`data/demo.duckdb`): clients, accounts, instruments, balances, operations, prices,
  benchmarks and a cash-flow schedule. The UHNWI template reads it with SQL on the bank's own tables.
- **Repeating pages:** a page set per data row, e.g. one per brokerage, IIS, trust or insurance account. Accounts
  closed in the period are marked as closed.

## 9–11 October 2026

### New
- **Quire started:**
  - template designer and report modes in one app (Rust + Tauri, vanilla JS);
  - data from CSV/Excel, SQL databases and Python;
  - a grid layout with automatic fit;
  - export to PDF and PowerPoint as vectors.
- **Theme studio:** every view of every object with sample data, saved as `.qtheme` and loaded by templates.
- **UHNWI portfolio report** sample, reproducing the *Modern UHNWI Report Design* reference.
- Reference looks are chart types with options (e.g. the peer "dot scale" is a bar chart → *Range*), not separate
  widgets.
