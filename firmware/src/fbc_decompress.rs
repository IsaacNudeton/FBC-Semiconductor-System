//! FBC Decompressor
//!
//! Translates compressed .fbc opcodes to FPGA bytecode.
//!
//! # Compression Format (Input)
//!
//! | Opcode | Name          | Payload         | Action                              |
//! |--------|---------------|-----------------|-------------------------------------|
//! | 0x00   | NOP           | -               | Skip                                |
//! | 0x01   | VECTOR_FULL   | 20 bytes        | Output raw 160-bit vector           |
//! | 0x02   | VECTOR_SPARSE | 1 + N bytes     | Modify prev_vector at N bit positions |
//! | 0x03   | VECTOR_RUN    | 4 bytes (u32 LE)| Repeat prev_vector N times          |
//! | 0x04   | VECTOR_ZERO   | -               | Output all zeros                    |
//! | 0x05   | VECTOR_ONES   | -               | Output all 0xFF                     |
//! | 0x06   | VECTOR_XOR    | 20 bytes        | Output prev_vector XOR mask         |
//! | 0x07   | END           | -               | Stop decompression                  |
//!
//! # FPGA Bytecode (Output)
//!
//! | Opcode | Name        | Description                    |
//! |--------|-------------|--------------------------------|
//! | 0xC0   | SET_PINS    | Set 128-bit pin values         |
//! | 0xB5   | PATTERN_REP | Repeat current pattern N times |
//! | 0xFF   | HALT        | End program                    |
//!
//! # Unsupported Opcodes
//!
//! The following FBC opcodes are defined but not implemented in hardware:
//! - `0xB0` (LOOP_N): Requires instruction buffer/PC replay (not in fbc_decoder.v)
//! - `0xD1` (SYNC): External trigger wait (not implemented)
//! - `0xE0` (IMM32): 32-bit immediate (not implemented)
//! - `0xE1` (IMM128): 128-bit immediate (not implemented)
//!
//! The decompressor will return `LoaderError::InvalidOpcode` if these are encountered.

use crate::fbc::FbcInstr;

/// Compressed FBC opcodes (from tools/fbc-vec format.rs)
pub mod opcodes {
    pub const NOP: u8 = 0x00;
    pub const VECTOR_FULL: u8 = 0x01;
    pub const VECTOR_SPARSE: u8 = 0x02;
    pub const VECTOR_RUN: u8 = 0x03;
    pub const VECTOR_ZERO: u8 = 0x04;
    pub const VECTOR_ONES: u8 = 0x05;
    pub const VECTOR_XOR: u8 = 0x06;
    pub const END: u8 = 0x07;
}

/// Bytes per raw vector (160 bits = 20 bytes)
pub const VECTOR_BYTES: usize = 20;

/// Number of DUT pins
pub const PIN_COUNT: usize = 160;

/// FBC Decompressor state machine
///
/// Decompresses .fbc format into FPGA bytecode instructions.
pub struct FbcDecompressor<'a> {
    data: &'a [u8],
    pos: usize,
    prev_vector: [u8; VECTOR_BYTES],
}

