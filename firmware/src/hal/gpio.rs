//! GPIO (MIO) Driver
//!
//! Controls the MIO pins on the Zynq PS.
//! Used for ADC mux select, core enables, and other control signals.

use super::{Reg, Register};

/// GPIO base address
const GPIO_BASE: usize = 0xE000_A000;

/// GPIO register offsets
mod regs {
    // Masked data registers (atomic read-modify-write)
    pub const MASK_DATA_0_LSW: usize = 0x000;  // Bank 0 bits 15:0
    pub const MASK_DATA_0_MSW: usize = 0x004;  // Bank 0 bits 31:16
    pub const MASK_DATA_1_LSW: usize = 0x008;  // Bank 1 bits 15:0
    pub const MASK_DATA_1_MSW: usize = 0x00C;  // Bank 1 bits 31:16
    pub const MASK_DATA_2_LSW: usize = 0x010;  // Bank 2 bits 15:0
    pub const MASK_DATA_2_MSW: usize = 0x014;  // Bank 2 bits 21:16 (EMIO)
    pub const MASK_DATA_3_LSW: usize = 0x018;  // Bank 3 bits 15:0 (EMIO)
    pub const MASK_DATA_3_MSW: usize = 0x01C;  // Bank 3 bits 31:16 (EMIO)

    // Data registers (direct read/write)
    pub const DATA_0: usize = 0x040;  // Bank 0 (MIO 0-31)
    pub const DATA_1: usize = 0x044;  // Bank 1 (MIO 32-53)
    pub const DATA_2: usize = 0x048;  // Bank 2 (EMIO 0-21)
    pub const DATA_3: usize = 0x04C;  // Bank 3 (EMIO 22-53)

    // Data read-only registers
    pub const DATA_RO_0: usize = 0x060;
    pub const DATA_RO_1: usize = 0x064;
    pub const DATA_RO_2: usize = 0x068;
    pub const DATA_RO_3: usize = 0x06C;

    // Direction registers
    pub const DIRM_0: usize = 0x204;  // Direction mode Bank 0
    pub const OEN_0: usize = 0x208;   // Output enable Bank 0
    pub const DIRM_1: usize = 0x244;  // Direction mode Bank 1
    pub const OEN_1: usize = 0x248;   // Output enable Bank 1
    pub const DIRM_2: usize = 0x284;  // Direction mode Bank 2 (EMIO)
    pub const OEN_2: usize = 0x288;   // Output enable Bank 2 (EMIO)
    pub const DIRM_3: usize = 0x2C4;  // Direction mode Bank 3 (EMIO)
    pub const OEN_3: usize = 0x2C8;   // Output enable Bank 3 (EMIO)
}

/// Known MIO pin assignments (from HPBI Controller schematic OP009-001-SCH)
/// ACTIVE ACTIVE ACTIVE - ACTIVE ACTIVE ACTIVE - ACTIVE ACTIVE ACTIVE - ACTIVE ACTIVE
pub mod mio_pins {
    // =========================================================================
    // ACTIVE ACTIVE ACTIVE - ACTIVE ACTIVE from schematic 2024-02-12:
    // =========================================================================
    pub const STATUS_LED: u8 = 0;    // MIO0  = Ball G6 = STATUS_LED (active high)
    pub const PHY_RESET: u8 = 11;    // MIO11 = Ball B4 = PHY_RESET_B_AND (active low)

    // UART1 for console (directly verified):
    pub const CONSOLE_UART_RX: u8 = 48;  // MIO48 = Ball D11 = CONSOLE_UART_RX
    pub const CONSOLE_UART_TX: u8 = 49;  // MIO49 = Ball C14 = CONSOLE_UART_TX

