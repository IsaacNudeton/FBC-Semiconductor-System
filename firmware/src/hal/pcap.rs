//! PCAP (Processor Configuration Access Port)
//!
//! Controls FPGA configuration from the ARM processor.
//! Used for programming partial or full bitstreams.
//!
//! # ONETWO Design
//!
//! Invariant: DEVCFG register addresses, programming sequence, DMA format
//! Varies: Bitstream data, configuration options
//! Pattern: unlock → reset PL → DMA bitstream → wait DONE → lock
//!
//! # Programming Flow
//!
//! 1. Unlock SLCR and DEVCFG
//! 2. Assert PROG_B (reset PL)
//! 3. Wait for INIT_B to go high
//! 4. Transfer bitstream via DMA
//! 5. Wait for DONE signal
//! 6. Lock SLCR

use super::{Reg, Register, delay_us, delay_ms};

/// DEVCFG base address
const DEVCFG_BASE: usize = 0xF800_7000;

/// SLCR base address (for unlock/reset)
const SLCR_BASE: usize = 0xF800_0000;

/// DEVCFG register offsets
mod regs {
    pub const CTRL: usize = 0x00;        // Control
    pub const LOCK: usize = 0x04;        // Lock
    pub const CFG: usize = 0x08;         // Configuration
    pub const INT_STS: usize = 0x0C;     // Interrupt Status
    pub const INT_MASK: usize = 0x10;    // Interrupt Mask
    pub const STATUS: usize = 0x14;      // Status
    pub const DMA_SRC_ADDR: usize = 0x18; // DMA Source Address
    pub const DMA_DST_ADDR: usize = 0x1C; // DMA Destination Address
    pub const DMA_SRC_LEN: usize = 0x20;  // DMA Source Length
    pub const DMA_DST_LEN: usize = 0x24;  // DMA Dest Length
    pub const ROM_SHADOW: usize = 0x28;  // ROM Shadow
    pub const MULTIBOOT: usize = 0x2C;   // Multiboot Address
    pub const SW_ID: usize = 0x30;       // Software ID
    pub const UNLOCK: usize = 0x34;      // Unlock
    pub const MCTRL: usize = 0x80;       // Miscellaneous Control
}

/// Control register bits
mod ctrl {
    pub const FORCE_RST: u32 = 1 << 31;      // Force reset
    pub const PCFG_PROG_B: u32 = 1 << 30;    // Program B signal
    pub const PCFG_POR_CNT_4K: u32 = 1 << 29;
    pub const PCAP_PR: u32 = 1 << 27;        // PCAP mode read
    pub const PCAP_MODE: u32 = 1 << 26;      // PCAP mode
    pub const MULTIBOOT_EN: u32 = 1 << 24;
    pub const USER_MODE: u32 = 1 << 15;
    pub const PCFG_AES_FUSE: u32 = 1 << 12;  // AES key from fuses
    pub const PCFG_AES_EN: u32 = 0x7 << 9;   // AES encryption
    pub const SEU_EN: u32 = 1 << 8;          // SEU detection
    pub const SEC_EN: u32 = 1 << 7;          // Security enable
    pub const SPNIDEN: u32 = 1 << 6;
    pub const SPIDEN: u32 = 1 << 5;
    pub const NIDEN: u32 = 1 << 4;
    pub const DBGEN: u32 = 1 << 3;
    pub const DAP_EN: u32 = 0x7 << 0;        // DAP enable
}

/// Status register bits
mod status {
    pub const DMA_CMD_Q_F: u32 = 1 << 31;    // DMA command queue full
    pub const DMA_CMD_Q_E: u32 = 1 << 30;    // DMA command queue empty
    pub const DMA_DONE_CNT_MASK: u32 = 0x3 << 28;
    pub const RX_FIFO_LVL_MASK: u32 = 0x1F << 20;
    pub const TX_FIFO_LVL_MASK: u32 = 0x7F << 12;
    pub const PSS_GTS_USR_B: u32 = 1 << 11;
    pub const PSS_FST_CFG_B: u32 = 1 << 10;
    pub const PSS_GPWRDWN_B: u32 = 1 << 9;
    pub const PSS_GTS_CFG_B: u32 = 1 << 8;
    pub const ILL_APB_ACCE: u32 = 1 << 6;    // Illegal APB access
    pub const PSS_CFG_RESET_B: u32 = 1 << 5;
    pub const PCFG_INIT: u32 = 1 << 4;       // INIT signal (from PL)
    pub const EFUSE_BBRAM_KEY_DIS: u32 = 1 << 3;
    pub const EFUSE_SEC_EN: u32 = 1 << 2;
    pub const EFUSE_JTAG_DIS: u32 = 1 << 1;
    pub const PCFG_DONE: u32 = 1 << 0;       // DONE signal (from PL)
}

