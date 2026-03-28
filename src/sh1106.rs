use embedded_graphics_core::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::BinaryColor,
    Pixel,
};

pub struct Sh1106<I2C> {
    i2c: I2C,
    addr: u8,
    buffer: [u8; 1024],
    dirty: u8,
}

impl<I2C: embedded_hal_async::i2c::I2c> Sh1106<I2C> {
    /// Create driver instance. Does not touch hardware.
    pub fn new(i2c: I2C, addr: u8) -> Self {
        Self {
            i2c,
            addr,
            buffer: [0u8; 1024],
            dirty: 0,
        }
    }

    /// Send init sequence. Call once after power-on/reset.
    pub async fn init(&mut self) -> Result<(), I2C::Error> {
        self.send_cmds(&[
            0xAE,       // Display OFF
            0xD5, 0x80, // Clock divide ratio = 1, oscillator = default
            0xA8, 0x3F, // Multiplex ratio = 64
            0xD3, 0x00, // Display offset = 0
            0x40,       // Display start line = 0
            0xAD, 0x8B, // DC-DC ON (internal charge pump)
            0xA1,       // Segment remap: column 127 -> SEG0
            0xC8,       // COM scan direction: COM63 -> COM0
            0xDA, 0x12, // COM pins: alternative config
            0x81, 0x80, // Contrast: 0x80 (mid)
            0xD9, 0x22, // Pre-charge: 2 DCLKs / Discharge: 2 DCLKs
            0xDB, 0x35, // VCOM deselect level
            0x32,       // Pump voltage = 8.0V
            0xA6,       // Normal display (not inverted)
            0xAF,       // Display ON
        ])
        .await
    }

    /// Clear framebuffer to all zeros. Marks all pages dirty.
    pub fn clear(&mut self) {
        self.buffer = [0u8; 1024];
        self.dirty = 0xFF;
    }

    /// Set a single pixel. Marks containing page dirty.
    pub fn set_pixel(&mut self, x: u8, y: u8, on: bool) {
        if x >= 128 || y >= 64 {
            return;
        }
        let page = y >> 3;
        let bit = y & 0x07;
        let idx = (page as usize) << 7 | x as usize;
        if on {
            self.buffer[idx] |= 1 << bit;
        } else {
            self.buffer[idx] &= !(1 << bit);
        }
        self.dirty |= 1 << page;
    }

    /// Get raw mutable access to framebuffer.
    /// Caller must call mark_dirty() for affected pages.
    pub fn buffer_mut(&mut self) -> &mut [u8; 1024] {
        &mut self.buffer
    }

    /// Mark a page (0-7) as dirty.
    pub fn mark_dirty(&mut self, page: u8) {
        self.dirty |= 1 << (page & 7);
    }

    /// Mark all pages as dirty.
    pub fn mark_all_dirty(&mut self) {
        self.dirty = 0xFF;
    }

    /// Flush only dirty pages to display. Clears dirty flags.
    pub async fn flush(&mut self) -> Result<(), I2C::Error> {
        for page in 0u8..8 {
            if self.dirty & (1 << page) == 0 {
                continue;
            }
            self.send_page(page).await?;
        }
        self.dirty = 0;
        Ok(())
    }

    /// Flush all 8 pages regardless of dirty flags.
    pub async fn flush_all(&mut self) -> Result<(), I2C::Error> {
        self.dirty = 0xFF;
        self.flush().await
    }

    /// Set contrast (0x00-0xFF). Takes effect immediately.
    pub async fn set_contrast(&mut self, level: u8) -> Result<(), I2C::Error> {
        self.send_cmds(&[0x81, level]).await
    }

    /// Display ON/OFF (sleep mode control).
    pub async fn display_on(&mut self, on: bool) -> Result<(), I2C::Error> {
        self.send_cmds(&[if on { 0xAF } else { 0xAE }]).await
    }

    /// Send a command stream: [0x00, cmd0, cmd1, ...] in a single I2C transaction.
    async fn send_cmds(&mut self, cmds: &[u8]) -> Result<(), I2C::Error> {
        // Max command length: init sequence is 20 bytes + 1 control = 21
        let mut buf = [0u8; 24];
        buf[0] = 0x00; // Co=0, D/C=0 -> command stream
        buf[1..1 + cmds.len()].copy_from_slice(cmds);
        self.i2c.write(self.addr, &buf[..1 + cmds.len()]).await
    }

    /// Write one page of pixel data to the display.
    async fn send_page(&mut self, page: u8) -> Result<(), I2C::Error> {
        // Address setup: set page, column start = 2 (offset for 128-pixel modules)
        self.i2c
            .write(self.addr, &[0x00, 0xB0 | page, 0x02, 0x10])
            .await?;

        // Data: 0x40 control byte + 128 bytes of pixel data
        let mut buf = [0u8; 129];
        buf[0] = 0x40; // Co=0, D/C=1 -> data stream
        let start = (page as usize) << 7;
        buf[1..].copy_from_slice(&self.buffer[start..start + 128]);
        self.i2c.write(self.addr, &buf).await
    }
}

impl<I2C> DrawTarget for Sh1106<I2C>
where
    I2C: embedded_hal_async::i2c::I2c,
{
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            if coord.x >= 0 && coord.x < 128 && coord.y >= 0 && coord.y < 64 {
                self.set_pixel(coord.x as u8, coord.y as u8, color.is_on());
            }
        }
        Ok(())
    }
}

impl<I2C> OriginDimensions for Sh1106<I2C>
where
    I2C: embedded_hal_async::i2c::I2c,
{
    fn size(&self) -> Size {
        Size::new(128, 64)
    }
}
