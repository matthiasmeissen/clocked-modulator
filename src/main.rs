
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
    // let sio = hal::Sio::new(pac.SIO);
    // let pins = hal::gpio::Pins::new(
    //     pac.IO_BANK0, 
    //     pac.PADS_BANK0, 
    //     sio.gpio_bank0, 
    //     &mut pac.RESETS,
    // );
    // let mut led_pin = pins.gpio25.into_push_pull_output();
    // led_pin.set_high().unwrap();

    // Initialize USB
    let (mut serial, mut usb_device) = usb::init_usb( 
        pac.USB, 
        pac.USB_DPRAM, 
        clocks.usb_clock, 
        &mut pac.RESETS, 
    );

    loop {
        if usb_device.poll(&mut [&mut serial]) {
             let mut buf = [0u8; 64];

             match serial.read(&mut buf) {
                Ok(count) if count > 0 => {
                    // Simple Parser: Check if we have enough bytes for a command
                    // (Header + 4 bytes float = 5 bytes)
                    
                    // Iterate through buffer in case multiple commands arrived or offset
                    let mut i = 0;
                    while i + 5 <= count {
                        // Check for 'B' Header
                        if buf[i] == b'B' {
                            // Extract the next 4 bytes
                            let mut float_bytes = [0u8; 4];
                            float_bytes.copy_from_slice(&buf[i+1 .. i+5]);
                            
                            let new_bpm = f32::from_le_bytes(float_bytes);
                            
                            cortex_m::interrupt::free(|cs| {
                                if let Some(state) = SHARED_STATE.borrow(cs).borrow_mut().as_mut() {
                                    state.modulator.bank.set_bpm(new_bpm); 
                                }
                            });
                            
                            defmt::info!("Set BPM to: {}", new_bpm);
                            
                            i += 5;
                        } else {
                            i += 1;
                        }
                    }
                }
                _ => {} 
            }
        }

        // 2. Logic (Get data from interrupt)
        let (tx_buffer, snapshot) = cortex_m::interrupt::free(|cs| {
            if let Some(state) = SHARED_STATE.borrow(cs).borrow().as_ref() {
                let bytes = state.modulator.get_output_as_bytes();
                let data = state.modulator.get_all_outputs();
                (Some(bytes), Some(data))
            } else {
                (None, None)
            }
        });

        // 3. Send Data (Throttle this! Don't spam 100% CPU)
        cortex_m::asm::delay(12_000_000 / 100); // Wait ~10ms

        if let Some(bytes) = tx_buffer {
            let _ = serial.write(&bytes);
        }

        // For visualization in RTT (Optional)
        if let Some(data) = snapshot {
            defmt::info!("{}", modulator::Visualizer4(data));
        }

        // asm::wfi();
    }
}
