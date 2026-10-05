//! Driver for the wind speed and soil moisture sensors.
//!
//! Provides all the necessary functions for collecting wind speed and soil moisture data.
//! The wind speed sensor operates by reading out a voltage from 0V to 2V which corresponds
//! with wind speeds from 0 to 200 km/h.
//! The soil moisture sensor operates by reading out a voltage from 0V to 3V which corresponds
//! with soil moisture from 0% to 100%, where 100% represents full submersion of the sensor
//! in water.
//!
//! Values are collected using the ADS1115 ADC to I2C bus.

use defmt::{Format, debug,trace, error};
use embedded_hal_async::i2c::{Error, ErrorKind, SevenBitAddress};
use embassy_nrf::saadc::{ChannelConfig, Config, Saadc};
use embassy_time::Timer;
use num_traits::float::FloatCore;

/// Direction Sensor.
pub struct AdcSensors<I2C> {
    /// Onboard ADC for battery check
    adc: Saadc<'static, 1>,
    /// I2C bus from NRF52840
    i2c: I2C,
    /// I2C address of sensor
    addr: SevenBitAddress,
}
 
/// Fixed-size ring buffer of wind speed samples in km/h (240 bytes).
pub struct WindHistory {
    samples: [f32; WIND_HISTORY_LEN],
    /// Index the next sample will be written to (also the oldest sample once full).
    next: usize,
    /// Number of valid samples; < WIND_HISTORY_LEN until the buffer fills.
    len: usize,
}

/// List of read/writable registers (and their address) on the gas sensor.
#[derive(Format, Clone)]
pub enum Registers {
    Conversion = 0,
    Config = 1,
    LoThresh = 2,
    HiThresh = 3,
}

/// Select sensor to get ADC reading from
pub enum SensorSelect {
    SoilMoisture,
    WindSpeed,
    // Battery,
}

/// Number of samples kept. 60 samples at one per minute = the previous hour.
pub const WIND_HISTORY_LEN: usize = 60;
/// Soil calibration constants.
pub const SOIL_DRY_BOUND: f32 = 2934.0f32; //Place value here after sensor is calibrated
pub const SOIL_WET_BOUND: f32 = 1610.0f32; //Place value here after sensor is calibrated
pub const SOIL_RANGE: f32 = SOIL_DRY_BOUND - SOIL_WET_BOUND;

/// Direction Sensor Errors.
#[derive(Format, Debug)]
pub enum SensorError {
    GetDataError,
    I2cError(ErrorKind),
    InvalidParameter,
    BatteryMappingError,
}

impl<E: Error> From<E> for SensorError {
    fn from(value: E) -> Self {
        SensorError::I2cError(value.kind())
    }
}

impl<I2C: embedded_hal_async::i2c::I2c> AdcSensors<I2C> {
    /// Initialise a new direction sensor.
    pub async fn new(adc: Saadc<'static, 1>, i2c: I2C, addr: u8) -> Result<Self, SensorError>{
        Ok(Self { adc, i2c, addr })
    }

    /// Write to one register.
    async fn write(&mut self, reg_addr: Registers, data: &[u8; 2]) -> Result<(), SensorError> {
        let reg_cp = reg_addr.clone();

        let mut buf = [0u8; 3];
        buf[0] = reg_addr as u8;
        buf[1..].copy_from_slice(data);

        match self.i2c.write(self.addr, &buf).await {
            Ok(_) => {
                trace!("write to register [{:?}]: {:#010b}", reg_cp, data);
                Ok(())
            }
            Err(e) => {
                let err: SensorError = e.into();
                error!("Could not write register [{:?}]: {:?}", reg_cp, err);
                Err(err)
            }
        }
    }

    /// Read from arbitrary amount of registers in a single transaction based on the given buffer size.
    async fn read(&mut self, reg_addr: Registers, buf: &mut [u8]) -> Result<(), SensorError> {
        let reg_cp = reg_addr.clone();

        self.i2c.write(self.addr, &[reg_addr as u8]).await?; // just the register pointer

        match self.i2c.read(self.addr, buf).await {
            Ok(_) => {
                trace!("read from register [{:?}]: {:#010b}", reg_cp, buf);
                Ok(())
            }
            Err(e) => {
                let err: SensorError = e.into();
                error!("Could not read register [{:?}]: {:?}", reg_cp, err);
                Err(err)
            }
        }
    }

