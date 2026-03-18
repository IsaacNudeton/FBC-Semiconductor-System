//! XADC (Xilinx Analog-to-Digital Converter)
//!
//! On-chip ADC for temperature and voltage monitoring.
//! Accessed via PS-XADC interface (not PL XADC).
//!
//! # ONETWO Design
//!
//! Invariant: Register addresses, conversion formulas, channel assignments
//! Varies: Thresholds, alarm configuration, sampling rate
//! Pattern: Read raw ADC → apply formula → return physical value

use super::{Reg, Register};

/// XADC base address (PS-XADC interface)
const XADC_BASE: usize = 0xF800_7100;

/// XADC register offsets (from PS interface, not direct XADC)
mod regs {
    pub const CFG: usize = 0x00;       // Configuration
    pub const INT_STS: usize = 0x04;   // Interrupt Status
    pub const INT_MASK: usize = 0x08;  // Interrupt Mask
    pub const MSTS: usize = 0x0C;      // Miscellaneous Status
    pub const CMDFIFO: usize = 0x10;   // Command FIFO
    pub const RDFIFO: usize = 0x14;    // Read FIFO
    pub const MCTL: usize = 0x18;      // Miscellaneous Control
}

/// Direct XADC registers (accessed via CMDFIFO/RDFIFO)
mod xadc_regs {
    // Status registers (read-only)
    pub const TEMPERATURE: u8 = 0x00;  // On-chip temperature
    pub const VCCINT: u8 = 0x01;       // Internal core voltage
    pub const VCCAUX: u8 = 0x02;       // Auxiliary voltage
    pub const VPVN: u8 = 0x03;         // Dedicated analog input
    pub const VREFP: u8 = 0x04;        // Reference P
    pub const VREFN: u8 = 0x05;        // Reference N
    pub const VCCBRAM: u8 = 0x06;      // BRAM voltage

    // Auxiliary inputs (if routed in PL)
    pub const VAUX0: u8 = 0x10;
    pub const VAUX1: u8 = 0x11;
    pub const VAUX2: u8 = 0x12;
    pub const VAUX3: u8 = 0x13;
    pub const VAUX4: u8 = 0x14;
    pub const VAUX5: u8 = 0x15;
    pub const VAUX6: u8 = 0x16;
    pub const VAUX7: u8 = 0x17;

    // Max/min registers
    pub const MAX_TEMP: u8 = 0x20;
    pub const MAX_VCCINT: u8 = 0x21;
    pub const MAX_VCCAUX: u8 = 0x22;
    pub const MAX_VCCBRAM: u8 = 0x23;
    pub const MIN_TEMP: u8 = 0x24;
    pub const MIN_VCCINT: u8 = 0x25;
    pub const MIN_VCCAUX: u8 = 0x26;
    pub const MIN_VCCBRAM: u8 = 0x27;

    // Alarm thresholds
    pub const TEMP_UPPER: u8 = 0x50;
    pub const VCCINT_UPPER: u8 = 0x51;
    pub const VCCAUX_UPPER: u8 = 0x52;
    pub const OT_UPPER: u8 = 0x53;     // Over-temperature
    pub const TEMP_LOWER: u8 = 0x54;
    pub const VCCINT_LOWER: u8 = 0x55;
    pub const VCCAUX_LOWER: u8 = 0x56;
    pub const OT_LOWER: u8 = 0x57;

    // Configuration registers
    pub const CONFIG0: u8 = 0x40;
    pub const CONFIG1: u8 = 0x41;
    pub const CONFIG2: u8 = 0x42;
    pub const SEQ0: u8 = 0x48;         // Sequencer channel selection
    pub const SEQ1: u8 = 0x49;
    pub const SEQ2: u8 = 0x4A;
    pub const SEQ3: u8 = 0x4B;
}

/// CFG register bits
mod cfg {
    pub const CFIFOTH_MASK: u32 = 0xF << 20;  // Command FIFO threshold
    pub const DFIFOTH_MASK: u32 = 0xF << 16;  // Data FIFO threshold
    pub const WEDGE: u32 = 1 << 13;           // Write edge
    pub const REDGE: u32 = 1 << 12;           // Read edge
    pub const TCKRATE_MASK: u32 = 0x3 << 8;   // Clock rate
    pub const IGAP_MASK: u32 = 0x1F;          // Inter-access gap
}

