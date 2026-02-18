//! UART Driver
//!
//! Controls the Zynq PS UART0 and UART1 peripherals.
//! Used for serial console and debugging.
//!
//! # ONETWO Design
//!
//! Invariant: Register addresses, FIFO depths (64 bytes), baud rate formula
//! Varies: Baud rate, parity, stop bits, flow control
//! Pattern: init(baud) → tx/rx bytes via FIFO

use super::{Reg, Register};
use core::fmt::{self, Write};

/// UART0 base address
const UART0_BASE: usize = 0xE000_0000;
/// UART1 base address
const UART1_BASE: usize = 0xE000_1000;

/// UART register offsets
mod regs {
    pub const CR: usize = 0x00;        // Control Register
    pub const MR: usize = 0x04;        // Mode Register
    pub const IER: usize = 0x08;       // Interrupt Enable
    pub const IDR: usize = 0x0C;       // Interrupt Disable
    pub const IMR: usize = 0x10;       // Interrupt Mask
    pub const ISR: usize = 0x14;       // Interrupt Status (Channel_sts in TRM)
    pub const BAUDGEN: usize = 0x18;   // Baud Rate Generator
    pub const RXTOUT: usize = 0x1C;    // RX Timeout
    pub const RXWM: usize = 0x20;      // RX FIFO Trigger Level
    pub const MODEMCR: usize = 0x24;   // Modem Control
    pub const MODEMSR: usize = 0x28;   // Modem Status
    pub const SR: usize = 0x2C;        // Channel Status
    pub const FIFO: usize = 0x30;      // TX/RX FIFO
    pub const BAUDDIV: usize = 0x34;   // Baud Rate Divider
    pub const FLOWDEL: usize = 0x38;   // Flow Control Delay
    pub const TXWM: usize = 0x44;      // TX FIFO Trigger Level
}

/// Control Register bits
mod cr {
    pub const STPBRK: u32 = 1 << 8;    // Stop TX break
    pub const STTBRK: u32 = 1 << 7;    // Start TX break
    pub const RSTTO: u32 = 1 << 6;     // Restart RX timeout
    pub const TXDIS: u32 = 1 << 5;     // TX disable
    pub const TXEN: u32 = 1 << 4;      // TX enable
    pub const RXDIS: u32 = 1 << 3;     // RX disable
    pub const RXEN: u32 = 1 << 2;      // RX enable
    pub const TXRST: u32 = 1 << 1;     // TX reset
    pub const RXRST: u32 = 1 << 0;     // RX reset
}

/// Mode Register bits
mod mr {
    pub const CHMODE_MASK: u32 = 0x3 << 8;
    pub const CHMODE_NORMAL: u32 = 0 << 8;
    pub const CHMODE_ECHO: u32 = 1 << 8;
    pub const CHMODE_LLOOP: u32 = 2 << 8;  // Local loopback
    pub const CHMODE_RLOOP: u32 = 3 << 8;  // Remote loopback

    pub const NBSTOP_MASK: u32 = 0x3 << 6;
    pub const NBSTOP_1: u32 = 0 << 6;      // 1 stop bit
    pub const NBSTOP_1_5: u32 = 1 << 6;    // 1.5 stop bits
    pub const NBSTOP_2: u32 = 2 << 6;      // 2 stop bits

    pub const PAR_MASK: u32 = 0x7 << 3;
    pub const PAR_EVEN: u32 = 0 << 3;
    pub const PAR_ODD: u32 = 1 << 3;
    pub const PAR_SPACE: u32 = 2 << 3;
    pub const PAR_MARK: u32 = 3 << 3;
    pub const PAR_NONE: u32 = 4 << 3;

    pub const CHRL_MASK: u32 = 0x3 << 1;
    pub const CHRL_8: u32 = 0 << 1;        // 8 data bits
    pub const CHRL_7: u32 = 2 << 1;        // 7 data bits
    pub const CHRL_6: u32 = 3 << 1;        // 6 data bits

    pub const CLKS: u32 = 1 << 0;          // Clock source (0=uart_ref_clk)
}

/// Status Register bits
mod sr {
    pub const TNFUL: u32 = 1 << 14;   // TX FIFO nearly full
    pub const TTRIG: u32 = 1 << 13;   // TX FIFO trigger
    pub const FDELT: u32 = 1 << 12;   // RX FIFO fill over delay
    pub const TACTIVE: u32 = 1 << 11; // TX active
    pub const RACTIVE: u32 = 1 << 10; // RX active
    pub const TFUL: u32 = 1 << 4;     // TX FIFO full
    pub const TEMPTY: u32 = 1 << 3;   // TX FIFO empty
    pub const RFUL: u32 = 1 << 2;     // RX FIFO full
    pub const REMPTY: u32 = 1 << 1;   // RX FIFO empty
    pub const RTRIG: u32 = 1 << 0;    // RX FIFO trigger
}

