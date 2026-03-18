# Pattern Converter Migration Guide

**Purpose:** Document the path from legacy ATP/STIL/AVC patterns to FBC compressed format.

**Date:** March 13, 2026  
**Status:** ⚠️ **CRITICAL GAP IDENTIFIED**

---

## The Problem

Customer patterns in ATP/STIL/AVC format **cannot be converted to `.fbc`** (compressed FBC format).

```
ATP/STIL/AVC (customer patterns)
    │
    ▼
gui/src-tauri/c-engine/pc/  ✅ COMPLETE (14 C files)
    │
    ▼
.hex + .seq                 ✅ WORKS (40 bytes/vector, uncompressed)
    │
    ▼
Legacy Sonoma System        ✅ WORKS
    │
    ❌ NO PATH TO .fbc
    │
    ▼
.fbc format                 ❌ MISSING
    ▲
    │
.fvec ──▶ host/src/vector/  ✅ COMPLETE (Rust compiler)
```

---

## Why This Matters

### Format Comparison

| Format | Size per Vector | Compression | Used By |
|--------|----------------|-------------|---------|
| **`.hex`** | 40 bytes (fixed) | 1x (none) | Legacy Sonoma (Linux/kzhang_v2) |
| **`.fbc`** | 1-21 bytes (variable) | 4.8-710x | FBC System (bare-metal Rust) |

### `.fbc` Opcodes

| Opcode | Size | Use Case |
|--------|------|----------|
| `VECTOR_ZERO` (0x04) | 1 byte | All pins low |
| `VECTOR_ONES` (0x05) | 1 byte | All pins high |
| `VECTOR_RUN` (0x03) | 5 bytes | Repeat previous vector N times |
| `VECTOR_SPARSE` (0x02) | 2+N bytes | Small changes from previous (≤15 toggles) |
| `VECTOR_FULL` (0x01) | 21 bytes | Complete 160-bit vector |
| `VECTOR_XOR` (0x06) | 21 bytes | XOR with previous vector |

### Real Compression Numbers (Verified)

| File | Vectors | Uncompressed | Compressed | Ratio |
|------|---------|--------------|------------|-------|
| `test_core.fbc` | 2,759,718 | 55.2 MB | 77.7 KB | **710x** |
| `test_stil.fbc` | 18,539 | 371 KB | 77 KB | **4.8x** |

**Verified command:**
```bash
C:\Dev\projects\FBC-Semiconductor-System\host\target\release\fbc-vec.exe info reference/scratch/test_core.fbc
```

---

## What Exists Today

### ✅ Complete: C Engine Pattern Converter

**Location:** `gui/src-tauri/c-engine/pc/` (14 C files)

| File | Purpose | Lines |
|------|---------|-------|
| `pc.h` | Header — all types, API | 263 |
| `dll_api.c` | FFI API (handles) | 134 |
| `ir.c` | IR lifecycle, encoding | 134 |
| `crc32.c` | CRC32 checksum | 33 |
| `parse_atp.c` | ATP parser | 276 |
| `parse_stil_smart.c` | STIL smart parser | 403 |
| `parse_avc_smart.c` | AVC smart parser | 278 |
| `parse_pinmap.c` | Pin map parser (3 formats) | 154 |
| `gen_hex.c` | `.hex` generator (40B/vector) | 99 |
| `gen_seq.c` | `.seq` generator | 33 |
| `dc.h` | Device config header | 216 |
| `dc_api.c` | Device config FFI API | 193 |
| `dc_json.c` | JSON parser | (included) |
| `dc_gen.c` | Config file generators | (included) |
| `vendor/cJSON.c` | JSON library | (included) |

**Tauri Commands:**
- `pc_convert` — ATP/STIL/AVC → `.hex` + `.seq`
- `dc_generate_config` — Device JSON → PIN_MAP + .map + .lvl + .tim + .tp + PowerOn/Off.sh
- `dc_generate_file` — Generate single config file type
- `pc_version` — Version info

**GUI:** `PatternConverterPanel.tsx` (1019 lines, 3 tabs)

---

### ✅ Complete: Rust FBC Compiler

**Location:** `host/src/vector/`

| File | Purpose |
|------|---------|
| `format.rs` | `.fbc` binary format (header, opcodes, read/write) |
| `fvec.rs` | `.fvec` text format parser |
| `compiler.rs` | Compiles `.fvec` → `.fbc` (compressed) |

**CLI:** `fbc-vec compile input.fvec -o output.fbc`

