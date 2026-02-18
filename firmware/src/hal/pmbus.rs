//! PMBus Protocol Driver
//!
//! Power Management Bus communication for power supply control.
//! Built on top of the I2C driver.
//!
//! # ONETWO Design
//!
//! Invariant: PMBus command codes, data formats, CRC polynomial
//! Varies: Device addresses, voltage/current limits, manufacturer commands
//! Pattern: I2C transaction + PMBus command code + optional PEC (CRC-8)
//!
//! # Protocol Overview
//!
//! PMBus uses I2C with structured commands:
//! - Write: [addr+W] [cmd] [data...]
//! - Read:  [addr+W] [cmd] [addr+R] [data...]
//! - Optional PEC (Packet Error Check) byte for data integrity

use super::i2c::{I2c, I2cError};

/// PMBus command codes (common subset)
pub mod cmd {
    // Basic commands
    pub const PAGE: u8 = 0x00;              // Select page
    pub const OPERATION: u8 = 0x01;         // Enable/disable output
    pub const ON_OFF_CONFIG: u8 = 0x02;     // Power on/off configuration
    pub const CLEAR_FAULTS: u8 = 0x03;      // Clear fault bits
    pub const WRITE_PROTECT: u8 = 0x10;     // Write protection

    // Output voltage
    pub const VOUT_MODE: u8 = 0x20;         // Output voltage mode/exponent
    pub const VOUT_COMMAND: u8 = 0x21;      // Commanded output voltage
    pub const VOUT_TRIM: u8 = 0x22;         // Output voltage trim
    pub const VOUT_CAL_OFFSET: u8 = 0x23;   // Calibration offset
    pub const VOUT_MAX: u8 = 0x24;          // Maximum output voltage
    pub const VOUT_MARGIN_HIGH: u8 = 0x25;  // Margin high voltage
    pub const VOUT_MARGIN_LOW: u8 = 0x26;   // Margin low voltage
    pub const VOUT_TRANSITION_RATE: u8 = 0x27;
    pub const VOUT_DROOP: u8 = 0x28;        // Load line/droop
    pub const VOUT_SCALE_LOOP: u8 = 0x29;
    pub const VOUT_SCALE_MONITOR: u8 = 0x2A;
    pub const VOUT_MIN: u8 = 0x2B;          // Minimum output voltage

    // Fault limits
    pub const VOUT_OV_FAULT_LIMIT: u8 = 0x40;
    pub const VOUT_OV_FAULT_RESPONSE: u8 = 0x41;
    pub const VOUT_OV_WARN_LIMIT: u8 = 0x42;
    pub const VOUT_UV_WARN_LIMIT: u8 = 0x43;
    pub const VOUT_UV_FAULT_LIMIT: u8 = 0x44;
    pub const VOUT_UV_FAULT_RESPONSE: u8 = 0x45;
    pub const IOUT_OC_FAULT_LIMIT: u8 = 0x46;
    pub const IOUT_OC_FAULT_RESPONSE: u8 = 0x47;
    pub const IOUT_OC_LV_FAULT_LIMIT: u8 = 0x48;
    pub const IOUT_OC_LV_FAULT_RESPONSE: u8 = 0x49;
    pub const IOUT_OC_WARN_LIMIT: u8 = 0x4A;
    pub const IOUT_UC_FAULT_LIMIT: u8 = 0x4B;

    // Temperature limits
    pub const OT_FAULT_LIMIT: u8 = 0x4F;
    pub const OT_FAULT_RESPONSE: u8 = 0x50;
    pub const OT_WARN_LIMIT: u8 = 0x51;
    pub const UT_WARN_LIMIT: u8 = 0x52;
    pub const UT_FAULT_LIMIT: u8 = 0x53;

    // Input limits
    pub const VIN_ON: u8 = 0x35;
    pub const VIN_OFF: u8 = 0x36;
    pub const VIN_OV_FAULT_LIMIT: u8 = 0x55;
    pub const VIN_OV_WARN_LIMIT: u8 = 0x57;
    pub const VIN_UV_WARN_LIMIT: u8 = 0x58;
    pub const VIN_UV_FAULT_LIMIT: u8 = 0x59;
    pub const IIN_OC_FAULT_LIMIT: u8 = 0x5B;
    pub const IIN_OC_WARN_LIMIT: u8 = 0x5D;
    pub const POUT_OP_FAULT_LIMIT: u8 = 0x68;
    pub const POUT_OP_WARN_LIMIT: u8 = 0x6A;
    pub const PIN_OP_WARN_LIMIT: u8 = 0x6B;

