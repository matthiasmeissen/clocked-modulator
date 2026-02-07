
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


const NUM_SLOTS: usize = 4;

pub struct Modulator {
    pub bank: PhasorBank,
    pub slots: [ModSlot; NUM_SLOTS],
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

    pub fn get_all_outputs(&self) -> [f32; NUM_SLOTS] {
        let mut outputs = [0.0; NUM_SLOTS];
        for (i, slot) in self.slots.iter().enumerate() {
            outputs[i] = slot.output(&self.bank);
        }
        outputs
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
