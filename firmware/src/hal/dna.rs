//! Device DNA - Zynq 7000 Unique Device Identifier
//!
//! Every Zynq chip has a unique 57-bit DNA that can be used to:
//! - Generate unique MAC addresses
//! - Create device serial numbers
//! - Fingerprint boards
//!
//! # Hardware
//! The DNA is accessed via the Device DNA port in the FPGA fabric.
//! Since we're bare-metal on the ARM, we need the FPGA bitstream to
//! expose it via an AXI register.
//!
//! # Fallback
//! If DNA is not available (no bitstream loaded), we can use the
//! ARM CPU ID register as a less-unique fallback.

use core::ptr::read_volatile;
use crate::regs::DNA_BASE;

/// AXI register offsets within axi_device_dna (0x400A_0000)
const DNA_LO_OFF: usize     = 0x00;  // DNA[31:0]
const DNA_HI_OFF: usize     = 0x04;  // {7'b0, DNA[56:32]}
const DNA_STATUS_OFF: usize = 0x08;  // {31'b0, dna_valid}

/// ARM CPU ID base address (fallback only — identical across all Zynq 7020)
const MIDR_BASE: usize = 0xF8F00000;

/// Device DNA value (57 bits)
#[derive(Debug, Clone, Copy)]
pub struct DeviceDna {
    /// Lower 32 bits — DNA[31:0]
    pub low: u32,
    /// Upper 25 bits — DNA[56:32] (bits 31:25 always zero)
    pub high: u32,
}

impl DeviceDna {
    /// Read device DNA from FPGA axi_device_dna peripheral at 0x400A_0000.
    ///
    /// The DNA_PORT shift FSM completes ~57 clocks after reset (~570 ns at 100 MHz).
    /// DNA_STATUS bit 0 = dna_valid. Returns None if not yet valid or not present.
    ///
    /// SAFETY: Reading 0x400A_0000 when axi_device_dna is not in the bitstream
    /// causes an AXI decode error → Data Abort. We guard by checking FBC_CTRL
    /// version first — the March 12 bitstream has version=0 and no DNA peripheral.
    pub fn read_from_fpga() -> Option<Self> {
        // Guard: FBC_CTRL VERSION at 0x4004_001C reads 0x0001_0000 when axi_device_dna
        // is present. Old bitstreams without it return 0. Offset 0x00 is CTRL (also 0 at reset).
        let fbc_version = unsafe { read_volatile(0x4004_001C as *const u32) };
        if fbc_version == 0 || fbc_version == 0xFFFF_FFFF {
            return None;
        }

        let status = unsafe { read_volatile((DNA_BASE + DNA_STATUS_OFF) as *const u32) };
        if status & 1 == 0 {
            return None;
        }
        let low  = unsafe { read_volatile((DNA_BASE + DNA_LO_OFF) as *const u32) };
        let high = unsafe { read_volatile((DNA_BASE + DNA_HI_OFF) as *const u32) };
        Some(Self { low, high })
    }

    /// Generate DNA from ARM CPU ID (fallback when FPGA not programmed).
    ///
    /// WARNING: All Zynq 7020 silicon returns MIDR = 0x413FC090.
    /// This means ALL boards get the same MAC. Only use for bringup/debug.
    pub fn from_cpu_id() -> Self {
        let cpu_id = unsafe { read_volatile(MIDR_BASE as *const u32) };
        Self {
            low: cpu_id,
            high: 0xDEAD, // Marker to show this is fallback, not real DNA
        }
    }

    /// Read device DNA (tries FPGA first, falls back to CPU ID)
    pub fn read() -> Self {
        Self::read_from_fpga().unwrap_or_else(Self::from_cpu_id)
    }

    /// Generate a MAC address from device DNA
    ///
    /// Uses Xilinx OUI (00:0A:35) + device-specific bits
    pub fn to_mac(&self) -> [u8; 6] {
        let mut mac = [0u8; 6];

        // Xilinx OUI
        mac[0] = 0x00;
        mac[1] = 0x0A;
        mac[2] = 0x35;

        // Device-specific (from DNA)
        mac[3] = (self.high & 0xFF) as u8;
        mac[4] = (self.low >> 8) as u8;
        mac[5] = (self.low & 0xFF) as u8;

        mac
    }

    /// Generate static IP from DNA
    ///
    /// Returns IP in 172.16.0.0/16 range
    /// Last two octets derived from DNA to ensure uniqueness
    pub fn to_ip(&self) -> [u8; 4] {
        [
            172,
            16,
            (self.low >> 8) as u8,
            (self.low & 0xFF) as u8,
        ]
    }

    /// Get as 64-bit value for display/logging
    pub fn as_u64(&self) -> u64 {
        ((self.high as u64) << 32) | (self.low as u64)
    }
}

/// Generate MAC address from device DNA (convenience function)
pub fn mac_from_dna() -> [u8; 6] {
    DeviceDna::read().to_mac()
}

/// Generate static IP from device DNA (convenience function)
pub fn ip_from_dna() -> [u8; 4] {
    DeviceDna::read().to_ip()
}

/// Read raw device DNA as u64 (convenience function)
pub fn read_device_dna() -> u64 {
    DeviceDna::read().as_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mac_generation() {
        let dna = DeviceDna {
            low: 0x12345678,
            high: 0xAB,
        };

        let mac = dna.to_mac();
        assert_eq!(mac[0..3], [0x00, 0x0A, 0x35]); // Xilinx OUI
        assert_eq!(mac[3], 0xAB);
        assert_eq!(mac[4], 0x56);
        assert_eq!(mac[5], 0x78);
    }

    #[test]
    fn test_ip_generation() {
        let dna = DeviceDna {
            low: 0x12345678,
            high: 0xAB,
        };

        let ip = dna.to_ip();
        assert_eq!(ip, [172, 16, 0x56, 0x78]);
    }
}
