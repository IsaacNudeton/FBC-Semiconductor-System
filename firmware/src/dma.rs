//! Zynq AXI DMA Driver
//!
//! Manages DMA transfers from memory to the FBC decoder via AXI4-Stream.

use core::ptr::{read_volatile, write_volatile};

// =============================================================================
// AXI DMA Registers (Xilinx pg021)
// =============================================================================

pub const AXI_DMA_BASE: usize = 0x4040_0000;

// MM2S (Memory-Mapped to Stream) - for sending FBC to FPGA
const MM2S_DMACR: usize = 0x00;      // Control register
const MM2S_DMASR: usize = 0x04;      // Status register
const MM2S_SA: usize = 0x18;         // Source address (low)
const MM2S_SA_MSB: usize = 0x1C;     // Source address (high) - 64-bit addressing
const MM2S_LENGTH: usize = 0x28;     // Transfer length

// S2MM (Stream to Memory-Mapped) - for receiving data from FPGA
const S2MM_DMACR: usize = 0x30;
const S2MM_DMASR: usize = 0x34;
const S2MM_DA: usize = 0x48;         // Destination address (low)
const S2MM_DA_MSB: usize = 0x4C;     // Destination address (high)
const S2MM_LENGTH: usize = 0x58;

// Control register bits
const DMACR_RS: u32 = 1 << 0;        // Run/Stop
const DMACR_RESET: u32 = 1 << 2;     // Soft reset
const DMACR_IOC_IRQ_EN: u32 = 1 << 12;  // Interrupt on complete

// Status register bits
const DMASR_HALTED: u32 = 1 << 0;
const DMASR_IDLE: u32 = 1 << 1;
const DMASR_IOC_IRQ: u32 = 1 << 12;
const DMASR_ERR_IRQ: u32 = 1 << 14;

/// AXI DMA Controller
pub struct AxiDma {
    base: usize,
}

/// DMA transfer result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaResult {
    Ok,
    Busy,
    Error,
    Timeout,
}

impl AxiDma {
    pub const fn new() -> Self {
        Self { base: AXI_DMA_BASE }
    }

    /// Initialize the DMA controller
    pub fn init(&self) {
        // Reset both channels
        self.write_reg(MM2S_DMACR, DMACR_RESET);
        self.write_reg(S2MM_DMACR, DMACR_RESET);

        // Wait for reset complete
        while self.read_reg(MM2S_DMACR) & DMACR_RESET != 0 {}
        while self.read_reg(S2MM_DMACR) & DMACR_RESET != 0 {}
    }

    /// Start MM2S transfer (send FBC program to FPGA)
    ///
    /// # Arguments
    /// * `src_addr` - Physical address of FBC data in memory
    /// * `length` - Number of bytes to transfer (must be 256-bit aligned = 32 bytes)
    pub fn send_fbc(&self, src_addr: u32, length: u32) -> DmaResult {
        // Check if already running
        if !self.is_mm2s_idle() {
            return DmaResult::Busy;
        }

        // Clear any pending interrupts
        self.write_reg(MM2S_DMASR, DMASR_IOC_IRQ | DMASR_ERR_IRQ);

        // Enable the channel with interrupt
        self.write_reg(MM2S_DMACR, DMACR_RS | DMACR_IOC_IRQ_EN);

        // Set source address (32-bit addressing for Zynq 7020)
        self.write_reg(MM2S_SA, src_addr);
        self.write_reg(MM2S_SA_MSB, 0);

        // Set length to start transfer
        self.write_reg(MM2S_LENGTH, length);

        DmaResult::Ok
    }

    /// Check if MM2S channel is idle
    pub fn is_mm2s_idle(&self) -> bool {
        let status = self.read_reg(MM2S_DMASR);
        status & DMASR_IDLE != 0 || status & DMASR_HALTED != 0
    }

    /// Check if MM2S transfer is complete
    pub fn is_mm2s_complete(&self) -> bool {
        self.read_reg(MM2S_DMASR) & DMASR_IOC_IRQ != 0
    }

    /// Check if MM2S had an error
    pub fn has_mm2s_error(&self) -> bool {
        self.read_reg(MM2S_DMASR) & DMASR_ERR_IRQ != 0
    }

    /// Wait for MM2S transfer to complete
    pub fn wait_mm2s(&self, timeout_cycles: u32) -> DmaResult {
        let mut count = 0u32;
        loop {
            if self.is_mm2s_complete() {
                // Clear interrupt
                self.write_reg(MM2S_DMASR, DMASR_IOC_IRQ);
                return DmaResult::Ok;
            }
            if self.has_mm2s_error() {
                return DmaResult::Error;
            }
            count += 1;
            if count > timeout_cycles {
                return DmaResult::Timeout;
            }
            core::hint::spin_loop();
        }
    }

    /// Start S2MM transfer (receive data from FPGA)
    pub fn receive(&self, dst_addr: u32, length: u32) -> DmaResult {
        if !self.is_s2mm_idle() {
            return DmaResult::Busy;
        }

        self.write_reg(S2MM_DMASR, DMASR_IOC_IRQ | DMASR_ERR_IRQ);
        self.write_reg(S2MM_DMACR, DMACR_RS | DMACR_IOC_IRQ_EN);
        self.write_reg(S2MM_DA, dst_addr);
        self.write_reg(S2MM_DA_MSB, 0);
        self.write_reg(S2MM_LENGTH, length);

        DmaResult::Ok
    }

