# CLAUDE.md — FBC Semiconductor System

Ground truth from code-level audit (March 2026). Every claim verified by reading source.

---

## What This Is

Burn-in test system for semiconductor chips. ~44 Zynq 7020 FPGA boards per system.
Replaces the 2016 Sonoma/kzhang_v2 Linux system with:
- **Bare-metal Rust firmware** (no OS, <1s boot)
- **Custom FPGA toolchain** (ONETWO-derived, no Vivado)
- **Optimized RTL** (first-principles design, not legacy port)

**Owner:** Isaac Nudeton / ISE Labs

---

## CRITICAL: Read Before Touching Anything

### Terminology
| Term | Meaning |
|------|---------|
| VICOR | 6 high-current core power supplies |
| LCPS | Low Current Power Supply (PMBus) |
| BIM | Board Interface Module — EEPROM on interposer identifies board type |
| DUT | Device Under Test (the chip being burned in) |
| Fast Pins | gpio[128:159], direct FPGA I/O, 1-cycle latency |
| BIM Pins | gpio[0:127], through interposer, 2-cycle latency |
| ONETWO | Our methodology: decompose to invariants (ONE), then build (TWO) |
| `.hex` | Legacy Sonoma format — 40 bytes/vector, uncompressed |
| `.fbc` | **FBC compressed format** — 1-21 bytes/vector (VECTOR_ZERO, VECTOR_RUN, VECTOR_SPARSE opcodes) |

### Source of Truth Files
| What | File | Why |
|------|------|-----|
| FBC instruction set | `rtl/fbc_pkg.vh` | Defines all opcodes, widths, parameters |
| Instruction execution | `rtl/fbc_decoder.v` | State machine that runs FBC bytecode |
| FPGA integration | `rtl/system_top.v` | Zynq PS7 + all AXI peripherals |
| Protocol wire format | `firmware/src/fbc_protocol.rs` | 28 commands, 8 subsystems, all payload structs (was protocol.rs, deleted) |
| Register access | `firmware/src/regs.rs` | All FPGA register offsets (verified vs RTL) |
| Main firmware loop | `firmware/src/main.rs` | Boot, networking, command dispatch |
| GUI protocol client | `gui/src-tauri/src/fbc.rs` | Socket, types, constants |
| GUI state machine | `gui/src-tauri/src/state.rs` | All command send/recv, payload parsing |
| JTAG programmer | `C:\Dev\xyzt_pico_Hardware\fpga_jtag.py` | Multi-device JTAG, programs PL via FT232H |
| Build script | `scripts/build_bitstream.tcl` | Full Vivado build: PS7 + synth + impl + bitgen |

---

## ✅ Pattern Converter — All Output Formats Complete (March 2026)

**Full pipeline now works:**
```
ATP/STIL/AVC + PIN_MAP
    ↓
C Engine (gui/src-tauri/c-engine/pc/)
    ↓
├── gen_hex.c  → .hex (40 bytes/vector, legacy Sonoma)
├── gen_seq.c  → .seq (test sequence text)
└── gen_fbc.c  → .fbc (compressed FBC: 1-21 bytes/vector, 4.8-710x compression)
```

| Converter | Input | Output | Status |
|-----------|-------|--------|--------|
| **C Engine** (`c-engine/pc/`) | ATP/STIL/AVC | `.hex` + `.seq` | ✅ Complete |
| **gen_fbc.c** (`c-engine/pc/`) | PcPattern IR | `.fbc` | ✅ Complete (March 2026) |
| **Rust Compiler** (`host/src/vector/`) | `.fvec` (text) | `.fbc` | ✅ Complete |

**gen_fbc.c** implements the same compression algorithm as the Rust compiler (compiler.rs),
byte-compatible: ZERO(1B) / ONES(1B) / RUN(1+4B) / SPARSE(1+1+NB, crossover=15) / FULL(1+20B).
CRC32 over header+pin_config+data (IEEE 802.3). Tauri command `pc_convert` accepts `fbc_output`
parameter. Frontend has `.fbc OUT` file picker in Pattern Conversion tab.

### Pin Import Feature (March 2026)

**Solved:** Engineers can now import pin tables directly from datasheets without writing device JSON manually.

| Source | Input | Output | Status |
|--------|-------|--------|--------|
| CSV/Excel/PDF → Pin Table | .csv/.xlsx/.pdf | Editable pin table + device JSON | ✅ Complete |
| Pin Table → Device Config | Extracted pins | PIN_MAP + .map + .lvl + .tim + .tp | ✅ Complete |
| Cross-Verification | 2x pin tables | Mismatch report (channel/voltage/direction) | ✅ Complete |

