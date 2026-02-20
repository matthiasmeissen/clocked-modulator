#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::i2c;
use embassy_rp::peripherals::I2C0;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::pubsub::PubSubChannel;
use embassy_time::{Duration, Instant, Timer,  Ticker};
use embassy_rp::gpio::{Input, Level, Pull};
use {defmt_rtt as _, panic_probe as _};

mod display;
mod modulator;
mod phasor;
mod usb;

// Shared communication channels (const-constructed, no StaticCell needed)
//
// BPM_BUS broadcasts BPM changes to multiple subscribers.
// PubSubChannel<Mutex, Type, capacity, max_subscribers, max_publishers>
// - USB task publishes new BPM values
// - Modulator task and display task each subscribe independently
static BPM_BUS: PubSubChannel<CriticalSectionRawMutex, f32, 2, 2, 1> = PubSubChannel::new();

// USB_TX carries fixed-size modulator output packets to the USB write task.
// Channel depth of 4 absorbs timing jitter between modulator and USB polling.
static USB_TX: Channel<CriticalSectionRawMutex, [u8; modulator::PACKET_SIZE], 4> = Channel::new();

// Timing-critical task: ticks the phasor at 1kHz, computes and sends modulator output at 100Hz.
// Owns the phasor directly (no mutex) so nothing can delay the tick.
#[embassy_executor::task]
async fn modulator_task() {
    let mut bpm_sub = BPM_BUS.subscriber().unwrap();
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
        if let Some(bpm) = bpm_sub.try_next_message_pure() {
            phasor.set_bpm(bpm);
            info!("BPM updated to {}", bpm);
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

// Redraws at most 10Hz. Collects the latest state each frame, skips draw if nothing changed.
#[embassy_executor::task]
async fn display_task(i2c: i2c::I2c<'static, I2C0, i2c::Blocking>) {
    let mut disp = display::Display::new(i2c);
    let mut bpm_sub = BPM_BUS.subscriber().unwrap();
    let mut ticker = Ticker::every(Duration::from_millis(100)); // 10Hz

    let mut current_bpm: f32 = 120.0;
    let mut dirty = true;

    loop {
        ticker.next().await;

        // Drain all pending BPM updates, keep only the latest
        while let Some(bpm) = bpm_sub.try_next_message_pure() {
            current_bpm = bpm;
            dirty = true;
        }

        if dirty {
            disp.draw_main(current_bpm);
            dirty = false;
        }
    }
}

#[embassy_executor::task]
async fn button_task(mut button: Input<'static>) {
    let debounce = Duration::from_millis(50);

    loop {
        // Wait for press (pull-up → low = pressed)
        button.wait_for_low().await;
        Timer::after(debounce).await;

        if button.is_low() {
            let press_start = Instant::now();
            info!("Button pressed!");

            // Wait for release
            button.wait_for_high().await;
            Timer::after(debounce).await;

            let held_ms = press_start.elapsed().as_millis();
            info!("Button released (held {}ms)", held_ms);
        }
    }
}

#[embassy_executor::task]
async fn encoder_task(mut pin_a: Input<'static>, pin_b: Input<'static>) {
    let mut position: i32 = 0;
    let mut last_a = pin_a.is_low();

    loop {
        // Wait for any edge on pin A
        if last_a {
            pin_a.wait_for_high().await;
        } else {
            pin_a.wait_for_low().await;
        }

        let a = pin_a.is_low();
        let b = pin_b.is_low();

        // Determine direction: if A and B differ → CW, same → CCW
        if a != last_a {
            if a != b {
                position += 1;
            } else {
                position -= 1;
            }
            info!("Position: {}", position);
        }

        last_a = a;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    info!("Rotary encoder test started");

    // Encoder pins - internal pull-ups, active low
    let pin_a = Input::new(p.PIN_14, Pull::Up);  // CLK
    let pin_b = Input::new(p.PIN_15, Pull::Up);  // DT
    let button = Input::new(p.PIN_18, Pull::Up);  // Push button

    spawner.must_spawn(encoder_task(pin_a, pin_b));
    spawner.must_spawn(button_task(button));

    // let mut i2c_config = i2c::Config::default();
    // i2c_config.frequency = 400_000;
    // let i2c = i2c::I2c::new_blocking(p.I2C0, p.PIN_17, p.PIN_16, i2c_config);

    // // Start the USB task (handles enumeration, TX from USB_TX channel, RX publishes to BPM_BUS)
    // usb::init(p.USB, BPM_BUS.publisher().unwrap(), USB_TX.receiver(), spawner);

    // spawner.spawn(modulator_task()).unwrap();
    // spawner.spawn(display_task(i2c)).unwrap();
}
