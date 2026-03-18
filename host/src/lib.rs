//! FBC Host Library v2 - Unified Semiconductor Test Control
//!
//! Supports both FBC (raw Ethernet) and Sonoma (SSH) board communication.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────┐         ┌──────────────────┐
//! │    GUI / CLI     │         │   FBC Board(s)   │
//! │                  │  0x88B5 │  (bare-metal)    │
//! │  FbcClient ──────┼────────▶│  MAC: from DNA   │
//! │                  │         └──────────────────┘
//! │  SonomaClient ───┼─SSH────▶┌──────────────────┐
//! │                  │         │  Sonoma Board(s)  │
//! └──────────────────┘         │  (Linux/Zynq)    │
//!                              └──────────────────┘
//! ```

pub mod fbc_protocol;
pub mod types;
pub mod vector;
pub mod sonoma;
pub mod sonoma_parse;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use byteorder::{BigEndian, ByteOrder};
use thiserror::Error;

// Re-export protocol primitives
pub use fbc_protocol::{
    FbcRawSocket, FbcPacket, FbcHeader, FBC_MAGIC, ETHERTYPE_FBC, BROADCAST_MAC,
    AnnouncePayload, HeartbeatPayload, StatusPayload,
};

// Re-export shared types
pub use types::*;

// =============================================================================
// Error Types
// =============================================================================

#[derive(Error, Debug)]
pub enum FbcError {
    #[error("Network interface error: {0}")]
    Interface(String),

    #[error("Send failed: {0}")]
    Send(String),

    #[error("Receive failed: {0}")]
    Receive(String),

    #[error("Board error: {0}")]
    Board(String),

    #[error("Timeout")]
    Timeout,

    #[error("No boards found")]
    NoBoards,

    #[error("Board not found: {0}")]
    BoardNotFound(String),
}

pub type Result<T> = std::result::Result<T, FbcError>;

impl From<fbc_protocol::FbcError> for FbcError {
    fn from(e: fbc_protocol::FbcError) -> Self {
        match e {
            fbc_protocol::FbcError::Interface(s) => FbcError::Interface(s),
            fbc_protocol::FbcError::Send(s) => FbcError::Send(s),
            fbc_protocol::FbcError::Receive(s) => FbcError::Receive(s),
            fbc_protocol::FbcError::Timeout => FbcError::Timeout,
            fbc_protocol::FbcError::InvalidPacket(s) => FbcError::Board(s),
        }
    }
}

// =============================================================================
// FBC Client — wraps FbcRawSocket with all 28 protocol commands
// =============================================================================

/// FBC Client for raw Ethernet communication using proper 8-byte FBC headers.
pub struct FbcClient {
    socket: FbcRawSocket,
    boards: HashMap<[u8; 6], BoardInfo>,
}

impl FbcClient {
    /// Create a new FBC client on the specified network interface
    pub fn new(interface_name: &str) -> Result<Self> {
        let socket = FbcRawSocket::new(interface_name)?;
        Ok(Self {
            socket,
            boards: HashMap::new(),
        })
    }

    /// List available network interfaces
    pub fn list_interfaces() -> Vec<String> {
        FbcRawSocket::list_interfaces()
    }

    // =========================================================================
    // Discovery
    // =========================================================================

    /// Discover all FBC boards on the network.
    /// Sends BIM_STATUS_REQ (0x10) broadcast, collects ANNOUNCE (0x01) responses.
    pub fn discover(&mut self, timeout: Duration) -> Result<Vec<BoardInfo>> {
        self.boards.clear();

        let seq = self.socket.next_seq();
        let packet = FbcPacket::new(fbc_protocol::setup::BIM_STATUS_REQ, seq);
        self.socket.send(BROADCAST_MAC, &packet)?;

        let start = Instant::now();
        while start.elapsed() < timeout {
            if let Some((src_mac, rsp)) = self.socket.recv_timeout(Duration::from_millis(50))? {
                if rsp.header.cmd == fbc_protocol::setup::ANNOUNCE && rsp.payload.len() >= 16 {
                    if let Some(announce) = AnnouncePayload::from_bytes(&rsp.payload) {
                        let board = BoardInfo {
                            system_type: types::SystemType::Fbc,
                            mac: src_mac,
                            serial: announce.serial,
                            fw_version: announce.fw_version,
                            hw_revision: announce.hw_revision,
                            bim_type: announce.bim_type,
                            has_bim: announce.has_bim,
                            bim_programmed: announce.bim_programmed,
                        };
                        self.boards.insert(src_mac, board);
                    }
                }
            }
        }

        Ok(self.boards.values().cloned().collect())
    }

