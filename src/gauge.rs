
use embassy_futures::join::join;
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::Pio;
use smart_leds::RGB8;
use crate::{GaugePins, Irqs, ToGaugeEvents, ToLcdEvents, ToMainEvents, GAUGE_EVENT_CHANNEL, INCOMING_EVENT_CHANNEL};
use crate::data_point::{DataPoint, Datum};
use crate::errors::{ToRustAGaugeErrorSeverity, ToRustAGaugeErrorWithSeverity};
use crate::pio_servo::{PwmPio, ServoBuilder, ServoDegrees};
use crate::ws2812::Ws2812;


// this file uses both `embassy_time::Duration` and `core::time::Duration`. Be careful

const NUM_LEDS: usize = 32;

const WHITE: RGB8 = RGB8 { r: 255, g: 255, b: 255 };
const BLACK: RGB8 = RGB8 { r: 0, g: 0, b: 0 };
const BACKLIGHT_BRIGHT_BRIGHTNESS_MULTIPLIER: f32 = 1.0;
const BACKLIGHT_DIM_BRIGHTNESS_MULTIPLIER: f32 = 0.5;

/// Wait at least this long between updates to servo and LEDs. This is done because the servo signal
/// has a 20ms period, and only one 'command' can be sent during that time
const MIN_UPDATE_DELAY: embassy_time::Duration = embassy_time::Duration::from_millis(50);

/// the maximum RPM value that can be displayed. Higher values will be checked for and handled,
/// but this value is used for scaling.
const GAUGE_MAX_RPM: f64 = 9000.0;


#[embassy_executor::task]
pub async fn gauge_task(r: GaugePins) {
    let receiver = GAUGE_EVENT_CHANNEL.receiver();
    let sender = INCOMING_EVENT_CHANNEL.sender();

    let mut neo_p_data: [RGB8; NUM_LEDS] = [BLACK; NUM_LEDS];

    let Pio { mut common, sm0, .. } = Pio::new(r.led_pio, Irqs);
    let mut ws2812: Ws2812<embassy_rp::peripherals::PIO1, 0, NUM_LEDS> = Ws2812::new(&mut common, sm0, r.led_dma, r.neo_pixel);


    let Pio { mut common, sm0, .. } = Pio::new(r.servo_pio, Irqs);
    let pwm_pio = PwmPio::new(&mut common, sm0, r.servo_pin);
    let mut servo = ServoBuilder::new(pwm_pio)
        .set_max_degree_rotation(270.0)
        .set_min_pulse_width(core::time::Duration::from_micros(500))
        .set_max_pulse_width(core::time::Duration::from_micros(2500))
        .build();

    servo.start();
    sender.send(ToMainEvents::GaugeInitComplete).await;
    
    let mut is_backlight_on = false;
    let mut ticker = embassy_time::Ticker::every(MIN_UPDATE_DELAY);
    loop {
        ticker.next().await;
        match receiver.receive().await {
            ToGaugeEvents::NewData(data) => {
                match data.data {
                    Datum::RPM(rpm) => {
                        do_backlight(&mut neo_p_data, rpm, is_backlight_on);
                        ws2812.write(&neo_p_data).await;
                        servo.rotate(rpm_to_servo_degrees(rpm))
                    }
                    _ => {defmt::error!("Gauge received data point containing data that isn't RPM. Ignoring")}
                }
            }
            ToGaugeEvents::IsBackLightOn(new_bl_state) => {
                is_backlight_on = new_bl_state;
            }
        }
    }
}


/// Input a value 0 to 255 to get a color value
/// The colours are a transition r - g - b - back to r.
fn wheel(mut wheel_pos: u8) -> RGB8 {
    wheel_pos = 128 - wheel_pos;
    if wheel_pos < 85 {
        return (255 - wheel_pos * 3, 0, wheel_pos * 3).into();
    }
    if wheel_pos < 170 {
        wheel_pos -= 85;
        return (0, wheel_pos * 3, 255 - wheel_pos * 3).into();
    }
    wheel_pos -= 170;
    (wheel_pos * 3, 255 - wheel_pos * 3, 0).into()
}


fn do_backlight(neo_p_data: &mut [RGB8; NUM_LEDS], value: f64, is_backlight_on: bool){


    const NUMERICAL_BACK_LIGHT_START_INDEX: usize = 4;
    const NEEDLE_BACKLIGHT_START_INDEX: usize = 29;
    const FINAL_INDICATOR_START_INDEX: usize = 31;

    const NUM_IND_LEDS: f64 = NEEDLE_BACKLIGHT_START_INDEX as f64 - NUMERICAL_BACK_LIGHT_START_INDEX as f64;

    let rpm_index_in_indicator_leds: usize = ((NUM_IND_LEDS * value) / GAUGE_MAX_RPM)
        .clamp(0.0, NUM_IND_LEDS) as usize;

    
    let dim_factor: f32 = if is_backlight_on {
        BACKLIGHT_BRIGHT_BRIGHTNESS_MULTIPLIER
    } else {
        BACKLIGHT_DIM_BRIGHTNESS_MULTIPLIER
    };
    for i in 0..NUMERICAL_BACK_LIGHT_START_INDEX {
        neo_p_data[i] = BLACK;
    }
    for i in NUMERICAL_BACK_LIGHT_START_INDEX..NEEDLE_BACKLIGHT_START_INDEX {
        let indicator_index = i-NUMERICAL_BACK_LIGHT_START_INDEX;
        if indicator_index <= rpm_index_in_indicator_leds{
            neo_p_data[i] = dim_color_by_factor(wheel(((indicator_index*10)%256) as u8), dim_factor);
        } else {
            neo_p_data[i] = BLACK;
        }
    }
    for i in NEEDLE_BACKLIGHT_START_INDEX..FINAL_INDICATOR_START_INDEX {
        neo_p_data[i] = dim_color_by_factor(WHITE, dim_factor);
    }
    for i in FINAL_INDICATOR_START_INDEX..NUM_LEDS {
        neo_p_data[i] = BLACK;
    }
}

fn dim_color_by_factor(color: RGB8, factor: f32) -> RGB8 {
    RGB8{
        r: (color.r as f32 * factor).clamp(0.0, 255.0) as u8,
        g: (color.g as f32 * factor).clamp(0.0, 255.0) as u8,
        b: (color.b as f32 * factor).clamp(0.0, 255.0) as u8,
    }
}

fn rpm_to_servo_degrees(rpm: f64) -> ServoDegrees{
    const MAX_DEGREES: f64 = 270.0;
    
    const FACTOR: f64 = MAX_DEGREES/GAUGE_MAX_RPM;

    MAX_DEGREES - (rpm * FACTOR)
}