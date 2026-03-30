/// Transport layer — dispatches commands to FbcClient / SonomaClient.
/// Runs on a dedicated tokio runtime in a background thread.
/// Unified: no mode toggle, both protocols active concurrently.

use std::io::{Read as IoRead, Write as IoWrite};
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;
use fbc_host::FbcClient;
use fbc_host::sonoma::SonomaClient;
use fbc_host::types::*;
use crate::state::SwitchPort;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BoardId {
    Mac([u8; 6]),
    Ip(String),
}

/// Commands sent from UI thread to hardware thread
#[allow(dead_code)]
pub enum HwCommand {
    // ---- Discovery ----
    ListInterfaces,
    Discover { interface: String, timeout_ms: u64 },
    ScanSonoma { start: String, end: String, user: String, password: String },
    DiscoverAll { interface: String, timeout_ms: u64, start: String, end: String, user: String, password: String },

    // ---- Board status (FBC) ----
    GetStatus(BoardId),
    Ping(BoardId),

    // ---- Control (FBC) ----
    Start(BoardId),
    Stop(BoardId),
    Reset(BoardId),
    EmergencyStop(BoardId),

    // ---- Power (FBC) ----
    GetVicorStatus(BoardId),
    SetVicorEnable(BoardId, u8),       // core_mask
    SetVicorVoltage(BoardId, u8, u16), // core, voltage_mv
    GetPmbusStatus(BoardId),
    SetPmbusEnable(BoardId, u8, bool), // addr, enable
    PowerSequenceOn(BoardId, [u16; 6]),
    PowerSequenceOff(BoardId),

    // ---- Analog (FBC) ----
    ReadAnalog(BoardId),

    // ---- Vectors (FBC) ----
    GetVectorStatus(BoardId),
    UploadVectors(BoardId, Vec<u8>),
    StartVectors(BoardId, u32),        // loops
    PauseVectors(BoardId),
    ResumeVectors(BoardId),
    StopVectors(BoardId),

    // ---- EEPROM (FBC) ----
    ReadEeprom(BoardId, u8, u8),       // offset, length
    WriteEeprom(BoardId, u8, Vec<u8>), // offset, data

    // ---- Fast pins (FBC) ----
    GetFastPins(BoardId),
    SetFastPins(BoardId, u32, u32),    // dout, oen

    // ---- Error log (FBC) ----
    GetErrorLog(BoardId, u32, u32),    // start, count

    // ---- Firmware (FBC) ----
    GetFirmwareInfo(BoardId),
    FirmwareUpdate(BoardId, Vec<u8>, u32), // data, checksum

    // ---- Flight recorder (FBC) ----
    GetLogInfo(BoardId),

    // ---- Sonoma: Status / Discovery ----
    SonomaGetStatus(String),           // ip

    // ---- Sonoma: Power ----
    SonomaVicorInit(String, u8, f32),      // ip, core, voltage
    SonomaVicorVoltage(String, u8, f32),   // ip, core, voltage
    SonomaVicorDisable(String, u8),        // ip, core
    SonomaPmbusSet(String, u8, f32),       // ip, channel, voltage
    SonomaPmbusOff(String, u8),            // ip, channel
    SonomaIoPs(String, f32, f32, f32, f32), // ip, b13, b33, b34, b35
    SonomaEmergencyStop(String),           // ip

    // ---- Sonoma: Analog ----
    SonomaReadXadc(String),                // ip
    SonomaReadAdc32(String),               // ip

    // ---- Sonoma: Vectors ----
    SonomaLoadVectors(String, String, String),         // ip, seq, hex
    SonomaRunVectors(String, String, u32, bool),       // ip, seq, time_s, debug

    // ---- Sonoma: Config ----
    SonomaSetPinType(String, u8, u8),                  // ip, pin, type
    SonomaSetFrequency(String, u8, u32, u8),           // ip, pll, freq_hz, duty
    SonomaSetTemperature(String, f32, f32, bool),      // ip, setpoint, r25c, cool
    SonomaInit(String),                                // ip
    SonomaToggleMio(String, u8, u8),                   // ip, pin, value

