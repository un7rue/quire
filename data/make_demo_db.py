#!/usr/bin/env python3
"""Builds Quire's demo database: data/demo.duckdb.

A private bank's data: clients and their sales people, accounts (brokerage,
bank, asset management, insurance), instruments, daily balances per position
and daily account returns. Deterministic (fixed seed): run it again to
rebuild the same file. Ends with self-checks that the numbers reconcile.

    python3 data/make_demo_db.py            # writes data/demo.duckdb

Rules the data follows (see database.md):
- every calendar day from 2024-01-01 to 2026-12-31 for each open account;
- markets move on business days only; on weekends and holidays prices and FX
  carry over, but bond interest (accrued coupon) and deposit interest still
  accrue, so returns are slim but not exactly 1;
- inflows are positive, outflows negative; fees and taxes are negative and
  are not flows;
- bal_in = previous bal_out; bal_out = sum of amt_rub of the account's
  positions that day;
- net_twr  = bal_out / (bal_in + net_flow)              when net_flow >= 0
           = (bal_out - net_flow) / bal_in              when net_flow < 0
  gross_twr is the same with fees and taxes added back (bal_out - fees - taxes);
- bond prices are dirty (accrued coupon included), as a fraction of nominal:
  amt = qty * price * nominal; deposits, cash and savings have price 1 and
  their qty is the amount (interest makes it grow).
"""

import csv
import datetime as dt
import math
import os
import shutil
import sys

import duckdb
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, 'demo.duckdb')
TMP = os.path.join(HERE, '_build')

START, END = dt.date(2024, 1, 1), dt.date(2026, 12, 31)
DAY0 = START - dt.timedelta(days=1)  # market values the day before the data starts
rng = np.random.default_rng(20260630)

DAYS = [START + dt.timedelta(days=i) for i in range((END - START).days + 1)]
MKT_DAYS = [DAY0] + DAYS
IDX = {d: i for i, d in enumerate(MKT_DAYS)}

# Russian non-working days (approximate production calendar)
HOLIDAYS = set()
for y in (2024, 2025, 2026):
    for m, d in [(1, 1), (1, 2), (1, 3), (1, 4), (1, 5), (1, 6), (1, 7), (1, 8), (2, 23), (3, 8), (5, 1), (5, 9), (6, 12), (11, 4), (12, 31)]:
        HOLIDAYS.add(dt.date(y, m, d))


def bday(d):
    return d.weekday() < 5 and d not in HOLIDAYS


def cents(x):
    return int(round(x * 100))


def money(c):
    """Cents as a decimal string."""
    return f'{"-" if c < 0 else ""}{abs(c) // 100}.{abs(c) % 100:02d}'


def first_bday(d):
    """The first working day on or after d."""
    while not bday(d):
        d += dt.timedelta(days=1)
    return d


def interp(points, days):
    """A path through dated anchor points, linear between them."""
    pts = sorted(points)
    out = []
    for d in days:
        if d <= pts[0][0]:
            out.append(pts[0][1])
            continue
        for (d0, v0), (d1, v1) in zip(pts, pts[1:]):
            if d0 <= d <= d1:
                t = (d - d0).days / max(1, (d1 - d0).days)
                out.append(v0 + (v1 - v0) * t)
                break
        else:
            out.append(pts[-1][1])
    return np.array(out)


def isin(country):
    """A well-formed ISIN (country + 9 characters + Luhn check digit), not a real one."""
    alnum = '0123456789ABCDEFGHJKLMNPQRSTUVWXYZ'
    body = country + ('000A' if country == 'RU' else '') + ''.join(rng.choice(list(alnum), 9 - (4 if country == 'RU' else 0)))
    digits = ''.join(str(int(c, 36)) for c in body)
    total, dbl = 0, True
    for ch in reversed(digits):
        n = int(ch) * (2 if dbl else 1)
        total += n // 10 + n % 10
        dbl = not dbl
    return body + str((10 - total % 10) % 10)


# ---------------------------------------------------------------- market

N = len(MKT_DAYS)
BD = np.array([bday(d) for d in MKT_DAYS])


def walk(daily_vol, drift=0.0, shocks=None, factor=None, beta=1.0):
    """Log-returns on business days only (0 on non-working days)."""
    r = np.where(BD, rng.normal(drift, daily_vol, N), 0.0)
    if factor is not None:
        r = r + beta * factor
    for d, s in (shocks or {}).items():
        if d in IDX:
            r[IDX[d]] += s
    r[0] = 0.0
    return r


# market factors: common moves for Russian and US equities, with real-looking shock days
RU_SHOCKS = {dt.date(2024, 5, 30): -0.03, dt.date(2024, 7, 29): -0.025, dt.date(2024, 10, 25): -0.03, dt.date(2024, 12, 20): 0.05,
             dt.date(2025, 2, 12): 0.055, dt.date(2025, 2, 13): 0.03, dt.date(2025, 4, 7): -0.045, dt.date(2025, 9, 12): -0.02, dt.date(2026, 3, 16): -0.035}
US_SHOCKS = {dt.date(2024, 8, 5): -0.03, dt.date(2025, 4, 3): -0.05, dt.date(2025, 4, 4): -0.06, dt.date(2025, 4, 9): 0.09, dt.date(2026, 2, 3): -0.025}
RU_MKT = walk(0.009, 0.0001, RU_SHOCKS)
US_MKT = walk(0.008, 0.0005, US_SHOCKS)
RATES = walk(0.0012)  # Russian yields (bond prices move against it)

# FX: RUB per unit, moving on business days through real-looking anchors
FX_ANCHORS = {
    'USD': [(DAY0, 89.69), (dt.date(2024, 6, 30), 85.75), (dt.date(2024, 11, 27), 113.5), (dt.date(2025, 1, 1), 101.7), (dt.date(2025, 4, 1), 84.1),
            (dt.date(2025, 7, 1), 78.5), (dt.date(2025, 10, 1), 82.6), (dt.date(2026, 1, 1), 79.8), (dt.date(2026, 6, 30), 78.4), (dt.date(2026, 9, 30), 82.0), (END, 84.5)],
}


def fx_path(anchors, vol):
    base = interp(anchors, MKT_DAYS)
    noise = np.zeros(N)
    for i in range(1, N):
        noise[i] = noise[i - 1] * 0.97 + (rng.normal(0, vol) if BD[i] else 0.0)
    path = base * np.exp(noise)
    for i in range(1, N):  # no moves on non-working days
        if not BD[i]:
            path[i] = path[i - 1]
    return np.round(path, 4)


FX = {'RUB': np.ones(N)}
FX['USD'] = fx_path(FX_ANCHORS['USD'], 0.004)
FX['CNY'] = np.round(FX['USD'] / interp([(DAY0, 7.10), (dt.date(2025, 4, 1), 7.30), (END, 7.15)], MKT_DAYS) * np.exp(walk(0.0015).cumsum() * 0.3), 4)
FX['EUR'] = np.round(FX['USD'] * interp([(DAY0, 1.10), (dt.date(2025, 1, 1), 1.04), (dt.date(2025, 7, 1), 1.17), (END, 1.16)], MKT_DAYS), 4)
FX['AED'] = np.round(FX['USD'] / 3.6725, 4)
for c in ('CNY', 'EUR', 'AED'):
    for i in range(1, N):
        if not BD[i]:
            FX[c][i] = FX[c][i - 1]


def series(p0, rets):
    return p0 * np.exp(np.cumsum(rets))


# ---------------------------------------------------------------- instruments

instruments = []  # rows for the instruments table
PRICE = {}        # instr_cd -> price per market day (np.array); deposits/cash/savings have none (price 1)
META = {}         # instr_cd -> dict(kind, cur, nominal, coupon, maturity, region, ...)
_seq = [0]


def new_cd():
    _seq[0] += 1
    return f'I{_seq[0]:04d}'


def add(kind, row, cur, price=None, **meta):
    cd = new_cd()
    instruments.append({'instr_cd': cd, 'instr_cur': cur, **row})
    META[cd] = {'kind': kind, 'cur': cur, **meta}
    if price is not None:
        PRICE[cd] = price
    return cd


TYPE_RU = {'Equity': 'Акции', 'Bonds': 'Облигации', 'Funds': 'Фонды', 'Currency': 'Денежные средства', 'Metals': 'ОМС',
           'Deposit': 'Депозиты', 'Savings': 'Накопительные счета', 'Insurance': 'Страхование'}


def inst(issuer, name, typ, isin_cc, nominal=None, maturity=None, coupon=None):
    return {'isin': isin(isin_cc) if isin_cc else None, 'issuer_cd': issuer, 'instr_name': name, 'instr_type': typ,
            'instr_type_ru': TYPE_RU[typ], 'instr_nominal': nominal, 'maturity_dt': maturity, 'coupon': coupon}


# currencies
CASH = {}
for c, name in [('RUB', 'Рубли РФ'), ('USD', 'Доллар США'), ('CNY', 'Китайский юань'), ('EUR', 'Евро'), ('AED', 'Дирхам ОАЭ')]:
    CASH[c] = add('cash', inst(c, name, 'Currency', None), c)

