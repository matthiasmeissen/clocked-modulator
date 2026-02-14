#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::{Executor, task};
use embassy_rp::peripherals::I2C0;
use embassy_rp::i2c;
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::pixelcolor::BinaryColor;
use sh1106::{prelude::*, Builder};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod phasor;
mod modulator;
mod display;

// Core 1 stack - must be static
static mut CORE1_STACK: Stack<16384> = Stack::new();

// Executors must be static to live forever
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

// Shared state across cores
static SHARED_VALUES: Mutex<CriticalSectionRawMutex, [f32; 4]> = Mutex::new([0.0; 4]);

pub type DisplayType = GraphicsMode<I2cInterface<i2c::I2c<'static, I2C0, i2c::Blocking>>>;

// --- CORE 1: DISPLAY ---
#[task]
async fn display_task(i2c: i2c::I2c<'static, I2C0, i2c::Blocking>) {
    let mut display: GraphicsMode<_> = Builder::new().connect_i2c(i2c).into();

    // Fix 1: sh1106::Error doesn't implement defmt::Format, so don't try to print it
    if display.init().is_err() {
        error!("Display init failed");
        return;
    }
    info!("Core 1: Display initialized");

    let mut ticker = Ticker::every(Duration::from_hz(30));

    loop {
        ticker.next().await;
        
        let values = *SHARED_VALUES.lock().await;

        display::draw_screen(&mut display, values);
    }
}

// --- CORE 0: MODULATOR ---
#[task]
async fn modulator_task() {
    let mut phasor = phasor::PhasorBank::new(120.0, 1000.0);
    let engine = modulator::ModulatorEngine;
    let config = modulator::ModulatorConfig::default();
    
    let mut ticker = Ticker::every(Duration::from_micros(1000));
    let mut count = 0u32;
    
    info!("Core 0: Modulator started");

    loop {
        ticker.next().await;
        phasor.tick();
        count += 1;
        
        if count % 10 == 0 {
            let values = engine.compute(phasor, &config);
            *SHARED_VALUES.lock().await = values;
            info!("Core 0: {:?}", values);
        }
    }
}

fn core1_main(i2c: i2c::I2c<'static, I2C0, i2c::Blocking>) -> ! {
    let executor1 = EXECUTOR1.init(Executor::new());
    executor1.run(|spawner| {
        let _ = spawner.spawn(display_task(i2c));
    });
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());

    let sda = p.PIN_16;
    let scl = p.PIN_17;
    let i2c_config = i2c::Config::default();
    let i2c = i2c::I2c::new_blocking(p.I2C0, scl, sda, i2c_config);

    // Launch Core 1
    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || core1_main(i2c),
    );

    // Fix 3: Use static EXECUTOR0, not a local StaticCell
    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        let _ = spawner.spawn(modulator_task());
    });
}