//! What differs between the desktop app and the web server: where the app keeps its
//! files, and what needs the person's own computer (file dialogs, the screen eyedropper,
//! Finder, the web inspector). Everything else is the same core.

use std::path::{Path, PathBuf};

/// The platform the core runs in.
pub trait Host: Send + Sync {
    /// The app's own folder: settings, accounts, themes, the AI and activity logs.
    fn data_dir(&self) -> Option<PathBuf>;
    /// A file the person picks to open. Ok(None): cancelled.
    fn pick_file(&self, d: &FileSpec) -> Result<Option<PathBuf>, String>;
    /// Where the person saves a file. Ok(None): cancelled.
    fn save_file(&self, d: &FileSpec) -> Result<Option<PathBuf>, String>;
    /// A folder the person picks. Ok(None): cancelled.
    fn pick_folder(&self) -> Result<Option<PathBuf>, String>;
    /// A colour picked anywhere on screen, as "#rrggbb". Ok(None): cancelled.
    fn pick_color(&self) -> Result<Option<String>, String>;
    /// Shows a file in Finder (`open`: opens it instead).
    fn reveal(&self, path: &Path, open: bool) -> Result<(), String>;
    /// Opens the web inspector (desktop only).
    fn devtools(&self) {}
    /// Who is signed in now (None: signed out). The web server keeps each person's settings and
    /// files apart by it; the desktop app has one person and ignores it.
    fn signed_in(&self, _login: Option<&str>) {}
    /// The settings file of the person in front: the computer's on the desktop, each person's on the server.
    fn settings_file(&self) -> Option<PathBuf> {
        self.data_dir().map(|d| d.join("settings.json"))
    }
    /// Where the browser version keeps the person's own files (saved reports, exports) — the files
    /// it may download and list. None on the desktop, where files go wherever the person saves them.
    fn files_dir(&self) -> Option<PathBuf> {
        None
    }
    /// Whether a document may be opened from this path (the server: only inside its own folder).
    fn may_open(&self, _path: &Path) -> bool {
        true
    }
}

/// A file dialog's filters, suggested name and starting folder.
#[derive(Clone, Debug, Default)]
pub struct FileSpec {
    pub filters: Vec<(String, Vec<String>)>,
    pub name: Option<String>,
    pub dir: Option<PathBuf>,
}

/// `host.dialog().file()…`: a file dialog built up as Tauri's is.
pub struct Dialogs<'a>(&'a dyn Host);

pub struct FileDialog<'a> {
    host: &'a dyn Host,
    spec: FileSpec,
}

/// A picked path.
pub struct Picked(PathBuf);

impl Picked {
    pub fn into_path(self) -> Result<PathBuf, String> {
        Ok(self.0)
    }
}

impl<'a> dyn Host + 'a {
    pub fn dialog(&'a self) -> Dialogs<'a> {
        Dialogs(self)
    }
}

impl<'a> Dialogs<'a> {
    pub fn file(self) -> FileDialog<'a> {
        FileDialog { host: self.0, spec: FileSpec::default() }
    }
}

impl FileDialog<'_> {
    pub fn add_filter(mut self, name: impl Into<String>, exts: &[&str]) -> Self {
        self.spec.filters.push((name.into(), exts.iter().map(|e| e.to_string()).collect()));
        self
    }
    pub fn set_file_name(mut self, name: impl Into<String>) -> Self {
        self.spec.name = Some(name.into());
        self
    }
    pub fn set_directory(mut self, dir: impl AsRef<Path>) -> Self {
        self.spec.dir = Some(dir.as_ref().to_path_buf());
        self
    }
    pub fn blocking_pick_file(self) -> Result<Option<Picked>, String> {
        Ok(self.host.pick_file(&self.spec)?.map(Picked))
    }
    pub fn blocking_save_file(self) -> Result<Option<Picked>, String> {
        Ok(self.host.save_file(&self.spec)?.map(Picked))
    }
    pub fn blocking_pick_folder(self) -> Result<Option<Picked>, String> {
        Ok(self.host.pick_folder()?.map(Picked))
    }
}

/// A host's handle, as commands hold it.
pub type H<'a> = &'a dyn Host;
