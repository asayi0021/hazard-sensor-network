use crate::Irqs;
use async_modbus::client::{read_inputs, write_holding};
use defmt::debug;
use embassy_nrf::{
    Peri, peripherals,
    uarte::{self, Baudrate, Config, Parity, Uarte},
};

const RAIN_HOUR_HOLDING_REG: u16 = 0x0006;
const TIME_RAINFALL_L_REG: u16 = 0x0006;

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
    // Initialising the uarte connection for the tipping bucket sensor
    pub fn init_tipping_bucket_uarte(
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

    // Initialising the modbus connection for the tipping bucket sensor
    pub async fn init_tipping_bucket_modbus(
        uart: &mut Uarte<'static>,
    ) -> Result<(u32, u32), SensorError> {
        let regs = read_inputs::<2, _>(&mut *uart, SLAVE_ADDR, PID_VID_REG).await?;

        let reg0 = regs[0].get() as u32; // PID low word
        let reg1 = regs[1].get() as u32; // VID + PID high bits, packed

        let pid = ((reg1 & 0xC000) << 2) | reg0;
        let vid = reg1 & 0x3FFF;

        Ok((pid, vid))
    }
    // Initialising the tipping bucket sensor - higher level
    pub async fn init_tipping_bucket(
        uarte1: Peri<'static, peripherals::UARTE1>,
        rx_pin: Peri<'static, peripherals::P0_09>, //Double check that this pin is correct
        tx_pin: Peri<'static, peripherals::P0_10>, //Double check that this pin is correct
    ) -> Result<Uarte<'static>, SensorError> {
        let mut uart = init_tipping_bucket_uarte(uarte1, rx_pin, tx_pin)?;
        let (pid, vid) = init_tipping_bucket_modbus(&mut uart).await?;

        if pid != EXPECTED_PID || vid != EXPECTED_VID {
            //find the expected values
            return Err(SensorError::UnexpectedDevice); // check how errors should be handled
        }

        Ok(uart)
    }

    // pub fn new(port: twim::Twim<'static>, addr: u8) -> Self {
    //     Self { port, addr }
    // }

    // Don't think a write function is needed, can just use the write_holding function from async_modbus
    // pub async fn write(&mut self, data: [u8; 3]) -> Result<(), SensorError> {
    //     self.port.blocking_write(self.addr, &data)?;
    //     debug!("write to tipping bucket: {:?}", data);
    //     Ok(())
    // }

    // Don't think a read function is needed, can just use the read_inputs function from async_modbus
    // pub async fn read(&mut self) -> Result<[u8; 3], SensorError> {
    //     let mut read_buf = [0; 3];
    //     self.port.blocking_read(self.addr, &mut read_buf)?;
    //     debug!("read from tipping bucket: {:?}", read_buf);
    //     Ok(read_buf)
    // }

    // Gets the tipping bucket value from the sensor - specifially from the set time cumulative rainfall registers
    pub async fn get_tipping_bucket(
        uart: &mut Uarte<'static>,
        slave_addr: u8,
        hours: u8,
    ) -> Result<f32, SensorError> {
        write_holding(&mut *uart, slave_addr, RAIN_HOUR_HOLDING_REG, hours as u16).await?;

        let regs = read_inputs::<2, _>(&mut *uart, slave_addr, TIME_RAINFALL_L_REG).await?;
        let raw = (regs[1].get() as u32) << 16 | (regs[0].get() as u32);
        let rainfall_mm = raw as f32 / 10000.0;

        Ok(rainfall_mm)
    }

    pub fn get_fault() -> Result<u16, SensorError> {
        todo!()
    }
}
