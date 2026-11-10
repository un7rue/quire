# Guide screenshots

Drives the real UI in headless Chrome against the web server (no Tauri window), for `ui/help/img`.

    cargo run --no-default-features --bin quire-server -- --dev   # the core on :8766 (keep running)
    python3 -m http.server 8767             # from the repo root
    node dev/shots/guide-all.mjs ui/help/img  # the guide's screenshots 01–48, in Quire
    node dev/shots/guide-ai.mjs ui/help/img # the AI chapter's screenshots

For the guide, run the server on clean data so nothing from earlier tests shows up:

    rm -rf /tmp/qb && mkdir /tmp/qb && cargo run --no-default-features --bin quire-server -- --dev --data /tmp/qb

Without `--data` the server keeps its data in `QUIRE_DATA` (or `QUIRE_BRIDGE_DIR`, as the old bridge did), else `~/.quire-server`. Picture 57 (Activity,
an AI request) needs AI requests in the log: put a few lines in `$QUIRE_BRIDGE_DIR/ai_log.jsonl` (fields as
`ai::log` writes them: `at`, `user`, `feature`, `provider`, `model`, `ms`, `ok`, `tokens_in`, `tokens_out`,
`prompt`). Pictures 54–58 come out under other names; copy them as `design-page-en` → `54-ai-design`,
`restyle-en` → `55-ai-restyle`, `admin-access-en` → `56-admin-access`, `admin-activity-ai-en` →
`57-admin-activity`, `admin-ai-en` → `58-admin-ai`.

| Script | What it shoots |
|---|---|
| `guide-all.mjs <out> [prefixes]` | The guide's pictures 01–48 (home, workspace, menus, inspector, charts, tables, contents, data, parameters, settings, theme studio), each document in Quire; `13` shrinks a widget for the cut-off and undoes it; prefixes pick some (`01 13 4`) |
| `guide-ai.mjs` | The SQL assistant, a question about the database, *make it a report source*, Settings → AI, the AI button (49–53) |
| `design-ai.mjs <out> [en\|ru]` | The design assistant building a page (54 as `design-page-en.png`) |
| `restyle-ai.mjs <out> [en\|ru]` | *Look*: restyling after a drawn reference picture (55 as `restyle-en.png`) |
| `admin.mjs <out> [en\|ru]` | Admin's parts: Users & roles, Access, AI, Activity (a session, then *All events* with an AI request open) (56–58) |
| `sessions.mjs <out> [en\|ru]` | Admin → Activity → Sessions: makes a banker's session through the server (a report, an edit, time, another client), then shows it — it writes to the app's activity log |
| `clients.mjs <out> [en\|ru]` | The banker's client list: figures, *Needs attention*, a client's details, ⌘] |
| `admin-api.mjs <out> [en\|ru]` | Admin → API: adds an API account (its key shown once, then nowhere in the page or Activity), one banker's clients as its access; lets the UHNWI report take products with a default; adds a folder destination (in `<out>/dest`), tests it, has the account copy there and allow callback hosts (prints ✓-style lines; screenshots `admin-api-*`). Uses `live.html?web` |
| `api-render.mjs [<files>]` | The report API's pages (V8 in the server) against the same report drawn in Chrome: text items whose words differ, and how far the rest moved. Needs no server: run `cargo test --lib --no-default-features --features render -- report_api::tests::draws` first (it writes `$TMPDIR/quire_api_report.*`) |
| `pages.mjs <out> "<client>" <pages>` | Report pages as the app draws them, for a client (e.g. `19,20`) |
| `theme-pages.mjs <out> <file.qtheme> <pages>` | The UHNWI template in a saved theme, pages as PNG (`SVG=1` writes the SVG instead) — for checking a restyled theme |
| `inspector.mjs <out> [kinds]` | The Design tab of one widget per kind: its parts in order (printed) and screenshots |
| `probe-controls.mjs <out>` | Sets each newer setting on a real widget and reports whether the page's drawing changed (✓ / ✗) |
| `dblreset.mjs <out>` | Double-click a setting's name to reset it, as a designer and as a report user; the Design home's Edit |
| `design-home.mjs <out> [en\|ru]` | The Design area's start page |
| `publish-flow.mjs <out>` | Duplicate → listed as a draft → Publish… → listed as published |
| `named-colours.mjs <out>` | Picks *Accent* in a widget's colour field: kept as `$accent`, drawn in the accent, follows a new accent (prints the result) |
| `multi-select.mjs <out>` | ⇧-click two column charts, then a chart and a KPI: the shared tabs and sections (printed); a change, a move and a delete reach both, each one undo step; *Mixed* where they differ; search in the shared settings |
| `dividers.mjs <out>` | Halves without a divider: no vertical divider left, where the halves meet (12; the forecast's thirds 8 and 16), layout issues; pages as PNG |
| `series-parts.mjs <out>` | A two-series chart: the layout of Series, Colours, Marks, Highlights and Data labels (printed), and one per-series setting of each kind changing the drawing (✓ / ✗) |
| `parts.mjs <out>` | Click-a-part: hovers and clicks each part of a few widgets and prints which inspector part opened |
| `proposal.mjs <out> ["<client>"]` | The proposal: the dialog, the page laid out as written, and a fuller page from a stored answer (no AI called) |
| `batch.mjs <out> [en\|ru]` | Batch reports: the dialog, the panel when done — writes the six demo clients' PDFs into the temp folder `quire-batch` |
| `admin-ai.mjs <out> [en\|ru]` | Admin's AI assistant from stored answers (no provider called): a question, access in words, a review, a failure explained, cost (`SCHEME=dark` for dark) |
| `brief.mjs <out> <brief.json> [en\|ru]` | The brief in the report pane from a stored answer, plus its e-mail HTML and PDF pages as SVG (`BRIEF_SVGS=<dir> cargo test brief_pdf_from_svgs -- --ignored` makes the PDF) |
| `guide-page.mjs <out> [anchor]` | The in-app guide opened at a chapter, to check how it renders |
| `restyle-real.mjs <out> <picture or GIF> "<words>"` | **A real restyle** of the UHNWI template — it calls the AI provider and costs a request with pictures; pages before and after (`PAGES=3,4,6` picks them) |

Except `restyle-real.mjs`, the AI screenshots use stored sample answers: no AI provider is called.

`live.html` / `live.js` load `ui/` with its commands sent to the web server (looking like the desktop app; add `?web` for the browser version); `cdp.mjs` is a tiny
Chrome DevTools client (Chrome must be installed).
