//! database/mod.rs — Database Layer
//!
//! Uses LRM custom C database engine via FFI.

pub mod lrm_ffi;

pub use lrm_ffi::*;
