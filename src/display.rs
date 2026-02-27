use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle, RoundedRectangle},
    text::{Baseline, Text, TextStyleBuilder},
};
use embassy_rp::i2c;
use embassy_rp::peripherals::I2C0;
use sh1106::{interface::I2cInterface, prelude::*, Builder};

use crate::{encoder::InputEvent, modulator::{ModSlot, ModulatorConfig}};

type Driver = GraphicsMode<I2cInterface<i2c::I2c<'static, I2C0, i2c::Blocking>>>;

const CHARACTER_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
const CHARACTER_STYLE_INVERT: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_6X10, BinaryColor::Off);
const FILL_STYLE: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_fill(BinaryColor::On);
const BORDER_STYLE: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_stroke(BinaryColor::On, 1);


// ------------------------------
// State Machine
// ------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum SlotId {
    A, B, C, D,
}

impl SlotId {
    fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
            Self::C => 2,
            Self::D => 3,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::A => "Slot A",
            Self::B => "Slot B",
            Self::C => "Slot C",
            Self::D => "Slot D",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum EditPage {
    Waves, Range,
}

impl EditPage {
    fn toggle(self) -> Self {
        match self {
            EditPage::Waves => EditPage::Range,
            EditPage::Range => EditPage::Waves,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum PlaybackState {
    Playing, Paused,
}

#[derive(Clone, Copy, PartialEq)]
pub enum NavState {
    Overview,
    TapMode,
    ModEdit { 
        slot: SlotId, 
        page: EditPage, 
        draft: ModSlot 
    },
}

impl NavState {
    pub fn handle(self, event: InputEvent, bpm: &mut u16, config: &mut ModulatorConfig, playback: &mut PlaybackState, rest_bar: &mut bool) -> Self {
        use InputEvent::*;
        use NavState::*;
        match (self, event) {
            // Overview
            (Overview, Enc1Rotate(delta)) => {
                *bpm = (*bpm as i16 + delta as i16).clamp(20, 300) as u16;
                Overview
            },
            (Overview, B1Press) => TapMode,
            _ => Overview
        }
    }
}


// ------------------------------
// Display and UI
// ------------------------------

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
            NavState::Overview => self.draw_overview(bpm),
            NavState::TapMode => self.draw_tapmode(),
            _ => self.draw_overview(bpm),
        }
        
        self.driver.flush().ok();
    }

    fn draw_overview(&mut self, bpm: f32) {
        let bpm_int = bpm.clamp(0.0, 999.0) as u16;
        let buf = format_u16(bpm_int);
        let s = core::str::from_utf8(&buf.0[..buf.1]).unwrap_or("ERR");

        Text::with_baseline(s, Point::new(0, 0), CHARACTER_STYLE, Baseline::Top)
        .draw(&mut self.driver)
        .ok();

        self.draw_grid_element(2);
        self.draw_grid_element(3);
        self.draw_grid_element(4);
        self.draw_grid_element(6);
        self.draw_grid_element(7);
        self.draw_grid_element(8);
    }

    fn draw_tapmode(&mut self) {
        Text::with_baseline("Tap", Point::new(0, 0), CHARACTER_STYLE, Baseline::Top)
        .draw(&mut self.driver)
        .ok();

        self.draw_grid_element(2);
        self.draw_grid_element(3);
        self.draw_grid_element(4);
        self.draw_grid_element(6);
        self.draw_grid_element(7);
        self.draw_grid_element(8);
    }

    fn draw_grid_element(&mut self, slot: usize) {
        // 128 x 64
        // Cell is 30 x 30
        let point = match slot {
            1 => Point::new(0, 0),
            2 => Point::new(32, 0),
            3 => Point::new(64, 0),
            4 => Point::new(96, 0),
            5 => Point::new(0, 32),
            6 => Point::new(32, 32),
            7 => Point::new(64, 32),
            8 => Point::new(96, 32),
            _ => Point::new(0, 0),
        };

        Rectangle::new(point, Size::new(30, 30))
            .into_styled(BORDER_STYLE)
            .draw(&mut self.driver).ok();
    }

    fn draw_modulator(&mut self, pos: Point, wave: &str, mul: &str) {
        Rectangle::new(pos, Size::new(28, 22))
            .into_styled(BORDER_STYLE)
            .draw(&mut self.driver).ok();

        Text::with_baseline(wave, Point::new(pos.x + 2, pos.y + 1), CHARACTER_STYLE, Baseline::Top)
            .draw(&mut self.driver).ok();

        Text::with_baseline(mul, Point::new(pos.x + 2, pos.y + 12), CHARACTER_STYLE, Baseline::Top)
            .draw(&mut self.driver).ok();
    }

    pub fn draw_bars(&mut self, values: &[f32; 4]) {
        self.driver.clear();

        for (i, &value) in values.iter().enumerate() {
            let y = 25 + i as i32 * 10;
            let width = (value * 128.0) as u32;
            Rectangle::new(Point::new(0, y), Size::new(width, 5))
                .into_styled(FILL_STYLE)
                .draw(&mut self.driver)
                .ok();
        }

        self.driver.flush().ok();
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