/// Interrupt status bits
mod int_sts {
    pub const PSS_CFG_RESET_B: u32 = 1 << 27;
    pub const AXI_WTO: u32 = 1 << 23;        // AXI write timeout
    pub const AXI_WERR: u32 = 1 << 22;       // AXI write error
    pub const AXI_RTO: u32 = 1 << 21;        // AXI read timeout
    pub const AXI_RERR: u32 = 1 << 20;       // AXI read error
    pub const RX_FIFO_OV: u32 = 1 << 18;     // RX FIFO overflow
    pub const WR_FIFO_LVL: u32 = 1 << 17;    // Write FIFO level
    pub const RD_FIFO_LVL: u32 = 1 << 16;    // Read FIFO level
    pub const DMA_CMD_ERR: u32 = 1 << 15;    // DMA command error
    pub const DMA_Q_OV: u32 = 1 << 14;       // DMA queue overflow
    pub const DMA_DONE: u32 = 1 << 13;       // DMA done
    pub const D_P_DONE: u32 = 1 << 12;       // D_P done
    pub const P2D_LEN_ERR: u32 = 1 << 11;    // P2D length error
    pub const PCFG_HMAC_ERR: u32 = 1 << 6;   // HMAC error
    pub const PCFG_SEU_ERR: u32 = 1 << 5;    // SEU error
    pub const PCFG_POR_B: u32 = 1 << 4;
    pub const PCFG_CFG_RST: u32 = 1 << 3;
    pub const PCFG_DONE: u32 = 1 << 2;       // FPGA programming done
    pub const PCFG_INIT_PE: u32 = 1 << 1;    // INIT positive edge
    pub const PCFG_INIT_NE: u32 = 1 << 0;    // INIT negative edge
}

/// Unlock keys
const UNLOCK_KEY: u32 = 0x757BDF0D;
const SLCR_UNLOCK_KEY: u32 = 0xDF0D;
const SLCR_LOCK_KEY: u32 = 0x767B;

/// SLCR register offsets
mod slcr_regs {
    pub const LOCK: usize = 0x004;
    pub const UNLOCK: usize = 0x008;
    pub const FPGA_RST_CTRL: usize = 0x240;
    pub const LVL_SHFTR_EN: usize = 0x900;
}

/// PCAP error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcapError {
    /// FPGA didn't respond to reset
    InitTimeout,
    /// DMA transfer failed
    DmaError,
    /// Configuration failed (DONE not asserted)
    ConfigFailed,
    /// Bitstream too large
    BitstreamTooLarge,
    /// HMAC verification failed
    HmacError,
    /// SEU (Single Event Upset) detected
    SeuError,
}

/// PCAP controller for FPGA programming
pub struct Pcap {
    base: Reg,
    slcr: Reg,
}

impl Pcap {
    /// Create PCAP instance
    pub const fn new() -> Self {
        Self {
            base: Reg::new(DEVCFG_BASE),
            slcr: Reg::new(SLCR_BASE),
        }
    }

    /// Unlock SLCR
    fn slcr_unlock(&self) {
        self.slcr.offset(slcr_regs::UNLOCK).write(SLCR_UNLOCK_KEY);
    }

    /// Lock SLCR
    fn slcr_lock(&self) {
        self.slcr.offset(slcr_regs::LOCK).write(SLCR_LOCK_KEY);
    }

    /// Unlock DEVCFG
    fn unlock(&self) {
        self.base.offset(regs::UNLOCK).write(UNLOCK_KEY);
    }

    /// Initialize PCAP
    pub fn init(&self) {
        // Unlock DEVCFG
        self.unlock();

        // Enable PCAP mode
        self.base.offset(regs::CTRL).modify(|v| v | ctrl::PCAP_MODE);

        // Clear all interrupts
        self.base.offset(regs::INT_STS).write(0xFFFFFFFF);

        // Enable level shifters
        self.slcr_unlock();
        self.slcr.offset(slcr_regs::LVL_SHFTR_EN).write(0xF);
        self.slcr_lock();
    }

    /// Check if FPGA is configured (DONE = 1)
    pub fn is_configured(&self) -> bool {
        self.base.offset(regs::STATUS).read() & status::PCFG_DONE != 0
    }

    /// Check if INIT is high (ready for programming)
    pub fn is_init_high(&self) -> bool {
        self.base.offset(regs::STATUS).read() & status::PCFG_INIT != 0
    }

    /// Get status register
    pub fn get_status(&self) -> u32 {
        self.base.offset(regs::STATUS).read()
    }

