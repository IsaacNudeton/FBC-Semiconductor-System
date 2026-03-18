# FBC Semiconductor System

FPGA-based burn-in test system for ~500 Zynq 7020 controllers. Modernizing the 2016 Sonoma/kzhang_v2 system with bare-metal firmware and custom FPGA toolchain.

## Project Status: Active Development

| Component | Progress | Notes |
|-----------|----------|-------|
| RTL Core | 85% | I/O subsystem, clock gen, ARM interface complete |
| Testbenches | 40% | Decoder, io_cell, io_bank, clk_gen, top |
| Firmware | 90% | HAL complete (17 drivers), FBC Protocol implemented, **running on hardware** |
| FPGA Toolchain | 99% | ONETWO routing complete (3,488 frames validated) |
| **Host CLI** | **100%** | **28 FBC commands + 20 Sonoma + profile — fully implemented** |
| FBC System GUI | 100% | Tauri + React, 57 commands, Pattern Converter + Pin Import integrated |
| Pattern Converter | 100% | ATP/STIL/AVC → `.hex` ✅, `.fbc` ✅, CSV/Excel/PDF → Pin Import ✅ |
| Vector Converters (Rust) | 100% | `.fvec` → `.fbc` (tested, 4.8-710x compression) |

**✅ March 2026:** First Light achieved — firmware running on Zynq 7020 (CPU @ 667MHz, DDR @ 533MHz)
**✅ March 2026:** Pattern Converter complete — `gen_fbc.c` added, outputs `.fbc` compressed format
**✅ March 2026:** Host CLI complete — all 28 FBC commands implemented (pause, resume, pmbus, eeprom-write, firmware-update, log-info, read-log)

---

## ⚠️ Pattern Converter Gap

**UPDATE March 2026:** This gap has been **FIXED**. `gen_fbc.c` exists and is integrated.

~~**Problem:** Customer patterns (ATP/STIL/AVC) cannot be converted to `.fbc` (compressed FBC format).~~

~~```
ATP/STIL/AVC ──▶ C Engine (gui/src-tauri/c-engine/pc/) ──▶ .hex + .seq ✅
                                                              │
                                                              ▼
                                                        Legacy system
                                                              │
                                                              ❌
                                                              │
                                                        .fbc format
                                                              ▲
                                                              │
.fvec ──▶ Rust (host/src/vector/) ──▶ .fbc ✅─────────────────┘
```~~

~~**Why This Matters:**~~
~~`.hex` = 40 bytes/vector (uncompressed, legacy Sonoma format)~~
~~`.fbc` = 1-21 bytes/vector (compressed: VECTOR_ZERO=1B, VECTOR_RUN=5B, VECTOR_SPARSE=2+N bytes)~~
~~**Compression:** 4.8-710x smaller (verified: test_core.fbc = 77KB vs 55MB uncompressed)~~
~~**Migration:** All existing ATP/STIL/AVC patterns need `.fbc` for FBC system~~

~~**Implementation Status:**~~
| Converter | Input | Output | Status |
|-----------|-------|--------|--------|
| C Engine (`gui/src-tauri/c-engine/pc/`) | ATP/STIL/AVC | `.hex` + `.seq` | ✅ Complete (14 C files) |
| Rust Compiler (`host/src/vector/`) | `.fvec` | `.fbc` | ✅ Complete |
| **gen_fbc.c** (`gui/src-tauri/c-engine/pc/`) | PcPattern IR | `.fbc` | ✅ **Complete March 2026** |

