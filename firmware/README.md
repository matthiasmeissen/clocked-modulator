# Firmware

Rust firmware for the Raspberry Pi Pico 2 (RP2350), built on Embassy.

## Build and flash

Run all commands from this folder, because Cargo reads `.cargo/config.toml` from here.

```sh
rustup target add thumbv8m.main-none-eabihf
cargo run --release
```

This flashes through a debug probe with [probe-rs](https://probe.rs) and shows
the `defmt` log output. To flash over USB in BOOTSEL mode instead, switch the
`runner` line in `.cargo/config.toml` to the `picotool` one.

### Debug probe wiring

With a female pin cable and the Pico's USB port pointing left, the debug pins
are on the right. Top to bottom: yellow (data), black (ground), orange (clock).

## Make a release file

```sh
cargo build --release
picotool uf2 convert target/thumbv8m.main-none-eabihf/release/clocked-modulator -t elf target/clocked-modulator.uf2
```

Attach `target/clocked-modulator.uf2` to a GitHub Release.

## Display graphics

The sprite sheets in `assets/` are drawn in Aseprite and exported as 24-bit
BMP. Convert them to 1-bit to save flash space:

```sh
ffmpeg -i source.bmp -pix_fmt monob target_1bit.bmp -y
```

## Resources

- [Pico 2 Book](https://pico.implrust.com/index.html)
- [rp-235x template](https://github.com/rp-rs/rp235x-project-template/tree/main)
- [pico 2 template](https://github.com/ImplFerris/pico2-template/tree/main)
- [rp235x hal examples](https://github.com/rp-rs/rp-hal/tree/main/rp235x-hal-examples)
- [embassy rp235x examples](https://github.com/embassy-rs/embassy/tree/main/examples/rp235x)
- [Pico 2 Pinout](https://www.raspberrypi.com/documentation/microcontrollers/images/pico-2-r4-pinout.svg)
