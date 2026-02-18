//! FBC Host Library v2 - Raw Ethernet Communication
//!
//! No TCP/IP. No bullshit. Just Layer 2 Ethernet frames.
//!
//! # New FBC Protocol
//!
//! The `fbc_protocol` module contains the NEW protocol that matches the firmware exactly.
//! Use this for new development. The old protocol in this file is legacy.
//!
//! # Legacy Protocol (below)
//!
//! No TCP/IP. No bullshit. Just Layer 2 Ethernet frames.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────┐                    ┌──────────────────┐
//! │    GUI / CLI     │                    │   FBC Board(s)   │
//! │                  │   Raw Ethernet     │                  │
//! │  FbcClient ──────┼───────────────────▶│  MAC: from EEPROM│
//! │                  │   EtherType 0x88B5 │                  │
//! └──────────────────┘                    └──────────────────┘
//! ```
//!
//! # Features
//!
//! - **Zero-config discovery**: Boards announce themselves, no IP setup
//! - **Sub-100µs latency**: Raw Ethernet, no protocol overhead
//! - **Multi-board support**: 88+ boards on one switch
//! - **Collision avoidance**: Staggered discovery responses
//!
//! # Usage
//!
//! ```no_run
//! use fbc_host::FbcClient;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create client on network interface
//!     let mut client = FbcClient::new("Ethernet")?; // Windows interface name
//!
//!     // Discover all boards
//!     let boards = client.discover(std::time::Duration::from_secs(1)).await?;
//!     println!("Found {} boards", boards.len());
//!
//!     for board in &boards {
//!         println!("  Board {}: MAC={}, Status={:?}",
//!             board.board_id, board.mac, board.status);
//!     }
//!
//!     // Upload and run a script on first board
//!     if let Some(board) = boards.first() {
//!         let script = std::fs::read("test.fbc")?;
//!         client.upload_script(&board.mac, 0, &script).await?;
//!         client.run_script(&board.mac, 0, 1000).await?;
//!
//!         // Wait for completion
//!         let status = client.wait_done(&board.mac, std::time::Duration::from_secs(60)).await?;
//!         println!("Completed: {} cycles, {} errors", status.cycle_count, status.error_count);
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod fbc_protocol;
pub mod vector;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pnet_datalink::{self, Channel, DataLinkReceiver, DataLinkSender, NetworkInterface, MacAddr};
use pnet_packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet_packet::Packet;
use tokio::sync::{mpsc, Mutex, RwLock};
use byteorder::{BigEndian, ByteOrder};
use thiserror::Error;

// =============================================================================
// Protocol Constants
// =============================================================================

/// FBC EtherType (custom protocol identifier)
pub const ETHERTYPE_FBC: u16 = 0x88B5;

/// Broadcast MAC address
pub const BROADCAST_MAC: [u8; 6] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

/// Maximum payload per frame
pub const MAX_PAYLOAD: usize = 1494;  // 1500 - 6 (our header)

// Command opcodes
pub const CMD_DISCOVER: u8 = 0x01;
pub const CMD_UPLOAD: u8 = 0x02;
pub const CMD_RUN: u8 = 0x03;
pub const CMD_STOP: u8 = 0x04;
pub const CMD_STATUS: u8 = 0x05;
pub const CMD_RESULTS: u8 = 0x06;
pub const CMD_CONFIG: u8 = 0x07;
pub const CMD_PING: u8 = 0xFE;

// Response codes
pub const RSP_OK: u8 = 0x00;
pub const RSP_ERROR: u8 = 0x01;
pub const RSP_BUSY: u8 = 0x02;
pub const RSP_INVALID: u8 = 0x03;

// Board status
pub const STATUS_IDLE: u8 = 0x00;
pub const STATUS_RUNNING: u8 = 0x01;
pub const STATUS_DONE: u8 = 0x02;
pub const STATUS_ERROR: u8 = 0x03;
pub const STATUS_SYNC_WAIT: u8 = 0x04;

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

// =============================================================================
// Board Types
// =============================================================================

/// Board information from discovery
#[derive(Debug, Clone)]
pub struct BoardInfo {
    pub mac: [u8; 6],
    pub board_id: u16,
    pub serial: u32,
    pub hw_rev: u16,
    pub status: BoardStatus,
    pub last_seen: Instant,
}

/// Board execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardStatus {
    Idle,
    Running,
    Done,
    Error,
    SyncWait,
    Unknown(u8),
}

impl From<u8> for BoardStatus {
    fn from(val: u8) -> Self {
        match val {
            STATUS_IDLE => BoardStatus::Idle,
            STATUS_RUNNING => BoardStatus::Running,
            STATUS_DONE => BoardStatus::Done,
            STATUS_ERROR => BoardStatus::Error,
            STATUS_SYNC_WAIT => BoardStatus::SyncWait,
            x => BoardStatus::Unknown(x),
        }
    }
}

