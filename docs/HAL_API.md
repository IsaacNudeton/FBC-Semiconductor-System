# Firmware HAL API Reference

Hardware Abstraction Layer for Zynq 7020 PS Peripherals.

**Status:** Functional, needs optimization (see bottom)

---

## Design Pattern: ONETWO

All HAL modules follow the ONETWO pattern:

| Aspect | Description |
|--------|-------------|
| **Invariant** | Register addresses, bit fields, protocol sequences |
| **Varies** | Device addresses, data values, thresholds |
| **Pattern** | `Peripheral = base + registers + init + read/write` |

---

## Module: `hal::Slcr`

System Level Control Registers - clocks, resets, MIO configuration.

**Base Address:** `0xF800_0000`

### Constants
```rust
const UNLOCK_KEY: u32 = 0xDF0D;
const LOCK_KEY: u32 = 0x767B;
```

### API
```rust
impl Slcr {
    pub const fn new() -> Self;

    // Lock/Unlock (required before any SLCR write)
    pub fn unlock(&self);
    pub fn lock(&self);
    pub fn with_unlock<F, R>(&self, f: F) -> R;  // RAII pattern

    // Peripheral Clocks
    pub fn enable_peripheral_clock(&self, mask: u32);
    pub fn disable_peripheral_clock(&self, mask: u32);
    pub fn get_peripheral_clocks(&self) -> u32;
    pub fn enable_i2c0(&self);
    pub fn enable_i2c1(&self);
    pub fn enable_spi0(&self);
    pub fn enable_spi1(&self);
    pub fn enable_uart0(&self);
    pub fn enable_uart1(&self);
    pub fn enable_gpio(&self);
    pub fn enable_gem0(&self);

    // Reset Control
    pub fn reset_fpga(&self);
    pub fn get_fpga_reset(&self) -> u32;

    // MIO Pin Configuration
    pub fn configure_mio(&self, pin: u8, config: u32);
    pub fn get_mio_config(&self, pin: u8) -> u32;

    // FPGA Clocks
    pub fn set_fclk0_divisor(&self, divisor0: u8, divisor1: u8);
    pub fn get_reboot_status(&self) -> u32;
}
```

### Usage Example
```rust
let slcr = Slcr::new();
slcr.enable_i2c0();
slcr.enable_gpio();
slcr.with_unlock(|s| {
    s.configure_mio(36, mio::GPIO);  // ADC_MUX_SEL
});
```

---

## Module: `hal::I2c`

I2C Master Driver for PMBus communication.

**Base Addresses:** I2C0=`0xE000_4000`, I2C1=`0xE000_5000`

### Error Types
```rust
pub enum I2cError {
    Nack,              // Device didn't acknowledge
    ArbitrationLost,   // Multi-master collision
    Timeout,           // Operation timed out
    RxOverflow,        // RX FIFO overflow
    TxOverflow,        // TX FIFO overflow
    InvalidAddress,    // Address > 0x7F
}
```

### API
```rust
impl I2c {
    pub const fn i2c0() -> Self;
    pub const fn i2c1() -> Self;

    // Initialization
    pub fn init(&self, speed_khz: u32);  // 100=standard, 400=fast

    // Status
    pub fn is_busy(&self) -> bool;

    // Basic Operations
    pub fn write(&self, addr: u8, data: &[u8]) -> Result<(), I2cError>;
    pub fn read(&self, addr: u8, buf: &mut [u8]) -> Result<(), I2cError>;
    pub fn write_read(&self, addr: u8, write: &[u8], read: &mut [u8]) -> Result<(), I2cError>;

    // Discovery
    pub fn scan(&self) -> [u8; 16];  // Bitmask of responding addresses
}
```

### Usage Example
```rust
let i2c = I2c::i2c0();
i2c.init(400);  // 400 kHz

// Write register
i2c.write(0x50, &[0x00, 0x42])?;

// Read register (write addr, then read)
let mut buf = [0u8; 2];
i2c.write_read(0x50, &[0x00], &mut buf)?;

// Scan bus
let found = i2c.scan();
for addr in 0x08..0x78 {
    if found[(addr/8) as usize] & (1 << (addr % 8)) != 0 {
        // Device found at addr
    }
}
```

---

## Module: `hal::Spi`

SPI Master Driver for ADC/DAC communication.

**Base Addresses:** SPI0=`0xE000_6000`, SPI1=`0xE000_7000`

### Types
```rust
pub enum SpiMode {
    Mode0,  // CPOL=0, CPHA=0
    Mode1,  // CPOL=0, CPHA=1
    Mode2,  // CPOL=1, CPHA=0
    Mode3,  // CPOL=1, CPHA=1
}

pub enum SpiError {
    TxOverflow,
    RxOverflow,
    Timeout,
    ModeFault,
}
```

### API
```rust
impl Spi {
    pub const fn spi0() -> Self;
    pub const fn spi1() -> Self;

    // Initialization
    pub fn init(&self, mode: SpiMode, baud_div: u8);  // div: 0-7 → /4 to /256

    // Chip Select
    pub fn select(&self, cs: u8);    // Assert CS (0-3)
    pub fn deselect(&self);          // Deassert all CS

    // Transfer
    pub fn transfer_byte(&self, tx: u8) -> Result<u8, SpiError>;
    pub fn transfer(&self, tx: &[u8], rx: &mut [u8]) -> Result<(), SpiError>;
    pub fn write(&self, data: &[u8]) -> Result<(), SpiError>;
    pub fn read(&self, buf: &mut [u8]) -> Result<(), SpiError>;

    // Sonoma Helpers
    pub fn read_adc(&self, channel: u8) -> Result<u16, SpiError>;
    pub fn write_dac(&self, channel: u8, value: u16) -> Result<(), SpiError>;
}
```

