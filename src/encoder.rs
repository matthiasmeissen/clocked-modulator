use defmt::*;
use embassy_time::{Duration, Instant, Timer};
use embassy_rp::gpio::Input;
use rotary_encoder_embedded::{RotaryEncoder, Direction};

#[derive(Clone, Copy)]
pub enum InputEvent {
    Prev,
    Next,
    Enter,
    Back,
}

impl defmt::Format for InputEvent {
    fn format(&self, f: Formatter) {
        match self {
            InputEvent::Prev => defmt::write!(f, "Prev"),
            InputEvent::Next => defmt::write!(f, "Next"),
            InputEvent::Enter => defmt::write!(f, "Enter"),
            InputEvent::Back => defmt::write!(f, "Back"),
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

#[embassy_executor::task]
async fn button_task(mut button: Input<'static>) {
    let debounce = Duration::from_millis(50);

    loop {
        button.wait_for_low().await;
        Timer::after(debounce).await;

        if button.is_low() {
            let press_start = Instant::now();
            info!("Button pressed!");

            button.wait_for_high().await;
            Timer::after(debounce).await;

            let held_ms = press_start.elapsed().as_millis();
            info!("Button released (held {}ms)", held_ms);
        }
    }
}

#[embassy_executor::task]
async fn encoder_task(pin_a: Input<'static>, pin_b: Input<'static>) {
    let mut encoder = RotaryEncoder::new(pin_a, pin_b).into_standard_mode();
    let mut position: i32 = 0;

    loop {
        Timer::after(Duration::from_millis(1)).await;
        match encoder.update() {
            Direction::Clockwise => {
                position += 1;
                info!("CW  → Position: {}", position);
            }
            Direction::Anticlockwise => {
                position -= 1;
                info!("CCW → Position: {}", position);
            }
            Direction::None => {}
        }
    }
}