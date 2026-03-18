//! Types for the FBC Semiconductor System
//!
//! Organized by system profile:
//! - **Common**: Types shared across all tester systems (same Zynq hardware)
//! - **FBC-specific**: Raw Ethernet protocol responses (bare-metal firmware)
//! - **Sonoma-specific**: SSH command responses (Linux firmware)
//!
//! Each system has its own transport, profile, and calibration — but the
//! underlying hardware (VICOR, ADC, PMBus, XADC) is the same.

use serde::{Deserialize, Serialize};

// =============================================================================
// System Type (mirrors C engine: lrm_schema.h SystemType enum)
// =============================================================================

/// Tester system type — determines transport, profile, and pattern format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemType {
    /// Aehr HX — 4-axis, INSPIRE transport
    Hx = 0,
    /// Sonoma — 1-axis, SSH + ELF binaries (Linux)
    Sonoma = 1,
    /// XP-160 — 8-axis, INSPIRE transport
    Xp160 = 2,
    /// MCC — 1-axis, Modbus TCP
    Mcc = 3,
    /// Shasta — 8-axis, INSPIRE transport (newer XP-160)
    Shasta = 4,
    /// FBC — 1-axis, raw Ethernet 0x88B5 (bare-metal)
    Fbc = 5,
}

impl SystemType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Hx),
            1 => Some(Self::Sonoma),
            2 => Some(Self::Xp160),
            3 => Some(Self::Mcc),
            4 => Some(Self::Shasta),
            5 => Some(Self::Fbc),
            _ => None,
        }
    }

    /// Profile name used by the C engine pattern converter
    pub fn profile_name(&self) -> &'static str {
        match self {
            Self::Hx => "hx",
            Self::Sonoma => "sonoma",
            Self::Xp160 | Self::Shasta => "xp160",
            Self::Mcc => "mcc",
            Self::Fbc => "fbc",
        }
    }

    /// Total pin channels for this system
    pub fn total_channels(&self) -> u16 {
        self.profile().total_channels
    }

    /// Get the full system profile for this type
    pub fn profile(&self) -> SystemProfile {
        match self {
            Self::Fbc => SystemProfile {
                system_type: *self,
                total_channels: 160,
                bim_channels: 128,
                fast_channels: 32,
                vicor_cores: 6,
                pattern_format: PatternFormat::Fbc,
                transport: Transport::RawEthernet,
                voltage_limits: VoltageLimits {
                    vicor_min_mv: 500,
                    vicor_max_mv: 5000,
                    pmbus_min_mv: 800,
                    pmbus_max_mv: 3600,
                },
            },
            Self::Sonoma => SystemProfile {
                system_type: *self,
                total_channels: 128,
                bim_channels: 128,
                fast_channels: 0,
                vicor_cores: 6,
                pattern_format: PatternFormat::Hex,
                transport: Transport::Ssh,
                voltage_limits: VoltageLimits {
                    vicor_min_mv: 500,
                    vicor_max_mv: 5000,
                    pmbus_min_mv: 800,
                    pmbus_max_mv: 3600,
                },
            },
            Self::Hx => SystemProfile {
                system_type: *self,
                total_channels: 160,
                bim_channels: 160,
                fast_channels: 0,
                vicor_cores: 6,
                pattern_format: PatternFormat::Hex,
                transport: Transport::Inspire,
                voltage_limits: VoltageLimits {
                    vicor_min_mv: 500,
                    vicor_max_mv: 5000,
                    pmbus_min_mv: 800,
                    pmbus_max_mv: 3600,
                },
            },
            Self::Xp160 | Self::Shasta => SystemProfile {
                system_type: *self,
                total_channels: 160,
                bim_channels: 160,
                fast_channels: 0,
                vicor_cores: 6,
                pattern_format: PatternFormat::Hex,
                transport: Transport::Inspire,
                voltage_limits: VoltageLimits {
                    vicor_min_mv: 500,
                    vicor_max_mv: 5000,
                    pmbus_min_mv: 800,
                    pmbus_max_mv: 3600,
                },
            },
            Self::Mcc => SystemProfile {
                system_type: *self,
                total_channels: 128,
                bim_channels: 128,
                fast_channels: 0,
                vicor_cores: 6,
                pattern_format: PatternFormat::Hex,
                transport: Transport::ModbusTcp,
                voltage_limits: VoltageLimits {
                    vicor_min_mv: 500,
                    vicor_max_mv: 5000,
                    pmbus_min_mv: 800,
                    pmbus_max_mv: 3600,
                },
            },
        }
    }
}

