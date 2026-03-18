//! FBC Protocol - Raw Ethernet Frame Format
//!
//! No IP, no UDP, no TCP - just raw Ethernet frames.
//! EtherType: 0x88B5 (custom FBC protocol)
//!
//! Design principles:
//! - GUI has full control (no autonomous behavior)
//! - Controllers wait for commands
//! - Real-time monitoring via telemetry
//! - Takeover capability at any time

use core::mem::size_of;

/// FBC EtherType (custom protocol)
pub const ETHERTYPE_FBC: u16 = 0x88B5;

/// FBC Protocol Magic (validates FBC packets)
pub const FBC_MAGIC: u16 = 0xFBC0;

/// Maximum payload size (Ethernet MTU 1500 - 14 Ethernet - 8 FBC header)
pub const MAX_PAYLOAD: usize = 1478;

// =============================================================================
// Commands
// =============================================================================

/// Setup Phase Commands
pub mod setup {
    pub const ANNOUNCE:         u8 = 0x01;  // Controller → GUI (on boot)
    pub const BIM_STATUS_REQ:   u8 = 0x10;  // GUI → Controller
    pub const BIM_STATUS_RSP:   u8 = 0x11;  // Controller → GUI
    pub const WRITE_BIM:        u8 = 0x20;  // GUI → Controller
    pub const UPLOAD_VECTORS:   u8 = 0x21;  // GUI → Controller (chunked)
    pub const CONFIGURE:        u8 = 0x30;  // GUI → Controller
}

/// Runtime Commands
pub mod runtime {
    pub const START:            u8 = 0x40;  // GUI → Controller
    pub const STOP:             u8 = 0x41;  // GUI → Controller
    pub const RESET:            u8 = 0x42;  // GUI → Controller
    pub const HEARTBEAT:        u8 = 0x50;  // Controller → GUI
    pub const ERROR:            u8 = 0xE0;  // Controller → GUI
    pub const STATUS_REQ:       u8 = 0xF0;  // GUI → Controller
    pub const STATUS_RSP:       u8 = 0xF1;  // Controller → GUI
}

/// Flight Recorder Commands (SD card log retrieval)
pub mod flight_recorder {
    pub const LOG_READ_REQ:     u8 = 0x60;  // GUI → Controller (request log sector)
    pub const LOG_READ_RSP:     u8 = 0x61;  // Controller → GUI (log data)
    pub const LOG_INFO_REQ:     u8 = 0x62;  // GUI → Controller (request log info)
    pub const LOG_INFO_RSP:     u8 = 0x63;  // Controller → GUI (log metadata)
}

/// Firmware Update Commands (network reflash)
pub mod firmware {
    pub const INFO_REQ:         u8 = 0xE1;  // GUI → Controller (get current version)
    pub const INFO_RSP:         u8 = 0xE2;  // Controller → GUI (version + build info)
    pub const BEGIN:            u8 = 0xE3;  // GUI → Controller (start update, total_size)
    pub const BEGIN_ACK:        u8 = 0xE4;  // Controller → GUI (ready/error)
    pub const CHUNK:            u8 = 0xE5;  // GUI → Controller (offset + data)
    pub const CHUNK_ACK:        u8 = 0xE6;  // Controller → GUI (offset received)
    pub const COMMIT:           u8 = 0xE7;  // GUI → Controller (finalize, checksum)
    pub const COMMIT_ACK:       u8 = 0xE8;  // Controller → GUI (success, rebooting)
    pub const ABORT:            u8 = 0xE9;  // GUI → Controller (cancel update)
}

/// Analog Monitoring Commands (AnalogMonitor - 32ch XADC + MAX11131)
pub mod analog {
    pub const READ_ALL_REQ:     u8 = 0x70;  // GUI → Controller
    pub const READ_ALL_RSP:     u8 = 0x71;  // Controller → GUI (32 readings)
}

/// Power Control Commands (VicorController + PMBus)
pub mod power {
    pub const VICOR_STATUS_REQ: u8 = 0x80;  // GUI → Controller
    pub const VICOR_STATUS_RSP: u8 = 0x81;  // Controller → GUI (6 cores status)
    pub const VICOR_ENABLE:     u8 = 0x82;  // GUI → Controller (core_mask)
    pub const VICOR_SET_VOLTAGE:u8 = 0x83;  // GUI → Controller (core, mv)
    pub const PMBUS_STATUS_REQ: u8 = 0x84;  // GUI → Controller
    pub const PMBUS_STATUS_RSP: u8 = 0x85;  // Controller → GUI
    pub const PMBUS_ENABLE:     u8 = 0x86;  // GUI → Controller (addr, enable)
    pub const EMERGENCY_STOP:   u8 = 0x8F;  // GUI → Controller (disable all)
    pub const POWER_SEQUENCE_ON:u8 = 0x90;  // GUI → Controller (voltages[6])
    pub const POWER_SEQUENCE_OFF:u8 = 0x91; // GUI → Controller
}

/// EEPROM Commands (BimEeprom - 256 bytes)
pub mod eeprom {
    pub const READ_REQ:         u8 = 0xA0;  // GUI → Controller (offset, len)
    pub const READ_RSP:         u8 = 0xA1;  // Controller → GUI (data)
    pub const WRITE:            u8 = 0xA2;  // GUI → Controller (offset, data)
    pub const WRITE_ACK:        u8 = 0xA3;  // Controller → GUI (status)
}

/// Vector Engine Commands (extended control)
pub mod vector {
    pub const STATUS_REQ:       u8 = 0xB0;  // GUI → Controller
    pub const STATUS_RSP:       u8 = 0xB1;  // Controller → GUI
    pub const LOAD:             u8 = 0xB2;  // GUI → Controller (from SD cache)
    pub const LOAD_ACK:         u8 = 0xB3;  // Controller → GUI
    pub const START:            u8 = 0xB4;  // GUI → Controller
    pub const PAUSE:            u8 = 0xB5;  // GUI → Controller
    pub const RESUME:           u8 = 0xB6;  // GUI → Controller
    pub const STOP:             u8 = 0xB7;  // GUI → Controller
}

/// Fast Pin Commands (gpio[128:159] direct control)
pub mod fastpins {
    pub const READ_REQ:         u8 = 0xD0;  // GUI → Controller
    pub const READ_RSP:         u8 = 0xD1;  // Controller → GUI (32-bit state)
    pub const WRITE:            u8 = 0xD2;  // GUI → Controller (dout, oen)
}

/// Error Log Commands (read error BRAM contents)
pub mod error_log {
    pub const ERROR_LOG_REQ:    u8 = 0x4A;  // GUI → Controller (start_index, count)
    pub const ERROR_LOG_RSP:    u8 = 0x4B;  // Controller → GUI (error entries)
}

// =============================================================================
// FBC Packet Structure
// =============================================================================

/// FBC Header (8 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct FbcHeader {
    /// Magic number (0xFBC0)
    pub magic: u16,

    /// Sequence number (for duplicate detection)
    pub seq: u16,

    /// Command code
    pub cmd: u8,

    /// Flags (reserved for future use)
    pub flags: u8,

    /// Payload length
    pub length: u16,
}

impl FbcHeader {
    pub fn new(cmd: u8, seq: u16, payload_len: u16) -> Self {
        Self {
            magic: FBC_MAGIC,
            seq,
            cmd,
            flags: 0,
            length: payload_len,
        }
    }