    // =========================================================================
    // Other pins from schematic:
    // =========================================================================
    pub const DEV_ID0: u8 = 7;       // MIO7  = Ball D5 = DEV_ID0
    pub const DEV_ID1: u8 = 8;       // MIO8  = Ball E5 = DEV_ID1
    pub const SMBUS_ALERT_N: u8 = 9; // MIO9  = Ball C4 = SMBUS_ALERT_N (input)
    pub const DUT_PRESENT_N: u8 = 10;// MIO10 = Ball G7 = DUT_PRESENT_N (input)

    // I2C1:
    pub const I2C1_SCL: u8 = 12;     // MIO12 = Ball C5 = I2C1_SCL/MIO12
    pub const I2C1_SDA: u8 = 13;     // MIO13 = Ball A6 = I2C1_SDA/MIO13

    // DAC/ADC:
    pub const DAC_LD: u8 = 14;       // MIO14 = Ball B6 = DAC_LD
    pub const ADC_EOC: u8 = 15;      // MIO15 = Ball F6 = ADC_EOC/MIO15
    pub const DAC_RESET: u8 = 47;    // MIO47 = Ball B10 = DAC_RESET/MIO47

    // I2C0:
    pub const I2C0_SCL: u8 = 50;     // MIO50 = Ball D13 = I2C0_SCL
    pub const I2C0_SDA: u8 = 51;     // MIO51 = Ball C10 = I2C0_SDA

    // Ethernet MDIO:
    pub const ETH_MDC: u8 = 52;      // MIO52 = Ball D10 = ETH_MDC
    pub const ETH_MDIO: u8 = 53;     // MIO53 = Ball C12 = ETH_MDIO
}

/// MIO pin abstraction
#[derive(Clone, Copy)]
pub struct MioPin {
    pub pin: u8,
}

impl MioPin {
    pub const fn new(pin: u8) -> Self {
        Self { pin }
    }

    /// Get bank number (0 or 1 for MIO)
    pub const fn bank(&self) -> u8 {
        if self.pin < 32 { 0 } else { 1 }
    }

    /// Get bit within bank
    pub const fn bit(&self) -> u8 {
        self.pin % 32
    }
}

/// GPIO controller
pub struct Gpio {
    base: Reg,
}

impl Gpio {
    /// Create GPIO instance
    pub const fn new() -> Self {
        Self { base: Reg::new(GPIO_BASE) }
    }

    /// Set pin direction
    ///
    /// # Arguments
    /// * `pin` - MIO pin number (0-53)
    /// * `output` - true for output, false for input
    pub fn set_direction(&self, pin: MioPin, output: bool) {
        if pin.pin > 53 { return; }

        let (dirm_offset, oen_offset) = if pin.bank() == 0 {
            (regs::DIRM_0, regs::OEN_0)
        } else {
            (regs::DIRM_1, regs::OEN_1)
        };

        let mask = 1u32 << pin.bit();

        if output {
            self.base.offset(dirm_offset).set_bits(mask);
            self.base.offset(oen_offset).set_bits(mask);
        } else {
            self.base.offset(dirm_offset).clear_bits(mask);
            self.base.offset(oen_offset).clear_bits(mask);
        }
    }

    /// Configure pin as output
    pub fn set_output(&self, pin: MioPin) {
        self.set_direction(pin, true);
    }

    /// Configure pin as input
    pub fn set_input(&self, pin: MioPin) {
        self.set_direction(pin, false);
    }

    /// Write pin value (must be configured as output)
    pub fn write_pin(&self, pin: MioPin, high: bool) {
        if pin.pin > 53 { return; }

        // Use masked write for atomic operation
        let (offset, shift) = if pin.bank() == 0 {
            if pin.bit() < 16 {
                (regs::MASK_DATA_0_LSW, pin.bit())
            } else {
                (regs::MASK_DATA_0_MSW, pin.bit() - 16)
            }
        } else {
            let bit = pin.pin - 32;
            if bit < 16 {
                (regs::MASK_DATA_1_LSW, bit)
            } else {
                (regs::MASK_DATA_1_MSW, bit - 16)
            }
        };

        // Masked write format: [31:16] = mask (1 = don't modify), [15:0] = data
        let mask = !(1u32 << shift) << 16;  // Clear the mask bit to enable write
        let data = if high { 1u32 << shift } else { 0 };

        self.base.offset(offset).write(mask | data);
    }

