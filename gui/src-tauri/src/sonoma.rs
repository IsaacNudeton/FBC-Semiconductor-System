//! Sonoma SSH Control — programmatic board control via SSH + ELF binaries
//!
//! Standalone module (not an fbc-host dependency) that replicates
//! SonomaClient's SSH command patterns for the GUI backend.
//!
//! Sonoma boards run Linux on Zynq 7020. Control is via SSH:
//! - Connect as root (password or none auth)
//! - Execute ELF binaries in /mnt/bin/
//! - Parse stdout for readings, exit code for success/failure
//! - Hardware lock via `flock -x /tmp/LockBit` for concurrent access

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use russh::client;
use russh::ChannelMsg;
use russh::Disconnect;
use russh_keys::key;
use serde::Serialize;
use tokio::sync::RwLock;

// =============================================================================
// Error Type
// =============================================================================

#[derive(Debug, thiserror::Error)]
pub enum SonomaError {
    #[error("SSH connection failed: {0}")]
    Connection(String),

    #[error("SSH auth failed: {0}")]
    Auth(String),

    #[error("Command failed (exit {code}): {stderr}")]
    Command { code: u32, stderr: String },

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Timeout")]
    Timeout,
}

impl From<russh::Error> for SonomaError {
    fn from(e: russh::Error) -> Self {
        SonomaError::Connection(e.to_string())
    }
}

// =============================================================================
// SSH Handler (required by russh 0.42)
// =============================================================================

struct SonomaHandler;

#[async_trait]
impl client::Handler for SonomaHandler {
    type Error = russh::Error;

    async fn check_server_key(
        self,
        _server_public_key: &key::PublicKey,
    ) -> Result<(Self, bool), Self::Error> {
        // Accept all host keys (embedded Linux boards on trusted lab network)
        Ok((self, true))
    }
}

// =============================================================================
// Command Result + Shared Types
// =============================================================================

#[derive(Debug, Clone)]
struct ExecResult {
    stdout: String,
    stderr: String,
    exit_code: u32,
}

/// Single ADC channel reading
#[derive(Debug, Clone, Serialize)]
pub struct SonomaAnalogReading {
    pub channel: u8,
    pub raw: u16,
    pub voltage_mv: f32,
}

/// Vector run result
#[derive(Debug, Clone, Serialize)]
pub struct SonomaRunResult {
    pub passed: bool,
    pub vectors_executed: u32,
    pub errors: u32,
    pub duration_s: f32,
}

/// Composite board status
#[derive(Debug, Clone, Serialize)]
pub struct SonomaStatus {
    pub alive: bool,
    pub ip: String,
    pub hostname: String,
    pub fw_version: String,
    pub xadc: Vec<SonomaAnalogReading>,
    pub adc32: Vec<SonomaAnalogReading>,
}

/// Board info for scan results
#[derive(Debug, Clone, Serialize)]
pub struct SonomaBoardInfo {
    pub ip: String,
    pub alive: bool,
    pub hostname: String,
}

// =============================================================================
// VICOR Channel Mapping
// =============================================================================

/// VICOR core → (DAC channel, MIO pin) mapping
/// Verified from Sonoma RunVectors AWK lines 630-636
const VICOR_MAP: [(u8, u8); 6] = [
    (9, 0),   // Core 1: DAC=9, MIO=0
    (3, 39),  // Core 2: DAC=3, MIO=39
    (7, 47),  // Core 3: DAC=7, MIO=47
    (8, 8),   // Core 4: DAC=8, MIO=8
    (4, 38),  // Core 5: DAC=4, MIO=38
    (2, 37),  // Core 6: DAC=2, MIO=37
];

fn vicor_map(core: u8) -> Result<(u8, u8), SonomaError> {
    if core < 1 || core > 6 {
        return Err(SonomaError::Parse(format!(
            "Invalid VICOR core {}, must be 1-6",
            core
        )));
    }
    let (dac, mio) = VICOR_MAP[(core - 1) as usize];
    Ok((dac, mio))
}

// =============================================================================
// Output Parsers (inlined from sonoma_parse.rs)
// =============================================================================

