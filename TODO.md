# FBC Semiconductor System - Development Roadmap

## Goal
Modernize the 2016 Sonoma/kzhang_v2 burn-in test system:
- Replace Linux with bare-metal Rust firmware (eliminate boot overhead)
- Build custom FPGA toolchain (eliminate Vivado dependency)
- Optimize RTL for efficiency
- Auto-configure hardware (PMBus device discovery)
- Custom GUI to replace Everest software
- Learn every layer from transistors to TCP packets

---

## Current Progress Summary

| Component | Status | Notes |
|-----------|--------|-------|
| RTL Core | 90% | 160-pin I/O (128 BIM + 32 fast), clock gen, ARM interface |
| Testbenches | 60% | Decoder, io_cell, io_bank, clk_gen, top, integration, error_inject |
| Firmware | 85% | HAL + PMBus + FBC + Thermal + I2C recovery complete |
| Toolchain | 90% | ONETWO bitstream working, self-contained (no ext deps) |
| **Host CLI** | **95%** | Full multi-board CLI with monitor, batch, JSON output |
| **GUI** | **80%** | All panels complete, needs backend integration |
| Documentation | 85% | Pin mapping + reference + bitstream + HAL docs |

### Recent Updates (Feb 2026)
- **Host CLI Complete**: Full-featured CLI with multi-board support
  - `run all --vectors test.fbc --wait` for batch execution
  - `monitor` for live status dashboard
  - `batch` for scripted operations
  - `--json` output for automation
- **I2C Error Recovery**: Bus recovery and retry with exponential backoff
- **Integration Testbenches**: fbc_integration_tb.v (1000 vectors, error injection)
- **Error Injection Testbench**: error_inject_tb.v (error counter verification)
- **GUI Export**: ExportDialog with CSV/JSON/STDF export support
- **GUI 80% Complete**: All major panels implemented (Tauri + React + Three.js)
  - 3D rack visualization with click-to-select
  - Sidebar navigation with 8 views
  - BoardDetail, AnalogMonitor, PowerControl, EEPROM, VectorEngine, DeviceConfig panels
  - Professional dark theme with CSS variables
- **fbc-config Tool**: Sonoma .bim/.map/.tim to binary converter (complete)
- **Pin Category Filtering**: GPIO vs power/ADC pins properly separated

### Previous Updates (Jan 2026)
- **ONETWO Bitstream**: Removed Project X-Ray, derived bit positions from reference analysis
- **Self-Contained**: Toolchain has zero external database dependencies
- **Verified Output**: 10,390 non-zero bits, correct word distribution (32-37 LUTs, 50 routing)
- **160-Pin Support**: RTL now supports 128 BIM + 32 fast pins
- **Constraints**: Real pin assignments from reference (gpio_old_board.xdc)
- **io_cell.v**: Added FAST_MODE parameter (1-stage vs 2-stage pipeline)
- **io_bank.v**: Split architecture for BIM and fast pins
- **io_config.v**: Expanded to 160-pin configuration registers
- **docs/PIN_MAPPING.md**: Complete pin documentation

---

## ONETWO → FBC Pipeline

