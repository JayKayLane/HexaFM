//! Output stage: resonant low-pass, chorus, stereo delay and reverb.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ReverbParams {
    pub on: bool,
    pub amount: f32,
    pub size: f32,
    pub damping: f32,
    pub predelay_ms: f32,
}
impl Default for ReverbParams {
    fn default() -> Self {
        // createParameterLayout 0x1001805d4: amount 0 (0..1), size 0.21, damping 0.5, pre-delay 45 ms (0..250)
        ReverbParams { on: true, amount: 0.0, size: 0.21, damping: 0.5, predelay_ms: 45.0 }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChorusMode {
    One,
    Two,
    Both,
}
impl ChorusMode {
    pub const ALL: [ChorusMode; 3] = [ChorusMode::One, ChorusMode::Two, ChorusMode::Both];
    pub fn label(self) -> &'static str {
        match self {
            ChorusMode::One => "I",
            ChorusMode::Two => "II",
            ChorusMode::Both => "I + II",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ChorusParams {
    pub on: bool,
    pub amount: f32,
    /// LFO rate in Hz, 0.05..5
    pub rate_hz: f32,
    /// modulation depth in ms, 0..5
    pub depth_ms: f32,
    pub mode: ChorusMode,
    pub spread: f32,
}
impl Default for ChorusParams {
    fn default() -> Self {
        // createParameterLayout 0x1001805d4: amount 0, mode I, rate 0.5 Hz (0.05..5), depth 2 (0..5), spread 1
        ChorusParams { on: true, amount: 0.0, rate_hz: 0.5, depth_ms: 2.0, mode: ChorusMode::One, spread: 1.0 }
    }
}

/// Reverb pre-delay range (createParameterLayout: 0..250 ms, step 0.1).
pub const PREDELAY_MAX_MS: f32 = 250.0;
/// Local tempo range (createParameterLayout: 20..999 BPM, default 120).
pub const TEMPO_MAX_BPM: f32 = 999.0;
/// Chorus: base delay 7 ms +/- depth; chorus II runs its LFO 1.65x faster (processBlock 0x100018000).
const CHORUS_BASE_MS: f32 = 7.0;
const CHORUS_II_RATIO: f32 = 1.65;

pub const DIVISIONS: [(&str, f32); 8] = [
    ("1/16", 0.25),
    ("1/8T", 1.0 / 3.0),
    ("1/8", 0.5),
    ("1/8D", 0.75),
    ("1/4T", 2.0 / 3.0),
    ("1/4", 1.0),
    ("1/4D", 1.5),
    ("1/2", 2.0),
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DelayParams {
    pub on: bool,
    pub amount: f32,
    pub feedback: f32,
    pub damping: f32,
    pub time_l_ms: f32,
    pub time_r_ms: f32,
    pub sync: bool,
    pub div_l: usize,
    pub div_r: usize,
    pub bpm: f32,
}
impl Default for DelayParams {
    fn default() -> Self {
        // MECHANICS: feedback 0.35, damping 0.3, time 500 ms
        DelayParams { on: true, amount: 0.0, feedback: 0.35, damping: 0.3, time_l_ms: 500.0, time_r_ms: 500.0, sync: false, div_l: 3, div_r: 5, bpm: 120.0 }
    }
}
impl DelayParams {
    pub fn times(&self) -> (f32, f32) {
        if self.sync {
            let beat = 60000.0 / self.bpm.max(20.0);
            (
                (beat * DIVISIONS[self.div_l.min(7)].1).clamp(20.0, 1500.0),
                (beat * DIVISIONS[self.div_r.min(7)].1).clamp(20.0, 1500.0),
            )
        } else {
            (self.time_l_ms.clamp(20.0, 1500.0), self.time_r_ms.clamp(20.0, 1500.0))
        }
    }
}

/// The sound-shaping settings that are saved with each sound ("Play" page).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PlayParams {
    pub volume_db: f32,
    pub cutoff: f32,
    pub resonance: f32,
    pub pan: f32,
    pub reverb: ReverbParams,
    pub chorus: ChorusParams,
    pub delay: DelayParams,
}
impl Default for PlayParams {
    fn default() -> Self {
        PlayParams {
            volume_db: 0.0,
            cutoff: 1.0,
            resonance: 0.0,
            pan: 0.0,
            reverb: ReverbParams::default(),
            chorus: ChorusParams::default(),
            delay: DelayParams::default(),
        }
    }
}

// ---------------------------------------------------------------- filter

/// Cutoff knob 0..1 → Hz on the original's log curve: (e^(c·ln 20) − 1)/19 · (19000 − 60) + 60
/// (processBlock 0x100018000: 0x403fba14 = 2.99573, /19, 18940, 60).
pub fn cutoff_hz(c: f32) -> f32 {
    logsc(c.clamp(0.0, 1.0), 60.0, 19000.0)
}

fn logsc(x: f32, min: f32, max: f32) -> f32 {
    ((x * 2.995_732_3).exp() - 1.0) / 19.0 * (max - min) + min
}

/// Four-pole resonant low-pass in the original's structure: 15 Hz DC blocker (×0.45), resonance
/// feedback R24 = 3.5·rReso with rReso = 0.991 − logsc(1 − reso, 0, 0.991), a damped first pole
/// (atan), three more one-pole stages and level compensation (1 + 0.45·R24).
#[derive(Default, Clone)]
struct DexedFilter {
    dc: f32,
    bright: f32,
    s1: f32,
    s2: f32,
    s3: f32,
    s4: f32,
}

/// First-pole damping: s = atan(s*rcor24)/rcor24 with rcor24 = 0.022045*sqrt(44000/sr)
/// (PluginFx init inlined in prepareToPlay 0x100016ce0: 0x3F96931569315693, 44000 = 0x472be000).
const RCOR24_BASE: f32 = 0.022045454;

#[inline]
fn tpt(state: &mut f32, x: f32, g: f32) -> f32 {
    let v = (x - *state) * g / (1.0 + g);
    let res = v + *state;
    *state = res + v;
    res
}

impl DexedFilter {
    #[inline]
    fn process(&mut self, x: f32, g: f32, r24: f32, c: &FilterConsts) -> f32 {
        let s = x - 0.45 * tpt(&mut self.dc, x, c.g_dc);
        // one-pole just below Nyquist (tan((sr/2 - 10)*pi/sr)) before the ladder
        let s = tpt(&mut self.bright, s, c.g_bright);
        let lpc = g / (1.0 + g);
        let ml = 1.0 / (1.0 + g);
        let ss = (lpc * (lpc * (lpc * self.s1 + self.s2) + self.s3) + self.s4) * ml;
        let gg = lpc * lpc * lpc * lpc;
        let y0 = (s - r24 * ss) / (1.0 + r24 * gg);
        let v = (y0 - self.s1) * lpc;
        let res = v + self.s1;
        self.s1 = res + v;
        self.s1 = (self.s1 * c.rcor24).atan() / c.rcor24;
        let y2 = tpt(&mut self.s2, res, g);
        let y3 = tpt(&mut self.s3, y2, g);
        let y4 = tpt(&mut self.s4, y3, g);
        y4 * (1.0 + r24 * 0.45)
    }
}

struct FilterConsts {
    g_dc: f32,
    g_bright: f32,
    rcor24: f32,
    smooth: f32,
}

impl FilterConsts {
    fn new(sr: f32) -> Self {
        FilterConsts {
            // DC blocker coefficient is 15*pi/sr (no tan in the original)
            g_dc: 15.0 * std::f32::consts::PI / sr,
            g_bright: ((sr * 0.5 - 10.0) * std::f32::consts::PI / sr).tan(),
            rcor24: RCOR24_BASE * (44000.0 / sr).sqrt(),
            // cutoff and resonance glide with a 10 ms one-pole: exp(-1/(0.01*sr))
            smooth: (-1.0 / (0.01 * sr)).exp(),
        }
    }
}

// ---------------------------------------------------------------- delay line

#[derive(Clone)]
struct Line {
    buf: Vec<f32>,
    pos: usize,
}
impl Line {
    fn new(n: usize) -> Self {
        Line { buf: vec![0.0; n.max(2)], pos: 0 }
    }
    #[inline]
    fn push(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.pos = (self.pos + 1) % self.buf.len();
    }
    #[inline]
    fn read(&self, delay: f32) -> f32 {
        let len = self.buf.len();
        let d = delay.clamp(1.0, (len - 2) as f32);
        let i = d as usize;
        let f = d - i as f32;
        let a = self.buf[(self.pos + len - 1 - i) % len];
        let b = self.buf[(self.pos + len - 2 - i) % len];
        a + (b - a) * f
    }
    /// Four-point cubic (Catmull-Rom) read, as the original chorus does.
    #[inline]
    fn read_cubic(&self, delay: f32) -> f32 {
        let len = self.buf.len();
        let d = delay.clamp(2.0, (len - 3) as f32);
        let i = d as usize;
        let f = d - i as f32;
        let at = |k: usize| self.buf[(self.pos + len * 2 - 1 - k) % len];
        let (y0, y1, y2, y3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        let c1 = 0.5 * (y2 - y0);
        let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
        let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
        ((c3 * f + c2) * f + c1) * f + y1
    }
    fn clear(&mut self) {
        self.buf.fill(0.0);
    }
}

#[derive(Clone)]
struct Comb {
    buf: Vec<f32>,
    pos: usize,
    store: f32,
}
impl Comb {
    #[inline]
    fn process(&mut self, x: f32, fb: f32, damp: f32) -> f32 {
        let out = self.buf[self.pos];
        self.store = out * (1.0 - damp) + self.store * damp;
        self.buf[self.pos] = x + self.store * fb;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}
#[derive(Clone)]
struct Allpass {
    buf: Vec<f32>,
    pos: usize,
}
impl Allpass {
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let b = self.buf[self.pos];
        let out = b - x;
        self.buf[self.pos] = x + b * 0.5;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

const COMB_TUNING: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNING: [usize; 4] = [556, 441, 341, 225];

pub struct FxChain {
    sr: f32,
    ladder: [DexedFilter; 2],
    fconst: FilterConsts,
    g_s: f32,
    r24_s: f32,
    chorus_l: Line,
    chorus_r: Line,
    chorus_phase: [f32; 2],
    delay_l: Line,
    delay_r: Line,
    delay_lp: [f32; 2],
    pre_l: Line,
    pre_r: Line,
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<Allpass>; 2],
    smooth_gain: f32,
    pub peak: f32,
}

impl FxChain {
    pub fn new(sr: f32) -> Self {
        let scale = sr / 44100.0;
        let mk_combs = |spread: usize| {
            COMB_TUNING
                .iter()
                .map(|&t| Comb { buf: vec![0.0; ((t + spread) as f32 * scale) as usize], pos: 0, store: 0.0 })
                .collect::<Vec<_>>()
        };
        let mk_ap = |spread: usize| {
            ALLPASS_TUNING
                .iter()
                .map(|&t| Allpass { buf: vec![0.0; ((t + spread) as f32 * scale) as usize], pos: 0 })
                .collect::<Vec<_>>()
        };
        FxChain {
            sr,
            ladder: [DexedFilter::default(), DexedFilter::default()],
            fconst: FilterConsts::new(sr),
            g_s: -1.0,
            r24_s: 0.0,
            chorus_l: Line::new((sr * 0.05) as usize),
            chorus_r: Line::new((sr * 0.05) as usize),
            chorus_phase: [0.0, 0.25],
            delay_l: Line::new((sr * 1.6) as usize),
            delay_r: Line::new((sr * 1.6) as usize),
            delay_lp: [0.0; 2],
            pre_l: Line::new((sr * (PREDELAY_MAX_MS / 1000.0 + 0.01)) as usize),
            pre_r: Line::new((sr * (PREDELAY_MAX_MS / 1000.0 + 0.01)) as usize),
            combs: [mk_combs(0), mk_combs(23)],
            allpasses: [mk_ap(0), mk_ap(23)],
            smooth_gain: 1.0,
            peak: 0.0,
        }
    }

    pub fn clear(&mut self) {
        self.delay_l.clear();
        self.delay_r.clear();
        self.chorus_l.clear();
        self.chorus_r.clear();
        for ch in self.combs.iter_mut() {
            for c in ch.iter_mut() {
                c.buf.fill(0.0);
                c.store = 0.0;
            }
        }
        for ch in self.allpasses.iter_mut() {
            for a in ch.iter_mut() {
                a.buf.fill(0.0);
            }
        }
    }

    /// mono in → stereo out (interleaved pairs written into `l`, `r`).
    pub fn process(&mut self, p: &PlayParams, master_db: f32, mono: &[f32], l: &mut [f32], r: &mut [f32]) {
        let sr = self.sr;
        // filter (original: bypassed while cutoff is fully open)
        let fc = cutoff_hz(p.cutoff).min(sr * 0.45);
        let g = (std::f32::consts::PI * fc / sr).tan();
        let r_reso = 0.991 - logsc(1.0 - p.resonance.clamp(0.0, 1.0), 0.0, 0.991);
        let k = 3.5 * r_reso;
        let bypass_filter = p.cutoff >= 1.0;
        if self.g_s < 0.0 || bypass_filter {
            self.g_s = g;
            self.r24_s = k;
        }
        let smooth = self.fconst.smooth;
        let target_gain = 10f32.powf((p.volume_db + master_db) / 20.0);
        let pan = p.pan.clamp(-1.0, 1.0);
        let (pl, pr) = (((1.0 - pan) * 0.5).sqrt() * std::f32::consts::SQRT_2, ((1.0 + pan) * 0.5).sqrt() * std::f32::consts::SQRT_2);

        // chorus settings
        let ch = &p.chorus;
        let ch_mix = if ch.on { ch.amount.clamp(0.0, 1.0) } else { 0.0 };
        let rate1 = ch.rate_hz.clamp(0.05, 5.0);
        let rate2 = rate1 * CHORUS_II_RATIO;
        let depth_ms = ch.depth_ms.clamp(0.0, 5.0);
        let base_ms = CHORUS_BASE_MS;
        // right side runs (0.25 + 0.25*spread) of a cycle behind the left
        let offset = 0.25 + 0.25 * ch.spread.clamp(0.0, 1.0);
        // delay settings
        let d = &p.delay;
        let d_mix = if d.on { d.amount.clamp(0.0, 1.0) } else { 0.0 };
        let (tl, tr) = d.times();
        let (dl, dr) = (tl * 0.001 * sr, tr * 0.001 * sr);
        let d_fb = d.feedback.clamp(0.0, 0.9);
        let d_damp = d.damping.clamp(0.0, 1.0) * 0.9;
        // reverb settings
        let rv = &p.reverb;
        let r_mix = if rv.on { rv.amount.clamp(0.0, 1.0) } else { 0.0 };
        let room = 0.7 + rv.size.clamp(0.0, 1.0) * 0.28;
        let damp = rv.damping.clamp(0.0, 1.0) * 0.4;
        let pre = rv.predelay_ms.clamp(0.0, PREDELAY_MAX_MS) * 0.001 * sr;

        let mut peak = self.peak * 0.97;
        for i in 0..mono.len() {
            self.smooth_gain += (target_gain - self.smooth_gain) * 0.002;
            let mut x = mono[i];
            if !bypass_filter {
                self.g_s = g + (self.g_s - g) * smooth;
                self.r24_s = k + (self.r24_s - k) * smooth;
                let (gs, rs) = (self.g_s, self.r24_s);
                x = self.ladder[0].process(x, gs, rs, &self.fconst);
            }
            let mut a = x * pl;
            let mut b = x * pr;

            // chorus
            self.chorus_l.push(a);
            self.chorus_r.push(b);
            if ch_mix > 0.0 {
                self.chorus_phase[0] = (self.chorus_phase[0] + rate1 / sr).fract();
                self.chorus_phase[1] = (self.chorus_phase[1] + rate2 / sr).fract();
                let tri = |ph: f32| if ph < 0.5 { 4.0 * ph - 1.0 } else { 3.0 - 4.0 * ph };
                let tap = |line: &Line, ph: f32| line.read_cubic((base_ms + depth_ms * tri(ph)) * 0.001 * sr);
                let (wl, wr) = match ch.mode {
                    ChorusMode::One => (tap(&self.chorus_l, self.chorus_phase[0]), tap(&self.chorus_r, (self.chorus_phase[0] + offset).fract())),
                    ChorusMode::Two => (tap(&self.chorus_l, self.chorus_phase[1]), tap(&self.chorus_r, (self.chorus_phase[1] + offset).fract())),
                    ChorusMode::Both => (
                        0.5 * (tap(&self.chorus_l, self.chorus_phase[0]) + tap(&self.chorus_l, self.chorus_phase[1])),
                        0.5 * (tap(&self.chorus_r, (self.chorus_phase[0] + offset).fract()) + tap(&self.chorus_r, (self.chorus_phase[1] + offset).fract())),
                    ),
                };
                a = a * (1.0 - ch_mix * 0.5) + wl * ch_mix * 0.7;
                b = b * (1.0 - ch_mix * 0.5) + wr * ch_mix * 0.7;
            }

            // delay
            let yl = self.delay_l.read(dl);
            let yr = self.delay_r.read(dr);
            self.delay_lp[0] = yl * (1.0 - d_damp) + self.delay_lp[0] * d_damp;
            self.delay_lp[1] = yr * (1.0 - d_damp) + self.delay_lp[1] * d_damp;
            self.delay_l.push(a + self.delay_lp[0] * d_fb);
            self.delay_r.push(b + self.delay_lp[1] * d_fb);
            if d_mix > 0.0 {
                a += yl * d_mix;
                b += yr * d_mix;
            }

            // reverb
            self.pre_l.push(a);
            self.pre_r.push(b);
            if r_mix > 0.0 {
                let inp = (self.pre_l.read(pre) + self.pre_r.read(pre)) * 0.015;
                let mut out = [0.0f32; 2];
                for c in 0..2 {
                    let mut s = 0.0;
                    for comb in self.combs[c].iter_mut() {
                        s += comb.process(inp, room, damp);
                    }
                    for ap in self.allpasses[c].iter_mut() {
                        s = ap.process(s);
                    }
                    out[c] = s;
                }
                // JUCE reverb with dry 0, width 1: the dry sound passes untouched, wet = 3*amount
                a += out[0] * r_mix * 3.0;
                b += out[1] * r_mix * 3.0;
            }

            let gl = soft_clip(a * self.smooth_gain);
            let gr = soft_clip(b * self.smooth_gain);
            peak = peak.max(gl.abs()).max(gr.abs());
            l[i] = gl;
            r[i] = gr;
        }
        self.peak = peak;
    }
}

#[inline]
fn soft_clip(x: f32) -> f32 {
    if x.abs() < 0.8 { x } else { x.signum() * (0.8 + 0.2 * ((x.abs() - 0.8) / 0.2).tanh()) }
}
