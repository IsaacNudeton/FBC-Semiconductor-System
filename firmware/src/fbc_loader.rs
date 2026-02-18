//! FBC Program Loader
//!
//! Loads FBC binary vector files into FPGA for execution.
//!
//! ONETWO Design:
//!   INVARIANT: Header format, pin types, clock frequencies (5 options)
//!   VARIES: Actual FBC file content, selected frequency, pin config
//!   PATTERN: Parse header → configure clock → configure pins → DMA vectors
//!
//! # Usage
//!
//! ```no_run
//! let loader = FbcLoader::new();
//! let result = loader.load_and_run(fbc_data);
//! ```

use core::convert::TryInto;
use crate::regs::{ClkCtrl, PinCtrl, FbcCtrl, PinType, VecClockFreq};
use crate::dma::{FbcStreamer, DmaResult};
use crate::fbc_decompress::{decompress_to_bytecode, MAX_BYTECODE_SIZE};

// =============================================================================
// FBC Format Constants (matches tools/fbc-vec/src/format.rs)
// =============================================================================

/// Magic number: "FBC\0" in little endian
pub const FBC_MAGIC: u32 = 0x00434246;

/// Header size in bytes
pub const HEADER_SIZE: usize = 32;

/// Pin config size in bytes (160 pins × 4 bits = 80 bytes)
pub const PIN_CONFIG_SIZE: usize = 80;

/// Total header + pin config size
pub const HEADER_TOTAL_SIZE: usize = HEADER_SIZE + PIN_CONFIG_SIZE;

/// Number of DUT pins
pub const PIN_COUNT: usize = 160;

// =============================================================================
// FBC Header (no_std compatible)
// =============================================================================

/// FBC file header (32 bytes, little-endian)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FbcHeader {
    pub magic: u32,
    pub version: u16,
    pub pin_count: u8,
    pub flags: u8,
    pub num_vectors: u32,
    pub compressed_size: u32,
    pub vec_clock_hz: u32,
    pub crc32: u32,
    pub _reserved: [u8; 8],
}

impl FbcHeader {
    /// Parse header from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, LoaderError> {
        if bytes.len() < HEADER_SIZE {
            return Err(LoaderError::HeaderTooShort);
        }

        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != FBC_MAGIC {
            return Err(LoaderError::InvalidMagic(magic));
        }

        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        let pin_count = bytes[6];
        let flags = bytes[7];
        let num_vectors = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let compressed_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let vec_clock_hz = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let crc32 = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
        let mut reserved = [0u8; 8];
        reserved.copy_from_slice(&bytes[24..32]);

        Ok(Self {
            magic,
            version,
            pin_count,
            flags,
            num_vectors,
            compressed_size,
            vec_clock_hz,
            crc32,
            _reserved: reserved,
        })
    }

    /// Get vector clock frequency as enum
    pub fn clock_freq(&self) -> VecClockFreq {
        VecClockFreq::from_hz(self.vec_clock_hz)
    }
}

// =============================================================================
// Loader Error
// =============================================================================

/// Loader error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderError {
    /// Data too short for header
    HeaderTooShort,
    /// Invalid magic number
    InvalidMagic(u32),
    /// Data too short for pin config
    PinConfigTooShort,
    /// Data too short for vectors
    VectorsTooShort,
    /// Clock not locked after timeout
    ClockNotLocked,
    /// DMA transfer failed
    DmaError,
    /// DMA busy
    DmaBusy,
    /// DMA timeout
    DmaTimeout,
}

// =============================================================================
// FBC Loader
// =============================================================================

/// FBC program loader
pub struct FbcLoader {
    clk_ctrl: ClkCtrl,
    pin_ctrl: PinCtrl,
    fbc_ctrl: FbcCtrl,
    streamer: FbcStreamer,
}

impl FbcLoader {
    /// Create a new loader
    pub const fn new() -> Self {
        Self {
            clk_ctrl: ClkCtrl::new(),
            pin_ctrl: PinCtrl::new(),
            fbc_ctrl: FbcCtrl::new(),
            streamer: FbcStreamer::new(),
        }
    }

    /// Initialize the loader
    pub fn init(&mut self) {
        self.streamer.init();
    }

    /// Load and run an FBC program
    ///
    /// This is the main entry point for loading FBC files.
    /// It performs the following steps:
    /// 1. Parse FBC header
    /// 2. Set vector clock frequency (ONETWO: MUX selection)
    /// 3. Configure pin types
    /// 4. DMA vector data to FPGA
    /// 5. Enable execution
    ///
    /// # Arguments
    /// * `fbc_data` - Complete FBC file data (header + pin config + vectors)
    pub fn load_and_run(&mut self, fbc_data: &[u8]) -> Result<FbcHeader, LoaderError> {
        // Parse header
        let header = FbcHeader::from_bytes(fbc_data)?;

        // Validate data size
        let expected_size = HEADER_TOTAL_SIZE + header.compressed_size as usize;
        if fbc_data.len() < expected_size {
            return Err(LoaderError::VectorsTooShort);
        }

        // Configure clock (ONETWO: <100ns switch via BUFGMUX)
        self.configure_clock(header.vec_clock_hz)?;

        // Configure pins
        let pin_config = &fbc_data[HEADER_SIZE..HEADER_SIZE + PIN_CONFIG_SIZE];
        self.configure_pins(pin_config)?;

        // Reset FBC decoder before loading
        self.fbc_ctrl.reset();

        // DMA vector data to FPGA
        let vector_data = &fbc_data[HEADER_TOTAL_SIZE..];
        self.stream_vectors(vector_data)?;

        // Enable FBC execution
        self.fbc_ctrl.enable();
        self.clk_ctrl.enable();

        Ok(header)
    }

