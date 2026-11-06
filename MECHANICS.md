# Mechanics sheet — Hexa FM (rebuild of AV Dexed 0.2.1)

All values were read from the original program `AV Dexed.app/Contents/MacOS/AV Dexed`
(arm64 slice) unless marked **estimated**. Code in this folder is written from this
sheet, not copied.

## Sound engine (6-operator FM, DX7 style)

| Value | Number | Source |
|---|---|---|
| Operators per voice | 6 | `FmCore::algorithms` table 0x1004b4c30 (6 words per algorithm) |
| Algorithms | 32 | same table, 32 × 6 flag words, ends at 0x1004b4f30 |
| Algorithm flag bits | out bus 1 = 0x01, out bus 2 = 0x02, add = 0x04, in bus 1 = 0x10, in bus 2 = 0x20, feedback in = 0x40, feedback out = 0x80 | decoded from the table values (0xc1, 0x11, 0x14, 0x25, 0x94, 0xc5 …) |
| Self feedback only when both feedback bits set | flags & 0xc0 == 0xc0 | table: cross-feedback ops (0x94) carry only the out bit |
| Feedback shift | 8 − feedback; 16 (off) when feedback = 0; input = (y₀+y₁) >> (shift+1) | `Dx7Note::update` 0x100008bac (`8 - fb`, `csel … 16`) |
| Detune | logf += 0.0209·e^(−0.396·logf)/7 · logf · (detune − 7) | `Dx7Note::osc_freq` 0x100008824 (0x3F9566CF41F212D7, 0xBFD95810624DD2F2) |
| Fixed frequency | logf = ((coarse & 3)·100 + fine) · 557327, + 13457·(detune − 7) when detune > 7 | `Dx7Note::osc_freq` (0x8810f, 0x3491) |
| Fine ratio | logf += log2(1 + 0.01·fine)·2²⁴ | `Dx7Note::osc_freq` (0x3F847AE147AE147B) |
| Envelope block | 64 samples per envelope step | `Env::getsample` 0x1000093f8 (`static -= 0x40`) |
| Envelope rate | qrate = min(63, rate·41/64 + rate scaling) | `Env::getsample` (`*0x29`, `csel …0x3f`) |
| Envelope increment | (4 + (qrate & 3)) << (8 + qrate/4), × sample-rate multiplier >> 24 | `Env::getsample` (`bfxil w11,#0,#2`, `asr #2`, `+8`) |
| Envelope level | target = (max(4272, (scaledLevel/2)·64 + outlevel) − 4256) << 16 | `Env::getsample` (0x10b0, 0xef600000) |
| Attack jump-start | 1716 << 16 (0x6b40000) | `Env::getsample` |
| Attack curve | level += ((17<<24) − level) >> 24 × inc | `Env::getsample` (0x11000000) |
| Level table below 20 | 0,5,9,13,17,20,23,25,27,29,31,33,35,37,39,41,42,43,45,46 | `levellut` 0x1004b4aa0 |
| Level above 20 | level + 28 | `Env::getsample` (`w10 + 0x1c`) |
| Velocity table | 64 bytes 0,70,86,97,106,114,121,126,… ,254 | `velocity_data` 0x1004b4a24 |
| Exponential scale table | 0,1,2,3,4,5,6,7,8,9,11,14,16,19,23,27,33,39,47,56,66,80,94,110,126,142,158,174,190,206,222,238,250 | 0x1004b4a64 |
| Coarse multipliers | coarse 0 = ½, else coarse × (log table) | `coarsemul` 0x1004b49a4 |
| Amp-mod sensitivity | 0, 4342338, 7171437, 16777216 (Q24) | `ampmodsenstab` 0x1004b4a88 |
| Pitch-mod sensitivity | 0,10,20,33,55,92,153,255 | `pitchmodsenstab` 0x1004b4a98 |
| LFO waves | Triangle, Saw down, Saw up, Square, Sine, Sample & hold | strings "TRIANGLE … S&HOLD" |
| LFO speed | 100-entry Hz table: 0.062541 (speed 0) … 5.434783 (34) … 10.117361 (63) … 49.261084 (99); phase step = Hz·2³²/sr | table `lfoSource` 0x1004b4f40, read in `processBlock` 0x100018000; 2³⁸/sr per block from `prepareToPlay` 0x100016ce0 |
| LFO waves (0..1) | tri = phase>>7 folded; saw down = (~p ^ 2³¹)>>8; saw up = (p ^ 2³¹)>>8; square = (~p>>7) & 2²⁴; sine = ½ + ½·sin; S&H rand = (rand·179 + 17) & 255 on each wrap | LFO code inlined in `processBlock` |
| LFO key sync | phase = 2³¹ − 1 on note-on when sync is on; the delay counter always restarts | same |
| LFO delay | a = 99 − delay; inc = (16 + (a & 15)) << (1 + a>>4), after half-way max(0x80, inc & 0xff80); unit = 64·25190424/sr per block; 0 for the first half, then (state>>7) & (2²⁴−1) | `processBlock` (`#0x63`, `#0x10`, `#0xff80`, `#0x80`) |
| LFO depths | pitch = (PMD·165)>>6, amp = (AMD·165)>>6 | `Dx7Note::update` 0x100008bac (`*0xa5`, `lsr #6`) |
| Pitch modulation | octaves = depth·delay·PMS·(lfo − ½)/32768 (from `>> 39`); mod wheel = wheel·127·PMS·(lfo − ½)/16384; larger magnitude wins | `processBlock` 0x100018000 |
| Amp modulation | amod = depth·delay·(1 − lfo)·2¹⁶ (wheel ·127·2¹⁷); sens = amod·AMS-table/2²⁴; envelope level −= level·e^(sens/262144·0.07 + 12.2)/2²⁴, only when AMS ≠ 0 | `processBlock` (`ucvtf #0x12`, 0x3FB1EB851EB851EC = 0.07, 0x4028666666666666 = 12.2, `_exp`) |
| Pitch envelope levels | level = table[L] << 19 in Q24 octaves: −128 … 0 (L = 50) … 127, ±4 octaves | table 0x1004b52cd, `processBlock` (`ldrsb`, `lsl #19`) |
| Pitch envelope rates | step per block = rateTable[R]·64·2²⁴/(21.3·sr); table 1, 2, 3, 3, 4 … 243, 254, 255 | table 0x1004b5261; 21.3 = 0x40354CCCCCCCCCCD in `prepareToPlay` |
| Voice output | per voice (sum >> 4 >> 9)·2⁻¹⁵ = sum/16, clipped to ±1, then all voices added | `processBlock` (`sshr #4`, `sshr #13`, `movi #0x38, lsl #24`) |
| Polyphony | 16 voices | `DexedAudioProcessor::keyup` 0x10001da84 (voice loop unrolled 16×) |

