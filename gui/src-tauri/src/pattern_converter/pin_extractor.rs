//! pattern_converter/pin_extractor.rs — Pin Table Extraction from CSV/Excel/PDF
//!
//! Extracts pin mappings from engineer-friendly formats (CSV, Excel, PDF datasheets)
//! into an editable intermediate form, then serializes to device JSON for the
//! existing C pipeline (dc_json.c → dc_gen.c).
//!
//! Data flow:
//!   Source File (CSV/Excel/PDF)
//!     → ExtractedPinTable (editable in frontend)
//!     → device.json (temp file)
//!     → dc_generate_config (existing C pipeline)
//!     → PIN_MAP + .map + .lvl + .tim + .tp + PowerOn/Off.sh

use serde::{Deserialize, Serialize};
use std::path::Path;

/* ═══════════════════════════════════════════════════════════════
 * CORE TYPES
 * ═══════════════════════════════════════════════════════════════ */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedPinTable {
    pub device_name: String,
    pub source_format: String, // "csv", "xlsx", "pdf"
    pub channels: Vec<ExtractedChannel>,
    pub supplies: Vec<ExtractedSupply>,
    pub bank_voltages: Vec<BankVoltage>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedChannel {
    pub signal_name: String,
    pub channel: i32,
    pub direction: String, // "IO", "Input", "Output"
    pub voltage: Option<f64>,
    pub group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSupply {
    pub core_name: String,
    pub voltage: f64,
    pub sequence_order: i32,
    pub ramp_delay_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankVoltage {
    pub bank_name: String,
    pub voltage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub primary: ExtractedPinTable,
    pub mismatches: Vec<PinMismatch>,
    pub match_count: usize,
    pub mismatch_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinMismatch {
    pub signal_name: String,
    pub field: String,           // "channel", "direction", "voltage"
    pub primary_value: String,
    pub secondary_value: String,
}

/* ═══════════════════════════════════════════════════════════════
 * FORMAT DETECTION
 * ═══════════════════════════════════════════════════════════════ */

pub fn detect_format(path: &str) -> Result<String, String> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "csv" | "tsv" | "txt" => Ok("csv".into()),
        "xlsx" | "xls" | "xlsm" | "xlsb" | "ods" => Ok("xlsx".into()),
        "pdf" => Ok("pdf".into()),
        _ => Err(format!("Unsupported file format: .{}", ext)),
    }
}

pub fn extract_pin_table(path: &str) -> Result<ExtractedPinTable, String> {
    let fmt = detect_format(path)?;
    match fmt.as_str() {
        "csv" => extract_from_csv(path),
        "xlsx" => extract_from_excel(path),
        "pdf" => extract_from_pdf(path),
        _ => Err(format!("Unsupported format: {}", fmt)),
    }
}

/* ═══════════════════════════════════════════════════════════════
 * CSV EXTRACTION
 *
 * Ports dc_csv.c column detection heuristics to Rust:
 * - Auto-detect delimiter (comma, tab, semicolon)
 * - Find header row by scanning for column name patterns
 * - Match columns: signal/pin/name, channel/pin_number/gpio, direction, voltage, group
 * - Detect supply rows (voltage + core name patterns)
 * ═══════════════════════════════════════════════════════════════ */

/// Column role detected from header text
#[derive(Debug, Clone, Copy, PartialEq)]
enum ColRole {
    Signal,
    Channel,
    Direction,
    Voltage,
    Group,
    CoreName,
    Sequence,
    RampDelay,
    Unknown,
}

fn classify_column(header: &str) -> ColRole {
    let h = header.to_lowercase();
    let h = h.trim();

    // Signal / pin name
    if h == "signal" || h == "signal_name" || h == "pin_name" || h == "net"
        || h == "net_name" || h == "name" || h == "pin"
    {
        return ColRole::Signal;
    }
    // Channel / GPIO number
    if h == "channel" || h == "ch" || h == "gpio" || h == "pin_number"
        || h == "pin_num" || h == "pin_no" || h == "number"
    {
        return ColRole::Channel;
    }
    // Direction
    if h == "direction" || h == "dir" || h == "type" || h == "io"
        || h == "io_type" || h == "pin_type"
    {
        return ColRole::Direction;
    }
    // Voltage
    if h == "voltage" || h == "v" || h == "vcc" || h == "level"
        || h == "bank_voltage" || h == "io_voltage"
    {
        return ColRole::Voltage;
    }
    // Group
    if h == "group" || h == "bank" || h == "bus" || h == "interface" {
        return ColRole::Group;
    }
    // Supply-specific columns
    if h == "core" || h == "core_name" || h == "supply" || h == "rail" {
        return ColRole::CoreName;
    }
    if h == "sequence" || h == "sequence_order" || h == "order" || h == "seq" {
        return ColRole::Sequence;
    }
    if h == "ramp" || h == "ramp_delay" || h == "ramp_delay_ms" || h == "delay" || h == "ramp_ms" {
        return ColRole::RampDelay;
    }

    ColRole::Unknown
}

fn detect_delimiter(line: &str) -> u8 {
    let commas = line.chars().filter(|c| *c == ',').count();
    let tabs = line.chars().filter(|c| *c == '\t').count();
    let semis = line.chars().filter(|c| *c == ';').count();

    if tabs >= commas && tabs >= semis && tabs > 0 {
        b'\t'
    } else if semis > commas && semis > 0 {
        b';'
    } else {
        b','
    }
}

fn normalize_direction(raw: &str) -> String {
    let d = raw.trim().to_lowercase();
    match d.as_str() {
        "io" | "bi" | "bidirectional" | "bidir" | "inout" | "0" => "IO".into(),
        "in" | "input" | "i" | "1" => "Input".into(),
        "out" | "output" | "o" | "2" => "Output".into(),
        _ => "IO".into(), // default to bidirectional
    }
}

fn parse_voltage(raw: &str) -> Option<f64> {
    let s = raw.trim().to_lowercase();
    if s.is_empty() || s == "-" || s == "n/a" || s == "na" {
        return None;
    }
    // Strip trailing 'v' (e.g. "1.8V" → "1.8")
    let s = s.trim_end_matches('v');
    s.parse::<f64>().ok()
}

pub fn extract_from_csv(path: &str) -> Result<ExtractedPinTable, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path, e))?;

    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Err("Empty CSV file".into());
    }

    // Detect delimiter from first non-empty line
    let first_line = lines.iter().find(|l| !l.trim().is_empty()).unwrap_or(&"");
    let delim = detect_delimiter(first_line);

    // Build CSV reader
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .has_headers(false)
        .from_reader(content.as_bytes());

    let records: Vec<csv::StringRecord> = rdr
        .records()
        .filter_map(|r| r.ok())
        .collect();

    if records.is_empty() {
        return Err("No records in CSV".into());
    }

    // Find header row — scan first 10 rows for one that has >=2 recognized columns
    let mut header_idx = 0;
    let mut col_roles: Vec<ColRole> = Vec::new();

    for (i, rec) in records.iter().enumerate().take(10) {
        let roles: Vec<ColRole> = rec.iter().map(|cell| classify_column(cell)).collect();
        let recognized = roles.iter().filter(|r| **r != ColRole::Unknown).count();
        if recognized >= 2 {
            header_idx = i;
            col_roles = roles;
            break;
        }
    }

    if col_roles.is_empty() || col_roles.iter().all(|r| *r == ColRole::Unknown) {
        // No header found — try positional: assume col0=signal, col1=channel
        col_roles = vec![ColRole::Signal, ColRole::Channel];
        if records[0].len() > 2 {
            col_roles.push(ColRole::Direction);
        }
        if records[0].len() > 3 {
            col_roles.push(ColRole::Voltage);
        }
        if records[0].len() > 4 {
            col_roles.push(ColRole::Group);
        }
        header_idx = 0; // data starts at row 0 (no header)
        // Check if first row looks like a header anyway
        let first_val = records[0].get(1).unwrap_or("");
        if first_val.parse::<i32>().is_err() && !first_val.is_empty() {
            header_idx = 0; // skip row 0 as header
        }
    }

    // Find column indices for each role
    let find_col = |role: ColRole| -> Option<usize> {
        col_roles.iter().position(|r| *r == role)
    };

    let sig_col = find_col(ColRole::Signal);
    let ch_col = find_col(ColRole::Channel);
    let dir_col = find_col(ColRole::Direction);
    let volt_col = find_col(ColRole::Voltage);
    let grp_col = find_col(ColRole::Group);

    // Supply columns
    let core_col = find_col(ColRole::CoreName);
    let seq_col = find_col(ColRole::Sequence);
    let ramp_col = find_col(ColRole::RampDelay);

    let mut channels = Vec::new();
    let mut supplies = Vec::new();
    let mut warnings = Vec::new();

    // Check if this CSV has a supply section (look for CORE/supply keywords in data rows)
    let has_supply_section = core_col.is_some();

    // Parse data rows
    for (i, rec) in records.iter().enumerate().skip(header_idx + 1) {
        // Skip empty rows
        if rec.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }

        // If we see a supply section header mid-file, switch modes
        let first_cell = rec.get(0).unwrap_or("").trim().to_lowercase();
        if first_cell == "supplies" || first_cell == "power" || first_cell == "supply" {
            // Re-classify this row as supply header and continue
            continue;
        }

        // Try to detect if this row is a supply row
        // (has core_col with a CORE-like name, or voltage but no channel number)
        if has_supply_section {
            if let Some(cc) = core_col {
                let core_val = rec.get(cc).unwrap_or("").trim();
                if !core_val.is_empty() && core_val.to_lowercase().starts_with("core") {
                    let voltage = volt_col
                        .and_then(|c| rec.get(c))
                        .and_then(|v| parse_voltage(v))
                        .unwrap_or(0.0);
                    let seq = seq_col
                        .and_then(|c| rec.get(c))
                        .and_then(|v| v.trim().parse::<i32>().ok())
                        .unwrap_or(supplies.len() as i32);
                    let ramp = ramp_col
                        .and_then(|c| rec.get(c))
                        .and_then(|v| v.trim().parse::<f64>().ok())
                        .unwrap_or(10.0);

                    supplies.push(ExtractedSupply {
                        core_name: core_val.to_string(),
                        voltage,
                        sequence_order: seq,
                        ramp_delay_ms: ramp,
                    });
                    continue;
                }
            }
        }

        // Channel row
        let signal = sig_col
            .and_then(|c| rec.get(c))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if signal.is_empty() {
            continue;
        }

        let channel = ch_col
            .and_then(|c| rec.get(c))
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or(-1);

        if channel < 0 {
            warnings.push(format!("Row {}: no channel number for signal '{}'", i + 1, signal));
            continue;
        }

        let direction: String = dir_col
            .and_then(|c| rec.get(c))
            .map(|d: &str| normalize_direction(d))
            .unwrap_or_else(|| "IO".into());

        let voltage: Option<f64> = volt_col
            .and_then(|c| rec.get(c))
            .and_then(|v: &str| parse_voltage(v));

        let group: Option<String> = grp_col
            .and_then(|c| rec.get(c))
            .map(|g: &str| g.trim().to_string())
            .filter(|g: &String| !g.is_empty());

        channels.push(ExtractedChannel {
            signal_name: signal,
            channel,
            direction,
            voltage,
            group,
        });
    }

    if channels.is_empty() && supplies.is_empty() {
        return Err("No pin data found in CSV. Check column headers.".into());
    }

    // Derive device name from filename
    let device_name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    // Derive bank voltages from channel voltage values
    let bank_voltages = derive_bank_voltages(&channels);

    Ok(ExtractedPinTable {
        device_name,
        source_format: "csv".into(),
        channels,
        supplies,
        bank_voltages,
        warnings,
    })
}

