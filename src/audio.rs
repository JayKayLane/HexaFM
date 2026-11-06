//! Audio output (cpal) and MIDI input (midir). The synth lives behind a mutex shared with the UI.

use crate::fm::{FmEngine, BLOCK};
use crate::fx::{FxChain, PlayParams};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub struct Synth {
    pub engine: FmEngine,
    pub fx: FxChain,
    pub play: PlayParams,
    pub master_db: f32,
    pub midi_channel: u8, // 0 = omni, else 1..16
    pub test_tone: u32,
    mono_buf: Vec<f32>,
    l: Vec<f32>,
    r: Vec<f32>,
}

impl Synth {
    pub fn new(sr: f32) -> Self {
        Synth {
            engine: FmEngine::new(sr),
            fx: FxChain::new(sr),
            play: PlayParams::default(),
            master_db: -3.0,
            midi_channel: 0,
            test_tone: 0,
            mono_buf: vec![0.0; BLOCK],
            l: vec![0.0; BLOCK],
            r: vec![0.0; BLOCK],
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.engine.sr
    }

    /// Fill an interleaved buffer with `channels` channels.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let frames = out.len() / channels.max(1);
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(BLOCK);
            self.mono_buf[..n].fill(0.0);
            self.engine.render_block(&mut self.mono_buf[..n]);
            if self.test_tone > 0 {
                let sr = self.engine.sr;
                for i in 0..n {
                    let t = self.test_tone as f32 / sr;
                    self.mono_buf[i] += (t * 440.0 * std::f32::consts::TAU).sin() * 0.2;
                    self.test_tone = self.test_tone.saturating_sub(1);
                    if self.test_tone == 0 {
                        break;
                    }
                }
            }
            let (l, r) = (&mut self.l[..n], &mut self.r[..n]);
            self.fx.process(&self.play, self.master_db, &self.mono_buf[..n], l, r);
            for i in 0..n {
                let f = (done + i) * channels;
                if channels == 1 {
                    out[f] = (l[i] + r[i]) * 0.5;
                } else {
                    out[f] = l[i];
                    out[f + 1] = r[i];
                    for c in 2..channels {
                        out[f + c] = 0.0;
                    }
                }
            }
            done += n;
        }
    }

    /// MIDI from a plugin host (no window events needed).
    pub fn handle_host_midi(&mut self, msg: &[u8]) {
        let (tx, _rx) = std::sync::mpsc::channel();
        self.handle_midi(msg, &tx);
    }

    pub fn handle_midi(&mut self, msg: &[u8], events: &Sender<UiEvent>) {
        if msg.is_empty() {
            return;
        }
        let status = msg[0] & 0xF0;
        let ch = (msg[0] & 0x0F) + 1;
        if self.midi_channel != 0 && ch != self.midi_channel && status != 0xF0 {
            return;
        }
        match (status, msg.len()) {
            (0x90, 3) if msg[2] > 0 => {
                self.engine.note_on(msg[1], msg[2]);
                let _ = events.send(UiEvent::Note(msg[1], true));
            }
            (0x80, 3) | (0x90, 3) => {
                self.engine.note_off(msg[1]);
                let _ = events.send(UiEvent::Note(msg[1], false));
            }
            (0xB0, 3) => {
                match msg[1] {
                    1 => self.engine.ctrl.mod_wheel = msg[2] as f32 / 127.0,
                    64 => self.engine.set_sustain(msg[2] >= 64),
                    120 | 123 => self.engine.all_notes_off(),
                    _ => {}
                }
                let _ = events.send(UiEvent::Cc(msg[1], msg[2]));
            }
            (0xE0, 3) => {
                let v = ((msg[2] as i32) << 7 | msg[1] as i32) - 8192;
                self.engine.ctrl.pitch_bend = v as f32 / 8192.0;
            }
            (0xC0, 2) => {
                let _ = events.send(UiEvent::Program(msg[1]));
            }
            _ => {}
        }
    }
}

/// Things the audio/MIDI side tells the window.
#[derive(Debug, Clone)]
pub enum UiEvent {
    Note(u8, bool),
    Cc(u8, u8),
    Program(u8),
}

