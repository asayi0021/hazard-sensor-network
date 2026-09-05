#![no_std]
#![no_main]

use {
    crate::sensors::tipping_bucket::TippingBucket, core::error::Error, defmt_rtt as _,
    panic_probe as _,
};

mod sensors;

use defmt::info;
use embassy_executor::Spawner;
use embassy_nrf::{
    saadc::{ChannelConfig, Config, Saadc},
    *,
};
use sensors::adc_sensors::AdcSensors;
use static_cell::ConstStaticCell;

bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
    UARTE1  => uarte::InterruptHandler<peripherals::UARTE1>;
    SAADC => saadc::InterruptHandler;
});

static TX_BUFF: ConstStaticCell<[u8; 16]> = ConstStaticCell::new([0; 16]);

pub struct NRF52840 {
    i2c: twim::Twim<'static>,
    saadc: Saadc<'static, 2>,
}

impl NRF52840 {
    pub fn new() -> Self {
        let p = embassy_nrf::init(Default::default());
        let twim_config = twim::Config::default();

        let adc_config = saadc::Config::default();
        let channel0 = ChannelConfig::single_ended(p.P0_02); //Double check pins
        let channel1 = ChannelConfig::single_ended(p.P0_03); //Double check pins
        let saadc = Saadc::new(p.SAADC, Irqs, adc_config, [channel0, channel1]);

        // Initialize the TWIM driver
        let mut i2c = twim::Twim::new(
            p.TWISPI0,
            Irqs,
            p.P0_13,
            p.P0_14,
            twim_config,
            TX_BUFF.take(),
        );

        NRF52840 { i2c, saadc }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mcu = NRF52840::new();

    let adc = AdcSensors::new(mcu.saadc);

    info!("Hello, world!");
}
