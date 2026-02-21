# Embedded Display UI: The Mental Model

## The Core Concept: Hierarchical State Machine + Separated Concerns

Your diagram reveals three distinct concerns that you need to keep cleanly separated:

1. **Data Model** — What does the modulator *know*? (BPM value, waveform type, multiplier, etc.)
2. **Navigation State** — Where am I in the menu tree? (Which item is focused, am I editing?)
3. **Rendering** — How do I draw the current state to pixels?
4. **Input** — What abstract actions can the user perform?

The key insight: **your navigation/UI state is itself a state machine, completely independent of your domain data.** The rotary encoder (or buttons, or whatever) produces abstract *events*, the state machine processes them and may mutate either the navigation state or the domain data, and then the renderer draws based on both.

```
┌──────────┐     ┌─────────────────┐     ┌────────────┐     ┌──────────┐
│  Input    │────▶│  State Machine  │────▶│   Domain   │────▶│ Renderer │
│ (encoder) │     │  (navigation)   │     │   Data     │     │ (display)│
└──────────┘     └─────────────────┘     └────────────┘     └──────────┘
     │                    │                                       ▲
     │                    └───────────────────────────────────────┘
     │                         (state also feeds renderer)
```

---

## Mental Model 1: The Enum State Machine (Start Here)

For your modulator, the simplest and most Rust-idiomatic approach is a **nested enum** for navigation state. No crate needed. This is the pattern you should internalize first.

The modulator has 8 slots arranged in a 4x2 grid on the display, plus a BPM value. The navigation tree:

```
Browse (index 0-8, flat list)
├── BPM (index 0)    → Enter → EditBpm
├── Slot 0 (index 1) → Enter → SlotFocus(0, Wave) ←→ SlotFocus(0, Mul)
│                                    ↓ Enter              ↓ Enter
│                              SlotEdit(0, Wave)    SlotEdit(0, Mul)
├── Slot 1 (index 2) → ...
├── ...
└── Slot 7 (index 8) → ...
```

The navigation has two "axes":
- **NEXT/PREV** = move between siblings at the current level (rotate encoder)
- **ENTER** = go deeper (short press) / **BACK** = go up (long press)

This is a classic **tree cursor** or **focus path** pattern.

---

## Concrete Types for the Clocked Modulator

The domain types already exist in the codebase. Don't create new parallel types — extend the existing ones.

### Existing types (no changes needed)

```rust
// phasor.rs — already has ALL, factor(), index(), name()
pub enum Multiplier { D4, D2, X1, X2 }

// modulator.rs
pub enum Waveshape { Sin, Tri, Squ, Saw }

// modulator.rs — pairs one Multiplier with one Waveshape
pub struct ModSlot { pub mul: Multiplier, pub wave: Waveshape }

// modulator.rs — the full config: 8 slots
pub struct ModulatorConfig { pub slots: [ModSlot; 8] }
```

### To add: cycling methods for the state machine

`Multiplier` already has `ALL`. `Waveshape` needs it too, plus both need `next()`/`prev()`:

```rust
// Add to Waveshape in modulator.rs
impl Waveshape {
    pub const ALL: [Waveshape; 4] = [
        Waveshape::Sin, Waveshape::Tri, Waveshape::Squ, Waveshape::Saw,
    ];

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&w| w == self).unwrap();
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let i = Self::ALL.iter().position(|&w| w == self).unwrap();
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

// Add to Multiplier in phasor.rs (same pattern)
impl Multiplier {
    pub fn next(self) -> Self { /* same pattern using ALL */ }
    pub fn prev(self) -> Self { /* same pattern using ALL */ }
}
```

Note: `Waveshape` needs `PartialEq` derived for `position()` to work.


### Input events (new type)

```rust
#[derive(Clone, Copy)]
pub enum InputEvent {
    Next,       // encoder clockwise
    Prev,       // encoder counter-clockwise
    Enter,      // short press (< 500ms)
    Back,       // long press (>= 500ms)
}
```

