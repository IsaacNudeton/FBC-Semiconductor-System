//! Sonoma FPGA Register Definitions
//!
//! Bare metal register access for Zynq 7020 FPGA peripherals.
//! Generated from kzhang_v2 Verilog source.

#![no_std]

use core::ptr::{read_volatile, write_volatile};

// =============================================================================
// Base Addresses
// =============================================================================

pub const AXI_IO_TABLE_BASE: usize      = 0x4004_0000;
pub const AXI_PULSE_CTRL_BASE: usize    = 0x4005_0000;
pub const AXI_FREQ_COUNTER_BASE: usize  = 0x4006_0000;
pub const AXI_VECTOR_STATUS_BASE: usize = 0x4007_0000;

// =============================================================================
// Constants
// =============================================================================

pub const FPGA_VERSION: u32     = 0x0916_010F;
pub const PIN_COUNT: usize      = 160;
pub const VECTOR_WIDTH: usize   = 128;
pub const EXTRA_GPIO_WIDTH: usize = 32;
pub const MAX_ERROR_COUNT: usize = 1024;
pub const NUM_FREQ_COUNTERS: usize = 8;

// =============================================================================
// Pin Types
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PinType {
    /// Bidirectional pin {1,0,H,L} - default type
    Bidi = 0,
    /// Input only pin {X,X,H,L}
    Input = 1,
    /// Output only pin {1,0,Z,Z} - oen always 0
    Output = 2,
    /// Open collector pin {1,0,X,L}
    OpenCollector = 3,
    /// Positive pulse pin {posedge T/4, negedge 3T/4}
    Pulse = 4,
    /// Negative pulse pin (inverted pulse)
    NPulse = 5,
    /// Error trigger output (scope trigger)
    ErrorTrig = 6,
    /// Vector clock output
    VecClk = 7,
    /// Vector clock enable output
    VecClkEn = 8,
}

impl From<u8> for PinType {
    fn from(val: u8) -> Self {
        match val & 0xF {
            0 => PinType::Bidi,
            1 => PinType::Input,
            2 => PinType::Output,
            3 => PinType::OpenCollector,
            4 => PinType::Pulse,
            5 => PinType::NPulse,
            6 => PinType::ErrorTrig,
            7 => PinType::VecClk,
            8 => PinType::VecClkEn,
            _ => PinType::Bidi,
        }
    }
}

// =============================================================================
// Frequency Counter
// =============================================================================

/// Pin select values for frequency counter
#[derive(Clone, Copy, Debug)]
pub enum PinSelect {
    /// Vector pin 0-127
    VectorPin(u8),
    /// Extra GPIO pin 0-31
    ExtraGpio(u8),
    /// Immediate trigger (no pin wait)
    Immediate,
    /// Disabled
    Disabled,
}

impl PinSelect {
    pub fn to_u8(self) -> u8 {
        match self {
            PinSelect::VectorPin(n) => n.min(127),
            PinSelect::ExtraGpio(n) => 128 + n.min(31),
            PinSelect::Immediate => 0xFE,
            PinSelect::Disabled => 0xFF,
        }
    }

    pub fn from_u8(val: u8) -> Self {
        match val {
            0..=127 => PinSelect::VectorPin(val),
            128..=159 => PinSelect::ExtraGpio(val - 128),
            0xFE => PinSelect::Immediate,
            _ => PinSelect::Disabled,
        }
    }
}

/// Frequency counter control register
#[derive(Clone, Copy, Debug, Default)]
pub struct FreqControlReg {
    pub enable: bool,
    pub irq_enable: bool,
    pub signal_select: u8,
    pub trigger_select: u8,
}

impl FreqControlReg {
    pub fn to_u32(self) -> u32 {
        let mut val = 0u32;
        if self.enable { val |= 1 << 0; }
        if self.irq_enable { val |= 1 << 1; }
        val |= (self.signal_select as u32) << 8;
        val |= (self.trigger_select as u32) << 16;
        val |= 0x01 << 24; // Default upper byte
        val
    }

    pub fn from_u32(val: u32) -> Self {
        Self {
            enable: (val & (1 << 0)) != 0,
            irq_enable: (val & (1 << 1)) != 0,
            signal_select: ((val >> 8) & 0xFF) as u8,
            trigger_select: ((val >> 16) & 0xFF) as u8,
        }
    }
}

