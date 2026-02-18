//! FBC Communication Protocol
//!
//! Binary protocol for host-to-controller communication.
//! Designed for efficiency and determinism - no JSON, no strings.

use crate::dma::{DmaResult, FbcStreamer};
use crate::regs::{FbcCtrl, PinCtrl, VectorStatus, PinType};

// =============================================================================
// Protocol Constants
// =============================================================================

/// Protocol magic number (ASCII "FBC\x01")
pub const MAGIC: u32 = 0x46424301;

/// Maximum FBC program size (64KB)
pub const MAX_PROGRAM_SIZE: usize = 64 * 1024;

// =============================================================================
// Command Types
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    /// No operation
    Nop = 0x00,
    /// Load FBC program (followed by length + data)
    LoadProgram = 0x01,
    /// Start execution
    Start = 0x02,
    /// Stop execution
    Stop = 0x03,
    /// Reset the controller
    Reset = 0x04,
    /// Get status
    GetStatus = 0x10,
    /// Get error count
    GetErrors = 0x11,
    /// Get cycle count
    GetCycles = 0x12,
    /// Get version
    GetVersion = 0x13,
    /// Set pin configuration (followed by pin configs)
    SetPinConfig = 0x20,
    /// Get pin configuration
    GetPinConfig = 0x21,
    /// Ping (echo back)
    Ping = 0xFE,
    /// Unknown command
    Unknown = 0xFF,
}

impl From<u8> for Command {
    fn from(val: u8) -> Self {
        match val {
            0x00 => Command::Nop,
            0x01 => Command::LoadProgram,
            0x02 => Command::Start,
            0x03 => Command::Stop,
            0x04 => Command::Reset,
            0x10 => Command::GetStatus,
            0x11 => Command::GetErrors,
            0x12 => Command::GetCycles,
            0x13 => Command::GetVersion,
            0x20 => Command::SetPinConfig,
            0x21 => Command::GetPinConfig,
            0xFE => Command::Ping,
            _ => Command::Unknown,
        }
    }
}

// =============================================================================
// Response Types
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResponseCode {
    Ok = 0x00,
    Error = 0x01,
    Busy = 0x02,
    InvalidCommand = 0x03,
    InvalidLength = 0x04,
    DmaError = 0x05,
    Timeout = 0x06,
}

// =============================================================================
// Packet Structures
// =============================================================================

/// Request header (8 bytes)
#[repr(C, packed)]
pub struct RequestHeader {
    pub magic: u32,
    pub cmd: u8,
    pub flags: u8,
    pub length: u16,  // Length of payload following header
}

/// Response header (8 bytes)
#[repr(C, packed)]
pub struct ResponseHeader {
    pub magic: u32,
    pub cmd: u8,
    pub status: u8,
    pub length: u16,  // Length of payload following header
}

/// Status response payload (32 bytes)
#[repr(C, packed)]
pub struct StatusPayload {
    pub state: u8,
    pub flags: u8,
    pub reserved: u16,
    pub cycle_count: u64,
    pub vector_count: u32,
    pub error_count: u32,
    pub instr_count: u32,
    pub reserved2: [u8; 8],
}

// =============================================================================
// Protocol Handler
// =============================================================================

/// FBC execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    Idle = 0,
    Loading = 1,
    Running = 2,
    Done = 3,
    Error = 4,
}

/// Protocol handler - processes commands and manages execution
pub struct ProtocolHandler {
    state: State,
    streamer: FbcStreamer,
    fbc: FbcCtrl,
    pins: PinCtrl,
    status: VectorStatus,
}

impl ProtocolHandler {
    pub const fn new() -> Self {
        Self {
            state: State::Idle,
            streamer: FbcStreamer::new(),
            fbc: FbcCtrl::new(),
            pins: PinCtrl::new(),
            status: VectorStatus::new(),
        }
    }

    /// Initialize the handler
    pub fn init(&mut self) {
        self.streamer.init();
        self.state = State::Idle;
    }

