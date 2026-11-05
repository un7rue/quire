# Demo database

`demo.duckdb` is a private bank's data for building and testing reports. It is generated, not real:
client and sales names are inspired by NBA players and well-known investors, ISINs are well-formed but invented.

Rebuild it (deterministic, about 10 seconds; ends with self-checks):

    python3 data/make_demo_db.py

## The test client

**Кирилл Шарихин** (`C1337`, born 1984-09-20, sales person Warren Buffington, profile «Консервативный») is the client of the
UHNWI report template, kept in the same tables as every other client: 16 accounts in `accounts` (brokerage 400PX0F
and an IIS 510IIS7, asset management DU.SBOLPF.256730, an endowment policy INS-777120, savings, deposits, payment
accounts, metal accounts, a card), their positions in
`balances`, daily results in `portf_acct` and trades in `operations`, simulated like the other clients' from
positions sized to the template's figures on 2026-06-15 (about 355 million roubles).

The template reads every source with SQL on these tables (connection “Demo bank”, queries in `src/uhnwi_sql.rs`),
for the client picked in the toolbar (`:client`) and the report period (`:period_start`, `:period_end`), so any
client's report can be built. A period's return is `net_twr_ltd` on its last day / `net_twr_ltd` the day before it
starts − 1; for a group of accounts (all, the bank's, …) the daily `net_twr` comes from the accounts' summed
balances and flows, with the same formula. Similar clients are the clients with the same risk profile.

## Tables

The first six follow `database.md`; the others were added so reports have trades, payment calendars, market data
and benchmarks.

| Table | Rows | What it holds |
|---|---|---|
| `sales` | 8 | Sales people (relationship managers) |
| `clients` | 41 | Clients (with their risk profile), each with a sales person; C1337 is the test client |
| `accounts` | 155 | Brokerage (BROK, IIS too), bank (BANK), asset management (AM, with its `manager`) and insurance (INS, with `policy_end_dt`) accounts; `close_dt` = 3000-01-01 when open |
| `instruments` | 127 | Equities, bonds, funds, currencies, metals, deposits, a savings product, insurance policies |
| `balances` | ~1.28M | Every position of every open account, every calendar day, with its average purchase price (`avg_price`); insurance policies also their surrender value (`redeem_amt`) |
| `portf_acct` | ~137k | Every open account, every calendar day: balances, flows, income, fees, taxes and returns |
| `operations` | ~12.7k | Top-ups, withdrawals, buys and sells (with `venue` and `fee`), conversions, coupons, redemptions, dividends, fees, taxes |
| `cashflow_schedule` | 769 | Payment calendar per instrument to 2027-12-31: coupons, redemptions, dividends, interest (amount per unit) |
| `fx_rates` | ~5.5k | RUB per USD, CNY, EUR, AED (and RUB = 1), every calendar day |
| `prices` | ~86k | Market prices of securities, funds and metals, every calendar day |
| `benchmarks` | ~6.6k | IMOEX, MCFTR, RGBI, S&P 500, the key rate and consumer prices |
| `products` | 4 | The products (source systems) `accounts.src_cd` names — Russian and English name and description, display order; Admin shows these |

Period: 2024-01-01 to 2026-12-31. Base currency: RUB.

## Rules the numbers follow

- **Markets move on working days only.** On weekends and holidays prices and FX carry over; bond coupons, deposit and
  savings interest and insurance guarantees still accrue, so returns are slim but not exactly 1.
- **Bond prices are dirty**: they include the accrued coupon, as a fraction of nominal: `amt = qty × price × nominal`.
  The price drops back on a coupon date, when the coupon is paid in cash.
- **Cash, deposits and savings** have price 1; their `qty` is the amount, growing with interest.
- **Flows**: inflows are positive, outflows negative (`net_flow`, and `Пополнение` / `Вывод` in `operations`).
  Fees and taxes are negative and are **not** flows.
- `bal_in` = the previous day's `bal_out`; `bal_out` = the sum of `amt_rub` of the account's positions that day.
- `net_twr = bal_out / (bal_in + net_flow)` when `net_flow ≥ 0`, else `(bal_out − net_flow) / bal_in`.
  `gross_twr` is the same with fees and taxes added back (`bal_out − fees − taxes`).
  `net_twr_ltd` / `gross_twr_ltd` are their running products from the account's first day.
- `income` is the day's accrued interest (bond coupons, deposits, savings, policy guarantees) plus dividends.
- An account opened during the period starts with a top-up; a closed one ends with everything paid out (`bal_out` = 0).
- A few accounts pay a large one-off tax, so some days fall outside 0.9–1.1.

The generator's self-checks verify each of these on every row.
