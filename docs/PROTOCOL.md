# FBC Protocol Specification

**Last Verified:** March 26, 2026
**Source:** `firmware/src/fbc_protocol.rs` — 79 commands across 13 subsystems

---

## Overview

Raw Ethernet protocol for FBC burn-in system.

**Characteristics:**
- EtherType: `0x88B5`
- No IP, no TCP, no UDP
- Big-endian byte order
- 8-byte header + variable payload

---

## Packet Format

```
Ethernet Frame
├─ 14 bytes: Ethernet header
│  ├─ Destination MAC (6 bytes)
│  ├─ Source MAC (6 bytes)
│  └─ EtherType (2 bytes) = 0x88B5
│
├─ 8 bytes: FBC header
│  ├─ Magic (2 bytes) = 0xFBC0
│  ├─ Sequence (2 bytes)
│  ├─ Command (1 byte)
│  ├─ Flags (1 byte) = 0
│  └─ Length (2 bytes)
│
└─ N bytes: Payload (big-endian)
```

### FBC Header Structure

```rust
#[repr(C, packed)]
pub struct FbcHeader {
    pub magic: u16,    // 0xFBC0
    pub seq: u16,      // Sequence number
    pub cmd: u8,       // Command code
    pub flags: u8,     // Reserved (0)
    pub length: u16,   // Payload length
}
```

---

## Command Reference

### Setup Commands (0x01-0x30)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `ANNOUNCE` | 0x01 | Board → GUI | 16 bytes | Sent on boot |
| `BIM_STATUS_REQ` | 0x10 | GUI → Board | 0 | Request BIM info |
| `BIM_STATUS_RSP` | 0x11 | Board → GUI | — | **Orphaned**: handler returns ANNOUNCE (0x01, 16B) instead |
| `WRITE_BIM` | 0x20 | GUI → Board | 260 bytes | Write BIM EEPROM |
| `UPLOAD_VECTORS` | 0x21 | GUI → Board | Chunked | Upload test vectors |
| `UPLOAD_TO_SLOT` | 0x22 | GUI → Board | [pattern_id:u8][offset:u32][total:u32][len:u16][data...] | Upload .fbc to SD card (chunked at 1400B) |
| `SLOT_STATUS_REQ` | 0x23 | GUI → Board | 0 | Request pattern directory from SD |
| `SLOT_STATUS_RSP` | 0x24 | Board → GUI | [count:u16 BE] + N×14B | [pattern_id:u8][flags:u8][num_vectors:u32][size:u32][vec_clock_hz:u32] |
| `INVALIDATE_SLOT` | 0x25 | GUI → Board | [pattern_id:u8] | Clear pattern (0xFF = all) |
| `SET_PLAN` | 0x26 | GUI → Board | [num_steps:u8][loop_start:u8][total_duration:u32 BE] + N×[pattern_id:u8][duration:u32 BE][fail_action:u8][threshold:u32 BE][temp_dc:i16 BE][clk_div:u8] | Upload test plan (13B/step, up to 96 steps) |
| `SET_PLAN_ACK` | 0x27 | Board → GUI | [status:u8] | 0=OK |
| `RUN_PLAN` | 0x28 | GUI → Board | 0 | Start executing loaded plan |
| `RUN_PLAN_ACK` | 0x29 | Board → GUI | [status:u8] | 0=OK |
| `PLAN_STATUS_REQ` | 0x2A | GUI → Board | 0 | Request plan executor state |
| `PLAN_STATUS_RSP` | 0x2B | Board → GUI | 15B | [state:u8][step:u8][total:u8][loop_count:u32][elapsed:u32][errors:u32] |
| `STEP_RESULT` | 0x2C | Board → GUI | 11B | [step:u8][slot:u8][errors:u32][duration_ms:u32][passed:u8] |
| `CONFIGURE` | 0x30 | GUI → Board | 18 bytes | Configure clock/voltages |
| `IO_BANK_SET` | 0x35 | GUI → Board | [bank:u8][voltage_mv:u16 BE] | Set IO bank voltage via I2C regulator |
| `IO_BANK_SET_ACK` | 0x36 | Board → GUI | [status:u8] | 0=OK |

