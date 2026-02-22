# Clocked Modulator UI v2 — State Machine

## Hardware Controls

```
Physical:
  ENC1 (rotation + push button B1)
  ENC2 (rotation + push button B4)
  B2, B3, B5, B6 (push buttons)

Maps to InputEvent:
  Enc1Rotate(i8)    — signed delta, + = clockwise
  Enc2Rotate(i8)    — signed delta
  B1Press            — ENC1 push
  B2Press            — top-left button
  B3Press            — top-right button
  B4Press            — ENC2 push
  B5Press            — bottom-left button
  B6Press            — bottom-right button
```

## Domain Types

```rust
/// Existing types (extend, don't duplicate)

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Waveshape { Sin, Tri, Squ, Saw }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Multiplier { D4, D2, X1, X2 }

// Both need: next(), prev(), name(), ALL const


/// Extended slot with range remapping (NEW)
#[derive(Clone, Copy, Debug)]
pub struct ModSlot {
    pub wave: Waveshape,
    pub mul: Multiplier,
    pub min: f32,       // output range minimum (0.0..=1.0)
    pub max: f32,       // output range maximum (0.0..=1.0)
}

impl Default for ModSlot {
    fn default() -> Self {
        Self {
            wave: Waveshape::Sin,
            mul: Multiplier::X1,
            min: 0.0,
            max: 1.0,
        }
    }
}

/// The 4 mod slots
#[derive(Clone, Copy, Debug)]
pub struct ModulatorConfig {
    pub slots: [ModSlot; 4],
}

/// Global transport state
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaybackState {
    Playing,
    Paused,
}
```


## Input Events

```rust
#[derive(Clone, Copy, Debug)]
pub enum InputEvent {
    Enc1Rotate(i8),   // signed: positive = CW, negative = CCW
    Enc2Rotate(i8),
    B1Press,          // ENC1 button
    B2Press,
    B3Press,
    B4Press,          // ENC2 button
    B5Press,
    B6Press,
}
```


## Navigation State

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SlotId { A, B, C, D }

impl SlotId {
    pub fn index(self) -> usize {
        match self {
            SlotId::A => 0,
            SlotId::B => 1,
            SlotId::C => 2,
            SlotId::D => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SlotId::A => "A",
            SlotId::B => "B",
            SlotId::C => "C",
            SlotId::D => "D",
        }
    }
}

/// Which sub-page of Mod Edit we're on
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EditPage {
    Waves,   // ENC1 = wave, ENC2 = mult
    Range,   // ENC1 = max,  ENC2 = min
}

impl EditPage {
    pub fn toggle(self) -> Self {
        match self {
            EditPage::Waves => EditPage::Range,
            EditPage::Range => EditPage::Waves,
        }
    }
}


/// The full UI state — this IS the state machine.
#[derive(Clone, Copy, Debug)]
pub enum NavState {
    /// Home screen: BPM + 4 slots + beat indicator
    Overview,

    /// Tap tempo mode: big BPM display, tap/play/pause
    TapMode,

