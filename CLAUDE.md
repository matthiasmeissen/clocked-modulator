# CLAUDE.md

## Project Overview

A clocked modulation source for music/audio, running on a Raspberry Pi Pico 2 (RP2350, Cortex-M33). Generates four synchronized LFO outputs at different beat multipliers with configurable waveforms. Outputs are streamed over USB CDC serial to TouchDesigner.

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
- Flash via debug probe: `cargo run` (uses `probe-rs run --chip RP235x --protocol swd`)
- Flash via bootsel: switch runner in `.cargo/config.toml` to `picotool`
- Logging: `defmt` over RTT, log level set via `DEFMT_LOG=debug` in `.cargo/config.toml`

## Architecture

Dual-core embassy async architecture. Core 0 handles time-critical tasks. Core 1 isolates slow blocking I2C display writes.

```
Core 0 (embassy executor):
  input.rs    → GPIO polling: 2 encoders + 6 buttons → InputEvent channel
  nav.rs      → State machine: InputEvent → NavState transitions + side effects
  main.rs     → input_task: fans out bpm/config/display updates via channels
              → modulator_task: 1kHz ticker, advances phasor, computes outputs
  usb.rs      → CDC-ACM serial: streams 18-byte packets to host at 125Hz

Core 1 (separate embassy executor):
  display.rs  → SH1106 OLED (128x64) rendering via blocking I2C
```

### Inter-task Communication

All via `embassy_sync::Channel` (lock-free, critical-section-based):

- `INPUT_EVENTS` — buttons/encoders → input_task (mpsc)
- `BPM_CHANNEL` — input_task → modulator_task
- `CONFIG_CHANNEL` — input_task → modulator_task (only sent on change)
- `USB_TX` — modulator_task → USB writer (125Hz packets)
- `DISPLAY_UPDATE` — input_task (Core 0) → display_task (Core 1, cross-core)

### Data Flow Per Tick

`Ticker` at 1kHz → `PhasorBank::tick()` advances 4 phase accumulators → `ModSlot::output()` applies waveshape + range mapping → 4 normalized f32 values → packed into 18-byte USB packet (0xAA 0xBB header + 4x f32 LE) every 8th tick.

## Key Types

- `Multiplier` — Beat division: D4 (bar, 0.25x), D2 (half, 0.5x), X1 (beat, 1.0x), X2 (eighth, 2.0x)
- `PhasorBank` — 4 phase accumulators `[f32; 4]`, ticked at 1kHz, phases wrap at 1.0
- `Waveshape` — Sin (256-entry LUT), Tri, Squ, Saw. All output [0.0, 1.0]
- `ModSlot` — Multiplier + Waveshape + min/max range → one output channel
- `ModulatorConfig` — `[ModSlot; 4]`, sent via channel when edited
- `ModulatorEngine` — Stateless; computes all 4 outputs from PhasorBank + ModulatorConfig
- `NavState` — Overview | TapMode | ModEditWave | ModEditRange. `handle()` is a pure state machine
- `InputEvent` — Enc1Rotate(i8) | Enc2Rotate(i8) | B1Press–B6Press

## Roadmap (v2 UI)

See `docs/modulator_state_ui_guide_v2.md` for the full spec. Key remaining work:

- Unified `ModEdit { slot, page: EditPage, draft }` state (currently split into ModEditWave/ModEditRange)
- ENC2 support in nav: cycle multiplier (Waves page), adjust min (Range page)
- Tap tempo logic in input_task (B3Press interval timing)
- Playback pause/resume (B5/B6 in TapMode)
- Beat indicator on Overview (requires BEAT_TICK channel from modulator)
- Bar reset via B4Press / Enc2Rotate in Overview

## Embedded Rust Notes

- `#![no_std]`, `#![no_main]` — bare metal, no standard library
- Rust edition 2024 requires `#[unsafe(link_section = "...")]`
- `IMAGE_DEF` boot block required for RP2350 Boot ROM
- Debug logging via `defmt` + RTT; structs need `defmt::Format` (not `core::fmt::Debug`)
- Sin waveshape uses a 256-entry LUT — avoids float math in hot paths
- Embassy async runtime: use `Ticker`, `Timer`, `Channel` for timing and communication
- Each core has its own `embassy_executor::Executor` initialized via `StaticCell`
