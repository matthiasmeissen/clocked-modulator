
use rp235x_hal as hal;
use hal::fugit::RateExtU32;
use hal::usb::UsbBus;
use usb_device::prelude::*;
use usbd_serial::SerialPort;
use usb_device::class_prelude::*;
use static_cell::StaticCell;

pub type I2CType = rp235x_hal::I2C<rp235x_hal::pac::I2C0, (rp235x_hal::gpio::Pin<rp235x_hal::gpio::bank0::Gpio16, rp235x_hal::gpio::FunctionI2c, rp235x_hal::gpio::PullUp>, rp235x_hal::gpio::Pin<rp235x_hal::gpio::bank0::Gpio17, rp235x_hal::gpio::FunctionI2c, rp235x_hal::gpio::PullUp>)>;
pub type LedPinType = rp235x_hal::gpio::Pin<rp235x_hal::gpio::bank0::Gpio25, rp235x_hal::gpio::FunctionSio<rp235x_hal::gpio::SioOutput>, rp235x_hal::gpio::PullDown>;

static USB_BUS: StaticCell<UsbBusAllocator<UsbBus>> = StaticCell::new();

pub struct Board {
    pub timer: rp235x_hal::Timer<rp235x_hal::timer::CopyableTimer0>,
    pub serial: SerialPort<'static, UsbBus>,
    pub usb_device: UsbDevice<'static, UsbBus>,
    pub i2c: I2CType,

    pub led_pin: LedPinType,
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

        // Initialize Pins
        let sio = hal::Sio::new(pac.SIO);
        let pins: rp235x_hal::gpio::Pins = hal::gpio::Pins::new(
            pac.IO_BANK0, 
            pac.PADS_BANK0, 
            sio.gpio_bank0, 
            &mut pac.RESETS,
        );

        // Initialize LED pins
        let led_pin = pins.gpio25.into_push_pull_output();

        // Initialize USB
        let usb_bus = USB_BUS.init(UsbBusAllocator::new(hal::usb::UsbBus::new(
            pac.USB,
            pac.USB_DPRAM,
            clocks.usb_clock,
            true,
            &mut pac.RESETS,
        )));

        let serial = SerialPort::new(usb_bus);

        let usb_device = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x16c0, 0x27dd))
            .strings(&[StringDescriptors::default()
                .manufacturer("matthiasmeissen")
                .product("ClockedModulator")
                .serial_number("RP2350")])
            .unwrap()
            .device_class(2)
            .build();

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
            serial, 
            usb_device,
            i2c,
            led_pin
        }
    }
}