/* ═══════════════════════════════════════════════════════════════
 * EXCEL EXTRACTION
 *
 * Uses calamine to parse .xlsx/.xls/.xlsm/.ods files.
 * Same column detection heuristics as CSV.
 * ═══════════════════════════════════════════════════════════════ */

pub fn extract_from_excel(path: &str) -> Result<ExtractedPinTable, String> {
    use calamine::{open_workbook_auto, Data, Reader};

    let mut workbook = open_workbook_auto(path)
        .map_err(|e| format!("Failed to open Excel file: {}", e))?;

    // Get first sheet
    let sheet_names = workbook.sheet_names().to_vec();
    if sheet_names.is_empty() {
        return Err("Excel file has no sheets".into());
    }

    let range = workbook
        .worksheet_range(&sheet_names[0])
        .map_err(|e| format!("Failed to read sheet '{}': {}", sheet_names[0], e))?;

    let rows: Vec<Vec<String>> = range
        .rows()
        .map(|row| {
            row.iter()
                .map(|cell: &Data| match cell {
                    Data::String(s) => s.clone(),
                    Data::Float(f) => {
                        if *f == (*f as i64) as f64 {
                            format!("{}", *f as i64)
                        } else {
                            format!("{}", f)
                        }
                    }
                    Data::Int(i) => format!("{}", i),
                    Data::Bool(b) => format!("{}", b),
                    _ => String::new(),
                })
                .collect()
        })
        .collect();

    if rows.is_empty() {
        return Err("Empty Excel sheet".into());
    }

    // Parse rows into pin table using shared logic
    parse_string_rows_into_table(&rows, "xlsx", path)
}

