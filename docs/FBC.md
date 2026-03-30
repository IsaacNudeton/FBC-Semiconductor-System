# FBC — Force Burn-in Controller

**Last Verified:** March 28, 2026 (FIRMWARE FEATURE-COMPLETE. 56 operations/79 wire codes across 13 subsystems. 47 FbcClient methods. All Sonoma gaps closed: thermal DAC (BU2505 ch0=cooler, ch1=heater), V×I power feedback, per-step temp+clock, undervoltage detection, min/max XADC tracking, IO bank command, network health. Current formula ×80 gain verified against Sonoma. 43 tests, 0 warnings. Needs single JTAG deploy.)
**Transport:** Raw Ethernet, EtherType 0x88B5
**Target:** Zynq 7020 (XC7Z020-1CLG484C), bare-metal Rust firmware

---

## Hardware

### Board Overview

- **SoC:** Zynq 7020 — dual Cortex-A9 (667MHz) + Artix-7 FPGA
- **DDR:** 512MB DDR3 @ 533MHz
- **GPIO:** 160 pins — 128 BIM (interposer, 2-cycle latency) + 32 fast (direct, 1-cycle)
- **Power:** 6 VICOR cores (programmable 500-1500mV), PMBus rails, 12V input
- **Analog:** 16 XADC channels (internal) + 16 MAX11131 channels (external SPI)
- **DAC:** BU2505FV (10ch, SPI, 10-bit, 4.096V ref) — drives VICOR voltage setpoints
- **Scale:** ~44 boards per rack

### Current Test Setup (Calibration)

Two boards physically connected:
- **Controller board** (GREEN) — Zynq 7020, runs firmware, has GEM0 Ethernet, JTAG header
- **Calibration BIM** (BLUE) — full-featured Burn-In Module connected via J3/J4/J5. Has EEPROM (never programmed), NTC thermistors, heater/cooler FETs, interposer — everything a production BIM has
- **Power:** 12V/3A only (BK Precision 9206 via VCC12 pad). No LCPS, no 48V, no DUT
- **EEPROM:** Present on BIM but never programmed. I2C bus may not be powered without LCPS — reads return empty/error, which is expected
- **Adapter:** TP-Link (ASIX chipset) USB Ethernet, forced 100M full-duplex, direct USB (no hub)

### Power Architecture

**12V Input** → TLV62130 regulators → 6 voltage banks:

| Bank | Voltage | Pins | Purpose |
|------|---------|------|---------|
| Bank 13 | 3.3V | gpio[0:47] | BIM I/O |
| Bank 33 | 3.3V | gpio[48:95] | BIM I/O |
| Bank 34 | 3.3V | gpio[96:127] | BIM I/O |
| Bank 35 | 3.3V | gpio[128:159] | Fast I/O |
| PS | 3.3V | MIO | ARM peripherals |
| VCCINT | 1.0V | — | FPGA core |

**VICOR Core Mapping (6 cores, 6 MIO enables):**

Software core numbers follow the production AWK scripts — they differ from PCB silkscreen labels.

| SW Core | DAC Ch | MIO Enable | PCB Label | Verified |
|---------|--------|------------|-----------|----------|
| 1 | 9 | MIO 0 | EN_COREPS1 | ✅ |
| 2 | 3 | MIO 39 | EN_COREPS4 | ✅ |
| 3 | 7 | MIO 47 | EN_COREPS3 | ✅ |
| 4 | 8 | MIO 8 | EN_COREPS2 | ✅ |
| 5 | 4 | MIO 38 | EN_COREPS5 | ✅ |
| 6 | 2 | MIO 37 | EN_COREPS6 | ✅ |

VICOR_ENABLE_PINS = `[0, 39, 47, 8, 38, 37]` (all 6 MIO pins, verified from production PowerOn scripts).

**Power-Up Sequence:** 12V → wait 100ms → PS rails → wait 50ms → PL bitstream → firmware → VICOR (software-controlled)

### Pin Mapping

160 GPIO pins in 4 banks:

| Range | Bank | Latency | Interface | Drive |
|-------|------|---------|-----------|-------|
| gpio[0:47] | 13 | 2-cycle | BIM (QSH connector) | 8mA (0-16), 4mA (17-47) |
| gpio[48:95] | 33 | 2-cycle | BIM | 4mA |
| gpio[96:127] | 34 | 2-cycle | BIM | 4mA |
| gpio[128:159] | 35 | 1-cycle | Direct FPGA | 4mA |

**Signal path:** FPGA → QSH → BIM interposer → DUT socket

**Special pins:**
- gpio[128] = TDO (test data out)
- gpio[129] = MDIO0 (management data)
- gpio[130] = vec_clk (vector clock output)
- gpio[131] = clock output

**Pin types** (4-bit per pin, AXI @ 0x4005_0000):

| Value | Type | Description |
|-------|------|-------------|
| 0 | BIDI | Bidirectional (default) |
| 1 | INPUT | Input only |
| 2 | OUTPUT | Output only |
| 3 | OPEN_C | Open collector |
| 4 | PULSE | Pulse driver |
| 5 | NPULSE | Negative pulse |
| 6 | ERR_TRIG | Error trigger |
| 7 | VEC_CLK | Vector clock |
| 8 | VEC_CLK_EN | Vector clock enable |

### Analog Channels (32 total)

| Channel | Source | Name | Formula | Unit |
|---------|--------|------|---------|------|
| 0 | XADC | DIE_TEMP | DieTemp | °C |
| 1 | XADC | VCCINT | Voltage(3000) | mV |
| 2 | XADC | VCCAUX | Voltage(3000) | mV |
| 3 | XADC | VCCBRAM | Voltage(3000) | mV |
| 4-15 | XADC | XADC_AUX0-11 | Voltage(1000) | mV |
| 16-21 | MAX11131 | VDD_CORE1-6 | Voltage(2000) | mV |
| 22 | MAX11131 | THERM_CASE | Thermistor(B=3985.3, R=30kΩ) | °C | ✅ Corrected March 25 — 30kΩ NTC default, `set_ntc_type()` for 10kΩ |
| 23 | MAX11131 | THERM_DUT | Thermistor(B=3985.3, R=30kΩ) | °C | ✅ Corrected March 25 — same NTC type, switchable at runtime |
| 24-25 | MAX11131 | I_CORE1-2 | Current(50mΩ shunt) | mA |
| 26 | MAX11131 | VDD_IO | Voltage(5000) | mV |
| 27 | MAX11131 | VDD_3V3 | Voltage(5000) | mV |
| 28 | MAX11131 | VDD_1V8 | Voltage(3000) | mV |
| 29 | MAX11131 | VDD_1V2 | Voltage(2000) | mV |
| 30-31 | MAX11131 | EXT_ADC14-15 | Voltage(4096) | mV |

**Formula types** (from `firmware/src/analog.rs`):

| Formula | Conversion | Notes |
|---------|-----------|-------|
| DieTemp | `raw × 503.975 / 65536 - 273.15` | Xilinx UG480, 16-bit register |
| Voltage { scale } | `raw × scale / 4096` | MAX11131: 12-bit, 4.096V ref |
| Thermistor { B, R } | Linear approx on firmware, full Steinhart-Hart on GUI | Firmware sends raw + rough value |
| Current { shunt } | `V_adc(mV) / R(mΩ)` | High-side shunt measurement |
| VicorCurrent { gain } | `V_adc(mV) × gain / 1000` | Sonoma uses gain=80 |

**Runtime formula switching:** The host can override any channel's formula at runtime
via `AnalogMonitor::set_formula(channel, formula)`. This lets test plans switch between
10kΩ (B=3492) and 30kΩ (B=3985.3) thermistors per test step, or change VICOR current
sense gain for different modules — without recompiling firmware.

**Calibration offsets:** EEPROM stores per-channel voltage_cal[16] and current_cal[16]
(signed i16 mV/mA offsets). Applied automatically via `BoardConfig.calibrate_voltage()`
on every analog read. Host can override via SET_OVERRIDE command (0x31).

**Temperature pipeline:**
- Die temp: XADC raw → `DieTemp` formula → °C (firmware-side, no libm needed)
- Case/DUT temp: MAX11131 raw → `Thermistor` linear approx → rough °C (firmware)
  + raw value sent to GUI → GUI applies full Steinhart-Hart with f64 math
- Sonoma does Steinhart-Hart in AWK on-board (burns CPU on log()). FBC is smarter:
  firmware stays simple (no libm), GUI has full precision

### JTAG

**Connector:** J1, Molex 87832-1420 (2x7, 2mm pitch, shrouded)

```
Pin 1 (keyed)
 1  GND     2  VREF (3.3V)
 3  GND     4  TMS
 5  GND     6  TCK
 7  GND     8  TDO
 9  GND    10  TDI
11  GND    12  NC
13  GND    14  n_SRST
```

**Adapters tested:** FT232H (channel A, MPSSE), Basys3 (BSCAN bridge)

### BIM EEPROM (24LC02, 256 bytes)

The BIM EEPROM is what makes each controller self-identifying and self-sufficient.
Sonoma never reads the physical EEPROM — everything comes from NFS-hosted XML files.
FBC reads it at boot, validates it, and reports it in the ANNOUNCE broadcast.

**Interface:** I2C @ 0x50, 400kHz, 8-byte pages, 5ms write cycle
**Magic:** 0xBEEF_CAFE (absent = unprogrammed/blank)

```
0x00-0x0F: Header (16 bytes)
  magic:            u32    — 0xBEEFCAFE = programmed
  version:          u8     — format version (current: 2)
  bim_type:         u8     — Normandy(1), SyrosV2(2), Aurora(3), Iliad(4)
  hw_revision:      u8
  _reserved:        u8
  serial_number:    u32    — unique board ID (overrides Device DNA in ANNOUNCE)
  manufacture_date: u32    — Unix timestamp

0x10-0x8F: Power Rail Config (128 bytes)
  rails[16]:        RailConfig × 16 — self-describing, any order
    channel_id:     u8     — PMBus channel 1-24 (0xFF = disabled/unused)
    flags:          u8     — bit 0: is_hcps, bit 1: monitored
    max_voltage_mv: u16    — over-voltage shutdown limit
    min_voltage_mv: u16    — under-voltage / brownout limit
    max_current_ma: u16    — over-current shutdown limit

  Supports LCPS J6 (ch 1-8), LCPS J7 (ch 9-16), and HCPS (ch 17-24).
  Unused slots have channel_id=0xFF. Nominal voltage = (max+min)/2.

0x90-0xCF: Calibration Data (64 bytes)
  voltage_cal[16]:  i16 × 16 — per-channel ADC offset (mV, signed)
  current_cal[16]:  i16 × 16 — per-channel current offset (mA, signed)

0xD0-0xD7: Project ID (8 bytes)
  project_code[8]:  ASCII — e.g., "S0026" (LRM database lookup key)

0xD8-0xD9: BIM Number (2 bytes)
  bim_number:       u16   — which unit in production batch (e.g., 17 of 40)

0xDA-0xDF: Thermal Config (6 bytes)
  setpoint_dc:      i16   — temperature setpoint in 0.1°C (e.g., 1250 = 125.0°C)
  sensor_type:      u8    — 0=NTC30k, 1=NTC10k, 2=diode
  cooling_type:     u8    — 0=air, 1=water
  heater_pin:       u8    — MIO pin for heater FET (0xFF = unknown)
  fan_pin:          u8    — MIO pin for cooler/fan FET (0xFF = unknown)

0xE0-0xE7: Statistics (8 bytes)
  program_count:    u16   — times reprogrammed (aging/audit)
  bim_asset_id[6]:  ASCII — physical inventory tag, e.g., "BIM-042"

0xE8-0xF7: Reserved (16 bytes)

0xF8-0xFB: CRC32 (4 bytes)
  checksum:         u32   — IEEE 802.3 CRC of bytes 0x00-0xF7

0xFC-0xFF: Reserved (4 bytes)
```

**v2 changes (March 2026):** Replaced 96B DUT strings (vendor/part/description) with
structured data: 16 self-describing rails (128B), project code (8B), BIM number (2B),
thermal config (6B). Project code (e.g., "S0026") maps to full customer/device info
in the LRM database — the board doesn't need ASCII strings.

**What's active (March 2026):**
- Header: auto-detect at boot, serial in ANNOUNCE, bim_type to host/GUI
- EEPROM read/write: CLI commands work end-to-end (0xA0-0xA3)
- WRITE_BIM (0x20): **full pipeline** — host `write_bim()` + firmware handler + CLI `write-bim`
- `rails[]` → **enforced** via `BoardConfig.check_vicor_voltage()` and `check_pmbus_voltage()` on every voltage command
- `voltage_cal[]` → **applied** to ADC readings via `BoardConfig.calibrate_voltage()`
- `current_cal[]` → **applied** to current readings via `BoardConfig.calibrate_current()`
- PMBUS_SET_VOLTAGE (0x87): **full pipeline** — host `pmbus_set_voltage()` + firmware safety check + CLI `pmbus-set-voltage`
- Clock configure: **re-enabled** — clk_ctrl AXI crash fixed March 25

