// when you read the name of the file in your head, it is imperative that you think of it as 'freak' counter

use embassy_rp::gpio::Pull;
use crate::{FreakyResources, ToMainEvents, INCOMING_EVENT_CHANNEL};
use embassy_rp::pwm;
use embassy_rp::pwm::InputMode;
use embedded_hal_1::delay::DelayNs;

/// the number of pulses that the RPM signal undergoes in a full rotation of the driveshaft
const RPM_PULSES_PER_REV: f64 = 26f64;

const PULSE_MEASURE_WINDOW_uS: u64 = 100_000;

const MIN_DELAY_BETWEEN_UPDATES: embassy_time::Duration = embassy_time::Duration::from_micros(PULSE_MEASURE_WINDOW_uS);

const PULSE_MEASURE_WINDOW_S: f64 = PULSE_MEASURE_WINDOW_uS as f64 / 1_000_000.0;

#[embassy_executor::task]
pub async fn freq_counter_task(r: FreakyResources) {
    let cfg: pwm::Config = pwm::Config::default();
    let pwm = pwm::Pwm::new_input(r.freak_slice, r.freak_pin, Pull::None, InputMode::RisingEdge, cfg);
    let mut start_time: embassy_time::Instant;
    let mut update_ticker = embassy_time::Ticker::every(MIN_DELAY_BETWEEN_UPDATES);
    let mut pulses: u16;
    loop{
        start_time = embassy_time::Instant::now();
        pwm.set_counter(0);
        
        update_ticker.next().await;
        pulses = pwm.counter();

        let elapsed_time_s = start_time.elapsed().as_micros() as f64 / 1_000_000.0;
        
        send_rpm(((pulses as f64/RPM_PULSES_PER_REV)/elapsed_time_s)*200.0).await;
    }
}


async fn send_rpm(rpm_val: f64){
    defmt::info!("Sending rpm val: {}", rpm_val);
    INCOMING_EVENT_CHANNEL.send(ToMainEvents::FreqCountedRpm(rpm_val)).await;
}