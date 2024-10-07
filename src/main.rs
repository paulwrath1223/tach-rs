

#![no_std]
#![no_main]

mod data_point;
mod elm_commands;
mod elm_uart;
mod errors;
mod display;
mod byte_parsing;
mod gauge;
mod pio_stepper;
mod ws2812;
mod error_lifetime;
mod freq_counter;

use embassy_rp::{bind_interrupts};
use assign_resources::assign_resources;
use embassy_rp::peripherals;
use {defmt_rtt as _, panic_probe as _};
use defmt::*;
use embassy_rp::gpio::Level;
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use crate::data_point::Datum;
use crate::display::display_task;
use crate::elm_uart::elm_uart_task;
use crate::gauge::gauge_task;
use crate::error_lifetime::ErrorFifo;
use crate::errors::{ToRustAGaugeError, ToRustAGaugeErrorSeverity, ToRustAGaugeErrorWithSeverity};

const ERROR_CHECKING_INTERVAL: embassy_time::Duration = embassy_time::Duration::from_millis(1000); // error checking will wait at least this long, maybe more


pub static INCOMING_EVENT_CHANNEL: Channel<CriticalSectionRawMutex, ToMainEvents, 10> = Channel::new();

pub enum ToMainEvents {
    GaugeInitComplete,
    GaugeError(errors::ToRustAGaugeErrorWithSeverity),
    LcdInitComplete,
    LcdError(errors::ToRustAGaugeErrorWithSeverity),
    ElmInitComplete,
    ElmError(errors::ToRustAGaugeErrorWithSeverity),
    ElmDataPoint(data_point::DataPoint),
}

pub static LCD_EVENT_CHANNEL: Channel<CriticalSectionRawMutex, ToLcdEvents, 10> = Channel::new();

pub enum ToLcdEvents {
    NewData(data_point::DataPoint),
    Error(Option<ToRustAGaugeErrorWithSeverity>),
    IsBackLightOn(bool),
}

pub static GAUGE_EVENT_CHANNEL: Channel<CriticalSectionRawMutex, ToGaugeEvents, 10> = Channel::new();
pub enum ToGaugeEvents {
    NewData(data_point::DataPoint),
    IsBackLightOn(bool),
}

assign_resources! { // I hate this macro shit
    elm_uart: ElmUart{
        tx_pin: PIN_0,
        rx_pin: PIN_1,
        uart0: UART0,
        dma0: DMA_CH0,
        dma1: DMA_CH1,
    },
    backlight_sensor: BacklightSensor{
        bl_pin: PIN_14,
    },
    gauge: GaugePins{
        stepper_a1_pin: PIN_4,
        stepper_a2_pin: PIN_5,
        stepper_b1_pin: PIN_6,
        stepper_b2_pin: PIN_7,
        neo_pixel: PIN_3,
        stepper_pio: PIO0,
        led_pio: PIO1,
        led_dma: DMA_CH3
    }
    display: DisplayPins{
        bl: PIN_13,
        bl_pwm: PWM_SLICE6,
        rst: PIN_15,
        display_cs: PIN_9,
        dcx: PIN_8,
        miso: PIN_12,
        mosi: PIN_11,
        clk: PIN_10,
        spi_resource: SPI1,
    }
    freak_counter: FreakyResources{ // freak is short for frequency OFC
        freak_pin: PIN_16,
    }
}

bind_interrupts!(struct Irqs {
    UART0_IRQ => embassy_rp::uart::InterruptHandler<peripherals::UART0>;
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<peripherals::PIO0>; // stepper
    PIO1_IRQ_0 => embassy_rp::pio::InterruptHandler<peripherals::PIO1>; // ws2812
});



