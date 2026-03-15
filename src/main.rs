#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU16, AtomicBool, Ordering};
use defmt::*;
use embassy_executor::Executor;
use embassy_rp::i2c;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Ticker};
use static_cell::StaticCell;
use crate::modulator::{MIDI_FRAME_SIZE, ModulatorFrame, NUM_MODULATORS};
use crate::nav::PlaybackState;

use {defmt_rtt as _, panic_probe as _};

mod display;
mod modulator;
mod phasor;
mod tap_tempo;
mod usb;
mod input;
mod led;
mod nav;

// Atomic shared state - no channel contention
pub static CURRENT_BPM: AtomicU16 = AtomicU16::new(120);
pub static PLAYBACK_STATE: AtomicBool = AtomicBool::new(true); // true = playing

// Channels for less time-critical communication
static CONFIG_CHANNEL: Channel<CriticalSectionRawMutex, modulator::ModulatorConfig, 2> = Channel::new();
static RESET_CHANNEL: Channel<CriticalSectionRawMutex, bool, 2> = Channel::new();
static INPUT_EVENTS: Channel<CriticalSectionRawMutex, input::InputEvent, 4> = Channel::new();
static DISPLAY_UPDATE: Channel<CriticalSectionRawMutex, DisplayState, 2> = Channel::new();
static LED_VALUES: Channel<CriticalSectionRawMutex, [f32; modulator::NUM_MODULATORS], 2> = Channel::new();
static USB_TX: Channel<CriticalSectionRawMutex, [u8; modulator::MIDI_FRAME_SIZE], 8> = Channel::new();

// Each core needs its own stack and executor
static mut CORE1_STACK: Stack<16384> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

#[derive(Clone, Copy)]
struct DisplayState {
    bpm: u16,
    nav: nav::NavState,
    playback: PlaybackState,
}

// Runs on Core 0 at 250Hz - control rate, not audio rate
const TICK_RATE: f32 = 250.0;
const TICK_INTERVAL_US: u64 = 4000; // 1_000_000 / 250
const USB_SEND_EVERY: u32 = 2;      // 250 / 2 = 125Hz
const LED_SEND_EVERY: u32 = 4;      // 250 / 4 ≈ 62.5Hz

#[embassy_executor::task]
async fn modulator_task() {
    let usb_tx = USB_TX.sender();

    let mut phasor = phasor::PhasorBank::new(120.0, TICK_RATE);
    let mut config = modulator::ModulatorConfig::default();
    let engine = modulator::ModulatorEngine;

    let mut ticker = Ticker::every(Duration::from_micros(TICK_INTERVAL_US));
    let mut tick_count: u32 = 0;

    let mut frame = ModulatorFrame {
        midi_bytes: [0; MIDI_FRAME_SIZE],
        outputs: [0.0; NUM_MODULATORS],
    };
    
    // Cache atomic values to minimize atomic operations
    let mut current_bpm = CURRENT_BPM.load(Ordering::Relaxed);
    let mut playing = PLAYBACK_STATE.load(Ordering::Relaxed);
    
    loop {
        ticker.next().await;
        
        // Read atomics once per tick - very fast
        let new_bpm = CURRENT_BPM.load(Ordering::Relaxed);
        if new_bpm != current_bpm {
            current_bpm = new_bpm;
            phasor.set_bpm(current_bpm as f32);
        }
        
        let new_playing = PLAYBACK_STATE.load(Ordering::Relaxed);
        if new_playing != playing {
            playing = new_playing;
        }
        
        // Check for config updates (less frequent)
        if let Ok(new_config) = CONFIG_CHANNEL.try_receive() {
            config = new_config;
        }
        
        // Check for reset signal
        if let Ok(true) = RESET_CHANNEL.try_receive() {
            phasor.reset();
        }
        
        if playing {
            phasor.tick();
        }
        
        tick_count += 1;
        
        if tick_count % USB_SEND_EVERY == 0 {
            frame = engine.compute_midi_bytes(&phasor, &config);
            let _ = usb_tx.try_send(frame.midi_bytes);
        }

        if tick_count % LED_SEND_EVERY == 0 {
            let _ = LED_VALUES.try_send(frame.outputs);
        }
    }
}

