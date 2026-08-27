use crate::{Irqs, sensors::tipping_bucket::InputRegister::TimeRainfallLReg};
use async_modbus::client::{read_inputs, write_holding};
use defmt::{Format, debug};
use embassy_nrf::{
    Peri, peripherals,
    uarte::{self, Baudrate, Config, Parity, Uarte},
};

//use nrf_pac::{radio::vals::State::Rx, wdt::regs::Config};
//use nrf_pac::uarte::regs::Baudrate;

// SEN0575 Tipping Bucket Rainfall Sensor
pub struct TippingBucket {
    //     //
    //     parity: uarte::Parity,
    //     //
    //     baudrate: uarte::Baudrate,
    //     //
}

#[derive(Format, Clone)]
//The names of these registers mirror the given python library's input register names for the tipping bucket sensor
pub enum InputRegister {
    InputRegPID = 0x0000,
    InputRegVID = 0x0001,
    InputRegBaud = 0x0003,
    InputRegVerifyAndStop = 0x0004,
    TimeRainfallLReg = 0x0006,
}

#[derive(Format, Clone)]
//The names of these registers mirror the given python library's holding register names for the tipping bucket sensor
pub enum HoldingRegister {
    RainHourHoldingReg = 0x0006,
}

// Hardcoded values found in the given python library
pub const SLAVE_ADDR: u8 = 0xC0;
pub const READ_REGISTER_COUNT: u16 = 2;
pub const EXPECTED_PID: u32 = 0x100C0;
pub const EXPECTED_VID: u32 = 0x3343;

pub enum SensorError {
    GetDataError,
    UARTError(uarte::Error),
    UnexpectedDevice,
}

impl From<uarte::Error> for SensorError {
    fn from(value: uarte::Error) -> Self {
        SensorError::UARTError(value)
    }
}

impl From<read_inputs> for SensorError {
    fn from(value: uarte::Error) -> Self {
        SensorError(value)
    }
}

impl TippingBucket {
    // Initialising the uarte connection for the tipping bucket sensor
    pub fn init_tipping_bucket_uarte(
        uarte1: Peri<'static, peripherals::UARTE1>,
        rx_pin: Peri<'static, peripherals::P0_09>, //Double check that this pin is correct
        tx_pin: Peri<'static, peripherals::P0_10>, //Double check that this pin is correct
    ) -> Result<Uarte<'static>, SensorError> {
        let mut config = Config::default();
        config.parity = Parity::Excluded; //Based on what found in the given library for the sensor
        config.baudrate = Baudrate::Baud9600; //Based on what found in the given library for the sensor
        let uart = Uarte::new(uarte1, rx_pin, tx_pin, Irqs, config);
        debug!("tipping bucket initialised succesfully");
        Ok(uart)
    }

    // Initialising the modbus connection for the tipping bucket sensor
    pub async fn init_tipping_bucket_modbus(
        uart: &mut Uarte<'static>,
    ) -> Result<(u32, u32), SensorError> {
        let regs =
            read_inputs::<2, _>(&mut *uart, SLAVE_ADDR, InputRegiser::InputRegPID as u16).await?;

        let reg0 = regs[0].get() as u32; // PID low word
        let reg1 = regs[1].get() as u32; // VID + PID high bits, packed

        let pid = ((reg1 & 0xC000) << 2) | reg0;
        let vid = reg1 & 0x3FFF;

        Ok((pid, vid))
    }
    // Initialising the tipping bucket sensor - higher level
    pub async fn init_tipping_bucket(
        uarte1: Peri<'static, peripherals::UARTE1>,
        rx_pin: Peri<'static, peripherals::P0_09>, //Double check that this pin is correct
        tx_pin: Peri<'static, peripherals::P0_10>, //Double check that this pin is correct
    ) -> Result<Uarte<'static>, SensorError> {
        let mut uart = Self::init_tipping_bucket_uarte(uarte1, rx_pin, tx_pin)?;
        let (pid, vid) = Self::init_tipping_bucket_modbus(&mut uart).await?;

        if pid != EXPECTED_PID || vid != EXPECTED_VID {
            //find the expected values
            return Err(SensorError::UnexpectedDevice); // check how errors should be handled
        }

        Ok(uart)
    }

    // Gets the tipping bucket value from the sensor - specifially from the set time cumulative rainfall registers
    // Potentially add a flag for div by 10000.0 But need to decide later.
    pub async fn get_tipping_bucket(
        uart: &mut Uarte<'static>,
        hours: u8,
    ) -> Result<f32, SensorError> {
        write_holding(
            &mut *uart,
            SLAVE_ADDR,
            RainHourHoldingReg as u16,
            hours as u16,
        )
        .await?;

        let regs = read_inputs::<2, _>(
            &mut *uart,
            SLAVE_ADDR,
            InputRegister::TimeRainfallLReg as u16,
        )
        .await?;
        let raw = (regs[1].get() as u32) << 16 | (regs[0].get() as u32);
        let rainfall_mm = raw as f32 / 10000.0;

        Ok(rainfall_mm)
    }

    pub fn get_fault() -> Result<u16, SensorError> {
        todo!()
    }
}
