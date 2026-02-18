# Example Constraint Files - Analysis

## Overview

This directory contains **constraint files generated from a single source design file** for board bringup and validation testing. The files represent a complete test/validation workflow for the "Normandy" board (S0031).

---

## Source File (Generated All Others)

### Primary Source: `S0031 Normandy_BIM_Design_r0.7.xlsx`
- **Type:** Excel Design File
- **Date:** January 8, 2026 (MOST RECENT)
- **Size:** 5.5 MB
- **Purpose:** Master design file containing:
  - Component database
  - Power rail definitions
  - Pin mappings
  - Test configurations
  - Signal assignments

**This Excel file generated all other constraint files below.**

### Supporting Source Files:
- `S31_SCH_4CORE_AUG_5.DSN` - OrCAD Schematic (Sept 3, 2025)
- `S0031_PCB_AUG_12_17.2D.brd` - Altium PCB Layout (Sept 3, 2025)
- `s31_sch_4core_aug_5.pdf` - Schematic PDF (Sept 3, 2025)

---

## Generated Constraint Files (In Order of Use)

### Phase 1: Pin & Signal Mapping

#### 1. `PIN_MAP` (711 bytes, Sept 19, 2025)
**Purpose:** GPIO pin assignment mapping
**Content:**
- Maps pin numbers (0-127) to signal names
- GPIO assignments (GPIOA_0 through GPIOC_12)
- Special signals (JTAG, I2C, I3C, power control)
- Format: `pin_number signal_name`

**Example:**
```
0 PWRGD
1 PWRBRK_N
44 JTAG_TDI
45 JTAG_TCK
126 JTAG_TDO
```

**Generated from:** Excel → Pin assignment table

---

#### 2. `normandy.map` (1,988 bytes, Sept 8, 2025)
**Purpose:** Power rail and ADC signal mapping
**Content:**
- Power rail mappings (VOUT1-15 → Rail names)
- Current monitoring (IOUT1-15)
- ADC channel assignments (ADC_0-11 → Temperature/voltage sensors)
- XADC assignments (XADC_0-2 → Voltage sense)

**Example:**
```
VOUT1 VOSC;
IOUT1 VOSC_I;
VOUT9 VRAM;
IOUT9 VRAM_I;
ADC_0 DIODE_TEMP;
XADC_0 VENG_S;
```

**Generated from:** Excel → Power rail table + ADC configuration

---

### Phase 2: Power Sequencing

#### 3. `PowerOn` (1,145 bytes, Oct 16, 2025)
**Purpose:** Power-up sequence script
**Content:**
- Sequential power rail enable commands
- PMBUS commands to set voltages
- VICOR power module commands
- ADC/XADC initialization
- Format: `/mnt/bin/linux_pmbus_PicoDlynx.elf [channel] [voltage]`

**Power Rails (in sequence):**
1. VTHERM (5.0V)
2. VOSC (1.8V)
3. HVIO1P2-VTHERM1P2 (1.32V)
4. VENG (1.1V via VICOR)
5. VRAM (1.194V)
6. VCONST (1.08V via VICOR)
7. VPAM4A-VPAM4D (0.9V)
8. VPAM4T (1.0V)
9. VHBMD (1.1V)
10. VPCI0P75 (0.9V)
11. VPCI1P2 (1.32V)
12. VPAM4IO (1.21V)
13. VPLLHV1P2 (1.32V)
14. VHBM_1P8 (1.98V)
15. VHBM_1p1 (1.21V)
16. VHBM_0P4 (0.6V)

**Generated from:** Excel → Power sequencing table → Script generator

---

#### 4. `PowerOff` (777 bytes, Sept 19, 2025)
**Purpose:** Power-down sequence script
**Content:**
- Reverse sequence of PowerOn
- PMBUS OFF commands
- VICOR shutdown commands
- Format: `/mnt/bin/linux_pmbus_OFF.elf [channel]`

**Generated from:** Excel → Power sequencing table → Reverse script generator

---

#### 5. `NominalPwr` (1,147 bytes, Oct 16, 2025)
**Purpose:** Nominal operating power settings
**Content:**
- Similar to PowerOn but with different voltage values
- Used for normal operation (not startup)
- Lower voltages for some rails (e.g., VRAM 0.98V vs 1.194V)