/// Parse ADC CSV output (3 lines: Max, Avg, Min — we use Avg)
fn parse_adc_csv(stdout: &str, channel_offset: u8) -> Result<Vec<SonomaAnalogReading>, String> {
    let lines: Vec<&str> = stdout.trim().lines().collect();
    if lines.len() < 2 {
        return Ok(Vec::new());
    }

    let avg_line = lines.get(1).unwrap_or(&"");
    let values: Vec<&str> = avg_line.split(',').collect();

    let mut readings = Vec::new();
    for (i, val_str) in values.iter().enumerate() {
        let val_str = val_str.trim();
        if val_str.is_empty() {
            continue;
        }
        let raw: f32 = val_str
            .parse()
            .map_err(|e| format!("Failed to parse ADC value '{}': {}", val_str, e))?;

        readings.push(SonomaAnalogReading {
            channel: channel_offset + i as u8,
            raw: raw as u16,
            voltage_mv: raw,
        });
    }

    Ok(readings)
}

/// Parse vector run result from RunSuperVector.elf output
fn parse_run_result(stdout: &str, time_s: u32) -> Result<SonomaRunResult, String> {
    let mut errors = 0u32;
    let mut passed = true;

    for line in stdout.lines() {
        if line.contains("VECTOR FAILED") {
            passed = false;
            if let Some(count_str) = line.split("error_count=").nth(1) {
                if let Ok(n) = count_str.trim().parse::<u32>() {
                    errors = n;
                }
            }
        }
    }

    Ok(SonomaRunResult {
        passed,
        vectors_executed: 0,
        errors,
        duration_s: time_s as f32,
    })
}

/// Parse hex value from rwmem output
fn parse_hex_value(stdout: &str) -> Result<u32, String> {
    let s = stdout.trim();
    if s.is_empty() {
        return Err("Empty rwmem output".into());
    }

    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u32::from_str_radix(hex.trim(), 16)
            .map_err(|e| format!("Failed to parse hex '{}': {}", s, e));
    }

    if s.len() == 8 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        return u32::from_str_radix(s, 16)
            .map_err(|e| format!("Failed to parse hex '{}': {}", s, e));
    }

    s.parse::<u32>()
        .map_err(|e| format!("Failed to parse value '{}': {}", s, e))
}

// =============================================================================
// SonomaClient — stateless SSH command executor
// =============================================================================

/// SSH client for controlling Sonoma legacy boards.
/// Each method connects, runs a command, captures output, disconnects.
struct SonomaClient {
    host: String,
    user: String,
    password: String,
}

impl SonomaClient {
    fn new(host: &str, user: &str, password: &str) -> Self {
        SonomaClient {
            host: host.to_string(),
            user: user.to_string(),
            password: password.to_string(),
        }
    }

    /// Internal: connect, authenticate, return session handle
    async fn connect(&self) -> Result<client::Handle<SonomaHandler>, SonomaError> {
        let config = Arc::new(client::Config::default());
        let handler = SonomaHandler;

        let mut session =
            client::connect(config, (self.host.as_str(), 22u16), handler).await?;

        let auth_ok = if self.password.is_empty() {
            session
                .authenticate_none(&self.user)
                .await
                .map_err(|e| SonomaError::Auth(e.to_string()))?
        } else {
            session
                .authenticate_password(&self.user, &self.password)
                .await
                .map_err(|e| SonomaError::Auth(e.to_string()))?
        };

        if !auth_ok {
            return Err(SonomaError::Auth("Authentication rejected".into()));
        }

        Ok(session)
    }

