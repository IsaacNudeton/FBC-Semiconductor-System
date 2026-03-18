//! Sonoma SSH Client — Legacy board control via SSH + ELF binaries
//!
//! Sonoma boards run Linux on Zynq 7020. Control is via SSH:
//! - Connect as root (no password, authenticate_none)
//! - Execute ELF binaries in /mnt/bin/
//! - Parse stdout for readings, exit code for success/failure
//! - Hardware lock via `flock -x /tmp/LockBit` for concurrent access

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use russh::*;
use russh::client;
use russh_keys::key;
use thiserror::Error;

use crate::types::*;
use crate::sonoma_parse;

// =============================================================================
// Error Types
// =============================================================================

#[derive(Error, Debug)]
pub enum SonomaError {
    #[error("SSH connection failed: {0}")]
    Connection(String),

    #[error("SSH auth failed: {0}")]
    Auth(String),

    #[error("Command failed (exit {code}): {stderr}")]
    Command { code: u32, stderr: String },

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("File transfer failed: {0}")]
    Transfer(String),

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
        // Accept all host keys (embedded Linux boards)
        Ok((self, true))
    }
}

// =============================================================================
// Command Result
// =============================================================================

/// Raw result from executing a command
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: u32,
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

// =============================================================================
// SonomaClient
// =============================================================================

/// SSH client for controlling Sonoma legacy boards
///
/// Each method connects, runs a command, captures output, disconnects.
/// This is intentionally stateless — no persistent connection.
pub struct SonomaClient {
    host: String,
    port: u16,
    user: String,
    password: String,
}

