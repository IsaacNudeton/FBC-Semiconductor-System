# QWEN.md — FBC Semiconductor System

Comprehensive context file for AI-assisted development of the FBC Semiconductor burn-in test system.

---

## What This Is

**FBC Semiconductor System** is a modernized burn-in test platform for semiconductor chips, replacing a 2016 Linux-based design with:

- **Bare-metal Rust firmware** running on ARM Cortex-A9 (Zynq 7020) — boots in <1s
- **Custom FPGA toolchain** (ONETWO-derived) — no Vivado required
- **Raw Ethernet protocol** (EtherType 0x88B5) — no TCP/IP stack overhead
- **Tauri + React GUI** — modern control interface for ~500 concurrent Zynq controllers

**System Scale:** Each rack contains ~44 Zynq 7020 boards; each board controls 160 GPIO pins (128 BIM + 32 fast) for chip-under-test interfacing.

**Owner:** Isaac Nudeton / ISE Labs

---

## Quick Reference

| What | File/Path | Purpose |
|------|-----------|---------|
| FBC instruction set | `rtl/fbc_pkg.vh` | Opcodes, pin types, timing constants |
| Top-level integration | `rtl/system_top.v` | Zynq PS7 + AXI peripherals |
| Protocol wire format | `firmware/src/fbc_protocol.rs` | 27 commands, all payload structs |
| Register map | `firmware/src/regs.rs` | AXI register offsets (verified vs RTL) |
| Main firmware loop | `firmware/src/main.rs` | Boot, networking, command dispatch |
| GUI protocol client | `gui/src-tauri/src/fbc.rs` | Raw Ethernet socket, types |
| GUI state machine | `gui/src-tauri/src/state.rs` | Command send/recv, payload parsing |

---

## Project Structure

```
FBC-Semiconductor-System/
├── rtl/                    # 14 Verilog modules
│   ├── fbc_pkg.vh          # Global defines, opcodes, pin types
│   ├── fbc_top.v           # FBC wrapper
│   ├── fbc_decoder.v       # Bytecode decoder (7 opcodes)
│   ├── vector_engine.v     # Test vector execution
│   ├── error_counter.v     # Pin mismatch counting
│   ├── axi_fbc_ctrl.v      # AXI-Lite control interface
│   ├── axi_stream_fbc.v    # AXI-Stream to FBC decoder
│   ├── fbc_dma.v           # AXI DMA for vector loading
│   ├── error_bram.v        # Error logging BRAMs (×3)
│   ├── io_bank.v           # 160-pin I/O bank (128 BIM + 32 fast)
│   ├── io_cell.v           # Single pin I/O cell
│   ├── io_config.v         # Pin type configuration
│   ├── clk_ctrl.v          # Clock control (freq_sel)
│   ├── clk_gen.v           # MMCM clock generation
│   ├── axi_vector_status.v # Vector execution status
│   ├── axi_freq_counter.v  # 8-channel frequency counter
│   └── system_top.v        # Top-level: PS7 + all AXI peripherals
├── tb/                     # Verilog testbenches
│   ├── fbc_decoder_tb.v
│   ├── io_bank_tb.v
│   ├── clk_gen_tb.v
│   └── ...
├── firmware/               # Bare-metal Rust (armv7a-none-eabi)
│   ├── src/
│   │   ├── main.rs         # Entry point, boot, main loop
│   │   ├── fbc_protocol.rs # Protocol: 27 commands, payloads
│   │   ├── regs.rs         # Memory-mapped register access
│   │   ├── net.rs          # Zynq GEM Ethernet driver
│   │   ├── dma.rs          # AXI DMA + FbcStreamer
│   │   ├── analog.rs       # 32-channel ADC (XADC + MAX11131)
│   │   ├── fbc.rs          # FBC hardware interface
│   │   ├── fbc_loader.rs   # Vector loading
│   │   ├── fbc_decompress.rs # FBC decompression
│   │   └── hal/            # Hardware Abstraction Layer (17 drivers)
│   │       ├── gpio.rs     # MIO/EMIO GPIO
│   │       ├── xadc.rs     # XADC monitoring
│   │       ├── i2c.rs      # I2C controller
│   │       ├── spi.rs      # SPI controller
│   │       ├── sd.rs       # SD card
│   │       ├── uart.rs     # UART
│   │       ├── vicor.rs    # VICOR core power supplies (6ch)
│   │       ├── pmbus.rs    # PMBus (LCPS)
│   │       ├── eeprom.rs   # EEPROM (BIM)
│   │       ├── max11131.rs # MAX11131 ADC
│   │       ├── bu2505.rs   # BU2505 DAC
│   │       ├── slcr.rs     # System-Level Control Registers
│   │       ├── ddr.rs      # DDR initialization
│   │       ├── dna.rs      # Device DNA
│   │       ├── pcap.rs     # PCAP (FPGA reflash)
│   │       └── thermal.rs  # Thermal monitoring
│   ├── Cargo.toml
│   └── link.ld             # Linker script
├── gui/                    # Tauri + React + Three.js
│   ├── src/                # React frontend
│   ├── src-tauri/          # Rust backend
│   │   ├── src/
│   │   │   ├── lib.rs      # Tauri commands (54 total)
│   │   │   ├── fbc.rs      # FBC protocol client
│   │   │   ├── state.rs    # AppState (connection, discovery)
│   │   │   ├── config.rs   # Rack configuration
│   │   │   ├── export.rs   # Results export
│   │   │   ├── switch.rs   # Serial switch control
│   │   │   └── realtime.rs # Realtime telemetry
│   │   └── tauri.conf.json
│   └── package.json
├── host/                   # CLI tool (multi-board control)
│   ├── src/
│   │   ├── lib.rs          # FBC host library
│   │   └── bin/
│   │       ├── cli.rs      # CLI: discover, ping
│   │       └── fvec.rs     # fbc-vec: STIL/AVC → FBC converter
│   └── Cargo.toml
├── fsbl/                   # First Stage Boot Loader (Rust)
├── constraints/            # XDC pin constraints
│   ├── zynq7020.xdc
│   └── zynq7020_sonoma.xdc
├── docs/                   # Documentation
│   ├── register_map.md     # AXI register map
│   ├── GUI.md              # GUI documentation
│   ├── HAL_API.md          # HAL API reference
│   ├── PIN_MAPPING.md      # Pin mapping reference
│   ├── FIRMWARE_API_FOR_GUI.md
│   ├── FSBL_DDR_ANALYSIS.md
│   └── VICOR_ADC_DAC_USAGE.md
├── reference/              # 2016 Sonoma/kzhang_v2 (READ ONLY)
│   ├── sonoma_docs/        # Verified Sonoma documentation
│   ├── kzhang_v2_2016/     # Original HDL source
│   └── ...
├── tools/                  # Utilities
│   ├── onetwo_routing_verify.py
│   └── rawwrite.c
├── testplans/              # Test plan examples
├── onetwo.c                # ONETWO reasoning scaffold
├── CLAUDE.md               # AI context (project state & bugs)
└── README.md               # Project overview
```

