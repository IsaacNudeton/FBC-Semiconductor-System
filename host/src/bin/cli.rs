//! FBC CLI - Unified Command Line Interface
//!
//! Controls both FBC (raw Ethernet) and Sonoma (SSH) boards.
//!
//! # Examples
//!
//! ```bash
//! # FBC board commands
//! fbc-cli fbc discover
//! fbc-cli fbc status all
//! fbc-cli fbc fastpins 00:0A:35:AD:00:02
//! fbc-cli fbc analog all
//! fbc-cli fbc vicor all
//! fbc-cli fbc run all --vectors test.fbc --wait
//!
//! # Sonoma board commands
//! fbc-cli sonoma status 172.16.0.10
//! fbc-cli sonoma analog 172.16.0.10
//! fbc-cli sonoma vicor 172.16.0.10 --init 1 1.02
//! fbc-cli sonoma pmbus 172.16.0.10 --set 1 1.8
//! fbc-cli sonoma run 172.16.0.10 --seq vectors/test.seq --time 60
//!
//! # JSON output for scripting
//! fbc-cli --json fbc status all
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use clap::{Parser, Subcommand};
use fbc_host::{FbcClient, format_mac, parse_mac};
use fbc_host::types::*;
use fbc_host::sonoma::SonomaClient;

#[derive(Parser)]
#[command(name = "fbc-cli")]
#[command(about = "FBC Semiconductor System — Unified CLI for FBC + Sonoma boards")]
#[command(version = "2.0.0")]
struct Cli {
    /// Output as JSON (for scripting)
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// FBC board commands (raw Ethernet 0x88B5)
    Fbc {
        /// Network interface name (partial match supported)
        #[arg(short, long, default_value = "Ethernet")]
        interface: String,

        #[command(subcommand)]
        cmd: FbcCommands,
    },

    /// Sonoma board commands (SSH + ELF binaries)
    Sonoma {
        /// SSH username
        #[arg(short, long, default_value = "root")]
        user: String,

        /// SSH password (empty = authenticate_none)
        #[arg(short, long, default_value = "")]
        password: String,

        #[command(subcommand)]
        cmd: SonomaCommands,
    },

    /// List available network interfaces
    Interfaces,

    /// Show system profile for a given system type
    Profile {
        /// System type: fbc, sonoma, hx, xp160, mcc, shasta
        system: String,
    },
}

// =============================================================================
// FBC Subcommands
// =============================================================================

#[derive(Subcommand)]
enum FbcCommands {
    /// Discover all FBC boards on the network
    Discover {
        /// Discovery timeout in seconds
        #[arg(short, long, default_value = "2")]
        timeout: u64,
    },

    /// Ping a board
    Ping {
        /// Board MAC address
        mac: String,
    },

    /// Get board status
    Status {
        /// Board MAC address (or "all" for all discovered boards)
        #[arg(default_value = "all")]
        target: String,
    },

    /// Read fast pin state (gpio[128:159])
    Fastpins {
        /// Board MAC address (or "all")
        #[arg(default_value = "all")]
        target: String,
    },

    /// Set fast pin output
    SetFastpins {
        /// Board MAC address
        mac: String,
        /// Output drive value (hex, e.g. 0xFF)
        #[arg(long)]
        dout: String,
        /// Output enable value (hex, e.g. 0xFF)
        #[arg(long)]
        oen: String,
    },

    /// Read analog channels (32 ADC readings)
    Analog {
        /// Board MAC address (or "all")
        #[arg(default_value = "all")]
        target: String,
    },

    /// Get VICOR core power status
    Vicor {
        /// Board MAC address (or "all")
        #[arg(default_value = "all")]
        target: String,
    },

    /// Enable/disable VICOR cores
    VicorEnable {
        /// Board MAC address
        mac: String,
        /// Core bitmask (e.g. 0x3F for all 6)
        mask: String,
    },

    /// Set VICOR core voltage
    VicorVoltage {
        /// Board MAC address
        mac: String,
        /// Core number (0-5)
        core: u8,
        /// Voltage in millivolts
        mv: u16,
    },

    /// Emergency stop — kill all power immediately
    EmergencyStop {
        /// Board MAC address (or "all")
        #[arg(default_value = "all")]
        target: String,
    },

    /// Power sequence on (set 6 VICOR voltages)
    PowerOn {
        /// Board MAC address
        mac: String,
        /// 6 voltages in mV, comma-separated (e.g. 1020,825,850,1800,1200,3300)
        voltages: String,
    },

    /// Power sequence off
    PowerOff {
        /// Board MAC address (or "all")
        #[arg(default_value = "all")]
        target: String,
    },

    /// Read EEPROM
    Eeprom {
        /// Board MAC address
        mac: String,
        /// Start offset
        #[arg(long, default_value = "0")]
        offset: u8,
        /// Bytes to read
        #[arg(long, default_value = "64")]
        len: u8,
    },

    /// Get vector engine status
    VectorStatus {
        /// Board MAC address (or "all")
        #[arg(default_value = "all")]
        target: String,
    },

    /// Get error log
    Errors {
        /// Board MAC address
        mac: String,
        /// Max entries to retrieve
        #[arg(long, default_value = "8")]
        count: u8,
    },

    /// Get firmware info
    FirmwareInfo {
        /// Board MAC address
        mac: String,
    },

    /// Upload vectors to a board
    Upload {
        /// Board MAC address
        mac: String,
        /// FBC vector file
        file: String,
    },

    /// Upload and run vectors on boards
    Run {
        /// Target boards: MAC address, "all", or comma-separated MACs
        #[arg(default_value = "all")]
        targets: String,

        /// FBC vector file to upload
        #[arg(short, long)]
        vectors: PathBuf,

        /// Wait for completion (or error)
        #[arg(short, long)]
        wait: bool,

        /// Timeout for --wait in seconds (0 = no timeout)
        #[arg(long, default_value = "0")]
        timeout: u64,
    },

    /// Stop execution on boards
    Stop {
        /// Target boards: MAC address, "all", or comma-separated MACs
        #[arg(default_value = "all")]
        targets: String,
    },

    /// Configure board (clock divider)
    Configure {
        /// Board MAC address
        mac: String,
        /// Clock divider (0=5MHz, 1=10MHz, 2=25MHz, 3=50MHz, 4=100MHz)
        #[arg(long)]
        clock: u8,
    },

    /// Pause vector execution
    Pause {
        /// Board MAC address
        mac: String,
    },

    /// Resume vector execution
    Resume {
        /// Board MAC address
        mac: String,
    },

    /// Get PMBus power status
    PmbusStatus {
        /// Board MAC or 'all'
        target: Option<String>,
    },

