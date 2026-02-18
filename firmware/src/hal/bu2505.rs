//! BU2505FV 10-Channel 10-bit DAC Driver
//!
//! ROHM BU2505FV-E2 on SPI0, CS0
//!
//! Protocol (INVARIANT from datasheet):
//! - 14-bit SPI frame: [ADDR(4)][DATA(10)]
//! - 10 channels (0-9)
//! - Address 0xF = broadcast to all channels

use super::{Spi, SpiError};

/// DAC chip select (INVARIANT: hardware wiring)
const DAC_CS: u8 = 0;

/// Number of channels
pub const NUM_CHANNELS: usize = 10;

/// Maximum 10-bit value
const MAX_VALUE: u16 = 1023;

/// BU2505FV DAC driver
pub struct Bu2505<'a> {
    spi: &'a Spi,
    vref_mv: u16,
}

impl<'a> Bu2505<'a> {
    /// Create new DAC driver
    ///
    /// # Arguments
    /// * `spi` - SPI0 peripheral reference
    /// * `vref_mv` - Reference voltage in millivolts (typically 4096 from LM4132)
    pub fn new(spi: &'a Spi, vref_mv: u16) -> Self {
        Self { spi, vref_mv }
    }

    /// Initialize DAC
    ///
    /// Sets all channels to 0V output
    pub fn init(&self) -> Result<(), SpiError> {
        // Set all channels to 0
        self.set_all_raw(0)?;
        Ok(())
    }

    /// Set channel to raw 10-bit value
    ///
    /// # Arguments
    /// * `ch` - Channel 0-9
    /// * `value` - 10-bit value (0-1023)
    pub fn set_raw(&self, ch: u8, value: u16) -> Result<(), SpiError> {
        if ch >= NUM_CHANNELS as u8 {
            return Ok(()); // Ignore invalid channel
        }

        let value = value.min(MAX_VALUE);

        // Frame format: [ADDR(4)][DATA(10)]
        // Bits 13-10: Address
        // Bits 9-0: Data
        let frame = ((ch as u16) << 10) | value;

        self.write_frame(frame)
    }

    /// Set channel to voltage in millivolts
    ///
    /// # Arguments
    /// * `ch` - Channel 0-9
    /// * `mv` - Voltage in millivolts (0 to vref_mv)
    pub fn set_voltage_mv(&self, ch: u8, mv: u16) -> Result<(), SpiError> {
        let mv = mv.min(self.vref_mv);

        // DATA = (Vout / Vref) × 1023
        let value = ((mv as u32) * (MAX_VALUE as u32) / (self.vref_mv as u32)) as u16;

        self.set_raw(ch, value)
    }

    /// Set all channels to same raw value
    ///
    /// Uses broadcast address (0xF)
    pub fn set_all_raw(&self, value: u16) -> Result<(), SpiError> {
        let value = value.min(MAX_VALUE);

        // Broadcast address = 0xF
        let frame = (0xF_u16 << 10) | value;

        self.write_frame(frame)
    }

    /// Set multiple channels at once
    ///
    /// # Arguments
    /// * `values` - Array of 10 raw values, one per channel
    pub fn set_channels_raw(&self, values: &[u16; NUM_CHANNELS]) -> Result<(), SpiError> {
        for (ch, &value) in values.iter().enumerate() {
            self.set_raw(ch as u8, value)?;
        }
        Ok(())
    }

    /// Read back current reference voltage setting
    pub fn vref_mv(&self) -> u16 {
        self.vref_mv
    }

    /// Convert millivolts to raw DAC value
    pub fn mv_to_raw(&self, mv: u16) -> u16 {
        let mv = mv.min(self.vref_mv);
        ((mv as u32) * (MAX_VALUE as u32) / (self.vref_mv as u32)) as u16
    }

    /// Convert raw DAC value to millivolts
    pub fn raw_to_mv(&self, raw: u16) -> u16 {
        let raw = raw.min(MAX_VALUE);
        ((raw as u32) * (self.vref_mv as u32) / (MAX_VALUE as u32)) as u16
    }

    /// Write 14-bit frame to DAC
    fn write_frame(&self, frame: u16) -> Result<(), SpiError> {
        self.spi.select(DAC_CS);

        // Send MSB first (14 bits in 16-bit transfer)
        // DAC expects: [0][0][A3][A2][A1][A0][D9][D8] [D7][D6][D5][D4][D3][D2][D1][D0]
        let msb = (frame >> 8) as u8;
        let lsb = (frame & 0xFF) as u8;

        let result = self.spi.write(&[msb, lsb]);

        self.spi.deselect();

        result
    }
}
