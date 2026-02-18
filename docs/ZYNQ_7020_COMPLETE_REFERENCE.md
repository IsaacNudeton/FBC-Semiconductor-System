# ZYNQ 7020 ARM Controller - Complete Technical Reference

> **Purpose:** Enable full understanding and modification of ZYNQ 7020-based burn-in controllers
> **Hardware:** ~500 controllers available | Part: xc7z020clg484-1 / xc7z020clg400-1

---

## !! CRITICAL: TWO SYSTEM VERSIONS !!

```
+===========================================================================+
|                    READ THIS FIRST - SYSTEM VERSIONS                      |
+===========================================================================+

  +========================================+================================+
  ||      CURRENT SYSTEM (ACTIVE)         ||    FBC (IN DEVELOPMENT)      ||
  ||      ** USE THIS FOR CHANGES **      ||    ** NOT ON HARDWARE **     ||
  +========================================+================================+
  ||                                      ||                              ||
  || Platform:   Embedded Linux           || Platform:   Bare-metal Rust  ||
  || Boot time:  ~30 seconds              || Boot time:  <1 second        ||
  || Project:    kzhang_v2 (Vivado)       || Project:    FBC Semiconductor||
  || Interface:  SSH + linux_*.elf        || Interface:  TCP binary       ||
  || Main app:   HPBI.elf                 || Main app:   firmware.elf     ||
  ||                                      ||                              ||
  || Files at:                            || Files at:                    ||
  ||   OneDrive/.../Volt/kzhang_v2/       ||   C:\Dev\projects\FBC...     ||
  ||   OneDrive/.../Volt 4/ (API docs)    ||                              ||
  ||                                      ||                              ||
  +========================================+================================+

  TO MODIFY THE ~500 EXISTING CONTROLLERS:
    --> Use "CURRENT SYSTEM" information (Sections marked [CURRENT])
    --> SSH to controller, use linux_*.elf commands
    --> Modify kzhang_v2 Vivado project for FPGA changes

  THE FBC IMPLEMENTATION:
    --> Is a NEW design, not yet deployed to any hardware
    --> Shown for reference/future use (Sections marked [FBC])
    --> Ignore for now if you're modifying existing controllers

+===========================================================================+
```

---

## TABLE OF CONTENTS

