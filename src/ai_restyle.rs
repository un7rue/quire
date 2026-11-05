//! Restyling a template from a reference image. A vision model looks at the picture and
//! answers with a theme *look* — paper, ink, accent, performance and category colours,
//! typefaces and the shape language (corners, grid, bars, donut, cards) — written over the
//! look the template has now. The app checks it (colours, installed typefaces, readable
//! contrast) and builds the whole theme from it the way the built-in themes are built, so
//! every design view of the template follows. No data is sent: only the picture, the look.

use crate::uhnwi_theme::{Look, Shape, LOOKS, UHNWI};
use serde_json::{json, Value as Json};
use std::collections::HashSet;

/// The look a theme was built from: a restyle's own (kept in the theme), a built-in look by
/// its name, else the Modern UHNWI look.
pub fn base_look(theme: &crate::model::Theme) -> Look {
    let named = LOOKS.iter().find(|(_, l)| l.name == theme.name).map(|(_, l)| **l).unwrap_or(UHNWI);
    if theme.look.is_object() {
        // its own typefaces count as installed: they were checked when it was made
        let t = &theme.look["type"];
        let fonts: HashSet<String> = [t["display"].as_str(), t["ui"].as_str()].into_iter().flatten().map(String::from).collect();
        if let Ok(l) = look_from(&named, &json!({"name": theme.name, "look": theme.look}), &fonts) {
            return l;
        }
    }
    named
}

/// A look as the model reads and writes it.
pub fn look_json(l: &Look) -> Json {
    let s = &l.shape;
    json!({
        "name": l.name,
        "colors": {
            "page": l.page, "card": l.card, "card2": l.card2, "ink": l.ink, "ink2": l.ink2, "body": l.body, "soft": l.soft,
            "line": l.line, "line2": l.line2, "accent_line": l.accent_line, "accent": l.accent, "good": l.good, "bad": l.bad,
            "gain_tint": l.gain_tint, "loss_tint": l.loss_tint, "gain_mid": l.gain_mid, "loss_mid": l.loss_mid, "received": l.received,
            "icon_ink": l.icon_ink, "chip": l.chip, "share": l.share, "forecast": l.forecast, "donut_base": l.donut_base, "frame": l.frame,
            "rule": l.rule, "head_fill": l.head_fill, "head_ink": l.head_ink, "zebra": l.zebra,
            "cats": l.cats, "cats_soft": l.cats_soft, "classes": l.classes, "palette": l.palette,
        },
        "type": {"display": l.display, "display_weight": l.display_weight, "display_size": l.display_size, "display_tracking": l.display_tracking, "ui": l.ui, "title_weight": l.title_weight, "title_upper": l.title_upper},
        "blocks": l.blocks.iter().filter(|b| !b.is_empty()).collect::<Vec<_>>(), "cover_fill": l.cover_fill, "back_fill": l.back_fill,
        "radius": l.radius, "card_border_w": l.card_border_w,
        "shape": {
            "bar_radius": s.bar_radius, "round_all": s.round_all, "tint": s.tint, "outline": s.outline, "gradient": s.gradient, "bar_width": s.bar_width,
            "max_bar": s.max_bar, "grid": s.grid, "axis": s.axis, "line_width": s.line_width, "curve": s.curve, "markers": s.markers,
            "marker_fill": s.marker_fill, "area": s.area, "guides": s.guides, "donut": s.donut, "slice_gap": s.slice_gap, "labels": s.labels,
            "cards": s.cards, "kpi_weight": s.kpi_weight, "cards_all": s.cards_all, "card_pad": s.card_pad,
            "tile_fill": s.tile_fill, "tile_gap": s.tile_gap, "tile_labels": s.tile_labels,
        },
    })
}

