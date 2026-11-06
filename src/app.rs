//! Main window: header, library navigation, sound browser, pages and keyboard.

use crate::audio::{self, AudioOut, MidiIn, Synth, UiEvent};
use crate::fx::PlayParams;
use crate::library::{self, now, Category, Library, Preset, Source};
use crate::patch::{self, Voice};
use crate::theme::{category_color, Theme, ThemeId};
use crate::widgets;
use egui::{self, vec2, Align, Color32, CornerRadius, Key, Layout, Margin, RichText, Sense, Stroke, StrokeKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeId,
    pub ui_scale: f32,
    pub audio_device: Option<String>,
    pub midi_port: Option<String>,
    pub midi_channel: u8,
    pub next_cc: Option<u8>,
    pub prev_cc: Option<u8>,
    pub program_change: bool,
    pub bend_up: f32,
    pub bend_down: f32,
    pub wheel_pitch: f32,
    pub wheel_amp: f32,
    pub portamento: f32,
    pub mono: bool,
    pub master_db: f32,
    pub master_tune: f32,
    pub last_preset: Option<u64>,
    pub kb_octave: i32,
    pub show_keyboard: bool,
    pub text_size: f32,
    pub text_grows: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: ThemeId::DxClassic,
            ui_scale: 1.0,
            audio_device: None,
            midi_port: None,
            midi_channel: 0,
            next_cc: None,
            prev_cc: None,
            program_change: true,
            bend_up: 2.0,
            bend_down: 2.0,
            wheel_pitch: 0.5,
            wheel_amp: 0.0,
            portamento: 0.0,
            mono: false,
            master_db: -3.0,
            master_tune: 0.0,
            last_preset: None,
            kb_octave: 4,
            show_keyboard: true,
            text_size: 1.0,
            text_grows: true,
        }
    }
}