/// Interrupt bits (ISR/IER/IDR/IMR)
mod isr {
    pub const TOVR: u32 = 1 << 12;    // TX FIFO overflow
    pub const TNFUL: u32 = 1 << 11;   // TX FIFO nearly full
    pub const TTRIG: u32 = 1 << 10;   // TX FIFO trigger
    pub const DMSI: u32 = 1 << 9;     // Delta modem status
    pub const TIMEOUT: u32 = 1 << 8;  // RX timeout
    pub const PARE: u32 = 1 << 7;     // Parity error
    pub const FRAME: u32 = 1 << 6;    // Framing error
    pub const ROVR: u32 = 1 << 5;     // RX overflow
    pub const TFUL: u32 = 1 << 4;     // TX FIFO full
    pub const TEMPTY: u32 = 1 << 3;   // TX FIFO empty
    pub const RFUL: u32 = 1 << 2;     // RX FIFO full
    pub const REMPTY: u32 = 1 << 1;   // RX FIFO empty
    pub const RTRIG: u32 = 1 << 0;    // RX FIFO trigger
}

/// UART configuration
#[derive(Clone, Copy)]
pub struct UartConfig {
    pub baud_rate: u32,
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBits,
}

impl Default for UartConfig {
    fn default() -> Self {
        Self {
            baud_rate: 115200,
            data_bits: DataBits::Eight,
            parity: Parity::None,
            stop_bits: StopBits::One,
        }
    }
}

/// Data bits
#[derive(Clone, Copy)]
pub enum DataBits {
    Six,
    Seven,
    Eight,
}

/// Parity
#[derive(Clone, Copy)]
pub enum Parity {
    None,
    Even,
    Odd,
}

/// Stop bits
#[derive(Clone, Copy)]
pub enum StopBits {
    One,
    OnePointFive,
    Two,
}

/// UART error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UartError {
    /// TX FIFO overflow
    TxOverflow,
    /// RX FIFO overflow
    RxOverflow,
    /// Parity error
    ParityError,
    /// Framing error
    FramingError,
    /// Timeout
    Timeout,
}

/// UART instance
pub struct Uart {
    base: Reg,
}

impl Uart {
    /// Create UART0 instance (typically console)
    pub const fn uart0() -> Self {
        Self { base: Reg::new(UART0_BASE) }
    }

    /// Create UART1 instance
    pub const fn uart1() -> Self {
        Self { base: Reg::new(UART1_BASE) }
    }

    /// Initialize UART with configuration
    ///
    /// # Arguments
    /// * `config` - UART configuration (baud rate, data bits, parity, stop bits)
    /// * `ref_clk` - Reference clock frequency in Hz (typically 100 MHz)
    pub fn init(&self, config: &UartConfig, ref_clk: u32) {
        // Disable TX and RX, reset FIFOs
        self.base.offset(regs::CR).write(cr::TXDIS | cr::RXDIS | cr::TXRST | cr::RXRST);

        // Configure mode
        let mut mode = mr::CHMODE_NORMAL;

        mode |= match config.data_bits {
            DataBits::Six => mr::CHRL_6,
            DataBits::Seven => mr::CHRL_7,
            DataBits::Eight => mr::CHRL_8,
        };

        mode |= match config.parity {
            Parity::None => mr::PAR_NONE,
            Parity::Even => mr::PAR_EVEN,
            Parity::Odd => mr::PAR_ODD,
        };

        mode |= match config.stop_bits {
            StopBits::One => mr::NBSTOP_1,
            StopBits::OnePointFive => mr::NBSTOP_1_5,
            StopBits::Two => mr::NBSTOP_2,
        };

        self.base.offset(regs::MR).write(mode);

        // Calculate baud rate divisors
        // Baud = ref_clk / (CD * (BDIV + 1))
        // We use BDIV = 4 (5 cycles per bit), solve for CD
        let bdiv = 4u32;
        let cd = ref_clk / (config.baud_rate * (bdiv + 1));

        self.base.offset(regs::BAUDGEN).write(cd);
        self.base.offset(regs::BAUDDIV).write(bdiv);

        // Set RX FIFO trigger level (1 = interrupt on any data)
        self.base.offset(regs::RXWM).write(1);

        // Set RX timeout (10 character times)
        self.base.offset(regs::RXTOUT).write(10);

        // Clear all interrupts
        self.base.offset(regs::ISR).write(0xFFFFFFFF);

        // Disable all interrupts (we'll poll)
        self.base.offset(regs::IDR).write(0xFFFFFFFF);

        // Enable TX and RX
        self.base.offset(regs::CR).write(cr::TXEN | cr::RXEN);
    }

    /// Initialize with default settings (115200 8N1)
    pub fn init_default(&self) {
        self.init(&UartConfig::default(), 100_000_000);
    }

