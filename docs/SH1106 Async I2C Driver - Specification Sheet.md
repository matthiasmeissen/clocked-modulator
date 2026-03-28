# SH1106 Async I2C Display Driver - Specification Sheet

Target: RP2350 + Embassy, `no_std`, `f32`-free, zero external display driver dependencies.

---

## 1. Hardware Reference

### 1.1 Display Controller

- **IC**: SH1106 (Sino Wealth)
- **Internal RAM**: 132 × 64 bits (132 segments × 64 commons)
- **Visible area**: 128 × 64 pixels on most modules (columns 2–129 of the 132-wide RAM)
- **Organization**: 8 pages × 132 columns. Each page is 8 pixels tall. Bit 0 = topmost pixel in that page row.
- **Column auto-increment**: After each data byte write, column address increments by 1. Wraps at column 131. **Does NOT auto-advance to next page.**
- **Contrast**: 256 steps (0x00–0xFF), default 0x80. Current formula: `ISEG = (a/256) × IREF × 16`, where IREF = 12.5µA.
- **Internal charge pump**: Generates VPP (6.4V–9.0V) from VDD2 (3.0V–4.2V). Selectable output: 6.4V / 7.4V / 8.0V (default) / 9.0V.
- **Internal oscillator**: ~360 kHz typical (315–420 kHz). Adjustable ±25% to +50% via command 0xD5.

### 1.2 I2C Specifics

- **Max SCL frequency**: 400 kHz (Fast Mode). This is the hard ceiling from the datasheet.
- **Slave addresses**: `0x3C` (SA0=LOW, most common) or `0x3D` (SA0=HIGH).
- **Bus capacitance limit**: 400 pF per line.
- **Timing** (at VDD1 = 1.65–3.5V):
  - SCL LOW pulse width: ≥ 1.3 µs
  - SCL HIGH pulse width: ≥ 0.6 µs
  - Data setup time: ≥ 100 ns
  - Data hold time: 0–900 ns
  - Rise/fall time: 20 + 0.1×Cb to 300 ns
  - Bus free time (STOP→START): ≥ 1.3 µs

### 1.3 I2C Protocol Frame Structure

Each I2C write transaction after the slave address consists of one or more **control bytes** followed by data bytes:

```
[START] [slave_addr + W] [ACK]
  [control_byte] [ACK] [data_byte] [ACK] ... [STOP]
```

**Control byte** format: `| Co | D/C# | 0 | 0 | 0 | 0 | 0 | 0 |`

- `Co = 1`: Another control+data byte pair follows.
- `Co = 0`: Only data bytes follow until STOP. **This is the fast path.**
- `D/C# = 0`: Following byte(s) are commands.
- `D/C# = 1`: Following byte(s) are display RAM data.

**Optimal I2C writes:**

| Purpose | Control byte | Followed by |
|---|---|---|
| Single command | `0x80` (Co=1, D/C=0) | 1 command byte, then next control+data pair |
| Command stream | `0x00` (Co=0, D/C=0) | N command bytes until STOP |
| Data stream (fast) | `0x40` (Co=0, D/C=1) | N data bytes until STOP |

**Performance-critical insight**: To write one page of display data (128 bytes), issue page/column address commands as a command stream, then start a new I2C transaction with `0x40` + 128 data bytes. This minimizes per-byte overhead. The alternative — interleaving commands and data in one transaction using Co=1 — adds a control byte per switch and prevents DMA-friendly contiguous writes.

### 1.4 Column Offset

The SH1106 has 132-column RAM but most 128-pixel modules map the visible area to columns 2–129. This means:

- Column address lower nibble starts at `0x02` (not `0x00`)
- Column address upper nibble starts at `0x10` (unchanged)
- If you write starting at column 0, the first 2 bytes go to invisible columns (left edge), and the last 2 bytes of a 132-byte write also fall off the visible area (right edge)

**Driver must set column start to 2** (lower=0x02, upper=0x10) before each page write.

---

## 2. Command Reference (I2C, subset relevant to driver)