**Files:**
- Rust: `gui/src-tauri/src/pattern_converter/pin_extractor.rs` (CSV/Excel/PDF extraction)
- Commands: `extract_pin_table`, `verify_pin_tables`, `generate_from_extracted`
- Frontend: PatternConverterPanel.tsx → Pin Import tab (editable tables, format badges, mismatch highlighting)
- Dependencies: calamine (Excel), pdf-extract (PDF), csv (CSV)

**Workflow:** Select source file → Extract → Edit inline → (optional) Verify against secondary source → Generate All

---

## Multi-System Profiling Architecture

This app supports 5 tester system types. The profiling concept exists at **3 layers**:

### Layer 1: Inventory Database (lrm_schema.h)
```c
// gui/src-tauri/c-engine/lrm_schema.h:28-31
typedef enum {
    SYS_HX=0, SYS_SONOMA=1, SYS_XP160=2, SYS_MCC=3, SYS_SHASTA=4, SYS__COUNT=5
} SystemType;
```
- Used in `System` struct (every physical machine record has a type)
- Used in `HardwareType` struct (`for_system_type` restricts hardware to specific systems)
- String table: `schema.c:12` — `{"HX","Sonoma","XP-160","MCC","Shasta"}`
- Runtime branching: `inventory.c:187` — location tree generation differs per system type

### Layer 2: Pattern Converter Profiles (dc.h / dc_json.c)
```c
// gui/src-tauri/c-engine/pc/dc.h:94-107
typedef struct {
    char       name[DC_MAX_NAME];      // "Sonoma", "HX", "XP-160/Shasta", "MCC"
    int        total_channels;         // 128 (Sonoma/MCC), 160 (HX/XP-160 per axis)
    DcGpioBank banks[DC_MAX_BANKS];    // Pin bank layout
    int        num_banks;
    DcCoreHw   cores[DC_MAX_SUPPLIES]; // Power supply hardware
    int        num_cores;
    // ... firmware_path, vector_dir, timing defaults
} DcTesterProfile;
```
- Built-in profiles embedded as JSON strings in `dc_json.c`
- Currently: **only Sonoma built in** — HX, XP-160/Shasta, MCC profiles need adding
- Lookup: `dc_get_builtin_profile("sonoma")` → returns JSON, parsed into struct
- See `PROFILE-INSTRUCTIONS.md` for exact code to add remaining profiles

### Layer 3: Host CLI Transport (host/src/bin/cli.rs)
```rust
// host/src/bin/cli.rs:48-76
enum Commands {
    Fbc { ... },     // Raw Ethernet 0x88B5 (bare-metal FPGA)
    Sonoma { ... },  // SSH + ELF binaries (Linux Zynq)
}
```
- Transport selection is implicit via CLI subcommand
- `SonomaClient` (SSH) vs `FbcClient` (raw Ethernet) in host/src/
- **Missing link:** board's SystemType should auto-select transport + profile

### System Specs (from FSHC tester.rs + hardware verification)

| System | Channels | Axes | Supplies | Timing | Thermal | Transport |
|--------|----------|------|----------|--------|---------|-----------|
| **Sonoma** | 128 | 1 | 6 VICOR | 100ps/200MHz | Watlow 4-zone | SSH (Linux) |
| **HX** | 160/axis | 4 | 16 RMA5608 | 200ps/200MHz | RMA5608 4-zone | INSPIRE |
| **XP-160/Shasta** | 160/axis | 8 | 32 RMA5608 | 200ps/200MHz | RMA5608 8-zone | INSPIRE |
| **MCC** | 128 | 1 | 8 | 1ns/50MHz | Watlow 1-zone | Modbus TCP |
| **FBC** (future) | 160 | 1 | 6 VICOR | 100ps/200MHz | Watlow 4-zone | Raw Ethernet |

HX and XP-160/Shasta use **the same driver** — Shasta is just newer. Only difference = axis count.
Per-axis layout identical: 96 drive + 60 monitor + 4 reserved = 160 channels.

### What's Complete vs Missing

| Layer | Sonoma | HX | XP-160/Shasta | MCC | FBC |
|-------|--------|----|---------------|-----|-----|
| LRM SystemType enum | ✅ | ✅ | ✅ | ✅ | ❌ (add SYS_FBC=5) |
| C Engine profile (dc_json.c) | ✅ | ❌ | ❌ | ❌ | ❌ |
| GUI dropdown | ✅ | ❌ | ❌ | ❌ | ❌ |
| Host transport | ✅ SSH | ❌ | ❌ | ❌ | ✅ Raw Ethernet |
| Pattern converter output | ✅ .hex/.seq/.fbc | ❌ | ❌ | ❌ | ✅ .fbc |

### Key Files for Multi-System Work

