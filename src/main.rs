#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Executor;
use embassy_rp::i2c;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Ticker};
use static_cell::StaticCell;
use crate::nav::PlaybackState;

use {defmt_rtt as _, panic_probe as _};

mod display;
mod modulator;
mod phasor;
mod tap_tempo;
mod usb;
mod input;
mod nav;

// Channels for inter-task communication
static BPM_CHANNEL: Channel<CriticalSectionRawMutex, f32, 2> = Channel::new();                              // input → modulator (spsc)
static CONFIG_CHANNEL: Channel<CriticalSectionRawMutex, modulator::ModulatorConfig, 2> = Channel::new();    // input → modulator (spsc)
static USB_TX: Channel<CriticalSectionRawMutex, [u8; modulator::MIDI_FRAME_SIZE], 8> = Channel::new();     // modulator → usb (spsc)
static INPUT_EVENTS: Channel<CriticalSectionRawMutex, input::InputEvent, 4> = Channel::new();               // buttons + encoder → input (mpsc)
static DISPLAY_UPDATE: Channel<CriticalSectionRawMutex, DisplayState, 2> = Channel::new();                  // input → display on Core 1 (spsc)
static PLAYBACK_CHANNEL: Channel<CriticalSectionRawMutex, PlaybackState, 2> = Channel::new();               // input → modulator (spsc)
static RESET_CHANNEL: Channel<CriticalSectionRawMutex, bool, 2> = Channel::new();                           // input → modulator (spsc, one-shot)

// Each core needs its own stack and executor for independent async runtimes
static mut CORE1_STACK: Stack<16384> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

// Snapshot of UI state sent from Core 0 to Core 1 for rendering
#[derive(Clone, Copy)]
struct DisplayState {
    bpm: u16,
    nav: nav::NavState,
    playback: PlaybackState,
}

// Runs at 1kHz. Advances phasor, computes waveforms, sends USB packets every 8th tick.
#[embassy_executor::task]
async fn modulator_task() {
    let usb_tx = USB_TX.sender();

    let mut playback = PlaybackState::Playing;

    let mut phasor = phasor::PhasorBank::new(120.0, 1000.0);
    let mut config = modulator::ModulatorConfig::default();
    let engine = modulator::ModulatorEngine;

    let mut ticker = Ticker::every(Duration::from_micros(1000));
    let mut tick_count: u32 = 0;

    loop {
        ticker.next().await;

        if let Ok(new_bpm) = BPM_CHANNEL.try_receive() {
            phasor.set_bpm(new_bpm);
            info!("BPM updated to {}", new_bpm);
        }

        if let Ok(new_config) = CONFIG_CHANNEL.try_receive() {
            config = new_config;
        }

        if let Ok(new_playback) = PLAYBACK_CHANNEL.try_receive() {
            playback = new_playback;
        }

        if let Ok(true) = RESET_CHANNEL.try_receive() {
            phasor.reset();
        }

        if playback == PlaybackState::Playing {
            phasor.tick();
        }

        tick_count += 1;

        if tick_count % 8 == 0 {
            let frame = engine.compute_midi_bytes(&phasor, &config);
            let _ = usb_tx.try_send(frame);
        }
    }
}

// Runs on Core 1. Redraws display whenever Core 0 sends new state.
// Blocking I2C writes here can never delay modulator or USB on Core 0.
#[embassy_executor::task]
async fn display_task(i2c: i2c::I2c<'static, embassy_rp::peripherals::I2C0, i2c::Blocking>) {
    let mut disp = display::Display::new(i2c);

    let mut state = DisplayState { bpm: 120, nav: nav::NavState::Overview, playback: PlaybackState::Playing };
    disp.draw_main(state.bpm as f32, &state.nav);

    loop {
        state = DISPLAY_UPDATE.receive().await;
        disp.draw_main(state.bpm as f32, &state.nav);
    }
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());

    // Input pins (Core 0)
    let e1_clk = Input::new(p.PIN_14, Pull::Up);
    let e1_dta = Input::new(p.PIN_15, Pull::Up);
    let e2_clk = Input::new(p.PIN_12, Pull::Up);
    let e2_dta = Input::new(p.PIN_13, Pull::Up);
    let b1 = Input::new(p.PIN_18, Pull::Up);
    let b2 = Input::new(p.PIN_20, Pull::Up);
    let b3 = Input::new(p.PIN_21, Pull::Up);
    let b4 = Input::new(p.PIN_19, Pull::Up);
    let b5 = Input::new(p.PIN_11, Pull::Up);
    let b6 = Input::new(p.PIN_10, Pull::Up);

    // Display I2C — moves to Core 1
    let mut i2c_config = i2c::Config::default();
    i2c_config.frequency = 400_000;
    let i2c = i2c::I2c::new_blocking(p.I2C0, p.PIN_17, p.PIN_16, i2c_config);

    // Core 1: display only — runs its own executor so I2C can't block Core 0
    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| {
                spawner.spawn(display_task(i2c)).unwrap();
            });
        },
    );

    // Core 0: modulator, USB, encoder, button, input handling
    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        usb::init(p.USB, USB_TX.receiver(), spawner);
        input::init_encoder(spawner, e1_clk, e1_dta, e2_clk, e2_dta, b1, b2, b3, b4, b5, b6);
        spawner.spawn(modulator_task()).unwrap();
        spawner.spawn(input_task()).unwrap();
    });
}

// Handles input events, runs the nav state machine, and fans out updates to
// modulator (bpm + config channels) and display (Core 1).
#[embassy_executor::task]
async fn input_task() {
    let mut nav = nav::NavState::Overview;
    let mut config = modulator::ModulatorConfig::default();
    let mut bpm: u16 = 120;
    let mut playback = PlaybackState::Playing;
    let mut reset_bar = false;
    let mut tap_tempo = tap_tempo::TapTempo::new();

    loop {
        let event = INPUT_EVENTS.receive().await;

        // Snapshot state before the state machine runs
        let prev_bpm = bpm;
        let prev_config = config;
        let prev_playback = playback;

        // State machine: process input, may mutate bpm/config/playback/reset_bar
        nav = nav.handle(event, &mut bpm, &mut config, &mut playback, &mut reset_bar);

        // Tap tempo: measure B3 intervals in TapMode
        if matches!(nav, nav::NavState::TapMode) && matches!(event, input::InputEvent::B3Press) {
            if let Some(new_bpm) = tap_tempo.tap() {
                bpm = new_bpm;
            }
        }

        // Fan out changes to modulator task (only send what actually changed)
        if bpm != prev_bpm {
            let _ = BPM_CHANNEL.try_send(bpm as f32);
        }
        if config != prev_config {
            let _ = CONFIG_CHANNEL.try_send(config);
        }
        if playback != prev_playback {
            let _ = PLAYBACK_CHANNEL.try_send(playback);
        }
        if reset_bar {
            let _ = RESET_CHANNEL.try_send(true);
            reset_bar = false;
        }

        // Always update display so UI reflects current state
        let _ = DISPLAY_UPDATE.try_send(DisplayState { bpm, nav, playback });
    }
}
