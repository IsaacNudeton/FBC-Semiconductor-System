//! I2C Master Driver
//!
//! Supports Zynq I2C0 and I2C1 peripherals.
//! Used for PMBus communication with power supplies.
//!
//! # Protocol
//!
//! Standard I2C master with 7-bit addressing.
//! Supports repeated start for read-after-write.

use super::{Reg, Register, delay_us};

/// I2C0 base address
const I2C0_BASE: usize = 0xE000_4000;
/// I2C1 base address
const I2C1_BASE: usize = 0xE000_5000;

/// I2C register offsets
mod regs {
    pub const CR: usize = 0x00;       // Control Register
    pub const SR: usize = 0x04;       // Status Register
    pub const ADDR: usize = 0x08;     // Address Register
    pub const DATA: usize = 0x0C;     // Data Register
    pub const ISR: usize = 0x10;      // Interrupt Status
    pub const XFER_SIZE: usize = 0x14; // Transfer Size
    pub const SLV_PAUSE: usize = 0x18; // Slave Monitor Pause
    pub const TIME_OUT: usize = 0x1C;  // Time Out
    pub const IMR: usize = 0x20;      // Interrupt Mask
    pub const IER: usize = 0x24;      // Interrupt Enable
    pub const IDR: usize = 0x28;      // Interrupt Disable
}

/// Control Register bits
mod cr {
    pub const DIV_A_MASK: u32 = 0x0000_C000;
    pub const DIV_A_SHIFT: u32 = 14;
    pub const DIV_B_MASK: u32 = 0x0000_3F00;
    pub const DIV_B_SHIFT: u32 = 8;
    pub const CLR_FIFO: u32 = 1 << 6;
    pub const SLVMON: u32 = 1 << 5;
    pub const HOLD: u32 = 1 << 4;
    pub const ACKEN: u32 = 1 << 3;
    pub const NEA: u32 = 1 << 2;
    pub const MS: u32 = 1 << 1;
    pub const RW: u32 = 1 << 0;  // 0=write, 1=read
}

/// Status Register bits
mod sr {
    pub const BA: u32 = 1 << 8;      // Bus Active
    pub const RXOVF: u32 = 1 << 7;   // RX Overflow
    pub const TXDV: u32 = 1 << 6;    // TX Data Valid
    pub const RXDV: u32 = 1 << 5;    // RX Data Valid
    pub const RXRW: u32 = 1 << 3;    // RX Read/Write
}

/// Interrupt Status bits
mod isr {
    pub const ARB_LOST: u32 = 1 << 9;
    pub const RX_UNF: u32 = 1 << 7;
    pub const TX_OVF: u32 = 1 << 6;
    pub const RX_OVF: u32 = 1 << 5;
    pub const SLV_RDY: u32 = 1 << 4;
    pub const TO: u32 = 1 << 3;
    pub const NACK: u32 = 1 << 2;
    pub const DATA: u32 = 1 << 1;
    pub const COMP: u32 = 1 << 0;
}

/// I2C error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I2cError {
    /// No acknowledge from device
    Nack,
    /// Bus arbitration lost
    ArbitrationLost,
    /// Timeout waiting for operation
    Timeout,
    /// RX FIFO overflow
    RxOverflow,
    /// TX FIFO overflow
    TxOverflow,
    /// Invalid address
    InvalidAddress,
    /// Bus stuck (recovery failed)
    BusStuck,
    /// Maximum retries exceeded
    MaxRetriesExceeded,
}

/// I2C instance
pub struct I2c {
    base: Reg,
}

impl I2c {
    /// Create I2C0 instance
    pub const fn i2c0() -> Self {
        Self { base: Reg::new(I2C0_BASE) }
    }

    /// Create I2C1 instance
    pub const fn i2c1() -> Self {
        Self { base: Reg::new(I2C1_BASE) }
    }

    /// Initialize I2C peripheral
    ///
    /// # Arguments
    /// * `speed_khz` - Bus speed in kHz (100 for standard, 400 for fast)
    pub fn init(&self, speed_khz: u32) {
        // Calculate divisors for desired speed
        // Input clock is typically 111 MHz (CPU_1x)
        // SCL = 111 MHz / (22 * (DIV_A + 1) * (DIV_B + 1))
        let (div_a, div_b) = match speed_khz {
            0..=100 => (3, 14),    // ~100 kHz
            101..=400 => (1, 7),   // ~400 kHz
            _ => (0, 4),           // ~1 MHz
        };

        // Clear FIFO and configure
        let mut ctrl = cr::CLR_FIFO | cr::ACKEN | cr::MS;
        ctrl |= (div_a << cr::DIV_A_SHIFT) & cr::DIV_A_MASK;
        ctrl |= (div_b << cr::DIV_B_SHIFT) & cr::DIV_B_MASK;
        self.base.offset(regs::CR).write(ctrl);

        // Clear any pending interrupts
        self.base.offset(regs::ISR).write(0x2FF);

        // Set timeout
        self.base.offset(regs::TIME_OUT).write(0xFF);
    }

