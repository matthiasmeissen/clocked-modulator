use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use embassy_rp::i2c;
use embassy_rp::peripherals::I2C0;
use sh1106::{interface::I2cInterface, prelude::*, Builder};

type Driver = GraphicsMode<I2cInterface<i2c::I2c<'static, I2C0, i2c::Blocking>>>;

const TEXT_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
const BAR_STYLE: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_fill(BinaryColor::On);

pub struct Display(Driver);

impl Display {
    pub fn new(i2c: i2c::I2c<'static, I2C0, i2c::Blocking>) -> Self {
        let mut driver: Driver = Builder::new().connect_i2c(i2c).into();
        driver.init().expect("Display init failed");
        Self(driver)
    }

    pub fn draw_bpm(&mut self, bpm: f32) {
        self.0.clear();

        let bpm_int = bpm.clamp(0.0, 999.0) as u16;
        let buf = format_u16(bpm_int);
        let s = core::str::from_utf8(&buf.0[..buf.1]).unwrap_or("ERR");

        Text::new(s, Point::new(0, 20), TEXT_STYLE)
            .draw(&mut self.0)
            .ok();
        self.0.flush().ok();
    }

    pub fn draw_bars(&mut self, values: &[f32; 4]) {
        self.0.clear();

        for (i, &value) in values.iter().enumerate() {
            let y = 25 + i as i32 * 10;
            let width = (value * 128.0) as u32;
            Rectangle::new(Point::new(0, y), Size::new(width, 5))
                .into_styled(BAR_STYLE)
                .draw(&mut self.0)
                .ok();
        }

        self.0.flush().ok();
    }
}

/// Integer-to-string without fmt machinery. Returns (buffer, length).
fn format_u16(mut n: u16) -> ([u8; 4], usize) {
    let mut buf = [0u8; 4];
    if n == 0 {
        buf[0] = b'0';
        return (buf, 1);
    }
    let mut i = 4;
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    let len = 4 - i;
    // Shift to start of buffer
    buf.copy_within(i..4, 0);
    (buf, len)
}