    /// Serialize header to bytes (network byte order)
    pub fn to_bytes(&self) -> [u8; 8] {
        [
            (self.magic >> 8) as u8,
            (self.magic & 0xFF) as u8,
            (self.seq >> 8) as u8,
            (self.seq & 0xFF) as u8,
            self.cmd,
            self.flags,
            (self.length >> 8) as u8,
            (self.length & 0xFF) as u8,
        ]
    }

    /// Parse header from bytes
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        let magic = ((data[0] as u16) << 8) | (data[1] as u16);
        if magic != FBC_MAGIC {
            return None;
        }

        Some(Self {
            magic,
            seq: ((data[2] as u16) << 8) | (data[3] as u16),
            cmd: data[4],
            flags: data[5],
            length: ((data[6] as u16) << 8) | (data[7] as u16),
        })
    }
}

// =============================================================================
// Payloads
// =============================================================================

/// ANNOUNCE Payload (Controller → GUI on boot)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct AnnouncePayload {
    pub mac: [u8; 6],
    pub bim_type: u8,
    pub serial: u32,
    pub hw_revision: u8,
    pub fw_version: u16,
    pub has_bim: u8,         // 0=no BIM, 1=BIM detected
    pub bim_programmed: u8,  // 0=blank, 1=programmed
}

const _: () = assert!(size_of::<AnnouncePayload>() == 16);

impl AnnouncePayload {
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..6].copy_from_slice(&self.mac);
        buf[6] = self.bim_type;
        buf[7..11].copy_from_slice(&self.serial.to_be_bytes());
        buf[11] = self.hw_revision;
        buf[12..14].copy_from_slice(&self.fw_version.to_be_bytes());
        buf[14] = self.has_bim;
        buf[15] = self.bim_programmed;
        buf
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 16 {
            return None;
        }

        let mut mac = [0u8; 6];
        mac.copy_from_slice(&data[0..6]);

        Some(Self {
            mac,
            bim_type: data[6],
            serial: u32::from_be_bytes([data[7], data[8], data[9], data[10]]),
            hw_revision: data[11],
            fw_version: u16::from_be_bytes([data[12], data[13]]),
            has_bim: data[14],
            bim_programmed: data[15],
        })
    }
}

/// BIM_STATUS_RSP Payload (Controller → GUI)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct BimStatusPayload {
    pub has_bim: u8,
    pub bim_type: u8,
    pub serial: u32,
    pub hw_revision: u8,
    pub magic: u32,          // EEPROM magic (0xBEEFCAFE if valid)
    pub vector_set: u8,
    pub clock_freq_mhz: u16,
    pub error_threshold: u32,
}

const _: () = assert!(size_of::<BimStatusPayload>() == 18);

/// HEARTBEAT Payload (Controller → GUI during test)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct HeartbeatPayload {
    pub cycles: u32,
    pub errors: u32,
    pub temp_c: i16,         // Temperature × 10 (e.g., 452 = 45.2°C)
    pub state: u8,           // 0=IDLE, 1=RUNNING, 2=DONE, 3=ERROR
}

const _: () = assert!(size_of::<HeartbeatPayload>() == 11);

impl HeartbeatPayload {
    pub fn to_bytes(&self) -> [u8; 11] {
        let mut buf = [0u8; 11];
        buf[0..4].copy_from_slice(&self.cycles.to_be_bytes());
        buf[4..8].copy_from_slice(&self.errors.to_be_bytes());
        buf[8..10].copy_from_slice(&self.temp_c.to_be_bytes());
        buf[10] = self.state;
        buf
    }
}

/// STATUS_RSP Payload (Full telemetry)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct StatusPayload {
    pub cycles: u32,
    pub errors: u32,
    pub temp_c: i16,
    pub state: u8,
    pub rail_voltage: [u16; 8],  // mV (cores 1-6, then IO rails)
    pub rail_current: [u16; 8],  // mA (cores 1-6, then IO rails)
    pub fpga_vccint: u16,        // mV
    pub fpga_vccaux: u16,        // mV
}

const _: () = assert!(size_of::<StatusPayload>() == 47);

/// Configuration payload (GUI → Controller)
/// Sent with CONFIGURE command to set clock and voltage
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ConfigPayload {
    /// Clock divisor for FCLK0 (100MHz / divisor = vec_clk)
    /// 0 = no change, 1-255 = set divisor
    pub clock_div: u8,
    /// VICOR core voltages in mV (0 = no change, 500-1500 = set)
    pub core_voltage_mv: [u16; 6],
    /// Reserved for future use
    pub reserved: [u8; 5],
}

const _: () = assert!(size_of::<ConfigPayload>() == 18);

impl ConfigPayload {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 18 {
            return None;
        }

        let clock_div = data[0];
        let mut core_voltage_mv = [0u16; 6];
        for i in 0..6 {
            core_voltage_mv[i] = u16::from_be_bytes([data[1 + i*2], data[2 + i*2]]);
        }

        Some(Self {
            clock_div,
            core_voltage_mv,
            reserved: [0; 5],
        })
    }
}

/// Live telemetry data (updated by main loop, read by handler)
#[derive(Debug, Clone, Copy, Default)]
pub struct TelemetryData {
    /// VICOR core voltages in mV (from ADC, not setpoints)
    pub core_voltage_mv: [u16; 6],
    /// VICOR core currents in mA (from ADC shunt sense)
    pub core_current_ma: [u16; 6],
    /// IO rail voltages [VDD_IO, VDD_3V3, VDD_1V8, VDD_1V2, ...]
    pub io_voltage_mv: [u16; 4],
    /// Case/DUT temperatures in 0.1°C (from thermistors)
    pub case_temp_dc: i16,
    pub dut_temp_dc: i16,
}

impl StatusPayload {
    pub fn to_bytes(&self) -> [u8; 47] {
        let mut buf = [0u8; 47];
        let mut offset = 0;

        // Copy primitive fields (avoid taking references to packed fields)
        let cycles = self.cycles;
        let errors = self.errors;
        let temp_c = self.temp_c;
        let state = self.state;
        let fpga_vccint = self.fpga_vccint;
        let fpga_vccaux = self.fpga_vccaux;

        buf[offset..offset+4].copy_from_slice(&cycles.to_be_bytes());
        offset += 4;
        buf[offset..offset+4].copy_from_slice(&errors.to_be_bytes());
        offset += 4;
        buf[offset..offset+2].copy_from_slice(&temp_c.to_be_bytes());
        offset += 2;
        buf[offset] = state;
        offset += 1;

        // Copy arrays element by element
        for i in 0..8 {
            let v = self.rail_voltage[i];
            buf[offset..offset+2].copy_from_slice(&v.to_be_bytes());
            offset += 2;
        }
        for i in 0..8 {
            let current = self.rail_current[i];
            buf[offset..offset+2].copy_from_slice(&current.to_be_bytes());
            offset += 2;
        }

        buf[offset..offset+2].copy_from_slice(&fpga_vccint.to_be_bytes());
        offset += 2;
        buf[offset..offset+2].copy_from_slice(&fpga_vccaux.to_be_bytes());

        buf
    }
}

/// ERROR Payload
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ErrorPayload {
    pub error_type: u8,      // 0=vector mismatch, 1=power fault, 2=temp, etc.
    pub cycle: u32,
    pub error_count: u32,
    pub details: u32,        // Error-specific info
}

const _: () = assert!(size_of::<ErrorPayload>() == 13);

