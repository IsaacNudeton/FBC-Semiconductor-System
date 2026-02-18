# Sonoma/Everest System Architecture Reference

> This document describes the legacy Sonoma burn-in system architecture.
> Use this as a reference when understanding how FBC maps to the original system.
> See `OPERATIONAL_WORKFLOW.md` for the FBC equivalents.

---

## Sonoma Burn-In System: Complete Dependency Map

  Layer 0: Purpose (The Root)

  WHY DOES THIS EXIST?
  │
  └── Accelerated life testing of semiconductor devices
      │
      ├── Apply stress (voltage, temperature, patterns)
      ├── Detect infant mortality failures
      └── Screen devices before shipping to customers

  Nothing to change here - this is the fixed purpose.

  ---
  Layer 1: Test Definition

  WHAT ARE WE TESTING?
  │
  ├── Device Type (e.g., ARA, C9402, Treasure)
  │   └── Defines: Pin count, power rails, protocols
  │
  ├── Stress Conditions
  │   ├── Temperature (e.g., 125°C)
  │   ├── Voltage (nominal, +10%, -10%)
  │   └── Duration (24h, 48h, 168h)
  │
  └── Pass/Fail Criteria
      ├── Current limits (overcurrent = fail)
      ├── Pattern errors (mismatch = fail)
      └── Temperature excursions (overheat = fail)

  | Change            | Affects                                  | Also Change                      |
  |-------------------|------------------------------------------|----------------------------------|
  | New device type   | BIM file, Map file, PowerOn/Off, Vectors | Load board if pinout differs     |
  | Temperature range | Testplan, heater/cooler capacity         | Thermal design if exceeds limits |
  | Voltage range     | Testplan, LCPS configuration             | LCPS modules if out of range     |

  ---
  Layer 2: System Architecture

  HOW IS IT ORGANIZED?
  │
  ├── Everest Server (Windows PC)
  │   ├── Manages multiple shelves
  │   ├── Stores testplans, logs results
  │   └── Unity.exe GUI for operators
  │
  ├── Shelf (×11 in your system)
  │   ├── Front Tray + Back Tray
  │   └── Ethernet switch for controllers
  │
  ├── Tray
  │   ├── Load Board (power infrastructure)
  │   └── BIM (device-specific sockets)
  │
  └── Controller (Zynq-7020)
      ├── One per load board
      ├── SD card with firmware
      └── Ethernet to Everest

  | Change                                     | Affects                 | Also Change                    |
  |--------------------------------------------|-------------------------|--------------------------------|
  | Add more shelves                           | Everest config, network | IP addressing, switch capacity |
  | Full board → Half board                    | Tray layout             | BIM mounting, controller count |
  | Controller location (on BIM vs load board) | Cabling                 | Connector pinout               |

  ---
  Layer 3: Everest Server

  EVEREST (Windows Application)
  │
  ├── Configuration Database
  │   ├── Shelf definitions (IP addresses)
  │   ├── Device library (BIM files)
  │   └── Testplan library (.tpf files)
  │
  ├── Communication Layer
  │   ├── TCP/IP to each controller
  │   ├── Commands: PowerOn, LoadVectors, StartTest, Abort
  │   └── Status: Temperature, Current, Errors, Progress
  │
  ├── Data Logging
  │   ├── CSV files per test run
  │   ├── Min/Max/Avg per channel
  │   └── Error logs
  │
  └── Unity.exe (GUI)
      ├── Shelf/Tray/BIM visualization
      ├── Real-time analog display
      └── Test control buttons

  Everest File Structure

  Everest/
  ├── Data/
  │   ├── Bims/
  │   │   └── <device>.bim          ← DEVICE DEFINITION
  │   ├── Testplans/
  │   │   └── <test>.tpf            ← TEST SEQUENCE
  │   └── Logs/
  │       └── <date>_<device>/      ← RESULTS
  │
  ├── devices/
  │   └── <device>/
  │       ├── <device>.map          ← PIN MAPPING
  │       ├── PowerOn               ← POWER SEQUENCE
  │       ├── PowerOff              ← SHUTDOWN SEQUENCE
  │       ├── vectors/
  │       │   ├── <pattern>.hex     ← BIT PATTERNS
  │       │   └── <pattern>.seq     ← SEQUENCE CONTROL
  │       └── <test>.tim            ← TIMING FILE
  │
  └── Firmware/
      └── FW_v2.11/
          ├── top.bit               ← FPGA BITSTREAM
          ├── BOOT.bin              ← BOOTLOADER
          ├── init.sh               ← BOOT SCRIPT
          └── bin/                  ← EXECUTABLES

  | File          | What It Defines                    | Change Affects              |
  |---------------|------------------------------------|-----------------------------|
  | <device>.bim  | Pin names, DUT layout, power rails | PowerOn, Map file, Testplan |
  | <device>.map  | GPIO# → Pin name → ADC channel     | Firmware if GPIO reassigned |
  | PowerOn       | Supply sequencing, voltages        | Device behavior, safety     |
  | PowerOff      | Shutdown sequence                  | Safe power-down             |
  | <pattern>.hex | Bit patterns per cycle             | Test coverage               |
  | <pattern>.seq | Timing, repeat counts              | Test duration               |
  | <test>.tpf    | Full test sequence                 | Everything it references    |

  ---
  Layer 4: Controller (The Brain)

  CONTROLLER (Zynq-7020)
  │
  ├── Hardware (Fixed)
  │   ├── ARM Cortex-A9 Dual Core @ 667 MHz
  │   ├── FPGA Fabric: 85K LUTs, 106K FFs
  │   ├── DDR3: 1 GB
  │   ├── GPIO: 128 vector + 32 extra
  │   └── Interfaces: Ethernet, I2C, SPI, UART
  │
  ├── SD Card (Changeable)
  │   ├── BOOT.bin (U-Boot bootloader)
  │   ├── FSBL.elf (First Stage Boot Loader)
  │   ├── top.bit (FPGA configuration)
  │   └── /mnt/ filesystem
  │       ├── init.sh
  │       ├── contact.sh
  │       ├── bin/*.elf
  │       └── AWK scripts
  │
  └── Runtime State
      ├── /tmp/LockBit (hardware mutex)
      ├── /tmp/JobStatus (current state)
      ├── /tmp/TemperatureLimits
      └── /tmp/VectorName

  Controller Boot Sequence

  Power On
      │
      ▼
  FSBL.elf loads from SD card
      │
      ▼
  U-Boot (BOOT.bin) initializes DDR, loads Linux
      │
      ▼
  Linux kernel boots
      │
      ▼
  init.sh runs:
      ├── cat top.bit > /dev/xdevcfg     ← Load FPGA
      ├── linux_pmbus_OFF.elf ×16        ← All supplies OFF
      ├── linux_EXT_DAC.elf 0 0 0...     ← Zero DACs
      ├── ToggleMio.elf 39 0             ← GPIO low
      ├── linux_init_XADC.elf            ← Init ADC
      └── contact.sh                      ← Connect to Everest
      │
      ▼
  Waiting for Everest commands

  | Change         | Affects                          | Also Change                   |
  |----------------|----------------------------------|-------------------------------|
  | top.bit (FPGA) | GPIO behavior, timing, pin count | Map file if pins change       |
  | init.sh        | Boot behavior, default states    | PowerOn if sequence conflicts |
  | bin/*.elf      | Hardware control capabilities    | AWK scripts that call them    |
  | Controller IP  | Everest connection               | Everest shelf config          |

  ---
  Layer 5: FPGA Fabric (Real-Time Engine)

  FPGA (top.bit)
  │
  ├── Clock Generation
  │   ├── vec_clk_pll (MMCM)
  │   │   └── 50 MHz vector clock (configurable via DRP)
  │   └── 100 MHz AXI clock (from PS)
  │
  ├── DMA Engines (×3)
  │   ├── axi_dma_value  → 128-bit output values
  │   ├── axi_dma_oen    → 128-bit output enables
  │   └── axi_dma_repeat → 32-bit repeat counts
  │
  ├── CDC FIFOs
  │   └── 100 MHz (AXI) → 50 MHz (vector) crossing
  │
  ├── Pattern Engine
  │   ├── axis_comb      → Combines 3 DMA streams
  │   ├── repeat_counter → Expands patterns
  │   └── io_table       → Routes to 128 pins
  │
  ├── Per-Pin Logic (×128)
  │   ├── single_pin module
  │   │   ├── Type 0: INPUT (compare, detect errors)
  │   │   ├── Type 1: OUTPUT (drive)
  │   │   ├── Type 2: OPEN_COLLECTOR
  │   │   ├── Type 3: PULSE (timed high)
  │   │   ├── Type 4: NPULSE (timed low)
  │   │   ├── Type 5: VEC_CLK (clock output)
  │   │   └── Type 6: BIDI (bidirectional)
  │   └── IOBUF (tri-state control)
  │
  └── AXI Peripherals
      ├── axi_io_table     @ 0x40040000 (pin types)
      ├── axi_pulse_ctrl   @ 0x40050000 (pulse timing)
      ├── axi_freq_counter @ 0x40060000 (frequency measurement)
      └── axi_vector_status @ 0x40070000 (errors, status)

  FPGA Data Flow

  DDR3 Memory
      │ (patterns loaded by linux_load_vectors.elf)
      ▼
  ┌─────────────────────────────────────────────┐
  │ AXI DMA Engines (×3)                        │
  │ ├── value[127:0]  ─┐                        │
  │ ├── oen[127:0]    ─┼──→ 100 MHz             │
  │ └── repeat[31:0]  ─┘                        │
  └─────────────────────────────────────────────┘
      │
      ▼
  ┌─────────────────────────────────────────────┐
  │ CDC FIFOs (Clock Domain Crossing)           │
  │ 100 MHz write ──→ 50 MHz read               │
  └─────────────────────────────────────────────┘
      │
      ▼
  ┌─────────────────────────────────────────────┐
  │ axis_comb (Stream Combiner)                 │
  │ {value, oen, repeat} → 272 bits             │
  └─────────────────────────────────────────────┘
      │
      ▼
  ┌─────────────────────────────────────────────┐
  │ repeat_counter                              │
  │ Input: 1 vector + repeat=N                  │
  │ Output: N copies of vector                  │
  └─────────────────────────────────────────────┘
      │
      ▼
  ┌─────────────────────────────────────────────┐
  │ io_table                                    │
  │ For each pin 0-127:                         │
  │   Read pin_type from axi_io_table           │
  │   Route to appropriate single_pin logic     │
  └─────────────────────────────────────────────┘
      │
      ▼
  ┌─────────────────────────────────────────────┐
  │ single_pin[i] (×128)                        │
  │ Based on pin_type:                          │
  │   OUTPUT → drive pin_dout                   │
  │   INPUT  → compare pin_din, flag errors     │
  │   PULSE  → generate timed pulse             │
  └─────────────────────────────────────────────┘
      │
      ▼
  ┌─────────────────────────────────────────────┐
  │ IOBUF[i] (×128)                             │
  │ T = ~oen → tri-state when oen=0             │
  │ I = dout → drive value                      │
  │ O = din  → sense value                      │
  └─────────────────────────────────────────────┘
      │
      ▼
  GPIO Pins → Load Board → BIM → DUT

  | Change                 | Affects                   | Also Change                |
  |------------------------|---------------------------|----------------------------|
  | Vector clock frequency | Pattern timing            | Testplan timing values     |
  | Pin count (128→256)    | io_table width, DMA width | Map file, load board, BIM  |
  | Pin type logic         | Signal behavior           | Vectors that use that type |
  | AXI addresses          | Firmware register access  | All *.elf binaries         |

  ---
  Layer 6: Load Board (Power Infrastructure)

  LOAD BOARD
  │
  ├── Controller Mount
  │   ├── [Newer] Controller screwed to board
  │   └── [Older] Connector for controller on BIM
  │
  ├── LCPS Connectors (1-2 modules)
  │   ├── PMBus addressable supplies
  │   ├── Address set by resistors (29k/54k)
  │   └── Controlled via I2C from controller
  │
  ├── VICOR Modules
  │   ├── High-current core voltage
  │   ├── Trim via DAC output from controller
  │   └── Enable via MIO GPIO
  │
  ├── Signal Conditioning
  │   ├── Level shifters (2.5V ↔ DUT voltage)
  │   ├── Series resistors (impedance matching)
  │   └── ESD protection
  │
  ├── Sense Circuits
  │   ├── Current sense resistors → ADC
  │   ├── Voltage dividers → XADC
  │   └── Temperature sense → XADC
  │
  ├── BIM Connectors
  │   ├── High-density connectors to standoffs
  │   ├── Power rails routed to BIM
  │   └── GPIO signals routed to BIM
  │
  └── Manual Switch
      └── Direct power override (bypasses controller)

  LCPS Addressing

  No resistor      → Address 0 (or addressless mode)
  29k resistor     → Address X
  54k resistor     → Address Y
  Both resistors   → Address Z

  Controller sends: linux_pmbus_PicoDlynx.elf <address> <voltage>

  | Change                 | Affects                  | Also Change              |
  |------------------------|--------------------------|--------------------------|
  | LCPS address resistors | PMBus addressing         | PowerOn/PowerOff scripts |
  | LCPS module type       | Voltage/current capacity | Testplan voltage limits  |
  | VICOR module           | Core voltage range       | PowerOn script, testplan |
  | Sense resistor values  | ADC scaling              | Map file formulas        |
  | Level shifter voltage  | Signal compatibility     | BIM design if mismatched |

  ---
  Layer 7: BIM (Burn-In Module)

  BIM
  │
  ├── DUT Sockets
  │   ├── Layout: Rows × Columns (e.g., 3×4 = 12 DUTs)
  │   ├── Socket type matches device package
  │   └── Individual thermal interface per DUT
  │
  ├── Signal Routing
  │   ├── Shared signals → All DUTs (JTAG, clocks)
  │   ├── Some DUTs → Groups (power monitoring)
  │   └── One DUT → Individual (case temp)
  │
  ├── Power Distribution
  │   ├── Rail per supply (VDD, AVDD, AVDDEL, etc.)
  │   ├── Decoupling capacitors per DUT
  │   └── Current sense per rail or per DUT
  │
  ├── Thermal Management
  │   ├── Heater zones
  │   ├── Temperature sensors per DUT
  │   └── Thermal interface material
  │
  └── Connector to Load Board
      └── High-density connector to standoffs

  BIM File Structure (ara.bim)

  <BimType type="0027" hardware="Sonoma" version="2.0">
      <Layout>
          <Rows>3</Rows>        ← Physical layout
          <Columns>4</Columns>
      </Layout>

      <Device name="ara">
          <Pins total="55">
              <!-- Shared signals go to all 12 DUTs -->
              <Pin share="All DUTs" type="Signal">TCK</Pin>
              <Pin share="All DUTs" type="Signal">TDI</Pin>

              <!-- Power supplies may be shared or grouped -->
              <Pin share="Some DUTs" type="PmbPS">PS2_AVDDEL</Pin>
              <Pin share="Some DUTs" type="CorePS">PS1_VDD</Pin>

              <!-- ADC channels are often per-DUT -->
              <Pin share="One DUT" type="ADC">CASE_TEMP</Pin>
              <Pin share="One DUT" type="ADC">VDD_I_DUT</Pin>
          </Pins>

          <CorePsMonitor>
              <Pin name="PS1_VDD" vAdc="VDD_S" iAdc="VDD_I"/>
          </CorePsMonitor>
      </Device>

      <Duts total="12">
          <!-- Each DUT maps logical pins to physical resources -->
          <Dut name="C1">1,2,3,...,CPS1,A63,A48,...</Dut>
          <Dut name="C2">1,2,3,...,CPS1,A36,A32,...</Dut>
          ...
      </Duts>
  </BimType>

  | Change              | Affects                 | Also Change                     |
  |---------------------|-------------------------|---------------------------------|
  | DUT count (12→24)   | BIM file, ADC mapping   | Load board if more power needed |
  | Socket type         | Physical BIM design     | Nothing in software             |
  | Pin sharing         | BIM file, routing       | Map file                        |
  | Power rail grouping | BIM file, current sense | Map file, testplan limits       |

  ---
  Layer 8: Map File (GPIO ↔ Function)

  MAP FILE (<device>.map)
  │
  ├── GPIO Assignments
  │   ├── B13_GPIO0 MONITOR_SELECT;
  │   ├── B13_GPIO27 TCK;
  │   └── B13_GPIO33 TDO;
  │
  ├── External ADC Channels
  │   ├── ADC_0 AVDDL_I_C10
  │   ├── ADC_10 AVDDEL_I_C1
  │   └── ADC_31 AVDD_I_C10_C11
  │
  └── Internal XADC Channels
      ├── XADC_0 VDD_I_C2
      ├── XADC_1 CASE_TEMP_C10
      └── XADC_31 CASE_TEMP_C1

  Map File → Hardware Binding

  B13_GPIO27 TCK
      │
      ├── "B13" = Bank 13 (2.5V IO bank on Zynq)
      ├── "GPIO27" = Pin 27 of 128 vector pins
      └── "TCK" = JTAG clock signal to DUT

  | Change              | Affects                        | Also Change                 |
  |---------------------|--------------------------------|-----------------------------|
  | GPIO assignment     | Which physical pin drives what | XDC constraints in FPGA     |
  | ADC channel mapping | Which measurement goes where   | BIM file, testplan formulas |
  | Signal name         | Nothing (just label)           | Documentation               |

  ---
  Layer 9: Vectors (Test Patterns)

  VECTOR FILES
  │
  ├── <pattern>.hex (Bit Patterns)
  │   └── Raw hex data: what to drive/expect each cycle
  │
  ├── <pattern>.seq (Sequence Control)
  │   ├── Repeat counts
  │   ├── Timing parameters
  │   └── Condition codes
  │
  └── <pattern>.tim (Timing File)
      ├── Clock period
      ├── Edge positions
      └── Strobe points

  Vector Execution

  linux_load_vectors.elf reads:
      .seq file → DMA descriptors
      .hex file → DDR3 pattern data

  RunSuperVector.elf executes:
      Start DMA → patterns stream to FPGA
      FPGA drives GPIO at 50 MHz
      Errors flagged in axi_vector_status

  | Change          | Affects                     | Also Change              |
  |-----------------|-----------------------------|--------------------------|
  | Pattern content | Test coverage, DUT behavior | Nothing (self-contained) |
  | Timing          | Pattern speed               | Testplan MASTER_PERIOD   |
  | Repeat counts   | Test duration               | Testplan VECTOR_TIME     |

  ---
  Layer 10: Testplan (Orchestration)

  TESTPLAN (<test>.tpf)
  │
  ├── Header
  │   ├── TEST_PROGRAM_NAME
  │   ├── TEST_DURATION (hours)
  │   ├── DEVICE_DIR
  │   ├── PIN_MAP_FILE → references .map
  │   └── SETUP_FILE → references .lvl
  │
  ├── TEST_STEPS (sequence)
  │   ├── TEST_STEP "PowerOn"
  │   │   ├── PU_LIST = power supplies to enable
  │   │   ├── SETPOINT_TEMP = target temperature
  │   │   └── VECTOR_FILE = patterns to run
  │   │
  │   ├── TEST_STEP "BurnIn_Start"
  │   │   └── OPTIONS = "5" (loop marker)
  │   │
  │   ├── TEST_STEP "BurnIn_Loop"
  │   │   ├── VECTOR_FILE = stress patterns
  │   │   ├── REPEAT_COUNT = iterations
  │   │   └── MASTER_PERIOD = clock period (ns)
  │   │
  │   └── TEST_STEP "BurnIn_End"
  │       └── OPTIONS = "6" (end loop)
  │
  └── ADC Configuration
      ├── SHUTDOWN_UPPER_LIMIT
      ├── SHUTDOWN_LOWER_LIMIT
      └── FORMULA (scaling)

  | Change           | Affects            | Also Change               |
  |------------------|--------------------|---------------------------|
  | TEST_DURATION    | How long test runs | Nothing                   |
  | SETPOINT_TEMP    | DUT temperature    | Heater capacity check     |
  | PU_LIST voltages | DUT stress level   | LCPS capability check     |
  | MASTER_PERIOD    | Pattern speed      | Vector timing if too fast |
  | VECTOR_FILE      | Which patterns run | Nothing (must exist)      |

  ---
  Master Dependency Graph

                      ┌─────────────────┐
                      │   NEW DEVICE    │
                      └────────┬────────┘
                               │
           ┌───────────────────┼───────────────────┐
           ▼                   ▼                   ▼
      ┌─────────┐        ┌──────────┐        ┌──────────┐
      │ BIM.xml │        │ .map     │        │ PowerOn/ │
      │         │◄──────►│          │◄──────►│ PowerOff │
      └────┬────┘        └────┬─────┘        └────┬─────┘
           │                  │                   │
           │     ┌────────────┼───────────────────┘
           │     │            │
           ▼     ▼            ▼
      ┌─────────────────────────────┐
      │        Load Board           │
      │  (LCPS, VICOR, connectors)  │
      └──────────────┬──────────────┘
                     │
           ┌─────────┴─────────┐
           ▼                   ▼
      ┌─────────┐        ┌──────────┐
      │ top.bit │        │ bin/*.elf│
      │ (FPGA)  │◄──────►│(firmware)│
      └────┬────┘        └────┬─────┘
           │                  │
           └────────┬─────────┘
                    │
                    ▼
           ┌────────────────┐
           │   Controller   │
           │   (SD Card)    │
           └────────┬───────┘
                    │
                    ▼
           ┌────────────────┐
           │    Everest     │
           │   (testplans)  │
           └────────────────┘

  ---
  Change Impact Matrix

  | If You Change...    | You Must Also Change...                                                 |
  |---------------------|-------------------------------------------------------------------------|
  | Device type         | BIM file, Map file, PowerOn/Off, Vectors, Testplan, possibly Load Board |
  | Pin count           | FPGA (top.bit), Map file, Load Board, BIM                               |
  | Power supply        | PowerOn/Off, LCPS config, Testplan limits                               |
  | Temperature range   | Testplan, possibly thermal hardware                                     |
  | GPIO assignment     | Map file, XDC constraints, rebuild top.bit                              |
  | ADC formula         | Map file or Testplan (depends on where formula lives)                   |
  | Vector patterns     | Nothing else (self-contained)                                           |
  | Test sequence       | Testplan only                                                           |
  | Controller firmware | SD card only (unless AXI addresses change)                              |
  | FPGA design         | top.bit, possibly firmware if registers change                          |
  | Everest version     | Possibly firmware if protocol changes                                   |

  ---
  What's Changeable Without Hardware Modifications

  | Layer                | Software Only       | Needs Hardware               |
  |----------------------|---------------------|------------------------------|
  | Testplan             | ✅                  |                              |
  | Vectors              | ✅                  |                              |
  | PowerOn/Off          | ✅                  |                              |
  | Map file             | ✅ (if GPIOs exist) | If new GPIOs needed          |
  | BIM file             | ✅                  | If socket/routing changes    |
  | Firmware (bin/*.elf) | ✅                  |                              |
  | FPGA (top.bit)       | ✅ (rebuild)        | If pin count changes         |
  | LCPS addressing      |                     | ✅ (resistors)               |
  | Load Board           |                     | ✅                           |
  | BIM                  |                     | ✅                           |
  | Controller           |                     | ✅ (but SD card is software) |