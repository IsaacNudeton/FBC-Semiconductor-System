# FBC System Operational Workflow

This document describes the end-to-end workflow from test pattern creation to burn-in test execution.

---

## Table of Contents

1. [System Architecture Overview](#system-architecture-overview)
2. [Data Flow Diagram](#data-flow-diagram)
3. [Test Plan Architecture](#test-plan-architecture)
4. [GUI Design Philosophy](#gui-design-philosophy)
5. [Detailed Phase Breakdown](#detailed-phase-breakdown)
6. [Board Identification](#board-identification)
7. [EEPROM Storage](#eeprom-storage)
8. [File Formats Reference](#file-formats-reference)
9. [Workflow Summary](#workflow-summary)
10. [Autonomous Operation](#autonomous-operation)
11. [FAQ](#faq)
12. [Sonoma/Everest to FBC Mapping](#sonomaeverest-to-fbc-mapping)

---

## System Architecture Overview

```
                           TEST PLAN CREATION (Offline)
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   Customer STIL/AVC files     Device Config (.bim/.map/.tim)                │
│         │                              │                                    │
│         ▼                              ▼                                    │
│   ┌─────────────┐              ┌──────────────┐                            │
│   │  fbc-vec    │              │  fbc-config  │                            │
│   │  Compiler   │              │  Compiler    │                            │
│   └──────┬──────┘              └──────┬───────┘                            │
│          │                            │                                     │
│          ▼                            ▼                                     │
│   ┌─────────────┐              ┌──────────────┐                            │
│   │  .fbc file  │              │  .fbcfg file │                            │
│   │  (vectors)  │              │  (pin config)│                            │
│   └──────┬──────┘              └──────┬───────┘                            │
│          │                            │                                     │
│          └────────────┬───────────────┘                                     │
│                       ▼                                                     │
│               ┌──────────────┐                                              │
│               │  Test Plan   │  ← JSON file defining:                       │
│               │  (.json)     │    - Vector file reference                   │
│               │              │    - Device config reference                 │
│               │              │    - Power sequences                         │
│               │              │    - Loop counts, timing                     │
│               └──────────────┘                                              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
                            GUI / HOST SOFTWARE
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   ┌────────────────┐    ┌────────────────┐    ┌────────────────┐           │
│   │  React + Tauri │    │  Zustand Store │    │  FBC Protocol  │           │
│   │  Frontend      │◄──►│  State Mgmt    │◄──►│  Client        │           │
│   └────────────────┘    └────────────────┘    └────────────────┘           │
│                                                                             │
│   Storage Locations (Host PC):                                              │
│   ├── C:\FBC\test-plans\       ← Test plan JSON files                      │
│   ├── C:\FBC\vectors\          ← Compiled .fbc vector files                │
│   ├── C:\FBC\device-configs\   ← Compiled .fbcfg device configs            │
│   ├── C:\FBC\rack-configs\     ← Rack slot assignments                     │
│   └── C:\FBC\logs\             ← Test results, error logs                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                     Raw Ethernet (EtherType 0x88B5)
                                      │
                                      ▼
                            FPGA BOARDS (x500)
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   ┌──────────────────────────────────────────────────────────────────┐     │
│   │  Board EEPROM (256 bytes) - Persistent per-board storage         │     │
│   │                                                                   │     │
│   │  ├── Board Identity (32 bytes)                                   │     │
│   │  │   └── Serial, MAC, manufactured date, slot position           │     │
│   │  ├── Rail Configuration (48 bytes)                               │     │
│   │  │   └── 6 rails × voltage/current limits                        │     │
│   │  ├── Calibration Data (64 bytes)                                 │     │
│   │  │   └── ADC offsets, timing trim values                         │     │
│   │  ├── Last Test Reference (32 bytes)                              │     │
│   │  │   └── Test plan ID, DUT part number, last run timestamp       │     │
│   │  └── Reserved (80 bytes)                                         │     │
│   └──────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│   ┌──────────────────────────────────────────────────────────────────┐     │
│   │  Controller SD Card - Persistent vector + log storage             │     │
│   │                                                                   │     │
│   │  ├── vectors/        - Cached .fbc files (loaded from PC once)   │     │
│   │  └── logs/           - Test results (survives GUI disconnect)    │     │
│   │                                                                   │     │
│   │  Capacity: Gigabytes (compression makes large patterns fit)       │     │
│   │  Purpose: Autonomous operation when GUI/PC unavailable            │     │
│   └──────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│   ┌──────────────────────────────────────────────────────────────────┐     │
│   │  FPGA BRAM (Block RAM) - Active vector execution buffer           │     │
│   │                                                                   │     │
│   │  Capacity: ~140KB (enough for ~7,000 vectors at 20 bytes each)   │     │
│   │  Fed from: SD card or direct DMA from PC                          │     │
│   └──────────────────────────────────────────────────────────────────┘     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Data Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          COMPLETE DATA FLOW                                 │
└─────────────────────────────────────────────────────────────────────────────┘

    PATTERN SOURCE                    HOST PC                      FPGA BOARD
    ─────────────                    ─────────                     ──────────

    STIL/AVC/PAT           ┌──────────────────────┐
    (from ATE vendor)      │    fbc-vec compile   │
         │                 │    ────────────────  │
         └────────────────►│  Parse + Compress    │
                           │  145-95,952x ratio   │
                           └──────────┬───────────┘
                                      │
                                      ▼
                           ┌──────────────────────┐
                           │  vectors/test.fbc    │   Stored on PC,
                           │  (binary, ~10KB)     │   cached on controller
                           └──────────┬───────────┘   SD card
                                      │
    .bim/.map/.tim         ┌──────────────────────┐
    (Sonoma format)        │   fbc-config compile │
         │                 │   ─────────────────  │
         └────────────────►│  Parse + Convert     │
                           └──────────┬───────────┘
                                      │
                                      ▼
                           ┌──────────────────────┐
                           │  device-configs/     │   Stored on PC
                           │  ddr4_x16.fbcfg      │   Cached reference
                           └──────────┬───────────┘
                                      │
                                      │
                           ┌──────────────────────┐
                           │    Test Plan JSON    │
                           │    ───────────────   │
                           │  {                   │
                           │    "name": "DDR4.."  │
                           │    "vectors": "..."  │
                           │    "device": "..."   │
                           │    "loops": 10000    │
                           │    "temp_c": 85      │
                           │  }                   │
                           └──────────┬───────────┘
                                      │
                                      │
                    ┌─────────────────┴─────────────────┐
                    │         GUI OPERATIONS            │
                    │                                   │
                    │  1. discover_boards()             │
                    │     └──► DISCOVER broadcast       │──────────────────┐
                    │                                   │                  │
                    │  2. Load test plan                │                  ▼
                    │     └──► Read JSON + .fbc + .fbcfg│         ┌───────────────┐
                    │                                   │         │ Board replies │
                    │  3. Configure boards              │         │ with MAC +    │
                    │     └──► power_sequence_on()      │────────►│ slot position │
                    │     └──► set_vicor_voltage()      │         └───────────────┘
                    │                                   │
                    │  4. Upload vectors                │
                    │     └──► load_vectors()           │──────────────────┐
                    │         (chunked transfer)        │                  │
                    │                                   │                  ▼
                    │  5. Start test                    │         ┌───────────────┐
                    │     └──► start_vectors(loops)     │────────►│ Vectors in    │
                    │                                   │         │ BRAM, ready   │
                    │  6. Monitor                       │         └───────────────┘
                    │     └──► get_vector_status()      │
                    │         (polling every 100ms)     │◄─────── Status responses
                    │                                   │
                    └───────────────────────────────────┘
```

---

## Test Plan Architecture

### Where Data Lives

| Data Type | Storage Location | Size | Persistence |
|-----------|-----------------|------|-------------|
| **Vectors** | Host PC → Controller SD → FPGA BRAM | 10KB-10MB | Cached on controller |
| **Device Config** | Host PC → AXI registers | ~2KB | Loaded each test |
| **Test Plans** | Host PC filesystem | ~1KB JSON | Permanent |
| **Test Logs** | Controller SD card | Variable | Survives disconnect |
| **Board Identity** | Board EEPROM | 32 bytes | Factory programmed |
| **Calibration** | Board EEPROM | 64 bytes | Updated occasionally |
| **Rail Limits** | Board EEPROM | 48 bytes | Per-DUT configuration |
| **Last Test Ref** | Board EEPROM | 32 bytes | Auto-updated |

**Key insight:** Vectors can be cached on the controller's SD card. Once uploaded, the controller can run tests autonomously even if the GUI/PC disconnects. The compression optimizations (145-95,952x) ensure large patterns fit on the controller.

### Test Plan JSON Schema

```json
{
  "$schema": "fbc-test-plan-v1",
  "name": "DDR4 Burn-in Stress Test",
  "description": "High-temperature stress with March-C pattern",
  "version": "1.0.0",
  "created": "2026-02-01T10:00:00Z",

  "vectors": {
    "file": "vectors/ddr4_march_c.fbc",
    "checksum": "a1b2c3d4",
    "vector_count": 50000,
    "compression_ratio": 145.3
  },

  "device": {
    "file": "device-configs/ddr4_x16.fbcfg",
    "checksum": "e5f6g7h8",
    "part_number": "MT40A1G16",
    "pin_count": 96
  },

  "execution": {
    "loops": 10000,
    "loop_mode": "continuous",
    "stop_on_error": false,
    "error_threshold": 100
  },

  "power": {
    "sequence": "standard",
    "rails": [
      {"id": 0, "name": "VDD", "voltage_mv": 1200, "current_limit_ma": 2000},
      {"id": 1, "name": "VDDQ", "voltage_mv": 1200, "current_limit_ma": 500},
      {"id": 2, "name": "VPP", "voltage_mv": 2500, "current_limit_ma": 100}
    ],
    "ramp_rate_mv_per_ms": 10
  },

  "thermal": {
    "setpoint_c": 85,
    "tolerance_c": 2,
    "soak_time_s": 300
  },

  "timing": {
    "vector_clock_mhz": 100,
    "setup_ns": 2,
    "hold_ns": 2
  },

  "board_filter": {
    "mode": "all",
    "rack": null,
    "slots": null,
    "macs": null
  }
}
```

### Rack Configuration (Slot Assignments)

```json
{
  "rack_id": "RACK-01",
  "location": "Lab A, Row 3",
  "slots": {
    "1-50": {
      "dut_type": "DDR4_X16",
      "test_plan": "ddr4_stress_85c.json"
    },
    "51-88": {
      "dut_type": "DDR4_X8",
      "test_plan": "ddr4_stress_85c.json"
    }
  }
}
```

---

## GUI Design Philosophy

**The GUI is a translation layer.** It translates human intent into protocol commands.

- Backend handles complexity (protocol, timing, error handling)
- Frontend stays clean (familiar terms, minimal clicks)
- No artificial "modes" - one unified interface
- Uses same terminology as existing Sonoma/Everest system

### What the GUI Does

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         GUI = TRANSLATION LAYER                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   USER INTENT                          WHAT BACKEND DOES                    │
│   ───────────                          ─────────────────                    │
│   "Power on this board"          →     VICOR enable sequence + PMBus init  │
│   "Load these vectors"           →     Chunk + DMA + CRC verify            │
│   "Start the test"               →     Write control register, poll status │
│   "Show me the current"          →     Read XADC + MAX11131, scale, display│
│   "Emergency stop"               →     Broadcast STOP + power down all     │
│                                                                             │
│   User doesn't need to know:                                                │
│   - AXI register addresses                                                  │
│   - DMA chunk sizes                                                         │
│   - I2C bus arbitration                                                     │
│   - Frame timing                                                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Terminology (Match Existing System)

The GUI uses the same names operators already know:

| GUI Label | What It Is | Backend |
|-----------|------------|---------|
| **LCPS** | Low-Cost Power Supply | PMBus at addresses 0x10-0x1F |
| **VICOR** | Core voltage modules | DAC trim + GPIO enable |
| **BIM** | Burn-In Module | The board with DUT sockets |
| **Load Board** | Power/signal infrastructure | Controller + LCPS + VICOR |
| **DUT** | Device Under Test | The chip being tested |
| **Vectors** | Test patterns | .fbc file → BRAM |
| **XADC** | On-chip ADC | Zynq internal channels |
| **Case Temp** | DUT temperature | Thermistor via ADC |

### Unified Workflow

No modes. Just actions. The GUI figures out what you mean.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   SELECT what you want to work with:                                        │
│   ──────────────────────────────────                                        │
│   • Click a board → work with that board                                    │
│   • Click multiple boards → work with those boards                          │
│   • Click nothing → work with all boards                                    │
│                                                                             │
│   DO what you want to do:                                                   │
│   ──────────────────────                                                    │
│   • Load Vectors → pick file, sends to selected boards                      │
│   • Power On → runs power sequence on selected boards                       │
│   • Start → begins test on selected boards                                  │
│   • Stop → stops selected boards                                            │
│                                                                             │
│   SMART DEFAULTS:                                                           │
│   ───────────────                                                           │
│   • BIM detected? GUI suggests matching vectors                             │
│   • No selection? Actions apply to all boards                               │
│   • Power already on? Skip redundant sequence                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Device Profiles

The **device profile** is the single source of truth. Engineers create/edit profiles, the system uses them.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           DEVICE PROFILE STRUCTURE                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   profiles/DDR4_X16/                                                        │
│   ├── DDR4_X16.bim          ← Pin definitions (XML)                        │
│   ├── DDR4_X16.map          ← GPIO assignments                             │
│   ├── DDR4_X16.tim          ← Timing config                                │
│   ├── march_c.stil          ← Test patterns (source)                       │
│   └── compiled/                                                             │
│       ├── config.fbc        ← Cached binary (pin types, timing)            │
│       └── march_c.fbc       ← Cached binary (vectors)                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Flow:**
1. BIM EEPROM says "I'm DDR4_X16"
2. GUI loads `profiles/DDR4_X16/`
3. Cached binaries get pushed to board (config → registers, vectors → BRAM)

**Engineers edit** the source files (.bim, .map, .tim, .stil)
**System uses** the compiled binaries (.fbc) for fast loading

```bash
# Engineer compiles after editing
fbc-config compile DDR4_X16.bim DDR4_X16.map DDR4_X16.tim -o compiled/config.fbc
fbc-vec compile march_c.stil -o compiled/march_c.fbc
```

The GUI can also load different vectors for checkout/debug - just browse and pick a file.

### Common Tasks

**Bringup a new board:**
1. Click board
2. Power On
3. Check voltages/currents look right
4. Load simple test vectors
5. Run short test
6. Verify no errors

**Production run:**
1. Open GUI (boards auto-discover)
2. Load Vectors (GUI suggests based on BIM types)
3. Power On All
4. Start All
5. Monitor dashboard
6. Export results when done

**Debug a failing board:**
1. Click the failing board
2. Stop it
3. Check analog readings
4. Load different vectors
5. Run, observe
6. Repeat

**Pattern checkout:**
1. Pick a known-good board
2. Load new pattern file
3. Run 100 loops
4. Check for errors
5. If clean, use pattern on all boards

All the same interface. No switching. No complexity gates.

---

## Detailed Phase Breakdown

### Phase 1: Pattern Compilation (Offline)

```bash
# Convert customer test patterns to FBC format
fbc-vec compile customer_patterns.stil -o vectors/test.fbc --format fbc

# Output:
# Parsed 50,000 vectors from STIL file
# Compression: 50,000 × 20 bytes → 6,890 bytes (145x ratio)
# Output: vectors/test.fbc
# CRC32: 0xA1B2C3D4
```

### Phase 2: Device Configuration (Offline)

```bash
# Convert Sonoma device files to FBC format
fbc-config compile device.bim device.map device.tim -o device-configs/ddr4.fbcfg

# Output:
# Parsed 96 pins from BIM file
# Parsed timing from TIM file
# Output: device-configs/ddr4.fbcfg
```

### Phase 3: Test Plan Creation (Offline)

Create JSON file referencing vectors and device config:

```json
{
  "name": "DDR4 Stress",
  "vectors": {"file": "vectors/test.fbc"},
  "device": {"file": "device-configs/ddr4.fbcfg"},
  "loops": 10000
}
```

### Phase 4: Board Discovery (Runtime)

```typescript
// GUI discovers all boards on network
const boards = await invoke('discover_boards')
// Returns: [{mac: "00:1A:2B:...", slot: 1, status: "idle"}, ...]
```

### Phase 5: Vector Upload (Runtime)

```typescript
// Upload vectors to each board (parallel)
for (const board of selectedBoards) {
  await invoke('load_vectors', { mac: board.mac, data: vectorBytes })
}
// Vectors now in board BRAM, ready for execution
```

### Phase 6: Power Sequence (Runtime)

```typescript
// Execute power-on sequence per test plan
await invoke('power_sequence_on', { mac: board.mac })
// Waits for rails to stabilize, thermal soak
```

### Phase 7: Test Execution (Runtime)

```typescript
// Start vector execution
await invoke('start_vectors', { mac: board.mac, loops: 10000 })

// Poll status every 100ms
const status = await invoke('get_vector_status', { mac: board.mac })
// {state: "Running", loop_count: 523, error_count: 0, ...}
```

### Phase 8: Results Collection (Runtime)

```typescript
// When complete, collect results
const finalStatus = await invoke('get_vector_status', { mac: board.mac })
// {state: "Complete", loop_count: 10000, error_count: 0, run_time_ms: 3600000}

// Log to file
await writeResultsLog(board, finalStatus)
```

---

## Board Identification

### Device DNA (Unique ID)

Each Zynq 7020 has a factory-programmed 57-bit Device DNA:

```rust
// Read unique ID
let dna = DeviceDna::new();
let id = dna.read();  // 0x1234567890ABCDEF (unique per chip)

// Generate MAC address from DNA
let mac = dna.generate_mac();  // 02:xx:xx:xx:xx:xx (locally administered)
```

### EEPROM Board Identity

```
Offset  Size  Field
──────  ────  ─────
0x00    8     Magic ("FBC\x00\x01\x00\x00\x00")
0x08    8     Device DNA (from silicon)
0x10    6     MAC Address
0x16    2     Slot Position (rack-relative)
0x18    4     Manufactured Date (Unix timestamp)
0x1C    4     Firmware Version
```

### Slot Position Assignment

Boards are identified by slot position within the rack:
- **Rack ID**: Physical rack number (1-10)
- **Shelf**: Vertical position (1-11 shelves per rack)
- **Slot**: Horizontal position (1-8 slots per shelf)
- **Total**: 88 boards per rack, 500+ in facility

```
Slot Address = (Rack × 100) + (Shelf × 10) + Slot
Example: Rack 3, Shelf 5, Slot 7 = 357
```

---

## EEPROM Storage

There are TWO EEPROMs in the system - one on the controller board and one on the BIM.

### Controller Board EEPROM (256 bytes)

Located on the Zynq controller board. Stores board identity and calibration.

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Offset │ Size │ Field                      │ Description                  │
├────────┼──────┼────────────────────────────┼──────────────────────────────┤
│ 0x00   │  8   │ Header                     │ Magic + Version              │
│ 0x08   │  8   │ Device DNA                 │ Silicon unique ID            │
│ 0x10   │  6   │ MAC Address                │ Generated from DNA           │
│ 0x16   │  2   │ Slot Position              │ Rack-relative slot #         │
│ 0x18   │  4   │ Manufactured Date          │ Unix timestamp               │
│ 0x1C   │  4   │ Firmware Version           │ Major.Minor.Patch            │
├────────┼──────┼────────────────────────────┼──────────────────────────────┤
│ 0x20   │ 48   │ Rail Configuration (6×8B)  │ Per-rail voltage/current     │
│        │      │   Rail 0: VDD              │   voltage_mv (2B)            │
│        │      │   Rail 1: VDDQ             │   current_limit_ma (2B)      │
│        │      │   Rail 2: VPP              │   flags (2B)                 │
│        │      │   Rail 3-5: Aux            │   reserved (2B)              │
├────────┼──────┼────────────────────────────┼──────────────────────────────┤
│ 0x50   │ 64   │ Calibration Data           │ Factory calibration          │
│        │      │   ADC offsets (32B)        │   Per-channel offset         │
│        │      │   Timing trim (16B)        │   Clock phase adjustments    │
│        │      │   Temperature cal (16B)    │   Thermal sensor calibration │
├────────┼──────┼────────────────────────────┼──────────────────────────────┤
│ 0x90   │ 32   │ Last Test Reference        │ Most recent test info        │
│        │      │   Test Plan ID (16B)       │   Hash of test plan JSON     │
│        │      │   DUT Part Number (8B)     │   e.g., "MT40A1G16"          │
│        │      │   Last Run Timestamp (4B)  │   Unix timestamp             │
│        │      │   Last Result (4B)         │   Pass/fail + error count    │
├────────┼──────┼────────────────────────────┼──────────────────────────────┤
│ 0xB0   │ 80   │ Reserved                   │ Future expansion             │
└────────┴──────┴────────────────────────────┴──────────────────────────────┘
```

### BIM EEPROM (256 bytes)

Located on the Burn-In Module (BIM). Stores device type and BIM-specific info.
**This is what enables auto-detection of vectors.**

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Offset │ Size │ Field                      │ Description                  │
├────────┼──────┼────────────────────────────┼──────────────────────────────┤
│ 0x00   │  8   │ Header                     │ Magic "FBCBIM\x01\x00"       │
│ 0x08   │ 32   │ BIM Type                   │ e.g., "DDR4_X16_R3"          │
│ 0x28   │ 32   │ Part Number                │ BIM part number              │
│ 0x48   │ 16   │ Serial Number              │ Unique BIM serial            │
│ 0x58   │  4   │ Manufactured Date          │ Unix timestamp               │
│ 0x5C   │  4   │ Revision                   │ Hardware revision            │
├────────┼──────┼────────────────────────────┼──────────────────────────────┤
│ 0x60   │  2   │ DUT Count                  │ Number of DUT sockets        │
│ 0x62   │  2   │ Pin Count                  │ Pins per DUT                 │
│ 0x64   │ 32   │ Device Name                │ Target DUT (e.g., "MT40A1G") │
│ 0x84   │ 32   │ Default Test Plan          │ Suggested test plan name     │
├────────┼──────┼────────────────────────────┼──────────────────────────────┤
│ 0xA4   │ 92   │ Reserved                   │ Future expansion             │
└────────┴──────┴────────────────────────────┴──────────────────────────────┘
```

### How BIM Auto-Detection Works

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           BIM DETECTION FLOW                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   Controller Board                         BIM (plugged in)                 │
│   ────────────────                         ────────────────                 │
│   ┌──────────────┐                         ┌──────────────┐                │
│   │ Board EEPROM │  (I2C addr 0x50)        │ BIM EEPROM   │ (I2C addr 0x51)│
│   │ - MAC addr   │                         │ - BIM Type   │                │
│   │ - Slot pos   │                         │ - DUT info   │                │
│   │ - Calibration│                         │ - Pin count  │                │
│   └──────────────┘                         └──────────────┘                │
│          │                                        │                         │
│          └────────────────┬───────────────────────┘                         │
│                           │                                                 │
│                           ▼                                                 │
│                    Firmware reads both                                      │
│                           │                                                 │
│                           ▼                                                 │
│              ┌─────────────────────────────┐                               │
│              │ DISCOVER Response to GUI    │                               │
│              │ {                           │                               │
│              │   "mac": "00:1A:2B:...",    │                               │
│              │   "slot": 42,               │                               │
│              │   "bim_type": "DDR4_X16_R3",│  ← From BIM EEPROM           │
│              │   "bim_serial": "BIM-001",  │                               │
│              │   "dut_count": 12,          │                               │
│              │   "pin_count": 96           │                               │
│              │ }                           │                               │
│              └─────────────────────────────┘                               │
│                           │                                                 │
│                           ▼                                                 │
│              ┌─────────────────────────────┐                               │
│              │ GUI looks up in Device      │                               │
│              │ Library:                    │                               │
│              │                             │                               │
│              │ DDR4_X16_R3 →               │                               │
│              │   test-plans/ddr4_85c.json  │                               │
│              │   vectors/ddr4_march_c.fbc  │                               │
│              │   device-configs/ddr4.fbcfg │                               │
│              └─────────────────────────────┘                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Points

| EEPROM | Location | Stores | Used For |
|--------|----------|--------|----------|
| **Board EEPROM** | Controller | MAC, slot, calibration | Board identity |
| **BIM EEPROM** | Burn-In Module | BIM type, DUT info | Auto-detect vectors |

- **Vectors** are NOT stored in either EEPROM (too large)
- **BIM Type** in BIM EEPROM enables auto-association with test plans
- **Device Library** (on host PC) maps BIM types to test plans
- **Engineer verifies** the association once, then operators just click Start

---

## File Formats Reference

### .fbc (Vector Binary)

```
┌─────────────────────────────────────────────────────────────────┐
│ FBC Vector File Format                                          │
├─────────────────────────────────────────────────────────────────┤
│ Header (32 bytes)                                               │
│   Magic:      "FBCV" (4 bytes)                                  │
│   Version:    0x0001 (2 bytes)                                  │
│   Pin Count:  160 (2 bytes)                                     │
│   Vec Count:  N (4 bytes)                                       │
│   Flags:      (4 bytes)                                         │
│   CRC32:      (4 bytes)                                         │
│   Reserved:   (12 bytes)                                        │
├─────────────────────────────────────────────────────────────────┤
│ Vector Data (20 bytes per vector)                               │
│   Pin States: 160 pins × 2 bits = 320 bits = 40 bytes           │
│   But packed: drive + expect in 20 bytes                        │
│   Repeat:     4 bytes (run-length count)                        │
├─────────────────────────────────────────────────────────────────┤
│ Total: 32 + (N × 24) bytes                                      │
└─────────────────────────────────────────────────────────────────┘
```

### .fbcfg (Device Config Binary)

```
┌─────────────────────────────────────────────────────────────────┐
│ FBC Device Config Format                                        │
├─────────────────────────────────────────────────────────────────┤
│ Header (64 bytes)                                               │
│   Magic, version, pin count, timing params                      │
├─────────────────────────────────────────────────────────────────┤
│ Pin Table (pin_count × 8 bytes)                                 │
│   Per pin: type, driver, timing, flags                          │
├─────────────────────────────────────────────────────────────────┤
│ Timing Table (variable)                                         │
│   Clock frequencies, edge positions                             │
└─────────────────────────────────────────────────────────────────┘
```

### Test Plan JSON

See [Test Plan JSON Schema](#test-plan-json-schema) above.

---

## Workflow Summary

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TYPICAL PRODUCTION WORKFLOW                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   SETUP (Once per DUT type)                                                 │
│   ─────────────────────────                                                 │
│   1. Compile vectors:     fbc-vec compile patterns.stil -o test.fbc        │
│   2. Compile device:      fbc-config compile dev.bim -o dev.fbcfg          │
│   3. Create test plan:    Edit test_plan.json                              │
│                                                                             │
│   DAILY OPERATION                                                           │
│   ───────────────                                                           │
│   1. Launch GUI                                                             │
│   2. Connect to network interface                                           │
│   3. Wait for board discovery                                               │
│   4. Load test plan (auto-loads vectors + config)                          │
│   5. Select boards (all, by slot, by filter)                               │
│   6. Click "Power On" (executes sequence)                                  │
│   7. Click "Start" (begins test on all boards)                             │
│   8. Monitor dashboard (errors, temperature, progress)                      │
│   9. When complete, export results                                          │
│   10. Click "Power Off"                                                     │
│                                                                             │
│   VECTOR STORAGE OPTIONS                                                    │
│   ──────────────────────                                                    │
│   - Controller SD card: Cached for autonomous operation                     │
│   - Direct upload: PC → Controller → FPGA BRAM                              │
│   - Once loaded, controller runs independently (GUI can disconnect)         │
│   - CRC verified after upload                                               │
│   - Logs stored on SD card, survive GUI/PC disconnect                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Autonomous Operation

The controller (Zynq ARM + FPGA) can operate independently once a test is started.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         AUTONOMOUS OPERATION MODE                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   GUI CONNECTED (Normal)          GUI DISCONNECTED (Autonomous)             │
│   ──────────────────────          ─────────────────────────────             │
│                                                                             │
│   PC ◄───────► Controller         PC ╳          Controller                  │
│   │              │                               │                          │
│   │ Commands     │                               │ Continues:               │
│   │ Telemetry    │                               │ - Executing vectors      │
│   │ Real-time    │                               │ - Logging to SD card     │
│   │ monitoring   │                               │ - Tracking errors        │
│   │              │                               │ - Monitoring temperature │
│   │              ▼                               │                          │
│   │           FPGA                               ▼                          │
│   │           executing                       FPGA                          │
│   │           vectors                         executing                     │
│   │                                           vectors                       │
│   ▼                                              │                          │
│   Dashboard                                      │ On reconnect:            │
│   updates                                        │ - GUI retrieves logs     │
│                                                  │ - Sync status            │
│                                                  │ - Resume monitoring      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Why this matters for burn-in testing:**
- 168-hour tests should not fail because of PC issues
- Windows Update rebooting the PC should not abort a week-long test
- Network glitches should not lose test data
- Controller is the reliable component; GUI is the convenient interface

**What the controller stores locally (SD card):**
- Cached vector files (loaded once, reusable)
- Test execution logs (cycle count, error count, timestamps)
- Error details (which vectors failed, when)
- Temperature history

**What the GUI provides:**
- Initial configuration and test start
- Real-time monitoring when connected
- Data export and analysis
- Multi-board orchestration

---

## FAQ

**Q: Do I have to manually load vectors every time?**
A: No. BIM EEPROM identifies the device (bim_type). Device profile on PC specifies the vectors. GUI loads them automatically. You can also browse and load different vectors anytime.

**Q: Do vectors get stored on the board?**
A: Yes - on the controller's SD card. Flow: PC compiles vectors → uploads to controller SD card → controller loads to FPGA BRAM for execution. Not stored in EEPROM (256 bytes, too small). The SD card provides persistent storage that survives power cycles and GUI disconnects.

**Q: What's in the BIM EEPROM?**
A: (From `firmware/src/hal/eeprom.rs` - `BimEeprom` struct, 256 bytes total):

| Section | Size | Contents |
|---------|------|----------|
| **Header** | 16B | magic (0xBEEFCAFE), version, bim_type, hw_revision, serial_number, manufacture_date |
| **LCPS Rails** | 64B | 8 rails × (voltage_mv, max_voltage_mv, min_voltage_mv, max_current_ma) |
| **Calibration** | 64B | 16 voltage offsets + 16 current offsets (signed, for ADC calibration) |
| **DUT Metadata** | 96B | dut_vendor (32B), dut_part_number (32B), dut_description (32B) |
| **Statistics** | 8B | program_count + reserved |
| **Checksum** | 4B | CRC32 of bytes 0-247 |
| **Reserved** | 4B | - |

**Q: What's a device profile?**
A: A folder on the PC containing everything for a device type:
- Source files (.bim, .map, .tim, .stil) - what engineers edit
- Compiled binaries (.fbc) - what gets loaded to boards
- BIM EEPROM `bim_type` → GUI loads that profile → binaries go to board

**Q: What about the LCPS rail config in EEPROM?**
A: The EEPROM stores the power limits for each rail (nominal voltage, min/max voltage, max current). These are safety limits - the firmware checks against them. The device profile on PC can specify different operating voltages within these limits.

**Q: Can boards run without the GUI?**
A: Yes. Once a test is started, the controller runs autonomously. If the GUI disconnects or the PC crashes:
- Controller continues executing vectors
- Controller keeps logging results locally (SD card)
- Controller sends heartbeats periodically
- When ethernet reconnects, GUI retrieves logged data

This is critical for burn-in testing - a 168-hour test shouldn't fail because someone unplugged a monitor or Windows Update rebooted the PC. The GUI has **control** but is not **required** for continued operation.

**Q: Where can vectors be stored?**
A: Two places:
1. **Board BRAM** - ~140KB volatile, vectors loaded via DMA from controller
2. **Controller SD card** - Megabytes, persistent, vectors loaded at test start

The compression optimizations (145-95,952x) proved that large test patterns can fit on the controller. No constant PC connection needed during execution.

**Q: What happens if power fails mid-test?**
A: Board resets to idle. EEPROM data persists. Vectors need to be re-uploaded (automatic when you reconnect).

**Q: Why use Sonoma/Everest terminology?**
A: Because operators already know it. LCPS is LCPS. VICOR is VICOR. BIM is BIM. Don't rename things that already have names.

**Q: Is there a separate "engineering mode"?**
A: No. Same interface for everyone. Engineers edit device profiles, compile them, test them. Operators run tests. Same GUI, same actions.

---

## Sonoma/Everest to FBC Mapping

This section maps the legacy Sonoma/Everest file structure to the new FBC system.

### File Structure Comparison

```
EVEREST (Legacy)                          FBC (New)
────────────────                          ────────
Everest/                                  C:\FBC\
├── Data/                                 ├── test-plans/
│   ├── Bims/                             │   └── *.json           ← Replaces .tpf
│   │   └── <device>.bim                  │
│   ├── Testplans/                        ├── vectors/
│   │   └── <test>.tpf                    │   └── *.fbc            ← Replaces .hex/.seq
│   └── Logs/                             │
│       └── <date>_<device>/              ├── device-configs/
│                                         │   └── *.fbcfg          ← Replaces .bim/.map/.tim
├── devices/                              │
│   └── <device>/                         ├── rack-configs/
│       ├── <device>.map                  │   └── *.json           ← NEW: slot assignments
│       ├── PowerOn                       │
│       ├── PowerOff                      └── logs/
│       ├── vectors/                          └── <date>/          ← Same concept
│       │   ├── <pattern>.hex
│       │   └── <pattern>.seq
│       └── <test>.tim
│
└── Firmware/
    └── FW_v2.11/
        ├── top.bit
        ├── BOOT.bin
        └── bin/
```

### File Format Mapping

| Sonoma File | FBC Equivalent | Notes |
|-------------|----------------|-------|
| `<device>.bim` | `*.fbcfg` | Pin definitions, power rails, layout |
| `<device>.map` | `*.fbcfg` | GPIO assignments, ADC channels |
| `<test>.tim` | `*.fbcfg` | Timing parameters |
| `<pattern>.hex` | `*.fbc` | Vector bit patterns (compressed) |
| `<pattern>.seq` | `*.fbc` | Repeat counts (embedded in .fbc) |
| `<test>.tpf` | `*.json` | Test plan orchestration |
| `PowerOn` | Test plan JSON | Power sequence in `power.sequence` field |
| `PowerOff` | Test plan JSON | Shutdown in test plan |
| `top.bit` | `top.bit` | FPGA bitstream (unchanged) |
| `bin/*.elf` | Rust firmware | Bare-metal Rust replaces Linux + ELF tools |

### Layer Mapping

```
EVEREST LAYERS                            FBC LAYERS
──────────────                            ──────────

Layer 0: Purpose                          Same
(Accelerated life testing)                (Unchanged)

Layer 1: Test Definition                  Test Plan JSON
(Device type, stress conditions)          (device, execution, power, thermal)

Layer 2: System Architecture              Same (11 shelves, 8 boards each)
(Everest → Shelf → Tray → Controller)     (GUI → Rack → Slot → Board)

Layer 3: Everest Server                   FBC GUI (Tauri)
(Unity.exe, TCP/IP, data logging)         (React, raw Ethernet, Zustand)

Layer 4: Controller                       FBC Firmware (bare-metal Rust)
(Zynq + Linux + SD card)                  (Zynq + no OS + flash)

Layer 5: FPGA Fabric                      Same (optimized RTL)
(DMA, pattern engine, per-pin logic)      (Simpler design, same function)

Layer 6: Load Board                       Same (hardware unchanged)
(LCPS, VICOR, signal conditioning)        (Same power infrastructure)

Layer 7: BIM                              Same (hardware unchanged)
(DUT sockets, thermal, power dist)        (Same burn-in modules)

Layer 8: Map File                         Embedded in .fbcfg
(GPIO ↔ Function)                         (Pin assignments in device config)

Layer 9: Vectors                          .fbc format
(.hex + .seq files)                       (Compressed binary, single file)

Layer 10: Testplan                        Test Plan JSON
(.tpf orchestration)                      (Modern JSON format)
```

### Key Differences

| Aspect | Sonoma/Everest | FBC |
|--------|----------------|-----|
| **Protocol** | TCP/IP over Ethernet | Raw Ethernet (Layer 2) |
| **Controller OS** | Linux + shell scripts | Bare-metal Rust |
| **Boot time** | ~30 seconds | <1 second |
| **File format** | XML, custom text | JSON, binary |
| **Vector storage** | .hex + .seq (separate) | .fbc (single compressed) |
| **Config storage** | .bim + .map + .tim | .fbcfg (single binary) |
| **GUI** | Unity.exe (C#) | Tauri (Rust + React) |
| **Test plan** | .tpf (custom format) | JSON (standard) |

### Migration Path

**Converting Sonoma files to FBC:**

```bash
# Convert vectors
fbc-vec compile old_patterns.hex old_patterns.seq -o new_patterns.fbc

# Convert device config
fbc-config compile device.bim device.map device.tim -o device.fbcfg

# Test plan: manual conversion to JSON
# (or write a converter script)
```

### Test Plan Comparison

**Sonoma .tpf:**
```
TEST_PROGRAM_NAME  DDR4_Stress
TEST_DURATION      24
DEVICE_DIR         C:\Everest\devices\DDR4\
PIN_MAP_FILE       DDR4.map
SETUP_FILE         DDR4.lvl

TEST_STEP "PowerOn"
    PU_LIST        PS1_VDD=1200,PS2_VDDQ=1200
    SETPOINT_TEMP  85
END_TEST_STEP

TEST_STEP "BurnIn_Loop"
    VECTOR_FILE    march_c
    REPEAT_COUNT   10000
    MASTER_PERIOD  10
END_TEST_STEP
```

**FBC test-plan.json:**
```json
{
  "name": "DDR4_Stress",
  "execution": {
    "loops": 10000,
    "stop_on_error": false
  },
  "vectors": {
    "file": "vectors/march_c.fbc"
  },
  "device": {
    "file": "device-configs/ddr4.fbcfg"
  },
  "power": {
    "sequence": "standard",
    "rails": [
      {"id": 0, "name": "VDD", "voltage_mv": 1200},
      {"id": 1, "name": "VDDQ", "voltage_mv": 1200}
    ]
  },
  "thermal": {
    "setpoint_c": 85
  },
  "timing": {
    "vector_clock_mhz": 100
  }
}
```

### What Stays the Same

- Hardware: Load boards, BIMs, VICOR modules, LCPS, controllers
- Physical layout: 11 shelves × 8 boards = 88 boards per rack
- Pin count: 128 vector pins + 32 fast pins = 160 total
- DUT sockets: Same burn-in sockets
- Power infrastructure: Same PMBus + VICOR architecture

### What Changes

- **Faster boot**: No Linux, bare-metal Rust boots in <1 second
- **Simpler protocol**: Raw Ethernet instead of TCP/IP stack
- **Better compression**: 145-95,952x vector compression
- **Modern GUI**: React + Three.js instead of Unity
- **Single files**: One .fbc instead of .hex+.seq, one .fbcfg instead of .bim+.map+.tim
- **JSON config**: Human-readable test plans

---

*Last updated: 2026-02-03*
