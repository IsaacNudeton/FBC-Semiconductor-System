//! System Level Control Registers (SLCR)
//!
//! Controls clocks, resets, MIO pin muxing, and peripheral enables.
//!
//! # Safety
//!
//! SLCR must be unlocked before writing. Always re-lock after changes.

use super::{Reg, Register};

/// SLCR base address
const SLCR_BASE: usize = 0xF800_0000;

/// Unlock key
const UNLOCK_KEY: u32 = 0xDF0D;
/// Lock key
const LOCK_KEY: u32 = 0x767B;

/// SLCR register offsets
mod regs {
    pub const LOCK: usize = 0x004;
    pub const UNLOCK: usize = 0x008;
    pub const ARM_CLK_CTRL: usize = 0x120;
    pub const DDR_CLK_CTRL: usize = 0x124;
    pub const DCI_CLK_CTRL: usize = 0x128;
    pub const APER_CLK_CTRL: usize = 0x12C;
    pub const USB0_CLK_CTRL: usize = 0x130;
    pub const USB1_CLK_CTRL: usize = 0x134;
    pub const GEM0_CLK_CTRL: usize = 0x140;
    pub const GEM1_CLK_CTRL: usize = 0x144;
    pub const SMC_CLK_CTRL: usize = 0x148;
    pub const LQSPI_CLK_CTRL: usize = 0x14C;
    pub const SDIO_CLK_CTRL: usize = 0x150;
    pub const UART_CLK_CTRL: usize = 0x154;
    pub const SPI_CLK_CTRL: usize = 0x158;
    pub const CAN_CLK_CTRL: usize = 0x15C;
    pub const PCAP_CLK_CTRL: usize = 0x168;
    pub const FPGA0_CLK_CTRL: usize = 0x170;
    pub const FPGA1_CLK_CTRL: usize = 0x180;
    pub const FPGA2_CLK_CTRL: usize = 0x190;
    pub const FPGA3_CLK_CTRL: usize = 0x1A0;
    pub const CLK_621_TRUE: usize = 0x1C4;
    pub const PSS_RST_CTRL: usize = 0x200;
    pub const DDR_RST_CTRL: usize = 0x204;
    pub const FPGA_RST_CTRL: usize = 0x240;
    pub const A9_CPU_RST_CTRL: usize = 0x244;
    pub const RS_AWDT_CTRL: usize = 0x24C;
    pub const REBOOT_STATUS: usize = 0x258;
    pub const MIO_PIN_00: usize = 0x700;  // Through MIO_PIN_53 at 0x7D4
    pub const MIO_LOOPBACK: usize = 0x804;
    pub const MIO_MST_TRI0: usize = 0x80C;
    pub const MIO_MST_TRI1: usize = 0x810;
    pub const GPIOB_CTRL: usize = 0xB00;
    pub const GPIOB_CFG_CMOS18: usize = 0xB04;
    pub const GPIOB_CFG_CMOS25: usize = 0xB08;
    pub const GPIOB_CFG_CMOS33: usize = 0xB0C;
}

/// APER_CLK_CTRL bits
pub mod aper_clk {
    pub const DMA_EN: u32 = 1 << 0;
    pub const USB0_EN: u32 = 1 << 2;
    pub const USB1_EN: u32 = 1 << 3;
    pub const GEM0_EN: u32 = 1 << 6;
    pub const GEM1_EN: u32 = 1 << 7;
    pub const SDI0_EN: u32 = 1 << 10;
    pub const SDI1_EN: u32 = 1 << 11;
    pub const SPI0_EN: u32 = 1 << 14;
    pub const SPI1_EN: u32 = 1 << 15;
    pub const CAN0_EN: u32 = 1 << 16;
    pub const CAN1_EN: u32 = 1 << 17;
    pub const I2C0_EN: u32 = 1 << 18;
    pub const I2C1_EN: u32 = 1 << 19;
    pub const UART0_EN: u32 = 1 << 20;
    pub const UART1_EN: u32 = 1 << 21;
    pub const GPIO_EN: u32 = 1 << 22;
    pub const LQSPI_EN: u32 = 1 << 23;
    pub const SMC_EN: u32 = 1 << 24;
}

/// System Level Control
pub struct Slcr {
    base: Reg,
}

impl Slcr {
    /// Create SLCR instance (singleton in practice)
    pub const fn new() -> Self {
        Self { base: Reg::new(SLCR_BASE) }
    }

    /// Unlock SLCR for writing
    #[inline]
    pub fn unlock(&self) {
        self.base.offset(regs::UNLOCK).write(UNLOCK_KEY);
    }

    /// Lock SLCR (prevent writes)
    #[inline]
    pub fn lock(&self) {
        self.base.offset(regs::LOCK).write(LOCK_KEY);
    }

