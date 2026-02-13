
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

mod phasor;
mod modulator;
mod usb;
mod display;


use embedded_graphics::Drawable;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::Point;
use embedded_graphics::text::Text;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use sh1106::{prelude::*, Builder};
use hal::gpio::{FunctionI2C, Pin, PullDown, bank0};
use hal::fugit::RateExtU32;
use hal::clocks::SystemClock;

#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

// 1kHz = 1000 microseconds
const TICK_INTERVAL: MicrosDurationU32 = MicrosDurationU32::micros(1000);

type AlarmType = hal::timer::Alarm0<hal::timer::CopyableTimer0>;

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
    let mut pac = hal::pac::Peripherals::take().unwrap();

    // Initialize Clocks
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        12_000_000u32, 
        pac.XOSC, 
        pac.CLOCKS, 
        pac.PLL_SYS, 
        pac.PLL_USB, 
        &mut pac.RESETS, 
        &mut watchdog,
    ).unwrap();

    let mut timer = hal::Timer::new_timer0(
        pac.TIMER0, 
        &mut pac.RESETS, 
        &clocks,
    );

    let mut alarm0: AlarmType = timer.alarm_0().unwrap();
    let first_fire = timer.get_counter() + TICK_INTERVAL;
    alarm0.schedule_at(first_fire).unwrap();
    alarm0.enable_interrupt();

    let tick_rate_hz = 1_000_000.0 / TICK_INTERVAL.ticks() as f32;
    let modulator = modulator::Modulator::new(120.0, tick_rate_hz);

    cortex_m::interrupt::free(|cs| {
        SHARED_STATE.borrow(cs).replace(Some(SharedState { 
            alarm: alarm0, 
            next_fire: first_fire.ticks(),
            modulator,
        }));
    });

    unsafe { cortex_m::peripheral::NVIC::unmask(hal::pac::Interrupt::TIMER0_IRQ_0) };
    cortex_m::peripheral::NVIC::unpend(hal::pac::Interrupt::TIMER0_IRQ_0);


    // GPIO and LED init
    let sio = hal::Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0, 
        pac.PADS_BANK0, 
        sio.gpio_bank0, 
        &mut pac.RESETS,
    );
    let mut led_pin = pins.gpio25.into_push_pull_output();
    led_pin.set_high().unwrap();

    // Initialize USB
    let (mut serial, mut usb_device) = usb::init_usb(
        pac.USB,
        pac.USB_DPRAM,
        clocks.usb_clock,
        &mut pac.RESETS,
    );

    // Display
    let sda_pin: Pin<_, FunctionI2C, _> = pins.gpio16.reconfigure();
    let scl_pin: Pin<_, FunctionI2C, _> = pins.gpio17.reconfigure();

    let i2c = hal::I2C::i2c0(
        pac.I2C0,
        sda_pin,
        scl_pin,
        400.kHz(),
        &mut pac.RESETS,
        &clocks.system_clock,
    );

    let mut display: GraphicsMode<_> = Builder::new().connect_i2c(i2c).into();

    display.init().unwrap();
    display.flush().unwrap();

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    Text::new("Test", Point::new(20, 20), text_style)
        .draw(&mut display)
        .unwrap();

    display.flush().unwrap();

    loop {
        // USB poll data
        if usb_device.poll(&mut [&mut serial]) {
             let mut buf = [0u8; 64];

             if let Ok(count) = serial.read(&mut buf) {
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
            let _ = serial.write(&bytes);
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
