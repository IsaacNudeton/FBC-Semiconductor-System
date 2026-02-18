//! VICOR Core Supply Controller
//!
//! Controls 6 VICOR PRM+VTM power modules via DAC (voltage) + GPIO (enable)
//!
//! Core mapping (INVARIANT from schematic):
//! | Core | DAC Ch | MIO Pin |
//! |------|--------|---------|
//! | 1    | 9      | 0       |
//! | 2    | 3      | 39      |
//! | 3    | 7      | 47      |
//! | 4    | 8      | 8       |
//! | 5    | 4      | 38      |
//! | 6    | 2      | 37      |

use super::{Bu2505, Gpio, MioPin, SpiError, delay_us};

/// Number of VICOR core supplies
pub const NUM_CORES: usize = 6;

/// Core mapping: (DAC channel, MIO pin) - INVARIANT from schematic
const CORE_MAP: [(u8, u8); NUM_CORES] = [
    (9, 0),   // Core 1
    (3, 39),  // Core 2
    (7, 47),  // Core 3
    (8, 8),   // Core 4
    (4, 38),  // Core 5
    (2, 37),  // Core 6
];

/// Voltage limits (safety)
pub const CORE_VOLTAGE_MIN_MV: u16 = 500;   // 0.5V minimum
pub const CORE_VOLTAGE_MAX_MV: u16 = 1500;  // 1.5V maximum

/// DAC voltage multiplier (Sonoma uses voltage × 2)
/// This is the VICOR feedback divider ratio
const VOLTAGE_MULTIPLIER: u16 = 2;

/// VICOR controller error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VicorError {
    /// Invalid core number (must be 1-6)
    InvalidCore,
    /// Voltage out of range
    VoltageOutOfRange,
    /// SPI communication error
    Spi(SpiError),
}

impl From<SpiError> for VicorError {
    fn from(e: SpiError) -> Self {
        VicorError::Spi(e)
    }
}

/// VICOR core supply controller
pub struct VicorController<'a> {
    dac: &'a Bu2505<'a>,
    gpio: &'a Gpio,
    enabled: [bool; NUM_CORES],
    voltages_mv: [u16; NUM_CORES],
}

impl<'a> VicorController<'a> {
    /// Create new VICOR controller
    pub fn new(dac: &'a Bu2505<'a>, gpio: &'a Gpio) -> Self {
        Self {
            dac,
            gpio,
            enabled: [false; NUM_CORES],
            voltages_mv: [0; NUM_CORES],
        }
    }

    /// Initialize controller
    ///
    /// Disables all cores and sets voltages to 0
    pub fn init(&mut self) -> Result<(), VicorError> {
        // Disable all cores first (safety)
        for core in 1..=NUM_CORES {
            self.disable_core(core as u8)?;
        }

        // Set all DAC channels to 0
        for &(dac_ch, _) in CORE_MAP.iter() {
            self.dac.set_raw(dac_ch, 0)?;
        }

        Ok(())
    }

    /// Set core voltage
    ///
    /// # Arguments
    /// * `core` - Core number 1-6
    /// * `mv` - Voltage in millivolts (500-1500)
    ///
    /// Note: Does NOT enable the core. Call `enable_core()` separately.
    pub fn set_core_voltage(&mut self, core: u8, mv: u16) -> Result<(), VicorError> {
        let idx = self.validate_core(core)?;

        if mv < CORE_VOLTAGE_MIN_MV || mv > CORE_VOLTAGE_MAX_MV {
            return Err(VicorError::VoltageOutOfRange);
        }

        let (dac_ch, _) = CORE_MAP[idx];

        // VICOR feedback: DAC voltage = Core voltage × multiplier
        let dac_mv = mv * VOLTAGE_MULTIPLIER;

        self.dac.set_voltage_mv(dac_ch, dac_mv)?;
        self.voltages_mv[idx] = mv;

        Ok(())
    }