| File | Purpose |
|------|---------|
| `gui/src-tauri/c-engine/lrm_schema.h` | SystemType enum, System/HardwareType structs |
| `gui/src-tauri/c-engine/schema.c` | system_type_str(), enum-to-string |
| `gui/src-tauri/c-engine/inventory.c` | System-specific location tree generation |
| `gui/src-tauri/c-engine/pc/dc.h` | DcTesterProfile struct, limits (DC_MAX_SUPPLIES etc.) |
| `gui/src-tauri/c-engine/pc/dc_json.c` | Built-in profile JSONs + dc_get_builtin_profile() |
| `gui/src-tauri/c-engine/pc/dc_gen.c` | File generators (profile-agnostic, work with any profile) |
| `gui/src-tauri/src/pattern_converter/pc_ffi.rs` | Rust→C FFI (passes profile name as string) |
| `gui/src-tauri/src/lib.rs` | Tauri commands (profile parameter flows through) |
| `gui/src/components/PatternConverterPanel.tsx` | Profile dropdowns (currently Sonoma-only) |
| `host/src/bin/cli.rs` | Transport subcommands (Fbc vs Sonoma) |
| `host/src/types.rs` | SonomaStatus, RunResult (system-specific types) |
| `PROFILE-INSTRUCTIONS.md` | Implementation guide for adding HX/XP-160/MCC profiles |
| `C:\Dev\FSHC - Hardware\fshc\crates\fshc-platform\src\tester.rs` | Complete Rust TesterProfile definitions for all systems |

---

## Directory Structure (What's Real)

```
├── rtl/                   # 16 Verilog modules (VERIFIED, programmed on hardware)
│   ├── system_top.v       # Top: PS7 + clk_gen + clk_ctrl + freq_counter + fbc_top + fbc_dma + 3×error_bram
│   ├── fbc_top.v          # FBC core: io_config + io_bank + axi_stream_fbc + fbc_decoder + vector_engine + error_counter + axi_fbc_ctrl + axi_vector_status
│   ├── fbc_dma.v          # AXI DMA: HP0 DDR read → 256-bit AXI-Stream to fbc_decoder
│   └── (13 more)          # io_cell, clk_gen, error_bram, etc.
├── tb/                    # Testbenches
├── constraints/           # Pin constraints (.xdc)
├── firmware/              # ARM Cortex-A9 bare-metal Rust (27 source files)
│   └── src/
│       ├── main.rs        # Entry, boot, main loop
│       ├── fbc_protocol.rs # 28 commands, all payloads
│       ├── regs.rs        # FPGA register access
│       ├── dma.rs         # AXI DMA + FbcStreamer
│       ├── analog.rs      # 32-ch ADC (XADC + MAX11131)
│       ├── net.rs         # Zynq GEM Ethernet driver
│       └── hal/           # 16 hardware drivers
├── gui/                   # Tauri + React + Three.js
│   ├── src/               # React frontend
│   └── src-tauri/         # Rust backend
├── host/                  # CLI for multi-board control (discover + ping only)
├── reference/             # Old 2016 kzhang_v2 design (READ ONLY reference)
├── docs/                  # Architecture analysis docs
├── fsbl/                  # First Stage Boot Loader
├── tools/                 # Utilities (routing verify, rawwrite)
├── testplans/             # Test plan examples
├── onetwo.c               # ONETWO reasoning scaffold
└── CLAUDE.md              # THIS FILE
```

### Hardware Status (March 2026)

| Component | Status | Notes |
|-----------|--------|-------|
| **PL (FPGA)** | ✅ **PROGRAMMED** | Bitstream loaded via JTAG, all AXI peripherals accessible |
| **PS (ARM Firmware)** | ✅ **RUNNING** | First Light achieved March 2026 — CPU @ 667MHz, DDR @ 533MHz, ANNOUNCE packet sent |
| **VICOR GPIO** | ✅ FIXED | SLCR MIO mux configured in `main.rs:61-78` |
| **Error BRAM** | ✅ WIRED | 3× BRAMs at 0x4009_0000, protocol handler added |
| **DMA** | ✅ WIRED | `fbc_dma.v` instantiated, used by `FbcStreamer` |

### What's NOT Here
- `archive/` — DELETED (was OBI-1 physics, wrong repo)
- `learning/` — DELETED (stale HTML tutorials)
- `STATUS.md` — DELETED (stale percentages)
- `TODO.md` — DELETED (outdated 677-line roadmap)

---

## Architecture: What Actually Works End-to-End

