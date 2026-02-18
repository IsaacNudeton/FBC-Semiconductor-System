# Zynq 7020 Register Map for Bare-Metal Firmware

Quick reference for direct hardware access from bare-metal Rust.

---

## Peripheral Base Addresses

| Peripheral | Base Address | Description |
|------------|--------------|-------------|
| UART0 | 0xE000_0000 | Serial console |
| UART1 | 0xE000_1000 | Serial console |
| USB0 | 0xE000_2000 | USB controller |
| USB1 | 0xE000_3000 | USB controller |
| **I2C0** | 0xE000_4000 | PMBus/Pico/Lynx |
| **I2C1** | 0xE000_5000 | PMBus/Pico/Lynx |
| **SPI0** | 0xE000_6000 | External ADC/DAC |
| **SPI1** | 0xE000_7000 | External ADC/DAC |
| CAN0 | 0xE000_8000 | CAN bus |
| CAN1 | 0xE000_9000 | CAN bus |
| **GPIO** | 0xE000_A000 | MIO pins |
| GEM0 | 0xE000_B000 | Gigabit Ethernet |
| GEM1 | 0xE000_C000 | Gigabit Ethernet |
| QSPI | 0xE000_D000 | Quad SPI flash |
| SMCC | 0xE000_E000 | Shared memory controller |
| SLCR | 0xF800_0000 | System Level Control |
| TTC0 | 0xF800_1000 | Triple Timer Counter |
| TTC1 | 0xF800_2000 | Triple Timer Counter |
| DMAC_S | 0xF800_3000 | DMA Secure |
| DMAC_NS | 0xF800_4000 | DMA Non-Secure |
| SWDT | 0xF800_5000 | System Watchdog |
| DDRC | 0xF800_6000 | DDR Controller |
| **DEVCFG** | 0xF800_7000 | Device Config (FPGA load) |
| AXI_HP0 | 0xF800_8000 | High Performance AXI |
| AXI_HP1 | 0xF800_9000 | High Performance AXI |
| AXI_HP2 | 0xF800_A000 | High Performance AXI |
| AXI_HP3 | 0xF800_B000 | High Performance AXI |
| OCM | 0xF800_C000 | On-Chip Memory regs |

---

## I2C Register Offsets (for PMBus)

Base: I2C0=0xE000_4000, I2C1=0xE000_5000

| Offset | Name | Description |
|--------|------|-------------|
| 0x00 | CR | Control Register |
| 0x04 | SR | Status Register |
| 0x08 | ADDR | Address Register |
| 0x0C | DATA | Data Register |
| 0x10 | ISR | Interrupt Status |
| 0x14 | XFER_SIZE | Transfer Size |
| 0x18 | SLV_PAUSE | Slave Monitor Pause |
| 0x1C | TIME_OUT | Time Out |
| 0x20 | IMR | Interrupt Mask |
| 0x24 | IER | Interrupt Enable |
| 0x28 | IDR | Interrupt Disable |

### I2C Control Register (CR) Bits
| Bit | Name | Description |
|-----|------|-------------|
| 0 | MS | Master mode (1=master) |
| 1 | ACKEN | ACK enable |
| 2 | NEA | Addressing mode (0=normal) |
| 3 | HOLD | Hold bus after transfer |
| 4 | SLVMON | Slave monitor mode |
| 5 | CLR_FIFO | Clear FIFO |
| 6 | DIV_A | Divisor A (bits 15:14) |
| 14:15 | DIV_B | Divisor B |

---

## SPI Register Offsets (for ADC/DAC)

Base: SPI0=0xE000_6000, SPI1=0xE000_7000

| Offset | Name | Description |
|--------|------|-------------|
| 0x00 | CR | Config Register |
| 0x04 | ISR | Interrupt Status |
| 0x08 | IER | Interrupt Enable |
| 0x0C | IDR | Interrupt Disable |
| 0x10 | IMR | Interrupt Mask |
| 0x14 | ER | Enable Register |
| 0x18 | DR | Delay Register |
| 0x1C | TXD | TX Data |
| 0x20 | RXD | RX Data |
| 0x24 | SICR | Slave Idle Count |
| 0x28 | TXWR | TX Watermark |
| 0x2C | RXWR | RX Watermark |

---

## GPIO Register Offsets

Base: 0xE000_A000

