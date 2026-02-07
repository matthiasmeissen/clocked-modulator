#[derive(Debug, defmt::Format)]
pub struct PhasorBank {
    phasor_d2: f32,
    phasor_x1: f32,
    phasor_x2: f32,

    tick_rate: f32,
    base_increment: f32,
}

impl PhasorBank {
    pub fn new(bpm: f32, tick_rate: f32) -> Self {
        Self { 
            phasor_x2: 0.0, 
            phasor_x1: 0.0, 
            phasor_d2: 0.0, 
            tick_rate,
            base_increment: bpm / 60.0 / tick_rate,
        }
    }
    
    pub fn set_bpm(&mut self, bpm: f32) {
        self.base_increment = bpm / 60.0 / self.tick_rate;
    }
    
    pub fn tick(&mut self) {
        self.phasor_d2 = (self.phasor_d2 + self.base_increment * 0.5) % 1.0;
        self.phasor_x1 = (self.phasor_x1 + self.base_increment) % 1.0;
        self.phasor_x2 = (self.phasor_x2 + self.base_increment * 2.0) % 1.0;
    }
}