**Compression Strategy:**
```rust
if vector == all_zeros      → VECTOR_ZERO (1 byte)
else if vector == all_ones   → VECTOR_ONES (1 byte)
else if vector == previous   → VECTOR_RUN (accumulate count)
else if hamming(vector, prev) <= 15 → VECTOR_SPARSE (2+N bytes)
else                           → VECTOR_FULL (21 bytes)
```

---

### ❌ Missing: ATP/STIL/AVC → `.fbc`

**Gap:** No converter from ATP/STIL/AVC directly to `.fbc` format.

**Impact:**
- Customer patterns stuck in legacy `.hex` format
- Cannot use FBC compression (4.8-710x smaller)
- Migration to FBC system requires manual conversion

---

## Implementation Options

### Option 1: Add `gen_fbc.c` to C Engine ⭐ **RECOMMENDED**

**New file:** `gui/src-tauri/c-engine/pc/gen_fbc.c`

```c
/*
 * gen_fbc.c — .fbc compressed binary generator
 *
 * Uses FBC opcodes:
 * - VECTOR_ZERO (0x04) for all-zero vectors
 * - VECTOR_ONES (0x05) for all-one vectors
 * - VECTOR_RUN (0x03) for repeated vectors
 * - VECTOR_SPARSE (0x02) for small changes
 * - VECTOR_FULL (0x01) for complete vectors
 */

int pc_gen_fbc(const PcPattern *p, const char *path) {
    FILE *f = fopen(path, "wb");
    if (!f) return PC_ERR_FILE;
    
    // Write header (32 bytes)
    FbcHeader header = {
        .magic = 0x00434246,  // "FBC\0"
        .version = 1,
        .num_vectors = p->num_vectors,
        // ...
    };
    fwrite(&header, sizeof(header), 1, f);
    
    // Write compressed vectors
    PcVector prev_vector = {0};
    for (int vi = 0; vi < p->num_vectors; vi++) {
        PcVector *vec = &p->vectors[vi];
        
        // Choose best encoding
        if (is_all_zeros(vec)) {
            uint8_t opcode = 0x04;  // VECTOR_ZERO
            fwrite(&opcode, 1, 1, f);
        } else if (is_all_ones(vec)) {
            uint8_t opcode = 0x05;  // VECTOR_ONES
            fwrite(&opcode, 1, 1, f);
        } else if (same_as_previous(vec, &prev_vector)) {
            // VECTOR_RUN logic
        } else if (hamming_distance(vec, &prev_vector) <= 15) {
            // VECTOR_SPARSE logic
        } else {
            // VECTOR_FULL logic
        }
        
        prev_vector = *vec;
    }
    
    fclose(f);
    return PC_OK;
}
```

**Pros:**
- Reuses existing C parsers (ATP/STIL/AVC)
- Same compression logic as Rust compiler
- Minimal new code (~200 lines)
- Updates to `dll_api.c` and `pc_ffi.rs` are trivial

**Cons:**
- Duplicates compression logic (Rust already has it)

**Effort:** 1-2 days

---

### Option 2: Add `.hex` → `.fbc` Converter in Rust

**New file:** `host/src/vector/hex_to_fbc.rs`

```rust
pub fn hex_to_fbc(hex_path: &str, fbc_path: &str) -> Result<()> {
    // Read .hex file (40 bytes/vector)
    let hex_data = std::fs::read(hex_path)?;
    
    // Parse vectors
    let vectors = parse_hex_vectors(&hex_data)?;
    
    // Compress to .fbc
    let mut compressor = VectorCompressor::new();
    for vec in vectors {
        compressor.emit(&vec);
    }
    
    // Write .fbc file
    compressor.write_to_file(fbc_path)?;
    Ok(())
}
```

**Pros:**
- Reuses Rust compression logic
- Works with existing C engine output

**Cons:**
- Intermediate format (inefficient)
- Requires two-step conversion: ATP → .hex → .fbc

**Effort:** 2-3 days

---

### Option 3: Port C Parsers to Rust

**New files:**
- `host/src/vector/parse_atp.rs`
- `host/src/vector/parse_stil_smart.rs`
- `host/src/vector/parse_avc_smart.rs`

**Pros:**
- Single Rust codebase
- No C FFI needed

**Cons:**
- Most work (~2000 lines of parser code)
- Duplicates existing C parsers

**Effort:** 1-2 weeks

---

## Recommended Path

**Option 1: Add `gen_fbc.c` to C engine**