```
GUI (Tauri) ──Raw Ethernet 0x88B5──▶ Firmware (bare-metal Rust on Cortex-A9)
                                         │
                                    AXI-Lite bus (GP0)
                                         │
       ┌──────────┬──────────┬───────────┼───────────┬──────────┬──────────┐
       ▼          ▼          ▼           ▼           ▼          ▼          ▼
  fbc_ctrl   io_config   vec_status  clk_ctrl  freq_counter err_bram   fbc_dma
  0x4004_0   0x4005_0    0x4006_0   0x4008_0   0x4007_0    0x4009_0   0x4040_0
  ctrl/stat  pin types   error/vec  freq_sel   4ch meas    3×BRAM     HP0 DMA
       │          │          ▲           │                     ▲          │
       ▼          ▼          │           ▼                     │          ▼
    fbc_dma ──▶ axi_stream_fbc ──▶ fbc_decoder ──▶ vector_engine ──▶ io_bank ──▶ 160 Pins
    (HP0 DDR)   (256→64+128)      (7 opcodes)    (repeat+errors)   (128 BIM + 32 fast)
```

**Protocol:** Raw Ethernet frames, EtherType 0x88B5, 8-byte FbcHeader (magic=0xFBC0, seq, cmd, flags, length), big-endian payloads.
**JTAG chain:** TDI → ARM DAP (0x4BA00477) → XC7Z020 PL (0x23727093) → TDO
**Bitstream:** `build/fbc_system.bit` (3.9 MB) — programmed and verified on real hardware March 13, 2026.

---

## KNOWN BUGS (Re-verified March 2026)

### ~~🔴 Critical~~

~~1. **DMA not integrated**~~ — **FIXED.** `fbc_dma.v` instantiated in `system_top.v` at lines 834-884. AXI-Lite on GP0 at 0x4040_0000, AXI master on HP0, AXI-Stream to FBC decoder. IRQ wired.

### ~~FIXED~~

2. ~~**Host CLI broken**~~ — FbcClient now wraps `FbcRawSocket`. Correct 8-byte FBC header.
3. ~~**VICOR status always timeouts**~~ — Length 48→30, parser 8B→5B per core.
4. ~~**FastPins wire-order swap**~~ — GUI reads `(din, dout, oen)` matching firmware.
7. ~~**Rail data dropped**~~ — `BoardStatus` has `rail_voltage_mv`/`rail_current_ma`, `parse_status()` reads all 47B.
12. ~~**443 lines dead code**~~ — `firmware/src/protocol.rs` deleted.
13. ~~**~290 lines dead code (host)**~~ — Old FbcClient replaced.
14. ~~**Duplicate assigns fbc_top.v**~~ — Already gone. Lines 519-521 are the only assignments.

### ~~🟡 High~~

~~5. **All responses broadcast**~~ — **FIXED.** All `send_fbc()` calls now use `sender_mac` or `last_sender_mac`. Only the initial ANNOUNCE (line 240) uses `BROADCAST_MAC`, which is correct behavior.

### 🟡 Medium

6. **LOOP_N non-functional** — `fbc_decoder.v:126-128`: counts iterations but has no instruction buffer/PC to replay loop body. All loops must be unrolled in bytecode.

~~8. **Error BRAMs unconnected**~~ — **FIXED.** 3× `error_bram.v` instantiated in `system_top.v` (lines 892-935). AXI read interface at 0x4009_0000 (lines 937-996). Write index at 0x00, read pattern/vector/cycle at 0x04-0x1C.

~~8b. **fast_error dropped at system_top**~~ — **FIXED.** `fast_error[31:0]` now wired through `fbc_top` → `axi_fbc_ctrl` at register offset 0x2C (`REG_FAST_ERR`). Firmware reads via `FbcCtrl::read_fast_error()`.

~~9. **VICOR GPIO enable commented out**~~ — **FIXED.** `main.rs:61-78` now configures SLCR MIO mux for all 6 VICOR enable pins `[0, 39, 47, 8, 38, 37]` as GPIO before initializing them as outputs. Note: MIO 0 is shared with status LED.

~~10. **Error BRAM readback returns demo data**~~ — **FIXED.** New protocol commands ERROR_LOG_REQ(0x4A) / ERROR_LOG_RSP(0x4B) implemented end-to-end. Firmware reads `ErrorBram` at 0x4009_0000, returns up to 8 entries (28B each: pattern[128b] + vector + cycle). GUI `get_pattern_stats()`/`get_pattern_errors()` now use real data.

10b. **Phase clocks hardwired** — `clk_gen.v` CLKOUT5/6 fixed at 50MHz@90/180. They don't follow freq_sel. Pulse timing only correct at 50MHz. Would require separate phase shifters per frequency to fix.

### 🟢 Low

11. **5 opcodes unimplemented** — SYNC, IMM32, IMM128, PATTERN_SEQ, SET_BOTH defined in `fbc_pkg.vh` but not in decoder → S_ERROR. SET_BOTH requires 256-bit payload (dout+oen) but bus only carries 128 bits. Use SET_PINS + SET_OEN as two instructions instead.

15. ~~**Host CLI limited**~~ — **FIXED March 2026.** All 28 FBC commands implemented in CLI: pause, resume, pmbus-status, pmbus-enable, eeprom-write, firmware-update, log-info, read-log. Input validation added (core 0-5, voltage 0-5000mV, mask 0-63).

