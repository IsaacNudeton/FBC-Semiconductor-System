//! database/lrm_ffi.rs — Rust FFI bindings for LRM C Database Engine
//!
//! This wraps the custom C database engine (4KB pages, B-tree, WAL)
//! for use in Rust via FFI.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

/* ═══════════════════════════════════════════════════════════════
 * OPAQUE TYPES
 * ═══════════════════════════════════════════════════════════════ */

#[repr(C)]
pub struct LrmDatabase {
    _private: [u8; 0],
}

#[repr(C)]
pub struct LrmResult {
    _private: [u8; 0],
}

/* ═══════════════════════════════════════════════════════════════
 * FFI DECLARATIONS
 * ═══════════════════════════════════════════════════════════════ */

#[link(name = "lrm_db")]
extern "C" {
    /* Lifecycle */
    fn lrm_create(path: *const c_char) -> *mut LrmDatabase;
    fn lrm_destroy(db: *mut LrmDatabase);

    /* Query API */
    fn lrm_query(db: *mut LrmDatabase, query: *const c_char) -> *mut LrmResult;
    fn lrm_free_result(result: *mut LrmResult);
    fn lrm_result_json(result: *mut LrmResult) -> *const c_char;
    fn lrm_result_count(result: *mut LrmResult) -> c_int;
    fn lrm_result_error(result: *mut LrmResult) -> c_int;

    /* Controllers */
    fn lrm_get_controllers(db: *mut LrmDatabase) -> *mut LrmResult;
    fn lrm_get_controller(db: *mut LrmDatabase, id: *const c_char) -> *mut LrmResult;
    fn lrm_insert_controller(
        db: *mut LrmDatabase,
        id: *const c_char,
        ip_address: *const c_char,
        mac_address: *const c_char,
        status: c_int,
        firmware_version: *const c_char,
    ) -> c_int;

    /* Boards */
    fn lrm_get_boards(db: *mut LrmDatabase) -> *mut LrmResult;
    fn lrm_get_boards_by_lot(db: *mut LrmDatabase, lot_id: *const c_char) -> *mut LrmResult;

    /* LOTs */
    fn lrm_get_lots(db: *mut LrmDatabase) -> *mut LrmResult;
    fn lrm_get_lot(db: *mut LrmDatabase, id: *const c_char) -> *mut LrmResult;
    fn lrm_insert_lot(
        db: *mut LrmDatabase,
        id: *const c_char,
        project_id: *const c_char,
        system_id: *const c_char,
        lot_number: *const c_char,
        customer_lot: *const c_char,
        expected_qty: c_int,
    ) -> c_int;
    fn lrm_advance_lot(db: *mut LrmDatabase, lot_id: *const c_char) -> c_int;

    /* Schema */
    fn lrm_init_schema(db: *mut LrmDatabase) -> c_int;

    /* Error handling */
    fn lrm_get_error(error_code: c_int) -> *const c_char;
}

/* ═══════════════════════════════════════════════════════════════
 * RESULT CODES
 * ═══════════════════════════════════════════════════════════════ */

pub const LRM_OK: c_int = 0;
pub const LRM_ERR: c_int = -1;
pub const LRM_NOT_FOUND: c_int = -2;
pub const LRM_EXISTS: c_int = -3;

/* ═══════════════════════════════════════════════════════════════
 * RUST WRAPPER
 * ═══════════════════════════════════════════════════════════════ */

pub struct Database {
    inner: *mut LrmDatabase,
}

unsafe impl Send for Database {}
unsafe impl Sync for Database {}

impl Database {
    /* Create or open database */
    pub fn open(path: &str) -> Result<Self, String> {
        let c_path = CString::new(path).map_err(|e| format!("Invalid path: {}", e))?;

        unsafe {
            let db = lrm_create(c_path.as_ptr());
            if db.is_null() {
                return Err("Failed to create database".to_string());
            }

            Ok(Database { inner: db })
        }
    }

    /* Initialize schema (creates tables if needed) */
    pub fn init_schema(&self) -> Result<(), String> {
        unsafe {
            let rc = lrm_init_schema(self.inner);
            if rc != LRM_OK {
                let err = CStr::from_ptr(lrm_get_error(rc)).to_string_lossy().into_owned();
                return Err(format!("Schema init failed: {}", err));
            }
            Ok(())
        }
    }

