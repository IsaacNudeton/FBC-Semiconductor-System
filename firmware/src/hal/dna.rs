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

use super::{Reg, Register};

/// ARM CPU ID base address
const MIDR_BASE: usize = 0xF8F00000;

/// Device DNA value (57 bits)
#[derive(Debug, Clone, Copy)]
pub struct DeviceDna {
    /// Lower 32 bits
    pub low: u32,
    /// Upper 25 bits (stored in lower bits of u32)
    pub high: u32,
}

impl DeviceDna {
    /// Read device DNA from FPGA fabric (if available)
    ///
    /// NOTE: Requires FPGA bitstream with DNA port exposed via AXI.
    /// If not available, returns None and caller should use fallback.
    pub fn read_from_fpga() -> Option<Self> {
        // TODO: This requires FPGA bitstream support
        // For now, return None and use fallback
        None
    }

    /// Generate DNA from ARM CPU ID (fallback when FPGA not programmed)
    ///
    /// Uses CPU ID registers to create a semi-unique identifier.
    /// Not as unique as real DNA, but good enough for MAC generation.
    pub fn from_cpu_id() -> Self {
        let midr = Reg::new(MIDR_BASE);
        let cpu_id = midr.read();

        // Use CPU ID + a counter/variant
        // In real deployment, you'd combine this with board-specific data
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
