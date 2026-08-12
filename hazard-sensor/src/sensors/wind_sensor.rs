use embassy_nrf::*;
use defmt::debug;

pub struct WindSensor {
    port: twim::Twim<'static>,
    addr: u8,
}

pub enum SensorError {
    GetDataError,
    I2cError(twim::Error),
}

impl From<twim::Error> for SensorError {
    fn from(value: twim::Error) -> Self {
        SensorError::I2cError(value)
    }
}

impl WindSensor {
    pub fn new(port: twim::Twim<'static>, addr: u8) -> Self {
        Self { port, addr }
    }

    pub fn write(&mut self, data: [u8; 3]) -> Result<(), SensorError> {
        self.port.blocking_write(self.addr, &data)?;
        debug!("write to wind sensor: {:?}", data);
        Ok(())
    }

    pub fn read(&mut self) -> Result<[u8; 3], SensorError> {
        let mut read_buf = [0; 3];
        self.port.blocking_read(self.addr, &mut read_buf)?;
        debug!("read from wind sensor: {:?}", read_buf);
        Ok(read_buf)
    }

    pub fn get_wind_speed() -> Result<u16, SensorError> {
        todo!()
    }

    pub fn get_fault () -> Result<u16, SensorError> {
        todo!()
    }
}
