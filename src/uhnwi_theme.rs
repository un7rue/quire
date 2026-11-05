//! Private-banking themes for UHNWI reports.
//!
//! All of them share one set of design views (same ids), written with theme
//! colour references (`$good`, `$accent`, `$dividend`…), so applying any of
//! them to a report restyles every widget. Each theme is a *look*: paper,
//! ink, accent, the performance and category colours, and its typefaces.
//!
//! - **Quire** — the house dashboard: white paper, grey cards, blue first, pastel marks, IBM Plex Sans.
//! - **Modern UHNWI** — warm paper, Spectral + Manrope, deep crimson accent.
//! - **Alpine** — Swiss restraint: white paper, graphite, one signal red, IBM Plex Sans only.
//! - **Сбер**, **Сбер Private Banking** — the banks' own sites, in the client's typeface when the build has it.
//! - **Изумруд**, **Инфографика** — drawn from the reference designs in resources/examples (an investment PDF, a
//!   flat budget infographic); **Bold Blocks** and **Data Atlas** from brand panels and a data-journalism treemap.

use crate::model::*;
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;

// Colour references the sample and the views use; each theme fills them in.
pub const PAPER: &str = "$page";
pub const CARD: &str = "$card";
pub const CARD2: &str = "$card2";
pub const INK: &str = "$ink";
pub const INK2: &str = "$ink2";
pub const BODY: &str = "$body";
pub const SOFT: &str = "$muted";
pub const LINE: &str = "$grid";
pub const LINE2: &str = "$cardBorder";
pub const ACCENT_LINE: &str = "$accentLine";
pub const ACCENT: &str = "$accent";
pub const GAIN: &str = "$good";
/// Losses, the worst rows, warnings: the theme's "bad" colour (its accent may be green or gold).
pub const LOSS: &str = "$bad";
pub const GAIN_TINT: &str = "$gainTint";
pub const LOSS_TINT: &str = "$lossTint";
pub const TINT_RECEIVED: &str = "$received";
pub const INTEREST: &str = "$interest";
pub const DIVIDEND: &str = "$dividend";
pub const COUPON: &str = "$coupon";
pub const INCOME: &str = "$income";
pub const PRINCIPAL: &str = "$principal";
pub const ICON_INK: &str = "$iconInk";
pub const CHIP: &str = "$chip";
pub const SHARE: &str = "$share";
pub const FORECAST: &str = "$forecast";
pub const HEAD_FILL: &str = "$headFill";
pub const HEAD_INK: &str = "$headInk";
pub const ZEBRA: &str = "$zebra";

