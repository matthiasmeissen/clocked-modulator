#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Multiplier {
    D8,     // 8 bars
    D4,     // 4 bars
    D2,     // 2 bars
    X1,     // 1 bar
    X2,     // 1/2 bar (2 beats)
    X4,     // 1/4 bar (1 beat)
}

impl Multiplier {
    pub const ALL: [Multiplier; 6] = [
        Multiplier::D8,
        Multiplier::D4,
        Multiplier::D2,
        Multiplier::X1,
        Multiplier::X2,
        Multiplier::X4,
    ];

    pub fn next(self) -> Self {
        match self {
            Multiplier::D8 => Multiplier::D4,
            Multiplier::D4 => Multiplier::D2,
            Multiplier::D2 => Multiplier::X1,
            Multiplier::X1 => Multiplier::X2,
            Multiplier::X2 => Multiplier::X4,
            Multiplier::X4 => Multiplier::D8,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Multiplier::D8 => Multiplier::X4,
            Multiplier::D4 => Multiplier::D8,
            Multiplier::D2 => Multiplier::D4,
            Multiplier::X1 => Multiplier::D2,
            Multiplier::X2 => Multiplier::X1,
            Multiplier::X4 => Multiplier::X2,
        }
    }

    pub fn factor(self) -> f32 {
        match self {
            Multiplier::D8 => 0.03125, // 1 cycle = 8 bars = 32 beats
            Multiplier::D4 => 0.0625,  // 1 cycle = 4 bars = 16 beats
            Multiplier::D2 => 0.125,   // 1 cycle = 2 bars = 8 beats
            Multiplier::X1 => 0.25,    // 1 cycle = 1 bar  = 4 beats
            Multiplier::X2 => 0.5,     // 1 cycle = 2 beats
            Multiplier::X4 => 1.0,     // 1 cycle = 1 beat
        }
    }

    pub fn index(self) -> usize {
        match self {
            Multiplier::D8 => 0,
            Multiplier::D4 => 1,
            Multiplier::D2 => 2,
            Multiplier::X1 => 3,
            Multiplier::X2 => 4,
            Multiplier::X4 => 5,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Multiplier::D8 => "/8",
            Multiplier::D4 => "/4",
            Multiplier::D2 => "/2",
            Multiplier::X1 => "x1",
            Multiplier::X2 => "x2",
            Multiplier::X4 => "x4",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum GlobalSpeed {
    Quarter,  // 0.25x
    Half,     // 0.5x
    X1,       // 1.0x (default)
    Double,   // 2.0x
    Quad,     // 4.0x
}

impl GlobalSpeed {
    pub fn next(self) -> Self {
        match self {
            GlobalSpeed::Quarter => GlobalSpeed::Half,
            GlobalSpeed::Half => GlobalSpeed::X1,
            GlobalSpeed::X1 => GlobalSpeed::Double,
            GlobalSpeed::Double => GlobalSpeed::Quad,
            GlobalSpeed::Quad => GlobalSpeed::Quad,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            GlobalSpeed::Quarter => GlobalSpeed::Quarter,
            GlobalSpeed::Half => GlobalSpeed::Quarter,
            GlobalSpeed::X1 => GlobalSpeed::Half,
            GlobalSpeed::Double => GlobalSpeed::X1,
            GlobalSpeed::Quad => GlobalSpeed::Double,
        }
    }

    pub fn factor(self) -> f32 {
        match self {
            GlobalSpeed::Quarter => 0.25,
            GlobalSpeed::Half => 0.5,
            GlobalSpeed::X1 => 1.0,
            GlobalSpeed::Double => 2.0,
            GlobalSpeed::Quad => 4.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            GlobalSpeed::Quarter => "/4",
            GlobalSpeed::Half => "/2",
            GlobalSpeed::X1 => "x1",
            GlobalSpeed::Double => "x2",
            GlobalSpeed::Quad => "x4",
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => GlobalSpeed::Quarter,
            1 => GlobalSpeed::Half,
            2 => GlobalSpeed::X1,
            3 => GlobalSpeed::Double,
            4 => GlobalSpeed::Quad,
            _ => GlobalSpeed::X1,
        }
    }
}

/// Time-based phasor bank. Computes phases from absolute elapsed time
/// rather than accumulating per tick, so jitter in tick timing has
/// zero effect on the output.
#[derive(Debug, Clone, Copy)]
pub struct PhasorBank {
    pub phases: [f32; Multiplier::ALL.len()],
    bpm: f32,
    speed_factor: f32,
    /// Effective rate: (bpm / 60) * speed_factor
    beats_per_sec: f32,
    /// Elapsed seconds at the time origin
    origin_secs: f32,
    /// Accumulated base beats at origin, so phase carries over on BPM/speed change
    beat_offset: f32,
}

/// Wrap point for beat_offset. Must equal the period (in beats) of the
/// slowest multiplier so all variants complete full cycles before wrapping.
/// D8 = 8 bars = 32 beats → 1/0.03125 = 32.
const BEAT_WRAP: f32 = 32.0;

impl PhasorBank {
    pub fn new(bpm: f32) -> Self {
        Self {
            phases: [0.0; Multiplier::ALL.len()],
            bpm,
            speed_factor: 1.0,
            beats_per_sec: bpm / 60.0,
            origin_secs: 0.0,
            beat_offset: 0.0,
        }
    }

    fn carry_over(&mut self, elapsed_secs: f32) {
        let dt = elapsed_secs - self.origin_secs;
        self.beat_offset += dt * self.beats_per_sec;
        if self.beat_offset >= BEAT_WRAP {
            self.beat_offset -= BEAT_WRAP;
        }
        self.origin_secs = elapsed_secs;
    }

    fn recompute_rate(&mut self) {
        self.beats_per_sec = (self.bpm / 60.0) * self.speed_factor;
    }

    pub fn set_bpm(&mut self, bpm: f32, elapsed_secs: f32) {
        self.carry_over(elapsed_secs);
        self.bpm = bpm;
        self.recompute_rate();
    }

    pub fn set_speed(&mut self, factor: f32, elapsed_secs: f32) {
        self.carry_over(elapsed_secs);
        self.speed_factor = factor;
        self.recompute_rate();
    }

    /// Recompute all phases from the current time.
    pub fn update(&mut self, elapsed_secs: f32) {
        let dt = elapsed_secs - self.origin_secs;
        let base_beats = self.beat_offset + dt * self.beats_per_sec;

        for (idx, mul) in Multiplier::ALL.iter().enumerate() {
            let mut phase = base_beats * mul.factor();
            while phase >= 1.0 {
                phase -= 1.0;
            }
            self.phases[idx] = phase;
        }

        // Re-anchor origin every BEAT_WRAP beats to keep dt small
        if dt > BEAT_WRAP / self.beats_per_sec {
            self.beat_offset = base_beats;
            if self.beat_offset >= BEAT_WRAP {
                self.beat_offset -= BEAT_WRAP;
            }
            self.origin_secs = elapsed_secs;
        }
    }

    pub fn get_phase(&self, mul: Multiplier) -> f32 {
        self.phases[mul.index()]
    }

    pub fn reset(&mut self, elapsed_secs: f32) {
        self.origin_secs = elapsed_secs;
        self.beat_offset = 0.0;
        self.phases = [0.0; Multiplier::ALL.len()];
    }
}
