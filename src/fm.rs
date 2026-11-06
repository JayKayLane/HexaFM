//! Six-operator FM voice engine. Rules and numbers come from MECHANICS.md.

use crate::patch::*;

/// Samples per envelope / LFO step (MECHANICS: envelope block 64).
pub const BLOCK: usize = 64;
const LG_N: i32 = 6;

/// Operator routing per algorithm, operator 6 first (MECHANICS: algorithm table).
/// bits: 0x01 out bus 1, 0x02 out bus 2, 0x04 add, 0x10 in bus 1, 0x20 in bus 2,
/// 0x40 feedback in, 0x80 feedback out.
pub const ALGORITHMS: [[u8; 6]; 32] = [
    [0xc1, 0x11, 0x11, 0x14, 0x01, 0x14],
    [0x01, 0x11, 0x11, 0x14, 0xc1, 0x14],
    [0xc1, 0x11, 0x14, 0x01, 0x11, 0x14],
    [0xc1, 0x11, 0x94, 0x01, 0x11, 0x14],
    [0xc1, 0x14, 0x01, 0x14, 0x01, 0x14],
    [0xc1, 0x94, 0x01, 0x14, 0x01, 0x14],
    [0xc1, 0x11, 0x05, 0x14, 0x01, 0x14],
    [0x01, 0x11, 0xc5, 0x14, 0x01, 0x14],
    [0x01, 0x11, 0x05, 0x14, 0xc1, 0x14],
    [0x01, 0x05, 0x14, 0xc1, 0x11, 0x14],
    [0xc1, 0x05, 0x14, 0x01, 0x11, 0x14],
    [0x01, 0x05, 0x05, 0x14, 0xc1, 0x14],
    [0xc1, 0x05, 0x05, 0x14, 0x01, 0x14],
    [0xc1, 0x05, 0x11, 0x14, 0x01, 0x14],
    [0x01, 0x05, 0x11, 0x14, 0xc1, 0x14],
    [0xc1, 0x11, 0x02, 0x25, 0x05, 0x14],
    [0x01, 0x11, 0x02, 0x25, 0xc5, 0x14],
    [0x01, 0x11, 0x11, 0xc5, 0x05, 0x14],
    [0xc1, 0x14, 0x14, 0x01, 0x11, 0x14],
    [0x01, 0x05, 0x14, 0xc1, 0x14, 0x14],
    [0x01, 0x14, 0x14, 0xc1, 0x14, 0x14],
    [0xc1, 0x14, 0x14, 0x14, 0x01, 0x14],
    [0xc1, 0x14, 0x14, 0x01, 0x14, 0x04],
    [0xc1, 0x14, 0x14, 0x14, 0x04, 0x04],
    [0xc1, 0x14, 0x14, 0x04, 0x04, 0x04],
    [0xc1, 0x05, 0x14, 0x01, 0x14, 0x04],
    [0x01, 0x05, 0x14, 0xc1, 0x14, 0x04],
    [0x04, 0xc1, 0x11, 0x14, 0x01, 0x14],
    [0xc1, 0x14, 0x01, 0x14, 0x04, 0x04],
    [0x04, 0xc1, 0x11, 0x14, 0x04, 0x04],
    [0xc1, 0x14, 0x04, 0x04, 0x04, 0x04],
    [0xc4, 0x04, 0x04, 0x04, 0x04, 0x04],
];

/// Is operator `op` (0 = operator 1) a carrier in this algorithm?
pub fn is_carrier(alg: usize, op: usize) -> bool {
    let f = ALGORITHMS[alg & 31][5 - op];
    f & 0x03 == 0
}

/// Which operators (0 = operator 1) modulate `op` in this algorithm.
pub fn modulators_of(alg: usize, op: usize) -> Vec<usize> {
    let flags = ALGORITHMS[alg & 31];
    let idx = 5 - op; // processing order index
    let mut out = Vec::new();
    let inbus = (flags[idx] >> 4) & 3;
    if inbus != 0 {
        // Walk back to the ops that wrote that bus since it was last overwritten.
        let mut j = idx;
        while j > 0 {
            j -= 1;
            let f = flags[j];
            if f & 3 == inbus {
                out.push(5 - j);
                if f & 4 == 0 {
                    break;
                }
            }
        }
    }
    if flags[idx] & 0xc0 == 0xc0 {
        out.push(op);
    }
    out
}

