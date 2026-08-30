//! Driver for the BME680 gas sensor.
//!
//! Provides all the necessary functions for collecting temperature, pressure, air quality and humidity measurements.

use defmt::{Format, debug, error, info, trace};
use embedded_hal_async::i2c::{Error, ErrorKind, Operation, SevenBitAddress};
use embassy_time::Timer;

/// BME680 Gas Sensor.
pub struct GasSensor<I2C> {
    /// I2C bus from NRF52840
    i2c: I2C,
    /// I2C address of sensor
    addr: SevenBitAddress,
    calibration: CalibrationData,
}

/// Calibration/trim values needed to compute measurement values from raw ADC readings.
/// These are burned into the sensor at the factory and read once at startup.
#[derive(Debug)]
pub struct CalibrationData {
    par_t1: i16,
    par_t2: i16,
    par_t3: i8,
    par_p1: u16,
    par_p2: i16,
    par_p3: i8,
    par_p4: i16,
    par_p5: i16,
    par_p6: i8,
    par_p7: i8,
    par_p8: i16,
    par_p9: i16,
    par_p10: i8,
    par_h1: u16,
    par_h2: i16,
    par_h3: i8,
    par_h4: i8,
    par_h5: i8,
    par_h6: u8,
    par_h7: i8,
    par_g1: i8,
    par_g2: i16,
    par_g3: i8,
    res_heat_range: u8,
    res_heat_val: i8,
    range_switching_error: i8,
}

/// List of read/writable registers (and their address) on the gas sensor.
/// For more details, see [BME680 datasheet][registers].
///
/// [registers]: https://www.bosch-sensortec.com/media/boschsensortec/downloads/datasheets/bst-bme680-ds001.pdf
#[derive(Format, Clone)]
pub enum Registers {
    Status = 0x73,
    Reset = 0xE0,
    Id = 0xD0,
    Config = 0x75,
    CtrlMeas = 0x74,
    CtrlHum = 0x72,
    CtrlGas1 = 0x71,
    CtrlGas0 = 0x70,

    // Gas control registers
    // There are actually 10 registers for each of the 10 set-points.
    // The recorded address is just the addressof set-point 0. The next 9 address in sequence correspond to
    // each subsequent set-point sequentially.
    // x is the set-point (ranging from 0 to 9).
    // To get to a certain X value, just add X to the base address.
    GasWaitX = 0x64,
    ResHeatX = 0x5A,
    IdacHeatX = 0x50,

    GasRLsb = 0x2B,
    GasRMsb = 0x2A,
    HumLsb = 0x26,
    HumMsb = 0x25,
    TempXlsb = 0x24,
    TempLsb = 0x23,
    TempMsb = 0x22,
    PresXlsb = 0x21,
    PressLsb = 0x20,
    PressMsb = 0x1F,
    MeasStatus0 = 0x1D,
    Calib1 = 0x00,
    Calib2 = 0xEB,
}

/// Look-up table 1 for gas resistance calculation, indexed by gas_range (0-15).
/// Values taken directly from Bosch's BME680 reference driver / datasheet.
const LOOKUP_TABLE_1: [u32; 16] = [
    2147483647, 2147483647, 2147483647, 2147483647,
    2147483647, 2126008810, 2147483647, 2130303777,
    2147483647, 2147483647, 2143188679, 2136746228,
    2147483647, 2126008810, 2147483647, 2147483647,
];

/// Look-up table 2 for gas resistance calculation, indexed by gas_range (0-15).
const LOOKUP_TABLE_2: [u32; 16] = [
    4096000000, 2048000000, 1024000000, 512000000,
    255744255, 127110228, 64000000, 32258064,
    16016016, 8000000, 4000000, 2000000,
    1000000, 500000, 250000, 125000,
];

/// Gas Sensor Modes.
///
/// During Sleep mode, no measurements are performed and there is minimal power consumption.
///
/// During Forced mode, a single TPHG (Temperature, Pressure, Humidty and Gas) is performed. Sensor automatically returns to sleep mode afterwards. Gas sensor heater only operates during gas measurement.
pub enum GSMode {
    Sleep,
    Forced,
}

