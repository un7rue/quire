# Quire report API

For teams whose systems ask Quire for reports: a CRM, a client portal, a scheduler. You send the client and the
period, Quire makes the report and you download the PDF (or a zip for a batch). The full description, for your
tools, is [`openapi.json`](openapi.json); a running server also serves it at `GET /api/v1/openapi.json`. The people
who run Quire set it up as described in the user guide, chapter 17 *Report API* (`ui/help/guide.md`).

- [Before you start](#before-you-start)
- [Quick start](#quick-start)
- [Asking for reports](#asking-for-reports)
- [Jobs and files](#jobs-and-files)
- [Errors](#errors)
- [Callbacks](#callbacks)
- [Limits](#limits)

## Before you start

Quire's administrator makes an **API account** for your system (Quire → Admin → API) and gives you its **key**,
which looks like `qk.<account>.<secret>`. Keep it like a password: Quire shows it once and keeps only a fingerprint.
The account decides:

- **which clients and products** your reports may cover (the same access a banker has);
- **which reports** you may ask for, and which of their settings you may choose (see `GET /api/v1/templates`);
- whether reports are **also copied** to a shared folder or bucket, and **which hosts** callbacks may go to.

If the key leaks, ask the administrator for a new one: the old key stops working at once.

Every request carries the key:

```
Authorization: Bearer qk.crm.Lf3…
```

## Quick start

```sh
KEY='qk.crm.…'
Q=https://quire.bank.local

# what you may ask for
curl -s -H "Authorization: Bearer $KEY" $Q/api/v1/templates

# a report for one client, last quarter
curl -s -H "Authorization: Bearer $KEY" -H 'Content-Type: application/json' \
     -d '{"client_id": "C0025", "period": {"preset": "last_quarter"}}' $Q/api/v1/reports
# → 202 {"job_id": "j4c1…", "status": "queued", …}

# its status, until "done"
curl -s -H "Authorization: Bearer $KEY" $Q/api/v1/jobs/j4c1…

# the PDF
curl -s -H "Authorization: Bearer $KEY" -o report.pdf $Q/api/v1/jobs/j4c1…/file
```

A report takes a second or two; a batch takes about that per client, a few at a time.

## Asking for reports

`POST /api/v1/reports` with a JSON body:

| Field | | |
|---|---|---|
| `client_id` | one report | The bank's id of the client (`clients.id`), like `C0025`. |
| `clients` | a batch | Several ids (up to 2000; each made once). Give `client_id` or `clients`, not both. |
| `period` | required | `{"start": "2026-07-01", "end": "2026-09-30"}`, or `{"preset": "ytd"}` — `ytd` (this year so far), `last_month`, `last_quarter` or `last_year`, counted back from the day of the request. |
| `template` | optional | A published report's id from `/api/v1/templates`; left out, the default one (`uhnwi`). |
| `params` | optional | The report's other settings, by name — only those `/api/v1/templates` lists. A text or number, or a list for one that takes several: `{"product": ["BROK", "BANK"]}`. Left out, the administrator's default for API requests applies, else the report's own. |
| `format` | optional | `"pdf"` (the only one for now). |
| `callback_url` | optional | Called when the job is done: see [Callbacks](#callbacks). |

Any other field is refused (a typo shouldn't pass unnoticed).

The request is checked before anything starts. If Quire can't make every report it asks for, it answers with the
reason and starts nothing. A refused batch lists each client refused, under `error.clients`.

**Retries.** Send an `Idempotency-Key` header (any text unique to the request, such as your own request id). Sending
the same request again with the same key returns the job made the first time, with `Idempotent-Replayed: true`,
instead of making the reports twice.

## Jobs and files

An accepted request answers `202` with a **job** (and `Location: /api/v1/jobs/<id>`). `GET /api/v1/jobs/<id>`:

```json
{
  "job_id": "j4c1…", "status": "done",
  "created": "2026-11-10T14:02:11+03:00", "finished": "2026-11-10T14:02:12+03:00", "expires": "2026-11-17T14:02:12+03:00",
  "template": "uhnwi", "period": {"start": "2026-07-01", "end": "2026-09-30"},
  "reports": 1, "done": 1, "failed": 0,
  "file": "/api/v1/jobs/j4c1…/file",
  "items": [{
    "client_id": "C0025", "client": "Anthony Edwardson", "status": "done",
    "pages": 18, "layout_issues": 0, "bytes": 1060550,
    "file": "/api/v1/jobs/j4c1…/files/C0025",
    "values": {"template": "uhnwi", "title": "Отчёт по активам и доходности", "client_id": "C0025", "client": "Anthony Edwardson",
               "period": {"start": "2026-07-01", "end": "2026-09-30", "label": "01.07.2026 – 30.09.2026"},
               "params": {"product": ["All"]}}
  }],
  "callback": null
}
```

- `status`: `queued`, `running`, then `done`, `partial` (some reports failed) or `failed`. Each item has its own
  status, and an `error` (`status`, `message`) if it failed.
- `values`: what the report was made with, defaults included: keep it with the file.
- `layout_issues`: pages where content was cut off or ran past the page. The PDF is still made; such a report is
  worth a look in Quire.
- `GET …/file`: the PDF for one client, or a zip of the batch's PDFs. Before the job is done it answers `409`.
- `GET …/files/<client_id>`: one client's PDF of a batch.
- Files are kept for the days the administrator set (7 to start; `expires`), then deleted. After that the job
  answers `404`.
- If the account copies reports to a shared folder or bucket, each item also has `copy` (`status`, `to`, `at` or
  `error`).

## Errors

Errors are JSON: `{"error": {"status": 422, "message": "…"}}`.

| Status | When |
|---|---|
| `400` | The body isn't JSON. |
| `401` | No key, a wrong one, or the account is switched off. |
| `403` | A client or product outside the account's access, a report it may not ask for or one kept off the API. |
| `404` | No such report, client or job (another account's job counts as missing). |
| `409` | The file isn't ready yet. |
| `422` | The request doesn't fit. For example: a bad period, a parameter the report doesn't take, a value it doesn't accept, or a callback host not allowed. The message says which. |
| `429` | Too many requests: wait the seconds in `Retry-After`. |
| `500` | Quire couldn't make the report; the message says why. Asking again may help. |

## Callbacks

Give `callback_url` and Quire calls it when the job is done: a `POST` of the job (the same JSON as
`GET /api/v1/jobs/<id>`). The host must be one the administrator allows for your account. A call that doesn't get
a `2xx` within 30 s is tried again after 10 s, 1 min and 5 min. The job's `callback` says how it went (`pending`,
`sent`, `failed`, with `attempts` and the last `code`).

Each call is signed, so you can tell it came from Quire:

```
X-Quire-Event: job.finished
X-Quire-Job: j4c1…
X-Quire-Timestamp: 1762772532
X-Quire-Signature: sha256=9f2c…
```

The signature is HMAC-SHA256 of `<timestamp>.<body>` (the body as received, byte for byte). It is keyed with the
**signing key**: the SHA-256 of your whole API key, as lowercase hex. Check it, and check the timestamp is recent:

```python
import hashlib, hmac, time

def from_quire(api_key: str, headers, body: bytes) -> bool:
    sign_key = hashlib.sha256(api_key.encode()).hexdigest().encode()
    ts = headers["X-Quire-Timestamp"]
    good = "sha256=" + hmac.new(sign_key, ts.encode() + b"." + body, hashlib.sha256).hexdigest()
    return hmac.compare_digest(good, headers["X-Quire-Signature"]) and abs(time.time() - int(ts)) < 300
```

```js
const crypto = require('node:crypto');

function fromQuire(apiKey, headers, body /* a Buffer */) {
  const signKey = crypto.createHash('sha256').update(apiKey).digest('hex');
  const ts = headers['x-quire-timestamp'];
  const good = 'sha256=' + crypto.createHmac('sha256', signKey).update(`${ts}.`).update(body).digest('hex');
  const got = String(headers['x-quire-signature'] || '');
  return got.length === good.length && crypto.timingSafeEqual(Buffer.from(good), Buffer.from(got))
    && Math.abs(Date.now() / 1000 - Number(ts)) < 300;
}
```

After a new key the signing key changes with it.

## Limits

- 60 new requests a minute per account; 1200 other requests (status, downloads). Past that: `429` with `Retry-After`.
- 2000 clients per request.
- A request body of 300 MB at most, answered within 2 minutes (the job itself has no limit).