impl Settings {
    fn path() -> PathBuf {
        library::data_dir().join("settings.json")
    }
    fn load() -> Settings {
        std::fs::read_to_string(Self::path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }
    fn save(&self) {
        let _ = std::fs::create_dir_all(library::data_dir());
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(), t);
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LibView {
    All,
    Favorites,
    Recent,
    User,
    Bank(String),
    Folder(u64),
    Trash,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortKey {
    Name,
    Category,
    Source,
    Rating,
    RecentlyPlayed,
    MostPlayed,
    DateAdded,
}
impl SortKey {
    const ALL: [SortKey; 7] = [SortKey::Name, SortKey::Category, SortKey::Source, SortKey::Rating, SortKey::RecentlyPlayed, SortKey::MostPlayed, SortKey::DateAdded];
    fn label(self) -> &'static str {
        match self {
            SortKey::Name => "Name",
            SortKey::Category => "Category",
            SortKey::Source => "Bank / slot",
            SortKey::Rating => "Rating",
            SortKey::RecentlyPlayed => "Recently played",
            SortKey::MostPlayed => "Most played",
            SortKey::DateAdded => "Date added",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Play,
    Edit,
    Effects,
    Settings,
}

pub enum Dialog {
    SaveAs { name: String, category: Category, folder: Option<u64> },
    Rename { id: u64, name: String },
    NewFolder { name: String, add: Option<u64> },
    RenameFolder { id: u64, name: String },
    DeleteFolder { id: u64 },
    Unsaved { then: PendingAction },
    EmptyTrash,
    ImportBanks { paths: Vec<PathBuf>, folder: Option<u64>, new_folder: String },
}

#[derive(Clone, Copy, Debug)]
pub enum PendingAction {
    Load(u64),
}

pub struct EditBuffer {
    pub voice: Voice,
    pub play: PlayParams,
    pub name: String,
}

pub struct App {
    /// running inside a plugin host (no own audio device or MIDI port)
    pub plugin: bool,
    pub settings: Settings,
    pub theme: Theme,
    applied_theme: Option<(ThemeId, f32, f32)>,
    pub synth: Arc<Mutex<Synth>>,
    audio: Option<AudioOut>,
    midi: Option<MidiIn>,
    ui_rx: Receiver<UiEvent>,
    ui_tx: Sender<UiEvent>,
    pub lib: Library,
    lib_dirty: bool,
    last_lib_save: Instant,
    pub current: Option<u64>,
    pub edit: EditBuffer,
    pushed: Option<(Voice, PlayParams)>,
    pub comparing: bool,
    view: LibView,
    search: String,
    cat_filter: BTreeSet<Category>,
    tag_filter: Option<String>,
    min_rating: u8,
    sort: SortKey,
    filtered: Vec<u64>,
    /// centre the current sound in the list (startup, switching lists)
    scroll_to_current: bool,
    /// scroll only if the current sound is out of view (arrow keys, MIDI, saving)
    reveal_current: bool,
    list_offset: f32,
    list_view_h: f32,
    last_width: f32,
    width_changed_at: Instant,
    filter_key: Option<FilterKey>,
    nav_cache: Option<NavCache>,
    text_setting_applied: (f32, bool),
    pub tab: Tab,
    pub selected_op: usize,
    pub op_clipboard: Option<[u8; patch::OP_SIZE]>,
    dialog: Option<Dialog>,
    toasts: Vec<(String, Instant)>,
    ui_held: BTreeSet<u8>,
    midi_held: BTreeSet<u8>,
    mouse_note: Option<u8>,
    key_notes: Vec<(Key, u8)>,
    pub learn: Option<LearnTarget>,
    pub audio_devices: Vec<String>,
    pub midi_ports: Vec<String>,
    tag_input: String,
    screenshot: Option<PathBuf>,
    frame: u64,
    screenshot_requested: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LearnTarget {
    Next,
    Prev,
}

/// Width of the sound list (MECHANICS: preset list width 320 px).
const PRESET_LIST_WIDTH: f32 = 320.0;
/// Height of one row in the sound list (AVDexedPlayView ctor 0x1000a7424: setRowHeight(0x22)).
const PRESET_ROW_HEIGHT: f32 = 34.0;

const COMPUTER_KEYS: [(Key, i32); 17] = [
    (Key::A, 0),
    (Key::W, 1),
    (Key::S, 2),
    (Key::E, 3),
    (Key::D, 4),
    (Key::F, 5),
    (Key::T, 6),
    (Key::G, 7),
    (Key::Y, 8),
    (Key::H, 9),
    (Key::U, 10),
    (Key::J, 11),
    (Key::K, 12),
    (Key::O, 13),
    (Key::L, 14),
    (Key::P, 15),
    (Key::Semicolon, 16),
];

impl App {
    /// Standalone app: opens its own audio output and MIDI input.
    pub fn new(ctx: &egui::Context, screenshot: Option<PathBuf>) -> Self {
        Self::build(ctx, screenshot, None)
    }

    /// Inside a plugin host: the host supplies audio and MIDI, the synth is shared with the plugin.
    pub fn new_plugin(ctx: &egui::Context, synth: Arc<Mutex<Synth>>) -> Self {
        Self::build(ctx, None, Some(synth))
    }

    fn build(ctx: &egui::Context, screenshot: Option<PathBuf>, host_synth: Option<Arc<Mutex<Synth>>>) -> Self {
        let settings = Settings::load();
        let theme = Theme::get(settings.theme);
        theme.apply(ctx, settings.ui_scale);
        let (lib, warn) = Library::load_or_seed();
        let plugin = host_synth.is_some();
        let synth = host_synth.unwrap_or_else(|| Arc::new(Mutex::new(Synth::new(48000.0))));
        let (ui_tx, ui_rx) = channel();
        let audio = if screenshot.is_none() && !plugin { Some(audio::start_audio(synth.clone(), settings.audio_device.as_deref())) } else { None };
        let mut app = App {
            plugin,
            theme,
            applied_theme: None,
            synth,
            audio,
            midi: None,
            ui_rx,
            ui_tx,
            lib,
            lib_dirty: false,
            last_lib_save: Instant::now(),
            current: None,
            edit: EditBuffer { voice: Voice::init(), play: PlayParams::default(), name: "Init Voice".into() },
            pushed: None,
            comparing: false,
            view: LibView::All,
            search: String::new(),
            cat_filter: BTreeSet::new(),
            tag_filter: None,
            min_rating: 0,
            sort: SortKey::Source,
            filtered: Vec::new(),
            scroll_to_current: true,
            reveal_current: false,
            list_offset: 0.0,
            list_view_h: 0.0,
            last_width: 0.0,
            width_changed_at: Instant::now(),
            filter_key: None,
            nav_cache: None,
            text_setting_applied: (0.0, false),
            tab: Tab::Play,
            selected_op: 0,
            op_clipboard: None,
            dialog: None,
            toasts: Vec::new(),
            ui_held: BTreeSet::new(),
            midi_held: BTreeSet::new(),
            mouse_note: None,
            key_notes: Vec::new(),
            learn: None,
            audio_devices: Vec::new(),
            midi_ports: Vec::new(),
            tag_input: String::new(),
            screenshot,
            frame: 0,
            screenshot_requested: false,
            settings,
        };
        if app.screenshot.is_none() && !app.plugin {
            app.lib_dirty = true;
            app.audio_devices = audio::output_devices();
            app.midi_ports = audio::midi_ports();
            let wanted = app.settings.midi_port.clone().or_else(|| app.midi_ports.first().cloned());
            if let Some(p) = wanted {
                app.connect_midi(&p, false);
            }
        }
        if let Some(w) = warn {
            app.toast(w);
        }
        app.apply_controller_settings();
        let start = app.settings.last_preset.filter(|id| app.lib.get(*id).map(|p| !p.trashed).unwrap_or(false)).or_else(|| app.lib.presets.iter().find(|p| !p.trashed).map(|p| p.id));
        if let Some(id) = start {
            app.load_preset(id, false);
        }
        app
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.toasts.push((msg.into(), Instant::now()));
    }

    pub fn apply_controller_settings(&mut self) {
        let s = &self.settings;
        if let Ok(mut syn) = self.synth.lock() {
            syn.engine.ctrl.bend_up = s.bend_up;
            syn.engine.ctrl.bend_down = s.bend_down;
            syn.engine.ctrl.wheel_pitch = s.wheel_pitch;
            syn.engine.ctrl.wheel_amp = s.wheel_amp;
            syn.engine.ctrl.portamento = s.portamento;
            syn.engine.ctrl.master_tune = s.master_tune;
            syn.engine.mono = s.mono;
            syn.master_db = s.master_db;
            syn.midi_channel = s.midi_channel;
        }
    }

    pub fn connect_midi(&mut self, port: &str, announce: bool) {
        self.midi = None;
        match audio::connect_midi(port, self.synth.clone(), self.ui_tx.clone()) {
            Ok(m) => {
                self.midi = Some(m);
                self.settings.midi_port = Some(port.to_string());
                if announce {
                    self.toast(format!("MIDI input: {port}"));
                }
            }
            Err(e) => {
                if announce {
                    self.toast(e);
                }
            }
        }
    }

    pub fn disconnect_midi(&mut self) {
        self.midi = None;
        self.settings.midi_port = None;
    }

    pub fn midi_port_name(&self) -> Option<&str> {
        self.midi.as_ref().and_then(|m| m.port_name.as_deref())
    }

    /// Put back the sound a host project was saved with. If it matches a library sound, that
    /// sound is selected; otherwise it shows as an unsaved sound.
    pub fn restore_sound(&mut self, preset_id: Option<u64>, voice: Voice, play: PlayParams, name: String) {
        // by library id first (so edits still show as edits), then by exact match
        let by_id = preset_id.filter(|id| self.lib.get(*id).map(|p| !p.trashed).unwrap_or(false));
        let found = by_id.or_else(|| self.lib.presets.iter().find(|p| !p.trashed && p.voice == voice.0.to_vec() && p.name == name).map(|p| p.id));
        self.current = found;
        self.edit = EditBuffer { voice, play, name };
        self.reveal_current = true;
        self.pushed = None;
    }

    /// A sender the plugin uses to pass host MIDI to the window (lights keys, MIDI learn).
    pub fn event_sender(&self) -> Sender<UiEvent> {
        self.ui_tx.clone()
    }

    pub fn audio_status(&self) -> (String, u32, Option<String>) {
        if self.plugin {
            let sr = self.synth.lock().map(|s| s.sample_rate() as u32).unwrap_or(0);
            return ("Plugin · host audio".into(), sr, None);
        }
        match &self.audio {
            Some(a) => (a.device_name.clone(), a.sample_rate, a.error.clone()),
            None => ("Audio off".into(), 0, None),
        }
    }

    pub fn restart_audio(&mut self, device: Option<String>) {
        self.audio = None;
        self.settings.audio_device = device.clone();
        self.audio = Some(audio::start_audio(self.synth.clone(), device.as_deref()));
        self.pushed = None;
        self.apply_controller_settings();
    }

    pub fn test_tone(&self) {
        if let Ok(mut s) = self.synth.lock() {
            let sr = s.sample_rate() as u32;
            s.test_tone = sr;
        }
    }

    // ------------------------------------------------------------ presets

    pub fn is_dirty(&self) -> bool {
        match self.current.and_then(|id| self.lib.get(id)) {
            Some(p) => p.voice != self.edit.voice.0.to_vec() || p.play != self.edit.play || p.name != self.edit.name,
            None => true,
        }
    }

    fn request_load(&mut self, id: u64) {
        if Some(id) == self.current {
            return;
        }
        if self.is_dirty() && self.current.is_some() {
            self.dialog = Some(Dialog::Unsaved { then: PendingAction::Load(id) });
        } else {
            self.load_preset(id, true);
        }
    }

    pub fn load_preset(&mut self, id: u64, count_play: bool) {
        let Some(p) = self.lib.get_mut(id) else { return };
        if count_play {
            p.last_played = now();
            p.play_count += 1;
        }
        self.edit = EditBuffer { voice: p.voice(), play: p.play.clone(), name: p.name.clone() };
        self.current = Some(id);
        self.comparing = false;
        self.settings.last_preset = Some(id);
        self.lib_dirty = true;
        self.reveal_current = true;
        if let Ok(mut s) = self.synth.lock() {
            s.fx.clear();
        }
    }

    fn navigate(&mut self, delta: i32) {
        if self.filtered.is_empty() {
            return;
        }
        let pos = self.current.and_then(|c| self.filtered.iter().position(|&x| x == c));
        let n = self.filtered.len() as i32;
        let next = match pos {
            Some(i) => ((i as i32 + delta).rem_euclid(n)) as usize,
            None => 0,
        };
        let id = self.filtered[next];
        self.request_load(id);
    }

    pub fn save_current(&mut self) {
        let Some(id) = self.current else {
            self.open_save_as();
            return;
        };
        let factory = self.lib.get(id).map(|p| p.is_factory()).unwrap_or(true);
        if factory {
            self.open_save_as();
            return;
        }
        let mut voice = self.edit.voice.clone();
        voice.set_name(&self.edit.name);
        self.edit.voice = voice.clone();
        if let Some(p) = self.lib.get_mut(id) {
            p.voice = voice.0.to_vec();
            p.play = self.edit.play.clone();
            p.name = self.edit.name.clone();
            p.modified = now();
        }
        self.lib_dirty = true;
        self.toast(format!("Saved: {}", self.edit.name));
    }

    pub fn open_save_as(&mut self) {
        let (category, folder) = match self.current.and_then(|id| self.lib.get(id)) {
            Some(p) => (p.category, if let LibView::Folder(f) = self.view { Some(f) } else { p.folders.first().copied() }),
            None => (Category::Other, None),
        };
        let name = if self.current.and_then(|id| self.lib.get(id)).map(|p| p.is_factory()).unwrap_or(false) { format!("{} (my version)", self.edit.name) } else { format!("{} copy", self.edit.name) };
        self.dialog = Some(Dialog::SaveAs { name, category, folder });
    }

    fn save_as(&mut self, name: String, category: Category, folder: Option<u64>) {
        let name = library::display_name(&name);
        let mut voice = self.edit.voice.clone();
        voice.set_name(&name);
        let src = self.current.and_then(|id| self.lib.get(id)).cloned();
        let id = self.lib.alloc_id();
        let t = now();
        let p = Preset {
            id,
            name: name.clone(),
            voice: voice.0.to_vec(),
            play: self.edit.play.clone(),
            category,
            tags: src.as_ref().map(|s| s.tags.clone()).unwrap_or_default(),
            notes: src.as_ref().map(|s| if s.notes.is_empty() { format!("Based on {} ({})", s.name, s.source.label()) } else { s.notes.clone() }).unwrap_or_default(),
            source: Source::User,
            folders: folder.into_iter().collect(),
            created: t,
            modified: t,
            last_played: t,
            ..Default::default()
        };
        self.lib.presets.push(p);
        self.edit.voice = voice;
        self.edit.name = name.clone();
        self.current = Some(id);
        self.settings.last_preset = Some(id);
        self.lib_dirty = true;
        self.reveal_current = true;
        self.toast(format!("Saved as new sound: {name}"));
    }

    pub fn revert(&mut self) {
        if let Some(id) = self.current {
            if let Some(p) = self.lib.get(id) {
                self.edit = EditBuffer { voice: p.voice(), play: p.play.clone(), name: p.name.clone() };
                self.comparing = false;
            }
        }
    }

    fn duplicate(&mut self, id: u64) {
        if let Some(src) = self.lib.get(id).cloned() {
            let nid = self.lib.alloc_id();
            let t = now();
            let mut p = src.clone();
            p.id = nid;
            p.name = format!("{} copy", src.name);
            p.source = Source::User;
            p.slot = None;
            p.created = t;
            p.modified = t;
            p.play_count = 0;
            if p.notes.is_empty() {
                p.notes = format!("Copied from {} ({})", src.name, src.source.label());
            }
            self.lib.presets.push(p);
            self.lib_dirty = true;
            self.toast(format!("Duplicated {}", src.name));
        }
    }

    fn trash(&mut self, id: u64) {
        if let Some(p) = self.lib.get_mut(id) {
            p.trashed = true;
            let name = p.name.clone();
            self.lib_dirty = true;
            self.toast(format!("Moved to Trash: {name} — restore it from Trash"));
        }
    }

    fn export_syx(&mut self, ids: Vec<u64>, suggested: &str) {
        let voices: Vec<Voice> = ids
            .iter()
            .filter_map(|id| self.lib.get(*id))
            .map(|p| {
                let mut v = p.voice();
                v.set_name(&p.name);
                v
            })
            .collect();
        if voices.is_empty() {
            return;
        }
        let chunks: Vec<Vec<Voice>> = voices.chunks(32).map(|c| c.to_vec()).collect();
        let file = rfd::FileDialog::new().add_filter("SysEx bank", &["syx"]).set_file_name(format!("{suggested}.syx")).save_file();
        if let Some(path) = file {
            let mut written = 0;
            for (i, chunk) in chunks.iter().enumerate() {
                let p = if i == 0 { path.clone() } else { path.with_file_name(format!("{}_{}.syx", path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), i + 1)) };
                let bytes = if chunk.len() == 1 && chunks.len() == 1 && ids.len() == 1 { patch::write_single(&chunk[0]) } else { patch::write_bank(chunk) };
                match std::fs::write(&p, bytes) {
                    Ok(_) => written += 1,
                    Err(e) => self.toast(format!("Unable to write {}: {e}", p.display())),
                }
            }
            if written > 0 {
                self.toast(format!("Exported {} sound(s) to {} file(s)", voices.len(), written));
            }
        }
    }

    fn export_sound(&mut self, id: u64) {
        if let Some(p) = self.lib.get(id).cloned() {
            if let Some(path) = rfd::FileDialog::new().add_filter("Hexa FM sound", &["hexafm"]).set_file_name(format!("{}.hexafm", p.name)).save_file() {
                match serde_json::to_string_pretty(&p).map_err(|e| e.to_string()).and_then(|t| std::fs::write(&path, t).map_err(|e| e.to_string())) {
                    Ok(_) => self.toast(format!("Exported {}", p.name)),
                    Err(e) => self.toast(format!("Export failed: {e}")),
                }
            }
        }
    }

    fn pick_import(&mut self) {
        if let Some(paths) = rfd::FileDialog::new().add_filter("Banks and sounds", &["syx", "SYX", "hexafm"]).pick_files() {
            self.begin_import(paths);
        }
    }

    fn begin_import(&mut self, paths: Vec<PathBuf>) {
        let folder = if let LibView::Folder(f) = self.view { Some(f) } else { None };
        self.dialog = Some(Dialog::ImportBanks { paths, folder, new_folder: String::new() });
    }

    fn do_import(&mut self, paths: Vec<PathBuf>, folder: Option<u64>) {
        let mut ok = Vec::new();
        let mut bad = Vec::new();
        let mut first_bank = None;
        let mut last_id = None;
        for p in paths {
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if ext == "hexafm" {
                match self.lib.import_sound(&p) {
                    Ok(id) => {
                        if let Some(f) = folder {
                            if let Some(pp) = self.lib.get_mut(id) {
                                if !pp.folders.contains(&f) {
                                    pp.folders.push(f);
                                }
                            }
                        }
                        last_id = Some(id);
                        ok.push(p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
                    }
                    Err(_) => bad.push(p.display().to_string()),
                }
            } else {
                match self.lib.import_syx(&p, folder) {
                    Ok((n, bank)) => {
                        ok.push(format!("{bank} ({n})"));
                        first_bank.get_or_insert(bank);
                    }
                    Err(_) => bad.push(p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()),
                }
            }
        }
        self.lib_dirty = true;
        if !ok.is_empty() {
            self.toast(format!("Added to your library: {}", ok.join(", ")));
            if let Some(f) = folder {
                self.view = LibView::Folder(f);
            } else if let Some(b) = first_bank {
                self.view = LibView::Bank(b);
            }
            if let Some(id) = last_id {
                self.load_preset(id, true);
            }
        }
        if !bad.is_empty() {
            self.toast(format!("Could not import: {}. Only six-operator banks (.syx) and Hexa FM sounds (.hexafm) are accepted.", bad.join(", ")));
        }
    }

    // ------------------------------------------------------------ filtering

    fn rebuild_filter(&mut self) {
        let key = FilterKey {
            search: self.search.clone(),
            cats: self.cat_filter.iter().copied().collect(),
            tag: self.tag_filter.clone(),
            min_rating: self.min_rating,
            sort: self.sort,
            view: self.view.clone(),
        };
        if !self.lib_dirty && self.filter_key.as_ref() == Some(&key) && self.nav_cache.is_some() {
            return;
        }
        self.filter_key = Some(key);
        self.nav_cache = Some(NavCache::build(&self.lib));
        let q = self.search.trim().to_lowercase();
        let words: Vec<&str> = q.split_whitespace().collect();
        let mut list: Vec<&Preset> = self
            .lib
            .presets
            .iter()
            .filter(|p| match &self.view {
                LibView::Trash => p.trashed,
                _ if p.trashed => false,
                LibView::All => true,
                LibView::Favorites => p.favorite,
                LibView::Recent => p.last_played > 0,
                LibView::User => p.source == Source::User,
                LibView::Bank(b) => p.source.bank() == Some(b.as_str()),
                LibView::Folder(f) => p.folders.contains(f),
            })
            .filter(|p| self.cat_filter.is_empty() || self.cat_filter.contains(&p.category))
            .filter(|p| p.rating >= self.min_rating)
            .filter(|p| self.tag_filter.as_ref().map(|t| p.tags.iter().any(|x| x.eq_ignore_ascii_case(t))).unwrap_or(true))
            .filter(|p| {
                if words.is_empty() {
                    return true;
                }
                let hay = format!("{} {} {} {} {}", p.name, p.source.label(), p.category.label(), p.tags.join(" "), p.notes).to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .collect();
        let sort = if self.view == LibView::Recent { SortKey::RecentlyPlayed } else { self.sort };
        match sort {
            SortKey::Name => list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            SortKey::Category => list.sort_by(|a, b| a.category.cmp(&b.category).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))),
            SortKey::Source => list.sort_by(|a, b| {
                let ka = (matches!(a.source, Source::User), a.source.label(), a.slot.unwrap_or(255));
                let kb = (matches!(b.source, Source::User), b.source.label(), b.slot.unwrap_or(255));
                ka.cmp(&kb).then(a.id.cmp(&b.id))
            }),
            SortKey::Rating => list.sort_by(|a, b| b.rating.cmp(&a.rating).then(b.favorite.cmp(&a.favorite)).then(a.name.cmp(&b.name))),
            SortKey::RecentlyPlayed => list.sort_by(|a, b| b.last_played.cmp(&a.last_played)),
            SortKey::MostPlayed => list.sort_by(|a, b| b.play_count.cmp(&a.play_count).then(a.name.cmp(&b.name))),
            SortKey::DateAdded => list.sort_by(|a, b| b.created.cmp(&a.created).then(b.id.cmp(&a.id))),
        }
        if self.view == LibView::Recent {
            list.truncate(100);
        }
        self.filtered = list.into_iter().map(|p| p.id).collect();
    }

    // ------------------------------------------------------------ audio sync

    fn push_to_synth(&mut self) {
        let (voice, play) = if self.comparing {
            match self.current.and_then(|id| self.lib.get(id)) {
                Some(p) => (p.voice(), p.play.clone()),
                None => (self.edit.voice.clone(), self.edit.play.clone()),
            }
        } else {
            (self.edit.voice.clone(), self.edit.play.clone())
        };
        let changed = match &self.pushed {
            Some((v, p)) => *v != voice || *p != play,
            None => true,
        };
        if changed {
            if let Ok(mut s) = self.synth.lock() {
                s.engine.patch = voice.clone();
                s.play = play.clone();
            }
            self.pushed = Some((voice, play));
        }
    }

    pub fn note_on(&mut self, n: u8, vel: u8) {
        self.ui_held.insert(n);
        if let Ok(mut s) = self.synth.lock() {
            s.engine.note_on(n, vel);
        }
    }
    pub fn note_off(&mut self, n: u8) {
        self.ui_held.remove(&n);
        if let Ok(mut s) = self.synth.lock() {
            s.engine.note_off(n);
        }
    }
    pub fn panic(&mut self) {
        self.ui_held.clear();
        self.midi_held.clear();
        if let Ok(mut s) = self.synth.lock() {
            s.engine.all_notes_off();
            s.fx.clear();
        }
    }

    fn handle_events(&mut self) {
        while let Ok(ev) = self.ui_rx.try_recv() {
            match ev {
                UiEvent::Note(n, on) => {
                    if on {
                        self.midi_held.insert(n);
                    } else {
                        self.midi_held.remove(&n);
                    }
                }
                UiEvent::Cc(cc, v) => {
                    if let Some(t) = self.learn.take() {
                        match t {
                            LearnTarget::Next => self.settings.next_cc = Some(cc),
                            LearnTarget::Prev => self.settings.prev_cc = Some(cc),
                        }
                        self.toast(format!("Assigned CC {cc} to {}", if t == LearnTarget::Next { "next sound" } else { "previous sound" }));
                    } else if v >= 64 {
                        if Some(cc) == self.settings.next_cc {
                            self.navigate(1);
                        } else if Some(cc) == self.settings.prev_cc {
                            self.navigate(-1);
                        }
                    }
                }
                UiEvent::Program(pc) => {
                    if self.settings.program_change {
                        if let Some(&id) = self.filtered.get(pc as usize) {
                            self.request_load(id);
                        }
                    }
                }
            }
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::S)) && self.dialog.is_none() {
            self.save_current();
        }
        if ctx.wants_keyboard_input() || self.dialog.is_some() {
            return;
        }
        let base = (self.settings.kb_octave + 1) * 12;
        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            if let egui::Event::Key { key, pressed, repeat, modifiers, .. } = ev {
                if modifiers.command || modifiers.ctrl {
                    continue;
                }
                // Up/Down step through the list like the original (AVDexedPlayView::keyPressed
                // 0x1000ba8e4: only with no shift/ctrl/alt, and held keys repeat).
                if pressed && !modifiers.shift && !modifiers.alt && matches!(key, Key::ArrowDown | Key::ArrowUp) {
                    self.navigate(if key == Key::ArrowDown { 1 } else { -1 });
                    continue;
                }
                if repeat {
                    continue;
                }
                if let Some((_, off)) = COMPUTER_KEYS.iter().find(|(k, _)| *k == key) {
                    let n = (base + off).clamp(0, 127) as u8;
                    if pressed {
                        self.key_notes.push((key, n));
                        self.note_on(n, 100);
                    } else if let Some(pos) = self.key_notes.iter().position(|(k, _)| *k == key) {
                        let (_, n) = self.key_notes.remove(pos);
                        self.note_off(n);
                    }
                    continue;
                }
                if pressed {
                    match key {
                        Key::Z => self.settings.kb_octave = (self.settings.kb_octave - 1).max(0),
                        Key::X => self.settings.kb_octave = (self.settings.kb_octave + 1).min(8),
                        Key::Escape => self.panic(),
                        _ => {}
                    }
                }
            }
        }
    }
}

// ================================================================= drawing

pub fn card<R>(ui: &mut egui::Ui, t: &Theme, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                if !title.is_empty() {
                    ui.label(RichText::new(title.to_uppercase()).size(crate::theme::fs(11.0)).color(t.text_dim).strong());
                    ui.add_space(4.0);
                }
                add(ui)
            })
            .inner
        })
        .inner
}

