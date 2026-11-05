# Quire as a web app (server mode)

> **Status:** started 29 Oct 2026 — phases 1–3 done (one command layer; a workspace per browser, each person's settings and files; sealed secrets, data fence, sign-in lockout, HTTP limits, Dockerfile, checks and a load check). Open: Python per person on a remote server, single sign-on, the Docker image built and tried on Linux, more than one server.
> **Goal:** users open Quire in a web browser, sign in and work as in the desktop app — up to **500 people at the same time**.

---

## Summary

Feasible without a rewrite. The interface is already a web page (HTML + JavaScript that draws the report pages itself), and the Rust core already answers it over HTTP in development (the *dev bridge* used for every screenshot). The work is turning a **one-person desktop core** into a **many-person server**: a workspace per session, storage instead of local files, single sign-on, and enough capacity for 500 users.

---

## What already fits

| Today | Why it helps |
|---|---|
| UI is plain HTML/JS, pages drawn as SVG in the UI | Runs in any browser; the server doesn't spend CPU drawing pages |
| Dev bridge (`cargo test dev_bridge -- --ignored`, `dev/shots/live.html`) | A working prototype of "UI in a browser ↔ core over HTTP" |
| Access rules in the core (clients × products, AI levels, report edit policy) | A browser user can't get around them — checks are server-side |
| PDF / PowerPoint export made by the core (`src/export.rs`) | The server writes the file; the browser downloads it |

---

## What has to change

### 1. One shared state → a workspace per session *(the biggest change)*
- Today: one `App` behind one `Mutex` (`AppState` in `src/main.rs`) — one set of open documents, undo/redo, data caches.
- Needed: a workspace per signed-in session (open documents, undo, caches), closed after inactivity; long work (queries, exports) outside the lock, on a worker pool.

### 2. Desktop-only features → web equivalents
69 Tauri commands; ~86 uses of `app_handle`; 18 places use file dialogs or the Keychain.

| Desktop | Web |
|---|---|
| Open / Save (`.rpt`, `.qtpl` files) | Reports and templates **stored on the server**: "My reports", sharing, upload / download |
| Batch reports → pick a folder | **ZIP download**, or a server folder |
| "Show in Finder", "Open in Mail" | Download; `mailto:` link |
| AI keys in macOS Keychain | Server secret store |
| Per-computer settings (`settings.json`) | Per-user settings |
| Theme library folder | Shared theme library on the server |

### 3. Shared state that must become per-user (or safe to share)
- Fonts embedded in templates — `fonts::add` registers them globally (`src/fonts.rs`).
- In-memory caches: `overview_cached` (`src/overview.rs`), `my_reports` index (`src/audit.rs`), thread-locals in `ai.rs`, `data.rs`, `period.rs`, `vm.rs`.
- Activity log `activity.jsonl` and `ai_log.jsonl` → a database table.

### 4. Python data sources
Designers can write Python that runs in the app (`PyEngine`). On a shared server: **sandbox** it (separate process, no network / filesystem, limits) or allow it only for trusted roles.

### 5. Sign-in and security
- Built-in accounts work today (`src/auth.rs`, salted hashes); a bank will want **single sign-on** (Active Directory / SAML / OIDC).
- HTTPS, secure session cookies, CSRF protection, session timeout.
- Most likely deployed **inside the bank's network** (client data).

---

## Capacity: 500 simultaneous users

| | Estimate |
|---|---|
| Memory per open UHNWI report (document + data) | ~5–20 MB |
| 500 active users | ~3–10 GB in total |
| Opening a report | ~40 SQL queries against the bank's database |
| Client list figures, 4,100 clients | ~1 s (measured, DuckDB, 15 M daily rows) |
| Page drawing | in each user's browser — no server cost |

- **The bank's database is the real bottleneck** — 500 users opening reports means thousands of queries a minute. Plan a **read replica** and caching of shared data (benchmarks, prices, peer figures).
- **Batch reports and AI requests** as **background jobs with a queue**, so one banker's quarter-end run doesn't slow everyone else.
- **Starting size:** 2 application servers (16 cores, 32–64 GB each) behind a load balancer with sticky sessions; one database for documents, users and the activity log; a read replica of the bank's data.

---

## Progress

**29 Oct 2026 — phase 1 done.**
- The core is a library (`src/lib.rs`, `src/app.rs`); `quire::dispatch` is the one command layer. The desktop app sends
  every command through one Tauri command, `call`; the server through `POST /api/call`. The dev bridge (its second
  copy of the routing and role checks) is gone; the screenshot and render checks run against the server.
- `src/host.rs`: what differs between platforms — the app's folder, file dialogs, eyedropper, Finder.
- `src/bin/quire-server.rs`: builds without Tauri (`--no-default-features`), serves `ui/` and the commands. Plain std
  HTTP, a thread per request — enough for one workspace; a proper HTTP stack comes with sessions.
- `ui/platform.js`: the UI picks the desktop or web way in; desktop-only actions hidden in the browser.

Decisions taken (29 Oct): runs **inside the bank's network**, one workspace per organisation; the web version is for
**everyone**, designers too; the desktop app stays. **Python runs in the person's own environment** (their Python, their
packages), not a shared server one — how that works for a remote server is open (below).