/// The system prompt: what a look and the design views are, and how to answer.
pub fn system(current: &Json, views: &str, fonts: &[String], report_lang: &str, ui_lang: &str) -> String {
    let explain = if ui_lang == "ru" { "Russian" } else { "English" };
    format!(
        "You are the design assistant of Quire, an app a private bank's designers use to build client report templates.\n\
         You restyle a template so it looks as if the house in the reference pictures had designed it: the same pages, widgets and data, drawn in that house's visual language.\n\
         You see only the pictures — never the bank's data. Study every picture: page backgrounds, colour use, typefaces, titles, how charts, tables, numbers and legends are drawn, white space, rules, cards.\n\n\
         PART 1 — THE THEME LOOK (colours, typefaces, base shapes). Now:\n{current}\n\n\
         WHAT THE LOOK'S KEYS MEAN\n\
         - colors.page: page paper; card / card2: card fills; ink / ink2: headings and figures; body: running text; soft: captions, axis labels.\n\
         - line: gridlines and hairlines; line2: card outlines; accent / accent_line: brand colour and its rules; frame: thin page frame; rule: strong rules.\n\
         - good / bad: gains and losses; gain_tint / loss_tint: pale fills of good / bad's own hue (behind their icons and figures); gain_mid / loss_mid: middle steps.\n\
         - palette: 8 series colours, most important first; classes: 6 asset-class colours; cats: interest, dividend, coupon, income, principal; cats_soft: soft steps of dividend, coupon, income.\n\
         - received, chip, share, forecast, icon_ink, donut_base: small accents (icon_ink is drawn on chip: keep 3:1).\n\
         - head_fill / head_ink: a filled table-header band (\"\" = none); zebra: alternate rows (\"\" = none).\n\
         - type.display: headings' typeface (display_weight, display_size pt ~20–48, display_tracking em: -0.04…0 tightens heavy headlines); type.ui: everything else;\n\
           title_weight; title_upper: widget titles in capitals (false: as written — sentence case).\n\
         - blocks: up to 4 colours for a house that sets figures on SOLID COLOUR BLOCKS (e.g. yellow, orange, pink panels); views with style.fillCycle true take them in turn\n\
           on a page (text on a block is the ink, or the page colour where the ink wouldn't read). [] = none.\n\
         - cover_fill / back_fill: the cover's and the closing page's paper (\"\" = the page colour); only light papers (the ink stays the text colour).\n\
         - radius: card corners (pt); card_border_w.\n\
         - shape: bar_radius (0 square … 99 capsule), round_all, tint (0 solid … 1 white), outline, gradient, bar_width, max_bar (pt), grid none|solid|dashed|dotted, axis,\n\
           line_width, curve straight|smooth, markers none|last|all, marker_fill hollow|solid, area none|flat|gradient, guides, donut (hole 0.5–0.9), slice_gap, labels none|box|pill,\n\
           cards outlined|filled|shadow|plain, kpi_weight, cards_all, card_pad (pt), tile_fill (treemap tiles: 0.24 light tint … 1 full colour), tile_gap (pt), tile_labels split|top (top: name and value together in the corner).\n\n\
         PART 2 — THE DESIGN VIEWS. Every widget uses a design view of its kind; this is where the feel lives (how big numbers are, how titles, tables, legends,\n\
         labels, callouts, rings and cards are drawn). The views now (built from the look above), key \"kind/id\", what each is for, how many widgets use it, its settings:\n{views}\n\
         You may change, per view, only drawing settings:\n\
         - opts.viz: {viz}\n\
         - opts.table: {table} (header {{fill,color,upper,bold}}, totalFont {{family,size,weight}}; lines rows|grid|header|none; totalLine single|double|thick|none)\n\
         - opts.rule (headings: rule width under the title, 0 = none) and opts.ruleColor, opts.align, opts.density (compact|normal)\n\
         - headings' badge (the chip before the eyebrow, e.g. an account number): opts.badgeStyle solid|outline|text, opts.badgeFill (chip or frame colour),\n\
           opts.badgeColor (its text), opts.badgeRadius (pt; 0 square), opts.badgeWeight. Now: a solid $ink chip with rounded corners.\n\
         - style: card (true/false), fillCycle (true: this view's cards take the look's blocks in turn), pad, background, border, borderWidth, radius, titleStyle (\"\"|strong|dim), rule (sides: [\"left\"]…), ruleWidth, ruleColor, titleIcon (null removes the icon),\n\
           fonts.{{{roles}}}.{{family,size,weight,color,upper,tracking,italic}}\n\
         Colours may be #rrggbb or a theme colour as $name ($ink, $ink2, $body, $muted, $grid, $accent, $good, $bad, $page, $card, $card2, $rule, $frame…). null removes a setting.\n\
         Never change what a view shows (its data, chart type, orientation, columns, number formats, texts) — the report keeps its structure.\n\n\
         PART 3 — THE PAGE: page.header and page.footer bands: fill, line (none|top|bottom), line_color; header.tabs (none|left|center|right|full).\n\
         page.tab_style — how the section tabs in the header are drawn: style pill|box|underline|text (the current tab: a filled pill, a filled box, a rule under it,\n\
         or bold text alone), fill (the mark's colour), color (the current tab's text), weight, upper. Now: dark filled pills.\n\n\
         TYPEFACES installed here, with the weights each has (use only these; give the family name without the weights): {fonts}\n\n\
         RULES\n\
         - Be thorough: change every view whose drawing differs from the pictures' language, starting with the most used ones. Restyling only colours is not enough.\n\
         - TYPE IS THE SIGNATURE: match the headline's weight class first (a black/heavy headline needs a family that has 900 and display_weight 900 — a light\n\
           face at 800 does not read the same), its case (sentence case vs capitals), its tracking and its size relative to the body; then the body face.\n\
           When the house's own typefaces are named (FROM THE PDF FILE below), pick the installed family closest in build and weight.\n\
         - Houses that use flat COLOUR BLOCKS (solid panels in brand colours, square corners, no outlines or shadows): set blocks, fillCycle on the KPI and tile\n\
           views, radius 0, card_border_w 0, cards plain, labels box; a thick accent bar (style.rule [\"left\"], ruleWidth 4–6, ruleColor the brand colour) where\n\
           the pictures put one beside quotes and figure lists; table headers on a dark band (head_fill / head_ink) when the pictures do.\n\
         - Pictures of a style guide or a deck show examples on the guide's own pages: take the style of the examples (charts, tables, numbers, titles, legends), not the guide's page furniture\n\
           (its grey canvas, the frames or shadowed cards it puts examples on, its captions). A report page in that style is usually plain: white paper, no card frames unless the examples use them.\n\
         - Copy how colour is USED, not only which colours: if the examples draw data in one or two hues with a single highlight, do the same — views that colour bars by sign\n\
           (colorMode sign, green up / red down) take the house's hues instead (e.g. good = the main data hue, bad = a contrasting one from the pictures); set good and bad in the look\n\
           to the house's colours for up and down. Keep up and down distinguishable.\n\
         - Match the details the examples share: value labels on or off, legends or direct labels, gridlines, axis lines, ring thickness, bar width, number weight, title case and size.\n\
         - Figures written as text (KPI values, table cells, labels) keep the pictures' text colour — usually the ink — unless the examples colour them; colour belongs on the marks.\n\
           (In views: opts.viz.valueColor \"ink\" instead of \"sign\", tileTone/tone off, labelColor ink.)\n\
         - If the examples sit directly on the page (no panels), set style.card false and drop tinted backgrounds (style.background null, opts.viz.tileBg $page) in the views that draw panels.\n\
         - Text must stay readable (at least 4.5:1 on paper and cards) and must still fit: keep sizes close to the current ones unless the pictures clearly call for more air or less.\n\
         - All colours as #rrggbb or $name.\n\n\
         ANSWER with ONE JSON object and nothing else:\n\
         {{\"explanation\": \"2–4 sentences in {explain}: the visual language you took from the pictures\",\n\
          \"name\": \"a short theme name in {report}\",\n\
          \"look\": {{ the look, same keys as above; keys left out stay }},\n\
          \"views\": {{ \"kind/id\": {{\"opts\": {{…changes…}}, \"style\": {{…changes…}}}}, … }},\n\
          \"page\": {{\"header\": {{…}}, \"footer\": {{…}}, \"tab_style\": {{…}}}},\n\
          \"changes\": [\"short lines in {explain}, one per element: e.g. 'Tables: hairlines instead of a thick rule under the header'\"] }}",
        current = serde_json::to_string(current).unwrap_or_default(),
        viz = VIZ_KEYS.join(", "),
        table = TABLE_KEYS.join(", "),
        roles = FONT_ROLES.join(","),
        fonts = fonts.join(", "),
        report = if report_lang.starts_with("ru") { "Russian" } else { "English" },
    )
}