    // =========================================================================
    // Status & Control
    // =========================================================================

    /// Get full 47-byte status telemetry from a board
    pub fn get_status(&mut self, mac: &[u8; 6]) -> Result<StatusResponse> {
        let rsp = self.send_req(mac, fbc_protocol::runtime::STATUS_REQ,
                                fbc_protocol::runtime::STATUS_RSP, &[], 500)?;

        let status = StatusPayload::from_bytes(&rsp.payload)
            .ok_or_else(|| FbcError::Board("Invalid STATUS_RSP payload".into()))?;

        Ok(StatusResponse {
            state: types::ControllerState::from_u8(status.state as u8),
            cycles: status.cycles,
            errors: status.errors,
            temp_c: status.temp_c,
            rail_voltage: status.rail_voltage,
            rail_current: status.rail_current,
            fpga_vccint: status.fpga_vccint,
            fpga_vccaux: status.fpga_vccaux,
        })
    }

    /// Ping a board (measures round-trip time)
    pub fn ping(&mut self, mac: &[u8; 6]) -> Result<Duration> {
        let start = Instant::now();
        let _ = self.get_status(mac)?;
        Ok(start.elapsed())
    }

    /// Start test execution
    pub fn start(&mut self, mac: &[u8; 6]) -> Result<()> {
        self.send_cmd(mac, fbc_protocol::runtime::START, &[])
    }

    /// Stop test execution
    pub fn stop(&mut self, mac: &[u8; 6]) -> Result<()> {
        self.send_cmd(mac, fbc_protocol::runtime::STOP, &[])
    }

    /// Reset board state
    pub fn reset(&mut self, mac: &[u8; 6]) -> Result<()> {
        self.send_cmd(mac, fbc_protocol::runtime::RESET, &[])
    }

    /// Upload vectors to a board (chunked, 1400 bytes per chunk)
    pub fn upload_vectors(&mut self, mac: &[u8; 6], data: &[u8]) -> Result<()> {
        let chunk_size = 1400;
        let total = data.len() as u32;
        let mut offset = 0u32;

        while (offset as usize) < data.len() {
            let end = ((offset as usize) + chunk_size).min(data.len());
            let chunk = &data[offset as usize..end];

            let mut payload = Vec::with_capacity(10 + chunk.len());
            payload.extend_from_slice(&offset.to_be_bytes());
            payload.extend_from_slice(&total.to_be_bytes());
            payload.extend_from_slice(&(chunk.len() as u16).to_be_bytes());
            payload.extend_from_slice(chunk);

            let seq = self.socket.next_seq();
            let packet = FbcPacket::with_payload(
                fbc_protocol::setup::UPLOAD_VECTORS, seq, payload,
            );
            self.socket.send(*mac, &packet)?;

            let ack = self.wait_for_cmd(mac, fbc_protocol::setup::UPLOAD_VECTORS, Duration::from_millis(500))?;
            if ack.is_none() {
                return Err(FbcError::Timeout);
            }

            offset += chunk.len() as u32;
        }

        Ok(())
    }

    /// Configure clock and voltages
    pub fn configure(&mut self, mac: &[u8; 6], clock_div: u8, voltages: [u16; 6]) -> Result<()> {
        let mut payload = vec![0u8; 18];
        payload[0] = clock_div;
        for i in 0..6 {
            BigEndian::write_u16(&mut payload[1 + i * 2..3 + i * 2], voltages[i]);
        }
        self.send_cmd(mac, fbc_protocol::setup::CONFIGURE, &payload)
    }

