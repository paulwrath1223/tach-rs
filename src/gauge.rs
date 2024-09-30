use embassy_rp::pio::{Instance, Pio};
use smart_leds::RGB8;
use crate::{GaugePins, Irqs, ToMainEvents, GAUGE_EVENT_CHANNEL, INCOMING_EVENT_CHANNEL};
use crate::pio_stepper::PioStepper;
use crate::ws2812::Ws2812;
const STEPPER_SM: usize = 0;
#[embassy_executor::task]
pub async fn gauge_task(r: GaugePins) {
    let receiver = GAUGE_EVENT_CHANNEL.receiver();
    let sender = INCOMING_EVENT_CHANNEL.sender();

    let Pio { mut common, sm0, .. } = Pio::new(r.led_pio, Irqs);

    // This is the number of leds in the string. Helpfully, the sparkfun thing plus and adafruit
    // feather boards for the 2040 both have one built in.
    const NUM_LEDS: usize = 24;
    let mut data: [RGB8; NUM_LEDS];

    // Common neopixel pins:
    // Thing plus: 8
    // Adafruit Feather: 16;  Adafruit Feather+RFM95: 4
    let mut ws2812: Ws2812<embassy_rp::peripherals::PIO1, 0, NUM_LEDS> = Ws2812::new(&mut common, sm0, r.led_dma, r.neo_pixel);
    
    let Pio {
        mut common, irq0, sm0, ..
    } = Pio::new(r.stepper_pio, Irqs);
    

    let mut stepper = PioStepper::new(
        &mut common,
        sm0,
        irq0,
        r.stepper_a1_pin,
        r.stepper_a2_pin,
        r.stepper_b1_pin,
        r.stepper_b2_pin
    );
    stepper.set_frequency(120);
    
    sender.send(ToMainEvents::GaugeInitComplete).await;
    let green = RGB8::new(0, 255, 0);
    let red = RGB8::new(255, 0, 0);
    
    loop {
        data = [green; 24];
        ws2812.write(&data).await;
        stepper.step2(100).await;
        data = [red; 24];
        ws2812.write(&data).await;
        stepper.step2(-100).await;
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
        self.pio_stepper.step2(-1000).await;
        self.current_position = Some(0);
    }
    
    /// ! dropping this future will cause a disconnect between the actual and internal position of the stepper 
    pub async fn set_position(&mut self, target_position: u32){
        let delta: i32 = target_position as i32 - self.current_position
                .expect("tried to set stepper pos before calibration") as i32;
        self.pio_stepper.step2(delta).await;
        self.current_position = Some(target_position);
    }
}