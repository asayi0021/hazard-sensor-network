use embassy_nrf::*;
use defmt::info;

pub struct WindSensor {
    port: twim::TWIM<'static>,
}

impl WindSensor {
    pub fn new(port: twim::TWIM<'static>) -> Self {
        Self { port }
    }

    pub fn write(data: [u8; 3]) {
        self.port.write(data)
    }

    pub fn get_windspeed() -> u16 {
        todo!()
    }
}
