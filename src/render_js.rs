//! Report pages drawn without a browser, for the report API: the UI's own drawing code
//! (`ui/render.js` and what it imports) runs in V8, so the pages are the ones the app draws.
//! The one thing the browser gave it, measuring text, comes from the core (`text::width_f`,
//! the same font files), through `quireMeasure` (see `textWidth` in `ui/util.js`).
//!
//! Each thread keeps one engine with the drawing code loaded; drawing a report then costs only
//! the drawing itself.

use crate::text;
use serde_json::Value as Json;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Once;

/// The module the engine starts from: it hands the report's state to `pageSvg` for each shown page.
const ENTRY: &str = r#"
import { pageSvg } from './render.js';
globalThis.quireDraw = (json) => {
  const st = JSON.parse(json);
  const shown = st.doc.pages.map((_, i) => i).filter((i) => !st.pages[i]?.hidden);
  return JSON.stringify(shown.map((i) => pageSvg(st, i, { exportMode: true })));
};
"#;
const ENTRY_NAME: &str = "quire-draw.js";
/// The engine's heap kept between reports; past it, it is cleaned up after a report.
const HEAP_KEPT: usize = 96 << 20;

fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // number formats by language (Intl) read ICU's data
        let _ = v8::icu::set_common_data_78(deno_core_icudata::ICU_DATA);
        v8::V8::initialize_platform(v8::new_default_platform(0, false).make_shared());
        v8::V8::initialize();
    });
}

/// One engine with the drawing code loaded.
struct Engine {
    // handles go before the isolate they live in
    context: v8::Global<v8::Context>,
    draw: v8::Global<v8::Function>,
    ui: PathBuf,
    isolate: v8::OwnedIsolate,
}

thread_local! {
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
    /// While the drawing code loads: each module by its file, and each module's file by its id.
    static LOADING: RefCell<(PathBuf, HashMap<PathBuf, v8::Global<v8::Module>>, HashMap<i32, PathBuf>)> = RefCell::new(Default::default());
}

/// The pages of a report (its state, as the app gets it) as SVG documents, the hidden ones left
/// out, drawn by the UI's code in `ui`.
pub fn pages(ui: &Path, state: &Json) -> Result<Vec<String>, String> {
    let json = serde_json::to_string(state).map_err(|e| e.to_string())?;
    ENGINE.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.as_ref().is_some_and(|e| e.ui != ui) {
            *slot = None;
        }
        if slot.is_none() {
            *slot = Some(Engine::new(ui)?);
        }
        let r = slot.as_mut().unwrap().draw(&json);
        // a broken engine is started afresh next time
        if r.is_err() {
            *slot = None;
        }
        r
    })
}

/// `quireMeasure(text, size, weight, family, italic)`: the width in points.
fn measure(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue<v8::Value>) {
    let s = args.get(0).to_rust_string_lossy(scope);
    let size = args.get(1).number_value(scope).unwrap_or(0.0);
    let weight = args.get(2).number_value(scope).unwrap_or(400.0).clamp(100.0, 900.0) as u16;
    let family = args.get(3).to_rust_string_lossy(scope);
    let italic = args.get(4).boolean_value(scope);
    let f = text::Font { family, size, weight, italic, color: String::new(), upper: false, tracking: 0.0 };
    rv.set(v8::Number::new(scope, text::width_f(&s, &f)).into());
}

/// What went wrong in the drawing code, with where.
fn failure(tc: &mut v8::PinnedRef<'_, v8::TryCatch<v8::HandleScope>>) -> String {
    let what = tc.exception().map(|e| e.to_rust_string_lossy(tc)).unwrap_or_else(|| "the drawing code failed".into());
    let at = tc.message().map(|m| {
        let file = m.get_script_resource_name(tc).map(|n| n.to_rust_string_lossy(tc)).unwrap_or_default();
        let file = Path::new(&file).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or(file);
        format!(" ({file}:{})", m.get_line_number(tc).unwrap_or(0))
    });
    format!("Drawing the pages failed: {what}{}", at.unwrap_or_default())
}

/// Compiles the module in `path` (`source` when given, else the file's text).
fn compile<'s>(scope: &mut v8::PinScope<'s, '_>, path: &Path, source: Option<&str>) -> Option<v8::Local<'s, v8::Module>> {
    let text = match source {
        Some(s) => s.to_string(),
        None => match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                let msg = v8::String::new(scope, &format!("can't read {}: {e}", path.display()))?;
                let err = v8::Exception::error(scope, msg);
                scope.throw_exception(err);
                return None;
            }
        },
    };
    let code = v8::String::new(scope, &text)?;
    let name = v8::String::new(scope, &path.display().to_string())?;
    let origin = v8::ScriptOrigin::new(scope, name.into(), 0, 0, false, 0, None, false, false, true, None);
    let mut src = v8::script_compiler::Source::new(code, Some(&origin));
    let module = v8::script_compiler::compile_module(scope, &mut src)?;
    let global = v8::Global::new(scope, module);
    LOADING.with(|l| {
        let mut l = l.borrow_mut();
        l.1.insert(path.to_path_buf(), global);
        l.2.insert(module.get_identity_hash().get(), path.to_path_buf());
    });
    Some(module)
}

