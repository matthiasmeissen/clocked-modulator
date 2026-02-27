use crate::phasor::{Multiplier, PhasorBank};

#[derive(Clone, Copy, PartialEq)]
pub enum Waveshape {
    Sin,
    Tri,
    Squ,
    Saw,
}

impl Waveshape {
    pub const ALL: [Waveshape; 4] = [
        Waveshape::Sin,
        Waveshape::Tri,
        Waveshape::Squ,
        Waveshape::Saw,
    ];

    // Those could be solved more elegantly 
    // but this approach is readable and fast
    pub fn next(self) -> Self {
        match self {
            Waveshape::Sin => Waveshape::Tri,
            Waveshape::Tri => Waveshape::Squ,
            Waveshape::Squ => Waveshape::Saw,
            Waveshape::Saw => Waveshape::Sin,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Waveshape::Sin => Waveshape::Saw,
            Waveshape::Tri => Waveshape::Sin,
            Waveshape::Squ => Waveshape::Tri,
            Waveshape::Saw => Waveshape::Squ,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Waveshape::Sin => "SIN",
            Waveshape::Tri => "TRI",
            Waveshape::Squ => "SQU",
            Waveshape::Saw => "SAW",
        }
    }

    //* Normalized values between 0.0 and 1.0 */
    pub fn compute_from_phasor(self, phase: f32) -> f32 {
        match self {
            Waveshape::Sin => SIN_LUT[(phase * 255.0) as usize],
            Waveshape::Tri => 1.0 - ((phase - 0.5).abs() * 2.0),
            Waveshape::Squ => if phase > 0.5 { 1.0 } else { 0.0 },
            Waveshape::Saw => phase,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct ModSlot {
    pub mul: Multiplier,
    pub wave: Waveshape,
    pub min: f32,
    pub max: f32,
}

impl ModSlot {
    pub fn new(mul: Multiplier, wave: Waveshape, min: f32, max: f32) -> Self {
        Self { mul, wave, min, max }
    }

    pub fn output(&self, phases: &[f32; Multiplier::ALL.len()]) -> f32 {
        let raw_value = self.wave.compute_from_phasor(phases[self.mul.index()]);
        let mapped_value = self.min + raw_value * (self.max - self.min);
        mapped_value
    }
}

impl Default for ModSlot {
    fn default() -> Self {
        Self { mul: Multiplier::X1, wave: Waveshape::Saw, min: 0.0, max: 1.0 }
    }
}


pub const NUM_MODULATORS: usize = 4;
pub const PACKET_SIZE: usize = 2 + NUM_MODULATORS * 4;

#[derive(Clone, Copy)]
pub struct ModulatorConfig {
    pub slots: [ModSlot; NUM_MODULATORS],
}

impl Default for ModulatorConfig {
    fn default() -> Self {
        Self {
            slots: [
                ModSlot::new(Multiplier::X1, Waveshape::Saw, 0.0, 1.0),
                ModSlot::new(Multiplier::X1, Waveshape::Saw, 0.2, 0.8),
                ModSlot::new(Multiplier::D2, Waveshape::Sin, 0.0, 1.0),
                ModSlot::new(Multiplier::D4, Waveshape::Squ, 0.0, 1.0),
            ]
        }
    }
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

    pub fn compute_bytes(&self, phasor: &PhasorBank, config: &ModulatorConfig) -> [u8; PACKET_SIZE] {
        let outputs = self.compute(phasor, config);
        let mut buffer = [0u8; PACKET_SIZE];

        // Sync header (must match TouchDesigner parser)
        buffer[0] = 0xAA;
        buffer[1] = 0xBB;

        // Pack f32 outputs as little-endian bytes
        for (i, &output) in outputs.iter().enumerate() {
            let bytes = output.to_le_bytes();
            buffer[2 + i * 4..6 + i * 4].copy_from_slice(&bytes);
        }

        buffer
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

// Wavetables

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