### Runtime Commands (0x40-0xF3)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `START` | 0x40 | GUI → Board | 0 | Start test |
| `STOP` | 0x41 | GUI → Board | 0 | Stop test |
| `RESET` | 0x42 | GUI → Board | 0 | Reset decoder |
| `HEARTBEAT` | 0x50 | Board → GUI | 11 bytes | Periodic telemetry |
| `ERROR` | 0xE0 | Board → GUI | 13 bytes | Error notification |
| `STATUS_REQ` | 0xF0 | GUI → Board | 0 | Request status |
| `STATUS_RSP` | 0xF1 | Board → GUI | 47 bytes | Status response |
| `MIN_MAX_REQ` | 0xF2 | GUI → Board | 0 | Request XADC min/max tracking |
| `MIN_MAX_RSP` | 0xF3 | Board → GUI | 32B | 4 channels × (min_i32 + max_i32): VCCINT, VCCAUX, VCCBRAM, temp |

### Analog Commands (0x70-0x71)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `READ_ALL_REQ` | 0x70 | GUI → Board | 0 | Read 32 channels |
| `READ_ALL_RSP` | 0x71 | Board → GUI | 192 bytes | 32 × (raw + scaled) |

### Power Commands (0x80-0x91)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `VICOR_STATUS_REQ` | 0x80 | GUI → Board | 0 | Read VICOR status |
| `VICOR_STATUS_RSP` | 0x81 | Board → GUI | 30 bytes | 6 cores × 5 bytes |
| `VICOR_ENABLE` | 0x82 | GUI → Board | 1 byte | Core mask |
| `VICOR_SET_VOLTAGE` | 0x83 | GUI → Board | 3 bytes | Core + voltage |
| `PMBUS_STATUS_REQ` | 0x84 | GUI → Board | 0 | **GAP**: defined but no handler — silently dropped |
| `PMBUS_STATUS_RSP` | 0x85 | Board → GUI | Variable | **GAP**: never built (no handler for REQ) |
| `PMBUS_ENABLE` | 0x86 | GUI → Board | 2 bytes | Addr + enable |
| `EMERGENCY_STOP` | 0x8F | GUI → Board | 0 | Kill all power |
| `POWER_SEQUENCE_ON` | 0x90 | GUI → Board | 12 bytes | 6 voltages |
| `POWER_SEQUENCE_OFF` | 0x91 | GUI → Board | 0 | Power down sequence |

### Board Config Commands (0x31-0x34)

Runtime overrides without touching EEPROM. Overrides are volatile (lost on power cycle).

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `SET_OVERRIDE` | 0x31 | GUI → Board | 3 bytes | field_id + value (i16 BE) |
| `CLEAR_OVERRIDES` | 0x32 | GUI → Board | 0 | Revert all to EEPROM defaults |
| `GET_EFFECTIVE` | 0x33 | GUI → Board | 0 | Request merged config |
| `EFFECTIVE_RSP` | 0x34 | Board → GUI | 114 bytes | 8 rails × 6B + 16 vcal × 2B + 16 ical × 2B + temp 2B |

**SET_OVERRIDE field IDs:**

| Range | Meaning | Value type |
|-------|---------|-----------|
| 0x01-0x08 | Rail 1-8 max_voltage_mv | u16 (as i16) |
| 0x11-0x18 | Rail 1-8 min_voltage_mv | u16 (as i16) |
| 0x21-0x28 | Rail 1-8 max_current_ma | u16 (as i16) |
| 0x40-0x4F | Voltage cal offset ch 0-15 | i16 (mV, signed) |
| 0x50-0x5F | Current cal offset ch 0-15 | i16 (mA, signed) |
| 0x80 | Temperature setpoint | i16 (0.1°C units) |

**EFFECTIVE_RSP payload (114 bytes):**
```
Bytes 0-47:   8 rails × (max_v:u16 + min_v:u16 + max_i:u16) = 48 bytes
Bytes 48-79:  16 × voltage_cal:i16 = 32 bytes
Bytes 80-111: 16 × current_cal:i16 = 32 bytes
Bytes 112-113: temp_setpoint:i16 = 2 bytes
```

### EEPROM Commands (0xA0-0xA3)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `READ_REQ` | 0xA0 | GUI → Board | 2 bytes | Offset + length |
| `READ_RSP` | 0xA1 | Board → GUI | 64 bytes | EEPROM data |
| `WRITE` | 0xA2 | GUI → Board | 66 bytes | Offset + data |
| `WRITE_ACK` | 0xA3 | Board → GUI | 1 byte | Status |

