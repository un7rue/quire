//! Chart style presets ("clean", "soft").
//!
//! A preset used to be a `look` setting at any level: the theme, the theme's
//! chart defaults, a design view or a widget. Now the theme's `chart_look` is
//! the only preset, and a `look` anywhere else is written out as the explicit
//! settings it stood for, so files look the same and every setting is visible.
//! The values mirror `PRESETS` and `BASE` in `ui/charts.js`.

use crate::model::{Doc, Theme, Widget};
use serde_json::{json, Map, Value as Json};

/// The "soft" preset, and what the same keys are without it.
fn soft() -> [(&'static str, Json, Json); 25] {
    [
        ("tint", json!(0.5), json!(0)),
        ("outline", json!(true), json!(false)),
        ("radius", json!(4), json!(2.5)),
        ("roundAll", json!(true), json!(false)),
        ("maxBar", json!(34), json!(24)),
        ("labels", json!("all"), json!("auto")),
        ("labelColor", json!("sign"), json!("auto")),
        ("legend", json!("top-right"), json!("auto")),
        ("extremes", json!(true), json!(false)),
        ("callout", json!("last"), json!("none")),
        ("markers", json!("all"), json!("last")),
        ("markerFill", json!("hollow"), json!("solid")),
        ("lineWidth", json!(2), json!(1.75)),
        ("rankStyle", json!("ranked"), json!("bars")),
        ("legendStyle", json!("table"), json!("simple")),
        ("legendAmount", json!(true), json!(false)),
        ("mono", json!(true), json!(false)),
        ("inner", json!(0.56), json!(0.62)),
        ("sliceGap", json!(2), json!(1.2)),
        ("tmLayout", json!("split"), json!("squarify")),
        ("ticks", json!(4), json!(5)),
        ("colorMode", json!("signOne"), json!("auto")),
        ("signedTicks", json!(true), json!(false)),
        ("labelBold", json!(true), json!(false)),
        ("centerUpper", json!(true), json!(false)),
    ]
}

fn is_soft(look: &str) -> bool {
    look == "soft"
}

fn viz(o: &Json) -> Option<&Map<String, Json>> {
    o.get("viz").and_then(|v| v.as_object())
}

fn look_of(v: Option<&Map<String, Json>>) -> Option<String> {
    v.and_then(|m| m.get("look")).and_then(|l| l.as_str()).filter(|l| !l.is_empty()).map(String::from)
}

/// Keys a widget's older top-level options set (they rank above the preset).
fn legacy_keys(opts: &Json) -> Vec<&'static str> {
    let has = |k: &str| opts.get(k).is_some_and(|v| !v.is_null());
    let mut out = Vec::new();
    if has("labels") || opts.get("endLabels") == Some(&json!(false)) {
        out.push("labels");
    }
    if has("signColors") {
        out.push("colorMode");
    }
    for k in ["extremes", "callout", "mono"] {
        if has(k) {
            out.push(k);
        }
    }
    if opts.get("donut") == Some(&json!(false)) {
        out.push("inner");
    }
    if has("legendValues") {
        out.extend(["legendStyle", "legendAmount"]);
    }
    if opts.get("legend") == Some(&json!(false)) {
        out.push("legend");
    }
    out
}

/// Writes preset `look` (in place of `base`, the preset that applies once
/// looks are gone) into `target` for every key not already decided at a
/// higher level (`taken`). Explicit "auto" values that meant something
/// different under the soft preset become their explicit form.
fn spell_out(target: &mut Map<String, Json>, look: &str, base: &str, taken: &[&Map<String, Json>], extra: &[&str]) {
    let above = |k: &str| taken.iter().any(|m| m.get(k).is_some_and(|v| !v.is_null())) || extra.contains(&k);
    if is_soft(look) {
        for (k, explicit) in [("colorMode", "signOne"), ("legend", "top-right")] {
            if target.get(k) == Some(&json!("auto")) {
                target.insert(k.into(), json!(explicit));
            }
        }
    }
    if is_soft(look) != is_soft(base) {
        for (k, s, b) in soft() {
            if !target.contains_key(k) && !above(k) {
                target.insert(k.into(), if is_soft(look) { s } else { b });
            }
        }
    }
}

fn viz_mut(o: &mut Json) -> &mut Map<String, Json> {
    if !o.is_object() {
        *o = json!({});
    }
    let m = o.as_object_mut().unwrap();
    if !m.get("viz").is_some_and(|v| v.is_object()) {
        m.insert("viz".into(), json!({}));
    }
    m.get_mut("viz").unwrap().as_object_mut().unwrap()
}

