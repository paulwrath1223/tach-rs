// when you read the name of the file in your head, it is imperative that you think of it as 'freak' counter

use embassy_rp::gpio::Pull;
use crate::{FreakyResources, ToMainEvents, INCOMING_EVENT_CHANNEL};
use embassy_rp::pwm;
use embassy_rp::pwm::InputMode;

/// the number of pulses that the RPM signal undergoes in a full rotation of the driveshaft
const RPM_PULSES_PER_REV: u8 = 26u8;

const MIN_DELAY_BETWEEN_UPDATES: embassy_time::Duration = embassy_time::Duration::from_millis(50);
const ONE_SECOND: embassy_time::Duration = embassy_time::Duration::from_secs(1);


#[embassy_executor::task]
pub async fn freq_counter_task(r: FreakyResources) {
    let cfg: pwm::Config = pwm::Config::default();
    let pwm = pwm::Pwm::new_input(r.freak_slice, r.freak_pin, Pull::None, InputMode::RisingEdge, cfg);
    let mut measured_freq = 0.0;
    let mut start_measure_time: embassy_time::Instant;
    let mut update_ticker = embassy_time::Ticker::every(MIN_DELAY_BETWEEN_UPDATES);
    let mut pulses: u16 = 0;
    loop{
        start_measure_time = embassy_time::Instant::now();
        pwm.set_counter(0);
        update_ticker.next().await;
        pulses = pwm.counter();
        let period_secs = start_measure_time.elapsed().as_micros() as f64 / 1_000_000.0;
        
        send_rpm(pulses as f64/period_secs).await;
    }
}


async fn send_rpm(rpm_val: f64){
    defmt::info!("Sending rpm val: {}", rpm_val);
    INCOMING_EVENT_CHANNEL.send(ToMainEvents::FreqCountedRpm(rpm_val)).await;
}