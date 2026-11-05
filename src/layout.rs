//! The page grid and its fit engine.
//!
//! Widgets occupy whole grid cells. After every change the engine resolves
//! collisions (pushing widgets down), and in auto-arrange mode compacts the
//! page, shrinks flexible widgets until everything fits and stretches widgets
//! into leftover space. Whatever it can't resolve on its own comes back as
//! issues with concrete, one-click suggestions.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn bottom(&self) -> u32 {
        self.y + self.h
    }
    pub fn right(&self) -> u32 {
        self.x + self.w
    }
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }
    fn cols_overlap(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right()
    }
}

/// Size rules per widget kind.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Spec {
    pub min_w: u32,
    pub min_h: u32,
    pub max_h: u32,
    pub def_w: u32,
    pub def_h: u32,
    /// Can absorb spare height (charts, tables) when filling a page.
    pub grow: bool,
}

pub fn spec(kind: &str) -> Spec {
    let s = |min_w, min_h, max_h, def_w, def_h, grow| Spec { min_w, min_h, max_h, def_w, def_h, grow };
    match kind {
        "kpi" => s(2, 2, 12, 3, 2, false),
        "bar" | "line" | "area" => s(3, 3, 99, 6, 4, true),
        "pie" => s(3, 3, 99, 4, 4, true),
        "table" => s(3, 2, 99, 6, 5, true),
        "heading" => s(2, 1, 99, 12, 1, false),
        "comment" => s(2, 1, 99, 4, 2, false),
        "text" => s(2, 1, 99, 4, 2, false),
        "divider" => s(1, 1, 99, 12, 1, false),
        "image" => s(1, 1, 99, 2, 1, false),
        "treemap" => s(3, 3, 99, 6, 4, true),
        "waterfall" => s(3, 3, 99, 6, 4, true),
        "tiles" => s(3, 2, 99, 12, 3, true),
        "scale" => s(3, 1, 99, 6, 2, true),
        "toc" => s(3, 3, 99, 6, 8, false),
        // several parts in one card: sized by hand, its parts laid out inside
        "composite" => s(3, 2, 99, 6, 4, false),
        _ => s(1, 1, 99, 4, 2, false),
    }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub kind: String,
    pub rect: Rect,
    pub locked: bool,
    /// Rows the content needs (wrapped text, table rows), if known.
    pub need_h: Option<u32>,
    /// Rows at which every table row is visible.
    pub table_rows: Option<(u32, u32)>,
}

impl Item {
    fn spec(&self) -> Spec {
        spec(&self.kind)
    }
    /// The lowest height the engine may shrink this item to.
    fn floor_h(&self) -> u32 {
        let s = self.spec();
        let content = if matches!(self.kind.as_str(), "text" | "comment" | "heading") { self.need_h.unwrap_or(0) } else { 0 };
        s.min_h.max(content)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Grid {
    pub cols: u32,
    pub rows: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// Compact, shrink and fill automatically after each change.
    pub auto: bool,
    /// Stretch widgets into leftover space.
    pub fill: bool,
}

// ------------------------------------------------------------------ primitives

fn collides(items: &[Item], i: usize, r: &Rect) -> bool {
    items.iter().enumerate().any(|(j, o)| j != i && o.rect.overlaps(r))
}

/// Pushes widgets down until nothing overlaps. The anchor (the widget the
/// user just placed) keeps its spot unless it lands on a locked widget.
pub fn resolve(items: &mut [Item], anchor: Option<&str>) {
    let n = items.len();
    for _ in 0..(n * n * 4 + 16) {
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&i| {
            let it = &items[i];
            let rank = if Some(it.id.as_str()) == anchor {
                0
            } else if it.locked {
                1
            } else {
                2
            };
            (rank, it.rect.y, it.rect.x)
        });
        let mut moved = false;
        'scan: for (k, &a) in order.iter().enumerate() {
            for &b in &order[k + 1..] {
                if items[a].rect.overlaps(&items[b].rect) {
                    if items[b].locked && !items[a].locked {
                        items[a].rect.y = items[b].rect.bottom();
                    } else {
                        items[b].rect.y = items[a].rect.bottom();
                    }
                    moved = true;
                    break 'scan;
                }
            }
        }
        if !moved {
            break;
        }
    }
}