    /* Generic query */
    pub fn query(&self, sql: &str) -> Result<QueryResult, String> {
        let c_sql = CString::new(sql).map_err(|e| format!("Invalid query: {}", e))?;

        unsafe {
            let result = lrm_query(self.inner, c_sql.as_ptr());
            if result.is_null() {
                return Err("Query failed".to_string());
            }

            let json_ptr = lrm_result_json(result);
            let json = if json_ptr.is_null() {
                "{}".to_string()
            } else {
                CStr::from_ptr(json_ptr).to_string_lossy().into_owned()
            };

            let count = lrm_result_count(result);
            let error = lrm_result_error(result);

            lrm_free_result(result);

            if error != LRM_OK {
                let err = CStr::from_ptr(lrm_get_error(error)).to_string_lossy().into_owned();
                return Err(format!("Query error: {}", err));
            }

            Ok(QueryResult { json, count: count as usize })
        }
    }

    /* Get all controllers */
    pub fn get_controllers(&self) -> Result<QueryResult, String> {
        unsafe {
            let result = lrm_get_controllers(self.inner);
            if result.is_null() {
                return Err("Failed to get controllers".to_string());
            }

            let json_ptr = lrm_result_json(result);
            let json = if json_ptr.is_null() {
                "{}".to_string()
            } else {
                CStr::from_ptr(json_ptr).to_string_lossy().into_owned()
            };

            let count = lrm_result_count(result);
            lrm_free_result(result);

            Ok(QueryResult { json, count: count as usize })
        }
    }

    /* Insert controller */
    pub fn insert_controller(
        &self,
        id: &str,
        ip_address: &str,
        mac_address: &str,
        status: i32,
        firmware_version: &str,
    ) -> Result<(), String> {
        let c_id = CString::new(id).unwrap();
        let c_ip = CString::new(ip_address).unwrap();
        let c_mac = CString::new(mac_address).unwrap();
        let c_fw = CString::new(firmware_version).unwrap();

        unsafe {
            let rc = lrm_insert_controller(
                self.inner,
                c_id.as_ptr(),
                c_ip.as_ptr(),
                c_mac.as_ptr(),
                status,
                c_fw.as_ptr(),
            );

            if rc != LRM_OK {
                let err = CStr::from_ptr(lrm_get_error(rc)).to_string_lossy().into_owned();
                return Err(format!("Insert failed: {}", err));
            }
            Ok(())
        }
    }

    /* Get all LOTs */
    pub fn get_lots(&self) -> Result<QueryResult, String> {
        unsafe {
            let result = lrm_get_lots(self.inner);
            if result.is_null() {
                return Err("Failed to get LOTs".to_string());
            }

            let json_ptr = lrm_result_json(result);
            let json = if json_ptr.is_null() {
                "{}".to_string()
            } else {
                CStr::from_ptr(json_ptr).to_string_lossy().into_owned()
            };

            let count = lrm_result_count(result);
            lrm_free_result(result);

            Ok(QueryResult { json, count: count as usize })
        }
    }

    /* Get all boards */
    pub fn get_boards(&self) -> Result<QueryResult, String> {
        unsafe {
            let result = lrm_get_boards(self.inner);
            if result.is_null() {
                return Err("Failed to get boards".to_string());
            }

            let json_ptr = lrm_result_json(result);
            let json = if json_ptr.is_null() {
                "{}".to_string()
            } else {
                CStr::from_ptr(json_ptr).to_string_lossy().into_owned()
            };

            let count = lrm_result_count(result);
            lrm_free_result(result);

            Ok(QueryResult { json, count: count as usize })
        }
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        unsafe {
            lrm_destroy(self.inner);
        }
    }
}

/* Query result */
pub struct QueryResult {
    pub json: String,
    pub count: usize,
}

/* Controller status enum */
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(i32)]
pub enum ControllerStatus {
    Offline = 0,
    Online = 1,
    Testing = 2,
    Error = 3,
}

/* LOT step enum */
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(i32)]
pub enum LotStep {
    Received = 0,
    Setup = 1,
    Loading = 2,
    BurnIn = 3,
    Readpoint = 4,
    Unloading = 5,
    Shipping = 6,
    Complete = 7,
}
