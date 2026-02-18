//! FBC Binary Vector Format
//!
//! Zero-copy format designed for DMA to FPGA BRAM.
//!
//! # File Layout
//!
//! ```text
//! ┌───────────────────────────────────────┐
//! │ FbcHeader (32 bytes)                  │
//! ├───────────────────────────────────────┤
//! │ PinConfig (80 bytes)                  │
//! │   160 pins × 4 bits = 640 bits        │
//! ├───────────────────────────────────────┤
//! │ Vector Data (variable)                │
//! │   Opcodes + compressed data           │
//! └───────────────────────────────────────┘
//! ```
//!
//! # Opcodes
//!
//! | Opcode | Name           | Payload      | Description                    |
//! |--------|----------------|--------------|--------------------------------|
//! | 0x00   | NOP            | -            | No operation                   |
//! | 0x01   | VECTOR_FULL    | 20 bytes     | Raw 160-bit vector             |
//! | 0x02   | VECTOR_SPARSE  | 1 + N bytes  | Count + changed pin indices    |
//! | 0x03   | VECTOR_RUN     | 4 bytes      | Repeat previous N times        |
//! | 0x04   | VECTOR_ZERO    | -            | All pins low                   |
//! | 0x05   | VECTOR_ONES    | -            | All pins high                  |
//! | 0x06   | VECTOR_XOR     | 20 bytes     | XOR with previous              |
//! | 0x07   | END            | -            | End of program                 |

use std::io::{self, Read, Write};
use std::path::Path;
use std::fs::File;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use crc32fast::Hasher;

/// Magic number: "FBC\0" in little endian
pub const FBC_MAGIC: u32 = 0x00434246; // "FBC\0"

/// Current format version
pub const FBC_VERSION: u16 = 1;

/// Number of DUT pins
pub const PIN_COUNT: usize = 160;

/// Bytes per raw vector (160 bits = 20 bytes)
pub const VECTOR_BYTES: usize = 20;

/// ONETWO-derived constant: sparse encoding crossover point
/// When toggles > 15, full encoding is smaller than sparse
pub const SPARSE_CROSSOVER: usize = 15;

// Opcodes
pub const OP_NOP: u8 = 0x00;
pub const OP_VECTOR_FULL: u8 = 0x01;
pub const OP_VECTOR_SPARSE: u8 = 0x02;
pub const OP_VECTOR_RUN: u8 = 0x03;
pub const OP_VECTOR_ZERO: u8 = 0x04;
pub const OP_VECTOR_ONES: u8 = 0x05;
pub const OP_VECTOR_XOR: u8 = 0x06;
pub const OP_END: u8 = 0x07;

/// Pin types (from vector.vh)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PinType {
    #[default]
    Bidi = 0,       // Bidirectional (default)
    Input = 1,      // Input only (compare)
    Output = 2,     // Output only (drive)
    OpenCollector = 3,
    Pulse = 4,      // Pulse at T/4, 3T/4
    NPulse = 5,     // Inverted pulse
    ErrorTrig = 6,  // Error trigger output
    VecClk = 7,     // Vector clock output
    VecClkEn = 8,   // Clock enable output
}

impl From<u8> for PinType {
    fn from(val: u8) -> Self {
        match val {
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

/// FBC file header (32 bytes, aligned)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FbcHeader {
    pub magic: u32,          // 0x00434246 ("FBC\0")
    pub version: u16,        // Format version (1)
    pub pin_count: u8,       // 160
    pub flags: u8,           // Reserved
    pub num_vectors: u32,    // Total vectors after expansion
    pub compressed_size: u32, // Size of compressed data section
    pub vec_clock_hz: u32,   // Vector clock frequency
    pub crc32: u32,          // CRC32 of entire file (excluding this field)
    pub _reserved: [u8; 8],  // Padding to 32 bytes
}

impl Default for FbcHeader {
    fn default() -> Self {
        Self {
            magic: FBC_MAGIC,
            version: FBC_VERSION,
            pin_count: PIN_COUNT as u8,
            flags: 0,
            num_vectors: 0,
            compressed_size: 0,
            vec_clock_hz: 100_000_000, // 100 MHz default
            crc32: 0,
            _reserved: [0; 8],
        }
    }
}

impl FbcHeader {
    pub fn write_to<W: Write>(&self, w: &mut W) -> io::Result<()> {
        w.write_u32::<LittleEndian>(self.magic)?;
        w.write_u16::<LittleEndian>(self.version)?;
        w.write_u8(self.pin_count)?;
        w.write_u8(self.flags)?;
        w.write_u32::<LittleEndian>(self.num_vectors)?;
        w.write_u32::<LittleEndian>(self.compressed_size)?;
        w.write_u32::<LittleEndian>(self.vec_clock_hz)?;
        w.write_u32::<LittleEndian>(self.crc32)?;
        w.write_all(&self._reserved)?;
        Ok(())
    }

    pub fn read_from<R: Read>(r: &mut R) -> io::Result<Self> {
        let magic = r.read_u32::<LittleEndian>()?;
        if magic != FBC_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid magic: expected 0x{:08X}, got 0x{:08X}", FBC_MAGIC, magic),
            ));
        }

