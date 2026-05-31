# CLAUDE.md

## Project Overview

A clocked modulation source for music/audio, running on a Raspberry Pi Pico 2 (RP2350, Cortex-M33). Generates four synchronized LFO outputs at different beat multipliers with configurable waveforms. Outputs are streamed to the host as **14-bit USB MIDI CC** messages (consumed by TouchDesigner or any MIDI host) and mirrored to four PWM-driven LED indicators.

This is a learning project. The goal is to practice software craftsmanship in embedded Rust.

## Programming Style

- Write clean, readable code — clarity over cleverness
- Give functions and types meaningful, descriptive names
- Keep functions small and focused on a single responsibility
- Prefer explicit match arms over generic fallbacks
- Use `next()`/`prev()` cycling methods on enums rather than index arithmetic
- Explain your reasoning when suggesting changes, so I can learn from it

## Build and Run

- Target: `thumbv8m.main-none-eabihf` (Cortex-M33, hard float)
- Build: `cargo build`
- Flash via debug probe: `cargo run --release` (uses `probe-rs run --chip RP235x --protocol swd`)
- Flash via bootsel: switch runner in `.cargo/config.toml` to `picotool`
- Logging: `defmt` over RTT, log level set via `DEFMT_LOG=debug` in `.cargo/config.toml`

## Architecture

Dual-core embassy async architecture. Core 0 handles time-critical tasks. Core 1 isolates slow blocking I2C display writes. To avoid cross-core channel contention (which previously caused output lag), live playback values — BPM, speed, playback state — are shared as plain atomics rather than channels.

```
Core 0 (embassy executor):
  input.rs     → GPIO: 2 encoders + 6 buttons, one task each → InputEvent channel
  nav.rs       → State machine: (NavState, InputEvent) → NavState + side effects
  tap_tempo.rs → TapTempo: averages tap intervals → BPM
  main.rs      → input_task: runs nav.handle(), updates atomics/channels/display
               → modulator_task: 250Hz ticker, drives phasor, packs MIDI, feeds LEDs
  usb.rs       → USB MIDI (MidiClass): sends a frame whenever USB_TX is signalled
  led.rs       → 2 PWM slices (4 channels) → LED brightness, gamma 2.2

Core 1 (separate embassy executor):
  display.rs   → screen layout / rendering via embedded-graphics
  sh1106.rs    → SH1106 OLED (128x64) driver, double-buffered async I2C
```

### Inter-task Communication

Live values use atomics (no channel contention); structured/edited data uses channels/signals.

Atomics (`main.rs`), read each tick by `modulator_task`:
- `CURRENT_BPM: AtomicU16`, `CURRENT_SPEED: AtomicU8` (a `GlobalSpeed`), `PLAYBACK_STATE: AtomicBool`

Channels / signals (`embassy_sync`):
- `INPUT_EVENTS` — buttons/encoders → input_task (`ThreadModeRawMutex`)
- `CONFIG_CHANNEL` — input_task → modulator_task (only sent when config changes)
- `RESET_CHANNEL` — input_task → modulator_task (bar reset)
- `LED_VALUES` — modulator_task → led_task (~62.5Hz)
- `USB_TX` — `Signal`, modulator_task → USB writer (latest MIDI frame, 125Hz)
- `DISPLAY_UPDATE` — input_task (Core 0) → display_task (Core 1, cross-core, `CriticalSectionRawMutex`)

### Data Flow Per Tick

`Ticker` at 250Hz (`TICK_RATE`) → read atomics, apply BPM/speed/reset changes → `PhasorBank::update(elapsed)` recomputes all phases from absolute elapsed time → every 2nd tick (`USB_SEND_EVERY`, 125Hz): `ModulatorEngine::compute()` → optional per-slot EMA smoothing (`SMOOTH_ALPHA`) → `pack_midi_bytes()` builds the MIDI frame → `USB_TX.signal()`. Every 4th tick (`LED_SEND_EVERY`, ~62.5Hz) the raw outputs are pushed to `LED_VALUES`.

### MIDI Output Format

Each of the 4 outputs (0.0–1.0) becomes a **14-bit MIDI CC** value (0–16383) split across two CCs on channel 1: CC `i` carries the MSB, CC `i+32` the LSB. The frame is 8 USB-MIDI packets × 4 bytes = `MIDI_FRAME_SIZE` (32) bytes, each packet `0x0B 0xB0 <cc> <value>`.