/// Typefaces worth offering: installed, not monospace or hidden, plus the built-in looks' own.
pub fn font_choices() -> Vec<String> {
    let mut v: Vec<String> = crate::fonts::families().into_iter().filter(|f| !f.monospace && !f.family.starts_with('.')).map(|f| f.family).collect();
    for (_, l) in LOOKS.iter() {
        for f in [l.display, l.ui] {
            if !v.iter().any(|x| x == f) {
                v.push(f.to_string());
            }
        }
    }
    v.sort();
    v.dedup();
    v.truncate(400);
    v
}

/// The typefaces offered, each with the weights it has: "Montserrat (400–900)".
pub fn font_menu(choices: &[String]) -> Vec<String> {
    let fams = crate::fonts::families();
    choices.iter().map(|c| match fams.iter().find(|f| &f.family == c).map(|f| &f.weights).filter(|w| !w.is_empty()) {
        Some(w) if w.len() > 1 => format!("{c} ({}–{})", w[0], w[w.len() - 1]),
        Some(w) => format!("{c} ({})", w[0]),
        None => c.clone(),
    }).collect()
}

/// A string the look keeps for good (looks hold `&'static str`; a restyle makes a few dozen short ones).
fn keep(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

fn hex_ok(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn rgb(s: &str) -> (f64, f64, f64) {
    let p = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0) as f64 / 255.0;
    (p(1), p(3), p(5))
}

/// The hue of a #rrggbb colour in degrees, or None for a near-grey (no hue to speak of).
pub fn hue(c: &str) -> Option<f64> {
    let (r, g, b) = rgb(c);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let d = mx - mn;
    let l = (mx + mn) / 2.0;
    let sat = if d == 0.0 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()) };
    if sat < 0.15 || d < 0.07 { return None; }
    let h = if mx == r { ((g - b) / d).rem_euclid(6.0) } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    Some(h * 60.0)
}

