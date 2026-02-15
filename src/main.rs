#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::i2c;
use embassy_rp::peripherals::I2C0;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::pubsub::PubSubChannel;
use embassy_time::{Duration, Ticker};
use {defmt_rtt as _, panic_probe as _};

mod display;
mod modulator;
mod phasor;
mod usb;

// BPM broadcast: 1 publisher (USB), 2 subscribers (modulator + display), 2 slots
static BPM_BUS: PubSubChannel<CriticalSectionRawMutex, f32, 2, 2, 1> = PubSubChannel::new();

// Fixed-size USB TX channel
static USB_TX: Channel<CriticalSectionRawMutex, [u8; modulator::PACKET_SIZE], 4> = Channel::new();

#[embassy_executor::task]
async fn modulator_task() {
    let mut bpm_sub = BPM_BUS.subscriber().unwrap();
    let usb_tx = USB_TX.sender();

    let mut phasor = phasor::PhasorBank::new(120.0, 1000.0);
    let config = modulator::ModulatorConfig::default();
    let engine = modulator::ModulatorEngine;

    let mut ticker = Ticker::every(Duration::from_micros(1000)); // 1kHz
    let mut tick_count: u32 = 0;

    loop {
        ticker.next().await;

        if let Some(bpm) = bpm_sub.try_next_message_pure() {
            phasor.set_bpm(bpm);
            info!("BPM updated to {}", bpm);
        }

        phasor.tick();
        tick_count += 1;

        if tick_count % 10 == 0 {
            let packet = engine.compute_bytes(&phasor, &config);
            let _ = usb_tx.try_send(packet);
        }
    }
}

#[embassy_executor::task]
async fn display_task(i2c: i2c::I2c<'static, I2C0, i2c::Blocking>) {
    let mut disp = display::Display::new(i2c);
    let mut bpm_sub = BPM_BUS.subscriber().unwrap();

    disp.draw_bpm(120.0);

    loop {
        let bpm = bpm_sub.next_message_pure().await;
        disp.draw_bpm(bpm);
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let i2c = i2c::I2c::new_blocking(p.I2C0, p.PIN_17, p.PIN_16, i2c::Config::default());

    usb::init(p.USB, BPM_BUS.publisher().unwrap(), USB_TX.receiver(), spawner);

    spawner.spawn(modulator_task()).unwrap();
    spawner.spawn(display_task(i2c)).unwrap();
}