### Usage Example
```rust
let spi = Spi::spi0();
spi.init(SpiMode::Mode0, 2);  // Mode 0, /16 clock

// Read ADC channel 0
let value = spi.read_adc(0)?;

// Manual transaction
spi.select(0);
let response = spi.transfer_byte(0x80)?;
spi.deselect();
```

---

## Module: `hal::Gpio`

GPIO Driver for MIO pin control.

**Base Address:** `0xE000_A000`

### Types
```rust
pub struct MioPin { pub pin: u8 }

impl MioPin {
    pub const fn new(pin: u8) -> Self;
    pub const fn bank(&self) -> u8;  // 0 for pins 0-31, 1 for 32-53
    pub const fn bit(&self) -> u8;   // Bit within bank
}
```

### Known Pin Assignments (Sonoma)
```rust
pub mod mio_pins {
    pub const ADC_MUX_SEL: u8 = 36;
    pub const CORE6_EN: u8 = 37;
    pub const CORE5_EN: u8 = 38;
    pub const CORE2_EN: u8 = 39;
    pub const CORE3_EN: u8 = 47;
}
```

### API
```rust
impl Gpio {
    pub const fn new() -> Self;

    // Direction
    pub fn set_direction(&self, pin: MioPin, output: bool);
    pub fn set_output(&self, pin: MioPin);
    pub fn set_input(&self, pin: MioPin);

    // Pin I/O
    pub fn write_pin(&self, pin: MioPin, high: bool);
    pub fn set_high(&self, pin: MioPin);
    pub fn set_low(&self, pin: MioPin);
    pub fn toggle(&self, pin: MioPin);
    pub fn read_pin(&self, pin: MioPin) -> bool;

    // Bank I/O
    pub fn write_bank(&self, bank: u8, value: u32);
    pub fn read_bank(&self, bank: u8) -> u32;

    // Sonoma Helpers
    pub fn set_adc_mux(&self, value: bool);
    pub fn set_core_enable(&self, core: u8, enable: bool);  // core: 2,3,5,6
    pub fn enable_all_cores(&self);
    pub fn disable_all_cores(&self);
}
```

### Usage Example
```rust
let gpio = Gpio::new();

// Enable cores 2,3,5,6
gpio.enable_all_cores();

// Toggle ADC mux
gpio.set_adc_mux(true);

// Manual pin control
let led = MioPin::new(7);
gpio.set_output(led);
gpio.toggle(led);
```

---

## Module: `hal::Xadc`

On-chip ADC for temperature and voltage monitoring.

**Base Address:** `0xF800_7100` (PS-XADC interface)

### Types
```rust
pub enum XadcError {
    FifoError,
    Timeout,
    OverTemperature,
}

pub struct SystemStatus {
    pub temperature_mc: i32,   // millidegrees Celsius
    pub vccint_mv: u32,        // millivolts
    pub vccaux_mv: u32,
    pub vccbram_mv: u32,
    pub over_temp: bool,
    pub alarms: u8,
}
```

### API
```rust
impl Xadc {
    pub const fn new() -> Self;
    pub fn init(&self);

    // Temperature
    pub fn read_temperature_raw(&self) -> Result<u16, XadcError>;
    pub fn read_temperature_celsius(&self) -> Result<i32, XadcError>;
    pub fn read_temperature_millicelsius(&self) -> Result<i32, XadcError>;
    pub fn get_max_temperature_raw(&self) -> Result<u16, XadcError>;
    pub fn get_min_temperature_raw(&self) -> Result<u16, XadcError>;

    // Supply Voltages
    pub fn read_vccint_raw(&self) -> Result<u16, XadcError>;
    pub fn read_vccint_mv(&self) -> Result<u32, XadcError>;
    pub fn read_vccaux_raw(&self) -> Result<u16, XadcError>;
    pub fn read_vccaux_mv(&self) -> Result<u32, XadcError>;
    pub fn read_vccbram_raw(&self) -> Result<u16, XadcError>;
    pub fn read_vccbram_mv(&self) -> Result<u32, XadcError>;

    // Auxiliary Channels
    pub fn read_vaux_raw(&self, channel: u8) -> Result<u16, XadcError>;
    pub fn read_vaux_mv(&self, channel: u8) -> Result<u32, XadcError>;

    // Alarms
    pub fn is_over_temperature(&self) -> bool;
    pub fn get_alarm_flags(&self) -> u8;
    pub fn set_temperature_alarms(&self, upper: u16, lower: u16) -> Result<(), XadcError>;
    pub fn set_over_temperature_threshold(&self, threshold: u16) -> Result<(), XadcError>;

    // Full Status
    pub fn get_system_status(&self) -> Result<SystemStatus, XadcError>;
}
```

### Conversion Formulas
```
Temperature: T(°C) = (ADC × 503.975 / 65536) - 273.15
Voltage:     V(V)  = ADC × 3.0 / 65536
```

