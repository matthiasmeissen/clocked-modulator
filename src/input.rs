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
    e1_clk: Input<'static>,
    e1_dta: Input<'static>,
    b1: Input<'static>,
    b2: Input<'static>,
) {
    spawner.must_spawn(encoder_task(e1_clk, e1_dta));
    spawner.must_spawn(button_task(b1, InputEvent::B1Press));
    spawner.must_spawn(button_task(b2, InputEvent::B2Press));
}

#[embassy_executor::task(pool_size = 6)]
async fn button_task(mut button: Input<'static>, event: InputEvent) {
    let debounce = Duration::from_millis(50);

    loop {
        button.wait_for_low().await;
        Timer::after(debounce).await;

        if button.is_low() {
            button.wait_for_high().await;
            Timer::after(debounce).await;
            let _ = INPUT_EVENTS.try_send(event);
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