    // Status commands
    pub const STATUS_BYTE: u8 = 0x78;       // Summary status
    pub const STATUS_WORD: u8 = 0x79;       // Extended status
    pub const STATUS_VOUT: u8 = 0x7A;       // Output voltage status
    pub const STATUS_IOUT: u8 = 0x7B;       // Output current status
    pub const STATUS_INPUT: u8 = 0x7C;      // Input status
    pub const STATUS_TEMPERATURE: u8 = 0x7D;// Temperature status
    pub const STATUS_CML: u8 = 0x7E;        // Communication/logic status
    pub const STATUS_OTHER: u8 = 0x7F;      // Other status
    pub const STATUS_MFR_SPECIFIC: u8 = 0x80;
    pub const STATUS_FANS_1_2: u8 = 0x81;
    pub const STATUS_FANS_3_4: u8 = 0x82;

    // Readback commands
    pub const READ_VIN: u8 = 0x88;          // Read input voltage
    pub const READ_IIN: u8 = 0x89;          // Read input current
    pub const READ_VCAP: u8 = 0x8A;         // Read capacitor voltage
    pub const READ_VOUT: u8 = 0x8B;         // Read output voltage
    pub const READ_IOUT: u8 = 0x8C;         // Read output current
    pub const READ_TEMPERATURE_1: u8 = 0x8D;
    pub const READ_TEMPERATURE_2: u8 = 0x8E;
    pub const READ_TEMPERATURE_3: u8 = 0x8F;
    pub const READ_FAN_SPEED_1: u8 = 0x90;
    pub const READ_FAN_SPEED_2: u8 = 0x91;
    pub const READ_FAN_SPEED_3: u8 = 0x92;
    pub const READ_FAN_SPEED_4: u8 = 0x93;
    pub const READ_DUTY_CYCLE: u8 = 0x94;
    pub const READ_FREQUENCY: u8 = 0x95;
    pub const READ_POUT: u8 = 0x96;         // Read output power
    pub const READ_PIN: u8 = 0x97;          // Read input power

    // Identification
    pub const PMBUS_REVISION: u8 = 0x98;
    pub const MFR_ID: u8 = 0x99;
    pub const MFR_MODEL: u8 = 0x9A;
    pub const MFR_REVISION: u8 = 0x9B;
    pub const MFR_LOCATION: u8 = 0x9C;
    pub const MFR_DATE: u8 = 0x9D;
    pub const MFR_SERIAL: u8 = 0x9E;

    // Coefficients for linear data format
    pub const COEFFICIENTS: u8 = 0x30;

    // Power control
    pub const CAPABILITY: u8 = 0x19;
    pub const QUERY: u8 = 0x1A;
}

/// Operation modes for OPERATION command
pub mod operation {
    pub const IMMEDIATE_OFF: u8 = 0x00;
    pub const SOFT_OFF: u8 = 0x40;
    pub const ON: u8 = 0x80;
    pub const MARGIN_LOW: u8 = 0x94;
    pub const MARGIN_HIGH: u8 = 0x98;
}

/// Status byte bits
pub mod status {
    pub const NONE_OF_THE_ABOVE: u8 = 1 << 0;
    pub const CML: u8 = 1 << 1;
    pub const TEMPERATURE: u8 = 1 << 2;
    pub const VIN_UV: u8 = 1 << 3;
    pub const IOUT_OC: u8 = 1 << 4;
    pub const VOUT_OV: u8 = 1 << 5;
    pub const OFF: u8 = 1 << 6;
    pub const BUSY: u8 = 1 << 7;
}

/// PMBus error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmbusError {
    /// I2C communication error
    I2c(I2cError),
    /// Device returned NACK
    Nack,
    /// PEC (CRC) mismatch
    PecError,
    /// Invalid data format
    InvalidData,
    /// Command not supported
    UnsupportedCommand,
}

impl From<I2cError> for PmbusError {
    fn from(e: I2cError) -> Self {
        PmbusError::I2c(e)
    }
}