**Why:**
1. Fastest implementation (1-2 days)
2. Reuses existing C parsers
3. Same compression as Rust compiler
4. Minimal FFI changes (just add `pc_dll_convert_to_fbc()`)

**Implementation Plan:**

### Step 1: Create `gen_fbc.c`
- Copy compression logic from `host/src/vector/compiler.rs`
- Implement VECTOR_ZERO, VECTOR_ONES, VECTOR_RUN, VECTOR_SPARSE, VECTOR_FULL
- Write `.fbc` binary format (matches `format.rs`)

### Step 2: Update `dll_api.c`
```c
PC_API int pc_dll_convert_to_fbc(int h, const char *fbc_path) {
    if (!SAFE(h)) return PC_ERR_HANDLE;
    PcPattern *p = &g_patterns[h];
    return pc_gen_fbc(p, fbc_path);
}
```

### Step 3: Update `pc_ffi.rs`
```rust
pub fn convert_to_fbc(&self, fbc_path: &str) -> Result<(), String> {
    let c_path = CString::new(fbc_path)?;
    let rc = unsafe { pc_dll_convert_to_fbc(self.handle, c_path.as_ptr()) };
    if rc != 0 { Err(self.last_error()) } else { Ok(()) }
}
```

### Step 4: Add Tauri Command
```rust
#[tauri::command]
pub async fn pc_convert_to_fbc(
    input_path: String,
    pinmap_path: Option<String>,
    fbc_output: String,
    format: Option<String>,
) -> Result<serde_json::Value, String> {
    // Similar to pc_convert, but outputs .fbc
}
```

### Step 5: Update GUI Panel
- Add "Output Format" dropdown: `.hex` | `.fbc`
- When `.fbc` selected, call `pc_convert_to_fbc`

---

## Testing Plan

### Test Files Available
- `reference/scratch/test_core.fbc` (77KB, 2.7M vectors)
- `reference/scratch/test_stil.fbc` (77KB, 18K vectors)
- `testplans/vectors/calibration_board_revB.fvec`

### Verification Steps

1. **Compression Ratio Test**
   ```bash
   # Convert ATP → .fbc
   fbc-vec compile test.fvec -o test.fbc
   
   # Check stats
   fbc-vec info test.fbc
   # Expected: compression ratio > 4.8x
   ```

2. **Round-Trip Test**
   ```bash
   # Decompress
   fbc-vec decompile test.fbc -o test_roundtrip.fvec
   
   # Compare
   diff test.fvec test_roundtrip.fvec
   # Expected: identical
   ```

3. **Hardware Test**
   ```bash
   # Load to board via GUI
   # Run test
   # Verify results match legacy .hex run
   ```

---

## Migration Impact

### For Customers

**Before (Legacy):**
```
ATP/STIL/AVC → .hex (55MB) → SD card → Load to board
```

**After (FBC):**
```
ATP/STIL/AVC → .fbc (77KB) → SD card → Load to board
```

**Benefits:**
- 710x smaller files
- Faster upload (77KB vs 55MB)
- Less SD card space
- Same test results

### For Development

**Current Workflow:**
```
ATP/STIL/AVC → .hex → Manual conversion → .fbc → Test
```

**After Fix:**
```
ATP/STIL/AVC → .fbc → Test
```

**Time Savings:**
- Eliminate manual conversion step
- One-click conversion in GUI
- Automatic compression

---

## Related Files

| File | Purpose |
|------|---------|
| `gui/src-tauri/c-engine/pc/pc.h` | C engine header |
| `gui/src-tauri/c-engine/pc/gen_hex.c` | `.hex` generator (reference) |
| `host/src/vector/format.rs` | `.fbc` format spec |
| `host/src/vector/compiler.rs` | Compression logic (reference) |
| `firmware/src/fbc_decompress.rs` | `.fbc` decompressor (firmware side) |
| `rtl/fbc_decoder.v` | FPGA bytecode decoder |

---

## Next Steps

1. **Implement `gen_fbc.c`** (1-2 days)
2. **Add FFI bindings** (2 hours)
3. **Add Tauri command** (1 hour)
4. **Update GUI** (2 hours)
5. **Test with real patterns** (1 day)

**Total Effort:** 3-5 days

**Priority:** 🔴 **HIGH** — Blocks customer migration to FBC system

---

**Questions?**
- See `host/src/vector/compiler.rs` for compression logic
- See `host/src/vector/format.rs` for `.fbc` binary format
- See `gui/src-tauri/c-engine/pc/gen_hex.c` for generator pattern
