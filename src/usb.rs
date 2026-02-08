use rp235x_hal as hal;
use hal::usb::UsbBus;
use usb_device::class_prelude::*;
use usb_device::prelude::*;
use usbd_serial::SerialPort;
use static_cell::StaticCell;

// Static allocator with 'static lifetime using StaticCell
static USB_BUS: StaticCell<UsbBusAllocator<UsbBus>> = StaticCell::new();

pub fn init_usb(
    usb: hal::pac::USB,
    usb_dpram: hal::pac::USB_DPRAM,
    usb_clock: hal::clocks::UsbClock,
    resets: &mut hal::pac::RESETS,
) -> (SerialPort<'static, UsbBus>, UsbDevice<'static, UsbBus>) {

    // Initialize the USB bus allocator - StaticCell handles the safety for us
    let usb_bus = USB_BUS.init(UsbBusAllocator::new(hal::usb::UsbBus::new(
        usb,
        usb_dpram,
        usb_clock,
        true,
        resets,
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

    (serial, usb_device)
}
