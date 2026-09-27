//! Driver for the MIKROE-4416 Watchdog Click board (TI TPS3430 window watchdog timer),
//! wired with SET0 permanently tied to 3V3 and CWD left floating (NC).
//!
//! With SET0 hardwired high, only two window states are reachable via SET1
//! (per the TPS3430 datasheet's Table 6):
//!   - SET0 = 1, SET1 = 0 -> watchdog disabled
//!   - SET0 = 1, SET1 = 1 -> 1:2 lower:upper boundary ratio
//! The 1:8 and 3:4 ratio modes require SET1 = 0 (with a different SET0) or a
//! mix this wiring can't produce, and are unreachable here. This scheme is
//! deliberately chosen over hardwiring SET1 instead: it keeps a real
//! software-selectable Disabled state, and 1:2 gives a far more forgiving
//! window (tWDL/tWDU roughly 800 ms / 1.6 s typ. with CWD=NC) than 1:8's
//! tens-of-milliseconds window would.

use defmt::{Format, debug};
use embassy_time::Timer;
use embedded_hal::digital::{Error, ErrorKind, OutputPin};

/// Watchdog Timer driver for the MIKROE-4416 Watchdog Click (TPS3430),
/// with SET0 hardwired to 3V3.
pub struct WatchdogTimer<WDI, S1> {
    /// GPIO output connected to the WDI ("pet"/feed) pin.
    wdi: WDI,
    /// GPIO output connected to the SET1 window-select pin.
    s1: S1,
}

/// Watchdog window modes reachable with SET0 hardwired to 3V3.
/// Per the TPS3430 datasheet's Table 6.
#[derive(Format, Clone, Copy, PartialEq)]
pub enum WatchdogWindow {
    /// SET1 = 0 (SET0 = 1) — watchdog disabled; WDO never asserts and WDI is ignored.
    Disabled,
    /// SET1 = 1 (SET0 = 1) — 1:2 lower:upper boundary ratio.
    Ratio1To2,
}

/// Watchdog Timer driver errors.
#[derive(Format, Debug)]
pub enum WatchdogError {
    GpioError,
    //  GpioError(ErrorKind) simplified due to compile error, may revise
}

impl<E: Error> From<E> for WatchdogError {
    fn from(value: E) -> Self {
        // WatchdogError::GpioError(value.kind()) simplified due to compile error, may revise
        WatchdogError::GpioError
    }
}

impl<WDI, S1> WatchdogTimer<WDI, S1>
where
    WDI: OutputPin,
    S1: OutputPin,
{
    /// Initialise the watchdog timer driver.
    ///
    /// `wdi` and `s1` must already be configured as GPIO outputs. SET0 is
    /// assumed hardwired to 3V3 on the PCB and is not driven by this driver.
    /// WDI is driven low and SET1 defaults to the disabled state (SET1=0),
    /// so the watchdog doesn't start expecting pulses before your
    /// application calls `set_window`.
    pub fn new(mut wdi: WDI, mut s1: S1) -> Result<Self, WatchdogError> {
        wdi.set_low()?;
        s1.set_low()?; // SET1=0 (SET0=1 via hardware) => disabled
        debug!("Watchdog timer initialised (disabled)");
        Ok(Self { wdi, s1 })
    }

    /// Set the watchdog window by driving the SET1 pin.
    ///
    /// With SET0 hardwired to 3V3, only `Disabled` and `Ratio1To2` are
    /// reachable — the 1:8 and 3:4 ratio modes need a different SET0 value
    /// and are not available in this hardware configuration.
    pub fn set_window(&mut self, window: WatchdogWindow) -> Result<(), WatchdogError> {
        match window {
            WatchdogWindow::Disabled => self.s1.set_low()?,
            WatchdogWindow::Ratio1To2 => self.s1.set_high()?,
        }
        debug!("Watchdog window set to {:?}", window);
        Ok(())
    }

    /// Send a single pulse on the WDI pin to feed ("pet") the watchdog.
    ///
    /// Must land as a falling edge inside the configured watchdog window
    /// or the TPS3430 will assert WDO / trigger a reset.
    pub async fn send_pulse(&mut self) -> Result<(), WatchdogError> {
        self.wdi.set_high()?;
        Timer::after_micros(100).await;
        self.wdi.set_low()?;
        debug!("Watchdog pulse sent");
        Ok(())
    }
}