    /// Check if TX FIFO is full
    #[inline]
    pub fn is_tx_full(&self) -> bool {
        self.base.offset(regs::SR).read() & sr::TFUL != 0
    }

    /// Check if TX FIFO is empty
    #[inline]
    pub fn is_tx_empty(&self) -> bool {
        self.base.offset(regs::SR).read() & sr::TEMPTY != 0
    }

    /// Check if RX FIFO is empty
    #[inline]
    pub fn is_rx_empty(&self) -> bool {
        self.base.offset(regs::SR).read() & sr::REMPTY != 0
    }

    /// Check if RX FIFO has data
    #[inline]
    pub fn is_rx_available(&self) -> bool {
        !self.is_rx_empty()
    }

    /// Write a single byte (blocking)
    pub fn write_byte(&self, byte: u8) {
        // Wait for TX FIFO not full
        while self.is_tx_full() {
            core::hint::spin_loop();
        }
        self.base.offset(regs::FIFO).write(byte as u32);
    }

    /// Try to write a byte (non-blocking)
    ///
    /// Returns Ok(()) if written, Err(TxOverflow) if FIFO full
    pub fn try_write_byte(&self, byte: u8) -> Result<(), UartError> {
        if self.is_tx_full() {
            Err(UartError::TxOverflow)
        } else {
            self.base.offset(regs::FIFO).write(byte as u32);
            Ok(())
        }
    }

    /// Read a single byte (blocking)
    pub fn read_byte(&self) -> u8 {
        // Wait for RX data
        while self.is_rx_empty() {
            core::hint::spin_loop();
        }
        self.base.offset(regs::FIFO).read() as u8
    }

    /// Try to read a byte (non-blocking)
    ///
    /// Returns Some(byte) if data available, None otherwise
    pub fn try_read_byte(&self) -> Option<u8> {
        if self.is_rx_empty() {
            None
        } else {
            Some(self.base.offset(regs::FIFO).read() as u8)
        }
    }

    /// Read byte with timeout
    pub fn read_byte_timeout(&self, timeout_us: u32) -> Result<u8, UartError> {
        for _ in 0..(timeout_us / 10) {
            if let Some(byte) = self.try_read_byte() {
                return Ok(byte);
            }
            super::delay_us(10);
        }
        Err(UartError::Timeout)
    }

    /// Write bytes (blocking)
    pub fn write_bytes(&self, data: &[u8]) {
        for &byte in data {
            self.write_byte(byte);
        }
    }

    /// Write a string
    pub fn write_str(&self, s: &str) {
        self.write_bytes(s.as_bytes());
    }

    /// Read bytes into buffer
    ///
    /// Reads until buffer is full or no more data available
    pub fn read_bytes(&self, buf: &mut [u8]) -> usize {
        let mut count = 0;
        for byte in buf.iter_mut() {
            if let Some(b) = self.try_read_byte() {
                *byte = b;
                count += 1;
            } else {
                break;
            }
        }
        count
    }

    /// Flush TX FIFO (wait for all data to be transmitted)
    pub fn flush(&self) {
        while !self.is_tx_empty() {
            core::hint::spin_loop();
        }
        // Also wait for transmitter to finish
        while self.base.offset(regs::SR).read() & sr::TACTIVE != 0 {
            core::hint::spin_loop();
        }
    }

    /// Check and clear error flags
    pub fn check_errors(&self) -> Result<(), UartError> {
        let isr = self.base.offset(regs::ISR).read();

        if isr & isr::ROVR != 0 {
            self.base.offset(regs::ISR).write(isr::ROVR);
            return Err(UartError::RxOverflow);
        }
        if isr & isr::PARE != 0 {
            self.base.offset(regs::ISR).write(isr::PARE);
            return Err(UartError::ParityError);
        }
        if isr & isr::FRAME != 0 {
            self.base.offset(regs::ISR).write(isr::FRAME);
            return Err(UartError::FramingError);
        }

        Ok(())
    }

    /// Enable loopback mode (for testing)
    pub fn enable_loopback(&self) {
        self.base.offset(regs::MR).modify(|v| (v & !mr::CHMODE_MASK) | mr::CHMODE_LLOOP);
    }

    /// Disable loopback mode
    pub fn disable_loopback(&self) {
        self.base.offset(regs::MR).modify(|v| (v & !mr::CHMODE_MASK) | mr::CHMODE_NORMAL);
    }
}

/// Implement fmt::Write for UART (enables write! macro)
impl Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes());
        Ok(())
    }
}

/// Print macro for UART0
#[macro_export]
macro_rules! uart_print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let uart = $crate::hal::Uart::uart0();
        let _ = write!(uart, $($arg)*);
    }};
}

/// Println macro for UART0
#[macro_export]
macro_rules! uart_println {
    () => ($crate::uart_print!("\r\n"));
    ($($arg:tt)*) => {{
        $crate::uart_print!($($arg)*);
        $crate::uart_print!("\r\n");
    }};
}
