//! The desktop app: Quire's core in a Tauri window, with the computer's own file dialogs,
//! eyedropper and Finder (on macOS; elsewhere a sealed secrets file and the file manager). Every command goes through `quire::dispatch`, as on the web server.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use quire::{AppState, FileSpec, Host};
use serde_json::Value as Json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::menu::Menu;
use tauri::{Manager, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

/// The desktop platform: Tauri's app folder and dialogs, macOS's eyedropper and Finder.
struct Desktop(tauri::AppHandle);

impl Desktop {
    fn dialog(&self, d: &FileSpec) -> tauri_plugin_dialog::FileDialogBuilder<tauri::Wry> {
        let mut b = self.0.dialog().file();
        for (name, exts) in &d.filters {
            let exts: Vec<&str> = exts.iter().map(String::as_str).collect();
            b = b.add_filter(name, &exts);
        }
        if let Some(n) = &d.name {
            b = b.set_file_name(n);
        }
        if let Some(dir) = &d.dir {
            b = b.set_directory(dir);
        }
        b
    }
}

impl Host for Desktop {
    fn data_dir(&self) -> Option<PathBuf> {
        self.0.path().app_data_dir().ok()
    }

    fn pick_file(&self, d: &FileSpec) -> Result<Option<PathBuf>, String> {
        self.dialog(d).blocking_pick_file().map(|f| f.into_path().map_err(|e| e.to_string())).transpose()
    }

    fn save_file(&self, d: &FileSpec) -> Result<Option<PathBuf>, String> {
        self.dialog(d).blocking_save_file().map(|f| f.into_path().map_err(|e| e.to_string())).transpose()
    }

    fn pick_folder(&self) -> Result<Option<PathBuf>, String> {
        self.0.dialog().file().blocking_pick_folder().map(|f| f.into_path().map_err(|e| e.to_string())).transpose()
    }

    /// A colour picked anywhere on screen (another app's window included) with the system
    /// eyedropper, as "#rrggbb"; None when the user cancels.
    fn pick_color(&self) -> Result<Option<String>, String> {
        #[cfg(target_os = "macos")]
        {
            let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
            self.0
                .run_on_main_thread(move || {
                    use objc2_app_kit::{NSColor, NSColorSampler, NSColorSpace};
                    let tx = std::sync::Mutex::new(Some(tx));
                    let handler = block2::RcBlock::new(move |c: *mut NSColor| {
                        let hex = unsafe { c.as_ref() }.and_then(|c| c.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())).map(|c| {
                            let ch = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                            format!("#{:02x}{:02x}{:02x}", ch(c.redComponent()), ch(c.greenComponent()), ch(c.blueComponent()))
                        });
                        if let Some(tx) = tx.lock().ok().and_then(|mut t| t.take()) {
                            let _ = tx.send(hex);
                        }
                    });
                    let sampler = NSColorSampler::new();
                    unsafe { sampler.showSamplerWithSelectionHandler(&handler) };
                })
                .map_err(|e| e.to_string())?;
            Ok(rx.recv().ok().flatten())
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("The screen eyedropper is available on macOS.".into())
        }
    }

    /// Shows the file in Finder (or the file manager), or opens it with its app (`open`).
    fn reveal(&self, path: &Path, open: bool) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        let mut cmd = {
            let mut c = std::process::Command::new("open");
            if !open {
                c.arg("-R");
            }
            c.arg(path);
            c
        };
        #[cfg(windows)]
        let mut cmd = {
            let mut c = std::process::Command::new("explorer");
            c.arg(if open { path.as_os_str().to_owned() } else { format!("/select,{}", path.display()).into() });
            c
        };
        // Linux and the like: the desktop's opener, on the file's folder to show it
        #[cfg(not(any(target_os = "macos", windows)))]
        let mut cmd = {
            let mut c = std::process::Command::new("xdg-open");
            c.arg(if open || path.is_dir() { path } else { path.parent().unwrap_or(path) });
            c
        };
        cmd.spawn().map_err(|e| format!("Couldn't open the file manager: {e}"))?;
        Ok(())
    }

    fn devtools(&self) {
        if let Some(w) = self.0.get_webview_window("main") {
            w.open_devtools();
        }
    }
}

type Core = Arc<AppState>;

/// The one command the window sends: any of the core's, by name, run off the main thread.
#[tauri::command]
async fn call(state: tauri::State<'_, Core>, cmd: String, args: Option<Json>) -> Result<Json, String> {
    let core = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let r = quire::dispatch(&core, &cmd, &args.unwrap_or(Json::Null));
        // QUIRE_TRACE=1: each command and how it went, in the terminal
        if std::env::var_os("QUIRE_TRACE").is_some() {
            eprintln!("[call] {cmd} → {}", r.as_ref().map(|_| "ok".to_string()).unwrap_or_else(|e| format!("error: {e}")));
        }
        r
    })
    .await
    .map_err(|e| e.to_string())?
}

fn main() {
    quire::warm_up();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .menu(|app| Menu::default(app))
        .setup(|app| {
            let host = Desktop(app.handle().clone());
            if let Some(d) = host.data_dir() {
                let _ = std::fs::create_dir_all(&d);
                // no Keychain outside macOS: AI keys and database passwords go to a sealed file, as on the server
                #[cfg(not(target_os = "macos"))]
                {
                    let master = quire::secrets::master_key(&d)?.0;
                    quire::secrets::use_file(&d.join("secrets.json"), &master)?;
                }
            }
            app.manage::<Core>(Arc::new(AppState::new(Box::new(host), None)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![call])
        .on_window_event(|window, event| {
            // quitting ends every work session (with its time)
            if let WindowEvent::Destroyed = event {
                window.state::<Core>().end_work("app closed");
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<Core>().dirty() {
                    api.prevent_close();
                    let w = window.clone();
                    window
                        .dialog()
                        .message("You have unsaved changes. Close without saving?")
                        .title("Unsaved changes")
                        .kind(MessageDialogKind::Warning)
                        .buttons(MessageDialogButtons::OkCancelCustom("Close without saving".into(), "Cancel".into()))
                        .show(move |ok| {
                            if ok {
                                w.state::<Core>().discard_changes();
                                let _ = w.close();
                            }
                        });
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Quire");
}
