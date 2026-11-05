# Rebuilding Quire: a guide for a coding model

This file is written for **an AI coding model (or a developer) working on Linux, without the original
author's assistant**. It describes what Quire is, how it is built, and the phases to build it again, with
what "done" means for each phase. Work through the phases **in order**; each one ends with checks that must
pass before you go on.

> **To the model reading this:** read sections 1–5 completely before writing any code. Then do one phase
> at a time. At the end of each phase, run its checks, write a dated entry in `RELEASES.md`, and stop
> to show the user what works. Never mark a phase done while a check fails. When a decision is not
> covered here, ask the user rather than guess.

---

## Contents

1. [Two ways to start](#1-two-ways-to-start)
2. [What Quire is](#2-what-quire-is)
3. [Rules the user insists on](#3-rules-the-user-insists-on)
4. [Architecture](#4-architecture)
5. [Data model](#5-data-model)
6. [Phases](#6-phases)
7. [Testing](#7-testing)
8. [Linux notes](#8-linux-notes)
9. [Pitfalls already learned](#9-pitfalls-already-learned)
10. [Working conventions](#10-working-conventions)
11. [Prompt to start each phase](#11-prompt-to-start-each-phase)

---

## 1. Two ways to start

### A. You can copy the original folder (recommended)

If the `Quire/` folder can be copied to the Linux laptop, **don't rebuild from scratch**: build it there
and fix the few macOS-only parts (section 8). Copy everything except `target/` (the build cache, many GB).

```sh
# Ubuntu / Debian
sudo apt install build-essential pkg-config curl python3 python3-pandas nodejs chromium \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libsoup-3.0-dev
curl https://sh.rustup.rs -sSf | sh          # Rust stable
cd Quire
python3 data/make_demo_db.py                 # only if data/demo.duckdb was not copied
cargo test                                   # ~150 engine tests; first build ~5 min (DuckDB compiles)
cargo run --no-default-features --bin quire-server   # web version → http://127.0.0.1:8766/
cargo run                                    # desktop app (Tauri)
```

Sign in with the demo accounts `admin`, `data`, `design`, `user`; the password for each is `quire`.
Then do section 8 (Linux fixes) and carry on with new work using sections 3, 7, 9 and 10.

The documents in the folder are the real specification. Read them in this order:
- `README.md` (features and file layout);
- `ui/help/guide.md` (the user guide, 17 chapters, each with a *Test* list);
- `TESTING.md`, `RELEASES.md` and `backlog/`;
- `database.md` and `data/README.md` (the demo bank).

### B. You must rebuild from nothing

Follow section 6 phase by phase. The phases are ordered so that each one gives something you can run and see.
The original is about **56,000 lines** (Rust ~30k, JS ~25k), built over three weeks. Expect the same order of
size. Don't try to do it in one go.

---

## 2. What Quire is

**Quire is a report designer and generator for banks** (and any company that produces designed reports:
private-banking portfolio reports, sales packs, annual reports). The bank's own staff:

- **design report templates**: pages on a grid, with KPI cards, charts, tables, text, contents pages, and headers and footers, all in a theme;
- connect them to the bank's own data;
- **produce a finished report for any client and period** as vector PDF or native PowerPoint. This can be one report at a time or a batch for every client.

The same app runs as a **desktop app** (Rust + Tauri) and as a **web server** opened in a browser. Both use
one core and one UI.

### The four roles

They are shown as areas in a left rail. Admin implies every role.

| Role | Area | What they do |
|---|---|---|
| **Data** | Data | Connects databases (DuckDB, SQLite, PostgreSQL, ClickHouse) and files (CSV, Excel, Parquet). Writes SQL in a SQL workspace with an editor and preview, and builds **shared data sources** and **report parameters** that templates reuse. Python data sources. |
| **Designer** | Design | Builds templates: pages, widgets, themes, design views, contents, header/footer and repeating pages. Decides **what users may change** (policy). Publishes templates as reports to an audience. |
| **User** | Reports | Opens a published report and picks a client and a period, and the report fills in. Light changes only, within the policy: theme, replace or delete a widget, comments. Exports PDF/PPTX. Runs **batch reports** (every client, one run). Has a client list with "needs attention", and proposals. |
| **Administrator** | Admin | Users and roles, groups, access (**clients × products**), AI policy and keys, activity log (audit), and the server's switches. |

Every role has an **AI assistant** (section 6, phase 15) under the bank's data policy.

### Files

- `.rtpl` is a template.
- `.rpt` is a report: a self-contained snapshot of its template plus the user's period, layout and comments.

Both are pretty-printed JSON. Relative data paths resolve against the document's folder.

---

## 3. Rules the user insists on

These came from the user's corrections. Breaking them is the fastest way to lose their trust.

1. **Every visible control must change the output.** A setting that does nothing is the user's top
   complaint. Only show settings that apply in the current context. Show values "as drawn": a design view's
   value merged under the widget's own. Verify each new control by toggling it and diffing the rendered SVG
   (see the probe in section 7).
2. **Common controls exist everywhere the user expects them.** For example, if "Lighten fills" or "Outline" is
   on bars, it is also on donuts, treemaps and tiles. When a style control goes onto one chart kind, add it to
   every kind with similar marks.
3. **Visuals are chart types plus options, never a separate widget kind.** A "dot scale" from a
   reference design became bar chart → type *Range*. A row list (name · bar · value) is a bar chart's *List*
   layout, drawn by the table engine.
4. **Design views must differ in structure, not in a switchable option.** Don't add a view that a user
   option can reproduce; add the option instead.
5. **Themes change shapes, not only colours and fonts.** Corners, gridlines (on, off or dashed), fills,
   labels and cards all change with the theme. Each theme carries a *Shape* set.
6. **Reproduce reference designs through generic, configurable features**, never one-off hacks for one report.
7. **Report data comes only from the bank's existing tables.** There are no report-specific tables. If a table is
   missing, ask the user before generating it. Report texts that depend on data use placeholders
   (`{{data:…}}`, `{{sql:…}}`), never typed numbers.
8. **Return rule:** a period's return = `net_twr_ltd` at the period end ÷ `net_twr_ltd` on the day before the period
   starts − 1.
9. **Totals agree across pages.** Products add up to the headline, each position table adds up to its
   account result, and the bridge sums to the end value. Tests assert it.
10. **Export stays vector and native.** That means vector PDF with fonts embedded, and PPTX with real text boxes and tables.
11. **The app must look professionally designed.** Inspector categories are grouped and icon-led, and the theme
    studio has one gallery per object type.
12. **Never fork the UI or the command list per platform.** Platform differences go behind
    `ui/platform.js` (UI) and the `Host` trait (Rust).
13. **AI acts only through normal edits:** a preview and one undo step, within role and template policy.
    Client identities are always masked before anything leaves.
14. **The report user's UI is simple:** a theme picker, delete or replace a widget, basic look per widget. Never put designer controls in the user's view.
15. **Write for users:** short words, no jargon, facts not hype. Release notes are for users.

---

## 4. Architecture

```
            ┌──────────────── ui/ (plain HTML + ES modules, NO build step, no framework) ───────────────┐
            │  state.js  call(cmd,args) ──► platform.js ──► desktop: Tauri invoke('call')                 │
            │  render.js / charts.js / table.js draw every page as SVG (screen = PDF = PPTX source)       │
            └──────────────────────────────────────────────┬─────────────────────────────────────────────┘
                                                           │ JSON {cmd, args}  →  full view state JSON
     ┌───────────────────────────┐                ┌────────▼─────────┐               ┌──────────────────────────┐
     │ src/main.rs (Tauri)       │──── one ──────►│ quire::dispatch  │◄──── one ─────│ src/bin/quire-server.rs  │
     │ native dialogs, eyedropper│   `call` cmd   │ (src/app.rs)     │ POST /api/call│ std TcpListener, cookies, │
     └───────────────────────────┘                │ access checks    │               │ workspace per browser     │
                                                  └────────┬─────────┘               └──────────────────────────┘
                     model.rs · layout.rs · vm.rs · query.rs · data.rs · export.rs · auth.rs · ai*.rs · …
```

### Stack

- **Rust 2021** throughout.
- The core is a library (`src/lib.rs`).
- **Tauri v2** for the desktop window only. It is behind the cargo feature `desktop`, so the server builds with `--no-default-features`.
- **UI: vanilla JS ES modules + one CSS file**, served as static files. There is no npm, bundler or React.
  - `ui/index.html` loads `app.js`.
  - Layout: title bar, left **rail**, stage (toolbar, left panel, canvas, inspector, AI pane), and a home/area page.

### Crates used (keep the same)

| Purpose | Crates |
|---|---|
| Serialisation and dates | `serde`, `serde_json`, `chrono` |
| Excel and CSV | `calamine` (Excel), `csv` |
| Databases | `rusqlite` (bundled), `postgres`, `duckdb` (bundled) |
| PDF | `svg2pdf`, `resvg`, `pdf-writer` |
| PPTX packaging | `zip` |
| Fonts | `ttf-parser` |
| Hashing and secrets | `sha2` (password hashes), `chacha20poly1305` + `getrandom` (sealed secrets) |
| Images | `image` (gif/png/jpeg) |

ClickHouse and every AI provider are reached through the system's **`curl`**, so there is no TLS stack in Rust.

### Core principles

- **Rust owns the state.** Each command returns the **whole view state** the UI needs (document, page
  geometry, wrapped text, query results, fit issues, permissions). The UI keeps only view things (zoom, the
  selection, open panels).
- **One command layer.** At the end of `src/app.rs` there is a `commands! { name(arg: Type, …), … }` macro table.
  Adding a command is one line there. Args are read in snake_case or camelCase. Each command checks the role
  (`gate(...)`) inside the core, not only in the UI.
- **All document changes go through `edit(op)`.** `Edit` is a serde enum tagged `"op"`, snake_case. Every
  edit is checked against the user's permissions, recorded in undo/redo and written to the activity log.
  `Batch` makes several edits one undo step. Variants in the original:
  - `Batch`
  - Widgets: `AddWidget`, `AddFromCatalog`, `UpdateWidget` (a JSON merge patch), `SetRect`, `RemoveWidget`, `ReplaceWidget`, `DuplicateWidget`, `MoveToPage`, `AddPart`
  - Widget types: `AdoptWidgetType`, `DetachWidgetType`, `AddTypedWidget`, `SetWidgetType`
  - Pages: `AddPage`, `RemovePage`, `RenamePage`, `SetPageSection`, `SetPagePlain`, `SetPageRepeat`, `SetPageProps`, `MovePage`, `DuplicatePage`
  - AI and proposals: `AiPage`, `SetProposal`
  - Document settings: `SetMeta`, `SetFormats`, `SetIcons`, `SetVariables`, `SetPageSetup`, `SetTheme`, `SetPolicy`, `SetArrange`, `SetParams`, `SetParamDefs`, `SetConnections`
  - Data and gallery: `UpsertDataset`, `RemoveDataset`, `UpsertCatalog`, `RemoveCatalog`, `SaveToCatalog`
  - Layout: `ApplyOps`, `AutoArrange`
  - Fonts: `AddFont`, `RemoveFont`
  - Design views: `AddVariant`, `UpdateVariant`, `RemoveVariant`, `MoveVariant`
- **`Host` trait** (`src/host.rs`) is everything that differs between desktop and server:
  - `data_dir`;
  - `pick_file`, `save_file`, `pick_folder`;
  - `pick_color`;
  - `reveal`, `devtools`;
  - `signed_in`, `settings_file`, `files_dir`, `may_open`.
- **One renderer.** The UI draws every page as SVG in JS. The same SVG is sent to Rust for the PDF (`svg2pdf`)
  and for PPTX pictures, so screen, PDF and PPTX match.
- **Text is measured in Rust** with the real font files (`fonts.rs`, `text.rs`), so wrapping and fit
  are the same everywhere. Fonts (Inter, IBM Plex Sans, Manrope, Spectral, …) are bundled in `ui/fonts/`.

### The command list

The original has ~80 commands. Build them as their phase needs them:

| Group | Commands |
|---|---|
| Core | `auth(cmd, args)` (sign-in, accounts, groups, publishing, client options; a sub-command router), `edit(op)`, `undo`, `redo`, `get_state`, `refresh` |
| Settings | `get_settings`, `save_settings` |
| Documents and files | `new_doc(kind, client)`, `open_path`, `open_dialog`, `save(save_as)`, `report_from_template`, `doc_activate`, `doc_close` |
| Themes | `list_themes`, `theme_library_op`, `theme_studio_open`, `theme_studio_close`, `apply_theme` |
| Data | `pick_data_file`, `excel_sheets`, `preview_dataset`, `field_values`, `repeat_preview` |
| Database connections | `db_tables`, `db_run`, `db_apply`, `db_params`, `db_demo_path` |
| Python | `python_info`, `python_envs`, `python_test`, `python_policy`, `py_run` |
| Export | `export(format, pages)`, `brief_pdf` |
| Batch | `batch_report`, `batch_save` |
| Proposals | `proposal_set`, `proposal_make`, `proposals_list`, `proposal_status`, `proposal_read`, `proposal_remove` |
| AI | `ai_sql`, `ai_design`, `ai_restyle`, `ai_report`, `ai_admin`, `ai_status`, `ai_set_key`, `ai_set_policy`, `ai_log`, `ai_feedback` |
| Activity | `activity`, `work_sessions`, `activity_ping` |
| Browser files | `upload_file`, `saved_file`, `saved_documents`, `delete_saved` |
| Fonts | `list_fonts`, `font_files` |
| Desktop only | `pick_screen_color`, `reveal`, `open_devtools` |
| Logging | `js_log` (UI errors to the terminal) |

### Source files

Keep the layout below. Other models navigate better with this split than with a few giant files.

| Rust (`src/`) | JS (`ui/`) |
|---|---|
| `lib.rs`, `app.rs` (state, commands, edits, undo) | `index.html`, `styles.css`, `app.js` (title bar, menus, toolbar, home, shortcuts) |
| `host.rs`, `main.rs`, `bin/quire-server.rs` | `platform.js` (desktop vs browser), `state.js` (`S`, `call()`, edits, undo) |
| `model.rs` (document types) | `rail.js`, `areas.js` (Reports / Design / Data / Admin pages) |
| `layout.rs` (grid fit engine) | `canvas.js` (pages, selection, drag/resize, inline text, fit advisor) |
| `vm.rs` (view model: geometry, wrapped text, fit) | `panels.js` (left panel + inspector), `vizschema.js` (Design-tab schema) |
| `fonts.rs`, `text.rs` | `render.js`, `charts.js`, `table.js`, `rowlist.js`, `richtext.js` |
| `query.rs`, `period.rs`, `data.rs`, `dbconn.rs`, `python.rs` | `dbx.js`, `sqled.js`, `pylang.js` (SQL workspace, editor, Python) |
| `export.rs` (PDF, PPTX) | `fontui.js`, `colorpick.js`, `icons.js`, `glyphs.js`, `iconlib.js`, `logolib.js` |
| `theme_studio.rs`, `uhnwi_theme.rs`, `looks.rs` | `studio.js` (theme studio), `advanced.js` (widget studio) |
| `auth.rs`, `audit.rs`, `secrets.rs` | `auth.js`, `admin.js` |
| `uhnwi_sample.rs`, `uhnwi_sql.rs`, `sample.rs`, `client_sample.rs` | `repeat.js`, `batch.js`, `proposal.js` |
| `overview.rs` (client list), `proposal.rs`, `proposals.rs` | `ai.js`, `aichat.js`, `aiassist.js`, `aidesign.js`, `aireport.js`, `aiadmin.js`, `brief.js`, `pdfpages.js` |
| `ai.rs`, `ai_design.rs`, `ai_restyle.rs`, `ai_report.rs`, `ai_admin.rs` | `i18n.js`, `i18n-ru.js`, `help.js`, `help/guide.md`, `ui.js`, `util.js` |

---

## 5. Data model

All types are `serde` with `#[serde(default)]` so old files keep opening. Migrate old shapes on load and never
break a saved file.

```text
Doc { format, kind: Template|Report, meta: Meta, page: PageSetup, theme: Theme,
      params: Params, param_defs: [ParamDef], connections: [Connection], datasets: [Dataset],
      pages: [Page], catalog: [CatalogItem] /*gallery*/, policy: Policy, fonts: [EmbeddedFont],
      arrange: layout::Options, next_id, formats: Formats, icons: [IconFile], variables: [Variable],
      access_src /*session-only product filter*/ }

PageSetup { size:"A4", landscape, margin, cols:12, rows:12, gap, header: Band, footer: Band }
Band  { show, height, fill, line, line_color, tabs, left/center/right: Slot{text,image,font} }
Page  { id, title, widgets:[Widget], section, plain /*cover: no header/footer*/, group, label,
        frame, description, toc_hidden, show_if, repeat: Repeat, repeat_of, row{field→value} }
Repeat { dataset, field, values, label }        // one page set per data row, e.g. per account
Widget { id, kind, x,y,w,h /*grid cells*/, title, subtitle, locked, binding: Option<Binding>,
         style: Json, opts: Json, catalog, variant /*design view id*/, wtype /*widget type*/,
         parts: [Widget] /*composite*/ }
Binding { dataset, dims:[field], measures:[Measure], window, filters:[Filter], sort, limit,
          others /*top N + Other*/, mode: summary|detail, columns, pivot, where_sql }
Measure { id, field, agg: sum|avg|min|max|count|last|first|avg_dates, label, format,
          divide_by, calc: value|prev|change|pct_change|share|rank|running, chart, color }
Dataset { id, name, source: Source, date_field, fields:[Field{name,kind}], snapshot,
          transform /*SQL over `data`*/, calc_fields:[{name, expr}], shared }
Source = Csv{path} | Excel{path,sheet} | Sqlite{path,query} | Postgres{url,query}
       | Clickhouse{url,query} | Duckdb{path,query} | Python{code} | Inline{columns,rows}
       | Query{sql /*SQL over other datasets by name*/} | Db{connection,query}
Theme { palette, accent, ink, muted, grid, page_bg, card_bg, card, card_border, radius,
        good, bad, currency, locale, font_family, fonts{role→FontSpec}, viz /*chart defaults*/,
        styles{kind→[StyleVariant]} /*design views*/, tokens{name→colour}, look, widget_types, tabs }
StyleVariant { id, name, description, sample, opts, style }   // a design view
Policy { add, remove, arrange, restyle, comment, pages, edit_text, period, compare }  // what users may do
Params { grain: month|quarter|year|ytd|custom, date, start, compare: prev|yoy|none, values{name→v} }
ParamDef { name, label, options, default, kind, source{dataset,field,label}, multi, field, datasets }
```

### Widget kinds

- `kpi`;
- the charts: `bar` (types columns / bars / 100% / Range; layouts Chart and List), `line`, `area`, `donut`, `treemap`, `waterfall`, `tiles`, `combo`;
- `table`;
- `text`, `heading`, `comment`, `divider`, `image`, `contents`;
- `composite` (parts in their own grid, each with its own data).

### Placeholders in any text

- `{{period}}`, `{{compare}}`, `{{start}}`, `{{end}}`, `{{today}}`;
- `{{page}}`, `{{pages}}`, `{{section}}`, `{{title}}`;
- any parameter;
- `{{data:Source.field|sum|first|count|list}}`, `{{sql: select …}}`, `{{row.field}}` (repeating pages);
- `$token` colours.

### SQL parameters

SQL gets `:start`, `:end`, `:prev_start`, `:prev_end` and `:param`. A multi-choice parameter becomes a list.

### Stores outside documents

They live in the app data folder (desktop: the OS app-data dir; server: `--data DIR`):

| File or folder | Holds |
|---|---|
| `users.json` | The `auth::Store`: accounts (salted SHA-256), roles, access, groups, published reports, shared datasets and their usage, the AI policy, `python_on_server` |
| `templates/` | Templates |
| `reports/<id>.rtpl` | Published snapshots |
| `themes/*.qtheme` | The theme library |
| `uploads/` | Uploaded files |
| `settings.json` | Settings: the computer's on the desktop, `users/<login>/settings.json` on the server |
| `ai_log.jsonl`, `activity.jsonl` | The AI request log and the activity log |
| `secrets.json` | Server only; sealed |
| `files/<login>/` | Server only; each person's saved reports and exports |

---

## 6. Phases

Each phase lists **Build**, then **Done when** (the checks). Add tests as you go: section 7 lists the
kinds. Commit after every phase.

### Phase 0: skeleton that runs in a window and in a browser

- **Build:**
  - The Cargo project with a `desktop` feature (Tauri optional), `src/lib.rs`, `src/app.rs` with `App` state
    and the `commands!` macro, plus `dispatch(app, host, cmd, json) -> Result<Json, String>`.
  - `src/main.rs`: a Tauri window with `frontendDist: "ui"` and one Tauri command `call(cmd, args)`.
  - `src/bin/quire-server.rs`: a `std::net::TcpListener` that serves `ui/` files and `POST /api/call`, with a thread per connection.
  - `ui/platform.js` picks Tauri `invoke` or `fetch('/api/call')`.
  - `ui/state.js`: `S`, `call()`, `render()` listeners.
  - `ui/index.html` with the shell (title bar, rail, stage, panels) and `ui/styles.css` with CSS variables, light and dark.
- **Done when:**
  - `cargo run` opens a window and `cargo run --no-default-features --bin quire-server` serves the same page;
  - both show the result of a `get_state` call;
  - `cargo test` runs.

### Phase 1: document model, edits, undo, files

- **Build:**
  - `model.rs` (section 5) and `Edit` with the widget and page ops.
  - Undo and redo are snapshot stacks of `Doc`.
  - `new_doc`, `save`, `open_path`: `.rtpl` and `.rpt` as pretty JSON.
  - A first canvas in `canvas.js`: pages drawn at scale, with widgets as boxes on the 12×12 grid. Select, drag and resize snap to cells.
- **Done when:**
  - widgets can be added, moved and deleted;
  - ⌘Z and ⇧⌘Z work;
  - a saved file reopens identical;
  - there is a test of an edit → undo → redo round trip.

### Phase 2: the demo bank database

The original generator is `data/make_demo_db.py` (Python with the `duckdb` package; deterministic seed, ~10 s,
self-checks at the end). Rebuild it to this spec.

- **Tables** (the first six are the user's spec from `database.md`):
  - `clients`: `id` C0001…, `name` (inspired by NBA players), `birthday`, `nationality`, `sales_id`, `risk_profile`.
  - `sales`: S0001…; names inspired by famous investors; `sales_phone` "8-903-1234567"; `sales_email`.
  - `accounts`:
    - `acct_cd`, `acct_numb`, `acct_name` (Russian, e.g. `Вклад "Премиум"`), `acct_type`, `client_id`;
    - `src_cd` ∈ BANK, BROK, AM, INS (IIS counts as BROK);
    - `open_dt`, `close_dt` (3000-01-01 when open), `manager` (AM), `policy_end_dt` (INS).
  - `instruments`:
    - `instr_cd`, `isin`, `issuer_cd`, `instr_name` (bonds: `SBER-20260430-7.5`);
    - `instr_type`, `instr_type_ru` (Акции, Облигации, Фонды, Денежные средства, ОМС…);
    - `instr_nominal`, `maturity_dt`, `coupon`.
  - `balances`: daily, per position. `report_dt`, `client_id`, `acct_cd`, `instr_cd`, `qty`, `price`, `price_cur`,
    `amt` (equity qty×price; bonds qty×price×nominal), `fx_rate`, `amt_rub`, `avg_price`, `redeem_amt` (INS).
  - `portf_acct`: daily, per account. `bal_in` (= yesterday's `bal_out`), `net_flow`, `bal_out` (= Σ `amt_rub`),
    `net_twr`, `income`, `fees` (negative), `taxes` (negative), `gross_twr`, `net_twr_ltd`, `gross_twr_ltd`.
  - Added later:
    - `operations`: top-ups, withdrawals, buys and sells with `venue` and `fee`, conversions, coupons, redemptions, dividends, fees, taxes;
    - `cashflow_schedule`: coupons, redemptions, dividends and interest per unit, to 2027-12-31;
    - `fx_rates`: RUB per USD, CNY, EUR, AED, every day;
    - `prices`;
    - `benchmarks`: IMOEX, MCFTR, RGBI, S&P 500, the key rate, CPI;
    - `products`: `src_cd`, `name`, `description`, `name_en`, `description_en`, `sort_order`.
- **Rules:**
  - Currency RUB (not RUR).
  - Calendar days 2024-01-01 … 2026-12-31. Markets move on working days only, but bond accrued interest, deposit and savings interest still accrue on weekends.
  - Bond prices are dirty and drop back on the coupon date.
  - Inflows are positive and outflows negative. Fees and taxes are negative and are **not** flows.
  - `net_twr = bal_out/(bal_in+net_flow)` when `net_flow ≥ 0`, else `(bal_out − net_flow)/bal_in`. `gross_twr` is the same with fees and taxes added back.
  - An account opened in the period starts with a top-up; a closed account ends paid out.
- **Data:**
  - 41 clients and 8 sales people.
  - Test client **C1337 «Кирилл Шарихин»** (born 1984-09-20, sales S0001 Warren Buffington, profile «Консервативный», ~355 mln RUB on 2026-06-15). The client has 16 accounts: brokerage 400PX0F, IIS 510IIS7, AM DU.SBOLPF.256730, insurance INS-777120, savings, deposits, metal accounts, a card.
- **Done when:** the self-checks pass on every row (each rule above), and rerunning gives the same file.

### Phase 3: data sources, query engine, periods

- **Build:**
  - **`data.rs`** loads every `Source` kind into in-memory tables and infers types.
    - ClickHouse: HTTP via `curl`, `clickhouse://user:pw@host:8123/db`.
    - Python: a persistent `python3` worker over JSON lines (`python.rs`).
    - `Query`: SQL over other datasets by name in a **sandboxed DuckDB** with no file or network access.
  - Calculated fields (`name = SQL expr`), a per-dataset transform, and a per-widget `where_sql`.
  - **`period.rs`:** month, quarter, year, YTD and custom periods; comparison with the previous period or the same period last year; windows (period, YTD, last 3/6/12/24 months, all).
  - **`query.rs`:**
    - group by up to 3 dims;
    - aggregations, including **last value / first value / average across dates** for snapshot data such as balances;
    - comparisons, change and % change, share, rank, running total;
    - top N + "Other";
    - pivot;
    - detail mode.
- **Done when:** there are tests for YTD vs last year, balances using closing values not sums, top N with Other, a sandboxed SQL refusing file access, and joins across sources.

### Phase 4: fit engine

- **Build** `layout.rs` (`Spec` per kind: min/max sizes):
  - `resolve` collisions by pushing down;
  - `compact` gaps;
  - `shrink_to_fit` flexible widgets, never below a readable minimum or what text needs;
  - `fill` spare space;
  - `equalize_rows`;
  - `find_slot`;
  - `analyze` turns what cannot be solved into **issues with one-click fixes**: fit everything, move to next or new page, top N + Other, grow to fit text, remove. Hovering a fix previews it.
  - Widgets placed by hand stay put.
  - Long tables continue onto automatically added pages with the header repeated.
- **Done when:** there are tests named like `long_tables_continue_on_new_pages`, `a_widget_placed_by_hand_stays_put` and `overflow_reports_suggestions`.

### Phase 5: fonts and text measurement

- **Build:**
  - `fonts.rs`: bundled fonts from `ui/fonts/` (OFL: Inter, IBM Plex Sans, Manrope, Spectral, Cormorant Garamond, Playfair Display, Raleway, Montserrat, Roboto Condensed), system fonts, and fonts embedded in a document.
  - `text.rs`: measure and wrap with `ttf-parser` (Latin and Cyrillic), plus rich markup: `*italic*`, `**bold**`, `{#hex|coloured}`, `{$token|words}`, bullets.
  - `vm.rs` returns page geometry, wrapped lines and table metrics with each state.
  - Eleven text roles, each with family, size, weight, italic, colour, caps and tracking: heading, heading subtitle, widget title, widget subtitle, text, caption, KPI value, table header, table cell, chart label, footer.
- **Done when:** the tests `measures_and_wraps` and `cyrillic_is_measured` pass, and the UI draws text exactly where Rust wrapped it.

### Phase 6: the renderer (KPI, charts, tables)

- **Build** in JS (`render.js`, `charts.js`, `table.js`). Every widget becomes SVG from **one resolved
  settings object**: theme defaults ← chart preset ← design view ← widget's own.
  - **KPI:**
    - value, change as %, points or amount, "up is good";
    - sparkline (line or bars);
    - note with embedded values (`{1} profit since January`), other values as rows, a chip;
    - the card coloured by result.
  - **Charts:**
    - columns, bars, 100% bar, Range, line, area, donut, treemap, waterfall, tiles, bar + line combo with two axes;
    - colour by series or by sign;
    - labels, axis and grid, legend, best/worst highlights, reference lines;
    - per-kind options (donut hole/centre/table legend; waterfall totals and connectors; treemap by result…).
  - **Tables:**
    - summary or detail, column formats, data bars, colour scales, pills, markers and icons;
    - conditional rules on cell or row, best & worst marks;
    - subtotals and group rows, pivot, header groups, totals row styles, a second line per cell, row heights.
  - **Content that does not fit shrinks first.** Anything still cut off gets a dashed outline and a "Cut off · Fit" badge.
  - `vizschema.js` describes every setting with the section it belongs to (`SECTION_ORDER`: Type & view, Layout, Series, Value, Colours, Marks, Numbers, Data labels, …, Card & title) and the facts that decide when it shows.
- **Done when:**
  - `node dev/checks/inspector.mjs` passes (every setting in one section; no duplicates; every `S.x` read by `charts.js` has a control);
  - the probe in section 7 shows no dead controls.

### Phase 7: the designer UI

- **Build:**
  - **Left panel:** gallery, widgets, data and pages.
  - **Inspector:**
    - for a widget: **Content**, **Data** (teal), **Design** (blue), **Text** (amber), **Layout** (slate);
    - for the template: **Document**, **Page**, **Theme**, **Access**;
    - collapsible sections with icons and a badge counting changed settings;
    - a search box (⌘F) across all tabs, synonyms included;
    - Delete, Replace and Save to gallery pinned at the bottom.
  - **Chart type** gallery: switching keeps the data and ⌘Z undoes it.
  - **Click a part:** clicking a legend, axis, series, cell or KPI value opens its settings.
  - Multi-select with ⇧ applies shared settings to all.
  - **Menus:** a menu bar (File, Edit, Insert, Page, View, Theme, Help), right-click menus everywhere, and the shortcuts from README.
  - **Variables tab** for every `{{placeholder}}`.
  - Double-click on a control resets it.
  - Settings (⌘,): language, currency, date format, separators, negative style.
- **Done when:** the guide's chapters 2–6 *Test* lists pass by hand.

### Phase 8: themes, design views, theme studio

- **Build:**
  - Themes carry colours, fonts, **tokens** (`$accent`, `$good`, `$bad`, `$muted`, custom), chart defaults, a **Shape** (corners, grid, fills, labels, cards) and **design views** per widget kind (~45 shared views).
  - Each widget keeps only its view id plus its own overrides, so applying a theme with the same view ids restyles the whole report.
  - **Theme studio:** a workspace with an Overview specimen and a gallery per object type. Each card is the view drawn with sample data in the theme's language. Includes new view (from blank or another view), rename, default, reorder, delete.
  - **Theme library:** `.qtheme` files.
  - **Nine built-in themes:** Quire (IBM Plex Sans; white, grey cards, blue), Modern UHNWI (Spectral + Manrope; warm paper, oxblood), Alpine, Сбер, Сбер Private Banking (a typeface the client supplies, in `ui/fonts/private/`; IBM Plex Sans and Inter without it), Изумруд, Инфографика, Bold Blocks, Data Atlas.
- **Done when:** there are tests such as `studio_shows_every_view_and_edits_reach_the_theme`, `every_built_in_look_has_its_typefaces_bundled` and `theme_colours_by_name_follow_the_theme`, and switching themes on the UHNWI report changes shapes, not only colours.

### Phase 9: export

- **Build** `export.rs`:
  - **PDF:** each page's SVG → `svg2pdf` with fonts embedded, pages joined with `pdf-writer`.
  - **PPTX:** a hand-written OOXML zip with native text boxes, native tables (where the table has no pills or bars) and charts as SVG with a PNG fallback.
- **Done when:** the tests `pdf_has_pages_and_embedded_text` and `pptx_is_a_valid_package` pass, and the PPTX opens in LibreOffice or PowerPoint with editable text.

### Phase 10: templates for users

- **Build:**
  - **Parameters:**
    - choice lists typed or read from a column, multi-select, text/number/date;
    - a column each parameter filters in every source that has it;
    - a toolbar picker.
  - **Policy** (Document → Access): add, remove, arrange, restyle, comment, pages, edit text. The period and each parameter are *change*, *see only* or *hidden*. Locked widgets.
  - **Gallery** of saved widgets that users add or swap in.
  - **Repeating pages:**
    - `Page.repeat` (dataset + field/values + label) makes a page set per data row with `{{row.field}}`;
    - a dialog, badges and a row picker;
    - this is a **main feature** and must stay easy to configure.
  - **Contents widget:** List or Chapters layout, numbering, leaders, smart entries from page headings, "Fill in from the report…".
  - **Header and footer bands:** three slots each, with text, logo and placeholders; section tabs.
  - **Report language** (en, ru, de): `21 649 000 ₽`, `+2,34%`, `10,0 млн ₽`, `15.06.2026`.
  - **Empty widgets explain why** and offer a fix.
- **Done when:**
  - there is a test that report users are limited (`report_users_are_limited`);
  - account pages repeat per account;
  - closed accounts are marked "закрыт dd.mm.yyyy".

### Phase 11: the UHNWI sample report

- **Build** `uhnwi_sample.rs` + `uhnwi_sql.rs`:
  - an ~18–22 page Russian private-banking report on the Modern UHNWI design;
  - pages: cover, contents, summary, overview, results and benchmarks, brokerage/trust/bank account pages (repeating), holdings tables continuing across pages, monthly cash-flow forecast, trades and operations, legal page;
  - **every source is SQL on the demo tables**, using `:client`, `:period_start`, `:period_end` and `:access_src`.
- **Reconciliation:**
  - The account result `Σ(bal_out − bal_in − net_flow)` is the truth.
  - A shared CTE `acct_adj` adds "Налоги, комиссии и курсовая разница" so products = headline and positions = account.
  - "Получено" = dividends + coupons + redemptions (deposits: Σ income).
- **Done when:** the tests `every_source_runs_for_every_client`, `the_uhnwi_report_reads_every_source_from_the_bank_tables` and the totals-agree tests pass.

### Phase 12: roles, sign-in, areas, admin, audit, publishing

- **Build:**
  - **Sign-in screen** with demo buttons. The demo accounts are admin, data, design and user, password `quire`; `user` is a banker with coverage S0001.
  - **Rail areas:** Reports, Design, Data, Admin; an *Open* group for open documents; guide, settings and account at the bottom. The title bar shows only context. The user rejected a second bar per area as "looks like separate apps".
  - **Access = Clients** (every client, or coverage `clients.sales_id` + picked clients) **× Products** (every product, or a list of `accounts.src_cd`). It is enforced in the core. Insurance-only users see only policy pages.
  - **Publishing:** a template's Access tab publishes it as a report to an audience (everyone with Reports, groups or people). A snapshot goes to `reports/<id>.rtpl`.
  - **Admin:** Users & roles · Groups · Access overview · AI · Activity (audit of everything, names not logins, CSV export).
  - `audit.rs` writes `activity.jsonl` and builds work sessions.
- **Done when:** the tests `access_by_coverage_source_and_client` and `reports_go_to_their_audience` pass, and every new command has a `gate(...)`.

### Phase 13: the Data area

- **Build:**
  - **Connections and shared data sources** live in the store, owned by Data. Templates keep `shared: true` copies refreshed on load; only Data may change them.
  - **SQL workspace** as tabs inside the Data area:
    - tables tree;
    - editor with highlighting, completion and formatting;
    - read-only run with preview and `:parameters`;
    - edits written back all-or-nothing;
    - "add as data source".
  - **Report parameters page.**
  - **Python:** the person's own `python3` (Settings → Python; `QUIRE_PYTHON`).
- **Done when:** a source made in Data shows up in a template's data panel and changes there only when Data changes it.

### Phase 14: the user's area

- **Build:**
  - **Client list** (`overview.rs`): figures per client and "needs attention" (coming events).
  - **Batch reports:** one run makes every client's report. It doesn't touch the open document and writes PDF or PPTX into a folder.
  - **Proposals:** a proposal page before the legal text (two pages when long), a register with versions and status; export → *Presented*.
  - **Saved list:** on the web.
- **Done when:** the tests `batch_reports_leave_the_open_document_alone`, `overview_reads_figures_and_whats_coming` and `proposals_are_kept_in_the_register` pass.

### Phase 15: AI assistants

**Read the whole phase before starting.**

- **Providers** (`ai.rs`):

  | Provider | Endpoint | Default model | API style |
  |---|---|---|---|
  | Claude | `api.anthropic.com/v1` | `claude-sonnet-5-5` | Anthropic |
  | ChatGPT | `api.openai.com/v1` | `gpt-4.1` | OpenAI |
  | DeepSeek | `api.deepseek.com` | `deepseek-chat` | OpenAI |
  | Qwen | `dashscope-intl.aliyuncs.com/compatible-mode/v1` | `qwen-plus` | OpenAI |
  | GigaChat | `gigachat.devices.sberbank.ru/api/v1` | `GigaChat-2-Max` | OAuth at `ngw.devices.sberbank.ru:9443/api/v2/oauth` |

  - Calls go through `curl`.
  - Endpoints and models can be changed in settings, so a test can point at a local stand-in.
  - Keys come from a personal key first, then the bank-wide ("org") key set by an admin.
- **Policy** (Admin → AI):
  - The AI switch is on or off.
  - Allowed providers.
  - **Two data levels only:**
    - *Structure only* (the default) sends names of tables and fields, never rows;
    - *Figures* sends aggregated numbers.
  - The level is set by bank default, then group (the widest wins), then person (first).
  - **Identities are always masked:** client names, account, instrument and ISIN codes, and people become `[CLIENT]`, `[ID-n]`, `[P-n]`, `[B-n]`, `[CL-n]` per request and are unmasked locally.
  - Every request is logged in `ai_log.jsonl` with the prompt, the exact masked text sent, the raw answer and tokens. Use and cost use editable prices.
- **Assistants, in the order the user approved:**
  1. **Data, the SQL assistant:**
     - write, explain or fix SQL, make a report source, answer questions about the schema;
     - only structure is sent;
     - the app runs each attempt and sends back only the error (up to 3 tries).
  2. **Designer:**
     - "New page" / "This page" from a description;
     - the plan is checked against the template, applied as `Edit::AiPage` (one undo step), and data errors and fit issues are fed back.
     - **Restyle from a picture:** PNG, JPEG, GIF frames, or PDF pages via pdf.js. The model returns a look plus per-view patches; colours, installed typefaces and contrast are checked.
  3. **User:**
     - brief & pitch with main ideas in `**…**`, highlighted;
     - an e-mail to the client: a ready-to-send letter with no internal notes;
     - a check before sending;
     - a note for a page;
     - proposals.
     - This needs the *Figures* level.
  4. **Administrator:**
     - ask the activity log (refs become links);
     - change access in words (proposal → before/after → apply on confirmation; never applied by the model);
     - explain a failure;
     - review access.
- **Done when:**
  - the tests run against a **stand-in provider on a local port**, with no real calls;
  - they include `identities_go_out_masked_and_come_back`, `ai_level_by_person_group_or_bank`, `sql_assistant_retries_with_the_database_error` and `design_assistant_builds_a_page_in_one_undo_step`;
  - **Real AI calls only with the user's permission**, because they cost money.

### Phase 16: widget types and composites

- **Build:**
  - **Widget types:** save a configured widget as a type; widgets of that type follow updates to it.
  - **Composite widgets:** parts on their own grid inside one card, each with its own data, edited as `parent::part`.
  - **Widget studio** (Advanced view) to build and version them.
- **Done when:** the test `a_composite_lays_out_its_parts_with_their_own_data` passes, and updating a type updates every widget of that type.

### Phase 17: web server mode

- **Build** in `quire-server.rs` and the core:
  - A **workspace per browser** via cookie: its own sign-in, documents and undo, closed after 12 idle hours.
  - Per-person settings and files.
  - Uploads and downloads through `platform.js`. Only documents inside the data folder open.
  - **Secrets:** `secrets.rs` seals AI keys and database passwords in `secrets.json` with ChaCha20-Poly1305 and `QUIRE_SECRET_KEY` (16+ characters). The page sees `••••••`.
  - **Data fence:**
    - files only from the data folder, the demo database's folder and `--allow-dir`;
    - DuckDB `allowed_directories`;
    - no SQLite `ATTACH`;
    - `--allow-host` for database servers.
  - **Python is off on a server** until an admin switches it on (Admin → Access).
  - **Sign-in lockout:** 5 failures rest the login for 15 minutes.
  - **HTTP limits:** `--secure` cookies, security headers, no framing, 300 MB / 2 min / 512 connection limits.
  - **Server flags:** `--port 8766 --data DIR --host --ui DIR --allow-dir --allow-host --secure --dev`.
  - **Dockerfile:** `rust:1-bookworm` build → `debian:bookworm-slim` with curl, python3, pandas.
- **Done when:**
  - `node dev/web/checks.mjs <url> <datadir>` passes all checks (workspaces apart, files, sign-in, Python, secrets, fence, HTTP);
  - `node dev/load/reports.mjs 10 <url>` runs without failures.
  - Expect about 3 CPU-seconds per person for sign-in, report and one period change, so the server serves about one person at once per core.

### Phase 18: language, guide, polish

- **Build:**
  - **Interface languages:** English and Russian (`i18n.js` + dictionary `i18n-ru.js`). Text, tooltips and placeholders are translated as they render. Report content never is.
  - **In-app user guide:** `ui/help/guide.md`, opened with F1. It has 17 chapters, each ending with a *Test* checklist, and screenshots made by `dev/shots/`.
  - **Icon library** (`iconlib.js`): Lucide (ISC), flag-icons (MIT), Simple Icons (CC0), generated into one JS file. Plus report glyphs that export as vectors.
- **Done when:** the guide's chapter 17, *Full test checklist*, passes by hand.

### Backlog after the phases

These are not started. The details are in `backlog/*.md`.

- Act on "needs attention" (done, snooze, log contact);
- Report API for other systems;
- Proposal campaigns;
- SSO;
- a VPS deployment behind Caddy;
- per-person Python on a server: an open question for the user.

---

## 7. Testing

| When | Check |
|---|---|
| Every change | `cargo test`: the whole suite passes. Failures are fixed, not skipped. Tests need `data/demo.duckdb`. |
| Design-tab settings changed | `node dev/checks/inspector.mjs`: reads `vizschema.js` and `charts.js`, no browser |
| Anything that draws pages or screens | Headless Chrome against `quire-server --dev`, driven over the Chrome DevTools Protocol by small Node scripts (`dev/shots/cdp.mjs` exports `send`, `ev`, `shot`, `sleep`): sign in (`.lg-demo button[data-l=design]`), open a document, screenshot, and read the app's own `fit_problems` |
| A new setting | **Probe:** set it on a real widget, compare the widget SVG before and after; ✗ = dead control. Compare `.svg` (not the `{svg, frame}` object) and normalise running ids (`/ic\d+/g`) or everything looks changed. |
| Server, sign-in, secrets, files | `dev/web/checks.mjs` on a fresh data folder |
| AI | Stored answers or a stand-in HTTP provider; **never a real provider without the user's OK** |
| Before a release | The guide's *Test* steps, by a person |

On Linux the headless browser is `chromium --headless=new --remote-debugging-port=9222`. Change the Chrome
path in `cdp.mjs`, which points at the macOS Chrome app.

Each fixed bug gets a test that fails without the fix. Each new screen gets a `dev/shots/` script.

---

## 8. Linux notes

Quire builds and runs on Linux since 0.2.0; GitHub Actions checks it on Ubuntu 24.04 (`.github/workflows/linux.yml`).
What differs from macOS:

| macOS piece | Linux |
|---|---|
| AI keys and connection passwords in the Keychain (`/usr/bin/security`, `src/ai.rs`) | The sealed `secrets.json` the server uses, in the app's data folder (`src/main.rs` setup → `secrets::use_file`). |
| "Show in Finder" (`open -R`) | `xdg-open` on the file's folder; the UI says "Show in folder" (`FILES_APP` in `ui/platform.js`). |
| Screen eyedropper (NSColorSampler, `objc2`, macOS-only deps) | Hidden (`CAN_PICK_SCREEN`); WebKitGTK has no EyeDropper. |
| Title bar overlay with window buttons | Ignored by Tauri; the title bar drops its left gap (`html[data-os="other"]`). |
| App data folder | `~/.local/share/app.quire.reports/` (Tauri). |
| Headless Chrome for checks (`dev/shots/cdp.mjs`) | `google-chrome`, or the path in `CHROME`. |
| Shortcuts shown as ⌘ | Ctrl (`isMac`/`mod` in `ui/util.js`). |

Packages: `libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libssl-dev libxdo-dev`
for the desktop app; the web server needs none of them.

---

## 9. Pitfalls already learned

- **Build disk use.** `target/` once grew to 92 GB. Keep the profile below, and `cargo clean` when `target/` passes ~20 GB. A clean build takes ~4 min, mostly DuckDB.
  ```toml
  [profile.dev]
  opt-level = 1
  debug = "line-tables-only"
  [profile.dev.package."*"]
  opt-level = 2
  debug = false
  ```
- **Claude Sonnet 5.5 always thinks.** Use effort low or medium and `max_tokens ≥ 16000` for report tasks, or answers come back empty.
- **Clearing a setting (null) can't override a design view's value.** Offer an explicit *off* (`false`).
- **Demo logins are common words** (`user`, `data`). When masking people in AI text, replace logins only in person fields.
- **DuckDB in a multi-user server:** share instances per file (`duck_shared`) and release them, or memory explodes.
- **Old documents must keep opening.** Migrate on load (e.g. old `look` → explicit settings), and test that the result looks identical.
- **A data source added to a template moves the template's period onto the data** if the data has no rows in the current period.
- **The UI is served at the root** on the server (`/state.js`, not `/ui/state.js`). Test scripts import from there.
- **Never kill or reuse the user's own server port** (8766) in tests; start a test server on another port with a fresh `--data` folder.

---

## 10. Working conventions

- **`RELEASES.md`:**
  - newest first, one dated heading per working day, with **New / Improved / Fixed**;
  - written for users, with file names as pointers;
  - add an entry after every change.
- **`TESTING.md`:** describes every kind of check. When you add a new kind, list it there.
- **`backlog/`:**
  - one markdown file per deferred idea, plus a `README.md` table (item, status, file);
  - when an idea is discussed and deferred, offer to add it;
  - when one is built, mark it done in its file and the table.
- **User guide:** a new feature adds its text and *Test* steps to the matching chapter of `ui/help/guide.md`.
- **Comments:** short and in plain words, explaining *why*. Match the surrounding code's density and naming.
- **Asking first:**
  - before adding a database table;
  - before any real AI call (it costs money);
  - before anything destructive;
  - before changing a decision recorded here.

---

## 11. Prompt to start each phase

Give the model this, filling in the phase number:

```
You are rebuilding Quire, following REBUILD.md in this folder. Read sections 1–5 and 9 first.
Now do Phase <N> only. Before coding, list the files you will create or change and the tests you
will add. Write the code, run `cargo test` (and the phase's other checks), and fix every failure.
Then add today's entry to RELEASES.md and show me: what works, how to try it, and what you
did not do. Follow the rules in section 3 — in particular, every control you add must visibly
change the output. If something is unclear, ask me instead of guessing.
```

When a phase is too big for one session, split it at the bullets in its **Build** list, one bullet per session,
and keep the **Done when** checks for the end.
