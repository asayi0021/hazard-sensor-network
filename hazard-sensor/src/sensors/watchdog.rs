//! Driver for the MIKROE-4416 Watchdog Click board (TI TPS3430 window watchdog timer).
//!
//! Unlike the other sensors in this project, this board has no I2C/SPI bus —
//! it is entirely GPIO-driven. `WDI` must receive a falling edge within the
//! configured watchdog window to prevent a reset. The window *ratio*
//! (lower boundary : upper boundary) is selected via the `SET0`/`SET1` pins
//! (labelled S0/S1 on the Click board). The absolute time base of the
//! window is additionally set by the board's CWD pin/jumper — a
//! hardware-only configuration not controllable from the MCU.
//!
//! Reference: TPS3430 datasheet (Texas Instruments), Table 6 (SET0/SET1 modes).

use defmt::{Format, debug};
use embassy_time::Timer;
use embedded_hal::digital::{Error, ErrorKind, OutputPin};

/// Watchdog Timer driver for the MIKROE-4416 Watchdog Click (TPS3430).
pub struct WatchdogTimer<WDI, S0, S1> {
    /// GPIO output connected to the WDI ("pet"/feed) pin.
    wdi: WDI,
    /// GPIO output connected to the SET0 window-ratio-select pin.
    s0: S0,
    // /// GPIO output connected to the SET1 window-ratio-select pin.
    // s1: S1,
}

/// Watchdog window ratio modes, selected via the SET0/SET1 pins.
/// Per the TPS3430 datasheet's Table 6.
#[derive(Format, Clone, Copy, PartialEq)]
pub enum WatchdogWindow {
    /// SET0 = 1, SET1 = 0 — watchdog disabled; WDO never asserts and WDI is ignored.
    Disabled,
    /// SET0 = 0, SET1 = 0 — 1:8 lower:upper boundary ratio.
    Ratio1To8,
    // /// SET0 = 0, SET1 = 1 — 3:4 lower:upper boundary ratio.
    // Ratio3To4,
    // /// SET0 = 1, SET1 = 1 — 1:2 lower:upper boundary ratio.
    // Ratio1To2,
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
    // S1: OutputPin,
{
    /// Initialise the watchdog timer driver.
    ///
    /// `wdi`, `s0`, and `s1` must already be configured as GPIO outputs.
    /// WDI is driven low and SET0/SET1 are set to the disabled state
    /// (SET0=1, SET1=0), so the watchdog doesn't start expecting pulses
    /// before your application calls `set_window`.
    pub fn new(mut wdi: WDI, mut s0: S0) -> Result<Self, SensorError> {
        wdi.set_low()?;
        s0.set_high()?; // SET0=1, SET1=0 => disabled, per datasheet Table 6
        // s1.set_low()?;
        debug!("Watchdog timer initialised (disabled)");
        Ok(Self { wdi, s0 })
    }

    /// Set the watchdog window ratio via the SET0/SET1 pins.
    ///
    /// Per the datasheet, SET0 and SET1 "cannot be changed at the same
    /// time" while the device is operational — a 500us (tSET) settling
    /// delay is required between switching the two pins. This function
    /// enforces that ordering and delay automatically.
    ///
    /// Note: this only selects the lower:upper boundary *ratio*. The
    /// absolute window duration is set separately by the board's CWD
    /// pin/jumper (hardware-only) — consult the TPS3430 datasheet's
    /// timing table for the resulting duration for your CWD configuration.
    pub async fn set_window(&mut self, window: WatchdogWindow) -> Result<(), SensorError> {
        let (s0_level, s1_level) = match window {
            WatchdogWindow::Disabled => (true, false),
            WatchdogWindow::Ratio1To8 => (false, false),
            // WatchdogWindow::Ratio3To4 => (false, true),
            // WatchdogWindow::Ratio1To2 => (true, true),
        };

        if s0_level {
            self.s0.set_high()?;
        } else {
            self.s0.set_low()?;
        }

        Timer::after_micros(500).await; // required tSET settling time

        if s1_level {
            self.s1.set_high()?;
        } else {
            self.s1.set_low()?;
        }

        debug!("Watchdog window set to {:?}", window);
        Ok(())
    }

    /// Send a single pulse on the WDI pin to feed ("pet") the watchdog.
    ///
    /// Must land as a falling edge inside the configured watchdog window
    /// (see `set_window` and the board's CWD configuration) or the
    /// TPS3430 will assert WDO / trigger a reset.
    pub async fn send_pulse(&mut self) -> Result<(), SensorError> {
        self.wdi.set_high()?;
        // Brief high period before the falling edge the TPS3430 looks for —
        // check the datasheet's Figure 7-5 timing diagram if this needs
        // tightening for your specific configuration.
        Timer::after_micros(100).await;
        self.wdi.set_low()?;
        debug!("Watchdog pulse sent");
        Ok(())
    }
}
