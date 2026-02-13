use embedded_graphics::{
    Drawable,
    mono_font::MonoTextStyleBuilder,
    pixelcolor::BinaryColor,
    prelude::Point,
    text::Text,
    mono_font::ascii::FONT_6X10,
};

use sh1106::{prelude::*, Builder};

pub fn init(i2c: crate::board::I2CType) {
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
