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

fn modbus_crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= byte as u16;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

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
        uart_config.baudrate = uarte::Baudrate::Baud9600; //Based on what found in the given library for the sensor

        let adc_config = saadc::Config::default();
        let channel0 = ChannelConfig::single_ended(p.P0_03);
        let channel1 = ChannelConfig::single_ended(p.P0_31); //Double check pins
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
        let uart = uarte::Uarte::new(p.UARTE1, p.P0_15, p.P0_16, Irqs, uart_config);

        //TippingBucket::init_tipping_bucket_uarte(p.UARTE1, p.P0_19, p.P0_20);

        NRF52840 { i2c, saadc, uart }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mcu = NRF52840::new();

    let mut adc = AdcSensors::new(mcu.saadc);

    info!("Hello, world!");

    loop {
        let windspeed = adc.get_wind_speed().await;
        defmt::info!("Windspeed: {}m/s", windspeed);
        embassy_time::Timer::after_secs(1).await;
    }

    // loop {
    //     match adc.get_soil_moisture().await {
    //         Ok(raw) => defmt::info!("Moisture Capacitance: {}", raw),
    //         Err(_) => defmt::error!("Failed to read moisture sensor"),
    //     }
    // }

    // let tipping_bucket = TippingBucket::new(mcu.uart);
    // let mut uart = mcu.uart;

    // let test_bytes = [0xDE, 0xAD, 0xBE, 0xEF];

    // loop {
    //     defmt::info!("Sending loopback test bytes");
    //     uart.write(&test_bytes).await.ok();

    //     let mut buf = [0u8; 4];
    //     match embassy_time::with_timeout(embassy_time::Duration::from_secs(1), uart.read(&mut buf))
    //         .await
    //     {
    //         Ok(Ok(())) => defmt::info!("Loopback received: {:02X}", buf),
    //         Ok(Err(_)) => defmt::error!("Loopback UART read error"),
    //         Err(e) => defmt::error!("UART READ FAILED: {}", e),
    //     }
    //     embassy_time::Timer::after_secs(3).await;
    // }

    // let mut tipping_bucket = match tipping_bucket.await {
    //     Ok(tb) => tb,
    //     Err(e) => {
    //         defmt::error!("Failed to initialise tipping bucket");
    //         // decide how to handle this — panic, retry, skip the sensor, etc.
    //         panic!("tipping bucket init failed");
    //     }
    // };

    // info!("Hello, world! two");
    // loop {
    //     match tipping_bucket.get_tipping_bucket(1).await {
    //         Ok(rainfall_mm) => defmt::info!("Rainfall: {} mm", rainfall_mm),
    //         Err(_) => defmt::error!("Failed to read tipping bucket"),
    //     }

    //     embassy_time::Timer::after_secs(5).await; // poll once a minute, or whatever interval fits
    // }
}
