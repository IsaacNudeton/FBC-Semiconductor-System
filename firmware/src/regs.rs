//! FPGA Register Definitions
//!
//! Memory-mapped register access for FBC FPGA peripherals.

use core::ptr::{read_volatile, write_volatile};

// =============================================================================
// Base Addresses
// =============================================================================

pub const FBC_CTRL_BASE: usize    = 0x4004_0000;
pub const PIN_CTRL_BASE: usize    = 0x4005_0000;
pub const STATUS_BASE: usize      = 0x4006_0000;
pub const FREQ_COUNTER_BASE: usize = 0x4007_0000;
pub const CLK_CTRL_BASE: usize    = 0x4008_0000;
pub const ERROR_BRAM_BASE: usize  = 0x4009_0000;
pub const DNA_BASE: usize         = 0x400A_0000;

// =============================================================================
// FBC Control Peripheral
// =============================================================================

/// FBC decoder control interface
pub struct FbcCtrl {
    base: usize,
}

impl FbcCtrl {
    pub const fn new() -> Self {
        Self { base: FBC_CTRL_BASE }
    }

    /// Enable the FBC decoder
    pub fn enable(&self) {
        self.write_ctrl(self.read_ctrl() | 0x01);
    }

    /// Disable the FBC decoder
    pub fn disable(&self) {
        self.write_ctrl(self.read_ctrl() & !0x01);
    }

    /// Reset the FBC decoder
    pub fn reset(&self) {
        self.write_ctrl(self.read_ctrl() | 0x02);
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.read_status() & 0x01 != 0
    }

    /// Check if done
    pub fn is_done(&self) -> bool {
        self.read_status() & 0x02 != 0
    }

    /// Check if error occurred
    pub fn has_error(&self) -> bool {
        self.read_status() & 0x04 != 0
    }

    /// Get instruction count
    pub fn get_instr_count(&self) -> u32 {
        self.read_reg(0x08)
    }

    /// Get cycle count
    pub fn get_cycle_count(&self) -> u64 {
        let lo = self.read_reg(0x10) as u64;
        let hi = self.read_reg(0x14) as u64;
        (hi << 32) | lo
    }

    /// Get version
    pub fn get_version(&self) -> u32 {
        self.read_reg(0x1C)
    }

    /// Get error register (bit 0 = error flag, same as has_error())
    /// NOTE: RTL currently only stores the flag. To get useful error info
    /// (first failing vector/cycle), check error_counter.v or VectorStatus.
    pub fn get_error_raw(&self) -> u32 {
        self.read_reg(0x18)
    }

    /// Enable interrupts from FBC decoder to Cortex-A9 GIC
    /// Sets CTRL register bits 2 (irq_done) and 3 (irq_error)
    pub fn enable_irq(&self) {
        let ctrl = self.read_ctrl();
        // Enable both done and error interrupts
        self.write_ctrl(ctrl | 0x04 | 0x08);
    }

    /// Disable interrupts from FBC decoder
    pub fn disable_irq(&self) {
        let ctrl = self.read_ctrl();
        self.write_ctrl(ctrl & !0x04 & !0x08);
    }

    // =========================================================================
    // Fast Pins (gpio[128:159] - direct FPGA control, 1-cycle latency)
    // =========================================================================

    /// Read fast pins output data register
    pub fn read_fast_dout(&self) -> u32 {
        self.read_reg(0x20)
    }

    /// Write fast pins output data register
    pub fn write_fast_dout(&self, val: u32) {
        self.write_reg(0x20, val)
    }

    /// Read fast pins output enable register
    pub fn read_fast_oen(&self) -> u32 {
        self.read_reg(0x24)
    }

    /// Write fast pins output enable register
    pub fn write_fast_oen(&self, val: u32) {
        self.write_reg(0x24, val)
    }

    /// Read fast pins input data register (directly from pins)
    pub fn read_fast_din(&self) -> u32 {
        self.read_reg(0x28)
    }

