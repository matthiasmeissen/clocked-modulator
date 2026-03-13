use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_usb::class::midi::MidiClass;
use embassy_usb::{Builder, Config};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Receiver;
use embassy_futures::join::join;
use static_cell::StaticCell;

use crate::modulator::{MIDI_FRAME_SIZE, MIDI_PACKET_SIZE};

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

/// Build the USB MIDI device and spawn the background task.
pub fn init(
    usb: embassy_rp::Peri<'static, embassy_rp::peripherals::USB>,
    tx_recv: Receiver<'static, CriticalSectionRawMutex, [u8; MIDI_FRAME_SIZE], 8>,
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

    // MIDI class: 1 in jack + 1 out jack (both needed for macOS to expose as input source)
    let midi = MidiClass::new(&mut builder, 1, 1, 64);

    spawner
        .spawn(usb_task(builder.build(), midi, tx_recv))
        .ok();
}

/// Runs USB enumeration + MIDI packet output concurrently.
#[embassy_executor::task]
async fn usb_task(
    mut usb: embassy_usb::UsbDevice<'static, Driver<'static, USB>>,
    midi: MidiClass<'static, Driver<'static, USB>>,
    tx_recv: Receiver<'static, CriticalSectionRawMutex, [u8; MIDI_FRAME_SIZE], 8>,
) {
    let (mut sender, _receiver) = midi.split();

    let usb_fut = usb.run();

    let tx_fut = async {
        loop {
            let frame = tx_recv.receive().await;
            // Send each 4-byte MIDI packet individually
            for chunk in frame.chunks(MIDI_PACKET_SIZE) {
                let _ = sender.write_packet(chunk).await;
            }
        }
    };

    join(usb_fut, tx_fut).await;
}