/// The theme's own levels: chart defaults per kind and for all kinds, and design views.
pub fn theme(th: &mut Theme) {
    let base = if th.chart_look.is_empty() { "clean".to_string() } else { th.chart_look.clone() };
    let orig_viz = th.viz.clone();
    let all = orig_viz.get("all").and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let all_look = look_of(Some(&all));
    let kind_map = |k: &str| orig_viz.get(k).and_then(|v| v.as_object()).cloned().unwrap_or_default();
    // chart defaults
    if let Some(m) = th.viz.as_object_mut() {
        let kinds: Vec<String> = m.keys().cloned().collect();
        for k in kinds {
            let Some(level) = m.get_mut(&k).and_then(|v| v.as_object_mut()) else { continue };
            let own = look_of(Some(level));
            level.remove("look");
            let look = if k == "all" { own } else { own.or_else(|| all_look.clone()) };
            if let Some(look) = look {
                let taken: Vec<&Map<String, Json>> = if k == "all" { vec![] } else { vec![&all] };
                spell_out(level, &look, &base, &taken, &[]);
            }
        }
    }
    // design views
    for (kind, list) in th.styles.iter_mut() {
        let km = kind_map(kind);
        let inherited = look_of(Some(&km)).or_else(|| all_look.clone());
        for sv in list.iter_mut() {
            let own = look_of(viz(&sv.opts)).or_else(|| sv.opts.get("look").and_then(|l| l.as_str()).filter(|l| !l.is_empty()).map(String::from));
            if let Some(o) = sv.opts.as_object_mut() {
                o.remove("look");
            }
            if let Some(v) = sv.opts.get_mut("viz").and_then(|v| v.as_object_mut()) {
                v.remove("look");
            }
            // a view without its own look follows the chart defaults, already spelled out
            if let Some(look) = own {
                let extra = legacy_keys(&sv.opts);
                let v = viz_mut(&mut sv.opts);
                // compared with what the view would inherit: the defaults' look, else the theme preset
                let inherited = inherited.clone().unwrap_or(base.clone());
                spell_out(v, &look, &inherited, &[&all, &km], &extra);
            }
        }
    }
}

/// A widget's own look, given its theme as it was before [`theme`] ran.
fn widget(w: &mut Widget, orig: &Theme, base: &str) {
    let own = look_of(viz(&w.opts)).or_else(|| w.opts.get("look").and_then(|l| l.as_str()).filter(|l| !l.is_empty()).map(String::from));
    if let Some(o) = w.opts.as_object_mut() {
        o.remove("look");
    }
    if let Some(v) = w.opts.get_mut("viz").and_then(|v| v.as_object_mut()) {
        v.remove("look");
    }
    let Some(look) = own else { return };
    let all = orig.viz.get("all").and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let km = orig.viz.get(&w.kind).and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let view = orig.styles.get(&w.kind).and_then(|l| l.iter().find(|s| !w.variant.is_empty() && s.id == w.variant));
    let view_viz = view.and_then(|s| viz(&s.opts)).cloned().unwrap_or_default();
    // what the widget would inherit now: its view's look, else the defaults', else the theme preset
    let inherited = view
        .and_then(|s| look_of(viz(&s.opts)))
        .or_else(|| look_of(Some(&km)))
        .or_else(|| look_of(Some(&all)))
        .unwrap_or_else(|| base.to_string());
    let mut extra = legacy_keys(&w.opts);
    if let Some(s) = view {
        extra.extend(legacy_keys(&s.opts));
    }
    let v = viz_mut(&mut w.opts);
    spell_out(v, &look, &inherited, &[&all, &km, &view_viz], &extra);
}

/// Spells out every `look` in a document; the theme's `chart_look` stays as its preset.
pub fn doc(d: &mut Doc) {
    let has = |o: &Json| o.get("look").is_some() || viz(o).is_some_and(|v| v.contains_key("look"));
    let any = d.pages.iter().flat_map(|p| &p.widgets).chain(d.catalog.iter().map(|c| &c.widget)).any(|w| has(&w.opts))
        || d.theme.viz.as_object().is_some_and(|m| m.values().any(|v| v.get("look").is_some()))
        || d.theme.styles.values().flatten().any(|s| has(&s.opts));
    if !any {
        return;
    }
    let orig = d.theme.clone();
    let base = if orig.chart_look.is_empty() { "clean".to_string() } else { orig.chart_look.clone() };
    for p in &mut d.pages {
        for w in &mut p.widgets {
            widget(w, &orig, &base);
        }
    }
    for c in &mut d.catalog {
        widget(&mut c.widget, &orig, &base);
    }
    theme(&mut d.theme);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::StyleVariant;

    #[test]
    fn looks_become_explicit_settings() {
        let mut th = Theme { chart_look: "clean".into(), ..Theme::default() };
        th.styles.insert("bar".into(), vec![StyleVariant { id: "s".into(), opts: json!({"viz": {"look": "soft", "labels": "none"}}), ..Default::default() }]);
        let mut d = Doc { theme: th, ..Default::default() };
        d.pages.push(crate::model::Page { id: "p".into(), widgets: vec![
            Widget { id: "a".into(), kind: "bar".into(), variant: "s".into(), opts: json!({}), ..Default::default() },
            Widget { id: "b".into(), kind: "bar".into(), variant: "s".into(), opts: json!({"viz": {"look": "clean"}}), ..Default::default() },
        ], ..Default::default() });
        doc(&mut d);
        let v = &d.theme.styles["bar"][0].opts["viz"];
        assert!(v.get("look").is_none());
        assert_eq!(v["tint"], json!(0.5));
        assert_eq!(v["labels"], json!("none"), "explicit settings win");
        assert_eq!(v["colorMode"], json!("signOne"));
        // a widget that chose "clean" over its soft view gets the plain values back
        let b = &d.pages.last().unwrap().widgets[1].opts["viz"];
        assert!(b.get("look").is_none());
        assert_eq!(b["tint"], json!(0));
        assert!(b.get("labels").is_none(), "the view's own label setting still applies");
        assert!(d.pages.last().unwrap().widgets[0].opts.get("viz").is_none());
    }
}