/// MSTS register bits
mod msts {
    pub const CFIFO_LVL_MASK: u32 = 0xF << 16;
    pub const DFIFO_LVL_MASK: u32 = 0xF << 12;
    pub const CFIFOF: u32 = 1 << 11;  // Command FIFO full
    pub const CFIFOE: u32 = 1 << 10;  // Command FIFO empty
    pub const DFIFOF: u32 = 1 << 9;   // Data FIFO full
    pub const DFIFOE: u32 = 1 << 8;   // Data FIFO empty
    pub const OT: u32 = 1 << 7;       // Over-temperature
    pub const ALM_MASK: u32 = 0x7F;   // Alarm flags
}

/// XADC error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XadcError {
    /// FIFO overflow/underflow
    FifoError,
    /// Timeout waiting for conversion
    Timeout,
    /// Over-temperature condition
    OverTemperature,
}

/// XADC driver
pub struct Xadc {
    base: Reg,
}

impl Xadc {
    /// Create XADC instance
    pub const fn new() -> Self {
        Self { base: Reg::new(XADC_BASE) }
    }

    /// Initialize XADC
    pub fn init(&self) {
        // Configure PS-XADC interface
        // Use default clock rate, reasonable FIFO thresholds
        let config = (4 << 20)  // Command FIFO threshold = 4
            | (4 << 16)         // Data FIFO threshold = 4
            | (2 << 8)          // Clock rate = divide by 4
            | 5;                // Inter-access gap = 5

        self.base.offset(regs::CFG).write(config);

        // Clear any pending interrupts
        self.base.offset(regs::INT_STS).write(0xFFFFFFFF);

        // Mask all interrupts (we'll poll)
        self.base.offset(regs::INT_MASK).write(0xFFFFFFFF);
    }

    /// Read raw 16-bit value from XADC register
    fn read_raw(&self, reg: u8) -> Result<u16, XadcError> {
        // Wait for command FIFO not full
        for _ in 0..1000 {
            if self.base.offset(regs::MSTS).read() & msts::CFIFOF == 0 {
                break;
            }
            super::delay_us(1);
        }

        // Command format: [31:26]=0, [25:16]=addr, [15:0]=data (ignored for read)
        // Bit 26 = 0 for read, 1 for write
        let cmd = (reg as u32) << 16;
        self.base.offset(regs::CMDFIFO).write(cmd);

        // Wait for data FIFO not empty
        for _ in 0..10000 {
            if self.base.offset(regs::MSTS).read() & msts::DFIFOE == 0 {
                return Ok((self.base.offset(regs::RDFIFO).read() & 0xFFFF) as u16);
            }
            super::delay_us(1);
        }

        Err(XadcError::Timeout)
    }

    /// Write to XADC register
    fn write_raw(&self, reg: u8, value: u16) -> Result<(), XadcError> {
        // Wait for command FIFO not full
        for _ in 0..1000 {
            if self.base.offset(regs::MSTS).read() & msts::CFIFOF == 0 {
                break;
            }
            super::delay_us(1);
        }

        // Command format: bit 26 = 1 for write
        let cmd = (1 << 26) | ((reg as u32) << 16) | (value as u32);
        self.base.offset(regs::CMDFIFO).write(cmd);

        Ok(())
    }

    // =========================================================================
    // Temperature
    // =========================================================================

    /// Read on-chip temperature (raw ADC value)
    pub fn read_temperature_raw(&self) -> Result<u16, XadcError> {
        self.read_raw(xadc_regs::TEMPERATURE)
    }

    /// Read on-chip temperature in degrees Celsius
    ///
    /// Formula: T(°C) = (ADC * 503.975 / 65536) - 273.15
    pub fn read_temperature_celsius(&self) -> Result<i32, XadcError> {
        let raw = self.read_temperature_raw()? as u32;
        // Scaled calculation to avoid floating point
        // T * 1000 = (raw * 503975 / 65536) - 273150
        let millidegrees = ((raw * 503975) / 65536) as i32 - 273150;
        Ok(millidegrees / 1000)
    }

    /// Read on-chip temperature in millidegrees Celsius (more precision)
    pub fn read_temperature_millicelsius(&self) -> Result<i32, XadcError> {
        let raw = self.read_temperature_raw()? as u32;
        let millidegrees = ((raw * 503975) / 65536) as i32 - 273150;
        Ok(millidegrees)
    }

    /// Get maximum recorded temperature (raw)
    pub fn get_max_temperature_raw(&self) -> Result<u16, XadcError> {
        self.read_raw(xadc_regs::MAX_TEMP)
    }

