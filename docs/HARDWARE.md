# FBC Hardware Status & Reference

**Last Verified:** March 2026 (First Light)
**Status:** ✅ PL programmed, ✅ PS firmware running

---

## Current Hardware Status

### PL (FPGA Fabric)

| Component | Status | Notes |
|-----------|--------|-------|
| **Bitstream** | ✅ PROGRAMMED | Loaded via JTAG (FT232H/FTDI) |
| **AXI Peripherals** | ✅ WIRED | All 6 peripherals instantiated in `system_top.v` |
| **DMA** | ✅ WIRED | `fbc_dma.v` at 0x4040_0000, HP0 master |
| **Error BRAMs** | ✅ WIRED | 3× BRAMs at 0x4009_0000 |
| **Fast Error** | ✅ WIRED | `fast_error[31:0]` → `axi_fbc_ctrl` at 0x2C |
| **Clock Gen** | ✅ WIRED | MMCM configured, 5 frequencies available |

**Verification:** Read register 0x4004_001C → should return `0x00010000` (VERSION)

### PS (ARM Cortex-A9)

| Component | Status | Notes |
|-----------|--------|-------|
| **Firmware** | ✅ **RUNNING** | First Light achieved March 2026 — CPU @ 667MHz, DDR @ 533MHz |
| **Ethernet** | ✅ WORKING | GEM0 initialized, ANNOUNCE packet sent |
| **VICOR GPIO** | ✅ FIXED | SLCR `configure_mio()` called in `main.rs:61-78` |
| **HAL Drivers** | ✅ READY | 17 drivers in `firmware/src/hal/` |
| **FBC Protocol** | ✅ READY | 28 commands in `fbc_protocol.rs` |
| **Interrupt Handler** | ✅ READY | `fbc_irq_handler()` in `main.rs` |

**Next Step:** Test AXI register access, run simple vector, verify GPIO toggling

---

## Power Requirements

### 12V Main Input

**Test Point:** TP16 (or J3/J4 pins 181-184 for backplane)

| Pin | Signal | Notes |
|-----|--------|-------|
| 181-184 | 12VIN | All 4 pins paralleled |

**Current Draw:** ~2-3A idle, more under load (depends on DUT)

**⚠️ WARNING:** No barrel jack — must use TP16 or backplane connector

### Onboard Regulators

| Rail | Voltage | Current | Regulator |
|------|---------|---------|-----------|
| VCCINT | ~1.0V | 3A | TLV62130 |
| VCCAUX | ~1.8V | 3A | TLV62130 |
| 3.3V | 3.3V | 3A | TLV62130 |
| 1.8V | 1.8V | 3A | TLV62130 |
| 1.5V | 1.5V | 3A | TLV62130 |
| 1.0V | 1.0V | 3A | TLV62130 |
| 1.2V (DDR) | 1.2V | LDO | TPS74012 |

### Bank Voltages (Programmable)

| Bank | Pins | Voltage Range | Regulator |
|------|------|---------------|-----------|
| B13 | 0-47 | 0.8V-3.3V | SC202AMLTRT |
| B33 | 48-95 | 0.8V-3.3V | SC202AMLTRT |
| B34 | 96-127 | 0.8V-3.3V | SC202AMLTRT |
| B35 | 128-159 | 0.8V-3.3V | SC202AMLTRT |

**Control:** `linux_IO_PS.elf <B13> <B33> <B34> <B35>` (legacy) or GUI (new)

---

## JTAG Interface (J1)

**Connector:** Molex 87832-1420 (2x7, 2mm pitch, shrouded)

### Pinout

```
    Top View (component side, keyed)
    ═══ = key notch
     ┌─────────────────┐
GND  │  1   2  │  VREF (3.3V from board)
GND  │  3   4  │  TMS → U1-G12 (via R132)
GND  │  5   6  │  TCK → U1-G11 (via R133)
GND  │  7   8  │  TDO ← U1-G14 (via R134)
GND  │  9  10  │  TDI → U1-H13 (via R135)
GND  │ 11  12  │  GND/NC
GND  │ 13  14  │  n_SRST → U12 (via R66)
     └─────────────────┘
       Pin 1 (square pad)
```

### Wiring for FT232H

| FT232H | J1 Pin | Signal |
|--------|--------|--------|
| AD0 | 6 | TCK |
| AD1 | 10 | TDI |
| AD2 | 8 | TDO |
| AD3 | 4 | TMS |
| GND | 1/3/5/7/9/11/13 | GND |
| VCCA (3.3V) | 2 | VREF (only if board unpowered) |

### Wiring for Basys 3

| Basys 3 JTAG | J1 Pin | Signal |
|--------------|--------|--------|
| Pin 1 (TCK) | 6 | TCK |
| Pin 2 (TMS) | 4 | TMS |
| Pin 3 (TDI) | 10 | TDI |
| Pin 4 (TDO) | 8 | TDO |
| Pin 5 (GND) | 1/3/5/etc | GND |
| Pin 6 (VREF) | 2 | VREF (optional) |

