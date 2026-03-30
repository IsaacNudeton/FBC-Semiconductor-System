//! FFI wrappers for the C pattern converter and device config APIs.
//!
//! Two RAII handles: `PcHandle` (pattern conversion) and `DcHandle` (device config generation).
//! Both wrap integer handles into a static C-side pool of 16 slots.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint};

// ---------------------------------------------------------------------------
// C FFI declarations
// ---------------------------------------------------------------------------

extern "C" {
    // Pattern converter DLL API (dll_api.c)
    fn pc_create() -> c_int;
    fn pc_destroy(h: c_int);
    fn pc_dll_load_pinmap(h: c_int, path: *const c_char) -> c_int;
    fn pc_dll_load_input(h: c_int, path: *const c_char, format: c_int) -> c_int;
    fn pc_dll_convert(h: c_int, hex_path: *const c_char, seq_path: *const c_char) -> c_int;
    fn pc_dll_gen_fbc(h: c_int, fbc_path: *const c_char, vec_clock_hz: c_uint) -> c_int;
    fn pc_dll_num_signals(h: c_int) -> c_int;
    fn pc_dll_num_vectors(h: c_int) -> c_int;
    fn pc_dll_last_error(h: c_int) -> *const c_char;
    fn pc_dll_version() -> *const c_char;

    // Device config API (dc_api.c)
    fn dc_create() -> c_int;
    fn dc_destroy(h: c_int);
    fn dc_load_profile(h: c_int, path_or_name: *const c_char) -> c_int;
    fn dc_load_device(h: c_int, path: *const c_char) -> c_int;
    fn dc_validate(h: c_int) -> c_int;
    fn dc_generate(h: c_int, out_dir: *const c_char) -> c_int;
    fn dc_gen_file(h: c_int, output_dir: *const c_char, file_type: c_int) -> c_int;
    fn dc_num_channels(h: c_int) -> c_int;
    fn dc_num_supplies(h: c_int) -> c_int;
    fn dc_num_steps(h: c_int) -> c_int;
    fn dc_profile_name(h: c_int) -> *const c_char;
    fn dc_last_error(h: c_int) -> *const c_char;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Return code: success.
const PC_OK: c_int = 0;

/// Input format auto-detection.
pub const FMT_AUTO: c_int = 0;
/// ATP input format.
pub const FMT_ATP: c_int = 1;
/// STIL input format.
pub const FMT_STIL: c_int = 2;
/// AVC input format.
pub const FMT_AVC: c_int = 3;

/// Device config file types for `dc_gen_file`.
pub const DC_FILE_PINMAP: c_int = 0;
pub const DC_FILE_MAP: c_int = 1;
pub const DC_FILE_LVL: c_int = 2;
pub const DC_FILE_TIM: c_int = 3;
pub const DC_FILE_TP: c_int = 4;
pub const DC_FILE_POWER_ON: c_int = 5;
pub const DC_FILE_POWER_OFF: c_int = 6;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_cstring(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|e| format!("invalid string (interior NUL): {e}"))
}

/// Read a C string pointer into an owned `String`. Returns `"(null)"` for null pointers.
unsafe fn read_c_str(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return "(null)".to_string();
    }
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}

// ---------------------------------------------------------------------------
// PcHandle — pattern converter
// ---------------------------------------------------------------------------

/// RAII wrapper around a C-side pattern converter handle.
pub struct PcHandle {
    handle: c_int,
}

impl PcHandle {
    /// Allocate a new pattern converter slot (max 16 concurrent).
    pub fn new() -> Result<Self, String> {
        let h = unsafe { pc_create() };
        if h < 0 {
            return Err(format!("pc_create failed (rc={h}), pool may be full"));
        }
        Ok(Self { handle: h })
    }

    /// Load a pin-map file (maps signal names to channel indices).
    pub fn load_pinmap(&self, path: &str) -> Result<(), String> {
        let c_path = to_cstring(path)?;
        let rc = unsafe { pc_dll_load_pinmap(self.handle, c_path.as_ptr()) };
        if rc != PC_OK {
            return Err(self.last_error_string());
        }
        Ok(())
    }

    /// Load an input pattern file. `format` should be one of `FMT_AUTO`, `FMT_ATP`,
    /// `FMT_STIL`, or `FMT_AVC`.
    pub fn load_input(&self, path: &str, format: c_int) -> Result<(), String> {
        let c_path = to_cstring(path)?;
        let rc = unsafe { pc_dll_load_input(self.handle, c_path.as_ptr(), format) };
        if rc != PC_OK {
            return Err(self.last_error_string());
        }
        Ok(())
    }

    /// Load input with auto-detected format.
    pub fn load_input_auto(&self, path: &str) -> Result<(), String> {
        self.load_input(path, FMT_AUTO)
    }