/// BME680 operation errors.
#[derive(Format, Debug)]
pub enum SensorError {
    GetDataError,
    I2cError(ErrorKind),
}

impl<E: Error> From<E> for SensorError {
    fn from(value: E) -> Self {
        SensorError::I2cError(value.kind())
    }
}

impl<I2C: embedded_hal_async::i2c::I2c> GasSensor<I2C> {
    /// Initialise new gas sensor.
    pub async fn new(mut i2c: I2C, addr: u8) -> Result<Self, SensorError> {
        let calibration = match Self::read_calibration_raw(&mut i2c, addr).await {
            Ok(calibration) => calibration,
            Err(e) => {
                error!("Failed to read calibration data: {}", e);
                return Err(e);
            }
        };
        Ok(Self {
            i2c,
            addr,
            calibration,
        })
    }

    /// Write to one or more registers
    async fn write(&mut self, reg_addr: Registers, data: &[u8]) -> Result<(), SensorError> {
        let reg_cp = reg_addr.clone();

        // Build one contiguous buffer: [register address, data...].
        // 8 bytes is comfortably larger than any register write this driver performs.
        let mut buf = [0u8; 8];
        buf[0] = reg_addr as u8;
        buf[1..1 + data.len()].copy_from_slice(data);

        match self.i2c.write(
            self.addr,
            &buf[..1 + data.len()],
        ).await {
            Ok(_) => {
                debug!("write to register [{:?}]: {:#010b}", reg_cp, data);
                Ok(())
            }
            Err(e) => {
                let err: SensorError = e.into();
                error!("Could not write register [{:?}]: {:?}", reg_cp, err);
                Err(err)
            }
        }
    }

    /// Read from a single register
    async fn read(&mut self, reg_addr: Registers) -> Result<u8, SensorError> {
        let mut read_buf = [0];
        let reg_cp = reg_addr.clone();
        match self
            .i2c
            .write_read(self.addr, &[reg_addr as u8], &mut read_buf).await
        {
            Ok(_) => {
                debug!("read from register [{:?}]: {:#010b}", reg_cp, read_buf);
                Ok(read_buf[0])
            }
            Err(e) => {
                let err: SensorError = e.into();
                error!("Could not read register [{:?}]: {:?}", reg_cp, err);
                Err(err)
            }
        }
    }

    /// Read from arbitrary amount of registers in a single transaction based on the given buffer size.
    async fn read_bytes(&mut self, start_reg: Registers, buf: &mut [u8]) -> Result<(), SensorError> {
        let reg_cp = start_reg.clone();
        match self.i2c.write_read(self.addr, &[start_reg as u8], buf).await {
            Ok(_) => {
                debug!("read from register [{:?}]: {:#010b}", reg_cp, buf);
                Ok(())
            }
            Err(e) => {
                let err: SensorError = e.into();
                error!("Could not read register [{:?}]: {:?}", reg_cp, err);
                Err(err)
            }
        }
    }

