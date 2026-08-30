//! Driver for the wind direction sensor.
//!
//! Provides all the necessary functions for collecting wind direction data.

use defmt::{Format, debug, error, info};
use embedded_hal_async::i2c::{Error, ErrorKind, Operation, SevenBitAddress};
use embassy_time::Timer;

/// Direction Sensor.
pub struct DirectionSensor<I2C> {
    /// I2C bus from NRF52840
    i2c: I2C,
    /// I2C address of sensor
    addr: SevenBitAddress,
    calibration: CalibrationData,
}

/// Direction Sensor Errors.
#[derive(Format, Debug)]
pub enum SensorError {
    GetDataError,
    I2cError(ErrorKind),
}

impl<E: Error> From<E> for SensorError {
    fn from(value: E) -> Self {
        SensorError::I2cError(value.kind())
    }
}
