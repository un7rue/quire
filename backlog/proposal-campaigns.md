# Proposal campaigns in batch reports

> **Status:** idea — discussed 23 Oct 2026, not started
> **Goal:** a banker distributes **the same proposal to a list of clients**: written once, put into each client's report in a batch run, then tracked client by client.

---

## Summary

A **campaign**: one proposal + a batch of clients. Each client's report gets the proposal (before the legal text, laid out and fit-checked as usual); each client gets **their own entry in the register** with their own status, linked to the campaign; a campaign summary shows how it went.

---

## What's already there

| Need | Today |
|---|---|
| One report per client, many at once | Batch reports — `ui/batch.js`, `batch_make` / `batch_write` in `src/main.rs` (each built apart from the open documents) |
| A proposal page from notes, a file or the AI | `src/proposal.rs` (`plain`, `build`, `extract`), `proposal_apply_as` in `src/main.rs`, `ui/proposal.js` |
| Fit check, two pages, a continuing actions table | `proposal_apply_as` (one page → two; `paginate` on the actions table) |
| The register: per-proposal record, status, history, versions | `src/proposals.rs` (`proposals.json`), Admin → Proposals (`ui/admin.js`) |
| Exported → *Presented* | `proposals_went_out` in `src/main.rs` |
| Placeholders filled from a report's data | `{{data:dataset.field}}` in texts (`vm::expand`), e.g. `{{data:port.value}}` |
| Situations per client | "Needs attention" items — `src/overview.rs` (idle cash, risk breach, maturities, withdrawals) |

---

## The idea

### 1. A proposal step in the batch window *(optional)*
Add the proposal once:
- typed, or from a Word / PowerPoint file
- one of the banker's earlier proposals, reused
- written up by the AI **once**, from notes

### 2. How it adapts to each client

| Level | What differs per client | Cost / risk |
|---|---|---|
| **A. Same text for everyone** | Nothing ("New OFZ issue: we suggest placing part of your free funds…") | Simplest; no AI per client |
| **B. Client figures filled in** *(recommended with A)* | Placeholders filled from each client's data: "Свободные деньги — **{{idle cash}}**", "Возвращается **{{maturing}}** до {{date}}" — chosen from a list while writing | No AI; numbers always right |
| **C. Tailored by AI per client** *(later)* | Wording and amounts adapted to each report | One AI request per client; each needs a review before it goes out |

Build **A + B** first; **C** later, with a review queue.

### 3. Choosing clients by situation
Filters in the batch window's client picker, from "Needs attention":
- clients with **idle cash**
- clients with **shares above their risk profile**
- clients with a **deposit / bond maturing** within N days
- clients with a **large withdrawal**

("Offer bonds to everyone with idle cash" = one click.)

### 4. Tracking
- One register entry **per client**, own status (one accepts, another declines), with a `campaign` link (name, banker, date).
- *Presented* when the batch exports it.
- **Campaign summary**: clients reached / presented / accepted / declined / waiting — beside the batch for the banker, and in Admin → Proposals.
- "Follow up on the proposal" per client after 14 days, as today.

### 5. Safeguards
- **Preview before running**: two or three clients' proposal pages with their figures filled in.
- **Missing figure** (e.g. a client has no idle cash) → that client is listed and **skipped**, not sent a "0 ₽" offer.
- **Approval**: a mass offer may need marketing / compliance sign-off — an "approved by" step if the bank requires it.

---

## What it takes

| Part | Notes |
|---|---|
| **Placeholders for proposals** | A curated list of per-client figures (idle cash, maturing amount and date, share in shares, value, return…) that map to report data (`{{data:port.…}}` or new fields in the `port` source, `src/uhnwi_sql.rs`); a picker in the proposal box; `proposal::build` keeps them in the texts so each report fills its own. A check that every placeholder resolves for the client (else skip). |
| **Batch + proposal** | `batch_make` gets an optional proposal spec → `SetProposal` on the client's report before drawing (the fit check per client — figures change lengths). |
| **Campaign record** | In `src/proposals.rs`: a `campaign` id/name on each entry; a campaign list/summary call. |
| **Client filters** | The batch dialog reads `my_overview` attention items to filter clients. |
| **UI** | Proposal step and preview in `ui/batch.js`; campaign summary in the batch panel and in Admin → Proposals (`ui/admin.js`). |
| **Tests** | Placeholders filled per client; a client without the figure skipped; one register entry per client with the campaign link; *Presented* on export. |

---

## Open questions

1. **Personalisation level** — same text, figures filled in, or AI-tailored?
2. **Client selection** — situation filters, or a manual list only?
3. **Approval** — must campaign proposals be signed off before they go out?
4. **Several proposals** — can a client's batch report carry their own proposal *and* the campaign one, or only the campaign's?
5. **Naming** — what does the client see: the campaign's title as written, or a per-client title?

---

## Related

- [act-on-needs-attention.md](act-on-needs-attention.md) — the same situations drive the client filters; a campaign's follow-ups land there.
- [report-api.md](report-api.md) — an API call could start a campaign batch from another system.
