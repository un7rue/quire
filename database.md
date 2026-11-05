Table: clients
Description: static table of all the clients
Fields:
    id - client id - type:text - unique - primary key - example: C0001
    name - client name - type:text - example: LeBron James (please generate clients using top NBA players)
    birthday - client's birthday - type:date - example: 1984-09-20
    nationality - client's nationality - text - example: Australia
    sales_id - id of sales people covering the client - type:text - example S0001. relation client.sales_id -> sales.id

Table: sales
Description: static table of all sales people
Fields:
    id - sales_id - type:text - unique - primaryt key - example: S0001
    name - sales name - type:text - example: Warren Buffet (please generate sales names using top Investment Professionals)
    sales_phone - sales phone numer - type:text random phone number in format 8-903-1234567
    sales_email - random email - type:text warren.buffet@pb.com

Table: accounts
Description: static table the has all the accounts related to the clients
fields:
    acct_cd - account code - type:text - unique - primary key - example: A0001
    acct_numb - accont number - examples ABCD23 fro brokerage acc, 34252523425250025 for bank acct,  DU.22592 for AM acc
    acct_name - name of the account - Вклад "Премиум",  Брокерский счет, Консервативаня Стратегия
    acct_type - type of the account - Брокерский Счет, Депозит, Стратегия ДУ и тд
    client_id - client_id account code relates to - type:text - relation acconuts.clinet_id -> client.id
    src_cd - source of the account - type:text - one of: BANK, BROK, AM, INS
    open_dt - date of the account open - type:date - example: 2025-04-25
    close_dt date of account closure - type:date - example: 2026-03-25 (if account is not closed the value should be 3000-01-01) 

Table: instruments
Description: static table of all the instruments available on the client's accounts
fields:
    instr_cd - instrument code - type:text - unique = primary key - example: I0001
    isin - ISIN of the instrument - example: RU000A043453 US000A023425
    issuer_cd - code of the instrument - APPL, SBER, RU (for russia as an issuer), US etc
    instr_name - instrument name - for equity issuer name: Apple, Сбербанк, for bonds issuer_cd+maturity_coupon : SBER-20260430-7.5, US-20380331-2.4
    instr_type -  type of instrument, Equity, Bonds, Funds, Currency, Metals etc 
    instr_type_ru - type of the instrument in russian - type:text one of : Акции, Облигации, Фонды, Денежные Средства, ОМС etc
    instr_nominal - nominal for bonds - int - 1000
    maturity_dt - maturity date for bonds - date
    coupon - coupon rate for bonds - 7.5%

Table: balances
Description: fact daily table of clients instruments for each of the account.
fields:
    report_dt - date of the fact value - type:date
    client_id - balance of the instrument related to client_id - type:text - relation balances.clinet_id -> client.id
    acct_cd - balance of the instrument related to acct_cd - type:text - relation balances.acct_cd -> accounts.acct_cd
    instr_cd - instrument code - type:text - example I0001
    qty - instrument quanity - int example: 20000
    price - instrument price for the date - for equity: 123, for bonds: 0.9745 (price for bonds in percent)
    price_cur - currency of the price type text - ex: USD, RUR etc
    amt - market value of the isntrument for the date - type:bigint? example 3423258023 . Should be calculated for equity qty * price, for bonds qty * price * nominal from instruments table
    fx_rate - fx rate for converstion into RUR - example: 83.4424
    amt_rub - market value in RUR - calculated as amt * fx_rate

Table: portf_acct
Description: fact daily table for account return calculation. Very important!
fields:
    report_dt - date of the return for a particular account - type:date
    client_id - type:text - relation balances.clinet_id -> client.id
    acct_cd -  type:text - relation balances.acct_cd -> accounts.acct_cd
    cur - currency of the fact values - for now use only RUR
    bal_in - morning RUR balance of the account (taken from the bal_out of the previous date)
    net_flow - sum of inflows (are positive values) and outflows (negative values) in RUR during the date. if otuflows are larget than infwlos net_flow is negative
    bal_out - evening balance of the account - reflects the market movement and respects the net_flows - should be equal of sum amt_rub from balances for particular account
    net_twr - net return (after taxes and fees) for the day. calculated as net_flow >= 0 then bal_out / (bal_in+net_flow), net_flow < 0 then (bal_out + net_flow) / bal_in should be in range: 0.9-1.1 unless some market volatility or huge taxes or fees paid 
    income - daily interest income for bonds (accrude interest) or deposit (daily percent income) - vital for correct return calculations
    fees - fees paid by the client during the day (not included in outflows) stored as negative amount
    taxes - taxes paid by the client during the day (noet icluded in the outflows) stored as negative amount
    gross_twr -  before taxes and fees return. calculated as net_flow >= 0 then (bal_out - taxes - fees) / (bal_in+net_flow), net_flow < 0 then (bal_out + net_flow - taxes - fees) / bal_in. Value should look like 1.00345, or 0.999423. should be in range: 0.9-1.1 unless some market volatility  
    net_twr_ltd - cumulative product of each day net_twr
    gross_twr_ltd - cumulative product of each day gross_twr