    /// Read fast pins error flags (which fast pins had compare errors)
    pub fn read_fast_error(&self) -> u32 {
        self.read_reg(0x2C)
    }

    // Internal helpers
    fn read_ctrl(&self) -> u32 { self.read_reg(0x00) }
    fn write_ctrl(&self, val: u32) { self.write_reg(0x00, val) }
    fn read_status(&self) -> u32 { self.read_reg(0x04) }

    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }

    fn write_reg(&self, offset: usize, val: u32) {
        unsafe { write_volatile((self.base + offset) as *mut u32, val) }
    }
}

// =============================================================================
// Pin Control Peripheral
// =============================================================================

/// Pin type configuration
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PinType {
    Bidi = 0,
    Input = 1,
    Output = 2,
    OpenCollector = 3,
    Pulse = 4,
    NPulse = 5,
    ErrorTrig = 6,
    VecClk = 7,
    VecClkEn = 8,
}

/// Pin control interface
///
/// Supports 160 pins in 3 groups:
/// - gpio[0:127]: BIM pins (Banks 13/33/34, through Quad Board, 2-cycle latency)
/// - gpio[128:145]: Fast pins (Bank 35, direct FPGA via J3, 1-cycle latency)
/// - gpio[146:159]: Reserved/spare (14 pins)
///
/// Bank 35 special pins (discovered via ONETWO schematic analysis):
/// - gpio[136:137]: SYSCLK0 (clock-capable LVDS pair, external clock/sync)
/// - gpio[130:131]: LVDS04 (differential pair, multi-board sync)
pub struct PinCtrl {
    base: usize,
}

/// Pin count constants
pub const BIM_PIN_COUNT: u8 = 128;
pub const FAST_PIN_COUNT: u8 = 32;
pub const TOTAL_PIN_COUNT: u8 = 160;

/// Bank 35 - Direct FPGA pins (no BIM, all 32 available)
pub const BANK35_START: u8 = 128;
pub const BANK35_END: u8 = 159;
pub const BANK35_COUNT: u8 = 32;

/// Special purpose Fast pins
pub const FAST_SCOPE_TRIG: u8 = 128;
pub const FAST_ERROR_STROBE: u8 = 129;
pub const FAST_SYNC_N: u8 = 130;
pub const FAST_SYNC_P: u8 = 131;
pub const FAST_SYSCLK_N: u8 = 136;
pub const FAST_SYSCLK_P: u8 = 137;

/// Pin type register offset (20 registers for 160 pins)
const PIN_TYPE_OFFSET: usize = 0x000;
/// Pulse timing register offset (80 registers for 160 pins)
const PULSE_CTRL_OFFSET: usize = 0x200;

impl PinCtrl {
    pub const fn new() -> Self {
        Self { base: PIN_CTRL_BASE }
    }

    /// Set pin type for a specific pin (0-159)
    pub fn set_pin_type(&self, pin: u8, pin_type: PinType) {
        if pin >= TOTAL_PIN_COUNT { return; }

        let reg_idx = (pin / 8) as usize;
        let bit_offset = (pin % 8) * 4;
        let addr = self.base + PIN_TYPE_OFFSET + reg_idx * 4;

        unsafe {
            let mut val = read_volatile(addr as *const u32);
            val &= !(0xF << bit_offset);
            val |= (pin_type as u32) << bit_offset;
            write_volatile(addr as *mut u32, val);
        }
    }