    /// Execute a function with SLCR unlocked
    #[inline]
    pub fn with_unlock<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Self) -> R,
    {
        self.unlock();
        let result = f(self);
        self.lock();
        result
    }

    // =========================================================================
    // Clock Control
    // =========================================================================

    /// Enable peripheral clock
    pub fn enable_peripheral_clock(&self, mask: u32) {
        self.with_unlock(|s| {
            s.base.offset(regs::APER_CLK_CTRL).set_bits(mask);
        });
    }

    /// Disable peripheral clock
    pub fn disable_peripheral_clock(&self, mask: u32) {
        self.with_unlock(|s| {
            s.base.offset(regs::APER_CLK_CTRL).clear_bits(mask);
        });
    }

    /// Get APER_CLK_CTRL value
    pub fn get_peripheral_clocks(&self) -> u32 {
        self.base.offset(regs::APER_CLK_CTRL).read()
    }

    /// Enable I2C0 clock
    pub fn enable_i2c0(&self) {
        self.enable_peripheral_clock(aper_clk::I2C0_EN);
    }

    /// Enable I2C1 clock
    pub fn enable_i2c1(&self) {
        self.enable_peripheral_clock(aper_clk::I2C1_EN);
    }

    /// Enable SPI0 clock
    pub fn enable_spi0(&self) {
        self.enable_peripheral_clock(aper_clk::SPI0_EN);
    }

    /// Enable SPI1 clock
    pub fn enable_spi1(&self) {
        self.enable_peripheral_clock(aper_clk::SPI1_EN);
    }

    /// Enable UART0 clock
    pub fn enable_uart0(&self) {
        self.enable_peripheral_clock(aper_clk::UART0_EN);
    }

    /// Enable UART1 clock
    pub fn enable_uart1(&self) {
        self.enable_peripheral_clock(aper_clk::UART1_EN);
    }

    /// Enable GPIO clock
    pub fn enable_gpio(&self) {
        self.enable_peripheral_clock(aper_clk::GPIO_EN);
    }

    /// Enable GEM0 (Ethernet) clock
    pub fn enable_gem0(&self) {
        self.enable_peripheral_clock(aper_clk::GEM0_EN);
    }

    /// Configure GEM0 reference clock for Ethernet
    /// This sets GEM0_CLK_CTRL (0x140) for TX clock generation
    /// IO PLL = 1000MHz. Dividers: 1G=8 (125MHz), 100M=40 (25MHz), 10M=400 (2.5MHz)
    pub fn configure_gem0_clock_for_speed(&self, speed_100m: bool) {
        self.with_unlock(|s| {
            // GEM0_CLK_CTRL:
            // Bits 25:20 = DIVISOR1
            // Bits 13:8 = DIVISOR0
            // Bits 5:4 = SRCSEL (00 = IO PLL)
            // Bit 0 = CLKACT (1 = active)
            let (div0, div1) = if speed_100m {
                (40, 1)   // 1000MHz / 40 = 25MHz for 100Mbps RGMII
            } else {
                (8, 1)    // 1000MHz / 8 = 125MHz for 1Gbps RGMII
            };
            let val = (div1 << 20)
                    | (div0 << 8)
                    | (0 << 4)    // SRCSEL = IO PLL
                    | (1 << 0);   // CLKACT = enabled
            s.base.offset(regs::GEM0_CLK_CTRL).write(val);
        });
    }

    /// Configure GEM0 reference clock (default 25MHz for 100M)
    pub fn configure_gem0_clock(&self) {
        self.configure_gem0_clock_for_speed(true);
    }

    /// Configure GEM0 RX clock source
    /// GEM0_RCLK_CTRL at SLCR offset 0x138:
    ///   Bit 0: CLKACT — 1 = enable RX reference clock
    ///   Bit 4: SRCSEL — 0 = RX clock from MIO/EMIO pad (PHY provides RGMII_RXC)
    pub fn configure_gem0_rclk(&self) {
        self.with_unlock(|s| {
            s.base.offset(0x138).write(1); // CLKACT=1, SRCSEL=0 (PHY provides RX clock via MIO)
        });
    }

    /// Enable SDIO0 clock
    pub fn enable_sd0(&self) {
        self.enable_peripheral_clock(aper_clk::SDI0_EN);
    }

    /// Enable SDIO1 clock
    pub fn enable_sd1(&self) {
        self.enable_peripheral_clock(aper_clk::SDI1_EN);
    }

    /// Enable PCAP clock (required for PS-XADC interface)
    /// PCAP_CLK_CTRL at 0x168: DIVISOR[25:20], SRCSEL[5:4], CLKACT[0]
    pub fn enable_pcap(&self) {
        self.with_unlock(|s| {
            let val = s.base.offset(regs::PCAP_CLK_CTRL).read();
            if val & 1 == 0 {
                // PCAP clock not active — enable with IO PLL, divisor 5 (200MHz)
                s.base.offset(regs::PCAP_CLK_CTRL).write(
                    (5 << 20)  // DIVISOR = 5 (IO PLL 1000MHz / 5 = 200MHz)
                    | (0 << 4) // SRCSEL = IO PLL
                    | 1        // CLKACT = enabled
                );
            }
        });
    }

    /// Set SDIO Clock Control
    /// Bits 13:8 = DIVISOR (6 bits)
    /// Bits 5:4  = CLKSRC (00=IOPLL, 10=ARM, 11=DDR)
    /// Bit  0    = CLKACT
    pub fn set_sdio_clk_ctrl(&self, val: u32) {
        self.with_unlock(|s| {
            s.base.offset(regs::SDIO_CLK_CTRL).write(val);
        });
    }

    // =========================================================================
    // Reset Control
    // =========================================================================

    /// Reset FPGA fabric
    pub fn reset_fpga(&self) {
        self.with_unlock(|s| {
            s.base.offset(regs::FPGA_RST_CTRL).write(0xF);
            super::delay_us(10);
            s.base.offset(regs::FPGA_RST_CTRL).write(0x0);
        });
    }

    /// Get FPGA reset state
    pub fn get_fpga_reset(&self) -> u32 {
        self.base.offset(regs::FPGA_RST_CTRL).read()
    }

    // =========================================================================
    // MIO Pin Configuration
    // =========================================================================

    /// Configure MIO pin (0-53)
    ///
    /// # Arguments
    /// * `pin` - MIO pin number (0-53)
    /// * `config` - Pin configuration value (see TRM Table 4-6)
    pub fn configure_mio(&self, pin: u8, config: u32) {
        if pin > 53 { return; }
        self.with_unlock(|s| {
            s.base.offset(regs::MIO_PIN_00 + (pin as usize) * 4).write(config);
        });
    }

    /// Get MIO pin configuration
    pub fn get_mio_config(&self, pin: u8) -> u32 {
        if pin > 53 { return 0; }
        self.base.offset(regs::MIO_PIN_00 + (pin as usize) * 4).read()
    }

    // =========================================================================
    // FPGA Clock Control
    // =========================================================================

    /// Configure FCLK0 (PL clock 0)
    pub fn set_fclk0_divisor(&self, divisor0: u8, divisor1: u8) {
        self.with_unlock(|s| {
            let val = (1 << 20)  // SRCSEL = IO PLL
                | ((divisor0 as u32) << 8)
                | ((divisor1 as u32) << 20);
            s.base.offset(regs::FPGA0_CLK_CTRL).write(val);
        });
    }

    /// Get reboot status (why did we boot?)
    pub fn get_reboot_status(&self) -> u32 {
        self.base.offset(regs::REBOOT_STATUS).read()
    }
}

