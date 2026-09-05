use crate::{Irqs, sensors::tipping_bucket::InputRegister::TimeRainfallLReg};
use async_modbus::client::{read_inputs, write_holding};
use defmt::{Format, debug};
use embassy_nrf::{
    Peri, peripherals,
    uarte::{self, Baudrate, Config, Error as UarteError, Parity, Uarte},
};
use embedded_io_async::{Error as EioError, ErrorKind, ErrorType, Read, Write};
//use nrf_pac::{radio::vals::State::Rx, wdt::regs::Config};
//use nrf_pac::uarte::regs::Baudrate;

// Wraps embassy-nrf's UARTE error type so it can implement `embedded_io_async::Error`.
// A direct impl isn't allowed here — neither the trait nor `UarteError` live in this crate.
#[derive(Debug, Clone, Copy)]
pub struct UartError(pub UarteError);

impl core::fmt::Display for UartError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "UART error")
    }
}

impl core::error::Error for UartError {}

impl EioError for UartError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

pub struct UarteAdapter<'d>(pub Uarte<'d>);

impl<'d> ErrorType for UarteAdapter<'d> {
    type Error = UartError;
}

impl<'d> Read for UarteAdapter<'d> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.0.read(buf).await.map_err(UartError)?;
        Ok(buf.len())
    }
}

impl<'d> Write for UarteAdapter<'d> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.0.write(buf).await.map_err(UartError)?;
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

// SEN0575 Tipping Bucket Rainfall Sensor
pub struct TippingBucket {
    uart: UarteAdapter<'static>,
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
    ModbusError(async_modbus::client::Error<UartError>),
    UnexpectedDevice,
}

impl From<async_modbus::client::Error<UartError>> for SensorError {
    fn from(value: async_modbus::client::Error<UartError>) -> Self {
        SensorError::ModbusError(value)
    }
}

impl From<uarte::Error> for SensorError {
    fn from(value: uarte::Error) -> Self {
        SensorError::UARTError(value)
    }
}

impl TippingBucket {
    // Initialising the uarte connection for the tipping bucket sensor
    pub fn init_tipping_bucket_uarte(
        uarte1: Peri<'static, peripherals::UARTE1>,
        rx_pin: Peri<'static, peripherals::P0_13>, //Double check that this pin is correct
        tx_pin: Peri<'static, peripherals::P0_14>, //Double check that this pin is correct
    ) -> Uarte<'static> {
        let mut config = Config::default();
        config.parity = Parity::Excluded; //Based on what found in the given library for the sensor
        config.baudrate = Baudrate::Baud9600; //Based on what found in the given library for the sensor
        let uart = Uarte::new(uarte1, rx_pin, tx_pin, Irqs, config);
        debug!("tipping bucket initialised succesfully");
        uart
    }

    // Initialising the modbus connection for the tipping bucket sensor
    pub async fn init_tipping_bucket_modbus(
        adapter: &mut UarteAdapter<'static>,
    ) -> Result<(u32, u32), SensorError> {
        let regs =
            read_inputs::<2, _>(adapter, SLAVE_ADDR, InputRegister::InputRegPID as u16).await?;

        let reg0 = regs[0].get() as u32; // PID low word
        let reg1 = regs[1].get() as u32; // VID + PID high bits, packed

        let pid = ((reg1 & 0xC000) << 2) | reg0;
        let vid = reg1 & 0x3FFF;

        Ok((pid, vid))
    }
    // Initialising the tipping bucket sensor - higher level
    pub async fn new(
        uarte1: Peri<'static, peripherals::UARTE1>,
        rx_pin: Peri<'static, peripherals::P0_13>, //Double check that this pin is correct
        tx_pin: Peri<'static, peripherals::P0_14>, //Double check that this pin is correct
    ) -> Result<Self, SensorError> {
        let mut uart = Self::init_tipping_bucket_uarte(uarte1, rx_pin, tx_pin);
        let mut adapter = UarteAdapter(uart);

        let (pid, vid) = Self::init_tipping_bucket_modbus(&mut adapter).await?;

        if pid != EXPECTED_PID || vid != EXPECTED_VID {
            //find the expected values
            return Err(SensorError::UnexpectedDevice); // check how errors should be handled
        }

        Ok(Self { uart: adapter })
    }

    // Gets the tipping bucket value from the sensor - specifially from the set time cumulative rainfall registers
    // Potentially add a flag for div by 10000.0 But need to decide later.
    pub async fn get_tipping_bucket(&mut self, hours: u8) -> Result<f32, SensorError> {
        write_holding(
            &mut self.uart,
            SLAVE_ADDR,
            HoldingRegister::RainHourHoldingReg as u16,
            hours as u16,
        )
        .await?;

        let regs = read_inputs::<2, _>(
            &mut self.uart,
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
