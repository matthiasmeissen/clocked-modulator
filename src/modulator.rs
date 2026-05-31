use crate::phasor::{Multiplier, PhasorBank};

#[derive(Clone, Copy, PartialEq)]
pub enum Waveshape {
    Sin,
    Tri,
    Squ,
    Saw,
    Con,
    Noi,
}

impl Waveshape {
    pub const ALL: [Waveshape; 6] = [
        Waveshape::Sin,
        Waveshape::Tri,
        Waveshape::Squ,
        Waveshape::Saw,
        Waveshape::Con,
        Waveshape::Noi,
    ];

    // Those could be solved more elegantly
    // but this approach is readable and fast
    pub fn next(self) -> Self {
        match self {
            Waveshape::Sin => Waveshape::Tri,
            Waveshape::Tri => Waveshape::Squ,
            Waveshape::Squ => Waveshape::Saw,
            Waveshape::Saw => Waveshape::Con,
            Waveshape::Con => Waveshape::Noi,
            Waveshape::Noi => Waveshape::Sin,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Waveshape::Sin => Waveshape::Noi,
            Waveshape::Tri => Waveshape::Sin,
            Waveshape::Squ => Waveshape::Tri,
            Waveshape::Saw => Waveshape::Squ,
            Waveshape::Con => Waveshape::Saw,
            Waveshape::Noi => Waveshape::Con,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Waveshape::Sin => "SIN",
            Waveshape::Tri => "TRI",
            Waveshape::Squ => "SQU",
            Waveshape::Saw => "SAW",
            Waveshape::Con => "CON",
            Waveshape::Noi => "NOI",
        }
    }

    //* Normalized values between 0.0 and 1.0 */
    // The five deterministic shapes depend only on `phase`; `Noi` also needs the
    // slot's per-slot noise params, so they ride along (ignored by the others).
    pub fn compute_from_phasor(self, phase: f32, noise_seed: u8, noise_freq: u8) -> f32 {
        match self {
            Waveshape::Sin => {
                let idx = phase * SIN_LUT.len() as f32;
                let idx0 = idx as usize % SIN_LUT.len();
                let idx1 = (idx0 + 1) % SIN_LUT.len();
                let frac = idx - (idx as usize) as f32;
                SIN_LUT[idx0] * (1.0 - frac) + SIN_LUT[idx1] * frac
            }
            Waveshape::Tri => 1.0 - ((phase - 0.5).abs() * 2.0),
            Waveshape::Squ => if phase > 0.5 { 1.0 } else { 0.0 },
            Waveshape::Saw => phase,
            Waveshape::Con => 1.0,
            Waveshape::Noi => perlin_cycle(phase, noise_seed, noise_freq),
        }
    }
}

// NOI starting values for a fresh slot; edited per-slot on the ModEditNoise page.
const DEFAULT_NOISE_SEED: u8 = 0;
const DEFAULT_NOISE_FREQ: u8 = 3;

#[derive(Clone, Copy, PartialEq)]
pub struct ModSlot {
    pub mul: Multiplier,
    pub wave: Waveshape,
    pub min: f32,
    pub max: f32,
    pub smooth: bool,
    pub noise_seed: u8, // NOI: selects the random shape
    pub noise_freq: u8, // NOI: humps per cycle (complexity); clamped to >= 1
}

impl ModSlot {
    pub fn new(mul: Multiplier, wave: Waveshape, min: f32, max: f32, smooth: bool) -> Self {
        Self {
            mul,
            wave,
            min,
            max,
            smooth,
            noise_seed: DEFAULT_NOISE_SEED,
            noise_freq: DEFAULT_NOISE_FREQ,
        }
    }

    pub fn output(&self, phases: &[f32; Multiplier::ALL.len()]) -> f32 {
        let phase = phases[self.mul.index()];
        let raw_value = self.wave.compute_from_phasor(phase, self.noise_seed, self.noise_freq);
        let mapped_value = self.min + raw_value * (self.max - self.min);
        mapped_value
    }
}

impl Default for ModSlot {
    fn default() -> Self {
        Self {
            mul: Multiplier::X1,
            wave: Waveshape::Saw,
            min: 0.0,
            max: 1.0,
            smooth: false,
            noise_seed: DEFAULT_NOISE_SEED,
            noise_freq: DEFAULT_NOISE_FREQ,
        }
    }
}


pub const NUM_MODULATORS: usize = 4;
pub const MIDI_PACKET_SIZE: usize = 4;
pub const MIDI_PACKETS_PER_FRAME: usize = NUM_MODULATORS * 2;
pub const MIDI_FRAME_SIZE: usize = MIDI_PACKETS_PER_FRAME * MIDI_PACKET_SIZE;

#[derive(Clone, Copy, PartialEq)]
pub struct ModulatorConfig {
    pub slots: [ModSlot; NUM_MODULATORS],
}

