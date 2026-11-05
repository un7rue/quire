//! The font store shared by text measurement and export: the bundled Inter,
//! every font installed on the system, and fonts embedded in the open document.

use crate::text::{INTER_BOLD, INTER_MEDIUM, INTER_REGULAR, INTER_SEMIBOLD};
use resvg::usvg::fontdb::{self, Database, Family, Query, Stretch, Style, Weight};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

pub const DEFAULT_FAMILY: &str = "Inter";

/// Glyph advances for one face, read lazily.
pub struct Face {
    data: Arc<Vec<u8>>,
    index: u32,
    upem: f64,
    advances: Mutex<HashMap<char, Option<f64>>>,
}

impl Face {
    /// Advance of `ch` in em units, `None` when the face lacks the glyph.
    pub fn advance(&self, ch: char) -> Option<f64> {
        if let Some(a) = self.advances.lock().unwrap().get(&ch) {
            return *a;
        }
        let a = ttf_parser::Face::parse(&self.data, self.index)
            .ok()
            .and_then(|f| f.glyph_index(ch).and_then(|g| f.glyph_hor_advance(g)))
            .map(|a| a as f64 / self.upem);
        self.advances.lock().unwrap().insert(ch, a);
        a
    }
}

struct Store {
    db: Database,
    /// Database snapshot handed to usvg; rebuilt when fonts are added.
    shared: Arc<Database>,
    faces: HashMap<(String, u16, bool), Arc<Face>>,
    embedded: HashSet<String>,
}

fn store() -> &'static RwLock<Store> {
    static S: OnceLock<RwLock<Store>> = OnceLock::new();
    S.get_or_init(|| {
        let mut db = Database::new();
        for f in [INTER_REGULAR, INTER_MEDIUM, INTER_SEMIBOLD, INTER_BOLD] {
            db.load_font_data(f.to_vec());
        }
        for f in crate::text::BUNDLED.iter().chain(crate::text::private::FONTS) {
            db.load_font_data(f.to_vec());
        }
        if std::env::var("QUIRE_NO_SYSTEM_FONTS").is_err() {
            db.load_system_fonts();
        }
        db.set_sans_serif_family(DEFAULT_FAMILY);
        let shared = Arc::new(db.clone());
        RwLock::new(Store { db, shared, faces: HashMap::new(), embedded: HashSet::new() })
    })
}

/// Loads the store in the background so the first measurement doesn't wait.
pub fn warm_up() {
    std::thread::spawn(|| {
        let _ = store();
    });
}

pub fn database() -> Arc<Database> {
    store().read().unwrap().shared.clone()
}

/// The closest face for a family/weight/style, falling back to Inter.
pub fn face(family: &str, weight: u16, italic: bool) -> Arc<Face> {
    let key = (family.to_string(), weight, italic);
    if let Some(f) = store().read().unwrap().faces.get(&key) {
        return f.clone();
    }
    let mut s = store().write().unwrap();
    let style = if italic { Style::Italic } else { Style::Normal };
    let find = |db: &Database, fam: &str| {
        let fams = [Family::Name(fam)];
        db.query(&Query { families: &fams, weight: Weight(weight), stretch: Stretch::Normal, style })
    };
    let id = find(&s.db, family).or_else(|| find(&s.db, DEFAULT_FAMILY));
    let face = id
        .and_then(|id| s.db.with_face_data(id, |data, index| (data.to_vec(), index)))
        .and_then(|(data, index)| {
            let upem = ttf_parser::Face::parse(&data, index).ok()?.units_per_em() as f64;
            Some(Face { data: Arc::new(data), index, upem, advances: Mutex::new(HashMap::new()) })
        })
        .unwrap_or_else(|| Face {
            data: Arc::new(INTER_REGULAR.to_vec()),
            index: 0,
            upem: 2048.0,
            advances: Mutex::new(HashMap::new()),
        });
    let face = Arc::new(face);
    s.faces.insert(key, face.clone());
    face
}

/// Adds a font file's faces; returns the family name(s) it provides.
pub fn add(data: Vec<u8>) -> Result<Vec<String>, String> {
    let n = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
    let mut names = Vec::new();
    for i in 0..n {
        let f = ttf_parser::Face::parse(&data, i).map_err(|_| "That file is not a TrueType or OpenType font.".to_string())?;
        if let Some(name) = family_name(&f) {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    if names.is_empty() {
        return Err("The font has no family name.".into());
    }
    let mut s = store().write().unwrap();
    s.db.load_font_data(data);
    for n in &names {
        s.embedded.insert(n.clone());
    }
    s.faces.retain(|k, _| !names.contains(&k.0));
    s.shared = Arc::new(s.db.clone());
    Ok(names)
}

fn family_name(f: &ttf_parser::Face) -> Option<String> {
    use ttf_parser::name_id;
    let names = f.names();
    let pick = |id: u16| {
        names
            .into_iter()
            .filter(|n| n.name_id == id && n.is_unicode())
            .find_map(|n| n.to_string())
    };
    pick(name_id::TYPOGRAPHIC_FAMILY).or_else(|| pick(name_id::FAMILY))
}

#[derive(Serialize)]
pub struct FamilyInfo {
    pub family: String,
    pub weights: Vec<u16>,
    pub italic: bool,
    pub monospace: bool,
    pub embedded: bool,
}

/// Every family available for reports, sorted by name.
pub fn families() -> Vec<FamilyInfo> {
    let s = store().read().unwrap();
    let mut by: BTreeMap<String, FamilyInfo> = BTreeMap::new();
    for f in s.db.faces() {
        let Some((name, _)) = f.families.first() else { continue };
        if name.starts_with('.') {
            continue; // hidden system UI faces
        }
        let e = by.entry(name.clone()).or_insert_with(|| FamilyInfo {
            family: name.clone(),
            weights: Vec::new(),
            italic: false,
            monospace: f.monospaced,
            embedded: s.embedded.contains(name),
        });
        if !e.weights.contains(&f.weight.0) {
            e.weights.push(f.weight.0);
        }
        if f.style != fontdb::Style::Normal {
            e.italic = true;
        }
    }
    let mut v: Vec<FamilyInfo> = by.into_values().collect();
    for f in &mut v {
        f.weights.sort();
    }
    v
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_built_in_look_has_its_typefaces_bundled() {
        let fams = super::families();
        for (id, l) in crate::uhnwi_theme::LOOKS {
            for (f, w) in [(l.display, l.display_weight), (l.ui, 400)] {
                let found = fams.iter().find(|x| x.family == f);
                assert!(found.is_some_and(|x| x.weights.contains(&w)), "{id}: {f} {w} not bundled");
            }
        }
    }
}