    /// Get pin type for a specific pin (0-159)
    pub fn get_pin_type(&self, pin: u8) -> PinType {
        if pin >= TOTAL_PIN_COUNT { return PinType::Bidi; }

        let reg_idx = (pin / 8) as usize;
        let bit_offset = (pin % 8) * 4;
        let addr = self.base + PIN_TYPE_OFFSET + reg_idx * 4;

        unsafe {
            let val = read_volatile(addr as *const u32);
            match ((val >> bit_offset) & 0xF) as u8 {
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

    /// Set pulse timing for a specific pin (0-159)
    /// start: vec_clk_cnt value when pulse starts
    /// end: vec_clk_cnt value when pulse ends
    pub fn set_pulse_timing(&self, pin: u8, start: u8, end: u8) {
        if pin >= TOTAL_PIN_COUNT { return; }

        let reg_idx = (pin / 2) as usize;
        let is_upper = (pin % 2) == 1;
        let addr = self.base + PULSE_CTRL_OFFSET + reg_idx * 4;

        let timing = ((start as u16) << 8) | (end as u16);

        unsafe {
            let mut val = read_volatile(addr as *const u32);
            if is_upper {
                val &= 0x0000_FFFF;
                val |= (timing as u32) << 16;
            } else {
                val &= 0xFFFF_0000;
                val |= timing as u32;
            }
            write_volatile(addr as *mut u32, val);
        }
    }

    /// Check if pin is a fast pin (128-159)
    pub fn is_fast_pin(pin: u8) -> bool {
        pin >= BIM_PIN_COUNT && pin < TOTAL_PIN_COUNT
    }

    /// Check if pin is a BIM pin (0-127)
    pub fn is_bim_pin(pin: u8) -> bool {
        pin < BIM_PIN_COUNT
    }
}

// =============================================================================
// Vector Status Peripheral
// =============================================================================

/// Vector execution status
pub struct VectorStatus {
    base: usize,
}

impl VectorStatus {
    pub const fn new() -> Self {
        Self { base: STATUS_BASE }
    }

    /// Get error count
    pub fn get_error_count(&self) -> u32 {
        self.read_reg(0x00)
    }

    /// Get vector count
    pub fn get_vector_count(&self) -> u32 {
        self.read_reg(0x04)
    }

    /// Get cycle count
    pub fn get_cycle_count(&self) -> u64 {
        let lo = self.read_reg(0x08) as u64;
        let hi = self.read_reg(0x0C) as u64;
        (hi << 32) | lo
    }

    /// Get FPGA version
    pub fn get_version(&self) -> u32 {
        self.read_reg(0x3C)
    }

    /// Check if done
    pub fn is_done(&self) -> bool {
        self.read_reg(0x14) & 0x01 != 0
    }

    /// Check if errors detected
    pub fn has_errors(&self) -> bool {
        self.read_reg(0x14) & 0x02 != 0
    }

    /// Get first error vector number (register 0x10)
    pub fn get_first_err_vec(&self) -> u32 {
        self.read_reg(0x10)
    }

    /// Check if first error info is valid (STATUS bit 29)
    pub fn first_error_valid(&self) -> bool {
        self.read_reg(0x14) & (1 << 29) != 0
    }

    /// Get first error cycle count (registers 0x18 + 0x1C)
    pub fn get_first_err_cycle(&self) -> u64 {
        let lo = self.read_reg(0x18) as u64;
        let hi = self.read_reg(0x1C) as u64;
        (hi << 32) | lo
    }

    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }
}

// =============================================================================
// Frequency Counter
// =============================================================================

/// Frequency counter instance
pub struct FreqCounter {
    base: usize,
}

impl FreqCounter {
    pub const fn new(index: usize) -> Self {
        Self {
            base: FREQ_COUNTER_BASE + index * 0x20,
        }
    }

    /// Enable counter
    pub fn enable(&self, signal_pin: u8, trigger_pin: u8, irq: bool) {
        let mut ctrl = 0u32;
        ctrl |= 1;  // enable
        if irq { ctrl |= 2; }
        ctrl |= (signal_pin as u32) << 8;
        ctrl |= (trigger_pin as u32) << 16;
        self.write_reg(0x00, ctrl);
    }

    /// Disable counter
    pub fn disable(&self) {
        self.write_reg(0x00, 0);
    }

    /// Check if done
    pub fn is_done(&self) -> bool {
        self.read_reg(0x04) & 0x01 != 0
    }

    /// Get cycle count
    pub fn get_cycle_count(&self) -> u32 {
        self.read_reg(0x10)
    }

    /// Get time count
    pub fn get_time_count(&self) -> u32 {
        self.read_reg(0x14)
    }

    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }

    fn write_reg(&self, offset: usize, val: u32) {
        unsafe { write_volatile((self.base + offset) as *mut u32, val) }
    }
}

// =============================================================================
// Clock Control Peripheral
// =============================================================================
//
// ONETWO Design: Pre-generate all needed frequencies, select via MUX.
// INVARIANT: Test plans use ~5 distinct frequencies (5, 10, 25, 50, 100 MHz)
// VARIES: Which frequency is selected for each test step
//
// This is BETTER than Sonoma's DRP approach:
//   - Switch time: <100ns (vs 100µs for PLL relock)
//   - Glitch-free: BUFGMUX guarantees clean transitions
//   - Simpler: 5 lines of RTL vs 50+ for DRP FSM
//

/// Vector clock frequency selection
///
/// Maps to pre-generated MMCM outputs via BUFGMUX
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VecClockFreq {
    /// 5 MHz (MASTER_PERIOD = 200ns)
    Mhz5 = 0,
    /// 10 MHz (MASTER_PERIOD = 100ns)
    Mhz10 = 1,
    /// 25 MHz (MASTER_PERIOD = 40ns)
    Mhz25 = 2,
    /// 50 MHz (MASTER_PERIOD = 20ns) - Default
    Mhz50 = 3,
    /// 100 MHz (MASTER_PERIOD = 10ns) - Fast debug
    Mhz100 = 4,
}

impl VecClockFreq {
    /// Convert frequency in Hz to enum variant
    pub fn from_hz(hz: u32) -> Self {
        match hz {
            0..=7_500_000 => VecClockFreq::Mhz5,
            7_500_001..=17_500_000 => VecClockFreq::Mhz10,
            17_500_001..=37_500_000 => VecClockFreq::Mhz25,
            37_500_001..=75_000_000 => VecClockFreq::Mhz50,
            _ => VecClockFreq::Mhz100,
        }
    }