/// PMBus device abstraction
pub struct PmbusDevice<'a> {
    i2c: &'a I2c,
    address: u8,
    use_pec: bool,
}

impl<'a> PmbusDevice<'a> {
    /// Create a new PMBus device
    ///
    /// # Arguments
    /// * `i2c` - I2C peripheral instance
    /// * `address` - 7-bit I2C address of the device
    pub fn new(i2c: &'a I2c, address: u8) -> Self {
        Self {
            i2c,
            address,
            use_pec: false,
        }
    }

    /// Enable PEC (Packet Error Checking)
    pub fn enable_pec(&mut self) {
        self.use_pec = true;
    }

    /// Disable PEC
    pub fn disable_pec(&mut self) {
        self.use_pec = false;
    }

    /// Calculate CRC-8 for PEC
    fn calculate_pec(data: &[u8]) -> u8 {
        let mut crc: u8 = 0;
        for &byte in data {
            crc ^= byte;
            for _ in 0..8 {
                if crc & 0x80 != 0 {
                    crc = (crc << 1) ^ 0x07;  // CRC-8 polynomial
                } else {
                    crc <<= 1;
                }
            }
        }
        crc
    }

    // =========================================================================
    // Low-level Commands
    // =========================================================================

    /// Send command only (no data)
    pub fn send_byte(&self, cmd: u8) -> Result<(), PmbusError> {
        self.i2c.write(self.address, &[cmd])?;
        Ok(())
    }

    /// Write byte (command + 1 byte data)
    pub fn write_byte(&self, cmd: u8, data: u8) -> Result<(), PmbusError> {
        if self.use_pec {
            let pec_data = [(self.address << 1), cmd, data];
            let pec = Self::calculate_pec(&pec_data);
            self.i2c.write(self.address, &[cmd, data, pec])?;
        } else {
            self.i2c.write(self.address, &[cmd, data])?;
        }
        Ok(())
    }

    /// Write word (command + 2 bytes data, little-endian)
    pub fn write_word(&self, cmd: u8, data: u16) -> Result<(), PmbusError> {
        let lo = data as u8;
        let hi = (data >> 8) as u8;
        if self.use_pec {
            let pec_data = [(self.address << 1), cmd, lo, hi];
            let pec = Self::calculate_pec(&pec_data);
            self.i2c.write(self.address, &[cmd, lo, hi, pec])?;
        } else {
            self.i2c.write(self.address, &[cmd, lo, hi])?;
        }
        Ok(())
    }

    /// Read byte
    pub fn read_byte(&self, cmd: u8) -> Result<u8, PmbusError> {
        let mut buf = [0u8; 1];
        self.i2c.write_read(self.address, &[cmd], &mut buf)?;
        Ok(buf[0])
    }

    /// Read word (little-endian)
    pub fn read_word(&self, cmd: u8) -> Result<u16, PmbusError> {
        let mut buf = [0u8; 2];
        self.i2c.write_read(self.address, &[cmd], &mut buf)?;
        Ok((buf[1] as u16) << 8 | (buf[0] as u16))
    }

    /// Read block (variable length)
    pub fn read_block(&self, cmd: u8, buf: &mut [u8]) -> Result<usize, PmbusError> {
        // First byte is length
        let mut temp = [0u8; 33];  // Max 32 bytes + length
        self.i2c.write_read(self.address, &[cmd], &mut temp[..buf.len() + 1])?;
        let len = temp[0] as usize;
        if len > buf.len() {
            return Err(PmbusError::InvalidData);
        }
        buf[..len].copy_from_slice(&temp[1..len + 1]);
        Ok(len)
    }

    // =========================================================================
    // High-level Commands
    // =========================================================================

    /// Clear all faults
    pub fn clear_faults(&self) -> Result<(), PmbusError> {
        self.send_byte(cmd::CLEAR_FAULTS)
    }

    /// Turn output on
    pub fn enable_output(&self) -> Result<(), PmbusError> {
        self.write_byte(cmd::OPERATION, operation::ON)
    }

    /// Turn output off (soft)
    pub fn disable_output(&self) -> Result<(), PmbusError> {
        self.write_byte(cmd::OPERATION, operation::SOFT_OFF)
    }