        let version = r.read_u16::<LittleEndian>()?;
        let pin_count = r.read_u8()?;
        let flags = r.read_u8()?;
        let num_vectors = r.read_u32::<LittleEndian>()?;
        let compressed_size = r.read_u32::<LittleEndian>()?;
        let vec_clock_hz = r.read_u32::<LittleEndian>()?;
        let crc32 = r.read_u32::<LittleEndian>()?;
        let mut reserved = [0u8; 8];
        r.read_exact(&mut reserved)?;

        Ok(Self {
            magic,
            version,
            pin_count,
            flags,
            num_vectors,
            compressed_size,
            vec_clock_hz,
            crc32,
            _reserved: reserved,
        })
    }
}

/// Pin configuration (80 bytes for 160 pins × 4 bits)
#[derive(Debug, Clone)]
pub struct PinConfig {
    pub types: [PinType; PIN_COUNT],
}

impl Default for PinConfig {
    fn default() -> Self {
        Self {
            types: [PinType::Bidi; PIN_COUNT],
        }
    }
}

impl PinConfig {
    /// Pack 160 4-bit pin types into 80 bytes
    pub fn to_bytes(&self) -> [u8; 80] {
        let mut bytes = [0u8; 80];
        for i in 0..80 {
            let lo = self.types[i * 2] as u8;
            let hi = self.types[i * 2 + 1] as u8;
            bytes[i] = lo | (hi << 4);
        }
        bytes
    }

    /// Unpack 80 bytes into 160 4-bit pin types
    pub fn from_bytes(bytes: &[u8; 80]) -> Self {
        let mut types = [PinType::Bidi; PIN_COUNT];
        for i in 0..80 {
            types[i * 2] = PinType::from(bytes[i] & 0x0F);
            types[i * 2 + 1] = PinType::from(bytes[i] >> 4);
        }
        Self { types }
    }
}

/// A single vector (160 bits = 20 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Vector {
    pub data: [u8; VECTOR_BYTES],
}

impl Vector {
    /// All zeros
    pub const ZERO: Vector = Vector { data: [0; VECTOR_BYTES] };

    /// All ones
    pub const ONES: Vector = Vector { data: [0xFF; VECTOR_BYTES] };

    /// Get bit at position (0-159)
    pub fn get_bit(&self, pos: usize) -> bool {
        debug_assert!(pos < PIN_COUNT);
        let byte_idx = pos / 8;
        let bit_idx = pos % 8;
        (self.data[byte_idx] >> bit_idx) & 1 == 1
    }

    /// Set bit at position (0-159)
    pub fn set_bit(&mut self, pos: usize, value: bool) {
        debug_assert!(pos < PIN_COUNT);
        let byte_idx = pos / 8;
        let bit_idx = pos % 8;
        if value {
            self.data[byte_idx] |= 1 << bit_idx;
        } else {
            self.data[byte_idx] &= !(1 << bit_idx);
        }
    }

    /// Count number of 1 bits
    pub fn popcount(&self) -> usize {
        self.data.iter().map(|b| b.count_ones() as usize).sum()
    }

    /// XOR with another vector
    pub fn xor(&self, other: &Vector) -> Vector {
        let mut result = Vector::ZERO;
        for i in 0..VECTOR_BYTES {
            result.data[i] = self.data[i] ^ other.data[i];
        }
        result
    }