fn stars(ui: &mut egui::Ui, t: &Theme, rating: &mut u8, size: f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        for i in 1..=5u8 {
            let filled = *rating >= i;
            let r = ui.add(egui::Label::new(RichText::new(if filled { "★" } else { "☆" }).size(crate::theme::fs(size)).color(if filled { t.star } else { t.text_faint })).sense(Sense::click()));
            if r.clicked() {
                *rating = if *rating == i { 0 } else { i };
                changed = true;
            }
            r.on_hover_text(format!("Rate {i} star{}", if i > 1 { "s" } else { "" }));
        }
    });
    changed
}

fn chip(ui: &mut egui::Ui, t: &Theme, text: &str, selected: bool, color: Option<Color32>) -> egui::Response {
    let font = crate::theme::fp(11.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font.clone(), t.text);
    let extra = if color.is_some() { 26.0 } else { 18.0 };
    let size = vec2(galley.size().x + extra, crate::theme::fs(22.0));
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let c = color.unwrap_or(t.accent);
    let (fill, stroke, txt) = if selected {
        (c.linear_multiply(0.3), Stroke::new(1.0, c), t.text)
    } else if resp.hovered() {
        (t.hover, Stroke::new(1.0, t.border), t.text)
    } else {
        (t.raised, Stroke::new(1.0, t.border), t.text_dim)
    };
    p.rect_filled(rect, CornerRadius::same(11), fill);
    p.rect_stroke(rect, CornerRadius::same(11), stroke, StrokeKind::Inside);
    if let Some(col) = color {
        p.circle_filled(egui::pos2(rect.left() + 10.0, rect.center().y), 2.5, col);
        p.text(egui::pos2(rect.center().x + 4.0, rect.center().y), egui::Align2::CENTER_CENTER, text, font, txt);
    } else {
        p.text(rect.center(), egui::Align2::CENTER_CENTER, text, font, txt);
    }
    resp
}