All commands are sent with D/C# = 0.

### 2.1 Addressing Commands

| Command | Byte(s) | Description |
|---|---|---|
| Set Lower Column Addr | `0x00`–`0x0F` | Lower 4 bits of column address |
| Set Higher Column Addr | `0x10`–`0x1F` | Upper 4 bits of column address |
| Set Page Address | `0xB0`–`0xB7` | Select page 0–7 |

### 2.2 Display Configuration

| Command | Byte(s) | Default | Description |
|---|---|---|---|
| Display OFF | `0xAE` | ON | Enter sleep mode |
| Display ON | `0xAF` | — | Exit sleep mode |
| Set Display Start Line | `0x40`–`0x7F` | `0x40` | Top line = RAM line 0–63 |
| Set Segment Remap | `0xA0` / `0xA1` | `0xA0` | `0xA1` = mirror horizontally (column 127→SEG0) |
| Set COM Scan Direction | `0xC0` / `0xC8` | `0xC0` | `0xC8` = scan from COM63 to COM0 (flip vertically) |
| Set Normal/Inverse | `0xA6` / `0xA7` | `0xA6` | Invert all pixels |
| Entire Display ON/OFF | `0xA4` / `0xA5` | `0xA4` | `0xA5` = force all pixels ON regardless of RAM |

### 2.3 Double-Byte Commands

| Command | Byte 1 | Byte 2 (default) | Description |
|---|---|---|---|
| Set Contrast | `0x81` | `0x80` | 0x00–0xFF (256 steps) |
| Set Multiplex Ratio | `0xA8` | `0x3F` | Mux ratio 1–64. `0x3F` = 64 lines |
| Set Display Offset | `0xD3` | `0x00` | COM shift 0–63 |
| Set Clock Div / Osc Freq | `0xD5` | `0x50` | [7:4] = osc freq, [3:0] = divide ratio−1 |
| Set Pre-charge Period | `0xD9` | `0x22` | [7:4] = discharge, [3:0] = pre-charge (in DCLKs) |
| Set COM Pins Config | `0xDA` | `0x12` | `0x12` = alternative COM pin config |
| Set VCOM Deselect | `0xDB` | `0x35` | VCOM = (0.430 + val×0.006415) × VREF |
| Set DC-DC | `0xAD` | `0x8B` | `0x8B` = internal DC-DC ON on display ON |
| Set Pump Voltage | `0x30`–`0x33` | `0x32` | `0x30`=6.4V, `0x31`=7.4V, `0x32`=8.0V, `0x33`=9.0V |

### 2.4 Read-Modify-Write (not used in this driver)

Commands `0xE0` (start) and `0xEE` (end). Useful for cursor blinking without full page rewrites. Not available in I2C write-only mode. Noted for future reference.

---

## 3. Init Sequence

After hardware reset (RES low ≥ 10µs at VDD1 ≥ 2.4V, or ≥ 10µs at lower VDD1), wait for reset recovery (≤ 2ms):

```
0xAE        // Display OFF
0xD5, 0x80  // Clock divide ratio = 1, oscillator = default
0xA8, 0x3F  // Multiplex ratio = 64
0xD3, 0x00  // Display offset = 0
0x40        // Display start line = 0
0xAD, 0x8B  // DC-DC ON (internal charge pump)
0xA1        // Segment remap: column 127 → SEG0
0xC8        // COM scan direction: COM63 → COM0
0xDA, 0x12  // COM pins: alternative config
0x81, 0x80  // Contrast: 0x80 (mid)
0xD9, 0x22  // Pre-charge: 2 DCLKs / Discharge: 2 DCLKs
0xDB, 0x35  // VCOM deselect level
0x32        // Pump voltage = 8.0V
0xA6        // Normal display (not inverted)
0xAF        // Display ON
```

After Display ON, wait 100ms before sending display data (per datasheet power-on sequence).

**Note**: `0xA1` + `0xC8` together produce the standard "correct orientation" for most modules. Omitting these or using `0xA0` + `0xC0` gives a 180° rotated image.