| Offset | Name | Description |
|--------|------|-------------|
| 0x204 | DIRM_0 | Direction Mode Bank 0 |
| 0x208 | OEN_0 | Output Enable Bank 0 |
| 0x244 | DIRM_1 | Direction Mode Bank 1 |
| 0x248 | OEN_1 | Output Enable Bank 1 |
| 0x040 | DATA_0 | Data Bank 0 |
| 0x044 | DATA_1 | Data Bank 1 |
| 0x048 | DATA_2 | Data Bank 2 |
| 0x04C | DATA_3 | Data Bank 3 |
| 0x000 | MASK_DATA_0_LSW | Masked write Bank 0 low |
| 0x004 | MASK_DATA_0_MSW | Masked write Bank 0 high |
| 0x008 | MASK_DATA_1_LSW | Masked write Bank 1 low |
| 0x00C | MASK_DATA_1_MSW | Masked write Bank 1 high |

### MIO Pin Mapping (from firmware)
| MIO | Function |
|-----|----------|
| 36 | ADC Mux Select |
| 37 | Core 6 Enable |
| 38 | Core 5 Enable |
| 39 | Core 2 Enable |
| 47 | Core 3 Enable |

---

## XADC Register Offsets

XADC is accessed through DEVCFG at 0xF800_7000 or directly through SYSMON at PL addresses.

### XADC via SLCR (recommended for bare-metal)
Base: 0xF800_7100

| Offset | Name | Description |
|--------|------|-------------|
| 0x200 | TEMP | On-chip temperature |
| 0x204 | VCCINT | VCCINT supply |
| 0x208 | VCCAUX | VCCAUX supply |
| 0x20C | VP_VN | VP/VN dedicated analog input |
| 0x210-0x23C | VAUX[0-11] | Auxiliary analog inputs |
| 0x280 | MAX_TEMP | Max temperature |
| 0x284 | MAX_VCCINT | Max VCCINT |
| 0x288 | MAX_VCCAUX | Max VCCAUX |
| 0x290 | MIN_TEMP | Min temperature |
| 0x294 | MIN_VCCINT | Min VCCINT |
| 0x298 | MIN_VCCAUX | Min VCCAUX |

### XADC Temperature Conversion
```rust
fn xadc_to_celsius(raw: u16) -> f32 {
    // 12-bit ADC, full scale = 503.975°C, offset = -273.15°C
    let code = (raw >> 4) as f32;  // 16-bit to 12-bit
    (code / 4096.0) * 503.975 - 273.15
}
```

### XADC Voltage Conversion
```rust
fn xadc_to_volts(raw: u16) -> f32 {
    // Full scale = 3.0V for unipolar
    let code = (raw >> 4) as f32;
    (code / 4096.0) * 3.0
}
```

---

## SLCR (System Level Control)

Base: 0xF800_0000

| Offset | Name | Description |
|--------|------|-------------|
| 0x008 | SLCR_LOCK | Lock SLCR writes (0x767B to lock) |
| 0x004 | SLCR_UNLOCK | Unlock SLCR (0xDF0D to unlock) |
| 0x100 | ARM_CLK_CTRL | ARM PLL control |
| 0x120 | CLK_621_TRUE | 6:2:1 clock mode |
| 0x170 | APER_CLK_CTRL | AMBA peripheral clock control |
| 0x200 | FPGA_RST_CTRL | FPGA software reset |
| 0x700 | MIO_PIN_00 | MIO pin 0 config |
| ... | ... | ... |
| 0x7D0 | MIO_PIN_53 | MIO pin 53 config |
| 0x900 | XADC_CFG | XADC configuration |

### SLCR Unlock Sequence
```rust
const SLCR_UNLOCK_KEY: u32 = 0xDF0D;
const SLCR_LOCK_KEY: u32 = 0x767B;

unsafe fn slcr_unlock() {
    write_volatile(0xF800_0004 as *mut u32, SLCR_UNLOCK_KEY);
}

unsafe fn slcr_lock() {
    write_volatile(0xF800_0008 as *mut u32, SLCR_LOCK_KEY);
}
```

---

## Device Configuration (FPGA Loading)

Base: 0xF800_7000

