
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
    fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
            Self::C => 2,
            Self::D => 3,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::A => "Slot A",
            Self::B => "Slot B",
            Self::C => "Slot C",
            Self::D => "Slot D",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum EditPage {
    Waves,
    Range,
}

impl EditPage {
    fn toggle(self) -> Self {
        match self {
            EditPage::Waves => EditPage::Range,
            EditPage::Range => EditPage::Waves,
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
    ModEdit {
        slot: SlotId,
        page: EditPage,
        draft: ModSlot,
    },
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
            (Overview, B2Press) => ModEdit { slot: SlotId::A, page: EditPage::Waves, draft: ModSlot::default() },
            _ => Overview,
        }
    }
}