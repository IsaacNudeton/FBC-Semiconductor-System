//! Application State
//!
//! Manages FBC connection, board tracking, and configuration.

use crate::config::RackConfig;
use crate::fbc::{
    self, AnalogChannels, AnalogReading, BoardInfo, BoardStatus, CalibrationData,
    ControllerState, DutMetadata, EepromData, EepromHeader, FbcSocket, FastPinState,
    PmBusRail, PmBusStatus, RailConfig, VectorEngineStatus, VectorState, VicorCore,
    VicorCoreStatus, VicorStatus, BROADCAST_MAC,
};
use crate::realtime::{BoardEvent, HeartbeatPacket, LiveBoardState, RealtimeMonitor};
use crate::ssh::SshSessionManager;
use crate::switch::SwitchConfig;
use byteorder::{BigEndian, ByteOrder};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};

/// FBC firmware update command codes
mod fw_cmd {
    pub const INFO_REQ: u8 = 0xE1;
    pub const INFO_RSP: u8 = 0xE2;
    pub const BEGIN: u8 = 0xE3;
    pub const BEGIN_ACK: u8 = 0xE4;
    pub const CHUNK: u8 = 0xE5;
    pub const CHUNK_ACK: u8 = 0xE6;
    pub const COMMIT: u8 = 0xE7;
    pub const COMMIT_ACK: u8 = 0xE8;
}

