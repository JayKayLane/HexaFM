//! The sound library: every sound with its category, tags, rating, folders and history.

use crate::fx::PlayParams;
use crate::patch::{read_syx, Voice, VOICE_SIZE};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const FACTORY_BANKS: &[(&str, &[u8])] = &[
    ("Dexed 01", include_bytes!("../assets/factory/Dexed_01.syx")),
    ("SynprezFM 01", include_bytes!("../assets/factory/SynprezFM_01.syx")),
    ("SynprezFM 02", include_bytes!("../assets/factory/SynprezFM_02.syx")),
    ("SynprezFM 03", include_bytes!("../assets/factory/SynprezFM_03.syx")),
    ("SynprezFM 04", include_bytes!("../assets/factory/SynprezFM_04.syx")),
    ("SynprezFM 05", include_bytes!("../assets/factory/SynprezFM_05.syx")),
    ("SynprezFM 06", include_bytes!("../assets/factory/SynprezFM_06.syx")),
    ("SynprezFM 07", include_bytes!("../assets/factory/SynprezFM_07.syx")),
    ("SynprezFM 08", include_bytes!("../assets/factory/SynprezFM_08.syx")),
    ("SynprezFM 09", include_bytes!("../assets/factory/SynprezFM_09.syx")),
    ("SynprezFM 10", include_bytes!("../assets/factory/SynprezFM_10.syx")),
    ("SynprezFM 11", include_bytes!("../assets/factory/SynprezFM_11.syx")),
    ("SynprezFM 12", include_bytes!("../assets/factory/SynprezFM_12.syx")),
    ("SynprezFM 13", include_bytes!("../assets/factory/SynprezFM_13.syx")),
    ("SynprezFM 14", include_bytes!("../assets/factory/SynprezFM_14.syx")),
    ("SynprezFM 15", include_bytes!("../assets/factory/SynprezFM_15.syx")),
    ("SynprezFM 16", include_bytes!("../assets/factory/SynprezFM_16.syx")),
    ("SynprezFM 17", include_bytes!("../assets/factory/SynprezFM_17.syx")),
    ("SynprezFM 18", include_bytes!("../assets/factory/SynprezFM_18.syx")),
    ("SynprezFM 19", include_bytes!("../assets/factory/SynprezFM_19.syx")),
    ("SynprezFM 20", include_bytes!("../assets/factory/SynprezFM_20.syx")),
    ("SynprezFM 21", include_bytes!("../assets/factory/SynprezFM_21.syx")),
    ("SynprezFM 22", include_bytes!("../assets/factory/SynprezFM_22.syx")),
    ("SynprezFM 23", include_bytes!("../assets/factory/SynprezFM_23.syx")),
    ("SynprezFM 24", include_bytes!("../assets/factory/SynprezFM_24.syx")),
    ("SynprezFM 25", include_bytes!("../assets/factory/SynprezFM_25.syx")),
    ("SynprezFM 26", include_bytes!("../assets/factory/SynprezFM_26.syx")),
    ("SynprezFM 27", include_bytes!("../assets/factory/SynprezFM_27.syx")),
    ("SynprezFM 28", include_bytes!("../assets/factory/SynprezFM_28.syx")),
    ("SynprezFM 29", include_bytes!("../assets/factory/SynprezFM_29.syx")),
    ("SynprezFM 30", include_bytes!("../assets/factory/SynprezFM_30.syx")),
    ("SynprezFM 31", include_bytes!("../assets/factory/SynprezFM_31.syx")),
    ("SynprezFM 32", include_bytes!("../assets/factory/SynprezFM_32.syx")),
];

