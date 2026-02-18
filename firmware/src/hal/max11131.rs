//! MAX11131 16-Channel 12-bit ADC Driver
//!
//! Maxim/Analog Devices MAX11131ATI+T on SPI0, CS1
//!
//! Protocol (INVARIANT from datasheet):
//! - 16-bit SPI frames
//! - Register select in bits 15:11
//! - 16 single-ended channels (AIN0-AIN15)
//! - 12-bit results with optional channel ID

use super::{Spi, SpiError, delay_us};

/// ADC chip select (INVARIANT: hardware wiring)
const ADC_CS: u8 = 1;

/// Number of channels
pub const NUM_CHANNELS: usize = 16;

/// Register addresses (bits 15:11)
mod reg {
    pub const ADC_MODE_CONTROL: u16 = 0b00000 << 11;
    pub const ADC_CONFIGURATION: u16 = 0b00001 << 11;
    pub const UNIPOLAR: u16 = 0b00010 << 11;
    pub const BIPOLAR: u16 = 0b00011 << 11;
    pub const RANGE: u16 = 0b00100 << 11;
    pub const CSCAN0: u16 = 0b10100 << 11;  // Custom scan AIN8-15
    pub const CSCAN1: u16 = 0b10101 << 11;  // Custom scan AIN0-7
}

/// ADC_MODE_CONTROL scan modes (bits 10:7)
mod scan {
    pub const MANUAL: u16 = 0b0001 << 7;      // Single channel, manual trigger
    pub const REPEAT: u16 = 0b0010 << 7;      // Repeat same channel
    pub const STANDARD_INT: u16 = 0b0011 << 7; // Scan 0→N, internal clock
    pub const STANDARD_EXT: u16 = 0b0100 << 7; // Scan 0→N, external clock
    pub const UPPER_INT: u16 = 0b0101 << 7;   // Scan N→15, internal clock
    pub const UPPER_EXT: u16 = 0b0110 << 7;   // Scan N→15, external clock
    pub const CUSTOM_INT: u16 = 0b0111 << 7;  // Custom mask, internal clock
    pub const CUSTOM_EXT: u16 = 0b1000 << 7;  // Custom mask, external clock
}

/// ADC_CONFIGURATION bits
mod config {
    pub const REFSEL_INT: u16 = 0 << 10;      // Internal reference
    pub const REFSEL_EXT: u16 = 1 << 10;      // External reference
    pub const AVGON: u16 = 1 << 9;            // Averaging enable
    pub const NAVG_4: u16 = 0b00 << 7;        // 4 samples
    pub const NAVG_8: u16 = 0b01 << 7;        // 8 samples
    pub const NAVG_16: u16 = 0b10 << 7;       // 16 samples
    pub const NAVG_32: u16 = 0b11 << 7;       // 32 samples
    pub const NSCAN_4: u16 = 0b00 << 5;       // 4 results per scan
    pub const NSCAN_8: u16 = 0b01 << 5;       // 8 results per scan
    pub const NSCAN_12: u16 = 0b10 << 5;      // 12 results per scan
    pub const NSCAN_16: u16 = 0b11 << 5;      // 16 results per scan
    pub const RESET: u16 = 1 << 0;            // Reset to power-up state
}

/// MAX11131 ADC driver
pub struct Max11131<'a> {
    spi: &'a Spi,
}

impl<'a> Max11131<'a> {
    /// Create new ADC driver
    pub fn new(spi: &'a Spi) -> Self {
        Self { spi }
    }

    /// Initialize ADC
    ///
    /// Configures for:
    /// - External reference
    /// - No averaging (fastest)
    /// - 16 results per scan
    /// - All channels enabled for custom scan
    pub fn init(&self) -> Result<(), SpiError> {
        // Reset to known state
        self.write_reg(reg::ADC_CONFIGURATION | config::RESET)?;
        delay_us(100);

        // Configure: external ref, no averaging, 16 results
        let cfg = reg::ADC_CONFIGURATION
            | config::REFSEL_EXT
            | config::NSCAN_16;
        self.write_reg(cfg)?;

        // Enable all channels in custom scan registers
        // CSCAN0: AIN8-15 (bits 7:0 = channels 15:8)
        self.write_reg(reg::CSCAN0 | 0xFF)?;

        // CSCAN1: AIN0-7 (bits 7:0 = channels 7:0)
        self.write_reg(reg::CSCAN1 | 0xFF)?;

        // Set all channels to unipolar mode
        self.write_reg(reg::UNIPOLAR | 0xFFFF)?;

        Ok(())
    }

    /// Read all 16 channels using Custom Scan mode
    ///
    /// Returns array of 12-bit raw values, indexed by channel
    pub fn read_all(&self) -> Result<[u16; NUM_CHANNELS], SpiError> {
        let mut results = [0u16; NUM_CHANNELS];

        // Start custom scan with external clock
        let cmd = reg::ADC_MODE_CONTROL | scan::CUSTOM_EXT | (15 << 3); // CHSEL=15
        self.write_reg(cmd)?;

        // Read 16 results
        // Each read returns: [CHAN_ID(4)][DATA(12)]
        for _ in 0..NUM_CHANNELS {
            let raw = self.read_result()?;

            // Extract channel ID (bits 15:12) and data (bits 11:0)
            let ch = ((raw >> 12) & 0x0F) as usize;
            let data = raw & 0x0FFF;

            if ch < NUM_CHANNELS {
                results[ch] = data;
            }
        }

        Ok(results)
    }

    /// Read single channel using Manual mode
    ///
    /// # Arguments
    /// * `ch` - Channel 0-15
    pub fn read_channel(&self, ch: u8) -> Result<u16, SpiError> {
        if ch >= NUM_CHANNELS as u8 {
            return Ok(0);
        }

        // Manual scan mode with channel select
        let cmd = reg::ADC_MODE_CONTROL | scan::MANUAL | ((ch as u16) << 3);
        self.write_reg(cmd)?;

        // Read result
        let raw = self.read_result()?;

        // Return 12-bit data (ignore channel ID)
        Ok(raw & 0x0FFF)
    }

    /// Convert raw 12-bit value to millivolts
    ///
    /// # Arguments
    /// * `raw` - 12-bit ADC value (0-4095)
    /// * `vref_mv` - Reference voltage in millivolts
    pub fn raw_to_mv(raw: u16, vref_mv: u16) -> u16 {
        ((raw as u32) * (vref_mv as u32) / 4096) as u16
    }

    /// Write 16-bit register value
    fn write_reg(&self, value: u16) -> Result<(), SpiError> {
        self.spi.select(ADC_CS);

        let msb = (value >> 8) as u8;
        let lsb = (value & 0xFF) as u8;

        let result = self.spi.write(&[msb, lsb]);

        self.spi.deselect();

        result
    }

    /// Read 16-bit result
    fn read_result(&self) -> Result<u16, SpiError> {
        self.spi.select(ADC_CS);

        let mut buf = [0u8; 2];
        self.spi.transfer(&[0, 0], &mut buf)?;

        self.spi.deselect();

        Ok(((buf[0] as u16) << 8) | (buf[1] as u16))
    }
}