/// `a` moved toward `b` by `t` (0 = a, 1 = b).
pub fn mix(a: &str, b: &str, t: f64) -> String {
    let (x, y) = (rgb(a), rgb(b));
    let m = |p: f64, q: f64| ((p + (q - p) * t) * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", m(x.0, y.0), m(x.1, y.1), m(x.2, y.2))
}

/// WCAG contrast ratio of two #rrggbb colours.
pub fn contrast(a: &str, b: &str) -> f64 {
    let lum = |c: &str| {
        let (r, g, b) = rgb(c);
        let f = |x: f64| if x <= 0.03928 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) };
        0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// The model's look written over `base`, checked. Problems go back to the model.
pub fn look_from(base: &Look, ans: &Json, fonts: &HashSet<String>) -> Result<Look, Vec<String>> {
    let mut l = *base;
    let mut probs = Vec::new();
    let j = &ans["look"];
    if !j.is_object() {
        return Err(vec!["The answer has no \"look\" object.".into()]);
    }
    if let Some(n) = ans["name"].as_str().or(j["name"].as_str()).map(str::trim).filter(|n| !n.is_empty()) {
        l.name = keep(&n.chars().take(60).collect::<String>());
    }
    let c = &j["colors"];
    let mut col = |key: &str, slot: &mut &'static str, empty_ok: bool| {
        if let Some(v) = c[key].as_str() {
            let v = v.trim().to_lowercase();
            if (empty_ok && v.is_empty()) || hex_ok(&v) {
                *slot = keep(&v);
            } else {
                probs.push(format!("colors.{key}: \"{v}\" is not a #rrggbb colour."));
            }
        }
    };
    col("page", &mut l.page, false);
    col("card", &mut l.card, false);
    col("card2", &mut l.card2, false);
    col("ink", &mut l.ink, false);
    col("ink2", &mut l.ink2, false);
    col("body", &mut l.body, false);
    col("soft", &mut l.soft, false);
    col("line", &mut l.line, false);
    col("line2", &mut l.line2, false);
    col("accent_line", &mut l.accent_line, false);
    col("accent", &mut l.accent, false);
    col("good", &mut l.good, false);
    col("bad", &mut l.bad, false);
    col("gain_tint", &mut l.gain_tint, false);
    col("loss_tint", &mut l.loss_tint, false);
    col("gain_mid", &mut l.gain_mid, false);
    col("loss_mid", &mut l.loss_mid, false);
    col("received", &mut l.received, false);
    col("icon_ink", &mut l.icon_ink, false);
    col("chip", &mut l.chip, false);
    col("share", &mut l.share, false);
    col("forecast", &mut l.forecast, false);
    col("donut_base", &mut l.donut_base, false);
    col("frame", &mut l.frame, false);
    col("rule", &mut l.rule, false);
    col("head_fill", &mut l.head_fill, true);
    col("head_ink", &mut l.head_ink, true);
    col("zebra", &mut l.zebra, true);
    fn list<const N: usize>(c: &Json, key: &str, slot: &mut [&'static str; N], probs: &mut Vec<String>) {
        let Some(a) = c[key].as_array() else { return };
        if a.len() < N {
            probs.push(format!("colors.{key} needs {N} colours (it has {}).", a.len()));
            return;
        }
        for (i, v) in a.iter().take(N).enumerate() {
            match v.as_str().map(|s| s.trim().to_lowercase()) {
                Some(s) if hex_ok(&s) => slot[i] = keep(&s),
                _ => probs.push(format!("colors.{key}[{i}] is not a #rrggbb colour.")),
            }
        }
    }
    list(c, "cats", &mut l.cats, &mut probs);
    list(c, "cats_soft", &mut l.cats_soft, &mut probs);
    list(c, "classes", &mut l.classes, &mut probs);
    list(c, "palette", &mut l.palette, &mut probs);

    let t = &j["type"];
    for (key, slot) in [("display", &mut l.display), ("ui", &mut l.ui)] {
        if let Some(f) = t[key].as_str().map(str::trim) {
            if fonts.contains(f) {
                *slot = keep(f);
            } else {
                probs.push(format!("type.{key}: \"{f}\" isn't installed here; pick one from the list."));
            }
        }
    }
    let num = |v: &Json, lo: f64, hi: f64| v.as_f64().map(|x| x.clamp(lo, hi));
    let weight = |v: &Json| v.as_u64().map(|w| ((w.clamp(100, 900) / 100) * 100) as u16);
    if let Some(w) = weight(&t["display_weight"]) { l.display_weight = w; }
    if let Some(x) = num(&t["display_size"], 14.0, 60.0) { l.display_size = x; }
    if let Some(x) = num(&t["display_tracking"], -0.08, 0.3) { l.display_tracking = x; }
    if let Some(w) = weight(&t["title_weight"]) { l.title_weight = w; }
    if let Some(b) = t["title_upper"].as_bool() { l.title_upper = b; }
    if let Some(x) = num(&j["radius"], 0.0, 30.0) { l.radius = x; }
    // colour blocks (up to 4, in turn) and the cover's / closing page's paper
    if let Some(a) = j["blocks"].as_array() {
        let mut b = ["", "", "", ""];
        for (i, v) in a.iter().take(4).enumerate() {
            match v.as_str().map(|x| x.trim().to_lowercase()) {
                Some(x) if hex_ok(&x) => b[i] = keep(&x),
                _ => probs.push(format!("blocks[{i}] is not a #rrggbb colour.")),
            }
        }
        l.blocks = b;
    }
    for (key, slot) in [("cover_fill", &mut l.cover_fill), ("back_fill", &mut l.back_fill)] {
        if let Some(v) = j[key].as_str().map(|x| x.trim().to_lowercase()) {
            if v.is_empty() || hex_ok(&v) { *slot = keep(&v); } else { probs.push(format!("{key}: \"{v}\" is not a #rrggbb colour.")); }
        }
    }
    if let Some(x) = num(&j["card_border_w"], 0.0, 3.0) { l.card_border_w = x; }

    let s = &j["shape"];
    let mut sh: Shape = l.shape;
    let pick = |v: &Json, ok: &[&'static str], key: &str, slot: &mut &'static str, probs: &mut Vec<String>| {
        if let Some(x) = v.as_str() {
            match ok.iter().find(|o| **o == x) {
                Some(o) => *slot = o,
                None => probs.push(format!("shape.{key}: \"{x}\" is not one of {}.", ok.join(", "))),
            }
        }
    };
    if let Some(x) = num(&s["bar_radius"], 0.0, 99.0) { sh.bar_radius = x; }
    if let Some(b) = s["round_all"].as_bool() { sh.round_all = b; }
    if let Some(x) = num(&s["tint"], 0.0, 1.0) { sh.tint = x; }
    if let Some(b) = s["outline"].as_bool() { sh.outline = b; }
    if let Some(b) = s["gradient"].as_bool() { sh.gradient = b; }
    if let Some(x) = num(&s["bar_width"], 0.2, 0.95) { sh.bar_width = x; }
    if let Some(x) = num(&s["max_bar"], 6.0, 80.0) { sh.max_bar = x; }
    pick(&s["grid"], &["none", "solid", "dashed", "dotted"], "grid", &mut sh.grid, &mut probs);
    if let Some(b) = s["axis"].as_bool() { sh.axis = b; }
    if let Some(x) = num(&s["line_width"], 0.5, 5.0) { sh.line_width = x; }
    pick(&s["curve"], &["straight", "smooth"], "curve", &mut sh.curve, &mut probs);
    pick(&s["markers"], &["none", "last", "all"], "markers", &mut sh.markers, &mut probs);
    pick(&s["marker_fill"], &["hollow", "solid"], "marker_fill", &mut sh.marker_fill, &mut probs);
    pick(&s["area"], &["none", "flat", "gradient"], "area", &mut sh.area, &mut probs);
    if let Some(b) = s["guides"].as_bool() { sh.guides = b; }
    if let Some(x) = num(&s["donut"], 0.3, 0.95) { sh.donut = x; }
    if let Some(x) = num(&s["slice_gap"], 0.0, 6.0) { sh.slice_gap = x; }
    pick(&s["labels"], &["none", "box", "pill"], "labels", &mut sh.labels, &mut probs);
    pick(&s["cards"], &["outlined", "filled", "shadow", "plain"], "cards", &mut sh.cards, &mut probs);
    if let Some(w) = weight(&s["kpi_weight"]) { sh.kpi_weight = w; }
    if let Some(b) = s["cards_all"].as_bool() { sh.cards_all = b; }
    if let Some(x) = num(&s["card_pad"], 4.0, 24.0) { sh.card_pad = x; }
    if let Some(x) = num(&s["tile_fill"], 0.1, 1.0) { sh.tile_fill = x; }
    if let Some(x) = num(&s["tile_gap"], 0.0, 8.0) { sh.tile_gap = x; }
    pick(&s["tile_labels"], &["split", "top"], "tile_labels", &mut sh.tile_labels, &mut probs);
    l.shape = sh;

    // readable: text on paper and on cards
    if probs.is_empty() {
        for (a, an, b, bn, min) in [(l.ink, "ink", l.page, "page", 4.5), (l.ink, "ink", l.card, "card", 4.5), (l.body, "body", l.page, "page", 4.5), (l.soft, "soft", l.page, "page", 2.5)] {
            let r = contrast(a, b);
            if r < min {
                probs.push(format!("colors.{an} {a} on colors.{bn} {b} has contrast {r:.1}:1 — it needs at least {min}:1."));
            }
        }
        for (fill, name) in [(l.cover_fill, "cover_fill"), (l.back_fill, "back_fill")] {
            if !fill.is_empty() && contrast(l.ink, fill) < 4.5 {
                probs.push(format!("{name} {fill}: the ink {} on it has contrast {:.1}:1 — pages keep their dark text, so a paper needs at least 4.5:1 (leave it \"\" for a dark cover).", l.ink, contrast(l.ink, fill)));
            }
        }
        if contrast(l.icon_ink, l.chip) < 3.0 {
            probs.push(format!("colors.icon_ink {} on colors.chip {} has contrast {:.1}:1 — icons sit on the chip, so it needs at least 3:1.", l.icon_ink, l.chip, contrast(l.icon_ink, l.chip)));
        }
        for (t, tn, b, bn) in [(l.gain_tint, "gain_tint", l.good, "good"), (l.loss_tint, "loss_tint", l.bad, "bad")] {
            if !crate::model::tint_fits(t, b) {
                probs.push(format!("colors.{tn} {t} must be a pale tint of colors.{bn} {b} (the same hue, light enough for {bn} to read on it): it stands behind {bn} icons and figures, and another hue reads as the wrong sign."));
            }
        }
        if !l.head_fill.is_empty() && !l.head_ink.is_empty() && contrast(l.head_ink, l.head_fill) < 4.5 {
            probs.push(format!("colors.head_ink on colors.head_fill has contrast {:.1}:1 — it needs at least 4.5:1.", contrast(l.head_ink, l.head_fill)));
        }
    }
    if probs.is_empty() { Ok(l) } else { Err(probs) }
}

// ------------------------------------------------------------------ pictures

/// Pictures to send, from a file the person gave: a still picture as it is, an animated GIF
/// (a style guide, a deck recorded as a GIF) as up to `max` of its distinct frames. Each is
/// scaled to at most 1400 px and sent as JPEG; `thumb` is a small preview for the pane.
pub fn pictures(bytes: &[u8], max: usize) -> Result<Vec<Json>, String> {
    use image::{AnimationDecoder, DynamicImage, ImageFormat};
    let fmt = image::guess_format(bytes).map_err(|_| "That file isn’t a picture.".to_string())?;
    let mut frames: Vec<DynamicImage> = Vec::new();
    if fmt == ImageFormat::Gif {
        let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
        // the frames that differ from the one kept before (a GIF repeats a page for many frames)
        let mut last: Option<image::RgbImage> = None;
        for f in dec.into_frames().take(2000) {
            let f = f.map_err(|e| e.to_string())?;
            let img = DynamicImage::ImageRgba8(f.into_buffer());
            let small = img.thumbnail_exact(64, 40).to_rgb8();
            let differs = last.as_ref().is_none_or(|l| {
                let d: u64 = l.as_raw().iter().zip(small.as_raw()).map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs() as u64).sum();
                d / small.as_raw().len() as u64 > 6
            });
            if differs {
                last = Some(small);
                frames.push(img);
            }
        }
    } else {
        frames.push(image::load_from_memory(bytes).map_err(|_| "That file isn’t a picture.".to_string())?);
    }
    if frames.is_empty() {
        return Err("The picture has no frames.".into());
    }
    // evenly spread over the whole file
    let n = frames.len();
    let pick: Vec<usize> = if n <= max { (0..n).collect() } else { (0..max).map(|k| k * (n - 1) / (max - 1).max(1)).collect() };
    let jpeg = |img: &DynamicImage, side: u32, q: u8| -> Result<String, String> {
        use base64::Engine as _;
        let img = if img.width().max(img.height()) > side { img.resize(side, side, image::imageops::FilterType::Triangle) } else { img.clone() };
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q).encode_image(&img.to_rgb8()).map_err(|e| e.to_string())?;
        Ok(base64::engine::general_purpose::STANDARD.encode(out))
    };
    pick.into_iter()
        .map(|i| Ok(json!({"media": "image/jpeg", "data": jpeg(&frames[i], 1400, 85)?, "thumb": format!("data:image/jpeg;base64,{}", jpeg(&frames[i], 240, 70)?), "frames": n})))
        .collect()
}

