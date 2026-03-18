# FBC Semiconductor System — Architecture

**Last Verified:** March 2026 (First Light)
**Status:** ✅ Complete (PL programmed, PS firmware running)

---

## System Overview

FBC (Force Burn-in Controller) is a modernized semiconductor burn-in test system that replaces the 2016 Linux-based Sonoma design with:

- **Bare-metal Rust firmware** — No OS, <1s boot time
- **Custom FPGA toolchain** — ONETWO-derived, no Vivado required
- **Raw Ethernet protocol** — EtherType 0x88B5, no TCP/IP overhead
- **Modern GUI** — Tauri + React + Three.js

**Scale:** ~44 Zynq 7020 boards per rack, each controlling 160 GPIO pins (128 BIM + 32 fast).

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        HOST PC                                  │
│  ┌────────────────┐                                             │
│  │  FBC GUI       │  (Tauri + React + Three.js)                │
│  │  (Raw Ethernet)│────────────────────┐                        │
│  └────────────────┘                    │                        │
└────────────────────────────────────────┼────────────────────────┘
                                         │ Raw Ethernet (0x88B5)
                                         │
┌────────────────────────────────────────┼────────────────────────┐
│             ZYNQ 7020 BOARD (×44/rack)                          │
│  ┌──────────────────────┐    ┌──────────────────────────────┐  │
│  │   ARM Cortex-A9 (PS) │    │      FPGA Fabric (PL)        │  │
│  │  ┌────────────────┐  │    │  ┌────────────────────────┐  │  │
│  │  │  firmware/     │  │◄──►│  │  rtl/                  │  │  │
│  │  │  (bare-metal   │  │AXI │  │  - fbc_decoder         │  │  │
│  │  │   Rust)        │  │    │  │  - vector_engine       │  │  │
│  │  │  - Ethernet    │  │    │  │  - error_counter       │  │  │
│  │  │  - FBC protocol│  │    │  │  - io_bank (160 pins)  │  │  │
│  │  │  - HAL drivers │  │    │  └────────────────────────┘  │  │
│  │  └────────────────┘  │    └──────────────┬───────────────┘  │
│  └──────────────────────┘                   │                  │
└──────────────────────────────────────────────┼──────────────────┘
                                               ▼
                                      ┌────────────────┐
                                      │ Chip Under Test│
                                      │     (DUT)      │
                                      └────────────────┘
```

---

## Data Flow

### 1. Pattern Conversion (Pre-Test)

```
ATP/STIL/AVC (customer patterns)
    │
    ▼
gui/src-tauri/c-engine/pc/ (C engine)
    │
    ├──▶ .hex + .seq ✅ (Legacy format, 40 bytes/vector)
    │
    └──▶ .fbc ❌ (MISSING — see docs/MIGRATION.md)
```

### 2. Test Execution

```
GUI (Tauri)
    │
    │ Raw Ethernet (0x88B5)
    │ Commands: UPLOAD_VECTORS, START, STATUS_REQ, etc.
    ▼
Firmware (ARM Cortex-A9)
    │
    │ AXI-Lite bus (GP0)
    │ Base addresses: 0x4004_0000 - 0x4009_0000
    ▼
FPGA Peripherals (PL)
    │
    ├── axi_fbc_ctrl (0x4004_0000) — FBC decoder control
    ├── io_config (0x4005_0000) — Pin type configuration
    ├── axi_vector_status (0x4006_0000) — Vector execution status
    ├── axi_freq_counter (0x4007_0000) — 8-channel frequency counter
    ├── clk_ctrl (0x4008_0000) — Clock control (freq_sel)
    └── error_bram (0x4009_0000) — Error logging (3× BRAMs)
    │
    ▼
io_bank (160 GPIO pins)
    │
    ├── gpio[0:127] — BIM pins (2-cycle latency)
    └── gpio[128:159] — Fast pins (1-cycle latency)
    │
    ▼
DUT (Device Under Test)
```

### 3. Error Capture

```
DUT mismatch detected
    │
    ▼
io_bank.v → fast_error[31:0]
    │
    ▼
axi_fbc_ctrl.v → error register (0x18, 0x2C)
    │
    ▼
error_bram.v (3× BRAMs)
    ├── Pattern BRAM (128-bit error mask)
    ├── Vector BRAM (vector number)
    └── Cycle BRAM (64-bit cycle count)
    │
    ▼
Firmware reads via ERROR_LOG_REQ/RSP protocol
    │
    ▼
GUI displays error details
```

---

## Component Status

### PL (FPGA Fabric)

| Component | Status | Notes |
|-----------|--------|-------|
| **Bitstream** | ✅ PROGRAMMED | Loaded via JTAG |
| **AXI Peripherals** | ✅ WIRED | All 6 peripherals instantiated |
| **DMA** | ✅ WIRED | `fbc_dma.v` at 0x4040_0000 |
| **Error BRAMs** | ✅ WIRED | 3× BRAMs at 0x4009_0000 |
| **Fast Error** | ✅ WIRED | Wired through `axi_fbc_ctrl` at 0x2C |

### PS (ARM Cortex-A9)

| Component | Status | Notes |
|-----------|--------|-------|
| **Firmware** | ✅ **RUNNING** | First Light March 2026 — CPU @ 667MHz, DDR @ 533MHz |
| **Ethernet** | ✅ WORKING | GEM0 initialized, ANNOUNCE packet sent |
| **VICOR GPIO** | ✅ FIXED | SLCR MIO configured (`main.rs:61-78`) |
| **HAL Drivers** | ✅ READY | 17 drivers in `hal/` |
| **FBC Protocol** | ✅ READY | 28 commands implemented |

### GUI (Tauri + React)

| Component | Status | Notes |
|-----------|--------|-------|
| **Protocol Client** | ✅ READY | `gui/src-tauri/src/fbc.rs` |
| **State Machine** | ✅ READY | `gui/src-tauri/src/state.rs` |
| **Pattern Converter** | ✅ COMPLETE | `gen_fbc.c` added March 2026, outputs `.fbc` |
| **Device Config** | ✅ READY | Generates PIN_MAP, .map, .lvl, etc. |
| **Real-time Monitor** | ✅ READY | Heartbeat listener, telemetry |

---

## Clock Architecture

```
33.333 MHz oscillator
    │
    ▼
