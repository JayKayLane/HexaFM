//! Hexa FM as an instrument plugin (VST3 and CLAP; the AU is a wrapper around the CLAP).
//! The sound engine, sound library and interface are the same code as the standalone app.

use hexa_fm::app::App;
use hexa_fm::audio::{Synth, UiEvent};
use hexa_fm::fx::PlayParams;
use hexa_fm::patch::{Voice, VOICE_SIZE};
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, EguiState};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, RwLock};

/// What the host stores with a project. Like the original's getStateInformation (0x100199464:
/// schemaVersion, avPresetId, avPresetName, voice …) it keeps the sound's library identity, so a
/// reopened project selects the same library sound and still knows whether it was edited.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct SoundState {
    #[serde(default = "schema_v1")]
    pub schema: u32,
    #[serde(default)]
    pub preset_id: Option<u64>,
    pub voice: Vec<u8>,
    pub play: PlayParams,
    pub name: String,
}

fn schema_v1() -> u32 {
    1
}

/// Editor size from DexedAudioProcessorEditor::resetSize 0x1000662e0: 1440 × 940 with the
/// keyboard (828 without).
const EDITOR_W: u32 = 1440;
const EDITOR_H: u32 = 940;
/// Tail reported once all notes have stopped (DexedAudioProcessor::getTailLengthSeconds
/// 0x10001e38c): 50 s while the delay is in use, 18 s for reverb, threshold 1e-5.
const TAIL_DELAY_S: f32 = 50.0;
const TAIL_REVERB_S: f32 = 18.0;
const TAIL_THRESHOLD: f32 = 1e-5;

#[derive(Params)]
pub struct HexaParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
    #[persist = "sound"]
    sound: RwLock<Option<SoundState>>,
}

/// The window's state. nih-plug only touches it on the editor (GUI) thread; the plugin never
/// creates an audio stream or MIDI port inside it, so it is safe to mark it Send + Sync.
pub struct PluginUi(App);
unsafe impl Send for PluginUi {}
unsafe impl Sync for PluginUi {}

pub struct HexaFm {
    params: Arc<HexaParams>,
    synth: Arc<Mutex<Synth>>,
    events: Arc<Mutex<Option<Sender<UiEvent>>>>,
    buf: Vec<f32>,
}

impl Default for HexaFm {
    fn default() -> Self {
        HexaFm {
            params: Arc::new(HexaParams { editor_state: EguiState::from_size(EDITOR_W, EDITOR_H), sound: RwLock::new(None) }),
            synth: Arc::new(Mutex::new(Synth::new(48000.0))),
            events: Arc::new(Mutex::new(None)),
            buf: Vec::new(),
        }
    }
}

fn voice_from(bytes: &[u8]) -> Voice {
    let mut d = [0u8; VOICE_SIZE];
    for (i, b) in bytes.iter().take(VOICE_SIZE).enumerate() {
        d[i] = *b;
    }
    let mut v = Voice(d);
    v.clamp();
    v
}

impl HexaFm {
    fn forward(&self, msg: &[u8], synth: &mut Synth) {
        synth.handle_host_midi(msg);
        if let Ok(guard) = self.events.try_lock() {
            if let Some(tx) = guard.as_ref() {
                let ev = match (msg[0] & 0xF0, msg.len()) {
                    (0x90, 3) if msg[2] > 0 => Some(UiEvent::Note(msg[1], true)),
                    (0x80, 3) | (0x90, 3) => Some(UiEvent::Note(msg[1], false)),
                    (0xB0, 3) => Some(UiEvent::Cc(msg[1], msg[2])),
                    (0xC0, 2) => Some(UiEvent::Program(msg[1])),
                    _ => None,
                };
                if let Some(e) = ev {
                    let _ = tx.send(e);
                }
            }
        }
    }
}

