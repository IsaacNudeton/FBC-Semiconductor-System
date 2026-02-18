//! FBC Vector Compiler
//!
//! Compiles parsed vector programs to optimized FBC binary format.
//!
//! # Compression Strategy (ONETWO-derived)
//!
//! For each vector, choose the smallest encoding:
//!
//! | Condition                    | Encoding       | Size                |
//! |------------------------------|----------------|---------------------|
//! | vector == 0                  | VECTOR_ZERO    | 1 byte              |
//! | vector == all_ones           | VECTOR_ONES    | 1 byte              |
//! | vector == previous           | VECTOR_RUN     | (accumulate count)  |
//! | hamming(vector, prev) <= 15  | VECTOR_SPARSE  | 2 + N bytes         |
//! | otherwise                    | VECTOR_FULL    | 21 bytes            |
//!
//! The crossover at 15 is mathematically optimal:
//! - Sparse: 1 (opcode) + 1 (count) + N (indices) = 2 + N bytes
//! - Full: 1 (opcode) + 20 (data) = 21 bytes
//! - Crossover: 2 + N = 21 → N = 19 (but we use hamming distance, so 15 is safer)

use super::format::*;
use super::fvec::{FvecProgram, FvecVector};

/// Compiler configuration
#[derive(Debug, Clone)]
pub struct CompilerConfig {
    /// Minimum run length to use RUN opcode (default: 2)
    pub min_run_length: u32,
    /// Sparse encoding crossover (default: 15)
    pub sparse_crossover: usize,
    /// Enable XOR encoding optimization
    pub enable_xor: bool,
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            min_run_length: 2,
            sparse_crossover: SPARSE_CROSSOVER,
            enable_xor: false, // Not implemented yet
        }
    }
}

/// Vector compiler
pub struct VectorCompiler {
    config: CompilerConfig,
}

impl VectorCompiler {
    /// Create a new compiler with default settings
    pub fn new() -> Self {
        Self {
            config: CompilerConfig::default(),
        }
    }

    /// Create a compiler with custom configuration
    pub fn with_config(config: CompilerConfig) -> Self {
        Self { config }
    }

    /// Compile an FVEC program to FBC binary
    pub fn compile(&self, program: &FvecProgram) -> FbcFile {
        let mut data = Vec::new();
        let mut total_vectors: u64 = 0;
        let mut prev_vector = Vector::ZERO;
        let mut pending_run: Option<(Vector, u32)> = None;

        // Process each vector entry
        for entry in &program.vectors {
            let vec = entry.vector;
            let repeat = entry.repeat;

            // Handle runs of the same vector
            if let Some((run_vec, run_count)) = pending_run {
                if vec == run_vec {
                    // Extend the run
                    pending_run = Some((run_vec, run_count + repeat));
                    total_vectors += repeat as u64;
                    continue;
                } else {
                    // Flush the pending run
                    self.emit_run(&mut data, run_vec, run_count, &mut prev_vector);
                }
            }

            // Check if this vector starts a new run
            if repeat >= self.config.min_run_length {
                pending_run = Some((vec, repeat));
                total_vectors += repeat as u64;
            } else {
                // Emit individual vectors
                for _ in 0..repeat {
                    self.emit_vector(&mut data, vec, &mut prev_vector);
                    total_vectors += 1;
                }
            }
        }

        // Flush any remaining run
        if let Some((run_vec, run_count)) = pending_run {
            self.emit_run(&mut data, run_vec, run_count, &mut prev_vector);
        }

        // Emit END opcode
        data.push(OP_END);

        // Build the FBC file
        let header = FbcHeader {
            magic: FBC_MAGIC,
            version: FBC_VERSION,
            pin_count: PIN_COUNT as u8,
            flags: 0,
            num_vectors: total_vectors.min(u32::MAX as u64) as u32,
            compressed_size: data.len() as u32,
            vec_clock_hz: program.clock_hz,
            crc32: 0, // Will be calculated later
            _reserved: [0; 8],
        };

        let mut fbc = FbcFile {
            header,
            pin_config: program.pin_config.clone(),
            data,
        };

        // Calculate and set CRC32
        fbc.header.crc32 = fbc.calculate_crc();

        fbc
    }