# Russian equities: ticker, name, price on 2023-12-31, beta, dividend yield, dividend month
RU_EQ = [('SBER', 'Сбербанк', 271.0, 1.0, 0.11, 7), ('GAZP', 'Газпром', 162.0, 0.9, 0.0, 7), ('LKOH', 'Лукойл', 6900.0, 0.9, 0.12, 6),
         ('ROSN', 'Роснефть', 560.0, 0.9, 0.08, 7), ('NVTK', 'Новатэк', 1500.0, 1.1, 0.05, 5), ('GMKN', 'Норникель', 160.0, 1.0, 0.0, 6),
         ('YDEX', 'Яндекс', 3900.0, 1.3, 0.02, 10), ('TATN', 'Татнефть', 650.0, 0.9, 0.12, 7), ('MTSS', 'МТС', 260.0, 0.8, 0.13, 7),
         ('PLZL', 'Полюс', 11500.0, 0.8, 0.03, 6), ('MGNT', 'Магнит', 7300.0, 1.0, 0.09, 6), ('CHMF', 'Северсталь', 1650.0, 1.1, 0.08, 6),
         ('NLMK', 'НЛМК', 190.0, 1.1, 0.07, 6), ('ALRS', 'Алроса', 72.0, 1.0, 0.06, 7), ('MOEX', 'Мосбиржа', 200.0, 0.9, 0.09, 5),
         ('PHOR', 'ФосАгро', 6500.0, 0.7, 0.08, 6), ('AFLT', 'Аэрофлот', 40.0, 1.3, 0.04, 7), ('OZON', 'Озон', 2600.0, 1.4, 0.0, 6),
         ('SNGS', 'Сургутнефтегаз', 30.0, 0.7, 0.03, 7), ('MAGN', 'ММК', 54.0, 1.1, 0.06, 6), ('HHRU', 'Хэдхантер', 4000.0, 1.2, 0.07, 9),
         ('TCSG', 'Т-Технологии', 2900.0, 1.3, 0.03, 7), ('SVCB', 'Совкомбанк', 198.0, 1.0, 0.07, 7), ('AQUA', 'Инарктика', 650.0, 0.9, 0.06, 6),
         ('HNFG', 'Хэндерсон', 580.0, 1.1, 0.05, 6), ('VTBR', 'Банк ВТБ', 95.0, 1.2, 0.0, 7)]
US_EQ = [('AAPL', 'Apple', 192.0, 1.1, 0.005, 5), ('MSFT', 'Microsoft', 376.0, 1.0, 0.008, 6), ('NVDA', 'Nvidia', 49.5, 1.8, 0.0, 6),
         ('AMZN', 'Amazon', 151.0, 1.3, 0.0, 6), ('GOOGL', 'Alphabet', 140.0, 1.1, 0.004, 6), ('META', 'Meta Platforms', 354.0, 1.4, 0.004, 6),
         ('TSLA', 'Tesla', 248.0, 2.0, 0.0, 6), ('JPM', 'JPMorgan Chase', 170.0, 1.0, 0.024, 7), ('BRKB', 'Berkshire Hathaway', 357.0, 0.8, 0.0, 6),
         ('V', 'Visa', 260.0, 0.9, 0.008, 6), ('KO', 'Coca-Cola', 59.0, 0.5, 0.03, 6), ('JNJ', 'Johnson & Johnson', 157.0, 0.5, 0.03, 6),
         ('XOM', 'Exxon Mobil', 100.0, 0.8, 0.035, 6), ('NFLX', 'Netflix', 487.0, 1.3, 0.0, 6), ('DIS', 'Walt Disney', 90.0, 1.1, 0.01, 7)]
EQUITY_RU, EQUITY_US = [], []
for tk, name, p0, beta, dy, dm in RU_EQ:
    EQUITY_RU.append(add('equity', inst(tk, name, 'Equity', 'RU'), 'RUB', series(p0, walk(0.011, 0.0, factor=RU_MKT, beta=beta)), div=dy, div_month=dm, region='RU'))
for tk, name, p0, beta, dy, dm in US_EQ:
    EQUITY_US.append(add('equity', inst(tk, name, 'Equity', 'US'), 'USD', series(p0, walk(0.012, 0.0002, factor=US_MKT, beta=beta)), div=dy, div_month=dm, region='US'))

# funds
gold_rub = interp([(DAY0, 5900.0), (dt.date(2024, 11, 27), 9200.0), (dt.date(2025, 4, 1), 8700.0), (dt.date(2025, 10, 15), 10600.0), (END, 10200.0)], MKT_DAYS) * np.exp(walk(0.009).cumsum() * 0.6)
for _i in range(1, N):  # markets are closed on non-working days
    if not BD[_i]:
        gold_rub[_i] = gold_rub[_i - 1]
FUNDS_RU, FUNDS_US = [], []
for name, p0, how in [('БПИФ «Капитал – Топ российских акций»', 17.5, 'ru'), ('БПИФ «Капитал – Корпоративные облигации»', 18.9, 'bond'),
                      ('БПИФ «Капитал – Государственные облигации»', 11.0, 'bond'), ('БПИФ «Золото. Биржевой»', 1.33, 'gold'),
                      ('БПИФ «Денежный рынок»', 1.0, 'mm'), ('БПИФ «Капитал – Сбалансированный»', 12.0, 'mix'),
                      ('БПИФ «Капитал – Сбережения в юанях»', 11.0, 'cny'), ('ПИФ «БКС Дивидендные акции»', 4500.0, 'ru')]:
    if how == 'ru':
        p = series(p0, walk(0.003, 0.0, factor=RU_MKT, beta=1.0))
    elif how == 'bond':
        p = series(p0, walk(0.002, 0.0004, factor=-RATES, beta=3.0))
    elif how == 'gold':
        p = p0 * gold_rub / gold_rub[0]
    elif how == 'mm':
        p = series(p0, np.where(BD, 0.17 / 250, 0.0))
    elif how == 'cny':
        p = p0 * FX['CNY'] / FX['CNY'][0] * series(1.0, np.where(BD, 0.05 / 250, 0.0))
    else:
        p = series(p0, walk(0.002, 0.0002, factor=RU_MKT * 0.5 - RATES, beta=1.0))
    FUNDS_RU.append(add('fund', inst('UKK', name, 'Funds', 'RU'), 'RUB', np.round(p, 4), region='RU'))
for tk, name, p0, how in [('SPY', 'SPDR S&P 500 ETF', 475.0, 'eq'), ('QQQ', 'Invesco QQQ Trust', 410.0, 'tech'),
                          ('TLT', 'iShares 20+ Year Treasury Bond ETF', 98.0, 'bond'), ('GLD', 'SPDR Gold Shares', 190.0, 'gold')]:
    if how == 'eq':
        p = series(p0, walk(0.002, 0.0, factor=US_MKT, beta=1.0))
    elif how == 'tech':
        p = series(p0, walk(0.004, 0.0, factor=US_MKT, beta=1.25))
    elif how == 'bond':
        p = series(p0, walk(0.009, -0.0001))
    else:
        p = p0 * (gold_rub / FX['USD']) / (gold_rub[0] / FX['USD'][0])
    FUNDS_US.append(add('fund', inst(tk, name, 'Funds', 'US'), 'USD', np.round(p, 4), region='US'))

# metals (bank metal accounts), price per gram in roubles
METALS = {}
for code, name, path in [('GOLD', 'ОМС золото', gold_rub),
                         ('SILVER', 'ОМС серебро', interp([(DAY0, 74.0), (dt.date(2025, 3, 1), 98.0), (END, 118.0)], MKT_DAYS) * np.exp(walk(0.013).cumsum() * 0.5)),
                         ('PLATINUM', 'ОМС платина', interp([(DAY0, 2800.0), (dt.date(2025, 1, 1), 3050.0), (END, 3600.0)], MKT_DAYS) * np.exp(walk(0.012).cumsum() * 0.5)),
                         ('PALLADIUM', 'ОМС палладий', interp([(DAY0, 3050.0), (dt.date(2025, 2, 1), 2700.0), (END, 3150.0)], MKT_DAYS) * np.exp(walk(0.015).cumsum() * 0.5))]:
    p = path.copy()
    for i in range(1, N):
        if not BD[i]:
            p[i] = p[i - 1]
    METALS[code] = add('metal', inst(code, name, 'Metals', None), 'RUB', np.round(p, 2))

# bonds: issuer, coupon %, maturity, currency, starting clean price
BOND_LIST = [('RU', 6.0, dt.date(2024, 11, 15), 'RUB', 0.985), ('RU', 7.7, dt.date(2026, 3, 18), 'RUB', 0.965), ('RU', 7.0, dt.date(2027, 2, 3), 'RUB', 0.93),
             ('RU', 6.9, dt.date(2029, 7, 23), 'RUB', 0.84), ('RU', 7.65, dt.date(2033, 4, 6), 'RUB', 0.80), ('RU', 7.25, dt.date(2034, 5, 10), 'RUB', 0.78),
             ('RU', 12.0, dt.date(2037, 1, 21), 'RUB', 0.98), ('RU', 7.7, dt.date(2039, 3, 16), 'RUB', 0.76), ('RU', 6.1, dt.date(2025, 8, 27), 'RUB', 0.975),
             ('SBER', 9.5, dt.date(2026, 10, 30), 'RUB', 0.96), ('SBER', 13.0, dt.date(2028, 4, 25), 'RUB', 1.0), ('GAZP', 8.1, dt.date(2027, 6, 18), 'RUB', 0.92),
             ('GAZP', 10.2, dt.date(2025, 6, 4), 'RUB', 0.99), ('RZD', 8.9, dt.date(2029, 9, 12), 'RUB', 0.88), ('RZD', 14.5, dt.date(2031, 2, 20), 'RUB', 1.01),
             ('LKOH', 9.0, dt.date(2027, 11, 5), 'RUB', 0.95), ('ROSN', 11.0, dt.date(2028, 8, 15), 'RUB', 0.97), ('MTSS', 12.5, dt.date(2026, 12, 10), 'RUB', 0.99),
             ('US', 4.0, dt.date(2028, 2, 15), 'USD', 0.975), ('US', 4.375, dt.date(2034, 5, 15), 'USD', 0.985), ('US', 2.4, dt.date(2038, 3, 31), 'USD', 0.82),
             ('US', 5.0, dt.date(2025, 9, 30), 'USD', 1.0), ('AAPL', 4.4, dt.date(2030, 5, 11), 'USD', 0.99), ('JPM', 4.9, dt.date(2032, 7, 25), 'USD', 0.98)]