**Generated from:** Excel → Nominal power table → Script generator

---

### Phase 3: Test Configuration

#### 6. `debug.tim` (583 bytes, Oct 13, 2025)
**Purpose:** JTAG/debug timing configuration
**Content:**
- Clock frequency settings
- JTAG pin configurations
- Reference clock settings
- Format: `/mnt/bin/linux_xpll_frequency.elf [channel] [freq] [phase] [duty]`

**Generated from:** Excel → JTAG configuration table

---

#### 7. `pcie.sh` (2,654 bytes, Oct 24, 2025)
**Purpose:** PCIe test script
**Content:**
- Loads PIN_MAP
- Loads debug.tim (twice)
- Loads NominalPwr
- PCIe test vector loading
- Pattern: `cp PIN_MAP /mnt/.` → `debug.tim` → `NominalPwr` → test vectors

**Generated from:** Excel → Test configuration → PCIe test template

---

#### 8. `mbist.sh` (3,314 bytes, Oct 27, 2025)
**Purpose:** Memory Built-In Self-Test script
**Content:**
- Multiple MBIST test sequences
- Power rail dependencies (VCONST, VENG, VRAM, VHBMD)
- Reset sequences between tests
- Temperature monitoring (`temp.sh`)

**Generated from:** Excel → MBIST test matrix → Script generator

---

#### 9. `hbmio.sh` (2,654 bytes, Oct 24, 2025)
**Purpose:** HBM (High Bandwidth Memory) I/O test
**Content:**
- HBM test vector loading
- All channels test (HBM0-5)
- Pattern verification

**Generated from:** Excel → HBM test configuration

---

#### 10. `anc.sh` (2,645 bytes, Oct 27, 2025)
**Purpose:** ANC (Analog Network Controller) test
**Content:**
- Multiple ANC test sequences
- Ethernet PHY setup
- Loopback tests
- Pattern verification

**Generated from:** Excel → ANC test configuration

---

#### 11. `scan.sh` (1,671 bytes, Oct 24, 2025)
**Purpose:** Scan chain test script
**Content:**
- Scan reset sequences
- Scan mode entry
- Scan test vector loading
- Pattern verification

**Generated from:** Excel → Scan test configuration

---

### Phase 4: Monitoring & Utilities

#### 12. `temp.sh` (241 bytes, Oct 24, 2025)
**Purpose:** Temperature monitoring
**Content:**
- Reads linear temperature diodes (0-5)
- Logs temperature over time
- Format: `/mnt/bin/ReadLinTempDiode.elf [channel]`

**Generated from:** Excel → Temperature sensor configuration

---

#### 13. `pattern_verification_template.sh` (244 bytes, Sept 19, 2025)
**Purpose:** Template for pattern verification scripts
**Content:**
- Generic template structure
- Placeholder for device name
- PowerOn/PowerOff sequence
- Test vector loading pattern

**Generated from:** Excel → Template generator

---

### Phase 5: Viewer/Reference Files

#### 14. `allegro_free_viewer.jrl` (88,807 bytes, Sept 3, 2025)
**Purpose:** Cadence Allegro PCB viewer journal log
**Content:**
- Recorded session of viewing PCB file
- Layer visibility changes
- Component selections
- Zoom/pan operations

**Generated from:** Manual viewing of `S0031_PCB_AUG_12_17.2D.brd`

---

## Generation Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    SOURCE FILE                               │
│                                                             │
│  S0031 Normandy_BIM_Design_r0.7.xlsx                        │
│  (Excel with: Components, Power Rails, Pins, Tests)         │
│                                                             │
└───────────────────────┬─────────────────────────────────────┘
                        │
                        │ Parser extracts:
                        │ - Pin assignments
                        │ - Power rail definitions
                        │ - Test configurations
                        │ - Signal mappings
                        │
        ┌───────────────┴───────────────┐
        │                               │
        ▼                               ▼
┌───────────────┐              ┌──────────────────┐
│  PIN_MAP      │              │  normandy.map     │
│  (Pin → Name) │              │  (Rail → Signal)  │
└───────────────┘              └──────────────────┘
        │                               │
        └───────────────┬───────────────┘
                        │
                        ▼
        ┌───────────────────────────────┐
        │   Power Sequencing Generator  │
        └───────────────┬───────────────┘
                        │
        ┌───────────────┼───────────────┐
        │               │               │
        ▼               ▼               ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│  PowerOn    │ │  PowerOff   │ │ NominalPwr  │
