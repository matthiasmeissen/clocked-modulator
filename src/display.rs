use embedded_graphics::Drawable;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::Point;
use embedded_graphics::text::Text;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use sh1106::{prelude::*, Builder};
use rp235x_hal as hal;
use hal::gpio::{FunctionI2C, Pin, PullDown, bank0};
use hal::fugit::RateExtU32;
use hal::clocks::SystemClock;

pub fn init(
    sda_pin: Pin<bank0::Gpio18, FunctionI2C, PullDown>,
    scl_pin: Pin<bank0::Gpio19, FunctionI2C, PullDown>,
    i2c1: hal::pac::I2C1,
    resets: &mut hal::pac::RESETS,
    system_clock: SystemClock
) {
    let sda_pin = sda_pin.reconfigure();
    let scl_pin = scl_pin.reconfigure();

    let i2c = hal::I2C::i2c1(
        i2c1,
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