fn nav_item(ui: &mut egui::Ui, t: &Theme, icon: &str, label: &str, count: Option<usize>, selected: bool) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, crate::theme::fs(26.0)), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(4), t.raised);
        p.rect_filled(egui::Rect::from_min_size(rect.min, vec2(3.0, rect.height())), CornerRadius::same(1), t.accent);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(4), t.surface);
    }
    let col = if selected { t.text } else { t.text_dim };
    let ic = if selected { t.accent } else { t.text_faint };
    let c = egui::pos2(rect.left() + 17.0, rect.center().y);
    match icon {
        "all" => {
            for i in 0..3 {
                p.line_segment([c + vec2(-5.0, -4.0 + i as f32 * 4.0), c + vec2(5.0, -4.0 + i as f32 * 4.0)], Stroke::new(1.4, ic));
            }
        }
        "recent" => {
            p.circle_stroke(c, 5.0, Stroke::new(1.3, ic));
            p.line_segment([c, c + vec2(0.0, -3.5)], Stroke::new(1.3, ic));
            p.line_segment([c, c + vec2(2.5, 0.0)], Stroke::new(1.3, ic));
        }
        "user" => {
            p.circle_stroke(c + vec2(0.0, -2.5), 2.5, Stroke::new(1.3, ic));
            p.line_segment([c + vec2(-4.5, 5.0), c + vec2(4.5, 5.0)], Stroke::new(1.3, ic));
        }
        "folder" => {
            p.rect_stroke(egui::Rect::from_center_size(c + vec2(0.0, 1.0), vec2(11.0, 8.0)), CornerRadius::same(1), Stroke::new(1.3, ic), StrokeKind::Middle);
            p.line_segment([c + vec2(-5.5, -3.0), c + vec2(-1.5, -3.0)], Stroke::new(1.6, ic));
        }
        "bank" => {
            for i in 0..3 {
                p.rect_filled(egui::Rect::from_min_size(c + vec2(-5.0 + i as f32 * 4.0, -5.0), vec2(2.4, 10.0)), CornerRadius::ZERO, ic);
            }
        }
        "trash" => {
            p.rect_stroke(egui::Rect::from_center_size(c + vec2(0.0, 1.5), vec2(8.0, 9.0)), CornerRadius::same(1), Stroke::new(1.3, ic), StrokeKind::Middle);
            p.line_segment([c + vec2(-5.5, -3.5), c + vec2(5.5, -3.5)], Stroke::new(1.3, ic));
        }
        other => {
            p.text(c, egui::Align2::CENTER_CENTER, other, crate::theme::fp(12.0), ic);
        }
    }
    let max_w = rect.width() - 64.0;
    let mut lbl = label.to_string();
    while ui.painter().layout_no_wrap(lbl.clone(), crate::theme::fp(13.0), col).size().x > max_w && lbl.chars().count() > 3 {
        lbl = lbl.trim_end_matches('…').to_string();
        lbl.pop();
        lbl.push('…');
    }
    p.text(egui::pos2(rect.left() + 30.0, rect.center().y), egui::Align2::LEFT_CENTER, lbl, crate::theme::fp(13.0), col);
    if let Some(c) = count {
        p.text(egui::pos2(rect.right() - 8.0, rect.center().y), egui::Align2::RIGHT_CENTER, c.to_string(), crate::theme::fm(11.0), t.text_faint);
    }
    resp
}

fn section_label(ui: &mut egui::Ui, t: &Theme, text: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(text).size(crate::theme::fs(10.5)).color(t.text_faint).strong());
    ui.add_space(2.0);
}

impl App {
    /// Draw one frame and run the app logic.
    pub fn ui(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        // Text grows gently with the window (at most +15%), times the size chosen in Settings.
        // Re-sizing text means new glyphs and a new layout, so it waits until the window has
        // stopped changing size for a moment; while dragging, the current size is kept.
        let width = ctx.screen_rect().width();
        if (width - self.last_width).abs() > 0.5 {
            self.last_width = width;
            self.width_changed_at = Instant::now();
        }
        let settled = self.applied_theme.is_none() || self.width_changed_at.elapsed() > Duration::from_millis(250);
        let grow = if self.settings.text_grows { (1.0 + 0.25 * (width / 1360.0 - 1.0)).clamp(1.0, 1.15) } else { 1.0 };
        let wanted = ((self.settings.text_size.clamp(0.8, 1.4) * grow) / 0.05).round() * 0.05;
        let text_scale = match self.applied_theme {
            Some((th, sc, cur)) if !settled && th == self.settings.theme && sc == self.settings.ui_scale && self.text_setting_applied == (self.settings.text_size, self.settings.text_grows) => {
                ctx.request_repaint_after(Duration::from_millis(260));
                cur
            }
            _ => wanted,
        };
        self.text_setting_applied = (self.settings.text_size, self.settings.text_grows);
        if self.applied_theme != Some((self.settings.theme, self.settings.ui_scale, text_scale)) {
            crate::theme::set_text_scale(text_scale);
            self.theme = Theme::get(self.settings.theme);
            self.theme.apply(ctx, self.settings.ui_scale);
            self.applied_theme = Some((self.settings.theme, self.settings.ui_scale, text_scale));
        }
        self.handle_events();
        self.handle_keys(ctx);

        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() {
            self.begin_import(dropped);
        }

        self.rebuild_filter();
        let t = self.theme;

        self.header(ctx, &t);
        if self.settings.show_keyboard {
            self.keyboard_panel(ctx, &t);
        }
        self.nav_panel(ctx, &t);
        self.browser_panel(ctx, &t);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg).inner_margin(Margin::same(14))).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
                Tab::Play => crate::pages::play_page(self, ui),
                Tab::Edit => crate::pages::edit_page(self, ui),
                Tab::Effects => crate::pages::effects_page(self, ui),
                Tab::Settings => crate::pages::settings_page(self, ui),
            });
        });
        self.dialogs(ctx, &t);
        self.draw_toasts(ctx, &t);

        self.push_to_synth();

        if self.lib_dirty && self.screenshot.is_none() && self.last_lib_save.elapsed() > Duration::from_millis(800) {
            if let Err(e) = self.lib.save() {
                self.toast(format!("Could not save the library: {e}"));
            }
            self.settings.save();
            self.lib_dirty = false;
            self.last_lib_save = Instant::now();
        }

        if let Some(path) = self.screenshot.clone() {
            if self.frame >= 120 && !self.screenshot_requested {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.screenshot_requested = true;
            }
            let shots: Vec<Arc<egui::ColorImage>> = ctx.input(|i| i.raw.events.iter().filter_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None }).collect());
            if let Some(img) = shots.first() {
                if let Err(e) = save_png(&path, img) {
                    eprintln!("screenshot failed: {e}");
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }

    /// Save the library and settings (window closing, plugin editor closing).
    pub fn shutdown(&mut self) {
        if self.screenshot.is_none() {
            let _ = self.lib.save();
            self.settings.save();
        }
    }
}

fn save_png(path: &std::path::Path, img: &egui::ColorImage) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let [w, h] = img.size;
    let mut buf = Vec::with_capacity(w * h * 4);
    for c in &img.pixels {
        buf.extend_from_slice(&[c.r(), c.g(), c.b(), 255]);
    }
    image::save_buffer(path, &buf, w as u32, h as u32, image::ColorType::Rgba8).map_err(|e| e.to_string())
}

