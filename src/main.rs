#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Ticker};
use embassy_rp::peripherals::I2C0;
use embassy_rp::i2c;
use heapless::Vec;
use static_cell::StaticCell;
use sh1106::{prelude::*, Builder};
use {defmt_rtt as _, panic_probe as _};

mod modulator;
mod phasor;
mod usb;
mod display;

static BPM_SIGNAL: StaticCell<Signal<CriticalSectionRawMutex, f32>> = StaticCell::new();
static DISPLAY_BPM_SIGNAL: StaticCell<Signal<CriticalSectionRawMutex, f32>> = StaticCell::new();

#[embassy_executor::task]
async fn modulator_task(
    bpm_signal: &'static Signal<CriticalSectionRawMutex, f32>,
    usb_tx: embassy_sync::channel::Sender<'static, CriticalSectionRawMutex, Vec<u8, 64>, 4>,
) {
    let mut phasor = phasor::PhasorBank::new(120.0, 1000.0);
    let mod_config = modulator::ModulatorConfig::default();
    let mod_engine = modulator::ModulatorEngine;

    let mut ticker = Ticker::every(Duration::from_micros(1000)); // 1kHz
    let mut tick_count: u32 = 0;

    loop {
        ticker.next().await;
        
        if let Some(bpm) = bpm_signal.try_take() {
            phasor.set_bpm(bpm);
            info!("BPM updated to {}", bpm);
        }
        
        phasor.tick();
        tick_count += 1;
        
        if tick_count % 10 == 0 {
            let bytes = mod_engine.compute_bytes(phasor, &mod_config);
            
            let mut packet: Vec<u8, 64> = Vec::new();
            let _ = packet.extend_from_slice(&bytes);
            let _ = usb_tx.try_send(packet);
        }
        
        //let start = Instant::now();
        // if tick_count % 1000 == 0 {
        //     let elapsed_us = start.elapsed().as_micros();
        //     info!("Tick compute: {}us / 1000us budget", elapsed_us);
        // }
    }
}

#[embassy_executor::task]
async fn display_task(
    i2c: i2c::I2c<'static, I2C0, i2c::Blocking>,
    bpm_signal: &'static Signal<CriticalSectionRawMutex, f32>,
) {
    let mut display: GraphicsMode<_> = Builder::new().connect_i2c(i2c).into();

    if display.init().is_err() {
        error!("Display init failed");
        return;
    }
    info!("Display initialized");

    display::draw_bpm_screen(&mut display, 120.0);

    loop {
        let bpm = bpm_signal.wait().await;
        display::draw_bpm_screen(&mut display, bpm);
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let sda = p.PIN_16;
    let scl = p.PIN_17;
    let i2c_config = i2c::Config::default();
    let i2c = i2c::I2c::new_blocking(p.I2C0, scl, sda, i2c_config);

    let bpm_signal = BPM_SIGNAL.init(Signal::new());
    let display_bpm_signal = DISPLAY_BPM_SIGNAL.init(Signal::new());
    let usb_tx = usb::init(p.USB, bpm_signal, display_bpm_signal, spawner);

    spawner.spawn(modulator_task(bpm_signal, usb_tx)).ok();
    spawner.spawn(display_task(i2c, display_bpm_signal)).ok();
}