---

## 4. Page Write Sequence

To update page N (0–7) with 128 bytes of pixel data:

```
// Transaction 1: Set address (command mode)
I2C write to 0x3C:
  [0x00]              // Co=0, D/C=0 → command stream
  [0xB0 | N]          // Set page address
  [0x02]              // Set lower column address = 2 (offset)
  [0x10]              // Set upper column address = 0

// Transaction 2: Write data (data mode)
I2C write to 0x3C:
  [0x40]              // Co=0, D/C=1 → data stream
  [128 bytes of pixel data for this page]
```

Total bytes on wire per page: 4 (addr transaction) + 1 + 128 (data transaction) + I2C framing overhead ≈ **137 bytes** effective payload.

Full screen: 8 pages × 137 bytes ≈ **1096 bytes** + I2C start/stop/addr overhead.

### 4.1 Transfer Time Estimates

| I2C Speed | Time per page | Time full screen | Theoretical max FPS |
|---|---|---|---|
| 100 kHz | ~11 ms | ~88 ms | ~11 FPS |
| 400 kHz | ~2.75 ms | ~22 ms | ~45 FPS |

These assume 9 SCL clocks per byte (8 data + 1 ACK) plus start/stop overhead.

---

## 5. Driver Architecture

### 5.1 Trait Bound

```rust
I2C: embedded_hal_async::i2c::I2c
```

Single trait. No `OutputPin`, no `DelayNs` (the RP2350 Embassy I2C handles timing internally). No `display-interface` crate.

### 5.2 Struct

```rust
pub struct Sh1106<I2C> {
    i2c: I2C,
    addr: u8,                   // 0x3C or 0x3D
    buffer: [u8; 1024],         // 128 × 8 pages = 1024 bytes framebuffer
    dirty: u8,                  // Bitmask: bit N = page N is dirty
}
```

**Memory**: 1024 bytes framebuffer + 1 byte dirty mask + I2C handle. No heap.

### 5.3 Public API

```rust
impl<I2C: embedded_hal_async::i2c::I2c> Sh1106<I2C> {
    /// Create driver instance. Does not touch hardware.
    pub fn new(i2c: I2C, addr: u8) -> Self;

    /// Send init sequence. Call once after power-on/reset.
    pub async fn init(&mut self) -> Result<(), I2C::Error>;

    /// Clear framebuffer to all zeros. Marks all pages dirty.
    pub fn clear(&mut self);

    /// Set a single pixel. Marks containing page dirty.
    pub fn set_pixel(&mut self, x: u8, y: u8, on: bool);

    /// Get raw mutable access to framebuffer (for embedded-graphics DrawTarget).
    /// Caller must call mark_dirty() for affected pages.
    pub fn buffer_mut(&mut self) -> &mut [u8; 1024];

    /// Mark a page (0–7) as dirty.
    pub fn mark_dirty(&mut self, page: u8);

    /// Mark all pages as dirty.
    pub fn mark_all_dirty(&mut self);

    /// Flush only dirty pages to display. Clears dirty flags.
    pub async fn flush(&mut self) -> Result<(), I2C::Error>;

    /// Flush all 8 pages regardless of dirty flags.
    pub async fn flush_all(&mut self) -> Result<(), I2C::Error>;

    /// Set contrast (0x00–0xFF). Takes effect immediately.
    pub async fn set_contrast(&mut self, level: u8) -> Result<(), I2C::Error>;

    /// Display ON/OFF (sleep mode control).
    pub async fn display_on(&mut self, on: bool) -> Result<(), I2C::Error>;
}
```

### 5.4 embedded-graphics Integration