const LEVEL_LUT: [i32; 20] = [0, 5, 9, 13, 17, 20, 23, 25, 27, 29, 31, 33, 35, 37, 39, 41, 42, 43, 45, 46];
const VELOCITY_DATA: [i32; 64] = [
    0, 70, 86, 97, 106, 114, 121, 126, 132, 138, 142, 148, 152, 156, 160, 163, 166, 170, 173, 174, 178, 181,
    184, 186, 189, 190, 194, 196, 198, 200, 202, 205, 206, 209, 211, 214, 216, 218, 220, 222, 224, 225, 227,
    229, 230, 232, 233, 235, 237, 238, 240, 241, 242, 243, 244, 246, 246, 248, 249, 250, 251, 252, 253, 254,
];
const EXP_SCALE_DATA: [i32; 33] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 14, 16, 19, 23, 27, 33, 39, 47, 56, 66, 80, 94, 110, 126, 142, 158, 174,
    190, 206, 222, 238, 250,
];
const AMP_MOD_SENS: [i32; 4] = [0, 4342338, 7171437, 16777216];
const PITCH_MOD_SENS: [i32; 8] = [0, 10, 20, 33, 55, 92, 153, 255];

fn scale_out_level(x: i32) -> i32 {
    if x >= 20 { 28 + x } else { LEVEL_LUT[x.max(0) as usize] }
}

fn scale_curve(group: i32, depth: i32, curve: i32) -> i32 {
    let scale = if curve == 0 || curve == 3 {
        (group * depth * 329) >> 12
    } else {
        let raw = EXP_SCALE_DATA[group.clamp(0, 32) as usize];
        (raw * depth * 329) >> 15
    };
    if curve < 2 { -scale } else { scale }
}

fn scale_level(note: i32, bp: i32, ld: i32, rd: i32, lc: i32, rc: i32) -> i32 {
    let offset = note - bp - 17;
    if offset >= 0 {
        scale_curve((offset + 1) / 3, rd, rc)
    } else {
        scale_curve(-(offset - 1) / 3, ld, lc)
    }
}

fn scale_rate(note: i32, sens: i32) -> i32 {
    let x = (note / 3 - 7).clamp(0, 31);
    (sens * x) >> 3
}

fn scale_velocity(vel: i32, sens: i32) -> i32 {
    let v = VELOCITY_DATA[(vel.clamp(0, 127) >> 1) as usize] - 239;
    ((sens * v + 7) >> 3) << 4
}

#[derive(Clone, Default)]
struct Env {
    rates: [i32; 4],
    levels: [i32; 4],
    outlevel: i32,
    rate_scaling: i32,
    level: i32,
    target: i32,
    rising: bool,
    ix: usize,
    inc: i32,
    down: bool,
    sr_mult: i64,
}

impl Env {
    fn init(&mut self, rates: [i32; 4], levels: [i32; 4], outlevel: i32, rate_scaling: i32, sr_mult: i64) {
        self.rates = rates;
        self.levels = levels;
        self.outlevel = outlevel;
        self.rate_scaling = rate_scaling;
        self.sr_mult = sr_mult;
        self.level = 0;
        self.down = true;
        self.advance(0);
    }

    fn advance(&mut self, ix: usize) {
        self.ix = ix;
        if ix < 4 {
            let mut actual = scale_out_level(self.levels[ix]) >> 1;
            actual = ((actual << 6) + self.outlevel).max(4272) - 4256;
            self.target = actual << 16;
            self.rising = self.target > self.level;
            let mut qrate = (self.rates[ix] * 41) >> 6;
            qrate = (qrate + self.rate_scaling).min(63);
            let inc = ((4 + (qrate & 3)) as i64) << (2 + LG_N + (qrate >> 2));
            self.inc = ((inc * self.sr_mult) >> 24) as i32;
        }
    }

