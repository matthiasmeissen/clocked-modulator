# Clocked Modulator

<!-- TODO: one photo of the finished device, e.g. ![Clocked Modulator](docs/img/hero.jpg) -->

A small USB box that generates four tempo-synced LFOs. Each output has its own
waveform, cycle length and range, and all four follow one shared clock that you
set by encoder or tap tempo. The values are sent to the computer as 14-bit USB
MIDI CC, so TouchDesigner or any other MIDI host can use them without a driver.
It runs on a Raspberry Pi Pico 2 and the firmware is written in Rust with Embassy.

**[Manual and build guide](https://matthiasmeissen.github.io/clocked-modulator/)**

## What is in this repo

| Folder | Contents |
| --- | --- |
| [firmware/](firmware/) | Rust firmware (Cargo project) |
| [hardware/](hardware/) | PCB source files and fabrication outputs |
| [enclosure/](enclosure/) | Printable and editable enclosure files |
| [docs/](docs/) | The manual (also the website) and design notes |

## Install the firmware

Without Rust: download `clocked-modulator.uf2` from the
[latest release](https://github.com/matthiasmeissen/clocked-modulator/releases/latest),
hold BOOTSEL while plugging in the Pico 2, and drag the file onto the `RP2350` drive.

From source, with a debug probe connected:

```sh
cd firmware
cargo run --release
```

The [manual](https://matthiasmeissen.github.io/clocked-modulator/) covers
wiring, assembly and how to use the device.

## License

The firmware is released under the [MIT License](firmware/LICENSE).

<!-- TODO: licence for hardware/, enclosure/ and docs/ is not chosen yet -->
The hardware, enclosure and documentation files do not have a license yet.