### Vector Commands (0xB0-0xB7)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `STATUS_REQ` | 0xB0 | GUI → Board | 0 | Vector engine status |
| `STATUS_RSP` | 0xB1 | Board → GUI | 32 bytes | Status fields |
| `LOAD` | 0xB2 | GUI → Board | Chunked | **GAP**: defined but no handler |
| `LOAD_ACK` | 0xB3 | Board → GUI | 1 byte | **GAP**: never built |
| `START` | 0xB4 | GUI → Board | 0 | **GAP**: defined but no handler (use runtime::START 0x40) |
| `PAUSE` | 0xB5 | GUI → Board | 0 | Pause execution |
| `RESUME` | 0xB6 | GUI → Board | 0 | Resume execution |
| `STOP` | 0xB7 | GUI → Board | 0 | **GAP**: defined but no handler (use runtime::STOP 0x41) |

### Fast Pins Commands (0xD0-0xD2)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `READ_REQ` | 0xD0 | GUI → Board | 0 | Read fast pins |
| `READ_RSP` | 0xD1 | Board → GUI | 12 bytes | din:u32 + dout:u32 + oen:u32 |
| `WRITE` | 0xD2 | GUI → Board | 8 bytes | dout + oen |

### Error Log Commands (0x4A-0x4B)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `ERROR_LOG_REQ` | 0x4A | GUI → Board | 8 bytes | Start index + count |
| `ERROR_LOG_RSP` | 0x4B | Board → GUI | 232 bytes | 8 entries × 28 bytes |

### Flight Recorder Commands (0x60-0x67)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `LOG_READ_REQ` | 0x60 | GUI → Board | 4 bytes | Sector number |
| `LOG_READ_RSP` | 0x61 | Board → GUI | 517 bytes | SD sector + status |
| `LOG_INFO_REQ` | 0x62 | GUI → Board | 0 | Request log info |
| `LOG_INFO_RSP` | 0x63 | Board → GUI | 22 bytes | Log metadata |
| `SD_FORMAT` | 0x64 | GUI → Board | 0 | Format SD card (erase all logs) |
| `SD_FORMAT_ACK` | 0x65 | Board → GUI | 1 byte | Status (0=OK) |
| `SD_REPAIR` | 0x66 | GUI → Board | 0 | Repair SD card (fix header/index) |
| `SD_REPAIR_ACK` | 0x67 | Board → GUI | 2 bytes | Status + health |

### Firmware Update Commands (0xE1-0xE9)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `INFO_REQ` | 0xE1 | GUI → Board | 0 | Get firmware info |
| `INFO_RSP` | 0xE2 | Board → GUI | 32 bytes | Version + build |
| `BEGIN` | 0xE3 | GUI → Board | 8 bytes | Total size + checksum |
| `BEGIN_ACK` | 0xE4 | Board → GUI | 3 bytes | Status + max chunk |
| `CHUNK` | 0xE5 | GUI → Board | Chunked | Offset + data |
| `CHUNK_ACK` | 0xE6 | Board → GUI | 5 bytes | Offset + status |
| `COMMIT` | 0xE7 | GUI → Board | 0 | Finalize update |
| `COMMIT_ACK` | 0xE8 | Board → GUI | 9 bytes | Status + size + checksum |
| `ABORT` | 0xE9 | GUI → Board | 0 | Cancel update |

---

## Payload Structures

### ANNOUNCE Payload (16 bytes)

```rust
pub struct AnnouncePayload {
    pub mac: [u8; 6],           // Board MAC address
    pub bim_type: u8,           // BIM type (0=none)
    pub serial: u32,            // Board serial number
    pub hw_revision: u8,        // Hardware revision
    pub fw_version: u16,        // Firmware version (0x0100 = v1.0)
    pub has_bim: u8,            // 0=no BIM, 1=BIM detected
    pub bim_programmed: u8,     // 0=blank, 1=programmed
}
```

### STATUS_RSP Payload (47 bytes)

```rust
pub struct StatusPayload {
    pub cycles: u32,            // Cycle count
    pub errors: u32,            // Error count
    pub temp_c: i16,            // Die temperature (0.1°C units)
    pub state: u8,              // ControllerState
    pub rail_voltage_mv: [u16; 8],  // [Core1-6, VDD_IO, VDD_3V3]
    pub rail_current_ma: [u16; 8],  // [Core1-6, 0, 0]
    pub fpga_vccint_mv: u16,    // FPGA VCCINT
    pub fpga_vccaux_mv: u16,    // FPGA VCCAUX
}
```

### ERROR_LOG_RSP Payload (232 bytes)