/// UPLOAD_VECTORS chunk
#[derive(Debug)]
pub struct VectorChunk {
    pub offset: u32,
    pub total_size: u32,
    pub chunk_size: u16,
    pub data: [u8; MAX_PAYLOAD - 10],
}

/// LOG_READ_REQ Payload (GUI → Controller)
/// Requests a specific SD card sector from the Flight Recorder
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct LogReadReqPayload {
    pub sector: u32,  // Sector number (1000=boot, 1001-2000=heartbeat circular buffer)
}

impl LogReadReqPayload {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        Some(Self {
            sector: u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
        })
    }
}

/// LOG_READ_RSP Payload (Controller → GUI)
/// Returns data from requested sector
#[derive(Debug)]
pub struct LogReadRspPayload {
    pub sector: u32,
    pub status: u8,      // 0=OK, 1=SD not present, 2=read error
    pub data: [u8; 512], // Full sector data
}

impl LogReadRspPayload {
    pub fn to_bytes(&self) -> [u8; 517] {
        let mut buf = [0u8; 517];
        buf[0..4].copy_from_slice(&self.sector.to_be_bytes());
        buf[4] = self.status;
        buf[5..517].copy_from_slice(&self.data);
        buf
    }
}

/// LOG_INFO_RSP Payload (Controller → GUI)
/// Returns Flight Recorder metadata
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct LogInfoRspPayload {
    pub sd_present: u8,       // 1 if SD card is present
    pub boot_sector: u32,     // Sector 1000 (boot log)
    pub log_start: u32,       // First log sector (1001)
    pub log_end: u32,         // Last log sector (2000)
    pub current_index: u32,   // Current write index in circular buffer
    pub total_entries: u32,   // Total heartbeats logged this session
}

impl LogInfoRspPayload {
    pub fn to_bytes(&self) -> [u8; 21] {
        let mut buf = [0u8; 21];
        buf[0] = self.sd_present;
        buf[1..5].copy_from_slice(&self.boot_sector.to_be_bytes());
        buf[5..9].copy_from_slice(&self.log_start.to_be_bytes());
        buf[9..13].copy_from_slice(&self.log_end.to_be_bytes());
        buf[13..17].copy_from_slice(&self.current_index.to_be_bytes());
        buf[17..21].copy_from_slice(&self.total_entries.to_be_bytes());
        buf
    }
}

// =============================================================================
// Firmware Update Payloads
// =============================================================================

/// Firmware version info
pub const FW_VERSION_MAJOR: u8 = 1;
pub const FW_VERSION_MINOR: u8 = 0;
pub const FW_VERSION_PATCH: u8 = 0;
pub const FW_BUILD_DATE: &[u8; 10] = b"2026-02-10";

/// FIRMWARE_INFO_RSP Payload (Controller → GUI)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct FirmwareInfoRspPayload {
    pub version_major: u8,
    pub version_minor: u8,
    pub version_patch: u8,
    pub build_date: [u8; 10],  // "YYYY-MM-DD"
    pub board_serial: u32,
    pub hw_revision: u8,
    pub bootloader_version: u8,
    pub sd_present: u8,
    pub update_in_progress: u8,
}

impl FirmwareInfoRspPayload {
    pub fn to_bytes(&self) -> [u8; 20] {
        let mut buf = [0u8; 20];
        buf[0] = self.version_major;
        buf[1] = self.version_minor;
        buf[2] = self.version_patch;
        buf[3..13].copy_from_slice(&self.build_date);
        buf[13..17].copy_from_slice(&self.board_serial.to_be_bytes());
        buf[17] = self.hw_revision;
        buf[18] = self.bootloader_version;
        buf[19] = ((self.sd_present & 1) << 1) | (self.update_in_progress & 1);
        buf
    }
}

/// FIRMWARE_BEGIN Payload (GUI → Controller)
#[derive(Debug, Clone, Copy)]
pub struct FirmwareBeginPayload {
    pub total_size: u32,   // Total firmware size in bytes
    pub checksum: u32,     // CRC32 of entire firmware
}

impl FirmwareBeginPayload {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        Some(Self {
            total_size: u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
            checksum: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
        })
    }
}

/// FIRMWARE_BEGIN_ACK Payload (Controller → GUI)
#[derive(Debug, Clone, Copy)]
pub struct FirmwareBeginAckPayload {
    pub status: u8,        // 0=ready, 1=no SD, 2=SD write error, 3=already in progress
    pub max_chunk_size: u16,
}

impl FirmwareBeginAckPayload {
    pub fn to_bytes(&self) -> [u8; 3] {
        let mut buf = [0u8; 3];
        buf[0] = self.status;
        buf[1..3].copy_from_slice(&self.max_chunk_size.to_be_bytes());
        buf
    }
}

/// FIRMWARE_CHUNK Payload (GUI → Controller)
/// Header only - data follows in packet payload
#[derive(Debug, Clone, Copy)]
pub struct FirmwareChunkPayload {
    pub offset: u32,       // Offset in firmware image
    pub size: u16,         // Size of this chunk
    // data[size] follows
}

impl FirmwareChunkPayload {
    pub fn from_bytes(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < 6 {
            return None;
        }
        let header = Self {
            offset: u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
            size: u16::from_be_bytes([data[4], data[5]]),
        };
        let chunk_data = &data[6..];
        Some((header, chunk_data))
    }
}

/// FIRMWARE_CHUNK_ACK Payload (Controller → GUI)
#[derive(Debug, Clone, Copy)]
pub struct FirmwareChunkAckPayload {
    pub offset: u32,       // Offset that was written
    pub status: u8,        // 0=OK, 1=write error, 2=offset mismatch
}

impl FirmwareChunkAckPayload {
    pub fn to_bytes(&self) -> [u8; 5] {
        let mut buf = [0u8; 5];
        buf[0..4].copy_from_slice(&self.offset.to_be_bytes());
        buf[4] = self.status;
        buf
    }
}

/// FIRMWARE_COMMIT_ACK Payload (Controller → GUI)
#[derive(Debug, Clone, Copy)]
pub struct FirmwareCommitAckPayload {
    pub status: u8,        // 0=success (rebooting), 1=checksum mismatch, 2=incomplete
    pub received_size: u32,
    pub computed_checksum: u32,
}

impl FirmwareCommitAckPayload {
    pub fn to_bytes(&self) -> [u8; 9] {
        let mut buf = [0u8; 9];
        buf[0] = self.status;
        buf[1..5].copy_from_slice(&self.received_size.to_be_bytes());
        buf[5..9].copy_from_slice(&self.computed_checksum.to_be_bytes());
        buf
    }
}

// =============================================================================
// Controller State
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ControllerState {
    Idle = 0,
    Running = 1,
    Done = 2,
    Error = 3,
}

// =============================================================================
// Packet Builder
// =============================================================================

pub struct FbcPacket {
    pub header: FbcHeader,
    pub payload: [u8; MAX_PAYLOAD],
    pub payload_len: usize,
}

impl Default for FbcPacket {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl FbcPacket {
    pub fn new(cmd: u8, seq: u16) -> Self {
        Self {
            header: FbcHeader::new(cmd, seq, 0),
            payload: [0; MAX_PAYLOAD],
            payload_len: 0,
        }
    }

    pub fn with_payload(cmd: u8, seq: u16, payload: &[u8]) -> Self {
        let mut pkt = Self::new(cmd, seq);
        let len = payload.len().min(MAX_PAYLOAD);
        pkt.payload[..len].copy_from_slice(&payload[..len]);
        pkt.payload_len = len;
        pkt.header.length = len as u16;
        pkt
    }

