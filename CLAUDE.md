# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A clocked modulation source for music/audio applications, running on a Raspberry Pi Pico 2 (RP2350, Cortex-M33). Generates four synchronized LFO outputs at different beat multipliers with configurable waveforms, driven by a 1kHz timer interrupt.

## Programming Style

- You adhere to the software craftsmanship rules
- Make everything simple and easy to understand
- Give functions meaningful names


## Architecture

```
main.rs → Hardware init, 1kHz timer interrupt, shared state
  └── modulator.rs → Modulator (4 ModSlots, each combining a Multiplier + Waveshape)
        └── phasor.rs → PhasorBank (4 phase accumulators at different beat divisions)
```

**Data flow per tick:** Timer ISR → `modulator.tick()` → `bank.tick()` advances all phases → `get_all_outputs()` applies waveshapes to phases → 4 normalized f32 values [0.0–1.0].

**Shared state pattern:** `Mutex<RefCell<Option<SharedState>>>` with `cortex_m::interrupt::free` for ISR-safe access. Use `borrow_mut()` in the ISR, `borrow()` (read-only) in the main loop.

**Timer precision:** Uses `schedule_at()` with absolute timestamps to avoid jitter accumulation. The ISR tracks `next_fire` and advances it by `TICK_INTERVAL` each tick rather than scheduling relative to "now".

## Key Types

- `Multiplier` — Beat division enum (D4=bar, D2=half, X1=beat, X2=eighth). Has `ALL` const array, `factor()`, and `index()`.
- `PhasorBank` — Owns `[f32; N]` phase array, ticked at 1kHz. Phase values wrap at 1.0.
- `Waveshape` — Sin/Tri/Squ/Saw enum. `compute_from_phasor(phase) -> f32`.
- `ModSlot` — Pairs a `Multiplier` with a `Waveshape` to produce one output.
- `Modulator` — Facade holding `PhasorBank` + `[ModSlot; 4]`.

## Embedded Rust Notes

- `#![no_std]`, `#![no_main]` — bare metal, no standard library
- Rust edition 2024 requires `#[unsafe(link_section = "...")]` (not just `#[link_section]`)
- `IMAGE_DEF` boot block is required for RP2350 Boot ROM to execute firmware
- `#[hal::pac::interrupt]` proc macro must be imported via `use hal::pac::interrupt;` then used as `#[interrupt]` — nested attribute paths don't resolve
- Debug logging via `defmt` + RTT; structs need `defmt::Format` (not `core::fmt::Debug`) for `defmt::info!`
- Math uses `libm::sinf` (no std math available)
