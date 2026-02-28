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
mod usb;
mod input;
mod nav;

// Channels for inter-task communication
static BPM_CHANNEL: Channel<CriticalSectionRawMutex, f32, 2> = Channel::new();                           // input → modulator (spsc)
static CONFIG_CHANNEL: Channel<CriticalSectionRawMutex, modulator::ModulatorConfig, 2> = Channel::new(); // input → modulator (spsc)
static USB_TX: Channel<CriticalSectionRawMutex, [u8; modulator::PACKET_SIZE], 8> = Channel::new();       // modulator → usb (spsc)
static INPUT_EVENTS: Channel<CriticalSectionRawMutex, input::InputEvent, 4> = Channel::new();            // buttons + encoder → input (mpsc)
static DISPLAY_UPDATE: Channel<CriticalSectionRawMutex, DisplayState, 2> = Channel::new();               // input → display on Core 1 (spsc)

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

        phasor.tick();
        tick_count += 1;

        if tick_count % 8 == 0 {
            let packet = engine.compute_bytes(&phasor, &config);
            let _ = usb_tx.try_send(packet);
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

    // Encoder pins (Core 0)
    let e1_clk = Input::new(p.PIN_14, Pull::Up);
    let e1_dta = Input::new(p.PIN_15, Pull::Up);
    let b1 = Input::new(p.PIN_18, Pull::Up);
    let b2 = Input::new(p.PIN_10, Pull::Up);

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
        input::init_encoder(spawner, e1_clk, e1_dta, b1, b2);
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

    loop {
        let event = INPUT_EVENTS.receive().await;
        let prev_config = config;
        nav = nav.handle(event, &mut bpm, &mut config, &mut playback, &mut reset_bar);
        let _ = BPM_CHANNEL.try_send(bpm as f32);
        // Only send config, when values have changed
        if prev_config != config {
            let _ = CONFIG_CHANNEL.try_send(config);
        }
        let _ = DISPLAY_UPDATE.try_send(DisplayState { bpm, nav, playback });
    }
}