    /// Internal: run a command on an existing session
    async fn run_cmd(
        session: &client::Handle<SonomaHandler>,
        cmd: &str,
    ) -> Result<ExecResult, SonomaError> {
        let mut channel = session.channel_open_session().await?;
        channel.exec(true, cmd).await?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code: u32 = 0;

        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => {
                    stdout.extend_from_slice(&data);
                }
                ChannelMsg::ExtendedData { data, ext } => {
                    if ext == 1 {
                        stderr.extend_from_slice(&data);
                    }
                }
                ChannelMsg::ExitStatus { exit_status } => {
                    exit_code = exit_status;
                }
                _ => {}
            }
        }

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&stdout).to_string(),
            stderr: String::from_utf8_lossy(&stderr).to_string(),
            exit_code,
        })
    }

    /// Execute a command with hardware lock (flock)
    async fn exec(&self, cmd: &str) -> Result<ExecResult, SonomaError> {
        let session = self.connect().await?;
        let locked_cmd = format!("flock -x /tmp/LockBit {}", cmd);
        let result = Self::run_cmd(&session, &locked_cmd).await?;

        session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await?;

        if result.exit_code != 0 {
            return Err(SonomaError::Command {
                code: result.exit_code,
                stderr: result.stderr.clone(),
            });
        }

        Ok(result)
    }

    /// Execute without hardware lock (for read-only ops)
    async fn exec_unlocked(&self, cmd: &str) -> Result<ExecResult, SonomaError> {
        let session = self.connect().await?;
        let result = Self::run_cmd(&session, cmd).await?;

        session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await?;

        Ok(result)
    }

    // =========================================================================
    // Status / Discovery
    // =========================================================================

    async fn is_alive(&self) -> bool {
        self.exec_unlocked("echo ok").await.is_ok()
    }

    async fn hostname(&self) -> Result<String, SonomaError> {
        let r = self.exec_unlocked("cat /etc/hostname").await?;
        Ok(r.stdout.trim().to_string())
    }

    async fn fw_version(&self) -> Result<String, SonomaError> {
        let r = self.exec_unlocked(
            "cat /mnt/version.txt 2>/dev/null || stat -c '%y' /boot/BOOT.BIN 2>/dev/null || echo unknown"
        ).await?;
        Ok(r.stdout.trim().to_string())
    }

    async fn get_status(&self) -> Result<SonomaStatus, SonomaError> {
        let alive = self.is_alive().await;
        if !alive {
            return Ok(SonomaStatus {
                alive: false,
                ip: self.host.clone(),
                hostname: String::new(),
                fw_version: String::new(),
                xadc: Vec::new(),
                adc32: Vec::new(),
            });
        }

        let hostname = self.hostname().await.unwrap_or_default();
        let fw = self.fw_version().await.unwrap_or_else(|_| "unknown".into());
        let xadc = self.read_xadc().await.unwrap_or_default();
        let adc32 = self.read_adc32().await.unwrap_or_default();

        Ok(SonomaStatus {
            alive: true,
            ip: self.host.clone(),
            hostname,
            fw_version: fw,
            xadc,
            adc32,
        })
    }

    // =========================================================================
    // Power — VICOR Core Supplies
    // =========================================================================

    async fn vicor_init(&self, core: u8, voltage: f32) -> Result<(), SonomaError> {
        let (dac, mio) = vicor_map(core)?;
        self.exec(&format!(
            "/mnt/bin/linux_VICOR.elf {} {} {}",
            voltage, mio, dac
        ))
        .await?;
        Ok(())
    }

    async fn vicor_voltage(&self, core: u8, voltage: f32) -> Result<(), SonomaError> {
        let (dac, _mio) = vicor_map(core)?;
        self.exec(&format!(
            "/mnt/bin/linux_VICOR_Voltage.elf {} {}",
            voltage * 2.0,
            dac
        ))
        .await?;
        Ok(())
    }

    async fn vicor_disable(&self, core: u8) -> Result<(), SonomaError> {
        let (dac, mio) = vicor_map(core)?;
        self.exec(&format!("/mnt/bin/linux_VICOR.elf 0.0 {} {}", mio, dac))
            .await?;
        Ok(())
    }

    // =========================================================================
    // Power — PMBus
    // =========================================================================

    async fn pmbus_set(&self, channel: u8, voltage: f32) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_pmbus_PicoDlynx.elf {} {}",
            channel, voltage
        ))
        .await?;
        Ok(())
    }

    async fn pmbus_off(&self, channel: u8) -> Result<(), SonomaError> {
        self.exec(&format!("/mnt/bin/linux_pmbus_OFF.elf {}", channel))
            .await?;
        Ok(())
    }

    async fn io_ps(&self, b13: f32, b33: f32, b34: f32, b35: f32) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_IO_PS.elf {} {} {} {}",
            b13, b33, b34, b35
        ))
        .await?;
        Ok(())
    }

    async fn emergency_stop(&self) -> Result<(), SonomaError> {
        // Disable all PMBus channels
        self.exec(
            "for i in $(seq 1 99); do /mnt/bin/linux_pmbus_OFF.elf $i 0 2>/dev/null; done",
        )
        .await
        .ok();
        // Disable all VICOR cores
        for core in 1..=6 {
            self.vicor_disable(core).await.ok();
        }
        // Zero IO power supplies
        self.io_ps(0.0, 0.0, 0.0, 0.0).await.ok();
        Ok(())
    }

    // =========================================================================
    // Analog Reading
    // =========================================================================

    async fn read_xadc(&self) -> Result<Vec<SonomaAnalogReading>, SonomaError> {
        let r = self.exec("/mnt/bin/XADC32Ch.elf").await?;
        parse_adc_csv(&r.stdout, 0).map_err(SonomaError::Parse)
    }

    async fn read_adc32(&self) -> Result<Vec<SonomaAnalogReading>, SonomaError> {
        let r = self.exec("/mnt/bin/ADC32ChPlusStats.elf").await?;
        parse_adc_csv(&r.stdout, 0).map_err(SonomaError::Parse)
    }

    async fn read_adc32_high(&self) -> Result<Vec<SonomaAnalogReading>, SonomaError> {
        self.exec("/mnt/bin/ToggleMio.elf 36 1").await?;
        let r = self.exec("/mnt/bin/ADC32ChPlusStats.elf").await?;
        self.exec("/mnt/bin/ToggleMio.elf 36 0").await?;
        parse_adc_csv(&r.stdout, 16).map_err(SonomaError::Parse)
    }

    // =========================================================================
    // Vector Engine
    // =========================================================================

    async fn load_vectors(&self, seq: &str, hex: &str) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_load_vectors.elf {} {}",
            seq, hex
        ))
        .await?;
        Ok(())
    }

    async fn run_vectors(
        &self,
        seq: &str,
        time_s: u32,
        debug: bool,
    ) -> Result<SonomaRunResult, SonomaError> {
        let debug_flag = if debug { 1 } else { 0 };
        let r = self
            .exec(&format!(
                "/mnt/bin/RunSuperVector.elf {} {} {}",
                seq, time_s, debug_flag
            ))
            .await;

        match r {
            Ok(result) => parse_run_result(&result.stdout, time_s).map_err(SonomaError::Parse),
            Err(SonomaError::Command { code, .. }) => Ok(SonomaRunResult {
                passed: false,
                vectors_executed: 0,
                errors: code,
                duration_s: time_s as f32,
            }),
            Err(e) => Err(e),
        }
    }

    async fn set_frequency(
        &self,
        pll: u8,
        freq_hz: u32,
        duty_cycle: u8,
    ) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_xpll_frequency.elf {} {} 0 {}",
            pll, freq_hz, duty_cycle
        ))
        .await?;
        Ok(())
    }

    // =========================================================================
    // Pin Configuration
    // =========================================================================

    async fn set_pin_type(&self, pin: u8, pin_type: u8) -> Result<(), SonomaError> {
        self.exec(&format!("/mnt/bin/linux_pin_type.elf {} {}", pin, pin_type))
            .await?;
        Ok(())
    }

    async fn set_pulse_delays(
        &self,
        pin: u8,
        ptype: u8,
        rise: u32,
        fall: u32,
        period: u32,
    ) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_Pulse_Delays.elf {} {} {} {} {}",
            pin, ptype, rise, fall, period
        ))
        .await?;
        Ok(())
    }

    async fn pll_on_off(&self, states: [bool; 4]) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_xpll_off_on.elf {} {} {} {}",
            states[0] as u8, states[1] as u8, states[2] as u8, states[3] as u8
        ))
        .await?;
        Ok(())
    }

    // =========================================================================
    // DAC / GPIO / Memory
    // =========================================================================

    async fn set_ext_dac(&self, channels: &[f32; 10]) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_EXT_DAC.elf {} {} {} {} {} {} {} {} {} {}",
            channels[0],
            channels[1],
            channels[2],
            channels[3],
            channels[4],
            channels[5],
            channels[6],
            channels[7],
            channels[8],
            channels[9]
        ))
        .await?;
        Ok(())
    }

    async fn toggle_mio(&self, pin: u8, value: u8) -> Result<(), SonomaError> {
        self.exec(&format!("/mnt/bin/ToggleMio.elf {} {}", pin, value))
            .await?;
        Ok(())
    }

    async fn read_mem(&self, addr: u32) -> Result<u32, SonomaError> {
        let r = self
            .exec(&format!("/mnt/rwmem.elf 0x{:08X}", addr))
            .await?;
        parse_hex_value(&r.stdout).map_err(SonomaError::Parse)
    }

    async fn write_mem(&self, addr: u32, value: u32) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/rwmem.elf 0x{:08X} 0x{:08X}",
            addr, value
        ))
        .await?;
        Ok(())
    }

    // =========================================================================
    // Temperature
    // =========================================================================

    async fn set_temperature(
        &self,
        setpoint: f32,
        r25c: f32,
        cool_after: bool,
    ) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_set_temperature.elf {} {} {}",
            setpoint, r25c, cool_after as u8
        ))
        .await?;
        Ok(())
    }

    // =========================================================================
    // Init Sequence
    // =========================================================================

    async fn init(&self) -> Result<(), SonomaError> {
        self.exec("/mnt/bin/linux_cpu1_wakeup.elf").await.ok();
        self.set_ext_dac(&[0.0; 10]).await?;
        self.exec("/mnt/bin/linux_init_XADC.elf").await?;
        self.exec(
            "for i in $(seq 1 99); do /mnt/bin/linux_pmbus_OFF.elf $i 0 2>/dev/null; done",
        )
        .await
        .ok();
        Ok(())
    }
}

