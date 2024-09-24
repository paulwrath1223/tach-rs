//! This example shows how to use SPI (Serial Peripheral Interface) in the RP2040 chip.
//!
//! Example written for a display using the ST7789 chip. Possibly the Waveshare Pico-ResTouch
//! (https://www.waveshare.com/wiki/Pico-ResTouch-LCD-2.8)

use core::cell::RefCell;
use defmt::*;
use display_interface_spi::SPIInterface;
use embassy_embedded_hal::shared_bus::blocking::spi::SpiDeviceWithConfig;

use embassy_rp::gpio::{Level, Output};
use embassy_rp::spi;
use embassy_rp::spi::{Blocking, Spi};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::channel::Sender;
use embassy_time::{Delay, Duration, Ticker};
use embedded_graphics::image::Image;
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};
use embedded_graphics::text;
use embedded_graphics::text::Text;
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, Orientation};
use {defmt_rtt as _, panic_probe as _};
use crate::{DisplayPins, ToLcdEvents, ToMainEvents, INCOMING_EVENT_CHANNEL, LCD_EVENT_CHANNEL};
use tinybmp::Bmp;
use profont;
use crate::byte_parsing::float_as_str;
use crate::data_point::Datum;
use crate::errors::{ToRustAGaugeError, ToRustAGaugeErrorWithSeverity};

const DISPLAY_FREQ: u32 = 64_000_000;

const BG_COLOR: Rgb565 = Rgb565::BLACK;
const ORANG: Rgb565 = Rgb565::new(29, 24, 3);
const VBAT_TEXT_POINT: Point = Point::new(108, 48);
const COOLANT_TEXT_POINT: Point = Point::new(108, 134);
const MAIN_TEXT_STYLE: MonoTextStyle<Rgb565> = MonoTextStyle::new(&profont::PROFONT_24_POINT, ORANG);

