
use defmt::*;
use embassy_time::{Duration, Instant, Timer};
use embassy_rp::gpio::{Input};

pub fn init_encoder(spawner: embassy_executor::Spawner, button: Input<'static>, pin_a: Input<'static>, pin_b: Input<'static>) {
    spawner.must_spawn(encoder_task(pin_a, pin_b));
    spawner.must_spawn(button_task(button));
}


#[embassy_executor::task]
async fn button_task(mut button: Input<'static>) {
    let debounce = Duration::from_millis(50);

    loop {
        // Wait for press (pull-up → low = pressed)
        button.wait_for_low().await;
        Timer::after(debounce).await;

        if button.is_low() {
            let press_start = Instant::now();
            info!("Button pressed!");

            // Wait for release
            button.wait_for_high().await;
            Timer::after(debounce).await;

            let held_ms = press_start.elapsed().as_millis();
            info!("Button released (held {}ms)", held_ms);
        }
    }
}

#[embassy_executor::task]
async fn encoder_task(mut pin_a: Input<'static>, pin_b: Input<'static>) {
    let mut position: i32 = 0;
    let mut last_a = pin_a.is_low();

    loop {
        // Wait for any edge on pin A
        if last_a {
            pin_a.wait_for_high().await;
        } else {
            pin_a.wait_for_low().await;
        }

        let a = pin_a.is_low();
        let b = pin_b.is_low();

        // Determine direction: if A and B differ → CW, same → CCW
        if a != last_a {
            if a != b {
                position += 1;
            } else {
                position -= 1;
            }
            info!("Position: {}", position);
        }

        last_a = a;
    }
}