impl std::fmt::Display for SystemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hx => write!(f, "HX"),
            Self::Sonoma => write!(f, "Sonoma"),
            Self::Xp160 => write!(f, "XP-160"),
            Self::Mcc => write!(f, "MCC"),
            Self::Shasta => write!(f, "Shasta"),
            Self::Fbc => write!(f, "FBC"),
        }
    }
}

// =============================================================================
// System Profile — per-system configuration
// =============================================================================

/// Transport protocol used to communicate with the board
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// FBC: Raw Ethernet 0x88B5 (bare-metal firmware)
    RawEthernet,
    /// Sonoma: SSH + ELF binaries (Linux)
    Ssh,
    /// HX/XP-160/Shasta: INSPIRE transport
    Inspire,
    /// MCC: Modbus TCP
    ModbusTcp,
}

/// Pattern file format for vector data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternFormat {
    /// `.fbc` — compressed opcodes (VECTOR_ZERO, VECTOR_RUN, VECTOR_SPARSE)
    Fbc,
    /// `.hex` — legacy 40-byte/vector uncompressed
    Hex,
}

/// Voltage limits for safety validation
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VoltageLimits {
    pub vicor_min_mv: u16,
    pub vicor_max_mv: u16,
    pub pmbus_min_mv: u16,
    pub pmbus_max_mv: u16,
}

impl VoltageLimits {
    /// Validate a VICOR voltage setting
    pub fn validate_vicor(&self, mv: u16) -> std::result::Result<(), String> {
        if mv == 0 { return Ok(()); } // 0 = disable
        if mv < self.vicor_min_mv || mv > self.vicor_max_mv {
            Err(format!("VICOR voltage {}mV out of range ({}-{}mV)",
                mv, self.vicor_min_mv, self.vicor_max_mv))
        } else {
            Ok(())
        }
    }

    /// Validate a PMBus voltage setting
    pub fn validate_pmbus(&self, mv: u16) -> std::result::Result<(), String> {
        if mv == 0 { return Ok(()); }
        if mv < self.pmbus_min_mv || mv > self.pmbus_max_mv {
            Err(format!("PMBus voltage {}mV out of range ({}-{}mV)",
                mv, self.pmbus_min_mv, self.pmbus_max_mv))
        } else {
            Ok(())
        }
    }
}

/// Complete system profile — everything that differs between system types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SystemProfile {
    pub system_type: SystemType,
    /// Total pin channels (BIM + fast)
    pub total_channels: u16,
    /// BIM channels (through interposer, 2-cycle latency)
    pub bim_channels: u16,
    /// Fast channels (direct FPGA I/O, 1-cycle latency) — FBC only
    pub fast_channels: u16,
    /// Number of VICOR core power supplies
    pub vicor_cores: u8,
    /// Pattern file format (.fbc or .hex)
    pub pattern_format: PatternFormat,
    /// Communication transport
    pub transport: Transport,
    /// Voltage safety limits
    pub voltage_limits: VoltageLimits,
}

impl SystemProfile {
    /// Pin range for BIM channels
    pub fn bim_range(&self) -> std::ops::Range<u16> {
        0..self.bim_channels
    }

    /// Pin range for fast channels (empty if no fast channels)
    pub fn fast_range(&self) -> std::ops::Range<u16> {
        self.bim_channels..(self.bim_channels + self.fast_channels)
    }
}

// =============================================================================
// Common Types (shared hardware across all Zynq-based systems)
// =============================================================================

/// Controller execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerState {
    Idle = 0,
    Running = 1,
    Done = 2,
    Error = 3,
}

impl ControllerState {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Idle,
            1 => Self::Running,
            2 => Self::Done,
            3 => Self::Error,
            _ => Self::Error,
        }
    }
}

