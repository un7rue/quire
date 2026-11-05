use std::{env, fs, path::Path};

fn main() {
    private_fonts();
    // only the desktop app is a Tauri app; the web server builds without it
    #[cfg(feature = "desktop")]
    tauri_build::build()
}

/// A client's own typeface can't be published with the source. Its files go in
/// ui/fonts/private/ (kept out of git), with `families.txt` saying which themes use it:
///   green = Display Family, Text Family
///   graphite = Display Family, Text Family
/// They're embedded when present; without them those themes use the bundled fonts.
fn private_fonts() {
    println!("cargo:rerun-if-changed=ui/fonts/private");
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("ui/fonts/private");
    let mut files: Vec<String> = fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "ttf" || x == "otf")).map(|p| p.display().to_string()).collect())
        .unwrap_or_default();
    files.sort();
    let fonts = files.iter().map(|p| format!("include_bytes!({p:?})")).collect::<Vec<_>>().join(", ");
    // theme id → (display family, text family)
    let families: Vec<String> = fs::read_to_string(dir.join("families.txt"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (id, fams) = l.split_once('=')?;
            let (d, t) = fams.split_once(',')?;
            Some(format!("({:?}, {:?}, {:?})", id.trim(), d.trim(), t.trim()))
        })
        .collect();
    let out = format!("pub static FONTS: &[&[u8]] = &[{fonts}];\npub const FAMILIES: &[(&str, &str, &str)] = &[{}];\n", families.join(", "));
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("private_fonts.rs"), out).unwrap();
}