    /// Turn output off (immediate)
    pub fn disable_output_immediate(&self) -> Result<(), PmbusError> {
        self.write_byte(cmd::OPERATION, operation::IMMEDIATE_OFF)
    }

    /// Set margin high
    pub fn set_margin_high(&self) -> Result<(), PmbusError> {
        self.write_byte(cmd::OPERATION, operation::MARGIN_HIGH)
    }

    /// Set margin low
    pub fn set_margin_low(&self) -> Result<(), PmbusError> {
        self.write_byte(cmd::OPERATION, operation::MARGIN_LOW)
    }

    /// Read status byte
    pub fn read_status(&self) -> Result<u8, PmbusError> {
        self.read_byte(cmd::STATUS_BYTE)
    }

    /// Read status word
    pub fn read_status_word(&self) -> Result<u16, PmbusError> {
        self.read_word(cmd::STATUS_WORD)
    }

    /// Check if output is on
    pub fn is_output_on(&self) -> Result<bool, PmbusError> {
        let status = self.read_status()?;
        Ok(status & status::OFF == 0)
    }

    /// Check if any fault present
    pub fn has_fault(&self) -> Result<bool, PmbusError> {
        let status = self.read_status()?;
        Ok(status & (status::VOUT_OV | status::IOUT_OC | status::VIN_UV | status::TEMPERATURE) != 0)
    }

    // =========================================================================
    // Voltage Readback
    // =========================================================================

    /// Read output voltage (raw LINEAR16 format)
    pub fn read_vout_raw(&self) -> Result<u16, PmbusError> {
        self.read_word(cmd::READ_VOUT)
    }

    /// Read output voltage in millivolts
    ///
    /// Assumes VOUT_MODE indicates LINEAR16 with typical exponent (-12)
    pub fn read_vout_mv(&self) -> Result<u32, PmbusError> {
        let raw = self.read_vout_raw()?;
        // LINEAR16: V = raw * 2^N where N is exponent from VOUT_MODE
        // Common exponent is -12: V = raw * 2^-12 = raw / 4096
        // For mV: V_mV = raw * 1000 / 4096
        Ok((raw as u32 * 1000) / 4096)
    }

    /// Read input voltage (raw LINEAR11 format)
    pub fn read_vin_raw(&self) -> Result<u16, PmbusError> {
        self.read_word(cmd::READ_VIN)
    }

    /// Read input voltage in millivolts
    pub fn read_vin_mv(&self) -> Result<i32, PmbusError> {
        let raw = self.read_vin_raw()?;
        Ok(Self::linear11_to_milli(raw))
    }

    // =========================================================================
    // Current Readback
    // =========================================================================

    /// Read output current (raw LINEAR11 format)
    pub fn read_iout_raw(&self) -> Result<u16, PmbusError> {
        self.read_word(cmd::READ_IOUT)
    }

    /// Read output current in milliamps
    pub fn read_iout_ma(&self) -> Result<i32, PmbusError> {
        let raw = self.read_iout_raw()?;
        Ok(Self::linear11_to_milli(raw))
    }

    /// Read input current in milliamps
    pub fn read_iin_ma(&self) -> Result<i32, PmbusError> {
        let raw = self.read_word(cmd::READ_IIN)?;
        Ok(Self::linear11_to_milli(raw))
    }

    // =========================================================================
    // Power Readback
    // =========================================================================

    /// Read output power in milliwatts
    pub fn read_pout_mw(&self) -> Result<i32, PmbusError> {
        let raw = self.read_word(cmd::READ_POUT)?;
        Ok(Self::linear11_to_milli(raw))
    }

    /// Read input power in milliwatts
    pub fn read_pin_mw(&self) -> Result<i32, PmbusError> {
        let raw = self.read_word(cmd::READ_PIN)?;
        Ok(Self::linear11_to_milli(raw))
    }

    // =========================================================================
    // Temperature Readback
    // =========================================================================

    /// Read temperature 1 in millidegrees Celsius
    pub fn read_temperature_1_mc(&self) -> Result<i32, PmbusError> {
        let raw = self.read_word(cmd::READ_TEMPERATURE_1)?;
        Ok(Self::linear11_to_milli(raw))
    }

    // =========================================================================
    // Voltage Setting
    // =========================================================================