impl Plugin for HexaFm {
    const NAME: &'static str = "Hexa FM";
    const VENDOR: &'static str = "Hexa FM";
    const URL: &'static str = "https://asb2m10.github.io/dexed/";
    const EMAIL: &'static str = "info@example.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // stereo or mono output, as DexedAudioProcessor::isBusesLayoutSupported 0x10001e128 allows
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout { main_input_channels: None, main_output_channels: NonZeroU32::new(2), ..AudioIOLayout::const_default() },
        AudioIOLayout { main_input_channels: None, main_output_channels: NonZeroU32::new(1), ..AudioIOLayout::const_default() },
    ];
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let synth = self.synth.clone();
        let events = self.events.clone();
        let params = self.params.clone();
        create_egui_editor(
            self.params.editor_state.clone(),
            None::<PluginUi>,
            |_, _| {},
            move |ctx, _setter, state| {
                let ui = state.get_or_insert_with(|| {
                    let mut a = App::new_plugin(ctx, synth.clone());
                    if let Some(s) = params.sound.read().ok().and_then(|g| g.clone()) {
                        a.restore_sound(s.preset_id, voice_from(&s.voice), s.play, s.name);
                    }
                    if let Ok(mut e) = events.lock() {
                        *e = Some(a.event_sender());
                    }
                    PluginUi(a)
                });
                let app = &mut ui.0;
                app.ui(ctx);
                // keep the project's copy of the sound up to date
                let now = SoundState { schema: 1, preset_id: app.current, voice: app.edit.voice.0.to_vec(), play: app.edit.play.clone(), name: app.edit.name.clone() };
                let differs = params.sound.read().map(|g| g.as_ref() != Some(&now)).unwrap_or(false);
                if differs {
                    if let Ok(mut g) = params.sound.write() {
                        *g = Some(now);
                    }
                }
            },
        )
    }

    fn initialize(&mut self, _layout: &AudioIOLayout, config: &BufferConfig, _context: &mut impl InitContext<Self>) -> bool {
        let sr = config.sample_rate;
        if let Ok(mut s) = self.synth.lock() {
            if (s.sample_rate() - sr).abs() > 0.5 {
                let play = s.play.clone();
                let patch = s.engine.patch.clone();
                *s = Synth::new(sr);
                s.play = play;
                s.engine.patch = patch;
            }
            if let Some(st) = self.params.sound.read().ok().and_then(|g| g.clone()) {
                s.engine.patch = voice_from(&st.voice);
                s.play = st.play;
            }
        }
        self.buf = vec![0.0; config.max_buffer_size as usize * 2];
        true
    }

    fn reset(&mut self) {
        if let Ok(mut s) = self.synth.lock() {
            s.engine.all_notes_off();
            s.fx.clear();
        }
    }

    fn process(&mut self, buffer: &mut Buffer, _aux: &mut AuxiliaryBuffers, context: &mut impl ProcessContext<Self>) -> ProcessStatus {
        let synth_arc = self.synth.clone();
        let Ok(mut synth) = synth_arc.lock() else {
            return ProcessStatus::Normal;
        };
        while let Some(event) = context.next_event() {
            let msg: Option<[u8; 3]> = match event {
                NoteEvent::NoteOn { channel, note, velocity, .. } => Some([0x90 | (channel & 15), note, ((velocity * 127.0).round() as u8).clamp(1, 127)]),
                NoteEvent::NoteOff { channel, note, .. } => Some([0x80 | (channel & 15), note, 0]),
                NoteEvent::MidiCC { channel, cc, value, .. } => Some([0xB0 | (channel & 15), cc, (value * 127.0).round() as u8]),
                NoteEvent::MidiPitchBend { channel, value, .. } => {
                    let v = (value * 16383.0).round() as u16;
                    Some([0xE0 | (channel & 15), (v & 0x7f) as u8, (v >> 7) as u8])
                }
                NoteEvent::MidiChannelPressure { .. } => None,
                NoteEvent::MidiProgramChange { channel, program, .. } => {
                    self.forward(&[0xC0 | (channel & 15), program], &mut synth);
                    None
                }
                _ => None,
            };
            if let Some(m) = msg {
                self.forward(&m, &mut synth);
            }
        }
        let n = buffer.samples();
        if self.buf.len() < n * 2 {
            self.buf.resize(n * 2, 0.0);
        }
        synth.render(&mut self.buf[..n * 2], 2);
        let mono = buffer.channels() == 1;
        for (i, mut frame) in buffer.iter_samples().enumerate() {
            let l = self.buf[i * 2];
            let r = self.buf[i * 2 + 1];
            let mut ch = frame.iter_mut();
            if mono {
                if let Some(s) = ch.next() {
                    *s = 0.5 * (l + r);
                }
                continue;
            }
            if let Some(s) = ch.next() {
                *s = l;
            }
            if let Some(s) = ch.next() {
                *s = r;
            }
        }
        if synth.engine.active_voices() > 0 {
            return ProcessStatus::KeepAlive;
        }
        let p = &synth.play;
        let tail_s = if p.delay.on && p.delay.amount > TAIL_THRESHOLD {
            TAIL_DELAY_S
        } else if p.reverb.on && p.reverb.amount > TAIL_THRESHOLD {
            TAIL_REVERB_S
        } else {
            0.0
        };
        if tail_s > 0.0 {
            ProcessStatus::Tail((tail_s * synth.sample_rate()) as u32)
        } else {
            ProcessStatus::Normal
        }
    }
}

impl ClapPlugin for HexaFm {
    const CLAP_ID: &'static str = "app.hexafm.synth";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Six-operator FM synthesizer with a sound library");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Stereo];
}

impl Vst3Plugin for HexaFm {
    const VST3_CLASS_ID: [u8; 16] = *b"HexaFMSynth0001A";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(HexaFm);
nih_export_vst3!(HexaFm);