    /// Count bits that differ from another vector
    pub fn hamming_distance(&self, other: &Vector) -> usize {
        self.xor(other).popcount()
    }

    /// Get indices of bits that are 1 (for sparse encoding)
    pub fn ones_indices(&self) -> Vec<u8> {
        let mut indices = Vec::new();
        for i in 0..PIN_COUNT {
            if self.get_bit(i) {
                indices.push(i as u8);
            }
        }
        indices
    }

    /// Create from indices of bits that should be 1
    pub fn from_indices(indices: &[u8]) -> Self {
        let mut vec = Vector::ZERO;
        for &idx in indices {
            if (idx as usize) < PIN_COUNT {
                vec.set_bit(idx as usize, true);
            }
        }
        vec
    }

    /// Parse from hex string (40 chars for 160 bits)
    pub fn from_hex(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if s.len() != 40 {
            return Err(format!("Expected 40 hex chars, got {}", s.len()));
        }

        let mut data = [0u8; VECTOR_BYTES];
        for i in 0..VECTOR_BYTES {
            data[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
                .map_err(|e| format!("Invalid hex at position {}: {}", i * 2, e))?;
        }
        Ok(Vector { data })
    }

    /// Format as hex string
    pub fn to_hex(&self) -> String {
        self.data.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Parse from binary string (160 chars of 0/1)
    pub fn from_binary(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if s.len() != PIN_COUNT {
            return Err(format!("Expected {} binary chars, got {}", PIN_COUNT, s.len()));
        }

        let mut vec = Vector::ZERO;
        for (i, c) in s.chars().enumerate() {
            match c {
                '1' | 'H' => vec.set_bit(i, true),
                '0' | 'L' => vec.set_bit(i, false),
                'X' | 'Z' | '-' => {} // Don't care / high-Z = 0
                _ => return Err(format!("Invalid binary char '{}' at position {}", c, i)),
            }
        }
        Ok(vec)
    }

    /// Format as binary string
    pub fn to_binary(&self) -> String {
        (0..PIN_COUNT)
            .map(|i| if self.get_bit(i) { '1' } else { '0' })
            .collect()
    }
}

/// Complete FBC vector file
#[derive(Debug, Clone)]
pub struct FbcFile {
    pub header: FbcHeader,
    pub pin_config: PinConfig,
    pub data: Vec<u8>, // Compressed vector data
}

impl FbcFile {
    /// Write to file
    pub fn write_to_file(&self, path: &Path) -> io::Result<()> {
        let mut file = File::create(path)?;
        self.write_to(&mut file)
    }

    /// Write to any writer
    pub fn write_to<W: Write>(&self, w: &mut W) -> io::Result<()> {
        // Write header (without CRC)
        let mut header = self.header;
        header.crc32 = 0;
        header.write_to(w)?;

        // Write pin config
        w.write_all(&self.pin_config.to_bytes())?;

        // Write compressed data
        w.write_all(&self.data)?;

        Ok(())
    }

    /// Read from file
    pub fn read_from_file(path: &Path) -> io::Result<Self> {
        let mut file = File::open(path)?;
        Self::read_from(&mut file)
    }

    /// Read from any reader
    pub fn read_from<R: Read>(r: &mut R) -> io::Result<Self> {
        let header = FbcHeader::read_from(r)?;

        let mut pin_bytes = [0u8; 80];
        r.read_exact(&mut pin_bytes)?;
        let pin_config = PinConfig::from_bytes(&pin_bytes);

        let mut data = vec![0u8; header.compressed_size as usize];
        r.read_exact(&mut data)?;

        Ok(Self {
            header,
            pin_config,
            data,
        })
    }

    /// Calculate CRC32 of file contents
    pub fn calculate_crc(&self) -> u32 {
        let mut hasher = Hasher::new();

        // Hash header (with crc32 field zeroed)
        let mut header_bytes = Vec::with_capacity(32);
        let mut header = self.header;
        header.crc32 = 0;
        header.write_to(&mut header_bytes).unwrap();
        hasher.update(&header_bytes);

        // Hash pin config
        hasher.update(&self.pin_config.to_bytes());

        // Hash data
        hasher.update(&self.data);

        hasher.finalize()
    }

    /// Validate CRC32
    pub fn validate_crc(&self) -> bool {
        self.header.crc32 == self.calculate_crc()
    }

    /// Get statistics about the file
    pub fn stats(&self) -> FbcStats {
        let uncompressed_size = self.header.num_vectors as usize * VECTOR_BYTES;
        let compressed_size = self.data.len();
        let compression_ratio = if compressed_size > 0 {
            uncompressed_size as f64 / compressed_size as f64
        } else {
            1.0
        };

        // Count opcodes
        let mut op_counts = [0usize; 8];
        let mut i = 0;
        while i < self.data.len() {
            let op = self.data[i];
            if (op as usize) < op_counts.len() {
                op_counts[op as usize] += 1;
            }

            // Skip payload
            i += match op {
                OP_NOP | OP_VECTOR_ZERO | OP_VECTOR_ONES | OP_END => 1,
                OP_VECTOR_FULL | OP_VECTOR_XOR => 1 + VECTOR_BYTES,
                OP_VECTOR_SPARSE => {
                    if i + 1 < self.data.len() {
                        1 + 1 + self.data[i + 1] as usize
                    } else {
                        1
                    }
                }
                OP_VECTOR_RUN => 1 + 4,
                _ => 1,
            };
        }

        FbcStats {
            num_vectors: self.header.num_vectors,
            uncompressed_size,
            compressed_size,
            compression_ratio,
            vec_clock_hz: self.header.vec_clock_hz,
            op_counts,
        }
    }
}

/// Statistics about an FBC file
#[derive(Debug)]
pub struct FbcStats {
    pub num_vectors: u32,
    pub uncompressed_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f64,
    pub vec_clock_hz: u32,
    pub op_counts: [usize; 8], // One per opcode
}

impl std::fmt::Display for FbcStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "FBC Vector File Statistics:")?;
        writeln!(f, "  Vectors:           {:>12}", self.num_vectors)?;
        writeln!(f, "  Vector clock:      {:>12} Hz", self.vec_clock_hz)?;
        writeln!(f, "  Uncompressed:      {:>12} bytes", self.uncompressed_size)?;
        writeln!(f, "  Compressed:        {:>12} bytes", self.compressed_size)?;
        writeln!(f, "  Compression ratio: {:>12.2}x", self.compression_ratio)?;
        writeln!(f)?;
        writeln!(f, "  Opcode usage:")?;
        let names = ["NOP", "FULL", "SPARSE", "RUN", "ZERO", "ONES", "XOR", "END"];
        for (i, &count) in self.op_counts.iter().enumerate() {
            if count > 0 {
                writeln!(f, "    {:8}: {:>10}", names[i], count)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_bits() {
        let mut v = Vector::ZERO;
        assert!(!v.get_bit(0));
        assert!(!v.get_bit(159));

        v.set_bit(0, true);
        v.set_bit(159, true);
        assert!(v.get_bit(0));
        assert!(v.get_bit(159));
        assert!(!v.get_bit(1));

        assert_eq!(v.popcount(), 2);
    }

    #[test]
    fn test_vector_hex() {
        let hex = "0102030405060708090a0b0c0d0e0f1011121314";
        let v = Vector::from_hex(hex).unwrap();
        assert_eq!(v.to_hex(), hex);
    }

    #[test]
    fn test_vector_binary() {
        let mut bin = String::new();
        for i in 0..160 {
            bin.push(if i % 2 == 0 { '1' } else { '0' });
        }
        let v = Vector::from_binary(&bin).unwrap();
        assert_eq!(v.to_binary(), bin);
    }

    #[test]
    fn test_hamming_distance() {
        let a = Vector::ZERO;
        let b = Vector::ONES;
        assert_eq!(a.hamming_distance(&b), 160);

        let mut c = Vector::ZERO;
        c.set_bit(0, true);
        c.set_bit(1, true);
        assert_eq!(a.hamming_distance(&c), 2);
    }

    #[test]
    fn test_pin_config_roundtrip() {
        let mut config = PinConfig::default();
        config.types[0] = PinType::Input;
        config.types[1] = PinType::Output;
        config.types[159] = PinType::Pulse;

        let bytes = config.to_bytes();
        let config2 = PinConfig::from_bytes(&bytes);

        assert_eq!(config.types[0], config2.types[0]);
        assert_eq!(config.types[1], config2.types[1]);
        assert_eq!(config.types[159], config2.types[159]);
    }
}
