# Clocked Modulator

![Clocked Modulator](docs/img/clocked-modulator.webp)

four lfos, one clock.  
a small usb box for tempo-synced modulation. each output has its own waveform, cycle length and range. set the tempo by encoder or tap. sends 14-bit midi cc to touchdesigner or any midi host, no driver needed.  
built on raspberry pi pico 2. written in rust.

**[Manual and build guide](https://matthiasmeissen.github.io/clocked-modulator/)**

## What is in this repo

- [Firmware](firmware/) Rust firmware (Cargo project)
- [Hardware](hardware/) PCB source files and fabrication outputs
- [Enclosure](enclosure/) Printable and editable enclosure files
- [Docs](docs/) The manual (also the website)
- [Specs](specs/) Design notes for the UI and the display driver

## License

The firmware is released under the [MIT License](firmware/LICENSE).

<!-- TODO: licence for hardware/, enclosure/ and docs/ is not chosen yet -->
The hardware, enclosure and documentation files do not have a license yet.