def coupon_dates(maturity):
    """Semiannual coupon dates backwards from maturity, enough to cover the data."""
    out, d = [], maturity
    while d >= DAY0 - dt.timedelta(days=200):
        out.append(d)
        y, m = (d.year, d.month - 6) if d.month > 6 else (d.year - 1, d.month + 6)
        d = dt.date(y, m, min(d.day, 28))
    return sorted(out)


BONDS_RU, BONDS_US = [], []
for issuer, cpn, mat, cur, clean0 in BOND_LIST:
    cds = coupon_dates(mat)
    total_days = max(30, (mat - DAY0).days)
    noise, clean, accr = 0.0, np.zeros(N), np.zeros(N)
    last_bd = DAY0
    for i, d in enumerate(MKT_DAYS):
        if BD[i]:
            last_bd = d
        if d > mat:
            clean[i] = np.nan
            continue
        if BD[i] and i > 0:
            noise = noise * 0.985 + rng.normal(0, 0.0018) - (RATES[i] * 2.5 if cur == 'RUB' else 0.0)
        # pulled to par as maturity nears; the clean price moves on working days only
        pull = (mat - last_bd).days / total_days
        clean[i] = 1 + (clean0 - 1) * pull + noise * min(1.0, (mat - last_bd).days / 365)
        prev = max(x for x in cds if x <= d) if any(x <= d for x in cds) else d - dt.timedelta(days=182)
        nxt = min(x for x in cds if x > d) if any(x > d for x in cds) else mat
        accr[i] = cpn / 200 * (d - prev).days / max(1, (nxt - prev).days)
    name = f'{issuer}-{mat:%Y%m%d}-{cpn:g}'
    cd = add('bond', inst(issuer, name, 'Bonds', 'RU' if cur == 'RUB' else 'US', nominal=1000, maturity=mat, coupon=cpn), cur,
             np.round(clean + accr, 6), coupon=cpn, maturity=mat, coupons=set(cds), nominal=1000, accr=accr)
    (BONDS_RU if cur == 'RUB' else BONDS_US).append(cd)

# savings account product (one instrument, rate as its coupon)
SAVINGS = add('savings', inst('BANK', 'Накопительный счёт', 'Savings', None, coupon=14.0), 'RUB', rate=0.14)

# ---------------------------------------------------------------- people

SALES = [('Warren Buffington', 'warren.buffington'), ('Ray Dalinsky', 'ray.dalinsky'), ('Peter Lynchfield', 'peter.lynchfield'),
         ('Cathie Woodward', 'cathie.woodward'), ('Jack Boglesby', 'jack.boglesby'), ('Howard Marksman', 'howard.marksman'),
         ('Abby Cohenridge', 'abby.cohenridge'), ('Bill Ackmanson', 'bill.ackmanson')]
sales = [{'id': f'S{i + 1:04d}', 'name': n, 'sales_phone': f'8-903-{rng.integers(1000000, 9999999)}', 'sales_email': f'{e}@pb.com'} for i, (n, e) in enumerate(SALES)]

CLIENTS = [('LeBron Jameson', 'USA'), ('Stephen Curryman', 'USA'), ('Kevin Durantino', 'USA'), ('Giannis Antetokos', 'Greece'), ('Nikola Jokich', 'Serbia'),
           ('Luka Donchev', 'Slovenia'), ('Joel Embiado', 'Cameroon'), ('Jayson Tatumson', 'USA'), ('Kawhi Leonardson', 'USA'), ('Anthony Davison', 'USA'),
           ('Damian Lillardson', 'USA'), ('Jimmy Butlerson', 'USA'), ('Devin Bookerman', 'USA'), ('Ja Morantez', 'USA'), ('Zion Williamsby', 'USA'),
           ('Kyrie Irvington', 'Australia'), ('James Hardenberg', 'USA'), ('Russell Westbrookes', 'USA'), ('Chris Paulsen', 'USA'), ('Paul Georgeson', 'USA'),
           ('Trae Youngblood', 'USA'), ('Donovan Mitchum', 'USA'), ('Shai Gilgeous-Alexandrov', 'Canada'), ('Victor Wembanyard', 'France'), ('Anthony Edwardson', 'USA'),
           ('Tyrese Haliburtin', 'USA'), ('Bam Adebayer', 'USA'), ('Karl-Anthony Townsend', 'Dominican Republic'), ('Jaylen Brownell', 'USA'), ('Pascal Siakamba', 'Cameroon'),
           ('Rudy Gobertin', 'France'), ('Klay Thompsonne', 'USA'), ('Draymond Greenway', 'USA'), ('DeMar DeRozanne', 'Canada'), ('Kristaps Porzingov', 'Latvia'),
           ('Domantas Sabonov', 'Lithuania'), ('Lauri Markkanson', 'Finland'), ('Jalen Brunsworth', 'USA'), ('Paolo Banchetti', 'Italy'), ('Alperen Sengunov', 'Turkey')]
clients = []
PROFILES = ['Консервативный', 'Умеренный', 'Умеренно-агрессивный', 'Агрессивный']
for i, (n, nat) in enumerate(CLIENTS):
    bd = dt.date(int(rng.integers(1976, 2003)), int(rng.integers(1, 13)), int(rng.integers(1, 29)))
    clients.append({'id': f'C{i + 1:04d}', 'name': n, 'birthday': bd, 'nationality': nat, 'sales_id': sales[i % len(sales)]['id'],
                    'risk_profile': PROFILES[i % len(PROFILES)]})

# ---------------------------------------------------------------- accounts

FAR = dt.date(3000, 1, 1)
accounts = []
_acct = [0]


def acct_numb(src):
    if src == 'BROK':
        return ''.join(rng.choice(list('ABCDEFGHKLMNPRSTUVWXYZ'), 4)) + f'{rng.integers(10, 99)}'
    if src == 'BANK':
        return '408' + ''.join(str(x) for x in rng.integers(0, 10, 14))
    if src == 'AM':
        return f'DU.{rng.integers(10000, 99999)}'
    return f'INS-{rng.integers(100000, 999999)}'


def rand_date(a, b):
    return a + dt.timedelta(days=int(rng.integers(0, max(1, (b - a).days))))


MANAGERS = ['УК Капитал', 'УК Альфа-Инвест', 'УК Сбалансированный рост']


def new_account(client, src, typ, name, open_dt, close_dt, numb=None, manager=None, **plan):
    _acct[0] += 1
    a = {'acct_cd': f'A{_acct[0]:04d}', 'acct_numb': numb or acct_numb(src), 'acct_name': name, 'acct_type': typ, 'client_id': client['id'],
         'src_cd': src, 'open_dt': open_dt, 'close_dt': close_dt,
         'manager': manager or (str(rng.choice(MANAGERS)) if src == 'AM' else None),
         'policy_end_dt': None}  # insurance policies: the end of the policy term
    accounts.append(a)
    a['_plan'] = plan
    return a


DEPOSIT_NAMES = ['Вклад «Премиум»', 'Вклад «Доходный»', 'Вклад «Максимальный»', 'Вклад «Сберегательный»', 'Вклад «Капитал»']
DEPOSITS = {}  # acct_cd -> instrument code
for c in clients:
    wealth = float(rng.lognormal(math.log(150e6), 0.7))  # roubles
    early = lambda: rand_date(dt.date(2018, 1, 1), dt.date(2023, 12, 1))
    some_open = lambda: early() if rng.random() < 0.7 else rand_date(dt.date(2024, 1, 10), dt.date(2026, 6, 1))
    maybe_close = lambda o: rand_date(max(o, START) + dt.timedelta(days=120), END - dt.timedelta(days=10)) if rng.random() < 0.08 else FAR
    made = 0
    if rng.random() < 0.75 or True:
        o = some_open()
        new_account(c, 'BROK', 'Брокерский счёт', 'Брокерский счёт', o, maybe_close(o), size=wealth * rng.uniform(0.25, 0.5), style='brok')
        made += 1
    if rng.random() < 0.25:
        o = some_open()
        new_account(c, 'BROK', 'ИИС', 'Индивидуальный инвестиционный счёт', o, maybe_close(o), size=min(wealth * 0.03, 9e6), style='iis')
        made += 1
    for _ in range(int(rng.integers(1, 3))):
        kind = rng.choice(['deposit', 'deposit', 'savings', 'current', 'oms'])
        if kind == 'deposit':
            term = int(rng.choice([181, 367, 548, 731]))
            o = rand_date(dt.date(2023, 3, 1), dt.date(2026, 7, 15))
            mat = o + dt.timedelta(days=term)
            if mat < START + dt.timedelta(days=30):
                o, mat = START + dt.timedelta(days=int(rng.integers(5, 300))), None
                mat = o + dt.timedelta(days=term)
            rate = float(rng.choice([14.5, 15.0, 16.0, 17.5, 19.0, 21.0]))
            nm = str(rng.choice(DEPOSIT_NAMES))
            a = new_account(c, 'BANK', 'Депозит', nm, o, mat if mat <= END else FAR, size=wealth * rng.uniform(0.1, 0.3), style='deposit', rate=rate / 100, maturity=mat)
            DEPOSITS[a['acct_cd']] = add('deposit', inst('BANK', f'{nm} {rate:g}%', 'Deposit', None, nominal=None, maturity=mat, coupon=rate), 'RUB', rate=rate / 100)
        elif kind == 'savings':
            o = some_open()
            new_account(c, 'BANK', 'Накопительный счёт', 'Накопительный счёт', o, maybe_close(o), size=wealth * rng.uniform(0.03, 0.12), style='savings')
        elif kind == 'current':
            o = early()
            new_account(c, 'BANK', 'Текущий счёт', 'Текущий счёт', o, FAR, size=wealth * rng.uniform(0.005, 0.03), style='current')
        else:
            o = some_open()
            metal = str(rng.choice(['GOLD', 'GOLD', 'SILVER', 'PLATINUM', 'PALLADIUM']))
            new_account(c, 'BANK', 'ОМС', {'GOLD': 'ОМС «Золото»', 'SILVER': 'ОМС «Серебро»', 'PLATINUM': 'ОМС «Платина»', 'PALLADIUM': 'ОМС «Палладий»'}[metal],
                        o, maybe_close(o), size=wealth * rng.uniform(0.02, 0.08), style='oms', metal=metal)
        made += 1
    if rng.random() < 0.45:
        risk = str(rng.choice(['Консервативная', 'Умеренная', 'Агрессивная']))
        o = some_open()
        new_account(c, 'AM', 'Стратегия ДУ', f'{risk} стратегия', o, maybe_close(o), size=wealth * rng.uniform(0.15, 0.3), style='am', risk=risk)
        made += 1
    if rng.random() < 0.25 and made < 5:
        kind = str(rng.choice(['ИСЖ', 'НСЖ']))
        o = some_open()
        nm = 'ИСЖ «Капитал Плюс»' if kind == 'ИСЖ' else 'НСЖ «Семейный резерв»'
        a = new_account(c, 'INS', kind, nm, o, FAR, size=wealth * rng.uniform(0.05, 0.1), style='ins')
        # the term: investment life insurance for 5 years, endowment for 10 or 15
        yrs = 5 if kind == 'ИСЖ' else int(rng.choice([10, 15]))
        a['policy_end_dt'] = (o + dt.timedelta(days=1) if o.month == 2 and o.day == 29 else o).replace(year=o.year + yrs)
        a['_plan']['policy'] = add('policy', inst('INS', f'Полис {nm} №{a["acct_numb"][4:]}', 'Insurance', None), 'RUB',
                                  np.round(series(1.0, np.where(BD, rng.normal(0.00035, 0.0025, N), 0.0) + np.where(BD, 0, 0.03 / 365)), 6))