    // ---- Sonoma: Firmware ----
    SonomaUpdateFirmware(String, String),               // ip, path

    // ---- Sonoma: Generic exec ----
    SonomaExec(String, String),                         // ip, command

    // ---- Cisco Switch (Serial) ----
    SwitchConnect { com_port: String },
    SwitchDisconnect,
    SwitchPollPorts,                                     // show interfaces status + show mac address-table
    SwitchSendCommand(String),                           // raw CLI command
    SwitchSetVlan { port: String, vlan: u16 },
    SwitchSetDescription { port: String, desc: String },
    SwitchShutdown { port: String, shutdown: bool },     // true=shutdown, false=no shutdown
}

/// Responses sent from hardware thread back to UI
#[allow(dead_code)]
pub enum HwResponse {
    // Discovery
    FbcBoards(Vec<fbc_host::BoardInfo>),
    SonomaBoards(Vec<(String, bool, String)>), // Vec<(ip, alive, fw_version)>

    // FBC typed responses
    BoardStatus(BoardId, StatusResponse),
    VicorStatus(BoardId, VicorStatus),
    VectorStatus(BoardId, VectorEngineStatus),
    AnalogChannels(BoardId, AnalogChannels),
    EepromData(BoardId, EepromData),
    FastPins(BoardId, FastPinState),
    ErrorLog(BoardId, ErrorLogResponse),
    FirmwareInfoResp(BoardId, FirmwareInfo),
    PmbusStatus(BoardId, PmBusStatus),

    // Sonoma typed responses
    SonomaStatusResp(BoardId, SonomaStatus),
    RunResult(BoardId, RunResult),

    // Cisco Switch
    SwitchConnected(String),                            // hostname
    SwitchDisconnected,
    SwitchPortMap(Vec<crate::state::SwitchPort>),
    SwitchCommandResult(String),                        // raw output
    SwitchError(String),

    // Generic
    Error(String),
    Ok(String),
}

/// Helper: create a SonomaClient for a given IP (using stored creds)
fn sonoma_client(ip: &str) -> SonomaClient {
    SonomaClient::new(ip, "root", "")
}