/// Application state shared across Tauri commands
#[derive(Clone)]
pub struct AppState {
    socket: Arc<RwLock<Option<FbcSocket>>>,
    boards: Arc<RwLock<HashMap<String, BoardInfo>>>,
    config: Arc<RwLock<RackConfig>>,
    /// Real-time board monitor
    realtime: Arc<RealtimeMonitor>,
    /// Switch configuration for position discovery
    switch_config: Arc<RwLock<SwitchConfig>>,
    /// SSH session manager for fleet terminal
    ssh: Arc<SshSessionManager>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            socket: Arc::new(RwLock::new(None)),
            boards: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(RackConfig::default())),
            realtime: Arc::new(RealtimeMonitor::new()),
            switch_config: Arc::new(RwLock::new(SwitchConfig::default())),
            ssh: Arc::new(SshSessionManager::new()),
        }
    }

    /// Get a reference to the realtime monitor
    pub fn realtime(&self) -> &Arc<RealtimeMonitor> {
        &self.realtime
    }

    /// Subscribe to board events
    pub fn subscribe_events(&self) -> broadcast::Receiver<BoardEvent> {
        self.realtime.subscribe()
    }

    /// Get all live board states
    pub async fn get_live_boards(&self) -> Vec<LiveBoardState> {
        self.realtime.get_all_boards().await
    }

    /// Get a specific board's live state
    pub async fn get_live_board(&self, mac: &str) -> Option<LiveBoardState> {
        self.realtime.get_board(mac).await
    }

    /// Update switch configuration
    pub async fn set_switch_config(&self, config: SwitchConfig) {
        *self.switch_config.write().await = config.clone();
        self.realtime.set_switch_config(config).await;
    }

    /// Get switch configuration
    pub async fn get_switch_config(&self) -> SwitchConfig {
        self.switch_config.read().await.clone()
    }

    /// Get SSH session manager
    pub fn ssh(&self) -> &Arc<SshSessionManager> {
        &self.ssh
    }

    /// Connect to FBC network
    pub async fn connect(&self, interface: &str) -> fbc::Result<()> {
        let socket = FbcSocket::new(interface)?;
        *self.socket.write().await = Some(socket);

        // Start heartbeat listener
        self.start_heartbeat_listener(interface).await;

        // Verify connection by sending a discovery broadcast
        // This ensures the socket actually works (not just opened)
        if let Err(e) = self.verify_connection().await {
            tracing::warn!("Connection verification failed: {}", e);
            // Don't fail connect, just warn - user can still try to use it
        }

        Ok(())
    }

    /// Verify connection works by sending discovery and checking for response
    async fn verify_connection(&self) -> fbc::Result<()> {
        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        // Send discovery broadcast
        socket
            .send(BROADCAST_MAC, fbc::setup::BIM_STATUS_REQ, &[])
            .await?;

        let our_mac_str = fbc::format_mac(&socket.our_mac());
        tracing::info!("Sent discovery broadcast on {}", our_mac_str);

        // Wait up to 500ms for any response
        let timeout = std::time::Duration::from_millis(500);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some((src_mac, cmd, _payload)) =
                socket.recv_timeout(std::time::Duration::from_millis(50)).await?
            {
                // Got a response! Connection verified
                let src_mac_str = fbc::format_mac(&src_mac);
                tracing::info!("Connection verified - received {} from {}", cmd, src_mac_str);
                return Ok(());
            }
        }

        // No response - connection may not work
        Err(fbc::FbcError::Timeout)
    }

    /// Start background listener for heartbeat packets
    async fn start_heartbeat_listener(&self, interface: &str) {
        use pnet_datalink::{self, Channel::Ethernet};

        let interfaces = pnet_datalink::interfaces();
        let iface = match interfaces.into_iter().find(|i| i.name == interface) {
            Some(i) => i,
            None => {
                tracing::warn!("Interface {} not found for heartbeat listener", interface);
                return;
            }
        };

        // Create a channel to receive packets
        let (_, mut rx) = match pnet_datalink::channel(&iface, Default::default()) {
            Ok(Ethernet(tx, rx)) => (tx, rx),
            Ok(_) => {
                tracing::warn!("Unknown channel type for {}", interface);
                return;
            }
            Err(e) => {
                tracing::warn!("Failed to create channel for {}: {}", interface, e);
                return;
            }
        };

        let realtime = self.realtime.clone();
        let iface_name = iface.name.clone();

        // Use an mpsc channel to forward heartbeats to async context
        let (tx, mut heartbeat_rx) = tokio::sync::mpsc::unbounded_channel::<HeartbeatPacket>();

        // Spawn blocking listener task
        std::thread::spawn(move || {
            tracing::info!("Heartbeat listener started on {}", iface_name);
            loop {
                match rx.next() {
                    Ok(packet) => {
                        // Try to parse as heartbeat
                        if let Some(heartbeat) = HeartbeatPacket::parse(packet) {
                            if tx.send(heartbeat).is_err() {
                                // Channel closed, exit
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Packet read error: {}", e);
                        // Small delay on error
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
            }
        });

        // Spawn async task to process heartbeats
        tokio::spawn(async move {
            while let Some(heartbeat) = heartbeat_rx.recv().await {
                realtime.process_heartbeat(heartbeat).await;
            }
        });
    }

    /// Disconnect from FBC network
    pub async fn disconnect(&self) {
        *self.socket.write().await = None;
        self.boards.write().await.clear();
    }

    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        self.socket.read().await.is_some()
    }

    /// Discover boards on the network
    pub async fn discover(&self) -> fbc::Result<Vec<BoardInfo>> {
        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        // Send discovery broadcast (using BIM_STATUS_REQ to trigger ANNOUNCE responses)
        socket
            .send(BROADCAST_MAC, fbc::setup::BIM_STATUS_REQ, &[])
            .await?;

        // Collect responses
        let mut discovered = Vec::new();
        let timeout = Duration::from_secs(2);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some((src_mac, cmd, payload)) =
                socket.recv_timeout(Duration::from_millis(100)).await?
            {
                if cmd == fbc::setup::ANNOUNCE && payload.len() >= 16 {
                    let info = parse_announce(&src_mac, &payload);
                    // Deduplicate by MAC — board may send multiple ANNOUNCEs
                    if !discovered.iter().any(|b: &BoardInfo| b.mac == info.mac) {
                        discovered.push(info);
                    }
                }
            }
        }

        // Update board map
        let mut boards = self.boards.write().await;
        for info in &discovered {
            boards.insert(info.mac.clone(), info.clone());
        }

        // Auto-assign positions for new boards
        let macs: Vec<String> = boards.keys().cloned().collect();
        drop(boards);
        self.config.write().await.auto_assign(&macs);

        Ok(discovered)
    }

    /// Get status of a specific board
    pub async fn get_status(&self, mac: &str) -> fbc::Result<BoardStatus> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::runtime::STATUS_REQ, &[]).await?;

        let timeout = Duration::from_millis(500);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some((src_mac, cmd, payload)) =
                socket.recv_timeout(Duration::from_millis(50)).await?
            {
                if src_mac == mac_bytes && cmd == fbc::runtime::STATUS_RSP && payload.len() >= 11 {
                    return Ok(parse_status(&payload));
                }
            }
        }

        Err(fbc::FbcError::Timeout)
    }

    /// Start test on a board
    pub async fn start(&self, mac: &str) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::runtime::START, &[]).await?;

        // Wait for ACK
        let timeout = Duration::from_millis(500);
        if socket.recv_timeout(timeout).await?.is_none() {
            return Err(fbc::FbcError::Timeout);
        }

        Ok(())
    }

    /// Stop test on a board
    pub async fn stop(&self, mac: &str) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::runtime::STOP, &[]).await?;

        let timeout = Duration::from_millis(500);
        if socket.recv_timeout(timeout).await?.is_none() {
            return Err(fbc::FbcError::Timeout);
        }

        Ok(())
    }

    /// Reset a board
    pub async fn reset(&self, mac: &str) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::runtime::RESET, &[]).await?;

        let timeout = Duration::from_millis(500);
        if socket.recv_timeout(timeout).await?.is_none() {
            return Err(fbc::FbcError::Timeout);
        }

        Ok(())
    }

    /// Upload vectors to a board
    pub async fn upload(&self, mac: &str, data: &[u8]) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        // Chunk the data
        let chunk_size = 1400; // Leave room for headers
        let total = data.len() as u32;
        let mut offset = 0u32;

        while (offset as usize) < data.len() {
            let end = ((offset as usize) + chunk_size).min(data.len());
            let chunk = &data[offset as usize..end];

            // Build payload: offset(4) + total(4) + chunk_size(2) + data
            let mut payload = Vec::with_capacity(10 + chunk.len());
            payload.extend_from_slice(&offset.to_be_bytes());
            payload.extend_from_slice(&total.to_be_bytes());
            payload.extend_from_slice(&(chunk.len() as u16).to_be_bytes());
            payload.extend_from_slice(chunk);

            socket
                .send(mac_bytes, fbc::setup::UPLOAD_VECTORS, &payload)
                .await?;

            // Wait for ACK
            let timeout = Duration::from_millis(500);
            if socket.recv_timeout(timeout).await?.is_none() {
                return Err(fbc::FbcError::Timeout);
            }

            offset += chunk.len() as u32;
        }

        Ok(())
    }

    /// Get fast pin state
    pub async fn get_fast_pins(&self, mac: &str) -> fbc::Result<FastPinState> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::fastpins::READ_REQ, &[]).await?;

        let timeout = Duration::from_millis(500);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some((src_mac, cmd, payload)) =
                socket.recv_timeout(Duration::from_millis(50)).await?
            {
                if src_mac == mac_bytes && cmd == fbc::fastpins::READ_RSP && payload.len() >= 12 {
                    return Ok(parse_fast_pins(&payload));
                }
            }
        }

        Err(fbc::FbcError::Timeout)
    }

    /// Set fast pin state
    pub async fn set_fast_pins(&self, mac: &str, dout: u32, oen: u32) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&dout.to_be_bytes());
        payload.extend_from_slice(&oen.to_be_bytes());

        socket.send(mac_bytes, fbc::fastpins::WRITE, &payload).await?;

        Ok(())
    }

    // =========================================================================
    // Analog Monitoring
    // =========================================================================

    /// Read all analog channels (32 total: 16 XADC + 16 external)
    pub async fn read_analog_channels(&self, mac: &str) -> fbc::Result<AnalogChannels> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::analog::READ_ALL_REQ, &[]).await?;

        let timeout = Duration::from_millis(500);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some((src_mac, cmd, payload)) =
                socket.recv_timeout(Duration::from_millis(50)).await?
            {
                if src_mac == mac_bytes && cmd == fbc::analog::READ_ALL_RSP && payload.len() >= 64 {
                    return Ok(parse_analog_channels(&payload));
                }
            }
        }

        Err(fbc::FbcError::Timeout)
    }

    // =========================================================================
    // Power Control - VICOR
    // =========================================================================

    /// Get VICOR core status
    pub async fn get_vicor_status(&self, mac: &str) -> fbc::Result<VicorStatus> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::power::VICOR_STATUS_REQ, &[]).await?;

        let timeout = Duration::from_millis(500);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some((src_mac, cmd, payload)) =
                socket.recv_timeout(Duration::from_millis(50)).await?
            {
                if src_mac == mac_bytes && cmd == fbc::power::VICOR_STATUS_RSP && payload.len() >= 30 {
                    return Ok(parse_vicor_status(&payload));
                }
            }
        }

        Err(fbc::FbcError::Timeout)
    }

    /// Enable/disable a VICOR core
    pub async fn set_vicor_enable(&self, mac: &str, core_id: u8, enable: bool) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        let payload = [core_id, if enable { 1 } else { 0 }];
        socket.send(mac_bytes, fbc::power::VICOR_ENABLE, &payload).await?;

        let timeout = Duration::from_millis(500);
        if socket.recv_timeout(timeout).await?.is_none() {
            return Err(fbc::FbcError::Timeout);
        }

        Ok(())
    }

    /// Set VICOR core voltage
    pub async fn set_vicor_voltage(&self, mac: &str, core_id: u8, voltage_mv: u16) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        let mut payload = [0u8; 3];
        payload[0] = core_id;
        BigEndian::write_u16(&mut payload[1..3], voltage_mv);
        socket.send(mac_bytes, fbc::power::VICOR_SET_VOLTAGE, &payload).await?;

        let timeout = Duration::from_millis(500);
        if socket.recv_timeout(timeout).await?.is_none() {
            return Err(fbc::FbcError::Timeout);
        }

        Ok(())
    }

    // =========================================================================
    // Power Control - PMBus
    // =========================================================================

    /// Get PMBus rail status
    pub async fn get_pmbus_status(&self, mac: &str) -> fbc::Result<PmBusStatus> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::power::PMBUS_STATUS_REQ, &[]).await?;

        let timeout = Duration::from_millis(500);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some((src_mac, cmd, payload)) =
                socket.recv_timeout(Duration::from_millis(50)).await?
            {
                if src_mac == mac_bytes && cmd == fbc::power::PMBUS_STATUS_RSP && payload.len() >= 2 {
                    return Ok(parse_pmbus_status(&payload));
                }
            }
        }

        Err(fbc::FbcError::Timeout)
    }

    /// Enable/disable a PMBus rail
    pub async fn set_pmbus_enable(&self, mac: &str, address: u8, enable: bool) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        let payload = [address, if enable { 1 } else { 0 }];
        socket.send(mac_bytes, fbc::power::PMBUS_ENABLE, &payload).await?;

        let timeout = Duration::from_millis(500);
        if socket.recv_timeout(timeout).await?.is_none() {
            return Err(fbc::FbcError::Timeout);
        }

        Ok(())
    }

    /// Emergency stop all power
    pub async fn emergency_stop(&self, mac: &str) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::power::EMERGENCY_STOP, &[]).await?;

        // Emergency stop is fire-and-forget for safety
        Ok(())
    }

    /// Execute power-on sequence
    pub async fn power_sequence_on(&self, mac: &str) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::power::POWER_SEQUENCE_ON, &[]).await?;

        let timeout = Duration::from_secs(5); // Sequences take longer
        if socket.recv_timeout(timeout).await?.is_none() {
            return Err(fbc::FbcError::Timeout);
        }

        Ok(())
    }

    /// Execute power-off sequence
    pub async fn power_sequence_off(&self, mac: &str) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::power::POWER_SEQUENCE_OFF, &[]).await?;

        let timeout = Duration::from_secs(5);
        if socket.recv_timeout(timeout).await?.is_none() {
            return Err(fbc::FbcError::Timeout);
        }

        Ok(())
    }

    // =========================================================================
    // EEPROM
    // =========================================================================

    /// Read EEPROM contents
    pub async fn read_eeprom(&self, mac: &str) -> fbc::Result<EepromData> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::eeprom::READ_REQ, &[]).await?;

        let timeout = Duration::from_millis(1000);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some((src_mac, cmd, payload)) =
                socket.recv_timeout(Duration::from_millis(50)).await?
            {
                if src_mac == mac_bytes && cmd == fbc::eeprom::READ_RSP && payload.len() >= 256 {
                    return Ok(parse_eeprom(&payload));
                }
            }
        }

        Err(fbc::FbcError::Timeout)
    }

    /// Write EEPROM data
    pub async fn write_eeprom(&self, mac: &str, offset: u8, data: &[u8]) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        // Payload: offset(1) + length(1) + data
        let mut payload = Vec::with_capacity(2 + data.len());
        payload.push(offset);
        payload.push(data.len() as u8);
        payload.extend_from_slice(data);

        socket.send(mac_bytes, fbc::eeprom::WRITE, &payload).await?;

        let timeout = Duration::from_millis(500);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some((src_mac, cmd, _)) =
                socket.recv_timeout(Duration::from_millis(50)).await?
            {
                if src_mac == mac_bytes && cmd == fbc::eeprom::WRITE_ACK {
                    return Ok(());
                }
            }
        }

        Err(fbc::FbcError::Timeout)
    }

    // =========================================================================
    // Vector Engine
    // =========================================================================

    /// Get vector engine status
    pub async fn get_vector_status(&self, mac: &str) -> fbc::Result<VectorEngineStatus> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::vector::STATUS_REQ, &[]).await?;

        let timeout = Duration::from_millis(500);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some((src_mac, cmd, payload)) =
                socket.recv_timeout(Duration::from_millis(50)).await?
            {
                if src_mac == mac_bytes && cmd == fbc::vector::STATUS_RSP && payload.len() >= 29 {
                    return Ok(parse_vector_status(&payload));
                }
            }
        }

        Err(fbc::FbcError::Timeout)
    }

    // =========================================================================
    // Error Log
    // =========================================================================

    /// Request error log entries from a board
    pub async fn request_error_log(
        &self,
        mac: &str,
        start_index: u32,
        count: u32,
    ) -> fbc::Result<fbc::ErrorLogResponse> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        // Build request payload: start_index (4 bytes) + count (4 bytes)
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&start_index.to_be_bytes());
        payload.extend_from_slice(&count.to_be_bytes());

        socket.send(mac_bytes, fbc::error_log::ERROR_LOG_REQ, &payload).await?;

        let timeout = Duration::from_millis(1000);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some((src_mac, cmd, payload)) =
                socket.recv_timeout(Duration::from_millis(50)).await?
            {
                if src_mac == mac_bytes && cmd == fbc::error_log::ERROR_LOG_RSP && payload.len() >= 8 {
                    return Ok(parse_error_log_response(&payload));
                }
            }
        }

        Err(fbc::FbcError::Timeout)
    }

    /// Load vectors from file data
    pub async fn load_vectors(&self, mac: &str, data: &[u8]) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        // Chunk the data similar to upload
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

            socket.send(mac_bytes, fbc::vector::LOAD, &payload).await?;

            // Wait for ACK
            let timeout = Duration::from_millis(500);
            let start = std::time::Instant::now();
            let mut acked = false;

            while start.elapsed() < timeout {
                if let Some((src_mac, cmd, _)) =
                    socket.recv_timeout(Duration::from_millis(50)).await?
                {
                    if src_mac == mac_bytes && cmd == fbc::vector::LOAD_ACK {
                        acked = true;
                        break;
                    }
                }
            }

            if !acked {
                return Err(fbc::FbcError::Timeout);
            }

            offset += chunk.len() as u32;
        }

        Ok(())
    }

    /// Start vector execution
    pub async fn start_vectors(&self, mac: &str, loops: u32) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        let mut payload = [0u8; 4];
        BigEndian::write_u32(&mut payload, loops);
        socket.send(mac_bytes, fbc::vector::START, &payload).await?;

        let timeout = Duration::from_millis(500);
        if socket.recv_timeout(timeout).await?.is_none() {
            return Err(fbc::FbcError::Timeout);
        }

        Ok(())
    }

    /// Pause vector execution
    pub async fn pause_vectors(&self, mac: &str) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::vector::PAUSE, &[]).await?;

        let timeout = Duration::from_millis(500);
        if socket.recv_timeout(timeout).await?.is_none() {
            return Err(fbc::FbcError::Timeout);
        }

        Ok(())
    }

    /// Resume vector execution
    pub async fn resume_vectors(&self, mac: &str) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::vector::RESUME, &[]).await?;

        let timeout = Duration::from_millis(500);
        if socket.recv_timeout(timeout).await?.is_none() {
            return Err(fbc::FbcError::Timeout);
        }

        Ok(())
    }

    /// Stop vector execution
    pub async fn stop_vectors(&self, mac: &str) -> fbc::Result<()> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fbc::vector::STOP, &[]).await?;

        let timeout = Duration::from_millis(500);
        if socket.recv_timeout(timeout).await?.is_none() {
            return Err(fbc::FbcError::Timeout);
        }

        Ok(())
    }

    /// Get rack configuration
    pub fn get_rack_config(&self) -> RackConfig {
        // Note: This is sync because config is rarely written
        // In production, you'd want to use try_read or make this async
        self.config.blocking_read().clone()
    }

    /// Set rack configuration
    pub fn set_rack_config(&self, config: RackConfig) {
        *self.config.blocking_write() = config;
    }

    // =========================================================================
    // Detailed Status (for BoardDetailPanel)
    // =========================================================================

    /// Get detailed board status
    pub async fn get_detailed_status(&self, mac: &str) -> fbc::Result<serde_json::Value> {
        // Get the basic status
        let status = self.get_status(mac).await?;
        let vector_status = self.get_vector_status(mac).await.ok();

        // Combine into detailed JSON
        let detailed = serde_json::json!({
            "state": format!("{:?}", status.state).to_lowercase(),
            "cycles": status.cycles,
            "errors": status.errors,
            "temp_c": status.temp_c,
            "rail_voltage_mv": status.rail_voltage_mv,
            "rail_current_ma": status.rail_current_ma,
            "fpga_vccint_mv": status.fpga_vccint_mv,
            "fpga_vccaux_mv": status.fpga_vccaux_mv,
            "fpga_vccbram_mv": 0, // Would need additional protocol support
            "fpga_temp_c": status.temp_c, // Same as die temp for now
            "uptime_secs": vector_status.as_ref().map(|v| (v.run_time_ms / 1000) as u64).unwrap_or(0),
            "vectors_loaded": vector_status.as_ref().map(|v| v.total_vectors > 0).unwrap_or(false),
            "vector_count": vector_status.as_ref().map(|v| v.total_vectors).unwrap_or(0),
            "current_vector": vector_status.as_ref().map(|v| v.current_address).unwrap_or(0),
            "freq_sel": 3, // Default 50MHz, would need protocol support for actual value
            "vec_clock_hz": 50_000_000
        });

        Ok(detailed)
    }

    /// Get EEPROM info formatted for BoardDetailPanel
    pub async fn get_eeprom_info(&self, mac: &str) -> fbc::Result<serde_json::Value> {
        let eeprom = self.read_eeprom(mac).await?;

        let is_programmed = eeprom.header.magic == 0xFBC0;
        let is_valid = is_programmed; // CRC check would go here

        let info = serde_json::json!({
            "magic": eeprom.header.magic,
            "version": eeprom.header.version,
            "bim_type": 0, // Would extract from raw data
            "hw_revision": eeprom.header.hw_revision,
            "serial_number": eeprom.header.board_serial,
            "manufacture_date": eeprom.header.mfg_date,
            "vendor": "FBC", // Would extract from raw data if stored
            "part_number": eeprom.dut.part_number,
            "description": format!("FBC Board S/N {}", eeprom.header.board_serial),
            "is_programmed": is_programmed,
            "is_valid": is_valid
        });

        Ok(info)
    }

    /// Resolve MAC: use explicit arg, or auto-select if only one board discovered
    async fn resolve_mac<'a>(&self, parts: &'a [&'a str], arg_index: usize) -> fbc::Result<String> {
        if parts.len() > arg_index {
            return Ok(parts[arg_index].to_string());
        }
        let boards = self.boards.read().await;
        if boards.len() == 1 {
            Ok(boards.keys().next().unwrap().clone())
        } else if boards.is_empty() {
            Err(fbc::FbcError::SendFailed("No boards discovered. Run 'discover' first.".into()))
        } else {
            Err(fbc::FbcError::SendFailed(format!("Multiple boards found. Specify MAC: {} <mac>", parts[0])))
        }
    }

    /// Execute a terminal command
    pub async fn execute_command(&self, command: &str) -> fbc::Result<String> {
        let parts: Vec<&str> = command.trim().split_whitespace().collect();
        if parts.is_empty() {
            return Ok(String::new());
        }

        match parts[0] {
            "help" => Ok(HELP_TEXT.to_string()),

            "list" | "ls" => {
                let boards = self.boards.read().await;
                if boards.is_empty() {
                    return Ok("No boards discovered. Run 'discover' first.".to_string());
                }

                let mut output = String::from("MAC Address        State    Position\n");
                output.push_str("─".repeat(45).as_str());
                output.push('\n');

                let config = self.config.read().await;
                for (mac, info) in boards.iter() {
                    let pos = config
                        .get_position(mac)
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "?".to_string());
                    output.push_str(&format!(
                        "{:<18} {:<8} {}\n",
                        mac,
                        format!("{:?}", info.state),
                        pos
                    ));
                }

                Ok(output)
            }

            "discover" => {
                let boards = self.discover().await?;
                Ok(format!("Discovered {} board(s)", boards.len()))
            }

            "status" => {
                let mac = self.resolve_mac(&parts, 1).await?;
                let status = self.get_status(&mac).await?;
                Ok(format!(
                    "Board: {}\nState: {:?}\nCycles: {}\nErrors: {}\nTemp: {:.1}°C\nVCCINT: {} mV\nVCCAUX: {} mV",
                    mac,
                    status.state,
                    status.cycles,
                    status.errors,
                    status.temp_c,
                    status.fpga_vccint_mv,
                    status.fpga_vccaux_mv
                ))
            }

            "start" => {
                let mac = self.resolve_mac(&parts, 1).await?;
                self.start(&mac).await?;
                Ok(format!("Started {}", mac))
            }

            "stop" => {
                let mac = self.resolve_mac(&parts, 1).await?;
                self.stop(&mac).await?;
                Ok(format!("Stopped {}", mac))
            }

            "reset" => {
                let mac = self.resolve_mac(&parts, 1).await?;
                self.reset(&mac).await?;
                Ok(format!("Reset {}", mac))
            }

            "fastpins" => {
                 let mac_result = self.resolve_mac(&parts, 1).await;
                 // If arg 1 looks like a MAC, use it; otherwise auto-resolve
                 let (mac, cmd_offset) = if parts.len() >= 2 && parts[1].contains(':') {
                     (parts[1].to_string(), 2)
                 } else if let Ok(m) = mac_result {
                     (m, 1)
                 } else {
                     return Ok("No board selected. Usage: fastpins [mac] [set <dout> <oen>]".into());
                 };
                 let mac = mac.as_str();
                 let rest = &parts[cmd_offset..];
                 if rest.is_empty() {
                     // Get status
                     let state = self.get_fast_pins(mac).await?;
                     Ok(format!(
                         "Fast Pins (Bank 35):\n  DOUT: 0x{:08X}\n  OEN:  0x{:08X}\n  DIN:  0x{:08X}",
                         state.dout, state.oen, state.din
                     ))
                 } else if rest.len() >= 3 && rest[0] == "set" {
                     let dout = match u32::from_str_radix(rest[1].trim_start_matches("0x"), 16) {
                         Ok(v) => v,
                         Err(_) => return Ok("Error: Invalid DOUT hex value".to_string()),
                     };
                     let oen = match u32::from_str_radix(rest[2].trim_start_matches("0x"), 16) {
                         Ok(v) => v,
                         Err(_) => return Ok("Error: Invalid OEN hex value".to_string()),
                     };
                     self.set_fast_pins(mac, dout, oen).await?;
                     Ok(format!("Set fast pins for {}", mac))
                 } else {
                     Ok("Usage: fastpins [mac] [set <dout_hex> <oen_hex>]".to_string())
                 }
            }

            _ => Ok(format!("Unknown command: {}. Type 'help' for commands.", parts[0])),
        }
    }

    // =========================================================================
    // Firmware Update (FBC protocol)
    // =========================================================================

    /// Get firmware info from a board
    pub async fn get_firmware_info(&self, mac: &str) -> fbc::Result<crate::commands::FbcFirmwareInfo> {
        let mac_bytes = fbc::parse_mac(mac).ok_or_else(|| fbc::FbcError::InvalidMac(mac.to_string()))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or(fbc::FbcError::NotConnected)?;

        socket.send(mac_bytes, fw_cmd::INFO_REQ, &[]).await?;

        let timeout = Duration::from_millis(500);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some((src_mac, cmd, payload)) =
                socket.recv_timeout(Duration::from_millis(50)).await?
            {
                if src_mac == mac_bytes && cmd == fw_cmd::INFO_RSP && payload.len() >= 20 {
                    // Parse firmware info response
                    let version = format!("{}.{}.{}", payload[0], payload[1], payload[2]);
                    let build_date = String::from_utf8_lossy(&payload[3..13]).to_string();
                    let board_serial = u32::from_be_bytes([payload[13], payload[14], payload[15], payload[16]]);
                    let hw_revision = payload[17];
                    let flags = payload[19];
                    let sd_present = (flags & 0x02) != 0;
                    let update_in_progress = (flags & 0x01) != 0;

                    return Ok(crate::commands::FbcFirmwareInfo {
                        version,
                        build_date,
                        board_serial,
                        hw_revision,
                        sd_present,
                        update_in_progress,
                    });
                }
            }
        }

        Err(fbc::FbcError::Timeout)
    }

    /// Update firmware via FBC protocol
    pub async fn update_firmware_fbc(
        &self,
        mac: &str,
        firmware_data: &[u8],
        app_handle: tauri::AppHandle,
    ) -> Result<String, String> {
        use tauri::Emitter;

        let mac_bytes = fbc::parse_mac(mac)
            .ok_or_else(|| format!("Invalid MAC: {}", mac))?;

        let socket_guard = self.socket.read().await;
        let socket = socket_guard.as_ref().ok_or("Not connected")?;

        // Calculate simple checksum (XOR of all bytes)
        let checksum: u32 = firmware_data.iter().fold(0u32, |acc, &b| acc ^ (b as u32));
        let total_size = firmware_data.len() as u32;

        // Step 1: Send BEGIN
        let mut begin_payload = [0u8; 8];
        begin_payload[0..4].copy_from_slice(&total_size.to_be_bytes());
        begin_payload[4..8].copy_from_slice(&checksum.to_be_bytes());

        socket.send(mac_bytes, fw_cmd::BEGIN, &begin_payload).await
            .map_err(|e| format!("Failed to send BEGIN: {}", e))?;

        // Wait for BEGIN_ACK
        let timeout = Duration::from_secs(5);
        let start = std::time::Instant::now();
        let mut max_chunk_size = 1024u16;

        loop {
            if start.elapsed() > timeout {
                return Err("Timeout waiting for BEGIN_ACK".to_string());
            }
            if let Some((src_mac, cmd, payload)) = socket
                .recv_timeout(Duration::from_millis(100)).await
                .map_err(|e| e.to_string())?
            {
                if src_mac == mac_bytes && cmd == fw_cmd::BEGIN_ACK && payload.len() >= 3 {
                    let status = payload[0];
                    if status != 0 {
                        return Err(match status {
                            1 => "No SD card present".to_string(),
                            2 => "SD write error".to_string(),
                            3 => "Update already in progress".to_string(),
                            _ => format!("BEGIN failed with status {}", status),
                        });
                    }
                    max_chunk_size = u16::from_be_bytes([payload[1], payload[2]]);
                    break;
                }
            }
        }

        let _ = app_handle.emit("firmware:progress", serde_json::json!({
            "stage": "uploading",
            "percent": 0,
            "message": "Starting upload..."
        }));

        // Step 2: Send chunks
        let chunk_size = (max_chunk_size as usize).min(1024);
        let mut offset = 0u32;

        while (offset as usize) < firmware_data.len() {
            let end = ((offset as usize) + chunk_size).min(firmware_data.len());
            let chunk = &firmware_data[offset as usize..end];
            let chunk_len = chunk.len() as u16;

            // Build chunk payload: offset(4) + size(2) + data
            let mut payload = Vec::with_capacity(6 + chunk.len());
            payload.extend_from_slice(&offset.to_be_bytes());
            payload.extend_from_slice(&chunk_len.to_be_bytes());
            payload.extend_from_slice(chunk);

            socket.send(mac_bytes, fw_cmd::CHUNK, &payload).await
                .map_err(|e| format!("Failed to send chunk at offset {}: {}", offset, e))?;

            // Wait for CHUNK_ACK
            let chunk_timeout = Duration::from_secs(2);
            let chunk_start = std::time::Instant::now();

            loop {
                if chunk_start.elapsed() > chunk_timeout {
                    return Err(format!("Timeout waiting for CHUNK_ACK at offset {}", offset));
                }
                if let Some((src_mac, cmd, ack_payload)) = socket
                    .recv_timeout(Duration::from_millis(100)).await
                    .map_err(|e| e.to_string())?
                {
                    if src_mac == mac_bytes && cmd == fw_cmd::CHUNK_ACK && ack_payload.len() >= 5 {
                        let ack_offset = u32::from_be_bytes([ack_payload[0], ack_payload[1], ack_payload[2], ack_payload[3]]);
                        let status = ack_payload[4];

                        if status != 0 {
                            return Err(format!("Chunk write failed at offset {}: status {}", ack_offset, status));
                        }
                        break;
                    }
                }
            }

            offset += chunk.len() as u32;

            // Emit progress
            let percent = ((offset as f64 / total_size as f64) * 100.0) as u8;
            let _ = app_handle.emit("firmware:progress", serde_json::json!({
                "stage": "uploading",
                "percent": percent,
                "message": format!("Uploaded {} / {} bytes", offset, total_size)
            }));
        }

        // Step 3: Send COMMIT
        let _ = app_handle.emit("firmware:progress", serde_json::json!({
            "stage": "committing",
            "percent": 100,
            "message": "Finalizing update..."
        }));

        socket.send(mac_bytes, fw_cmd::COMMIT, &[]).await
            .map_err(|e| format!("Failed to send COMMIT: {}", e))?;

        // Wait for COMMIT_ACK
        let commit_timeout = Duration::from_secs(10);
        let commit_start = std::time::Instant::now();

        loop {
            if commit_start.elapsed() > commit_timeout {
                return Err("Timeout waiting for COMMIT_ACK".to_string());
            }
            if let Some((src_mac, cmd, payload)) = socket
                .recv_timeout(Duration::from_millis(100)).await
                .map_err(|e| e.to_string())?
            {
                if src_mac == mac_bytes && cmd == fw_cmd::COMMIT_ACK && payload.len() >= 9 {
                    let status = payload[0];
                    let received = u32::from_be_bytes([payload[1], payload[2], payload[3], payload[4]]);
                    let computed_checksum = u32::from_be_bytes([payload[5], payload[6], payload[7], payload[8]]);

                    if status != 0 {
                        return Err(match status {
                            1 => format!("Checksum mismatch: expected {:08X}, got {:08X}", checksum, computed_checksum),
                            2 => format!("Incomplete: received {} of {} bytes", received, total_size),
                            _ => format!("COMMIT failed with status {}", status),
                        });
                    }

                    let _ = app_handle.emit("firmware:progress", serde_json::json!({
                        "stage": "rebooting",
                        "percent": 100,
                        "message": "Update complete! Board is rebooting..."
                    }));

                    return Ok(format!(
                        "Firmware updated successfully! {} bytes written. Board is rebooting.",
                        received
                    ));
                }
            }
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

fn parse_announce(mac: &[u8; 6], payload: &[u8]) -> BoardInfo {
    BoardInfo {
        mac: fbc::format_mac(mac),
        serial: u32::from_be_bytes([payload[7], payload[8], payload[9], payload[10]]),
        hw_revision: payload[11],
        fw_version: format!("{}.{}", payload[12], payload[13]),
        has_bim: payload[14] != 0,
        bim_type: payload[6],
        state: ControllerState::Idle,
        slot: None,
    }
}

fn parse_status(payload: &[u8]) -> BoardStatus {
    let temp_raw = i16::from_be_bytes([payload[8], payload[9]]);

    // Firmware StatusPayload (47 bytes):
    //   [0..4]   cycles(u32)
    //   [4..8]   errors(u32)
    //   [8..10]  temp_c(i16)
    //   [10]     state(u8)
    //   [11..27] rail_voltage[8](u16) — 16 bytes
    //   [27..43] rail_current[8](u16) — 16 bytes
    //   [43..45] fpga_vccint(u16)
    //   [45..47] fpga_vccaux(u16)
    let mut rail_voltage_mv = [0u16; 8];
    let mut rail_current_ma = [0u16; 8];

    if payload.len() >= 47 {
        for i in 0..8 {
            rail_voltage_mv[i] = u16::from_be_bytes([payload[11 + i * 2], payload[12 + i * 2]]);
        }
        for i in 0..8 {
            rail_current_ma[i] = u16::from_be_bytes([payload[27 + i * 2], payload[28 + i * 2]]);
        }
    }

    BoardStatus {
        cycles: u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]),
        errors: u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]),
        temp_c: (temp_raw as f32) / 10.0,
        state: ControllerState::from(payload[10]),
        rail_voltage_mv,
        rail_current_ma,
        fpga_vccint_mv: if payload.len() >= 45 {
            u16::from_be_bytes([payload[43], payload[44]])
        } else {
            0
        },
        fpga_vccaux_mv: if payload.len() >= 47 {
            u16::from_be_bytes([payload[45], payload[46]])
        } else {
            0
        },
    }
}