~~**Recommended Fix:** Add `gen_fbc.c` to `gui/src-tauri/c-engine/pc/` — outputs `.fbc` opcodes directly from C engine. — **DONE**~~

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         HOST PC                                     │
│  ┌──────────────┐                                                   │
│  │   host/      │  fbc-cli --host 172.16.0.100 run test.fbc        │
│  │  (Rust CLI)  │────────────────────┐                              │
│  └──────────────┘                    │ TCP/IP                       │
└──────────────────────────────────────┼──────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      ZYNQ 7020 BOARD (x500)                         │
│  ┌──────────────────────────┐    ┌──────────────────────────────┐  │
│  │      ARM Cores (PS)      │    │        FPGA Fabric (PL)      │  │
│  │  ┌────────────────────┐  │    │  ┌────────────────────────┐  │  │
│  │  │    firmware/       │  │◄──►│  │        rtl/            │  │  │
│  │  │  (bare-metal Rust) │  │AXI │  │  fbc_top.v             │  │  │
│  │  │   - TCP server     │  │    │  │  fbc_decoder.v         │  │  │
│  │  │   - DMA control    │  │    │  │  vector_engine.v       │  │  │
│  │  │   - Vector loading │  │    │  │  error_counter.v       │  │  │
│  │  └────────────────────┘  │    │  └────────────────────────┘  │  │
│  └──────────────────────────┘    └──────────────┬───────────────┘  │
│                                                  │ 160 GPIOs        │
└──────────────────────────────────────────────────┼──────────────────┘
                                                   ▼
                                          ┌───────────────┐
                                          │  Chip Under   │
                                          │     Test      │
                                          └───────────────┘
```

## Project Structure

```
FBC Semiconductor System/
├── constraints/        # XDC pin constraints for Zynq 7020
├── docs/               # Learning notes on Zynq architecture
├── firmware/           # Bare-metal Rust for ARM Cortex-A9 (replaces Linux)
├── fpga-toolchain/     # Custom Verilog→bitstream flow (replaces Vivado)
├── host/               # PC-side CLI tool for controlling boards
├── reference/          # 2016 kzhang_v2 files for comparison
├── rtl/                # New FBC Verilog design
├── scripts/            # Vivado TCL scripts (legacy flow)
├── tb/                 # Verilog testbenches
├── Makefile            # Build automation
├── TODO.md             # Development roadmap
└── CLAUDE.md           # AI context file (project state & directions)
```

## RTL Modules

| File | Purpose |
|------|---------|
| `fbc_pkg.vh` | Global defines (widths, opcodes, timing) |
| `fbc_top.v` | Top-level wrapper |
| `fbc_decoder.v` | Decodes FBC instructions → control signals |
| `vector_engine.v` | Executes test vectors, handles timing |
| `error_counter.v` | Counts and logs pin mismatches |
| `axi_fbc_ctrl.v` | AXI register interface to ARM |
| `axi_stream_fbc.v` | AXI-Stream for vector DMA |

## FBC Opcodes

| Opcode | Hex | Description |
|--------|-----|-------------|
| PATTERN_REP | 0xB5 | Repeat pin pattern N times |
| LOOP_N | 0xB0 | Loop instruction block N times |
| PATTERN_SEQ | 0xB6 | Generate sequential pattern |
| SET_PINS | 0xC0 | Set pin values directly |
| SET_OEN | 0xC1 | Set output enables |
| WAIT | 0xD0 | Wait N cycles |
| HALT | 0xFF | End of program |

## Building

### FPGA (Custom Toolchain)
```bash
cd fpga-toolchain
cargo build --release
./target/release/fbc-synth build ../rtl/fbc_pkg.vh ../rtl/fbc_top.v \
    ../rtl/fbc_decoder.v ../rtl/axi_stream_fbc.v ../rtl/vector_engine.v \
    ../rtl/error_counter.v ../rtl/axi_fbc_ctrl.v -o ../top.bit
```

### Simulation (Icarus Verilog)
```bash
make sim-fbc      # Run decoder testbench
make sim-top      # Run top-level testbench
```

### Firmware
```bash
cd firmware
cargo build --release --target armv7a-none-eabi
```

## Key Improvements Over 2016 System

| Aspect | Legacy (Sonoma) | FBC System |
|--------|-----------------|------------|
| OS | Linux (~30s boot) | Bare-metal (<1s boot) |
| Build | Vivado 2015.4 | Custom toolchain |
| Pattern encoding | Raw vectors | FBC compressed |
| Firmware | Shell scripts + netcat | Rust with proper protocol |
| Error handling | /tmp fills up, crashes | Hardware-limited |

## License

Proprietary - Isaac Nudeton / ISE Labs