    /// Reads the gas-heater calibration/trim data from the sensor.
    /// Must be called once (e.g. from an `init()` step) before computing
    /// any res_heat_x value.
    /// Must pass in raw I2C bus and address since it is called before constructor is fully initialised.
    async fn read_calibration_raw(i2c: &mut I2C, addr: u8) -> Result<CalibrationData, SensorError> {
        // res_heat_val (0x00) and res_heat_range (bits 5:4 of 0x02)
        let mut buf0 = [0u8; 4];
        i2c.write_read(addr, &[0x00], &mut buf0).await?;
        let res_heat_val = buf0[0] as i8;
        let res_heat_range = (buf0[2] & 0x30) >> 4;
        // dividing by 16 allows for sign extension
        let range_switching_error = ((buf0[3] & 0xF0) as i8) / 16;

        // Coefficient block 1: registers 0x89..=0xA1 (25 bytes)
        let mut c1 = [0u8; 25];
        i2c.write_read(addr, &[0x89], &mut c1).await?;

        // Coefficient block 2: registers 0xE1..=0xF0 (16 bytes)
        let mut c2 = [0u8; 16];
        i2c.write_read(addr, &[0xE1], &mut c2).await?;

        // --- Temperature ---
        let par_t1 = i16::from_le_bytes([c2[8], c2[9]]); // 0xE9, 0xEA
        let par_t2 = i16::from_le_bytes([c1[1], c1[2]]); // 0x8A, 0x8B
        let par_t3 = c1[3] as i8; // 0x8C

        // --- Pressure ---
        let par_p1 = u16::from_le_bytes([c1[5], c1[6]]); // 0x8E, 0x8F
        let par_p2 = i16::from_le_bytes([c1[7], c1[8]]); // 0x90, 0x91
        let par_p3 = c1[9] as i8; // 0x92
        let par_p4 = i16::from_le_bytes([c1[11], c1[12]]); // 0x94, 0x95
        let par_p5 = i16::from_le_bytes([c1[13], c1[14]]); // 0x96, 0x97
        let par_p7 = c1[15] as i8; // 0x99
        let par_p6 = c1[16] as i8; // 0x98
        let par_p8 = i16::from_le_bytes([c1[19], c1[20]]); // 0x9C, 0x9D
        let par_p9 = i16::from_le_bytes([c1[21], c1[22]]); // 0x9E, 0x9F
        let par_p10 = c1[23] as i8; // 0xA0

        // --- Humidity (par_h1/par_h2 are 12-bit values sharing one byte, nibble-packed) ---
        let par_h1 = (((c2[2] as i16) << 4) | ((c2[1] as i16) & 0x0F)) as u16; // 0xE3 MSB, 0xE2 lo-nibble
        let par_h2 = (((c2[0] as i16) << 4) | ((c2[1] as i16) >> 4)) as i16; // 0xE1 MSB, 0xE2 hi-nibble
        let par_h3 = c2[3] as i8; // 0xE4
        let par_h4 = c2[4] as i8; // 0xE5
        let par_h5 = c2[5] as i8; // 0xE6
        let par_h6 = c2[6] as u8; // 0xE7
        let par_h7 = c2[7] as i8; // 0xE8

        // --- Gas heater ---
        let par_g2 = i16::from_le_bytes([c2[10], c2[11]]); // 0xEB, 0xEC
        let par_g1 = c2[12] as i8; // 0xED
        let par_g3 = c2[13] as i8; // 0xEE

        debug!(
            "calibration: t1={:?} t2={:?} t3={:?} p1={:?} h1={:?} h2={:?} g1={:?} g2={:?} g3={:?}",
            par_t1, par_t2, par_t3, par_p1, par_h1, par_h2, par_g1, par_g2, par_g3
        );

        Ok(CalibrationData {
            par_t1,
            par_t2,
            par_t3,
            par_p1,
            par_p2,
            par_p3,
            par_p4,
            par_p5,
            par_p6,
            par_p7,
            par_p8,
            par_p9,
            par_p10,
            par_h1,
            par_h2,
            par_h3,
            par_h4,
            par_h5,
            par_h6,
            par_h7,
            par_g1,
            par_g2,
            par_g3,
            res_heat_range,
            res_heat_val,
            range_switching_error,
        })
    }

    /// Computes the res_heat_x register value using integer-only arithmetic,
    /// matching Bosch's official fixed-point reference implementation.
    async fn calc_res_heat(&mut self, target_temp_c: i32, amb_temp_c: i32) -> u8 {
        let var1 = ((amb_temp_c * self.calibration.par_g3 as i32) / 10) << 8;
        let var2 = (self.calibration.par_g1 as i32 + 784)
            * (((((self.calibration.par_g2 as i32 + 154009) * target_temp_c * 5) / 100) + 3276800)
                / 10);
        let var3 = var1 + (var2 >> 1);
        let var4 = var3 / (self.calibration.res_heat_range as i32 + 4);
        let var5 = (131 * self.calibration.res_heat_val as i32) + 65536;
        let res_heat_x100 = ((var4 / var5) - 250) * 34;
        let res_heat_x = ((res_heat_x100 + 50) / 100);
        res_heat_x as u8
    }