16. **Dead code in net.rs** — `TcpServer` struct (~125 lines), `UdpPacket`/`RawFrame` builders (~136 lines) unused. Were "for initial testing".

17. **FreqCounter never used** — `axi_freq_counter.v` fully implemented (4 independent channels), registered at 0x4007_0000, but firmware never reads it.

18. **PCAP module unused** — `hal/pcap.rs` (358 lines) for FPGA reprogramming, not called from anywhere.

19. **Firmware update untested** — `BEGIN/CHUNK/COMMIT` pipeline exists in both GUI and firmware but never tested on real hardware. **Note:** Protocol layer implemented, but `main.rs` doesn't process `pending_fw_begin/chunk/commit` requests yet (needs wiring like VICOR pattern).

---

## AXI Register Map (Verified: Firmware matches RTL)

### axi_fbc_ctrl — 0x4004_0000

| Offset | Name | R/W | Bits |
|--------|------|-----|------|
| 0x00 | CTRL | R/W | [0]=enable, [1]=reset, [2]=irq_done, [3]=irq_error |
| 0x04 | STATUS | R | [0]=running, [1]=done, [2]=error |
| 0x08 | INSTR_LO | R | instruction count |
| 0x0C | INSTR_HI | R | always 0 (reserved) |
| 0x10 | CYCLE_LO | R | cycle count low |
| 0x14 | CYCLE_HI | R | cycle count high |
| 0x18 | ERROR | R | error flag |
| 0x1C | VERSION | R | 0x0001_0000 |
| 0x20 | FAST_DOUT | R/W | fast pin drive |
| 0x24 | FAST_OEN | R/W | fast pin output enable |
| 0x28 | FAST_DIN | R | fast pin input readback |
| 0x2C | FAST_ERR | R | fast pin error flags (from io_bank) |

### clk_ctrl — 0x4008_0000

| Offset | Name | R/W | Default |
|--------|------|-----|---------|
| 0x00 | FREQ_SEL | R/W | 3 (50MHz). Values: 0=5, 1=10, 2=25, 3=50, 4=100 MHz |
| 0x04 | STATUS | R | [0]=MMCM locked |
| 0x08 | ENABLE | R/W | 0 (disabled) |

### io_config — 0x4005_0000

- 0x000-0x04C: 20 × pin_type registers (4 bits/pin, 160 pins)
- 0x200-0x33C: 80 × pulse_ctrl registers (16 bits/pin, 160 pins)

### axi_vector_status — 0x4006_0000

| Offset | Name | R/W | Bits |
|--------|------|-----|------|
| 0x00 | ERROR_COUNT | R | Total errors from error_counter |
| 0x04 | VECTOR_COUNT | R | Vectors executed |
| 0x08 | CYCLE_LO | R | Cycle count low |
| 0x0C | CYCLE_HI | R | Cycle count high |
| 0x10 | FIRST_ERR_VEC | R | Vector # of first error |
| 0x14 | STATUS | R | [0]=done, [1]=has_errors, [29]=first_error_valid |
| 0x18 | FIRST_ERR_LO | R | First error cycle low |
| 0x1C | FIRST_ERR_HI | R | First error cycle high |
| 0x3C | VERSION | R | 0x0001_0000 |

### axi_freq_counter — 0x4007_0000

4 independent channels, 0x20 bytes per channel. Per-channel registers:

| Offset | Name | R/W | Bits |
|--------|------|-----|------|
| 0x00 | CTRL | R/W | [0]=enable, [1]=irq_en, [23:16]=trig_sel, [15:8]=sig_sel |
| 0x04 | STATUS | R | State + flags |
| 0x08 | MAX_CYCLES | R/W | Max cycle count limit |
| 0x0C | MAX_TIME | R/W | Max time count limit |
| 0x10 | CYCLES | R | Measured cycle count |
| 0x14 | TIME | R | Measured time count |
| 0x18 | TIMEOUT | R/W | Timeout value |

### error_bram — 0x4009_0000

| Offset | Name | R/W | Bits |
|--------|------|-----|------|
| 0x00 | READ_INDEX | W | Error entry index to read (sets address for pattern/vector/cycle BRAMs) |
| 0x04 | PATTERN_0 | R | Error pattern bits [31:0] |
| 0x08 | PATTERN_1 | R | Error pattern bits [63:32] |
| 0x0C | PATTERN_2 | R | Error pattern bits [95:64] |
| 0x10 | PATTERN_3 | R | Error pattern bits [127:96] |
| 0x18 | VECTOR | R | Vector number when error occurred |
| 0x1C | CYCLE_LO | R | Cycle count low 32 bits |
| 0x20 | CYCLE_HI | R | Cycle count high 32 bits |

### fbc_dma — 0x4040_0000