    fn keyup(&mut self) {
        if self.down {
            self.down = false;
            self.advance(3);
        }
    }

    fn step(&mut self) -> i32 {
        if self.ix < 3 || (self.ix < 4 && !self.down) {
            if self.rising {
                const JUMP: i32 = 1716 << 16;
                if self.level < JUMP {
                    self.level = JUMP;
                }
                let delta = (((17i64 << 24) - self.level as i64) >> 24) * self.inc as i64;
                self.level = (self.level as i64 + delta).min(i32::MAX as i64) as i32;
                if self.level >= self.target {
                    self.level = self.target;
                    self.advance(self.ix + 1);
                }
            } else {
                self.level -= self.inc;
                if self.level <= self.target {
                    self.level = self.target;
                    self.advance(self.ix + 1);
                }
            }
        }
        self.level
    }

    fn finished(&self) -> bool {
        self.ix >= 4
    }
}

/// Pitch envelope level table (program constant 0x1004b52cd); level = value << 19 in Q24 octaves.
const PITCHENV_TAB: [i8; 100] = [
    -128, -116, -104, -95, -85, -76, -68, -61, -56, -52, -49, -46, -43, -41, -39, -37, -35, -33, -32, -31, -30, -29,
    -28, -27, -26, -25, -24, -23, -22, -21, -20, -19, -18, -17, -16, -15, -14, -13, -12, -11, -10, -9, -8, -7, -6,
    -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 38, 40, 43, 46, 49, 53, 58, 65, 73, 82, 92, 103, 115, 127,
];
/// Pitch envelope rate table (program constant 0x1004b5261).
const PITCHENV_RATE: [u8; 100] = [
    1, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 16, 16, 17, 18, 18,
    19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 30, 31, 33, 34, 36, 37, 38, 39, 41, 42, 44, 46, 47, 49, 51, 53, 54, 56,
    58, 60, 62, 64, 66, 68, 70, 72, 74, 76, 79, 82, 85, 88, 91, 94, 98, 102, 106, 110, 115, 120, 125, 130, 135, 141,
    147, 153, 159, 165, 171, 178, 185, 193, 202, 211, 232, 243, 254, 255,
];
/// Pitch envelope step divisor: unit = 64·2^24 / (21.3·sample rate) per block (prepareToPlay 0x100016ce0).
const PITCHENV_DIV: f64 = 21.3;

/// Pitch envelope, same staging as the amplitude envelope, in Q24 octave units.
#[derive(Clone, Default)]
struct PitchEnv {
    rates: [i32; 4],
    levels: [i32; 4],
    level: i64,
    target: i64,
    rising: bool,
    inc: i64,
    ix: usize,
    down: bool,
    unit: i64,
}

impl PitchEnv {
    fn init(&mut self, rates: [i32; 4], levels: [i32; 4], sr: f32) {
        self.unit = ((BLOCK as f64) * (1u64 << 24) as f64 / (PITCHENV_DIV * sr as f64) + 0.5) as i64;
        self.rates = rates;
        self.levels = levels;
        self.level = (PITCHENV_TAB[levels[3].clamp(0, 99) as usize] as i64) << 19;
        self.down = true;
        self.advance(0);
    }
    fn advance(&mut self, ix: usize) {
        self.ix = ix;
        if ix < 4 {
            self.target = (PITCHENV_TAB[self.levels[ix].clamp(0, 99) as usize] as i64) << 19;
            self.rising = self.target > self.level;
            self.inc = PITCHENV_RATE[self.rates[ix].clamp(0, 99) as usize] as i64 * self.unit;
        }
    }
    fn keyup(&mut self) {
        if self.down {
            self.down = false;
            self.advance(3);
        }
    }
    /// One block step; returns the offset in octaves.
    fn step(&mut self, n: usize) -> f32 {
        if self.ix < 3 || (self.ix < 4 && !self.down) {
            let inc = self.inc * n as i64 / BLOCK as i64;
            if self.rising {
                self.level += inc;
                if self.level >= self.target {
                    self.level = self.target;
                    self.advance(self.ix + 1);
                }
            } else {
                self.level -= inc;
                if self.level <= self.target {
                    self.level = self.target;
                    self.advance(self.ix + 1);
                }
            }
        }
        self.level as f32 / (1u32 << 24) as f32
    }
}