    /// Encodes a heating duration in milliseconds into the gas_wait_x register format:
    /// bits<5:0> = value (0-63), bits<7:6> = multiplier factor (00=x1, 01=x4, 10=x16, 11=x64).
    /// Max representable duration is 63 * 64 = 4032ms.
    async fn encode_gas_wait(duration_ms: u16) -> u8 {
        const MAX_DURATION_MS: u16 = 0xFC0; // 4032ms

        if duration_ms >= MAX_DURATION_MS {
            0xFF // saturate at the maximum representable duration
        } else {
            let mut dur = duration_ms;
            let mut factor: u8 = 0;

            while dur > 0x3F {
                dur /= 4;
                factor += 1;
            }

            dur as u8 + (factor * 64)
        }
    }

    /// Initial configuration based on BME860 quick start guide.
    /// Needs to be called after `new`.
    pub async fn init_config(&mut self) -> Result<(), SensorError> {
        // Set humidity oversampling
        let ctrl_hum = self.read(Registers::CtrlHum).await?;
        self.write(Registers::CtrlHum, &[(ctrl_hum & !0b111) | 0b001]).await?;

        // Set temperature oversampling (osrs_t=2), pressure oversampling (osrs_p=5), mode=sleep
        self.write(Registers::CtrlMeas, &[0b0101_0100]).await?;

        let mut read_buf = [0; 3];
        self.read_bytes(Registers::CtrlHum, &mut read_buf).await?;
        trace!("oversampling registers: {:#010b}", read_buf);

        // Set hot plate temperature set-point to X and heating duration to 100ms.
        let duration = Self::encode_gas_wait(150).await;
        // 320C seems to be typical Bosch application target temperature, give it 25 degrees for default ambient temperature for set up.
        let res_heat = self.calc_res_heat(320, 25).await;
        self.write(Registers::GasWaitX, &[duration]).await?;
        self.write(Registers::ResHeatX, &[res_heat]).await?;
        // Configure sensor to use set-point 0 and enable gas measurement. (Writes to nv_conv<3:0> and run_gas_l)
        let ctrl_gas_1 = self.read(Registers::CtrlGas1).await?;
        self.write(Registers::CtrlGas1, &[(ctrl_gas_1 & !0b1_1111) | 0b1_0000]).await?;

        Ok(())
    }

    /// Set operation mode
    pub async fn set_mode(&mut self, mode: GSMode) -> Result<(), SensorError> {
        let ctrl_meas = self.read(Registers::CtrlMeas).await?;
        let new_value = match mode {
            GSMode::Forced => (ctrl_meas & !0b11) | 0b01,
            GSMode::Sleep => ctrl_meas & !0b11,
        };
        match self.write(Registers::CtrlMeas, &[new_value]).await {
            Ok(_) => {
                match mode {
                    GSMode::Forced => debug!("Mode set to Forced"),
                    GSMode::Sleep => debug!("Mode set to Sleep"),
                }
                Ok(())
            }
            Err(e) => {
                error!("Could not set gas sensor mode: {:?}", e);
                Err(e)
            }
        }
    }

    /// Computes gas resistance (in Ohms) from raw ADC gas reading and gas_range.
    async fn calc_gas_resistance(&self, gas_adc: u16, gas_range: u8) -> i32 {
        let range_switching_error = self.calibration.range_switching_error as i64;
        let gas_range = gas_range as usize;

        let var1: i64 = ((1340 + (5 * range_switching_error))
            * (LOOKUP_TABLE_1[gas_range] as i64))
            >> 16;

        let var2: i64 = ((gas_adc as i64) << 15) - (1i64 << 24) + var1;

        let gas_res: i32 = ((((LOOKUP_TABLE_2[gas_range] as i64 * var1) >> 9) + (var2 >> 1)) / var2) as i32;

        gas_res
    }

    /// Computes compensated gas sensor resitance output data in Ohms.
    // Seems like IAQ is only provided through the BSEC software
    async fn get_gas_resistance(&mut self) -> Result<i32, SensorError> {
        // Covers MSB (0x2A) and LSB (0x2B) registers for gas quality
        let mut read_buf = [0; 2];
        self.read_bytes(Registers::GasRMsb, &mut read_buf).await?;
        let gas_adc: u16 = ((read_buf[0] as u16) << 2)
            | ((read_buf[1] & 0b1100_0000) as u16 >> 6);
        let gas_range = read_buf[1] & 0b0000_1111;
        let final_res = self.calc_gas_resistance(gas_adc, gas_range).await;
        info!("Gas resistance read success: {} Ohms", final_res);
        Ok(final_res)
    }