    /// Serialize to complete FBC packet (header + payload)
    pub fn serialize(&self, buf: &mut [u8]) -> usize {
        let header_bytes = self.header.to_bytes();
        let total_len = 8 + self.payload_len;

        if buf.len() < total_len {
            return 0;
        }

        buf[0..8].copy_from_slice(&header_bytes);
        buf[8..total_len].copy_from_slice(&self.payload[..self.payload_len]);

        total_len
    }

    /// Parse from raw bytes
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        let header = FbcHeader::from_bytes(data)?;
        let payload_len = header.length as usize;

        if data.len() < 8 + payload_len {
            return None;
        }

        let mut payload = [0u8; MAX_PAYLOAD];
        payload[..payload_len].copy_from_slice(&data[8..8 + payload_len]);

        Some(Self {
            header,
            payload,
            payload_len,
        })
    }
}

// =============================================================================
// FBC Protocol Handler (Raw Ethernet)
// =============================================================================

use crate::dma::{DmaResult, FbcStreamer};
use crate::regs::{FbcCtrl, PinCtrl, VectorStatus};
use crate::hal::Xadc;

/// Configuration result returned by handle_configure
#[derive(Debug, Clone, Copy)]
pub struct ConfigResult {
    /// Clock divisor to apply (0 = no change)
    pub clock_div: u8,
    /// Core voltages to apply in mV (0 = no change)
    pub core_voltage_mv: [u16; 6],
}

/// Pending log read request (main.rs reads SD and calls build_log_response)
#[derive(Debug, Clone, Copy)]
pub struct PendingLogRead {
    pub sector: u32,
}

/// Pending log info request
#[derive(Debug, Clone, Copy)]
pub struct PendingLogInfo;

/// Pending analog read request
#[derive(Debug, Clone, Copy)]
pub struct PendingAnalogRead;

/// Pending VICOR command
#[derive(Debug, Clone, Copy)]
pub enum PendingVicor {
    StatusReq,
    Enable { core_mask: u8 },
    SetVoltage { core: u8, mv: u16 },
    EmergencyStop,
    PowerSequenceOn { voltages_mv: [u16; 6] },
    PowerSequenceOff,
}

/// Pending PMBus command
#[derive(Debug, Clone, Copy)]
pub struct PendingPmbus {
    pub addr: u8,
    pub enable: bool,
}

/// Pending EEPROM command
#[derive(Debug, Clone, Copy)]
pub enum PendingEeprom {
    Read { offset: u8, len: u8 },
    Write { offset: u8, len: u8, data: [u8; 64] },
}

/// Pending fast pins command
#[derive(Debug, Clone, Copy)]
pub enum PendingFastPins {
    Read,
    Write { dout: u32, oen: u32 },
}

/// Error log entry (28 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ErrorLogEntry {
    pub pattern: [u32; 4],  // 128-bit pattern value
    pub vector: u32,        // Vector number when error occurred
    pub cycle_lo: u32,      // Cycle count low
    pub cycle_hi: u32,      // Cycle count high
}

impl ErrorLogEntry {
    pub fn to_bytes(&self) -> [u8; 28] {
        let mut buf = [0u8; 28];
        buf[0..4].copy_from_slice(&self.pattern[0].to_be_bytes());
        buf[4..8].copy_from_slice(&self.pattern[1].to_be_bytes());
        buf[8..12].copy_from_slice(&self.pattern[2].to_be_bytes());
        buf[12..16].copy_from_slice(&self.pattern[3].to_be_bytes());
        buf[16..20].copy_from_slice(&self.vector.to_be_bytes());
        buf[20..24].copy_from_slice(&self.cycle_lo.to_be_bytes());
        buf[24..28].copy_from_slice(&self.cycle_hi.to_be_bytes());
        buf
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 28 {
            return None;
        }
        Some(Self {
            pattern: [
                u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
                u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
                u32::from_be_bytes([data[8], data[9], data[10], data[11]]),
                u32::from_be_bytes([data[12], data[13], data[14], data[15]]),
            ],
            vector: u32::from_be_bytes([data[16], data[17], data[18], data[19]]),
            cycle_lo: u32::from_be_bytes([data[20], data[21], data[22], data[23]]),
            cycle_hi: u32::from_be_bytes([data[24], data[25], data[26], data[27]]),
        })
    }
}

/// Error log request payload (8 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ErrorLogReqPayload {
    pub start_index: u32,
    pub count: u32,
}

impl ErrorLogReqPayload {
    pub fn to_bytes(&self) -> [u8; 8] {
        [
            (self.start_index >> 24) as u8,
            (self.start_index >> 16) as u8,
            (self.start_index >> 8) as u8,
            (self.start_index) as u8,
            (self.count >> 24) as u8,
            (self.count >> 16) as u8,
            (self.count >> 8) as u8,
            (self.count) as u8,
        ]
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        Some(Self {
            start_index: u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
            count: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
        })
    }
}

/// Error log response payload (8 + 28*N bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ErrorLogRspPayload {
    pub total_errors: u32,
    pub num_entries: u32,
    pub entries: [ErrorLogEntry; 8],  // Max 8 entries per response
}

impl ErrorLogRspPayload {
    pub fn to_bytes(&self) -> [u8; 232] {
        let mut buf = [0u8; 232];
        buf[0..4].copy_from_slice(&self.total_errors.to_be_bytes());
        buf[4..8].copy_from_slice(&self.num_entries.to_be_bytes());
        for (i, entry) in self.entries.iter().enumerate() {
            buf[8 + i * 28..8 + (i + 1) * 28].copy_from_slice(&entry.to_bytes());
        }
        buf
    }
}

/// Pending error log request
#[derive(Debug, Clone, Copy)]
pub struct PendingErrorLog {
    pub start_index: u32,
    pub count: u32,
}

/// FBC Protocol Handler - processes raw Ethernet FBC commands
pub struct FbcProtocolHandler {
    state: ControllerState,
    seq: u16,
    streamer: FbcStreamer,
    fbc: FbcCtrl,
    pins: PinCtrl,
    status: VectorStatus,
    xadc: Xadc,
    // Vector upload state
    upload_offset: u32,
    upload_total: u32,
    upload_buf: [u8; 65536],
    // Identity for discovery responses
    mac: [u8; 6],
    serial: u32,
    fw_version: u16,
    // BIM (Board Interface Module) status from EEPROM
    has_bim: bool,
    bim_programmed: bool,
    bim_type: u8,
    // Live telemetry (updated externally via update_telemetry())
    telemetry: TelemetryData,
    // Pending configuration (set by CONFIGURE, applied by main loop)
    pending_config: Option<ConfigResult>,
    // Pending log read request (main.rs reads SD and calls build_log_response)
    pending_log_read: Option<PendingLogRead>,
    // Pending log info request
    pending_log_info: bool,
    // Flight Recorder state (tracked for LOG_INFO responses)
    pub log_index: u32,  // Current write index (public so main.rs can update)
    // Pending requests for drivers (main.rs fulfills these)
    pending_analog_read: bool,
    pending_vicor: Option<PendingVicor>,
    pending_pmbus: Option<PendingPmbus>,
    pending_eeprom: Option<PendingEeprom>,
    pending_fastpins: Option<PendingFastPins>,
    pending_error_log: Option<PendingErrorLog>,
    // Firmware update state
    fw_update_in_progress: bool,
    fw_update_total_size: u32,
    fw_update_expected_checksum: u32,
    fw_update_received: u32,
    fw_update_running_checksum: u32,
    // Pending firmware update operations (main.rs handles SD writes)
    pub pending_fw_info: bool,
    pub pending_fw_begin: Option<PendingFwBegin>,
    pub pending_fw_chunk: Option<PendingFwChunk>,
    pub pending_fw_commit: bool,
}