// =============================================================================
// Connection Manager — stores credentials per IP, creates clients on demand
// =============================================================================

/// Stored credentials for a Sonoma board
#[derive(Clone)]
struct SonomaCredentials {
    user: String,
    password: String,
}

/// Manages Sonoma board connections (credential storage + client factory)
pub struct SonomaConnectionManager {
    credentials: RwLock<HashMap<String, SonomaCredentials>>,
}

impl SonomaConnectionManager {
    pub fn new() -> Self {
        Self {
            credentials: RwLock::new(HashMap::new()),
        }
    }

    /// Store credentials for an IP
    pub async fn add(&self, ip: &str, user: &str, password: &str) {
        self.credentials.write().await.insert(
            ip.to_string(),
            SonomaCredentials {
                user: user.to_string(),
                password: password.to_string(),
            },
        );
    }

    /// Remove stored credentials
    pub async fn remove(&self, ip: &str) {
        self.credentials.write().await.remove(ip);
    }

    /// Get a client for the given IP (or error if not registered)
    async fn client(&self, ip: &str) -> Result<SonomaClient, String> {
        let creds = self.credentials.read().await;
        let cred = creds
            .get(ip)
            .ok_or_else(|| format!("No credentials stored for {}", ip))?;
        Ok(SonomaClient::new(ip, &cred.user, &cred.password))
    }

