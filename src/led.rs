use embassy_rp::Peri;
use embassy_rp::peripherals::{PIN_0, PWM_SLICE0};
use embassy_rp::pwm::{Config, Pwm};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Receiver;
use micromath::F32Ext;

use crate::modulator::NUM_MODULATORS;

const PWM_TOP: u16 = 255;

fn f32_to_duty(v: f32) -> u16 {
    (v.clamp(0.0, 1.0).powf(2.2) * PWM_TOP as f32) as u16
}

#[embassy_executor::task]
async fn led_task(
    mut pwm: Pwm<'static>,
    rx: Receiver<'static, CriticalSectionRawMutex, [f32; NUM_MODULATORS], 2>,
) {
    let mut cfg = Config::default();
    cfg.top = PWM_TOP;

    loop {
        let values = rx.receive().await;
        cfg.compare_a = f32_to_duty(values[0]);
        pwm.set_config(&cfg);
    }
}

pub fn init(
    slice: Peri<'static, PWM_SLICE0>,
    pin: Peri<'static, PIN_0>,
    rx: Receiver<'static, CriticalSectionRawMutex, [f32; NUM_MODULATORS], 2>,
    spawner: &embassy_executor::Spawner,
) {
    let mut cfg = Config::default();
    cfg.top = PWM_TOP;

    let pwm = Pwm::new_output_a(slice, pin, cfg);
    spawner.spawn(led_task(pwm, rx)).unwrap();
}
