use embassy_time::Instant;

const BUFFER_SIZE: usize = 4;
const TIMEOUT_MS: u64 = 2000;

pub struct TapTempo {
    last_tap: Option<Instant>,
    intervals: [u64; BUFFER_SIZE],
    count: usize,
    index: usize,
}

impl TapTempo {
    pub fn new() -> Self {
        Self {
            last_tap: None,
            intervals: [0; BUFFER_SIZE],
            count: 0,
            index: 0,
        }
    }

    /// Record a tap. Returns new BPM if enough data, None on first tap or after timeout.
    pub fn tap(&mut self) -> Option<u16> {
        let now = Instant::now();
        let result = if let Some(prev) = self.last_tap {
            let interval_ms = now.duration_since(prev).as_millis() as u64;
            if interval_ms < TIMEOUT_MS {
                self.intervals[self.index] = interval_ms;
                self.index = (self.index + 1) % BUFFER_SIZE;
                if self.count < BUFFER_SIZE { self.count += 1; }

                let sum: u64 = self.intervals[..self.count].iter().sum();
                let avg = sum / self.count as u64;
                Some((60_000 / avg).clamp(20, 300) as u16)
            } else {
                self.count = 0;
                self.index = 0;
                None
            }
        } else {
            None
        };
        self.last_tap = Some(now);
        result
    }
}