1. [System Architecture Overview](#1-system-architecture-overview) - Hardware (shared)
2. [Hardware Block Diagram](#2-hardware-block-diagram) - Hardware (shared)
3. [Memory Maps](#3-memory-maps) - Mostly shared, some differences noted
4. [ARM Processing System](#4-arm-processing-system) - [CURRENT] Linux boot
5. [FPGA Programmable Logic](#5-fpga-programmable-logic) - [CURRENT] + [FBC]
6. [Pin Configuration](#6-pin-configuration) - Shared
7. [Firmware Layer](#7-firmware-layer) - [CURRENT] Linux API
8. [Communication Protocol](#8-communication-protocol) - [CURRENT] SSH, [FBC] TCP
9. [Vector/Pattern System](#9-vectorpattern-system) - [CURRENT] tools
10. [How To Modify Each Component](#10-how-to-modify-each-component) - [CURRENT] focus

---

## 1. SYSTEM ARCHITECTURE OVERVIEW

```
+============================================================================+
|                        ZYNQ 7020 SYSTEM-ON-CHIP                            |
+============================================================================+
|                                                                            |
|  +----------------------------------+  +--------------------------------+  |
|  |    PROCESSING SYSTEM (PS)       |  |   PROGRAMMABLE LOGIC (PL)      |  |
|  |         ARM Cortex-A9           |  |         FPGA Fabric            |  |
|  +----------------------------------+  +--------------------------------+  |
|  |                                  |  |                                |  |
|  |  +----------+    +----------+   |  |  +----------+   +----------+   |  |
|  |  |  CPU 0   |    |  CPU 1   |   |  |  | Vector   |   | Freq     |   |  |
|  |  | 667 MHz  |    | 667 MHz  |   |  |  | Engine   |   | Counter  |   |  |
|  |  +----------+    +----------+   |  |  +----------+   +----------+   |  |
|  |        |              |         |  |       |              |         |  |
|  |  +-------------------------+    |  |  +-------------------------+   |  |
|  |  |    L2 Cache (512KB)    |    |  |  |    AXI Interconnect     |   |  |
|  |  +-------------------------+    |  |  +-------------------------+   |  |
|  |        |                        |  |       |                        |  |
|  |  +-------------------------+    |  |  +-------------------------+   |  |
|  |  |   DDR Controller       |    |  |  |   Pin Control Logic    |   |  |
|  |  |   (512MB DDR3)         |    |  |  |   (160 I/O pins)       |   |  |
|  |  +-------------------------+    |  |  +-------------------------+   |  |
|  |        |                        |  |       |                        |  |
|  |  +------------+  +----------+   |  |  +----------+   +----------+   |  |
|  |  | GEM (ETH)  |  | SD/MMC   |   |  |  | BRAM     |   | DSP48    |   |  |
|  |  | 10/100/1G  |  | Boot     |   |  |  | 4.9 Mb   |   | 220 slc  |   |  |
|  |  +------------+  +----------+   |  |  +----------+   +----------+   |  |
|  |                                  |  |                                |  |
|  +----------------------------------+  +--------------------------------+  |
|              |                                     |                       |
|              +------ AXI GP0/GP1 Interface --------+                       |
|                      (32-bit @ 150MHz)                                     |
+============================================================================+
                |                                    |
                v                                    v
        +---------------+                  +------------------+
        | ETHERNET      |                  | DUT INTERFACE    |
        | 172.16.0.xxx  |                  | 128 Vector Pins  |
        | TCP Port 3000 |                  | 32 GPIO Pins     |
        +---------------+                  +------------------+
```

### Key Specifications Table

| Component | Specification | Notes |
|-----------|--------------|-------|
| ARM Cores | 2x Cortex-A9 @ 667 MHz | Can run bare-metal or Linux |
| L1 Cache | 32KB I + 32KB D per core | |
| L2 Cache | 512 KB shared | |
| DDR3 | 512 MB | Configurable |
| OCM | 256 KB | On-chip, low latency |
| Logic Cells | 85,000 | |
| BRAM | 4.9 Mb (140x 36Kb blocks) | |
| DSP Slices | 220 | |
| I/O Pins | 200+ | Bank dependent |
| Package | CLG484 (original) / CLG400 (FBC) | |

---

## 2. HARDWARE BLOCK DIAGRAM

### 2.1 Controller Board Layout

```
+------------------------------------------------------------------+
|                     HPBI CONTROLLER BOARD                         |
+------------------------------------------------------------------+
|                                                                   |
|  +-------------+     +------------------+     +----------------+  |
|  | POWER INPUT |     |   ZYNQ 7020      |     | ETHERNET PHY   |  |
|  | 12V / 5V    |---->|   SoC Module     |---->| RJ45 Connector |  |
|  +-------------+     +------------------+     +----------------+  |
|                             |                                     |
|        +--------------------+--------------------+                |
|        |                    |                    |                |
|        v                    v                    v                |
|  +----------+        +------------+       +------------+          |
|  | SD CARD  |        | EXTERNAL   |       | EXTERNAL   |          |
|  | (Boot)   |        | ADC x16    |       | DAC x10    |          |
|  | /mnt     |        | MAX11129   |       | 0-4.096V   |          |
|  +----------+        +------------+       +------------+          |
|                             |                                     |
|        +--------------------+--------------------+                |
|        |                    |                    |                |
|        v                    v                    v                |
|  +----------+        +------------+       +------------+          |
|  | XADC x16 |        | PLL/CLOCK  |       | TEMP CTRL  |          |
|  | Internal |        | Generator  |       | NTC/Heater |          |
|  | 0-1V     |        | 1-200MHz   |       |            |          |
|  +----------+        +------------+       +------------+          |
|                                                                   |
|  +-------------------------------------------------------------+  |
|  |                    I/O CONNECTOR BANK                        |  |
|  |  GPIO[0:127] = Vector Pins (to DUT)                         |  |
|  |  GPIO[128:159] = Extra GPIO (triggers, status)              |  |
|  +-------------------------------------------------------------+  |
|                                                                   |
|  +-------------------------------------------------------------+  |
|  |                   POWER SUPPLY INTERFACE                     |  |
|  |  16x Peripheral Supplies (LC/HC via PMBus)                  |  |
|  |  4-6x Core Supplies (VICOR modules)                         |  |
|  +-------------------------------------------------------------+  |
+------------------------------------------------------------------+
```

### 2.2 Signal Flow Diagram

```
                    SIGNAL FLOW: HOST PC -> DUT
                    ============================

+----------+     TCP/IP      +------------+     AXI      +----------+
|  HOST    |---------------->|   ARM      |------------->|  FPGA    |
|  PC      |    Port 3000    |  (Linux)   |    GP0/GP1   |  Logic   |
| Everest  |<----------------|  HPBI.elf  |<-------------|          |
+----------+     Results     +------------+   Status     +----------+
                                   |                          |
                                   | SSH                      | GPIO
                                   | /mnt/*.elf               | Pins
                                   v                          v
                            +------------+            +-------------+
                            |  SD Card   |            |    DUT      |
                            | - Vectors  |            | (Device     |
                            | - Config   |            |  Under Test)|
                            | - Logs     |            +-------------+
                            +------------+
```

---

## 3. MEMORY MAPS

### 3.1 ARM (PS) Memory Map

```
+------------------+------------------+----------------------------------+
|   START ADDR     |    END ADDR      |          DESCRIPTION             |
+------------------+------------------+----------------------------------+
| 0x0000_0000      | 0x0001_FFFF      | Boot ROM (128 KB, read-only)     |
+------------------+------------------+----------------------------------+
| 0x0010_0000      | 0x1FFF_FFFF      | DDR3 SDRAM (511 MB usable)       |
|                  |                  |   0x0010_0000: Code start        |
|                  |                  |   Stack: 64KB, Heap: 64KB        |
+------------------+------------------+----------------------------------+
| 0x4000_0000      | 0x7FFF_FFFF      | PL (FPGA) via AXI GP0            |
|   0x4004_0000    |   0x4004_0FFF    |   FBC Decoder Control (4 KB)     |
|   0x4005_0000    |   0x4005_0FFF    |   Pin Configuration (4 KB)       |
|   0x4006_0000    |   0x4006_0FFF    |   Vector Status (4 KB)           |
|   0x4007_0000    |   0x4007_0FFF    |   Frequency Counters (4 KB)      |
|   0x4040_0000    |   0x4040_FFFF    |   AXI DMA Controller             |
+------------------+------------------+----------------------------------+
| 0x8000_0000      | 0xBFFF_FFFF      | PL (FPGA) via AXI GP1            |
+------------------+------------------+----------------------------------+
| 0xE000_0000      | 0xE02F_FFFF      | I/O Peripherals (IOP)            |
|   0xE000_0000    |   0xE000_0FFF    |   UART0                          |
|   0xE000_1000    |   0xE000_1FFF    |   UART1                          |
|   0xE000_4000    |   0xE000_4FFF    |   I2C0                           |
|   0xE000_5000    |   0xE000_5FFF    |   I2C1                           |
|   0xE000_A000    |   0xE000_AFFF    |   GPIO (MIO)                     |
|   0xE000_B000    |   0xE000_BFFF    |   GEM0 (Ethernet)                |
|   0xE000_C000    |   0xE000_CFFF    |   GEM1 (Ethernet)                |
|   0xE000_D000    |   0xE000_DFFF    |   QSPI                           |
|   0xE010_0000    |   0xE010_0FFF    |   SD/SDIO 0                      |
+------------------+------------------+----------------------------------+
| 0xF800_0000      | 0xF800_0BFF      | SLCR (System Level Control)      |
+------------------+------------------+----------------------------------+
| 0xFFFC_0000      | 0xFFFF_FFFF      | OCM (256 KB on-chip memory)      |
|                  |                  | Used for DMA buffers             |
+------------------+------------------+----------------------------------+
```

### 3.2 FPGA Register Map (AXI Peripherals)

```
FBC DECODER CONTROL (Base: 0x4004_0000)
========================================
+--------+----------+------+--------------------------------------------------+
| OFFSET |   NAME   | R/W  |                   DESCRIPTION                    |
+--------+----------+------+--------------------------------------------------+
| 0x00   | CTRL     | R/W  | [0]=Enable [1]=Reset [2]=IRQ_Enable              |
| 0x04   | STATUS   | RO   | [0]=Running [1]=Done [2]=Error                   |
| 0x08   | INSTR_LO | RO   | Instruction count [31:0]                         |
| 0x0C   | INSTR_HI | RO   | Instruction count [63:32]                        |
| 0x10   | CYCLE_LO | RO   | Cycle count [31:0]                               |
| 0x14   | CYCLE_HI | RO   | Cycle count [63:32]                              |
| 0x18   | ERROR    | RO   | Error code if STATUS.Error=1                     |
| 0x1C   | VERSION  | RO   | Firmware version (0x00010000 = v1.0.0)           |
+--------+----------+------+--------------------------------------------------+

PIN CONFIGURATION (Base: 0x4005_0000)
======================================
+--------+------------+------+------------------------------------------------+
| OFFSET |    NAME    | R/W  |                  DESCRIPTION                   |
+--------+------------+------+------------------------------------------------+
| 0x00   | PIN_TYPE_0 | R/W  | Pin types for GPIO[7:0], 4 bits each           |
| 0x04   | PIN_TYPE_1 | R/W  | Pin types for GPIO[15:8]                       |
| ...    | ...        | ...  | ... (8 pins per register)                      |
| 0x3C   | PIN_TYPE_15| R/W  | Pin types for GPIO[127:120]                    |
| 0x40   | PIN_TYPE_16| R/W  | Pin types for GPIO[135:128] (extra GPIO)       |
| ...    | ...        | ...  |                                                |
| 0x4C   | PIN_TYPE_19| R/W  | Pin types for GPIO[159:152]                    |
+--------+------------+------+------------------------------------------------+

Pin Type Encoding (4 bits per pin):
  0 = BIDIR       (Bidirectional)
  1 = INPUT       (Controller receives from DUT)
  2 = OUTPUT      (Controller drives to DUT)
  3 = OPEN_C      (Open Collector)
  4 = PULSE       (Positive pulse clock)
  5 = NPULSE      (Negative pulse clock)
  6 = ERROR_TRIG  (Error trigger - unused)
  7 = VECTOR_CLK  (Vector clock output)

VECTOR STATUS (Base: 0x4006_0000)
==================================
+--------+-------------+------+-----------------------------------------------+
| OFFSET |    NAME     | R/W  |                 DESCRIPTION                   |
+--------+-------------+------+-----------------------------------------------+
| 0x00   | VEC_CTRL    | R/W  | [0]=Start [1]=Stop [2]=Continuous            |
| 0x04   | VEC_STATUS  | RO   | [0]=Running [1]=Done [7:4]=Error_Code        |
| 0x08   | ERROR_COUNT | RO   | Number of vector compare errors              |
| 0x0C   | LOOP_COUNT  | R/W  | Number of times to repeat pattern            |
| 0x10   | VEC_ADDR    | RO   | Current vector address in BRAM               |
+--------+-------------+------+-----------------------------------------------+

FREQUENCY COUNTERS (Base: 0x4007_0000)
=======================================
8 independent frequency counters, 32 bytes each:

+--------+-------------+------+-----------------------------------------------+
| OFFSET |    NAME     | R/W  |                 DESCRIPTION                   |
+--------+-------------+------+-----------------------------------------------+
| 0x00   | FREQ0_CTRL  | R/W  | [7:0]=Pin_Number [8]=Enable [9]=Reset        |
| 0x04   | FREQ0_COUNT | RO   | Measured frequency count                     |
| 0x08   | FREQ0_PERIOD| RO   | Measured period (clock cycles)               |
| 0x0C   | FREQ0_RESV  | --   | Reserved                                     |
| 0x10   | FREQ0_HZ_LO | RO   | Calculated Hz [31:0]                         |
| 0x14   | FREQ0_HZ_HI | RO   | Calculated Hz [63:32]                        |
| 0x18-1F| Reserved    | --   |                                              |
+--------+-------------+------+-----------------------------------------------+
| 0x20   | FREQ1_*     | ...  | Counter 1 (same structure)                   |
| 0x40   | FREQ2_*     | ...  | Counter 2                                    |
| ...    | ...         | ...  | ...                                          |
| 0xE0   | FREQ7_*     | ...  | Counter 7                                    |
+--------+-------------+------+-----------------------------------------------+
```

---

## 4. ARM PROCESSING SYSTEM [CURRENT SYSTEM]

> **This section describes the CURRENT Linux-based system running on the ~500 controllers.**
> For the FBC bare-metal approach (not yet deployed), see Section 5.2.

### 4.1 Boot Sequence [CURRENT]

```
BOOT SEQUENCE DIAGRAM (CURRENT LINUX SYSTEM)
=============================================

  Power On
     |
     v
+--------------------+
| 1. BootROM         |  Fixed in silicon, cannot modify
|    (0x0000_0000)   |  Reads boot mode pins (MIO[8:2])
+--------------------+
     |
     | Boot Mode: SD Card (typical)
     v
+--------------------+
| 2. FSBL            |  First Stage Boot Loader
|    (SD: BOOT.BIN)  |  - Initializes DDR3
|                    |  - Programs FPGA bitstream
|                    |  - Loads next stage
+--------------------+
     |
     v
+--------------------+
| 3. U-Boot OR       |  Second stage (Linux path)
|    Bare Metal App  |  OR direct bare-metal (FBC path)
+--------------------+
     |
     v (Linux path)
+--------------------+
| 4. Linux Kernel    |  Embedded Linux
|    + Root FS       |  - /mnt = SD card mount
|                    |  - *.elf executables
+--------------------+
     |
     v
+--------------------+
| 5. HPBI.elf        |  Main application
|    (User Space)    |  Runs burn-in tests
+--------------------+
```

### 4.2 Linux Filesystem Structure (SD Card)

```
/mnt/  (SD Card Root)
  |
  +-- BOOT.BIN              # FSBL + Bitstream + U-Boot
  +-- image.ub              # Linux kernel + device tree
  +-- init.sh               # Startup script (sets MAC address)
  +-- mount.sh              # NFS mount helper
  |
  +-- HPBI.elf              # Main burn-in controller
  +-- linux_pin_type.elf    # Pin configuration
  +-- linux_Allpin_type.elf # Bulk pin config
  +-- linux_Pulse_Delays.elf# Pulse timing
  +-- linux_load_vectors.elf# Load patterns
  +-- linux_run_vector.elf  # Execute patterns
  +-- linux_xpll_frequency.elf # PLL config
  +-- linux_XADC.elf        # Internal ADC read
  +-- linux_EXT_ADC.elf     # External ADC read
  +-- linux_EXT_DAC.elf     # DAC output control
  +-- linux_pmbus_*.elf     # Power supply control
  +-- linux_VICOR.elf       # Core power control
  +-- linux_set_temperature.elf # Temp setpoint
  +-- linux_freq_counter.elf# Frequency measurement
  +-- [other utilities]
  |
  +-- home/
       +-- [device_name]/
            +-- vectors/    # Binary vector files (.hex)
            +-- timing/     # Timing files
            +-- [device].map# Pin mapping
            +-- [device].lvl# Level settings
            +-- [device].tp # Test plan
```

---

## 5. FPGA PROGRAMMABLE LOGIC

### 5.1 RTL Module Hierarchy [CURRENT SYSTEM]

> **CURRENT SYSTEM:** The kzhang_v2 Vivado project is what's deployed on hardware.
> **Location:** `OneDrive/.../Volt/kzhang_v2/kzhang_v2.srcs/`

```
FPGA TOP-LEVEL HIERARCHY (CURRENT - kzhang_v2)
===============================================

top.v
  |
  +-- design_1_wrapper.v          # Vivado block design wrapper
  |     |
  |     +-- processing_system7    # Zynq PS (ARM) hard IP
  |     |     +-- M_AXI_GP0       # AXI Master to PL
  |     |     +-- FCLK_CLK0       # 100 MHz fabric clock
  |     |     +-- FCLK_RESET0_N   # Fabric reset
  |     |
  |     +-- axi_interconnect      # Routes AXI to peripherals
  |
  +-- axi_io_table.v              # Pin configuration
  |     +-- io_table.v            # Pin type storage
  |
  +-- axi_vector_status.v         # Vector execution status
  |
  +-- axi_freq_counter.v          # Frequency measurement
  |
  +-- axi_pulse_ctrl.v            # Pulse/clock generation
  |
  +-- GPIO pads                   # Physical I/O
        +-- gpio[127:0]           # Vector pins to DUT
        +-- gpio[159:128]         # Extra GPIO

Key source files (CURRENT):
  - axi_slave.v / axi_slave.vh    # AXI interface
  - io_table.v                    # Pin type storage
  - axi_io_table.v                # AXI wrapper for io_table
  - axi_vector_status.v           # Vector execution status
  - axi_freq_counter.v            # Frequency counter
  - axi_pulse_ctrl.v              # Pulse generation
  - vector.vh                     # Vector width definitions
  - top.v                         # Top-level wrapper
```

---

### 5.2 FBC Decoder State Machine [FBC - NOT ON HARDWARE]

> **WARNING: This section describes the NEW FBC design that is NOT yet deployed.**
> **Skip this section if you're modifying current controllers.**
> **Location:** `C:\Dev\projects\FBC Semiconductor System\rtl\`

```
FBC DECODER FSM
===============

         +-------+
         | IDLE  |<-----------------------------+
         +-------+                              |
             |                                  |
             | start=1                          |
             v                                  |
         +--------+                             |
         | FETCH  |  Read 64-bit instruction    |
         +--------+  from AXI Stream            |
             |                                  |
             | instr_valid                      |
             v                                  |
         +--------+                             |
         | DECODE |  Parse opcode & operands    |
         +--------+                             |
             |                                  |
             +--------+--------+--------+       |
             |        |        |        |       |
             v        v        v        v       |
        +------+ +------+ +------+ +------+     |
        | NOP  | | SET  | | WAIT | | LOOP |     |
        |      | | PINS | |      | |      |     |
        +------+ +------+ +------+ +------+     |
             |        |        |        |       |
             v        v        v        v       |
         +--------+                             |
         |EXECUTE |  Apply to hardware          |
         +--------+                             |
             |                                  |
             | done OR wait_cycles=0            |
             v                                  |
         +--------+     last_instr=1            |
         |WAIT_OUT|--------------------------->+
         +--------+
             |
             | last_instr=0
             +-----> (back to FETCH)


SUPPORTED OPCODES:
==================
+--------+------+--------------------------------------------------+
| OPCODE | HEX  |                   DESCRIPTION                    |
+--------+------+--------------------------------------------------+
| NOP    | 0x00 | No operation, advance to next instruction        |
| HALT   | 0xFF | Stop execution, set done flag                    |
| LOOP_N | 0xB0 | Loop next N instructions, operand = count        |
| PAT_REP| 0xB5 | Repeat pattern N times                           |
| PAT_SEQ| 0xB6 | Generate counting sequence pattern               |
| SET_PIN| 0xC0 | Set 128 pin values (+ 128-bit payload)           |
| SET_OEN| 0xC1 | Set 128 output enables (+ 128-bit payload)       |
| SET_BTH| 0xC2 | Set pins + OEN (+ 256-bit payload)               |
| WAIT   | 0xD0 | Wait N clock cycles, operand = count             |
| SYNC   | 0xD1 | Wait for external trigger signal                 |
+--------+------+--------------------------------------------------+

INSTRUCTION FORMAT (64-bit base):
=================================
  63      56 55      48 47                              0
  +--------+----------+--------------------------------+
  | OPCODE |  FLAGS   |           OPERAND             |
  +--------+----------+--------------------------------+

  FLAGS:
    [0] = LAST  - Last instruction in program
    [1] = IRQ   - Generate interrupt after execution
    [2] = LOOP  - Part of loop body

EXTENDED INSTRUCTIONS (SET_PINS, SET_OEN):
==========================================
  Bytes 0-7:   Header (opcode + flags + reserved)
  Bytes 8-23:  128-bit pin data (for SET_PINS or SET_OEN)

  SET_BOTH uses 256-bit payload:
  Bytes 0-7:   Header
  Bytes 8-23:  128-bit pin values
  Bytes 24-39: 128-bit output enables
```

### 5.3 Vector Engine Data Flow

```
VECTOR ENGINE DATA PATH
=======================

                    +----------------+
                    | Pattern Memory |
                    |    (BRAM)      |
                    | 64KB vectors   |
                    +----------------+
                           |
                           | 64-bit instructions
                           v
                    +----------------+
                    | FBC Decoder    |
                    | (State Machine)|
                    +----------------+
                           |
              +------------+------------+
              |                         |
              v                         v
      +---------------+         +---------------+
      | Pin Values    |         | Output Enable |
      | [127:0]       |         | [127:0]       |
      +---------------+         +---------------+
              |                         |
              v                         v
      +-------------------------------------------+
      |              PIN DRIVER                   |
      |  for each pin i in 0..127:                |
      |    if (oen[i] == 1)                       |
      |      pad[i] <= pin_val[i]  // drive       |
      |    else                                   |
      |      pad[i] <= 'Z'         // tri-state   |
      +-------------------------------------------+
                           |
                           v
      +-------------------------------------------+
      |           COMPARE LOGIC                   |
      |  for each pin i where type[i]==INPUT:     |
      |    expected[i] vs actual[i]               |
      |    if mismatch: error_count++             |
      +-------------------------------------------+
                           |
                           v
      +-------------------------------------------+
      |            ERROR BRAM                     |
      |  Log: cycle_number, pin_mask, expected,   |
      |       actual                              |
      +-------------------------------------------+
```

---

## 6. PIN CONFIGURATION

### 6.1 Pin Type Definitions

```
PIN TYPES AND BEHAVIOR
======================

+------+------------+-----+-----+------------------------------------------+
| CODE |    NAME    | DIR | OEN |              BEHAVIOR                    |
+------+------------+-----+-----+------------------------------------------+
|  0   | BIDIR      | I/O | Var | Bidirectional - direction per vector    |
|  1   | INPUT      | IN  |  0  | Always input, compare DUT output        |
|  2   | OUTPUT     | OUT |  1  | Always output, drive to DUT input       |
|  3   | OPEN_C     | OUT |  1  | Open collector - only drives low        |
|  4   | PULSE      | OUT |  1  | Positive clock pulse (rise/fall edges)  |
|  5   | NPULSE     | OUT |  1  | Negative clock pulse (fall/rise edges)  |
|  6   | ERROR_TRIG | IN  |  0  | Trigger on DUT error signal (unused)    |
|  7   | VECTOR_CLK | OUT |  1  | Free-running vector clock output        |
+------+------------+-----+-----+------------------------------------------+

PULSE TIMING DIAGRAM:
=====================

PULSE (Type 4):
              Rise Delay    Fall Delay
                 |<-->|      |<-->|
    ___________  +----+      +----+  ___________
               | |    |      |    | |
               +-+    +------+    +-+
               ^      ^           ^
            vec_clk   |        vec_clk
                   pulse_high

NPULSE (Type 5):
    -----------+      +------+      +-----------
               |      |      |      |
               +------+      +------+
               ^             ^
            vec_clk       vec_clk
```

### 6.2 Pin Mapping (Hardware to Software)

```
PIN MAPPING STRUCTURE
=====================

Controller Pin <--> DUT Pin Mapping is defined in .map file:

Example .map file format:
-------------------------
# Tester_Channel    DUT_Pin;
B13_GPIO0           TDI;
B13_GPIO1           TDO;
B13_GPIO2           TCK;
B13_GPIO3           TMS;
B13_GPIO4           TRST_N;
B13_GPIO5           RESET_N;
...
B13_GPIO127         DATA_OUT_127;

Physical Pin Banks:
-------------------
+------------+------------------+------------------+
|   BANK     |    GPIO RANGE    |    VOLTAGE       |
+------------+------------------+------------------+
| Bank 13    | GPIO[0:31]       | 3.3V (LVCMOS33)  |
| Bank 33    | GPIO[32:63]      | 3.3V (LVCMOS33)  |
| Bank 34    | GPIO[64:95]      | 3.3V (LVCMOS33)  |
| Bank 35    | GPIO[96:127]     | 3.3V (LVCMOS33)  |
| Bank 12    | GPIO[128:159]    | 3.3V (Extra)     |
+------------+------------------+------------------+

VIH/VIL Configuration:
----------------------
4 VIH groups, configurable via linux_IO_PS.elf:
  - Group 1: GPIO[0:31]
  - Group 2: GPIO[32:63]
  - Group 3: GPIO[64:95]
  - Group 4: GPIO[96:127]

VIH range: 0.8V to 3.6V (set per group)
```

---

## 7. FIRMWARE LAYER [CURRENT SYSTEM]

> **This is the CURRENT firmware API running on all ~500 controllers.**
> **Use these commands via SSH to control the existing hardware.**
> **Reference:** `OneDrive/.../Volt 4/SONOMA_FIRMWARE_API_SPECIFICATIONS.md`

### 7.1 Firmware Command Reference (Complete) [CURRENT]

```
FIRMWARE API - COMPLETE COMMAND REFERENCE (ACTIVE ON HARDWARE)
===============================================================

MAIN EXECUTABLE:
----------------
/mnt/HPBI.elf <testProgramFile> <serial> <rack> <tray> <bim>
  - Runs complete burn-in test program
  - Invokes other commands internally
  - Runs until test duration expires

PIN CONFIGURATION:
------------------
/mnt/linux_pin_type.elf <pinNum> <pinType>
  - pinNum: 0-127 (vector pins) or 128-159 (extra GPIO)
  - pinType: 0=BIDIR, 1=INPUT, 2=OUTPUT, 3=OPEN_C,
             4=PULSE, 5=NPULSE, 6=ERROR_TRIG, 7=VECTOR_CLK

/mnt/linux_Allpin_type.elf <pinStateFile>
  - Bulk configure all pins from .pinstate file
  - File format: "D0   B13_GPIO0: 0 L 1 H X"

/mnt/linux_Pulse_Delays.elf <pinNum> <pinType> <riseNS> <fallNS> <freq>
  - Configure pulse timing for PULSE/NPULSE pins
  - freq: Always use 200 (200 MHz reference)

CLOCK/PLL CONTROL:
------------------
/mnt/linux_xpll_frequency.elf <clkNum> <frequency> <phase> <dutyCycle>
  - clkNum: 1=vector, 2=pulse, 3-5=system clocks
  - frequency: Hz value
  - phase: degrees (0-360)
  - dutyCycle: percentage (e.g., 50)

/mnt/linux_xpll_off_on.elf <sysClk1> <sysClk2> <sysClk3> <sysClk4>
  - Enable/disable system clocks (0=OFF, 1=ON)

VECTOR OPERATIONS:
------------------
/mnt/linux_load_vectors.elf <seqFile> <hexFile>
  - seqFile: Pattern sequence file (.seq)
  - hexFile: Binary vector data (.hex)

/mnt/linux_run_vector.elf <patNum> <enFreq> <patName> <logErr> <enADC> <vecTime>
  - patNum: Pattern number in sequence
  - enFreq: Enable frequency measurement (0/1)
  - patName: Pattern name string
  - logErr: Enable error logging (0/1)
  - enADC: Enable ADC sampling (0/1)
  - vecTime: Run time in minutes

FREQUENCY MEASUREMENT:
----------------------
/mnt/linux_freq_counter.elf <triggerPin> <signalPin1> [signalPin2..7]
  - Measure frequency of up to 8 DUT output pins
  - triggerPin: Usually 128

/mnt/linux_start_freq_counter.elf <triggerPin> <signalPins...>
  - Start continuous frequency measurement

/mnt/linux_report_freq_counter.elf
  - Read frequency measurement results

ADC OPERATIONS:
---------------
/mnt/linux_init_XADC.elf
  - Initialize internal FPGA ADCs (16 channels)

/mnt/linux_XADC.elf [samplesNum]
  - Read all 16 internal ADCs
  - Range: 0-1V differential
  - ~20ns per sample

/mnt/linux_EXT_ADC.elf [samplesNum]
  - Read all 16 external ADCs (MAX11129)
  - Range: 0-3V

DAC OPERATIONS:
---------------
/mnt/linux_EXT_DAC.elf <v1> <v2> ... <v10>
  - Set all 10 DAC outputs
  - Range: 0-4.096V

/mnt/linux_EXT_DAC_singleCh.elf <channel> <voltage>
  - Set single DAC channel (1-10)

I/O LEVELS:
-----------
/mnt/linux_IO_PS.elf <vih1> <vih2> <vih3> <vih4>
  - Set VIH reference voltage per pin group
  - Range: 0.8V - 3.6V

POWER SUPPLY CONTROL:
---------------------
/mnt/linux_pmbus_PicoDlynx.elf <psNum> <voltage>
  - Turn on and set LC/HC supply (0-15)

/mnt/linux_pmbus_OFF.elf <psNum>
  - Turn off LC/HC supply

/mnt/linux_read_PicoDlynx.elf <psNum> [samples]
  - Read supply voltage and current

/mnt/linux_VICOR.elf <voltage> [mioNum] [dacNum]
  - Set core power supply voltage

TEMPERATURE CONTROL:
--------------------
/mnt/linux_set_temperature.elf <setpoint> <NTC_value>
  - Set case temperature setpoint (Celsius)

/mnt/linux_log_temperature.elf <NTC_value> <XADC_channel>
  - Read temperature sensor

/mnt/linuxLinTempDiode.elf <setpoint> <coolerDAC> <heaterDAC>
  - Configure linear temp diode control

/mnt/ReadLinTempDiode.elf <ADC_channel>
  - Read linear temp diode

UTILITY:
--------
/mnt/linux_dut_present.elf
  - Check if DUT is inserted (returns 0=present, -1=absent)

/mnt/linux_EEPROM.elf [eepromFile]
  - Read/write BIM EEPROM (serial number, etc.)
  - WARNING: Writing can corrupt BIM data
```

### 7.2 Test Plan File Format (.tp)

```
TEST PLAN STRUCTURE
===================

#----- HEADER SECTION -----#
TEST_PROGRAM_NAME = MyDevice_HTOL
TEST_DURATION = 168.0                    # Hours
DEVICE_DIR = /home/avagobi/MyDevice
ADC_SAMPLE_PERIOD = 30                   # Seconds
PIN_MAP_FILE = /home/avagobi/MyDevice/MyDevice.map
SETUP_FILE = /home/avagobi/MyDevice/MyDevice.lvl

#----- TEST STEPS -----#
TEST_STEPS START :

TEST_STEP : POWER_UP{
    VECTOR_FILE = N/A
    TIMING_FILE = SetClocks.sh
    SETPOINT_TEMP = 25
    TEMP_UL = 150
    TEMP_LL = -40
    TEMP_RAMP_RATE = 5
    REPEAT_COUNT = 1
    MASTER_PERIOD = 50.0                 # ns
    DELAY_CYC = 0                        # Use 5 to start timer
    VECTOR_TIME = 0:
    PU_LIST = <power_supply_config>
    PD_LIST = Nolist
}

TEST_STEP : FUNCTIONAL_125C{
    VECTOR_FILE = pattern1:pattern2:pattern3:
    TIMING_FILE = timing/functional.tim
    SETPOINT_TEMP = 125
    TEMP_UL = 150
    TEMP_LL = 100
    TEMP_RAMP_RATE = 10
    REPEAT_COUNT = 1000
    MASTER_PERIOD = 10.0
    DELAY_CYC = 5                        # Start loop here
    VECTOR_TIME = 60:30:30:              # Minutes per pattern
    PU_LIST = <power_config>
    PD_LIST = Nolist
}

TEST_STEP : POWER_DOWN{
    VECTOR_FILE = N/A
    TIMING_FILE = PowerOff.sh
    SETPOINT_TEMP = 25
    ...
}

TEST_STEPS END :

#----- ADC MONITORING -----#
ADC_MONITOR_SETUPS START :

ADC : VDD_CORE{
    UPPER_LIMIT = 1.10
    LOWER_LIMIT = 0.90
    UNITS = V
    SHUTDOWN_LOWER_LIMIT = 0.80
    SHUTDOWN_UPPER_LIMIT = 1.20
    DATA_COLLECT = Avg
}

ADC : VDD_CORE_I{
    UPPER_LIMIT = 5.0
    LOWER_LIMIT = 0.1
    UNITS = A
    SHUTDOWN_LOWER_LIMIT = -0.1
    SHUTDOWN_UPPER_LIMIT = 10.0
    DATA_COLLECT = Max
}

ADC_MONITOR_SETUPS END :
```

---

## 8. COMMUNICATION PROTOCOL

> **Network topology is shared. SSH access is CURRENT. TCP binary is FBC (not deployed).**

### 8.1 Network Architecture [SHARED]

```
NETWORK TOPOLOGY
================

                          +------------------+
                          |    HOST PC       |
                          |  172.16.0.49     |
                          |  (Everest SW)    |
                          +--------+---------+
                                   |
                                   | Gigabit Ethernet
                                   v
                          +------------------+
                          | Cisco 4948       |
                          | Ethernet Switch  |
                          | DHCP Server      |
                          +--------+---------+
                                   |
          +------------------------+------------------------+
          |            |           |           |            |
          v            v           v           v            v
    +---------+  +---------+  +---------+  +---------+  +---------+
    |Controller|  |Controller|  |Controller|  |Controller|  |   ...   |
    |172.16.0. |  |172.16.0. |  |172.16.0. |  |172.16.0. |  |         |
    |   101    |  |   102    |  |   103    |  |   104    |  |   248   |
    +---------+  +---------+  +---------+  +---------+  +---------+
         |            |           |           |
         v            v           v           v
       [BIM]        [BIM]       [BIM]       [BIM]
       [DUT]        [DUT]       [DUT]       [DUT]

IP ADDRESS ASSIGNMENT:
======================
Host PC:           172.16.0.49
Front Tray Range:  172.16.0.101 - 172.16.0.148
Back Tray Range:   172.16.0.201 - 172.16.0.248
Max Controllers:   96 (11 racks x 2 trays x 4 BIMs = 88 typical)
```

### 8.2 SSH Access [CURRENT SYSTEM]

```
SSH CONNECTION (USE THIS FOR CURRENT CONTROLLERS)
==============

# Connect to controller
ssh root@172.16.0.xxx

# Default credentials
Username: root
Password: root

# After login, you see:
zynq>

# Navigate to executables
cd /mnt

# List available commands
ls linux*.elf

# Example session:
zynq> /mnt/linux_XADC.elf
0.0515 0.2786 0.2549 0.2282 0.3436 0.3094 0.2740 0.2451 ...

zynq> /mnt/linux_dut_present.elf
0
```

### 8.3 Raw Ethernet FBC Protocol [IMPLEMENTED]

> **✅ This protocol is IMPLEMENTED in bare-metal firmware.**
> **Replaces TCP/IP with raw Ethernet for zero-latency control.**

```
FBC RAW ETHERNET PROTOCOL (IMPLEMENTED)
========================================

EtherType: 0x88B5 (custom FBC protocol)
Magic:     0xFBC0 (validates FBC packets)
Transport: Raw Ethernet (no TCP/IP stack)

ETHERNET FRAME STRUCTURE:
-------------------------
+----------------------+  \
| Dst MAC (6 bytes)    |   |
| Src MAC (6 bytes)    |   | Standard Ethernet Header
| EtherType: 0x88B5    |   | (14 bytes)
+----------------------+  /
| FBC Header (8 bytes) |  \
|   Magic: 0xFBC0      |   |
|   SeqNum: u16        |   | FBC Protocol Header
|   Cmd: u8            |   |
|   Flags: u8          |   |
|   Length: u16        |   |
+----------------------+  /
| Payload (0-1478)     |   | Command-specific data
+----------------------+

FBC HEADER (8 bytes):
---------------------
+--------+--------+--------+--------+
| Magic  | SeqNum | Cmd    | Flags  |
| 2 bytes| 2 bytes| 1 byte | 1 byte |
+--------+--------+--------+--------+
| Length (16-bit)          |
| 2 bytes                  |
+--------------------------+

SETUP PHASE COMMANDS:
---------------------
+------+----------------+------------------------------------------+
| CMD  |   NAME         |              DESCRIPTION                 |
+------+----------------+------------------------------------------+
| 0x01 | ANNOUNCE       | Controller → GUI (on boot, includes MAC) |
| 0x10 | BIM_STATUS_REQ | GUI → Controller (query BIM config)      |
| 0x11 | BIM_STATUS_RSP | Controller → GUI (BIM configuration)     |
| 0x20 | WRITE_BIM      | GUI → Controller (configure BIM EEPROM)  |
| 0x21 | UPLOAD_VECTORS | GUI → Controller (chunked vector upload) |
| 0x30 | CONFIGURE      | GUI → Controller (test parameters)       |
+------+----------------+------------------------------------------+

RUNTIME COMMANDS:
-----------------
+------+----------------+------------------------------------------+
| CMD  |   NAME         |              DESCRIPTION                 |
+------+----------------+------------------------------------------+
| 0x40 | START          | GUI → Controller (begin test execution)  |
| 0x41 | STOP           | GUI → Controller (halt execution)        |
| 0x42 | RESET          | GUI → Controller (reset to idle)         |
| 0x50 | HEARTBEAT      | Controller → GUI (periodic status)       |
| 0xE0 | ERROR          | Controller → GUI (error notification)    |
| 0xF0 | STATUS_REQ     | GUI → Controller (request status)        |
| 0xF1 | STATUS_RSP     | Controller → GUI (status response)       |
+------+----------------+------------------------------------------+

MAC ADDRESS ASSIGNMENT:
-----------------------
Each controller generates unique MAC from device DNA (eFUSE):
  MAC = OUI:DNA[47:0]
  OUI = 02:00:00 (locally administered, unicast)
  DNA = Zynq Device DNA (unique per chip)

Example:
  DNA  = 0x1234567890ABCDEF
  MAC  = 02:00:00:90:AB:EF

BENEFITS OF RAW ETHERNET:
--------------------------
✓ Zero TCP/IP overhead (<1ms latency)
✓ No socket buffer allocation
✓ No congestion control delays
✓ Deterministic packet timing
✓ Direct DMA to/from MAC buffers
✓ Switch handles routing (no ARP/DHCP needed)
```

**Implementation Files:**
- `firmware/src/net.rs` - GEM Ethernet MAC driver
- `firmware/src/fbc_protocol.rs` - FBC protocol handler
- `host/src/lib.rs` - Host-side FBC client
- `fpga-toolchain/src/gui.rs` - GUI integration

**Documentation:**
- `docs/FBC_PROTOCOL.md` - Complete protocol specification
- `docs/HAL_API.md` - Firmware API documentation

---

## 9. VECTOR/PATTERN SYSTEM [CURRENT SYSTEM]

> **These tools and formats work with the CURRENT deployed system.**
> **Reference:** `OneDrive/.../Volt 4/SONOMA_TOOLS_SPECIFICATIONS.md`

### 9.1 Vector Format Pipeline [CURRENT]

```
VECTOR CONVERSION PIPELINE
==========================

  Customer Source        Intermediate          Binary Output
  ===============        ============          =============

  +------------+        +------------+        +------------+
  | STIL File  |------->| ATP File   |------->| .hex File  |
  | (IEEE std) |  Parse | (ASCII)    | makePN | (Binary)   |
  +------------+        +------------+        +------------+
        |                     ^                     |
        v                     |                     v
  +------------+              |              +------------+
  | AVC File   |--------------+              | .seq File  |
  | (93K fmt)  |  Avc2Atp                    | (Sequence) |
  +------------+                             +------------+
        |                                          |
        v                                          v
  +------------+                             +------------+
  | MCC .vec   |                             | .pinstate  |
  | File       |                             | (Debug)    |
  +------------+                             +------------+


CONVERSION TOOLS:
=================
ParseSTIL08Sep2018.awk  : STIL -> ATP
Avc2Atp.awk             : AVC  -> ATP
MakeAtp.awk             : MCC .vec -> ATP
makePN14Nov2018.py      : ATP  -> .hex + .seq + .pinstate + .ascii
```

### 9.2 Vector File Formats

```
ATP FORMAT (Intermediate ASCII)
===============================

(                           # Pin list header
Pin1
Pin2
Pin3
...
)
{                           # Vector data block
> ts1 01lhx;                # Single vector, no repeat
repeat 9 > ts1 0lhxz;       # Vector repeats 10 times total
> ts2 11001100...;          # Different timing set
}

Character meanings:
  0 = Drive low
  1 = Drive high
  L = Expect low (input compare)
  H = Expect high (input compare)
  X = Don't care / tri-state
  Z = High impedance
  P = Positive clock pulse
  N = Negative clock pulse
  C = Clock (same as P)


.HEX FORMAT (Binary)
====================

Binary format generated by makePN14Nov2018.py:
- Packed bit vectors, 128 bits (16 bytes) per vector
- Repeat counts encoded in-line
- Total file contains: header + vector data

Header structure (varies by implementation):
  Offset 0x00: Magic number
  Offset 0x04: Vector count
  Offset 0x08: Pin count
  Offset 0x0C: Flags

Data section:
  Each vector = 16 bytes (128 pins)
  Bit ordering: pin[0] = byte[0] bit 0


.SEQ FORMAT (Sequence)
======================

Text file listing patterns and their memory locations:

0 1531 pattern_name_1.atp
1 3062 pattern_name_2.atp
2 4593 pattern_name_3.atp

Fields:
  Column 1: Pattern index
  Column 2: Start address in memory
  Column 3: Source pattern name


.PINSTATE FORMAT (Debug)
========================

Generated during conversion, shows states found per pin:

D0   RESET_N: X 0 1
D1   JTRST_N: 1 0
D2   JTMS: 0 1
D3   JTCK: 0 P
D4   JTDI: X 0 1
D5   JTDO: X H L

Format: <index> <pin_name>: <states_found>
Used to auto-generate pin type configuration.
```

### 9.3 Vector Timing

```
VECTOR TIMING MODEL
===================

                    MASTER_PERIOD (e.g., 10ns)
              |<--------------------------------->|
              |                                   |
    vec_clk   |   +---------------------------+   |
    __________+   |                           |   +__________
              |   |                           |   |
              +---+                           +---+
              ^                               ^
              |                               |
         Rising Edge                     Falling Edge
         (data setup)                    (data sample)


TIMING PARAMETERS:
==================
MASTER_PERIOD  : Base clock period in nanoseconds
Rise Delay     : Delay from vec_clk to pulse rise (for PULSE pins)
Fall Delay     : Delay from vec_clk to pulse fall (for PULSE pins)

Example:
  MASTER_PERIOD = 10.0   (100 MHz vector rate)
  Rise Delay = 2ns       (pulse rises 2ns after vec_clk)
  Fall Delay = 5ns       (pulse falls 5ns after rise)
```

---

## 10. HOW TO MODIFY EACH COMPONENT [CURRENT SYSTEM FOCUS]

> **This section focuses on modifying the CURRENT deployed system.**
> **Use the kzhang_v2 Vivado project and linux_*.elf commands.**

This section provides step-by-step instructions for modifying every part of the system.

### 10.1 MODIFYING THE FPGA LOGIC (RTL) [CURRENT]

```
FPGA MODIFICATION WORKFLOW (FOR CURRENT CONTROLLERS)
=====================================================

SOURCE FILES FOR CURRENT SYSTEM:
  Vivado Project: C:\Users\isaac\OneDrive...\Volt\kzhang_v2\kzhang_v2.xpr
  RTL Sources:    C:\Users\isaac\OneDrive...\Volt\kzhang_v2\kzhang_v2.srcs\

  (FBC project at C:\Dev\projects\FBC... is NOT deployed - ignore for now)

STEP 1: Set Up Vivado Project
-----------------------------
# Option A: Open existing project
vivado kzhang_v2.xpr

# Option B: Create new project
vivado -mode tcl
create_project my_project ./my_project -part xc7z020clg484-1
add_files -fileset sources_1 [list your_files.v]
add_files -fileset constrs_1 [list constraints.xdc]

STEP 2: Modify RTL
------------------
Key files to modify for custom functionality:

+------------------------+--------------------------------------------+
|         FILE           |            WHAT TO CHANGE                  |
+------------------------+--------------------------------------------+
| axi_io_table.v         | Add new pin types or behaviors             |
| axi_vector_status.v    | Add status registers, counters             |
| axi_freq_counter.v     | Add measurement channels                   |
| axi_pulse_ctrl.v       | Change pulse generation logic              |
| vector.vh              | Change vector width (default 128)          |
| top.v                  | Add new modules, change interconnects      |
+------------------------+--------------------------------------------+

STEP 3: Update Constraints (.xdc)
---------------------------------
File: constrs_1/imports/new/gpio_old_board.xdc

# Pin assignment format:
set_property PACKAGE_PIN <pin> [get_ports gpio[<num>]]
set_property IOSTANDARD LVCMOS33 [get_ports gpio[<num>]]

# Add new pins:
set_property PACKAGE_PIN AA10 [get_ports my_new_signal]
set_property IOSTANDARD LVCMOS33 [get_ports my_new_signal]

# Timing constraints:
create_clock -period 10.000 -name clk [get_ports clk]
set_input_delay -clock clk -max 2.0 [get_ports data_in]
set_output_delay -clock clk -max 2.0 [get_ports data_out]

STEP 4: Synthesize and Implement
--------------------------------
# In Vivado TCL console:
launch_runs synth_1
wait_on_run synth_1
launch_runs impl_1 -to_step write_bitstream
wait_on_run impl_1

# Output files:
#   impl_1/design_1_wrapper.bit  - FPGA bitstream
#   impl_1/design_1_wrapper.hwh  - Hardware handoff

STEP 5: Create Boot Image
-------------------------
# Generate BOOT.BIN with new bitstream:
bootgen -image bootgen.bif -o BOOT.BIN -w

# bootgen.bif contents:
the_ROM_image:
{
    [bootloader]fsbl.elf
    design_1_wrapper.bit
    u-boot.elf
}
```

### 10.2 MODIFYING THE ARM FIRMWARE

```
ARM FIRMWARE MODIFICATION
=========================

TWO APPROACHES:
===============

APPROACH A: Modify Linux Executables (Original System)
------------------------------------------------------

1. Source code location (if available):
   - C code compiled with arm-linux-gnueabihf-gcc
   - Uses memory-mapped I/O to FPGA registers

2. Create new executable:

   // my_new_command.c
   #include <stdio.h>
   #include <fcntl.h>
   #include <sys/mman.h>

   #define FPGA_BASE 0x40040000
   #define MAP_SIZE  0x10000

   int main(int argc, char *argv[]) {
       int fd = open("/dev/mem", O_RDWR | O_SYNC);
       void *mapped = mmap(NULL, MAP_SIZE, PROT_READ|PROT_WRITE,
                           MAP_SHARED, fd, FPGA_BASE);

       // Read/write FPGA registers
       volatile uint32_t *regs = (uint32_t *)mapped;
       printf("STATUS = 0x%08x\n", regs[1]);

       munmap(mapped, MAP_SIZE);
       close(fd);
       return 0;
   }

3. Cross-compile:
   arm-linux-gnueabihf-gcc -o my_new_command.elf my_new_command.c

4. Deploy:
   scp my_new_command.elf root@172.16.0.xxx:/mnt/


APPROACH B: Bare Metal Rust (FBC Project)
-----------------------------------------

1. Source location:
   C:\Dev\projects\FBC Semiconductor System\firmware\

2. Modify source:

   // firmware/src/regs.rs - Add new register definitions
   pub const MY_NEW_REG: usize = 0x4008_0000;

   pub fn read_my_reg() -> u32 {
       unsafe { core::ptr::read_volatile(MY_NEW_REG as *const u32) }
   }

   // firmware/src/main.rs - Use new registers
   fn main() -> ! {
       let value = regs::read_my_reg();
       // ...
   }

3. Build:
   cd firmware
   cargo build --release --target armv7a-none-eabi

4. Output: target/armv7a-none-eabi/release/firmware (ELF binary)
```

### 10.3 MODIFYING PIN CONFIGURATION

```
PIN CONFIGURATION CHANGES
=========================

METHOD 1: Runtime Configuration (No Rebuild)
--------------------------------------------

# SSH to controller
ssh root@172.16.0.xxx

# Set single pin type
/mnt/linux_pin_type.elf 0 2       # GPIO0 = OUTPUT
/mnt/linux_pin_type.elf 1 1       # GPIO1 = INPUT
/mnt/linux_pin_type.elf 2 4       # GPIO2 = PULSE

# Set pulse timing
/mnt/linux_Pulse_Delays.elf 2 4 10 20 200   # pin2, PULSE, 10ns rise, 20ns fall

# Bulk configuration via file
/mnt/linux_Allpin_type.elf /home/device/vectors/pattern.pinstate


METHOD 2: Modify .pinstate File
-------------------------------

Create/edit pinstate file:

# /home/device/vectors/my_pattern.pinstate
D0   GPIO0: 0 1             # Output pin (drives 0 and 1)
D1   GPIO1: L H             # Input pin (expects L and H)
D2   GPIO2: P               # Pulse clock
D3   GPIO3: X 0 1           # Bidirectional
...


METHOD 3: Modify Pin Mapping (.map)
-----------------------------------

Edit the .map file to reassign physical pins:

# /home/device/device.map
# Tester_Channel   DUT_Pin;
B13_GPIO0          MY_SIGNAL_A;
B13_GPIO1          MY_SIGNAL_B;
B13_GPIO2          MY_CLOCK;
# ...
```

### 10.4 MODIFYING VECTOR PATTERNS

```
VECTOR PATTERN MODIFICATION
===========================

STEP 1: Choose Source Format
----------------------------
Option A: Create ATP directly (simplest)
Option B: Convert from STIL
Option C: Convert from AVC

STEP 2: Create/Edit ATP File
----------------------------

# my_pattern.atp
(
CLK
DATA_IN
DATA_OUT
RESET
)
{
> ts1 0000;                 # Initial state
repeat 10 > ts1 P000;       # 10 clock cycles, everything low
> ts1 P100;                 # Clock + DATA_IN high
> ts1 P0H0;                 # Clock + expect DATA_OUT high
repeat 100 > ts1 P1H0;      # 100 cycles driving/checking
> ts1 0000;                 # Return to idle
}

STEP 3: Convert to Binary
-------------------------
# Using Python tool
python /opt/tools/makePN14Nov2018.py -m device.map -c my_pattern.atp

# Outputs:
#   my_pattern.hex       - Binary vectors
#   my_pattern.seq       - Sequence file
#   my_pattern.pinstate  - Pin states (for pin type config)
#   my_pattern.ascii     - Debug ASCII view

STEP 4: Deploy to Controller
----------------------------
scp my_pattern.hex root@172.16.0.xxx:/home/device/vectors/
scp my_pattern.seq root@172.16.0.xxx:/home/device/vectors/

STEP 5: Run Pattern
-------------------
ssh root@172.16.0.xxx
/mnt/linux_load_vectors.elf /home/device/vectors/my_pattern.seq \
                            /home/device/vectors/my_pattern.hex
/mnt/linux_run_vector.elf 0 0 my_pattern 1 0 5
```

### 10.5 MODIFYING POWER SUPPLY CONFIGURATION

```
POWER SUPPLY MODIFICATION
=========================

PERIPHERAL SUPPLIES (LC/HC - PMBus)
-----------------------------------
16 channels, 12A (LC) or 40A (HC) each

# Turn on supply 0 at 1.8V
/mnt/linux_pmbus_PicoDlynx.elf 0 1.8

# Read voltage and current
/mnt/linux_read_PicoDlynx.elf 0

# Turn off
/mnt/linux_pmbus_OFF.elf 0


CORE SUPPLIES (VICOR)
---------------------
4-6 high-current supplies

# Set core voltage to 1.0V
/mnt/linux_VICOR.elf 1.0

# With specific MIO and DAC channels
/mnt/linux_VICOR.elf 0.9 0 9


IN TEST PLAN (.tp file)
-----------------------
Modify PU_LIST in test step:

PU_LIST = 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,V_x,0,v_xx,0,p_xx,0,V_8,1,1.000,0.000:...

Format breakdown:
  16 peripheral channel enables (0 or 1)
  Core supply type
  Core supply enable
  Core supply name
  Core supply active
  Voltage
  Delay_ms
```

### 10.6 MODIFYING CLOCK/TIMING

```
CLOCK AND TIMING MODIFICATION
=============================

PLL CONFIGURATION
-----------------
# Set vector clock to 50 MHz, 0 phase, 50% duty
/mnt/linux_xpll_frequency.elf 1 50000000 0 50

# Set pulse clock to 100 MHz
/mnt/linux_xpll_frequency.elf 2 100000000 0 50

# Set system clocks
/mnt/linux_xpll_frequency.elf 3 25000000 0 50   # sys_clk1
/mnt/linux_xpll_frequency.elf 4 10000000 0 50   # sys_clk2

# Enable/disable clocks
/mnt/linux_xpll_off_on.elf 1 1 0 0   # Enable clk1,clk2; disable clk3,clk4


TIMING FILE (.tim)
------------------
Timing files are shell scripts executed before vectors:

# timing/my_timing.tim
#!/bin/bash
/mnt/linux_xpll_frequency.elf 1 100000000 0 50
/mnt/linux_xpll_frequency.elf 2 100000000 0 50
/mnt/linux_xpll_off_on.elf 1 1 1 1
/mnt/linux_Pulse_Delays.elf 2 4 5 15 200


TEST PLAN MASTER_PERIOD
-----------------------
In .tp file, MASTER_PERIOD sets base vector rate:

TEST_STEP : MY_STEP{
    MASTER_PERIOD = 10.0     # 10ns = 100 MHz
    # or
    MASTER_PERIOD = 50.0     # 50ns = 20 MHz
}
```

### 10.7 MODIFYING BOOT/SD CARD

```
SD CARD MODIFICATION
====================

SD CARD STRUCTURE (REQUIRED FILES):
-----------------------------------
/
├── BOOT.BIN        # First-stage bootloader + bitstream + u-boot
├── image.ub        # Linux kernel + device tree + ramdisk
├── init.sh         # Startup script (sets MAC address)
└── [*.elf files]   # Application executables

MODIFYING BOOT.BIN:
-------------------
BOOT.BIN contains three components:
1. FSBL (First Stage Boot Loader)
2. FPGA Bitstream (.bit)
3. U-Boot or bare-metal application

To rebuild with new bitstream:

# Create .bif file
cat > bootgen.bif << EOF
the_ROM_image:
{
    [bootloader]fsbl.elf
    my_new_design.bit
    u-boot.elf
}
EOF

# Generate BOOT.BIN
bootgen -image bootgen.bif -o BOOT.BIN -w on

MODIFYING init.sh:
------------------
# /mnt/init.sh - Sets MAC address on boot
#!/bin/sh
# Unique MAC for this controller
ifconfig eth0 hw ether 00:0a:35:00:01:XX
ifconfig eth0 up
udhcpc -i eth0

MODIFYING LINUX KERNEL:
-----------------------
1. Get Xilinx Linux source
2. Configure for Zynq:
   make ARCH=arm xilinx_zynq_defconfig
3. Build:
   make ARCH=arm CROSS_COMPILE=arm-linux-gnueabihf- uImage
4. Package with device tree:
   mkimage -A arm -T multi -C none -a 0 -e 0 \
     -n 'Linux' -d Image:devicetree.dtb image.ub
```

### 10.8 CREATING A COMPLETELY NEW SYSTEM

```
BUILDING FROM SCRATCH
=====================

OPTION 1: Keep Linux, Replace Application
-----------------------------------------
Easiest path - keep existing boot, modify only user-space code.

1. Write new C/C++ application
2. Cross-compile for ARM Linux
3. Replace HPBI.elf with your application
4. Modify init.sh to launch your app on boot

OPTION 2: Bare Metal (No Linux)
-------------------------------
Higher performance, faster boot, but more work.

1. Use FBC project as template:
   C:\Dev\projects\FBC Semiconductor System\firmware\

2. Create FSBL that jumps directly to your code
3. No Linux means:
   - You implement all drivers (Ethernet, SD, etc.)
   - Direct hardware access (faster)
   - No filesystem (vectors in flash/RAM)

3. Build flow:
   cargo build --release --target armv7a-none-eabi
   # Link at 0x00100000 (DDR start after boot vectors)
   # Create BOOT.BIN with FSBL + your.elf (no u-boot)

OPTION 3: Complete Redesign
---------------------------
Change everything: FPGA logic, firmware, and protocol.

1. Design new FPGA block diagram in Vivado
   - Keep processing_system7 (Zynq PS)
   - Replace all custom IP with your design
   - Export hardware (.xsa file)

2. Create new firmware
   - Use Xilinx SDK or bare-metal Rust
   - Implement your register interface
   - Implement your protocol

3. Create new host software
   - Python, C++, or other
   - Implement your protocol
   - Build user interface

4. Build SD card image
   - FSBL from Xilinx
   - Your bitstream
   - Your application
```

---

## APPENDIX A: FILE LOCATIONS SUMMARY

```
+====================================================================+
|                    FILE LOCATIONS BY SYSTEM                        |
+====================================================================+

+=========================== CURRENT SYSTEM ===========================+
| USE THESE FILES TO MODIFY THE ~500 EXISTING CONTROLLERS             |
+=======================================================================+

VIVADO PROJECT (FPGA Design):
  C:\Users\isaac\OneDrive - ISE Labs\IMPORTANT DOCUMENTS\Training\
    Volt\kzhang_v2\
      ├── kzhang_v2.xpr                     # <-- OPEN THIS IN VIVADO
      └── kzhang_v2.srcs\
          ├── sources_1\bd\design_1\        # Block design
          ├── sources_1\imports\hdl\        # RTL source files
          └── constrs_1\                    # Pin constraints

FIRMWARE API DOCUMENTATION:
  C:\Users\isaac\OneDrive - ISE Labs\IMPORTANT DOCUMENTS\Training\
    Volt 4\
      ├── SONOMA_FIRMWARE_API_SPECIFICATIONS.md  # <-- ALL COMMANDS
      ├── SONOMA_TOOLS_SPECIFICATIONS.md         # <-- VECTOR TOOLS
      └── Controller Pin Out.xlsx                # Pin mapping

HARDWARE DOCUMENTATION:
  C:\Users\isaac\OneDrive - ISE Labs\IMPORTANT DOCUMENTS\Training\
    Volt\hpbicontroller-rev1-0-2014-02-22\
      ├── Documents\
      │   ├── ug585-Zynq-7000-TRM.pdf      # CRITICAL - Zynq manual
      │   ├── ug470_7Series_Config.pdf      # Config guide
      │   └── pg144-axi-gpio.pdf            # AXI GPIO
      ├── HPBIController_SCH_Rev1-0.PDF     # Controller schematic
      └── 02290C_HPBIController_GER.zip     # Gerber files

CONFIG FILE REFERENCE:
  C:\Users\isaac\OneDrive - ISE Labs\IMPORTANT DOCUMENTS\Training\
    Notes\
      └── Device Files -PIN MAP, LEVEL, TIMING...txt

+======================== FBC PROJECT (FUTURE) ========================+
| NOT ON HARDWARE - Reference only for future deployment              |
+=======================================================================+

C:\Dev\projects\FBC Semiconductor System\
  ├── README.md                    # Project docs
  ├── docs\register_map.md         # Register definitions
  ├── docs\STATUS.md               # Status notes
  ├── constraints\zynq7020.xdc     # Pin constraints
  ├── firmware\                    # Rust bare-metal firmware
  ├── rtl\                         # New Verilog RTL
  ├── fpga-toolchain\              # Custom synthesis tool
  └── host\                        # PC-side tools
```

---

## APPENDIX B: QUICK REFERENCE CARD

```
+------------------------------------------------------------------+
|                     QUICK REFERENCE                              |
+------------------------------------------------------------------+

SSH ACCESS:
  ssh root@172.16.0.xxx    (password: root)

COMMON COMMANDS:
  /mnt/linux_pin_type.elf <pin> <type>    # Configure pin
  /mnt/linux_XADC.elf                      # Read ADCs
  /mnt/linux_EXT_DAC.elf v1 v2...v10       # Set DACs
  /mnt/linux_xpll_frequency.elf clk hz ph duty  # Set clock
  /mnt/linux_load_vectors.elf seq hex      # Load pattern
  /mnt/linux_run_vector.elf ...            # Run pattern

PIN TYPES:
  0=BIDIR  1=INPUT  2=OUTPUT  3=OPEN_C
  4=PULSE  5=NPULSE 6=ERROR_TRIG 7=VEC_CLK

MEMORY MAP:
  0x4004_0000 = FBC Decoder
  0x4005_0000 = Pin Config
  0x4006_0000 = Vector Status
  0x4007_0000 = Freq Counters
  0xE000_B000 = GEM Ethernet
  0xFFFC_0000 = OCM (256KB)

BUILD COMMANDS:
  make firmware      # Build ARM code
  make sim           # Simulate RTL
  make vivado        # Full FPGA build
```

---

**END OF DOCUMENT**

*Document Version: 1.0*
*Generated: 2026-01-13*
*For: ZYNQ 7020 ARM Controller repurposing project*

