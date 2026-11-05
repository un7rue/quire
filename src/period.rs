//! Reporting periods: turning parameters into date ranges, comparison ranges
//! and the trailing windows charts use.

use crate::model::Params;
use chrono::{Datelike, Duration, NaiveDate};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Range {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl Range {
    pub fn contains(&self, d: NaiveDate) -> bool {
        d >= self.start && d <= self.end
    }

    pub fn union(&self, o: &Range) -> Range {
        Range { start: self.start.min(o.start), end: self.end.max(o.end) }
    }
}

/// Month names and phrasing for one language.
struct Lang {
    short: [&'static str; 12],
    long: [&'static str; 12],
    quarter: fn(u32, i32) -> String,
    ytd: &'static str,
    /// Dates as numbers (15.06.2026) rather than "15 Jun 2026".
    numeric_dates: bool,
}

const EN: Lang = Lang {
    short: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
    long: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
    quarter: |q, y| format!("Q{q} {y}"),
    ytd: "YTD",
    numeric_dates: false,
};

const RU: Lang = Lang {
    short: ["Янв", "Фев", "Мар", "Апр", "Май", "Июн", "Июл", "Авг", "Сен", "Окт", "Ноя", "Дек"],
    long: ["Январь", "Февраль", "Март", "Апрель", "Май", "Июнь", "Июль", "Август", "Сентябрь", "Октябрь", "Ноябрь", "Декабрь"],
    quarter: |q, y| format!("{} кв. {y}", ["I", "II", "III", "IV"][(q as usize).clamp(1, 4) - 1]),
    ytd: "С начала года",
    numeric_dates: true,
};

const DE: Lang = Lang {
    short: ["Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez"],
    long: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
    quarter: |q, y| format!("Q{q} {y}"),
    ytd: "YTD",
    numeric_dates: true,
};

thread_local! {
    static LOCALE: std::cell::RefCell<String> = std::cell::RefCell::new("en-US".into());
    static FORMATS: std::cell::RefCell<crate::model::Formats> = std::cell::RefCell::new(crate::model::Formats::default());
}

/// The formats in force (the document's over the app settings), for text the engine writes.
pub fn set_formats(f: &crate::model::Formats) {
    FORMATS.with(|x| *x.borrow_mut() = f.clone());
}

pub fn formats() -> crate::model::Formats {
    FORMATS.with(|x| x.borrow().clone())
}

/// Sets the language for labels produced on this thread (the report's locale).
pub fn set_locale(locale: &str) {
    LOCALE.with(|l| *l.borrow_mut() = locale.to_string());
}

/// A few words the engine writes into results, in the report's language.
pub fn tr(word: &str) -> String {
    let ru = LOCALE.with(|l| l.borrow().starts_with("ru"));
    let de = LOCALE.with(|l| l.borrow().starts_with("de"));
    match (word, ru, de) {
        ("Total", true, _) => "Итого".into(),
        ("Total", _, true) => "Gesamt".into(),
        ("Other", true, _) => "Прочие".into(),
        ("Other", _, true) => "Sonstige".into(),
        (w, _, _) => w.into(),
    }
}

pub fn locale() -> String {
    LOCALE.with(|l| l.borrow().clone())
}

fn lang() -> &'static Lang {
    LOCALE.with(|l| {
        let l = l.borrow();
        if l.starts_with("ru") {
            &RU
        } else if l.starts_with("de") {
            &DE
        } else {
            &EN
        }
    })
}

/// A date the way the report's language writes it.
pub fn fmt_date(d: NaiveDate) -> String {
    let lg = lang();
    let f = formats().date;
    match f.as_str() {
        "dd.mm.yyyy" => return d.format("%d.%m.%Y").to_string(),
        "dd/mm/yyyy" => return d.format("%d/%m/%Y").to_string(),
        "mm/dd/yyyy" => return d.format("%m/%d/%Y").to_string(),
        "yyyy-mm-dd" => return d.format("%Y-%m-%d").to_string(),
        "d mmm yyyy" => return format!("{} {} {}", d.day(), lg.short[d.month0() as usize], d.year()),
        "mmm d, yyyy" => return format!("{} {}, {}", lg.short[d.month0() as usize], d.day(), d.year()),
        _ => {}
    }
    if lg.numeric_dates {
        d.format("%d.%m.%Y").to_string()
    } else {
        format!("{} {} {}", d.day(), lg.short[d.month0() as usize], d.year())
    }
}

pub fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap_or(NaiveDate::MIN)
}

pub fn month_start(d: NaiveDate) -> NaiveDate {
    ymd(d.year(), d.month(), 1)
}

pub fn month_end(d: NaiveDate) -> NaiveDate {
    add_months(month_start(d), 1) - Duration::days(1)
}

/// Shifts by whole months, clamping the day to the target month's length.
pub fn add_months(d: NaiveDate, n: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + n;
    let (y, m) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
    let mut day = d.day();
    loop {
        if let Some(r) = NaiveDate::from_ymd_opt(y, m, day) {
            return r;
        }
        day -= 1;
    }
}

fn quarter_start(d: NaiveDate) -> NaiveDate {
    ymd(d.year(), (d.month0() / 3) * 3 + 1, 1)
}

/// The reporting period the parameters select.
pub fn period(p: &Params) -> Range {
    let d = p.date;
    match p.grain.as_str() {
        "quarter" => {
            let s = quarter_start(d);
            Range { start: s, end: add_months(s, 3) - Duration::days(1) }
        }
        "year" => Range { start: ymd(d.year(), 1, 1), end: ymd(d.year(), 12, 31) },
        "ytd" => Range { start: ymd(d.year(), 1, 1), end: month_end(d) },
        "custom" => {
            let s = p.start.unwrap_or(month_start(d));
            Range { start: s.min(d), end: s.max(d) }
        }
        _ => Range { start: month_start(d), end: month_end(d) },
    }
}

/// The range the current period is compared against.
pub fn compare(p: &Params) -> Option<Range> {
    let r = period(p);
    match p.compare.as_str() {
        "none" => None,
        "year" => Some(Range { start: add_months(r.start, -12), end: shift_end(r.end, -12) }),
        _ => Some(match p.grain.as_str() {
            "quarter" => Range { start: add_months(r.start, -3), end: shift_end(r.end, -3) },
            "year" | "ytd" => Range { start: add_months(r.start, -12), end: shift_end(r.end, -12) },
            "custom" => {
                let len = (r.end - r.start).num_days() + 1;
                Range { start: r.start - Duration::days(len), end: r.start - Duration::days(1) }
            }
            _ => Range { start: add_months(r.start, -1), end: month_end(add_months(r.start, -1)) },
        }),
    }
}

/// Keeps month-ends at month-ends when shifting (30 Jun − 1 month → 31 May).
fn shift_end(end: NaiveDate, months: i32) -> NaiveDate {
    if end == month_end(end) {
        month_end(add_months(month_start(end), months))
    } else {
        add_months(end, months)
    }
}

/// The date range a widget window covers, `None` meaning all data.
pub fn window(p: &Params, window: &str) -> Option<Range> {
    let r = period(p);
    match window {
        "all" => None,
        "ytd" => Some(Range { start: ymd(r.end.year(), 1, 1), end: r.end }),
        w if w.starts_with("trailing") => {
            let n: i32 = w["trailing".len()..].parse().unwrap_or(12).max(1);
            Some(Range { start: add_months(month_start(r.end), 1 - n), end: r.end })
        }
        _ => Some(r),
    }
}

/// The same window, moved back to the comparison period.
pub fn window_compare(p: &Params, window: &str) -> Option<Range> {
    let cur = period(p);
    let cmp = compare(p)?;
    let w = self::window(p, window)?;
    if w == cur {
        return Some(cmp);
    }
    let months = (cur.end.year() - cmp.end.year()) * 12 + cur.end.month() as i32 - cmp.end.month() as i32;
    if months <= 0 {
        let days = (cur.end - cmp.end).num_days();
        return Some(Range { start: w.start - Duration::days(days), end: w.end - Duration::days(days) });
    }
    Some(Range { start: add_months(w.start, -months), end: shift_end(w.end, -months) })
}

pub fn label(p: &Params) -> String {
    label_range(&period(p), &p.grain)
}

pub fn compare_label(p: &Params) -> Option<String> {
    let c = compare(p)?;
    let grain = if p.compare == "year" && p.grain == "month" { "month" } else { p.grain.as_str() };
    Some(label_range(&c, grain))
}

fn label_range(r: &Range, grain: &str) -> String {
    let (s, e) = (r.start, r.end);
    let lg = lang();
    match grain {
        "month" if s == month_start(s) && e == month_end(s) => format!("{} {}", lg.long[s.month0() as usize], s.year()),
        "quarter" if s.day() == 1 && s.month0() % 3 == 0 => (lg.quarter)(s.month0() / 3 + 1, s.year()),
        "year" if s.ordinal() == 1 && e.month() == 12 && e.day() == 31 => s.year().to_string(),
        "ytd" => format!("{} {} {}", lg.ytd, lg.short[e.month0() as usize], e.year()),
        _ if lg.numeric_dates => format!("{} – {}", fmt_date(s), fmt_date(e)),
        _ => {
            if s.year() == e.year() {
                format!("{} {} – {} {} {}", s.day(), lg.short[s.month0() as usize], e.day(), lg.short[e.month0() as usize], e.year())
            } else {
                format!("{} – {}", fmt_date(s), fmt_date(e))
            }
        }
    }
}

/// Truncates a date to a grain, returning a sortable key and a display label.
pub fn bucket(d: NaiveDate, grain: &str) -> (String, String) {
    let lg = lang();
    match grain {
        "year" => (format!("{:04}", d.year()), d.year().to_string()),
        "quarter" => {
            let q = d.month0() / 3 + 1;
            (format!("{:04}-Q{q}", d.year()), (lg.quarter)(q, d.year()))
        }
        "week" => {
            let w = d.iso_week();
            (format!("{:04}-W{:02}", w.year(), w.week()), format!("W{} {}", w.week(), w.year()))
        }
        "day" => (d.format("%Y-%m-%d").to_string(), format!("{} {}", d.day(), lg.short[d.month0() as usize])),
        _ => (
            format!("{:04}-{:02}", d.year(), d.month()),
            format!("{} {}", lg.short[d.month0() as usize], d.format("%y")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(grain: &str, date: NaiveDate, compare: &str) -> Params {
        Params { grain: grain.into(), date, compare: compare.into(), ..Default::default() }
    }

    #[test]
    fn month_and_previous() {
        let p = params("month", ymd(2026, 3, 15), "previous");
        assert_eq!(period(&p), Range { start: ymd(2026, 3, 1), end: ymd(2026, 3, 31) });
        assert_eq!(compare(&p), Some(Range { start: ymd(2026, 2, 1), end: ymd(2026, 2, 28) }));
        assert_eq!(label(&p), "March 2026");
    }

    #[test]
    fn quarter_year_ago() {
        let p = params("quarter", ymd(2026, 8, 2), "year");
        assert_eq!(period(&p), Range { start: ymd(2026, 7, 1), end: ymd(2026, 9, 30) });
        assert_eq!(compare(&p), Some(Range { start: ymd(2025, 7, 1), end: ymd(2025, 9, 30) }));
        assert_eq!(label(&p), "Q3 2026");
    }

    #[test]
    fn trailing_window() {
        let p = params("month", ymd(2026, 3, 10), "previous");
        assert_eq!(window(&p, "trailing12"), Some(Range { start: ymd(2025, 4, 1), end: ymd(2026, 3, 31) }));
        assert_eq!(window(&p, "ytd"), Some(Range { start: ymd(2026, 1, 1), end: ymd(2026, 3, 31) }));
        assert_eq!(
            window_compare(&p, "ytd"),
            Some(Range { start: ymd(2025, 12, 1), end: ymd(2026, 2, 28) })
        );
    }

    #[test]
    fn russian_labels() {
        set_locale("ru-RU");
        let p = params("month", ymd(2026, 6, 15), "previous");
        assert_eq!(label(&p), "Июнь 2026");
        assert_eq!(bucket(ymd(2026, 1, 3), "month").1, "Янв 26");
        assert_eq!(bucket(ymd(2026, 8, 3), "quarter").1, "III кв. 2026");
        let c = Params { grain: "custom".into(), start: Some(ymd(2026, 1, 1)), date: ymd(2026, 6, 15), ..Default::default() };
        assert_eq!(label(&c), "01.01.2026 – 15.06.2026");
        set_locale("en-US");
    }

    #[test]
    fn ytd_compares_to_last_year() {
        let p = params("ytd", ymd(2026, 5, 20), "previous");
        assert_eq!(compare(&p), Some(Range { start: ymd(2025, 1, 1), end: ymd(2025, 5, 31) }));
        assert_eq!(label(&p), "YTD May 2026");
    }
}