#[derive(Clone, Default)]
struct Op {
    env: Env,
    phase: f32,
    freq_ratio_log: f32, // log2 of the frequency (Hz) before pitch modulation
    fixed: bool,
    gain: f32,
    ams: i32,
    fb: [f32; 2],
}

#[derive(Clone, Default)]
pub struct FmVoice {
    pub note: i32,
    pub velocity: i32,
    pub active: bool,
    pub key_down: bool,
    pub sustained: bool,
    pub age: u64,
    ops: [Op; 6],
    penv: PitchEnv,
    glide_from: f32,
}

/// LFO speed in Hz for speed 0..99 (program table `lfoSource` 0x1004b4f40).
const LFO_HZ: [f64; 100] = [
    0.062541, 0.125031, 0.312393, 0.43712, 0.62461, 0.750694, 0.93633, 1.125302, 1.249609, 1.436782, 1.560915,
    1.752081, 1.875117, 2.062494, 2.247191, 2.374451, 2.560492, 2.686728, 2.873976, 2.99895, 3.188013, 3.36984,
    3.500175, 3.682224, 3.812065, 4.0008, 4.186202, 4.310716, 4.50126, 4.623209, 4.814636, 4.93048, 5.121901,
    5.315191, 5.434783, 5.617346, 5.750431, 5.946717, 6.062811, 6.248438, 6.431695, 6.564264, 6.74946, 6.868132,
    7.052186, 7.25058, 7.375719, 7.556294, 7.687577, 7.877738, 7.993605, 8.181967, 8.372405, 8.504848, 8.685079,
    8.810573, 8.986341, 9.122423, 9.300595, 9.500285, 9.607994, 9.798158, 9.950249, 10.117361, 11.251125,
    11.384335, 12.562814, 13.676149, 13.904338, 15.092062, 16.366612, 16.638935, 17.869907, 19.193858, 19.425019,
    20.833333, 21.034918, 22.50225, 24.003841, 24.260068, 25.746653, 27.173913, 27.578599, 29.052876, 30.693677,
    31.191516, 32.658393, 34.31709, 34.674064, 36.416606, 38.197097, 38.550501, 40.387722, 40.749796, 42.625746,
    44.326241, 44.883303, 46.772685, 48.590865, 49.261084,
];
/// LFO delay unit: 64·25190424 / sample rate per block (prepareToPlay 0x100016ce0).
const LFO_DELAY_UNIT: f64 = 25190424.0;

/// Global LFO in the original's fixed-point form: 32-bit phase, Q24 output (0..1).
#[derive(Clone, Default)]
pub struct Lfo {
    phase: u32,
    delay_state: u32,
    rand: u32,
    sample: u32,
}

