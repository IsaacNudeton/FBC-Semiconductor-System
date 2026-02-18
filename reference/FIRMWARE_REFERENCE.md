# Sonoma/Everest Firmware Architecture Reference

Reference document for understanding the existing Linux-based firmware to inform bare-metal Rust optimization.

---

## System Overview

The current system runs **PetaLinux on Zynq 7020** with:
- AWK scripts orchestrating test execution
- Compiled ELF binaries for hardware I/O
- Shell scripts for initialization and monitoring
- File-based IPC between threads (`/tmp/` files)

### Boot Sequence (init.sh)
```
1. Load FPGA bitstream → /dev/xdevcfg
2. Set MAC address → ResetMac.awk
3. Wake CPU1 → linux_cpu1_wakeup.elf
4. Power down I/O banks → linux_IO_PS.elf 0 0 0 0
5. Power down all PSUs → linux_pmbus_OFF.elf (loop 1-99)
6. Zero DACs → linux_EXT_DAC.elf 0 0 0 0 0 0 0 0 0 0
7. Set MIO lines low → ToggleMio.elf
8. Init XADC → linux_init_XADC.elf + linux_XADC.elf
9. Contact Everest server
10. Start network monitor
```

---

## Hardware Interfaces

### 1. PMBus / I2C Power Supplies

**Current Implementation:**
- Each PSU has unique I2C address (1-99 range)
- `linux_pmbus_OFF.elf <addr> 0` - Power off PSU
- `VoutPmbus.elf <addr> <voltage> <delay> <mode>` - Set voltage
  - mode=0: Initialize only
  - mode=1: Initialize + power on
  - mode=2: Adjust voltage (already initialized)
- `Vout40Ch.elf <hwType> <addr:type>...` - Batch read V/I from up to 40 PSUs
  - hwType=1: Address has embedded type
  - hwType=2: Detect hardware type

**Address Problem:**
The current system hardcodes PMBus addresses and requires manual reconfiguration when swapping PSUs. Each PSU type has different detection.

**Optimization Opportunities:**
1. **Auto-discovery**: Scan I2C bus on boot, enumerate all devices
2. **Type detection**: Read manufacturer ID register to auto-detect PSU type
3. **Address remapping**: Virtual address table → physical address mapping
4. **Batch operations**: Single transaction for multiple PSUs instead of serial calls
5. **Caching**: Cache PSU types after first detection

### 2. Pico (Power Supply Controller)

**What it is:** Vicor power module controller for core supplies

**Current Implementation:**
```
linux_VICOR.elf <voltage> <ctrl_mio> <dac_ch>  # Initialize + enable
linux_VICOR_Voltage.elf <voltage*2> <dac_ch>   # Adjust voltage
```

Core supply mapping:
| Core | DAC Ch | MIO Ctrl |
|------|--------|----------|
| 1    | 9      | 0        |
| 2    | 3      | 39       |
| 3    | 7      | 47       |
| 4    | 8      | 8        |
| 5    | 4      | 38       |
| 6    | 2      | 37       |

**Optimization:**
- Direct DAC register writes instead of ELF spawn
- MIO control via memory-mapped GPIO
- No shell overhead

### 3. Lynx (Thermal Controller)

**What it is:** Temperature control module (heater/fan)

**Current Implementation:**
```
linux_set_temperature.elf <setpoint> <R25C> <coolafter>  # Case temp (NTC)
linuxLinTempDiode.elf <setpoint> <ideality> <coolafter>  # Diode temp
```

Temperature formulas in ReadAnalog:
- **Diode temp (formula 2):** `T = (1.02 * V / 0.004) - 273.15`
- **Case temp (formula 3):** NTC thermistor with B25_100 coefficient
  - R25C = 10000Ω → B = 3492.0
  - R25C = 30000Ω → B = 3985.3
  - `RT = (4980 * (4.096/V)) - 4980 - 150`
  - `T = 1/((ln(RT/R25C)/B) + 1/298.15) - 273.15`

**Optimization:**
- PID loop in bare-metal (no shell/file IPC)
- Direct PWM control for heater/fan
- Faster response time

### 4. XADC (Zynq Internal ADC)

**What it is:** Built-in 12-bit ADC for FPGA temperature/voltage monitoring

**Current Implementation:**
```
linux_init_XADC.elf       # Initialize XADC
linux_XADC.elf            # Read values
XADC32Ch.elf              # Read 32 channels with min/max/avg
```

Channels mapped via mapping file:
```
XADC_<ch> <signal_name>
```

**Optimization:**
- Direct XADC register access (0xF8007100)
- DMA for continuous sampling
- No userspace ELF overhead

### 5. External ADC (32 channels)

**Current Implementation:**
```
ADC32ChPlusStats.elf      # Read all 32 channels, compute min/max/avg
```

Output format: 3 lines (max, avg, min) comma-separated values

**Optimization:**
- SPI DMA burst reads
- Hardware averaging in FPGA fabric

### 6. External DAC (10 channels)

**Current Implementation:**
```
linux_EXT_DAC.elf <v0> <v1> <v2> <v3> <v4> <v5> <v6> <v7> <v8> <v9>
linux_EXT_DAC_singleCh.elf  # Single channel update
```

**Optimization:**
- Direct SPI writes
- Batch DAC updates in single transaction

### 7. FPGA Vector Engine

**Current Implementation:**
```
linux_load_vectors.elf <seq_file> <hex_file>  # Load vector data
RunSuperVector.elf <seq_file> <time> <debug> <run_count> >> log
linux_pin_type.elf                             # Configure pin types
linux_Pulse_Delays.elf                         # Set pulse timing
linux_xpll_frequency.elf 1 <freq_hz>          # Set vector clock
```