impl SonomaClient {
    pub fn new(host: &str, user: &str, password: &str) -> Self {
        SonomaClient {
            host: host.to_string(),
            port: 22,
            user: user.to_string(),
            password: password.to_string(),
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Internal: connect, authenticate, return session handle
    async fn connect(&self) -> Result<client::Handle<SonomaHandler>, SonomaError> {
        let config = Arc::new(client::Config::default());
        let handler = SonomaHandler;

        let mut session = client::connect(
            config,
            (self.host.as_str(), self.port),
            handler,
        )
        .await?;

        // Try password auth, fall back to none auth (embedded Linux root)
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

    /// Internal: run a command on an existing session, return ExecResult
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
    pub async fn exec(&self, cmd: &str) -> Result<ExecResult, SonomaError> {
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

    /// Execute without hardware lock (for read-only ops like hostname)
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

    /// Check if board is alive (SSH reachable)
    pub async fn is_alive(&self) -> bool {
        self.exec_unlocked("echo ok").await.is_ok()
    }

    /// Get hostname
    pub async fn hostname(&self) -> Result<String, SonomaError> {
        let r = self.exec_unlocked("cat /etc/hostname").await?;
        Ok(r.stdout.trim().to_string())
    }

    /// Get firmware version string
    pub async fn fw_version(&self) -> Result<String, SonomaError> {
        let r = self.exec_unlocked(
            "cat /mnt/version.txt 2>/dev/null || stat -c '%y' /boot/BOOT.BIN 2>/dev/null || echo unknown"
        ).await?;
        Ok(r.stdout.trim().to_string())
    }

    /// Composite status (alive + XADC + ADC32)
    pub async fn get_status(&self) -> Result<SonomaStatus, SonomaError> {
        let alive = self.is_alive().await;
        if !alive {
            return Ok(SonomaStatus {
                system_type: crate::types::SystemType::Sonoma,
                alive: false,
                ip: self.host.clone(),
                fw_version: String::new(),
                xadc: Vec::new(),
                adc32: Vec::new(),
            });
        }

        let fw = self.fw_version().await.unwrap_or_else(|_| "unknown".into());
        let xadc = self.read_xadc().await.unwrap_or_default();
        let adc32 = self.read_adc32().await.unwrap_or_default();

        Ok(SonomaStatus {
            system_type: crate::types::SystemType::Sonoma,
            alive: true,
            ip: self.host.clone(),
            fw_version: fw,
            xadc,
            adc32,
        })
    }

    // =========================================================================
    // Power — VICOR Core Supplies
    // =========================================================================

    /// Initialize VICOR core (first call — sets voltage + enables MIO)
    pub async fn vicor_init(&self, core: u8, voltage: f32) -> Result<(), SonomaError> {
        let (dac, mio) = Self::vicor_map(core)?;
        self.exec(&format!(
            "/mnt/bin/linux_VICOR.elf {} {} {}",
            voltage, mio, dac
        )).await?;
        Ok(())
    }

    /// Adjust VICOR voltage (after init)
    pub async fn vicor_voltage(&self, core: u8, voltage: f32) -> Result<(), SonomaError> {
        let (dac, _mio) = Self::vicor_map(core)?;
        // Sonoma uses voltage*2 for this ELF
        self.exec(&format!(
            "/mnt/bin/linux_VICOR_Voltage.elf {} {}",
            voltage * 2.0, dac
        )).await?;
        Ok(())
    }

    /// Disable a VICOR core (set voltage to 0)
    pub async fn vicor_disable(&self, core: u8) -> Result<(), SonomaError> {
        let (dac, mio) = Self::vicor_map(core)?;
        self.exec(&format!(
            "/mnt/bin/linux_VICOR.elf 0.0 {} {}",
            mio, dac
        )).await?;
        Ok(())
    }

    fn vicor_map(core: u8) -> Result<(u8, u8), SonomaError> {
        if core < 1 || core > 6 {
            return Err(SonomaError::Parse(format!("Invalid VICOR core {}, must be 1-6", core)));
        }
        let (dac, mio) = VICOR_MAP[(core - 1) as usize];
        Ok((dac, mio))
    }

    // =========================================================================
    // Power — PMBus (PicoDlynx / Low Current)
    // =========================================================================

    /// Set PMBus channel voltage
    pub async fn pmbus_set(&self, channel: u8, voltage: f32) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_pmbus_PicoDlynx.elf {} {}",
            channel, voltage
        )).await?;
        Ok(())
    }

    /// Turn off PMBus channel
    pub async fn pmbus_off(&self, channel: u8) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_pmbus_OFF.elf {}",
            channel
        )).await?;
        Ok(())
    }

    /// Set IO power supply voltages (4 banks)
    pub async fn io_ps(&self, b13: f32, b33: f32, b34: f32, b35: f32) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_IO_PS.elf {} {} {} {}",
            b13, b33, b34, b35
        )).await?;
        Ok(())
    }

    /// Emergency stop — disable all power
    pub async fn emergency_stop(&self) -> Result<(), SonomaError> {
        // Disable all PMBus channels (1-99)
        self.exec(
            "for i in $(seq 1 99); do /mnt/bin/linux_pmbus_OFF.elf $i 0 2>/dev/null; done"
        ).await.ok();
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

    /// Read XADC (internal Zynq ADC, 32 channels)
    pub async fn read_xadc(&self) -> Result<Vec<AnalogReading>, SonomaError> {
        let r = self.exec("/mnt/bin/XADC32Ch.elf").await?;
        sonoma_parse::parse_adc_csv(&r.stdout, 0)
            .map_err(SonomaError::Parse)
    }

    /// Read external 32-channel ADC (MAX11131)
    pub async fn read_adc32(&self) -> Result<Vec<AnalogReading>, SonomaError> {
        let r = self.exec("/mnt/bin/ADC32ChPlusStats.elf").await?;
        sonoma_parse::parse_adc_csv(&r.stdout, 0)
            .map_err(SonomaError::Parse)
    }

    /// Read high bank (channels 16-31) — toggles MIO 36
    pub async fn read_adc32_high(&self) -> Result<Vec<AnalogReading>, SonomaError> {
        self.exec("/mnt/bin/ToggleMio.elf 36 1").await?;
        let r = self.exec("/mnt/bin/ADC32ChPlusStats.elf").await?;
        self.exec("/mnt/bin/ToggleMio.elf 36 0").await?;
        sonoma_parse::parse_adc_csv(&r.stdout, 16)
            .map_err(SonomaError::Parse)
    }

    // =========================================================================
    // Vector Engine
    // =========================================================================

    /// Load vectors from files already on the board
    pub async fn load_vectors(&self, seq: &str, hex: &str) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_load_vectors.elf {} {}",
            seq, hex
        )).await?;
        Ok(())
    }

    /// Run vectors (production mode)
    pub async fn run_vectors(
        &self,
        seq: &str,
        time_s: u32,
        debug: bool,
    ) -> Result<RunResult, SonomaError> {
        let debug_flag = if debug { 1 } else { 0 };
        let r = self.exec(&format!(
            "/mnt/bin/RunSuperVector.elf {} {} {}",
            seq, time_s, debug_flag
        )).await;

        match r {
            Ok(result) => {
                sonoma_parse::parse_run_result(&result.stdout, time_s)
                    .map_err(SonomaError::Parse)
            }
            Err(SonomaError::Command { code, stderr: _ }) => {
                // Non-zero exit may mean vector failures (not SSH failure)
                Ok(RunResult {
                    passed: false,
                    vectors_executed: 0,
                    errors: code,
                    duration_s: time_s as f32,
                })
            }
            Err(e) => Err(e),
        }
    }

    /// Run single vector pattern (debug mode)
    pub async fn run_vector_debug(
        &self,
        pattern: u8,
        freq_en: u8,
        test_name: &str,
        log_data: bool,
    ) -> Result<RunResult, SonomaError> {
        let log_flag = if log_data { 1 } else { 0 };
        let r = self.exec(&format!(
            "/mnt/bin/linux_run_vector.elf {} {} {} {} 0 0",
            pattern, freq_en, test_name, log_flag
        )).await;

        match r {
            Ok(result) => {
                sonoma_parse::parse_run_result(&result.stdout, 0)
                    .map_err(SonomaError::Parse)
            }
            Err(SonomaError::Command { code, stderr: _ }) => {
                Ok(RunResult {
                    passed: false,
                    vectors_executed: 0,
                    errors: code,
                    duration_s: 0.0,
                })
            }
            Err(e) => Err(e),
        }
    }

    // =========================================================================
    // Clock / PLL
    // =========================================================================

    /// Set PLL frequency
    pub async fn set_frequency(
        &self,
        pll: u8,
        freq_hz: u32,
        duty_cycle: u8,
    ) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_xpll_frequency.elf {} {} 0 {}",
            pll, freq_hz, duty_cycle
        )).await?;
        Ok(())
    }

    /// Enable/disable PLLs (4 PLLs)
    pub async fn pll_on_off(&self, states: [bool; 4]) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_xpll_off_on.elf {} {} {} {}",
            states[0] as u8, states[1] as u8, states[2] as u8, states[3] as u8
        )).await?;
        Ok(())
    }

    // =========================================================================
    // Pin Configuration
    // =========================================================================

    /// Set pin type (0=BIDI, 1=INPUT, 2=OUTPUT, 3=OPEN_COLLECTOR, 4=PULSE, 5=NPULSE, 6=ERR_TRIG, 7=VEC_CLK)
    pub async fn set_pin_type(&self, pin: u8, pin_type: u8) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_pin_type.elf {} {}",
            pin, pin_type
        )).await?;
        Ok(())
    }

    /// Set pulse delays for a pin
    pub async fn set_pulse_delays(
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
        )).await?;
        Ok(())
    }

    // =========================================================================
    // DAC
    // =========================================================================

    /// Set all 10 external DAC channels
    pub async fn set_ext_dac(&self, channels: &[f32; 10]) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_EXT_DAC.elf {} {} {} {} {} {} {} {} {} {}",
            channels[0], channels[1], channels[2], channels[3], channels[4],
            channels[5], channels[6], channels[7], channels[8], channels[9]
        )).await?;
        Ok(())
    }

    /// Set single external DAC channel
    pub async fn set_ext_dac_single(&self, channel: u8, value: f32) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_EXT_DAC_singleCh.elf {} {}",
            channel, value
        )).await?;
        Ok(())
    }

    // =========================================================================
    // GPIO / MIO
    // =========================================================================

    /// Toggle MIO pin
    pub async fn toggle_mio(&self, pin: u8, value: u8) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/ToggleMio.elf {} {}",
            pin, value
        )).await?;
        Ok(())
    }

    // =========================================================================
    // Memory Access
    // =========================================================================

    /// Read memory address (rwmem)
    pub async fn read_mem(&self, addr: u32) -> Result<u32, SonomaError> {
        let r = self.exec(&format!("/mnt/rwmem.elf 0x{:08X}", addr)).await?;
        sonoma_parse::parse_hex_value(&r.stdout)
            .map_err(SonomaError::Parse)
    }

    /// Write memory address (rwmem)
    pub async fn write_mem(&self, addr: u32, value: u32) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/rwmem.elf 0x{:08X} 0x{:08X}",
            addr, value
        )).await?;
        Ok(())
    }

    // =========================================================================
    // Temperature
    // =========================================================================

    /// Set temperature setpoint (heater/fan control)
    pub async fn set_temperature(
        &self,
        setpoint: f32,
        r25c: f32,
        cool_after: bool,
    ) -> Result<(), SonomaError> {
        self.exec(&format!(
            "/mnt/bin/linux_set_temperature.elf {} {} {}",
            setpoint, r25c, cool_after as u8
        )).await?;
        Ok(())
    }

    // =========================================================================
    // Firmware Update
    // =========================================================================

    /// Update firmware by uploading BOOT.BIN via SCP and rebooting
    pub async fn update_firmware(&self, boot_bin: &Path) -> Result<(), SonomaError> {
        if !boot_bin.exists() {
            return Err(SonomaError::Transfer(format!(
                "File not found: {}",
                boot_bin.display()
            )));
        }

        // Upload via SCP (shell out — russh doesn't have SCP built in)
        let scp_status = tokio::process::Command::new("scp")
            .args([
                "-o", "StrictHostKeyChecking=no",
                "-o", "ConnectTimeout=10",
                &boot_bin.to_string_lossy(),
                &format!("{}@{}:/boot/BOOT.BIN", self.user, self.host),
            ])
            .status()
            .await
            .map_err(|e| SonomaError::Transfer(format!("scp failed: {}", e)))?;

        if !scp_status.success() {
            return Err(SonomaError::Transfer("SCP upload failed".into()));
        }

        // Sync and reboot
        self.exec_unlocked("sync && reboot").await.ok();

        Ok(())
    }

    // =========================================================================
    // Init Sequence (full board initialization)
    // =========================================================================

    /// Run the standard Sonoma init sequence
    /// Mirrors init.sh: CPU1 wakeup, zero DACs, XADC init, PMBus off
    pub async fn init(&self) -> Result<(), SonomaError> {
        self.exec("/mnt/bin/linux_cpu1_wakeup.elf").await.ok();
        self.set_ext_dac(&[0.0; 10]).await?;
        self.exec("/mnt/bin/linux_init_XADC.elf").await?;
        self.exec(
            "for i in $(seq 1 99); do /mnt/bin/linux_pmbus_OFF.elf $i 0 2>/dev/null; done"
        ).await.ok();
        Ok(())
    }
}