## Key Types

- `Multiplier` (`phasor.rs`) — cycle length: D8 (8 bars) · D4 (4 bars) · D2 (2 bars) · X1 (1 bar) · X2 (2 beats) · X4 (1 beat). 6 variants; `factor()` is cycles-per-beat (0.03125 … 1.0)
- `GlobalSpeed` (`phasor.rs`) — global rate scaler: Quarter · Half · X1 · Double · Quad (0.25× … 4.0×); `next()`/`prev()` clamp at the ends; round-trips through `to_u8`/`from_u8` for the atomic
- `PhasorBank` (`phasor.rs`) — time-based, not an accumulator: holds `[f32; 6]` phases (one per `Multiplier`) and recomputes them from absolute elapsed seconds, so tick jitter has zero effect. Carries phase over on BPM/speed change via `beat_offset`; re-anchors every `BEAT_WRAP` (32) beats
- `Waveshape` (`modulator.rs`) — Sin (256-entry LUT, linearly interpolated), Tri, Squ, Saw, Con (constant 1.0), Noi ("NOI" — 1D gradient Perlin noise, a pure function of phase so it repeats seamlessly each cycle; driven by the slot's `noise_seed`/`noise_freq`). All output [0.0, 1.0]
- `ModSlot` (`modulator.rs`) — Multiplier + Waveshape + min/max range + `smooth: bool` + `noise_seed`/`noise_freq` (NOI params; hardcoded defaults, set via the `with_noise()` builder) → one output channel
- `ModulatorConfig` — `{ slots: [ModSlot; 4] }`, sent via `CONFIG_CHANNEL` when edited
- `ModulatorEngine` — stateless; `compute()` produces 4 outputs, `pack_midi_bytes()` builds the 14-bit CC frame
- `NavState` (`nav.rs`) — Overview | TapMode | ModEditWave { slot, draft } | ModEditRange { slot, draft }. `handle()` is a pure state machine matching on `(state, event)`; edits mutate a `draft` ModSlot
- `SlotId` (`nav.rs`) — A | B | C | D, the four modulator slots
- `PlaybackState` (`nav.rs`) — Playing | Paused
- `TapTempo` (`tap_tempo.rs`) — ring buffer of tap intervals; averages once enough taps land within the timeout window
- `InputEvent` (`input.rs`) — Enc1Rotate(i8) | Enc2Rotate(i8) | B1Press–B6Press

## Roadmap (v2 UI)

See `docs/modulator_state_ui_guide_v2.md` for the full spec.

Done since the v2 spec was written:
- ENC2 in nav: cycles multiplier on the Wave page, adjusts max on the Range page; on Overview it cycles `GlobalSpeed`
- Tap tempo (`tap_tempo.rs`, driven by B3Press in TapMode)
- Playback pause/resume (B5/B6 in TapMode) and bar reset via `RESET_CHANNEL`
- Per-slot value smoothing (`smooth` flag, toggled with B5 on the Range page)

Remaining work:
- Unified `ModEdit { slot, page: EditPage, draft }` state (still split into ModEditWave/ModEditRange)
- Beat indicator on Overview (would need a beat-tick signal from the modulator)

## Embedded Rust Notes

- `#![no_std]`, `#![no_main]` — bare metal, no standard library
- Rust edition 2024 requires `#[unsafe(link_section = "...")]`
- RP2350 boot block is provided by `embassy-rp`'s `rp235xa` + `binary-info` features (no hand-written `IMAGE_DEF`)
- Debug logging via `defmt` + RTT; structs need `defmt::Format` (not `core::fmt::Debug`)
- Sin waveshape uses a 256-entry LUT with linear interpolation — avoids trig in hot paths
- Float math off the hot path (LED gamma, etc.) uses `micromath::F32Ext`
- Embassy async runtime: use `Ticker`, `Timer`, `Channel`/`Signal` for timing and communication; prefer atomics for high-rate shared scalars to avoid channel contention
- The display task holds two 1024-byte framebuffers (current + previous for diffing), so Core 1 needs a 32KB stack
- Each core has its own `embassy_executor::Executor` initialized via `StaticCell`. The I2C peripheral is constructed inside the Core 1 closure so `I2C0_IRQ` binds to Core 1's NVIC