// ------------------------------------------------------------------ design views

/// Chart options the restyle may change: how things are drawn, not what is shown.
const VIZ_KEYS: &[&str] = &[
    "areaFill", "areaOpacity", "arrow", "axis", "axisTicks", "bands", "barBase", "barFill", "barWidth", "baseColor", "callout", "calloutBg", "calloutColor",
    "center", "centerSize", "centerUpper", "chipBelow", "colorDown", "colorMode", "colorStart", "colorTotal", "colorUp", "connectors", "curve", "delta", "extremes",
    "extremesRule", "grid", "gridDir", "gridStyle", "guides", "highlight", "inner", "kpiAlign", "labelBold", "labelBox", "labelColor", "labelPos", "labels",
    "legend", "legendAmount", "legendCols", "legendShare", "legendStyle", "legendTotal", "lineColor", "lineStyle", "lineWidth", "markBest", "markerFill",
    "markers", "markerSize", "markerStyle", "markExtremes", "maxBar", "mono", "nameLabels", "noteColor", "outline", "outlineColor", "outlineWidth",
    "partOutline", "partTint", "radius", "rankHighlight", "rankNumbers", "rankStyle", "roundAll", "rowLines", "shareAside", "shareGap", "shareHeight",
    "sliceGap", "sortSlices", "spark", "sparkColor", "sparkPos", "subLine", "ticks", "tileBg", "tileBorder", "tileTone", "tint", "tmFill", "tmGap",
    "tmOutline", "tone", "track", "trackFill", "trackLines", "trackStyle", "valueColor", "valueLabels", "valueSize", "xLabels", "zeroLine", "cols", "signed",
];
/// Table options it may change (in opts.table).
const TABLE_KEYS: &[&str] = &[
    "header", "zebra", "zebraFill", "lines", "row_h", "head_h", "lineColor", "headLineColor", "headLineWidth", "groupFill", "groupColor", "groupUpper",
    "groupLine", "groupCount", "indent", "totalFill", "totalColor", "totalLine", "totalFont", "hideHead", "fill", "size",
];
const TABLE_SUB: &[(&str, &[&str])] = &[("header", &["fill", "color", "upper", "bold"]), ("totalFont", &["family", "size", "weight"])];
/// Widget style it may change.
const STYLE_KEYS: &[&str] = &["card", "fillCycle", "pad", "background", "border", "borderWidth", "radius", "color", "width", "titleIcon", "titleStyle", "rule", "ruleWidth", "ruleColor", "fonts"];
const FONT_ROLES: &[&str] = &["title", "subtitle", "kpi", "caption", "body", "heading", "heading_sub", "eyebrow", "table_head", "table_cell", "chart", "axis", "data_label", "legend"];
const FONT_KEYS: &[&str] = &["family", "size", "weight", "color", "upper", "tracking", "italic"];
/// Other options it may change, beside viz and table.
const OPTS_KEYS: &[&str] = &["viz", "table", "rule", "ruleColor", "align", "density", "badgeStyle", "badgeFill", "badgeColor", "badgeRadius", "badgeWeight"];

