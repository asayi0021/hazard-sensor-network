use crate::Irqs;
use async_modbus::client::{read_inputs, write_holding};
use defmt::{Format, debug};
use embassy_nrf::saadc::{ChannelConfig, Config, InterruptHandler, Saadc};
use embassy_nrf::{Peri, bind_interrupts, peripherals};

//use nrf_pac::{radio::vals::State::Rx, wdt::regs::Config};
//use nrf_pac::uarte::regs::Baudrate;

// SEN0193 Capacitive Soil Moisture Sensor
pub struct AdcSensors {
    saadc: Saadc,
}

// pub enum SensorError {
//     GetDataError,
//     UARTError(uarte::Error),
//     UnexpectedDevice,
// }

pub const SOIL_MOISTURE_CHANNEL: usize = 0; //Confirm ADC wiring
pub const WIND_SPEED_CHANNEL: usize = 0; //Confirm ADC wiring

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

impl AdcSensors {
    fn new(saadc: Peri<'static, peripherals::SAADC>, // confirm actual RAK4631 mapping
    ) -> Self {
        Self { saadc }
    }

    pub async fn get_soil_moisture(adc: &mut Saadc<'static, 2>) -> i16 {
        let mut buf = [0i16; 2];
        adc.sample(&mut buf).await;
        buf[SOIL_MOISTURE_CHANNEL]
    }

    pub async fn get_wind_speed(&mut self) -> i16 {
        let mut buf = [0i16; 2];
        self.saadc.sample(&mut buf).await;
        buf[WIND_SPEED_CHANNEL]
    }

    pub fn get_fault() -> Result<u16, SensorError> {
        todo!()
    }
}
