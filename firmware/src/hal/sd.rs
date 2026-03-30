//! SD Card Driver for Zynq 7020 (SDIO0 @ 0xE010_0000)
//!
//! All register accesses are 32-bit word-aligned. The SDHCI standard defines
//! some registers at byte/halfword offsets, but on Zynq APB without MMU,
//! sub-word writes cause Data Abort (DFSR=0x801). This driver packs sub-word
//! fields into their containing 32-bit words.
//!
//! Register word layout:
//!   0x04: BLK_SIZE[15:0] | BLK_CNT[31:16]
//!   0x0C: XFER_MODE[15:0] | CMD[31:16]
//!   0x28: HOST_CTRL[7:0] | PWR_CTRL[15:8] | BLK_GAP[23:16] | WAKEUP[31:24]
//!   0x2C: CLK_CTRL[15:0] | TIMEOUT_CTRL[23:16] | SW_RST[31:24]

use core::ptr::{read_volatile, write_volatile};

/// SD Card Errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdError {
    CmdTimeout,
    DataTimeout,
    CrcError,
    DmaError,
    CardNotPresent,
    InitFailed,
    ResponseError,
    WriteError,
    ReadError,
}

/// SDIO0 base address
const SDIO0_BASE: usize = 0xE010_0000;

/// Register offsets (all 32-bit word-aligned)
mod regs {
    pub const DMA_ADDR: usize      = 0x00;
    pub const BLK: usize           = 0x04; // [15:0]=BLK_SIZE, [31:16]=BLK_CNT
    pub const ARG: usize           = 0x08;
    pub const CMD: usize           = 0x0C; // [15:0]=XFER_MODE, [31:16]=CMD
    pub const RESP0: usize         = 0x10;
    pub const RESP1: usize         = 0x14;
    pub const RESP2: usize         = 0x18;
    pub const RESP3: usize         = 0x1C;
    pub const DATA: usize          = 0x20;
    pub const PRESENT_STATE: usize = 0x24;
    pub const HOST_PWR: usize      = 0x28; // [7:0]=HOST, [15:8]=PWR, [23:16]=GAP, [31:24]=WAKE
    pub const CLK_RST: usize       = 0x2C; // [15:0]=CLK, [23:16]=TIMEOUT, [31:24]=SW_RST
    pub const INT_STS: usize       = 0x30;
    pub const INT_STS_EN: usize    = 0x34;
    pub const INT_SIG_EN: usize    = 0x38;
}

/// SD commands
mod cmd {
    pub const GO_IDLE_STATE: u8 = 0;
    pub const SEND_IF_COND: u8 = 8;
    pub const ALL_SEND_CID: u8 = 2;
    pub const SEND_RELATIVE_ADDR: u8 = 3;
    pub const SELECT_CARD: u8 = 7;
    pub const SET_BLOCKLEN: u8 = 16;
    pub const READ_SINGLE_BLOCK: u8 = 17;
    pub const WRITE_SINGLE_BLOCK: u8 = 24;
    pub const APP_CMD: u8 = 55;
    pub const SD_SEND_OP_COND: u8 = 41;
}

/// CMD register flags (bits [15:0] of 16-bit CMD field)
mod flags {
    pub const CMD_RESP_NONE: u16 = 0x00;
    pub const CMD_RESP_136: u16 = 0x01;
    pub const CMD_RESP_48: u16 = 0x02;
    #[allow(dead_code)]
    pub const CMD_RESP_48_BUSY: u16 = 0x03;
    pub const CMD_CRC_CHECK: u16 = 0x08;
    pub const CMD_IDX_CHECK: u16 = 0x10;
    pub const CMD_DATA_PRESENT: u16 = 0x20;

    // INT STATUS bits
    pub const INT_CMD_COMPLETE: u32 = 0x0001;
    pub const INT_XFER_COMPLETE: u32 = 0x0002;
    pub const INT_BUF_WR_READY: u32 = 0x0010;
    pub const INT_BUF_RD_READY: u32 = 0x0020;
    pub const INT_ERROR: u32 = 0x8000;

    // XFER_MODE bits
    pub const XFER_READ: u16 = 1 << 4; // Data direction: card-to-host
}

/// SD Card Interface
pub struct SdCard {
    base: usize,
    rca: u32,
    high_capacity: bool,
}

impl SdCard {
    pub const fn new() -> Self {
        Self {
            base: SDIO0_BASE,
            rca: 0,
            high_capacity: false,
        }
    }