/// An `import` of the drawing code: a file of the UI folder, relative to the importing one.
fn resolve<'s>(
    context: v8::Local<'s, v8::Context>,
    specifier: v8::Local<'s, v8::String>,
    _attributes: v8::Local<'s, v8::FixedArray>,
    referrer: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
    v8::callback_scope!(unsafe scope, context);
    let spec = specifier.to_rust_string_lossy(scope);
    let (ui, from) = LOADING.with(|l| {
        let l = l.borrow();
        (l.0.clone(), l.2.get(&referrer.get_identity_hash().get()).cloned())
    });
    let dir = from.as_deref().and_then(Path::parent).unwrap_or(&ui).to_path_buf();
    let path = normalize(&dir.join(&spec));
    if !spec.starts_with("./") && !spec.starts_with("../") || !path.starts_with(&ui) {
        let msg = v8::String::new(scope, &format!("can't import “{spec}”: only files of the UI folder"))?;
        let err = v8::Exception::error(scope, msg);
        scope.throw_exception(err);
        return None;
    }
    if let Some(g) = LOADING.with(|l| l.borrow().1.get(&path).cloned()) {
        return Some(v8::Local::new(scope, g));
    }
    compile(scope, &path, None)
}

/// `a/./b/../c` as `a/c`.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

impl Engine {
    fn new(ui: &Path) -> Result<Engine, String> {
        init();
        let ui = normalize(&std::path::absolute(ui).map_err(|e| e.to_string())?);
        if !ui.join("render.js").is_file() {
            return Err(format!("The drawing code isn't in {}.", ui.display()));
        }
        let mut isolate = v8::Isolate::new(v8::CreateParams::default());
        LOADING.with(|l| *l.borrow_mut() = (ui.clone(), HashMap::new(), HashMap::new()));
        let made = (|| {
            v8::scope!(let scope, &mut isolate);
            let context = v8::Context::new(scope, Default::default());
            let scope = &mut v8::ContextScope::new(scope, context);
            v8::tc_scope!(let tc, scope);
            let f = v8::Function::new(tc, measure).ok_or("no measure")?;
            let key = v8::String::new(tc, "quireMeasure").ok_or("no key")?;
            context.global(tc).set(tc, key.into(), f.into());
            let Some(entry) = compile(tc, &ui.join(ENTRY_NAME), Some(ENTRY)) else { return Err(failure(tc)) };
            if entry.instantiate_module(tc, resolve) != Some(true) {
                return Err(failure(tc));
            }
            if entry.evaluate(tc).is_none() || entry.get_status() == v8::ModuleStatus::Errored {
                if !tc.has_caught() {
                    let e = entry.get_exception();
                    return Err(format!("Drawing the pages failed: {}", e.to_rust_string_lossy(tc)));
                }
                return Err(failure(tc));
            }
            let key = v8::String::new(tc, "quireDraw").ok_or("no key")?;
            let draw = context.global(tc).get(tc, key.into()).and_then(|v| v8::Local::<v8::Function>::try_from(v).ok()).ok_or("The drawing code didn't start.")?;
            Ok((v8::Global::new(tc, context), v8::Global::new(tc, draw)))
        })();
        // the modules are held by the engine from here on
        LOADING.with(|l| *l.borrow_mut() = Default::default());
        let (context, draw) = made?;
        Ok(Engine { context, draw, ui, isolate })
    }

    fn draw(&mut self, json: &str) -> Result<Vec<String>, String> {
        let out = {
            v8::scope!(let scope, &mut self.isolate);
            let context = v8::Local::new(scope, &self.context);
            let scope = &mut v8::ContextScope::new(scope, context);
            v8::tc_scope!(let tc, scope);
            let f = v8::Local::new(tc, &self.draw);
            let arg = v8::String::new(tc, json).ok_or("The report is too large to draw.")?;
            let recv = v8::undefined(tc);
            match f.call(tc, recv.into(), &[arg.into()]) {
                Some(v) => v.to_rust_string_lossy(tc),
                None => return Err(failure(tc)),
            }
        };
        // a report's state is large and the engine keeps what it allocated: past a size, it gives it back
        let before = self.isolate.get_heap_statistics().total_heap_size();
        if before > HEAP_KEPT {
            self.isolate.low_memory_notification();
        }
        serde_json::from_str(&out).map_err(|e| format!("Drawing the pages failed: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Text is measured by the core, and numbers are written as the browser writes them.
    #[test]
    fn measures_text_and_formats_numbers() {
        init();
        let mut isolate = v8::Isolate::new(v8::CreateParams::default());
        v8::scope!(let scope, &mut isolate);
        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);
        let f = v8::Function::new(scope, measure).unwrap();
        let key = v8::String::new(scope, "quireMeasure").unwrap();
        context.global(scope).set(scope, key.into(), f.into());
        let eval = |scope: &mut v8::PinScope, s: &str| {
            let code = v8::String::new(scope, s).unwrap();
            let v = v8::Script::compile(scope, code, None).unwrap().run(scope).unwrap();
            v.to_rust_string_lossy(scope)
        };
        let w: f64 = eval(scope, "quireMeasure('Portfolio', 12, 400, 'Inter', false)").parse().unwrap();
        assert!((w - text::width("Portfolio", 12.0, 400)).abs() < 1e-9, "{w}");
        assert_eq!(eval(scope, "(1234567.5).toLocaleString('ru-RU', { maximumFractionDigits: 1 })"), "1\u{a0}234\u{a0}567,5");
        assert_eq!(eval(scope, "(1234567.5).toLocaleString('en-US', { maximumFractionDigits: 1 })"), "1,234,567.5");
        assert_eq!(eval(scope, "(1234.5).toLocaleString('de-DE', { minimumFractionDigits: 2 })"), "1.234,50");
        assert_eq!(eval(scope, "['b', 'а', 'a'].sort((x, y) => x.localeCompare(y)).join('')"), "abа");
    }

    /// Drawing code that fails says where.
    #[test]
    fn missing_drawing_code_is_reported() {
        let e = pages(Path::new("/nonexistent/ui"), &serde_json::json!({})).unwrap_err();
        assert!(e.contains("isn't in"), "{e}");
    }
}
