//! Voice data in the classic 155-byte six-operator layout, plus SysEx bank reading and writing.

pub const VOICE_SIZE: usize = 155;
pub const OP_SIZE: usize = 21;

// Offsets inside one operator block (operator 6 is stored first).
pub const OP_R1: usize = 0;
pub const OP_L1: usize = 4;
pub const OP_BREAK: usize = 8;
pub const OP_LDEPTH: usize = 9;
pub const OP_RDEPTH: usize = 10;
pub const OP_LCURVE: usize = 11;
pub const OP_RCURVE: usize = 12;
pub const OP_RATESCALE: usize = 13;
pub const OP_AMS: usize = 14;
pub const OP_VEL: usize = 15;
pub const OP_LEVEL: usize = 16;
pub const OP_MODE: usize = 17;
pub const OP_COARSE: usize = 18;
pub const OP_FINE: usize = 19;
pub const OP_DETUNE: usize = 20;

pub const PEG_R1: usize = 126;
pub const PEG_L1: usize = 130;
pub const ALGORITHM: usize = 134;
pub const FEEDBACK: usize = 135;
pub const OSC_SYNC: usize = 136;
pub const LFO_SPEED: usize = 137;
pub const LFO_DELAY: usize = 138;
pub const LFO_PMD: usize = 139;
pub const LFO_AMD: usize = 140;
pub const LFO_SYNC: usize = 141;
pub const LFO_WAVE: usize = 142;
pub const PMS: usize = 143;
pub const TRANSPOSE: usize = 144;
pub const NAME: usize = 145;

/// Upper limit of every byte in the voice.
pub fn param_max(index: usize) -> u8 {
    if index < 126 {
        match index % OP_SIZE {
            OP_LCURVE | OP_RCURVE => 3,
            OP_RATESCALE => 7,
            OP_AMS => 3,
            OP_VEL => 7,
            OP_MODE => 1,
            OP_COARSE => 31,
            OP_DETUNE => 14,
            _ => 99,
        }
    } else {
        match index {
            ALGORITHM => 31,
            FEEDBACK => 7,
            OSC_SYNC | LFO_SYNC => 1,
            LFO_WAVE => 5,
            PMS => 7,
            TRANSPOSE => 48,
            i if i >= NAME => 127,
            _ => 99,
        }
    }
}

