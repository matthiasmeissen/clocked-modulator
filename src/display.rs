use embedded_graphics::{
    Drawable, mono_font::{MonoTextStyleBuilder, ascii::FONT_6X10}, pixelcolor::BinaryColor, prelude::{DrawTarget, Point, Primitive, Size}, primitives::{PrimitiveStyle, Rectangle}, text::Text
};

use sh1106::{prelude::*, Builder, interface::I2cInterface};

pub type Display = GraphicsMode<I2cInterface<crate::board::I2CType>>;

pub fn init(i2c: crate::board::I2CType) -> Display {
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

    display
}

pub fn draw_screen(display: &mut Display, values: [f32; 4]) {
    display.clear();

    draw_bar(display, values[0], 5);
    draw_bar(display, values[1], 15);
    draw_bar(display, values[2], 25);
    draw_bar(display, values[3], 35);

    let _ = display.flush();
}

pub fn draw_bar(display: &mut Display, value: f32, pos_y: i32) {
    let bar_width = (value * 128.0) as u32;

    let style = PrimitiveStyle::with_fill(BinaryColor::On);
    let _ = Rectangle::new(
        Point::new(0, pos_y),
        Size::new(bar_width, 5)
    ).into_styled(style).draw(display).unwrap();
}