// MIO pin configuration helpers
pub mod mio {
    /// MIO pin config: L0_SEL (peripheral select)
    pub const fn l0_sel(sel: u32) -> u32 { sel & 0x1 }
    /// MIO pin config: L1_SEL
    pub const fn l1_sel(sel: u32) -> u32 { (sel & 0x1) << 1 }
    /// MIO pin config: L2_SEL
    pub const fn l2_sel(sel: u32) -> u32 { (sel & 0x3) << 2 }
    /// MIO pin config: L3_SEL
    pub const fn l3_sel(sel: u32) -> u32 { (sel & 0x7) << 5 }
    /// MIO pin config: Speed (0=fast, 1=slow)
    pub const fn speed(fast: bool) -> u32 { if fast { 0 } else { 1 << 8 } }
    /// MIO pin config: IO type (1=LVCMOS18, 2=LVCMOS25, 3=LVCMOS33)
    pub const fn io_type(t: u32) -> u32 { (t & 0x7) << 9 }
    /// MIO pin config: Pullup enable
    pub const fn pullup(en: bool) -> u32 { if en { 1 << 12 } else { 0 } }
    /// MIO pin config: Disable HSTL receiver
    pub const fn disable_rcvr(dis: bool) -> u32 { if dis { 1 << 13 } else { 0 } }

    /// LVCMOS33 with pullup
    pub const LVCMOS33_PULLUP: u32 = io_type(3) | pullup(true);
    /// LVCMOS33 output
    pub const LVCMOS33_OUT: u32 = io_type(3);
    /// I2C config (open drain, pullup)
    pub const I2C: u32 = io_type(3) | pullup(true) | l3_sel(0);
    /// SPI config
    pub const SPI: u32 = io_type(3) | l3_sel(0);
    /// GPIO config
    pub const GPIO: u32 = io_type(3);
}