    /// Enable/disable PMBus supply
    PmbusEnable {
        /// Board MAC address
        mac: String,
        /// PMBus address (decimal or 0x hex)
        addr: String,
        /// Enable (true/false or 1/0)
        enable: String,
    },

    /// Write EEPROM data
    EepromWrite {
        /// Board MAC address
        mac: String,
        /// EEPROM offset (0-255)
        #[arg(long, help = "EEPROM offset (0-255)")]
        offset: u8,
        /// Hex data to write (e.g. 'DEADBEEF')
        #[arg(long, help = "Hex data to write (e.g. 'DEADBEEF')")]
        data: String,
    },

    /// Update firmware
    FirmwareUpdate {
        /// Board MAC address
        mac: String,
        /// Firmware binary file path
        #[arg(short, long, help = "Firmware binary file path")]
        file: PathBuf,
    },

    /// Get flight recorder log info
    LogInfo {
        /// Board MAC address
        mac: String,
    },

    /// Read flight recorder sector
    ReadLog {
        /// Board MAC address
        mac: String,
        /// SD card sector number
        #[arg(long, help = "SD card sector number")]
        sector: u32,
    },

    /// Monitor running boards with live updates
    Monitor {
        /// Refresh interval in milliseconds
        #[arg(short, long, default_value = "500")]
        interval: u64,

        /// Exit when all boards are idle/done
        #[arg(short, long)]
        exit_when_done: bool,
    },
}

// =============================================================================
// Sonoma Subcommands
// =============================================================================

#[derive(Subcommand)]
enum SonomaCommands {
    /// Get board status (alive + XADC + ADC32)
    Status {
        /// Board IP address
        ip: String,
    },

    /// Read XADC channels
    Xadc {
        /// Board IP address
        ip: String,
    },

    /// Read external ADC (32 channels)
    Adc {
        /// Board IP address
        ip: String,
        /// Read high bank (channels 16-31)
        #[arg(long)]
        high: bool,
    },

    /// VICOR power control
    Vicor {
        /// Board IP address
        ip: String,
        /// Initialize core (core,voltage) — e.g. "1,1.02"
        #[arg(long)]
        init: Option<String>,
        /// Adjust voltage (core,voltage) — e.g. "1,1.05"
        #[arg(long)]
        set: Option<String>,
        /// Disable core
        #[arg(long)]
        disable: Option<u8>,
    },

    /// PMBus channel control
    Pmbus {
        /// Board IP address
        ip: String,
        /// Set channel voltage (channel,voltage) — e.g. "1,1.8"
        #[arg(long)]
        set: Option<String>,
        /// Turn off channel
        #[arg(long)]
        off: Option<u8>,
    },

    /// Set IO power supply voltages
    IoPs {
        /// Board IP address
        ip: String,
        /// 4 bank voltages: B13,B33,B34,B35 — e.g. "1.4,1.6,1.2,2.0"
        voltages: String,
    },

    /// Emergency stop — disable all power
    EmergencyStop {
        /// Board IP address
        ip: String,
    },

    /// Set PLL frequency
    Clock {
        /// Board IP address
        ip: String,
        /// PLL number (0-3)
        #[arg(long)]
        pll: u8,
        /// Frequency in Hz
        #[arg(long)]
        freq: u32,
        /// Duty cycle (default 50)
        #[arg(long, default_value = "50")]
        duty: u8,
    },

    /// Enable/disable PLLs
    PllOnOff {
        /// Board IP address
        ip: String,
        /// 4 PLL states: 0/1,0/1,0/1,0/1 — e.g. "1,1,1,1"
        states: String,
    },

    /// Set pin type
    PinType {
        /// Board IP address
        ip: String,
        /// Pin number
        pin: u8,
        /// Type (0=BIDI, 1=INPUT, 2=OUTPUT, 3=OC, 4=PULSE, 5=NPULSE, 6=ERR_TRIG, 7=VEC_CLK)
        pin_type: u8,
    },

    /// Set pulse delays
    PulseDelays {
        /// Board IP address
        ip: String,
        /// Pin number
        pin: u8,
        /// Pin type
        ptype: u8,
        /// Rise time
        rise: u32,
        /// Fall time
        fall: u32,
        /// Period
        period: u32,
    },

    /// Load vectors (from board filesystem)
    Load {
        /// Board IP address
        ip: String,
        /// Sequence file path (on board)
        seq: String,
        /// Hex file path (on board)
        hex: String,
    },

    /// Run vectors
    Run {
        /// Board IP address
        ip: String,
        /// Sequence file path (on board)
        #[arg(long)]
        seq: String,
        /// Time to run in seconds
        #[arg(long)]
        time: u32,
        /// Enable debug output
        #[arg(long)]
        debug: bool,
    },

    /// Set external DAC channels
    Dac {
        /// Board IP address
        ip: String,
        /// 10 channel values, comma-separated
        values: String,
    },

    /// Toggle MIO pin
    Mio {
        /// Board IP address
        ip: String,
        /// MIO pin number
        pin: u8,
        /// Value (0 or 1)
        value: u8,
    },

    /// Read/write memory
    Mem {
        /// Board IP address
        ip: String,
        /// Memory address (hex, e.g. 0x40040000)
        addr: String,
        /// Value to write (hex) — omit for read
        #[arg(long)]
        write: Option<String>,
    },

    /// Set temperature setpoint
    Temperature {
        /// Board IP address
        ip: String,
        /// Setpoint in °C
        setpoint: f32,
        /// R25C thermistor value
        #[arg(long, default_value = "10000")]
        r25c: f32,
        /// Cool after test
        #[arg(long)]
        cool_after: bool,
    },

    /// Update firmware (SCP BOOT.BIN + reboot)
    Firmware {
        /// Board IP address
        ip: String,
        /// Path to BOOT.BIN
        file: PathBuf,
    },

    /// Run board init sequence
    Init {
        /// Board IP address
        ip: String,
    },

    /// Execute raw SSH command
    Exec {
        /// Board IP address
        ip: String,
        /// Command to execute
        cmd: String,
    },
}

// =============================================================================
// Helpers
// =============================================================================

fn resolve_targets(client: &mut FbcClient, targets: &str) -> anyhow::Result<Vec<[u8; 6]>> {
    if targets.eq_ignore_ascii_case("all") {
        let boards = client.discover(Duration::from_secs(2))?;
        if boards.is_empty() {
            anyhow::bail!("No boards found on network");
        }
        Ok(boards.into_iter().map(|b| b.mac).collect())
    } else if targets.contains(',') && targets.contains(':') {
        // Comma-separated MAC addresses
        targets
            .split(',')
            .map(|s| {
                parse_mac(s.trim())
                    .ok_or_else(|| anyhow::anyhow!("Invalid MAC address: {}", s))
            })
            .collect()
    } else {
        let mac = parse_mac(targets)
            .ok_or_else(|| anyhow::anyhow!("Invalid MAC address: {}", targets))?;
        Ok(vec![mac])
    }
}

