//! FBC Protocol - Host Side (Raw Ethernet)
//!
//! This mirrors the firmware protocol implementation but for the host (Rust std).
//! Same packet format, same commands, different networking primitives.

use byteorder::{BigEndian, ByteOrder};
use std::fmt;
use std::time::Duration;

use pnet_datalink::{self, Channel, DataLinkReceiver, DataLinkSender, NetworkInterface, MacAddr};
use pnet_packet::ethernet::{EthernetPacket, MutableEthernetPacket};
use pnet_packet::Packet;
use thiserror::Error;

use crate::types::ControllerState;

/// FBC EtherType (custom protocol)
pub const ETHERTYPE_FBC: u16 = 0x88B5;

/// FBC Protocol Magic
pub const FBC_MAGIC: u16 = 0xFBC0;

/// Maximum payload size
pub const MAX_PAYLOAD: usize = 1478;

// =============================================================================
// Commands (must match firmware exactly)
// =============================================================================

/// Setup Phase Commands
pub mod setup {
    pub const ANNOUNCE: u8 = 0x01;
    pub const BIM_STATUS_REQ: u8 = 0x10;
    pub const BIM_STATUS_RSP: u8 = 0x11;
    pub const WRITE_BIM: u8 = 0x20;
    pub const UPLOAD_VECTORS: u8 = 0x21;
    pub const CONFIGURE: u8 = 0x30;
}

/// Runtime Commands
pub mod runtime {
    pub const START: u8 = 0x40;
    pub const STOP: u8 = 0x41;
    pub const RESET: u8 = 0x42;
    pub const HEARTBEAT: u8 = 0x50;
    pub const ERROR: u8 = 0xE0;
    pub const STATUS_REQ: u8 = 0xF0;
    pub const STATUS_RSP: u8 = 0xF1;
    pub const MIN_MAX_REQ: u8 = 0xF2;
    pub const MIN_MAX_RSP: u8 = 0xF3;
}

/// Error Log Commands
pub mod error_log {
    pub const ERROR_LOG_REQ: u8 = 0x4A;
    pub const ERROR_LOG_RSP: u8 = 0x4B;
}

/// Flight Recorder Commands
pub mod flight_recorder {
    pub const LOG_READ_REQ: u8 = 0x60;
    pub const LOG_READ_RSP: u8 = 0x61;
    pub const LOG_INFO_REQ: u8 = 0x62;
    pub const LOG_INFO_RSP: u8 = 0x63;
    pub const SD_FORMAT: u8 = 0x64;
    pub const SD_FORMAT_ACK: u8 = 0x65;
    pub const SD_REPAIR: u8 = 0x66;
    pub const SD_REPAIR_ACK: u8 = 0x67;
}

/// Analog Monitoring Commands
pub mod analog {
    pub const READ_ALL_REQ: u8 = 0x70;
    pub const READ_ALL_RSP: u8 = 0x71;
}

/// Power Control Commands (VICOR + PMBus)
pub mod power {
    pub const VICOR_STATUS_REQ: u8 = 0x80;
    pub const VICOR_STATUS_RSP: u8 = 0x81;
    pub const VICOR_ENABLE: u8 = 0x82;
    pub const VICOR_SET_VOLTAGE: u8 = 0x83;
    pub const PMBUS_STATUS_REQ: u8 = 0x84;
    pub const PMBUS_STATUS_RSP: u8 = 0x85;
    pub const PMBUS_ENABLE: u8 = 0x86;
    pub const PMBUS_SET_VOLTAGE: u8 = 0x87;
    pub const EMERGENCY_STOP: u8 = 0x8F;
    pub const POWER_SEQUENCE_ON: u8 = 0x90;
    pub const POWER_SEQUENCE_OFF: u8 = 0x91;
    pub const IO_BANK_SET: u8 = 0x35;
    pub const IO_BANK_SET_ACK: u8 = 0x36;
}

/// EEPROM Commands
pub mod eeprom {
    pub const READ_REQ: u8 = 0xA0;
    pub const READ_RSP: u8 = 0xA1;
    pub const WRITE: u8 = 0xA2;
    pub const WRITE_ACK: u8 = 0xA3;
}

/// Vector Engine Commands (advanced control)
pub mod vector_engine {
    pub const STATUS_REQ: u8 = 0xB0;
    pub const STATUS_RSP: u8 = 0xB1;
    pub const LOAD: u8 = 0xB2;
    pub const LOAD_ACK: u8 = 0xB3;
    pub const START: u8 = 0xB4;
    pub const PAUSE: u8 = 0xB5;
    pub const RESUME: u8 = 0xB6;
    pub const STOP: u8 = 0xB7;
}

