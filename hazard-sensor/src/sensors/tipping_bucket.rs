use crate::Irqs;
use defmt::debug;
use embassy_nrf::{
    Peri, peripherals,
    uarte::{self, Baudrate, Config, Parity, Uarte},
};
//use nrf_pac::{radio::vals::State::Rx, wdt::regs::Config};
//use nrf_pac::uarte::regs::Baudrate;

// SEN0575 Tipping Bucket Rainfall Sensor
pub struct TippingBucket {
    //
    parity: uarte::Parity,
    //
    baudrate: uarte::Baudrate,
}

pub enum SensorError {
    GetDataError,
    UARTError(uarte::Error),
}

impl From<uarte::Error> for SensorError {
    fn from(value: uarte::Error) -> Self {
        SensorError::UARTError(value)
    }
}

impl TippingBucket {
    // Initialising the tipping bucket sensor
    pub fn init_tipping_bucket(
        uarte1: Peri<'static, peripherals::UARTE1>,
        rx_pin: Peri<'static, peripherals::P0_09>, //Double check that this pin is correct
        tx_pin: Peri<'static, peripherals::P0_10>, //Double check that this pin is correct
    ) -> Result<Uarte<'static>, SensorError> {
        let mut config = Config::default();
        config.parity = Parity::Included; //Double check later what parity is needed
        config.baudrate = Baudrate::Baud115200; //Double check later what baudrate is needed
        let uart = Uarte::new(uarte1, rx_pin, tx_pin, Irqs, config);
        debug!("tipping bucket initialised succesfully");
        Ok(uart)
    }

    // pub fn new(port: twim::Twim<'static>, addr: u8) -> Self {
    //     Self { port, addr }
    // }

    pub async fn write(&mut self, data: [u8; 3]) -> Result<(), SensorError> {
        self.port.blocking_write(self.addr, &data)?;
        debug!("write to tipping bucket: {:?}", data);
        Ok(())
    }

    // pub async fn read(&mut self) -> Result<[u8; 3], SensorError> {
    //     let mut read_buf = [0; 3];
    //     self.port.blocking_read(self.addr, &mut read_buf)?;
    //     debug!("read from tipping bucket: {:?}", read_buf);
    //     Ok(read_buf)
    // }

    pub fn get_tipping_bucket() -> Result<u16, SensorError> {
        todo!()
    }

    pub fn get_fault() -> Result<u16, SensorError> {
        todo!()
    }
}