    /// Check if bus is busy
    pub fn is_busy(&self) -> bool {
        self.base.offset(regs::SR).read() & sr::BA != 0
    }

    /// Wait for bus to be free
    fn wait_bus_free(&self) -> Result<(), I2cError> {
        for _ in 0..10000 {
            if !self.is_busy() {
                return Ok(());
            }
            delay_us(1);
        }
        Err(I2cError::Timeout)
    }

    /// Wait for transfer complete
    fn wait_complete(&self) -> Result<(), I2cError> {
        for _ in 0..10000 {
            let isr = self.base.offset(regs::ISR).read();

            if isr & isr::NACK != 0 {
                self.base.offset(regs::ISR).write(isr::NACK);
                return Err(I2cError::Nack);
            }
            if isr & isr::ARB_LOST != 0 {
                self.base.offset(regs::ISR).write(isr::ARB_LOST);
                return Err(I2cError::ArbitrationLost);
            }
            if isr & isr::TO != 0 {
                self.base.offset(regs::ISR).write(isr::TO);
                return Err(I2cError::Timeout);
            }
            if isr & isr::COMP != 0 {
                self.base.offset(regs::ISR).write(isr::COMP);
                return Ok(());
            }

            delay_us(1);
        }
        Err(I2cError::Timeout)
    }

    /// Write bytes to device
    ///
    /// # Arguments
    /// * `addr` - 7-bit device address
    /// * `data` - Bytes to write
    pub fn write(&self, addr: u8, data: &[u8]) -> Result<(), I2cError> {
        if data.is_empty() {
            return Ok(());
        }
        if addr > 0x7F {
            return Err(I2cError::InvalidAddress);
        }

        self.wait_bus_free()?;

        // Clear FIFO
        self.base.offset(regs::CR).modify(|v| v | cr::CLR_FIFO);

        // Clear interrupts
        self.base.offset(regs::ISR).write(0x2FF);

        // Set address (write mode = bit 0 clear)
        self.base.offset(regs::ADDR).write((addr as u32) << 0);

        // Set transfer size
        self.base.offset(regs::XFER_SIZE).write(data.len() as u32);

        // Write data to FIFO
        for &byte in data {
            self.base.offset(regs::DATA).write(byte as u32);
        }

        // Start transfer (master, write)
        let ctrl = self.base.offset(regs::CR).read();
        self.base.offset(regs::CR).write((ctrl | cr::MS) & !cr::RW & !cr::HOLD);

        self.wait_complete()
    }

    /// Read bytes from device
    ///
    /// # Arguments
    /// * `addr` - 7-bit device address
    /// * `buf` - Buffer to store read data
    pub fn read(&self, addr: u8, buf: &mut [u8]) -> Result<(), I2cError> {
        if buf.is_empty() {
            return Ok(());
        }
        if addr > 0x7F {
            return Err(I2cError::InvalidAddress);
        }

        self.wait_bus_free()?;

        // Clear FIFO
        self.base.offset(regs::CR).modify(|v| v | cr::CLR_FIFO);

        // Clear interrupts
        self.base.offset(regs::ISR).write(0x2FF);

        // Set transfer size
        self.base.offset(regs::XFER_SIZE).write(buf.len() as u32);

        // Set address (read mode = bit 0 set) and start transfer
        self.base.offset(regs::ADDR).write((addr as u32) << 0);

        // Start transfer (master, read)
        let ctrl = self.base.offset(regs::CR).read();
        self.base.offset(regs::CR).write(ctrl | cr::MS | cr::RW);

        // Wait for data
        for byte in buf.iter_mut() {
            // Wait for RX data valid
            for _ in 0..10000 {
                if self.base.offset(regs::SR).read() & sr::RXDV != 0 {
                    break;
                }
                delay_us(1);
            }
            *byte = self.base.offset(regs::DATA).read() as u8;
        }

        self.wait_complete()
    }

