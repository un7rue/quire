//! Text measurement with the report's real fonts. Rust wraps every text block
//! so the fit engine knows exact heights and the screen, PDF and PPTX all
//! break lines in the same places.

use crate::fonts;
use serde::{Deserialize, Serialize};

pub static INTER_REGULAR: &[u8] = include_bytes!("../ui/fonts/Inter-Regular.ttf");
pub static INTER_MEDIUM: &[u8] = include_bytes!("../ui/fonts/Inter-Medium.ttf");
pub static INTER_SEMIBOLD: &[u8] = include_bytes!("../ui/fonts/Inter-SemiBold.ttf");
pub static INTER_BOLD: &[u8] = include_bytes!("../ui/fonts/Inter-Bold.ttf");

/// Families bundled with the app besides Inter: Manrope and IBM Plex Sans (sans),
/// Spectral, Cormorant Garamond and Playfair Display (serif).
pub static BUNDLED: &[&[u8]] = &[
    include_bytes!("../ui/fonts/Manrope-Regular.ttf"),
    include_bytes!("../ui/fonts/Manrope-Medium.ttf"),
    include_bytes!("../ui/fonts/Manrope-SemiBold.ttf"),
    include_bytes!("../ui/fonts/Manrope-Bold.ttf"),
    include_bytes!("../ui/fonts/Manrope-ExtraBold.ttf"),
    include_bytes!("../ui/fonts/Spectral-Light.ttf"),
    include_bytes!("../ui/fonts/Spectral-LightItalic.ttf"),
    include_bytes!("../ui/fonts/Spectral-Regular.ttf"),
    include_bytes!("../ui/fonts/Spectral-Italic.ttf"),
    include_bytes!("../ui/fonts/Spectral-Medium.ttf"),
    include_bytes!("../ui/fonts/CormorantGaramond-Light.ttf"),
    include_bytes!("../ui/fonts/CormorantGaramond-Regular.ttf"),
    include_bytes!("../ui/fonts/CormorantGaramond-Medium.ttf"),
    include_bytes!("../ui/fonts/CormorantGaramond-SemiBold.ttf"),
    include_bytes!("../ui/fonts/CormorantGaramond-Italic.ttf"),
    include_bytes!("../ui/fonts/PlayfairDisplay-Regular.ttf"),
    include_bytes!("../ui/fonts/PlayfairDisplay-Medium.ttf"),
    include_bytes!("../ui/fonts/PlayfairDisplay-Bold.ttf"),
    include_bytes!("../ui/fonts/PlayfairDisplay-Italic.ttf"),
    include_bytes!("../ui/fonts/IBMPlexSans-Light.ttf"),
    include_bytes!("../ui/fonts/IBMPlexSans-Regular.ttf"),
    include_bytes!("../ui/fonts/IBMPlexSans-Medium.ttf"),
    include_bytes!("../ui/fonts/IBMPlexSans-SemiBold.ttf"),
    include_bytes!("../ui/fonts/IBMPlexSans-Bold.ttf"),
    // Raleway and Montserrat (OFL, Google Fonts) for the themes drawn from reference designs
    include_bytes!("../ui/fonts/Raleway-Regular.ttf"),
    include_bytes!("../ui/fonts/Raleway-Medium.ttf"),
    include_bytes!("../ui/fonts/Raleway-SemiBold.ttf"),
    include_bytes!("../ui/fonts/Raleway-Bold.ttf"),
    include_bytes!("../ui/fonts/Raleway-ExtraBold.ttf"),
    include_bytes!("../ui/fonts/Montserrat-Regular.ttf"),
    include_bytes!("../ui/fonts/Montserrat-Medium.ttf"),
    include_bytes!("../ui/fonts/Montserrat-SemiBold.ttf"),
    include_bytes!("../ui/fonts/Montserrat-Bold.ttf"),
    include_bytes!("../ui/fonts/Montserrat-ExtraBold.ttf"),
    // black (900) weights, for houses whose headlines are heavy (e.g. BlackRock's): OFL
    include_bytes!("../ui/fonts/Montserrat-Black.ttf"),
    include_bytes!("../ui/fonts/Inter-ExtraBold.ttf"),
    include_bytes!("../ui/fonts/Inter-Black.ttf"),
    include_bytes!("../ui/fonts/RobotoCondensed-Regular.ttf"),
    include_bytes!("../ui/fonts/RobotoCondensed-Medium.ttf"),
    include_bytes!("../ui/fonts/RobotoCondensed-SemiBold.ttf"),
    include_bytes!("../ui/fonts/RobotoCondensed-Bold.ttf"),
    include_bytes!("../ui/fonts/RobotoCondensed-ExtraBold.ttf"),
];