/// Pending firmware update begin request
#[derive(Clone, Copy)]
pub struct PendingFwBegin {
    pub total_size: u32,
    pub checksum: u32,
}

/// Pending firmware update chunk
pub struct PendingFwChunk {
    pub offset: u32,
    pub size: u16,
    pub data: [u8; 1024],  // Max chunk size
}

impl FbcProtocolHandler {
    pub const fn new(mac: [u8; 6], serial: u32, fw_version: u16) -> Self {
        Self {
            state: ControllerState::Idle,
            seq: 0,
            streamer: FbcStreamer::new(),
            fbc: FbcCtrl::new(),
            pins: PinCtrl::new(),
            status: VectorStatus::new(),
            xadc: Xadc::new(),
            upload_offset: 0,
            upload_total: 0,
            upload_buf: [0u8; 65536],
            mac,
            serial,
            fw_version,
            has_bim: false,
            bim_programmed: false,
            bim_type: 0,
            telemetry: TelemetryData {
                core_voltage_mv: [0; 6],
                core_current_ma: [0; 6],
                io_voltage_mv: [0; 4],
                case_temp_dc: 0,
                dut_temp_dc: 0,
            },
            pending_config: None,
            pending_log_read: None,
            pending_log_info: false,
            log_index: 0,
            pending_analog_read: false,
            pending_vicor: None,
            pending_pmbus: None,
            pending_eeprom: None,
            pending_fastpins: None,
            pending_error_log: None,
            fw_update_in_progress: false,
            fw_update_total_size: 0,
            fw_update_expected_checksum: 0,
            fw_update_received: 0,
            fw_update_running_checksum: 0,
            pending_fw_info: false,
            pending_fw_begin: None,
            pending_fw_chunk: None,
            pending_fw_commit: false,
        }
    }

    /// Set BIM (Board Interface Module) status from EEPROM check
    ///
    /// # Arguments
    /// * `has_bim` - true if EEPROM I2C device responded
    /// * `bim_programmed` - true if EEPROM has valid magic (0xBEEF_CAFE)
    /// * `bim_type` - BIM type from EEPROM (0 if not programmed)
    /// * `serial` - Serial number from EEPROM (overrides DNA serial if programmed)
    pub fn set_bim_info(&mut self, has_bim: bool, bim_programmed: bool, bim_type: u8, serial: Option<u32>) {
        self.has_bim = has_bim;
        self.bim_programmed = bim_programmed;
        self.bim_type = bim_type;
        if let Some(s) = serial {
            self.serial = s;
        }
    }

    /// Initialize the handler
    pub fn init(&mut self) {
        self.streamer.init();
        self.state = ControllerState::Idle;
        self.seq = 0;
        self.pending_config = None;
        self.pending_log_read = None;
        self.pending_log_info = false;
        self.log_index = 0;
    }

    /// Update live telemetry data (called by main loop after reading ADCs)
    pub fn update_telemetry(&mut self, data: TelemetryData) {
        self.telemetry = data;
    }

    /// Get and clear pending configuration (main loop applies then clears)
    pub fn take_pending_config(&mut self) -> Option<ConfigResult> {
        self.pending_config.take()
    }

    /// Check if there's pending configuration to apply
    pub fn has_pending_config(&self) -> bool {
        self.pending_config.is_some()
    }

    /// Get and clear pending log read request (main loop reads SD and calls build_log_read_response)
    pub fn take_pending_log_read(&mut self) -> Option<PendingLogRead> {
        self.pending_log_read.take()
    }

    /// Check if there's a pending log info request
    pub fn take_pending_log_info(&mut self) -> bool {
        let pending = self.pending_log_info;
        self.pending_log_info = false;
        pending
    }

    /// Get and clear pending analog read request
    pub fn take_pending_analog_read(&mut self) -> bool {
        let pending = self.pending_analog_read;
        self.pending_analog_read = false;
        pending
    }

    /// Get and clear pending VICOR command
    pub fn take_pending_vicor(&mut self) -> Option<PendingVicor> {
        self.pending_vicor.take()
    }

    /// Get and clear pending PMBus command
    pub fn take_pending_pmbus(&mut self) -> Option<PendingPmbus> {
        self.pending_pmbus.take()
    }

    /// Get and clear pending EEPROM command
    pub fn take_pending_eeprom(&mut self) -> Option<PendingEeprom> {
        self.pending_eeprom.take()
    }

    /// Get and clear pending fast pins command
    pub fn take_pending_fastpins(&mut self) -> Option<PendingFastPins> {
        self.pending_fastpins.take()
    }

    /// Get and clear pending error log request
    pub fn take_pending_error_log(&mut self) -> Option<PendingErrorLog> {
        self.pending_error_log.take()
    }

    /// Build ERROR_LOG_RSP packet (called by main.rs after reading error BRAM)
    pub fn build_error_log_response(
        &mut self,
        total_errors: u32,
        entries: &[ErrorLogEntry],
    ) -> FbcPacket {
        let mut payload = ErrorLogRspPayload {
            total_errors,
            num_entries: entries.len() as u32,
            entries: [ErrorLogEntry {
                pattern: [0; 4],
                vector: 0,
                cycle_lo: 0,
                cycle_hi: 0,
            }; 8],
        };
        for (i, entry) in entries.iter().take(8).enumerate() {
            payload.entries[i] = *entry;
        }
        FbcPacket::with_payload(error_log::ERROR_LOG_RSP, self.next_seq(), &payload.to_bytes())
    }

    /// Build LOG_READ_RSP packet (called by main.rs after reading SD)
    pub fn build_log_read_response(&mut self, sector: u32, status: u8, data: &[u8; 512]) -> FbcPacket {
        let mut payload = [0u8; 517];
        payload[0..4].copy_from_slice(&sector.to_be_bytes());
        payload[4] = status;
        payload[5..517].copy_from_slice(data);
        FbcPacket::with_payload(flight_recorder::LOG_READ_RSP, self.next_seq(), &payload)
    }

    /// Build LOG_INFO_RSP packet (called by main.rs)
    pub fn build_log_info_response(&mut self, sd_present: bool) -> FbcPacket {
        let info = LogInfoRspPayload {
            sd_present: if sd_present { 1 } else { 0 },
            boot_sector: 1000,
            log_start: 1001,
            log_end: 2000,
            current_index: self.log_index,
            total_entries: self.log_index,  // Simple: index = entries this session
        };
        FbcPacket::with_payload(flight_recorder::LOG_INFO_RSP, self.next_seq(), &info.to_bytes())
    }

    /// Build ANALOG_READ_RSP packet (called by main.rs after reading AnalogMonitor)
    /// readings: 32 values, each as (raw_u16, scaled_i32 in 0.001 units)
    pub fn build_analog_response(&mut self, readings: &[(u16, i32); 32]) -> FbcPacket {
        // Payload: 32 * (2 bytes raw + 4 bytes scaled) = 192 bytes
        let mut payload = [0u8; 192];
        for (i, (raw, scaled)) in readings.iter().enumerate() {
            let offset = i * 6;
            payload[offset..offset+2].copy_from_slice(&raw.to_be_bytes());
            payload[offset+2..offset+6].copy_from_slice(&scaled.to_be_bytes());
        }
        FbcPacket::with_payload(analog::READ_ALL_RSP, self.next_seq(), &payload)
    }

