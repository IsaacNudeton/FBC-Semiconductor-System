# FBC Semiconductor System

FPGA-based burn-in test system for ~500 Zynq 7020 controllers. Modernizing the 2016 Sonoma/kzhang_v2 system with bare-metal firmware and custom FPGA toolchain.

## Project Status: Active Development

| Component | Progress | Notes |
|-----------|----------|-------|
| RTL Core | 85% | I/O subsystem, clock gen, ARM interface complete |
| Testbenches | 40% | Decoder, io_cell, io_bank, clk_gen, top |
| Firmware | 45% | HAL complete (12 drivers), FBC Protocol implemented |
| FPGA Toolchain | 99% | ONETWO routing complete (3,488 frames validated) |
| Host CLI | 20% | FBC Protocol structures defined |
| FBC System GUI | 0% | Planned (egui/Tauri), docs complete |
| Vector Converters | 100% | fbc-vec tool: STIL/AVC/PAT/APS → FBC/Sonoma/PAT (tested, 145-95,952x compression) |

See `TODO.md` for detailed roadmap.

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