Vector execution controlled via AXI registers (memory-mapped).

**Optimization:**
- DMA vector loading (already done in FPGA)
- Direct register control from ARM
- Eliminate file-based sequencing

---

## IPC Architecture (Current - File Based)

All inter-thread communication uses `/tmp/` files with `flock`:

| File | Purpose |
|------|---------|
| `/tmp/LockBit` | Hardware access mutex |
| `/tmp/TempLimitLock` | Temperature limits file mutex |
| `/tmp/TemperatureLimits` | Current temp LL/UL/R25C |
| `/tmp/CoreSettings` | Core voltage settings |
| `/tmp/VectorName` | Current test step + vector name |
| `/tmp/Shutdown` | Shutdown signal |
| `/tmp/JobStatus` | Firmware state for Everest sync |
| `/tmp/JobRunControl` | Everest run commands |
| `/tmp/WatchStatus` | Watched measurements |
| `/tmp/ErrorLog` | Error messages |

**Optimization:**
- Replace with shared memory or direct function calls
- No file I/O overhead
- No flock overhead
- Atomic operations for state

---

## Test Execution Flow (RunVectors)

```
1. Parse test plan → extract test steps
2. For each test step:
   a. Set temperature (if changed)
   b. Update temp limits file
   c. Sleep for ramp rate
   d. Run timing file
   e. Execute PU_LIST (power sequencing)
   f. Start analog sampling thread (first pass)
   g. Set vector clock frequency
   h. Load + run vectors
3. Loop until duration exceeded or abort
4. Write "Shutdown" to signal completion
```

### Power Supply Sequencing (PU_LIST)
Format: `TYPE,ADDR,VOLTAGE,DELAY:TYPE,ADDR,VOLTAGE,DELAY:...`

Types:
- `VOUT` - PMBus power supply
- `CORE` - Vicor core supply (uses DAC + MIO)
- `APS` - Analog power supply

Two-pass execution:
1. First pass: Initialize all supplies
2. Second pass: Enable all supplies

---

## Analog Sampling (ReadAnalog)

Continuous loop:
```
1. Read ADC32Ch (external 32-ch ADC)
2. Read XADC32Ch (internal XADC)
3. Read Vout40Ch (PMBus V/I)
4. Map readings to signal names
5. Apply formulas
6. Check shutdown limits
7. Log to CSV if sampling enabled
8. Send to Everest via UDP
```

Sampling modes:
- Mode 0: No sampling
- Mode 1: Slow (>2s interval) - write immediately
- Mode 2: Fast (<2s) - buffer in /tmp, flush every 2s

---

## Network Communication

`send_server.sh` sends UDP messages to Everest:
- `HEADER` - Column names
- `SAMPLE` - Current readings
- `MINMAX` - Min/max since last report
- `WATCH` - Watched channel values
- `SHUTDOWN` - Shutdown event + reason
- `UPDATE` - State changes
- `ERROR` - Error messages

---

## Bare-Metal Optimization Strategy

### 1. Replace File IPC with Direct State
```rust
struct SystemState {
    temp_limits: TempLimits,
    core_settings: [CoreSetting; 6],
    current_vector: VectorInfo,
    shutdown: AtomicBool,
    // ... etc
}
```

### 2. Replace ELF Spawning with Direct Calls
Instead of: `system("linux_XADC.elf")`
Do: `xadc.read_all_channels()`

### 3. Eliminate Locking Overhead
- Single-threaded event loop or
- Proper RTOS tasks with message passing

### 4. PMBus Auto-Configuration
```rust
fn discover_pmbus_devices() -> Vec<PmbusDevice> {
    let mut devices = Vec::new();
    for addr in 0x10..0x7F {
        if let Ok(mfr_id) = i2c.read_byte(addr, PMBUS_MFR_ID) {
            let device_type = match mfr_id {
                0x41 => DeviceType::MPS,
                0x49 => DeviceType::Infineon,
                // ...
            };
            devices.push(PmbusDevice { addr, device_type });
        }
    }
    devices
}
```

### 5. Direct Hardware Access
```rust
// XADC base address
const XADC_BASE: u32 = 0xF800_7100;

fn read_xadc_temp() -> f32 {
    let raw = unsafe { read_volatile((XADC_BASE + 0x200) as *const u32) };
    // Convert to temperature
    (raw as f32 / 65536.0) * 503.975 - 273.15
}
```

---

## Key Addresses (Zynq 7020)

| Peripheral | Base Address |
|------------|--------------|
| XADC | 0xF800_7100 |
| GPIO | 0xE000_A000 |
| I2C0 | 0xE000_4000 |
| I2C1 | 0xE000_5000 |
| SPI0 | 0xE000_6000 |
| SPI1 | 0xE000_7000 |
| UART0 | 0xE000_0000 |
| UART1 | 0xE000_1000 |
| SLCR | 0xF800_0000 |
| DEVCFG | 0xF800_7000 |

---

## Summary: What to Keep, What to Change

### Keep
- PMBus protocol (standard)
- XADC access method (works well)
- Temperature formulas (calibrated)
- Vector engine interface (AXI)
- Basic test plan structure

### Change
- File-based IPC → shared state
- Shell scripts → Rust modules
- ELF spawning → direct function calls
- Serial hardware init → parallel where possible
- Polling → interrupt-driven where beneficial
- Manual PSU addressing → auto-discovery
