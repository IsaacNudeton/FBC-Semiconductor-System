# kzhang_v2 Reference Design (2016)

Vivado 2015.4 project for xc7z020clg484-1 (Zynq 7020).
Original burn-in test system - basis for FBC modernization.

## Files

### Core RTL

| File | Purpose |
|------|---------|
| `top.v` | Top-level module. Integrates Zynq PS, AXI peripherals, vector engine, I/O control. 75KB - the main design. |
| `vector.vh` | Global defines: PIN_COUNT=160, VECTOR_WIDTH=128, REPEAT_WIDTH=32, pin type encodings (BIDI, INPUT, OUTPUT, PULSE, etc.) |
| `axi_slave.v` | Generic AXI4-Lite slave interface. Handles read/write transactions from PS. |
| `axi_slave.vh` | AXI slave register definitions and macros. |

### Peripherals

| File | Purpose |
|------|---------|
| `axi_vector_status.v` | Vector execution status registers. Tracks run state, error counts, current position. |
| `axi_io_table.v` | I/O configuration table. Maps logical pins to physical pins, sets pin types. |
| `io_table.v` | Pin type decoder. Converts 4-bit pin type to control signals (oen, compare_en, etc.) |
| `axi_freq_counter.v` | Frequency measurement peripheral. Measures input clock frequencies. |
| `axi_pulse_ctrl.v` | Pulse timing control. Generates timed pulses with programmable width/delay. |

### Constraints

| File | Purpose |
|------|---------|
| `broadcom_v21d.xdc` | Pin assignments for Broadcom test board. Maps FPGA pins to connector signals. |
| `gpio_old_board.xdc` | Alternate pin assignments for older board revision. |

### Build Artifacts

| File | Purpose |
|------|---------|
| `top.bit` | Working bitstream (4MB). Generated 2016/09/30 with Vivado 2015.4. |
| `kzhang_v2.xpr` | Vivado project file. References all sources and constraints. |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Zynq PS (ARM Cortex-A9)                                    │
│  - Runs Linux/bare-metal                                    │
│  - Controls FPGA via AXI                                    │
│  - DMA for vector streaming                                 │
└─────────────────────┬───────────────────────────────────────┘
                      │ AXI4-Lite / AXI4-Stream
┌─────────────────────▼───────────────────────────────────────┐
│  FPGA Fabric (PL)                                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │ axi_slave   │  │ vector_     │  │ axi_pulse_  │         │
│  │ (registers) │  │ status      │  │ ctrl        │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │ axi_io_     │  │ axi_freq_   │  │ io_table    │         │
│  │ table       │  │ counter     │  │ (decode)    │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
│                         │                                   │
│                         ▼                                   │
│              ┌─────────────────────┐                       │
│              │  160 GPIO pins      │                       │
│              │  (to DUT)           │                       │
│              └─────────────────────┘                       │
└─────────────────────────────────────────────────────────────┘
```

## Pin Types (from vector.vh)

| Code | Name | Behavior |
|------|------|----------|
| 0000 | BIDI_PIN | Bidirectional: drive or compare |
| 0001 | INPUT_PIN | Input only: compare against expected |
| 0010 | OUTPUT_PIN | Output only: drive pattern |
| 0011 | OPEN_C_PIN | Open collector: pull low or float |
| 0100 | PULSE_PIN | Pulse: rising edge at T/4, falling at 3T/4 |
| 0101 | NPULSE_PIN | Inverted pulse |
| 0110 | ERROR_TRIG | Outputs error detection signal |
| 0111 | VEC_CLK_PIN | Outputs vector clock |
| 1000 | VEC_CLK_EN_PIN | Outputs vector clock enable |

## Vector Format

- 128-bit vector data (VECTOR_WIDTH)
- 32-bit repeat count (REPEAT_WIDTH)
- Streamed via AXI4-Stream from PS memory
- FIFO buffered (256 entries)

## Key Parameters

- Target: xc7z020clg484-1
- FCLK_CLK0: 100 MHz (AXI clock)
- FCLK_CLK1: 200 MHz (vector timing)
- PIN_COUNT: 160 GPIOs
- VECTOR_WIDTH: 128 bits
- Built: Vivado 2015.4, September 2016