```rust
impl<I2C> DrawTarget for Sh1106<I2C>
where I2C: embedded_hal_async::i2c::I2c
{
    type Color = BinaryColor;
    type Error = core::convert::Infallible;  // draw is framebuffer-only, never fails

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where I: IntoIterator<Item = Pixel<Self::Color>>
    {
        for Pixel(coord, color) in pixels {
            if coord.x >= 0 && coord.x < 128 && coord.y >= 0 && coord.y < 64 {
                let x = coord.x as u8;
                let y = coord.y as u8;
                self.set_pixel(x, y, color.is_on());
            }
        }
        Ok(())
    }
}

impl<I2C> OriginDimensions for Sh1106<I2C>
where I2C: embedded_hal_async::i2c::I2c
{
    fn size(&self) -> Size { Size::new(128, 64) }
}
```

`draw_iter` only touches the framebuffer and dirty flags. The actual I2C transfer happens in `flush()` — this separation is fundamental for async performance.

---

## 6. Internal Implementation Details

### 6.1 send_cmds (command stream)

```
I2C write: [0x00, cmd0, cmd1, cmd2, ...]
```

Single I2C transaction. Control byte `0x00` = Co=0, D/C=0 → all following bytes are commands until STOP.

### 6.2 send_page_data (data stream)

For each dirty page:

```
// Step 1: address setup (3 command bytes)
I2C write: [0x00, 0xB0|page, 0x02, 0x10]

// Step 2: pixel data (128 bytes)
I2C write: [0x40, buffer[page*128 .. (page+1)*128]]
```

**Performance note on step 2**: This requires a 129-byte I2C write (1 control + 128 data). Options:

- **Stack buffer**: Copy 128 bytes from framebuffer into a `[u8; 129]` with `[0] = 0x40`. Simple, costs 129 bytes of stack per flush call. ← **Recommended approach.**
- **Scatter-gather / vectored write**: `embedded_hal_async::i2c::I2c` trait's `write()` takes a single `&[u8]`, not vectored. No way to avoid the copy without a custom I2C extension.
- **Write from framebuffer directly**: Would require `0x40` byte stored contiguously before each 128-byte page in the framebuffer, changing the memory layout. Trades RAM clarity for zero-copy. Complex, not recommended for initial version.

### 6.3 Dirty Page Tracking

```rust
fn set_pixel(&mut self, x: u8, y: u8, on: bool) {
    let page = y >> 3;             // y / 8
    let bit = y & 0x07;           // y % 8
    let idx = (page as usize) << 7 | x as usize;  // page * 128 + x
    if on {
        self.buffer[idx] |= 1 << bit;
    } else {
        self.buffer[idx] &= !(1 << bit);
    }
    self.dirty |= 1 << page;
}
```

`flush()` iterates `for page in 0..8` and skips pages where `dirty & (1 << page) == 0`.

After flushing: `self.dirty = 0;`

### 6.4 DrawTarget dirty tracking

`draw_iter` calls `set_pixel` which marks pages dirty automatically. For `fill_contiguous` or other bulk operations, the page dirty bits accumulate naturally.

---

## 7. Performance Strategy

### 7.1 Async I2C (non-negotiable)

Embassy's `I2c::new_async()` with DMA means `flush().await` yields the executor. The DSP task runs during the entire I2C transfer. This is the single largest performance gain.

### 7.2 400 kHz I2C Clock

Set via `embassy_rp::i2c::Config`. Reduces full-screen flush from ~88ms to ~22ms. Most SH1106 modules handle this without issues.

### 7.3 Dirty Page Flushing

For the Clocked Modulator UI — a few parameter labels + values — typically only 1–3 of 8 pages change per frame. Dirty tracking reduces the common case from ~22ms to ~3–8ms at 400 kHz.

### 7.4 Throttled Update Rate

Display task runs at 10–20 Hz via `Timer::after(Duration::from_millis(50..100)).await`. No perceptual benefit beyond 20 Hz for parameter readouts.

### 7.5 Deferred Rendering

All `embedded-graphics` draw calls write to the in-memory framebuffer (synchronous, fast — just memory writes). The single `flush().await` at the end of the render cycle is the only I2C operation. Never interleave draw + flush.

### 7.6 Transfer Overhead Budget

At 400 kHz, updating 2 dirty pages per frame at 20 Hz:

```
Per page:  4 bytes cmd + 129 bytes data = 133 bytes
           133 × 9 bits / 400,000 Hz ≈ 3.0 ms
Two pages: ~6 ms
At 20 Hz:  6 ms × 20 = 120 ms of I2C per second
           = 12% bus utilization
```

This leaves 88% of I2C bandwidth free for other peripherals on the same bus.

---

## 8. Embassy Task Integration Pattern

```rust
#[embassy_executor::task]
async fn display_task(
    display: Sh1106<I2c<'static, I2C1, i2c::Async>>,
    state: &'static SharedState,  // AtomicU32s, Signals, etc.
) {
    // One-time init
    display.init().await.unwrap();
    display.clear();
    display.flush_all().await.unwrap();
    Timer::after(Duration::from_millis(100)).await;

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    loop {
        // 1. Read shared state (lock-free reads)
        let bpm = state.bpm.load(Ordering::Relaxed);

        // 2. Draw to framebuffer (CPU-only, fast)
        display.clear();
        Text::with_baseline("BPM:", Point::new(0, 0), text_style, Baseline::Top)
            .draw(&mut display).unwrap();
        // ... draw parameter values ...

        // 3. Flush dirty pages (async I2C — yields to executor)
        display.flush().await.unwrap();

        // 4. Throttle
        Timer::after(Duration::from_millis(100)).await;
    }
}
```

The DSP task, USB task, and encoder/button tasks all run cooperatively with this display task. The `flush().await` is the yield point where other tasks get CPU time.

---

## 9. Rust Module Structure

```
src/
  sh1106.rs          // Struct, init, flush, set_pixel, send_cmds, send_page_data
                     // impl DrawTarget, impl OriginDimensions
```

Single file. No `mod.rs`. No sub-modules. No builder pattern — constructor takes `(i2c, addr)` directly.

### 9.1 Dependencies (Cargo.toml)

```toml
[dependencies]
embedded-hal-async = "1"
embedded-graphics-core = "0.4"  # Only the core traits, not full embedded-graphics
```

That's it. Two dependencies, both `no_std`, both trait-only.

---

## 10. Test Plan

1. **Smoke test**: Init → clear → flush_all → confirm blank screen.
2. **Pixel test**: set_pixel at corners (0,0), (127,0), (0,63), (127,63) → flush → confirm 4 dots.
3. **Dirty tracking**: Draw in page 3 only → flush → verify only page 3 is sent (use defmt logging or logic analyzer).
4. **Orientation**: Confirm `0xA1` + `0xC8` gives correct orientation for your specific module. Swap to `0xA0`/`0xC0` if mirrored.
5. **Performance**: Measure time between flush start and completion with `embassy_time::Instant::now()`. Target: < 3ms for single page at 400 kHz.
6. **Integration**: Run display task + DSP task concurrently. Verify DSP output jitter with logic analyzer — should show no degradation compared to display-off baseline.

---

## 11. Key Differences from Existing Crates

| | `sh1106` crate | `oled_async` | This driver |
|---|---|---|---|
| Async | No (blocking) | Yes | Yes |
| `embedded-hal` version | 0.2 | async traits | 1.0 async |
| Dirty page tracking | No | No | Yes |
| Dependencies | `display-interface` + HAL 0.2 | `display-interface`, builder | Only `embedded-hal-async` + `e-g-core` |
| Framebuffer | Yes (1024 B) | Yes | Yes (1024 B) |
| Column offset | Configurable | Configurable | Hardcoded to 2 (can be made configurable later) |
| Module count | ~10 files | ~15 files | 1 file |

---

## 12. Datasheet Reference

- Full datasheet: [Pololu SH1106 PDF](https://www.pololu.com/file/0J1813/SH1106.pdf)
- I2C protocol: Section "I²C-bus Interface", pages 11–14
- Command set: Pages 19–31
- I2C AC timing: Page 43
- Init/power sequence: Pages 32–34
- Application circuit (I2C): Page 48, Figure 15