//! This example shows how to use SPI (Serial Peripheral Interface) in the RP2040 chip.
//!
//! Example written for a display using the ST7789 chip. Possibly the Waveshare Pico-ResTouch
//! (https://www.waveshare.com/wiki/Pico-ResTouch-LCD-2.8)

use core::cell::RefCell;

use defmt::*;
use embassy_embedded_hal::shared_bus::blocking::spi::SpiDeviceWithConfig;

use embassy_rp::gpio::{Level, Output};
use embassy_rp::spi;
use embassy_rp::spi::{Blocking, Spi};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::{Delay, Duration, Ticker};
use embedded_graphics::image::{Image, ImageRawLE};
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::{Rgb565};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::PrimitiveStyle;
use embedded_graphics::text;
use mipidsi::options::{ColorInversion, Orientation};
use {defmt_rtt as _, panic_probe as _};
use crate::DisplayPins;

const DISPLAY_FREQ: u32 = 64_000_000;

#[embassy_executor::task]
pub async fn display_task(r: DisplayPins) {

    info!("Hello from display task!");

    let bl = r.bl;
    let rst = r.rst;
    let display_cs = r.display_cs;
    let dcx = r.dcx;
    let miso = r.miso;
    let mosi = r.mosi;
    let clk = r.clk;

    // create SPI
    let mut display_config = spi::Config::default();
    display_config.frequency = DISPLAY_FREQ;
    display_config.phase = spi::Phase::CaptureOnSecondTransition;
    display_config.polarity = spi::Polarity::IdleHigh;


    let spi: Spi<'_, _, Blocking> = Spi::new_blocking(r.spi_resource, clk, mosi, miso, display_config.clone());
    let spi_bus: Mutex<NoopRawMutex, _> = Mutex::new(RefCell::new(spi));

    let display_spi = SpiDeviceWithConfig::new(&spi_bus, Output::new(display_cs, Level::High), display_config);


    let dcx = Output::new(dcx, Level::Low);
    let rst = Output::new(rst, Level::Low);
    let bl = Output::new(bl, Level::High);

    // display interface abstraction from SPI and DC
    let di = display_interface_spi::SPIInterface::new(display_spi, dcx);
    
    let display_orientation = Orientation::new().rotate(mipidsi::options::Rotation::Deg90);

    // create driver
    let mut display = mipidsi::Builder::new(mipidsi::models::ST7789, di)
        .reset_pin(rst)
        .display_size(170, 320)
        .display_offset(32, 0)
        .invert_colors(ColorInversion::Inverted)
        .orientation(display_orientation)
        .init(&mut Delay)
        .expect("failed to initialize display");


    // initialize
    
    info!("initialized display");



    display.clear(Rgb565::BLACK).unwrap();

    let raw_image_data = ImageRawLE::new(include_bytes!("../display_assets/ferris.raw"), 86);
    let ferris = Image::new(&raw_image_data, Point::new(5, 40));
    
    info!("initialized ferris");

    // Display the image
    

    let style = MonoTextStyle::new(&FONT_10X20, Rgb565::GREEN);
    let text = text::Text::new(
        "Hello embedded_graphics \n + embassy + RP2040!",
        Point::new(90, 30),
        style,
    );

    let mut ticker = Ticker::every(Duration::from_millis(50));

    let mut counter: u64 = 0;
    let fill = PrimitiveStyle::with_fill(Rgb565::BLUE);
    
    let mut circle = embedded_graphics::primitives::Circle::new(
        Point::new((counter % 320) as i32, 100), 15)
        .into_styled(fill);

    ferris.draw(&mut display).expect("drawing ferris failed");
    text.draw(&mut display).expect("text.draw failed");
    
    loop {

        circle.draw(&mut display).expect("drawing circle failed");
        circle.primitive.top_left.x = (counter%320) as i32;
        counter = counter.overflowing_add(1).0;
        
        ticker.next().await;
        
    }
}




