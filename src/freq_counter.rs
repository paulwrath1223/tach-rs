// when you read the name of the file in your head, it is imperative that you think of it as 'freak' counter

use embassy_rp::interrupt::InterruptExt;
use embassy_time::WithTimeout;
use crate::FreakyResources;


/// the number of pulses that the RPM signal undergoes in a full rotation of the driveshaft
const RPM_PULSES_PER_REV: u8 = 26u8;

/// if no pulses are recorded for this time, we conclude the RPM is zero to avoid delaying the update rate too much
const RPM_SIG_TIMEOUT_DURATION_MS: embassy_time::Duration = embassy_time::Duration::from_millis(20);
// ^ this will affect precision at low RPM's. 
// For reference, at 500rpm and under, the delay between pulses will not exceed 5ms.
// If `RPM_SIG_TIMEOUT_DURATION_MS` were set to 5ms for example, the alg will measure erroneously 
// low values when the real rpm goes below 500 rpm

#[embassy_executor::task]
pub async fn freq_counter_task(r: FreakyResources) {
    let rpm_pin = r.freak_pin;
    let mut rpm_in = embassy_rp::gpio::Input::new(rpm_pin, embassy_rp::gpio::Pull::None);
    loop{
        send_rpm(measure_rpm(&mut rpm_in).await).await;
    }
}


async fn send_rpm(rpm_val: f64){
    defmt::info!("Sending rpm val: {}", rpm_val); //TODO
}

async fn measure_rpm(rpm_in: &mut embassy_rp::gpio::Input<'static>) -> f64{
    let start_time = embassy_time::Instant::now();
    let mut sig_index: u8 = 0;
    while sig_index < RPM_PULSES_PER_REV{
        match rpm_in.wait_for_rising_edge().with_timeout(RPM_SIG_TIMEOUT_DURATION_MS).await {
            Ok(_) => {
                sig_index+=1;
            }
            Err(_) => {
                return 0.0;
            }
        }
    }
    let elapsed_seconds = start_time.elapsed().as_micros() as f64 / 1_000_000.0;
    60.0/elapsed_seconds
}