//! 24LC02 I2C EEPROM Driver
//!
//! Driver for 256-byte I2C EEPROM used for BIM configuration storage.
//!
//! # Hardware
//! - Device: 24LC02BHT-I/LT (Microchip)
//! - Capacity: 256 bytes (2 Kbit)
//! - Page size: 8 bytes
//! - Write time: 5ms max per page
//! - Interface: I2C, 400 kHz
//!
//! # EEPROM Layout
//!
//! ```text
//! 0x00-0x0F: Header (16 bytes)
//! 0x10-0x4F: Power rail config (64 bytes)
//! 0x50-0x8F: Calibration data (64 bytes)
//! 0x90-0xEF: DUT metadata (96 bytes)
//! 0xF0-0xF7: Statistics (8 bytes)
//! 0xF8-0xFB: CRC32 checksum (4 bytes)
//! 0xFC-0xFF: Reserved (4 bytes)
//! ```

use super::{I2c, I2cError, Gpio, MioPin, delay_ms};

/// EEPROM size in bytes
pub const EEPROM_SIZE: usize = 256;

/// EEPROM page size (write granularity)
const PAGE_SIZE: usize = 8;

/// Write cycle time in milliseconds
const WRITE_CYCLE_MS: u32 = 5;

/// Default I2C address (A0 = 0)
pub const EEPROM_ADDR: u8 = 0x50;

/// EEPROM error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EepromError {
    /// I2C communication error
    I2c(I2cError),
    /// Invalid EEPROM address
    InvalidAddress,
    /// Write verification failed
    VerifyFailed,
    /// Invalid checksum
    ChecksumMismatch,
    /// Invalid magic number (EEPROM not programmed)
    InvalidMagic,
}

impl From<I2cError> for EepromError {
    fn from(e: I2cError) -> Self {
        EepromError::I2c(e)
    }
}

/// 24LC02 EEPROM driver
///
/// Supports optional Write Protect (WP) pin control. The WP pin on 24LC02:
/// - HIGH = Write protected (reads only)
/// - LOW = Write enabled
///
/// BIM Hardware Notes:
/// - Most BIMs (e.g., Normandy) use a **jumper header** for WP control, not GPIO.
///   The jumper (J152 on Normandy) selects write-enable or write-protect.
/// - If WP is tied to GND on the hardware, no GPIO is needed - use `new()`.
/// - If WP is connected to a GPIO (rare), pass it to `with_wp_pin()`.
pub struct Eeprom<'a> {
    i2c: &'a I2c,
    addr: u8,
    /// Optional GPIO controller for WP pin
    gpio: Option<&'a Gpio>,
    /// Optional WP pin number (MIO)
    wp_pin: Option<MioPin>,
}

impl<'a> Eeprom<'a> {
    /// Create new EEPROM instance (no WP pin control)
    ///
    /// Use this if WP is tied to GND on the hardware (most common).
    ///
    /// # Arguments
    /// * `i2c` - I2C peripheral instance
    /// * `addr` - Device I2C address (0x50-0x57 depending on A0-A2 pins)
    pub const fn new(i2c: &'a I2c, addr: u8) -> Self {
        Self { i2c, addr, gpio: None, wp_pin: None }
    }

    /// Create new EEPROM instance with WP pin control
    ///
    /// Use this if WP is connected to a GPIO on the hardware.
    ///
    /// # Arguments
    /// * `i2c` - I2C peripheral instance
    /// * `addr` - Device I2C address (0x50-0x57 depending on A0-A2 pins)
    /// * `gpio` - GPIO controller
    /// * `wp_pin` - MIO pin connected to EEPROM WP
    pub const fn with_wp_pin(i2c: &'a I2c, addr: u8, gpio: &'a Gpio, wp_pin: MioPin) -> Self {
        Self { i2c, addr, gpio: Some(gpio), wp_pin: Some(wp_pin) }
    }

    /// Enable writes by driving WP LOW
    fn enable_writes(&self) {
        if let (Some(gpio), Some(pin)) = (self.gpio, self.wp_pin) {
            gpio.set_output(pin);
            gpio.write_pin(pin, false);  // LOW = write enabled
            delay_ms(1);  // Allow pin to settle
        }
    }

    /// Disable writes by driving WP HIGH
    fn disable_writes(&self) {
        if let (Some(gpio), Some(pin)) = (self.gpio, self.wp_pin) {
            gpio.write_pin(pin, true);  // HIGH = write protected
        }
    }