impl std::fmt::Display for ControllerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Done => write!(f, "Done"),
            Self::Error => write!(f, "Error"),
        }
    }
}

// =============================================================================
// Board Discovery
// =============================================================================

/// Board info from discovery (parsed from ANNOUNCE payload)
#[derive(Debug, Clone, Serialize)]
pub struct BoardInfo {
    pub system_type: SystemType,
    pub mac: [u8; 6],
    pub serial: u32,
    pub fw_version: u16,
    pub hw_revision: u8,
    pub bim_type: u8,
    pub has_bim: bool,
    pub bim_programmed: bool,
}

// =============================================================================
// Status
// =============================================================================

/// Full board status from STATUS_RSP (47 bytes)
#[derive(Debug, Clone, Serialize)]
pub struct StatusResponse {
    pub state: ControllerState,
    pub cycles: u32,
    pub errors: u32,
    pub temp_c: f32,
    pub rail_voltage: [u16; 8],
    pub rail_current: [u16; 8],
    pub fpga_vccint: u16,
    pub fpga_vccaux: u16,
}

// =============================================================================
// Fast Pins
// =============================================================================

/// Fast pin state (gpio[128:159], 1-cycle latency)
#[derive(Debug, Clone, Serialize)]
pub struct FastPinState {
    /// Input readback
    pub din: u32,
    /// Output drive
    pub dout: u32,
    /// Output enable
    pub oen: u32,
}

// =============================================================================
// Analog
// =============================================================================

/// Single analog reading
#[derive(Debug, Clone, Serialize)]
pub struct AnalogReading {
    pub channel: u8,
    pub raw: u16,
    pub voltage_mv: f32,
}

/// All 32 analog channels
#[derive(Debug, Clone, Serialize)]
pub struct AnalogChannels {
    /// XADC channels 0-15 (internal Zynq ADC)
    pub xadc: Vec<AnalogReading>,
    /// External ADC channels 16-31 (MAX11131)
    pub external: Vec<AnalogReading>,
}

// =============================================================================
// VICOR Power
// =============================================================================

/// Single VICOR core status (5 bytes on wire)
#[derive(Debug, Clone, Copy, Serialize)]
pub struct VicorCore {
    pub id: u8,
    pub enabled: bool,
    pub voltage_mv: u16,
    pub current_ma: u16,
}

/// All 6 VICOR core power supplies
#[derive(Debug, Clone, Serialize)]
pub struct VicorStatus {
    pub cores: [VicorCore; 6],
}

// =============================================================================
// PMBus Power
// =============================================================================

/// PMBus rail status
#[derive(Debug, Clone, Serialize)]
pub struct PmBusRail {
    pub address: u8,
    pub enabled: bool,
    pub voltage_mv: u16,
    pub current_ma: u16,
}

/// PMBus status response
#[derive(Debug, Clone, Serialize)]
pub struct PmBusStatus {
    pub rails: Vec<PmBusRail>,
}

// =============================================================================
// Vector Engine
// =============================================================================

/// Vector engine execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorState {
    Idle = 0,
    Loading = 1,
    Running = 2,
    Paused = 3,
    Done = 4,
    Error = 5,
}

impl VectorState {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Idle,
            1 => Self::Loading,
            2 => Self::Running,
            3 => Self::Paused,
            4 => Self::Done,
            5 => Self::Error,
            _ => Self::Error,
        }
    }
}

impl std::fmt::Display for VectorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Loading => write!(f, "Loading"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
            Self::Error => write!(f, "Error"),
        }
    }
}

/// Vector engine status (from axi_vector_status or SSH query)
#[derive(Debug, Clone, Serialize)]
pub struct VectorEngineStatus {
    pub state: VectorState,
    pub current_address: u32,
    pub total_vectors: u32,
    pub loop_count: u32,
    pub target_loops: u32,
    pub error_count: u32,
    pub first_fail_addr: u32,
    pub run_time_ms: u64,
}

// =============================================================================
// Error Log
// =============================================================================

