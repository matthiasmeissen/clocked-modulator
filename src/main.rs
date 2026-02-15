#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::{Executor, Spawner, task};
use embassy_rp::peripherals::I2C0;
use embassy_rp::i2c;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod phasor;
mod modulator;

static SHARED_PHASOR: StaticCell<Mutex<CriticalSectionRawMutex, phasor::PhasorBank>> = StaticCell::new();
static BPM_SIGNAL: StaticCell<Signal<CriticalSectionRawMutex, f32>> = StaticCell::new();

#[task]
async fn phasor_task(phasor: &'static Mutex<CriticalSectionRawMutex, phasor::PhasorBank>, bpm_signal: &'static Signal<CriticalSectionRawMutex, f32>,) {
    let mut ticker = Ticker::every(Duration::from_micros(1000)); // Duration must match tick_rate from phasor bank: 1kHz
    
    loop {
        ticker.next().await;
        
        let mut p = phasor.lock().await;
        
        if let Some(bpm) = bpm_signal.try_take() {
            p.set_bpm(bpm);
            info!("Phasor BPM updated to {}", bpm);
        }
        
        p.tick();
    }
}

#[task]
async fn modulator_calculation_task(phasor: &'static Mutex<CriticalSectionRawMutex, phasor::PhasorBank>,) {
    let mod_config = modulator::ModulatorConfig::default();
    let mod_engine = modulator::ModulatorEngine;

    let mut ticker = Ticker::every(Duration::from_micros(10_000)); // 100Hz
    
    loop {
        ticker.next().await;
        
        let phasor_snapshot = {
            let p = phasor.lock().await;
            p.clone()
        };
        
        let values = mod_engine.compute(phasor_snapshot, &mod_config);
        
        info!("{}", modulator::Visualizer4(values));
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let phasor = SHARED_PHASOR.init(Mutex::new(phasor::PhasorBank::new(120.0, 1000.0)));
    let bpm_signal = BPM_SIGNAL.init(Signal::new());
    
    let _ = spawner.spawn(phasor_task(phasor, bpm_signal));
    let _ = spawner.spawn(modulator_calculation_task(phasor));
}
