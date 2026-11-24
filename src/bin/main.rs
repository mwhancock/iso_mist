#![no_std]
#![no_main]

use defmt::info;
use defmt_rtt as _;
use esp_backtrace as _;
use esp_hal::{delay::Delay, gpio::Flex, main};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let delay = Delay::new();

    let mut humid_pin = Flex::new(peripherals.GPIO17);

    info!("ESP32-C6 DHT11 Initialized. Starting sensor reads...");

    loop {
        humid_sense.delay.delay_millis(2000);

        match humid_sense.read(&mut humid_pin) {
            Ok(reading) => {
                info!(
                    "Temperature: {} degrees Celsius | Humidity: {} %",
                    reading.temperature, reading.humidity
                );
            }
            Err(e) => {
                info!("Sensor read failed: {:?}", e);

                humid_sense.delay.delay_millis(2000);
                let mut trace = EdgeTrace::new();
                humid_sense.capture_edges(&mut humid_pin, &mut trace);
                info!(
                    "edge capture: {} edges, high_mask=0x{:08x}, widths_us={}",
                    trace.count,
                    trace.high_mask,
                    &trace.widths_us[..trace.count]
                );
            }
        }
    }
}