# ---------------------------------------------------------------- the test client (the UHNWI report's client)

# Кирилл Шарихин (the test client): the accounts as the report shows them, holdings sized so that on the report date
# (15.06.2026) they come close to the report's figures; from there he is simulated like every client
TEST_ID, TEST_NAME = 'C1337', 'Кирилл Шарихин'
REF = IDX[dt.date(2026, 6, 15)]
test_client = {'id': TEST_ID, 'name': TEST_NAME, 'birthday': dt.date(1984, 9, 20), 'nationality': 'Russia', 'sales_id': 'S0001',
               'risk_profile': 'Консервативный'}
clients.append(test_client)
BY_NAME = {r['instr_name']: r['instr_cd'] for r in instruments}
BY_ISSUER = {r['issuer_cd']: r['instr_cd'] for r in instruments if r['instr_type'] == 'Equity'}
TEST_BROK = [(FUNDS_RU[0], 17_556_000), (BY_ISSUER['SVCB'], 6_817_000), (BY_ISSUER['MAGN'], 3_670_000), (BY_ISSUER['AQUA'], 1_888_000),
             (BY_ISSUER['HNFG'], 1_426_000), (BY_ISSUER['AFLT'], 1_367_000), (BY_NAME['БПИФ «Капитал – Сбережения в юанях»'], 230_000),
             (BY_ISSUER['VTBR'], 159_000), (BY_NAME['ПИФ «БКС Дивидендные акции»'], 76_000), (FUNDS_RU[2], 16_000), (FUNDS_RU[3], 3_000),
             (BY_NAME['RU-20290723-6.9'], 1_500_000), (BY_NAME['SBER-20280425-13'], 1_190_000),
             (METALS['GOLD'], 9_643_000), (CASH['RUB'], 1_971_000), (CASH['CNY'], 11_000)]
TEST_DU = [(FUNDS_RU[0], 5_398_000), (FUNDS_RU[1], 1_517_000), (CASH['RUB'], 3_000)]
new_account(test_client, 'BROK', 'Брокерский счёт', 'Брокерский счёт', dt.date(2021, 3, 15), FAR, numb='400PX0F', size=0, style='brok', holdings=TEST_BROK,
            flows=[(dt.date(2026, 2, 10), 5_000_000), (dt.date(2026, 4, 14), 3_000_000), (dt.date(2026, 5, 20), -1_200_000)])
new_account(test_client, 'AM', 'Стратегия ДУ', 'Умеренно-агрессивная стратегия', dt.date(2024, 5, 3), FAR, numb='DU.SBOLPF.256730', manager='УК Капитал',
            size=0, style='am', risk='Умеренная', holdings=TEST_DU)
new_account(test_client, 'BANK', 'Накопительный счёт', 'Накопительный счёт', dt.date(2023, 2, 1), FAR, numb='40817810000023990', size=118e6, style='savings')
for numb, nm, rate, o, c, size in [('42305810000044021', 'Вклад «Доходный»', 16.0, dt.date(2025, 7, 5), dt.date(2026, 7, 5), 60e6),
                                   ('42305810000052210', 'Вклад «Премиум»', 14.5, dt.date(2025, 10, 18), dt.date(2026, 10, 18), 42e6),
                                   ('42305810000030510', 'Вклад «Сбережения-2024»', 12.5, dt.date(2024, 3, 26), dt.date(2026, 3, 26), 30e6)]:
    a = new_account(test_client, 'BANK', 'Депозит', nm, o, c, numb=numb, size=size, style='deposit', rate=rate / 100, maturity=c)
    DEPOSITS[a['acct_cd']] = add('deposit', inst('BANK', f'{nm} {rate:g}%', 'Deposit', None, maturity=c, coupon=rate), 'RUB', rate=rate / 100)
new_account(test_client, 'BANK', 'Текущий счёт', 'Платёжный счёт (руб.)', dt.date(2019, 6, 10), FAR, numb='40817810000007926', size=7_448_000, style='current')
new_account(test_client, 'BANK', 'Текущий счёт', 'Платёжный счёт (руб.)', dt.date(2020, 9, 1), FAR, numb='40817810000081638', size=6_226_000, style='current')
new_account(test_client, 'BANK', 'Накопительный счёт', 'Накопительный счёт', dt.date(2023, 4, 12), dt.date(2026, 2, 12), numb='40817810000075058', size=5e6, style='savings')
for numb, metal, nm, size in [('20309810000019623', 'GOLD', 'ОМС «Золото»', 2_893_000), ('20309810000013055', 'SILVER', 'ОМС «Серебро»', 1_788_000),
                              ('20309810000003446', 'PLATINUM', 'ОМС «Платина»', 1_555_000), ('20309810000004087', 'PALLADIUM', 'ОМС «Палладий»', 1_146_000)]:
    new_account(test_client, 'BANK', 'ОМС', nm, dt.date(2022, 11, 1) if metal in ('GOLD', 'SILVER') else dt.date(2023, 1, 20), FAR, numb=numb,
                size=size, style='oms', metal=metal, at_ref=True)
new_account(test_client, 'BANK', 'Текущий счёт', 'Премиальная карта Gold', dt.date(2023, 11, 1), FAR, numb='40817810000033540', size=0, style='current')
# a second brokerage account (an IIS) and an endowment policy, so the report's per-account pages repeat
TEST_IIS = [(BY_NAME['RU-20330406-7.65'], 1_200_000), (FUNDS_RU[0], 900_000), (BY_ISSUER['SBER'], 650_000), (CASH['RUB'], 85_000)]
new_account(test_client, 'BROK', 'ИИС', 'Индивидуальный инвестиционный счёт', dt.date(2025, 1, 20), FAR, numb='510IIS7', size=0, style='iis', holdings=TEST_IIS)
pol_a = new_account(test_client, 'INS', 'НСЖ', 'НСЖ «Семейный резерв»', dt.date(2023, 3, 15), FAR, numb='INS-777120', size=6_000_000, style='ins')
pol_a['policy_end_dt'] = dt.date(2038, 3, 15)
pol_a['_plan']['policy'] = add('policy', inst('INS', 'Полис НСЖ «Семейный резерв» №777120', 'Insurance', None), 'RUB',
                               np.round(series(1.0, np.where(BD, rng.normal(0.00035, 0.0025, N), 0.0) + np.where(BD, 0, 0.03 / 365)), 6))

# ---------------------------------------------------------------- deposits closing this year and next

# more deposits maturing in 2026 and 2027 (for maturity reminders and the cash-flow forecast); a random
# stream of their own, and made last, so every other account and figure stays as it was
xr = np.random.default_rng(20261014)
x_date = lambda a, b: a + dt.timedelta(days=int(xr.integers(0, (b - a).days + 1)))


def extra_deposit(c, nm, rate, o, mat, size, numb=None):
    numb = numb or '42305810' + ''.join(str(x) for x in xr.integers(0, 10, 9))  # its own stream, not the shared one
    a = new_account(c, 'BANK', 'Депозит', nm, o, mat if mat <= END else FAR, numb=numb, size=size, style='deposit', rate=rate / 100, maturity=mat)
    DEPOSITS[a['acct_cd']] = add('deposit', inst('BANK', f'{nm} {rate:g}%', 'Deposit', None, nominal=None, maturity=mat, coupon=rate), 'RUB', rate=rate / 100)


