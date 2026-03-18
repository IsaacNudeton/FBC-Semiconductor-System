//! FBC CLI - Command line interface for FBC controllers
//!
//! Usage:
//!   fbc-cli --interface "Ethernet" discover
//!   fbc-cli --interface "Ethernet" ping <mac>

use clap::{Parser, Subcommand};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "fbc-cli")]
#[command(about = "FBC Semiconductor System CLI", long_about = None)]
struct Cli {
    /// Network interface name
    #[arg(short, long, default_value = "Ethernet")]
    interface: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Discover boards on the network
    Discover {
        /// Timeout in seconds
        #[arg(short, long, default_value_t = 2)]
        timeout: u64,
    },

    /// Ping a board by MAC address
    Ping {
        /// MAC address (e.g., "00:11:22:33:44:55")
        mac: String,
    },

    /// List available network interfaces
    ListInterfaces,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ListInterfaces => {
            let interfaces = fbc_host::FbcClient::list_interfaces();
            println!("Available network interfaces:");
            for iface in interfaces {
                println!("  {}", iface);
            }
        }

        Commands::Discover { timeout } => {
            println!("Creating client on interface '{}'...", cli.interface);
            let mut client = fbc_host::FbcClient::new(&cli.interface)?;

            println!("Discovering boards ({}s timeout)...", timeout);
            let boards = client.discover(Duration::from_secs(timeout))?;

            if boards.is_empty() {
                println!("No boards found.");
            } else {
                println!("Found {} board(s):", boards.len());
                for board in &boards {
                    println!("  S/N {:08X} (MAC: {}, FW: {}.{})",
                        board.serial,
                        fbc_host::format_mac(&board.mac),
                        board.fw_version >> 8,
                        board.fw_version & 0xFF,
                    );
                }
            }
        }

        Commands::Ping { mac } => {
            let mac_bytes = fbc_host::parse_mac(&mac)
                .ok_or_else(|| anyhow::anyhow!("Invalid MAC address: {}", mac))?;

            let mut client = fbc_host::FbcClient::new(&cli.interface)?;
            let rtt = client.ping(&mac_bytes)?;

            println!("Ping {}: {:.2}ms", mac, rtt.as_secs_f64() * 1000.0);
        }
    }

    Ok(())
}