    /// Initial configuration to set up the 16-bit ADC.
    pub async fn init_config(&mut self) -> Result<(), SensorError> {
        debug!("Configuring ADC BUS...");
        // init_config: OS=0, MUX=100, PGA=000, MODE=1 (single-shot)
        self.write(Registers::Config, &[0b0100_0001, 0b1000_0011]).await?;
        Ok(())
    }

    /// Set analog input to read from and trigger a one shot measurement.
    /// ADC returns back to low-power mode once conversion is done.
    async fn select_input_and_trigger(&mut self, input: SensorSelect) -> Result<(), SensorError> {
        match input {
            SensorSelect::SoilMoisture => {
                // init_config: OS=0, MUX=100, PGA=000, MODE=1 (single-shot)
                self.write(Registers::Config, &[0b1100_0001, 0b1000_0011]).await?;
            },
            SensorSelect::WindSpeed => {
                // init_config: OS=0, MUX=101, PGA=000, MODE=1 (single-shot)
                self.write(Registers::Config, &[0b1101_0001, 0b1000_0011]).await?;
            },
            // SensorSelect::Battery => {
            //     // init_config: OS=0, MUX=110, PGA=000, MODE=1 (single-shot)
            //     self.write(Registers::Config, &[0b0110_0001, 0b1000_0011]).await?;
            // },
        }
        Ok(())
    }

    /// Poll the Config register's OS bit until the one-shot conversion
    /// completes. Per the ADS1115 datasheet, OS reads back 0 while a
    /// conversion is in progress and 1 once the result is ready in the
    /// Conversion register -- without this, a read immediately after
    /// triggering can return a stale result left over from whichever
    /// conversion last completed, rather than the one just triggered.
    async fn wait_for_conversion(&mut self) -> Result<(), SensorError> {
        loop {
            let mut cfg = [0u8; 2];
            self.read(Registers::Config, &mut cfg).await?;
            if cfg[0] & 0b1000_0000 != 0 {
                break;
            }

            // Default data rate (128 SPS) gives a conversion time of ~7.8ms;
            // 500us keeps polling overhead low without meaningfully
            // lengthening the wait once the result is actually ready.
            Timer::after_micros(500).await;
        }

        Ok(())
    }

    async fn get_adc_reading_mv(&mut self, input: SensorSelect) -> Result<u32, SensorError> {
        self.select_input_and_trigger(input).await?;
        self.wait_for_conversion().await?;

        let mut raw = [0; 2];
        self.read(Registers::Conversion, &mut raw).await?;

        // Interpret the two bytes as a signed 16-bit two's complement value.
        // Change to `from_le_bytes` if your ADC sends the low byte first.
        let raw_value = i16::from_be_bytes(raw);

        // Clamp any unexpected negative reading to 0 rather than wrapping/underflowing.
        let raw_value = raw_value.max(0) as u32;

        // Full-scale range for PGA=000 is ±6.144V, mapped over the ADC's positive range (32768 counts).
        let voltage_mv = raw_value * 6144 / 32768;
        Ok(voltage_mv)
    }

    // Get voltage reading of direction sensor via 16-bit ADC.
    // pub async fn get_direction_reading(&mut self) -> Result<u16, SensorError> {
    //     let voltage_mv = self.get_adc_reading_mv(Sen).await?;
    //     let degrees = (voltage_mv * 360) / 5000;

    //     Ok(degrees.min(360) as u16)
    // }

    /// Get soil moisture reading from ADC and return moisutre in %.
    pub async fn get_soil_moisture(&mut self) -> Result<f32, SensorError> {
        let voltage_mv = self.get_adc_reading_mv(SensorSelect::SoilMoisture).await?;
        debug!("soil moisture sensor voltage: {}", voltage_mv);
        // Calculate moisture percentage
        let moisture = 100f32 - (((voltage_mv as f32 - SOIL_WET_BOUND) / SOIL_RANGE) * 100f32);
        Ok(moisture)
    }

    /// Get raw soil moisture ADC reading.
    pub async fn get_raw_soil_moisture(&mut self) -> Result<u32, SensorError> {
        let voltage_mv = self.get_adc_reading_mv(SensorSelect::SoilMoisture).await?;
        debug!("soil moisture sensor voltage: {}", voltage_mv);
        Ok(voltage_mv)
    }