### Usage Example
```rust
let xadc = Xadc::new();
xadc.init();

let temp_c = xadc.read_temperature_celsius()?;
let vccint = xadc.read_vccint_mv()?;

if xadc.is_over_temperature() {
    // Emergency shutdown
}

let status = xadc.get_system_status()?;
```

---

## Module: `hal::Uart`

UART Driver for serial console.

**Base Addresses:** UART0=`0xE000_0000`, UART1=`0xE000_1000`

### Types
```rust
pub struct UartConfig {
    pub baud_rate: u32,
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBits,
}

pub enum DataBits { Six, Seven, Eight }
pub enum Parity { None, Even, Odd }
pub enum StopBits { One, OnePointFive, Two }

pub enum UartError {
    TxOverflow,
    RxOverflow,
    ParityError,
    FramingError,
    Timeout,
}
```

### API
```rust
impl Uart {
    pub const fn uart0() -> Self;
    pub const fn uart1() -> Self;

    // Initialization
    pub fn init(&self, config: &UartConfig, ref_clk: u32);
    pub fn init_default(&self);  // 115200 8N1

    // Status
    pub fn is_tx_full(&self) -> bool;
    pub fn is_tx_empty(&self) -> bool;
    pub fn is_rx_empty(&self) -> bool;
    pub fn is_rx_available(&self) -> bool;

    // Blocking I/O
    pub fn write_byte(&self, byte: u8);
    pub fn read_byte(&self) -> u8;
    pub fn write_bytes(&self, data: &[u8]);
    pub fn write_str(&self, s: &str);
    pub fn flush(&self);

    // Non-blocking I/O
    pub fn try_write_byte(&self, byte: u8) -> Result<(), UartError>;
    pub fn try_read_byte(&self) -> Option<u8>;
    pub fn read_byte_timeout(&self, timeout_us: u32) -> Result<u8, UartError>;
    pub fn read_bytes(&self, buf: &mut [u8]) -> usize;

    // Error Handling
    pub fn check_errors(&self) -> Result<(), UartError>;

    // Testing
    pub fn enable_loopback(&self);
    pub fn disable_loopback(&self);
}

// Implements core::fmt::Write for use with write! macro
impl Write for Uart { ... }
```

### Macros
```rust
uart_print!("Hello, {}!", name);
uart_println!("Temperature: {} C", temp);
```

### Usage Example
```rust
let uart = Uart::uart0();
uart.init_default();

uart.write_str("Boot complete\r\n");

while let Some(byte) = uart.try_read_byte() {
    uart.write_byte(byte);  // Echo
}
```

---

## Module: `hal::Pcap`

FPGA Programming via Processor Configuration Access Port.

**Base Address:** `0xF800_7000` (DEVCFG)

### Types
```rust
pub enum PcapError {
    InitTimeout,       // FPGA didn't respond to reset
    DmaError,          // DMA transfer failed
    ConfigFailed,      // DONE signal not asserted
    BitstreamTooLarge,
    HmacError,
    SeuError,
}
```

### API
```rust
impl Pcap {
    pub const fn new() -> Self;
    pub fn init(&self);

    // Status
    pub fn is_configured(&self) -> bool;      // DONE signal
    pub fn is_init_high(&self) -> bool;       // INIT signal
    pub fn get_status(&self) -> u32;
    pub fn get_interrupt_status(&self) -> u32;
    pub fn clear_interrupts(&self);

    // Programming
    pub fn reset_fpga(&self) -> Result<(), PcapError>;
    pub fn program(&self, bitstream: *const u8, length: usize) -> Result<(), PcapError>;
    pub fn program_slice(&self, bitstream: &[u8]) -> Result<(), PcapError>;

    // Readback
    pub fn readback(&self, buffer: *mut u8, length: usize) -> Result<(), PcapError>;
}
```

### Programming Sequence
1. `reset_fpga()` - Assert PROG_B, wait for INIT
2. DMA bitstream to PCAP FIFO
3. Wait for DONE signal
4. Release FPGA from reset

### Usage Example
```rust
let pcap = Pcap::new();
pcap.init();

// Load bitstream from memory
let bitstream: &[u8] = include_bytes!("../design.bit");
pcap.program_slice(bitstream)?;

if pcap.is_configured() {
    // FPGA ready
}
```

---

## Module: `hal::PmbusDevice`

PMBus Protocol for power supply communication.

**Transport:** I2C

### Types
```rust
pub enum PmbusError {
    I2c(I2cError),
    Nack,
    PecError,          // CRC mismatch
    InvalidData,
    UnsupportedCommand,
}
```

### Command Codes (subset)
```rust
pub mod cmd {
    pub const OPERATION: u8 = 0x01;
    pub const CLEAR_FAULTS: u8 = 0x03;
    pub const VOUT_COMMAND: u8 = 0x21;
    pub const STATUS_BYTE: u8 = 0x78;
    pub const STATUS_WORD: u8 = 0x79;
    pub const READ_VIN: u8 = 0x88;
    pub const READ_VOUT: u8 = 0x8B;
    pub const READ_IOUT: u8 = 0x8C;
    pub const READ_TEMPERATURE_1: u8 = 0x8D;
    pub const READ_POUT: u8 = 0x96;
    pub const MFR_ID: u8 = 0x99;
    pub const MFR_MODEL: u8 = 0x9A;
    // ... 50+ more commands
}
```