fn parse_hex_arg(s: &str) -> anyhow::Result<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Ok(u32::from_str_radix(hex, 16)?)
    } else {
        Ok(s.parse::<u32>()?)
    }
}

fn print_status_table(rows: &[(String, StatusResponse)]) {
    println!("{:<18} {:>10} {:>12} {:>8} {:>8}",
        "MAC", "State", "Cycles", "Errors", "Temp");
    println!("{}", "-".repeat(60));
    for (mac, s) in rows {
        println!("{:<18} {:>10} {:>12} {:>8} {:>7.1}C",
            mac,
            format!("{}", s.state),
            s.cycles,
            s.errors,
            s.temp_c,
        );
    }
}

fn clear_lines(n: usize) {
    for _ in 0..n {
        print!("\x1b[1A\x1b[2K");
    }
    std::io::stdout().flush().ok();
}

// =============================================================================
// Main
// =============================================================================

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Interfaces => {
            let interfaces = FbcClient::list_interfaces();
            if cli.json {
                let names: Vec<String> = interfaces.iter().map(|s| format!("\"{}\"", s)).collect();
                println!("[{}]", names.join(","));
            } else {
                println!("Available network interfaces:");
                for iface in interfaces {
                    println!("  {}", iface);
                }
            }
        }

        Commands::Profile { system } => {
            let system_type = match system.to_lowercase().as_str() {
                "fbc" => SystemType::Fbc,
                "sonoma" => SystemType::Sonoma,
                "hx" => SystemType::Hx,
                "xp160" | "xp-160" => SystemType::Xp160,
                "mcc" => SystemType::Mcc,
                "shasta" => SystemType::Shasta,
                _ => anyhow::bail!("Unknown system type: {}. Valid: fbc, sonoma, hx, xp160, mcc, shasta", system),
            };
            let profile = system_type.profile();
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&profile).unwrap());
            } else {
                println!("{} System Profile", system_type);
                println!("{}", "=".repeat(40));
                println!("  Profile Name:    {}", system_type.profile_name());
                println!("  Transport:       {:?}", profile.transport);
                println!("  Pattern Format:  {:?}", profile.pattern_format);
                println!("  Total Channels:  {}", profile.total_channels);
                println!("  BIM Channels:    {} (pins {:?})", profile.bim_channels, profile.bim_range());
                println!("  Fast Channels:   {} (pins {:?})", profile.fast_channels, profile.fast_range());
                println!("  VICOR Cores:     {}", profile.vicor_cores);
                println!("  Voltage Limits:");
                println!("    VICOR:  {}-{} mV", profile.voltage_limits.vicor_min_mv, profile.voltage_limits.vicor_max_mv);
                println!("    PMBus:  {}-{} mV", profile.voltage_limits.pmbus_min_mv, profile.voltage_limits.pmbus_max_mv);
            }
        }

        Commands::Fbc { interface, cmd } => {
            run_fbc_command(&interface, cmd, cli.json)?;
        }

        Commands::Sonoma { user, password, cmd } => {
            // Build tokio runtime for async Sonoma commands
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(run_sonoma_command(&user, &password, cmd, cli.json))?;
        }
    }

    Ok(())
}

// =============================================================================
// FBC Command Runner
// =============================================================================

