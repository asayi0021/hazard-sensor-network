//! Driver for the DFRobot SEN0575 Gravity Tipping Bucket Rainfall Sensor (I2C mode).
//!
//! The sensor has no internal electronics of its own for measurement -- a magnetic
//! reed switch closes every time the tipping bucket flips, and an onboard companion
//! MCU on the sensor's PCB debounces/counts these tips and exposes the result over
//! I2C as a set of input registers. Cumulative rainfall is reported by the sensor
//! itself as a raw count multiplied internally by a per-tip rainfall depth
//! (`BaseRainfall`, in mm), so no conversion math is needed on the host side --
//! only a fixed-point scale-down of the register value.

use defmt::{Format, debug, error, trace};
use embedded_hal_async::i2c::{Error, ErrorKind, SevenBitAddress};

/// Default rainfall depth per bucket tip, in millimetres, as used by DFRobot's
/// reference driver. Written to the `BaseRainfall` register (scaled by 10000,
/// see [`Registers::BaseRainfall`]) during configuration.
const DEFAULT_MM_PER_TIP: f32 = 0.2794;

/// Expected VID reported by a genuine SEN0575, used as a sanity check in
/// `init_config`. Note that on the wire VID and PID are bit-packed together
/// in the 4th byte of the PID/VID block -- see `read_pid_vid` for the split.
const EXPECTED_VID: u16 = 0x3343;

/// Expected PID reported by a genuine SEN0575, used alongside [`EXPECTED_VID`].
const EXPECTED_PID: u32 = 0x100C0;

/// Default I2C address of the SEN0575
pub const RAINFALL_SENSOR_ADDR: u8 = 0x1D;

/// DFRobot SEN0575 Tipping Bucket Rainfall Sensor.
pub struct RainfallSensor<I2C> {
    /// I2C bus from NRF52840
    i2c: I2C,
    /// I2C address of sensor
    addr: SevenBitAddress,
    /// Raw `CumulativeRainfall` count captured at the last call to [`Self::reset`],
    /// used to report rainfall since that point rather than since the sensor's
    /// own power-on. The sensor has no hardware reset/zero command of its own --
    /// its cumulative counter only ever increases -- so this baseline is a
    /// software-side stand-in for one.
    baseline: u32,
}

/// List of read/writable registers (and their address) on the rainfall sensor.
/// For more details, see the [DFRobot_RainfallSensor reference driver][registers].
///
/// [registers]: https://github.com/DFRobot/DFRobot_RainfallSensor
#[derive(Format, Clone)]
pub enum Registers {
    /// 4 bytes -- PID and VID, bit-packed together. Bytes 0-1 are the low 16
    /// bits of PID; byte 2 is the low 8 bits of VID; byte 3 packs the top 2
    /// bits of PID (bits 6-7) with the top 6 bits of VID (bits 0-5). See
    /// `read_pid_vid` for the split.
    Pid = 0x00,
    /// 2 bytes -- firmware version.
    Version = 0x0A,
    /// 4 bytes -- accumulated rainfall (scaled, see [`Self::CumulativeRainfall`])
    /// over the last N hours, where N is set via [`Self::RainHour`].
    TimeRainfall = 0x0C,
    /// 4 bytes -- cumulative rainfall since the sensor was last reset, as an
    /// unsigned little-endian count scaled by 1/10000 to give millimetres.
    CumulativeRainfall = 0x10,
    /// 4 bytes -- raw tipping-bucket count, unscaled.
    RawData = 0x14,
    /// 1 byte -- window (in hours, 1-24) used by [`Self::TimeRainfall`].
    RainHour = 0x26,
    /// 2 bytes -- rainfall depth per bucket tip, as an unsigned little-endian
    /// integer scaled by 10000 (e.g. 0.2794 mm is stored as `2794`) -- the
    /// same scale used by [`Self::CumulativeRainfall`].
    BaseRainfall = 0x28,
}

/// Rainfall Sensor errors.
#[derive(Format, Debug)]
pub enum SensorError {
    GetDataError,
    I2cError(ErrorKind),
    InvalidParameter,
}

impl<E: Error> From<E> for SensorError {
    fn from(value: E) -> Self {
        SensorError::I2cError(value.kind())
    }
}

impl<I2C: embedded_hal_async::i2c::I2c> RainfallSensor<I2C> {
    /// Initialise a new rainfall sensor.
    pub async fn new(i2c: I2C, addr: u8) -> Result<Self, SensorError> {
        Ok(Self { i2c, addr, baseline: 0 })
    }

    /// Write to one register.
    async fn write(&mut self, reg_addr: Registers, data: &[u8]) -> Result<(), SensorError> {
        let reg_cp = reg_addr.clone();

        // Build one contiguous buffer: [register address, data...].
        // Largest write we currently do is the 2-byte scaled BaseRainfall value.
        let mut buf = [0u8; 5];
        buf[0] = reg_addr as u8;
        buf[1..1 + data.len()].copy_from_slice(data);

        match self.i2c.write(self.addr, &buf[..1 + data.len()]).await {
            Ok(_) => {
                trace!("write to register [{:?}]: {:#010b}", reg_cp, data);
                Ok(())
            }
            Err(e) => {
                let err: SensorError = e.into();
                error!("Could not write register [{:?}]: {:?}", reg_cp, err);
                Err(err)
            }
        }
    }