/// DDR Slot Commands
pub mod slot {
    pub const UPLOAD_TO_SLOT: u8 = 0x22;
    pub const SLOT_STATUS_REQ: u8 = 0x23;
    pub const SLOT_STATUS_RSP: u8 = 0x24;
    pub const INVALIDATE: u8 = 0x25;
}

/// Test Plan Commands
pub mod testplan {
    pub const SET_PLAN: u8 = 0x26;
    pub const SET_PLAN_ACK: u8 = 0x27;
    pub const RUN_PLAN: u8 = 0x28;
    pub const RUN_PLAN_ACK: u8 = 0x29;
    pub const PLAN_STATUS_REQ: u8 = 0x2A;
    pub const PLAN_STATUS_RSP: u8 = 0x2B;
    pub const STEP_RESULT: u8 = 0x2C;
}

/// Fast Pins Commands (gpio[128:159])
pub mod fastpins {
    pub const READ_REQ: u8 = 0xD0;
    pub const READ_RSP: u8 = 0xD1;
    pub const WRITE: u8 = 0xD2;
}

/// Firmware Update Commands
pub mod firmware {
    pub const INFO_REQ: u8 = 0xE1;
    pub const INFO_RSP: u8 = 0xE2;
    pub const BEGIN: u8 = 0xE3;
    pub const BEGIN_ACK: u8 = 0xE4;
    pub const CHUNK: u8 = 0xE5;
    pub const CHUNK_ACK: u8 = 0xE6;
    pub const COMMIT: u8 = 0xE7;
    pub const COMMIT_ACK: u8 = 0xE8;
    pub const ABORT: u8 = 0xE9;
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
// Payloads
// =============================================================================

#[derive(Debug, Clone)]
pub struct AnnouncePayload {
    pub mac: [u8; 6],
    pub bim_type: u8,
    pub serial: u32,
    pub hw_revision: u8,
    pub fw_version: u16,
    pub has_bim: bool,
    pub bim_programmed: bool,
}

impl AnnouncePayload {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 16 {
            return None;
        }

        let mut mac = [0u8; 6];
        mac.copy_from_slice(&data[0..6]);

        Some(Self {
            mac,
            bim_type: data[6],
            serial: BigEndian::read_u32(&data[7..11]),
            hw_revision: data[11],
            fw_version: BigEndian::read_u16(&data[12..14]),
            has_bim: data[14] != 0,
            bim_programmed: data[15] != 0,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; 16];
        buf[0..6].copy_from_slice(&self.mac);
        buf[6] = self.bim_type;
        BigEndian::write_u32(&mut buf[7..11], self.serial);
        buf[11] = self.hw_revision;
        BigEndian::write_u16(&mut buf[12..14], self.fw_version);
        buf[14] = self.has_bim as u8;
        buf[15] = self.bim_programmed as u8;
        buf
    }
}

impl fmt::Display for AnnouncePayload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}, BIM: {}, S/N: {}, FW: {}.{}",
            self.mac[0], self.mac[1], self.mac[2],
            self.mac[3], self.mac[4], self.mac[5],
            if self.bim_programmed { "programmed" } else { "blank" },
            self.serial,
            self.fw_version >> 8,
            self.fw_version & 0xFF
        )
    }
}

#[derive(Debug, Clone)]
pub struct HeartbeatPayload {
    pub cycles: u32,
    pub errors: u32,
    pub temp_c: f32,
    pub state: ControllerState,
}

impl HeartbeatPayload {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 11 {
            return None;
        }

        let temp_raw = BigEndian::read_i16(&data[8..10]);
        Some(Self {
            cycles: BigEndian::read_u32(&data[0..4]),
            errors: BigEndian::read_u32(&data[4..8]),
            temp_c: (temp_raw as f32) / 10.0,
            state: ControllerState::from_u8(data[10]),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; 11];
        BigEndian::write_u32(&mut buf[0..4], self.cycles);
        BigEndian::write_u32(&mut buf[4..8], self.errors);
        BigEndian::write_i16(&mut buf[8..10], (self.temp_c * 10.0) as i16);
        buf[10] = self.state as u8;
        buf
    }
}

#[derive(Debug, Clone)]
pub struct StatusPayload {
    pub cycles: u32,
    pub errors: u32,
    pub temp_c: f32,
    pub state: ControllerState,
    pub rail_voltage: [u16; 8],  // mV
    pub rail_current: [u16; 8],  // mA
    pub fpga_vccint: u16,
    pub fpga_vccaux: u16,
}