**Remaining gaps:**
- `program_count` — incremented on write, not tracked by host.
- `thermal` — loaded at boot into Thermal controller, but heater/fan GPIO pins not yet identified on schematic.

### Runtime Override Architecture

EEPROM values are **production defaults** — programmed once per BIM, stable baseline.
At runtime, the host can override any parameter without touching the EEPROM:

```
EEPROM (factory)  ──┐
                     ├──▶ BoardConfig (effective values) ──▶ Power/ADC/Thermal
Host Overrides    ──┘     overrides win if set
```

**Override commands** (FBC protocol 0x31-0x34):
- `SET_OVERRIDE` (0x31): field_id + value → override one parameter
- `CLEAR_OVERRIDES` (0x32): revert all to EEPROM defaults
- `GET_EFFECTIVE` (0x33/0x34): request/response of merged config

**Override field IDs:**
| Range | Meaning |
|-------|---------|
| 0x01-0x08 | Rail 1-8 max_voltage_mv |
| 0x11-0x18 | Rail 1-8 min_voltage_mv |
| 0x21-0x28 | Rail 1-8 max_current_ma |
| 0x40-0x4F | Voltage cal offset channel 0-15 |
| 0x50-0x5F | Current cal offset channel 0-15 |
| 0x80 | Temperature setpoint (0.1°C) |

**Safety invariant:** Hardware maximum per BIM type (e.g., Normandy: 1500mV VICOR)
is NEVER overridable. The host can lower limits or raise them up to the hardware max,
but cannot exceed it. This prevents software bugs from destroying DUTs.

**On power cycle:** All overrides are lost. EEPROM defaults resume. This is intentional —
overrides are experimental, EEPROM is the known-good baseline.

**Key files:** `firmware/src/board_config.rs` (config merge layer),
`firmware/src/main.rs` (enforcement in VICOR/ADC handlers)

**Why this matters for autonomous operation:**
The EEPROM is the controller's local identity + configuration store. When a run starts, the
controller already knows: what device type it's testing (bim_type), what voltage limits are
safe (lcps_rails), what calibration to apply (voltage_cal/current_cal), and what DUT is
installed (vendor/part_number). Combined with vectors in DDR and test plan parameters, the
controller has everything needed to run a complete burn-in without the host. The host can
fine-tune via overrides, but the controller is never helpless without them.

This is the "don't make them dumb" philosophy — each board carries its own context.
The controller knows the device better than a human: it has accurate per-board calibration,
it enforces safe limits even when the host asks for something dangerous, and it runs
autonomously once initiated. The host supervises — it doesn't own the operation.

---

## Board LEDs

Same PCB (HPBIController) as Sonoma — same LEDs, different behavior.

| LED | Net | Color | Meaning |
|-----|-----|-------|---------|
| D4 | LED_3.3V | Green | 3.3V power good (always on when powered) |
| D2 | FPGA_DONE | Blue | PL bitstream loaded (Zynq DONE pin T12) |
| RJ45 (×2) | ETH_LED_G / ETH_LED_O | Green/Amber | Ethernet PHY link/activity (KSZ9021RNI) |

### FBC vs Sonoma LED Behavior

| Event | FBC | Sonoma |
|-------|-----|--------|
| Power on | Green (D4) instant | Green (D4) instant |
| Bitstream loaded | **Blue (D2) instant** — FSBL loads from BOOT.BIN | Blue (D2) after ~30s — Linux loads via `/dev/xdevcfg` |
| No Ethernet cable | **Still goes blue** — bitstream is in BOOT.BIN, no network needed | **Stays green forever** — `mount -a` hangs on NFS, `init.sh` never runs |
| No host software | **Still goes blue** — firmware runs standalone | **Stays green** — even if bitstream loads, `contact.sh` blocks forever on Everest |
| Bad SD card | No boot at all (BootROM fails) | Same — stays green |

**Key architectural difference:** FBC bakes the bitstream into `BOOT.BIN`. FSBL loads it
directly before firmware starts — no OS, no network, no scripts. Sonoma keeps `top.bit` as a
separate file on the SD card and loads it from Linux userspace via `cat /mnt/top.bit > /dev/xdevcfg`
inside `init.sh`, which depends on `mount -a` succeeding (including NFS mounts in fstab).

### Why This Matters: Independent Controllers vs Thin Clients

**Sonoma = thin client.** Every board depends on infrastructure to function. NFS server
down → entire rack is bricked. Everest not running → boards stuck at `contact.sh` forever.
The board cannot operate without the host. This made sense for centralized fleet updates
(change one file on NFS, all 44 boards get it), but creates a single point of failure.

**FBC = independent controller.** The host is required for initial setup and fleet
orchestration, but NOT for ongoing operation. The architecture:

1. **Host initiates.** Controller boots standalone, reads EEPROM (board identity, device
   config). Host connects, queries state, makes decisions — load vectors, set power profile,
   start a test plan, run diagnostics (continuity checks, individual vectors, debugging).

2. **Controller owns the run.** Once a test starts, the controller has everything it needs
   locally: test plan (from EEPROM), device config, vectors (in DDR), duration, limits.
   The host is no longer needed. If connection drops — **the controller doesn't care.**
   It continues the burn-in, stores telemetry and errors locally (flight recorder on SD),
   and finishes the designated time autonomously.

3. **Reconnection is non-destructive.** If the host reconnects mid-run, it doesn't reset
   anything. The controller reports its current state — what vector it's on, how long
   remaining, any errors logged — and the host refreshes its view. No interruption,
   no restart. (This should be extremely rare in practice.)

4. **Fault recovery is informed.** If a board faults or shuts down, the controller reports
   what happened — last vector, cycle count, error pins, power state, duration completed.
   The host has the full picture: what the firmware was doing, what the hardware state was,
   what failed. It diagnoses, reports to the engineer, and when the fix is applied, the
   host already knows the exact state of the run — resume from the same point with full
   device accuracy.

**The result:** Duration is known. Last vector is known. Error state is known. The entire
firmware and hardware state is known. The host automates what engineers currently do manually —
monitor boards, diagnose faults, track run progress, resume after interruption. Each controller
is a self-sufficient worker; the host is a supervisor that delegates, monitors, and handles
exceptions. Connection is a convenience for orchestration, not a requirement for operation.

---

## Firmware

**Language:** Rust, bare-metal (`#![no_std]`), no OS
**Boot time:** <1 second
**Source:** `firmware/src/`

### Boot Sequence

```
boot.S → set stack → clear BSS → enable VFP/NEON → enable I-cache → main()
  PHASE 1: Power Safety
    - Configure VICOR MIO pins [0,39,47,8,38,37] as GPIO output (SLCR unlock/lock)
    - Set all VICOR pins disabled (LOW)
  PHASE 2: System Init
    - OCM remap to HIGH (0xFFFC0000) — SLCR unlock required!
    - UART (115200), I2C, SPI
    - XADC init (CFG_ENABLE bit 31 + MCTL reset clear)
    - MAX11131 ADC init (SPI)
    - SD card init (flight recorder)
    - Interrupt controller (GIC)
  PHASE 3: Network
    - GEM0 init (100Mbps RGMII, MIO 16-27)
    - Override ps7_init GEM0_CLK from 125MHz→25MHz for 100M
    - Send ANNOUNCE broadcast packet
    - Enter main loop
```

**Critical boot bugs (all fixed):**
- OCM_CFG write silently dropped if SLCR locked → Data Abort accessing 0xFFFD0000
- I-cache must be enabled in boot.S (without it, CPU runs ~100x slower)
- SD byte writes to APB → alignment fault without MMU (fixed with 32-bit RMW)

### Main Loop

```
loop {
    poll_ethernet()       // Check for FBC packets
    update_handler()      // Process pending configs
    pending_requests()    // Slow ops (EEPROM, firmware, board_config overrides)
    state_transitions()   // Idle→Running→Done→Error
    heartbeat()           // Every 1s: state + temp + cycles + errors
    safety_monitor()      // Every ~500ms: temp/voltage/current limit checks
}
```

### Safety Monitor (autonomous, no GUI required)

The safety loop runs every ~500ms regardless of GUI connection. On violation, it
kills all power, latches the error, and broadcasts an ERROR packet. The host must
send RESET to clear the latch.

**Checks (in order):**
1. **Die temperature** — XADC millicelsius vs `BoardConfig.temp_shutdown_dc()` (default 150°C, per BIM type)
2. **XADC OT hardware flag** — catches sensor failures the software formula can't
3. **VICOR overvoltage** — reads actual output voltage, compares against `effective_rail().max_voltage_mv + 100mV` transient margin

**On violation:**
- `vicor.disable_all()` — all 6 VICOR cores off
- `psu_mgr.disable_all()` — all PMBus/LCPS rails off
- `safety_tripped = true` — latched until host sends RESET command
- Broadcasts ERROR packet (type 1=over-temp, 2=over-voltage) so any listener sees it

**RESET clears the latch:** `main.rs:358-361` — `handler.take_pending_reset()` → `safety_tripped = false`.
Protocol handler sets `pending_reset = true` on RESET command (`fbc_protocol.rs:1498`).

**Why this runs on firmware, not GUI:** The GUI might not be connected. The Ethernet
cable might be unplugged. The host PC might be rebooting. The controller MUST protect
itself and the DUT autonomously. This is the "don't make them dumb" philosophy applied
to safety — the board doesn't need permission to shut down when something is dangerous.

**Key file:** `firmware/src/main.rs` (safety_counter loop at end of main loop)

### Thermal Controller (ONETWO Crystallization Feedforward) — v2

**File:** `firmware/src/hal/thermal.rs` (340 lines)

**CORRECTED March 24:** There is NO external Watlow controller on Sonoma. The ARM CPU directly
PWMs heater/cooler FETs on the BIM through GPIO pins. `linux_set_temperature.elf` is a PID loop
binary that reads NTC → computes correction → writes HEATER_SW/COOLER_SW GPIOs. The BIM has
STD16NF06LT4 N-ch MOSFETs and a hardware comparator (safety cutoff), but all decisions are firmware.
**Same architecture as what we're building** — the only difference is the control algorithm.

**Physical thermal path (same on Sonoma and FBC):**
```
NTC thermistor → MAX11131 ch22 → ARM reads temp
                                      ↓
                          Thermal controller
                          (Sonoma: PID in ELF / FBC: crystallization v2)
                                      ↓
                          HEATER_SW / COOLER_SW GPIO → BIM connector
                                      ↓
                          STD16NF06LT4 N-ch MOSFETs on BIM
                                      ↓
                          Physical heater / cooler elements
```

**v2 Control Law (P + I with crystallization decay + D + feedforward):**
1. **Before vectors run:** `estimate_power()` XORs consecutive vector pairs, counts pin toggles.
   High toggle rate + many active pins = `PowerLevel::High` → applies feedforward correction
   BEFORE temperature moves. Feedforward values: Medium=-60, High=-180.
2. **During run:** `update(actual_mc)` takes real temperature from MAX11131 THERM_CASE ch 22
   (falls back to XADC die temp). Computes P + I + D correction:
   - **P term:** `KP=15 × error` — immediate proportional response
   - **I term:** `KI=3 × integral` — integral **decays** at rate (e - 2) ≈ 0.718 per iteration
     (this IS the crystallization — bounded convergence, never windup). Clamped to ±500.
   - **D term:** `KD=5 × (error - prev_error)` — damping against overshoot
3. **Crystallizes** via integral decay — settles to stable output without oscillation.
4. **Outputs:** `output_to_heater()` and `output_to_fan()` produce duty cycle values (0-100%).

**v1 → v2 upgrade:** v1 was a pure integrator (`output += error × 0.718`) that oscillated ±26.6°C
indefinitely (proven by `scripts/thermal_sim.py` simulation). v2 fixes this by making (e - 2) the
integral decay factor instead of a raw gain, and adding P+D terms for damping.

**Simulation results** (`scripts/thermal_sim.py`, 300s burn-in physics: 2 J/°C mass, 50W heater, 15W fan):

| Controller | Overshoot | Settle Time | Steady-State |
|------------|-----------|-------------|--------------|
| v1 (pure integrator) | +26.6°C | Never (oscillates forever) | ±26.6°C oscillation |
| Standard PID (Kp=8,Ki=0.5,Kd=2) | +3.1°C | 11.4s | Stable |
| **v2 Crystallization** | **+0.3°C** | **Instant** | **Stable** |

**Available inputs (current + planned):**

| Input | Used? | How |
|-------|-------|-----|
| THERM_CASE (ch 22) | Yes | Primary temperature feedback |
| Vector toggle analysis | Yes | Feedforward via `estimate_power()` |
| THERM_DUT (ch 23) | Not yet | Could track DUT junction vs case delta |
| I_CORE1/I_CORE2 (ch 24-25) | Not yet | Direct power measurement — V×I = actual watts |
| VDD_CORE1-6 (ch 16-21) | Not yet | Combined with current = real-time power |
| XADC die temp | Fallback only | FPGA self-protection |
| CMP_IN1 hardware comparator | Not yet | Hardware safety cutoff (independent of firmware) |

