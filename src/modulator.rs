
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
        [
            self.slots[0].output(&self.bank),
            self.slots[1].output(&self.bank),
            self.slots[2].output(&self.bank),
            self.slots[3].output(&self.bank),
        ]
    }
}