    /// Read from an arbitrary number of registers in a single transaction, based
    /// on the given buffer size.
    async fn read(&mut self, reg_addr: Registers, buf: &mut [u8]) -> Result<(), SensorError> {
        let reg_cp = reg_addr.clone();

        match self.i2c.write_read(self.addr, &[reg_addr as u8], buf).await {
            Ok(_) => {
                trace!("read from register [{:?}]: {:#010b}", reg_cp, buf);
                Ok(())
            }
            Err(e) => {
                let err: SensorError = e.into();
                error!("Could not read register [{:?}]: {:?}", reg_cp, err);
                Err(err)
            }
        }
    }

    /// Read and unpack the PID/VID block.
    ///
    /// PID and VID are not two clean, separate registers -- they're bit-packed
    /// into a single 4-byte block. Bytes 0-1 hold the low 16 bits of PID; byte
    /// 2 holds the low 8 bits of VID; byte 3 splits its 8 bits between the two,
    /// with the top 2 bits (mask `0xC0`) extending PID and the bottom 6 bits
    /// (mask `0x3F`) extending VID. This split matches DFRobot's reference
    /// driver (`DFRobot_RainfallSensor::getPidVid`).
    async fn read_pid_vid(&mut self) -> Result<(u32, u16), SensorError> {
        let mut buf = [0u8; 4];
        self.read(Registers::Pid, &mut buf).await?;

        let pid = (buf[0] as u32) | ((buf[1] as u32) << 8) | (((buf[3] & 0xC0) as u32) << 10);
        let vid = (buf[2] as u16) | (((buf[3] & 0x3F) as u16) << 8);

        Ok((pid, vid))
    }

    /// Initial configuration of the rainfall sensor.
    ///
    /// Confirms the device on the bus is a genuine SEN0575 by checking its PID
    /// and VID, then (re)writes the default rainfall-per-tip calibration value
    /// so the sensor's own cumulative-rainfall scaling is in a known state.
    pub async fn init_config(&mut self) -> Result<(), SensorError> {
        debug!("Configuring RAINFALL SENSOR...");

        let (pid, vid) = self.read_pid_vid().await?;
        if vid != EXPECTED_VID || pid != EXPECTED_PID {
            error!("Unexpected RAINFALL SENSOR PID/VID: {:#07x}/{:#06x}", pid, vid);
            return Err(SensorError::InvalidParameter);
        }

        let scaled_mm_per_tip = (DEFAULT_MM_PER_TIP * 10000.0) as u16;
        self.write(Registers::BaseRainfall, &scaled_mm_per_tip.to_le_bytes())
            .await?;

        self.reset().await?;

        debug!("RAINFALL SENSOR configuration success.");
        Ok(())
    }

    /// Read the raw `CumulativeRainfall` register.
    async fn read_cumulative_raw(&mut self) -> Result<u32, SensorError> {
        let mut raw = [0u8; 4];
        self.read(Registers::CumulativeRainfall, &mut raw).await?;
        Ok(u32::from_le_bytes(raw))
    }

    /// "Reset" the sensor for a new reading.
    ///
    /// The SEN0575 has no hardware command to zero its cumulative rainfall
    /// counter -- it only ever counts up from power-on. This instead captures
    /// the current raw count as a baseline, so that subsequent calls to
    /// [`Self::get_rainfall`] report rainfall accumulated *since this call*
    /// rather than the sensor's lifetime total.
    pub async fn reset(&mut self) -> Result<(), SensorError> {
        self.baseline = self.read_cumulative_raw().await?;
        debug!("RAINFALL SENSOR reset, new baseline: {} counts", self.baseline);
        Ok(())
    }

    /// Get the cumulative rainfall measurement, in millimetres, since the last
    /// call to [`Self::reset`] (or since power-on, if `reset` has not been
    /// called yet).
    pub async fn get_rainfall(&mut self) -> Result<f64, SensorError> {
        let raw_value = self.read_cumulative_raw().await?;

        // Register holds an unsigned little-endian count, internally scaled by
        // the sensor's BaseRainfall calibration and reported as mm * 10000.
        //
        // wrapping_sub (not saturating_sub) is deliberate: the sensor's u32
        // counter only ever counts up and, given enough continuous uptime,
        // will eventually wrap past u32::MAX back to 0. Because unsigned
        // subtraction wraps modulo 2^32, raw_value.wrapping_sub(baseline)
        // still yields the correct delta across exactly one such wrap --
        // saturating_sub would instead clamp to 0 and silently hide rainfall
        // for a stretch after the wrap.
        let rainfall_mm = raw_value.wrapping_sub(self.baseline) as f64 / 10000.0;

        debug!("Rainfall read success: {} mm", rainfall_mm);
        Ok(rainfall_mm)
    }
}
