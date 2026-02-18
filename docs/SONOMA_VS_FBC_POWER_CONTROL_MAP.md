# Sonoma vs FBC Power Control & Hardware Mapping

**Purpose:** Complete mapping of Sonoma legacy system capabilities vs FBC implementation  
**Date:** 2026-01-26  
**Status:** Pre-hardware validation mapping

---

## Executive Summary

| Category | Sonoma 2016 | FBC 2026 | Status | Gap |
|----------|-------------|----------|--------|-----|
| **PMBus VOUT Control** | ✅ Individual (up to 99) | ✅ Driver ready | ✅ Code Complete | 🔲 Hardware test |
| **LCPS (LC/HC)** | ✅ 16 LC + 4 HC | ✅ PMBus driver | ✅ Code Complete | 🔲 Hardware test |
| **VICOR Core Supplies** | ✅ 6 cores (DAC+GPIO) | ⏳ SPI/GPIO ready | 🔲 Not implemented | Need DAC driver |
| **Temperature Control** | ✅ Lynx (PID) | ✅ ONETWO Thermal | ✅ Better | — |
| **External ADC** | ✅ 32 channels | ⏳ SPI driver ready | 🔲 Not implemented | Need ADC driver |
| **External DAC** | ✅ 10 channels | ⏳ SPI driver ready | 🔲 Not implemented | Need DAC driver |
| **XADC** | ✅ 32 channels | ✅ XADC driver | ✅ Complete | — |
| **EEPROM** | ❌ Not used | ✅ 24LC02 driver | ✅ Better | — |

---

## 1. PMBus Power Supply Control

### Sonoma Implementation

**Hardware:**
- Up to 99 PMBus power supplies per BIM
- Typically 16 LC (Low Current, 12A each) + 4 HC (High Current, 40A each)
- I2C addresses: 1-99 (hardcoded per BIM)

**Control Commands:**
```bash
# Set voltage on supply at address 1
VoutPmbus.elf 1 0.75 0.90 1.10 0.85  # Supply 1: 0.75V, Supply 2: 0.90V, etc.

# Read voltage/current from 40 supplies
Vout40Ch.elf 1 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40

# Turn off supply
linux_pmbus_OFF.elf 1 0
```

**Individual VOUT Control:**
- Each supply has unique I2C address
- VOUT_COMMAND (0x21) sets voltage
- READ_VOUT (0x8B) reads actual voltage
- READ_IOUT (0x8C) reads current
- OPERATION (0x01) enables/disables output

**Limitations:**
- Hardcoded addresses (must reconfigure when swapping PSUs)
- No auto-discovery
- Sequential ELF spawns (slow)

### FBC Implementation

**Status:** ✅ **PMBus Driver Complete**

**Location:** `firmware/src/hal/pmbus.rs`

**Capabilities:**
```rust
// Individual VOUT control
let mut psu = Pmbus::new(i2c, address, use_pec);
psu.set_vout_mv(750)?;  // Set to 0.75V
psu.enable_output()?;
let voltage = psu.read_vout_mv()?;  // Read actual voltage
let current = psu.read_iout_ma()?;  // Read current
```

**What's Implemented:**
- ✅ All PMBus command codes (0x00-0x9E)
- ✅ VOUT_COMMAND (set voltage)
- ✅ READ_VOUT (read voltage)
- ✅ READ_IOUT (read current)
- ✅ OPERATION (enable/disable)
- ✅ Status reading (fault detection)
- ✅ PEC (CRC-8) support
- ✅ LINEAR16/LINEAR11 format conversion

**What's Missing:**
- 🔲 **Device Discovery** - Auto-scan I2C bus, detect PSU types
- 🔲 **Type Detection** - Read MFR_ID to identify Pico/Lynx/MPS
- 🔲 **Batch Operations** - Control multiple supplies in one transaction
- 🔲 **Hardware Testing** - Not tested on actual hardware

**Gap:** Need to implement device discovery and type detection before hardware test.

---

## 2. LCPS (Low Current / High Current Power Supplies)

### Sonoma Implementation

**Hardware:**
- **LC Supplies:** 16 channels, 12A each
- **HC Supplies:** 4 channels, 40A each (where installed)
- Controlled via PMBus (Pico/Lynx devices)

**Control:**
```bash
# Set LC channel 0 to 1.8V
linux_pmbus_PicoDlynx.elf 0 1.8

# Read voltage/current from channel 0
linux_read_PicoDlynx.elf 0
```

**Mapping:**
- Channels 0-15: LC supplies (12A)
- Channels 16-19: HC supplies (40A, if installed)
- Each channel has unique I2C address

