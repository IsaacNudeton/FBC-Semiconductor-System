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
use super::fvec::FvecProgram;

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

/// Thermal accumulator for compile-time profiling
struct ThermalAccum {
    total_toggles: u64,
    total_active: u64,
    vectors_in_segment: u32,
    segment_start: u32,
    segments: Vec<ThermalSegment>,
}

impl ThermalAccum {
    fn new() -> Self {
        Self {
            total_toggles: 0,
            total_active: 0,
            vectors_in_segment: 0,
            segment_start: 0,
            segments: Vec::new(),
        }
    }

    fn add(&mut self, toggles: usize, active_pins: usize, repeat: u32) {
        for _ in 0..repeat {
            self.total_toggles += toggles as u64;
            self.total_active += active_pins as u64;
            self.vectors_in_segment += 1;

            if self.vectors_in_segment >= THERMAL_SEGMENT_SIZE {
                self.flush_segment();
            }
        }
    }

    fn flush_segment(&mut self) {
        if self.vectors_in_segment == 0 {
            return;
        }

        let avg_toggles = (self.total_toggles / self.vectors_in_segment as u64) as u8;
        let avg_active = (self.total_active / self.vectors_in_segment as u64) as u8;

        let power_level = match (avg_toggles, avg_active) {
            (t, a) if t > 40 && a > 60 => ThermalPowerLevel::High,
            (t, a) if t > 20 || a > 40 => ThermalPowerLevel::Medium,
            _ => ThermalPowerLevel::Low,
        };

        self.segments.push(ThermalSegment {
            vector_offset: self.segment_start,
            avg_toggle_rate: avg_toggles.min(160),
            avg_active_pins: avg_active.min(160),
            power_level,
            _reserved: 0,
        });

        self.segment_start += self.vectors_in_segment;
        self.total_toggles = 0;
        self.total_active = 0;
        self.vectors_in_segment = 0;
    }

    fn finish(mut self) -> Vec<ThermalSegment> {
        self.flush_segment();
        self.segments
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
        let mut thermal_prev = Vector::ZERO;
        let mut thermal = ThermalAccum::new();
        let mut pending_run: Option<(Vector, u32)> = None;

        // Process each vector entry
        for entry in &program.vectors {
            let vec = entry.vector;
            let repeat = entry.repeat;

            // Thermal analysis: XOR + popcount (free — already computed by compression)
            let diff = vec.xor(&thermal_prev);
            let toggles = diff.popcount();
            let active = vec.popcount();
            thermal.add(toggles, active, repeat);
            thermal_prev = vec;

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
                    pending_run = None;
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

        // Finalize thermal profile and append after OP_END
        let thermal_segments = thermal.finish();
        let profile = ThermalProfile { segments: thermal_segments };
        let profile_bytes = profile.to_bytes();
        data.extend_from_slice(&profile_bytes);

        // Build the FBC file
        let mut reserved = [0u8; 8];
        let seg_count = profile.segments.len() as u32;
        reserved[0..4].copy_from_slice(&seg_count.to_le_bytes());

        let flags = if !profile.segments.is_empty() {
            FLAG_THERMAL_PROFILE
        } else {
            0
        };

        let header = FbcHeader {
            magic: FBC_MAGIC,
            version: FBC_VERSION,
            pin_count: PIN_COUNT as u8,
            flags,
            num_vectors: total_vectors.min(u32::MAX as u64) as u32,
            compressed_size: data.len() as u32,
            vec_clock_hz: program.clock_hz,
            crc32: 0, // Will be calculated later
            _reserved: reserved,
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
        // Emit the vector itself if it differs from previous
        let emitted = if vec != *prev {
            self.emit_vector(data, vec, prev);
            true
        } else {
            false
        };

        // RUN count = additional repeats beyond the emitted vector
        // If we emitted the vector, run_count = count - 1
        // If vector was same as prev (not emitted), run_count = count
        let run_count = if emitted { count - 1 } else { count };
        if run_count > 0 {
            data.push(OP_VECTOR_RUN);
            data.extend_from_slice(&run_count.to_le_bytes());
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

            if op == OP_END {
                break; // thermal profile data follows — not vectors
            }

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

    #[test]
    fn test_thermal_profile() {
        // 2048 ZERO vectors + 2048 ONES vectors = 4 segments of 1024
        let program = FvecProgram::from_str(r#"
            ZERO REPEAT 2048
            ONES REPEAT 2048
        "#).unwrap();

        let fbc = compile_fvec(&program);

        // Header should have thermal flag
        assert!(fbc.has_thermal_profile());
        assert_eq!(fbc.thermal_segment_count(), 4);

        // Extract and verify profile
        let profile = fbc.thermal_profile().unwrap();
        assert_eq!(profile.segments.len(), 4);

        // Segment 0: 1024 ZERO vectors (zero toggles, zero active)
        assert_eq!(profile.segments[0].vector_offset, 0);
        assert_eq!(profile.segments[0].avg_toggle_rate, 0);
        assert_eq!(profile.segments[0].avg_active_pins, 0);
        assert_eq!(profile.segments[0].power_level, ThermalPowerLevel::Low);

        // Segment 1: 1024 more ZEROs (still zero toggles)
        assert_eq!(profile.segments[1].vector_offset, 1024);

        // Segment 2: first 1024 of ONES — transition from ZERO→ONES at boundary
        // Only the first vector has 160 toggles, rest have 0
        assert_eq!(profile.segments[2].vector_offset, 2048);
        assert_eq!(profile.segments[2].avg_active_pins, 160);

        // Segment 3: remaining ONES
        assert_eq!(profile.segments[3].vector_offset, 3072);
        assert_eq!(profile.segments[3].avg_active_pins, 160);

        // CRC should still validate with thermal data
        assert!(fbc.validate_crc());
    }

    #[test]
    fn test_thermal_profile_small() {
        // Fewer than 1024 vectors = 1 segment
        let program = FvecProgram::from_str("ZERO REPEAT 100").unwrap();
        let fbc = compile_fvec(&program);

        assert!(fbc.has_thermal_profile());
        assert_eq!(fbc.thermal_segment_count(), 1);

        let profile = fbc.thermal_profile().unwrap();
        assert_eq!(profile.segments[0].vector_offset, 0);
        assert_eq!(profile.segments[0].avg_toggle_rate, 0);
        assert_eq!(profile.segments[0].power_level, ThermalPowerLevel::Low);
    }
}
