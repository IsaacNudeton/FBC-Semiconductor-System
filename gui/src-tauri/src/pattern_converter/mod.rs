//! pattern_converter/mod.rs — Pattern Converter + Device Config Generator
//!
//! Two pipelines powered by zero-dependency C engine:
//!   Pipeline 1: ATP/STIL/AVC + PIN_MAP -> .hex/.seq (pattern conversion)
//!   Pipeline 2: DeviceJSON/CSV + TesterProfile -> PIN_MAP + config files (device config)

pub mod pc_ffi;
pub mod pin_extractor;

pub use pc_ffi::*;
pub use pin_extractor::*;
