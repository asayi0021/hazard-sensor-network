#![no_std]
#![no_main]

use crate::sensors::gas_sensor::GasSensor;

use {defmt_rtt as _, panic_probe as _};

mod sensors;

use defmt::info;
use embassy_executor::Spawner;
use embassy_nrf::*;
use static_cell::ConstStaticCell;

bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
});

static TX_BUFF: ConstStaticCell<[u8; 16]> = ConstStaticCell::new([0; 16]);

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
    let mcu = NRF52840::new();

    let gas_sensor = GasSensor::new(mcu.i2c, 0x10);


    info!("Hello, world!");
}