    /// Set pin high
    pub fn set_high(&self, pin: MioPin) {
        self.write_pin(pin, true);
    }

    /// Set pin low
    pub fn set_low(&self, pin: MioPin) {
        self.write_pin(pin, false);
    }

    /// Toggle pin
    pub fn toggle(&self, pin: MioPin) {
        let current = self.read_pin(pin);
        self.write_pin(pin, !current);
    }

    /// Read pin value
    pub fn read_pin(&self, pin: MioPin) -> bool {
        if pin.pin > 53 { return false; }

        let offset = if pin.bank() == 0 {
            regs::DATA_RO_0
        } else {
            regs::DATA_RO_1
        };

        self.base.offset(offset).read() & (1 << pin.bit()) != 0
    }

    /// Write entire bank (32 bits)
    pub fn write_bank(&self, bank: u8, value: u32) {
        let offset = match bank {
            0 => regs::DATA_0,
            1 => regs::DATA_1,
            2 => regs::DATA_2,
            3 => regs::DATA_3,
            _ => return,
        };
        self.base.offset(offset).write(value);
    }

    /// Read entire bank (32 bits)
    pub fn read_bank(&self, bank: u8) -> u32 {
        let offset = match bank {
            0 => regs::DATA_RO_0,
            1 => regs::DATA_RO_1,
            2 => regs::DATA_RO_2,
            3 => regs::DATA_RO_3,
            _ => return 0,
        };
        self.base.offset(offset).read()
    }

    // =========================================================================
    // Sonoma-specific helpers - DISABLED pending schematic verification
    // =========================================================================
    // TODO: These functions need correct MIO pin mappings from schematic.
    // ADC mux and CORE enables may be controlled via DAC/EMIO, not MIO GPIO.
    //
    // pub fn set_adc_mux(&self, value: bool) { ... }
    // pub fn set_core_enable(&self, core: u8, enable: bool) { ... }
    // pub fn enable_all_cores(&self) { ... }
    // pub fn disable_all_cores(&self) { ... }

    // =========================================================================
    // Status LED (MIO0 per schematic)
    // =========================================================================

    /// Initialize status LED pin as output (safe - has current limiting resistor on PCB)
    pub fn init_status_led(&self) {
        let pin = MioPin::new(mio_pins::STATUS_LED);
        self.set_output(pin);
        self.write_pin(pin, false);  // Start off
    }

    /// Set status LED state
    pub fn set_status_led(&self, on: bool) {
        let pin = MioPin::new(mio_pins::STATUS_LED);
        self.write_pin(pin, on);
    }

    /// Toggle status LED
    pub fn toggle_status_led(&self) {
        let pin = MioPin::new(mio_pins::STATUS_LED);
        self.toggle(pin);
    }

    // =========================================================================
    // PHY Reset (for Ethernet) - MIO11 per schematic
    // =========================================================================

    /// Reset the Ethernet PHY
    ///
    /// The PHY requires a hardware reset before MDIO communication works.
    /// PHY_RESET_B_AND is active low, active through AND gate with POWER_GOOD.
    /// This toggles MIO11 low for 10ms, then high, then waits 100ms
    /// for the PHY to complete internal initialization.
    pub fn reset_phy(&self) {
        let pin = MioPin::new(mio_pins::PHY_RESET);  // MIO11

        // Configure as output
        self.set_output(pin);

        // Assert reset (active low)
        self.write_pin(pin, false);
        super::delay_ms(10);

        // Release reset
        self.write_pin(pin, true);

        // Wait for PHY to initialize (datasheet says 100ms typical)
        super::delay_ms(100);
    }
}
