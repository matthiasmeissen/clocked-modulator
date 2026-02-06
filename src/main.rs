
#![no_std]
#![no_main]

use core::cell::RefCell;
use cortex_m::{self, asm, interrupt::Mutex};
use embedded_hal::digital::OutputPin;
use rp235x_hal as hal;
use hal::timer::Alarm;
use hal::pac::interrupt;
use hal::fugit::MicrosDurationU32;
use panic_halt as _;
use defmt_rtt as _;
use defmt;

#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

type AlarmType = rp235x_hal::timer::Alarm0<rp235x_hal::timer::CopyableTimer0>;

static SHARED_STATE: Mutex<RefCell<Option<AlarmType>>> = Mutex::new(RefCell::new(None));

#[interrupt]
fn TIMER0_IRQ_0() {
    cortex_m::interrupt::free(|cs| {
        if let Some(alarm0) = SHARED_STATE.borrow(cs).borrow_mut().as_mut() {
            alarm0.clear_interrupt();
            defmt::info!("Counter");

            alarm0.schedule(MicrosDurationU32::millis(500)).unwrap();
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
    let _ = alarm0.schedule(MicrosDurationU32::millis(500)).unwrap();
    alarm0.enable_interrupt();

    cortex_m::interrupt::free(|cs| {
        SHARED_STATE.borrow(cs).replace(Some(alarm0));
    });

    let sio = hal::Sio::new(pac.SIO);

    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0, 
        pac.PADS_BANK0, 
        sio.gpio_bank0, 
        &mut pac.RESETS,
    );

    defmt::info!("Test");

    let mut led_pin = pins.gpio25.into_push_pull_output();
    led_pin.set_high().unwrap();

    unsafe { cortex_m::peripheral::NVIC::unmask(hal::pac::Interrupt::TIMER0_IRQ_0) };
    cortex_m::peripheral::NVIC::unpend(hal::pac::Interrupt::TIMER0_IRQ_0);

    loop {
        asm::wfi();
    }
}