    /// Process a complete packet
    ///
    /// # Arguments
    /// * `request` - Request packet bytes (header + payload)
    /// * `response` - Buffer to write response into
    ///
    /// # Returns
    /// Number of bytes written to response
    pub fn process(&mut self, request: &[u8], response: &mut [u8]) -> usize {
        // Validate minimum size
        if request.len() < 8 {
            return self.error_response(response, Command::Unknown, ResponseCode::InvalidLength);
        }

        // Parse header
        let header = unsafe { &*(request.as_ptr() as *const RequestHeader) };

        // Validate magic
        if header.magic != MAGIC {
            return self.error_response(response, Command::Unknown, ResponseCode::InvalidCommand);
        }

        let cmd = Command::from(header.cmd);
        let payload_len = header.length as usize;
        let payload = if payload_len > 0 && request.len() >= 8 + payload_len {
            &request[8..8 + payload_len]
        } else {
            &[]
        };

        match cmd {
            Command::Nop => self.ok_response(response, cmd),
            Command::Ping => self.handle_ping(response, payload),
            Command::LoadProgram => self.handle_load(response, payload),
            Command::Start => self.handle_start(response),
            Command::Stop => self.handle_stop(response),
            Command::Reset => self.handle_reset(response),
            Command::GetStatus => self.handle_get_status(response),
            Command::GetErrors => self.handle_get_errors(response),
            Command::GetCycles => self.handle_get_cycles(response),
            Command::GetVersion => self.handle_get_version(response),
            Command::SetPinConfig => self.handle_set_pin_config(response, payload),
            Command::GetPinConfig => self.handle_get_pin_config(response),
            Command::Unknown => self.error_response(response, cmd, ResponseCode::InvalidCommand),
        }
    }

    /// Update internal state based on hardware status
    pub fn poll(&mut self) {
        match self.state {
            State::Running => {
                if self.fbc.is_done() {
                    self.state = if self.fbc.has_error() {
                        State::Error
                    } else {
                        State::Done
                    };
                }
            }
            _ => {}
        }
    }

    /// Get current state
    pub fn state(&self) -> State {
        self.state
    }

    // =========================================================================
    // Command Handlers
    // =========================================================================

    fn handle_ping(&self, response: &mut [u8], payload: &[u8]) -> usize {
        // Echo payload back
        let resp_len = 8 + payload.len();
        if response.len() < resp_len {
            return 0;
        }

        self.write_header(response, Command::Ping, ResponseCode::Ok, payload.len() as u16);
        if !payload.is_empty() {
            response[8..8 + payload.len()].copy_from_slice(payload);
        }
        resp_len
    }

    fn handle_load(&mut self, response: &mut [u8], payload: &[u8]) -> usize {
        if self.state == State::Running {
            return self.error_response(response, Command::LoadProgram, ResponseCode::Busy);
        }

        if payload.is_empty() || payload.len() > MAX_PROGRAM_SIZE {
            return self.error_response(response, Command::LoadProgram, ResponseCode::InvalidLength);
        }

        self.state = State::Loading;

        // Stream to FPGA via DMA
        let result = self.streamer.stream_program(payload);

        match result {
            DmaResult::Ok => {
                self.state = State::Idle;
                self.ok_response(response, Command::LoadProgram)
            }
            DmaResult::Busy => {
                self.error_response(response, Command::LoadProgram, ResponseCode::Busy)
            }
            DmaResult::Timeout => {
                self.state = State::Error;
                self.error_response(response, Command::LoadProgram, ResponseCode::Timeout)
            }
            DmaResult::Error => {
                self.state = State::Error;
                self.error_response(response, Command::LoadProgram, ResponseCode::DmaError)
            }
        }
    }

    fn handle_start(&mut self, response: &mut [u8]) -> usize {
        if self.state == State::Running {
            return self.error_response(response, Command::Start, ResponseCode::Busy);
        }

        self.fbc.enable();
        self.state = State::Running;
        self.ok_response(response, Command::Start)
    }

    fn handle_stop(&mut self, response: &mut [u8]) -> usize {
        self.fbc.disable();
        self.state = State::Idle;
        self.ok_response(response, Command::Stop)
    }

