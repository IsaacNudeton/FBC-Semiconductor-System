//! Sonoma ELF output parsers
//!
//! Each ELF binary outputs text to stdout. These parsers convert
//! that text into typed structs shared with the FBC system.
//!
//! Output formats (verified from Sonoma AWK scripts):
//! - ADC/XADC: 3 CSV lines (Max, Avg, Min) with 32 comma-separated values
//! - RunSuperVector: "VECTOR FAILED: error_count=N" on failure
//! - rwmem: hex value like "0x00010000"
//! - Most power/config ELFs: silent (exit code only)

use crate::types::*;

/// Parse ADC CSV output (3 lines: Max, Avg, Min — we use Avg)
///
/// Format: `val0,val1,val2,...,val31\n` × 3 lines
/// `channel_offset` shifts channel numbers (0 for low bank, 16 for high bank)
pub fn parse_adc_csv(stdout: &str, channel_offset: u8) -> Result<Vec<AnalogReading>, String> {
    let lines: Vec<&str> = stdout.trim().lines().collect();
    if lines.len() < 2 {
        // Some ELFs return empty output on first run
        return Ok(Vec::new());
    }

    // Use the Avg line (second line, index 1)
    let avg_line = lines.get(1).unwrap_or(&"");
    let values: Vec<&str> = avg_line.split(',').collect();

    let mut readings = Vec::new();
    for (i, val_str) in values.iter().enumerate() {
        let val_str = val_str.trim();
        if val_str.is_empty() {
            continue;
        }
        let raw: f32 = val_str.parse().map_err(|e| {
            format!("Failed to parse ADC value '{}': {}", val_str, e)
        })?;

        let channel = channel_offset + i as u8;
        readings.push(AnalogReading {
            channel,
            raw: raw as u16,
            voltage_mv: raw, // Raw values from Sonoma are already in mV
        });
    }

    Ok(readings)
}

/// Parse vector run result from RunSuperVector.elf output
///
/// Success: no "VECTOR FAILED" in output
/// Failure: "VECTOR FAILED: error_count=N"
pub fn parse_run_result(stdout: &str, time_s: u32) -> Result<RunResult, String> {
    let mut errors = 0u32;
    let mut passed = true;

    for line in stdout.lines() {
        if line.contains("VECTOR FAILED") {
            passed = false;
            // Try to extract error_count=N
            if let Some(count_str) = line.split("error_count=").nth(1) {
                if let Ok(n) = count_str.trim().parse::<u32>() {
                    errors = n;
                }
            }
        }
    }

    Ok(RunResult {
        passed,
        vectors_executed: 0, // Not available from stdout
        errors,
        duration_s: time_s as f32,
    })
}

/// Parse hex value from rwmem output
///
/// Format: "0x00010000" or just "00010000" or decimal
pub fn parse_hex_value(stdout: &str) -> Result<u32, String> {
    let s = stdout.trim();
    if s.is_empty() {
        return Err("Empty rwmem output".into());
    }

    // Try hex with 0x prefix
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u32::from_str_radix(hex.trim(), 16)
            .map_err(|e| format!("Failed to parse hex '{}': {}", s, e));
    }

    // Try plain hex (all hex digits)
    if s.len() == 8 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        return u32::from_str_radix(s, 16)
            .map_err(|e| format!("Failed to parse hex '{}': {}", s, e));
    }

    // Try decimal
    s.parse::<u32>()
        .map_err(|e| format!("Failed to parse value '{}': {}", s, e))
}

/// Parse PMBus voltage/current from Vout40Ch.elf output
///
/// Format: 6 CSV lines (MaxV, AvgV, MinV, MaxI, AvgI, MinI)
/// Returns (voltage_readings, current_readings) using Avg values
pub fn parse_pmbus_readings(stdout: &str) -> Result<(Vec<f32>, Vec<f32>), String> {
    let lines: Vec<&str> = stdout.trim().lines().collect();
    if lines.len() < 5 {
        return Err(format!("Expected 6 CSV lines, got {}", lines.len()));
    }

    // AvgV is line 1 (index 1), AvgI is line 4 (index 4)
    let voltages = parse_csv_floats(lines[1])?;
    let currents = parse_csv_floats(lines[4])?;

    Ok((voltages, currents))
}

fn parse_csv_floats(line: &str) -> Result<Vec<f32>, String> {
    line.split(',')
        .map(|s| {
            let s = s.trim();
            if s.is_empty() {
                Ok(0.0)
            } else {
                s.parse::<f32>()
                    .map_err(|e| format!("Failed to parse '{}': {}", s, e))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_adc_csv() {
        let output = "100,200,300,400\n150,250,350,450\n120,220,320,420\n";
        let readings = parse_adc_csv(output, 0).unwrap();
        assert_eq!(readings.len(), 4);
        assert_eq!(readings[0].channel, 0);
        assert_eq!(readings[0].raw, 150); // Avg line
        assert_eq!(readings[3].channel, 3);
    }

    #[test]
    fn test_parse_adc_csv_with_offset() {
        let output = "100,200\n150,250\n120,220\n";
        let readings = parse_adc_csv(output, 16).unwrap();
        assert_eq!(readings[0].channel, 16);
        assert_eq!(readings[1].channel, 17);
    }

    #[test]
    fn test_parse_run_result_pass() {
        let output = "Loading vectors...\nRunning...\nDone.\n";
        let r = parse_run_result(output, 10).unwrap();
        assert!(r.passed);
        assert_eq!(r.errors, 0);
    }

    #[test]
    fn test_parse_run_result_fail() {
        let output = "VECTOR FAILED: error_count=5\nVector# = 42 Cycle# = 100\n";
        let r = parse_run_result(output, 10).unwrap();
        assert!(!r.passed);
        assert_eq!(r.errors, 5);
    }

    #[test]
    fn test_parse_hex_value() {
        assert_eq!(parse_hex_value("0x00010000").unwrap(), 0x00010000);
        assert_eq!(parse_hex_value("DEADBEEF").unwrap(), 0xDEADBEEF);
        assert_eq!(parse_hex_value("42").unwrap(), 42);
    }

    #[test]
    fn test_parse_hex_empty() {
        assert!(parse_hex_value("").is_err());
    }
}
