use embassy_rp::Peri;
use embassy_rp::peripherals::{PIN_0, PIN_1, PIN_2, PIN_3, PWM_SLICE0, PWM_SLICE1};
use embassy_rp::pwm::{Config, Pwm};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Receiver;
use micromath::F32Ext;

use crate::modulator::NUM_MODULATORS;

const PWM_TOP: u16 = 255;

fn f32_to_duty(v: f32) -> u16 {
    (v.clamp(0.0, 1.0).powf(2.2) * PWM_TOP as f32) as u16
}

#[embassy_executor::task]
async fn led_task(
    mut pwm0: Pwm<'static>,
    mut pwm1: Pwm<'static>,
    rx: Receiver<'static, ThreadModeRawMutex, [f32; NUM_MODULATORS], 2>,
) {
    let mut cfg0 = Config::default();
    cfg0.top = PWM_TOP;
    let mut cfg1 = Config::default();
    cfg1.top = PWM_TOP;

    loop {
        let values = rx.receive().await;
        cfg0.compare_a = f32_to_duty(values[0]);
        cfg0.compare_b = f32_to_duty(values[1]);
        cfg1.compare_a = f32_to_duty(values[2]);
        cfg1.compare_b = f32_to_duty(values[3]);
        pwm0.set_config(&cfg0);
        pwm1.set_config(&cfg1);
    }
}

pub fn init(
    slice0: Peri<'static, PWM_SLICE0>,
    pin0: Peri<'static, PIN_0>,
    pin1: Peri<'static, PIN_1>,
    slice1: Peri<'static, PWM_SLICE1>,
    pin2: Peri<'static, PIN_2>,
    pin3: Peri<'static, PIN_3>,
    rx: Receiver<'static, ThreadModeRawMutex, [f32; NUM_MODULATORS], 2>,
    spawner: &embassy_executor::Spawner,
) {
    let mut cfg = Config::default();
    cfg.top = PWM_TOP;

    let pwm0 = Pwm::new_output_ab(slice0, pin0, pin1, cfg.clone());
    let pwm1 = Pwm::new_output_ab(slice1, pin2, pin3, cfg);
    spawner.spawn(led_task(pwm0, pwm1, rx)).unwrap();
}
