
use core::f32::consts::TAU;
use libm::sinf;
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
            Waveshape::Sin => (sinf(phase * TAU) + 1.0) * 0.5,
            Waveshape::Tri => 1.0 - ((phase - 0.5).abs() * 2.0),
            Waveshape::Squ => if phase > 0.5 { 1.0 } else { 0.0 },
            Waveshape::Saw => phase,
        }
    }
}


pub struct ModSlot {
    mul: Multiplier,
    wave: Waveshape,
}

impl ModSlot {
    pub fn new(mul: Multiplier, wave: Waveshape) -> Self {
        Self { mul, wave }
    }

    pub fn output(&self, bank: &PhasorBank) -> f32 {
        let phase_value = bank.get_phase(self.mul);
        self.wave.compute_from_phasor(phase_value)
    }
}


pub const NUM_MODULATORS: usize = 4;

pub struct Modulator {
    pub bank: PhasorBank,
    pub slots: [ModSlot; NUM_MODULATORS],
}

impl Modulator {
    pub fn new(bpm: f32, tick_rate: f32) -> Self {
        Self { 
            bank: PhasorBank::new(bpm, tick_rate), 
            slots: [
                ModSlot::new(Multiplier::D4, Waveshape::Sin),
                ModSlot::new(Multiplier::D2, Waveshape::Tri),
                ModSlot::new(Multiplier::X1, Waveshape::Saw),
                ModSlot::new(Multiplier::X2, Waveshape::Squ),
            ]
        }   
    }

    pub fn tick(&mut self) {
        self.bank.tick();
    }

    pub fn get_all_outputs(&self) -> [f32; NUM_MODULATORS] {
        let mut outputs = [0.0; NUM_MODULATORS];
        for (i, slot) in self.slots.iter().enumerate() {
            outputs[i] = slot.output(&self.bank);
        }
        outputs
    }

    pub fn get_output_as_bytes(&self) -> [u8; 18] {
        let outputs = self.get_all_outputs();
        let mut buffer = [0u8; 18];

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


pub struct Visualizer4(pub [f32; 4]);

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

            if i < 3 {
                defmt::write!(f, " | ");
            }
        }
    }
}
