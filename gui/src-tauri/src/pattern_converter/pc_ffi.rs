//! pattern_converter/pc_ffi.rs — Rust FFI bindings for Pattern Converter C Engine
//!
//! Two APIs, both handle-based (integer handles, no structs cross boundary):
//!   - pc_* : Pattern conversion (ATP/STIL/AVC + PIN_MAP -> .hex/.seq)
//!   - dc_* : Device config generation (JSON/CSV + profile -> PIN_MAP + .map + .lvl + .tim + .tp + scripts)

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

/* ═══════════════════════════════════════════════════════════════
 * INPUT FORMAT ENUM (mirrors pc.h FMT_*)
 * ═══════════════════════════════════════════════════════════════ */

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputFormat {
    Auto = 0,
    Atp  = 1,
    Stil = 2,
    Avc  = 3,
}

/* ═══════════════════════════════════════════════════════════════
 * DEVICE CONFIG FILE TYPE ENUM (mirrors dc.h DC_FILE_*)
 * ═══════════════════════════════════════════════════════════════ */

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DcFileType {
    PinMap   = 0,
    Map      = 1,
    Lvl      = 2,
    Tim      = 3,
    Tp       = 4,
    PowerOn  = 5,
    PowerOff = 6,
}

/* ═══════════════════════════════════════════════════════════════
 * RAW FFI DECLARATIONS
 * ═══════════════════════════════════════════════════════════════ */

#[link(name = "pattern_converter")]
extern "C" {
    // ── Pattern Converter (dll_api.c) ──
    fn pc_create() -> c_int;
    fn pc_destroy(h: c_int);
    fn pc_dll_load_pinmap(h: c_int, path: *const c_char) -> c_int;
    fn pc_dll_load_input(h: c_int, path: *const c_char, format: c_int) -> c_int;
    fn pc_dll_convert(h: c_int, hex_path: *const c_char, seq_path: *const c_char) -> c_int;
    fn pc_dll_gen_fbc(h: c_int, fbc_path: *const c_char, vec_clock_hz: u32) -> c_int;
    fn pc_dll_num_signals(h: c_int) -> c_int;
    fn pc_dll_num_vectors(h: c_int) -> c_int;
    fn pc_dll_last_error(h: c_int) -> *const c_char;
    fn pc_dll_version() -> *const c_char;

    // ── Device Config Generator (dc_api.c) ──
    fn dc_create() -> c_int;
    fn dc_destroy(h: c_int);
    fn dc_load_profile(h: c_int, path_or_name: *const c_char) -> c_int;
    fn dc_load_device(h: c_int, path: *const c_char) -> c_int;
    fn dc_validate(h: c_int) -> c_int;
    fn dc_generate(h: c_int, output_dir: *const c_char) -> c_int;
    fn dc_gen_file(h: c_int, output_dir: *const c_char, file_type: c_int) -> c_int;
    fn dc_num_channels(h: c_int) -> c_int;
    fn dc_num_supplies(h: c_int) -> c_int;
    fn dc_num_steps(h: c_int) -> c_int;
    fn dc_last_error(h: c_int) -> *const c_char;
    fn dc_profile_name(h: c_int) -> *const c_char;
}

/* ═══════════════════════════════════════════════════════════════
 * SAFE WRAPPER: PatternConverter
 *
 * Converts ATP/STIL/AVC test patterns to Sonoma .hex/.seq binary format.
 * ═══════════════════════════════════════════════════════════════ */

pub struct PatternConverter {
    handle: c_int,
}

impl PatternConverter {
    pub fn new() -> Result<Self, String> {
        let h = unsafe { pc_create() };
        if h < 0 {
            return Err("Failed to create pattern converter handle (pool full)".into());
        }
        Ok(Self { handle: h })
    }

    pub fn load_pinmap(&self, path: &str) -> Result<(), String> {
        let c_path = CString::new(path).map_err(|e| format!("Invalid path: {}", e))?;
        let rc = unsafe { pc_dll_load_pinmap(self.handle, c_path.as_ptr()) };
        if rc != 0 {
            return Err(self.last_error());
        }
        Ok(())
    }

    pub fn load_input(&self, path: &str, format: InputFormat) -> Result<(), String> {
        let c_path = CString::new(path).map_err(|e| format!("Invalid path: {}", e))?;
        let rc = unsafe { pc_dll_load_input(self.handle, c_path.as_ptr(), format as c_int) };
        if rc != 0 {
            return Err(self.last_error());
        }
        Ok(())
    }

    pub fn convert(&self, hex_path: Option<&str>, seq_path: Option<&str>) -> Result<(), String> {
        let c_hex = hex_path
            .map(|p| CString::new(p).unwrap())
            .unwrap_or_else(|| CString::new("").unwrap());
        let c_seq = seq_path
            .map(|p| CString::new(p).unwrap())
            .unwrap_or_else(|| CString::new("").unwrap());

        let hex_ptr = if hex_path.is_some() { c_hex.as_ptr() } else { std::ptr::null() };
        let seq_ptr = if seq_path.is_some() { c_seq.as_ptr() } else { std::ptr::null() };

        let rc = unsafe { pc_dll_convert(self.handle, hex_ptr, seq_ptr) };
        if rc != 0 {
            return Err(self.last_error());
        }
        Ok(())
    }

