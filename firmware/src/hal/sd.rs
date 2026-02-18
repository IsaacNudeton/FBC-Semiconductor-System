//! SD Card Driver for Zynq 7020 (SDIO0 @ 0xE010_0000)
//! 
//! # Design Pattern: ONETWO
//! - Invariant: SDIO Controller Interface (PrimeCell PL180/SDhci)
//! - Varies: Card capacity, Block size
//! - Pattern: Polling-based command/data transfer for simplicity/determinism.

use crate::hal::{Reg, Register, Slcr};

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

/// Zynq SDIO Register Map
mod regs {
    pub const DMA_ADDR: usize   = 0x00;
    pub const BLK_SIZE: usize   = 0x04;
    pub const BLK_CNT: usize    = 0x06;
    pub const ARG: usize        = 0x08;
    pub const XFER_MODE: usize  = 0x0C;
    pub const CMD: usize        = 0x0E;
    pub const RESP0: usize      = 0x10;
    pub const RESP1: usize      = 0x14;
    pub const RESP2: usize      = 0x18;
    pub const RESP3: usize      = 0x1C;
    pub const DATA: usize       = 0x20;
    pub const PRESENT_STATE: usize = 0x24; // PSTATE
    pub const HOST_CTRL: usize  = 0x28;
    pub const PWR_CTRL: usize   = 0x29;
    pub const BLK_GAP_CTRL: usize = 0x2A;
    pub const WAKEUP_CTRL: usize = 0x2B;
    pub const CLK_CTRL: usize   = 0x2C;
    pub const TIMEOUT_CTRL: usize = 0x2E;
    pub const SW_RST: usize     = 0x2F;
    pub const INT_STS: usize    = 0x30;
    pub const INT_STS_EN: usize = 0x34;
    pub const INT_SIG_EN: usize = 0x38;
}

/// Commands
mod cmd {
    pub const GO_IDLE_STATE: u8 = 0;
    pub const SEND_OP_COND: u8  = 1;
    pub const ALL_SEND_CID: u8  = 2;
    pub const SEND_RELATIVE_ADDR: u8 = 3;
    pub const SELECT_CARD: u8   = 7;
    pub const SEND_IF_COND: u8  = 8;
    pub const SEND_CSD: u8      = 9;
    pub const STOP_TRANSMISSION: u8 = 12;
    pub const SET_BLOCKLEN: u8  = 16;
    pub const READ_SINGLE_BLOCK: u8 = 17;
    pub const WRITE_SINGLE_BLOCK: u8 = 24;
    pub const APP_CMD: u8       = 55;
    pub const SD_SEND_OP_COND: u8 = 41; // ACMD41
}

/// Flags
mod flags {
    // CMD Register
    pub const CMD_RESP_NONE: u16 = 0x00;
    pub const CMD_RESP_136: u16  = 0x01; // Response length 136 bit
    pub const CMD_RESP_48: u16   = 0x02; // Response length 48 bit
    pub const CMD_RESP_48_BUSY: u16 = 0x03; // Response length 48 bit with busy check
    pub const CMD_CRC_CHECK: u16 = 0x08;
    pub const CMD_IDX_CHECK: u16 = 0x10;
    pub const CMD_DATA_PRESENT: u16 = 0x20;
    pub const CMD_TYPE_NORMAL: u16 = 0x00;
    pub const CMD_TYPE_SUSPEND: u16 = 0x40;
    pub const CMD_TYPE_RESUME: u16 = 0x80;
    pub const CMD_TYPE_ABORT: u16 = 0xC0;
    
    // INT STATUS Register
    #[allow(dead_code)]
    pub const INT_CMD_COMPLETE: u32 = 0x0001;
    pub const INT_XFER_COMPLETE: u32 = 0x0002;
    pub const INT_BUF_WR_READY: u32 = 0x0010;
    pub const INT_BUF_RD_READY: u32 = 0x0020;
    pub const INT_CARD_INSERTION: u32 = 0x0040;
    pub const INT_ERR_CMD_TIMEOUT: u32 = 0x0001_0000;
}

/// SD Card Interface
pub struct SdCard {
    base: Reg,
    rca: u32,               // Relative Card Address
    high_capacity: bool,
}

impl SdCard {
    /// Create a new SD Card driver instance
    pub const fn new() -> Self {
        // SDIO 0 Controller Base Address
        Self {
            base: Reg::new(0xE010_0000),
            rca: 0,
            high_capacity: false,
        }
    }

