# Quire — User guide

Quire designs report **templates** and turns them into finished **reports**: annual reports, client portfolio
reports, management packs. Designers connect data, lay out pages on a grid and style every chart and table;
report users pick a period and filters, add commentary and export to PDF or PowerPoint.

This guide walks through every feature. Each chapter ends with a **Test** list: steps and the result to expect,
so the guide doubles as a test script. Screenshots use the built-in examples (*Modern UHNWI report* and the
*Sales Performance Review* sample).

**Contents**

1. [Getting started](#1-getting-started)
2. [The workspace](#2-the-workspace)
3. [Pages and layout](#3-pages-and-layout)
4. [Widgets](#4-widgets)
5. [Charts](#5-charts)
6. [Tables](#6-tables)
7. [Contents (table of contents)](#7-contents)
8. [Icons, flags and logos](#8-icons-flags-and-logos)
9. [Data: sources, SQL and level of detail](#9-data)
10. [Period, parameters and filters](#10-period-parameters-and-filters)
11. [Formats and settings](#11-formats-and-settings)
12. [Themes and the theme studio](#12-themes)
13. [Templates, reports and permissions](#13-templates-reports-and-permissions)
14. [Export](#14-export)
15. [Keyboard shortcuts](#15-keyboard-shortcuts)
16. [AI assistant](#16-ai-assistant)
17. [Report API: other systems ask for reports](#17-report-api)
18. [Full test checklist](#18-full-test-checklist)

---

## 1. Getting started

### Signing in, the rail and the four areas

Quire opens with a **sign-in** screen. Each account has a login, a password and one or more **roles**.

The **rail** on the left is the one way around the app, on every screen:

- **top — the areas** your roles open: **Reports** (User role), **Design** (Design), **Data** (Data or Design), **Admin** (Admin).
  **Admin includes every other role.**
- **middle — what is open** (*Open*): the documents you work on — **up to 10 at once**, templates, reports and themes.
  Each shows as a small card — a report with its client's initials and name, a template or theme with its icon and
  name; the pointer on one shows its full name, client, period and whether it has unsaved changes. Click one to show
  it; going to an area keeps them all open. A dot marks unsaved changes; the × on a document closes
  it (asking first when it has unsaved changes). Opening an eleventh lets the oldest saved one go; when all ten have
  unsaved changes, save or close one first. A theme opened in the studio from a template stays linked with it.
  SQL workspaces open as **tabs inside the Data area** (*Overview · Demo bank · …*); the Data icon shows how many are
  open.
- **bottom** — the user guide, settings, and your account (change password, sign out).

The title bar shows where you are: an area's name, a workspace (*Data › Demo bank*), or the document's menus.

| Area | What it is for |
|---|---|
| **Reports** | Pick a report, then a client from *your* list (searchable, with each client's figures and what's coming up) and **Create report**; or open a saved report. |
| **Design** | New and example templates, and themes. |
| **Data** | Everything about data: all shared **connections** and **data sources**, and — with a template open — the sources and parameters that are **only in that template**. A list on the left, the chosen item on the right (details, a live preview, *Edit query*, *Settings*, which templates use it, a connection's tables, *SQL workspace*). |
| **Admin** | Users & roles, groups, an access overview, AI settings and the activity log — and an AI assistant for all of it. |

**Appearance.** The interface is light, dark or *as the system* — *Settings → Language & region → Appearance*, or the
account menu (your initials, bottom left). It changes at once and is remembered on this computer; reports keep their
own theme.

**Working with data.** Data sources are **made and changed in the Data area**:

- **Shared** data sources (and connections) belong to the Data area: every template can use them, and a change
  there reaches every template that uses them (each template keeps a copy, refreshed when it opens). Only the
  **Data** role creates, edits or deletes them; deleting lists what it affects — this template's widgets and the
  saved templates that use it (they keep their last copy).
- A template can also have sources (and connections) **of its own**, which no other template can reuse.
- **New data source** (Data area) asks where it lives — *Shared* or *Only in this template* — and what it reads.
- In a template, the left *Data* tab is an index: **Add** picks a shared source (*From the Data area*) or *Create a
  data source…*; clicking a source opens it in the Data area. Designers use shared sources as they are and run
  read-only SQL; changing values in the database (**Edit** in a workspace) needs the Data role.

**A data source's page.** The list has three colour-coded groups — **Connections** (slate), **Shared data sources**
(blue) and **Only in “template”** (violet); the page of the chosen one carries the same colour and a label saying
which it is. On it: **Edit query** (SQL sources — opens the query as a tab in the SQL workspace) or **Edit source**
(files, scripts, pasted data — where the data comes from); the **name**, **date field** and **values over time**
(*Add up* / *Snapshots*) change right there; **calculated fields**; where it is used; a live preview. Shared
sources are read-only without the Data role. The preview shows numbers and dates the way the SQL workspace does.

**Report parameters** (in a template's own group) are cards like the Variables tab: each parameter's tokens (`:name`
for SQL, `{{name}}` for texts — click to copy), its value now, where its choices come from, what it filters and what
report users can do with it; then the values that are always there (`:period_start`, `:period_end`, `:data_start`,
`:data_end`, with their dates) and the variables SQL can use.

**Previews and SQL need a client and a period.** With a report or template open they use its values; with nothing
open, the bar at the top of the Data area sets them (*Preview with* client, *from … to …*; it starts with this year
and the first client in your access, and follows the last document you opened). They fill `:client`,
`:period_start`, `:period_end` and the rest in previews and in the SQL workspace.

The Data area starts with four shared sources on the demo bank: *Клиенты и банкиры*, *Счета клиентов*, *Позиции на
дату* and *Операции*.

**Access** has two parts, set in *Admin*:

- **Clients** — *every client*, or *selected*: the clients of chosen bankers (**coverage**, `clients.sales_id`) plus
  clients ticked one by one.
- **Products** — *every product*, or *selected* source systems (`accounts.src_cd`: Bank, Brokerage, Asset
  management, Insurance). Only clients with such an account are listed, and **their reports show only those
  accounts**: an insurance-only user gets the policy pages and a portfolio made of the policies — brokerage, DU and
  bank pages are left out. (Report queries read it as `:access_src`.)

A preview shows exactly who an account sees. The report's client picker lists only those clients, a report can't be
switched to anyone else, and a report saved for another client opens on the first client in the list.
Administrators always see every client and product. **All of this is enforced by the app's core**, not only hidden in
the interface.

**Who gets which report.** A template reaches report users when it is **published**: in the template, *Access* →
**Who can use this report** — *Everyone with Reports*, or chosen **groups** and **people** — then **Publish to
Reports** (later **Publish changes**, or **Unpublish** to make it a draft again). It shows whom it reaches. Report
users see only the published reports given to them (and, in them, only their own clients and products); designers
and administrators see every report. The core refuses a report someone wasn't given.

### Your clients at a glance

The client list in **Reports** shows what a banker needs before a call. Everything comes from the bank's own tables
as of today (the last day with data on or before it), and only for the clients and products in your access. None of
it goes to the AI.

- **Each client:** the products they hold; the **risk profile** as a scale of four bars with its name (its own
  column, so it doesn't read as one more product); a line of the portfolio's value over 12 months; the value now;
  the **return this year** (by the report's rule: the cumulative return at the end over the one on 31 December, − 1);
  the deposits minus withdrawals this year; and when **you** last worked on a report for them (and last exported one).
- **Return vs peers:** this year's return minus the median of the other clients with the same risk profile (as the
  report works it out), in percentage points. **Fees this year:** what the client paid the bank.
- **Columns** picks which of these show (risk, the 12-month line, deposits and withdrawals, vs peers, fees, last
  report); the choice is remembered. Narrow windows drop the 12-month line and the extra columns first.
- **A note by the name** when something needs attention: the most pressing item, and how many more (**+2**). Red,
  with a red edge on the row: a large withdrawal. Amber: more in shares than the risk profile allows. Blue: money
  coming free (a deposit or bond maturing), idle cash, or a policy ending. Grey: a birthday or routine income. Click
  the note to open the details under the row: a card for each thing to act on (what, when, how much, which
  accounts, the instrument's rate and ISIN, the withdrawals one by one, and why it matters), and the coupons and
  dividends due as a table with their total. **⌘]** closes the details opened under the rows (again: opens them back).
- **Sort** by name, value, return, return vs peers, fees or last report (remembered). The header sums the clients' value.
- **Recent:** the clients you worked on last; a click creates a report for one.
- **Large books:** the list draws 100 clients at a time and more as you scroll (search and sort still cover all of
  them). The figures are kept for the day and worked out again when the bank's data changes.
- **Needs attention** (left): coupons, dividends and maturities (bonds and deposits) in the next 30 days, worked out
  from the client's holdings and the payment schedule (one item per payment, however many accounts hold it);
  insurance policies ending within 90 days; birthdays within 14 days; withdrawals in the last 30 days of at least 5%
  of the portfolio or 10 M ₽; **idle cash** of 10% of the portfolio or more (and at least 5 M ₽); and **shares above
  the risk profile's limit** (Conservative 40%, Moderate 60%, Moderately aggressive 80% — the bank's own limits
  belong here). A click opens that client's details.

### Batch reports

**Batch reports…** at the top of *Reports* makes one published report for many clients at once — the quarter-end run.

- Pick the **report**, the **period** (the report's own, this year so far, last quarter, last month, last year, or
  dates), **PDF or PowerPoint**, a **folder**, and the **clients** (all of yours by default; *All* / *None* / find).
  The choices are remembered for next time.
- Each report is made apart from your open documents — they stay as they are — and saved as
  "Client — Report — Period.pdf" (a file of that name is replaced). Only clients and products in your access.
- A panel in the corner shows the progress while you keep working (minimise it, or **Stop** after the current
  report). Each line: saved with its pages, or **pages need a look** (content cut off or past the page — open it
  in Quire to check), or the error. Open a file, or the report itself (for that client and period), from its line;
  **Show folder** when done.
- Each file is logged (Admin → Activity) and counts as the client's latest exported report in the client list.

### The banker's proposal

**Quire keeps every proposal** (the register), apart from the report file:
- **What's kept:** client, report, period, who made it and how (from notes, written up by the AI, or an AI draft),
  every earlier version, and its **status** — Draft → Presented → Accepted / Partly accepted / Declined, or
  Withdrawn — with the history of who changed it, when, and what the client said. Removing a proposal from a report
  keeps it in the register. Exporting a report with a draft proposal in it marks the proposal *Presented*.
- **In the client list:** a client with proposals shows a chip (the latest one's status); it opens the client's
  proposals under the row, with **Status…** to record the outcome. A proposal presented 14 days ago without an
  answer comes up in *Needs attention* as **Follow up on the proposal**.
- **In the proposal window:** the status of the one you opened, and the client's **earlier proposals** — **Start
  from it** puts its notes in the box.
- **The report's AI** knows the client's earlier proposals and how they ended (masked like the rest): the brief can
  recall them, and *Proposal ideas* won't suggest again what was declined.
- **Admin → Proposals** lists everyone's proposals (search, banker, status) for support: the full content, every
  earlier version, the status history, the report and file, the AI request behind it (**Open in Activity**), and
  **Copy for support** — everything as text for a ticket.

### The banker's proposal

In a report, **Add proposal** in the toolbar (or *Proposal ideas* in the AI pane) makes a page with your recommendations
to the client. It goes after the facts and before the legal text, in a section of its own (*Предложение*).

- **Several proposals:** a report may hold more than one, each with its own pages, one after another before the legal
  text. The toolbar button then reads **Proposal** / **Proposals · 2** and lists them: open one to change or remove
  it, or **New proposal…** for another. In the AI pane a draft goes in as **Add as a new proposal**, or **Replace…**
  one of those there are.
- **Long proposals:** a proposal that doesn't fit one page takes two — the actions first, then *Почему сейчас*,
  *Риски*, *Следующие шаги*. A long table of actions goes on over further pages (its header repeated) before them.
  Nothing is cut short.

- **Your notes:** type or paste them, or **upload** a Word (.docx), PowerPoint (.pptx) or text file (dropping it on
  the text box works too). Old .doc / .ppt files aren't read: save them as .docx / .pptx.
- **Suggest from the report** (in the window) — or **Proposal ideas** in the AI pane — has the AI draft a proposal
  from what the report shows: holdings outside the risk profile, deposits and bonds coming back, idle cash, a large
  position, weak results. Every action rests on the report's figures (with its page); instruments are only those in
  the report or a kind of product in general words — no invented securities, rates or returns. In the window, what
  you have typed is its starting point and the draft replaces the notes; in the pane, **Add as a page** puts it in
  the report and **Edit first…** opens it as notes. It needs client figures allowed, as the brief does.
- **Write it up with AI:** a clear page from your notes, tied to the report's figures (e.g. "доля акций — 62%,
  стр. 4"). Your actions, instruments and amounts stay as you wrote them; nothing is added. It needs client figures
  allowed (as the brief does); names and account numbers go as placeholders.
- **Lay out as written:** no AI. The first short line is the title, the first paragraph the lead, list items the
  actions (a verb first — *Купить*, *Продать*… — and an amount where there is one, then "— why"); lines
  *Риски:*, *Следующие шаги:*, *Почему:*, *Ожидаемый эффект:* start their blocks.
- **The page:** a title and a large lead with its main idea in bold; the expected effect on a green card; the
  actions as a table — buying, opening, adding in **green**, selling, reducing, closing in **red**; and *Почему
  сейчас*, *Риски*, *Следующие шаги* side by side, each with its colour and icon, every point opening with its key
  phrase in bold. A line at the bottom says it is for discussion and not individual investment advice.
- **Change or remove it:** the button reads **Proposal** once there is one; your notes are kept with the page.
  Undo (⌘Z) works as for any change.
- The **brief and pitch** present it after the results (a message of its own, the questions it may raise, and a
  mention in the e-mail); the **check** makes sure its figures agree with the report and flags promised returns —
  it doesn't judge the proposal itself.

### The Admin area

Admin has these parts, listed on its left:

| Part | What is there |
|---|---|
| **Users & roles** | The people: account, password, roles, client and product access, groups, reports, what AI may send for them and their own AI keys. |
| **Groups** | Groups of people: members, the reports given to each, and an AI level for the whole group. |
| **Access** | Everyone at a glance, one row per person: roles, clients, products, groups, the reports they get and what AI may send for them (and why). Tiles on top count who can sign in, administrators, who sees every client, who is restricted, who may send figures to AI and who still has the demo password. Click a person to open them in *Users & roles*. |
| **AI** | The bank's AI policy (on/off, *Structure only* or *Figures*, providers allowed), bank-wide keys, what AI may send person by person, and the use of the last 30 days by person and by task. |
| **Activity** | Work sessions (who worked on which report, for which client, how long, what they changed, exported and asked AI) and every event — see below. |
| **API** | *Web version only.* The systems that ask Quire for reports (a CRM, a client portal, a scheduler): their keys, clients and reports, what each report accepts from them, where their reports are copied — see [17. Report API](#17-report-api). |

![Admin → Access](img/56-admin-access.png)

**Activity** opens on **Sessions**. A session is one person working on one report or template **for one
client**: it starts when they open it and ends when they switch it to another client, close it, sign out or quit
the app (a client picked right after opening belongs to the same session). Each row shows when it started, who,
the client and the report, the **active time** (of the time it was open), what was done (edits, exports, AI
requests) and how it ended. Click one and it opens right under its row, across the page (click it again or press
**Esc** to close it, **↑ / ↓** to move to the next): a strip with the active time, changes, exports and AI, then:

- the report, the **template it came from**, the **period** and the other **parameters**;
- **active time** and how long it was open; saved or not; **exported** or not (PDF, PowerPoint, the brief's PDF);
  AI requests with the tokens sent and received;
- **what happened**, in order: opening, each change with its **before and after** (“title: Результат по
  продуктам → … · с начала года”), saves, exports, each AI request — *Show what was sent and received* opens it in
  place.

In **All events** a row opens the same way; an event that belongs to a session has *Open its session*.

**Administrators' sessions.** Time in the Admin area is a session too, marked *Admin* and called *Administration*:
what it changed — people (each with before and after), groups, who gets a report, the bank's AI policy and keys — the
Admin assistant's requests, and the active time there (looking at Activity counts). It ends when they sign out or quit,
or after 30 minutes away from Admin (shown as *away from Admin for 30 min*, ending at their last action); coming back
later starts a new one.

**Active time** counts half a minute at a time while the window is in front and someone typed, clicked, scrolled or
moved the mouse in the last two minutes, for the document in front only: a report left open over lunch doesn't
count. If the app is quit without saying so (a crash), the session keeps the time counted up to five minutes
before. **Export CSV** saves the sessions shown, one row each (minutes active and open, counts, parameters).

**All events** is the full log for finding out what happened and why. It records:

- **Sign-in** — sign-ins, failed ones too (with the login that was tried), sign-outs and password changes;
- **Admin** — users and groups added, changed (with *before* and *after*) or deleted, group membership, publishing
  and who gets a report;
- **Files** — new, open, save, export (with the file), close;
- **Edits** — every change to a document: what kind, which widget on which page and what each setting was and
  became (“Table” on page 19 · title: … → …), undo and redo, themes applied;
- **Sessions** — when a session started, its client changed and it ended;
- **Data** — SQL runs (the query and the number of rows), values written to a database, connections and data sources
  saved;
- **AI** — every AI request (who, which task, provider and model, tokens sent and received, time, result) and
  changes to the bank's AI policy and keys.

Filter by **words** (searches everything, SQL and errors included), **person**, **area**, **result** (*Failed only*)
and **time** (*Today*, *7 days*, *30 days* or two dates). Tiles count the events, people, failures, AI requests and
tokens. Click an event for everything about it: when, who (name and login), what it was about, the error if it
failed, the before/after of an account change, the SQL — and for an AI request what the person asked, **exactly
what was sent** (as the AI saw it) and **what came back**. **Export CSV** saves the events shown (for Excel, with
Cyrillic intact). Passwords and keys are never written to the log. It lives in the app's data folder
(`activity.jsonl`; the AI requests in `ai_log.jsonl`).

![Admin → Activity, an AI request](img/57-admin-activity.png)

In **Admin**, *Groups* lists the groups (Private bankers, Insurance desk, Analysts to start with): name, members and
the reports given to each. A person's page shows their **groups** (tick to join) and their **reports** — through a
group, to everyone, or given to them (tick to give or take away one directly).

**The report API** (answered by the web server): other systems ask Quire for reports with a key, set up in **Admin → API**. It has a
chapter of its own: [17. Report API](#17-report-api).

The **Data** role looks after every report's data without opening templates: with nothing open, the violet group in
the Data area has a report picker and lists that published report's own sources and parameters, read-only — preview
them and **Open query** in the SQL workspace.

**In the Data area** each group — Connections, Shared data sources, the open template's own and **each published
report** (a section per report) — folds with a click on its header; the choice is remembered. With the Data or
Design role, **+** on a report's section adds a data source to that report, and its sources can be edited there:
such changes are marked *changed in Data*, reach report users at once, appear in the template when its designer
opens it, and go out with the next publish (*Undo the change made here* reverts one).

**Python.** *Settings → Python* lists the Pythons on this computer (PATH, Homebrew, system, pyenv, conda, virtualenvs)
with their version and whether pandas is installed; pick one, keep *Automatic*, or browse to any interpreter (a
project's `.venv/bin/python`). A **Python data source** gets `params` and can read other data sources by name —
`sources["Позиции на дату"]` is a DataFrame — and returns a table. In the **SQL workspace**, switch **SQL | Python**:
Python runs on the last SQL result as `df` (printed text above, the result as a table below), and the same **Save** button
saves it as a Python source that reads the SQL source (saving the SQL as a source first when it isn't one) and runs
the script each time the report loads. In the lists, each source carries a tag for what it reads: **SQL**, **PY**,
**CSV**, **XLS** or **DATA** (pasted). Python is
written in the same editor as SQL, with highlighting and completion (⌃Space): `df.` lists DataFrame methods (and the
result's columns), `df['` the result's columns, `pd.` pandas, `np.` numpy, `sources["` the data sources, `params["`
the report's values, otherwise keywords, built-ins and your own variables; Tab indents four spaces, Enter after `:`
indents the block, ⌘/ comments lines with `#`. A **Python data source** is written in a Python tab of the Data area
(*New data source → Python script*, or *Edit source* on one): the other sources are listed on the left (click to
insert `sources["…"]`), **Run** shows what the source returns, **Save source** stores it. The workspace shortcuts
work in Python too (⇧⌘M maximize, ⌘[ ⌘] panes, ⌘S / ⌘O save and open a `.py` script).

**Deleting a data source**: at the bottom of its page in the Data area — *Delete from this template…*, *Delete from the
Data area…* (shared, Data role), or for a published report's source added in Data, *Delete this source* (one changed
there: *Undo the change made here*). A report's own original sources are removed by its designer, in the template.

**SQL dates.** In SQL, dates may be written `'20261231'::date`, `date '31.12.2026'`, `cast('31/12/2026' as
date)`, `'2026/12/31'::date` (and with a time); `current_date`, `today()`, `now()` and `current_timestamp` give
today's date and time. The SQL workspace shows timestamps with their time.

**Demo accounts** (password `quire`): `admin` (Alex Morgan, every role), `data` (Dana Fields), `design` (Iris Lane) and
`user` (Warren Buffington — a banker who sees his 6 clients). The sign-in screen offers them while they still have
the demo password. Accounts live in `users.json` in the app's data folder, with salted password hashes — never in
the bank's database.

### The Templates area

![Home](img/01-home.png)

The Templates area:

| Part | What it does |
|---|---|
| **Open… / New template** (top right) | Open a template (`.rtpl`) or report (`.rpt`); start an empty grid set up from your [settings](#11-formats-and-settings). |
| **Needs attention · Your templates** | What needs a look, then every template on one line with its theme, status and use; **Publish…** where it's due, then icons to open, set the audience, preview, duplicate and delete ([more](#the-design-areas-start-page)). |
| **Start from** | **Modern UHNWI report**: an 18-page private-banking report. **Open template** opens it for designing; **Preview as a report** opens it the way report users get it. **Blank template** starts an empty one. |
| **Themes** | Start a new theme, open one, or manage your theme library ([chapter 12](#12-themes)). |

The status line at the bottom shows whether Python is available for Python data sources.

**Test**

- [ ] Start the app: the sign-in screen. A wrong password says so; a demo account signs in with one click.
- [ ] `user`: the rail shows only **Reports**; 6 clients; **Create report** opens the report on that client, and the client picker lists only those 6.
- [ ] `data`: only **Data**; **SQL workspace** opens in the rail under *Open*; **Back** returns.
- [ ] `admin`: open the UHNWI template, go to **Data**, open a workspace: the rail shows *Template* and *Demo bank*; clicking *Template* returns to it unchanged.
- [ ] Data area with nothing open: *Demo bank* and 4 shared sources, each with a preview (no `:client` error); change the client in *Preview with*: the preview follows.
- [ ] Open the UHNWI template, Data tab → **Add** → *Позиции на дату*: it shows as *Shared* in the tab and *in template* in the Data area.
- [ ] Shared source → **Edit query**: a tab opens inside Data; **Save source** changes it in the Data area.
- [ ] Delete a shared source: the warning lists this template and other saved templates using it.
- [ ] As `design`: the rail shows Templates and Data; the Data area has no *Connect a database*; *Add a connection* lists only shared ones.
- [ ] Admin → Warren Buffington → Products → *Selected* → **Insurance**: 3 clients; as `user`, a report has only the policy pages.
- [ ] Admin can't take Admin from themselves or delete their own account; the last administrator stays.
- [ ] Admin → **Access**: one row per person with roles, clients, products, groups, reports and AI level; clicking a
  row opens that person in *Users & roles*.
- [ ] Sign in as `eve` with a wrong password, then as admin: *Activity* → *Failed only* shows the failed sign-in with
  *Wrong login or password.*
- [ ] Change a user's roles, save: *Activity* → *Admin* shows *Change user* with before and after.
- [ ] Edit a widget and run an SQL query: both appear under *Edits* and *Data*; the SQL is in the detail.
- [ ] *Export CSV* saves the events shown; Excel opens it with Cyrillic intact.
- [ ] As a banker, make a report for one client, change a widget's title, work a few minutes, switch the client,
  sign out. As admin, *Activity → Sessions* lists two sessions: the first with the client, period, the title's before and
  after, its active time and *switched to another client*; the second *signed out*.
- [ ] Leave a report open in the background for ten minutes: its active time doesn't grow.
- [ ] Templates area: Examples (UHNWI, Blank template) and Themes.
- [ ] **Open template** on the example: an 18-page template opens, the home screen is gone (nothing of it shows under the pages).
- [ ] **Home** button (house icon, top left): returns to the home screen; with unsaved changes it asks first.
- [ ] **Create a report**: the same example opens with a blue **REPORT** chip and the end-user panels.

---

### The Design area's start page

Three views: **Templates** (the main one, open by default), **Widgets** and **Themes**. Templates holds what a designer needs first:
- **Needs attention:** a published report whose sources the Data area changed (open its template to take the change in,
  then publish again), a draft nobody sees, a report published to nobody, or one nobody has used in 30 days.
- **Your templates:** every template, each on one line with its published report. Templates are saved in the
  **templates library** (a first *Save* puts them there; *Save as…* picks another place, and templates you opened from
  elsewhere are listed too). Each line shows the theme (click it to open the theme), the status (*Draft · not
  published*, *Published*, *Unpublished*, and *changed since* when the template changed after it was published),
  the version, how many people have it, and its use in the last 30 days. Published templates come first, then
  unpublished ones, drafts last. A draft has **Publish…** (a changed template **Publish changes…**); then
  icons: **open** (or click the name), **audience**, **preview** (the way report users get it), **duplicate** (a copy
  under a new name, in a theme you pick) and **delete** — always asked first, saying what goes (the file and, if
  published, its report and how many people lose it); it goes to the library's *.deleted* folder, recoverable. The copy is saved in the library straight away and starts as a draft.

**How publishing works.** A template is your working copy; report users never see it. **Publish** (in the template's
toolbar, or *Publish…* on its line) puts a copy in the Reports area for the people you choose: everyone with Reports,
or chosen groups and people. Report users then make reports from that copy. Changing the template doesn't change
what they get until you publish again (*Publish changes*): the version goes up and the line stops saying *changed
since*. Publishing also saves the template, so it stays linked to its report.
- **Start from:** the UHNWI example or a blank template. **Help** links to the guide's chapters.

**Widgets** is the widgets library as a table, one per kind of widget (KPI cards, tiles, tables, charts…). Each
widget appears **once**, whatever the themes: built-in ones and yours together. A row shows a preview on sample data,
the name and description, the source (*Built-in*, or *Yours* with its version and author), **the themes that include
it** (yours: any theme, as it takes the theme's colours and type) and **the templates that use it**. *Previews in*
picks the theme the previews are drawn in.

**Click a widget to work on it in the widget studio.** The widget is drawn large on sample data with the full
inspector; the pane on the left has *About* (name and description, for yours), *Preview size* (small, medium,
large, to see how it behaves), *Used by* (themes and templates) and saving: **Save widget** makes a new version of
yours, **Save as a new widget…** a copy, and on a built-in widget **Save as my widget…** keeps your version (the
built-in one stays as it is). The sample data and title never go into the widget.

**Themes** is a table: your library first, then the built-in themes, those in use first. Filter by *All*, *Your
library*, *Built-in* or *In use*. Each row shows:
- the theme's **colours**: paper, text and accent, the series colours, then gains and losses;
- its **type**: the heading font and the text font, each written in itself;
- its **cards & charts**: the card style, corner radius, and clean or soft charts;
- how many **design views** it has, and for how many kinds of widget;
- the **templates that use it**: the count (e.g. *14 templates · 3 published*) opens a list of all of them, published
  first, with a search box when there are many; click one to open it. **Show in Templates** opens the Templates tab
  filtered to that theme, with all the usual columns and actions; the chip above the list clears the filter.

**Open** opens the theme in the studio (double-clicking a row does too), and **Duplicate…** copies it into your
library. Your own themes can also be renamed and deleted there. *New theme* is at the top.

## 2. The workspace

![Workspace](img/02-workspace.png)

- **Title bar**: Home, the menus (File, Edit, Insert, Page, View, Theme, Help), the document's name and type
  (Template / Report / Theme), undo/redo, **Preview as user**, **Create report**, **Export**, **Save**.
- **Toolbar**: period and comparison, report parameters (e.g. *Клиент*), reload data, the fit summary, grid,
  zoom, panel toggles.
- **Left panel**: **Widgets** (drag or click to add), **Gallery** (widgets prepared for report users), **Data**
  (sources and parameters), **Pages**.
- **Canvas**: the pages. Each page header shows its number, title and fit status.
- **Inspector** (right): settings for what is selected, in tabs: *Content*, *Data*, *Design*, *Text*, *Layout*
  for a widget; *Document*, *Page*, *Theme*, *Access* when nothing is selected.

### Colours

Every colour field opens the same popover. **Theme colours** lists the theme's colours by the names its *Colours*
section gives them — *Accent*, *Text*, *Muted text*, *Positive*, *Negative*, *Page*, *Cards*, *Lines & grid*, *Card
outline* — then the **Series** colours, and **More theme colours** opens the rest (gain and loss tints, asset classes,
icon colours…). A colour picked by name **follows the theme**: change the theme's Accent and everything set to
*Accent* changes with it. Hover the field to see the name: "Accent (theme colour)". The themes work the same way:
where a theme uses one of its named colours (an accent line, a table header band, a share bar), it uses it by name,
so changing a colour in *Theme → Colours* changes everything drawn in it.

Below them, **Custom colours** starts with **Transparent** (what is behind shows through), then the colours this
template already uses, most used first — a colour picked once is a click away everywhere else. **Pick a colour** has
a shade square, a hue slider, a **hex** field and the **eyedropper**, which picks a colour anywhere on the screen,
another app or a brand page included (in the desktop app). A custom colour stays as it is whatever the theme does.
**Auto** / **None** clear the setting where it has a default.

### Menus

![File menu](img/03-menu-file.png) ![Insert menu](img/04-menu-insert.png) ![View menu](img/05-menu-view.png)

With one menu open, pointing at another opens it. **Insert** adds a widget to the current page; **Page** holds the
current page's actions and **Fit every page**; **View** toggles panels and the grid and zooms.

### Right-click menus

![Right-click on a page](img/06-rightclick-page.png) ![Right-click on a widget](img/07-rightclick-widget.png)

| Where | Menu |
|---|---|
| Empty spot on a page | **Add here**: KPI card, bar chart, table, text placed where you clicked; the page's actions; page settings |
| A widget | Edit text, duplicate, move to page, lock for end users, save to gallery, delete |
| Page title above a page, or a page in the Pages list | That page's actions |
| Left panel item | Widget types: add to the current page; gallery and data items: their own menus |
| Inspector | Find a setting, expand or collapse all sections |
| Text fields | Cut, copy, paste, select all |
| Around the pages | Undo/redo, zoom, grid, add a page, save, export |

Holding **⌥ Option** while right-clicking adds **Inspect element** (developer tools).

### Find a setting

![Inspector search](img/08-inspector-search.png)

The search box above the inspector's tabs (**⌘F**) searches every tab at once: setting names, section names and
the choices inside them ("dots" finds *Leader*, "roman" finds *Numbering*), with both spellings (colour/color,
centre/center). Results are live controls grouped by *Tab › Section*; click a group heading to open it in its tab.
**Esc** clears the search.

**Test**

- [ ] Each menu opens; moving the pointer along the menu bar switches menus.
- [ ] Right-click an empty part of a page → **Add here › Table**: a table appears at the clicked spot and is selected.
- [ ] Right-click a widget → **Duplicate**: a copy appears; **⌘Z** removes it.
- [ ] Right-click with ⌥ held: **Inspect element** is listed; without ⌥ it is not.
- [ ] Select a waterfall, *Design → Axes & grid → Grid colour* → **Accent**: the grid takes the accent and the field's tooltip reads *Accent (theme colour)*; the field stays a small swatch. In *Theme → Colours* change Accent: the grid follows.
- [ ] Waterfall with *Starts from a level* on → *Colours* → **Start** → *Accent*: the first bar takes the accent.
- [ ] Apply **Data Atlas**, select a number in the *Card* view: it sits on a light grey card with a hairline frame. In **Bold Blocks** the KPI cards stay flat colour blocks with no frame.
- [ ] Apply the **Изумруд** (Emerald) theme, then change its Accent: the table header bands, accent lines and share bars change with it; the header text stays white.
- [ ] Select a chart, press **⌘F**, type `legend`: only legend settings show, with "legend" highlighted. Change one: the chart redraws, the results stay. **Esc** brings the tabs back. Type `subtitle`: the *Widget subtitles* text role opens in the results.

---

## 3. Pages and layout

![Pages](img/10-pages.png)

Pages sit on a grid (columns × rows, set in *Page* setup). Widgets snap to it: drag to move, drag an edge or corner
to resize, use arrow keys to nudge (⇧ + arrows to resize).

**Two halves of a page** are 12 columns each, told apart by the gap between them — no divider needed. A vertical
divider can still be added: in the middle of a column of its own, or with *Place → In the gap* (Design) on the line
between two columns, where it takes no room and the widgets either side meet it.

**Several widgets at once:** ⇧-click more widgets on the same page to add them to the selection (⇧-click one again
to take it out; a plain click selects one). The first has a solid outline, the others a dashed one. The inspector
then shows **the settings they all have** — two column charts share nearly everything, a chart and a KPI card share
colours, numbers, the card and text styles. A setting they have set differently reads **Mixed** (an empty field,
a striped swatch or switch, a *Mixed* tag by its name); setting it gives them all that value. A change applies to all
of them in one step (one ⌘Z), and **Find a setting** searches the shared settings. Each keeps its own data, title and words; a design view or chart type goes only to widgets of the same
kind. Dragging any of them, the arrow keys, **⌘D** and **⌫** move, duplicate or delete them together; resizing gives
them all the new size.

- **Page settings** (double-click a page in the Pages list, right-click → *Page settings…*): title, **section**
  (groups pages in section tabs and the contents), **description** (shown under the page in the contents), cover page
  (no tabs or footer), page group and navigation label, an inset frame.

![Page settings](img/11-page-settings.png)

### Repeating pages — one set of pages per account (or any row)

A page, or all the pages of its **page group**, can repeat once for each row of a data source. The UHNWI report
uses it for its detail pages: every brokerage account (an IIS too), every asset-management account and every
insurance policy of the chosen client gets its own pages, while the bank accounts stay one portfolio. A client with
two brokerage accounts gets two sets; a client without asset management gets none — Quire works it out from the data.

**Set it up** — page menu (⋯ in the page header) → **Repeat for each…**, or *Page settings → Repeat → Set up…*:

1. **For each row of** — the data source with one row per copy (e.g. *Счета клиента*: the client's accounts).
2. **Only rows where** — optional: a column and the values to keep, ticked from the column's values (e.g. `kind` is
   `BROK`). Leave it on *every row* to repeat for all rows.
3. **Name each copy by** — the column that names a copy in the contents, the page navigation and its badge.
4. **Repeat the whole group together** — on for a page in a group: overview and holdings repeat as one set.

The dialog shows how many copies the current data gives and their names, and lists the row's fields with the first
copy's values — click one to copy `{{row.field}}`.

**Make each copy show its own data**:

- **Texts**: titles, headings, notes and page titles take `{{row.field}}` — e.g. the page title `{{row.title}}`, a
  badge `{{row.badge}}`. *Insert* in a text field lists *This page's row*.
- **Widgets**: in the widget's *Data* tab add a filter on the account column with the value `{{row.acct_cd}}` — the
  **{{row}}** button next to a filter value picks a field. Every copy then filters by its own row.

**On the canvas** the pages you design show the first row and carry a **Repeats ×N** badge (click to change the
setting). The copies follow them with a **Copy 2 of N · name** badge; they can't be edited themselves — change the
page they copy (*edit on page …* in the badge) and every copy follows. Tables that run long continue on extra pages
within each copy. Copies are numbered, listed in the contents and exported like any page; when no row matches, the
pages are left out. **Stop repeating** in the dialog turns it off.

**Show only when** (Page settings) leaves a single page out when its condition comes out empty or 0, e.g.
`{{data:port.has_bank}}` — for a page that either shows once or not at all.

- **Fit to page** (Document tab): *Auto-arrange* keeps the grid tidy after each change: closes gaps, makes room by
  shrinking flexible charts. A widget you move or resize by hand keeps its place and size — also when the page is
  tidied after later changes; if the page then overflows, the advisor offers to move what no longer fits.
  *Fill empty space* stretches widgets into leftover room (off for new reports). Report users find both in the
  report's panel too.
- **Fit advisor**: each page header shows *Fits*, *Fits · tip* or the number of problems. Click it for suggestions;
  hovering a suggestion previews it.

![Fit advisor](img/12-fit-advisor.png)

- **Cut off**: when a widget's drawing runs past its frame, it gets a dashed outline and a **Cut off · Fit** badge.
  **Fit** makes it as many rows taller as it needs. Charts first try to fit on their own: KPI values shrink a little
  (never below 75% of their size), legends tighten and fold the smallest rows into *Other*.

![Cut off](img/13-cut-off.png)

**Test**

- [ ] Page *Обзор портфеля*: the two halves are the same width, with no divider between them.
- [ ] ⇧-click a column chart and a KPI card: the inspector reads *2 widgets selected* and lists only shared sections. Set *Numbers → Decimals* on one first: with both selected it reads *Mixed*; set it: both take it. Set *Card & title → Fill*: both change; ⌘F `fill` finds it; **⌘Z** undoes both. Drag one: both move. **⌫**: both go.
- [ ] Drag a chart's bottom edge down on a full page: the chart keeps the new height; the widget below moves past the page end and the page header shows a problem with a *move to next page* suggestion.
- [ ] Drag the bottom edge past the page's last row: the outline stops at the page edge.
- [ ] Make the UHNWI hero figure (page 3) 2 rows tall: the value shrinks a little, the note is cut, the **Cut off · Fit** badge appears; **Fit** restores a height where everything shows.
- [ ] KPI cards can be made up to 12 rows tall.
- [ ] Page settings → Description: the text appears under the page's entry in the contents.

---

## 4. Widgets

| Widget | Use |
|---|---|
| KPI card | A headline value with change, sparkline, note or breakdown |
| Bar (columns, bars, 100% bar, range), line, area, donut, waterfall, treemap, tiles | Charts ([chapter 5](#5-charts)) |
| Table | Exact figures, groups, totals, bars, icons, best & worst ([chapter 6](#6-tables)) |
| Heading, text, comment | Titles, paragraphs, commentary (double-click to edit; `{{period}}`, `{{end}}`, `{{client}}`… placeholders) |
| Divider, image | Rules and logos |
| Contents | Table of contents ([chapter 7](#7-contents)) |

### The inspector's tabs

Every widget has the same tabs: **Content · Design · Text · Layout**. *Content* comes first and holds what the widget
says: for a chart, table, KPI or tiles, a **Title** section (title and subtitle) and then **Data** (source, fields,
filters); for a text, heading or note, its words; for a contents list, its entries. The title's look (bold, dim, off,
the icon) is under Design → *Card & title*.

### The Design tab: one order for every visual

The Design tab lists a visual's settings by the part they change, in the same order and under the same names for
every chart, KPI, tile and table. Only the parts that apply show:

| Part | What's there |
|---|---|
| **Type & view** | the chart type and the design view |
| **Layout** | how the marks are arranged and sized: stacked, bar width, donut hole, tile columns, treemap layout, a waterfall's total |
| **Series** | which values are drawn as lines over columns |
| **Value** | a KPI's main figure: size, alignment, colour, arrow, a second value beside it |
| **Colours** | every colour of the marks: by sign, up / down, one colour, and each series' or category's own colour |
| **Marks** | how marks are drawn: lighten, outline, corners, gradient; lines, markers and areas |
| **Numbers** | format, units, decimals, + sign, suffix |
| **Data labels** | labels on the marks: show, position, frame, colour and their own number format |
| **Centre** | what a donut shows in its hole |
| **Axes & grid** | value axis, ticks, minimum / maximum, gridlines, zero line, category labels |
| **List** | a ranked list's rows: rank numbers, lines, track, bar length |
| **Legend** | position and texts (a donut's and a 100% bar's amounts and shares) |
| **Note & breakdown**, **Change**, **Trend** | a KPI's note, comparison and sparkline |
| **Highlights** | best & worst (one name everywhere), callouts, a highlighted row |
| **Icons** | icons per category |
| **Card & title** | card, fill, outline, corners, padding, divider lines, title style and icon — and a KPI's *Colour the card* |

**Click a part to get its settings.** With a widget selected, the part under the pointer is outlined and named
(*Legend*, *Axis & grid*, *Data labels*, a series' or category's name, *Centre*, *Value*, *Title*…). Click it and the
Design tab opens at that part, unfolded and pointed out; a series or category opens *Colours* at its own colour. A
cell of a table opens its column's settings, and a row of a ranked list opens *List*.

**Series** (column, line and area charts). One card per series, in the chart's order:
- its **name** in the legend (empty: the value's own name), with a dot of its colour;
- **Show as** *Columns* or *Line* (column charts; at least one value stays as columns), and *Add a line* for another value;
- **Axis** *Left* or *Right* (with two or more series). A series on the right axis gets its own scale: the legend tags
  it *(RHS)*. Its minimum, maximum and units are under *Axes & grid*.
  Use a right axis for related measures on different scales, e.g. monthly results and the running total. The legend tags
  each series *(LHS)* or *(RHS)* (in Russian *(лев.)* / *(прав.)*) and shows whenever there is a right axis; the
  right axis's numbers are the colour of the left's unless *Right axis → Label colour* sets one.

**A part per series.** With two or more series, *Colours*, *Marks*, *Highlights* and *Data labels* each have what the
series share first (*All series*), then a part under each series' name, as *Axes & grid* has one per axis:
- **Colours**: *Colour by* its own colour or *Up / down*, and those colours;
- **Marks**: columns — *Lighten fills*, *Outline* (width, colour), *Fill* solid or fading to the axis; a line —
  *Width*, *Style*, *Shape* (straight, smooth), *Markers* (size, fill), *Area under*;
- **Highlights**: *Mark best & worst* with *Background bands* (columns side by side), and a *Callout* on the last or
  the largest value;
- **Data labels**: **Show** (all, last, min & max, none…), **Position** (*Next to axis* puts columns' labels in one row
  along the axis — under it for a rise, over it for a fall; *Above / Below* for a line), **Colour**,
  **Bold**, **Frame**, **Units**, **Decimals** and **Suffix** — e.g. a percentage series on the right axis in its own
  colour with one decimal, beside amounts in whole numbers. Their text style, shared, comes first.

A series with nothing set draws as the chart did. Stacked columns are labelled with their totals: one set of label
settings.

Clicking a series on the page opens its card.

**Text of each part.** A chart's axis labels, data labels and legend each have their own text style: a **Text** row
inside *Axes & grid*, *Data labels* and *Legend* (shown while that part is drawn), the same as *Axis labels*, *Data
labels* and *Legends* in the Text tab and in the theme's Typography. Each starts as the chart text (*Chart text (all
parts)*) at the size it has always had, so nothing moves until you set one.

**Spacing and lines.** *Axes & grid* sets the gridlines' colour and width and the zero line's colour; tiles have a
*Gap between tiles* and *Padding in a tile* (Layout); *Card & title* has *Space under title*.

**Padding** (*Card & title*) is the space between a widget's edge and its content: **S** (6 pt) for every widget unless
set, so content lines up across columns; *None* (the dashed-box icon), *M*, *L* or a number for any widget. A widget
without a card or title keeps its default S at its sides only; on the cover and other plain pages, none. **Title
align** (*Card & title*) puts the title (and the subtitle under it) on the left, in the centre or on the right.

A table adds its own parts after *Type & view*: Columns, Header row, Lines & rows, Totals & groups (the totals row
on or off and its look), Highlights and Rows & pages. Parts you've changed open by themselves and show a count.

### Putting a setting back

A setting changed on this widget shows its name in colour with a dot. **Double-click the name** to put it back to what
it follows: the design view, the theme, the palette, or (for report users) the report as designed. There are no reset
buttons. It works the same in the widget studio.

An empty number box shows the value in force in grey (e.g. 18); its arrows and ↑ / ↓ start from that value. The same works for each part of a text style (*Font*, *Size*, *Style*, *Spacing*, *Colour*), and
double-clicking a text style's name resets the whole style. *Reset* at the top of the Design tab still puts back
every change at once.

**Where a value comes from.** A setting that isn't set on the widget shows where its value comes from, when it isn't
the built-in default: a small **ring** before its name means its design view sets it, a small **square** the template's
chart defaults (*Theme → Chart defaults*); the pointer on the name says which. **Changed only** (at the top of the
Design tab, once anything is set) cuts the tab down to what this widget sets itself.

**Advanced settings…** (the widget's ⋯ menu) shows its own settings as JSON, to read, copy or change in one go;
*As drawn* shows everything merged (the view's, the theme's), read-only. What you type is checked against the same
schema the inspector is built from: a value of the wrong kind or outside its choices is an error and **Apply** waits;
a name no setting has is a warning. Apply is one undo step.

**Test**

- [ ] A chart on a design view: settings the view sets have a ring; set one here: it gets the dot; *Changed only*
  shows just that one.
- [ ] ⋯ → *Advanced settings…*: type `"xRotate": 30` in `opts.viz` → an error names the choices (0, 45, 90), Apply is
  off; `"xRotate": "45"` → Apply → the labels turn; ⌘Z puts it back.

### Your own widget types

Configure a widget the way you want it, then choose **Save as widget type…** in its ⋯ menu. Give it a name, a
description and an icon. The type keeps the widget's look and settings, plus the data, title and size new widgets
start with. It goes in the **widget types library**, so every template can use it: it appears under *Your widget
types* in the Widgets panel (click or drag it onto a page).

- **Widgets of a type follow it.** Settings you change on one widget apply to that widget only; **Update the type**
  (in Design → *Type & view*, or the ⋯ menu) passes them to the type, and every widget of the type follows.
  **Detach** keeps a widget's look but stops it following.
- **Each template keeps a copy** of the types it uses. Opening a template picks up the library's newer versions;
  report users get a change only when the template is published again. Deleting a type from the library leaves
  templates as they are.
- Changes to types are recorded in Admin → Activity.

### Composite widgets

**Insert → Composite** puts several parts in one card: a figure, a chart, a table, text, a heading, a line, an image.
They share one data source and its filters (the composite's *Content → Data*), so a fact card, an account summary or a
"figure + trend" panel is one widget on the page.

- **Parts** (Content tab) lists them; **Add a part** puts one in the first free cells of the composite's grid (the grid
  grows when it is full). *Layout → Grid inside* sets its columns and rows (12 × 6 to start).
- **With the composite selected, click a part** to select it: it has its full inspector (Content, Design, Text,
  Layout), and *The composite* at the top leads back. Drag it to move it, its handles to resize it, on the composite's
  grid; parts don't cover each other (the place shows red, and the part stays).
- A part reads the composite's data source unless it picks another; its own filters add to the shared ones.
- A composite can be **saved as a widget type**: widgets made from it get its parts, and *Update the type* passes new
  parts to every widget of the type.
- PDF draws it as on screen; in PowerPoint each part is an element of its own (text stays text, tables stay tables).

**Test**

- [ ] Insert → Composite on a page with data: a figure, a line and a line of text appear in one card.
- [ ] Select it, click the line: its inspector opens with *A part of …*; set *Highlights → Reference line → Average*:
  the line shows; drag the part onto the figure: red, and it stays where it was.
- [ ] *Add a part → Donut*: it goes into free cells (or new rows); remove it from *Parts*.

### Design views

Every widget type has **design views** in the theme (e.g. *Hero figure*, *Card with icon*). The design view
picker at the top of the Design tab (*Type & view*) switches a widget's look; settings changed below apply to that widget only
(the count of own changes shows, with **Reset** and **Use as default**).

![KPI design](img/14-kpi-design.png)

### Card & title

Card on/off, outline, fill, **corners** (*Corners (pt)* — this card's own rounding; empty follows the theme's *Corner
radius* in *Theme → Cards & charts*), **padding** (None, S, M, L or exact points; on a card at least 6 pt, so there it's S, M or L),
divider lines, and a **title icon** in a chip (circle, rounded, square or none) with its colours. Bars and tiles have
their own *Corner radius* under *Marks*.

![Card and frame](img/15-card-frame.png)

**KPI values** (Data tab): the first value is the **Main value** — the big number. Every other value says where it
shows: **Beside the main value**, **In the note line** (as `{1}`, `{2}`… in the note), **Chip at the right**, **Rows
under the value** or **Not shown**. The star makes another value the main one; references in the note follow it.
**Add value** works for KPIs too.

**KPI notes**: the **Change** and **Trend** sections say why they are empty when the data has no comparison period
or no dated series.

**Test**

- [ ] Select a KPI → Design → change the design view: the card's look changes, the data does not.
- [ ] Select a column chart, a donut, tiles and a KPI in turn: the Design tab's parts come in the same order and
  under the same names (Type & view, Layout, …, Card & title); each series' colour is under *Colours*.
- [ ] Select a column chart, then point at its legend: a dashed outline names it *Legend*; click: the Design tab opens
  at *Legend*. The same for the axis, the data labels, a donut's centre, a KPI's value and a table's cell (its column).
- [ ] Column chart or waterfall → Axes & grid → Text → size 11: only the axis labels grow; Data labels → Text →
  colour red: only the labels turn red; the legend keeps its look.
- [ ] Grid on → *Grid colour* and *Grid width* change the gridlines; tiles → *Gap between tiles* 14: the tiles move apart.
- [ ] A ranked list (e.g. "Доходность против ориентиров") → List → *Values at the right* off: the numbers go, the bars
  stay; *Bars fill the row* on: only the bars get longer; *Names in bold* and *Bar thickness* change just those.
- [ ] "Доходность за месяц и итог" (column + line) → Series → the line's card → *Axis: Right*: the line gets its own
  scale on the right (0–15%), the columns fill the height on the left (0–2%), and the legend tags the series *(LHS)* and *(RHS)*;
  *Line: Dashed* and *Markers: All* change only that line.
- [ ] A table → a text column with icons → *Wrap* on: long names break onto more lines at the column's width; the
  icons stay; no text runs into the next column.
- [ ] A KPI → ⋯ → *Save as widget type…* → name it: it appears under *Your widget types*; click it: a second
  widget of the type appears. Change the first (e.g. Align: Centre) → *Update the type*: the second follows.
- [ ] Select tiles and click one tile: *Colours* opens at that category's colour.
- [ ] KPI → Value → *Size*: the same number as Text → *KPI values* size; changing one changes the other.
- [ ] Padding → L: the content moves inward; → None (any widget, the dashed-box icon): content touches the frame. A new widget starts at S.
- [ ] A KPI with a title → *Card & title* → **Title align** → Centre, then Right: the title (and subtitle) move.
- [ ] A column chart with two series → Design → **Data labels**: *Text · all series*, then a part per series. Set one series' Show → None and the other's Colour → Text, Decimals → 2: each changes only its own labels.
- [ ] The same chart → **Colours**: the column series → *Colour by: Up / down*; **Marks**: the line → *Shape: Smooth*, *Area under: Flat*; **Highlights**: the columns → *Mark best & worst*: only that series changes each time.
- [ ] On a repeated page, select a chart and point at a series: the outline stays on this chart.
- [ ] Title icon → pick an icon: it shows before the title; chip → Circle: the chip becomes round.
- [ ] Text tab → KPI value size up: the value grows (it only shrinks if the card is too small, and never below 75%).
- [ ] UHNWI hero (page 3) → Data: value 2 reads *Beside the main value*, value 3 *In the note line*; set value 3 to *Chip at the right*: the return moves into a chip.

---

## 5. Charts

![Chart design](img/16-chart-design.png)

The Design tab lists only the settings the chart can show, in the order above: layout, series, colours, marks,
numbers, data labels, axes & grid, legend, highlights, icons, card & title.

**Axes & grid** has headings: **Value axis** (or **Left axis** and **Right axis** once a series is on the right),
**Category axis** and **Grid**. Each value axis has its own *Show*, *Label colour*, *Ticks* (how many) or *Step* (a tick
every so much, e.g. 0.05 for every 5%), *Signed ticks*, *Minimum* and *Maximum*; the right axis also has *Units*, and
its label colour is the series' own unless set. 

**Category axis**: *Every Nth label* (2 shows every other one; empty: every label that fits), *Turn labels* (level,
45° or 90°, for long names) and, for dates, *Dates as* — Jan, Jan 26, Jan 2026, 01.26, 01.2026 or 2026 (quarters and
days follow the same choice).

**Legend**: *Swatch* (rounded, square, dot or line), *Swatch size* and *Order* (as the data, reversed, by name).

**Highlights → Reference line**: a line at a value (a target) or at the first series' average, median, highest or
lowest, with its *Label* (`{value}` is its value), colour and style. **Band**: a range shaded behind the marks (*From*,
*To*), with a label and colour. Both are on the left axis, which reaches them.

**Axes & grid**: gridlines run *Horizontal*, *Vertical* or *Both*, *Solid*, *Dashed* or *Dotted*. Vertical lines run
**between** the bars of a column chart or a waterfall (they part the categories), and **through** the points of a line
or area chart. *Signed ticks (+5%)* shows only when the axis is in percent, the one place a sign on the ticks means
something.

**Data labels** have their own **colour** — *Series colour* (each label in its bar's, line's or slice's colour),
*Up / down*, *Text* or *Muted* — and their own **format**: *Label units*, *Label decimals* and *Label suffix* (each "as
values" unless set), on every chart type (a donut's legend amounts too). To keep "млн ₽" out of every number, set
*Numbers → Units* to millions and put "млн ₽" in the subtitle: the axis and labels then read 7,4.

**Best & worst** (bars, waterfalls, tiles, bar lists): by default *Gains & losses only* — the best is marked only if
it gained (above 0) and the worst only if it lost (below 0); a zero is neither, and when everything gained there is
no worst. *Always highest & lowest* marks the extremes whatever their sign. **If empty, show** (Data tab) puts your
own words where a widget has no rows, e.g. "Убыточных позиций за период нет".

- **Chart type** switches between number (KPI card), table, columns, bars, 100% bar, range, line, area, donut, treemap,
  tiles and waterfall, keeping
  the data. Ranked bars hide settings they don't use (corners, labels, legend).

![Chart type](img/17-chart-type.png)

- **Colours**: per series or per category, by sign (up/down), or one colour per category you pick.
  A waterfall has **Up**, **Down**, **Total** and, when it *Starts from a level*, **Start** (the first bar; the
  theme's muted colour unless set).
- **Lighten fills** and **Outline** (with its width and colour) are on every chart with filled marks: bars, columns,
  waterfalls, donuts and the 100% bar. A treemap has **Fill strength**, an area chart **Area opacity**.
- **Top N** (Data section, beside **Sort**) keeps the first N rows; **As in the data** keeps the source's order.
- **Icons** (donut, treemap, 100% bar, tiles): **Show icons** turns them all off; an icon per category,
  **Flags & logos → Find them by name** to find flags and logos automatically, and the chip shape.

![Chart icons](img/18-chart-icons.png)

**Test**

- [ ] Change chart type Columns → Bars → Donut: the data stays, the look changes.
- [ ] Ranked bars: *Data labels*, *Legend*, corner radius are not offered.
- [ ] Waterfall: *Colour by* is not offered (steps are always coloured by direction).
- [ ] Fact cards are KPI cards on data: "Крупнейший класс" (page 4) has Group by = class and shows the top category
  with its share; Categories → *Shows* → Shares of all gives "81 · 15 · …". "Структура" (page 8) shows shares,
  "Пиковый месяц" (forecast) the top month and its value on one line, "Покупки" (trades) "24 · 52,8 млн".
- [ ] Any column chart → Design → **Series**: turn a value into a line, or **Add a line** from the data.
- [ ] A table with best/worst marked as *Row* and 2–3 rows each side: 1st place has the strongest fill, 2nd and 3rd
  lighter (*Fade by place*).
- [ ] Every icon chip (chart icons, title icon, table column icons, Contents) has the same settings: **Chip** shape
  (circle, rounded, square, none), **Chip background** (Auto / None / a colour), **Icon colour** and **Chip outline**
  with its width and colour. Flags and logos get the outline too. On charts the fields show what is drawn: the
  background's automatic tint (transparent on tiles), and the outline **on** in a dimmed icon colour — switching it
  off removes it.
- [ ] An icon's chip colour → **None**: no chip behind the icon (Auto brings the default tint back). Chart icons with
  Chip = None show flags without a frame.
- [ ] Marks → **Outline** on: a visible outline; *Outline width* and *Outline colour* change it.
- [ ] Table → Design → Columns → **Add column**: pick a field (numbers as a total, text as it is); it appears at the
  end. The bin on a column removes it (the eye only hides it); the last column can't be removed.
- [ ] Bars → Layout **List** is drawn by the table engine. "Результат по классам активов" (page 7) → Chart type
  **Table**: it looks the same and shows "This table is a ranked bar list… Show as a bar chart"; click it: back to
  the same list and design view. A table column's bar has the same *Bar length* (from zero / by size) and
  *Track divisions* options as the list.
- [ ] "Результат по классам активов" (page 7) is a bar chart (Bars, design view "Результат по классам"): name with a
  best/worst mark, a track by size, the result with "return · value" below. Ranked list → turn off *Best & worst marks*,
  *Other values below* or *Track fills the row*: each changes it.
- [ ] "Портфель против ориентиров" (page 6) → Chart type → **Number**, then **Table**, then **Bars**: it comes back as
  ranked bars. A KPI card (no Group by) offers Number and Table; the charts are greyed out with the reason.
- [ ] "Позиция по доходности" (page 6) is a bar chart: Chart type shows **Range** selected. *Range* places each
  category as a point on one scale (benchmarks, a peer group); switch it to Columns and back to Range — the data stays.
- [ ] A 100% bar ("По валютам", page 3) → Icons → turn **Flags & logos** off: the flags disappear; on: they return.
- [ ] Set *Units* to millions: the *Format* setting hides; values show in millions.

---

**Treemap**: every tile's area is its share of the value and the tiles fill the box with thin gaps (**Gap**, 2 pt).
**Layout**: *Tiles* (squarified) or *Largest left, rest tiled* — the largest a full-height column, the others bands
across the rest, each as high as its share (a dominant one leaves them room for a name and a value). **Group below
(%)** folds the smallest categories into one “Other · N” tile. **Colour by**: category, result (green / red, stronger
the larger the result) or value (one colour, darker the larger). What a tile writes follows its size.

## 6. Tables

![Table design](img/21-table-design.png)

- **Columns**: drag to reorder, the eye hides a column, click a column to open its settings in the inspector
  (arrows step to the next column; **‹** goes back to the table).
- **Column widths**: each column in the list has a width box — a % of the table, empty for automatic. Or select
  the table on the page and drag the borders between its columns (a tip shows the two widths); double-click a
  border, or **Automatic widths**, to go back. Numbers and rank pills never get narrower than their values, and
  text keeps its icon and a few letters.
- **Header row**, **Lines & rows** (row height presets Compact / Normal / Roomy or exact points), **Totals &
  groups** (subtotals, group rows), **Rows & pages** (continue onto new pages).
- **Long tables continue** on pages Quire adds and removes by itself (*Rows & pages → Continue on next pages*, on by
  default). A continuing table keeps the row height you give it: make the rows taller (*Row height* presets, or exact points in *Rows (pt)*; *Header row (pt)* is the header's height) and the rows
  that no longer fit carry on to the next page; *Fill height* only stretches rows, it never squeezes them below that
  height. With continuing off, rows tighten (down to about one and a half lines) before any are cut.
- Pages: the pieces follow the rows, and the totals
  show on the piece where the rows end. In a grouped table a subheader never stays alone at the bottom of a page —
  it moves on with its rows — and when a group's rows run onto the next page its subheader is repeated on top, marked
  *(продолжение)* / *(continued)*. When a table continues and there is free space under it, the badge
  *Continues on the next page* offers **Use free space**: the table grows into it, and the extra page goes away
  once every row fits. A continuation page you designed yourself (with its own cards, notes or charts) is left out
  of the report — not numbered, not exported — while the table ends before it, e.g. for an account whose
  holdings fit on one page.

**A widget's name.** *Layout → Name* gives a widget a name for the editor only — it is never printed. The canvas
label, the inspector, lists such as *Keep with the end of* and the fit messages use it. Left empty, a widget goes by
its title, its text's first line, or — untitled figures — its type and data source (*Table · Банковские счета*);
the ↶ button next to the field goes back to that.

**Keeping widgets with the end of a table.** A total, a chart or a note under a table can be *kept with the end of*
it (the widget's **Linked to a table** section, or **Keep the N below with its end** on the table's *Continues on the
next page* badge). While the table continues onto more pages, the linked widgets go right after its last rows (at
the same distance) and the table takes their place on its own page; at least one row always continues, so they never
stand alone. When the table fits, nothing moves. On the canvas the moved originals show striped, the placed copies
dashed — click a copy to edit the original.

Linked widgets move with the table's bottom edge: drag the table taller and they move down with it; drag it back
and they come back. When they would run past the page, the table's last row goes on to the next page and they go
after it there, so nothing is cut off.

**Highlight rules on a hidden column.** A rule can sit on a column that is hidden and still mark the row: in the
UHNWI template, the bank table's hidden column *Дней до закрытия* (days until a deposit ends) has *When between 0
and 30 → Whole row, Stripe, Label “{v} дней” in the column “Срок · закрытие”*, and the brokerage holdings' *Дней до
погашения* does the same for bonds. Change the numbers to change the window; the label's column and colours are in
the rule.

**Cell padding (pt)** (*Lines & rows*): the space at a cell's sides, 5 pt unless set; PowerPoint tables take it too.

### A column's settings

![Column shown as a bar](img/23-column-bar.png) ![Text column with icons](img/24-column-icons.png)

Only settings that can show are offered:

- **Number columns**: *Show as* **Number**, **Bar** or **Both**. A bar has colour, negative colour, thickness,
  round ends and a background track. Numbers have format, decimals, before/after text, plus sign, zero as dash, and
  colour by value (sign colours, arrows, colour scale, icon sets).
- **Text columns**: *Show as* text or pills; a **Marker** — an icon per value (picked, or **Flags & logos** found by name for flags and
  logos) or a series-colour swatch.
- **Highlights & rules**: colour the cell, row, stripe or pill, put an icon after the row's name, add a label or a
  note, when a value meets a condition.
- **Column**: width, header group, fill, band, divider, visibility in group rows.

### Best & worst

**Show as Facts** (table → Layout): each row becomes a fact — its name small and upper-case on top, its value large
below — 1 to 4 across, with hairlines between. Made for key → value lists such as a policy's terms or a portfolio's
"about"; *Value size* sets how large the values are.

![Best and worst](img/22-best-worst.png)

Marks the best and worst rows by any value (shown or hidden): pick the column it ranks by, lower-is-better, how many
rows, and whether to show an **icon** after the name, a tinted **row**, or **both**. **Add another ranking** marks
by a second value (e.g. return *and* result): icons say which value (arrows for returns, coins for amounts), colour
says best or worst. A row hit by several rankings shows every icon; only one fill can show, so the first ranking
set to *Row* or *Both* decides it. **Mark** *Gains & losses* (default) marks a best row only if it gained and a worst
row only if it lost (*Lower is better* flips what counts as a gain); *Highest & lowest* marks them whatever their sign.

![Holdings](img/25-holdings-best-worst.png)

**Test**

- [ ] UHNWI page 3 table → Design → Best & worst: turned on, ranked by *Доходн.*; Депозиты shows ↗, Металлы · ОМС shows ↘.
- [ ] Add another ranking by *Результат*: Депозиты shows two icons; *Best label* "лучший результат" appears under the name.
- [ ] Holdings (page 8): the best holding row is green with ↗, the two worst are red with ↘.
- [ ] Click *Доля* → Show as **Number**: the percentage shows; **Bar**: the number options disappear, bar options show.
- [ ] Bar thickness Thin/Thick and Round ends change the bars on the page.
- [ ] Header CAPS off: header labels are no longer in capitals.

---

## 7. Contents

![Contents design](img/26-contents-design.png)

The Contents widget lists sections and pages with page numbers that always follow the pages.

- **Design**: layout **List** or **Chapters** (a block per section with number, icon and name); show sections and
  pages, sections only or pages only; 1–3 columns; spacing; page numbers with dotted, solid or no leader;
  descriptions; lines between; section numbering (I, II / 1, 2 / A, B), a label such as "Chapter", colour; icon
  chips and an icon per section.
- **Content → Entries**: titles from **Page titles** or each page's main **Heading** (so the contents follows the
  pages as they are edited); descriptions from page notes or heading subtitles; pages *in a section* or *all*.
- **Pages in the contents**: every entry (click to edit its page, the eye leaves it out) and a **Left out** list
  (cover page, no section, hidden) with **+** to add it back.

![Contents entries](img/27-contents-entries.png)

- **Fill in from the report…** proposes missing sections (from the page before), titles (from the page heading) and
  descriptions (heading subtitle or the first sentence of the page's text); review, edit and apply in one step.

![Contents assistant](img/28-contents-assistant.png)

The contents always fits its box: spacing tightens first, then descriptions are dropped if needed.

**Test**

- [ ] Layout List ↔ Chapters on the UHNWI contents (page 2): the look switches.
- [ ] Hide a page (eye): it moves to *Left out · hidden*; **+** brings it back.
- [ ] Rename a page's main heading, with *Titles from: Headings*: the contents entry changes too.
- [ ] Clear a page's section and title it "Page 5", then **Fill in…**: the row is proposed with the previous section and the heading as title; **Apply** fixes it; one **⌘Z** undoes it all.

---

## 8. Icons, flags and logos

![Icon picker](img/19-icon-picker.png) ![Flags](img/20-icon-flags.png)

The icon picker (everywhere an icon is chosen) has:

- **Icons**: the report set and 1,539 line icons in categories (Finance, Business & charts, People, Places & real
  estate, Transport, Energy & industry, Health, Technology, Time, Status & arrows, Documents) — or *All*.
- **Flags**: 79 countries plus the EU.
- **Brands**: 68 company marks (payments, crypto, tech, carmakers, consumer brands).
- **Companies**: 50 issuer logos for securities — Russian blue chips (Сбербанк, Газпром, Лукойл, ВТБ, Норникель,
  Яндекс, Т-Технологии…), US large caps and ETFs (Apple, Microsoft, SPDR, iShares…) and precious metals. With
  *Flags & logos* on, a column finds them from a security's name or ticker: "Сбербанк" and "SBER-20280425-13" both get
  Sber's logo, and a government bond named by its country ("RU-20290723-6.9", "US-…") gets the country's flag.
  In a table column (*Marker → Icon*), **Size** sets how large the icons are against the text: 50–140%.
- **Letters**: a badge of 1–8 letters instead of an icon — type any (B, S, IN, OUT, SELL…) or pick one; a single
  letter makes a round badge, more a rounded one.
- **My logos**: SVG or PNG files you upload; they travel inside the document.
- **Search** across all of them: names, keywords, countries, currency codes.

**A table's word column, value by value.** Select the table, click the column (or *Design → Columns*), then:
*Display → Show as* text or **pills**; *Icons → Marker* none, an icon or a swatch (with the chip's shape, outline,
size and *Flags & logos*), and *Text → Marker only* to show the marker without the value's text; and **Each value**: every value the column can take — from the whole data source, so values
not in this period's report are there too (marked *not in the data now*) — with its pill colours (background and
text) and its marker, an icon or letters, with the chip's and the mark's colours as drawn. A value with nothing of its
own takes the column's (*Other values*). Choosing *Marker → Icon* sets the usual words at once: *Покупка* → B on
green, *Продажа* → S on red, *Ввод* → IN, *Вывод* → OUT.

Every icon can sit in a **circle**, **rounded** or **square** chip, or none.

**Flags & logos → Find them by name** (table markers, chart icons) finds a flag or logo from the value itself: a country name, currency code or
currency name in English or Russian — "AED", "UAE dirham", "Дирхам ОАЭ" all give the UAE flag; "Canadian dollar" the
Canadian, "Доллара США" the US flag. A value with an icon picked for it keeps that icon.

*Line icons: Lucide (ISC licence). Flags: flag-icons (MIT). Brand marks: Simple Icons (CC0). Company logos: TradingView symbol logos, Russian Railways from Wikimedia Commons
— for demonstration only; license them before shipping. Brands and logos are trademarks of
their owners — use them only where you have the right to.*

**Test**

- [ ] Open any icon picker → search `dirham`: the UAE and Morocco flags show.
- [ ] Brands tab → pick Visa: the logo shows in its colour chip.
- [ ] **Upload logo…** with an SVG: it appears under My logos and is selected; save, reopen the file: it is still there.
- [ ] Add a currency "UAE dirham" to a table with *Flags & logos* on: it gets the UAE flag without any setup.
- [ ] Treemap "По классам активов" (page 3) → Icons → give the small tiles (Фонды, Облигации…) flags: they show in
  the compact tiles too.
- [ ] "Позиция по доходности" (Key figures, "Клиенты, похожие на вас"): label left, "впереди 88% клиентов" right; min and
  max close the scale with МИН/МАКС; ticks every 1%, a zero line, the median grey and "Вы" a large green dot.
  Turn off Scale → *Axis ticks*, *Zero line*, *Lowest & highest at the ends*, or set Markers → Rings: each changes it.
- [ ] "По валютам" (page 3) → Icons: Доллар США and Юань are folded into "Прочее", and the panel says their icons
  can't show until the chart has more room.
- [ ] Contents (page 2) → click a section's icon on the page: the inspector opens on Design → Icons with that
  section's picker open; pick another icon and it changes on the page. With Layout = List there are no icons and the
  hint says so.

---

## 9. Data

![Data tab](img/29-data-tab.png)

### Database connections and the SQL workspace

**Connect a database** (top of the Data area, the **+** on *Connections*, or the last choice under *New data
source*) adds a database: a **DuckDB** or **SQLite** file, or a **PostgreSQL** or **ClickHouse** server.
**Test connection** lists its tables. Queries in the editor only read data; values change only through **Edit**
in the SQL workspace, when you press **Save** (not on ClickHouse, which is read-only here).

- **Who adds them:** shared connections are made by the **Data** role and every template can read from them. A
  designer with a template open can connect a database **only for that template**.
- **A connection's page** shows its type, where it points (passwords hidden), whether it is shared, the data sources
  on it and its tables with their row counts. **Edit connection** (at the top) changes its name, type and address —
  *Test connection* checks it before saving; the **…** menu has the SQL workspace, *Remove from this template* and
  *Delete from the Data area…*.
- **ClickHouse** is reached over its HTTP interface: `clickhouse://user:password@host:8123/database`, or
  `clickhouses://…:8443/…` for TLS (`http://` and `https://` work too; settings can follow `?`, e.g.
  `?session_timezone=Europe/Moscow`). The user and password go as headers, never in the address the server logs.
  Tables come with the row counts ClickHouse keeps; `:client`, `:period_start` and the other parameters are filled
  in as everywhere. Dates and times arrive as dates (with their time in the workspace), numbers as numbers.

**Test**

- [ ] Data area → **New data source** right after opening the area: its page opens (also before the lists have loaded).
- [ ] **Connect a database** → ClickHouse → `clickhouses://explorer@play.clickhouse.com:443/default` → *Test connection*:
  "Connected: … tables"; **Connect**: it is listed with *ClickHouse*, its page shows the tables with row counts.
- [ ] On a connection's page, **Edit connection** opens it filled in; change the name → *Save*: the list shows the new name.

Click a connection to open its **SQL workspace**. It opens as a view in the main window, not a dialog: the
title bar shows **Report** and a tab per open workspace — click one, or press **⌃Tab**, to switch between the
report and the SQL. Each workspace keeps its query, results and edits while you work on the report; × on its tab
closes it. The tables pane, the editor and the results are separated by **splitters**: drag to resize, double-click
to reset. The two pane buttons in the toolbar show or hide the tables pane and the row / column pane.

- **Parameters** (the tab next to *Tables*) lists every `:name` a query can use with the value it takes now —
  `:data_start`, `:data_end`, `:period_start`, `:period_end` and the report's parameters such as `:client`. Click or drag
  one into the query, or type `:` in the editor to pick from the same list. Values follow the report's period and
  parameters as you change them. **Variables** from the Variables tab are listed too and work as `:name` in any
  SQL (the workspace and data sources) when their name is a plain word such as `min_value`; text variables have
  their placeholders filled in, SQL and data variables give their computed value.
- **`:data_start` / `:data_end` vs `:period_start` / `:period_end`**: `:period_start`/`:period_end` are exactly the
  period in the toolbar — use them when a query computes the period's figures itself (returns, operations in the
  period). `:data_start`/`:data_end` are the dates of data to load: the period plus what a comparison or history
  (trailing months, sparklines) needs — use them to load a source with a date column, which Quire then slices.
  Without comparisons or history they are equal. *Help → Parameters* in the workspace explains it with examples.
  (Templates made earlier keep working: the old `:start`/`:end` mean the same as before.)
- **Shortcuts**: *Help → SQL editor shortcuts* (also in the workspace's Help menu, ⇧⌘/). In the workspace ⌘[ and
  ⌘] show or hide the tables pane and the row / column pane, as they do for the report's panels; ⇧⌘M maximizes
  the editor.

- **Tables** on the left with their row counts; click one to show its columns and types. *Find* searches tables and
  columns. **Drag** a table or column into the editor to insert its name where you drop it (double-click inserts it
  at the cursor). **Right-click** a table for ready queries — `select *` (first rows), every column, count rows,
  profile columns — or a column for its distinct values or min/max/nulls. If the editor already has a query, Quire
  asks first: **Replace**, **Insert at cursor** or cancel.
- **The editor** colours SQL (keywords, functions, strings, numbers, parameters, comments, your tables and columns)
  and numbers its lines. Typing offers **completions** from the connection: tables after `from`/`join`, the columns
  of tables in the query, `alias.` → that table's columns, functions and keywords (⌃Space opens the list; ↑↓,
  Enter or Tab, Esc). Enter keeps the indent; Tab / ⇧Tab indent lines; ⌘/ comments lines.
- **Run** (⌘↵) runs the query — or only the **selection** when text is selected (the button says *Run
  selection*). **Rows** sets how many rows the preview shows (100 – 10 000); the status shows the total, the
  columns and the time. An error shows the database's own message.
- **Results** are coloured by type: numbers blue, dates green, text as is, true/false purple, nulls grey. Click a
  column header for its figures in the right pane, computed by the database over the whole result: rows, filled,
  empty, distinct, min, max, and for numbers sum, average, median and standard deviation, plus the most frequent
  values. Click a **row** to see its values in the right pane, column name next to value (⇧-click selects a range,
  ⌘-click adds rows, ↑↓ step to the next row).
- **Copy as a table**: click **#** in the header (or ⌘A in the results) to select every row, then ⌘C — or
  right-click → *Copy all rows as a table*. Paste into Excel, Numbers, Word or an e-mail and it arrives as a table
  with its column names (the rows shown; raise **Rows** for more).
- **Edit data**: run a query on one table (`select … from accounts where …`, no joins, groups or windows) and press
  **Edit**. Double-click a cell — or change a value in the row pane — Enter keeps it, Esc drops it, Tab moves to
  the next column; right-click → *Set to null*. Changed values are tinted until you press **Save**, which writes
  them to the database in one go (nothing is saved if any value is refused), or **Revert**. Quire asks before
  running another query or closing with unsaved changes. Columns computed in the query can't be edited.
- **Toolbar and menus** (File, Edit, Run, View): undo/redo (⌘Z, ⇧⌘Z), **Format SQL** (⇧⌘F, also on right-click;
  formats the selection or everything, keywords in lower or UPPER case), open and **save scripts as .sql** (⌘O, ⌘S,
  ⇧⌘S), **maximize the editor** (hides the tables and results; the next Run brings the results back), show or hide
  the tables and column-figures panes, and **Fonts…** for the editor and the results (kept on this computer).
- `:data_start`, `:data_end`, `:period_start`, `:period_end` and report parameters (`:client`…) are filled in with the
  report's current values.
- **Add as data source** saves the last query that ran as a source (pick its date column). It runs each time the
  report loads; edit it later from its ⋯ menu — it reopens in the workspace.
- **AI assistant** (⌘I): writes, explains and fixes queries, turns a query into a report source and answers
  questions about the database — see [AI assistant](#16-ai-assistant).

The demo bank database (`data/demo.duckdb`, see `data/README.md`) has clients, accounts, instruments, daily
balances and returns, operations, payment calendars, prices, FX and benchmarks: **Connect → Use the demo bank
database**.

**Test**

- [ ] Connect → *Use the demo bank database* → Test connection: “Connected: 11 tables”.
- [ ] Right-click `portf_acct` → *select \** → its first rows; the editor had text, so Quire asked first.
- [ ] Drag `clients` after `from`, type `c.` after an alias: the list shows the table's columns.
- [ ] Select one statement of two and press ⌘↵: only the selection runs.
- [ ] Click a number column's header: the right pane shows its sum, average and most frequent values.
- [ ] *Format SQL* on a one-line query: one clause per line, the select list one per line.
- [ ] Run a query with a CTE and `rank() over (…)` and `:data_start`; then *Add as data source*: it appears in Data
  sources as “SQL on Demo bank” and previews its rows.
- [ ] Open the workspace, press ⌃Tab: the report; ⌃Tab again: the same query and results.
- [ ] Parameters tab: `:period_start` 2026-01-01, `:client` the chosen client; type `:cl` in the editor → `:client`.
- [ ] Drag the splitters; hide both panes with the toolbar buttons.
- [ ] Click a row: its values appear on the right. Click **#**, ⌘C, paste into a spreadsheet: a table with headers.
- [ ] On a copy of a database: `select * from accounts` → **Edit** → change a name → **Save** → run again: the new
  name is there. Change another, **Revert**: it's back.
- [ ] `delete from clients` → refused: queries typed in the editor only read data (changes go through **Edit**).

### Sources

CSV, Excel, SQLite, PostgreSQL, ClickHouse, DuckDB (files or a database), Python scripts, pasted data, and **SQL over other
sources**. Each source has a date field (for periods), an optional *snapshot* flag (balances that don't add up over
time) and **Shape with SQL**: calculated fields and a transform.

![Dataset editor](img/30-dataset-editor.png)

### SQL over other sources

![SQL dataset](img/43-sql-dataset.png)

A dataset that is one DuckDB query over the other sources by name: CTEs, joins, window functions. Report values are
filled in: `:period_start`, `:period_end` (the period), `:prev_start`, `:prev_end` (the comparison), `:data_start`, `:data_end` (the data loaded) and `:name` for every
parameter (a multi-choice parameter becomes a list: `client in (:client)`). The same placeholders work in transforms,
calculated fields and widget SQL filters. It runs sandboxed, without file or network access.

### Calculated fields and level of detail

![Level of detail](img/42-lod-builder.png)

A calculated field is any SQL expression over the columns. The **Level of detail** builder writes window-function SQL
(visible and editable):

| Result | SQL shape | Use in a widget |
|---|---|---|
| The group's value on every row (Tableau **FIXED**) | `sum(x) OVER (PARTITION BY region)` | *Maximum* or *Average* |
| Once per group (Tableau **INCLUDE**) | value on the group's first row only | *Average* = average across groups |
| Each row's share of its group | `x / sum(x) OVER (…)` | *Sum*, as a percent |

Rows can be all rows, the report period, the comparison period or year to date. *Fixed by* none = the whole dataset.

**Test**

- [ ] Sales sample → Data → Sales → Add calculated field → Level of detail: Sum of revenue, fixed by region, report period → **Use this SQL** → **Test**: shows sample values; add the field.
- [ ] A KPI with *Maximum* of that field and region = North shows North's revenue for the period; change the period: it changes.
- [ ] Add source → **SQL over other sources** with a CTE and a join → **Load preview**: rows show; use it in a table.
- [ ] Use `:period_start` in a transform; change the period: the result changes.

---

## 10. Period, parameters and filters

### Variables

The **Variables** tab (left panel) lists every placeholder you can use in text — period (`{{period}}`, `{{start}}`,
`{{end}}`, `{{compare}}`, `{{today}}`), document (`{{title}}`…), page (`{{page}}`, `{{pages}}`, `{{section}}`,
headers and footers only), each parameter — with **its value now** and **how it is filled**. Click a token to copy it.

**My variables → Add** makes your own `{{name}}`, filled from **Text** (which may hold other variables, e.g. "as of
{{end}}"), an **SQL query** (its first cell; `:period_start`, `:period_end`, parameters work) or a **Data value**
(`Source.field|agg`), formatted as a number, currency, percent or date. **Test** shows the value before you save. Your
variables also appear in the *insert a value* menu of text fields.

Built-in variables are read-only here: each shows a lock and where it is set (*Set in the toolbar · period*,
*Set in Document → Title*, *Filled automatically*…). Click one to go there — the toolbar control flashes, or the
inspector opens on the field.

Tokens are **coloured by where their value comes from**, here and in every hint across the app: violet — my
variables, blue — period and dates (also `:period_start`/`:data_start`… in SQL), green — document settings, amber — parameters,
grey — page numbers, teal — data and SQL. The legend sits at the top of the tab.

### Period

The toolbar's period (month, quarter, year, year to date, custom range) and comparison (previous period, same period
last year, none) apply to every widget. `{{period}}`, `{{start}}`, `{{end}}`, `{{compare}}` work in any text.

### Parameters

![Parameter editor](img/41-parameter-editor.png)

Data → Report parameters → **Edit**. A parameter has a label and name, a type (choice list, text, number, date) and:

- **Choices**: typed in, or **from the data** (a source and column: its distinct values, always up to date).
- **Several at once**: report users tick any number of values.
- **Filters column**: the parameter filters every source that has that column — no per-widget setup. Below it,
  **Filters these sources** lists every source: **Auto** (a column with that name), another column (when a source
  names it differently, e.g. `acct`), or **Not filtered**. Sources left unfiltered are counted in a warning.
- The filter applies **before anything reads the data**: SQL datasets and their joins, `{{data:…}}` and `{{sql:…}}`
  values, variables and level-of-detail fields all see only the chosen values. The parameter's own choices still list
  every value.
- **Default**.

A parameter also works as `{{name}}` in text and widget filters and `:name` in SQL.

### What report users do

![Parameters applied](img/39-parameters-applied.png) ![Multi-select](img/40-parameter-multiselect.png)

Pick values in the toolbar (search, tick several, **All**, **Apply**): the whole report redraws. The reset button
next to the parameters returns them to their defaults.

In a report the inspector is kept simple, without tabs:

- **Nothing selected:** **Theme** lists the themes that have every look this report uses (switching keeps each widget's
  look), and the report's title, subtitle and author. **Theme** in the menu bar does the same.
- **A widget selected:** **Look** (the theme's prepared looks for it, one click each; long lists show five and
  **More looks**), then:
  - **Fit**: **Title** Default (as the template draws it), Bold (the theme’s title: capitals, bold), Dim (smaller, in grey) or Off (its text is
    kept); **Spacing** (None, S, M, L) and **Text size** (Small, Normal, Large). Designers find **Title** under Card &
    frame.
  - **Numbers**: full (1 234 567) or short (1,2 M), and decimals. Offered only where the widget shows amounts of
    1 000 or more; on a table it sets every number column.
  - **Chart**: values on the chart and the legend on or off. **Table**: the totals row and the header row.
  - **Colours**, **Text** (title, subtitle, or the text itself) and **Time & rows** (**Rows**: how many of the best or
    worst a list shows — 3, 5, 8…; the table's own order decides which).
- **Chart type** shows every chart as a picture: one click draws the same data as a number, a table, columns, bars, a
  donut… Types the data can't draw are greyed out.
- **Table**: the totals row, the header row and every column can be shown or hidden (one always stays).
- **Best & worst** (tables): mark the best and the worst rows — **Ranked by** any value (return, result, price
  change…), **Best** and **Worst** 0–3, **Mark with** an icon, a tinted row or both; **Add another ranking** puts a
  second one beside it (e.g. by return and by result). Icons, colours and rules are the designer's.
- KPI cards: **Colour by result**, **Card colour** (plain, tinted, solid) and **Align**; donuts: **Centre shows**.
- **Replace…** puts another figure of the report in its place; **Delete** removes it.
- **Add** (left panel): a **Note to the client**, text, a heading, a line or a picture, and every figure of the report
  in groups by page. The page being edited comes first and open (**This page**); the others fold. A figure of an
  account's page (brokerage, asset management…) goes only on that kind of page, and on every account's copy of it.
- Notes and text are written with a small toolbar: **B**, *I*, the theme's colours, smaller and larger words and icons
  (▲ ▼ ↗ ↘ ✓ ✕ ⚠ ★ …). **Arrows** puts a green ▲ before +6,48% and a red ▼ before −2,1% by itself. A **Signed note**
  shows it is from you — your initials, name, the date, tinted in the theme's accent; switch it off for plain text.
  The note's colour (**Accent**) changes only its bar and the initials; the name and the words keep their own colours.
- Double-click a note or a text block (or **Edit the note…** in the panel) for its window: the words with bold, italic,
  colours, sizes and icons (question, important, warning, comment, ✓ ✕ ▲ ▼ ↗ ↘ ★…); for a note its **Background**
  (Neutral — the theme's card, Warm, Mint, Sky, Lavender, White), its **Mark** (a badge on the initials: question,
  important, warning, comment), **Signed by you** and **Arrows on numbers**. ⌘↵ or **Save** keeps it, Esc cancels.
- What is offered follows the template's **What end users can do** (Change look, Edit text, Remove widgets…).

- [ ] Sign in as `user`, create a report, select a donut: Look, Colours, Text, Time & rows; no tabs.
- [ ] With nothing selected, pick **Alpine**: the report re-themes and **All pages fit** stays.
- [ ] Replace… → Bars: the same data as bars. ⌘Z undoes it.
- [ ] Pick another Look: it applies (no "doesn't allow" message). Hide the title, set Spacing None and Text size Small:
  the widget redraws each time; export keeps it.
- [ ] On a table, turn off Totals row and Header row; on a chart, Values on the chart and Legend.

### Which filters report users get

Document → Access → **Filters for report users**: for the period, the comparison and each parameter choose **Can
change**, **Show only** (visible, fixed at the template's value) or **Hidden**. The report's toolbar follows it, and
the engine refuses changes to fixed filters.

**Test**

- [ ] Sales sample → edit parameters → Region: choices *From the data* (Sales · region), several at once, filters column *region*.
- [ ] Toolbar → Region → tick North and South → Apply: KPIs drop (revenue 681 412 → 363 734), the subtitle says "Region: North, South".
- [ ] **All**: totals return. Reset button: defaults return.
- [ ] Add a parameter *Account* (choices from the data, filters column `account`); for a source that calls it `acct`, pick `acct`. Choose one account: every widget, SQL dataset and `{{sql:…}}` value shows only that account; sources marked *Not filtered* stay whole.
- [ ] Report mode: report users can change the period and parameters.
- [ ] Access → Period *Show only*, Region *Hidden* → Preview as user: the period shows but can't be changed, Region is gone.
- [ ] Variables tab → click `{{end}}`: the toolbar period flashes; click `{{title}}`: Document opens with Title focused.
- [ ] Variables tab → Add → name `north_revenue`, SQL `select sum(revenue) from sales where region = 'North' and date between :period_start and :period_end`, format Currency → Test: shows a value; add it; type `{{north_revenue}}` in a text: the value shows; change the period: it changes.

---


### Adding a filter for report users (example: products)

A filter is a **report parameter**: *Data → Report parameters → New parameter*. For a product filter:

1. **Choices** — *From the data*: a source listing the values (in the UHNWI template, *Продукты клиента*: the client's
   products), its **Column** (`src_cd`, what queries get) and **Shown as** (`product`: what users see — *Брокерские
   счета* for `BROK`). Tick **Several at once** to let users pick more than one; the default *All* means no filter.
2. **Make the data follow it.** A source follows a parameter in one of three ways:
   - it has the column, and the parameter's **Filters column** names it (rows are filtered automatically);
   - its SQL uses it — for a parameter with *All* and several values:
     `and ('All' in (:product) or src_cd in (:product))`;
   - a Python source reads `params["product"]`, or reads a source that follows it.
   Put the condition where the data is first read (in the UHNWI template, the step that reads the client's accounts),
   and every total, chart, account page and section built on it follows.
3. **Check the reach.** The parameter's card in *Data → Report parameters* says *followed by all N sources* — or lists
   the sources that don't follow it, with the widgets (and pages) that would show unfiltered data. For a source that
   shouldn't follow it (e.g. the client's name), set it to *Not filtered* in the parameter's settings: it is then
   listed as *left out on purpose*.

Pages built from the data follow too: repeated account pages repeat only for the accounts left, and pages shown only
when there is data (e.g. the bank pages) drop out. A report user's own access (clients and products set in Admin)
always applies on top: the choices list only the products they may see.

## 11. Formats and settings

### App settings (⌘,)

![Settings: language](img/36-settings-region.png)
![Settings: numbers](img/37-settings-numbers.png)
![Settings: new documents](img/38-settings-new.png)

- **Language & region**: the language and currency symbol new documents use (any symbol or code, e.g. `AED`).
- **Numbers & dates**: date format, thousands and decimal separators, negatives as `−1 234` or `(1 234)`, currency
  symbol and its position — with a live sample.
- **New documents**: author, organisation, page size and orientation, period and comparison.

- **App language**: English or Русский for menus, panels and messages (reports keep their own language). The AI
  assistant answers in this language too.
- **AI assistant**: providers, API keys and models — see [AI assistant](#16-ai-assistant).

**Save** applies at once: every document follows these formats, except for choices it sets itself (Document →
Formats). A thousands separator that would clash with the decimal mark switches the decimal mark (1,000 → 0.5).

### Document formats

![Document formats](img/32-document-formats.png)

Document → **Formats**: the same choices for this document only. Choices left on **Follow app settings** take yours;
set one here to fix it for this document on any computer. **Follow app settings** clears the document's own choices.

**Test**

- [ ] Settings → Numbers: thousands `1,234,567` → Save: the open UHNWI report changes at once ("354,266,000", decimals become "2.34%").
- [ ] Settings → Numbers: negatives *(1 234)*, currency `AED` after → Save: KPIs read "… AED", negatives show in brackets.
- [ ] Settings → Language & region → App language *Русский* → Save: menus, panels, tooltips and messages are in Russian; the report itself is unchanged. Back to English: everything returns.
- [ ] New template: it starts with your author, organisation, page size and formats.
- [ ] Reopen the saved document on another machine (or after clearing settings): formats stay.

---

## 12. Themes

Each theme's looks are distinct: looks that differed only by a setting a user can switch (no header, colour by
result, a card, a legend) were folded into one look plus that setting. Templates made with the older looks open with
each widget on the kept look and the difference as its own setting, so they draw exactly as before.


![Theme menu](img/31-theme-menu.png)

A theme holds colours, fonts, chart defaults and the **design views** of every widget type. **Theme** in the menu
bar applies a theme to the template or opens it in the **theme studio**.

![Theme studio](img/44-theme-studio-overview.png)

The studio shows the theme's colours and type, then one page per widget type with every design view on sample data.

![KPI views](img/45-theme-studio-kpi.png) ![A view selected](img/46-design-view-selected.png)

- Select a view to edit it in the inspector (Design, Text, About); star it to make it the default; duplicate, rename or
  delete from its menu. **New design view** starts from any existing one or blank.
- **Contents** has its own page with List and Chapters views.

![Studio contents](img/47-theme-studio-contents.png)

- **Charts** tab: the chart preset (*Clean* / *Soft*) and defaults for all charts.
- **Back to template** applies the theme to the template (or returns without applying). **Save theme** adds it to
  your library.

![Theme library](img/48-theme-library.png)

**Manage themes…**: rename, duplicate, delete your themes; duplicate a built-in one to change it.

**Test**

- [ ] Theme → Edit in the theme studio → KPI cards → select a view → change its value size → Back to template → Apply: every KPI using that view changes.
- [ ] New design view from an existing one: it appears in the list; in the template the Design view picker offers it.
- [ ] Manage themes → duplicate a built-in theme → rename → delete: the list follows.

---

## 13. Templates, reports and permissions

- **Preview as user** shows the template as report users will see it.

![Preview as user](img/34-preview-as-user.png)

- **Create report** starts a report (`.rpt`) from the template: a snapshot report users fill in.
- **Access** (Document → Access): what report users may do — add from the gallery, remove, move and resize, restyle,
  edit text, comment, add and remove pages. Widgets can be locked individually (right-click → *Lock for end users*).
- **Gallery**: widgets saved for report users (right-click a widget → *Save to gallery…*).

![Access](img/33-access.png)

**Test**

- [ ] Access → turn off *Remove widgets* → Create report → select a widget: no Delete.
- [ ] Lock a heading → in the report it can't be moved or edited.
- [ ] Save a widget to the gallery → in the report, Gallery tab → add it.

---

## 14. Export

![Export](img/35-export.png)

- **PDF** (⌘E): vector pages with selectable text and embedded fonts.
- **PowerPoint**: one slide per page; text and simple tables stay editable; tables with pills or bars export as exact
  pictures (choose per table under *Rows & pages → PowerPoint*).

Pages with layout problems ask before exporting.

**Test**

- [ ] Export PDF of the UHNWI example: 18 pages, text selectable, flags and icons sharp at any zoom.
- [ ] Export PowerPoint: one slide per page; a plain table is editable in PowerPoint.

---

## 15. Keyboard shortcuts

![Shortcuts](img/09-shortcuts.png)

| Keys | Action |
|---|---|
| ⌘S / ⇧⌘S | Save / Save as |
| ⌘O | Open |
| ⌘E | Export PDF |
| ⌘Z / ⇧⌘Z | Undo / redo |
| ⌘D | Duplicate the selected widget |
| ⌫ | Delete the selected widget |
| ↵ | Edit text |
| Esc | Deselect, clear the search |
| ← → ↑ ↓ / ⇧ + arrows | Move / resize by a grid cell |
| ⌘F | Find a setting |
| ⌘, | Settings |
| ⌘+ / ⌘− / ⌘0 | Zoom in / out / fit width |
| ⌘[ / ⌘] | The left part of where you are / the inspector. In a document: the left panel. In the SQL workspace: the tables pane. On Admin: its menu folds to icons; on Data: the list of connections and sources; on Reports: the report picker (⌘[) and the details opened under the clients (⌘]). The sidebar always stays |
| ⌘I | AI assistant: open or close |

---

## 16. AI assistant

AI lives in one place for every role: the **AI** button at the far right of the top bar, or **⌘I**. It opens the
assistant of the screen you are on; pressed again, it closes it. Where your role has no assistant yet, the button
shows what AI offers to each of your roles today and what is coming, with the provider and model that would answer.

![The AI button where a role has no assistant yet](img/53-ai-button.png)

Today three roles have their assistant: **Data** (the SQL assistant), **Design** (the design assistant) and
**Reports** (the report assistant) and **Admin** (the administrator's assistant), all described below.

### What leaves the bank

Both assistants send **structure**, never data. Restyling sends a picture you chose and the theme's look. The design assistant sends the template's page grid, its
sources' names with their field names and types, the widget types, the theme's design views and — when changing a
page — the widgets on it (type, place, title, which fields they show). The SQL assistant sends the **structure** of
the database: table and column names, their types, the names of the
report parameters (`:client`, `:period_end`…) and what each means. It never sends rows, parameter values or row
counts. The app runs each query the model writes on your computer; if the database refuses it, only the
database's error message goes back, and the model gets up to three tries.

One thing does go as written: **the SQL you ask about**. *Explain*, *Fix* and *Make it a report source* send the
editor's query, so a value typed into it (`where client_id = 'C1337'`) is sent too.

Every request is written to the AI log on this computer: who asked, the provider and model, which key, what for,
the tokens sent and received, whether it worked and how long it took — and the request itself: what the person
asked, the text that was sent (masked as the AI saw it) and the answer. Administrators read it in *Admin → Activity* (filter *AI*).

### Was it helpful? (👍 / 👎)

Under every answer of every assistant — SQL, design, report and Admin — are **👍** and **👎**. 👍 says it helped; 👎
asks what went wrong (*Wrong or made up*, *Not what I asked*, *Not useful*, *Too slow* — any of them) and lets you add a
line about what you expected. Pressing the same thumb again takes the vote back; the latest vote counts. Votes are
tied to the exact request, so administrators can open it and see what was asked, sent and received.

Administrators find them in **Admin → AI → Feedback** (helpful share by task and by person, and the latest 👎 with their
reasons and comments — click one to open its request), in **Activity → All events** (*Result → AI: not helpful 👎*, and
a 👍/👎 mark on each rated request), in the session it happened in, and through the Admin assistant (“which AI
answers didn't help this week, and why?”).

**Copy for Claude.** Every AI request opened in Admin (Activity, a session, the 👎 list) has *Copy for Claude*: it copies,
as Markdown, the assistant and task, who and when, provider, model and tokens, the person's vote with reasons and
comment, what they asked, exactly what was sent (each try) and what came back, the source files behind that assistant,
and what to do — ready to paste to Claude (Claude Code) to find and fix why it didn't help. What was sent is masked as
the AI saw it, so no client names go with it.

### Setting it up

**Settings (⌘,) → AI assistant.**

![Settings: AI assistant](img/52-ai-settings.png)

| Provider | Key from | Default model | Notes |
|---|---|---|---|
| Claude | console.anthropic.com | `claude-sonnet-5-5` | |
| ChatGPT | platform.openai.com | `gpt-4.1` | |
| DeepSeek | platform.deepseek.com | `deepseek-chat` | |
| Qwen | Alibaba Cloud Model Studio (DashScope) | `qwen-plus` | |
| GigaChat | developers.sber.ru | `GigaChat-2-Max` | Paste the **Authorization key** (Base64); pick the **Scope**: Personal, Business or Corporate. The Russian root certificates it needs are built in. |

1. Paste a key and press **Save key**. Keys go to the **macOS Keychain** and are never shown again: the field
   then says *saved — paste to replace*; **Remove** deletes it.
2. **Test** connects with the key and lists the models the provider offers; pick one in **Model** (or type a
   name). **Endpoint** changes the address, for a proxy or the bank's own gateway.
3. **Assistants use**: *The first provider with a key*, or one provider by name. The chip at the top of the AI
   pane switches between the providers you have keys for at any time.

**Administrators** set the bank's AI in **Admin → AI** (Settings → AI assistant links there; so does the report
assistant's *Open Settings* button):

![Admin → AI](img/58-admin-ai.png)


- **AI assistants on** — off switches AI off for everyone; the AI pane then says so.
- **Providers allowed** — e.g. only GigaChat. A provider that isn't allowed is marked *not allowed by the bank*
  and is never used, even with a personal key.
- **Data that may be sent** — the bank's default: *Structure only* (the default) or *Figures* (what a report shows).
  Saved as soon as you change it. The SQL and design assistants always send structure only; the report assistant
  needs *Figures*. At either level, **names and numbers of clients, accounts and instruments never leave**: they go
  as placeholders ([CLIENT], [ID-3], [INS-2]) — new for every request — and are put back on this computer.
  Per person or per group: *Admin → Users → a user → AI*, or *Admin → Groups → a group → AI*. A person's own setting
  comes first; otherwise the widest of their groups'; otherwise the bank's. The user's page shows which applies now
  and why (*Now: Figures (set for the group “Private bankers”)*).
- **Bank-wide keys** — used by anyone who has no key of their own. A person's own key always comes first.
- **A person's own keys** — *Admin → Users → a user → AI keys*: set or remove anyone's own key for each provider
  (it is saved at once and never shown). People can still set their own in Settings. Deleting a user removes their
  keys too.
- **AI requests** — opens *Admin → Activity* at the AI requests: who, which task, provider and model, tokens sent
  and received, time and result. Click one to see **what the person asked**, **exactly what was sent** (as the AI saw
  it, with the placeholders) — each try when it took several — and **what came back**.

**Structure only or Figures?** *Structure only* is right for the bank as a whole and for data and design people:
the SQL assistant, the design assistant and restyling need nothing more. Give *Figures* to the people who prepare
client meetings — private bankers, usually as a group — because the report assistant (brief & pitch, check, notes)
works with what a report shows. Names and IDs stay masked either way.

### The SQL assistant

Open any SQL workspace — **Data → a connection** (its SQL), or **a source → Edit query** — and press **⌘I** or the
**AI** button. In the Data area ⌘I also works without a workspace open: with a **source selected** it opens that
source's query with the assistant; with a **connection selected**, or nothing, it opens a new query.

The assistant is a pane on the right of the workspace. Its header shows **which AI answers** (provider and model —
click to switch), **history** (earlier conversations on this connection, kept on this computer) and **+** for a new
conversation. Answers come in the app's language (Settings → Language & region → App language); SQL comes formatted,
one clause per line.

![The SQL assistant](img/49-ai-pane.png)

**Ask for data.** Describe what you need in plain words. The assistant writes the query, the app runs it, and the
answer shows the query, what it returns and whether it runs (*Runs · 3 columns · 230 rows*). Then:

- **Use** puts it in the editor and runs it;
- **Insert** puts it at the cursor;
- the copy button copies it.

Nothing reaches the editor until you choose. **Follow-ups refine the last query**: “only clients with more than 100
million in total” changes the query just written instead of starting again.

*Example* — “Value of each client’s portfolio at the end of the period, by asset class”:

```
select
  c.name as client,
  i.instr_type as asset_class,
  sum(b.amt_rub) as value_rub
from balances b
join clients c on c.id = b.client_id
join instruments i on i.instr_cd = b.instr_cd
where b.report_dt = :period_end
group by 1, 2
order by 1, 3 desc
```

Report parameters are used by name, never as values: the query follows the period and client chosen in the
toolbar.

**Ask about the database.** A question rather than a request for data gets an answer in words, naming the tables
and columns and how they join — nothing runs.

*Example* — “Where are FX rates kept, and how do they join to positions?” → *Rates are in `fx_rates` (`report_dt`,
`cur`, `rate` — roubles per unit). Positions in `balances` carry their currency in `price_cur`: join on
`f.cur = b.price_cur and f.report_dt = b.report_dt`. `balances` already holds the day’s rate in `fx_rate` and the
value in roubles in `amt_rub`.*

![A question about the database](img/50-ai-question.png)

**Explain the editor’s query.** Says what the query returns, then how, in plain words — useful for a query someone
else wrote.

**Fix the last error.** When a run fails, the error box has **Fix with AI**, and the pane's *Fix the last error*
becomes available. The assistant gets the query and the database's message, writes a corrected query and runs it.

*Example* — `select client, sum(amt_rub) from balances group by client` fails with *Referenced column "client" not
found*. The fix joins `clients` for the name:

```
select
  c.name as client,
  sum(b.amt_rub) as value_rub
from balances b
join clients c on c.id = b.client_id
group by 1
```

**Make it a report source.** Turns a query written for one case into a source a report can reuse. Literal values
that a report varies — dates, client and account ids, currencies — become the matching report parameters. Values
that are part of the logic stay. Every column gets a clear name. The answer lists **Now parameters** and **Kept as
they are**, suggests a name and the date column, and runs the result. **Save as source…** puts it in the editor,
runs it and opens *Add as data source* with that name and date column filled in.

*Example* — before:

```
select b.acct_cd, b.instr_cd, b.amt_rub
from balances b
join accounts a on a.acct_cd = b.acct_cd
where b.client_id = 'C1337'
  and b.report_dt = '2026-06-30'
  and a.acct_type = 'Брокерский счёт'
```

After — *Now parameters:* `'C1337' → :client`, `'2026-06-30' → :period_end`; *Kept as they are:* `'Брокерский
счёт'` (the account type is what this source is about); suggested name *Brokerage positions at period end*,
dated by `value_date`:

```
select
  b.acct_cd as account,
  b.report_dt as value_date,
  b.instr_cd as instrument,
  b.amt_rub as value_rub
from balances b
join accounts a on a.acct_cd = b.acct_cd
where b.client_id = :client
  and b.report_dt = :period_end
  and a.acct_type = 'Брокерский счёт'
```

![Make it a report source](img/51-ai-source.png)

**Keys in the pane.** ↵ sends, ⇧↵ starts a new line. ⌘ shortcuts work from the question box as anywhere in the
workspace: ⌘I closes the pane, ⌘[ and ⌘] show or hide the side panes.

**Good questions** name the measure, the level and the time: “monthly deposits and withdrawals per client for the
period”, “top 10 holdings by value with their share of the portfolio”, “accounts opened in the period, by type”.
The assistant knows only names and types, so when a column's meaning is ambiguous, say what you mean (“`amt` is in
the instrument's currency, use `amt_rub`”).

**Test**

- [ ] Settings → AI assistant: paste a key, **Save key**, **Test**: “Connected (your key) · N models”.
- [ ] Data → select *Demo bank* → ⌘I: a new query opens with the assistant; the chip shows the provider and model.
- [ ] Ask “value of each client’s portfolio at the end of the period, by asset class”: formatted SQL with
  `:period_end`, *Runs · 3 columns*; **Use** fills the editor and runs it.
- [ ] Follow up “only clients above 100 million”: the same query, narrowed.
- [ ] Ask “where are FX rates kept?”: an answer in words naming `fx_rates`; nothing runs.
- [ ] Type `select client from balances`, run it: the error shows **Fix with AI**; it returns a query that runs.
- [ ] With `client_id = 'C1337'` and a date in a query: **Make it a report source** → `:client` and `:period_end`;
  **Save as source…** opens *Add as data source* with the suggested name.
- [ ] Switch App language to Русский: the pane's labels are in Russian and the next answer is in Russian.
- [ ] Select a data source in the Data area, ⌘I: its query opens with the assistant. ⌘I again closes it.
- [ ] Admin: untick every provider but GigaChat in *Providers allowed*, save: the others are *not allowed by the
  bank* and the pane's chip offers only GigaChat. *Admin → Activity → AI* lists the requests.
- [ ] Reports area → AI button: the list of assistants by role, with *Available* / *Coming*.

### The design assistant

Open a **template** and press **⌘I** or the **AI** button: the assistant opens in a pane right of the inspector
(drag its edge to resize). Describe a page and it lays it out from this template's own **sources** and **design
views** — the looks the theme prepared for each widget type — so the result already looks like the rest of the
report. The switch above the question box says what it works on:

- **New page** — builds a page and adds it after the one in front;
- **This page** — changes the page in front: adds, removes, moves and resizes widgets, switches a chart's type or
  design view, renames. On an empty page it fills the page;
- **Look** — restyles the whole template after a picture (see below).

![The design assistant](img/54-ai-design.png)

Before anything reaches the template the app **checks the plan**: every source and field exists, every widget fits
the grid without overlaps, every new widget shows its data and the page's fit advisor finds nothing to fix. What
fails goes back to the model with the reason (“source port has no field “nope””, “The text in “Обзор портфеля” is
cut off (the app suggests: Grow to fit text)”), up to three tries. The result is applied as **one undo step**:
**Undo** in the answer (or ⌘Z) takes the whole page or change back. **Show page** scrolls to it. If something still
doesn't fit after three tries, the answer lists it under *Still to fix* and the page's fit pill offers the usual
fixes.

Titles and texts come in the **report's language** (a Russian template gets Russian titles); the explanation comes in
the app's language. Titles may use placeholders such as `{{end}}` or `{{client}}`.

*Example* — on the UHNWI template, **New page**: “An overview page: total assets, result and return as KPI cards,
allocation by asset class as a donut, the largest holdings in a table and income by month”. The assistant answers
*An overview page: three headline figures in a row, allocation by asset class as a ring, the largest holdings as a
table and income by month as bars* and adds **Обзор портфеля** with 7 widgets: a page heading; three KPI cards —
*Итого активы на {{end}}, ₽* in the *Главная цифра* view, *Результат в деньгах* and *Доходность за период* as
cards — from `port`; a ring (*Кольцо · итог в центре*) of `cls` by `class`; a table of `top5`; and monthly bars
(*По месяцам и итог*) of `mon` by `date:month`.

*Examples* for **This page**:

- “Put the KPI cards in one row of equal width” — moves and resizes the three cards, nothing else changes.
- “Show the allocation as a horizontal bar chart instead” — the donut becomes a bar chart of the same data, in the
  theme's bar view.
- “Add a section heading above the charts and make the table taller” — a new heading; the charts move down.
- “Remove the income chart and give the table its space” — one widget removed, the table grows.

**Good requests** name what to show and how: the figure (“total assets”, “return for the period”), the breakdown
(“by asset class”, “by month for the last 12 months”) and, if it matters, the chart type or the theme's view by its
name (“as a ring with the total in the centre”). The assistant knows the sources' and fields' names, not their
values; if a figure lives in a source whose name doesn't say so, name the source.

**Test**

- [ ] Open the UHNWI template, ⌘I: the pane opens right of the inspector, *New page* is selected on a full page.
- [ ] Ask for the overview page above: a new page after the current one with a heading, KPI cards, a ring, a table
  and bars, every widget showing data; the answer says *Applied* and lists *7 widgets*.
- [ ] ⌘Z (or **Undo** in the answer): the whole page goes; the answer says *Undone*.
- [ ] On that page, *This page* → “put the KPI cards in one row of equal width”: only the cards move.
- [ ] Ask for a field that doesn't exist (“a KPI of the client's shoe size”): no broken widget — the assistant says
  it can't, or picks a real field, after the app sent its problems back.
- [ ] In a report (not a template) ⌘I: the about panel, not the design pane.
- [ ] App language Русский: the pane is in Russian; the template's titles stay in the report's language.

### Restyling after a picture

The third switch in the design pane, **Look**, restyles the template after pictures: pages of another report, a
deck, a style guide, the bank's brand site. **Paste** screenshots (⌘V in the question box), **drop** pictures on the
pane, or **attach** them with the picture button — up to 8. An **animated GIF** (a style guide or a deck recorded as
a GIF) becomes its distinct pages, up to 8 spread over the whole file. A **PDF** (a brand book, a report, a deck saved
as PDF) becomes its pages the same way — drawn in the app (pdf.js, offline), up to the 8 pictures' room, spread over
the file; a message says which pages were taken. A PDF protected with a password can't be read. A PDF also tells the
assistant what pictures can't: the app reads **its typefaces by name** (e.g. *BLKFort-Extrabold* for headings up to
42 pt, *BLKFort-Book* for the body) and **its exact colours** with how much of the pages each covers, and sends them
with the pages — so the assistant picks the closest installed typeface in build and weight, and the house's exact
brand colours. Add a few words if you like — “only the colours”, “keep our typefaces” — and send.

![Restyling after a picture](img/55-ai-restyle.png)

The aim is the report **as if that house had designed it**: the same pages, widgets and data, drawn in its visual
language. The assistant changes three things at once:

- **the look** — paper and ink, brand colours, the colours for gains and losses, typefaces (the closest ones installed
  on this Mac — it sees the weights each has, so a black-weight headline gets a family with 900) with the headline's
  size and letter-spacing, and base shapes (bar corners and widths, gridlines, donut holes, cards). For houses that
  set figures on **solid colour blocks**, the look holds up to four **block colours**: KPI cards take them in turn on
  a page (figures in black, or white on a dark block), and a thick accent bar can run beside the big figures. The
  **cover** and the **closing page** can have their own paper (e.g. a yellow back page);
- **every design view** — where the feel lives: how big and heavy numbers are, title case and size, table rules and
  header bands, value labels, legends, callouts, ring thickness, panels or no panels. It may change only how things
  are **drawn**; what a view **shows** (its data, chart type, columns, number formats, texts) is locked, so the report
  keeps its structure;
- **the page header and footer** — their fill and rules, and how the **section tabs** mark the current section (a
  filled pill, a box, a rule under it, or bold text alone), with their colour and capitals. These are part of the
  theme: saving it in the studio keeps them, and switching to another theme puts the template's own header and
  footer back (a band you colour by hand afterwards stays as you set it);
- **badges** (the chip before a heading's eyebrow, e.g. an account number) — filled, outlined or text only, their
  colours and corners.

**Bold Blocks**, a built-in theme, shows this language without a restyle: white paper, Montserrat Black headlines,
figures on yellow / orange / pink / black blocks, black table headers, thin rings, a yellow closing page.

**Data Atlas**, another built-in theme, follows data-journalism infographics: a heavy condensed headline (Roboto
Condensed), quiet condensed labels, flat muted pastels (teal, rose, butter, sage, amber) as solid treemap tiles split
by white gutters with the name and value together in the top-left corner, square corners, no grid or frames, and one
strong red kept for what must stand out.

From the look the app builds a **whole theme** the way the built-in ones (Quire, Сбер, Alpine…) are built, then
applies the views' changes on top. Before applying anything it checks it:

- every colour is a real colour, every typeface is installed, every view and setting exists and may be changed;
- text is readable: ink and body text against the paper and the cards at least 4.5 : 1 (WCAG AA), table headers on
  their band too.

- no page breaks: the app redraws every page, and text the restyle cuts off or widgets that no longer fit go back to
  the model with the view involved.

What fails goes back to the model (“colors.ink #f0f0f0 on colors.page #ffffff has contrast 1.1:1 — it needs at
least 4.5:1”, “page “Обзор”: the text in “Обзор и доходность” is cut off [view heading/account]”), up to three
tries. The new theme is **one undo step** (⌘Z or **Undo** in the answer). The answer
shows the theme's name, its paper, card, ink, accent, gain and loss colours, the series palette, the typefaces, the
shapes, how many design views changed and **What changed**, element by element. **Theme studio** opens the theme to fine-tune it or save it to the library for other templates.

The template keeps its **language and currency**. The look is kept in the theme, so a follow-up — “a warmer
accent”, “square bars” — changes the look you have, not the original; the picture stays attached for follow-ups
until you remove it (×) or start a new conversation.

What is sent: the pictures (scaled down to at most 1568 px — PDF pages too; GIF pages to 1400 px), the theme's look and its design
views' drawing settings. No data, no field names. A restyle takes about a minute. Claude, ChatGPT, Qwen and GigaChat read pictures; DeepSeek doesn't, and says so.

*Example* — McKinsey's data visualisation guidelines as an animated GIF (60 pages → 8 pictures), with “make the report
look as if McKinsey designed it”, on the UHNWI template: *clean white paper, deep navy ink, electric and active blue
as the data colours and grey for the rest; headings in a bookish serif, chart text in a neutral grotesque, sentence
case; flat square bars, hairline rules, thin rings; no cards, shadows or tinted panels*. What changed, among others:
the beige paper and cards are gone; the 40 pt heavy numbers became light serif figures in navy; small capitals with
wide tracking became sentence-case titles; the thick rule under table headers became one thin navy line; rounded
tinted bars became flat square blue ones; rings became thin. Every page keeps its widgets and data.

**Test**

- [ ] Open a template, ⌘I → **Look**: the empty pane explains pasting, dropping and attaching a picture.
- [ ] Paste a screenshot with ⌘V: it shows as *Reference picture* above the question box.
- [ ] Send: the whole template takes the new colours, typefaces and shapes; the answer shows the swatches.
- [ ] ⌘Z: the old theme is back. Send again, then “a warmer accent”: only the accent changes.
- [ ] **Theme studio** opens the new theme; save it to the library and apply it to another template.
- [ ] Pick DeepSeek in the chip and send a picture: it says DeepSeek can't read pictures.
- [ ] After the McKinsey restyle, the header's current section is underlined instead of a dark pill, and the account
  badge follows the new look. *Page → Header → Tab style* and a heading's *Badge style* show the same settings.
- [ ] Attach an animated GIF of a style guide: up to 8 of its pages appear as thumbnails; × removes one.
- [ ] Attach a PDF of 20+ pages: 8 pages spread over it appear as thumbnails, and a message names them (1, 4, 8, …).
- [ ] After a restyle, every page still says *Fits* (or lists what's left under *Still to fix*), and every widget
  shows the same data as before.

### The report assistant

In an open **report**, ⌘I or the **AI** button opens the report assistant on the right. It works with the client's
figures, so it runs only when an administrator allows them: *Admin → AI → Bank policy → Figures* (saved at once), or for some
people only, in *Admin → Users & roles* or *Groups → AI*. An administrator can also
press **Allow for the bank…** right in the report pane (with *Providers allowed* you can limit it to, say, GigaChat). What it sends is the figures the report
shows — page by page, each table at most a dozen rows — with the **client's name and every account, contract and
security code replaced by placeholders** ([CLIENT], [ID-3]); the answer comes back with them put in again, on this
computer. Every request is in the AI log.

Pick a task (an optional line above says what to focus on — “the client worries about the outflows”):

- **Brief & pitch** — a summary of the report, **Pay attention** (the points the client will notice or ask about,
  good and bad, each with a link to its page) and **How to present it**: an opening line, the key messages in the
  order to say them, the questions the client is likely to ask with short answers from the report, and a closing
  line. **The main ideas and key figures stand out**: the model marks the phrase that carries each point, and figures
  with a unit or a sign are bold — gains green, losses red, the rest in a highlight colour.
  - **E-mail to the client** (beside *How to present it*) opens a ready-to-send e-mail for the client: a subject,
    a greeting by name, the main results in a few short paragraphs, an invitation to talk and a sign-off with your
    name — nothing internal (no page numbers, no *If the client asks*). Edit it in place, then **Copy e-mail** (paste
    into Mail or Outlook with its formatting) or **Open in Mail** (a new message with the subject and text). Attach
    the report itself (*Export → PDF*). Briefs made before this build their e-mail from the pitch.
  - **Copy brief** (at the bottom) copies the whole brief, formatted, for yourself or a colleague — it is internal.
    The small copy button copies plain text.
  - **Save as PDF** saves the brief as an A4 document **in the report's style** (its colours and typefaces), headed
    with the **client's name, the report and its period** and the date it was prepared; every page's footer marks it
    *For internal use — not for the client*. A saved brief keeps the client and period it was made for.
- **Check before sending** — figures that contradict each other between pages, notes whose words don't match the
  figures, empty parts, striking figures left without a note, and wording a compliance officer would stop; each with
  its severity, a suggested fix and a link to the page.
- **Note for this page** — a short note to the client about the page in front, in the report's language and the
  bank's tone. Nothing is added until you press **Add to page**: it goes in full width (on a page of its own right
  after, if the page is full) and you edit it like any note.

Answers rely only on the report: no market news, no advice, no promises of returns. A full brief or check takes about
half a minute.

*Example* (the UHNWI report, Brief & pitch): *Pay attention — the capital fell from 437.3 to 313.6 million: outflows
of 209.8 million against 43 million in; explain it first (p. 3) · the return of 10.7% is below the median of similar
clients (13.4%) but the portfolio is far calmer: volatility 1.4% against 6.9% (p. 6) · 53% of the assets sit on one
savings account (p. 4) · deposits of 14, 22 and 16 million come back from December (p. 20).* The check on the same
report once found the investment income shown as 7 874 061 on page 3 and 8 741 788 on page 5 (metal accounts counted
as deposits on one page and as investments on the other); the template now follows one rule. The check reads tables
as they are drawn — each continued page with its own rows, hidden columns left out, totals only where shown, figures
with their units — so it doesn't report what the reader can't see.

**Test**

- [ ] With *Data that may be sent* at *Structure only*, the report assistant says figures aren't allowed; as admin,
  **Allow for the bank…** allows them after one confirmation and runs the task.
- [ ] Bank at *Structure only*, the group *Private bankers* at *Figures*: the banker in it gets a brief; the user's
  Admin page says *Now: Figures (set for the group “Private bankers”)*.
- [ ] At *Figures*: **Brief & pitch** answers in about half a minute; the page links go to the pages; the main ideas
  and figures are bold and coloured (gains green, losses red).
- [ ] **E-mail to the client**: a subject and a letter to the client signed with your name, without page numbers or
  the client's likely questions; **Copy e-mail** pastes into Mail formatted; **Open in Mail** opens a new message.
- [ ] **Copy brief**, paste into Mail: headings, coloured figures and the numbered messages survive.
- [ ] **Save as PDF**: an A4 PDF in the report's colours and typefaces, with the client, report and period on top and
  page numbers below.
- [ ] **Check before sending** lists findings by severity, each with a fix and a page link.
- [ ] On a page, **Note for this page** → **Add to page**: the note appears; ⌘Z takes it back.
- [ ] *Admin → Activity → AI* shows the requests as *Report: brief & pitch*, *Report: check*, *Report: note*, with
  the person's name and their tokens; the detail shows what was sent — no client name, account number or instrument name, only placeholders — and the answer.


### The administrator's assistant

In the **Admin** area, ⌘I or the **AI** button opens the assistant in a drawer on the right (also *Ask AI* in
Activity, *Change with AI* in Users & roles and Access). It has two modes:

- **Ask the log** — questions about the last 30 days in plain words: *who exported PDFs this week?*, *what did each
  banker do yesterday?*, *why did sign-ins fail?*, *who used the most AI, and for what?* The answer comes with links
  to the **sessions** and **events** it rests on; a link opens them in Activity.
- **Change access** — say what to change: *give Warren insurance clients only and the Insurance desk group*, *add
  Anna Petrova (apetrova) for the clients of Ivan Sokolov, Private bankers, with the UHNWI report*. You get, for each
  person, a **before → after** of roles, clients, products, groups, AI level, sign-in and reports — and anything
  unclear as a question. Nothing is saved until you press **Apply**; the changes are then saved exactly as on the Users
  page (each one in the activity log). A new person gets a first password, shown once, and chooses their own at the
  first sign-in. A name, banker, client, group or report the assistant can't find is shown as a problem, and Apply
  waits until you ask again more precisely.

Two more places use it:

- **Explain with AI** on a **failed** event (Activity → All events → open it): the most likely cause and what to do,
  read from the event, its error and what the same person did around it.
- **Review with AI** on the **Access** page: who has more than they need, demo passwords, accounts never used,
  failed sign-ins (for logins that don't exist too), unusual activity (exports at night, many clients opened, very
  long sessions), people who may send figures to AI but don't use the report assistant. Each finding has its
  severity, what the facts show, links, and where it fits a button: *Open* the person, *Disable account*, *AI: Structure
  only* (each asks first).

**What leaves the bank.** The people (their roles, access, groups and AI level), the sessions and events of the last
30 days, the bankers, clients, products, groups and reports — with **every person, banker and client as a code**
([P-2], [B-1], [CL-7]); logins and client codes are never sent, and names in your question are replaced too. The
answer comes back with the names put in. It needs no client figures, so it works at *Structure only*.

**Admin → AI → Use and cost** shows the last 30 days by person, task and provider with an **approximate cost**, from
prices per million tokens that you can set (*Prices per million tokens* — list prices to start with; put your
contract's), and tips such as *these people may send figures but don't use the report assistant* or *a third of the
requests failed — check the keys*.

**Test**

- [ ] Admin → ⌘I → *Ask the log*: “who signed in today?” — an answer with links; a link opens the session or event.
- [ ] *Change access*: “give Warren Buffington insurance clients only” — Products: every product → Insurance; Apply,
  then Users & roles shows it and Activity has *Change user* with before and after.
- [ ] Ask for someone who doesn't exist: the change shows the problem and Apply is disabled.
- [ ] A failed sign-in → *Explain with AI*: cause and what to do.
- [ ] Access → *Review with AI*: findings with severities; *AI: Structure only* on one asks, saves and the table shows it.
- [ ] Admin → AI: the costs follow a changed price at once.
---

## 17. Report API

Other systems can ask Quire for a client's report, or a batch of them, and get the PDFs back without anyone opening
Quire. For example:

- a **CRM** puts the quarter's report next to the client;
- a **client portal** offers it for download;
- a **scheduler** makes every client's report at quarter end.

This chapter is for the people who run Quire: building the server with the API, setting it up in Admin, trying it
out, and running it day to day. The developers of the calling systems have a guide of their own,
`docs/report-api/README.md`. The server also describes itself in the OpenAPI format at `/api/v1/openapi.json`, for
their tools.

**How it works, in short.** Each calling system has an **API account** with a key. A request names the client (by the
bank's id), the period and, if the account may, some of the report's settings. Quire checks the request at once and
answers with a **job**. Workers on the server make the reports with the same pages, data and drawing as the app. The
system downloads the PDF, or a zip for a batch. Optionally, Quire also copies each report to a shared folder or bucket,
and calls the system back when the job is done.

### 17.1 Running the server with the API

The API is part of the **web server**, built with the report renderer. The desktop app shows **Admin → API** too, but
doesn't answer API requests: its page says so at the top, and what is set there stays on that computer — a server is
set up in its own browser version. If the server was built without the renderer, the page says so at the top:
settings can be made, but requests are refused until it is rebuilt.

```sh
cargo run --release --no-default-features --features render --bin quire-server -- --data /srv/quire --api-workers 4
```

- **`--features render`** builds in the page renderer (V8, Chrome's JavaScript engine). The first build downloads it,
  which takes a few minutes. A server built without it answers every API request with `501` and says so.
- **`--api-workers N`** is how many reports are made at once (2 if not given, up to 64). One report takes about 0.75
  CPU-seconds, so up to the number of cores, more workers make reports faster. See [17.6](#176-running-it-day-to-day)
  for measured numbers.
- **`--data`** holds everything, the API's included: the accounts in `users.json` (keys only as fingerprints), the S3
  secret keys sealed in `secrets.json`, and the jobs and their files in `api/jobs/`.
- **HTTPS.** Run the server behind an HTTPS proxy, as for people signing in. A key travels in every request.
- **Docker.** The image is built with the renderer: `docker build -t quire-server .`. Workers go after the image name:
  `docker run … quire-server --api-workers 4`. A destination folder on the host must be mounted into the container
  (`-v /mnt/reports:/mnt/reports`).

### 17.2 Setting it up in Admin → API

Sign in to the browser version as an administrator and open **Admin → API**. A good order is: first decide what each
report accepts (17.2.3), then add the destinations (17.2.4), then the accounts (17.2.1).

![Admin → API](img/59-admin-api.png)

#### 17.2.1 API accounts

Add one account per calling system with **Add API account**:

| Setting | What it does |
|---|---|
| **Name**, **What it is** | How the system shows here and in Activity. *What it is* is a note, such as who runs the system. |
| **Id** | Made from the name (`client-portal`) if left empty. It is part of the key and of the log (`api:client-portal`). Changing it makes a new key. |
| **Its key works** | Off: the system is refused (`401 … switched off`) until it's switched back on. Nothing else changes. |
| **Client access** | The same as a person's: every client, or bankers' coverage and clients one by one; every product, or some. The preview says how many clients that comes to. A request for any other client is refused (`403`). With *Selected products*, reports show only those products' accounts. |
| **Reports** | The published reports it may ask for. None ticked: every published report (except those kept off the API). |
| **Copy its reports to** | A destination (17.2.4) that each of its reports is also copied to. *Nowhere*: download only. |
| **Callbacks may go to** | The hosts its callbacks may go to, separated by commas (`crm.bank.local`, `hooks.bank.local:8443`). A host without a port allows any port; with a port, only that one. Empty: the system can't ask for callbacks. |

#### 17.2.2 The key

![The key, shown once](img/60-api-key.png)

**Add and make its key** saves the account and shows its key **once**, with **Copy**. Give it to the system's team as
you would a password, for example in your password manager. Quire keeps only a fingerprint and can't show the key
again. The list shows its last four characters, to tell keys apart.

**New key…** makes a new key and the old one **stops working at once**. Do it together with the system's team, when a
key may have leaked, or when someone who had it leaves. Callbacks are signed with a key worked out from the API key,
so they change with it. An account made before callbacks existed needs a new key before it can sign them; its page
says so.

#### 17.2.3 What each report accepts

![What each report accepts, destinations, finished files](img/61-api-reports.png)

Every published report has a box:

- **Offered over the API**: off keeps the report away from every system (`403`), whatever their accounts say.
- **A request may set it**: a request always gives the client and the period. Tick the report's other parameters a
  system may choose; for the UHNWI report that is *Products*. A parameter not ticked is refused with `422`, and the
  message lists what the report takes. **Nothing is ticked at first.**
- **Default for API requests**: used when a request leaves the parameter out, for example `BROK, BANK` (several
  values by comma; the choices are listed under the box). Empty: the template's own default (*Template's default*).
  A default applies even when the parameter isn't ticked, so you can fix a value for every system.

Press **Save** in the report's box.

#### 17.2.4 Destinations

![A destination](img/62-api-destination.png)

**Add destination** makes a place that an account's reports are copied to, besides the download:

- **Folder or file share**: a full path **on the server** (`/mnt/reports`). A network share must be mounted on the
  server first, and in Docker mounted into the container. Quire needs to be able to write there.
- **S3 bucket**: any S3-compatible service, addressed path-style (`<address>/<bucket>/<key>`). The address and region
  for some common services:

| Service | Service address | Region |
|---|---|---|
| AWS | `https://s3.eu-central-1.amazonaws.com` | `eu-central-1` |
| Yandex Object Storage | `https://storage.yandexcloud.net` | `ru-central1` |
| MinIO | `http://minio.bank.local:9000` | `us-east-1` (MinIO's default) |

  Add the **bucket**, an optional **prefix** (a folder inside the bucket) and the **access key** and **secret key**.
  The keys need permission to write objects (`s3:PutObject`). The secret is sealed on the server and never shown
  again; leave it empty when changing the destination to keep it.

Reports go into folders by account, day and request: `<account>/<date>/<job>/`. In a folder they are named
`<client id> — <client> — <report> — <period>.pdf`; in a bucket, `<client id>.pdf`.

**Test** writes a small file there to show Quire can reach it (in a folder it is taken away again; in a bucket a
`quire-test-….txt` stays). A destination an account copies to can't be deleted; change the account first.

#### 17.2.5 Finished files

Finished files are kept for download for the days set under **Finished files** (7 to start, 1 to 365), then deleted
with their job: a system asking later gets `404`. Copies in a destination stay. Old jobs are looked for every hour.

![Delivery](img/63-api-delivery.png)

### 17.3 Trying it out

After setting up an account, check it end to end before giving the key to the system's team. With the account's key
in `KEY` and the server's address in `Q`:

```sh
# 1. the reports the account may ask for, and what each takes
curl -s -H "Authorization: Bearer $KEY" $Q/api/v1/templates

# 2. one report: answers 202 and a job right away
curl -s -H "Authorization: Bearer $KEY" -H 'Content-Type: application/json' \
     -d '{"client_id": "C0025", "period": {"preset": "last_quarter"}}' $Q/api/v1/reports

# 3. its status: "queued", "running", then "done" (a few seconds)
curl -s -H "Authorization: Bearer $KEY" $Q/api/v1/jobs/<job_id>

# 4. the PDF
curl -s -H "Authorization: Bearer $KEY" -o report.pdf $Q/api/v1/jobs/<job_id>/file
```

Then check what it **mustn't** do:

- a client outside its access: `403`;
- a parameter you didn't tick, such as `"params": {"product": ["BROK"]}`: `422`;
- without the key, or with a wrong one: `401`.

Look in **Admin → Activity → API**: the requests and the report are there under `api:<id>`.

For a destination, press **Test**, then ask for a report and look in the folder or bucket. For callbacks, the system's
team gives a test address on an allowed host. The job's `callback` then says `sent`, or why not.

**Checks the developers run** (TESTING.md § 6):

- `node dev/web/api-checks.mjs`: 28 checks against a server on fresh data. It covers keys, a single report, a batch's
  zip, refusals, a folder destination, a signed callback and Activity.
- `node dev/load/api.mjs`: quarter-end batches and single requests at once, for sizing.

### 17.4 How requests work

**A request** is `POST /api/v1/reports` with JSON:

| Field | | |
|---|---|---|
| `client_id` | one report | The bank's id of the client, like `C0025`. |
| `clients` | a batch | Several ids, up to 2000, each made once. Give `client_id` or `clients`, not both. |
| `period` | required | Two dates, `{"start": "2026-07-01", "end": "2026-09-30"}`, or a named period, `{"preset": …}`: `ytd` (this year so far), `last_month`, `last_quarter` or `last_year`, counted back from the day of the request. |
| `template` | optional | A published report's id; the UHNWI report if left out. |
| `params` | optional | Only the parameters ticked for the report (17.2.3): a text or number, or a list for one that takes several. |
| `callback_url` | optional | Called when the job is done, on a host allowed for the account. |

Any other field is refused, so a typo doesn't go unnoticed.

**Checked first.** Quire checks everything it can before starting: the account, the report, every client and the
access to it, the period and the parameters. A request it can't serve whole is refused with the reason, and a batch
with one client it can't report on starts nothing; `error.clients` lists each refused client.

**The job.** An accepted request answers `202` with the job, and `GET /api/v1/jobs/<id>` shows it as it goes:

- `status`: `queued`, `running`, then `done`, `partial` (some reports failed) or `failed`;
- for each client: `status`, `pages`, `layout_issues`, and **`values`**, what the report was made with, defaults
  included. Keep `values` with the file;
- `copy`: the copy to the destination, `done` with where, or `failed` with why;
- `callback`: `pending`, `sent` or `failed`, with the attempts;
- `expires`: when its files are deleted.

**Single requests go first.** Reports asked for one client at a time, such as a click in a CRM, are made before the
batches already waiting. With three quarter-end batches running, one is ready in about a second.

**Files.**

- `GET …/file` gives the PDF, or a zip of a batch's PDFs once the batch is finished. Before then it answers `409`.
- `GET …/files/<client_id>` gives one client's PDF of a batch.
- A copy that fails leaves the report downloadable.

**Retries.** A system that sends the same request again with the same `Idempotency-Key` header gets the first job
back instead of new reports.

**Errors** are JSON, `{"error": {"status": …, "message": "…"}}`, and the message says what to change:

| Status | When |
|---|---|
| `400` | The body isn't JSON. |
| `401` | No key, a wrong one, or the account is switched off. |
| `403` | A client or product outside the account's access, a report it may not ask for, or one kept off the API. |
| `404` | No such report, client or job (another account's job counts as missing, and so does one past its days). |
| `409` | The file isn't ready yet. |
| `422` | The request doesn't fit: a bad period, a parameter not ticked, a value the report doesn't accept, or a callback host not allowed. |
| `429` | Over the limits below: wait the seconds in `Retry-After`. |
| `500` | Quire couldn't make the report; the message says why. |

**Callbacks.** When a job with a `callback_url` finishes, Quire POSTs it there, the same JSON as
`GET /api/v1/jobs/<id>`. A call that doesn't get a `2xx` within 30 s is tried again after 10 s, 1 minute and 5
minutes. Each call is signed: `X-Quire-Signature` is HMAC-SHA256 of `<X-Quire-Timestamp>.<body>`, keyed with the
SHA-256 of the API key. The developers' guide shows how to check it in Python and Node.

**Limits** per account: 60 new requests a minute, and 1200 other requests (status, downloads). Past either, `429` with
`Retry-After`. Up to 2000 clients in one request.

### 17.5 What is recorded

Everything is in **Admin → Activity**, under the account as `api:<id>`. The **API** filter shows the API's own events:

| Event | Area | Written when |
|---|---|---|
| *report request* | API | A request accepted (its clients, period and parameters) or refused (why). |
| *api export* | Files | A report made, with the client, its pages and layout issues; or failed, with the error. (It doesn't show as a banker's latest report in their client list: that lists their own.) |
| *api copy* | API | A copy to a destination that failed. |
| *api callback* | API | A callback delivered, or failed after its attempts. |
| *add / change / delete API account*, *API key*, *API report*, *API destination*, *API files* | Admin | Every change made in Admin → API, with what it was set to. |

Keys and S3 secrets are never written to the log.

### 17.6 Running it day to day

- **Keys.** Make a new key when one may have leaked or someone who had it leaves. To stop a system for a while, switch
  *Its key works* off; to stop it for good, delete the account. Its requests stay in Activity.
- **Restarts.** Jobs are kept on disk: a restarted server finishes the reports that were waiting, and sends callbacks
  that were due.
- **Disk.** A report's PDF is about 1 MB, kept for the retention days. A quarter-end run for 1000 clients needs about
  1 GB until its files expire.
- **Sizing.** Measured on a 10-core Mac (release build), with three batches and single requests at once:

| `--api-workers` | Reports a minute | 1000 clients in about |
|---|---|---|
| 1 | 83 | 12 min |
| 2 | 150 | 7 min |
| 4 | 253 | 4 min |
| 8 | 366 | 3 min |

  Memory as the system shows it was 1 GB with one worker up to 3 GB with eight. Most of it is freed memory kept for
  the next reports; the memory actually in use stays at about 130 MB, and none leaks. Measure on your own server with
  `node dev/load/api.mjs` before choosing the workers; Linux may differ.
- **Reports with layout issues** (`layout_issues` above 0) were made, but some content was cut off or ran past the
  page. Open the same report in Quire, for that client and period, to see why: usually a client with more accounts or
  holdings than the template was laid out for.

### 17.7 Troubleshooting

| What you see | Why, and what to do |
|---|---|
| Requests don't see what was set in Admin → API | It was set in the desktop app, which keeps its own settings. Set it up in the server's browser version. |
| A warning on top of Admin → API, or `501 … built without the report renderer` | The server was built without `--features render`: its settings can be made, but requests are refused. Build it with the renderer. |
| `401 Invalid API key` | The key was copied wrong, or a new key was made since. Make a new key and give it to the team. |
| `401 … switched off` | *Its key works* is off for the account. |
| `403 … outside your access` | The client or product isn't in the account's *Client access*. |
| `403 … isn't one of your reports` / `… isn't offered over the API` | The account's *Reports*, or *Offered over the API* for the report. |
| `422 … can't be set over the API` | The parameter isn't ticked under *What each report accepts*. |
| `422 Callbacks may go to … only` / `… aren't allowed` | Add the host to the account's *Callbacks may go to*. |
| `409 Not ready` | The job is still running: ask again when `status` is `done`. |
| `404` for a job | It belongs to another account, or its files were deleted after the retention days. |
| A copy `failed` | Press **Test** on the destination. For a folder: does it exist on the server, can Quire write there, and is it mounted into Docker? For S3: the address, bucket, region and keys, and the keys' permission to write. |
| A callback `failed` | Can the server reach the address? It must answer `2xx` within 30 s. The job's `callback.error` says what happened. |
| Reports come slowly | More `--api-workers`, up to the server's cores. Single requests already go before batches. |

**Test**

- [ ] The desktop app has Admin → API with a note on top that it doesn't answer requests; the browser version has it
  without that note. A server built without `--features render` shows a warning on top of it.
- [ ] *What each report accepts* → UHNWI → tick *Products*, default `BROK` → **Save**.
- [ ] **Add API account** → *Client portal*, *Selected clients* → one banker → **Add and make its key**: the key is
  shown once with *Copy*; after *Done* only its last four characters show.
- [ ] With the key (17.3): templates list UHNWI with *product*; a report for one of the banker's clients is `202`, then
  `done` with 18 pages and `values.params.product` = `["BROK"]`; the PDF downloads.
- [ ] A client of another banker: `403`. `"params": {"colour": "red"}`: `422`. No key: `401`.
- [ ] A batch of three of the banker's clients: one job; `…/file` is `409` while it runs, then a zip of three PDFs.
- [ ] **Add destination** → a folder → **Test**: *Quire can write to …*. The account's *Copy its reports to* → it →
  a new report is also in `<folder>/client-portal/<date>/<job>/`.
- [ ] *Callbacks may go to* empty → a request with `callback_url`: `422`.
- [ ] *Its key works* off → `401 … switched off`. **New key…** → the old key `401`, the new one works.
- [ ] Activity → **API**: the requests, reports and copies under `api:client-portal`; *Admin*: each change made here.
- [ ] *Finished files* → 1 day; a job's `expires` is a day after it finished.

---

## 18. Full test checklist

A quick pass through every area; details are in each chapter's **Test** list.

| # | Area | Check | Pass |
|---|---|---|---|
| 1 | Home | Examples show only the UHNWI report; opening it hides the home screen | ☐ |
| 2 | Menus | All seven menus open and their items work | ☐ |
| 3 | Right-click | Page, widget, inspector, text field, left panel each show their menu | ☐ |
| 4 | Search | ⌘F, type a setting name, change it in the results | ☐ |
| 5 | Layout | Hand resize sticks; cut-off badge and Fit work | ☐ |
| 6 | KPI | Value size changes take effect; hero fits | ☐ |
| 7 | Charts | Chart type switch; only relevant settings shown | ☐ |
| 8 | Tables | Column panel; bar options; best & worst with two rankings | ☐ |
| 9 | Contents | List/Chapters; hide/add pages; Fill in from the report | ☐ |
| 10 | Icons | Search, flags, brands, upload; Flags & logos finds "UAE dirham" by name | ☐ |
| 11 | SQL | SQL dataset with CTE + join; `:period_start` / `:period_end` follow the period | ☐ |
| 12 | LOD | FIXED / INCLUDE / share fields built and used in widgets | ☐ |
| 13 | Parameters | From data, multi-select, filters column, report redraws | ☐ |
| 14 | Formats | App settings, document formats, apply | ☐ |
| 15 | Themes | Studio edits reach the template; library manage | ☐ |
| 16 | Reports | Preview as user, permissions, gallery | ☐ |
| 17 | Export | PDF and PowerPoint | ☐ |
| 18 | AI | Key saved and tested; SQL written, explained, fixed, made a source; answers in the app's language | ☐ |
| 19 | AI design | A page built from a description and undone in one step; the page in front changed | ☐ |
| 20 | AI look | Template restyled after a pasted picture; text readable; undone in one step | ☐ |
| 21 | AI report | Brief & pitch, check and a page note on a report; identities masked; refused at Structure only | ☐ |
| 22 | Report API | *(web)* An API account's key shown once; a report asked for with it downloads as a PDF; another banker's client refused; a copy lands in the destination | ☐ |
