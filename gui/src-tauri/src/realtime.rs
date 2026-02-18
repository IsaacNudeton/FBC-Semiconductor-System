//! Real-time board detection and monitoring
//!
//! Event-driven system that automatically detects board connections/disconnections
//! by listening for FBC heartbeat packets. No polling required.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};

use crate::switch::{RackPosition, SwitchConfig};

/// Board state tracked in real-time
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LiveBoardState {
    pub mac: String,
    pub position: Option<RackPosition>,

    // Connection state
    pub online: bool,
    pub last_seen: i64,  // Unix timestamp ms
    pub first_seen: i64, // When this session started

    // From heartbeat
    pub state: String,        // "idle", "running", "error", "done"
    pub cycles: u64,
    pub errors: u64,
    pub temp_c: f32,

    // Computed
    pub run_time_ms: u64,     // How long it's been running this session
    pub uptime_ms: u64,       // How long since first seen
}

/// Event emitted when board state changes
#[derive(Debug, Clone, serde::Serialize)]
pub enum BoardEvent {
    Connected { mac: String, position: Option<RackPosition> },
    Disconnected { mac: String },
    StateChanged { mac: String, old_state: String, new_state: String },
    Error { mac: String, error_count: u64 },
    Heartbeat { mac: String, state: LiveBoardState },
}

/// FBC Protocol constants
pub mod fbc {
    pub const ETHERTYPE: u16 = 0x88B5;
    pub const MAGIC: u16 = 0xFBC0;
    pub const ANNOUNCE: u8 = 0x01;    // Sent on boot
    pub const HEARTBEAT: u8 = 0x50;   // Sent during Running state
}

/// Heartbeat packet from firmware (matches fbc_protocol.rs)
#[derive(Debug, Clone)]
pub struct HeartbeatPacket {
    pub src_mac: [u8; 6],
    pub cycles: u32,
    pub errors: u32,
    pub temp_c: i16,  // Temperature × 10 (e.g., 452 = 45.2°C)
    pub state: u8,
}

impl HeartbeatPacket {
    /// Parse heartbeat from raw Ethernet frame
    ///
    /// Frame structure:
    /// - Bytes 0-5: Destination MAC
    /// - Bytes 6-11: Source MAC
    /// - Bytes 12-13: EtherType (0x88B5)
    /// - Bytes 14-15: FBC Magic (0xFBC0)
    /// - Bytes 16-17: Sequence number
    /// - Byte 18: Command (0x50 = HEARTBEAT)
    /// - Byte 19: Flags
    /// - Bytes 20-21: Payload length
    /// - Bytes 22+: Payload (cycles, errors, temp, state)
    pub fn parse(data: &[u8]) -> Option<Self> {
        // Minimum size: 14 (eth) + 8 (fbc header) + 11 (heartbeat payload) = 33 bytes
        if data.len() < 33 {
            return None;
        }

        // Check EtherType (0x88B5 = FBC)
        let ethertype = u16::from_be_bytes([data[12], data[13]]);
        if ethertype != fbc::ETHERTYPE {
            return None;
        }

        // Check FBC Magic (0xFBC0)
        let magic = u16::from_be_bytes([data[14], data[15]]);
        if magic != fbc::MAGIC {
            return None;
        }

        // Check command (0x50 = HEARTBEAT or 0x01 = ANNOUNCE)
        let cmd = data[18];

        // Extract source MAC
        let mut src_mac = [0u8; 6];
        src_mac.copy_from_slice(&data[6..12]);

        // Handle ANNOUNCE (board just booted - idle state)
        if cmd == fbc::ANNOUNCE {
            return Some(Self {
                src_mac,
                cycles: 0,
                errors: 0,
                temp_c: 0,
                state: 0, // Idle
            });
        }

        // Only accept HEARTBEAT from here
        if cmd != fbc::HEARTBEAT {
            return None;
        }

        // Parse payload (starts at byte 22, big-endian)
        // Payload: cycles(4) + errors(4) + temp_c(2) + state(1)
        let cycles = u32::from_be_bytes([data[22], data[23], data[24], data[25]]);
        let errors = u32::from_be_bytes([data[26], data[27], data[28], data[29]]);
        let temp_c = i16::from_be_bytes([data[30], data[31]]);
        let state = data[32];

        Some(Self {
            src_mac,
            cycles,
            errors,
            temp_c,
            state,
        })
    }

    pub fn mac_string(&self) -> String {
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.src_mac[0], self.src_mac[1], self.src_mac[2],
            self.src_mac[3], self.src_mac[4], self.src_mac[5]
        )
    }

    pub fn state_string(&self) -> String {
        match self.state {
            0 => "idle".into(),
            1 => "running".into(),
            2 => "done".into(),
            3 => "error".into(),
            _ => "unknown".into(),
        }
    }

    pub fn temp_celsius(&self) -> f32 {
        // Firmware sends temp × 10 (e.g., 452 = 45.2°C)
        (self.temp_c as f32) / 10.0
    }
}

/// Real-time board monitor
pub struct RealtimeMonitor {
    /// All known boards (MAC -> state)
    boards: Arc<RwLock<HashMap<String, LiveBoardState>>>,

    /// Port-to-position mapping
    switch_config: Arc<RwLock<SwitchConfig>>,

    /// Event broadcaster
    event_tx: broadcast::Sender<BoardEvent>,

    /// Timeout for considering a board disconnected
    disconnect_timeout: Duration,
}

