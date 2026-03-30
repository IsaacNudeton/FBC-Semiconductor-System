# FBC Semiconductor System - Pin Mapping Reference

## Overview

The Sonoma board uses 160 GPIO pins organized into 4 groups:

| Range | Count | Bank | Path | Latency | Purpose |
|-------|-------|------|------|---------|---------|
| gpio[0:47] | 48 | Bank 13 | FPGA → QSH → BIM → DUT | 2 cycles | DUT I/O |
| gpio[48:95] | 48 | Bank 33 | FPGA → QSH → BIM → DUT | 2 cycles | DUT I/O |
| gpio[96:127] | 32 | Bank 34 | FPGA → QSH → BIM → DUT | 2 cycles | DUT I/O |
| gpio[128:159] | 32 | Direct | FPGA Direct (no BIM) | 1 cycle | Triggers, Clocks |

> **Note:** Quad Board handles power distribution only (VICOR 48V→12V).
> GPIO signals go directly from Controller to BIM via QSH connectors.

## Signal Path

```
SIGNAL PATH (Controller → BIM → DUT)     POWER PATH (Quad Board → BIM)
========================================  ================================

Controller (FPGA)                         Quad Board
┌──────────────────────────────────┐     ┌────────────────────┐
│  Bank 13    Bank 33    Bank 34   │     │  VICOR DC-DC (x4)  │
│  gpio[0:47] gpio[48:95] [96:127] │     │  48V → 12V @ 1.2kW │
└──────┬─────────┬──────────┬──────┘     └─────────┬──────────┘
       │         │          │                      │
       │    QSH Connectors (J3/J4/J5)             │ Power Rails
       │    (signals directly to BIM)             │
       ▼         ▼          ▼                      ▼
┌──────────────────────────────────────────────────────────────┐
│                            BIM                                │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Signal Routing (from QSH)    │  Power (from Quad)   │   │
│  │  128 GPIO → level shift → DUT │  12V → regulators    │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│                    ┌──────────────┐                         │
│                    │     DUT      │                         │
│                    │ (Device Under│                         │
│                    │    Test)     │                         │
│                    └──────────────┘                         │
└──────────────────────────────────────────────────────────────┘

Fast Pins (gpio[128:159]): Direct FPGA → External, no BIM
```

## Special Pin Functions

From reference `broadcom_v21d.xdc`:

| Pin | Function | Notes |
|-----|----------|-------|
| gpio[92] | TDO | JTAG Test Data Out |
| gpio[97] | MDIO2 | Control interface |
| gpio[101] | MDIO5 | Control interface |
| gpio[105] | MDIO1 | Control interface |
| gpio[110] | MDIO6 | Control interface |
| gpio[121] | MDIO4 | Control interface |
| gpio[122] | MDIO3 | Control interface |
| gpio[123] | MDIO0 | Often used for vec_clk output |

## Clock Outputs

Dedicated clock output pins (separate from GPIO):

| Port | Package Pin | Notes |
|------|-------------|-------|
| clk_out_p[0] | D18 | Differential clock pair |
| clk_out_p[1] | Y9 | Differential clock pair |
| clk_out_p[2] | Y18 | Differential clock pair |
| clk_out_p[3] | L18 | Differential clock pair |

## Drive Strength

From reference design:
- **gpio[0:16]**: 8mA (high drive for clock/critical signals)
- **gpio[17:159]**: 4mA (standard drive)

## Pin Type Configuration

Each pin can be configured as one of 9 types via AXI registers:

| Type | Value | Description |
|------|-------|-------------|
| BIDI | 0x0 | Bidirectional with compare |
| INPUT | 0x1 | Input only, compare enabled |
| OUTPUT | 0x2 | Output only, no compare |
| OPEN_C | 0x3 | Open collector |
| PULSE | 0x4 | Pulse output with timing |
| NPULSE | 0x5 | Inverted pulse output |
| ERR_TRIG | 0x6 | Error trigger (reserved) |
| VEC_CLK | 0x7 | Vector clock output |
| VEC_CLK_EN | 0x8 | Vector clock enable |

## AXI Register Map for Pin Configuration

Base address: 0x4005_0000 (AXI_PIN_CTRL_BASE)

| Offset | Size | Description |
|--------|------|-------------|
| 0x000-0x04F | 20 x 32-bit | Pin type (4 bits/pin, 160 pins) |
| 0x200-0x33F | 80 x 32-bit | Pulse timing (16 bits/pin, 160 pins) |

### Pin Type Register Format
Each 32-bit register holds 8 pin types (4 bits each):
```
Register N: [pin8*N+7][pin8*N+6][pin8*N+5][pin8*N+4][pin8*N+3][pin8*N+2][pin8*N+1][pin8*N]
```

### Pulse Timing Register Format
Each 32-bit register holds 2 pin timings (16 bits each):
```
Register N: [pin2*N+1 timing][pin2*N timing]
Timing format: [15:8] = start count, [7:0] = end count
```

## Fast Pin Usage

gpio[128:159] are "fast pins" that bypass the BIM interface:

**Advantages:**
- 1 cycle latency (vs 2 cycles for BIM pins)
- Direct FPGA → external connection
- No BIM board required

**Use Cases:**
- Scope triggers
- External clock input/output
- Handshake signals with external equipment
- High-speed synchronization

**In RTL:**
```verilog
// Fast pins are controlled separately from FBC decoder
input wire [31:0] fast_dout,    // Direct drive values
input wire [31:0] fast_oen,     // Direct output enables
input wire        fast_clk_en,  // Enable for fast pins
output wire [31:0] fast_error   // Error flags for fast pins
```

## Package Pin Reference


## Facility Control (Pump / Valve / Heater)
**Location:** Quad Board Connector J3b
**Signals:**
- **Water Valve**: Digital Output (likely GPIO, active high)
- **Pump Flow**: Digital Inputs (Flow switches: Top, Mid, Bot)
- **Heater/Cooler**: Digital Outputs (PWM capable)

**Note:** Specific GPIO numbers for these are not explicitly named in RTL. They are likely mapped to specific pins within the generic `gpio[0:127]` or `gpio[128:139]` ranges. Can check project schematics.

## Host-Side I/O
**Temperature Logger (TC-08)**:
- **Interface**: USB (Host PC)
- **Hardware**: Pico Technology TC-08
- **Software**: Requires `usbtc08.dll` (Windows) or `libusbtc08` (Linux).
- **Not visible to Firmware**.