    async fn calc_temp(&mut self, temp_adc: u32) -> [i32; 2] {
        let var1 = (temp_adc as i32 >> 3) - ((self.calibration.par_t1 as i32) << 1);
        let var2 = (var1 * self.calibration.par_t2 as i32) >> 11;
        let var3 =
            ((((var1 >> 1) * (var1 >> 1)) >> 12) * ((self.calibration.par_t3 as i32) << 4)) >> 14;
        let t_fine = var2 + var3;
        let temp_comp = ((t_fine * 5) + 128) >> 8;
        [temp_comp, t_fine]
    }

    async fn get_temp(&mut self) -> Result<[i32; 2], SensorError> {
        // Covers MSB (0x22), LSB (0x23), XLSB (0x24) registers for temperature
        let mut read_buf = [0; 3];
        self.read_bytes(Registers::TempMsb, &mut read_buf).await?;
        let temp_adc: u32 = ((read_buf[0] as u32) << 12)
            | ((read_buf[1] as u32) << 4)
            | ((read_buf[2] as u32) >> 4);
        let final_temp = self.calc_temp(temp_adc).await;
        info!("Temperature read success: {} centidegrees Celsius", final_temp[0]);
        Ok(final_temp)
    }

    /// Computes compensated humidity (in milli%) from raw ADC humidity and temp_comp
    /// (the calculated temperature compensation value).
    async fn calc_humidity(&self, hum_adc: u16, temp_comp: i32) -> u32 {
        let par_h1 = self.calibration.par_h1 as i32;
        let par_h2 = self.calibration.par_h2 as i32;
        let par_h3 = self.calibration.par_h3 as i32;
        let par_h4 = self.calibration.par_h4 as i32;
        let par_h5 = self.calibration.par_h5 as i32;
        let par_h6 = self.calibration.par_h6 as i32;
        let par_h7 = self.calibration.par_h7 as i32;

        let var1 = (hum_adc as i32) - (par_h1 << 4) - (((temp_comp * par_h3) / 100) >> 1);

        let var2 = (par_h2
            * (((temp_comp * par_h4) / 100)
                + (((temp_comp * ((temp_comp * par_h5) / 100)) >> 6) / 100)
                + (1 << 14)))
            >> 10;

        let var3 = var1 * var2;

        let var4 = ((par_h6 << 7) + ((temp_comp * par_h7) / 100)) >> 4;

        let var5 = ((var3 >> 14) * (var3 >> 14)) >> 10;

        let var6 = (var4 * var5) >> 1;

        let hum_comp = (((var3 + var6) >> 10) * 1000) >> 12;

        hum_comp as u32
    }

    /// Returns compensated humidity in milli%, still need to divide data by 1000.
    async fn get_humidity(&mut self, temp_comp: i32) -> Result<u32, SensorError> {
        // Covers MSB (0x25) and LSB (0x26) registers for humidity
        let mut read_buf = [0; 2];
        self.read_bytes(Registers::HumMsb, &mut read_buf).await?;
        let hum_adc: u16 = ((read_buf[0] as u16) << 8)
            | (read_buf[1] as u16);
        let final_hum = self.calc_humidity(hum_adc, temp_comp).await;
        info!("Humidity read success: {} milli%", final_hum);
        Ok(final_hum)
    }

