# FBC Protocol Specification

**Last Verified:** March 13, 2026  
**Source:** `firmware/src/fbc_protocol.rs` (verified against actual code)

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
| `BIM_STATUS_RSP` | 0x11 | Board → GUI | 12 bytes | BIM info response |
| `WRITE_BIM` | 0x20 | GUI → Board | 260 bytes | Write BIM EEPROM |
| `UPLOAD_VECTORS` | 0x21 | GUI → Board | Chunked | Upload test vectors |
| `CONFIGURE` | 0x30 | GUI → Board | 8 bytes | Configure clock/voltages |

### Runtime Commands (0x40-0xF1)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `START` | 0x40 | GUI → Board | 0 | Start test |
| `STOP` | 0x41 | GUI → Board | 0 | Stop test |
| `RESET` | 0x42 | GUI → Board | 0 | Reset decoder |
| `HEARTBEAT` | 0x50 | Board → GUI | 47 bytes | Periodic telemetry |
| `ERROR` | 0xE0 | Board → GUI | 12 bytes | Error notification |
| `STATUS_REQ` | 0xF0 | GUI → Board | 0 | Request status |
| `STATUS_RSP` | 0xF1 | Board → GUI | 47 bytes | Status response |

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
| `PMBUS_STATUS_REQ` | 0x84 | GUI → Board | 0 | Read PMBus status |
| `PMBUS_STATUS_RSP` | 0x85 | Board → GUI | Variable | Rail statuses |
| `PMBUS_ENABLE` | 0x86 | GUI → Board | 2 bytes | Addr + enable |
| `EMERGENCY_STOP` | 0x8F | GUI → Board | 0 | Kill all power |
| `POWER_SEQUENCE_ON` | 0x90 | GUI → Board | 12 bytes | 6 voltages |
| `POWER_SEQUENCE_OFF` | 0x91 | GUI → Board | 0 | Power down sequence |

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
| `LOAD` | 0xB2 | GUI → Board | Chunked | Load from SD cache |
| `LOAD_ACK` | 0xB3 | Board → GUI | 1 byte | Acknowledge |
| `START` | 0xB4 | GUI → Board | 0 | Start vectors |
| `PAUSE` | 0xB5 | GUI → Board | 0 | Pause execution |
| `RESUME` | 0xB6 | GUI → Board | 0 | Resume execution |
| `STOP` | 0xB7 | GUI → Board | 0 | Stop execution |

### Fast Pins Commands (0xD0-0xD2)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `READ_REQ` | 0xD0 | GUI → Board | 0 | Read fast pins |
| `READ_RSP` | 0xD1 | Board → GUI | 6 bytes | din, dout, oen |
| `WRITE` | 0xD2 | GUI → Board | 8 bytes | dout + oen |

### Error Log Commands (0x4A-0x4B)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `ERROR_LOG_REQ` | 0x4A | GUI → Board | 8 bytes | Start index + count |
| `ERROR_LOG_RSP` | 0x4B | Board → GUI | 232 bytes | 8 entries × 28 bytes |

### Flight Recorder Commands (0x60-0x63)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `LOG_READ_REQ` | 0x60 | GUI → Board | 4 bytes | Sector number |
| `LOG_READ_RSP` | 0x61 | Board → GUI | 517 bytes | SD sector + status |
| `LOG_INFO_REQ` | 0x62 | GUI → Board | 0 | Request log info |
| `LOG_INFO_RSP` | 0x63 | Board → GUI | 16 bytes | Log metadata |

### Firmware Update Commands (0xE1-0xE9)

| Command | Code | Direction | Payload | Description |
|---------|------|-----------|---------|-------------|
| `INFO_REQ` | 0xE1 | GUI → Board | 0 | Get firmware info |
| `INFO_RSP` | 0xE2 | Board → GUI | 32 bytes | Version + build |
| `BEGIN` | 0xE3 | GUI → Board | 8 bytes | Total size + checksum |
| `BEGIN_ACK` | 0xE4 | Board → GUI | 2 bytes | Status + max chunk |
| `CHUNK` | 0xE5 | GUI → Board | Chunked | Offset + data |
| `CHUNK_ACK` | 0xE6 | Board → GUI | 5 bytes | Offset + status |
| `COMMIT` | 0xE7 | GUI → Board | 0 | Finalize update |
| `COMMIT_ACK` | 0xE8 | Board → GUI | 8 bytes | Status + checksum |
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
| Command codes | `fbc_protocol.rs` | 28-125 | ✅ |
| Header format | `fbc_protocol.rs` | 127-190 | ✅ |
| Payload structs | `fbc_protocol.rs` | 200-800 | ✅ |
| Handler dispatch | `fbc_protocol.rs` | 1000-1500 | ✅ |

---

**Related:**
- `docs/FIRMWARE.md` — Firmware architecture
- `firmware/src/fbc_protocol.rs` — Actual protocol code (authoritative)