    /// Get wind speed reading from ADC and return wind speed in km/h.
    pub async fn get_wind_speed(&mut self) -> Result<f32, SensorError> {
        // Get voltage reading from ADC
        let voltage_mv = self.get_adc_reading_mv(SensorSelect::WindSpeed).await?;
        debug!("wind speed sensor voltage: {}", voltage_mv);

        // Convert mV to km/h
        let wind_speed_kmh = (voltage_mv as f32 / 2000.0) * 200.0;
        Ok(wind_speed_kmh)
    }

    /// Get battery voltage from internal ADC, and return approximated percentage.
    pub async fn get_battery_level(&mut self) -> Result<u8, SensorError> {
        // ADC conversion and compensation
        const ADC_COMPENSATION_FACTOR: f32 = 1.73;
        const ADC_CONVERSION_FACTOR: f32 = 0.87890625; // 1/2^12 * 0.6/(1/6) * 1000 (mV/LSB)
        // Need to verify above value 

        // ADC sampling
        let mut raw = [0i16];
        self.adc.sample(&mut raw).await;

        // ADC conversion and scaling
        let adc_data = raw[0] as f32; 
        let v_bat = adc_data * ADC_CONVERSION_FACTOR * ADC_COMPENSATION_FACTOR; // in mV
        
        // Map mV value to percent
        mv_to_percent(v_bat)
    }
}

impl WindHistory {
    pub const fn new() -> Self {
        Self {
            samples: [0.0; WIND_HISTORY_LEN],
            next: 0,
            len: 0,
        }
    }
 
    /// Store a sample, overwriting the oldest once the buffer is full.
    pub fn push(&mut self, kmh: f32) {
        self.samples[self.next] = kmh;
        self.next = (self.next + 1) % WIND_HISTORY_LEN;
        if self.len < WIND_HISTORY_LEN {
            self.len += 1;
        }
    }
 
    /// Number of samples currently held (0..=60).
    pub fn len(&self) -> usize {
        self.len
    }
 
    /// The `i`th most recent sample (0 = newest). Caller guarantees i < len.
    fn recent(&self, i: usize) -> f32 {
        self.samples[(self.next + WIND_HISTORY_LEN - 1 - i) % WIND_HISTORY_LEN]
    }
 
    /// Mean of the most recent `minutes` samples (one sample per minute).
    ///
    /// If fewer than `minutes` samples exist yet (e.g. just after boot), the
    /// mean of whatever is available is returned. Returns `None` if the
    /// buffer is empty. `minutes` is capped at the buffer length.
    pub fn average_last(&self, minutes: usize) -> Option<f32> {
        let n = minutes.min(self.len);
        if n == 0 {
            return None;
        }
        let sum: f32 = (0..n).map(|i| self.recent(i)).sum();
        Some(sum / n as f32)
    }
}

// Approximate mapping battery voltage (mV) to %
fn mv_to_percent(mv: f32) -> Result<u8, SensorError> {
    // Mapping error boundaries 
    const VBAT_MIN_MV: f32 = 2500.0;
    const VBAT_MAX_MV: f32 = 4500.0;

    if mv > VBAT_MAX_MV || mv < VBAT_MIN_MV{
        return  Err(SensorError::BatteryMappingError)
    } 

    // Define mapping from mV to % 
    const CURVE: [(u16, u8); 11] = [
        (3300, 0), (3500, 5), (3600, 10), (3700, 25), (3750, 40), (3800, 55),
        (3850, 65), (3900, 75), (4000, 85), (4100, 95), (4200, 100),
    ];

    // Project voltage to mapping.
    let mv = mv as u16;
    if mv <= CURVE[0].0 { return Ok(0); }
    if mv >= CURVE[CURVE.len() - 1].0 { return Ok(100); }
    for w in CURVE.windows(2) {
        let (v0, p0) = w[0];
        let (v1, p1) = w[1];
        if mv <= v1 {
            let frac = (mv - v0) as f32 / (v1 - v0) as f32;
            return Ok((p0 as f32 + frac * (p1 as f32 - p0 as f32)) as u8);
        }
    }
    Err(SensorError::BatteryMappingError)
}


