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
use embedded_graphics::pixelcolor::{Rgb565, Rgb888};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};
use embedded_graphics::{mono_font, text};
use embedded_graphics::text::Text;
use mipidsi::options::{ColorInversion, Orientation};
use {defmt_rtt as _, panic_probe as _};
use crate::{DisplayPins, ToLcdEvents, LCD_EVENT_CHANNEL};
use tinybmp::Bmp;
use profont;
use crate::data_point::Datum;
use crate::errors::{ToRustAGaugeErrorWithSeverity};

const DISPLAY_FREQ: u32 = 64_000_000;

#[embassy_executor::task]
pub async fn display_task(r: DisplayPins) {

    info!("Hello from display task!");

    let bl = r.bl;
    let rst_resource = r.rst;
    let display_cs = r.display_cs;
    let dcx_resource = r.dcx;
    let miso = r.miso;
    let mosi = r.mosi;
    let clk = r.clk;

    // create SPI
    let mut display_config = spi::Config::default();
    display_config.frequency = DISPLAY_FREQ;
    display_config.phase = spi::Phase::CaptureOnSecondTransition;
    display_config.polarity = spi::Polarity::IdleHigh;
    
    let receiver = LCD_EVENT_CHANNEL.receiver();


    let spi: Spi<'_, _, Blocking> = Spi::new_blocking(r.spi_resource, clk, mosi, miso, display_config.clone());
    let spi_bus: Mutex<NoopRawMutex, _> = Mutex::new(RefCell::new(spi));

    let display_spi = SpiDeviceWithConfig::new(&spi_bus, Output::new(display_cs, Level::High), display_config);

    let mut current_error: Option<ToRustAGaugeErrorWithSeverity> = None;
    
    let dcx = Output::new(dcx_resource, Level::Low);
    let rst = Output::new(rst_resource, Level::Low);
    

    // display interface abstraction from SPI and DC
    let di = display_interface_spi::SPIInterface::new(display_spi, dcx);

    let display_orientation = Orientation::new().rotate(mipidsi::options::Rotation::Deg90);

    // create driver
    let mut display = mipidsi::Builder::new(mipidsi::models::ST7789, di)
        .reset_pin(rst)
        .display_size(170, 320)
        .display_offset(35, 0)
        .invert_colors(ColorInversion::Inverted)
        .orientation(display_orientation)
        .init(&mut Delay)
        .expect("failed to initialize display");


    // initialize

    info!("initialized display");


    let bg_color = Rgb565::BLACK;
    let pure_orang = Rgb888::new(234, 94, 26);
    let orang = Rgb565::from(pure_orang);

    let rust_logo_data: Bmp<Rgb565> = Bmp::from_slice(include_bytes!("../display_assets/Rust Logo Layer.bmp")).expect("failed to parse bmp");
    let rust_logo = Image::with_center(&rust_logo_data, Point::new(260, 128));

    let coolant_temp_icon_data: Bmp<Rgb565> = Bmp::from_slice(include_bytes!("../display_assets/Coolant Temp Layer.bmp")).expect("failed to parse bmp");
    let coolant_temp_icon = Image::with_center(&coolant_temp_icon_data, Point::new(48, 128));

    let warning_icon_data: Bmp<Rgb565> = Bmp::from_slice(include_bytes!("../display_assets/Warning Layer.bmp")).expect("failed to parse bmp");
    let warning_icon = Image::with_center(&warning_icon_data, Point::new(279, 42));

    let light_icon_data: Bmp<Rgb565> = Bmp::from_slice(include_bytes!("../display_assets/Light Indicator Layer.bmp")).expect("failed to parse bmp");
    let light_icon = Image::with_center(&light_icon_data, Point::new(222, 24));

    let good_vbat_icon_data: Bmp<Rgb565> = Bmp::from_slice(include_bytes!("../display_assets/Good Battery Layer.bmp")).expect("failed to parse bmp");
    let good_vbat_icon = Image::with_center(&good_vbat_icon_data, Point::new(48, 42));

    let bad_vbat_icon_data: Bmp<Rgb565> = Bmp::from_slice(include_bytes!("../display_assets/Bad Battery Layer.bmp")).expect("failed to parse bmp");
    let bad_vbat_icon = Image::with_center(&bad_vbat_icon_data, Point::new(48, 42));

    display.clear(bg_color).expect("failed to clear");

    info!("initialized icons");

    // Display the image


    let line_style = PrimitiveStyle::with_stroke(orang, 2);
    let error_text_style = MonoTextStyle::new(&FONT_10X20, orang);

    let main_text_style = MonoTextStyle::new(&profont::PROFONT_24_POINT, orang);



    Line::new(Point::new(200, 0), Point::new(200, 170))
        .into_styled(line_style)
        .draw(&mut display).expect("failed to make vertical line");

    Line::new(Point::new(0, 85), Point::new(320, 85))
        .into_styled(line_style)
        .draw(&mut display).expect("failed to make horizontal line");

    let mut ticker = Ticker::every(Duration::from_millis(50));
    
    let error_quadrant_clear = Rectangle::with_corners(
        Point::new(202, 87), Point::new(320, 170))
        .into_styled(PrimitiveStyle::with_fill(bg_color));

    let mut counter: u64 = 0;

    let mut error_text = Text::new("Hello World\nLine 2??\nLine 3??\nLine 4??", Point::new(206, 103), error_text_style);

    let mut vbat_text = Text::new("?????", Point::new(108, 48), main_text_style);

    vbat_text.text = "12.5V";

    let mut coolant_temp_text = Text::new("?????", Point::new(108, 134), main_text_style);

    coolant_temp_text.text = "105°C";

    rust_logo.draw(&mut display).expect("failed to draw rust_logo");
    coolant_temp_icon.draw(&mut display).expect("failed to draw coolant_temp_icon");
    warning_icon.draw(&mut display).expect("failed to draw warning_icon");
    light_icon.draw(&mut display).expect("failed to draw light_icon");
    good_vbat_icon.draw(&mut display).expect("failed to draw good_vbat_icon");
    // bad_vbat_icon.draw(&mut display).expect("failed to draw bad_vbat_icon");

    // error_text.draw(&mut display).expect("failed to draw error_text");
    vbat_text.draw(&mut display).expect("failed to draw vbat_text");
    coolant_temp_text.draw(&mut display).expect("failed to draw coolant_temp_text");
    
    

    loop {
        match receiver.receive().await{
            ToLcdEvents::NewData(d) => {
                match d.data{
                    Datum::VBat(v) => {
                        
                    }
                    Datum::CoolantTempC(v) => {
                        
                    }
                    _ => {
                        defmt::error!("LCD received unknown datum (not Vbat or Coolant temp)");
                    }
                }
            }
            ToLcdEvents::IsBackLightOn(new_bl_state) => {
                defmt::todo!();
            }
            ToLcdEvents::Error(new_error) => {
                match new_error{
                    Some(e) => {
                        error_text.text = e.error.to_str();
                        error_text.draw(&mut display).expect("failed to draw error_text");
                    }
                    None => {
                        error_quadrant_clear.draw(&mut display).expect("failed to clear error quadrant");
                        rust_logo.draw(&mut display).expect("failed to draw ferris in error quad");
                    }
                }
            }
        }
        

        counter = counter.overflowing_add(1).0;

        ticker.next().await;

    }
}