/// Main hardware I/O loop — runs on a dedicated thread with its own tokio runtime.
/// Processes commands from UI and auto-polls discovered boards every 3 seconds.
pub async fn hardware_loop(
    mut cmd_rx: mpsc::Receiver<HwCommand>,
    rsp_tx: mpsc::Sender<HwResponse>,
) {
    let mut fbc_client: Option<FbcClient> = None;
    let mut discovered_macs: Vec<[u8; 6]> = Vec::new();
    let mut discovered_ips: Vec<String> = Vec::new();
    let mut switch_port: Option<Box<dyn serialport::SerialPort>> = None;
    let mut poll_interval = tokio::time::interval(Duration::from_secs(3));
    poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        let cmd = tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(c) => c,
                None => break, // UI closed
            },
            _ = poll_interval.tick() => {
                // Auto-poll: send GetStatus for all discovered boards
                for mac in &discovered_macs {
                    if let Some(ref mut c) = fbc_client {
                        if let Ok(s) = c.get_status(mac) {
                            let _ = rsp_tx.send(HwResponse::BoardStatus(BoardId::Mac(*mac), s)).await;
                        }
                    }
                }
                for ip in &discovered_ips {
                    let client = sonoma_client(ip);
                    if let Ok(s) = client.get_status().await {
                        let _ = rsp_tx.send(HwResponse::SonomaStatusResp(BoardId::Ip(ip.clone()), s)).await;
                    }
                }
                continue;
            }
        };
        let rsp = match cmd {
            HwCommand::ListInterfaces => {
                let ifaces = FbcClient::list_interfaces();
                HwResponse::Ok(format!("Interfaces: {}", ifaces.join(", ")))
            }

            HwCommand::Discover { interface, timeout_ms } => {
                match FbcClient::new(&interface) {
                    Ok(mut client) => {
                        match client.discover(Duration::from_millis(timeout_ms)) {
                            Ok(boards) => {
                                discovered_macs = boards.iter().map(|b| b.mac).collect();
                                let rsp = HwResponse::FbcBoards(boards);
                                fbc_client = Some(client);
                                rsp
                            }
                            Err(e) => HwResponse::Error(format!("Discovery failed: {}", e)),
                        }
                    }
                    Err(e) => HwResponse::Error(format!("Interface error: {}", e)),
                }
            }

            HwCommand::ScanSonoma { start, end, user, password } => {
                match fbc_host::sonoma::expand_ip_range(&format!("{}-{}", start, end)) {
                    Ok(ips) => {
                        let mut entries = Vec::new();
                        let mut alive_ips = Vec::new();
                        for ip in ips {
                            let client = SonomaClient::new(&ip, &user, &password);
                            let alive = client.is_alive().await;
                            let fw: String = if alive {
                                alive_ips.push(ip.clone());
                                client.fw_version().await.unwrap_or_default()
                            } else {
                                String::new()
                            };
                            entries.push((ip, alive, fw));
                        }
                        discovered_ips = alive_ips;
                        HwResponse::SonomaBoards(entries)
                    }
                    Err(e) => HwResponse::Error(format!("IP range error: {}", e)),
                }
            }

            HwCommand::DiscoverAll { interface, timeout_ms, start, end, user, password } => {
                // Run FBC discovery
                let mut fbc_boards = Vec::new();
                if let Ok(mut client) = FbcClient::new(&interface) {
                    if let Ok(boards) = client.discover(Duration::from_millis(timeout_ms)) {
                        discovered_macs = boards.iter().map(|b| b.mac).collect();
                        fbc_boards = boards;
                        fbc_client = Some(client);
                    }
                }
                // Send FBC results first
                if !fbc_boards.is_empty() {
                    let _ = rsp_tx.send(HwResponse::FbcBoards(fbc_boards)).await;
                }
                // Then scan Sonoma
                if let Ok(ips) = fbc_host::sonoma::expand_ip_range(&format!("{}-{}", start, end)) {
                    let mut entries = Vec::new();
                    let mut alive_ips = Vec::new();
                    for ip in ips {
                        let client = SonomaClient::new(&ip, &user, &password);
                        let alive = client.is_alive().await;
                        let fw = if alive {
                            alive_ips.push(ip.clone());
                            client.fw_version().await.unwrap_or_default()
                        } else {
                            String::new()
                        };
                        entries.push((ip, alive, fw));
                    }
                    discovered_ips = alive_ips;
                    HwResponse::SonomaBoards(entries)
                } else {
                    HwResponse::Ok("FBC discovery complete".into())
                }
            }

            // ==================================================================
            // FBC Board Commands
            // ==================================================================

            HwCommand::GetStatus(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.get_status(&mac).map(|s| HwResponse::BoardStatus(BoardId::Mac(mac), s))
                })
            }

            HwCommand::Ping(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.ping(&mac).map(|rtt| HwResponse::Ok(format!("Ping: {:.2}ms", rtt.as_secs_f64() * 1000.0)))
                })
            }

            HwCommand::Start(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| c.start(&mac).map(|()| HwResponse::Ok("Started".into())))
            }

            HwCommand::Stop(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| c.stop(&mac).map(|()| HwResponse::Ok("Stopped".into())))
            }

            HwCommand::Reset(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| c.reset(&mac).map(|()| HwResponse::Ok("Reset".into())))
            }

            HwCommand::EmergencyStop(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| c.emergency_stop(&mac).map(|()| HwResponse::Ok("EMERGENCY STOP".into())))
            }

            HwCommand::GetVicorStatus(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.get_vicor_status(&mac).map(|v| HwResponse::VicorStatus(BoardId::Mac(mac), v))
                })
            }

            HwCommand::SetVicorEnable(BoardId::Mac(mac), mask) => {
                fbc_cmd(&mut fbc_client, |c| c.set_vicor_enable(&mac, mask).map(|()| HwResponse::Ok("VICOR enabled".into())))
            }

            HwCommand::SetVicorVoltage(BoardId::Mac(mac), core, mv) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.set_vicor_voltage(&mac, core, mv).map(|()| HwResponse::Ok(format!("Core {} -> {} mV", core, mv)))
                })
            }

            HwCommand::ReadAnalog(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.read_analog(&mac).map(|a| HwResponse::AnalogChannels(BoardId::Mac(mac), a))
                })
            }

            HwCommand::GetVectorStatus(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.get_vector_status(&mac).map(|v| HwResponse::VectorStatus(BoardId::Mac(mac), v))
                })
            }

            HwCommand::UploadVectors(BoardId::Mac(mac), data) => {
                fbc_cmd(&mut fbc_client, |c| c.upload_vectors(&mac, &data).map(|()| HwResponse::Ok("Vectors uploaded".into())))
            }

            HwCommand::StartVectors(BoardId::Mac(mac), loops) => {
                fbc_cmd(&mut fbc_client, |c| c.start_vectors(&mac, loops).map(|()| HwResponse::Ok("Vectors started".into())))
            }

            HwCommand::PauseVectors(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| c.pause_vectors(&mac).map(|()| HwResponse::Ok("Vectors paused".into())))
            }

            HwCommand::ResumeVectors(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| c.resume_vectors(&mac).map(|()| HwResponse::Ok("Vectors resumed".into())))
            }

            HwCommand::StopVectors(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| c.stop_vectors(&mac).map(|()| HwResponse::Ok("Vectors stopped".into())))
            }

            HwCommand::ReadEeprom(BoardId::Mac(mac), offset, len) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.read_eeprom(&mac, offset, len).map(|data| HwResponse::EepromData(BoardId::Mac(mac), data))
                })
            }

            HwCommand::WriteEeprom(BoardId::Mac(mac), offset, data) => {
                fbc_cmd(&mut fbc_client, |c| c.write_eeprom(&mac, offset, &data).map(|()| HwResponse::Ok("EEPROM written".into())))
            }

            HwCommand::GetFastPins(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.get_fast_pins(&mac).map(|p| HwResponse::FastPins(BoardId::Mac(mac), p))
                })
            }

            HwCommand::SetFastPins(BoardId::Mac(mac), dout, oen) => {
                fbc_cmd(&mut fbc_client, |c| c.set_fast_pins(&mac, dout, oen).map(|()| HwResponse::Ok("Fast pins set".into())))
            }

            HwCommand::GetErrorLog(BoardId::Mac(mac), start, count) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.get_error_log(&mac, start, count).map(|log| HwResponse::ErrorLog(BoardId::Mac(mac), log))
                })
            }

            HwCommand::GetFirmwareInfo(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.get_firmware_info(&mac).map(|fw| HwResponse::FirmwareInfoResp(BoardId::Mac(mac), fw))
                })
            }

            HwCommand::FirmwareUpdate(BoardId::Mac(mac), data, checksum) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.firmware_update(&mac, &data, checksum).map(|ack| HwResponse::Ok(format!("FW update: status={}", ack.status)))
                })
            }

            HwCommand::GetLogInfo(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.get_log_info(&mac).map(|info| HwResponse::Ok(format!("Log: {} entries", info.total_entries)))
                })
            }

            HwCommand::GetPmbusStatus(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| {
                    c.get_pmbus_status(&mac).map(|pm| HwResponse::PmbusStatus(BoardId::Mac(mac), pm))
                })
            }

            HwCommand::SetPmbusEnable(BoardId::Mac(mac), addr, enable) => {
                fbc_cmd(&mut fbc_client, |c| c.set_pmbus_enable(&mac, addr, enable).map(|()| HwResponse::Ok("PMBus set".into())))
            }

            HwCommand::PowerSequenceOn(BoardId::Mac(mac), voltages) => {
                fbc_cmd(&mut fbc_client, |c| c.power_sequence_on(&mac, voltages).map(|()| HwResponse::Ok("Power ON".into())))
            }

            HwCommand::PowerSequenceOff(BoardId::Mac(mac)) => {
                fbc_cmd(&mut fbc_client, |c| c.power_sequence_off(&mac).map(|()| HwResponse::Ok("Power OFF".into())))
            }

            // ==================================================================
            // Sonoma SSH Commands
            // ==================================================================

            HwCommand::SonomaGetStatus(ip) => {
                let client = sonoma_client(&ip);
                match client.get_status().await {
                    Ok(s) => HwResponse::SonomaStatusResp(BoardId::Ip(ip), s),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaVicorInit(ip, core, voltage) => {
                let client = sonoma_client(&ip);
                match client.vicor_init(core, voltage).await {
                    Ok(()) => HwResponse::Ok(format!("VICOR core {} init @ {:.3}V", core, voltage)),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaVicorVoltage(ip, core, voltage) => {
                let client = sonoma_client(&ip);
                match client.vicor_voltage(core, voltage).await {
                    Ok(()) => HwResponse::Ok(format!("VICOR core {} -> {:.3}V", core, voltage)),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaVicorDisable(ip, core) => {
                let client = sonoma_client(&ip);
                match client.vicor_disable(core).await {
                    Ok(()) => HwResponse::Ok(format!("VICOR core {} disabled", core)),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaPmbusSet(ip, ch, voltage) => {
                let client = sonoma_client(&ip);
                match client.pmbus_set(ch, voltage).await {
                    Ok(()) => HwResponse::Ok(format!("PMBus ch{} -> {:.3}V", ch, voltage)),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaPmbusOff(ip, ch) => {
                let client = sonoma_client(&ip);
                match client.pmbus_off(ch).await {
                    Ok(()) => HwResponse::Ok(format!("PMBus ch{} off", ch)),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaIoPs(ip, b13, b33, b34, b35) => {
                let client = sonoma_client(&ip);
                match client.io_ps(b13, b33, b34, b35).await {
                    Ok(()) => HwResponse::Ok("IO PS set".into()),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaEmergencyStop(ip) => {
                let client = sonoma_client(&ip);
                match client.emergency_stop().await {
                    Ok(()) => HwResponse::Ok("Sonoma EMERGENCY STOP".into()),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaReadXadc(ip) => {
                let client = sonoma_client(&ip);
                match client.read_xadc().await {
                    Ok(readings) => {
                        HwResponse::AnalogChannels(
                            BoardId::Ip(ip),
                            AnalogChannels { xadc: readings, external: Vec::new() },
                        )
                    }
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaReadAdc32(ip) => {
                let client = sonoma_client(&ip);
                match client.read_adc32().await {
                    Ok(readings) => {
                        HwResponse::AnalogChannels(
                            BoardId::Ip(ip),
                            AnalogChannels { xadc: Vec::new(), external: readings },
                        )
                    }
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaLoadVectors(ip, seq, hex) => {
                let client = sonoma_client(&ip);
                match client.load_vectors(&seq, &hex).await {
                    Ok(()) => HwResponse::Ok("Vectors loaded".into()),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaRunVectors(ip, seq, time_s, debug) => {
                let client = sonoma_client(&ip);
                match client.run_vectors(&seq, time_s, debug).await {
                    Ok(r) => HwResponse::RunResult(BoardId::Ip(ip), r),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaSetPinType(ip, pin, ptype) => {
                let client = sonoma_client(&ip);
                match client.set_pin_type(pin, ptype).await {
                    Ok(()) => HwResponse::Ok(format!("Pin {} -> type {}", pin, ptype)),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaSetFrequency(ip, pll, freq, duty) => {
                let client = sonoma_client(&ip);
                match client.set_frequency(pll, freq, duty).await {
                    Ok(()) => HwResponse::Ok(format!("PLL{} -> {}Hz", pll, freq)),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaSetTemperature(ip, sp, r25c, cool) => {
                let client = sonoma_client(&ip);
                match client.set_temperature(sp, r25c, cool).await {
                    Ok(()) => HwResponse::Ok(format!("Temp -> {:.1}C", sp)),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaInit(ip) => {
                let client = sonoma_client(&ip);
                match client.init().await {
                    Ok(()) => HwResponse::Ok("Sonoma initialized".into()),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaToggleMio(ip, pin, value) => {
                let client = sonoma_client(&ip);
                match client.toggle_mio(pin, value).await {
                    Ok(()) => HwResponse::Ok(format!("MIO{} -> {}", pin, value)),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaUpdateFirmware(ip, path) => {
                let client = sonoma_client(&ip);
                match client.update_firmware(Path::new(&path)).await {
                    Ok(()) => HwResponse::Ok("Firmware uploaded, rebooting...".into()),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::SonomaExec(ip, command) => {
                let client = sonoma_client(&ip);
                match client.exec(&command).await {
                    Ok(r) => HwResponse::Ok(r.stdout),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            // Sonoma-addressed variants for FBC-only commands
            HwCommand::EmergencyStop(BoardId::Ip(ip)) => {
                let client = sonoma_client(&ip);
                match client.emergency_stop().await {
                    Ok(()) => HwResponse::Ok("Sonoma EMERGENCY STOP".into()),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::GetStatus(BoardId::Ip(ip)) => {
                let client = sonoma_client(&ip);
                match client.get_status().await {
                    Ok(s) => HwResponse::SonomaStatusResp(BoardId::Ip(ip), s),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            HwCommand::ReadAnalog(BoardId::Ip(ip)) => {
                let client = sonoma_client(&ip);
                let xadc = client.read_xadc().await.unwrap_or_default();
                let ext = client.read_adc32().await.unwrap_or_default();
                HwResponse::AnalogChannels(
                    BoardId::Ip(ip),
                    AnalogChannels { xadc, external: ext },
                )
            }

            HwCommand::GetVicorStatus(BoardId::Ip(_ip)) => {
                HwResponse::Error("VICOR status not available via SSH (no readback)".into())
            }

            HwCommand::GetVectorStatus(BoardId::Ip(_ip)) => {
                HwResponse::Error("Vector status not available via Sonoma SSH".into())
            }

            HwCommand::GetFirmwareInfo(BoardId::Ip(ip)) => {
                let client = sonoma_client(&ip);
                match client.fw_version().await {
                    Ok(fw) => HwResponse::Ok(format!("Sonoma FW: {}", fw)),
                    Err(e) => HwResponse::Error(format!("{}", e)),
                }
            }

            // ==================================================================
            // Cisco Switch Serial Commands
            // ==================================================================

            HwCommand::SwitchConnect { com_port } => {
                match serialport::new(&com_port, 9600)
                    .data_bits(serialport::DataBits::Eight)
                    .parity(serialport::Parity::None)
                    .stop_bits(serialport::StopBits::One)
                    .timeout(Duration::from_secs(2))
                    .open()
                {
                    Ok(mut port) => {
                        // Send a newline to get a prompt
                        let _ = port.write_all(b"\r\n");
                        std::thread::sleep(Duration::from_millis(500));
                        let hostname = switch_read_until_prompt(&mut port);
                        switch_port = Some(port);
                        HwResponse::SwitchConnected(hostname)
                    }
                    Err(e) => HwResponse::SwitchError(format!("Serial open failed: {}", e)),
                }
            }

            HwCommand::SwitchDisconnect => {
                switch_port = None;
                HwResponse::SwitchDisconnected
            }

            HwCommand::SwitchPollPorts => {
                match switch_port.as_mut() {
                    Some(port) => {
                        // Get interface status
                        let iface_output = switch_send_command(port, "show interfaces status");
                        // Get MAC address table
                        let mac_output = switch_send_command(port, "show mac address-table");
                        let ports = parse_switch_ports(&iface_output, &mac_output);
                        HwResponse::SwitchPortMap(ports)
                    }
                    None => HwResponse::SwitchError("Switch not connected".into()),
                }
            }

            HwCommand::SwitchSendCommand(cmd) => {
                match switch_port.as_mut() {
                    Some(port) => {
                        let output = switch_send_command(port, &cmd);
                        HwResponse::SwitchCommandResult(output)
                    }
                    None => HwResponse::SwitchError("Switch not connected".into()),
                }
            }

            HwCommand::SwitchSetVlan { port: port_name, vlan } => {
                match switch_port.as_mut() {
                    Some(port) => {
                        switch_send_command(port, "configure terminal");
                        switch_send_command(port, &format!("interface {}", port_name));
                        switch_send_command(port, &format!("switchport access vlan {}", vlan));
                        switch_send_command(port, "end");
                        HwResponse::SwitchCommandResult(format!("{} -> VLAN {}", port_name, vlan))
                    }
                    None => HwResponse::SwitchError("Switch not connected".into()),
                }
            }

            HwCommand::SwitchSetDescription { port: port_name, desc } => {
                match switch_port.as_mut() {
                    Some(port) => {
                        switch_send_command(port, "configure terminal");
                        switch_send_command(port, &format!("interface {}", port_name));
                        switch_send_command(port, &format!("description {}", desc));
                        switch_send_command(port, "end");
                        HwResponse::SwitchCommandResult(format!("{} desc: {}", port_name, desc))
                    }
                    None => HwResponse::SwitchError("Switch not connected".into()),
                }
            }

            HwCommand::SwitchShutdown { port: port_name, shutdown } => {
                match switch_port.as_mut() {
                    Some(port) => {
                        switch_send_command(port, "configure terminal");
                        switch_send_command(port, &format!("interface {}", port_name));
                        let cmd = if shutdown { "shutdown" } else { "no shutdown" };
                        switch_send_command(port, cmd);
                        switch_send_command(port, "end");
                        let action = if shutdown { "shut down" } else { "enabled" };
                        HwResponse::SwitchCommandResult(format!("{} {}", port_name, action))
                    }
                    None => HwResponse::SwitchError("Switch not connected".into()),
                }
            }

            // Catch-all for unmatched Ip-addressed FBC commands
            _ => HwResponse::Error("Unsupported command for board type".into()),
        };

        if rsp_tx.send(rsp).await.is_err() {
            break; // UI closed
        }
    }
}

/// Helper to run an FBC command, handling the "not connected" case
fn fbc_cmd<F>(client: &mut Option<FbcClient>, f: F) -> HwResponse
where
    F: FnOnce(&mut FbcClient) -> Result<HwResponse, fbc_host::FbcError>,
{
    match client.as_mut() {
        Some(c) => match f(c) {
            Ok(rsp) => rsp,
            Err(e) => HwResponse::Error(format!("{}", e)),
        },
        None => HwResponse::Error("Not connected".into()),
    }
}

// ==================================================================
// Cisco Switch Serial Helpers
// ==================================================================

/// Read from serial port until we see a prompt (# or >) or timeout
fn switch_read_until_prompt(port: &mut Box<dyn serialport::SerialPort>) -> String {
    let mut buf = [0u8; 4096];
    let mut output = String::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);

    while std::time::Instant::now() < deadline {
        match port.read(&mut buf) {
            Ok(n) if n > 0 => {
                let chunk = String::from_utf8_lossy(&buf[..n]);
                output.push_str(&chunk);
                // Check for prompt
                let trimmed = output.trim_end();
                if trimmed.ends_with('#') || trimmed.ends_with('>') {
                    break;
                }
            }
            _ => {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    // Extract hostname from prompt (last line before # or >)
    output.lines()
        .last()
        .unwrap_or("")
        .trim_end_matches('#')
        .trim_end_matches('>')
        .trim_end_matches("(config)")
        .trim_end_matches("(config-if)")
        .trim()
        .to_string()
}

/// Send a command to the switch and read the response
fn switch_send_command(port: &mut Box<dyn serialport::SerialPort>, cmd: &str) -> String {
    let _ = port.write_all(format!("{}\r\n", cmd).as_bytes());
    let _ = port.flush();
    std::thread::sleep(Duration::from_millis(200));

    let mut buf = [0u8; 8192];
    let mut output = String::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);

    while std::time::Instant::now() < deadline {
        match port.read(&mut buf) {
            Ok(n) if n > 0 => {
                let chunk = String::from_utf8_lossy(&buf[..n]);
                output.push_str(&chunk);
                // If we see "--More--", send space to continue
                if output.contains("--More--") {
                    let _ = port.write_all(b" ");
                    let _ = port.flush();
                    output = output.replace("--More--", "");
                    continue;
                }
                let trimmed = output.trim_end();
                if trimmed.ends_with('#') || trimmed.ends_with('>') {
                    break;
                }
            }
            _ => {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    output
}

/// Parse `show interfaces status` + `show mac address-table` into SwitchPort entries
fn parse_switch_ports(iface_output: &str, mac_output: &str) -> Vec<SwitchPort> {
    // Build MAC→port mapping from mac address-table
    // Format: "   1    xxxx.xxxx.xxxx    DYNAMIC     Gi0/1"
    let mut mac_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for line in mac_output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            // Look for lines with MAC format (xxxx.xxxx.xxxx)
            if parts[1].contains('.') && parts[1].len() == 14 {
                let mac = parts[1].to_string();
                let port_name = parts[parts.len() - 1].to_string();
                mac_map.insert(port_name, mac);
            }
        }
    }

    // Parse interface status lines
    // Format: "Gi0/1     description  connected    1          a-full  a-100  10/100/1000BaseTX"
    let mut ports = Vec::new();
    let mut in_table = false;

    for line in iface_output.lines() {
        let trimmed = line.trim();
        // Detect header line
        if trimmed.starts_with("Port") && trimmed.contains("Status") {
            in_table = true;
            continue;
        }
        if trimmed.starts_with("---") {
            continue;
        }
        if !in_table {
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }

        // Parse fixed-width columns (Cisco IOS format)
        // Port: first token (e.g. Gi0/1, Fa0/12)
        let parts: Vec<&str> = trimmed.splitn(2, char::is_whitespace).collect();
        if parts.is_empty() {
            continue;
        }
        let port_name = parts[0].to_string();

        // Rest has: [description] status vlan duplex speed type
        // Status is one of: connected, notconnect, disabled, err-disabled
        let rest = if parts.len() > 1 { parts[1].trim() } else { "" };

        let (description, status, vlan, duplex, speed) = parse_iface_line(rest);

        let mac_address = mac_map.get(&port_name).cloned().unwrap_or_default();

        ports.push(SwitchPort {
            port: port_name,
            description,
            status,
            vlan,
            speed,
            duplex,
            mac_address,
            board_id: None,
        });
    }

    ports
}

/// Parse the rest of a `show interfaces status` line after the port name.
/// Returns (description, status, vlan, duplex, speed)
fn parse_iface_line(rest: &str) -> (String, String, String, String, String) {
    // Try to find status keyword to split description from structured fields
    let status_keywords = ["connected", "notconnect", "disabled", "err-disabled", "sfpAbsent"];

    for keyword in &status_keywords {
        if let Some(pos) = rest.find(keyword) {
            let description = rest[..pos].trim().to_string();
            let after_status = &rest[pos + keyword.len()..];
            let fields: Vec<&str> = after_status.split_whitespace().collect();
            let vlan = fields.first().unwrap_or(&"1").to_string();
            let duplex = fields.get(1).unwrap_or(&"---").to_string();
            let speed = fields.get(2).unwrap_or(&"---").to_string();
            return (description, keyword.to_string(), vlan, duplex, speed);
        }
    }

    // Fallback: no status keyword found
    (String::new(), "unknown".into(), "1".into(), "---".into(), "---".into())
}
