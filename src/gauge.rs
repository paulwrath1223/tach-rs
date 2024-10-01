use embassy_futures::join::join;
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::Pio;
use smart_leds::RGB8;
use crate::{GaugePins, Irqs, ToGaugeEvents, ToLcdEvents, ToMainEvents, GAUGE_EVENT_CHANNEL, INCOMING_EVENT_CHANNEL};
use crate::data_point::{DataPoint, Datum};
use crate::errors::ToRustAGaugeError::UnreliableRPM;
use crate::errors::{ToRustAGaugeErrorSeverity, ToRustAGaugeErrorWithSeverity};
use crate::pio_stepper::PioStepper;
use crate::ws2812::Ws2812;
const STEPPER_SM: usize = 0;
const NUM_LEDS: usize = 24;
const NUM_LABEL_LEDS: usize = 5;
const LED_ZERO_OFFSET: usize = 12;

const WHITE: RGB8 = RGB8 { r: 255, g: 255, b: 255 };
const BLACK: RGB8 = RGB8 { r: 0, g: 0, b: 0 };
const BACKLIGHT_BRIGHT_BRIGHTNESS_MULTIPLIER: f32 = 1.0;
const BACKLIGHT_DIM_BRIGHTNESS_MULTIPLIER: f32 = 0.5;
#[embassy_executor::task]
pub async fn gauge_task(r: GaugePins) {
    let receiver = GAUGE_EVENT_CHANNEL.receiver();
    let sender = INCOMING_EVENT_CHANNEL.sender();

    let Pio { mut common, sm0, .. } = Pio::new(r.led_pio, Irqs);

    // This is the number of leds in the string. Helpfully, the sparkfun thing plus and adafruit
    // feather boards for the 2040 both have one built in.

    let mut neo_p_data: [RGB8; NUM_LEDS] = [BLACK; NUM_LEDS];

    // Common neopixel pins:
    // Thing plus: 8
    // Adafruit Feather: 16;  Adafruit Feather+RFM95: 4
    let mut ws2812: Ws2812<embassy_rp::peripherals::PIO1, 0, NUM_LEDS> = Ws2812::new(&mut common, sm0, r.led_dma, r.neo_pixel);
    
    let Pio {
        mut common, irq0, sm0, ..
    } = Pio::new(r.stepper_pio, Irqs);
    

    let mut pio_stepper = PioStepper::new(
        &mut common,
        sm0,
        irq0,
        r.stepper_a1_pin,
        r.stepper_a2_pin,
        r.stepper_b1_pin,
        r.stepper_b2_pin
    );
    pio_stepper.set_frequency(120);

    let mut stepper: PositionalStepper<'static, PIO0> = PositionalStepper{
        current_position: None,
        pio_stepper,
    };
    stepper.calibrate().await;
    
    sender.send(ToMainEvents::GaugeInitComplete).await;

    let mut is_backlight_on = false;
    
    loop {
        match receiver.receive().await {
            ToGaugeEvents::NewData(data) => {
                match data.data {
                    Datum::RPM(rpm) => {
                        do_backlight(&mut neo_p_data, rpm, is_backlight_on);
                        join(ws2812.write(&neo_p_data), stepper.set_position_from_val(rpm)).await;
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
    wheel_pos = 255 - wheel_pos;
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

pub struct PositionalStepper<'a, T: embassy_rp::pio::Instance>{
    current_position: Option<u32>, // none if uncalibrated
    pio_stepper: PioStepper<'a, T, { STEPPER_SM }>,
}

impl<'a, T: embassy_rp::pio::Instance> PositionalStepper<'a, T> {
    pub async fn calibrate(&mut self){
        self.pio_stepper.step2(-800).await;
        self.current_position = Some(0);
    }
    
    /// ! dropping this future will cause a disconnect between the actual and internal position of the stepper 
    pub async fn set_position(&mut self, target_position: u32){
        let delta: i32 = target_position as i32 - self.current_position
                .expect("tried to set stepper pos before calibration") as i32;
        self.current_position = Some(target_position);
        self.pio_stepper.step2(delta).await;
    }
    
    /// if this future is dropped, the motor must be recalibrated
    pub async fn set_position_from_val(&mut self, value: f64){
        let scaled_value = (540.0 * value / 9000.0).clamp(0.0, 540.0) as u32;
        self.set_position(scaled_value).await;
    }
}

fn do_backlight(neo_p_data: &mut [RGB8; NUM_LEDS], value: f64, is_backlight_on: bool){
    
    let normalized_val: usize = (19.0 * value / 9000.0).clamp(0.0, 19.0) as usize;
    
    let dim_factor: f32 = if is_backlight_on {
        BACKLIGHT_BRIGHT_BRIGHTNESS_MULTIPLIER
    } else {
        BACKLIGHT_DIM_BRIGHTNESS_MULTIPLIER
    };
    
    for i in 0..NUM_LEDS {
        let offset_index = (i + LED_ZERO_OFFSET) % NUM_LEDS;
        if offset_index <= normalized_val {
            neo_p_data[offset_index] = dim_color_by_factor(wheel(((i*12)%256) as u8), dim_factor);
        } else if offset_index > NUM_LEDS-NUM_LABEL_LEDS {
            neo_p_data[offset_index] = dim_color_by_factor(WHITE, dim_factor);
        } else {
            neo_p_data[offset_index] = BLACK;
        }
    }
}

fn dim_color_by_factor(color: RGB8, factor: f32) -> RGB8 {
    RGB8{
        r: (color.r as f32 * factor).clamp(0.0, 255.0) as u8,
        g: (color.g as f32 * factor).clamp(0.0, 255.0) as u8,
        b: (color.b as f32 * factor).clamp(0.0, 255.0) as u8,
    }
}