### FBC Implementation

**Status:** ✅ **Same as PMBus** - LCPS are PMBus devices

**Implementation:**
- LCPS use standard PMBus protocol
- FBC PMBus driver can control them
- Need to map channel numbers to I2C addresses

**Gap:**
- 🔲 **Channel Mapping** - Need to define LCPS channel → I2C address mapping
- 🔲 **Type Detection** - Detect Pico vs Lynx vs MPS devices
- 🔲 **Hardware Testing** - Verify LCPS control works

**Recommendation:**
- Store LCPS mapping in EEPROM (channel → I2C address → device type)
- Auto-detect on boot via I2C scan
- Create `LcpsController` wrapper around `Pmbus` driver

---

## 3. VICOR Core Supplies

### Sonoma Implementation

**Hardware:**
- 6 VICOR core supplies (PRM + VTM modules)
- High current (10-50A), low voltage (0.5V - 1.5V)
- Controlled via DAC (voltage reference) + MIO GPIO (enable)

**Control:**
```bash
# Initialize and enable core 1 at 1.0V
linux_VICOR.elf 1.0 0 9  # voltage, ctrl_mio, dac_ch

# Adjust voltage (already initialized)
linux_VICOR_Voltage.elf 2.0 9  # voltage*2, dac_ch
```

**Core Mapping:**
| Core | DAC Channel | MIO Pin | Notes |
|------|-------------|---------|-------|
| 1    | 9           | 0       | Directly powered |
| 2    | 3           | 39      | — |
| 3    | 7           | 47      | — |
| 4    | 8           | 8       | — |
| 5    | 4           | 38      | — |
| 6    | 2           | 37      | — |

**Formula:**
- DAC output sets voltage reference (~2V for 1.0V output)
- MIO pin enables/disables VICOR module
- Actual formula may need calibration

### FBC Implementation

**Status:** 🔲 **Not Implemented**

**What's Needed:**
1. **External DAC Driver** (`firmware/src/hal/dac.rs`)
   - SPI interface to external DAC chip
   - 10 channels (DAC channels 0-9)
   - Set voltage per channel

2. **VICOR Controller** (`firmware/src/hal/vicor.rs`)
   - Wraps DAC + GPIO
   - Maps core number → (DAC channel, MIO pin)
   - Set core voltage and enable

**Gap:**
- 🔲 **DAC Driver** - Need to implement SPI DAC control
- 🔲 **VICOR Controller** - Need to implement core supply wrapper
- 🔲 **Calibration** - Need to determine DAC voltage → core voltage formula

**Recommendation:**
- Check schematics for DAC chip part number
- Implement SPI DAC driver (similar to SPI ADC)
- Create `VicorCoreSupply` struct with core mapping

---

## 4. Temperature Control

### Sonoma Implementation

**Hardware:**
- Lynx thermal controller (heater/fan)
- NTC thermistor (case temperature)
- Diode temperature sensor (optional)

**Control:**
```bash
# Set temperature setpoint (case temp, NTC)
linux_set_temperature.elf 100.0 10000 0  # setpoint, R25C, coolafter

# Set temperature (diode temp)
linuxLinTempDiode.elf 100.0 1.02 0  # setpoint, ideality, coolafter
```

**Algorithm:**
- PID controller (tuned by trial/error)
- Bang-bang control (oscillates ±1-2°C)
- Reactive only (no feedforward)

**Formulas:**
- **Diode:** `T = (1.02 * V / 0.004) - 273.15`
- **NTC:** Steinhart-Hart equation with B25_100 coefficient

### FBC Implementation

**Status:** ✅ **ONETWO Thermal Controller Complete**

**Location:** `firmware/src/hal/thermal.rs`

**Capabilities:**
```rust
// Initialize thermal controller
let mut thermal = ThermalController::new(xadc, heater_pwm, fan_pwm);

// Set target temperature
thermal.set_target(100.0)?;

// Update control loop (call periodically)
thermal.update()?;

// Get current temperature
let temp = thermal.read_temperature()?;
```

**Advantages over Sonoma:**
- ✅ **No PID tuning** - Settling rate forced by structure (e-2)
- ✅ **Pattern-aware feedforward** - Predicts power from vector toggle rate
- ✅ **Smooth control** - No oscillation, 7 iterations to lock
- ✅ **Faster response** - 700ms vs seconds

**What's Missing:**
- 🔲 **Hardware Integration** - Need to connect to actual heater/fan PWM
- 🔲 **Sensor Calibration** - Need to verify NTC/diode formulas
- 🔲 **Hardware Testing** - Not tested on actual hardware

