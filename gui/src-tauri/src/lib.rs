//! FBC System GUI - Tauri Backend
//!
//! Bridges the React frontend to raw Ethernet FBC protocol.

mod fbc;
mod config;
mod state;
mod export;
mod switch;
mod realtime;

use state::AppState;
use tauri::{Emitter, Manager};

/// Tauri commands exposed to the frontend
mod commands {
    use super::*;
    use crate::config::RackConfig;
    use crate::fbc::{
        AnalogChannels, BoardInfo, BoardStatus, EepromData, FastPinState, PmBusStatus,
        VectorEngineStatus, VicorStatus,
    };

    /// List available network interfaces
    #[tauri::command]
    pub async fn list_interfaces() -> Result<Vec<String>, String> {
        Ok(fbc::list_interfaces())
    }

    /// Connect to FBC network on specified interface
    #[tauri::command]
    pub async fn connect(
        state: tauri::State<'_, AppState>,
        interface: String,
    ) -> Result<(), String> {
        state.connect(&interface).await.map_err(|e| e.to_string())
    }

    /// Disconnect from FBC network
    #[tauri::command]
    pub async fn disconnect(state: tauri::State<'_, AppState>) -> Result<(), String> {
        state.disconnect().await;
        Ok(())
    }

    /// Discover all boards on the network
    #[tauri::command]
    pub async fn discover_boards(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<BoardInfo>, String> {
        state.discover().await.map_err(|e| e.to_string())
    }

    /// Get status of a specific board
    #[tauri::command]
    pub async fn get_board_status(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<BoardStatus, String> {
        state.get_status(&mac).await.map_err(|e| e.to_string())
    }

    /// Start test on a board
    #[tauri::command]
    pub async fn start_board(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<(), String> {
        state.start(&mac).await.map_err(|e| e.to_string())
    }

    /// Stop test on a board
    #[tauri::command]
    pub async fn stop_board(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<(), String> {
        state.stop(&mac).await.map_err(|e| e.to_string())
    }

    /// Reset a board
    #[tauri::command]
    pub async fn reset_board(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<(), String> {
        state.reset(&mac).await.map_err(|e| e.to_string())
    }

    /// Upload vectors to a board
    #[tauri::command]
    pub async fn upload_vectors(
        state: tauri::State<'_, AppState>,
        mac: String,
        data: Vec<u8>,
    ) -> Result<(), String> {
        state.upload(&mac, &data).await.map_err(|e| e.to_string())
    }

    /// Get rack configuration
    #[tauri::command]
    pub fn get_rack_config(state: tauri::State<'_, AppState>) -> RackConfig {
        state.get_rack_config()
    }

    /// Set rack configuration
    #[tauri::command]
    pub fn set_rack_config(
        state: tauri::State<'_, AppState>,
        config: RackConfig,
    ) -> Result<(), String> {
        state.set_rack_config(config);
        Ok(())
    }

    /// Execute terminal command
    #[tauri::command]
    pub async fn terminal_command(
        state: tauri::State<'_, AppState>,
        command: String,
    ) -> Result<String, String> {
        state.execute_command(&command).await.map_err(|e| e.to_string())
    }
    /// Get fast pin state
    #[tauri::command]
    pub async fn get_fast_pins(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<FastPinState, String> {
        state.get_fast_pins(&mac).await.map_err(|e| e.to_string())
    }

    /// Set fast pin state
    #[tauri::command]
    pub async fn set_fast_pins(
        state: tauri::State<'_, AppState>,
        mac: String,
        dout: u32,
        oen: u32,
    ) -> Result<(), String> {
        state.set_fast_pins(&mac, dout, oen).await.map_err(|e| e.to_string())
    }

    // =========================================================================
    // Analog Monitoring Commands
    // =========================================================================

    /// Read all analog channels
    #[tauri::command]
    pub async fn read_analog_channels(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<AnalogChannels, String> {
        state.read_analog_channels(&mac).await.map_err(|e| e.to_string())
    }

    // =========================================================================
    // Power Control Commands - VICOR
    // =========================================================================

    /// Get VICOR core status
    #[tauri::command]
    pub async fn get_vicor_status(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<VicorStatus, String> {
        state.get_vicor_status(&mac).await.map_err(|e| e.to_string())
    }

    /// Enable/disable a VICOR core
    #[tauri::command]
    pub async fn set_vicor_enable(
        state: tauri::State<'_, AppState>,
        mac: String,
        core_id: u8,
        enable: bool,
    ) -> Result<(), String> {
        state.set_vicor_enable(&mac, core_id, enable).await.map_err(|e| e.to_string())
    }

    /// Set VICOR core voltage
    #[tauri::command]
    pub async fn set_vicor_voltage(
        state: tauri::State<'_, AppState>,
        mac: String,
        core_id: u8,
        voltage_mv: u16,
    ) -> Result<(), String> {
        state.set_vicor_voltage(&mac, core_id, voltage_mv).await.map_err(|e| e.to_string())
    }

    // =========================================================================
    // Power Control Commands - PMBus
    // =========================================================================

    /// Get PMBus rail status
    #[tauri::command]
    pub async fn get_pmbus_status(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<PmBusStatus, String> {
        state.get_pmbus_status(&mac).await.map_err(|e| e.to_string())
    }

    /// Enable/disable a PMBus rail
    #[tauri::command]
    pub async fn set_pmbus_enable(
        state: tauri::State<'_, AppState>,
        mac: String,
        address: u8,
        enable: bool,
    ) -> Result<(), String> {
        state.set_pmbus_enable(&mac, address, enable).await.map_err(|e| e.to_string())
    }

    /// Emergency stop all power
    #[tauri::command]
    pub async fn emergency_stop(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<(), String> {
        state.emergency_stop(&mac).await.map_err(|e| e.to_string())
    }

    /// Execute power-on sequence
    #[tauri::command]
    pub async fn power_sequence_on(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<(), String> {
        state.power_sequence_on(&mac).await.map_err(|e| e.to_string())
    }

    /// Execute power-off sequence
    #[tauri::command]
    pub async fn power_sequence_off(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<(), String> {
        state.power_sequence_off(&mac).await.map_err(|e| e.to_string())
    }

    // =========================================================================
    // EEPROM Commands
    // =========================================================================

    /// Read EEPROM contents
    #[tauri::command]
    pub async fn read_eeprom(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<EepromData, String> {
        state.read_eeprom(&mac).await.map_err(|e| e.to_string())
    }

    /// Write EEPROM data
    #[tauri::command]
    pub async fn write_eeprom(
        state: tauri::State<'_, AppState>,
        mac: String,
        offset: u8,
        data: Vec<u8>,
    ) -> Result<(), String> {
        state.write_eeprom(&mac, offset, &data).await.map_err(|e| e.to_string())
    }

    // =========================================================================
    // Vector Engine Commands
    // =========================================================================

    /// Get vector engine status
    #[tauri::command]
    pub async fn get_vector_status(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<VectorEngineStatus, String> {
        state.get_vector_status(&mac).await.map_err(|e| e.to_string())
    }

    /// Load vectors from file data
    #[tauri::command]
    pub async fn load_vectors(
        state: tauri::State<'_, AppState>,
        mac: String,
        data: Vec<u8>,
    ) -> Result<(), String> {
        state.load_vectors(&mac, &data).await.map_err(|e| e.to_string())
    }

    /// Start vector execution
    #[tauri::command]
    pub async fn start_vectors(
        state: tauri::State<'_, AppState>,
        mac: String,
        loops: u32,
    ) -> Result<(), String> {
        state.start_vectors(&mac, loops).await.map_err(|e| e.to_string())
    }

    /// Pause vector execution
    #[tauri::command]
    pub async fn pause_vectors(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<(), String> {
        state.pause_vectors(&mac).await.map_err(|e| e.to_string())
    }

    /// Resume vector execution
    #[tauri::command]
    pub async fn resume_vectors(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<(), String> {
        state.resume_vectors(&mac).await.map_err(|e| e.to_string())
    }

    /// Stop vector execution
    #[tauri::command]
    pub async fn stop_vectors(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<(), String> {
        state.stop_vectors(&mac).await.map_err(|e| e.to_string())
    }

    // =========================================================================
    // File I/O Commands (for Test Plan Editor)
    // =========================================================================

    /// Read a file from disk
    #[tauri::command]
    pub async fn read_file(path: String) -> Result<String, String> {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read file '{}': {}", path, e))
    }

    /// Write a file to disk
    #[tauri::command]
    pub async fn write_file(path: String, content: String) -> Result<(), String> {
        std::fs::write(&path, &content)
            .map_err(|e| format!("Failed to write file '{}': {}", path, e))
    }

    /// Get detailed board status (for BoardDetailPanel)
    #[tauri::command]
    pub async fn get_detailed_status(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<serde_json::Value, String> {
        state.get_detailed_status(&mac).await.map_err(|e| e.to_string())
    }

    /// Get EEPROM info (for BoardDetailPanel)
    #[tauri::command]
    pub async fn get_eeprom_info(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<serde_json::Value, String> {
        state.get_eeprom_info(&mac).await.map_err(|e| e.to_string())
    }

    // =========================================================================
    // Export Commands
    // =========================================================================

    /// Export test results to file
    #[tauri::command]
    pub async fn export_results(
        options: crate::export::ExportOptions,
        output_path: String,
        results: Vec<crate::export::TestResult>,
    ) -> Result<crate::export::ExportStats, String> {
        let path = std::path::PathBuf::from(output_path);
        crate::export::export_results(&results, &path, &options)
    }

    // =========================================================================
    // Device Configuration Commands
    // =========================================================================

    /// Pin configuration from device files
    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct DeviceConfigPin {
        pub pin: usize,
        pub name: String,
        #[serde(rename = "type")]
        pub pin_type: String,
        pub bank: usize,
        pub group: String,
    }

    /// Timing configuration from device files
    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct DeviceConfigTiming {
        pub setup_ns: u32,
        pub hold_ns: u32,
        pub strobe_ns: u32,
        pub period_ns: u32,
    }

    /// Compiled device configuration
    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct DeviceConfigResult {
        pub name: String,
        #[serde(rename = "type")]
        pub device_type: u16,
        pub version: u8,
        pub pins: Vec<DeviceConfigPin>,
        pub timing: DeviceConfigTiming,
        pub loaded: bool,
    }

    /// Compile device configuration from Sonoma files
    #[tauri::command]
    pub async fn compile_device_config(
        bim_path: String,
        map_path: Option<String>,
        tim_path: Option<String>,
    ) -> Result<DeviceConfigResult, String> {
        use std::io::{BufRead, BufReader};
        use std::fs::File;

        // Parse BIM file (simple XML parsing)
        let bim_content = std::fs::read_to_string(&bim_path)
            .map_err(|e| format!("Failed to read BIM file: {}", e))?;

        // Extract device name and type from BIM XML
        let device_name = extract_xml_attr(&bim_content, "Device", "name")
            .unwrap_or_else(|| "Unknown".to_string());
        let bim_type: u16 = extract_xml_attr(&bim_content, "BimType", "type")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Extract pin names from BIM (Signal type pins only)
        let mut pins = Vec::new();
        let mut pin_idx = 0;

        // Simple XML parsing for <Pin> elements
        for line in bim_content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("<Pin") && trimmed.contains("type=\"Signal\"") {
                // Extract pin name from element content
                if let Some(start) = trimmed.find('>') {
                    if let Some(end) = trimmed.rfind("</Pin>") {
                        let name = &trimmed[start + 1..end];
                        pins.push(DeviceConfigPin {
                            pin: pin_idx,
                            name: name.to_string(),
                            pin_type: "bidi".to_string(),
                            bank: pin_idx / 32,
                            group: format!("Bank {}", pin_idx / 32),
                        });
                        pin_idx += 1;
                    }
                }
            }
        }

        // Parse MAP file if provided (overrides pin names)
        if let Some(map_path) = map_path {
            if let Ok(file) = File::open(&map_path) {
                let reader = BufReader::new(file);
                for line in reader.lines().flatten() {
                    let line = line.trim().trim_end_matches(';');
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }

                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        // Format: "B13_GPIO25 SIGNAL_NAME"
                        if let Some(gpio_num) = extract_gpio_num(parts[0]) {
                            if gpio_num < 160 {
                                // Find or create pin entry
                                if let Some(pin) = pins.iter_mut().find(|p| p.pin == gpio_num) {
                                    pin.name = parts[1].to_string();
                                } else {
                                    pins.push(DeviceConfigPin {
                                        pin: gpio_num,
                                        name: parts[1].to_string(),
                                        pin_type: "bidi".to_string(),
                                        bank: gpio_num / 32,
                                        group: format!("Bank {}", gpio_num / 32),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Parse TIM file if provided (sets pin types and timing)
        let mut timing = DeviceConfigTiming {
            setup_ns: 10,
            hold_ns: 10,
            strobe_ns: 50,
            period_ns: 100,
        };

        if let Some(tim_path) = tim_path {
            if let Ok(content) = std::fs::read_to_string(&tim_path) {
                for line in content.lines() {
                    let trimmed = line.trim();

                    // Parse pintype commands like "pintype 0 BIDI"
                    if trimmed.starts_with("pintype ") {
                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        if parts.len() >= 3 {
                            if let Ok(pin_num) = parts[1].parse::<usize>() {
                                let pin_type = match parts[2].to_uppercase().as_str() {
                                    "BIDI" => "bidi",
                                    "INPUT" | "IN" => "input",
                                    "OUTPUT" | "OUT" => "output",
                                    "OPEN_C" | "OC" => "open_c",
                                    "PULSE" => "pulse",
                                    "NPULSE" => "npulse",
                                    _ => "bidi",
                                };

                                if let Some(pin) = pins.iter_mut().find(|p| p.pin == pin_num) {
                                    pin.pin_type = pin_type.to_string();
                                }
                            }
                        }
                    }

                    // Parse freq_sel for period calculation
                    if trimmed.starts_with("freq_sel ") {
                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        if parts.len() >= 2 {
                            if let Ok(freq_sel) = parts[1].parse::<u8>() {
                                timing.period_ns = match freq_sel {
                                    0 => 200,  // 5 MHz
                                    1 => 100,  // 10 MHz
                                    2 => 40,   // 25 MHz
                                    3 => 20,   // 50 MHz
                                    4 => 10,   // 100 MHz
                                    _ => 100,
                                };
                            }
                        }
                    }
                }
            }
        }

        // Sort pins by number
        pins.sort_by_key(|p| p.pin);

        Ok(DeviceConfigResult {
            name: device_name,
            device_type: bim_type,
            version: 1,
            pins,
            timing,
            loaded: true,
        })
    }

    /// Helper to extract XML attribute value
    fn extract_xml_attr(content: &str, element: &str, attr: &str) -> Option<String> {
        let pattern = format!("<{}", element);
        for line in content.lines() {
            if line.contains(&pattern) {
                let attr_pattern = format!("{}=\"", attr);
                if let Some(start) = line.find(&attr_pattern) {
                    let rest = &line[start + attr_pattern.len()..];
                    if let Some(end) = rest.find('"') {
                        return Some(rest[..end].to_string());
                    }
                }
            }
        }
        None
    }

    /// Helper to extract GPIO number from pin names like "B13_GPIO25"
    fn extract_gpio_num(name: &str) -> Option<usize> {
        let upper = name.to_uppercase();
        if let Some(pos) = upper.find("GPIO") {
            let after_gpio = &name[pos + 4..];
            let num_str = after_gpio.trim_start_matches('_');
            let digits: String = num_str.chars().take_while(|c| c.is_ascii_digit()).collect();
            return digits.parse().ok();
        }
        None
    }

    // =========================================================================
    // Firmware Update Commands
    // =========================================================================

    /// Board firmware type detection result
    #[derive(serde::Serialize)]
    pub struct FirmwareInfo {
        pub board_type: String,      // "linux" or "fbc"
        pub version: String,         // Firmware version string
        pub ip_address: Option<String>,  // IP if Linux (for SSH)
        pub mac_address: String,
    }

    /// Firmware update progress
    #[derive(serde::Serialize, Clone)]
    pub struct UpdateProgress {
        pub stage: String,           // "connecting", "uploading", "rebooting", "done", "error"
        pub percent: u8,
        pub message: String,
    }

    /// Detect what firmware a board is running (Linux SSH or FBC bare-metal)
    #[tauri::command]
    pub async fn detect_firmware_type(
        ip_or_mac: String,
    ) -> Result<FirmwareInfo, String> {
        // Try SSH first (Linux boards have IP)
        if ip_or_mac.contains('.') {
            // Looks like IP address, try SSH
            let output = std::process::Command::new("ssh")
                .args(["-o", "ConnectTimeout=2", "-o", "BatchMode=yes",
                       &format!("root@{}", ip_or_mac), "cat /etc/hostname"])
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    let hostname = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    return Ok(FirmwareInfo {
                        board_type: "linux".to_string(),
                        version: hostname,
                        ip_address: Some(ip_or_mac.clone()),
                        mac_address: String::new(),
                    });
                }
            }
        }

        // Fall back to FBC detection via raw Ethernet
        Ok(FirmwareInfo {
            board_type: "fbc".to_string(),
            version: "bare-metal".to_string(),
            ip_address: None,
            mac_address: ip_or_mac,
        })
    }

    /// Update firmware via SSH (for Linux boards)
    #[tauri::command]
    pub async fn update_firmware_ssh(
        ip_address: String,
        firmware_path: String,
    ) -> Result<String, String> {
        // Step 1: SCP the BOOT.BIN file
        let scp_result = std::process::Command::new("scp")
            .args(["-o", "StrictHostKeyChecking=no",
                   &firmware_path,
                   &format!("root@{}:/boot/BOOT.BIN", ip_address)])
            .output()
            .map_err(|e| format!("SCP failed: {}", e))?;

        if !scp_result.status.success() {
            return Err(format!("SCP failed: {}",
                String::from_utf8_lossy(&scp_result.stderr)));
        }

        // Step 2: Reboot the board
        let reboot_result = std::process::Command::new("ssh")
            .args(["-o", "StrictHostKeyChecking=no",
                   &format!("root@{}", ip_address),
                   "sync && reboot"])
            .output()
            .map_err(|e| format!("Reboot command failed: {}", e))?;

        Ok(format!("Firmware uploaded and reboot initiated for {}", ip_address))
    }

    /// Firmware info from board
    #[derive(serde::Serialize)]
    pub struct FbcFirmwareInfo {
        pub version: String,
        pub build_date: String,
        pub board_serial: u32,
        pub hw_revision: u8,
        pub sd_present: bool,
        pub update_in_progress: bool,
    }

    /// Get firmware info from a bare-metal FBC board
    #[tauri::command]
    pub async fn get_firmware_info(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<FbcFirmwareInfo, String> {
        state.get_firmware_info(&mac).await.map_err(|e| e.to_string())
    }

    /// Update firmware via FBC protocol (for bare-metal boards)
    /// Sends firmware in chunks, board writes to SD, then reboots
    #[tauri::command]
    pub async fn update_firmware_fbc(
        state: tauri::State<'_, AppState>,
        mac: String,
        firmware_data: Vec<u8>,
        app_handle: tauri::AppHandle,
    ) -> Result<String, String> {
        state.update_firmware_fbc(&mac, &firmware_data, app_handle).await
    }

    /// Batch update multiple boards
    #[tauri::command]
    pub async fn update_firmware_batch(
        boards: Vec<String>,  // List of IPs or MACs
        firmware_path: String,
    ) -> Result<Vec<(String, Result<String, String>)>, String> {
        let mut results = Vec::new();

        for board in boards {
            let result = if board.contains('.') {
                // IP address - use SSH
                match std::process::Command::new("scp")
                    .args(["-o", "StrictHostKeyChecking=no", "-o", "ConnectTimeout=5",
                           &firmware_path,
                           &format!("root@{}:/boot/BOOT.BIN", board)])
                    .output() {
                    Ok(out) if out.status.success() => {
                        // Trigger reboot
                        let _ = std::process::Command::new("ssh")
                            .args(["-o", "StrictHostKeyChecking=no",
                                   &format!("root@{}", board), "sync && reboot"])
                            .output();
                        Ok("Updated and rebooting".to_string())
                    }
                    Ok(out) => Err(String::from_utf8_lossy(&out.stderr).to_string()),
                    Err(e) => Err(e.to_string()),
                }
            } else {
                Err("FBC update not implemented".to_string())
            };
            results.push((board, result));
        }

        Ok(results)
    }

    // =========================================================================
    // Pattern Analysis Commands (for VectorPatternPanel)
    // =========================================================================

    /// Pattern statistics for visualization
    #[derive(serde::Serialize)]
    pub struct PatternStats {
        pub total_vectors: u32,
        pub total_cycles: u32,
        pub total_errors: u32,
        pub first_error_vector: i32,  // -1 if no errors
        pub first_error_cycle: i32,
        pub error_pins: Vec<u8>,      // List of pins with errors
    }

    /// Error info for error log
    #[derive(serde::Serialize)]
    pub struct ErrorInfo {
        pub vector: u32,
        pub cycle: u32,
        pub first_fail_pin: u8,
        pub error_mask: Vec<u8>,
        pub timestamp: i64,
    }

    /// Get pattern statistics for a board
    ///
    /// Returns statistics about vector execution including error information.
    /// NOTE: The error_pins list currently uses demo data. In production with
    /// real hardware, this would read from the error_counter RTL module's BRAM
    /// to get the actual list of pins that have errors.
    #[tauri::command]
    pub async fn get_pattern_stats(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<PatternStats, String> {
        // Get vector status from board
        let status = state.get_vector_status(&mac).await.map_err(|e| e.to_string())?;

        // Build error pin list from error counter
        // TODO: In production, read actual error pins from error_counter BRAM via:
        //   - AXI register at ERROR_BRAM_BASE that contains the error mask
        //   - Or iterate through error log entries to collect unique failing pins
        let mut error_pins = Vec::new();
        if status.error_count > 0 {
            // Demo data: simulate which pins have errors based on error count
            // This provides visual feedback during development without hardware
            error_pins.push(0);
            error_pins.push(1);
            if status.error_count > 5 {
                error_pins.push(7);
            }
            if status.error_count > 10 {
                error_pins.push(15);
                error_pins.push(42);
            }
            if status.error_count > 50 {
                error_pins.push(63);
                error_pins.push(128); // Fast pin
            }
        }

        Ok(PatternStats {
            total_vectors: status.total_vectors,
            total_cycles: status.total_vectors, // Assuming 1 cycle per vector; adjust based on repeat counts
            total_errors: status.error_count,
            first_error_vector: if status.first_fail_addr > 0 { status.first_fail_addr as i32 } else { -1 },
            first_error_cycle: if status.first_fail_addr > 0 { status.first_fail_addr as i32 } else { -1 },
            error_pins,
        })
    }

    /// Get error log entries
    ///
    /// Returns a list of error events captured during vector execution.
    /// NOTE: Currently returns demo data. In production with real hardware,
    /// this would read from the error_counter module's three BRAMs:
    ///   - error_bram: 128-bit error mask per error event
    ///   - vec_bram: vector number when error occurred
    ///   - cyc_bram: cycle number when error occurred
    #[tauri::command]
    pub async fn get_pattern_errors(
        state: tauri::State<'_, AppState>,
        mac: String,
        limit: u32,
    ) -> Result<Vec<ErrorInfo>, String> {
        let status = state.get_vector_status(&mac).await.map_err(|e| e.to_string())?;

        let mut errors = Vec::new();

        // TODO: In production, read from error BRAMs via AXI:
        //   for i in 0..min(status.error_count, limit) {
        //       let mask = read_axi(ERROR_BRAM_BASE + i * 16);  // 128-bit mask
        //       let vec = read_axi(VEC_BRAM_BASE + i * 4);      // vector number
        //       let cyc = read_axi(CYC_BRAM_BASE + i * 8);      // cycle number
        //       errors.push(ErrorInfo { ... });
        //   }

        // Demo data: generate error entries based on error count
        if status.error_count > 0 && status.first_fail_addr > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;

            // First error (always included if there are any errors)
            errors.push(ErrorInfo {
                vector: status.first_fail_addr,
                cycle: status.first_fail_addr,
                first_fail_pin: 0,
                error_mask: vec![0, 1],
                timestamp: now - 1000, // 1 second ago
            });

            // Additional demo errors for visualization
            if status.error_count > 5 {
                errors.push(ErrorInfo {
                    vector: status.first_fail_addr + 10,
                    cycle: status.first_fail_addr + 10,
                    first_fail_pin: 7,
                    error_mask: vec![7],
                    timestamp: now - 500,
                });
            }

            if status.error_count > 10 {
                errors.push(ErrorInfo {
                    vector: status.first_fail_addr + 25,
                    cycle: status.first_fail_addr + 25,
                    first_fail_pin: 15,
                    error_mask: vec![15, 42],
                    timestamp: now - 200,
                });
            }
        }

        // Respect the limit parameter
        errors.truncate(limit as usize);

        Ok(errors)
    }

    // ==================== Switch Integration ====================

    /// Get switch configuration
    #[tauri::command]
    pub async fn get_switch_config(
        state: tauri::State<'_, AppState>,
    ) -> Result<crate::switch::SwitchConfig, String> {
        Ok(state.get_switch_config().await)
    }

    /// Set switch configuration
    #[tauri::command]
    pub async fn set_switch_config(
        state: tauri::State<'_, AppState>,
        config: crate::switch::SwitchConfig,
    ) -> Result<(), String> {
        state.set_switch_config(config).await;
        Ok(())
    }

    /// Discover board positions by querying switch MAC table
    #[tauri::command]
    pub async fn discover_board_positions(
        state: tauri::State<'_, AppState>,
    ) -> Result<std::collections::HashMap<String, crate::switch::RackPosition>, String> {
        let config = state.get_switch_config().await;
        crate::switch::discover_board_positions(&config)
    }

    /// List available serial ports for switch connection
    #[tauri::command]
    pub fn list_serial_ports() -> Vec<String> {
        serialport::available_ports()
            .map(|ports| ports.into_iter().map(|p| p.port_name).collect())
            .unwrap_or_default()
    }

    // ==================== Realtime Monitoring ====================

    /// Get all live board states
    #[tauri::command]
    pub async fn get_live_boards(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<crate::realtime::LiveBoardState>, String> {
        Ok(state.get_live_boards().await)
    }

    /// Get a specific board's live state
    #[tauri::command]
    pub async fn get_live_board(
        state: tauri::State<'_, AppState>,
        mac: String,
    ) -> Result<Option<crate::realtime::LiveBoardState>, String> {
        Ok(state.get_live_board(&mac).await)
    }
}

/// Start background tasks for realtime monitoring
fn setup_realtime_tasks(app: &tauri::App) {
    let state = app.state::<AppState>();
    let realtime = state.realtime().clone();
    let app_handle = app.handle().clone();

    // Task 1: Emit board events to frontend
    let event_monitor = realtime.clone();
    let event_app = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let mut rx = event_monitor.subscribe();
        loop {
            match rx.recv().await {
                Ok(event) => {
                    // Emit to frontend
                    let event_name = match &event {
                        realtime::BoardEvent::Connected { .. } => "board:connected",
                        realtime::BoardEvent::Disconnected { .. } => "board:disconnected",
                        realtime::BoardEvent::StateChanged { .. } => "board:state-changed",
                        realtime::BoardEvent::Error { .. } => "board:error",
                        realtime::BoardEvent::Heartbeat { .. } => "board:heartbeat",
                    };
                    let _ = event_app.emit(event_name, &event);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // Receiver lagged, continue
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    // Task 2: Check for disconnected boards (timeout detection)
    let timeout_monitor = realtime.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
        loop {
            interval.tick().await;
            timeout_monitor.check_timeouts().await;
        }
    });

    tracing::info!("Realtime monitoring tasks started");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .setup(|app| {
            setup_realtime_tasks(app);

            // Auto-connect to first Ethernet interface
            let state = app.state::<AppState>();
            let state_inner = state.inner().clone();
            tauri::async_runtime::spawn(async move {
                // Find first Ethernet-like interface (not Wi-Fi, not loopback)
                let interfaces = fbc::list_interfaces();
                for iface in interfaces {
                    let iface_lower = iface.to_lowercase();
                    // Skip Wi-Fi and virtual adapters
                    if iface_lower.contains("wi-fi") ||
                       iface_lower.contains("wifi") ||
                       iface_lower.contains("wireless") ||
                       iface_lower.contains("virtual") ||
                       iface_lower.contains("loopback") {
                        continue;
                    }
                    // Try to connect to this interface
                    tracing::info!("Auto-connecting to interface: {}", iface);
                    if let Err(e) = state_inner.connect(&iface).await {
                        tracing::warn!("Failed to auto-connect to {}: {}", iface, e);
                    } else {
                        tracing::info!("Auto-connected to {}", iface);
                        break;
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Connection & discovery
            commands::list_interfaces,
            commands::connect,
            commands::disconnect,
            commands::discover_boards,
            // Board control
            commands::get_board_status,
            commands::start_board,
            commands::stop_board,
            commands::reset_board,
            commands::upload_vectors,
            // Configuration
            commands::get_rack_config,
            commands::set_rack_config,
            // Fast pins
            commands::get_fast_pins,
            commands::set_fast_pins,
            // Analog monitoring
            commands::read_analog_channels,
            // Power control - VICOR
            commands::get_vicor_status,
            commands::set_vicor_enable,
            commands::set_vicor_voltage,
            // Power control - PMBus
            commands::get_pmbus_status,
            commands::set_pmbus_enable,
            commands::emergency_stop,
            commands::power_sequence_on,
            commands::power_sequence_off,
            // EEPROM
            commands::read_eeprom,
            commands::write_eeprom,
            // Vector engine
            commands::get_vector_status,
            commands::load_vectors,
            commands::start_vectors,
            commands::pause_vectors,
            commands::resume_vectors,
            commands::stop_vectors,
            // Terminal
            commands::terminal_command,
            // File I/O
            commands::read_file,
            commands::write_file,
            // Board detail
            commands::get_detailed_status,
            commands::get_eeprom_info,
            // Export
            commands::export_results,
            // Pattern analysis
            commands::get_pattern_stats,
            commands::get_pattern_errors,
            // Device configuration
            commands::compile_device_config,
            // Firmware update
            commands::detect_firmware_type,
            commands::update_firmware_ssh,
            commands::update_firmware_fbc,
            commands::update_firmware_batch,
            // Switch integration
            commands::get_switch_config,
            commands::set_switch_config,
            commands::discover_board_positions,
            commands::list_serial_ports,
            // Realtime monitoring
            commands::get_live_boards,
            commands::get_live_board,
            // FBC firmware info/update
            commands::get_firmware_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