    /// Emit a single vector using the optimal encoding
    fn emit_vector(&self, data: &mut Vec<u8>, vec: Vector, prev: &mut Vector) {
        // Check special cases first
        if vec == Vector::ZERO {
            data.push(OP_VECTOR_ZERO);
            *prev = vec;
            return;
        }

        if vec == Vector::ONES {
            data.push(OP_VECTOR_ONES);
            *prev = vec;
            return;
        }

        // Calculate hamming distance from previous
        let diff = vec.xor(prev);
        let toggles = diff.popcount();

        if toggles == 0 {
            // Same as previous - this shouldn't happen often due to run detection
            // but handle it anyway
            data.push(OP_VECTOR_RUN);
            data.extend_from_slice(&1u32.to_le_bytes());
            return;
        }

        if toggles <= self.config.sparse_crossover {
            // Sparse encoding: opcode + count + indices
            let indices = diff.ones_indices();
            data.push(OP_VECTOR_SPARSE);
            data.push(indices.len() as u8);

            // For each changed bit, encode: pin index + new value
            // We pack: (pin_index << 1) | new_value
            for idx in indices {
                let new_value = if vec.get_bit(idx as usize) { 1u8 } else { 0u8 };
                data.push((idx << 1) | new_value);
            }
        } else {
            // Full encoding
            data.push(OP_VECTOR_FULL);
            data.extend_from_slice(&vec.data);
        }

        *prev = vec;
    }

    /// Emit a run of identical vectors
    fn emit_run(&self, data: &mut Vec<u8>, vec: Vector, count: u32, prev: &mut Vector) {
        // First emit the vector itself
        if vec != *prev {
            self.emit_vector(data, vec, prev);
        }

        // Then emit run opcode if count > 1
        if count > 1 {
            // The run count is how many ADDITIONAL times to repeat
            // (the first one was already emitted by emit_vector)
            data.push(OP_VECTOR_RUN);
            data.extend_from_slice(&(count - 1).to_le_bytes());
        }
    }
}

impl Default for VectorCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Compile FVEC to FBC (convenience function)
pub fn compile_fvec(program: &FvecProgram) -> FbcFile {
    VectorCompiler::new().compile(program)
}

/// Decompiler: FBC binary → individual vectors
pub struct VectorDecompiler<'a> {
    data: &'a [u8],
    pos: usize,
    prev: Vector,
}

impl<'a> VectorDecompiler<'a> {
    /// Create a new decompiler
    pub fn new(fbc: &'a FbcFile) -> Self {
        Self {
            data: &fbc.data,
            pos: 0,
            prev: Vector::ZERO,
        }
    }

    /// Get the next vector (returns None at end)
    pub fn next(&mut self) -> Option<Vector> {
        if self.pos >= self.data.len() {
            return None;
        }

        let op = self.data[self.pos];
        self.pos += 1;

        match op {
            OP_NOP => self.next(), // Skip NOPs

            OP_VECTOR_FULL => {
                if self.pos + VECTOR_BYTES > self.data.len() {
                    return None;
                }
                let mut vec = Vector::ZERO;
                vec.data.copy_from_slice(&self.data[self.pos..self.pos + VECTOR_BYTES]);
                self.pos += VECTOR_BYTES;
                self.prev = vec;
                Some(vec)
            }

            OP_VECTOR_SPARSE => {
                if self.pos >= self.data.len() {
                    return None;
                }
                let count = self.data[self.pos] as usize;
                self.pos += 1;

                if self.pos + count > self.data.len() {
                    return None;
                }

                let mut vec = self.prev;
                for i in 0..count {
                    let encoded = self.data[self.pos + i];
                    let pin = (encoded >> 1) as usize;
                    let value = (encoded & 1) != 0;
                    if pin < PIN_COUNT {
                        vec.set_bit(pin, value);
                    }
                }
                self.pos += count;
                self.prev = vec;
                Some(vec)
            }

            OP_VECTOR_RUN => {
                // Run returns the SAME vector, but we need to handle repeat count
                // This is a simplified version - full implementation would track runs
                if self.pos + 4 > self.data.len() {
                    return None;
                }
                let _count = u32::from_le_bytes([
                    self.data[self.pos],
                    self.data[self.pos + 1],
                    self.data[self.pos + 2],
                    self.data[self.pos + 3],
                ]);
                self.pos += 4;
                // Return previous vector (caller should handle repeat count)
                Some(self.prev)
            }

            OP_VECTOR_ZERO => {
                self.prev = Vector::ZERO;
                Some(Vector::ZERO)
            }

            OP_VECTOR_ONES => {
                self.prev = Vector::ONES;
                Some(Vector::ONES)
            }

            OP_VECTOR_XOR => {
                if self.pos + VECTOR_BYTES > self.data.len() {
                    return None;
                }
                let mut xor_data = Vector::ZERO;
                xor_data.data.copy_from_slice(&self.data[self.pos..self.pos + VECTOR_BYTES]);
                self.pos += VECTOR_BYTES;
                let vec = self.prev.xor(&xor_data);
                self.prev = vec;
                Some(vec)
            }

            OP_END => None,

            _ => {
                // Unknown opcode - skip and try next
                self.next()
            }
        }
    }