/// A client's own typeface, when its files are in ui/fonts/private/ (see build.rs), and the
/// themes that use it.
pub mod private {
    include!(concat!(env!("OUT_DIR"), "/private_fonts.rs"));
}

/// A fully resolved font for one text role.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Font {
    pub family: String,
    pub size: f64,
    pub weight: u16,
    pub italic: bool,
    pub color: String,
    /// Render in capitals.
    pub upper: bool,
    /// Letter spacing in em.
    #[serde(default)]
    pub tracking: f64,
}

impl Font {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn inter(size: f64, weight: u16) -> Font {
        Font { family: fonts::DEFAULT_FAMILY.into(), size, weight, italic: false, color: "#0b0b0b".into(), upper: false, tracking: 0.0 }
    }

    /// The text as it will be drawn.
    pub fn apply<'a>(&self, s: &'a str) -> std::borrow::Cow<'a, str> {
        if self.upper {
            s.to_uppercase().into()
        } else {
            s.into()
        }
    }
}

/// Advance width of `s` in points (the text is measured as given).
pub fn width_f(s: &str, f: &Font) -> f64 {
    let face = fonts::face(&f.family, f.weight, f.italic);
    let fallback = if f.family == fonts::DEFAULT_FAMILY { None } else { Some(fonts::face(fonts::DEFAULT_FAMILY, f.weight, false)) };
    let mut w = 0.0;
    let mut n = 0usize;
    for ch in s.chars() {
        n += 1;
        w += face
            .advance(ch)
            // glyphs the face lacks fall back to another font in the renderer
            .or_else(|| fallback.as_ref().and_then(|fb| fb.advance(ch)))
            .unwrap_or(if ch.is_whitespace() { 0.28 } else if (ch as u32) > 0x2E80 { 1.0 } else { 0.55 });
    }
    w * f.size + f.tracking * f.size * n as f64
}

/// Inter at a size and weight (tests).
#[cfg_attr(not(test), allow(dead_code))]
pub fn width(s: &str, size: f64, weight: u16) -> f64 {
    width_f(s, &Font::inter(size, weight))
}

/// Greedy word wrap. Words longer than a line are broken by characters.
pub fn wrap(text: &str, max_w: f64, f: &Font) -> Vec<String> {
    // a little slack absorbs kerning differences between renderers
    let limit = (max_w / 1.02).max(f.size);
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if width_f(&cand, f) <= limit {
            cur = cand;
            continue;
        }
        if !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        if width_f(word, f) <= limit {
            cur = word.to_string();
        } else {
            for ch in word.chars() {
                let mut c2 = cur.clone();
                c2.push(ch);
                if width_f(&c2, f) > limit && !cur.is_empty() {
                    lines.push(std::mem::take(&mut cur));
                    cur.push(ch);
                } else {
                    cur = c2;
                }
            }
        }
    }
    lines.push(cur);
    lines
}

/// Truncates with an ellipsis to fit `max_w`.
pub fn ellipsize(s: &str, max_w: f64, f: &Font) -> String {
    let s = f.apply(s);
    if width_f(&s, f) <= max_w {
        return s.into_owned();
    }
    let mut out = String::new();
    for ch in s.chars() {
        let mut t = out.clone();
        t.push(ch);
        t.push('…');
        if width_f(&t, f) > max_w {
            break;
        }
        out.push(ch);
    }
    format!("{}…", out.trim_end())
}