    // =========================================================================
    // 32-bit aligned register access
    // =========================================================================

    #[inline]
    fn reg_read(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }

    #[inline]
    fn reg_write(&self, offset: usize, val: u32) {
        unsafe { write_volatile((self.base + offset) as *mut u32, val) }
    }

    /// Read-modify-write: set bits in a 32-bit register
    #[inline]
    fn reg_set(&self, offset: usize, mask: u32) {
        let val = self.reg_read(offset);
        self.reg_write(offset, val | mask);
    }

    /// Read-modify-write: clear bits in a 32-bit register
    #[inline]
    fn reg_clear(&self, offset: usize, mask: u32) {
        let val = self.reg_read(offset);
        self.reg_write(offset, val & !mask);
    }

    // =========================================================================
    // Sub-word field helpers (32-bit RMW for SDHCI packed registers)
    // =========================================================================

    /// Write SW_RST field (byte 3 of word at 0x2C)
    fn write_sw_rst(&self, val: u8) {
        let word = self.reg_read(regs::CLK_RST);
        self.reg_write(regs::CLK_RST, (word & 0x00FF_FFFF) | ((val as u32) << 24));
    }

    /// Read SW_RST field (byte 3 of word at 0x2C)
    fn read_sw_rst(&self) -> u8 {
        ((self.reg_read(regs::CLK_RST) >> 24) & 0xFF) as u8
    }

    /// Write PWR_CTRL field (byte 1 of word at 0x28)
    fn write_pwr_ctrl(&self, val: u8) {
        let word = self.reg_read(regs::HOST_PWR);
        self.reg_write(regs::HOST_PWR, (word & 0xFFFF_00FF) | ((val as u32) << 8));
    }

    /// Write CLK_CTRL field (lower 16 bits of word at 0x2C), preserving upper 16
    fn write_clk_ctrl(&self, val: u16) {
        let word = self.reg_read(regs::CLK_RST);
        self.reg_write(regs::CLK_RST, (word & 0xFFFF_0000) | (val as u32));
    }

    /// Read CLK_CTRL field (lower 16 bits of word at 0x2C)
    fn read_clk_ctrl(&self) -> u16 {
        (self.reg_read(regs::CLK_RST) & 0xFFFF) as u16
    }

    /// Set bits in CLK_CTRL (lower 16 bits of 0x2C), preserving upper 16
    fn clk_ctrl_set(&self, mask: u16) {
        let word = self.reg_read(regs::CLK_RST);
        self.reg_write(regs::CLK_RST, word | (mask as u32));
    }

    /// Clear bits in CLK_CTRL (lower 16 bits of 0x2C), preserving upper 16
    fn clk_ctrl_clear(&self, mask: u16) {
        let word = self.reg_read(regs::CLK_RST);
        self.reg_write(regs::CLK_RST, word & !(mask as u32));
    }

    /// Write BLK_SIZE[15:0] and BLK_CNT[31:16] as a single 32-bit write
    fn write_blk_size_cnt(&self, size: u16, count: u16) {
        self.reg_write(regs::BLK, (count as u32) << 16 | (size as u32));
    }

    /// Write CMD[31:16] + XFER_MODE[15:0] as a single 32-bit write
    /// This triggers command execution (writing CMD upper byte starts it)
    fn write_cmd_xfer(&self, cmd_val: u16, xfer_mode: u16) {
        self.reg_write(regs::CMD, ((cmd_val as u32) << 16) | (xfer_mode as u32));
    }

    // =========================================================================
    // Init
    // =========================================================================

