use embassy_rp::gpio::{Level, Output};
use embassy_rp::Peri;
use embassy_rp::peripherals::PIN_0;
use embassy_time::{Duration, Timer};

#[embassy_executor::task]
async fn blink_task(mut led: Output<'static>) {
    loop {
        led.set_high();
        Timer::after(Duration::from_millis(500)).await;
        led.set_low();
        Timer::after(Duration::from_millis(500)).await;
    }
}

pub fn init(pin: Peri<'static, PIN_0>, spawner: &embassy_executor::Spawner) {
    let led = Output::new(pin, Level::Low);
    spawner.spawn(blink_task(led)).unwrap();
}
