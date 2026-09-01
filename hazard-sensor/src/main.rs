#![no_std]
#![no_main]

use crate::sensors::{gas_sensor::GasSensor};
use crate::sensors::wind_direction::DirectionSensor;

use {defmt_rtt as _, panic_probe as _};

mod sensors;

use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_nrf::*;
use static_cell::ConstStaticCell;
use embassy_time::Timer;

/// Gas sensor I2C slave address
const GAS_SENSOR_ADDR: u8 = 0x77;
/// Direction sensor I2C slave address
const DIRECTION_SENSOR_ADDR: u8 = 0x48;


bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
    TWISPI1 => twim::InterruptHandler<peripherals::TWISPI1>;
});

/// Transmission buffer for I2C Bus 1
static TX_BUFF1: ConstStaticCell<[u8; 16]> = ConstStaticCell::new([0; 16]);
/// Transmission buffer for I2C Bus 2
static TX_BUFF2: ConstStaticCell<[u8; 16]> = ConstStaticCell::new([0; 16]);

/// NRF52840 struct containing all necessary peripherals
pub struct NRF52840 {
    i2c1: twim::Twim<'static>,
    i2c2: twim::Twim<'static>,
    // uart1: uarte::Uarte<'static>,
}

impl NRF52840 {
    /// Initialise a new NRF52840 chip
    pub fn new() -> Self {
        let p = embassy_nrf::init(Default::default());
        let i2c1_config = twim::Config::default();
        let i2c2_config = twim::Config::default();

        // First I2C bus, on TWISPI0
        let i2c1 = twim::Twim::new(p.TWISPI0, Irqs, p.P0_13, p.P0_14, i2c1_config, TX_BUFF1.take());

        // Second I2C bus, on TWISPI1
        let i2c2 = twim::Twim::new(p.TWISPI1, Irqs, p.P0_15, p.P0_16, i2c2_config, TX_BUFF2.take());

        NRF52840 { i2c1, i2c2 }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Hello, world!");
    let mcu = NRF52840::new();

    let mut gas_sensor =
        match GasSensor::new(mcu.i2c1, GAS_SENSOR_ADDR).await {
            Ok(sensor) => {
                info!("GAS SENSOR initialised.");
                sensor
            },
            Err(e) => panic!("Could not intialise GAS SENSOR: {:?}", e),
        };
    match gas_sensor.init_config().await {
        Ok(_) => info!("GAS SENSOR configuration success."),
        Err(err) => panic!("Failed to configure GAS SENSOR: {:?}", err),
    };

    let mut direction_sensor = match DirectionSensor::new(mcu.i2c2, DIRECTION_SENSOR_ADDR).await {
        Ok(sensor) => {
            info!("DIRECTION SENSOR initialised.");
            sensor
        },
        Err(e) => panic!("Could not intialise DIRECTION SENSOR: {:?}", e),
    };
    match direction_sensor.init_config().await {
        Ok(_) => info!("DIRECTION SENSOR configuration success."),
        Err(err) => panic!("Failed to configure DIRECTION SENSOR: {:?}", err),
    };

    loop {
        let measurement = gas_sensor.get_measurements().await.unwrap();
        // info!("Temperature: {}, Humidity: {}, Pressure: {}, Air Quality: {}", measurement.0, measurement.1, measurement.2, measurement.3);

        let direction = direction_sensor.get_direction_reading().await.unwrap();
        info!("Direction: {} degrees", direction);

        Timer::after_secs(3).await;
    }
}