    /// Set output voltage command (raw LINEAR16)
    pub fn set_vout_raw(&self, value: u16) -> Result<(), PmbusError> {
        self.write_word(cmd::VOUT_COMMAND, value)
    }

    /// Set output voltage in millivolts
    ///
    /// Assumes exponent of -12 (standard LINEAR16)
    pub fn set_vout_mv(&self, mv: u32) -> Result<(), PmbusError> {
        // raw = mV * 4096 / 1000
        let raw = ((mv * 4096) / 1000) as u16;
        self.set_vout_raw(raw)
    }

    // =========================================================================
    // Identification
    // =========================================================================

    /// Read PMBus revision
    pub fn read_revision(&self) -> Result<u8, PmbusError> {
        self.read_byte(cmd::PMBUS_REVISION)
    }

    /// Read manufacturer ID (string)
    pub fn read_mfr_id(&self, buf: &mut [u8]) -> Result<usize, PmbusError> {
        self.read_block(cmd::MFR_ID, buf)
    }

    /// Read model number (string)
    pub fn read_model(&self, buf: &mut [u8]) -> Result<usize, PmbusError> {
        self.read_block(cmd::MFR_MODEL, buf)
    }

    // =========================================================================
    // Data Format Helpers
    // =========================================================================

    /// Convert LINEAR11 to millivalue (mV, mA, mW, etc.)
    ///
    /// LINEAR11 format: [15:11] = exponent (signed), [10:0] = mantissa (signed)
    fn linear11_to_milli(raw: u16) -> i32 {
        // Extract exponent (5 bits, signed)
        let exp = ((raw >> 11) & 0x1F) as i8;
        let exp = if exp & 0x10 != 0 { exp | 0xE0u8 as i8 } else { exp };  // Sign extend

        // Extract mantissa (11 bits, two's complement)
        let mant = (raw & 0x7FF) as i16;
        let mant = if mant & 0x400 != 0 { mant | 0xF800u16 as i16 } else { mant };  // Sign extend

        // Value = mantissa * 2^exponent
        // For millivalue: multiply by 1000 first, then apply exponent
        let milli = (mant as i32) * 1000;

        if exp >= 0 {
            milli << (exp as u32)
        } else {
            milli >> ((-exp) as u32)
        }
    }

    /// Convert millivalue to LINEAR11
    fn milli_to_linear11(milli: i32) -> u16 {
        if milli == 0 {
            return 0;
        }

        // Find best exponent
        let mut exp: i8 = 0;
        let mut mant = milli;

        // Scale down if needed
        while mant > 1023 || mant < -1024 {
            mant /= 2;
            exp += 1;
        }

        // Scale up if we can get more precision
        while exp > -16 && mant < 512 && mant > -512 {
            mant *= 2;
            exp -= 1;
        }

        // Convert from millivalue
        mant /= 1000;

        // Pack into LINEAR11
        let exp_bits = ((exp as u16) & 0x1F) << 11;
        let mant_bits = (mant as u16) & 0x7FF;

        exp_bits | mant_bits
    }
}

/// Scan for PMBus devices on I2C bus
pub fn scan_pmbus_devices(i2c: &I2c) -> [Option<u8>; 16] {
    let mut found = [None; 16];
    let mut count = 0;

    // Scan typical PMBus address range (0x10-0x6F)
    for addr in 0x10..0x70 {
        let device = PmbusDevice::new(i2c, addr);
        if device.read_status().is_ok() {
            if count < 16 {
                found[count] = Some(addr);
                count += 1;
            }
        }
    }

    found
}

// =============================================================================
// LCPS Channel Mapping (Sonoma/BIM Compatibility)
// =============================================================================
//
// ONETWO: The I2C addresses are INVARIANT - set by SA0/SA1 resistors on hardware.
// This mapping matches the Sonoma convention so existing .bim/.tp files work.
//
// Hardware layout (3 × 8-channel LCPS modules):
//   Module 1: SA0=0, SA1=0 → Base 0x10 → Channels 1-8
//   Module 2: SA0=1, SA1=0 → Base 0x18 → Channels 9-16
//   Module 3: SA0=0, SA1=1 → Base 0x20 → Channels 17-24
//

/// LCPS module base addresses (INVARIANT from hardware)
const LCPS_MODULE_BASE: [u8; 3] = [0x10, 0x18, 0x20];