**Compile-time thermal profiling (IMPLEMENTED — 25/25 tests pass):**
Both encoders (`gen_fbc.c` + `compiler.rs`) now analyze toggle rate per segment during compression
as a free byproduct — XOR + popcount of consecutive vectors, bucketed every 1024 vectors.
The THERMAL_PROFILE section is appended after OP_END in the .fbc file:
```
Header changes:
  [7]      flags        bit 0 = FBC_FLAG_THERMAL_PROFILE (0x01)
  [24:28]  _reserved    segment_count (u32 LE, was reserved/zero)

THERMAL_PROFILE (after OP_END, N × 8 bytes):
  per segment:
    vector_offset: u32   (which vector this segment starts at)
    avg_toggle_rate: u8  (avg toggles per vector, 0-160)
    avg_active_pins: u8  (avg active pins, 0-160)
    power_level: u8      (0=Low, 1=Medium, 2=High — same thresholds as thermal.rs)
    reserved: u8
```
**Files:** `gen_fbc.c:58-153` (C ThermalAccum), `compiler.rs:46-110` (Rust ThermalAccum),
`format.rs:497-644` (ThermalSegment/ThermalProfile types + FbcFile reader).
**Backward compatible:** old readers stop at OP_END and ignore trailing bytes. New readers
check `flags & 0x01` and read segment_count from `_reserved[0:4]`.
CRC32 covers header+pin_config+data (including thermal profile bytes).

Firmware reads this at load time → builds feedforward schedule BEFORE first vector fires.
Combined with real-time V×I power from core supplies during run = closed-loop with prediction.

**Architecture advantage:** Sonoma's PID in `linux_set_temperature.elf` is purely reactive — it
sees a temperature spike 2-5s after vectors start hammering 60 pins, then slowly corrects.
FBC thermal.rs knows the power profile at compile time AND reads real-time core power during run.

**Status: FULLY WIRED March 27 (simulation validated, thermal DAC routing complete)**
- `Thermal` instance created at boot, target set from `board_config.temp_setpoint_dc()`
- Safety loop feeds `thermal.update()` with THERM_CASE temperature (MAX11131 ch 22, falls back to XADC die temp)
- `output_to_heater()` / `output_to_fan()` → BU2505 DAC ch 1 (heater) / ch 0 (cooler)
- **Not GPIO-driven** — thermal uses the same BU2505FV DAC as VICOR voltage setpoints. DAC output → comparator on BIM → STD16NF06LT4 FET gate. Duty 0-100% → 0-4100mV.
- Thermal target updates live when host sends SET_OVERRIDE (field 0x80)
- Per-step temp setpoint in test plan: `temp_setpoint_dc` field (0x7FFF = no change)
- **V×I power feedback (March 27):** `read_core_power_mw()` reads VDD_CORE1×I_CORE1 + VDD_CORE2×I_CORE2 (MAX11131 ch 16-17, 24-25), extrapolates ×3 for all 6 cores, maps to PowerLevel (Low/Medium/High) → feeds `thermal.set_power_level()` → adjusts feedforward offset every 500ms. Compile-time profile gives initial estimate, V×I corrects in real-time.

### HAL Drivers + Application Modules

| Module | Location | Notes |
|--------|----------|-------|
| gpio | hal/gpio.rs | MIO + EMIO pin read/write/direction |
| slcr | hal/slcr.rs | MIO mux, clock config, SLCR unlock/lock |
| xadc | hal/xadc.rs | 16ch internal ADC (die temp, VCCINT/AUX/BRAM, 8 aux) |
| i2c | hal/i2c.rs | I2C0/1 — EEPROM, PMBus |
| spi | hal/spi.rs | SPI0 — DAC (BU2505) + ADC (MAX11131) |
| uart | hal/uart.rs | Debug console 115200 8N1 |
| sd | hal/sd.rs | Flight recorder (fixed — 32-bit RMW for APB) |
| vicor | hal/vicor.rs | 6 VICOR cores via DAC + MIO enable |
| pmbus | hal/pmbus.rs | PMBus/LCPS low-current rails |
| eeprom | hal/eeprom.rs | 24LC02 I2C EEPROM — board identity + rail limits + cal |
| max11131 | hal/max11131.rs | 16ch external SPI ADC, 12-bit |
| bu2505 | hal/bu2505.rs | 10ch external SPI DAC, 10-bit |
| dna | hal/dna.rs | Zynq Device DNA unique ID |
| pcap | hal/pcap.rs | FPGA reconfig (unused — available) |
| gic | hal/gic.rs | Interrupt controller, FPGA → ARM IRQ routing |
| analog | analog.rs | Unified 32-ch monitor (XADC + MAX11131), formula engine, runtime switching |
| board_config | board_config.rs | EEPROM defaults + host overrides → effective config, rail enforcement, ADC cal |
| fbc_loader | fbc_loader.rs | .fbc parse → clock → pins → OEN → decompress → DMA |
| fbc_decompress | fbc_decompress.rs | .fbc compressed opcodes → 32-byte FPGA instruction words |
| thermal | hal/thermal.rs | ONETWO crystallization-based feedforward thermal controller (340 lines) — **wired into main.rs safety loop (deployed March 25)** |
| flight_recorder | flight_recorder.rs | Corruption-resistant SD log (dual headers + CRC32) |

### Vector Pipeline

Two formats serve different layers — compression for storage/transfer, ISA for hardware execution:

```
.fbc file (compressed)              FPGA bytecode (instruction words)
─────────────────────               ───────────────────────────────────
0x01 VECTOR_FULL (20B)              0xC1 SET_OEN  + 128-bit OEN mask ← derived from pin_config
0x02 VECTOR_SPARSE (N bytes)   →    0xC0 SET_PINS + 128-bit payload
0x03 VECTOR_RUN (4B count)     →    0xB5 PATTERN_REP (count in operand)
0x04 VECTOR_ZERO (1B)          →    0xC0 SET_PINS + all zeros
0x05 VECTOR_ONES (1B)          →    0xC0 SET_PINS + all ones
0x07 END                       →    0xFF HALT
```

**Data flow:**

```
Host: STIL/ATP → gen_fbc.c → .fbc file (compressed, 710x ratio)
  ↓ Ethernet (UPLOAD_VECTORS, chunked)
Firmware: .fbc in DDR → fbc_loader.rs parses header
  ↓ fbc_decompress.rs: walks compressed opcodes
  ↓ Emits: SET_OEN (from pin_config) + SET_PINS per vector + PATTERN_REP for runs + HALT
  ↓ Each instruction = 32 bytes (256-bit word, DMA-aligned)
DMA: fbc_dma.v reads 4×64-bit beats per instruction from OCM
  ↓ AXI-Stream (256-bit)
RTL: axi_stream_fbc.v splits [63:0] instr + [191:64] payload
  ↓
RTL: fbc_decoder.v state machine executes opcodes
  ↓ vec_dout[127:0] + vec_oen[127:0]
RTL: io_bank.v → io_cell.v per pin → physical GPIO
```

**Key files:**
- `firmware/src/fbc_loader.rs` — orchestrates: header → clock → pins → OEN → decompress → DMA
- `firmware/src/fbc_decompress.rs` — .fbc opcodes → 32-byte FPGA instruction words
- `firmware/src/fbc.rs` — FbcInstr packing (opcode in bits [63:56])
- `firmware/src/dma.rs` — FbcStreamer + AxiDma + DmaBuffer
- `rtl/fbc_dma.v` — MM2S DMA, 32-byte bursts, AXI-Lite register interface
- `rtl/axi_stream_fbc.v` — 256-bit AXI-Stream → 64-bit instr + 128-bit payload
- `rtl/fbc_decoder.v` — 7-state machine, executes SET_PINS/SET_OEN/PATTERN_REP/WAIT/HALT
- `gui/src-tauri/c-engine/pc/gen_fbc.c` — .fbc file compression (C pattern converter)

**Critical design note:** The fbc_decoder resets `current_oen` to all 1s (all tristate).
The loader MUST emit a SET_OEN instruction before any SET_PINS, otherwise no BIDI/OUTPUT
pin will drive. The OEN mask is derived from pin_config: OUTPUT/BIDI/PULSE/VEC_CLK → drive,
INPUT → tristate.

**DMA buffer:** 0xFFFC_0000 (OCM, 64KB). GEM0 descriptors start at 0xFFFD_0000. No overlap.

### SD Pattern Storage + DDR Double-Buffer

**Files:** `firmware/src/ddr_slots.rs` (454 lines), `host/src/lib.rs` (6 methods), `host/src/types.rs`

**Architecture (replaced fixed 8-slot model March 29):**
- **SD card** holds ALL patterns for the device (up to 256 .fbc files, 16GB+ capacity)
- **DDR** has two regions: ACTIVE (FPGA reads from) + STAGING (next pattern loads into)
- Firmware loads SD → DDR on step transitions (~60ms for 3MB pattern)
- Board is fully autonomous — no PC needed during 500-hour test
- Real projects: Cisco C512 = 107 patterns, Tesla Dojo = 357, Cayman DCM = 36

**DDR Layout (512MB total):**
```
0x0010_0000 - 0x002F_FFFF  Firmware (2MB)
0x0030_0000 - 0x0030_0FFF  Metadata (4KB — checkpoint, pattern table cache)
0x0030_1000 - 0x0030_1FFF  Plan checkpoint (persists across warm reset)
0x0040_0000 - 0x0FFF_FFFF  DDR Region A (252MB) — active or staging
0x1000_0000 - 0x1FFF_FFFF  DDR Region B (256MB) — staging or active
```

**SD Layout (sectors):**
```
0-7:         SD header (magic 0x46425344 "FBSD", pattern count, BIM serial)
8-2047:      Pattern directory (256 entries × 16B: start_sector, size, num_vectors, vec_clock_hz)
2048-4095:   Flight recorder (existing)
4096+:       Pattern data (sequential .fbc files)
```

**Boot flow:**
```
Boot → read EEPROM project_code → load SD pattern directory into RAM
Plan start → load pattern[step.pattern_id] from SD → DDR Region A → parse → DMA → execute
Step transition → load next pattern → DDR staging → swap active/staging → execute
```

**Upload flow:** Host sends patterns via Ethernet, firmware writes to SD:
```
Host: upload_to_slot(mac, pattern_id, fbc_data)
  → chunked at 1400B: [pattern_id:u8][offset:u32][total:u32][chunk_len:u16][data...]
  → firmware writes chunks to SD card sectors
  → pattern directory updated on completion
```

**CLI commands:** `slot-upload` (writes to SD), `slot-status` (reads pattern directory), `slot-invalidate`

### Test Plan Executor — Autonomous Burn-In

**Files:** `firmware/src/testplan.rs` (647 lines), `host/src/types.rs` (TestPlanDef, PlanStatus)

The firmware runs multi-step burn-in autonomously once the host sends a plan. Operator walks
away for 500+ hours. Each step references a DDR slot, has its own duration, fail action, and
error threshold.

**State machine:**
```
SET_PLAN → Idle
RUN_PLAN → Loading (load slot 0 from DDR)
         → Running (vectors executing, monitoring errors)
         → StepDone (vectors complete for this step)
           ├─ errors > threshold AND fail_action=Abort → Aborted
           └─ next step exists → Loading (load next slot)
              └─ no more steps AND duration remaining → loop from loop_start
                 └─ duration expired → Complete
```

**Plan definition (host → firmware wire format):**
```
[num_steps:u8][loop_start:u8][total_duration_secs:u32 BE]
Per step (13 bytes each):
  [pattern_id:u8][duration_secs:u32 BE][fail_action:u8][error_threshold:u32 BE]
  [temp_setpoint_dc:i16 BE][clock_div:u8]
```

**Key fields:**
- `loop_start` — step index to resume from on subsequent passes (skip init/continuity after first pass)
- `total_duration_secs` — 0 = single pass; 1800000 = 500 hours (500 × 3600)
- `fail_action` — Abort (0) or Continue (1) per step
- `error_threshold` — per-step error tolerance (0 = any error triggers fail_action)
- `temp_setpoint_dc` — temperature in 0.1°C units (1250 = 125.0°C). 0x7FFF = no change from previous step
- `clock_div` — vector clock (0=5MHz, 1=10MHz, 2=25MHz, 3=50MHz, 4=100MHz). 0xFF = no change

**Checkpoint persistence:** `plan_executor.checkpoint_to_ddr()` saves state (elapsed_ms,
current_step, loop_count, bim_serial) to DDR at 0x0030_1000 every step transition. On warm
reset, `read_checkpoint_from_ddr()` resumes where it left off.

**End-to-end flow:**
```bash
fbc-cli fbc -i Ethernet -a <MAC> slot-upload 0 continuity.fbc
fbc-cli fbc -i Ethernet -a <MAC> slot-upload 1 init.fbc
fbc-cli fbc -i Ethernet -a <MAC> slot-upload 2 stress.fbc
fbc-cli fbc -i Ethernet -a <MAC> set-plan plan.json
fbc-cli fbc -i Ethernet -a <MAC> run-plan
fbc-cli fbc -i Ethernet -a <MAC> plan-status   # poll this
```

