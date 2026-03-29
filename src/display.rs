use embassy_rp::i2c;
use embassy_rp::peripherals::I2C0;
use embedded_graphics::{
    image::Image,
    mono_font::{MonoTextStyle, ascii::FONT_6X9},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{CornerRadii, PrimitiveStyle, Rectangle, RoundedRectangle},
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use crate::sh1106::Sh1106;
use tinybmp::Bmp;

use crate::modulator::{ModSlot, ModulatorConfig, Waveshape};
use crate::nav::{NavState, SlotId};
use crate::phasor::GlobalSpeed;
use crate::phasor::Multiplier;

type Driver = Sh1106<i2c::I2c<'static, I2C0, i2c::Async>>;

const WAVESHAPES_BMP: &[u8] = include_bytes!("../assets/export/waveshapes_1bit.bmp");
const ICONS_BMP: &[u8] = include_bytes!("../assets/export/icons_1bit.bmp");
const TITLES_BMP: &[u8] = include_bytes!("../assets/export/page_titles_1bit.bmp");

const CHARACTER_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_6X9, BinaryColor::On);
const BORDER_STYLE: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
const FILL_STYLE: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_fill(BinaryColor::On);
// TO DO: Add center text alignment as style

pub enum SpritesheetIndex {
    Normalized(f32),
    Index(usize),
}

enum IconSprite {
    arrow_up,
    cross,
    check,
    range,
    wave,
    pause,
    play,
    tap,
}

pub struct Display {
    driver: Driver,
}

impl Display {
    pub async fn new(i2c: i2c::I2c<'static, I2C0, i2c::Async>) -> Self {
        let mut driver = Sh1106::new(i2c, 0x3C);
        driver.init().await.expect("Display init failed");
        driver.clear();
        driver.flush_all().await.expect("Display flush failed");
        Self { driver }
    }

    pub async fn draw_main(&mut self, bpm: f32, speed: GlobalSpeed, nav: &NavState, config: &ModulatorConfig) {
        self.driver.clear();

        match nav {
            NavState::Overview => self.draw_screen_overview(bpm, speed, config),
            NavState::TapMode => self.draw_screen_tapmode(bpm),
            NavState::ModEditWave { draft, slot } => self.draw_screen_modedit_wave(draft, slot),
            NavState::ModEditRange { slot, draft } => self.draw_screen_modedit_range(draft, slot),
        }

        self.driver.flush().await.ok();
    }

    fn draw_screen_overview(&mut self, bpm: f32, speed: GlobalSpeed, config: &ModulatorConfig) {
        self.draw_element_title(get_slot_position(1), "", 0);

        self.draw_element_bpm(get_slot_position(2), bpm);
        self.draw_element_wave_teaser(get_slot_position(3), "A", &config.slots[0]);
        self.draw_element_wave_teaser(get_slot_position(4), "B", &config.slots[1]);

        self.draw_element_value(get_slot_position(6), "MUL", speed.name());
        self.draw_element_wave_teaser(get_slot_position(7), "C", &config.slots[2]);
        self.draw_element_wave_teaser(get_slot_position(8), "D", &config.slots[3]);
    }

    fn draw_screen_tapmode(&mut self, bpm: f32) {
        self.draw_element_title(get_slot_position(1), "", 1);

        self.draw_element_bpm(get_slot_position(2), bpm);
        self.draw_element_icon(get_slot_position(3), "UP", IconSprite::arrow_up);
        self.draw_element_icon(get_slot_position(4), "TAP", IconSprite::tap);

        self.draw_element_icon(get_slot_position(6), "RES", IconSprite::cross);
        self.draw_element_icon(get_slot_position(7), "PAUS", IconSprite::pause);
        self.draw_element_icon(get_slot_position(8), "PLAY", IconSprite::play);
    }

    fn draw_screen_modedit_wave(&mut self, draft: &ModSlot, slot: &SlotId) {
        self.draw_element_title(get_slot_position(1), slot.label(), 2);

        self.draw_element_wave(get_slot_position(2), "WAVE", draft.wave);
        self.draw_element_icon(get_slot_position(3), "UP", IconSprite::arrow_up);
        self.draw_element_icon(get_slot_position(4), "RES", IconSprite::cross);

        self.draw_element_value(get_slot_position(6), "MULT", draft.mul.name());
        self.draw_element_icon(get_slot_position(7), "OK", IconSprite::check);
        self.draw_element_icon(get_slot_position(8), "RNG", IconSprite::range);
    }