Uploads added the same day (*Browse…*, *File → Open*, scripts → the server's `uploads` folder). Not yet in the
browser: (downloads added the same day) per-person sessions (today
the server has one sign-in for all its users, so it only listens on this computer), AI keys off the macOS Keychain.

**29 Oct 2026 — phase 2 done.** A workspace per browser (cookie `quire_sid`: HttpOnly, SameSite=Strict, 128 random
bits; closed after 12 idle hours, at most 1000). Each person's settings in `users/<login>/settings.json` (a copy of
the server's the first time) and files in `files/<login>/`; downloads and deletes only from one's own folder; documents
open only from inside the data folder. *Saved* list in the Reports area. `--dev` pages without the cookie share one
workspace (the screenshot checks).

Still open for phase 3: HTTPS (reverse proxy) and sign-in hardening (lockout, session timeout on the sign-in itself,
single sign-on); AI keys in a server secret store (today the macOS Keychain); Python **per person** on a remote server
(question 5 below); data sources may still name any path on the server (an allowlist for designers' CSV/DuckDB/SQLite
paths); writes to shared files (`users.json`, logs) from many workspaces at once are not locked against each other;
Docker image; load tests.

**29 Oct 2026 — phase 3 done.** Secrets sealed in `secrets.json` (`QUIRE_SECRET_KEY`): AI keys and connection passwords
(the page sees `••••••`). The data fence: files only from the data folder, the demo database's and `--allow-dir`
folders (DuckDB file functions and symlinks too; no SQLite ATTACH); `--allow-host` for database servers. Sign-in
lockout (5 wrong → 15 min). `--secure`, security headers, request limits, the accounts file written whole (0600).
`Dockerfile` (not built yet: no Docker on the dev Mac). Checks: `dev/web/checks.mjs`; load: `dev/load/reports.mjs`.
Load (10-core Mac, dev build): about 3 CPU-seconds per person (sign-in, the 22-page report, a period change); 50 at
once in 11 s, 100 in 24 s, no failures. Shared DuckDB instances cut it from 17 s for 50.

What 500 simultaneous users need, from that: report openings are CPU-bound — about 3 CPU-seconds per person
(sign-in, the report, a period change), the same in the release build: it is DuckDB running the report's ~40 queries
over 15 M daily rows. If a tenth of 500 people open a report in the same minute, that's about 2–3 cores' work on
average; quarter-end peaks (everyone at 9:00) want several application servers, or figures shared between people
(benchmarks, prices) computed once. That needs sessions that survive a server
(sticky sessions at the load balancer, as in the plan) and the shared files (`users.json`, logs) in a database.

Still open:
- **Python per person on a remote server** (question 5): Python runs on the server as the server's user, outside the
  data fence, so it is **off on a server by default** (29 Oct; an administrator switches it on in Admin → Access).
  Still to choose for when it's needed: an environment per person on the server, or a helper on the person's computer.
- Single sign-on (AD / SAML / OIDC); sign-in lockout per address as well as per login.
- The Docker image built and tried on Linux (fonts, curl, Python inside).
- Templates' own connections keep their passwords in the template file.
- Writes to `users.json` from many workspaces at once: written whole and one at a time, but two changes at the same
  moment keep the later (a database fixes it).

## Plan

### Stage 1 — Server mode, same code
- ✅ A second binary, `quire-server`, built from the existing core.
- ✅ **One command layer** used by both the Tauri window and the HTTP server (`quire::dispatch`; the dev bridge is gone).
- ✅ A workspace per session; idle sessions closed.
- ✅ The UI talks to the server instead of Tauri (`ui/platform.js`).
- ✅ Desktop app keeps working.
- **Result:** a usable internal web version.

### Stage 2 — Storage
- Reports, templates, themes, users, settings and the activity log in a database (PostgreSQL).
- Upload / download instead of file dialogs; batch reports as ZIP.
- AI keys in a server secret store.

### Stage 3 — Production for 500 users
- Single sign-on, HTTPS, session security.
- Job queue for batch reports and AI.
- Python sandbox or policy. *(open: question 5)*
- Shared caches for bank data; connection pool to the bank's database.
- Load tests at 500 users; Docker images; deployment inside the bank's network.
- Monitoring: request times, queue length, database load.

---

## Open questions (decide before starting)

1. **Hosting** — inside the bank's network, or a cloud?
2. **Sign-in** — Active Directory / other single sign-on, or Quire's own accounts for now?
3. **Desktop app** — keep it alongside the web version (e.g. for designers), or web only?
4. **Bank database** — PostgreSQL, Oracle, other? (The demo uses DuckDB; the server's database access depends on it.)
5. **Python in the person's own environment, on a server** — decided: each person's Python, not a shared one. Options:
   an environment per person on the server (they choose packages; isolated per person), or Python running on the
   person's computer next to the browser (a small local helper the page talks to; their machine, their data access).

Answered 29 Oct: 1 inside the bank's network; 3 keep the desktop app; web for everyone.

---

## Pointers in the code

| Area | Where |
|---|---|
| App state, commands, edit operations | `src/app.rs` (`struct App`, `AppState`, the commands, `dispatch`) |
| Browser ↔ core over HTTP | `src/bin/quire-server.rs`, `ui/platform.js`; platform differences in `src/host.rs` (desktop: `src/main.rs`) |
| Accounts, access, roles | `src/auth.rs` |
| Activity log, sessions | `src/audit.rs` |
| AI providers, keys, log | `src/ai.rs` |
| Exports | `src/export.rs` |
| Fonts (global registry) | `src/fonts.rs` |
| UI calls into the core | `ui/state.js` (`call`) |