    /// Convert enum variant to frequency in Hz
    pub fn to_hz(self) -> u32 {
        match self {
            VecClockFreq::Mhz5 => 5_000_000,
            VecClockFreq::Mhz10 => 10_000_000,
            VecClockFreq::Mhz25 => 25_000_000,
            VecClockFreq::Mhz50 => 50_000_000,
            VecClockFreq::Mhz100 => 100_000_000,
        }
    }

    /// Convert MASTER_PERIOD (ns) to frequency
    pub fn from_period_ns(period_ns: u32) -> Self {
        if period_ns == 0 {
            return VecClockFreq::Mhz50; // Default
        }
        let hz = 1_000_000_000 / period_ns;
        Self::from_hz(hz)
    }
}

/// Clock control peripheral
pub struct ClkCtrl {
    base: usize,
}

impl ClkCtrl {
    pub const fn new() -> Self {
        Self { base: CLK_CTRL_BASE }
    }

    /// Set vector clock frequency
    ///
    /// Set vector clock frequency — safe BUFGCE-gated switching
    ///
    /// Clock path: MMCM → BUFGMUX → BUFGCE → vec_clk → BRAMs
    /// Gate BUFGCE → switch BUFGMUX (glitch hidden) → release BUFGCE
    pub fn set_vec_clock(&self, freq: VecClockFreq) {
        // Read current value — skip write if already set (prevents any glitch)
        let current = self.read_reg(0x00) & 0x07;
        if current == freq as u32 {
            return; // Already at this frequency, no write needed
        }
        // Gate BUFGCE → switch BUFGMUX → release
        self.write_reg(0x08, 0);
        crate::delay_us(1000);
        self.write_reg(0x00, freq as u32);
        crate::delay_us(1000);
        self.write_reg(0x08, 1);
    }