```rust
pub struct ErrorLogRspPayload {
    pub total_errors: u32,      // Total errors recorded
    pub num_entries: u32,       // Number of entries (max 8)
    pub entries: [ErrorLogEntry; 8],
}

pub struct ErrorLogEntry {
    pub pattern: [u32; 4],      // 128-bit error mask
    pub vector: u32,            // Vector number
    pub cycle_lo: u32,          // Cycle count (low)
    pub cycle_hi: u32,          // Cycle count (high)
}
```

### CONFIGURE Payload (18 bytes)

```rust
pub struct ConfigPayload {
    pub clock_div: u8,              // Clock divider
    pub core_voltage_mv: [u16; 6],  // VICOR core voltages (BE)
    pub reserved: [u8; 5],          // Reserved
}
```

### HEARTBEAT Payload (11 bytes)

```rust
pub struct HeartbeatPayload {
    pub cycles: u32,        // Cycle count
    pub errors: u32,        // Error count
    pub temp_c: i16,        // Die temperature (0.1°C units)
    pub state: u8,          // ControllerState
}
```

### ERROR Payload (13 bytes)

```rust
pub struct ErrorPayload {
    pub error_type: u8,     // Error type code
    pub cycle: u32,         // Cycle at which error occurred
    pub error_count: u32,   // Total error count
    pub details: u32,       // Error-type specific details
}
```

### UPLOAD_VECTORS Chunk Format

```
Bytes 0-3:   offset (u32 BE) — byte offset into vector buffer
Bytes 4-7:   total_size (u32 BE) — total upload size
Bytes 8-9:   chunk_size (u16 BE) — this chunk's data size
Bytes 10+:   data (up to 1468 bytes per chunk)
```

### VECTOR_STATUS_RSP Payload (33 bytes)

Custom packed format — not a named struct (fbc_protocol.rs:1584-1592):
```
Byte 0:      state (u8) — ControllerState enum
Bytes 1-4:   current_address (u32 BE) — always 0 (not tracked)
Bytes 5-8:   vector_count (u32 BE) — total_vectors
Bytes 9-12:  loop_count (u32 BE) — always 0 (not tracked)
Bytes 13-16: target_loops (u32 BE) — always 0 (not tracked)
Bytes 17-20: error_count (u32 BE)
Bytes 21-24: first_fail_addr (u32 BE)
Bytes 25-32: cycle_count (u64 BE) — run_time_ms
```

### LOG_INFO_RSP Payload (22 bytes)

```rust
pub struct LogInfoRspPayload {
    pub sd_present: u8,         // 0=no SD, 1=present
    pub sd_health: u8,          // SdHealth enum
    pub data_start: u32,        // First data sector
    pub capacity: u32,          // Total sectors available
    pub current_index: u32,     // Current write position
    pub total_entries: u32,     // Total log entries written
}
```

### LOG_READ_RSP Payload (517 bytes)

```
Bytes 0-3:   sector (u32 BE) — requested sector
Byte 4:      status (u8) — 0=OK, 1=error
Bytes 5-516: data (512 bytes) — raw SD sector contents
```

### FIRMWARE_INFO_RSP Payload (20 bytes)

```rust
pub struct FirmwareInfoRspPayload {
    pub version_major: u8,          // byte 0
    pub version_minor: u8,          // byte 1
    pub version_patch: u8,          // byte 2
    pub build_date: [u8; 10],       // bytes 3-12, "2026-03-17"
    pub board_serial: u32,          // bytes 13-16 (BE)
    pub hw_revision: u8,            // byte 17
    pub bootloader_version: u8,     // byte 18
    // byte 19: bit-packed — (sd_present << 1) | update_in_progress
    pub sd_present: u8,             // bit 1 of byte 19
    pub update_in_progress: u8,     // bit 0 of byte 19
}
```

### FIRMWARE_BEGIN_ACK Payload (3 bytes)

```
Byte 0:   status (u8) — 0=ready, 1=no SD, 2=error, 3=already in progress
Bytes 1-2: max_chunk_size (u16 BE)
```

### FIRMWARE_CHUNK_ACK Payload (5 bytes)

```
Bytes 0-3: offset (u32 BE) — echoed offset
Byte 4:    status (u8) — 0=OK, 1=write error, 2=offset mismatch
```

### FIRMWARE_COMMIT_ACK Payload (9 bytes)