impl App {
    fn header(&mut self, ctx: &egui::Context, t: &Theme) {
        egui::TopBottomPanel::top("header").exact_height(crate::theme::fs(104.0)).frame(egui::Frame::new().fill(t.panel).inner_margin(Margin { left: 14, right: 14, top: 10, bottom: 0 })).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new("HEXA FM").size(crate::theme::fs(19.0)).strong().color(t.text).extra_letter_spacing(3.0));
                    let (r, _) = ui.allocate_exact_size(vec2(108.0, 6.0), Sense::hover());
                    let p = ui.painter();
                    let cols = [t.accent2, t.accent2.linear_multiply(0.75), t.accent, t.accent.linear_multiply(0.75)];
                    for (i, c) in cols.iter().enumerate() {
                        p.rect_filled(egui::Rect::from_min_size(egui::pos2(r.left() + i as f32 * 27.0, r.top()), vec2(25.0, 2.0)), CornerRadius::ZERO, *c);
                    }
                    ui.label(RichText::new("6-OPERATOR FM SYNTHESIZER").size(crate::theme::fs(9.0)).color(t.text_faint).extra_letter_spacing(1.0));
                });
                ui.add_space(18.0);

                if ui.add_sized([30.0, 44.0], egui::Button::new(RichText::new("◀").size(crate::theme::fs(14.0)))).on_hover_text("Previous sound (Up arrow)").clicked() {
                    self.navigate(-1);
                }
                self.lcd(ui, t);
                if ui.add_sized([30.0, 44.0], egui::Button::new(RichText::new("▶").size(crate::theme::fs(14.0)))).on_hover_text("Next sound (Down arrow)").clicked() {
                    self.navigate(1);
                }
                ui.add_space(6.0);
                if let Some(id) = self.current {
                    let fav = self.lib.get(id).map(|p| p.favorite).unwrap_or(false);
                    let b = egui::Button::new(RichText::new(if fav { "♥" } else { "♡" }).size(crate::theme::fs(17.0)).color(if fav { t.accent2.gamma_multiply(1.4) } else { t.text_dim })).min_size(vec2(36.0, 44.0));
                    if ui.add(b).on_hover_text(if fav { "Remove from Favourites" } else { "Add to Favourites" }).clicked() {
                        if let Some(p) = self.lib.get_mut(id) {
                            p.favorite = !p.favorite;
                            self.lib_dirty = true;
                        }
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let dirty = self.is_dirty();
                    let save_btn = egui::Button::new(RichText::new("Save").color(if dirty { t.accent_text } else { t.text_dim })).fill(if dirty { t.accent } else { t.raised }).min_size(vec2(64.0, 30.0));
                    if ui.add(save_btn).on_hover_text("Save changes (Cmd+S). Factory sounds are saved as a new copy.").clicked() {
                        self.save_current();
                    }
                    if ui.add(egui::Button::new("Save as…").min_size(vec2(0.0, 30.0))).clicked() {
                        self.open_save_as();
                    }
                    if ui.add_enabled(dirty, egui::Button::new("Revert").min_size(vec2(0.0, 30.0))).on_hover_text("Throw away changes since the last save").clicked() {
                        self.revert();
                    }
                    let cmp = egui::Button::new(RichText::new(if self.comparing { "Comparing: saved" } else { "Compare" }).color(if self.comparing { t.accent_text } else { t.text })).fill(if self.comparing { t.accent2 } else { t.raised }).min_size(vec2(0.0, 30.0));
                    if ui.add_enabled(dirty || self.comparing, cmp).on_hover_text("Hear the saved version and your edited version in turn").clicked() {
                        self.comparing = !self.comparing;
                    }
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (tab, label) in [(Tab::Play, "Play"), (Tab::Edit, "Edit voice"), (Tab::Effects, "Effects"), (Tab::Settings, "Settings")] {
                    let sel = self.tab == tab;
                    let (rect, resp) = ui.allocate_exact_size(vec2(crate::theme::fs(96.0), crate::theme::fs(28.0)), Sense::click());
                    let p = ui.painter();
                    if sel {
                        p.rect_filled(rect, CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 }, t.bg);
                        p.rect_filled(egui::Rect::from_min_size(rect.left_top(), vec2(rect.width(), 2.0)), CornerRadius::ZERO, t.accent);
                    } else if resp.hovered() {
                        p.rect_filled(rect, CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 }, t.surface);
                    }
                    p.text(rect.center(), egui::Align2::CENTER_CENTER, label, crate::theme::fp(13.0), if sel { t.text } else { t.text_dim });
                    if resp.clicked() {
                        self.tab = tab;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let (peak, voices) = self.synth.lock().map(|s| (s.fx.peak, s.engine.active_voices())).unwrap_or((0.0, 0));
                    if ui.small_button("Panic").on_hover_text("Stop all notes (Esc)").clicked() {
                        self.panic();
                    }
                    widgets::peak_meter(ui, t, peak, 90.0);
                    ui.label(RichText::new("OUT").size(crate::theme::fs(10.0)).color(t.text_faint));
                    ui.label(RichText::new(format!("{voices:>2} voices")).size(crate::theme::fs(11.0)).monospace().color(t.text_faint));
                    let (dev, sr, err) = self.audio_status();
                    let txt = if err.is_some() { "Audio unavailable".to_string() } else if sr > 0 { format!("{dev} · {} kHz", sr as f32 / 1000.0) } else { dev };
                    ui.label(RichText::new(txt).size(crate::theme::fs(11.0)).color(if err.is_some() { t.danger } else { t.text_faint }));
                });
            });
        });
    }

    fn lcd(&mut self, ui: &mut egui::Ui, t: &Theme) {
        let (rect, _) = ui.allocate_exact_size(vec2(crate::theme::fs(360.0), crate::theme::fs(44.0)), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(3), t.lcd_bg);
        p.rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.0, t.border), StrokeKind::Inside);
        let (slot, src, cat, rating) = match self.current.and_then(|id| self.lib.get(id)) {
            Some(pr) => (pr.slot.map(|s| format!("{:02}", s + 1)).unwrap_or_else(|| "U ".into()), pr.source.label(), pr.category.label(), pr.rating),
            None => ("--".into(), "Unsaved".into(), "", 0),
        };
        let dirty = self.is_dirty();
        p.text(egui::pos2(rect.left() + 10.0, rect.top() + 7.0), egui::Align2::LEFT_TOP, &slot, crate::theme::fm(20.0), t.lcd_dim);
        let mut name = self.edit.name.clone();
        if name.chars().count() > 22 {
            name = name.chars().take(21).collect::<String>() + "…";
        }
        p.text(egui::pos2(rect.left() + crate::theme::fs(46.0), rect.top() + 5.0), egui::Align2::LEFT_TOP, name.to_uppercase(), crate::theme::fm(19.0), t.lcd_text);
        let mut sub = format!("{src} · {cat}");
        if rating > 0 {
            sub.push_str(&format!(" · {}", "★".repeat(rating as usize)));
        }
        p.text(egui::pos2(rect.left() + crate::theme::fs(46.0), rect.bottom() - 6.0), egui::Align2::LEFT_BOTTOM, sub, crate::theme::fm(10.5), t.lcd_dim);
        if self.comparing {
            p.text(egui::pos2(rect.right() - 8.0, rect.top() + 6.0), egui::Align2::RIGHT_TOP, "SAVED", crate::theme::fm(10.0), t.lcd_text);
        } else if dirty {
            p.text(egui::pos2(rect.right() - 8.0, rect.top() + 6.0), egui::Align2::RIGHT_TOP, "EDITED", crate::theme::fm(10.0), t.lcd_text);
        }
    }

    fn nav_panel(&mut self, ctx: &egui::Context, t: &Theme) {
        egui::SidePanel::left("nav").exact_width(206.0).resizable(false).frame(egui::Frame::new().fill(t.panel).inner_margin(Margin { left: 8, right: 8, top: 4, bottom: 8 }).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            let nc = self.nav_cache.clone().unwrap_or_else(|| NavCache::build(&self.lib));
            let (all, favs, user, recent, trash) = (nc.all, nc.favs, nc.user, nc.recent, nc.trash);
            let mut new_view = None;
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                section_label(ui, t, "LIBRARY");
                for (icon, label, view, n) in [("all", "All sounds", LibView::All, all), ("♥", "Favourites", LibView::Favorites, favs), ("recent", "Recently played", LibView::Recent, recent), ("user", "My sounds", LibView::User, user)] {
                    if nav_item(ui, t, icon, label, Some(n), self.view == view).clicked() {
                        new_view = Some(view);
                    }
                }
                ui.horizontal(|ui| {
                    section_label(ui, t, "FOLDERS");
                    ui.with_layout(Layout::right_to_left(Align::Max), |ui| {
                        if ui.small_button("+").on_hover_text("New folder").clicked() {
                            self.dialog = Some(Dialog::NewFolder { name: String::new(), add: None });
                        }
                    });
                });
                if self.lib.folders.is_empty() {
                    ui.label(RichText::new("No folders yet").size(crate::theme::fs(11.5)).color(t.text_faint));
                }
                let folders = self.lib.folders.clone();
                for f in &folders {
                    let n = nc.folder_counts.iter().find(|(id, _)| *id == f.id).map(|(_, n)| *n).unwrap_or(0);
                    let r = nav_item(ui, t, "folder", &f.name, Some(n), self.view == LibView::Folder(f.id));
                    if r.clicked() {
                        new_view = Some(LibView::Folder(f.id));
                    }
                    r.context_menu(|ui| {
                        if ui.button("Rename folder…").clicked() {
                            self.dialog = Some(Dialog::RenameFolder { id: f.id, name: f.name.clone() });
                            ui.close_menu();
                        }
                        if ui.button("Export folder as .syx…").clicked() {
                            let ids: Vec<u64> = self.lib.presets.iter().filter(|p| !p.trashed && p.folders.contains(&f.id)).map(|p| p.id).collect();
                            self.export_syx(ids, &f.name);
                            ui.close_menu();
                        }
                        if ui.button("Delete folder…").clicked() {
                            self.dialog = Some(Dialog::DeleteFolder { id: f.id });
                            ui.close_menu();
                        }
                    });
                }
                let banks = nc.banks.clone();
                let imported: Vec<_> = banks.iter().filter(|b| !b.1).collect();
                if !imported.is_empty() {
                    section_label(ui, t, "IMPORTED BANKS");
                    for (b, _, n) in imported {
                        let r = nav_item(ui, t, "bank", b, Some(*n), self.view == LibView::Bank(b.clone()));
                        if r.clicked() {
                            new_view = Some(LibView::Bank(b.clone()));
                        }
                        r.context_menu(|ui| {
                            if ui.button("Export bank as .syx…").clicked() {
                                let ids: Vec<u64> = self.lib.presets.iter().filter(|p| !p.trashed && p.source.bank() == Some(b.as_str())).map(|p| p.id).collect();
                                self.export_syx(ids, b);
                                ui.close_menu();
                            }
                            if ui.button("Move bank to Trash").clicked() {
                                for p in self.lib.presets.iter_mut().filter(|p| p.source.bank() == Some(b.as_str())) {
                                    p.trashed = true;
                                }
                                self.lib_dirty = true;
                                ui.close_menu();
                            }
                        });
                    }
                }
                let is_included = |name: &str| library::INCLUDED_BANKS.iter().any(|(n, _)| *n == name);
                let included: Vec<_> = banks.iter().filter(|b| b.1 && is_included(&b.0)).collect();
                if !included.is_empty() {
                    section_label(ui, t, "INCLUDED BANKS");
                    for (b, _, n) in included {
                        if nav_item(ui, t, "bank", b, Some(*n), self.view == LibView::Bank(b.clone())).clicked() {
                            new_view = Some(LibView::Bank(b.clone()));
                        }
                    }
                }
                section_label(ui, t, "FACTORY BANKS");
                let factory: Vec<_> = banks.iter().filter(|b| b.1 && !is_included(&b.0)).collect();
                egui::CollapsingHeader::new(RichText::new(format!("{} banks", factory.len())).size(crate::theme::fs(12.0)).color(t.text_dim)).id_salt("factory_banks").default_open(false).show(ui, |ui| {
                    for (b, _, n) in factory {
                        if nav_item(ui, t, "bank", b, Some(*n), self.view == LibView::Bank(b.clone())).clicked() {
                            new_view = Some(LibView::Bank(b.clone()));
                        }
                    }
                });
                ui.add_space(8.0);
                if nav_item(ui, t, "trash", "Trash", Some(trash), self.view == LibView::Trash).clicked() {
                    new_view = Some(LibView::Trash);
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Import…").on_hover_text("Add .syx banks or .hexafm sounds. You can also drop files on the window.").clicked() {
                        self.pick_import();
                    }
                    if ui.button("Export…").on_hover_text("Export the sounds in the current list as .syx banks").clicked() {
                        let ids = self.filtered.clone();
                        let name = self.view_title();
                        self.export_syx(ids, &name);
                    }
                });
            });
            if let Some(v) = new_view {
                self.view = v;
                self.scroll_to_current = true;
            }
        });
    }

    fn view_title(&self) -> String {
        match &self.view {
            LibView::All => "All sounds".into(),
            LibView::Favorites => "Favourites".into(),
            LibView::Recent => "Recently played".into(),
            LibView::User => "My sounds".into(),
            LibView::Bank(b) => b.clone(),
            LibView::Folder(f) => self.lib.folder_name(*f).unwrap_or("Folder").to_string(),
            LibView::Trash => "Trash".into(),
        }
    }

    fn browser_panel(&mut self, ctx: &egui::Context, t: &Theme) {
        egui::SidePanel::left("browser").exact_width(PRESET_LIST_WIDTH + 20.0).resizable(false).frame(egui::Frame::new().fill(t.surface).inner_margin(Margin { left: 10, right: 10, top: 10, bottom: 6 }).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(self.view_title()).size(crate::theme::fs(15.0)).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("{} sounds", self.filtered.len())).size(crate::theme::fs(11.5)).color(t.text_faint));
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let resp = ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search name, bank, tag or note").desired_width(ui.available_width() - 40.0));
                if ui.input(|i| i.modifiers.command && i.key_pressed(Key::F)) {
                    resp.request_focus();
                }
                if ui.add_enabled(!self.search.is_empty(), egui::Button::new("×").min_size(vec2(24.0, 24.0))).on_hover_text("Clear search").clicked() {
                    self.search.clear();
                }
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                if chip(ui, t, "All", self.cat_filter.is_empty(), None).clicked() {
                    self.cat_filter.clear();
                }
                let present: BTreeSet<Category> = self.lib.presets.iter().filter(|p| !p.trashed).map(|p| p.category).collect();
                for c in Category::ALL {
                    if !present.contains(&c) {
                        continue;
                    }
                    let sel = self.cat_filter.contains(&c);
                    if chip(ui, t, c.label(), sel, Some(category_color(t, c))).on_hover_text("Click to filter · Cmd-click to combine").clicked() {
                        let multi = ui.input(|i| i.modifiers.command || i.modifiers.shift);
                        if !multi {
                            let only = sel && self.cat_filter.len() == 1;
                            self.cat_filter.clear();
                            if !only {
                                self.cat_filter.insert(c);
                            }
                        } else if sel {
                            self.cat_filter.remove(&c);
                        } else {
                            self.cat_filter.insert(c);
                        }
                    }
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Sort").size(crate::theme::fs(11.5)).color(t.text_dim));
                egui::ComboBox::from_id_salt("sort").selected_text(self.sort.label()).width(120.0).show_ui(ui, |ui| {
                    for s in SortKey::ALL {
                        ui.selectable_value(&mut self.sort, s, s.label());
                    }
                });
                ui.label(RichText::new("Min").size(crate::theme::fs(11.5)).color(t.text_dim));
                stars(ui, t, &mut self.min_rating, 13.0);
            });
            let tags = self.nav_cache.as_ref().map(|n| n.tags.clone()).unwrap_or_default();
            if !tags.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    ui.label(RichText::new("Tags").size(crate::theme::fs(11.5)).color(t.text_dim));
                    for tag in tags.iter().take(14) {
                        let sel = self.tag_filter.as_ref().map(|x| x.eq_ignore_ascii_case(tag)).unwrap_or(false);
                        if chip(ui, t, &format!("#{tag}"), sel, None).clicked() {
                            self.tag_filter = if sel { None } else { Some(tag.clone()) };
                        }
                    }
                });
            }
            ui.add_space(4.0);
            ui.separator();

            if self.filtered.is_empty() {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(if self.view == LibView::Trash { "Trash is empty" } else { "No matching sounds" }).color(t.text_dim));
                    if (!self.search.is_empty() || !self.cat_filter.is_empty() || self.min_rating > 0 || self.tag_filter.is_some()) && ui.button("Clear filters").clicked() {
                        self.search.clear();
                        self.cat_filter.clear();
                        self.min_rating = 0;
                        self.tag_filter = None;
                    }
                });
                return;
            }
            if self.view == LibView::Trash {
                ui.horizontal(|ui| {
                    if ui.button("Empty Trash…").clicked() {
                        self.dialog = Some(Dialog::EmptyTrash);
                    }
                    if ui.button("Restore all").clicked() {
                        for p in self.lib.presets.iter_mut().filter(|p| p.trashed) {
                            p.trashed = false;
                        }
                        self.lib_dirty = true;
                    }
                });
            }
            let row_h = crate::theme::fs(PRESET_ROW_HEIGHT);
            let ids = self.filtered.clone();
            let current_pos = self.current.and_then(|c| ids.iter().position(|&x| x == c));
            // Rows sit back to back: the row pitch is exactly row_h, so the maths below lands on
            // the right row. (Before, egui added 5 px of spacing per row that the scroll maths
            // didn't know about, so the list overshot further the lower the sound was.)
            ui.spacing_mut().item_spacing.y = 0.0;
            let visible_h = ui.available_height();
            let mut sa = egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt("presets");
            if let Some(pos) = current_pos {
                let top = pos as f32 * row_h;
                // The original never centres: like JUCE's ListBox::scrollToEnsureRowIsOnscreen
                // (AVDexedPlayView::followActiveRow 0x1000b2388, navigatePreset 0x1000ba670) it
                // scrolls only as far as needed to show the current sound.
                if self.scroll_to_current || self.reveal_current {
                    let view_h = if self.list_view_h > 0.0 { self.list_view_h } else { visible_h };
                    if top < self.list_offset {
                        sa = sa.vertical_scroll_offset(top);
                    } else if top + row_h > self.list_offset + view_h {
                        sa = sa.vertical_scroll_offset((top + row_h - view_h).max(0.0));
                    }
                }
            }
            if current_pos.is_some() || !ids.is_empty() {
                self.scroll_to_current = false;
                self.reveal_current = false;
            }
            let mut action: Option<(u64, RowAction)> = None;
            let out = sa.show_rows(ui, row_h, ids.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for i in range {
                    let id = ids[i];
                    let Some(p) = self.lib.get(id) else { continue };
                    let selected = Some(id) == self.current;
                    let w = ui.available_width();
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, row_h), Sense::click());
                    let painter = ui.painter();
                    if selected {
                        painter.rect_filled(rect, CornerRadius::same(4), t.accent.linear_multiply(0.22));
                        painter.rect_filled(egui::Rect::from_min_size(rect.min, vec2(3.0, rect.height())), CornerRadius::same(1), t.accent);
                    } else if resp.hovered() {
                        painter.rect_filled(rect, CornerRadius::same(4), t.raised);
                    }
                    let heart_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + 16.0, rect.center().y), vec2(20.0, 20.0));
                    let heart_hover = resp.hover_pos().map(|h| heart_rect.contains(h)).unwrap_or(false);
                    painter.text(heart_rect.center(), egui::Align2::CENTER_CENTER, if p.favorite { "♥" } else { "♡" }, crate::theme::fp(13.0), if p.favorite { t.accent2.gamma_multiply(1.4) } else if heart_hover { t.text_dim } else { t.text_faint.linear_multiply(0.6) });
                    let cc = category_color(t, p.category);
                    painter.circle_filled(egui::pos2(rect.left() + 32.0, rect.center().y), 3.0, cc);
                    let mut name = p.name.clone();
                    if name.chars().count() > 24 {
                        name = name.chars().take(23).collect::<String>() + "…";
                    }
                    painter.text(egui::pos2(rect.left() + 42.0, rect.center().y), egui::Align2::LEFT_CENTER, &name, crate::theme::fp(13.0), t.text);
                    let right = if p.rating > 0 { format!("{}  ", "★".repeat(p.rating as usize)) } else { String::new() };
                    let src = match &p.source {
                        Source::User => "User".to_string(),
                        s => {
                            let short = s.label().replace("SynprezFM ", "SPZ ");
                            format!("{} {:02}", short, p.slot.map(|x| x + 1).unwrap_or(0))
                        }
                    };
                    let rx = rect.right() - 8.0;
                    painter.text(egui::pos2(rx, rect.center().y), egui::Align2::RIGHT_CENTER, &src, crate::theme::fm(10.5), t.text_faint);
                    if !right.is_empty() {
                        let sw = painter.layout_no_wrap(src.clone(), crate::theme::fm(10.5), t.text_faint).size().x;
                        painter.text(egui::pos2(rx - sw - 4.0, rect.center().y), egui::Align2::RIGHT_CENTER, right, crate::theme::fp(10.0), t.star);
                    }
                    if resp.clicked() {
                        action = Some((id, if heart_hover { RowAction::ToggleFav } else { RowAction::Load }));
                    }
                    let folders = self.lib.folders.clone();
                    let in_trash = p.trashed;
                    let p_folders = p.folders.clone();
                    let p_cat = p.category;
                    let p_rating = p.rating;
                    resp.context_menu(|ui| {
                        ui.set_min_width(190.0);
                        if in_trash {
                            if ui.button("Restore").clicked() {
                                action = Some((id, RowAction::Restore));
                                ui.close_menu();
                            }
                            if ui.button("Delete forever").clicked() {
                                action = Some((id, RowAction::DeleteForever));
                                ui.close_menu();
                            }
                            return;
                        }
                        if ui.button("Load").clicked() {
                            action = Some((id, RowAction::Load));
                            ui.close_menu();
                        }
                        ui.separator();
                        let mut r = p_rating;
                        ui.horizontal(|ui| {
                            ui.label("Rating");
                            if stars(ui, t, &mut r, 14.0) {
                                action = Some((id, RowAction::Rate(r)));
                            }
                        });
                        ui.menu_button("Category", |ui| {
                            for c in Category::ALL {
                                if ui.selectable_label(c == p_cat, c.label()).clicked() {
                                    action = Some((id, RowAction::Category(c)));
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.menu_button("Folders", |ui| {
                            for f in &folders {
                                let mut inside = p_folders.contains(&f.id);
                                if ui.checkbox(&mut inside, &f.name).changed() {
                                    action = Some((id, RowAction::ToggleFolder(f.id)));
                                }
                            }
                            ui.separator();
                            if ui.button("New folder with this sound…").clicked() {
                                action = Some((id, RowAction::NewFolder));
                                ui.close_menu();
                            }
                        });
                        ui.separator();
                        if ui.button("Rename…").clicked() {
                            action = Some((id, RowAction::Rename));
                            ui.close_menu();
                        }
                        if ui.button("Duplicate").clicked() {
                            action = Some((id, RowAction::Duplicate));
                            ui.close_menu();
                        }
                        if ui.button("Export as .syx…").clicked() {
                            action = Some((id, RowAction::ExportSyx));
                            ui.close_menu();
                        }
                        if ui.button("Export as Hexa FM sound…").clicked() {
                            action = Some((id, RowAction::ExportSound));
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(RichText::new("Move to Trash").color(t.danger)).clicked() {
                            action = Some((id, RowAction::Trash));
                            ui.close_menu();
                        }
                    });
                }
            });
            self.list_offset = out.state.offset.y;
            self.list_view_h = out.inner_rect.height();
            if let Some((id, a)) = action {
                self.row_action(id, a);
            }
        });
    }

    fn row_action(&mut self, id: u64, a: RowAction) {
        match a {
            RowAction::Load => self.request_load(id),
            RowAction::ToggleFav => {
                if let Some(p) = self.lib.get_mut(id) {
                    p.favorite = !p.favorite;
                }
            }
            RowAction::Rate(r) => {
                if let Some(p) = self.lib.get_mut(id) {
                    p.rating = r;
                }
            }
            RowAction::Category(c) => {
                if let Some(p) = self.lib.get_mut(id) {
                    p.category = c;
                }
            }
            RowAction::ToggleFolder(f) => {
                if let Some(p) = self.lib.get_mut(id) {
                    if let Some(pos) = p.folders.iter().position(|x| *x == f) {
                        p.folders.remove(pos);
                    } else {
                        p.folders.push(f);
                    }
                }
            }
            RowAction::NewFolder => self.dialog = Some(Dialog::NewFolder { name: String::new(), add: Some(id) }),
            RowAction::Rename => self.rename_dialog(id),
            RowAction::Duplicate => self.duplicate(id),
            RowAction::ExportSyx => {
                let n = self.lib.get(id).map(|p| p.name.clone()).unwrap_or_default();
                self.export_syx(vec![id], &n);
            }
            RowAction::ExportSound => self.export_sound(id),
            RowAction::Trash => self.trash(id),
            RowAction::Restore => {
                if let Some(p) = self.lib.get_mut(id) {
                    p.trashed = false;
                }
            }
            RowAction::DeleteForever => {
                self.lib.presets.retain(|p| p.id != id);
                if self.current == Some(id) {
                    self.current = None;
                }
            }
        }
        self.lib_dirty = true;
    }

    fn keyboard_panel(&mut self, ctx: &egui::Context, t: &Theme) {
        egui::TopBottomPanel::bottom("keyboard").exact_height(crate::theme::fs(104.0).max(104.0)).frame(egui::Frame::new().fill(t.panel).inner_margin(Margin { left: 12, right: 12, top: 8, bottom: 8 }).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(crate::theme::fs(150.0));
                    ui.horizontal(|ui| {
                        if ui.small_button("-").on_hover_text("Octave down (Z)").clicked() {
                            self.settings.kb_octave = (self.settings.kb_octave - 1).max(0);
                        }
                        ui.label(RichText::new(format!("Octave C{}", self.settings.kb_octave)).size(crate::theme::fs(11.5)).color(t.text_dim));
                        if ui.small_button("+").on_hover_text("Octave up (X)").clicked() {
                            self.settings.kb_octave = (self.settings.kb_octave + 1).min(8);
                        }
                    });
                    let (mut bend, mut wheel) = self.synth.lock().map(|s| (s.engine.ctrl.pitch_bend, s.engine.ctrl.mod_wheel)).unwrap_or((0.0, 0.0));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().slider_width = 92.0;
                        ui.label(RichText::new("Pitch").size(crate::theme::fs(11.0)).color(t.text_faint));
                        let r = ui.add(egui::Slider::new(&mut bend, -1.0..=1.0).show_value(false));
                        if r.drag_stopped() {
                            bend = 0.0;
                        }
                        if r.changed() || r.drag_stopped() {
                            if let Ok(mut s) = self.synth.lock() {
                                s.engine.ctrl.pitch_bend = bend;
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.spacing_mut().slider_width = 92.0;
                        ui.label(RichText::new("Mod  ").size(crate::theme::fs(11.0)).color(t.text_faint));
                        if ui.add(egui::Slider::new(&mut wheel, 0.0..=1.0).show_value(false)).changed() {
                            if let Ok(mut s) = self.synth.lock() {
                                s.engine.ctrl.mod_wheel = wheel;
                            }
                        }
                    });
                });
                let mut held: BTreeSet<u8> = self.ui_held.union(&self.midi_held).copied().collect();
                for (_, n) in &self.key_notes {
                    held.insert(*n);
                }
                let first = ((self.settings.kb_octave - 1).max(0) * 12 + 12) as u8;
                let out = widgets::keyboard(ui, t, first, 4, &held, &mut self.mouse_note, 86.0);
                if let Some(n) = out.released {
                    self.note_off(n);
                }
                if let Some((n, v)) = out.pressed {
                    self.note_on(n, v);
                }
            });
        });
    }

    fn draw_toasts(&mut self, ctx: &egui::Context, t: &Theme) {
        self.toasts.retain(|(_, at)| at.elapsed() < Duration::from_secs(5));
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts")).anchor(egui::Align2::RIGHT_BOTTOM, vec2(-16.0, -118.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            for (msg, _) in self.toasts.iter().rev().take(4) {
                egui::Frame::new().fill(t.raised).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(5)).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                    ui.set_max_width(360.0);
                    ui.label(RichText::new(msg).size(crate::theme::fs(12.5)));
                });
                ui.add_space(4.0);
            }
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context, t: &Theme) {
        let Some(mut dialog) = self.dialog.take() else { return };
        let mut keep = true;
        let enter = ctx.input(|i| i.key_pressed(Key::Enter));
        let esc = ctx.input(|i| i.key_pressed(Key::Escape));
        let title = match &dialog {
            Dialog::SaveAs { .. } => "Save as new sound",
            Dialog::Rename { .. } => "Rename sound",
            Dialog::NewFolder { .. } => "New folder",
            Dialog::RenameFolder { .. } => "Rename folder",
            Dialog::DeleteFolder { .. } => "Delete folder",
            Dialog::Unsaved { .. } => "Unsaved changes",
            Dialog::EmptyTrash => "Empty Trash",
            Dialog::ImportBanks { .. } => "Import",
        };
        egui::Area::new(egui::Id::new("modal_dim")).fixed_pos(egui::pos2(0.0, 0.0)).order(egui::Order::Middle).show(ctx, |ui| {
            let r = ctx.screen_rect();
            ui.allocate_rect(r, Sense::click());
            ui.painter().rect_filled(r, CornerRadius::ZERO, Color32::from_black_alpha(110));
        });
        egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, -40.0)).order(egui::Order::Foreground).frame(egui::Frame::window(&ctx.style()).inner_margin(Margin::same(16))).show(ctx, |ui| {
            ui.set_width(380.0);
            match &mut dialog {
                Dialog::SaveAs { name, category, folder } => {
                    ui.label(RichText::new("Name").color(t.text_dim));
                    let r = ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                    r.request_focus();
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Category").color(t.text_dim));
                        egui::ComboBox::from_id_salt("saveas_cat").selected_text(category.label()).show_ui(ui, |ui| {
                            for c in Category::ALL {
                                ui.selectable_value(category, c, c.label());
                            }
                        });
                        ui.label(RichText::new("Folder").color(t.text_dim));
                        let fname = folder.and_then(|f| self.lib.folder_name(f)).unwrap_or("None").to_string();
                        egui::ComboBox::from_id_salt("saveas_folder").selected_text(fname).show_ui(ui, |ui| {
                            ui.selectable_value(folder, None, "None");
                            for f in &self.lib.folders {
                                ui.selectable_value(folder, Some(f.id), &f.name);
                            }
                        });
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new("It will be saved in My sounds. The original stays unchanged.").size(crate::theme::fs(11.5)).color(t.text_faint));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let ok = !name.trim().is_empty();
                        if ui.add_enabled(ok, egui::Button::new(RichText::new("Save").color(t.accent_text)).fill(t.accent)).clicked() || (enter && ok) {
                            let (n, c, f) = (name.clone(), *category, *folder);
                            self.save_as(n, c, f);
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() || esc {
                            keep = false;
                        }
                    });
                }
                Dialog::Rename { id, name } => {
                    let r = ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                    r.request_focus();
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let ok = !name.trim().is_empty();
                        if ui.add_enabled(ok, egui::Button::new(RichText::new("Rename").color(t.accent_text)).fill(t.accent)).clicked() || (enter && ok) {
                            let n = library::display_name(name);
                            if let Some(p) = self.lib.get_mut(*id) {
                                p.name = n.clone();
                                p.modified = now();
                            }
                            if self.current == Some(*id) {
                                self.edit.name = n;
                            }
                            self.lib_dirty = true;
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() || esc {
                            keep = false;
                        }
                    });
                }
                Dialog::NewFolder { name, add } => {
                    ui.label(RichText::new("Enter a name for this folder.").color(t.text_dim));
                    let r = ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                    r.request_focus();
                    let exists = self.lib.folders.iter().any(|f| f.name.eq_ignore_ascii_case(name.trim()));
                    if exists {
                        ui.label(RichText::new("A folder with this name already exists.").size(crate::theme::fs(11.5)).color(t.danger));
                    }
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let ok = !name.trim().is_empty() && !exists;
                        if ui.add_enabled(ok, egui::Button::new(RichText::new("Create").color(t.accent_text)).fill(t.accent)).clicked() || (enter && ok) {
                            let fid = self.lib.add_folder(name.trim());
                            if let Some(pid) = add {
                                if let Some(p) = self.lib.get_mut(*pid) {
                                    p.folders.push(fid);
                                }
                            }
                            self.view = LibView::Folder(fid);
                            self.lib_dirty = true;
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() || esc {
                            keep = false;
                        }
                    });
                }
                Dialog::RenameFolder { id, name } => {
                    let r = ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                    r.request_focus();
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let ok = !name.trim().is_empty();
                        if ui.add_enabled(ok, egui::Button::new(RichText::new("Rename").color(t.accent_text)).fill(t.accent)).clicked() || (enter && ok) {
                            if let Some(f) = self.lib.folders.iter_mut().find(|f| f.id == *id) {
                                f.name = name.trim().to_string();
                            }
                            self.lib_dirty = true;
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() || esc {
                            keep = false;
                        }
                    });
                }
                Dialog::DeleteFolder { id } => {
                    let n = self.lib.presets.iter().filter(|p| p.folders.contains(id)).count();
                    ui.label(format!("Delete this folder? The {n} sound(s) inside it stay in your library."));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Delete").color(Color32::WHITE)).fill(t.danger)).clicked() {
                            let fid = *id;
                            self.lib.folders.retain(|f| f.id != fid);
                            for p in self.lib.presets.iter_mut() {
                                p.folders.retain(|f| *f != fid);
                            }
                            if self.view == LibView::Folder(fid) {
                                self.view = LibView::All;
                            }
                            self.lib_dirty = true;
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() || esc {
                            keep = false;
                        }
                    });
                }
                Dialog::Unsaved { then } => {
                    ui.label("Save your changes before loading another sound?");
                    ui.add_space(10.0);
                    let PendingAction::Load(target) = *then;
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Save").color(t.accent_text)).fill(t.accent)).clicked() {
                            let factory = self.current.and_then(|id| self.lib.get(id)).map(|p| p.is_factory()).unwrap_or(true);
                            if factory {
                                let name = format!("{} (my version)", self.edit.name);
                                let cat = self.current.and_then(|id| self.lib.get(id)).map(|p| p.category).unwrap_or(Category::Other);
                                self.save_as(name, cat, None);
                            } else {
                                self.save_current();
                            }
                            self.load_preset(target, true);
                            keep = false;
                        }
                        if ui.button("Discard").clicked() {
                            self.load_preset(target, true);
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() || esc {
                            keep = false;
                        }
                    });
                }
                Dialog::EmptyTrash => {
                    let n = self.lib.presets.iter().filter(|p| p.trashed).count();
                    ui.label(format!("Permanently delete {n} sound(s)? Factory sounds can be restored later from Settings."));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Delete forever").color(Color32::WHITE)).fill(t.danger)).clicked() {
                            let n = self.lib.empty_trash();
                            if self.current.map(|c| self.lib.get(c).is_none()).unwrap_or(false) {
                                self.current = None;
                            }
                            self.toast(format!("Deleted {n} sound(s)"));
                            self.lib_dirty = true;
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() || esc {
                            keep = false;
                        }
                    });
                }
                Dialog::ImportBanks { paths, folder, new_folder } => {
                    ui.label(format!("{} file(s) to add. Banks keep their own name in the sidebar; you can also put the sounds in a folder.", paths.len()));
                    for p in paths.iter().take(6) {
                        ui.label(RichText::new(p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()).monospace().size(crate::theme::fs(11.5)).color(t.text_dim));
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Folder").color(t.text_dim));
                        let fname = folder.and_then(|f| self.lib.folder_name(f)).unwrap_or("None").to_string();
                        egui::ComboBox::from_id_salt("import_folder").selected_text(fname).show_ui(ui, |ui| {
                            ui.selectable_value(folder, None, "None");
                            for f in &self.lib.folders {
                                ui.selectable_value(folder, Some(f.id), &f.name);
                            }
                        });
                        ui.add(egui::TextEdit::singleline(new_folder).hint_text("or new folder name").desired_width(140.0));
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Import").color(t.accent_text)).fill(t.accent)).clicked() || enter {
                            let mut f = *folder;
                            if !new_folder.trim().is_empty() {
                                f = Some(self.lib.add_folder(new_folder.trim()));
                            }
                            let ps = paths.clone();
                            self.do_import(ps, f);
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() || esc {
                            keep = false;
                        }
                    });
                }
            }
        });
        if keep && self.dialog.is_none() {
            self.dialog = Some(dialog);
        }
    }

    pub fn set_start_tab(&mut self, tab: Option<&str>) {
        self.tab = match tab {
            Some("edit") => Tab::Edit,
            Some("effects") => Tab::Effects,
            Some("settings") => Tab::Settings,
            _ => Tab::Play,
        };
    }

    // helpers used by pages
    pub fn current_preset(&self) -> Option<&Preset> {
        self.current.and_then(|id| self.lib.get(id))
    }
    pub fn mark_lib_dirty(&mut self) {
        self.lib_dirty = true;
    }
    pub fn tag_input(&mut self) -> &mut String {
        &mut self.tag_input
    }
    pub fn restore_factory(&mut self) {
        let n = self.lib.restore_factory();
        self.lib_dirty = true;
        self.toast(if n == 0 { "No factory sound was missing".to_string() } else { format!("Factory sounds restored: {n}") });
    }
    pub fn open_data_folder(&self) {
        let dir = library::data_dir();
        let _ = std::fs::create_dir_all(&dir);
        #[cfg(target_os = "windows")]
        let opener = "explorer";
        #[cfg(target_os = "macos")]
        let opener = "open";
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let opener = "xdg-open";
        let _ = std::process::Command::new(opener).arg(dir).spawn();
    }
    pub fn stars_ui(&self, ui: &mut egui::Ui, rating: &mut u8) -> bool {
        stars(ui, &self.theme, rating, 16.0)
    }
    pub fn chip_ui(&self, ui: &mut egui::Ui, text: &str, selected: bool, color: Option<Color32>) -> egui::Response {
        chip(ui, &self.theme, text, selected, color)
    }
    pub fn new_folder_dialog(&mut self, add: Option<u64>) {
        self.dialog = Some(Dialog::NewFolder { name: String::new(), add });
    }
    pub fn rename_dialog(&mut self, id: u64) {
        let name = self.lib.get(id).map(|p| p.name.clone()).unwrap_or_default();
        self.dialog = Some(Dialog::Rename { id, name });
    }
    pub fn set_view_tag(&mut self, tag: String) {
        self.tag_filter = Some(tag);
    }
}

