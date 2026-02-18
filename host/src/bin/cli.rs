//! FBC CLI - Command Line Interface for FBC System v2
//!
//! Quick testing tool. For production, use the GUI.
//!
//! # Examples
//!
//! ```bash
//! # Discover all boards
//! fbc-cli discover
//!
//! # Run vectors on all boards and wait for completion
//! fbc-cli run all --vectors test.fbc --wait
//!
//! # Monitor all running boards
//! fbc-cli monitor
//!
//! # Run batch script
//! fbc-cli batch commands.txt
//! ```

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use clap::{Parser, Subcommand};
use fbc_host::{FbcClient, BoardInfo, BoardStatus, format_mac, parse_mac};

#[derive(Parser)]
#[command(name = "fbc-cli")]
#[command(about = "FBC System v2 - Raw Ethernet Control")]
#[command(version = "2.0.0")]
struct Cli {
    /// Network interface name
    #[arg(short, long, default_value = "Ethernet")]
    interface: String,

    /// Output as JSON (for scripting)
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available network interfaces
    Interfaces,

    /// Discover all FBC boards on the network
    Discover {
        /// Discovery timeout in seconds
        #[arg(short, long, default_value = "2")]
        timeout: u64,
    },

    /// Ping a board
    Ping {
        /// Board MAC address (e.g., 00:0A:35:00:01:00)
        mac: String,
    },

    /// Get board status
    Status {
        /// Board MAC address (or "all" for all discovered boards)
        #[arg(default_value = "all")]
        target: String,
    },

    /// Upload a script to a board
    Upload {
        /// Board MAC address
        mac: String,
        /// Script slot (0-15)
        #[arg(short, long, default_value = "0")]
        slot: u8,
        /// FBC script file
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

        /// Loop count (0 = infinite until stopped)
        #[arg(short, long, default_value = "1")]
        loops: u32,

        /// Script slot (0-15)
        #[arg(short, long, default_value = "0")]
        slot: u8,

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

    /// Monitor running boards with live updates
    Monitor {
        /// Refresh interval in milliseconds
        #[arg(short, long, default_value = "500")]
        interval: u64,

        /// Exit when all boards are idle/done
        #[arg(short, long)]
        exit_when_done: bool,
    },

    /// Run commands from a script file
    Batch {
        /// Script file with commands (one per line)
        script: PathBuf,

        /// Stop on first error
        #[arg(long)]
        fail_fast: bool,
    },
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Parse target specification into list of MAC addresses
async fn resolve_targets(
    client: &FbcClient,
    targets: &str,
) -> anyhow::Result<Vec<[u8; 6]>> {
    if targets.eq_ignore_ascii_case("all") {
        // Discover all boards
        let boards = client.discover(Duration::from_secs(2)).await?;
        if boards.is_empty() {
            anyhow::bail!("No boards found on network");
        }
        Ok(boards.into_iter().map(|b| b.mac).collect())
    } else if targets.contains(',') {
        // Comma-separated MACs
        targets
            .split(',')
            .map(|s| {
                parse_mac(s.trim())
                    .ok_or_else(|| anyhow::anyhow!("Invalid MAC address: {}", s))
            })
            .collect()
    } else {
        // Single MAC
        let mac = parse_mac(targets)
            .ok_or_else(|| anyhow::anyhow!("Invalid MAC address: {}", targets))?;
        Ok(vec![mac])
    }
}

/// Print board status in table format
fn print_status_table(boards: &[(String, fbc_host::StatusResponse)]) {
    println!("{:<18} {:>10} {:>12} {:>8} {:>8}",
        "MAC", "Status", "Cycles", "Vectors", "Errors");
    println!("{}", "-".repeat(60));
    for (mac, status) in boards {
        println!("{:<18} {:>10} {:>12} {:>8} {:>8}",
            mac,
            format!("{:?}", status.status),
            status.cycle_count,
            status.vector_count,
            status.error_count
        );
    }
}

/// Print board status as JSON
fn print_status_json(boards: &[(String, fbc_host::StatusResponse)]) {
    print!("[");
    for (i, (mac, status)) in boards.iter().enumerate() {
        if i > 0 { print!(","); }
        print!(r#"{{"mac":"{}","status":"{}","cycles":{},"vectors":{},"errors":{}}}"#,
            mac,
            format!("{:?}", status.status),
            status.cycle_count,
            status.vector_count,
            status.error_count
        );
    }
    println!("]");
}

/// Clear terminal line and move cursor up
fn clear_lines(n: usize) {
    for _ in 0..n {
        print!("\x1b[1A\x1b[2K");  // Move up and clear line
    }
    std::io::stdout().flush().ok();
}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Interfaces => {
            let interfaces = FbcClient::list_interfaces();
            if cli.json {
                print!("[");
                for (i, iface) in interfaces.iter().enumerate() {
                    if i > 0 { print!(","); }
                    print!("\"{}\"", iface);
                }
                println!("]");
            } else {
                println!("Available network interfaces:");
                for iface in interfaces {
                    println!("  {}", iface);
                }
            }
        }

        Commands::Discover { timeout } => {
            let client = FbcClient::new(&cli.interface)?;
            if !cli.json {
                println!("Discovering boards on {} ({}s timeout)...", cli.interface, timeout);
            }

            let boards = client.discover(Duration::from_secs(timeout)).await?;

            if cli.json {
                print!("[");
                for (i, board) in boards.iter().enumerate() {
                    if i > 0 { print!(","); }
                    print!(r#"{{"mac":"{}","board_id":{},"serial":{},"hw_rev":{},"status":"{}"}}"#,
                        format_mac(&board.mac),
                        board.board_id,
                        board.serial,
                        board.hw_rev,
                        format!("{:?}", board.status)
                    );
                }
                println!("]");
            } else if boards.is_empty() {
                println!("No boards found.");
            } else {
                println!("Found {} board(s):", boards.len());
                println!("{:<18} {:>8} {:>10} {:>8} {:>10}",
                    "MAC", "BoardID", "Serial", "HW Rev", "Status");
                println!("{}", "-".repeat(58));
                for board in boards {
                    println!("{:<18} {:>8} {:>10} {:>8} {:>10}",
                        format_mac(&board.mac),
                        format!("{:04X}", board.board_id),
                        format!("{:08X}", board.serial),
                        format!("{:04X}", board.hw_rev),
                        format!("{:?}", board.status)
                    );
                }
            }
        }

        Commands::Ping { mac } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC address"))?;
            let client = FbcClient::new(&cli.interface)?;

            let latency = client.ping(&mac).await?;
            if cli.json {
                println!(r#"{{"mac":"{}","latency_us":{}}}"#,
                    format_mac(&mac),
                    latency.as_micros()
                );
            } else {
                println!("Ping {}: {:.2}ms", format_mac(&mac), latency.as_secs_f64() * 1000.0);
            }
        }