/// Detailed status from STATUS command
#[derive(Debug, Clone)]
pub struct StatusResponse {
    pub status: BoardStatus,
    pub cycle_count: u64,
    pub vector_count: u32,
    pub error_count: u32,
}

// =============================================================================
// FBC Client
// =============================================================================

/// FBC Client for raw Ethernet communication
pub struct FbcClient {
    interface: NetworkInterface,
    our_mac: [u8; 6],
    tx: Arc<Mutex<Box<dyn DataLinkSender>>>,
    rx: Arc<Mutex<Box<dyn DataLinkReceiver>>>,
    boards: Arc<RwLock<HashMap<[u8; 6], BoardInfo>>>,
    seq: Arc<Mutex<u16>>,
}

impl FbcClient {
    /// Create a new FBC client on the specified network interface
    ///
    /// # Arguments
    /// * `interface_name` - Name of network interface (e.g., "Ethernet", "eth0")
    ///
    /// # Platform Notes
    /// - **Windows**: Requires Administrator privileges for raw sockets
    /// - **Linux**: Requires CAP_NET_RAW capability or root
    pub fn new(interface_name: &str) -> Result<Self> {
        // Find interface
        let interfaces = pnet_datalink::interfaces();
        let interface = interfaces
            .into_iter()
            .find(|iface| iface.name == interface_name || iface.description == interface_name)
            .ok_or_else(|| FbcError::Interface(format!("Interface '{}' not found", interface_name)))?;

        let our_mac = interface.mac
            .ok_or_else(|| FbcError::Interface("Interface has no MAC address".into()))?
            .octets();

        // Create channel
        let (tx, rx) = match pnet_datalink::channel(&interface, Default::default()) {
            Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
            Ok(_) => return Err(FbcError::Interface("Unexpected channel type".into())),
            Err(e) => return Err(FbcError::Interface(format!("Failed to create channel: {}", e))),
        };

        Ok(Self {
            interface,
            our_mac,
            tx: Arc::new(Mutex::new(tx)),
            rx: Arc::new(Mutex::new(rx)),
            boards: Arc::new(RwLock::new(HashMap::new())),
            seq: Arc::new(Mutex::new(0)),
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

    /// Discover all FBC boards on the network
    pub async fn discover(&self, timeout: Duration) -> Result<Vec<BoardInfo>> {
        // Clear existing boards
        self.boards.write().await.clear();

        // Send discovery broadcast
        self.send_frame(&BROADCAST_MAC, CMD_DISCOVER, &[]).await?;

        // Collect responses for timeout duration
        let start = Instant::now();
        while start.elapsed() < timeout {
            if let Ok(Ok(Some((src_mac, cmd, _seq, payload)))) =
                tokio::time::timeout(Duration::from_millis(50), self.recv_frame()).await
            {
                if cmd == CMD_DISCOVER && payload.len() >= 15 {
                    let board = BoardInfo {
                        board_id: BigEndian::read_u16(&payload[0..2]),
                        serial: BigEndian::read_u32(&payload[2..6]),
                        hw_rev: BigEndian::read_u16(&payload[6..8]),
                        status: BoardStatus::from(payload[8]),
                        mac: src_mac,
                        last_seen: Instant::now(),
                    };
                    self.boards.write().await.insert(src_mac, board);
                }
            }
        }

        Ok(self.boards.read().await.values().cloned().collect())
    }

    /// Upload a script to a board
    pub async fn upload_script(&self, mac: &[u8; 6], script_id: u8, data: &[u8]) -> Result<()> {
        // Build payload: script_id + data
        let mut payload = Vec::with_capacity(1 + data.len());
        payload.push(script_id);
        payload.extend_from_slice(data);

        // May need to chunk if too large
        if payload.len() <= MAX_PAYLOAD {
            self.send_and_wait(mac, CMD_UPLOAD, &payload).await?;
        } else {
            // Chunked upload (future enhancement)
            return Err(FbcError::Send("Script too large for single frame".into()));
        }

        Ok(())
    }

    /// Run a script on a board
    pub async fn run_script(&self, mac: &[u8; 6], script_id: u8, loop_count: u32) -> Result<()> {
        let mut payload = [0u8; 5];
        payload[0] = script_id;
        BigEndian::write_u32(&mut payload[1..5], loop_count);

        self.send_and_wait(mac, CMD_RUN, &payload).await
    }

    /// Stop execution on a board
    pub async fn stop(&self, mac: &[u8; 6]) -> Result<()> {
        self.send_and_wait(mac, CMD_STOP, &[]).await
    }

    /// Get status from a board
    pub async fn get_status(&self, mac: &[u8; 6]) -> Result<StatusResponse> {
        self.send_frame(mac, CMD_STATUS, &[]).await?;

        let timeout = Duration::from_millis(100);
        let start = Instant::now();

        while start.elapsed() < timeout {
            if let Ok(Ok(Some((src_mac, cmd, _seq, payload)))) =
                tokio::time::timeout(Duration::from_millis(10), self.recv_frame()).await
            {
                if src_mac == *mac && cmd == CMD_STATUS && payload.len() >= 17 {
                    return Ok(StatusResponse {
                        status: BoardStatus::from(payload[0]),
                        cycle_count: BigEndian::read_u64(&payload[1..9]),
                        vector_count: BigEndian::read_u32(&payload[9..13]),
                        error_count: BigEndian::read_u32(&payload[13..17]),
                    });
                }
            }
        }

        Err(FbcError::Timeout)
    }

    /// Wait for execution to complete
    pub async fn wait_done(&self, mac: &[u8; 6], timeout: Duration) -> Result<StatusResponse> {
        let start = Instant::now();

        while start.elapsed() < timeout {
            let status = self.get_status(mac).await?;
            match status.status {
                BoardStatus::Done | BoardStatus::Error => return Ok(status),
                BoardStatus::Idle => return Err(FbcError::Board("Board is idle, not running".into())),
                _ => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        }

        Err(FbcError::Timeout)
    }

    /// Ping a board
    pub async fn ping(&self, mac: &[u8; 6]) -> Result<Duration> {
        let start = Instant::now();
        self.send_and_wait(mac, CMD_PING, &[]).await?;
        Ok(start.elapsed())
    }

    /// Configure pin type on a board
    pub async fn set_pin_config(&self, mac: &[u8; 6], configs: &[(u8, u8)]) -> Result<()> {
        let mut payload = Vec::with_capacity(configs.len() * 2);
        for (pin, pin_type) in configs {
            payload.push(*pin);
            payload.push(*pin_type);
        }
        self.send_and_wait(mac, CMD_CONFIG, &payload).await
    }

    // =========================================================================
    // Internal Methods
    // =========================================================================

    async fn get_seq(&self) -> u16 {
        let mut seq = self.seq.lock().await;
        let val = *seq;
        *seq = seq.wrapping_add(1);
        val
    }

    async fn send_frame(&self, dst_mac: &[u8; 6], cmd: u8, payload: &[u8]) -> Result<()> {
        let seq = self.get_seq().await;

        // Build frame: Ethernet header (14) + our header (5) + payload
        let frame_len = 14 + 5 + payload.len();
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

        // Our header: cmd(1) + seq(2) + len(2) + payload
        buffer[14] = cmd;
        BigEndian::write_u16(&mut buffer[15..17], seq);
        BigEndian::write_u16(&mut buffer[17..19], payload.len() as u16);
        if !payload.is_empty() {
            buffer[19..19 + payload.len()].copy_from_slice(payload);
        }

        // Send
        let mut tx = self.tx.lock().await;
        tx.send_to(&buffer, None)
            .ok_or_else(|| FbcError::Send("Send returned None".into()))?
            .map_err(|e| FbcError::Send(e.to_string()))?;

        Ok(())
    }

    async fn recv_frame(&self) -> Result<Option<([u8; 6], u8, u16, Vec<u8>)>> {
        let mut rx = self.rx.lock().await;

        match rx.next() {
            Ok(packet) => {
                if let Some(eth) = EthernetPacket::new(packet) {
                    // Check EtherType
                    if eth.get_ethertype().0 != ETHERTYPE_FBC {
                        return Ok(None);
                    }

                    let payload = eth.payload();
                    if payload.len() < 5 {
                        return Ok(None);
                    }

                    let src_mac: [u8; 6] = eth.get_source().octets();
                    let cmd = payload[0];
                    let seq = BigEndian::read_u16(&payload[1..3]);
                    let len = BigEndian::read_u16(&payload[3..5]) as usize;

                    if payload.len() >= 5 + len {
                        let data = payload[5..5 + len].to_vec();
                        return Ok(Some((src_mac, cmd, seq, data)));
                    }
                }
                Ok(None)
            }
            Err(e) => Err(FbcError::Receive(e.to_string())),
        }
    }

    async fn send_and_wait(&self, mac: &[u8; 6], cmd: u8, payload: &[u8]) -> Result<()> {
        self.send_frame(mac, cmd, payload).await?;

        let timeout = Duration::from_millis(100);
        let start = Instant::now();

        while start.elapsed() < timeout {
            if let Ok(Ok(Some((src_mac, rsp_cmd, _seq, rsp_payload)))) =
                tokio::time::timeout(Duration::from_millis(10), self.recv_frame()).await
            {
                if src_mac == *mac && rsp_cmd == cmd {
                    // Check status if present
                    if !rsp_payload.is_empty() && rsp_payload[0] != RSP_OK {
                        return Err(FbcError::Board(format!("Board returned error: {}", rsp_payload[0])));
                    }
                    return Ok(());
                }
            }
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