/// Byte offset of a parameter of operator `op` (0 = operator 1 as shown to the player).
pub fn op_index(op: usize, param: usize) -> usize {
    (5 - op) * OP_SIZE + param
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Voice(pub [u8; VOICE_SIZE]);

impl Default for Voice {
    fn default() -> Self {
        Voice::init()
    }
}

impl Voice {
    /// The classic "init voice": one sine carrier at full level.
    pub fn init() -> Self {
        let mut d = [0u8; VOICE_SIZE];
        for op in 0..6 {
            let b = op_index(op, 0);
            for i in 0..4 {
                d[b + OP_R1 + i] = 99;
            }
            d[b + OP_L1] = 99;
            d[b + OP_L1 + 1] = 99;
            d[b + OP_L1 + 2] = 99;
            d[b + OP_L1 + 3] = 0;
            d[b + OP_BREAK] = 39;
            d[b + OP_COARSE] = 1;
            d[b + OP_DETUNE] = 7;
            d[b + OP_LEVEL] = if op == 0 { 99 } else { 0 };
        }
        for i in 0..4 {
            d[PEG_R1 + i] = 99;
            d[PEG_L1 + i] = 50;
        }
        d[OSC_SYNC] = 1;
        d[LFO_SPEED] = 35;
        d[LFO_SYNC] = 1;
        d[PMS] = 3;
        d[TRANSPOSE] = 24;
        let mut v = Voice(d);
        v.set_name("INIT VOICE");
        v
    }

    pub fn name(&self) -> String {
        let s: String = self.0[NAME..NAME + 10]
            .iter()
            .map(|&c| if (32..127).contains(&c) { c as char } else { ' ' })
            .collect();
        s.trim_end().to_string()
    }

    pub fn set_name(&mut self, name: &str) {
        let bytes: Vec<u8> = name.bytes().filter(|b| (32..127).contains(b)).take(10).collect();
        for i in 0..10 {
            self.0[NAME + i] = *bytes.get(i).unwrap_or(&b' ');
        }
    }

    pub fn clamp(&mut self) {
        for i in 0..VOICE_SIZE {
            let m = param_max(i);
            if self.0[i] > m {
                self.0[i] = m;
            }
        }
    }

    pub fn get(&self, i: usize) -> u8 {
        self.0[i]
    }

    pub fn op(&self, op: usize, p: usize) -> u8 {
        self.0[op_index(op, p)]
    }

    pub fn set_op(&mut self, op: usize, p: usize, v: u8) {
        let i = op_index(op, p);
        self.0[i] = v.min(param_max(i));
    }

    /// Pack into the 128-byte bank layout.
    pub fn pack(&self) -> [u8; 128] {
        let d = &self.0;
        let mut p = [0u8; 128];
        for op in 0..6 {
            let s = op * OP_SIZE;
            let t = op * 17;
            p[t..t + 11].copy_from_slice(&d[s..s + 11]);
            p[t + 11] = (d[s + OP_LCURVE] & 3) | ((d[s + OP_RCURVE] & 3) << 2);
            p[t + 12] = (d[s + OP_RATESCALE] & 7) | ((d[s + OP_DETUNE] & 15) << 3);
            p[t + 13] = (d[s + OP_AMS] & 3) | ((d[s + OP_VEL] & 7) << 2);
            p[t + 14] = d[s + OP_LEVEL];
            p[t + 15] = (d[s + OP_MODE] & 1) | ((d[s + OP_COARSE] & 31) << 1);
            p[t + 16] = d[s + OP_FINE];
        }
        p[102..110].copy_from_slice(&d[126..134]);
        p[110] = d[ALGORITHM] & 31;
        p[111] = (d[FEEDBACK] & 7) | ((d[OSC_SYNC] & 1) << 3);
        p[112] = d[LFO_SPEED];
        p[113] = d[LFO_DELAY];
        p[114] = d[LFO_PMD];
        p[115] = d[LFO_AMD];
        p[116] = (d[LFO_SYNC] & 1) | ((d[LFO_WAVE] & 7) << 1) | ((d[PMS] & 7) << 4);
        p[117] = d[TRANSPOSE];
        p[118..128].copy_from_slice(&d[NAME..NAME + 10]);
        p
    }

    pub fn unpack(p: &[u8]) -> Voice {
        let mut d = [0u8; VOICE_SIZE];
        for op in 0..6 {
            let s = op * OP_SIZE;
            let t = op * 17;
            d[s..s + 11].copy_from_slice(&p[t..t + 11]);
            d[s + OP_LCURVE] = p[t + 11] & 3;
            d[s + OP_RCURVE] = (p[t + 11] >> 2) & 3;
            d[s + OP_RATESCALE] = p[t + 12] & 7;
            d[s + OP_DETUNE] = (p[t + 12] >> 3) & 15;
            d[s + OP_AMS] = p[t + 13] & 3;
            d[s + OP_VEL] = (p[t + 13] >> 2) & 7;
            d[s + OP_LEVEL] = p[t + 14];
            d[s + OP_MODE] = p[t + 15] & 1;
            d[s + OP_COARSE] = (p[t + 15] >> 1) & 31;
            d[s + OP_FINE] = p[t + 16];
        }
        d[126..134].copy_from_slice(&p[102..110]);
        d[ALGORITHM] = p[110] & 31;
        d[FEEDBACK] = p[111] & 7;
        d[OSC_SYNC] = (p[111] >> 3) & 1;
        d[LFO_SPEED] = p[112];
        d[LFO_DELAY] = p[113];
        d[LFO_PMD] = p[114];
        d[LFO_AMD] = p[115];
        d[LFO_SYNC] = p[116] & 1;
        d[LFO_WAVE] = (p[116] >> 1) & 7;
        d[PMS] = (p[116] >> 4) & 7;
        d[TRANSPOSE] = p[117];
        d[NAME..NAME + 10].copy_from_slice(&p[118..128]);
        let mut v = Voice(d);
        v.clamp();
        v
    }
}

fn checksum(data: &[u8]) -> u8 {
    let sum: u32 = data.iter().map(|&b| b as u32).sum();
    ((128 - (sum & 127)) & 127) as u8
}

/// Read every voice found in a SysEx file: 32-voice banks, single voices, or raw 4096-byte data.
pub fn read_syx(bytes: &[u8]) -> Result<Vec<Voice>, String> {
    let mut voices = Vec::new();
    let mut i = 0;
    while i + 6 <= bytes.len() {
        if bytes[i] == 0xF0 && bytes[i + 1] == 0x43 {
            let fmt = bytes[i + 3];
            if fmt == 0x09 && i + 6 + 4096 <= bytes.len() {
                let data = &bytes[i + 6..i + 6 + 4096];
                for v in 0..32 {
                    voices.push(Voice::unpack(&data[v * 128..v * 128 + 128]));
                }
                i += 6 + 4096;
                continue;
            } else if fmt == 0x00 && i + 6 + VOICE_SIZE <= bytes.len() {
                let mut d = [0u8; VOICE_SIZE];
                d.copy_from_slice(&bytes[i + 6..i + 6 + VOICE_SIZE]);
                let mut v = Voice(d);
                v.clamp();
                voices.push(v);
                i += 6 + VOICE_SIZE;
                continue;
            }
        }
        i += 1;
    }
    if voices.is_empty() && bytes.len() == 4096 {
        for v in 0..32 {
            voices.push(Voice::unpack(&bytes[v * 128..v * 128 + 128]));
        }
    }
    if voices.is_empty() {
        Err("This file does not contain a valid six-operator bank or voice.".into())
    } else {
        Ok(voices)
    }
}

/// Write up to 32 voices as one bank (missing slots get the init voice).
pub fn write_bank(voices: &[Voice]) -> Vec<u8> {
    let mut data = Vec::with_capacity(4096);
    for i in 0..32 {
        let v = voices.get(i).cloned().unwrap_or_else(Voice::init);
        data.extend_from_slice(&v.pack());
    }
    let mut out = vec![0xF0, 0x43, 0x00, 0x09, 0x20, 0x00];
    let cs = checksum(&data);
    out.extend_from_slice(&data);
    out.push(cs);
    out.push(0xF7);
    out
}

/// Write a single voice message.
pub fn write_single(v: &Voice) -> Vec<u8> {
    let mut out = vec![0xF0, 0x43, 0x00, 0x00, 0x01, 0x1B];
    out.extend_from_slice(&v.0);
    out.push(checksum(&v.0));
    out.push(0xF7);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pack_roundtrip() {
        let mut v = Voice::init();
        v.set_op(2, OP_COARSE, 14);
        v.set_op(4, OP_DETUNE, 10);
        v.0[ALGORITHM] = 17;
        v.0[LFO_WAVE] = 4;
        let p = v.pack();
        assert_eq!(Voice::unpack(&p), v);
        let bank = write_bank(&[v.clone()]);
        assert_eq!(bank.len(), 4104);
        assert_eq!(read_syx(&bank).unwrap()[0], v);
    }
}