/// Frequency counter status register (read-only)
#[derive(Clone, Copy, Debug, Default)]
pub struct FreqStatusReg {
    pub done: bool,
    pub idle: bool,
    pub waiting: bool,
    pub running: bool,
    pub irq_en_shadow: bool,
    pub invalid_test: bool,
    pub signal_timeout: bool,
    pub timeout: bool,
    pub signal_select_shadow: u8,
    pub trigger_select_shadow: u8,
}

impl FreqStatusReg {
    pub fn from_u32(val: u32) -> Self {
        Self {
            done: (val & (1 << 0)) != 0,
            idle: (val & (1 << 1)) != 0,
            waiting: (val & (1 << 2)) != 0,
            running: (val & (1 << 3)) != 0,
            irq_en_shadow: (val & (1 << 4)) != 0,
            invalid_test: (val & (1 << 5)) != 0,
            signal_timeout: (val & (1 << 6)) != 0,
            timeout: (val & (1 << 7)) != 0,
            signal_select_shadow: ((val >> 8) & 0xFF) as u8,
            trigger_select_shadow: ((val >> 16) & 0xFF) as u8,
        }
    }
}

/// Frequency counter state machine states
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreqState {
    Idle,
    Waiting,
    Running,
    Done,
    Unknown,
}

impl FreqStatusReg {
    pub fn state(&self) -> FreqState {
        if self.done { FreqState::Done }
        else if self.running { FreqState::Running }
        else if self.waiting { FreqState::Waiting }
        else if self.idle { FreqState::Idle }
        else { FreqState::Unknown }
    }
}

/// Vector status register
#[derive(Clone, Copy, Debug, Default)]
pub struct VectorStatusReg {
    pub repeat_count_done: bool,
    pub errors_detected: bool,
}

impl VectorStatusReg {
    pub fn from_u32(val: u32) -> Self {
        Self {
            repeat_count_done: (val & (1 << 0)) != 0,
            errors_detected: (val & (1 << 1)) != 0,
        }
    }
}

// =============================================================================
// Register Access Structures
// =============================================================================

/// IO Table peripheral
pub struct IoTable {
    base: usize,
}

impl IoTable {
    pub const fn new() -> Self {
        Self { base: AXI_IO_TABLE_BASE }
    }

    /// Set pin type for a specific pin (0-127)
    pub fn set_pin_type(&self, pin: u8, pin_type: PinType) {
        if pin >= VECTOR_WIDTH as u8 { return; }

        let reg_idx = (pin / 8) as usize;
        let bit_offset = (pin % 8) * 4;
        let addr = self.base + reg_idx * 4;

        unsafe {
            let mut val = read_volatile(addr as *const u32);
            val &= !(0xF << bit_offset);
            val |= (pin_type as u32) << bit_offset;
            write_volatile(addr as *mut u32, val);
        }
    }

    /// Get pin type for a specific pin (0-127)
    pub fn get_pin_type(&self, pin: u8) -> PinType {
        if pin >= VECTOR_WIDTH as u8 { return PinType::Bidi; }

        let reg_idx = (pin / 8) as usize;
        let bit_offset = (pin % 8) * 4;
        let addr = self.base + reg_idx * 4;

        unsafe {
            let val = read_volatile(addr as *const u32);
            PinType::from(((val >> bit_offset) & 0xF) as u8)
        }
    }

    /// Set delay0 register
    pub fn set_delay0(&self, val: u32) {
        unsafe { write_volatile((self.base + 0x40) as *mut u32, val); }
    }

    /// Set delay1 register
    pub fn set_delay1(&self, val: u32) {
        unsafe { write_volatile((self.base + 0x44) as *mut u32, val); }
    }

    /// Get delay0 register
    pub fn get_delay0(&self) -> u32 {
        unsafe { read_volatile((self.base + 0x40) as *const u32) }
    }

    /// Get delay1 register
    pub fn get_delay1(&self) -> u32 {
        unsafe { read_volatile((self.base + 0x44) as *const u32) }
    }
}

