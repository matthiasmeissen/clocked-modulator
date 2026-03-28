use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_usb::class::midi::MidiClass;
use embassy_usb::{Builder, Config};
use embassy_futures::join::join;
use embassy_time::{Duration, Ticker};
use static_cell::StaticCell;

use crate::USB_TX;
use crate::modulator::MIDI_PACKET_SIZE;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static CTRL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

pub fn init(
    usb: embassy_rp::Peri<'static, embassy_rp::peripherals::USB>,
    spawner: embassy_executor::Spawner,
) {
    let driver = Driver::new(usb, Irqs);

    let mut config = Config::new(0xc0de, 0xcaf1);
    config.manufacturer = Some("matthiasmeissen");
    config.product = Some("Clocked Modulator");
    config.serial_number = Some("CM-001");

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0u8; 256]),
        BOS_DESC.init([0u8; 256]),
        MSOS_DESC.init([0u8; 256]),
        CTRL_BUF.init([0u8; 64]),
    );

    let midi = MidiClass::new(&mut builder, 1, 1, 64);

    spawner
        .spawn(usb_task(builder.build(), midi))
        .ok();
}

#[embassy_executor::task]
async fn usb_task(
    mut usb: embassy_usb::UsbDevice<'static, Driver<'static, USB>>,
    midi: MidiClass<'static, Driver<'static, USB>>,
) {
    let (mut sender, _receiver) = midi.split();

    let usb_fut = usb.run();

    let tx_fut = async {
        let mut ticker = Ticker::every(Duration::from_millis(8)); // 125Hz

        loop {
            ticker.next().await;

            let frame = USB_TX.wait().await;

            let _ = sender.write_packet(&frame).await;
        }
    };

    join(usb_fut, tx_fut).await;
}