/// Single error log entry (28 bytes from error BRAM)
#[derive(Debug, Clone, Serialize)]
pub struct ErrorLogEntry {
    /// 128-bit error pattern (4 x u32)
    pub pattern: [u32; 4],
    /// Vector number when error occurred
    pub vector: u32,
    /// Cycle count when error occurred
    pub cycle: u64,
}

/// Error log response
#[derive(Debug, Clone, Serialize)]
pub struct ErrorLogResponse {
    pub total_errors: u32,
    pub entries: Vec<ErrorLogEntry>,
}

// =============================================================================
// EEPROM
// =============================================================================

/// Raw EEPROM data
#[derive(Debug, Clone, Serialize)]
pub struct EepromData {
    pub offset: u8,
    pub data: Vec<u8>,
}

// =============================================================================
// Flight Recorder
// =============================================================================

/// Flight recorder log info
#[derive(Debug, Clone, Serialize)]
pub struct LogInfo {
    pub sd_present: bool,
    pub boot_sector: u32,
    pub log_start: u32,
    pub log_end: u32,
    pub current_index: u32,
    pub total_entries: u32,
}

/// Flight recorder sector data
#[derive(Debug, Clone, Serialize)]
pub struct LogSector {
    pub sector: u32,
    pub status: u8,
    pub data: Vec<u8>,
}

// =============================================================================
// Firmware
// =============================================================================

/// Firmware info response
#[derive(Debug, Clone, Serialize)]
pub struct FirmwareInfo {
    pub version_major: u8,
    pub version_minor: u8,
    pub version_patch: u8,
    pub build_date: String,
    pub serial: u32,
    pub hw_revision: u8,
    pub bootloader_version: u8,
    pub update_in_progress: bool,
    pub sd_present: bool,
}

/// Firmware update begin acknowledgment
#[derive(Debug, Clone, Serialize)]
pub struct FwBeginAck {
    /// 0=ready, 1=no SD, 2=SD error, 3=in progress
    pub status: u8,
    pub max_chunk_size: u16,
}

/// Firmware chunk acknowledgment
#[derive(Debug, Clone, Serialize)]
pub struct FwChunkAck {
    pub offset: u32,
    /// 0=OK, 1=write error, 2=offset mismatch
    pub status: u8,
}

/// Firmware commit acknowledgment
#[derive(Debug, Clone, Serialize)]
pub struct FwCommitAck {
    /// 0=success, 1=checksum mismatch, 2=incomplete
    pub status: u8,
    pub received_size: u32,
    pub computed_checksum: u32,
}

// =============================================================================
// Sonoma-specific
// =============================================================================

/// Sonoma board status (composite of multiple SSH queries)
#[derive(Debug, Clone, Serialize)]
pub struct SonomaStatus {
    pub system_type: SystemType,
    pub alive: bool,
    pub ip: String,
    pub fw_version: String,
    pub xadc: Vec<AnalogReading>,
    pub adc32: Vec<AnalogReading>,
}

/// Result of running vectors on Sonoma
#[derive(Debug, Clone, Serialize)]
pub struct RunResult {
    pub passed: bool,
    pub vectors_executed: u32,
    pub errors: u32,
    pub duration_s: f32,
}

// =============================================================================
// Helpers
// =============================================================================

/// Format MAC address as string
pub fn format_mac(mac: &[u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

/// Parse MAC address from string
pub fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 6 {
        return None;
    }
    let mut mac = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        mac[i] = u8::from_str_radix(part, 16).ok()?;
    }
    Some(mac)
}

/// XADC raw to voltage (mV) conversion
/// Channel 0 = temperature: raw/65536 * 503.975 - 273.15 (°C, not mV)
/// Channels 1,2,6-9 = voltage: raw/65536 * 3000 (mV)
pub fn xadc_to_voltage(channel: u8, raw: u16) -> f32 {
    let raw_f = raw as f32 / 65536.0;
    match channel {
        0 => raw_f * 503.975 - 273.15, // temperature in °C
        _ => raw_f * 3000.0,           // voltage in mV
    }
}

/// External MAX11131 ADC raw to voltage (mV)
/// 12-bit, 2.5V reference
pub fn ext_adc_to_voltage(raw: u16) -> f32 {
    (raw as f32 / 4096.0) * 2500.0
}