    /// List all registered IPs
    pub async fn list_ips(&self) -> Vec<String> {
        self.credentials.read().await.keys().cloned().collect()
    }
}

/// Expand IP range string to list of full IPs
/// "101-104" → ["172.16.0.101", ..., "172.16.0.104"]
fn expand_ip_range(start: u8, end: u8) -> Result<Vec<String>, String> {
    if start > end {
        return Err(format!("Start {} > end {} in range", start, end));
    }
    Ok((start..=end)
        .map(|i| format!("172.16.0.{}", i))
        .collect())
}

// =============================================================================
// Tauri Commands (27 total)
// =============================================================================

/// Connect to a Sonoma board (store credentials, verify alive)
#[tauri::command]
pub async fn sonoma_connect(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    user: String,
    password: String,
) -> Result<bool, String> {
    let mgr = state.sonoma();
    mgr.add(&ip, &user, &password).await;
    let client = mgr.client(&ip).await?;
    let alive = client.is_alive().await;
    if !alive {
        mgr.remove(&ip).await;
    }
    Ok(alive)
}

/// Disconnect from a Sonoma board
#[tauri::command]
pub async fn sonoma_disconnect(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
) -> Result<(), String> {
    state.sonoma().remove(&ip).await;
    Ok(())
}

/// Check if a board is alive
#[tauri::command]
pub async fn sonoma_is_alive(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
) -> Result<bool, String> {
    let client = state.sonoma().client(&ip).await?;
    Ok(client.is_alive().await)
}