### API
```rust
impl<'a> PmbusDevice<'a> {
    pub fn new(i2c: &'a I2c, address: u8) -> Self;

    // PEC (CRC) Control
    pub fn enable_pec(&mut self);
    pub fn disable_pec(&mut self);

    // Low-level
    pub fn send_byte(&self, cmd: u8) -> Result<(), PmbusError>;
    pub fn write_byte(&self, cmd: u8, data: u8) -> Result<(), PmbusError>;
    pub fn write_word(&self, cmd: u8, data: u16) -> Result<(), PmbusError>;
    pub fn read_byte(&self, cmd: u8) -> Result<u8, PmbusError>;
    pub fn read_word(&self, cmd: u8) -> Result<u16, PmbusError>;
    pub fn read_block(&self, cmd: u8, buf: &mut [u8]) -> Result<usize, PmbusError>;

    // Control
    pub fn clear_faults(&self) -> Result<(), PmbusError>;
    pub fn enable_output(&self) -> Result<(), PmbusError>;
    pub fn disable_output(&self) -> Result<(), PmbusError>;
    pub fn disable_output_immediate(&self) -> Result<(), PmbusError>;
    pub fn set_margin_high(&self) -> Result<(), PmbusError>;
    pub fn set_margin_low(&self) -> Result<(), PmbusError>;

    // Status
    pub fn read_status(&self) -> Result<u8, PmbusError>;
    pub fn read_status_word(&self) -> Result<u16, PmbusError>;
    pub fn is_output_on(&self) -> Result<bool, PmbusError>;
    pub fn has_fault(&self) -> Result<bool, PmbusError>;

    // Voltage (millivolts)
    pub fn read_vout_raw(&self) -> Result<u16, PmbusError>;
    pub fn read_vout_mv(&self) -> Result<u32, PmbusError>;
    pub fn read_vin_mv(&self) -> Result<i32, PmbusError>;
    pub fn set_vout_raw(&self, value: u16) -> Result<(), PmbusError>;
    pub fn set_vout_mv(&self, mv: u32) -> Result<(), PmbusError>;

    // Current (milliamps)
    pub fn read_iout_ma(&self) -> Result<i32, PmbusError>;
    pub fn read_iin_ma(&self) -> Result<i32, PmbusError>;

    // Power (milliwatts)
    pub fn read_pout_mw(&self) -> Result<i32, PmbusError>;
    pub fn read_pin_mw(&self) -> Result<i32, PmbusError>;

    // Temperature (millidegrees Celsius)
    pub fn read_temperature_1_mc(&self) -> Result<i32, PmbusError>;

    // Identification
    pub fn read_revision(&self) -> Result<u8, PmbusError>;
    pub fn read_mfr_id(&self, buf: &mut [u8]) -> Result<usize, PmbusError>;
    pub fn read_model(&self, buf: &mut [u8]) -> Result<usize, PmbusError>;
}

// Bus scan
pub fn scan_pmbus_devices(i2c: &I2c) -> [Option<u8>; 16];
```

### Data Formats
```
LINEAR11: [15:11]=exponent (signed), [10:0]=mantissa (signed)
          Value = mantissa × 2^exponent

LINEAR16: 16-bit unsigned with implicit exponent from VOUT_MODE
          Typically exponent = -12, so Value = raw / 4096
```

### Usage Example
```rust
let i2c = I2c::i2c0();
i2c.init(400);

// Find all PMBus devices
let devices = scan_pmbus_devices(&i2c);

// Control a specific device
let psu = PmbusDevice::new(&i2c, 0x58);

if psu.has_fault()? {
    psu.clear_faults()?;
}

psu.enable_output()?;
let vout = psu.read_vout_mv()?;
let iout = psu.read_iout_ma()?;
let temp = psu.read_temperature_1_mc()?;
```

---

## Module: `hal::Eeprom`

24LC02 I2C EEPROM driver for BIM configuration storage.

**Device:** Microchip 24LC02BHT-I/LT
**Capacity:** 256 bytes (2 Kbit)
**Page Size:** 8 bytes
**Interface:** I2C, 400 kHz
**Default Address:** `0x50`

### EEPROM Layout
```
0x00-0x0F:  Header (16 bytes)
0x10-0x4F:  Power rail config (64 bytes)
0x50-0x8F:  Calibration data (64 bytes)
0x90-0xEF:  DUT metadata (96 bytes)
0xF0-0xF7:  Statistics (8 bytes)
0xF8-0xFB:  CRC32 checksum (4 bytes)
0xFC-0xFF:  Reserved (4 bytes)
```

### API
```rust
pub const EEPROM_SIZE: usize = 256;
pub const EEPROM_ADDR: u8 = 0x50;

pub enum EepromError {
    I2c(I2cError),
    InvalidAddress,
    VerifyFailed,
    ChecksumMismatch,
    InvalidMagic,
}

pub struct BimEeprom<'a> {
    i2c: &'a I2c,
    addr: u8,
}

impl<'a> BimEeprom<'a> {
    pub fn new(i2c: &'a I2c, addr: u8) -> Self;

    // Single-byte operations
    pub fn read_byte(&self, addr: u8) -> Result<u8, EepromError>;
    pub fn write_byte(&self, addr: u8, data: u8) -> Result<(), EepromError>;

    // Bulk operations
    pub fn read(&self, addr: u8, buf: &mut [u8]) -> Result<(), EepromError>;
    pub fn write(&self, addr: u8, data: &[u8]) -> Result<(), EepromError>;

    // Page-aligned write (faster)
    pub fn write_page(&self, page: u8, data: &[u8; 8]) -> Result<(), EepromError>;

    // Checksum validation
    pub fn verify_checksum(&self) -> Result<bool, EepromError>;
    pub fn update_checksum(&self) -> Result<(), EepromError>;
}
```