    /// Load FBC without starting execution
    ///
    /// Use this when you want to configure everything but start manually.
    pub fn load(&mut self, fbc_data: &[u8]) -> Result<FbcHeader, LoaderError> {
        let header = FbcHeader::from_bytes(fbc_data)?;

        let expected_size = HEADER_TOTAL_SIZE + header.compressed_size as usize;
        if fbc_data.len() < expected_size {
            return Err(LoaderError::VectorsTooShort);
        }

        self.configure_clock(header.vec_clock_hz)?;

        let pin_config = &fbc_data[HEADER_SIZE..HEADER_SIZE + PIN_CONFIG_SIZE];
        self.configure_pins(pin_config)?;

        self.fbc_ctrl.reset();

        let vector_data = &fbc_data[HEADER_TOTAL_SIZE..];
        self.stream_vectors(vector_data)?;

        Ok(header)
    }

    /// Start execution (after load())
    pub fn start(&self) {
        self.fbc_ctrl.enable();
        self.clk_ctrl.enable();
    }

    /// Stop execution
    pub fn stop(&self) {
        self.clk_ctrl.disable();
        self.fbc_ctrl.disable();
    }

    /// Check if execution is complete
    pub fn is_done(&self) -> bool {
        self.fbc_ctrl.is_done()
    }

    /// Check if execution is running
    pub fn is_running(&self) -> bool {
        self.fbc_ctrl.is_running()
    }

    /// Check for errors
    pub fn has_error(&self) -> bool {
        self.fbc_ctrl.has_error()
    }

    /// Get instruction count
    pub fn instr_count(&self) -> u32 {
        self.fbc_ctrl.get_instr_count()
    }

    /// Get cycle count
    pub fn cycle_count(&self) -> u64 {
        self.fbc_ctrl.get_cycle_count()
    }

    // =========================================================================
    // Internal Configuration Methods
    // =========================================================================

    /// Configure vector clock frequency
    fn configure_clock(&self, hz: u32) -> Result<(), LoaderError> {
        // Set frequency via ONETWO MUX (no PLL relock needed)
        self.clk_ctrl.set_vec_clock_hz(hz);

        // Wait for MMCM lock (should be nearly instant for MUX switch)
        // Give it up to 1000 iterations just in case
        for _ in 0..1000 {
            if self.clk_ctrl.is_locked() {
                return Ok(());
            }
            core::hint::spin_loop();
        }

        Err(LoaderError::ClockNotLocked)
    }

    /// Configure pin types from packed bytes
    fn configure_pins(&self, pin_config: &[u8]) -> Result<(), LoaderError> {
        if pin_config.len() < PIN_CONFIG_SIZE {
            return Err(LoaderError::PinConfigTooShort);
        }

        // Unpack pin types (2 pins per byte, 4 bits each)
        for i in 0..PIN_CONFIG_SIZE {
            let byte = pin_config[i];
            let pin0 = (i * 2) as u8;
            let pin1 = (i * 2 + 1) as u8;

            let type0 = byte_to_pin_type(byte & 0x0F);
            let type1 = byte_to_pin_type(byte >> 4);

            self.pin_ctrl.set_pin_type(pin0, type0);
            self.pin_ctrl.set_pin_type(pin1, type1);
        }

        Ok(())
    }

    /// Stream vector data to FPGA via DMA
    ///
    /// This decompresses the .fbc format (opcodes 0x00-0x07) into FPGA bytecode
    /// (SET_PINS, PATTERN_REP, HALT) before DMA transfer.
    fn stream_vectors(&mut self, compressed_data: &[u8]) -> Result<(), LoaderError> {
        // Allocate bytecode buffer on stack (64KB max)
        // For larger programs, this would need chunked streaming
        let mut bytecode = [0u8; MAX_BYTECODE_SIZE];

        // Decompress .fbc format to FPGA bytecode
        let bytecode_len = match decompress_to_bytecode(compressed_data, &mut bytecode) {
            Some(len) => len,
            None => return Err(LoaderError::DmaError), // Buffer too small
        };

        // DMA the decompressed bytecode to FPGA
        match self.streamer.stream_program(&bytecode[..bytecode_len]) {
            DmaResult::Ok => Ok(()),
            DmaResult::Busy => Err(LoaderError::DmaBusy),
            DmaResult::Error => Err(LoaderError::DmaError),
            DmaResult::Timeout => Err(LoaderError::DmaTimeout),
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Convert byte value to PinType enum
fn byte_to_pin_type(val: u8) -> PinType {
    match val {
        0 => PinType::Bidi,
        1 => PinType::Input,
        2 => PinType::Output,
        3 => PinType::OpenCollector,
        4 => PinType::Pulse,
        5 => PinType::NPulse,
        6 => PinType::ErrorTrig,
        7 => PinType::VecClk,
        8 => PinType::VecClkEn,
        _ => PinType::Bidi,
    }
}

// =============================================================================
// Convenience Functions
// =============================================================================

/// Parse FBC header only (without loading)
///
/// Useful for inspecting a file before committing to load it.
pub fn parse_header(fbc_data: &[u8]) -> Result<FbcHeader, LoaderError> {
    FbcHeader::from_bytes(fbc_data)
}

/// Get expected clock frequency from FBC data
pub fn get_clock_freq(fbc_data: &[u8]) -> Result<u32, LoaderError> {
    let header = FbcHeader::from_bytes(fbc_data)?;
    Ok(header.vec_clock_hz)
}