/// What makes one theme different from another.
/// A theme's shape language: how marks, grids, donuts, labels and cards are drawn.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    /// Bar corners (0 square, 99 capsule) and whether both ends round (or only the data end).
    pub bar_radius: f64,
    pub round_all: bool,
    /// Lighter fill (0 solid … 1 white) and a darker outline; or bars fading to the axis.
    pub tint: f64,
    pub outline: bool,
    pub gradient: bool,
    /// Bar thickness as a share of its slot, and its cap in points.
    pub bar_width: f64,
    pub max_bar: f64,
    /// Gridlines: none, solid, dashed or dotted; and the value axis' labels.
    pub grid: &'static str,
    pub axis: bool,
    pub line_width: f64,
    /// straight or smooth
    pub curve: &'static str,
    /// none, last or all; hollow or solid
    pub markers: &'static str,
    pub marker_fill: &'static str,
    /// Under a lone line: none, flat or gradient; guide lines from points to the axis.
    pub area: &'static str,
    pub guides: bool,
    /// Donut hole for a standard ring (0.6 = as designed) and gap between slices (1.5 = as designed).
    pub donut: f64,
    pub slice_gap: f64,
    /// Value labels: none (plain), box or pill.
    pub labels: &'static str,
    /// Cards: outlined, filled, shadow (raised) or plain.
    pub cards: &'static str,
    /// Weight of big numbers (KPI values).
    pub kpi_weight: u16,
    /// Every chart, table and KPI card on a card (headline numbers, page headings and tile
    /// groups, whose tiles are framed already, stay bare).
    pub cards_all: bool,
    /// Inner padding of cards in points (10 is the standard).
    pub card_pad: f64,
    /// Treemap tiles: how strong their colour is (0.24 = a light tint, 1 = the colour itself) and the gap between them (pt).
    pub tile_fill: f64,
    pub tile_gap: f64,
    /// Treemap labels: "split" (name on top, the value large at the bottom) or "top" (together, top left).
    pub tile_labels: &'static str,
    /// KPI cards that colour their figure by its sign also sit on a soft fill of their result (gain or loss), with
    /// its arrow: accents that come from the figures themselves.
    pub kpi_tone: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub name: &'static str,
    pub page: &'static str,
    pub card: &'static str,
    pub card2: &'static str,
    pub ink: &'static str,
    pub ink2: &'static str,
    pub body: &'static str,
    pub soft: &'static str,
    pub line: &'static str,
    pub line2: &'static str,
    pub accent_line: &'static str,
    pub accent: &'static str,
    pub good: &'static str,
    pub bad: &'static str,
    pub gain_tint: &'static str,
    pub loss_tint: &'static str,
    pub gain_mid: &'static str,
    pub loss_mid: &'static str,
    pub received: &'static str,
    pub icon_ink: &'static str,
    pub chip: &'static str,
    pub share: &'static str,
    pub forecast: &'static str,
    /// interest, dividend, coupon, income, principal
    pub cats: [&'static str; 5],
    /// soft steps of dividend, coupon, income (forecast tints)
    pub cats_soft: [&'static str; 3],
    /// asset classes, in the order of the overview treemap
    pub classes: [&'static str; 6],
    pub palette: [&'static str; 8],
    pub donut_base: &'static str,
    /// Display face (headings) and its weight, and the UI face (everything else).
    pub display: &'static str,
    pub display_weight: u16,
    pub display_size: f64,
    pub ui: &'static str,
    /// Weight of widget titles (capitals): heavy for most, lighter for a restrained look.
    pub title_weight: u16,
    pub radius: f64,
    pub frame: &'static str,
    /// Widget titles and section headings in capitals (false: as written).
    pub title_upper: bool,
    /// Table headers on a filled band (empty: none) and their text colour on it.
    pub head_fill: &'static str,
    pub head_ink: &'static str,
    /// Alternate table rows on this tint (empty: none).
    pub zebra: &'static str,
    /// Strong rules: under table headers, page dividers.
    pub rule: &'static str,
    /// Letter spacing of the display face (em; negative = tighter, as heavy headlines often are).
    pub display_tracking: f64,
    /// A house's colour blocks: cards of a view set to `fillCycle` take these in turn on a page
    /// (empty: none). Text on a block is the ink, or the page colour where the ink wouldn't read.
    pub blocks: [&'static str; 4],
    /// The cover's and the closing page's paper ("" = the page colour).
    pub cover_fill: &'static str,
    pub back_fill: &'static str,
    /// Frame width of cards (0.75 is a hairline).
    pub card_border_w: f64,
    pub shape: Shape,
}

/// The soft look the template was designed in: rounded tinted bars with outlines.
const SOFT_SHAPE: Shape = Shape {
    bar_radius: 4.0, round_all: true, tint: 0.45, outline: true, gradient: false, bar_width: 0.7, max_bar: 34.0,
    grid: "solid", axis: true, line_width: 2.0, curve: "straight", markers: "all", marker_fill: "hollow", area: "none", guides: false,
    donut: 0.6, slice_gap: 1.5, labels: "none", cards: "outlined", kpi_weight: 800, cards_all: false, card_pad: 10.0, tile_fill: 0.24, tile_gap: 2.0, tile_labels: "split", kpi_tone: false,
};

pub const UHNWI: Look = Look {
    name: "Modern UHNWI",
    page: "#f5f0e6", card: "#efe7d8", card2: "#e9e1d1", ink: "#201d18", ink2: "#4a4439", body: "#6b6254", soft: "#9a8f7c",
    line: "#e4dccb", line2: "#e0d6c2", accent_line: "#c0b189", accent: "#a12a2a", good: "#166f5e", bad: "#a12a2a",
    gain_tint: "#e6efe3", loss_tint: "#f4e4da", gain_mid: "#5f9a85", loss_mid: "#c7726c", received: "#efe8d6", icon_ink: "#3f382b",
    chip: "#ece3d2", share: "#a99069", forecast: "#a08a63",
    cats: ["#6f675c", "#a67f2e", "#6e5c41", "#8a6a45", "#6f839a"], cats_soft: ["#cdb78a", "#a59a85", "#c9ac7a"],
    classes: ["#c98a6b", "#7d8794", "#c9a46a", "#e8d5ad", "#b9c4c9", "#7aa6b0"],
    palette: ["#201d18", "#a12a2a", "#c98a6b", "#a67f2e", "#e3cba0", "#6e5c41", "#6f839a", "#7cb6a1"],
    donut_base: "#b8a283", display: "Spectral", display_weight: 300, display_size: 25.5, ui: "Manrope", title_weight: 800, radius: 9.0, frame: "#ddccae",
    title_upper: true, head_fill: "", head_ink: "", zebra: "", rule: "#201d18", card_border_w: 0.75, shape: SOFT_SHAPE,
    display_tracking: 0.0, blocks: ["", "", "", ""], cover_fill: "", back_fill: "",
};

/// Swiss restraint: white paper, graphite and one signal red.
pub const ALPINE: Look = Look {
    name: "Alpine",
    page: "#ffffff", card: "#f5f5f3", card2: "#ececea", ink: "#121212", ink2: "#2e2e2e", body: "#555555", soft: "#8d8d8d",
    line: "#e6e6e4", line2: "#dcdcda", accent_line: "#121212", accent: "#d0021b", good: "#1d6b45", bad: "#d0021b",
    gain_tint: "#e8f1ec", loss_tint: "#fbe7e9", gain_mid: "#6c9c84", loss_mid: "#e0707e", received: "#f2f2f0", icon_ink: "#121212",
    chip: "#ededeb", share: "#9a9a9a", forecast: "#7a7a7a",
    cats: ["#4d4d4d", "#9c7a3c", "#6d6d6d", "#3a3a3a", "#4f6d8a"], cats_soft: ["#c9b48c", "#b0b0b0", "#a0a0a0"],
    classes: ["#d0021b", "#3a3a3a", "#9a9a9a", "#d6d6d4", "#bcc6cf", "#6f8597"],
    palette: ["#121212", "#d0021b", "#6f8597", "#9a9a9a", "#c9b48c", "#3a3a3a", "#1d6b45", "#bcc6cf"],
    donut_base: "#8d8d8d", display: "IBM Plex Sans", display_weight: 300, display_size: 24.0, ui: "IBM Plex Sans", title_weight: 800, radius: 3.0, frame: "#121212",
    rule: "#121212", shape: Shape { bar_radius: 0.0, round_all: false, tint: 0.0, outline: false, gradient: false, bar_width: 0.5, max_bar: 22.0, grid: "solid", axis: true, line_width: 1.25, curve: "straight", markers: "none", marker_fill: "solid", area: "none", guides: false, donut: 0.8, slice_gap: 0.5, labels: "none", cards: "plain", kpi_weight: 500, cards_all: false, card_pad: 10.0, tile_fill: 0.24, tile_gap: 2.0, tile_labels: "split", kpi_tone: false }, ..UHNWI
};

/// A theme's typeface from ui/fonts/private/families.txt (see build.rs): the display (`true`)
/// or text family for theme `id`, else `fallback`.
const fn private_type(id: &str, display: bool, fallback: &'static str) -> &'static str {
    const fn same(a: &str, b: &str) -> bool {
        let (a, b) = (a.as_bytes(), b.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        let mut i = 0;
        while i < a.len() {
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }
        true
    }
    let fams = crate::text::private::FAMILIES;
    let mut i = 0;
    while i < fams.len() {
        if same(fams[i].0, id) {
            return if display { fams[i].1 } else { fams[i].2 };
        }
        i += 1;
    }
    fallback
}

/// Сбер: pale blue paper, white cards with large corners, bars in teal-green fading to the
/// axis (last year's in pale blue), thin rings, guide lines under points, light big numbers,
/// headings as written, a green rule under table headers. The client's typeface when the
/// build has it (`private_type`), else IBM Plex Sans (it has the light weight the headings use) and Inter.
pub const GREEN: Look = Look {
    name: "Сбер",
    page: "#eef3fb", card: "#ffffff", card2: "#f4f7fc", ink: "#141414", ink2: "#2b2f33", body: "#5c6670", soft: "#8a96a3",
    line: "#e3e9f2", line2: "#d3e2eb", accent_line: "#21a038", accent: "#21a038", good: "#1a9e32", bad: "#d92b3a",
    gain_tint: "#e1f6e8", loss_tint: "#fde4e6", gain_mid: "#5fd08f", loss_mid: "#f07b86", received: "#e8f6ee", icon_ink: "#2b2f33",
    chip: "#edf2f9", share: "#3cc19a", forecast: "#21a038",
    cats: ["#3cc19a", "#21a038", "#9be15d", "#1f8c7e", "#b9d7e3"], cats_soft: ["#a6e6b6", "#d7f5b6", "#a8d8cf"],
    classes: ["#3cc19a", "#5be07c", "#c3f36e", "#1f8c7e", "#7ae3f5", "#cfe3ec"],
    palette: ["#3cc19a", "#cfe3ec", "#5be07c", "#c3f36e", "#1f8c7e", "#7ae3f5", "#21a038", "#9fb8cf"],
    donut_base: "#3cc19a", display: private_type("green", true, "IBM Plex Sans"), display_weight: 300, display_size: 24.0, ui: private_type("green", false, "Inter"), title_weight: 600, radius: 14.0, frame: "#21a038",
    title_upper: false, head_fill: "", head_ink: "", zebra: "", rule: "#21a038", card_border_w: 0.75,
    shape: Shape { bar_radius: 6.0, round_all: false, tint: 0.0, outline: false, gradient: true, bar_width: 0.5, max_bar: 30.0, grid: "none", axis: false, line_width: 2.0, curve: "straight", markers: "all", marker_fill: "hollow", area: "none", guides: true, donut: 0.86, slice_gap: 0.0, labels: "none", cards: "outlined", kpi_weight: 700, cards_all: true, card_pad: 12.0, tile_fill: 0.24, tile_gap: 2.0, tile_labels: "split", kpi_tone: false },
    display_tracking: 0.0, blocks: ["", "", "", ""], cover_fill: "", back_fill: "",
};

/// Сбер Private Banking: graphite and steel blue-greys, copper accent, gold and copper
/// charts, light display headings, on light paper for print. Typefaces as Сбер's.
/// Built like Сбер: every figure on a card in a pale steel frame, bold numbers, headings as
/// written, a copper rule under table headers.
pub const GRAPHITE: Look = Look {
    name: "Сбер Private Banking",
    page: "#f5f6f7", card: "#ffffff", card2: "#edeeef", ink: "#111111", ink2: "#283038", body: "#485966", soft: "#8c959c",
    line: "#dae0e6", line2: "#d3dce3", accent_line: "#dd6120", accent: "#dd6120", good: "#25a481", bad: "#e0414f",
    gain_tint: "#e2f3ee", loss_tint: "#fce4e6", gain_mid: "#6cc2a8", loss_mid: "#f28c95", received: "#eef1f4", icon_ink: "#283038",
    chip: "#edf0f2", share: "#94a6b3", forecast: "#e88e44",
    cats: ["#566a7a", "#dd6120", "#b8941a", "#333f48", "#94a6b3"], cats_soft: ["#efaf7a", "#e8d58a", "#bec9d1"],
    classes: ["#333f48", "#dd6120", "#94a6b3", "#e88e44", "#c5cfd6", "#dab527"],
    palette: ["#283038", "#dd6120", "#5d7384", "#e88e44", "#94a6b3", "#dab527", "#25a481", "#c5cfd6"],
    donut_base: "#5d7384", display: private_type("graphite", true, "IBM Plex Sans"), display_weight: 300, display_size: 24.0, ui: private_type("graphite", false, "Inter"), title_weight: 600, radius: 12.0, frame: "#dd6120",
    title_upper: false, card_border_w: 0.75, rule: "#dd6120", shape: Shape { bar_radius: 1.5, round_all: false, tint: 0.0, outline: false, gradient: false, bar_width: 0.5, max_bar: 24.0, grid: "dashed", axis: true, line_width: 1.75, curve: "smooth", markers: "last", marker_fill: "solid", area: "gradient", guides: false, donut: 0.8, slice_gap: 1.0, labels: "none", cards: "outlined", kpi_weight: 700, cards_all: true, card_pad: 12.0, tile_fill: 0.24, tile_gap: 2.0, tile_labels: "split", kpi_tone: false }, ..UHNWI
};

/// Investment report (reference PDF): white paper, emerald green and dark slate, tables
/// with green header bands and zebra rows, Raleway, headings as written.
pub const EMERALD: Look = Look {
    name: "Изумруд",
    page: "#ffffff", card: "#f5f7f6", card2: "#edf1ef", ink: "#2b3a42", ink2: "#34444c", body: "#56646b", soft: "#8a969c",
    line: "#e6eae8", line2: "#d9dfdc", accent_line: "#10b96d", accent: "#10b96d", good: "#0e9f5e", bad: "#d64545",
    gain_tint: "#dff5ea", loss_tint: "#fbe3e3", gain_mid: "#6fd1a2", loss_mid: "#ec9090", received: "#eaf7f0", icon_ink: "#2b3a42",
    chip: "#eef2f0", share: "#10b96d", forecast: "#2b3a42",
    cats: ["#10b96d", "#2b3a42", "#5fa8d3", "#7e8c93", "#0b7f4b"], cats_soft: ["#a9b5bb", "#b9dcf0", "#c9d1d5"],
    classes: ["#10b96d", "#2b3a42", "#5fa8d3", "#9adfbe", "#aeb8bd", "#0b7f4b"],
    palette: ["#2b3a42", "#10b96d", "#5fa8d3", "#0b7f4b", "#9adfbe", "#7e8c93", "#b9dcf0", "#c7d0d4"],
    donut_base: "#10b96d", display: "Raleway", display_weight: 700, display_size: 24.0, ui: "Raleway", title_weight: 700, radius: 8.0, frame: "#10b96d",
    title_upper: false, head_fill: "#10b96d", head_ink: "#ffffff", zebra: "#f4f6f5", rule: "#2b3a42", card_border_w: 1.2,
    shape: Shape { bar_radius: 0.0, round_all: false, tint: 0.0, outline: false, gradient: false, bar_width: 0.7, max_bar: 40.0, grid: "solid", axis: true, line_width: 1.5, curve: "straight", markers: "none", marker_fill: "solid", area: "none", guides: false, donut: 0.6, slice_gap: 1.0, labels: "box", cards: "outlined", kpi_weight: 700, cards_all: false, card_pad: 10.0, tile_fill: 0.24, tile_gap: 2.0, tile_labels: "split", kpi_tone: false },
    display_tracking: 0.0, blocks: ["", "", "", ""], cover_fill: "", back_fill: "",
};

/// Flat infographic (reference "Budget infographic"): white paper, navy ink, teal,
/// sunflower yellow, coral and blue, Montserrat, bold capital titles, teal rules.
pub const FLAT: Look = Look {
    name: "Инфографика",
    page: "#ffffff", card: "#f2f7f9", card2: "#e6f0f4", ink: "#0e3b5c", ink2: "#1c4a6b", body: "#4a6378", soft: "#8798a6",
    line: "#e4ecf1", line2: "#d3e0e8", accent_line: "#f5b91c", accent: "#13a3b4", good: "#109aaa", bad: "#e84a5f",
    gain_tint: "#ddf2f4", loss_tint: "#fce2e5", gain_mid: "#6fcad3", loss_mid: "#f29aa6", received: "#fff6da", icon_ink: "#0e3b5c",
    chip: "#eaf3f6", share: "#13a3b4", forecast: "#f5b91c",
    cats: ["#13a3b4", "#f5b91c", "#0e4c75", "#e84a5f", "#2f7bc2"], cats_soft: ["#fbe0a0", "#9db8cc", "#f6b3bc"],
    classes: ["#13a3b4", "#0e4c75", "#f5b91c", "#e84a5f", "#2f7bc2", "#9fdce3"],
    palette: ["#0e3b5c", "#13a3b4", "#f5b91c", "#e84a5f", "#2f7bc2", "#0e4c75", "#9fdce3", "#fbe0a0"],
    donut_base: "#13a3b4", display: "Montserrat", display_weight: 800, display_size: 20.0, ui: "Montserrat", title_weight: 800, radius: 6.0, frame: "#13a3b4",
    title_upper: true, head_fill: "", head_ink: "", zebra: "", rule: "#13a3b4", card_border_w: 0.75,
    shape: Shape { bar_radius: 99.0, round_all: true, tint: 0.0, outline: false, gradient: false, bar_width: 0.5, max_bar: 22.0, grid: "none", axis: false, line_width: 3.0, curve: "smooth", markers: "all", marker_fill: "solid", area: "flat", guides: false, donut: 0.5, slice_gap: 3.0, labels: "none", cards: "filled", kpi_weight: 800, cards_all: false, card_pad: 10.0, tile_fill: 0.24, tile_gap: 2.0, tile_labels: "split", kpi_tone: false },
    display_tracking: 0.0, blocks: ["", "", "", ""], cover_fill: "", back_fill: "",
};

pub const LOOKS: [(&str, &Look); 9] = [("quire", &QUIRE), ("uhnwi", &UHNWI), ("alpine", &ALPINE), ("green", &GREEN), ("graphite", &GRAPHITE),
    ("emerald", &EMERALD), ("flat", &FLAT), ("blocks", &BLOCKS), ("atlas", &ATLAS)];

/// Quire, the house look: a dashboard's. White paper; cards on a soft cool grey with a hairline; IBM Plex Sans
/// throughout — light page headings, bold figures, even figures in tables. Blue leads (the first series, shares, the
/// donut, accents) through teal, lavender, amber, slate and sky; red is only for what is negative. Marks are pastel
/// (columns a light tint of their colour, their figures at full strength); KPI cards that colour a result by its sign
/// sit on a soft fill of it — green or red — so the page's accents come from its figures. Asset-class tiles in pale
/// tints, smooth lines on a soft fade, a thin donut ring.
pub const QUIRE: Look = Look {
    name: "Quire",
    page: "#ffffff", card: "#f5f7fa", card2: "#eaeef4", ink: "#262b35", ink2: "#3a404c", body: "#5b6170", soft: "#8f95a3",
    line: "#eceef2", line2: "#e1e5ec", accent_line: "#4a72c2", accent: "#4a72c2", good: "#16896f", bad: "#c23d4f",
    gain_tint: "#e1f3ee", loss_tint: "#fbe4e7", gain_mid: "#7fd1bc", loss_mid: "#eda3ae", received: "#edf2fb", icon_ink: "#5b6170",
    chip: "#eaeef4", share: "#4a72c2", forecast: "#7f9ad6",
    cats: ["#4a72c2", "#3fb59a", "#e9a85b", "#8f7ad6", "#8f95a3"], cats_soft: ["#aee0d4", "#f6d6ad", "#d3c9f1"],
    classes: ["#3fb59a", "#4a72c2", "#8f7ad6", "#e9b84f", "#6fa8dc", "#9aa3b2"],
    palette: ["#4a72c2", "#3fb59a", "#8f7ad6", "#e9b84f", "#5d6b82", "#6fb6dc", "#e98f5b", "#b8bfcc"],
    donut_base: "#4a72c2", display: "IBM Plex Sans", display_weight: 300, display_size: 24.0, ui: "IBM Plex Sans", title_weight: 600, radius: 8.0, frame: "#e1e5ec",
    title_upper: false, head_fill: "", head_ink: "", zebra: "", rule: "#262b35", card_border_w: 0.75,
    shape: Shape { bar_radius: 2.5, round_all: false, tint: 0.35, outline: false, gradient: false, bar_width: 0.6, max_bar: 26.0, grid: "solid", axis: true, line_width: 2.0, curve: "smooth", markers: "all", marker_fill: "hollow", area: "gradient", guides: false, donut: 0.86, slice_gap: 1.5, labels: "none", cards: "outlined", kpi_weight: 700, cards_all: false, card_pad: 10.0, tile_fill: 0.4, tile_gap: 3.0, tile_labels: "top", kpi_tone: true },
    display_tracking: -0.01, blocks: ["", "", "", ""], cover_fill: "", back_fill: "",
};

/// Data atlas: the data-journalism infographic — a heavy condensed headline, quiet condensed labels,
/// flat muted pastels (teal, rose, butter, sage, amber) as solid tiles split by white gutters,
/// square corners, no grid, no frames, and one strong red kept for what must stand out.
pub const ATLAS: Look = Look {
    name: "Data Atlas",
    page: "#ffffff", card: "#ffffff", card2: "#f4f4f1", ink: "#1a1a1a", ink2: "#2a2a2a", body: "#3d3d3d", soft: "#6b6b6b",
    line: "#e6e6e2", line2: "#cfcfca", accent_line: "#c8102e", accent: "#c8102e", good: "#2e8668", bad: "#c8102e",
    gain_tint: "#dcefe6", loss_tint: "#f8dadd", gain_mid: "#7fbf9f", loss_mid: "#e57a88", received: "#fbf0c4", icon_ink: "#1a1a1a",
    chip: "#efefeb", share: "#7fc4c3", forecast: "#f4d86a",
    cats: ["#8ecccb", "#f2d264", "#ecbcc0", "#b9cdb5", "#eaae4c"], cats_soft: ["#f8e8a8", "#f6dde0", "#dbe5d8"],
    classes: ["#a9d3d2", "#efc6c9", "#f4d86a", "#bfd0bc", "#edb34f", "#dcd68e"],
    palette: ["#8ecccb", "#ecbcc0", "#f2d264", "#b9cdb5", "#eaae4c", "#d8d189", "#97c1aa", "#c8102e"],
    donut_base: "#8ecccb", display: "Roboto Condensed", display_weight: 800, display_size: 23.0, ui: "Roboto Condensed", title_weight: 700, radius: 0.0, frame: "#1a1a1a",
    title_upper: false, head_fill: "", head_ink: "", zebra: "", rule: "#1a1a1a", card_border_w: 0.0,
    shape: Shape { bar_radius: 0.0, round_all: false, tint: 0.0, outline: false, gradient: false, bar_width: 0.72, max_bar: 34.0, grid: "none", axis: true, line_width: 2.0, curve: "straight", markers: "last", marker_fill: "solid", area: "none", guides: false, donut: 0.55, slice_gap: 2.5, labels: "none", cards: "plain", kpi_weight: 700, cards_all: false, card_pad: 10.0, tile_fill: 1.0, tile_gap: 3.0, tile_labels: "top", kpi_tone: false },
    display_tracking: -0.01, blocks: ["", "", "", ""], cover_fill: "", back_fill: "",
};

/// Bold blocks: white paper, black type at its heaviest, and figures on flat colour blocks
/// (yellow, orange, pink, black) with square corners — the language of houses whose reports set
/// numbers on solid brand panels; black table headers, thin rings, a yellow closing page.
pub const BLOCKS: Look = Look {
    name: "Bold Blocks",
    page: "#ffffff", card: "#ffffff", card2: "#f2f2f2", ink: "#000000", ink2: "#1a1a1a", body: "#2b2b2b", soft: "#6b6b6b",
    line: "#d9d9d9", line2: "#000000", accent_line: "#ffd800", accent: "#eb6027", good: "#00926c", bad: "#e4042a",
    gain_tint: "#d9f0e8", loss_tint: "#fbe0e3", gain_mid: "#5bb89a", loss_mid: "#ef6b7b", received: "#fff3b0", icon_ink: "#000000",
    chip: "#f2f2f2", share: "#eb6027", forecast: "#ffe766",
    cats: ["#ffd800", "#eb6027", "#9468a9", "#00926c", "#7c7b7f"], cats_soft: ["#f7b4a3", "#d2c1e0", "#a8dcc9"],
    classes: ["#ffd800", "#eb6027", "#f3a4bb", "#9468a9", "#00926c", "#e4042a"],
    palette: ["#000000", "#ffd800", "#eb6027", "#f3a4bb", "#9468a9", "#00926c", "#e4042a", "#7c7b7f"],
    donut_base: "#ffd800", display: "Montserrat", display_weight: 900, display_size: 21.0, ui: "Inter", title_weight: 900, radius: 0.0, frame: "#000000",
    title_upper: false, head_fill: "#000000", head_ink: "#ffffff", zebra: "", rule: "#000000", card_border_w: 0.0,
    shape: Shape { bar_radius: 0.0, round_all: false, tint: 0.0, outline: false, gradient: false, bar_width: 0.6, max_bar: 30.0, grid: "none", axis: true, line_width: 2.0, curve: "straight", markers: "none", marker_fill: "solid", area: "none", guides: false, donut: 0.86, slice_gap: 1.0, labels: "box", cards: "plain", kpi_weight: 900, cards_all: false, card_pad: 12.0, tile_fill: 0.24, tile_gap: 2.0, tile_labels: "split", kpi_tone: false },
    display_tracking: -0.02, blocks: ["#ffd800", "#eb6027", "#f3a4bb", "#000000"], cover_fill: "", back_fill: "#ffd800",
};

fn v(id: &str, name: &str, description: &str, sample: &str, opts: Json, style: Json) -> StyleVariant {
    StyleVariant { id: id.into(), name: name.into(), description: description.into(), sample: sample.into(), opts, style }
}

/// Small caps label used by KPI cards and card titles.
fn caps(size: f64) -> Json {
    json!({"size": size, "weight": 700, "color": SOFT, "tracking": 0.1, "upper": true})
}

fn kpi_viz(extra: Json) -> Json {
    let mut o = json!({"viz": {"spark": "none", "delta": false, "compact": "off"}});
    merge_json(&mut o, &json!({"viz": extra}));
    o
}

/// Looks that became settings of another: (kind, old view, the view it folds into).
/// What set them apart (no header, colour by result, a card…) is now the
/// widget's own setting, which report users can switch themselves.
pub const FOLDED: [(&str, &str, &str); 17] = [
    ("kpi", "hero-signed", "hero"), ("kpi", "value-right", "hero"),
    ("kpi", "card-result", "card"), ("kpi", "card-toned", "card"), ("kpi", "card-solid", "card"), ("kpi", "icon", "breakdown"),
    ("table", "list", "lines"), ("table", "leaders", "lines"), ("table", "card", "lines"),
    ("bar", "monthly-titled", "monthly"), ("bar", "share-rows", "share"), ("bar", "split", "share"),
    ("pie", "classes", "top"), ("pie", "inflow", "ring"),
    ("heading", "eyebrow", "page"), ("heading", "section-accent", "section"),
    ("text", "small", "body"),
];

/// The merge patch that turns `from` into `to` (RFC 7386: a removed key is null).
pub fn json_diff(from: &Json, to: &Json) -> Json {
    match (from, to) {
        (Json::Object(a), Json::Object(b)) => {
            let mut p = serde_json::Map::new();
            for (k, v) in b {
                match a.get(k) {
                    Some(av) if av == v => {}
                    Some(av) if av.is_object() && v.is_object() => {
                        let d = json_diff(av, v);
                        if d.as_object().is_some_and(|o| !o.is_empty()) {
                            p.insert(k.clone(), d);
                        }
                    }
                    _ => {
                        p.insert(k.clone(), v.clone());
                    }
                }
            }
            for k in a.keys().filter(|k| !b.contains_key(*k)) {
                p.insert(k.clone(), Json::Null);
            }
            Json::Object(p)
        }
        _ => to.clone(),
    }
}

/// Moves widgets off folded looks onto the look they fold into, keeping what
/// set the old look apart as the widget's own settings: each draws as before.
/// The folded looks then leave the document's theme. Returns how many moved.
pub fn fold_views(doc: &mut crate::model::Doc) -> usize {
    let look = LOOKS.iter().find(|(_, l)| l.name == doc.theme.name).map(|(_, l)| *l).unwrap_or(&UHNWI);
    let all = styles(look);
    let find = |th: &Theme, kind: &str, id: &str| th.variant(kind, id).cloned().or_else(|| all.get(kind).and_then(|l| l.iter().find(|v| v.id == id).cloned()));
    let theme = doc.theme.clone();
    // only a theme with the kept looks takes part (a theme of one's own keeps its views)
    let folds: Vec<(&str, &str, StyleVariant, StyleVariant)> = FOLDED.iter()
        .filter(|(k, _, new)| theme.variant(k, new).is_some())
        .filter_map(|(k, old, new)| Some((*k, *old, find(&theme, k, old)?, find(&theme, k, new)?)))
        .collect();
    let mut moved = 0;
    let widgets = doc.pages.iter_mut().flat_map(|p| p.widgets.iter_mut()).chain(doc.catalog.iter_mut().map(|c| &mut c.widget));
    for w in widgets {
        if let Some((_, _, o, n)) = folds.iter().find(|(k, old, ..)| *k == w.kind && *old == w.variant) {
            for (own, from, to) in [(&mut w.opts, &n.opts, &o.opts), (&mut w.style, &n.style, &o.style)] {
                let mut p = json_diff(&obj(from), &obj(to));
                merge_keep_nulls(&mut p, own);
                *own = p;
            }
            w.variant = n.id.clone();
            moved += 1;
        }
    }
    for (k, old, ..) in &folds {
        if let Some(l) = doc.theme.styles.get_mut(*k) {
            l.retain(|v| v.id != *old);
        }
    }
    moved
}

fn obj(v: &Json) -> Json {
    if v.is_object() { v.clone() } else { json!({}) }
}

/// `own` over `base`, objects merged, keeping `base`'s nulls where `own` says nothing.
fn merge_keep_nulls(base: &mut Json, own: &Json) {
    match (base.as_object_mut(), own.as_object()) {
        (Some(b), Some(o)) => {
            for (k, v) in o {
                match b.get_mut(k) {
                    Some(bv) if bv.is_object() && v.is_object() => merge_keep_nulls(bv, v),
                    _ => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        _ => *base = own.clone(),
    }
}

/// The design views every UHNWI theme shares (fonts follow the look).
fn styles(l: &Look) -> BTreeMap<String, Vec<StyleVariant>> {
    let ui = l.ui;
    let display = l.display;
    let card_fonts = |kpi: f64| json!({"title": caps(5.8), "kpi": {"size": kpi, "weight": 800}, "caption": {"size": 6.2, "weight": 500, "color": BODY}});
    // table headers: a filled band, or text over a strong rule
    let band = !l.head_fill.is_empty();
    let head = |line: &str| -> Json {
        let mut t = if band { json!({"header": {"fill": HEAD_FILL, "color": HEAD_INK}, "headLineColor": HEAD_FILL}) } else { json!({"headLineColor": line}) };
        if !l.zebra.is_empty() {
            merge_json(&mut t, &json!({"zebra": true, "zebraFill": ZEBRA}));
        }
        t
    };
    let tbl = |base: Json, line: &str| -> Json {
        let mut o = base;
        merge_json(&mut o, &json!({"table": head(line)}));
        o
    };
    let upper = l.title_upper;
    let mut m = BTreeMap::new();
    m.insert("kpi".into(), vec![
        v("hero", "Главная цифра", "Крупное значение без карточки, подпись капителью", "kpi", kpi_viz(json!({})),
            json!({"card": false, "fonts": {"title": {"size": 6.6, "weight": 700, "color": SOFT, "tracking": 0.12, "upper": true}, "kpi": {"size": 40, "weight": 800}, "caption": {"size": 7.6, "weight": 500, "color": BODY}}})),
        v("hero-signed", "Главная цифра · результат", "Зелёный или красный по знаку", "kpi", kpi_viz(json!({"valueColor": "sign", "signed": true})),
            json!({"card": false, "fonts": {"title": {"size": 6.6, "weight": 700, "color": SOFT, "tracking": 0.12, "upper": true}, "kpi": {"size": 40, "weight": 800}, "caption": {"size": 7.6, "weight": 500, "color": BODY}}})),
        v("value-right", "Стоимость справа", "Сумма счёта у правого края страницы", "kpi", json!({"align": "right", "viz": {"spark": "none", "delta": false, "compact": "off", "kpiAlign": "right"}}),
            json!({"card": false, "fonts": {"title": {"size": 6, "weight": 700, "color": SOFT, "tracking": 0.12, "upper": true}, "kpi": {"size": 20, "weight": 800}}})),
        v("card", "Карточка", "Подпись, значение, пояснение", "kpi", kpi_viz(json!({})), json!({"card": true, "fonts": card_fonts(13.0)})),
        v("card-toned", "Карточка по знаку", "Фон и стрелка по результату", "kpi-pct", kpi_viz(json!({"tone": "sign", "arrow": true, "valueColor": "sign", "signed": true, "decimals": 1})), json!({"card": true, "fonts": card_fonts(13.0)})),
        v("card-result", "Карточка результата", "Значение зелёным или красным", "kpi", kpi_viz(json!({"valueColor": "sign", "signed": true})), json!({"card": true, "fonts": card_fonts(13.0)})),
        v("card-solid", "Акцентная карточка", "Залита цветом результата", "kpi-pct", kpi_viz(json!({"tone": "solid", "arrow": true, "signed": true, "decimals": 2})), json!({"card": true, "fonts": card_fonts(13.0)})),
        v("breakdown", "Карточка с разбивкой", "Значение и строки: пополнения, выводы…", "kpi", kpi_viz(json!({"valueColor": "sign", "signed": true, "breakdown": true})), json!({"card": true, "fonts": card_fonts(15.0)})),
        v("icon", "Карточка с иконкой", "Иконка в заголовке и строки с деталями", "kpi", kpi_viz(json!({"valueColor": "sign", "signed": true, "breakdown": true})),
            json!({"card": true, "titleIcon": {"icon": "cash", "shape": "square", "fill": CARD2, "color": ICON_INK, "size": 2.3}, "fonts": card_fonts(13.0)})),
        v("stat", "Показатель с медианой", "Значение, медиана и разница в чипе", "kpi", kpi_viz(json!({"valueColor": "sign", "signed": true, "note": "мед. {1}", "noteColor": "ink", "chip": 2})), json!({"card": true, "fonts": card_fonts(12.5)})),
        v("inline", "Строка-ориентир", "Подпись слева, значение справа", "kpi-pct", kpi_viz(json!({"tone": "good", "valueColor": "sign", "signed": true, "decimals": 2, "inline": true})),
            json!({"card": true, "fonts": {"title": {"size": 7, "weight": 700, "color": GAIN}, "kpi": {"size": 13, "weight": 800}}})),
    ]);
    m.insert("table".into(), vec![
        v("lines", "Таблица", "Строки с тонкими линиями, жирная линия под шапкой", "categories",
            tbl(json!({"totals": true, "table": {"row_h": 22, "lines": "rows", "lineColor": LINE, "headLineWidth": 1.2, "totalLine": "thick", "paginate": false}}), "$rule"), json!({"card": false})),
        v("list", "Список без шапки", "Для коротких перечней", "categories", json!({"totals": false, "table": {"hideHead": true, "row_h": 21, "lines": "rows", "lineColor": LINE, "paginate": false}}), json!({"card": false})),
        v("grouped", "Состав по группам", "Группы капителью, количество, линия", "grouped",
            tbl(json!({"totals": true, "table": {"subtotals": true, "groupFill": "none", "groupColor": INK, "groupUpper": true, "groupCount": true, "groupLine": "$rule", "indent": false,
                "totalFont": {"family": display, "size": 10, "weight": 500}, "totalLine": "thick", "head_h": 26, "row_h": 22, "lines": "rows", "lineColor": LINE}}), LINE),
            json!({"card": false, "fonts": {"table_cell": {"size": 7.6}, "table_head": {"size": 5.6}}})),
        v("leaders", "Лидеры и аутсайдеры", "Без шапки, с иконкой в заголовке", "categories",
            json!({"totals": false, "table": {"hideHead": true, "row_h": 22, "lines": "rows", "lineColor": LINE, "paginate": false}}),
            json!({"card": false, "titleIcon": {"icon": "trend-up", "shape": "circle", "fill": GAIN_TINT, "color": GAIN, "size": 1.9}, "fonts": {"title": {"size": 7, "tracking": 0.08, "color": GAIN}, "table_cell": {"size": 7.4}}})),
        v("matrix", "Матрица", "Колонки по периодам, полосы под числами", "categories", tbl(json!({"totals": false, "table": {"head_h": 24, "row_h": 22, "lines": "rows", "lineColor": LINE, "paginate": false}}), LINE), json!({"card": false})),
        v("card", "Таблица в карточке", "Компактная, на фоне карточки", "categories", json!({"totals": false, "density": "compact", "table": {"row_h": 12.5, "head_h": 12, "lines": "rows", "lineColor": LINE2, "headLineColor": LINE2, "paginate": false}}),
            json!({"card": true, "pad": 7, "background": CARD, "fonts": {"title": {"size": 5.8, "weight": 800, "color": INK, "tracking": 0.1, "upper": true}, "table_cell": {"size": 6.9}, "table_head": {"size": 5.2}}})),
    ]);
    m.insert("bar".into(), vec![
        v("monthly", "По месяцам и итог", "Столбцы по знаку, линия итога, лучший и худший", "combo",
            json!({"viz": {"colorMode": "sign", "labels": "all", "signed": true, "callout": "last", "extremes": true, "legend": "none", "ticks": 3, "units": ""}}),
            json!({"card": false, "pad": 0, "fonts": {"title": {"size": 6.6, "weight": 600, "upper": false, "tracking": 0, "color": BODY}}})),
        v("monthly-titled", "По месяцам · с заголовком", "Тот же график с заголовком раздела и легендой", "combo",
            json!({"viz": {"colorMode": "sign", "labels": "all", "signed": true, "callout": "last", "extremes": true, "legend": "top-right", "ticks": 3, "units": ""}}), json!({"card": false})),
        v("classes", "Результат по классам", "Строки: имя с отметкой лучший/худший, шкала по размеру, значение и пояснение", "signed",
            json!({"orientation": "horizontal", "viz": {"rankStyle": "ranked", "signed": true, "compact": "off", "rankNumbers": false, "rankHighlight": false, "markBest": true, "rowLines": true, "trackFill": true, "trackLines": false, "barBase": "size", "subLine": true}}), json!({"card": false})),
        v("ranked", "Рейтинг", "Строки с номером, шкалой и подсветкой", "ranked", json!({"orientation": "horizontal", "viz": {"rankStyle": "ranked", "signed": true, "decimals": 1}}), json!({"card": false})),
        v("diverging", "Плюс и минус", "Горизонтальные столбцы от нуля", "signed", json!({"orientation": "horizontal", "viz": {"colorMode": "sign", "labels": "all", "signed": true, "tint": 0.6, "outline": true, "barWidth": 0.45, "rankStyle": "bars"}}), json!({"card": false})),
        v("share", "Доли · полоса", "Полоса 100% с легендой", "categories", json!({"orientation": "share", "viz": {"legendShare": true, "shareHeight": 12, "radius": 6, "tint": 0, "outline": false, "decimals": 1}}), json!({"card": false})),
        v("share-rows", "Доли · полоса и строки", "Легенда строками с иконками", "categories", json!({"orientation": "share", "viz": {"legendStyle": "rows", "legendShare": true, "shareHeight": 18, "radius": 6, "tint": 0, "outline": false, "insideLabels": true}}), json!({"card": false})),
        v("share-result", "Доли · результат", "Цвет по результату, подписи внутри", "signed",
            json!({"orientation": "share", "viz": {"colorMode": "sign", "insideLabels": true, "insideValue": "second", "legendText": "{value} · {share} · рез. {2}", "shareHeight": 36, "radius": 5, "tint": 0, "outline": false, "shareGap": 2}}), json!({"card": false})),
        v("split", "Два цвета", "Например, покупки и продажи · подписи внутри", "categories", json!({"orientation": "share", "viz": {"insideLabels": true, "insideValue": "value", "legend": "none", "shareHeight": 12, "radius": 3, "tint": 0, "outline": false}}), json!({"card": false})),

        v("bench", "Шкала · точки с подписями", "Ориентиры кружками, имена под ними, счёт выделен", "bench", json!({"orientation": "scale", "viz": {"signed": true, "decimals": 1, "trackStyle": "gradient", "shareHeight": 4}}), json!({"card": false})),
        // the peer-group card: min and max close the scale, zero splits red from green, "you" stands out
        v("cohort", "Шкала · трек с осью", "Точки на цветном треке, деления оси, в карточке", "bench",
            json!({"orientation": "scale", "subSide": true, "viz": {"signed": true, "decimals": 1, "hlDecimals": 2, "trackStyle": "gradient", "shareHeight": 10, "markerStyle": "dots", "axisTicks": true, "scaleZero": true, "scaleEnds": true}}),
            json!({"card": true, "fonts": {"title": {"size": 7, "weight": 700, "color": SOFT, "upper": true, "tracking": 0.08}, "subtitle": {"size": 9.5, "weight": 800, "color": INK}}})),
    ]);
    m.insert("pie".into(), vec![
        v("top", "Кольцо · главная доля", "Доля крупнейшей категории в центре, легенда-таблица", "categories",
            json!({"viz": {"mono": true, "baseColor": "$donut", "inner": 0.6, "center": "top", "legendStyle": "table", "legendAmount": true, "legendShare": true, "sliceGap": 1.5, "tint": 0, "outline": false, "centerSize": 18}}), json!({"card": false})),
        v("classes", "Кольцо без легенды", "Центр — доля крупнейшей категории", "categories", json!({"viz": {"mono": false, "inner": 0.6, "center": "top", "legendStyle": "none", "tint": 0, "outline": false, "centerSize": 17}}), json!({"card": false})),
        v("contribution", "Вклад в результат", "Оттенки одного цвета, итог в центре", "signed",
            json!({"viz": {"mono": true, "signed": true, "sortSlices": false, "inner": 0.62, "center": "total", "suffix": "%", "decimals": 1, "centerLabel": " ", "centerSize": 13, "legendStyle": "simple", "legendAmount": true, "legendShare": false, "sliceGap": 1.2, "tint": 0, "outline": false}}),
            json!({"card": false, "fonts": {"title": {"size": 6.8, "upper": false, "tracking": 0, "weight": 700}}})),
        v("ring", "Кольцо · итог в центре", "Сумма в центре, легенда по желанию", "categories", json!({"viz": {"mono": false, "inner": 0.72, "center": "total", "centerLabel": " ", "legendStyle": "none", "sliceGap": 1, "tint": 0, "outline": false, "decimals": 1, "centerSize": 9}}), json!({"card": false})),
        v("inflow", "Кольцо с легендой", "Суммы и доли справа", "categories", json!({"viz": {"mono": false, "tint": 0.2, "outline": false, "inner": 0.6, "center": "total", "legendStyle": "simple", "legendAmount": true, "legendShare": true, "sortSlices": false, "decimals": 1}}), json!({"card": false})),
    ]);
    m.insert("waterfall".into(), vec![v("soft", "Водопад", "Рост, спад, итог плашкой", "trend",
        json!({"viz": {"callout": "last", "units": "millions", "extremes": true, "legend": "top-right", "totalLabel": "Итог"}}), json!({"card": false}))]);
    m.insert("treemap".into(), vec![
        v("split", "Плитки · крупнейший слева", "Остальные справа, мелкие — строками", "categories", json!({"viz": {"tmLayout": "split", "chipBelow": 2.5, "shareText": "портфеля"}}), json!({"card": false})),
        v("result", "Плитки по результату", "Цвет — доходность, размер — сумма", "signed", json!({"viz": {"tmLayout": "split", "chipBelow": 3, "colorMode": "sign", "shareText": " ", "compact": "on", "decimals": 2}}), json!({"card": false})),
    ]);
    m.insert("tiles".into(), vec![
        v("horizons", "Доходность за периоды", "Маленькие карточки в ряд", "signed",
            json!({"viz": {"cols": 4, "signed": true, "decimals": 1, "markExtremes": false, "tileBg": CARD, "radius": 6}}),
            json!({"card": false, "fonts": {"caption": {"size": 5.8, "weight": 700, "color": SOFT, "upper": true, "tracking": 0.1}, "kpi": {"size": 12.5, "weight": 800}}})),
        v("results", "Результаты с иконками", "Лучший и худший обведены", "signed",
            json!({"viz": {"cols": 2, "signed": true, "markExtremes": true, "tileBg": CARD, "radius": 7}}),
            json!({"card": false, "fonts": {"caption": {"size": 6.8, "weight": 700, "color": INK}, "kpi": {"size": 11.5, "weight": 800}}})),
        v("stats", "Числа без рамок", "Сводка: число и подпись", "categories", json!({"viz": {"cols": 4, "tileBorder": false, "tileBg": CARD, "markExtremes": false, "valueColor": "ink", "decimals": 0}}),
            json!({"card": true, "background": CARD, "fonts": {"title": caps(5.8), "caption": {"size": 6, "weight": 600, "color": SOFT}, "kpi": {"size": 15, "weight": 800}}})),
    ]);
    m.insert("heading".into(), vec![
        v("page", "Заголовок страницы", "Крупный заголовок, линия под ним, пояснение справа", "", json!({"rule": 1.5}), json!({"fonts": {"caption": {"size": 7.2, "weight": 600, "color": SOFT}}})),
        v("eyebrow", "Заголовок с надзаголовком", "Серая капитель над названием", "", json!({"rule": 1.5, "eyebrow": "Раздел"}), json!({"fonts": {"eyebrow": {"color": SOFT}, "caption": {"size": 6.8, "weight": 500, "color": BODY}}})),
        v("account", "Счёт", "Плашка с номером, тип, навигация по страницам", "", json!({"badge": "№ 400PX0F", "eyebrow": "Брокерский счёт", "nav": true}), json!({"fonts": {"eyebrow": {"color": INK, "size": 7.4, "tracking": 0.08}}})),
        v("section", "Заголовок раздела", "Капитель", "", json!({}), json!({"fonts": {"heading": {"family": ui, "size": if upper { 9.4 } else { 11.0 }, "weight": l.title_weight, "upper": upper, "tracking": if upper { 0.05 } else { 0.0 }}, "caption": {"size": 7, "weight": 500, "color": SOFT}}})),
        v("section-accent", "Раздел · акцент", "Капитель, рядом количество", "", json!({}), json!({"fonts": {"heading": {"family": ui, "size": if upper { 11.0 } else { 12.5 }, "weight": l.title_weight, "upper": upper, "tracking": if upper { 0.05 } else { 0.0 }, "color": INK}, "caption": {"size": 7, "weight": 500, "color": SOFT}}})),
        v("stat", "Карточка-факт", "Подпись, значение текстом, пояснение", "", json!({}),
            json!({"card": true, "background": CARD, "fonts": {"heading": {"family": ui, "size": 13, "weight": 800}, "eyebrow": {"size": 5.8, "color": SOFT, "tracking": 0.1}, "heading_sub": {"size": 6.2, "color": BODY}}})),
        v("cover", "Обложка", "Крупный заголовок с надзаголовком", "", json!({"eyebrow": "Персональный отчёт"}), json!({"fonts": {"heading": {"size": l.display_size * 2.0}, "eyebrow": {"size": 7.4, "tracking": 0.24, "weight": 600}}})),
        v("person", "Подпись лица", "Надзаголовок, имя, контакты", "", json!({"eyebrow": "Ваш персональный банкир"}),
            json!({"fonts": {"heading": {"family": ui, "size": 9.5, "weight": 700}, "eyebrow": {"size": 5.8, "tracking": 0.14}, "heading_sub": {"size": 7, "color": BODY}}})),
    ]);
    m.insert("text".into(), vec![
        v("body", "Текст", "", "", json!({}), json!({"fonts": {"body": {"size": 7.4, "color": INK2}}})),
        v("small", "Мелкий текст", "Пояснения и сноски", "", json!({}), json!({"fonts": {"body": {"size": 6.2, "color": SOFT}}})),
        v("legend", "Легенда", "Цветные квадраты и подписи", "", json!({}), json!({"fonts": {"body": {"size": 6.4, "weight": 600, "color": BODY}}})),
    ]);
    m.insert("divider".into(), vec![
        v("ink", "Жирная линия", "", "", json!({}), json!({"color": "$rule", "width": 1.5})),
        v("hair", "Тонкая линия", "", "", json!({}), json!({"color": LINE2, "width": 0.75})),
        v("vertical", "Вертикальная", "Между колонками", "", json!({"vertical": true}), json!({"color": LINE2, "width": 0.75})),
    ]);
    shape_views(&mut m, l);
    m.insert("toc".into(), vec![v("sections", "Содержание по разделам", "Номер раздела, иконка, описание страниц", "",
        json!({"toc": {"style": "sections", "cols": 2, "label": "Раздел"}}), json!({"card": false}))]);
    m
}

/// The shared views redrawn in the look's shapes: donut holes and slice gaps,
/// bar and tile corners scale from the soft look they were designed in; bars
/// lose their own tint and outline (the theme's apply); big numbers take the
/// look's weight.
fn shape_views(m: &mut BTreeMap<String, Vec<StyleVariant>>, l: &Look) {
    let sh = &l.shape;
    let hole = |x: f64| 1.0 - (1.0 - x) * (1.0 - sh.donut) / 0.4;
    for (kind, views) in m.iter_mut() {
        for v in views.iter_mut() {
            let share = v.opts.get("orientation").and_then(|o| o.as_str()) == Some("share");
            if let Some(z) = v.opts.get_mut("viz").and_then(|z| z.as_object_mut()) {
                if let Some(x) = z.get("inner").and_then(|x| x.as_f64()) {
                    z.insert("inner".into(), json!((hole(x) * 100.0).round() / 100.0));
                }
                if let Some(x) = z.get("sliceGap").and_then(|x| x.as_f64()) {
                    z.insert("sliceGap".into(), json!(x * sh.slice_gap / 1.5));
                }
                if let Some(x) = z.get("radius").and_then(|x| x.as_f64()) {
                    let r = if kind == "tiles" { x * l.radius / 9.0 } else { x * sh.bar_radius / 4.0 };
                    z.insert("radius".into(), json!(r));
                }
                if kind == "bar" && !share {
                    z.remove("tint");
                    z.remove("outline");
                }
            }
            // tables spread their rows over their height when they all fit
            if kind == "table" {
                if let Some(t) = v.opts.get_mut("table").and_then(|t| t.as_object_mut()) {
                    t.entry("fill").or_insert(json!(true));
                }
            }
            // a theme of cards: every figure framed, but not the headline numbers
            let bare = kind == "kpi" && matches!(v.id.as_str(), "hero" | "hero-signed" | "value-right");
            if sh.cards_all && !bare && matches!(kind.as_str(), "table" | "bar" | "pie" | "waterfall" | "treemap" | "line" | "area" | "kpi") {
                if let Some(o) = v.style.as_object_mut() {
                    o.insert("card".into(), json!(true));
                    // drawn for no frame: on a card it takes the theme's inset
                    if o.get("pad").and_then(|p| p.as_f64()).is_some_and(|p| p < sh.card_pad) {
                        o.remove("pad");
                    }
                }
            }
            for part in ["kpi"] {
                if let Some(f) = v.style.pointer_mut(&format!("/fonts/{part}")).and_then(|f| f.as_object_mut()) {
                    if f.get("weight").and_then(|w| w.as_u64()) == Some(800) {
                        f.insert("weight".into(), json!(sh.kpi_weight));
                    }
                }
            }
        }
    }
}

/// A theme from a look: tokens, fonts and the shared design views.
pub fn theme_from(l: &Look) -> Theme {
    let mut th = Theme::default();
    th.name = l.name.into();
    th.locale = "ru-RU".into();
    th.currency = "₽".into();
    th.page_bg = l.page.into();
    th.card_bg = l.card.into();
    th.ink = l.ink.into();
    th.muted = l.soft.into();
    th.grid = l.line.into();
    th.good = l.good.into();
    th.bad = l.bad.into();
    th.accent = l.accent.into();
    th.card = l.shape.cards.into();
    th.card_border = l.line2.into();
    th.card_border_w = l.card_border_w;
    th.card_pad = l.shape.card_pad;
    th.radius = l.radius;
    th.chart_look = "soft".into();
    th.font_family = l.ui.into();
    th.palette = l.palette.iter().map(|s| s.to_string()).collect();
    let sh = &l.shape;
    th.viz = json!({"kpi": {"spark": "none", "delta": false, "compact": "off"}, "all": {
        "tint": sh.tint, "outline": sh.outline, "radius": sh.bar_radius, "roundAll": sh.round_all, "barFill": if sh.gradient { "gradient" } else { "flat" },
        "barWidth": sh.bar_width, "maxBar": sh.max_bar, "grid": sh.grid != "none", "gridStyle": if sh.grid == "none" { "solid" } else { sh.grid }, "axis": sh.axis,
        "lineWidth": sh.line_width, "curve": sh.curve, "markers": sh.markers, "markerFill": sh.marker_fill, "areaFill": sh.area, "guides": sh.guides,
        "labelBox": sh.labels, "inner": sh.donut, "sliceGap": sh.slice_gap}});
    // treemap tiles in the look's own strength and spacing (only where it differs from the standard)
    if sh.tile_fill != 0.24 || sh.tile_gap != 2.0 || sh.tile_labels != "split" {
        th.viz["treemap"] = json!({"tmFill": sh.tile_fill, "tmGap": sh.tile_gap, "tmLabels": sh.tile_labels});
    }
    let tokens: [(&str, &str); 39] = [
        ("card2", l.card2), ("ink2", l.ink2), ("body", l.body), ("accentLine", l.accent_line),
        ("gainTint", l.gain_tint), ("lossTint", l.loss_tint), ("gainMid", l.gain_mid), ("lossMid", l.loss_mid),
        ("received", l.received), ("iconInk", l.icon_ink), ("chip", l.chip), ("share", l.share), ("forecast", l.forecast),
        ("interest", l.cats[0]), ("dividend", l.cats[1]), ("coupon", l.cats[2]), ("income", l.cats[3]), ("principal", l.cats[4]),
        ("dividendSoft", l.cats_soft[0]), ("couponSoft", l.cats_soft[1]), ("incomeSoft", l.cats_soft[2]),
        ("class1", l.classes[0]), ("class2", l.classes[1]), ("class3", l.classes[2]), ("class4", l.classes[3]), ("class5", l.classes[4]), ("class6", l.classes[5]),
        ("donut", l.donut_base), ("frame", l.frame), ("rule", l.rule),
        ("block1", l.blocks[0]), ("block2", l.blocks[1]), ("block3", l.blocks[2]), ("block4", l.blocks[3]),
        ("coverFill", l.cover_fill), ("backFill", l.back_fill), ("headFill", l.head_fill), ("headInk", l.head_ink), ("zebra", l.zebra),
    ];
    th.tokens = tokens.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let f = |family: Option<&str>, size: f64, weight: u16, color: Option<&str>, upper: Option<bool>, tracking: Option<f64>| FontSpec {
        family: family.map(String::from),
        size: Some(size),
        weight: Some(weight),
        color: color.map(String::from),
        upper,
        tracking,
        ..Default::default()
    };
    th.fonts.insert("heading".into(), f(Some(l.display), l.display_size, l.display_weight, Some(INK), None, (l.display_tracking != 0.0).then_some(l.display_tracking)));
    th.fonts.insert("heading_sub".into(), f(None, 7.5, 500, Some(SOFT), None, None));
    // headers stay neutral: red is for losses
    th.fonts.insert("eyebrow".into(), f(None, 6.8, 700, Some(SOFT), Some(true), Some(0.14)));
    th.fonts.insert("title".into(), f(None, if l.title_upper { 9.0 } else { 10.5 }, l.title_weight, Some(INK), Some(l.title_upper), Some(if l.title_upper { 0.05 } else { 0.0 })));
    th.fonts.insert("subtitle".into(), f(None, 6.8, 500, Some(SOFT), None, None));
    th.fonts.insert("body".into(), f(None, 8.0, 500, Some(INK2), None, None));
    th.fonts.insert("caption".into(), f(None, 6.6, 600, Some(SOFT), None, None));
    th.fonts.insert("kpi".into(), f(None, 18.0, l.shape.kpi_weight, Some(INK), None, None));
    th.fonts.insert("table_head".into(), f(None, 5.8, 700, Some(SOFT), Some(true), Some(0.08)));
    th.fonts.insert("table_cell".into(), f(None, 8.2, 500, Some(INK), None, None));
    th.fonts.insert("chart".into(), f(None, 6.4, 500, Some(SOFT), None, None));
    th.fonts.insert("footer".into(), f(None, 6.6, 600, Some(SOFT), None, None));
    th.styles = styles(l);
    // a look with colour blocks: KPI cards take them in turn (figures in the ink, as on a brand panel),
    // and the big figures get a thick accent bar on the left
    if !l.blocks[0].is_empty() {
        if let Some(list) = th.styles.get_mut("kpi") {
            for v in list.iter_mut() {
                if !v.style.is_object() { v.style = json!({}); }
                match v.id.as_str() {
                    "card" | "card-result" | "card-toned" | "stat" | "breakdown" | "icon" => {
                        v.style["fillCycle"] = json!(true);
                        merge_json(&mut v.opts, &json!({"viz": {"valueColor": "ink", "tone": null}}));
                    }
                    "hero" | "hero-signed" => {
                        v.style["rule"] = json!(["left"]);
                        v.style["ruleWidth"] = json!(5);
                        v.style["ruleColor"] = json!(ACCENT_LINE);
                        v.style["pad"] = json!(12);
                    }
                    _ => {}
                }
            }
        }
    }
    // cards must show: a look whose cards are the paper with no frame (a flat infographic) puts the views
    // that ask for a card on its second card colour, framed by a hairline in the card outline (colour
    // blocks stay unframed: `on_block` in vm.rs)
    if !card_shows(&th, &json!({"card": true})) {
        th.card_bg = l.card2.into();
        th.tokens.insert("card2".into(), crate::ai_restyle::mix(l.card2, l.ink, 0.07));
        th.card = "outlined".into();
        if th.card_border_w <= 0.0 {
            th.card_border_w = 0.75;
        }
    }
    // KPI cards that colour their figure by its sign sit on a soft fill of their result
    if l.shape.kpi_tone {
        if let Some(list) = th.styles.get_mut("kpi") {
            for v in list.iter_mut().filter(|v| v.style["card"] == json!(true)) {
                if v.opts.pointer("/viz/valueColor").and_then(|x| x.as_str()) == Some("sign") && v.opts.pointer("/viz/tone").is_none() {
                    merge_json(&mut v.opts, &json!({"viz": {"tone": "sign"}}));
                }
            }
        }
    }
    for (k, old, _) in FOLDED {
        if let Some(l) = th.styles.get_mut(k) {
            l.retain(|v| v.id != old);
        }
    }
    // colours that repeat a named one (the accent line in the accent, white header text…) follow it by name
    th.link_colours();
    th
}

/// The Modern UHNWI theme.
pub fn theme() -> Theme {
    theme_from(&UHNWI)
}

#[cfg(test)]
mod fold_tests {
    use super::*;

    fn clean(v: &Json) -> Json {
        match v {
            Json::Object(m) => Json::Object(m.iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.clone(), clean(v))).collect()),
            _ => v.clone(),
        }
    }

    #[test]
    fn built_in_themes_use_their_colours_by_name() {
        for (id, l) in LOOKS {
            let th = theme_from(l);
            // the derived colours draw as the look has them, linked or not
            for (k, hex) in [("accentLine", l.accent_line), ("share", l.share), ("frame", l.frame), ("rule", l.rule), ("donut", l.donut_base),
                ("headFill", l.head_fill), ("headInk", l.head_ink), ("zebra", l.zebra), ("backFill", l.back_fill), ("block1", l.blocks[0])] {
                assert_eq!(th.token(k).unwrap_or_default(), hex, "{id}: {k}");
            }
            // no view, text role or derived colour repeats a named colour as a hex
            let all = format!("{}{}{}", serde_json::to_string(&th.styles).unwrap(), serde_json::to_string(&th.fonts).unwrap(), serde_json::to_string(&th.tokens).unwrap()).to_lowercase();
            for (n, hex) in th.named_colours() {
                assert!(!all.contains(&format!("\"{hex}\"")), "{id}: {hex} is written out, not as {n}");
            }
            assert_eq!(th.clone().link_colours(), 0, "{id}: linking twice");
        }
        // a new accent takes what was in the accent with it: Emerald's accent line, share bar and table header
        let mut th = theme_from(&EMERALD);
        th.accent = "#123456".into();
        for k in ["accentLine", "share", "headFill"] {
            assert_eq!(th.token(k).as_deref(), Some("#123456"), "{k}");
        }
        assert_eq!(th.token("headInk").as_deref(), Some("#ffffff"), "white on the band stays white");
    }

    #[test]
    fn folded_looks_draw_as_before() {
        let raw = crate::uhnwi_sample::template_raw();
        let mut full = raw.theme.clone();
        full.styles = styles(&UHNWI);
        let mut doc = raw.clone();
        let n = fold_views(&mut doc);
        assert!(n >= 10, "{n} widgets moved");
        for (p0, p1) in raw.pages.iter().zip(&doc.pages) {
            for (a, b) in p0.widgets.iter().zip(&p1.widgets) {
                let (ea, eb) = (effective(&full, a), effective(&doc.theme, b));
                assert_eq!(clean(&ea.opts), clean(&eb.opts), "opts of {} on {} → {}", a.kind, a.variant, b.variant);
                assert_eq!(clean(&ea.style), clean(&eb.style), "style of {} on {} → {}", a.kind, a.variant, b.variant);
            }
        }
        for (k, old, _) in FOLDED {
            assert!(doc.theme.variant(k, old).is_none(), "{k}/{old} left the theme");
            assert!(!doc.pages.iter().flat_map(|p| &p.widgets).any(|w| w.kind == k && w.variant == old));
        }
        // folding twice changes nothing
        let again = doc.clone();
        assert_eq!(fold_views(&mut doc), 0);
        assert_eq!(serde_json::to_string(&doc.pages).unwrap(), serde_json::to_string(&again.pages).unwrap());
    }
}

/// Whether a view's card can be seen on the page: a fill that stands off the paper, a frame or a shadow.
pub fn card_shows(th: &Theme, style: &Json) -> bool {
    use crate::ai_restyle::contrast;
    if style["card"] != json!(true) || style["fillCycle"] == json!(true) {
        return true;
    }
    let page = th.color("$page");
    let fill = style["background"].as_str().map(|c| th.color(c)).unwrap_or_else(|| th.color("$card"));
    let border = style["border"].as_str().map(|c| th.color(c))
        .or_else(|| (th.card == "outlined").then(|| th.color("$cardBorder")));
    let bw = style["borderWidth"].as_f64().unwrap_or(th.card_border_w);
    let framed = border.is_some_and(|b| bw > 0.0 && contrast(&b, &page) >= 1.15);
    th.card == "shadow" || framed || contrast(&fill, &page) >= 1.07
}

#[cfg(test)]
mod card_tests {
    use super::*;

    #[test]
    fn every_card_view_shows_a_card() {
        let mut bad = vec![];
        for (id, l) in LOOKS {
            let th = theme_from(l);
            for (kind, list) in &th.styles {
                for v in list {
                    if !card_shows(&th, &v.style) {
                        bad.push(format!("{id}: {kind}/{}", v.id));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "cards that don't show: {bad:?}");
    }
}
