use embassy_rp::i2c;
use embassy_rp::peripherals::I2C0;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text, TextStyleBuilder},
};
use sh1106::{Builder, interface::I2cInterface, prelude::*};

use crate::nav::NavState;

type Driver = GraphicsMode<I2cInterface<i2c::I2c<'static, I2C0, i2c::Blocking>>>;

const CHARACTER_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
const BORDER_STYLE: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

pub struct Display {
    driver: Driver,
}

impl Display {
    pub fn new(i2c: i2c::I2c<'static, I2C0, i2c::Blocking>) -> Self {
        let mut driver: Driver = Builder::new().connect_i2c(i2c).into();
        driver.init().expect("Display init failed");
        Self { driver }
    }

    pub fn draw_main(&mut self, bpm: f32, nav: &NavState) {
        self.driver.clear();

        match nav {
            NavState::Overview => self.draw_screen_overview(bpm),
            NavState::TapMode => self.draw_screen_tapmode(bpm),
            _ => self.draw_screen_modedit(1),
        }

        self.driver.flush().ok();
    }

    fn draw_screen_overview(&mut self, bpm: f32) {
        self.draw_element_text(get_slot_position(1), "Main", false);
        self.draw_element_bpm(get_slot_position(2), bpm);
        self.draw_element_text(get_slot_position(3), "A", true);
        self.draw_element_text(get_slot_position(4), "B", true);
        self.draw_element_text(get_slot_position(6), "TEMP", true);
        self.draw_element_text(get_slot_position(7), "C", true);
        self.draw_element_text(get_slot_position(8), "D", true);
    }

    fn draw_screen_tapmode(&mut self, bpm: f32) {
        self.draw_element_text(get_slot_position(1), "Tap", false);
        self.draw_element_bpm(get_slot_position(2), bpm);
        self.draw_element_text(get_slot_position(3), "Up", true);
        self.draw_element_text(get_slot_position(4), "Tap", true);
        self.draw_element_text(get_slot_position(6), "TEMP", true);
        self.draw_element_text(get_slot_position(7), "PAUS", true);
        self.draw_element_text(get_slot_position(8), "PLAY", true);
    }

    fn draw_screen_modedit(&mut self, slot: usize) {
        self.draw_element_text(get_slot_position(1), "A", false);
        
        self.draw_element_values(get_slot_position(2), "Wave", "SIN");
        self.draw_element_text(get_slot_position(3), "Up", true);
        self.draw_element_text(get_slot_position(4), "Res", true);

        self.draw_element_values(get_slot_position(6), "Mult", "X2");
        self.draw_element_text(get_slot_position(7), "Ok", true);
        self.draw_element_text(get_slot_position(8), "Rng", true);
    }

    fn draw_element_bpm(&mut self, point: Point, bpm: f32) {
        let bpm_int = bpm.clamp(0.0, 999.0) as u16;
        let buf = format_u16(bpm_int);
        let s = core::str::from_utf8(&buf.0[..buf.1]).unwrap_or("ERR");

        Text::with_baseline(s, Point::new(point.x + 3, point.y + 2), CHARACTER_STYLE, Baseline::Top)
            .draw(&mut self.driver)
            .ok();

        self.draw_element_outline(point);
    }

    fn draw_element_text(&mut self, point: Point, text: &'static str, border: bool) {
        Text::with_baseline(text, Point::new(point.x + 3, point.y + 2), CHARACTER_STYLE, Baseline::Top)
            .draw(&mut self.driver).ok();

        if border {
            self.draw_element_outline(point);
        }
    }

    fn draw_element_values(&mut self, point: Point, label: &'static str, value: &'static str) {
        Text::with_baseline(label, Point::new(point.x + 3, point.y + 2), CHARACTER_STYLE, Baseline::Top)
            .draw(&mut self.driver).ok();

        Text::with_baseline(value, Point::new(point.x + 3, point.y + 19), CHARACTER_STYLE, Baseline::Top)
            .draw(&mut self.driver).ok();

        self.draw_element_outline(point);
    }

    fn draw_element_outline(&mut self, point: Point) {
        Rectangle::new(point, Size::new(30, 30))
            .into_styled(BORDER_STYLE)
            .draw(&mut self.driver).ok();
    }
}

fn get_slot_position(slot: usize) -> Point {
    match slot {
        1 => Point::new(0, 0),
        2 => Point::new(32, 0),
        3 => Point::new(64, 0),
        4 => Point::new(96, 0),
        5 => Point::new(0, 32),
        6 => Point::new(32, 32),
        7 => Point::new(64, 32),
        8 => Point::new(96, 32),
        _ => Point::new(0, 0),
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