impl Default for ModulatorConfig {
    fn default() -> Self {
        Self {
            slots: [
                ModSlot::new(Multiplier::X1, Waveshape::Saw, 0.0, 1.0, false),
                ModSlot::new(Multiplier::X1, Waveshape::Con, 0.0, 1.0, false),
                ModSlot::new(Multiplier::X1, Waveshape::Con, 0.0, 1.0, false),
                ModSlot::new(Multiplier::X1, Waveshape::Con, 0.0, 1.0, false),
            ]
        }
    }
}

pub struct ModulatorFrame {
    pub midi_bytes: [u8; MIDI_FRAME_SIZE],
    pub outputs: [f32; NUM_MODULATORS],
}

pub struct ModulatorEngine;

impl ModulatorEngine {
    pub fn compute(&self, phasor: &PhasorBank, config: &ModulatorConfig) -> [f32; NUM_MODULATORS] {
        let mut values = [0.0; NUM_MODULATORS];

        for (i, slot) in config.slots.iter().enumerate() {
            values[i] = slot.output(&phasor.phases)
        }

        values
    }

    /// Pack pre-computed outputs into 8 USB MIDI packets (14-bit CC: MSB + LSB per slot).
    /// Each output (0.0–1.0) maps to 0–16383, split across CC N (MSB) and CC N+32 (LSB).
    pub fn pack_midi_bytes(&self, outputs: &[f32; NUM_MODULATORS]) -> ModulatorFrame {
        let mut midi_bytes = [0u8; MIDI_FRAME_SIZE];

        for (i, &output) in outputs.iter().enumerate() {
            let value_14bit = (output.clamp(0.0, 1.0) * 16383.0) as u16;
            let msb = (value_14bit >> 7) as u8;
            let lsb = (value_14bit & 0x7F) as u8;

            let base = i * 8;

            // MSB packet: CC i on channel 1
            midi_bytes[base]     = 0x0B; // cable 0, CIN = Control Change
            midi_bytes[base + 1] = 0xB0; // CC, channel 1
            midi_bytes[base + 2] = i as u8;
            midi_bytes[base + 3] = msb;

            // LSB packet: CC i+32 on channel 1
            midi_bytes[base + 4] = 0x0B;
            midi_bytes[base + 5] = 0xB0;
            midi_bytes[base + 6] = (i + 32) as u8;
            midi_bytes[base + 7] = lsb;
        }

        ModulatorFrame { midi_bytes, outputs: *outputs }
    }
}


pub struct Visualizer4(pub [f32; NUM_MODULATORS]);

impl defmt::Format for Visualizer4 {
    fn format(&self, f: defmt::Formatter) {
        for (i, val) in self.0.iter().enumerate() {
            let clamped = if *val < 0.0 { 0.0 } else if *val > 1.0 { 1.0 } else { *val };
            let level = (clamped * 8.0) as usize;
            match level {
                0 => defmt::write!(f, "{}: ", i),
                1 => defmt::write!(f, "{}:▂", i),
                2 => defmt::write!(f, "{}:▃", i),
                3 => defmt::write!(f, "{}:▄", i),
                4 => defmt::write!(f, "{}:▅", i),
                5 => defmt::write!(f, "{}:▆", i),
                6 => defmt::write!(f, "{}:▇", i),
                _ => defmt::write!(f, "{}:█", i),
            }

            if i < NUM_MODULATORS - 1 {
                defmt::write!(f, " | ");
            }
        }
    }
}

// 1D Perlin noise
//
// NOI is a pure function of the phasor phase, so it repeats each cycle like the
// other waveshapes. Gradient noise passes through zero at every integer lattice
// point, so laying `freq` lattice points across the cycle and wrapping the
// lattice index `mod freq` makes the value AND slope match at the phase 1->0
// boundary — seamless, with no special casing.

// 1D gradient noise is bounded to [-0.5, 0.5], so a gain of 1.0 maps it exactly
// onto [0, 1] without ever clipping; the `clamp` is then only a float-rounding guard.
const NOISE_NORM: f32 = 1.0;

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

/// Scalar gradient in [-1.0, ~1.0] for a lattice point, picked from the table.
fn grad(i: usize) -> f32 {
    PERM[i & 255] as f32 / 127.5 - 1.0
}

/// 1D Perlin over one cycle: `freq` humps, `seed` selects the shape.
fn perlin_cycle(phase: f32, seed: u8, freq: u8) -> f32 {
    let n = freq.max(1) as usize;
    let x = phase * n as f32; // phase in [0.0, 1.0) -> x in [0.0, n)
    let i = x as usize; // floor, since x >= 0
    let f = x - i as f32;

    let s = seed as usize;
    let g0 = grad(s + (i % n));
    let g1 = grad(s + ((i + 1) % n)); // lattice wraps mod n -> seamless

    let raw = lerp(g0 * f, g1 * (f - 1.0), fade(f));
    (raw * NOISE_NORM + 0.5).clamp(0.0, 1.0)
}

// Wavetables

