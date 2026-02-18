//! DDR Controller (DDRC) Driver
//!
//! Controls DDR3 memory initialization and configuration.
//!
//! # Note
//!
//! Normally, the FSBL initializes DDR before our firmware runs.
//! This module provides the ability to:
//! - Check if DDR is already initialized
//! - Re-initialize DDR if needed (JTAG debug scenario)
//! - Read DDR controller status
//!
//! # Board-Specific Values
//!
//! The timing values in this module were extracted from the Sonoma FSBL
//! and are calibrated for the specific DDR3 chips on the FBC controller boards.
//! Do NOT change these values without recalibrating for your DDR chips.

use super::{Reg, Register};

/// DDR Controller base address
const DDRC_BASE: usize = 0xF800_6000;

/// DDR Controller registers
mod regs {
    pub const CTRL: usize = 0x000;           // Main control
    pub const TWO_RANK_CFG: usize = 0x004;   // Dual rank config
    pub const HPR_REG: usize = 0x00C;        // High priority read
    pub const LPR_REG: usize = 0x010;        // Low priority read
    pub const WR_REG: usize = 0x014;         // Write config
    pub const DRAM_PARAM0: usize = 0x018;    // Timing parameters
    pub const DRAM_PARAM1: usize = 0x01C;    // Timing parameters
    pub const DRAM_PARAM2: usize = 0x020;    // Timing parameters
    pub const DRAM_PARAM3: usize = 0x028;    // Refresh/ZQ
    pub const DRAM_PARAM4: usize = 0x02C;    // Reserved
    pub const DRAM_INIT: usize = 0x030;      // Init sequence
    pub const DRAM_EMR: usize = 0x034;       // Extended mode
    pub const DRAM_EMR_MR: usize = 0x040;    // Mode register
    pub const DRAM_BURST8: usize = 0x044;    // Burst config
    pub const DRAM_DISABLE_DQ: usize = 0x050;
    pub const DRAM_ADDR_MAP_BANK: usize = 0x05C;
    pub const DRAM_ADDR_MAP_COL: usize = 0x064;
    pub const PHY_CMD_TIMEOUT: usize = 0x0A4;
    pub const PHY_CTRL_STS: usize = 0x0B8;
    pub const PHY_DLL_LOCK0: usize = 0x17C;
    pub const PHY_DLL_LOCK1: usize = 0x180;
    pub const PHY_DLL_LOCK2: usize = 0x184;
    pub const PHY_DLL_LOCK3: usize = 0x188;
    pub const ECC_SCRUB: usize = 0x200;
}

/// Board-specific DDR timing values (extracted from Sonoma FSBL)
///
/// These values are calibrated for the DDR3 chips on the FBC controller boards.
/// Changing these without proper calibration will cause DDR initialization to fail.
mod board_config {
    pub const CTRL_INIT: u32 = 0x0000_0200;      // Initial (disabled)
    pub const TWO_RANK_CFG: u32 = 0x000C_1061;
    pub const HPR_REG: u32 = 0x0300_1001;
    pub const LPR_REG: u32 = 0x0001_4001;
    pub const WR_REG: u32 = 0x0004_E020;
    pub const DRAM_PARAM0: u32 = 0x349B_48CD;    // Timing!
    pub const DRAM_PARAM1: u32 = 0x8201_58A4;    // Timing!
    pub const DRAM_PARAM2: u32 = 0x2508_82C4;    // Timing!
    pub const DRAM_PARAM3: u32 = 0x0080_9004;
    pub const DRAM_PARAM4: u32 = 0x0000_0000;
    pub const DRAM_INIT: u32 = 0x0004_0952;
    pub const DRAM_EMR: u32 = 0x0002_0022;
    pub const DRAM_EMR_MR: u32 = 0xFF00_0000;
    pub const DRAM_BURST8: u32 = 0x0FF6_6666;
    pub const DRAM_DISABLE_DQ: u32 = 0x0000_0256;
    pub const DRAM_ADDR_MAP_BANK: u32 = 0x0000_2223;
    pub const DRAM_ADDR_MAP_COL: u32 = 0x0002_0FE0;
    pub const PHY_CMD_TIMEOUT: u32 = 0x1020_0800;
    pub const PHY_CTRL_STS: u32 = 0x0020_0065;
    pub const PHY_DLL_LOCK: u32 = 0x0000_0050;   // Same for all 4
    pub const ECC_SCRUB: u32 = 0x0000_0000;      // ECC disabled
}