impl Lfo {
    pub fn keydown(&mut self, sync: bool) {
        if sync {
            self.phase = (1u32 << 31) - 1;
        }
        self.delay_state = 0;
    }
    /// Advance one block. Returns (value 0..1, delay ramp 0..1).
    fn step(&mut self, v: &Voice, n: usize, sr: f32) -> (f32, f32) {
        let delta = (LFO_HZ[(v.get(LFO_SPEED) as usize).min(99)] * 4294967296.0 * n as f64 / sr as f64) as u64;
        let (np, wrapped) = self.phase.overflowing_add(delta.min(u32::MAX as u64) as u32);
        self.phase = np;
        let x: u32 = match v.get(LFO_WAVE) {
            0 => {
                let mut t = np >> 7;
                if np >> 31 != 0 {
                    t = !t;
                }
                t & ((1 << 24) - 1)
            }
            1 => (!np ^ (1u32 << 31)) >> 8,
            2 => (np ^ (1u32 << 31)) >> 8,
            3 => ((!np) >> 7) & (1 << 24),
            4 => ((1u32 << 23) as f32 + ((np as f64 / 4294967296.0 * std::f64::consts::TAU).sin() as f32) * (1u32 << 23) as f32) as u32,
            _ => {
                if wrapped {
                    self.rand = (self.rand.wrapping_mul(179).wrapping_add(17)) & 0xff;
                }
                self.rand << 16
            }
        };
        self.sample = x;
        // delay: silent for the first half of the delay counter, then a ramp to full depth
        let d = v.get(LFO_DELAY) as i32;
        let ramp = if d == 0 {
            1.0
        } else {
            let unit = BLOCK as f64 * LFO_DELAY_UNIT / sr as f64;
            let a = 99 - d;
            let inc1 = ((16 + (a & 15)) << (1 + (a >> 4))) as f64;
            let inc2 = (((16 + (a & 15)) << (1 + (a >> 4))) & 0xff80).max(0x80) as f64;
            let inc = (if self.delay_state < (1u32 << 31) { inc1 } else { inc2 }) * unit * n as f64 / BLOCK as f64;
            let next = self.delay_state as f64 + inc;
            if next > u32::MAX as f64 {
                self.delay_state = u32::MAX;
                1.0
            } else {
                self.delay_state = next as u32;
                if self.delay_state < (1u32 << 31) { 0.0 } else { ((self.delay_state >> 7) & ((1 << 24) - 1)) as f32 / (1u32 << 24) as f32 }
            }
        };
        (x.min(1 << 24) as f32 / (1u32 << 24) as f32, ramp)
    }
}

/// Sine lookup with linear interpolation.
pub struct SineTable(Vec<f32>);
const SIN_SIZE: usize = 4096;
impl SineTable {
    pub fn new() -> Self {
        SineTable((0..=SIN_SIZE).map(|i| (i as f32 / SIN_SIZE as f32 * std::f32::consts::TAU).sin()).collect())
    }
    #[inline]
    fn sin(&self, phase: f32) -> f32 {
        let p = (phase - phase.floor()) * SIN_SIZE as f32;
        if !p.is_finite() {
            return 0.0;
        }
        let ip = p as usize;
        let f = p - ip as f32;
        let i = ip & (SIN_SIZE - 1);
        let a = self.0[i];
        a + (self.0[i + 1] - a) * f
    }
}

pub struct Controllers {
    pub pitch_bend: f32,    // -1..1
    pub bend_up: f32,       // semitones
    pub bend_down: f32,     // semitones
    pub mod_wheel: f32,     // 0..1
    pub wheel_pitch: f32,   // 0..1 how much the wheel adds to pitch depth
    pub wheel_amp: f32,     // 0..1
    pub master_tune: f32,   // cents
    pub portamento: f32,    // seconds, 0 = off
}

impl Default for Controllers {
    fn default() -> Self {
        Controllers { pitch_bend: 0.0, bend_up: 2.0, bend_down: 2.0, mod_wheel: 0.0, wheel_pitch: 0.5, wheel_amp: 0.0, master_tune: 0.0, portamento: 0.0 }
    }
}

pub struct FmEngine {
    pub sr: f32,
    pub voices: Vec<FmVoice>,
    pub patch: Voice,
    pub lfo: Lfo,
    pub ctrl: Controllers,
    pub mono: bool,
    pub sustain: bool,
    pub op_enabled: [bool; 6],
    sine: SineTable,
    counter: u64,
    last_note_log: f32,
    pub op_meter: [f32; 6],
}

pub const POLYPHONY: usize = 16;
/// Each voice's Q24 sum is shifted >>4 then >>9 and scaled by 2^-15, i.e. divided by 16, then
/// clipped to ±1 per voice (processBlock 0x100018000, vector conversion after FmCore::render).
pub const VOICE_SCALE: f32 = 1.0 / 16.0;

impl FmEngine {
    pub fn new(sr: f32) -> Self {
        FmEngine {
            sr,
            voices: vec![FmVoice::default(); POLYPHONY],
            patch: Voice::init(),
            lfo: Lfo::default(),
            ctrl: Controllers::default(),
            mono: false,
            sustain: false,
            op_enabled: [true; 6],
            sine: SineTable::new(),
            counter: 0,
            last_note_log: 0.0,
            op_meter: [0.0; 6],
        }
    }