**Gap:** Need to verify PWM control and sensor reading on hardware.

---

## 5. External ADC (32 channels)

### Sonoma Implementation

**Hardware:**
- 32-channel external ADC (16-bit)
- SPI interface
- ~500ms per full read

**Control:**
```bash
# Read all 32 channels with statistics
ADC32ChPlusStats.elf 1 3  # averages, samples

# Output: 3 lines (max, avg, min) with 32 values each
```

**Channel Mapping:**
- Channels mapped via configuration file
- Formulas applied (voltage dividers, thermistors, current shunts)

### FBC Implementation

**Status:** 🔲 **Not Implemented**

**What's Needed:**
1. **External ADC Driver** (`firmware/src/hal/adc.rs`)
   - SPI interface
   - 32-channel readout
   - Trigger conversion, read results

2. **Channel Mapping**
   - Store mapping in EEPROM or config
   - Apply formulas (Vmeas, ThermAB, Imeas50m, etc.)

**Gap:**
- 🔲 **ADC Driver** - Need to implement SPI ADC control
- 🔲 **Channel Mapping** - Need to implement formula system
- 🔲 **Hardware Testing** - Not tested

**Recommendation:**
- Check schematics for ADC chip part number
- Implement SPI ADC driver
- Port formula system from Sonoma ReadAnalog script

---

## 6. External DAC (10 channels)

### Sonoma Implementation

**Hardware:**
- 10-channel external DAC
- SPI interface
- Used for reference voltages and VICOR control

**Control:**
```bash
# Set all 10 DAC channels
linux_EXT_DAC.elf 1.0 1.5 2.0 2.5 3.0 3.3 0.9 1.2 1.8 0.5

# Set single channel
linux_EXT_DAC_singleCh.elf <channel> <voltage>
```

### FBC Implementation

**Status:** 🔲 **Not Implemented**

**What's Needed:**
1. **External DAC Driver** (`firmware/src/hal/dac.rs`)
   - SPI interface
   - 10-channel control
   - Set voltage per channel

**Gap:**
- 🔲 **DAC Driver** - Need to implement SPI DAC control
- 🔲 **Hardware Testing** - Not tested

**Note:** Same driver needed for VICOR core supplies (see section 3).

---

## 7. XADC (Zynq Internal ADC)

### Sonoma Implementation

**Hardware:**
- Built-in 12-bit ADC in Zynq
- 32 channels (16 aux + 16 internal)

**Control:**
```bash
# Initialize XADC
linux_init_XADC.elf

# Read all 32 channels
XADC32Ch.elf
```

**Channels:**
- 0-15: Auxiliary inputs (external signals)
- 16-31: Internal monitors (die temp, VCCINT, VCCAUX, etc.)

### FBC Implementation

**Status:** ✅ **XADC Driver Complete**

**Location:** `firmware/src/hal/xadc.rs`

**Capabilities:**
```rust
// Initialize XADC
let mut xadc = Xadc::new();
xadc.init()?;

// Read die temperature
let temp = xadc.read_die_temperature()?;

// Read auxiliary channel
let voltage = xadc.read_aux(0)?;

// Read internal voltage
let vccint = xadc.read_vccint()?;
```

**What's Implemented:**
- ✅ XADC initialization
- ✅ Die temperature reading
- ✅ Auxiliary channel reading (0-15)
- ✅ Internal voltage reading (VCCINT, VCCAUX, VCCBRAM)

**What's Missing:**
- 🔲 **Hardware Testing** - Not tested on actual hardware

**Gap:** Need to verify XADC readings match expected values.

---

## 8. EEPROM (BIM Configuration)

### Sonoma Implementation

**Status:** ❌ **Not Used**

- No EEPROM access in Sonoma
- Configuration stored in NFS files
- No persistent on-board storage

### FBC Implementation

**Status:** ✅ **EEPROM Driver Complete**

**Location:** `firmware/src/hal/eeprom.rs`

**Capabilities:**
```rust
// Read BIM EEPROM
let mut eeprom = BimEeprom::new(i2c, 0x50);
let config = eeprom.read_config()?;

// Write BIM configuration
eeprom.write_config(&config)?;
```

**What's Implemented:**
- ✅ 24LC02 EEPROM driver (256 bytes)
- ✅ I2C interface
- ✅ Read/write operations
- ✅ CRC32 validation