/// Shared logic for parsing string-based row data (used by both CSV and Excel)
fn parse_string_rows_into_table(
    rows: &[Vec<String>],
    source_format: &str,
    path: &str,
) -> Result<ExtractedPinTable, String> {
    if rows.is_empty() {
        return Err("No data rows".into());
    }

    // Find header row
    let mut header_idx = 0;
    let mut col_roles: Vec<ColRole> = Vec::new();

    for (i, row) in rows.iter().enumerate().take(10) {
        let roles: Vec<ColRole> = row.iter().map(|cell| classify_column(cell)).collect();
        let recognized = roles.iter().filter(|r| **r != ColRole::Unknown).count();
        if recognized >= 2 {
            header_idx = i;
            col_roles = roles;
            break;
        }
    }

    if col_roles.is_empty() || col_roles.iter().all(|r| *r == ColRole::Unknown) {
        col_roles = vec![ColRole::Signal, ColRole::Channel];
        if rows[0].len() > 2 { col_roles.push(ColRole::Direction); }
        if rows[0].len() > 3 { col_roles.push(ColRole::Voltage); }
        if rows[0].len() > 4 { col_roles.push(ColRole::Group); }
    }

    let find_col = |role: ColRole| -> Option<usize> {
        col_roles.iter().position(|r| *r == role)
    };

    let sig_col = find_col(ColRole::Signal);
    let ch_col = find_col(ColRole::Channel);
    let dir_col = find_col(ColRole::Direction);
    let volt_col = find_col(ColRole::Voltage);
    let grp_col = find_col(ColRole::Group);
    let core_col = find_col(ColRole::CoreName);
    let seq_col = find_col(ColRole::Sequence);
    let ramp_col = find_col(ColRole::RampDelay);

    let has_supply_section = core_col.is_some();

    let mut channels = Vec::new();
    let mut supplies = Vec::new();
    let mut warnings = Vec::new();

    for (i, row) in rows.iter().enumerate().skip(header_idx + 1) {
        if row.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }

        let first_cell = row.first().map(|s| s.trim().to_lowercase()).unwrap_or_default();
        if first_cell == "supplies" || first_cell == "power" || first_cell == "supply" {
            continue;
        }

        // Supply row detection
        if has_supply_section {
            if let Some(cc) = core_col {
                let core_val = row.get(cc).map(|s| s.as_str().trim()).unwrap_or("");
                if !core_val.is_empty() && core_val.to_lowercase().starts_with("core") {
                    let voltage = volt_col
                        .and_then(|c| row.get(c))
                        .and_then(|v| parse_voltage(v))
                        .unwrap_or(0.0);
                    let seq = seq_col
                        .and_then(|c| row.get(c))
                        .and_then(|v| v.trim().parse::<i32>().ok())
                        .unwrap_or(supplies.len() as i32);
                    let ramp = ramp_col
                        .and_then(|c| row.get(c))
                        .and_then(|v| v.trim().parse::<f64>().ok())
                        .unwrap_or(10.0);

                    supplies.push(ExtractedSupply {
                        core_name: core_val.to_string(),
                        voltage,
                        sequence_order: seq,
                        ramp_delay_ms: ramp,
                    });
                    continue;
                }
            }
        }

        let signal = sig_col
            .and_then(|c| row.get(c))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if signal.is_empty() {
            continue;
        }

        let channel = ch_col
            .and_then(|c| row.get(c))
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or(-1);

        if channel < 0 {
            warnings.push(format!("Row {}: no channel for '{}'", i + 1, signal));
            continue;
        }

        let direction = dir_col
            .and_then(|c| row.get(c))
            .map(|d| normalize_direction(d.as_str()))
            .unwrap_or_else(|| "IO".into());

        let voltage = volt_col
            .and_then(|c| row.get(c))
            .and_then(|v| parse_voltage(v.as_str()));

        let group = grp_col
            .and_then(|c| row.get(c))
            .map(|g: &String| g.trim().to_string())
            .filter(|g: &String| !g.is_empty());

        channels.push(ExtractedChannel {
            signal_name: signal,
            channel,
            direction,
            voltage,
            group,
        });
    }

    if channels.is_empty() && supplies.is_empty() {
        return Err(format!("No pin data found in {}. Check column headers.", source_format));
    }

    let device_name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let bank_voltages = derive_bank_voltages(&channels);

    Ok(ExtractedPinTable {
        device_name,
        source_format: source_format.into(),
        channels,
        supplies,
        bank_voltages,
        warnings,
    })
}