**ONETWO** = Smart detection/learning system for parsing any format
- Parses Verilog, bitstreams, protocols, any structured data
- Finds invariants (what's constant)
- Finds variables (what changes)
- Extracts patterns for FBC encoding

**FBC** = Compaction method for all system communication
- Vectors: DONE (fbc.rs)
- PMBus commands: PLANNED
- Temperature readings: PLANNED
- GPIO/Config: PLANNED

Goal: ALL board communication goes through FBC for maximum compaction

---

## Phase 1: Foundation & Understanding
*Learn the existing system before changing it*

### 1.1 Study Reference Design (DONE)
- [x] Read through `reference/kzhang_v2_2016/top.v`
- [x] Read through `reference/kzhang_v2_2016/vector.vh`
- [x] Read through `reference/kzhang_v2_2016/axi_slave.v`
- [x] Read through `reference/kzhang_v2_2016/io_table.v`
- [x] Document data flow: ARM → AXI → Vector Engine → GPIO → DUT
- [x] Document timing: How vec_clk, phases, and strobes work

### 1.2 Study Existing Firmware (DONE - Jan 2026)
- [x] Analyze FW_v4.6d/FW_v4.8C scripts and ELF binaries
- [x] Document `init.sh` boot sequence
- [x] Document `ReadAnalog` sampling loop (AWK)
- [x] Document `RunVectors` test execution (AWK)
- [x] Document PMBus/Pico/Lynx communication
- [x] Document XADC usage and formulas
- [x] Create `reference/FIRMWARE_REFERENCE.md`
- [x] Create `reference/ZYNQ_REGISTER_MAP.md`

### 1.3 Study Zynq 7020 Architecture (DONE)
- [x] Review UG585 Zynq TRM
- [x] Document peripheral base addresses
- [x] Document I2C/SPI/GPIO register maps
- [x] Document SLCR unlock sequences
- [x] Understand PS-PL interface (AXI interconnect)

---

## Phase 2: RTL Development
*Build the FPGA logic piece by piece*

### 2.1 Core Infrastructure (DONE)
- [x] `fbc_pkg.vh` - Package defines
- [x] `fbc_decoder.v` - Instruction decoder
- [x] `fbc_top.v` - Top wrapper
- [x] `fbc_top_v2.v` - Enhanced with ARM interface, XADC, thermal

### 2.2 Vector Engine (DONE)
- [x] `vector_engine.v` - Basic implementation
- [x] Repeat counter logic
- [x] Timing control (setup/hold via vec_clk_cnt)
- [x] Pause/resume capability

### 2.3 I/O Subsystem (DONE - Enhanced Jan 2026)
- [x] `io_config.v` - Pin type configuration for all 160 pins (AXI-Lite)
- [x] `io_cell.v` - Single I/O cell with FAST_MODE parameter
- [x] `io_bank.v` - 160 I/O cells (128 BIM + 32 fast)
- [x] All pin types: BIDI, INPUT, OUTPUT, OPEN_C, PULSE, NPULSE, VEC_CLK, VEC_CLK_EN
- [x] BIM pins: 2-stage pipeline (timing closure at 200MHz)
- [x] Fast pins: 1-stage pipeline (single cycle latency)
- [x] Constraints: All 160 pins mapped from reference XDC
- [x] Documentation: `docs/PIN_MAPPING.md`

### 2.4 Clock Generation (DONE)
- [x] `clk_gen.v` - MMCME2_ADV wrapper
- [x] vec_clk (0°), vec_clk_90 (90°), vec_clk_180 (180°)
- [x] Clock enable gating (BUFGCE)
- [ ] Dynamic frequency selection (DRP interface - future)

### 2.5 ARM/Peripheral Interfaces (DONE)
- [x] `arm_interface.v` - AXI-Lite bridge for PS communication
- [x] `xadc_interface.v` - FPGA temperature/voltage monitoring
- [x] `pico_interface.v` - SPI controller for Pico (power)
- [x] `lynx_interface.v` - SPI controller for Lynx (thermal)
- [x] `pid_controller.v` - Thermal PID loop
- [ ] Add DMA controller for high-speed vector streaming

### 2.6 Error Handling (PARTIAL)
- [x] `error_counter.v` - Basic counting
- [ ] Add error logging (store first N failures)
- [ ] Add error location (which pin, which vector)
- [ ] Add error masking (ignore specific pins)

---

## Phase 3: Simulation & Verification

### 3.1 Unit Tests (DONE)
- [x] `tb/fbc_decoder_tb.v`
- [x] `tb/io_cell_tb.v`
- [x] `tb/io_bank_tb.v`
- [x] `tb/clk_gen_tb.v`
- [x] `tb/xilinx_sim_stubs.v` - Behavioral models

### 3.2 Integration Tests (PARTIAL)
- [x] `tb/fbc_top_tb.v` - Updated for new architecture
- [ ] Test error detection scenarios
- [ ] Test all pin types in integration

### 3.3 Simulation Infrastructure
- [ ] Set up Icarus Verilog flow
- [ ] Set up waveform viewing (GTKWave)
- [ ] Create test vector files

---

## Phase 4: Firmware Development (PRIORITY)
*Bare-metal Rust on ARM Cortex-A9*

### 4.1 Build Infrastructure (DONE)
- [x] `firmware/Cargo.toml` - Rust project config
- [x] `firmware/.cargo/config.toml` - armv7a-none-eabi target
- [x] `firmware/link.ld` - Linker script for Zynq OCM (192KB)
- [x] `firmware/build.rs` - Build script
- [x] `firmware/build.bat` - Windows build script
- [x] Stable Rust (no nightly required)

### 4.2 Hardware Abstraction Layer (DONE)
All Zynq PS peripherals implemented using ONETWO pattern:

- [x] **I2C Driver** (`hal/i2c.rs`)
  - I2C0/I2C1 support (0xE000_4000, 0xE000_5000)
  - init(), write(), read(), write_read() (repeated start)
  - scan() for device discovery
  - Error handling (Nack, ArbitrationLost, Timeout)

- [x] **SPI Driver** (`hal/spi.rs`)
  - SPI0/SPI1 support (0xE000_6000, 0xE000_7000)
  - All SPI modes (CPOL/CPHA), manual CS control
  - transfer_byte(), transfer(), read(), write()
  - ADC/DAC helper functions

- [x] **GPIO Driver** (`hal/gpio.rs`)
  - GPIO bank 0/1 (MIO pins 0-53)
  - set_direction(), write_pin(), read_pin(), toggle()
  - Sonoma helpers: set_adc_mux(), set_core_enable()
  - Atomic masked writes for pin manipulation

- [x] **XADC Driver** (`hal/xadc.rs`)
  - Temperature reading (raw, Celsius, milliCelsius)
  - Supply voltages: VCCINT, VCCAUX, VCCBRAM
  - Auxiliary inputs (0-7)
  - Alarm thresholds and over-temperature detection
  - get_system_status() for full readout

- [x] **UART Driver** (`hal/uart.rs`)
  - UART0/UART1 support (0xE000_0000, 0xE000_1000)
  - Configurable baud, parity, stop bits
  - Blocking and non-blocking TX/RX
  - fmt::Write impl for write! macro support

- [x] **PCAP Driver** (`hal/pcap.rs`)
  - FPGA programming via DEVCFG (0xF800_7000)
  - reset_fpga(), program(), readback()
  - DMA-based bitstream transfer
  - DONE/INIT signal monitoring

- [x] **SLCR Driver** (`hal/slcr.rs`)
  - Unlock/lock sequence (0xDF0D/0x767B)
  - Peripheral clock enables (I2C, SPI, UART, GPIO, GEM)
  - FPGA reset control
  - MIO pin configuration
  - FCLK divisor configuration

### 4.2.1 HAL Optimization (DONE)

- [x] **ONETWO Thermal Controller** (DONE - Jan 2026)
  - Created `hal/thermal.rs`
  - Crystallization-based control (settling rate = e-2, no arbitrary tuning)
  - Pattern-aware feedforward from vector toggle rate analysis
  - `estimate_power()` scans vectors ONCE, predicts power level
  - High toggle rate → pre-cool before temp rises
  - 7 iterations to lock (structurally forced)
  - No PID - constants derived from ONETWO framework

- [ ] **Timer-Based Delays**
  - Replace busy-wait `delay_us()` with TTC (Triple Timer Counter)
  - Accurate timing for protocols
  - Lower power consumption

- [ ] **Interrupt-Driven I/O**
  - I2C transfer complete interrupts
  - UART RX buffer interrupts
  - GIC (Generic Interrupt Controller) setup

- [ ] **DMA Transfers**
  - UART TX DMA (ring buffer)
  - SPI bulk transfers
  - Zero-copy where possible

- [x] **Error Recovery** (DONE - Feb 2026)
  - [x] I2C bus recovery (`recover_bus()`)
  - [x] Retry with exponential backoff (`transfer_with_retry()`)
  - [x] `write_with_retry()`, `read_with_retry()`, `write_read_with_retry()`
  - [ ] Fault logging to memory (optional enhancement)

### 4.3 PMBus Device Management (PARTIAL)
PMBus protocol implemented, needs device discovery:

- [x] **PMBus Protocol** (`hal/pmbus.rs`)
  - All standard command codes (0x00-0x9E)
  - read_byte/word/block(), write_byte/word()
  - PEC (CRC-8) support for data integrity
  - Linear11/Linear16 format conversion
  - Voltage: read_vout_mv(), set_vout_mv()
  - Current: read_iout_ma(), read_iin_ma()
  - Power: read_pout_mw(), read_pin_mw()
  - Temperature: read_temperature_1_mc()
  - Status: read_status(), has_fault(), is_output_on()
  - Control: enable_output(), disable_output(), clear_faults()
  - ID: read_revision(), read_mfr_id(), read_model()
  - scan_pmbus_devices() helper function

- [ ] **Device Discovery**
  - Read manufacturer ID on scan
  - Build device table with type detection
  - Support: Pico, Lynx, MPS, Infineon, Vicor

- [ ] **Virtual Addressing**
  - Logical address → physical I2C address mapping
  - Persist mapping in config
  - Handle hot-swap detection

### 4.4 Thermal Control (DONE)
ONETWO crystallization replaces PID - no tuning needed:

- [x] **XADC Temperature** (via hal/xadc.rs)
  - On-chip temperature with millidegree precision
  - Over-temperature detection and alarms
  - System status aggregation

- [x] **ONETWO Thermal Controller** (via hal/thermal.rs)
  - Crystallization dynamics: settling rate = (e-2)
  - 7 iterations to lock (structurally forced)
  - Pattern-aware feedforward from vector toggle rate
  - No PID tuning needed - works on all 500+ boards

- [ ] **External Temperature Sensors** (optional enhancement)
  - Case temp (NTC thermistor): formula 3
  - Diode temp (linear): formula 2
  - ADC channel routing via MUX

- [ ] **Limit Checking** (optional enhancement)
  - Upper/lower limits per channel
  - Shutdown on violation
  - Configurable per test step

### 4.5 Vector Execution
- [ ] Parse FBC program format
- [ ] Load vectors into DDR/BRAM
- [ ] Configure DMA transfers
- [ ] Start/stop/pause vector execution
- [ ] Error collection and reporting

### 4.6 Network Stack
- [ ] Initialize GigE MAC
- [ ] Use smoltcp for TCP/IP
- [ ] Command protocol (replaces Everest API)
- [ ] Status reporting (UDP broadcast)

### 4.7 Main Control Loop
- [ ] State machine: IDLE → INIT → RUNNING → COMPLETE
- [ ] Test step sequencing
- [ ] Power supply sequencing (PU_LIST equivalent)
- [ ] Watchdog timer
- [ ] Status LED control (RGB via PWM)

---

## Phase 5: FPGA Toolchain

### 5.1 Core Flow (DONE)
- [x] Verilog parser
- [x] Elaboration (hierarchical)
- [x] Synthesis (to LUTs/FFs)
- [x] Technology mapping
- [x] Placement (simulated annealing)
- [x] Routing (PathFinder)
- [x] Bitstream generation (structure correct)
- [x] Verilog simulator (`fbc-synth test`)

### 5.2 ONETWO Pattern Learning (DONE)
Derived bitstream format from Vivado analysis (no Project X-Ray needed):
- [x] `learn.rs` - Pattern learning module
- [x] `BitstreamAnalyzer` - Parse Vivado bitstreams
- [x] CLI commands: `analyze`, `patterns`, `diff`
- [x] Derived frame structure: 101 words, CLB0=0-49, routing=50, CLB1=51-100
- [x] Derived LUT word placement: CLB0 at 4-11, CLB1 at 54-61
- [x] Derived FF init location: word 50, bits 0-15
- [x] Verified against Vivado: word spacing=2, same word pairs used
- [x] Documentation: `docs/bitstream_format.md`

### 5.2.1 ONETWO Self-Derived Bitstream Format (DONE - Jan 24, 2026)
Bitstream format derived entirely from ONETWO analysis (no external dependencies):
- [x] Analyzed reference bitstream (`reference/kzhang_v2_2016/top.bit`)
- [x] Derived frame structure: 101 words, 79 columns, 36 minors/CLB
- [x] Derived LUT INIT bit positions: words 32-35 for LUT A, offset patterns for B/C/D
- [x] Derived routing word: word 50, bits 0-12 for interconnect muxes
- [x] Implemented `LUT_A_INIT[64]` constant array with exact (word, bit) mappings
- [x] Removed Project X-Ray dependency - toolchain is fully self-contained
- [x] Verified output: 10,390 non-zero bits, correct word distribution

**ONETWO-Derived Constants (in bitstream.rs):**
```rust
CLB_BASE_COLUMN = 4          // First CLB column
MINORS_PER_CLB = 36          // Minor frames per CLB
LUT_A_INIT[64]               // (word, bit) for each INIT bit
LUT_B_BIT_OFFSET = 16        // B is 16 bits above A
LUT_CD_WORD_OFFSET = 4       // C/D use words 36-39
ROUTING_WORD = 50            // Interconnect configuration
```

### 5.3 Optimization Engine (DONE)
Criticality-aware placement and routing for optimal bitstreams:
- [x] `optimize.rs` - Optimization engine with custom cost functions
- [x] Optimization profiles: timing, power, area, burn_in
- [x] CLI: `--optimize <profile> --critical-nets <net1,net2>`
- [x] Criticality-weighted placement (critical nets placed closer)
- [x] Criticality-weighted routing (critical nets win wire contention)
- [x] Integrated with place.rs (place_optimized) and route.rs (route_optimized)

### 5.4 Routing PIPs (DONE - Jan 2026)
Word 50 contains routing configuration, learned via ONETWO methodology:
- [x] Extract Word 50 patterns from reference bitstream (5,131 patterns from top.bit)
- [x] Analyze patterns by tile type and wire connections (column-based grouping)
- [x] Build PIP database from patterns (86 entries in pip_database.rs)
- [x] Create pip_analyzer.rs tool for pattern analysis and database generation
- [x] Implement 3-tier lookup in bitstream.rs encode_pip():
  - Exact match: (column, direction) → pattern
  - Fallback: nearby columns or Local direction
  - Default: most common patterns from ONETWO analysis
- [x] Generate test bitstream and validate patterns (36 frames, 100% match)
- [x] Generate full FBC design bitstream (3,488 frames, 92% database hit rate)
- [x] Validate all generated patterns exist in reference bitstream
- [x] Statistical validation: bit usage within 10% of reference
- [ ] Hardware testing on Zynq 7020 (pending board availability)
- [ ] Fine-tune PIP encodings with more Vivado samples (optional)

### 5.5 Remaining (OPTIONAL)
- [ ] CRC calculation for bitstream validation
- [ ] Partial reconfiguration support
- [ ] BRAM initialization

---

## Phase 6: Host Tools / GUI

### 6.1 CLI Tool (DONE - Feb 2026)
- [x] `host/` - Basic structure
- [x] Implement ping/status/load/run commands
- [x] Multi-board targeting ("all", comma-separated MACs)
- [x] `run` with `--wait` and `--timeout` flags
- [x] `monitor` command with live refresh
- [x] `batch` command for scripted operations
- [x] JSON output (`--json`) for scripting integration

### 6.2 GUI (80% Complete - Feb 2026)
Tauri + React + Three.js application replacing Everest software.

**Completed Components:**
- [x] `gui/` - Tauri project structure
- [x] `Sidebar.tsx` - Collapsible navigation (8 views)
- [x] `RackView.tsx` - 3D rack visualization (Three.js)
- [x] `BoardDetailPanel.tsx` - Stats, EEPROM info, test controls
- [x] `AnalogMonitorPanel.tsx` - 32ch ADC display (XADC + MAX11131)
- [x] `PowerControlPanel.tsx` - VICOR cores (6), PMBus rails, emergency stop
- [x] `EepromPanel.tsx` - Header, rails, DUT, calibration, hex viewer
- [x] `VectorEnginePanel.tsx` - Load/run/pause vectors, progress tracking
- [x] `DeviceConfigPanel.tsx` - Pin mapping, timing diagram
- [x] `StatusPanel.tsx` - Board list with status badges
- [x] `Terminal.tsx` - Command terminal
- [x] `Toolbar.tsx` - Connection management
- [x] `store.ts` - Zustand state management
- [x] CSS for all components (dark theme)

**Remaining:**
- [ ] Connect panels to Tauri backend commands
- [ ] Add missing Tauri commands (read_eeprom, set_vicor_voltage, etc.)
- [ ] Test plan editor
- [x] Data logging and export (ExportDialog + CSV/JSON/STDF - Feb 2026)
- [ ] Charts/historical data visualization

### 6.3 Vector Tools (DONE - Jan 2026)

**Status:** ✅ Complete - All components implemented and tested with real customer files

**Location:** `tools/fbc-vec/` (standalone Rust crate)

**Components:**
- [x] **AVC Parser** (`tools/fbc-vec/src/avc.rs`)
  - Parse 93K AVC format
  - Extract pin states, timing, sequencing
  - Tested with real AVC files (145-710x compression)

- [x] **STIL Parser** (`tools/fbc-vec/src/stil.rs`)
  - Parse IEEE 1450 STIL format
  - Extract WaveformTable, SignalGroups, PatternBurst
  - Tested with real STIL files (936 signals found)

- [x] **PAT Parser** (`tools/fbc-vec/src/pat.rs`)
  - Parse XPS/ISE Pattern Editor format
  - Support H[num] hold commands (suffix parsing)
  - Support A...B, C...D loop expansion
  - Tested with real PAT files (65K entries → 2.6M vectors)

- [x] **APS Parser** (`tools/fbc-vec/src/aps.rs`)
  - Parse Automatic Test Pattern format
  - Tested with real APS files

- [x] **FVEC Parser** (`tools/fbc-vec/src/fvec.rs`)
  - Parse FBC text format
  - Simple syntax for test patterns

- [x] **FBC Compiler** (`tools/fbc-vec/src/compiler.rs`)
  - Convert parsed vectors to FBC binary format
  - Apply compression (pattern detection, run-length encoding)
  - 145-95,952x compression ratios achieved

- [x] **FBC Format** (`tools/fbc-vec/src/format.rs`)
  - FbcVectorFormat struct (160-pin, 20 bytes per vector)
  - Pin type configuration
  - Compression metadata
  - CRC32 validation

- [x] **Multi-format Output**
  - FBC binary (for FBC FPGA system)
  - Sonoma .hex (40-byte format, tested)
  - PAT (XPS format, tested)
  - HX/Shasta (planned for future)

**Tools Built:**
- [x] `fbc-vec compile` - Convert any format → FBC/Sonoma/PAT
- [x] `fbc-vec info` - Inspect FBC vector files
- [x] `fbc-vec validate` - Validate CRC32
- [x] `fbc-vec disasm` - Disassemble to text

**Testing:**
- [x] Tested with real STIL files (936 signals)
- [x] Tested with real AVC files (145-710x compression)
- [x] Tested with real PAT files (H[num] hold commands, 95,952x compression)
- [x] Tested with real APS files
- [x] Validated against Sonoma .hex format (correct 40-byte output)
- [x] Validated PAT output format (generates valid XPS patterns)
- [x] 160-channel signal mapping verified

---

## Phase 7: Integration & Testing

### 7.1 Hardware Testing
- [ ] Load bitstream via JTAG
- [ ] Test AXI register access
- [ ] Test GPIO toggling
- [ ] Test vector execution
- [ ] Test PMBus communication

### 7.2 System Testing
- [ ] Full flow: load → run → collect results
- [ ] Test with real DUT
- [ ] Stress testing

### 7.3 Multi-Board Testing
- [ ] Control 500 boards simultaneously
- [ ] Parallel operations
- [ ] Performance benchmarking

---

## Reference Documents

Located in `reference/`:

| File | Description |
|------|-------------|
| `FIRMWARE_REFERENCE.md` | Analysis of Linux firmware (FW_v4.x) |
| `ZYNQ_REGISTER_MAP.md` | Peripheral addresses and register offsets |
| `kzhang_v2_2016/` | Original 2016 RTL design |
| `Everest_3.7.3.exe` | Current Everest software |
| `SONOMA_FW_v4.8C_01-21-2026.ifw` | Latest firmware package |

Source documents (OneDrive, read-only):
- `Sonoma Firmware API Specifications.doc` - ELF binary API
- `Sonoma TOOLS Specifications.doc` - Vector translation tools
- `ug585-Zynq-7000-TRM.pdf` - Zynq Technical Reference Manual

---

## Optimization Targets (vs Linux firmware)

| Metric | Linux (Sonoma) | Bare-Metal (FBC) | Status |
|--------|----------------|------------------|--------|
| Boot time | 10-30s | <100ms | Ready |
| Vector load | NFS file I/O | Direct DMA | TODO |
| ADC sampling | 500ms/loop | <1ms/loop | DONE (HAL) |
| PMBus access | ELF spawn (50-100ms) | Direct I2C (~500μs) | DONE (HAL) |
| Temp control | Bang-bang (Lynx) | ONETWO crystallization | DONE |
| IPC overhead | flock + files | None (single binary) | DONE |
| Network | NFS mount | Direct FBC protocol | TODO |

---

## Next Steps (Recommended Order)

1. ~~**Toolchain Training**: Generate Vivado bitstreams, run `fbc-synth learn`~~ DONE
2. ~~**Firmware HAL**: Implement I2C/SPI/GPIO/XADC/UART/PCAP drivers~~ DONE
3. ~~**PMBus Protocol**: Full PMBus command set with Linear11/16~~ DONE
4. ~~**Thermal Controller**: ONETWO crystallization + pattern feedforward~~ DONE
5. ~~**GUI Panels**: All major panels implemented~~ DONE (80%)
6. **GUI Backend**: Connect Tauri commands to firmware API
7. **FBC Expansion**: Add PMBus/Temp/GPIO opcodes to fbc.rs
8. **PMBus Discovery**: Scan and identify Pico/Lynx/MPS devices
9. **Network Stack**: Basic TCP server for commands
10. **Vector Loading**: DMA-based vector transfer
11. **Hardware Testing**: Validate on actual Zynq 7020 board

---

## Phase 8: Unified Toolchain (FUTURE)

*After Rust firmware is complete and optimized, rebuild as fully owned system.*

### 8.1 Vision
One toolchain that generates EVERYTHING from a single source of truth:
- FPGA bitstream (hardware)
- Firmware binary (software)
- GUI protocol handlers (host)

All three share the same internal model - same registers, same FBC opcodes, same data structures. No mismatch possible by construction.

### 8.2 FBC as Universal Protocol
FBC encodes ALL system communication:
- Vectors (done in current fbc.rs)
- PMBus commands
- Temperature readings
- GPIO/Config
- Error reports with pin/vector/cycle location

Compact 32-bit instruction format:
```
[8-bit opcode][8-bit flags][16-bit operand]
- Delta encoding for bandwidth reduction
- SET_DELTA: 16-bit delta from last value
- SET_FULL: followed by full payload (rare)
- FAIL_REPORT: pin_mask + vector_id + expected/actual
```

### 8.3 Custom Firmware Compiler
Replace Rust/LLVM with our own:
- Parse our DSL
- Generate ARM Cortex-A9 instructions directly
- ONETWO learns ARM encoding (like it learned bitstream format)
- Zero external dependencies

### 8.4 Custom Libraries
```
libs/
├── net/     # TCP/IP or custom protocol (no smoltcp)
├── fbc/     # Encode/decode (shared with FPGA)
├── arm/     # Instruction encoding
├── math/    # Fixed-point, PID
└── core/    # Minimal runtime
```

### 8.5 Integration
```
Toolchain knows:
├── Register addresses (it placed them)
├── Data widths (it sized them)
├── Timing requirements (it routed them)
└── FBC opcodes (it defined them)

Therefore:
├── Firmware generation = emit ARM for each register
├── GUI generation = emit handlers for each opcode
└── Zero mismatch by construction
```

### 8.6 Error Philosophy
- NO auto-retry (masks hardware failures)
- Accurate failure capture: which pin, which vector, which cycle
- ONETWO analyzes failure PATTERNS (not just input patterns)
- Controller reports its own issues
- GUI has BIM/DUT map, flags chip vs BIM issues

---

## Quick Reference: Toolchain Commands

```bash
# Full build flow
fbc-synth build design.v -o design.bit

# Build with optimization profile
fbc-synth build design.v -o design.bit --optimize timing --critical-nets "vec_clk,data_out"
fbc-synth build design.v -o design.bit --optimize burn_in   # Pre-configured for FBC system
fbc-synth build design.v -o design.bit --optimize power     # Low power mode
fbc-synth build design.v -o design.bit --optimize area      # Minimum area

# Run testbench simulation
fbc-synth test tb/my_test.v rtl/design.v

# Learn bit patterns from Vivado bitstreams
fbc-synth learn bit1.bit bit2.bit bit3.bit \
    --labels "x=0,y=0" "x=1,y=0" "x=0,y=1" \
    -o learned_xc7z020.json

# Analyze/diff bitstreams
fbc-synth analyze reference.bit --frames
fbc-synth diff ours.bit vivado.bit --verbose
```