# the test client: one closing just after the report date, then through 2027
for numb, nm, rate, o, mat, size in [('42305810000061770', 'Вклад «Надёжный»', 15.5, dt.date(2025, 12, 26), dt.date(2026, 6, 26), 18e6),
                                     ('42305810000063115', 'Вклад «Максимальный»', 17.0, dt.date(2025, 8, 1), dt.date(2026, 8, 1), 25e6),
                                     ('42305810000064402', 'Вклад «Сберегательный»', 15.0, dt.date(2025, 12, 1), dt.date(2026, 12, 1), 12e6),
                                     ('42305810000065988', 'Вклад «Капитал»', 15.0, dt.date(2026, 1, 20), dt.date(2027, 1, 20), 20e6),
                                     ('42305810000067241', 'Вклад «Стабильный»', 14.0, dt.date(2026, 3, 10), dt.date(2027, 9, 10), 15e6)]:
    extra_deposit(test_client, nm, rate, o, mat, size, numb=numb)
# the other clients: most get one or two more, maturing between this summer and the end of next year
for c in clients:
    if c['id'] == TEST_ID or xr.random() > 0.65:
        continue
    for _ in range(int(xr.integers(1, 3))):
        mat = x_date(dt.date(2026, 6, 20), dt.date(2027, 12, 20))
        term = int(xr.choice([181, 367, 548]))
        o = max(mat - dt.timedelta(days=term), dt.date(2024, 2, 1))
        extra_deposit(c, str(xr.choice(DEPOSIT_NAMES)), float(xr.choice([14.0, 15.0, 16.0, 17.5, 19.0])), o, mat, float(xr.lognormal(math.log(25e6), 0.6)))

# ---------------------------------------------------------------- the payment calendar

# per unit of each instrument: coupons and redemptions (bonds), expected dividends (equities),
# monthly interest and repayment (deposits, savings); actual dividends are paid from it too
SCHED_END = dt.date(2027, 12, 31)
schedule = []
DIVPS = {}
for cd, m in META.items():
    if m['kind'] == 'bond':
        for d in sorted(m['coupons']):
            if START <= d <= min(m['maturity'], SCHED_END):
                schedule.append((cd, d, 'Купон', m['nominal'] * m['coupon'] / 200, m['cur']))
        if START <= m['maturity'] <= SCHED_END:
            schedule.append((cd, m['maturity'], 'Погашение', float(m['nominal']), m['cur']))
    elif m['kind'] == 'equity' and m['div'] > 0:
        for y in range(START.year, SCHED_END.year + 1):
            d = first_bday(dt.date(y, m['div_month'], 15))
            base = float(PRICE[cd][IDX[first_bday(dt.date(y, 1, 9))]]) if dt.date(y, 1, 9) <= END else float(PRICE[cd][-1]) * 1.05
            DIVPS[(cd, y)] = round(m['div'] * base, 4)
            schedule.append((cd, d, 'Дивиденд', DIVPS[(cd, y)], m['cur']))
    elif m['kind'] == 'savings':
        d = START
        while d <= SCHED_END:
            nxt = (d.replace(day=28) + dt.timedelta(days=4)).replace(day=1)
            schedule.append((cd, nxt - dt.timedelta(days=1), 'Проценты', round(m['rate'] / 12, 6), m['cur']))
            d = nxt
for acct_cd, cd in DEPOSITS.items():
    a = next(x for x in accounts if x['acct_cd'] == acct_cd)
    m, mat = META[cd], a['_plan']['maturity']
    d = max(a['open_dt'], START)
    while d < mat and d <= SCHED_END:
        nxt = (d.replace(day=28) + dt.timedelta(days=4)).replace(day=1)
        if nxt - dt.timedelta(days=1) < mat:
            schedule.append((cd, nxt - dt.timedelta(days=1), 'Проценты', round(m['rate'] / 12, 6), 'RUB'))
        d = nxt
    if mat <= SCHED_END:
        schedule.append((cd, mat, 'Погашение', 1.0, 'RUB'))

# ---------------------------------------------------------------- benchmarks and key rate

IMOEX = series(3099.0, RU_MKT)
bench = {
    'IMOEX': ('Индекс МосБиржи', 'pts', IMOEX),
    'MCFTR': ('Индекс МосБиржи полной доходности', 'pts', series(6950.0, RU_MKT + np.where(BD, 0.07 / 250, 0))),
    'RGBI': ('Индекс государственных облигаций', 'pts', series(116.5, walk(0.0015, -0.00005, factor=-RATES, beta=3.5))),
    'SPX': ('S&P 500', 'pts', series(4770.0, US_MKT)),
    'KEY_RATE': ('Ключевая ставка ЦБ', '%', interp([(DAY0, 16.0), (dt.date(2024, 7, 26), 16.0), (dt.date(2024, 7, 29), 18.0), (dt.date(2024, 9, 13), 18.0),
                                                   (dt.date(2024, 9, 16), 19.0), (dt.date(2024, 10, 25), 19.0), (dt.date(2024, 10, 28), 21.0), (dt.date(2025, 6, 6), 21.0),
                                                   (dt.date(2025, 6, 9), 20.0), (dt.date(2025, 7, 25), 20.0), (dt.date(2025, 7, 28), 18.0), (dt.date(2025, 9, 12), 18.0),
                                                   (dt.date(2025, 9, 15), 17.0), (dt.date(2025, 10, 24), 17.0), (dt.date(2025, 10, 27), 16.5), (dt.date(2025, 12, 19), 16.5),
                                                   (dt.date(2025, 12, 22), 16.0), (END, 16.0)], MKT_DAYS)),
}
cpi, v = [], 100.0
for d in MKT_DAYS:
    if d.day == 1:
        v *= 1 + rng.normal(0.0065 if d.year == 2024 else 0.005, 0.0015)
    cpi.append(v)
bench['CPI_RU'] = ('Индекс потребительских цен (2023-12 = 100)', 'index', np.array(cpi))

# ---------------------------------------------------------------- simulation

os.makedirs(TMP, exist_ok=True)
fb = open(os.path.join(TMP, 'balances.csv'), 'w', newline='')
fp = open(os.path.join(TMP, 'portf_acct.csv'), 'w', newline='')
fo = open(os.path.join(TMP, 'operations.csv'), 'w', newline='')
wb, wp, wo = csv.writer(fb), csv.writer(fp), csv.writer(fo)
_op = [0]


def price_of(cd, i):
    m = META[cd]
    if m['kind'] in ('cash', 'deposit', 'savings'):
        return 1.0
    return float(PRICE[cd][i])


def value(cd, qty, i):
    """(amt in the price currency, amt in roubles) in cents, from the stored (rounded) price."""
    m = META[cd]
    p = price_of(cd, i)
    if m['kind'] == 'bond':
        p = round(p, 6)
        amt = cents(qty * p * m['nominal'])
    elif m['kind'] in ('cash', 'deposit', 'savings'):
        amt = cents(qty)
    else:
        p = round(p, 6) if m['kind'] != 'metal' else round(p, 2)
        amt = cents(qty * p)
    fx = float(FX[m['cur']][i])
    return amt, cents(amt / 100 * fx), p, fx


def venue(cd):
    m = META.get(cd, {})
    if m.get('kind') == 'fund' and next(r for r in instruments if r['instr_cd'] == cd)['instr_name'].startswith('ПИФ'):
        return 'УК'
    return 'NYSE' if m.get('cur') == 'USD' else 'MOEX'


def op(d, a, typ, cd, qty, amount_cur, cur, amount_rub_c, fee_c=0):
    """An operation; deals (Покупка, Продажа) carry their venue and fee."""
    _op[0] += 1
    deal = typ in ('Покупка', 'Продажа')
    wo.writerow([f'O{_op[0]:07d}', d, a['client_id'], a['acct_cd'], typ, cd or '', f'{qty:.4f}' if qty is not None else '', f'{amount_cur:.2f}', cur,
                 money(amount_rub_c), venue(cd) if deal and cd else '', money(-fee_c) if deal else ''])


def bought(a, cd, q, price):
    """The position's average purchase price after buying q at price."""
    cost = a.setdefault('_cost', {})
    old_q = a.setdefault('_pos_q', {}).get(cd, 0)
    cost[cd] = (cost.get(cd, price) * old_q + price * q) / (old_q + q) if old_q + q else price


def buy_basket(a, pos, budget_rub, i, choices, weights=None):
    """Spend roubles on a basket of instruments (whole units for securities)."""
    picks = list(choices)
    w = weights or list(rng.dirichlet(np.ones(len(picks))))
    for cd, share in zip(picks, w):
        m = META[cd]
        if m['kind'] == 'bond' and (m['maturity'] - MKT_DAYS[i]).days < 120:
            continue
        unit = price_of(cd, i) * (m.get('nominal') or 1) * float(FX[m['cur']][i])
        if not unit or math.isnan(unit):
            continue
        q = math.floor(budget_rub * share / unit)
        if q > 0:
            pos[cd] = pos.get(cd, 0) + q