    /// Initialize the SD Card driver
    pub fn init(&mut self, slcr: &Slcr) -> Result<(), SdError> {
        // 1. Enable Clock in SLCR
        slcr.unlock();
        slcr.enable_sd0();
        
        // Disable clock before changing frequency
        // Select Src=IOPLL(00), Div=32 (for ~30MHz base) -> Slow for init?
        // Actually, internal clock needs to be set.
        // Let's assume standard 100MHz input.
        slcr.set_sdio_clk_ctrl(0x0202); // Enable, Src=ARM
        slcr.lock();

        // 2. Soft Reset
        self.base.offset(regs::SW_RST).write(0x1); // Reset all
        while (self.base.offset(regs::SW_RST).read() & 0x1) != 0 {}

        // 3. Set Clock to 400kHz (Init Mode)
        // Enable internal clock (bit 0), SD clock enable (bit 2)
        // Divisor in upper byte. 100MHz / 256 ~= 390kHz
        self.base.offset(regs::CLK_CTRL).write(0x8001); 
        while (self.base.offset(regs::CLK_CTRL).read() & 0x2) == 0 {} // Wait for stable
        self.base.offset(regs::CLK_CTRL).set_bits(0x4); // Enable SD Clock

        // 4. Power On (3.3V)
        self.base.offset(regs::PWR_CTRL).write(0x0F); // 3.3V | Power ON

        // 5. Send Initialization Sequence
        crate::hal::delay_ms(10); // Wait for power stable

        // CMD0: Go Idle
        self.send_cmd(cmd::GO_IDLE_STATE, 0, flags::CMD_RESP_NONE)?;

        // CMD8: Send IF Cond (Check voltage 2.7-3.6V, check pattern 0xAA)
        let resp = self.send_cmd(cmd::SEND_IF_COND, 0x1AA, flags::CMD_RESP_48 | flags::CMD_CRC_CHECK | flags::CMD_IDX_CHECK)?;
        if (resp[0] & 0xFF) != 0xAA {
            return Err(SdError::InitFailed); // Pattern mismatch
        }

        // ACMD41: Send Op Cond (Loop until ready)
        let mut timeout = 1000;
        loop {
            // APP_CMD
            self.send_cmd(cmd::APP_CMD, 0, flags::CMD_RESP_48 | flags::CMD_CRC_CHECK)?;
            
            // SD_SEND_OP_COND (HCS=1 for SDHC/SDXC support, 3.2-3.3V)
            // Arg: 0x40FF8000 (HCS=1, Volts=Window)
            let ocr = self.send_cmd(cmd::SD_SEND_OP_COND, 0x40300000, flags::CMD_RESP_48)?; // HCS, 3.2-3.3V
            
            // Check busy bit (31)
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
        self.send_cmd(cmd::ALL_SEND_CID, 0, flags::CMD_RESP_136 | flags::CMD_CRC_CHECK)?;

        // CMD3: Send Relative Address
        let rca_resp = self.send_cmd(cmd::SEND_RELATIVE_ADDR, 0, flags::CMD_RESP_48 | flags::CMD_CRC_CHECK)?;
        self.rca = rca_resp[0] >> 16;

        // CMD7: Select Card
        self.send_cmd(cmd::SELECT_CARD, self.rca << 16, flags::CMD_RESP_48 | flags::CMD_CRC_CHECK)?;

        // Switch to High Speed (25MHz or 50MHz)
        // Disable clock
        self.base.offset(regs::CLK_CTRL).clear_bits(0x4);
        // Set divisor 0 (Base clock directly, typically 50MHz if supported, or div 2)
        // Safe: Div 2 (0x1 in bits 15:8) -> 100MHz/2 = 50MHz
        self.base.offset(regs::CLK_CTRL).write(0x0101); // Internal enable + div 2
        while (self.base.offset(regs::CLK_CTRL).read() & 0x2) == 0 {}
        self.base.offset(regs::CLK_CTRL).set_bits(0x4); // Enable SD Clock

        Ok(()) // Init success
    }

    /// Read a single block (512 bytes)
    pub fn read_block(&self, block_idx: u32, _timeout_ms: u32) -> Result<[u8; 512], SdError> {
        let addr = if self.high_capacity { block_idx } else { block_idx * 512 };
        
        // Set Block Size
        self.base.offset(regs::BLK_SIZE).write(0x200); // 512 bytes
        self.base.offset(regs::BLK_CNT).write(1);

        // Send CMD17 (READ_SINGLE_BLOCK)
        let flags = flags::CMD_RESP_48 | flags::CMD_CRC_CHECK | flags::CMD_IDX_CHECK | flags::CMD_DATA_PRESENT;
        self.send_cmd(cmd::READ_SINGLE_BLOCK, addr, flags)?;

        let mut buffer = [0u8; 512];
        let mut idx = 0;
        
        // Polling read loop
        while idx < 512 {
            let status = self.base.offset(regs::INT_STS).read();
            
            // Check errors
            if (status & 0x8000) != 0 { // Error Int
                 return Err(SdError::ReadError);
            }

            // Buffer Read Ready
            if (status & flags::INT_BUF_RD_READY) != 0 {
                let word = self.base.offset(regs::DATA).read();
                // Little endian unpacking
                buffer[idx] = (word & 0xFF) as u8;
                buffer[idx+1] = ((word >> 8) & 0xFF) as u8;
                buffer[idx+2] = ((word >> 16) & 0xFF) as u8;
                buffer[idx+3] = ((word >> 24) & 0xFF) as u8;
                idx += 4;
                
                // Clear wait? register is FIFO, just read.
            }
            
             // Transfer Complete
            if (status & flags::INT_XFER_COMPLETE) != 0 {
                // Should be done
                self.base.offset(regs::INT_STS).write(flags::INT_XFER_COMPLETE | flags::INT_BUF_RD_READY);
                break;
            }
        }

        Ok(buffer)
    }

    /// Write a single block (512 bytes)
    pub fn write_block(&self, block_idx: u32, data: &[u8; 512]) -> Result<(), SdError> {
        let addr = if self.high_capacity { block_idx } else { block_idx * 512 };

        // Set Block Size
        self.base.offset(regs::BLK_SIZE).write(0x200); // 512 bytes
        self.base.offset(regs::BLK_CNT).write(1);

        // Send CMD24 (WRITE_SINGLE_BLOCK)
        let flags = flags::CMD_RESP_48 | flags::CMD_CRC_CHECK | flags::CMD_IDX_CHECK | flags::CMD_DATA_PRESENT;
        self.send_cmd(cmd::WRITE_SINGLE_BLOCK, addr, flags)?;

        let mut idx = 0;
        
        // Polling write loop
        while idx < 512 {
            let status = self.base.offset(regs::INT_STS).read();
            
            // Check errors
            if (status & 0x8000) != 0 {
                 return Err(SdError::WriteError);
            }

            // Buffer Write Ready
            if (status & flags::INT_BUF_WR_READY) != 0 {
                let word = (data[idx] as u32) 
                         | ((data[idx+1] as u32) << 8)
                         | ((data[idx+2] as u32) << 16) 
                         | ((data[idx+3] as u32) << 24);
                self.base.offset(regs::DATA).write(word);
                idx += 4;
            }
            
            // Transfer Complete
            if (status & flags::INT_XFER_COMPLETE) != 0 {
                self.base.offset(regs::INT_STS).write(flags::INT_XFER_COMPLETE | flags::INT_BUF_WR_READY);
                break;
            }
        }

        Ok(())
    }

    /// Send a command
    fn send_cmd(&self, cmd: u8, arg: u32, flags: u16) -> Result<[u32; 4], SdError> {
        // Clear status
        self.base.offset(regs::INT_STS).write(0xFFFF_FFFF);

        // Write argument
        self.base.offset(regs::ARG).write(arg);

        // Compose CMD register
        // [13:8] CMD Index
        // [1:0] Resp Type (00=None, 01=136, 10=48, 11=48busy)
        // [3] CRC Check
        // [4] Index Check
        // [5] Data Present
        let cmd_reg = ((cmd as u32) << 8) | (flags as u32);
        self.base.offset(regs::CMD).write(cmd_reg);

        // Wait for Command Complete
        let mut timeout = 10000;
        while (self.base.offset(regs::INT_STS).read() & 0x1) == 0 {
            timeout -= 1;
            if timeout == 0 {
                return Err(SdError::CmdTimeout);
            }
            crate::hal::delay_us(10);
        }

        // Clear Command Complete
        self.base.offset(regs::INT_STS).write(0x1);

        // Read Response
        let mut resp = [0u32; 4];
        resp[0] = self.base.offset(regs::RESP0).read();
        resp[1] = self.base.offset(regs::RESP1).read();
        resp[2] = self.base.offset(regs::RESP2).read();
        resp[3] = self.base.offset(regs::RESP3).read();

        Ok(resp)
    }
}
