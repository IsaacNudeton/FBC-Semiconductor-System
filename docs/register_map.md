# FBC Semiconductor System - Register Map

**Last Verified:** March 13, 2026  
**Source:** `firmware/src/regs.rs` (verified against actual code)  
**Status:** ✅ VERIFIED - Matches firmware/src/regs.rs exactly

---

## Memory Map Overview

| Peripheral    | Base Address | Size | Description                |
|---------------|--------------|------|----------------------------|
| FBC Control   | 0x40040000   | 4KB  | FBC decoder control        |
| Pin Control   | 0x40050000   | 4KB  | Pin type configuration     |
| Status        | 0x40060000   | 4KB  | Vector status & errors     |
| Freq Counter  | 0x40070000   | 4KB  | Frequency counters (8)     |
| Clock Ctrl    | 0x40080000   | 4KB  | Clock control (freq_sel)   |
| Error BRAM    | 0x40090000   | 4KB  | Error logging BRAMs (×3)   |

**Verified against:** `firmware/src/regs.rs` lines 11-16

---

## FBC Control (0x40040000)

Controls the FBC bytecode decoder.

| Offset | Name      | R/W | Reset    | Description                    |
|--------|-----------|-----|----------|--------------------------------|
| 0x00   | CTRL      | R/W | 0x00     | Control register               |
| 0x04   | STATUS    | RO  | 0x00     | Status register                |
| 0x08   | INSTR_LO  | RO  | 0x00     | Instruction count [31:0]       |
| 0x0C   | INSTR_HI  | RO  | 0x00     | Instruction count [63:32]      |
| 0x10   | CYCLE_LO  | RO  | 0x00     | Cycle count [31:0]             |
| 0x14   | CYCLE_HI  | RO  | 0x00     | Cycle count [63:32]            |
| 0x18   | ERROR     | RO  | 0x00     | Error information              |
| 0x1C   | VERSION   | RO  | 0x00010000 | Firmware version (v1.0.0)    |

### CTRL Register (0x00)

| Bit | Name           | Description                              |
|-----|----------------|------------------------------------------|
| 0   | enable         | 1 = Enable FBC decoder                   |
| 1   | reset          | 1 = Reset decoder (self-clearing)        |
| 2   | irq_en_done    | 1 = Enable interrupt on completion       |
| 3   | irq_en_error   | 1 = Enable interrupt on error            |
| 31:4| reserved       | Reserved                                 |

### STATUS Register (0x04)

| Bit | Name     | Description                                |
|-----|----------|--------------------------------------------|
| 0   | running  | 1 = Decoder is executing                   |
| 1   | done     | 1 = Program complete (HALT reached)        |
| 2   | error    | 1 = Decode error occurred                  |
| 31:3| reserved | Reserved                                   |

---

## Pin Control (0x40050000)

Configures pin types for 128 vector pins.

| Offset | Name          | R/W | Description                        |
|--------|---------------|-----|------------------------------------|
| 0x00   | PIN_TYPE[0]   | R/W | Pin types for pins 0-7             |
| 0x04   | PIN_TYPE[1]   | R/W | Pin types for pins 8-15            |
| ...    | ...           | ... | ...                                |
| 0x3C   | PIN_TYPE[15]  | R/W | Pin types for pins 120-127         |
| 0x40   | DELAY0        | R/W | Phase delay configuration 0        |
| 0x44   | DELAY1        | R/W | Phase delay configuration 1        |

### Pin Type Encoding (4 bits per pin)

| Value | Type           | Description                         |
|-------|----------------|-------------------------------------|
| 0x0   | BIDI           | Bidirectional (default)             |
| 0x1   | INPUT          | Input only                          |
| 0x2   | OUTPUT         | Output only                         |
| 0x3   | OPEN_C         | Open collector                      |
| 0x4   | PULSE          | Pulse output (+ve)                  |
| 0x5   | NPULSE         | Pulse output (-ve)                  |
| 0x6   | ERR_TRIG       | Error trigger output                |
| 0x7   | VEC_CLK        | Vector clock output                 |
| 0x8   | VEC_CLK_EN     | Vector clock enable output          |

### PIN_TYPE Register Bit Packing

Each 32-bit register holds 8 pin types:
```
[3:0]   = Pin N+0
[7:4]   = Pin N+1
[11:8]  = Pin N+2
[15:12] = Pin N+3
[19:16] = Pin N+4
[23:20] = Pin N+5
[27:24] = Pin N+6
[31:28] = Pin N+7
```

---

## Status (0x40060000)

Vector execution status and error information.

