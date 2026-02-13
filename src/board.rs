
use rp235x_hal as hal;
use hal::fugit::RateExtU32;
use panic_halt as _;
use defmt_rtt as _;
use hal::usb::UsbBus;
use usb_device::prelude::*;
use usbd_serial::SerialPort;

use crate::usb;

pub struct Board {
    pub timer: rp235x_hal::Timer<rp235x_hal::timer::CopyableTimer0>,
    //pub pins: rp235x_hal::gpio::Pins,
    pub serial: SerialPort<'static, UsbBus>,
    pub usb_device: UsbDevice<'static, UsbBus>,
    pub i2c: crate::I2CType,
}

impl Board {
    pub fn init() -> Self {
        let mut pac = hal::pac::Peripherals::take().unwrap();

        // Initialize Clocks
        let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
        let clocks = hal::clocks::init_clocks_and_plls(
            12_000_000u32, 
            pac.XOSC, 
            pac.CLOCKS, 
            pac.PLL_SYS, 
            pac.PLL_USB, 
            &mut pac.RESETS, 
            &mut watchdog,
        ).unwrap();

        // Initialize Timer
        let timer: rp235x_hal::Timer<rp235x_hal::timer::CopyableTimer0> = hal::Timer::new_timer0(
            pac.TIMER0, 
            &mut pac.RESETS, 
            &clocks,
        );

        // GPIO and LED init
        let sio = hal::Sio::new(pac.SIO);
        let pins: rp235x_hal::gpio::Pins = hal::gpio::Pins::new(
            pac.IO_BANK0, 
            pac.PADS_BANK0, 
            sio.gpio_bank0, 
            &mut pac.RESETS,
        );

        // Initialize USB
        let (serial, usb_device) = usb::init_usb(
            pac.USB,
            pac.USB_DPRAM,
            clocks.usb_clock,
            &mut pac.RESETS,
        );

        // Init I2C
        let sda_pin = pins.gpio16.reconfigure();
        let scl_pin = pins.gpio17.reconfigure();

        let i2c = hal::I2C::i2c0(
            pac.I2C0,
            sda_pin,
            scl_pin,
            400.kHz(),
            &mut pac.RESETS,
            &clocks.system_clock
        );

        Self { 
            timer, 
            //pins, 
            serial, 
            usb_device,
            i2c,
        }
    }
}