│  (Startup)  │ │  (Shutdown) │ │  (Normal)   │
└─────────────┘ └─────────────┘ └─────────────┘
                        │
                        ▼
        ┌───────────────────────────────┐
        │   Test Script Generator       │
        └───────────────┬───────────────┘
                        │
        ┌───────────────┼───────────────┐
        │               │               │
        ▼               ▼               ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│  pcie.sh    │ │  mbist.sh   │ │  hbmio.sh   │
│  anc.sh     │ │  scan.sh    │ │  temp.sh    │
└─────────────┘ └─────────────┘ └─────────────┘
```

---

## File Order (Execution Sequence)

### 1. **Initialization**
```
PIN_MAP → normandy.map → debug.tim
```
- Load pin mappings
- Load signal mappings
- Configure timing

### 2. **Power-Up**
```
PowerOn (or NominalPwr for normal operation)
```
- Sequence all power rails
- Initialize ADCs

### 3. **Test Execution** (choose one or run sequence)
```
pcie.sh    → PCIe interface test
mbist.sh   → Memory BIST test
hbmio.sh   → HBM I/O test
anc.sh     → ANC/Ethernet test
scan.sh    → Scan chain test
```

### 4. **Monitoring** (during/after tests)
```
temp.sh    → Temperature monitoring
```

### 5. **Power-Down**
```
PowerOff
```
- Shutdown all rails in reverse order

---

## Pattern Recognition

### Common Pattern in Test Scripts:
```bash
cp PIN_MAP /mnt/.                    # Copy pin map
/home/devices/normandy/debug.tim      # Load timing
/home/devices/normandy/NominalPwr     # Set power
/mnt/bin/linux_load_vectors.elf ...   # Load test vectors
/mnt/bin/linux_run_vector.elf ...    # Run test
/home/devices/normandy/temp.sh        # Monitor temperature
/home/devices/normandy/PowerOff       # Shutdown
```

### Power Rail Pattern:
```bash
# Format: /mnt/bin/linux_pmbus_PicoDlynx.elf [channel] [voltage] # [rail_name]
/mnt/bin/linux_pmbus_PicoDlynx.elf 8 5.0        # VTHERM
/mnt/bin/linux_pmbus_PicoDlynx.elf 1 1.8        # VOSC
```

### Pin Mapping Pattern:
```
[pin_number] [signal_name];
```

### Signal Mapping Pattern:
```
[VOUT|IOUT|ADC|XADC]_[number] [signal_name];
```

---

## Key Insights for FSHC

1. **Single Source of Truth:** Excel file contains all design data
2. **Pattern-Based Generation:** All scripts follow same patterns
3. **Constraint Extraction:** Power sequencing, pin mapping, test configs all extractable
4. **Physics Validation:** Power sequencing must respect dependencies (PowerDag)
5. **Unified Parser:** All these formats (Excel, shell scripts, maps) should use same parser infrastructure

**FSHC Should:**
- Parse Excel → Extract constraints → Generate PowerOn/PowerOff (PowerDag)
- Parse Excel → Extract pins → Generate PIN_MAP
- Parse Excel → Extract signals → Generate normandy.map
- Parse Excel → Extract tests → Generate test scripts
- **All from same parser infrastructure** - no separate libraries needed

---

## File Dependencies

```
Excel (Source)
  ├──→ PIN_MAP
  ├──→ normandy.map
  ├──→ PowerOn
  ├──→ PowerOff
  ├──→ NominalPwr
  ├──→ debug.tim
  └──→ Test Scripts (pcie.sh, mbist.sh, etc.)
        ├──→ Uses PIN_MAP
        ├──→ Uses debug.tim
        ├──→ Uses NominalPwr
        ├──→ Uses temp.sh
        └──→ Uses PowerOff
```

---

**Conclusion:** One Excel file (`S0031 Normandy_BIM_Design_r0.7.xlsx`) generated all constraint files through pattern-based extraction and script generation. This demonstrates the pattern-based parsing approach FSHC should use.