/// Pulse Control peripheral
pub struct PulseCtrl {
    base: usize,
}

impl PulseCtrl {
    pub const fn new() -> Self {
        Self { base: AXI_PULSE_CTRL_BASE }
    }

    /// Write pulse control register (0-63)
    pub fn write(&self, reg: u8, val: u32) {
        if reg >= 64 { return; }
        let addr = self.base + (reg as usize) * 4;
        unsafe { write_volatile(addr as *mut u32, val); }
    }

    /// Read pulse control register (0-63)
    pub fn read(&self, reg: u8) -> u32 {
        if reg >= 64 { return 0; }
        let addr = self.base + (reg as usize) * 4;
        unsafe { read_volatile(addr as *const u32) }
    }
}

/// Single frequency counter instance
pub struct FreqCounter {
    base: usize,
}

impl FreqCounter {
    const fn counter_base(counter_idx: usize) -> usize {
        AXI_FREQ_COUNTER_BASE + counter_idx * 0x20
    }

    pub const fn new(counter_idx: usize) -> Self {
        Self { base: Self::counter_base(counter_idx) }
    }

    /// Write control register
    pub fn set_control(&self, ctrl: FreqControlReg) {
        unsafe { write_volatile(self.base as *mut u32, ctrl.to_u32()); }
    }

    /// Read control register
    pub fn get_control(&self) -> FreqControlReg {
        unsafe { FreqControlReg::from_u32(read_volatile(self.base as *const u32)) }
    }

    /// Read status register
    pub fn get_status(&self) -> FreqStatusReg {
        unsafe { FreqStatusReg::from_u32(read_volatile((self.base + 0x04) as *const u32)) }
    }

    /// Set max cycle count
    pub fn set_max_cycle_count(&self, val: u32) {
        unsafe { write_volatile((self.base + 0x08) as *mut u32, val); }
    }

    /// Set max time count
    pub fn set_max_time_count(&self, val: u32) {
        unsafe { write_volatile((self.base + 0x0C) as *mut u32, val); }
    }

    /// Read current cycle count
    pub fn get_cycle_count(&self) -> u32 {
        unsafe { read_volatile((self.base + 0x10) as *const u32) }
    }

    /// Read current time count
    pub fn get_time_count(&self) -> u32 {
        unsafe { read_volatile((self.base + 0x14) as *const u32) }
    }

    /// Set timeout threshold
    pub fn set_max_timeout(&self, val: u32) {
        unsafe { write_volatile((self.base + 0x18) as *mut u32, val); }
    }

    /// Enable counter with specified signal and trigger pins
    pub fn enable(&self, signal: PinSelect, trigger: PinSelect, irq: bool) {
        let ctrl = FreqControlReg {
            enable: true,
            irq_enable: irq,
            signal_select: signal.to_u8(),
            trigger_select: trigger.to_u8(),
        };
        self.set_control(ctrl);
    }

    /// Disable counter
    pub fn disable(&self) {
        let mut ctrl = self.get_control();
        ctrl.enable = false;
        self.set_control(ctrl);
    }

    /// Check if measurement is complete
    pub fn is_done(&self) -> bool {
        self.get_status().done
    }

    /// Wait for measurement to complete (blocking)
    pub fn wait_done(&self) {
        while !self.is_done() {
            core::hint::spin_loop();
        }
    }
}

/// Vector Status peripheral
pub struct VectorStatus {
    base: usize,
}

impl VectorStatus {
    pub const fn new() -> Self {
        Self { base: AXI_VECTOR_STATUS_BASE }
    }

    /// Read final error count
    pub fn get_error_count(&self) -> u32 {
        unsafe { read_volatile(self.base as *const u32) }
    }

    /// Read final vector count
    pub fn get_vector_count(&self) -> u32 {
        unsafe { read_volatile((self.base + 0x04) as *const u32) }
    }

    /// Read final cycle count (64-bit)
    pub fn get_cycle_count(&self) -> u64 {
        unsafe {
            let lo = read_volatile((self.base + 0x08) as *const u32) as u64;
            let hi = read_volatile((self.base + 0x0C) as *const u32) as u64;
            (hi << 32) | lo
        }
    }