### Usage Example
```rust
let i2c = I2c::i2c0();
i2c.init(400);

let eeprom = BimEeprom::new(&i2c, EEPROM_ADDR);

// Read configuration
let mut config = [0u8; 64];
eeprom.read(0x10, &mut config)?;

// Verify data integrity
if !eeprom.verify_checksum()? {
    // EEPROM corrupted or not programmed
}

// Write calibration data
let cal_data = [0x12, 0x34, 0x56, 0x78];
eeprom.write(0x50, &cal_data)?;
eeprom.update_checksum()?;
```

---

## Module: `hal::Thermal`

ONETWO Crystallization-based temperature controller.

**Design:** Pattern-aware feedforward with structurally-forced settling rate.
**No PID tuning required** - constants derived from structure, not guesswork.

### Constants (Structurally Forced)
```rust
SETTLE = e - 2 ≈ 0.71828     // Settling rate per iteration (not tunable)
LOCK_ITERATIONS = 7          // Iterations to crystallize
FLOOR_PCT = 10               // 10% residual jitter (physical limit)
```

### API
```rust
pub enum PowerLevel {
    Low,       // Low toggle rate
    Medium,    // Medium activity
    High,      // High toggle rate
}

impl PowerLevel {
    // Feedforward compensation multiplier
    pub fn feedforward(&self) -> i32;  // 0, -100, or -300
}

pub struct ThermalController {
    target_mc: i32,         // Target temperature (millidegrees C)
    current_mc: i32,        // Current temperature
    error_accum: i32,       // Accumulated error
    iterations: u8,         // Crystallization counter
    locked: bool,           // Temperature stable
}

impl ThermalController {
    pub fn new(target_mc: i32) -> Self;

    // Update with current temperature reading
    pub fn update(&mut self, temp_mc: i32, power: PowerLevel) -> i32;

    // Status
    pub fn is_locked(&self) -> bool;
    pub fn get_error_mc(&self) -> i32;
    pub fn get_iterations(&self) -> u8;

    // Configuration
    pub fn set_target(&mut self, target_mc: i32);
    pub fn reset(&mut self);
}
```

### How ONETWO Thermal Works

**Invariant:** Thermal settling follows e^(-t/τ) decay
**Variation:** Different power levels, different thermal loads
**Pattern:** Feedforward based on predicted power

**Crystallization Algorithm:**
```
1. Measure temperature error
2. Apply settling rate: correction = error × SETTLE
3. Add feedforward based on power level
4. Count iterations below threshold
5. Lock when iterations >= LOCK_ITERATIONS
```

**Why it works without PID:**
- Settling rate (e-2) is structurally optimal for 1st-order thermal systems
- Feedforward compensates for known disturbances (vector power)
- No Kp/Ki/Kd tuning - constants forced by physics

### Usage Example
```rust
let xadc = Xadc::new();
let mut thermal = ThermalController::new(45_000);  // 45°C target

loop {
    let temp_mc = xadc.read_temp_millidegree();
    let power = analyze_vector_power();  // PowerLevel::Low/Med/High

    let pwm_duty = thermal.update(temp_mc, power);
    set_fan_pwm(pwm_duty);

    if thermal.is_locked() {
        // Temperature stable within 10%
    }

    delay_ms(100);
}
```

---

## Module: `hal::Gem` / `net`

Zynq Gigabit Ethernet MAC (GEM) driver for raw Ethernet frames.

**Base Address:** `0xE000_B000` (GEM0)
**Speed:** 1 Gbps
**Protocol:** Raw Ethernet + FBC Protocol (EtherType 0x88B5)
**No TCP/IP overhead**

### API
```rust
pub struct Gem {
    base: usize,
}

impl Gem {
    pub fn new() -> Self;

    // Initialization
    pub fn init(&self);
    pub fn set_mac(&self, mac: [u8; 6]);
    pub fn enable_rx(&self);
    pub fn enable_tx(&self);

    // Packet transmission
    pub fn send_raw(&self, dst_mac: [u8; 6], ethertype: u16, data: &[u8]);
    pub fn send_fbc(&self, dst_mac: [u8; 6], data: &[u8]);

    // Packet reception
    pub fn recv(&self, buf: &mut [u8]) -> Option<usize>;
    pub fn poll(&self) -> bool;  // Check if packet available

    // Status
    pub fn link_up(&self) -> bool;
    pub fn get_speed(&self) -> LinkSpeed;
}

pub enum LinkSpeed {
    Speed10M,
    Speed100M,
    Speed1G,
}

// Helper functions
pub fn mac_from_dna(dna: u64) -> [u8; 6];     // Generate MAC from device DNA
pub fn ip_from_dna(dna: u64) -> [u8; 4];      // Generate IP from device DNA
```

