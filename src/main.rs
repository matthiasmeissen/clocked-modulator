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
use {defmt_rtt as _, panic_probe as _};

mod display;
mod modulator;
mod phasor;
mod usb;
mod encoder;

static BPM_CHANNEL: Channel<CriticalSectionRawMutex, f32, 2> = Channel::new();
static USB_TX: Channel<CriticalSectionRawMutex, [u8; modulator::PACKET_SIZE], 8> = Channel::new();
static INPUT_EVENTS: Channel<CriticalSectionRawMutex, encoder::InputEvent, 4> = Channel::new();
static DISPLAY_UPDATE: Channel<CriticalSectionRawMutex, DisplayState, 2> = Channel::new();

static mut CORE1_STACK: Stack<16384> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

#[derive(Clone, Copy)]
struct DisplayState {
    bpm: u16,
    nav: display::NavState,
}

#[embassy_executor::task]
async fn modulator_task() {
    let usb_tx = USB_TX.sender();

    let mut phasor = phasor::PhasorBank::new(120.0, 1000.0);
    let config = modulator::ModulatorConfig::default();
    let engine = modulator::ModulatorEngine;

    let mut ticker = Ticker::every(Duration::from_micros(1000));
    let mut tick_count: u32 = 0;

    loop {
        ticker.next().await;

        if let Ok(new_bpm) = BPM_CHANNEL.try_receive() {
            phasor.set_bpm(new_bpm);
            info!("BPM updated to {}", new_bpm);
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

    let mut state = DisplayState { bpm: 120, nav: display::NavState::Browse { index: 0 } };
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
    let pin_a = Input::new(p.PIN_14, Pull::Up);
    let pin_b = Input::new(p.PIN_15, Pull::Up);
    let button = Input::new(p.PIN_18, Pull::Up);

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
        encoder::init_encoder(spawner, button, pin_a, pin_b);
        spawner.spawn(modulator_task()).unwrap();
        spawner.spawn(input_task()).unwrap();
    });
}

// Handles input events and sends display updates to Core 1.
#[embassy_executor::task]
async fn input_task() {
    let mut nav = display::NavState::Browse { index: 0 };
    let mut config = modulator::ModulatorConfig::default();
    let mut bpm: u16 = 120;

    loop {
        let event = INPUT_EVENTS.receive().await;
        nav = nav.handle(event, &mut config, &mut bpm);
        let _ = BPM_CHANNEL.try_send(bpm as f32);
        let _ = DISPLAY_UPDATE.try_send(DisplayState { bpm, nav });
    }
}