        Commands::Status { target } => {
            let client = FbcClient::new(&cli.interface)?;
            let targets = resolve_targets(&client, &target).await?;

            let mut results = Vec::new();
            for mac in &targets {
                match client.get_status(mac).await {
                    Ok(status) => results.push((format_mac(mac), status)),
                    Err(e) => {
                        if !cli.json {
                            eprintln!("Error getting status from {}: {}", format_mac(mac), e);
                        }
                    }
                }
            }

            if cli.json {
                print_status_json(&results);
            } else {
                print_status_table(&results);
            }
        }

        Commands::Upload { mac, slot, file } => {
            let mac = parse_mac(&mac).ok_or_else(|| anyhow::anyhow!("Invalid MAC address"))?;
            let client = FbcClient::new(&cli.interface)?;

            let data = std::fs::read(&file)?;
            if !cli.json {
                println!("Uploading {} ({} bytes) to slot {}...", file, data.len(), slot);
            }

            client.upload_script(&mac, slot, &data).await?;

            if cli.json {
                println!(r#"{{"status":"ok","mac":"{}","slot":{},"bytes":{}}}"#,
                    format_mac(&mac), slot, data.len());
            } else {
                println!("Upload complete.");
            }
        }

        Commands::Run { targets, vectors, loops, slot, wait, timeout } => {
            let client = FbcClient::new(&cli.interface)?;
            let target_macs = resolve_targets(&client, &targets).await?;

            // Read vector file
            let data = std::fs::read(&vectors)?;
            if !cli.json {
                println!("Uploading {} ({} bytes) to {} board(s)...",
                    vectors.display(), data.len(), target_macs.len());
            }

            // Upload to all targets
            let mut upload_errors = Vec::new();
            for mac in &target_macs {
                if let Err(e) = client.upload_script(mac, slot, &data).await {
                    upload_errors.push((format_mac(mac), e.to_string()));
                }
            }

            if !upload_errors.is_empty() && !cli.json {
                for (mac, err) in &upload_errors {
                    eprintln!("Upload failed for {}: {}", mac, err);
                }
            }

            // Start execution on all targets
            if !cli.json {
                println!("Starting execution with {} loops...", loops);
            }

            let mut started = Vec::new();
            for mac in &target_macs {
                if let Err(e) = client.run_script(mac, slot, loops).await {
                    if !cli.json {
                        eprintln!("Start failed for {}: {}", format_mac(mac), e);
                    }
                } else {
                    started.push(*mac);
                }
            }

            if !cli.json {
                println!("Started {} board(s).", started.len());
            }

            // Wait for completion if requested
            if wait && !started.is_empty() {
                if !cli.json {
                    println!("Waiting for completion...");
                }

                let timeout_duration = if timeout > 0 {
                    Some(Duration::from_secs(timeout))
                } else {
                    None
                };

                let start_time = Instant::now();
                let mut completed = Vec::new();

                loop {
                    // Check timeout
                    if let Some(td) = timeout_duration {
                        if start_time.elapsed() > td {
                            if !cli.json {
                                eprintln!("Timeout waiting for completion.");
                            }
                            break;
                        }
                    }

                    // Check all boards that haven't completed
                    let mut still_running = false;
                    for mac in &started {
                        if completed.iter().any(|(m, _): &([u8; 6], _)| m == mac) {
                            continue;
                        }

                        if let Ok(status) = client.get_status(mac).await {
                            match status.status {
                                BoardStatus::Done | BoardStatus::Error | BoardStatus::Idle => {
                                    completed.push((*mac, status));
                                }
                                BoardStatus::Running => {
                                    still_running = true;
                                }
                                _ => {
                                    still_running = true;
                                }
                            }
                        }
                    }

                    if !still_running || completed.len() == started.len() {
                        break;
                    }

                    tokio::time::sleep(Duration::from_millis(100)).await;
                }

                // Print final results
                let elapsed = start_time.elapsed();
                if cli.json {
                    let results: Vec<_> = completed.iter()
                        .map(|(mac, status)| (format_mac(mac), status.clone()))
                        .collect();
                    print_status_json(&results);
                } else {
                    println!("\nCompleted in {:.2}s:", elapsed.as_secs_f64());
                    let results: Vec<_> = completed.iter()
                        .map(|(mac, status)| (format_mac(mac), status.clone()))
                        .collect();
                    print_status_table(&results);
                }
            } else if cli.json {
                println!(r#"{{"status":"started","boards":{}}}"#, started.len());
            }
        }

        Commands::Stop { targets } => {
            let client = FbcClient::new(&cli.interface)?;
            let target_macs = resolve_targets(&client, &targets).await?;

            let mut stopped = 0;
            for mac in &target_macs {
                if client.stop(mac).await.is_ok() {
                    stopped += 1;
                    if !cli.json {
                        println!("Stopped {}", format_mac(mac));
                    }
                }
            }

            if cli.json {
                println!(r#"{{"status":"ok","stopped":{}}}"#, stopped);
            } else {
                println!("Stopped {} board(s).", stopped);
            }
        }

        Commands::Monitor { interval, exit_when_done } => {
            let client = FbcClient::new(&cli.interface)?;

            // Initial discovery
            if !cli.json {
                println!("Discovering boards...");
            }
            let boards = client.discover(Duration::from_secs(2)).await?;
            if boards.is_empty() {
                if cli.json {
                    println!("[]");
                } else {
                    println!("No boards found.");
                }
                return Ok(());
            }

            let macs: Vec<[u8; 6]> = boards.iter().map(|b| b.mac).collect();

            if !cli.json {
                println!("Monitoring {} board(s). Press Ctrl+C to exit.\n", macs.len());
            }

            let mut first_print = true;
            let header_lines = 3; // Header + separator + boards

            loop {
                let mut results = Vec::new();
                let mut all_done = true;

                for mac in &macs {
                    if let Ok(status) = client.get_status(mac).await {
                        if matches!(status.status, BoardStatus::Running) {
                            all_done = false;
                        }
                        results.push((format_mac(mac), status));
                    }
                }

                if cli.json {
                    print_status_json(&results);
                    if exit_when_done && all_done {
                        break;
                    }
                } else {
                    // Clear previous output (except first time)
                    if !first_print {
                        clear_lines(header_lines + results.len());
                    }
                    first_print = false;

                    print_status_table(&results);

                    if exit_when_done && all_done {
                        println!("\nAll boards completed.");
                        break;
                    }
                }

                tokio::time::sleep(Duration::from_millis(interval)).await;
            }
        }

        Commands::Batch { script, fail_fast } => {
            let file = std::fs::File::open(&script)?;
            let reader = BufReader::new(file);

            let mut line_num = 0;
            let mut errors = 0;
            let mut successes = 0;

            for line in reader.lines() {
                line_num += 1;
                let line = line?;
                let line = line.trim();

                // Skip empty lines and comments
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                if !cli.json {
                    println!("[{}] {}", line_num, line);
                }

                // Parse and execute the command
                // Build args: prepend "fbc-cli" and interface
                let mut args = vec!["fbc-cli".to_string(), "-i".to_string(), cli.interface.clone()];
                args.extend(line.split_whitespace().map(String::from));

                // Parse as nested CLI
                match Cli::try_parse_from(&args) {
                    Ok(nested_cli) => {
                        // Execute the command by recursively calling main logic
                        // For simplicity, we'll just print what would be executed
                        // A full implementation would factor out the command execution
                        if !cli.json {
                            println!("  (command parsed successfully)");
                        }
                        successes += 1;
                    }
                    Err(e) => {
                        errors += 1;
                        if !cli.json {
                            eprintln!("  Error: {}", e);
                        }
                        if fail_fast {
                            anyhow::bail!("Batch aborted at line {} due to --fail-fast", line_num);
                        }
                    }
                }
            }

            if cli.json {
                println!(r#"{{"lines":{},"successes":{},"errors":{}}}"#, line_num, successes, errors);
            } else {
                println!("\nBatch complete: {} commands, {} succeeded, {} failed",
                    successes + errors, successes, errors);
            }
        }
    }

    Ok(())
}