### Buffer Descriptors
```rust
/// RX buffer descriptor (8 bytes)
#[repr(C, packed)]
pub struct RxBd {
    pub addr: u32,      // Buffer address
    pub status: u32,    // Status and length
}

/// TX buffer descriptor (8 bytes)
#[repr(C, packed)]
pub struct TxBd {
    pub addr: u32,      // Buffer address
    pub status: u32,    // Status and length
}
```

### Usage Example
```rust
use hal::{Dna, Gem, mac_from_dna};

// Get unique MAC address from device DNA
let dna = Dna::read();
let mac = mac_from_dna(dna);

// Initialize Ethernet
let gem = Gem::new();
gem.init();
gem.set_mac(mac);
gem.enable_rx();
gem.enable_tx();

// Send raw Ethernet frame
let dst_mac = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];  // Broadcast
gem.send_fbc(dst_mac, b"HELLO");

// Receive frames
let mut buf = [0u8; 1500];
if let Some(len) = gem.recv(&mut buf) {
    // Process received frame
}
```

---

## Module: `fbc_protocol`

FBC Protocol - Raw Ethernet frame format for burn-in control.

**EtherType:** `0x88B5` (custom FBC protocol)
**Magic:** `0xFBC0` (validates FBC packets)
**Max Payload:** 1478 bytes (Ethernet MTU 1500 - 14 Ethernet - 8 FBC header)

### Design Principles
- **GUI has full control** - no autonomous behavior
- **Controllers wait for commands** - reactive, not proactive
- **Real-time monitoring** - telemetry via heartbeat
- **Takeover capability** - GUI can interrupt at any time

### FBC Header (8 bytes)
```rust
#[repr(C, packed)]
pub struct FbcHeader {
    pub magic: u16,    // 0xFBC0
    pub seq: u16,      // Sequence number
    pub cmd: u8,       // Command code
    pub flags: u8,     // Reserved
    pub length: u16,   // Payload length
}
```

### Commands

**Setup Phase:**
```rust
pub mod setup {
    pub const ANNOUNCE:         u8 = 0x01;  // Controller → GUI (on boot)
    pub const BIM_STATUS_REQ:   u8 = 0x10;  // GUI → Controller
    pub const BIM_STATUS_RSP:   u8 = 0x11;  // Controller → GUI
    pub const WRITE_BIM:        u8 = 0x20;  // GUI → Controller
    pub const UPLOAD_VECTORS:   u8 = 0x21;  // GUI → Controller (chunked)
    pub const CONFIGURE:        u8 = 0x30;  // GUI → Controller
}
```

**Runtime Commands:**
```rust
pub mod runtime {
    pub const START:            u8 = 0x40;  // GUI → Controller
    pub const STOP:             u8 = 0x41;  // GUI → Controller
    pub const RESET:            u8 = 0x42;  // GUI → Controller
    pub const HEARTBEAT:        u8 = 0x50;  // Controller → GUI
    pub const ERROR:            u8 = 0xE0;  // Controller → GUI
    pub const STATUS_REQ:       u8 = 0xF0;  // GUI → Controller
    pub const STATUS_RSP:       u8 = 0xF1;  // Controller → GUI
}
```

### API
```rust
impl FbcHeader {
    pub fn new(cmd: u8, seq: u16, payload_len: u16) -> Self;
    pub fn to_bytes(&self) -> [u8; 8];
    pub fn from_bytes(data: &[u8]) -> Option<Self>;
    pub fn validate(&self) -> bool;  // Check magic
}

// Packet construction
pub fn build_announce(mac: [u8; 6], seq: u16) -> Vec<u8>;
pub fn build_status_rsp(seq: u16, status: &Status) -> Vec<u8>;
pub fn build_heartbeat(seq: u16, cycle_count: u64) -> Vec<u8>;

// Packet parsing
pub fn parse_fbc(frame: &[u8]) -> Option<(FbcHeader, &[u8])>;
```

### Frame Format
```
┌────────────────────────────────────────────────────────────┐
│ Ethernet Header (14 bytes)                                 │
│  - Destination MAC (6 bytes)                               │
│  - Source MAC (6 bytes)                                    │
│  - EtherType: 0x88B5 (2 bytes)                             │
├────────────────────────────────────────────────────────────┤
│ FBC Header (8 bytes)                                       │
│  - Magic: 0xFBC0 (2 bytes)                                 │
│  - Sequence: u16 (2 bytes)                                 │
│  - Command: u8 (1 byte)                                    │
│  - Flags: u8 (1 byte)                                      │
│  - Length: u16 (2 bytes)                                   │
├────────────────────────────────────────────────────────────┤
│ Payload (0-1478 bytes)                                     │
│  - Command-specific data                                   │
└────────────────────────────────────────────────────────────┘
```

### Usage Example
```rust
use fbc_protocol::{FbcHeader, setup, runtime, ETHERTYPE_FBC};

// Send ANNOUNCE on boot
let mac = mac_from_dna(dna);
let header = FbcHeader::new(setup::ANNOUNCE, 0, 6);
let mut packet = header.to_bytes().to_vec();
packet.extend_from_slice(&mac);  // Payload: MAC address

gem.send_fbc(BROADCAST_MAC, &packet);

// Receive and parse FBC commands
let mut buf = [0u8; 1500];
if let Some(len) = gem.recv(&mut buf) {
    if let Some((header, payload)) = parse_fbc(&buf[..len]) {
        match header.cmd {
            runtime::START => { /* Start vector execution */ },
            runtime::STOP => { /* Stop execution */ },
            runtime::STATUS_REQ => { /* Send status response */ },
            _ => { /* Unknown command */ }
        }
    }
}
```

