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
        match self {
            Multiplier::D4 => 0,
            Multiplier::D2 => 1,
            Multiplier::X1 => 2,
            Multiplier::X2 => 3,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Multiplier::D4 => "/4",
            Multiplier::D2 => "/2",
            Multiplier::X1 => "x1",
            Multiplier::X2 => "x2",
        }
    }
}

/// Time-based phasor bank. Computes phases from absolute elapsed time
/// rather than accumulating per tick, so jitter in tick timing has
/// zero effect on the output.
#[derive(Debug, Clone, Copy)]
pub struct PhasorBank {
    pub phases: [f32; Multiplier::ALL.len()],
    /// Beats per second (bpm / 60)
    beats_per_sec: f32,
    /// Elapsed seconds at the time origin
    origin_secs: f32,
    /// Accumulated base beats at origin, so phase carries over on BPM change
    beat_offset: f32,
}

impl PhasorBank {
    pub fn new(bpm: f32) -> Self {
        Self {
            phases: [0.0; Multiplier::ALL.len()],
            beats_per_sec: bpm / 60.0,
            origin_secs: 0.0,
            beat_offset: 0.0,
        }
    }

    pub fn set_bpm(&mut self, bpm: f32, elapsed_secs: f32) {
        // Carry over accumulated beats before changing rate
        let dt = elapsed_secs - self.origin_secs;
        self.beat_offset += dt * self.beats_per_sec;
        if self.beat_offset >= 4.0 {
            self.beat_offset -= 4.0;
        }
        self.origin_secs = elapsed_secs;
        self.beats_per_sec = bpm / 60.0;
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

        // Re-anchor origin every bar to keep dt small
        if dt > 4.0 / self.beats_per_sec {
            self.beat_offset = base_beats;
            if self.beat_offset >= 4.0 {
                self.beat_offset -= 4.0;
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