/// Convert Sonoma LCPS channel number (1-24) to I2C address
///
/// # Examples
/// ```
/// assert_eq!(lcps_channel_to_addr(1),  Some(0x10));  // Module 1, ch 0
/// assert_eq!(lcps_channel_to_addr(8),  Some(0x17));  // Module 1, ch 7
/// assert_eq!(lcps_channel_to_addr(9),  Some(0x18));  // Module 2, ch 0
/// assert_eq!(lcps_channel_to_addr(17), Some(0x20));  // Module 3, ch 0
/// assert_eq!(lcps_channel_to_addr(24), Some(0x27));  // Module 3, ch 7
/// assert_eq!(lcps_channel_to_addr(0),  None);        // Invalid
/// assert_eq!(lcps_channel_to_addr(25), None);        // Invalid
/// ```
pub const fn lcps_channel_to_addr(channel: u8) -> Option<u8> {
    if channel < 1 || channel > 24 {
        return None;
    }
    let module = ((channel - 1) / 8) as usize;
    let offset = (channel - 1) % 8;
    Some(LCPS_MODULE_BASE[module] + offset)
}

/// Convert I2C address back to Sonoma channel number
pub const fn lcps_addr_to_channel(addr: u8) -> Option<u8> {
    match addr {
        0x10..=0x17 => Some(addr - 0x10 + 1),       // Module 1: channels 1-8
        0x18..=0x1F => Some(addr - 0x18 + 9),       // Module 2: channels 9-16
        0x20..=0x27 => Some(addr - 0x20 + 17),      // Module 3: channels 17-24
        _ => None,
    }
}

// =============================================================================
// Power Supply Manager - Auto-Discovery for HCPS/LCPS
// =============================================================================

/// Maximum number of power supplies (4 HCPS + 8 LCPS = 12, but support up to 16)
pub const MAX_POWER_SUPPLIES: usize = 16;

/// Discovered power supply info
#[derive(Debug, Clone, Copy)]
pub struct PowerSupplyInfo {
    /// I2C address (7-bit)
    pub address: u8,
    /// Which I2C bus (0 or 1)
    pub bus: u8,
    /// PMBus revision reported by device
    pub revision: u8,
    /// Is output currently enabled?
    pub output_on: bool,
    /// Last read output voltage (mV)
    pub vout_mv: u32,
    /// Last read output current (mA)
    pub iout_ma: i32,
}

impl Default for PowerSupplyInfo {
    fn default() -> Self {
        Self {
            address: 0,
            bus: 0,
            revision: 0,
            output_on: false,
            vout_mv: 0,
            iout_ma: 0,
        }
    }
}

/// Power Supply Manager - discovers and controls all PMBus devices
///
/// Doesn't care about specific addresses - just scans and finds whatever exists.
/// Works with HCPS (High Current) and LCPS (Low Current) regardless of their
/// resistor-programmed addresses.
pub struct PowerSupplyManager<'a> {
    /// I2C bus 0 (typically HCPS)
    i2c0: &'a I2c,
    /// I2C bus 1 (typically LCPS) - optional
    i2c1: Option<&'a I2c>,
    /// Discovered power supplies
    supplies: [PowerSupplyInfo; MAX_POWER_SUPPLIES],
    /// Number of discovered supplies
    count: usize,
}

impl<'a> PowerSupplyManager<'a> {
    /// Create manager with single I2C bus
    pub fn new(i2c: &'a I2c) -> Self {
        Self {
            i2c0: i2c,
            i2c1: None,
            supplies: [PowerSupplyInfo::default(); MAX_POWER_SUPPLIES],
            count: 0,
        }
    }

    /// Create manager with two I2C buses (HCPS on bus0, LCPS on bus1)
    pub fn new_dual(i2c0: &'a I2c, i2c1: &'a I2c) -> Self {
        Self {
            i2c0,
            i2c1: Some(i2c1),
            supplies: [PowerSupplyInfo::default(); MAX_POWER_SUPPLIES],
            count: 0,
        }
    }

    /// Scan I2C buses and discover all PMBus power supplies
    ///
    /// Call this on startup to find all connected supplies.
    /// Returns number of devices found.
    pub fn discover(&mut self) -> usize {
        self.count = 0;

        // Scan bus 0
        self.scan_bus(self.i2c0, 0);

        // Scan bus 1 if present
        if let Some(i2c1) = self.i2c1 {
            self.scan_bus(i2c1, 1);
        }

        self.count
    }