| Offset | Name | Description |
|--------|------|-------------|
| 0x00 | CTRL | Control register |
| 0x04 | LOCK | Configuration lock |
| 0x08 | CFG | Configuration info |
| 0x0C | INT_STS | Interrupt status |
| 0x10 | INT_MASK | Interrupt mask |
| 0x14 | STATUS | Status register |
| 0x18 | DMA_SRC_ADDR | DMA source address |
| 0x1C | DMA_DST_ADDR | DMA destination address |
| 0x20 | DMA_SRC_LEN | DMA source length |
| 0x24 | DMA_DST_LEN | DMA destination length |

### FPGA Programming via PCAP
```rust
const PCAP_CTRL: *mut u32 = 0xF800_7000 as *mut u32;

unsafe fn program_fpga(bitstream: &[u8]) {
    // 1. Unlock SLCR
    slcr_unlock();

    // 2. Enable PCAP clock
    let aper_clk = read_volatile(0xF800_012C as *const u32);
    write_volatile(0xF800_012C as *mut u32, aper_clk | (1 << 1));

    // 3. Reset FPGA
    write_volatile(0xF800_0240 as *mut u32, 0xF);
    write_volatile(0xF800_0240 as *mut u32, 0x0);

    // 4. Configure DMA transfer
    // ... (detailed sequence in TRM Chapter 6)
}
```

---

## Sonoma Firmware API Summary

From SONOMA_FIRMWARE_API_SPECIFICATIONS.md:

| ELF Binary | Purpose | Key Parameters |
|------------|---------|----------------|
| linux_init_XADC.elf | Init 16 FPGA ADCs | None |
| linux_XADC.elf | Read all 16 XADCs | [samples] |
| linux_EXT_ADC.elf | Read 16 external ADCs | [samples] |
| linux_EXT_DAC.elf | Set 10 DAC outputs | v1..v10 (0-4.096V) |
| linux_EXT_DAC_singleCh.elf | Set 1 DAC | ch, voltage |
| linux_IO_PS.elf | Set VIH levels | vih1 vih2 vih3 vih4 (0.8-3.6V) |
| linux_pin_type.elf | Set GPIO type | pin, type (0-7) |
| linux_Pulse_Delays.elf | Config pulse timing | pin, type, rise_ns, fall_ns, 200 |
| linux_pmbus_OFF.elf | Turn off PSU | addr (0-99) |
| linux_pmbus_PicoDlynx.elf | Turn on PSU | addr, voltage |
| linux_VICOR.elf | Set core supply | voltage, mio, dac |
| linux_set_temperature.elf | Set temp setpoint | temp, R25C |
| linuxLinTempDiode.elf | Set diode temp | temp, ideality, coolafter |
| linux_load_vectors.elf | Load vectors | seq_file, hex_file |
| linux_run_vector.elf | Run vectors | pattern, freq, name, log, adc, time |
| linux_xpll_frequency.elf | Set PLL freq | clk_num, freq_hz, phase, duty |

---

## Rust Bare-Metal Access Pattern

```rust
use core::ptr::{read_volatile, write_volatile};

// Type-safe register access
struct Reg<T> {
    addr: *mut T,
}

impl<T: Copy> Reg<T> {
    const fn new(addr: usize) -> Self {
        Self { addr: addr as *mut T }
    }

    #[inline(always)]
    fn read(&self) -> T {
        unsafe { read_volatile(self.addr) }
    }

    #[inline(always)]
    fn write(&self, val: T) {
        unsafe { write_volatile(self.addr, val) }
    }
}

// I2C registers
mod i2c0 {
    use super::Reg;
    pub const CR: Reg<u32> = Reg::new(0xE000_4000);
    pub const SR: Reg<u32> = Reg::new(0xE000_4004);
    pub const ADDR: Reg<u32> = Reg::new(0xE000_4008);
    pub const DATA: Reg<u32> = Reg::new(0xE000_400C);
    // ...
}

// Example: Read from PMBus device
fn pmbus_read_byte(addr: u8, cmd: u8) -> Result<u8, I2cError> {
    // Set slave address
    i2c0::ADDR.write((addr as u32) & 0x7F);

    // Write command byte
    i2c0::DATA.write(cmd as u32);

    // Wait for TX complete
    while (i2c0::SR.read() & (1 << 0)) == 0 {}

    // Read data byte
    Ok(i2c0::DATA.read() as u8)
}
```