fn parse_fast_pins(payload: &[u8]) -> FastPinState {
    // Firmware sends: din(4) + dout(4) + oen(4) — match that order
    FastPinState {
        din: u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]),
        dout: u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]),
        oen: u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]),
    }
}

/// Channel names for XADC
const XADC_NAMES: [&str; 16] = [
    "TEMP", "VCCINT", "VCCAUX", "VP/VN", "VREFP", "VREFN", "VCCBRAM", "VCCPINT",
    "VCCPAUX", "VCCO_DDR", "AUX0", "AUX1", "AUX2", "AUX3", "AUX4", "AUX5",
];

/// Channel names for external ADC (MAX11131)
const EXT_ADC_NAMES: [&str; 16] = [
    "VDD_CORE", "VDD_IO", "VDD_AUX", "VDD_PLL", "VDD_SRAM", "VDD_FLASH", "VREF", "GND_SENSE",
    "DUT_I0", "DUT_I1", "DUT_I2", "DUT_I3", "DUT_V0", "DUT_V1", "DUT_V2", "DUT_V3",
];

fn parse_analog_channels(payload: &[u8]) -> AnalogChannels {
    let mut channels = AnalogChannels::default();

    // Parse XADC channels (32 bytes: 16 x 2-byte values)
    for i in 0..16 {
        let raw = BigEndian::read_u16(&payload[i * 2..i * 2 + 2]);
        channels.xadc[i] = AnalogReading {
            raw,
            voltage_mv: xadc_to_voltage(i, raw),
            name: XADC_NAMES[i].to_string(),
        };
    }

    // Parse external ADC channels (32 bytes: 16 x 2-byte values)
    for i in 0..16 {
        let raw = BigEndian::read_u16(&payload[32 + i * 2..32 + i * 2 + 2]);
        channels.external[i] = AnalogReading {
            raw,
            voltage_mv: ext_adc_to_voltage(raw),
            name: EXT_ADC_NAMES[i].to_string(),
        };
    }

    channels
}