    fn handle_reset(&mut self, response: &mut [u8]) -> usize {
        self.fbc.reset();
        self.streamer.init();
        self.state = State::Idle;
        self.ok_response(response, Command::Reset)
    }

    fn handle_get_status(&self, response: &mut [u8]) -> usize {
        if response.len() < 8 + 32 {
            return 0;
        }

        self.write_header(response, Command::GetStatus, ResponseCode::Ok, 32);

        let payload = StatusPayload {
            state: self.state as u8,
            flags: self.build_flags(),
            reserved: 0,
            cycle_count: self.status.get_cycle_count(),
            vector_count: self.status.get_vector_count(),
            error_count: self.status.get_error_count(),
            instr_count: self.fbc.get_instr_count(),
            reserved2: [0; 8],
        };

        unsafe {
            let src = &payload as *const StatusPayload as *const u8;
            let dst = response.as_mut_ptr().add(8);
            core::ptr::copy_nonoverlapping(src, dst, 32);
        }

        40
    }

    fn handle_get_errors(&self, response: &mut [u8]) -> usize {
        if response.len() < 12 {
            return 0;
        }

        self.write_header(response, Command::GetErrors, ResponseCode::Ok, 4);
        let count = self.status.get_error_count();
        response[8..12].copy_from_slice(&count.to_le_bytes());
        12
    }

    fn handle_get_cycles(&self, response: &mut [u8]) -> usize {
        if response.len() < 16 {
            return 0;
        }

        self.write_header(response, Command::GetCycles, ResponseCode::Ok, 8);
        let count = self.status.get_cycle_count();
        response[8..16].copy_from_slice(&count.to_le_bytes());
        16
    }

    fn handle_get_version(&self, response: &mut [u8]) -> usize {
        if response.len() < 12 {
            return 0;
        }

        self.write_header(response, Command::GetVersion, ResponseCode::Ok, 4);
        let version = self.fbc.get_version();
        response[8..12].copy_from_slice(&version.to_le_bytes());
        12
    }

    fn handle_set_pin_config(&mut self, response: &mut [u8], payload: &[u8]) -> usize {
        // Payload format: [pin_number: u8, pin_type: u8] pairs
        if payload.len() % 2 != 0 {
            return self.error_response(response, Command::SetPinConfig, ResponseCode::InvalidLength);
        }

        for chunk in payload.chunks(2) {
            let pin = chunk[0];
            let pin_type = match chunk[1] {
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
            };
            self.pins.set_pin_type(pin, pin_type);
        }

        self.ok_response(response, Command::SetPinConfig)
    }

    fn handle_get_pin_config(&self, response: &mut [u8]) -> usize {
        // Return all 128 pin types (128 bytes)
        if response.len() < 8 + 128 {
            return 0;
        }

        self.write_header(response, Command::GetPinConfig, ResponseCode::Ok, 128);
        for i in 0..128u8 {
            response[8 + i as usize] = self.pins.get_pin_type(i) as u8;
        }
        8 + 128
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    fn write_header(&self, response: &mut [u8], cmd: Command, status: ResponseCode, length: u16) {
        response[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        response[4] = cmd as u8;
        response[5] = status as u8;
        response[6..8].copy_from_slice(&length.to_le_bytes());
    }

    fn ok_response(&self, response: &mut [u8], cmd: Command) -> usize {
        if response.len() < 8 {
            return 0;
        }
        self.write_header(response, cmd, ResponseCode::Ok, 0);
        8
    }

    fn error_response(&self, response: &mut [u8], cmd: Command, code: ResponseCode) -> usize {
        if response.len() < 8 {
            return 0;
        }
        self.write_header(response, cmd, code, 0);
        8
    }

    fn build_flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.fbc.is_running() { flags |= 0x01; }
        if self.fbc.is_done() { flags |= 0x02; }
        if self.fbc.has_error() { flags |= 0x04; }
        if self.status.has_errors() { flags |= 0x08; }
        flags
    }
}