    /// Wait for execution to complete
    pub fn wait_done(&mut self, mac: &[u8; 6], timeout: Duration) -> Result<StatusResponse> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            let status = self.get_status(mac)?;
            match status.state {
                types::ControllerState::Done | types::ControllerState::Error => return Ok(status),
                types::ControllerState::Idle => return Err(FbcError::Board("Board is idle".into())),
                _ => std::thread::sleep(Duration::from_millis(100)),
            }
        }
        Err(FbcError::Timeout)
    }

    // =========================================================================
    // Fast Pins (gpio[128:159])
    // =========================================================================

    /// Read fast pin state (din, dout, oen)
    pub fn get_fast_pins(&mut self, mac: &[u8; 6]) -> Result<FastPinState> {
        let rsp = self.send_req(mac, fbc_protocol::fastpins::READ_REQ,
                                fbc_protocol::fastpins::READ_RSP, &[], 500)?;
        if rsp.payload.len() < 12 {
            return Err(FbcError::Board("FastPins response too short".into()));
        }
        // Firmware sends: din, dout, oen
        Ok(FastPinState {
            din: BigEndian::read_u32(&rsp.payload[0..4]),
            dout: BigEndian::read_u32(&rsp.payload[4..8]),
            oen: BigEndian::read_u32(&rsp.payload[8..12]),
        })
    }

    /// Write fast pin outputs (dout + oen)
    pub fn set_fast_pins(&mut self, mac: &[u8; 6], dout: u32, oen: u32) -> Result<()> {
        let mut payload = [0u8; 8];
        BigEndian::write_u32(&mut payload[0..4], dout);
        BigEndian::write_u32(&mut payload[4..8], oen);
        // Fire-and-forget (no response expected)
        let seq = self.socket.next_seq();
        let packet = FbcPacket::with_payload(fbc_protocol::fastpins::WRITE, seq, payload.to_vec());
        self.socket.send(*mac, &packet)?;
        Ok(())
    }

    // =========================================================================
    // Analog Monitoring
    // =========================================================================

    /// Read all 32 analog channels (16 XADC + 16 external MAX11131)
    pub fn read_analog(&mut self, mac: &[u8; 6]) -> Result<AnalogChannels> {
        let rsp = self.send_req(mac, fbc_protocol::analog::READ_ALL_REQ,
                                fbc_protocol::analog::READ_ALL_RSP, &[], 500)?;
        if rsp.payload.len() < 192 {
            return Err(FbcError::Board(format!(
                "Analog response too short: {} bytes (need 192)", rsp.payload.len()
            )));
        }

        let mut xadc = Vec::with_capacity(16);
        let mut external = Vec::with_capacity(16);

        for i in 0..16 {
            let offset = i * 6;
            let raw = BigEndian::read_u16(&rsp.payload[offset..offset + 2]);
            let scaled = BigEndian::read_i32(&rsp.payload[offset + 2..offset + 6]);
            xadc.push(AnalogReading {
                channel: i as u8,
                raw,
                voltage_mv: scaled as f32 / 1000.0,
            });
        }

        for i in 0..16 {
            let offset = 96 + i * 6;
            let raw = BigEndian::read_u16(&rsp.payload[offset..offset + 2]);
            let scaled = BigEndian::read_i32(&rsp.payload[offset + 2..offset + 6]);
            external.push(AnalogReading {
                channel: (16 + i) as u8,
                raw,
                voltage_mv: scaled as f32 / 1000.0,
            });
        }

        Ok(AnalogChannels { xadc, external })
    }

    // =========================================================================
    // Power Control (VICOR)
    // =========================================================================

    /// Get VICOR core power supply status (6 cores, 5 bytes each = 30 bytes)
    pub fn get_vicor_status(&mut self, mac: &[u8; 6]) -> Result<VicorStatus> {
        let rsp = self.send_req(mac, fbc_protocol::power::VICOR_STATUS_REQ,
                                fbc_protocol::power::VICOR_STATUS_RSP, &[], 500)?;
        if rsp.payload.len() < 30 {
            return Err(FbcError::Board("VICOR response too short".into()));
        }

        let mut cores = [VicorCore { id: 0, enabled: false, voltage_mv: 0, current_ma: 0 }; 6];
        for i in 0..6 {
            let off = i * 5;
            cores[i] = VicorCore {
                id: (i + 1) as u8,
                enabled: rsp.payload[off] != 0,
                voltage_mv: BigEndian::read_u16(&rsp.payload[off + 1..off + 3]),
                current_ma: BigEndian::read_u16(&rsp.payload[off + 3..off + 5]),
            };
        }

        Ok(VicorStatus { cores })
    }

    /// Enable/disable VICOR cores (bitmask: bit 0 = core 1, etc.)
    pub fn set_vicor_enable(&mut self, mac: &[u8; 6], core_mask: u8) -> Result<()> {
        let seq = self.socket.next_seq();
        let packet = FbcPacket::with_payload(fbc_protocol::power::VICOR_ENABLE, seq, vec![core_mask]);
        self.socket.send(*mac, &packet)?;
        Ok(())
    }

    /// Set VICOR core voltage (core 1-6, voltage in mV)
    pub fn set_vicor_voltage(&mut self, mac: &[u8; 6], core: u8, voltage_mv: u16) -> Result<()> {
        let mut payload = [0u8; 3];
        payload[0] = core;
        BigEndian::write_u16(&mut payload[1..3], voltage_mv);
        let seq = self.socket.next_seq();
        let packet = FbcPacket::with_payload(fbc_protocol::power::VICOR_SET_VOLTAGE, seq, payload.to_vec());
        self.socket.send(*mac, &packet)?;
        Ok(())
    }

    // =========================================================================
    // Power Control (PMBus)
    // =========================================================================

    /// Get PMBus status
    pub fn get_pmbus_status(&mut self, mac: &[u8; 6]) -> Result<PmBusStatus> {
        let rsp = self.send_req(mac, fbc_protocol::power::PMBUS_STATUS_REQ,
                                fbc_protocol::power::PMBUS_STATUS_RSP, &[], 500)?;
        let mut rails = Vec::new();
        // Parse variable-length response
        if rsp.payload.len() >= 2 {
            let count = rsp.payload[0] as usize;
            let mut offset = 1;
            for _ in 0..count {
                if offset + 7 > rsp.payload.len() { break; }
                rails.push(PmBusRail {
                    address: rsp.payload[offset],
                    enabled: rsp.payload[offset + 1] != 0,
                    voltage_mv: BigEndian::read_u16(&rsp.payload[offset + 2..offset + 4]),
                    current_ma: BigEndian::read_u16(&rsp.payload[offset + 4..offset + 6]),
                });
                offset += 7;
            }
        }
        Ok(PmBusStatus { rails })
    }

    /// Enable/disable a PMBus supply
    pub fn set_pmbus_enable(&mut self, mac: &[u8; 6], addr: u8, enable: bool) -> Result<()> {
        let seq = self.socket.next_seq();
        let packet = FbcPacket::with_payload(
            fbc_protocol::power::PMBUS_ENABLE, seq, vec![addr, enable as u8],
        );
        self.socket.send(*mac, &packet)?;
        Ok(())
    }

    /// Emergency stop — kill all power immediately
    pub fn emergency_stop(&mut self, mac: &[u8; 6]) -> Result<()> {
        self.send_cmd(mac, fbc_protocol::power::EMERGENCY_STOP, &[])
    }

    /// Power sequence on — ramp all 6 core voltages
    pub fn power_sequence_on(&mut self, mac: &[u8; 6], voltages: [u16; 6]) -> Result<()> {
        let mut payload = [0u8; 12];
        for i in 0..6 {
            BigEndian::write_u16(&mut payload[i * 2..i * 2 + 2], voltages[i]);
        }
        // Longer timeout for power sequencing
        let seq = self.socket.next_seq();
        let packet = FbcPacket::with_payload(fbc_protocol::power::POWER_SEQUENCE_ON, seq, payload.to_vec());
        self.socket.send(*mac, &packet)?;
        // Wait up to 5s for ACK
        let _ = self.wait_for_cmd(mac, fbc_protocol::power::POWER_SEQUENCE_ON, Duration::from_secs(5));
        Ok(())
    }

    /// Power sequence off — safe shutdown
    pub fn power_sequence_off(&mut self, mac: &[u8; 6]) -> Result<()> {
        let seq = self.socket.next_seq();
        let packet = FbcPacket::new(fbc_protocol::power::POWER_SEQUENCE_OFF, seq);
        self.socket.send(*mac, &packet)?;
        let _ = self.wait_for_cmd(mac, fbc_protocol::power::POWER_SEQUENCE_OFF, Duration::from_secs(5));
        Ok(())
    }

    // =========================================================================
    // EEPROM
    // =========================================================================

    /// Read EEPROM data
    pub fn read_eeprom(&mut self, mac: &[u8; 6], offset: u8, length: u8) -> Result<EepromData> {
        let rsp = self.send_req(mac, fbc_protocol::eeprom::READ_REQ,
                                fbc_protocol::eeprom::READ_RSP, &[offset, length], 500)?;
        if rsp.payload.len() < 2 {
            return Err(FbcError::Board("EEPROM response too short".into()));
        }
        Ok(EepromData {
            offset: rsp.payload[0],
            data: rsp.payload[2..].to_vec(),
        })
    }

    /// Write EEPROM data
    pub fn write_eeprom(&mut self, mac: &[u8; 6], offset: u8, data: &[u8]) -> Result<()> {
        let mut payload = Vec::with_capacity(2 + data.len());
        payload.push(offset);
        payload.push(data.len() as u8);
        payload.extend_from_slice(data);
        self.send_cmd(mac, fbc_protocol::eeprom::WRITE, &payload)
    }

    // =========================================================================
    // Vector Engine (advanced control)
    // =========================================================================

    /// Get vector engine status
    pub fn get_vector_status(&mut self, mac: &[u8; 6]) -> Result<VectorEngineStatus> {
        let rsp = self.send_req(mac, fbc_protocol::vector_engine::STATUS_REQ,
                                fbc_protocol::vector_engine::STATUS_RSP, &[], 500)?;
        if rsp.payload.len() < 33 {
            return Err(FbcError::Board("Vector status response too short".into()));
        }

        Ok(VectorEngineStatus {
            state: VectorState::from_u8(rsp.payload[0]),
            current_address: BigEndian::read_u32(&rsp.payload[1..5]),
            total_vectors: BigEndian::read_u32(&rsp.payload[5..9]),
            loop_count: BigEndian::read_u32(&rsp.payload[9..13]),
            target_loops: BigEndian::read_u32(&rsp.payload[13..17]),
            error_count: BigEndian::read_u32(&rsp.payload[17..21]),
            first_fail_addr: BigEndian::read_u32(&rsp.payload[21..25]),
            run_time_ms: BigEndian::read_u64(&rsp.payload[25..33]),
        })
    }

    /// Start vector engine with loop count
    pub fn start_vectors(&mut self, mac: &[u8; 6], loops: u32) -> Result<()> {
        self.send_cmd(mac, fbc_protocol::vector_engine::START, &loops.to_be_bytes())
    }

    /// Pause vector engine
    pub fn pause_vectors(&mut self, mac: &[u8; 6]) -> Result<()> {
        self.send_cmd(mac, fbc_protocol::vector_engine::PAUSE, &[])
    }

    /// Resume vector engine
    pub fn resume_vectors(&mut self, mac: &[u8; 6]) -> Result<()> {
        self.send_cmd(mac, fbc_protocol::vector_engine::RESUME, &[])
    }

    /// Stop vector engine
    pub fn stop_vectors(&mut self, mac: &[u8; 6]) -> Result<()> {
        self.send_cmd(mac, fbc_protocol::vector_engine::STOP, &[])
    }

    // =========================================================================
    // Error Log
    // =========================================================================

    /// Read error log entries from error BRAMs
    pub fn get_error_log(&mut self, mac: &[u8; 6], start_index: u32, count: u32) -> Result<ErrorLogResponse> {
        let mut payload = [0u8; 8];
        BigEndian::write_u32(&mut payload[0..4], start_index);
        BigEndian::write_u32(&mut payload[4..8], count);

        let rsp = self.send_req(mac, fbc_protocol::error_log::ERROR_LOG_REQ,
                                fbc_protocol::error_log::ERROR_LOG_RSP, &payload, 500)?;
        if rsp.payload.len() < 8 {
            return Err(FbcError::Board("Error log response too short".into()));
        }

        let total_errors = BigEndian::read_u32(&rsp.payload[0..4]);
        let num_entries = BigEndian::read_u32(&rsp.payload[4..8]);

        let mut entries = Vec::new();
        let mut offset = 8;
        for _ in 0..num_entries {
            if offset + 28 > rsp.payload.len() { break; }
            entries.push(ErrorLogEntry {
                pattern: [
                    BigEndian::read_u32(&rsp.payload[offset..offset + 4]),
                    BigEndian::read_u32(&rsp.payload[offset + 4..offset + 8]),
                    BigEndian::read_u32(&rsp.payload[offset + 8..offset + 12]),
                    BigEndian::read_u32(&rsp.payload[offset + 12..offset + 16]),
                ],
                vector: BigEndian::read_u32(&rsp.payload[offset + 16..offset + 20]),
                cycle: ((BigEndian::read_u32(&rsp.payload[offset + 24..offset + 28]) as u64) << 32)
                    | (BigEndian::read_u32(&rsp.payload[offset + 20..offset + 24]) as u64),
            });
            offset += 28;
        }

        Ok(ErrorLogResponse { total_errors, entries })
    }

    // =========================================================================
    // Flight Recorder
    // =========================================================================

    /// Get flight recorder log info
    pub fn get_log_info(&mut self, mac: &[u8; 6]) -> Result<LogInfo> {
        let rsp = self.send_req(mac, fbc_protocol::flight_recorder::LOG_INFO_REQ,
                                fbc_protocol::flight_recorder::LOG_INFO_RSP, &[], 500)?;
        if rsp.payload.len() < 21 {
            return Err(FbcError::Board("Log info response too short".into()));
        }
        Ok(LogInfo {
            sd_present: rsp.payload[0] != 0,
            boot_sector: BigEndian::read_u32(&rsp.payload[1..5]),
            log_start: BigEndian::read_u32(&rsp.payload[5..9]),
            log_end: BigEndian::read_u32(&rsp.payload[9..13]),
            current_index: BigEndian::read_u32(&rsp.payload[13..17]),
            total_entries: BigEndian::read_u32(&rsp.payload[17..21]),
        })
    }

    /// Read a flight recorder sector
    pub fn read_log_sector(&mut self, mac: &[u8; 6], sector: u32) -> Result<LogSector> {
        let rsp = self.send_req(mac, fbc_protocol::flight_recorder::LOG_READ_REQ,
                                fbc_protocol::flight_recorder::LOG_READ_RSP,
                                &sector.to_be_bytes(), 1000)?;
        if rsp.payload.len() < 5 {
            return Err(FbcError::Board("Log read response too short".into()));
        }
        Ok(LogSector {
            sector: BigEndian::read_u32(&rsp.payload[0..4]),
            status: rsp.payload[4],
            data: rsp.payload[5..].to_vec(),
        })
    }

    // =========================================================================
    // Firmware Update
    // =========================================================================

    /// Get firmware version info
    pub fn get_firmware_info(&mut self, mac: &[u8; 6]) -> Result<FirmwareInfo> {
        let rsp = self.send_req(mac, fbc_protocol::firmware::INFO_REQ,
                                fbc_protocol::firmware::INFO_RSP, &[], 500)?;
        if rsp.payload.len() < 20 {
            return Err(FbcError::Board("Firmware info response too short".into()));
        }
        Ok(FirmwareInfo {
            version_major: rsp.payload[0],
            version_minor: rsp.payload[1],
            version_patch: rsp.payload[2],
            build_date: String::from_utf8_lossy(&rsp.payload[3..13]).trim_end_matches('\0').to_string(),
            serial: BigEndian::read_u32(&rsp.payload[13..17]),
            hw_revision: rsp.payload[17],
            bootloader_version: rsp.payload[18],
            update_in_progress: (rsp.payload[19] & 0x01) != 0,
            sd_present: (rsp.payload[19] & 0x02) != 0,
        })
    }

    /// Full firmware update: begin → chunks → commit
    pub fn firmware_update(&mut self, mac: &[u8; 6], data: &[u8], checksum: u32) -> Result<FwCommitAck> {
        // BEGIN
        let mut begin_payload = [0u8; 8];
        BigEndian::write_u32(&mut begin_payload[0..4], data.len() as u32);
        BigEndian::write_u32(&mut begin_payload[4..8], checksum);

        let begin_rsp = self.send_req(mac, fbc_protocol::firmware::BEGIN,
                                      fbc_protocol::firmware::BEGIN_ACK, &begin_payload, 2000)?;
        if begin_rsp.payload.len() < 3 || begin_rsp.payload[0] != 0 {
            return Err(FbcError::Board(format!(
                "Firmware begin failed: status={}", begin_rsp.payload.get(0).unwrap_or(&0xFF)
            )));
        }
        let max_chunk = BigEndian::read_u16(&begin_rsp.payload[1..3]) as usize;
        let chunk_size = max_chunk.min(1024);

        // CHUNKS
        let mut offset = 0u32;
        while (offset as usize) < data.len() {
            let end = ((offset as usize) + chunk_size).min(data.len());
            let chunk = &data[offset as usize..end];

            let mut payload = Vec::with_capacity(6 + chunk.len());
            payload.extend_from_slice(&offset.to_be_bytes());
            payload.extend_from_slice(&(chunk.len() as u16).to_be_bytes());
            payload.extend_from_slice(chunk);

            let chunk_rsp = self.send_req(mac, fbc_protocol::firmware::CHUNK,
                                          fbc_protocol::firmware::CHUNK_ACK, &payload, 2000)?;
            if chunk_rsp.payload.len() >= 5 && chunk_rsp.payload[4] != 0 {
                return Err(FbcError::Board(format!(
                    "Firmware chunk failed at offset {}: status={}", offset, chunk_rsp.payload[4]
                )));
            }

            offset += chunk.len() as u32;
        }

        // COMMIT
        let commit_rsp = self.send_req(mac, fbc_protocol::firmware::COMMIT,
                                       fbc_protocol::firmware::COMMIT_ACK, &[], 10000)?;
        if commit_rsp.payload.len() < 9 {
            return Err(FbcError::Board("Firmware commit response too short".into()));
        }

        Ok(FwCommitAck {
            status: commit_rsp.payload[0],
            received_size: BigEndian::read_u32(&commit_rsp.payload[1..5]),
            computed_checksum: BigEndian::read_u32(&commit_rsp.payload[5..9]),
        })
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    /// Send a request and wait for a specific response command
    fn send_req(&mut self, mac: &[u8; 6], send_cmd: u8, recv_cmd: u8,
                payload: &[u8], timeout_ms: u64) -> Result<FbcPacket> {
        let seq = self.socket.next_seq();
        let packet = FbcPacket::with_payload(send_cmd, seq, payload.to_vec());
        self.socket.send(*mac, &packet)?;

        self.wait_for_cmd(mac, recv_cmd, Duration::from_millis(timeout_ms))?
            .ok_or(FbcError::Timeout)
    }

    /// Send a command and wait for same-cmd ACK
    fn send_cmd(&mut self, mac: &[u8; 6], cmd: u8, payload: &[u8]) -> Result<()> {
        let _ = self.socket.send_and_wait(*mac, cmd, payload.to_vec(), Duration::from_millis(500))?;
        Ok(())
    }

    /// Wait for a specific command response from a specific MAC
    fn wait_for_cmd(&mut self, mac: &[u8; 6], cmd: u8, timeout: Duration) -> Result<Option<FbcPacket>> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if let Some((src_mac, rsp)) = self.socket.recv_timeout(Duration::from_millis(10))? {
                if src_mac == *mac && rsp.header.cmd == cmd {
                    return Ok(Some(rsp));
                }
            }
        }
        Ok(None)
    }
}
