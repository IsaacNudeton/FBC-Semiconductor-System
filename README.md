# FBC Semiconductor System

Autonomous burn-in test system for semiconductor chips. Bare-metal Zynq 7020 firmware, raw Ethernet protocol, compressed vector format. Boards run 500+ hours without a PC.

## What It Does

Plugs into existing burn-in hardware (~44 boards per system). Each board has a Zynq 7020 FPGA + ARM that drives 160 GPIO pins into a Device Under Test, monitors power/temperature/errors, and decides pass/fail. The PC uploads vectors and a test plan, then walks away.

## Architecture

```
PC (CLI / GUI)                    Zynq 7020 Board
  |                                  |
  |  Raw Ethernet 0x88B5             |
  |  (no IP, no TCP, <1ms RTT)      |
  |--------------------------------->|
  |                                  |  ARM Cortex-A9 (bare-metal Rust)
  |  79 protocol commands            |    - 13 subsystems
  |  across 13 subsystems            |    - SD card pattern storage
  |                                  |    - DDR double-buffer
  |                                  |    - Autonomous test plan executor
  |                                  |
  |                                  |  FPGA Fabric (Verilog)
  |                                  |    - FBC decoder (7 opcodes)
  |                                  |    - 160-pin vector engine
  |                                  |    - Error detection BRAMs
  |                                  |    - DMA from DDR
  |                                  |
  |                                  |-----> 160 GPIO pins --> DUT
```

## Key Numbers

| Metric | Value |
|--------|-------|
| Protocol commands | 79 across 13 subsystems |
| FbcClient methods | 47 (Rust host library) |
| Firmware tests | 16 passing |
| Host tests | 27 passing |
| Vector compression | 4.8x - 710x (.hex -> .fbc) |
| Boot time | <1 second (bare-metal, no OS) |
| Protocol latency | <1ms round-trip (raw Ethernet) |
| Max patterns per test | 256 on SD card |
| Max test steps | 96 per plan |
| DDR vector buffer | 508MB (dual 252/256MB regions) |
| Thermal controller | Lean-verified headroom kernel (0 tuned constants) |

## Components

### Firmware (`firmware/`)
Bare-metal Rust for ARM Cortex-A9. No OS, no Linux, no SSH.

- **Protocol handler:** 79 commands — power, vectors, analog, thermal, EEPROM, test plan
- **Test plan executor:** Autonomous burn-in with per-step temp/clock/fail-action
- **SD pattern storage:** 256 patterns, DDR double-buffer with non-blocking chunked loading
- **Thermal control:** Headroom kernel with Lean-verified stability (MetabolicAge_v3.lean)
- **Safety monitor:** Over/under voltage, over-temperature, emergency stop — runs every loop iteration
- **HAL:** 17 drivers (VICOR, PMBus, MAX11131 ADC, BU2505 DAC, EEPROM, SD, SPI, I2C, UART, XADC, GPIO, DMA, DNA, GIC, SLCR, Ethernet PHY, thermal)

### Host CLI (`host/`)
Rust library + CLI for controlling boards from a PC.

```bash
fbc-cli fbc discover                           # Find boards on network
fbc-cli fbc status all                         # All board telemetry
fbc-cli fbc slot-upload <MAC> 0 pattern.fbc    # Upload pattern to SD
fbc-cli fbc set-plan <MAC> plan.json           # Load test plan
fbc-cli fbc run-plan <MAC>                     # Start autonomous execution
fbc-cli fbc plan-status <MAC>                  # Check progress
fbc-cli fbc record <MAC> -o test.fbd           # Record binary datalog
```

### Pattern Converter (`gui/src-tauri/c-engine/pc/`)
Zero-dependency C11 library. Converts customer vector formats to FBC compressed binary.

```
ATP/STIL/AVC + PIN_MAP --> .hex (legacy) + .seq + .fbc (compressed)
CSV/Excel/PDF --> Pin Table --> Device Config --> 8 output files
```

### RTL (`rtl/`)
16 Verilog modules. Verified on hardware, timing closure WNS=+0.018ns.

- `fbc_decoder.v` — 7-opcode instruction set
- `vector_engine.v` — 160-pin drive + compare
- `fbc_dma.v` — AXI DMA from DDR
- `axi_device_dna.v` — Unique per-silicon MAC address

### Native GUI (`app/`)
wgpu immediate-mode renderer. 14 panels, 0 warnings.

- Dashboard, Device Profiling, Engineering, Datalogs tabs
- Board tree sidebar (System -> Shelf -> Tray -> Board)
- Transport layer dispatches to FBC (Ethernet) or legacy (SSH)

## .fbc Compressed Vector Format

```
HEADER (32 bytes): magic, version, pin_count, num_vectors, vec_clock_hz, CRC32
PIN_CONFIG (80 bytes): 160 pins x 4 bits
COMPRESSED DATA:
  OP_VECTOR_FULL   0x01  1+20B   (raw 160-bit vector)
  OP_VECTOR_SPARSE 0x02  1+1+NB  (delta from previous)
  OP_VECTOR_RUN    0x03  1+4B    (repeat count)
  OP_VECTOR_ZERO   0x04  1B      (all zeros)
  OP_VECTOR_ONES   0x05  1B      (all ones)
  OP_END           0x07  1B      (stream terminator)
THERMAL_PROFILE: power estimates per 1024 vectors
```

## Binary Datalog Format (.fbd)

Packet capture of all board telemetry during a test. ~4x denser than CSV, CRC-verified.

```
HEADER (32 bytes): magic, board_mac, test_start_epoch, plan_hash
BODY (repeating): [offset_ms:u32][raw FBC packet (8B header + payload)]
FOOTER (12 bytes): record_count, body_crc32, end_magic
```

## Building

```bash
# Firmware (bare-metal ARM)
cd firmware && cargo build --release --target armv7a-none-eabi

# Host CLI + tests
cd host && cargo build --release && cargo test

# Native GUI
cd app && cargo build --release

# FPGA bitstream (Vivado)
vivado -mode batch -source scripts/build_bitstream.tcl
```

## Hardware

- **Part:** XC7Z020-1CLG484C (Zynq 7020, 484-pin)
- **DDR3:** IS43TR16256A-125KBLI (1GB, 2x 512MB)
- **ADC:** MAX11131 (16-channel external) + XADC (on-die)
- **DAC:** BU2505FV (10-channel, controls VICOR + thermal)
- **Ethernet PHY:** 88E1512
- **JTAG:** FT232H via MPSSE

## License

MIT

Isaac Nudeton / ISE Labs