    /// Editing a mod slot's wave + multiplier
    /// `draft` holds the uncommitted changes
    ModEdit {
        slot: SlotId,
        page: EditPage,
        draft: ModSlot,
    },
}
```

Note: The `draft` lives inside the `NavState` variant. When you enter ModEdit,
the current slot values are copied into `draft`. ENC rotations modify `draft`.
B5 (confirm) writes `draft` back to `ModulatorConfig`. B2 (discard) just
transitions back to Overview without writing.

This means `NavState` is no longer `Copy`-cheap in the sense of being tiny
(ModSlot has 2 enums + 2 floats = 10 bytes, total NavState ≈ 14 bytes),
but that's absolutely fine for embedded — it's still stack-allocated and small.


## Global App State

```rust
/// Everything the UI task owns
pub struct AppState {
    pub nav: NavState,
    pub bpm: u16,
    pub config: ModulatorConfig,
    pub playback: PlaybackState,
    pub bar_beat: u8,           // 0..3 for the beat indicator
}
```


## Transition Function

```rust
impl NavState {
    /// Process an input event. Returns new NavState.
    /// May mutate app-level state (bpm, config, playback).
    pub fn handle(
        self,
        event: InputEvent,
        bpm: &mut u16,
        config: &mut ModulatorConfig,
        playback: &mut PlaybackState,
        reset_bar: &mut bool,        // flag: caller resets bar counter
    ) -> Self {
        use InputEvent::*;
        use NavState::*;

        match (self, event) {

            // ================================================================
            // OVERVIEW
            // ================================================================

            // ENC1 rotate: adjust BPM
            (Overview, Enc1Rotate(delta)) => {
                *bpm = (*bpm as i16 + delta as i16).clamp(20, 300) as u16;
                Overview
            }

            // ENC1 press (B1): enter Tap Mode
            (Overview, B1Press) => TapMode,

            // ENC2 rotate: reset bar (same as B4)
            (Overview, Enc2Rotate(_)) => {
                *reset_bar = true;
                Overview
            }

            // ENC2 press (B4): reset bar
            (Overview, B4Press) => {
                *reset_bar = true;
                Overview
            }

            // B2: enter Mod A edit
            (Overview, B2Press) => ModEdit {
                slot: SlotId::A,
                page: EditPage::Waves,
                draft: config.slots[SlotId::A.index()],
            },

            // B3: enter Mod B edit
            (Overview, B3Press) => ModEdit {
                slot: SlotId::B,
                page: EditPage::Waves,
                draft: config.slots[SlotId::B.index()],
            },

            // B5: enter Mod C edit
            (Overview, B5Press) => ModEdit {
                slot: SlotId::C,
                page: EditPage::Waves,
                draft: config.slots[SlotId::C.index()],
            },

            // B6: enter Mod D edit
            (Overview, B6Press) => ModEdit {
                slot: SlotId::D,
                page: EditPage::Waves,
                draft: config.slots[SlotId::D.index()],
            },


            // ================================================================
            // TAP MODE
            // ================================================================

            // ENC1 rotate: adjust BPM
            (TapMode, Enc1Rotate(delta)) => {
                *bpm = (*bpm as i16 + delta as i16).clamp(20, 300) as u16;
                TapMode
            }

            // B2: back to Overview
            (TapMode, B2Press) => Overview,

            // B3: tap tempo (caller handles the timing calculation)
            // We just signal it — could be a separate flag or channel
            (TapMode, B3Press) => {
                // TODO: tap tempo logic lives outside state machine
                // The UI task timestamps each B3Press and computes BPM
                TapMode
            }

            // B5: pause playback
            (TapMode, B5Press) => {
                *playback = PlaybackState::Paused;
                TapMode
            }

            // B6: start/resume playback
            (TapMode, B6Press) => {
                *playback = PlaybackState::Playing;
                TapMode
            }

            // All other inputs in TapMode: ignore
            (TapMode, _) => TapMode,


            // ================================================================
            // MOD EDIT — WAVES PAGE
            // ================================================================

            // ENC1 rotate: cycle waveshape
            (ModEdit { slot, page: EditPage::Waves, mut draft }, Enc1Rotate(delta)) => {
                draft.wave = if delta > 0 { draft.wave.next() } else { draft.wave.prev() };
                ModEdit { slot, page: EditPage::Waves, draft }
            }

            // ENC2 rotate: cycle multiplier
            (ModEdit { slot, page: EditPage::Waves, mut draft }, Enc2Rotate(delta)) => {
                draft.mul = if delta > 0 { draft.mul.next() } else { draft.mul.prev() };
                ModEdit { slot, page: EditPage::Waves, draft }
            }

            // B2: discard changes, back to Overview
            (ModEdit { .. }, B2Press) => Overview,

            // B3: reset values to defaults
            (ModEdit { slot, page, .. }, B3Press) => {
                ModEdit { slot, page, draft: ModSlot::default() }
            }

            // B5: confirm changes, write to config, back to Overview
            (ModEdit { slot, draft, .. }, B5Press) => {
                config.slots[slot.index()] = draft;
                Overview
            }

            // B6: toggle between Waves ↔ Range page
            (ModEdit { slot, page, draft }, B6Press) => {
                ModEdit { slot, page: page.toggle(), draft }
            }


            // ================================================================
            // MOD EDIT — RANGE PAGE
            // ================================================================

            // ENC1 rotate: adjust max value
            (ModEdit { slot, page: EditPage::Range, mut draft }, Enc1Rotate(delta)) => {
                draft.max = (draft.max + delta as f32 * 0.05).clamp(0.0, 1.0);
                ModEdit { slot, page: EditPage::Range, draft }
            }

            // ENC2 rotate: adjust min value
            (ModEdit { slot, page: EditPage::Range, mut draft }, Enc2Rotate(delta)) => {
                draft.min = (draft.min + delta as f32 * 0.05).clamp(0.0, 1.0);
                ModEdit { slot, page: EditPage::Range, draft }
            }

            // B2, B3, B5, B6 already handled above (shared across both pages)

            // Catch-all: ignore unhandled combinations
            (state, _) => state,
        }
    }
}
```


## Display Refresh Strategy for Beat Indicator

The beat indicator updates every beat (at 120 BPM = every 500ms, at 240 BPM = every 250ms).
Full I2C OLED redraws at 400kHz typically take 20-30ms for a 128x64 display.

Options, from simplest to most optimized:

### 1. Dirty flag — only redraw on change (start here)
```rust
loop {
    let mut dirty = false;

    // Check for input events (non-blocking)
    if let Ok(event) = INPUT_EVENTS.try_receive() {
        nav = nav.handle(event, &mut bpm, &mut config, &mut playback, &mut reset_bar);
        dirty = true;
    }

    // Check for beat tick (from modulator task via channel)
    if let Ok(beat) = BEAT_TICK.try_receive() {
        bar_beat = beat;
        dirty = true;  // only if on Overview page
    }

    if dirty {
        display.render(&nav, bpm, &config, bar_beat);
    }

    Timer::after_millis(1).await;  // yield to other tasks
}
```

### 2. Partial update — only redraw the beat indicator region
If full redraws are too slow, use `embedded-graphics` to only draw the
beat indicator rectangle region and flush just that portion. Most SSD1306
drivers support setting the draw window (page/column address).

### 3. Separate the beat indicator to a different output
If display refresh is genuinely a bottleneck, consider driving the beat
indicator with 4 discrete LEDs instead. Offloads the display entirely
and gives you a always-visible beat reference even when in Mod Edit pages.
This is actually how most eurorack modules do it.


## Embassy Task Architecture

```
                                ┌─────────────────────────────┐