---

## GPIO Pin Assignments

### VICOR Enable Pins

| Core | MIO Pin | Package Pin | SLCR Config |
|------|---------|-------------|-------------|
| 1 | 0 | R7 | `configure_mio(0, GPIO)` |
| 2 | 39 | V4 | `configure_mio(39, GPIO)` |
| 3 | 47 | U7 | `configure_mio(47, GPIO)` |
| 4 | 8 | V12 | `configure_mio(8, GPIO)` |
| 5 | 38 | T4 | `configure_mio(38, GPIO)` |
| 6 | 37 | U4 | `configure_mio(37, GPIO)` |

**Note:** MIO 0 is shared with status LED — configuring as VICOR enable will toggle LED.

### Special Pin Functions

| Pin | Function | Bank | Notes |
|-----|----------|------|-------|
| 128 | Scope trigger | 35 | Direct FPGA |
| 129 | Error strobe | 35 | Direct FPGA |
| 130-131 | LVDS sync | 35 | Differential pair |
| 136-137 | SYSCLK0 | 35 | Clock input (differential) |

### Bank Assignments

| Bank | Pins | Type | Latency |
|------|------|------|---------|
| 13 | 0-47 | BIM | 2 cycles |
| 33 | 48-95 | BIM | 2 cycles |
| 34 | 96-127 | BIM | 2 cycles |
| 35 | 128-159 | Fast | 1 cycle |

---

## Test Points

| Test Point | Signal | Expected Value |
|------------|--------|----------------|
| TP16 | 12VIN | 12V (external supply) |
| Near USB | 3.3V | 3.3V ±5% |
| Near USB | 1.8V | 1.8V ±5% |
| XADC channels | VCCINT | ~1.0V |
| XADC channels | VCCAUX | ~1.8V |
| XADC channels | DIE_TEMP | Ambient + 10-20°C |

---

## Power-Up Sequence

### 1. Apply 12V

```
1. Connect 12V supply to TP16 (+) and GND (-)
2. Verify 3.3V, 1.8V regulators output
3. Check J1 pin 2 (VREF) → should read 3.3V
```

### 2. Load Firmware

**Option A: JTAG (temporary)**
```bash
# Using OpenOCD + FT232H
openocd -f openocd_zynq.cfg -c "load_image fbc-firmware.bin 0x00100000"
```

**Option B: SD Card (permanent)**
```bash
# Create BOOT.BIN
bootgen -image boot.bif -arch zynq -o BOOT.BIN

# Copy to SD card
cp BOOT.BIN /mnt/sdcard/
```

### 3. Verify AXI Access

```tcl
# Vivado Hardware Manager
open_hw
connect_hw_server
open_hw_target

# Read FBC version register
read_hw_regs -address 0x4004001C -size 4
# Expected: 0x00010000
```

### 4. Test Ethernet

```bash
# Run GUI
cd gui
npm run tauri dev

# Click "Discover Boards"
# Should see ANNOUNCE packet from board
```

---

## Troubleshooting

### No JTAG Connection

**Symptoms:** OpenOCD can't connect, IDCODE read fails

**Check:**
1. FT232H wiring (TCK/TDI/TDO/TMS)
2. 12V power applied
3. JTAG header seated properly
4. Pin 1 orientation (keyed)

### AXI Register Read Returns 0 or 0xFFFFFFFF

**Symptoms:** `read_hw_regs` returns 0x00000000 or 0xFFFFFFFF

**Causes:**
1. PL not programmed — load bitstream
2. Wrong address — verify base address
3. AXI interconnect issue — check `system_top.v`

### VICOR Cores Don't Enable

**Symptoms:** VICOR_ENABLE command sent, no voltage change

**Check:**
1. SLCR MIO configured — verify `main.rs:61-78`
2. MIO pin wiring — check schematic
3. VICOR DAC voltage — measure DAC output

### Ethernet ANNOUNCE Not Received

**Symptoms:** GUI shows "No boards discovered"

**Check:**
1. Ethernet cable connected
2. Same subnet (DHCP or static)
3. Firmware loaded and running
4. MAC address correct (from DNA)

---

## Reference Files

| File | Purpose |
|------|---------|
| `constraints/zynq7020_sonoma.xdc` | Pin constraints |
| `rtl/system_top.v` | Top-level integration |
| `firmware/src/main.rs` | Firmware entry point |
| `firmware/src/regs.rs` | Register definitions |
| `docs/ARCHITECTURE.md` | System architecture |
| `docs/MIGRATION.md` | Pattern converter migration |

---

**Next Steps:**
1. ✅ PL programmed
2. 🔴 Load PS firmware
3. 🔴 Verify AXI registers
4. 🔴 Test Ethernet ANNOUNCE
5. 🔴 Run vector test