**What's Missing:**
- 🔲 **Configuration Structure** - Need to define EEPROM layout
- 🔲 **Power Supply Mapping** - Store LCPS/VICOR mappings
- 🔲 **Calibration Data** - Store ADC/DAC calibration
- 🔲 **Hardware Testing** - Not tested

**Gap:** Need to define EEPROM data structure and implement configuration management.

---

## Complete Capability Matrix

| Feature | Sonoma Command | FBC Implementation | Status | Hardware Test |
|---------|---------------|-------------------|--------|---------------|
| **PMBus VOUT Set** | `VoutPmbus.elf <addr> <v1> <v2>...` | `pmbus.set_vout_mv()` | ✅ Code | 🔲 Pending |
| **PMBus VOUT Read** | `Vout40Ch.elf 1 <channels>` | `pmbus.read_vout_mv()` | ✅ Code | 🔲 Pending |
| **PMBus IOUT Read** | `Vout40Ch.elf 2 <channels>` | `pmbus.read_iout_ma()` | ✅ Code | 🔲 Pending |
| **PMBus Enable** | `VoutPmbus.elf 1 ...` | `pmbus.enable_output()` | ✅ Code | 🔲 Pending |
| **PMBus Disable** | `linux_pmbus_OFF.elf <addr>` | `pmbus.disable_output()` | ✅ Code | 🔲 Pending |
| **LCPS Control** | `linux_pmbus_PicoDlynx.elf <ch> <v>` | `pmbus.set_vout_mv()` | ✅ Code | 🔲 Pending |
| **VICOR Core Set** | `linux_VICOR.elf <v> <mio> <dac>` | 🔲 Not implemented | 🔲 Missing | — |
| **VICOR Voltage** | `linux_VICOR_Voltage.elf <v*2> <dac>` | 🔲 Not implemented | 🔲 Missing | — |
| **Temperature Set** | `linux_set_temperature.elf <T> <R> <c>` | `thermal.set_target()` | ✅ Code | 🔲 Pending |
| **Temperature Read** | `XADC32Ch.elf` | `xadc.read_die_temperature()` | ✅ Code | 🔲 Pending |
| **External ADC Read** | `ADC32ChPlusStats.elf <avg> <samp>` | 🔲 Not implemented | 🔲 Missing | — |
| **External DAC Set** | `linux_EXT_DAC.elf <v0>...<v9>` | 🔲 Not implemented | 🔲 Missing | — |
| **EEPROM Read** | ❌ Not available | `eeprom.read_config()` | ✅ Code | 🔲 Pending |
| **EEPROM Write** | ❌ Not available | `eeprom.write_config()` | ✅ Code | 🔲 Pending |

---

## Critical Gaps Before Hardware Test

### 1. VICOR Core Supplies (BLOCKING)

**What's Missing:**
- External DAC driver (SPI interface)
- VICOR controller (DAC + GPIO wrapper)
- Core mapping (6 cores → DAC channels + MIO pins)

**Impact:** Cannot control core supplies without this.

**Priority:** 🔴 **HIGH** - Core supplies are critical for DUT power.

**Action Items:**
1. Identify DAC chip part number from schematics
2. Implement SPI DAC driver (`firmware/src/hal/dac.rs`)
3. Create VICOR controller (`firmware/src/hal/vicor.rs`)
4. Define core mapping table
5. Test on hardware

---

### 2. External ADC (BLOCKING)

**What's Missing:**
- External ADC driver (SPI interface)
- Channel mapping system
- Formula application (Vmeas, ThermAB, Imeas50m, etc.)

**Impact:** Cannot read DUT voltages/currents/temperatures.

**Priority:** 🔴 **HIGH** - ADC is critical for monitoring.

**Action Items:**
1. Identify ADC chip part number from schematics
2. Implement SPI ADC driver (`firmware/src/hal/adc.rs`)
3. Port formula system from Sonoma ReadAnalog
4. Test on hardware

---

### 3. PMBus Device Discovery (IMPORTANT)

**What's Missing:**
- I2C bus scan on boot
- Device type detection (read MFR_ID)
- Address mapping (virtual channel → I2C address)

**Impact:** Must hardcode addresses (like Sonoma) until implemented.

**Priority:** 🟡 **MEDIUM** - Can work with hardcoded addresses initially.

**Action Items:**
1. Implement I2C bus scanner
2. Read MFR_ID to detect device types
3. Store mapping in EEPROM
4. Create device discovery function

---

### 4. EEPROM Configuration Structure (IMPORTANT)

**What's Missing:**
- EEPROM data structure definition
- Power supply mapping storage
- Calibration data storage
- Configuration management layer