/// Get composite board status (alive + hostname + FW + ADC readings)
#[tauri::command]
pub async fn sonoma_get_status(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
) -> Result<SonomaStatus, String> {
    let client = state.sonoma().client(&ip).await?;
    client.get_status().await.map_err(|e| e.to_string())
}

/// Scan an IP range for alive boards
#[tauri::command]
pub async fn sonoma_scan_range(
    state: tauri::State<'_, crate::state::AppState>,
    start: u8,
    end: u8,
    user: String,
    password: String,
) -> Result<Vec<SonomaBoardInfo>, String> {
    let ips = expand_ip_range(start, end)?;
    let mgr = state.sonoma();

    let mut results = Vec::new();

    // Scan concurrently using join handles
    let mut handles = Vec::new();
    for ip in ips {
        let user = user.clone();
        let password = password.clone();
        handles.push(tokio::spawn(async move {
            let client = SonomaClient::new(&ip, &user, &password);
            let alive = client.is_alive().await;
            let hostname = if alive {
                client.hostname().await.unwrap_or_default()
            } else {
                String::new()
            };
            SonomaBoardInfo {
                ip,
                alive,
                hostname,
            }
        }));
    }

    for handle in handles {
        if let Ok(info) = handle.await {
            if info.alive {
                // Store credentials for alive boards
                mgr.add(&info.ip, &user, &password).await;
            }
            results.push(info);
        }
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Power Commands
// ---------------------------------------------------------------------------

/// Initialize VICOR core (first-time setup with MIO)
#[tauri::command]
pub async fn sonoma_vicor_init(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    core: u8,
    voltage: f32,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client.vicor_init(core, voltage).await.map_err(|e| e.to_string())
}

/// Adjust VICOR voltage (after init)
#[tauri::command]
pub async fn sonoma_vicor_voltage(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    core: u8,
    voltage: f32,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client
        .vicor_voltage(core, voltage)
        .await
        .map_err(|e| e.to_string())
}

/// Disable a VICOR core
#[tauri::command]
pub async fn sonoma_vicor_disable(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    core: u8,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client.vicor_disable(core).await.map_err(|e| e.to_string())
}

/// Set PMBus channel voltage
#[tauri::command]
pub async fn sonoma_pmbus_set(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    channel: u8,
    voltage: f32,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client
        .pmbus_set(channel, voltage)
        .await
        .map_err(|e| e.to_string())
}

/// Disable PMBus channel
#[tauri::command]
pub async fn sonoma_pmbus_off(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    channel: u8,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client.pmbus_off(channel).await.map_err(|e| e.to_string())
}

/// Set IO power supply bank voltages
#[tauri::command]
pub async fn sonoma_io_ps(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    b13: f32,
    b33: f32,
    b34: f32,
    b35: f32,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client
        .io_ps(b13, b33, b34, b35)
        .await
        .map_err(|e| e.to_string())
}

/// Emergency stop — kill all power immediately
#[tauri::command]
pub async fn sonoma_emergency_stop(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client.emergency_stop().await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Analog Commands
// ---------------------------------------------------------------------------

/// Read XADC (32 internal FPGA channels)
#[tauri::command]
pub async fn sonoma_read_xadc(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
) -> Result<Vec<SonomaAnalogReading>, String> {
    let client = state.sonoma().client(&ip).await?;
    client.read_xadc().await.map_err(|e| e.to_string())
}

/// Read external ADC (32 channels, MAX11131)
#[tauri::command]
pub async fn sonoma_read_adc32(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
) -> Result<Vec<SonomaAnalogReading>, String> {
    let client = state.sonoma().client(&ip).await?;
    client.read_adc32().await.map_err(|e| e.to_string())
}

/// Read external ADC high bank (channels 16-31)
#[tauri::command]
pub async fn sonoma_read_adc32_high(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
) -> Result<Vec<SonomaAnalogReading>, String> {
    let client = state.sonoma().client(&ip).await?;
    client.read_adc32_high().await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Vector Commands
// ---------------------------------------------------------------------------

/// Load vectors from files on the board
#[tauri::command]
pub async fn sonoma_load_vectors(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    seq_path: String,
    hex_path: String,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client
        .load_vectors(&seq_path, &hex_path)
        .await
        .map_err(|e| e.to_string())
}

/// Run vectors (production mode)
#[tauri::command]
pub async fn sonoma_run_vectors(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    seq_path: String,
    time_s: u32,
    debug: bool,
) -> Result<SonomaRunResult, String> {
    let client = state.sonoma().client(&ip).await?;
    client
        .run_vectors(&seq_path, time_s, debug)
        .await
        .map_err(|e| e.to_string())
}

/// Set PLL frequency
#[tauri::command]
pub async fn sonoma_set_frequency(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    pll: u8,
    freq_hz: u32,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client
        .set_frequency(pll, freq_hz, 50)
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Pin/Timing Commands
// ---------------------------------------------------------------------------

/// Set pin type
#[tauri::command]
pub async fn sonoma_set_pin_type(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    pin: u8,
    pin_type: u8,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client
        .set_pin_type(pin, pin_type)
        .await
        .map_err(|e| e.to_string())
}

/// Set pulse delays for a pin
#[tauri::command]
pub async fn sonoma_set_pulse_delays(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    pin: u8,
    ptype: u8,
    rise: u32,
    fall: u32,
    period: u32,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client
        .set_pulse_delays(pin, ptype, rise, fall, period)
        .await
        .map_err(|e| e.to_string())
}

/// PLL on/off control (4 PLLs)
#[tauri::command]
pub async fn sonoma_pll_on_off(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    states: [bool; 4],
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client.pll_on_off(states).await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// DAC / GPIO / Memory Commands
// ---------------------------------------------------------------------------

/// Set all 10 external DAC channels
#[tauri::command]
pub async fn sonoma_set_ext_dac(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    channels: [f32; 10],
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client
        .set_ext_dac(&channels)
        .await
        .map_err(|e| e.to_string())
}

/// Toggle MIO pin
#[tauri::command]
pub async fn sonoma_toggle_mio(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    pin: u8,
    value: u8,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client.toggle_mio(pin, value).await.map_err(|e| e.to_string())
}

/// Read memory address
#[tauri::command]
pub async fn sonoma_read_mem(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    addr: u32,
) -> Result<u32, String> {
    let client = state.sonoma().client(&ip).await?;
    client.read_mem(addr).await.map_err(|e| e.to_string())
}

/// Write memory address
#[tauri::command]
pub async fn sonoma_write_mem(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    addr: u32,
    value: u32,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client.write_mem(addr, value).await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Temperature + Init Commands
// ---------------------------------------------------------------------------

/// Set temperature setpoint
#[tauri::command]
pub async fn sonoma_set_temperature(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
    setpoint: f32,
    r25c: f32,
    cool_after: bool,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client
        .set_temperature(setpoint, r25c, cool_after)
        .await
        .map_err(|e| e.to_string())
}

/// Full board initialization sequence
#[tauri::command]
pub async fn sonoma_init(
    state: tauri::State<'_, crate::state::AppState>,
    ip: String,
) -> Result<(), String> {
    let client = state.sonoma().client(&ip).await?;
    client.init().await.map_err(|e| e.to_string())
}