    /// Write then read (repeated start)
    ///
    /// # Arguments
    /// * `addr` - 7-bit device address
    /// * `write_data` - Bytes to write first
    /// * `read_buf` - Buffer for read data
    pub fn write_read(&self, addr: u8, write_data: &[u8], read_buf: &mut [u8]) -> Result<(), I2cError> {
        if addr > 0x7F {
            return Err(I2cError::InvalidAddress);
        }

        self.wait_bus_free()?;

        // Clear FIFO
        self.base.offset(regs::CR).modify(|v| v | cr::CLR_FIFO);

        // Clear interrupts
        self.base.offset(regs::ISR).write(0x2FF);

        // Set address for write
        self.base.offset(regs::ADDR).write((addr as u32) << 0);

        // Set transfer size for write
        self.base.offset(regs::XFER_SIZE).write(write_data.len() as u32);

        // Write data to FIFO
        for &byte in write_data {
            self.base.offset(regs::DATA).write(byte as u32);
        }

        // Start write with HOLD (keep bus after transfer)
        let ctrl = self.base.offset(regs::CR).read();
        self.base.offset(regs::CR).write((ctrl | cr::MS | cr::HOLD) & !cr::RW);

        // Wait for write complete
        self.wait_complete()?;

        // Clear FIFO for read
        self.base.offset(regs::CR).modify(|v| v | cr::CLR_FIFO);

        // Set transfer size for read
        self.base.offset(regs::XFER_SIZE).write(read_buf.len() as u32);

        // Start read (still holding bus, then release)
        let ctrl = self.base.offset(regs::CR).read();
        self.base.offset(regs::CR).write((ctrl | cr::MS | cr::RW) & !cr::HOLD);

        // Read data
        for byte in read_buf.iter_mut() {
            for _ in 0..10000 {
                if self.base.offset(regs::SR).read() & sr::RXDV != 0 {
                    break;
                }
                delay_us(1);
            }
            *byte = self.base.offset(regs::DATA).read() as u8;
        }

        self.wait_complete()
    }

    /// Scan I2C bus for devices
    ///
    /// Returns a bitmask of addresses that responded (0x08-0x77)
    pub fn scan(&self) -> [u8; 16] {
        let mut found = [0u8; 16];  // 128 bits for addresses 0-127

        for addr in 0x08..0x78 {
            // Try to read one byte from each address
            let mut buf = [0u8; 1];
            if self.read(addr, &mut buf).is_ok() {
                found[(addr / 8) as usize] |= 1 << (addr % 8);
            }
            delay_us(100);  // Small delay between probes
        }

        found
    }

    // =========================================================================
    // Bus Recovery (Gap 5)
    // =========================================================================

    /// Attempt I2C bus recovery when bus is stuck
    ///
    /// This follows the I2C specification for bus recovery:
    /// 1. Generate 9 clock pulses to complete any stuck transaction
    /// 2. Generate STOP condition
    /// 3. Re-initialize the peripheral
    ///
    /// # Returns
    /// * `Ok(())` if bus was recovered
    /// * `Err(I2cError::BusStuck)` if bus remains stuck after recovery attempt
    pub fn recover_bus(&self) -> Result<(), I2cError> {
        // Step 1: Reset the I2C controller
        // Clear FIFO and disable controller
        self.base.offset(regs::CR).write(cr::CLR_FIFO);

        // Wait a bit for controller to reset
        delay_us(100);

        // Step 2: Toggle SCL 9 times by attempting a dummy read
        // The Zynq I2C controller doesn't expose direct SCL/SDA control,
        // so we use the hardware to generate clocks via a dummy transaction
        //
        // We do this by setting up a read with no data expected,
        // which will generate clock pulses until NACK

        // Clear any pending interrupts
        self.base.offset(regs::ISR).write(0x2FF);

        // Set a short timeout to avoid hanging
        self.base.offset(regs::TIME_OUT).write(0x20);

        // Try to read from address 0x00 (general call - usually not used)
        // This will generate clock cycles
        self.base.offset(regs::XFER_SIZE).write(1);
        self.base.offset(regs::ADDR).write(0);

        // Start read transfer
        let ctrl = cr::MS | cr::RW | cr::ACKEN;
        self.base.offset(regs::CR).write(ctrl);

        // Wait for it to complete (expect NACK or timeout)
        for _ in 0..1000 {
            let isr = self.base.offset(regs::ISR).read();
            if isr & (isr::NACK | isr::COMP | isr::TO) != 0 {
                break;
            }
            delay_us(10);
        }

        // Clear all interrupts
        self.base.offset(regs::ISR).write(0x2FF);

        // Step 3: Re-initialize the controller
        // Restore normal timeout
        self.base.offset(regs::TIME_OUT).write(0xFF);

        // Re-initialize with standard settings
        let ctrl = cr::CLR_FIFO | cr::ACKEN | cr::MS;
        self.base.offset(regs::CR).write(ctrl);

        delay_us(100);

        // Step 4: Verify bus is now free
        if self.is_busy() {
            return Err(I2cError::BusStuck);
        }

        Ok(())
    }