    /// Read a single byte from EEPROM
    ///
    /// # Arguments
    /// * `address` - EEPROM address (0-255)
    pub fn read_byte(&self, address: u8) -> Result<u8, EepromError> {
        if address as usize >= EEPROM_SIZE {
            return Err(EepromError::InvalidAddress);
        }

        let mut buf = [0u8; 1];
        self.i2c.write_read(self.addr, &[address], &mut buf)?;
        Ok(buf[0])
    }

    /// Write a single byte to EEPROM
    ///
    /// # Arguments
    /// * `address` - EEPROM address (0-255)
    /// * `data` - Byte to write
    pub fn write_byte(&self, address: u8, data: u8) -> Result<(), EepromError> {
        if address as usize >= EEPROM_SIZE {
            return Err(EepromError::InvalidAddress);
        }

        // Enable writes (if WP pin is controlled)
        self.enable_writes();

        // Write byte
        let result = self.i2c.write(self.addr, &[address, data]);

        // Wait for write cycle to complete
        delay_ms(WRITE_CYCLE_MS);

        // Disable writes (if WP pin is controlled)
        self.disable_writes();

        // Check I2C result
        result?;

        // Verify write
        let verify = self.read_byte(address)?;
        if verify != data {
            return Err(EepromError::VerifyFailed);
        }

        Ok(())
    }

    /// Read sequential bytes from EEPROM
    ///
    /// # Arguments
    /// * `address` - Starting EEPROM address
    /// * `buf` - Buffer to store read data
    pub fn read(&self, address: u8, buf: &mut [u8]) -> Result<(), EepromError> {
        if address as usize + buf.len() > EEPROM_SIZE {
            return Err(EepromError::InvalidAddress);
        }

        // Set address pointer, then read sequentially
        self.i2c.write_read(self.addr, &[address], buf)?;
        Ok(())
    }

    /// Write sequential bytes to EEPROM (page-aware)
    ///
    /// This handles page boundaries automatically. The 24LC02 has 8-byte pages,
    /// and writes must not cross page boundaries.
    ///
    /// # Arguments
    /// * `address` - Starting EEPROM address
    /// * `data` - Data to write
    pub fn write(&self, address: u8, data: &[u8]) -> Result<(), EepromError> {
        if address as usize + data.len() > EEPROM_SIZE {
            return Err(EepromError::InvalidAddress);
        }

        // Enable writes (if WP pin is controlled)
        self.enable_writes();

        let mut offset = 0;
        let mut current_addr = address as usize;
        let mut result: Result<(), EepromError> = Ok(());

        while offset < data.len() {
            // Calculate bytes remaining in current page
            let page_start = current_addr & !(PAGE_SIZE - 1);  // Align to page boundary
            let page_end = page_start + PAGE_SIZE;
            let bytes_in_page = page_end - current_addr;
            let bytes_to_write = core::cmp::min(bytes_in_page, data.len() - offset);

            // Build write command: [address, data...]
            let mut write_buf = [0u8; PAGE_SIZE + 1];  // Max page size + address byte
            write_buf[0] = current_addr as u8;
            write_buf[1..1 + bytes_to_write].copy_from_slice(&data[offset..offset + bytes_to_write]);

            // Write page
            if let Err(e) = self.i2c.write(self.addr, &write_buf[..1 + bytes_to_write]) {
                result = Err(e.into());
                break;
            }

            // Wait for write cycle
            delay_ms(WRITE_CYCLE_MS);

            offset += bytes_to_write;
            current_addr += bytes_to_write;
        }

        // Disable writes (if WP pin is controlled) - do this before verification
        self.disable_writes();

        // Check for I2C errors
        result?;

        // Verify all written data
        let mut verify_buf = [0u8; EEPROM_SIZE];
        self.read(address, &mut verify_buf[..data.len()])?;
        if verify_buf[..data.len()] != *data {
            return Err(EepromError::VerifyFailed);
        }

        Ok(())
    }

    /// Read entire EEPROM contents
    pub fn read_all(&self, buf: &mut [u8; EEPROM_SIZE]) -> Result<(), EepromError> {
        self.read(0, buf)
    }

    /// Write entire EEPROM contents
    pub fn write_all(&self, data: &[u8; EEPROM_SIZE]) -> Result<(), EepromError> {
        self.write(0, data)
    }

    /// Erase EEPROM (write all 0xFF)
    pub fn erase(&self) -> Result<(), EepromError> {
        let empty = [0xFFu8; EEPROM_SIZE];
        self.write_all(&empty)
    }
}