**Plan JSON example:**
```json
{
  "num_steps": 3,
  "loop_start": 2,
  "total_duration_secs": 1800000,
  "steps": [
    { "pattern_id": 0, "duration_secs": 60,   "fail_action": "abort",    "error_threshold": 0, "temp_setpoint_dc": 250, "clock_div": 1 },
    { "pattern_id": 1, "duration_secs": 120,  "fail_action": "abort",    "error_threshold": 0, "temp_setpoint_dc": 250, "clock_div": 3 },
    { "pattern_id": 2, "duration_secs": 3600, "fail_action": "continue", "error_threshold": 100, "temp_setpoint_dc": 1250, "clock_div": 3 }
  ]
}
```
Step 0 (continuity @ 25.0°C, 10MHz) and step 1 (init @ 25.0°C, 50MHz) run once.
Step 2 (stress @ 125.0°C, 50MHz) loops for 500 hours. Omit temp/clock fields → no change from previous step.
If continuity or init fail → abort. If stress gets <100 errors per pass → continue.

**Why this matters:** Sonoma has no autonomous execution — `RunSuperVector.elf` runs one vector
set for N seconds, then the AWK script decides what to do next. If the host loses connection,
the board is stuck. FBC's plan executor runs the entire test sequence independently.

### Memory Layout

| Region | Address | Size | Purpose |
|--------|---------|------|---------|
| DDR — Firmware | 0x0010_0000 | 2MB | Code + stack + heap |
| DDR — Metadata | 0x0030_0000 | 4KB | Pattern table cache, checkpoint |
| DDR — Plan Checkpoint | 0x0030_1000 | 4KB | Plan executor state (survives warm reset) |
| DDR — Region A | 0x0040_0000 | 252MB | Active or staging (double-buffer) |
| DDR — Region B | 0x1000_0000 | 256MB | Staging or active (double-buffer) |
| AXI GP0 | 0x4004_0000 | 24KB | FPGA peripherals |
| AXI DMA | 0x4040_0000 | 4KB | DMA engine |
| OCM | 0xFFFC_0000 | 256KB | GEM0 DMA descriptors + TX/RX buffers |
| Stack | 0x1010_0000 | — | Grows down |

---

## Protocol

**Wire format:** Raw Ethernet (0x88B5), no TCP/IP
**Byte order:** Big-endian payloads
**Timeouts:** 500ms commands, 1s firmware update
**Detail:** See `docs/PROTOCOL.md` for full payload structures

### Packet Header (8 bytes)

```
magic:  u16 = 0xFBC0
seq:    u16     (sequence number)
cmd:    u8      (command code)
flags:  u8
length: u16     (payload length)
```

### 56 Operations (79 wire codes) across 13 Subsystems

All command codes defined in `firmware/src/fbc_protocol.rs:27-132`.

| Subsystem | Operations | Wire Codes | Host Methods |
|-----------|-----------|------------|-------------|
| Setup (5) | ANNOUNCE, BIM_STATUS, WRITE_BIM, UPLOAD_VECTORS, CONFIGURE | 0x01, 0x10-0x11, 0x20-0x21, 0x30 | `discover`, `upload_vectors`, `write_bim`, `configure` |
| Board Config (5) | SET_OVERRIDE, CLEAR_OVERRIDES, GET_EFFECTIVE, IO_BANK_SET | 0x31-0x36 | (not yet in host crate) |
| Runtime (8) | START, STOP, RESET, HEARTBEAT, ERROR, STATUS, MIN_MAX | 0x40-0x42, 0x50, 0xE0, 0xF0-0xF3 | `start`, `stop`, `reset`, `get_status` |
| Analog (1) | READ_ALL | 0x70-0x71 | `read_analog` |
| Power (9) | VICOR_STATUS, VICOR_ENABLE, VICOR_SET_VOLTAGE, PMBUS_STATUS, PMBUS_ENABLE, PMBUS_SET_VOLTAGE, EMERGENCY_STOP, POWER_SEQ_ON, POWER_SEQ_OFF | 0x80-0x87, 0x8F-0x91 | `get_vicor_status`, `set_vicor_enable`, `set_vicor_voltage`, `get_pmbus_status`, `set_pmbus_enable`, `pmbus_set_voltage`, `emergency_stop`, `power_sequence_on/off` |
| EEPROM (2) | READ, WRITE | 0xA0-0xA3 | `read_eeprom`, `write_eeprom` |
| Vector Engine (6) | STATUS, LOAD, START, PAUSE, RESUME, STOP | 0xB0-0xB7 | `get_vector_status`, `start_vectors`, `pause_vectors`, `resume_vectors`, `stop_vectors` |
| Fast Pins (2) | READ, WRITE | 0xD0-0xD2 | `get_fast_pins`, `set_fast_pins` |
| Error Log (1) | READ | 0x4A-0x4B | `get_error_log` |
| Flight Recorder (4) | LOG_READ, LOG_INFO, SD_FORMAT, SD_REPAIR | 0x60-0x67 | `read_log_sector`, `get_log_info`, `sd_format`, `sd_repair` |
| Firmware Update (5) | INFO, BEGIN, CHUNK, COMMIT, ABORT | 0xE1-0xE9 | `get_firmware_info`, `firmware_update` |
| **DDR Slots (4)** | UPLOAD_TO_SLOT, SLOT_STATUS_REQ/RSP, INVALIDATE | 0x22-0x25 | `upload_to_slot`, `get_slot_status`, `invalidate_slot` |
| **Test Plan (6)** | SET_PLAN, SET_PLAN_ACK, RUN_PLAN, RUN_PLAN_ACK, PLAN_STATUS_REQ/RSP, STEP_RESULT | 0x26-0x2C | `set_test_plan`, `run_test_plan`, `get_plan_status` |

---

## AXI Register Map

**Detail:** See `docs/register_map.md` for full register definitions

| Peripheral | Base | Size | Purpose |
|------------|------|------|---------|
| axi_fbc_ctrl | 0x4004_0000 | 4KB | FBC decoder control |
| io_config | 0x4005_0000 | 4KB | Pin type config (4-bit per pin) |
| axi_vector_status | 0x4006_0000 | 4KB | Vector engine status |
| axi_freq_counter | 0x4007_0000 | 4KB | 8-channel frequency counter |
| clk_ctrl | 0x4008_0000 | 4KB | Clock control |
| error_bram | 0x4009_0000 | 4KB | Error log (3x BRAM) |
| axi_device_dna | 0x400A_0000 | 4KB | 57-bit silicon ID (DNA_LO/HI/STATUS, read-only) |
| fbc_dma | 0x4040_0000 | 4KB | AXI DMA |

### Clock Architecture

```
33.333 MHz oscillator → MMCM
  clk_100m (100 MHz) → AXI bus
  clk_200m (200 MHz) → vector timing
  vec_clk (selectable) → vector execution
    freq_sel: 0→5MHz, 1→10MHz, 2→25MHz, 3→50MHz, 4→100MHz
```

Phase clocks (CLKOUT5/6) hardwired at 50MHz — don't follow freq_sel.

### FBC Instruction Format

Each instruction is a **256-bit (32-byte) word** in the DMA buffer:

```
Bytes 0-7:   64-bit instruction word
               [63:56] opcode
               [55:48] flags (LAST=0x01, IRQ=0x02, LOOP=0x04)
               [47:0]  operand
Bytes 8-23:  128-bit payload (pin values for SET_PINS/SET_OEN)
Bytes 24-31: reserved (zeros)
```

fbc_dma.v reads 4 × 64-bit HP0 beats per word. axi_stream_fbc.v extracts
`[63:0]` as instruction and `[191:64]` as 128-bit payload.

**Opcodes (from `rtl/fbc_pkg.vh`):**

| Opcode | Value | Payload | Description |
|--------|-------|---------|-------------|
| NOP | 0x00 | — | No operation |
| LOOP_N | 0xB0 | — | Loop N times (BROKEN — no instruction buffer) |
| PATTERN_REP | 0xB5 | — | Repeat current vector `operand[31:0]` times |
| SET_PINS | 0xC0 | 128-bit | Set pin output values |
| SET_OEN | 0xC1 | 128-bit | Set output enables (0=drive, 1=tristate) |
| SET_BOTH | 0xC2 | — | **S_ERROR** — 256-bit payload won't fit 128-bit bus |
| WAIT | 0xD0 | — | Wait `operand[31:0]` cycles |
| HALT | 0xFF | — | Stop execution |

---

## Verified Status (March 25, 2026)

### PL (FPGA) — DONE

| What | Status | Notes |
|------|--------|-------|
| Bitstream | ✅ Loaded | `build/fbc_system.bit` — rebuilt March 25 with full timing closure (WNS=+0.018ns, TNS=0.000ns). All 8 AXI peripherals including `axi_device_dna.v`. Deployed via JTAG. |
| Timing closure | ✅ Met | WNS=+0.018ns at 200MHz (IO domain), 100MHz (AXI domain). CDC constraints: `set_clock_groups -logically_exclusive` for MMCM clocks, `set_false_path` for all cross-domain paths. Local `vec_clk_cnt` register in `io_cell.v` broke 5.5ns routing path. |
| 8 AXI peripherals | ✅ Accessible | All base addresses respond — including `axi_device_dna` at 0x400A_0000 (was missing from March 12 bitstream) |
| fbc_decoder | ✅ Instantiated | 7 opcodes, state machine operational |
| vector_engine | ✅ Wired | Error counting, repeat logic |
| fbc_dma | ✅ Wired | HP0 DDR→AXI-Stream path exists |
| io_bank (160 pins) | ✅ Wired | 128 BIM + 32 fast, io_cell per pin |
| error_bram (3x) | ✅ Wired | Pattern/vector/cycle BRAMs at 0x4009_0000 |