    /// Convert the loaded pattern to `.hex` and `.seq` output files.
    pub fn convert(&self, hex_path: &str, seq_path: &str) -> Result<(), String> {
        let c_hex = to_cstring(hex_path)?;
        let c_seq = to_cstring(seq_path)?;
        let rc = unsafe { pc_dll_convert(self.handle, c_hex.as_ptr(), c_seq.as_ptr()) };
        if rc != PC_OK {
            return Err(self.last_error_string());
        }
        Ok(())
    }

    /// Generate a compressed `.fbc` file from the loaded pattern.
    pub fn gen_fbc(&self, fbc_path: &str, vec_clock_hz: u32) -> Result<(), String> {
        let c_path = to_cstring(fbc_path)?;
        let rc = unsafe { pc_dll_gen_fbc(self.handle, c_path.as_ptr(), vec_clock_hz as c_uint) };
        if rc != PC_OK {
            return Err(self.last_error_string());
        }
        Ok(())
    }

    /// Number of signals in the loaded pattern.
    pub fn num_signals(&self) -> i32 {
        unsafe { pc_dll_num_signals(self.handle) }
    }

    /// Number of vectors in the loaded pattern.
    pub fn num_vectors(&self) -> i32 {
        unsafe { pc_dll_num_vectors(self.handle) }
    }

    /// Last error message from the C library for this handle.
    pub fn last_error_string(&self) -> String {
        unsafe { read_c_str(pc_dll_last_error(self.handle)) }
    }

    /// Library version string.
    pub fn version() -> String {
        unsafe { read_c_str(pc_dll_version()) }
    }
}

impl Drop for PcHandle {
    fn drop(&mut self) {
        unsafe {
            pc_destroy(self.handle);
        }
    }
}

// ---------------------------------------------------------------------------
// DcHandle — device config generator
// ---------------------------------------------------------------------------

/// RAII wrapper around a C-side device config handle.
pub struct DcHandle {
    handle: c_int,
}

impl DcHandle {
    /// Allocate a new device config slot (max 16 concurrent).
    pub fn new() -> Result<Self, String> {
        let h = unsafe { dc_create() };
        if h < 0 {
            return Err(format!("dc_create failed (rc={h}), pool may be full"));
        }
        Ok(Self { handle: h })
    }

    /// Load a tester profile by built-in name (e.g. `"sonoma"`) or file path.
    pub fn load_profile(&self, path_or_name: &str) -> Result<(), String> {
        let c_str = to_cstring(path_or_name)?;
        let rc = unsafe { dc_load_profile(self.handle, c_str.as_ptr()) };
        if rc != PC_OK {
            return Err(self.last_error_string());
        }
        Ok(())
    }

    /// Load a device configuration JSON file.
    pub fn load_device(&self, path: &str) -> Result<(), String> {
        let c_path = to_cstring(path)?;
        let rc = unsafe { dc_load_device(self.handle, c_path.as_ptr()) };
        if rc != PC_OK {
            return Err(self.last_error_string());
        }
        Ok(())
    }

    /// Validate the loaded profile + device configuration against constraints.
    pub fn validate(&self) -> Result<(), String> {
        let rc = unsafe { dc_validate(self.handle) };
        if rc != PC_OK {
            return Err(self.last_error_string());
        }
        Ok(())
    }

    /// Generate all 7 device files into `out_dir`.
    pub fn generate(&self, out_dir: &str) -> Result<(), String> {
        let c_dir = to_cstring(out_dir)?;
        let rc = unsafe { dc_generate(self.handle, c_dir.as_ptr()) };
        if rc != PC_OK {
            return Err(self.last_error_string());
        }
        Ok(())
    }

    /// Generate a single device file. `file_type` is one of `DC_FILE_PINMAP` through
    /// `DC_FILE_POWER_OFF`.
    pub fn gen_file(&self, output_dir: &str, file_type: c_int) -> Result<(), String> {
        let c_dir = to_cstring(output_dir)?;
        let rc = unsafe { dc_gen_file(self.handle, c_dir.as_ptr(), file_type) };
        if rc != PC_OK {
            return Err(self.last_error_string());
        }
        Ok(())
    }

    /// Number of channels defined by the loaded profile.
    pub fn num_channels(&self) -> i32 {
        unsafe { dc_num_channels(self.handle) }
    }

    /// Number of power supplies defined by the loaded profile.
    pub fn num_supplies(&self) -> i32 {
        unsafe { dc_num_supplies(self.handle) }
    }

    /// Number of test steps in the loaded device configuration.
    pub fn num_steps(&self) -> i32 {
        unsafe { dc_num_steps(self.handle) }
    }

    /// Name of the loaded tester profile.
    pub fn profile_name(&self) -> String {
        unsafe { read_c_str(dc_profile_name(self.handle)) }
    }

    /// Last error message from the C library for this handle.
    pub fn last_error_string(&self) -> String {
        unsafe { read_c_str(dc_last_error(self.handle)) }
    }
}

impl Drop for DcHandle {
    fn drop(&mut self) {
        unsafe {
            dc_destroy(self.handle);
        }
    }
}
