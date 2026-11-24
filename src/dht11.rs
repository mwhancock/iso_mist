use esp_hal::delay::Delay;
use esp_hal::gpio::{Flex, InputConfig, Pull};
use esp_hal::time::Instant;

const START_LOW_MS: u32 = 20;
const START_HIGH_US: u32 = 40;

const RESPONSE_TIMEOUT_MS: u64 = 100;
const RESPONSE_PULSE_TIMEOUT_US: u64 = 200;
const BIT_LOW_TIMEOUT_US: u64 = 100;
const BIT_HIGH_TIMEOUT_US: u64 = 100;
const BIT_ONE_THRESHOLD_US: u64 = 50;

/// Number of level transitions recorded by [`DHT11::capture_edges`].
pub const CAPTURE_EDGES: usize = 32;
/// How long [`DHT11::capture_edges`] samples the bus, in milliseconds.
pub const CAPTURE_WINDOW_MS: u64 = 10;

/// Raw edge trace produced by [`DHT11::capture_edges`].
///
/// `widths_us[i]` is the duration of the `i`-th level segment after the host
/// releases the bus, and bit `i` of `high_mask` is set when that segment was
/// high. A healthy DHT11 frame starts roughly like
/// `[30, 80, 80, 50, 26, 50, 26, ...]`: response delay, response low, response
/// high, then 50 us low + 26 us (0) or 70 us (1) high per bit.
#[derive(Debug, defmt::Format)]
pub struct EdgeTrace {
    pub count: usize,
    pub widths_us: [u16; CAPTURE_EDGES],
    pub high_mask: u32,
}

impl EdgeTrace {
    pub const fn new() -> Self {
        Self {
            count: 0,
            widths_us: [0; CAPTURE_EDGES],
            high_mask: 0,
        }
    }
}

impl Default for EdgeTrace {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, defmt::Format)]
pub enum SensorError {
    NoResponse,
    ResponseLowTimeout,
    ResponseHighTimeout,
    BitLowTimeout { index: u8 },
    BitHighTimeout { index: u8 },
    ChecksumMismatch,
}

#[derive(Debug, Copy, Clone, defmt::Format)]
pub struct Reading {
    pub humidity: u8,
    pub temperature: i8,
}

pub struct DHT11 {
    pub delay: Delay,
}

impl DHT11 {
    pub fn new(delay: Delay) -> Self {
        Self { delay }
    }

    pub fn read(&mut self, pin: &mut Flex) -> Result<Reading, SensorError> {
        let data = self.read_raw(pin)?;

        Ok(Reading {
            humidity: data[0],          // DHT11 humidity integer part
            temperature: data[2] as i8, // DHT11 temperature integer part
        })
    }

    fn read_raw(&mut self, pin: &mut Flex) -> Result<[u8; 5], SensorError> {
        pin.apply_input_config(&InputConfig::default().with_pull(Pull::Up));

        pin.set_low();
        pin.set_output_enable(true);
        self.delay.delay_millis(START_LOW_MS);

        pin.set_high();
        self.delay.delay_micros(START_HIGH_US);

        pin.set_output_enable(false);
        pin.set_input_enable(true);
 
        let start = Instant::now();
        while pin.is_high() {
            if start.elapsed().as_millis() > RESPONSE_TIMEOUT_MS {
                return Err(SensorError::NoResponse);
            }
        }

        wait_while_low(pin, RESPONSE_PULSE_TIMEOUT_US)
            .map_err(|_| SensorError::ResponseLowTimeout)?;
        wait_while_high(pin, RESPONSE_PULSE_TIMEOUT_US)
            .map_err(|_| SensorError::ResponseHighTimeout)?;

        let mut buf = [0u8; 5];
        for byte in buf.iter_mut() {
            *byte = read_byte(pin)?;
        }

        let sum = buf[0]
            .wrapping_add(buf[1])
            .wrapping_add(buf[2])
            .wrapping_add(buf[3]);

        if buf[4] == sum {
            Ok(buf)
        } else {
            Err(SensorError::ChecksumMismatch)
        }
    }

    /// Performs a start signal and records the widths of the following edges.
    ///
    /// This is a pure diagnostic: it does not decode anything. It samples the
    /// line for up to [`CAPTURE_WINDOW_MS`] and fills `trace` with the first
    /// [`CAPTURE_EDGES`] level transitions, so the caller can tell clean DHT11
    /// timings apart from electrical garbage.
    pub fn capture_edges(&mut self, pin: &mut Flex, trace: &mut EdgeTrace) {
        trace.count = 0;
        trace.high_mask = 0;

        pin.apply_input_config(&InputConfig::default().with_pull(Pull::Up));

        pin.set_low();
        pin.set_output_enable(true);
        self.delay.delay_millis(START_LOW_MS);

        pin.set_high();
        self.delay.delay_micros(START_HIGH_US);

        pin.set_output_enable(false);
        pin.set_input_enable(true);

        let window_start = Instant::now();
        let mut seg_start = Instant::now();
        let mut last_high = pin.is_high();
        let mut count = 0;

        while count < CAPTURE_EDGES {
            let high = pin.is_high();
            if high != last_high {
                let width = seg_start.elapsed().as_micros();
                trace.widths_us[count] = width.min(u16::MAX as u64) as u16;
                if last_high {
                    trace.high_mask |= 1 << count;
                }
                count += 1;
                last_high = high;
                seg_start = Instant::now();
            }

            if window_start.elapsed().as_millis() > CAPTURE_WINDOW_MS {
                break;
            }
        }

        trace.count = count;
    }
}

fn read_byte(pin: &mut Flex) -> Result<u8, SensorError> {
    let mut byte = 0u8;

    for index in 0..8u8 {
        wait_while_low(pin, BIT_LOW_TIMEOUT_US)
            .map_err(|_| SensorError::BitLowTimeout { index })?;

        let high_start = Instant::now();
        while pin.is_high() {
            if high_start.elapsed().as_micros() > BIT_HIGH_TIMEOUT_US {
                return Err(SensorError::BitHighTimeout { index });
            }
        }

        if high_start.elapsed().as_micros() > BIT_ONE_THRESHOLD_US {
            byte |= 1 << (7 - index);
        }
    }

    Ok(byte)
}

/// Waits for the bus to stop being low, giving up after `timeout_us`.
fn wait_while_low(pin: &mut Flex, timeout_us: u64) -> Result<(), ()> {
    let start = Instant::now();
    while pin.is_low() {
        if start.elapsed().as_micros() > timeout_us {
            return Err(());
        }
    }
    Ok(())
}

/// Waits for the bus to stop being high, giving up after `timeout_us`.
fn wait_while_high(pin: &mut Flex, timeout_us: u64) -> Result<(), ()> {
    let start = Instant::now();
    while pin.is_high() {
        if start.elapsed().as_micros() > timeout_us {
            return Err(());
        }
    }
    Ok(())
}