def initial_positions(a, i):
    p, size = a['_plan'], a['_plan']['size']
    pos = {}
    st = p['style']
    if p.get('holdings'):
        # the test client: each holding sized to its value on the report date
        for cd, target in p['holdings']:
            m = META[cd]
            unit = price_of(cd, REF) * (m.get('nominal') or 1) * float(FX[m['cur']][REF])
            pos[cd] = round(target / unit, 2) if m['kind'] == 'cash' else max(1, math.floor(target / unit))
    elif st in ('brok', 'iis'):
        pos[CASH['RUB']] = size * rng.uniform(0.03, 0.08)
        ru = list(rng.choice(EQUITY_RU, size=int(rng.integers(4, 9)), replace=False))
        bonds = list(rng.choice([b for b in BONDS_RU if META[b]['maturity'] > MKT_DAYS[i] + dt.timedelta(days=200)], size=int(rng.integers(1, 4)), replace=False))
        funds = list(rng.choice(FUNDS_RU, size=int(rng.integers(0, 3)), replace=False))
        buy_basket(a, pos, size * 0.55, i, ru)
        buy_basket(a, pos, size * 0.25, i, bonds)
        if funds:
            buy_basket(a, pos, size * 0.1, i, funds)
        if st == 'brok' and rng.random() < 0.7:
            usd = size * rng.uniform(0.08, 0.2)
            pos[CASH['USD']] = usd * 0.1 / float(FX['USD'][i])
            buy_basket(a, pos, usd * 0.6, i, list(rng.choice(EQUITY_US, size=int(rng.integers(2, 6)), replace=False)))
            buy_basket(a, pos, usd * 0.3, i, list(rng.choice(BONDS_US + FUNDS_US, size=2, replace=False)))
    elif st == 'am':
        mix = {'Консервативная': (0.15, 0.75), 'Умеренная': (0.45, 0.45), 'Агрессивная': (0.75, 0.15)}[p['risk']]
        pos[CASH['RUB']] = size * 0.03
        buy_basket(a, pos, size * mix[0], i, list(rng.choice(EQUITY_RU + FUNDS_RU[:1], size=6, replace=False)))
        buy_basket(a, pos, size * mix[1], i, list(rng.choice([b for b in BONDS_RU if META[b]['maturity'] > MKT_DAYS[i] + dt.timedelta(days=200)] + FUNDS_RU[1:3], size=4, replace=False)))
        buy_basket(a, pos, size * (1 - sum(mix) - 0.03), i, [FUNDS_RU[3], FUNDS_RU[4]], [0.5, 0.5])
    elif st == 'deposit':
        pos[DEPOSITS[a['acct_cd']]] = round(size, 2)
    elif st == 'savings':
        pos[SAVINGS] = round(size, 2)
    elif st == 'current':
        pos[CASH['RUB']] = round(size, 2)
    elif st == 'oms':
        g = math.floor(size / price_of(METALS[p['metal']], REF if p.get('at_ref') else i))
        pos[METALS[p['metal']]] = g
    elif st == 'ins':
        pos[p['policy']] = round(size / price_of(p['policy'], i), 4)
    return pos


def cash_cd(cur):
    return CASH[cur]


def pay(pos, a, d, i, rub):
    """Takes roubles from the account's RUB cash, selling rouble securities first if
    the cash isn't there (as a broker would), so cash never goes negative."""
    rc = CASH['RUB']
    short = rub - pos.get(rc, 0)
    for cd in sorted([x for x in pos if META[x]['kind'] in ('equity', 'fund', 'bond') and META[x]['cur'] == 'RUB'], key=lambda x: -value(x, pos[x], i)[1]):
        if short <= 0:
            break
        unit = price_of(cd, i) * (META[cd].get('nominal') or 1)
        q = min(pos[cd], math.ceil(short / unit))
        got = q * unit
        pos[cd] -= q
        if pos[cd] <= 0:
            del pos[cd]
        pos[rc] = pos.get(rc, 0) + got
        op(d, a, 'Продажа', cd, q, got, 'RUB', cents(got))
        short -= got
    pos[rc] = pos.get(rc, 0) - rub


def set_costs(a, pos, i, older):
    """Average prices of an account's first positions: today's price, or an older (unknown) one."""
    cost = a.setdefault('_cost', {})
    for cd in pos:
        if META[cd]['kind'] in ('equity', 'bond', 'fund', 'metal', 'policy'):
            cost[cd] = price_of(cd, i) * (float(rng.uniform(0.82, 1.12)) if older else 1.0)