/// Convert XADC raw value to voltage
fn xadc_to_voltage(channel: usize, raw: u16) -> f32 {
    let raw_f = raw as f32 / 65536.0;
    match channel {
        0 => raw_f * 503.975 - 273.15, // Temperature: °C = (ADC × 503.975) / 65536 - 273.15
        1 | 2 | 6..=9 => raw_f * 3000.0, // Power rails: 0-3V range
        _ => raw_f * 1000.0, // Aux inputs: 0-1V range
    }
}

/// Convert external ADC raw value to voltage
fn ext_adc_to_voltage(raw: u16) -> f32 {
    // MAX11131: 12-bit, 0-2.5V reference
    (raw as f32 / 4096.0) * 2500.0
}

fn parse_vicor_status(payload: &[u8]) -> VicorStatus {
    let mut cores = [VicorCore::default(); 6];

    // Firmware sends 5 bytes per core: enabled(1) + voltage_mv(2) + current_ma(2)
    for i in 0..6 {
        let offset = i * 5;
        let enabled = payload[offset] != 0;
        cores[i] = VicorCore {
            id: i as u8,
            enabled,
            voltage_mv: BigEndian::read_u16(&payload[offset + 1..offset + 3]),
            current_ma: BigEndian::read_u16(&payload[offset + 3..offset + 5]),
            temp_c: 0.0, // Not sent in this payload
            status: if enabled { VicorCoreStatus::On } else { VicorCoreStatus::Off },
        };
    }

    VicorStatus { cores }
}

