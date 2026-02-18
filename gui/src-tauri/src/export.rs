//! Test Results Export Module
//!
//! Exports test results to various formats:
//! - CSV (Excel compatible)
//! - JSON (API compatible)
//! - STDF (Industry standard - Standard Test Data Format)

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

/// Export options from the UI
#[derive(Debug, Deserialize)]
pub struct ExportOptions {
    pub format: String,  // "csv", "json", "stdf"
    pub include_raw_data: bool,
    pub include_waveforms: bool,
    pub time_range: String,  // "all", "last-hour", "custom"
    pub custom_start: Option<i64>,
    pub custom_end: Option<i64>,
}

/// Test result data to export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub timestamp: i64,
    pub mac: String,
    pub board_id: u16,
    pub serial: u32,
    pub state: String,
    pub cycles: u64,
    pub errors: u32,
    pub temp_c: f32,
    pub duration_ms: u64,
    pub vectors_loaded: u32,
    pub vectors_executed: u32,
    pub error_rate_ppm: f32,
}

/// Statistics about the export
#[derive(Debug, Serialize)]
pub struct ExportStats {
    pub rows_exported: u32,
    pub file_size_bytes: u64,
    pub duration_ms: u64,
}

/// Export results to CSV format
pub fn export_csv(
    data: &[TestResult],
    path: &PathBuf,
    options: &ExportOptions,
) -> Result<ExportStats, String> {
    let start = Instant::now();

    let mut file = File::create(path)
        .map_err(|e| format!("Failed to create file: {}", e))?;

    // Write header
    writeln!(
        file,
        "Timestamp,MAC,BoardID,Serial,State,Cycles,Errors,Temp_C,Duration_ms,Vectors_Loaded,Vectors_Executed,Error_Rate_PPM"
    ).map_err(|e| format!("Write error: {}", e))?;

    // Filter by time range
    let filtered: Vec<_> = filter_by_time_range(data, options);

    // Write data rows
    for row in &filtered {
        writeln!(
            file,
            "{},{},{},{},{},{},{},{:.1},{},{},{},{:.2}",
            row.timestamp,
            row.mac,
            row.board_id,
            row.serial,
            row.state,
            row.cycles,
            row.errors,
            row.temp_c,
            row.duration_ms,
            row.vectors_loaded,
            row.vectors_executed,
            row.error_rate_ppm
        ).map_err(|e| format!("Write error: {}", e))?;
    }

    let file_size = file.metadata()
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(ExportStats {
        rows_exported: filtered.len() as u32,
        file_size_bytes: file_size,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Export results to JSON format
pub fn export_json(
    data: &[TestResult],
    path: &PathBuf,
    options: &ExportOptions,
) -> Result<ExportStats, String> {
    let start = Instant::now();

    // Filter by time range
    let filtered: Vec<_> = filter_by_time_range(data, options);

    // Build JSON structure
    let export_data = serde_json::json!({
        "export_timestamp": chrono::Utc::now().to_rfc3339(),
        "format_version": "1.0",
        "total_records": filtered.len(),
        "options": {
            "include_raw_data": options.include_raw_data,
            "include_waveforms": options.include_waveforms,
            "time_range": options.time_range,
        },
        "results": filtered,
    });

    let json_string = serde_json::to_string_pretty(&export_data)
        .map_err(|e| format!("JSON serialization error: {}", e))?;

    let mut file = File::create(path)
        .map_err(|e| format!("Failed to create file: {}", e))?;

    file.write_all(json_string.as_bytes())
        .map_err(|e| format!("Write error: {}", e))?;

    let file_size = file.metadata()
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(ExportStats {
        rows_exported: filtered.len() as u32,
        file_size_bytes: file_size,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Export results to STDF (Standard Test Data Format)
/// STDF is the semiconductor industry standard for test data
pub fn export_stdf(
    data: &[TestResult],
    path: &PathBuf,
    options: &ExportOptions,
) -> Result<ExportStats, String> {
    let start = Instant::now();

    // Filter by time range
    let filtered: Vec<_> = filter_by_time_range(data, options);

    let mut file = File::create(path)
        .map_err(|e| format!("Failed to create file: {}", e))?;

    // STDF binary format header (FAR record)
    // Record length (2 bytes) + record type (1 byte) + record sub-type (1 byte)
    let far_record: [u8; 6] = [
        0x02, 0x00,  // Record length (2 bytes, little-endian)
        0x00,        // Record type: FAR
        0x0A,        // Record sub-type
        0x02,        // CPU type (2 = PC)
        0x04,        // STDF version (4)
    ];
    file.write_all(&far_record)
        .map_err(|e| format!("Write error: {}", e))?;

    // MIR (Master Information Record)
    let setup_time = chrono::Utc::now().timestamp() as u32;
    let mir_data = build_mir_record(setup_time);
    file.write_all(&mir_data)
        .map_err(|e| format!("Write error: {}", e))?;

    // PIR (Part Information Record) and PRR (Part Results Record) for each part
    for (idx, result) in filtered.iter().enumerate() {
        // PIR - start of part
        let pir = build_pir_record(idx as u16);
        file.write_all(&pir)
            .map_err(|e| format!("Write error: {}", e))?;

        // PRR - part results
        let prr = build_prr_record(idx as u16, result);
        file.write_all(&prr)
            .map_err(|e| format!("Write error: {}", e))?;
    }

    // MRR (Master Results Record) - end of file
    let mrr = build_mrr_record(chrono::Utc::now().timestamp() as u32);
    file.write_all(&mrr)
        .map_err(|e| format!("Write error: {}", e))?;

    let file_size = file.metadata()
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(ExportStats {
        rows_exported: filtered.len() as u32,
        file_size_bytes: file_size,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Filter results by time range
fn filter_by_time_range(data: &[TestResult], options: &ExportOptions) -> Vec<TestResult> {
    let now = chrono::Utc::now().timestamp_millis();

    match options.time_range.as_str() {
        "all" => data.to_vec(),
        "last-hour" => {
            let one_hour_ago = now - 3600 * 1000;
            data.iter()
                .filter(|r| r.timestamp >= one_hour_ago)
                .cloned()
                .collect()
        }
        "custom" => {
            let start = options.custom_start.unwrap_or(0);
            let end = options.custom_end.unwrap_or(now);
            data.iter()
                .filter(|r| r.timestamp >= start && r.timestamp <= end)
                .cloned()
                .collect()
        }
        _ => data.to_vec(),
    }
}

// STDF record builders

fn build_mir_record(setup_time: u32) -> Vec<u8> {
    let mut data = Vec::new();

    // Record header will be added at the end
    let rec_type: u8 = 1;   // MIR
    let rec_sub: u8 = 10;

    // MIR fields
    data.extend_from_slice(&setup_time.to_le_bytes()); // SETUP_T
    data.extend_from_slice(&setup_time.to_le_bytes()); // START_T
    data.push(1);  // STAT_NUM (station number)
    data.push(b' '); // MODE_COD (test mode: production)
    data.push(b' '); // RTST_COD (retest code)
    data.push(b' '); // PROT_COD (protection code)
    data.extend_from_slice(&0u16.to_le_bytes()); // BURN_TIM

    // Add string fields (length-prefixed)
    add_stdf_string(&mut data, b"C"); // CMOD_COD
    add_stdf_string(&mut data, b"FBC-001"); // LOT_ID
    add_stdf_string(&mut data, b"BURN-IN"); // PART_TYP
    add_stdf_string(&mut data, b"FBC System"); // NODE_NAM
    add_stdf_string(&mut data, b"FBC"); // TSTR_TYP
    add_stdf_string(&mut data, b"FBC System v2.0"); // JOB_NAM
    add_stdf_string(&mut data, b""); // JOB_REV
    add_stdf_string(&mut data, b""); // SBLOT_ID
    add_stdf_string(&mut data, b"ISE Labs"); // OPER_NAM
    add_stdf_string(&mut data, b""); // EXEC_TYP
    add_stdf_string(&mut data, b""); // EXEC_VER
    add_stdf_string(&mut data, b""); // TEST_COD
    add_stdf_string(&mut data, b""); // TST_TEMP
    add_stdf_string(&mut data, b""); // USER_TXT
    add_stdf_string(&mut data, b""); // AUX_FILE
    add_stdf_string(&mut data, b""); // PKG_TYP
    add_stdf_string(&mut data, b"FBC Burn-in"); // FAMLY_ID
    add_stdf_string(&mut data, b""); // DATE_COD
    add_stdf_string(&mut data, b"ISE Labs"); // FACIL_ID
    add_stdf_string(&mut data, b""); // FLOOR_ID
    add_stdf_string(&mut data, b"FBC"); // PROC_ID
    add_stdf_string(&mut data, b""); // OPER_FRQ
    add_stdf_string(&mut data, b""); // SPEC_NAM
    add_stdf_string(&mut data, b""); // SPEC_VER
    add_stdf_string(&mut data, b""); // FLOW_ID
    add_stdf_string(&mut data, b""); // SETUP_ID
    add_stdf_string(&mut data, b""); // DSGN_REV
    add_stdf_string(&mut data, b""); // ENG_ID
    add_stdf_string(&mut data, b""); // ROM_COD
    add_stdf_string(&mut data, b""); // SERL_NUM
    add_stdf_string(&mut data, b""); // SUPR_NAM

    // Build final record with header
    let rec_len = data.len() as u16;
    let mut record = Vec::new();
    record.extend_from_slice(&rec_len.to_le_bytes());
    record.push(rec_type);
    record.push(rec_sub);
    record.extend(data);

    record
}

fn build_pir_record(part_idx: u16) -> Vec<u8> {
    let mut record = Vec::new();
    let rec_len: u16 = 2;
    record.extend_from_slice(&rec_len.to_le_bytes());
    record.push(5);  // PIR type
    record.push(10); // PIR sub-type
    record.push(1);  // HEAD_NUM
    record.push((part_idx % 255) as u8); // SITE_NUM
    record
}

fn build_prr_record(part_idx: u16, result: &TestResult) -> Vec<u8> {
    let mut data = Vec::new();

    // PRR fields
    data.push(1);  // HEAD_NUM
    data.push((part_idx % 255) as u8); // SITE_NUM
    data.push(if result.errors > 0 { 8 } else { 0 }); // PART_FLG (bit 3 = fail)
    data.extend_from_slice(&0u16.to_le_bytes()); // NUM_TEST
    data.extend_from_slice(&0u16.to_le_bytes()); // HARD_BIN
    data.extend_from_slice(&0u16.to_le_bytes()); // SOFT_BIN
    data.extend_from_slice(&0i16.to_le_bytes()); // X_COORD
    data.extend_from_slice(&0i16.to_le_bytes()); // Y_COORD
    data.extend_from_slice(&(result.duration_ms as u32).to_le_bytes()); // TEST_T
    add_stdf_string(&mut data, result.serial.to_string().as_bytes()); // PART_ID
    add_stdf_string(&mut data, b""); // PART_TXT
    data.push(0); // PART_FIX (length = 0)

    // Build final record with header
    let rec_len = data.len() as u16;
    let mut record = Vec::new();
    record.extend_from_slice(&rec_len.to_le_bytes());
    record.push(5);  // PRR type
    record.push(20); // PRR sub-type
    record.extend(data);

    record
}

fn build_mrr_record(finish_time: u32) -> Vec<u8> {
    let mut data = Vec::new();

    data.extend_from_slice(&finish_time.to_le_bytes()); // FINISH_T
    data.push(b' '); // DISP_COD
    add_stdf_string(&mut data, b""); // USR_DESC
    add_stdf_string(&mut data, b""); // EXC_DESC

    let rec_len = data.len() as u16;
    let mut record = Vec::new();
    record.extend_from_slice(&rec_len.to_le_bytes());
    record.push(1);  // MRR type
    record.push(20); // MRR sub-type
    record.extend(data);

    record
}

fn add_stdf_string(data: &mut Vec<u8>, s: &[u8]) {
    data.push(s.len() as u8);
    data.extend_from_slice(s);
}

/// Main export function - dispatches to appropriate format handler
pub fn export_results(
    data: &[TestResult],
    path: &PathBuf,
    options: &ExportOptions,
) -> Result<ExportStats, String> {
    match options.format.as_str() {
        "csv" => export_csv(data, path, options),
        "json" => export_json(data, path, options),
        "stdf" => export_stdf(data, path, options),
        _ => Err(format!("Unknown export format: {}", options.format)),
    }
}
