#![no_std]
#![no_main]

use {
    crate::sensors::soil_moisture::SoilMoisture, crate::sensors::tipping_bucket::TippingBucket,
    core::error::Error, defmt_rtt as _, panic_probe as _,
};

mod sensors;

use defmt::info;
use embassy_executor::Spawner;
use embassy_nrf::{
    saadc::{ChannelConfig, Config, Saadc},
    *,
};
use sensors::soil_moisture;
use sensors::tipping_bucket;
use static_cell::ConstStaticCell;

bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
    UARTE1  => uarte::InterruptHandler<peripherals::UARTE1>;
    SAADC => saadc::InterruptHandler;
});

fn init_adc(
    saadc: Peri<'static, peripherals::SAADC>,
    soil_moisture_pin: Peri<'static, peripherals::P0_02>, // confirm actual RAK4631 mapping
    wind_speed_pin: Peri<'static, peripherals::P0_03>,    // confirm actual RAK4631 mapping
) -> Saadc<'static, 2> {
    let config = Config::default();
    let channel0 = ChannelConfig::single_ended(soil_moisture_pin);
    let channel1 = ChannelConfig::single_ended(wind_speed_pin);
    Saadc::new(saadc, Irqs, config, [channel0, channel1])
}

static TX_BUFF: ConstStaticCell<[u8; 16]> = ConstStaticCell::new([0; 16]);

pub struct NRF52840 {
    i2c: twim::Twim<'static>,
}

impl NRF52840 {
    pub fn new() -> Self {
        let p = embassy_nrf::init(Default::default());
        let config = twim::Config::default();

        // Initialize the TWIM driver
        let mut i2c = twim::Twim::new(p.TWISPI0, Irqs, p.P0_13, p.P0_14, config, TX_BUFF.take());

        NRF52840 { i2c }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mcu = NRF52840::new();

    let ws = WindSensor::new(mcu.i2c);

    info!("Hello, world!");
}