fn strip(v: &Json, keep: &dyn Fn(&str) -> bool) -> Json {
    match v.as_object() {
        Some(o) => Json::Object(o.iter().filter(|(k, _)| keep(k)).map(|(k, v)| (k.clone(), v.clone())).collect()),
        None => json!({}),
    }
}

/// The design views as the model sees them: what each is for, how often the template uses
/// it, and the settings it may change (what it shows is left out: it stays).
pub fn views_context(styles: &std::collections::BTreeMap<String, Vec<crate::model::StyleVariant>>, used: &std::collections::HashMap<String, usize>) -> String {
    let mut out = String::new();
    for (kind, list) in styles {
        for v in list {
            let key = format!("{kind}/{}", v.id);
            let mut opts = strip(&v.opts, &|k| OPTS_KEYS.contains(&k));
            if let Some(z) = opts.get_mut("viz") { *z = strip(z, &|k| VIZ_KEYS.contains(&k)); }
            if let Some(t) = opts.get_mut("table") { *t = strip(t, &|k| TABLE_KEYS.contains(&k)); }
            let style = strip(&v.style, &|k| STYLE_KEYS.contains(&k));
            let shows = v.opts.get("orientation").and_then(|o| o.as_str()).map(|o| format!(" · {o}")).unwrap_or_default();
            out.push_str(&format!("- {key} \"{}\"{} — {}{}: {}\n", v.name, shows, v.description,
                match used.get(&key) { Some(n) => format!(" [used by {n}]"), None => " [unused]".into() },
                serde_json::to_string(&json!({"opts": opts, "style": style})).unwrap_or_default()));
        }
    }
    out
}

/// Settings that hold a colour (others with "color" in the name, like colorMode or valueColor,
/// take a mode such as "sign" or "ink").
const COLOUR_KEYS: &[&str] = &["colorUp", "colorDown", "colorTotal", "colorStart", "lineColor", "outlineColor", "calloutBg", "calloutColor", "sparkColor", "tileBg", "baseColor",
    "zebraFill", "lineColor", "headLineColor", "groupFill", "groupColor", "groupLine", "totalFill", "totalColor"];

/// A colour, a theme colour ($name) or a keyword such as "none" or "ink".
fn colour_or_word(v: &Json) -> bool {
    colour_ok(v) || v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 20 && s.chars().all(|c| c.is_ascii_alphabetic()))
}

fn colour_ok(v: &Json) -> bool {
    v.is_null() || v.as_str().is_some_and(|s| s.is_empty() || hex_ok(&s.to_lowercase()) || (s.starts_with('$') && s.len() > 1 && s[1..].chars().all(|c| c.is_ascii_alphanumeric())))
}

