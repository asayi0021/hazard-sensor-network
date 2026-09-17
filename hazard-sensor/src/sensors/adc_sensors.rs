use crate::Irqs;
use async_modbus::client::{read_inputs, write_holding};
use defmt::{Format, debug};
use embassy_nrf::saadc::{ChannelConfig, Config, InterruptHandler, Saadc};
use embassy_nrf::{Peri, bind_interrupts, peripherals};

//use nrf_pac::{radio::vals::State::Rx, wdt::regs::Config};
//use nrf_pac::uarte::regs::Baudrate;

// SEN0193 Capacitive Soil Moisture Sensor
pub struct AdcSensors {
    saadc: Saadc<'static, 2>,
}

pub enum SensorError {
    GetDataError,
}

pub const SOIL_MOISTURE_CHANNEL: usize = 0; //Confirm ADC wiring
pub const WIND_SPEED_CHANNEL: usize = 1; //Confirm ADC wiring
pub const SOIL_LOWER_BOUND: f32 = 2934.0f32; //Place value here after sensor is calibrated
// pub const SOIL_UPPER_BOUND: f32 = 100f32; //Place value here after sensor is calibrated
// pub const SOIL_RANGE: f32 = SOIL_UPPER_BOUND - SOIL_LOWER_BOUND;

impl AdcSensors {
    pub fn new(saadc: Saadc<'static, 2>, // confirm actual RAK4631 mapping
    ) -> Self {
        Self { saadc }
    }

    pub async fn get_soil_moisture(&mut self) -> f32 {
        let mut buf = [0i16; 2];
        self.saadc.sample(&mut buf).await;
        let raw = buf[SOIL_MOISTURE_CHANNEL];
        raw as f32
        //(raw as f32 - SOIL_LOWER_BOUND) / SOIL_RANGE
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
