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

use crate::{modulator::ModSlot, nav::SlotId};
use crate::nav::NavState;

type Driver = GraphicsMode<I2cInterface<i2c::I2c<'static, I2C0, i2c::Blocking>>>;

const CHARACTER_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
const BORDER_STYLE: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
const FILL_STYLE: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_fill(BinaryColor::On);

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
            NavState::ModEditWave { draft, slot } => self.draw_screen_modedit_wave(draft, slot),
            NavState::ModEditRange { slot, draft } => self.draw_screen_modedit_range(draft, slot),
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

    fn draw_screen_modedit_wave(&mut self, draft: &ModSlot, slot: &SlotId) {
        self.draw_element_text(get_slot_position(1), slot.label(), false);

        self.draw_element_values(get_slot_position(2), "Wave", draft.wave.name());
        self.draw_element_text(get_slot_position(3), "Up", true);
        self.draw_element_text(get_slot_position(4), "Res", true);

        self.draw_element_values(get_slot_position(6), "Mult", draft.mul.name());
        self.draw_element_text(get_slot_position(7), "Ok", true);
        self.draw_element_text(get_slot_position(8), "Rng", true);
    }

    fn draw_screen_modedit_range(&mut self, draft: &ModSlot, slot: &SlotId) {
        self.draw_element_text(get_slot_position(1), slot.label(), false);

        self.draw_element_range(get_slot_position(2), draft.min, draft.max);
        self.draw_element_text(get_slot_position(3), "Up", true);
        self.draw_element_text(get_slot_position(4), "Res", true);
        
        self.draw_element_text(get_slot_position(7), "Ok", true);
        self.draw_element_text(get_slot_position(8), "Wave", true);
    }

    // TO DO: Convert into generic draw float element
    /// Draws a grid cell with bpm as text
    fn draw_element_bpm(&mut self, point: Point, bpm: f32) {
        let bpm_int = bpm.clamp(0.0, 999.0) as u16;
        let buf = format_u16(bpm_int);
        let s = core::str::from_utf8(&buf.0[..buf.1]).unwrap_or("ERR");

        Text::with_baseline(s, Point::new(point.x + 3, point.y + 2), CHARACTER_STYLE, Baseline::Top)
            .draw(&mut self.driver)
            .ok();

        self.draw_element_outline(point);
    }

    /// Draws a grid cell with a text
    fn draw_element_text(&mut self, point: Point, text: &'static str, border: bool) {
        Text::with_baseline(text, Point::new(point.x + 3, point.y + 2), CHARACTER_STYLE, Baseline::Top)
            .draw(&mut self.driver).ok();

        if border {
            self.draw_element_outline(point);
        }
    }

    /// Draws a grid cell with text label and value
    fn draw_element_values(&mut self, point: Point, label: &'static str, value: &'static str) {
        Text::with_baseline(label, Point::new(point.x + 3, point.y + 2), CHARACTER_STYLE, Baseline::Top)
            .draw(&mut self.driver).ok();

        Text::with_baseline(value, Point::new(point.x + 3, point.y + 19), CHARACTER_STYLE, Baseline::Top)
            .draw(&mut self.driver).ok();

        self.draw_element_outline(point);
    }

    /// Draw two grid cells as column with range adjust
    fn draw_element_range(&mut self, point: Point, min: f32, max: f32) {
        let bar_x = point.x + 11;
        let bar_y = point.y + 3;
        let bar_height: i32 = 56;

        // Bar outline (full range 0.0–1.0)
        Rectangle::new(Point::new(bar_x, bar_y), Size::new(8, bar_height as u32))
            .into_styled(BORDER_STYLE)
            .draw(&mut self.driver).ok();

        // Map values to pixel y (inverted: 1.0 = top, 0.0 = bottom)
        let max_y = bar_y + ((1.0 - max) * bar_height as f32) as i32;
        let min_y = bar_y + ((1.0 - min) * bar_height as f32) as i32;

        // Filled region between min and max
        let fill_height = (min_y - max_y).max(0) as u32;
        Rectangle::new(Point::new(bar_x, max_y), Size::new(8, fill_height))
            .into_styled(FILL_STYLE)
            .draw(&mut self.driver).ok();

        // Min indicator (left side)
        Rectangle::new(Point::new(bar_x - 8, min_y - 1), Size::new(6, 3))
            .into_styled(BORDER_STYLE)
            .draw(&mut self.driver).ok();

        // Max indicator (right side)
        Rectangle::new(Point::new(bar_x + 10, max_y - 1), Size::new(6, 3))
            .into_styled(BORDER_STYLE)
            .draw(&mut self.driver).ok();

        self.draw_element_outline_column(point);
    }

    /// Outline for one grid cell
    fn draw_element_outline(&mut self, point: Point) {
        Rectangle::new(point, Size::new(30, 30))
            .into_styled(BORDER_STYLE)
            .draw(&mut self.driver).ok();
    }

    /// Outline for two grid cells spanning a column
    fn draw_element_outline_column(&mut self, point: Point) {
        Rectangle::new(point, Size::new(30, 62))
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