    /// Enable core output
    ///
    /// # Arguments
    /// * `core` - Core number 1-6
    pub fn enable_core(&mut self, core: u8) -> Result<(), VicorError> {
        let idx = self.validate_core(core)?;
        let (_, mio_num) = CORE_MAP[idx];
        let pin = MioPin::new(mio_num);

        self.gpio.set_output(pin);
        self.gpio.write_pin(pin, true);
        self.enabled[idx] = true;

        Ok(())
    }

    /// Disable core output
    ///
    /// # Arguments
    /// * `core` - Core number 1-6
    pub fn disable_core(&mut self, core: u8) -> Result<(), VicorError> {
        let idx = self.validate_core(core)?;
        let (_, mio_num) = CORE_MAP[idx];
        let pin = MioPin::new(mio_num);

        self.gpio.write_pin(pin, false);
        self.enabled[idx] = false;

        Ok(())
    }

    /// Disable all cores immediately (emergency stop)
    pub fn disable_all(&mut self) {
        for i in 0..NUM_CORES {
            let (_, mio_num) = CORE_MAP[i];
            let pin = MioPin::new(mio_num);
            self.gpio.write_pin(pin, false);
            self.enabled[i] = false;
        }
    }

    /// Power-on sequence with proper timing
    ///
    /// # Arguments
    /// * `voltages_mv` - Array of 6 target voltages in millivolts
    ///
    /// Sequence:
    /// 1. Disable all cores
    /// 2. Set all DAC voltages
    /// 3. Wait for DAC settling (10ms)
    /// 4. Enable cores 1→6 with 1ms delay each
    /// 5. Wait for power good (50ms)
    pub fn power_on_sequence(&mut self, voltages_mv: &[u16; NUM_CORES]) -> Result<(), VicorError> {
        // 1. Disable all cores (safety)
        self.disable_all();

        // 2. Set all voltages
        for (core, &mv) in voltages_mv.iter().enumerate() {
            self.set_core_voltage((core + 1) as u8, mv)?;
        }

        // 3. Wait for DAC settling
        delay_us(10_000); // 10ms

        // 4. Enable cores sequentially
        for core in 1..=NUM_CORES {
            self.enable_core(core as u8)?;
            delay_us(1_000); // 1ms between enables
        }

        // 5. Wait for power good
        delay_us(50_000); // 50ms

        Ok(())
    }

    /// Power-off sequence
    ///
    /// Disables cores in reverse order with delays
    pub fn power_off_sequence(&mut self) -> Result<(), VicorError> {
        // Disable cores 6→1
        for core in (1..=NUM_CORES).rev() {
            self.disable_core(core as u8)?;
            delay_us(1_000); // 1ms between disables
        }

        // Set all voltages to 0
        for core in 1..=NUM_CORES {
            let idx = core - 1;
            let (dac_ch, _) = CORE_MAP[idx];
            self.dac.set_raw(dac_ch, 0)?;
            self.voltages_mv[idx] = 0;
        }

        Ok(())
    }

    /// Check if core is enabled
    pub fn is_enabled(&self, core: u8) -> bool {
        if let Ok(idx) = self.validate_core(core) {
            self.enabled[idx]
        } else {
            false
        }
    }

    /// Get current voltage setting for core
    pub fn get_voltage_mv(&self, core: u8) -> u16 {
        if let Ok(idx) = self.validate_core(core) {
            self.voltages_mv[idx]
        } else {
            0
        }
    }

    /// Get status of all cores
    ///
    /// Returns array of (enabled, voltage_mv) tuples
    pub fn get_status(&self) -> [(bool, u16); NUM_CORES] {
        let mut status = [(false, 0u16); NUM_CORES];
        for i in 0..NUM_CORES {
            status[i] = (self.enabled[i], self.voltages_mv[i]);
        }
        status
    }

    /// Validate core number and return index
    fn validate_core(&self, core: u8) -> Result<usize, VicorError> {
        if core < 1 || core > NUM_CORES as u8 {
            Err(VicorError::InvalidCore)
        } else {
            Ok((core - 1) as usize)
        }
    }
}
