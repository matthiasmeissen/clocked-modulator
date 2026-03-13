#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Multiplier {
    D4,     // 4 Beats, One Bar (To complete)
    D2,     // 2 Beats (To complete)
    X1,     // 1 Beat (To complete)
    X2,     // 1/2 Beat (To complete)
}

impl Multiplier {
    pub const ALL: [Multiplier; 4] = [
        Multiplier::D4,
        Multiplier::D2,
        Multiplier::X1,
        Multiplier::X2,
    ];

    // Those could be solved more elegantly 
    // but this approach is readable and fast
    pub fn next(self) -> Self {
        match self {
            Multiplier::D4 => Multiplier::D2,
            Multiplier::D2 => Multiplier::X1,
            Multiplier::X1 => Multiplier::X2,
            Multiplier::X2 => Multiplier::D4,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Multiplier::D4 => Multiplier::X2,
            Multiplier::D2 => Multiplier::D4,
            Multiplier::X1 => Multiplier::D2,
            Multiplier::X2 => Multiplier::X1,
        }
    }

    pub fn factor(self) -> f32 {
        match self {
            Multiplier::D4 => 0.25,
            Multiplier::D2 => 0.5,
            Multiplier::X1 => 1.0,
            Multiplier::X2 => 2.0,
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&m| m == self).unwrap()
    }

    pub fn name(self) -> &'static str {
        match self {
            Multiplier::D4 => "D4",
            Multiplier::D2 => "D2",
            Multiplier::X1 => "X1",
            Multiplier::X2 => "X2",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PhasorBank {
    pub phases: [f32; Multiplier::ALL.len()],
    tick_rate: f32,
    base_increment: f32,
}

impl PhasorBank {
    pub fn new(bpm: f32, tick_rate: f32) -> Self {
        Self { 
            phases: [0.0; Multiplier::ALL.len()],
            tick_rate,
            base_increment: bpm / 60.0 / tick_rate,
        }
    }
    
    pub fn set_bpm(&mut self, bpm: f32) {
        self.base_increment = bpm / 60.0 / self.tick_rate;
    }

    pub fn get_phase(&self, mul: Multiplier) -> f32 {
        self.phases[mul.index()]
    }
    
    pub fn tick(&mut self) {
        for (idx, mul) in Multiplier::ALL.iter().enumerate() {
            self.phases[idx] = (self.phases[idx] + self.base_increment * mul.factor()) % 1.0;
        }
    }

    pub fn reset(&mut self) {
        self.phases = [0.0; Multiplier::ALL.len()];
    }
}