    /// Write bytes to device with automatic retry and bus recovery
    ///
    /// # Arguments
    /// * `addr` - 7-bit device address
    /// * `data` - Bytes to write
    /// * `max_retries` - Maximum number of retry attempts (0 = no retries)
    ///
    /// # Retry behavior
    /// - On NACK: retry immediately (device may be busy)
    /// - On ArbitrationLost/Timeout: attempt bus recovery, then retry
    /// - Uses exponential backoff: 1ms, 2ms, 4ms, 8ms, ... up to 100ms
    pub fn write_with_retry(&self, addr: u8, data: &[u8], max_retries: u8) -> Result<(), I2cError> {
        let mut delay_ms = 1u32;

        for attempt in 0..=max_retries {
            match self.write(addr, data) {
                Ok(()) => return Ok(()),
                Err(I2cError::Nack) if attempt < max_retries => {
                    // Device busy, retry after short delay
                    delay_us(delay_ms * 1000);
                    delay_ms = (delay_ms * 2).min(100);
                }
                Err(I2cError::ArbitrationLost) | Err(I2cError::Timeout) if attempt < max_retries => {
                    // Bus issue, attempt recovery
                    let _ = self.recover_bus();
                    delay_us(delay_ms * 1000);
                    delay_ms = (delay_ms * 2).min(100);
                }
                Err(e) => return Err(e),
            }
        }

        Err(I2cError::MaxRetriesExceeded)
    }

    /// Read bytes from device with automatic retry and bus recovery
    ///
    /// # Arguments
    /// * `addr` - 7-bit device address
    /// * `buf` - Buffer to store read data
    /// * `max_retries` - Maximum number of retry attempts (0 = no retries)
    pub fn read_with_retry(&self, addr: u8, buf: &mut [u8], max_retries: u8) -> Result<(), I2cError> {
        let mut delay_ms = 1u32;

        for attempt in 0..=max_retries {
            match self.read(addr, buf) {
                Ok(()) => return Ok(()),
                Err(I2cError::Nack) if attempt < max_retries => {
                    delay_us(delay_ms * 1000);
                    delay_ms = (delay_ms * 2).min(100);
                }
                Err(I2cError::ArbitrationLost) | Err(I2cError::Timeout) if attempt < max_retries => {
                    let _ = self.recover_bus();
                    delay_us(delay_ms * 1000);
                    delay_ms = (delay_ms * 2).min(100);
                }
                Err(e) => return Err(e),
            }
        }

        Err(I2cError::MaxRetriesExceeded)
    }

    /// Write-read (repeated start) with automatic retry and bus recovery
    ///
    /// # Arguments
    /// * `addr` - 7-bit device address
    /// * `write_data` - Bytes to write first
    /// * `read_buf` - Buffer for read data
    /// * `max_retries` - Maximum number of retry attempts (0 = no retries)
    pub fn write_read_with_retry(
        &self,
        addr: u8,
        write_data: &[u8],
        read_buf: &mut [u8],
        max_retries: u8,
    ) -> Result<(), I2cError> {
        let mut delay_ms = 1u32;

        for attempt in 0..=max_retries {
            match self.write_read(addr, write_data, read_buf) {
                Ok(()) => return Ok(()),
                Err(I2cError::Nack) if attempt < max_retries => {
                    delay_us(delay_ms * 1000);
                    delay_ms = (delay_ms * 2).min(100);
                }
                Err(I2cError::ArbitrationLost) | Err(I2cError::Timeout) if attempt < max_retries => {
                    let _ = self.recover_bus();
                    delay_us(delay_ms * 1000);
                    delay_ms = (delay_ms * 2).min(100);
                }
                Err(e) => return Err(e),
            }
        }

        Err(I2cError::MaxRetriesExceeded)
    }

    /// Generic transfer with retry wrapper
    ///
    /// Executes an arbitrary I2C operation with retry logic
    ///
    /// # Arguments
    /// * `max_retries` - Maximum retry attempts
    /// * `f` - Closure that performs the I2C operation
    pub fn transfer_with_retry<T, F>(&self, max_retries: u8, mut f: F) -> Result<T, I2cError>
    where
        F: FnMut(&Self) -> Result<T, I2cError>,
    {
        let mut delay_ms = 1u32;

        for attempt in 0..=max_retries {
            match f(self) {
                Ok(result) => return Ok(result),
                Err(I2cError::Nack) if attempt < max_retries => {
                    delay_us(delay_ms * 1000);
                    delay_ms = (delay_ms * 2).min(100);
                }
                Err(I2cError::ArbitrationLost) | Err(I2cError::Timeout) if attempt < max_retries => {
                    let _ = self.recover_bus();
                    delay_us(delay_ms * 1000);
                    delay_ms = (delay_ms * 2).min(100);
                }
                Err(e) => return Err(e),
            }
        }

        Err(I2cError::MaxRetriesExceeded)
    }
}