/// DDR Controller
pub struct Ddr {
    base: Reg,
}

impl Ddr {
    /// Create DDR controller instance
    pub const fn new() -> Self {
        Self { base: Reg::new(DDRC_BASE) }
    }

    /// Check if DDR is already initialized (FSBL ran)
    pub fn is_initialized(&self) -> bool {
        let ctrl = self.base.offset(regs::CTRL).read();
        (ctrl & 1) == 1
    }

    /// Get DDR controller status register
    pub fn status(&self) -> u32 {
        self.base.offset(regs::CTRL).read()
    }

    /// Initialize DDR controller with board-specific timing
    ///
    /// # Safety
    ///
    /// This should only be called if DDR is NOT already initialized.
    /// Calling this when DDR is in use will crash the system.
    ///
    /// Normally the FSBL handles this. Only use this for:
    /// - JTAG debugging without FSBL
    /// - Custom boot scenarios
    pub unsafe fn init(&self) {
        use board_config::*;

        // Write configuration registers (DDR disabled)
        self.base.offset(regs::CTRL).write(CTRL_INIT);
        self.base.offset(regs::TWO_RANK_CFG).write(TWO_RANK_CFG);
        self.base.offset(regs::HPR_REG).write(HPR_REG);
        self.base.offset(regs::LPR_REG).write(LPR_REG);
        self.base.offset(regs::WR_REG).write(WR_REG);

        // Timing parameters (board-specific!)
        self.base.offset(regs::DRAM_PARAM0).write(DRAM_PARAM0);
        self.base.offset(regs::DRAM_PARAM1).write(DRAM_PARAM1);
        self.base.offset(regs::DRAM_PARAM2).write(DRAM_PARAM2);
        self.base.offset(regs::DRAM_PARAM3).write(DRAM_PARAM3);
        self.base.offset(regs::DRAM_PARAM4).write(DRAM_PARAM4);

        // Init and mode registers
        self.base.offset(regs::DRAM_INIT).write(DRAM_INIT);
        self.base.offset(regs::DRAM_EMR).write(DRAM_EMR);
        self.base.offset(regs::DRAM_EMR_MR).write(DRAM_EMR_MR);
        self.base.offset(regs::DRAM_BURST8).write(DRAM_BURST8);
        self.base.offset(regs::DRAM_DISABLE_DQ).write(DRAM_DISABLE_DQ);

        // Address mapping
        self.base.offset(regs::DRAM_ADDR_MAP_BANK).write(DRAM_ADDR_MAP_BANK);
        self.base.offset(regs::DRAM_ADDR_MAP_COL).write(DRAM_ADDR_MAP_COL);

        // PHY configuration
        self.base.offset(regs::PHY_CMD_TIMEOUT).write(PHY_CMD_TIMEOUT);
        self.base.offset(regs::PHY_CTRL_STS).write(PHY_CTRL_STS);
        self.base.offset(regs::PHY_DLL_LOCK0).write(PHY_DLL_LOCK);
        self.base.offset(regs::PHY_DLL_LOCK1).write(PHY_DLL_LOCK);
        self.base.offset(regs::PHY_DLL_LOCK2).write(PHY_DLL_LOCK);
        self.base.offset(regs::PHY_DLL_LOCK3).write(PHY_DLL_LOCK);

        // ECC disabled
        self.base.offset(regs::ECC_SCRUB).write(ECC_SCRUB);

        // Enable DDR controller
        let ctrl = self.base.offset(regs::CTRL).read();
        self.base.offset(regs::CTRL).write(ctrl | 1);

        // Wait for DDR to initialize
        // The hardware needs time to complete the init sequence
        super::delay_ms(10);
    }

    /// Read a timing parameter register (for debugging)
    pub fn read_timing(&self, reg: u8) -> u32 {
        match reg {
            0 => self.base.offset(regs::DRAM_PARAM0).read(),
            1 => self.base.offset(regs::DRAM_PARAM1).read(),
            2 => self.base.offset(regs::DRAM_PARAM2).read(),
            3 => self.base.offset(regs::DRAM_PARAM3).read(),
            _ => 0,
        }
    }
}

/// Quick check: is DDR ready?
pub fn is_ddr_ready() -> bool {
    let ddr = Ddr::new();
    ddr.is_initialized()
}