/* ═══════════════════════════════════════════════════════════════
 * PDF EXTRACTION
 *
 * Two strategies:
 * 1. Datasheet PDF — find pin table headers, parse rows
 * 2. Schematic PDF — extract net name + pin number pairs
 *
 * PCB tools (Altium, Eagle, KiCad) embed text as real PDF text,
 * not raster images, so text extraction works well.
 * ═══════════════════════════════════════════════════════════════ */

pub fn extract_from_pdf(path: &str) -> Result<ExtractedPinTable, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Failed to read PDF: {}", e))?;

    let text = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| format!("Failed to extract PDF text: {}", e))?;

    if text.trim().is_empty() {
        return Err("PDF contains no extractable text (may be scanned/raster)".into());
    }

    let lines: Vec<&str> = text.lines().collect();
    let mut warnings = Vec::new();
    let mut channels = Vec::new();

    // Strategy 1: Find tabular pin data
    // Look for header-like lines containing "Pin" + "Signal" or "Name" + "Channel"
    let mut in_table = false;
    let mut table_col_pattern: Option<TablePattern> = None;

    for (_line_num, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check if this line looks like a pin table header
        if !in_table {
            if let Some(pattern) = detect_table_header(trimmed) {
                in_table = true;
                table_col_pattern = Some(pattern);
                continue;
            }
        }

        // Parse table rows
        if in_table {
            if let Some(ref pattern) = table_col_pattern {
                match parse_table_row(trimmed, pattern) {
                    Some(ch) => channels.push(ch),
                    None => {
                        // End of table (non-matching row after we started)
                        if channels.len() > 2 {
                            in_table = false;
                        }
                    }
                }
            }
        }
    }

    // Strategy 2: If no table found, try to extract pin/signal pairs from scattered text
    if channels.is_empty() {
        warnings.push("No pin table found. Trying pattern extraction...".into());
        channels = extract_scattered_pins(&lines, &mut warnings);
    }

    if channels.is_empty() {
        return Err("Could not extract pin data from PDF. Try CSV or Excel instead.".into());
    }

    warnings.push(format!(
        "PDF extraction found {} pins. Verify and edit before generating.",
        channels.len()
    ));

    let device_name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let bank_voltages = derive_bank_voltages(&channels);

    Ok(ExtractedPinTable {
        device_name,
        source_format: "pdf".into(),
        channels,
        supplies: Vec::new(), // PDF extraction rarely catches supplies reliably
        bank_voltages,
        warnings,
    })
}

