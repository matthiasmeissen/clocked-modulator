use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config};
use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Sender, Receiver};
use embassy_futures::join::join3;
use heapless::Vec;
use static_cell::StaticCell;
use defmt::*;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

static TX_CHAN: StaticCell<Channel<CriticalSectionRawMutex, Vec<u8, 64>, 4>> = StaticCell::new();

static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static CTRL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static CDC_STATE: StaticCell<State> = StaticCell::new();

pub fn init(
    usb: embassy_rp::Peri<'static, embassy_rp::peripherals::USB>,
    bpm_signal: &'static Signal<CriticalSectionRawMutex, f32>,
    spawner: embassy_executor::Spawner,
) -> Sender<'static, CriticalSectionRawMutex, Vec<u8, 64>, 4> {
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
    let tx_chan = TX_CHAN.init(Channel::new());

    spawner.spawn(usb_task(builder.build(), class, tx_chan.receiver(), bpm_signal)).ok();

    tx_chan.sender()
}

#[embassy_executor::task]
async fn usb_task(
    mut usb: embassy_usb::UsbDevice<'static, Driver<'static, USB>>,
    class: CdcAcmClass<'static, Driver<'static, USB>>,
    tx_recv: Receiver<'static, CriticalSectionRawMutex, Vec<u8, 64>, 4>,
    bpm_signal: &'static Signal<CriticalSectionRawMutex, f32>,
) {
    let (mut usb_tx, mut usb_rx) = class.split();

    let usb_fut = usb.run();

    let write_fut = async {
        loop {
            let data = tx_recv.receive().await;
            let _ = usb_tx.write_packet(&data).await;
            if data.len() == 64 {
                let _ = usb_tx.write_packet(&[]).await;
            }
        }
    };

    let read_fut = async {
        let mut buf = [0u8; 64];
        let mut stash: [u8; 10] = [0; 10];
        let mut stash_len = 0;

        loop {
            match usb_rx.read_packet(&mut buf).await {
                Ok(n) => {
                    for i in 0..n {
                        if stash_len < 10 {
                            stash[stash_len] = buf[i];
                            stash_len += 1;
                        }
                    }

                    let mut i = 0;
                    while i + 5 <= stash_len {
                        if stash[i] == b'B' {
                            let bpm = f32::from_le_bytes([
                                stash[i + 1],
                                stash[i + 2],
                                stash[i + 3],
                                stash[i + 4],
                            ]);
                            bpm_signal.signal(bpm);
                            info!("USB BPM: {}", bpm);

                            let remaining = stash_len - (i + 5);
                            for j in 0..remaining {
                                stash[j] = stash[i + 5 + j];
                            }
                            stash_len = remaining;
                            i = 0;
                        } else {
                            i += 1;
                        }
                    }
                    if stash_len > 5 {
                        stash_len = 0;
                    }
                }
                Err(_) => {
                    stash_len = 0;
                }
            }
        }
    };

    join3(usb_fut, write_fut, read_fut).await;
}