## Play screen controls

| Control | Range / default | Source |
|---|---|---|
| Volume, Cutoff, Pan, Reverb, Chorus, Delay | six macro knobs | strings `knobVolume`, `knobBrightness=Cutoff`, `knobPan`, `knobReverb`, `knobChorus`, `knobDelay`; six 96×96 knobs in `AVDexedPlayView::resized` 0x1000b60a0 |
| Resonance | 0–1, default 0 | parameter string `resonance` |
| Preset list column | 320 px wide list, search above, folder picker, Manage button | `AVDexedPlayView::resized` 0x1000b60a0 |
| Preset row keys | ↓ (0xF701) = next, ↑ (0xF700) = previous, only with no shift/ctrl/alt; held keys repeat; wraps round: next = (active + n ± 1) mod n; then select, scroll into view, load | `AVDexedPlayView::keyPressed` 0x1000ba8e4, `navigatePreset` 0x1000ba670 |
| Preset row height | 34 px (`setRowHeight(0x22)`), viewport single step 20 px | `AVDexedPlayView` constructor 0x1000a7424 (at 0x1000a87ec) |
| List follows the active sound | remembers the last followed key ("preset\|id" / "bank\|n"); only when it changes and the row is in the list does it call `ListBox::scrollToEnsureRowIsOnscreen` — scroll just enough, never centre | `AVDexedPlayView::followActiveRow` 0x1000b2388 |

## Filter (original's resonant low-pass)