#[derive(Clone, PartialEq, Debug)]
struct FilterKey {
    search: String,
    cats: Vec<Category>,
    tag: Option<String>,
    min_rating: u8,
    sort: SortKey,
    view: LibView,
}

/// Sidebar counts, banks and tags, rebuilt only when the library changes.
#[derive(Clone, Default)]
struct NavCache {
    all: usize,
    favs: usize,
    user: usize,
    recent: usize,
    trash: usize,
    folder_counts: Vec<(u64, usize)>,
    banks: Vec<(String, bool, usize)>,
    tags: Vec<String>,
}

impl NavCache {
    fn build(lib: &Library) -> NavCache {
        let mut c = NavCache::default();
        for p in &lib.presets {
            if p.trashed {
                c.trash += 1;
                continue;
            }
            c.all += 1;
            c.favs += p.favorite as usize;
            c.user += (p.source == Source::User) as usize;
            c.recent += (p.last_played > 0) as usize;
            for f in &p.folders {
                match c.folder_counts.iter_mut().find(|(id, _)| id == f) {
                    Some(e) => e.1 += 1,
                    None => c.folder_counts.push((*f, 1)),
                }
            }
        }
        c.recent = c.recent.min(100);
        c.banks = lib.banks();
        c.tags = lib.all_tags();
        c
    }
}

#[derive(Clone, Copy)]
enum RowAction {
    Load,
    ToggleFav,
    Rate(u8),
    Category(Category),
    ToggleFolder(u64),
    NewFolder,
    Rename,
    Duplicate,
    ExportSyx,
    ExportSound,
    Trash,
    Restore,
    DeleteForever,
}