impl RealtimeMonitor {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(1000);

        Self {
            boards: Arc::new(RwLock::new(HashMap::new())),
            switch_config: Arc::new(RwLock::new(SwitchConfig::default())),
            event_tx,
            disconnect_timeout: Duration::from_millis(500), // 5 missed heartbeats
        }
    }

    /// Subscribe to board events
    pub fn subscribe(&self) -> broadcast::Receiver<BoardEvent> {
        self.event_tx.subscribe()
    }

    /// Get all current board states
    pub async fn get_all_boards(&self) -> Vec<LiveBoardState> {
        self.boards.read().await.values().cloned().collect()
    }

    /// Get a specific board's state
    pub async fn get_board(&self, mac: &str) -> Option<LiveBoardState> {
        self.boards.read().await.get(mac).cloned()
    }

    /// Update switch configuration
    pub async fn set_switch_config(&self, config: SwitchConfig) {
        *self.switch_config.write().await = config;
    }

    /// Process incoming heartbeat packet
    pub async fn process_heartbeat(&self, heartbeat: HeartbeatPacket) {
        let mac = heartbeat.mac_string();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let mut boards = self.boards.write().await;

        let is_new = !boards.contains_key(&mac);
        let old_state = boards.get(&mac).map(|b| b.state.clone());

        // Get or create board entry
        let board = boards.entry(mac.clone()).or_insert_with(|| {
            LiveBoardState {
                mac: mac.clone(),
                position: None, // Will be set by switch query
                online: true,
                last_seen: now_ms,
                first_seen: now_ms,
                state: "unknown".into(),
                cycles: 0,
                errors: 0,
                temp_c: 0.0,
                run_time_ms: 0,
                uptime_ms: 0,
            }
        });

        // Update from heartbeat
        let new_state = heartbeat.state_string();
        board.online = true;
        board.last_seen = now_ms;
        board.state = new_state.clone();
        board.cycles = heartbeat.cycles as u64;
        board.errors = heartbeat.errors as u64;
        board.temp_c = heartbeat.temp_celsius();
        board.uptime_ms = (now_ms - board.first_seen) as u64;

        // Calculate run time (cycles * clock period, assuming 100MHz = 10ns/cycle)
        // Or just track time since state became "running"
        if new_state == "running" {
            board.run_time_ms = board.uptime_ms; // Simplified for now
        }

        let board_clone = board.clone();
        drop(boards);

        // Emit events
        if is_new {
            // New board detected - query switch for position
            let _ = self.event_tx.send(BoardEvent::Connected {
                mac: mac.clone(),
                position: None, // TODO: Query switch
            });
        }

        if let Some(old) = old_state {
            if old != new_state {
                let _ = self.event_tx.send(BoardEvent::StateChanged {
                    mac: mac.clone(),
                    old_state: old,
                    new_state: new_state.clone(),
                });
            }
        }

        // Always emit heartbeat event
        let _ = self.event_tx.send(BoardEvent::Heartbeat {
            mac,
            state: board_clone,
        });
    }

    /// Check for disconnected boards (call periodically)
    pub async fn check_timeouts(&self) {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let timeout_ms = self.disconnect_timeout.as_millis() as i64;

        let mut boards = self.boards.write().await;
        let mut disconnected = Vec::new();

        for (mac, board) in boards.iter_mut() {
            if board.online && (now_ms - board.last_seen) > timeout_ms {
                board.online = false;
                disconnected.push(mac.clone());
            }
        }

        drop(boards);

        for mac in disconnected {
            let _ = self.event_tx.send(BoardEvent::Disconnected { mac });
        }
    }

    /// Look up position for a MAC by querying switch
    pub async fn lookup_position(&self, mac: &str) -> Option<RackPosition> {
        let config = self.switch_config.read().await;

        // Query switch MAC table
        match crate::switch::discover_board_positions(&config) {
            Ok(positions) => positions.get(mac).cloned(),
            Err(_) => None,
        }
    }

    /// Update a board's position
    pub async fn set_board_position(&self, mac: &str, position: RackPosition) {
        let mut boards = self.boards.write().await;
        if let Some(board) = boards.get_mut(mac) {
            board.position = Some(position);
        }
    }
}

impl Default for RealtimeMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_parse() {
        // Construct a mock heartbeat packet
        let mut packet = vec![0u8; 28];

        // Destination MAC
        packet[0..6].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        // Source MAC
        packet[6..12].copy_from_slice(&[0x00, 0x50, 0x56, 0x45, 0x00, 0x01]);
        // EtherType (0x88B5)
        packet[12] = 0x88;
        packet[13] = 0xB5;
        // Packet type (0x02 = HEARTBEAT)
        packet[14] = 0x02;
        // Cycles (1000)
        packet[15..19].copy_from_slice(&1000u32.to_le_bytes());
        // Errors (0)
        packet[19..23].copy_from_slice(&0u32.to_le_bytes());
        // Temp (raw)
        packet[23..25].copy_from_slice(&32768u16.to_le_bytes());
        // State (1 = running)
        packet[25] = 1;

        let heartbeat = HeartbeatPacket::parse(&packet).unwrap();
        assert_eq!(heartbeat.mac_string(), "00:50:56:45:00:01");
        assert_eq!(heartbeat.cycles, 1000);
        assert_eq!(heartbeat.errors, 0);
        assert_eq!(heartbeat.state_string(), "running");
    }
}