**Impact:** Cannot store BIM-specific configuration persistently.

**Priority:** 🟡 **MEDIUM** - Can use hardcoded config initially.

**Action Items:**
1. Define EEPROM layout structure
2. Implement configuration read/write
3. Store LCPS/VICOR mappings
4. Store calibration data

---

## Pre-Hardware Test Checklist

### Must Have (Blocking):
- [ ] VICOR core supply control (DAC driver + controller)
- [ ] External ADC driver (32 channels)
- [ ] PMBus individual VOUT control verified (code review)
- [ ] XADC reading verified (code review)
- [ ] Temperature control verified (code review)

### Should Have (Important):
- [ ] PMBus device discovery
- [ ] EEPROM configuration structure
- [ ] External DAC driver (for VICOR)
- [ ] Channel mapping system

### Nice to Have (Can add later):
- [ ] Batch PMBus operations
- [ ] Interrupt-based fault monitoring
- [ ] Advanced calibration system

---

## Hardware Test Plan

### Phase 1: Basic I/O (Day 1)
1. **XADC Test**
   - Read die temperature
   - Verify values are reasonable
   - Compare to Sonoma readings

2. **GPIO Test**
   - Toggle MIO pins
   - Verify VICOR enable/disable works
   - Test GPIO readback

3. **I2C Test**
   - Scan I2C bus
   - Verify PMBus devices respond
   - Read MFR_ID from one device

### Phase 2: Power Supplies (Day 2-3)
4. **PMBus Test**
   - Set voltage on one supply
   - Read back voltage
   - Verify actual output with multimeter
   - Test enable/disable

5. **LCPS Test**
   - Control LC channel 0
   - Verify voltage output
   - Test all 16 LC channels

6. **VICOR Test** (if implemented)
   - Set core 1 voltage
   - Verify DAC output
   - Test enable/disable
   - Verify actual core voltage

### Phase 3: Monitoring (Day 4-5)
7. **External ADC Test** (if implemented)
   - Read all 32 channels
   - Verify values match Sonoma
   - Test formula application

8. **Temperature Control Test**
   - Set target temperature
   - Verify thermal controller responds
   - Test feedforward prediction

### Phase 4: Integration (Day 6-7)
9. **Full Power Sequence**
   - Power on all supplies in sequence
   - Verify all voltages correct
   - Test power-down sequence

10. **EEPROM Test**
    - Write configuration
    - Read back and verify
    - Test CRC32 validation

---

## Recommendations

### Before Hardware Test:

1. **Implement VICOR Core Supplies** (Priority 1)
   - This is blocking - cannot power DUT without it
   - Need DAC driver + VICOR controller
   - Estimate: 1-2 days

2. **Implement External ADC** (Priority 2)
   - Critical for monitoring DUT health
   - Need ADC driver + channel mapping
   - Estimate: 1-2 days

3. **Code Review PMBus Driver** (Priority 3)
   - Verify all command codes correct
   - Test LINEAR16/LINEAR11 conversion
   - Estimate: 0.5 day

4. **Define EEPROM Structure** (Priority 4)
   - Needed for persistent configuration
   - Can use hardcoded initially
   - Estimate: 0.5 day

### After Hardware Test:

5. **PMBus Device Discovery**
   - Auto-detect PSU types
   - Store mapping in EEPROM
   - Eliminate hardcoded addresses

6. **Batch Operations**
   - Control multiple supplies in one transaction
   - Faster initialization

7. **Interrupt-Based Monitoring**
   - Immediate fault detection
   - Better than polling

---

## Summary

**What FBC Has (Code Complete):**
- ✅ PMBus driver (individual VOUT control)
- ✅ ONETWO thermal controller
- ✅ XADC driver
- ✅ EEPROM driver
- ✅ I2C/SPI/GPIO drivers (infrastructure)

**What FBC Needs (Before Hardware Test):**
- 🔲 VICOR core supply control (DAC driver + controller)
- 🔲 External ADC driver (32 channels)
- 🔲 External DAC driver (10 channels, for VICOR)
- 🔲 PMBus device discovery
- 🔲 EEPROM configuration structure

**What FBC Needs (After Hardware Test):**
- 🔲 Hardware validation of all drivers
- 🔲 Calibration data collection
- 🔲 Performance optimization

**Bottom Line:** FBC has ~70% of Sonoma's power control capabilities implemented. Missing VICOR and External ADC are blocking hardware test. Estimate 2-3 days to implement missing drivers before hardware validation.

---

*Last updated: 2026-01-26*