The encoder hardware produces rotation + press. The `button_task` already measures hold duration — use a threshold (e.g. 500ms) to distinguish Enter from Back.


### Navigation state (new type)

```rust
#[derive(Clone, Copy)]
pub enum SlotParam {
    Wave,
    Mul,
}

/// The full navigation state — this IS the state machine.
/// Browse cycles through 9 items: index 0 = BPM, index 1-8 = slots 0-7.
#[derive(Clone, Copy)]
pub enum NavState {
    /// Top level: NEXT/PREV cycles through BPM and 8 slots
    Browse { index: u8 },

    /// Editing BPM value directly (NEXT/PREV change the value)
    EditBpm,

    /// Inside a slot, picking which param to edit
    SlotFocus { slot: u8, param: SlotParam },

    /// Actively changing a waveshape or multiplier value
    SlotEdit { slot: u8, param: SlotParam },
}
```

Note: With 8 slots in a 4x2 grid on the display, `Browse { index }` treats them as a flat list (0-8). An alternative would be 2D cursor navigation `Browse { row: u8, col: u8 }` — but start simple with the flat list.

---

## The Transition Function (the Heart of It)

The `handle()` method takes the current nav state + an input event, optionally mutates the domain data (BPM or `ModulatorConfig`), and returns the new nav state. This is where all UI logic lives — one pure function, no callbacks.

```rust
const BROWSE_COUNT: u8 = 9; // 1 BPM + 8 slots

impl NavState {
    pub fn handle(self, event: InputEvent, bpm: &mut u16, config: &mut ModulatorConfig) -> Self {
        match (self, event) {

            // ── Top-level browsing (index 0 = BPM, 1-8 = slots) ──
            (NavState::Browse { index }, InputEvent::Next) =>
                NavState::Browse { index: (index + 1) % BROWSE_COUNT },

            (NavState::Browse { index }, InputEvent::Prev) =>
                NavState::Browse { index: (index + BROWSE_COUNT - 1) % BROWSE_COUNT },

            (NavState::Browse { index: 0 }, InputEvent::Enter) =>
                NavState::EditBpm,

            (NavState::Browse { index }, InputEvent::Enter) =>
                NavState::SlotFocus { slot: index - 1, param: SlotParam::Wave },

            // ── BPM editing ──
            (NavState::EditBpm, InputEvent::Next) => {
                *bpm = (*bpm + 1).min(300);
                NavState::EditBpm
            }
            (NavState::EditBpm, InputEvent::Prev) => {
                *bpm = bpm.saturating_sub(1).max(20);
                NavState::EditBpm
            }
            (NavState::EditBpm, InputEvent::Enter | InputEvent::Back) =>
                NavState::Browse { index: 0 },

            // ── Slot: param focus (Wave ↔ Mul) ──
            (NavState::SlotFocus { slot, param }, InputEvent::Next | InputEvent::Prev) =>
                NavState::SlotFocus { slot, param: param.toggle() },

            (NavState::SlotFocus { slot, param }, InputEvent::Enter) =>
                NavState::SlotEdit { slot, param },

            (NavState::SlotFocus { slot, .. }, InputEvent::Back) =>
                NavState::Browse { index: slot + 1 },

            // ── Slot: value editing ──
            (NavState::SlotEdit { slot, param: SlotParam::Wave }, InputEvent::Next) => {
                config.slots[slot as usize].wave =
                    config.slots[slot as usize].wave.next();
                self
            }
            (NavState::SlotEdit { slot, param: SlotParam::Wave }, InputEvent::Prev) => {
                config.slots[slot as usize].wave =
                    config.slots[slot as usize].wave.prev();
                self
            }
            (NavState::SlotEdit { slot, param: SlotParam::Mul }, InputEvent::Next) => {
                config.slots[slot as usize].mul =
                    config.slots[slot as usize].mul.next();
                self
            }
            (NavState::SlotEdit { slot, param: SlotParam::Mul }, InputEvent::Prev) => {
                config.slots[slot as usize].mul =
                    config.slots[slot as usize].mul.prev();
                self
            }
            (NavState::SlotEdit { slot, param }, InputEvent::Enter | InputEvent::Back) =>
                NavState::SlotFocus { slot, param },

            _ => self,
        }
    }
}
```

