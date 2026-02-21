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
use embassy_rp::gpio::{Input, Pull};
use {defmt_rtt as _, panic_probe as _};

mod display;
mod modulator;
mod phasor;
mod usb;
mod encoder;


static BPM_CHANNEL: Channel<CriticalSectionRawMutex, f32, 2> = Channel::new();

// USB_TX carries fixed-size modulator output packets to the USB write task.
// Channel depth of 4 absorbs timing jitter between modulator and USB polling.
static USB_TX: Channel<CriticalSectionRawMutex, [u8; modulator::PACKET_SIZE], 4> = Channel::new();

static INPUT_EVENTS: Channel<CriticalSectionRawMutex, encoder::InputEvent, 4> = Channel::new();

// Timing-critical task: ticks the phasor at 1kHz, computes and sends modulator output at 100Hz.
// Owns the phasor directly (no mutex) so nothing can delay the tick.
#[embassy_executor::task]
async fn modulator_task() {
    //let mut bpm_sub = BPM_BUS.subscriber().unwrap();
    let usb_tx = USB_TX.sender();

    let mut phasor = phasor::PhasorBank::new(120.0, 1000.0);
    let config = modulator::ModulatorConfig::default();
    let engine = modulator::ModulatorEngine;

    // Ticker uses absolute timestamps: next wake = start + N * 1000us.
    // Computation time doesn't cause drift.
    let mut ticker = Ticker::every(Duration::from_micros(1000));
    let mut tick_count: u32 = 0;

    loop {
        ticker.next().await;

        // Non-blocking check: if USB received a new BPM, apply it
        if let Ok(new_bpm) = BPM_CHANNEL.try_receive() {
            phasor.set_bpm(new_bpm);
            info!("BPM updated to {}", new_bpm);
        }

        // Advance all 4 phase accumulators (one per beat multiplier)
        phasor.tick();
        tick_count += 1;

        // Every 10th tick (100Hz): apply waveshapes and send result over USB
        if tick_count % 10 == 0 {
            let packet = engine.compute_bytes(&phasor, &config);
            // try_send is non-blocking; drops the packet if the channel is full
            let _ = usb_tx.try_send(packet);
        }
    }
}

// // Redraws at most 10Hz. Collects the latest state each frame, skips draw if nothing changed.
// #[embassy_executor::task]
// async fn display_task(i2c: i2c::I2c<'static, I2C0, i2c::Blocking>) {
//     let mut disp = display::Display::new(i2c);
//     //let mut bpm_sub = BPM_BUS.subscriber().unwrap();
//     let mut ticker = Ticker::every(Duration::from_millis(100)); // 10Hz

//     let mut current_bpm: f32 = 120.0;
//     let mut dirty = true;

//     loop {
//         ticker.next().await;

//         // Drain all pending BPM updates, keep only the latest
//         while let Some(bpm) = bpm_sub.try_next_message_pure() {
//             current_bpm = bpm;
//             dirty = true;
//         }

//         if dirty {
//             disp.draw_main(current_bpm);
//             dirty = false;
//         }
//     }
// }

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Encoder pins - internal pull-ups, active low
    let pin_a = Input::new(p.PIN_14, Pull::Up);  // CLK
    let pin_b = Input::new(p.PIN_15, Pull::Up);  // DT
    let button = Input::new(p.PIN_18, Pull::Up);  // Push button

    let mut i2c_config = i2c::Config::default();
    i2c_config.frequency = 400_000;
    let i2c = i2c::I2c::new_blocking(p.I2C0, p.PIN_17, p.PIN_16, i2c_config);

    // Start the USB task (handles enumeration, TX from USB_TX channel, RX publishes to BPM_BUS)
    usb::init(p.USB, USB_TX.receiver(), spawner);

    encoder::init_encoder(spawner, button, pin_a, pin_b);

    spawner.spawn(modulator_task()).unwrap();
    //spawner.spawn(display_task(i2c)).unwrap();

    let mut nav = display::NavState::Browse { index: 0 };
    let mut config = modulator::ModulatorConfig::default();
    let mut bpm: u16 = 120;

    let mut disp = display::Display::new(i2c);
    disp.draw_main(bpm as f32);

    loop {
        let event = INPUT_EVENTS.receive().await;
        nav = nav.handle(event, &mut config, &mut bpm);
        let _ = BPM_CHANNEL.try_send(bpm as f32);
        disp.draw_main(bpm as f32);
        info!("nav bpm: {}", bpm);
    }
}