/// Ken Perlin's classic permutation of 0..=255, used to derive noise gradients.
static PERM: [u8; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69,
    142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219,
    203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122, 60, 211, 133, 230,
    220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1, 216, 80, 73, 209, 76,
    132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173,
    186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212, 207, 206,
    59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213, 119, 248, 152, 2, 44, 154, 163,
    70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232,
    178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162,
    241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157, 184, 84, 204,
    176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141,
    128, 195, 78, 66, 215, 61, 156, 180,
];

static SIN_LUT: [f32; 256] = [
    0.50000000, 0.51227061, 0.52453384, 0.53678228, 0.54900857, 0.56120534, 0.57336524, 0.58548094,
    0.59754516, 0.60955062, 0.62149009, 0.63335638, 0.64514234, 0.65684087, 0.66844493, 0.67994752,
    0.69134172, 0.70262066, 0.71377755, 0.72480566, 0.73569837, 0.74644910, 0.75705137, 0.76749881,
    0.77778512, 0.78790410, 0.79784965, 0.80761580, 0.81719664, 0.82658642, 0.83577948, 0.84477027,
    0.85355339, 0.86212354, 0.87047556, 0.87860442, 0.88650523, 0.89417321, 0.90160377, 0.90879241,
    0.91573481, 0.92242678, 0.92886431, 0.93504350, 0.94096063, 0.94661215, 0.95199465, 0.95710488,
    0.96193977, 0.96649640, 0.97077203, 0.97476409, 0.97847017, 0.98188803, 0.98501563, 0.98785107,
    0.99039264, 0.99263882, 0.99458825, 0.99623977, 0.99759236, 0.99864523, 0.99939773, 0.99984941,
    1.00000000, 0.99984941, 0.99939773, 0.99864523, 0.99759236, 0.99623977, 0.99458825, 0.99263882,
    0.99039264, 0.98785107, 0.98501563, 0.98188803, 0.97847017, 0.97476409, 0.97077203, 0.96649640,
    0.96193977, 0.95710488, 0.95199465, 0.94661215, 0.94096063, 0.93504350, 0.92886431, 0.92242678,
    0.91573481, 0.90879241, 0.90160377, 0.89417321, 0.88650523, 0.87860442, 0.87047556, 0.86212354,
    0.85355339, 0.84477027, 0.83577948, 0.82658642, 0.81719664, 0.80761580, 0.79784965, 0.78790410,
    0.77778512, 0.76749881, 0.75705137, 0.74644910, 0.73569837, 0.72480566, 0.71377755, 0.70262066,
    0.69134172, 0.67994752, 0.66844493, 0.65684087, 0.64514234, 0.63335638, 0.62149009, 0.60955062,
    0.59754516, 0.58548094, 0.57336524, 0.56120534, 0.54900857, 0.53678228, 0.52453384, 0.51227061,
    0.50000000, 0.48772939, 0.47546616, 0.46321772, 0.45099143, 0.43879466, 0.42663476, 0.41451906,
    0.40245484, 0.39044938, 0.37850991, 0.36664362, 0.35485766, 0.34315913, 0.33155507, 0.32005248,
    0.30865828, 0.29737934, 0.28622245, 0.27519434, 0.26430163, 0.25355090, 0.24294863, 0.23250119,
    0.22221488, 0.21209590, 0.20215035, 0.19238420, 0.18280336, 0.17341358, 0.16422052, 0.15522973,
    0.14644661, 0.13787646, 0.12952444, 0.12139558, 0.11349477, 0.10582679, 0.09839623, 0.09120759,
    0.08426519, 0.07757322, 0.07113569, 0.06495650, 0.05903937, 0.05338785, 0.04800535, 0.04289512,
    0.03806023, 0.03350360, 0.02922797, 0.02523591, 0.02152983, 0.01811197, 0.01498437, 0.01214893,
    0.00960736, 0.00736118, 0.00541175, 0.00376023, 0.00240764, 0.00135477, 0.00060227, 0.00015059,
    0.00000000, 0.00015059, 0.00060227, 0.00135477, 0.00240764, 0.00376023, 0.00541175, 0.00736118,
    0.00960736, 0.01214893, 0.01498437, 0.01811197, 0.02152983, 0.02523591, 0.02922797, 0.03350360,
    0.03806023, 0.04289512, 0.04800535, 0.05338785, 0.05903937, 0.06495650, 0.07113569, 0.07757322,
    0.08426519, 0.09120759, 0.09839623, 0.10582679, 0.11349477, 0.12139558, 0.12952444, 0.13787646,
    0.14644661, 0.15522973, 0.16422052, 0.17341358, 0.18280336, 0.19238420, 0.20215035, 0.21209590,
    0.22221488, 0.23250119, 0.24294863, 0.25355090, 0.26430163, 0.27519434, 0.28622245, 0.29737934,
    0.30865828, 0.32005248, 0.33155507, 0.34315913, 0.35485766, 0.36664362, 0.37850991, 0.39044938,
    0.40245484, 0.41451906, 0.42663476, 0.43879466, 0.45099143, 0.46321772, 0.47546616, 0.48772939,
];
