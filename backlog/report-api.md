# Report API for other systems

> **Status:** agreed 10 Nov 2026 — see *Decisions*; steps 1–2 done 10 Nov 2026 (the core call, the V8 renderer); next: step 3
> **Goal:** other systems (CRM, client portal, a scheduler…) ask Quire for a client's report over an API — **client id, period, template, products** (what a user picks in the app) — and get a PDF or PowerPoint back.

---

## Summary

Most of it exists already: **batch reports** build a report for a given client and period apart from the open documents, with the person's access, and the core writes PDF / PowerPoint. The one real decision is **how to draw the pages on a server**, since today the drawing runs in the interface's JavaScript and measures text with the browser.

---

## What's already there

| Need | Today |
|---|---|
| A report for a client, period and products without the UI | `batch_make` in `src/main.rs`: a published template, for a client and a period, in a workspace of its own; access applied |
| Products filter | the template's `product` parameter (`src/uhnwi_sample.rs`), plus the person's products (`access_src`) |
| PDF / PowerPoint | `src/export.rs` (`export::pdf`, `export::pptx`) |
| Access: clients × products | `src/auth.rs` — an API client gets its own rules, like a person |
| Logging | `src/audit.rs` (Admin → Activity) |

---

## The decision: drawing pages without a browser

Pages are drawn by the UI's JavaScript (`pageSvg` in `ui/render.js`, `slide` for PowerPoint), which measures text with the browser's canvas (`textWidth` in `ui/util.js`).

| Option | How | Output | Effort |
|---|---|---|---|
| **A. Headless Chrome** *(recommended first)* | The server runs the existing drawing code in a pool of invisible Chrome instances — as `dev/shots/*.mjs` and the batch tests already do | Identical to the app | Small |
| **B. Embedded JS engine** | The drawing code runs inside the server; text measured by the core's font code (`src/text.rs`, `src/fonts.rs`) instead of the browser | Very close; line breaks may differ slightly | Medium |
| **C. Drawing ported to Rust** | The page drawing moves into the core | Same, once done | Large |

Start with **A** (exact output, quick); move to **B** if volumes need it.
**Timing:** ~1–2 s for the data + ~1–3 s to draw per report; several in parallel with a pool.

---

## Proposed API (REST over HTTPS)

### Make a report
```
POST /api/v1/reports
{
  "template": "uhnwi",                                         // a published report
  "client_id": "C0017",
  "period": { "start": "2026-07-01", "end": "2026-09-30" },    // or { "preset": "last_quarter" }
  "products": ["BROK", "BANK"],                                // or "all"
  "format": "pdf",                                             // pdf | pptx
  "language": "ru",                                            // optional
  "callback_url": "https://…"                                  // optional (async)
}
```

- **Synchronous** (one report): the response is the PDF itself.
- **Asynchronous**: `202 { "job_id": "…" }`, then
  - `GET /api/v1/jobs/{id}` → status, page count, layout warnings, data as-of date
  - `GET /api/v1/jobs/{id}/file` → the file
  - or a push to `callback_url` when done
- **Batch**: several clients in one async call (a quarter-end run from another system).

### Discovery
- `GET /api/v1/templates` — the published reports and the **parameters each accepts**, read from the template itself (a new template with new filters works without code changes).
- `GET /api/v1/clients` *(optional)* — the clients this caller may report on.

### Errors
| Code | When |
|---|---|
| `404` | unknown client or template |
| `403` | client or product outside this caller's access |
| `422` | invalid period or parameter (with the reason) |
| `429` | rate limit |

---

## Security and control

- **API clients are accounts**: each system gets its own (API key, or OAuth client credentials / mTLS), with its own access — clients, products, templates. Managed in **Admin**.
- **Every call logged** in Activity: caller, client, template, period, result, time taken.
- **Rate limits** per caller; an **idempotency key** so a retried call doesn't make two reports.
- **Client id → name**: the UHNWI template looks clients up by name (`clients.name = :client` in `src/uhnwi_sql.rs`); the API takes the bank's client id and translates it (or the template moves to ids).
- The bank's data never goes to an AI provider here — the API only renders reports.

---

## Relation to the web-app plan

Shares the server part with [web-server-mode.md](web-server-mode.md) (stage 1: a server binary built from the same core, one command layer). The API can come **first, as a smaller standalone service** — requests in, files out; no interactive users, sessions or document storage — and be the first step towards the web version.

---

## Plan

1. ✅ *(10 Nov 2026: `src/app/report_api.rs` — `make`, `draw`)* **Core as a library call**: "make report (template, client id, period, products, language) → report state", reusing `batch_make`; parameters validated against the template's parameter definitions.
2. ✅ *(10 Nov 2026: `src/render_js.rs`, cargo feature `render`; V8 152 + ICU 78; checked against Chrome by `dev/shots/api-render.mjs`)* **Renderer**: embedded JS engine running `ui/render.js` (text measured by `src/text.rs`) → SVG pages → `export::pdf`.
3. **HTTP service** (axum): the endpoints above; API-client accounts and access; logging; rate limits; idempotency.
4. **Jobs**: async queue, status, file store (local folder or S3-compatible), callbacks; batch requests.
5. **Admin**: API clients (create, keys, access, revoke) and their activity.
6. **Docs**: an OpenAPI description for the calling teams.
7. **Load test** at the expected volumes.

---

## Decisions (10 Nov 2026)

| Topic | Decision |
|---|---|
| Callers | Single reports (CRM, portal) and large batch runs (scheduler). Every request is a **job**: `202 { job_id }`, then status / file. Batch = many clients in one request. |
| Host | Inside `quire-server`, under `/api/v1/…`; shares its connections, templates, access and Activity. Not exposed by the desktop app. |
| Rendering | **Embedded JS engine** in the server running `ui/render.js` & co.; `textWidth` backed by the core's font code (`src/text.rs`) instead of a browser canvas. No Chrome. |
| Request | `client_id` and `period` required; `template` optional (defaults to UHNWI). |
| API parameters | **Admin configures** which extra parameters the API accepts per template (e.g. products, language) and their **default values**; left-out parameters take those defaults. Job status lists the values used. |
| Client id | The bank's client id; Quire maps it to the template's client. |
| Auth | **API keys**: an API account per calling system in Admin, with its own clients × products access; keys can be revoked. |
| Delivery | Download from Quire **and**, optionally, a copy to a shared store — a **folder/file share or an S3-compatible bucket** (Admin configures either; S3 keys sealed). |
| Notify | Polling `GET /jobs/{id}` always; optional `callback_url` POSTed when done, signed with the caller's key. |
| Retention | Files kept **7 days** (Admin can change), then deleted; shared-store copies untouched. |
| Formats | **PDF**; a **zip** of a batch's files. (No PowerPoint for now.) |
| Saving | Only the file — no document kept in Quire; Activity logs every request. |
