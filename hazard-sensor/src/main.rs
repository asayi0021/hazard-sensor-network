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
    uarte::Uarte,
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
    uart: Uarte<'static>,
}

impl NRF52840 {
    pub fn new() -> Self {
        let p = embassy_nrf::init(Default::default());
        let twim_config = twim::Config::default();
        let mut uart_config = uarte::Config::default();
        uart_config.parity = uarte::Parity::Excluded; //Based on what found in the given library for the sensor
        uart_config.baudrate = uarte::Baudrate::Baud9600; //Based on what found in the given library for the sensor

        let adc_config = saadc::Config::default();
        let channel0 = ChannelConfig::single_ended(p.P0_02); //Double check pins
        let channel1 = ChannelConfig::single_ended(p.P0_03); //Double check pins
        let saadc = Saadc::new(p.SAADC, Irqs, adc_config, [channel0, channel1]);

        // Initialise the TWIM driver
        let i2c = twim::Twim::new(
            p.TWISPI0,
            Irqs,
            p.P0_13,
            p.P0_14,
            twim_config,
            TX_BUFF.take(),
        );

        //Initialise the UARTE driver
        let uart = uarte::Uarte::new(p.UARTE1, p.P0_19, p.P0_20, Irqs, uart_config);

        //TippingBucket::init_tipping_bucket_uarte(p.UARTE1, p.P0_19, p.P0_20);

        NRF52840 { i2c, saadc, uart }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mcu = NRF52840::new();

    let adc = AdcSensors::new(mcu.saadc);

    info!("Hello, world!");

    let tipping_bucket = TippingBucket::new(mcu.uart);

    let mut tipping_bucket = match tipping_bucket.await {
        Ok(tb) => tb,
        Err(e) => {
            defmt::error!("Failed to initialise tipping bucket");
            // decide how to handle this — panic, retry, skip the sensor, etc.
            panic!("tipping bucket init failed");
        }
    };

    info!("Hello, world! two");
    loop {
        match tipping_bucket.get_tipping_bucket(1).await {
            Ok(rainfall_mm) => defmt::info!("Rainfall: {} mm", rainfall_mm),
            Err(_) => defmt::error!("Failed to read tipping bucket"),
        }

        embassy_time::Timer::after_secs(5).await; // poll once a minute, or whatever interval fits
    }
}