fn parse_pmbus_status(payload: &[u8]) -> PmBusStatus {
    let rail_count = payload[0] as usize;
    let mut rails = Vec::with_capacity(rail_count);

    // Each rail: 16 bytes
    for i in 0..rail_count {
        let offset = 1 + i * 16;
        if offset + 16 > payload.len() {
            break;
        }

        let name_bytes = &payload[offset + 1..offset + 9];
        let name = String::from_utf8_lossy(name_bytes).trim_end_matches('\0').to_string();

        rails.push(PmBusRail {
            address: payload[offset],
            name,
            enabled: payload[offset + 9] != 0,
            voltage_mv: BigEndian::read_u16(&payload[offset + 10..offset + 12]),
            current_ma: BigEndian::read_u16(&payload[offset + 12..offset + 14]),
            power_mw: 0, // Calculate from V*I if needed
            temp_c: 0.0,
            status_word: BigEndian::read_u16(&payload[offset + 14..offset + 16]),
        });
    }

    PmBusStatus { rails }
}

fn parse_eeprom(payload: &[u8]) -> EepromData {
    let raw = payload[0..256].to_vec();

    // Parse header (bytes 0-15)
    let header = EepromHeader {
        magic: BigEndian::read_u16(&payload[0..2]),
        version: payload[2],
        board_serial: BigEndian::read_u32(&payload[3..7]),
        hw_revision: payload[7],
        mfg_date: BigEndian::read_u32(&payload[8..12]),
        config_crc: BigEndian::read_u16(&payload[12..14]),
    };

    // Parse rail configs (bytes 16-111: 6 x 16 bytes)
    let mut rails = Vec::with_capacity(6);
    for i in 0..6 {
        let offset = 16 + i * 16;
        let mut name = [0u8; 8];
        name.copy_from_slice(&payload[offset + 1..offset + 9]);
        rails.push(RailConfig {
            rail_id: payload[offset],
            name,
            nominal_mv: BigEndian::read_u16(&payload[offset + 9..offset + 11]),
            max_mv: BigEndian::read_u16(&payload[offset + 11..offset + 13]),
            max_ma: BigEndian::read_u16(&payload[offset + 13..offset + 15]),
            enabled_by_default: payload[offset + 15] != 0,
        });
    }

    // Parse DUT metadata (bytes 112-159)
    let part_bytes = &payload[112..128];
    let lot_bytes = &payload[128..144];
    let dut = DutMetadata {
        part_number: String::from_utf8_lossy(part_bytes).trim_end_matches('\0').to_string(),
        lot_id: String::from_utf8_lossy(lot_bytes).trim_end_matches('\0').to_string(),
        wafer_id: payload[144],
        die_x: payload[145],
        die_y: payload[146],
        test_count: BigEndian::read_u32(&payload[147..151]),
        last_test_time: BigEndian::read_u64(&payload[151..159]),
    };

    // Parse calibration data (bytes 160-255)
    let mut calibration = CalibrationData::default();
    for i in 0..16 {
        calibration.adc_offset[i] = BigEndian::read_i16(&payload[160 + i * 2..162 + i * 2]);
        calibration.adc_gain[i] = BigEndian::read_u16(&payload[192 + i * 2..194 + i * 2]);
    }
    for i in 0..10 {
        calibration.dac_offset[i] = BigEndian::read_i16(&payload[224 + i * 2..226 + i * 2]);
        calibration.dac_gain[i] = BigEndian::read_u16(&payload[244 + i * 2..246 + i * 2]);
    }

    EepromData {
        raw,
        header,
        rails,
        dut,
        calibration,
    }
}

