//! Zynq 7020 GIC (Generic Interrupt Controller) Driver
//!
//! Cortex-A9 MPCore GIC with distributor + CPU interface.
//!
//! # Zynq Interrupt Map (relevant to FBC)
//!
//! IRQ_F2P[0] = GIC interrupt ID 61 (SPI #29)
//! All PL interrupt sources are OR'd into this single line:
//!   - irq_done:  FBC decoder finished
//!   - irq_error: FBC decoder error or vector mismatch
//!   - irq_freq:  Frequency counter done
//!   - irq_dma:   DMA transfer complete

use super::{Reg, Register};
use core::sync::atomic::{AtomicU32, Ordering};

// GIC base addresses (Zynq 7020 Cortex-A9 MPCore)
const GICD_BASE: usize = 0xF8F0_1000; // Distributor
const GICC_BASE: usize = 0xF8F0_0100; // CPU Interface

// Distributor registers
const GICD_CTLR: usize      = 0x000; // Distributor Control
const GICD_ISENABLER: usize  = 0x100; // Interrupt Set-Enable (32 IRQs per reg)
const GICD_ICENABLER: usize  = 0x180; // Interrupt Clear-Enable
const GICD_IPRIORITYR: usize = 0x400; // Interrupt Priority (4 IRQs per reg)
const GICD_ITARGETSR: usize  = 0x800; // Interrupt Processor Targets (4 per reg)
const GICD_ICFGR: usize     = 0xC00; // Interrupt Configuration (16 per reg)
const GICD_ICPENDR: usize   = 0x280; // Interrupt Clear-Pending

// CPU Interface registers
const GICC_CTLR: usize = 0x00; // CPU Interface Control
const GICC_PMR: usize  = 0x04; // Priority Mask
const GICC_IAR: usize  = 0x0C; // Interrupt Acknowledge
const GICC_EOIR: usize = 0x10; // End of Interrupt

// Zynq IRQ_F2P[0] = GIC interrupt ID 61
const IRQ_FBC: u32 = 61;

/// Interrupt flags set by IRQ handler, read by main loop
pub static IRQ_FLAGS: AtomicU32 = AtomicU32::new(0);

/// Flag bits
pub const IRQ_FLAG_FBC: u32 = 1 << 0;

/// GIC controller
pub struct Gic;

impl Gic {
    pub fn new() -> Self { Self }

    /// Initialize GIC distributor and CPU interface.
    /// Call once during boot, before enabling CPU interrupts.
    pub fn init(&self) {
        // ---- Distributor ----

        // Disable distributor during config
        self.dist_write(GICD_CTLR, 0);

        // Configure IRQ 61 (IRQ_F2P[0])
        let irq = IRQ_FBC;

        // Set priority (lower = higher priority, 0 = highest)
        // Priority register: 4 IRQs per 32-bit reg, 8 bits each
        let pri_reg = GICD_IPRIORITYR + ((irq / 4) as usize) * 4;
        let pri_shift = ((irq % 4) * 8) as u32;
        let pri_val = self.dist_read(pri_reg);
        let pri_val = (pri_val & !(0xFF << pri_shift)) | (0xA0 << pri_shift); // priority 0xA0
        self.dist_write(pri_reg, pri_val);

        // Target CPU 0
        let tgt_reg = GICD_ITARGETSR + ((irq / 4) as usize) * 4;
        let tgt_shift = ((irq % 4) * 8) as u32;
        let tgt_val = self.dist_read(tgt_reg);
        let tgt_val = (tgt_val & !(0xFF << tgt_shift)) | (0x01 << tgt_shift); // CPU 0
        self.dist_write(tgt_reg, tgt_val);

        // Configure as level-sensitive (IRQ_F2P is active-high level)
        let cfg_reg = GICD_ICFGR + ((irq / 16) as usize) * 4;
        let cfg_shift = ((irq % 16) * 2) as u32;
        let cfg_val = self.dist_read(cfg_reg);
        let cfg_val = cfg_val & !(0x3 << cfg_shift); // 0b00 = level-sensitive
        self.dist_write(cfg_reg, cfg_val);

        // Clear any pending interrupt
        let pend_reg = GICD_ICPENDR + ((irq / 32) as usize) * 4;
        self.dist_write(pend_reg, 1 << (irq % 32));

        // Enable interrupt 61
        let en_reg = GICD_ISENABLER + ((irq / 32) as usize) * 4;
        self.dist_write(en_reg, 1 << (irq % 32));

        // Enable distributor
        self.dist_write(GICD_CTLR, 1);

        // ---- CPU Interface ----

        // Set priority mask (allow all priorities)
        self.cpu_write(GICC_PMR, 0xFF);

        // Enable CPU interface
        self.cpu_write(GICC_CTLR, 1);
    }

    /// Acknowledge interrupt — returns interrupt ID.
    /// Call from IRQ handler. Returns 1023 if spurious.
    #[inline]
    pub fn acknowledge(&self) -> u32 {
        self.cpu_read(GICC_IAR) & 0x3FF
    }

    /// Signal end of interrupt processing.
    /// Call from IRQ handler after handling.
    #[inline]
    pub fn end_interrupt(&self, irq_id: u32) {
        self.cpu_write(GICC_EOIR, irq_id);
    }

    /// Disable a specific interrupt
    pub fn disable_irq(&self, irq: u32) {
        let reg = GICD_ICENABLER + ((irq / 32) as usize) * 4;
        self.dist_write(reg, 1 << (irq % 32));
    }

    /// Enable a specific interrupt
    pub fn enable_irq(&self, irq: u32) {
        let reg = GICD_ISENABLER + ((irq / 32) as usize) * 4;
        self.dist_write(reg, 1 << (irq % 32));
    }

    // Register access helpers
    #[inline]
    fn dist_read(&self, offset: usize) -> u32 {
        Reg::new(GICD_BASE + offset).read()
    }
    #[inline]
    fn dist_write(&self, offset: usize, val: u32) {
        Reg::new(GICD_BASE + offset).write(val);
    }
    #[inline]
    fn cpu_read(&self, offset: usize) -> u32 {
        Reg::new(GICC_BASE + offset).read()
    }
    #[inline]
    fn cpu_write(&self, offset: usize, val: u32) {
        Reg::new(GICC_BASE + offset).write(val);
    }
}

/// Called from assembly IRQ handler.
/// Acknowledges GIC, identifies source, sets flags, signals EOI.
#[no_mangle]
pub extern "C" fn gic_irq_dispatch() {
    let gic = Gic::new();
    let irq_id = gic.acknowledge();

    if irq_id == 1023 {
        return; // Spurious
    }

    if irq_id == IRQ_FBC {
        // Set flag for main loop
        IRQ_FLAGS.fetch_or(IRQ_FLAG_FBC, Ordering::Release);
    }

    gic.end_interrupt(irq_id);
}
