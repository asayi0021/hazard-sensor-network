//! Firmware for the BME680 gas sensor.
//!
//! Provides all the necessary functions for collecting temperature, pressure, air quality and humidity measurements.

use embedded_hal::i2c::{Operation, Error, ErrorKind, SevenBitAddress};
use defmt::{Format, debug, error, info};

/// BME680 Gas Sensor.
pub struct GasSensor<I2C> {
    /// I2C bus from NRF52840
    i2c: I2C,
    /// I2C address of sensor
    addr: SevenBitAddress,
}

/// List of read/writable registers (and their address) on the gas sensor.
/// For more details, see [BME680 datasheet][registers].
///
/// [registers]: https://www.bosch-sensortec.com/media/boschsensortec/downloads/datasheets/bst-bme680-ds001.pdf
pub enum Registers {
    Status = 0x73,
    Reset = 0xE0,
    Id = 0xD0,
    Config = 0x75,
    CtrlMeas = 0x74,
    CtrlHum = 0x72,
    CtrlGas1 = 0x71,
    CtrlGas0 = 0x70,

    // Gas control registers
    // There are actually 10 registers for each of the 10 set-points.
    // The recorded address is just the addressof set-point 0. The next 9 address in sequence correspond to
    // each subsequent set-point sequentially.
    // x is the set-point (ranging from 0 to 9).
    GasWaitX = 0x64,
    ResHeatX = 0x5A,
    IdacHeatX = 0x50,

    GasRLsb = 0x2B,
    GasRMsb = 0x2A,
    HumLsb = 0x26,
    HumMsb = 0x25,
    TempXlsb = 0x24,
    TempLsb = 0x23,
    TempMsb = 0x22,
    PresXlsb = 0x21,
    PressLsb = 0x20,
    PressMsb = 0x1F,
    EasStatus0 = 0x1D,
}

/// Gas Sensor Modes.
///
/// During Sleep mode, no measurements are performed and there is minimal power consumption.
///
/// During Forced mode, a single TPHG (Temperature, Pressure, Humidty and Gas) is performed. Sensor automatically returns to sleep mode afterwards. Gas sensor heater only operates during gas measurement.
pub enum GSMode {
    Sleep,
    Forced,
}

/// BME680 operation errors.
#[derive(Format)]
pub enum SensorError {
    GetDataError,
    I2cError(ErrorKind),
}

impl<E: Error> From<E> for SensorError {
    fn from(value: E) -> Self {
        SensorError::I2cError(value.kind())
    }
}

impl<I2C: embedded_hal::i2c::I2c> GasSensor<I2C> {
    /// Initialise new gas sensor.
    pub fn new(i2c: I2C, addr: u8) -> Self {
        Self { i2c, addr }
    }

    /// Write to a register
    fn write(&mut self, reg_addr: Registers, data: &[u8]) -> Result<(), SensorError> {
        self.i2c.transaction(self.addr, &mut [Operation::Write(&[reg_addr as u8]), Operation::Write(data)])?;
        debug!("write to wind sensor: {:?}", data);
        Ok(())
    }

    /// Read from a register
    fn read(&mut self, reg: Registers) -> Result<[u8; 3], SensorError> {
        let mut read_buf = [0; 3];
        self.i2c.write_read(self.addr, &[reg as u8], &mut read_buf)?;
        debug!("read from wind sensor: {:?}", read_buf);
        Ok(read_buf)
    }

    /// Set
    pub fn set_mode(&mut self, mode: GSMode) -> Result<(), SensorError>{
        match mode {
            GSMode::Forced => {
                match self.write(Registers::CtrlMeas, &[1]) {
                    Ok(_) => {
                        debug!("Mode set to Forced");
                        Ok(())
                    },
                    Err(e) => {
                        error!("Could not set gas sensor mode to Forced: {:?}", e);
                        Err(e)
                    },
                }
            },
            GSMode::Sleep => {
                match self.write(Registers::CtrlMeas, &[0]) {
                    Ok(_) => {
                        debug!("Mode set to Sleep");
                        Ok(())
                    },
                    Err(e) => {
                        error!("Could not set gas sensor mode to Sleep: {:?}", e);
                        Err(e)
                    },
                }
            },
        }
    }

    pub fn get_air_quality(&mut self) -> Result<u16, SensorError> {
        todo!()
    }

    pub fn get_fault (&mut self) -> Result<u16, SensorError> {
        todo!()
    }
}