    fn sr_mult(&self) -> i64 {
        ((44100.0 / self.sr as f64) * (1u64 << 24) as f64) as i64
    }

    fn op_log_freq(v: &Voice, op: usize, note: i32) -> (f32, bool) {
        let mode = v.op(op, OP_MODE);
        let coarse = v.op(op, OP_COARSE) as i32;
        let fine = v.op(op, OP_FINE) as f32;
        let detune = v.op(op, OP_DETUNE) as f32 - 7.0;
        if mode == 1 {
            let n = ((coarse & 3) * 100) as f32 + fine;
            let mut lf = n * 0.033_219_28;
            if detune > 0.0 {
                lf += 13457.0 * detune / (1u32 << 24) as f32;
            }
            (lf, true)
        } else {
            let base = (440.0f32 * 2f32.powf((note as f32 - 69.0) / 12.0)).log2();
            let det_ratio = 0.0209 * (-0.396 * base).exp() / 7.0;
            let mut lf = base + det_ratio * base * detune;
            let ratio = if coarse == 0 { 0.5 } else { coarse as f32 };
            lf += ratio.log2() + (1.0 + fine * 0.01).log2();
            (lf, false)
        }
    }

    pub fn note_on(&mut self, note: u8, vel: u8) {
        let v = self.patch.clone();
        let note_t = note as i32 + v.get(TRANSPOSE) as i32 - 24;
        self.counter += 1;
        let idx = if self.mono {
            0
        } else if let Some(i) = self.voices.iter().position(|x| !x.active) {
            i
        } else {
            // steal: prefer released voices, then the oldest
            let mut best = 0;
            let mut best_key = (true, u64::MAX);
            for (i, x) in self.voices.iter().enumerate() {
                let key = (x.key_down, x.age);
                if key < best_key {
                    best_key = key;
                    best = i;
                }
            }
            best
        };
        let sr_mult = self.sr_mult();
        let legato = self.mono && self.voices[0].active && self.voices[0].key_down;
        let voice = &mut self.voices[idx];
        voice.note = note as i32;
        voice.velocity = vel as i32;
        voice.active = true;
        voice.key_down = true;
        voice.sustained = false;
        voice.age = self.counter;
        let note_log = (440.0f32 * 2f32.powf((note_t as f32 - 69.0) / 12.0)).log2();
        voice.glide_from = if self.ctrl.portamento > 0.0 && self.last_note_log != 0.0 { self.last_note_log - note_log } else { 0.0 };
        self.last_note_log = note_log;
        let sync = v.get(OSC_SYNC) == 1;
        for op in 0..6 {
            let mut rates = [0; 4];
            let mut levels = [0; 4];
            for i in 0..4 {
                rates[i] = v.op(op, OP_R1 + i) as i32;
                levels[i] = v.op(op, OP_L1 + i) as i32;
            }
            let mut outlevel = scale_out_level(v.op(op, OP_LEVEL) as i32);
            outlevel += scale_level(
                note_t,
                v.op(op, OP_BREAK) as i32,
                v.op(op, OP_LDEPTH) as i32,
                v.op(op, OP_RDEPTH) as i32,
                v.op(op, OP_LCURVE) as i32,
                v.op(op, OP_RCURVE) as i32,
            );
            outlevel = outlevel.min(127) << 5;
            outlevel += scale_velocity(vel as i32, v.op(op, OP_VEL) as i32);
            outlevel = outlevel.max(0);
            let rs = scale_rate(note_t, v.op(op, OP_RATESCALE) as i32);
            let o = &mut voice.ops[op];
            if legato {
                o.env.outlevel = outlevel;
            } else {
                o.env.init(rates, levels, outlevel, rs, sr_mult);
            }
            let (lf, fixed) = Self::op_log_freq(&v, op, note_t);
            o.freq_ratio_log = lf;
            o.fixed = fixed;
            o.ams = v.op(op, OP_AMS) as i32;
            if sync && !legato {
                o.phase = 0.0;
                o.fb = [0.0; 2];
            }
        }
        if !legato {
            let mut pr = [0; 4];
            let mut pl = [0; 4];
            for i in 0..4 {
                pr[i] = v.get(PEG_R1 + i) as i32;
                pl[i] = v.get(PEG_L1 + i) as i32;
            }
            voice.penv.init(pr, pl, self.sr);
        }
        self.lfo.keydown(v.get(LFO_SYNC) == 1);
    }