    /// Build VICOR_STATUS_RSP packet (called by main.rs)
    /// status: 6 cores, each (enabled: bool, voltage_mv: u16, current_ma: u16)
    pub fn build_vicor_status_response(&mut self, status: &[(bool, u16, u16); 6]) -> FbcPacket {
        // Payload: 6 * (1 byte enabled + 2 bytes voltage + 2 bytes current) = 30 bytes
        let mut payload = [0u8; 30];
        for (i, (enabled, voltage, current)) in status.iter().enumerate() {
            let offset = i * 5;
            payload[offset] = if *enabled { 1 } else { 0 };
            payload[offset+1..offset+3].copy_from_slice(&voltage.to_be_bytes());
            payload[offset+3..offset+5].copy_from_slice(&current.to_be_bytes());
        }
        FbcPacket::with_payload(power::VICOR_STATUS_RSP, self.next_seq(), &payload)
    }

    /// Build EEPROM_READ_RSP packet (called by main.rs)
    pub fn build_eeprom_read_response(&mut self, offset: u8, data: &[u8]) -> FbcPacket {
        let mut payload = [0u8; 66]; // 1 offset + 1 len + 64 data max
        payload[0] = offset;
        payload[1] = data.len() as u8;
        let len = data.len().min(64);
        payload[2..2+len].copy_from_slice(&data[..len]);
        FbcPacket::with_payload(eeprom::READ_RSP, self.next_seq(), &payload[..2+len])
    }

    /// Build EEPROM_WRITE_ACK packet (called by main.rs)
    pub fn build_eeprom_write_ack(&mut self, success: bool) -> FbcPacket {
        let payload = [if success { 0 } else { 1 }];
        FbcPacket::with_payload(eeprom::WRITE_ACK, self.next_seq(), &payload)
    }

    /// Build FASTPINS_READ_RSP packet (called by main.rs)
    pub fn build_fastpins_response(&mut self, din: u32, dout: u32, oen: u32) -> FbcPacket {
        let mut payload = [0u8; 12];
        payload[0..4].copy_from_slice(&din.to_be_bytes());
        payload[4..8].copy_from_slice(&dout.to_be_bytes());
        payload[8..12].copy_from_slice(&oen.to_be_bytes());
        FbcPacket::with_payload(fastpins::READ_RSP, self.next_seq(), &payload)
    }

    /// Get next sequence number
    fn next_seq(&mut self) -> u16 {
        let s = self.seq;
        self.seq = self.seq.wrapping_add(1);
        s
    }

    /// Get current state
    pub fn state(&self) -> ControllerState {
        self.state
    }

    /// Process incoming FBC packet
    ///
    /// Returns Some(response_packet) if a response should be sent
    pub fn process(&mut self, packet: &FbcPacket) -> Option<FbcPacket> {
        let cmd = packet.header.cmd;
        let payload = &packet.payload[..packet.payload_len];

        match cmd {
            // Setup commands
            // Host uses BIM_STATUS_REQ for discovery, expects ANNOUNCE response
            setup::BIM_STATUS_REQ => self.handle_discovery(),
            setup::CONFIGURE => self.handle_configure(payload),
            setup::UPLOAD_VECTORS => self.handle_upload_vectors(payload),

            // Runtime commands
            runtime::START => self.handle_start(),
            runtime::STOP => self.handle_stop(),
            runtime::RESET => self.handle_reset(),
            runtime::STATUS_REQ => self.handle_status_req(),

            // Flight Recorder commands (responses built by main.rs after SD access)
            flight_recorder::LOG_READ_REQ => self.handle_log_read_req(payload),
            flight_recorder::LOG_INFO_REQ => self.handle_log_info_req(),

            // Analog monitoring (responses built by main.rs after reading AnalogMonitor)
            analog::READ_ALL_REQ => self.handle_analog_read_req(),

            // Power control (responses built by main.rs with VicorController/PMBus)
            power::VICOR_STATUS_REQ => self.handle_vicor_status_req(),
            power::VICOR_ENABLE => self.handle_vicor_enable(payload),
            power::VICOR_SET_VOLTAGE => self.handle_vicor_set_voltage(payload),
            power::EMERGENCY_STOP => self.handle_emergency_stop(),
            power::POWER_SEQUENCE_ON => self.handle_power_sequence_on(payload),
            power::POWER_SEQUENCE_OFF => self.handle_power_sequence_off(),
            power::PMBUS_ENABLE => self.handle_pmbus_enable(payload),

            // EEPROM commands (responses built by main.rs with BimEeprom)
            eeprom::READ_REQ => self.handle_eeprom_read(payload),
            eeprom::WRITE => self.handle_eeprom_write(payload),

            // Fast pins (direct FPGA gpio[128:159])
            fastpins::READ_REQ => self.handle_fastpins_read(),
            fastpins::WRITE => self.handle_fastpins_write(payload),

            // Error log (read error BRAM contents)
            error_log::ERROR_LOG_REQ => self.handle_error_log_req(payload),

            // Firmware update commands
            firmware::INFO_REQ => self.handle_fw_info_req(),
            firmware::BEGIN => self.handle_fw_begin(payload),
            firmware::CHUNK => self.handle_fw_chunk(payload),
            firmware::COMMIT => self.handle_fw_commit(),
            firmware::ABORT => self.handle_fw_abort(),

            _ => None,  // Unknown command, no response
        }
    }

    /// Poll for state changes
    pub fn poll(&mut self) {
        match self.state {
            ControllerState::Running => {
                if self.fbc.is_done() {
                    self.state = if self.fbc.has_error() {
                        ControllerState::Error
                    } else {
                        ControllerState::Done
                    };
                }
            }
            _ => {}
        }
    }

    /// Build an ANNOUNCE packet (sent on boot or discovery)
    pub fn build_announce(&mut self) -> FbcPacket {
        let announce = AnnouncePayload {
            mac: self.mac,
            bim_type: self.bim_type,
            serial: self.serial,
            hw_revision: 1,
            fw_version: self.fw_version,
            has_bim: if self.has_bim { 1 } else { 0 },
            bim_programmed: if self.bim_programmed { 1 } else { 0 },
        };

        FbcPacket::with_payload(setup::ANNOUNCE, self.next_seq(), &announce.to_bytes())
    }

    /// Build a HEARTBEAT packet (sent periodically during test)
    pub fn build_heartbeat(&mut self) -> FbcPacket {
        let cycles = self.status.get_cycle_count() as u32;
        let errors = self.status.get_error_count();
        // Get temperature in 0.1°C units (e.g., 452 = 45.2°C)
        let temp_c = self.xadc.read_temperature_millicelsius()
            .map(|mc| (mc / 100) as i16)  // Convert mC to 0.1°C
            .unwrap_or(0);

        let heartbeat = HeartbeatPayload {
            cycles,
            errors,
            temp_c,
            state: self.state as u8,
        };

        FbcPacket::with_payload(runtime::HEARTBEAT, self.next_seq(), &heartbeat.to_bytes())
    }