big_tax_accounts = set()
for a in accounts:
    p = a['_plan']
    st = p['style']
    first = max(a['open_dt'], START)
    last = min(a['close_dt'], END) if a['close_dt'] != FAR else END
    if last < first:
        continue
    if st == 'brok' and a['client_id'] != TEST_ID and len(big_tax_accounts) < 3 and rng.random() < 0.15:
        big_tax_accounts.add(a['acct_cd'])
    opened_inside = a['open_dt'] >= START
    # positions as of the day before the first row (for accounts that already existed)
    i0 = IDX[first] - (0 if opened_inside else 1)
    pos = {} if opened_inside else initial_positions(a, i0)
    set_costs(a, pos, i0, older=True)
    bal = sum(value(cd, q, i0)[1] for cd, q in pos.items()) if pos else 0
    ltd_n = ltd_g = 1.0
    for d in [x for x in DAYS if first <= x <= last]:
        i = IDX[d]
        bal_in = bal
        flow = fees = taxes = 0
        income = 0.0
        # interest accrues every calendar day: bonds through their dirty price, deposits and savings here
        def accrual(m):  # the day's accrued coupon per unit (a coupon date resets it)
            reset = d in m['coupons'] or d == m['maturity']
            return (m['coupon'] / 200 if reset else 0.0) + m['accr'][i] - m['accr'][i - 1]
        income = sum(pos[cd] * META[cd]['nominal'] * accrual(META[cd]) * float(FX[META[cd]['cur']][i]) for cd in pos if META[cd]['kind'] == 'bond')
        # insurance policies: the guaranteed 3% a year accrues daily, like interest
        income += sum(value(cd, pos[cd], i - 1)[1] / 100 * (math.exp(0.03 / 365) - 1) for cd in pos if META[cd]['kind'] == 'policy')
        for cd in list(pos):
            if META[cd]['kind'] in ('deposit', 'savings'):
                interest = round(pos[cd] * META[cd]['rate'] / 365, 2)
                pos[cd] = round(pos[cd] + interest, 2)
                income += interest
        # coupons and redemptions are paid on their date, working day or not
        for cd in list(pos):
            m = META[cd]
            if m['kind'] == 'bond' and (d in m['coupons'] or d == m['maturity']):
                cpn = pos[cd] * m['nominal'] * m['coupon'] / 200
                cc = cash_cd(m['cur'])
                pos[cc] = pos.get(cc, 0) + cpn
                op(d, a, 'Купон', cd, pos[cd], cpn, m['cur'], cents(cpn * float(FX[m['cur']][i])))
                tax = cents(cpn * float(FX[m['cur']][i]) * 0.13)
                pay(pos, a, d, i, tax / 100)
                taxes -= tax
                op(d, a, 'Налог', cd, None, -tax / 100, 'RUB', -tax)
                if d == m['maturity']:
                    red = pos[cd] * m['nominal']
                    pos[cc] = pos.get(cc, 0) + red
                    op(d, a, 'Погашение', cd, pos[cd], red, m['cur'], cents(red * float(FX[m['cur']][i])))
                    del pos[cd]
        if d == a['open_dt']:
            # funding on the opening day
            pos = initial_positions(a, i)
            set_costs(a, pos, i, older=False)
            amt = sum(value(cd, q, i)[1] for cd, q in pos.items())
            flow += amt
            # a deposit, savings or metal account is funded into its one position
            main = next(iter(pos)) if st in ('deposit', 'savings', 'oms', 'ins') and pos else CASH['RUB']
            op(d, a, 'Пополнение', main, None, amt / 100, 'RUB', amt)
        else:
            if bday(d):
                # dividends: once a year, on the first working day from the 15th of the month
                for cd in list(pos):
                    m = META[cd]
                    if m['kind'] == 'equity' and m['div'] > 0 and d == first_bday(dt.date(d.year, m['div_month'], 15)):
                        div = pos[cd] * DIVPS[(cd, d.year)]
                        cc = cash_cd(m['cur'])
                        pos[cc] = pos.get(cc, 0) + div
                        div_rub = cents(div * float(FX[m['cur']][i]))
                        income += div_rub / 100
                        op(d, a, 'Дивиденд', cd, pos[cd], div, m['cur'], div_rub)
                        tax = cents(div_rub / 100 * 0.13)
                        pay(pos, a, d, i, tax / 100)
                        taxes -= tax
                        op(d, a, 'Налог', cd, None, -tax / 100, 'RUB', -tax)
                # trades
                fee = 0
                if st in ('brok', 'iis', 'am') and rng.random() < 1 / 18:
                    rub = pos.get(CASH['RUB'], 0)
                    secs = [cd for cd in pos if META[cd]['kind'] in ('equity', 'bond', 'fund') and META[cd]['cur'] == 'RUB']
                    if rub * 100 > bal_in * 0.06 or not secs:
                        universe = EQUITY_RU + FUNDS_RU + [b for b in BONDS_RU if META[b]['maturity'] > d + dt.timedelta(days=200)]
                        cd = str(rng.choice(universe))
                        unit = price_of(cd, i) * (META[cd].get('nominal') or 1)
                        q = math.floor(rub * rng.uniform(0.3, 0.7) / unit)
                        if q > 0:
                            cost = q * unit
                            a['_pos_q'] = pos
                            bought(a, cd, q, price_of(cd, i))
                            pos[cd] = pos.get(cd, 0) + q
                            pos[CASH['RUB']] = rub - cost
                            fee = cents(cost * 0.0005)
                            op(d, a, 'Покупка', cd, q, -cost, 'RUB', cents(-cost), fee)
                    else:
                        cd = str(rng.choice(secs))
                        unit = price_of(cd, i) * (META[cd].get('nominal') or 1)
                        q = max(1, math.floor(pos[cd] * rng.uniform(0.2, 0.5)))
                        got = q * unit
                        pos[cd] -= q
                        if pos[cd] <= 0:
                            del pos[cd]
                        pos[CASH['RUB']] = rub + got
                        fee = cents(got * 0.0005)
                        op(d, a, 'Продажа', cd, q, got, 'RUB', cents(got), fee)
                    if fee:
                        # the broker's fee: on the deal itself, not a separate operation
                        pay(pos, a, d, i, fee / 100)
                        fees -= fee
                        fee = 0
                # currency conversions (brokerage accounts)
                if st == 'brok' and rng.random() < 1 / 45:
                    cur = 'USD' if rng.random() < 0.7 else 'CNY'
                    fx = float(FX[cur][i])
                    have = pos.get(CASH[cur], 0)
                    if have * fx > bal_in / 100 * 0.02 and rng.random() < 0.5:
                        q = -round(have * rng.uniform(0.3, 0.8), 2)          # sell currency for roubles
                    else:
                        q = round(min(pos.get(CASH['RUB'], 0) * rng.uniform(0.1, 0.4), bal_in / 100 * 0.04) / fx, 2)  # buy currency
                    if abs(q) >= 1:
                        pos[CASH[cur]] = have + q
                        pos[CASH['RUB']] = pos.get(CASH['RUB'], 0) - q * fx
                        op(d, a, 'Конвертация', CASH[cur], q, q, cur, cents(q * fx))
                # management fee (asset management), first working day of the month
                if st == 'am' and d == first_bday(dt.date(d.year, d.month, 1)):
                    fee = cents(bal_in / 100 * 0.01 / 12)
                    pay(pos, a, d, i, fee / 100)
                    fees -= fee
                    op(d, a, 'Комиссия', None, None, -fee / 100, 'RUB', -fee)
                # a rare large tax (e.g. on a year's gains), so some days fall out of the usual range
                if a['acct_cd'] in big_tax_accounts and d == first_bday(dt.date(d.year, 12, 24)):
                    tax = cents(bal_in / 100 * 0.12)
                    pay(pos, a, d, i, tax / 100)
                    taxes -= tax
                    op(d, a, 'Налог', None, None, -tax / 100, 'RUB', -tax)
                # the test client's top-ups and withdrawals are the report's own
                for fd, famt in p.get('flows', []):
                    if fd == d:
                        amt = cents(famt)
                        pos[CASH['RUB']] = pos.get(CASH['RUB'], 0) + famt
                        flow += amt
                        op(d, a, 'Пополнение' if famt > 0 else 'Вывод', CASH['RUB'], None, famt, 'RUB', amt)
                # client top-ups and withdrawals
                if a['client_id'] != TEST_ID and st in ('brok', 'iis', 'am', 'savings', 'current', 'oms') and d != a['close_dt'] and rng.random() < (1 / 25 if st == 'current' else 1 / 70):
                    sign = 1 if rng.random() < 0.55 else -1
                    size = bal_in / 100 * (rng.uniform(0.02, 0.12) if sign > 0 else rng.uniform(0.01, 0.08))
                    if st == 'oms':
                        cd = METALS[p['metal']]
                        g = math.floor(size / price_of(cd, i)) * sign
                        if g and pos.get(cd, 0) + g > 0:
                            if g > 0:
                                a['_pos_q'] = pos
                                bought(a, cd, g, price_of(cd, i))
                            pos[cd] = pos.get(cd, 0) + g
                            amt = cents(g * round(price_of(cd, i), 2))
                            flow += amt
                            op(d, a, 'Пополнение' if sign > 0 else 'Вывод', cd, g, amt / 100, 'RUB', amt)
                    else:
                        cd = SAVINGS if st == 'savings' else CASH['RUB']
                        if sign < 0:
                            size = min(size, pos.get(cd, 0) * 0.9)
                        amt = cents(size) * sign
                        if amt:
                            pos[cd] = pos.get(cd, 0) + amt / 100
                            flow += amt
                            op(d, a, 'Пополнение' if sign > 0 else 'Вывод', cd, None, amt / 100, 'RUB', amt)
            # deposits pay their tax on interest at each month end
            if st in ('deposit', 'savings') and (d + dt.timedelta(days=1)).day == 1 and d != a['close_dt']:
                cd = DEPOSITS[a['acct_cd']] if st == 'deposit' else SAVINGS
                month_int = pos.get(cd, 0) * META[cd]['rate'] / 365 * d.day
                tax = cents(month_int * 0.13)
                if tax:
                    pos[cd] = round(pos[cd] - tax / 100, 2)
                    taxes -= tax
                    op(d, a, 'Налог', cd, None, -tax / 100, 'RUB', -tax)
            if st == 'ins' and d.month == a['open_dt'].month and d.day == a['open_dt'].day and d != a['open_dt']:
                pol = p['policy']
                fee = cents(value(pol, pos[pol], i)[1] / 100 * 0.015)
                pos[pol] = round(pos[pol] - fee / 100 / price_of(pol, i), 4)
                fees -= fee
                op(d, a, 'Комиссия', pol, None, -fee / 100, 'RUB', -fee)
        if d == a['close_dt']:
            # closing: everything is sold and paid out
            out = sum(value(cd, q, i)[1] for cd, q in pos.items())
            for cd, q in pos.items():
                if META[cd]['kind'] not in ('cash',):
                    op(d, a, 'Продажа' if META[cd]['kind'] in ('equity', 'bond', 'fund', 'metal') else 'Закрытие', cd, q, value(cd, q, i)[0] / 100, META[cd]['cur'], value(cd, q, i)[1])
            flow -= out
            op(d, a, 'Вывод', CASH['RUB'], None, -out / 100, 'RUB', -out)
            pos = {}
        # balances and the day's return
        bal = 0
        for cd, q in sorted(pos.items()):
            if q <= 0 and META[cd]['kind'] != 'cash':
                continue
            amt, amt_rub, pr, fx = value(cd, q, i)
            if META[cd]['kind'] == 'cash' and abs(amt) < 1:
                continue
            bal += amt_rub
            m = META[cd]
            avg = a.get('_cost', {}).get(cd, pr if m['kind'] in ('equity', 'bond', 'fund', 'metal', 'policy') else 1.0)
            # a policy's surrender value: 70% of its value at the start, rising to all of it at the end of the term
            redeem = ''
            if m['kind'] == 'policy' and a.get('policy_end_dt'):
                term = (a['policy_end_dt'] - a['open_dt']).days
                share = min(1.0, 0.7 + 0.3 * max(0, (d - a['open_dt']).days) / term)
                redeem = money(round(amt_rub * share))
            wb.writerow([d, a['client_id'], a['acct_cd'], cd, f'{q:.4f}', f'{pr:.6f}', m['cur'], money(amt), f'{fx:.4f}', money(amt_rub), f'{avg:.6f}', redeem])
        if flow >= 0:
            denom = bal_in + flow
            net = bal / denom if denom else 1.0
            gross = (bal - fees - taxes) / denom if denom else 1.0
        else:
            net = (bal - flow) / bal_in if bal_in else 1.0
            gross = (bal - flow - fees - taxes) / bal_in if bal_in else 1.0
        ltd_n *= net
        ltd_g *= gross
        wp.writerow([d, a['client_id'], a['acct_cd'], 'RUB', money(bal_in), money(flow), money(bal), repr(net), f'{income:.2f}', money(fees), money(taxes), repr(gross), repr(ltd_n), repr(ltd_g)])

fb.close(); fp.close(); fo.close()

# ---------------------------------------------------------------- write the database

if os.path.exists(OUT):
    os.remove(OUT)
con = duckdb.connect(OUT)
con.execute("""
CREATE TABLE sales (id TEXT PRIMARY KEY, name TEXT, sales_phone TEXT, sales_email TEXT);
CREATE TABLE clients (id TEXT PRIMARY KEY, name TEXT, birthday DATE, nationality TEXT, sales_id TEXT REFERENCES sales(id), risk_profile TEXT);
CREATE TABLE accounts (acct_cd TEXT PRIMARY KEY, acct_numb TEXT, acct_name TEXT, acct_type TEXT, client_id TEXT REFERENCES clients(id),
                       src_cd TEXT, open_dt DATE, close_dt DATE, manager TEXT, policy_end_dt DATE);
CREATE TABLE instruments (instr_cd TEXT PRIMARY KEY, isin TEXT, issuer_cd TEXT, instr_name TEXT, instr_type TEXT, instr_type_ru TEXT,
                          instr_nominal INTEGER, maturity_dt DATE, coupon DECIMAL(8,3), instr_cur TEXT);
CREATE TABLE balances (report_dt DATE, client_id TEXT, acct_cd TEXT, instr_cd TEXT, qty DECIMAL(20,4), price DECIMAL(18,6), price_cur TEXT,
                       amt DECIMAL(20,2), fx_rate DECIMAL(12,4), amt_rub DECIMAL(20,2), avg_price DECIMAL(18,6),
                       redeem_amt DECIMAL(20,2));
CREATE TABLE portf_acct (report_dt DATE, client_id TEXT, acct_cd TEXT, cur TEXT, bal_in DECIMAL(20,2), net_flow DECIMAL(20,2), bal_out DECIMAL(20,2),
                         net_twr DOUBLE, income DECIMAL(20,2), fees DECIMAL(20,2), taxes DECIMAL(20,2), gross_twr DOUBLE, net_twr_ltd DOUBLE, gross_twr_ltd DOUBLE);
CREATE TABLE operations (op_id TEXT PRIMARY KEY, op_dt DATE, client_id TEXT, acct_cd TEXT, op_type TEXT, instr_cd TEXT, qty DECIMAL(20,4),
                         amount DECIMAL(20,2), cur TEXT, amount_rub DECIMAL(20,2), venue TEXT, fee DECIMAL(20,2));
CREATE TABLE cashflow_schedule (instr_cd TEXT, pay_dt DATE, cf_type TEXT, amount_per_unit DECIMAL(18,6), cur TEXT);
CREATE TABLE fx_rates (report_dt DATE, cur TEXT, rate DECIMAL(12,4));
CREATE TABLE prices (report_dt DATE, instr_cd TEXT, price DECIMAL(18,6), price_cur TEXT);
CREATE TABLE benchmarks (report_dt DATE, bench_cd TEXT, bench_name TEXT, unit TEXT, value DOUBLE);
CREATE TABLE products (src_cd TEXT PRIMARY KEY, name TEXT, description TEXT, name_en TEXT, description_en TEXT, sort_order INTEGER);
""")
# the bank's products (source systems): what accounts.src_cd means, in words
PRODUCTS = [
    ('BANK', 'Банк', 'Текущие счета, вклады и накопительные счета', 'Bank', 'Current accounts, deposits and savings accounts', 1),
    ('BROK', 'Брокерское обслуживание', 'Брокерские счета, включая ИИС', 'Brokerage', 'Brokerage accounts, incl. IIS', 2),
    ('AM', 'Доверительное управление', 'Стратегии доверительного управления (ДУ)', 'Asset management', 'Discretionary (DU) mandates', 3),
    ('INS', 'Страхование', 'Инвестиционное и накопительное страхование жизни', 'Insurance', 'Investment and savings policies', 4),
]
con.executemany('INSERT INTO products VALUES (?, ?, ?, ?, ?, ?)', PRODUCTS)
con.executemany('INSERT INTO sales VALUES (?, ?, ?, ?)', [tuple(s.values()) for s in sales])
con.executemany('INSERT INTO clients VALUES (?, ?, ?, ?, ?, ?)', [tuple(c.values()) for c in clients])
con.executemany('INSERT INTO accounts VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)', [tuple(v for k, v in a.items() if not k.startswith('_')) for a in accounts])
con.executemany('INSERT INTO cashflow_schedule VALUES (?, ?, ?, ?, ?)', schedule)
con.executemany('INSERT INTO instruments VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)',
                [(r['instr_cd'], r['isin'], r['issuer_cd'], r['instr_name'], r['instr_type'], r['instr_type_ru'], r['instr_nominal'], r['maturity_dt'], r['coupon'], r['instr_cur']) for r in instruments])