---

## FPGA Register Interface (`regs`)

Memory-mapped register access for FBC FPGA peripherals.

**Note:** This is separate from the HAL (which controls Zynq PS peripherals). The FPGA registers control the programmable logic that connects to the DUT.

**Base Addresses:**
- `FBC_CTRL_BASE`: `0x4004_0000` - FBC decoder control
- `PIN_CTRL_BASE`: `0x4005_0000` - Pin configuration
- `STATUS_BASE`: `0x4006_0000` - Vector status/errors
- `FREQ_COUNTER_BASE`: `0x4007_0000` - Frequency counters

---

### Module: `regs::FbcCtrl`

FBC decoder control interface (FPGA-based instruction decoder).

**Base Address:** `0x4004_0000`

### API
```rust
pub struct FbcCtrl {
    base: usize,
}

impl FbcCtrl {
    pub const fn new() -> Self;

    // Control
    pub fn enable(&self);                 // Enable FBC decoder
    pub fn disable(&self);                // Disable FBC decoder
    pub fn reset(&self);                  // Reset decoder to idle

    // Status
    pub fn is_running(&self) -> bool;     // Check if executing
    pub fn is_done(&self) -> bool;        // Check if finished
    pub fn has_error(&self) -> bool;      // Check if error occurred

    // Counters
    pub fn get_instr_count(&self) -> u32; // Instructions executed
    pub fn get_cycle_count(&self) -> u64; // Vector cycles generated
    pub fn get_version(&self) -> u32;     // FPGA version
}
```

### Register Map
```
Offset  Name           Description
0x00    CTRL           Control register (bit 0: enable, bit 1: reset)
0x04    STATUS         Status register (bit 0: running, bit 1: done, bit 2: error)
0x08    INSTR_COUNT    Instruction counter (32-bit)
0x10    CYCLE_COUNT_LO Cycle counter low (32-bit)
0x14    CYCLE_COUNT_HI Cycle counter high (32-bit)
0x1C    VERSION        FPGA version
```

### Usage Example
```rust
use regs::FbcCtrl;

let fbc = FbcCtrl::new();

// Start execution
fbc.reset();
fbc.enable();

// Poll for completion
while !fbc.is_done() {
    // Wait...
}

if fbc.has_error() {
    // Handle error
}

// Get statistics
let instructions = fbc.get_instr_count();
let cycles = fbc.get_cycle_count();
```

---

### Module: `regs::PinCtrl`

FPGA Pin Control for DUT Interface (160 pins).

**Base Address:** `0x4005_0000`

### Pin Architecture
- **gpio[0:127]**: BIM pins (QSH direct to BIM, 2-cycle latency)
- **gpio[128:159]**: Fast pins (direct FPGA, 1-cycle latency)

> Note: Quad Board handles power only. GPIO signals go directly Controller → BIM via QSH.

### Constants
```rust
pub const BIM_PIN_COUNT: u8 = 128;
pub const FAST_PIN_COUNT: u8 = 32;
pub const TOTAL_PIN_COUNT: u8 = 160;
```

### Pin Types
```rust
pub enum PinType {
    Bidi = 0,          // Bidirectional (drive or compare)
    Input = 1,         // Input only (compare)
    Output = 2,        // Output only (drive)
    OpenCollector = 3, // Open collector output
    Pulse = 4,         // Pulse (edge at T/4, 3T/4)
    NPulse = 5,        // Inverted pulse
    ErrorTrig = 6,     // Error trigger output
    VecClk = 7,        // Vector clock output
    VecClkEn = 8,      // Clock enable output
}
```

### API
```rust
pub struct PinCtrl {
    base: usize,
}

impl PinCtrl {
    pub const fn new() -> Self;

    // Pin type configuration
    pub fn set_pin_type(&self, pin: u8, pin_type: PinType);
    pub fn get_pin_type(&self, pin: u8) -> PinType;

    // Pulse timing configuration
    pub fn set_pulse_timing(&self, pin: u8, start: u8, end: u8);

    // Pin classification
    pub fn is_fast_pin(pin: u8) -> bool;  // Pin 128-159
    pub fn is_bim_pin(pin: u8) -> bool;   // Pin 0-127
}
```

### Register Map
```
Offset       Name           Description
0x000-0x04C  PIN_TYPE       Pin type configuration (20 registers, 4 bits/pin)
0x200-0x33C  PULSE_CTRL     Pulse timing (80 registers, 16 bits/pin)
```

### Usage Example
```rust
use regs::{PinCtrl, PinType, TOTAL_PIN_COUNT};

let pin_ctrl = PinCtrl::new();

// Configure pin 0 as output
pin_ctrl.set_pin_type(0, PinType::Output);

// Configure pin 64 as pulse output
pin_ctrl.set_pin_type(64, PinType::Pulse);
pin_ctrl.set_pulse_timing(64, 25, 75);  // Edges at 25% and 75% of cycle

// Check if pin 150 is a fast pin
if PinCtrl::is_fast_pin(150) {
    // 1-cycle latency
}

// Configure all pins as bidirectional (default)
for pin in 0..TOTAL_PIN_COUNT {
    pin_ctrl.set_pin_type(pin, PinType::Bidi);
}
```

