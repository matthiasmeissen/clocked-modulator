
use crate::{
    input::InputEvent,
    modulator::{ModSlot, ModulatorConfig},
};

#[derive(Clone, Copy, PartialEq)]
pub enum SlotId {
    A,
    B,
    C,
    D,
}

impl SlotId {
    pub fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
            Self::C => 2,
            Self::D => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum PlaybackState {
    Playing,
    Paused,
}

#[derive(Clone, Copy, PartialEq)]
pub enum NavState {
    Overview,
    TapMode,
    ModEditWave { slot: SlotId, draft: ModSlot },
    ModEditRange { slot: SlotId, draft: ModSlot },
}

impl NavState {
    pub fn handle(
        self,
        event: InputEvent,
        bpm: &mut u16,
        config: &mut ModulatorConfig,
        playback: &mut PlaybackState,
        rest_bar: &mut bool,
    ) -> Self {
        use InputEvent::*;
        use NavState::*;
        match (self, event) {
            // Overview
            (Overview, Enc1Rotate(delta)) => {
                *bpm = (*bpm as i16 + delta as i16).clamp(20, 300) as u16;
                Overview
            }
            (Overview, B1Press) => TapMode,
            (Overview, B2Press) => ModEditWave { slot: SlotId::A, draft: config.slots[SlotId::A.index()] },
            (Overview, B3Press) => ModEditWave { slot: SlotId::B, draft: config.slots[SlotId::B.index()] },
            (Overview, B5Press) => ModEditWave { slot: SlotId::C, draft: config.slots[SlotId::C.index()] },
            (Overview, B6Press) => ModEditWave { slot: SlotId::D, draft: config.slots[SlotId::D.index()] },

            // Tap Mode
            (TapMode, Enc1Rotate(..)) => self,
            (TapMode, B1Press) => { *bpm = 120; TapMode },      // Reset BPM to 120
            (TapMode, B2Press) => Overview,

            // WaveEdit
            (ModEditWave { slot, mut draft }, Enc1Rotate(delta)) => {
                draft.wave = if delta > 0 {draft.wave.next()} else {draft.wave.prev()};
                ModEditWave { slot, draft }
            }
            (ModEditWave { slot, draft}, B1Press) => {
                config.slots[slot.index()] = draft;
                self
            },
            (ModEditWave {..}, B2Press) => Overview,
            (state, _) => state,
        }
    }
}