    /// Generate compressed .fbc binary output.
    /// vec_clock_hz: vector clock frequency (0 = default 100MHz)
    pub fn gen_fbc(&self, fbc_path: &str, vec_clock_hz: u32) -> Result<(), String> {
        let c_path = CString::new(fbc_path).map_err(|e| format!("Invalid path: {}", e))?;
        let rc = unsafe { pc_dll_gen_fbc(self.handle, c_path.as_ptr(), vec_clock_hz) };
        if rc != 0 {
            return Err(self.last_error());
        }
        Ok(())
    }

    pub fn num_signals(&self) -> i32 {
        unsafe { pc_dll_num_signals(self.handle) }
    }

    pub fn num_vectors(&self) -> i32 {
        unsafe { pc_dll_num_vectors(self.handle) }
    }

    pub fn last_error(&self) -> String {
        unsafe {
            let ptr = pc_dll_last_error(self.handle);
            if ptr.is_null() {
                return "Unknown error".into();
            }
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }

    pub fn version() -> String {
        unsafe {
            let ptr = pc_dll_version();
            if ptr.is_null() {
                return "unknown".into();
            }
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }
}

impl Drop for PatternConverter {
    fn drop(&mut self) {
        unsafe { pc_destroy(self.handle) }
    }
}

/* ═══════════════════════════════════════════════════════════════
 * SAFE WRAPPER: DeviceConfigGenerator
 *
 * Generates device config files (PIN_MAP, .map, .lvl, .tim, .tp, power scripts)
 * from device JSON/CSV + tester profile.
 * ═══════════════════════════════════════════════════════════════ */

pub struct DeviceConfigGenerator {
    handle: c_int,
}

impl DeviceConfigGenerator {
    pub fn new() -> Result<Self, String> {
        let h = unsafe { dc_create() };
        if h < 0 {
            return Err("Failed to create device config handle (pool full)".into());
        }
        Ok(Self { handle: h })
    }

    /// Load tester profile. Pass "sonoma" for built-in, or a file path for custom.
    pub fn load_profile(&self, path_or_name: &str) -> Result<(), String> {
        let c_str = CString::new(path_or_name).map_err(|e| format!("Invalid name: {}", e))?;
        let rc = unsafe { dc_load_profile(self.handle, c_str.as_ptr()) };
        if rc != 0 {
            return Err(self.last_error());
        }
        Ok(())
    }

    /// Load device config from JSON file.
    pub fn load_device(&self, path: &str) -> Result<(), String> {
        let c_path = CString::new(path).map_err(|e| format!("Invalid path: {}", e))?;
        let rc = unsafe { dc_load_device(self.handle, c_path.as_ptr()) };
        if rc != 0 {
            return Err(self.last_error());
        }
        Ok(())
    }

    /// Validate device config against profile (channel bounds, supply existence).
    pub fn validate(&self) -> Result<(), String> {
        let rc = unsafe { dc_validate(self.handle) };
        if rc != 0 {
            return Err(self.last_error());
        }
        Ok(())
    }

    /// Generate ALL device config files to output_dir.
    pub fn generate_all(&self, output_dir: &str) -> Result<(), String> {
        let c_dir = CString::new(output_dir).map_err(|e| format!("Invalid path: {}", e))?;
        let rc = unsafe { dc_generate(self.handle, c_dir.as_ptr()) };
        if rc != 0 {
            return Err(self.last_error());
        }
        Ok(())
    }

    /// Generate a single file type to output_dir.
    pub fn generate_file(&self, output_dir: &str, file_type: DcFileType) -> Result<(), String> {
        let c_dir = CString::new(output_dir).map_err(|e| format!("Invalid path: {}", e))?;
        let rc = unsafe { dc_gen_file(self.handle, c_dir.as_ptr(), file_type as c_int) };
        if rc != 0 {
            return Err(self.last_error());
        }
        Ok(())
    }

    pub fn num_channels(&self) -> i32 {
        unsafe { dc_num_channels(self.handle) }
    }

    pub fn num_supplies(&self) -> i32 {
        unsafe { dc_num_supplies(self.handle) }
    }

    pub fn num_steps(&self) -> i32 {
        unsafe { dc_num_steps(self.handle) }
    }

    pub fn last_error(&self) -> String {
        unsafe {
            let ptr = dc_last_error(self.handle);
            if ptr.is_null() {
                return "Unknown error".into();
            }
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }

    pub fn profile_name(&self) -> String {
        unsafe {
            let ptr = dc_profile_name(self.handle);
            if ptr.is_null() {
                return String::new();
            }
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }
}

impl Drop for DeviceConfigGenerator {
    fn drop(&mut self) {
        unsafe { dc_destroy(self.handle) }
    }
}