/// A widget pushed past the page end by a collision moves into the nearest
/// free spot on the page instead (keeping its size), so dragging one widget
/// onto another doesn't knock a third off the page when there's room.
pub fn settle(items: &mut [Item], grid: Grid, anchor: Option<&str>, before: &[Item]) {
    let mut order: Vec<usize> = (0..items.len())
        .filter(|&i| items[i].rect.bottom() > grid.rows && !items[i].locked && Some(items[i].id.as_str()) != anchor)
        .collect();
    let home = |it: &Item| before.iter().find(|b| b.id == it.id).map(|b| b.rect).unwrap_or(it.rect);
    order.sort_by_key(|&i| home(&items[i]).y);
    for i in order {
        let r = items[i].rect;
        if r.h > grid.rows {
            continue;
        }
        let want = home(&items[i]);
        let mut best: Option<(u32, Rect)> = None;
        for y in 0..=grid.rows - r.h {
            for x in 0..=grid.cols.saturating_sub(r.w) {
                let c = Rect { x, y, ..r };
                if collides(items, i, &c) {
                    continue;
                }
                // nearest to where it was: rows count more than columns
                let d = want.y.abs_diff(y) * 4 + want.x.abs_diff(x);
                if best.map_or(true, |(bd, _)| d < bd) {
                    best = Some((d, c));
                }
            }
        }
        if let Some((_, c)) = best {
            items[i].rect = c;
        }
    }
}

/// Floats every unlocked widget up as far as it goes.
/// Moves every widget up as far as it goes, except locked ones and `keep` (the one just placed).
pub fn compact(items: &mut [Item], keep: Option<&str>) {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| (items[i].rect.y, items[i].rect.x));
    for i in order {
        if items[i].locked || Some(items[i].id.as_str()) == keep {
            continue;
        }
        while items[i].rect.y > 0 {
            let r = Rect { y: items[i].rect.y - 1, ..items[i].rect };
            if collides(items, i, &r) {
                break;
            }
            items[i].rect = r;
        }
    }
}

pub fn bottom(items: &[Item]) -> u32 {
    items.iter().map(|i| i.rect.bottom()).max().unwrap_or(0)
}

fn clamp_x(items: &mut [Item], cols: u32) {
    for it in items.iter_mut() {
        it.rect.w = it.rect.w.clamp(1, cols);
        if it.rect.right() > cols {
            it.rect.x = cols - it.rect.w;
        }
        it.rect.h = it.rect.h.max(1);
    }
}

/// Shrinks flexible widgets, one row at a time, picking whichever shrink
/// lowers the page bottom most, until the page fits or nothing helps.
fn shrink_to_fit(items: &mut Vec<Item>, rows: u32, pinned: Option<&str>) -> bool {
    for _ in 0..400 {
        let cur = bottom(items);
        if cur <= rows {
            return true;
        }
        let mut best: Option<(u32, i64, Vec<Item>)> = None;
        for i in 0..items.len() {
            let it = &items[i];
            if it.locked || Some(it.id.as_str()) == pinned || it.rect.h <= it.floor_h() {
                continue;
            }
            let mut trial = items.clone();
            trial[i].rect.h -= 1;
            compact(&mut trial, pinned);
            let b = bottom(&trial);
            // prefer the biggest widgets so shrinking spreads evenly
            let score = -(it.rect.h as i64 * 100 - it.floor_h() as i64);
            if b < cur && best.as_ref().is_none_or(|(bb, bs, _)| b < *bb || (b == *bb && score < *bs)) {
                best = Some((b, score, trial));
            }
        }
        match best {
            Some((_, _, t)) => *items = t,
            None => {
                // no single shrink lowers the bottom: shrink the tallest
                // flexible widget in the overflowing stack and retry
                let over: Vec<Rect> = items.iter().filter(|i| i.rect.bottom() > rows).map(|i| i.rect).collect();
                let cand = items
                    .iter()
                    .enumerate()
                    .filter(|(_, it)| {
                        !it.locked
                            && Some(it.id.as_str()) != pinned
                            && it.rect.h > it.floor_h()
                            && over.iter().any(|o| o.cols_overlap(&it.rect))
                    })
                    .max_by_key(|(_, it)| it.rect.h - it.floor_h())
                    .map(|(i, _)| i);
                match cand {
                    Some(i) => {
                        items[i].rect.h -= 1;
                        compact(items, pinned);
                    }
                    None => return false,
                }
            }
        }
    }
    bottom(items) <= rows
}