#[derive(Debug, Clone)]
struct TablePattern {
    // Which whitespace-delimited fields map to what
    pin_field: Option<usize>,
    signal_field: Option<usize>,
    direction_field: Option<usize>,
    voltage_field: Option<usize>,
    total_fields: usize,
}

fn detect_table_header(line: &str) -> Option<TablePattern> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 2 {
        return None;
    }

    let mut pin_field = None;
    let mut signal_field = None;
    let mut direction_field = None;
    let mut voltage_field = None;
    let mut recognized = 0;

    for (i, field) in fields.iter().enumerate() {
        let f = field.to_lowercase();
        if (f == "pin" || f == "pin#" || f == "pin_no" || f == "number") && pin_field.is_none() {
            pin_field = Some(i);
            recognized += 1;
        } else if (f == "signal" || f == "name" || f == "net" || f == "function"
            || f == "signal_name" || f == "pin_name")
            && signal_field.is_none()
        {
            signal_field = Some(i);
            recognized += 1;
        } else if (f == "direction" || f == "dir" || f == "type" || f == "i/o")
            && direction_field.is_none()
        {
            direction_field = Some(i);
            recognized += 1;
        } else if (f == "voltage" || f == "level" || f == "vcc") && voltage_field.is_none() {
            voltage_field = Some(i);
            recognized += 1;
        }
    }

    if recognized >= 2 && (pin_field.is_some() || signal_field.is_some()) {
        Some(TablePattern {
            pin_field,
            signal_field,
            direction_field,
            voltage_field,
            total_fields: fields.len(),
        })
    } else {
        None
    }
}