---

### Module: `regs::VectorStatus`

Vector execution status and error reporting.

**Base Address:** `0x4006_0000`

### API
```rust
pub struct VectorStatus {
    base: usize,
}

impl VectorStatus {
    pub const fn new() -> Self;

    // Error tracking
    pub fn get_error_count(&self) -> u32;     // Total errors detected
    pub fn has_errors(&self) -> bool;         // Error flag

    // Execution tracking
    pub fn get_vector_count(&self) -> u32;    // Vectors executed
    pub fn get_cycle_count(&self) -> u64;     // Cycles executed
    pub fn is_done(&self) -> bool;            // Execution complete

    // Version
    pub fn get_version(&self) -> u32;         // FPGA version
}
```

### Register Map
```
Offset  Name           Description
0x00    ERROR_COUNT    Error counter (32-bit)
0x04    VECTOR_COUNT   Vector counter (32-bit)
0x08    CYCLE_COUNT_LO Cycle counter low (32-bit)
0x0C    CYCLE_COUNT_HI Cycle counter high (32-bit)
0x14    STATUS         Status flags (bit 0: done, bit 1: has_errors)
0x3C    VERSION        FPGA version
```

### Usage Example
```rust
use regs::VectorStatus;

let status = VectorStatus::new();

// Check for errors during execution
if status.has_errors() {
    let error_count = status.get_error_count();
    println!("Errors detected: {}", error_count);
}

// Get execution statistics
let vectors = status.get_vector_count();
let cycles = status.get_cycle_count();
println!("Executed {} vectors in {} cycles", vectors, cycles);

// Check FPGA version
let version = status.get_version();
println!("FPGA version: 0x{:08X}", version);
```

---

### Module: `regs::FreqCounter`

Frequency counter for vector clock monitoring.

**Base Address:** `0x4007_0000` (+ `0x20` per counter index)

### API
```rust
pub struct FreqCounter {
    base: usize,
}

impl FreqCounter {
    pub const fn new(index: usize) -> Self;  // index = counter instance

    pub fn enable(&self);                     // Start counting
    pub fn disable(&self);                    // Stop counting
    pub fn get_count(&self) -> u32;           // Read counter
    pub fn reset(&self);                      // Reset to zero
}
```

### Usage Example
```rust
use regs::FreqCounter;

// Monitor vec_clk frequency
let freq_counter = FreqCounter::new(0);
freq_counter.reset();
freq_counter.enable();

// Wait 1 second...
delay_ms(1000);

let count = freq_counter.get_count();
println!("Vector clock frequency: {} Hz", count);
```

---

## Common Register Trait

All HAL modules use this trait for register access:

```rust
pub trait Register {
    fn read(&self) -> u32;
    fn write(&self, val: u32);
    fn modify<F: FnOnce(u32) -> u32>(&self, f: F);
    fn set_bits(&self, mask: u32);
    fn clear_bits(&self, mask: u32);
}

pub struct Reg(usize);

impl Reg {
    pub const fn new(addr: usize) -> Self;
    pub const fn offset(&self, off: usize) -> Self;
}
```

---

## Utility Functions

```rust
pub fn delay_us(us: u32);   // Busy-wait microseconds
pub fn delay_ms(ms: u32);   // Busy-wait milliseconds
```

---

## What's Missing / Needs Optimization

### Current Limitations

| Area | Current | Status |
|------|---------|--------|
| Temperature | ONETWO Crystallization | ✅ Implemented (hal::Thermal) |
| EEPROM | 24LC02 Driver | ✅ Implemented (hal::Eeprom) |
| Network | Raw Ethernet GEM | ✅ Implemented (hal::Gem / net) |
| Protocol | FBC Raw Ethernet | ✅ Implemented (fbc_protocol) |
| I2C/SPI | Polling | ⏳ Interrupt/DMA optimization |
| UART | Blocking TX | ⏳ Ring buffer + DMA |
| PMBus | Manual scan | ⏳ Auto-discovery with type ID |
| Delays | Busy-wait | ⏳ Timer-based |
| Errors | Basic | ⏳ Retry with backoff |

### Needed Optimizations

1. **ONETWO Temperature Controller** ✅ DONE
   - Crystallization-based settling (e-2 rate)
   - Pattern-aware feedforward (power prediction)
   - No PID tuning required
   - 7 iterations to lock, 10% jitter floor

2. **DMA Transfers**
   - UART TX/RX via DMA
   - SPI bulk transfers
   - Zero-copy where possible

3. **Interrupt-Driven I/O**
   - I2C completion interrupts
   - UART RX interrupts
   - Timer interrupts for periodic tasks

4. **PMBus Device Discovery**
   - Read MFR_ID on scan
   - Type detection (Pico, Lynx, MPS, Infineon)
   - Virtual addressing layer

5. **Error Handling**
   - Retry with exponential backoff
   - Fault logging
   - Recovery strategies

6. **Power Management**
   - Sleep modes
   - Clock gating
   - Peripheral power-down

See `TODO.md` Phase 4.4-4.7 for implementation plan.
