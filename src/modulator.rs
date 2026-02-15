use core::f32::consts::TAU;
use micromath::F32Ext;
use crate::phasor::{Multiplier, PhasorBank};

#[derive(Clone, Copy)]
pub enum Waveshape {
    Sin,
    Tri,
    Squ,
    Saw,
}

impl Waveshape {
    //* Normalized values between 0.0 and 1.0 */
    pub fn compute_from_phasor(self, phase: f32) -> f32 {
        match self {
            Waveshape::Sin => ((phase * TAU).sin() + 1.0) * 0.5,
            Waveshape::Tri => 1.0 - ((phase - 0.5).abs() * 2.0),
            Waveshape::Squ => if phase > 0.5 { 1.0 } else { 0.0 },
            Waveshape::Saw => phase,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ModSlot {
    mul: Multiplier,
    wave: Waveshape,
}

impl ModSlot {
    pub fn new(mul: Multiplier, wave: Waveshape) -> Self {
        Self { mul, wave }
    }

    pub fn output(&self, phases: &[f32; Multiplier::ALL.len()]) -> f32 {
        self.wave.compute_from_phasor(phases[self.mul.index()])
    }
}


pub const NUM_MODULATORS: usize = 8;
const OUTPUT_BUFFER_SIZE: usize = 2 + NUM_MODULATORS * 4;

#[derive(Clone, Copy)]
pub struct ModulatorConfig {
    pub slots: [ModSlot; NUM_MODULATORS],
}

impl Default for ModulatorConfig {
    fn default() -> Self {
        Self {
            slots: [
                ModSlot::new(Multiplier::D4, Waveshape::Sin),
                ModSlot::new(Multiplier::D2, Waveshape::Tri),
                ModSlot::new(Multiplier::X1, Waveshape::Saw),
                ModSlot::new(Multiplier::X2, Waveshape::Squ),
                ModSlot::new(Multiplier::D4, Waveshape::Tri),
                ModSlot::new(Multiplier::D2, Waveshape::Sin),
                ModSlot::new(Multiplier::X1, Waveshape::Squ),
                ModSlot::new(Multiplier::X2, Waveshape::Saw),
            ]
        }
    }
}

pub struct ModulatorEngine;

impl ModulatorEngine {
    pub fn compute(&self, phasor: PhasorBank, config: &ModulatorConfig) -> [f32; NUM_MODULATORS] {
        let mut values = [0.0; NUM_MODULATORS];

        for (i, slot) in config.slots.iter().enumerate() {
            values[i] = slot.output(&phasor.phases)
        }

        values
    }

    pub fn compute_bytes(&self, phasor: PhasorBank, config: &ModulatorConfig) -> [u8; OUTPUT_BUFFER_SIZE] {
        let outputs = self.compute( phasor, config );
        let mut buffer = [0u8; OUTPUT_BUFFER_SIZE];

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