impl<'a> FbcDecompressor<'a> {
    /// Create a new decompressor from compressed data
    ///
    /// # Arguments
    /// * `data` - The compressed vector data (after header + pin config)
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            prev_vector: [0u8; VECTOR_BYTES],
        }
    }

    /// Reset decompressor to beginning
    pub fn reset(&mut self) {
        self.pos = 0;
        self.prev_vector = [0u8; VECTOR_BYTES];
    }

    /// Get current position in data
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Check if at end of data
    pub fn is_done(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// Decompress next vector
    ///
    /// Returns `Some((vector, repeat_count))` where:
    /// - `vector` is the 20-byte (160-bit) pin values
    /// - `repeat_count` is how many times to output this vector (1 = single, >1 = run)
    ///
    /// Returns `None` when decompression is complete (END opcode or end of data).
    pub fn next(&mut self) -> Option<([u8; VECTOR_BYTES], u32)> {
        loop {
            if self.pos >= self.data.len() {
                return None;
            }

            let opcode = self.data[self.pos];
            self.pos += 1;

            match opcode {
                opcodes::NOP => {
                    // Skip NOP, continue to next opcode
                    continue;
                }

                opcodes::VECTOR_FULL => {
                    // Full 20-byte vector follows
                    if self.pos + VECTOR_BYTES > self.data.len() {
                        return None; // Truncated data
                    }
                    self.prev_vector.copy_from_slice(&self.data[self.pos..self.pos + VECTOR_BYTES]);
                    self.pos += VECTOR_BYTES;
                    return Some((self.prev_vector, 1));
                }

                opcodes::VECTOR_SPARSE => {
                    // Sparse encoding: modify specific bits in prev_vector
                    if self.pos >= self.data.len() {
                        return None;
                    }
                    let count = self.data[self.pos] as usize;
                    self.pos += 1;

                    if self.pos + count > self.data.len() {
                        return None; // Truncated data
                    }

                    for i in 0..count {
                        let encoded = self.data[self.pos + i];
                        let pin = (encoded >> 1) as usize;
                        let value = encoded & 1;

                        if pin < PIN_COUNT {
                            let byte_idx = pin / 8;
                            let bit_idx = pin % 8;
                            if value != 0 {
                                self.prev_vector[byte_idx] |= 1 << bit_idx;
                            } else {
                                self.prev_vector[byte_idx] &= !(1 << bit_idx);
                            }
                        }
                    }
                    self.pos += count;
                    return Some((self.prev_vector, 1));
                }

                opcodes::VECTOR_RUN => {
                    // Repeat prev_vector N times
                    // Note: Compiler stores (count - 1), so we add 1 back
                    // See compiler.rs line 124: data.extend_from_slice(&(count - 1).to_le_bytes());
                    if self.pos + 4 > self.data.len() {
                        return None;
                    }
                    let stored_count = u32::from_le_bytes([
                        self.data[self.pos],
                        self.data[self.pos + 1],
                        self.data[self.pos + 2],
                        self.data[self.pos + 3],
                    ]);
                    self.pos += 4;
                    return Some((self.prev_vector, stored_count + 1));
                }

                opcodes::VECTOR_ZERO => {
                    self.prev_vector = [0x00; VECTOR_BYTES];
                    return Some((self.prev_vector, 1));
                }

                opcodes::VECTOR_ONES => {
                    self.prev_vector = [0xFF; VECTOR_BYTES];
                    return Some((self.prev_vector, 1));
                }

                opcodes::VECTOR_XOR => {
                    // XOR mask follows
                    if self.pos + VECTOR_BYTES > self.data.len() {
                        return None;
                    }
                    for i in 0..VECTOR_BYTES {
                        self.prev_vector[i] ^= self.data[self.pos + i];
                    }
                    self.pos += VECTOR_BYTES;
                    return Some((self.prev_vector, 1));
                }

                opcodes::END => {
                    return None;
                }

                _ => {
                    // Unknown opcode - skip and continue
                    continue;
                }
            }
        }
    }
}

/// Maximum size of decompressed bytecode buffer
/// This is sized for typical programs - larger programs may need chunked streaming
pub const MAX_BYTECODE_SIZE: usize = 64 * 1024; // 64KB

/// Size of one FPGA instruction word in bytes.
///
/// fbc_dma.v reads 4 × 64-bit beats = 32 bytes per burst, producing one
/// 256-bit AXI-Stream word. axi_stream_fbc.v extracts:
///   [63:0]    = instruction (opcode + flags + operand)
///   [191:64]  = 128-bit payload (pin values for SET_PINS/SET_OEN)
///   [255:192] = reserved
///
/// Every instruction MUST be exactly 32 bytes in the DMA buffer.
/// Misalignment causes the DMA to split instructions across bursts,
/// feeding garbled data to the decoder.
const INSTR_WORD_SIZE: usize = 32;

/// Decompress FBC data to FPGA bytecode
///
/// Converts compressed .fbc opcodes (0x01 VECTOR_FULL, 0x03 VECTOR_RUN, etc.)
/// into 32-byte FPGA instruction words (SET_PINS, PATTERN_REP, HALT) suitable
/// for DMA transfer to fbc_decoder.v.
///
/// # Arguments
/// * `compressed_data` - The compressed vector data (after header + pin config)
/// * `output` - Buffer to write FPGA bytecode into (must be 32-byte aligned capacity)
///
/// # Returns
/// Number of bytes written to output, or None if buffer too small
pub fn decompress_to_bytecode(compressed_data: &[u8], output: &mut [u8]) -> Option<usize> {
    let mut decompressor = FbcDecompressor::new(compressed_data);
    let mut write_pos = 0;

    while let Some((vector, repeat_count)) = decompressor.next() {
        // Each instruction = 32 bytes:
        //   bytes  0-7:  64-bit instruction word (opcode in byte 7 on LE)
        //   bytes  8-23: 128-bit payload (pin values)
        //   bytes 24-31: reserved (zeros)
        if write_pos + INSTR_WORD_SIZE > output.len() {
            return None;
        }

        // Zero the full 32-byte word first (clears reserved bytes + payload)
        output[write_pos..write_pos + INSTR_WORD_SIZE].fill(0);

        // Bytes 0-7: SET_PINS instruction
        let instr = FbcInstr::new(crate::fbc::FbcOpcode::SetPins, 0, 0);
        let instr_bytes = instr.to_u64().to_le_bytes();
        output[write_pos..write_pos + 8].copy_from_slice(&instr_bytes);

        // Bytes 8-23: first 128 bits (16 bytes) of 160-bit vector as payload
        output[write_pos + 8..write_pos + 24].copy_from_slice(&vector[..16]);

        // Bytes 24-31: already zeroed (reserved)
        write_pos += INSTR_WORD_SIZE;

        // Generate PATTERN_REP if repeat > 1
        if repeat_count > 1 {
            if write_pos + INSTR_WORD_SIZE > output.len() {
                return None;
            }

            output[write_pos..write_pos + INSTR_WORD_SIZE].fill(0);

            let rep_instr = FbcInstr::pattern_rep(repeat_count - 1);
            let rep_bytes = rep_instr.to_u64().to_le_bytes();
            output[write_pos..write_pos + 8].copy_from_slice(&rep_bytes);
            // Bytes 8-31: zeros (PATTERN_REP has no payload, count is in operand)
            write_pos += INSTR_WORD_SIZE;
        }
    }

    // Add HALT instruction
    if write_pos + INSTR_WORD_SIZE > output.len() {
        return None;
    }
    output[write_pos..write_pos + INSTR_WORD_SIZE].fill(0);
    let halt = FbcInstr::halt();
    let halt_bytes = halt.to_u64().to_le_bytes();
    output[write_pos..write_pos + 8].copy_from_slice(&halt_bytes);
    write_pos += INSTR_WORD_SIZE;

    Some(write_pos)
}

