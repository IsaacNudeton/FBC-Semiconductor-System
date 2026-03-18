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