/// CRC32 lookup table (IEEE 802.3 polynomial: 0xEDB88320)
const CRC32_TABLE: [u32; 256] = generate_crc32_table();

/// Generate CRC32 lookup table at compile time
const fn generate_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// Calculate CRC32 checksum
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[index];
    }
    !crc
}

/// BIM type identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BimType {
    Unknown = 0x00,
    Normandy = 0x01,
    SyrosV2 = 0x02,
    Aurora = 0x03,
    Iliad = 0x04,
    // Add more as needed
}

impl BimType {
    /// Convert from u8
    pub fn from_u8(val: u8) -> Self {
        match val {
            0x01 => BimType::Normandy,
            0x02 => BimType::SyrosV2,
            0x03 => BimType::Aurora,
            0x04 => BimType::Iliad,
            _ => BimType::Unknown,
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            BimType::Unknown => "Unknown",
            BimType::Normandy => "Normandy",
            BimType::SyrosV2 => "Syros v2",
            BimType::Aurora => "Aurora",
            BimType::Iliad => "Iliad",
        }
    }
}

/// Power rail configuration
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct RailConfig {
    /// Nominal voltage in millivolts
    pub voltage_mv: u16,
    /// Maximum safe voltage in millivolts
    pub max_voltage_mv: u16,
    /// Minimum safe voltage in millivolts
    pub min_voltage_mv: u16,
    /// Maximum current in milliamps
    pub max_current_ma: u16,
}

impl RailConfig {
    pub const fn disabled() -> Self {
        Self {
            voltage_mv: 0,
            max_voltage_mv: 0,
            min_voltage_mv: 0,
            max_current_ma: 0,
        }
    }
}

/// BIM EEPROM data structure (256 bytes total)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct BimEeprom {
    // ===== HEADER (16 bytes) =====
    /// Magic number: 0xBEEFCAFE indicates programmed EEPROM
    pub magic: u32,

    /// EEPROM format version (current: 1)
    pub version: u8,

    /// BIM type identifier
    pub bim_type: u8,

    /// Hardware revision
    pub hw_revision: u8,

    /// Reserved byte (was incorrectly documented as slot number)
    /// Note: Rack position is determined by Ethernet cable, not stored in EEPROM.
    /// BIMs can move between slots; use `bim_asset_id` for BIM identification.
    pub _reserved_header: u8,

    /// Unique serial number (board DNA or assigned serial)
    pub serial_number: u32,

    /// Manufacturing date (Unix timestamp)
    pub manufacture_date: u32,

    // ===== POWER RAIL CONFIG (64 bytes) =====
    /// LCPS rails (VOUT1-8 on J6)
    pub lcps_rails: [RailConfig; 8],  // 8 * 8 = 64 bytes

    // ===== CALIBRATION DATA (64 bytes) =====
    /// Voltage calibration offset in mV (signed)
    pub voltage_cal: [i16; 16],  // 16 * 2 = 32 bytes

    /// Current calibration offset in mA (signed)
    pub current_cal: [i16; 16],  // 16 * 2 = 32 bytes

    // ===== DUT METADATA (96 bytes) =====
    /// DUT vendor name (null-terminated ASCII)
    pub dut_vendor: [u8; 32],

    /// DUT part number (null-terminated ASCII)
    pub dut_part_number: [u8; 32],

    /// DUT description (null-terminated ASCII)
    pub dut_description: [u8; 32],

    // ===== STATISTICS (8 bytes) =====
    /// Number of times reprogrammed
    pub program_count: u16,

    /// BIM asset ID / inventory tag (null-terminated, e.g., "BIM-042")
    pub bim_asset_id: [u8; 6],

    // ===== CHECKSUM (4 bytes) =====
    /// CRC32 of bytes 0-247 (everything except checksum itself)
    pub checksum: u32,

    // ===== RESERVED (4 bytes) =====
    pub _reserved3: [u8; 4],
}

// Compile-time assertion that BimEeprom is exactly 256 bytes
const _: () = assert!(core::mem::size_of::<BimEeprom>() == 256);

impl BimEeprom {
    /// Magic number for valid EEPROM
    pub const MAGIC: u32 = 0xBEEF_CAFE;

    /// Current EEPROM format version
    pub const VERSION: u8 = 1;