/// Grows widgets into free space: first sideways into gaps, then down.
fn fill(items: &mut [Item], grid: Grid, pinned: Option<&str>) {
    let n = items.len();
    // sideways: close holes next to a widget, over its full height
    for _ in 0..grid.cols {
        let mut changed = false;
        for i in 0..n {
            if items[i].locked || Some(items[i].id.as_str()) == pinned || items[i].kind == "divider" {
                continue;
            }
            let r = items[i].rect;
            if r.right() < grid.cols {
                let t = Rect { w: r.w + 1, ..r };
                if !collides(items, i, &t) && gap_is_orphan(items, i, &t, grid) {
                    items[i].rect = t;
                    changed = true;
                    continue;
                }
            }
            if r.x > 0 {
                let t = Rect { x: r.x - 1, w: r.w + 1, ..r };
                if !collides(items, i, &t) && gap_is_orphan(items, i, &t, grid) {
                    items[i].rect = t;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    equalize_rows(items, pinned);
    // downward: hand spare rows to flexible widgets, bottom-most first
    for _ in 0..grid.rows {
        let mut changed = false;
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&i| std::cmp::Reverse((items[i].rect.bottom(), items[i].rect.h)));
        for i in order {
            let it = &items[i];
            if it.locked || Some(it.id.as_str()) == pinned || !it.spec().grow || it.rect.h >= it.spec().max_h {
                continue;
            }
            // a table taller than its rows is just empty space
            if it.kind == "table" && it.need_h.is_some_and(|n| it.rect.h >= n) {
                continue;
            }
            let t = Rect { h: it.rect.h + 1, ..it.rect };
            if t.bottom() <= grid.rows && !collides(items, i, &t) {
                items[i].rect = t;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

/// Side-by-side widgets of the same kind and height (a row of KPI cards)
/// share their combined width evenly.
fn equalize_rows(items: &mut [Item], pinned: Option<&str>) {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| (items[i].rect.y, items[i].rect.x));
    let mut k = 0;
    while k < order.len() {
        let first = &items[order[k]];
        let mut group = vec![order[k]];
        let mut j = k + 1;
        while j < order.len() {
            let prev = &items[*group.last().unwrap()];
            let it = &items[order[j]];
            if it.rect.y == first.rect.y && it.rect.h == first.rect.h && it.kind == first.kind && it.rect.x == prev.rect.right() {
                group.push(order[j]);
                j += 1;
            } else {
                break;
            }
        }
        k = j;
        if group.len() < 2 || group.iter().any(|&g| items[g].locked || Some(items[g].id.as_str()) == pinned) {
            continue;
        }
        let total: u32 = group.iter().map(|&g| items[g].rect.w).sum();
        let n = group.len() as u32;
        let (base, rem) = (total / n, total % n);
        if base < items[group[0]].spec().min_w {
            continue;
        }
        let mut x = items[group[0]].rect.x;
        for (i, &g) in group.iter().enumerate() {
            let w = base + u32::from((i as u32) < rem);
            items[g].rect.x = x;
            items[g].rect.w = w;
            x += w;
        }
    }
}

/// A sideways grow only claims cells nobody else could use: the strip it
/// adds must not be the top of a free area that continues below.
fn gap_is_orphan(items: &[Item], i: usize, t: &Rect, grid: Grid) -> bool {
    let r = items[i].rect;
    let strip_x = if t.x < r.x { t.x } else { r.right() };
    let below = Rect { x: strip_x, y: r.bottom(), w: 1, h: 1 };
    // free space directly below the strip means the gap is a column of free
    // cells another widget (or the downward fill) should take instead
    below.bottom() > grid.rows || items.iter().any(|o| o.rect.overlaps(&below))
}

/// Applies an edit's aftermath. Returns the arranged items. A `firm` pinned
/// widget was sized by the user and keeps that size even if the page then
/// overflows (the advisor offers to move what no longer fits).
pub fn arrange(mut items: Vec<Item>, grid: Grid, opts: &Options, pinned: Option<&str>, firm: bool) -> Vec<Item> {
    clamp_x(&mut items, grid.cols);
    if opts.auto {
        // text never hides: grow text widgets to their content
        for it in items.iter_mut() {
            if matches!(it.kind.as_str(), "text" | "comment" | "heading") {
                if let Some(need) = it.need_h {
                    it.rect.h = it.rect.h.max(need);
                }
            }
        }
    }
    let before = items.clone();
    resolve(&mut items, pinned);
    settle(&mut items, grid, pinned, &before);
    if opts.auto {
        // a widget the person has just placed stays where they put it
        compact(&mut items, if firm { pinned } else { None });
        let base = items.clone();
        if !shrink_to_fit(&mut items, grid.rows, pinned) {
            // the pinned widget may have to give way after all; if even that
            // can't fit the page, keep everyone's size and let the advisor
            // recommend a fix instead of squashing widgets for nothing
            let mut alt = base.clone();
            items = if !firm && shrink_to_fit(&mut alt, grid.rows, None) { alt } else { base };
        }
        if opts.fill && bottom(&items) <= grid.rows {
            fill(&mut items, grid, pinned);
        }
    }
    items
}

// ------------------------------------------------------------------ analysis

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    SetRect { id: String, rect: Rect },
    /// Moves a widget to another page (`None` = a new page after this one).
    MoveToPage { id: String, page: Option<String> },
    Remove { id: String },
    /// Limits a table to its first N rows (folding the rest into "Other").
    SetLimit { id: String, n: u32 },
    /// Lets a table continue onto new pages.
    Paginate { id: String },
    RemovePage { page: String },
}

#[derive(Clone, Debug, Serialize)]
pub struct Suggestion {
    pub label: String,
    pub detail: String,
    pub ops: Vec<Op>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Issue {
    /// overflow, table_rows, text_clipped, too_small, empty_space, gaps, empty_page
    pub kind: String,
    /// error, warning, info
    pub severity: String,
    pub widget: Option<String>,
    pub message: String,
    pub suggestions: Vec<Suggestion>,
}

pub struct PageCtx<'a> {
    pub page_id: &'a str,
    /// The following page, if any, with its items (to check for free room).
    pub next: Option<(&'a str, &'a [Item])>,
    pub titles: &'a dyn Fn(&str) -> String,
}

fn rect_ops(before: &[Item], after: &[Item]) -> Vec<Op> {
    after
        .iter()
        .filter(|a| before.iter().find(|b| b.id == a.id).is_none_or(|b| b.rect != a.rect))
        .map(|a| Op::SetRect { id: a.id.clone(), rect: a.rect })
        .collect()
}

/// Finds a free spot on a page for a widget of the given size.
pub fn find_slot(items: &[Item], grid: Grid, w: u32, h: u32) -> Option<Rect> {
    let w = w.min(grid.cols);
    for y in 0..=grid.rows.saturating_sub(h) {
        for x in 0..=grid.cols - w {
            let r = Rect { x, y, w, h };
            if !items.iter().any(|o| o.rect.overlaps(&r)) {
                return Some(r);
            }
        }
    }
    None
}

pub fn analyze(items: &[Item], grid: Grid, ctx: &PageCtx) -> Vec<Issue> {
    let mut issues = Vec::new();
    let title = |id: &str| (ctx.titles)(id);

    if items.is_empty() {
        issues.push(Issue {
            kind: "empty_page".into(),
            severity: "info".into(),
            widget: None,
            message: "This page is empty.".into(),
            suggestions: vec![Suggestion {
                label: "Remove page".into(),
                detail: "Delete the empty page from the report.".into(),
                ops: vec![Op::RemovePage { page: ctx.page_id.into() }],
            }],
        });
        return issues;
    }

    // ---- overflow past the bottom of the page
    let over: Vec<&Item> = items.iter().filter(|i| i.rect.bottom() > grid.rows).collect();
    if !over.is_empty() {
        let fitted = {
            let mut t = items.to_vec();
            compact(&mut t, None);
            shrink_to_fit(&mut t, grid.rows, None).then_some(t)
        };
        for it in &over {
            let mut sugg = Vec::new();
            if let Some(t) = &fitted {
                let ops = rect_ops(items, t);
                let shrunk = ops.len();
                sugg.push(Suggestion {
                    label: "Fit everything on this page".into(),
                    detail: format!("Compact the page and make {shrunk} widget{} a little smaller.", if shrunk == 1 { "" } else { "s" }),
                    ops,
                });
            }
            let room = ctx.next.and_then(|(pid, next)| find_slot(next, grid, it.rect.w, it.rect.h).map(|r| (pid, r)));
            match room {
                Some((pid, _)) => sugg.push(Suggestion {
                    label: "Move to the next page".into(),
                    detail: format!("There is room for “{}” on the next page.", title(&it.id)),
                    ops: vec![Op::MoveToPage { id: it.id.clone(), page: Some(pid.into()) }],
                }),
                None => sugg.push(Suggestion {
                    label: "Move to a new page".into(),
                    detail: format!("Start a new page with “{}”.", title(&it.id)),
                    ops: vec![Op::MoveToPage { id: it.id.clone(), page: None }],
                }),
            }
            if let Some((shown, total)) = it.table_rows {
                let _ = shown;
                let spare = grid.rows.saturating_sub(it.rect.y);
                if spare >= spec("table").min_h && total > 0 {
                    sugg.push(Suggestion {
                        label: format!("Shrink the table to {spare} rows of space"),
                        detail: "Keep the table on this page and show its top rows.".into(),
                        ops: vec![Op::SetRect { id: it.id.clone(), rect: Rect { h: spare, ..it.rect } }],
                    });
                }
            }
            sugg.push(Suggestion {
                label: "Remove it".into(),
                detail: format!("Delete “{}” from the report.", title(&it.id)),
                ops: vec![Op::Remove { id: it.id.clone() }],
            });
            let by = it.rect.bottom() - grid.rows;
            issues.push(Issue {
                kind: "overflow".into(),
                severity: "error".into(),
                widget: Some(it.id.clone()),
                message: format!(
                    "“{}” runs {by} row{} past the bottom of the page.",
                    title(&it.id),
                    if by == 1 { "" } else { "s" }
                ),
                suggestions: sugg,
            });
        }
    }

    // ---- content that doesn't fit its box
    for it in items {
        if let Some((shown, total)) = it.table_rows {
            if shown < total {
                let hidden = total - shown;
                let need = it.need_h.unwrap_or(it.rect.h);
                let mut sugg = vec![Suggestion {
                    label: "Continue on next pages".into(),
                    detail: "Carry the remaining rows onto new pages, with the header repeated.".into(),
                    ops: vec![Op::Paginate { id: it.id.clone() }],
                }];
                let grown = Rect { h: need, ..it.rect };
                if grown.bottom() <= grid.rows {
                    let mut t = items.to_vec();
                    if let Some(x) = t.iter_mut().find(|x| x.id == it.id) {
                        x.rect = grown;
                    }
                    resolve(&mut t, Some(&it.id));
                    if bottom(&t) <= grid.rows || shrink_to_fit(&mut t, grid.rows, Some(&it.id)) {
                        sugg.push(Suggestion {
                            label: format!("Make the table {} row{} taller", need - it.rect.h, if need - it.rect.h == 1 { "" } else { "s" }),
                            detail: "Show every row; other widgets make room.".into(),
                            ops: rect_ops(items, &t),
                        });
                    }
                }
                if shown >= 2 {
                    sugg.push(Suggestion {
                        label: format!("Show top {} + Other", shown - 1),
                        detail: "Keep the size and fold the remaining rows into one “Other” row.".into(),
                        ops: vec![Op::SetLimit { id: it.id.clone(), n: shown - 1 }],
                    });
                }
                sugg.push(Suggestion {
                    label: "Give it its own page".into(),
                    detail: "Move the table to a new page with room for every row.".into(),
                    ops: vec![Op::MoveToPage { id: it.id.clone(), page: None }],
                });
                issues.push(Issue {
                    kind: "table_rows".into(),
                    severity: "warning".into(),
                    widget: Some(it.id.clone()),
                    message: format!("“{}” hides {hidden} of {total} rows.", title(&it.id)),
                    suggestions: sugg,
                });
            }
        }
        // text, and a KPI's figure with the lines under it, are cut off when the widget is shorter
        if matches!(it.kind.as_str(), "text" | "comment" | "heading" | "kpi") {
            let kpi = it.kind == "kpi";
            if let Some(need) = it.need_h {
                if need > it.rect.h {
                    let mut t = items.to_vec();
                    if let Some(x) = t.iter_mut().find(|x| x.id == it.id) {
                        x.rect.h = need;
                    }
                    resolve(&mut t, Some(&it.id));
                    let mut sugg = Vec::new();
                    if bottom(&t) <= grid.rows || shrink_to_fit(&mut t, grid.rows, Some(&it.id)) {
                        sugg.push(Suggestion {
                            label: if kpi { "Grow to fit the figures".into() } else { "Grow to fit the text".into() },
                            detail: format!("Make it {} row{} taller; other widgets make room.", need - it.rect.h, if need - it.rect.h == 1 { "" } else { "s" }),
                            ops: rect_ops(items, &t),
                        });
                    }
                    // a wider box needs fewer lines (a KPI's lines don't wrap)
                    if it.rect.w < grid.cols && !kpi {
                        let mut wide = it.rect;
                        while wide.right() < grid.cols && !collides(items, items.iter().position(|x| x.id == it.id).unwrap(), &Rect { w: wide.w + 1, ..wide }) {
                            wide.w += 1;
                        }
                        if wide.w > it.rect.w {
                            sugg.push(Suggestion {
                                label: "Widen into free space".into(),
                                detail: "Use the empty columns beside it so the text needs fewer lines.".into(),
                                ops: vec![Op::SetRect { id: it.id.clone(), rect: wide }],
                            });
                        }
                    }
                    issues.push(Issue {
                        kind: "text_clipped".into(),
                        severity: "warning".into(),
                        widget: Some(it.id.clone()),
                        message: if kpi { format!("The figures in “{}” are cut off at the bottom.", title(&it.id)) } else { format!("The text in “{}” is cut off.", title(&it.id)) },
                        suggestions: sugg,
                    });
                }
            }
        }
        let s = it.spec();
        if it.rect.w < s.min_w || it.rect.h < s.min_h {
            let mut t = items.to_vec();
            if let Some(x) = t.iter_mut().find(|x| x.id == it.id) {
                x.rect.w = x.rect.w.max(s.min_w);
                x.rect.h = x.rect.h.max(s.min_h);
            }
            clamp_x(&mut t, grid.cols);
            resolve(&mut t, Some(&it.id));
            issues.push(Issue {
                kind: "too_small".into(),
                severity: "warning".into(),
                widget: Some(it.id.clone()),
                message: format!("“{}” is too small to read well (minimum {}×{}).", title(&it.id), s.min_w, s.min_h),
                suggestions: vec![Suggestion {
                    label: "Enlarge to the minimum size".into(),
                    detail: "Other widgets move down to make room.".into(),
                    ops: rect_ops(items, &t),
                }],
            });
        }
    }

    // ---- unused space (only when nothing is broken)
    if issues.iter().all(|i| i.severity == "info") {
        let used = bottom(items);
        let mut compacted = items.to_vec();
        compact(&mut compacted, None);
        let holes = rect_ops(items, &compacted);
        if !holes.is_empty() {
            issues.push(Issue {
                kind: "gaps".into(),
                severity: "info".into(),
                widget: None,
                message: "There are empty gaps between widgets.".into(),
                suggestions: vec![Suggestion {
                    label: "Close the gaps".into(),
                    detail: "Move widgets up into the empty space.".into(),
                    ops: holes,
                }],
            });
        } else {
            let mut filled = items.to_vec();
            fill(&mut filled, grid, None);
            let ops = rect_ops(items, &filled);
            let spare = grid.rows.saturating_sub(used);
            let free_cells = (grid.rows * grid.cols) as i64 - items.iter().map(|i| (i.rect.w * i.rect.h) as i64).sum::<i64>();
            if !ops.is_empty() && (spare >= 1 || free_cells >= grid.rows as i64) {
                issues.push(Issue {
                    kind: "empty_space".into(),
                    severity: "info".into(),
                    widget: None,
                    message: if spare > 0 {
                        format!("{spare} empty row{} at the bottom of the page.", if spare == 1 { "" } else { "s" })
                    } else {
                        "Part of the page is unused.".into()
                    },
                    suggestions: vec![Suggestion {
                        label: "Stretch widgets to fill the page".into(),
                        detail: "Grow charts and tables into the free space.".into(),
                        ops,
                    }],
                });
            }
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: &str, x: u32, y: u32, w: u32, h: u32) -> Item {
        Item { id: id.into(), kind: kind.into(), rect: Rect { x, y, w, h }, locked: false, need_h: None, table_rows: None }
    }

    const G: Grid = Grid { cols: 12, rows: 12 };

    fn get<'a>(items: &'a [Item], id: &str) -> Rect {
        items.iter().find(|i| i.id == id).unwrap().rect
    }

    fn no_overlaps(items: &[Item]) -> bool {
        for (i, a) in items.iter().enumerate() {
            for b in &items[i + 1..] {
                if a.rect.overlaps(&b.rect) {
                    return false;
                }
            }
        }
        true
    }

    #[test]
    fn placing_pushes_others_down() {
        let items = vec![item("a", "bar", 0, 0, 6, 4), item("b", "bar", 0, 4, 6, 4), item("c", "bar", 6, 0, 6, 4)];
        let mut moved = items.clone();
        moved[2].rect = Rect { x: 0, y: 2, w: 6, h: 4 };
        let out = arrange(moved, G, &Options::default(), Some("c"), false);
        assert!(no_overlaps(&out));
        assert_eq!(get(&out, "c"), Rect { x: 0, y: 2, w: 6, h: 4 });
        assert_eq!(get(&out, "a"), Rect { x: 0, y: 0, w: 6, h: 4 }.with_y(6));
        // b would land past the page end; it takes the free spot beside instead
        assert!(get(&out, "b").bottom() <= 12, "{:?}", get(&out, "b"));
    }

    #[test]
    fn locked_widgets_stay_put() {
        let mut a = item("head", "heading", 0, 0, 12, 1);
        a.locked = true;
        let items = vec![a, item("x", "bar", 0, 0, 6, 4)];
        let out = arrange(items, G, &Options::default(), Some("x"), false);
        assert_eq!(get(&out, "head").y, 0);
        assert_eq!(get(&out, "x").y, 1);
    }

    #[test]
    fn auto_mode_shrinks_charts_to_fit() {
        // three stacked rows of charts plus a comment: 4+4+4+2 = 14 rows > 12
        let items = vec![
            item("a", "bar", 0, 0, 6, 4),
            item("b", "line", 6, 0, 6, 4),
            item("c", "bar", 0, 4, 12, 4),
            item("d", "table", 0, 8, 12, 4),
            item("note", "comment", 0, 12, 12, 2),
        ];
        let mut with_need = items.clone();
        with_need[4].need_h = Some(2);
        let out = arrange(with_need, G, &Options { auto: true, fill: true }, Some("note"), false);
        assert!(no_overlaps(&out));
        assert!(bottom(&out) <= 12, "bottom {}", bottom(&out));
        assert_eq!(get(&out, "note").h, 2, "the comment keeps its text height");
        for id in ["a", "b", "c"] {
            assert!(get(&out, id).h >= 3);
        }
    }

    #[test]
    fn fill_stretches_into_free_space() {
        let items = vec![item("k1", "kpi", 0, 0, 4, 2), item("k2", "kpi", 4, 0, 4, 2), item("k3", "kpi", 8, 0, 4, 2), item("c", "bar", 0, 2, 12, 5)];
        let out = arrange(items.clone(), G, &Options { auto: true, fill: true }, None, false);
        assert_eq!(get(&out, "c").bottom(), 12, "chart takes the spare rows");
        assert_eq!(get(&out, "k1").h, 2, "kpis don't stretch");
        // removing the middle KPI leaves a hole the neighbours close
        let mut removed = items;
        removed.remove(1);
        let out = arrange(removed, G, &Options { auto: true, fill: true }, None, false);
        let covered: u32 = out.iter().filter(|i| i.rect.y == 0).map(|i| i.rect.w).sum();
        assert_eq!(covered, 12);
    }

    #[test]
    fn rows_of_cards_share_width_evenly() {
        let items = vec![item("k1", "kpi", 0, 0, 3, 2), item("k3", "kpi", 6, 0, 3, 2), item("k4", "kpi", 9, 0, 3, 2), item("c", "bar", 0, 2, 12, 10)];
        let out = arrange(items, G, &Options { auto: true, fill: true }, None, false);
        let ws: Vec<u32> = ["k1", "k3", "k4"].iter().map(|id| get(&out, id).w).collect();
        assert_eq!(ws, vec![4, 4, 4]);
    }

    #[test]
    fn unfittable_widget_keeps_its_size() {
        let mut big = item("big", "bar", 0, 0, 12, 11);
        big.locked = true;
        let items = vec![big, item("t", "table", 0, 11, 7, 4)];
        let out = arrange(items, G, &Options { auto: true, fill: true }, Some("t"), false);
        assert_eq!(get(&out, "t").h, 4, "not squashed when it can't fit anyway");
    }

    #[test]
    fn tables_stop_growing_at_their_rows() {
        let mut t = item("t", "table", 0, 0, 12, 3);
        t.need_h = Some(5);
        let out = arrange(vec![t], G, &Options { auto: true, fill: true }, None, false);
        assert_eq!(get(&out, "t").h, 5);
    }

    #[test]
    fn fill_does_not_regrow_pinned_widget() {
        let items = vec![item("a", "bar", 0, 0, 12, 4)];
        let out = arrange(items, G, &Options { auto: true, fill: true }, Some("a"), false);
        assert_eq!(get(&out, "a").h, 4);
    }

    #[test]
    fn a_hand_resize_sticks_on_a_full_page() {
        // a chart grown into a full page: the note below can't shrink, so it
        // moves past the page end instead of the chart snapping back
        let mut note = item("note", "comment", 0, 9, 12, 3);
        note.need_h = Some(3);
        let items = vec![item("kpi", "kpi", 0, 0, 12, 2), item("c", "line", 0, 2, 12, 9), note];
        let out = arrange(items.clone(), G, &Options { auto: true, fill: true }, Some("c"), true);
        assert_eq!(get(&out, "c").h, 9);
        assert!(no_overlaps(&out));
        // placing (not resizing) still gives way so the page fits
        let out = arrange(items, G, &Options { auto: true, fill: true }, Some("c"), false);
        assert!(get(&out, "c").h < 9);
    }

    #[test]
    fn a_drop_does_not_knock_others_off_a_roomy_page() {
        // a cover: title, period, then a rule and two cards at the bottom;
        // dropping the left card onto the rule pushes the rule and the right
        // card down; they move into the free rows instead of off the page
        let items = vec![
            item("title", "heading", 1, 2, 10, 4),
            item("period", "text", 1, 6, 10, 1),
            item("rule", "divider", 0, 8, 12, 1),
            item("person", "heading", 0, 9, 6, 3),
            item("date", "heading", 8, 9, 4, 3),
        ];
        let mut moved = items.clone();
        moved[3].rect.y = 8;
        let out = arrange(moved, G, &Options::default(), Some("person"), false);
        assert!(no_overlaps(&out));
        assert!(bottom(&out) <= 12, "{:?}", out.iter().map(|i| (&i.id, i.rect)).collect::<Vec<_>>());
        assert_eq!(get(&out, "person").y, 8);
    }

    #[test]
    fn overflow_reports_suggestions() {
        let mut a = item("big", "bar", 0, 0, 12, 8);
        a.locked = true;
        let items = vec![a, item("t", "table", 0, 8, 12, 6)];
        let titles = |id: &str| id.to_string();
        let ctx = PageCtx { page_id: "p1", next: None, titles: &titles };
        let issues = analyze(&items, G, &ctx);
        let o = issues.iter().find(|i| i.kind == "overflow").expect("overflow issue");
        assert_eq!(o.widget.as_deref(), Some("t"));
        assert!(o.suggestions.iter().any(|s| s.ops.contains(&Op::MoveToPage { id: "t".into(), page: None })));
        let fit = o.suggestions.iter().find(|s| s.label.starts_with("Fit everything")).expect("fit suggestion");
        assert!(fit.ops.iter().any(|op| matches!(op, Op::SetRect { id, rect } if id == "t" && rect.bottom() <= 12)));
    }

    #[test]
    fn clipped_table_suggests_growth_or_top_n() {
        let mut t = item("t", "table", 0, 0, 6, 4);
        t.table_rows = Some((6, 10));
        t.need_h = Some(6);
        let titles = |id: &str| id.to_string();
        let ctx = PageCtx { page_id: "p1", next: None, titles: &titles };
        let issues = analyze(&[t], G, &ctx);
        let i = issues.iter().find(|i| i.kind == "table_rows").unwrap();
        assert!(i.suggestions.iter().any(|s| s.ops == vec![Op::SetLimit { id: "t".into(), n: 5 }]));
        assert_eq!(i.suggestions[0].ops, vec![Op::Paginate { id: "t".into() }]);
        assert!(i.suggestions[1].ops.iter().any(|op| matches!(op, Op::SetRect { rect, .. } if rect.h == 6)));
    }

    #[test]
    fn finds_free_slot() {
        let items = vec![item("a", "bar", 0, 0, 12, 10)];
        assert_eq!(find_slot(&items, G, 6, 2), Some(Rect { x: 0, y: 10, w: 6, h: 2 }));
        assert_eq!(find_slot(&items, G, 6, 3), None);
    }

    impl Rect {
        fn with_y(self, y: u32) -> Rect {
            Rect { y, ..self }
        }
    }
}
