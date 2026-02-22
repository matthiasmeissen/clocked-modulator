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

use crate::{encoder::InputEvent, modulator::ModulatorConfig};

type Driver = GraphicsMode<I2cInterface<i2c::I2c<'static, I2C0, i2c::Blocking>>>;

const CHARACTER_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
const CHARACTER_STYLE_INVERT: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_6X10, BinaryColor::Off);
const FILL_STYLE: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_fill(BinaryColor::On);
const BORDER_STYLE: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_stroke(BinaryColor::On, 1);


// ------------------------------
// State Machine
// ------------------------------

#[derive(Clone, Copy)]
pub enum SlotParam {
    Wave,
    Mul,
}

#[derive(Clone, Copy)]
pub enum NavState {
    Browse { index: u8 },
    EditBpm { draft: u16 },
    SlotFocus { slot: u8, param: SlotParam },
    SlotEdit { slot: u8, param: SlotParam },
}

impl NavState {
    pub fn handle(self, event: InputEvent, config: &mut ModulatorConfig, bpm: &mut u16) -> Self {
        match (self, event) {
            (Self::Browse { index: 0 }, InputEvent::Enter) => {
                NavState::EditBpm { draft: *bpm }
            }
            // Edit BPM
            (Self::EditBpm { draft }, InputEvent::Next) => {
                NavState::EditBpm { draft: (draft + 1).min(300)}
            }
            (Self::EditBpm { draft }, InputEvent::Prev) => {
                NavState::EditBpm { draft: draft.saturating_sub(1).max(20)}
            }
            (Self::EditBpm { draft }, InputEvent::Enter) => {
                *bpm = draft;
                NavState::Browse { index: 0 }
            }
            (Self::EditBpm { .. }, InputEvent::Back) => {
                NavState::Browse { index: 0 }
            }
            _ => self
        }
    }
}


// ------------------------------
// Display and UI
// ------------------------------

enum UiState {
    Default,
    Hover,
    Active,
}

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
            NavState::Browse { index: 0 } => self.draw_bpm(bpm, UiState::Hover),
            NavState::EditBpm { draft } => self.draw_bpm(*draft as f32, UiState::Active),
            _ => self.draw_bpm(bpm, UiState::Default),
        }
        
        self.draw_modulator(Point::new(0, 12), "SIN", "X2");
        self.draw_modulator(Point::new(30, 12), "SIN", "X2");
        self.draw_modulator(Point::new(60, 12), "SIN", "X2");
        self.draw_modulator(Point::new(90, 12), "SIN", "X2");

        self.draw_modulator(Point::new(0, 36), "SAW", "D4");
        self.draw_modulator(Point::new(30, 36), "SAW", "D4");
        self.draw_modulator(Point::new(60, 36), "SAW", "D4");
        self.draw_modulator(Point::new(90, 36), "SAW", "D4");
        
        self.driver.flush().ok();
    }

    fn draw_bpm(&mut self, bpm: f32, state: UiState) {
        let bpm_int = bpm.clamp(0.0, 999.0) as u16;
        let buf = format_u16(bpm_int);
        let s = core::str::from_utf8(&buf.0[..buf.1]).unwrap_or("ERR");

        match state {
            UiState::Default => {
                Text::with_baseline(s, Point::new(0, 0), CHARACTER_STYLE, Baseline::Top)
                .draw(&mut self.driver)
                .ok();
            },
            UiState::Hover => {
                Rectangle::new(Point::new(0, 0), Size::new(128, 10))
                    .into_styled(BORDER_STYLE)
                    .draw(&mut self.driver).ok();
                Text::with_baseline(s, Point::new(0, 0), CHARACTER_STYLE, Baseline::Top)
                    .draw(&mut self.driver)
                    .ok();
            }
            UiState::Active => {
                Rectangle::new(Point::new(0, 0), Size::new(128, 10))
                    .into_styled(FILL_STYLE)
                    .draw(&mut self.driver).ok();
                Text::with_baseline(s, Point::new(0, 0), CHARACTER_STYLE_INVERT, Baseline::Top)
                    .draw(&mut self.driver)
                    .ok();
            }
        }
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
