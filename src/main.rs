

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

use embassy_executor::Spawner;
use embassy_rp::{bind_interrupts};
use assign_resources::assign_resources;
use embassy_rp::peripherals;
use {defmt_rtt as _, panic_probe as _};
use defmt::*;
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use crate::data_point::Datum;
use crate::display::display_task;
use crate::elm_uart::elm_uart_task;
use crate::errors::{ToRustAGaugeError, ToRustAGaugeErrorSeverity, ToRustAGaugeErrorWithSeverity};
use crate::gauge::gauge_task;

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
    backlight_adc: BacklightAdc{
        adc_pin: PIN_14,
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
}

bind_interrupts!(struct Irqs {
    UART0_IRQ => embassy_rp::uart::InterruptHandler<peripherals::UART0>;
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<peripherals::PIO0>; // stepper
    PIO1_IRQ_0 => embassy_rp::pio::InterruptHandler<peripherals::PIO1>; // ws2812
});



#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let r = split_resources!(p);

    
    let mut is_backlight_on = false;
    
    let receiver = INCOMING_EVENT_CHANNEL.receiver();

    spawner.spawn(gauge_task(r.gauge)).expect("failed to spawn elm uart task");
    spawner.spawn(elm_uart_task(r.elm_uart)).expect("failed to spawn elm uart task");
    spawner.spawn(display_task(r.display)).expect("failed to spawn display task");
    
    let lcd_sender = LCD_EVENT_CHANNEL.sender();
    let gauge_sender = GAUGE_EVENT_CHANNEL.sender();
    
    let mut is_gauge_init: bool = false;
    let mut is_lcd_init: bool = false;
    let mut is_elm_init: bool = false;
    
    loop {
        let event = receiver.receive().await;
        match event{
            ToMainEvents::GaugeInitComplete => {
                info!("Gauge initialized");
                is_gauge_init = true;
                gauge_sender.send(ToGaugeEvents::IsBackLightOn(is_backlight_on)).await;
            }
            ToMainEvents::GaugeError(e) => {
                warn!("Gauge error: {:?}", e);
            }
            ToMainEvents::LcdInitComplete => {
                info!("LCD initialized");
                is_lcd_init = true;
                lcd_sender.send(ToLcdEvents::IsBackLightOn(is_backlight_on)).await;
            }
            ToMainEvents::LcdError(e) => {
                warn!("LCD error: {:?}", e);
            }
            ToMainEvents::ElmInitComplete => {
                info!("Elm initialized");
                is_elm_init = true;
            }
            ToMainEvents::ElmError(e) => {
                warn!("Elm error: {:?}", e);
            }
            ToMainEvents::ElmDataPoint(d) => {
                info!("Elm data point: {:?}", d);
                match d.data{
                    Datum::RPM(_) => {
                        if is_gauge_init{
                            gauge_sender.send(ToGaugeEvents::NewData(d)).await;
                        }
                    }
                    _ => {
                        if is_lcd_init{
                            if d.data.is_value_sane_check(){
                                lcd_sender.send(ToLcdEvents::NewData(d)).await;
                            } else {
                                defmt::error!("Insane data point: {:?}", d);
                            }
                        }
                    }
                }
            }
        }
    }
}
