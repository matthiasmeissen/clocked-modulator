#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Ticker};
use heapless::Vec;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod modulator;
mod phasor;
mod usb;

static SHARED_PHASOR: StaticCell<Mutex<CriticalSectionRawMutex, phasor::PhasorBank>> =
    StaticCell::new();
static BPM_SIGNAL: StaticCell<Signal<CriticalSectionRawMutex, f32>> = StaticCell::new();

#[embassy_executor::task]
async fn phasor_task(
    phasor: &'static Mutex<CriticalSectionRawMutex, phasor::PhasorBank>,
    bpm_signal: &'static Signal<CriticalSectionRawMutex, f32>,
) {
    let mut ticker = Ticker::every(Duration::from_micros(1000));

    loop {
        ticker.next().await;

        let mut p = phasor.lock().await;

        if let Some(bpm) = bpm_signal.try_take() {
            p.set_bpm(bpm);
            info!("BPM updated to {}", bpm);
        }

        p.tick();
    }
}

#[embassy_executor::task]
async fn modulator_task(
    phasor: &'static Mutex<CriticalSectionRawMutex, phasor::PhasorBank>,
    usb_tx: embassy_sync::channel::Sender<'static, CriticalSectionRawMutex, Vec<u8, 64>, 4>,
) {
    let mod_config = modulator::ModulatorConfig::default();
    let mod_engine = modulator::ModulatorEngine;

    let mut ticker = Ticker::every(Duration::from_micros(1000)); // 1kHz

    loop {
        ticker.next().await;

        let phasor_snapshot = {
            let p = phasor.lock().await;
            p.clone()
        };

        let bytes = mod_engine.compute_bytes(phasor_snapshot, &mod_config);

        let mut packet: Vec<u8, 64> = Vec::new();
        let _ = packet.extend_from_slice(&bytes);
        let _ = usb_tx.try_send(packet);
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let phasor = SHARED_PHASOR.init(Mutex::new(phasor::PhasorBank::new(120.0, 1000.0)));
    let bpm_signal = BPM_SIGNAL.init(Signal::new());

    let usb_tx = usb::init(p.USB, bpm_signal, spawner);

    spawner.spawn(phasor_task(phasor, bpm_signal)).ok();
    spawner.spawn(modulator_task(phasor, usb_tx)).ok();
}