| Offset | Name             | R/W | Description                      |
|--------|------------------|-----|----------------------------------|
| 0x00   | ERROR_COUNT      | RO  | Total errors detected            |
| 0x04   | VECTOR_COUNT     | RO  | Total vectors executed           |
| 0x08   | CYCLE_COUNT_LO   | RO  | Total cycles [31:0]              |
| 0x0C   | CYCLE_COUNT_HI   | RO  | Total cycles [63:32]             |
| 0x10   | GAP_COUNT        | RO  | Gap count                        |
| 0x14   | STATUS           | RO  | Status flags                     |
| 0x18   | IRQ_ENABLE       | R/W | Interrupt enable                 |
| 0x3C   | FPGA_VERSION     | RO  | FPGA version                     |

### STATUS Register (0x14)

| Bit | Name              | Description                          |
|-----|-------------------|--------------------------------------|
| 0   | done              | 1 = Vector execution complete        |
| 1   | errors_detected   | 1 = At least one error occurred      |
| 31:2| reserved          | Reserved                             |

---

## Frequency Counter (0x40070000)

8 independent frequency counters for timing measurement.

Each counter occupies 32 bytes (8 registers):

| Counter | Base Offset |
|---------|-------------|
| 0       | 0x00        |
| 1       | 0x20        |
| 2       | 0x40        |
| 3       | 0x60        |
| 4       | 0x80        |
| 5       | 0xA0        |
| 6       | 0xC0        |
| 7       | 0xE0        |

### Per-Counter Registers

| Offset | Name            | R/W | Description                    |
|--------|-----------------|-----|--------------------------------|
| +0x00  | CR              | R/W | Control register               |
| +0x04  | SR              | RO  | Status register                |
| +0x08  | MAX_CYCLE       | R/W | Target cycle count             |
| +0x0C  | MAX_TIME        | R/W | Maximum time count             |
| +0x10  | CYCLE_COUNT     | RO  | Current cycle count            |
| +0x14  | TIME_COUNT      | RO  | Current time count             |
| +0x18  | MAX_TIMEOUT     | R/W | Timeout threshold              |
| +0x1C  | (reserved)      | -   | Reserved                       |

### CR (Control Register)

| Bits   | Name        | Description                          |
|--------|-------------|--------------------------------------|
| 0      | enable      | 1 = Enable counter                   |
| 1      | irq_en      | 1 = Enable interrupt on done         |
| 7:2    | reserved    | Reserved                             |
| 15:8   | sig_sel     | Signal input pin select              |
| 23:16  | trig_sel    | Trigger input pin select             |
| 31:24  | reserved    | Reserved                             |

### SR (Status Register)

| Bits   | Name              | Description                      |
|--------|-------------------|----------------------------------|
| 0      | done              | 1 = Measurement complete         |
| 1      | idle              | 1 = Counter idle                 |
| 2      | waiting           | 1 = Waiting for trigger          |
| 3      | running           | 1 = Counter running              |
| 4      | irq_en_shadow     | Shadow of irq_en                 |
| 5      | invalid_test      | 1 = Invalid measurement          |
| 6      | sig_timeout       | 1 = Signal timeout               |
| 7      | timeout           | 1 = Timeout error                |
| 15:8   | sig_sel_shadow    | Shadow of sig_sel                |
| 23:16  | trig_sel_shadow   | Shadow of trig_sel               |
| 31:24  | reserved          | Reserved                         |

### Pin Select Values

| Value     | Description                |
|-----------|----------------------------|
| 0-127     | Vector pin 0-127           |
| 128-159   | Extra GPIO 0-31            |
| 0xFE      | Immediate trigger          |
| 0xFF      | Disabled                   |

---

## FBC Opcodes Reference

| Opcode | Hex  | Description                              |
|--------|------|------------------------------------------|
| NOP    | 0x00 | No operation                             |
| HALT   | 0xFF | End of program                           |
| LOOP_N | 0xB0 | Loop next block N times                  |
| PATTERN_REP | 0xB5 | Repeat current pattern N times      |
| PATTERN_SEQ | 0xB6 | Generate sequence                    |
| SET_PINS | 0xC0 | Set pin values (128-bit payload)       |
| SET_OEN  | 0xC1 | Set output enables (128-bit payload)   |
| SET_BOTH | 0xC2 | Set pins and OEN (256-bit payload)     |
| WAIT     | 0xD0 | Wait N cycles                          |
| SYNC     | 0xD1 | Wait for external trigger              |

### Instruction Format

```
64-bit instruction word:
┌────────┬────────┬────────────────────────────────────────────┐
│ opcode │ flags  │              operand                       │
│ [63:56]│ [55:48]│              [47:0]                        │
└────────┴────────┴────────────────────────────────────────────┘

Flags:
  [0] = LAST  - Last instruction in block
  [1] = IRQ   - Generate interrupt after
  [2] = LOOP  - Part of loop body
```
