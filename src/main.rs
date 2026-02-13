
#![no_std]
#![no_main]

use core::cell::RefCell;
use cortex_m::{self, asm, interrupt::Mutex};
use embedded_hal::digital::OutputPin;
use rp235x_hal as hal;
use hal::timer::{Alarm, Instant};
use hal::pac::interrupt;
use hal::fugit::MicrosDurationU32;
use panic_halt as _;
use defmt_rtt as _;
use defmt;

use crate::board::Board;

mod phasor;
mod modulator;
mod board;
mod usb;
mod display;

#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

// 1kHz = 1000 microseconds
pub const TICK_INTERVAL: MicrosDurationU32 = MicrosDurationU32::micros(1000);

pub type AlarmType = hal::timer::Alarm0<hal::timer::CopyableTimer0>;
pub type I2CType = rp235x_hal::I2C<rp235x_hal::pac::I2C0, (rp235x_hal::gpio::Pin<rp235x_hal::gpio::bank0::Gpio16, rp235x_hal::gpio::FunctionI2c, rp235x_hal::gpio::PullUp>, rp235x_hal::gpio::Pin<rp235x_hal::gpio::bank0::Gpio17, rp235x_hal::gpio::FunctionI2c, rp235x_hal::gpio::PullUp>)>;

struct SharedState {
    alarm: AlarmType,
    next_fire: u64,
    modulator: modulator::Modulator,
}

static SHARED_STATE: Mutex<RefCell<Option<SharedState>>> = Mutex::new(RefCell::new(None));

#[interrupt]
fn TIMER0_IRQ_0() {
    cortex_m::interrupt::free(|cs| {
        if let Some(state) = SHARED_STATE.borrow(cs).borrow_mut().as_mut() {
            state.alarm.clear_interrupt();

            state.modulator.tick();

            let next = state.next_fire + TICK_INTERVAL.ticks() as u64;
            state.next_fire = next;
            let _ = state.alarm.schedule_at(Instant::from_ticks(next));
        }
    })
}

#[hal::entry]
fn main() -> ! {
    let mut board = Board::init();

    let tick_rate_hz = 1_000_000.0 / TICK_INTERVAL.ticks() as f32;
    let modulator = modulator::Modulator::new(120.0, tick_rate_hz);

    let mut alarm: crate::AlarmType = board.timer.alarm_0().unwrap();
    let first_fire: rp235x_hal::fugit::Instant<u64, 1, 1000000> = board.timer.get_counter() + crate::TICK_INTERVAL;
    alarm.schedule_at(first_fire).unwrap();
    alarm.enable_interrupt();

    cortex_m::interrupt::free(|cs| {
        SHARED_STATE.borrow(cs).replace(Some(SharedState { 
            alarm: alarm, 
            next_fire: first_fire.ticks(),
            modulator,
        }));
    });

    unsafe { cortex_m::peripheral::NVIC::unmask(hal::pac::Interrupt::TIMER0_IRQ_0) };
    cortex_m::peripheral::NVIC::unpend(hal::pac::Interrupt::TIMER0_IRQ_0);

    display::init(board.i2c);

    let mut led_pin = board.led_pin;
    led_pin.set_high().unwrap();

    loop {
        // USB poll data
        if board.usb_device.poll(&mut [&mut board.serial]) {
             let mut buf = [0u8; 64];

             if let Ok(count) = board.serial.read(&mut buf) {
                let mut i = 0;
                // Keep looping while we have at least 5 bytes remaining (1 Header + 4 Float)
                while i + 5 <= count {
                    // Check for the 'B' Header
                    if buf[i] == b'B' {                        
                        // 1. Grab the 4 bytes representing the float
                        let bytes = [buf[i+1], buf[i+2], buf[i+3], buf[i+4]];
                        // 2. Convert bytes to float (Standard Rust function)
                        let bpm = f32::from_le_bytes(bytes);
                        
                        cortex_m::interrupt::free(|cs| {
                            if let Some(state) = SHARED_STATE.borrow(cs).borrow_mut().as_mut() {
                                state.modulator.bank.set_bpm(bpm);
                            }
                        });
                        
                        defmt::info!("Received BPM: {}", bpm);

                        // 4. Important: Jump forward 5 bytes so we don't read the same data again
                        i += 5;
                        
                    } else {
                        // If it wasn't 'B', move 1 byte forward and try again
                        i += 1;
                    }
                }
            }
        }

        // Get shared data and store as bytes and floats
        let (tx_buffer, snapshot) = cortex_m::interrupt::free(|cs| {
            if let Some(state) = SHARED_STATE.borrow(cs).borrow().as_ref() {
                let bytes = state.modulator.get_output_as_bytes();
                let data = state.modulator.get_all_outputs();
                (Some(bytes), Some(data))
            } else {
                (None, None)
            }
        });

        // Send data bytes over USB with delay
        cortex_m::asm::delay(12_000_000 / 100);
        if let Some(bytes) = tx_buffer {
            let _ = board.serial.write(&bytes);
        }

        // Toggle led based on data and log values to rtt
        if let Some(data) = snapshot {
            if data[3] > 0.5 {
                led_pin.set_high().unwrap();
            } else {
                led_pin.set_low().unwrap();
            }
            defmt::info!("{}", modulator::Visualizer4(data));
        }

        // asm::wfi();
    }
}