| Value | Number | Source |
|---|---|---|
| Cutoff curve | Hz = (e^(c·2.99573) − 1)/19 · 18940 + 60 (60 Hz … 19 kHz) | `processBlock` 0x100018000 (0x403fba14, 19.0, 0x4693f800, 0x42700000) |
| Coefficient | g = tan(π·Hz/sr) | same (`_tanf`, π = 0x40490fdb) |
| Resonance | rReso = 0.991 − 0.991·(e^((1−r)·2.99573) − 1)/19; feedback R24 = 3.5·rReso | same (0x3f7db22d, 0x3FEFB645A1CAC083, 3.5) |
| DC blocker | input − 0.45 × one-pole at 15 Hz | same (±0.45 = 0x3FDCCCCCCCCCCCCD, 15.0) |
| Damping | first-pole state = atan(s·rcor24)/rcor24, rcor24 = 0.022045·sqrt(44000/sr) | `processBlock` (`ldr [x19,#0x52c]`, `_atanf`, `ldr [x19,#0x530]`); value set in PluginFx init inlined in `prepareToPlay` 0x100016ce0 (0x3F96931569315693, 44000 = 0x472be000, `fsqrt`) |
| Input chain | DC blocker coefficient 15·π/sr (no tan), then a one-pole at tan((sr/2 − 10)·π/sr) before the ladder | `processBlock` (15.0, π) and `prepareToPlay` (−10.0, 0.5, `_tanf`) |
| Smoothing | cutoff and resonance glide with k = e^(−1/(0.01·sr)) per sample (10 ms) | `prepareToPlay` (0x3c23d70a = 0.01, `_expf`), applied in `processBlock` (`fmla`) |
| Compensation | output × (1 + 0.45·R24) | same |
| Bypass | while cutoff = 1.0 | same (`fcmp s9, 1.0`) |

## Effects

