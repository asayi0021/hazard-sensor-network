#![no_std]
#![no_main]

use crate::sensors::gas_sensor::GasSensor;

use {defmt_rtt as _, panic_probe as _};

mod sensors;

use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_nrf::*;
use static_cell::ConstStaticCell;
use embassy_time::Timer;

bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
});

static TX_BUFF: ConstStaticCell<[u8; 16]> = ConstStaticCell::new([0; 16]);
const GAS_SENSOR_ADDR: u8 = 0x77;

pub struct NRF52840 {
    i2c: twim::Twim<'static>,
    // uart1: uarte::Uarte<'static>,
}

impl NRF52840 {
    pub fn new() -> Self {
        let p = embassy_nrf::init(Default::default());
        let config = twim::Config::default();

        // Initialize the TWIM driver
        let i2c = twim::Twim::new(p.TWISPI0, Irqs, p.P0_13, p.P0_14, config, TX_BUFF.take());
        // let mut uart1 = uarte::Uarte::new(uarte, rxd, txd, irq, config);

        NRF52840 {
            i2c,
            // uart1,
        }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Hello, world!");
    let mcu = NRF52840::new();

    let mut gas_sensor =
        match GasSensor::new(mcu.i2c, GAS_SENSOR_ADDR).await {
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

    loop {
        let measurement = gas_sensor.get_measurements().await.unwrap();
        // info!("Temperature: {}, Humidity: {}, Pressure: {}, Air Quality: {}", measurement.0, measurement.1, measurement.2, measurement.3);
        Timer::after_secs(3).await;
    }
}