#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = embassy_rp::init(Default::default());

    let r = split_resources!(p);
    
    let mut is_backlight_on = true;
    
    let receiver = INCOMING_EVENT_CHANNEL.receiver();
    
    let backlight_input = embassy_rp::gpio::Input::new(r.backlight_sensor.bl_pin, embassy_rp::gpio::Pull::None);
    
    spawner.spawn(gauge_task(r.gauge)).expect("failed to spawn elm uart task");
    spawner.spawn(elm_uart_task(r.elm_uart)).expect("failed to spawn elm uart task");
    spawner.spawn(display_task(r.display)).expect("failed to spawn display task");
    
    let lcd_sender = LCD_EVENT_CHANNEL.sender();
    let gauge_sender = GAUGE_EVENT_CHANNEL.sender();

    let mut error_fifo = ErrorFifo::new();
    
    let mut is_gauge_init: bool = false;
    let mut is_lcd_init: bool = false;

    let mut last_error_check: embassy_time::Instant = embassy_time::Instant::now();
    
    loop {
        let event = receiver.receive().await;

        if last_error_check.elapsed() > ERROR_CHECKING_INTERVAL {
            last_error_check = embassy_time::Instant::now();
            error_fifo.clear_inactive();
            lcd_sender.send(ToLcdEvents::Error(error_fifo.get_most_relevant_error())).await;

            is_backlight_on = match backlight_input.get_level(){
                Level::Low => {true}
                Level::High => {false}
            };
            lcd_sender.send(ToLcdEvents::IsBackLightOn(is_backlight_on)).await;
            gauge_sender.send(ToGaugeEvents::IsBackLightOn(is_backlight_on)).await;
        }

        match event{
            ToMainEvents::GaugeInitComplete => {
                info!("Gauge initialized");
                is_gauge_init = true;
                gauge_sender.send(ToGaugeEvents::IsBackLightOn(is_backlight_on)).await;
            }
            ToMainEvents::GaugeError(e) => {
                warn!("Gauge error: {:?}", e);
                error_fifo.add(e);
            }
            ToMainEvents::LcdInitComplete => {
                info!("LCD initialized");
                is_lcd_init = true;
                lcd_sender.send(ToLcdEvents::IsBackLightOn(is_backlight_on)).await;
                lcd_sender.send(ToLcdEvents::Error(None)).await;
            }
            ToMainEvents::LcdError(e) => {
                warn!("LCD error: {:?}", e);
                error_fifo.add(e);
            }
            ToMainEvents::ElmInitComplete => {
                info!("Elm initialized");
            }
            ToMainEvents::ElmError(e) => {
                warn!("Elm error: {:?}", e);
                error_fifo.add(e);
            }
            ToMainEvents::ElmDataPoint(d) => {
                // info!("Elm data point: {:?}", d);
                let gauge_channel_fifo_length = GAUGE_EVENT_CHANNEL.len();
                if gauge_channel_fifo_length > 4 {
                    defmt::warn!("Gauge event channel overflow. Length: {}", gauge_channel_fifo_length);
                }
                match d.data{
                    Datum::RPM(rpm) => {
                        if !d.data.is_value_sane_check(){
                            defmt::warn!("Insane RPM value: {}, ignoring", rpm);
                            error_fifo.add(ToRustAGaugeErrorWithSeverity{
                                error: ToRustAGaugeError::UnreliableRPM(),
                                severity: ToRustAGaugeErrorSeverity::LossOfSomeFunctionality,
                            });
                        } else {
                            if !d.data.is_value_normal() {
                                defmt::warn!("Received value of dubious validity: {}", d.data);
                                error_fifo.add(ToRustAGaugeErrorWithSeverity{
                                    error: ToRustAGaugeError::StrangeRPM(),
                                    severity: ToRustAGaugeErrorSeverity::MaybeRecoverable,
                                });
                            }
                            if is_gauge_init {
                                gauge_sender.send(ToGaugeEvents::NewData(d)).await;
                            }
                        }
                    }
                    Datum::VBat(vbat) => {
                        if !d.data.is_value_sane_check(){
                            defmt::warn!("Insane VBAT value: {}, ignoring", vbat);
                            error_fifo.add(ToRustAGaugeErrorWithSeverity{
                                error: ToRustAGaugeError::UnreliableVBAT(),
                                severity: ToRustAGaugeErrorSeverity::LossOfSomeFunctionality,
                            });
                        } else {
                            if !d.data.is_value_normal() {
                                defmt::warn!("Received value of dubious validity: {}", d.data);
                                error_fifo.add(ToRustAGaugeErrorWithSeverity{
                                    error: ToRustAGaugeError::StrangeVBAT(),
                                    severity: ToRustAGaugeErrorSeverity::MaybeRecoverable,
                                });
                            }
                            if is_lcd_init {
                                lcd_sender.send(ToLcdEvents::NewData(d)).await;
                            }
                        }
                    }
                    Datum::CoolantTempC(temperature) => {
                        if !d.data.is_value_sane_check(){
                            defmt::warn!("Insane coolant temperature value: {}, ignoring", temperature);
                            error_fifo.add(ToRustAGaugeErrorWithSeverity{
                                error: ToRustAGaugeError::UnreliableCoolant(),
                                severity: ToRustAGaugeErrorSeverity::LossOfSomeFunctionality,
                            });
                        } else {
                            if !d.data.is_value_normal() {
                                defmt::warn!("Received value of dubious validity: {}", d.data);
                                error_fifo.add(ToRustAGaugeErrorWithSeverity{
                                    error: ToRustAGaugeError::StrangeCoolant(),
                                    severity: ToRustAGaugeErrorSeverity::MaybeRecoverable,
                                });
                            }
                            if is_lcd_init {
                                lcd_sender.send(ToLcdEvents::NewData(d)).await;
                            }
                        }
                    }
                }
            }
        }
    }
}