| Offset | Name | R/W | Bits |
|--------|------|-----|------|
| 0x00 | MM2S_DMACR | R/W | [0]=run, [2]=reset, [12]=irq_en |
| 0x04 | MM2S_DMASR | R | [0]=halted, [1]=idle, [12]=ioc_irq, [14]=err_irq |
| 0x18 | MM2S_SA | R/W | Source address |
| 0x28 | MM2S_LENGTH | R/W | Transfer length (write triggers start) |

---

## Protocol Command Map (28 commands)

| Subsystem | Send | Receive | Notes |
|-----------|------|---------|-------|
| Setup | BIM_STATUS_REQ(0x01) | ANNOUNCE(0x02) | Discovery |
| Setup | CONFIGURE(0x30) | CONFIGURE ack | clock_div + voltages |
| Setup | UPLOAD_VECTORS(0x21) | ack | Chunked: offset+total+size+data |
| Runtime | START(0x40) | ack | Enables FBC decoder |
| Runtime | STOP(0x41) | ack | Disables decoder |
| Runtime | RESET(0x42) | ack | Full reset |
| Runtime | STATUS_REQ(0xF0) | STATUS_RSP(0xF1) | 47-byte telemetry |
| Runtime | — | HEARTBEAT(0x48) | 11B, sent every 100ms |
| Runtime | — | ERROR(0x49) | error_type + cycle + count |
| ErrorLog | ERROR_LOG_REQ(0x4A) | ERROR_LOG_RSP(0x4B) | start_index+count → up to 8×28B entries |
| Analog | READ_ALL_REQ(0x70) | READ_ALL_RSP(0x71) | 32 readings, 192B |
| Power | VICOR_STATUS_REQ(0x80) | VICOR_STATUS_RSP(0x81) | 30B (6×5B) |
| Power | VICOR_ENABLE(0x82) | deferred | core_mask byte |
| Power | VICOR_SET_VOLTAGE(0x83) | deferred | core + mv |
| Power | EMERGENCY_STOP(0x88) | immediate ack | Fire-and-forget |
| Power | POWER_SEQUENCE_ON(0x89) | deferred | 6 voltages |
| Power | POWER_SEQUENCE_OFF(0x8A) | deferred | |
| Power | PMBUS_ENABLE(0x8B) | deferred | addr + enable |
| EEPROM | READ_REQ(0xA0) | READ_RSP(0xA1) | offset + data |
| EEPROM | WRITE(0xA2) | WRITE_ACK(0xA3) | offset + len + data |
| FastPins | READ_REQ(0xD0) | READ_RSP(0xD1) | Wire: din,dout,oen. Moved from 0xC0 to avoid collision |
| FastPins | WRITE(0xD2) | — | dout + oen |
| FlightRec | LOG_READ_REQ(0x60) | LOG_READ_RSP(0x61) | SD sector read |
| FlightRec | LOG_INFO_REQ(0x62) | LOG_INFO_RSP(0x63) | Log metadata |
| FW Update | INFO_REQ(0xE1) | INFO_RSP(0xE2) | Version + build |
| FW Update | BEGIN(0xE3) | BEGIN_ACK(0xE4) | size + checksum |
| FW Update | CHUNK(0xE5) | CHUNK_ACK(0xE6) | offset + data |
| FW Update | COMMIT(0xE7) | COMMIT_ACK(0xE8) | Verify + apply |

---

## GUI Command Surface (54 Tauri commands)

| Category | Commands | Count |
|----------|----------|-------|
| Connection | list_interfaces, connect, disconnect | 3 |
| Discovery | discover_boards | 1 |
| Board Control | get_board_status, start/stop/reset_board, upload_vectors | 5 |
| Config | get/set_rack_config, compile_device_config | 3 |
| FastPins | get_fast_pins, set_fast_pins | 2 |
| Analog | read_analog_channels | 1 |
| Power (VICOR) | get_vicor_status, set_vicor_enable, set_vicor_voltage | 3 |
| Power (PMBus) | get_pmbus_status, set_pmbus_enable, emergency_stop, power_seq_on/off | 5 |
| EEPROM | read_eeprom, write_eeprom | 2 |
| Vector Engine | get_vector_status, load/start/pause/resume/stop_vectors | 6 |
| Firmware | detect_fw_type, update_fw_ssh/fbc/batch, get_firmware_info | 5 |
| Realtime | get_live_boards, get_live_board | 2 |
| Switch | get/set_switch_config, discover_board_positions, list_serial_ports | 4 |
| Export/File | export_results, read_file, write_file | 3 |
| Other | terminal_command, get_detailed_status, get_eeprom_info, pattern_stats/errors | 5 |

---

## Build Commands