/// Checks the views' changes: known views, settings it may change, real colours and installed
/// typefaces. Answers them keyed "kind/id" → {opts, style}.
pub fn check_views(styles: &std::collections::BTreeMap<String, Vec<crate::model::StyleVariant>>, views: &Json, fonts: &HashSet<String>) -> Result<serde_json::Map<String, Json>, Vec<String>> {
    let mut probs = Vec::new();
    let mut out = serde_json::Map::new();
    let Some(all) = views.as_object() else { return Ok(out) };
    for (key, patch) in all {
        let Some((kind, id)) = key.split_once('/') else { probs.push(format!("views: \"{key}\" should be kind/id.")); continue };
        if !styles.get(kind).is_some_and(|l| l.iter().any(|v| v.id == id)) {
            probs.push(format!("views: there is no design view \"{key}\"."));
            continue;
        }
        let mut bad = |what: String| probs.push(format!("{key}: {what}"));
        for (k, v) in patch["opts"].as_object().into_iter().flatten() {
            match k.as_str() {
                "viz" => for (zk, zv) in v.as_object().into_iter().flatten() {
                    if !VIZ_KEYS.contains(&zk.as_str()) { bad(format!("opts.viz.{zk} can't be changed (it isn't a drawing setting).")); }
                    else if COLOUR_KEYS.contains(&zk.as_str()) && !colour_or_word(zv) { bad(format!("opts.viz.{zk} {zv} is not a colour.")); }
                },
                "table" => for (tk, tv) in v.as_object().into_iter().flatten() {
                    if !TABLE_KEYS.contains(&tk.as_str()) { bad(format!("opts.table.{tk} can't be changed.")); continue; }
                    if let Some((_, sub)) = TABLE_SUB.iter().find(|(n, _)| *n == tk) {
                        for (sk, sv) in tv.as_object().into_iter().flatten() {
                            if !sub.contains(&sk.as_str()) { bad(format!("opts.table.{tk}.{sk} isn't a setting.")); }
                            if sk == "family" && !sv.as_str().is_some_and(|f| fonts.contains(f)) { bad(format!("opts.table.{tk}.family {sv} isn't installed.")); }
                            if (sk == "fill" || sk == "color") && !colour_ok(sv) { bad(format!("opts.table.{tk}.{sk} {sv} is not a colour.")); }
                        }
                    } else if COLOUR_KEYS.contains(&tk.as_str()) && !colour_or_word(tv) {
                        bad(format!("opts.table.{tk} {tv} is not a colour."));
                    }
                },
                "badgeStyle" if !matches!(v.as_str(), Some("solid" | "outline" | "text")) => bad(format!("opts.badgeStyle {v} is not solid, outline or text.")),
                "badgeFill" | "badgeColor" | "ruleColor" if !colour_ok(v) => bad(format!("opts.{k} {v} is not a colour.")),
                k if OPTS_KEYS.contains(&k) => {}
                _ => bad(format!("opts.{k} can't be changed: it decides what the view shows, which stays.")),
            }
        }
        for (k, v) in patch["style"].as_object().into_iter().flatten() {
            match k.as_str() {
                "fonts" => for (role, f) in v.as_object().into_iter().flatten() {
                    if !FONT_ROLES.contains(&role.as_str()) { bad(format!("style.fonts.{role} isn't a text role ({}).", FONT_ROLES.join(", "))); continue; }
                    for (fk, fv) in f.as_object().into_iter().flatten() {
                        if !FONT_KEYS.contains(&fk.as_str()) { bad(format!("style.fonts.{role}.{fk} isn't a font setting.")); }
                        if fk == "family" && !fv.is_null() && !fv.as_str().is_some_and(|x| fonts.contains(x)) { bad(format!("style.fonts.{role}.family {fv} isn't installed.")); }
                        if fk == "color" && !colour_ok(fv) { bad(format!("style.fonts.{role}.color {fv} is not a colour.")); }
                    }
                },
                "titleIcon" if !v.is_null() && v != &json!(false) => bad("style.titleIcon can only be removed (null).".into()),
                "background" | "color" | "border" | "ruleColor" if !colour_ok(v) => bad(format!("style.{k} {v} is not a colour.")),
                k if STYLE_KEYS.contains(&k) => {}
                _ => bad(format!("style.{k} isn't a style setting.")),
            }
        }
        let mut clean = json!({});
        if patch["opts"].is_object() { clean["opts"] = patch["opts"].clone(); }
        if patch["style"].is_object() { clean["style"] = patch["style"].clone(); }
        if let Some(t) = clean.pointer_mut("/style/titleIcon").filter(|t| **t == json!(false)) { *t = Json::Null; }
        out.insert(key.clone(), clean);
    }
    if probs.is_empty() { Ok(out) } else { Err(probs) }
}