---

## Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         HOST PC                                     │
│  ┌──────────────┐                                                   │
│  │  FBC GUI     │  (Tauri + React + Three.js)                      │
│  │  (Raw Eth)   │────────────────────┐                              │
│  └──────────────┘                    │                              │
└──────────────────────────────────────┼──────────────────────────────┘
                                       │ Raw Ethernet (0x88B5)
                                       │
┌──────────────────────────────────────┼──────────────────────────────┐
│                      ZYNQ 7020 BOARD (×44 per rack)                 │
│  ┌──────────────────────────┐    ┌──────────────────────────────┐  │
│  │      ARM Cortex-A9       │    │        FPGA Fabric (PL)      │  │
│  │  ┌────────────────────┐  │◄──►│  ┌────────────────────────┐  │  │
│  │  │    firmware/       │  │AXI │  │        rtl/            │  │  │
│  │  │  (bare-metal Rust) │  │    │  │  fbc_decoder           │  │  │
│  │  │   - Ethernet       │  │    │  │  vector_engine         │  │  │
│  │  │   - FBC protocol   │  │    │  │  error_counter         │  │  │
│  │  │   - HAL drivers    │  │    │  │  io_bank (160 pins)    │  │  │
│  │  └────────────────────┘  │    │  └────────────────────────┘  │  │
│  └──────────────────────────┘    └──────────────┬───────────────┘  │
│                                                  │                  │
└──────────────────────────────────────────────────┼──────────────────┘
                                                   ▼
                                          ┌───────────────┐
                                          │  Chip Under   │
                                          │     Test      │
                                          └───────────────┘
```

---

## ⚠️ CRITICAL GAP: Pattern Converter Missing `.fbc` Output

**UPDATE March 2026:** This gap has been **FIXED**. `gen_fbc.c` exists and is integrated.

~~**The Problem:**~~
~~```
ATP/STIL/AVC (customer patterns)
    ↓