Note: `SlotParam` only has two variants (`Wave`, `Mul`), so `next()` and `prev()` are the same — a simple `toggle()` method works.

---

## Rendering

The renderer reads `NavState` + `ModulatorConfig` + BPM and draws. No mutation. Pure function of state to pixels.

The existing `display.rs` has `draw_bpm()` and `draw_modulator()` — these need to be extended with selection/editing visual feedback.

```rust
impl Display {
    pub fn render(&mut self, nav: &NavState, bpm: u16, config: &ModulatorConfig) {
        self.driver.clear();

        // BPM at top — highlight when selected, show cursor when editing
        let bpm_selected = matches!(nav,
            NavState::Browse { index: 0 } | NavState::EditBpm);
        let bpm_editing = matches!(nav, NavState::EditBpm);
        self.draw_bpm(bpm, bpm_selected, bpm_editing);

        // 8 slots in a 4x2 grid (existing layout from draw_main)
        for slot in 0..8u8 {
            let row = slot / 4;
            let col = slot % 4;
            let pos = Point::new(col as i32 * 30, 12 + row as i32 * 24);

            let mod_slot = &config.slots[slot as usize];

            // Determine visual state from NavState
            let highlighted = matches!(nav, NavState::Browse { index } if index == slot + 1);
            let focused_param = match nav {
                NavState::SlotFocus { slot: s, param } if *s == slot => Some(*param),
                NavState::SlotEdit { slot: s, param } if *s == slot => Some(*param),
                _ => None,
            };
            let editing = matches!(nav, NavState::SlotEdit { slot: s, .. } if *s == slot);

            self.draw_modulator(pos, mod_slot.wave.name(), mod_slot.mul.name(),
                                highlighted, focused_param, editing);
        }

        self.driver.flush().ok();
    }
}
```

Note: `Waveshape` needs a `name()` method (like `Multiplier` already has). The visual feedback for "selected", "focused", and "editing" states could be as simple as inverting colors or drawing a thicker border — keep it minimal to start.

---

## Embassy Async Architecture

The app uses Embassy with separate async tasks. The state machine lives in a **UI task** that receives input events and owns the display.

### Event channel

A static channel carries input events from encoder/button tasks to the UI task:

```rust
// main.rs
static INPUT_EVENTS: Channel<CriticalSectionRawMutex, InputEvent, 4> = Channel::new();
```

### Encoder and button tasks send events

```rust
// encoder.rs — encoder_task sends Next/Prev
match encoder.update() {
    Direction::Clockwise     => { let _ = INPUT_EVENTS.try_send(InputEvent::Next); }
    Direction::Anticlockwise => { let _ = INPUT_EVENTS.try_send(InputEvent::Prev); }
    Direction::None => {}
}

// encoder.rs — button_task sends Enter or Back based on hold duration
let held_ms = press_start.elapsed().as_millis();
let event = if held_ms >= 500 { InputEvent::Back } else { InputEvent::Enter };
let _ = INPUT_EVENTS.try_send(event);
```

### UI task receives events and updates state

```rust
// main.rs
#[embassy_executor::task]
async fn ui_task(i2c: i2c::I2c<'static, I2C0, i2c::Blocking>) {
    let mut display = Display::new(i2c);
    let mut nav = NavState::Browse { index: 0 };
    let mut bpm: u16 = 120;
    let mut config = ModulatorConfig::default();

    // Initial draw
    display.render(&nav, bpm, &config);

    loop {
        // Await next input event (async, no busy-waiting)
        let event = INPUT_EVENTS.receive().await;

        // Process through state machine
        nav = nav.handle(event, &mut bpm, &mut config);

        // Re-render (only on input, not on a timer)
        display.render(&nav, bpm, &config);

        // If config changed, propagate to modulator task
        // (via another channel or shared mutex)
    }
}
```

