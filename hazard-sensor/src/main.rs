#![no_std]
#![no_main]

use {defmt_rtt as _, panic_probe as _};

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
}

impl NRF52840 {
    pub fn new() -> Self {
        let p = embassy_nrf::init(Default::default());
        let config = twim::Config::default();

        // Initialize the TWIM driver
        let mut i2c = twim::Twim::new(p.TWISPI0, Irqs, p.P0_13, p.P0_14, config, TX_BUFF.take());

        NRF52840 {
            i2c,
        }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());

    info!("Hello, world!");
}