gui/src-tauri/c-engine/pc/ (C engine — 14 files, COMPLETE)
    ↓
.hex + .seq (Legacy format — 40 bytes/vector, uncompressed)
    ↓
❌ NO PATH TO .fbc (FBC compressed format — 1-21 bytes/vector)
```~~

~~**Why This Matters:**~~
~~**Legacy system** uses `.hex` (40 bytes/vector)~~
~~**FBC system** uses `.fbc` (compressed: VECTOR_ZERO=1B, VECTOR_RUN=5B, VECTOR_SPARSE=2+N bytes)~~
~~**Compression ratio:** 4.8-710x smaller (verified: test_core.fbc = 77KB vs 55MB uncompressed)~~
~~**Customer migration:** All existing ATP/STIL/AVC patterns need `.fbc` output for FBC system~~

~~**What Exists:**~~
| Converter | Input | Output | Status |
|-----------|-------|--------|--------|
| **C Engine** (`gui/src-tauri/c-engine/pc/`) | ATP/STIL/AVC | `.hex` + `.seq` | ✅ Complete (14 C files) |
| **Rust Compiler** (`host/src/vector/`) | `.fvec` (text) | `.fbc` | ✅ Complete |
| **gen_fbc.c** (`gui/src-tauri/c-engine/pc/`) | PcPattern IR | `.fbc` | ✅ **Complete March 2026** |

~~**Implementation Options:**~~
~~1. **Add `gen_fbc.c` to C engine** — New file outputs `.fbc` opcodes (fastest path, 1-2 days)~~
~~2. **Add `.hex` → `.fbc` converter in Rust** — Intermediate format conversion (2-3 days)~~
~~3. **Port C parsers to Rust** — Full rewrite (1-2 weeks)~~

~~**Recommended:** Option 1 — Add `gen_fbc.c` to `gui/src-tauri/c-engine/pc/` — **DONE**~~

~~**See:** `docs/PATTERN_CONVERTER_MIGRATION.md` for full implementation plan.~~

---

### AXI Memory Map

| Peripheral | Base Address | Size | Description |
|------------|--------------|------|-------------|
| `axi_fbc_ctrl` | 0x4004_0000 | 4KB | FBC decoder control |
| `io_config` | 0x4005_0000 | 4KB | Pin type configuration |
| `axi_vector_status` | 0x4006_0000 | 4KB | Vector execution status |
| `axi_freq_counter` | 0x4007_0000 | 4KB | 8-channel frequency counter |
| `clk_ctrl` | 0x4008_0000 | 4KB | Clock control (freq_sel) |
| `fbc_dma` | 0x4040_0000 | 4KB | AXI DMA (wired in system_top.v) |
| `error_bram` | 0x4009_0000 | 4KB | Error logging BRAMs (×3) |

### FBC Protocol Commands (28 total)

| Subsystem | Commands | Count |
|-----------|----------|-------|
| Setup | ANNOUNCE, BIM_STATUS_REQ/RSP, WRITE_BIM, UPLOAD_VECTORS, CONFIGURE | 5 |
| Runtime | START, STOP, RESET, HEARTBEAT, ERROR, STATUS_REQ/RSP | 7 |
| Analog | READ_ALL_REQ/RSP | 2 |
| Power (VICOR) | VICOR_STATUS_REQ/RSP, VICOR_ENABLE, VICOR_SET_VOLTAGE, EMERGENCY_STOP, POWER_SEQ_ON/OFF | 6 |
| Power (PMBus) | PMBUS_STATUS_REQ/RSP, PMBUS_ENABLE | 3 |
| EEPROM | READ_REQ/RSP, WRITE, WRITE_ACK | 4 |
| FastPins | READ_REQ/RSP, WRITE | 3 |
| Vector Engine | STATUS_REQ/RSP, LOAD, LOAD_ACK, START, PAUSE, RESUME, STOP | 8 |
| Firmware Update | INFO_REQ/RSP, BEGIN, BEGIN_ACK, CHUNK, CHUNK_ACK, COMMIT, COMMIT_ACK, ABORT | 9 |
| Flight Recorder | LOG_INFO_REQ/RSP, LOG_READ_REQ/RSP | 4 |
| Error Log | ERROR_LOG_REQ/RSP | 2 |

**Note:** Counts exceed 28 because some commands are bidirectional (REQ/RSP pairs).

### FBC Opcodes (Bytecode)

| Opcode | Hex | Description |
|--------|-----|-------------|
| NOP | 0x00 | No operation |
| HALT | 0xFF | End of program |
| LOOP_N | 0xB0 | Loop next block N times (NOT FUNCTIONAL — no instruction buffer) |
| PATTERN_REP | 0xB5 | Repeat current pattern N times |
| PATTERN_SEQ | 0xB6 | Generate sequence |
| SET_PINS | 0xC0 | Set pin values (128-bit payload) |
| SET_OEN | 0xC1 | Set output enables (128-bit payload) |
| SET_BOTH | 0xC2 | Set both pins and OEN (256-bit payload) |
| WAIT | 0xD0 | Wait N cycles |
| SYNC | 0xD1 | Wait for external trigger (UNIMPLEMENTED) |
| IMM32 | 0xE0 | 32-bit immediate (UNIMPLEMENTED) |
| IMM128 | 0xE1 | 128-bit immediate (UNIMPLEMENTED) |

---

## Building and Running

### Firmware (Bare-Metal ARM)

```bash
cd firmware
cargo build --release --target armv7a-none-eabi
# Output: target/armv7a-none-eabi/release/fbc-firmware
```

**Flash to SD card:**
```bash
# Combine FSBL + firmware + bitstream (implementation-dependent)
```

### GUI (Tauri + React)

```bash
cd gui
npm install          # First time
npm run tauri dev    # Development mode
npm run tauri build  # Production build
```

### Host CLI

```bash
cd host
cargo build --release
# Output: target/release/fbc-cli, target/release/fbc-vec
```

### FPGA (Custom Toolchain)

The project references a separate `fpga-toolchain` repository for building bitstreams from Verilog:

```bash
cd C:\Dev\projects\fpga-toolchain
cargo build --release
./target/release/fbc-synth build ../rtl/*.v -o ../top.bit
```

### Simulation (Icarus Verilog)

No Makefile present. Individual testbenches can be run with:

```bash
iverilog -o sim tb/fbc_decoder_tb.v rtl/*.v
vvp sim
```

---

## Known Bugs (Verified March 2026)

### 🟡 High Priority

| Bug | Status | Details |
|-----|--------|---------|
| LOOP_N non-functional | **ACTIVE** | `fbc_decoder.v:126-128` counts iterations but has no instruction buffer/PC to replay loop body |
| Pattern Converter `.fbc` output | ✅ **FIXED** | `gen_fbc.c` exists and integrated March 2026 (see Pattern Converter section) |
| Firmware update not wired | **ACTIVE** | Protocol layer has BEGIN/CHUNK/COMMIT, but `main.rs` doesn't process `pending_fw_*` requests |

### ✅ Recently Fixed

| Bug | Status | Details |
|-----|--------|---------|
| All responses broadcast | **FIXED** | `main.rs` now uses `last_sender_mac` for unicast responses to all commands |
| VICOR GPIO enable commented out | **FIXED** | SLCR `configure_mio()` now configures MIO pins 0,8,37,38,39,47 as GPIO before use |
| Error BRAM readback via protocol | **FIXED** | New ERROR_LOG_REQ/RSP commands read actual error BRAM data (pattern, vector, cycle) |
| DMA not integrated | **FIXED** | `fbc_dma.v` instantiated in `system_top.v` at lines 834-884 |
| FastPins wire-order swap | **FIXED** | GUI `parse_fast_pins()` now reads `(din, dout, oen)` |
| VICOR status always timeouts | **FIXED** | Length 48→30, parser 8B→5B per core |
| Host CLI broken | **FIXED** | FbcClient now wraps `FbcRawSocket` with correct 8-byte header |
| Rail data dropped | **FIXED** | `BoardStatus` has `rail_voltage_mv`/`rail_current_ma`, `parse_status()` reads all 47B |
| Error BRAMs unconnected | **FIXED** | 3× `error_bram.v` instantiated, AXI read at 0x4009_0000 |
| fast_error dropped | **FIXED** | Wired through `fbc_top` → `axi_fbc_ctrl` at 0x2C |

---

## Hardware Status (March 2026)

| Component | Status | Notes |
|-----------|--------|-------|
| **PL (FPGA)** | ✅ **PROGRAMMED** | Bitstream loaded via JTAG, all AXI peripherals accessible |
| **PS (ARM Firmware)** | ✅ **RUNNING** | First Light achieved March 2026 — CPU @ 667MHz, DDR @ 533MHz, ANNOUNCE packet sent |
| **VICOR GPIO** | ✅ FIXED | SLCR MIO mux configured in `main.rs:61-78` |
| **Error BRAM** | ✅ WIRED | 3× BRAMs at 0x4009_0000, protocol handler added |
| **DMA** | ✅ WIRED | `fbc_dma.v` instantiated, used by `FbcStreamer` |

**Next Hardware Steps:**
1. ✅ Apply 12V to TP16 (or backplane connector) — DONE
2. ✅ Load firmware via JTAG — DONE (First Light March 2026)
3. Test AXI register access (read 0x4004_001C → should return 0x00010000)
4. Run simple vector, verify GPIO toggling
5. Test firmware update pipeline (BEGIN/CHUNK/COMMIT) on real board

### 🟢 Low Priority / Known Limitations

| Issue | Status | Details |
|-------|--------|---------|
| 4 opcodes unimplemented | **KNOWN** | SYNC, IMM32, IMM128, PATTERN_SEQ → S_ERROR |
| Host CLI limited | **KNOWN** | Only `discover` and `ping` commands exposed |
| Phase clocks hardwired | **KNOWN** | `clk_gen.v` CLKOUT5/6 fixed at 50MHz@90/180 |
| FreqCounter never used | **KNOWN** | Implemented but firmware never reads it |
| PCAP module unused | **KNOWN** | `hal/pcap.rs` (358 lines) not called |
| Firmware update untested | **KNOWN** | BEGIN/CHUNK/COMMIT pipeline exists but not tested on hardware |

---

## Development Conventions

### Code Style

**Rust (firmware, gui, host):**
- `snake_case` for functions/variables, `PascalCase` for types
- Explicit `#[repr(C, packed)]` for protocol structs
- Big-endian byte order for network payloads
- No `std` in firmware (`#![no_std]`)
- Panic handler: `panic_halt`

**Verilog:**
- `` `define CONSTANTS `` in `fbc_pkg.vh`
- Module names: `snake_case` (e.g., `axi_fbc_ctrl`)
- Port names: `snake_case` with direction suffix (`_i`, `_o`, `_io`)
- Always use parameterized widths from `fbc_pkg.vh`

### Testing Practices

**Firmware:**
- No unit tests (bare-metal, hardware-dependent)
- Manual testing via GUI commands
- LED blink codes for error states (see `main.rs:hang_with_blink()`)

**Verilog:**
- Testbenches in `tb/` directory
- Self-checking testbenches preferred
- VCD output for waveform debugging

**GUI:**
- React components with TypeScript
- Tauri commands tested via GUI interaction
- No automated test suite present

### Commit Conventions

Based on git history analysis:
- Descriptive commit messages (50-char summary + body)
- Reference files changed in body
- Bug fixes reference specific issues

---

## Hardware Reference

### Pin Banks

| Bank | Pins | Description |
|------|------|-------------|
| Bank 13 | 0-47 | 48 pins (BIM) |
| Bank 33 | 48-95 | 48 pins (BIM) |
| Bank 34 | 96-127 | 32 pins (BIM) |
| Bank 35 | 128-159 | 32 direct FPGA pins (fast) |

### Special Pin Assignments

| Pin | Function |
|-----|----------|
| 128 | Scope trigger output |
| 129 | Error strobe output |
| 130 | LVDS sync N |
| 131 | LVDS sync P |
| 136 | SYSCLK0_N (clock input) |
| 137 | SYSCLK0_P (clock input) |

### Power Rails

| Rail | Voltage | Source |
|------|---------|--------|
| VCCINT | ~1.0V | XADC monitored |
| VCCAUX | ~1.8V | XADC monitored |
| VICOR cores (×6) | Programmable | VICOR DAC control |
| LCPS channels | Programmable | PMBus |

---

## Key Terminology

| Term | Meaning |
|------|---------|
| BIM | Board Interface Module — EEPROM on interposer identifies board type |
| DUT | Device Under Test (the chip being burned in) |
| VICOR | 6 high-current core power supplies |
| LCPS | Low Current Power Supply (PMBus-controlled) |
| Fast Pins | gpio[128:159] — direct FPGA I/O, 1-cycle latency |
| BIM Pins | gpio[0:127] — through interposer, 2-cycle latency |
| ONETWO | Methodology: decompose to invariants (ONE), then build (TWO) |
| PS | Processing System (ARM Cortex-A9 cores) |
| PL | Programmable Logic (FPGA fabric) |

---

## Files to Read First

When starting work on this project, read these files in order:

1. **`CLAUDE.md`** — Current project state, verified bugs, what's broken/fixed
2. **`rtl/fbc_pkg.vh`** — FBC instruction set, pin types, timing constants
3. **`firmware/src/fbc_protocol.rs`** — All 27 commands, payload structures
4. **`firmware/src/main.rs`** — Boot sequence, main loop, command dispatch
5. **`rtl/system_top.v`** — Top-level integration, AXI addresses
6. **`docs/register_map.md`** — Complete AXI register map
7. **`gui/src-tauri/src/fbc.rs`** — GUI protocol client
8. **`gui/src-tauri/src/state.rs`** — GUI state machine

---

## Common Tasks

### Add a new FBC command

1. Add command constant to `firmware/src/fbc_protocol.rs` (appropriate subsystem module)
2. Add payload struct if needed (#[repr(C, packed)], big-endian)
3. Implement handler in `firmware/src/main.rs` command dispatch loop
4. Add GUI command in `gui/src-tauri/src/fbc.rs` and `state.rs`
5. Update `CLAUDE.md` command table

### Add a new AXI peripheral

1. Create RTL module with AXI-Lite interface
2. Instantiate in `rtl/system_top.v` with unique base address
3. Add register offsets to `firmware/src/regs.rs`
4. Create Rust wrapper struct with register accessors
5. Update `docs/register_map.md`

### Debug firmware issues

1. Check LED blink code (see `main.rs:hang_with_blink()`)
2. Enable `debug-uart` feature for serial output
3. Use GUI terminal command for live console access
4. Check XADC voltages via GUI analog read

### Debug RTL issues

1. Run testbench: `iverilog -o sim tb/<module>_tb.v rtl/*.v`
2. View VCD: `gtkwave sim.vcd`
3. Check `fbc_pkg.vh` for parameter values
4. Verify AXI addresses match `system_top.v` instantiation

---

## External Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| `volatile-register` | 0.2 | Memory-mapped register access |
| `panic-halt` | 0.2 | Panic handler |
| `pnet` | 0.35 | Raw Ethernet (GUI, host) |
| `tokio` | 1 | Async runtime |
| `tauri` | 2 | GUI framework |
| `react` | 18.2 | Frontend UI |
| `three.js` | 0.159 | 3D visualization |

---

## License

**Proprietary** — Isaac Nudeton / ISE Labs

---

## XYzt-MCP Analysis (March 2026)

### Structural Correlation Matrix

5-key file correlation analysis (xyzt_io matrix):

| File | fbc_pkg.vh | fbc_protocol.rs | main.rs | fbc.rs | system_top.v |
|------|------------|-----------------|---------|--------|--------------|
| fbc_pkg.vh | - | 67% | 67% | 68% | 69% |
| fbc_protocol.rs | 67% | - | 77% | 80% | 74% |
| main.rs | 67% | 77% | - | 75% | 73% |
| fbc.rs | 68% | 80% | 75% | - | 71% |
| system_top.v | 69% | 74% | 73% | 71% | - |

**Key Insight:** `[fbc_protocol.rs, main.rs, fbc.rs]` form a tight cluster (>75% correlation) - these are the core protocol implementation files that change together.

### Dependency Hubs (xyzt_map)

Top files by dependent count:
1. `gui/src/store.ts` - 15 dependents (TypeScript state management)
2. `rtl/fbc_pkg.vh` - 13 dependents (Verilog header, defines)
3. `reference/kzhang_v2_2016/axi_slave.vh` - 5 dependents
4. `reference/kzhang_v2_2016/vector.vh` - 4 dependents

### Project Stats

- **169 files** tracked in dependency map
- **53,019 lines** of code
- **66 dependencies** mapped
- **Languages:** Rust (49), JavaScript (38), Verilog (47), Markdown (21), TypeScript (7)

### File Fingerprints (xyzt_io)

| File | Size | Entropy | Domain |
|------|------|---------|--------|
| fbc_pkg.vh | 6,079B | 4.98 bits/B | C source |
| fbc_protocol.rs | 56,309B | 4.99 bits/B | Python (misclassified - Rust) |
| main.rs | 19,526B | 4.68 bits/B | Markdown (misclassified - Rust) |
| fbc.rs | 18,320B | 4.98 bits/B | Python (misclassified - Rust) |
| system_top.v | 46,253B | 4.69 bits/B | Verilog |

---

## Last Updated

March 12, 2026