/// Banks the owner added to their library and asked to ship with the app.
pub const INCLUDED_BANKS: &[(&str, &[u8])] = &[
    ("dx1", include_bytes!("../assets/added/dx1.syx")),
    ("ROM1A", include_bytes!("../assets/added/ROM1A.syx")),
    ("ROM1B", include_bytes!("../assets/added/ROM1B.syx")),
    ("ROM2A", include_bytes!("../assets/added/ROM2A.syx")),
    ("ROM2B", include_bytes!("../assets/added/ROM2B.syx")),
    ("ROM3A", include_bytes!("../assets/added/ROM3A.syx")),
    ("ROM3B", include_bytes!("../assets/added/ROM3B.syx")),
    ("ROM4A", include_bytes!("../assets/added/ROM4A.syx")),
    ("ROM4B", include_bytes!("../assets/added/ROM4B.syx")),
];

/// Every bank built into the app.
pub fn built_in_banks() -> impl Iterator<Item = &'static (&'static str, &'static [u8])> {
    FACTORY_BANKS.iter().chain(INCLUDED_BANKS.iter())
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    Piano,
    Keys,
    Organ,
    Bass,
    Brass,
    Strings,
    Pad,
    Lead,
    Bells,
    Mallets,
    Plucked,
    Wind,
    Voice,
    Percussion,
    Effects,
    Other,
}