    /// Scan a single I2C bus for PMBus devices
    fn scan_bus(&mut self, i2c: &I2c, bus: u8) {
        // Scan full PMBus address range
        // Common ranges: 0x10-0x1F, 0x20-0x2F, 0x40-0x4F, 0x50-0x5F
        for addr in 0x10..0x70 {
            if self.count >= MAX_POWER_SUPPLIES {
                break;
            }

            let device = PmbusDevice::new(i2c, addr);

            // Try to read status - if it ACKs, it's a PMBus device
            if let Ok(status) = device.read_status() {
                // Get revision if possible
                let revision = device.read_revision().unwrap_or(0);

                self.supplies[self.count] = PowerSupplyInfo {
                    address: addr,
                    bus,
                    revision,
                    output_on: (status & status::OFF) == 0,
                    vout_mv: 0,
                    iout_ma: 0,
                };
                self.count += 1;
            }
        }
    }

    /// Get number of discovered power supplies
    pub fn count(&self) -> usize {
        self.count
    }

    /// Get info for a power supply by index (0..count)
    pub fn get(&self, index: usize) -> Option<&PowerSupplyInfo> {
        if index < self.count {
            Some(&self.supplies[index])
        } else {
            None
        }
    }

    /// Get all discovered supplies
    pub fn all(&self) -> &[PowerSupplyInfo] {
        &self.supplies[..self.count]
    }

    /// Get I2C bus reference for a supply
    fn get_i2c(&self, bus: u8) -> &I2c {
        if bus == 1 {
            self.i2c1.unwrap_or(self.i2c0)
        } else {
            self.i2c0
        }
    }

