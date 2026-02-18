//! FBC Vector Converter - STIL/AVC/ATP → FBC Binary
//!
//! This module handles vector format conversion for the FBC burn-in system.
//!
//! # Architecture
//!
//! ```text
//! Input Formats          Intermediate            Output
//! ─────────────          ────────────            ──────
//!  .fvec (text)  ─┐
//!  .stil (IEEE)  ─┼──▶  VectorProgram  ──▶  FbcBinary (.fbc)
//!  .avc  (93K)   ─┤          │                    │
//!  .atp  (ATP)   ─┘          │                    │
//!                            ▼                    ▼
//!                     (in-memory repr)       DMA to FPGA
//! ```
//!
//! # FBC Binary Format
//!
//! Designed for zero-copy DMA to FPGA BRAM:
//! - Header: Magic, version, pin count, vector count, clock freq, CRC32
//! - Pin config: 160 pin types (4 bits each = 80 bytes)
//! - Vectors: Compressed opcodes + data
//!
//! # Compression
//!
//! Uses ONETWO-derived constants:
//! - SPARSE_CROSSOVER = 15 toggles (mathematically optimal)
//! - RUN encoding for repeated vectors
//! - Special opcodes for all-0 and all-1 vectors

pub mod format;
pub mod fvec;
pub mod compiler;

pub use format::*;
pub use fvec::*;
pub use compiler::*;