    /// Read gap count
    pub fn get_gap_count(&self) -> u32 {
        unsafe { read_volatile((self.base + 0x10) as *const u32) }
    }

    /// Read status register
    pub fn get_status(&self) -> VectorStatusReg {
        unsafe { VectorStatusReg::from_u32(read_volatile((self.base + 0x14) as *const u32)) }
    }

    /// Set IRQ enable
    pub fn set_irq_enable(&self, enable: bool) {
        let val = if enable { 1u32 } else { 0u32 };
        unsafe { write_volatile((self.base + 0x18) as *mut u32, val); }
    }

    /// Get IRQ enable state
    pub fn get_irq_enable(&self) -> bool {
        unsafe { read_volatile((self.base + 0x18) as *const u32) & 1 != 0 }
    }

    /// Read FPGA version
    pub fn get_fpga_version(&self) -> u32 {
        unsafe { read_volatile((self.base + 0x3C) as *const u32) }
    }

    /// Check if vector execution is complete
    pub fn is_done(&self) -> bool {
        self.get_status().repeat_count_done
    }

    /// Check if errors were detected
    pub fn has_errors(&self) -> bool {
        self.get_status().errors_detected
    }
}

// =============================================================================
// Global Peripheral Instances
// =============================================================================

/// Global IO Table peripheral instance
pub static IO_TABLE: IoTable = IoTable::new();

/// Global Pulse Control peripheral instance
pub static PULSE_CTRL: PulseCtrl = PulseCtrl::new();

/// Global Vector Status peripheral instance
pub static VECTOR_STATUS: VectorStatus = VectorStatus::new();

/// Frequency counter instances
pub static FREQ_COUNTER: [FreqCounter; 8] = [
    FreqCounter::new(0),
    FreqCounter::new(1),
    FreqCounter::new(2),
    FreqCounter::new(3),
    FreqCounter::new(4),
    FreqCounter::new(5),
    FreqCounter::new(6),
    FreqCounter::new(7),
];

// =============================================================================
// Convenience Functions
// =============================================================================

/// Read FPGA version
pub fn fpga_version() -> u32 {
    VECTOR_STATUS.get_fpga_version()
}

/// Set all pins to a specific type
pub fn set_all_pins(pin_type: PinType) {
    for pin in 0..VECTOR_WIDTH as u8 {
        IO_TABLE.set_pin_type(pin, pin_type);
    }
}

/// Configure a frequency measurement
pub fn measure_frequency(
    counter_idx: usize,
    signal_pin: PinSelect,
    trigger_pin: PinSelect,
    max_cycles: u32,
    timeout_cycles: u32,
) -> Option<(u32, u32)> {
    if counter_idx >= NUM_FREQ_COUNTERS { return None; }

    let counter = &FREQ_COUNTER[counter_idx];

    // Configure counter
    counter.set_max_cycle_count(max_cycles);
    counter.set_max_time_count(0xFFFF_FFFF);
    counter.set_max_timeout(timeout_cycles);

    // Start measurement
    counter.enable(signal_pin, trigger_pin, false);

    // Wait for completion
    counter.wait_done();

    // Check for errors
    let status = counter.get_status();
    if status.timeout || status.invalid_test {
        counter.disable();
        return None;
    }

    // Read results
    let cycles = counter.get_cycle_count();
    let time = counter.get_time_count();

    // Disable counter
    counter.disable();

    Some((cycles, time))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_type_conversion() {
        assert_eq!(PinType::from(0), PinType::Bidi);
        assert_eq!(PinType::from(4), PinType::Pulse);
        assert_eq!(PinType::from(15), PinType::Bidi); // Invalid maps to default
    }

    #[test]
    fn test_freq_control_reg() {
        let ctrl = FreqControlReg {
            enable: true,
            irq_enable: false,
            signal_select: 0x10,
            trigger_select: 0xFE,
        };
        let val = ctrl.to_u32();
        let ctrl2 = FreqControlReg::from_u32(val);
        assert_eq!(ctrl.enable, ctrl2.enable);
        assert_eq!(ctrl.signal_select, ctrl2.signal_select);
        assert_eq!(ctrl.trigger_select, ctrl2.trigger_select);
    }
}