    /// Reset the FPGA (assert PROG_B)
    pub fn reset_fpga(&self) -> Result<(), PcapError> {
        // Clear DONE status
        self.base.offset(regs::INT_STS).write(int_sts::PCFG_DONE);

        // Hold FPGA in reset via SLCR
        self.slcr_unlock();
        self.slcr.offset(slcr_regs::FPGA_RST_CTRL).write(0x1);  // Assert reset
        self.slcr_lock();

        delay_ms(1);

        // Assert PCFG_PROG_B (active low internally)
        self.base.offset(regs::CTRL).modify(|v| v & !ctrl::PCFG_PROG_B);

        delay_us(100);

        // Deassert PCFG_PROG_B
        self.base.offset(regs::CTRL).modify(|v| v | ctrl::PCFG_PROG_B);

        // Wait for INIT to go high
        for _ in 0..10000 {
            if self.is_init_high() {
                return Ok(());
            }
            delay_us(10);
        }

        Err(PcapError::InitTimeout)
    }

    /// Program FPGA with bitstream
    ///
    /// # Arguments
    /// * `bitstream` - Pointer to bitstream data (must be word-aligned)
    /// * `length` - Length in bytes
    ///
    /// # Safety
    /// The bitstream pointer must be valid and the data must remain valid
    /// during the DMA transfer.
    pub fn program(&self, bitstream: *const u8, length: usize) -> Result<(), PcapError> {
        // Must be word-aligned
        if bitstream as usize & 0x3 != 0 {
            return Err(PcapError::DmaError);
        }

        // Reset FPGA first
        self.reset_fpga()?;

        // Clear any pending interrupts
        self.base.offset(regs::INT_STS).write(0xFFFFFFFF);

        // Set DMA source address
        self.base.offset(regs::DMA_SRC_ADDR).write(bitstream as u32);

        // Set DMA destination (PCAP FIFO = 0xFFFFFFFF)
        self.base.offset(regs::DMA_DST_ADDR).write(0xFFFFFFFF);

        // Set length (in words)
        let words = (length + 3) / 4;
        self.base.offset(regs::DMA_SRC_LEN).write(words as u32);
        self.base.offset(regs::DMA_DST_LEN).write(0);

        // Wait for DMA done
        for _ in 0..1_000_000 {
            let int_sts = self.base.offset(regs::INT_STS).read();

            if int_sts & int_sts::DMA_DONE != 0 {
                // DMA complete, check for errors
                if int_sts & (int_sts::AXI_WERR | int_sts::AXI_RERR | int_sts::DMA_CMD_ERR) != 0 {
                    return Err(PcapError::DmaError);
                }
                break;
            }
            delay_us(10);
        }

        // Wait for DONE signal
        for _ in 0..100_000 {
            if self.is_configured() {
                // Release FPGA from reset
                self.slcr_unlock();
                self.slcr.offset(slcr_regs::FPGA_RST_CTRL).write(0x0);
                self.slcr_lock();

                return Ok(());
            }
            delay_us(10);
        }

        Err(PcapError::ConfigFailed)
    }

    /// Program FPGA from slice
    pub fn program_slice(&self, bitstream: &[u8]) -> Result<(), PcapError> {
        self.program(bitstream.as_ptr(), bitstream.len())
    }

    /// Read configuration data back from FPGA
    ///
    /// # Arguments
    /// * `buffer` - Buffer to store readback data (must be word-aligned)
    /// * `length` - Number of bytes to read
    pub fn readback(&self, buffer: *mut u8, length: usize) -> Result<(), PcapError> {
        // Must be word-aligned
        if buffer as usize & 0x3 != 0 {
            return Err(PcapError::DmaError);
        }

        // Enable PCAP read mode
        self.base.offset(regs::CTRL).modify(|v| v | ctrl::PCAP_PR);

        // Clear interrupts
        self.base.offset(regs::INT_STS).write(0xFFFFFFFF);

        // Set DMA source (PCAP FIFO = 0xFFFFFFFF)
        self.base.offset(regs::DMA_SRC_ADDR).write(0xFFFFFFFF);

        // Set DMA destination
        self.base.offset(regs::DMA_DST_ADDR).write(buffer as u32);

        // Set length (in words)
        let words = (length + 3) / 4;
        self.base.offset(regs::DMA_SRC_LEN).write(0);
        self.base.offset(regs::DMA_DST_LEN).write(words as u32);

        // Wait for DMA done
        for _ in 0..1_000_000 {
            let int_sts = self.base.offset(regs::INT_STS).read();

            if int_sts & int_sts::DMA_DONE != 0 {
                if int_sts & (int_sts::AXI_WERR | int_sts::AXI_RERR) != 0 {
                    return Err(PcapError::DmaError);
                }
                break;
            }
            delay_us(10);
        }

        // Disable read mode
        self.base.offset(regs::CTRL).modify(|v| v & !ctrl::PCAP_PR);

        Ok(())
    }

    /// Get interrupt status (for debugging)
    pub fn get_interrupt_status(&self) -> u32 {
        self.base.offset(regs::INT_STS).read()
    }

    /// Clear all interrupts
    pub fn clear_interrupts(&self) {
        self.base.offset(regs::INT_STS).write(0xFFFFFFFF);
    }
}