    /// Computes compensated pressure (in Pa) from raw ADC pressure and t_fine
    /// (the intermediate value produced during temperature compensation).
    // From AI: Rust panics on integer overflow in debug builds (unlike C, which silently wraps). This formula is designed by Bosch to stay within i32 range for realistic sensor inputs, so it should be fine in practice — but if you ever do hit a panic here during testing, it's a legitimate signal that an upstream value (bad calibration read, corrupted ADC data) is out of the expected range, not something to just wrap around and ignore.
    async fn calc_pressure(&self, press_adc: u32, t_fine: i32) -> u32 {
        let par_p1 = self.calibration.par_p1 as i32;
        let par_p2 = self.calibration.par_p2 as i32;
        let par_p3 = self.calibration.par_p3 as i32;
        let par_p4 = self.calibration.par_p4 as i32;
        let par_p5 = self.calibration.par_p5 as i32;
        let par_p6 = self.calibration.par_p6 as i32;
        let par_p7 = self.calibration.par_p7 as i32;
        let par_p8 = self.calibration.par_p8 as i32;
        let par_p9 = self.calibration.par_p9 as i32;
        let par_p10 = self.calibration.par_p10 as i32;

        let mut var1: i32 = (t_fine >> 1) - 64000;

        let mut var2: i32 = ((((var1 >> 2) * (var1 >> 2)) >> 11) * par_p6) >> 2;
        var2 += (var1 * par_p5) << 1;
        var2 = (var2 >> 2) + (par_p4 << 16);

        var1 =
            (((((var1 >> 2) * (var1 >> 2)) >> 13) * (par_p3 << 5)) >> 3) + ((par_p2 * var1) >> 1);
        var1 >>= 18;
        var1 = ((32768 + var1) * par_p1) >> 15;

        let mut press_comp: u32 = (1_048_576 - press_adc as i32 - (var2 >> 12)) as u32 * 3125;

        press_comp = if press_comp >= (1 << 30) {
            (press_comp / var1 as u32) << 1
        } else {
            (press_comp << 1) / var1 as u32
        };

        let var1_final = (par_p9 * ((((press_comp >> 3) * (press_comp >> 3)) >> 13) as i32)) >> 12;
        let var2_final = ((press_comp >> 2) as i32 * par_p8) >> 13;
        let var3_final = ((press_comp >> 8) as i32
            * (press_comp >> 8) as i32
            * (press_comp >> 8) as i32
            * par_p10)
            >> 17;

        press_comp = ((press_comp as i32)
            + ((var1_final + var2_final + var3_final + (par_p7 << 7)) >> 4))
            as u32;

        press_comp
    }

    async fn get_pressure(&mut self, t_fine: i32) -> Result<u32, SensorError> {
        // Covers MSB (0x1F), LSB (0x20) and XLSB (0x21) registers for humidity
        let mut read_buf = [0; 3];
        self.read_bytes(Registers::PressMsb, &mut read_buf).await?;
        let press_adc: u32 = ((read_buf[0] as u32) << 12)
            | ((read_buf[1] as u32) << 4)
            | ((read_buf[2] as u32) >> 4);
        let final_press = self.calc_pressure(press_adc, t_fine).await;
        info!("Pressure read success: {} Pa", final_press);
        Ok(final_press)
    }

    /// Check if measured data are ready to be read.
    async fn check_if_ready(&mut self) -> Result<(), SensorError> {
        loop {
            let meas_status = self.read(Registers::MeasStatus0).await?;
            if (meas_status & 0b1000_0000) != 0 {
                return Ok(());
            }
            Timer::after_millis(100).await;
        }
    }

    pub async fn get_measurements(&mut self) -> Result<(f64, f64, f64, u8), SensorError> {
        info!("Starting GAS SENSOR measurement...");
        self.set_mode(GSMode::Forced).await?;
        self.check_if_ready().await?;

        let temp = self.get_temp().await?;
        // convert from centidegrees Celsius to Celsius
        let temperature = temp[0] as f64 / 100.0;
        // convert from milli% to %
        let humidity = self.get_humidity(temp[0]).await? as f64 / 1000.0;
        // convert from Pa to hPa
        let pressure = self.get_pressure(temp[1]).await? as f64 / 100.0;
        let gas_res = self.get_gas_resistance().await?;

        info!("GAS SENSOR measurements done!");

        let mut air_quality = 0;
        if gas_res >= 35000 {
            air_quality = 0;    // clean
        } else if (5000..35000).contains(&gas_res) {
            air_quality = 1;    // polluted
        } else if gas_res < 5000 {
            air_quality = 2;    // heavily polluted
        }

        info!("Temperature: {}, Humidity: {}, Pressure: {}, Air Quality: {}", temperature, humidity, pressure, gas_res);

        debug!("GAS SENSOR going to sleep...");
        self.set_mode(GSMode::Sleep).await?;

        Ok((temperature, humidity, pressure, air_quality))
    }
}