    /// Build an ERROR packet
    pub fn build_error(&mut self, error_type: u8, cycle: u32, details: u32) -> FbcPacket {
        let error_count = self.status.get_error_count();
        let payload = [
            error_type,
            (cycle >> 24) as u8,
            (cycle >> 16) as u8,
            (cycle >> 8) as u8,
            cycle as u8,
            (error_count >> 24) as u8,
            (error_count >> 16) as u8,
            (error_count >> 8) as u8,
            error_count as u8,
            (details >> 24) as u8,
            (details >> 16) as u8,
            (details >> 8) as u8,
            details as u8,
        ];

        FbcPacket::with_payload(runtime::ERROR, self.next_seq(), &payload)
    }

    // =========================================================================
    // Command Handlers
    // =========================================================================

    fn handle_discovery(&mut self) -> Option<FbcPacket> {
        Some(self.build_announce())
    }

    fn handle_configure(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        // Parse ConfigPayload from incoming data
        let config = ConfigPayload::from_bytes(payload)?;

        // Copy packed fields to local variables to avoid unaligned access
        let clock_div = config.clock_div;
        let core_voltage_mv = config.core_voltage_mv;

        // Validate voltage ranges (500-1500 mV, or 0 for no change)
        for &mv in &core_voltage_mv {
            if mv != 0 && (mv < 500 || mv > 1500) {
                // Invalid voltage - send error response
                return Some(FbcPacket::new(runtime::ERROR, self.next_seq()));
            }
        }

        // Store pending configuration for main loop to apply
        // (main loop has access to SLCR and VicorController)
        self.pending_config = Some(ConfigResult {
            clock_div,
            core_voltage_mv,
        });

        // ACK the configuration (main loop will apply it)
        Some(FbcPacket::new(setup::CONFIGURE, self.next_seq()))
    }

    fn handle_upload_vectors(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        // Chunk format: offset(4) + total_size(4) + chunk_size(2) + data
        if payload.len() < 10 {
            return None;
        }

        let offset = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
        let total = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
        let chunk_size = u16::from_be_bytes([payload[8], payload[9]]) as usize;

        if payload.len() < 10 + chunk_size {
            return None;
        }

        // Store chunk
        let start = offset as usize;
        let end = start + chunk_size;
        if end > self.upload_buf.len() {
            return None;
        }

        self.upload_buf[start..end].copy_from_slice(&payload[10..10 + chunk_size]);
        self.upload_offset = offset + chunk_size as u32;
        self.upload_total = total;

        // Check if upload complete
        if self.upload_offset >= self.upload_total {
            // Stream to FPGA
            let program = &self.upload_buf[..self.upload_total as usize];
            match self.streamer.stream_program(program) {
                DmaResult::Ok => {
                    self.upload_offset = 0;
                    self.upload_total = 0;
                }
                _ => {
                    self.state = ControllerState::Error;
                }
            }
        }

        // ACK the chunk
        Some(FbcPacket::new(setup::UPLOAD_VECTORS, self.next_seq()))
    }

    fn handle_start(&mut self) -> Option<FbcPacket> {
        if self.state == ControllerState::Running {
            return None;  // Already running
        }

        // Enable FBC decoder
        self.fbc.enable();

        // Enable interrupts from FPGA (irq_done and irq_error)
        // These are wired to Cortex-A9 GIC interrupt input
        self.fbc.enable_irq();

        self.state = ControllerState::Running;

        Some(FbcPacket::new(runtime::START, self.next_seq()))
    }

    fn handle_stop(&mut self) -> Option<FbcPacket> {
        self.fbc.disable();
        self.state = ControllerState::Idle;

        Some(FbcPacket::new(runtime::STOP, self.next_seq()))
    }

    fn handle_reset(&mut self) -> Option<FbcPacket> {
        self.fbc.reset();
        self.streamer.init();
        self.state = ControllerState::Idle;
        self.upload_offset = 0;
        self.upload_total = 0;

        Some(FbcPacket::new(runtime::RESET, self.next_seq()))
    }

    fn handle_status_req(&mut self) -> Option<FbcPacket> {
        let cycles = self.status.get_cycle_count() as u32;
        let errors = self.status.get_error_count();

        // Get die temperature in 0.1°C units from XADC
        let temp_c = self.xadc.read_temperature_millicelsius()
            .map(|mc| (mc / 100) as i16)
            .unwrap_or(0);

        // Build rail_voltage array from live telemetry
        // [Core1, Core2, Core3, Core4, Core5, Core6, VDD_IO, VDD_3V3]
        let mut rail_voltage = [0u16; 8];
        for i in 0..6 {
            rail_voltage[i] = self.telemetry.core_voltage_mv[i];
        }
        if self.telemetry.io_voltage_mv.len() >= 2 {
            rail_voltage[6] = self.telemetry.io_voltage_mv[0]; // VDD_IO
            rail_voltage[7] = self.telemetry.io_voltage_mv[1]; // VDD_3V3
        }

        // Build rail_current array from live telemetry
        // [Core1, Core2, Core3, Core4, Core5, Core6, 0, 0]
        let mut rail_current = [0u16; 8];
        for i in 0..6 {
            rail_current[i] = self.telemetry.core_current_ma[i];
        }

        let status = StatusPayload {
            cycles,
            errors,
            temp_c,
            state: self.state as u8,
            rail_voltage,
            rail_current,
            fpga_vccint: self.xadc.read_vccint_mv().unwrap_or(0) as u16,
            fpga_vccaux: self.xadc.read_vccaux_mv().unwrap_or(0) as u16,
        };

        Some(FbcPacket::with_payload(runtime::STATUS_RSP, self.next_seq(), &status.to_bytes()))
    }

    // =========================================================================
    // Flight Recorder Handlers
    // =========================================================================

    fn handle_log_read_req(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        // Parse the request to get sector number
        let req = LogReadReqPayload::from_bytes(payload)?;

        // Validate sector range (boot=1000, logs=1001-2000)
        if req.sector < 1000 || req.sector > 2000 {
            return None;
        }

        // Store pending request - main.rs will read SD and send response
        self.pending_log_read = Some(PendingLogRead { sector: req.sector });

        // No immediate response - main.rs builds and sends it after SD read
        None
    }

    fn handle_log_info_req(&mut self) -> Option<FbcPacket> {
        // Set flag for main.rs to send response (it knows SD state)
        self.pending_log_info = true;

        // No immediate response - main.rs builds and sends it
        None
    }

    // =========================================================================
    // Analog Monitoring Handlers
    // =========================================================================

    fn handle_analog_read_req(&mut self) -> Option<FbcPacket> {
        self.pending_analog_read = true;
        None // main.rs builds response with AnalogMonitor
    }

    // =========================================================================
    // Power Control Handlers
    // =========================================================================

    fn handle_vicor_status_req(&mut self) -> Option<FbcPacket> {
        self.pending_vicor = Some(PendingVicor::StatusReq);
        None // main.rs builds response with VicorController
    }

    fn handle_vicor_enable(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        if payload.is_empty() {
            return None;
        }
        self.pending_vicor = Some(PendingVicor::Enable { core_mask: payload[0] });
        None
    }

    fn handle_vicor_set_voltage(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        if payload.len() < 3 {
            return None;
        }
        let core = payload[0];
        let mv = u16::from_be_bytes([payload[1], payload[2]]);
        self.pending_vicor = Some(PendingVicor::SetVoltage { core, mv });
        None
    }

    fn handle_emergency_stop(&mut self) -> Option<FbcPacket> {
        self.pending_vicor = Some(PendingVicor::EmergencyStop);
        // ACK immediately - this is critical
        Some(FbcPacket::new(power::EMERGENCY_STOP, self.next_seq()))
    }