clk_gen.v (MMCM)
    │
    ├── clk_100m (100 MHz) ──▶ AXI bus, PS peripherals
    ├── clk_200m (200 MHz) ──▶ Vector timing base
    └── vec_clk (5/10/25/50/100 MHz) ──▶ Vector execution
         │
         ▼
    clk_ctrl.v (freq_sel)
         │
         ├── 0 → 5 MHz
         ├── 1 → 10 MHz
         ├── 2 → 25 MHz
         ├── 3 → 50 MHz (default)
         └── 4 → 100 MHz
```

**Note:** Phase clocks (CLKOUT5/6) are hardwired at 50MHz@90°/180° — they don't follow `freq_sel`.

---

## Memory Map

### AXI Peripherals (PS View)

| Peripheral | Base Address | Size | Purpose |
|------------|--------------|------|---------|
| `axi_fbc_ctrl` | 0x4004_0000 | 4KB | FBC decoder control |
| `io_config` | 0x4005_0000 | 4KB | Pin type configuration |
| `axi_vector_status` | 0x4006_0000 | 4KB | Vector execution status |
| `axi_freq_counter` | 0x4007_0000 | 4KB | 8-channel frequency counter |
| `clk_ctrl` | 0x4008_0000 | 4KB | Clock control (freq_sel) |
| `error_bram` | 0x4009_0000 | 4KB | Error logging BRAMs (×3) |
| `fbc_dma` | 0x4040_0000 | 4KB | AXI DMA (HP0 master) |

### FPGA Address Decode

```verilog
// system_top.v:675-682
wire fbc_sel    = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h4);
wire io_sel     = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h5);
wire status_sel = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h6);
wire freq_sel   = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h7);
wire clk_sel    = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h8);
wire err_sel    = (m_axi_gp0_awaddr[31:20] == 12'h400) && (m_axi_gp0_awaddr[19:16] == 4'h9);
wire dma_sel    = (m_axi_gp0_awaddr[31:20] == 12'h404);
```

---

## Protocol Overview

### FBC Commands (28 total)

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

### Packet Format

```
Ethernet Frame (EtherType 0x88B5)
├─ 14 bytes: Ethernet header (src MAC, dst MAC, EtherType)
├─ 8 bytes: FBC header (magic=0xFBC0, seq, cmd, flags, length)
└─ N bytes: Payload (big-endian)
```

---

## Hardware Interfaces

### Power

| Rail | Voltage | Source | Monitored |
|------|---------|--------|-----------|
| VCCINT | ~1.0V | Zynq PS | XADC |
| VCCAUX | ~1.8V | Zynq PS | XADC |
| VICOR cores (×6) | Programmable | VICOR DAC | Firmware |
| LCPS channels | Programmable | PMBus | Firmware |

### JTAG Header (J1)

**Connector:** Molex 87832-1420 (2x7, 2mm pitch, shrouded)

```
    Top View (component side)
    ═══ = key notch
     ┌─────────────────┐
GND  │  1   2  │  VREF (3.3V)
GND  │  3   4  │  TMS
GND  │  5   6  │  TCK
GND  │  7   8  │  TDO
GND  │  9  10  │  TDI
GND  │ 11  12  │  GND/NC
GND  │ 13  14  │  n_SRST
     └─────────────────┘
       Pin 1 (keyed)
```

### 12V Power Input

**Test Point:** TP16 (or J3/J4 pins 181-184 for backplane)

**Pins:** All 4 pins are paralleled 12V input

---

## Known Gaps

### 🔴 HIGH Priority

| Gap | Impact | Fix | Effort |
|-----|--------|-----|--------|
| Firmware update not wired | Can't test FW update end-to-end | Wire `pending_fw_*` in `main.rs` | 1 day |

### 🟡 LOW Priority

| Gap | Impact | Fix | Effort |
|-----|--------|-----|--------|
| LOOP_N non-functional | Loops must be unrolled | Add instruction buffer to decoder | 1 week |
| Phase clocks hardwired | Pulse timing only correct at 50MHz | Add phase shifters per frequency | 1 week |
| 4 opcodes unimplemented | SYNC/IMM32/IMM128/PATTERN_SEQ → S_ERROR | Implement in decoder | 2 days |
| FreqCounter never used | Implemented but firmware never reads it | Add to analog telemetry | 1 day |
| PCAP module unused | FPGA reprogramming capability exists | Call from firmware update | 1 day |

**See:** `docs/GAPS.md` for detailed implementation plans.

---

## Related Documentation

| Document | Purpose |
|----------|---------|
| `README.md` | Project overview, quick start |
| `CLAUDE.md` | Ground truth, hardware status |
| `docs/HARDWARE.md` | Hardware status, pinouts, power |
| `docs/FIRMWARE.md` | Firmware architecture |
| `docs/PROTOCOL.md` | FBC protocol spec |
| `docs/MIGRATION.md` | Legacy → FBC migration guide |
| `docs/GAPS.md` | Known gaps and implementation plans |

---

**Next Steps:**
1. Test AXI register access (all 6 peripherals)
2. Run simple vector, verify GPIO toggling
3. Test firmware update pipeline (BEGIN/CHUNK/COMMIT)