fn parse_table_row(line: &str, pattern: &TablePattern) -> Option<ExtractedChannel> {
    let fields: Vec<&str> = line.split_whitespace().collect();

    // Row must have roughly the same field count
    if fields.len() < 2 || (fields.len() as i32 - pattern.total_fields as i32).unsigned_abs() > 2 {
        return None;
    }

    let signal = pattern
        .signal_field
        .and_then(|i| fields.get(i))
        .map(|s| s.to_string());

    let channel = pattern
        .pin_field
        .and_then(|i| fields.get(i))
        .and_then(|v| v.parse::<i32>().ok());

    // Must have at least a signal or channel
    let signal_name = signal.unwrap_or_default();
    let channel_num = channel.unwrap_or(-1);

    if signal_name.is_empty() && channel_num < 0 {
        return None;
    }

    // Signal name should look like an identifier, not a sentence
    if signal_name.contains(' ') || signal_name.len() > 64 {
        return None;
    }

    let direction = pattern
        .direction_field
        .and_then(|i| fields.get(i))
        .map(|d| normalize_direction(d))
        .unwrap_or_else(|| "IO".into());

    let voltage = pattern
        .voltage_field
        .and_then(|i| fields.get(i))
        .and_then(|v| parse_voltage(v));

    Some(ExtractedChannel {
        signal_name: if signal_name.is_empty() {
            format!("PIN_{}", channel_num)
        } else {
            signal_name
        },
        channel: channel_num,
        direction,
        voltage,
        group: None,
    })
}

/// Strategy 2: Find scattered pin/signal pairs in PDF text
/// Looks for patterns like: "GPIO_0 (Pin 14)" or "Pin 3 DQ0" or "14 CLK"
fn extract_scattered_pins(lines: &[&str], warnings: &mut Vec<String>) -> Vec<ExtractedChannel> {
    use std::collections::HashMap;
    let mut pin_map: HashMap<i32, String> = HashMap::new();

    // Pattern: "Pin <N> <SIGNAL>" or "<SIGNAL> Pin <N>" or "<SIGNAL> (<N>)"
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Try pattern: "Pin <N> <SIGNAL>"
        if let Some(caps) = simple_pin_signal_match(trimmed) {
            let (pin, signal) = caps;
            if pin >= 0 && pin < 256 && !signal.is_empty() {
                pin_map.entry(pin).or_insert(signal);
            }
        }
    }

    if pin_map.is_empty() {
        warnings.push("No pin/signal patterns found in PDF text".into());
    }

    let mut channels: Vec<ExtractedChannel> = pin_map
        .into_iter()
        .map(|(pin, signal)| ExtractedChannel {
            signal_name: signal,
            channel: pin,
            direction: "IO".into(),
            voltage: None,
            group: None,
        })
        .collect();

    channels.sort_by_key(|c| c.channel);
    channels
}