```
Byte 0:    status (u8) — 0=OK, 1=checksum mismatch, 2=size mismatch
Bytes 1-4: received_size (u32 BE)
Bytes 5-8: computed_checksum (u32 BE)
```

---

## Handler Response Patterns

**Immediate ACK** (returns packet inline, no payload):
- START, STOP, RESET, PAUSE, RESUME
- EMERGENCY_STOP, SET_OVERRIDE, CLEAR_OVERRIDES
- FIRMWARE_ABORT

**Immediate Response with Payload**:
- BIM_STATUS_REQ → AnnouncePayload
- STATUS_REQ → StatusPayload (47B)
- VECTOR_STATUS_REQ → 33-byte packed status

**Deferred** (handler queues request, main.rs polls and sends response):
- All Flight Recorder commands (SD I/O is slow)
- All Analog commands (ADC reads are slow)
- All Power/VICOR/PMBus commands (I2C/GPIO is slow)
- All EEPROM read/write (I2C is slow)
- All Fast Pins operations
- All Firmware update commands (except ABORT)
- Error log requests
- Board Config GET_EFFECTIVE

**Special Cases**:
- FIRMWARE_CHUNK: Immediate error ACK on offset mismatch, otherwise deferred
- WRITE_BIM: Validates magic (0xBEEFCAFE) & CRC32 before queuing; returns error ACK if invalid

---

## Communication Flow

### Discovery

```
Board boots
    │
    ▼
Send ANNOUNCE (broadcast)
    │
    ▼
GUI receives ANNOUNCE
    │
    ▼
GUI stores board info (MAC, serial, etc.)
```

### Test Execution

```
GUI                            Board
 │                               │
 ├──UPLOAD_VECTORS (chunked)──▶ │
 │                               │
 │◀──────UPLOAD_VECTORS_ACK─────┤
 │                               │
 │──────────START──────────────▶│
 │                               │
 │◀──────START_ACK──────────────┤
 │                               │
 │◀──────HEARTBEAT (periodic)───┤
 │                               │
 │◀──────ERROR (if error)───────┤
 │                               │
 │──────────STOP───────────────▶│
 │                               │
 │◀──────STOP_ACK───────────────┤
```

### Error Log Read

```
GUI                            Board
 │                               │
 │────ERROR_LOG_REQ(start, n)──▶│
 │                               │
 │◀────ERROR_LOG_RSP(entries)───┤
 │                               │
 │  Entries: pattern, vector,    │
 │  cycle for each error         │
```

---

## Byte Order

All multi-byte fields are **big-endian** (network byte order).

Example:
```rust
// u32 to big-endian bytes
let bytes = value.to_be_bytes();

// Big-endian bytes to u32
let value = u32::from_be_bytes(bytes);
```

---

## Timing

| Operation | Timeout | Notes |
|-----------|---------|-------|
| Command response | 500ms | Most commands |
| UPLOAD_VECTORS chunk | 1s | Per chunk |
| ERROR_LOG_REQ | 1s | Per request |
| Heartbeat interval | 100ms | During test execution |

---

## Error Codes

| Error | Code | Description |
|-------|------|-------------|
| `OK` | 0 | Success |
| `INVALID_PARAM` | 1 | Invalid parameter |
| `BUSY` | 2 | Resource busy |
| `TIMEOUT` | 3 | Operation timed out |
| `NOT_FOUND` | 4 | Resource not found |

---

## Verified Against Code

| Component | File | Lines | Verified |
|-----------|------|-------|----------|
| Command codes (63 total) | `fbc_protocol.rs` | 28-132 | ✅ March 24 |
| Header format | `fbc_protocol.rs` | 134-190 | ✅ |
| Payload structs | `fbc_protocol.rs` | 200-800 | ✅ |
| Handler dispatch (36 match arms) | `fbc_protocol.rs` | 1253-1314 | ✅ March 24 |
| Handler implementations | `fbc_protocol.rs` | 1393-1971 | ✅ March 24 |

**Command count:** 63 command codes across 11 subsystems.
36 dispatch match arms route inbound commands to handler functions.
**Unhandled codes** (defined but no dispatch): PMBUS_STATUS_REQ (0x84), vector::LOAD (0xB2), vector::START (0xB4), vector::STOP (0xB7).

---

**Related:**
- `firmware/src/fbc_protocol.rs` — Actual protocol code (authoritative)
- `docs/FBC.md` — FBC system reference (GUI panel mapping, FbcClient methods)
- `docs/SONOMA_VS_FBC.md` — Side-by-side protocol comparison
