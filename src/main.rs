
#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::{Spawner, task};
use embassy_rp::gpio;
use embassy_rp::peripherals::I2C0;
use embassy_rp::{bind_interrupts, i2c};
use embassy_time::{Duration, Ticker, Timer};

use embedded_graphics::Drawable;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::pixelcolor::BinaryColor;

use embedded_graphics::prelude::Point;
use embedded_graphics::text::{Baseline, Text};
use gpio::{Level, Output};
use {defmt_rtt as _, panic_probe as _};

use sh1106::{prelude::*, Builder, interface::I2cInterface};

mod phasor;
mod modulator;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => i2c::InterruptHandler<I2C0>;
});

#[task]
async fn display_task(i2c_bus: i2c::I2c<'static, I2C0, i2c::Async>) {
    let mut display: GraphicsMode<_> = Builder::new().connect_i2c(i2c_bus).into();

    display
        .init()
        .expect("failed to initialize the display");

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    Text::with_baseline("Hello, Rust!", Point::new(0, 16), text_style, Baseline::Top)
        .draw(&mut display)
        .expect("failed to draw text to display");

    display
        .flush()
        .expect("failed to flush data to display");
}

#[task]
async fn modulator_task() {
    let mut phasor = phasor::PhasorBank::new(120.0, 1000.0);
    let engine = modulator::ModulatorEngine;
    let config = modulator::ModulatorConfig::default();
    
    let mut ticker = Ticker::every(Duration::from_micros(1000)); // 1kHz base
    
    let mut count = 0u32;
    
    loop {
        ticker.next().await;
        
        // 1kHz: Tick phasor
        phasor.tick();
        count += 1;
        
        // 100Hz: Every 10 ticks
        if count % 10 == 0 {
            let values = engine.compute(phasor, &config);
            let bytes = engine.compute_bytes(phasor, &config);

            info!("{}", values);
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_25, Level::Low);

    let sda = p.PIN_16;
    let scl = p.PIN_17;

    let mut i2c_config = i2c::Config::default();
    i2c_config.frequency = 400_000;

    let i2c_bus = i2c::I2c::new_async(p.I2C0, scl, sda, Irqs, i2c_config);

    let _ = spawner.spawn(modulator_task());
    let _ = spawner.spawn(display_task(i2c_bus));

    loop {
        info!("led on!");
        led.set_high();
        Timer::after_secs(1).await;

        info!("led off!");
        led.set_low();
        Timer::after_secs(1).await;
    }
}