/// Simple regex-free pattern matching for "Pin N SIGNAL" patterns
fn simple_pin_signal_match(line: &str) -> Option<(i32, String)> {
    let lower = line.to_lowercase();

    // "Pin <N> <SIGNAL>"
    if let Some(rest) = lower.strip_prefix("pin") {
        let rest = rest.trim_start();
        if let Some((num_str, after)) = split_first_number(rest) {
            if let Ok(pin) = num_str.parse::<i32>() {
                let signal = extract_signal_name(after);
                if !signal.is_empty() {
                    return Some((pin, signal));
                }
            }
        }
    }

    // "<N> <SIGNAL>" — line starts with a number (tabular data)
    let trimmed = line.trim();
    if let Some((num_str, after)) = split_first_number(trimmed) {
        if let Ok(pin) = num_str.parse::<i32>() {
            if pin < 256 {
                let signal = extract_signal_name(after);
                if !signal.is_empty() && is_signal_like(&signal) {
                    return Some((pin, signal));
                }
            }
        }
    }

    None
}

fn split_first_number(s: &str) -> Option<(&str, &str)> {
    let s = s.trim();
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    Some((&s[..end], s[end..].trim()))
}

fn extract_signal_name(s: &str) -> String {
    let s = s.trim().trim_start_matches(|c: char| c == ':' || c == '.' || c == '-');
    let s = s.trim();
    // Take first whitespace-delimited token
    let end = s.find(|c: char| c.is_whitespace()).unwrap_or(s.len());
    let token = &s[..end];
    // Must start with letter or underscore
    if token.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        token.to_string()
    } else {
        String::new()
    }
}