### Propagating config changes to the modulator task

The modulator task needs to know when `ModulatorConfig` or BPM changes. Two options:

1. **Channel** — send the full `ModulatorConfig` (it's `Copy`, 72 bytes). Simple and decoupled.
2. **Shared Mutex** — `Mutex<RefCell<ModulatorConfig>>` read by the modulator task each tick. Lower latency but couples the tasks.

Start with the channel approach — it matches the existing `BPM_BUS` pattern in the codebase.

---

## When to Reach for a Crate vs. Hand-Roll

| Approach | When to use |
|----------|-------------|
| **Hand-rolled enum + match** | Your case. <10 states, clear hierarchy. Maximum control, zero dependencies, easy to debug on embedded. |
| **`statig` crate** | When you need hierarchical superstates with entry/exit actions (e.g. "entering EditBpm always resets a cursor"). `no_std` compatible, zero alloc. |
| **`embedded-menu` crate** | If your UI is literally a scrolling list menu. Not flexible enough for your grid layout. |
| **`kolibri-embedded-gui`** | If you want widget-level abstractions (buttons, labels). More overhead, good for larger displays. |

For your modulator with an OLED and a rotary encoder, **hand-rolled enum state machine is the right call.** It's transparent, debuggable, and fits the Pico 2's constraints perfectly.

---

## Key Resources

### State Machines in Rust
- **"Pretty State Machine Patterns in Rust"** — https://hoverbear.org/blog/rust-state-machine-pattern/
  The foundational article on enum-based state machines in Rust. Read this first.
- **`statig` crate** — https://github.com/mdeloof/statig
  Hierarchical state machines, `no_std`, no alloc. Study this if your UI grows complex.
- **David Harel's Statecharts paper (1987)** — the original theory behind hierarchical state machines.
  Overview: https://statecharts.github.io/

### Embedded Graphics
- **`embedded-graphics` docs** — https://docs.rs/embedded-graphics/latest/embedded_graphics/
- **`embedded-graphics-simulator`** — SDL2-based simulator so you can iterate on your UI on desktop before flashing to hardware.
- **`embedded-menu`** — https://github.com/bugadani/embedded-menu (reference for how others solve the menu problem)

### Conceptual / Architecture
- **The Elm Architecture (TEA)** — Model → Update → View. Your pattern IS this: `ModulatorState` is the model, `NavState::handle()` is update, `render()` is view. Understanding TEA will solidify your mental model even though you won't use Elm.
- **Josh on Design: Embedded Rust GUI** — https://joshondesign.com/2025/09/16/embedded_rust_03
  Recent progress report on building GUIs with `embedded-graphics`. Good for seeing where the ecosystem is headed.

---

## Summary: The Mental Model

Think of it as three layers:

```
  ┌─────────────────────────────┐
  │   What you see (Renderer)   │  Pure function of state → pixels
  ├─────────────────────────────┤
  │   Where you are (NavState)  │  Enum state machine, transitions on input
  ├─────────────────────────────┤
  │   What it is (Domain Data)  │  BPM + ModulatorConfig (8 slots)
  └─────────────────────────────┘
        ▲
        │  InputEvent (Next, Prev, Enter, Back)
        │  from encoder rotation + button press/long-press
```

In Embassy, these layers map to async tasks connected by channels:

```
encoder_task ──┐
               ├── InputEvent Channel ──→ ui_task (owns NavState + Config + Display)
button_task  ──┘                              │
                                              ├── Config Channel ──→ modulator_task
                                              └── BPM Channel   ──→ modulator_task
```

The state machine is the glue. Input events flow in, the state machine decides whether to change navigation or mutate domain data (or both), and the renderer just draws whatever the current state is. No callbacks, no event listeners, no observers — just data in, data out.