#[derive(Clone, Debug, Serialize)]
pub struct Line {
    pub text: String,
    /// Styled pieces when the paragraph uses *italic* or **bold** markup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runs: Option<Vec<Run>>,
    /// Offset from the block's left edge.
    pub x: f64,
    /// Baseline offset from the block's top.
    pub y: f64,
    pub bullet: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Run {
    pub text: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Size against the text around it (1.25 larger, 0.85 smaller).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// An icon in the line ("up", "down", "check"…), drawn the size of a letter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

impl Run {
    fn plain(text: &str) -> Run {
        Run { text: text.into(), bold: false, italic: false, color: None, size: None, icon: None }
    }
    fn same_style(&self, o: &Run) -> bool {
        self.icon.is_none() && o.icon.is_none() && self.bold == o.bold && self.italic == o.italic && self.size == o.size
    }
}

/// The width an icon takes in a line, against the letter size.
pub const ICON_W: f64 = 1.1;

/// Width of a run in its own style.
pub fn run_width(r: &Run, f: &Font) -> f64 {
    let k = r.size.unwrap_or(1.0);
    if r.icon.is_some() {
        return f.size * k * ICON_W;
    }
    width_f(&r.text, &styled(f, r.bold, r.italic, k))
}

/// Width of a laid-out line, styled pieces measured in their own style.
pub fn line_width(l: &Line, f: &Font) -> f64 {
    match &l.runs {
        Some(runs) => runs.iter().map(|r| run_width(r, f)).sum(),
        None => width_f(&l.text, f),
    }
}

/// Splits markup into styled pieces:
///   `*italic*` and `**bold**`;
///   `{#rrggbb|coloured}`, `{+|larger}`, `{++|much larger}`, `{-|smaller}` — spans nest,
///   and a span may combine a colour with a size (`{#a12a2a+|…}`);
///   `{!up}` an icon ("up", "down", "check"…).
/// Text with a single or unmatched asterisk is left alone.
pub fn parse_rich(s: &str) -> Option<Vec<Run>> {
    if s.matches('*').count() < 2 && !s.contains('{') {
        return None;
    }
    let runs = rich(s);
    if runs.len() == 1 && runs[0] == Run::plain(s) {
        return None;
    }
    Some(runs)
}

enum Brace<'a> {
    Icon(&'a str),
    Span { color: Option<String>, scale: Option<f64>, inner: &'a str },
}

/// A span or an icon at the start of `s` (which starts with '{'), and its length.
fn brace(s: &str) -> Option<(Brace<'_>, usize)> {
    let body = &s[1..];
    if let Some(name) = body.strip_prefix('!') {
        let end = name.find('}')?;
        let n = &name[..end];
        if !n.is_empty() && n.len() <= 24 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Some((Brace::Icon(n), end + 3));
        }
        return None;
    }
    let bar = body.find('|')?;
    let spec = &body[..bar];
    if spec.is_empty() || spec.contains('}') || spec.contains('{') {
        return None;
    }
    let (color, size) = match spec.strip_prefix('#') {
        Some(rest) => {
            let hex: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if !(3..=8).contains(&hex.len()) {
                return None;
            }
            (Some(format!("#{hex}")), &rest[hex.len()..])
        }
        None => (None, spec),
    };
    let scale = match size {
        "" => None,
        "+" => Some(1.25),
        "++" => Some(1.5),
        "-" => Some(0.85),
        _ => return None,
    };
    // the closing brace that matches, spans inside counted
    let inner_start = bar + 1;
    let mut depth = 0usize;
    for (k, c) in body[inner_start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' if depth == 0 => {
                let inner = &body[inner_start..inner_start + k];
                return Some((Brace::Span { color, scale, inner }, 1 + inner_start + k + 1));
            }
            '}' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn rich(s: &str) -> Vec<Run> {
    let mut out = Vec::new();
    let (mut i, mut from) = (0, 0);
    while let Some(off) = s[i..].find('{') {
        let at = i + off;
        match brace(&s[at..]) {
            Some((b, len)) => {
                out.extend(emphasis(&s[from..at]));
                match b {
                    Brace::Icon(name) => out.push(Run { icon: Some(name.to_string()), ..Run::plain("") }),
                    Brace::Span { color, scale, inner } => {
                        for mut r in rich(inner) {
                            if r.color.is_none() {
                                r.color = color.clone();
                            }
                            if let Some(k) = scale {
                                r.size = Some(r.size.unwrap_or(1.0) * k);
                            }
                            out.push(r);
                        }
                    }
                }
                i = at + len;
                from = i;
            }
            None => i = at + 1,
        }
    }
    out.extend(emphasis(&s[from..]));
    out
}

/// `*italic*` and `**bold**` in plain text; unbalanced, the text stays as it is.
fn emphasis(s: &str) -> Vec<Run> {
    if s.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Run> = Vec::new();
    let (mut bold, mut italic) = (false, false);
    let mut cur = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let flush = |out: &mut Vec<Run>, cur: &mut String, bold: bool, italic: bool| {
        if !cur.is_empty() {
            out.push(Run { bold, italic, ..Run::plain(&std::mem::take(cur)) });
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && chars.get(i + 1) == Some(&'*') {
            cur.push('*');
            i += 2;
            continue;
        }
        if c == '*' {
            flush(&mut out, &mut cur, bold, italic);
            if chars.get(i + 1) == Some(&'*') {
                bold = !bold;
                i += 2;
            } else {
                italic = !italic;
                i += 1;
            }
            continue;
        }
        cur.push(c);
        i += 1;
    }
    flush(&mut out, &mut cur, bold, italic);
    if bold || italic {
        return vec![Run::plain(s)];
    }
    out
}

fn styled(f: &Font, bold: bool, italic: bool, scale: f64) -> Font {
    let mut g = f.clone();
    g.size *= scale;
    if bold {
        g.weight = if f.weight >= 600 { 800 } else { 700 };
    }
    if italic {
        g.italic = !f.italic;
    }
    g
}

/// Word wrap for styled text: each word is measured in its own style.
fn wrap_rich(runs: &[Run], max_w: f64, f: &Font) -> Vec<Vec<Run>> {
    let limit = (max_w / 1.02).max(f.size);
    // words carrying their style; a word may span runs ("*доход*ности")
    let mut words: Vec<Vec<Run>> = vec![Vec::new()];
    for r in runs {
        if r.icon.is_some() {
            words.last_mut().unwrap().push(r.clone());
            continue;
        }
        for (k, piece) in r.text.split(' ').enumerate() {
            if k > 0 {
                words.push(Vec::new());
            }
            if !piece.is_empty() {
                words.last_mut().unwrap().push(Run { text: piece.to_string(), ..r.clone() });
            }
        }
    }
    let wwidth = |w: &Vec<Run>| w.iter().map(|r| run_width(r, f)).sum::<f64>();
    let space = width_f(" ", f);
    let mut lines: Vec<Vec<Run>> = Vec::new();
    let mut cur: Vec<Run> = Vec::new();
    let mut cur_w = 0.0;
    for w in words.into_iter().filter(|w| !w.is_empty()) {
        let ww = wwidth(&w);
        let add = if cur.is_empty() { ww } else { cur_w + space + ww };
        if add <= limit || cur.is_empty() {
            if !cur.is_empty() {
                // a space is styled only when both neighbours share the style
                let last = cur.last().map(|r| (r.bold, r.italic)).unwrap();
                let next = (w[0].bold, w[0].italic);
                cur.push(Run { bold: last.0 && next.0, italic: last.1 && next.1, ..Run::plain(" ") });
            }
            cur.extend(w);
            cur_w = add;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = w;
            cur_w = ww;
        }
    }
    lines.push(cur);
    // merge neighbours that share a style
    lines
        .into_iter()
        .map(|l| {
            let mut m: Vec<Run> = Vec::new();
            for r in l {
                match m.last_mut() {
                    Some(p) if p.same_style(&r) && (p.color == r.color || r.text == " ") => p.text.push_str(&r.text),
                    _ => m.push(r),
                }
            }
            m
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
pub struct Block {
    pub lines: Vec<Line>,
    pub size: f64,
    pub weight: u16,
    pub family: String,
    pub italic: bool,
    pub tracking: f64,
    pub line_h: f64,
    pub height: f64,
}

pub const LINE_FACTOR: f64 = 1.42;
const BULLET_INDENT: f64 = 1.1;

/// Lays out paragraphs. Lines starting with "- ", "* " or "• " become bullets.
pub fn block(text: &str, max_w: f64, f: &Font) -> Block {
    block_lh(text, max_w, f, LINE_FACTOR)
}

/// The same with a chosen line height (headings set tighter, with no gap
/// between their lines).
pub fn block_lh(text: &str, max_w: f64, f: &Font, factor: f64) -> Block {
    let text = f.apply(text);
    let size = f.size;
    let line_h = ((size * factor) * 100.0).round() / 100.0;
    let mut lines = Vec::new();
    let mut y = 0.0;
    let para_gap = if factor < 1.3 { 0.0 } else { size * 0.45 };
    let mut first = true;
    for para in text.replace("\r\n", "\n").split('\n') {
        let trimmed = para.trim_start();
        let (bullet, body) = match trimmed.get(..2) {
            Some("- ") | Some("* ") => (true, &trimmed[2..]),
            _ if trimmed.starts_with("• ") => (true, &trimmed["• ".len()..]),
            _ => (false, para),
        };
        if para.trim().is_empty() {
            if !first {
                y += line_h * 0.6;
            }
            continue;
        }
        if !first {
            y += para_gap * if bullet { 0.3 } else { 1.0 };
        }
        first = false;
        let indent = if bullet { size * BULLET_INDENT } else { 0.0 };
        if let Some(runs) = parse_rich(body.trim()) {
            for (i, l) in wrap_rich(&runs, max_w - indent, f).into_iter().enumerate() {
                let text: String = l.iter().map(|r| r.text.as_str()).collect();
                // a line with larger words takes their height
                let k = l.iter().filter_map(|r| r.size).fold(1.0f64, f64::max);
                let extra = size * (k - 1.0);
                lines.push(Line { text, runs: Some(l), x: indent, y: y + extra + size * 0.96, bullet: bullet && i == 0 });
                y += line_h + extra * 1.2;
            }
            continue;
        }
        for (i, l) in wrap(body.trim(), max_w - indent, f).into_iter().enumerate() {
            lines.push(Line { text: l, runs: None, x: indent, y: y + size * 0.96, bullet: bullet && i == 0 });
            y += line_h;
        }
    }
    let height = if lines.is_empty() { 0.0 } else { y - (line_h - size * 1.2) };
    Block { lines, size, weight: f.weight, family: f.family.clone(), italic: f.italic, tracking: f.tracking, line_h, height: height.max(0.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_and_wraps() {
        let w = width("Revenue", 10.0, 400);
        assert!(w > 30.0 && w < 50.0, "{w}");
        assert!(width("Revenue", 10.0, 700) > w);
        let f = Font::inter(10.0, 400);
        let lines = wrap("the quick brown fox jumps over the lazy dog", 80.0, &f);
        assert!(lines.len() >= 3);
        for l in &lines {
            assert!(width_f(l, &f) <= 80.0);
        }
    }

    #[test]
    fn bullets_and_paragraphs() {
        let b = block("Summary line\n- first point\n- second point", 200.0, &Font::inter(10.0, 400));
        assert_eq!(b.lines.len(), 3);
        assert!(b.lines[1].bullet && b.lines[1].x > 0.0);
        assert!(b.height > 3.0 * 10.0);
    }

    #[test]
    fn rich_markup_makes_runs() {
        assert!(parse_rich("2 * 3").is_none());
        let r = parse_rich("Отчёт и *доходности*").unwrap();
        assert_eq!(r.len(), 2);
        assert!(r[1].italic && r[1].text == "доходности");
        let b = block("не **является** офертой", 400.0, &Font::inter(10.0, 400));
        let runs = b.lines[0].runs.as_ref().unwrap();
        assert!(runs.iter().any(|r| r.bold && r.text == "является"));
        assert_eq!(b.lines[0].text, "не является офертой");
        // sizes, icons and nested spans
        let r = parse_rich("Доход {#1f7a52+|{!up} **+6,48%**} за год {-|сноска}").unwrap();
        let up = r.iter().find(|x| x.icon.as_deref() == Some("up")).unwrap();
        assert_eq!((up.color.as_deref(), up.size), (Some("#1f7a52"), Some(1.25)));
        assert!(r.iter().any(|x| x.bold && x.text == "+6,48%" && x.color.as_deref() == Some("#1f7a52") && x.size == Some(1.25)));
        assert!(r.iter().any(|x| x.text == "сноска" && x.size == Some(0.85)));
        assert!(parse_rich("a {b} c").is_none(), "plain braces stay text");
        let b = block("Итог {++|большой} и {!check} готово", 400.0, &Font::inter(10.0, 400));
        assert!(b.height > block("Итог большой и готово", 400.0, &Font::inter(10.0, 400)).height, "a larger word makes the line taller");
        assert!(b.lines[0].runs.as_ref().unwrap().iter().any(|r| r.icon.as_deref() == Some("check")));
        let c = parse_rich("{#1f7a52|■} Вклады {#b1542f|■ **Дивиденды**}").unwrap();
        assert_eq!(c[0].color.as_deref(), Some("#1f7a52"));
        assert!(c.iter().any(|r| r.bold && r.color.as_deref() == Some("#b1542f")));
        // tracking widens text
        let t = Font { tracking: 0.1, ..Font::inter(10.0, 400) };
        assert!(width_f("ABCD", &t) > width_f("ABCD", &Font::inter(10.0, 400)) + 3.9);
    }

    #[test]
    fn cyrillic_is_measured() {
        assert!(width("Выручка", 10.0, 400) > 30.0);
    }

    #[test]
    fn other_fonts_measure_differently() {
        // a monospace system font is wider than Inter for narrow letters
        let inter = width_f("iiiiiiiiii", &Font::inter(10.0, 400));
        let mono = Font { family: "Courier New".into(), ..Font::inter(10.0, 400) };
        let m = width_f("iiiiiiiiii", &mono);
        if fonts::families().iter().any(|f| f.family == "Courier New") {
            assert!(m > inter * 1.5, "mono {m} vs inter {inter}");
        }
        let caps = Font { upper: true, ..Font::inter(10.0, 400) };
        assert_eq!(block("abc", 100.0, &caps).lines[0].text, "ABC");
    }
}