encoder1_task ─── Enc1Rotate ──▶│                             │
button1_task  ─── B1Press    ──▶│                             │
                                │   INPUT_EVENTS channel      │
encoder2_task ─── Enc2Rotate ──▶│   (capacity: 8)             │
button4_task  ─── B4Press    ──▶│                             │
                                │                             │
button2_task  ─── B2Press    ──▶│                             │
button3_task  ─── B3Press    ──▶├────────────────────────────▶│  ui_task
button5_task  ─── B5Press    ──▶│                             │  (owns NavState,
button6_task  ─── B6Press    ──▶│                             │   AppState,
                                └─────────────────────────────┘   Display)
                                                                     │
modulator_task ◀─── CONFIG_CHANNEL (ModulatorConfig, on B5 confirm) ─┘
                ◀─── BPM_CHANNEL (u16, on any BPM change)
                ◀─── PLAYBACK_CHANNEL (PlaybackState)
                ───▶ BEAT_TICK channel (u8, beat index 0..3) ──▶ ui_task
```

Note: You don't need a separate task per button. A single `input_task` can
poll all 6 buttons + 2 encoders and send events. But with Embassy, separate
tasks using `wait_for_falling_edge()` per pin are idiomatic and free
(Embassy tasks are zero-cost when awaiting).


## Min/Max Remapping in the Modulator

The range remap is a simple linear interpolation applied to the raw 0.0–1.0
waveform output:

```rust
/// Remap a normalized 0.0..1.0 value to the min..max range
fn remap(value: f32, min: f32, max: f32) -> f32 {
    min + value * (max - min)
}
```

Note: if `min > max`, this inverts the waveform — which could be a useful
feature (inverted LFO). Decide whether to allow or clamp `min <= max`.


## Tap Tempo Implementation Note

Tap tempo lives *outside* the state machine — it's timing logic, not state
transitions. The `ui_task` handles it:

```rust
let mut last_tap: Option<Instant> = None;

// Inside the event loop, when in TapMode and B3Press:
if matches!(nav, NavState::TapMode) && matches!(event, InputEvent::B3Press) {
    if let Some(prev) = last_tap {
        let interval_ms = prev.elapsed().as_millis() as u32;
        if interval_ms > 150 && interval_ms < 2000 {
            // Could average last N taps for stability
            bpm = (60_000 / interval_ms) as u16;
        }
    }
    last_tap = Some(Instant::now());
}
```