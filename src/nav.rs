use crate::input::InputEvent;
use crate::modulator::{ModSlot, ModulatorConfig};

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
        reset_bar: &mut bool,
    ) -> Self {
        use InputEvent::*;
        use NavState::*;
        match (self, event) {
            // ------------------------
            // OVERVIEW PAGE
            // ------------------------
            (Overview, Enc1Rotate(delta)) => {
                *bpm = (*bpm as i16 + delta as i16).clamp(20, 300) as u16;
                Overview
            }
            // Encoder 2 Rotate does nothing
            (Overview, B1Press) => TapMode,
            (Overview, B2Press) => ModEditWave { slot: SlotId::A, draft: config.slots[SlotId::A.index()] },
            (Overview, B3Press) => ModEditWave { slot: SlotId::B, draft: config.slots[SlotId::B.index()] },
            // Encoder Button 4 Press does nothing
            (Overview, B5Press) => ModEditWave { slot: SlotId::C, draft: config.slots[SlotId::C.index()] },
            (Overview, B6Press) => ModEditWave { slot: SlotId::D, draft: config.slots[SlotId::D.index()] },

            // ------------------------
            // TAP TEMPO PAGE
            // ------------------------
            (TapMode, Enc1Rotate(delta)) => {
                *bpm = (*bpm as i16 + delta as i16).clamp(20, 300) as u16;
                TapMode
            }
            // Encoder 2 Rotate does nothing
            // Button 1 Press does nothing
            (TapMode, B2Press) => Overview,
            // B3Press (tap tempo) is handled in input_task, not here
            (TapMode, B4Press) => { *bpm = 120; TapMode },      // Reset BPM to 120
            (TapMode, B5Press) => {
                *playback = PlaybackState::Paused;
                TapMode
            }
            (TapMode, B6Press) => {
                match *playback {
                    PlaybackState::Paused => *playback = PlaybackState::Playing,
                    PlaybackState::Playing => *reset_bar = true,
                }
                TapMode
            }

            // ------------------------
            // MODEDIT WAVE PAGE
            // ------------------------
            (ModEditWave { slot, mut draft }, Enc1Rotate(delta)) => {
                draft.wave = if delta > 0 {draft.wave.next()} else {draft.wave.prev()};
                ModEditWave { slot, draft }
            }
            (ModEditWave { slot, mut draft }, Enc2Rotate(delta)) => {
                draft.mul = if delta > 0 {draft.mul.next()} else {draft.mul.prev()};
                ModEditWave { slot, draft }
            }
            // Encoder Button 1 Press does nothing
            (ModEditWave {..}, B2Press) => Overview,
            (ModEditWave {slot, mut draft}, B3Press) => {
                let defaults = ModSlot::default();
                draft.wave = defaults.wave;
                draft.mul = defaults.mul;
                ModEditWave { slot, draft }
            },
            (ModEditWave { slot, draft}, B5Press) => {
                // TO DO: Consider showing "Ok+" as label, when pending changes
                config.slots[slot.index()] = draft;
                self
            },
            (ModEditWave {slot, draft}, B6Press) => ModEditRange { slot, draft },
            
            // ------------------------
            // MODEDIT RANGE PAGE
            // ------------------------
            (ModEditRange { slot, mut draft }, Enc1Rotate(delta)) => {
                draft.min = (draft.min + delta as f32 * 0.05).clamp(0.0, 1.0);
                ModEditRange { slot, draft }
            }
            (ModEditRange { slot, mut draft }, Enc2Rotate(delta)) => {
                draft.max = (draft.max + delta as f32 * 0.05).clamp(0.0, 1.0);
                ModEditRange { slot, draft }
            }
            // Encoder Button 1 Press does nothing
            (ModEditRange {..}, B2Press) => Overview,
            (ModEditRange {slot, mut draft}, B3Press) => {
                draft.min = 0.0;
                draft.max = 1.0;
                ModEditRange { slot, draft }
            },
            // Encoder Button 4 Press does nothing
            (ModEditRange { slot, draft}, B5Press) => {
                config.slots[slot.index()] = draft;
                self
            },
            (ModEditRange {slot, draft}, B6Press) => ModEditWave { slot, draft },

            (state, _) => state,
        }
    }
}