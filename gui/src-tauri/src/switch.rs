// Cisco switch integration for MAC-to-port discovery
// Queries the switch's MAC address table to determine board positions

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

/// Physical position in the rack
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RackPosition {
    pub shelf: u8,      // 1-11
    pub tray: String,   // "front" or "back"
    pub slot: u8,       // 1 (A) or 2 (B)
}

/// Port-to-position mapping (configured once based on rack wiring)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PortMapping {
    pub port: String,           // e.g., "Gi0/1", "Fa0/24"
    pub position: RackPosition,
}

/// Switch configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SwitchConfig {
    pub serial_port: String,    // e.g., "COM4"
    pub baud_rate: u32,         // e.g., 9600
    pub port_mappings: Vec<PortMapping>,
}

impl Default for SwitchConfig {
    fn default() -> Self {
        // Default port mappings based on typical Sonoma rack wiring
        // This assumes a 48-port switch with ports mapped sequentially
        let mut mappings = Vec::new();

        // Generate default mappings:
        // Ports 1-4   = Shelf 11 (Front A, Front B, Back A, Back B)
        // Ports 5-8   = Shelf 10
        // ...
        // Ports 41-44 = Shelf 1
        for shelf in (1..=11).rev() {
            let base_port = (11 - shelf) * 4 + 1;

            mappings.push(PortMapping {
                port: format!("Gi0/{}", base_port),
                position: RackPosition { shelf, tray: "front".into(), slot: 1 },
            });
            mappings.push(PortMapping {
                port: format!("Gi0/{}", base_port + 1),
                position: RackPosition { shelf, tray: "front".into(), slot: 2 },
            });
            mappings.push(PortMapping {
                port: format!("Gi0/{}", base_port + 2),
                position: RackPosition { shelf, tray: "back".into(), slot: 1 },
            });
            mappings.push(PortMapping {
                port: format!("Gi0/{}", base_port + 3),
                position: RackPosition { shelf, tray: "back".into(), slot: 2 },
            });
        }

        Self {
            serial_port: "COM4".into(),
            baud_rate: 9600,
            port_mappings: mappings,
        }
    }
}

/// MAC address table entry from switch
#[derive(Debug, Clone)]
pub struct MacTableEntry {
    pub mac: String,
    pub vlan: u16,
    pub port: String,
}

/// Parse Cisco "show mac address-table" output
/// Example format:
/// Vlan    Mac Address       Type        Ports
/// ----    -----------       --------    -----
///    1    0050.5645.0001    DYNAMIC     Gi0/1
///    1    0050.5645.0002    DYNAMIC     Gi0/2
pub fn parse_mac_table(output: &str) -> Vec<MacTableEntry> {
    let mut entries = Vec::new();

    for line in output.lines() {
        let line = line.trim();

        // Skip header lines and empty lines
        if line.is_empty() || line.starts_with("Vlan") || line.starts_with("----") {
            continue;
        }

        // Parse: "   1    0050.5645.0001    DYNAMIC     Gi0/1"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            if let Ok(vlan) = parts[0].parse::<u16>() {
                let mac = parts[1].to_string();
                let port = parts[3].to_string();

                // Only include dynamic entries on Gi/Fa ports (not CPU, etc.)
                if port.starts_with("Gi") || port.starts_with("Fa") {
                    entries.push(MacTableEntry { mac, vlan, port });
                }
            }
        }
    }

    entries
}

/// Convert Cisco MAC format (0050.5645.0001) to standard format (00:50:56:45:00:01)
pub fn normalize_mac(cisco_mac: &str) -> String {
    let clean: String = cisco_mac.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() == 12 {
        format!(
            "{}:{}:{}:{}:{}:{}",
            &clean[0..2], &clean[2..4], &clean[4..6],
            &clean[6..8], &clean[8..10], &clean[10..12]
        ).to_uppercase()
    } else {
        cisco_mac.to_uppercase()
    }
}

/// Query switch and return MAC-to-position mapping
pub fn discover_board_positions(
    config: &SwitchConfig,
) -> Result<HashMap<String, RackPosition>, String> {
    // Build port-to-position lookup
    let port_to_pos: HashMap<String, RackPosition> = config
        .port_mappings
        .iter()
        .map(|m| (m.port.clone(), m.position.clone()))
        .collect();

    // Connect to switch
    let mut port = serialport::new(&config.serial_port, config.baud_rate)
        .timeout(Duration::from_secs(5))
        .open()
        .map_err(|e| format!("Failed to open serial port: {}", e))?;

    // Send command
    port.write_all(b"\r\n").map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(500));

    port.write_all(b"show mac address-table\r\n")
        .map_err(|e| e.to_string())?;

    // Read response (with timeout)
    std::thread::sleep(Duration::from_secs(2));

    let mut output = String::new();
    let mut reader = BufReader::new(port);

    // Read available data
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => output.push_str(&line),
            Err(_) => break,
        }
    }

    // Parse MAC table
    let entries = parse_mac_table(&output);

    // Map MAC to position
    let mut result = HashMap::new();
    for entry in entries {
        if let Some(pos) = port_to_pos.get(&entry.port) {
            let normalized_mac = normalize_mac(&entry.mac);
            result.insert(normalized_mac, pos.clone());
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mac_table() {
        let output = r#"
Vlan    Mac Address       Type        Ports
----    -----------       --------    -----
   1    0050.5645.0001    DYNAMIC     Gi0/1
   1    0050.5645.0002    DYNAMIC     Gi0/2
   1    0050.5645.0003    DYNAMIC     Gi0/5
All    0100.0ccc.cccc    STATIC      CPU
"#;

        let entries = parse_mac_table(output);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].mac, "0050.5645.0001");
        assert_eq!(entries[0].port, "Gi0/1");
    }

    #[test]
    fn test_normalize_mac() {
        assert_eq!(normalize_mac("0050.5645.0001"), "00:50:56:45:00:01");
        assert_eq!(normalize_mac("aabb.ccdd.eeff"), "AA:BB:CC:DD:EE:FF");
    }
}