    /// Create empty (unprogrammed) EEPROM
    pub const fn empty() -> Self {
        Self {
            magic: 0xFFFFFFFF,  // Unprogrammed EEPROM reads all 1s
            version: 0xFF,
            bim_type: 0xFF,
            hw_revision: 0xFF,
            _reserved_header: 0,
            serial_number: 0xFFFFFFFF,
            manufacture_date: 0xFFFFFFFF,
            lcps_rails: [RailConfig::disabled(); 8],
            voltage_cal: [0; 16],
            current_cal: [0; 16],
            dut_vendor: [0xFF; 32],
            dut_part_number: [0xFF; 32],
            dut_description: [0xFF; 32],
            program_count: 0,
            bim_asset_id: [0; 6],  // Empty string
            checksum: 0xFFFFFFFF,
            _reserved3: [0xFF; 4],
        }
    }

    /// Check if EEPROM is programmed
    pub fn is_programmed(&self) -> bool {
        self.magic == Self::MAGIC
    }

    /// Check if EEPROM is blank (all 0xFF)
    pub fn is_blank(&self) -> bool {
        self.magic == 0xFFFFFFFF
    }

    /// Calculate CRC32 checksum of EEPROM data
    pub fn calculate_crc32(&self) -> u32 {
        // Calculate CRC of everything except the checksum field itself
        let bytes = unsafe {
            core::slice::from_raw_parts(
                self as *const _ as *const u8,
                core::mem::size_of::<Self>() - 8  // Exclude checksum + reserved
            )
        };
        crc32(bytes)
    }

    /// Verify checksum
    pub fn verify_checksum(&self) -> bool {
        self.checksum == self.calculate_crc32()
    }

    /// Update checksum field
    pub fn update_checksum(&mut self) {
        self.checksum = self.calculate_crc32();
    }

    /// Get BIM type enum
    pub fn get_bim_type(&self) -> BimType {
        BimType::from_u8(self.bim_type)
    }

    /// Convert to byte array for EEPROM writing
    pub fn to_bytes(&self) -> &[u8; EEPROM_SIZE] {
        unsafe { &*(self as *const _ as *const [u8; EEPROM_SIZE]) }
    }

    /// Parse from byte array
    pub fn from_bytes(bytes: &[u8; EEPROM_SIZE]) -> &Self {
        unsafe { &*(bytes.as_ptr() as *const Self) }
    }

    /// Parse from byte array (mutable)
    pub fn from_bytes_mut(bytes: &mut [u8; EEPROM_SIZE]) -> &mut Self {
        unsafe { &mut *(bytes.as_mut_ptr() as *mut Self) }
    }

    /// Validate EEPROM data
    pub fn validate(&self) -> Result<(), EepromError> {
        if !self.is_programmed() {
            return Err(EepromError::InvalidMagic);
        }
        if !self.verify_checksum() {
            return Err(EepromError::ChecksumMismatch);
        }
        Ok(())
    }

    /// Get DUT vendor as string slice
    pub fn get_vendor(&self) -> &str {
        // Find null terminator or end of array
        let len = self.dut_vendor.iter().position(|&c| c == 0).unwrap_or(32);
        core::str::from_utf8(&self.dut_vendor[..len]).unwrap_or("???")
    }

    /// Get DUT part number as string slice
    pub fn get_part_number(&self) -> &str {
        let len = self.dut_part_number.iter().position(|&c| c == 0).unwrap_or(32);
        core::str::from_utf8(&self.dut_part_number[..len]).unwrap_or("???")
    }

    /// Get DUT description as string slice
    pub fn get_description(&self) -> &str {
        let len = self.dut_description.iter().position(|&c| c == 0).unwrap_or(32);
        core::str::from_utf8(&self.dut_description[..len]).unwrap_or("???")
    }

    /// Get BIM asset ID as string slice (e.g., "BIM-042")
    ///
    /// This is the physical inventory tag that travels with the BIM board.
    /// Rack position is determined by Ethernet cable connection, not stored here.
    pub fn get_asset_id(&self) -> &str {
        let len = self.bim_asset_id.iter().position(|&c| c == 0).unwrap_or(6);
        core::str::from_utf8(&self.bim_asset_id[..len]).unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32() {
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
        assert_eq!(crc32(b""), 0x00000000);
        assert_eq!(crc32(b"hello"), 0x3610A686);
    }

    #[test]
    fn test_bim_eeprom_size() {
        assert_eq!(core::mem::size_of::<BimEeprom>(), 256);
    }

    #[test]
    fn test_bim_eeprom_empty() {
        let eeprom = BimEeprom::empty();
        assert!(!eeprom.is_programmed());
        assert!(eeprom.is_blank());
    }
}