impl Category {
    pub const ALL: [Category; 16] = [
        Category::Piano,
        Category::Keys,
        Category::Organ,
        Category::Bass,
        Category::Brass,
        Category::Strings,
        Category::Pad,
        Category::Lead,
        Category::Bells,
        Category::Mallets,
        Category::Plucked,
        Category::Wind,
        Category::Voice,
        Category::Percussion,
        Category::Effects,
        Category::Other,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Category::Piano => "Piano",
            Category::Keys => "Keys",
            Category::Organ => "Organ",
            Category::Bass => "Bass",
            Category::Brass => "Brass",
            Category::Strings => "Strings",
            Category::Pad => "Pad",
            Category::Lead => "Lead",
            Category::Bells => "Bells",
            Category::Mallets => "Mallets",
            Category::Plucked => "Plucked",
            Category::Wind => "Wind",
            Category::Voice => "Voice",
            Category::Percussion => "Percussion",
            Category::Effects => "Effects",
            Category::Other => "Other",
        }
    }

    /// Name first; if the name says nothing, look at how the carriers behave.
    pub fn guess_voice(name: &str, v: &Voice) -> Category {
        let c = Self::guess(name);
        if c != Category::Other {
            return c;
        }
        let alg = (v.get(crate::patch::ALGORITHM) & 31) as usize;
        let carriers: Vec<usize> = (0..6).filter(|&o| crate::fm::is_carrier(alg, o) && v.op(o, crate::patch::OP_LEVEL) > 50).collect();
        if carriers.is_empty() {
            return Category::Other;
        }
        let avg = |p: usize| carriers.iter().map(|&o| v.op(o, p) as f32).sum::<f32>() / carriers.len() as f32;
        let r1 = avg(crate::patch::OP_R1);
        let l3 = avg(crate::patch::OP_L1 + 2);
        let r2 = avg(crate::patch::OP_R1 + 1);
        if r1 < 55.0 && l3 > 70.0 {
            Category::Pad
        } else if r1 > 80.0 && l3 < 30.0 && r2 > 25.0 {
            Category::Plucked
        } else {
            Category::Other
        }
    }

    /// Guess a category from a sound's name.
    pub fn guess(name: &str) -> Category {
        let n = name.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
        if has(&["e.p", "ep ", "rhodes", "wurl", "tine", "e-p", "epian", "e.pno", "clav", "harpsi", "celest", "dyno", "elpno", "e piano"]) {
            Category::Keys
        } else if has(&["piano", "pno", "grand", "upright"]) {
            Category::Piano
        } else if has(&["organ", "orgn", "orga", "b3", "hammond", "pipe", "church", "drawb", "perc org", "farf"]) {
            Category::Organ
        } else if has(&["bass", "bas ", "bss", "slap", "fretl", "sub "]) {
            Category::Bass
        } else if has(&["brass", "brs", "horn", "trump", "trp", "tromb", "tuba", "sax", "bone"]) {
            Category::Brass
        } else if has(&["string", "str ", "strng", "strg", "violin", "viola", "cello", "orch", "ensemb", "pizz", "fiddle"]) {
            Category::Strings
        } else if has(&["pad", "warm", "atmos", "space", "sweep", "glass", "air ", "dream", "cloud", "wash"]) {
            Category::Pad
        } else if has(&["lead", "solo", "sync", "saw", "square", "mono", "synth", "syn "]) {
            Category::Lead
        } else if has(&["bell", "chime", "gong", "tubul", "glock", "bel ", "carill"]) {
            Category::Bells
        } else if has(&["marimba", "vibe", "xylo", "kalimba", "mallet", "steel", "balaf", "log drum", "vibra"]) {
            Category::Mallets
        } else if has(&["guitar", "gtr", "harp", "koto", "pluck", "sitar", "banjo", "mando", "lute", "shami", "jazz g"]) {
            Category::Plucked
        } else if has(&["flute", "flt", "oboe", "clarin", "bassoon", "pan ", "recorder", "whistle", "pipe", "shaku", "harmonica", "accord"]) {
            Category::Wind
        } else if has(&["voice", "vox", "choir", "chor", "vocal", "aah", "ooh"]) {
            Category::Voice
        } else if has(&["drum", "perc", "snare", "kick", "tom", "cymb", "hat", "conga", "timp", "clap"]) {
            Category::Percussion
        } else if has(&["fx", "sfx", "noise", "wind ", "ufo", "laser", "rain", "storm", "zap", "bird", "train", "heli", "whoosh"]) {
            Category::Effects
        } else {
            Category::Other
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Source {
    Factory(String),
    Imported(String),
    User,
}

impl Source {
    pub fn label(&self) -> String {
        match self {
            Source::Factory(b) => b.clone(),
            Source::Imported(b) => b.clone(),
            Source::User => "User".into(),
        }
    }
    pub fn bank(&self) -> Option<&str> {
        match self {
            Source::Factory(b) | Source::Imported(b) => Some(b),
            Source::User => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preset {
    pub id: u64,
    pub name: String,
    pub voice: Vec<u8>,
    pub play: PlayParams,
    pub category: Category,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub rating: u8,
    pub notes: String,
    pub author: String,
    pub source: Source,
    pub slot: Option<u8>,
    pub folders: Vec<u64>,
    pub created: u64,
    pub modified: u64,
    pub last_played: u64,
    pub play_count: u32,
    pub trashed: bool,
}

impl Default for Preset {
    fn default() -> Self {
        Preset {
            id: 0,
            name: "Init Voice".into(),
            voice: Voice::init().0.to_vec(),
            play: PlayParams::default(),
            category: Category::Other,
            tags: Vec::new(),
            favorite: false,
            rating: 0,
            notes: String::new(),
            author: String::new(),
            source: Source::User,
            slot: None,
            folders: Vec::new(),
            created: 0,
            modified: 0,
            last_played: 0,
            play_count: 0,
            trashed: false,
        }
    }
}

impl Preset {
    pub fn voice(&self) -> Voice {
        let mut d = [0u8; VOICE_SIZE];
        for (i, b) in self.voice.iter().take(VOICE_SIZE).enumerate() {
            d[i] = *b;
        }
        let mut v = Voice(d);
        v.clamp();
        v
    }
    pub fn is_factory(&self) -> bool {
        matches!(self.source, Source::Factory(_))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Folder {
    pub id: u64,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Library {
    pub version: u32,
    pub presets: Vec<Preset>,
    pub folders: Vec<Folder>,
    pub next_id: u64,
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("Hexa FM")
}

/// Keep user-facing names tidy: trim, collapse spaces, title-case shouty factory names.
pub fn display_name(raw: &str) -> String {
    let t = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() { "Untitled".into() } else { t }
}

/// Neater display names for sounds that come from banks: trims decoration such as
/// trailing slashes or dashes and softens ALL-CAPS, while keeping every word recognisable.
pub fn tidy_name(raw: &str) -> String {
    let s: String = raw.chars().map(|c| if c == '_' || !(' '..='~').contains(&c) { ' ' } else { c }).collect();
    let deco = |c: char| matches!(c, '\\' | '/' | '|' | '-' | '=' | '~' | '*' | '.' | ',' | ':' | ';' | '<' | '>' | '^' | '"' | '\'' | '`' | '#' | ' ' | '_' | '!' | '(' | ')' | '[' | ']');
    let mut t = s.trim_matches(|c| deco(c) && c != '(' && c != ')' && c != '[' && c != ']').to_string();
    // a lone bracket left at an end is decoration too
    t = t.trim_start_matches(|c| c == ')' || c == ']').trim_end_matches(|c| c == '(' || c == '[').trim().to_string();
    if t.is_empty() {
        return display_name(raw);
    }
    let words: Vec<&str> = t.split_whitespace().collect();
    let shouty = !t.chars().any(|c| c.is_ascii_lowercase()) && t.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 3;
    const KEEP: [&str; 22] = ["EP", "FM", "FX", "SFX", "OB", "DX", "TX", "PWM", "LFO", "CS", "SY", "JX", "JP", "MG", "AM", "PM", "TB", "TR", "OP", "VCF", "VCO", "II"];
    let out: Vec<String> = words
        .iter()
        .map(|w| {
            if !shouty || w.chars().any(|c| c.is_ascii_digit()) || KEEP.contains(w) {
                return w.to_string();
            }
            // capitalise after separators so E.PIANO becomes E.Piano
            let mut r = String::new();
            let mut start = true;
            for c in w.chars() {
                if c.is_ascii_alphabetic() {
                    r.push(if start { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() });
                    start = false;
                } else {
                    r.push(c);
                    start = matches!(c, '.' | '-' | '/' | '&' | '+');
                }
            }
            r
        })
        .collect();
    out.join(" ")
}

impl Library {
    pub fn path() -> PathBuf {
        data_dir().join("library.json")
    }

    pub fn load_or_seed() -> (Library, Option<String>) {
        let path = Self::path();
        if let Ok(text) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<Library>(&text) {
                Ok(mut lib) => {
                    if lib.version < 2 {
                        for p in lib.presets.iter_mut() {
                            if p.source != Source::User {
                                let raw = p.voice().name();
                                if p.name == display_name(&raw) {
                                    p.name = tidy_name(&raw);
                                }
                            }
                        }
                        lib.version = 2;
                    }
                    if lib.next_id == 0 {
                        lib.next_id = lib.presets.iter().map(|p| p.id).max().unwrap_or(0) + 1;
                    }
                    if lib.version < 3 {
                        // reverb defaults now follow the original (createParameterLayout 0x1001805d4)
                        let old = crate::fx::ReverbParams { on: true, amount: 0.18, size: 0.6, damping: 0.5, predelay_ms: 20.0 };
                        for p in lib.presets.iter_mut() {
                            if p.play.reverb == old {
                                p.play.reverb = crate::fx::ReverbParams::default();
                            }
                        }
                        lib.version = 3;
                    }
                    lib.add_missing_built_in();
                    return (lib, None);
                }
                Err(e) => {
                    let backup = path.with_extension("damaged.json");
                    let _ = std::fs::rename(&path, &backup);
                    let lib = Library::seeded();
                    return (lib, Some(format!("The library file could not be read ({e}). A fresh library was created; the old file was kept as {}.", backup.display())));
                }
            }
        }
        (Library::seeded(), None)
    }

    pub fn seeded() -> Library {
        let mut lib = Library { version: 3, presets: Vec::new(), folders: Vec::new(), next_id: 1 };
        lib.restore_factory();
        let fav = lib.add_folder("Live set");
        let _ = lib.add_folder("Sketches");
        // Put a few useful sounds in the example folder.
        let ids: Vec<u64> = lib.presets.iter().filter(|p| matches!(p.category, Category::Keys | Category::Brass | Category::Bass)).take(6).map(|p| p.id).collect();
        for id in ids {
            if let Some(p) = lib.get_mut(id) {
                p.folders.push(fav);
            }
        }
        lib
    }

    /// Adds any factory sound that is missing. Returns how many were added.
    /// Adds built-in banks that are new in this version. A bank already imported under the same
    /// name becomes built-in in place, keeping its favourites, ratings, tags and folders.
    pub fn add_missing_built_in(&mut self) -> usize {
        for (bank, _) in built_in_banks() {
            for p in self.presets.iter_mut() {
                if p.source == Source::Imported(bank.to_string()) {
                    p.source = Source::Factory(bank.to_string());
                }
            }
        }
        let trashed: Vec<(Source, Option<u8>)> = self.presets.iter().filter(|p| p.trashed).map(|p| (p.source.clone(), p.slot)).collect();
        let n = self.restore_factory_inner();
        // keep sounds the person deliberately moved to Trash
        for p in self.presets.iter_mut() {
            if trashed.contains(&(p.source.clone(), p.slot)) {
                p.trashed = true;
            }
        }
        n
    }

    /// Adds any factory sound that is missing and brings back trashed factory sounds.
    pub fn restore_factory(&mut self) -> usize {
        self.restore_factory_inner()
    }

    fn restore_factory_inner(&mut self) -> usize {
        let mut added = 0;
        let t = now();
        for (bank, bytes) in built_in_banks() {
            if let Ok(voices) = read_syx(bytes) {
                for (slot, v) in voices.into_iter().enumerate() {
                    let exists = self.presets.iter().any(|p| p.source == Source::Factory(bank.to_string()) && p.slot == Some(slot as u8));
                    if exists {
                        continue;
                    }
                    let name = tidy_name(&v.name());
                    let id = self.alloc_id();
                    self.presets.push(Preset {
                        id,
                        category: Category::guess_voice(&name, &v),
                        name,
                        voice: v.0.to_vec(),
                        source: Source::Factory(bank.to_string()),
                        slot: Some(slot as u8),
                        created: t,
                        modified: t,
                        ..Default::default()
                    });
                    added += 1;
                }
            }
        }
        // restore trashed factory sounds too
        for p in self.presets.iter_mut() {
            if p.is_factory() && p.trashed {
                p.trashed = false;
                added += 1;
            }
        }
        added
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        id
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("tmp");
        let text = serde_json::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    pub fn get(&self, id: u64) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id == id)
    }
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Preset> {
        self.presets.iter_mut().find(|p| p.id == id)
    }

    pub fn add_folder(&mut self, name: &str) -> u64 {
        let id = self.alloc_id();
        self.folders.push(Folder { id, name: name.to_string() });
        id
    }

    pub fn folder_name(&self, id: u64) -> Option<&str> {
        self.folders.iter().find(|f| f.id == id).map(|f| f.name.as_str())
    }

    pub fn banks(&self) -> Vec<(String, bool, usize)> {
        let mut out: Vec<(String, bool, usize)> = Vec::new();
        for p in self.presets.iter().filter(|p| !p.trashed) {
            if let Some(b) = p.source.bank() {
                if let Some(e) = out.iter_mut().find(|e| e.0 == b) {
                    e.2 += 1;
                } else {
                    out.push((b.to_string(), p.is_factory(), 1));
                }
            }
        }
        out
    }

    pub fn all_tags(&self) -> Vec<String> {
        let mut t: Vec<String> = self.presets.iter().filter(|p| !p.trashed).flat_map(|p| p.tags.iter().cloned()).collect();
        t.sort_by_key(|s| s.to_lowercase());
        t.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        t
    }

    /// Import every voice from a .syx file. Returns (count, bank name).
    pub fn import_syx(&mut self, path: &std::path::Path, folder: Option<u64>) -> Result<(usize, String), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("Unable to read {}: {e}", path.display()))?;
        let voices = read_syx(&bytes)?;
        let mut bank = path.file_stem().map(|s| s.to_string_lossy().replace('_', " ")).unwrap_or_else(|| "Imported".into());
        // keep bank names unique
        let base = bank.clone();
        let mut n = 2;
        while self.presets.iter().any(|p| p.source.bank() == Some(bank.as_str())) {
            bank = format!("{base} ({n})");
            n += 1;
        }
        let t = now();
        let count = voices.len();
        for (slot, v) in voices.into_iter().enumerate() {
            let name = tidy_name(&v.name());
            let id = self.alloc_id();
            self.presets.push(Preset {
                id,
                category: Category::guess_voice(&name, &v),
                name,
                voice: v.0.to_vec(),
                source: Source::Imported(bank.clone()),
                slot: Some(slot as u8),
                folders: folder.into_iter().collect(),
                created: t,
                modified: t,
                ..Default::default()
            });
        }
        Ok((count, bank))
    }

    /// Import a single sound saved by this app (.hexafm JSON).
    pub fn import_sound(&mut self, path: &std::path::Path) -> Result<u64, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("Unable to read {}: {e}", path.display()))?;
        let mut p: Preset = serde_json::from_str(&text).map_err(|_| "This file is not a valid Hexa FM sound.".to_string())?;
        p.id = self.alloc_id();
        p.source = Source::User;
        p.folders.retain(|f| self.folders.iter().any(|x| x.id == *f));
        p.trashed = false;
        let id = p.id;
        self.presets.push(p);
        Ok(id)
    }

    pub fn empty_trash(&mut self) -> usize {
        let before = self.presets.len();
        self.presets.retain(|p| !p.trashed);
        before - self.presets.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fm::{FmEngine, BLOCK};
    #[test]
    fn factory_levels() {
        let mut peaks = Vec::new();
        for (bank, bytes) in FACTORY_BANKS {
            let voices = read_syx(bytes).unwrap();
            assert_eq!(voices.len(), 32, "{bank}");
            for v in voices {
                let mut e = FmEngine::new(48000.0);
                e.patch = v.clone();
                for n in [48u8, 55, 60, 64] {
                    e.note_on(n, 100);
                }
                let mut buf = vec![0.0f32; BLOCK];
                let mut peak = 0.0f32;
                for _ in 0..750 {
                    buf.fill(0.0);
                    e.render_block(&mut buf);
                    for s in &buf {
                        assert!(s.is_finite());
                        peak = peak.max(s.abs());
                    }
                }
                peaks.push((peak, v.name()));
            }
        }
        peaks.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let n = peaks.len();
        println!("min {:?} p10 {:?} median {:?} p90 {:?} max {:?}", peaks[0], peaks[n / 10], peaks[n / 2], peaks[n * 9 / 10], peaks[n - 1]);
        let cats: std::collections::BTreeMap<_, usize> = FACTORY_BANKS.iter().flat_map(|(_, b)| read_syx(b).unwrap()).fold(Default::default(), |mut m, v| { *m.entry(Category::guess_voice(&v.name(), &v).label()).or_default() += 1; m });
        println!("{cats:?}");
        for raw in ["Chroma 5 \\", "SloSwl //\\", "OB Genviv\\", "-ANALOG 1-", "P.ICE 25.1", "LFORez ++\\", "SAW EM UP", "Say Again.", "E.PIANO 1", "LAURIE", "DX7 BRASS", "==PAD==", "Lavitar \\"] {
            println!("{raw:>14} -> {}", tidy_name(raw));
        }
        let mut odd = 0;
        for (_, b) in FACTORY_BANKS { for v in read_syx(b).unwrap() { let t = tidy_name(&v.name()); if t.len() < 2 { odd += 1; println!("short: {:?} -> {:?}", v.name(), t); } } }
        println!("short names: {odd}");
    }
}
