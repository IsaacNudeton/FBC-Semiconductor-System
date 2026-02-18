# Firmware API for GUI

**What's implemented and ready for GUI integration**

---

## Quick Reference

| Capability | Module | Status |
|------------|--------|--------|
| **Analog Monitor (32ch)** | `analog.rs` | ✅ Ready |
| **External ADC (16ch)** | `max11131.rs` | ✅ Ready |
| **External DAC (10ch)** | `bu2505.rs` | ✅ Ready |
| **VICOR Cores (6)** | `vicor.rs` | ✅ Ready |
| **PMBus Power Supplies** | `pmbus.rs` | ✅ Ready |
| **XADC (16ch internal)** | `xadc.rs` | ✅ Ready |
| **Thermal Control** | `thermal.rs` | ✅ Ready |
| **EEPROM (256 bytes)** | `eeprom.rs` | ✅ Ready |
| **FPGA Programming** | `pcap.rs` | ✅ Ready |
| **GPIO Control** | `gpio.rs` | ✅ Ready |
| **SPI Bus** | `spi.rs` | ✅ Ready |
| **I2C Bus** | `i2c.rs` | ✅ Ready |
| **UART Console** | `uart.rs` | ✅ Ready |
| **Device DNA (Unique ID)** | `dna.rs` | ✅ Ready |
| **Ethernet/TCP** | `net.rs` | ✅ Ready |
| **FBC Vector Engine** | `fbc.rs`, `dma.rs` | ✅ Ready |

---

## 1. Analog Monitor (32 Channels)

**What:** Unified interface to all ADC channels (XADC + MAX11131)

**GUI Actions:**
- Read all 32 channels
- Read single channel by number or name
- Get channel names for dropdowns/labels
- Auto-refresh at configurable interval

**Data per channel:**
```
channel: u8        // 0-31
name: &str         // "VDD_CORE1", "DIE_TEMP", etc.
value: f32         // Converted value (mV, °C, mA)
unit: &str         // "mV", "°C", "mA"
raw: u16           // Raw ADC value (for debugging)
```

**Channel Map:**
| Ch | Name | Type | Unit |
|----|------|------|------|
| 0 | DIE_TEMP | XADC | °C |
| 1 | VCCINT | XADC | mV |
| 2 | VCCAUX | XADC | mV |
| 3 | VCCBRAM | XADC | mV |
| 4-15 | XADC_AUX0-11 | XADC | mV |
| 16-21 | VDD_CORE1-6 | MAX11131 | mV |
| 22 | THERM_CASE | MAX11131 | °C |
| 23 | THERM_DUT | MAX11131 | °C |
| 24-25 | I_CORE1-2 | MAX11131 | mA |
| 26-31 | VDD_IO, VDD_3V3, etc. | MAX11131 | mV |

---

## 2. VICOR Core Supplies (6 Cores)

**What:** Control 6 high-current core voltage supplies

**GUI Actions:**
- Set voltage per core (500-1500 mV)
- Enable/disable individual cores
- Power-on sequence (all cores with safe timing)
- Power-off sequence
- Emergency stop (disable all immediately)
- Read status (enabled, voltage)

**Core Mapping (fixed by hardware):**
| Core | Voltage Range | Current |
|------|---------------|---------|
| 1-6 | 0.5V - 1.5V | 10-50A each |

**Safety:**
- Voltage clamped to 500-1500 mV
- Power-on sequence has delays between enables
- Emergency stop disables all instantly

---

## 3. PMBus Power Supplies

**What:** Control all PMBus power supplies (LC, HC, Pico, Lynx, MPS)

**GUI Actions:**
- Set voltage (VOUT_COMMAND)
- Read voltage (READ_VOUT)
- Read current (READ_IOUT)
- Enable/disable output
- Read status/faults
- Scan I2C bus for devices

**Supported Devices:**
- Up to 99 supplies per BIM
- Typical: 16 LC (12A) + 4 HC (40A)
- Auto-detect device type via MFR_ID

---

## 4. Thermal Control

**What:** Temperature control with heater/fan

**GUI Actions:**
- Set target temperature
- Read current temperature
- Read heater/fan output levels
- Enable/disable thermal control

**Features:**
- ONETWO-based (no PID tuning needed)
- Settles in 7 iterations
- Feedforward from vector power prediction

---

## 5. EEPROM (BIM Configuration)

**What:** 256 bytes persistent storage per BIM

**GUI Actions:**
- Read BIM configuration
- Write BIM configuration
- Store calibration data
- Store power supply mappings