/// Iterator adapter for decompressor
impl<'a> Iterator for FbcDecompressor<'a> {
    type Item = ([u8; VECTOR_BYTES], u32);

    fn next(&mut self) -> Option<Self::Item> {
        FbcDecompressor::next(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompress_zero() {
        let data = [opcodes::VECTOR_ZERO, opcodes::END];
        let mut decompressor = FbcDecompressor::new(&data);

        let (vec, count) = decompressor.next().unwrap();
        assert_eq!(vec, [0u8; VECTOR_BYTES]);
        assert_eq!(count, 1);

        assert!(decompressor.next().is_none());
    }

    #[test]
    fn test_decompress_ones() {
        let data = [opcodes::VECTOR_ONES, opcodes::END];
        let mut decompressor = FbcDecompressor::new(&data);

        let (vec, count) = decompressor.next().unwrap();
        assert_eq!(vec, [0xFFu8; VECTOR_BYTES]);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_decompress_run() {
        // VECTOR_ZERO followed by RUN
        // Compiler stores (count - 1), so storing 99 means "repeat 100 times"
        let mut data = vec![opcodes::VECTOR_ZERO, opcodes::VECTOR_RUN];
        data.extend_from_slice(&99u32.to_le_bytes()); // Stored as count-1
        data.push(opcodes::END);

        let mut decompressor = FbcDecompressor::new(&data);

        // First: the zero vector
        let (vec, count) = decompressor.next().unwrap();
        assert_eq!(vec, [0u8; VECTOR_BYTES]);
        assert_eq!(count, 1);

        // Second: run of 100 (stored as 99, decompressor adds 1)
        let (vec, count) = decompressor.next().unwrap();
        assert_eq!(vec, [0u8; VECTOR_BYTES]);
        assert_eq!(count, 100);

        assert!(decompressor.next().is_none());
    }

    #[test]
    fn test_decompress_sparse() {
        // Set bit 0 to 1, bit 7 to 1
        let data = [
            opcodes::VECTOR_ZERO,
            opcodes::VECTOR_SPARSE,
            2,              // 2 changes
            (0 << 1) | 1,   // pin 0 = 1
            (7 << 1) | 1,   // pin 7 = 1
            opcodes::END,
        ];

        let mut decompressor = FbcDecompressor::new(&data);

        // First: zero
        decompressor.next().unwrap();

        // Second: sparse update
        let (vec, count) = decompressor.next().unwrap();
        assert_eq!(count, 1);
        assert_eq!(vec[0], 0b10000001); // bits 0 and 7 set
        assert_eq!(vec[1], 0);
    }

    #[test]
    fn test_decompress_full() {
        let mut data = vec![opcodes::VECTOR_FULL];
        let test_vec = [0xAA; VECTOR_BYTES];
        data.extend_from_slice(&test_vec);
        data.push(opcodes::END);

        let mut decompressor = FbcDecompressor::new(&data);

        let (vec, count) = decompressor.next().unwrap();
        assert_eq!(vec, test_vec);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_decompress_xor() {
        // Start with zeros, XOR with 0xFF to get ones
        let mut data = vec![opcodes::VECTOR_ZERO, opcodes::VECTOR_XOR];
        data.extend_from_slice(&[0xFF; VECTOR_BYTES]);
        data.push(opcodes::END);

        let mut decompressor = FbcDecompressor::new(&data);

        // Skip zero
        decompressor.next().unwrap();

        // XOR should give all ones
        let (vec, _) = decompressor.next().unwrap();
        assert_eq!(vec, [0xFF; VECTOR_BYTES]);
    }
}