impl StatusPayload {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 47 {
            return None;
        }

        let temp_raw = BigEndian::read_i16(&data[8..10]);
        let mut rail_voltage = [0u16; 8];
        let mut rail_current = [0u16; 8];

        for i in 0..8 {
            rail_voltage[i] = BigEndian::read_u16(&data[11 + i * 2..13 + i * 2]);
        }
        for i in 0..8 {
            rail_current[i] = BigEndian::read_u16(&data[27 + i * 2..29 + i * 2]);
        }

        Some(Self {
            cycles: BigEndian::read_u32(&data[0..4]),
            errors: BigEndian::read_u32(&data[4..8]),
            temp_c: (temp_raw as f32) / 10.0,
            state: ControllerState::from_u8(data[10]),
            rail_voltage,
            rail_current,
            fpga_vccint: BigEndian::read_u16(&data[43..45]),
            fpga_vccaux: BigEndian::read_u16(&data[45..47]),
        })
    }
}

impl fmt::Display for StatusPayload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Cycles: {}", self.cycles)?;
        writeln!(f, "Errors: {}", self.errors)?;
        writeln!(f, "Temp: {:.1}°C", self.temp_c)?;
        writeln!(f, "State: {:?}", self.state)?;
        writeln!(f, "FPGA Vccint: {} mV", self.fpga_vccint)?;
        writeln!(f, "FPGA Vccaux: {} mV", self.fpga_vccaux)?;
        for i in 0..8 {
            writeln!(
                f,
                "Rail {}: {} mV, {} mA",
                i, self.rail_voltage[i], self.rail_current[i]
            )?;
        }
        Ok(())
    }
}

// =============================================================================
// FBC Packet
// =============================================================================

#[derive(Debug, Clone)]
pub struct FbcPacket {
    pub header: FbcHeader,
    pub payload: Vec<u8>,
}

impl FbcPacket {
    pub fn new(cmd: u8, seq: u16) -> Self {
        Self {
            header: FbcHeader::new(cmd, seq, 0),
            payload: Vec::new(),
        }
    }

    pub fn with_payload(cmd: u8, seq: u16, payload: Vec<u8>) -> Self {
        let len = payload.len().min(MAX_PAYLOAD) as u16;
        Self {
            header: FbcHeader::new(cmd, seq, len),
            payload,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + self.payload.len());
        buf.extend_from_slice(&self.header.to_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        let header = FbcHeader::from_bytes(data)?;
        let payload_len = header.length as usize;

        if data.len() < 8 + payload_len {
            return None;
        }

        Some(Self {
            header,
            payload: data[8..8 + payload_len].to_vec(),
        })
    }
}

impl fmt::Display for FbcPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "FBC [seq={}, cmd=0x{:02X}, len={}]",
            self.header.seq, self.header.cmd, self.header.length
        )
    }
}

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

    #[error("Timeout")]
    Timeout,

    #[error("Invalid packet: {0}")]
    InvalidPacket(String),
}

pub type Result<T> = std::result::Result<T, FbcError>;

// =============================================================================
// FBC Raw Socket
// =============================================================================

/// Raw Ethernet socket for FBC protocol communication
pub struct FbcRawSocket {
    _interface: NetworkInterface,
    our_mac: [u8; 6],
    tx: Box<dyn DataLinkSender>,
    rx: Box<dyn DataLinkReceiver>,
    seq: u16,
}

impl FbcRawSocket {
    /// Create a new FBC raw socket on the specified network interface
    ///
    /// # Arguments
    /// * `interface_name` - Name of network interface (e.g., "Ethernet", "eth0")
    ///
    /// # Platform Notes
    /// - **Windows**: Requires Administrator privileges for raw sockets
    /// - **Linux**: Requires CAP_NET_RAW capability or root
    pub fn new(interface_name: &str) -> Result<Self> {
        // Find interface by exact or partial match on name/description
        let interfaces = pnet_datalink::interfaces();

        let interface = interfaces
            .into_iter()
            .find(|iface| {
                iface.name == interface_name
                    || iface.description == interface_name
                    || iface.name.contains(interface_name)
                    || iface.description.contains(interface_name)
            })
            .ok_or_else(|| FbcError::Interface(format!("Interface '{}' not found", interface_name)))?;

        let our_mac = interface.mac
            .ok_or_else(|| FbcError::Interface("Interface has no MAC address".into()))?
            .octets();

        // Create channel
        // Promiscuous mode required on Windows to capture non-IP EtherType (0x88B5)
        let config = pnet_datalink::Config {
            promiscuous: true,
            ..Default::default()
        };
        let (tx, rx) = match pnet_datalink::channel(&interface, config) {
            Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
            Ok(_) => return Err(FbcError::Interface("Unexpected channel type".into())),
            Err(e) => return Err(FbcError::Interface(format!("Failed to create channel: {}", e))),
        };

        Ok(Self {
            _interface: interface,
            our_mac,
            tx,
            rx,
            seq: 0,
        })
    }

