#![no_std]
#![no_main]

use core::{cell::RefCell, fmt::Write};
use defmt_rtt as _;
use embassy_embedded_hal::{
    adapter::BlockingAsync
};
use embedded_graphics::{
    mono_font::{MonoTextStyleBuilder, ascii::FONT_9X15},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use embassy_time::Delay;
use embedded_hal_bus::i2c::RefCellDevice;
use esp_backtrace as _;
use esp_hal::{
    gpio::{Level, Output, OutputConfig},
    i2c::master::I2c,
    main,
};
use esp_println::println;
use heapless::String;
use sht4x::{
    Sht4xAsync,
    Precision,
    Measurement,
    Error
};
use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let delay = Delay::new();

    //SHARED I2C BUS SETUP
    //init peripherals and define sda and scl pins
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let sda = peripherals.GPIO22;
    let scl = peripherals.GPIO23;
    let mut relay_control = Output::new(peripherals.GPIO1, Level::Low, OutputConfig::default());

    //create a RefCell for the shared bus so that multiple devices can safely borrow the reference to it
    let shared_bus = RefCell::new(I2cDevice::new(peripherals.I2C0).with_sda(sda).with_scl(scl));

    //setup sht40 enviro sensor, pointing it to the shared_bus reference
    let mut sht40_driver = ;
    let mut relay_on = false;

    //setup internal oled display, pointing it to the shared_bus reference
    let display_interface = I2CDisplayInterface::new(RefCellDevice::new(&shared_bus));
    let mut display = Ssd1306::new(
        display_interface,
        DisplaySize128x64,
        DisplayRotation::Rotate0,
    )
    .into_buffered_graphics_mode();

    display.init().unwrap();

    //TEXT STYLES SETUP
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_9X15)
        .text_color(BinaryColor::On)
        .build();

    loop {
        //initialize sensor handle
        let sensor = sht40_driver.borrow_mut();
        let mut display_text: String<32> = String::new();
        let saved_reading;

        match sensor.read() {
            Ok(reading) => {
                println!(
                    "Temp: {:.1}C\nHumidity: {:.1} %\n\n",
                    reading.temperature, reading.humidity,
                );
                saved_reading = reading;
            }
            Err(_e) => {
                println!("Failed to read from SHT31 sensor");
                saved_reading = Reading {
                    temperature: -1.0,
                    humidity: -1.0,
                };
            }
        }

        //initialize display handle
        let oled = display.borrow_mut();

        if saved_reading.temperature == -1.0 || saved_reading.humidity == -1.0 {
            Text::with_baseline(
                "Could not connect to sensor",
                Point::zero(),
                text_style,
                Baseline::Top,
            )
            .draw(oled)
            .unwrap();
        } else {
            //writes formatted_string to oled display
            write!(
                &mut display_text,
                "Temp: {:.1} C\nHum: {:.1} %",
                saved_reading.temperature, saved_reading.humidity,
            )
            .unwrap();

            //takes formatted text and translates it into something that the oled can display
            Text::with_baseline(&display_text, Point::zero(), text_style, Baseline::Top)
                .draw(oled)
                .unwrap();
        }

        //draws to the display
        oled.flush().unwrap();

        //clears display at end of loop so that values dont stack until it becomes unreadable
        oled.clear(BinaryColor::Off).unwrap();

        if !relay_on && saved_reading.humidity < 55.0 {
            relay_control.set_high();
            relay_on = true;
        }

        if relay_on && saved_reading.humidity >= 65.0 {
            relay_control.set_low();
            relay_on = false;
        }

        delay.delay_millis(1000);
    }
}