with open(os.path.join(TMP, 'fx.csv'), 'w', newline='') as f:
    w = csv.writer(f)
    for c, path in FX.items():
        for d, v in zip(MKT_DAYS[1:], path[1:]):
            w.writerow([d, c, f'{v:.4f}'])
with open(os.path.join(TMP, 'prices.csv'), 'w', newline='') as f:
    w = csv.writer(f)
    for cd, path in PRICE.items():
        m = META[cd]
        if m['kind'] in ('policy',):
            continue
        for d, v in zip(MKT_DAYS[1:], path[1:]):
            if not math.isnan(v):
                w.writerow([d, cd, f'{v:.6f}', m['cur']])
with open(os.path.join(TMP, 'bench.csv'), 'w', newline='') as f:
    w = csv.writer(f)
    for code, (name, unit, path) in bench.items():
        for d, v in zip(MKT_DAYS[1:], path[1:]):
            w.writerow([d, code, name, unit, f'{v:.4f}'])
for table, file in [('balances', 'balances.csv'), ('portf_acct', 'portf_acct.csv'), ('operations', 'operations.csv'), ('fx_rates', 'fx.csv'),
                    ('prices', 'prices.csv'), ('benchmarks', 'bench.csv')]:
    con.execute(f"COPY {table} FROM '{os.path.join(TMP, file)}' (HEADER false, NULLSTR '')")
shutil.rmtree(TMP)

# ---------------------------------------------------------------- self-checks

CHECKS = [
    ('bal_out = sum of amt_rub (per account and day)', """
        SELECT count(*) FROM portf_acct p LEFT JOIN (SELECT report_dt, acct_cd, sum(amt_rub) s FROM balances GROUP BY ALL) b USING (report_dt, acct_cd)
        WHERE p.bal_out <> coalesce(b.s, 0)"""),
    ('bal_in = previous day bal_out', """
        SELECT count(*) FROM (SELECT bal_in, lag(bal_out) OVER (PARTITION BY acct_cd ORDER BY report_dt) prev FROM portf_acct) WHERE prev IS NOT NULL AND bal_in <> prev"""),
    ('every calendar day for each open account', """
        SELECT count(*) FROM (SELECT acct_cd, count(*) n, min(report_dt) a, max(report_dt) b FROM portf_acct GROUP BY 1) WHERE n <> date_diff('day', a, b) + 1"""),
    ('net_twr follows the formula', """
        SELECT count(*) FROM portf_acct WHERE bal_in + net_flow <> 0 AND abs(net_twr - CASE WHEN net_flow >= 0 THEN bal_out / (bal_in + net_flow)
                                                                                         ELSE (bal_out - net_flow) / bal_in END) > 1e-9"""),
    ('gross_twr follows the formula', """
        SELECT count(*) FROM portf_acct WHERE bal_in + net_flow <> 0 AND abs(gross_twr - CASE WHEN net_flow >= 0 THEN (bal_out - fees - taxes) / (bal_in + net_flow)
                                                                                           ELSE (bal_out - net_flow - fees - taxes) / bal_in END) > 1e-9"""),
    ('net_twr_ltd = running product of net_twr', """
        SELECT count(*) FROM (SELECT net_twr_ltd, exp(sum(ln(net_twr)) OVER (PARTITION BY acct_cd ORDER BY report_dt)) p FROM portf_acct WHERE net_twr > 0)
        WHERE abs(net_twr_ltd / p - 1) > 1e-9"""),
    ('net_flow = top-ups and withdrawals in operations', """
        SELECT count(*) FROM portf_acct p LEFT JOIN (SELECT op_dt report_dt, acct_cd, sum(amount_rub) s FROM operations WHERE op_type IN ('Пополнение', 'Вывод') GROUP BY ALL) o
        USING (report_dt, acct_cd) WHERE p.net_flow <> coalesce(o.s, 0)"""),
    ('fees and taxes = operations', """
        SELECT count(*) FROM portf_acct p LEFT JOIN (SELECT op_dt report_dt, acct_cd, coalesce(sum(amount_rub) FILTER (op_type = 'Комиссия'), 0) + coalesce(sum(fee), 0) f,
                                                     sum(amount_rub) FILTER (op_type = 'Налог') t FROM operations GROUP BY ALL) o USING (report_dt, acct_cd)
        WHERE p.fees <> coalesce(o.f, 0) OR p.taxes <> coalesce(o.t, 0)"""),
    ('amt = qty × price (× nominal for bonds), amt_rub = amt × fx', """
        SELECT count(*) FROM balances b JOIN instruments i USING (instr_cd)
        WHERE abs(b.amt - round(b.qty * b.price * coalesce(CASE WHEN i.instr_type = 'Bonds' THEN i.instr_nominal END, 1), 2)) > 0.011
           OR abs(b.amt_rub - round(b.amt * b.fx_rate, 2)) > 0.011"""),
    ('no market moves on non-working days (only bonds accrue)', """
        SELECT count(*) FROM prices p0 JOIN prices p1 ON p1.instr_cd = p0.instr_cd AND p1.report_dt = p0.report_dt + 1
        JOIN instruments i ON i.instr_cd = p0.instr_cd
        WHERE i.instr_type <> 'Bonds' AND p1.price <> p0.price AND (dayofweek(p1.report_dt) IN (0, 6) OR strftime(p1.report_dt, '%m-%d') IN ('01-01', '01-02', '01-03', '01-04', '01-05', '01-06', '01-07', '01-08', '02-23', '03-08', '05-01', '05-09', '06-12', '11-04', '12-31'))"""),
    ('coupons and dividends paid = schedule × quantity', """
        SELECT count(*) FROM operations o JOIN cashflow_schedule s ON s.instr_cd = o.instr_cd AND s.pay_dt = o.op_dt
          AND s.cf_type = o.op_type WHERE o.op_type IN ('Купон', 'Дивиденд') AND abs(o.amount - o.qty * s.amount_per_unit) > 0.02"""),
    ('insurance policies have an end date and a surrender value within their value', """
        SELECT (SELECT count(*) FROM accounts WHERE src_cd = 'INS' AND policy_end_dt IS NULL)
             + (SELECT count(*) FROM balances b JOIN accounts a USING (acct_cd) WHERE a.src_cd = 'INS' AND (b.redeem_amt IS NULL OR b.redeem_amt > b.amt_rub + 0.01))"""),
    ('the test client has accounts, balances and returns', """
        SELECT count(*) FROM (SELECT 1) WHERE NOT EXISTS (SELECT 1 FROM portf_acct WHERE client_id = 'C1337')
           OR NOT EXISTS (SELECT 1 FROM balances WHERE client_id = 'C1337')"""),
    ('closed accounts end at zero on close_dt', """
        SELECT count(*) FROM accounts a JOIN portf_acct p ON p.acct_cd = a.acct_cd AND p.report_dt = a.close_dt WHERE p.bal_out <> 0"""),
]
print(f'{OUT}')
bad = 0
for name, sql in CHECKS:
    n = con.execute(sql).fetchone()[0]
    bad += n
    print(f'  {"ok " if n == 0 else "FAIL"} {name}' + (f'  ({n} rows)' if n else ''))
out = con.execute("SELECT count(*) FILTER (net_twr < 0.9 OR net_twr > 1.1), count(*) FILTER (gross_twr < 0.9 OR gross_twr > 1.1) FROM portf_acct").fetchone()
print(f'  info: days outside 0.9–1.1 — net {out[0]}, gross {out[1]} (large taxes and market shocks)')
for t in ['sales', 'clients', 'accounts', 'instruments', 'balances', 'portf_acct', 'operations', 'cashflow_schedule', 'fx_rates', 'prices', 'benchmarks']:
    print(f'  {t:12s} {con.execute(f"SELECT count(*) FROM {t}").fetchone()[0]:>9,} rows')
con.close()
sys.exit(1 if bad else 0)