    /// List available network interfaces
    pub fn list_interfaces() -> Vec<String> {
        pnet_datalink::interfaces()
            .into_iter()
            .filter(|iface| iface.mac.is_some())
            .map(|iface| {
                if iface.description.is_empty() {
                    iface.name
                } else {
                    format!("{} ({})", iface.name, iface.description)
                }
            })
            .collect()
    }

    /// Get our MAC address
    pub fn our_mac(&self) -> [u8; 6] {
        self.our_mac
    }

    /// Send an FBC packet
    pub fn send(&mut self, dst_mac: [u8; 6], packet: &FbcPacket) -> Result<()> {
        // Serialize FBC packet
        let fbc_data = packet.serialize();

        // Build Ethernet frame: header (14) + FBC data
        let frame_len = 14 + fbc_data.len();
        let mut buffer = vec![0u8; frame_len.max(60)];  // Minimum Ethernet frame size

        {
            let mut eth = MutableEthernetPacket::new(&mut buffer)
                .ok_or_else(|| FbcError::Send("Failed to create Ethernet packet".into()))?;

            eth.set_destination(MacAddr::new(
                dst_mac[0], dst_mac[1], dst_mac[2], dst_mac[3], dst_mac[4], dst_mac[5]
            ));
            eth.set_source(MacAddr::new(
                self.our_mac[0], self.our_mac[1], self.our_mac[2],
                self.our_mac[3], self.our_mac[4], self.our_mac[5]
            ));
            eth.set_ethertype(pnet_packet::ethernet::EtherType(ETHERTYPE_FBC));
        }

        // Copy FBC data after Ethernet header
        buffer[14..14 + fbc_data.len()].copy_from_slice(&fbc_data);

        // Send frame
        self.tx.send_to(&buffer, None)
            .ok_or_else(|| FbcError::Send("Send returned None".into()))?
            .map_err(|e| FbcError::Send(e.to_string()))?;

        Ok(())
    }

    /// Receive an FBC packet (non-blocking)
    pub fn recv(&mut self) -> Result<Option<([u8; 6], FbcPacket)>> {
        match self.rx.next() {
            Ok(packet) => {
                if let Some(eth) = EthernetPacket::new(packet) {
                    // Check EtherType
                    if eth.get_ethertype().0 != ETHERTYPE_FBC {
                        return Ok(None);
                    }

                    let payload = eth.payload();
                    if payload.len() < 8 {
                        return Ok(None);
                    }

                    // Parse FBC packet
                    if let Some(fbc_packet) = FbcPacket::parse(payload) {
                        let src_mac = eth.get_source().octets();
                        return Ok(Some((src_mac, fbc_packet)));
                    }
                }
                Ok(None)
            }
            Err(e) => Err(FbcError::Receive(e.to_string())),
        }
    }

    /// Receive an FBC packet with timeout
    pub fn recv_timeout(&mut self, timeout: Duration) -> Result<Option<([u8; 6], FbcPacket)>> {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some(result) = self.recv()? {
                return Ok(Some(result));
            }
            std::thread::sleep(Duration::from_micros(100));
        }

        Ok(None)
    }

    /// Get next sequence number
    pub fn next_seq(&mut self) -> u16 {
        let seq = self.seq;
        self.seq = self.seq.wrapping_add(1);
        seq
    }

    /// Send command and wait for response
    pub fn send_and_wait(
        &mut self,
        dst_mac: [u8; 6],
        cmd: u8,
        payload: Vec<u8>,
        timeout: Duration,
    ) -> Result<FbcPacket> {
        let seq = self.next_seq();
        let packet = FbcPacket::with_payload(cmd, seq, payload);

        self.send(dst_mac, &packet)?;

        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if let Some((src_mac, rsp_packet)) = self.recv()? {
                if src_mac == dst_mac && rsp_packet.header.cmd == cmd {
                    return Ok(rsp_packet);
                }
            }
            std::thread::sleep(Duration::from_micros(100));
        }

        Err(FbcError::Timeout)
    }
}

// =============================================================================
// Utility Functions
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

/// Broadcast MAC address
pub const BROADCAST_MAC: [u8; 6] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