    /// Initialize the SD Card driver
    pub fn init(&mut self, slcr: &crate::hal::Slcr) -> Result<(), SdError> {
        // 1. Enable SDIO clock in SLCR
        slcr.enable_sd0();
        slcr.set_sdio_clk_ctrl(
            (30 << 8) // DIVISOR = 30 (IO PLL 1000MHz / 30 ≈ 33MHz base)
            | (0 << 4) // SRCSEL = IO PLL
            | 1,       // CLKACT = enabled
        );

        // 2. Software reset (all)
        self.write_sw_rst(0x01);
        let mut timeout = 10000u32;
        while self.read_sw_rst() & 0x01 != 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(SdError::InitFailed);
            }
            crate::hal::delay_us(10);
        }

        // 3. Set clock to ~400kHz for init (divisor=128: 33MHz/256 ≈ 128kHz)
        // CLK_CTRL: bit 0 = internal clock enable, bits [15:8] = divisor upper byte
        self.write_clk_ctrl(0x8001); // Divisor=128 (0x80<<8), internal clock enable
        // Wait for internal clock stable (bit 1)
        timeout = 10000;
        while self.read_clk_ctrl() & 0x02 == 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(SdError::InitFailed);
            }
            crate::hal::delay_us(10);
        }
        // Enable SD clock output (bit 2)
        self.clk_ctrl_set(0x04);

        // 4. Power on (3.3V)
        // PWR_CTRL: bits [3:1]=voltage (111=3.3V), bit 0=power on
        self.write_pwr_ctrl(0x0F);

        // 5. Wait for power stable
        crate::hal::delay_ms(10);

        // Enable all interrupt status bits for polling
        self.reg_write(regs::INT_STS_EN, 0xFFFF_FFFF);

        // CMD0: Go Idle State
        self.send_cmd(cmd::GO_IDLE_STATE, 0, flags::CMD_RESP_NONE, 0)?;

        // CMD8: Send IF Cond (voltage 2.7-3.6V, check pattern 0xAA)
        let resp = self.send_cmd(
            cmd::SEND_IF_COND,
            0x1AA,
            flags::CMD_RESP_48 | flags::CMD_CRC_CHECK | flags::CMD_IDX_CHECK,
            0,
        )?;
        if (resp[0] & 0xFF) != 0xAA {
            return Err(SdError::InitFailed);
        }

        // ACMD41: Send Op Cond (loop until card ready)
        timeout = 1000;
        loop {
            // CMD55 (APP_CMD)
            self.send_cmd(cmd::APP_CMD, 0, flags::CMD_RESP_48 | flags::CMD_CRC_CHECK, 0)?;

            // ACMD41 (SD_SEND_OP_COND): HCS=1, voltage window 3.2-3.3V
            let ocr = self.send_cmd(cmd::SD_SEND_OP_COND, 0x40300000, flags::CMD_RESP_48, 0)?;

            // Check busy bit (31) = card ready
            if (ocr[0] & 0x8000_0000) != 0 {
                self.high_capacity = (ocr[0] & 0x4000_0000) != 0;
                break;
            }

            timeout -= 1;
            if timeout == 0 {
                return Err(SdError::InitFailed);
            }
            crate::hal::delay_ms(10);
        }

        // CMD2: All Send CID
        self.send_cmd(cmd::ALL_SEND_CID, 0, flags::CMD_RESP_136 | flags::CMD_CRC_CHECK, 0)?;

        // CMD3: Get Relative Card Address
        let rca_resp = self.send_cmd(
            cmd::SEND_RELATIVE_ADDR,
            0,
            flags::CMD_RESP_48 | flags::CMD_CRC_CHECK,
            0,
        )?;
        self.rca = rca_resp[0] >> 16;

        // CMD7: Select card
        self.send_cmd(
            cmd::SELECT_CARD,
            self.rca << 16,
            flags::CMD_RESP_48 | flags::CMD_CRC_CHECK,
            0,
        )?;

        // Switch to higher speed clock (divisor=2: 33MHz/4 ≈ 8MHz)
        self.clk_ctrl_clear(0x04); // Disable SD clock
        self.write_clk_ctrl(0x0101); // Divisor=1 (bits [15:8]=0x01), internal clock enable
        timeout = 10000;
        while self.read_clk_ctrl() & 0x02 == 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(SdError::InitFailed);
            }
            crate::hal::delay_us(10);
        }
        self.clk_ctrl_set(0x04); // Re-enable SD clock

        Ok(())
    }

    // =========================================================================
    // Block I/O
    // =========================================================================

    /// Read a single 512-byte block
    pub fn read_block(&self, block_idx: u32, _timeout_ms: u32) -> Result<[u8; 512], SdError> {
        let addr = if self.high_capacity { block_idx } else { block_idx * 512 };

        // Set block size=512, count=1
        self.write_blk_size_cnt(0x200, 1);

        // Send CMD17 (READ_SINGLE_BLOCK) with XFER_MODE direction=read
        let cmd_flags = flags::CMD_RESP_48
            | flags::CMD_CRC_CHECK
            | flags::CMD_IDX_CHECK
            | flags::CMD_DATA_PRESENT;
        self.send_cmd(cmd::READ_SINGLE_BLOCK, addr, cmd_flags, flags::XFER_READ)?;

        let mut buffer = [0u8; 512];
        let mut idx = 0;

        let mut timeout = 1_000_000u32;
        while idx < 512 {
            let status = self.reg_read(regs::INT_STS);

            if status & flags::INT_ERROR != 0 {
                return Err(SdError::ReadError);
            }

            if status & flags::INT_BUF_RD_READY != 0 {
                // Read 32-bit word from DATA port
                let word = self.reg_read(regs::DATA);
                buffer[idx] = (word & 0xFF) as u8;
                buffer[idx + 1] = ((word >> 8) & 0xFF) as u8;
                buffer[idx + 2] = ((word >> 16) & 0xFF) as u8;
                buffer[idx + 3] = ((word >> 24) & 0xFF) as u8;
                idx += 4;
            }

            if status & flags::INT_XFER_COMPLETE != 0 {
                self.reg_write(regs::INT_STS, flags::INT_XFER_COMPLETE | flags::INT_BUF_RD_READY);
                break;
            }

            timeout -= 1;
            if timeout == 0 {
                return Err(SdError::DataTimeout);
            }
        }

        Ok(buffer)
    }

    /// Write a single 512-byte block
    pub fn write_block(&self, block_idx: u32, data: &[u8; 512]) -> Result<(), SdError> {
        let addr = if self.high_capacity { block_idx } else { block_idx * 512 };

        // Set block size=512, count=1
        self.write_blk_size_cnt(0x200, 1);

        // Send CMD24 (WRITE_SINGLE_BLOCK) with XFER_MODE direction=write (0)
        let cmd_flags = flags::CMD_RESP_48
            | flags::CMD_CRC_CHECK
            | flags::CMD_IDX_CHECK
            | flags::CMD_DATA_PRESENT;
        self.send_cmd(cmd::WRITE_SINGLE_BLOCK, addr, cmd_flags, 0)?;

        let mut idx = 0;

        let mut timeout = 1_000_000u32;
        while idx < 512 {
            let status = self.reg_read(regs::INT_STS);

            if status & flags::INT_ERROR != 0 {
                return Err(SdError::WriteError);
            }

            if status & flags::INT_BUF_WR_READY != 0 {
                let word = (data[idx] as u32)
                    | ((data[idx + 1] as u32) << 8)
                    | ((data[idx + 2] as u32) << 16)
                    | ((data[idx + 3] as u32) << 24);
                self.reg_write(regs::DATA, word);
                idx += 4;
            }

            if status & flags::INT_XFER_COMPLETE != 0 {
                self.reg_write(regs::INT_STS, flags::INT_XFER_COMPLETE | flags::INT_BUF_WR_READY);
                break;
            }

            timeout -= 1;
            if timeout == 0 {
                return Err(SdError::DataTimeout);
            }
        }

        Ok(())
    }

    // =========================================================================
    // Command
    // =========================================================================

    /// Send a command via SDHCI
    ///
    /// Writes ARG, then XFER_MODE+CMD as a single 32-bit write (CMD upper
    /// halfword triggers execution). Returns 4-word response.
    fn send_cmd(&self, cmd: u8, arg: u32, cmd_flags: u16, xfer_mode: u16) -> Result<[u32; 4], SdError> {
        // Clear all interrupt status
        self.reg_write(regs::INT_STS, 0xFFFF_FFFF);

        // Write argument
        self.reg_write(regs::ARG, arg);

        // Build CMD register value (16-bit):
        //   [13:8] = command index
        //   [5]    = data present
        //   [4]    = index check
        //   [3]    = CRC check
        //   [1:0]  = response type
        let cmd_val = ((cmd as u16) << 8) | cmd_flags;

        // Single 32-bit write: XFER_MODE[15:0] | CMD[31:16]
        // Writing CMD triggers command execution
        self.write_cmd_xfer(cmd_val, xfer_mode);

        // Wait for Command Complete (INT_STS bit 0)
        let mut timeout = 100_000u32;
        while self.reg_read(regs::INT_STS) & flags::INT_CMD_COMPLETE == 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(SdError::CmdTimeout);
            }
            crate::hal::delay_us(1);
        }

        // Clear Command Complete
        self.reg_write(regs::INT_STS, flags::INT_CMD_COMPLETE);

        // Read response
        Ok([
            self.reg_read(regs::RESP0),
            self.reg_read(regs::RESP1),
            self.reg_read(regs::RESP2),
            self.reg_read(regs::RESP3),
        ])
    }
}
