//! Driver for the wind direction sensor.
//!
//! Provides all the necessary functions for collecting wind direction data.
//! The sensor operates by reading out a voltage from 0V to 5V which corresponds
//! with a rotation of the flange rotating from 0 degrees to 360 degrees.
//! The sensor is to be mounted with the marker pointed South. This means that 0V
//! indicates the flange is pointed to the West, meaning the wind is blowing in
//! fromt the East.

use defmt::{Format, debug,trace, error};
use embedded_hal_async::i2c::{Error, ErrorKind, SevenBitAddress};

/// Direction Sensor.
pub struct DirectionSensor<I2C> {
    /// I2C bus from NRF52840
    i2c: I2C,
    /// I2C address of sensor
    addr: SevenBitAddress,
}

/// List of read/writable registers (and their address) on the gas sensor.
#[derive(Format, Clone)]
pub enum Registers {
    Conversion = 0,
    Config = 1,
    LoThresh = 2,
    HiThresh = 3,
}

/// Direction Sensor Errors.
#[derive(Format, Debug)]
pub enum SensorError {
    GetDataError,
    I2cError(ErrorKind),
    InvalidParameter,
}

impl<E: Error> From<E> for SensorError {
    fn from(value: E) -> Self {
        SensorError::I2cError(value.kind())
    }
}

impl<I2C: embedded_hal_async::i2c::I2c> DirectionSensor<I2C> {
    /// Initialise a new direction sensor.
    pub async fn new(i2c: I2C, addr: u8) -> Result<Self, SensorError>{
        Ok(Self { i2c, addr })
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
        debug!("Configuring DIRECTION SENSOR...");
        // init_config: OS=0, MUX=100, PGA=000, MODE=1 (single-shot)
        self.write(Registers::Config, &[0b0100_0001, 0b1000_0011]).await?;
        Ok(())
    }

    /// Trigger a single one-shot conversion. ADC returns back to low-power mode
    /// conversion is done.
    async fn trigger_one_shot(&mut self) -> Result<(), SensorError> {
        // trigger_one_shot: OS=1 (trigger), MUX=100, PGA=000, MODE=1
        self.write(Registers::Config, &[0b1100_0001, 0b1000_0011]).await?;
        Ok(())
    }

    /// Get voltage reading of direction sensor via 16-bit ADC.
    pub async fn get_direction_reading(&mut self) -> Result<u16, SensorError> {
        self.trigger_one_shot().await?;

        let mut raw = [0; 2];
        self.read(Registers::Conversion, &mut raw).await?;

        // Interpret the two bytes as a signed 16-bit two's complement value.
        // Change to `from_le_bytes` if your ADC sends the low byte first.
        let raw_value = i16::from_be_bytes(raw);

        // Clamp any unexpected negative reading to 0 rather than wrapping/underflowing.
        let raw_value = raw_value.max(0) as u32;

        // Full-scale range for PGA=000 is ±6.144V, mapped over the ADC's positive range (32768 counts).
        // voltage_mv = raw_value * 6144 / 32768
        // degrees   = voltage_mv * 360 / 5000
        let degrees = (raw_value * 6144 * 360) / (32768 * 5000);

        Ok(degrees.min(360) as u16)
    }
}
