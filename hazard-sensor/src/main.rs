#![no_std]
#![no_main]

use crate::sensors::{gas_sensor::GasSensor};
use crate::sensors::wind_direction::DirectionSensor;
use crate::sensors::watchdog::WatchdogTimer;
use crate::sensors::tipping_bucket::RainfallSensor;

use {defmt_rtt as _, panic_probe as _};

mod sensors;

use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_nrf::*;
use static_cell::{ConstStaticCell, StaticCell};
use embassy_time::Timer;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_sync::mutex::Mutex;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;

/// Gas sensor I2C slave address
const GAS_SENSOR_ADDR: u8 = 0x77;
/// Rainfall sensor I2C slave address
const RAINFALL_SENSOR_ADDR: u8 = 0x1D;
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

static I2C_BUS: StaticCell<Mutex<NoopRawMutex, twim::Twim>> = StaticCell::new();

/// NRF52840 struct containing all necessary peripherals
pub struct NRF52840 {
    i2c: twim::Twim<'static>,
    // uart1: uarte::Uarte<'static>,
}

impl NRF52840 {
    /// Initialise a new NRF52840 chip
    pub fn new() -> Self {
        let p = embassy_nrf::init(Default::default());
        let i2c_config = twim::Config::default();

        // First I2C bus, on TWISPI0
        let i2c = twim::Twim::new(p.TWISPI0, Irqs, p.P0_13, p.P0_14, i2c_config, TX_BUFF1.take());

        NRF52840 { i2c }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Hello, world!");
    let mcu = NRF52840::new();
    let i2c_bus = Mutex::new(mcu.i2c);
    let i2c_bus = I2C_BUS.init(i2c_bus);

    let gas_i2c = I2cDevice::new(i2c_bus);
    let rain_i2c = I2cDevice::new(i2c_bus);

    let mut gas_sensor =
        match GasSensor::new(gas_i2c, GAS_SENSOR_ADDR).await {
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

    let mut rainfall_sensor =
        match RainfallSensor::new(rain_i2c, RAINFALL_SENSOR_ADDR).await {
            Ok(sensor) => {
                info!("RAINFALL SENSOR initialised.");
                sensor
            },
            Err(e) => panic!("Could not intialise RAINFALL SENSOR: {:?}", e),
        };
    match rainfall_sensor.init_config().await {
        Ok(_) => info!("RAINFALL SENSOR configuration success."),
        Err(err) => panic!("Failed to configure RAINFALL SENSOR: {:?}", err),
    };

    // let mut direction_sensor = match DirectionSensor::new(mcu.i2c1, DIRECTION_SENSOR_ADDR).await {
    //     Ok(sensor) => {
    //         info!("DIRECTION SENSOR initialised.");
    //         sensor
    //     },
    //     Err(e) => panic!("Could not intialise DIRECTION SENSOR: {:?}", e),
    // };
    // match direction_sensor.init_config().await {
    //     Ok(_) => info!("DIRECTION SENSOR configuration success."),
    //     Err(err) => panic!("Failed to configure DIRECTION SENSOR: {:?}", err),
    // };

    loop {
        // let measurement = gas_sensor.get_measurements().await.unwrap();
        // info!("Temperature: {}, Humidity: {}, Pressure: {}, Air Quality: {}", measurement.0, measurement.1, measurement.2, measurement.3);

        let rainfall = rainfall_sensor.get_rainfall().await.unwrap();
        info!("Rainfall: {} mm", rainfall);

        // let direction = direction_sensor.get_direction_reading().await.unwrap();
        // info!("Direction: {} degrees", direction);

        Timer::after_millis(1000).await;
    }
}