**RTL limitations (won't fix now):**
- LOOP_N non-functional (no instruction buffer for replay — unroll in bytecode)
- SET_BOTH → S_ERROR (128-bit payload bus can't carry 256 bits; use SET_PINS + SET_OEN)
- Phase clocks hardwired at 50MHz (don't follow freq_sel)
- 4 opcodes unimplemented (SYNC, IMM32, IMM128, PATTERN_SEQ)

**Bugs found & fixed (March 23, 2026):**
- fbc_decompress.rs: instructions were 8-24 bytes instead of 32 → DMA misalignment (garbled data)
- fbc_loader.rs: no SET_OEN emitted → all pins tristated (decoder resets OEN to all 1s)
- fbc_dma.v S_STREAM_OUT: bytes_left/current_addr decremented every cycle instead of on handshake
- analog.rs ch 0: double-conversion (raw→millicelsius→deciKelvin→pseudo-raw→formula) lost precision → direct UG480 formula
- analog.rs: XADC voltage channels used `scale/65536` but correct divisor is `4096` for MAX11131 (XADC uses `/65536`)

### Firmware — DONE (with known bugs)

| What | Status | Notes |
|------|--------|-------|
| Boot to main loop | ✅ | <1s, SVC mode, IRQ enabled, I-cache on |
| GEM0 Ethernet | ✅ | 100M RGMII, ANNOUNCE sent at boot |
| SLCR/OCM | ✅ | OCM remapped, SLCR unlock/lock correct |
| VICOR GPIO | ✅ | 6 MIO pins configured [0,39,47,8,38,37], enable/disable works |
| GIC interrupts | ✅ | FPGA→ARM IRQ path wired |
| Flight recorder | ✅ | SD init works, `sd_init_ok` guard prevents crash when no card present |
| XADC init | ✅ | Formula fixed (was double-converting through deciKelvin; now direct UG480) |
| MAX11131 | ⚠️ | Init code fixed, untested on real ADC hardware |
| BoardConfig | ✅ | EEPROM defaults + host overrides, rail enforcement, ADC calibration |
| Safety monitor | ✅ | Over-temp + overvoltage → emergency stop, autonomous (no GUI needed) |
| Analog formulas | ✅ | 6 formula types, runtime switching per channel, VicorCurrent added. **NTC thermistor upgraded (deployed March 25):** proper B-equation + ln_approx(), Sonoma-matched divider (4980Ω pulldown + 150Ω series), 30kΩ default with `set_ntc_type()` runtime switch, `read_case_temp_mc()`/`read_dut_temp_mc()` convenience methods |
| Calibration | ✅ | EEPROM voltage_cal[16] + current_cal[16] applied to every read |

**XADC bugs (two found, both fixed):**
1. (March 23) Double-conversion: `read_temperature_millicelsius()` → deciKelvin → pseudo-raw → DieTemp formula. Fixed: channel 0 now returns actual XADC 16-bit register via `read_temperature_raw()`, DieTemp formula applies UG480 directly.
2. (March 24) **u32 arithmetic overflow:** `raw * 503975` overflows u32 for any raw > 8522. Expected raw ~40955 × 503975 = 20.6B, u32 max = 4.29B → wraps → shows -220°C. Fixed: `as u32` → `as u64`. Sonoma doesn't hit this — Linux kernel IIO driver uses 64-bit math.

**Unconfirmed:** Whether the "raw ~6843" in earlier reports was directly observed (xsdb, which uses TCL bigint and wouldn't overflow) or back-calculated from the firmware's -220°C output. Both raw=40955 (u32 overflow) and raw=6843 (real) produce ~-220°C. **Deploy will resolve this** — if temp reads ~42°C, the overflow was the entire bug.

### CLI Commands — 21 TESTED on Hardware (March 24-25)

Full command sweep on calibration board (12V/0.22A, no DUT/BIM/48V/LCPS).
19 pass, 2 expected timeouts (correct behavior). All crashes fixed.
SD crash, pmbus-status timeout, and clk_ctrl crash all FIXED.

| Command | Result | Notes |
|---------|--------|-------|
| discover | ✅ PASS | MAC 00:0A:35:C6:B4:2A (DNA-derived unique), serial 3B10B42A, FW 1.0 |
| ping | ✅ PASS | 1.5ms RTT |
| status | ✅ PASS | Idle, 0 cycles, temp=39.5°C ✅ (was -223°C before u64 fix) |
| firmware-info | ✅ PASS | v1.0.0, build 2026-02-10, HW rev 1 |
| vector-status | ✅ PASS | Idle, 0 vectors/errors |
| errors | ✅ PASS | 8 entries, all zeroed (correct) |
| analog | ✅ PASS | 32 channels, all 0mV (no DUT, expected) |
| log-info | ✅ PASS | SD not present, 0 entries |
| vicor | ✅ PASS | 6 cores disabled, 0mV (no 48V, expected) |
| eeprom | ✅ PASS | Empty (no BIM, expected) |
| fastpins read | ✅ PASS | din/dout/oen = 0x00000000 |
| set-fastpins | ✅ PASS | Wrote dout=0xDEADBEEF oen=0xFFFFFFFF |
| fastpins readback | ✅ PASS | **Round-trip verified**: dout=0xDEADBEEF confirmed |
| configure | ✅ PASS | ACK received, clock write works (clk_ctrl AXI crash FIXED in March 25 bitstream) |
| emergency-stop | ✅ PASS | Immediate ACK |
| stop | ✅ PASS | Stopped |
| pause | ⏱️ TIMEOUT | Expected — state is Idle, not Running (handler returns None) |
| resume | ⏱️ TIMEOUT | Expected — state is Idle, not Paused (handler returns None) |
| pmbus-status | ✅ FIXED | Was timeout (no handler) — dispatch + handler added March 24 |
| read-log | ⏱️ TIMEOUT | No SD card on cal board |
| sd-repair | ✅ FIXED | Was crash (uninitialized SDHCI) — `sd_init_ok` guard added, returns error safely |

### Bugs Found During March 24 Testing

| # | Bug | Severity | Status |
|---|-----|----------|--------|
| 1 | ~~**clk_ctrl AXI crash**~~ — writing to 0x4008_0000 caused Data Abort | ~~HIGH~~ | ✅ FIXED — root cause: incomplete `case` statements in AXI write FSM (missing `default:` arms). Fixed in `clk_ctrl.v`, verified on hardware March 25: `configure --clock 3` succeeds, board survives. |
| 2 | ~~**SD commands crash**~~ — was using uninitialized SDHCI | ~~HIGH~~ | ✅ FIXED — `sd_init_ok` guard added |
| 3 | ~~**XADC temp wrong**~~ — u32 overflow (`as u32` → `as u64`). | ~~LOW~~ | ✅ FIXED — verified on hardware March 25: reads 39.5°C (was -220°C). |
| 4 | **pause/resume timeout when Idle** — CLI shows "Timeout" instead of useful message | LOW | UX improvement needed |

### Not Yet Tested on Real Hardware

| What | Why | Priority |
|------|-----|----------|
| Vector upload + run + GPIO toggle | Pipeline wired (decompressor+OEN+DMA fixed), need 48V/tray for BIM pins. Fast pins (128-159) could run on 12V only but no DUT to verify | **NEXT** |
| ~~clk_ctrl AXI register read/write~~ | ~~Write crashes board~~ | ✅ **FIXED** — `configure --clock 3` works, board survives |
| VICOR with real load | Cal board has no DUT, no 48V | After BIM board |
| PMBus control | No LCPS on cal board | After rack setup |
| Firmware update pipeline | Code exists, untested | Low |
| Full 160-pin config with BIM | No BIM on cal board | After BIM board |
| Multi-board orchestration | Only 1 board on bench | After vector test |

### Current Status (March 25)

**Board is ON and fully operational.** New bitstream with all 8 AXI peripherals deployed. All major bugs resolved.

**Verified on hardware this session:**
- ✅ **Bitstream rebuilt** — Vivado synthesis with `axi_device_dna.v`, full timing closure (WNS=+0.018ns)
- ✅ **BOOT.BIN packaged** — FSBL + bitstream + firmware ELF, deployed via JTAG
- ✅ **DNA-based unique MAC** — `00:0A:35:C6:B4:2A` (was `00:0A:35:AD:00:02` CPU ID fallback)
- ✅ **XADC temperature** — reads 39.5°C (confirms u64 overflow fix — was -220°C)
- ✅ **clk_ctrl AXI write** — `configure --clock 3` succeeds, board survives (was Data Abort crash)
- ✅ **160/160 pin assignments verified** — XDC matches Sonoma reference exactly (all banks, drive strengths, I/O standards)

**Next steps:**
1. **First vector run** — needs 48V + VICOR power for BIM pins. Fast pins (128-159) work on 12V but no DUT to verify against
2. **Thermal GPIO routing** — identify HEATER_SW/COOLER_SW pin numbers from J3c schematic
3. **Multi-board test** — rack setup with multiple boards
4. **GUI integration** — wire verified CLI commands to native app panels

### Recently Fixed (March 24-25)

- ✅ **SD crash** — was uninitialized SDHCI, not byte writes. `sd_init_ok` guard added.
- ✅ **PMBUS_STATUS_REQ (0x84)** — dispatch arm + handler added. No more silent drop.
- ✅ **Vector LOAD/START/STOP (0xB2/0xB4/0xB7)** — dispatch arms added. LOAD returns 0xFF (needs SD cache), START/STOP delegate to runtime handlers.
- ✅ **MAC collision** — all boards had identical MAC from ARM MIDR. Added `axi_device_dna.v` (DNA_PORT → AXI at 0x400A_0000). Each board now gets unique MAC from silicon DNA. **Verified on hardware:** MAC `00:0A:35:C6:B4:2A` (unique per chip).
- ✅ **DNA guard** — firmware checks FBC_CTRL VERSION at `0x4004_001C` (not offset 0x00 which is CTRL, always 0 at reset). Falls back to CPU ID MAC if VERSION=0 or 0xFFFFFFFF.
- ✅ **clk_ctrl AXI crash (bug #22)** — root cause: incomplete `case` statements in `clk_ctrl.v` AXI write FSM. Unhandled address ranges caused undefined behavior. Fix: added `default: ;` to both write case statements. Verified on hardware: `configure --clock 3` succeeds, board survives.
- ✅ **XADC temperature** — u32 overflow confirmed as entire bug. Reads 39.5°C on hardware (was -220°C).
- ✅ **Bitstream rebuilt with timing closure** — WNS went from -7.227ns to +0.018ns. Fixes: CDC constraints (`set_clock_groups -logically_exclusive` for MMCM, `set_false_path` for all cross-domain), local `vec_clk_cnt` register in `io_cell.v` (broke 5.5ns routing path to 160 pin cells).
- ✅ **Pinout verified** — 160/160 GPIO pin assignments match Sonoma reference `gpio_old_board.xdc` exactly. All banks, drive strengths (8mA/4mA), I/O standards (LVCMOS25/LVDS_25).

### Firmware Compiled & Deployed (March 25)

All fixes compiled, 0 warnings, 36 tests (25 host + 11 firmware). Deployed via JTAG, verified on hardware:

- ✅ **NTC thermistor formula rewrite** — `analog.rs`: Proper B-equation with `ln_approx()`, Sonoma-matched divider (4980Ω pulldown + 150Ω series), 30kΩ default with `set_ntc_type()` runtime switch. Matches Sonoma ReadAnalog.awk formula exactly.
- ✅ **Thermal controller wired** — `main.rs`: `Thermal` v2 instance created at boot, `update()` called in safety loop with THERM_CASE temp (MAX11131 ch 22, fallback to XADC die temp). Target updates on SET_OVERRIDE 0x80. Heater/fan duty cycles computed but no PWM output yet (GPIO pin identification pending).
- ✅ **Safety loop uses case temp** — `main.rs`: Reads THERM_CASE (actual NTC on BIM) instead of XADC die temp. Falls back to XADC if MAX11131 read fails.
- ✅ **`read_case_temp_mc()` / `read_dut_temp_mc()`** — `analog.rs`: Convenience methods for thermal controller and safety monitor. Returns milliCelsius from MAX11131 ch 22/23.
- ✅ **XADC u64 overflow fix** — `xadc.rs`: `as u32` → `as u64` in temp calculation. Needs hardware verification to confirm fix (die temp should read ~42°C now, not -220°C).
- ✅ **DNA guard** — `dna.rs`: FBC_CTRL version check before reading DNA peripheral. Prevents Data Abort on March 12 bitstream.
- ✅ **Compile-time thermal profiling** — `gen_fbc.c` + `compiler.rs`: Both compilers emit THERMAL_PROFILE section after OP_END. XOR + popcount during compression (zero extra cost). 25/25 tests pass.
- ✅ **Compiler pending_run bug fixed** — `compiler.rs`: `pending_run` wasn't reset after flushing.
- ✅ **Decompiler OP_END handling** — `compiler.rs` (to_vec) + `format.rs` (stats): Stop at OP_END instead of walking into thermal profile data.
- ✅ **CRC not written to disk** — `format.rs:write_to()` was zeroing CRC in output instead of calculating and writing it. Every .fbc file from the Rust compiler had CRC=0. Firmware would have rejected these. Fixed: `calculate_crc()` → `header.crc32 = crc` before writing.
- ✅ **emit_run off-by-one** — `compiler.rs`: When vec == prev, `emit_run` skipped emitting the vector but still used `count-1` for the RUN opcode. Fixed: uses `count` when vector not emitted, `count-1` when emitted.
- ✅ **Host/firmware warnings cleaned** — Host: unused FvecVector import, `_current_label`, `_interface`. Firmware: removed ClkCtrl/VecClockFreq imports from main.rs, `sd`/`analog_monitor` no longer mut. Both crates: 0 warnings.

---

## Host Interface

**Crate:** `host/` (`fbc-host` v2.0.0)
**CLI:** `host/src/bin/cli.rs`
**GUI:** `app/` (native wgpu, 7.4MB binary)

```rust
use fbc_host::FbcClient;

let mut client = FbcClient::new("Ethernet")?;
let boards = client.discover(Duration::from_secs(2))?;
let status = client.get_status(&boards[0].mac)?;
```

47 FbcClient pub methods (44 protocol + 3 utility). See `host/src/lib.rs` for full API.

### FbcClient Methods (47 pub fn)

**Utility (3):** `new`, `list_interfaces`, `recv_any`
**Discovery (3):** `discover`, `get_status`, `ping`
**Runtime (6):** `start`, `stop`, `reset`, `upload_vectors`, `configure`, `wait_done`
**Fast Pins (2):** `get_fast_pins`, `set_fast_pins`
**Analog (2):** `read_analog`, `get_min_max`
**VICOR (3):** `get_vicor_status`, `set_vicor_enable`, `set_vicor_voltage`
**PMBus (6):** `get_pmbus_status`, `set_pmbus_enable`, `pmbus_set_voltage`, `emergency_stop`, `power_sequence_on`, `power_sequence_off`
**EEPROM (3):** `read_eeprom`, `write_eeprom`, `write_bim`
**Vector Engine (5):** `get_vector_status`, `start_vectors`, `pause_vectors`, `resume_vectors`, `stop_vectors`
**Error Log (1):** `get_error_log`
**Flight Recorder (4):** `get_log_info`, `sd_format`, `sd_repair`, `read_log_sector`
**Firmware (2):** `get_firmware_info`, `firmware_update`
**Pattern Storage (3):** `upload_to_slot`, `get_slot_status`, `invalidate_slot`
**Test Plan (3):** `set_test_plan`, `run_test_plan`, `get_plan_status`
**Board Config (1):** `set_io_bank_voltage`

**Not yet in host crate:** board config overrides (0x31-0x34) except IO_BANK_SET

### GUI Panel → Host Method Mapping

This is the 1:1 reference for wiring GUI panels to the correct host crate calls.
FBC boards use `FbcClient` (raw Ethernet). Sonoma boards use `SonomaClient` (SSH).
The GUI dispatches based on `BoardId` — see `app/src/transport.rs`.

| GUI Panel | FBC Method | Sonoma Method | Wire Code(s) |
|-----------|-----------|---------------|---------------|
| **Board Discovery** | `discover()` | `expand_ip_range()` + `is_alive()` | 0x01 ANNOUNCE |
| **Board Status** | `get_status()` | `get_status()` | 0xF0/0xF1 |
| **Analog Monitor** | `read_analog()` | `read_xadc()` + `read_adc32()` | 0x70/0x71 |
| **Power — VICOR status** | `get_vicor_status()` | N/A (no batch read) | 0x80/0x81 |
| **Power — VICOR enable** | `set_vicor_enable(mask)` | `vicor_init(core, v)` | 0x82 |
| **Power — VICOR voltage** | `set_vicor_voltage(core, mv)` | `vicor_voltage(core, v)` | 0x83 |
| **Power — PMBus status** | `get_pmbus_status()` | N/A | 0x84/0x85 |
| **Power — PMBus enable** | `set_pmbus_enable(addr, en)` | `pmbus_set(ch, v)` / `pmbus_off(ch)` | 0x86 |
| **Power — IO Banks** | N/A (**GAP**) | `io_ps(b13, b33, b34, b35)` | — |
| **Power — Emergency Stop** | `emergency_stop()` | `emergency_stop()` | 0x8F |
| **Power — Sequence ON** | `power_sequence_on(voltages)` | `vicor_init()` × 6 (sequenced) | 0x90 |
| **Power — Sequence OFF** | `power_sequence_off()` | `vicor_disable()` × 6 (reverse) | 0x91 |
| **Fast Pins Read** | `get_fast_pins()` | N/A (Sonoma has no fast pins) | 0xD0/0xD1 |
| **Fast Pins Write** | `set_fast_pins(dout, oen)` | N/A | 0xD2 |
| **Vector Upload** | `upload_vectors(data)` | `load_vectors(seq, hex)` | 0x21 |
| **Vector Start** | `start_vectors(loops)` | `run_vectors(seq, time, dbg)` | 0xB4 |
| **Vector Pause/Resume** | `pause_vectors()` / `resume_vectors()` | N/A (Sonoma can't pause) | 0xB5/0xB6 |
| **Vector Stop** | `stop_vectors()` | N/A (kill process) | 0xB7 |
| **Vector Status** | `get_vector_status()` | N/A (parse stdout) | 0xB0/0xB1 |
| **Error Log** | `get_error_log(start, count)` | N/A (file-based) | 0x4A/0x4B |
| **EEPROM Read** | `read_eeprom(offset, len)` | N/A (Sonoma never reads EEPROM) | 0xA0/0xA1 |
| **EEPROM Write** | `write_eeprom(offset, data)` | N/A | 0xA2/0xA3 |
| **EEPROM Program BIM** | `write_bim(256B)` | N/A | 0x20 |
| **Board Config Override** | (needs host wrapper) | N/A | 0x31-0x34 |
| **Flight Recorder Info** | `get_log_info()` | N/A | 0x62/0x63 |
| **Flight Recorder Read** | `read_log_sector(sector)` | N/A | 0x60/0x61 |
| **SD Format** | `sd_format()` | N/A | 0x64/0x65 |
| **Firmware Info** | `get_firmware_info()` | `fw_version()` | 0xE1/0xE2 |
| **Firmware Update** | `firmware_update(data, crc)` | `update_firmware(path)` | 0xE3-0xE9 |
| **Clock Config** | `configure(div, voltages)` | `set_frequency(pll, hz, duty)` | 0x30 |
| **Pin Config** | (via CONFIGURE) | `set_pin_type(pin, type)` | 0x30 |
| **Temperature** | (via BoardConfig setpoint) | `set_temperature(sp, r25c, cool)` | 0x31 (field 0x80) |
| **DDR Slot Upload** | `upload_to_slot(mac, slot, data)` | N/A | 0x22 |
| **DDR Slot Status** | `get_slot_status()` | N/A | 0x23/0x24 |
| **DDR Slot Invalidate** | `invalidate_slot(mac, slot)` | N/A | 0x25 |
| **Test Plan Set** | `set_test_plan(mac, plan)` | N/A | 0x26/0x27 |
| **Test Plan Run** | `run_test_plan(mac)` | N/A | 0x28/0x29 |
| **Test Plan Status** | `get_plan_status(mac)` | N/A | 0x2A/0x2B |
| **Test Run (orchestrated)** | `start()` + `wait_done()` | `run_test(config)` | multiple |
| **Fleet Run** | (CLI: `Run` with multiple MACs) | `run_fleet(ips, config, ...)` | multiple |
| **PMBus Set Voltage** | `pmbus_set_voltage(ch, mv)` | `pmbus_set(ch, v)` | 0x87 |
| **Write BIM** | `write_bim(256B)` | N/A | 0x20 |
| **Switch Port Map** | `SwitchPollPorts` → serial `show interfaces status` + `show mac address-table` | Same switch (shared infrastructure) | Serial COM |
| **Switch VLAN Config** | `SwitchSetVlan(port, vlan)` → serial `configure terminal` | Same switch | Serial COM |
| **Switch Port Control** | `SwitchShutdown(port, bool)` → serial `shutdown`/`no shutdown` | Same switch | Serial COM |

**Key differences the GUI must handle:**
1. **FBC is stateless per-command** — each method is one Ethernet frame round-trip
2. **Sonoma is session-based** — SSH connection persists, `flock` prevents concurrent ELF execution
3. **FBC has no IO bank voltage command** — GAP, needs firmware handler added
4. **Sonoma has no pause/resume** — vectors run until completion or process kill
5. **FBC EEPROM is per-board identity** — Sonoma boards have no EEPROM interaction
6. **Analog formulas differ** — FBC firmware applies formulas + cal offsets, Sonoma host parses ELF CSV output
7. **Switch is shared infrastructure** — same Cisco switch connects both FBC and Sonoma boards. MAC→port cross-reference works for both profiles

---

## How FBC Replaces Sonoma's File Ecosystem

**Verified March 25, 2026** against live production bench (Bench 7 via RustDesk).

Sonoma requires 7+ separate text files per device, all served over NFS from the host PC.
FBC eliminates this entirely — everything the controller needs is either in EEPROM (256 bytes
on the BIM) or in the `.fbc` binary (vectors + pin config + thermal profile).

### File-by-File Replacement

| Sonoma File | What It Contains | FBC Replacement | Where It Lives |
|-------------|-----------------|-----------------|----------------|
| `PIN_MAP` | GPIO → signal name (128 lines) | **Not needed at firmware level.** Pin assignments are physical (XDC constraints). GUI/host knows signal names from device JSON. | Host-side only |
| `{device}.map` | GPIO + VOUT/IOUT + ADC/XADC signal mapping | **EEPROM** (rail config at 0x10-0x4F) + **BoardConfig** (channel→formula mapping). Production .map has 3 sections — GPIO, power supplies (VOUT1-12/IOUT1-12), ADC monitoring (ADC_0-15, XADC_0-2). FBC puts power/ADC config in EEPROM, GPIO mapping is implicit from pin_config in `.fbc`. | EEPROM (256B on BIM) |
| `{device}.tim` | **Shell script** calling ELFs (`linux_xpll_frequency.elf`, `linux_pin_type.elf`, `linux_Pulse_Delays.elf`) | **Protocol commands:** `CONFIGURE` (0x30) sets clock freq + pin types + pulse timing in one Ethernet frame. Pin config is also embedded in `.fbc` header (80-byte pin_config section, 4 bits per pin). | `.fbc` pin_config + protocol |
| `{device}.lvl` | Bank voltage levels (rare — only 13 across all production devices) | **Protocol command:** IO bank voltages via `CONFIGURE` or future `IO_PS` command. Not needed per-file — sent as parameters. | Protocol command |
| `{device}.tp` | Test plan steps (text file parsed by AWK) | **Not needed.** FBC test sequencing is handled by the host orchestrator (`run_test()` in host crate). Test parameters are protocol commands, not parsed text files. | Host-side logic |
| `PowerOn.sh` / `PowerNOM` | Shell script calling `ToggleMio.elf` + `linux_VICOR_Voltage.elf` in sequence | **Protocol command:** `POWER_SEQ_ON` (0x90) with voltage array. Firmware sequences enables + DAC writes atomically. Production uses `PowerNOM` (no `.sh` extension). | Protocol command |
| `PowerOff` | Reverse-order shutdown script | **Protocol command:** `POWER_SEQ_OFF` (0x91) or `EMERGENCY_STOP` (0x8F). | Protocol command |
| `{device}.bim` | XML BIM definition (pin types, DUT layout, power mapping) — 1-12 KB | **EEPROM** (256B): header (bim_type, serial, hw_rev), rail config (8 rails × 8B), calibration (32 channels), DUT metadata. XML schema has `<Pin type="Signal|PmbPS|CorePS|ADC">` — FBC maps these to EEPROM fields. | EEPROM (256B on BIM) |
| `.hex` vectors | 40 bytes/vector, uncompressed, streamed via NFS | **`.fbc` binary:** 1-21 bytes/vector (4.8-710x smaller), includes pin_config (80B) + thermal profile. Self-contained — no separate pin map or timing file needed. | `.fbc` file in DDR |
| `.seq` file | Sequence name string (18-87 bytes) | **Not needed.** Vector metadata is in the `.fbc` header (num_vectors, vec_clock_hz, pin_count). | `.fbc` header |

### The Consolidation

```
SONOMA (7+ files over NFS):                    FBC (2 sources, no filesystem):
┌──────────────────────┐                       ┌──────────────────────────┐
│ PIN_MAP              │                       │ EEPROM (256B on BIM)     │
│ {device}.map         │──── board identity ───▶│  header: type, serial    │
│ {device}.bim (XML)   │     + config          │  rails: 8 × voltage/I    │
│ PowerOn.sh           │                       │  cal: 32 ch offsets      │
│ PowerOff             │                       │  DUT: vendor/part        │
├──────────────────────┤                       └──────────────────────────┘
│ {device}.tim (script)│                       ┌──────────────────────────┐
│ {device}.lvl         │──── pin config ──────▶│ .fbc binary              │
│ .hex vectors         │     + vectors         │  pin_config: 80B (4b/pin)│
│ .seq sequence        │     + thermal         │  vectors: compressed     │
│ {device}.tp          │                       │  thermal_profile: N×8B   │
└──────────────────────┘                       └──────────────────────────┘
                                               ┌──────────────────────────┐
          (AWK scripts parse these             │ Protocol commands (0x88B5)│
           text files at runtime) ────────────▶│  CONFIGURE: clock+pins   │
                                               │  POWER_SEQ: voltages     │
                                               │  SET_OVERRIDE: runtime   │
                                               └──────────────────────────┘
```

### Why This Matters

1. **No NFS dependency.** Sonoma boards mount `C:\Everest` over NFS. If WinNFSd crashes or
   the network flakes, boards can't access their own config files. FBC boards carry everything
   in EEPROM — they know who they are even with zero network.

2. **No text parsing on embedded.** Sonoma's AWK scripts parse `.map`, `.tim`, `.tp` text files
   on a 667MHz ARM running Linux. FBC firmware reads structured binary (EEPROM) and receives
   typed protocol commands. No string parsing, no regex, no field delimiter ambiguity.

3. **Atomic configuration.** Sonoma's `.tim` is a shell script that calls 20+ ELFs sequentially —
   if any fails, you have a partially configured board. FBC's `CONFIGURE` command sets everything
   in one transaction. Power sequencing is atomic with voltage verification before enabling.

4. **Self-identifying boards.** Sonoma boards are interchangeable — identity comes from which
   NFS directory the host tells them to use. Swap two boards and nobody knows. FBC boards have
   EEPROM with serial number, BIM type, calibration data. Swap a board and the system detects it.

5. **Version coupling eliminated.** Sonoma's .tim files reference specific ELF binaries
   (`/mnt/bin/linux_xpll_frequency.elf`). Change the firmware → break the .tim files.
   FBC uses versioned protocol commands — the wire format is the API contract.

### Production Format Differences (What Our dc_gen.c Generates vs Reality)

Our `dc_gen.c` generates 7 file types for **Sonoma compatibility** (so existing Sonoma boards
can consume them). These are intentionally different from what FBC firmware uses:

| Our Generator | Format | Production Reality | Impact |
|---------------|--------|-------------------|--------|
| `dc_gen_map()` | `SIGNAL = BANK_GPIO# ; DIR` | Wider: includes VOUT/IOUT, ADC, XADC sections | **Need to add** power/ADC sections for full Sonoma compat |
| `dc_gen_tim()` | `PERIOD=X DRIVE_OFF=X COMPARE=X` | Shell scripts calling ELFs | **Different format** — production .tim are scripts, not key=value. Our format is a simplified abstraction. |
| `dc_gen_lvl()` | `BANK <name> VOLTAGE=X.XXX` | Rarely used (13 across all production devices) | Low priority — most devices don't have .lvl |
| `dc_gen_tp()` | `STEP PATTERN PATTERN_FILE LOOPS` | Only 118 across all devices; many use .sh scripts instead | Our format works but .tp usage is less universal than assumed |
| `dc_gen_power_on()` | `PowerOn.sh` | Named `PowerNOM` (no .sh extension) | Naming convention differs |
| `dc_gen_power_off()` | `PowerOff.sh` | Named `PowerOff` (no .sh extension) | Naming convention differs |
| `dc_gen_pinmap()` | `GPIO_IDX SIGNAL_NAME\n` | Same format ✅ | Matches production exactly |

**Bottom line:** These generators exist for Sonoma backward compatibility. FBC firmware never
reads any of these files — it uses EEPROM + `.fbc` + protocol commands.

---

## File Locations

| What | Where | Notes |
|------|-------|-------|
| Firmware source | `firmware/src/` | 32 source files, bare-metal Rust (includes ddr_slots.rs, testplan.rs, flight_recorder.rs, board_config.rs) |
| RTL source | `rtl/` | 16 Verilog modules |
| Constraints | `constraints/zynq7020_sonoma.xdc` | 160-pin mapping |
| Host crate | `host/` | FbcClient (lib.rs) + SonomaClient (sonoma.rs) |
| CLI | `host/src/bin/cli.rs` | 38 FBC + 20 Sonoma commands |
| Native GUI (PRODUCT) | `app/` | wgpu/DX12, 13 panels, unified dual-profile transport (FBC + Sonoma), all panels functional — **this is the target** |
| Tauri GUI (REFERENCE) | `gui/` | Tauri + React — reference impl to port FROM (pattern conv, device config, EEPROM, test plan) |
| C pattern converter | `gui/src-tauri/c-engine/pc/` | 12 C sources → libpattern_converter.a |
| FSBL | `fsbl/` | First Stage Boot Loader |

---


---

## GUI — Production Burn-In Control System

### What This Is

A **production-grade burn-in control application** that replaces three legacy Sonoma apps
(Unity, Everest, Bench App) with a single native .exe. This is the control plane for
semiconductor burn-in test systems that run 24/7 in production facilities — and the engineering
workbench for device bringup, debugging, and custom test sequences.

| Legacy App | What It Did | Our Replacement |
|------------|-------------|-----------------|
| **Unity** (Editor) | .tpf XML editor, .bim editor, PIN_MAP, device files, PowerOn/Off scripts | **Device Profiling** tab + `dc_gen.c` (all 7 file types) |
| **Everest** (Server) | NFS server, TCP :3000, .tpf→.tp, RunVectors, ReadAnalog, orchestration, CSV datalog | **Dashboard** tab (fleet + LOT loading) + hardware thread (tokio async) |
| **Bench App** (Manual) | SSH terminal, power control, pin debug, vector load/run, ADC, EEPROM | **Engineering** tab (debug tools + terminal + safety) |

**Deployment target:** Production system PCs — AMD Ryzen 7 5700G, Radeon Vega 8, 32GB RAM,
Win11 Pro, 1920×1080. Single .exe, ~10MB, no runtime deps. wgpu DX12/DX11. 60fps = ~2% GPU.

**Why not Everest:** Tcl/Tk runs 44 SSH streams in a single-threaded event loop → crashes.
Our app: Rust async (tokio) on a dedicated hardware thread, built-in SSH, no NFS dependency.

### Multi-System Support

Profile-switched to control **5 tester system types** from one window:

| System | Transport | Status | What's Needed |
|--------|-----------|--------|---------------|
| **FBC** | Raw Ethernet 0x88B5 | ✅ Full (37 commands) | Production-ready |
| **Sonoma** | SSH + ELF execution | ✅ Full (34 commands) | Production-ready |
| **HX** | INSPIRE protocol | ❌ Not implemented | Reverse-engineer INSPIRE or get Aehr SDK |
| **XP-160/Shasta** | INSPIRE protocol | ❌ Not implemented | Same driver as HX, different axis count |
| **MCC** | Modbus TCP | ❌ Not implemented | Modbus register map for Watlow + MCC |

All 5 share the same GUI tabs, orchestration, and database integration.
The board announces its identity at discovery — the GUI doesn't choose.

---

### GUI Architecture: 4 Tabs

The app is organized into **4 workflow-oriented tabs**, not a flat panel list.
Each tab serves a different user role and workflow stage.

```
┌─────────────────────────────────────────────────────────────────────┐
│  [1. Dashboard]  [2. Device Profiling]  [3. Engineering]  [4. Datalogs]  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Tab content (hierarchical navigation within each tab)              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

### Tab 1: Dashboard (Production Monitoring + LOT Loading)

**Who uses it:** Operators, production managers, anyone monitoring the system.

**What it is:** The main view. Shows the entire system hierarchy with real-time status,
and provides the primary interface for loading test runs onto boards.

#### Physical Hierarchy Navigation

```
System (1 PC controls 1 rack)
  └── Shelf (11 per system)
       └── Tray (2 per shelf: front + rear)
            └── BIM (Board Interface Module — the physical board assembly)
                 └── Controller Board (Zynq + firmware)
                      └── Socket (DUT site — holds one chip under test)
```

**Drill-down:** System view → click shelf → see 2 trays → click tray → see BIM(s) → click board → see sockets + status.
**Drill-up:** Should be fluent — breadcrumb or back navigation, never feel stuck.

#### Tray / BIM Rules

Each tray holds BIMs. BIM form factor determines how many fit:

| BIM Size | Boards per BIM | BIMs per Tray | Boards per Tray |
|----------|---------------|---------------|-----------------|
| **Full** | 4 | 1 | 4 |
| **Half** | 2 | 2 | 4 |
| **Quarter** | 1 | 4 | 4 |

**Constraint:** Cannot mix half + full BIM on one tray (physically doesn't fit).
Half + half = OK. Quarter + quarter + quarter + quarter = OK. Full alone = OK.

A tray can be **removed for servicing** — marked in the app, not physically tracked.

#### Board Status (visible at every level)

Each board shows:
- **Running** — actively executing test vectors
- **Manual Stopped** — operator paused/stopped
- **Completed** — test plan finished, all steps done
- **Lost Connection** — board stopped responding (SSH timeout / Ethernet timeout)
- **Shutdown** — powered down, with **reason** (normal completion, error, thermal, operator, power loss)
- **Duration** — how long the current/last run has been going

Color-coded at the system/shelf level so operators can see status at a glance from across the room.

#### LOT-Based Loading

A **LOT** is the unit of work. It ties together:

| LOT Field | What |
|-----------|------|
| **LOT #** | Unique identifier (manually entered now, auto from LRM v2 later) |
| **Customer** | Who the chips belong to (Cisco, Microsoft, etc.) |
| **Device** | Chip part number (e.g., C512, Normandy) |
| **Project** | Internal project code |
| **Unit Count** | How many DUTs in this lot |
| **Run Type** | HTOL, EFR, burn-in, qualification, etc. |
| **Duration** | How long to run (hours) |
| **Test Spec** | Which test plan / device profile to use |

**Loading flow:**

1. Select boards (all system, all shelf, specific boards via drill-down)
2. Enter or select LOT (LOT# → customer/device/project + units + duration + spec)
3. **EEPROM check** — BIM# must be on EEPROM. If missing: notifies user, can't load production run
   (engineer-only test mode bypasses this — but not for production LOTs)
4. **Board identity check** — EEPROM tells how many sockets per BIM. If BIM#5 has a dead socket,
   user marks it empty. GUI knows exactly how many units this board can test.
5. **Serial number entry** — User inputs DUT serial numbers into socket slots. Saved GUI-side
   (FPGA doesn't need serials — GUI tracks this for traceability).
6. **Persistent boardmap** — GUI saves the mapping: LOT X → boards Y, Z, W with serials.
   Next time this LOT comes back (e.g., HTOL additional hours), GUI **highlights where to load**
   so the same DUTs go back on the same boards/sockets.
7. **Run starts** — firmware gets test config, vectors loaded, power sequenced, test begins.

**Easy selection:**
- Click shelf → "Load this shelf" (all 8 boards)
- Click tray → "Load this tray" (4 boards)
- Checkboxes on individual boards
- "Select All" for whole system (88 boards)
- Remember LOT#s — preload for returning LOTs

#### LOT → LRM v2 Integration (Future)

LOTs will eventually auto-populate from LRM v2 database:
- `GET /api/lots?status=pending` → shows LOTs waiting to be loaded
- Operator selects LOT → boards auto-assigned based on availability + BIM compatibility
- Results auto-posted: `POST /api/test_results` per board per test step
- Full traceability: which DUT serial, which board, which socket, what temp, how long, pass/fail

---

### Tab 2: Device Profiling (Setup + File Generation)

**Who uses it:** Engineers setting up new customer devices.

**What it is:** The full device setup pipeline — from customer datasheet to production-ready
device package. Everything needed before a LOT can run.

#### What It Produces

Every customer device needs this file set (same across all system profiles):

| File | Generator | Purpose |
|------|-----------|---------|
| `PIN_MAP` | `dc_gen_pinmap()` | 128/160 pin→signal mapping |
| `{device}.map` | `dc_gen_map()` | Signal→bank assignment |
| `{device}.lvl` | `dc_gen_lvl()` | Bank voltages + CMOS levels |
| `{device}.tim` | `dc_gen_tim()` | Timing parameters |
| `{device}.tp` | `dc_gen_tp()` | Test plan steps |
| `PowerOn.sh` | `dc_gen_power_on()` | Sorted power-up sequence |
| `PowerOff.sh` | `dc_gen_power_off()` | Reverse order shutdown |
| Vectors (.fbc/.hex) | `gen_fbc.c` / `gen_hex.c` | Test patterns |

#### Pipeline

```
Source (datasheet PDF, CSV, schematic, existing Sonoma files)
    ↓
Import: extract_pin_table() → editable pin table
    ↓
Cross-verify: compare against secondary source (optional)
    ↓
Configure: set banks, voltages, timing, power supplies, thermal
    ↓
Generate: dc_gen_all() → 7 device files + vectors
    ↓
Save: JSON (in-progress) or final format (complete)
```

**Key features:**
- **Import from anything:** CSV, Excel, PDF (tabular or schematic), existing Sonoma device directories
- **Legacy Sonoma support:** Can open Everest directory structure (`/home/device/`) and convert to FBC format
- **Live editing:** Changes save continuously as JSON. When done, export as final device package.
- **Profile-aware:** Output format adapts to target system (Sonoma=128ch, HX=160ch/axis, etc.)
- **Vector conversion:** ATP/STIL/AVC → .hex (Sonoma) or .fbc (FBC) via C engine

#### Sub-sections

1. **Pin Map Editor** — GPIO→signal table, auto-detect from imports, manual override
2. **Signal Map** — Signal→bank assignment, derive from pin map
3. **Voltage/Levels** — Bank voltages, CMOS 70/30 derivation (VIH, VIL, VOH, VOL)
4. **Timing** — Period, rise, fall, compare per pin type
5. **Power Supplies** — VICOR cores, PMBus rails, IO banks, sequence order, delays
6. **Thermal** — Setpoint, NTC type, cool-after, ADC monitoring thresholds
7. **Test Plan** — Step sequence: temp→vectors→duration→power for each step
8. **Vector Conversion** — ATP/STIL/AVC input → .fbc/.hex output, with pin map applied

---

### Tab 3: Engineering (Debug + Bringup + Manual Control)

**Who uses it:** Engineers doing device bringup, failure analysis, custom testing.

**What it is:** Full manual control of the hardware with safety guardrails.
We built the system — we know what can go wrong and what the safe operating envelope is.

#### Safety Guardrails

The engineering tab lets you do anything, but **prevents damage:**
- Power supplies auto-set to safe defaults when enabled (user can adjust upward)
- Temperature auto-cooling if overheat detected (even if user forgets)
- Auto-shutoff on thermal runaway
- Voltage limits enforced per supply type (can't set VICOR to 5V when max is 1.2V)
- Current monitoring — alert if draw exceeds expected range

#### Sub-sections

1. **Power Control**
   - Enable/disable individual VICOR cores, PMBus rails, IO banks
   - Set voltages with sliders + numeric input
   - Real-time current monitoring
   - Emergency stop (always accessible, every view)
   - Example: "Turn on VOUT 8" → auto-sets safe voltage, user adjusts

2. **Thermal Control**
   - Set temperature target per board
   - Enable/disable heater and cooler independently
   - Real-time NTC temperature display (case + DUT)
   - Example: "Turn on C2 heater, set 85°C" → if overheat, auto cool + shut off heat

3. **Analog Monitor**
   - 32-channel ADC (XADC + MAX11131)
   - Real-time voltage bars, color-coded against expected ranges
   - Useful during probing: see voltage changes as you touch pins

4. **Vector Operations**
   - Upload and run specific vector files
   - Visualize vectors: per-pin waveform view (logic analyzer style)
   - Run specific vector ranges for targeted testing
   - Continuity checks: user-defined vector sequences that verify connections
   - Step-through mode for failure analysis

5. **EEPROM**
   - Read/write 256-byte BIM EEPROM
   - Hex viewer + ASCII sidebar
   - Program BIM identity (BIM#, socket count, calibration data)

6. **Fast Pins**
   - Direct GPIO control (pins 128-159)
   - Write patterns, read back, verify continuity

7. **BIM Visualization**
   - Visual representation of the BIM board
   - Show which sockets are populated, which are empty/damaged
   - Pin-level status overlay

8. **Terminal**
   - PowerShell-style command interface
   - Lists available commands, autocomplete
   - Profile-aware: FBC commands for FBC boards, Sonoma commands for Sonoma
   - History, scrollback
   - Example: `vicor enable 2 --voltage 0.9` or `analog read --channel 16`

9. **Automated Diagnostics**
   - We built the system — we can encode failure mode knowledge
   - Error pattern recognition: "Error on pin 47 during vector 1024? That's usually a BIM contact issue"
   - Setup validation: "You have VICOR 3 at 0.8V but your device spec says 0.9V — mismatch"
   - Custom command sequences: save a debug routine as a script, replay later

---

### Tab 4: Datalogs (Binary Interpretation + Export)

**Who uses it:** Engineers, QA, production managers, customer-facing reports.

**What it is:** Where raw binary test data becomes actionable information.
Controllers send binary datalogs during/after runs. This tab interprets, organizes,
visualizes, and exports them.

#### Data Flow

```
Board (binary datalog over Ethernet/SSH)
    ↓
GUI receives raw binary
    ↓
Interpret: decode fields, timestamps, measurements, error codes
    ↓
Tag: associate with customer/device/project/LOT#
    ↓
Store: local DB or LRM v2
    ↓
Visualize + Export
```

**Efficiency:** Don't send huge human-readable text over Ethernet — binary is compact.
Interpretation happens client-side (PC has the CPU cycles, board doesn't).

#### Organization

Datalogs organized by:
- **Project** (customer/device)
- **LOT #** (if written — mandatory for production, optional for engineering)
- **Date** (timestamp of run start/end)
- **Shutdown reason** (normal, error, thermal, operator, power loss)
- **Board** (which physical board)

Sort and filter by any combination: "Show me all LOTs for Cisco C512 that shut down due to errors in the last week."

#### Visualization

- **Graphs:** Temperature over time, voltage trends, error accumulation curves
- **Tables:** Per-vector error counts, per-channel ADC readings at each test step
- **Correlations:** ADC reading vs temperature, error rate vs voltage, duration vs failures
- **Measurements during vectors:** What the ADC read during specific vector execution windows

#### Export

- **Customer-facing reports:** Professional format — not just Excel dumps.
  Formatted PDF/HTML with graphs, summary tables, pass/fail verdict, test conditions.
- **Raw data:** CSV/JSON for customers who want to analyze themselves
- **LRM v2 push:** Auto-upload results to database (when integrated)
- **Efficient:** Binary → interpreted → formatted. No bottleneck from sending verbose text.

---

### How Current Code Maps to 4 Tabs

The existing 13 panel source files reorganize into the 4-tab structure:

| Tab | Current Panels | New Role |
|-----|---------------|----------|
| **Dashboard** | overview.rs + facility.rs (Board Grid, Slot Map, Thermal, Network tabs) + board.rs | Hierarchical System→Shelf→Tray→Board nav, LOT loading, fleet status, switch port map |
| **Device Profiling** | pattern.rs + device.rs + testplan.rs | Unified setup pipeline, import→configure→generate |
| **Engineering** | power.rs + analog.rs + vectors.rs + waveform.rs + eeprom.rs + terminal.rs + firmware.rs | Debug/bringup tools with safety, manual control |
| **Datalogs** | *(new)* | Binary interpretation, visualization, export |

The current flat panel navigation (sidebar with 13 items) becomes a **top tab bar** with
sub-navigation within each tab. The existing panel code is the building blocks — the
refactor is primarily about navigation structure, not rewriting widget logic.

---

### Core Principle: Profiles Are Independent

The firmware IS the board's identity. FBC boards announce via raw Ethernet (`magic=0xFBC0`).
Sonoma boards respond to SSH. **You don't choose the profile — you discover it.**

Each profile has its own wire protocol, command set, firmware binary, vector format, config model.
Profile-specific operations (Firmware, Vectors) are physically separated in code — the FBC section
can't reach Sonoma boards and vice versa. Orchestration targets resolve once, filter by profile.

### Architecture

~6500 lines of Rust across 25 source files. Two threads:

```
UI Thread (winit + wgpu)  ---cmd_tx (mpsc)--->  Hardware Thread (tokio)
       60fps redraw        <---rsp_tx (mpsc)---   FbcClient (0x88B5)
                                                  SonomaClient (SSH)
                                                  [future: InspireClient, ModbusClient]
```

- **UI thread:** winit event loop → input → draw → wgpu render. Polls responses every frame. Never blocks.
- **Hardware thread:** `tokio::select!` — commands from UI OR 3-second auto-poll for all boards.
- **No protocol abstraction.** FbcClient builds raw Ethernet. SonomaClient runs SSH. Direct.

### Orchestration

| Target | Resolves To | Use Case |
|--------|-------------|----------|
| `Selected` | Single clicked board | Debug one board |
| `Set(indices)` | Checkbox subset | "Flash boards 1-20 but skip 21-44" |
| `AllFbc` | All FBC boards | FBC-wide firmware update |
| `AllSonoma` | All Sonoma boards | Sonoma-wide deployment |
| `All` | Everything | E-stop only |

### Database Integration (LRM v2)

LRM v2 at `C:\Dev\projects\Lab-Resource-Manager-v2-Isaac-\` — C server, port 8080, 104 REST routes, 21 tables.

| GUI Action | LRM v2 Route | What |
|-----------|-------------|------|
| Board discovery | `POST /api/boards` | Register with serial, MAC, system_type |
| LOT assignment | `POST /api/lots/:id/assign` | Boards assigned to LOT |
| Test completion | `POST /api/test_results` | Pass/fail, errors, duration per board |
| ADC readings | `POST /api/telemetry` | ADC snapshots during burn-in |
| Firmware update | `POST /api/firmware_logs` | Version changes per board |
| LOT lookup | `GET /api/lots?status=pending` | Auto-populate pending LOTs |
| Datalog export | `POST /api/datalogs` | Binary data → database for reporting |

### Infrastructure

| File | Lines | Role |
|------|-------|------|
| `main.rs` | 266 | Window, event loop, thread spawn, frame dispatch |
| `state.rs` | 760 | BoardState, SwitchState, CommandTarget, orchestration, telemetry |
| `transport.rs` | 850 | 30 FBC + 20 Sonoma + 7 Switch HwCommand variants, 19 HwResponse types, auto-poll, serial switch backend |
| `ui.rs` | 494 | Immediate-mode widget API (17 widget types) |
| `draw.rs` | 265 | Vertex batching, rounded rects, text quads |
| `gpu.rs` | 415 | wgpu setup: surface, pipeline, buffers, render |
| `text.rs` | 209 | fontdue rasterizer, glyph atlas |
| `layout.rs` | 235 | Rect, Column, Row — layout math |
| `input.rs` | 132 | Mouse + keyboard |
| `theme.rs` | 87 | Colors, fonts, spacing |
| `pattern_converter.rs` | 278 | C engine FFI (PcHandle + DcHandle) |
| `build.rs` | 22 | cc crate compiles 15 C sources |

### Transport Commands

**FBC (30):** ListInterfaces, Discover, GetStatus, Ping, Start, Stop, Reset, EmergencyStop, GetVicorStatus, SetVicorEnable, SetVicorVoltage, GetPmbusStatus, SetPmbusEnable, PowerSequenceOn, PowerSequenceOff, ReadAnalog, GetVectorStatus, UploadVectors, StartVectors, PauseVectors, ResumeVectors, StopVectors, ReadEeprom, WriteEeprom, GetFastPins, SetFastPins, GetErrorLog, GetFirmwareInfo, FirmwareUpdate, GetLogInfo

**Sonoma (20):** ScanSonoma, SonomaGetStatus, SonomaVicorInit, SonomaVicorVoltage, SonomaVicorDisable, SonomaPmbusSet, SonomaPmbusOff, SonomaIoPs, SonomaEmergencyStop, SonomaReadXadc, SonomaReadAdc32, SonomaLoadVectors, SonomaRunVectors, SonomaSetPinType, SonomaSetFrequency, SonomaSetTemperature, SonomaInit, SonomaToggleMio, SonomaUpdateFirmware, SonomaExec

**Switch (7):** SwitchConnect, SwitchDisconnect, SwitchPollPorts, SwitchSendCommand, SwitchSetVlan, SwitchSetDescription, SwitchShutdown

**Auto-poll:** 3s concurrent — GetStatus (FBC) + SonomaGetStatus (Sonoma).

### Cisco Switch Integration (Network Tab)

Every Sonoma/FBC system has a **managed Cisco switch** connecting all boards to the host PC.
Production systems: 88-port switch (44 front + 44 rear boards). Dev bench: Cisco 3560 (4-8 ports, L3 capable).

**Console:** Serial (COM4 default, 9600 8N1). Full Cisco IOS CLI: `user → enable → configure terminal`.

**GUI integration (Facility → Network tab):**

| Feature | Implementation | Files |
|---------|---------------|-------|
| Connect/Disconnect | `serialport` crate, COM port text input | `transport.rs` (serial open/close) |
| Port Map | `show interfaces status` + `show mac address-table` → parsed into `SwitchPort` structs | `transport.rs` (parse_switch_ports) |
| MAC→Board cross-ref | Switch MAC compared against discovered board MACs → `SwitchPort.board_id` | `state.rs` (crossref_switch_boards) |
| VLAN config | `configure terminal` → `interface X` → `switchport access vlan Y` | `transport.rs` (SwitchSetVlan handler) |
| Port shutdown | `shutdown` / `no shutdown` via config mode | `transport.rs` (SwitchShutdown handler) |
| Port description | `description` command via config mode | `transport.rs` (SwitchSetDescription) |
| Raw CLI | Any IOS command, output returned to GUI | `transport.rs` (SwitchSendCommand) |

**Network topology:**
- **172.16.0.49** — Mini PC / Everest server (NFS + TCP :3000)
- **172.16.0.101-144** — Front boards (shelves 1-11, tray A, boards 1-4)
- **172.16.0.201-244** — Rear boards (shelves 1-11, tray B, boards 1-4)
- **Subnet:** 172.16.0.0/16 (all boards + server on flat network)

**State structs (`state.rs`):**
```rust
struct SwitchPort {
    port: String,              // "Gi0/1", "Fa0/12"
    description: String,       // port description
    status: String,            // "connected", "notconnect", "disabled"
    vlan: String,              // VLAN number or "trunk"
    speed: String,             // "100", "1000", "auto"
    duplex: String,            // "full", "half", "auto"
    mac_address: String,       // learned MAC from mac address-table
    board_id: Option<BoardId>, // cross-referenced board (if MAC matches)
}

struct SwitchState {
    connected: bool,
    com_port: String,          // "COM4"
    hostname: String,          // switch hostname
    ports: Vec<SwitchPort>,
    last_error: Option<String>,
}
```

**Serial protocol handling:**
- `switch_read_until_prompt()` — reads until `#` or `>` prompt, extracts hostname
- `switch_send_command()` — sends CLI command, auto-handles `--More--` pagination
- `parse_switch_ports()` — Cisco IOS fixed-width output → structured `SwitchPort` entries
- Runs on the hardware thread (blocking serial I/O in tokio context via spawn_blocking)

**Why this matters for production:**
1. **MAC→port→slot mapping:** `show mac address-table` + board discovery = physical position of every board without IP guessing
2. **Link state monitoring:** `show interfaces status` = which boards are physically plugged in
3. **VLAN isolation:** Separate FBC raw Ethernet (0x88B5) from Sonoma SSH traffic
4. **Port diagnostics:** Speed/duplex negotiation, error counters, CRC errors per port
5. **Auto-discovery validation:** Cross-reference switch port map against board discovery to detect wiring issues

**Existing tool (reference):** `C:\Users\isaac\Python Projects\NetworkShareTool\network_share_tool.py` — Tkinter GUI with serial terminal, quick-command buttons, mode switching, VLAN management. Our implementation replicates the essential operations in native Rust.

### Remaining Work (Priority)

| # | Area | Current | Target |
|---|------|---------|--------|
| 1 | **Dashboard hierarchy** | Flat board list | System→Shelf→Tray→Board drill-down |
| 2 | **LOT loading system** | Not started | LOT# → board assignment → serial entry → persistent boardmap |
| 3 | **EEPROM-gated loading** | Not started | Can't load production run without BIM# on EEPROM |
| 4 | **Device Profiling pipeline** | Mockup panels | Full import→configure→generate flow |
| 5 | **Engineering safety** | Basic power panel | Auto-safe defaults, overheat protection, diagnostics |
| 6 | **Datalogs tab** | Not started | Binary decode → visualize → export |
| 7 | **LRM v2 integration** | Not started | HTTP client → POST results/LOTs/telemetry |
| 8 | **Waveform data** | Fake patterns | Parse .fbc/.hex → real per-pin transitions |
| 9 | **HX/XP-160/MCC** | Enum stubs | InspireClient, ModbusClient |
| 10 | **Customer export** | Not started | Professional formatted reports (PDF/HTML) |
