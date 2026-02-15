use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Receiver;
use embassy_sync::pubsub::Publisher;
use embassy_futures::join::join3;
use static_cell::StaticCell;
use defmt::*;

use crate::modulator::PACKET_SIZE;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static CTRL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static CDC_STATE: StaticCell<State> = StaticCell::new();

pub fn init(
    usb: embassy_rp::Peri<'static, embassy_rp::peripherals::USB>,
    bpm_pub: Publisher<'static, CriticalSectionRawMutex, f32, 2, 2, 1>,
    tx_recv: Receiver<'static, CriticalSectionRawMutex, [u8; PACKET_SIZE], 4>,
    spawner: embassy_executor::Spawner,
) {
    let driver = Driver::new(usb, Irqs);

    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("matthiasmeissen");
    config.product = Some("Clocked Modulator");

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0u8; 256]),
        BOS_DESC.init([0u8; 256]),
        MSOS_DESC.init([0u8; 256]),
        CTRL_BUF.init([0u8; 64]),
    );

    let class = CdcAcmClass::new(&mut builder, CDC_STATE.init(State::new()), 64);

    spawner
        .spawn(usb_task(builder.build(), class, tx_recv, bpm_pub))
        .ok();
}

#[embassy_executor::task]
async fn usb_task(
    mut usb: embassy_usb::UsbDevice<'static, Driver<'static, USB>>,
    class: CdcAcmClass<'static, Driver<'static, USB>>,
    tx_recv: Receiver<'static, CriticalSectionRawMutex, [u8; PACKET_SIZE], 4>,
    bpm_pub: Publisher<'static, CriticalSectionRawMutex, f32, 2, 2, 1>,
) {
    let (mut tx, mut rx) = class.split();

    let usb_fut = usb.run();

    let tx_fut = async {
        loop {
            let data = tx_recv.receive().await;
            let _ = tx.write_packet(&data).await;
        }
    };

    let rx_fut = async {
        let mut buf = [0u8; 64];
        let mut state: u8 = 0;
        let mut bpm_bytes = [0u8; 4];

        loop {
            match rx.read_packet(&mut buf).await {
                Ok(n) => {
                    for &b in &buf[..n] {
                        match state {
                            0 => {
                                if b == b'B' {
                                    state = 1;
                                }
                            }
                            1..=4 => {
                                bpm_bytes[state as usize - 1] = b;
                                state += 1;
                                if state == 5 {
                                    let bpm = f32::from_le_bytes(bpm_bytes);
                                    bpm_pub.publish_immediate(bpm);
                                    info!("USB BPM: {}", bpm);
                                    state = 0;
                                }
                            }
                            _ => state = 0,
                        }
                    }
                }
                Err(_) => state = 0,
            }
        }
    };

    join3(usb_fut, tx_fut, rx_fut).await;
}
