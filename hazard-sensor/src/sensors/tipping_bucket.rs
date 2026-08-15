use defmt::debug;
use embassy_nrf::*;

pub struct TippingBucket {
    port: twim::Twim<'static>,
    addr: u8,
}

pub enum SensorError {
    GetDataError,
    UARTError(twim::Error),
}

impl From<twim::Error> for SensorError {
    fn from(value: twim::Error) -> Self {
        SensorError::UARTError(value)
    }
}

impl TippingBucket {
    pub fn new(port: twim::Twim<'static>, addr: u8) -> Self {
        Self { port, addr }
    }

    pub fn write(&mut self, data: [u8; 3]) -> Result<(), SensorError> {
        self.port.blocking_write(self.addr, &data)?;
        debug!("write to tipping bucket: {:?}", data);
        Ok(())
    }

    pub fn read(&mut self) -> Result<[u8; 3], SensorError> {
        let mut read_buf = [0; 3];
        self.port.blocking_read(self.addr, &mut read_buf)?;
        debug!("read from tipping bucket: {:?}", read_buf);
        Ok(read_buf)
    }

    pub fn get_tipping_bucket() -> Result<u16, SensorError> {
        todo!()
    }

    pub fn get_fault() -> Result<u16, SensorError> {
        todo!()
    }
}
