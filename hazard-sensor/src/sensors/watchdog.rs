//! Driver for the MIKROE-4416 Watchdog Click board (TI TPS3430 window watchdog timer),
//! wired with SET1 permanently tied to GND.
//!
//! With SET1 hardwired low, only two window states are reachable via SET0
//! (per the TPS3430 datasheet's Table 6):
//!   - SET0 = 1, SET1 = 0 -> watchdog disabled
//!   - SET0 = 0, SET1 = 0 -> 1:8 lower:upper boundary ratio
//! The 3:4 and 1:2 ratio modes require SET1 = 1 and are unreachable here.

use defmt::{Format, debug};
use embassy_time::Timer;
use embedded_hal::digital::{Error, ErrorKind, OutputPin};

/// Watchdog Timer driver for the MIKROE-4416 Watchdog Click (TPS3430),
/// with SET1 hardwired to GND.
pub struct WatchdogTimer<WDI, S0> {
    /// GPIO output connected to the WDI ("pet"/feed) pin.
    wdi: WDI,
    /// GPIO output connected to the SET0 window-select pin.
    s0: S0,
}

/// Watchdog window modes reachable with SET1 hardwired to GND.
/// Per the TPS3430 datasheet's Table 6.
#[derive(Format, Clone, Copy, PartialEq)]
pub enum WatchdogWindow {
    /// SET0 = 1 (SET1 = 0) — watchdog disabled; WDO never asserts and WDI is ignored.
    Disabled,
    /// SET0 = 0 (SET1 = 0) — 1:8 lower:upper boundary ratio.
    Ratio1To8,
}

/// Watchdog Timer driver errors.
#[derive(Format, Debug)]
pub enum SensorError {
    GpioError(ErrorKind),
}

impl<E: Error> From<E> for SensorError {
    fn from(value: E) -> Self {
        SensorError::GpioError(value.kind())
    }
}

impl<WDI, S0> WatchdogTimer<WDI, S0>
where
    WDI: OutputPin,
    S0: OutputPin,
{
    /// Initialise the watchdog timer driver.
    ///
    /// `wdi` and `s0` must already be configured as GPIO outputs. SET1 is
    /// assumed hardwired to GND on the PCB and is not driven by this driver.
    /// WDI is driven low and SET0 defaults to the disabled state (SET0=1),
    /// so the watchdog doesn't start expecting pulses before your
    /// application calls `set_window`.
    pub fn new(mut wdi: WDI, mut s0: S0) -> Result<Self, SensorError> {
        wdi.set_low()?;
        s0.set_high()?; // SET0=1 (SET1=0 via hardware) => disabled
        debug!("Watchdog timer initialised (disabled)");
        Ok(Self { wdi, s0 })
    }

    /// Set the watchdog window by driving the SET0 pin.
    ///
    /// With SET1 hardwired to GND, only `Disabled` and `Ratio1To8` are
    /// reachable — the 3:4 and 1:2 ratio modes require SET1 = 1 and are
    /// not available in this hardware configuration.
    pub fn set_window(&mut self, window: WatchdogWindow) -> Result<(), SensorError> {
        match window {
            WatchdogWindow::Disabled => self.s0.set_high()?,
            WatchdogWindow::Ratio1To8 => self.s0.set_low()?,
        }
        debug!("Watchdog window set to {:?}", window);
        Ok(())
    }

    /// Send a single pulse on the WDI pin to feed ("pet") the watchdog.
    ///
    /// Must land as a falling edge inside the configured watchdog window
    /// or the TPS3430 will assert WDO / trigger a reset.
    pub async fn send_pulse(&mut self) -> Result<(), SensorError> {
        self.wdi.set_high()?;
        Timer::after_micros(100).await;
        self.wdi.set_low()?;
        debug!("Watchdog pulse sent");
        Ok(())
    }
}