// Runs on Core 1 - handles display updates
#[embassy_executor::task]
async fn display_task(i2c: i2c::I2c<'static, embassy_rp::peripherals::I2C0, i2c::Blocking>) {
    let mut disp = display::Display::new(i2c);
    
    let mut state = DisplayState { 
        bpm: 120, 
        nav: nav::NavState::Overview, 
        playback: PlaybackState::Playing 
    };
    disp.draw_main(state.bpm as f32, &state.nav);
    
    loop {
        state = DISPLAY_UPDATE.receive().await;
        disp.draw_main(state.bpm as f32, &state.nav);
    }
}

// Runs on Core 1 - handles input processing
#[embassy_executor::task]
async fn input_task() {
    let mut nav = nav::NavState::Overview;
    let mut config = modulator::ModulatorConfig::default();
    let mut bpm: u16 = CURRENT_BPM.load(Ordering::Relaxed);
    let mut playback = if PLAYBACK_STATE.load(Ordering::Relaxed) {
        PlaybackState::Playing
    } else {
        PlaybackState::Paused
    };
    let mut reset_bar = false;
    let mut tap_tempo = tap_tempo::TapTempo::new();
    
    loop {
        let event = INPUT_EVENTS.receive().await;
        
        let prev_bpm = bpm;
        let prev_playback = playback;
        
        nav = nav.handle(event, &mut bpm, &mut config, &mut playback, &mut reset_bar);
        
        // Handle tap tempo specially
        if matches!(nav, nav::NavState::TapMode) && matches!(event, input::InputEvent::B3Press) {
            if let Some(new_bpm) = tap_tempo.tap() {
                bpm = new_bpm;
            }
        }
        
        // Update atomic variables (very fast, no blocking)
        if bpm != prev_bpm {
            CURRENT_BPM.store(bpm, Ordering::Relaxed);
        }
        if playback != prev_playback {
            PLAYBACK_STATE.store(
                playback == PlaybackState::Playing, 
                Ordering::Relaxed
            );
        }
        
        // Send config updates (less frequent)
        let _ = CONFIG_CHANNEL.try_send(config);
        
        // Send reset if needed
        if reset_bar {
            let _ = RESET_CHANNEL.try_send(true);
            reset_bar = false;
        }
        
        // Update display
        let _ = DISPLAY_UPDATE.try_send(DisplayState { 
            bpm, 
            nav, 
            playback 
        });
    }
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());
    
    // Input pins - will be used by Core 1
    let e1_clk = Input::new(p.PIN_14, Pull::Up);
    let e1_dta = Input::new(p.PIN_15, Pull::Up);
    let e2_clk = Input::new(p.PIN_12, Pull::Up);
    let e2_dta = Input::new(p.PIN_13, Pull::Up);
    let b1 = Input::new(p.PIN_18, Pull::Up);
    let b2 = Input::new(p.PIN_20, Pull::Up);
    let b3 = Input::new(p.PIN_21, Pull::Up);
    let b4 = Input::new(p.PIN_19, Pull::Up);
    let b5 = Input::new(p.PIN_11, Pull::Up);
    let b6 = Input::new(p.PIN_10, Pull::Up);
    
    // Display I2C - moves to Core 1
    let mut i2c_config = i2c::Config::default();
    i2c_config.frequency = 400_000;
    let i2c = i2c::I2c::new_blocking(p.I2C0, p.PIN_17, p.PIN_16, i2c_config);
    
    // Core 1: input handling and display
    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| {
                // Initialize input on Core 1
                input::init_encoder(
                    spawner, 
                    e1_clk, e1_dta, 
                    e2_clk, e2_dta, 
                    b1, b2, b3, b4, b5, b6
                );
                spawner.spawn(display_task(i2c)).unwrap();
                spawner.spawn(input_task()).unwrap();
            });
        },
    );
    
    // Core 0: timing-critical tasks only
    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        usb::init(p.USB, USB_TX.receiver(), spawner);
        led::init(p.PWM_SLICE0, p.PIN_0, p.PIN_1, p.PWM_SLICE1, p.PIN_2, p.PIN_3, LED_VALUES.receiver(), &spawner);
        spawner.spawn(modulator_task()).unwrap();
    });
}