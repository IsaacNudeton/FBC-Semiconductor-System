//! FBC Protocol - Raw Ethernet Communication
//!
//! Handles communication with FBC controller boards over raw Ethernet (EtherType 0x88B5).

use byteorder::{BigEndian, ByteOrder};
use pnet_datalink::{self, Channel, DataLinkReceiver, DataLinkSender, MacAddr, NetworkInterface};
use pnet_packet::ethernet::{EthernetPacket, MutableEthernetPacket};
use pnet_packet::Packet;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::Mutex;

// =============================================================================
// Protocol Constants
// =============================================================================

pub const ETHERTYPE_FBC: u16 = 0x88B5;
pub const FBC_MAGIC: u16 = 0xFBC0;
pub const MAX_PAYLOAD: usize = 1478;
pub const BROADCAST_MAC: [u8; 6] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

/// Setup commands (matches firmware fbc_protocol.rs)
pub mod setup {
    pub const ANNOUNCE: u8 = 0x01;         // Controller → GUI (on boot)
    pub const BIM_STATUS_REQ: u8 = 0x10;   // GUI → Controller
    pub const BIM_STATUS_RSP: u8 = 0x11;   // Controller → GUI
    pub const WRITE_BIM: u8 = 0x20;        // GUI → Controller
    pub const UPLOAD_VECTORS: u8 = 0x21;   // GUI → Controller (chunked)
    pub const CONFIGURE: u8 = 0x30;        // GUI → Controller
}

/// Runtime commands
pub mod runtime {
    pub const START: u8 = 0x40;
    pub const STOP: u8 = 0x41;
    pub const RESET: u8 = 0x42;
    pub const HEARTBEAT: u8 = 0x50;
    pub const ERROR: u8 = 0xE0;
    pub const STATUS_REQ: u8 = 0xF0;
    pub const STATUS_RSP: u8 = 0xF1;
}

/// Flight Recorder commands (matches firmware fbc_protocol.rs)
pub mod flight_recorder {
    pub const LOG_READ_REQ: u8 = 0x60;
    pub const LOG_READ_RSP: u8 = 0x61;
    pub const LOG_INFO_REQ: u8 = 0x62;
    pub const LOG_INFO_RSP: u8 = 0x63;
}

/// Fast Pin commands (moved to 0xD0 to avoid collision with flight_recorder)
pub mod fastpins {
    pub const READ_REQ: u8 = 0xD0;
    pub const READ_RSP: u8 = 0xD1;
    pub const WRITE: u8 = 0xD2;
}

/// Analog monitoring commands
pub mod analog {
    pub const READ_ALL_REQ: u8 = 0x70;
    pub const READ_ALL_RSP: u8 = 0x71;
}

/// Power control commands
pub mod power {
    pub const VICOR_STATUS_REQ: u8 = 0x80;
    pub const VICOR_STATUS_RSP: u8 = 0x81;
    pub const VICOR_ENABLE: u8 = 0x82;
    pub const VICOR_SET_VOLTAGE: u8 = 0x83;
    pub const PMBUS_STATUS_REQ: u8 = 0x84;
    pub const PMBUS_STATUS_RSP: u8 = 0x85;
    pub const PMBUS_ENABLE: u8 = 0x86;
    pub const EMERGENCY_STOP: u8 = 0x8F;
    pub const POWER_SEQUENCE_ON: u8 = 0x90;
    pub const POWER_SEQUENCE_OFF: u8 = 0x91;
}

/// EEPROM commands
pub mod eeprom {
    pub const READ_REQ: u8 = 0xA0;
    pub const READ_RSP: u8 = 0xA1;
    pub const WRITE: u8 = 0xA2;
    pub const WRITE_ACK: u8 = 0xA3;
}

/// Vector engine commands
pub mod vector {
    pub const STATUS_REQ: u8 = 0xB0;
    pub const STATUS_RSP: u8 = 0xB1;
    pub const LOAD: u8 = 0xB2;
    pub const LOAD_ACK: u8 = 0xB3;
    pub const START: u8 = 0xB4;
    pub const PAUSE: u8 = 0xB5;
    pub const RESUME: u8 = 0xB6;
    pub const STOP: u8 = 0xB7;
}

