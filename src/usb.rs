use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Receiver;
use embassy_sync::pubsub::Publisher;
use embassy_futures::join::join;
use static_cell::StaticCell;
use defmt::*;

use crate::modulator::PACKET_SIZE;

// Route the USB hardware interrupt to embassy's USB driver
bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

// USB descriptor buffers need StaticCell because the USB driver borrows them
// for the entire device lifetime (they can't be stack-allocated)
static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static CTRL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static CDC_STATE: StaticCell<State> = StaticCell::new();

/// Build the USB CDC-ACM device and spawn the background task.
///
/// - `bpm_pub`: publishes incoming BPM commands to BPM_BUS
/// - `tx_recv`: receives modulator packets from USB_TX channel
pub fn init(
    usb: embassy_rp::Peri<'static, embassy_rp::peripherals::USB>,
    tx_recv: Receiver<'static, CriticalSectionRawMutex, [u8; PACKET_SIZE], 8>,
    spawner: embassy_executor::Spawner,
) {
    let driver = Driver::new(usb, Irqs);

    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("matthiasmeissen");
    config.product = Some("Clocked Modulator");

    // Builder needs mutable references to descriptor buffers.
    // StaticCell::init() gives us &'static mut — valid for the device lifetime.
    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0u8; 256]),
        BOS_DESC.init([0u8; 256]),
        MSOS_DESC.init([0u8; 256]),
        CTRL_BUF.init([0u8; 64]),
    );

    // CDC-ACM: makes the device appear as a serial port on the host
    let class = CdcAcmClass::new(&mut builder, CDC_STATE.init(State::new()), 64);

    spawner
        .spawn(usb_task(builder.build(), class, tx_recv))
        .ok();
}

/// Runs three concurrent futures inside a single task:
/// 1. USB enumeration (responds to host control requests)
/// 2. TX: sends modulator packets to the host
/// 3. RX: parses incoming BPM commands from the host
#[embassy_executor::task]
async fn usb_task(
    mut usb: embassy_usb::UsbDevice<'static, Driver<'static, USB>>,
    class: CdcAcmClass<'static, Driver<'static, USB>>,
    tx_recv: Receiver<'static, CriticalSectionRawMutex, [u8; PACKET_SIZE], 8>,
) {
    let (mut tx, mut rx) = class.split();

    let usb_fut = usb.run();

    let tx_fut = async {
        loop {
            let data = tx_recv.receive().await;
            let _ = tx.write_packet(&data).await;
        }
    };

    join(usb_fut, tx_fut).await;
}
