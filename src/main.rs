
#![no_std]
#![no_main]

use cortex_m::{self, asm};
use rp235x_hal as hal;
use panic_halt as _;

#[hal::entry]
fn main() -> ! {
    loop {
        asm::wfi();
    }
}