fn is_signal_like(name: &str) -> bool {
    // Signal names typically: start with letter, contain only alphanum/underscore
    // Not too short (>1 char), not too long (<32)
    name.len() > 1
        && name.len() < 32
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/* ═══════════════════════════════════════════════════════════════
 * CROSS-VERIFICATION
 *
 * Compares two extracted pin tables, matching by signal name.
 * Reports mismatches in channel, direction, voltage.
 * ═══════════════════════════════════════════════════════════════ */

pub fn cross_verify(
    primary: ExtractedPinTable,
    secondary: &ExtractedPinTable,
) -> VerificationResult {
    use std::collections::HashMap;

    // Index secondary by signal name (case-insensitive)
    let secondary_map: HashMap<String, &ExtractedChannel> = secondary
        .channels
        .iter()
        .map(|ch| (ch.signal_name.to_lowercase(), ch))
        .collect();

    let mut mismatches = Vec::new();
    let mut match_count = 0;

    for primary_ch in &primary.channels {
        let key = primary_ch.signal_name.to_lowercase();
        if let Some(sec_ch) = secondary_map.get(&key) {
            // Channel number mismatch
            if primary_ch.channel != sec_ch.channel {
                mismatches.push(PinMismatch {
                    signal_name: primary_ch.signal_name.clone(),
                    field: "channel".into(),
                    primary_value: primary_ch.channel.to_string(),
                    secondary_value: sec_ch.channel.to_string(),
                });
            }
            // Direction mismatch
            if primary_ch.direction.to_lowercase() != sec_ch.direction.to_lowercase() {
                mismatches.push(PinMismatch {
                    signal_name: primary_ch.signal_name.clone(),
                    field: "direction".into(),
                    primary_value: primary_ch.direction.clone(),
                    secondary_value: sec_ch.direction.clone(),
                });
            }
            // Voltage mismatch (compare with tolerance)
            match (primary_ch.voltage, sec_ch.voltage) {
                (Some(v1), Some(v2)) => {
                    if (v1 - v2).abs() > 0.01 {
                        mismatches.push(PinMismatch {
                            signal_name: primary_ch.signal_name.clone(),
                            field: "voltage".into(),
                            primary_value: format!("{:.2}V", v1),
                            secondary_value: format!("{:.2}V", v2),
                        });
                    }
                }
                (Some(_), None) | (None, Some(_)) => {
                    mismatches.push(PinMismatch {
                        signal_name: primary_ch.signal_name.clone(),
                        field: "voltage".into(),
                        primary_value: primary_ch.voltage.map(|v| format!("{:.2}V", v)).unwrap_or_else(|| "N/A".into()),
                        secondary_value: sec_ch.voltage.map(|v| format!("{:.2}V", v)).unwrap_or_else(|| "N/A".into()),
                    });
                }
                _ => {} // Both None — fine
            }
            match_count += 1;
        }
        // Pins only in primary are not mismatches — just not verified
    }

    // Pins only in secondary
    for sec_ch in &secondary.channels {
        let key = sec_ch.signal_name.to_lowercase();
        let in_primary = primary.channels.iter().any(|c| c.signal_name.to_lowercase() == key);
        if !in_primary {
            mismatches.push(PinMismatch {
                signal_name: sec_ch.signal_name.clone(),
                field: "missing".into(),
                primary_value: "not found".into(),
                secondary_value: format!("ch{}", sec_ch.channel),
            });
        }
    }

    let mismatch_count = mismatches.len();

    VerificationResult {
        primary,
        mismatches,
        match_count,
        mismatch_count,
    }
}

/* ═══════════════════════════════════════════════════════════════
 * DEVICE JSON SERIALIZATION
 *
 * Converts ExtractedPinTable to the JSON format expected by dc_json.c.
 * Writes to a temp file, returns the path.
 * ═══════════════════════════════════════════════════════════════ */

pub fn to_device_json(table: &ExtractedPinTable) -> Result<String, String> {
    let channels: Vec<serde_json::Value> = table
        .channels
        .iter()
        .map(|ch| {
            let dir = match ch.direction.to_lowercase().as_str() {
                "io" | "bidirectional" => 0,
                "input" | "in" => 1,
                "output" | "out" => 2,
                _ => 0,
            };
            serde_json::json!({
                "signal_name": ch.signal_name,
                "channel": ch.channel,
                "direction": dir
            })
        })
        .collect();

    let supplies: Vec<serde_json::Value> = table
        .supplies
        .iter()
        .map(|s| {
            serde_json::json!({
                "core_name": s.core_name,
                "voltage": s.voltage,
                "sequence_order": s.sequence_order,
                "ramp_delay_ms": s.ramp_delay_ms
            })
        })
        .collect();

    let mut bank_voltages = serde_json::Map::new();
    for bv in &table.bank_voltages {
        bank_voltages.insert(
            bv.bank_name.clone(),
            serde_json::Value::from(bv.voltage),
        );
    }

    let device = serde_json::json!({
        "device_name": table.device_name,
        "lot_id": "",
        "channels": channels,
        "supplies": supplies,
        "bank_voltages": bank_voltages,
        "steps": []
    });

    serde_json::to_string_pretty(&device)
        .map_err(|e| format!("JSON serialization error: {}", e))
}

/// Write device JSON to a temp file and return the path
pub fn write_device_json_temp(table: &ExtractedPinTable) -> Result<String, String> {
    let json = to_device_json(table)?;

    let temp_dir = std::env::temp_dir();
    let filename = format!("{}_device.json", table.device_name.replace(' ', "_"));
    let path = temp_dir.join(filename);

    std::fs::write(&path, &json)
        .map_err(|e| format!("Failed to write temp file: {}", e))?;

    Ok(path.to_string_lossy().into_owned())
}

/* ═══════════════════════════════════════════════════════════════
 * HELPERS
 * ═══════════════════════════════════════════════════════════════ */

/// Derive bank voltages from channel voltage annotations.
/// Groups channels by voltage, maps to Sonoma banks by channel range.
fn derive_bank_voltages(channels: &[ExtractedChannel]) -> Vec<BankVoltage> {
    // Sonoma bank layout: B13 = 0..47, B33 = 48..95, B34 = 96..127
    let banks = [
        ("B13", 0, 47),
        ("B33", 48, 95),
        ("B34", 96, 127),
    ];

    let mut result = Vec::new();

    for (name, start, end) in &banks {
        let voltages: Vec<f64> = channels
            .iter()
            .filter(|ch| ch.channel >= *start && ch.channel <= *end)
            .filter_map(|ch| ch.voltage)
            .collect();

        if !voltages.is_empty() {
            // Use the most common voltage for this bank
            let avg = voltages.iter().sum::<f64>() / voltages.len() as f64;
            // Round to nearest 0.1V
            let rounded = (avg * 10.0).round() / 10.0;
            result.push(BankVoltage {
                bank_name: name.to_string(),
                voltage: rounded,
            });
        }
    }

    result
}