    /// Create a PmbusDevice for a discovered supply
    fn device_for(&self, index: usize) -> Option<PmbusDevice<'a>> {
        if index < self.count {
            let info = &self.supplies[index];
            // Note: This creates a device with the correct I2C bus
            // We need to be careful about lifetimes here
            let i2c = if info.bus == 1 {
                self.i2c1.unwrap_or(self.i2c0)
            } else {
                self.i2c0
            };
            Some(PmbusDevice::new(i2c, info.address))
        } else {
            None
        }
    }

    // =========================================================================
    // Control All Supplies
    // =========================================================================

    /// Enable output on all discovered supplies
    pub fn enable_all(&self) -> Result<(), PmbusError> {
        for i in 0..self.count {
            if let Some(dev) = self.device_for(i) {
                dev.enable_output()?;
            }
        }
        Ok(())
    }

    /// Disable output on all discovered supplies (emergency stop)
    pub fn disable_all(&self) {
        for i in 0..self.count {
            if let Some(dev) = self.device_for(i) {
                let _ = dev.disable_output_immediate();  // Ignore errors, just stop everything
            }
        }
    }

    /// Clear faults on all supplies
    pub fn clear_all_faults(&self) {
        for i in 0..self.count {
            if let Some(dev) = self.device_for(i) {
                let _ = dev.clear_faults();
            }
        }
    }

    // =========================================================================
    // Control Individual Supply by Index
    // =========================================================================

    /// Enable output on supply at index
    pub fn enable(&self, index: usize) -> Result<(), PmbusError> {
        self.device_for(index)
            .ok_or(PmbusError::InvalidData)?
            .enable_output()
    }

    /// Disable output on supply at index
    pub fn disable(&self, index: usize) -> Result<(), PmbusError> {
        self.device_for(index)
            .ok_or(PmbusError::InvalidData)?
            .disable_output()
    }

    /// Enable output on supply by I2C address
    pub fn enable_by_addr(&self, addr: u8) -> Result<(), PmbusError> {
        for i in 0..self.count {
            if self.supplies[i].address == addr {
                return self.device_for(i)
                    .ok_or(PmbusError::InvalidData)?
                    .enable_output();
            }
        }
        Err(PmbusError::InvalidData) // Address not found
    }

    /// Disable output on supply by I2C address
    pub fn disable_by_addr(&self, addr: u8) -> Result<(), PmbusError> {
        for i in 0..self.count {
            if self.supplies[i].address == addr {
                return self.device_for(i)
                    .ok_or(PmbusError::InvalidData)?
                    .disable_output();
            }
        }
        Err(PmbusError::InvalidData) // Address not found
    }

    /// Set voltage on supply at index (millivolts)
    pub fn set_voltage(&self, index: usize, mv: u32) -> Result<(), PmbusError> {
        self.device_for(index)
            .ok_or(PmbusError::InvalidData)?
            .set_vout_mv(mv)
    }

    // =========================================================================
    // Read Telemetry
    // =========================================================================

    /// Update telemetry for all supplies (call periodically)
    pub fn update_telemetry(&mut self) {
        for i in 0..self.count {
            // Copy values we need to avoid borrow checker issues
            let bus = self.supplies[i].bus;
            let address = self.supplies[i].address;

            // Read all values first, then drop the borrow before updating struct
            let (vout, iout, status_byte) = {
                let i2c = self.get_i2c(bus);
                let dev = PmbusDevice::new(i2c, address);
                (
                    dev.read_vout_mv().ok(),
                    dev.read_iout_ma().ok(),
                    dev.read_status().ok(),
                )
            }; // dev and i2c borrow dropped here

            // Now safe to mutate self.supplies
            if let Some(mv) = vout {
                self.supplies[i].vout_mv = mv;
            }
            if let Some(ma) = iout {
                self.supplies[i].iout_ma = ma;
            }
            if let Some(status) = status_byte {
                self.supplies[i].output_on = (status & status::OFF) == 0;
            }
        }
    }

    /// Read voltage from supply at index (millivolts)
    pub fn read_voltage(&self, index: usize) -> Result<u32, PmbusError> {
        self.device_for(index)
            .ok_or(PmbusError::InvalidData)?
            .read_vout_mv()
    }

    /// Read current from supply at index (milliamps)
    pub fn read_current(&self, index: usize) -> Result<i32, PmbusError> {
        self.device_for(index)
            .ok_or(PmbusError::InvalidData)?
            .read_iout_ma()
    }

    /// Check if any supply has a fault
    pub fn any_fault(&self) -> bool {
        for i in 0..self.count {
            if let Some(dev) = self.device_for(i) {
                if dev.has_fault().unwrap_or(true) {
                    return true;
                }
            }
        }
        false
    }

    // =========================================================================
    // Channel-Based API (Sonoma/BIM Compatibility)
    // =========================================================================

    /// Set voltage by Sonoma channel number (1-24)
    ///
    /// This is the primary API for BIM compatibility.
    /// Maps channel → I2C address automatically.
    pub fn set_voltage_by_channel(&self, channel: u8, mv: u32) -> Result<(), PmbusError> {
        let addr = lcps_channel_to_addr(channel).ok_or(PmbusError::InvalidData)?;
        let dev = PmbusDevice::new(self.i2c0, addr);
        dev.set_vout_mv(mv)
    }

    /// Enable output by Sonoma channel number (1-24)
    pub fn enable_by_channel(&self, channel: u8) -> Result<(), PmbusError> {
        let addr = lcps_channel_to_addr(channel).ok_or(PmbusError::InvalidData)?;
        let dev = PmbusDevice::new(self.i2c0, addr);
        dev.enable_output()
    }

    /// Disable output by Sonoma channel number (1-24)
    pub fn disable_by_channel(&self, channel: u8) -> Result<(), PmbusError> {
        let addr = lcps_channel_to_addr(channel).ok_or(PmbusError::InvalidData)?;
        let dev = PmbusDevice::new(self.i2c0, addr);
        dev.disable_output()
    }

    /// Read voltage by Sonoma channel number (1-24)
    pub fn read_voltage_by_channel(&self, channel: u8) -> Result<u32, PmbusError> {
        let addr = lcps_channel_to_addr(channel).ok_or(PmbusError::InvalidData)?;
        let dev = PmbusDevice::new(self.i2c0, addr);
        dev.read_vout_mv()
    }

    /// Read current by Sonoma channel number (1-24)
    pub fn read_current_by_channel(&self, channel: u8) -> Result<i32, PmbusError> {
        let addr = lcps_channel_to_addr(channel).ok_or(PmbusError::InvalidData)?;
        let dev = PmbusDevice::new(self.i2c0, addr);
        dev.read_iout_ma()
    }
}
