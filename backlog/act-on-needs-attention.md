# Act on "Needs attention"

> **Status:** idea — discussed 22 Oct 2026, the user is thinking about it
> **Goal:** turn the client list's "Needs attention" from something the banker *reads* into something they *work through*: each item can be done, snoozed or followed up, and what happened is remembered.

---

## Where it stands today

The banker's client list (Reports area) shows, per client and in the **Needs attention** card:

- large withdrawals (last 30 days, ≥ 5% of the portfolio or ≥ 10 M ₽)
- shares above the risk profile's limit (Conservative 40%, Moderate 60%, Moderately aggressive 80% — assumed limits)
- deposits and bonds maturing, coupons and dividends (next 30 days)
- idle cash (≥ 10% of the portfolio and ≥ 5 M ₽)
- insurance policies ending (90 days), birthdays (14 days)

Each item opens its details under the client's row (what, when, how much, accounts, why it matters). But nothing records **what the banker did about it**: the same items show every day until their date passes, and nobody can see whether a large withdrawal was followed up.

---

## The idea

### 1. Actions on each item
- **Done** — handled; it leaves the list.
- **Snooze** — 1 day / 1 week / until a date; it comes back then.
- **Not relevant** — dismissed, with an optional reason.
- **Log contact** — call / meeting / e-mail, with a short note ("Called: will reinvest in bonds").

Available on the item in **Needs attention**, on the note by the client's name, and in the details under the row.

### 2. Only what's new
- A handled item (the same coupon, maturity or flag) isn't raised again.
- A flag that comes back changed (e.g. shares went from 51% to 58%) is raised again as new.
- The client's row shows the last contact: *"Called 21 Oct — will reinvest in bonds"*.

### 3. History per client
A timeline under the client's row in the list, combining:
- contacts logged
- items done / snoozed / dismissed
- reports made and exported (already in the activity log)
- proposals made (and, later: sent, discussed)

### 4. Into the report's AI
- The **brief and pitch** know the history: *"last time the client asked about the deposit maturing on 21 Nov"*.
- The **e-mail** can refer back to the last conversation.
- **Proposal ideas** can build on what was discussed (and not suggest what was already declined).

### 5. For the administrator
In **Admin → Activity**:
- items handled vs. ignored, per banker
- how long large withdrawals waited before someone called
- the measure of whether the list is used at all

### 6. A nudge
A count on the **Reports** icon in the sidebar for new urgent items since the last visit (a large withdrawal, a risk-profile breach).

---

## What it takes

| Part | Notes |
|---|---|
| **A small store of item states and contacts** | Kept in Quire's own data (like accounts, groups, the activity log), **not** in the bank's database. Per item: client, kind, a key that identifies it (e.g. instrument + date, or flag + level), state (done / snoozed until / dismissed), who, when, note. Per contact: client, kind, date, note, who. |
| **Item identity** | A stable key per item so "the same coupon" is recognised tomorrow; a changed flag gets a new key. Items are worked out in `src/overview.rs`. |
| **Overview filters** | `overview()` leaves out handled / snoozed items (or marks them), and returns each client's last contact. |
| **UI** | An action menu on items and on the row's note; a "Log contact" form; a timeline in the details under the row (`ui/areas.js`, the client list). |
| **AI** | Recent contacts and handled items added to what the report assistant reads (`src/ai_report.rs` — kept masked like the rest). |
| **Admin** | New views in Activity (`ui/admin.js`), from the same store. |
| **Access** | A banker sees and acts on their own clients' items; an administrator sees all. |

**Effort:** moderate — one store, a menu and a form, a timeline view; the overview and the AI pick it up from there.

---

## Open questions

1. **Shared or personal?** If two bankers cover the same client, does "done" by one hide the item for the other?
2. **Snooze options** — fixed (1 day / 1 week / until a date) or free?
3. **Contacts** — logged only in Quire, or also from the bank's CRM if there is one (read-only)?
4. **Retention** — how long to keep contact notes (they may contain client details)?
5. **Notifications** — only the count on the sidebar, or also e-mail / a daily digest?

---

## Related

- Client list and Needs attention: `src/overview.rs`, `ui/areas.js` (`userArea`), guide section *"Your clients at a glance"* in `ui/help/guide.md`.
- Activity log and sessions: `src/audit.rs`.
- A related idea also deferred: a **review & send workflow** (Draft → Checked → Approved → Sent, with an approver and a log of what was sent to whom) — the user was unsure about the workflow; the client list already shows each client's last report and export.
