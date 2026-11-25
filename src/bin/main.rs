#![no_std]
#![no_main]

//IMPORTS
use core::{cell::RefCell, fmt::Write};
use defmt_rtt as _;
use embedded_graphics::{
    mono_font::{MonoTextStyleBuilder, ascii::FONT_9X15},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use embedded_hal_bus::i2c::RefCellDevice;
use esp_backtrace as _;
use esp_hal::{
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
    i2c::master::{Config, I2c},
    main,
};
use esp_println::println;
use heapless::String;
use sht4x::{Precision, Sht4x};
use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};

//embeds project and build metadata in the firmware image
esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let mut delay = Delay::new();
    let mut relay_on = false;

    //SHARED I2C BUS SETUP
    //init peripherals and define gpio pins
    let peripherals = esp_hal::init(Default::default());
    let sda = peripherals.GPIO22;
    let scl = peripherals.GPIO23;
    let mut relay_control = Output::new(peripherals.GPIO0, Level::Low, OutputConfig::default());

    //create a RefCell for the shared bus so that multiple devices can safely borrow its reference
    let shared_bus = RefCell::new(
        I2c::new(peripherals.I2C0, Config::default())
            .unwrap()
            .with_sda(sda)
            .with_scl(scl),
    );

    //declare sht40 enviro sensor, pointing it to the shared_bus reference
    let mut sht40 = Sht4x::new(RefCellDevice::new(&shared_bus));

    //declare display interface, pointing it to the shared_bus reference
    let display_interface = I2CDisplayInterface::new(RefCellDevice::new(&shared_bus));

    //initialize the display proper, taking in the mutable display_interface reference
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
        let mut display_text: String<32> = String::new();
        let measurement = sht40.measure(Precision::Medium, &mut delay);

        //creates reusable pattern for checking the sensor data and checking for errors
        let readings = match measurement {
            Ok(values) => {
                println!(
                    "Temp: {} C\nHumidity: {} %\n\n",
                    values.temperature_celsius(),
                    values.humidity_percent(),
                );
                Some(values)
            }
            Err(_e) => {
                println!("Failed to read from SHT40 sensor");
                None
            }
        };

        //uses the match pattern on readings
        match readings {
            Some(values) => {
                //if some value exists, this branch will run

                //formats the string for the display
                write!(
                    &mut display_text,
                    "Temp: {:.1} C\nHum: {:.1} %",
                    values.temperature_celsius(),
                    values.humidity_percent(),
                )
                .unwrap();

                //draws the formatted text to the displays drawing buffer
                Text::with_baseline(&display_text, Point::zero(), text_style, Baseline::Top)
                    .draw(&mut display)?;

                //draws the pending buffer to the display
                display.flush()?;

                //clears display buffer at end of loop so that values dont stack and become unreadable
                display.clear(BinaryColor::Off)?;

                //RELAY CONTROL LOGIC
                //ideal enclosure humidity is in the range of 50% - 75%, set hysteresis to fall
                //within this band

                if !relay_on && values.humidity_percent() < 55.0 {
                    relay_control.set_high();
                    relay_on = true;
                }
                if relay_on && values.humidity_percent() >= 70.0 {
                    relay_control.set_low();
                    relay_on = false;
                }
            }
            None => {
                //if no value exists, this branch will run

                //displays error message if no readings are found
                Text::with_baseline(
                    "Could not connect to sensor",
                    Point::zero(),
                    text_style,
                    Baseline::Top,
                )
                .draw(&mut display)?;

                //draws to the display
                display.flush()?;

                //clears display at end of loop so that values dont stack until it becomes unreadable
                display.clear(BinaryColor::Off)?;
            }
        };

        //delay so that the loop only runs once per second
        delay.delay_millis(1000);
    }
}
