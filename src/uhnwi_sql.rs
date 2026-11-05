//! The UHNWI report's data sources as SQL on the demo bank database's own
//! tables (clients, accounts, instruments, balances, portf_acct, operations,
//! cashflow_schedule, prices, fx_rates, benchmarks), for the client chosen in
//! the toolbar (`:client`) and the report period (`:period_start`,
//! `:period_end`).
//!
//! Account groups: every brokerage (IIS too), asset-management and insurance
//! account on its own (by its acct_cd: the report repeats their pages per
//! account), bank accounts together as BANK (a portfolio), and ALL. A period's return is
//! net_twr_ltd at the period's last day / net_twr_ltd the day before it
//! starts − 1; a group's daily series is built from its accounts' summed
//! balances and flows with the same formula as portf_acct.

/// The CTEs every query starts from (DuckDB inlines only those it uses).
const BASE: &str = r#"
with per as (select cast(:period_start as date) as s, cast(:period_end as date) as e),
cl as (select id, risk_profile from clients where name = :client),
acc as (
  select a.*,
         case when a.src_cd = 'BANK' then 'BANK' else a.acct_cd end as grp,
         '***' || right(a.acct_numb, 5) as short_no,
         -- how an account is named in lists: its type, then its number
         case a.src_cd when 'AM' then 'ДУ ' || right(a.acct_numb, 5) when 'BROK' then a.acct_type || ' ' || a.acct_numb
                       else a.acct_name || ' ***' || right(a.acct_numb, 5) end as acct_label
  from accounts a where a.client_id = (select id from cl)
    -- only the products (source systems) the signed-in person may see
    and (:access_src = '' or list_contains(string_split(:access_src, ','), a.src_cd))
    -- and those picked in the "Продукты" filter of the report (All, or one or several)
    and ('All' in (:product) or a.src_cd in (:product))),
pa as (select p.*, a.grp from portf_acct p join acc a using (acct_cd)),
gday as (
  select grp, report_dt, sum(bal_in) as bi, sum(net_flow) as nf, sum(bal_out) as bo, sum(income) as inc, sum(fees) + sum(taxes) as charges
  from (select grp, report_dt, bal_in, net_flow, bal_out, income, fees, taxes from pa
        union all select 'ALL', report_dt, bal_in, net_flow, bal_out, income, fees, taxes from pa)
  group by 1, 2),
gltd as (
  select *, exp(sum(ln(case when tw > 0 then tw else 1 end)) over (partition by grp order by report_dt)) as ltd
  from (select *, case when nf >= 0 then bo / nullif(bi + nf, 0) else (bo - nf) / nullif(bi, 0) end as tw from gday)),
gret as (
  select grp,
         max(ltd) filter (report_dt = (select e from per)) / coalesce(max(ltd) filter (report_dt = (select s from per) - 1), 1) - 1 as ret,
         sum(bo - bi - nf) filter (report_dt between (select s from per) and (select e from per)) as result,
         sum(inc) filter (report_dt between (select s from per) and (select e from per)) as inc,
         sum(charges) filter (report_dt between (select s from per) and (select e from per)) as charges,
         sum(greatest(nf, 0)) filter (report_dt between (select s from per) and (select e from per)) as deposits,
         sum(least(nf, 0)) filter (report_dt between (select s from per) and (select e from per)) as withdrawals,
         max(bo) filter (report_dt = (select e from per)) as value,
         coalesce(max(bo) filter (report_dt = (select s from per) - 1), 0) as start
  from gltd group by grp),
bret as (
  select bench_cd, max(value) filter (report_dt = (select e from per)) / max(value) filter (report_dt = (select s from per) - 1) - 1 as ret
  from benchmarks group by 1),
usd as (
  select max(rate) filter (report_dt = (select e from per)) / max(rate) filter (report_dt = (select s from per) - 1) - 1 as ret
  from fx_rates where cur = 'USD'),
dep_mkt as (  -- what a deposit at the key rate would have earned over the period
  select avg(value) / 100 * ((select e from per) - (select s from per) + 1) / 365 as ret
  from benchmarks where bench_cd = 'KEY_RATE' and report_dt between (select s from per) and (select e from per)),
peer_day as (  -- clients with the same risk profile, the client included
  select p.client_id, p.report_dt, sum(p.bal_in) as bi, sum(p.net_flow) as nf, sum(p.bal_out) as bo
  from portf_acct p join clients c on c.id = p.client_id
  where c.risk_profile = (select risk_profile from cl) and p.report_dt between (select s from per) and (select e from per)
  group by 1, 2),
peer_tw as (select *, case when nf >= 0 then bo / nullif(bi + nf, 0) else (bo - nf) / nullif(bi, 0) end as tw from peer_day),
peer as (select client_id, exp(sum(ln(case when tw > 0 then tw else 1 end))) - 1 as ret from peer_tw group by 1),
others as (select * from peer where client_id <> (select id from cl)),
me as (select (select ret from gret where grp = 'ALL') as ret),
bal_s as (select acct_cd, instr_cd, amt_rub as v0 from balances
          where report_dt = (select s from per) - 1 and acct_cd in (select acct_cd from acc)),
bal_e as (select acct_cd, instr_cd, amt_rub as v1, qty, price, avg_price from balances
          where report_dt = (select e from per) and acct_cd in (select acct_cd from acc)),
opp as (  -- money that went into (+) or came out of (−) each position during the period
  select acct_cd, instr_cd,
         sum(case when op_type in ('Пополнение', 'Вывод') then amount_rub else -amount_rub end) as fin,
         coalesce(sum(amount_rub) filter (op_type in ('Купон', 'Дивиденд')), 0) as paid,
         -- what the position paid out: dividends, coupons and maturities (part of its result)
         coalesce(sum(amount_rub) filter (op_type in ('Купон', 'Дивиденд', 'Погашение')), 0) as received
  from operations
  where op_dt between (select s from per) and (select e from per) and acct_cd in (select acct_cd from acc) and instr_cd is not null
    and op_type in ('Пополнение', 'Вывод', 'Покупка', 'Продажа', 'Купон', 'Дивиденд', 'Погашение', 'Закрытие')
  group by 1, 2),
pos as (  -- every position: value at the start and end of the period, and its result in between
  select x.acct_cd, x.instr_cd, a.grp, a.src_cd, a.acct_type, a.acct_name, a.acct_numb, a.short_no, a.manager,
         i.instr_name, i.instr_type, i.isin, i.instr_cur, i.issuer_cd,
         coalesce(x.v0, 0) as v0, coalesce(x.v1, 0) as v1, coalesce(x.fin, 0) as fin, coalesce(x.paid, 0) as paid, coalesce(x.received, 0) as received,
         x.qty, x.price, x.avg_price,
         case when i.instr_type = 'Currency' then 0 else coalesce(x.v1, 0) - coalesce(x.v0, 0) - coalesce(x.fin, 0) end as result
  from (select * from bal_e full join bal_s using (acct_cd, instr_cd) full join opp using (acct_cd, instr_cd)) as x
  join acc a using (acct_cd) join instruments i using (instr_cd)),
acct_adj as (  -- what an account earned beyond its positions: taxes, fees, the revaluation of its cash
  select r.grp, r.res - coalesce((select sum(result) from pos p where p.grp = r.grp), 0) as d,
         (select max(src_cd) from acc a where a.grp = r.grp) as src_cd
  from (select grp, sum(bo - bi - nf) as res from gday where grp <> 'ALL' and report_dt between (select s from per) and (select e from per) group by 1) r),
