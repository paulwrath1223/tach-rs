//! This example shows how to use USB (Universal Serial Bus) in the RP2040 chip.
//!
//! This creates the possibility to send info/warn/error/debug! to USB serial port.

#![no_std]
#![no_main]

mod data_point;
mod elm_commands;
mod elm_uart;
mod errors;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use assign_resources::assign_resources;
use embassy_rp::peripherals::{self, USB, PIO0};
use embassy_rp::usb::Driver;
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};
use defmt::*;
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use crate::elm_uart::elm_uart_task;

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

assign_resources! { // I hate this macro shit
    // led_ring: LedRing{
    //     data_pin: PIN_23,
    //     dma: DMA_CH0,
    // },
    elm_uart: ElmUart{
        tx_pin: PIN_0,
        rx_pin: PIN_1,
        uart0: UART0,
    },
    backlight_adc: BacklightAdc{
        adc_pin: PIN_14,
    },
    stepper: StepperPins{
        a1_pin: PIN_4,
        a2_pin: PIN_5,
        b1_pin: PIN_6,
        b2_pin: PIN_7,
    }
}

bind_interrupts!(struct Irqs {
    // USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<USB>;
    UART0_IRQ => embassy_rp::uart::BufferedInterruptHandler<peripherals::UART0>;
});

// #[embassy_executor::task]
// async fn logger_task(driver: Driver<'static, USB>) {
//     embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
// }

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let r = split_resources!(p);
    
    // let driver = Driver::new(p.USB, Irqs);
    // spawner.spawn(logger_task(driver)).unwrap();
    spawner.spawn(elm_uart_task(r.elm_uart)).unwrap();

    let receiver = INCOMING_EVENT_CHANNEL.receiver();

    
    loop {
        let event = receiver.receive().await;
        match event{
            ToMainEvents::GaugeInitComplete => {
                info!("Gauge initialized");
            }
            ToMainEvents::GaugeError(e) => {
                info!("Gauge error: {:?}", e);
            }
            ToMainEvents::LcdInitComplete => {
                info!("LCD initialized");
            }
            ToMainEvents::LcdError(e) => {
                info!("LCD error: {:?}", e);
            }
            ToMainEvents::ElmInitComplete => {
                info!("Elm initialized");
            }
            ToMainEvents::ElmError(e) => {
                info!("Elm error: {:?}", e);
            }
            ToMainEvents::ElmDataPoint(d) => {
                info!("Elm data point: {:?}", d);
            }
        }
    }
}