    /// Check if S2MM channel is idle
    pub fn is_s2mm_idle(&self) -> bool {
        let status = self.read_reg(S2MM_DMASR);
        status & DMASR_IDLE != 0 || status & DMASR_HALTED != 0
    }

    /// Check if S2MM transfer is complete
    pub fn is_s2mm_complete(&self) -> bool {
        self.read_reg(S2MM_DMASR) & DMASR_IOC_IRQ != 0
    }

    // Register access helpers
    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }

    fn write_reg(&self, offset: usize, val: u32) {
        unsafe { write_volatile((self.base + offset) as *mut u32, val) }
    }
}

// =============================================================================
// DMA Buffer Management
// =============================================================================

/// Simple ring buffer for DMA transfers
/// Uses a fixed memory region for DMA buffers
pub struct DmaBuffer {
    base_addr: u32,
    size: u32,
    head: u32,
    tail: u32,
}

impl DmaBuffer {
    /// Create a new DMA buffer at a fixed physical address
    ///
    /// # Safety
    /// The memory region must be:
    /// - Non-cached or cache-coherent
    /// - Not used by anything else
    /// - Aligned to 32 bytes (256-bit AXI)
    pub const fn new(base_addr: u32, size: u32) -> Self {
        Self {
            base_addr,
            size,
            head: 0,
            tail: 0,
        }
    }

    /// Write FBC instructions to the buffer
    /// Returns the physical address and length for DMA
    pub fn write(&mut self, data: &[u8]) -> Option<(u32, u32)> {
        let len = data.len() as u32;

        // Check if there's enough space
        if len > self.available_space() {
            return None;
        }

        let write_addr = self.base_addr + self.head;

        // Copy data to DMA buffer
        unsafe {
            let dst = write_addr as *mut u8;
            for (i, &byte) in data.iter().enumerate() {
                dst.add(i).write_volatile(byte);
            }
        }

        // Update head
        self.head = (self.head + len) % self.size;

        Some((write_addr, len))
    }

    /// Mark data as consumed after DMA completes
    pub fn consume(&mut self, len: u32) {
        self.tail = (self.tail + len) % self.size;
    }

    /// Available space in buffer
    pub fn available_space(&self) -> u32 {
        if self.head >= self.tail {
            self.size - (self.head - self.tail) - 1
        } else {
            self.tail - self.head - 1
        }
    }

    /// Reset buffer
    pub fn reset(&mut self) {
        self.head = 0;
        self.tail = 0;
    }
}

// =============================================================================
// FBC Streaming
// =============================================================================

/// FBC Streamer - manages DMA transfers of FBC programs
pub struct FbcStreamer {
    dma: AxiDma,
    buffer: DmaBuffer,
}

// DMA buffer located in non-cached OCM (On-Chip Memory)
// 0xFFFC_0000 - 0xFFFF_FFFF is 256KB OCM on Zynq
const DMA_BUFFER_ADDR: u32 = 0xFFFC_0000;
const DMA_BUFFER_SIZE: u32 = 64 * 1024;  // 64KB for FBC programs

impl FbcStreamer {
    pub const fn new() -> Self {
        Self {
            dma: AxiDma::new(),
            buffer: DmaBuffer::new(DMA_BUFFER_ADDR, DMA_BUFFER_SIZE),
        }
    }

    /// Initialize the streamer
    pub fn init(&mut self) {
        self.dma.init();
        self.buffer.reset();
    }

    /// Stream FBC program to FPGA
    ///
    /// # Arguments
    /// * `fbc_data` - Raw FBC bytecode (256-bit words)
    pub fn stream_program(&mut self, fbc_data: &[u8]) -> DmaResult {
        // Align to 32 bytes (256 bits)
        let aligned_len = (fbc_data.len() + 31) & !31;

        // Write to DMA buffer
        let (addr, len) = match self.buffer.write(fbc_data) {
            Some((a, _)) => (a, aligned_len as u32),
            None => return DmaResult::Error,
        };

        // Start DMA transfer
        let result = self.dma.send_fbc(addr, len);
        if result != DmaResult::Ok {
            return result;
        }

        // Wait for completion (10M cycles timeout = ~100ms at 100MHz)
        let result = self.dma.wait_mm2s(10_000_000);

        // Mark buffer as consumed
        self.buffer.consume(len);

        result
    }

    /// Stream from a DDR physical address directly (no OCM copy).
    /// Used by test plan executor to DMA from DDR vector slots.
    ///
    /// # Arguments
    /// * `ddr_addr` - Physical address in DDR (must be 32-byte aligned)
    /// * `length` - Number of bytes to transfer
    pub fn stream_from_ddr(&mut self, ddr_addr: u32, length: u32) -> DmaResult {
        let aligned_len = (length + 31) & !31;

        let result = self.dma.send_fbc(ddr_addr, aligned_len);
        if result != DmaResult::Ok {
            return result;
        }

        // Longer timeout for large DDR transfers (100M cycles = ~1s at 100MHz)
        self.dma.wait_mm2s(100_000_000)
    }

    /// Check if streamer is ready for more data
    pub fn is_ready(&self) -> bool {
        self.dma.is_mm2s_idle()
    }
}