fn run_fbc_command(interface: &str, cmd: FbcCommands, json: bool) -> anyhow::Result<()> {
    match cmd {
        FbcCommands::Discover { timeout } => {
            let mut client = FbcClient::new(interface)?;
            if !json {
                println!("Discovering boards on {} ({}s timeout)...", interface, timeout);
            }
            let boards = client.discover(Duration::from_secs(timeout))?;

            if json {
                println!("{}", serde_json::to_string_pretty(&boards).unwrap());
            } else if boards.is_empty() {
                println!("No boards found.");
            } else {
                println!("Found {} board(s):", boards.len());
                println!("{:<18} {:>10} {:>8} {:>6} {:>5}",
                    "MAC", "Serial", "FW", "BIM", "EEPROM");
                println!("{}", "-".repeat(52));
                for board in boards {
                    println!("{:<18} {:>10} {:>5}.{:<2} {:>6} {:>5}",
                        format_mac(&board.mac),
                        format!("{:08X}", board.serial),
                        board.fw_version >> 8,
                        board.fw_version & 0xFF,
                        if board.has_bim { "yes" } else { "no" },
                        if board.bim_programmed { "yes" } else { "no" },
                    );
                }
            }
        }

        FbcCommands::Ping { mac } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC address"))?;
            let mut client = FbcClient::new(interface)?;
            let latency = client.ping(&mac)?;
            if json {
                println!(r#"{{"mac":"{}","latency_us":{}}}"#,
                    format_mac(&mac), latency.as_micros());
            } else {
                println!("Ping {}: {:.2}ms", format_mac(&mac), latency.as_secs_f64() * 1000.0);
            }
        }

        FbcCommands::Status { target } => {
            let mut client = FbcClient::new(interface)?;
            let targets = resolve_targets(&mut client, &target)?;

            let mut results = Vec::new();
            for mac in &targets {
                match client.get_status(mac) {
                    Ok(status) => results.push((format_mac(mac), status)),
                    Err(e) => {
                        if !json { eprintln!("Error {}: {}", format_mac(mac), e); }
                    }
                }
            }

            if json {
                #[derive(serde::Serialize)]
                struct StatusEntry { mac: String, system_type: SystemType, #[serde(flatten)] status: StatusResponse }
                let entries: Vec<StatusEntry> = results.into_iter()
                    .map(|(mac, s)| StatusEntry { mac, system_type: SystemType::Fbc, status: s })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&entries).unwrap());
            } else {
                print_status_table(&results);
            }
        }

        FbcCommands::Fastpins { target } => {
            let mut client = FbcClient::new(interface)?;
            let targets = resolve_targets(&mut client, &target)?;

            for mac in &targets {
                match client.get_fast_pins(mac) {
                    Ok(fp) => {
                        if json {
                            println!(r#"{{"mac":"{}","din":"0x{:08X}","dout":"0x{:08X}","oen":"0x{:08X}"}}"#,
                                format_mac(mac), fp.din, fp.dout, fp.oen);
                        } else {
                            println!("{}: din=0x{:08X}  dout=0x{:08X}  oen=0x{:08X}",
                                format_mac(mac), fp.din, fp.dout, fp.oen);
                        }
                    }
                    Err(e) => {
                        if !json { eprintln!("Error {}: {}", format_mac(mac), e); }
                    }
                }
            }
        }

        FbcCommands::SetFastpins { mac, dout, oen } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            let dout_val = parse_hex_arg(&dout)?;
            let oen_val = parse_hex_arg(&oen)?;
            let mut client = FbcClient::new(interface)?;
            client.set_fast_pins(&mac, dout_val, oen_val)?;
            if json {
                println!(r#"{{"status":"ok"}}"#);
            } else {
                println!("Fast pins set: dout=0x{:08X} oen=0x{:08X}", dout_val, oen_val);
            }
        }

        FbcCommands::Analog { target } => {
            let mut client = FbcClient::new(interface)?;
            let targets = resolve_targets(&mut client, &target)?;

            for mac in &targets {
                match client.read_analog(mac) {
                    Ok(channels) => {
                        if json {
                            let readings: Vec<String> = channels.xadc.iter().chain(channels.external.iter())
                                .map(|r| format!(r#"{{"ch":{},"raw":{},"mv":{:.1}}}"#, r.channel, r.raw, r.voltage_mv))
                                .collect();
                            println!(r#"{{"mac":"{}","channels":[{}]}}"#, format_mac(mac), readings.join(","));
                        } else {
                            println!("{} — XADC:", format_mac(mac));
                            for r in &channels.xadc {
                                println!("  ch{:>2}: raw={:>5}  {:.1}mV", r.channel, r.raw, r.voltage_mv);
                            }
                            println!("  External ADC:");
                            for r in &channels.external {
                                println!("  ch{:>2}: raw={:>5}  {:.1}mV", r.channel, r.raw, r.voltage_mv);
                            }
                        }
                    }
                    Err(e) => {
                        if !json { eprintln!("Error {}: {}", format_mac(mac), e); }
                    }
                }
            }
        }

        FbcCommands::Vicor { target } => {
            let mut client = FbcClient::new(interface)?;
            let targets = resolve_targets(&mut client, &target)?;

            for mac in &targets {
                match client.get_vicor_status(mac) {
                    Ok(vs) => {
                        if json {
                            let cores: Vec<String> = vs.cores.iter().map(|c| {
                                format!(r#"{{"id":{},"enabled":{},"mv":{},"ma":{}}}"#,
                                    c.id, c.enabled, c.voltage_mv, c.current_ma)
                            }).collect();
                            println!(r#"{{"mac":"{}","cores":[{}]}}"#, format_mac(mac), cores.join(","));
                        } else {
                            println!("{} — VICOR:", format_mac(mac));
                            println!("  {:>4} {:>8} {:>8} {:>8}", "Core", "Enabled", "mV", "mA");
                            for c in &vs.cores {
                                println!("  {:>4} {:>8} {:>8} {:>8}",
                                    c.id, c.enabled, c.voltage_mv, c.current_ma);
                            }
                        }
                    }
                    Err(e) => {
                        if !json { eprintln!("Error {}: {}", format_mac(mac), e); }
                    }
                }
            }
        }

        FbcCommands::VicorEnable { mac, mask } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            let mask_val = parse_hex_arg(&mask)? as u8;
            if mask_val > 63 {
                anyhow::bail!("Mask must be 0-63 (6 bits for 6 cores)");
            }
            let mut client = FbcClient::new(interface)?;
            client.set_vicor_enable(&mac, mask_val)?;
            if json {
                println!(r#"{{"status":"ok","mask":"0x{:02X}"}}"#, mask_val);
            } else {
                println!("VICOR enable mask set to 0x{:02X}", mask_val);
            }
        }

        FbcCommands::VicorVoltage { mac, core, mv } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            let profile = SystemType::Fbc.profile();
            if core >= profile.vicor_cores {
                anyhow::bail!("Core must be 0-{}", profile.vicor_cores - 1);
            }
            if let Err(e) = profile.voltage_limits.validate_vicor(mv) {
                anyhow::bail!("{}", e);
            }
            let mut client = FbcClient::new(interface)?;
            client.set_vicor_voltage(&mac, core, mv)?;
            if json {
                println!(r#"{{"status":"ok","core":{},"mv":{}}}"#, core, mv);
            } else {
                println!("VICOR core {} voltage set to {}mV", core, mv);
            }
        }

        FbcCommands::EmergencyStop { target } => {
            let mut client = FbcClient::new(interface)?;
            let targets = resolve_targets(&mut client, &target)?;
            for mac in &targets {
                match client.emergency_stop(mac) {
                    Ok(()) => {
                        if !json { println!("Emergency stop sent to {}", format_mac(mac)); }
                    }
                    Err(e) => {
                        eprintln!("Error {}: {}", format_mac(mac), e);
                    }
                }
            }
            if json { println!(r#"{{"status":"ok","boards":{}}}"#, targets.len()); }
        }

        FbcCommands::PowerOn { mac, voltages } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            let v: Vec<u16> = voltages.split(',')
                .map(|s| s.trim().parse::<u16>())
                .collect::<Result<Vec<_>, _>>()?;
            if v.len() != 6 {
                anyhow::bail!("Expected 6 voltages, got {}", v.len());
            }
            let profile = SystemType::Fbc.profile();
            for (i, &val) in v.iter().enumerate() {
                if let Err(e) = profile.voltage_limits.validate_vicor(val) {
                    anyhow::bail!("Core {}: {}", i, e);
                }
            }
            let mut voltages_arr = [0u16; 6];
            voltages_arr.copy_from_slice(&v);
            let mut client = FbcClient::new(interface)?;
            client.power_sequence_on(&mac, voltages_arr)?;
            if json {
                println!(r#"{{"status":"ok"}}"#);
            } else {
                println!("Power sequence ON sent");
            }
        }

        FbcCommands::PowerOff { target } => {
            let mut client = FbcClient::new(interface)?;
            let targets = resolve_targets(&mut client, &target)?;
            for mac in &targets {
                client.power_sequence_off(mac)?;
                if !json { println!("Power off sent to {}", format_mac(mac)); }
            }
            if json { println!(r#"{{"status":"ok"}}"#); }
        }

        FbcCommands::Eeprom { mac, offset, len } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            let mut client = FbcClient::new(interface)?;
            let data = client.read_eeprom(&mac, offset, len)?;
            if json {
                let hex: Vec<String> = data.data.iter().map(|b| format!("{:02X}", b)).collect();
                println!(r#"{{"offset":{},"data":"{}"}}"#, data.offset, hex.join(""));
            } else {
                println!("EEPROM @ offset {}:", data.offset);
                for (i, chunk) in data.data.chunks(16).enumerate() {
                    let hex: Vec<String> = chunk.iter().map(|b| format!("{:02X}", b)).collect();
                    let ascii: String = chunk.iter()
                        .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' })
                        .collect();
                    println!("  {:04X}: {}  {}", offset as usize + i * 16, hex.join(" "), ascii);
                }
            }
        }

        FbcCommands::VectorStatus { target } => {
            let mut client = FbcClient::new(interface)?;
            let targets = resolve_targets(&mut client, &target)?;
            for mac in &targets {
                match client.get_vector_status(mac) {
                    Ok(vs) => {
                        if json {
                            println!(r#"{{"mac":"{}","state":"{}","vectors":{},"errors":{},"run_time_ms":{}}}"#,
                                format_mac(mac), vs.state, vs.total_vectors, vs.error_count, vs.run_time_ms);
                        } else {
                            println!("{}: state={} vectors={} loops={}/{} errors={} time={}ms",
                                format_mac(mac), vs.state, vs.total_vectors,
                                vs.loop_count, vs.target_loops,
                                vs.error_count, vs.run_time_ms);
                        }
                    }
                    Err(e) => {
                        if !json { eprintln!("Error {}: {}", format_mac(mac), e); }
                    }
                }
            }
        }

        FbcCommands::Errors { mac, count } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            let mut client = FbcClient::new(interface)?;
            let log = client.get_error_log(&mac, 0, count as u32)?;

            if json {
                let entries: Vec<String> = log.entries.iter().map(|e| {
                    format!(r#"{{"pattern":[{},{},{},{}],"vector":{},"cycle":{}}}"#,
                        e.pattern[0], e.pattern[1], e.pattern[2], e.pattern[3],
                        e.vector, e.cycle)
                }).collect();
                println!(r#"{{"total":{},"entries":[{}]}}"#, log.total_errors, entries.join(","));
            } else {
                println!("Error log ({} total):", log.total_errors);
                for e in &log.entries {
                    println!("  vec={} cycle={} pattern={:08X}_{:08X}_{:08X}_{:08X}",
                        e.vector, e.cycle,
                        e.pattern[3], e.pattern[2], e.pattern[1], e.pattern[0]);
                }
            }
        }

        FbcCommands::FirmwareInfo { mac } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            let mut client = FbcClient::new(interface)?;
            let fw = client.get_firmware_info(&mac)?;

            if json {
                println!(r#"{{"version":"{}.{}.{}","build":"{}","serial":{},"hw_rev":{}}}"#,
                    fw.version_major, fw.version_minor, fw.version_patch,
                    fw.build_date, fw.serial, fw.hw_revision);
            } else {
                println!("Firmware: v{}.{}.{}", fw.version_major, fw.version_minor, fw.version_patch);
                println!("Build:    {}", fw.build_date);
                println!("Serial:   {:08X}", fw.serial);
                println!("HW Rev:   {}", fw.hw_revision);
            }
        }

        FbcCommands::Upload { mac, file } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC address"))?;
            let mut client = FbcClient::new(interface)?;
            let data = std::fs::read(&file)?;
            if !json { println!("Uploading {} ({} bytes)...", file, data.len()); }
            client.upload_vectors(&mac, &data)?;
            if json {
                println!(r#"{{"status":"ok","bytes":{}}}"#, data.len());
            } else {
                println!("Upload complete.");
            }
        }

        FbcCommands::Run { targets, vectors, wait, timeout } => {
            let mut client = FbcClient::new(interface)?;
            let target_macs = resolve_targets(&mut client, &targets)?;
            let data = std::fs::read(&vectors)?;

            if !json {
                println!("Uploading {} ({} bytes) to {} board(s)...",
                    vectors.display(), data.len(), target_macs.len());
            }

            for mac in &target_macs {
                if let Err(e) = client.upload_vectors(mac, &data) {
                    if !json { eprintln!("Upload failed for {}: {}", format_mac(mac), e); }
                }
            }

            if !json { println!("Starting execution..."); }

            let mut started = Vec::new();
            for mac in &target_macs {
                if let Err(e) = client.start(mac) {
                    if !json { eprintln!("Start failed for {}: {}", format_mac(mac), e); }
                } else {
                    started.push(*mac);
                }
            }
            if !json { println!("Started {} board(s).", started.len()); }

            if wait && !started.is_empty() {
                if !json { println!("Waiting for completion..."); }

                let timeout_dur = if timeout > 0 { Some(Duration::from_secs(timeout)) } else { None };
                let start_time = Instant::now();
                let mut completed = Vec::new();

                loop {
                    if let Some(td) = timeout_dur {
                        if start_time.elapsed() > td {
                            if !json { eprintln!("Timeout."); }
                            break;
                        }
                    }

                    let mut still_running = false;
                    for mac in &started {
                        if completed.iter().any(|(m, _): &([u8; 6], _)| m == mac) {
                            continue;
                        }
                        if let Ok(status) = client.get_status(mac) {
                            match status.state {
                                ControllerState::Done | ControllerState::Error | ControllerState::Idle => {
                                    completed.push((*mac, status));
                                }
                                _ => still_running = true,
                            }
                        }
                    }

                    if !still_running || completed.len() == started.len() { break; }
                    std::thread::sleep(Duration::from_millis(100));
                }

                let results: Vec<_> = completed.iter()
                    .map(|(mac, status)| (format_mac(mac), status.clone()))
                    .collect();

                if json {
                    let entries: Vec<String> = results.iter().map(|(mac, s)| {
                        format!(r#"{{"mac":"{}","state":"{}","cycles":{},"errors":{}}}"#,
                            mac, s.state, s.cycles, s.errors)
                    }).collect();
                    println!("[{}]", entries.join(","));
                } else {
                    println!("\nCompleted in {:.2}s:", start_time.elapsed().as_secs_f64());
                    print_status_table(&results);
                }
            } else if json {
                println!(r#"{{"status":"started","boards":{}}}"#, started.len());
            }
        }

        FbcCommands::Stop { targets } => {
            let mut client = FbcClient::new(interface)?;
            let target_macs = resolve_targets(&mut client, &targets)?;
            let mut stopped = 0;
            for mac in &target_macs {
                if client.stop(mac).is_ok() {
                    stopped += 1;
                    if !json { println!("Stopped {}", format_mac(mac)); }
                }
            }
            if json { println!(r#"{{"status":"ok","stopped":{}}}"#, stopped); }
            else { println!("Stopped {} board(s).", stopped); }
        }

        FbcCommands::Configure { mac, clock } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            let mut client = FbcClient::new(interface)?;
            client.configure(&mac, clock, [0u16; 6])?;
            if json {
                println!(r#"{{"status":"ok","clock_div":{}}}"#, clock);
            } else {
                println!("Configured: clock_div={}", clock);
            }
        }

        FbcCommands::Pause { mac } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            let mut client = FbcClient::new(interface)?;
            client.pause_vectors(&mac)?;
            if json {
                println!(r#"{{"status":"ok","mac":"{}"}}"#, format_mac(&mac));
            } else {
                println!("Vectors paused on {}", format_mac(&mac));
            }
        }

        FbcCommands::Resume { mac } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            let mut client = FbcClient::new(interface)?;
            client.resume_vectors(&mac)?;
            if json {
                println!(r#"{{"status":"ok","mac":"{}"}}"#, format_mac(&mac));
            } else {
                println!("Vectors resumed on {}", format_mac(&mac));
            }
        }

        FbcCommands::PmbusStatus { target } => {
            let mut client = FbcClient::new(interface)?;
            let targets = if let Some(t) = target {
                resolve_targets(&mut client, &t)?
            } else {
                // Default to all discovered boards
                let boards = client.discover(Duration::from_secs(2))?;
                boards.into_iter().map(|b| b.mac).collect()
            };

            for mac in &targets {
                match client.get_pmbus_status(mac) {
                    Ok(status) => {
                        if json {
                            let rails: Vec<String> = status.rails.iter().map(|r| {
                                format!(r#"{{"addr":"0x{:02X}","enabled":{},"mv":{},"ma":{}}}"#,
                                    r.address, r.enabled, r.voltage_mv, r.current_ma)
                            }).collect();
                            println!(r#"{{"mac":"{}","rails":[{}]}}"#, format_mac(mac), rails.join(","));
                        } else {
                            println!("{} — PMBus:", format_mac(mac));
                            println!("  {:>6} {:>8} {:>10} {:>10}", "Addr", "Enabled", "mV", "mA");
                            for r in &status.rails {
                                println!("  0x{:02X} {:>8} {:>10} {:>10}",
                                    r.address, r.enabled, r.voltage_mv, r.current_ma);
                            }
                        }
                    }
                    Err(e) => {
                        if !json { eprintln!("Error {}: {}", format_mac(mac), e); }
                    }
                }
            }
        }

        FbcCommands::PmbusEnable { mac, addr, enable } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            // Parse address (support 0x prefix for hex)
            let addr_val = if let Some(hex) = addr.strip_prefix("0x").or_else(|| addr.strip_prefix("0X")) {
                u8::from_str_radix(hex, 16)?
            } else {
                addr.parse::<u8>()?
            };
            // Parse enable (support true/false or 1/0)
            let enable_val = enable.to_lowercase();
            let enable_bool = match enable_val.as_str() {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => anyhow::bail!("Enable must be true/false or 1/0"),
            };
            let mut client = FbcClient::new(interface)?;
            client.set_pmbus_enable(&mac, addr_val, enable_bool)?;
            if json {
                println!(r#"{{"status":"ok","addr":"0x{:02X}","enabled":{}}}"#, addr_val, enable_bool);
            } else {
                println!("PMBus 0x{:02X} {}", addr_val, if enable_bool { "enabled" } else { "disabled" });
            }
        }

        FbcCommands::EepromWrite { mac, offset, data } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            // Parse hex string to Vec<u8>
            let data_str = data.strip_prefix("0x").unwrap_or(&data);
            let mut data_bytes = Vec::with_capacity(data_str.len() / 2);
            for i in (0..data_str.len()).step_by(2) {
                let byte_str = &data_str[i..i+2];
                let byte = u8::from_str_radix(byte_str, 16)
                    .map_err(|_| anyhow::anyhow!("Invalid hex data at position {}", i))?;
                data_bytes.push(byte);
            }
            let mut client = FbcClient::new(interface)?;
            client.write_eeprom(&mac, offset, &data_bytes)?;
            if json {
                println!(r#"{{"status":"ok","offset":{},"bytes":{}}}"#, offset, data_bytes.len());
            } else {
                println!("Wrote {} bytes to EEPROM at offset 0x{:02X}", data_bytes.len(), offset);
            }
        }

        FbcCommands::FirmwareUpdate { mac, file } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            // Read firmware file
            let data = std::fs::read(&file)
                .map_err(|e| anyhow::anyhow!("Failed to read firmware file: {}", e))?;
            // Compute CRC32
            let checksum = crc32fast::hash(&data);
            let mut client = FbcClient::new(interface)?;
            if !json {
                println!("Updating firmware on {} ({} bytes)...", format_mac(&mac), data.len());
            }
            let result = client.firmware_update(&mac, &data, checksum)?;
            if json {
                println!(r#"{{"status":"ok","bytes":{},"checksum":"0x{:08X}"}}"#, data.len(), checksum);
            } else {
                println!("Firmware update complete ({} bytes, checksum 0x{:08X})", data.len(), checksum);
                if result.status == 0 {
                    println!("Board will reboot with new firmware.");
                } else {
                    println!("Warning: Update completed but board reported status {}", result.status);
                }
            }
        }

        FbcCommands::LogInfo { mac } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            let mut client = FbcClient::new(interface)?;
            let info = client.get_log_info(&mac)?;
            if json {
                println!(r#"{{"mac":"{}","sd_present":{},"boot_sector":{},"log_start":{},"log_end":{},"current_index":{},"total_entries":{}}}"#,
                    format_mac(&mac), info.sd_present, info.boot_sector, info.log_start,
                    info.log_end, info.current_index, info.total_entries);
            } else {
                println!("{} — Flight Recorder:", format_mac(&mac));
                println!("  SD Present:    {}", if info.sd_present { "yes" } else { "no" });
                println!("  Boot Sector:   {}", info.boot_sector);
                println!("  Log Start:     {}", info.log_start);
                println!("  Log End:       {}", info.log_end);
                println!("  Current Index: {}", info.current_index);
                println!("  Total Entries: {}", info.total_entries);
            }
        }

        FbcCommands::ReadLog { mac, sector } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC"))?;
            let mut client = FbcClient::new(interface)?;
            let log = client.read_log_sector(&mac, sector)?;
            if json {
                let data_hex = log.data.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join("");
                println!(r#"{{"sector":{},"status":{},"data":"{}"}}"#, log.sector, log.status, data_hex);
            } else {
                println!("Sector {} (status={}):", log.sector, log.status);
                // Hex dump with ASCII sidebar
                for (i, chunk) in log.data.chunks(16).enumerate() {
                    let hex: Vec<String> = chunk.iter().map(|b| format!("{:02X}", b)).collect();
                    let ascii: String = chunk.iter()
                        .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' })
                        .collect();
                    println!("  {:04X}: {}  {}",
                        sector as usize * 512 + i * 16,
                        hex.join(" "),
                        ascii);
                }
            }
        }

        FbcCommands::Monitor { interval, exit_when_done } => {
            let mut client = FbcClient::new(interface)?;
            if !json { println!("Discovering boards..."); }
            let boards = client.discover(Duration::from_secs(2))?;
            if boards.is_empty() {
                if json { println!("[]"); } else { println!("No boards found."); }
                return Ok(());
            }

            let macs: Vec<[u8; 6]> = boards.iter().map(|b| b.mac).collect();
            if !json { println!("Monitoring {} board(s). Press Ctrl+C to exit.\n", macs.len()); }

            let mut first_print = true;
            let header_lines = 3;

            loop {
                let mut results = Vec::new();
                let mut all_done = true;

                for mac in &macs {
                    if let Ok(status) = client.get_status(mac) {
                        if matches!(status.state, ControllerState::Running) {
                            all_done = false;
                        }
                        results.push((format_mac(mac), status));
                    }
                }

                if json {
                    let entries: Vec<String> = results.iter().map(|(mac, s)| {
                        format!(r#"{{"mac":"{}","state":"{}","cycles":{},"errors":{}}}"#,
                            mac, s.state, s.cycles, s.errors)
                    }).collect();
                    println!("[{}]", entries.join(","));
                    if exit_when_done && all_done { break; }
                } else {
                    if !first_print { clear_lines(header_lines + results.len()); }
                    first_print = false;
                    print_status_table(&results);
                    if exit_when_done && all_done {
                        println!("\nAll boards completed.");
                        break;
                    }
                }

                std::thread::sleep(Duration::from_millis(interval));
            }
        }
    }

    Ok(())
}

// =============================================================================
// Sonoma Command Runner
// =============================================================================

async fn run_sonoma_command(
    user: &str,
    password: &str,
    cmd: SonomaCommands,
    json: bool,
) -> anyhow::Result<()> {
    match cmd {
        SonomaCommands::Status { ip } => {
            let client = SonomaClient::new(&ip, user, password);
            let status = client.get_status().await
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            if json {
                println!(r#"{{"alive":{},"ip":"{}","fw":"{}","xadc_channels":{},"adc_channels":{}}}"#,
                    status.alive, status.ip, status.fw_version,
                    status.xadc.len(), status.adc32.len());
            } else {
                println!("Sonoma @ {}", ip);
                println!("  Alive:    {}", status.alive);
                println!("  Firmware: {}", status.fw_version);
                println!("  XADC:     {} channels", status.xadc.len());
                println!("  ADC32:    {} channels", status.adc32.len());
            }
        }

        SonomaCommands::Xadc { ip } => {
            let client = SonomaClient::new(&ip, user, password);
            let readings = client.read_xadc().await
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            if json {
                let entries: Vec<String> = readings.iter()
                    .map(|r| format!(r#"{{"ch":{},"raw":{},"mv":{:.1}}}"#, r.channel, r.raw, r.voltage_mv))
                    .collect();
                println!("[{}]", entries.join(","));
            } else {
                println!("XADC readings:");
                for r in &readings {
                    println!("  ch{:>2}: raw={:>5}  {:.1}mV", r.channel, r.raw, r.voltage_mv);
                }
            }
        }

        SonomaCommands::Adc { ip, high } => {
            let client = SonomaClient::new(&ip, user, password);
            let readings = if high {
                client.read_adc32_high().await
            } else {
                client.read_adc32().await
            }.map_err(|e| anyhow::anyhow!("{}", e))?;

            if json {
                let entries: Vec<String> = readings.iter()
                    .map(|r| format!(r#"{{"ch":{},"raw":{},"mv":{:.1}}}"#, r.channel, r.raw, r.voltage_mv))
                    .collect();
                println!("[{}]", entries.join(","));
            } else {
                println!("ADC32 readings{}:", if high { " (high bank)" } else { "" });
                for r in &readings {
                    println!("  ch{:>2}: raw={:>5}  {:.1}mV", r.channel, r.raw, r.voltage_mv);
                }
            }
        }

        SonomaCommands::Vicor { ip, init, set, disable } => {
            let client = SonomaClient::new(&ip, user, password);

            if let Some(init_str) = init {
                let parts: Vec<&str> = init_str.split(',').collect();
                if parts.len() != 2 { anyhow::bail!("Expected core,voltage (e.g. 1,1.02)"); }
                let core: u8 = parts[0].parse()?;
                let voltage: f32 = parts[1].parse()?;
                client.vicor_init(core, voltage).await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                if json { println!(r#"{{"status":"ok","action":"init","core":{},"voltage":{}}}"#, core, voltage); }
                else { println!("VICOR core {} initialized at {}V", core, voltage); }
            } else if let Some(set_str) = set {
                let parts: Vec<&str> = set_str.split(',').collect();
                if parts.len() != 2 { anyhow::bail!("Expected core,voltage (e.g. 1,1.05)"); }
                let core: u8 = parts[0].parse()?;
                let voltage: f32 = parts[1].parse()?;
                client.vicor_voltage(core, voltage).await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                if json { println!(r#"{{"status":"ok","action":"set","core":{},"voltage":{}}}"#, core, voltage); }
                else { println!("VICOR core {} voltage set to {}V", core, voltage); }
            } else if let Some(core) = disable {
                client.vicor_disable(core).await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                if json { println!(r#"{{"status":"ok","action":"disable","core":{}}}"#, core); }
                else { println!("VICOR core {} disabled", core); }
            } else {
                if !json { println!("Use --init, --set, or --disable"); }
            }
        }

        SonomaCommands::Pmbus { ip, set, off } => {
            let client = SonomaClient::new(&ip, user, password);

            if let Some(set_str) = set {
                let parts: Vec<&str> = set_str.split(',').collect();
                if parts.len() != 2 { anyhow::bail!("Expected channel,voltage (e.g. 1,1.8)"); }
                let ch: u8 = parts[0].parse()?;
                let voltage: f32 = parts[1].parse()?;
                client.pmbus_set(ch, voltage).await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                if json { println!(r#"{{"status":"ok","channel":{},"voltage":{}}}"#, ch, voltage); }
                else { println!("PMBus ch{} set to {}V", ch, voltage); }
            } else if let Some(ch) = off {
                client.pmbus_off(ch).await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                if json { println!(r#"{{"status":"ok","channel":{},"action":"off"}}"#, ch); }
                else { println!("PMBus ch{} turned off", ch); }
            } else {
                if !json { println!("Use --set or --off"); }
            }
        }

        SonomaCommands::IoPs { ip, voltages } => {
            let client = SonomaClient::new(&ip, user, password);
            let v: Vec<f32> = voltages.split(',')
                .map(|s| s.trim().parse::<f32>())
                .collect::<Result<Vec<_>, _>>()?;
            if v.len() != 4 { anyhow::bail!("Expected 4 voltages (B13,B33,B34,B35)"); }
            client.io_ps(v[0], v[1], v[2], v[3]).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json { println!(r#"{{"status":"ok"}}"#); }
            else { println!("IO PS set: B13={} B33={} B34={} B35={}", v[0], v[1], v[2], v[3]); }
        }

        SonomaCommands::EmergencyStop { ip } => {
            let client = SonomaClient::new(&ip, user, password);
            client.emergency_stop().await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json { println!(r#"{{"status":"ok","action":"emergency_stop"}}"#); }
            else { println!("Emergency stop sent to {}", ip); }
        }

        SonomaCommands::Clock { ip, pll, freq, duty } => {
            let client = SonomaClient::new(&ip, user, password);
            client.set_frequency(pll, freq, duty).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json { println!(r#"{{"status":"ok","pll":{},"freq":{},"duty":{}}}"#, pll, freq, duty); }
            else { println!("PLL{} set to {}Hz (duty {}%)", pll, freq, duty); }
        }

        SonomaCommands::PllOnOff { ip, states } => {
            let client = SonomaClient::new(&ip, user, password);
            let v: Vec<bool> = states.split(',')
                .map(|s| s.trim() == "1")
                .collect();
            if v.len() != 4 { anyhow::bail!("Expected 4 PLL states (0/1,0/1,0/1,0/1)"); }
            client.pll_on_off([v[0], v[1], v[2], v[3]]).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json { println!(r#"{{"status":"ok"}}"#); }
            else { println!("PLLs: {} {} {} {}", v[0] as u8, v[1] as u8, v[2] as u8, v[3] as u8); }
        }

        SonomaCommands::PinType { ip, pin, pin_type } => {
            let client = SonomaClient::new(&ip, user, password);
            client.set_pin_type(pin, pin_type).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json { println!(r#"{{"status":"ok","pin":{},"type":{}}}"#, pin, pin_type); }
            else { println!("Pin {} type set to {}", pin, pin_type); }
        }

        SonomaCommands::PulseDelays { ip, pin, ptype, rise, fall, period } => {
            let client = SonomaClient::new(&ip, user, password);
            client.set_pulse_delays(pin, ptype, rise, fall, period).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json { println!(r#"{{"status":"ok"}}"#); }
            else { println!("Pin {} pulse delays: rise={} fall={} period={}", pin, rise, fall, period); }
        }

        SonomaCommands::Load { ip, seq, hex } => {
            let client = SonomaClient::new(&ip, user, password);
            client.load_vectors(&seq, &hex).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json { println!(r#"{{"status":"ok"}}"#); }
            else { println!("Vectors loaded: {} + {}", seq, hex); }
        }

        SonomaCommands::Run { ip, seq, time, debug } => {
            let client = SonomaClient::new(&ip, user, password);
            let result = client.run_vectors(&seq, time, debug).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json {
                println!(r#"{{"passed":{},"vectors":{},"errors":{},"duration_s":{:.1}}}"#,
                    result.passed, result.vectors_executed, result.errors, result.duration_s);
            } else {
                println!("Result: {}", if result.passed { "PASS" } else { "FAIL" });
                println!("  Errors: {}", result.errors);
                println!("  Duration: {:.1}s", result.duration_s);
            }
        }

        SonomaCommands::Dac { ip, values } => {
            let client = SonomaClient::new(&ip, user, password);
            let v: Vec<f32> = values.split(',')
                .map(|s| s.trim().parse::<f32>())
                .collect::<Result<Vec<_>, _>>()?;
            if v.len() != 10 { anyhow::bail!("Expected 10 DAC values"); }
            let mut arr = [0.0f32; 10];
            arr.copy_from_slice(&v);
            client.set_ext_dac(&arr).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json { println!(r#"{{"status":"ok"}}"#); }
            else { println!("External DAC channels set"); }
        }

        SonomaCommands::Mio { ip, pin, value } => {
            let client = SonomaClient::new(&ip, user, password);
            client.toggle_mio(pin, value).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json { println!(r#"{{"status":"ok","mio":{},"value":{}}}"#, pin, value); }
            else { println!("MIO {} = {}", pin, value); }
        }

        SonomaCommands::Mem { ip, addr, write } => {
            let client = SonomaClient::new(&ip, user, password);
            let addr_val = parse_hex_arg(&addr)?;

            if let Some(val_str) = write {
                let val = parse_hex_arg(&val_str)?;
                client.write_mem(addr_val, val).await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                if json { println!(r#"{{"status":"ok","addr":"0x{:08X}","value":"0x{:08X}"}}"#, addr_val, val); }
                else { println!("[0x{:08X}] ← 0x{:08X}", addr_val, val); }
            } else {
                let val = client.read_mem(addr_val).await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                if json { println!(r#"{{"addr":"0x{:08X}","value":"0x{:08X}"}}"#, addr_val, val); }
                else { println!("[0x{:08X}] = 0x{:08X}", addr_val, val); }
            }
        }

        SonomaCommands::Temperature { ip, setpoint, r25c, cool_after } => {
            let client = SonomaClient::new(&ip, user, password);
            client.set_temperature(setpoint, r25c, cool_after).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json { println!(r#"{{"status":"ok","setpoint":{}}}"#, setpoint); }
            else { println!("Temperature setpoint: {}°C", setpoint); }
        }

        SonomaCommands::Firmware { ip, file } => {
            let client = SonomaClient::new(&ip, user, password);
            if !json { println!("Uploading firmware to {}...", ip); }
            client.update_firmware(&file).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json { println!(r#"{{"status":"ok","action":"firmware_update"}}"#); }
            else { println!("Firmware updated. Board rebooting."); }
        }

        SonomaCommands::Init { ip } => {
            let client = SonomaClient::new(&ip, user, password);
            if !json { println!("Running init sequence on {}...", ip); }
            client.init().await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if json { println!(r#"{{"status":"ok","action":"init"}}"#); }
            else { println!("Init complete."); }
        }

        SonomaCommands::Exec { ip, cmd } => {
            let client = SonomaClient::new(&ip, user, password);
            let result = client.exec(&cmd).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            if json {
                // Escape stdout for JSON
                let escaped = result.stdout.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
                println!(r#"{{"exit_code":{},"stdout":"{}"}}"#, result.exit_code, escaped);
            } else {
                print!("{}", result.stdout);
                if !result.stderr.is_empty() {
                    eprint!("{}", result.stderr);
                }
            }
        }
    }

    Ok(())
}
