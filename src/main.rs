
#![no_std]
#![no_main]

use cortex_m::{self, asm};
use embedded_hal::digital::OutputPin;
use rp235x_hal as hal;
use panic_halt as _;
use defmt_rtt as _;
use defmt;

#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

#[hal::entry]
fn main() -> ! {
    let mut pac = hal::pac::Peripherals::take().unwrap();

    let sio = hal::Sio::new(pac.SIO);

    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0, 
        pac.PADS_BANK0, 
        sio.gpio_bank0, 
        &mut pac.RESETS
    );

    defmt::info!("Test");

    let mut led_pin = pins.gpio25.into_push_pull_output();
    led_pin.set_high().unwrap();

    loop {
        asm::wfi();
    }
}