    /// Get all vectors as a Vec (expands runs)
    pub fn to_vec(&mut self) -> Vec<Vector> {
        let mut vectors = Vec::new();
        let mut run_remaining = 0u32;
        let mut run_vector = Vector::ZERO;

        loop {
            // Handle pending run
            if run_remaining > 0 {
                vectors.push(run_vector);
                run_remaining -= 1;
                continue;
            }

            if self.pos >= self.data.len() {
                break;
            }

            let op = self.data[self.pos];

            if op == OP_VECTOR_RUN {
                self.pos += 1;
                if self.pos + 4 > self.data.len() {
                    break;
                }
                let count = u32::from_le_bytes([
                    self.data[self.pos],
                    self.data[self.pos + 1],
                    self.data[self.pos + 2],
                    self.data[self.pos + 3],
                ]);
                self.pos += 4;
                run_remaining = count;
                run_vector = self.prev;
                continue;
            }

            match self.next() {
                Some(vec) => vectors.push(vec),
                None => break,
            }
        }

        vectors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_empty() {
        let program = FvecProgram::default();
        let fbc = compile_fvec(&program);

        assert_eq!(fbc.header.magic, FBC_MAGIC);
        assert_eq!(fbc.header.num_vectors, 0);
        assert!(fbc.data.len() >= 1); // At least END opcode
    }

    #[test]
    fn test_compile_zero_ones() {
        let program = FvecProgram::from_str(r#"
            ZERO
            ONES
            ZERO
        "#).unwrap();

        let fbc = compile_fvec(&program);
        assert_eq!(fbc.header.num_vectors, 3);

        // Should use special opcodes
        assert!(fbc.data.contains(&OP_VECTOR_ZERO));
        assert!(fbc.data.contains(&OP_VECTOR_ONES));
    }

    #[test]
    fn test_compile_run() {
        let program = FvecProgram::from_str(r#"
            ZERO REPEAT 1000
        "#).unwrap();

        let fbc = compile_fvec(&program);
        assert_eq!(fbc.header.num_vectors, 1000);

        // Should use run encoding - much smaller than 1000 vectors
        // ZERO (1) + RUN (1 + 4) + END (1) = 7 bytes
        assert!(fbc.data.len() < 20);
    }

    #[test]
    fn test_compile_sparse() {
        // Create vectors that differ by only a few bits
        let program = FvecProgram::from_str(r#"
            ZERO
            TOGGLE 0 1 2
            TOGGLE 0
        "#).unwrap();

        let fbc = compile_fvec(&program);
        assert_eq!(fbc.header.num_vectors, 3);

        // Should use sparse encoding
        assert!(fbc.data.contains(&OP_VECTOR_SPARSE));
    }

    #[test]
    fn test_roundtrip() {
        let program = FvecProgram::from_str(r#"
            ZERO
            ONES
            ZERO REPEAT 5
            TOGGLE 0 10 20
            TOGGLE 0
        "#).unwrap();

        let fbc = compile_fvec(&program);

        // Decompress and verify
        let mut decomp = VectorDecompiler::new(&fbc);
        let vectors = decomp.to_vec();

        assert_eq!(vectors.len(), 9); // 1 + 1 + 5 + 1 + 1

        // Check specific vectors
        assert_eq!(vectors[0], Vector::ZERO);
        assert_eq!(vectors[1], Vector::ONES);
        assert_eq!(vectors[2], Vector::ZERO);
        assert_eq!(vectors[6], Vector::ZERO); // Still in the run

        // Check toggle results
        assert!(vectors[7].get_bit(0));
        assert!(vectors[7].get_bit(10));
        assert!(vectors[7].get_bit(20));
        assert!(!vectors[7].get_bit(1));

        assert!(!vectors[8].get_bit(0)); // Toggled back
        assert!(vectors[8].get_bit(10));
        assert!(vectors[8].get_bit(20));
    }

    #[test]
    fn test_compression_ratio() {
        // Create a program with good compression potential
        let program = FvecProgram::from_str(r#"
            ZERO REPEAT 10000
            ONES REPEAT 10000
        "#).unwrap();

        let fbc = compile_fvec(&program);
        let stats = fbc.stats();

        println!("{}", stats);

        // Should have high compression ratio
        assert!(stats.compression_ratio > 100.0);
    }

    #[test]
    fn test_crc() {
        let program = FvecProgram::from_str("ZERO REPEAT 100").unwrap();
        let fbc = compile_fvec(&program);

        assert!(fbc.validate_crc());
    }
}
