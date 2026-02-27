use defmt::*;
use embassy_time::{Duration, Instant, Timer};
use embassy_rp::gpio::Input;
use rotary_encoder_embedded::{RotaryEncoder, Direction};

use crate::INPUT_EVENTS;

#[derive(Clone, Copy)]
pub enum InputEvent {
    Enc1Rotate(i8),
    Enc2Rotate(i8),
    B1Press,
    B2Press,
    B3Press,
    B4Press,
    B5Press,
    B6Press,
}

impl defmt::Format for InputEvent {
    fn format(&self, f: Formatter) {
        match self {
            InputEvent::Enc1Rotate(v) => defmt::write!(f, "Enc1 {}", v),
            InputEvent::Enc2Rotate(v) => defmt::write!(f, "Enc2 {}", v),
            InputEvent::B1Press => defmt::write!(f, "B1 Press"),
            InputEvent::B2Press => defmt::write!(f, "B2 Press"),
            InputEvent::B3Press => defmt::write!(f, "B3 Press"),
            InputEvent::B4Press => defmt::write!(f, "B4 Press"),
            InputEvent::B5Press => defmt::write!(f, "B5 Press"),
            InputEvent::B6Press => defmt::write!(f, "B6 Press"),
        }
    }
}

pub fn init_encoder(
    spawner: embassy_executor::Spawner,
    button: Input<'static>,
    pin_a: Input<'static>,
    pin_b: Input<'static>,
) {
    spawner.must_spawn(encoder_task(pin_a, pin_b));
    spawner.must_spawn(button_task(button));
}

// TODO: Fix this to handle all buttons (current implementation only for testing)
#[embassy_executor::task]
async fn button_task(mut button: Input<'static>) {
    let debounce = Duration::from_millis(50);

    loop {
        button.wait_for_low().await;
        Timer::after(debounce).await;

        if button.is_low() {
            let press_start = Instant::now();

            button.wait_for_high().await;
            Timer::after(debounce).await;

            let held_ms = press_start.elapsed().as_millis();

            if held_ms <= 500 {
                let _ = INPUT_EVENTS.try_send(InputEvent::B1Press);
            } else {
                let _ = INPUT_EVENTS.try_send(InputEvent::B2Press);
            }
        }
    }
}

// TODO: Fix this to handle both encoders (current implementation only for testing)
#[embassy_executor::task]
async fn encoder_task(pin_a: Input<'static>, pin_b: Input<'static>) {
    let mut encoder = RotaryEncoder::new(pin_a, pin_b).into_standard_mode();

    loop {
        Timer::after(Duration::from_millis(1)).await;
        match encoder.update() {
            Direction::Clockwise => {
                let _ = INPUT_EVENTS.try_send(InputEvent::Enc1Rotate(1));
            }
            Direction::Anticlockwise => {
                let _ = INPUT_EVENTS.try_send(InputEvent::Enc1Rotate(-1));
            }
            Direction::None => {}
        }
    }
}