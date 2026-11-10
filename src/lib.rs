//! Quire's core: documents, data, layout, export, AI, accounts — everything but the window.
//! The desktop app (`src/main.rs`, Tauri) and the web server (`src/bin/quire-server.rs`) both
//! run it through `dispatch`, so the two can't drift apart.
#![recursion_limit = "256"]

mod ai;
mod ai_design;
mod ai_restyle;
mod ai_report;
mod ai_admin;
mod api_accounts;
mod audit;
mod auth;
mod client_sample;
mod uhnwi_sample;
mod uhnwi_sql;
mod uhnwi_theme;
mod theme_studio;
mod data;
mod dbconn;
mod export;
mod fonts;
mod layout;
mod looks;
mod model;
mod overview;
mod proposal;
mod proposals;
mod period;
mod python;
mod query;
#[cfg(feature = "render")]
mod render_js;
mod sample;
mod text;
mod vm;

mod app;
mod host;
pub mod secrets;

pub use app::{dispatch, AppState, COMMANDS};
pub use fonts::warm_up;
pub use host::{FileSpec, Host};

/// Guards Python on the web server: it runs only when an administrator has switched it on
/// (saved in the accounts store at `users_json`; off when nothing is saved).
pub fn guard_python(users_json: &std::path::Path) {
    let on = users_json.exists() && auth::load(users_json).python_on_server;
    python::guard(on);
}

/// Fences data in for the web server: files only from `dirs` (and the demo bank database's
/// folder), database servers only `hosts` (None: any).
pub fn fence_data(dirs: &[std::path::PathBuf], hosts: Option<Vec<String>>) {
    let mut dirs = dirs.to_vec();
    if let Some(demo) = dbconn::demo_path().and_then(|p| std::path::Path::new(&p).parent().map(|d| d.to_path_buf())) {
        dirs.push(demo);
    }
    data::fence(&dirs, hosts);
}
