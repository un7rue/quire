# Testing

The checks run before a change is called done, and how to run them yourself. Changes are listed in
[RELEASES.md](RELEASES.md).

## What runs, and when

| When | Check | Command | Takes |
|---|---|---|---|
| **Every change** | Engine tests (Rust) | `cargo test` | ~35 s (plus build) |
| Any change to the Design tab's settings (`ui/vizschema.js`, `ui/panels.js`, `ui/charts.js`) | Inspector check | `node dev/checks/inspector.mjs` | 1 s |
| …and also, since the check only reads the settings list and doesn't build the panel | The inspector built in the app for one widget of each kind: its parts, and any script error | `node dev/shots/inspector.mjs <out>` (needs the web server) | 30 s |
| Any change to how pages look (themes, charts, tables, layout) | Render check in a headless browser: pages drawn as the app draws them, then looked at | `dev/shots/*.mjs` (see [below](#render-checks)) | 10–60 s per script |
| Any change to a chart's series settings | A part per series: layout and each setting drawn | `node dev/shots/series-parts.mjs <out>` (needs the web server) | 30 s |
| Any change to selection or the inspector's tabs | Several widgets selected: shared settings, changes, moves and deletes reach all | `node dev/shots/multi-select.mjs <out>` (needs the web server) | 30 s |
| A change to a screen (client list, dialogs, admin, AI panes) | The screen driven in a headless browser and screenshotted | `dev/shots/*.mjs` | 10–60 s |
| Any change to Admin → API (`ui/apiadmin.js`, the access editor in `ui/areas.js`) | An account added, its key shown once and nowhere after, a report's parameters and default saved, Activity without the key | `node dev/shots/admin-api.mjs <out> [en\|ru]` (needs the web server) | 30 s |
| Any change to the report API (`src/app/report_api.rs`, `src/api_accounts.rs`, `src/render_js.rs`) or to the drawing code it runs (`ui/render.js`, `ui/charts.js`, `ui/table.js`, `ui/util.js`…) | The API's tests with V8 built in, then its pages against the same report drawn in Chrome | `cargo test --lib --no-default-features --features render -- report_api render_js api_accounts api_jobs api_delivery`, then `node dev/shots/api-render.mjs` | ~1 min (first build: V8 downloads) |
| Any change to the report API's server side (`src/api_jobs.rs`, `src/bin/quire-server.rs`) | End to end against a running server: an account's key, one report, a batch's zip, refusals, Activity | `node dev/web/api-checks.mjs` ([below](#6-web-server-checks)) | 10 s |
| **Every push to GitHub** | On Linux (Ubuntu 24.04): engine tests (with the renderer), the web server and its checks, the report API's checks, the desktop app, `.deb` and AppImage | GitHub Actions, `.github/workflows/linux.yml`; `gh run watch` | ~20 min |
| Before a release that changes what users do | The **Test** steps in the user guide, by a person | [ui/help/guide.md](ui/help/guide.md), each chapter's *Test* list | — |

A change is done when `cargo test` passes in full. Failures are fixed, not skipped. If a check wasn't run, the release
note says so.

## 1. Engine tests — `cargo test`

About 150 tests in `src/` (`#[test]`); they need `data/demo.duckdb` (the demo bank database). The main areas:

| Area | What is checked (examples of test names) |
|---|---|
| **Data and SQL** | types read from files, SQL parameters and filters, joins, sandboxed SQL over loaded tables, Python sources, connections listing tables, ClickHouse over HTTP (a stand-in server checks what is sent), edited cells saved all-or-nothing (`a_clickhouse_source_loads_over_http`, `clickhouse_answers_become_tables`, `infers_types`, `sql_fields_transforms_joins_and_filters`, `sql_over_loaded_tables_is_sandboxed`, `python_reads_other_sources_and_runs_on_a_result`) |
| **Periods and figures** | month, quarter, year-to-date and trailing windows, comparisons, closing balances not sums, running totals, top N with "Other" (`ytd_compares_to_last_year`, `balances_use_closing_values_not_sums`, `top_n_with_other`) |
| **The UHNWI report** | every source runs for every client, totals agree across pages, account pages repeat per account, the money page agrees with its cards (`every_source_runs_for_every_client`, `the_uhnwi_report_reads_every_source_from_the_bank_tables`, `money_page_agrees_with_its_cards_and_deals_carry_type_and_result`) |
| **Composite widgets** | parts laid out in the card with their own data, edited / moved / removed by `parent::part`, overlaps refused, types carrying parts (`a_composite_lays_out_its_parts_with_their_own_data`) |
| **Layout and fitting** | free slots, pushing down, shrinking to fit, long tables continuing onto new pages, pinned and hand-placed widgets staying put, text that doesn't fit reported (`long_tables_continue_on_new_pages`, `a_widget_placed_by_hand_stays_put`, `overflow_reports_suggestions`) |
| **Text** | measuring and wrapping (Latin and Cyrillic), rich-text markup, bullets (`measures_and_wraps`, `cyrillic_is_measured`, `rich_markup_makes_runs`) |
| **Themes** | design views, the theme studio, typefaces following text roles, header and footer belonging to the theme, readable colour pairs, restyle checks (`studio_shows_every_view_and_edits_reach_the_theme`, `view_typefaces_follow_the_text_roles`, `a_theme_takes_its_header_and_footer_away_with_it`, `bad_colours_fonts_and_contrast_go_back`, `every_built_in_look_has_its_typefaces_bundled`, `theme_colours_by_name_follow_the_theme`, `built_in_themes_use_their_colours_by_name`, `every_card_view_shows_a_card`, `quire_names_shared_widgets_once_and_templates_follow`, `halves_meet_without_a_divider`, `monthly_and_running_totals_use_two_axes`) |
| **Access and roles** | passwords and roles, coverage × products, insurance-only users, report users limited to what the template offers, publishing to an audience (`access_by_coverage_source_and_client`, `report_users_are_limited`, `reports_go_to_their_audience`) |
| **Proposals** | the page before the legal text, two pages when long, actions continuing, the register with versions and status, export → *Presented* (`the_proposal_is_a_page_before_the_legal_text`, `proposals_are_kept_in_the_register`, `exporting_presents_the_proposals`) |
| **Batch and client list** | batch reports leave the open document alone, the overview's figures and coming events, incremental activity index (`batch_reports_leave_the_open_document_alone`, `overview_reads_figures_and_whats_coming`) |
| **AI** (no provider called) | identities masked and restored, data levels by bank / group / person, plans checked before applying, answers retried with the error, request formats per provider (`identities_go_out_masked_and_come_back`, `ai_level_by_person_group_or_bank`, `design_assistant_builds_a_page_in_one_undo_step`) |
| **Export** | PDF pages with embedded text, a valid PowerPoint package, Word and PowerPoint text extraction (`pdf_has_pages_and_embedded_text`, `pptx_is_a_valid_package`) |
| **Report API** | a request's client by the bank's id, named periods, parameters checked against the report's own and set only when Admin lets the API set them, Admin's defaults, refusals with their status (404 / 403 / 422), the open document untouched; API accounts: keys shown once and kept as a hash, a new key replacing the old, a switched-off account refused, an account acting with its own clients and reports, Admin's commands for administrators only and no key in the store or the log (`keys_sign_api_accounts_in`, `report_settings_are_checked`, `api_accounts_act_with_their_own_access`, `admin_manages_api_accounts`); with `--features render`: every page drawn by the UI's code in V8 and written as a PDF, ICU's number formats, text measured by the core (`makes_the_report_asked_for`, `refuses_what_it_cant_make`, `named_periods_count_back_from_today`, `draws_the_report_without_a_browser`, `measures_text_and_formats_numbers`) |
| **Activity log** | events written and read back, work sessions (`events_are_written_and_read_back_filtered_with_ai_requests`) |

**Without the client's typeface.** The repository doesn't have `ui/fonts/private/` (`.gitignore`), so before a
release move that folder aside and run `cargo test` again: `every_built_in_look_has_its_typefaces_bundled` checks the Сбер themes fall back to
bundled fonts. **Version:** the sign-in screen shows *Quire* and the `Cargo.toml` version; `quire-server --version` prints it.

Run one area: `cargo test proposal`, `cargo test theme`, etc. (matches test names). Full list: `cargo test -- --list`.

**Helpers, not tests** (marked `#[ignore]`, run by hand):

- `QUIRE_DUMP=dir cargo test dump -- --ignored` writes UI state snapshots.

## 2. Inspector check — `node dev/checks/inspector.mjs`

This check covers the designer's Design tab. For every chart kind and every combination of facts that decide which
settings show (horizontal, stacked, several series, percentages…), it checks three things:

- every setting lands in one of the agreed parts (`SECTION_ORDER` in `ui/vizschema.js`: *Type & view, Layout, Series,
  Value, Colours, Marks, Numbers, Data labels, …, Card & title*);
- no setting shows in two different parts within the same chart type;
- every setting the chart drawing reads (`S.something` in `ui/charts.js`) has a control. No hidden settings.

It prints `✓` or lists each problem and exits with an error.

## 3. Render checks — pages as the app draws them

The UI draws pages in JavaScript, so page checks run the real UI in headless Chrome against the web server (the
same core and commands as the desktop app):

```sh
cargo run --no-default-features --bin quire-server -- --dev   # the core on :8766 (keep running)
python3 -m http.server 8767             # from the repo root
node dev/shots/<script>.mjs <out dir> …
```

| Script | Used to check |
|---|---|
| `pages.mjs <out> "<client>" 19,20` | report pages for a client, as PNGs |
| `theme-pages.mjs <out> <file.qtheme> 4,6,20` | the UHNWI template in a saved theme; `SVG=1` writes SVGs to inspect colours and fonts |
| `inspector.mjs <out> [bar,kpi,…]` | the Design tab of one widget per kind: its parts in order, screenshots, and any script errors |
| `probe-controls.mjs <out>` | **every new setting must change the drawing**: sets each one on a real widget and compares the page before and after (✗ = a control that does nothing) |
| `dblreset.mjs <out>` | double-click reset: a designer's chart, card and text settings, a report user's Fit and Numbers; also the Design home's *Edit* |
| `design-home.mjs <out> [en\|ru]` | the Design area's start page and its Templates, Widgets and Themes tabs: screenshots, text, how many widgets were drawn, script errors |
| `publish-flow.mjs <out>` | duplicate a template → it is listed as a draft → *Publish…* from its line → listed as published |
| `theme-usedby.mjs <out>` | a theme used by many templates (makes 12 copies in Heritage): the *Used by* list and the Templates tab filtered to the theme |
| `series.mjs <out>` | Series and a right axis on the column + line chart: before and after, and the Series cards |
| `studio-reset.mjs <out>` | in the widget studio: double-click resets a chart setting, a card setting and a text size; ↑ on an empty size box steps from the value shown |
| `widget-open.mjs <out>` | every built-in widget opened in the studio shows what its Widgets-tab preview shows; a click brings the studio to the front |
| `widget-studio.mjs <out>` | the widget studio: open a built-in widget from the Widgets tab, change it, *Save as my widget…*, change again, *Save widget* (version 2) |
| `widget-types.mjs <out>` | widget types: save a KPI as a type, add another from the Widgets panel, update the type (both follow) |
| `parts.mjs <out>` | click-a-part: hovers and clicks a legend, axis, labels, a series, a table cell, a KPI value… and reports which part of the inspector opened |
| `proposal.mjs`, `batch.mjs`, `clients.mjs` | the proposal dialog and page, batch reports, the banker's client list |
| `admin.mjs`, `sessions.mjs`, `admin-ai.mjs`, `brief.mjs`, `design-ai.mjs`, `restyle-ai.mjs`, `guide-ai.mjs` | admin parts and the AI panes, from **stored** answers |
| `restyle-real.mjs` | a **real** restyle; it calls the AI provider and costs a request |

What a render check looks at:

- **Nothing cut off.** The app's own fit check (`fit_problems`) reports cut-off text, overflow and clipped table rows.
  A change must not add any.
- **Readable.** Text and icons stand out from their fill (the contrast rules in `ui/util.js` `legible`, and the theme
  checks).
- **Right data.** Figures agree with the cards and totals around them.
- **The screenshot itself**, compared with the reference or the user's screenshot of the problem.

## 4. AI checks

- Tests and screenshots of AI features use **stored answers**; no provider is called and nothing is spent.
- A real AI call (e.g. `restyle-real.mjs`) is run **only with the user's permission**, since it costs credit.
- Client, account and instrument names and numbers are masked before anything is sent; tests check this
  (`identities_go_out_masked_and_come_back`, `report_facts_are_figures_without_identities`).

## 5. The guide's test steps (by a person)

Each chapter of [ui/help/guide.md](ui/help/guide.md) ends with a **Test** checklist: what to click and what should
happen, e.g. *"Select a column chart, a donut, tiles and a KPI in turn: the Design tab's parts come in the same order"*.
These are for a person checking a release in the app. New features add their steps to the matching chapter.

## 6. Web server checks

Any change to the server, sign-in, secrets, files or what data may be read runs the server's checks against a fresh
data folder:

```sh
QUIRE_SECRET_KEY='a test passphrase here' cargo run --no-default-features --bin quire-server -- \
    --port 8770 --allow-host db.bank.local --data /tmp/quire-check      # a new, empty folder
node dev/web/checks.mjs http://127.0.0.1:8770 /tmp/quire-check
```

They check, each printed ✓ or ✗:
- **Workspaces apart:** two people side by side, a banker refused a designer's action, no cookie means signed out.
- **Files:** each person's saved reports their own (listed, downloaded and deleted only by them); documents open only
  from the data folder.
- **Sign-in:** five wrong passwords rest the login.
- **Python:** off on a fresh server, a Python source refused, only an administrator switches it.
- **Secrets:** no AI key or database password in plain text anywhere in the data folder; the page sees passwords as `••••••`.
- **The data fence:** CSV, DuckDB SQL and images outside the allowed folders refused, uploads read, a database server not
  allowed refused, and the demo report still loads whole.
- **HTTP:** security headers, the cookie's flags, oversized requests refused, no files outside the UI folder.

**Load:** `node dev/load/reports.mjs <people> <url>` — that many bankers at once sign in, open a client's 22-page report
and change the period. Prints the median, 95th percentile and slowest time for each step, and any failure. Measured on
a 10-core Mac (29 Oct 2026), dev build: one person's sign-in + report + period change costs about 3 CPU-seconds; 1
person opens a report in 0.6 s, 10 at once in 2.6 s, 50 in 11 s, 100 in 24 s, with no failures and no data errors —
the server is busy with the work, not waiting on locks. The release build measures the same (0.56 s alone, 3.1 s for
10, 13 s for 50, 25 s for 100): the time is DuckDB running the report's SQL, already optimised in the dev build.

**Report API end to end** (a server built with the renderer, on fresh data):

```sh
QUIRE_SECRET_KEY='a test passphrase here' cargo run --no-default-features --features render --bin quire-server -- \
    --port 8771 --data /tmp/quire-api-check      # a new, empty folder
node dev/web/api-checks.mjs http://127.0.0.1:8771
```

An administrator makes an API account; with its key: no key or a wrong one refused (401); the reports it may ask
for; one report (202, a job, done with its pages and values, the PDF); the same `Idempotency-Key` getting the same job;
a batch of 3 (409 while running, then a zip, and each PDF on its own); another banker's client refusing the whole batch
(403, the client named); an unknown client (404), a parameter or period that doesn't fit (422); a folder destination
tested; a callback to a host not allowed refused (422); a batch copied to the destination and called back once, the
signature checked with the key alone as the caller's team would (Node's crypto); Activity with the
requests and reports under the account, never the key; a switched-off account's key refused. On 10 Nov 2026 (dev
build, 2 workers): one report 1.0 s, a batch of 3 in 1.8 s. The unit tests (`api_jobs`, with `--features render`):
jobs kept across a restart and finished, files deleted past the retention days, the rate limit, copies and a signed
callback, a callback not taken tried again then failed; `api_delivery`: HMAC against RFC 4231, folder copies, an S3
upload as a signed PUT (to a stand-in server), signed callbacks; `api_accounts`: callback hosts, destinations.

**Report API pages against the browser:** `draws_the_report_without_a_browser` writes the report it drew to
`$TMPDIR/quire_api_report` (`.json` its state, `.svg` its pages, `.pdf`); `node dev/shots/api-render.mjs` draws the same
state in headless Chrome as the app does and compares every text item. On 10 Nov 2026 (UHNWI, 18 pages): all 1,750
items had the same words (no line wrapped or cut elsewhere); 1,210 in the same place, the rest within 1.85 pt (column
widths, from kerning the core doesn't measure). One report draws in ~50 ms once the engine is up (~0.27 s for the
first on a thread).

Unit tests for the server parts: `secrets_are_sealed_and_come_back`, `connection_passwords_come_off_and_back`,
`hidden_passwords_are_put_back`, `wrong_passwords_rest_the_login`.

## Adding checks

- **A fixed bug** gets a test that fails without the fix where the engine can show it, e.g.
  `a_theme_takes_its_header_and_footer_away_with_it`.
- **A new setting** goes into an existing part of the Design tab, and `node dev/checks/inspector.mjs` must pass. It
  must visibly change the output.
- **A new screen** gets a `dev/shots/` script if it needs checking by eye, listed in `dev/shots/README.md`.