    fn draw_screen_modedit_range(&mut self, draft: &ModSlot, slot: &SlotId) {
        self.draw_element_title(get_slot_position(1), slot.label(), 2);

        self.draw_element_range(get_slot_position(2), draft.min, draft.max);
        self.draw_element_icon(get_slot_position(3), "UP", IconSprite::arrow_up);
        self.draw_element_icon(get_slot_position(4), "RES", IconSprite::cross);

        self.draw_element_icon(get_slot_position(7), "OK", IconSprite::check);
        self.draw_element_icon(get_slot_position(8), "WAVE", IconSprite::wave);
    }

    /// Draws a grid cell with bpm as text
    fn draw_element_bpm(&mut self, point: Point, bpm: f32) {
        let bpm_int = bpm.clamp(0.0, 999.0) as u16;
        let buf = format_u16(bpm_int);
        let s = core::str::from_utf8(&buf.0[..buf.1]).unwrap_or("ERR");

        let text_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Top)
            .build();

        Text::with_text_style(
            s,
            Point::new(point.x + 14, point.y + 7),
            CHARACTER_STYLE,
            text_style,
        )
        .draw(&mut self.driver)
        .ok();

        self.draw_element_outline_with_label(point, "BPM");
    }

    /// Draws a grid cell with a text
    fn draw_element_text(&mut self, point: Point, text1: &'static str, text2: &'static str) {
        Text::with_baseline(
            text1,
            Point::new(point.x + 3, point.y + 2),
            CHARACTER_STYLE,
            Baseline::Top,
        )
        .draw(&mut self.driver)
        .ok();

        Text::with_baseline(
            text2,
            Point::new(point.x + 3, point.y + 11),
            CHARACTER_STYLE,
            Baseline::Top,
        )
        .draw(&mut self.driver)
        .ok();
    }

    /// Draws a grid cell with wave
    fn draw_element_wave(&mut self, point: Point, label: &'static str, wave: Waveshape) {
        self.draw_waveshape(Point::new(point.x + 9, point.y + 8), wave);

        self.draw_element_outline_with_label(point, label);
    }

    /// Draws a grid cell with text label and value
    fn draw_element_value(&mut self, point: Point, label: &'static str, value: &'static str) {
        let text_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Top)
            .build();

        Text::with_text_style(
            value,
            Point::new(point.x + 14, point.y + 7),
            CHARACTER_STYLE,
            text_style,
        )
        .draw(&mut self.driver)
        .ok();

        self.draw_element_outline_with_label(point, label);
    }

    /// Draw two grid cells as column with range adjust
    fn draw_element_range(&mut self, point: Point, min: f32, max: f32) {
        let bar_x = point.x + 11;
        let bar_y = point.y + 3;
        let bar_height: i32 = 49;

        // Bar outline (full range 0.0–1.0)
        Rectangle::new(Point::new(bar_x, bar_y), Size::new(8, bar_height as u32))
            .into_styled(BORDER_STYLE)
            .draw(&mut self.driver)
            .ok();

        // Map values to pixel y (inverted: 1.0 = top, 0.0 = bottom)
        let max_y = bar_y + ((1.0 - max) * bar_height as f32) as i32;
        let min_y = bar_y + ((1.0 - min) * bar_height as f32) as i32;

        // Filled region between min and max
        let fill_height = (min_y - max_y).max(0) as u32;
        Rectangle::new(Point::new(bar_x, max_y), Size::new(8, fill_height))
            .into_styled(FILL_STYLE)
            .draw(&mut self.driver)
            .ok();

        // Min indicator (left side)
        Rectangle::new(Point::new(bar_x - 8, min_y - 2), Size::new(6, 3))
            .into_styled(BORDER_STYLE)
            .draw(&mut self.driver)
            .ok();

        // Max indicator (right side)
        Rectangle::new(Point::new(bar_x + 10, max_y - 1), Size::new(6, 3))
            .into_styled(BORDER_STYLE)
            .draw(&mut self.driver)
            .ok();

        self.draw_element_outline_column_with_label(point, "REMP");
    }

    /// Wave teaser cell: outline with waveshape sprite, multiplier, and label
    fn draw_element_wave_teaser(&mut self, point: Point, label: &'static str, slot: &ModSlot) {
        self.draw_waveshape(Point::new(point.x + 9, point.y + 12), slot.wave);

        let mul_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Top)
            .build();

        Text::with_text_style(
            slot.mul.name(),
            Point::new(point.x + 15, point.y + 2),
            CHARACTER_STYLE,
            mul_style,
        )
        .draw(&mut self.driver)
        .ok();

        self.draw_element_outline_with_label(point, label);
    }

    fn draw_element_icon(&mut self, point: Point, label: &'static str, icon: IconSprite) {
        let index = icon as usize;
        self.draw_sprite(
            Point::new(point.x + 10, point.y + 6),
            SpritesheetIndex::Index(index),
            8,
            11,
            11,
            ICONS_BMP,
        );

        self.draw_element_outline_with_label(point, label);
    }

    /// Cell with label centered below the outline
    fn draw_element_outline_with_label(&mut self, point: Point, label: &'static str) {
        self.draw_element_outline(point);

        let centered = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Top)
            .build();

        Text::with_text_style(
            label,
            Point::new(point.x + 15, point.y + 23),
            CHARACTER_STYLE,
            centered,
        )
        .draw(&mut self.driver)
        .ok();
    }

    fn draw_element_title(&mut self, point: Point, label: &'static str, index: usize) {
        self.draw_sprite(
            Point::new(point.x, point.y),
            SpritesheetIndex::Index(index),
            3,
            30,
            64,
            TITLES_BMP,
        );

        Text::with_baseline(
            label,
            Point::new(point.x + 3, point.y + 2),
            CHARACTER_STYLE,
            Baseline::Top,
        )
        .draw(&mut self.driver)
        .ok();
    }

    /// Outline for one grid cell
    fn draw_element_outline(&mut self, point: Point) {
        RoundedRectangle::new(
            Rectangle::new(point, Size::new(30, 23)),
            CornerRadii::new(Size::new(4, 4)),
        )
        .into_styled(BORDER_STYLE)
        .draw(&mut self.driver)
        .ok();
    }

    fn draw_element_outline_column_with_label(&mut self, point: Point, label: &'static str) {
        self.draw_element_outline_column(point);

        let centered = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Top)
            .build();

        Text::with_text_style(
            label,
            Point::new(point.x + 15, point.y + 55),
            CHARACTER_STYLE,
            centered,
        )
        .draw(&mut self.driver)
        .ok();
    }

    /// Outline for two grid cells spanning a column
    fn draw_element_outline_column(&mut self, point: Point) {
        RoundedRectangle::new(
            Rectangle::new(point, Size::new(30, 55)),
            CornerRadii::new(Size::new(4, 4)),
        )
        .into_styled(BORDER_STYLE)
        .draw(&mut self.driver)
        .ok();
    }

    fn draw_waveshape(&mut self, position: Point, shape: Waveshape) {
        let index = shape as usize;
        self.draw_sprite(
            position,
            SpritesheetIndex::Index(index),
            5,
            13,
            7,
            WAVESHAPES_BMP,
        );
    }

    fn draw_sprite(
        &mut self,
        position: Point,
        val: SpritesheetIndex,
        items: usize,
        width: u32,
        height: u32,
        bytes: &'static [u8],
    ) {
        let index = match val {
            SpritesheetIndex::Normalized(v) => {
                let clamped = v.clamp(0.0, 1.0);
                (clamped * (items - 1) as f32) as usize
            }
            SpritesheetIndex::Index(i) => i,
        } % items;

        let x_offset = index as i32 * width as i32;
        let area = Rectangle::new(Point::new(x_offset, 0), Size::new(width, height));

        let bmp = Bmp::<BinaryColor>::from_slice(bytes).unwrap();
        let sprite = bmp.sub_image(&area);
        Image::new(&sprite, position).draw(&mut self.driver).ok();
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
