# CLAUDE.md - FBC Semiconductor System Context

This file provides context for AI assistants working on this project.

---

## Project Overview

**What:** Burn-in test system for semiconductor chips using ~44 Zynq 7020 FPGA boards in each system.

**Goal:** Modernize the 2016 Sonoma/kzhang_v2 system:
- Replace Linux with bare-metal Rust firmware (faster boot, more reliable)
- Build custom FPGA toolchain (eliminate Vivado dependency, learn the internals)
- Optimize RTL for efficiency
- Learn every layer from gates to network packets

**Owner:** Isaac Nudeton / ISE Labs

---

## BEFORE DOING ANYTHING

1. **READ THE ACTUAL CODE** - Do not assume, do not guess, do not propose changes without reading
2. **Key files to read first:**
   - `firmware/src/main.rs` - Main loop, heartbeat, error handling, SD logging
   - `firmware/src/fbc_protocol.rs` - All packet formats and protocol
   - `rtl/fbc_top.v` - FPGA top level integration
   - `gui/src-tauri/src/state.rs` - GUI backend socket/state management
3. **Use correct terminology:**
   - VICOR - 6 core high-current supplies
   - LCPS - Low Current Power Supply (PMBus)
   - BIM - Board Interface Module (EEPROM identifies board type)
   - DUT - Device Under Test
   - Fast Pins - gpio[128:159], direct FPGA, 1-cycle latency
   - BIM Pins - gpio[0:127], through Quad Board, 2-cycle latency

---

## Current Status (February 2026)

### What Works
- Custom FPGA toolchain: Verilog parser, synthesis, tech mapping, P&R, bitstream
- ONETWO pattern learning: extract bit positions from Vivado examples (no Project X-Ray needed)
- Optimization engine: timing/power/area profiles with criticality-aware P&R
- Verilog simulator: behavioral testbench execution with VCD output
- RTL modules: decoder, vector engine, I/O subsystem, clock gen, ARM interfaces
- Firmware HAL: Complete driver suite (see below)
- Vector format converter: fbc-vec tool (tools/fbc-vec/) - STIL/AVC/PAT/APS input, FBC/Sonoma/PAT output, tested with real files (145-95,952x compression)
- Device config compiler: fbc-config tool (tools/fbc-config/) - Sonoma .bim/.map/.tim to binary
- **GUI: Tauri + React + Three.js** - 3D rack view, all major panels complete
- Reference documentation: firmware analysis, Zynq register maps

### Complete Self-Contained Toolchain (NO XILINX SDK)