fn parse_vector_status(payload: &[u8]) -> VectorEngineStatus {
    VectorEngineStatus {
        state: VectorState::from(payload[0]),
        current_address: BigEndian::read_u32(&payload[1..5]),
        total_vectors: BigEndian::read_u32(&payload[5..9]),
        loop_count: BigEndian::read_u32(&payload[9..13]),
        target_loops: BigEndian::read_u32(&payload[13..17]),
        error_count: BigEndian::read_u32(&payload[17..21]),
        first_fail_addr: BigEndian::read_u32(&payload[21..25]),
        run_time_ms: BigEndian::read_u64(&payload[25..33]),
    }
}

fn parse_error_log_response(payload: &[u8]) -> fbc::ErrorLogResponse {
    if payload.len() < 8 {
        return fbc::ErrorLogResponse {
            total_errors: 0,
            num_entries: 0,
            entries: vec![],
        };
    }

    let total_errors = BigEndian::read_u32(&payload[0..4]);
    let num_entries = BigEndian::read_u32(&payload[4..8]);

    let mut entries = Vec::new();
    let max_entries = ((payload.len() - 8) / 28).min(num_entries as usize);

    for i in 0..max_entries {
        let offset = 8 + i * 28;
        if offset + 28 > payload.len() {
            break;
        }
        let pattern = [
            BigEndian::read_u32(&payload[offset..offset+4]),
            BigEndian::read_u32(&payload[offset+4..offset+8]),
            BigEndian::read_u32(&payload[offset+8..offset+12]),
            BigEndian::read_u32(&payload[offset+12..offset+16]),
        ];
        let vector = BigEndian::read_u32(&payload[offset+16..offset+20]);
        let cycle_lo = BigEndian::read_u32(&payload[offset+20..offset+24]);
        let cycle_hi = BigEndian::read_u32(&payload[offset+24..offset+28]);

        entries.push(fbc::ErrorLogEntry {
            pattern,
            vector,
            cycle_lo,
            cycle_hi,
        });
    }

    fbc::ErrorLogResponse {
        total_errors,
        num_entries,
        entries,
    }
}

const HELP_TEXT: &str = r#"FBC System Terminal Commands:

  help              - Show this help
  list, ls          - List discovered boards
  discover          - Discover boards on network
  status <mac>      - Get board status
  start <mac>       - Start test on board
  stop <mac>        - Stop test on board
  start <mac>       - Start test on board
  stop <mac>        - Stop test on board
  reset <mac>       - Reset board
  fastpins <mac>    - Get/Set fast pins

Examples:
  discover
  status 00:0A:35:00:01:02
  start 00:0A:35:00:01:02
"#;