```bash
# Firmware (bare-metal ARM)
cd firmware && cargo build --release --target armv7a-none-eabi

# GUI (Tauri + React)
cd gui && npm run tauri dev    # Development
cd gui && npm run tauri build  # Production

# FPGA Bitstream (Vivado — golden reference)
# Part: xc7z020clg484-1 (484-pin, NOT clg400!)
cd C:\Dev\projects\FBC-Semiconductor-System
vivado -mode batch -source scripts/build_bitstream.tcl
# Output: build/fbc_system.bit + reports in build/

# FPGA Toolchain (custom — separate repo)
cd C:\Dev\projects\fpga-toolchain && cargo build --release
```

### FPGA Build Notes

- **Part number**: `xc7z020clg484-1` — verified from reference/kzhang_v2_2016/kzhang_v2.xpr. Pin names like AB12 are 484-pin only.
- **Silicon revision**: IDCODE = `0x23727093` (rev 2). Scripts may reference `0x03727093` (rev 0) — both are XC7Z020, just different fab revisions.
- **PS7 IP**: Auto-generated by `scripts/build_bitstream.tcl`. Configures FCLK_CLK0=100MHz, FCLK_CLK1=200MHz, M_AXI_GP0, S_AXI_HP0, IRQ_F2P, DDR3, UART0/I2C0/SPI0/GEM0/SD0 on MIO.
- **DDR part**: MT41K256M16RE-125 assumed. If Sonoma uses different DDR, update `CONFIG.PCW_UIPARAM_DDR_PARTNO` in the TCL script.
- **Constraints**: `constraints/zynq7020_sonoma.xdc` — full 160-pin mapping from Sonoma schematics.

### JTAG Programming

- **Programmer**: FT232H breakout board via MPSSE (NOT FT2232H like Basys 3)
- **Script**: `C:\Dev\xyzt_pico_Hardware\fpga_jtag.py` — multi-device chain support for Zynq
- **J1 Header**: Molex 87832-1420, 2mm pitch, 2x7 pin. **Must solder wires** — 2.54mm dupont wires don't grip.
- **Chain**: TDI → ARM DAP (4-bit IR, 0x4BA00477) → XC7Z020 PL (6-bit IR, 0x23727093) → TDO
- **Wiring**: J1 pin 4 (TMS)→AD3, pin 6 (TCK)→AD0, pin 8 (TDO)→AD2, pin 10 (TDI)→AD1, pin 7 (GND)→GND
- **Programming**: `python fpga_jtag.py --device sonoma program build/fbc_system.bit` — 5.5s, 715 KB/s
- **Power**: 12V/3A via VCC12 pad on quad board (BK Precision 9206). Board draws ~3A with shorted FET on quad board.

---

## What the `reference/` Folder Is

The old 2016 kzhang_v2 design. Linux-based. Uses Vivado IPs, shell scripts, AWK for instrument control. **READ ONLY — for comparison, not for porting.** Our design is fundamentally different because we:
- Removed the OS (bare-metal Rust, not Linux)
- Removed Vivado (ONETWO-derived bitstream)
- Simplified the protocol (raw Ethernet, not TCP/IP stack)
- Unified the register interface (7 AXI peripherals, not scatter-gather)

### Key Reference Subdirectories

| Path | Contents |
|------|----------|
| `reference/sonoma_docs/` | **START HERE** — March 2026 verified Sonoma docs (hardware, firmware, vector engine, device files) |
| `reference/kzhang_v2_2016/` | Original HDL source (vector.vh, io_table.v, etc.) |
| `reference/hpbicontroller-rev1/` | Altium PCB schematics |
| `reference/Everest_3.7.3_20260122_FW_v4.8C/` | Production firmware package |
| `reference/ZYNQ_REGISTER_MAP.md` | Zynq PS peripheral addresses (still valid) |

### Key Sonoma Facts for FBC Development

| Sonoma Value | Source | FBC Equivalent |
|--------------|--------|----------------|
| AXI vector status: 0x43C00000 (inferred) / 0x404E0000 (from ELF) | sonoma_docs/04 | 0x4006_0000 (our design) |
| Sonoma DMA: 4 channels at 0x40400000-0x40430000 | sonoma_docs/01 | fbc_dma.v at 0x4040_0000 (wired in system_top.v) |
| VICOR MIO: [0, 39, 47, 8, 38, 37] | sonoma_docs/04 (AWK verified) | Same mapping in main.rs:61 (SLCR configured) |
| MIO 36 = ADC bank select | sonoma_docs/04 | Used by AnalogMonitor |
| .hex format: 80B/vector (header+oen+dout+mask+ctrl) | sonoma_docs/03 | Our format: 20B/vector (compressed) |
| Pin types 0-7 | sonoma_docs/03 (vector.vh) | Same codes in fbc_pkg.vh |
| Error formula: `oen & (dout ^ din)` | sonoma_docs/03 (io_table.v) | Same in our io_cell.v |