/// Error log commands (read error BRAM contents)
pub mod error_log {
    pub const ERROR_LOG_REQ: u8 = 0x4A;  // GUI → Controller (start_index, count)
    pub const ERROR_LOG_RSP: u8 = 0x4B;  // Controller → GUI (error entries)
}

// =============================================================================
// Types
// =============================================================================

#[derive(Error, Debug)]
pub enum FbcError {
    #[error("Interface not found: {0}")]
    InterfaceNotFound(String),
    #[error("Failed to open interface: {0}")]
    OpenFailed(String),
    #[error("Send failed: {0}")]
    SendFailed(String),
    #[error("Receive failed: {0}")]
    ReceiveFailed(String),
    #[error("Timeout")]
    Timeout,
    #[error("Not connected")]
    NotConnected,
    #[error("Invalid MAC address: {0}")]
    InvalidMac(String),
}

pub type Result<T> = std::result::Result<T, FbcError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardInfo {
    pub mac: String,
    pub serial: u32,
    pub hw_revision: u8,
    pub fw_version: String,
    pub has_bim: bool,
    pub bim_type: u8,
    pub state: ControllerState,
    pub slot: Option<SlotPosition>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ControllerState {
    Idle,
    Running,
    Done,
    Error,
    Unknown,
}

impl From<u8> for ControllerState {
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Idle,
            1 => Self::Running,
            2 => Self::Done,
            3 => Self::Error,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FastPinState {
    pub dout: u32,
    pub oen: u32,
    pub din: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardStatus {
    pub state: ControllerState,
    pub cycles: u32,
    pub errors: u32,
    pub temp_c: f32,
    /// Rail voltages in mV: [Core1..Core6, VDD_IO, VDD_3V3]
    pub rail_voltage_mv: [u16; 8],
    /// Rail currents in mA: [Core1..Core6, 0, 0]
    pub rail_current_ma: [u16; 8],
    pub fpga_vccint_mv: u16,
    pub fpga_vccaux_mv: u16,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SlotPosition {
    pub shelf: u8,     // 1-11
    pub tray: TrayPos, // Front or Back
    pub slot: u8,      // Position on tray (1-4)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrayPos {
    Front,
    Back,
}

// =============================================================================
// Analog Channel Data
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalogChannels {
    /// XADC channels (0-15): on-chip measurements
    pub xadc: [AnalogReading; 16],
    /// MAX11131 channels (0-15): external ADC
    pub external: [AnalogReading; 16],
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalogReading {
    pub raw: u16,
    pub voltage_mv: f32,
    pub name: String,
}

impl Default for AnalogChannels {
    fn default() -> Self {
        Self {
            xadc: std::array::from_fn(|_| AnalogReading::default()),
            external: std::array::from_fn(|_| AnalogReading::default()),
        }
    }
}

// =============================================================================
// Power Control Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VicorStatus {
    pub cores: [VicorCore; 6],
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct VicorCore {
    pub id: u8,
    pub enabled: bool,
    pub voltage_mv: u16,
    pub current_ma: u16,
    pub temp_c: f32,
    pub status: VicorCoreStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VicorCoreStatus {
    #[default]
    Off,
    On,
    Fault,
    OverTemp,
    OverCurrent,
}

impl From<u8> for VicorCoreStatus {
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Off,
            1 => Self::On,
            2 => Self::Fault,
            3 => Self::OverTemp,
            4 => Self::OverCurrent,
            _ => Self::Fault,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmBusStatus {
    pub rails: Vec<PmBusRail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmBusRail {
    pub address: u8,
    pub name: String,
    pub enabled: bool,
    pub voltage_mv: u16,
    pub current_ma: u16,
    pub power_mw: u32,
    pub temp_c: f32,
    pub status_word: u16,
}

// =============================================================================
// EEPROM Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EepromData {
    /// Raw 256-byte EEPROM contents
    pub raw: Vec<u8>,
    /// Parsed header
    pub header: EepromHeader,
    /// Rail configurations
    pub rails: Vec<RailConfig>,
    /// DUT metadata
    pub dut: DutMetadata,
    /// Calibration data
    pub calibration: CalibrationData,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct EepromHeader {
    pub magic: u16,
    pub version: u8,
    pub board_serial: u32,
    pub hw_revision: u8,
    pub mfg_date: u32,
    pub config_crc: u16,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct RailConfig {
    pub rail_id: u8,
    pub name: [u8; 8],
    pub nominal_mv: u16,
    pub max_mv: u16,
    pub max_ma: u16,
    pub enabled_by_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DutMetadata {
    pub part_number: String,
    pub lot_id: String,
    pub wafer_id: u8,
    pub die_x: u8,
    pub die_y: u8,
    pub test_count: u32,
    pub last_test_time: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct CalibrationData {
    pub adc_offset: [i16; 16],
    pub adc_gain: [u16; 16],
    pub dac_offset: [i16; 10],
    pub dac_gain: [u16; 10],
    pub temp_offset: i16,
}

impl Default for EepromData {
    fn default() -> Self {
        Self {
            raw: vec![0xFF; 256],
            header: EepromHeader::default(),
            rails: vec![RailConfig::default(); 6],
            dut: DutMetadata::default(),
            calibration: CalibrationData::default(),
        }
    }
}

// =============================================================================
// Vector Engine Types
// =============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VectorState {
    #[default]
    Idle,
    Loading,
    Ready,
    Running,
    Paused,
    Done,
    Error,
}

impl From<u8> for VectorState {
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Idle,
            1 => Self::Loading,
            2 => Self::Ready,
            3 => Self::Running,
            4 => Self::Paused,
            5 => Self::Done,
            6 => Self::Error,
            _ => Self::Error,
        }
    }
}

// =============================================================================
// FBC Header
// =============================================================================

#[derive(Debug, Clone, Copy)]
pub struct FbcHeader {
    pub magic: u16,
    pub seq: u16,
    pub cmd: u8,
    pub flags: u8,
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

    pub fn to_bytes(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        BigEndian::write_u16(&mut buf[0..2], self.magic);
        BigEndian::write_u16(&mut buf[2..4], self.seq);
        buf[4] = self.cmd;
        buf[5] = self.flags;
        BigEndian::write_u16(&mut buf[6..8], self.length);
        buf
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        let magic = BigEndian::read_u16(&data[0..2]);
        if magic != FBC_MAGIC {
            return None;
        }

        Some(Self {
            magic,
            seq: BigEndian::read_u16(&data[2..4]),
            cmd: data[4],
            flags: data[5],
            length: BigEndian::read_u16(&data[6..8]),
        })
    }
}

// =============================================================================
// FBC Socket
// =============================================================================

pub struct FbcSocket {
    interface: NetworkInterface,
    our_mac: [u8; 6],
    tx: Arc<Mutex<Box<dyn DataLinkSender>>>,
    rx: Arc<Mutex<Box<dyn DataLinkReceiver>>>,
    seq: Arc<Mutex<u16>>,
}

impl FbcSocket {
    pub fn new(interface_name: &str) -> Result<Self> {
        let interfaces = pnet_datalink::interfaces();

        // Handle combined format "name (description)" from list_interfaces()
        let search_name = if interface_name.contains(" (") {
            interface_name.split(" (").next().unwrap_or(interface_name)
        } else {
            interface_name
        };

        let interface = interfaces
            .into_iter()
            .find(|iface| {
                iface.name == search_name || iface.name == interface_name || iface.description == interface_name
            })
            .ok_or_else(|| FbcError::InterfaceNotFound(interface_name.to_string()))?;

        let our_mac = interface
            .mac
            .ok_or_else(|| FbcError::OpenFailed("No MAC address".into()))?
            .octets();

        let (tx, rx) = match pnet_datalink::channel(&interface, Default::default()) {
            Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
            Ok(_) => return Err(FbcError::OpenFailed("Unexpected channel type".into())),
            Err(e) => return Err(FbcError::OpenFailed(e.to_string())),
        };

        Ok(Self {
            interface,
            our_mac,
            tx: Arc::new(Mutex::new(tx)),
            rx: Arc::new(Mutex::new(rx)),
            seq: Arc::new(Mutex::new(0)),
        })
    }

    pub fn our_mac(&self) -> [u8; 6] {
        self.our_mac
    }

    async fn next_seq(&self) -> u16 {
        let mut seq = self.seq.lock().await;
        let val = *seq;
        *seq = seq.wrapping_add(1);
        val
    }

    pub async fn send(&self, dst_mac: [u8; 6], cmd: u8, payload: &[u8]) -> Result<()> {
        let seq = self.next_seq().await;
        let header = FbcHeader::new(cmd, seq, payload.len() as u16);

        // Build frame
        let frame_len = 14 + 8 + payload.len();
        let mut buffer = vec![0u8; frame_len.max(60)];

        {
            let mut eth = MutableEthernetPacket::new(&mut buffer)
                .ok_or_else(|| FbcError::SendFailed("Buffer too small".into()))?;

            eth.set_destination(MacAddr::new(
                dst_mac[0], dst_mac[1], dst_mac[2], dst_mac[3], dst_mac[4], dst_mac[5],
            ));
            eth.set_source(MacAddr::new(
                self.our_mac[0],
                self.our_mac[1],
                self.our_mac[2],
                self.our_mac[3],
                self.our_mac[4],
                self.our_mac[5],
            ));
            eth.set_ethertype(pnet_packet::ethernet::EtherType(ETHERTYPE_FBC));
        }

        // FBC header + payload
        buffer[14..22].copy_from_slice(&header.to_bytes());
        if !payload.is_empty() {
            buffer[22..22 + payload.len()].copy_from_slice(payload);
        }

        let mut tx = self.tx.lock().await;
        tx.send_to(&buffer, None)
            .ok_or_else(|| FbcError::SendFailed("Send returned None".into()))?
            .map_err(|e| FbcError::SendFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn recv(&self) -> Result<Option<([u8; 6], u8, Vec<u8>)>> {
        let mut rx = self.rx.lock().await;

        match rx.next() {
            Ok(packet) => {
                if let Some(eth) = EthernetPacket::new(packet) {
                    if eth.get_ethertype().0 != ETHERTYPE_FBC {
                        return Ok(None);
                    }

                    let payload = eth.payload();
                    if let Some(header) = FbcHeader::from_bytes(payload) {
                        let src_mac = eth.get_source().octets();
                        let data = payload[8..8 + header.length as usize].to_vec();
                        return Ok(Some((src_mac, header.cmd, data)));
                    }
                }
                Ok(None)
            }
            Err(e) => Err(FbcError::ReceiveFailed(e.to_string())),
        }
    }

    pub async fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<([u8; 6], u8, Vec<u8>)>> {
        let start = Instant::now();

        while start.elapsed() < timeout {
            if let Some(result) = self.recv().await? {
                return Ok(Some(result));
            }
            tokio::time::sleep(Duration::from_micros(100)).await;
        }

        Ok(None)
    }
}

// =============================================================================
// Error Log Types
// =============================================================================

/// Error log entry from BRAM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorLogEntry {
    /// 128-bit pattern value (4 × 32-bit words)
    pub pattern: [u32; 4],
    /// Vector number when error occurred
    pub vector: u32,
    /// Cycle count low
    pub cycle_lo: u32,
    /// Cycle count high
    pub cycle_hi: u32,
}

/// Error log response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorLogResponse {
    /// Total errors recorded
    pub total_errors: u32,
    /// Number of entries in this response (max 8)
    pub num_entries: u32,
    /// Error entries
    pub entries: Vec<ErrorLogEntry>,
}

// =============================================================================
// Helper Functions
// =============================================================================

pub fn list_interfaces() -> Vec<String> {
    pnet_datalink::interfaces()
        .into_iter()
        .filter(|iface| iface.mac.is_some() && !iface.is_loopback())
        .map(|iface| {
            if iface.description.is_empty() {
                iface.name
            } else {
                format!("{} ({})", iface.name, iface.description)
            }
        })
        .collect()
}

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

pub fn format_mac(mac: &[u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}
