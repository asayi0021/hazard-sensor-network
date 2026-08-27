use crate::Irqs;
use async_modbus::client::{read_inputs, write_holding};
use defmt::{Format, debug};
use embassy_nrf::saadc::{ChannelConfig, Config, InterruptHandler, Saadc};
use embassy_nrf::{Peri, bind_interrupts, peripherals};

//use nrf_pac::{radio::vals::State::Rx, wdt::regs::Config};
//use nrf_pac::uarte::regs::Baudrate;

// SEN0193 Capacitive Soil Moisture Sensor
pub struct SoilMoisture {
    //     //
    //     parity: uarte::Parity,
    //     //
    //     baudrate: uarte::Baudrate,
    //     //
}

// pub enum SensorError {
//     GetDataError,
//     UARTError(uarte::Error),
//     UnexpectedDevice,
// }

pub const SOIL_MOISTURE_CHANNEL: usize = 0; //Confirm ADC wiring

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

impl SoilMoisture {
    // This relies on the ADC being initialised in main to be able to be used
    pub async fn get_soil_moisture(adc: &mut Saadc<'static, 2>) -> i16 {
        let mut buf = [0i16; 2];
        adc.sample(&mut buf).await;
        buf[SOIL_MOISTURE_CHANNEL]
    }

    pub fn get_fault() -> Result<u16, SensorError> {
        todo!()
    }
}
