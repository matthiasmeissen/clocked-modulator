use embedded_graphics::{
    Drawable,
    mono_font::MonoTextStyleBuilder,
    pixelcolor::BinaryColor,
    prelude::Point,
    text::Text,
    mono_font::ascii::FONT_6X10,
};

use sh1106::{prelude::*, Builder};

use rp235x_hal as hal;
use hal::{
    gpio::{FunctionI2C, Pin, PullDown, bank0},
    fugit::RateExtU32,
    clocks::SystemClock,
    pac,
};

type pin16 = Pin<bank0::Gpio16, FunctionI2C, PullDown>;
type pin17 = Pin<bank0::Gpio17, FunctionI2C, PullDown>;

pub fn init(sda_pin: pin16, scl_pin: pin17, i2c0: pac::I2C0, resets: &mut pac::RESETS, system_clock: SystemClock) {
    let sda_pin = sda_pin.reconfigure();
    let scl_pin = scl_pin.reconfigure();

    let i2c = hal::I2C::i2c0(
        i2c0,
        sda_pin,
        scl_pin,
        400.kHz(),
        resets,
        &system_clock
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
}
