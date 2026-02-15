
use core::fmt::Write;
use heapless::String;
use embedded_graphics::{
    Drawable, mono_font::{MonoTextStyleBuilder, ascii::FONT_6X10}, pixelcolor::BinaryColor, prelude::{DrawTarget, Point, Primitive, Size}, primitives::{PrimitiveStyle, Rectangle}, text::Text
};
use embassy_rp::peripherals::I2C0;
use embassy_rp::i2c;

use sh1106::{prelude::*, Builder, interface::I2cInterface};

pub type DisplayType = GraphicsMode<I2cInterface<i2c::I2c<'static, I2C0, i2c::Blocking>>>;

pub fn draw_screen(display: &mut DisplayType, values: [f32; 4]) {
    display.clear();

    draw_bar(display, values[0], 25);
    draw_bar(display, values[1], 35);
    draw_bar(display, values[2], 45);
    draw_bar(display, values[3], 55);

    let _ = display.flush();
}

pub fn draw_bpm(display: &mut DisplayType, value: f32) {
    let mut buf: String<16> = String::new();
    write!(buf, "{:.0}", value).unwrap();
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    Text::new(&buf, Point::new(0, 20), text_style)
        .draw(display)
        .unwrap();
}

pub fn draw_bar(display: &mut DisplayType, value: f32, pos_y: i32) {
    let bar_width = (value * 128.0) as u32;

    let style = PrimitiveStyle::with_fill(BinaryColor::On);
    let _ = Rectangle::new(
        Point::new(0, pos_y),
        Size::new(bar_width, 5)
    ).into_styled(style).draw(display).unwrap();
}