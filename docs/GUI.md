# FBC GUI Documentation

This document covers both GUI applications in the FBC Semiconductor System.

---

## Table of Contents

1. [FPGA Toolchain GUI](#fpga-toolchain-gui) - Development tool
2. [FBC System GUI](#fbc-system-gui) - Production control interface

---

## FPGA Toolchain GUI

**Purpose:** Development tool for building FPGA bitstreams from Verilog source code.

**Target Users:** FPGA developers, firmware engineers

**Location:** `fpga-toolchain/src/gui.rs`

**Status:** ✅ Implemented

### Overview

Simple, no-nonsense GUI that replaces command-line complexity with three steps:
1. Pick your Verilog files
2. Select your target board
3. Click Build

### Features

#### 1. File Selection
- **Add Verilog Files** button opens file picker
- Supports multiple file formats:
  - `.v` - Verilog source
  - `.vh` - Verilog header (defines)
  - `.sv` - SystemVerilog
  - `.svh` - SystemVerilog header
- File list with remove (X) buttons
- Scrollable list for many files

#### 2. Board Selection
- Dropdown menu with available device profiles
- Built-in **Zynq 7020 Sonoma** profile
- Load custom profiles from `profiles/` directory (JSON)
- Shows device info:
  - Vendor and family
  - LUT/FF capacity
  - Pin count, BRAM, DSP resources

#### 3. Output Configuration
- Text field for output bitstream path
- Browse button for file picker
- Defaults to `output.bit`

#### 4. Build Process
**Progress Tracking:**
- Current build step with progress bar
- Realtime log output
- Step breakdown:
  1. **Parsing** (0-15%) - Read Verilog files, parse modules
  2. **Elaborating** (15-30%) - Build netlist hierarchy
  3. **Synthesizing** (30-40%) - Optimize logic
  4. **Mapping** (40-50%) - Convert to LUTs/FFs
  5. **Placing** (50-70%) - Assign physical locations
  6. **Routing** (70-95%) - Connect wires
  7. **Bitstream** (95-100%) - Generate output file

**Error Handling:**
- Parse errors show file and line number
- Synthesis errors explain issue
- Routing failures suggest solutions

#### 5. Results Display
- Resource utilization:
  - LUTs used / available
  - FFs used / available
  - Cells, nets
- Build log scrollable area
- Success/error status

### Running the GUI

```bash
cd fpga-toolchain
cargo run --release --bin gui
```

**Or build executable:**
```bash
cargo build --release --bin gui
# Output: target/release/gui.exe (Windows) or target/release/gui (Linux)
```

### Device Profiles

**Built-in Profile:**
- Zynq 7020 Sonoma (400-pin package)
- 53,200 LUTs, 106,400 FFs
- 140 BRAMs, 220 DSPs
- Pin constraints from `constraints/zynq7020.xdc`

**Custom Profiles:**
Create JSON file in `profiles/` directory:

```json
{
  "name": "Custom Board",
  "vendor": "Xilinx",
  "family": "Zynq-7000",
  "device": "xc7z020clg400-1",
  "luts": 53200,
  "ffs": 106400,
  "brams": 140,
  "dsps": 220,
  "pins": 400,
  "constraints": "path/to/constraints.xdc"
}
```

### Architecture

**Technology Stack:**
- **GUI Framework:** egui (immediate mode, cross-platform)
- **Native Window:** eframe (Rust, no web dependencies)
- **File Dialogs:** rfd (native OS dialogs)
- **Threading:** std::thread (build runs in background)
- **State Management:** Arc<Mutex<BuildState>> (thread-safe)

**Build Pipeline:**
```
Verilog Files → Parser → Elaborator → Synthesizer → Tech Mapper
                                                          ↓
Bitstream ← Generator ← Router ← Placer ← ONETWO Optimizer
```

**Benefits:**
- ✅ No command-line arguments to remember
- ✅ Visual progress feedback
- ✅ Error messages with context
- ✅ Cross-platform (Windows, Linux, macOS)
- ✅ Dark theme (programmer-friendly)
- ✅ Native performance (no web overhead)

### Screenshots

**Main Window Layout:**
```
┌─────────────────────────────────────────────────────────────┐
│ FBC FPGA Toolchain                                          │
│ Verilog to Bitstream - Simple.                              │
├───────────────────┬─────────────────────────────────────────┤
│ 1. Select Files   │                                         │
│ [Add Files...]    │  Build Progress: 45%                    │
│                   │  Step: Placing (assigning locations)    │
│ Files:            │  ████████████░░░░░░░░░░░░░              │
│ • fbc_top.v    X  │                                         │
│ • fbc_decoder.v X │  Log:                                   │
│ • vector_eng.v  X │  [1/6] Parsing Verilog...               │
│                   │  Found 6 modules                        │
│ 2. Select Board   │  [2/6] Elaborating...                   │
│ [Zynq 7020    ▼]  │  [3/6] Synthesizing...                  │
│                   │  [4/6] Technology mapping...            │
│ Xilinx - Zynq-7000│  Netlist Statistics:                    │
│ 53200 LUTs        │    Cells: 883                           │
│ 106400 FFs        │    Nets:  983                           │
│                   │    LUTs:  763                           │
│ 3. Output         │    FFs:   59                            │
│ output.bit [...]  │  [5/6] Placing...                       │
│                   │  Annealing: 224483 accepted             │
│ [Build Bitstream] │  Final cost: 38853.00                   │
│                   │  [6/6] Routing...                       │
└───────────────────┴─────────────────────────────────────────┘
```

### Future Enhancements

**Planned Features:**
- ⏳ Constraint editor (visual pin assignment)
- ⏳ Waveform viewer (integrated VCD display)
- ⏳ Resource utilization graph
- ⏳ Timing analysis view
- ⏳ ONETWO pattern browser

---

## FBC System GUI

**Purpose:** Production control interface for burn-in testing system.

**Target Users:** Test operators, production engineers

**Location:** `gui/` (Tauri + React application)

**Status:** ✅ Implemented (90% complete)

### Overview

Full-featured graphical interface for controlling 500+ Zynq boards running burn-in tests. Replaces command-line tools with visual monitoring and control.

**Design Philosophy:**
- **GUI = Translation Layer** - Translates user intent to protocol commands
- **One unified interface** - No modes, no gatekeeping, same tools for everyone
- **Familiar terminology** - Uses Sonoma/Everest names (LCPS, VICOR, BIM, DUT)
- **Smart defaults** - Suggests vectors based on BIM type, but never forces
- **Backend handles complexity** - User doesn't see AXI registers or DMA chunks

**Core Principles:**
- Click a board → work with that board
- Click multiple → work with those
- Click nothing → work with all
- Power On, Load Vectors, Start, Stop - that's the vocabulary

### Planned Features

#### 1. Board Discovery & Status

**Discovery:**
- Automatic board detection via ANNOUNCE broadcasts
- MAC address → Board ID mapping
- Status indicators (online/offline/error)
- Board count display (e.g., "487 / 500 boards online")

**Status Display:**
```
┌─────────────────────────────────────────────────────┐
│ Board Grid View                                     │
│ ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐          │
│ │ ✓ │ ✓ │ ✓ │ ✓ │ ✓ │ ✓ │ ✓ │ ✓ │ ✓ │ ✓ │ Row 1   │
│ │ ✓ │ ✓ │ ⚠ │ ✓ │ ✓ │ ✓ │ ✓ │ ✓ │ ✓ │ ✓ │ Row 2   │
│ │ ✓ │ ✓ │ ✓ │ ✓ │ ✗ │ ✓ │ ✓ │ ✓ │ ✓ │ ✓ │ Row 3   │
│ └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘          │
│                                                      │
│ Legend: ✓ Running  ⚠ Warning  ✗ Error  ○ Offline   │
└─────────────────────────────────────────────────────┘
```

#### 2. Test Plan Management

**Test Plan Editor:**
- Load vector files (STIL, AVC, ATP formats)
- Configure test parameters:
  - Vector clock frequency (1-200 MHz)
  - Temperature setpoint (25-85°C)
  - Duration (hours/cycles)
  - Error thresholds
- Pin configuration (type, pulse timing)
- BIM power rail settings

**Vector Upload:**
- Chunked upload to boards (FBC UPLOAD_VECTORS command)
- Progress bar per board
- Parallel uploads (max network bandwidth)
- Verification (CRC32 checksum)

#### 2.1 Vector Format Conversion (NEW - Jan 2026)

**Universal Pattern Converter:**
The GUI integrates `fbc-vec` tool for converting any input format to any output format:

**Input Formats:**
- STIL (IEEE 1450) - Standard Test Interface Language
- AVC (Advantest) - 93K format
- PAT (XPS/ISE Pattern Editor) - Xilinx pattern format
- APS (Automatic Test Pattern) - Intermediate ASCII format
- FVEC (FBC text format) - Simple text vectors

**Output Formats (Multi-System Support):**
- **FBC Binary** - For FBC FPGA system (Zynq 7020)
- **Sonoma .hex** - For legacy Sonoma system (40-byte format)
- **PAT** - For XPS/ISE Pattern Editor
- **HX/Shasta** - Planned for future HX and Shasta systems

**GUI Integration:**
- File picker for input format selection
- Format auto-detection (by file extension)
- Output format dropdown (FBC/Sonoma/PAT/HX/Shasta)
- Real-time compression ratio display
- Progress bar during conversion
- CRC32 validation after conversion

**Compression Performance:**
- Typical: 145-710x compression (AVC files)
- Best case: 95,952x compression (repeating patterns)
- Pattern detection: Automatic run-length encoding
- Loop expansion: A...B, C...D loops fully supported
- Hold commands: H[num] suffix parsing (PAT format)

**Location:** `tools/fbc-vec/` (standalone Rust crate)
**Status:** ✅ Complete and tested with real customer files

#### 3. BIM Configuration

**EEPROM Management:**
- Read BIM EEPROM (24LC02, 256 bytes)
- Edit configuration:
  - Power rail voltages/currents
  - Calibration data
  - DUT metadata
- Write and verify EEPROM
- Checksum validation

**BIM Status View:**
```
┌─────────────────────────────────────────────────────┐
│ Board 172.16.0.101 - BIM #1                         │
├─────────────────────────────────────────────────────┤
│ Power Rails:                                        │
│   VDD_CORE:   1.200V  /  850mA  [OK]                │
│   VDD_IO:     3.300V  /  120mA  [OK]                │
│   VDD_AUX:    1.800V  /   50mA  [OK]                │
│                                                      │
│ Temperature:   45.2°C  (Target: 45.0°C) [LOCKED]    │
│ Fan Speed:     3200 RPM (65% duty)                  │
│                                                      │
│ DUT:           Detected (ID: 0x4A2B1C3D)            │
│ Pins:          128 BIM + 32 Fast                    │
└─────────────────────────────────────────────────────┘
```

#### 4. Execution Control

**Commands:**
- **START** - Begin test execution on selected boards
- **STOP** - Halt execution (preserves state)
- **RESET** - Reset to idle (clears state)
- **PAUSE** - Pause execution (resume from same point)

**Selection:**
- Individual boards
- Board groups (rack, tray, column)
- All boards
- Filter by status (running, idle, error)

**Broadcast Control:**
- Single command to all boards (<1ms via raw Ethernet)
- No sequential SSH overhead
- Deterministic timing

#### 5. Real-Time Monitoring

**Heartbeat Display:**
- Periodic HEARTBEAT packets (every 100ms)
- Per-board telemetry:
  - Cycle count
  - Error count
  - Temperature
  - Vector execution rate
- Color-coded status

**Live Metrics:**
```
┌─────────────────────────────────────────────────────┐
│ System Overview                                     │
├─────────────────────────────────────────────────────┤
│ Total Cycles:        15,234,567,890                 │
│ Total Errors:        0                              │
│ Average Temp:        45.1°C  (σ = 0.8°C)            │
│ Vector Rate:         82.3 MHz (avg across boards)   │
│ Uptime:              48h 15m 32s                    │
│                                                      │
│ Network:             1.2 Gbps (24% utilization)     │
│ Packet Loss:         0.00%                          │
└─────────────────────────────────────────────────────┘
```

#### 6. Error Reporting

**Error Details:**
- Pin number (0-159)
- Vector number (cycle where error occurred)
- Expected vs. actual value
- Timestamp
- Board ID

**Error Table:**
```
┌──────────────────────────────────────────────────────────┐
│ Board      │ Pin │ Vector  │ Expected │ Actual │ Time   │
├────────────┼─────┼─────────┼──────────┼────────┼────────┤
│ 0.103 BIM1 │  42 │ 1234567 │    H     │   L    │ 14:32  │
│ 0.105 BIM2 │  17 │ 2345678 │    L     │   H    │ 14:35  │
│ 0.107 BIM3 │  89 │ 3456789 │    H     │   Z    │ 14:40  │
└────────────┴─────┴─────────┴──────────┴────────┴────────┘
```

**Filtering:**
- By board
- By pin
- By time range
- By error type

#### 7. Data Logging

**Log Files:**
- **Test Results** - Pass/fail per DUT
- **Error Log** - All errors with full context
- **Telemetry Log** - Temperature, voltage, current over time
- **Event Log** - Start/stop, configuration changes

**Export Formats:**
- CSV (Excel-compatible)
- JSON (programmatic analysis)
- STDF (Semiconductor Test Data Format)

#### 8. Firmware Management

**Firmware Upload:**
- Select firmware binary (`firmware.elf`)
- Broadcast to selected boards
- Flash programming via PCAP (FPGA config port)
- Automatic reboot
- Version verification

**Bitstream Upload:**
- Select FPGA bitstream (`.bit` file)
- Broadcast or per-board upload
- FPGA configuration via PCAP
- Verification (readback CRC)

**Version Display:**
```
┌─────────────────────────────────────────────────────┐
│ Firmware Versions                                   │
├─────────────────────────────────────────────────────┤
│ Firmware:     v2.1.0   (487 boards)                 │
│ FPGA:         v1.5.3   (487 boards)                 │
│                                                      │
│ Outdated:                                           │
│   Board 0.103: Firmware v2.0.9 (outdated)           │
│   Board 0.145: FPGA v1.4.2 (outdated)               │
└─────────────────────────────────────────────────────┘
```

### Architecture

**Technology Stack (Implemented):**
- **Framework:** Tauri v2 (Rust backend + React frontend)
- **Frontend:** React 18 + TypeScript + Vite
- **3D Graphics:** Three.js + React Three Fiber (rack visualization)
- **State Management:** Zustand (lightweight, TypeScript-native)
- **Styling:** CSS with CSS variables for theming
- **Networking:** Raw Ethernet via Tauri backend (EtherType 0x88B5)
- **Protocol:** FBC Protocol (custom Layer 2)

**Communication Flow:**
```
┌─────────────────────┐
│   FBC System GUI    │  (PC Host)
└──────────┬──────────┘
           │
           │ Raw Ethernet (EtherType 0x88B5)
           │ Broadcast / Unicast
           ▼
    ┌────────────┐
    │   Switch   │
    └──────┬─────┘
           │
     ┌─────┴──────────┬──────────┬──────────┐
     ▼                ▼          ▼          ▼
┌──────────┐    ┌──────────┐  ┌──────────┐  ...
│ Board 101│    │ Board 102│  │ Board 103│  (500 boards)
│ Firmware │    │ Firmware │  │ Firmware │
└──────────┘    └──────────┘  └──────────┘
```

### FBC Protocol Commands (GUI → Firmware)

**Setup Phase:**
- `BIM_STATUS_REQ` - Query BIM configuration
- `WRITE_BIM` - Write BIM EEPROM
- `UPLOAD_VECTORS` - Upload test vectors (chunked)
- `CONFIGURE` - Set test parameters

**Runtime:**
- `START` - Begin test execution
- `STOP` - Halt execution
- `RESET` - Reset to idle
- `STATUS_REQ` - Request status update

**Firmware → GUI:**
- `ANNOUNCE` - Board online notification (MAC address)
- `BIM_STATUS_RSP` - BIM configuration response
- `HEARTBEAT` - Periodic status (cycle count, errors, temp)
- `ERROR` - Error notification (pin, vector, value)
- `STATUS_RSP` - Full status response

### User Workflow

**Typical Test Session:**

1. **Startup**
   - Launch GUI
   - Wait for board discovery (ANNOUNCE packets)
   - Verify all boards online

2. **Configuration**
   - Load test plan (vectors + parameters)
   - Upload vectors to boards
   - Configure BIM settings (power, pins)
   - Set temperature setpoint

3. **Execution**
   - Click START (broadcast to all boards)
   - Monitor real-time telemetry
   - Watch for errors

4. **Analysis**
   - Review error log
   - Export test results
   - Identify failing DUTs

5. **Iteration**
   - Adjust test parameters
   - Re-run on failing boards
   - Update firmware if needed

### Security & Access Control

**Planned Features:**
- ⏳ User authentication (local accounts)
- ⏳ Role-based access (operator vs. engineer)
- ⏳ Audit log (who did what, when)
- ⏳ Configuration backup/restore

### Performance Requirements

**Scalability:**
- Support 500+ boards simultaneously
- <10ms latency for broadcast commands
- <100ms UI update rate
- <1% packet loss at full load

**Hardware Requirements:**
- **CPU:** Quad-core 2.5GHz+
- **RAM:** 8 GB
- **Network:** Gigabit Ethernet (dedicated port)
- **OS:** Windows 10+, Linux, macOS

### Development Status

**Current State (February 2026):**
- ✅ FBC Protocol defined (`gui/src-tauri/src/fbc.rs`)
- ✅ Tauri backend with 35+ commands (`gui/src-tauri/src/lib.rs`)
- ✅ State management (`gui/src-tauri/src/state.rs`)
- ✅ React frontend with all panels (`gui/src/components/`)
- ✅ 3D rack visualization with Three.js
- ✅ Zustand store for global state
- ✅ All major panels implemented:
  - Sidebar navigation
  - 3D Rack View (click-to-select boards)
  - Board Detail Panel
  - Analog Monitor (32ch ADC)
  - Power Control (VICOR + PMBus)
  - EEPROM Viewer/Editor
  - Vector Engine (load/run/pause)
  - Device Config (pin mapping, timing)
  - Terminal (command interface)

**Remaining Work:**
1. Test plan management UI (load/save JSON)
2. Rack configuration editor
3. Results export (CSV/JSON/STDF)
4. Hardware testing with real boards

---

## Comparison

| Feature | FPGA Toolchain GUI | FBC System GUI |
|---------|-------------------|----------------|
| **Purpose** | FPGA development | Production testing |
| **Users** | Engineers | Operators |
| **Boards** | 1 (target device) | 500+ (parallel) |
| **Protocol** | File I/O | Raw Ethernet |
| **Complexity** | Simple (3 steps) | Complex (multi-view) |
| **Status** | ✅ Implemented | ✅ Implemented (90%) |
| **Technology** | egui + eframe | Tauri + React |
| **Runtime** | Minutes (build) | Hours (burn-in) |

---

## Design Principles (Both GUIs)

### ONETWO for GUI Design

**Invariant (ONE):** Users want to complete tasks, not learn tools
- Minimize clicks
- Self-explanatory labels
- Obvious next steps

**Variation (TWO):** Different users, different contexts
- Toolchain: Engineers who understand Verilog
- System: Operators who may not know electronics

**Pattern:** Progressive disclosure
- Simple interface by default
- Advanced features hidden until needed
- Contextual help

### User Experience Goals

1. **No Command Line Required**
   - Everything clickable
   - File pickers, not paths
   - Visual feedback

2. **Real-Time Feedback**
   - Progress bars
   - Live status updates
   - Error messages with context

3. **Cross-Platform**
   - Same UI on Windows/Linux/macOS
   - Native look and feel
   - No web browser dependency

4. **Performance**
   - 60 FPS rendering
   - Instant response to clicks
   - Background threading for heavy work

---

## Future Integration

**Potential Enhancements:**
- 📊 Share device profiles between toolchain and system GUI
- 🔧 Integrate bitstream builder into system GUI (build + upload in one)
- 📈 Add waveform viewer to toolchain GUI
- 🔄 Remote firmware update from toolchain GUI

---

## Test Plan Workflow

See `docs/OPERATIONAL_WORKFLOW.md` for complete end-to-end workflow documentation including:
- Test plan JSON schema
- Rack configuration format
- EEPROM storage layout
- Operational modes (Manual, Test Plan, Rack Config)
- Data flow diagrams

---

*Last updated: 2026-02-02*