    /// Check if clk_ctrl peripheral is accessible at 0x4008_0000.
    /// If this returns false, do NOT read or write any ClkCtrl register.
    /// Accessing a non-responsive AXI slave hangs the bus → Data Abort.
    pub fn is_accessible(&self) -> bool {
        // Read the MMCM lock status register (offset 0x04).
        // If the peripheral exists, this returns 0 or 1.
        // If it doesn't exist, the read hangs and we never return.
        // Use a volatile read with a known-safe register.
        //
        // WARNING: There's no timeout mechanism in hardware — if the
        // peripheral doesn't respond, the CPU hangs. This function is
        // only safe to call during boot with JTAG available for recovery.
        //
        // TODO: Use the Zynq AXI timeout mechanism or a watchdog timer
        // to detect non-responsive peripherals safely.
        //
        // v5 bitstream: dont_touch fixed read path (0x04 reads OK at boot).
        // But writes to 0x00 still crash — even writing the same value.
        // Read-before-write to skip same-value writes also crashes on the read.
        // Root cause still unknown. Default 50MHz works for all operations.
        false
    }

    /// Get current vector clock frequency
    pub fn get_vec_clock(&self) -> VecClockFreq {
        match self.read_reg(0x00) & 0x07 {
            0 => VecClockFreq::Mhz5,
            1 => VecClockFreq::Mhz10,
            2 => VecClockFreq::Mhz25,
            3 => VecClockFreq::Mhz50,
            4 => VecClockFreq::Mhz100,
            _ => VecClockFreq::Mhz50,
        }
    }

    /// Set vector clock frequency from Hz value
    ///
    /// Convenience method for direct use with FBC header vec_clock_hz
    pub fn set_vec_clock_hz(&self, hz: u32) {
        self.set_vec_clock(VecClockFreq::from_hz(hz));
    }

    /// Check if MMCM is locked
    pub fn is_locked(&self) -> bool {
        self.read_reg(0x04) & 0x01 != 0
    }

    /// Enable vector clock output
    pub fn enable(&self) {
        let val = self.read_reg(0x08);
        self.write_reg(0x08, val | 0x01);
    }

    /// Disable vector clock output (gated)
    pub fn disable(&self) {
        let val = self.read_reg(0x08);
        self.write_reg(0x08, val & !0x01);
    }

    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }

    fn write_reg(&self, offset: usize, val: u32) {
        unsafe { write_volatile((self.base + offset) as *mut u32, val) }
    }
}

// =============================================================================
// Error BRAM Peripheral (0x4009_0000)
// =============================================================================

/// Error BRAM read interface - captures error details from 3 BRAMs:
/// - Pattern BRAM: 128-bit error mask (4 × 32-bit)
/// - Vector BRAM: vector number when error occurred
/// - Cycle BRAM: cycle count when error occurred
pub struct ErrorBram {
    base: usize,
}

impl ErrorBram {
    pub const BASE: usize = ERROR_BRAM_BASE;

    pub fn new() -> Self {
        Self { base: Self::BASE }
    }

    /// Set read index for BRAM access
    pub fn set_read_index(&self, idx: u32) {
        unsafe { write_volatile(self.base as *mut u32, idx); }
    }

    /// Read 128-bit pattern value (4 × 32-bit words)
    pub fn read_pattern(&self) -> [u32; 4] {
        let mut pattern = [0u32; 4];
        for i in 0..4 {
            pattern[i] = unsafe { read_volatile((self.base + 0x04 + (i as usize) * 4) as *const u32) };
        }
        pattern
    }

    /// Read vector number when error occurred
    /// RTL register map: 0x14 = vector number (system_top.v:944,987)
    pub fn read_vector(&self) -> u32 {
        unsafe { read_volatile((self.base + 0x14) as *const u32) }
    }

    /// Read cycle count when error occurred (64-bit)
    /// RTL register map: 0x18 = cycle[31:0], 0x1C = cycle[63:32] (system_top.v:945-946,988-989)
    pub fn read_cycle(&self) -> u64 {
        let lo = unsafe { read_volatile((self.base + 0x18) as *const u32) };
        let hi = unsafe { read_volatile((self.base + 0x1C) as *const u32) };
        ((hi as u64) << 32) | (lo as u64)
    }
}