depinc as (  -- income from deposits: what the product "Депозиты и вклады" earned (bank interest and its account's charges);
               -- metals (ОМС) and funds bought at the bank count as investments, on every page
  select coalesce((select sum(result) from pos where src_cd = 'BANK' and instr_type not in ('Funds', 'Metals')), 0)
       + coalesce((select sum(d) from acct_adj where src_cd = 'BANK'), 0) as v),
fc as (  -- the next 12 months' payments: the calendar × what is held at the period end, in roubles
  select s.instr_cd, s.pay_dt, s.cf_type, b.acct_cd, b.qty * s.amount_per_unit * f.rate as rub,
         -- month 1 is the month after the period's (the rest of that month goes with it)
         greatest(1, date_diff('month', date_trunc('month', (select e from per)), date_trunc('month', s.pay_dt))) as m
  from bal_e b join cashflow_schedule s using (instr_cd)
  join fx_rates f on f.cur = s.cur and f.report_dt = (select e from per)
  where s.pay_dt > (select e from per) and s.pay_dt <= last_day(cast((select e from per) + interval 12 month as date))),
fcx as (select fc.*, a.short_no, a.acct_name, a.acct_type, i.instr_name, i.coupon, i.instr_type
        from fc join acc a using (acct_cd) join instruments i using (instr_cd)),
ops as (select o.*, a.grp, a.acct_numb, a.short_no, a.acct_name, a.acct_type, a.acct_label, a.src_cd from operations o join acc a using (acct_cd)
        where o.op_dt between (select s from per) and (select e from per)),
mret as (  -- each month of the period: its return (ltd at its last day / ltd the day before it starts − 1) and result
  select date_trunc('month', report_dt) as mon, arg_max(ltd, report_dt) / (arg_min(ltd, report_dt) / arg_min(tw, report_dt)) - 1 as ret,
         sum(bo - bi - nf) as res, arg_min(bi, report_dt) as cap
  from gltd where grp = 'ALL' and report_dt between (select s from per) and (select e from per) group by 1),
mnames as (select ['Январь', 'Февраль', 'Март', 'Апрель', 'Май', 'Июнь', 'Июль', 'Август', 'Сентябрь', 'Октябрь', 'Ноябрь', 'Декабрь'] as mfull,
                  ['Янв', 'Фев', 'Мар', 'Апр', 'Май', 'Июн', 'Июл', 'Авг', 'Сен', 'Окт', 'Ноя', 'Дек'] as mshort)
"#;

/// The client's accounts that get pages of their own — brokerage (IIS
/// too), asset management, insurance — open at some time in the period, in
/// the order the report shows them; with every text their pages need.
pub const ACCOUNTS: &str = r#"
, k as (
  select a.*, g.result, g.value, g.ret, g.start,
         case a.src_cd when 'AM' then 'DU' else a.src_cd end as kind,
         case a.src_cd when 'BROK' then case a.acct_type when 'ИИС' then 2 else 1 end when 'AM' then 3 else 4 end as ord
  from acc a left join gret g on g.grp = a.acct_cd
  where a.src_cd in ('BROK', 'AM', 'INS') and a.open_dt <= (select e from per) and a.close_dt >= (select s from per)),
notes as (  -- a brokerage account's cash and bonds: value and share of the account
  select grp, max(n) filter (class = 'Денежные средства') as cash_note, max(n) filter (class = 'Облигации') as bond_note from (
    select grp, case when instr_type in ('Equity', 'Funds') then 'Акции и фонды' when instr_type = 'Bonds' then 'Облигации' else 'Денежные средства' end as class,
           replace(format('{:,}', cast(round(sum(v1)) as bigint)), ',', ' ') || ' ₽ · '
             || cast(round(100 * sum(v1) / nullif(sum(sum(v1)) over (partition by grp), 0)) as integer) || '%' as n
    from pos where src_cd = 'BROK' group by grp, 2) group by grp)
select acct_cd, acct_numb, kind,
       case kind when 'DU' then 'Доверительное управление' else acct_type end as type_name,
       case kind when 'BROK' then acct_type || ' ' || acct_numb when 'DU' then 'Доверительное управление · ' || acct_numb
                 else acct_name end as title,
       case kind when 'DU' then acct_numb else '№ ' || acct_numb end as badge,
       acct_name, coalesce(manager, '') as manager, replace(acct_name, ' стратегия', '') as strategy,
       case when close_dt <= (select e from per) then ' · закрыт ' || strftime(close_dt, '%d.%m.%Y') else '' end as closed_note,
       case when ret >= (select ret from bret where bench_cd = 'IMOEX') then 'опережает рынок акций' else 'отстаёт от рынка акций' end as vs_market,
       case when result < 0 then '−' else '+' end || replace(format('{:,}', cast(round(abs(coalesce(result, 0))) as bigint)), ',', ' ') || ' ₽' as result_txt,
       case when coalesce(result, 0) < 0 then 'bad' else 'good' end as result_tone,
       replace(format('{:,}', cast(round(coalesce(value, 0)) as bigint)), ',', ' ') || ' ₽' as value_txt,
       coalesce(n.cash_note, '—') as cash_note, coalesce(n.bond_note, '—') as bond_note,
       coalesce(strftime(policy_end_dt, '%d.%m.%Y'), '') as policy_end
from k left join notes n on n.grp = k.acct_cd
order by ord, value desc nulls last"#;

/// The client and their sales person (the "banker" on the cover and in the footer).
pub const CLIENTS: &str = "select c.name as client, s.name as banker, s.sales_email as email, s.sales_phone as phone, c.risk_profile as profile\n\
                           from clients c join sales s on s.id = c.sales_id\nwhere c.name = :client";

/// Every client, for the toolbar's picker.
pub const CLIENT_LIST: &str = "select name as client, id as client_id from clients order by name";

/// The query of one of the report's sources (by dataset id).
pub fn query(id: &str) -> Option<String> {
    let body = match id {
        "port" => r#"
select g.value, g.result as income, g.ret,
       (select v from depinc) as dep_income,
       g.result - (select v from depinc) as inv_income,
       round((g.ret - (select median(ret) from others)) * 100, 1) as vs_peers,
       round((g.ret - (select ret from bret where bench_cd = 'IMOEX')) * 100, 1) as vs_index,
       (select sum(rub) from fc where cf_type <> 'Погашение') as fc,
       (select sum(rub) from fc where cf_type <> 'Погашение') / 12 as fc_month,
       12 as horizon,
       (select sum(abs(amount_rub)) from ops where op_type in ('Покупка', 'Продажа')) as turnover,
       (select count(*) from ops where op_type in ('Покупка', 'Продажа')) as trades,
       (select sum(amount_rub) from ops where op_type in ('Пополнение', 'Вывод')) as net_in,
       -- the period's activity in parts (the same as "flows", in roubles)
       coalesce((select sum(abs(amount_rub)) from ops where op_type = 'Покупка'), 0) as buys,
       coalesce((select sum(abs(amount_rub)) from ops where op_type = 'Продажа'), 0) as sells,
       coalesce((select sum(abs(amount_rub)) from ops where op_type = 'Конвертация'), 0) as fx_total,
       coalesce((select sum(abs(amount_rub)) from ops where op_type = 'Конвертация' and qty > 0), 0) as fx_buy,
       coalesce((select sum(abs(amount_rub)) from ops where op_type = 'Конвертация' and qty < 0), 0) as fx_sell,
       (select count(*) from ops where op_type = 'Конвертация') as fx_n,
       coalesce((select sum(amount_rub) from ops where op_type = 'Пополнение'), 0) as cash_in,
       coalesce((select -sum(amount_rub) from ops where op_type = 'Вывод'), 0) as cash_out,
       coalesce((select sum(amount_rub) from ops where op_type = 'Пополнение' and acct_cd in (select acct_cd from acc where acct_type = 'Депозит' and open_dt = ops.op_dt)), 0) as dep_open,
       coalesce((select sum(amount_rub) from ops where op_type = 'Закрытие'), 0) as dep_close,
       coalesce((select -sum(amount_rub) from ops where op_type = 'Налог'), 0) as tax_paid,
       coalesce((select -sum(amount_rub) filter (op_type = 'Комиссия') from ops), 0) - coalesce((select sum(fee) from ops), 0) as fees_paid,
       coalesce((select -sum(amount_rub) from ops where op_type = 'Налог'), 0)
         + coalesce((select -sum(amount_rub) filter (op_type = 'Комиссия') from ops), 0) - coalesce((select sum(fee) from ops), 0) as charges,
       -- where the client stands among similar clients, in words
       case when (select count(*) from others) = 0 then 'нет похожих клиентов для сравнения'
            when (select count(*) from others where ret < g.ret) = 0 then 'ниже всех похожих клиентов'
            when (select count(*) from others where ret < g.ret) = (select count(*) from others) then 'впереди всех похожих клиентов'
            else 'впереди ' || cast(cast(round(100.0 * (select count(*) from others where ret < g.ret) / nullif((select count(*) from others), 0)) as integer) as varchar) || '% клиентов' end as ahead_txt,
       -- the client among similar clients: percentile and quartile, and how many there are
       cast(cast(round(100.0 * (select count(*) from others where ret < g.ret) / nullif((select count(*) from others), 0)) as integer) as varchar) as pctile,
       case when (select count(*) from others where ret < g.ret) >= 0.75 * (select count(*) from others) then 'Топ-квартиль'
            when (select count(*) from others where ret < g.ret) >= 0.5 * (select count(*) from others) then 'Второй квартиль'
            when (select count(*) from others where ret < g.ret) >= 0.25 * (select count(*) from others) then 'Третий квартиль'
            else 'Нижний квартиль' end as quartile,
       (select cast(n as varchar) || case when n % 10 = 1 and n % 100 <> 11 then ' портфель'
                                          when n % 10 between 2 and 4 and n % 100 not between 12 and 14 then ' портфеля' else ' портфелей' end
        from (select count(*) as n from others)) as peers,
       -- counts in words for the activity pages
       (select n || ' ' || case when n % 10 = 1 and n % 100 <> 11 then 'сделка' when n % 10 between 2 and 4 and n % 100 not between 12 and 14 then 'сделки' else 'сделок' end from (select count(*) as n from ops where op_type in ('Покупка', 'Продажа', 'Конвертация'))) as trades_txt,
       (select n || ' ' || case when n % 10 = 1 and n % 100 <> 11 then 'операция' when n % 10 between 2 and 4 and n % 100 not between 12 and 14 then 'операции' else 'операций' end
        from (select count(*) as n from ops where op_type in ('Пополнение', 'Вывод', 'Закрытие', 'Налог', 'Комиссия'))) as ops_txt,
       -- the counts under the activity cards: each card's own operations
       (select n || ' ' || case when n % 10 = 1 and n % 100 <> 11 then 'сделка' when n % 10 between 2 and 4 and n % 100 not between 12 and 14 then 'сделки' else 'сделок' end
        from (select count(*) as n from ops where op_type in ('Покупка', 'Продажа'))) as deals_txt,
       (select n || ' ' || case when n % 10 = 1 and n % 100 <> 11 then 'конверсия' when n % 10 between 2 and 4 and n % 100 not between 12 and 14 then 'конверсии' else 'конверсий' end
        from (select count(*) as n from ops where op_type in ('Конвертация'))) as fx_txt,
       (select n || ' ' || case when n % 10 = 1 and n % 100 <> 11 then 'операция' when n % 10 between 2 and 4 and n % 100 not between 12 and 14 then 'операции' else 'операций' end
        from (select count(*) as n from ops where op_type in ('Пополнение', 'Вывод'))) as cash_txt,
       (select n || ' ' || case when n % 10 = 1 and n % 100 <> 11 then 'начисление' when n % 10 between 2 and 4 and n % 100 not between 12 and 14 then 'начисления' else 'начислений' end
        from (select count(*) as n from ops where op_type in ('Налог', 'Комиссия'))) as charges_txt,
       (select 'Показаны ' || least(k, 11) || ' ' || case when least(k, 11) % 10 = 1 and least(k, 11) % 100 <> 11 then 'крупнейшая сделка' when least(k, 11) % 10 between 2 and 4 and least(k, 11) % 100 not between 12 and 14 then 'крупнейшие сделки' else 'крупнейших сделок' end || ' по обороту из ' || n || ' сделок и конверсий за период'
        from (select count(*) as n, count(distinct coalesce(instr_cd, cur)) as k
              -- as the table counts: the largest deal of each instrument or currency pair
              from ops where op_type in ('Покупка', 'Продажа', 'Конвертация'))) as deals_note,
       -- the period in two sentences: how the capital changed, and why
       (select 'Капитал ' || case when x.value >= x.start then 'вырос' else 'снизился' end
               || ' с ' || replace(printf('%.1f', x.start / 1e6), '.', ',') || ' до ' || replace(printf('%.1f', x.value / 1e6), '.', ',') || ' млн ₽. '
               || 'Доход по портфелю — ' || replace(printf('%.1f', x.result / 1e6), '.', ',') || ' млн ₽, '
               || case when x.deposits + x.withdrawals >= 0 then 'чистое пополнение — ' else 'чистый вывод — ' end
               || replace(printf('%.1f', abs(x.deposits + x.withdrawals) / 1e6), '.', ',') || ' млн ₽.'
        from gret x where x.grp = 'ALL') as story,
       -- the return in plain answers: after inflation, per year, the best and worst month, how often it gained, the deepest dip
       (1 + g.ret) / (1 + (select ret from bret where bench_cd = 'CPI_RU')) - 1 as real_ret,
       power(1 + g.ret, 365.0 / greatest(1, (select e from per) - (select s from per) + 1)) - 1 as ann_ret,
       (select max(ret) from mret) as best_m_ret,
       (select lower((select mfull from mnames)[month(arg_max(mon, ret))]) from mret) as best_m,
       (select min(ret) from mret) as worst_m_ret,
       (select lower((select mfull from mnames)[month(arg_min(mon, ret))]) from mret) as worst_m,
       (select count(*) filter (ret > 0) || ' из ' || count(*) from mret) as pos_months,
       (select min(ltd / m - 1) from (select ltd, max(ltd) over (order by report_dt) as m from gltd
          where grp = 'ALL' and report_dt between (select s from per) - 1 and (select e from per))) as max_dd,
       -- the months with the largest and smallest result in money (lowercase, for card titles)
       (select lower((select mfull from mnames)[month(arg_max(mon, res))]) from mret) as best_inc_month,
       (select lower((select mfull from mnames)[month(arg_min(mon, res))]) from mret) as worst_inc_month,
       -- the investments card says what happened: a gain or a loss
       case when g.result - coalesce((select result from gret where grp = 'BANK'), 0) >= 0 then 'Доход от инвестиций' else 'Убыток по инвестициям' end as inv_title,
       -- whether the bank portfolio pages have anything to show
       (select count(*) from acc where src_cd = 'BANK' and close_dt >= (select s from per)) as has_bank,
       -- the forecast table's month headers (the year under the first month and under January)
       (select mshort from mnames)[month(cast((select e from per) + interval 1 month as date))] || case when month(cast((select e from per) + interval 1 month as date)) = 1 or 1 = 1 then chr(10) || year(cast((select e from per) + interval 1 month as date)) else '' end as fc_m1,
       (select mshort from mnames)[month(cast((select e from per) + interval 2 month as date))] || case when month(cast((select e from per) + interval 2 month as date)) = 1 or 2 = 1 then chr(10) || year(cast((select e from per) + interval 2 month as date)) else '' end as fc_m2,
       (select mshort from mnames)[month(cast((select e from per) + interval 3 month as date))] || case when month(cast((select e from per) + interval 3 month as date)) = 1 or 3 = 1 then chr(10) || year(cast((select e from per) + interval 3 month as date)) else '' end as fc_m3,
       (select mshort from mnames)[month(cast((select e from per) + interval 4 month as date))] || case when month(cast((select e from per) + interval 4 month as date)) = 1 or 4 = 1 then chr(10) || year(cast((select e from per) + interval 4 month as date)) else '' end as fc_m4,
       (select mshort from mnames)[month(cast((select e from per) + interval 5 month as date))] || case when month(cast((select e from per) + interval 5 month as date)) = 1 or 5 = 1 then chr(10) || year(cast((select e from per) + interval 5 month as date)) else '' end as fc_m5,
       (select mshort from mnames)[month(cast((select e from per) + interval 6 month as date))] || case when month(cast((select e from per) + interval 6 month as date)) = 1 or 6 = 1 then chr(10) || year(cast((select e from per) + interval 6 month as date)) else '' end as fc_m6,
       (select mshort from mnames)[month(cast((select e from per) + interval 7 month as date))] || case when month(cast((select e from per) + interval 7 month as date)) = 1 or 7 = 1 then chr(10) || year(cast((select e from per) + interval 7 month as date)) else '' end as fc_m7,
       (select mshort from mnames)[month(cast((select e from per) + interval 8 month as date))] || case when month(cast((select e from per) + interval 8 month as date)) = 1 or 8 = 1 then chr(10) || year(cast((select e from per) + interval 8 month as date)) else '' end as fc_m8,
       (select mshort from mnames)[month(cast((select e from per) + interval 9 month as date))] || case when month(cast((select e from per) + interval 9 month as date)) = 1 or 9 = 1 then chr(10) || year(cast((select e from per) + interval 9 month as date)) else '' end as fc_m9,
       (select mshort from mnames)[month(cast((select e from per) + interval 10 month as date))] || case when month(cast((select e from per) + interval 10 month as date)) = 1 or 10 = 1 then chr(10) || year(cast((select e from per) + interval 10 month as date)) else '' end as fc_m10,
       (select mshort from mnames)[month(cast((select e from per) + interval 11 month as date))] || case when month(cast((select e from per) + interval 11 month as date)) = 1 or 11 = 1 then chr(10) || year(cast((select e from per) + interval 11 month as date)) else '' end as fc_m11,
       (select mshort from mnames)[month(cast((select e from per) + interval 12 month as date))] || case when month(cast((select e from per) + interval 12 month as date)) = 1 or 12 = 1 then chr(10) || year(cast((select e from per) + interval 12 month as date)) else '' end as fc_m12,
       -- what «received» covers: the period, e.g. «01.01–15.06»
       strftime((select s from per), '%d.%m') || '–' || strftime((select e from per), '%d.%m') as fact_range,
       -- the forecast's 12 months, e.g. «Июл 2026 – Июн 2027»
       (select mshort from mnames)[month(cast((select e from per) + interval 1 month as date))] || ' ' || year(cast((select e from per) + interval 1 month as date))
         || ' – ' || (select mshort from mnames)[month(cast((select e from per) + interval 12 month as date))] || ' ' || year(cast((select e from per) + interval 12 month as date)) as fc_range,
       -- the period's best and worst months (by return), for the contribution titles
       (select (select mfull from mnames)[month(arg_max(mon, ret))] from mret) as best_month,
       (select (select mfull from mnames)[month(arg_min(mon, ret))] from mret) as worst_month
from gret g where g.grp = 'ALL'"#,
        "cat" => r#"
, prod as (select *, case when src_cd = 'AM' then 'Доверительное управл.' when instr_type = 'Funds' then 'Паевые фонды · ПИФ'
                          when instr_type = 'Metals' then 'Металлы · ОМС' when src_cd = 'BANK' then 'Депозиты и вклады'
                          when src_cd = 'BROK' then 'Брокерские счета' else 'Страхование' end as product from pos),
-- an account's taxes, fees and cash revaluation go to its own product, so the products add up to the portfolio's result
adj as (select d, case src_cd when 'AM' then 'Доверительное управл.' when 'BANK' then 'Депозиты и вклады'
                    when 'BROK' then 'Брокерские счета' else 'Страхование' end as product from acct_adj),
all_rows as (select product, v1, v0, fin, result from prod union all select product, 0, 0, 0, d from adj),
-- what a product holds, in words: its accounts (or policies) and positions
held as (select product, count(distinct acct_cd) filter (v1 > 0) as na, count(*) filter (v1 > 0 and instr_type <> 'Currency') as np from prod group by 1)
select a.product, sum(v1) as value, sum(result) as income, sum(result) / nullif(sum(v0) + sum(greatest(fin, 0)), 0) as ret,
       case when coalesce(max(h.na), 0) = 0 then null when a.product = 'Страхование' then max(h.na) || ' ' || case when max(h.na) % 10 = 1 and max(h.na) % 100 <> 11 then 'полис' when max(h.na) % 10 between 2 and 4 and max(h.na) % 100 not between 12 and 14 then 'полиса' else 'полисов' end
            when a.product = 'Депозиты и вклады' then max(h.na) || ' ' || case when max(h.na) % 10 = 1 and max(h.na) % 100 <> 11 then 'счёт' when max(h.na) % 10 between 2 and 4 and max(h.na) % 100 not between 12 and 14 then 'счёта' else 'счетов' end
            else max(h.na) || ' ' || case when max(h.na) % 10 = 1 and max(h.na) % 100 <> 11 then 'счёт' when max(h.na) % 10 between 2 and 4 and max(h.na) % 100 not between 12 and 14 then 'счёта' else 'счетов' end || ' · ' || max(h.np) || ' ' || case when max(h.np) % 10 = 1 and max(h.np) % 100 <> 11 then 'позиция' when max(h.np) % 10 between 2 and 4 and max(h.np) % 100 not between 12 and 14 then 'позиции' else 'позиций' end end as held
from all_rows a left join held h using (product) group by 1 having sum(v1) > 0 or sum(result) <> 0 order by value desc"#,
        "cls" => r#"
, k as (select *, case instr_type when 'Equity' then 'Акции' when 'Funds' then 'Фонды' when 'Metals' then 'Драгметаллы'
                                  when 'Bonds' then 'Облигации' when 'Insurance' then 'Прочие' else 'Денежные средства' end as class from pos),
-- an account's taxes, fees and cash revaluation are the cash's, so the classes add up to the portfolio's result
x as (select class, v1, v0, fin, result from k union all select 'Денежные средства', 0, 0, 0, d from acct_adj)
select class, sum(v1) as value, sum(result) as income, sum(result) / nullif(sum(v0) + sum(greatest(fin, 0)), 0) as ret
from x group by 1 having sum(v1) > 0 order by value desc"#,
        "cur" => r#"
, k as (select *, case instr_cur when 'RUB' then 'Российский рубль' when 'USD' then 'Доллар США' when 'CNY' then 'Юань'
                                 when 'EUR' then 'Евро' when 'AED' then 'Дирхам ОАЭ' else instr_cur end as currency from pos),
-- taxes, fees and cash revaluation are booked in roubles: the currencies add up to the portfolio's result
x as (select currency, v1, v0, fin, result from k union all select 'Российский рубль', 0, 0, 0, d from acct_adj)
select currency, sum(v1) as value, sum(result) as income, sum(result) / nullif(sum(v0) + sum(greatest(fin, 0)), 0) as ret
from x group by 1 having sum(v1) > 0 order by value desc"#,
        "top5" => r#"
select case when instr_type in ('Deposit', 'Savings') then acct_name || ' ' || short_no else instr_name end as name,
       case src_cd when 'BROK' then 'Брокерский счёт ' || acct_numb when 'AM' then 'ДУ · ' || coalesce(manager, '') else 'Тест Банк' end
         || ' · ' || case instr_type when 'Equity' then 'акции' when 'Funds' then 'фонды' when 'Bonds' then 'облигации'
                                     when 'Metals' then 'драгметаллы' else 'денежные средства' end as sub,
       v1 as value, v1 / sum(v1) over () as share
from pos where v1 > 0 order by v1 desc limit 5"#,
        "mon" => r#"
select least(last_day(report_dt), (select e from per)) as date, sum(bo - bi - nf) as income
from gday where grp = 'ALL' and report_dt between (select s from per) and (select e from per)
group by 1 order by 1"#,
        // monthly and cumulative returns of each group, by the net_twr_ltd rule
        "am" => r#"
, ends as (select distinct least(last_day(report_dt), (select e from per)) as d from gday
           where report_dt between (select s from per) and (select e from per)),
x as (select g.grp, ends.d, g.ltd from gltd g join ends on g.report_dt = ends.d),
b as (select grp, ltd as l0 from gltd where report_dt = (select s from per) - 1)
select x.grp as acct, x.d as date,
       x.ltd / coalesce(lag(x.ltd) over (partition by x.grp order by x.d), b.l0, 1) - 1 as ret,
       x.ltd / coalesce(b.l0, 1) - 1 as cum
from x left join b using (grp) order by 1, 2"#,
        "hz" => r#"
, h as (select * from (values ('1 месяц', 1), ('3 месяца', 3), ('6 месяцев', 6), ('12 месяцев', 12)) t(label, n))
select g.grp as acct, h.label,
       max(g.ltd) filter (g.report_dt = (select e from per))
         / coalesce(max(g.ltd) filter (g.report_dt = cast((select e from per) - to_months(h.n) as date)), 1) - 1 as ret
from gltd g cross join h group by g.grp, h.label, h.n order by g.grp, h.n"#,
        "acct" => r#"
select grp as acct, value, start, value / nullif(start, 0) - 1 as value_chg, ret as twr, result,
       round((ret - (select ret from bret where bench_cd = 'IMOEX')) * 100, 1) as vs,
       deposits + withdrawals as net, deposits, withdrawals, result as inv,
       -- income net of fees and taxes (they are negative), and the rest of the result: revaluation
       inc + charges as divs, result - inc - charges as reval, value - start as change,
       -- as text, for the totals line under each account's holdings
       case when result < 0 then '−' else '+' end || replace(format('{:,}', cast(round(abs(result)) as bigint)), ',', ' ') || ' ₽' as result_txt,
       case when result < 0 then 'bad' else 'good' end as result_tone,
       replace(format('{:,}', cast(round(value) as bigint)), ',', ' ') || ' ₽' as value_txt
from gret where grp <> 'ALL' order by value desc"#,
        "cmp" => r#"
, kinds as (select distinct grp, src_cd from acc)
select acct, name, ret from (
  select g.grp as acct, case k.src_cd when 'AM' then 'Портфель' when 'INS' then 'Полис' else 'Счёт' end as name, g.ret, 1 as o
  from gret g join kinds k using (grp) where g.grp not in ('ALL', 'BANK')
  union all select g.grp, 'Счета', g.ret, 1 from gret g where g.grp = 'BANK'
  union all select g.grp, 'Портфели РФ', (select median(ret) from others), 2 from gret g where g.grp not in ('ALL', 'BANK')
  union all select g.grp, 'МосБиржа', (select ret from bret where bench_cd = 'IMOEX'), 3 from gret g where g.grp not in ('ALL', 'BANK')
  union all select g.grp, 'Вклады, рынок', (select ret from dep_mkt), 3 from gret g where g.grp = 'BANK'
  union all select g.grp, 'Инфляция', (select ret from bret where bench_cd = 'CPI_RU'), 4 from gret g where g.grp <> 'ALL'
  union all select 'COHORT', 'мин', (select min(ret) from others), 5
  union all select 'COHORT', 'медиана', (select median(ret) from others), 6
  union all select 'COHORT', 'Вы', (select ret from me), 7
  union all select 'COHORT', 'макс', (select max(ret) from others), 8)
order by acct, o"#,
        "acls" => r#"
, k as (select case when src_cd = 'BANK' then case acct_type when 'Текущий счёт' then 'Платёжные счета' when 'ОМС' then 'Драгметаллы (ОМС)' else 'Вклады и счета' end
                       when instr_type in ('Equity', 'Funds') then 'Акции и фонды' when instr_type = 'Bonds' then 'Облигации' else 'Денежные средства и металлы' end as class,
               grp, v0, v1, fin, result
        from pos where src_cd <> 'AM'
        -- the account's taxes, fees and cash revaluation: its classes then add up to its result (as the holdings table does)
        union all
        select case src_cd when 'BANK' then 'Вклады и счета' else 'Денежные средства и металлы' end, grp, 0, 0, 0, d from acct_adj where src_cd <> 'AM' and abs(d) >= 1)
select grp as acct, class, sum(result) as income, sum(result) / nullif(sum(v0) + sum(greatest(fin, 0)), 0) as ret, null as sub, sum(v1) as value,
       case class when 'Денежные средства и металлы' then 'Деньги' when 'Акции и фонды' then 'Акции' when 'Вклады и счета' then 'Вклады'
                  when 'Платёжные счета' then 'Платёжные' when 'Драгметаллы (ОМС)' then 'Металлы' else class end as short,
       -- the class's value and share of its account, as text
       replace(format('{:,}', cast(round(sum(v1)) as bigint)), ',', ' ') || ' ₽ · '
         -- an account worth nothing at the end (closed in the period) has no shares
         || coalesce(cast(round(100 * sum(v1) / nullif(sum(sum(v1)) over (partition by grp), 0)) as integer) || '%', '—') as note
from k group by grp, class having sum(v1) > 0 or sum(result) <> 0 order by acct, value desc"#,
        // every position's result, per group and for all accounts (the leaders tables take the top and bottom three)
        "inst" => r#"
, k as (select *, case when src_cd = 'BANK' then acct_name || ' ' || short_no else instr_name end as name,
               result / nullif(v0 + greatest(fin, 0), 0) as r
        from pos where instr_type <> 'Currency' and (v0 <> 0 or v1 <> 0)),
x as (select grp as acct, name, result, r from k union all select 'ALL', name, result, r from k where src_cd <> 'BANK'),
n as (select *, row_number() over (partition by acct order by result desc) as hi, row_number() over (partition by acct order by result) as lo from x)
-- the three best and the three worst of each group
select acct, name, result as income, r as ret from n where hi <= 3 or lo <= 3 order by acct, result desc"#,
        // the account groups' contribution to the best and the worst month, in percentage points
        "contrib" => r#"
-- the best and the worst month by return
, m as (select * from mret),
r as (select mon, 'best' as role from (select mon from m order by ret desc limit 1)
      union all select mon, 'worst' from (select mon from m order by ret limit 1)),
a as (select date_trunc('month', p.report_dt) as mon,
             case ac.src_cd when 'BROK' then 'Брокер. ' || ac.acct_numb when 'AM' then 'ДУ · ' || coalesce(replace(ac.manager, 'УК ', ''), '')
                            else case ac.acct_type when 'Накопительный счёт' then 'Накопит. ' || ac.short_no when 'Депозит' then 'Вклад ' || ac.short_no
                                                   when 'ОМС' then 'ОМС металлы' else 'Платёжные счета' end end as account,
             sum(p.bal_out - p.bal_in - p.net_flow) as res
      from portf_acct p join acc ac using (acct_cd)
      where p.report_dt between (select s from per) and (select e from per) group by 1, 2),
ranked as (select r.role, r.mon, a.account, a.res / m.cap * 100 as pp,
                  -- what moved the month most, either way: a weak month's drag and what still carried it
                  row_number() over (partition by r.role order by abs(a.res) desc) as n
           from r join a using (mon) join m using (mon) where a.res <> 0),
top as (select role, mon, case when n <= 4 then account else 'Прочие' end as account, sum(pp) as pp, min(n) as n from ranked group by all)
-- month: best or worst; month_name: its name, for the titles
select role as month, (select mfull from mnames)[month(mon)] as month_name, account, round(pp, 2) as pp
from top order by role, n"#,
        "vs" => r#"
select metric, round(pp, 1) as pp from (
  select 'VS похожие клиенты' as metric, ((select ret from me) - (select median(ret) from others)) * 100 as pp, 1 as o
  union all select 'VS индекс МосБиржи', ((select ret from me) - (select ret from bret where bench_cd = 'IMOEX')) * 100, 2
  union all select 'VS депозит', ((select ret from me) - (select ret from dep_mkt)) * 100, 3
  union all select 'VS инфляция', ((select ret from me) - (select ret from bret where bench_cd = 'CPI_RU')) * 100, 4) order by o"#,
        // how the capital went from the start of the period to its end: money in, money out, what it earned
        "bridge" => r#"
select step, amount from (
  select 'Начало' as step, start as amount, 1 as o from gret where grp = 'ALL'
  union all select 'Пополнения', deposits, 2 from gret where grp = 'ALL'
  union all select 'Выводы', withdrawals, 3 from gret where grp = 'ALL'
  union all select 'Вклады', (select v from depinc), 4
  union all select 'Инвестиции', (select result from gret where grp = 'ALL') - (select v from depinc), 5)
order by o"#,
        "bench" => r#"
select name, ret from (
  select 'Ваш портфель' as name, (select ret from me) as ret
  union all select 'Индекс МосБиржи', (select ret from bret where bench_cd = 'IMOEX')
  union all select 'Индекс МосБиржи полн. дох.', (select ret from bret where bench_cd = 'MCFTR')
  union all select 'Индекс гособлигаций', (select ret from bret where bench_cd = 'RGBI')
  union all select 'S&P 500', (select ret from bret where bench_cd = 'SPX')
  union all select 'Инфляция', (select ret from bret where bench_cd = 'CPI_RU')
  union all select 'Курс USD/RUB', (select ret from usd)
  -- what the client could compare with besides markets: similar clients and a deposit at the key rate
  union all select 'Похожие клиенты · медиана', (select median(ret) from others)
  union all select 'Вклад по ключевой ставке', (select ret from dep_mkt)) order by ret desc"#,
        // the client against similar clients: return, drawdown, profitable months, Sharpe, volatility, currencies
        "cohort" => r#"
, cday as (select client_id, report_dt, tw, exp(sum(ln(case when tw > 0 then tw else 1 end)) over (partition by client_id order by report_dt)) as ltd from peer_tw),
stats as (
  select client_id,
         exp(sum(ln(case when tw > 0 then tw else 1 end))) - 1 as ret,
         stddev_samp(tw - 1) filter (dayofweek(report_dt) between 1 and 5) * sqrt(252) as vol,
         avg(tw - 1) filter (dayofweek(report_dt) between 1 and 5) * 252 as ann
  from peer_tw group by 1),
dd as (select client_id, min(dd) as dd from (select client_id, ltd / max(ltd) over (partition by client_id order by report_dt) - 1 as dd from cday) group by 1),
mm as (select client_id, count(*) filter (r > 1) as up from
         (select client_id, date_trunc('month', report_dt) as m, exp(sum(ln(case when tw > 0 then tw else 1 end))) as r from peer_tw group by 1, 2) group by 1),
cur as (select a.client_id, count(distinct b.price_cur) as n from balances b join accounts a using (acct_cd)
        where b.report_dt = (select e from per) and a.client_id in (select client_id from peer) group by 1),
t as (select s.client_id, s.ret, d.dd, m.up, s.ann / nullif(s.vol, 0) as sharpe, s.vol, c.n
      from stats s join dd d using (client_id) join mm m using (client_id) left join cur c using (client_id)),
mine as (select * from t where client_id = (select id from cl)),
rest as (select * from t where client_id <> (select id from cl))
select metric, value, median, value - median as diff from (
  select 'Доходность YTD' as metric, (select ret from mine) as value, (select median(ret) from rest) as median, 1 as o
  union all select 'Устойчивость · просадка', (select dd from mine), (select median(dd) from rest), 2
  union all select 'Прибыльных месяцев', (select up from mine), (select median(up) from rest), 3
  union all select 'Коэффициент Шарпа', (select sharpe from mine), (select median(sharpe) from rest), 4
  union all select 'Волатильность', (select vol from mine), (select median(vol) from rest), 5
  union all select 'Валют в портфеле', (select n from mine), (select median(n) from rest), 6) order by o"#,
        // the brokerage account's positions
        "hold" => r#"
select * from (
select case when instr_type in ('Equity', 'Funds') then 'Акции и фонды' when instr_type = 'Bonds' then 'Облигации' else 'Денежные средства и металлы' end as bucket,
       instr_name as name,
       case when instr_type = 'Currency' then 'Валюта счёта · ' || instr_cur when instr_type = 'Metals' then 'Обезличенный металлический счёт'
            when instr_type = 'Bonds' then 'ISIN ' || coalesce(isin, '') || ' · погашение ' || strftime(i.maturity_dt, '%d.%m.%Y')
                                       || ' · купон ' || replace(printf('%.2f', i.coupon), '.', ',') || '%'
            else 'ISIN ' || coalesce(isin, '') end as isin,
       -- bond prices in % of nominal
       qty, case when instr_type = 'Currency' then null when instr_type = 'Bonds' then avg_price * 100 else avg_price end as avg,
       case when instr_type = 'Currency' then null when instr_type = 'Bonds' then price * 100 else price end as price,
       -- the price against the purchase price (the bank's book price of the position)
       case when instr_type = 'Currency' then null else price / nullif(avg_price, 0) - 1 end as chg,
       v1 as value, nullif(received, 0) as divs, result as income,
       case when instr_type = 'Currency' then null else result / nullif(v0 + greatest(fin, 0), 0) end as ret,
       v1 / sum(v1) over (partition by acct_cd) as share, v0 + greatest(fin, 0) as start, acct_cd as acct,
       -- a bond's days to maturity (the table's rule decides which count as soon)
       case when instr_type = 'Bonds' and i.maturity_dt >= (select e from per) then i.maturity_dt - (select e from per) end as days_left
from pos left join (select instr_cd, maturity_dt, coupon from instruments) i using (instr_cd)
-- held at the end, or sold or matured in the period (their payments and result count)
where src_cd = 'BROK' and (v1 > 0 or received <> 0 or (result <> 0 and instr_type <> 'Currency'))
-- the account's taxes, fees and cash revaluation: the table's result is then the account's
union all
select 'Денежные средства и металлы', 'Налоги, комиссии и курсовая разница', 'по счёту за период', null, null, null, null,
       null, null, d, null, null, null, grp, null
from acct_adj where src_cd = 'BROK' and abs(d) >= 1
)
order by acct, case bucket when 'Акции и фонды' then 1 when 'Облигации' then 2 else 3 end, value desc nulls last"#,
        "mix" => r#"
, k as (select *, case when src_cd = 'BANK' then case acct_type when 'Текущий счёт' then 'Платёжные счета' when 'ОМС' then 'Драгметаллы (ОМС)'
                                                                else 'Вклады и накопительные счета' end
                       when src_cd = 'AM' then case when instr_type = 'Currency' then 'Денежные средства'
                                                    when instr_type = 'Bonds' or instr_name ilike '%облигац%' then 'Облигации и фонды обл.' else 'Акции и фонды акций' end
                       when instr_type in ('Equity', 'Funds') then 'Акции и фонды' when instr_type = 'Bonds' then 'Облигации'
                       else 'Денежные средства и металлы' end as kind from pos)
select grp as acct, kind, sum(v1) as value, sum(result) + case when kind in ('Денежные средства', 'Денежные средства и металлы') then coalesce((select d from acct_adj j where j.grp = k.grp), 0) else 0 end as result
from k group by 1, 2 having sum(v1) > 0 order by acct, value desc"#,
        "duh" => r#"
select * from (
select case when instr_type = 'Currency' then 'Денежная позиция' when instr_type = 'Bonds' or instr_name ilike '%облигац%' then 'Облигации и фонды облигаций'
            else 'Акции и фонды акций' end as bucket,
       case when instr_type = 'Currency' then 'Рубли РФ' else instr_name end as name,
       case when instr_type = 'Currency' then 'денежные средства · ' || instr_cur else 'ISIN ' || coalesce(isin, '') end as isin,
       case when instr_type = 'Currency' then null else qty end as units,
       -- bond prices in % of nominal; the price against the purchase price (as in the brokerage account)
       case when instr_type = 'Currency' then null when instr_type = 'Bonds' then avg_price * 100 else avg_price end as avg,
       case when instr_type = 'Currency' then null when instr_type = 'Bonds' then price * 100 else price end as price,
       case when instr_type = 'Currency' then null else price / nullif(avg_price, 0) - 1 end as chg,
       v1 as value, nullif(received, 0) as received, result as income, case when instr_type = 'Currency' then null else result / nullif(v0 + greatest(fin, 0), 0) end as ret,
       v1 / sum(v1) over (partition by acct_cd) as share, acct_cd as acct
-- held at the end, or sold or matured in the period (their payments and result count)
from pos where src_cd = 'AM' and (v1 > 0 or received <> 0 or (result <> 0 and instr_type <> 'Currency'))
-- the mandate's fees, taxes and cash revaluation: the table's result is then the account's
union all
select 'Денежная позиция', 'Комиссии, налоги и курсовая разница', 'по счёту за период', null, null, null, null, null, null, d, null, null, grp
from acct_adj where src_cd = 'AM' and abs(d) >= 1
)
order by acct, case bucket when 'Акции и фонды акций' then 1 when 'Облигации и фонды облигаций' then 2 else 3 end, value desc nulls last"#,
        "dumix" => r#"
, k as (select *, case when instr_type = 'Currency' then 'Денежные средства'
                       when instr_type = 'Bonds' or instr_name ilike '%облигац%' then 'Облигации и фонды обл.' else 'Акции и фонды акций' end as class
        from pos where src_cd = 'AM' and v1 > 0)
select acct_cd as acct, class, string_agg(case when instr_type = 'Currency' then 'Рубли РФ' else instr_name end, ', ' order by v1 desc) as detail, sum(v1) as value,
       sum(v1) / max(t.total) as share,
       replace(printf('%.1f', sum(v1) / max(t.total) * 100), '.', ',') || '% портфеля' as sub,
       case class when 'Акции и фонды акций' then 'Акции' when 'Облигации и фонды обл.' then 'Облигации' else 'Деньги' end as short
from k join (select acct_cd, sum(v1) as total from k group by 1) t using (acct_cd)
group by acct_cd, class order by acct_cd, value desc"#,
        "about" => r#"
select acct, key, value from (
  select acct_cd as acct, 'Управляющая компания' as key, manager as value, 1 as o from acc where src_cd = 'AM'
  union all select acct_cd, 'Стратегия', replace(acct_name, ' стратегия', ''), 2 from acc where src_cd = 'AM'
  union all select acct_cd, 'Счёт открыт', strftime(open_dt, '%d.%m.%Y'), 3 from acc where src_cd = 'AM'
  union all select acct_cd, 'Инструменты', cast(count(*) as varchar) || case when count(*) % 10 = 1 and count(*) % 100 <> 11 then ' инструмент'
                                                                   when count(*) % 10 between 2 and 4 and count(*) % 100 not between 12 and 14 then ' инструмента'
                                                                   else ' инструментов' end, 4
            from pos where src_cd = 'AM' and instr_type <> 'Currency' and v1 > 0 group by acct_cd)
order by acct, o"#,
        // the bank accounts: rate, term, balance, result, closing soon, open or closed
        "bank" => r#"
, k as (
  select a.*, i.coupon, coalesce(e.v1, 0) as sum_e, a.close_dt <= (select e from per) as is_closed,
         coalesce(sum(p.bal_out - p.bal_in - p.net_flow) filter (p.report_dt between (select s from per) and (select e from per)), 0) as result,
         -- interest for the period, to the day a deposit closed (paid out with it)
         coalesce(sum(p.income) filter (p.report_dt between (select s from per) and (select e from per)), 0) as interest,
         coalesce(max(p.bal_out) filter (p.report_dt = (select s from per) - 1), 0)
           + coalesce(sum(greatest(p.net_flow, 0)) filter (p.report_dt between (select s from per) and (select e from per)), 0) as cap
  from acc a left join pa p using (acct_cd)
  left join (select acct_cd, sum(v1) as v1, max(instr_cd) as instr_cd from bal_e group by 1) e using (acct_cd)
  left join instruments i on i.instr_cd = e.instr_cd and i.instr_type in ('Deposit', 'Savings')
  where a.src_cd = 'BANK' and (a.close_dt >= (select s from per) or a.close_dt is null)
  group by all),
-- when it ends: a deposit at its maturity (the account's close date may lie beyond the data)
k2 as (select *, case when maturity_dt is not null and maturity_dt < close_dt then maturity_dt else close_dt end as ends from (
  select k.*, i2.maturity_dt from k left join instruments i2 on i2.instr_cd = (select max(instr_cd) from bal_e b where b.acct_cd = k.acct_cd) and i2.instr_type = 'Deposit'))
select case when acct_type = 'ОМС' then 'Драгметаллы · обезличенные металлические счета'
            when acct_type = 'Текущий счёт' or acct_name ilike '%карта%' then 'Платёжные счета и карты' else 'Процентные счета и вклады' end as "group",
       short_no || ' · ' || acct_name as account, coupon / 100 as rate,
       case when is_closed then 'закрыт ' || strftime(close_dt, '%d.%m.%Y')
            when ends < date '3000-01-01' then 'до ' || strftime(ends, '%d.%m.%Y') else 'бессрочный' end as term,
       case when is_closed then null else sum_e end as sum, nullif(interest, 0) as interest, result,
       case when acct_type = 'Текущий счёт' then null else result / nullif(cap, 0) end as ret,
       case when is_closed then null else sum_e / nullif(sum(case when is_closed then 0 else sum_e end) over (), 0) end as share,
       -- days until it ends (the table's rule decides which count as soon)
       case when not is_closed and ends < date '3000-01-01' and ends >= (select e from per) then ends - (select e from per) end as soon,
       case when is_closed then 0 else 1 end as open, case when is_closed then 1 else 0 end as closed
from k2 order by case when acct_type = 'ОМС' then 2 when acct_type = 'Текущий счёт' or acct_name ilike '%карта%' then 3 else 1 end, is_closed, sum_e desc"#,
        // the forecast page: received in the period and the next 12 months, millions of roubles
        "cfm" => r#"
, kinds as (select * from (values ('Проценты по вкладам', 'Проценты', 'вклады и накопительные счета', 1), ('Дивиденды', 'Дивиденд', 'портфель акций', 2),
                                  ('Купоны облигаций', 'Купон', 'облигации', 3)) t(source, cf, sub, o)),
fact as (select 'Проценты' as cf, (select sum(inc) from gday where grp = 'BANK' and report_dt between (select s from per) and (select e from per)) as v
         union all select op_type, sum(amount_rub) from ops where op_type in ('Дивиденд', 'Купон') group by 1),
items as (
  select 'item' as kind, k.source, k.sub, round((select v from fact where fact.cf = k.cf) / 1e6, 1) as fact,
         round(sum(rub) filter (m = 1) / 1e6, 1) as m1, round(sum(rub) filter (m = 2) / 1e6, 1) as m2, round(sum(rub) filter (m = 3) / 1e6, 1) as m3,
         round(sum(rub) filter (m = 4) / 1e6, 1) as m4, round(sum(rub) filter (m = 5) / 1e6, 1) as m5, round(sum(rub) filter (m = 6) / 1e6, 1) as m6,
         round(sum(rub) filter (m = 7) / 1e6, 1) as m7, round(sum(rub) filter (m = 8) / 1e6, 1) as m8, round(sum(rub) filter (m = 9) / 1e6, 1) as m9,
         round(sum(rub) filter (m = 10) / 1e6, 1) as m10, round(sum(rub) filter (m = 11) / 1e6, 1) as m11, round(sum(rub) filter (m = 12) / 1e6, 1) as m12,
         sum(rub) / 1e6 as tot, k.o
  from kinds k left join fc on fc.cf_type = k.cf group by all)
select kind, source, sub, fact, m1, m2, m3, m4, m5, m6, m7, m8, m9, m10, m11, m12,
       round(coalesce(fact, 0) + coalesce(tot, 0), 1) as total
from (select *, o as ord from items
      -- regular income from the payments themselves, rounded once (a sum of rounded parts drifts: 3.7 for 3.6)
      union all select 'sub', 'Регулярный доход', '', round((select sum(v) from fact) / 1e6, 1),
                       round(sum(rub) filter (m = 1) / 1e6, 1), round(sum(rub) filter (m = 2) / 1e6, 1), round(sum(rub) filter (m = 3) / 1e6, 1),
                       round(sum(rub) filter (m = 4) / 1e6, 1), round(sum(rub) filter (m = 5) / 1e6, 1), round(sum(rub) filter (m = 6) / 1e6, 1),
                       round(sum(rub) filter (m = 7) / 1e6, 1), round(sum(rub) filter (m = 8) / 1e6, 1), round(sum(rub) filter (m = 9) / 1e6, 1),
                       round(sum(rub) filter (m = 10) / 1e6, 1), round(sum(rub) filter (m = 11) / 1e6, 1), round(sum(rub) filter (m = 12) / 1e6, 1),
                       sum(rub) / 1e6, 4, 4 from fc where cf_type <> 'Погашение'
      union all select 'ret', 'Возврат капитала', 'погашения и закрытия', null,
                       round(sum(rub) filter (m = 1) / 1e6, 1), round(sum(rub) filter (m = 2) / 1e6, 1), round(sum(rub) filter (m = 3) / 1e6, 1),
                       round(sum(rub) filter (m = 4) / 1e6, 1), round(sum(rub) filter (m = 5) / 1e6, 1), round(sum(rub) filter (m = 6) / 1e6, 1),
                       round(sum(rub) filter (m = 7) / 1e6, 1), round(sum(rub) filter (m = 8) / 1e6, 1), round(sum(rub) filter (m = 9) / 1e6, 1),
                       round(sum(rub) filter (m = 10) / 1e6, 1), round(sum(rub) filter (m = 11) / 1e6, 1), round(sum(rub) filter (m = 12) / 1e6, 1),
                       sum(rub) / 1e6, 5, 5 from fc where cf_type = 'Погашение')
order by ord"#,
        "fcm" => r#"
, months as (select range as m from range(1, 13))
select (select mshort from mnames)[month(cast((select e from per) + to_months(cast(months.m as integer)) as date))] as month,
       -- in millions, not rounded: the cards add the months up and round once (3.7, as page 3 says, not 3.8)
       coalesce(sum(rub) filter (cf_type <> 'Погашение') / 1e6, 0) as regular,
       coalesce(sum(rub) filter (cf_type = 'Погашение') / 1e6, 0) as principal
from months left join fc on fc.m = months.m group by months.m order by months.m"#,
        "cfi" => r#"
, g as (select * from (values ('gd', 'ПРОЦЕНТЫ ПО ВКЛАДАМ И СЧЕТАМ', 'Проценты', 1), ('gv', 'ДИВИДЕНДЫ ПО АКЦИЯМ', 'Дивиденд', 2),
                              ('gc', 'КУПОНЫ ОБЛИГАЦИЙ', 'Купон', 3), ('gr', 'ВОЗВРАТ КАПИТАЛА · ПОГАШЕНИЯ И ЗАКРЫТИЯ', 'Погашение', 4)) t(kind, name, cf, o)),
-- received in the period: interest by account, dividends and coupons by security
got as (select 'Проценты' as cf, acct_cd as key, sum(income) as v from pa
        where grp = 'BANK' and report_dt between (select s from per) and (select e from per) group by 2
        union all select op_type, instr_cd, sum(amount_rub) from ops where op_type in ('Дивиденд', 'Купон') group by 1, 2),
items as (
  select g.o, g.cf, case when fcx.instr_type in ('Deposit', 'Savings') then fcx.acct_cd else fcx.instr_cd end as key,
         case when fcx.instr_type in ('Deposit', 'Savings') then fcx.acct_name || ' ' || fcx.short_no || ' · ' || lower(case g.cf when 'Погашение' then 'закрытие' else 'проценты' end)
                   || coalesce(' · ' || replace(printf('%.1f', fcx.coupon), '.', ',') || '%', '')
              else fcx.instr_name || ' · ' || case g.cf when 'Дивиденд' then 'дивиденды' when 'Купон' then 'купон' else 'погашение' end end as name,
         round(sum(rub) filter (m = 1) / 1e3) as m1, round(sum(rub) filter (m = 2) / 1e3) as m2, round(sum(rub) filter (m = 3) / 1e3) as m3,
         round(sum(rub) filter (m = 4) / 1e3) as m4, round(sum(rub) filter (m = 5) / 1e3) as m5, round(sum(rub) filter (m = 6) / 1e3) as m6,
         round(sum(rub) filter (m = 7) / 1e3) as m7, round(sum(rub) filter (m = 8) / 1e3) as m8, round(sum(rub) filter (m = 9) / 1e3) as m9,
         round(sum(rub) filter (m = 10) / 1e3) as m10, round(sum(rub) filter (m = 11) / 1e3) as m11, round(sum(rub) filter (m = 12) / 1e3) as m12,
         round(sum(rub) / 1e3) as total
  from g join fcx on fcx.cf_type = g.cf group by all),
withgot as (select items.*, round(got.v / 1e3) as fact,
                   row_number() over (partition by items.cf order by items.total desc nulls last) as n,
                   count(*) over (partition by items.cf) as cnt
            from items left join got on got.cf = items.cf and got.key = items.key),
-- each group's three largest, the rest as one line (the page holds about twenty rows)
shown as (select o, cf, case when n <= 3 or cnt = 4 then name else 'Прочие · ' || (cnt - 3) || ' поз.' end as name,
                 sum(fact) as fact, sum(m1) as m1, sum(m2) as m2, sum(m3) as m3, sum(m4) as m4, sum(m5) as m5, sum(m6) as m6, sum(m7) as m7,
                 sum(m8) as m8, sum(m9) as m9, sum(m10) as m10, sum(m11) as m11, sum(m12) as m12, sum(total) as total, min(n) as n
          from withgot group by all)
select kind, name, fact, m1, m2, m3, m4, m5, m6, m7, m8, m9, m10, m11, m12, total from (
  select g.kind, g.name, null as fact, null as m1, null as m2, null as m3, null as m4, null as m5, null as m6, null as m7, null as m8, null as m9,
         null as m10, null as m11, null as m12, null as total, g.o, 0 as s from g where exists (select 1 from items where items.cf = g.cf)
  union all select 'i', name, fact, m1, m2, m3, m4, m5, m6, m7, m8, m9, m10, m11, m12, total, o, 1 + n from shown)
order by o, s"#,
        "inflow" => r#"
select kind, amount from (
  select 'Возврат капитала' as kind, round(coalesce(sum(rub) filter (cf_type = 'Погашение'), 0) / 1e6, 1) as amount, 1 as o from fc
  union all select 'Регулярный доход', round(coalesce(sum(rub) filter (cf_type <> 'Погашение'), 0) / 1e6, 1), 2 from fc) order by o"#,
        "srcs" => r#"
select case when instr_type in ('Deposit', 'Savings') then acct_name || ' ' || short_no else instr_name end as name,
       case cf_type when 'Проценты' then 'проценты' when 'Дивиденд' then 'дивиденды' else 'купоны' end as kind,
       round(sum(rub) / 1e6, 1) as amount
from fcx where cf_type <> 'Погашение' group by all order by amount desc limit 5"#,
        // deposits closing and bonds redeemed in the next 12 months (bonds as one line)
        // the next twelve months' payments by kind: interest, dividends, coupons, capital coming back
        "fcsum" => r#"
, kinds as (select * from (values ('Проценты по вкладам', 'Проценты', 1), ('Дивиденды', 'Дивиденд', 2), ('Купоны облигаций', 'Купон', 3),
                                  ('Возврат капитала', 'Погашение', 4)) t(kind, cf, o))
select k.kind, coalesce(sum(f.rub), 0) as amount, count(distinct f.pay_dt) as n, strftime(min(f.pay_dt), '%d.%m.%Y') as next
from kinds k left join fc f on f.cf_type = k.cf group by k.kind, k.o order by k.o"#,
        "principal" => r#"
, k as (select case when instr_type in ('Deposit', 'Savings') then acct_name || ' ' || short_no else 'Облигации · погашения' end as name,
               min(pay_dt) as d, sum(rub) as rub, count(distinct pay_dt) as n
        from fcx where cf_type = 'Погашение' group by 1),
t as (select (select string_agg(mn, ' + ' order by mo) from (select distinct date_trunc('month', pay_dt) as mo, (select mshort from mnames)[month(pay_dt)] as mn
                                                            from fcx where cf_type = 'Погашение')) as months,
             sum(n) as n from k)
select name, strftime(d, '%d.%m.%Y') as date, round(rub / 1e6, 1) as amount, strftime(d, '%d.%m') as day,
       (select months || ' · ' || n || case when n % 10 = 1 and n % 100 <> 11 then ' поступление'
                                            when n % 10 between 2 and 4 and n % 100 not between 12 and 14 then ' поступления' else ' поступлений' end from t) as note
from k order by d"#,
        // trades and operations in the period, millions of roubles
        "flows" => r#"
select kind, part, amount, n from (
  select 'trades' as kind, 'Покупки' as part, sum(abs(amount_rub)) / 1e6 as amount, count(*) as n, 1 as o from ops where op_type = 'Покупка'
  union all select 'trades', 'Продажи', sum(abs(amount_rub)) / 1e6, count(*), 2 from ops where op_type = 'Продажа'
  union all select 'fx', 'Покупки', coalesce(sum(abs(amount_rub)), 0) / 1e6, count(*), 3 from ops where op_type = 'Конвертация' and qty > 0
  union all select 'fx', 'Продажи', coalesce(sum(abs(amount_rub)), 0) / 1e6, count(*), 4 from ops where op_type = 'Конвертация' and qty < 0
  union all select 'cash', 'Ввод ДС', coalesce(sum(amount_rub), 0) / 1e6, null, 5 from ops where op_type = 'Пополнение'
  union all select 'cash', 'Вывод ДС', coalesce(-sum(amount_rub), 0) / 1e6, null, 6 from ops where op_type = 'Вывод'
  union all select 'dep', 'Открыто', coalesce(sum(amount_rub), 0) / 1e6, null, 7 from ops
            where op_type = 'Пополнение' and acct_cd in (select acct_cd from acc where acct_type = 'Депозит' and open_dt = ops.op_dt)
  union all select 'dep', 'Закрыто', coalesce(sum(amount_rub), 0) / 1e6, null, 8 from ops where op_type = 'Закрытие'
  union all select 'tax', 'НДФЛ', coalesce(-sum(amount_rub), 0) / 1e6, null, 9 from ops where op_type = 'Налог'
  union all select 'tax', 'Комиссии', (coalesce(-sum(amount_rub) filter (op_type = 'Комиссия'), 0) - coalesce(sum(fee), 0)) / 1e6, null, 10 from ops)
order by o"#,
        "bigtrades" => r#"
select i.instr_name as name, case o.src_cd when 'AM' then 'ДУ ' || right(o.acct_numb, 5) else o.acct_numb end as acct,
       case i.instr_type when 'Equity' then 'Акция' when 'Funds' then 'Фонд' when 'Bonds' then 'Облигация' else i.instr_type_ru end as type,
       o.op_type as dir, replace(format('{:,}', cast(o.qty as bigint)), ',', ' ') as qty, round(abs(o.amount_rub) / 1e6, 2) as sum
from ops o join instruments i using (instr_cd) where o.op_type in ('Покупка', 'Продажа')
order by abs(o.amount_rub) desc limit 5"#,
        "conv" => r#"
select row_number() over (order by abs(amount_rub) desc) as n,
       cur || ' / RUB' as currency,
       round(amount_rub / qty, 2) as rate, case when qty > 0 then 'Покупка' else 'Продажа' end as dir,
       replace(format('{:,}', cast(abs(qty) as bigint)), ',', ' ') || ' ' || case cur when 'USD' then '$' when 'CNY' then '¥' when 'EUR' then '€' else cur end as qty,
       round(abs(amount_rub) / 1e6, 2) as sum
from ops where op_type = 'Конвертация' order by abs(amount_rub) desc limit 5"#,
        "ops" => r#"
select kind, name, acct, dir, sum from (
  -- each operation its own row: its date tells it from the others
  select 'cash' as kind, case when amount_rub > 0 then 'Ввод средств' else 'Вывод средств' end || ' · ' || strftime(op_dt, '%d.%m') as name,
         case src_cd when 'BROK' then acct_numb when 'AM' then 'ДУ ' || right(acct_numb, 5) else 'Банк' end as acct,
         case when amount_rub > 0 then 'Ввод' else 'Вывод' end as dir, round(amount_rub / 1e6, 2) as sum, abs(amount_rub) as size, 1 as o
  from ops where op_type in ('Пополнение', 'Вывод') and src_cd <> 'BANK'
  union all select 'dep', acct_name || ' ' || short_no, 'Банк', 'Открытие', round(amount_rub / 1e6, 2), abs(amount_rub), 2
  from ops where op_type = 'Пополнение' and acct_cd in (select acct_cd from acc where acct_type = 'Депозит' and open_dt = ops.op_dt)
  union all select 'dep', acct_name || ' ' || short_no, 'Банк', 'Закрытие', round(amount_rub / 1e6, 2), abs(amount_rub), 2 from ops where op_type = 'Закрытие'
  union all select 'tax', 'НДФЛ · дивиденды, купоны и проценты', case src_cd when 'BROK' then acct_numb when 'AM' then 'ДУ ' || right(acct_numb, 5) else 'Банк' end,
                   'НДФЛ', round(sum(amount_rub) / 1e6, 2), abs(sum(amount_rub)), 3 from ops where op_type = 'Налог' group by all)
order by size desc"#,
        // each payment into and out of the accounts (the largest, by date)
        "cashops" => r#"
, c as (select op_dt, acct_label as acct, case when op_type = 'Пополнение' then 'Ввод' else 'Вывод' end as dir, amount_rub as sum
        from ops where op_type in ('Пополнение', 'Вывод') order by abs(amount_rub) desc limit 14)
select row_number() over (order by op_dt, acct) as n, strftime(op_dt, '%d.%m.%Y') as date, acct, dir, round(sum / 1e6, 2) as sum,
       (select count(*) from ops where op_type in ('Пополнение', 'Вывод')) as total_n
from c order by op_dt, acct"#,
        // taxes and fees paid, by account and kind (roubles, positive = paid)
        "charges" => r#"
select kind, name, acct, n, round(paid, 2) as paid from (
  select 'tax' as kind, 'НДФЛ' as name, case src_cd when 'BANK' then 'Банковские счета' else acct_label end as acct, count(*) as n, -sum(amount_rub) as paid from ops where op_type = 'Налог' group by all
  union all
  select 'fee', case src_cd when 'INS' then 'По полису' when 'AM' then 'Управление' else 'Обслуживание' end,
         case src_cd when 'BANK' then 'Банковские счета' else acct_label end, count(*), -sum(amount_rub) from ops where op_type = 'Комиссия' group by all
  union all
  select 'fee', 'По сделкам', case src_cd when 'BANK' then 'Банковские счета' else acct_label end, count(*), -sum(fee) from ops where fee is not null and fee <> 0 group by all)
where paid <> 0 order by kind desc, paid desc"#,
        "deals" => r#"
-- a unit's value in roubles: bonds are priced as a share of their nominal
, unit as (select instr_cd, case when instr_type = 'Bonds' then coalesce(instr_nominal, 1000) else 1 end as nom from instruments),
-- the last price on or before the period's end, and the rates of that day
pend as (select instr_cd, arg_max(price, report_dt) as price, arg_max(price_cur, report_dt) as cur from prices where report_dt <= (select e from per) group by 1),
fxe as (select cur, arg_max(rate, report_dt) as rate from fx_rates where report_dt <= (select e from per) group by 1),
d as (
  -- the account a deal was made on (a client may have several): its type, then its number
  select i.instr_name as name, case i.instr_type when 'Equity' then 'Акции' when 'Bonds' then 'Облигации' when 'Funds' then 'Фонды' when 'Metals' then 'Металлы' else i.instr_type_ru end as kind,
         o.acct_label as acct, o.op_type as type, strftime(o.op_dt, '%d.%m') as date,
         replace(format('{:,}', cast(o.qty as bigint)), ',', ' ') as qty,
         round(abs(o.amount) / nullif(o.qty, 0) / case when i.instr_type = 'Bonds' then i.instr_nominal / 100.0 else 1 end, 2) as price,
         o.amount_rub as sum, o.fee, o.op_dt,
         -- the deal at the period's end: a purchase against the price then, a sale against the average cost before it
         case when o.op_type = 'Покупка'
              then pe.price * u.nom * coalesce(fe.rate, 1) / nullif(abs(o.amount_rub) / o.qty, 0) - 1
              else abs(o.amount_rub) / o.qty / nullif(bc.avg_price * u.nom * coalesce(bc.fx_rate, 1), 0) - 1 end as res_pct,
         case when o.op_type = 'Покупка'
              then pe.price * u.nom * coalesce(fe.rate, 1) * o.qty - abs(o.amount_rub)
              else abs(o.amount_rub) - bc.avg_price * u.nom * coalesce(bc.fx_rate, 1) * o.qty end as res
  from ops o join instruments i using (instr_cd) join unit u using (instr_cd)
  left join pend pe using (instr_cd) left join fxe fe on fe.cur = pe.cur
  left join balances bc on bc.acct_cd = o.acct_cd and bc.instr_cd = o.instr_cd and bc.report_dt = o.op_dt - 1
  where o.op_type in ('Покупка', 'Продажа')
  union all
  -- a conversion as a currency pair: buying it is buying the currency (selling roubles); at the period's end
  -- against that day's rate (a purchase gains when the currency rose, a sale when it fell)
  select o.cur || ' / RUB', 'Валюта', o.acct_label,
         case when o.qty > 0 then 'Покупка' else 'Продажа' end, strftime(o.op_dt, '%d.%m'), replace(format('{:,}', cast(abs(o.qty) as bigint)), ',', ' ') || ' ' || o.cur,
         round(o.amount_rub / o.qty, 2), -o.amount_rub, null, o.op_dt,
         case when o.qty > 0 then fe.rate / (o.amount_rub / o.qty) - 1 else (o.amount_rub / o.qty) / fe.rate - 1 end,
         (fe.rate - o.amount_rub / o.qty) * o.qty
  from ops o left join fxe fe on fe.cur = o.cur where o.op_type = 'Конвертация'),
-- the largest deal of each instrument, the eleven largest of those, by date
-- (one row per instrument or currency pair: its largest deal, either way)
one as (select * from d qualify row_number() over (partition by name order by abs(sum) desc) = 1),
top as (select * from one order by abs(sum) desc limit 11)
select name, kind, acct, type, date, qty, price, sum, fee, round(res_pct * 100, 1) as res_pct, res from top order by op_dt"#,
        "accounts" => ACCOUNTS,
        // an insurance policy: its terms, value and surrender value
        "policy" => r#"
, pol as (select a.*, b.v1, (select sum(redeem_amt) from balances r where r.acct_cd = a.acct_cd and r.report_dt = (select e from per)) as redeem,
                 (select sum(amount_rub) from ops o where o.acct_cd = a.acct_cd and o.op_type = 'Пополнение') as premiums,
                 (select -sum(amount_rub) from ops o where o.acct_cd = a.acct_cd and o.op_type = 'Комиссия') as fees,
                 (select sum(inc) from gday g where g.grp = a.acct_cd and g.report_dt between (select s from per) and (select e from per)) as guaranteed
          from acc a left join (select acct_cd, sum(v1) as v1 from bal_e group by 1) b using (acct_cd) where a.src_cd = 'INS')
select acct_cd as acct, key, value, num from (
  select acct_cd, 'Полис' as key, acct_name as value, null as num, 1 as o from pol
  union all select acct_cd, 'Начало', strftime(open_dt, '%d.%m.%Y'), null, 3 from pol
  union all select acct_cd, 'Окончание', strftime(policy_end_dt, '%d.%m.%Y'), null, 4 from pol
  union all select acct_cd, 'До окончания', replace(printf('%.1f', date_diff('day', (select e from per), policy_end_dt) / 365.25), '.', ',') || ' лет', null, 5 from pol
  union all select acct_cd, 'Стоимость полиса', null, v1, 7 from pol
  union all select acct_cd, 'Выкупная сумма', null, redeem, 8 from pol
  union all select acct_cd, 'Выкупная сумма, % стоимости', replace(printf('%.1f', 100 * redeem / nullif(v1, 0)), '.', ',') || '%', null, 9 from pol
  union all select acct_cd, 'Взносы за период', null, coalesce(premiums, 0), 10 from pol
  union all select acct_cd, 'Начисленный гарантированный доход', null, coalesce(guaranteed, 0), 11 from pol
  union all select acct_cd, 'Комиссия за период', null, coalesce(fees, 0), 12 from pol)
order by acct, o"#,
        // a policy's value and surrender value at each month's end
        "polhist" => r#"
select b.acct_cd as acct, least(last_day(b.report_dt), (select e from per)) as date, sum(b.amt_rub) as value, sum(b.redeem_amt) as redeem
from balances b join acc a using (acct_cd)
where a.src_cd = 'INS' and b.report_dt between (select s from per) and (select e from per)
  and b.report_dt = least(last_day(b.report_dt), (select e from per))
group by all order by 1, 2"#,
        // the "Продукты" filter's choices: the client's products in the period (by name);
        // read from the accounts directly, so picking some doesn't take the others away
        "products" => r#"
select src_cd,
       case src_cd when 'BANK' then 'Банковские счета' when 'BROK' then 'Брокерские счета'
                   when 'AM' then 'Доверительное управление' when 'INS' then 'Страхование' else src_cd end as product
from (select distinct a.src_cd from accounts a
      where a.client_id = (select id from cl) and a.close_dt >= (select s from per)
        and (:access_src = '' or list_contains(string_split(:access_src, ','), a.src_cd)))
order by case src_cd when 'BANK' then 1 when 'BROK' then 2 when 'AM' then 3 else 4 end"#,
        _ => return None,
    };
    Some(format!("{BASE}{body}"))
}