---

## What Needs Doing (Priority Order)

### Immediate Bug Fixes — DONE
1. ~~Fix FastPins wire order~~ — GUI parse_fast_pins now reads (din, dout, oen)
2. ~~Fix VICOR status length check~~ — 48→30, parser 8B→5B per core
3. ~~Wire FbcClient to fbc_protocol.rs~~ — FbcClient wraps FbcRawSocket
4. ~~Add rail_voltage/rail_current to GUI BoardStatus~~ — 8 voltages + 8 currents parsed

### RTL Integration — DONE
5. ~~Instantiate `fbc_dma.v` in `system_top.v`~~ — Wired: GP0 regs + HP0 master + AXI-Stream out
6. ~~Instantiate `error_bram.v` (×3) in `system_top.v`~~ — Wired: pattern/vector/cycle BRAMs + AXI read at 0x4009_0000
7. ~~Connect `fast_error` signal~~ — Wired: fbc_top → axi_fbc_ctrl at 0x2C

### Firmware Integration — DONE
8. ~~Implement unicast responses~~ — All `send_fbc()` calls use `sender_mac`/`last_sender_mac`. Only ANNOUNCE broadcasts.
9. ~~VICOR GPIO enable~~ — SLCR MIO mux configured for all 6 pins before GPIO init. MIO 0 shared with status LED.
10. ~~Error BRAM readback via protocol~~ — ERROR_LOG_REQ(0x4A)/RSP(0x4B), `ErrorBram` struct reads 0x4009_0000, GUI calls `request_error_log()`.

### FPGA Bitstream — DONE
11. ~~Build bitstream~~ — `build/fbc_system.bit` (3.9 MB), 10 Vivado iterations. 12.6% LUT, 7.5% FF, 84% IO. Timing: vec_clk@50MHz passes (+6.5ns margin), AXI@100MHz marginal (-1.0ns). Inter-clock violations from BUFGMUX tree (constraint issue, not design bug).

### Hardware Validation — DONE ✅
12. ~~Flash bitstream to real Zynq 7020~~ — **DONE March 13, 2026.** `fbc_system.bit` programmed via JTAG (FT232H + fpga_jtag.py). DONE pin asserted, STATUS=0x46107FFC. 5.5s @ 715 KB/s.
13. ~~Load ARM firmware via JTAG~~ — **DONE March 2026 (First Light).** CPU @ 667MHz, DDR @ 533MHz, ANNOUNCE packet sent.
14. ~~Verify ANNOUNCE packet on network~~ — **DONE.** Ethernet GEM0 initialized, broadcast sent.
15. Test AXI register access (all 6 peripherals incl. error_bram) — *In progress*
16. Run simple vector, verify GPIO toggling — *Next step*
17. Test firmware update pipeline (BEGIN/CHUNK/COMMIT) on real board — *Requires wiring in main.rs*

### Nice-to-Have
17. Expand host CLI beyond discover+ping
18. Implement LOOP_N instruction buffer in fbc_decoder
19. Add phase-shifted clocks that follow freq_sel
20. Clean up dead code (TcpServer, UdpPacket in net.rs)
21. Add `set_clock_groups -physically_exclusive` for BUFGMUX outputs to fix inter-clock timing warnings

---

## Uncertainties (Need Hardware/Schematic Verification)

These values are in the code but have NOT been verified against the actual hardware:

| What | Value in Code | File | Risk |
|------|--------------|------|------|
| VICOR DAC multiplier | 2× | `vicor.rs:36` | **VERIFIED** — Sonoma uses `voltage*2` in linux_VICOR_Voltage.elf |
| XADC voltage scale | 3000mV | `analog.rs:64` | **VERIFIED** — ADR5043BKSZ 3.0V precision reference on PCB |
| PMBus I2C addresses | `lcps_channel_to_addr()` | `hal/pmbus.rs` | May vary across boards |
| BIM EEPROM format | `BimEeprom` struct | `hal/eeprom.rs` | No external spec, only code |
| VICOR enable MIO pins | `[0, 39, 47, 8, 38, 37]` | `main.rs:61-78` (SLCR configured) | **VERIFIED** from Sonoma AWK. SLCR MIO mux now set to GPIO. MIO 0 shared with status LED. |
| MIO 36 = ADC bank select | ToggleMio 36 for ch 16-31 | Sonoma ReadAnalog.awk | Must not conflict with other MIO 36 usage |
| Clock freq boundaries | 7.5/17.5/37.5/75 MHz | `regs.rs:393-401` | Edge-case freq may pick wrong preset |
| ERR_TRIG pin type (0x6) | Falls to BIDI | `io_cell.v:244` | Marked "causes timing problems" |
| Flight recorder capacity | 1000 sectors (100s) | `main.rs:426` | May need more for long tests |