    /// Get minimum recorded temperature (raw)
    pub fn get_min_temperature_raw(&self) -> Result<u16, XadcError> {
        self.read_raw(xadc_regs::MIN_TEMP)
    }

    // =========================================================================
    // Supply Voltages
    // =========================================================================

    /// Read VCCINT (internal core voltage) raw
    pub fn read_vccint_raw(&self) -> Result<u16, XadcError> {
        self.read_raw(xadc_regs::VCCINT)
    }

    /// Read VCCINT in millivolts
    ///
    /// Formula: V = ADC * 3.0 / 65536 (for 0-3V range)
    pub fn read_vccint_mv(&self) -> Result<u32, XadcError> {
        let raw = self.read_vccint_raw()? as u32;
        // V(mV) = raw * 3000 / 65536
        Ok((raw * 3000) / 65536)
    }

    /// Read VCCAUX (auxiliary voltage) raw
    pub fn read_vccaux_raw(&self) -> Result<u16, XadcError> {
        self.read_raw(xadc_regs::VCCAUX)
    }

    /// Read VCCAUX in millivolts
    pub fn read_vccaux_mv(&self) -> Result<u32, XadcError> {
        let raw = self.read_vccaux_raw()? as u32;
        Ok((raw * 3000) / 65536)
    }

    /// Read VCCBRAM (BRAM voltage) raw
    pub fn read_vccbram_raw(&self) -> Result<u16, XadcError> {
        self.read_raw(xadc_regs::VCCBRAM)
    }

    /// Read VCCBRAM in millivolts
    pub fn read_vccbram_mv(&self) -> Result<u32, XadcError> {
        let raw = self.read_vccbram_raw()? as u32;
        Ok((raw * 3000) / 65536)
    }

    // =========================================================================
    // Alarm Status
    // =========================================================================

    /// Check if over-temperature condition exists
    pub fn is_over_temperature(&self) -> bool {
        self.base.offset(regs::MSTS).read() & msts::OT != 0
    }

    /// Get alarm flags
    pub fn get_alarm_flags(&self) -> u8 {
        (self.base.offset(regs::MSTS).read() & msts::ALM_MASK) as u8
    }

    /// Set temperature alarm thresholds (raw values)
    pub fn set_temperature_alarms(&self, upper: u16, lower: u16) -> Result<(), XadcError> {
        self.write_raw(xadc_regs::TEMP_UPPER, upper)?;
        self.write_raw(xadc_regs::TEMP_LOWER, lower)?;
        Ok(())
    }

    /// Set over-temperature threshold (raw value)
    ///
    /// Default is ~125°C. Hardware shutdown occurs above this.
    pub fn set_over_temperature_threshold(&self, threshold: u16) -> Result<(), XadcError> {
        self.write_raw(xadc_regs::OT_UPPER, threshold)
    }

    // =========================================================================
    // Auxiliary Channels
    // =========================================================================

    /// Read auxiliary channel (0-7)
    pub fn read_vaux_raw(&self, channel: u8) -> Result<u16, XadcError> {
        if channel > 7 {
            return Err(XadcError::FifoError);
        }
        self.read_raw(xadc_regs::VAUX0 + channel)
    }

    /// Read auxiliary channel in millivolts (assuming 0-1V unipolar)
    pub fn read_vaux_mv(&self, channel: u8) -> Result<u32, XadcError> {
        let raw = self.read_vaux_raw(channel)? as u32;
        // Unipolar: V = ADC * 1.0 / 65536
        Ok((raw * 1000) / 65536)
    }

    // =========================================================================
    // Full System Status
    // =========================================================================

    /// Get complete system status
    pub fn get_system_status(&self) -> Result<SystemStatus, XadcError> {
        Ok(SystemStatus {
            temperature_mc: self.read_temperature_millicelsius()?,
            vccint_mv: self.read_vccint_mv()?,
            vccaux_mv: self.read_vccaux_mv()?,
            vccbram_mv: self.read_vccbram_mv()?,
            over_temp: self.is_over_temperature(),
            alarms: self.get_alarm_flags(),
        })
    }
}

/// System status from XADC
#[derive(Debug, Clone)]
pub struct SystemStatus {
    /// Temperature in millidegrees Celsius
    pub temperature_mc: i32,
    /// VCCINT in millivolts
    pub vccint_mv: u32,
    /// VCCAUX in millivolts
    pub vccaux_mv: u32,
    /// VCCBRAM in millivolts
    pub vccbram_mv: u32,
    /// Over-temperature flag
    pub over_temp: bool,
    /// Alarm flags
    pub alarms: u8,
}