    pub fn note_off(&mut self, note: u8) {
        let sustain = self.sustain;
        for voice in self.voices.iter_mut() {
            if voice.active && voice.key_down && voice.note == note as i32 {
                voice.key_down = false;
                if sustain {
                    voice.sustained = true;
                } else {
                    Self::release(voice);
                }
            }
        }
    }

    fn release(voice: &mut FmVoice) {
        for o in voice.ops.iter_mut() {
            o.env.keyup();
        }
        voice.penv.keyup();
    }

    pub fn set_sustain(&mut self, on: bool) {
        self.sustain = on;
        if !on {
            for voice in self.voices.iter_mut() {
                if voice.sustained {
                    voice.sustained = false;
                    Self::release(voice);
                }
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        for voice in self.voices.iter_mut() {
            voice.active = false;
            voice.key_down = false;
        }
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// Render one block (at most BLOCK samples) adding into `out` (mono).
    pub fn render_block(&mut self, out: &mut [f32]) {
        let n = out.len().min(BLOCK);
        let dt_block = n as f32 / self.sr;
        let sr = self.sr;
        let patch = &self.patch;
        let (lfo_u, delay) = self.lfo.step(patch, n, sr);
        let alg = (patch.get(ALGORITHM) & 31) as usize;
        let flags = ALGORITHMS[alg];
        // Dx7Note::update 0x100008bac: fb shift = 8 - feedback (16 = off); depths = value*165 >> 6
        let fb_amount = patch.get(FEEDBACK) as i32;
        let fb_scale = if fb_amount == 0 { 0.0 } else { 1.0 / (1u32 << (8 - fb_amount)) as f32 };
        let pmd_s = ((patch.get(LFO_PMD) as i32 * 165) >> 6) as f32;
        let amd_s = ((patch.get(LFO_AMD) as i32 * 165) >> 6) as f32;
        let pms = PITCH_MOD_SENS[(patch.get(PMS) & 7) as usize] as f32;
        // pitch: pmod1 = pmd*delay*pms*(lfo-1/2)/32768 octaves; wheel: wheel*127*pms*(lfo-1/2)/16384
        let senslfo = pms * (lfo_u - 0.5);
        let p1 = (pmd_s * delay * senslfo / 32768.0).abs();
        let p2 = (self.ctrl.mod_wheel * self.ctrl.wheel_pitch * 127.0 * senslfo / 16384.0).abs();
        let pitch_lfo = p1.max(p2) * senslfo.signum();
        // amp: amod1 = amd*delay*(1-lfo)*2^16; wheel amod2 = wheel*127*(1-lfo)*2^17
        let inv = 1.0 - lfo_u;
        let amod1 = amd_s * delay * inv * 65536.0;
        let amod2 = self.ctrl.mod_wheel * self.ctrl.wheel_amp * 127.0 * inv * 131072.0;
        let amd_mod = amod1.max(amod2);
        let bend = if self.ctrl.pitch_bend >= 0.0 { self.ctrl.pitch_bend * self.ctrl.bend_up } else { self.ctrl.pitch_bend * self.ctrl.bend_down } / 12.0;
        let tune = self.ctrl.master_tune / 1200.0;
        let porta = self.ctrl.portamento;
        let op_enabled = self.op_enabled;
        let mut meter = [0.0f32; 6];

        let mut bus1 = [0.0f32; BLOCK];
        let mut bus2 = [0.0f32; BLOCK];
        let mut ybuf = [0.0f32; BLOCK];
        let mut vbuf = [0.0f32; BLOCK];

        for voice in self.voices.iter_mut() {
            if !voice.active {
                continue;
            }
            let penv = voice.penv.step(n);
            vbuf[..n].fill(0.0);
            if porta > 0.0 && voice.glide_from != 0.0 {
                let k = (-dt_block / porta * 4.0).exp();
                voice.glide_from *= k;
                if voice.glide_from.abs() < 0.0005 {
                    voice.glide_from = 0.0;
                }
            }
            let shift = penv + pitch_lfo + bend + tune + voice.glide_from;
            bus1[..n].fill(0.0);
            bus2[..n].fill(0.0);
            let mut all_done = true;
            for (k, &f) in flags.iter().enumerate() {
                let op = 5 - k;
                let o = &mut voice.ops[op];
                let level = o.env.step();
                let carrier = f & 3 == 0;
                if carrier && !o.env.finished() {
                    all_done = false;
                }
                let mut level = level as f32;
                if o.ams != 0 {
                    // processBlock: level -= level * exp(sens/262144*0.07 + 12.2) / 2^24
                    let sensamp = amd_mod * AMP_MOD_SENS[o.ams as usize] as f32 / 16777216.0;
                    let pt = (sensamp as f64 / 262144.0 * 0.07 + 12.2).exp() as f32;
                    level = (level - level * pt / 16777216.0).max(0.0);
                }
                let target_gain = if op_enabled[op] { 2f32.powf(level / 16777216.0 - 14.0) } else { 0.0 };
                let g0 = o.gain;
                let g1 = target_gain;
                o.gain = g1;
                let freq = 2f32.powf(o.freq_ratio_log + if o.fixed { 0.0 } else { shift });
                let inc = freq / sr;
                let inbus = (f >> 4) & 3;
                let self_fb = f & 0xc0 == 0xc0 && fb_scale > 0.0;
                let ginc = (g1 - g0) / n as f32;
                let mut g = g0;
                let mut ph = o.phase;
                let mut peak = 0.0f32;
                for i in 0..n {
                    let mut m = match inbus {
                        1 => bus1[i],
                        2 => bus2[i],
                        _ => 0.0,
                    };
                    if self_fb {
                        m += (o.fb[0] + o.fb[1]) * 0.5 * fb_scale;
                    }
                    let y = self.sine.sin(ph + m) * g;
                    if self_fb {
                        o.fb[0] = o.fb[1];
                        o.fb[1] = y;
                    }
                    ybuf[i] = y;
                    ph += inc;
                    g += ginc;
                    peak = peak.max(g);
                }
                o.phase = ph - ph.floor();
                meter[op] = meter[op].max(peak);
                let add = f & 4 != 0;
                let dst: &mut [f32] = match f & 3 {
                    1 => &mut bus1[..n],
                    2 => &mut bus2[..n],
                    _ => &mut vbuf[..n],
                };
                if add || f & 3 == 0 {
                    for i in 0..n {
                        dst[i] += ybuf[i];
                    }
                } else {
                    dst.copy_from_slice(&ybuf[..n]);
                }
            }
            for i in 0..n {
                out[i] += (vbuf[i] * VOICE_SCALE).clamp(-1.0, 1.0);
            }
            if all_done && !voice.key_down {
                voice.active = false;
                for o in voice.ops.iter_mut() {
                    o.gain = 0.0;
                }
            }
        }
        for i in 0..6 {
            self.op_meter[i] = (self.op_meter[i] * 0.9).max(meter[i] / 2.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn makes_sound_and_stops() {
        let mut e = FmEngine::new(48000.0);
        e.note_on(60, 100);
        let mut buf = vec![0.0f32; BLOCK];
        let mut peak = 0.0f32;
        for _ in 0..100 {
            buf.fill(0.0);
            e.render_block(&mut buf);
            peak = buf.iter().fold(peak, |a, &b| a.max(b.abs()));
        }
        assert!(peak > 0.05, "peak {peak}");
        e.note_off(60);
        for _ in 0..4000 {
            buf.fill(0.0);
            e.render_block(&mut buf);
        }
        assert_eq!(e.active_voices(), 0);
    }
    #[test]
    fn carriers() {
        assert!(is_carrier(0, 0));
        assert!(!is_carrier(0, 1));
        assert!(is_carrier(31, 5));
        assert_eq!(modulators_of(0, 0), vec![1]);
        assert_eq!(modulators_of(0, 5), vec![5]);
    }
}