| Value | Number | Source |
|---|---|---|
| Delay damping | default 0.3, 0–1 | `AVDexedDelayFaust::buildUserInterface` 0x10003a024 (0x3e99999a) |
| Delay feedback | default 0.35, 0–0.9 | same (0x3eb33333, 0x3f666666) |
| Delay time L/R | 20–1500 ms, default 500 ms, step 0.1 | same (20.0, 0x44bb8000, 0x43fa0000, 0x3dcccccd) |
| Delay divisions | 1/16, 1/8D, 1/8T, 1/8, 1/4D, 1/4T, 1/4, 1/2 | strings near "Delay Left Division" (1/8, 1/4, 1/2 **estimated**) |
| Chorus modes | I, II, I + II | strings "I + II", "Chorus Mode" |
| Chorus params | Amount, Rate, Depth, Mode, Stereo spread | strings `chorusAm/Mo/Ra/De/Spread` |
| Reverb params | Amount, Size/decay, Damping, Pre-delay | strings "Reverb Amount/Size/Pre-delay", `damping` |
| Effect order | voices → filter → chorus → delay → reverb (the original uses JUCE's built-in reverb) | call order in `processBlock` 0x100018000 (`_tanf`, `_atanf`, `_cos`, `AVDexedDelay::process`, `juce::Reverb::setParameters`) |

## Effect parameters (createParameterLayout 0x1001805d4)

| Parameter | Range / default | Source |
|---|---|---|
| Reverb Amount | 0–1 step 0.001, default 0 | `createParameterLayout` (range const 0x100707330, `movi v0, #0`) |
| Reverb Size | 0–1, default 0.21 | same (0x3e570a3d) |
| Reverb Damping | 0–1, default 0.5 | same (`fmov s0, #0.5` at 0x100184a28) |
| Reverb Pre-delay | 0–250 ms step 0.1, default 45 ms | same (range const 0x100707360, 0x42340000) |
| Reverb mix | JUCE reverb with dry 0, width 1 (wet only, added on top of the sound); wet gain 3·amount; room = size·0.28 + 0.7; damp = damping·0.4; input gain 0.015 | `processBlock` before `juce::Reverb::setParameters` (const 0x100704bc8 = [0.0, 1.0]); JUCE reverb rules |
| Delay Amount | 0–1, default 0 | `createParameterLayout` |
| Delay Feedback | 0–0.9, default 0.35 | same (range const 0x100707380, 0x3eb33333) |
| Delay Damping | 0–1, default 0.3 | same (0x3e99999a) |
| Delay times | 20–1500 ms step 0.1, default 500 | same (range const 0x100707370, 0x43fa0000) |
| Delay Local Tempo | 20–999 BPM, default 120 | same (range const 0x1007073b0, 0x42f00000) |
| Chorus Amount | 0–1, default 0 | same |
| Chorus Mode | choice I / II / I + II, default I | same (`AudioParameterChoice`) |
| Chorus Rate | 0.05–5 Hz step 0.001, default 0.5 Hz | same (range const 0x100707390) |
| Chorus Depth | 0–5 (ms) step 0.01, default 2 | same (range const 0x1007073a0, `fmov s0, #2.0`) |
| Chorus Spread | 0–1, default 1 | same (`fmov s0, #1.0`) |
| Chorus delay | base 7 ms + depth·LFO (triangle), Catmull-Rom (cubic) read; chorus II LFO = rate × 1.65; right side offset (0.25 + 0.25·spread) cycle | `processBlock` (7.0, 0x3fd33333 = 1.65, `fmadd spread·0.25 + 0.25`, −2.5/2.0/−0.5/1.5 coefficients, 1000/sr) |
| Chorus wet/dry balance | dry·(1 − 0.5·amount) + wet·0.7·amount | **estimated** |
| Output gain range | 0–15.85 (+24 dB), skew 0.25 | range const 0x100707350 |

## Plugin behaviour (host-facing rules)

| Rule | Value | Source |
|---|---|---|
| MIDI input | accepted (notes, CCs, pitch bend, program change) | `DexedAudioProcessor::acceptsMidi` 0x10001e37c (returns 1) |
| Output buses | stereo or mono | `DexedAudioProcessor::isBusesLayoutSupported` 0x10001e128 (compares with mono and `AudioChannelSet::stereo`) |
| Tail after the last note | 50 s while the delay is in use, 18 s for reverb, 0 otherwise; "in use" = amount > 1e-5 (0x3727c5ac) | `DexedAudioProcessor::getTailLengthSeconds` 0x10001e38c (0x4049000000000000 = 50.0, `fmov #18.0`) |
| Project state | schemaVersion, tuning, currentProgram, sourcePath, avPresetName, avPresetId, avKnownPresetOrigin, avSoundFromBankSlot, avPresetPath, avPresetBaseline (hex), voice, cartridge | `DexedAudioProcessor::getStateInformation` 0x100199464 (referenced strings) |
| Rebuild's project state | schema 1, preset id, voice, play settings, name; reopened projects select the library sound by id, so edits still show as EDITED | `plugin/src/lib.rs` from the rule above |
| Editor size | 1440 × 940 with keyboard (0x5a0 × 0x3ac), 1440 × 828 without (0x33c) | `DexedAudioProcessorEditor::resetSize` 0x1000662e0 |
| Interface scale | clamped to 0.8 … 1.6 (0x3f4ccccd, 0x3fcccccd); resize limits = base size × 0.8 … × 1.6 | same |

## Fonts and resizing

The original draws text with `AVDexedTheme::font` 0x10005d7e0: Montserrat at the caller's fixed point
size with −0.04 extra kerning (0xbd23d70a), and resizes by scaling the whole editor uniformly
("Drag to resize the interface while preserving its proportions"). The rebuild adds what the person asked
for: text grows up to 15% with the window width (1 + 0.25·(width/1360 − 1), clamped 1.0–1.15) and a
Text size setting (80–140%), applied as one factor so each section keeps its relative sizes.

## Original colours (AVDexedTheme, constant table 0x100704680)

canvas #000000 · panelTop #10141C · panelBottom #070809 · textPrimary #E0E0E0 · textSecondary #9A9A9A ·
oceanTop #248FC7 · oceanBottom #0B3558 · oceanHighlight #58BFE8 · dangerTop #B10004 · dangerBottom #780002 ·
controlTop #17212A · controlBottom #080B0E · outline #35434D · track #26313A · banner #0B0D0F ·
silverTop #666666 · silverBottom #424242 · knobTop #121212 · knobBottom #242424 · dotTop #999999 · dotBottom #D2D2D2.

The original paints glossy gradients (`setGradientFill` in `AVDexedPlayView::paint` 0x1000b585c) with
the blue "ocean" accent and silver knobs. As asked, the rebuild replaces this with a flat graphite
scheme (accent amber) and offers two more themes.

## Preset library (original behaviour → rebuild)

Original (strings + `AVDexedPlayView` methods): All Presets, Favorites, User Presets, folders
(New/Rename/Delete Folder, Add/Copy to folder), rename/delete sound, search by name or origin,
import .syx banks and .avdexpreset sounds, restore factory sounds, Save / Save As, unsaved-changes prompt,
MIDI-learn for next/previous preset.

Rebuild keeps all of that and adds: categories, tags, star ratings, notes, sort options, recently
played, A/B compare, revert, duplicate, a trash with restore, bank export to .syx.

## Sound names (rebuild)

Factory and imported bank names are shown tidied (trailing slashes/dashes removed, ALL-CAPS softened,
e.g. `SloSwl //\` → `SloSwl`, `-ANALOG 1-` → `Analog 1`, `E.PIANO 1` → `E.Piano 1`). The name stored in the
bank is kept and shown as "Bank name" on the Play page. Sounds whose names say nothing about their type
are categorised from their carriers: slow attack + high sustain → Pad, fast attack + low sustain → Plucked.

## Licence

AV Dexed is an open-source fork of DEXED 1.1.0 (GPL v3 or later). This rebuild is released under the
same licence and credits both projects; the factory banks are the ones they ship.
