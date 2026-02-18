//! SPI Master Driver
//!
//! Supports Zynq SPI0 and SPI1 peripherals.
//! Low-level SPI communication - device-specific drivers (ADC/DAC) built on top.

use super::{Reg, Register, delay_us};

/// SPI0 base address
const SPI0_BASE: usize = 0xE000_6000;
/// SPI1 base address
const SPI1_BASE: usize = 0xE000_7000;

/// SPI register offsets
mod regs {
    pub const CR: usize = 0x00;       // Config Register
    pub const ISR: usize = 0x04;      // Interrupt Status
    pub const IER: usize = 0x08;      // Interrupt Enable
    pub const IDR: usize = 0x0C;      // Interrupt Disable
    pub const IMR: usize = 0x10;      // Interrupt Mask
    pub const ER: usize = 0x14;       // Enable Register
    pub const DR: usize = 0x18;       // Delay Register
    pub const TXD: usize = 0x1C;      // TX Data
    pub const RXD: usize = 0x20;      // RX Data
    pub const SICR: usize = 0x24;     // Slave Idle Count
    pub const TXWR: usize = 0x28;     // TX Watermark
    pub const RXWR: usize = 0x2C;     // RX Watermark (not in all versions)
}

/// Config Register bits
mod cr {
    pub const MODFAIL_GEN_EN: u32 = 1 << 17;
    pub const MAN_START_COM: u32 = 1 << 16;
    pub const MAN_START_EN: u32 = 1 << 15;
    pub const MAN_CS: u32 = 1 << 14;
    pub const CS_MASK: u32 = 0xF << 10;
    pub const CS_SHIFT: u32 = 10;
    pub const PERI_SEL: u32 = 1 << 9;
    pub const REF_CLK: u32 = 1 << 8;
    pub const BAUD_MASK: u32 = 0x7 << 3;
    pub const BAUD_SHIFT: u32 = 3;
    pub const CLK_PH: u32 = 1 << 2;   // Clock phase
    pub const CLK_POL: u32 = 1 << 1;  // Clock polarity
    pub const MODE_SEL: u32 = 1 << 0; // 0=slave, 1=master
}

/// Interrupt Status bits
mod isr {
    pub const TX_FIFO_UNDERFLOW: u32 = 1 << 6;
    pub const RX_FIFO_FULL: u32 = 1 << 5;
    pub const RX_FIFO_NOT_EMPTY: u32 = 1 << 4;
    pub const TX_FIFO_FULL: u32 = 1 << 3;
    pub const TX_FIFO_NOT_FULL: u32 = 1 << 2;
    pub const MODE_FAIL: u32 = 1 << 1;
    pub const RX_OVERFLOW: u32 = 1 << 0;
}

/// SPI error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpiError {
    /// TX FIFO overflow
    TxOverflow,
    /// RX FIFO overflow
    RxOverflow,
    /// Timeout
    Timeout,
    /// Mode fault
    ModeFault,
}

/// SPI mode (clock polarity and phase)
#[derive(Clone, Copy)]
pub enum SpiMode {
    Mode0,  // CPOL=0, CPHA=0
    Mode1,  // CPOL=0, CPHA=1
    Mode2,  // CPOL=1, CPHA=0
    Mode3,  // CPOL=1, CPHA=1
}

/// SPI instance
pub struct Spi {
    base: Reg,
}

impl Spi {
    /// Create SPI0 instance
    pub const fn spi0() -> Self {
        Self { base: Reg::new(SPI0_BASE) }
    }

    /// Create SPI1 instance
    pub const fn spi1() -> Self {
        Self { base: Reg::new(SPI1_BASE) }
    }

    /// Initialize SPI peripheral
    ///
    /// # Arguments
    /// * `mode` - SPI mode (clock polarity/phase)
    /// * `baud_div` - Baud rate divisor (0-7, divides by 4, 8, 16, 32, 64, 128, 256)
    pub fn init(&self, mode: SpiMode, baud_div: u8) {
        // Disable first
        self.base.offset(regs::ER).write(0);

        // Configure: master mode, manual CS
        let mut config = cr::MODE_SEL | cr::MAN_CS | cr::MAN_START_EN;

        // Set baud rate
        config |= ((baud_div as u32) & 0x7) << cr::BAUD_SHIFT;

        // Set mode
        match mode {
            SpiMode::Mode0 => {}  // CPOL=0, CPHA=0
            SpiMode::Mode1 => config |= cr::CLK_PH,
            SpiMode::Mode2 => config |= cr::CLK_POL,
            SpiMode::Mode3 => config |= cr::CLK_POL | cr::CLK_PH,
        }

        // All CS lines high (inactive) initially
        config |= 0xF << cr::CS_SHIFT;

        self.base.offset(regs::CR).write(config);

        // Clear interrupts
        self.base.offset(regs::ISR).write(0x7F);

        // Enable
        self.base.offset(regs::ER).write(1);
    }

    /// Select chip (assert CS)
    ///
    /// # Arguments
    /// * `cs` - Chip select line (0-3)
    pub fn select(&self, cs: u8) {
        let cs = cs & 0x3;
        let mask = !(1u32 << cs) & 0xF;  // Active low
        self.base.offset(regs::CR).modify(|v| (v & !cr::CS_MASK) | (mask << cr::CS_SHIFT));
    }

    /// Deselect all chips
    pub fn deselect(&self) {
        self.base.offset(regs::CR).modify(|v| v | cr::CS_MASK);
    }

    /// Transfer a single byte
    pub fn transfer_byte(&self, tx: u8) -> Result<u8, SpiError> {
        // Wait for TX FIFO not full
        for _ in 0..10000 {
            if self.base.offset(regs::ISR).read() & isr::TX_FIFO_NOT_FULL != 0 {
                break;
            }
            delay_us(1);
        }

        // Write TX data
        self.base.offset(regs::TXD).write(tx as u32);

        // Start transfer
        self.base.offset(regs::CR).set_bits(cr::MAN_START_COM);

        // Wait for RX data
        for _ in 0..10000 {
            if self.base.offset(regs::ISR).read() & isr::RX_FIFO_NOT_EMPTY != 0 {
                break;
            }
            delay_us(1);
        }

        // Check for errors
        let status = self.base.offset(regs::ISR).read();
        if status & isr::RX_OVERFLOW != 0 {
            self.base.offset(regs::ISR).write(isr::RX_OVERFLOW);
            return Err(SpiError::RxOverflow);
        }

        Ok(self.base.offset(regs::RXD).read() as u8)
    }

    /// Transfer multiple bytes
    pub fn transfer(&self, tx: &[u8], rx: &mut [u8]) -> Result<(), SpiError> {
        let len = tx.len().min(rx.len());
        for i in 0..len {
            rx[i] = self.transfer_byte(tx[i])?;
        }
        Ok(())
    }

    /// Write bytes (ignore received data)
    pub fn write(&self, data: &[u8]) -> Result<(), SpiError> {
        for &byte in data {
            let _ = self.transfer_byte(byte)?;
        }
        Ok(())
    }

    /// Read bytes (send zeros)
    pub fn read(&self, buf: &mut [u8]) -> Result<(), SpiError> {
        for byte in buf.iter_mut() {
            *byte = self.transfer_byte(0x00)?;
        }
        Ok(())
    }

}
