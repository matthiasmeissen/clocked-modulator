
#[derive(Debug)]
pub struct PhasorBank {
    phasor_d4: f32,     // Four Bears, One Bar (To complete)
    phasor_d2: f32,     // Two Beats (To complete)
    phasor_x1: f32,     // One Beat (To complete)
    phasor_x2: f32,     // Half Beat (To complete)

    tick_rate: f32,
    base_increment: f32,
}

impl PhasorBank {
    pub fn new(bpm: f32, tick_rate: f32) -> Self {
        Self { 
            phasor_d4: 0.0, 
            phasor_d2: 0.0, 
            phasor_x1: 0.0, 
            phasor_x2: 0.0, 
            tick_rate,
            base_increment: bpm / 60.0 / tick_rate,
        }
    }
    
    pub fn set_bpm(&mut self, bpm: f32) {
        self.base_increment = bpm / 60.0 / self.tick_rate;
    }
    
    pub fn tick(&mut self) {
        self.phasor_d4 = (self.phasor_d4 + self.base_increment * 0.25) % 1.0;
        self.phasor_d2 = (self.phasor_d2 + self.base_increment * 0.5) % 1.0;
        self.phasor_x1 = (self.phasor_x1 + self.base_increment) % 1.0;
        self.phasor_x2 = (self.phasor_x2 + self.base_increment * 2.0) % 1.0;
    }
}

// Custom Formatting of Phasor Values
const BAR_WIDTH: usize = 32;

fn format_bar(value: f32) -> [u8; BAR_WIDTH] {
    let filled = (value * BAR_WIDTH as f32) as usize;
    let mut bar = [b'_'; BAR_WIDTH];
    for b in bar.iter_mut().take(filled.min(BAR_WIDTH)) {
        *b = b'#';
    }
    bar
}

impl defmt::Format for PhasorBank {
    fn format(&self, f: defmt::Formatter) {
        let d4 = format_bar(self.phasor_d4);
        let d2 = format_bar(self.phasor_d2);
        let x1 = format_bar(self.phasor_x1);
        let x2 = format_bar(self.phasor_x2);
        defmt::write!(f, "Bar: {} | Two Beats: {} | Beat: {} | Half Beat: {}",
            core::str::from_utf8(&d4).unwrap(),
            core::str::from_utf8(&d2).unwrap(),
            core::str::from_utf8(&x1).unwrap(),
            core::str::from_utf8(&x2).unwrap(),
        );
    }
}