The entire build system is vendor-independent:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        BUILD PIPELINE                                   │
├─────────────────────────────────────────────────────────────────────────┤
│  1. Firmware:    cargo build --release --target armv7a-none-eabi        │
│                  → firmware/target/.../fbc-firmware (ELF)               │
│                                                                         │
│  2. Bootgen:     tools/bootgen patch --base BOOT.bin --app fbc-firmware │
│                  → BOOT.BIN (ready for SD card)                         │
│                                                                         │
│  3. FSBL:        reference/sonoma_extracted/v4.8C/FSBL.elf              │
│                  (extracted from working boards, contains DDR timing)   │
│                                                                         │
│  4. Bitstream:   fpga-toolchain/fbc-synth build rtl/*.v -o top.bit      │
│                  (ONETWO-derived, no Vivado needed)                     │
└─────────────────────────────────────────────────────────────────────────┘
```

**Key insight (ONETWO on FSBL):** The FSBL is just a register write sequence.
Board-specific DDR timing values were extracted via disassembly and documented
in `docs/FSBL_DDR_ANALYSIS.md`. The HAL now includes `ddr.rs` with these values.

### Firmware Drivers (All Complete)
| Driver | File | Description |
|--------|------|-------------|
| I2C | `i2c.rs` | I2C master for PMBus |
| SPI | `spi.rs` | SPI master for ADC/DAC |
| GPIO | `gpio.rs` | MIO pin control |
| XADC | `xadc.rs` | Internal ADC (16ch, temp, voltages) |
| UART | `uart.rs` | Serial console |
| PCAP | `pcap.rs` | FPGA programming |
| PMBus | `pmbus.rs` | Power supply control (99 devices) |
| Thermal | `thermal.rs` | ONETWO temperature control |
| EEPROM | `eeprom.rs` | 24LC02 persistent storage (256B) |
| DNA | `dna.rs` | Device unique ID, MAC generation |
| **MAX11131** | `max11131.rs` | External ADC (16ch, 12-bit, 3Msps) |
| **BU2505** | `bu2505.rs` | External DAC (10ch, 10-bit) |
| **VICOR** | `vicor.rs` | 6 core supply controller |
| **AnalogMonitor** | `analog.rs` | Unified 32ch interface for GUI |
| **DDR** | `ddr.rs` | DDR controller (board-specific timing from FSBL) |

### GUI Components (All Complete)
| Component | File | Description |
|-----------|------|-------------|
| Sidebar | `Sidebar.tsx` | Collapsible nav, 9 views, board summary |
| 3D Rack | `RackView.tsx` | Three.js rack visualization, click-to-select |
| Board Detail | `BoardDetailPanel.tsx` | Stats, EEPROM info, test controls, history |
| Analog Monitor | `AnalogMonitorPanel.tsx` | 32ch ADC display, filtering, grid/list |
| Power Control | `PowerControlPanel.tsx` | 6 VICOR cores, PMBus rails, emergency stop |
| EEPROM Viewer | `EepromPanel.tsx` | Header, rails, calibration, hex view |
| Vector Engine | `VectorEnginePanel.tsx` | Load/run/pause, progress, file browser |
| Device Config | `DeviceConfigPanel.tsx` | Pin mapping, timing diagram, overview |
| **Test Plan Editor** | `TestPlanEditor.tsx` | Full .tp format, steps, ADC monitors, preview |

### What's Needed
- Rack configuration editor
- Results export (CSV/JSON/STDF)
- Hardware testing on actual Zynq board

### Progress Summary
| Component | % | Notes |
|-----------|---|-------|
| RTL | 95% | 160-pin I/O, 4 clock outputs, all AXI peripherals |
| Toolchain | 99% | Separate repo: `C:\Dev\projects\fpga-toolchain` |
| Firmware | 90% | Full HAL + ADC/DAC/VICOR + network pending |
| Host CLI | 20% | Skeleton |
| **GUI** | **95%** | All panels complete, backend integration 70% |
| Tests | 40% | Decoder, io_cell, io_bank, clk_gen, top |

**See `STATUS.md` for current blockers and next actions.**

---

## Directory Structure

```
C:\Dev\projects\FBC-Semiconductor-System\   # Main project
│
├── rtl/                   # Verilog RTL design
│   ├── fbc_pkg.vh         # Global defines (VECTOR_WIDTH, PIN_COUNT, etc.)
│   ├── system_top.v       # Top-level wrapper (PS7, clk_gen, fbc_top)
│   ├── fbc_top.v          # FBC core (decoder, vector_engine, io_bank)
│   ├── fbc_decoder.v      # Bytecode → vectors
│   ├── vector_engine.v    # Repeat counter, timing
│   ├── io_bank.v          # 160 I/O cells (128 BIM + 32 fast)
│   ├── io_cell.v          # Single I/O cell with pin types
│   ├── io_config.v        # AXI-Lite pin configuration
│   ├── clk_gen.v          # MMCM + MUX clock generation
│   ├── clk_ctrl.v         # AXI-Lite clock control
│   ├── error_counter.v    # Error tracking + BRAM
│   ├── axi_fbc_ctrl.v     # Main control registers
│   ├── axi_vector_status.v # Status readback
│   ├── axi_freq_counter.v # Frequency measurement
│   └── axi_stream_fbc.v   # AXI Stream interface
│
├── tb/                    # Testbenches
│   └── *.v                # Unit and integration tests
│
├── constraints/           # FPGA pin constraints
│   └── zynq7020_sonoma.xdc # Pin assignments for Zynq 7020
│
├── firmware/              # ARM Cortex-A9 bare-metal Rust
│   └── src/
│       ├── main.rs        # Entry point, main loop
│       └── hal/           # Hardware drivers (I2C, SPI, GPIO, etc.)
│
├── gui/                   # Tauri + React GUI application
│   ├── src/               # React frontend
│   └── src-tauri/         # Rust backend
│
├── tools/                 # Standalone tools
│   ├── fbc-vec/           # Vector converter (STIL/AVC/PAT → FBC)
│   ├── fbc-config/        # Device config compiler
│   └── bootgen/           # BOOT.BIN generator
│
├── host/                  # CLI for multi-board control
│
├── reference/             # 2016 kzhang_v2 design (for comparison)
│
├── docs/                  # Architecture docs, API specs
│
├── testplans/             # Example test plan files
│
├── learning/              # HTML progress tracker
│
├── STATUS.md              # Current blockers and next actions
├── TODO.md                # Detailed roadmap
└── CLAUDE.md              # This file

C:\Dev\projects\fpga-toolchain\            # SEPARATE REPO
├── src/                   # Verilog → bitstream pipeline
│   ├── verilog.rs         # Parser + elaborator
│   ├── synth.rs           # Logic synthesis
│   ├── place.rs           # Placement (simulated annealing)
│   ├── route.rs           # Routing (PathFinder)
│   ├── bitstream.rs       # Bitstream generation
│   ├── learn.rs           # ONETWO pattern learning
│   └── optimize.rs        # Timing/power/area profiles
└── docs/                  # ONETWO methodology docs
```

---

## Key Technical Details

### Target Device
- **Part:** xc7z020clg400-1 (Xilinx Zynq 7020)
- **Package:** 400-pin (reference uses 484-pin variant)
- **Resources:** 53,200 LUTs, 106,400 FFs, 140 BRAMs
- **ARM:** Dual Cortex-A9 @ 667MHz
- **Fabric:** 85K logic cells

### Pin Configuration
- **GPIO count:** 160 pins to DUT
- **Vector width:** 128 bits
- **Repeat width:** 32 bits

### Pin Types (from vector.vh)
```verilog
BIDI_PIN       = 4'b0000  // Bidirectional
INPUT_PIN      = 4'b0001  // Input only (compare)
OUTPUT_PIN     = 4'b0010  // Output only (drive)
OPEN_C_PIN     = 4'b0011  // Open collector
PULSE_PIN      = 4'b0100  // Pulse (edge at T/4, 3T/4)
NPULSE_PIN     = 4'b0101  // Inverted pulse
ERROR_TRIG     = 4'b0110  // Error trigger output
VEC_CLK_PIN    = 4'b0111  // Vector clock output
VEC_CLK_EN_PIN = 4'b1000  // Clock enable output
```

### Clocking
- FCLK_CLK0: 100 MHz (AXI bus clock)
- FCLK_CLK1: 200 MHz (vector timing base)
- vec_clk: Variable (derived from PLL)
- vec_clk_90, vec_clk_180: Phase-shifted for pulse timing

---

## Development Roadmap

See `TODO.md` for detailed checklist. High-level phases:

1. **Phase 1:** Study reference design (DONE)
2. **Phase 2:** RTL development (DONE - I/O, clock gen, ARM interfaces)
3. **Phase 3:** Simulation & verification (40%)
4. **Phase 4:** Firmware bare-metal Rust (75%)
5. **Phase 5:** Toolchain + ONETWO + Optimization (99%)
6. **Phase 6:** Host tools + GUI (90% - all panels + backend done)
7. **Phase 7:** Hardware integration (0%)

**Next steps:**
- Test plan management UI
- Rack configuration editor
- Hardware testing with real Zynq boards

---

## FPGA Toolchain Usage

**Note:** The toolchain is now a separate repository at `C:\Dev\projects\fpga-toolchain`.

Build and run:
```bash
cd C:\Dev\projects\fpga-toolchain
cargo build --release

# Build FBC design
./target/release/fbc-synth build \
    ../FBC-Semiconductor-System/rtl/fbc_pkg.vh \
    ../FBC-Semiconductor-System/rtl/system_top.v \
    ../FBC-Semiconductor-System/rtl/*.v \
    -o ../FBC-Semiconductor-System/top.bit
```

Output:
```
[1/6] Parsing Verilog...
[2/6] Elaborating...
[3/6] Synthesizing...
[4/6] Technology mapping...
       Netlist Statistics:
  Cells: 855
  Nets:  970
  LUTs:  755
  FFs:   60
[5/6] Placing...
[6/6] Routing...
Routing complete after 9 iterations
```

### Optimization Profiles

Generate optimal bitstreams for specific use cases:

```bash
# Timing-critical (minimize delay on specified nets)
./target/release/fbc-synth build design.v -o design.bit \
    --optimize timing --critical-nets "vec_clk,data_out"

# Burn-in tester (pre-configured for FBC: fast vec_clk, high reliability)
./target/release/fbc-synth build design.v -o design.bit --optimize burn_in

# Low power (minimize switching activity)
./target/release/fbc-synth build design.v -o design.bit --optimize power

# Minimum area (pack logic tightly)
./target/release/fbc-synth build design.v -o design.bit --optimize area
```

Profiles affect both placement (critical cells closer together) and routing
(critical nets win wire contention, get shorter paths).

---

## Common Issues & Solutions

### Routing stuck at N overused wires
- **Cause:** present_factor too low, nets don't detour
- **Fix:** In route.rs, increase present_factor (currently 4.0)

### Bitstream ready for hardware testing
- **Status:** Routing PIPs complete via ONETWO learning (Jan 2026)
- **LUT/FF configuration:** Correct (words 4-11, 54-61)
- **Routing configuration:** Complete (word 50, 86-entry PIP database)
- **Validation:** 3,488 frames configured, 100% pattern match with reference
- **Pending:** Hardware testing on Zynq 7020 board
- **Details:** See `docs/bitstream_format.md` and `C:\Dev\scratch\onetwo_routing_validation.md`

### Elaboration produces too few LUTs
- **Cause:** elaborate_assign not connecting drivers
- **Fix:** Check that buffer cells are created for assignments

---

## ONETWO Pattern Learning

Instead of Project X-Ray's 100MB+ database, we derived the bitstream format by analyzing Vivado outputs:

### What We Learned

**Frame Structure (101 words per frame):**
- Words 0-49: CLB0 (lower slice) configuration
- Word 50: Routing/interconnect muxes, FF init bits
- Words 51-100: CLB1 (upper slice) configuration

**LUT Word Placement:**
| Slice | LUT A | LUT B | LUT C | LUT D |
|-------|-------|-------|-------|-------|
| CLB0  | 4-5   | 6-7   | 8-9   | 10-11 |
| CLB1  | 54-55 | 56-57 | 58-59 | 60-61 |

**FF Init:** Word 50, bits 0-15

### Verification

```
=== Our Output ===              === Vivado Reference ===
words 4-5:  211 bits            words 4-5:  47190 bits
words 6-7:  259 bits            words 6-7:  48192 bits
words 8-9:  259 bits            words 8-9:  49756 bits
Word spacing: 2                 Word spacing: 2
```

### Commands

```bash
fbc-synth analyze <file.bit>   # Frame/bit statistics
fbc-synth patterns <file.bit>  # Word usage patterns
fbc-synth diff <a.bit> <b.bit> # Compare bitstreams
```

See `docs/bitstream_format.md` for full documentation.

**Multi-device capability:** The ONETWO approach works for any FPGA. Analyze vendor bitstreams, extract patterns, implement in bitstream.rs. No vendor database needed.

---

## Reference Files Location

Original 2016 design (OneDrive, read-only):
```
C:\Users\isaac\OneDrive - ISE Labs\IMPORTANT DOCUMENTS\Training\Volt\kzhang_v2\
```

Copied to project (can modify):
```
C:\Dev\projects\FBC Semiconductor System\reference\kzhang_v2_2016\
```

---

## OneDrive Policy

Per user's global CLAUDE.md:
- Treat OneDrive paths as **read-only**
- Do NOT write/edit/move/delete files under OneDrive
- Write outputs to `C:\Dev\scratch\` or project directories

---

## Commands Reference

### Simulation
```bash
make sim-fbc          # Decoder testbench
make sim-top          # Top-level testbench
```

### Build
```bash
# Firmware
cd firmware && cargo build --release --target armv7a-none-eabi

# GUI
cd gui && npm run tauri build

# Toolchain (separate repo)
cd C:\Dev\projects\fpga-toolchain && cargo build --release
```

---

## CRITICAL: Complete Data Flow (READ THIS FIRST)

**DO NOT MAKE ASSUMPTIONS. READ THE ACTUAL CODE FILES LISTED BELOW.**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              FPGA (RTL)                                     │
│  rtl/fbc_top.v → fbc_decoder.v → vector_engine.v → io_bank.v               │
│  rtl/error_counter.v - tracks errors, first_error_vector/cycle, BRAM       │
│  rtl/axi_fbc_ctrl.v - AXI registers: STATUS, CYCLE, INSTR, ERROR, FAST_PINS│
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         FIRMWARE (firmware/src/)                            │
│  main.rs lines 142-212 - MAIN LOOP:                                         │
│    - Receives packets via raw Ethernet (0x88B5)                            │
│    - Reads FPGA status via AXI (cycles, errors, state)                     │
│    - Sends HEARTBEAT every ~100ms when Running (lines 181-205)             │
│    - Sends ERROR packet on error state transition (lines 166-173)          │
│    - FLIGHT RECORDER: Logs heartbeats to SD card sectors 1001-2000         │
│                                                                             │
│  fbc_protocol.rs - Packet definitions:                                      │
│    - HEARTBEAT: cycles(4), errors(4), temp(2), state(1)                    │
│    - STATUS_RSP: full telemetry (47 bytes)                                 │
│    - ERROR: error_type, cycle, error_count, details                        │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           GUI (gui/src-tauri/)                              │
│  state.rs - FbcSocket, send/recv packets                                    │
│  lib.rs - 35+ Tauri commands                                                │
│                                                                             │
│  LIVE MONITORING = GUI polls STATUS_REQ → Firmware responds STATUS_RSP     │
│  LOGGING = GUI stores what it receives (that IS the log)                   │
│  DURATION = run_time_ms in VectorEngineStatus                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Architecture Facts

1. **LIVE MONITORING** - GUI polls, firmware responds, GUI displays AND stores
2. **HEARTBEAT** - Sent every ~100ms when Running, ALSO logged to SD card (Flight Recorder)
3. **ERROR EVENTS** - ERROR packet sent immediately on error with cycle count (causality)
4. **AUTONOMOUS OPERATION** - Controller keeps running when GUI disconnects, logs to SD
5. **VECTORS** - Can be cached on controller SD card (compression: 145-95,952x)

### Why Old CSV Logging Was Broken

Old Sonoma system logged to CSV columns left-to-right. If Supply A dies → Supply B shuts down (because A died) → CSV reports B as failure (wrong column order). New system uses timestamped events with cycle counts for causality.

---

## Notes for Future Sessions

1. **Check `STATUS.md`** for current blockers and next actions
2. **READ THE ACTUAL CODE** before making any assumptions
3. Reference design is in `reference/kzhang_v2_2016/`
4. **FPGA toolchain is separate:** `C:\Dev\projects\fpga-toolchain`
5. ONETWO validation report: `C:\Dev\scratch\onetwo_routing_validation.md`
6. **Firmware API:** see `docs/FIRMWARE_API_FOR_GUI.md`
7. **Operational workflow:** see `docs/OPERATIONAL_WORKFLOW.md`
8. Hardware specs: MAX11131 ADC (SPI0/CS1), BU2505 DAC (SPI0/CS0)
9. GUI stack: Tauri + React + Three.js + Zustand
10. **Vectors can be on controller** - SD card caching
11. **Controller runs autonomously** - Flight Recorder on SD card
12. Legacy system reference: `docs/SONOMA_EVEREST_ARCHITECTURE.md`

---

## GUI Architecture

### Technology Stack
- **Framework:** Tauri (Rust backend + web frontend)
- **Frontend:** React 18 + TypeScript
- **3D Graphics:** Three.js + React Three Fiber
- **State:** Zustand (lightweight Redux alternative)
- **Styling:** CSS with CSS variables for theming

### Components Overview
```
gui/src/
├── App.tsx              # Main app, view switching
├── store.ts             # Global state (boards, connection)
├── styles/
│   ├── global.css       # CSS variables, reset
│   └── app.css          # Layout styles
└── components/
    ├── Sidebar.tsx      # Navigation (8 views)
    ├── RackView.tsx     # 3D rack (11 shelves, 8 boards each)
    ├── BoardDetailPanel.tsx   # Stats, EEPROM, controls
    ├── AnalogMonitorPanel.tsx # 32ch ADC (XADC + MAX11131)
    ├── PowerControlPanel.tsx  # VICOR cores + PMBus rails
    ├── EepromPanel.tsx        # 256B config viewer/editor
    ├── VectorEnginePanel.tsx  # Vector load/run/progress
    ├── DeviceConfigPanel.tsx  # Pin/timing config
    ├── StatusPanel.tsx        # Board list
    ├── FacilityPanel.tsx      # Facility controls
    ├── Terminal.tsx           # Command terminal
    └── Toolbar.tsx            # Connection management
```

### Build Commands
```bash
cd gui
npm install              # Install dependencies
npm run dev              # Development server (hot reload)
npm run build            # Production build
npm run tauri dev        # Full Tauri development
npm run tauri build      # Production Tauri build
```