pub struct AudioOut {
    pub _stream: Option<StreamHandle>,
    pub device_name: String,
    pub sample_rate: u32,
    pub error: Option<String>,
}

/// The output stream. Only the standalone app owns one, on its UI thread; the plugin build never
/// creates it, so moving the (empty) handle between threads is safe.
pub struct StreamHandle(pub cpal::Stream);
unsafe impl Send for StreamHandle {}

pub fn output_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.output_devices().map(|it| it.filter_map(|d| d.name().ok()).collect()).unwrap_or_default()
}

pub fn start_audio(synth: Arc<Mutex<Synth>>, wanted: Option<&str>) -> AudioOut {
    let host = cpal::default_host();
    let device = wanted
        .and_then(|w| host.output_devices().ok().and_then(|mut it| it.find(|d| d.name().map(|n| n == w).unwrap_or(false))))
        .or_else(|| host.default_output_device());
    let Some(device) = device else {
        return AudioOut { _stream: None, device_name: "No audio output".into(), sample_rate: 0, error: Some("No audio output available".into()) };
    };
    let name = device.name().unwrap_or_else(|_| "Audio output".into());
    let config = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => return AudioOut { _stream: None, device_name: name, sample_rate: 0, error: Some(e.to_string()) },
    };
    let sr = config.sample_rate().0;
    let channels = config.channels() as usize;
    {
        let mut s = synth.lock().unwrap();
        if (s.sample_rate() - sr as f32).abs() > 1.0 {
            let play = s.play.clone();
            let patch = s.engine.patch.clone();
            let master = s.master_db;
            *s = Synth::new(sr as f32);
            s.play = play;
            s.engine.patch = patch;
            s.master_db = master;
        }
    }
    let err_fn = |e| eprintln!("audio error: {e}");
    let stream_config: cpal::StreamConfig = config.clone().into();
    let s2 = synth.clone();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _| {
                if let Ok(mut s) = s2.lock() {
                    s.render(data, channels);
                } else {
                    data.fill(0.0);
                }
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => {
            let mut tmp = Vec::new();
            device.build_output_stream(
                &stream_config,
                move |data: &mut [i16], _| {
                    tmp.resize(data.len(), 0.0f32);
                    if let Ok(mut s) = s2.lock() {
                        s.render(&mut tmp, channels);
                    }
                    for (d, x) in data.iter_mut().zip(tmp.iter()) {
                        *d = (x.clamp(-1.0, 1.0) * 32767.0) as i16;
                    }
                },
                err_fn,
                None,
            )
        }
        other => return AudioOut { _stream: None, device_name: name, sample_rate: sr, error: Some(format!("Unsupported sample format {other:?}")) },
    };
    match stream {
        Ok(st) => {
            let _ = st.play();
            AudioOut { _stream: Some(StreamHandle(st)), device_name: name, sample_rate: sr, error: None }
        }
        Err(e) => AudioOut { _stream: None, device_name: name, sample_rate: sr, error: Some(e.to_string()) },
    }
}

pub struct MidiIn {
    pub _conn: Option<midir::MidiInputConnection<()>>,
    pub port_name: Option<String>,
}

pub fn midi_ports() -> Vec<String> {
    match midir::MidiInput::new("Hexa FM scan") {
        Ok(m) => m.ports().iter().filter_map(|p| m.port_name(p).ok()).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn connect_midi(port: &str, synth: Arc<Mutex<Synth>>, events: Sender<UiEvent>) -> Result<MidiIn, String> {
    let mut input = midir::MidiInput::new("Hexa FM").map_err(|e| e.to_string())?;
    input.ignore(midir::Ignore::None);
    let ports = input.ports();
    let p = ports
        .iter()
        .find(|p| input.port_name(p).map(|n| n == port).unwrap_or(false))
        .ok_or_else(|| "Could not open the selected MIDI port. Check that the device is connected.".to_string())?
        .clone();
    let conn = input
        .connect(
            &p,
            "hexa-fm-in",
            move |_, msg, _| {
                if let Ok(mut s) = synth.lock() {
                    s.handle_midi(msg, &events);
                }
            },
            (),
        )
        .map_err(|e| e.to_string())?;
    Ok(MidiIn { _conn: Some(conn), port_name: Some(port.to_string()) })
}