/// The page header and footer bands: their fill, rule and section tabs (their texts stay), and
/// how the section tabs are drawn.
pub fn check_page(page: &Json) -> Result<Json, Vec<String>> {
    let mut probs = Vec::new();
    let mut out = json!({});
    for (k, v) in page["tab_style"].as_object().into_iter().flatten() {
        let ok = match k.as_str() {
            "style" => matches!(v.as_str(), Some("pill" | "box" | "underline" | "text")),
            "fill" | "color" => colour_ok(v),
            "weight" => v.as_u64().is_some_and(|w| (100..=900).contains(&w)),
            "upper" => v.is_boolean(),
            _ => false,
        };
        if ok { out["tab_style"][k] = v.clone(); } else { probs.push(format!("page.tab_style.{k} = {v} isn't a setting (style pill|box|underline|text, fill, color, weight, upper).")); }
    }
    for band in ["header", "footer"] {
        for (k, v) in page[band].as_object().into_iter().flatten() {
            let ok = match k.as_str() {
                "fill" | "line_color" => colour_ok(v),
                "line" => matches!(v.as_str(), Some("none" | "top" | "bottom")),
                "tabs" => band == "header" && matches!(v.as_str(), Some("none" | "left" | "center" | "right" | "full")),
                _ => false,
            };
            if ok { out[band][k] = v.clone(); } else { probs.push(format!("page.{band}.{k} = {v} isn't a setting it may change (fill, line none|top|bottom, line_color{}).", if band == "header" { ", tabs" } else { "" })); }
        }
    }
    if probs.is_empty() { Ok(out) } else { Err(probs) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonts() -> HashSet<String> {
        ["Inter", "Manrope", "Spectral"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_look_round_trips_and_overrides() {
        let base = base_look(&crate::uhnwi_theme::theme());
        let mut j = look_json(&base);
        j["colors"]["accent"] = json!("#0055AA");
        j["shape"]["grid"] = json!("dashed");
        j["type"]["display"] = json!("Inter");
        let l = look_from(&base, &json!({"name": "Blue", "look": j}), &fonts()).unwrap();
        assert_eq!(l.accent, "#0055aa");
        assert_eq!(l.shape.grid, "dashed");
        assert_eq!(l.display, "Inter");
        assert_eq!(l.name, "Blue");
        assert_eq!(l.page, base.page, "what it leaves out stays");
    }

    #[test]
    fn bad_colours_fonts_and_contrast_go_back() {
        let base = base_look(&crate::model::Theme::default());
        let e = look_from(&base, &json!({"look": {"colors": {"accent": "red", "palette": ["#000000"]}, "type": {"ui": "Comic Neue"}, "shape": {"grid": "wavy"}}}), &fonts()).unwrap_err().join("\n");
        assert!(e.contains("accent") && e.contains("palette needs 8") && e.contains("Comic Neue") && e.contains("wavy"), "{e}");
        let e = look_from(&base, &json!({"look": {"colors": {"ink": "#eeeeee", "page": "#ffffff"}}}), &fonts()).unwrap_err().join("\n");
        assert!(e.contains("contrast"), "{e}");
        assert!(contrast("#000000", "#ffffff") > 20.0);
        // every built-in look keeps its icons and gain / loss tints readable
        for (name, l) in crate::uhnwi_theme::LOOKS.iter().chain([("blocks", &crate::uhnwi_theme::BLOCKS)].iter()) {
            assert!(contrast(l.icon_ink, l.chip) >= 3.0, "{name}: icon on chip");
            assert!(crate::model::tint_fits(l.gain_tint, l.good) && crate::model::tint_fits(l.loss_tint, l.bad), "{name}: tints");
        }
        // a restyle's pink "gain" tint and black-on-black icons are mended when the theme loads
        let mut t = crate::model::Theme::default();
        t.good = "#00926c".into();
        t.bad = "#e4042a".into();
        t.tokens.insert("gainTint".into(), "#f3a4bb".into());
        t.tokens.insert("lossTint".into(), "#f7b4a3".into());
        t.tokens.insert("chip".into(), "#000000".into());
        t.tokens.insert("iconInk".into(), "#000000".into());
        t.heal_colours();
        assert_eq!(t.tokens["iconInk"], "#ffffff");
        assert_ne!(t.tokens["gainTint"], "#f3a4bb");
        assert!(crate::model::tint_fits(&t.tokens["gainTint"], "#00926c"));
        assert_eq!(t.tokens["lossTint"], "#f7b4a3");
    }

    #[test]
    fn views_may_change_drawing_not_content() {
        let th = crate::uhnwi_theme::theme();
        let f = fonts();
        let ok = check_views(&th.styles, &json!({"kpi/hero": {"style": {"card": false, "fonts": {"kpi": {"weight": 400, "family": "Inter", "color": "$ink"}}, "titleIcon": false}},
            "bar/ranked": {"opts": {"viz": {"rankNumbers": false, "labels": "all", "colorMode": "sign", "valueColor": "ink", "colorUp": "$accent"}}},
            "table/lines": {"opts": {"table": {"header": {"fill": "#051c2c", "color": "#ffffff"}, "groupFill": "none"}}}}), &f).unwrap();
        assert_eq!(ok.len(), 3, "modes such as colorMode sign, valueColor ink and groupFill none are not colours to check");
        assert!(check_views(&th.styles, &json!({"bar/ranked": {"opts": {"viz": {"colorUp": "light blue #00f"}}}}), &f).is_err());
        assert!(ok["kpi/hero"]["style"]["titleIcon"].is_null(), "false removes the icon");
        let e = check_views(&th.styles, &json!({"bar/ranked": {"opts": {"orientation": "vertical", "viz": {"breakdown": true, "format": "percent"}}},
            "kpi/nope": {}, "table/lines": {"style": {"fonts": {"kpi": {"family": "Comic"}, "banner": {}}, "background": "blue"}}}), &f).unwrap_err().join("\n");
        for want in ["opts.orientation can't be changed", "opts.viz.breakdown", "opts.viz.format", "no design view \"kpi/nope\"", "Comic", "banner", "background"] {
            assert!(e.contains(want), "{want}: {e}");
        }
        assert!(check_page(&json!({"header": {"line": "bottom", "fill": "#ffffff"}, "footer": {"tabs": "left"}})).unwrap_err()[0].contains("footer.tabs"));
        assert!(check_page(&json!({"tab_style": {"style": "underline", "fill": "$accent", "upper": true}})).is_ok());
        assert!(check_page(&json!({"tab_style": {"style": "wavy"}})).is_err());
        assert!(check_views(&th.styles, &json!({"heading/account": {"opts": {"badgeStyle": "outline", "badgeFill": "$ink", "badgeRadius": 0}}}), &f).is_ok());
        assert!(check_views(&th.styles, &json!({"heading/account": {"opts": {"badgeStyle": "glow"}}}), &f).is_err());
        let ctx = views_context(&th.styles, &[("kpi/hero".to_string(), 3)].into_iter().collect());
        assert!(ctx.contains("kpi/hero") && ctx.contains("[used by 3]") && ctx.contains("[unused]"));
    }

    #[test]
    fn a_gif_becomes_its_distinct_frames() {
        use image::{codecs::gif::GifEncoder, Delay, Frame, Rgba, RgbaImage};
        let mut bytes = Vec::new();
        {
            let mut enc = GifEncoder::new(&mut bytes);
            for (k, c) in [[10u8, 20, 60], [10, 20, 60], [240, 240, 240], [0, 160, 230]].iter().enumerate() {
                let img = RgbaImage::from_pixel(80, 50, Rgba([c[0], c[1], c[2], 255]));
                enc.encode_frame(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(100 + k as u32, 1))).unwrap();
            }
        }
        let p = pictures(&bytes, 8).unwrap();
        assert_eq!(p.len(), 3, "the repeated frame is left out");
        assert!(p[0]["thumb"].as_str().unwrap().starts_with("data:image/jpeg;base64,"));
        assert!(pictures(b"not a picture", 8).is_err());
    }
}