const MIN_GOOD_VOLTAGE: f64 = 11f64;

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
    let sender: Sender<CriticalSectionRawMutex, ToMainEvents, 10> = INCOMING_EVENT_CHANNEL.sender();


    let spi: Spi<'_, _, Blocking> = Spi::new_blocking(r.spi_resource, clk, mosi, miso, display_config.clone());
    let spi_bus: Mutex<NoopRawMutex, _> = Mutex::new(RefCell::new(spi));

    let display_spi = SpiDeviceWithConfig::new(&spi_bus, Output::new(display_cs, Level::High), display_config);
    
    let dcx = Output::new(dcx_resource, Level::Low);
    let rst = Output::new(rst_resource, Level::Low);

    // display interface abstraction from SPI and DC
    let di = SPIInterface::new(display_spi, dcx);

    let display_orientation = Orientation::new().rotate(mipidsi::options::Rotation::Deg90);

    // create driver
    let mut display = mipidsi::Builder::new(ST7789, di)
        .reset_pin(rst)
        .display_size(170, 320)
        .display_offset(35, 0)
        .invert_colors(ColorInversion::Inverted)
        .orientation(display_orientation)
        .init(&mut Delay)
        .expect("failed to initialize display");


    // initialize

    info!("initialized display");

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

    display.clear(BG_COLOR).expect("failed to clear");

    info!("initialized icons");

    // Display the image


    let line_style = PrimitiveStyle::with_stroke(ORANG, 2);
    let error_text_style = MonoTextStyle::new(&FONT_10X20, ORANG);

    

    Line::new(Point::new(200, 0), Point::new(200, 170))
        .into_styled(line_style)
        .draw(&mut display).expect("failed to make vertical line");

    Line::new(Point::new(0, 85), Point::new(320, 85))
        .into_styled(line_style)
        .draw(&mut display).expect("failed to make horizontal line");

    let mut ticker = Ticker::every(Duration::from_millis(50));
    
    let vbat_quadrant_clear = Rectangle::with_corners(
        Point::new(0, 0), Point::new(198, 83))
        .into_styled(PrimitiveStyle::with_fill(BG_COLOR));
    
    let coolant_text_clear = Rectangle::with_corners(
        Point::new(106, 87), Point::new(198, 170))
        .into_styled(PrimitiveStyle::with_fill(BG_COLOR));
    
    let mut last_error: Option<ToRustAGaugeErrorWithSeverity> = None;

    let mut counter: u64 = 0;

    let mut error_text = Text::new("Hello World\nLine 2??\nLine 3??\nLine 4??", Point::new(206, 103), error_text_style);
    


    rust_logo.draw(&mut display).expect("failed to draw rust_logo");
    coolant_temp_icon.draw(&mut display).expect("failed to draw coolant_temp_icon");
    warning_icon.draw(&mut display).expect("failed to draw warning_icon");
    light_icon.draw(&mut display).expect("failed to draw light_icon");
    good_vbat_icon.draw(&mut display).expect("failed to draw good_vbat_icon");
    // bad_vbat_icon.draw(&mut display).expect("failed to draw bad_vbat_icon");

    // error_text.draw(&mut display).expect("failed to draw error_text");
    // vbat_text.draw(&mut display).expect("failed to draw vbat_text");
    // coolant_temp_text.draw(&mut display).expect("failed to draw coolant_temp_text");
    
    let mut local_str_buf = [0u8; 12];
    
    sender.send(ToMainEvents::LcdInitComplete).await;
    
    loop {
        match receiver.receive().await{
            ToLcdEvents::NewData(d) => {
                match d.data{
                    Datum::VBat(v) => {
                        vbat_quadrant_clear.draw(&mut display).expect("failed to clear vbat quadrant");
                        if v > MIN_GOOD_VOLTAGE{
                            good_vbat_icon.draw(&mut display).expect("failed to draw good_vbat_icon");
                        } else {
                            bad_vbat_icon.draw(&mut display).expect("failed to draw bad_vbat_icon");
                        }
                        draw_vbat_text(v, &mut display, &mut local_str_buf);
                    }
                    Datum::CoolantTempC(v) => {
                        coolant_text_clear.draw(&mut display).expect("failed to clear coolant text");
                        draw_coolant_temp_text(v, &mut display, &mut local_str_buf);
                    }
                    _ => {
                        defmt::error!("LCD received unknown datum (not Vbat or Coolant temp)");
                    }
                }
            }
            ToLcdEvents::IsBackLightOn(new_bl_state) => {
                defmt::info!("received backlight state: {:?}", new_bl_state);
            }
            ToLcdEvents::Error(new_error) => {
                match (&new_error, &last_error){
                    (Some(some_new_error), Some(_last_error)) => {
                        error_text.clear_bounding_box(&mut display, BG_COLOR).expect("failed to clear text");
                        error_text.text = some_new_error.error.to_str();
                        error_text.draw(&mut display).expect("failed to draw error_text");
                    }
                    (Some(some_new_error), None) => {
                        rust_logo.clear_bounding_box(&mut display, BG_COLOR).expect("failed to clear rust logo");
                        error_text.text = some_new_error.error.to_str();
                        error_text.draw(&mut display).expect("failed to draw error_text");
                        warning_icon.draw(&mut display).expect("failed to draw warning icon");
                    }
                    (None, Some(_last_error)) => {
                        error_text.clear_bounding_box(&mut display, BG_COLOR).expect("failed to clear text");
                        rust_logo.draw(&mut display).expect("failed to draw ferris in error quad");
                        warning_icon.clear_bounding_box(&mut display, BG_COLOR).expect("failed to clear warning icon");
                    }
                    _ => {
                        
                    }
                }
                last_error = new_error;
            }
        }
        

        counter = counter.overflowing_add(1).0;

        ticker.next().await;

    }
}
const COOLANT_TEMP_UTF_8_UNIT_STR: [u8; 3] = [0xC2, 0xB0, b'C'];
const VBAT_UTF_8_UNIT_STR: u8 = b'V';
fn draw_vbat_text<D>(vbat_val: f64, display_ref: &mut D, byte_buf: &mut [u8])
where D: DrawTarget<Color = Rgb565>, D::Error: core::fmt::Debug
{
    let end_index = float_as_str(vbat_val, byte_buf, 1, -1);
    byte_buf[end_index] = VBAT_UTF_8_UNIT_STR;
    let text_str_ref = core::str::from_utf8(&byte_buf[..end_index+1]).expect("failed to interpret vbat_str_buffer as utf-8;");
    let text_obj = Text::new(text_str_ref, VBAT_TEXT_POINT, MAIN_TEXT_STYLE);
    text_obj.draw(display_ref).expect("failed to draw vbat text");
}

fn draw_coolant_temp_text<D>(coolant_temp: f64, display_ref: &mut D, byte_buf: &mut [u8])
where D: DrawTarget<Color = Rgb565>, D::Error: core::fmt::Debug
{
    let end_index = float_as_str(coolant_temp, byte_buf, 2, 0);
    byte_buf[end_index..end_index+3].copy_from_slice(&COOLANT_TEMP_UTF_8_UNIT_STR);
    let text_str_ref = core::str::from_utf8(&byte_buf[..end_index+3]).expect("failed to interpret coolant_str_buffer as utf-8;");
    let text_obj = Text::new(text_str_ref, COOLANT_TEXT_POINT, MAIN_TEXT_STYLE);
    text_obj.draw(display_ref).expect("failed to draw vbat text");
}








pub trait Clear<D>
where Self: Dimensions, D: DrawTarget<Color = Rgb565>
{
    fn clear_bounding_box(&self, display: &mut D, color: Rgb565) -> Result<(), ToRustAGaugeError>;
}

impl<C, D> Clear<D> for Image<'_, Bmp<'_, C>>
where C: PixelColor, D: DrawTarget<Color = Rgb565>
{
    fn clear_bounding_box(&self, display: &mut D, color: Rgb565) -> Result<(), ToRustAGaugeError> {
        display.fill_solid(&self.bounding_box(), color).or(Err(ToRustAGaugeError::MipiDsiError()))
    }
}

impl<S, D> Clear<D> for Text<'_, S>
where S: text::renderer::TextRenderer, D: DrawTarget<Color = Rgb565>
{
    fn clear_bounding_box(&self, display: &mut D, color: Rgb565) -> Result<(), ToRustAGaugeError> {
        display.fill_solid(&self.bounding_box(), color).or(Err(ToRustAGaugeError::MipiDsiError()))
    }
}





