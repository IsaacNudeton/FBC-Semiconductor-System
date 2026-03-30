//! Hardware Abstraction Layer for Zynq 7020
//!
//! Zero-cost abstractions for PS peripherals.
//!
//! # ONETWO Design
//!
//! Invariant: Register addresses, bit fields, protocol sequences
//! Varies: Device addresses, data, error handling
//! Pattern: Peripheral = base + registers + init + read/write
//!
//! # Modules
//!
//! - `slcr` - System Level Control (unlock/lock, clocks, resets)
//! - `i2c` - I2C master for PMBus
//! - `spi` - SPI master for ADC/DAC
//! - `gpio` - MIO pin control
//! - `xadc` - On-chip ADC (temperature, voltage)
//! - `uart` - Serial console
//! - `pcap` - FPGA programming via PCAP
//! - `pmbus` - PMBus protocol for power supplies
//! - `thermal` - ONETWO crystallization-based temperature control
//! - `eeprom` - 24LC02 I2C EEPROM for BIM configuration
//! - `dna` - Device DNA (unique chip ID for MAC/serial generation)
//! - `bu2505` - BU2505FV 10-channel DAC (SPI0/CS0)
//! - `max11131` - MAX11131 16-channel ADC (SPI0/CS1)
//! - `vicor` - VICOR core supply controller (6 cores)
//! - `ddr` - DDR controller (normally initialized by FSBL)

pub mod slcr;
pub mod ddr;
pub mod i2c;
pub mod spi;
pub mod gpio;
pub mod xadc;
pub mod uart;
pub mod pcap;
pub mod pmbus;
pub mod thermal;
pub mod eeprom;
pub mod dna;
pub mod bu2505;
pub mod max11131;
pub mod vicor;
pub mod sd;
pub mod gic;

// Re-export common types
pub use slcr::Slcr;
pub use i2c::{I2c, I2cError};
pub use spi::{Spi, SpiMode, SpiError};
pub use gpio::{Gpio, MioPin};
pub use xadc::Xadc;
pub use uart::Uart;
pub use pcap::Pcap;
pub use pmbus::{
    PmbusDevice, PmbusError, PowerSupplyManager, PowerSupplyInfo, MAX_POWER_SUPPLIES,
    lcps_channel_to_addr, lcps_addr_to_channel,
};
pub use thermal::{Thermal, ThermalOutput, PowerLevel, PowerEstimate, estimate_power, estimate_power_bytes};
pub use eeprom::{Eeprom, EepromError, BimEeprom, BimType, RailConfig, crc32, EEPROM_SIZE, EEPROM_ADDR};
pub use dna::{DeviceDna, mac_from_dna, ip_from_dna, read_device_dna};
pub use bu2505::Bu2505;
pub use max11131::Max11131;
pub use vicor::{VicorController, VicorError};
pub use sd::{SdCard, SdError};
pub use ddr::{Ddr, is_ddr_ready};
pub use gic::{Gic, IRQ_FLAGS, IRQ_FLAG_FBC};

/// Common register access trait
pub trait Register {
    fn read(&self) -> u32;
    fn write(&self, val: u32);

    #[inline(always)]
    fn modify<F: FnOnce(u32) -> u32>(&self, f: F) {
        self.write(f(self.read()));
    }

    #[inline(always)]
    fn set_bits(&self, mask: u32) {
        self.modify(|v| v | mask);
    }

    #[inline(always)]
    fn clear_bits(&self, mask: u32) {
        self.modify(|v| v & !mask);
    }
}

/// Memory-mapped register
#[derive(Clone, Copy)]
pub struct Reg(usize);

impl Reg {
    #[inline(always)]
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }

    #[inline(always)]
    pub const fn offset(&self, off: usize) -> Self {
        Self(self.0 + off)
    }
}

impl Register for Reg {
    #[inline(always)]
    fn read(&self) -> u32 {
        unsafe { core::ptr::read_volatile(self.0 as *const u32) }
    }

    #[inline(always)]
    fn write(&self, val: u32) {
        unsafe { core::ptr::write_volatile(self.0 as *mut u32, val) }
    }
}

// =============================================================================
// ARM Global Timer (MPCORE at 0xF8F00200)
// =============================================================================
// Runs at PERIPHCLK = CPU_CLK / 2 = 333.5 MHz (667 MHz / 2)
// 64-bit free-running counter, always available

const GLOBAL_TIMER_BASE: usize = 0xF8F0_0200;
const GT_COUNTER_LO: usize = GLOBAL_TIMER_BASE;
const GT_COUNTER_HI: usize = GLOBAL_TIMER_BASE + 0x04;
const GT_CONTROL: usize = GLOBAL_TIMER_BASE + 0x08;
const GT_TICKS_PER_MS: u64 = 333_500; // 333.5 MHz

/// Initialize the ARM Global Timer (enable if not already running)
pub fn init_global_timer() {
    unsafe {
        let ctrl = core::ptr::read_volatile(GT_CONTROL as *const u32);
        if ctrl & 1 == 0 {
            // Timer not running — enable it (prescaler=0, no IRQ, no compare)
            core::ptr::write_volatile(GT_CONTROL as *mut u32, 1);
        }
    }
}

/// Get milliseconds since timer started (wraps after ~584 years)
#[inline]
pub fn get_millis() -> u64 {
    // Read 64-bit counter atomically (read hi, lo, hi again — if hi changed, re-read)
    unsafe {
        loop {
            let hi1 = core::ptr::read_volatile(GT_COUNTER_HI as *const u32) as u64;
            let lo = core::ptr::read_volatile(GT_COUNTER_LO as *const u32) as u64;
            let hi2 = core::ptr::read_volatile(GT_COUNTER_HI as *const u32) as u64;
            if hi1 == hi2 {
                return ((hi1 << 32) | lo) / GT_TICKS_PER_MS;
            }
        }
    }
}

/// Busy-wait delay (approximate microseconds)
#[inline]
pub fn delay_us(us: u32) {
    // Assuming ~667 MHz CPU, ~3 cycles per loop iteration
    // This is approximate - real timing needs timer
    let cycles = us * 200;
    for _ in 0..cycles {
        core::hint::spin_loop();
    }
}

/// Busy-wait delay (approximate milliseconds)
#[inline]
pub fn delay_ms(ms: u32) {
    for _ in 0..ms {
        delay_us(1000);
    }
}