    fn handle_power_sequence_on(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        if payload.len() < 12 {
            return None;
        }
        let mut voltages_mv = [0u16; 6];
        for i in 0..6 {
            voltages_mv[i] = u16::from_be_bytes([payload[i*2], payload[i*2+1]]);
        }
        self.pending_vicor = Some(PendingVicor::PowerSequenceOn { voltages_mv });
        None
    }

    fn handle_power_sequence_off(&mut self) -> Option<FbcPacket> {
        self.pending_vicor = Some(PendingVicor::PowerSequenceOff);
        None
    }

    fn handle_pmbus_enable(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        if payload.len() < 2 {
            return None;
        }
        self.pending_pmbus = Some(PendingPmbus {
            addr: payload[0],
            enable: payload[1] != 0,
        });
        None
    }

    // =========================================================================
    // EEPROM Handlers
    // =========================================================================

    fn handle_eeprom_read(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        if payload.len() < 2 {
            return None;
        }
        self.pending_eeprom = Some(PendingEeprom::Read {
            offset: payload[0],
            len: payload[1].min(64),
        });
        None
    }

    fn handle_eeprom_write(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        if payload.len() < 2 {
            return None;
        }
        let offset = payload[0];
        let len = payload[1].min(64) as usize;
        if payload.len() < 2 + len {
            return None;
        }
        let mut data = [0u8; 64];
        data[..len].copy_from_slice(&payload[2..2+len]);
        self.pending_eeprom = Some(PendingEeprom::Write {
            offset,
            len: len as u8,
            data,
        });
        None
    }

    // =========================================================================
    // Fast Pins Handlers
    // =========================================================================

    fn handle_fastpins_read(&mut self) -> Option<FbcPacket> {
        self.pending_fastpins = Some(PendingFastPins::Read);
        None
    }

    fn handle_fastpins_write(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        if payload.len() < 8 {
            return None;
        }
        let dout = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
        let oen = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
        self.pending_fastpins = Some(PendingFastPins::Write { dout, oen });
        None
    }

    // =========================================================================
    // Error Log Handlers
    // =========================================================================

    fn handle_error_log_req(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        let req = ErrorLogReqPayload::from_bytes(payload)?;
        self.pending_error_log = Some(PendingErrorLog {
            start_index: req.start_index,
            count: req.count,
        });
        None
    }

    // =========================================================================
    // Firmware Update Handlers
    // =========================================================================

    fn handle_fw_info_req(&mut self) -> Option<FbcPacket> {
        // Request firmware info - main.rs will build the response with SD card status
        self.pending_fw_info = true;
        None
    }

    /// Build firmware info response (called by main.rs)
    pub fn build_fw_info_rsp(&mut self, sd_present: bool) -> FbcPacket {
        let payload = FirmwareInfoRspPayload {
            version_major: FW_VERSION_MAJOR,
            version_minor: FW_VERSION_MINOR,
            version_patch: FW_VERSION_PATCH,
            build_date: *FW_BUILD_DATE,
            board_serial: self.serial,
            hw_revision: 1,
            bootloader_version: 1,
            sd_present: if sd_present { 1 } else { 0 },
            update_in_progress: if self.fw_update_in_progress { 1 } else { 0 },
        };
        FbcPacket::with_payload(firmware::INFO_RSP, self.next_seq(), &payload.to_bytes())
    }

    fn handle_fw_begin(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        let begin = FirmwareBeginPayload::from_bytes(payload)?;

        // Store pending begin request - main.rs will check SD and respond
        self.pending_fw_begin = Some(PendingFwBegin {
            total_size: begin.total_size,
            checksum: begin.checksum,
        });
        None
    }

    /// Start firmware update (called by main.rs after SD check)
    pub fn start_fw_update(&mut self, total_size: u32, checksum: u32) {
        self.fw_update_in_progress = true;
        self.fw_update_total_size = total_size;
        self.fw_update_expected_checksum = checksum;
        self.fw_update_received = 0;
        self.fw_update_running_checksum = 0;
    }

    /// Build firmware begin ACK (called by main.rs)
    pub fn build_fw_begin_ack(&mut self, status: u8) -> FbcPacket {
        let payload = FirmwareBeginAckPayload {
            status,
            max_chunk_size: 1024,  // Max chunk size we accept
        };
        FbcPacket::with_payload(firmware::BEGIN_ACK, self.next_seq(), &payload.to_bytes())
    }

    fn handle_fw_chunk(&mut self, payload: &[u8]) -> Option<FbcPacket> {
        if !self.fw_update_in_progress {
            return None;  // Not in update mode
        }

        let (header, chunk_data) = FirmwareChunkPayload::from_bytes(payload)?;

        if header.offset != self.fw_update_received {
            // Offset mismatch - send error ACK immediately
            let ack = FirmwareChunkAckPayload {
                offset: header.offset,
                status: 2,  // Offset mismatch
            };
            return Some(FbcPacket::with_payload(
                firmware::CHUNK_ACK,
                self.next_seq(),
                &ack.to_bytes()
            ));
        }

        // Store pending chunk for main.rs to write to SD
        let mut chunk = PendingFwChunk {
            offset: header.offset,
            size: header.size.min(1024),
            data: [0u8; 1024],
        };
        let copy_len = (header.size as usize).min(chunk_data.len()).min(1024);
        chunk.data[..copy_len].copy_from_slice(&chunk_data[..copy_len]);
        self.pending_fw_chunk = Some(chunk);

        None  // main.rs will send ACK after SD write
    }

    /// Process chunk after SD write (called by main.rs)
    pub fn process_fw_chunk(&mut self, chunk_size: u32, crc_update: u32) {
        self.fw_update_received += chunk_size;
        self.fw_update_running_checksum ^= crc_update;  // Simple XOR for now
    }

    /// Build firmware chunk ACK (called by main.rs)
    pub fn build_fw_chunk_ack(&mut self, offset: u32, status: u8) -> FbcPacket {
        let payload = FirmwareChunkAckPayload { offset, status };
        FbcPacket::with_payload(firmware::CHUNK_ACK, self.next_seq(), &payload.to_bytes())
    }

    fn handle_fw_commit(&mut self) -> Option<FbcPacket> {
        if !self.fw_update_in_progress {
            return None;
        }
        self.pending_fw_commit = true;
        None  // main.rs will verify and respond
    }

    /// Build firmware commit ACK (called by main.rs)
    pub fn build_fw_commit_ack(&mut self, status: u8) -> FbcPacket {
        let payload = FirmwareCommitAckPayload {
            status,
            received_size: self.fw_update_received,
            computed_checksum: self.fw_update_running_checksum,
        };
        FbcPacket::with_payload(firmware::COMMIT_ACK, self.next_seq(), &payload.to_bytes())
    }

    fn handle_fw_abort(&mut self) -> Option<FbcPacket> {
        self.fw_update_in_progress = false;
        self.fw_update_total_size = 0;
        self.fw_update_received = 0;
        self.pending_fw_begin = None;
        self.pending_fw_chunk = None;
        self.pending_fw_commit = false;
        // ACK the abort
        Some(FbcPacket::new(firmware::ABORT, self.next_seq()))
    }

    /// Check if firmware update is in progress
    pub fn is_fw_update_in_progress(&self) -> bool {
        self.fw_update_in_progress
    }

    /// Get update progress
    pub fn get_fw_update_progress(&self) -> (u32, u32) {
        (self.fw_update_received, self.fw_update_total_size)
    }
}