**Layout:**
```
Offset  Size  Content
0x00    4     Magic (0xFBC_CFG1)
0x04    4     CRC32
0x08    1     BIM Type
0x09    1     BIM Revision
0x0A    6     MAC Address
0x10    16    Serial Number
0x20    32    Rail configurations
0x40    64    Calibration data
0x80    128   Reserved
```

**Features:**
- CRC32 validation
- Survives power cycle
- Per-BIM unique config

---

## 6. FPGA Programming (PCAP)

**What:** Load bitstreams into FPGA fabric

**GUI Actions:**
- Upload bitstream file
- Program FPGA
- Verify programming
- Read FPGA status

**Flow:**
1. GUI sends bitstream over TCP
2. Firmware writes to PCAP
3. PCAP programs PL (fabric)
4. Returns success/fail

---

## 7. Device DNA (Unique ID)

**What:** Read Zynq's unique 57-bit device identifier

**GUI Actions:**
- Read device DNA
- Generate MAC address from DNA
- Generate serial number from DNA

**Use Cases:**
- Unique board identification
- License validation
- Asset tracking

---

## 8. GPIO Control

**What:** Control MIO pins directly

**GUI Actions:**
- Set pin direction (input/output)
- Write pin value
- Read pin value
- Configure for special functions

**Pins Available:**
- MIO 0-53 (directly controllable)
- EMIO 0-53 (through PL)

---

## 9. FBC Vector Engine

**What:** Run test vectors on DUT

**GUI Actions:**
- Load vector file
- Start/stop execution
- Read error counts
- Configure pin types
- Set vector clock frequency

**Pin Types:**
- BIDI (bidirectional)
- INPUT (compare)
- OUTPUT (drive)
- PULSE (edge timing)
- CLOCK (vector clock output)

---

## 10. External DAC (10 Channels)

**What:** Set analog voltage references

**GUI Actions:**
- Set channel voltage (0-4096 mV)
- Set raw value (0-1023)
- Batch update all channels

**Channels:**
- 0-9: General purpose
- 2,3,4,7,8,9: Used by VICOR cores

---

## 11. External ADC (16 Channels)

**What:** Read external analog signals

**GUI Actions:**
- Read all 16 channels (batch)
- Read single channel
- Configure scan mode

**Features:**
- 12-bit resolution
- 3 Msps max
- Batch read in ~6μs

---

## Network Protocol

**Transport:** TCP over Ethernet

**Commands:** Binary protocol (see `protocol.rs`)

```
Request:  [CMD:1][LEN:2][PAYLOAD:N]
Response: [STATUS:1][LEN:2][PAYLOAD:N]
```

**Command Categories:**
- System: ping, reset, status
- Power: set voltage, enable, read
- Analog: read channels
- FPGA: program, verify
- Vector: load, run, stop
- Config: read/write EEPROM

---

## GUI Implementation Notes

**Connection:**
1. TCP connect to board IP:port
2. Send commands, receive responses
3. Poll for status updates

**Recommended Refresh Rates:**
- Analog readings: 1-10 Hz
- Power status: 1 Hz
- Temperature: 0.5 Hz
- Vector status: 10 Hz (during run)

**Error Handling:**
- All commands return status code
- Timeout on no response (5s)
- Reconnect on connection loss

---

## What GUI Needs to Implement

1. **Device Discovery** - Scan network for FBC boards
2. **Connection Manager** - TCP connection handling
3. **Command Encoder** - Build binary protocol messages
4. **Response Decoder** - Parse binary responses
5. **State Manager** - Track board state
6. **UI Components** - Buttons, sliders, displays
7. **File Handling** - Bitstream upload, vector files
8. **Logging** - Command/response history

---

## Files Summary

```
firmware/src/
├── hal/
│   ├── bu2505.rs      # External DAC (10ch)
│   ├── max11131.rs    # External ADC (16ch)
│   ├── vicor.rs       # VICOR controller (6 cores)
│   ├── pmbus.rs       # PMBus power supplies
│   ├── xadc.rs        # Internal ADC (16ch)
│   ├── thermal.rs     # Temperature control
│   ├── eeprom.rs      # Persistent storage
│   ├── pcap.rs        # FPGA programming
│   ├── gpio.rs        # Pin control
│   ├── spi.rs         # SPI bus
│   ├── i2c.rs         # I2C bus
│   ├── uart.rs        # Serial console
│   └── dna.rs         # Unique device ID
├── analog.rs          # 32ch unified monitor
├── net.rs             # Ethernet/TCP
├── protocol.rs        # Command protocol
├── fbc.rs             # Vector encoding
└── dma.rs             # DMA for vectors
```

---

*Last updated: 2026-01-26*
